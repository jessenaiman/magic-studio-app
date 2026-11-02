//! `doctor` subcommand (`magic-cli app doctor` / `app.doctor`):
//! lightweight local check of external CLI tool availability and stale `.video-server.json` state.
//! Never scans `/proc` to police or kill external processes.

use std::path::Path;

use serde_json::{Value, json};

use crate::workspace::resolve_tool;
use crate::{Result, Session};

pub fn cmd_app_doctor(s: &mut Session, params: &Value) -> Result<Value> {
    let clean = params
        .get("clean")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let cleaned_files: Vec<String> = Vec::new();
    let project_dir = s
        .project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    let mut video_server_state = json!("none");
    if server_file.is_file() {
        if clean {
            video_server_state = json!({"file": server_file, "clean_requested": true, "removed": false, "note": "ownership/staleness not verified; not deleted"});
        } else {
            video_server_state = json!({"file": server_file});
        }
    }

    let tools: Vec<Value> = [
        "magic-cli",
        "codex",
        "hyperframes",
        "photocraft-cli",
        "vectorcraft-cli",
        "effectcraft-cli",
        "filmcraft-cli",
        "deckcraft-cli",
        "pdfcraft-cli",
        "ffmpeg",
        "ffprobe",
    ]
    .iter()
    .map(|name| {
        let resolved = resolve_tool(name);
        let found = Path::new(&resolved).is_file() || which_in_path(&resolved);
        json!({ "tool": name, "resolved": resolved, "available": found })
    })
    .collect();

    Ok(json!({
        "ok": true,
        "cleaned": {
            "stopped_pids": null,
            "removed_files": cleaned_files,
            "note": "processes are not scanned or killed; .video-server.json is reported but not verified or deleted"
        },
        "processes": null,
        "video_server": video_server_state,
        "tools": tools,
        "ponytail_candidates": null,
        "recommendations": [],
    }))
}

pub fn cmd_app_analytics(s: &mut Session, params: &Value) -> Result<Value> {
    let scan = params.get("scan").and_then(Value::as_bool).unwrap_or(false);
    let doctor = cmd_app_doctor(s, &json!({ "clean": false }))?;
    let host = read_host_telemetry();
    let usage = read_omarchy_usage();
    let project = read_project_analytics(s);
    let root = crate::workspace::workspace_root(s);
    let complexity = crate::complexity::read(&root, scan);
    let worktrees = read_agent_worktrees(&root, scan);
    let errors = crate::activity::errors(s, &json!({}))?;

    Ok(json!({
        "ok": true,
        "host": host,
        "tools": doctor["tools"],
        "video_server": doctor["video_server"],
        "usage": usage,
        "project": project,
        "complexity": complexity,
        "worktrees": worktrees,
        "runtime_errors": errors,
        "sampled_at_unix_secs": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|v|v.as_secs()),
        "sources": {"host":"/proc","usage":"Omarchy local usage records","project":"Project session and .agent chat log; estimated tokens = characters / 4","worktrees":"git worktree list + gh issue list + herdr agent list"},
        "commands": crate::workshop::cmd_query_list(s, &json!({}))?,
    }))
}

fn read_host_telemetry() -> Value {
    let pid = std::process::id();
    let self_rss_kb = std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        });
    let load = std::fs::read_to_string("/proc/loadavg").ok().and_then(|s| {
        s.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
            .into()
    });
    let mut mem_total_kb: Option<u64> = None;
    let mut mem_avail_kb: Option<u64> = None;
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                mem_total_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                mem_avail_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
            }
        }
    }
    json!({
        "pid": pid,
        "self_rss_kb": self_rss_kb,
        "load": load,
        "mem_total_kb": mem_total_kb,
        "mem_used_kb": mem_total_kb.zip(mem_avail_kb).map(|(total,available)|total.saturating_sub(available)),
    })
}

#[derive(Default)]
struct UsageTotals {
    today_tokens: u64,
    today_prompts: u64,
    input: u64,
    output: u64,
    cache_read: u64,
}

fn read_usage_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()
}

