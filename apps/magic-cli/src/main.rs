//! `magic-cli`: thin headless CLI over `magic::Session` (modeled on `filmcraft-cli`).
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};
use std::io::BufRead;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "magic-cli",
    about = "Drive Magic Studio from the shell via magic"
)]
pub struct Cli {
    /// Open this .magic project first (or use the last opened project from session file)
    #[arg(long, global = true)]
    pub project: Option<String>,
    /// Connect to a running `magic-tauri --control <PORT>` window
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
    /// List engine commands (id, label, params)
    Commands { filter: Option<String> },
    /// Describe one command as JSON
    Describe { id: String },
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
        /// Live window action (`inspect`, `state`, `feedback`, `unconfigured`, `elements`, `focus`, `view`, `quit`)
        action: String,
        /// Optional argument for `ui focus <id>` or `ui view <mode>`
        target: Option<String>,
        /// JSON parameters for the live UI action
        #[arg(long)]
        params: Option<String>,
        /// Additional parameters as key=value pairs
        arguments: Vec<String>,
    },
    /// Work on a storyboard document built from the neutral template
    Storyboard {
        /// The project whose storyboard is the underlying document
        #[arg(long, global = true)]
        project: Option<String>,
        /// The document itself, instead of the project's storyboard
        #[arg(long, global = true)]
        document: Option<String>,
        #[command(subcommand)]
        command: SbCmd,
    },
    #[command(external_subcommand)]
    Registry(Vec<String>),
}

/// The storyboard surface: add and edit the template's pages and sections.
///
/// Every command edits labelled slots in a `.designcraft` document, so the page layout stays
/// fixed while the text and images inside it change.
#[derive(Subcommand)]
enum SbCmd {
    /// Create a storyboard from the template
    New {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        overwrite: bool,
    },
    /// List the pages, their archetype, section and slots
    Pages {
        #[arg(long)]
        path: Option<String>,
    },
    /// The song's verse and chorus, with each line's timing
    Cues {
        /// The canonical lyrics sheet (.json) — the artist's printed words
        #[arg(long)]
        lyrics: String,
        /// The transcript (.json) the timings come from
        #[arg(long)]
        transcript: Option<String>,
        /// Sections to pull, comma separated (default: every verse* and chorus* section)
        #[arg(long)]
        sections: Option<String>,
        /// Lines per scene cue
        #[arg(long, default_value_t = 1)]
        per: usize,
    },
    /// Run a command script against the storyboard
    Run {
        /// The script file, `-` for stdin, or the script text itself
        script: Option<String>,
    },
    /// List the fields still holding the template default, with the command to fill each
    Check {
        #[arg(long)]
        path: Option<String>,
    },
    /// List the labelled slots of one page
    Slots {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        scene: Option<usize>,
    },
    /// Set the text and images of one page
    Set {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        scene: Option<usize>,
        /// Text to set, as SLOT=VALUE (repeatable)
        #[arg(long = "text", value_name = "SLOT=VALUE")]
        text: Vec<String>,
        /// Image to place, as SLOT=PATH (repeatable)
        #[arg(long = "image", value_name = "SLOT=PATH")]
        image: Vec<String>,
        /// Image files for the contact strip (repeatable)
        #[arg(long = "ref", value_name = "PATH")]
        refs: Vec<String>,
    },
    /// Add a page copied from a template page
    Add {
        #[arg(long)]
        path: Option<String>,
        /// The template page to copy: cover, world, char or scene
        #[arg(long, default_value = "scene")]
        archetype: String,
        /// The page to copy from (default: the last page of that kind)
        #[arg(long)]
        from: Option<usize>,
        /// Where to put the new page
        #[arg(long)]
        at: Option<usize>,
        #[arg(long = "text", value_name = "SLOT=VALUE")]
        text: Vec<String>,
        #[arg(long = "image", value_name = "SLOT=PATH")]
        image: Vec<String>,
        #[arg(long = "ref", value_name = "PATH")]
        refs: Vec<String>,
    },
    /// Add a section (a parent page) and apply it to pages
    Section {
        #[arg(long)]
        path: Option<String>,
        /// The section prefix, e.g. B
        #[arg(long)]
        name: String,
        /// The section it inherits from, e.g. A-Parent
        #[arg(long)]
        based_on: Option<String>,
        /// Pages to apply it to (repeatable)
        #[arg(long = "page")]
        pages: Vec<usize>,
    },
    /// Overlay new art on a slot, keeping the original underneath for review
    Overlay {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        label: Option<String>,
        /// The image slot to cover
        #[arg(long)]
        slot: String,
        #[arg(long)]
        image: String,
        /// The layer the revision sits on
        #[arg(long)]
        layer: Option<String>,
        /// Leave the original visible under the new art
        #[arg(long)]
        keep_original: bool,
    },
    /// Render one page to PNG
    Render {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        page: Option<usize>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        scene: Option<usize>,
        #[arg(long)]
        out: String,
        #[arg(long, default_value_t = 0.5)]
        scale: f64,
    },
    /// Export the storyboard to PDF and/or PNG pages
    Export {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        pdf: Option<String>,
        #[arg(long)]
        png_dir: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
    },
}

