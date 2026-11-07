//! `cargo xtask ci`: run the Craft native quality gates.
//! Normal app install: `cargo build -p magic-tauri -p magic-cli && cargo install -p magic-tauri -p magic-cli`
//! Bundle: `cargo tauri build -p magic-tauri` (Tauri bundler makes AppImage/deb/dmg). No manual install code.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_names_in_the_shared_bin_are_removed_only_when_links() {
        let root = std::env::temp_dir().join(format!("xtask-retired-{}", std::process::id()));
        let (bin, links) = (root.join("share/magic/bin"), root.join("bin"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&links).unwrap();
        std::fs::write(bin.join("magic-shell"), "old app").unwrap();
        std::fs::write(links.join("python-magic-cli"), "someone else's").unwrap();
        std::os::unix::fs::symlink(bin.join("magic-shell"), links.join("magic-shell")).unwrap();
        remove_retired(&root, &bin).unwrap();
        assert!(!bin.join("magic-shell").exists());
        assert!(!links.join("magic-shell").is_symlink());
        assert!(links.join("python-magic-cli").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn runtime_resources_exclude_tests_and_unlisted_files() {
        assert!(wanted_resource("pipeline/templates/words-kinetic.html"));
        assert!(wanted_resource("apps/magic/agent.md"));
        assert!(!wanted_resource("pipeline/tests/test_layout.rs"));
        assert!(!wanted_resource("lib/test_helpers.rs"));
        assert!(!wanted_resource("apps/magic/src/main.rs"));
    }
}
