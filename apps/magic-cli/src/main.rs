//! `magic-cli`: thin headless CLI over `magic::Session` (modeled on `filmcraft-cli`).

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};
use std::io::BufRead;

const AFTER_HELP: &str = "\
FIND: magic-cli commands [filter], then magic-cli describe <id>
RUN:  magic-cli storyboard pages page=1 (any command id, with key=value)
LIVE: magic --control 9876, then magic-cli --bridge 127.0.0.1:9876 ui inspect";

#[derive(Parser)]
#[command(name = "magic-cli", about = "Drive Magic Studio through its Rust engine or a live Tauri session", after_help = AFTER_HELP)]
pub struct Cli {
    /// Open this .magic project first (headless)
    #[arg(long, global = true)]
    pub project: Option<String>,
    /// Save the project back to --project when done
    #[arg(long, global = true)]
    pub save: bool,
    /// Save the project to this path when done
    #[arg(long, global = true)]
    pub save_as: Option<String>,
    /// Connect to a running `magic --control <PORT>` window
    #[arg(long, global = true)]
    pub bridge: Option<String>,
    /// Preview mutations without applying them
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// JSON output (default)
    #[arg(long, global = true)]
    pub json: bool,
    /// Run an engine or UI command by ID, like the exec subcommand
    #[arg(long = "command")]
    command_id: Option<String>,
    /// JSON parameters for --command
    #[arg(long, requires = "command_id")]
    params: Option<String>,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// List engine commands (id and label; `describe` shows one in full)
    Commands { filter: Option<String> },
    /// Describe one command as JSON
    Describe { id: String },
    /// Inspect project or timeline state as JSON
    Inspect { target: Option<String> },
    /// Run one command (`exec <id> [key=value ...]` or `--params '<json>'`)
    Exec {
        command: String,
        #[arg(long)]
        params: Option<String>,
        arguments: Vec<String>,
    },
    /// Run commands from a JSONL file or stdin (`-`), one `{"id","params"}` per line
    Run {
        #[arg(default_value = "-")]
        file: String,
    },
    /// Serve the Model Context Protocol (JSON-RPC 2.0 over stdio)
    Mcp,
    /// Control a live desktop window through --bridge
    Ui {
        /// Live window action: `inspect`, `elements`, `click`, `move`, `drag`, `scroll`, `key`, `type`,
        /// `set`, `focus`, `view`, `resize`, `screenshot`, `quit` (any `ui.<method>` name also works)
        action: String,
        /// Optional argument for `ui focus <id>` or `ui view <mode>`
        target: Option<String>,
        /// JSON parameters for the live UI action
        #[arg(long)]
        params: Option<String>,
        /// Additional parameters as key=value pairs
        arguments: Vec<String>,
    },
    #[command(external_subcommand)]
    Registry(Vec<String>),
}

fn emit(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn dry_result(id: &str, params: &Value) -> Value {
    json!({ "dry_run": true, "id": id, "params": params })
}

fn parse_params(tokens: &[String], raw: Option<&str>) -> Result<Value, String> {
    let mut params = match raw {
        Some(t) => serde_json::from_str::<Value>(t).map_err(|e| format!("--params: {e}"))?,
        None => json!({}),
    };
    if !params.is_object() {
        return Err("--params must be a JSON object".into());
    }
    for token in tokens {
        if let Some(obj) = token.strip_prefix('{')
            && let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&format!("{{{obj}"))
        {
            for (k, v) in map {
                params[k] = v;
            }
            continue;
        }
        let (k, v) = token.split_once('=').ok_or_else(|| format!("Expected key=value, got {token:?}"))?;
        params[k] = serde_json::from_str(v).unwrap_or_else(|_| json!(v));
    }
    Ok(params)
}

fn control_address(spec: &str) -> String {
    if spec.contains(':') { spec.to_owned() } else { format!("127.0.0.1:{spec}") }
}

fn call_live_control(bridge: Option<&str>, method: &str, params: Value) -> Result<Value, String> {
    let spec = bridge.ok_or("Live UI commands require --bridge ADDRESS")?;
    let mut remote = magic_mcp::Remote::new(control_address(spec), magic::workspace::control_token());
    magic_mcp::Backend::execute(&mut remote, method, params)
}

/// Where commands run, and what to do with the project afterwards.
struct Target {
    session: Option<magic::Session>,
    bridge: Option<String>,
    project: Option<String>,
    save: bool,
    save_as: Option<String>,
    dry: bool,
}

