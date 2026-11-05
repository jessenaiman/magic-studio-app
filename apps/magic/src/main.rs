//! Native Magic window entry point.
//!
//! `magic` opens the GUI. `magic-cli` is a separate binary for CLI/agent use.

mod app;

use clap::Parser;
use std::path::PathBuf;

/// Operate Magic: projects, animation, and the desktop app.
///
/// Open an existing project window:
///   magic --project /absolute/Song.magic
#[derive(Parser)]
#[command(name = "magic", version)]
struct Cli {
    /// Select the intended .magic project
    #[arg(long)]
    project: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let project = cli.project;
    eframe::run_native(
        "Magic",
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(app::magicApp::new(cc, project)))),
    )
    .expect("Failed to run Magic");
}
