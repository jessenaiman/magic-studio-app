//! `cargo xtask ci`: fmt, layers, Clippy, tests, doctests and the control-protocol conformance test. Assets run with `CHECK_ASSETS` set.
//! `cargo xtask verify`: every gate `ci` runs plus the UI build and asset policy, collecting **all** problems
//! instead of stopping at the first, into `target/verify/report.jsonl` (one JSON record per failing gate) —
//! the terminal shows the same failures tersely; an agent cannot claim green without the report.
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

use crate::Result;

/// One failing gate: what ran and what the tools actually said.
pub(crate) struct Problem {
    gate: String,
    command: String,
    errors: Vec<String>,
    tail: String,
}

static PROBLEMS: Mutex<Vec<Problem>> = Mutex::new(Vec::new());
static CURRENT_GATE: Mutex<String> = Mutex::new(String::new());

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

/// Run a child process quietly on success; on failure echo its full output and record a problem.
fn run(cmd: &mut Command) -> Result<()> {
    eprintln!("$ {cmd:?}");
    let output = cmd.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let combined = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    print!("{combined}");
    let errors = combined.lines().filter(|l| l.contains("error") || l.contains("FAILED") || l.starts_with("failures:")).take(20).map(str::to_owned).collect();
    let tail = combined.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
    if let (Ok(mut problems), Ok(gate)) = (PROBLEMS.lock(), CURRENT_GATE.lock()) {
        problems.push(Problem { gate: gate.clone(), command: format!("{cmd:?}"), errors, tail });
    }
    Err(format!("failed ({}): {cmd:?}", output.status))
}

/// Run one gate and report how long it took, so a slow or failing gate is visible at a glance.
fn stage(name: &str, gate: impl FnOnce() -> Result<()>) -> Result<()> {
    if let Ok(mut current) = CURRENT_GATE.lock() {
        *current = name.to_owned();
    }
    let started = Instant::now();
    let outcome = gate();
    let mark = if outcome.is_ok() { "ok" } else { "FAILED" };
    eprintln!("== {name}: {mark} ({:.1}s)", started.elapsed().as_secs_f32());
    // A gate that failed without going through `run()` still owes the report a record,
    // so problem records and failed gates always agree.
    if let Err(error) = &outcome
        && let Ok(mut problems) = PROBLEMS.lock()
        && !problems.iter().any(|p| p.gate == name)
    {
        problems.push(Problem { gate: name.to_owned(), command: name.to_owned(), errors: vec![error.clone()], tail: error.clone() });
    }
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

/// `tsc && vite build` for the desktop UI — the gate whose absence lets TS breakage land.
fn ui() -> Result<()> {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/magic");
    let mut cmd = Command::new("bun");
    cmd.arg("run").arg("build").current_dir(app);
    run(&mut cmd)
}

fn write_report() -> Result<()> {
    let problems = PROBLEMS.lock().map_err(|e| e.to_string())?;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/verify");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("report.jsonl");
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    use std::io::Write as _;
    for problem in problems.iter() {
        let record = serde_json::json!({"gate": problem.gate, "command": problem.command, "errors": problem.errors, "tail": problem.tail});
        writeln!(file, "{record}").map_err(|e| e.to_string())?;
    }
    writeln!(file, "{}", serde_json::json!({"problems": problems.len()})).map_err(|e| e.to_string())?;
    eprintln!("verify: {} problem record(s) -> {}", problems.len(), path.display());
    Ok(())
}

/// Every gate, no fail-fast: one run tells the human and any agent **all** the places the system
/// is broken, in the terminal and machine-readably in `target/verify/report.jsonl`.
pub(crate) fn verify() -> Result<()> {
    let cargo = env!("CARGO");
    let results = [
        stage("build groups", check_sets),
        stage("fmt", || run(Command::new(cargo).args(["fmt", "--check"]))),
        stage("layers", crate::policy::layers),
        stage("clippy", clippy),
        stage("tests", tests),
        stage("control protocol", || run(Command::new(cargo).args(["test", "-p", "magic", "--bins", "control"]))),
        stage("assets", crate::policy::assets),
        stage("ui", ui),
    ];
    let failed = results.iter().filter(|r| r.is_err()).count();
    write_report()?;
    if failed > 0 { Err(format!("{failed} gate(s) failed; details in target/verify/report.jsonl")) } else { Ok(()) }
}
