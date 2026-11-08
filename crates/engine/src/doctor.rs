//! `app.doctor` (`magic-cli app doctor`): the install and environment checks, in the Craft suite's
//! doctor shape (`craft-anything-cli doctor`): a flat `checks` list, each with a `name`, a `status`
//! of `ok`, `warn` or `fail`, a `message`, and a `remediation` when something is wrong.
//!
//! Doctor reports whether Magic can run its tools here. Project progress is not a doctor concern:
//! that is `workshop.status`. Doctor never scans `/proc` or stops processes.

use std::path::Path;

use serde_json::{Value, json};

use crate::craft_hub::CraftTarget;
use crate::workspace::resolve_tool;
use crate::{Result, Session};

/// One doctor finding.
struct Check {
    name: String,
    status: &'static str,
    message: String,
    remediation: Option<String>,
}

impl Check {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), status: "ok", message: message.into(), remediation: None }
    }

    fn problem(status: &'static str, name: impl Into<String>, message: impl Into<String>, remediation: impl Into<String>) -> Self {
        Self { name: name.into(), status, message: message.into(), remediation: Some(remediation.into()) }
    }

    fn to_json(&self) -> Value {
        let mut out = json!({"name": self.name, "status": self.status, "message": self.message});
        if let Some(fix) = &self.remediation {
            out["remediation"] = json!(fix);
        }
        out
    }
}

/// An executable Magic resolves (environment override, `config.toml`, launcher install or `PATH`).
fn tool_check(name: &str, missing: &'static str, remediation: &str) -> Check {
    let resolved = resolve_tool(name);
    if Path::new(&resolved).is_file() || which_in_path(&resolved) {
        Check::ok(format!("tool:{name}"), format!("{name} is at {resolved}"))
    } else {
        Check::problem(missing, format!("tool:{name}"), format!("{name} was not found (looked for {resolved})"), remediation)
    }
}

fn checks(s: &Session) -> Vec<Check> {
    let mut out: Vec<Check> = CraftTarget::ALL
        .iter()
        .map(|target| {
            // TextureCraft has no Magic workflow yet, so its absence is a warning.
            let missing = if *target == CraftTarget::Texture { "warn" } else { "fail" };
            tool_check(target.cli_name(), missing, &format!("Install {} from https://getartcraft.com/apps/{}", target.name(), target.name()))
        })
        .collect();
    out.push(tool_check("magic-cli", "fail", "Build it with `cargo build -p magic-cli` and put target/debug on PATH"));
    out.push(tool_check("ffmpeg", "fail", "Install ffmpeg with the system package manager"));
    out.push(tool_check("codex", "fail", "Install the Codex CLI: Scribbles image generation runs through it"));

    out.push(match s.control_address {
        Some(addr) => Check::ok("control", format!("live control listening on {addr}")),
        None => Check::ok("control", "no live control listener in this session (headless)"),
    });

    let project_dir = s.project_path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf).unwrap_or_else(|| s.cwd.clone());
    let server_file = project_dir.join(".video-server.json");
    if server_file.is_file() {
        out.push(Check::problem(
            "warn",
            "video-server",
            format!("{} is present; its server is not verified", server_file.display()),
            "Delete the file if no video preview server is running for this project",
        ));
    }
    out
}

/// Whether each installed Craft app Magic drives has a newer release, from the Craft installer's
/// own detection and GitHub release check (`craft-update`). Network trouble is a warning: the
/// tools still run, the check just could not say whether they are current.
fn update_checks() -> Vec<Check> {
    // GitHub allows 60 anonymous API requests an hour per address; the token `gh` and CI use
    // (GH_TOKEN, GITHUB_TOKEN), when set, raises that to the account's limit.
    let token = ["GH_TOKEN", "GITHUB_TOKEN"].iter().find_map(|var| std::env::var(var).ok().filter(|t| !t.trim().is_empty()));
    let http = match craft_update::Http::new(concat!("MagicStudio/", env!("CARGO_PKG_VERSION"))) {
        Ok(http) => http.with_github_token(token),
        Err(error) => return vec![Check::problem("warn", "updates", format!("cannot check for updates: {error}"), "Check the network connection")],
    };
    let manifest = craft_update::manifest::Manifest::default();
    let snapshot = craft_update::detect::Snapshot::take();
    CraftTarget::ALL
        .iter()
        .filter_map(|target| craft_update::suite::by_id(target.name()))
        .map(|app| {
            let name = format!("update:{}", app.id);
            let Some(installed) = craft_update::detect::detect(app, &manifest, &snapshot).and_then(|i| i.version) else {
                return Check::problem(
                    "warn",
                    name,
                    format!("{}'s installed version could not be read", app.name),
                    format!("Install {} with the ArtCraft Launcher", app.name),
                );
            };
            match craft_update::check_for_update(app, &http, &installed.to_string()) {
                Ok(None) => Check::ok(name, format!("{} {installed} is the latest release", app.name)),
                Ok(Some(release)) => {
                    let latest = release.version.map_or_else(|| release.tag.clone(), |v| v.to_string());
                    Check::problem(
                        "warn",
                        name,
                        format!("{} {latest} is available (installed {installed})", app.name),
                        format!("Update with the ArtCraft Launcher, or from {}", release.html_url),
                    )
                }
                Err(error) => Check::problem(
                    "warn",
                    name,
                    format!("could not check {} for updates: {error}", app.name),
                    "Check the network connection, or set GH_TOKEN to lift GitHub's anonymous request limit",
                ),
            }
        })
        .collect()
}

/// `app.doctor`: the install and environment checks; `updates: false` skips the network check.
pub fn cmd_app_doctor(s: &mut Session, p: &Value) -> Result<Value> {
    let mut found = checks(s);
    if p.get("updates").and_then(Value::as_bool) != Some(false) {
        found.extend(update_checks());
    }
    let checks: Vec<Value> = found.iter().map(Check::to_json).collect();
    Ok(json!({"ok": true, "checks": checks}))
}

pub(crate) fn analytics_source_root(s: &crate::Session) -> std::path::PathBuf {
    let is_checkout = |r: &Path| r.join("Cargo.toml").is_file() && r.join("crates/engine/Cargo.toml").is_file();
    if is_checkout(&s.cwd) {
        return s.cwd.clone();
    }
    if let Ok(canon) = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()
        && is_checkout(&canon)
    {
        return canon;
    }
    crate::workspace::workspace_root(s)
}

fn which_in_path(bin: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(bin).is_file()))
}
