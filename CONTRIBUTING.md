# Contributing to Magic

Read [AGENTS.md](AGENTS.md) first. This is an adapted copy of the Craft code
rules for this application's Rust workspace. The sources and their exact
revisions are listed below. It is not a claim that existing Magic code already
meets every rule. Existing user instructions take precedence.

## Architecture and reuse

Adapted from FilmCraft and EffectCraft's engineering rules and PhotoCraft and
DesignCraft's thin-UI conventions:

- Keep the native product Rust-based. Magic's desktop frontend is egui/eframe.
  Keep engine behavior independent of the frontend.
- Put document operations in the owning engine or domain crate. The UI reads
  state and dispatches commands; it must not implement another document engine.
- Every user-visible operation has a command with a stable ID, label, menu path,
  shortcut where appropriate, documented parameters, enabled predicate, and
  handler. UI, CLI, control channel and MCP dispatch the same behavior.
- `magic-cli` is the app's agent-facing command interface. Keep it thin; do not
  duplicate engine behavior in the CLI.
- Register stable automation IDs for interactive widgets. Make relevant UI state
  inspectable and controllable through the existing automation interface.
- Use the existing Craft engine and shared library APIs before implementing an
  alternative. Keep platform and UI dependencies out of lower-level domain code.
  Follow the actual layer mapping enforced by `xtask/src/policy.rs`; upstream
  projects' numerical layer assignments are not interchangeable.
- Put new command families in focused modules rather than growing a shared
  registry or application file. Register those modules with small edits.
- New crates inherit workspace lint settings with `[lints] workspace = true`.
  Keep every manifest valid: the workspace discovers crates and apps by glob.
- Use the owning engine's exact time type for timeline and keyframe arithmetic.
  Convert seconds at API boundaries; do not build parallel floating-point edit
  math. Use `chrono` for calendar/UTC conversion rather than handwritten calendar
  algorithms when calendar handling is needed.

Magic-specific requirements from the maintainer:

- Follow Craft Anything as the native UI reference. Use shared Craft components
  and conventions, with Magic's own application content.
- Do not introduce replacement wrappers that merely rename existing behavior,
  parallel save implementations, or a second reporting framework.
- Keep modules focused. Put reports and follow-up findings in issues or the
  affected PR. Use the existing `xtask` gates.
- Make small, reviewable patches. Do not use ad hoc Python source-rewriting
  scripts for routine Rust edits.

## Never crash or lose work

Adapted from the Craft projects' common reliability rules:

- Return `Result<T, E>` through the crate's error type. Propagate with `?` and
  attach useful context. Report errors in the UI and agent response.
- No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or `unimplemented!` in
  production paths. An unfinished operation returns an unsupported-operation
  error. A fallback is valid only if it cannot silently corrupt a document.
- The upstream exception is a locally provable, infallible literal operation,
  with a narrowly scoped lint allowance and an explanation. Do not broaden that
  exception to file, user, network or agent input.
- No `unsafe` in Magic: preserve its workspace prohibition. FilmCraft's special
  platform-FFI exception is not an exception for this repository.
- Treat input-derived indices, lengths, counts and dimensions as untrusted.
  Validate bounds or use `get`, checked/saturating arithmetic, allocation limits,
  and character-boundary-safe string slicing. Guard zero divisors and nonfinite
  numeric values.
- Bound recursion and detect cycles in document graphs. Handle poisoned locks,
  failed worker jobs and failed joins without cascading into another failure.
- Panic-catching boundaries, where present, are last-resort protection, not
  permission to panic inside handlers. Preserve documents on failure.
- Every crash fix gets a small synthetic regression that reproduced the crash.
  Tests and development tooling may use assertions and `unwrap` for failures.

## Saving and view ownership

These are the maintainer's Magic requirements, not quotations from upstream:

- Underlying documents autosave, or their active view owns saving and closing
  them when switching away. Reuse the owning engine's save operation.
- Complete a required save before disposing the document or completing a view
  transition. A failed save keeps the document available and reports the failure;
  do not silently discard edits or report the switch as successful.
- UI navigation and agent-driven view switching use the same lifecycle behavior.
- Magic's project metadata and links remain distinct from each underlying
  document's content. Persist each through its owner; do not serialize another
  app's document into a competing Magic representation.
- Do not replace atomic-save or document-locking behavior without preserving and
  testing both guarantees. Test failed saves and reopening, not just command
  acknowledgements. Preserve format compatibility and unknown data where required.

## Tests and code organization

The Craft sources use separate test modules and public-API integration tests:

