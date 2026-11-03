//! App documents a view opens. Magic saves only `.magic`; the app saves its own document. A view
//! opens its document in the installed app with a control port, Magic's Save asks that app to
//! save, and leaving the view saves and closes it, all through the app's own CLI.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

// ---- The apps and the CLI calls this file makes -------------------------------------------

/// DeckCraft: `deckcraft --control PORT FILE`; `deckcraft-cli app --port PORT COMMAND`.
#[derive(Debug)]
pub struct DeckCraft;
/// EffectCraft: `effectcraft --control PORT FILE`; `effectcraft-cli --bridge PORT exec COMMAND`.
#[derive(Debug)]
pub struct EffectCraft;
/// FilmCraft: `filmcraft --control PORT FILE` with FILMCRAFT_CONTROL_TOKEN;
/// `filmcraft-cli --bridge 127.0.0.1:PORT exec COMMAND`. Its CLI has no quit command.
#[derive(Debug)]
pub struct FilmCraft;

pub static DECKCRAFT: DeckCraft = DeckCraft;
pub static EFFECTCRAFT: EffectCraft = EffectCraft;
pub static FILMCRAFT: FilmCraft = FilmCraft;

impl NativeApp for DeckCraft {
    fn name(&self) -> &'static str {
        "deckcraft"
    }
    fn bridge(&self, port: u16, command: &str) -> Vec<String> {
        args(&["app", "--port", &port.to_string(), command])
    }
    fn quit(&self, port: u16) -> Option<Vec<String>> {
        Some(args(&[
            "app",
            "--port",
            &port.to_string(),
            "--method",
            "app.quit",
        ]))
    }
}

impl NativeApp for EffectCraft {
    fn name(&self) -> &'static str {
        "effectcraft"
    }
    fn bridge(&self, port: u16, command: &str) -> Vec<String> {
        args(&["--bridge", &port.to_string(), "exec", command])
    }
    fn quit(&self, port: u16) -> Option<Vec<String>> {
        Some(self.bridge(port, "app.quit"))
    }
}

impl NativeApp for FilmCraft {
    fn name(&self) -> &'static str {
        "filmcraft"
    }
    fn bridge(&self, port: u16, command: &str) -> Vec<String> {
        args(&["--bridge", &format!("127.0.0.1:{port}"), "exec", command])
    }
    fn token_env(&self) -> Option<&'static str> {
        Some("FILMCRAFT_CONTROL_TOKEN")
    }
}

// ---- Shared behaviour ----------------------------------------------------------------------

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| (*a).to_owned()).collect()
}

/// An installed app with a control port; `<name>-cli` reaches the running window.
pub trait NativeApp: Send + Sync + std::fmt::Debug {
    /// The launcher command; its CLI is `<name>-cli`.
    fn name(&self) -> &'static str;
    /// CLI arguments that run `command` in the app started with `--control port`.
    fn bridge(&self, port: u16, command: &str) -> Vec<String>;
    /// CLI arguments that quit the app; None ends the process Magic started, after its save.
    fn quit(&self, _port: u16) -> Option<Vec<String>> {
        None
    }
    /// The variable that carries a control token shared by the app and its CLI, if it needs one.
    fn token_env(&self) -> Option<&'static str> {
        None
    }
}

/// A document Magic opened in its app.
#[derive(Debug)]
pub struct OpenDocument {
    app: &'static dyn NativeApp,
    file: PathBuf,
    port: u16,
    token: Option<String>,
    child: Child,
}

impl OpenDocument {
    fn cli(&self, arguments: Vec<String>) -> Result<()> {
        let tool = format!("{}-cli", self.app.name());
        let mut command = Command::new(crate::workspace::resolve_tool(&tool));
        command.args(arguments);
        if let (Some(var), Some(token)) = (self.app.token_env(), &self.token) {
            command.env(var, token);
        }
        let out = command
            .output()
            .map_err(|e| EngineError::ExternalTool(tool.clone(), e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(EngineError::ExternalTool(
                tool,
                String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            ))
        }
    }

    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// The app saves its document.
    pub fn save(&self) -> Result<()> {
        self.cli(self.app.bridge(self.port, "file.save"))
    }

    /// Save, then quit the app through its CLI (or end the process Magic started).
    pub fn close(mut self) -> Result<()> {
        self.save()?;
        match self.app.quit(self.port) {
            Some(quit) => self.cli(quit)?,
            None => {
                let pid = i32::try_from(self.child.id()).ok();
                if let Some(group) = pid.and_then(rustix::process::Pid::from_raw) {
                    let _ =
                        rustix::process::kill_process_group(group, rustix::process::Signal::TERM);
                }
            }
        }
        // Reap it once it has gone, so it does not linger as a zombie.
        std::thread::spawn(move || self.child.wait());
        Ok(())
    }

    fn summary(&self) -> Value {
        json!({"app": self.app.name(), "file": self.file, "port": self.port})
    }
}

fn free_port() -> Result<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(EngineError::from)
}

/// Open `file` in `app` for this session's view; a document already open is left as it is.
pub fn open(s: &mut Session, app: &'static dyn NativeApp, file: &Path) -> Result<Value> {
    s.open_documents.retain_mut(OpenDocument::running);
    if let Some(open) = s.open_documents.iter().find(|d| d.file == file) {
        return Ok(json!({"ok": true, "already_open": true, "document": open.summary()}));
    }
    let port = free_port()?;
    let token = match app.token_env() {
        Some(_) => {
            use std::io::Read;
            let mut bytes = [0u8; 32];
            std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
            Some(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
        }
        None => None,
    };
    let mut command = Command::new(crate::workspace::resolve_tool(app.name()));
    command
        .args(["--control", &port.to_string()])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let (Some(var), Some(token)) = (app.token_env(), &token) {
        command.env(var, token);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command
        .spawn()
        .map_err(|e| EngineError::ExternalTool(app.name().into(), e.to_string()))?;
    let document = OpenDocument {
        app,
        file: file.to_path_buf(),
        port,
        token,
        child,
    };
    let summary = document.summary();
    s.open_documents.push(document);
    Ok(json!({"ok": true, "document": summary}))
}

/// Each open app saves its document (Magic's Save).
pub fn save_all(s: &mut Session) -> Vec<Value> {
    s.open_documents.retain_mut(OpenDocument::running);
    s.open_documents
        .iter()
        .map(|d| match d.save() {
            Ok(()) => json!({"saved": true, "document": d.summary()}),
            Err(e) => json!({"saved": false, "document": d.summary(), "error": e.to_string()}),
        })
        .collect()
}

/// Save and close every app this session opened (leaving a view).
pub fn close_all(s: &mut Session) -> Vec<Value> {
    s.open_documents.retain_mut(OpenDocument::running);
    std::mem::take(&mut s.open_documents)
        .into_iter()
        .map(|d| {
            let summary = d.summary();
            match d.close() {
                Ok(()) => json!({"closed": true, "document": summary}),
                Err(e) => json!({"closed": false, "document": summary, "error": e.to_string()}),
            }
        })
        .collect()
}
