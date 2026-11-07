//! Repository tasks: `cargo xtask ci` runs the native quality gates, `layers` and `assets` the policy gates. There is no install task:
//! build the desktop with `bun run tauri build` in `apps/magic` and install the CLI with `cargo install --path apps/magic-cli`.
use clap::{Parser, Subcommand};
mod ci;
mod policy;

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Check formatting, Clippy, workspace tests and doctests using Craft's native gates.
    Ci,
    /// Check Magic's crates against the layers in docs/architecture.md.
    Layers,
    /// Check that every asset has an attribution sidecar and an ATTRIBUTION.md entry.
    Assets,
}

type Result<T> = std::result::Result<T, String>;

fn main() {
    let result = match Cli::parse().task {
        Task::Ci => ci::ci(),
        Task::Layers => policy::layers(),
        Task::Assets => policy::assets(),
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
