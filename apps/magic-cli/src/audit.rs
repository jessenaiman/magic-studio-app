//! Native repository checks, reported with unmodified tool output. Each tool is a command
//! that already exists, with the repository configuration it expects. This file only decides
//! what to run and where; nothing here parses, scores, ranks or summarizes what a tool prints.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Tool {
    pub name: &'static str,
    label: &'static str,
    command: &'static [&'static str],
    /// Exact file names, or a prefix ending in `*`.
    configs: &'static [&'static str],
}

/// Default order for a directory.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "clippy",
        label: "cargo clippy",
        command: &[
            "cargo",
            "clippy",
            "--workspace",
            "--lib",
            "--bins",
            "--locked",
            "--offline",
            "--",
            "-D",
            "warnings",
        ],
        configs: &["Cargo.toml"],
    },
    Tool {
        name: "eslint",
        label: "ESLint",
        command: &["eslint", "."],
        configs: &["eslint.config.js", "eslint.config.mjs", "eslint.config.cjs"],
    },
    Tool {
        name: "stylelint",
        label: "Stylelint",
        command: &["stylelint", "**/*.css"],
        configs: &["stylelint.config.*", ".stylelintrc*"],
    },
    Tool {
        name: "lizard",
        label: "Lizard",
        command: &["lizard", "."],
        configs: &["code-limits.json"],
    },
    Tool {
        name: "wallace",
        label: "Wallace",
        command: &["wallace"],
        configs: &[],
    },
    Tool {
        name: "jscpd",
        label: "jscpd",
        command: &["jscpd", "."],
        configs: &[".jscpd.json"],
    },
    Tool {
        name: "purgecss",
        label: "PurgeCSS",
        command: &["purgecss", "-c", "purgecss.config.cjs"],
        configs: &["purgecss.config.cjs"],
    },
    Tool {
        name: "scope",
        label: "git scope",
        command: &["git", "diff", "--stat=160"],
        configs: &[],
    },
];

/// Third-party stylesheets are measured by their own projects, not ours.
const FOREIGN: &[&str] = &["vendor", "node_modules", "target"];
/// A build emits `.js` beside the `.ts` it came from; measure the source.
const GENERATED_UI: &str = "apps/src-tauri/ui";

fn tool(name: &str) -> &'static Tool {
    TOOLS
        .iter()
        .find(|t| t.name == name)
        .expect("tool names come from TOOLS")
}

fn matches(file: &str, pattern: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => file.starts_with(prefix),
        None => file == pattern,
    }
}

fn has_config(dir: &Path, t: &Tool) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| {
            e.path().is_file()
                && t.configs
                    .iter()
                    .any(|p| matches(&e.file_name().to_string_lossy(), p))
        })
}

/// Nearest directory up to the repository top that holds the tool's config. Clippy stays on
/// the given crate directory; a tool without configuration runs from the repository top.
fn config_root(start: &Path, t: &Tool) -> PathBuf {
    if t.name == "clippy" {
        return start.to_path_buf();
    }
    for dir in start.ancestors() {
        if has_config(dir, t) {
            return dir.to_path_buf();
        }
        if dir.join(".git").exists() {
            return if t.configs.is_empty() {
                dir.to_path_buf()
            } else {
                start.to_path_buf()
            };
        }
    }
    start.to_path_buf()
}

fn walk(dir: &Path, base: &Path, suffix: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let foreign = path.strip_prefix(base).is_ok_and(|rel| {
            rel.components()
                .any(|c| FOREIGN.contains(&c.as_os_str().to_string_lossy().as_ref()))
        });
        if foreign {
            continue;
        }
        if path.is_dir() {
            walk(&path, base, suffix, out);
        } else if path.extension().is_some_and(|e| e == suffix) {
            out.push(path);
        }
    }
}

/// Our stylesheets under target, in stable order, without third-party CSS.
fn css_sources(target: &Path) -> Vec<PathBuf> {
    if target.is_file() {
        return vec![target.to_path_buf()];
    }
    let mut found = Vec::new();
    walk(target, target, "css", &mut found);
    found.sort();
    found
}

