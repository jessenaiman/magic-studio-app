//! Code complexity analytics for the app's Analytics view.
//!
//! Two costs are measured side by side: cyclomatic complexity (what a change
//! costs) and physical file length (what reading a change costs). Long files
//! are what slow agents down, so they are reported with the same weight as
//! complex functions instead of being left to guesswork.

use std::path::Path;

use serde_json::{json, Value};

use crate::workspace::resolve_craft_cli;

/// A file at or above this many lines is reported as a long file. The limit is
/// configurable in code-limits.json at the repository root.
const COMPLEXITY_LONG_FILE_LINES: u64 = 2000;

fn long_file_limit(root: &Path) -> u64 {
    let Ok(text) = std::fs::read_to_string(root.join("code-limits.json")) else {
        return COMPLEXITY_LONG_FILE_LINES;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return COMPLEXITY_LONG_FILE_LINES;
    };
    value["max_file_lines"]
        .as_u64()
        .unwrap_or(COMPLEXITY_LONG_FILE_LINES)
}

/// The sources worth measuring: the Rust engine and CLIs, the Python pipeline and
/// scripts, and the shell UI's own TypeScript/JavaScript modules (vendor bundles are excluded).
fn scan_targets(root: &Path) -> Vec<String> {
    let mut targets: Vec<String> = vec![
        "crates/engine/src".to_string(),
        "agent-harness/cli_anything/magic".to_string(),
        "apps/src-tauri/src".to_string(),
        "pipeline".to_string(),
        "scripts".to_string(),
    ];
    let ui_dir = root.join("apps/src-tauri/ui");
    let Ok(entries) = std::fs::read_dir(&ui_dir) else {
        return targets;
    };
    let all_paths: Vec<std::path::PathBuf> =
        entries.flatten().map(|entry| entry.path()).collect();
    let has_ts = all_paths
        .iter()
        .any(|p| p.extension().and_then(|ext| ext.to_str()) == Some("ts"));
    let wanted_ext = if has_ts { "ts" } else { "js" };
    let mut ui_files: Vec<String> = all_paths
        .into_iter()
        .filter_map(|path| {
            if path.extension().and_then(|ext| ext.to_str()) != Some(wanted_ext) {
                return None;
            }
            path.file_name()
                .map(|name| format!("apps/src-tauri/ui/{}", name.to_string_lossy()))
        })
        .collect();
    ui_files.sort();
    targets.extend(ui_files);
    targets
}

/// Run lizard over the given targets. A single scan keeps the common case fast; when
/// one target makes lizard fail to parse, the scan is retried per target so a single
/// unreadable file never hides every measurement. Failed targets are reported.
fn lizard_csv(lizard_bin: &str, root: &Path, targets: &[String]) -> (String, Vec<String>) {
    let run = |list: &[String]| {
        let mut command = std::process::Command::new(lizard_bin);
        command
            .current_dir(root)
            .args([
                "--csv",
                "-l",
                "rust",
                "-l",
                "python",
                "-l",
                "javascript",
                "-l",
                "typescript",
            ]);
        for target in list {
            command.arg(target);
        }
        command.output()
    };
    let Ok(out) = run(targets) else {
        return (String::new(), targets.to_vec());
    };
    if out.status.success() {
        return (String::from_utf8_lossy(&out.stdout).to_string(), Vec::new());
    }
    let mut stdout = String::new();
    let mut failures = Vec::new();
    for target in targets {
        match run(std::slice::from_ref(target)) {
            Ok(single) if single.status.success() => {
                stdout.push_str(&String::from_utf8_lossy(&single.stdout));
            }
            _ => failures.push(target.clone()),
        }
    }
    (stdout, failures)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                res.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    res.push(cur);
    res
}

fn saved_scan(root: &Path, cache: &Path) -> Option<Value> {
    std::fs::read(cache)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|v| v["available"] == true && v["source_root"] == root.to_string_lossy().as_ref())
}

fn unscanned_result(
    previous: Option<Value>,
    root: &Path,
    revision: &Option<String>,
    dirty: Option<bool>,
) -> Value {
    previous.map(|value| cached_result(value, revision, dirty)).unwrap_or_else(||
        json!({"available":false,"tool":"lizard","state":"not_measured","reason":"No saved scan. Refresh scan to measure code.","source_root":root,"current_revision":revision}))
}

fn annotate_tool(result: &mut Value) {
    let executable = resolve_craft_cli("lizard");
    let executable_path = if Path::new(&executable).is_file() {
        Some(std::path::PathBuf::from(&executable))
    } else {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(&executable))
                .find(|path| path.is_file())
        })
    };
    result["tool_path"] = json!(executable_path);
    result["tool_version"] = json!(std::process::Command::new(&executable)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()));
}

