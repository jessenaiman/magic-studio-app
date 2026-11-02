//! `magic-cli`: operate Magic Studio from a terminal or an agent. Every operation is an
//! engine registry command; this binary only chooses where it runs (the caller's window or
//! the engine in-process) and prints JSON.
mod audit;
mod mcp;
mod project;
mod session;
mod studio;

use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use session::Session;
use std::io::BufRead;
use std::path::PathBuf;

/// Operate installed Magic Studio: projects, storyboard, and the desktop app.
///
/// Open an existing project window:
///   magic-cli studio open /absolute/Song.magic
/// Read its storyboard through the shared engine:
///   magic-cli --project /absolute/Song.magic storyboard read
///
/// Any registry command also works as `magic-cli <domain> <verb> key=value ...`
/// (underscores may be written as hyphens) or `magic-cli <domain.verb> ...`.
/// Use key=value arguments or --params JSON. `magic-cli commands [FILTER]` lists IDs.
#[derive(Parser)]
#[command(name = "magic-cli", version, verbatim_doc_comment)]
struct Cli {
    /// JSON output (also the default)
    #[arg(long, global = true)]
    json: bool,
    /// Select the intended .magic project
    #[arg(long, global = true)]
    project: Option<String>,
    /// Separate caller session binding
    #[arg(long, global = true, env = "MAGIC_SESSION_FILE")]
    session_file: Option<PathBuf>,
    /// Preview mutations without applying them
    #[arg(long, global = true)]
    dry_run: bool,
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// List native command IDs and parameters; optional FILTER matches registry text.
    Commands { filter: Option<String> },
    /// Execute COMMAND as a native registry ID with key=value or --params JSON.
    Exec {
        command: String,
        /// JSON parameter object
        #[arg(long)]
        params: Option<String>,
        arguments: Vec<String>,
    },
    /// Run JSONL {id, params} steps in order; stop on error. Earlier steps remain applied.
    Run {
        #[arg(default_value = "-")]
        file: String,
    },
    /// Serve Magic commands over MCP stdio (official rmcp SDK).
    Mcp,
    /// Run the repository's own checks and print what they print.
    ///
    /// Default order: Ruff, cargo clippy, ESLint, Stylelint, Lizard, Wallace, jscpd,
    /// PurgeCSS, scope. Each tool uses PATH (or root node_modules/.bin) and the nearest
    /// repository configuration at or above PATH, up to the repository top. Missing
    /// tools/configuration are errors, not passing or skipped checks.
    ///
    /// Tool output reaches this terminal exactly as the tool wrote it. This command only
    /// decides what to run and where; it does not parse, score, rank or summarize findings.
    ///
    /// Lizard takes its thresholds from code-limits.json. Each native exit code is
    /// printed; the command returns the first nonzero status, or 0. Setup errors use 2;
    /// signals use 128 + signal. Global --dry-run prints commands without running them.
    /// A file defaults to Lizard; a Rust file such as module.rs also scans module/.
    #[command(verbatim_doc_comment)]
    Audit {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Select a native tool; repeat to choose several. Default: Lizard for a file,
        /// every tool for a directory.
        #[arg(long = "tool", value_parser = audit_tool)]
        tools: Vec<String>,
        /// Git revision the scope is measured from.
        #[arg(long, default_value = "HEAD")]
        base: String,
    },
    /// Select a desktop window; list inspects responsive windows without rebinding.
    Studio {
        #[command(subcommand)]
        action: StudioCmd,
    },
    /// Inspect or clear this caller's project/window binding.
    Session {
        #[command(subcommand)]
        action: SessionCmd,
    },
    #[command(external_subcommand)]
    Registry(Vec<String>),
}

#[derive(Subcommand)]
enum StudioCmd {
    /// Open/focus PROJECT, an existing .magic file; omit it for a blank window.
    Open { project: Option<String> },
    /// List responsive windows without changing the current selection.
    List,
    /// List HyperFrames preview servers and which have no open Studio window.
    ///
    /// One server runs per project whose Studio target was started; closing the window
    /// does not stop it. Without --stop-orphans nothing is changed.
    Previews {
        /// Stop preview servers whose project has no open Studio window.
        #[arg(long)]
        stop_orphans: bool,
    },
}

#[derive(Subcommand)]
enum SessionCmd {
    Status,
    Clear,
}

fn audit_tool(name: &str) -> Result<String, String> {
    let names: Vec<&str> = audit::TOOLS.iter().map(|t| t.name).collect();
    if names.contains(&name) {
        Ok(name.to_owned())
    } else {
        Err(format!("choose from {}", names.join(", ")))
    }
}

