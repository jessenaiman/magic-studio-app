//! `cargo xtask doctor`: report which workspace apps are configured, which are not,
//! and where the app breaks. Outputs a structured report for CI and pre-commit to consume.

use std::process::Command;

use crate::Result;

/// Run the doctor report: inventory workspace packages, check configuration,
/// run cargo check, and print a structured report.
pub(crate) fn doctor() -> Result<()> {
    let cargo = env!("CARGO");

    // 1. Inventory workspace packages via cargo metadata.
    let metadata = cargo_metadata(&cargo)?;
    let packages: Vec<(String, Vec<String>)> = workspace_packages(&metadata)?;

    // 2. Check each app package is configured.
    let mut configured = Vec::new();
    let mut unconfigured = Vec::new();
    for (name, deps) in &packages {
        if is_app(name) {
            if is_configured(name) {
                configured.push(name.clone());
            } else {
                unconfigured.push(format!("{name} (missing source or Cargo.toml)"));
            }
        }
        _ = deps;
    }

    // 3. Run cargo check and capture per-package breakage.
    let check = run_cargo_check(&cargo)?;
    let broken_packages = parse_broken_packages(&check);

    // 4. Print structured report.
    println!("=== Magic doctor report ===");
    println!();

    println!("Configured apps:");
    if configured.is_empty() {
        println!("  (none)");
    } else {
        for name in &configured {
            let broken = if broken_packages.contains(name) {
                " BROKEN"
            } else {
                " ok"
            };
            println!("  - {name}{broken}");
        }
    }

    println!();
    println!("Unconfigured apps:");
    if unconfigured.is_empty() {
        println!("  (none)");
    } else {
        for name in &unconfigured {
            println!("  - {name}");
        }
    }

    println!();
    println!("Build breakage:");
    if check.status && broken_packages.is_empty() {
        println!("  cargo check --workspace: ok");
    } else {
        println!("  cargo check --workspace: FAILED");
        if broken_packages.is_empty() {
            println!("  (no per-package breakdown available)");
        }
    }

    println!();
    let total_issues = unconfigured.len() + broken_packages.len();
    if total_issues == 0 {
        println!("doctor: no issues found");
        Ok(())
    } else {
        println!("doctor: {total_issues} issue(s) found");
        Err(format!("doctor: {total_issues} issue(s) found").into())
    }
}

/// Parse cargo metadata JSON and return (package_name, dependency_names) pairs.
fn cargo_metadata(cargo: &str) -> Result<serde_json::Value> {
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn workspace_packages(md: &serde_json::Value) -> Result<Vec<(String, Vec<String>)>> {
    let mut result = Vec::new();
    let arr = md["packages"]
        .as_array()
        .ok_or_else(|| "cargo metadata: no packages array".to_string())?;
    for p in arr {
        let name = p["name"]
            .as_str()
            .ok_or_else(|| "cargo metadata: package without name".to_string())?
            .to_string();
        let deps: Vec<String> = p["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["kind"].is_null() || d["kind"] == "build")
            .filter_map(|d| d["name"].as_str().map(str::to_string))
            .collect();
        result.push((name, deps));
    }
    Ok(result)
}

/// Heuristic: a package is an "app" if it's a binary target or the name
/// suggests a standalone app (magic, magic-cli, etc.).
fn is_app(name: &str) -> bool {
    matches!(name, "magic" | "magic-cli" | "magic-engine" | "xtask")
}

/// Check whether an app package appears configured: its manifest directory
/// contains a Cargo.toml and expected source files.
fn is_configured(name: &str) -> bool {
    let manifest_dir = match manifest_dir_for(name) {
        Some(d) => d,
        None => return false,
    };
    let cargo_toml = manifest_dir.join("Cargo.toml");
    if !cargo_toml.is_file() {
        return false;
    }
    // Check for source files.
    let src = manifest_dir.join("src");
    if src.is_dir() {
        let has_rs = match std::fs::read_dir(&src) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().map_or(false, |ext| ext == "rs")),
            Err(_) => false,
        };
        if !has_rs {
            return false;
        }
    } else {
        // No src directory: check for a single main.rs or lib.rs at package root
        if !manifest_dir.join("main.rs").is_file() && !manifest_dir.join("lib.rs").is_file() {
            return false;
        }
    }
    true
}

/// Guess the manifest directory for a package from its name.
fn manifest_dir_for(name: &str) -> Option<std::path::PathBuf> {
    let root = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let root = std::path::Path::new(&root);
    match name {
        "magic" => Some(root.join("../apps/magic")),
        "magic-cli" => Some(root.join("../apps/magic-cli")),
        "magic-engine" => Some(root.join("../crates/engine")),
        "xtask" => Some(root.join(".")),
        _ => None,
    }
}

/// Run `cargo check --workspace` and capture output.
struct CheckResult {
    status: bool,
    stdout: String,
    stderr: String,
}

fn run_cargo_check(cargo: &str) -> Result<CheckResult> {
    let out = Command::new(cargo)
        .args(["check", "--workspace", "--all-targets"])
        .output()
        .map_err(|e| format!("cargo check: {e}"))?;
    Ok(CheckResult {
        status: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

/// Extract package names that have errors in cargo check output.
fn parse_broken_packages(check: &CheckResult) -> Vec<String> {
    let mut broken = Vec::new();
    // Pattern: "error: could not compile \`package-name\`"
    for line in check.stderr.lines().chain(check.stdout.lines()) {
        if let Some(rest) = line.strip_prefix("error: could not compile `") {
            let name = rest.split('`').next().unwrap_or("").to_string();
            if !name.is_empty() && !broken.contains(&name) {
                broken.push(name);
            }
        }
    }
    broken
}
