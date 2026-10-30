# Magic Studio documentation

Magic Studio brings storyboard, animation, lyrics, and song-video composition
into one desktop workspace. This is the documentation entry point for creators,
contributors, and readers exploring Jesse Naiman's project.

## Choose a path

| I want to… | Start here |
|---|---|
| Understand the project | [Overview and native captures](../README.md) |
| Make a song video | [Production guide](../MAGIC-STUDIO.md) |
| Run or build the app | [Authoritative setup guide](../agent-harness/cli_anything/magic/README.md) |
| Understand how the pieces fit | [Architecture reading guide](architecture/README.md) |
| Browse Rust modules, types, and source | [Build the Rust API reference](RUSTDOC.md) |
| Use CLI or MCP commands | [Command architecture](../agent-harness/MAGIC.md) and [Magic skill](../agent-harness/cli_anything/magic/skills/SKILL.md) |
| Understand Backstage and its limits | [Observer guide](../apps/backstage/README.md) |
| Contribute safely | [Contribution and acceptance policy](../CONTRIBUTING.md) |

## Architecture and API are complementary

The architecture guide explains responsibilities and read/write boundaries.
Rustdoc makes the actual Rust modules, signatures, cross-references, and source
navigable. It does not infer an architecture graph, document the JavaScript or
Python APIs, or prove a workflow works. Existing diagrams are linked with their
scope and limitations in the architecture guide.

The standard reference build covers the shared engine and Backstage. The native
Tauri shell is documented separately when its platform build prerequisites and
locked dependencies are available. No hosted API-docs URL is advertised until
an actual publication has been configured and verified.

## Further reading

- [Project context](../PROJECT_CONTEXT.md): current layers and source contracts.
- [Pipeline internals](ARCHITECTURE.md): retained Python/YAML production toolkit.
- [Agent guide](../AGENT_GUIDE.md): choose the right production layer.
- [Reference → animation → video](workflows/reference-to-animation-video.md).
- [Source attribution](../SOURCE-ATTRIBUTION.md) and [license](../LICENSE).

Historical reviews and implementation handoffs under `architecture/` and
`reviews/` are evidence of their recorded revisions, not current setup commands
or blanket acceptance of this checkout.
