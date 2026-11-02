# Embedded frontend bundles

Build the declared dependencies with `bun run build` in this directory before
building the Tauri app. The generated bundles are embedded with the other UI files.

Bun 1.4.4 rewrites JSZip's optional CommonJS fallback to `__require` without
declaring it. The browser build banner declares that absent fallback explicitly;
JSZip's embedded browser modules supply its actual dependencies.

## Learning editor

`bun run build:learning` builds pinned Milkdown Crepe 7.22.3 (MIT) for the existing vanilla shell. No AI feature is enabled. Preserve the upstream MIT notice when distributing the generated bundle: https://github.com/Milkdown/milkdown/blob/main/LICENSE. Task bodies use structured Markdown sections; opaque frontmatter is retained. Unsupported HTML/extensions are refused rather than discarded.

Legacy VM/browser regression suites have been retired. Native acceptance requires explicit human approval of each workflow; the Save As specification is not approval.

## Backstage terminal

`bun run build:backstage` bundles pinned xterm.js 6.0.0 and FitAddon 0.11.0
with their MIT notices. The native shell build also regenerates this bundle. Embedded PTY sessions
currently support Linux only; other platforms receive a visible unsupported
message and can use standalone Backstage.
The desktop uses the companion `backstage` beside its executable via
portable-pty 0.9.0; no shell or network listener is involved. Output has one
8 KiB credit, acknowledged after xterm parses it; a lost renderer stops the
session after five seconds. Bounded input delivery never holds the lifecycle
lock, and Stop bypasses queued input. Session IDs reject delayed commands
from an earlier launch; a reloaded page can explicitly stop its old session. Mayhem's badge
uses observed missing executables and saved scan warnings; overall health
stays Unknown without audit evidence.

API references: [xterm Terminal](https://xtermjs.org/docs/api/terminal/classes/terminal/),
[FitAddon](https://github.com/xtermjs/xterm.js/tree/master/addons/addon-fit),
[portable-pty](https://docs.rs/portable-pty/0.9.0/portable_pty/).
