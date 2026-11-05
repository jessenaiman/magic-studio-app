//! `magic-cli`: operate Magic Studio from a terminal or an agent. Every operation is an
//! engine registry command; this binary only chooses where it runs (the caller's window or
//! the engine in-process) and prints JSON.
mod project;
mod session;
mod studio;

use clap::{Args, Subcommand};
use serde_json::{Value, json};
use session::Session;
use std::io::BufRead;
use std::path::PathBuf;

#[derive(Args)]
pub struct CommandArgs {
    /// JSON output (also the default)
    #[arg(long, global = true)]
    json: bool,
    /// Select the intended .magic project
    #[arg(long, global = true)]
    pub project: Option<String>,
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
    Open {
        // Its own id: sharing `project` with the global --project would also select it.
        #[arg(id = "studio_project", value_name = "PROJECT")]
        project: Option<String>,
    },
    /// Open PROJECT in Animation only: no song, storyboard, lyrics or film tabs.
    ///
    /// A missing PROJECT is created as an animation study (`project.new mode=animation`).
    /// A song project opens with only its Animation workspace. A window already serving
    /// the project is focused and switched to Animation only.
    Animate {
        #[arg(id = "studio_project", value_name = "PROJECT")]
        project: String,
    },
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
    magic_engine::command_specs().iter().map(|c| c.id).collect()
}

/// Map `words read k=v` / `words.read k=v` / `words studio-track` onto a registry ID and
/// its remaining arguments: the longest run of leading words that names a command.
fn resolve_registry(
    program: &str,
    tokens: &[String],
) -> Result<(String, Vec<String>, bool), String> {
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
            "{prefix} commands from the application registry:\n  {}\nNext: {program} {} <verb> --help",
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
    message += &format!(" Next: run '{program} commands [FILTER]' or '{program} --help'.");
    Err(message)
}

fn registry_help(id: &str) -> Value {
    magic_engine::find_command(id).map_or(Value::Null, |c| c.describe())
}

/// Split `--params JSON` out of a registry command's arguments.
fn split_params(rest: &[String]) -> Result<(Vec<String>, Option<String>), String> {
    let mut args = Vec::new();
    let mut raw = None;
    let mut it = rest.iter();
    while let Some(token) = it.next() {
        if token == "--params" {
            // A missing value is a malformed call, never a request to run with defaults.
            raw = Some(it.next().cloned().ok_or("--params needs a JSON object")?);
        } else if let Some(v) = token.strip_prefix("--params=") {
            raw = Some(v.to_owned());
        } else {
            args.push(token.clone());
        }
    }
    Ok((args, raw))
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

fn run_registry(
    program: &str,
    s: &mut Session,
    tokens: &[String],
    dry: bool,
) -> Result<(), String> {
    match resolve_registry(program, tokens) {
        Ok((id, _, true)) => emit(&registry_help(&id)),
        Ok((id, rest, false)) => {
            let (args, raw) = split_params(&rest)?;
            let params = parse_params(&args, raw.as_deref())?;
            let numbered = id == "resources.workflow" && params["steps"] == true && !dry;
            let value = run_one(s, &id, params, dry)?;
            if numbered {
                for (i, step) in value["steps"].as_array().into_iter().flatten().enumerate() {
                    println!("{}. {}", i + 1, step.as_str().unwrap_or_default());
                }
            } else {
                emit(&value);
            }
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
        StudioCmd::Animate { project } if dry => {
            emit(&json!({"dry_run": true, "project": project, "animation_only": true}))
        }
        StudioCmd::Animate { project } => emit(&s.open_animation(&project)?),
        StudioCmd::List => emit(&studio::discover()),
        StudioCmd::Previews { stop_orphans } => {
            let mut report = studio::previews();
            let candidates =
                studio::orphans(report["servers"].as_array().map_or(&[][..], Vec::as_slice));
            // --dry-run names what would stop; nothing is signalled.
            if stop_orphans && dry {
                report["would_stop"] = json!(candidates);
            } else {
                report["stopped"] = json!(if stop_orphans {
                    studio::stop(&candidates)
                } else {
                    Vec::new()
                });
            }
            emit(&report);
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

fn run_command(program: &str, s: &mut Session, command: Cmd, dry: bool) -> Result<(), String> {
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
        Cmd::Registry(tokens) => run_registry(program, s, &tokens, dry)?,
    }
    Ok(())
}

impl CommandArgs {
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }
}

pub fn run(cli: CommandArgs, mut help: clap::Command) -> Result<(), String> {
    let mut s = Session::load(cli.session_file, "CLI")?;
    // Selecting the project is always real; --dry-run previews the command that follows.
    if let Some(project) = &cli.project {
        s.execute("project.open", json!({"path": project}), false)?;
    }
    match cli.command {
        None => help.print_help().map_err(|e| e.to_string()),
        Some(command) => run_command(help.get_name(), &mut s, command, cli.dry_run),
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
fn fix_routes(program: &str, error: &str) -> String {
    let command = std::iter::once(program.to_owned())
        .chain(std::env::args().skip(1))
        .map(|a| shell_word(&a))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "Command:\n    {command}\n\nError:\n    {}",
        error.chars().take(1500).collect::<String>()
    );
    let title = format!(
        "{program} fails: {}",
        command.chars().take(70).collect::<String>()
    );
    let issue = ["gh", "issue", "create", "--title", &title, "--body", &body]
        .iter()
        .map(|a| shell_word(a))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "Request a {program} fix (do not work around it with the tool directly):\n  quick       small and in scope: fix this {program} command now\n  issue       {issue}\n  brainstorm  unsure which designer step or tool applies: ask Jesse first"
    )
}

pub fn finish(program: &str, result: Result<(), String>) {
    if let Err(error) = result {
        eprintln!("{program}: {error}\n\n{}", fix_routes(program, &error));
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
            resolve_registry("magic-cli", &toks("words read"))
                .unwrap()
                .0,
            "words.read"
        );
        assert_eq!(
            resolve_registry("magic-cli", &toks("words.read"))
                .unwrap()
                .0,
            "words.read"
        );
        let (id, rest, _) = resolve_registry("magic-cli", &toks("words studio-track a=1")).unwrap();
        assert_eq!((id.as_str(), rest), ("words.studio_track", toks("a=1")));
    }

    #[test]
    fn a_domain_alone_lists_its_commands() {
        let err = resolve_registry("magic-cli", &toks("words")).unwrap_err();
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
        let (args, raw) = split_params(&toks(r#"a=1 --params {"b":2}"#)).unwrap();
        assert_eq!((args, raw.as_deref()), (toks("a=1"), Some(r#"{"b":2}"#)));
        assert!(split_params(&toks("a=1 --params")).is_err());
    }
}
