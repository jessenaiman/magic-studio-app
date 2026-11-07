//! Repository tasks: `cargo xtask ci` runs the native quality gates, `verify` every gate with a machine-readable
//! problem report (`target/verify/report.jsonl`), `bench` records and compares size/CPU numbers, `layers` and
//! `assets` the policy gates. There is no install task:
//! build the desktop with `bun run tauri build` in `apps/magic` and install the CLI with `cargo install --path apps/magic-cli`.
use clap::{Parser, Subcommand};
mod bench;
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
    /// Run every gate (Rust + UI + assets), collect ALL problems into target/verify/report.jsonl.
    Verify,
    /// Record source lines, binary sizes and engine cold start into target/bench/results.jsonl.
    Bench {
        /// Compare against a previous results.jsonl and print the deltas.
        #[arg(long)]
        compare: Option<std::path::PathBuf>,
    },
    /// Check Magic's crates against the layers in docs/architecture.md.
    Layers,
    /// Check that every asset has an attribution sidecar and an ATTRIBUTION.md entry.
    Assets,
}

type Result<T> = std::result::Result<T, String>;

fn main() {
    let result = match Cli::parse().task {
        Task::Ci => ci::ci(),
        Task::Verify => ci::verify(),
        Task::Bench { compare } => bench::bench(compare),
        Task::Layers => policy::layers(),
        Task::Assets => policy::assets(),
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
