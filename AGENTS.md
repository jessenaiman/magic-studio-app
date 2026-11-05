# Magic Studio — agent instructions

- This app: `magic` ui, `magic-cli --help`, `magic-cli commands`, `magic-cli studio list`
- Desktop accessibility tree (`vercel-labs/agent-desktop`): `agent-desktop skills get desktop`, `agent-desktop skills get platform`, `agent-desktop snapshot --app magic -i`
- The suite, one line per app: [`.agents/skills/artcraft-suite/SKILL.md`](.agents/skills/artcraft-suite/SKILL.md)
- Engine behavior on this checkout: the command registry in `crates/engine/src/commands.rs`

### Rules
- Keep all work within this project. 
- Add reports and follow up details to issues, or to the PR that failed
- use git commit to force xtask and other checks on your code changes instead of manually reviewing
- improve and verify xtask checked your code correctly and is not missing others if and when you need to change configuration
- if you see a bug that can be fixed, you must fix it and report that you handled it. Never leave issues unhandled.
- when making a change that the user has not explicetly confirmed you must commit your changes with a github issue attached throughout the development so the user can review decisions and cherry pick the correct steps if you stray.

Use this checkout as the application source. The maintained Magic skill is
[`apps/magic-gui/SKILL.md`](apps/magic-gui/SKILL.md); read it here rather than
assuming a globally installed skill describes this app. Discover installed
tools through the version-independent CLI names on PATH. Do not substitute
commands or pipeline scripts from another checkout.

## Workflow

- Operate the installed app through `magic` / `magic-cli`. Discover commands
  rather than inventing them; ask when the goal or a choice is unclear.
- For coding, follow [CONTRIBUTING.md](CONTRIBUTING.md).
- Deeper guidance lives in [AGENT_GUIDE.md](AGENT_GUIDE.md) and
  [CLAUDE.md](CLAUDE.md); consult them only when the task needs them.