/// The engine command and parameters a storyboard request maps to.
///
/// `document` is the storyboard itself, given once for the whole `storyboard` group; a command's
/// own `--path` wins over it.
fn storyboard_params(command: &SbCmd, document: Option<&str>) -> Result<(String, Value), String> {
    let mut p = serde_json::Map::new();
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            p.insert(key.to_string(), value);
        }
    };
    // `SLOT=VALUE` pairs become an object of slot names to values.
    let pairs = |list: &[String]| -> Value {
        let mut map = serde_json::Map::new();
        for item in list {
            let Some((slot, value)) = item.split_once('=') else {
                continue;
            };
            map.insert(slot.to_string(), Value::String(value.to_string()));
        }
        Value::Object(map)
    };
    let id;
    match command {
        SbCmd::New { path, overwrite } => {
            id = "storyboard.new".into();
            put("path", path.clone().map(Value::String));
            put("overwrite", Some(Value::Bool(*overwrite)));
        }
        SbCmd::Pages { path } => {
            id = "storyboard.pages".into();
            put("path", path.clone().map(Value::String));
        }
        SbCmd::Cues {
            lyrics,
            transcript,
            sections,
            per,
        } => {
            id = "storyboard.cues".into();
            put("lyrics", Some(Value::String(lyrics.clone())));
            put("transcript", transcript.clone().map(Value::String));
            put("sections", sections.clone().map(Value::String));
            put("per", Some(json!(per)));
        }
        SbCmd::Run { script } => {
            id = "storyboard.run".into();
            let script = match script {
                Some(s) if s == "-" => {
                    let mut text = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                        .map_err(|e| format!("stdin: {e}"))?;
                    text
                }
                Some(s) => {
                    let path = Path::new(&s);
                    if path.is_file() {
                        std::fs::read_to_string(path).map_err(|e| format!("{s}: {e}"))?
                    } else {
                        s.clone()
                    }
                }
                None => String::new(),
            };
            put("script", Some(Value::String(script)));
        }
        SbCmd::Check { path } => {
            id = "storyboard.check".into();
            put("path", path.clone().map(Value::String));
        }
        SbCmd::Slots {
            path,
            page,
            label,
            scene,
        } => {
            id = "storyboard.slots".into();
            put("path", path.clone().map(Value::String));
            put("page", page.map(|n| json!(n)));
            put("label", label.clone().map(Value::String));
            put("scene", scene.map(|n| json!(n)));
        }
        SbCmd::Set {
            path,
            page,
            label,
            scene,
            text,
            image,
            refs,
        } => {
            id = "storyboard.set".into();
            put("path", path.clone().map(Value::String));
            put("page", page.map(|n| json!(n)));
            put("label", label.clone().map(Value::String));
            put("scene", scene.map(|n| json!(n)));
            if !text.is_empty() {
                p.insert("fields".into(), pairs(text));
            }
            if !image.is_empty() {
                p.insert("images".into(), pairs(image));
            }
            if !refs.is_empty() {
                p.insert(
                    "refs".into(),
                    Value::Array(refs.iter().map(|r| Value::String(r.clone())).collect()),
                );
            }
        }
        SbCmd::Add {
            path,
            archetype,
            from,
            at,
            text,
            image,
            refs,
        } => {
            id = "storyboard.add".into();
            put("path", path.clone().map(Value::String));
            put("archetype", Some(Value::String(archetype.clone())));
            put("from", from.map(|n| json!(n)));
            put("at", at.map(|n| json!(n)));
            if !text.is_empty() {
                p.insert("fields".into(), pairs(text));
            }
            if !image.is_empty() {
                p.insert("images".into(), pairs(image));
            }
            if !refs.is_empty() {
                p.insert(
                    "refs".into(),
                    Value::Array(refs.iter().map(|r| Value::String(r.clone())).collect()),
                );
            }
        }
        SbCmd::Section {
            path,
            name,
            based_on,
            pages,
        } => {
            id = "storyboard.section".into();
            put("path", path.clone().map(Value::String));
            put("name", Some(Value::String(name.clone())));
            put("based_on", based_on.clone().map(Value::String));
            if !pages.is_empty() {
                p.insert(
                    "pages".into(),
                    Value::Array(pages.iter().map(|n| json!(n)).collect()),
                );
            }
        }
        SbCmd::Overlay {
            path,
            page,
            label,
            slot,
            image,
            layer,
            keep_original,
        } => {
            id = "storyboard.overlay".into();
            put("path", path.clone().map(Value::String));
            put("page", page.map(|n| json!(n)));
            put("label", label.clone().map(Value::String));
            put("slot", Some(Value::String(slot.clone())));
            put("image", Some(Value::String(image.clone())));
            put("layer", layer.clone().map(Value::String));
            put("hide_original", Some(Value::Bool(!*keep_original)));
        }
        SbCmd::Render {
            path,
            page,
            label,
            scene,
            out,
            scale,
        } => {
            id = "storyboard.thumbnail".into();
            put("path", path.clone().map(Value::String));
            put("page", page.map(|n| json!(n)));
            put("label", label.clone().map(Value::String));
            put("scene", scene.map(|n| json!(n)));
            put("out", Some(Value::String(out.clone())));
            put("scale", Some(json!(scale)));
        }
        SbCmd::Export {
            path,
            pdf,
            png_dir,
            scale,
        } => {
            id = "storyboard.export".into();
            put("path", path.clone().map(Value::String));
            put("pdf", pdf.clone().map(Value::String));
            put("png_dir", png_dir.clone().map(Value::String));
            put("scale", Some(json!(scale)));
        }
    }
    // The document given once for the group applies to every command that did not name one.
    if !p.contains_key("path")
        && let Some(document) = document
    {
        p.insert("path".into(), Value::String(document.to_string()));
    }
    Ok((id, Value::Object(p)))
}

