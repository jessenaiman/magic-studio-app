# Magic Studio

The active desktop app is `magic-tauri` (see [its README](apps/magic-tauri/README.md)); the
installed `magic` binary is the legacy egui app. Discover operations with `magic-cli --help` and
`magic-cli commands`. Drive a running desktop with an explicit bridge
(`magic-cli --bridge 127.0.0.1:PORT ...`, [control protocol](docs/control-protocol.md)).
Ask when the goal or a choice is unclear.
For coding, follow the [architecture](docs/architecture.md) layers and the
[command contract](apps/magic/MAGIC.md).
