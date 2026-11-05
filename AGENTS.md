# Magic Studio — agent instructions

The native Rust UI rewrite is in `apps/magic/`. Preserve its work and
the intentional deletion of the former web UI and frontend setup. Do not restore
Tauri, add npm/Bun setup, or reinstate the retired frontend and acceptance checks.
Jesse must explicitly authorize any return to that stack. Do not run the old
checks while this rewrite is underway.

Learn from the apps, not the text. Every ArtCraft app — this one included —
ships a live `--help` and command catalogue that always match the installed
binary; written tool lists go stale the moment an app updates. Discover, don't
assume:

- This app: `magic-cli --help`, `magic-cli commands`, `magic-cli studio list`
- Desktop accessibility tree (`vercel-labs/agent-desktop`): `agent-desktop skills get desktop`, `agent-desktop skills get platform`, `agent-desktop snapshot --app magic -i`
- The suite, one line per app: [`.agents/skills/artcraft-suite/SKILL.md`](.agents/skills/artcraft-suite/SKILL.md)
- Engine behavior on this checkout: the command registry in `crates/engine/src/commands.rs`

Use this checkout as the application source. The maintained Magic skill is
[`apps/magic/SKILL.md`](apps/magic/SKILL.md); read it here rather than
assuming a globally installed skill describes this app. Discover installed
tools through the version-independent CLI names on PATH. Do not substitute
commands or pipeline scripts from another checkout.

## Workflow

- Operate the installed app through `magic` / `magic-cli`. Discover commands
  rather than inventing them; ask when the goal or a choice is unclear.
- For coding, follow [CONTRIBUTING.md](CONTRIBUTING.md).
- Deeper guidance lives in [AGENT_GUIDE.md](AGENT_GUIDE.md) and
  [CLAUDE.md](CLAUDE.md); consult them only when the task needs them.
