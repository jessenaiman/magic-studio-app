//! `cargo xtask bench [--compare OLD.jsonl]`: measurable "is the system healthy" numbers —
//! source lines per crate group, binary sizes and engine cold start — appended to
//! `target/bench/results.jsonl`. `--compare` prints deltas against a previous run, so code or
//! CPU added by an agent shows up as a diff line instead of an opinion.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::Result;

fn loc(dir: &Path, exts: &[&str]) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| entry.path())
        .map(|path| {
            if path.is_dir() {
                // Vendored bundles are not our code; their size is tracked separately.
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == "vendor" || n == "node_modules") {
                    return 0;
                }
                loc(&path, exts)
            } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.contains(&e)) {
                std::fs::read_to_string(&path).map(|text| text.lines().count() as u64).unwrap_or(0)
            } else {
                0
            }
        })
        .sum()
}

fn loc_vendor(dir: &Path) -> u64 {
    let vendor = dir.join("vendor");
    std::fs::read_dir(&vendor)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "js" || e == "ts"))
        .map(|p| std::fs::read_to_string(&p).map(|t| t.lines().count() as u64).unwrap_or(0))
        .sum()
}

/// Wall time of one command in milliseconds, or `None` when it cannot run.
fn probe_ms(cmd: &mut Command, runs: u32) -> Option<f64> {
    let mut times = Vec::new();
    for _ in 0..runs {
        let started = Instant::now();
        if cmd.status().is_ok_and(|s| s.success()) {
            times.push(started.elapsed().as_secs_f64() * 1000.0);
        }
    }
    (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64)
}

fn file_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub(crate) fn bench(compare: Option<PathBuf>) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_default();

    let mut sample = serde_json::Map::new();
    sample.insert("at_unix_ms".into(), json!(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default()));

    // Source lines per crate group (the `SETS` groups keep every crate counted exactly once).
    for (name, pkgs) in crate::ci::SETS {
        let mut lines = 0u64;
        for pkg in *pkgs {
            let dir = root.join(pkg_path(pkg)).join("src");
            lines += loc(&dir, &["rs"]);
        }
        sample.insert(format!("loc_rust.{name}"), json!(lines));
    }
    let ui = root.join("apps/magic/ui");
    let cli_ui = root.join("apps/magic-cli/ui");
    sample.insert("loc_ts".into(), json!(loc(&ui, &["ts", "tsx"]) + loc(&cli_ui, &["js"])));
    sample.insert("loc_ts_vendor".into(), json!(loc_vendor(&ui) + loc_vendor(&cli_ui)));

    // Built binary sizes and engine cold start: the numbers that grow when someone "adds more".
    for bin in ["magic", "magic-cli"] {
        sample.insert(format!("bin_bytes.{bin}"), json!(file_bytes(&root.join("target/debug").join(bin))));
    }
    let cli = root.join("target/debug/magic-cli");
    if cli.is_file()
        && let Some(ms) = probe_ms(Command::new(&cli).arg("commands").stdout(std::process::Stdio::null()), 3)
    {
        sample.insert("cli_cold_start_ms".into(), json!((ms * 10.0).round() / 10.0));
    }
    let out_dir = root.join("target/bench");
    std::fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let out_path = out_dir.join("results.jsonl");
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&out_path).map_err(|e| e.to_string())?;
    let line = Value::Object(sample).to_string();
    writeln!(file, "{line}").map_err(|e| e.to_string())?;
    eprintln!("bench: recorded {}", out_path.display());

    if let Some(old_path) = compare {
        let old: Value = std::fs::read_to_string(&old_path)
            .map_err(|e| e.to_string())
            .and_then(|text| text.lines().last().map(|l| l.to_owned()).ok_or_else(|| "compare file is empty".to_owned()))
            .and_then(|l| serde_json::from_str(&l).map_err(|e| e.to_string()))?;
        let new: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        for (key, value) in new.as_object().into_iter().flatten() {
            if key == "at_unix_ms" {
                continue;
            }
            let (Some(before), Some(after)) = (old[key].as_f64(), value.as_f64()) else { continue };
            let delta = after - before;
            if delta.abs() >= 1.0 {
                eprintln!("bench: {key}: {before:.0} -> {after:.0} ({delta:+.0})");
            }
        }
    }
    Ok(())
}

fn pkg_path(pkg: &str) -> String {
    match pkg {
        "magic-engine" => "crates/engine".into(),
        "magic-mcp" => "crates/mcp".into(),
        "magic-cli" => "apps/magic-cli".into(),
        "magic" => "apps/magic".into(),
        "magic-tui" => "apps/magic-tui".into(),
        "xtask" => "xtask".into(),
        other => format!("crates/{other}"),
    }
}
