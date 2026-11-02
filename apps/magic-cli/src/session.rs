//! One caller's binding (project, and the window serving it) and the single route every
//! command takes: the pinned window, else the window that has the project open, else the
//! engine in this process. The binding file never touches the `.magic` project itself.
use crate::{project, studio};
use magicstudio_engine::headless;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Default, Serialize, Deserialize)]
struct Binding {
    project_path: Option<String>,
    studio_endpoint: Option<String>,
    studio_instance: Option<String>,
}

/// Commands that take no project, so a pinned project is not added to them.
const GLOBAL: &[&str] = &[
    "project.new",
    "project.intake",
    "project.catalog",
    "project.root",
    "project.migrate",
    "project.identity",
    "command.list",
    "app.doctor",
];

/// Where this caller's binding lives: per Herdr pane for agents, per user otherwise.
pub fn default_file() -> PathBuf {
    if let Some(path) = std::env::var_os("MAGIC_SESSION_FILE") {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(|r| PathBuf::from(r).join("magicstudio"))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".magic")))
        .unwrap_or_else(|| PathBuf::from(".magic"));
    match std::env::var("HERDR_PANE_ID") {
        Ok(pane)
            if !pane.is_empty()
                && pane
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_:".contains(c)) =>
        {
            base.join("agent-studios").join(pane).join("session.json")
        }
        _ => base.join("session.json"),
    }
}

pub struct Session {
    file: PathBuf,
    bind: Binding,
    engine: Option<magicstudio_engine::Session>,
    origin: &'static str,
}

impl Session {
    pub fn load(file: Option<PathBuf>, origin: &'static str) -> Result<Self, String> {
        let file = file.unwrap_or_else(default_file);
        let bind = if file.is_file() {
            let text = std::fs::read_to_string(&file).map_err(|e| {
                format!(
                    "Cannot read session binding {}: {e}; no target selected",
                    file.display()
                )
            })?;
            serde_json::from_str(&text).map_err(|e| {
                format!(
                    "Cannot read session binding {}: {e}; no target selected",
                    file.display()
                )
            })?
        } else {
            Binding::default()
        };
        Ok(Self {
            file,
            bind,
            engine: None,
            origin,
        })
    }