fn session_file() -> PathBuf {
    std::env::var_os("MAGIC_SESSION_FILE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".config/magic/cli_session.json"))
        })
        .unwrap_or_else(|| PathBuf::from(".magic_session.json"))
}

fn load_saved_project() -> Option<String> {
    let text = std::fs::read_to_string(session_file()).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .get("project")?
        .as_str()
        .map(str::to_owned)
}

fn save_project(path: Option<&str>) {
    let file = session_file();
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&file, json!({ "project": path }).to_string());
}

fn emit(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
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
        let (k, v) = token
            .split_once('=')
            .ok_or_else(|| format!("Expected key=value, got {token:?}"))?;
        params[k] = serde_json::from_str(v).unwrap_or_else(|_| json!(v));
    }
    Ok(params)
}

fn control_address(spec: &str) -> String {
    if spec.contains(':') {
        spec.to_owned()
    } else {
        format!("127.0.0.1:{spec}")
    }
}

fn call_live_control(bridge: Option<&str>, method: &str, params: Value) -> Result<Value, String> {
    let spec = bridge.ok_or("Live UI commands require --bridge ADDRESS")?;
    let mut remote = magic_mcp::Remote::new(
        control_address(spec),
        std::env::var("MAGIC_CONTROL_TOKEN").ok(),
    );
    magic_mcp::Backend::execute(&mut remote, method, params)
}

fn exec_one(s: &mut magic::Session, id: &str, params: Value, dry: bool) -> Result<Value, String> {
    if dry {
        return Ok(json!({ "dry_run": true, "id": id, "params": params }));
    }
    let res = s.execute(id, params).map_err(|e| e.to_string())?;
    if id == "project.open" || id == "project.new" {
        if let Some(p) = s.project_path.as_ref().and_then(|p| p.to_str()) {
            save_project(Some(p));
        }
    } else if id == "project.close" {
        save_project(None);
    }
    Ok(res)
}