fn model_usage(v: &Value) -> (u64, u64, u64, Vec<Value>) {
    let mut input_sum = 0u64;
    let mut output_sum = 0u64;
    let mut cache_sum = 0u64;
    let mut models = Vec::new();
    if let Some(map) = v["modelUsage"].as_object() {
        for (m_id, mv) in map {
            let inp = mv["inputTokens"].as_u64().unwrap_or(0);
            let out = mv["outputTokens"].as_u64().unwrap_or(0);
            let cr = mv["cacheReadInputTokens"].as_u64().unwrap_or(0);
            input_sum = input_sum.saturating_add(inp);
            output_sum = output_sum.saturating_add(out);
            cache_sum = cache_sum.saturating_add(cr);
            if inp + out + cr > 0 {
                models.push(json!({
                    "model": m_id,
                    "input_tokens": inp,
                    "output_tokens": out,
                    "cache_read_tokens": cr,
                    "total_tokens": inp.saturating_add(out).saturating_add(cr),
                }));
            }
        }
    }
    models.sort_by_key(|m| std::cmp::Reverse(m["total_tokens"].as_u64().unwrap_or(0)));
    (input_sum, output_sum, cache_sum, models)
}

fn usage_row(v: &Value, totals: &mut UsageTotals) -> Option<Value> {
    let today_tokens = v["todayTotalTokens"].as_u64().unwrap_or(0);
    let today_prompts = v["todayPrompts"].as_u64().unwrap_or(0);
    let total_prompts = v["totalPrompts"].as_u64().unwrap_or(0);
    let total_sessions = v["totalSessions"].as_u64().unwrap_or(0);
    let (input_sum, output_sum, cache_sum, models) = model_usage(v);
    let total_agent_tokens = input_sum
        .saturating_add(output_sum)
        .saturating_add(cache_sum);
    if today_tokens == 0 && total_prompts == 0 && total_agent_tokens == 0 {
        return None;
    }
    totals.today_tokens = totals.today_tokens.saturating_add(today_tokens);
    totals.today_prompts = totals.today_prompts.saturating_add(today_prompts);
    totals.input = totals.input.saturating_add(input_sum);
    totals.output = totals.output.saturating_add(output_sum);
    totals.cache_read = totals.cache_read.saturating_add(cache_sum);
    Some(json!({
        "id": v["id"].as_str().unwrap_or_default(),
        "name": v["name"].as_str().unwrap_or_default(),
        "tier": v["tierLabel"].as_str().unwrap_or_default(),
        "today_tokens": today_tokens,
        "today_prompts": today_prompts,
        "total_prompts": total_prompts,
        "total_sessions": total_sessions,
        "input_tokens": input_sum,
        "output_tokens": output_sum,
        "cache_read_tokens": cache_sum,
        "total_tokens": total_agent_tokens,
        "limits": v["limits"],
        "models": models,
    }))
}

fn read_omarchy_usage() -> Value {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state"))
        });
    let Some(dir) = base.map(|b| b.join("omarchy/agents/usage")) else {
        return json!({ "available": false, "agents": [], "totals": {} });
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return json!({ "available": false, "agents": [], "totals": {} });
    };
    let mut agents = Vec::new();
    let mut totals = UsageTotals::default();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(v) = read_usage_json(&path) else {
            continue;
        };
        if let Some(row) = usage_row(&v, &mut totals) {
            agents.push(row);
        }
    }
    agents.sort_by_key(|a| {
        std::cmp::Reverse(
            a["today_tokens"]
                .as_u64()
                .unwrap_or(0)
                .max(a["total_tokens"].as_u64().unwrap_or(0) / 100),
        )
    });
    json!({
        "available": true,
        "agents": agents,
        "totals": {
            "today_tokens": totals.today_tokens,
            "today_prompts": totals.today_prompts,
            "input_tokens": totals.input,
            "output_tokens": totals.output,
            "cache_read_tokens": totals.cache_read,
            "total_tokens": totals.input.saturating_add(totals.output).saturating_add(totals.cache_read),
        }
    })
}

fn inspect_magic_project_file(path: &Path) -> (usize, usize, Option<u64>) {
    let doc = read_usage_json(path).unwrap_or_default();
    let assets_count = doc["assets"].as_array().map_or(0, Vec::len);
    let dir = path.parent().unwrap_or(path);
    let board = read_usage_json(&dir.join("storyboard/movie-board.json")).unwrap_or_default();
    let storyboard_pages = board["page_plan"]
        .as_array()
        .or_else(|| board["pages"].as_array())
        .or_else(|| board["scenes"].as_array())
        .map_or(0, Vec::len);
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    (assets_count, storyboard_pages, modified)
}

