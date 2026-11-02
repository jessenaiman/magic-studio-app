//! `cargo xtask install`: install this built checkout's Magic Studio app, `magic-cli`,
//! Backstage, runtime resources and desktop entry for the current user. It installs Magic
//! only; it never installs or configures other Craft apps. Build first:
//!   cargo build -p magic-studio -p magic-cli -p backstage [--release]
use clap::{Parser, Subcommand, ValueEnum};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Install the built app, CLI, resources and desktop entry under PREFIX.
    Install {
        /// Install prefix (binaries under PREFIX/share/magic-studio/bin, links in PREFIX/bin).
        #[arg(long)]
        prefix: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "debug")]
        profile: Profile,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Profile {
    Debug,
    Release,
}

/// The binaries this repository builds and installs.
const BINARIES: &[&str] = &["magic-studio", "magic-cli", "backstage"];
/// Names earlier installs left behind: the old app name and the retired Python CLI.
const RETIRED: &[&str] = &["magicstudio-shell", "python-magic-cli"];
/// Tracked runtime resources the app reads at run time.
const RESOURCE_PREFIXES: &[&str] = &["pipeline/", ".agents/skills/"];
const RESOURCE_FILES: &[&str] = &[
    "apps/src-tauri/agent.md",
    "config.yaml",
    "SOURCE-ATTRIBUTION.md",
    "LICENSE",
];

type Result<T> = std::result::Result<T, String>;

fn io<T>(what: impl std::fmt::Display, r: std::io::Result<T>) -> Result<T> {
    r.map_err(|e| format!("{what}: {e}"))
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = io(
        "git",
        Command::new("git").args(args).current_dir(root).output(),
    )?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Copy beside the destination, then rename: a running program keeps its old inode.
fn replace_file(source: &Path, target: &Path) -> Result<()> {
    let staged = target.with_extension("new");
    io(staged.display(), std::fs::copy(source, &staged))?;
    io(target.display(), std::fs::rename(&staged, target))
}

fn wanted_resource(name: &str) -> bool {
    let tested = Path::new(name)
        .components()
        .any(|c| c.as_os_str() == "tests")
        || Path::new(name)
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("test_"));
    (RESOURCE_PREFIXES.iter().any(|p| name.starts_with(p)) || RESOURCE_FILES.contains(&name))
        && !tested
}

/// Copy tracked runtime resources and record which source and app build they came from.
fn install_resources(root: &Path, prefix: &Path, app: &Path) -> Result<()> {
    let runtime = prefix.join("share/magic-studio/runtime");
    for name in git(root, &["ls-files", "-z"])?
        .split('\0')
        .filter(|n| wanted_resource(n))
    {
        let source = root.join(name);
        if !source.is_file() || source.is_symlink() {
            continue;
        }
        let target = runtime.join(name);
        io(
            target.display(),
            std::fs::create_dir_all(target.parent().expect("file has a folder")),
        )?;
        io(target.display(), std::fs::copy(&source, &target))?;
    }
    let revision = git(root, &["rev-parse", "HEAD"])?.trim().to_owned();
    let dirty: Vec<String> = git(root, &["diff", "--name-only", "HEAD"])?
        .lines()
        .map(str::to_owned)
        .collect();
    let native = format!(
        "{:x}",
        Sha256::digest(io(app.display(), std::fs::read(app))?)
    );
    let record = serde_json::json!({"source_revision": revision, "modified_source_files": dirty, "native_sha256": native});
    let text = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    io(
        "installation.json",
        std::fs::write(runtime.join("installation.json"), text),
    )
}

/// Point PREFIX/bin/NAME at the installed file; never replace a regular file we did not make.
fn link(entry: &Path, target: &Path) -> Result<()> {
    if target.is_symlink() {
        io(target.display(), std::fs::remove_file(target))?;
    } else if target.exists() {
        return Err(format!(
            "Refusing to replace an unrelated regular executable: {}",
            target.display()
        ));
    }
    io(target.display(), std::os::unix::fs::symlink(entry, target))
}