fn emit(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

/// key=value tokens (values parsed as JSON when they are JSON) over an optional JSON object.
fn parse_params(tokens: &[String], raw: Option<&str>) -> Result<Value, String> {
    let mut params = match raw {
        Some(text) => serde_json::from_str::<Value>(text).map_err(|e| format!("--params: {e}"))?,
        None => json!({}),
    };
    if !params.is_object() {
        return Err("--params must be a JSON object".into());
    }
    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("Expected key=value, got {token:?}; see --help"))?;
        if key.is_empty() {
            return Err("Parameter name cannot be empty".into());
        }
        params[key] = serde_json::from_str(value).unwrap_or_else(|_| json!(value));
    }
    Ok(params)
}

fn run_one(s: &mut Session, id: &str, params: Value, dry_run: bool) -> Result<Value, String> {
    let js = params["js"].as_str().map(str::to_owned);
    let mut result = s.execute(id, params, dry_run)?;
    if id == "ui.eval"
        && result.get("value").is_some_and(Value::is_null)
        && js.is_some_and(|j| !j.contains("return "))
    {
        result["hint"] = json!(
            "ui.eval wraps your script in `async () => { ... }`; prefix the expression with `return ` to receive its value."
        );
    }
    Ok(result)
}

fn ids() -> Vec<&'static str> {
    magicstudio_engine::command_specs()
        .iter()
        .map(|c| c.id)
        .collect()
}

/// Map `words read k=v` / `words.read k=v` / `words studio-track` onto a registry ID and
/// its remaining arguments: the longest run of leading words that names a command.
fn resolve_registry(tokens: &[String]) -> Result<(String, Vec<String>, bool), String> {
    let known = ids();
    let words: Vec<&String> = tokens
        .iter()
        .take_while(|t| !t.contains('=') && !t.starts_with('-') && !t.starts_with('{'))
        .collect();
    let id_of = |n: usize| {
        words[..n]
            .iter()
            .map(|w| w.replace('-', "_"))
            .collect::<Vec<_>>()
            .join(".")
    };
    for n in (1..=words.len()).rev() {
        let id = id_of(n);
        if known.contains(&id.as_str()) {
            let rest = tokens[n..].to_vec();
            let help = rest.iter().any(|t| t == "--help" || t == "-h");
            return Ok((id, rest, help));
        }
    }
    let prefix = id_of(words.len());
    let children: Vec<&str> = known
        .iter()
        .copied()
        .filter(|k| k.starts_with(&format!("{prefix}.")))
        .collect();
    if !prefix.is_empty() && !children.is_empty() {
        return Err(format!(
            "{prefix} commands from the application registry:\n  {}\nNext: magic-cli {} <verb> --help",
            children.join("\n  "),
            prefix.replace('.', " ").replace('_', "-")
        ));
    }
    let close: Vec<&str> = known
        .iter()
        .copied()
        .filter(|k| {
            words
                .first()
                .is_some_and(|w| k.contains(&w.replace('-', "_")))
        })
        .take(3)
        .collect();
    let mut message = format!("No such command '{}'.", tokens.join(" "));
    if !close.is_empty() {
        message += &format!(" Did you mean: {}?", close.join(", "));
    }
    message += " Next: run 'magic-cli commands [FILTER]' or 'magic-cli --help'.";
    Err(message)
}

fn registry_help(id: &str) -> Value {
    magicstudio_engine::find_command(id).map_or(Value::Null, |c| c.describe())
}

/// Split `--params JSON` out of a registry command's arguments.
fn split_params(rest: &[String]) -> (Vec<String>, Option<String>) {
    let mut args = Vec::new();
    let mut raw = None;
    let mut it = rest.iter();
    while let Some(token) = it.next() {
        if token == "--params" {
            raw = it.next().cloned();
        } else if let Some(v) = token.strip_prefix("--params=") {
            raw = Some(v.to_owned());
        } else {
            args.push(token.clone());
        }
    }
    (args, raw)
}

fn run_jsonl(s: &mut Session, file: &str, dry_run: bool) -> Result<(), String> {
    let reader: Box<dyn BufRead> = if file == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::io::BufReader::new(
            std::fs::File::open(file).map_err(|e| format!("{file}: {e}"))?,
        ))
    };
    for (n, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let item: Value =
            serde_json::from_str(trimmed).map_err(|e| format!("line {}: {e}", n + 1))?;
        let id = item["id"]
            .as_str()
            .or_else(|| item["command"].as_str())
            .unwrap_or_default();
        let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
        let result = run_one(s, id, params, dry_run)?;
        println!("{}", json!({"line": n + 1, "ok": true, "result": result}));
    }
    Ok(())
}

