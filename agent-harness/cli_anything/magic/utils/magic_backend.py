"""Backend integration for Magic Studio (`magicstudio-shell` and live Studio sockets).

Handles:
1. Finding the native `magicstudio-shell` binary (repo target/debug, installed ~/.local/share/magic-studio/bin, or PATH).
2. Discovering live Studio instances via `$XDG_RUNTIME_DIR/magicstudio-studio/*.sock` (`ui.feedback`).
3. Communicating with a live Studio window over its Unix domain socket or `--control` TCP port.
4. Executing headless engine commands via `magicstudio-shell --headless` (with no dependency on `mc`).
5. Launching or focusing a Studio window (`magic-cli studio open [project]`) and waiting for `ui_ready == true`.
"""

from __future__ import annotations

import json
from functools import lru_cache
import os
import shutil
import socket
import subprocess
import sys
import time
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path("/run/media/jesse/Speedy/Work/omhas-animation")


def find_magicstudio_shell() -> str:
    """Locate the native `magicstudio-shell` binary."""
    env_override = os.environ.get("MAGICSTUDIO_SHELL_BIN")
    if env_override and Path(env_override).is_file():
        return str(Path(env_override).resolve())

    candidates = [
        Path(sys.argv[0]).absolute().parent / "magicstudio-shell",
        Path.home() / ".local" / "share" / "magic-studio" / "bin" / "magicstudio-shell",
        Path.home() / ".local" / "bin" / "magicstudio-shell",
    ]
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate.resolve())

    candidates.append(REPO_ROOT / "target" / "debug" / "magicstudio-shell")
    for candidate in candidates[-1:]:
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate.resolve())

    which_path = shutil.which("magicstudio-shell")
    if which_path:
        return which_path

    raise RuntimeError(
        "Magic Studio (`magicstudio-shell`) is not installed or built. Build it with:\n"
        "  CARGO_BUILD_JOBS=1 cargo build -p magicstudio-shell"
    )


def studio_socket_dir() -> Path | None:
    """Return the runtime directory holding live Studio Unix sockets."""
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not runtime and sys.platform == "linux":
        # MCP clients may sanitize the environment; the per-user runtime remains valid.
        fallback = Path(f"/run/user/{os.getuid()}")
        if fallback.is_dir() and fallback.stat().st_uid == os.getuid():
            runtime = str(fallback)
    return Path(runtime) / "magicstudio-studio" if runtime else None


class CommandOutcomeUnknown(RuntimeError):
    """The connection ended after submission; this does not cancel engine work."""


@lru_cache(maxsize=1)
def _long_running_commands() -> frozenset[str]:
    return frozenset(spec["id"] for spec in list_engine_commands() if spec["long_running"])


def command_timeout(cmd_id: str) -> float:
    override = os.environ.get("MAGICSTUDIO_BRIDGE_TIMEOUT_MS")
    if override:
        try:
            value = float(override) / 1000
            if value <= 0:
                raise ValueError()
            return value
        except ValueError:
            raise ValueError("MAGICSTUDIO_BRIDGE_TIMEOUT_MS must be a positive number") from None
    return 3600.0 if cmd_id in _long_running_commands() else 30.0


def _send_jsonl_line(sock: socket.socket, cmd_id: str, params: dict[str, Any]) -> dict[str, Any]:
    payload = json.dumps({"id": cmd_id, "params": params}) + "\n"
    try:
        sock.sendall(payload.encode("utf-8"))
        buf = b""
        while not buf.endswith(b"\n"):
            chunk = sock.recv(65536)
            if not chunk:
                raise ConnectionError("Studio closed the connection before replying")
            buf += chunk
    except OSError as exc:
        raise CommandOutcomeUnknown(
            f"{cmd_id}: reply unavailable after submission ({exc}). Outcome unknown; "
            "the operation may still be running. It was not cancelled or retried. "
            "Inspect ui.feedback and the operation's saved result before retrying."
        ) from exc
    resp = json.loads(buf.decode("utf-8").strip())
    if resp.get("ok") is True:
        res = resp.get("result")
        return res if isinstance(res, dict) else {"value": res}
    raise RuntimeError(str(resp.get("error") or "unknown Studio bridge error"))