fn scan_state_label(result: &Value, partial: bool) -> &'static str {
    if result["available"] != true {
        "error"
    } else if partial {
        "partial"
    } else {
        "measured"
    }
}

fn unavailable_result(
    mut result: Value,
    previous: Option<Value>,
    revision: &Option<String>,
    dirty: Option<bool>,
) -> Value {
    let error = "Lizard returned no measurements; check tool availability and scan_failures.";
    if let Some(value) = previous {
        let mut saved = cached_result(value, revision, dirty);
        saved["last_error"] = json!(error);
        saved["last_scan_failures"] = result["scan_failures"].clone();
        return saved;
    }
    result["reason"] = json!(error);
    result
}

fn save_scan(cache: &Path, result: &Value) -> std::io::Result<()> {
    let parent = cache
        .parent()
        .ok_or_else(|| std::io::Error::other("cache has no parent"))?;
    std::fs::create_dir_all(parent)?;
    // A unique temporary file prevents concurrent scans from truncating the saved result.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_nanos())
        .unwrap_or(0);
    let temporary = cache.with_extension(format!("{}-{stamp}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(result).map_err(std::io::Error::other)?;
    std::fs::write(&temporary, bytes)?;
    if let Err(error) = std::fs::rename(&temporary, cache) {
        let _ = std::fs::remove_file(temporary);
        return Err(error);
    }
    Ok(())
}

/// Measure the repository's complexity and reading cost.
pub fn read(root: &Path, scan: bool) -> Value {
    let cache = root.join("target/analytics/lizard.json");
    let revision = source_revision(root);
    let dirty = source_dirty(root);
    let previous = saved_scan(root, &cache);
    if !scan {
        return unscanned_result(previous, root, &revision, dirty);
    }
    let mut result = measure(root);
    result["tool"] = json!("lizard");
    annotate_tool(&mut result);
    result["targets"] = json!(scan_targets(root));
    result["source_root"] = json!(root);
    result["source_revision"] = json!(revision);
    result["source_dirty"] = json!(dirty);
    result["measured_at_unix_secs"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|v| v.as_secs()));
    result["cached"] = json!(false);
    let partial = result["scan_failures"]
        .as_array()
        .is_some_and(|failures| !failures.is_empty());
    result["state"] = json!(scan_state_label(&result, partial));
    if result["available"] != true {
        return unavailable_result(result, previous, &revision, dirty);
    }
    // A partial scan must not replace the last complete saved measurement.
    if partial {
        return result;
    }
    if let Err(error) = save_scan(&cache, &result) {
        result["cache_error"] = json!(error.to_string());
    }
    result
}

fn source_revision(root: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn source_dirty(root: &Path) -> Option<bool> {
    std::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--",
            "crates/engine/src",
            "agent-harness/cli_anything/magic",
            "apps/src-tauri/src",
            "apps/src-tauri/ui",
            "pipeline",
            "scripts",
            "code-limits.json",
        ])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
}

fn cached_result(mut value: Value, revision: &Option<String>, dirty: Option<bool>) -> Value {
    value["cached"] = json!(true);
    value["current_revision"] = json!(revision);
    value["state"] = json!(if value["scan_failures"]
        .as_array()
        .is_some_and(|failures| !failures.is_empty())
    {
        "partial"
    } else if revision.is_none()
        || dirty.is_none()
        || dirty == Some(true)
        || value["source_dirty"] != false
    {
        "worktree_unverified"
    } else if value["source_revision"] != json!(revision) {
        "stale"
    } else {
        "saved"
    });
    value
}

struct ScanTally {
    functions: Vec<Value>,
    files: std::collections::HashSet<String>,
    per_file: std::collections::BTreeMap<String, (u64, usize, u64)>,
    total_nloc: u64,
    total_tokens: u64,
    over_15: usize,
    max_ccn: u64,
}

