//! MCP over stdio (official `rmcp` SDK). Three tools; the command vocabulary is the engine
//! registry, reached through the same caller session as the command line.
use crate::session::Session;
use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ServiceExt, schemars, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListArgs {
    /// Text to match against command IDs, labels and help; empty lists everything.
    #[serde(default)]
    pub filter: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RunArgs {
    /// Registry command ID, e.g. `words.read`.
    pub id: String,
    /// Parameters as documented by command_list.
    #[serde(default)]
    pub params: Option<serde_json::Map<String, Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OpenArgs {
    /// An existing .magic file or project folder; omit for a blank window.
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Clone)]
pub struct MagicMcp {
    // One command at a time: native work and the session binding finish before the next.
    session: Arc<Mutex<Session>>,
    dry_run: bool,
}

fn text(value: &Value) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string_pretty(value).unwrap_or_default(),
    )])
}

fn failure(error: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error)])
}

/// Run blocking engine or socket work off the async runtime, holding the session lock.
async fn blocking<T: Send + 'static>(
    session: &Arc<Mutex<Session>>,
    work: impl FnOnce(&mut Session) -> T + Send + 'static,
) -> T {
    let mut guard = session.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || work(&mut guard))
        .await
        .expect("session task panicked")
}

#[tool_router]
impl MagicMcp {
    #[tool(description = "Discover Magic registry IDs and engine-defined parameter documentation.")]
    async fn command_list(&self, Parameters(args): Parameters<ListArgs>) -> CallToolResult {
        let dry = self.dry_run;
        match blocking(&self.session, move |s| {
            s.execute("command.list", json!({"filter": args.filter}), dry)
        })
        .await
        {
            Ok(v) => text(&v),
            Err(e) => failure(e),
        }
    }

    #[tool(
        description = "Execute one Magic registry command. Use command_list for IDs and params."
    )]
    async fn command_run(&self, Parameters(args): Parameters<RunArgs>) -> CallToolResult {
        let dry = self.dry_run;
        let mut params = Value::Object(args.params.unwrap_or_default());
        params["_origin"] = json!("MCP");
        let look = args.id == "ui.look" && !dry && params["_dry_run"] != true;
        let folder = std::env::temp_dir().join(format!(
            "magic-look-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        if look && params["output"].is_null() {
            if let Err(e) = std::fs::create_dir_all(&folder) {
                return failure(format!("{}: {e}", folder.display()));
            }
            params["output"] = json!(folder.join("window.png"));
        }
        let id = args.id.clone();
        let result = blocking(&self.session, move |s| s.execute(&id, params, dry)).await;
        let out = match result {
            Err(e) => failure(e),
            Ok(v) if look => match v["output"].as_str().map(std::fs::read) {
                Some(Ok(png)) => CallToolResult::success(vec![
                    ContentBlock::image(
                        base64::engine::general_purpose::STANDARD.encode(png),
                        "image/png",
                    ),
                    ContentBlock::text(v.to_string()),
                ]),
                _ => text(&v),
            },
            Ok(v) => text(&v),
        };
        let _ = std::fs::remove_dir_all(&folder);
        out
    }

    #[tool(description = "Open/focus a Magic project window and bind this session.")]
    async fn studio_open(&self, Parameters(args): Parameters<OpenArgs>) -> CallToolResult {
        if self.dry_run {
            return text(&json!({"dry_run": true, "project": args.project}));
        }
        match blocking(&self.session, move |s| {
            s.open_studio(args.project.as_deref())
        })
        .await
        {
            Ok(v) => text(&v),
            Err(e) => failure(e),
        }
    }
}

fn rand_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

#[tool_handler(
    name = "Magic Studio",
    instructions = "Discover Magic operations with command_list, then call command_run. Select a project using project.open with params.path, or studio_open for a visible window. Operations retain this session's project/window target. Other apps use their own CLIs."
)]
impl rmcp::ServerHandler for MagicMcp {}

/// Serve until the client closes stdin.
pub fn serve(session: Session, dry_run: bool) -> Result<(), String> {
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let server = MagicMcp {
            session: Arc::new(Mutex::new(session)),
            dry_run,
        };
        let running = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| e.to_string())?;
        running.waiting().await.map_err(|e| e.to_string())?;
        Ok(())
    })
}
