# Native acceptance (issue #3)

The replacement runner lives in this directory. **No workflow is approved
or encoded yet.** The Save As architecture document is a pending specification,
not approval. Local agents own real UI observation, Jesse's per-workflow approval,
and subsequent approved-case authoring. Cloud work covers infrastructure,
retirement, static checks and draft PR review only; it has not established a
native behavioral pass. The future TUI consumes evidence; it is not implemented here.

Pinned registry versions: `@wdio/tauri-service` **1.5.1**, and `@wdio/cli`,
`@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`,
`@wdio/json-reporter`, `webdriverio` **9.32.0**. The service peers WDIO `^9`;
latest WDIO 10 is intentionally not selected. `acceptance/package-lock.json`
locks transitive packages. Use Node 20+ and `npm ci --ignore-scripts` there.
The production frontend remains Bun-built, with its existing production lockfile.

Linux uses `driverProvider: 'external'`, external `tauri-driver` plus
`WebKitWebDriver`, and one instance. The pinned 1.5.1 package source normalizes the deprecated `official` alias to
`external` (`dist/esm/index.js`, `normalizeTauriOptions`/`warnIfOfficialDriverProvider`);
this intentionally follows the installed package rather than lagging website examples. No Rust automation plugin, backend execute helper, mocked invoke,
embedded debug server, browser-only fallback or paid driver is installed.
`autoInstallTauriDriver` is false: system prerequisites are explicit.

Local owner preparation, only when ready to run an approved workflow:

1. Build **this checkout**: `cargo build -p magicstudio-shell` (Rust/toolchain and
   platform Tauri build dependencies required). Rebuild after source changes;
   never substitute an old installed native binary. The runner fixes the backend
   override and WDIO application to this repository's `target/debug/magicstudio-shell`.
2. Install `agent-harness` editable into the Python environment whose installed
   `magic-cli` is on PATH. The launcher checks that console script's interpreter
   resolves `cli_anything.magic.magic_cli` to this checkout before any workflow.
   The repository installer writes this exact absolute-Python-shebang entry point
   (`scripts/install_magic.py`); standard editable console-script installs match it.
3. Install compatible `tauri-driver` and the distro's WebKitGTK driver
   (`webkit2gtk-driver` on Debian/Ubuntu, `webkitgtk-6.0` on Arch/Omarchy, providing `WebKitWebDriver`). Have a
   headed desktop session for human review; `xvfb-run` is for unattended CI, not
   Jesse's observation/approval.
4. In `apps/magicstudio-shell/acceptance`, run `npm ci --ignore-scripts`,
   `npm run check`, then **`npm test`**. Empty approval inventory exits **2**, prints
   **pending / zero coverage**, and lists missing native prerequisites. It is not
   a green test run. Do not bypass the launcher by invoking WDIO directly.

After Jesse observes the real UI, record the exact command/action → result →
window/project → saved-file → visible-response chain and approval reference in a
reviewed change. Only then add its `approved/<name>.spec.mjs` and an entry in
`approved-workflows.json` with `spec`, `reference`, `approvedBy`, `approvedAt`.
Use WDIO/Mocha standard assertions and `createWorkflow` in `workflow.mjs`.
**Actions are `magic-cli` commands** (e.g. `project save path=…`, `ui click`, `ui key`,
`ui state`): with `--project <song copy>` the CLI routes to the one live window that owns
that project over `$XDG_RUNTIME_DIR/magicstudio-studio/*.sock`, and refuses if none or
several match. WDIO launches the real binary and **reads** the webview DOM
(header, title, footer build) as independent UI evidence. WebDriver only reaches web
content; native GTK menus and file choosers are outside it and are not automated here.
The official test song is **Nelly**. The launcher copies the needed parts of
`/run/media/jesse/Speedy/media/merriweather/nelly` into the run before launch and passes
the copied `Nelly.magic` as the app's launch argument (`tauri:options.args`, forwarded by
`tauri-driver`), so the window opens that project directly. The original is never
written. Never count a headless CLI result as a desktop pass: assert the step reached
the window (`ui feedback`/`ui state` identity matches the launched project).
A future approved MCP parity case must use the official SDK with `magic-cli mcp`.

Runs live in `acceptance/artifacts/run-<time>/` (git-ignored): standard spec/JSON
reports, `steps.jsonl` (actions, expectations, CLI exit/stdout/stderr, file hashes) and
640px JPEG previews. The song copy is deleted at the end of every run. Only the newest
passing run is kept, plus the newest failed run if it is more recent; everything else is
pruned automatically. Rerun a case if more detail is needed. Unknown CLI outcomes block
further steps without retries. WDIO closes its own driver session; helpers never kill
other app instances.

CI's syntax/source-lint job does not establish acceptance coverage. Manual native
CI uses a separately provisioned Linux `magic-native` runner and the exact same
`npm test` path; unavailable prerequisites or unapproved coverage fail explicitly,
with diagnostic artifacts uploaded. No cloud job installs system GUI/Rust tools
or operates Jesse's live desktop.

References (link rather than copy):
[Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/),
[service configuration](https://webdriver.io/docs/desktop-testing/tauri/configuration),
[Linux prerequisites](https://webdriver.io/docs/desktop-testing/tauri/platform-support),
[Tauri CI](https://v2.tauri.app/develop/tests/webdriver/ci/).
