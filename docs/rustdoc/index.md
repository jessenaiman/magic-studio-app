# Magic Studio Rust reference

Storyboard, animation, lyrics, and song-video composition share a native Rust
engine. This reference opens the implementation behind Magic Studio and the
separate Backstage observer, without replacing the creator and architecture
guides.

## Explore the code

| Reference | Responsibility | Start with |
|---|---|---|
| [Shared engine](magicstudio_engine/index.html) | Project state, validated command dispatch, domain operations | [Session](magicstudio_engine/session/struct.Session.html), [command registry](magicstudio_engine/commands/index.html) |
| [Backstage](backstage/index.html) | Bounded observations, PR handoffs, existing audit evidence, terminal views | [Observation provider](backstage/provider/index.html), [handoff](backstage/handoff/index.html), [audit](backstage/audit/index.html) |

Each crate page provides searchable items, module navigation, and source links.
Press `S` or `/` on a crate page to search. Engine pages describe public Rust
items; Backstage's binary pages include private implementation details and are
not a stable external library API.

## Understand the architecture

```text
Desktop UI / CLI / MCP → shared command registry → engine session and project
Backstage → explicit bounded observations → saved references and native evidence
```

Start with the repository's
[documentation index](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/README.md),
[architecture reading guide](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/architecture/README.md),
and [project context](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/PROJECT_CONTEXT.md).
Those explain ownership and the retained Python/YAML production toolkit. Rustdoc
does not infer an architecture graph or document JavaScript/Python APIs.
Repository links track the default branch; newly added guide pages are not live
at those links until this documentation commit merges.

## Scope and provenance

The standard reference includes **engine + Backstage**, not the native Tauri
shell. Shell documentation needs the optional workspace build and its platform
prerequisites. Generated pages are implementation reference, not evidence of
runtime acceptance, working providers, or completed creative output.

See [build instructions and coverage](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/RUSTDOC.md),
the [project overview](https://github.com/jessenaiman/magic-studio-app),
[source attribution](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/SOURCE-ATTRIBUTION.md),
and [license](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/LICENSE).