fn dispatch(
    session: &mut Option<magic::Session>,
    bridge: Option<&str>,
    id: &str,
    params: Value,
    dry: bool,
) -> Result<Value, String> {
    if dry {
        Ok(json!({ "dry_run": true, "id": id, "params": params }))
    } else if bridge.is_some() || id.starts_with("ui.") {
        call_live_control(bridge, id, params)
    } else {
        exec_one(
            session.as_mut().expect("headless session"),
            id,
            params,
            false,
        )
    }
}

fn ui_action(
    action: &str,
    target: Option<String>,
    mut args: Vec<String>,
    raw: Option<&str>,
) -> Result<(String, Value), String> {
    let method = match action {
        "inspect" | "state" => "ui.state".into(),
        "feedback" | "look" => "ui.feedback".into(),
        "unconfigured" | "empty" => "ui.unconfigured".into(),
        "view" => "ui.set_view".into(),
        "quit" => "app.quit".into(),
        id if id.contains('.') => id.to_owned(),
        id => format!("ui.{id}"),
    };
    if let Some(target) = target {
        match method.as_str() {
            "ui.focus" | "ui.click" if !target.contains('=') && !target.starts_with('{') => {
                args.insert(0, format!("id={target}"))
            }
            "ui.set_view" if !target.contains('=') && !target.starts_with('{') => {
                args.insert(0, format!("view={target}"))
            }
            _ => args.insert(0, target),
        }
    }
    Ok((method, parse_params(&args, raw)?))
}

fn resolve_shorthand(tokens: &[String]) -> Result<(String, Vec<String>), String> {
    if tokens.first().map(String::as_str) == Some("studio") {
        return match tokens.get(1).map(String::as_str) {
            Some("list") => Ok(("project.catalog".into(), vec![])),
            Some("open") => {
                let rest = tokens
                    .get(2)
                    .map(|p| vec![format!("path={p}")])
                    .unwrap_or_default();
                Ok(("project.open".into(), rest))
            }
            _ => Ok(("project.catalog".into(), vec![])),
        };
    }
    let known: Vec<&str> = magic::command_specs().iter().map(|c| c.id).collect();
    let words: Vec<&String> = tokens
        .iter()
        .take_while(|t| !t.contains('=') && !t.starts_with('-') && !t.starts_with('{'))
        .collect();
    for n in (1..=words.len()).rev() {
        let id = words[..n]
            .iter()
            .map(|w| w.replace('-', "_"))
            .collect::<Vec<_>>()
            .join(".");
        if known.contains(&id.as_str()) {
            return Ok((id, tokens[n..].to_vec()));
        }
    }
    Err(format!(
        "Unknown command '{}'. Run `magic-cli commands`.",
        tokens.join(" ")
    ))
}