def call_unix_socket(
    sock_path: str | Path, cmd_id: str, params: dict[str, Any] | None = None,
    timeout: float | None = None,
) -> dict[str, Any]:
    path_str = str(sock_path).removeprefix("unix:")
    req = dict(params or {})
    if "_client" not in req and os.environ.get("HERDR_PANE_ID"):
        req["_client"] = f"herdr:{os.environ['HERDR_PANE_ID']}"
    reply_timeout = command_timeout(cmd_id) if timeout is None else timeout
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(min(reply_timeout, 5.0))
        sock.connect(path_str)
        sock.settimeout(reply_timeout)
        result = _send_jsonl_line(sock, cmd_id, req)
    result.setdefault("endpoint", f"unix:{path_str}")
    result.setdefault("instance", Path(path_str).name)
    return result


def call_tcp_bridge(
    addr: str, cmd_id: str, params: dict[str, Any] | None = None,
    timeout: float | None = None,
) -> dict[str, Any]:
    host, port_str = ("127.0.0.1", addr) if ":" not in addr else addr.rsplit(":", 1)
    req = dict(params or {})
    if "_client" not in req and os.environ.get("HERDR_PANE_ID"):
        req["_client"] = f"herdr:{os.environ['HERDR_PANE_ID']}"
    reply_timeout = command_timeout(cmd_id) if timeout is None else timeout
    with socket.create_connection((host, int(port_str)), timeout=min(reply_timeout, 5.0)) as sock:
        sock.settimeout(reply_timeout)
        return _send_jsonl_line(sock, cmd_id, req)


def discover_studios() -> dict[str, list[dict[str, Any]]]:
    """Discover all responsive live Studio windows via `ui.feedback`."""
    sock_dir = studio_socket_dir()
    sessions: list[dict[str, Any]] = []
    errors: list[dict[str, Any]] = []
    if not sock_dir or not sock_dir.is_dir():
        return {"sessions": sessions, "errors": errors}

    for entry in sorted(sock_dir.iterdir()):
        if not entry.name.endswith(".sock"):
            continue
        try:
            feedback = call_unix_socket(entry, "ui.feedback", {}, timeout=1.5)
            feedback["endpoint"] = f"unix:{entry}"
            feedback["instance"] = entry.name
            active = feedback.get("active_project")
            if active and Path(active).exists():
                feedback["canonical_project"] = str(Path(active).resolve())
            sessions.append(feedback)
        except Exception as exc:
            errors.append({"endpoint": f"unix:{entry}", "error": str(exc)})
    return {"sessions": sessions, "errors": errors}


def studio_for_project(project_path: str | Path) -> dict[str, Any] | None:
    """Return the live Studio session owning `project_path`, if any."""
    try:
        canonical = str(Path(project_path).resolve())
    except OSError:
        return None
    discovery = discover_studios()
    matches = [
        s
        for s in discovery["sessions"]
        if s.get("canonical_project") == canonical or s.get("active_project") == canonical
    ]
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        raise RuntimeError(
            f"Multiple Studio windows report owning {canonical}; close duplicates first."
        )
    return None


def call_bridge(
    endpoint: str,
    cmd_id: str,
    params: dict[str, Any] | None = None,
    expected_project: str | None = None,
    expected_instance: str | None = None,
) -> dict[str, Any]:
    """Send a command to an explicit or discovered Studio bridge endpoint."""
    req_params = dict(params or {})
    if expected_project:
        req_params["_expected_project"] = str(Path(expected_project).resolve())
    if expected_instance:
        req_params["_expected_instance"] = expected_instance

    if endpoint == "auto":
        req_proj = req_params.get("_expected_project") or req_params.get("project")
        if req_proj and Path(str(req_proj)).exists():
            live = studio_for_project(str(req_proj))
            if live:
                return call_unix_socket(live["endpoint"], cmd_id, req_params)
        discovery = discover_studios()
        sessions = [] if req_proj else discovery["sessions"]
        if len(sessions) == 1:
            return call_unix_socket(sessions[0]["endpoint"], cmd_id, req_params)
        if not sessions:
            if cmd_id == "ui.feedback":
                return {
                    "ok": True,
                    "connection": "not_observed_open",
                    "project": req_proj,
                    "ui_ready": False,
                    "unsaved_state": "unknown",
                    "recovery": "No matching responsive Studio. No app launched or switched.",
                }
            raise RuntimeError("No matching connected Studio; no command executed or app launched")
        raise RuntimeError(
            "Multiple Studios available; select a project or studio window explicitly."
        )

    if endpoint.startswith("unix:"):
        return call_unix_socket(endpoint, cmd_id, req_params)
    return call_tcp_bridge(endpoint, cmd_id, req_params)


