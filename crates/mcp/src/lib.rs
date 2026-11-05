//! Magic Studio's MCP server (`magic-mcp`).
//!
//! Model Context Protocol over stdio (newline-delimited JSON-RPC 2.0, no async runtime).
//! Exposes [`magic::command_specs`] and [`magic::Session::execute`] as MCP tools
//! and resources, forwarding to either:
//! - [`Remote`]: a running `magic --control <port>` desktop window over loopback TCP, or
//! - [`Headless`]: an in-process [`magic::Session`].

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

use magic::{Session, command_specs, find_command};
use serde_json::{Value, json};

pub const PROTOCOL_VERSION: &str = "2024-11-05";
pub const DEFAULT_ADDR: &str = "127.0.0.1:7985";

/// Execution backend for MCP tool calls.
pub trait Backend {
    fn execute(&mut self, id: &str, params: Value) -> Result<Value, String>;
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

/// Loopback TCP backend talking to a running `magic --control <addr>` window.
pub struct Remote {
    pub addr: String,
    pub token: Option<String>,
    next_id: u64,
}

impl Remote {
    pub fn new(addr: impl Into<String>, token: Option<String>) -> Self {
        Self {
            addr: addr.into(),
            token,
            next_id: 1,
        }
    }
}

impl Backend for Remote {
    fn execute(&mut self, id: &str, params: Value) -> Result<Value, String> {
        let mut stream = TcpStream::connect(&self.addr)
            .map_err(|e| format!("connect {}: {e}", self.addr))?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(300)));
        let req_id = self.next_id;
        self.next_id += 1;
        let mut msg = json!({
            "id": req_id,
            "method": id,
            "params": params,
        });
        if let Some(tok) = &self.token {
            msg["token"] = json!(tok);
        }
        let line = format!("{msg}\n");
        stream
            .write_all(line.as_bytes())
            .map_err(|e| format!("write {}: {e}", self.addr))?;
        stream
            .flush()
            .map_err(|e| format!("flush {}: {e}", self.addr))?;
        let mut reader = BufReader::new(stream);
        let mut reply = String::new();
        reader
            .read_line(&mut reply)
            .map_err(|e| format!("read {}: {e}", self.addr))?;
        let parsed: Value =
            serde_json::from_str(reply.trim()).map_err(|e| format!("invalid reply: {e}"))?;
        if parsed["ok"] == true {
            Ok(parsed.get("result").cloned().unwrap_or(parsed))
        } else {
            Err(parsed["error"]
                .as_str()
                .unwrap_or("remote command failed")
                .to_owned())
        }
    }
}

/// MCP tool definitions exposing Magic Studio's engine registry.
pub fn tool_definitions() -> Value {
    let command_count = command_specs().len();
    json!([
        {
            "name": "magic_exec",
            "description": format!(
                "Execute any of Magic Studio's {command_count} registered engine commands by id (`domain.verb`) with JSON `params`."
            ),
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Engine command ID (e.g. `project.catalog`, `storyboard.scenes`, `fidget.prepare`, `scribbles.generate`)."
                    },
                    "params": {
                        "type": "object",
                        "description": "JSON object of command parameters."
                    }
                }
            }
        },
        {
            "name": "magic_commands",
            "description": "List Magic Studio engine commands, optionally filtered by substring.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "filter": {
                        "type": "string",
                        "description": "Optional substring filter (e.g. `storyboard`, `scribbles`, `fidget`, `character`)."
                    }
                }
            }
        },
        {
            "name": "magic_fidget_prepare",
            "description": "Prepare a structured Markdown drawing brief with YAML frontmatter and mandatory NOTE via Fidget.",
            "inputSchema": {
                "type": "object",
                "required": ["kind"],
                "properties": {
                    "kind": {
                        "type": "string",
                        "description": "Template kind (`new`, `image`, `sheet`, `poses`, `actions`, `storyboard_hero`, `storyboard_sketch`, `custom`)."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Character or scene description."
                    },
                    "reference": {
                        "type": "string",
                        "description": "Optional reference PNG path."
                    },
                    "action": {
                        "type": "string",
                        "description": "Optional action for `actions` template (`walk`, `run`, `jump`, `wave`, `turn`)."
                    }
                }
            }
        }
    ])
}

