//! Headless engine dispatch: one command (or a JSON-lines stream) against a session with no
//! window. Shared by `magic --headless` and the Rust `magic-cli`.
use crate::Session;
use serde_json::{Value, json};

fn evaluate_stream_line(s: &mut Session, line_num: usize, trimmed: &str, dry_run: bool) -> Value {
    let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
        return json!({"line": line_num, "ok": false, "error": "invalid json"});
    };
    let id = v["id"]
        .as_str()
        .or_else(|| v["command"].as_str())
        .unwrap_or("");
    let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
    if !v.is_object() || !params.is_object() {
        return json!({"line": line_num, "id": id, "ok": false, "error": "request and params must be objects"});
    }
    match dispatch(s, id, params, dry_run, true) {
        Ok(r) => json!({"line": line_num, "id": id, "ok": r["ok"] != false, "result": r}),
        Err(e) => json!({"line": line_num, "id": id, "ok": false, "error": e}),
    }
}

pub fn run_stream(s: &mut Session, keep_alive: bool, dry_run: bool) -> i32 {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut failed = false;
    for (n, line) in stdin.lock().lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let reply = evaluate_stream_line(s, n + 1, trimmed, dry_run);
        let error = reply["ok"] == false;
        println!("{reply}");
        if std::io::stdout().flush().is_err() {
            return 1;
        }
        failed |= error;
        if error && !keep_alive {
            break;
        }
    }
    if failed { 1 } else { 0 }
}

pub fn parse_cli_params(args: &[String]) -> (Value, bool) {
    let mut params = json!({});
    let mut dry_run = false;
    for arg in args {
        if arg == "--dry-run" {
            dry_run = true;
            continue;
        }
        if arg.starts_with('{')
            && let Ok(v) = serde_json::from_str::<Value>(arg)
        {
            params = v;
            continue;
        }
        if let Some((k, v)) = arg.split_once('=')
            && let Some(obj) = params.as_object_mut()
        {
            let parsed = serde_json::from_str::<Value>(v).unwrap_or_else(|_| json!(v));
            obj.insert(k.to_string(), parsed);
        }
    }
    (params, dry_run)
}

pub fn run_cli(args: Vec<String>) -> i32 {
    if args.first().map(String::as_str) == Some("commands") {
        let specs: Vec<Value> = crate::command_specs()
            .iter()
            .map(|s| s.describe())
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&specs).unwrap_or_default()
        );
        return 0;
    }
    let mut s = Session::user_session();
    if matches!(args.first().map(String::as_str), Some("run" | "serve")) {
        let keep_alive = args.first().map(String::as_str) == Some("serve");
        let dry_run = args.iter().any(|a| a == "--dry-run");
        return run_stream(&mut s, keep_alive, dry_run);
    }
    let (id, param_start) = if args.first().map(String::as_str) == Some("exec") {
        (args.get(1).map(String::as_str).unwrap_or(""), 2)
    } else {
        (args.first().map(String::as_str).unwrap_or(""), 1)
    };
    if id.is_empty() {
        eprintln!("magic --headless <command_id> [params_json | key=value ...]");
        return 2;
    }
    let (params, dry_run) = parse_cli_params(args.get(param_start..).unwrap_or(&[]));
    match dispatch(&mut s, id, params, dry_run, false) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            if v.get("ok").and_then(Value::as_bool) == Some(false) {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn attach_or_open_project(
    s: &mut Session,
    id: &str,
    params: &Value,
    persistent: bool,
) -> Result<(), String> {
    if matches!(
        id,
        "project.open"
            | "project.new"
            | "project.intake"
            | "project.attach"
            | "project.migrate"
            | "project.identity"
    ) {
        return Ok(());
    }
    let Some(target) = params["project"].as_str() else {
        return Ok(());
    };
    let identity = s
        .execute("project.identity", json!({"path": target}))
        .map_err(|e| e.to_string())?;
    if let Some(project) = identity["project"].as_str()
        && s.project_path.as_deref() != Some(std::path::Path::new(project))
    {
        s.execute(
            if persistent {
                "project.open"
            } else {
                "project.attach"
            },
            json!({"path": project, "claim_owner": persistent}),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn auto_load_asset(s: &mut Session, id: &str, params: &Value) {
    if !s.assets.is_empty() || s.project_assets.is_empty() || id.starts_with("project.") {
        return;
    }
    let wanted = params["asset"]
        .as_str()
        .or_else(|| params["character"].as_str())
        .map(str::to_owned)
        .or_else(|| {
            s.project_assets
                .iter()
                .find(|a| matches!(a.kind.as_str(), "character" | "cutout_rig"))
                .or_else(|| s.project_assets.first())
                .map(|a| a.id.clone())
        });
    if let Some(aid) = wanted {
        let _ = s.execute("project.load_asset", json!({"id": aid}));
    }
}

fn normalize_project_params(id: &str, params: &mut Value, dry_run: bool, persistent: bool) -> bool {
    let is_dry = dry_run || params.get("_dry_run").and_then(Value::as_bool) == Some(true);
    if let Some(obj) = params.as_object_mut() {
        obj.remove("_dry_run");
    }
    if matches!(id, "project.open" | "project.identity" | "project.attach")
        && params["path"].is_null()
        && params["project"].is_string()
    {
        params["path"] = params["project"].clone();
    }
    if persistent && id == "project.open" && !is_dry {
        params["claim_owner"] = json!(true);
    }
    is_dry
}

pub fn dispatch(
    s: &mut Session,
    id: &str,
    mut params: Value,
    dry_run: bool,
    persistent: bool,
) -> Result<Value, String> {
    if crate::find_command(id).is_none() {
        return Err(format!("Unknown engine command: {id}"));
    }
    let is_dry = normalize_project_params(id, &mut params, dry_run, persistent);
    // These observer queries validate their own arguments and must never attach, open or
    // auto-load an asset in the caller's persistent session, even for rejected requests.
    if matches!(id, "backstage.observe" | "backstage.changes") {
        return s.execute(id, params).map_err(|e| e.to_string());
    }
    attach_or_open_project(s, id, &params, persistent)?;
    if is_dry && !crate::commands::is_read_only_command(id, &params) {
        return Ok(json!({
            "ok": true,
            "dry_run": true,
            "command": id,
            "params": params,
            "project": s.project_path,
        }));
    }
    auto_load_asset(s, id, &params);
    let out = s.execute(id, params).map_err(|e| e.to_string())?;
    if !is_dry
        && crate::find_command(id).is_some_and(|c| c.journal)
        && s.edits_since_save > 0
        && s.project_path.is_some()
    {
        crate::project::write_project_file(s).map_err(|e| e.to_string())?;
    }
    Ok(out)
}
