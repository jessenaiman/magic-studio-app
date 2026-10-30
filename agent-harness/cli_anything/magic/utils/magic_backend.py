"""Backend integration for Magic Studio (`magicstudio-shell` and live Studio sockets).

Handles:
1. Finding the native `magicstudio-shell` binary (repo target/debug, installed ~/.local/share/magic-studio/bin, or PATH).
2. Discovering live Studio instances via `$XDG_RUNTIME_DIR/magicstudio-studio/*.sock` (`ui.feedback`).
3. Communicating with a live Studio window over its private Unix domain socket.
4. Executing headless engine commands via `magicstudio-shell --headless` (with no dependency on `mc`).
5. Launching or focusing a Studio window (`magic-cli studio open [project]`) and waiting for `ui_ready == true`.
"""

from __future__ import annotations

import json
from functools import lru_cache
import os
import shlex
import shutil
import socket
import selectors
import subprocess
import sys
import time
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[4]


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
    override = os.environ.get("MAGICSTUDIO_COMMAND_TIMEOUT_MS")
    if override:
        try:
            value = float(override) / 1000
            if value <= 0:
                raise ValueError()
            return value
        except ValueError:
            raise ValueError("MAGICSTUDIO_COMMAND_TIMEOUT_MS must be a positive number") from None
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
    raise RuntimeError(str(resp.get("error") or "unknown Studio command error"))


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
            feedback["pid"] = studio_pid(entry.name)
            active = feedback.get("active_project")
            if active and Path(active).exists():
                feedback["canonical_project"] = str(Path(active).resolve())
            sessions.append(feedback)
        except Exception as exc:
            errors.append({"endpoint": f"unix:{entry}", "error": str(exc)})
    return {"sessions": sessions, "errors": errors}


def studio_pid(instance: str) -> int | None:
    """The window's process id: Studio names its socket `<pid>-<nonce>.sock`."""
    head = instance.split("-", 1)[0]
    return int(head) if head.isdigit() else None


