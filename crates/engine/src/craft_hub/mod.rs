//! CraftHub: Adaptive transport router connecting Magic Studio to the ArtCraft suite.
//!
//! Reference:
//! - <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/apps/filmcraft/src/control_server.rs>
//! - <https://github.com/storytold/effectcraft/blob/e1de87e8c3efaf02293728716d296342e0383a81/apps/effectcraft/src/control_server.rs>
//! - <https://github.com/storytold/designcraft/blob/4f3bb5340eeee5f747f7ee2a6905d3c990831d22/apps/designcraft/src/control_server.rs>
//! - <https://github.com/storytold/photocraft/blob/a3ea9a4277e1e1067acbb30d592c56e9545b949d/apps/photocraft/src/control_server.rs>
//! - <https://github.com/storytold/vectorcraft/blob/a479c8baa1b783b5002f8ebd02391aa3395b5b49/apps/vectorcraft/src/control_server.rs>

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use crate::{EngineError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftTarget {
    Film,
    Photo,
    Effect,
    Vector,
    Design,
    Deck,
    Texture,
}

impl CraftTarget {
    /// The canonical application name (e.g. `filmcraft`).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Film => "filmcraft",
            Self::Photo => "photocraft",
            Self::Effect => "effectcraft",
            Self::Vector => "vectorcraft",
            Self::Design => "designcraft",
            Self::Deck => "deckcraft",
            Self::Texture => "texturecraft",
        }
    }

    /// The companion CLI binary name on PATH (e.g. `filmcraft-cli`).
    pub fn cli_name(&self) -> &'static str {
        match self {
            Self::Film => "filmcraft-cli",
            Self::Photo => "photocraft-cli",
            Self::Effect => "effectcraft-cli",
            Self::Vector => "vectorcraft-cli",
            Self::Design => "designcraft-cli",
            Self::Deck => "deckcraft-cli",
            Self::Texture => "texturecraft-cli",
        }
    }

    /// Standard default control port for the application (`9876` is reserved for `magic-tauri`).
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Film => 9875,
            Self::Design => 9877,
            Self::Effect => 9878,
            Self::Photo => 9879,
            Self::Vector => 9880,
            Self::Deck => 9881,
            Self::Texture => 9882,
        }
    }

    /// Environment variable used to override the default port.
    pub fn port_env(&self) -> &'static str {
        match self {
            Self::Film => "FILMCRAFT_CONTROL_PORT",
            Self::Design => "DESIGNCRAFT_CONTROL_PORT",
            Self::Effect => "EFFECTCRAFT_CONTROL_PORT",
            Self::Photo => "PHOTOCRAFT_CONTROL_PORT",
            Self::Vector => "VECTORCRAFT_CONTROL_PORT",
            Self::Deck => "DECKCRAFT_CONTROL_PORT",
            Self::Texture => "TEXTURECRAFT_CONTROL_PORT",
        }
    }

    /// Environment variable holding the authentication token, if required.
    pub fn token_env(&self) -> Option<&'static str> {
        match self {
            Self::Film => Some("FILMCRAFT_CONTROL_TOKEN"),
            Self::Photo => Some("PHOTOCRAFT_CONTROL_TOKEN"),
            _ => None,
        }
    }
}

pub struct CraftHub;

enum TcpOutcome {
    Unreachable,
    DifferentDocumentOpen,
    Connected(Result<Value>),
}

impl CraftHub {
    /// Execute a command against a Craft target application.
    ///
    /// 1. Connects to the live app via loopback TCP (`--control`). When `doc` is given, verifies the
    ///    live app has that exact canonical document open before dispatching.
    /// 2. Falls back to `<app>-cli` ONLY when TCP connect is refused/unreachable or the live app is
    ///    editing a different document — never after a connected live command errors or times out.
    pub fn execute(target: CraftTarget, doc: Option<&Path>, command: &str, params: &Value) -> Result<Value> {
        let addr = target_addr(target)?;
        let token = target.token_env().and_then(|var| std::env::var(var).ok());
        match call_tcp_at(target, addr, token.as_deref(), doc, command, params, false, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => res,
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => call_cli_with_save(target, doc, command, params, false),
        }
    }

