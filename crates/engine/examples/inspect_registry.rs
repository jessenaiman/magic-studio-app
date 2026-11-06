//! Example: Inspect Magic Studio engine command registry.
//!
//! Run with:
//!     cargo run -p magic-engine --example inspect_registry

use magic::{Session, command_specs, find_command};

fn main() {
    println!("=== Magic Studio Command Registry ===");
    let session = Session::default();
    let specs = command_specs();
    println!("Total registered commands: {}\n", specs.len());

    for spec in specs.iter().take(10) {
        let enabled = (spec.enabled)(&session);
        let status = if enabled.is_ok() { "yes" } else { "no" };
        println!(
            "- {:<24} enabled: {:<5} params: {}",
            spec.id, status, spec.params
        );
    }
    if specs.len() > 10 {
        println!("... and {} more commands.", specs.len() - 10);
    }

    if let Some(cmd) = find_command("project.open") {
        println!("\nSample lookup: 'project.open'");
        println!("  Label: {}", cmd.label);
        println!("  Params: {}", cmd.params);
    }
}
