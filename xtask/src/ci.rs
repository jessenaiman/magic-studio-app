//! `cargo xtask ci`: fmt, Clippy, tests, doctests, control-protocol conformance and layers. Assets run with `CHECK_ASSETS` set.
use std::process::Command;

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

pub(crate) fn ci() -> Result<()> {
    let cargo = env!("CARGO");
    run(Command::new(cargo).args(["fmt", "--check"]))?;
    run(Command::new(cargo).args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]))?;
    tests()?;
    run(Command::new(cargo).args([
        "test",
        "-p",
        "magic-ui-egui",
        "--lib",
        "control_conformance",
        "--",
        "--ignored",
        "--nocapture",
    ]))?;
    crate::policy::layers()?;
    if std::env::var("CHECK_ASSETS").is_ok() {
        crate::policy::assets()?;
    }
    Ok(())
}