    /// Execute a mutating command against `doc` and persist it (`file.save` / `app.save` on a live
    /// `--control` session, or `--save` / `--out` via `<app>-cli` when not open live).
    pub fn execute_and_save(target: CraftTarget, doc: &Path, command: &str, params: &Value) -> Result<Value> {
        let addr = target_addr(target)?;
        let token = target.token_env().and_then(|var| std::env::var(var).ok());
        match call_tcp_at(target, addr, token.as_deref(), Some(doc), command, params, true, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => res,
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => call_cli_with_save(target, Some(doc), command, params, true),
        }
    }

    /// Send `command` to the live app ONLY if it is running AND currently has `doc` open.
    pub fn try_live(target: CraftTarget, doc: &Path, command: &str, params: &Value, save: bool) -> Option<Result<Value>> {
        let addr = target_addr(target).ok()?;
        let token = target.token_env().and_then(|var| std::env::var(var).ok());
        match call_tcp_at(target, addr, token.as_deref(), Some(doc), command, params, save, Duration::from_millis(250)) {
            TcpOutcome::Connected(res) => Some(res),
            TcpOutcome::Unreachable | TcpOutcome::DifferentDocumentOpen => None,
        }
    }
}

fn target_addr(target: CraftTarget) -> Result<SocketAddr> {
    let port = std::env::var(target.port_env()).ok().and_then(|p| p.parse::<u16>().ok()).unwrap_or_else(|| target.default_port());
    format!("127.0.0.1:{port}").parse().map_err(|e| EngineError::InvalidParams(format!("invalid control address: {e}")))
}

fn same_canonical_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(ca), Ok(cb)) if ca == cb)
}

