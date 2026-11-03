//! Native repository checks, reported with unmodified tool output. Each tool is a command
//! that already exists, with the repository configuration it expects.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Tool {
    pub name: &'static str,
    label: &'static str,
    command: &'static [&'static str],
    /// Exact file names, or a prefix ending in `*`.
    configs: &'static [&'static str],
}

/// Default order for a directory.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "clippy",
        label: "cargo clippy",
        command: &[
            "cargo",
            "clippy",
            "--workspace",
            "--lib",
            "--bins",
            "--locked",
            "--offline",
            "--",
            "-D",
            "warnings",
        ],
        configs: &["Cargo.toml"],
    },
    Tool {
        name: "tsc",
        label: "tsc",
        command: &["tsc", "-p", ".", "--pretty", "false"],
        configs: &["tsconfig.json"],
    },
    Tool {
        name: "eslint",
        label: "ESLint",
        command: &["eslint", "."],
        configs: &["eslint.config.js", "eslint.config.mjs", "eslint.config.cjs"],
    },
    Tool {
        name: "stylelint",
        label: "Stylelint",
        command: &["stylelint", "**/*.css"],
        configs: &["stylelint.config.*", ".stylelintrc*"],
    },
    Tool {
        name: "lizard",
        label: "Lizard",
        command: &["lizard", "."],
        configs: &["code-limits.json"],
    },
    Tool {
        name: "wallace",
        label: "Wallace",
        command: &["wallace"],
        configs: &[],
    },
    Tool {
        name: "jscpd",
        label: "jscpd",
        command: &["jscpd", "."],
        configs: &[".jscpd.json"],
    },
    Tool {
        name: "purgecss",
        label: "PurgeCSS",
        command: &["purgecss", "-c", "purgecss.config.cjs"],
        configs: &["purgecss.config.cjs"],
    },
    Tool {
        name: "scope",
        label: "git scope",
        command: &["git", "diff", "--stat=160"],
        configs: &[],
    },
];

/// Third-party stylesheets are measured by their own projects, not ours.
const FOREIGN: &[&str] = &["vendor", "node_modules", "target"];
/// A build emits `.js` beside the `.ts` it came from; measure the source.
const GENERATED_UI: &str = "apps/src-tauri/ui";

fn tool(name: &str) -> &'static Tool {
    TOOLS
        .iter()
        .find(|t| t.name == name)
        .expect("tool names come from TOOLS")
}

fn matches(file: &str, pattern: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => file.starts_with(prefix),
        None => file == pattern,
    }
}

fn has_config(dir: &Path, t: &Tool) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            e.path().is_file()
                && t.configs
                    .iter()
                    .any(|p| matches(&e.file_name().to_string_lossy(), p))
        })
}

/// Nearest directory up to the repository top that holds the tool's config. Clippy stays on
/// the given crate directory; a tool without configuration runs from the repository top.
fn config_root(start: &Path, t: &Tool) -> PathBuf {
    if t.name == "clippy" {
        return start
            .ancestors()
            .find(|d| d.join("Cargo.toml").is_file())
            .unwrap_or(start)
            .to_path_buf();
    }
    for dir in start.ancestors() {
        if has_config(dir, t) {
            return dir.to_path_buf();
        }
        if dir.join(".git").exists() {
            return if t.configs.is_empty() {
                dir.to_path_buf()
            } else {
                start.to_path_buf()
            };
        }
    }
    start.to_path_buf()
}

fn walk(dir: &Path, base: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let foreign = path.strip_prefix(base).is_ok_and(|rel| {
            rel.components()
                .any(|c| FOREIGN.contains(&c.as_os_str().to_string_lossy().as_ref()))
        });
        if foreign {
            continue;
        }
        if path.is_dir() {
            walk(&path, base, suffix, out);
        } else if path.extension().is_some_and(|e| e == suffix) {
            out.push(path);
        }
    }
}

