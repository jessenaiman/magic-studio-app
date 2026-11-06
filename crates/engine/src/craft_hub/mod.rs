//! CraftHub: Adaptive transport router connecting Magic Studio to the ArtCraft suite.
//!
//! Provides a single, unified interface ([`CraftHub::execute`]) that:
//! 1. Dispatches to live desktop applications via loopback TCP JSON-lines (`--control <port>`)
//!    when the application GUI is running.
//! 2. Seamlessly falls back to headless CLI execution (`<app>-cli exec <command>`) when
//!    the application GUI is not running.
//!
//! Caller code never deals with sockets, port numbers, process lifecycles, or transport differences.

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

    /// Standard default control port for the application.
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Film => 9876,
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

impl CraftHub {
    /// Execute a command against a Craft target application.
    ///
    /// 1. First attempts to send the command to the running application via loopback TCP.
    /// 2. If the application is not running (connection refused / timeout), transparently falls
    ///    back to the headless CLI on PATH.
    pub fn execute(
        target: CraftTarget,
        doc: Option<&Path>,
        command: &str,
        params: &Value,
    ) -> Result<Value> {
        match call_tcp(target, command, params, Duration::from_millis(250)) {
            Ok(result) => Ok(result),
            Err(_) => call_cli(target, doc, command, params),
        }
    }
}

fn call_tcp(
    target: CraftTarget,
    command: &str,
    params: &Value,
    timeout: Duration,
) -> std::result::Result<Value, String> {
    let port = std::env::var(target.port_env())
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or_else(|| target.default_port());

    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|e| format!("invalid address: {e}"))?;

    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| format!("tcp connect failed: {e}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| format!("set read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set write timeout: {e}"))?;

    let token = target.token_env().and_then(|var| std::env::var(var).ok());

    let mut req_obj = serde_json::Map::new();
    req_obj.insert("id".into(), json!(1));
    if let Some(tok) = token {
        req_obj.insert("token".into(), json!(tok));
    }
    req_obj.insert("method".into(), json!("engine.execute"));
    req_obj.insert(
        "params".into(),
        json!({
            "command": command,
            "params": params
        }),
    );

    let mut payload =
        serde_json::to_vec(&Value::Object(req_obj)).map_err(|e| format!("serialize error: {e}"))?;
    payload.push(b'\n');

    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("clone stream: {e}"))?;
    writer
        .write_all(&payload)
        .map_err(|e| format!("write error: {e}"))?;
    writer.flush().map_err(|e| format!("flush error: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| format!("read error: {e}"))?;

    if response_line.trim().is_empty() {
        return Err("empty response from control server".into());
    }

    let parsed: Value =
        serde_json::from_str(&response_line).map_err(|e| format!("invalid JSON in reply: {e}"))?;

    if parsed["ok"] == true {
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    } else {
        let err = parsed["error"].as_str().unwrap_or("unknown control error");
        Err(err.to_string())
    }
}

fn call_cli(
    target: CraftTarget,
    doc: Option<&Path>,
    command: &str,
    params: &Value,
) -> Result<Value> {
    let cli = target.cli_name();
    let mut cmd = Command::new(cli);

    if let Some(path) = doc {
        cmd.arg("--project").arg(path);
    }

    let params_str =
        serde_json::to_string(params).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    cmd.arg("exec").arg(command).arg(&params_str);

    let output = cmd
        .output()
        .map_err(|e| EngineError::ExternalTool(cli.to_string(), e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::CommandFailed(
            command.to_string(),
            stderr.trim().to_string(),
        ));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| EngineError::InvalidParams(format!("invalid JSON from CLI: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn craft_targets_have_cli_names_and_ports() {
        assert_eq!(CraftTarget::Film.cli_name(), "filmcraft-cli");
        assert_eq!(CraftTarget::Vector.cli_name(), "vectorcraft-cli");
        assert_eq!(CraftTarget::Design.cli_name(), "designcraft-cli");
        assert_eq!(CraftTarget::Effect.cli_name(), "effectcraft-cli");
        assert_eq!(CraftTarget::Photo.cli_name(), "photocraft-cli");

        assert_eq!(CraftTarget::Film.default_port(), 9876);
        assert_eq!(CraftTarget::Design.default_port(), 9877);
        assert_eq!(CraftTarget::Effect.default_port(), 9878);
        assert_eq!(CraftTarget::Photo.default_port(), 9879);
        assert_eq!(CraftTarget::Vector.default_port(), 9880);
    }

    #[test]
    fn tcp_connect_gracefully_fails_when_port_closed() {
        let res = call_tcp(
            CraftTarget::Vector,
            "noop",
            &json!({}),
            Duration::from_millis(50),
        );
        // Expect an Err, not a panic or crash
        assert!(res.is_err());
    }
}