fn tally_rows(stdout: &str) -> ScanTally {
    let mut tally = ScanTally {
        functions: Vec::new(),
        files: std::collections::HashSet::new(),
        per_file: std::collections::BTreeMap::new(),
        total_nloc: 0,
        total_tokens: 0,
        over_15: 0,
        max_ccn: 0,
    };
    for line in stdout.lines() {
        let fields = parse_csv_line(line);
        if fields.len() < 11 {
            continue;
        }
        let Ok(nloc) = fields[0].parse::<u64>() else {
            continue;
        };
        let Ok(ccn) = fields[1].parse::<u64>() else {
            continue;
        };
        let Ok(tokens) = fields[2].parse::<u64>() else {
            continue;
        };
        let Ok(params) = fields[3].parse::<u64>() else {
            continue;
        };
        let file = fields[6].clone();
        let name = fields[7].clone();
        let start_line = fields[9].parse::<u64>().unwrap_or(1);
        let end_line = fields[10].parse::<u64>().unwrap_or(start_line);

        tally.files.insert(file.clone());
        let entry = tally.per_file.entry(file.clone()).or_insert((0, 0, 0));
        entry.0 = entry.0.saturating_add(ccn);
        entry.1 += 1;
        entry.2 = entry.2.max(ccn);
        tally.total_nloc = tally.total_nloc.saturating_add(nloc);
        tally.total_tokens = tally.total_tokens.saturating_add(tokens);
        if ccn > 15 {
            tally.over_15 += 1;
        }
        if ccn > tally.max_ccn {
            tally.max_ccn = ccn;
        }
        tally.functions.push(json!({
            "name": name,
            "file": file,
            "lang": file_lang(&file),
            "start_line": start_line,
            "end_line": end_line,
            "nloc": nloc,
            "ccn": ccn,
            "tokens": tokens,
            "params": params,
        }));
    }
    tally
}

fn file_lang(file: &str) -> &'static str {
    match Path::new(file).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "tsx",
        Some("js" | "mjs") => "javascript",
        Some("css") => "css",
        Some("html") => "html",
        Some("md") => "markdown",
        Some("py") => "python",
        _ => "other",
    }
}

fn lang_label(lang: &str) -> &'static str {
    match lang {
        "rust" => "Rust (.rs)",
        "typescript" => "TypeScript (.ts)",
        "tsx" => "TSX · Remotion (.tsx)",
        "javascript" => "JavaScript (.js/.mjs)",
        "css" => "CSS (.css)",
        "html" => "HTML · Shell (.html)",
        "markdown" => "Markdown (.md)",
        "python" => "Python (.py)",
        _ => "Other",
    }
}

fn tracked_files_for_exts(root: &Path) -> Vec<String> {
    let Ok(out) = std::process::Command::new("git")
        .args([
            "ls-files",
            "*.rs",
            "*.ts",
            "*.tsx",
            "*.js",
            "*.mjs",
            "*.css",
            "*.html",
            "*.md",
            "*.py",
        ])
        .current_dir(root)
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

struct LangBucket {
    files: usize,
    lines: u64,
    ccn: u64,
    max_ccn: u64,
    over_15: usize,
    long_files: usize,
    top_file: String,
    top_file_lines: u64,
}

fn update_bucket_code(bucket: &mut LangBucket, ccn: u64, max_file_ccn: u64, over_15: usize) {
    bucket.ccn = bucket.ccn.saturating_add(ccn);
    bucket.max_ccn = bucket.max_ccn.max(max_file_ccn);
    bucket.over_15 += over_15;
}

fn build_lang_summary(
    root: &Path,
    per_file: &std::collections::BTreeMap<String, (u64, usize, u64)>,
    functions: &[Value],
    long_file_limit: u64,
) -> (Vec<Value>, Vec<Value>) {
    let mut over_by_file: std::collections::BTreeMap<&str, usize> =
        std::collections::BTreeMap::new();
    for func in functions {
        if func["ccn"].as_u64().unwrap_or(0) > 15
            && let Some(path) = func["file"].as_str()
        {
            *over_by_file.entry(path).or_insert(0) += 1;
        }
    }
    let mut buckets: std::collections::BTreeMap<&'static str, LangBucket> =
        std::collections::BTreeMap::new();
    let mut all_files: Vec<Value> = Vec::new();
    for file in tracked_files_for_exts(root) {
        let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        let lines = text.lines().count() as u64;
        let lang = file_lang(&file);
        let (ccn, fn_cnt, max_ccn) = per_file.get(&file).copied().unwrap_or((0, 0, 0));
        let over_15 = over_by_file.get(file.as_str()).copied().unwrap_or(0);
        let is_long = lines >= long_file_limit;
        let bucket = buckets.entry(lang).or_insert(LangBucket {
            files: 0,
            lines: 0,
            ccn: 0,
            max_ccn: 0,
            over_15: 0,
            long_files: 0,
            top_file: String::new(),
            top_file_lines: 0,
        });
        bucket.files += 1;
        bucket.lines = bucket.lines.saturating_add(lines);
        if is_long {
            bucket.long_files += 1;
        }
        if lines > bucket.top_file_lines {
            bucket.top_file_lines = lines;
            bucket.top_file.clone_from(&file);
        }
        update_bucket_code(bucket, ccn, max_ccn, over_15);
        all_files.push(json!({
            "file": file,
            "lang": lang,
            "lines": lines,
            "ccn": ccn,
            "functions": fn_cnt,
            "max_ccn": max_ccn,
            "over_15": over_15,
            "long": is_long,
        }));
    }
    all_files.sort_by_key(|row| std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)));
    let mut lang_rows: Vec<Value> = buckets
        .into_iter()
        .map(|(lang, b)| {
            json!({
                "lang": lang,
                "label": lang_label(lang),
                "files": b.files,
                "lines": b.lines,
                "ccn": b.ccn,
                "max_ccn": b.max_ccn,
                "over_15": b.over_15,
                "long_files": b.long_files,
                "top_file": b.top_file,
                "top_file_lines": b.top_file_lines,
            })
        })
        .collect();
    lang_rows.sort_by_key(|row| std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)));
    (lang_rows, all_files)
}

