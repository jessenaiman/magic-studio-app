//! `cargo xtask ci`: fmt, layers, Clippy (`-D warnings`) and the UI build (`tsc && vite build`).
//! A failing gate prints the tool's full output and appends one JSON line to
//! `target/ci-report.jsonl` — the tools own the measurements; this only carries their answers.
use std::process::Command;
use std::time::Instant;

use crate::Result;

/// Workspace crates gated by `cargo xtask ci`. [`check_sets`] fails when a crate is in no group.
pub(crate) const SETS: &[(&str, &[&str])] = &[("tauri", &["magic-engine", "magic-mcp", "magic-cli", "magic", "magic-tui", "xtask"])];

/// `-p` arguments for one group.
pub(crate) fn packages(set: &[&str]) -> Vec<String> {
    set.iter().flat_map(|name| ["-p".to_owned(), (*name).to_owned()]).collect()
}

/// Every workspace crate belongs to exactly one group, so no crate escapes the gates.
fn check_sets() -> Result<()> {
    let md = crate::policy::metadata()?;
    let grouped: Vec<&str> = SETS.iter().flat_map(|(_, set)| set.iter().copied()).collect();
    let mut errors = Vec::new();
    for package in md["packages"].as_array().into_iter().flatten() {
        let name = package["name"].as_str().unwrap_or_default();
        match grouped.iter().filter(|g| **g == name).count() {
            1 => {}
            0 => errors.push(format!("{name}: in no build group (add it to xtask `SETS`)")),
            _ => errors.push(format!("{name}: in more than one build group")),
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

fn report_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/ci-report.jsonl")
}

/// Quiet on success. On failure the tool's full output reaches the terminal and one JSON line
/// (command + tail) reaches `target/ci-report.jsonl`, so a person and an agent see the same facts.
fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("$ {cmd:?}");
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    print!("{text}");
    let tail = text.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    let path = report_path();
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write as _;
        let _ = writeln!(file, "{}", serde_json::json!({"command": format!("{cmd:?}"), "tail": tail}));
    }
    Err(format!("failed ({}): {cmd:?}", output.status))
}

/// Run one gate and report how long it took, so a slow or failing gate is visible at a glance.
fn stage(name: &str, gate: impl FnOnce() -> Result<()>) -> Result<()> {
    let started = Instant::now();
    let outcome = gate();
    let mark = if outcome.is_ok() { "ok" } else { "FAILED" };
    eprintln!("== {name}: {mark} ({:.1}s)", started.elapsed().as_secs_f32());
    outcome
}

fn clippy() -> Result<()> {
    let cargo = env!("CARGO");
    for (_, set) in SETS {
        run(Command::new(cargo).arg("clippy").args(packages(set)).args(["--lib", "--bins", "--", "-D", "warnings"]))?;
    }
    Ok(())
}

/// `tsc && vite build` for the desktop UI — the gate whose absence lets TS breakage land.
fn ui() -> Result<()> {
    let mut cmd = Command::new("bun");
    cmd.arg("run").arg("build").current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/magic"));
    run(&mut cmd)
}

/// Cheapest gates first, so a mistake fails before the expensive builds start.
pub(crate) fn ci() -> Result<()> {
    let cargo = env!("CARGO");
    let path = report_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::File::create(path).map_err(|e| e.to_string())?;
    stage("build groups", check_sets)?;
    stage("fmt", || run(Command::new(cargo).args(["fmt", "--check"])))?;
    stage("layers", crate::policy::layers)?;
    stage("clippy", clippy)?;
    stage("ui", ui)?;
    Ok(())
}