/// Remove what earlier installs left: the old app name, the Python CLI and its environment.
fn remove_retired(prefix: &Path, bin: &Path) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for name in RETIRED {
        for path in [bin.join(name), prefix.join("bin").join(name)] {
            if path.is_symlink() || path.is_file() {
                io(path.display(), std::fs::remove_file(&path))?;
                removed.push(path);
            }
        }
    }
    // The old `magic` launcher was a Python script; only that script is ours to replace.
    let launcher = bin.join("magic");
    let python_launcher = !launcher.is_symlink()
        && std::fs::read_to_string(&launcher).is_ok_and(|s| s.contains("cli_anything.magic"));
    if python_launcher {
        io(launcher.display(), std::fs::remove_file(&launcher))?;
        removed.push(launcher);
    }
    let venv = prefix.join("share/magic-studio/.venv");
    if venv.is_dir() {
        io(venv.display(), std::fs::remove_dir_all(&venv))?;
        removed.push(venv);
    }
    Ok(removed)
}

fn install_desktop_entry(root: &Path, prefix: &Path) -> Result<()> {
    let icon = prefix.join("share/icons/hicolor/256x256/apps/ai.storyteller.magicstudio.png");
    io(
        icon.display(),
        std::fs::create_dir_all(icon.parent().expect("icon folder")),
    )?;
    io(
        icon.display(),
        std::fs::copy(root.join("apps/src-tauri/icons/icon.png"), &icon),
    )?;
    let template = io(
        "magic.desktop",
        std::fs::read_to_string(root.join("apps/src-tauri/magic.desktop")),
    )?;
    let launcher = prefix.join("bin/magic");
    let entry = template
        .replace("Name=Magic Studio", "Name=Magic ✨")
        .replace(
            "Exec=magic %f",
            &format!("Exec=\"{}\" %f", launcher.display()),
        );
    let desktop = prefix.join("share/applications/ai.storyteller.magicstudio.desktop");
    io(
        desktop.display(),
        std::fs::create_dir_all(desktop.parent().expect("applications folder")),
    )?;
    io(desktop.display(), std::fs::write(&desktop, entry))
}

/// The build folder holding every binary this repository installs.
fn built(root: &Path, profile: Profile) -> Result<PathBuf> {
    let (folder, flag) = match profile {
        Profile::Debug => ("debug", ""),
        Profile::Release => ("release", " --release"),
    };
    let build = root.join("target").join(folder);
    if BINARIES.iter().all(|name| build.join(name).is_file()) {
        Ok(build)
    } else {
        Err(format!(
            "Build first: cargo build -p magic-studio -p magic-cli -p backstage{flag}"
        ))
    }
}

/// Place each binary under PREFIX/share/magic-studio/bin with a PREFIX/bin link; `magic`
/// opens the app, so it is the app under its short name.
fn install_binaries(build: &Path, prefix: &Path) -> Result<PathBuf> {
    let bin = prefix.join("share/magic-studio/bin");
    let links = prefix.join("bin");
    io(bin.display(), std::fs::create_dir_all(&bin))?;
    io(links.display(), std::fs::create_dir_all(&links))?;
    for name in BINARIES {
        replace_file(&build.join(name), &bin.join(name))?;
        link(&bin.join(name), &links.join(name))?;
    }
    link(&bin.join("magic-studio"), &bin.join("magic"))?;
    link(&bin.join("magic"), &links.join("magic"))?;
    Ok(bin)
}

fn install(prefix: Option<PathBuf>, profile: Profile) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = io("repository", root.canonicalize())?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set")?;
    let prefix = prefix.unwrap_or_else(|| home.join(".local"));
    let build = built(&root, profile)?;
    install_resources(&root, &prefix, &build.join("magic-studio"))?;
    let removed = remove_retired(&prefix, &prefix.join("share/magic-studio/bin"))?;
    install_binaries(&build, &prefix)?;
    install_desktop_entry(&root, &prefix)?;
    println!(
        "Installed Magic app, magic-cli, Backstage and desktop entry under {}",
        prefix.display()
    );
    for path in removed {
        println!("Removed retired {}", path.display());
    }
    Ok(())
}

fn main() {
    let result = match Cli::parse().task {
        Task::Install { prefix, profile } => install(prefix, profile),
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
    fn runtime_resources_exclude_tests_and_unlisted_files() {
        assert!(wanted_resource("pipeline/templates/words-kinetic.html"));
        assert!(wanted_resource("config.yaml"));
        assert!(!wanted_resource("pipeline/tests/test_layout.py"));
        assert!(!wanted_resource("lib/test_helpers.py"));
        assert!(!wanted_resource("apps/magic-cli/src/main.rs"));
    }
}
