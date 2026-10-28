use crate::Event;
use notify::{RecursiveMode, Watcher};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
    },
    thread,
};

pub fn watch_directory(target: &Path) -> Result<PathBuf, String> {
    let mut dir = target.parent().ok_or("Notification path has no parent")?;
    while !dir.is_dir() {
        dir = dir.parent().ok_or("No accessible notification parent")?;
    }
    Ok(dir.to_path_buf())
}

pub fn token(status: &Value) -> Option<(String, String)> {
    if status["ok"] != true || status["available"] != true {
        return None;
    }
    let path = status["path"].as_str()?;
    if !Path::new(path).is_absolute() {
        return None;
    }
    let token = status["token"].as_str().filter(|s| !s.is_empty())?;
    Some((path.to_owned(), token.to_owned()))
}

pub fn watch(
    paths: &[PathBuf],
    tx: SyncSender<Event>,
    dirty: Arc<AtomicBool>,
) -> Result<notify::RecommendedWatcher, String> {
    let targets = paths.to_vec();
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<notify::Event>| match result {
            Ok(e)
                if !matches!(e.kind, notify::EventKind::Access(_))
                    && (e.need_rescan()
                        || e.paths
                            .iter()
                            .any(|p| targets.iter().any(|t| p == t || t.starts_with(p)))) =>
            {
                dirty.store(true, Ordering::Release);
                let _ = tx.try_send(Event::Changed);
            }
            Err(e) => {
                dirty.store(true, Ordering::Release);
                let _ = tx.try_send(Event::Notice(format!("Watcher: {e}; press R to refresh")));
            }
            _ => {}
        })
        .map_err(|e| e.to_string())?;
    let mut watched = std::collections::HashSet::new();
    for target in paths {
        let parent = watch_directory(target)?;
        if watched.insert(parent.clone()) {
            watcher
                .watch(&parent, RecursiveMode::NonRecursive)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(watcher)
}

pub fn agents(socket: PathBuf, tx: SyncSender<Event>) {
    thread::spawn(move || {
        loop {
            if let Err(e) = subscribe(&socket, &tx) {
                let _ = tx.send(Event::Agents(Err(e)));
                break;
            }
        }
    });
}

// Herdr JSONL API, not MCP. Only agent.list and events.subscribe are ever sent.
#[cfg(unix)]
fn subscribe(path: &Path, tx: &SyncSender<Event>) -> Result<(), String> {
    use std::os::unix::net::UnixStream;
    let mut stream =
        UnixStream::connect(path).map_err(|e| format!("Herdr observations unavailable: {e}"))?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let send = |s: &mut UnixStream, id: &str, method: &str, params: Value| -> Result<(), String> {
        writeln!(s, "{}", json!({"id":id,"method":method,"params":params}))
            .map_err(|e| e.to_string())
    };
    let list = agent_list(path)?;
    let mut agents = list["result"]["agents"]
        .as_array()
        .ok_or("Herdr did not return agents")?
        .clone();
    let mut subscriptions = vec![
        json!({"type":"pane.agent_detected"}),
        json!({"type":"pane.closed"}),
        json!({"type":"pane.exited"}),
    ];
    for agent in &agents {
        if let Some(id) = agent["pane_id"].as_str() {
            subscriptions.push(json!({"type":"pane.agent_status_changed","pane_id":id}));
        }
    }
    send(
        &mut stream,
        "backstage-subscribe",
        "events.subscribe",
        json!({"subscriptions":subscriptions}),
    )?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    let mut acknowledgment = Vec::new();
    std::io::Read::by_ref(&mut reader)
        .take(4 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut acknowledgment)
        .map_err(|e| e.to_string())?;
    let acknowledgment: Value =
        serde_json::from_slice(&acknowledgment).map_err(|e| e.to_string())?;
    if acknowledgment["result"]["type"] != "subscription_started" {
        return Err(format!(
            "Herdr did not acknowledge observation subscription: {acknowledgment}"
        ));
    }
    stream.set_read_timeout(None).map_err(|e| e.to_string())?;
    let ids = |rows: &[Value]| {
        rows.iter()
            .filter_map(|v| v["pane_id"].as_str().map(str::to_owned))
            .collect::<std::collections::BTreeSet<_>>()
    };
    let subscribed_ids = ids(&agents);
    // Each non-streaming request uses its own connection. Herdr closes it after the reply.
    let initial = agent_list(path)?;
    if let Some(rows) = initial["result"]["agents"].as_array() {
        agents = rows.clone();
    }
    if ids(&agents) != subscribed_ids {
        return Ok(());
    }
    tx.send(Event::Agents(Ok(agents.clone())))
        .map_err(|e| e.to_string())?;
    loop {
        let mut bytes = Vec::new();
        let size = std::io::Read::by_ref(&mut reader)
            .take(4 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if size == 0 {
            return Err("Herdr disconnected; performer state is unknown".into());
        }
        if size > 4 * 1024 * 1024 {
            return Err("Herdr response too large".into());
        }
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !v["error"].is_null() {
            return Err(format!("Herdr: {}", v["error"]));
        }
        if v["data"]["type"] == "pane_agent_status_changed" {
            let snapshot = agent_list(path)?;
            agents = snapshot["result"]["agents"]
                .as_array()
                .ok_or("Herdr did not return agents")?
                .clone();
            tx.send(Event::Agents(Ok(agents.clone())))
                .map_err(|e| e.to_string())?;
        } else if v.get("event").is_some() {
            // Reconnect only for roster changes, to install subscriptions for new pane IDs.
            return Ok(());
        }
    }
}

#[cfg(unix)]
fn agent_list(path: &Path) -> Result<Value, String> {
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    writeln!(
        stream,
        "{}",
        json!({"id":"backstage-list","method":"agent.list","params":{}})
    )
    .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take(4 * 1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Herdr response too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn subscribe(_path: &Path, _tx: &SyncSender<Event>) -> Result<(), String> {
    Err("Herdr socket observations need Unix".into())
}