fn run_registry(s: &mut Session, tokens: &[String], dry: bool) -> Result<(), String> {
    match resolve_registry(tokens) {
        Ok((id, _, true)) => emit(&registry_help(&id)),
        Ok((id, rest, false)) => {
            let (args, raw) = split_params(&rest);
            emit(&run_one(s, &id, parse_params(&args, raw.as_deref())?, dry)?);
        }
        Err(listing) if listing.contains("commands from the application registry") => {
            println!("{listing}");
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

fn run_studio(s: &mut Session, action: StudioCmd, dry: bool) -> Result<(), String> {
    match action {
        StudioCmd::Open { project } if dry => emit(&json!({"dry_run": true, "project": project})),
        StudioCmd::Open { project } => emit(&s.open_studio(project.as_deref())?),
        StudioCmd::List => emit(&studio::discover()),
        StudioCmd::Previews { stop_orphans } => {
            let servers = studio::previews();
            let stopped = if stop_orphans {
                studio::stop_orphans(&servers)
            } else {
                Vec::new()
            };
            emit(&json!({"servers": servers, "stopped": stopped}));
        }
    }
    Ok(())
}

fn run_session(s: &mut Session, action: SessionCmd) -> Result<(), String> {
    match action {
        SessionCmd::Status => emit(&s.status()),
        SessionCmd::Clear => {
            s.clear()?;
            emit(&json!({"ok": true}));
        }
    }
    Ok(())
}

fn run_command(s: &mut Session, command: Cmd, dry: bool) -> Result<(), String> {
    match command {
        Cmd::Commands { filter } => {
            let params = json!({"filter": filter.unwrap_or_default()});
            emit(&run_one(s, "command.list", params, dry)?);
        }
        Cmd::Exec {
            command,
            params,
            arguments,
        } => {
            let id = command.strip_prefix("id=").unwrap_or(&command).to_owned();
            emit(&run_one(
                s,
                &id,
                parse_params(&arguments, params.as_deref())?,
                dry,
            )?);
        }
        Cmd::Run { file } => run_jsonl(s, &file, dry)?,
        Cmd::Studio { action } => run_studio(s, action, dry)?,
        Cmd::Session { action } => run_session(s, action)?,
        Cmd::Registry(tokens) => run_registry(s, &tokens, dry)?,
        Cmd::Mcp | Cmd::Audit { .. } => unreachable!("handled before a session is loaded"),
    }
    Ok(())
}

fn run(cli: Cli) -> Result<(), String> {
    // The audit reads the repository, never a project or window: no session needed.
    if let Some(Cmd::Audit { path, tools, base }) = &cli.command {
        let target = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
        std::process::exit(audit::run(&target, tools, cli.dry_run, base));
    }
    let mcp = matches!(cli.command, Some(Cmd::Mcp));
    let mut s = Session::load(cli.session_file, if mcp { "MCP" } else { "CLI" })?;
    if let Some(project) = &cli.project {
        s.execute("project.open", json!({"path": project}), cli.dry_run)?;
    }
    match cli.command {
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().map_err(|e| e.to_string())
        }
        Some(Cmd::Mcp) => mcp::serve(s, cli.dry_run),
        Some(command) => run_command(&mut s, command, cli.dry_run),
    }
}

fn shell_word(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:@".contains(c))
    {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// A failure is a magic-cli gap: ask for a fix instead of working around it.
fn fix_routes(error: &str) -> String {
    let command = std::iter::once("magic-cli".to_owned())
        .chain(std::env::args().skip(1))
        .map(|a| shell_word(&a))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "Command:\n    {command}\n\nError:\n    {}",
        error.chars().take(1500).collect::<String>()
    );
    let title = format!(
        "magic-cli fails: {}",
        command.chars().take(70).collect::<String>()
    );
    let issue = ["gh", "issue", "create", "--title", &title, "--body", &body]
        .iter()
        .map(|a| shell_word(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "Request a magic-cli fix (do not work around it with the tool directly):\n  quick       small and in scope: fix this magic-cli command now\n  issue       {issue}\n  brainstorm  unsure which designer step or tool applies: ask Jesse first"
    )
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("magic-cli: {error}\n\n{}", fix_routes(&error));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn domain_verb_and_dotted_ids_resolve_to_the_same_command() {
        assert_eq!(
            resolve_registry(&toks("words read")).unwrap().0,
            "words.read"
        );
        assert_eq!(
            resolve_registry(&toks("words.read")).unwrap().0,
            "words.read"
        );
        let (id, rest, _) = resolve_registry(&toks("words studio-track a=1")).unwrap();
        assert_eq!((id.as_str(), rest), ("words.studio_track", toks("a=1")));
    }

    #[test]
    fn a_domain_alone_lists_its_commands() {
        let err = resolve_registry(&toks("words")).unwrap_err();
        assert!(err.contains("words.read"), "{err}");
    }

    #[test]
    fn params_merge_json_and_key_values() {
        let p = parse_params(&toks("n=2 name=fish flag=true"), Some(r#"{"a":1}"#)).unwrap();
        assert_eq!(p, json!({"a":1,"n":2,"name":"fish","flag":true}));
        assert!(parse_params(&toks("novalue"), None).is_err());
        assert!(parse_params(&[], Some("[1]")).is_err());
    }

    #[test]
    fn params_flag_is_split_from_key_values() {
        let (args, raw) = split_params(&toks(r#"a=1 --params {"b":2}"#));
        assert_eq!((args, raw.as_deref()), (toks("a=1"), Some(r#"{"b":2}"#)));
    }
}