/// Our stylesheets under target, in stable order, without third-party CSS.
fn css_sources(target: &Path) -> Vec<PathBuf> {
    if target.is_file() {
        return vec![target.to_path_buf()];
    }
    let mut found = Vec::new();
    walk(target, target, "css", &mut found);
    found.sort();
    found
}

fn has_ext_under(target: &Path, suffix: &str) -> bool {
    if target.is_file() {
        return target.extension().is_some_and(|e| e == suffix);
    }
    let mut found = Vec::new();
    walk(target, target, suffix, &mut found);
    !found.is_empty()
}

/// The thresholds the repository already declares in code-limits.json.
fn lizard_options(root: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(root.join("code-limits.json"))
        .map_err(|e| format!("code-limits.json: {e}"))?;
    let limits: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("code-limits.json: {e}"))?;
    if !limits.is_object() {
        return Err("code-limits.json must be an object".into());
    }
    let mut options: Vec<String> = ["-Ecognitive", "--sort", "cyclomatic_complexity"]
        .map(String::from)
        .to_vec();
    let fields = [
        ("max_function_ccn", None, "cyclomatic_complexity"),
        ("max_function_lines", Some(100), "length"),
        (
            "max_function_cognitive_complexity",
            Some(15),
            "cognitive_complexity",
        ),
    ];
    for (key, default, native) in fields {
        let limit = limits[key].as_u64().or(default).filter(|n| *n >= 1);
        let limit =
            limit.ok_or_else(|| format!("code-limits.json needs a positive integer {key}"))?;
        options.extend(["--Threshold".into(), format!("{native}={limit}")]);
    }
    Ok(options)
}

/// Skip `.js` a build emits beside its `.ts`. Lizard matches `-x` against the absolute
/// paths it walks, so the pattern is absolute too.
fn generated_excludes(root: &Path) -> Vec<String> {
    let dir = root.join(GENERATED_UI);
    let has_ts = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "ts"));
    if has_ts {
        vec!["-x".into(), dir.join("*.js").to_string_lossy().into_owned()]
    } else {
        Vec::new()
    }
}

/// A file the user may execute; a non-executable file of the same name is skipped, as
/// `shutil.which` did.
fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn which(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.is_absolute() {
        return runnable(candidate).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| runnable(p))
}

