//! `cargo xtask doctor`: report which workspace apps are configured, which are not,
//! and where the app breaks. Outputs a structured report for CI and pre-commit to consume.

use std::path::PathBuf;
use std::process::Command;

use crate::Result;

/// Patterns that indicate stale references to deleted source files.
const DELETED_FILE_PATTERNS: &[&str] = &[
    "apps/magic-cli/src/mcp.rs",
    "apps/magic-cli/src/doctor.rs",
    "apps/backstage/",
];

/// Patterns that indicate inline MCP hacks that should go through the registry.
const MCP_HACK_PATTERNS: &[&str] = &["control_call", "invoke_menu", "ui_control", "app_quit"];

/// Path strings that should use `magic` instead of `magic-studio`.
const STALE_PATH_PATTERNS: &[&str] = &["magic-studio"];

/// Run the doctor report: inventory workspace packages, check configuration,
/// run cargo check, scan for stale references, and print a structured report.
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

    // 4. Scan source for stale references (deleted files, MCP hacks, old paths).
    let stale_references = scan_stale_references()?;

    // 5. Print structured report.
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
    println!("Stale reference scan:");
    if stale_references.is_empty() {
        println!("  clean");
    } else {
        println!("  FOUND {} stale reference(s):", stale_references.len());
        for (file, line_no, pattern, line) in &stale_references {
            println!(
                "    {}:{}: pattern `{}` — {}",
                file.display(),
                line_no,
                pattern,
                line.trim()
            );
        }
    }

    println!();
    let total_issues = unconfigured.len() + broken_packages.len() + stale_references.len();
    if total_issues == 0 {
        println!("doctor: no issues found");
        Ok(())
    } else {
        println!("doctor: {total_issues} issue(s) found");
        Err(format!("doctor: {total_issues} issue(s) found").into())
    }
}

/// Scan tracked source files for stale references to deleted files, MCP hacks,
/// and old `magic-studio` path strings.
fn scan_stale_references() -> Result<Vec<(PathBuf, usize, String, String)>> {
    let root = std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(PathBuf::from)
        .map(|p| p.join(".."))
        .and_then(|p| p.canonicalize().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let git_files = Command::new("git")
        .args(["ls-files", "--cached"])
        .current_dir(&root)
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !git_files.status.success() {
        return Err(String::from_utf8_lossy(&git_files.stderr).into());
    }

    let mut findings = Vec::new();
    for line in String::from_utf8_lossy(&git_files.stdout).lines() {
        let path = root.join(line);
        if !path.is_file() {
            continue;
        }
        // Skip this file itself (its constants match the patterns it scans for).
        if path.ends_with("xtask/src/doctor.rs") {
            continue;
        }
        // Skip skill documentation — those describe other Craft apps' MCP interfaces,
        // not Magic's stale code.
        if path.starts_with(root.join(".agents/skills")) {
            continue;
        }
        // Only scan source/config files.
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "toml" | "md" | "yml" | "yaml" | "json") {
            continue;
        }
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (line_no, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments and URLs.
            if trimmed.starts_with("//")
                || trimmed.contains("http://")
                || trimmed.contains("https://")
            {
                continue;
            }
            for pattern in DELETED_FILE_PATTERNS {
                if line.contains(pattern) {
                    findings.push((
                        path.clone(),
                        line_no + 1,
                        pattern.to_string(),
                        line.to_string(),
                    ));
                }
            }
            for pattern in MCP_HACK_PATTERNS {
                if line_contains_mcp_call(trimmed, pattern) {
                    findings.push((
                        path.clone(),
                        line_no + 1,
                        pattern.to_string(),
                        line.to_string(),
                    ));
                }
            }
            for pattern in STALE_PATH_PATTERNS {
                if line.contains(pattern) && !line.contains("magicstudio") {
                    findings.push((
                        path.clone(),
                        line_no + 1,
                        pattern.to_string(),
                        line.to_string(),
                    ));
                }
            }
        }
    }

    findings.sort();
    findings.dedup();
    Ok(findings)
}

/// Check whether an MCP pattern appears as an actual call invocation
/// (e.g. `control_call {`, `"ui_control"`, `method":"app_quit"`), not as a
/// substring of a larger identifier like `specs_ui_controls`.
fn line_contains_mcp_call(line: &str, pattern: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = line[start..].find(pattern) {
        let abs = start + pos;
        let end = abs + pattern.len();
        // Word boundary before: not alphanumeric/underscore, or start of line.
        let before_ok = abs == 0
            || !line[abs - 1..abs]
                .chars()
                .next()
                .map_or(true, |c| c.is_alphanumeric() || c == '_');
        // Word boundary after: not alphanumeric/underscore, or end of line.
        let after_ok = end >= line.len()
            || !line[end..end + 1]
                .chars()
                .next()
                .map_or(true, |c| c.is_alphanumeric() || c == '_');
        if before_ok && after_ok {
            return true;
        }
        start = end;
    }
    false
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
