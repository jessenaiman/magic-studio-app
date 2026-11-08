//! Repository tasks: `cargo xtask ci` runs the quality gates (fmt, layers, Clippy, UI build —
//! failures land in `target/ci-report.jsonl`), `layers` the policy gate.
//! There is no install task: build the desktop with `bun run tauri build` in `apps/magic` and install
//! the CLI with `cargo install --path apps/magic-cli`.
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
    /// Check formatting, layers, Clippy (`-D warnings`) and the desktop UI build.
    Ci,
    /// Check Magic's crates against the layers in docs/architecture.md.
    Layers,
}

type Result<T> = std::result::Result<T, String>;

fn main() {
    let result = match Cli::parse().task {
        Task::Ci => ci::ci(),
        Task::Layers => policy::layers(),
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