/// The thresholds the repository already declares in code-limits.json.
fn lizard_options(root: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(root.join("code-limits.json"))
        .map_err(|e| format!("code-limits.json: {e}"))?;
    let limits: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("code-limits.json: {e}"))?;
    if !limits.is_object() {
        return Err("code-limits.json must be an object".into());
    }
    let mut options: Vec<String> = ["-Ecognitive", "--sort", "cyclomatic_complexity"]
        .map(String::from)
        .to_vec();
    let fields = [
        ("max_function_ccn", None, "cyclomatic_complexity"),
        ("max_function_lines", Some(100), "length"),
        (
            "max_function_cognitive_complexity",
            Some(15),
            "cognitive_complexity",
        ),
    ];
    for (key, default, native) in fields {
        let limit = limits[key].as_u64().or(default).filter(|n| *n >= 1);
        let limit =
            limit.ok_or_else(|| format!("code-limits.json needs a positive integer {key}"))?;
        options.extend(["--Threshold".into(), format!("{native}={limit}")]);
    }
    Ok(options)
}

/// Skip `.js` a build emits beside its `.ts`. Lizard matches `-x` against the absolute
/// paths it walks, so the pattern is absolute too.
fn generated_excludes(root: &Path) -> Vec<String> {
    let dir = root.join(GENERATED_UI);
    let has_ts = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().extension().is_some_and(|x| x == "ts"));
    if has_ts {
        vec!["-x".into(), dir.join("*.js").to_string_lossy().into_owned()]
    } else {
        Vec::new()
    }
}

/// A file the user may execute; a non-executable file of the same name is skipped, as
/// `shutil.which` did.
fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

fn which(name: &str) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.is_absolute() {
        return runnable(candidate).then(|| candidate.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(name))
        .find(|p| runnable(p))
}

/// Point the native command at target when it is not the config root.
fn with_target(mut command: Vec<String>, root: &Path, target: &Path, name: &str) -> Vec<String> {
    let last = command.len() - 1;
    match name {
        "wallace" if target.is_file() => command.push(target.to_string_lossy().into_owned()),
        "wallace" | "scope" | "purgecss" => {}
        _ if target.is_file() || name == "lizard" => {
            command[last] = target.to_string_lossy().into_owned()
        }
        "clippy" => {}
        "stylelint" if target != root => {
            command[last] = target.join("**/*.css").to_string_lossy().into_owned()
        }
        _ if target != root => command[last] = target.to_string_lossy().into_owned(),
        _ => {}
    }
    command
}

fn build_command(root: &Path, target: &Path, t: &Tool) -> Result<Vec<String>, String> {
    if target.is_file() && t.name == "clippy" {
        return Err("Clippy checks crates, not individual files; use a crate directory or --tool lizard for file complexity".into());
    }
    if !t.configs.is_empty() && !has_config(root, t) {
        return Err(format!(
            "missing repository configuration: {}",
            t.configs.join(", ")
        ));
    }
    let mut command = with_target(
        t.command.iter().map(|s| s.to_string()).collect(),
        root,
        target,
        t.name,
    );
    if t.name == "lizard" {
        let modules = target.with_extension("");
        if target.is_file() && target.extension().is_some_and(|e| e == "rs") && modules.is_dir() {
            command.push(modules.to_string_lossy().into_owned());
        }
        command.extend(lizard_options(root)?);
        command.extend(generated_excludes(root));
    }
    let found = which(&command[0])
        .or_else(|| {
            which(
                &root
                    .join("node_modules/.bin")
                    .join(&command[0])
                    .to_string_lossy(),
            )
        })
        .ok_or_else(|| format!("{} not on PATH or in root node_modules/.bin", command[0]))?;
    command[0] = found.to_string_lossy().into_owned();
    Ok(command)
}

