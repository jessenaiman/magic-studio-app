"""Magic's terminal adapter. Command vocabulary comes from the application registry."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import click

from cli_anything.magic import __version__
from cli_anything.magic.core import audit as audit_core
from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend


def parse_params(tokens: tuple[str, ...], raw: str | None = None) -> dict[str, Any]:
    params = json.loads(raw) if raw else {}
    if not isinstance(params, dict):
        raise click.BadParameter("--params must be a JSON object")
    for token in tokens:
        if "=" not in token:
            raise click.BadParameter(f"Expected key=value, got {token!r}; see --help")
        key, value = token.split("=", 1)
        if not key:
            raise click.BadParameter("Parameter name cannot be empty")
        try:
            params[key] = json.loads(value)
        except ValueError:
            params[key] = value
    return params


def emit(value: Any) -> None:
    click.echo(json.dumps(value, indent=2))


def execute(ctx: click.Context, command: str, params: dict[str, Any]) -> Any:
    params.setdefault("_origin", "CLI")
    return ctx.obj["session"].execute(
        command, params, dry_run=ctx.obj["dry_run"]
    )


class RegistryGroup(click.Group):
    """Lazily derive `domain verb` (underscores become hyphens) from command IDs."""

    def __init__(self, *args: Any, prefix: str = "", **kwargs: Any):
        super().__init__(*args, **kwargs)
        self.prefix = prefix
        self._specs: list[dict[str, Any]] | None = None

    def specs(self) -> list[dict[str, Any]]:
        if self._specs is None:
            self._specs = backend.list_engine_commands()
        return self._specs

    def list_commands(self, ctx: click.Context) -> list[str]:
        names = set(super().list_commands(ctx))
        for spec in self.specs():
            if spec["id"].startswith(self.prefix):
                names.add(
                    spec["id"][len(self.prefix) :].split(".")[0].replace("_", "-")
                )
        return sorted(names)

    def format_commands(
        self, ctx: click.Context, formatter: click.HelpFormatter
    ) -> None:
        # Initial help must remain usable when the native registry is unavailable.
        # Only static adapter commands are listed in that case; never invent domain IDs.
        try:
            super().format_commands(ctx, formatter)
        except (RuntimeError, OSError) as error:
            rows = [
                (name, command.get_short_help_str())
                for name, command in sorted(self.commands.items())
                if not command.hidden
            ]
            if rows:
                with formatter.section("Built-in commands"):
                    formatter.write_dl(rows)
            with formatter.section("Native commands unavailable"):
                formatter.write_text(
                    str(error).splitlines()[0].removesuffix(" Build it with:")
                )
                formatter.write_text(
                    "Use the installed Magic application for registry commands; "
                    "report the missing runtime rather than reinstalling or searching "
                    "the machine."
                )

    def get_command(self, ctx: click.Context, name: str) -> click.Command | None:
        explicit = super().get_command(ctx, name)
        if explicit:
            return explicit
        command_id = self.prefix + name.replace("-", "_")
        spec = next((s for s in self.specs() if s["id"] == command_id), None)
        children = any(s["id"].startswith(command_id + ".") for s in self.specs())
        if not spec and not children:
            return None

        def run(**args: Any) -> None:
            current = click.get_current_context()
            if current.invoked_subcommand:
                return
            if spec:
                emit(
                    execute(
                        current,
                        command_id,
                        parse_params(args.get("arguments", ()), args.get("params")),
                    )
                )
            else:
                click.echo(current.get_help())

        help_text = (
            f"{spec['label']}. Parameters: {spec['params']}"
            if spec
            else f"{name} commands from the application registry."
        )
        options = [click.Option(["--params"], help="JSON parameter object")]
        # Groups may also be commands (e.g. motion.path); use --params for the parent.
        if not children:
            options.append(click.Argument(["arguments"], nargs=-1))
        if children:
            group = RegistryGroup(
                name,
                prefix=command_id + ".",
                callback=run,
                params=options,
                help=help_text,
                invoke_without_command=True,
            )
            group._specs = self.specs()
            return group
        return click.Command(name, callback=run, params=options, help=help_text)


@click.group(cls=RegistryGroup, invoke_without_command=True)
@click.option("--json", "use_json", is_flag=True, help="JSON output (also the default)")
@click.option("--project", type=click.Path(), help="Select the intended .magic project")
@click.option(
    "--session-file",
    type=click.Path(),
    envvar="MAGIC_SESSION_FILE",
    help="Separate caller session binding",
)
@click.option("--dry-run", is_flag=True, help="Preview mutations without applying them")
@click.version_option(__version__, prog_name="magic-cli")
@click.pass_context
def cli(
    ctx: click.Context,
    use_json: bool,
    project: str | None,
    session_file: str | None,
    dry_run: bool,
) -> None:
    """Operate installed Magic Studio: projects, storyboard, and the desktop app.

    \b
    Open an existing project window:
      magic-cli studio open /absolute/Song.magic
    Read its storyboard through the shared engine:
      magic-cli --project /absolute/Song.magic storyboard read

    The app's Studio view embeds the project's linked HyperFrames workspace.
    Select Studio and use its existing switch if off; do not create a second studio.

    Use key=value arguments or --params JSON. Next: COMMAND --help for parameters,
    commands [FILTER] for registry IDs, or studio open --help for window selection.
    Use configured/supplied paths; report missing paths rather than search the machine.
    """
    session = Session(session_file)
    ctx.call_on_close(session.close)
    ctx.obj = {"session": session, "dry_run": dry_run}
    if project:
        session.select_project(project)
    if ctx.invoked_subcommand is None:
        click.echo(ctx.get_help())


@cli.command("audit")
@click.argument(
    "path", type=click.Path(exists=True, path_type=Path), default="."
)
@click.option(
    "--tool",
    "tools",
    multiple=True,
    type=click.Choice(tuple(audit_core.TOOLS)),
    help=(
        "Select a native tool; repeat to choose several. "
        "Default: Lizard for a file, all nine for a directory."
    ),
)
@click.option(
    "--format", "output_format", type=click.Choice(("text", "csv")), default="text",
    show_default=True,
    help="Lizard output: ranked native text diagnostics or native CSV measurements.",
)
@click.option(
    "--base", default="HEAD", show_default=True,
    help="Scope and --compare: git revision the change is measured from.",
)
@click.option(
    "--compare", is_flag=True,
    help="One before/now table of design and code metrics; nonzero if any got worse.",
)
@click.pass_context
def audit(
    ctx: click.Context, path: Path, tools: tuple[str, ...], output_format: str,
    base: str, compare: bool,
) -> None:
    """Run native repository checks with unmodified tool output.

    Default order: Ruff, cargo clippy, ESLint, Stylelint, Lizard, Wallace,
    jscpd, PurgeCSS, scope. Each
    tool uses PATH (or root node_modules/.bin) and the nearest repository
    configuration at or above PATH, up to the repository top.
    Missing tools/configuration are errors, not passing or skipped checks.
    Lizard shows function locations, file totals, CCN, length and cognitive
    complexity, with warnings ranked by CCN. code-limits.json requires
    max_function_ccn; optional max_function_lines (100),
    max_function_cognitive_complexity (15) set thresholds.
    Wallace measures our stylesheets (not vendor/node_modules/target) as one
    design: unique colours, font sizes and declarations, ID selectors and
    specificity. Compare its table before and after a UI change. jscpd lists
    repeated blocks (.jscpd.json): missed chances to share one class, helper
    or component. PurgeCSS (purgecss.config.cjs) lists selectors no component
    or page produces: dead styles; its JSON omits only the purged CSS text.
    CSS is component code: read Stylelint, Wallace, jscpd and PurgeCSS
    together after every UI change. scope prints git's diff stat since --base
    for the whole repository, and untracked files, so changes outside PATH
    show as creep.

    --compare measures PATH at --base and now with the same configuration and
    prints one table (before, now, change, better/worse; lower is better), the
    scope since --base, and each stylesheet's concerns with suggested files.
    It exits 1 when any metric got worse.
    CSV mode also runs native warnings on stderr so violations still fail.
    Other tools keep their native formats.

    Each native exit code is printed; the command returns the first nonzero
    status, or 0 if all selected tools return 0. Setup/invocation errors use 2;
    signals use 128 + signal. Remaining selected tools still run after failure.
    Global --dry-run validates setup and prints commands without running them.
    No install, autofix, Codacy, AgentLinter, or upload feature is invoked.

    A file defaults to Lizard. For a Rust file such as module.rs, Lizard also
    scans its companion module/ directory when present, so extracted functions
    stay in the audit. Native output names every analyzed file. Other explicit
    tools receive the exact file path. A Lizard directory check stays within
    that directory and inherits the repository limits.
    Clippy requires a crate directory. File checks use repository configuration.
    """
    if compare:
        from cli_anything.magic.core.audit_compare import compare as compare_metrics

        ctx.exit(compare_metrics(path.resolve(), base))
    ctx.exit(audit_core.run_audit(
        path.resolve(), tools, dry_run=ctx.obj["dry_run"], output_format=output_format,
        base=base,
    ))


@cli.command("commands")
@click.argument("filter", required=False)
@click.pass_context
def commands(ctx: click.Context, filter: str | None) -> None:
    """List native command IDs and parameters; optional FILTER matches registry text.

    Example: magic-cli commands storyboard

    Next: magic-cli storyboard --help, then the chosen command's --help.
    """
    emit(execute(ctx, "command.list", {"filter": filter or ""}))


@cli.command("exec")
@click.argument("command")
@click.argument("arguments", nargs=-1)
@click.option("--params", help="JSON parameter object")
@click.pass_context
def exec_command(
    ctx: click.Context, command: str, arguments: tuple[str, ...], params: str | None
) -> None:
    """Execute COMMAND as a native registry ID with key=value or --params JSON.

    Example: magic-cli --project /absolute/Song.magic exec workshop.status

    Next: magic-cli commands FILTER to discover IDs and parameter documentation.
    """
    emit(execute(ctx, command, parse_params(arguments, params)))


@cli.command("run")
@click.argument("file", type=click.File("r"), default="-")
@click.pass_context
def run(ctx: click.Context, file: Any) -> None:
    """Run JSONL {id, params} steps in order; stop on error.

    Earlier steps remain applied.
    """
    for number, line in enumerate(file, 1):
        if not line.strip() or line.lstrip().startswith(("#", "//")):
            continue
        item = json.loads(line)
        result = execute(ctx, item.get("id") or item["command"], item.get("params", {}))
        click.echo(json.dumps({"line": number, "ok": True, "result": result}))


@cli.command("mcp")
@click.pass_context
def mcp(ctx: click.Context) -> None:
    """Serve Magic commands over MCP stdio using the official Python SDK."""
    from cli_anything.magic.core.mcp_server import run_mcp_stdio

    run_mcp_stdio(
        ctx.obj["session"], dry_run=ctx.obj["dry_run"]
    )


@cli.group(cls=RegistryGroup, prefix="studio.")
def studio() -> None:
    """Select a desktop window; list inspects responsive windows without rebinding.

    Example: magic-cli studio open /absolute/Song.magic

    Next: studio open --help for the optional project argument.
    """


@studio.command("open")
@click.argument("project", required=False)
@click.pass_context
def studio_open(ctx: click.Context, project: str | None) -> None:
    """Open/focus PROJECT, an existing .magic file; omit it for a blank window.

    Example: magic-cli studio open /absolute/Song.magic

    Next: magic-cli ui feedback to inspect the selected window. In the app,
    select Studio for its linked HyperFrames workspace.
    """
    if ctx.obj["dry_run"]:
        emit({"dry_run": True, "project": project})
        return
    emit(ctx.obj["session"].open_studio(project))


@studio.command("list")
def studio_list() -> None:
    """List responsive windows without changing the current selection."""
    emit(backend.discover_studios())


@cli.group()
def session() -> None:
    """Inspect or clear this caller's project/window binding."""


@session.command("status")
@click.pass_context
def session_status(ctx: click.Context) -> None:
    sess = ctx.obj["session"]
    emit(
        {
            "project": sess.project_path,
            "endpoint": sess.studio_endpoint,
            "instance": sess.studio_instance,
        }
    )


@session.command("clear")
@click.pass_context
def session_clear(ctx: click.Context) -> None:
    ctx.obj["session"].clear()
    emit({"ok": True})


def main() -> None:
    try:
        cli(prog_name="magic-cli")
    except (RuntimeError, OSError, ValueError) as exc:
        click.echo(f"magic-cli: {exc}", err=True)
        sys.exit(1)


if __name__ == "__main__":
    main()
