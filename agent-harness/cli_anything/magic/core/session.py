"""Session state and unified execution routing for Magic Studio CLI.

Implements `_locked_save_json` from CLI-Anything `guides/session-locking.md` and
routes operations transparently between:
1. Live Studio windows (private same-user IPC) when the active project
   is open in Magic Studio or a Studio window is bound.
2. Headless engine execution (`magicstudio-shell --headless`) when working headlessly
   on a `.magic` project file.
Never invents a cross-domain undo ledger; undo delegates to the specific domain's
engine command (`motion.edit.undo`, `motion.undo_path`, `part.undo_image`,
`storyboard.update`, `skills.undo`).
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from cli_anything.magic.utils import magic_backend as backend


def _locked_save_json(path: str | Path, data: Any, **dump_kwargs: Any) -> None:
    """Lock before changing the session binding, including on first creation."""
    import fcntl
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    content = json.dumps(data, indent=2, **dump_kwargs)
    with path.open("a+", encoding="utf-8") as file:
        fcntl.flock(file.fileno(), fcntl.LOCK_EX)
        file.seek(0)
        file.truncate()
        file.write(content)
        file.flush()


def default_session_file() -> Path:
    """Return the runtime-scoped session file path for this caller/pane."""
    override = os.environ.get("MAGIC_SESSION_FILE")
    if override:
        return Path(override).expanduser()
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    base = Path(runtime) / "magicstudio" if runtime else Path.home() / ".magic"
    pane = os.environ.get("HERDR_PANE_ID")
    if pane and all(c.isalnum() or c in "-_:" for c in pane):
        return base / "agent-studios" / pane / "session.json"
    return base / "session.json"


class Session:
    """Stateful CLI-Anything session for Magic Studio."""

    def __init__(self, session_file: str | Path | None = None) -> None:
        self.session_file = Path(session_file) if session_file else default_session_file()
        self.project_path: str | None = None
        self.studio_endpoint: str | None = None
        self.studio_instance: str | None = None
        self._headless: backend.HeadlessSession | None = None
        self._headless_error: str | None = None
        self.load_session()

    def load_session(self) -> None:
        """Load persisted session binding from disk if present."""
        if self.session_file.is_file():
            try:
                raw = json.loads(self.session_file.read_text(encoding="utf-8"))
                self.project_path = raw.get("project_path")
                self.studio_endpoint = raw.get("studio_endpoint")
                self.studio_instance = raw.get("studio_instance")
            except (OSError, ValueError, TypeError) as exc:
                raise RuntimeError(f"Cannot read session binding {self.session_file}: {exc}; no target selected") from exc

    def save_session(self) -> None:
        """Persist current session binding (`session.json`).

        Never writes to the `.magic` file; project files are
        owned exclusively by the Rust engine (`project.save` and headless dispatch).
        """
        state = {
            "project_path": self.project_path,
            "studio_endpoint": self.studio_endpoint,
            "studio_instance": self.studio_instance,
        }
        _locked_save_json(self.session_file, state)

    def _live_for_pinned_project(self) -> list[dict[str, Any]]:
        return [x for x in backend.discover_studios()["sessions"]
                if self.project_path and self.project_path in (x.get("canonical_project"), x.get("active_project"))]

    def _unavailable_message(self) -> str:
        """Name the live windows for the pinned project; never rebind to one silently."""
        message = "Pinned Studio is unavailable; no other window or file writer was selected. "
        live = self._live_for_pinned_project()
        if live:
            found = ", ".join(f"pid {x.get('pid')} ({x.get('instance')})" for x in live)
            return message + f"A live window has this project open: {found}. Reattach with: magic-cli studio open {self.project_path}"
        return message + "No live window has this project open. Use project open or studio open to explicitly reattach."

    def bind_studio(self, live: dict[str, Any]) -> None:
        """Bind the session to a live Studio window receipt."""
        self._headless_error = None
        self.studio_endpoint = live.get("endpoint")
        self.studio_instance = live.get("instance")
        active = live.get("active_project")
        self.project_path = str(Path(active).resolve()) if active else None
        self.save_session()

    def clear(self) -> None:
        """Clear this caller's binding without changing its project or window."""
        self.close()
        self._headless_error = None
        self.project_path = None
        self.studio_endpoint = None
        self.studio_instance = None
        if self.session_file.is_file():
            try:
                self.session_file.unlink()
            except OSError:
                raise

    def close(self) -> None:
        if self._headless is not None:
            engine, self._headless = self._headless, None
            engine.close()

    def execute_headless(self, cmd_id: str, params: dict[str, Any]) -> dict[str, Any]:
        if self._headless_error:
            if cmd_id != "project.open":
                raise backend.CommandOutcomeUnknown(self._headless_error)
            if self._headless is not None:
                self._headless.recover()
                self._headless = None
            self._headless_error = None
        if self._headless is None:
            self._headless = backend.HeadlessSession()
        try:
            return self._headless.execute(cmd_id, params)
        except backend.CommandOutcomeUnknown as exc:
            self._headless_error = str(exc)
            if self._headless.process.poll() is not None:
                self.close()
            raise

    def open_studio(self, project: str | None = None) -> dict[str, Any]:
        from cli_anything.magic.core.project import resolve_project_path
        target = str(resolve_project_path(project)) if project else None
        if self._headless_error and self._headless is not None:
            self._headless.recover()  # Refuse a second writer while unknown work is still running.
            self._headless = None
        self.close()  # Explicitly transfer the saved project from this engine to the GUI.
        live = backend.open_studio_window(target)
        self.bind_studio(live)
        return live

    def select_project(self, path: str) -> dict[str, Any]:
        """Explicit selection may replace an old binding only after a successful open."""
        return self.execute("project.open", {"path": path})

    def execute(
        self, cmd_id: str, params: dict[str, Any] | None = None,
        dry_run: bool = False,
    ) -> dict[str, Any]:
        req = dict(params or {})
        dry_run = dry_run or req.get("_dry_run") is True
        if dry_run:
            req["_dry_run"] = True
        # Explicit project selection starts a new binding. Ordinary operations retain
        # their pinned window, even when another window now owns the same path.
        if cmd_id == "command.list":
            return backend.execute_headless(cmd_id, req)
        # Isolated read-only queries must not change the caller's target or writer session.
        if cmd_id in {"backstage.observe", "backstage.changes"}:
            return backend.execute_headless(cmd_id, req)
        selecting = cmd_id == "project.open"
        target = req.get("path") if selecting else req.get("project")
        global_command = cmd_id in {
            "project.new", "project.intake", "project.catalog", "project.root",
            "project.migrate", "project.identity", "command.list", "app.doctor",
        }
        if not target and not global_command and not selecting:
            target = self.project_path
        if target:
            from cli_anything.magic.core.project import resolve_project_path
            target = str(resolve_project_path(target))
            req["path" if selecting else "project"] = target

        if self.studio_endpoint and not selecting:
            if target and self.project_path and target != self.project_path:
                raise RuntimeError("Pinned Studio targets a different project; explicitly open the intended project first")
            try:
                result = backend.call_studio(
                    self.studio_endpoint, cmd_id, req,
                    expected_project=self.project_path, expected_instance=self.studio_instance,
                )
            except OSError as exc:
                # The window restarted: the same project in exactly one live window is the same
                # intent, so follow it (the expected-project guard still applies). Anything else
                # stays an explicit choice.
                live = self._live_for_pinned_project()
                if len(live) != 1:
                    raise RuntimeError(self._unavailable_message()) from exc
                self.bind_studio(live[0])
                result = backend.call_studio(
                    live[0]["endpoint"], cmd_id, req,
                    expected_project=self.project_path, expected_instance=live[0].get("instance"),
                )
        else:
            live = backend.studio_for_project(target) if target else None
            if live:
                self.close()
                result = backend.call_studio(
                    live["endpoint"], cmd_id, req, expected_project=target,
                    expected_instance=live.get("instance"),
                )
                if not dry_run:
                    self.bind_studio(live)
            elif cmd_id.startswith(("ui.", "app.quit")) and backend.agent_caller():
                raise RuntimeError(
                    "No Studio window is bound to this agent; run "
                    "'magic-cli studio open /absolute/Project.magic' first "
                    "(agent windows open on the agent workspace, never the user's window)."
                )
            elif cmd_id.startswith("ui.") or cmd_id == "app.quit":
                result = backend.call_studio("auto", cmd_id, req, expected_project=target)
            else:
                result = self.execute_headless(cmd_id, req)
            if selecting and not live and not dry_run and result.get("ok") is not False:
                self.studio_endpoint = None
                self.studio_instance = None

        if result.get("ok") is False:
            raise RuntimeError(result.get("error") or json.dumps(result))
        if not dry_run:
            if cmd_id in {"project.open", "project.new", "project.intake", "project.save"} and result.get("project"):
                self.project_path = str(Path(result["project"]).resolve())
                self.save_session()
            elif cmd_id == "project.close":
                self.project_path = None
                self.save_session()
            # Keep the pin after quit: subsequent calls must not pick another window.
        return result