fn file_cost_rows(
    root: &Path,
    per_file: &std::collections::BTreeMap<String, (u64, usize, u64)>,
    scan_failures: &mut Vec<String>,
    long_file_limit: u64,
) -> Vec<Value> {
    // Per-file cost: physical lines (what an agent has to read) next to cyclomatic
    // complexity (what the code costs to change).
    per_file
        .iter()
        .map(|(file, (ccn, functions, max_file_ccn))| {
            let lines = match std::fs::read_to_string(root.join(file)) {
                Ok(text) => Some(text.lines().count() as u64),
                Err(_) => {
                    scan_failures.push(file.clone());
                    None
                }
            };
            json!({
                "file": file,
                "lang": file_lang(file),
                "lines": lines,
                "ccn": ccn,
                "functions": functions,
                "max_ccn": max_file_ccn,
                "long": lines.map(|value| value >= long_file_limit),
            })
        })
        .collect()
}

fn long_file_rows(file_rows: &[Value], all_files: &[Value]) -> (Vec<Value>, usize) {
    let long_files: Vec<Value> = file_rows
        .iter()
        .filter(|row| row["long"].as_bool().unwrap_or(false))
        .cloned()
        .collect();
    let source = if all_files.is_empty() {
        &long_files
    } else {
        all_files
    };
    let longest_files: Vec<Value> = source.iter().take(15).cloned().collect();
    (longest_files, long_files.len())
}

fn largest_ccn_rows(file_rows: &[Value]) -> Vec<Value> {
    let mut by_ccn: Vec<Value> = file_rows.to_vec();
    by_ccn.sort_by_key(|row| {
        (
            std::cmp::Reverse(row["ccn"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)),
        )
    });
    by_ccn.iter().take(15).cloned().collect()
}

fn count_ui_files(file_rows: &[Value]) -> usize {
    file_rows
        .iter()
        .filter(|row| {
            row["file"]
                .as_str()
                .unwrap_or("")
                .starts_with("apps/src-tauri/ui/")
        })
        .count()
}

fn measure(root: &Path) -> Value {
    let lizard_bin = resolve_craft_cli("lizard");
    let long_file_limit = long_file_limit(root);
    let targets = scan_targets(root);
    let (stdout, mut scan_failures) = lizard_csv(&lizard_bin, root, &targets);
    let mut tally = tally_rows(&stdout);
    tally.functions.sort_by_key(|f| {
        (
            std::cmp::Reverse(f["ccn"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(f["nloc"].as_u64().unwrap_or(0)),
        )
    });
    let hotspots: Vec<Value> = tally.functions.iter().take(25).cloned().collect();
    let mut file_rows = file_cost_rows(root, &tally.per_file, &mut scan_failures, long_file_limit);
    file_rows.sort_by_key(|row| {
        (
            std::cmp::Reverse(row["lines"].as_u64().unwrap_or(0)),
            std::cmp::Reverse(row["ccn"].as_u64().unwrap_or(0)),
        )
    });
    let (languages, all_tracked_files) =
        build_lang_summary(root, &tally.per_file, &tally.functions, long_file_limit);
    let (longest_files, long_file_count) = long_file_rows(&file_rows, &all_tracked_files);
    let largest_ccn_files = largest_ccn_rows(&file_rows);
    let ui_files_analyzed = count_ui_files(&file_rows);
    let total_lines: u64 = file_rows
        .iter()
        .map(|row| row["lines"].as_u64().unwrap_or(0))
        .sum();

    json!({
        "available": !tally.functions.is_empty(),
        "tool": "lizard",
        "files_analyzed": tally.files.len(),
        "functions_analyzed": tally.functions.len(),
        "total_nloc": tally.total_nloc,
        "total_code_tokens": tally.total_tokens,
        "functions_over_ccn_15": tally.over_15,
        "max_ccn": tally.max_ccn,
        "hotspots": hotspots,
        "languages": languages,
        "long_file_threshold_lines": long_file_limit,
        "long_files": long_file_count,
        "longest_files": longest_files,
        "largest_ccn_files": largest_ccn_files,
        "ui_files_analyzed": ui_files_analyzed,
        "total_source_lines": total_lines,
        "scan_failures": scan_failures,
    })
}