fn main() {
    let cli = Cli::parse();
    let live = cli.bridge.is_some();
    if live && cli.project.is_some() {
        eprintln!(
            "--project opens a headless session; with --bridge use exec project.open path=..."
        );
        std::process::exit(2);
    }
    let mut session = (!live).then(magic::Session::user_session);
    if !live
        && let Some(proj) = cli.project.clone().or_else(load_saved_project)
        && std::path::Path::new(&proj).is_file()
    {
        let _ = session
            .as_mut()
            .expect("headless session")
            .execute("project.open", json!({ "path": proj }));
        if cli.project.is_some() {
            save_project(Some(&proj));
        }
    }
    let outcome: Result<(), String> = (|| {
        let bridge = cli.bridge.as_deref();
        if let Some(id) = cli.command_id.as_deref() {
            if cli.command.is_some() {
                return Err("Use --command or a subcommand, not both.".into());
            }
            let params = parse_params(&[], cli.params.as_deref())?;
            emit(&dispatch(&mut session, bridge, id, params, cli.dry_run)?);
            return Ok(());
        }
        match cli.command {
            None => {
                Cli::command().print_help().map_err(|e| e.to_string())?;
                println!();
            }
            Some(Cmd::Commands { filter }) => {
                emit(&dispatch(
                    &mut session,
                    bridge,
                    "command.list",
                    json!({ "filter": filter.unwrap_or_default() }),
                    false,
                )?);
            }
            Some(Cmd::Describe { id }) => {
                let v = magic::find_command(&id)
                    .map(|c| c.describe())
                    .ok_or_else(|| format!("Unknown command '{id}'"))?;
                emit(&v);
            }
            Some(Cmd::Exec {
                command,
                params,
                arguments,
            }) => {
                let p = parse_params(&arguments, params.as_deref())?;
                let out = dispatch(&mut session, bridge, &command, p, cli.dry_run)?;
                emit(&out);
            }
            Some(Cmd::Run { file }) => {
                let reader: Box<dyn BufRead> = if file == "-" {
                    Box::new(std::io::stdin().lock())
                } else {
                    Box::new(std::io::BufReader::new(
                        std::fs::File::open(&file).map_err(|e| format!("{file}: {e}"))?,
                    ))
                };
                for (n, line) in reader.lines().enumerate() {
                    let line = line.map_err(|e| e.to_string())?;
                    let t = line.trim();
                    if t.is_empty() || t.starts_with('#') {
                        continue;
                    }
                    let item: Value =
                        serde_json::from_str(t).map_err(|e| format!("line {}: {e}", n + 1))?;
                    let id = item["id"]
                        .as_str()
                        .or_else(|| item["command"].as_str())
                        .unwrap_or_default();
                    let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
                    let res = dispatch(&mut session, bridge, id, params, cli.dry_run)?;
                    println!("{}", json!({ "line": n + 1, "ok": true, "result": res }));
                }
            }
            Some(Cmd::Mcp) => {
                if let Some(addr) = cli.bridge {
                    let mut srv = magic_mcp::Server::new(magic_mcp::Remote::new(
                        control_address(&addr),
                        std::env::var("MAGIC_CONTROL_TOKEN").ok(),
                    ));
                    srv.serve().map_err(|e| e.to_string())?;
                } else {
                    let mut srv = magic_mcp::Server::new(magic_mcp::Headless::new(
                        session.take().expect("headless session"),
                    ));
                    srv.serve().map_err(|e| e.to_string())?;
                }
            }
            Some(Cmd::Ui {
                action,
                target,
                params,
                arguments,
            }) => {
                if bridge.is_none() {
                    return Err("Live UI commands require --bridge ADDRESS".into());
                }
                let (method, params) = ui_action(&action, target, arguments, params.as_deref())?;
                let res = if cli.dry_run {
                    json!({ "dry_run": true, "id": method, "params": params })
                } else {
                    call_live_control(bridge, &method, params)?
                };
                emit(&res);
            }
            Some(Cmd::Storyboard {
                project,
                document,
                command,
            }) => {
                if bridge.is_some() && project.is_some() {
                    return Err("storyboard --project cannot be combined with --bridge; use exec project.open path=...".into());
                }
                // A project named here applies to this invocation only: it is not written to
                // the session file, so the next command still works on the same project.
                if let Some(proj) = project.as_deref()
                    && Path::new(proj).is_file()
                {
                    let _ = session
                        .as_mut()
                        .expect("headless session")
                        .execute("project.open", json!({ "path": proj }));
                }
                let (id, params) = storyboard_params(&command, document.as_deref())?;
                emit(&dispatch(&mut session, bridge, &id, params, cli.dry_run)?);
            }
            Some(Cmd::Registry(tokens)) => {
                let (id, rest) = resolve_shorthand(&tokens)?;
                if rest.iter().any(|t| t == "--help" || t == "-h")
                    && let Some(c) = magic::find_command(&id)
                {
                    emit(&c.describe());
                    return Ok(());
                }
                let mut kv = Vec::new();
                let mut raw = None;
                let mut it = rest.into_iter();
                while let Some(tok) = it.next() {
                    if tok == "--params" {
                        raw = it.next();
                    } else if let Some(v) = tok.strip_prefix("--params=") {
                        raw = Some(v.to_owned());
                    } else {
                        kv.push(tok);
                    }
                }
                let p = parse_params(&kv, raw.as_deref())?;
                emit(&dispatch(&mut session, bridge, &id, p, cli.dry_run)?);
            }
        }
        Ok(())
    })();
    if let Err(err) = outcome {
        eprintln!("{}", json!({ "ok": false, "error": err }));
        std::process::exit(1);
    }
}