impl Target {
    fn call(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if self.bridge.is_some() || id.starts_with("ui.") {
            return call_live_control(self.bridge.as_deref(), id, params);
        }
        let session = self.session.as_mut().ok_or("no headless session: a bridge runs commands in the live app")?;
        session.execute(id, params).map_err(|e| e.to_string())
    }

    fn call_unless_dry(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if self.dry { Ok(dry_result(id, &params)) } else { self.call(id, params) }
    }

    fn save_project(&mut self) -> Result<(), String> {
        if self.dry {
            return Ok(());
        }
        let path = if self.save_as.is_some() { self.save_as.clone() } else { self.project.clone() };
        if self.save_as.is_some() || self.save {
            let params = path.map_or_else(|| json!({}), |path| json!({ "path": path }));
            self.call("project.save", params)?;
        }
        Ok(())
    }

    /// Run one command, print its result and save if asked.
    fn run(&mut self, id: &str, params: Value) -> Result<(), String> {
        emit(&self.call_unless_dry(id, params)?);
        self.save_project()
    }
}

fn list_commands(target: &mut Target, filter: Option<String>) -> Result<(), String> {
    let list = target.call("command.list", json!({ "filter": filter.unwrap_or_default() }))?;
    let rows = list["commands"].as_array().map_or(&[][..], Vec::as_slice);
    for row in rows {
        println!("{}  {}", row["id"].as_str().unwrap_or_default(), row["label"].as_str().unwrap_or_default());
    }
    Ok(())
}

fn inspect(target: &mut Target, what: Option<String>) -> Result<(), String> {
    let value = match what.as_deref().unwrap_or("all") {
        "project" => target.call("project.status", json!({}))?,
        "timeline" | "sequence" => target.call("film.timeline", json!({}))?,
        "all" => json!({
            "project": target.call("project.status", json!({}))?,
            "timeline": target.call("film.timeline", json!({})).unwrap_or(Value::Null),
        }),
        other => return Err(format!("inspect: unknown `{other}` (project | timeline)")),
    };
    emit(&value);
    Ok(())
}

fn run_file(target: &mut Target, file: &str) -> Result<(), String> {
    let reader: Box<dyn BufRead> = if file == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::io::BufReader::new(std::fs::File::open(file).map_err(|e| format!("{file}: {e}"))?))
    };
    for (n, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let text = line.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let item: Value = serde_json::from_str(text).map_err(|e| format!("line {}: {e}", n + 1))?;
        let id = item["id"].as_str().or_else(|| item["command"].as_str()).unwrap_or_default();
        let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = target.call_unless_dry(id, params)?;
        println!("{}", json!({ "line": n + 1, "ok": true, "result": result }));
    }
    target.save_project()
}

fn serve_mcp(target: &mut Target) -> Result<(), String> {
    if let Some(address) = &target.bridge {
        let remote = magic_mcp::Remote::new(control_address(address), magic::workspace::control_token());
        return magic_mcp::Server::new(remote).serve().map_err(|e| e.to_string());
    }
    let session = target.session.take().ok_or("no headless session: use --bridge for the live app")?;
    magic_mcp::Server::new(magic_mcp::Headless::new(session)).serve().map_err(|e| e.to_string())
}

fn ui_action(action: &str, target: Option<String>, mut args: Vec<String>, raw: Option<&str>) -> Result<(String, Value), String> {
    let method = match action {
        "inspect" | "state" => "ui.state".to_owned(),
        // The live app reports workspace feedback inside ui.inspect (`workspaces[].feedback`).
        "feedback" | "look" => "ui.inspect".to_owned(),
        "view" => "ui.set_view".to_owned(),
        "quit" => "app.quit".to_owned(),
        id if id.contains('.') => id.to_owned(),
        id => format!("ui.{id}"),
    };
    if let Some(target) = target {
        let key = match method.as_str() {
            "ui.focus" | "ui.click" => Some("id"),
            "ui.set_view" => Some("view"),
            "ui.key" => Some("key"),
            "ui.type" | "ui.text" => Some("text"),
            "ui.screenshot" => Some("path"),
            _ => None,
        };
        let bare = !target.contains('=') && !target.starts_with('{');
        args.insert(0, key.filter(|_| bare).map_or_else(|| target.clone(), |key| format!("{key}={target}")));
    }
    Ok((method, parse_params(&args, raw)?))
}

