//! `cargo xtask ci`: TOML, fmt, layers, Clippy (`-D warnings`) and the UI build
//! (`tsc && vite build`). Every stage prints its count and findings to the terminal and
//! nothing is written to a report file: the person or agent who made the change reads the
//! output before saying it passed.
use std::process::Command;
use std::time::Instant;

use crate::Result;

/// Workspace crates gated by `cargo xtask ci`. [`check_sets`] fails when a crate is in no group.
pub(crate) const SETS: &[(&str, &[&str])] = &[("tauri", &["craft-scribbles", "magic-engine", "magic-mcp", "magic-cli", "magic", "magic-tui", "xtask"])];

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
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    eprintln!("build groups: {} crates in exactly one group", md["packages"].as_array().map_or(0, Vec::len));
    Ok(())
}

/// Print the tool's command and, on failure, its full output. Success is silent.
fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("$ {cmd:?}");
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    print!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    Err(format!("failed ({}): {cmd:?}", output.status))
}

/// Run one gate and print how long it took.
fn stage(name: &str, gate: impl FnOnce() -> Result<()>) -> Result<()> {
    let started = Instant::now();
    let outcome = gate();
    eprintln!("== {name}: {} ({:.1}s)", if outcome.is_ok() { "ok" } else { "FAILED" }, started.elapsed().as_secs_f32());
    outcome
}

/// Run Clippy as JSON and print every finding with its file and line, plus a count even when
/// there are none.
fn clippy() -> Result<()> {
    let cargo = env!("CARGO");
    let mut warnings = 0usize;
    let mut errors = 0usize;
    for (_, set) in SETS {
        let output = Command::new(cargo)
            .arg("clippy")
            .args(packages(set))
            .args(["--lib", "--bins", "--message-format=json", "--", "-D", "warnings"])
            .output()
            .map_err(|e| e.to_string())?;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            if value["reason"].as_str() != Some("compiler-message") {
                continue;
            }
            let level = value["message"]["level"].as_str().unwrap_or_default();
            if level != "warning" && level != "error" {
                continue;
            }
            let message = value["message"]["message"].as_str().unwrap_or_default();
            let span = value["message"]["spans"].as_array().and_then(|spans| spans.first());
            let file = span.and_then(|span| span["file_name"].as_str()).unwrap_or_default();
            let line_no = span.and_then(|span| span["line_start"].as_u64()).unwrap_or(0);
            eprintln!("{level}: {file}:{line_no}: {message}");
            if level == "warning" {
                warnings += 1;
            } else {
                errors += 1;
            }
        }
        if !output.status.success() && warnings == 0 && errors == 0 {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            return Err(format!("clippy failed: {}", output.status));
        }
    }
    eprintln!("clippy: {warnings} warning(s), {errors} error(s)");
    if warnings == 0 && errors == 0 { Ok(()) } else { Err(format!("clippy: {warnings} warning(s), {errors} error(s)")) }
}

/// `tsc && vite build` for the desktop UI.
fn ui() -> Result<()> {
    let mut cmd = Command::new("bun");
    cmd.arg("run").arg("build").current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/magic"));
    run(&mut cmd)?;
    eprintln!("ui: 0 TypeScript error(s), build ok");
    Ok(())
}

/// Cheapest gates first, so a mistake fails before the expensive builds start.
pub(crate) fn ci() -> Result<()> {
    stage("build groups", check_sets)?;
    stage("toml", || {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        run(Command::new("python3").arg("scripts/check-toml.py").current_dir(root))?;
        eprintln!("toml: 0 duplicate key(s)");
        Ok(())
    })?;
    stage("fmt", || {
        run(Command::new(env!("CARGO")).args(["fmt", "--check"]))?;
        eprintln!("fmt: 0 file(s) need formatting");
        Ok(())
    })?;
    stage("layers", crate::policy::layers)?;
    // Tauri's `generate_context!` reads `apps/magic/dist` while Clippy compiles the desktop
    // binary. Build the frontend first so a clean checkout does not fail on a missing dist.
    stage("ui", ui)?;
    stage("clippy", clippy)?;
    Ok(())
}