    fn save(&self) -> Result<(), String> {
        let dir = self.file.parent().ok_or("session file has no folder")?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        // Write beside, then rename: a concurrent reader sees the old or the new binding.
        let tmp = dir.join(format!(".session.{}.tmp", std::process::id()));
        let text = serde_json::to_string_pretty(&self.bind).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.file).map_err(|e| e.to_string())
    }

    pub fn status(&self) -> Value {
        json!({"project": self.bind.project_path, "endpoint": self.bind.studio_endpoint, "instance": self.bind.studio_instance})
    }

    pub fn clear(&mut self) -> Result<(), String> {
        self.engine = None;
        self.bind = Binding::default();
        match std::fs::remove_file(&self.file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }

    fn bind_window(&mut self, live: &Value) -> Result<(), String> {
        self.bind.studio_endpoint = live["endpoint"].as_str().map(str::to_owned);
        self.bind.studio_instance = live["instance"].as_str().map(str::to_owned);
        self.bind.project_path = live["active_project"].as_str().map(|p| {
            std::fs::canonicalize(p).map_or(p.to_owned(), |c| c.to_string_lossy().into_owned())
        });
        self.save()
    }

    /// Open or reuse a window and bind this caller to it. The project moves from this
    /// process's engine to the window, so the engine is dropped first.
    pub fn open_studio(&mut self, raw: Option<&str>) -> Result<Value, String> {
        let target = raw.map(project::resolve).transpose()?;
        self.engine = None;
        let live = studio::open(target.as_deref().and_then(Path::to_str))?;
        self.bind_window(&live)?;
        Ok(live)
    }

    fn headless(&mut self, id: &str, params: Value, dry_run: bool) -> Result<Value, String> {
        let engine = self
            .engine
            .get_or_insert_with(magicstudio_engine::Session::user_session);
        headless::dispatch(engine, id, params, dry_run, true)
    }

    /// The target project for this command, written into its params.
    fn route(&self, id: &str, req: &mut Value) -> Result<(bool, Option<String>), String> {
        let selecting = id == "project.open";
        let field = if selecting { "path" } else { "project" };
        let mut target = req[field].as_str().map(str::to_owned);
        if target.is_none() && !selecting && !GLOBAL.contains(&id) {
            target = self.bind.project_path.clone();
        }
        if let Some(raw) = &target {
            let resolved = project::resolve(raw)?.to_string_lossy().into_owned();
            req[field] = json!(resolved);
            target = Some(resolved);
        }
        Ok((selecting, target))
    }

    fn unavailable(&self) -> String {
        let live = self.live_for_pinned();
        let base = "Pinned Studio is unavailable; no other window or file writer was selected. ";
        if live.is_empty() {
            return format!(
                "{base}No live window has this project open. Use project open or studio open to explicitly reattach."
            );
        }
        let found: Vec<String> = live
            .iter()
            .map(|x| {
                format!(
                    "pid {} ({})",
                    x["identity"]["pid"],
                    x["instance"].as_str().unwrap_or("?")
                )
            })
            .collect();
        format!(
            "{base}A live window has this project open: {}. Reattach with: magic-cli studio open {}",
            found.join(", "),
            self.bind.project_path.as_deref().unwrap_or("")
        )
    }

    fn live_for_pinned(&self) -> Vec<Value> {
        let Some(pinned) = self.bind.project_path.as_deref() else {
            return Vec::new();
        };
        studio::discover()["sessions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["canonical_project"] == pinned || s["active_project"] == pinned)
            .cloned()
            .collect()
    }

    /// The pinned window; when it restarted and exactly one window has the project, follow it.
    fn call_pinned(&mut self, id: &str, req: Value, target: Option<&str>) -> Result<Value, String> {
        if let (Some(t), Some(p)) = (target, self.bind.project_path.as_deref())
            && t != p
        {
            return Err("Pinned Studio targets a different project; explicitly open the intended project first".into());
        }
        let endpoint = self.bind.studio_endpoint.clone().unwrap_or_default();
        let project = self.bind.project_path.clone();
        let first = studio::call(
            &endpoint,
            id,
            req.clone(),
            project.as_deref(),
            self.bind.studio_instance.as_deref(),
        );
        match first {
            Err(e)
                if e.starts_with("stale endpoint")
                    || !Path::new(endpoint.trim_start_matches("unix:")).exists() =>
            {
                let live = self.live_for_pinned();
                let [only] = live.as_slice() else {
                    return Err(self.unavailable());
                };
                self.bind_window(only)?;
                studio::call(
                    only["endpoint"].as_str().unwrap_or_default(),
                    id,
                    req,
                    project.as_deref(),
                    only["instance"].as_str(),
                )
            }
            other => other,
        }
    }

    /// The window that has the project open, else the engine in this process.
    fn call_live(
        &mut self,
        id: &str,
        req: Value,
        target: Option<&str>,
        dry_run: bool,
    ) -> Result<(Value, bool), String> {
        let live = match target {
            Some(t) if Path::new(t).exists() => {
                magicstudio_engine::studio_sessions::for_project(Path::new(t))?
            }
            _ => None,
        };
        if let Some(live) = live {
            self.engine = None;
            let result = studio::call(
                live["endpoint"].as_str().unwrap_or_default(),
                id,
                req,
                target,
                live["instance"].as_str(),
            )?;
            if !dry_run {
                self.bind_window(&live)?;
            }
            return Ok((result, true));
        }
        let window_only = id.starts_with("ui.") || id == "app.quit";
        if window_only && studio::agent_caller() {
            return Err("No Studio window is bound to this agent; run 'magic-cli studio open /absolute/Project.magic' first (agent windows open on the agent workspace, never the user's window).".into());
        }
        if window_only {
            return Ok((studio::call_auto(id, req, target)?, false));
        }
        Ok((self.headless(id, req, dry_run)?, false))
    }

    fn remember(&mut self, id: &str, result: &Value) -> Result<(), String> {
        match id {
            "project.open" | "project.new" | "project.intake" | "project.save" => {
                if let Some(p) = result["project"].as_str() {
                    self.bind.project_path = Some(
                        std::fs::canonicalize(p)
                            .map_or(p.to_owned(), |c| c.to_string_lossy().into_owned()),
                    );
                    self.save()?;
                }
            }
            "project.close" => {
                self.bind.project_path = None;
                self.save()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// The request as sent: origin and dry-run marked.
    fn prepare(&self, params: Value, dry_run: bool) -> (Value, bool) {
        let mut req = if params.is_object() {
            params
        } else {
            json!({})
        };
        if req.get("_origin").is_none() {
            req["_origin"] = json!(self.origin);
        }
        let dry_run = dry_run || req["_dry_run"] == true;
        if dry_run {
            req["_dry_run"] = json!(true);
        }
        (req, dry_run)
    }

    /// The pinned window, else the project's window or this process's engine.
    fn deliver(
        &mut self,
        id: &str,
        req: Value,
        selecting: bool,
        target: Option<&str>,
        dry_run: bool,
    ) -> Result<Value, String> {
        if self.bind.studio_endpoint.is_some() && !selecting {
            return self.call_pinned(id, req, target);
        }
        let (result, live) = self.call_live(id, req, target, dry_run)?;
        // Selecting a project headlessly releases any earlier window pin.
        if selecting && !live && !dry_run && result["ok"] != false {
            self.bind.studio_endpoint = None;
            self.bind.studio_instance = None;
        }
        Ok(result)
    }

    /// Run one registry command for this caller and keep the binding it implies.
    pub fn execute(&mut self, id: &str, params: Value, dry_run: bool) -> Result<Value, String> {
        let (mut req, dry_run) = self.prepare(params, dry_run);
        // Discovery and isolated observers never change the caller's target or writer.
        if matches!(
            id,
            "command.list" | "backstage.observe" | "backstage.changes"
        ) {
            return headless::dispatch(
                &mut magicstudio_engine::Session::user_session(),
                id,
                req,
                dry_run,
                false,
            );
        }
        let (selecting, target) = self.route(id, &mut req)?;
        let result = self.deliver(id, req, selecting, target.as_deref(), dry_run)?;
        if result["ok"] == false {
            return Err(result["error"]
                .as_str()
                .map_or_else(|| result.to_string(), str::to_owned));
        }
        if !dry_run {
            self.remember(id, &result)?;
        }
        Ok(result)
    }
}
