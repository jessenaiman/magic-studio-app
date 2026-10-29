# Magic Studio entry-point review

## Current contract

`magic` launches the desktop application. `magic-cli` derives its Click command
tree from the native registry; `magic-cli mcp` uses the official Python MCP SDK.
Both target an explicitly selected project/window, retaining native writer
ownership and pending-work guards. UI project actions enter guarded native
dispatch directly rather than invoking a CLI subprocess.

See [command architecture](../../agent-harness/MAGIC.md),
[the Magic skill](../../agent-harness/cli_anything/magic/skills/SKILL.md), and
[setup](../../agent-harness/cli_anything/magic/README.md) for the maintained paths.
The 28 October 2026 implementation checkpoint and machine-specific verification
record remain in Git; they are not installation or regression-test instructions.

## Review boundaries

- Verify the exact selected project and visible/saved result, not just transport
  success. A lost reply is an unknown outcome and must not trigger blind replay.
- Preserve user-deleted tests and root instruction files; do not restore them
  from historical handoffs. Existing `CLAUDE.md`, `CODEX.md`, `CURSOR.md`, and
  `COPILOT.md` point to the current contracts.
- Follow [CONTRIBUTING](../../CONTRIBUTING.md) for approved native acceptance,
  checks actually run, and review before merge.