def preview_servers() -> list[dict[str, Any]]:
    """Running `hyperframes preview <dir>` servers, read from the process table.

    One server exists per project whose Studio target was started; each says whether a Studio
    window still has that project open, so leftovers from closed windows are visible.
    """
    open_dirs = {
        str(Path(s["canonical_project"]).parent)
        for s in discover_studios()["sessions"]
        if s.get("canonical_project")
    }
    servers: list[dict[str, Any]] = []
    for proc in Path("/proc").iterdir():
        if not proc.name.isdigit():
            continue
        try:
            argv = (proc / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        words = [a.decode(errors="replace") for a in argv if a]
        if "preview" not in words or not any(w.endswith("/hyperframes") and "node_modules" in w for w in words[:2]):
            continue
        dirs = [w for w in words[words.index("preview") + 1 :] if not w.startswith("--")]
        if not dirs:
            continue
        project = str(Path(dirs[0]).parent)
        servers.append({"pid": int(proc.name), "directory": dirs[0], "project_dir": project,
                        "window_open": project in open_dirs})
    return sorted(servers, key=lambda r: r["pid"])


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


def call_studio(
    endpoint: str,
    cmd_id: str,
    params: dict[str, Any] | None = None,
    expected_project: str | None = None,
    expected_instance: str | None = None,
) -> dict[str, Any]:
    """Send a command to a pinned or automatically discovered Studio window."""
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

    if not endpoint.startswith("unix:"):
        raise RuntimeError("Invalid Studio session endpoint; use studio open to select a window")
    return call_unix_socket(endpoint, cmd_id, req_params)


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
    """Own one native request session; transport failure requires explicit recovery."""

    def __init__(self) -> None:
        self.log = tempfile.TemporaryFile(mode="w+t", encoding="utf-8")
        self.unavailable: str | None = None
        try:
            self.process = subprocess.Popen(
                [find_magicstudio_shell(), "--headless", "serve"],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, bufsize=0,
            )
            os.set_blocking(self.process.stdin.fileno(), False)
            os.set_blocking(self.process.stdout.fileno(), False)
        except BaseException:
            self.log.close()
            raise

    @staticmethod
    def _ready(fd: int, event: int, deadline: float) -> None:
        remaining = deadline - time.monotonic()
        with selectors.DefaultSelector() as selector:
            selector.register(fd, event)
            if remaining <= 0 or not selector.select(remaining):
                raise TimeoutError("native reply deadline exceeded")

    def execute(self, cmd_id: str, params: dict[str, Any]) -> dict[str, Any]:
        if self.unavailable:
            raise CommandOutcomeUnknown(self.unavailable)
        # Encode and validate the deadline before submitting anything.
        request = (json.dumps({"id": cmd_id, "params": params}) + "\n").encode("utf-8")
        deadline = time.monotonic() + command_timeout(cmd_id)
        try:
            write_fd, read_fd = self.process.stdin.fileno(), self.process.stdout.fileno()
            sent = 0
            while sent < len(request):
                self._ready(write_fd, selectors.EVENT_WRITE, deadline)
                sent += os.write(write_fd, request[sent:])
            chunks = bytearray()
            while not chunks.endswith(b"\n"):
                self._ready(read_fd, selectors.EVENT_READ, deadline)
                chunk = os.read(read_fd, 65536)
                if not chunk:
                    raise EOFError("native engine exited before replying")
                chunks.extend(chunk)
            reply = json.loads(chunks.decode("utf-8"))
            if not isinstance(reply, dict) or "ok" not in reply:
                raise ValueError("invalid native response")
        except (OSError, EOFError, ValueError) as exc:
            self.unavailable = (
                f"{cmd_id}: native session {self.process.pid} lost or timed out ({exc}). "
                "Outcome unknown; no cancellation or retry was performed. "
                "Inspect saved results, then explicitly use project.open to recover. "
                "Recovery refuses while that native process is still running."
            )
            raise CommandOutcomeUnknown(self.unavailable) from exc
        if reply.get("ok") is not True:
            raise RuntimeError(reply.get("error") or json.dumps(reply.get("result")) or f"{cmd_id} failed")
        result = reply["result"]
        return result if isinstance(result, dict) else {"value": result}

    def _finish(self) -> bool:
        if self.process.stdin is not None:
            self.process.stdin.close()
            self.process.stdin = None
        try:
            # EOF ends the native loop after active work. Drain without waiting forever.
            self.process.communicate(timeout=1.0)
            return True
        except subprocess.TimeoutExpired:
            return False

    def recover(self) -> None:
        if not self._finish():
            raise CommandOutcomeUnknown(
                f"Native process {self.process.pid} is still running after the lost reply; "
                "no replacement was started. Inspect the operation before reselecting."
            )
        self.process.stdout.close()
        self.log.close()

    def close(self) -> None:
        try:
            finished = self._finish()
            if not finished:
                print(f"Native process {self.process.pid} still running after a lost reply; "
                      "stdin closed, no force kill or retry. Inspect saved results before retrying.", file=sys.stderr)
        finally:
            self.process.stdout.close()
            self.log.close()


def list_engine_commands() -> list[dict[str, Any]]:
    """The engine registry is the only command catalogue."""
    return execute_headless("command.list")["commands"]


def agent_caller() -> bool:
    """Herdr sets HERDR_PANE_ID in agent panes; their windows stay out of the way."""
    return bool(os.environ.get("HERDR_PANE_ID"))


def hyprctl_json(hyprctl: str, what: str) -> Any:
    result = subprocess.run(  # noqa: S603 -- Fixed argv; no shell.
        [hyprctl, what, "-j"], capture_output=True, text=True, check=False
    )
    return json.loads(result.stdout or "null")


def empty_workspace(hyprctl: str) -> str:
    """MAGIC_AGENT_WORKSPACE, else the emptiest of workspaces 1-4 (user-visible)."""
    if chosen := os.environ.get("MAGIC_AGENT_WORKSPACE"):
        return chosen
    windows = {w["id"]: w["windows"] for w in hyprctl_json(hyprctl, "workspaces") or []}
    return str(min(range(1, 5), key=lambda n: (windows.get(n, 0), n)))


def launch_on_empty_workspace(cmd: list[str]) -> str | None:
    """Start cmd unfocused on an empty workspace; its name, or None without Hyprland."""
    hyprctl = shutil.which("hyprctl")
    if not (hyprctl and os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")):
        return None
    workspace = empty_workspace(hyprctl)
    rule = f"[workspace {workspace} silent] {shlex.join(cmd)}"
    result = subprocess.run(  # noqa: S603 -- Fixed argv; no shell.
        [hyprctl, "dispatch", f"hl.dsp.exec_cmd({json.dumps(rule)})"],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0 and result.stdout.strip() == "ok":
        return workspace
    detail = result.stdout or result.stderr
    raise RuntimeError(f"Agent window launch failed: {detail}")


def studio_command(project: str | None, character: str | None) -> list[str]:
    cmd = [find_magicstudio_shell()]
    if character:
        cmd.extend(["--character", character])
    if project:
        cmd.append(project)
    else:
        cmd.append("--new")
    return cmd


def ready_window(
    entry: Path, pid: int | None, project: str | None
) -> dict[str, Any] | None:
    """The window behind a socket once its UI is ready for this pid/project."""
    try:
        live = call_unix_socket(entry, "ui.feedback", {}, timeout=1.5)
    except Exception:
        return None
    # An unfocused agent window is not painted, so its frontend withholds the
    # revision acknowledgement; its applied DOM is ready, marked unpainted.
    applied = live.get("phase") == "ready" and live.get("rendered") is True
    offscreen_ready = pid is None and applied and not live.get("errors")
    if live.get("ui_ready") is not True and not offscreen_ready:
        return None
    live["painted"] = live.get("ui_ready") is True
    if pid is not None and live.get("identity", {}).get("pid") != pid:
        return None
    if project and live.get("active_project") != project:
        return None
    live["endpoint"] = f"unix:{entry}"
    live["instance"] = entry.name
    return live


def reuse_window(existing: dict[str, Any], project: str) -> dict[str, Any] | None:
    """An agent reuses the window silently; a person brings it to the front."""
    if agent_caller():
        return existing
    try:
        call_unix_socket(
            existing["endpoint"],
            "ui.focus",
            {
                "_expected_instance": existing.get("instance"),
                "_expected_project": project,
            },
        )
        return existing
    except Exception:
        return None


def wait_for_window(
    proc: subprocess.Popen | None, before: set[str], project: str | None, timeout: float
) -> dict[str, Any]:
    """The new window: by pid when we started it, else the first new socket."""
    pid = proc.pid if proc else None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if live := find_new_window(before, pid, project):
            return live
        if proc and (ret := proc.poll()) is not None:
            raise RuntimeError(f"Studio {pid} exited while opening (code {ret})")
        time.sleep(0.1)
    who = pid or "agent workspace"
    raise RuntimeError(f"Studio window ({who}) not ready within {timeout}s.")


def find_new_window(
    before: set[str], pid: int | None, project: str | None
) -> dict[str, Any] | None:
    sock_dir = studio_socket_dir()
    for entry in sock_dir.iterdir() if sock_dir and sock_dir.is_dir() else []:
        new = entry.name.startswith(f"{pid}-") if pid else entry.name not in before
        if new and entry.name.endswith(".sock"):
            if live := ready_window(entry, pid, project):
                return live
    return None


def open_studio_window(
    project_path: str | None = None,
    character: str | None = None,
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Launch or reuse a Magic Studio window and wait until `ui_ready == true`.

    A person's call focuses the window. An agent's call never focuses one: a new
    agent window opens unfocused on an empty workspace (reported as "workspace"),
    so the user keeps their windows and cursor and can switch there to watch.
    """
    canonical_proj: str | None = None
    if project_path:
        p = Path(project_path)
        if not p.exists():
            raise RuntimeError(f"Project file does not exist: {project_path}")
        canonical_proj = str(p.resolve())
        existing = studio_for_project(canonical_proj)
        if existing and (live := reuse_window(existing, canonical_proj)):
            return live
    cmd = studio_command(canonical_proj, character)
    sock_dir = studio_socket_dir()
    exists = sock_dir and sock_dir.is_dir()
    before = {e.name for e in sock_dir.iterdir()} if exists else set()
    proc = None
    workspace = launch_on_empty_workspace(cmd) if agent_caller() else None
    if workspace is None:
        proc = subprocess.Popen(  # noqa: S603 -- Fixed argv; no shell.
            cmd,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    live = wait_for_window(proc, before, canonical_proj, timeout)
    if workspace:
        live["workspace"] = workspace
    return live
