//! Mayhem scores the last saved source measurement. Only an explicit scan runs Lizard.
use crate::doctor::analytics_source_root;
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const METRICS: [(&str, &str); 4] = [
    ("total_source_lines", "Source lines"),
    ("functions_over_ccn_15", "Functions above CCN 15"),
    ("max_ccn", "Peak complexity"),
    ("functions_analyzed", "Functions measured"),
];

fn snapshot(root: &Path) -> Option<Value> {
    let file = fs::File::open(root.join("target/analytics/lizard.json")).ok()?;
    let scan: Value = serde_json::from_reader(file).ok()?;
    (scan["available"] == true && scan["source_root"].as_str() == root.to_str()).then_some(scan)
}

fn key(scan: &Value) -> String {
    format!(
        "{}-{}",
        scan["source_revision"].as_str().unwrap_or("local"),
        scan["measured_at_unix_secs"]
    )
}

pub fn score(s: &mut Session, _: &Value) -> Result<Value> {
    let root = analytics_source_root(s);
    let Some(scan) = snapshot(&root) else {
        return Ok(
            json!({"signal":"yellow","reason":"Measure the source to establish a baseline.","metrics":[]}),
        );
    };
    let history = crate::complexity::read_history(&root, &scan);
    let previous = history
        .iter()
        .rev()
        .find(|h| h["measured_at_unix_secs"].as_u64() < scan["measured_at_unix_secs"].as_u64());
    let metrics: Vec<_> = METRICS.iter().map(|(k,label)| {
        let current = scan[*k].as_i64();
        let prior = previous.and_then(|h| h[*k].as_i64());
        json!({"key":k,"label":label,"current":current,"previous":prior,"delta":current.zip(prior).map(|(a,b)| a-b)})
    }).collect();
    let limits: Value = serde_json::from_reader(fs::File::open(root.join("code-limits.json"))?)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    let review = fs::read(root.join("target/analytics/package-review.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(|v| v["snapshot"] == key(&scan));
    let revision_changed =
        crate::complexity::source_revision(&root).as_deref() != scan["source_revision"].as_str();
    let violations = scan["critical_functions"]
        .as_array()
        .zip(scan["oversized_files"].as_array())
        .map(|(functions, files)| functions.len() + files.len());
    let (signal, reason) = if violations.is_some_and(|count| count > 0) {
        (
            "red",
            format!(
                "{} code-limit violations in the saved scan.",
                violations.unwrap_or_default()
            ),
        )
    } else if review.as_ref().is_some_and(|r| r["verdict"] == "violation") {
        (
            "red",
            "An outside review identified a violation of Rule #1.".into(),
        )
    } else if !scan["scan_failures"].as_array().is_some_and(Vec::is_empty)
        || violations.is_none()
        || scan["limits"] != limits
    {
        (
            "yellow",
            "Measurement or rule evidence is incomplete.".into(),
        )
    } else if !review.as_ref().is_some_and(|r| r["verdict"] == "clear") {
        (
            "yellow",
            "Rule #1 needs an outside review or follow-up.".into(),
        )
    } else if revision_changed {
        (
            "yellow",
            "The source revision changed after this measurement.".into(),
        )
    } else if previous.is_none()
        || metrics[..3]
            .iter()
            .any(|m| m["delta"].as_i64().is_some_and(|v| v > 0))
    {
        (
            "yellow",
            "Review the change against the previous measurement.".into(),
        )
    } else {
        (
            "green",
            "Measured limits and the package review passed at this scan.".into(),
        )
    };
    Ok(
        json!({"signal":signal,"reason":reason,"snapshot":key(&scan),"measured_at":scan["measured_at_unix_secs"],
        "revision":scan["source_revision"],"working_tree_modified":scan["source_dirty"],"revision_changed":revision_changed,"metrics":metrics,
        "package_review":review,"hotspot":scan["hotspots"][0],"history":history.iter().map(|h|
        json!({"time":h["measured_at_unix_secs"],"lines":h["total_source_lines"]})).collect::<Vec<_>>()}),
    )
}

pub fn review(s: &mut Session, p: &Value) -> Result<Value> {
    let root = analytics_source_root(s);
    let required = |k| {
        p.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| EngineError::InvalidParams(format!("Missing {k}")))
    };
    let token = required("snapshot")?;
    if !snapshot(&root).is_some_and(|v| key(&v) == token) {
        return Err(EngineError::InvalidParams(
            "Snapshot changed. Run mayhem.score first.".into(),
        ));
    }
    let verdict = required("verdict")?;
    if !matches!(verdict, "clear" | "follow_up" | "violation") {
        return Err(EngineError::InvalidParams(
            "Use clear, follow_up or violation.".into(),
        ));
    }
    let entry = json!({"snapshot":token,"verdict":verdict,"reviewer":required("reviewer")?,
        "package":required("package")?,"finding":required("finding")?,"evidence":required("evidence")?});
    crate::complexity::save_scan(&root.join("target/analytics/package-review.json"), &entry)?;
    score(s, &Value::Null)
}

pub fn scan(s: &mut Session, _: &Value) -> Result<Value> {
    let measured = crate::complexity::read(&analytics_source_root(s), true);
    if measured["state"] == "partial"
        || measured["available"] != true
        || measured["last_error"].is_string()
        || measured["cache_error"].is_string()
    {
        return Err(EngineError::InvalidParams(format!(
            "Measurement failed: {}",
            measured
        )));
    }
    score(s, &Value::Null)
}

/// Limit the self-review to the measurement, audit and dashboard implementations.
fn own_file(root: &Path, path: &Value) -> bool {
    let file = Path::new(path.as_str().unwrap_or_default());
    let file = file.strip_prefix(root).unwrap_or(file);
    matches!(
        file.to_str(),
        Some(
            "crates/engine/src/mayhem.rs"
                | "crates/engine/src/complexity.rs"
                | "crates/engine/src/doctor.rs"
                | "apps/magic-cli/src/audit.rs"
                | "apps/backstage/src/mayhem.rs"
                | "apps/src-tauri/ui/analytics.ts"
                | "apps/src-tauri/ui/mayhem.css"
                | "apps/src-tauri/frontend/analytics-charts.ts"
        )
    )
}

fn self_audit(root: &Path, scan: &Value, audit: &Value) -> Value {
    let mut findings = Vec::new();
    for kind in ["critical_functions", "oversized_files"] {
        for row in scan[kind]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|r| own_file(root, &r["file"]))
        {
            findings.push(
                json!({"tool":"lizard","file":row["file"],"line":row["start_line"],"evidence":row}),
            );
        }
    }
    let mut measured = Vec::new();
    for tool in audit["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["issues"].is_array() && t["status"] != "not_run" && t["tool"] != "lizard")
    {
        measured
            .push(json!({"tool":tool["tool"],"time":tool["measured_at"],"status":tool["status"]}));
        for issue in tool["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| own_file(root, &i["filePath"]))
        {
            findings.push(json!({"tool":tool["tool"],"file":issue["filePath"],"line":issue["line"],"evidence":issue}));
        }
    }
    json!({"findings":findings,"scanner_reports":measured,"source_measured":scan["critical_functions"].is_array()})
}

/// Dashboard evidence is separate so the terminal score never enumerates agents or commands.
pub fn evidence(s: &mut Session, params: &Value) -> Result<Value> {
    let root = analytics_source_root(s);
    let scan = snapshot(&root).unwrap_or_default();
    let audit = fs::read(root.join("target/analytics/audit.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|a| a["source_root"].as_str() == root.to_str())
        .unwrap_or_default();
    let own = self_audit(&root, &scan, &audit);
    if params["self"] == true {
        return Ok(
            json!({"snapshot":key(&scan),"measured_at":scan["measured_at_unix_secs"],"self_audit":own}),
        );
    }
    let log = crate::activity::events(s, &json!({"limit":192}))?;
    let cli_events: Vec<_> = log["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["origin"] == "CLI")
        .map(|e| json!({"command":e["command"],"stage":e["stage"],"request":e["request_id"]}))
        .collect();
    let commands: Vec<_> = crate::command_specs()
        .iter()
        .map(|c| json!({"id":c.id,"label":c.label,"params":c.params}))
        .collect();
    Ok(
        json!({"self_audit":own,"agents":crate::doctor::agent_locations(),"rules":scan["limits"],
        "critical_functions":scan["critical_functions"],"oversized_files":scan["oversized_files"],
        "cli_scope":scan["cli_scope"],"cli_events":cli_events,"commands":commands,
        "scanners":audit["tools"],"audit_revision":audit["source_revision"],"root":root}),
    )
}
