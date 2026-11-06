//! `cargo xtask ci`: run the Craft native quality gates.
//! `cargo xtask install`: install this built checkout's Magic Studio app, `magic-cli`,
//! runtime resources and desktop entry for the current user. It installs Magic
//! only; it never installs or configures other Craft apps. Build first:
//!   cargo build -p magic-gui -p magic-cli [--release]
use clap::{Parser, Subcommand, ValueEnum};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

mod ci;
mod doctor;
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
    /// Report which workspace apps are configured, which are not, and where the app breaks.
    Doctor,
    /// Check the workspace against FilmCraft's declared dependency layers.
    Layers,
    /// Check repository asset attribution using FilmCraft's requirements.
    Assets,
    /// Install the built app, CLI, resources and desktop entry under PREFIX.
    Install {
        /// Install prefix (binaries under PREFIX/share/magic/bin, links in PREFIX/bin).
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

/// Names earlier installs left behind: the old app name and the retired Python CLI.
const RETIRED: &[&str] = &["magic-shell", "python-magic-cli"];
const BINARIES: &[&str] = &["magic", "magic-cli"];
/// Tracked runtime resources the app reads at run time.
const RESOURCE_PREFIXES: &[&str] = &["pipeline/", ".agents/skills/", "apps/magic-cli/ui/"];
const RESOURCE_FILES: &[&str] = &[
    "apps/magic/agent.md",
    "apps/magic/MAGIC.md",
    "ATTRIBUTION.md",
    "LICENSE",
];
const LYRICS_BUNDLE: &[&str] = &[
    "apps/magic-cli/ui/lyrics-built/app.js",
    "apps/magic-cli/ui/lyrics-built/app.css",
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

struct Resource {
    name: String,
    data: Vec<u8>,
    permissions: std::fs::Permissions,
}

fn load_resources(root: &Path) -> Result<Vec<Resource>> {
    let tracked = git(root, &["ls-files", "-z"])?;
    let include_lyrics = LYRICS_BUNDLE.iter().any(|name| root.join(name).exists());
    tracked
        .split('\0')
        .filter(|n| wanted_resource(n) && !RESOURCE_FILES.contains(n))
        .chain(RESOURCE_FILES.iter().copied())
        .chain(LYRICS_BUNDLE.iter().copied().filter(|_| include_lyrics))
        .map(|name| {
            let source = root.join(name);
            let metadata = io(source.display(), std::fs::symlink_metadata(&source))?;
            if !metadata.is_file() {
                return Err(format!(
                    "{}: expected a regular runtime file",
                    source.display()
                ));
            }
            Ok(Resource {
                name: name.to_owned(),
                data: io(source.display(), std::fs::read(&source))?,
                permissions: metadata.permissions(),
            })
        })
        .collect()
}

fn installation_record(root: &Path, app: &Path) -> Result<String> {
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
    serde_json::to_string_pretty(&record).map_err(|e| e.to_string())
}

fn install_resources(prefix: &Path, resources: Vec<Resource>, record: &str) -> Result<()> {
    let runtime = prefix.join("share/magic/runtime");
    for subdir in RESOURCE_PREFIXES {
        let dir = runtime.join(subdir.trim_end_matches('/'));
        if dir.is_dir() {
            io(dir.display(), std::fs::remove_dir_all(&dir))?;
        }
    }
    for resource in resources {
        let target = runtime.join(resource.name);
        io(
            target.display(),
            std::fs::create_dir_all(target.parent().expect("file has a folder")),
        )?;
        io(target.display(), std::fs::write(&target, resource.data))?;
        io(
            target.display(),
            std::fs::set_permissions(&target, resource.permissions),
        )?;
    }
    io(
        "installation.json",
        std::fs::write(runtime.join("installation.json"), record),
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
        // Our own folder holds only what earlier installs put there.
        let owned = bin.join(name);
        if owned.is_symlink() || owned.is_file() {
            io(owned.display(), std::fs::remove_file(&owned))?;
            removed.push(owned);
        }
        // PREFIX/bin is shared: remove only a link, never someone else's program.
        let link = prefix.join("bin").join(name);
        if link.is_symlink() {
            io(link.display(), std::fs::remove_file(&link))?;
            removed.push(link);
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
    let venv = prefix.join("share/magic/.venv");
    if venv.is_dir() {
        io(venv.display(), std::fs::remove_dir_all(&venv))?;
        removed.push(venv);
    }
    Ok(removed)
}

fn install_desktop_entry(prefix: &Path, icon_data: &[u8], template: &str) -> Result<()> {
    let icon = prefix.join("share/icons/hicolor/256x256/apps/ai.storyteller.magic.png");
    io(
        icon.display(),
        std::fs::create_dir_all(icon.parent().expect("icon folder")),
    )?;
    io(icon.display(), std::fs::write(&icon, icon_data))?;
    let launcher = prefix.join("bin/magic");
    let entry = template
        .replace("Name=Magic Studio", "Name=Magic ✨")
        .replace(
            "Exec=magic %f",
            &format!("Exec=\"{}\" %f", launcher.display()),
        );
    let desktop = prefix.join("share/applications/ai.storyteller.magic.desktop");
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
    if build.join("magic").is_file() && build.join("magic-cli").is_file() {
        Ok(build)
    } else {
        Err(format!(
            "Build first: cargo build -p magic-gui -p magic-cli{flag}"
        ))
    }
}

/// Place each binary under PREFIX/share/magic/bin before publishing either launcher.
fn install_binaries(build: &Path, prefix: &Path) -> Result<PathBuf> {
    let bin = prefix.join("share/magic/bin");
    io(bin.display(), std::fs::create_dir_all(&bin))?;
    for name in BINARIES {
        replace_file(&build.join(name), &bin.join(name))?;
    }
    Ok(bin)
}

fn install(prefix: Option<PathBuf>, profile: Profile) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let root = io("repository", root.canonicalize())?;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is not set")?;
    let prefix = prefix.unwrap_or_else(|| home.join(".local"));
    let prefix = io(prefix.display(), std::path::absolute(&prefix))?;
    let build = built(&root, profile)?;
    let resources = load_resources(&root)?;
    let record = installation_record(&root, &build.join("magic"))?;
    let icon = root.join("apps/magic/icons/icon.png");
    let icon = io(icon.display(), std::fs::read(&icon))?;
    let template = root.join("apps/magic/magic.desktop");
    let template = io(template.display(), std::fs::read_to_string(&template))?;
    let links = prefix.join("bin");
    for name in BINARIES {
        let source = build.join(name);
        io(source.display(), std::fs::File::open(&source))?;
        let target = links.join(name);
        if target.exists() && !target.is_symlink() {
            return Err(format!(
                "Refusing to replace an unrelated executable: {}",
                target.display()
            ));
        }
    }

    install_resources(&prefix, resources, &record)?;
    let removed = remove_retired(&prefix, &prefix.join("share/magic/bin"))?;
    let bin = install_binaries(&build, &prefix)?;
    install_desktop_entry(&prefix, &icon, &template)?;
    io(links.display(), std::fs::create_dir_all(&links))?;
    for name in BINARIES {
        link(&bin.join(name), &links.join(name))?;
    }
    println!(
        "Installed Magic app, magic-cli and desktop entry under {}",
        prefix.display()
    );
    for path in removed {
        println!("Removed retired {}", path.display());
    }
    Ok(())
}

fn main() {
    let result = match Cli::parse().task {
        Task::Ci => ci::ci(),
        Task::Doctor => doctor::doctor(),
        Task::Layers => policy::layers(),
        Task::Assets => policy::assets(),
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
