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
    /// Show Animation only; a missing project is created as an animation study.
    #[serde(default)]
    pub animation_only: bool,
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
/// A command that panics fails its call; the server keeps serving.
async fn blocking<T: Send + 'static>(
    session: &Arc<Mutex<Session>>,
    work: impl FnOnce(&mut Session) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let mut guard = session.clone().lock_owned().await;
    tokio::task::spawn_blocking(move || work(&mut guard))
        .await
        .unwrap_or_else(|e| Err(format!("Magic command failed unexpectedly: {e}")))
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
        // Only a ui.look without an output needs a temporary picture folder.
        let folder = (look && params["output"].is_null()).then(|| {
            std::env::temp_dir().join(format!(
                "magic-look-{}-{}",
                std::process::id(),
                rand_suffix()
            ))
        });
        if let Some(folder) = &folder {
            if let Err(e) = std::fs::create_dir_all(folder) {
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
        if let Some(folder) = folder {
            let _ = std::fs::remove_dir_all(folder);
        }
        out
    }

    #[tool(
        description = "Open/focus a Magic project window and bind this session. animation_only shows only the Animation workspace and requires project."
    )]
    async fn studio_open(&self, Parameters(args): Parameters<OpenArgs>) -> CallToolResult {
        if self.dry_run {
            return text(
                &json!({"dry_run": true, "project": args.project, "animation_only": args.animation_only}),
            );
        }
        match blocking(&self.session, move |s| {
            match (args.animation_only, args.project) {
                (true, Some(project)) => s.open_animation(&project),
                (true, None) => Err("animation_only needs a project path".into()),
                (false, project) => s.open_studio(project.as_deref()),
            }
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