pub struct Server<B: Backend> {
    pub backend: B,
}

impl<B: Backend> Server<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Process one newline-delimited JSON-RPC 2.0 message. Returns `Some(response)` for requests
    /// and `None` for notifications.
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": format!("Parse error: {e}") }
                }));
            }
        };
        let id = msg.get("id").cloned()?;
        let method = msg["method"].as_str().unwrap_or_default();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "magic-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => self.call_tool(&params),
            "resources/list" => Ok(json!({
                "resources": [
                    {
                        "uri": "magic://commands",
                        "name": "Magic Studio Command Registry",
                        "mimeType": "application/json",
                        "description": "All registered magic commands and parameter schemas"
                    },
                    {
                        "uri": "magic://scribbles/templates",
                        "name": "Scribbles & Fidget Templates",
                        "mimeType": "application/json",
                        "description": "Template definitions, save commands, and Craft AI role status"
                    }
                ]
            })),
            "resources/read" => self.read_resource(&params),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        match result {
            Ok(res) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": res
            })),
            Err((code, message)) => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message }
            })),
        }
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i32, String)> {
        let name = params["name"]
            .as_str()
            .ok_or_else(|| (-32602, "Missing tool `name`".into()))?;
        let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        let outcome = match name {
            "magic_exec" => {
                let cmd = args["command"]
                    .as_str()
                    .ok_or_else(|| (-32602, "Missing `command` string".into()))?;
                let cmd_params = args.get("params").cloned().unwrap_or_else(|| json!({}));
                self.backend.execute(cmd, cmd_params)
            }
            "magic_commands" => {
                let filter = args["filter"].as_str().unwrap_or_default();
                let list: Vec<Value> = command_specs()
                    .iter()
                    .filter(|c| filter.is_empty() || c.id.contains(filter) || c.label.contains(filter))
                    .map(|c| c.describe())
                    .collect();
                Ok(json!({ "ok": true, "commands": list }))
            }
            "magic_fidget_prepare" => self.backend.execute("fidget.prepare", args),
            _ => {
                if find_command(name).is_some() {
                    self.backend.execute(name, args)
                } else {
                    Err(format!("Unknown MCP tool `{name}`"))
                }
            }
        };
        match outcome {
            Ok(val) => {
                let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
                Ok(json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false
                }))
            }
            Err(err) => Ok(json!({
                "content": [{ "type": "text", "text": err }],
                "isError": true
            })),
        }
    }

    fn read_resource(&mut self, params: &Value) -> Result<Value, (i32, String)> {
        let uri = params["uri"]
            .as_str()
            .ok_or_else(|| (-32602, "Missing resource `uri`".into()))?;
        let payload = match uri {
            "magic://commands" => {
                let list: Vec<Value> = command_specs().iter().map(|c| c.describe()).collect();
                json!({ "commands": list })
            }
            "magic://scribbles/templates" => self
                .backend
                .execute("scribbles.templates", json!({}))
                .map_err(|e| (-32000, e))?,
            _ => return Err((-32602, format!("Unknown resource URI: {uri}"))),
        };
        let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": text
            }]
        }))
    }

    /// Serve JSON-RPC 2.0 requests from `stdin` to `stdout`.
    pub fn serve(&mut self) -> std::io::Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().lock();
        for line in stdin.lock().lines() {
            let line = line?;
            if let Some(reply) = self.handle_line(&line) {
                writeln!(stdout, "{reply}")?;
                stdout.flush()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_lists_tools_and_prepares_fidget_markdown() {
        let mut server = Server::new(Headless::default());
        let init = server
            .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .unwrap();
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);

        let tools = server
            .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
            .unwrap();
        assert!(!tools["result"]["tools"].as_array().unwrap().is_empty());

        let call = server
            .handle_line(
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"magic_fidget_prepare","arguments":{"kind":"new","prompt":"a felt bear"}}}"#,
            )
            .unwrap();
        assert_eq!(call["result"]["isError"], json!(false));
        let text = call["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("save_command"));
        assert!(text.contains(magic::fidget::FIDGET_NOTE));
    }
}