fn run_ui(target: &mut Target, action: &str, value: Option<String>, params: Option<&str>, arguments: Vec<String>) -> Result<(), String> {
    if target.bridge.is_none() {
        return Err("Live UI commands require --bridge ADDRESS".into());
    }
    let (method, params) = ui_action(action, value, arguments, params)?;
    let result = if target.dry { dry_result(&method, &params) } else { call_live_control(target.bridge.as_deref(), &method, params)? };
    emit(&result);
    Ok(())
}

fn resolve_shorthand(tokens: &[String]) -> Result<(String, Vec<String>), String> {
    if tokens.first().map(String::as_str) == Some("locations") {
        return Ok(("storyboard.locations".into(), tokens[1..].to_vec()));
    }
    if tokens.first().map(String::as_str) == Some("studio") {
        return match tokens.get(1).map(String::as_str) {
            Some("open") => {
                let rest = tokens.get(2).map(|p| vec![format!("path={p}")]).unwrap_or_default();
                Ok(("project.open".into(), rest))
            }
            _ => Ok(("project.catalog".into(), vec![])),
        };
    }
    let known: Vec<&str> = magic::command_specs().iter().map(|c| c.id).collect();
    let words: Vec<&String> = tokens.iter().take_while(|t| !t.contains('=') && !t.starts_with('-') && !t.starts_with('{')).collect();
    for n in (1..=words.len()).rev() {
        let id = words[..n].iter().map(|w| w.replace('-', "_")).collect::<Vec<_>>().join(".");
        if known.contains(&id.as_str()) {
            return Ok((id, tokens[n..].to_vec()));
        }
    }
    Err(format!("Unknown command '{}'. Run `magic-cli commands`.", tokens.join(" ")))
}

/// `magic-cli storyboard pages page=1`: any command id, as words, then its parameters.
fn run_shorthand(target: &mut Target, tokens: &[String]) -> Result<(), String> {
    let (id, rest) = resolve_shorthand(tokens)?;
    if rest.iter().any(|t| t == "--help" || t == "-h")
        && let Some(command) = magic::find_command(&id)
    {
        emit(&command.describe());
        return Ok(());
    }
    let mut pairs = Vec::new();
    let mut raw = None;
    let mut tokens = rest.into_iter();
    while let Some(token) = tokens.next() {
        if token == "--params" {
            raw = tokens.next();
        } else if let Some(value) = token.strip_prefix("--params=") {
            raw = Some(value.to_owned());
        } else {
            pairs.push(token);
        }
    }
    let params = parse_params(&pairs, raw.as_deref())?;
    target.run(&id, params)
}

fn describe(id: &str) -> Result<(), String> {
    let command = magic::find_command(id).ok_or_else(|| format!("Unknown command '{id}'"))?;
    emit(&command.describe());
    Ok(())
}

fn run_command(target: &mut Target, command: Cmd) -> Result<(), String> {
    match command {
        Cmd::Commands { filter } => list_commands(target, filter),
        Cmd::Describe { id } => describe(&id),
        Cmd::Inspect { target: what } => inspect(target, what),
        Cmd::Exec { command, params, arguments } => {
            let params = parse_params(&arguments, params.as_deref())?;
            target.run(&command, params)
        }
        Cmd::Run { file } => run_file(target, &file),
        Cmd::Mcp => serve_mcp(target),
        Cmd::Ui { action, target: value, params, arguments } => run_ui(target, &action, value, params.as_deref(), arguments),
        Cmd::Registry(tokens) => run_shorthand(target, &tokens),
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let live = cli.bridge.is_some();
    let mut session = (!live).then(magic::Session::user_session);
    if let (Some(path), Some(session)) = (cli.project.as_deref(), session.as_mut()) {
        session.execute("project.open", json!({ "path": path })).map_err(|e| e.to_string())?;
    }
    let mut target = Target { session, bridge: cli.bridge, project: cli.project, save: cli.save, save_as: cli.save_as, dry: cli.dry_run };
    match (cli.command_id, cli.command) {
        (Some(_), Some(_)) => Err("Use --command or a subcommand, not both.".into()),
        (Some(id), None) => {
            let params = parse_params(&[], cli.params.as_deref())?;
            target.run(&id, params)
        }
        (None, Some(command)) => run_command(&mut target, command),
        (None, None) => {
            Cli::command().print_help().map_err(|e| e.to_string())?;
            println!();
            Ok(())
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.bridge.is_some() && cli.project.is_some() {
        eprintln!("--project opens a headless session; with --bridge use exec project.open path=...");
        std::process::exit(2);
    }
    if let Err(err) = run(cli) {
        eprintln!("{}", json!({ "ok": false, "error": err }));
        std::process::exit(1);
    }
}
