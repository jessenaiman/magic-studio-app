//! Magic Studio's MCP server (`magic-mcp`).
//!
//! Model Context Protocol over stdio, served with the `rmcp` crate on a tokio runtime (as FilmCraft
//! does). It exposes tools only: `command_list`, `command_run`, `command_batch` over
//! [`magic::command_specs`] and [`magic::Session::execute`], plus the `ui_*` tools for the live
//! window. Calls go to exactly one backend, chosen when the server starts:
//! - [`Remote`]: a running `magic --control <port>` desktop window over authenticated loopback
//!   TCP (`magic-cli --bridge ADDRESS mcp`); it never falls back to a local session.
//! - [`Headless`]: an in-process [`magic::Session`] (`magic-cli mcp`); `ui_*` tools are refused.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use magic::Session;
use serde_json::{Value, json};

/// Execution backend for MCP tool calls.
pub trait Backend {
    fn execute(&mut self, id: &str, params: Value) -> Result<Value, String>;
    fn is_live(&self) -> bool {
        false
    }
}

/// In-process headless [`Session`] backend.
#[derive(Default)]
pub struct Headless {
    pub session: Session,
}

impl Headless {
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

impl Backend for Headless {
    fn execute(&mut self, id: &str, params: Value) -> Result<Value, String> {
        self.session.execute(id, params).map_err(|e| e.to_string())
    }
}

/// Loopback TCP backend talking to a running `magic --control <port>` window.
pub struct Remote {
    pub addr: String,
    pub token: Option<String>,
    next_id: u64,
}

impl Remote {
    pub fn new(addr: impl Into<String>, token: Option<String>) -> Self {
        Self { addr: addr.into(), token, next_id: 1 }
    }
}

impl Backend for Remote {
    fn is_live(&self) -> bool {
        true
    }
    fn execute(&mut self, id: &str, params: Value) -> Result<Value, String> {
        let addr: std::net::SocketAddr = self.addr.parse().map_err(|_| "bridge requires a loopback IP:port")?;
        if !addr.ip().is_loopback() {
            return Err("bridge address must be loopback".into());
        }
        let token = self.token.as_deref().ok_or("MAGIC_CONTROL_TOKEN is required for --bridge")?;
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("MAGIC_CONTROL_TOKEN must contain 64 hexadecimal characters".into());
        }
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("connect {}: {e}", self.addr))?;
        stream.set_read_timeout(Some(Duration::from_secs(300))).map_err(|e| e.to_string())?;
        stream.set_write_timeout(Some(Duration::from_secs(120))).map_err(|e| e.to_string())?;
        let req_id = self.next_id;
        self.next_id += 1;
        let (method, params) = if id.starts_with("ui.") || id.starts_with("engine.") || id == "app.quit" {
            (id, params)
        } else {
            ("engine.execute", json!({"command":id,"params":params}))
        };
        let mut msg = json!({
            "id": req_id,
            "method": method,
            "params": params,
        });
        if let Some(tok) = &self.token {
            msg["token"] = json!(tok);
        }
        let line = format!("{msg}\n");
        stream.write_all(line.as_bytes()).map_err(|e| format!("write {}: {e}", self.addr))?;
        stream.flush().map_err(|e| format!("flush {}: {e}", self.addr))?;
        let mut reader = BufReader::new(stream);
        let mut reply = String::new();
        reader.read_line(&mut reply).map_err(|e| format!("read {}: {e}", self.addr))?;
        let parsed: Value = serde_json::from_str(reply.trim()).map_err(|e| format!("invalid reply: {e}"))?;
        if parsed["ok"] == true {
            Ok(parsed.get("result").cloned().unwrap_or(parsed))
        } else {
            Err(parsed["error"].as_str().unwrap_or("remote command failed").to_owned())
        }
    }
}

mod server;
pub use server::Server;
