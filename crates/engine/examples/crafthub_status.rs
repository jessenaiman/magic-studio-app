//! Example: Query live status of Craft suite apps via CraftHub.
//!
//! Run with:
//!     cargo run -p magic --example crafthub_status

use magic::craft_hub::CraftTarget;

fn main() {
    println!("=== Craft Suite Control Status ===");
    let targets = [
        CraftTarget::Film,
        CraftTarget::Photo,
        CraftTarget::Effect,
        CraftTarget::Vector,
        CraftTarget::Design,
        CraftTarget::Deck,
        CraftTarget::Texture,
    ];

    for target in targets {
        println!(
            "{:<12} default port: {:<5} CLI: {:<16} env: {}",
            target.name(),
            target.default_port(),
            target.cli_name(),
            target.port_env()
        );
    }
}