fn common_repo_root(root: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let git_dir = Path::new(&raw);
    let resolved = if git_dir.is_absolute() {
        git_dir.to_path_buf()
    } else {
        root.join(git_dir)
    };
    resolved
        .canonicalize()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

fn which_in_repo(root: &Path, name: &str) -> Option<PathBuf> {
    which(name)
        .or_else(|| which(&root.join("node_modules/.bin").join(name).to_string_lossy()))
        .or_else(|| {
            let common = common_repo_root(root)?;
            which(
                &common
                    .join("node_modules/.bin")
                    .join(name)
                    .to_string_lossy(),
            )
        })
}

/// Point the native command at target when it is not the config root.
fn with_target(mut command: Vec<String>, root: &Path, target: &Path, name: &str) -> Vec<String> {
    let last = command.len() - 1;
    match name {
        "wallace" if target.is_file() => command.push(target.to_string_lossy().into_owned()),
        "wallace" | "scope" | "purgecss" | "tsc" | "clippy" => {}
        _ if target.is_file() || name == "lizard" => {
            command[last] = target.to_string_lossy().into_owned()
        }
        "stylelint" if target != root => {
            command[last] = target.join("**/*.css").to_string_lossy().into_owned()
        }
        _ if target != root => command[last] = target.to_string_lossy().into_owned(),
        _ => {}
    }
    command
}

fn apply_worktree_eslint_config(root: &Path, command: &mut Vec<String>) {
    if root.join("node_modules").is_dir() {
        return;
    }
    let Some(common) = common_repo_root(root) else {
        return;
    };
    let config = common.join("eslint.config.mjs");
    if common.join("node_modules").is_dir() && config.is_file() {
        command.splice(1..1, ["-c".into(), config.to_string_lossy().into_owned()]);
    }
}

fn build_command(root: &Path, target: &Path, t: &Tool) -> Result<Vec<String>, String> {
    if !t.configs.is_empty() && !has_config(root, t) {
        return Err(format!(
            "missing repository configuration: {}",
            t.configs.join(", ")
        ));
    }
    let mut command = with_target(
        t.command.iter().map(|s| s.to_string()).collect(),
        root,
        target,
        t.name,
    );
    if t.name == "eslint" {
        apply_worktree_eslint_config(root, &mut command);
    }
    if t.name == "lizard" {
        let modules = target.with_extension("");
        if target.is_file() && target.extension().is_some_and(|e| e == "rs") && modules.is_dir() {
            command.push(modules.to_string_lossy().into_owned());
        }
        command.extend(lizard_options(root)?);
        command.extend(generated_excludes(root));
    }
    let found = which_in_repo(root, &command[0])
        .ok_or_else(|| format!("{} not on PATH or in root node_modules/.bin", command[0]))?;
    command[0] = found.to_string_lossy().into_owned();
    Ok(command)
}

fn shown(command: &[String]) -> String {
    command
        .iter()
        .map(|a| {
            if !a.is_empty()
                && a.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./=:@,+%".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run the tool, echo its output unchanged to the terminal, and return `(exit_code, output)`.
fn run_native(
    label: &str,
    command: &[String],
    root: &Path,
    dry_run: bool,
    stdin: Option<&[PathBuf]>,
) -> Result<(i32, String), String> {
    let mut line = shown(command);
    if let Some(files) = stdin {
        let names: Vec<String> = files
            .iter()
            .map(|f| f.to_string_lossy().into_owned())
            .collect();
        line = format!("cat {} | {line}", shown(&names));
    }
    eprintln!("[{label}] {line}");
    if dry_run {
        eprintln!("[{label}] not run (--dry-run)");
        return Ok((0, String::new()));
    }
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(root)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let (Some(files), Some(mut pipe)) = (stdin, child.stdin.take()) {
        for file in files {
            pipe.write_all(&std::fs::read(file).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            pipe.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = std::io::stdout().write_all(&out.stdout);
    let _ = std::io::stderr().write_all(&out.stderr);
    #[cfg(unix)]
    let code = {
        use std::os::unix::process::ExitStatusExt;
        out.status
            .code()
            .unwrap_or_else(|| 128 + out.status.signal().unwrap_or(0))
    };
    #[cfg(not(unix))]
    let code = out.status.code().unwrap_or(1);
    eprintln!("[{label}] exit {code}");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((code, combined))
}

fn run_tool(
    t: &Tool,
    command: &[String],
    root: &Path,
    target: &Path,
    dry_run: bool,
    base: &str,
) -> Result<(i32, String), String> {
    if t.name == "scope" {
        let mut diff = command.to_vec();
        diff.extend([base.to_owned(), "--".into()]);
        let (code, out1) = run_native(t.label, &diff, root, dry_run, None)?;
        let untracked: Vec<String> = [
            command[0].as_str(),
            "ls-files",
            "--others",
            "--exclude-standard",
        ]
        .map(String::from)
        .to_vec();
        let (other, out2) = run_native("git untracked", &untracked, root, dry_run, None)?;
        let final_code = if other != 0 { other } else { code };
        return Ok((final_code, format!("{out1}\n{out2}")));
    }
    if t.name == "wallace" && target.is_dir() {
        let found = css_sources(target);
        if found.is_empty() {
            return Err(format!("no stylesheets under {}", target.display()));
        }
        return run_native(t.label, command, root, dry_run, Some(&found));
    }
    run_native(t.label, command, root, dry_run, None)
}

fn parse_colon_line(trimmed: &str) -> Option<(u64, &str)> {
    let (loc, rest) = trimmed.split_once(char::is_whitespace)?;
    let (line_str, _col_str) = loc.split_once(':')?;
    let line = line_str.parse::<u64>().ok()?;
    Some((line, rest.trim()))
}

fn parse_lint_output(tool_id: &str, output: &str) -> Vec<serde_json::Value> {
    let mut issues = Vec::new();
    let mut current_file = String::new();
    for raw in output.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('✖') {
            continue;
        }
        let Some((line, rest)) = parse_colon_line(trimmed) else {
            if !raw.starts_with(char::is_whitespace)
                && (trimmed.contains('/') || trimmed.contains('.'))
            {
                current_file = trimmed.to_owned();
            }
            continue;
        };
        let body = rest
            .strip_prefix("error")
            .or_else(|| rest.strip_prefix("warning"))
            .or_else(|| rest.strip_prefix('✖'))
            .unwrap_or(rest)
            .trim();
        let mut parts = body.rsplitn(2, "  ");
        let rule = parts.next().unwrap_or(tool_id).trim();
        let msg = parts.next().map(str::trim).unwrap_or(body);
        issues.push(serde_json::json!({
            "toolId": tool_id,
            "patternId": rule,
            "filePath": current_file,
            "line": line,
            "severity": "Error",
            "message": msg,
        }));
    }
    issues
}

fn parse_tsc_output(output: &str) -> Vec<serde_json::Value> {
    let mut issues = Vec::new();
    for line in output.lines() {
        let Some((loc, msg)) = line.split_once("): error ") else {
            continue;
        };
        let Some((file, coords)) = loc.rsplit_once('(') else {
            continue;
        };
        let line_num = coords
            .split(',')
            .next()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1);
        let (code, text) = msg.split_once(':').unwrap_or(("TS", msg));
        issues.push(serde_json::json!({
            "toolId": "tsc",
            "patternId": code.trim(),
            "filePath": file.trim(),
            "line": line_num,
            "severity": "Error",
            "message": text.trim(),
        }));
    }
    issues
}

fn extract_tool_issues(name: &str, label: &str, output: &str) -> Vec<serde_json::Value> {
    match name {
        "eslint" | "stylelint" => parse_lint_output(label, output),
        "tsc" => parse_tsc_output(output),
        _ => Vec::new(),
    }
}

struct ToolOutcome {
    name: &'static str,
    label: &'static str,
    exit_code: i32,
    findings: usize,
    trace: String,
    issues: Vec<serde_json::Value>,
    error: Option<String>,
}

fn repo_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|d| d.join(".git").exists() || d.join("code-limits.json").is_file())
        .unwrap_or(start)
        .to_path_buf()
}

fn merge_tool_rows(root: &Path, outcomes: &[ToolOutcome]) -> Vec<serde_json::Value> {
    let existing = std::fs::read(root.join("target/analytics/audit.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .filter(|v| v["source_root"] == root.to_string_lossy().as_ref());
    let mut map = std::collections::BTreeMap::new();
    if let Some(prev) = existing.and_then(|v| v["tools"].as_array().cloned()) {
        for item in prev {
            if let Some(name) = item["tool"].as_str() {
                map.insert(name.to_owned(), item);
            }
        }
    }
    for o in outcomes {
        let status = if o.error.is_some() {
            "not_run"
        } else if o.exit_code == 0 {
            "passed"
        } else {
            "failed"
        };
        map.insert(
            o.name.to_owned(),
            serde_json::json!({
                "tool": o.name,
                "label": o.label,
                "ok": status == "passed",
                "exit_code": o.exit_code,
                "findings": o.findings,
                "issues": o.issues,
                "trace": o.trace,
                "measured_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
                "status": status,
                "note": o.error.as_deref().unwrap_or(status),
                "error": o.error,
            }),
        );
    }
    TOOLS
        .iter()
        .filter_map(|t| map.remove(t.name))
        .collect::<Vec<_>>()
}

fn build_native_report(
    root: &Path,
    tools: &[serde_json::Value],
    outcomes: &[ToolOutcome],
    comp: &serde_json::Value,
    stamp: u64,
) -> serde_json::Value {
    let files_analyzed = comp["files_analyzed"].as_u64().unwrap_or(0);
    let mut issues = Vec::new();
    for fn_row in comp["hotspots"].as_array().into_iter().flatten().take(25) {
        let ccn = fn_row["ccn"].as_u64().unwrap_or(0);
        let nloc = fn_row["nloc"].as_u64().unwrap_or(0);
        if ccn > 15 || nloc > 100 {
            issues.push(serde_json::json!({
                "toolId": "Lizard",
                "patternId": "Lizard_ccn-hotspot",
                "filePath": fn_row["file"],
                "line": fn_row["start_line"],
                "severity": "Warning",
                "message": format!("{} has CCN {} and NLOC {}", fn_row["name"].as_str().unwrap_or("fn"), ccn, nloc),
            }));
        }
    }
    let lizard_issues = issues.len();
    for o in outcomes {
        issues.extend(o.issues.iter().cloned());
    }
    let mut errors = Vec::new();
    let mut tool_results = Vec::new();
    for row in tools {
        let id = row["label"].as_str().unwrap_or("tool");
        let is_lizard = row["tool"].as_str() == Some("lizard");
        let status_str = row["status"].as_str().unwrap_or("failed");
        let err_cnt = usize::from(status_str == "not_run");
        if let Some(msg) = row["error"].as_str() {
            errors.push(serde_json::json!({
                "toolId": id, "phase": "execute", "kind": "setup", "level": "error", "message": msg
            }));
        }
        let recorded = row["findings"].as_u64().unwrap_or(0) as usize;
        let issue_cnt = if is_lizard {
            lizard_issues
        } else if recorded > 0 {
            recorded
        } else {
            usize::from(status_str == "failed")
        };
        tool_results.push(serde_json::json!({
            "toolId": id,
            "issueCount": issue_cnt,
            "errorCount": err_cnt,
            "filesAnalyzed": files_analyzed,
            "status": if status_str == "passed" { "success" } else { status_str },
        }));
    }
    serde_json::json!({
        "issues": issues,
        "errors": errors,
        "toolResults": tool_results,
        "skippedFiles": [],
        "metadata": {
            "repositoryRoot": root,
            "startedAt": stamp.to_string(),
            "completedAt": stamp.to_string(),
            "executionMode": "magic-cli audit",
            "sourceRevision": comp["source_revision"],
        }
    })
}

fn save_native_reports(dir: &Path, native: &serde_json::Value) {
    let base_path = dir.join("audit-baseline.json");
    let cand_path = dir.join("audit-candidate.json");
    if !base_path.is_file() {
        if cand_path.is_file() {
            let _ = std::fs::copy(&cand_path, &base_path);
        } else if let Ok(bytes) = serde_json::to_vec_pretty(native) {
            let _ = std::fs::write(&base_path, bytes);
        }
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(native) {
        let _ = std::fs::write(&cand_path, bytes);
    }
}

fn print_lizard_report(comp: &serde_json::Value) {
    eprintln!(
        "[Lizard] scanned {} files, {} functions (total NLOC {})",
        comp["files_analyzed"].as_u64().unwrap_or(0),
        comp["functions_analyzed"].as_u64().unwrap_or(0),
        comp["total_nloc"].as_u64().unwrap_or(0)
    );
    let over_15 = comp["functions_over_ccn_15"].as_u64().unwrap_or(0);
    if over_15 > 0 {
        eprintln!(
            "[Lizard] {} functions exceed CCN 15 (max CCN {}):",
            over_15,
            comp["max_ccn"].as_u64().unwrap_or(0)
        );
        eprintln!("  CCN   NLOC  Location");
        eprintln!("  ---   ----  --------");
        if let Some(hotspots) = comp["hotspots"].as_array() {
            for spot in hotspots.iter().take(20) {
                let ccn = spot["ccn"].as_u64().unwrap_or(0);
                if ccn > 15 {
                    eprintln!(
                        "  {:3}   {:4}  {}:{} ({})",
                        ccn,
                        spot["nloc"].as_u64().unwrap_or(0),
                        spot["file"].as_str().unwrap_or("?"),
                        spot["start_line"].as_u64().unwrap_or(0),
                        spot["name"].as_str().unwrap_or("fn"),
                    );
                }
            }
        }
    } else {
        eprintln!(
            "[Lizard] clean: 0 functions exceed CCN 15 (max CCN {})",
            comp["max_ccn"].as_u64().unwrap_or(0)
        );
    }
}

fn record_mayhem_audit(
    start: &Path,
    base: &str,
    status: i32,
    outcomes: &[ToolOutcome],
    lizard_comp: Option<serde_json::Value>,
) {
    let root = repo_root(start);
    let dir = root.join("target/analytics");
    let _ = std::fs::create_dir_all(&dir);
    let comp = lizard_comp.unwrap_or_else(|| magicstudio_engine::complexity::read(&root, false));
    let tools = merge_tool_rows(&root, outcomes);
    let passed = tools.iter().filter(|t| t["status"] == "passed").count();
    let failed = tools.len().saturating_sub(passed);
    let total_findings: u64 = tools.iter().filter_map(|t| t["findings"].as_u64()).sum();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let summary = serde_json::json!({
        "available": true,
        "measured_at_unix_secs": stamp,
        "source_root": root,
        "source_revision": comp["source_revision"],
        "source_dirty": comp["source_dirty"],
        "base": base,
        "exit_code": status,
        "total": tools.len(),
        "passed": passed,
        "failed": failed,
        "passed_count": passed,
        "failed_count": failed,
        "total_findings": total_findings,
        "tools": tools,
        "complexity": comp,
        "complexity_summary": {
            "files_analyzed": comp["files_analyzed"],
            "functions_analyzed": comp["functions_analyzed"],
            "total_nloc": comp["total_nloc"],
            "total_source_lines": comp["total_source_lines"],
            "functions_over_ccn_15": comp["functions_over_ccn_15"],
            "max_ccn": comp["max_ccn"],
            "long_files": comp["long_files"],
        }
    });
    if let Ok(bytes) = serde_json::to_vec_pretty(&summary) {
        let _ = std::fs::write(dir.join("audit.json"), bytes);
    }
    let native = build_native_report(&root, &tools, outcomes, &comp, stamp);
    save_native_reports(&dir, &native);
    eprintln!(
        "[Mayhem] saved target/analytics/{{audit,lizard}}.json — files={} functions={} over_ccn_15={} max_ccn={} lines={} checks_passed={}/{} findings={}",
        comp["files_analyzed"].as_u64().unwrap_or(0),
        comp["functions_analyzed"].as_u64().unwrap_or(0),
        comp["functions_over_ccn_15"].as_u64().unwrap_or(0),
        comp["max_ccn"].as_u64().unwrap_or(0),
        comp["total_source_lines"].as_u64().unwrap_or(0),
        passed,
        tools.len(),
        total_findings
    );
}

fn default_review_tools(target: &Path) -> Vec<&'static str> {
    if target.is_file() {
        return match target.extension().and_then(|e| e.to_str()) {
            Some("ts" | "js" | "mjs") => vec!["tsc", "eslint"],
            Some("css") => vec!["stylelint"],
            Some("rs") => vec!["clippy"],
            _ => Vec::new(),
        };
    }
    let mut out = Vec::new();
    if has_ext_under(target, "rs") {
        out.push("clippy");
    }
    if has_ext_under(target, "ts") || has_ext_under(target, "js") {
        out.extend(["tsc", "eslint"]);
    }
    if has_ext_under(target, "css") {
        out.push("stylelint");
    }
    out
}

/// Auto-fix formatting and lint issues (`cargo fmt --all`, `eslint --fix`, `stylelint --fix`).
pub fn doctor(target: &Path, dry_run: bool) -> i32 {
    let start = if target.is_file() {
        target.parent().unwrap_or(target)
    } else {
        target
    };
    let root = repo_root(start);
    let mut status = 0;
    let fmt_cmd = vec!["cargo".into(), "fmt".into(), "--all".into()];
    if let Ok((code, _)) = run_native("cargo fmt", &fmt_cmd, &root, dry_run, None)
        && status == 0
    {
        status = code;
    }
    for (tool_name, label) in [("eslint", "eslint --fix"), ("stylelint", "stylelint --fix")] {
        let t = tool(tool_name);
        if let Ok(mut cmd) = build_command(&root, target, t) {
            cmd.insert(1, "--fix".into());
            if let Ok((code, _)) = run_native(label, &cmd, &root, dry_run, None)
                && status == 0
            {
                status = code;
            }
        }
    }
    status
}

/// Run the selected tools with inherited output; the first nonzero status, else 0. Setup
/// errors report 2 and the remaining tools still run.
pub fn run(
    target: &Path,
    selected: &[String],
    dry_run: bool,
    base: &str,
    review_mode: bool,
) -> i32 {
    let single_file = target.is_file();
    let mut names: Vec<&str> = if !selected.is_empty() {
        selected.iter().map(String::as_str).collect()
    } else if review_mode {
        default_review_tools(target)
    } else if single_file {
        vec!["clippy"]
    } else {
        TOOLS.iter().map(|t| t.name).collect()
    };
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(*n));
    let start = if single_file {
        target.parent().unwrap_or(target)
    } else {
        target
    };
    let mut status = 0;
    let mut outcomes = Vec::new();
    let mut lizard_comp = None;
    for name in names {
        let t = tool(name);
        let root = config_root(start, t);
        if !review_mode && name == "lizard" {
            let comp = magicstudio_engine::complexity::read(&root, !dry_run);
            print_lizard_report(&comp);
            let over_15 = comp["functions_over_ccn_15"].as_u64().unwrap_or(0);
            let code = if comp["available"] != true || over_15 > 0 {
                1
            } else {
                0
            };
            let mut issues = Vec::new();
            if let Some(hotspots) = comp["hotspots"].as_array() {
                for fn_row in hotspots.iter().take(25) {
                    let ccn = fn_row["ccn"].as_u64().unwrap_or(0);
                    let nloc = fn_row["nloc"].as_u64().unwrap_or(0);
                    if ccn > 15 || nloc > 100 {
                        issues.push(serde_json::json!({
                            "toolId": "Lizard",
                            "patternId": "Lizard_ccn-hotspot",
                            "filePath": fn_row["file"],
                            "line": fn_row["start_line"],
                            "severity": "Warning",
                            "message": format!("{} has CCN {} and NLOC {}", fn_row["name"].as_str().unwrap_or("fn"), ccn, nloc),
                        }));
                    }
                }
            }
            lizard_comp = Some(comp);
            outcomes.push(ToolOutcome {
                name: t.name,
                label: t.label,
                exit_code: code,
                findings: over_15 as usize,
                trace: "Lizard source scan; see critical function records".into(),
                issues,
                error: None,
            });
            if status == 0 {
                status = code;
            }
            continue;
        }
        let mut run_err = None;
        let mut out_text = String::new();
        let code = build_command(&root, target, t)
            .and_then(|command| run_tool(t, &command, &root, target, dry_run, base))
            .map(|(c, text)| {
                out_text = text;
                c
            })
            .unwrap_or_else(|error| {
                eprintln!("[{}] not run: {error}", t.label);
                run_err = Some(error);
                2
            });
        let mut issues = extract_tool_issues(t.name, t.label, &out_text);
        if single_file && t.name == "tsc" {
            let needle = target.file_name().unwrap_or_default().to_string_lossy();
            issues.retain(|i| {
                i["filePath"]
                    .as_str()
                    .is_some_and(|p| p.ends_with(needle.as_ref()))
            });
        }
        let findings = issues.len();
        outcomes.push(ToolOutcome {
            name: t.name,
            label: t.label,
            exit_code: code,
            findings,
            trace: out_text.lines().filter(|l|!l.trim().is_empty()).take(120).collect::<Vec<_>>().join("\n"),
            issues,
            error: run_err,
        });
        if status == 0 {
            status = code;
        }
    }
    if !dry_run && !review_mode {
        record_mayhem_audit(start, base, status, &outcomes, lizard_comp);
    }
    status
}
