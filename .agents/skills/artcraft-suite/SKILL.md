---
name: artcraft-suite
description: Discover and use apps installed through ArtCraft Launcher, including community apps, and Magic Studio. Use for selecting an app, opening it, or checking its live commands.
---

# ArtCraft suite

Use the installed apps as the source of truth; this skill is not an app catalogue.

1. For an app the user named, resolve its version-independent terminal command
   with `command -v <app>` / `command -v <app>-cli`. Read that CLI's `--help`,
   then only the relevant commands or subcommand help it advertises. Do not
   assume every app exposes the same catalogue command or CLI interface.
2. When choosing an app or confirming installation, read the relevant entries
   in `~/.local/share/artcraft-launcher/installed.json`. Include community apps
   listed there, such as SolidCraft; the publisher does not exclude an app.
   Use stable terminal commands rather than the record's versioned file paths.
3. Magic Studio is installed separately: start with `magic-cli --help` and
   `magic-cli commands <relevant filter>`; use `magic` to open the app when
   requested. For project creation and storyboard behavior, read the maintained
   `apps/magic/SKILL.md` in this checkout. App discovery alone does not prove
   a project workflow is implemented or working.

Keep discovery scoped to the requested task. Do not load every app's help,
search old source checkouts, or inventory AppImages to open a named app. Ask
when the intended app or creative requirement is unclear. Launcher updates
change the installed record and live help; no version list belongs in this skill.

The user installs and updates these apps through the official ArtCraft Launcher.
Use the existing version-independent app and CLI commands on PATH. Do not pin
versions, invoke versioned AppImage paths, extract replacement CLIs, or repoint
installed command links. Access to an app does not authorize installation,
reinstallation, updates, or configuration changes; those require a separate
explicit user request. If a command fails, report the exact error and ask before
changing the installation. Installed apps are recorded in
`~/.local/share/artcraft-launcher/installed.json`.
