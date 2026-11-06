//! Interactive Terminal User Interface (TUI) for Magic Studio (`apps/magic-tui`).
use clap::Parser;
use magic::{Session, command_specs};
use std::io::{self, Write};

#[derive(Parser)]
#[command(
    name = "magic-tui",
    about = "Magic Studio interactive terminal UI dashboard"
)]
struct Cli {
    /// Open this `.magic` project at launch
    #[arg(long)]
    project: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut session = Session::default();

    if let Some(path) = cli.project {
        println!("Opening project: {path}...");
        match session.execute("project.open", serde_json::json!({ "path": path })) {
            Ok(res) => println!("Project loaded: {res}"),
            Err(e) => eprintln!("Error loading project: {e}"),
        }
    }

    println!("==================================================");
    println!(" Magic Studio TUI Dashboard");
    println!("==================================================");
    println!("Available engine commands:");
    for spec in command_specs().iter().take(10) {
        println!("  - {:<24} {}", spec.id, spec.label);
    }
    println!("  ... and more (type 'help' or command id to run)");
    println!("Type 'quit' or 'exit' to leave.");
    println!("==================================================");

    let stdin = io::stdin();
    loop {
        print!("magic-tui> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if matches!(trimmed, "quit" | "exit" | "q") {
            println!("Goodbye!");
            break;
        }
        if trimmed == "status" {
            println!("Session project: {:?}", session.project_path);
            continue;
        }

        // Execute command
        match session.execute(trimmed, serde_json::json!({})) {
            Ok(val) => println!("{val}"),
            Err(err) => eprintln!("Error: {err}"),
        }
    }

    Ok(())
}
