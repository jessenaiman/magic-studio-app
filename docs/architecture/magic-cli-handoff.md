# Magic Studio CLI handoff

The current application entry points are `magic` (native GUI), `magic-cli`
(registry-derived CLI), and `magic-cli mcp` (official SDK stdio server).
Project/window binding and native dispatch are shared; there is no REPL or
`mc` compatibility entry point.

Use these maintained contracts rather than the superseded command/setup notes:

- [CLI/MCP architecture](../../agent-harness/MAGIC.md)
- [Magic command skill](../../agent-harness/cli_anything/magic/skills/SKILL.md)
- [Installed app and source installation](../../agent-harness/cli_anything/magic/README.md)
- [Current app agent contract](../../apps/magicstudio-shell/agent.md)
- [Contribution and native acceptance policy](../../CONTRIBUTING.md)

The former implementation and its verification history remain available in Git.
Do not restore the old CLI command table, UI subprocess round trip, or editable
source installation from those historical instructions.
