# Magic Studio — agent instructions

Learn from the apps, not the text. Every ArtCraft app — this one included —
ships a live `--help` and command catalogue that always match the installed
binary; written tool lists go stale the moment an app updates. Discover, don't
assume:

- This app: `magic-cli --help`, `magic-cli commands`, `magic-cli studio list`
- Desktop accessibility tree (`vercel-labs/agent-desktop`): `agent-desktop skills get desktop`, `agent-desktop skills get platform`, `agent-desktop snapshot --app magic-studio -i`
- The suite, one line per app: [`.agents/skills/artcraft-suite/SKILL.md`](.agents/skills/artcraft-suite/SKILL.md)
- Engine behavior on this checkout: the command registry in `crates/engine/src/commands.rs`

## Workflow

- Operate the installed app through `magic` / `magic-cli`. Discover commands
  rather than inventing them; ask when the goal or a choice is unclear.
- For coding, follow [CONTRIBUTING.md](CONTRIBUTING.md).
- Deeper guidance lives in [AGENT_GUIDE.md](AGENT_GUIDE.md) and
  [CLAUDE.md](CLAUDE.md); consult them only when the task needs them.