fn collect_recent_project_files(dir: &Path) -> Vec<Value> {
    let mut found = Vec::new();
    let subdirs = [
        "",
        "assets",
        "storyboard",
        "hyperframes",
        "exports",
        "audio",
    ];
    for sub in subdirs {
        let target = if sub.is_empty() {
            dir.to_path_buf()
        } else {
            dir.join(sub)
        };
        let Ok(entries) = std::fs::read_dir(&target) else {
            continue;
        };
        for entry in entries.flatten().take(40) {
            let p = entry.path();
            if p.is_file()
                && let Ok(meta) = std::fs::metadata(&p)
            {
                let secs = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let rel = p
                    .strip_prefix(dir)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                found.push((secs, meta.len(), rel, p.to_string_lossy().to_string()));
            }
        }
    }
    found.sort_by_key(|item| std::cmp::Reverse(item.0));
    found
        .into_iter()
        .take(8)
        .map(|(secs, bytes, rel, full)| {
            json!({
                "rel_path": rel,
                "path": full,
                "bytes": bytes,
                "modified_unix_secs": secs,
            })
        })
        .collect()
}

fn enrich_catalog_projects(s: &mut Session) -> Vec<Value> {
    let Ok(cat) = crate::project_catalog::catalog(s, &json!({})) else {
        return Vec::new();
    };
    let Some(items) = cat["projects"].as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let mut row = item.clone();
            if let Some(p) = item["path"].as_str().map(Path::new)
                && p.is_file()
            {
                let (assets_count, pages, modified) = inspect_magic_project_file(p);
                row["assets_count"] = json!(assets_count);
                row["storyboard_pages"] = json!(pages);
                row["modified_unix_secs"] = json!(modified);
            }
            row
        })
        .collect()
}

fn resolve_activity_project(stream: &[Value]) -> Option<std::path::PathBuf> {
    for ev in stream {
        let Some(raw) = ev["project"].as_str() else {
            continue;
        };
        let p = std::path::PathBuf::from(raw);
        if p.is_file() {
            return Some(p);
        }
        if p.is_dir()
            && let Ok(entries) = std::fs::read_dir(&p)
        {
            for e in entries.flatten() {
                if e.path().extension().is_some_and(|ext| ext == "magic") {
                    return Some(e.path());
                }
            }
        }
    }
    None
}

fn read_project_analytics(s: &mut Session) -> Value {
    let turns = crate::agent::log_read(s);
    let list = turns.as_array().cloned().unwrap_or_default();
    let mut user_turns = 0usize;
    let mut assistant_turns = 0usize;
    let mut total_chars = 0usize;
    for item in &list {
        match item["role"].as_str().unwrap_or_default() {
            "user" => user_turns += 1,
            "assistant" => assistant_turns += 1,
            _ => {}
        }
        total_chars += item["text"].as_str().map(str::len).unwrap_or(0);
    }
    let catalog = enrich_catalog_projects(s);
    let activity_stream = crate::activity::recent_stream(30);
    let active_path = resolve_activity_project(&activity_stream)
        .or_else(|| s.project_path.clone())
        .or_else(|| {
            catalog
                .iter()
                .find(|p| p["exists"] == true)
                .and_then(|p| p["path"].as_str())
                .map(std::path::PathBuf::from)
        });
    let (assets_count, storyboard_pages, modified_unix_secs, recent_files) =
        if let Some(ref p) = active_path {
            let (ac, sp, mod_s) = inspect_magic_project_file(p);
            let rf = p
                .parent()
                .map(collect_recent_project_files)
                .unwrap_or_default();
            (ac.max(s.project_assets.len()), sp, mod_s, rf)
        } else {
            (s.project_assets.len(), 0, None, Vec::new())
        };
    json!({
        "project": active_path,
        "session_attached": s.project_path.is_some(),
        "assets_count": assets_count,
        "storyboard_pages": storyboard_pages,
        "modified_unix_secs": modified_unix_secs,
        "recent_files": recent_files,
        "activity_stream": activity_stream,
        "catalog": catalog,
        "user_turns": user_turns,
        "assistant_turns": assistant_turns,
        "total_turns": list.len(),
        "total_chars": total_chars,
        "estimated_tokens": (total_chars / 4) as u64,
    })
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
}

