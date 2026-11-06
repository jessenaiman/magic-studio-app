//! `cargo xtask ci`: fmt, Clippy, tests, doctests, control-protocol conformance and layers. Assets run with `CHECK_ASSETS` set.
use std::process::Command;
use std::time::Instant;

use crate::Result;

fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("$ {cmd:?}");
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("failed ({status}): {cmd:?}"))
    }
}

/// Run one gate and report how long it took, so a slow or failing gate is visible at a glance.
fn stage(name: &str, gate: impl FnOnce() -> Result<()>) -> Result<()> {
    let started = Instant::now();
    let outcome = gate();
    let mark = if outcome.is_ok() { "ok" } else { "FAILED" };
    eprintln!(
        "== {name}: {mark} ({:.1}s)",
        started.elapsed().as_secs_f32()
    );
    outcome
}

/// Lib, bin and integration tests, then doctests, which nextest does not run.
fn tests() -> Result<()> {
    let cargo = env!("CARGO");
    let targets = [
        "--workspace",
        "--lib",
        "--bins",
        "--tests",
        "--no-fail-fast",
    ];
    let nextest = Command::new(cargo)
        .args(["nextest", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if nextest {
        run(Command::new(cargo).args(["nextest", "run"]).args(targets))?;
    } else {
        println!("cargo-nextest not found, running cargo test");
        run(Command::new(cargo).arg("test").args(targets))?;
    }
    run(Command::new(cargo).args(["test", "--workspace", "--doc"]))
}

/// Cheapest gates first, so a mistake fails before the expensive builds start.
pub(crate) fn ci() -> Result<()> {
    let cargo = env!("CARGO");
    stage("fmt", || run(Command::new(cargo).args(["fmt", "--check"])))?;
    stage("layers", crate::policy::layers)?;
    stage("clippy", || {
        run(Command::new(cargo).args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]))
    })?;
    stage("tests", tests)?;
    stage("control protocol", || {
        run(Command::new(cargo).args([
            "test",
            "-p",
            "magic-ui-egui",
            "--lib",
            "control_conformance",
            "--",
            "--ignored",
            "--nocapture",
        ]))
    })?;
    if std::env::var("CHECK_ASSETS").is_ok() {
        stage("assets", crate::policy::assets)?;
    }
    Ok(())
}
