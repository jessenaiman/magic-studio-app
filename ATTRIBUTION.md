# Asset attribution index

This index records the four asset files currently present in the checkout.
Asset-specific author, source and license records have not been found. Listing
a file here does not establish permission to redistribute it or assign it the
repository's source-code license. See [source attribution](SOURCE-ATTRIBUTION.md)
for the separate source-code history and unresolved Cargo license declaration.

| Asset | Verified local origin | Asset-specific rights record |
|---|---|---|
| `apps/magic-gui/icons/icon.png` | Byte-identical to `apps/src-tauri/icons/icon.png` in Git commit `b6350c5ad59759233631f30e258f570fc708da8d`; retained by the native rewrite. | Not established. |
| `references/backstage/mayhem-pr30.png` | Tracked path last changed in commit `95acaf75a71695fd8c8efb9984d610864bb8af0f`. | Not established. |
| `references/backstage/mops-pr22.png` | Tracked path last changed in commit `95acaf75a71695fd8c8efb9984d610864bb8af0f`. | Not established. |
| `references/backstage/mops-task-handoff-v2.png` | Tracked path last changed in commit `88d71aa2df1f0bc48f29e5d247c30120255039b0`. | Not established. |

FilmCraft's asset gate requires a `<path>.attribution` sidecar with nonempty
`asset`, `title`, `author`, `source`, `license`, and `added` fields. No sidecars
have been fabricated. Record verified provenance and an allowed license for
each asset before filling these fields; commit authorship alone does not prove
asset authorship or licensing. Until then, `cargo xtask assets` must fail.
