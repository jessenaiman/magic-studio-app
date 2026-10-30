//! Native PR identity and passive canonical handoff validation. No command execution.
use super::Task;
use crate::data::clean;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn native<'a>(value: &'a Value, key: &str, fallback: &[&str]) -> Option<&'a str> {
    value[key].as_str().or_else(|| {
        let mut v = value;
        for key in fallback {
            v = &v[*key];
        }
        v.as_str()
    })
}
fn text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|s| !s.is_empty()).map(clean)
}
struct NativePr {
    number: u64,
    url: String,
    body: String,
    title: Option<String>,
    review: String,
    state: Option<String>,
    merged: Option<bool>,
    branch: Option<String>,
    head_sha: Option<String>,
    base_sha: Option<String>,
    author: Option<String>,
    feedback: Vec<String>,
    ci: String,
}
impl NativePr {
    fn read(value: &Value, repo: &str, requested: Option<u64>) -> Result<Self, String> {
        let (number, url) = identity(value, repo, requested)?;
        Ok(Self {
            number,
            url: url.into(),
            body: value["body"].as_str().unwrap_or("").into(),
            title: text(value["title"].as_str()),
            review: review(value),
            state: text(value["state"].as_str()),
            merged: value["merged"].as_bool(),
            branch: text(native(value, "headRefName", &["head", "ref"])),
            head_sha: text(native(value, "headRefOid", &["head", "sha"])),
            base_sha: text(native(value, "baseRefOid", &["base", "sha"])),
            author: text(native(value, "authorLogin", &["author", "login"]))
                .or_else(|| text(value["user"]["login"].as_str())),
            feedback: feedback(value),
            ci: ci(value),
        })
    }
}
fn identity<'a>(
    value: &'a Value,
    repo: &str,
    requested: Option<u64>,
) -> Result<(u64, &'a str), String> {
    let number = value["number"]
        .as_u64()
        .filter(|number| *number > 0)
        .ok_or("PR JSON has no valid number")?;
    if requested.is_some_and(|requested| requested != number) {
        return Err("PR response identity differs from requested number".into());
    }
    let url = value["html_url"]
        .as_str()
        .or_else(|| value["url"].as_str())
        .filter(|url| url.starts_with("https://github.com/"))
        .ok_or("PR JSON lacks canonical GitHub URL")?;
    let expected = format!("https://github.com/{repo}/pull/{number}");
    if !url.eq_ignore_ascii_case(&expected) {
        return Err(format!(
            "PR#{number} URL does not match configured repository"
        ));
    }
    Ok((number, url))
}
fn review(value: &Value) -> String {
    if value["isDraft"]
        .as_bool()
        .or_else(|| value["draft"].as_bool())
        == Some(true)
    {
        return "Draft".into();
    }
    match value["reviewDecision"].as_str() {
        Some("APPROVED") => "Approved",
        Some("CHANGES_REQUESTED") => "Changes requested",
        Some("REVIEW_REQUIRED") => "Awaiting review",
        _ => "Unknown",
    }
    .into()
}
fn supplied_field(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields.get(key).and_then(|value| supplied(value))
}
// Session handoff values are literal PR text, not normalized human labels.
fn recorded_field(fields: &BTreeMap<String, String>, key: &str) -> Option<String> {
    fields
        .get(key)
        .filter(|value| {
            let label = value.trim();
            !label.is_empty()
                && !["unknown", "none", "unavailable", "n/a"]
                    .iter()
                    .any(|missing| label.eq_ignore_ascii_case(missing))
        })
        .cloned()
}
fn worktree_target(worktree: Option<&str>) -> Option<PathBuf> {
    worktree
        .filter(|path| !path.chars().any(char::is_control) && Path::new(path).is_absolute())
        .and_then(|path| Path::new(path).canonicalize().ok())
        .filter(|path| path.is_dir())
}
fn resolve_role(
    fields: &BTreeMap<String, String>,
    worktree: Option<&str>,
    roles: &[PathBuf],
    warnings: &mut Vec<String>,
) -> (Option<String>, bool) {
    let supplied = supplied_field(fields, "role source");
    let target = supplied
        .as_ref()
        .and_then(|raw| role_target(raw, worktree, roles));
    let available = target.is_some();
    let path = target.map(|path| path.display().to_string()).or(supplied);
    if path.is_some() && !available {
        warnings.push("Role source is not a discovered existing crew.list role path".into());
    }
    (path, available)
}
fn action(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        match value.to_ascii_lowercase().as_str() {
            "needs decision" => Some("Needs decision"),
            "on hold" => Some("On hold"),
            "pending" => Some("Pending"),
            "in production" => Some("In production"),
            "blocked" => Some("Blocked"),
            _ => None,
        }
        .map(str::to_string)
    })
}
fn blocker_known(fields: &BTreeMap<String, String>) -> bool {
    fields.get("blocker").is_some_and(|value| {
        !value.trim().eq_ignore_ascii_case("unknown") && !value.trim().is_empty()
    })
}

