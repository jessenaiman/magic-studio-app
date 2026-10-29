# Rust API reference

Use the `rustdoc` shipped with the existing Rust toolchain. No documentation
framework, Python setup, app installation, model, or provider call is needed.
The workspace currently requires Rust 1.95; see `Cargo.toml`. These commands run
from the repository root and do not run tests or doctests.

## Build and browse the standard reference

```sh
cargo doc --no-deps --locked -p magicstudio-engine -p backstage
rustdoc --edition=2024 docs/rustdoc/index.md --markdown-no-toc \
  --markdown-css magic-rustdoc.css --out-dir "${CARGO_TARGET_DIR:-target}/doc"
cp docs/rustdoc/magic-rustdoc.css "${CARGO_TARGET_DIR:-target}/doc/"
```

Open `${CARGO_TARGET_DIR:-target}/doc/index.html` in your browser. Alternatively,
add `--open` to the Cargo command to open a crate page automatically. The native
landing page links to:

| Crate | Generated entry point | Coverage |
|---|---|---|
| `magicstudio-engine` | `magicstudio_engine/index.html` | Public modules, types, command registry, session, and domain handlers |
| `backstage` | `backstage/index.html` | Binary implementation, observation boundaries, view and data modules |

On crate pages, use the sidebar or **All Items**, the search box (`S` or `/`),
and **Source** links to navigate. Backstage is a binary, so Cargo includes its
private items by default; this is implementation reference, not a promised
external library API. Engine private items are omitted unless requested.

`--no-deps` omits dependency documentation, not dependency compilation. Cargo may
still need locked dependency sources and platform prerequisites. Use `--offline`
when network access/downloads are not authorized; a missing cached crate is a
blocker, not a reason to silently fetch it. `--locked` preserves `Cargo.lock`.

## Optional native-shell and internal reference

In the already prepared [native development environment](../agent-harness/cli_anything/magic/README.md):

```sh
cargo doc --workspace --no-deps --locked
```

This adds `magicstudio_shell/index.html`, including the shell's binary-private
implementation. It requires the shell's locked dependencies, build script, and
platform Tauri prerequisites. Do not claim shell coverage if that command fails.
To additionally inspect engine internals, append `--document-private-items` to
the standard Cargo command. Neither option is required for the public reference.

## Keep output and coverage honest

Generated pages stay under `target/doc` (or `$CARGO_TARGET_DIR/doc`) and must not
be committed. Cargo preserves old doc files across builds, so a leftover crate
page is not evidence that the current build covered it. Search indexes and
`static.files` are part of the site; copying only an `index.html` breaks browsing.

On 29 October 2026, the standard engine + Backstage build succeeded with
`--offline` on Rust 1.95.0. After authorized fetching of the existing locked
dependencies, the full-workspace build was blocked by the host's missing
`dbus-1 >= 1.6` development metadata (`dbus-1.pc`, required by `libdbus-sys`).
No OS packages were installed; shell-doc success is not implied. Re-run in a
prepared native development environment to establish shell coverage.

## GitHub visibility and optional hosting

The [repository documentation](README.md) and
[architecture reading guide](architecture/README.md) are readable on GitHub
without deploying anything. Links from generated pages to repository guides
track the default branch: newly added guide pages are not live at those links
until this documentation commit merges. To host the API reference, publish the
complete `target/doc` directory through an explicitly approved GitHub Pages
workflow using the standard Pages artifact/deployment actions. A maintainer must
configure Pages and authorize deployment separately; building docs does neither.
Do not advertise a Pages URL until the deployed page and navigation are verified.
Use GitHub's [official Pages workflow guide](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
for that publishing configuration.

See the official [Cargo doc command](https://doc.rust-lang.org/cargo/commands/cargo-doc.html),
[Rustdoc introduction](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html), and
[documentation-writing guide](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html).
Return to the [documentation index](README.md) for non-Rust layers and credits.