fn read_open_github_issues(root: &Path, force_refresh: bool) -> Vec<Value> {
    let cache = root.join("target/analytics/github-issues.json");
    let fresh = !force_refresh
        && std::fs::metadata(&cache)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() < 60);
    if fresh && let Some(v) = read_usage_json(&cache).and_then(|v| v.as_array().cloned()) {
        return v;
    }
    let Ok(out) = std::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "30",
            "--json",
            "number,title,url,body",
        ])
        .current_dir(root)
        .output()
    else {
        return read_usage_json(&cache)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
    };
    if !out.status.success() {
        return read_usage_json(&cache)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
    }
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
        let _ = std::fs::write(&cache, &out.stdout);
    }
    serde_json::from_slice::<Vec<Value>>(&out.stdout).unwrap_or_default()
}

fn read_herdr_agents() -> Vec<Value> {
    let Ok(out) = std::process::Command::new("herdr")
        .args(["agent", "list"])
        .output()
    else {
        return Vec::new();
    };
    serde_json::from_slice::<Value>(&out.stdout)
        .ok()
        .and_then(|v| v["result"]["agents"].as_array().cloned())
        .unwrap_or_default()
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(idx, _)| {
        let before_ok = idx == 0
            || !haystack[..idx]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        let after_idx = idx + needle.len();
        let after_ok = after_idx >= haystack.len()
            || !haystack[after_idx..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        before_ok && after_ok
    })
}

fn match_agent_issues(name: &str, issues: &[Value]) -> Vec<Value> {
    if name.is_empty() || name == "magic-studio-app" {
        return Vec::new();
    }
    let needle = name.to_ascii_lowercase();
    issues
        .iter()
        .filter(|issue| {
            let title = issue["title"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let body = issue["body"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if needle == "main" {
                body.contains("`main`")
            } else {
                contains_word(&title, &needle) || contains_word(&body, &needle)
            }
        })
        .take(4)
        .map(|issue| {
            json!({
                "number": issue["number"],
                "title": issue["title"],
                "url": issue["url"],
            })
        })
        .collect()
}

fn parse_worktree_blocks(stdout: &str) -> Vec<(String, String, String)> {
    let mut list = Vec::new();
    for block in stdout.trim().split("\n\n") {
        let mut wt_path = String::new();
        let mut head = String::new();
        let mut branch = String::new();
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                wt_path = p.trim().to_string();
            } else if let Some(h) = line.strip_prefix("HEAD ") {
                head = h.trim().chars().take(8).collect();
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                branch = b.trim().to_string();
            }
        }
        if !wt_path.is_empty() {
            list.push((wt_path, head, branch));
        }
    }
    list
}

fn worktree_git_summary(wt_path: &str) -> (usize, Option<String>) {
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .current_dir(wt_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or(0, |o| String::from_utf8_lossy(&o.stdout).lines().count());
    let commit = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(wt_path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    (dirty, commit)
}

fn read_agent_worktrees(root: &Path, force_refresh: bool) -> Vec<Value> {
    let Ok(out) = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(root)
        .output()
    else {
        return Vec::new();
    };
    let issues = read_open_github_issues(root, force_refresh);
    let herdr = read_herdr_agents();
    let mut rows = Vec::new();
    for (wt_path, head, branch) in parse_worktree_blocks(&String::from_utf8_lossy(&out.stdout)) {
        let name = Path::new(&wt_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| branch.clone());
        let agent_name = if name == "magic-studio-app" {
            "main".to_string()
        } else {
            name
        };
        let live = herdr.iter().find(|a| {
            a["cwd"].as_str() == Some(wt_path.as_str())
                || a["name"].as_str() == Some(agent_name.as_str())
        });
        let matched_issues = match_agent_issues(&agent_name, &issues);
        let (dirty_files, last_commit) = worktree_git_summary(&wt_path);
        rows.push(json!({
            "agent": agent_name,
            "path": wt_path,
            "branch": branch,
            "head": head,
            "dirty_files": dirty_files,
            "last_commit": last_commit,
            "status": live.and_then(|a| a["agent_status"].as_str()).unwrap_or("worktree"),
            "pane_id": live.and_then(|a| a["pane_id"].as_str()),
            "issues": matched_issues,
        }));
    }
    rows.sort_by_key(|r| {
        let is_bunzen = r["agent"].as_str() == Some("bunzen");
        let is_main = r["agent"].as_str() == Some("main");
        let is_live = r["pane_id"].is_string();
        let issue_count = r["issues"].as_array().map_or(0, Vec::len);
        (
            !is_bunzen,
            !is_main,
            !is_live,
            std::cmp::Reverse(issue_count),
        )
    });
    rows
}
