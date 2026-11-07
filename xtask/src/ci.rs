//! `cargo xtask ci`: fmt, layers, Clippy, tests, doctests and the control-protocol conformance test. Assets run with `CHECK_ASSETS` set.
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

fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("$ {cmd:?}");
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("failed ({status}): {cmd:?}")) }
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
        run(Command::new(cargo).arg("clippy").args(packages(set)).args(["--all-targets", "--", "-D", "warnings"]))?;
    }
    Ok(())
}

/// Lib, bin and integration tests, then doctests, which nextest does not run.
fn tests() -> Result<()> {
    let cargo = env!("CARGO");
    let targets = ["--lib", "--bins", "--tests", "--no-fail-fast"];
    let nextest = Command::new(cargo).args(["nextest", "--version"]).output().is_ok_and(|o| o.status.success());
    if !nextest {
        println!("cargo-nextest not found, running cargo test");
    }
    for (_, set) in SETS {
        if nextest {
            run(Command::new(cargo).args(["nextest", "run"]).args(packages(set)).args(targets))?;
        } else {
            run(Command::new(cargo).arg("test").args(packages(set)).args(targets))?;
        }
        run(Command::new(cargo).arg("test").args(packages(set)).arg("--doc"))?;
    }
    Ok(())
}

/// Cheapest gates first, so a mistake fails before the expensive builds start.
pub(crate) fn ci() -> Result<()> {
    let cargo = env!("CARGO");
    stage("build groups", check_sets)?;
    stage("fmt", || run(Command::new(cargo).args(["fmt", "--check"])))?;
    stage("layers", crate::policy::layers)?;
    stage("clippy", clippy)?;
    stage("tests", tests)?;
    // The Tauri shell's control channel: transport tests, and the documented method table against the
    // shell's dispatch and the frontend's request switch.
    stage("control protocol", || run(Command::new(cargo).args(["test", "-p", "magic", "--bins", "control"])))?;
    if std::env::var("CHECK_ASSETS").is_ok() {
        stage("assets", crate::policy::assets)?;
    }
    Ok(())
}