pub(super) fn parse(
    value: &Value,
    repo: &str,
    requested: Option<u64>,
    roles: &[PathBuf],
) -> Result<Task, String> {
    let native = NativePr::read(value, repo, requested)?;
    let (fields, warnings) = fields(&native.body);
    Ok(assemble(native, &fields, warnings, roles))
}
fn assemble(
    native: NativePr,
    fields: &BTreeMap<String, String>,
    mut warnings: Vec<String>,
    roles: &[PathBuf],
) -> Task {
    let get = |key: &str| supplied_field(fields, key);
    let worktree = get("worktree");
    let worktree_available = worktree_target(worktree.as_deref()).is_some();
    let (role_path, role_available) =
        resolve_role(fields, worktree.as_deref(), roles, &mut warnings);
    Task {
        pr_number: native.number,
        pr_url: native.url,
        name: get("task").or(native.title),
        issue: get("issue"),
        review: native.review,
        state: native.state,
        merged: native.merged,
        feedback: native.feedback,
        department: get("department"),
        implementation_role: get("implementation role"),
        agent: get("agent"),
        harness: get("harness"),
        session_id: recorded_field(fields, "session id"),
        branch: native.branch,
        worktree,
        role_path,
        resume_command: recorded_field(fields, "resume command"),
        model_provider: get("model / provider"),
        next_action: get("next action"),
        action: action(get("your action")),
        blocker: get("blocker"),
        role_available,
        worktree_available,
        blocker_known: blocker_known(fields),
        warnings,
        body: native.body,
        head_sha: native.head_sha,
        base_sha: native.base_sha,
        author: native.author,
        ci: native.ci,
        review_note: "Review records not supplied".into(),
        source: "GitHub metadata; retrieval provenance unknown".into(),
    }
}

fn supplied(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('`').trim();
    if value.is_empty()
        || ["unknown", "none", "unavailable", "n/a"]
            .iter()
            .any(|s| value.eq_ignore_ascii_case(s))
        || value.starts_with('<')
    {
        None
    } else {
        Some(value.into())
    }
}
fn fields(body: &str) -> (BTreeMap<String, String>, Vec<String>) {
    let mut active = false;
    let mut sections = 0;
    let mut fields = BTreeMap::new();
    let mut warnings = Vec::new();
    for raw in body.lines() {
        let line = raw.trim();
        if line.eq_ignore_ascii_case("## MOPS handoff") {
            active = true;
            sections += 1;
            continue;
        }
        if line.starts_with("## ") {
            active = false;
        }
        if !active {
            continue;
        }
        if let Some((key, value)) = raw
            .trim_start()
            .strip_prefix("- ")
            .and_then(|s| s.split_once(':'))
        {
            let key = key.trim().to_ascii_lowercase();
            if fields
                .insert(
                    key.clone(),
                    value.strip_prefix(' ').unwrap_or(value).to_string(),
                )
                .is_some()
            {
                warnings.push(format!("Duplicate handoff field: {key}"));
            }
        }
    }
    if sections == 0 {
        warnings.push(
            "No canonical MOPS handoff section; session fields are not inferred from prose".into(),
        );
    }
    if sections > 1 {
        warnings.push("Multiple MOPS handoff sections are ambiguous".into());
    }
    (fields, warnings)
}
fn role_target(raw: &str, worktree: Option<&str>, roles: &[PathBuf]) -> Option<PathBuf> {
    if raw.chars().any(char::is_control) {
        return None;
    }
    let raw = Path::new(raw);
    let target = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        let root = worktree
            .map(Path::new)
            .and_then(|p| p.canonicalize().ok())?;
        root.join(raw)
    };
    let target = target.canonicalize().ok().filter(|p| p.is_file())?;
    roles
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .any(|p| p == target)
        .then_some(target)
}
fn feedback(value: &Value) -> Vec<String> {
    let mut messages = Vec::new();
    for key in ["reviews", "comments"] {
        if let Some(records) = value[key].as_array() {
            for record in records.iter().rev().take(32) {
                if let Some(body) = record["body"].as_str().filter(|s| !s.trim().is_empty()) {
                    messages.push(clean(body));
                }
            }
        }
    }
    messages
}
fn ci(value: &Value) -> String {
    let Some(checks) = value["statusCheckRollup"].as_array() else {
        return "CI not supplied; independent of review approval".into();
    };
    let text = checks
        .iter()
        .take(8)
        .map(|c| {
            format!(
                "{}: {} {}",
                c["name"].as_str().unwrap_or("Check"),
                c["status"].as_str().unwrap_or("Unknown"),
                c["conclusion"].as_str().unwrap_or("Unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    if text.is_empty() {
        "No check runs supplied (not a review approval)".into()
    } else {
        text
    }
}