- Unit tests for internal behavior belong in focused files such as
  `src/<area>/tests.rs`, `src/<area>_tests.rs`, or modules under `src/tests/`.
  Production files need only their small `#[cfg(test)] mod tests;` declaration.
- Public command/API tests belong in `crates/<crate>/tests/`. Exercise the real
  session API and assert observable results, persisted content, and errors.
- EffectCraft's testing guide specifies one integration-test binary for a crate
  with several integration-test files: `tests/it/main.rs` declares the separate
  modules under `tests/it/`. Use that grouping as a suite grows; do not duplicate
  the same tests in standalone files and grouped modules.
- Search existing coverage first. Test Magic's integration contracts, not another
  copy of a Craft dependency's codec, serializer, or UI implementation.
- Changed behavior needs appropriate regression coverage. For command changes,
  include invalid parameters and disabled cases; test undo/redo where supported.
  For formats, use round trips, small synthetic fixtures and hostile input.
- Keep test helpers in test modules or test support files. Do not add production
  APIs solely for tests or accumulate test bodies inside a large application file.
- Generate disposable media fixtures under `target/fixtures/`. Large real-file
  corpora are external, revision-pinned and checksum-verified; do not commit them.
- Verify native UI changes visually using the app's supported screenshot/render
  commands. Inspect the image and exercise the actual interaction. A command
  acknowledgement or compilation alone does not prove a working UI.

## Verification and commits

Craft's standard is formatted, lint-clean, tested, layer-correct changes before
commit. Magic provides these existing commands:

```sh
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo xtask layers
cargo xtask assets
cargo xtask ci
```

`cargo xtask ci` runs doctor, formatting, Clippy, tests (nextest when available,
otherwise cargo test), doctests and layers. Set `CHECK_ASSETS=1` to include the
asset check. CI uses the pinned Rust toolchain and the committed Cargo.lock.
Record the results of the gates you run.

Run `git config --local core.hooksPath .githooks` after cloning to enable the
pre-commit hook. The hook runs `cargo xtask ci`.
Magic has no `cargo xtask wasm` subcommand; do not copy that upstream command as
though it exists here. Changes affecting the web target must verify the supported
web build separately and report any limitations.

Use `git commit` to invoke the existing hook. Do not bypass checks. Keep commits
focused and preserve unrelated user changes. Follow AGENTS.md's issue-linking
and reporting requirements. If gate configuration changes, verify what the gate
actually executes and that failures cause a nonzero exit status.

Do not add or modify CI jobs as incidental feature work. The Craft projects use
shared `storytold/craft-common-actions` workflows; CI migrations and changes to
shared gates are separately scoped work.

## Code and asset provenance

The upstream Craft rules require clean-room implementations from public
specifications and observable behavior, permissively licensed dependencies, no
copied proprietary or GPL/LGPL/AGPL implementation code, and no Adobe artwork,
icons, presets, fonts or other assets. External ffmpeg/ffprobe are test oracles
and fixture generators, not bundled codec implementations.

For new distributed assets, establish the author's permission and allowed license,
retain required license notices, add a `<file>.attribution` sidecar and an entry
in [ATTRIBUTION.md](ATTRIBUTION.md). Do not invent attribution or infer rights from
Git authorship. Shared fonts belong in `storytold/craft-fonts`, not newly copied
font files in this repository.

These imported contribution rules do not relicense Magic, establish rights to its
existing files, or resolve the existing provenance gaps recorded in ATTRIBUTION.md.
Do not copy upstream product branding or claim affiliation as part of this copy.

## Sources

Adapted from the source revisions pinned by Magic's Cargo.lock, inspected locally.
These are reproducible references, not a claim to track upstream HEAD:

- [FilmCraft AGENTS.md](https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/AGENTS.md),
  [CLAUDE.md](https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/CLAUDE.md),
  and [contributing guide](https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/docs/contributing.md).
- [EffectCraft AGENTS.md](https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/AGENTS.md),
  [CLAUDE.md](https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/CLAUDE.md),
  [CONTRIBUTING.md](https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/CONTRIBUTING.md),
  and [testing guide](https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/docs/testing.md).
- [PhotoCraft AGENTS.md](https://github.com/storytold/photocraft/blob/b3fd0159ec85a9dc1cbbfdfb5defc8f9afe5d19c/AGENTS.md).
- [DesignCraft AGENTS.md](https://github.com/storytold/designcraft/blob/4f3bb5340eeee5f747f7ee2a6905d3c990831d22/AGENTS.md).

The rules above are adapted guidance. The test-directory examples also reflect
the actual source layouts at those revisions. App-specific parity targets,
maintainer-local planning paths, platform exceptions, and unavailable xtask
commands have not been transplanted into Magic.
