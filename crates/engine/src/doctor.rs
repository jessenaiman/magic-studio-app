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

/// `app.doctor`: the install and environment checks.
pub fn cmd_app_doctor(s: &mut Session, _: &Value) -> Result<Value> {
    let checks: Vec<Value> = checks(s).iter().map(Check::to_json).collect();
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
