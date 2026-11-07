//! Craft-style MCP: a small tool surface over either the engine or the live app.
use crate::Backend;
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock as Content};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Server {
    backend: Arc<Mutex<Box<dyn Backend + Send>>>,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListParams {
    filter: Option<String>,
    enabled_only: Option<bool>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RunParams {
    id: String,
    params: Option<Value>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BatchParams {
    steps: Vec<RunParams>,
    stop_on_error: Option<bool>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ElementsParams {
    prefix: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClickParams {
    id: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ControlParams {
    method: String,
    params: Option<Value>,
}

fn output(result: Result<Value, String>) -> CallToolResult {
    match result {
        Ok(value) => CallToolResult::success(vec![Content::text(value.to_string())]),
        Err(error) => CallToolResult::error(vec![Content::text(error)]),
    }
}

impl Server {
    pub fn new(backend: impl Backend + Send + 'static) -> Self {
        Self {
            backend: Arc::new(Mutex::new(Box::new(backend))),
            tool_router: Self::tool_router(),
        }
    }
    pub fn serve(&mut self) -> Result<(), String> {
        tokio::runtime::Runtime::new()
            .map_err(|e| e.to_string())?
            .block_on(async {
                let service = ServiceExt::serve(self.clone(), rmcp::transport::stdio())
                    .await
                    .map_err(|e| e.to_string())?;
                service.waiting().await.map_err(|e| e.to_string())?;
                Ok(())
            })
    }
    async fn run(&self, id: &str, params: Value) -> Result<Value, String> {
        if !params.is_object() {
            return Err("params must be an object".into());
        }
        let backend = self.backend.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut backend = backend.lock().map_err(|e| e.to_string())?;
            if id.starts_with("ui.") && !backend.is_live() {
                return Err("UI commands require MCP --bridge ADDRESS".into());
            }
            backend.execute(&id, params)
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[tool_router]
impl Server {
    #[tool(
        description = "List engine command IDs, labels and parameter documentation. Use these IDs with command_run."
    )]
    async fn command_list(&self, Parameters(p): Parameters<ListParams>) -> CallToolResult {
        output(
            self.run(
                "command.list",
                json!({"filter":p.filter.unwrap_or_default()}),
            )
            .await
            .map(|v| {
                let commands = v
                    .get("commands")
                    .unwrap_or(&v)
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                json!(
                    commands
                        .into_iter()
                        .filter(|c| !p.enabled_only.unwrap_or(false) || c["enabled"] == true)
                        .collect::<Vec<_>>()
                )
            }),
        )
    }
    #[tool(
        description = "Run an engine command by id with optional JSON params in the selected backend."
    )]
    async fn command_run(&self, Parameters(p): Parameters<RunParams>) -> CallToolResult {
        output(self.run(&p.id, p.params.unwrap_or_else(|| json!({}))).await)
    }
    #[tool(
        description = "Run steps in order. Each step has id and optional params; stop_on_error defaults to true."
    )]
    async fn command_batch(&self, Parameters(p): Parameters<BatchParams>) -> CallToolResult {
        let mut results = Vec::new();
        let mut completed = 0;
        let mut failed = 0;
        for step in p.steps {
            match self
                .run(&step.id, step.params.unwrap_or_else(|| json!({})))
                .await
            {
                Ok(value) => {
                    completed += 1;
                    results.push(json!({"ok":true,"result":value}));
                }
                Err(error) => {
                    failed += 1;
                    results.push(json!({"ok":false,"error":error}));
                    if p.stop_on_error.unwrap_or(true) {
                        break;
                    }
                }
            }
        }
        let value = json!({"completed":completed,"failed":failed,"results":results});
        if failed > 0 {
            CallToolResult::error(vec![Content::text(value.to_string())])
        } else {
            output(Ok(value))
        }
    }
    #[tool(description = "Inspect the actual live desktop UI state. Requires --bridge.")]
    async fn ui_inspect(&self) -> CallToolResult {
        output(self.run("ui.inspect", json!({})).await)
    }
    #[tool(
        description = "List live DOM element IDs, labels, enabled state and bounds. Requires --bridge."
    )]
    async fn ui_elements(&self, Parameters(p): Parameters<ElementsParams>) -> CallToolResult {
        output(self.run("ui.elements", json!({"prefix":p.prefix})).await)
    }
    #[tool(
        description = "Click a visible enabled DOM element by ID from ui_elements. Requires --bridge."
    )]
    async fn ui_click(&self, Parameters(p): Parameters<ClickParams>) -> CallToolResult {
        output(self.run("ui.click", json!({"id":p.id})).await)
    }
    #[tool(
        description = "Call a live control method such as ui.set or ui.focus. Unsupported operations return errors."
    )]
    async fn ui_control(&self, Parameters(p): Parameters<ControlParams>) -> CallToolResult {
        if !p.method.starts_with("ui.") && p.method != "app.quit" {
            return output(Err("Use command_run for engine commands".into()));
        }
        output(
            self.run(&p.method, p.params.unwrap_or_else(|| json!({})))
                .await,
        )
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> rmcp::model::ServerConfig {
        rmcp::model::ServerConfig::new(rmcp::model::ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new("magic", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use command_list then command_run or command_batch. UI tools require an explicit --bridge connection to the live Tauri app. Headless sessions do not control a window.".to_string())
    }
}