struct ControlConn {
    target: CraftTarget,
    inline_token: Option<String>,
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl ControlConn {
    fn send_raw(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let mut req_obj = serde_json::Map::new();
        req_obj.insert("id".into(), json!(id));
        if let Some(tok) = &self.inline_token {
            req_obj.insert("token".into(), json!(tok));
        }
        req_obj.insert("method".into(), json!(method));
        req_obj.insert("params".into(), params);

        let mut payload = serde_json::to_vec(&Value::Object(req_obj)).map_err(|e| EngineError::InvalidParams(format!("serialize error: {e}")))?;
        payload.push(b'\n');
        self.writer.write_all(&payload).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control write error: {e}")))?;
        self.writer.flush().map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control flush error: {e}")))?;

        let mut response_line = String::new();
        self.reader.read_line(&mut response_line).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("control read error: {e}")))?;
        if response_line.trim().is_empty() {
            return Err(EngineError::ExternalTool(self.target.name().into(), "empty response from control server".into()));
        }
        let parsed: Value =
            serde_json::from_str(&response_line).map_err(|e| EngineError::ExternalTool(self.target.name().into(), format!("invalid JSON in reply: {e}")))?;
        if parsed["ok"] == true {
            Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
        } else {
            let err = parsed["error"].as_str().unwrap_or("unknown control error");
            Err(EngineError::CommandFailed(method.to_string(), err.to_string()))
        }
    }

    fn execute_engine(&mut self, command: &str, params: &Value) -> Result<Value> {
        self.send_raw("engine.execute", json!({"command": command, "params": params})).map_err(|e| match e {
            EngineError::CommandFailed(_, msg) => EngineError::CommandFailed(command.to_string(), msg),
            other => other,
        })
    }

    fn has_document_open(&mut self, expected: &Path) -> Result<bool> {
        match self.target {
            CraftTarget::Film => {
                let info = self.execute_engine("project.inspect", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Design => {
                let info = self.execute_engine("document.inspect", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Effect => {
                let info = self.execute_engine("project.summary", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Photo => {
                let info = self.send_raw("ui.inspect", json!({}))?;
                let docs = info.pointer("/session/documents").and_then(Value::as_array);
                Ok(docs.is_some_and(|list| list.iter().any(|d| d["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))))
            }
            CraftTarget::Vector => {
                let info = self.send_raw("document.inspect", json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
            CraftTarget::Deck | CraftTarget::Texture => {
                let info = self.execute_engine("document.inspect", &json!({}))?;
                Ok(info["path"].as_str().is_some_and(|p| same_canonical_path(Path::new(p), expected)))
            }
        }
    }

    fn save_open_document(&mut self, doc: &Path) -> Result<()> {
        match self.target {
            CraftTarget::Film | CraftTarget::Design | CraftTarget::Vector | CraftTarget::Deck | CraftTarget::Texture => {
                let _ = self.execute_engine("file.save", &json!({}))?;
            }
            CraftTarget::Effect => {
                let _ = self.execute_engine("file.save", &json!({"path": doc}))?;
            }
            CraftTarget::Photo => {
                let _ = self.execute_engine("app.save", &json!({}))?;
            }
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn call_tcp_at(
    target: CraftTarget,
    addr: SocketAddr,
    token: Option<&str>,
    doc: Option<&Path>,
    command: &str,
    params: &Value,
    save: bool,
    connect_timeout: Duration,
) -> TcpOutcome {
    let Ok(stream) = TcpStream::connect_timeout(&addr, connect_timeout) else {
        return TcpOutcome::Unreachable;
    };
    let setup = (|| -> Result<ControlConn> {
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set read timeout: {e}")))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| EngineError::ExternalTool(target.name().into(), format!("set write timeout: {e}")))?;
        let writer = stream.try_clone().map_err(|e| EngineError::ExternalTool(target.name().into(), format!("clone stream: {e}")))?;
        let inline_token = if target == CraftTarget::Film { token.map(str::to_owned) } else { None };
        let mut conn = ControlConn { target, inline_token, writer, reader: BufReader::new(stream), next_id: 1 };
        if target == CraftTarget::Photo
            && let Some(tok) = token
        {
            conn.send_raw("auth", json!({"token": tok}))?;
        }
        Ok(conn)
    })();
    let mut conn = match setup {
        Ok(c) => c,
        Err(e) => return TcpOutcome::Connected(Err(e)),
    };

    if let Some(expected) = doc {
        match conn.has_document_open(expected) {
            Ok(true) => {}
            Ok(false) => return TcpOutcome::DifferentDocumentOpen,
            Err(e) => return TcpOutcome::Connected(Err(e)),
        }
    }

    let result = conn.execute_engine(command, params).and_then(|val| {
        if save && let Some(path) = doc {
            conn.save_open_document(path)?;
        }
        Ok(val)
    });
    TcpOutcome::Connected(result)
}

fn call_cli_with_save(target: CraftTarget, doc: Option<&Path>, command: &str, params: &Value, save: bool) -> Result<Value> {
    let cli = crate::workspace::resolve_tool(target.cli_name());
    let mut cmd = Command::new(&cli);
    let params_str = serde_json::to_string(params).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    match target {
        CraftTarget::Film => {
            if let Some(path) = doc {
                cmd.arg("--project").arg(path);
                if save {
                    cmd.arg("--save");
                }
            }
            cmd.arg("exec").arg(command).arg(&params_str);
        }
        CraftTarget::Effect => {
            if let Some(path) = doc {
                cmd.arg("--project").arg(path);
                if save {
                    cmd.arg("--save");
                }
            }
            cmd.arg("exec").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Vector => {
            cmd.arg("run");
            if let Some(path) = doc {
                cmd.arg("--in").arg(path);
                if save {
                    cmd.arg("--out").arg(path);
                }
            }
            cmd.arg("--cmd").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Photo => {
            cmd.arg("run");
            if let Some(path) = doc {
                cmd.arg(path);
                if save {
                    cmd.arg("--out").arg(path);
                }
            } else {
                cmd.arg("--new").arg("{}");
            }
            cmd.arg("--cmd").arg(command).arg("--params").arg(&params_str);
        }
        CraftTarget::Design => {
            cmd.arg("run");
            if let Some(path) = doc {
                cmd.arg("--in").arg(path);
                if save {
                    cmd.arg("--save").arg(path);
                }
            }
            cmd.arg("--cmd").arg(format!("{command}={params_str}"));
        }
        CraftTarget::Deck | CraftTarget::Texture => {
            if let Some(path) = doc {
                cmd.arg("--project").arg(path);
                if save {
                    cmd.arg("--save");
                }
            }
            cmd.arg("exec").arg(command).arg(&params_str);
        }
    }

    let output = cmd.output().map_err(|e| EngineError::ExternalTool(target.cli_name().to_string(), e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::CommandFailed(command.to_string(), stderr.trim().to_string()));
    }

    let stdout_trim = std::str::from_utf8(&output.stdout).map_err(|e| EngineError::InvalidParams(format!("invalid UTF-8 from CLI: {e}")))?.trim();

    if stdout_trim.is_empty() || stdout_trim == "null" {
        return Ok(Value::Null);
    }

    let parsed: Value = serde_json::from_str(stdout_trim).map_err(|e| EngineError::InvalidParams(format!("invalid JSON from CLI: {e}")))?;

    if let Some(res) = parsed.get("result") { Ok(res.clone()) } else { Ok(parsed) }
}

#[cfg(test)]
mod tests {
    //! Reference:
    //! - <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/apps/filmcraft/src/control_server.rs>
    //! - <https://github.com/storytold/photocraft/blob/a3ea9a4277e1e1067acbb30d592c56e9545b949d/apps/photocraft/src/control_server.rs#L250-L288>
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn filmcraft_default_port_avoids_magic_tauri_port() {
        assert_eq!(CraftTarget::Film.default_port(), 9875);
        assert_ne!(CraftTarget::Film.default_port(), 9876);
    }

    #[test]
    fn photocraft_handshake_and_open_doc_verification_never_falls_back_on_live_error() {
        let dir = std::env::temp_dir().join(format!("magic-crafthub-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let doc = dir.join("poster.pcraft");
        std::fs::write(&doc, b"pcraft").unwrap();
        let doc_for_server = doc.clone();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();

            // 1. PhotoCraft first-frame auth handshake
            reader.read_line(&mut line).unwrap();
            let auth: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(auth["method"], "auth");
            assert_eq!(auth["params"]["token"], "secret-token");
            writeln!(stream, "{}", json!({"id": auth["id"], "ok": true, "result": {}})).unwrap();

            // 2. Open-document inspection via ui.inspect
            line.clear();
            reader.read_line(&mut line).unwrap();
            let inspect: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(inspect["method"], "ui.inspect");
            writeln!(stream, "{}", json!({"id": inspect["id"], "ok": true, "result": {"session": {"documents": [{"path": doc_for_server}]}}})).unwrap();

            // 3. Engine command fails in live session -> must surface Connected(Err), never CLI fallback
            line.clear();
            reader.read_line(&mut line).unwrap();
            let exec: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(exec["method"], "engine.execute");
            assert_eq!(exec["params"]["command"], "layer.rename");
            writeln!(stream, "{}", json!({"id": exec["id"], "ok": false, "error": "layer locked"})).unwrap();
        });

        let outcome =
            call_tcp_at(CraftTarget::Photo, addr, Some("secret-token"), Some(&doc), "layer.rename", &json!({"name": "Hero"}), false, Duration::from_secs(2));
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        match outcome {
            TcpOutcome::Connected(Err(EngineError::CommandFailed(cmd, msg))) => {
                assert_eq!(cmd, "layer.rename");
                assert_eq!(msg, "layer locked");
            }
            _ => panic!("expected Connected(Err(CommandFailed)), not CLI fallback"),
        }
    }

    #[test]
    fn mismatched_live_document_reports_different_document_open() {
        let dir = std::env::temp_dir().join(format!("magic-crafthub-mismatch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target_doc = dir.join("song-a.fcproj");
        let other_doc = dir.join("song-b.fcproj");
        std::fs::write(&target_doc, b"{}").unwrap();
        std::fs::write(&other_doc, b"{}").unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let req: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(req["token"], "fc-tok");
            assert_eq!(req["params"]["command"], "project.inspect");
            writeln!(stream, "{}", json!({"id": req["id"], "ok": true, "result": {"path": other_doc, "dirty": true}})).unwrap();
        });

        let outcome = call_tcp_at(
            CraftTarget::Film,
            addr,
            Some("fc-tok"),
            Some(&target_doc),
            "media.replaceFootage",
            &json!({"item": 1, "path": "/tmp/x.mov"}),
            true,
            Duration::from_secs(2),
        );
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(matches!(outcome, TcpOutcome::DifferentDocumentOpen));
    }
}