def execute_headless(
    cmd_id: str,
    params: dict[str, Any] | None = None,
    dry_run: bool = False,
) -> dict[str, Any]:
    """Execute an engine command headlessly via `magicstudio-shell --headless`."""
    shell_bin = find_magicstudio_shell()
    payload = dict(params or {})
    args = [shell_bin, "--headless", "exec", cmd_id, json.dumps(payload)]
    if dry_run:
        args.append("--dry-run")
    proc = subprocess.run(args, capture_output=True, text=True, check=False)
    if proc.returncode != 0:
        err_msg = proc.stderr.strip() or proc.stdout.strip() or f"{cmd_id} failed"
        raise RuntimeError(err_msg)
    out = proc.stdout.strip()
    if not out:
        return {"ok": True}
    return json.loads(out)


class HeadlessSession:
    """Own one existing native JSONL engine session until its caller closes it."""

    def __init__(self) -> None:
        self.log = tempfile.TemporaryFile(mode="w+t", encoding="utf-8")
        try:
            self.process = subprocess.Popen(
                [find_magicstudio_shell(), "--headless", "run"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
                text=True, encoding="utf-8", bufsize=1,
            )
        except BaseException:
            self.log.close()
            raise

    def execute(self, cmd_id: str, params: dict[str, Any]) -> dict[str, Any]:
        request = json.dumps({"id": cmd_id, "params": params}) + "\n"
        try:
            self.process.stdin.write(request)
            self.process.stdin.flush()
            line = self.process.stdout.readline()
        except (BrokenPipeError, OSError) as exc:
            raise CommandOutcomeUnknown(f"Native session lost during {cmd_id}; no retry was made") from exc
        if not line:
            self.log.seek(0)
            detail = self.log.read()[-8000:].strip()
            raise CommandOutcomeUnknown(f"Native session ended during {cmd_id}; outcome unknown; no retry. {detail}")
        reply = json.loads(line)
        if reply.get("ok") is not True:
            raise RuntimeError(reply.get("error") or f"{cmd_id} failed")
        return reply["result"]

    def close(self) -> None:
        # EOF ends the existing engine loop and releases the project writer lease.
        # Do not kill work that may already be changing a project.
        try:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
            self.process.wait()
        finally:
            self.process.stdout.close()
            self.log.close()


def list_engine_commands() -> list[dict[str, Any]]:
    """The engine registry is the only command catalogue."""
    return execute_headless("command.list")["commands"]


def open_studio_window(
    project_path: str | None = None,
    animate_only: bool = False,
    character: str | None = None,
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Launch or focus a Magic Studio window and wait until `ui_ready == true`."""
    canonical_proj: str | None = None
    if project_path:
        p = Path(project_path)
        if not p.exists():
            raise RuntimeError(f"Project file does not exist: {project_path}")
        canonical_proj = str(p.resolve())
        existing = studio_for_project(canonical_proj)
        if existing:
            try:
                call_unix_socket(
                    existing["endpoint"],
                    "ui.focus",
                    {
                        "_expected_instance": existing.get("instance"),
                        "_expected_project": canonical_proj,
                    },
                )
                return existing
            except Exception:
                pass

    shell_bin = find_magicstudio_shell()
    cmd = [shell_bin]
    if animate_only:
        cmd.append("--animate")
    if character:
        cmd.extend(["--character", character])
    if canonical_proj:
        cmd.append(canonical_proj)
    elif not animate_only:
        cmd.append("--new")

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )
    pid = proc.pid
    sock_dir = studio_socket_dir()
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if sock_dir and sock_dir.is_dir():
            for entry in sock_dir.iterdir():
                if not entry.name.startswith(f"{pid}-") or not entry.name.endswith(".sock"):
                    continue
                try:
                    live = call_unix_socket(entry, "ui.feedback", {}, timeout=1.5)
                    if live.get("ui_ready") is True and live.get("identity", {}).get("pid") == pid:
                        if canonical_proj and live.get("active_project") != canonical_proj:
                            continue
                        live["endpoint"] = f"unix:{entry}"
                        live["instance"] = entry.name
                        return live
                except Exception:
                    pass
        ret = proc.poll()
        if ret is not None:
            raise RuntimeError(f"Studio process {pid} exited during opening (code {ret})")
        time.sleep(0.1)

    raise RuntimeError(
        f"Studio process {pid} started but has not reported a ready window within {timeout}s."
    )