fn shown(command: &[String]) -> String {
    command
        .iter()
        .map(|a| {
            if !a.is_empty()
                && a.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./=:@,+%".contains(c))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run the tool and let it write to this terminal, unchanged.
fn run_native(
    label: &str,
    command: &[String],
    root: &Path,
    dry_run: bool,
    stdin: Option<&[PathBuf]>,
) -> Result<i32, String> {
    let mut line = shown(command);
    if let Some(files) = stdin {
        let names: Vec<String> = files
            .iter()
            .map(|f| f.to_string_lossy().into_owned())
            .collect();
        line = format!("cat {} | {line}", shown(&names));
    }
    eprintln!("[{label}] {line}");
    if dry_run {
        eprintln!("[{label}] not run (--dry-run)");
        return Ok(0);
    }
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(root)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .spawn()
        .map_err(|e| e.to_string())?;
    if let (Some(files), Some(mut pipe)) = (stdin, child.stdin.take()) {
        for file in files {
            pipe.write_all(&std::fs::read(file).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            pipe.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    #[cfg(unix)]
    let code = {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
    };
    #[cfg(not(unix))]
    let code = status.code().unwrap_or(1);
    eprintln!("[{label}] exit {code}");
    Ok(code)
}

fn run_tool(
    t: &Tool,
    command: &[String],
    root: &Path,
    target: &Path,
    dry_run: bool,
    base: &str,
) -> Result<i32, String> {
    if t.name == "scope" {
        let mut diff = command.to_vec();
        diff.extend([base.to_owned(), "--".into()]);
        let code = run_native(t.label, &diff, root, dry_run, None)?;
        let untracked: Vec<String> = [
            command[0].as_str(),
            "ls-files",
            "--others",
            "--exclude-standard",
        ]
        .map(String::from)
        .to_vec();
        let other = run_native("git untracked", &untracked, root, dry_run, None)?;
        return Ok(if other != 0 { other } else { code });
    }
    if t.name == "wallace" && target.is_dir() {
        let found = css_sources(target);
        if found.is_empty() {
            return Err(format!("no stylesheets under {}", target.display()));
        }
        return run_native(t.label, command, root, dry_run, Some(&found));
    }
    run_native(t.label, command, root, dry_run, None)
}

/// Run the selected tools with inherited output; the first nonzero status, else 0. Setup
/// errors report 2 and the remaining tools still run.
pub fn run(target: &Path, selected: &[String], dry_run: bool, base: &str) -> i32 {
    let single_file = target.is_file();
    let mut names: Vec<&str> = if selected.is_empty() {
        if single_file {
            vec!["lizard"]
        } else {
            TOOLS.iter().map(|t| t.name).collect()
        }
    } else {
        selected.iter().map(String::as_str).collect()
    };
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(*n));
    let start = if single_file {
        target.parent().unwrap_or(target)
    } else {
        target
    };
    let mut status = 0;
    for name in names {
        let t = tool(name);
        let root = config_root(start, t);
        let code = build_command(&root, target, t)
            .and_then(|command| run_tool(t, &command, &root, target, dry_run, base))
            .unwrap_or_else(|error| {
                eprintln!("[{}] not run: {error}", t.label);
                2
            });
        if status == 0 {
            status = code;
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_patterns_match_exact_names_and_prefixes() {
        assert!(matches("stylelint.config.mjs", "stylelint.config.*"));
        assert!(matches(".stylelintrc.json", ".stylelintrc*"));
        assert!(matches("Cargo.toml", "Cargo.toml"));
        assert!(!matches("Cargo.toml.bak", "Cargo.toml"));
    }

    #[test]
    fn a_file_is_measured_where_the_command_named_the_root() {
        let root = Path::new("/repo");
        let got = with_target(
            vec!["lizard".into(), ".".into()],
            root,
            Path::new("/repo"),
            "lizard",
        );
        assert_eq!(got, ["lizard", "/repo"]);
        let got = with_target(
            vec!["stylelint".into(), "**/*.css".into()],
            root,
            Path::new("/repo/ui"),
            "stylelint",
        );
        assert_eq!(got, ["stylelint", "/repo/ui/**/*.css"]);
        let got = with_target(
            vec!["git".into(), "diff".into()],
            root,
            Path::new("/repo/ui"),
            "scope",
        );
        assert_eq!(got, ["git", "diff"]);
    }
}
