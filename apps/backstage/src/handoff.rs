//! PR-owned handoff metadata. No session discovery, agent connection or command execution.
use crate::{data::clean, provider};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_JSON: u64 = 4 * 1024 * 1024;
#[derive(Clone, Default)]
pub struct Inputs {
    pub repo: String,
    pub prs: Vec<u64>,
    pub files: Vec<PathBuf>,
    pub reviews: Vec<(u64, PathBuf)>,
    pub checks: Vec<(u64, PathBuf)>,
    pub provenance: Vec<PathBuf>,
}
impl Inputs {
    pub fn parse(args: &[String]) -> Result<Option<Self>, String> {
        let mut input = Self::default();
        let mut present = false;
        let mut i = 0;
        while i < args.len() {
            let key = args[i].as_str();
            if matches!(
                key,
                "--mops-repo"
                    | "--mops-pr"
                    | "--mops-pr-file"
                    | "--mops-reviews-file"
                    | "--mops-checks-file"
                    | "--mops-provenance"
            ) {
                present = true;
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| format!("{key} requires a value"))?;
                match key {
                    "--mops-repo" => input.repo = value.clone(),
                    "--mops-pr" => {
                        let number = value
                            .parse::<u64>()
                            .ok()
                            .filter(|n| *n > 0)
                            .ok_or("--mops-pr requires a positive PR number")?;
                        if !input.prs.contains(&number) {
                            input.prs.push(number);
                        }
                    }
                    "--mops-pr-file" => input.files.push(value.into()),
                    "--mops-provenance" => input.provenance.push(value.into()),
                    _ => {
                        let (number, path) = value
                            .split_once('=')
                            .ok_or("Auxiliary PR file requires NUMBER=PATH")?;
                        let number = number
                            .parse::<u64>()
                            .ok()
                            .filter(|n| *n > 0)
                            .ok_or("Invalid auxiliary PR number")?;
                        if key == "--mops-reviews-file" {
                            input.reviews.push((number, path.into()))
                        } else {
                            input.checks.push((number, path.into()))
                        }
                    }
                }
            }
            i += 1;
        }
        if !present {
            return Ok(None);
        }
        let parts: Vec<_> = input.repo.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|p| {
                p.is_empty()
                    || p.starts_with('-')
                    || !p
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
            })
        {
            return Err("--mops-repo OWNER/REPO is required (github.com only)".into());
        }
        if input.prs.len() + input.files.len() > 20
            || input.reviews.len() + input.checks.len() > 40
            || input.provenance.len() > 4
        {
            return Err("MOPS is bounded to20 explicitly supplied PRs/files".into());
        }
        Ok(Some(input))
    }
}
pub struct Task {
    pub pr_number: u64,
    pub pr_url: String,
    pub name: Option<String>,
    pub issue: Option<String>,
    pub review: String,
    pub state: Option<String>,
    pub merged: Option<bool>,
    pub feedback: Vec<String>,
    pub department: Option<String>,
    pub implementation_role: Option<String>,
    pub agent: Option<String>,
    pub harness: Option<String>,
    pub session_id: Option<String>,
    pub branch: Option<String>,
    pub worktree: Option<String>,
    pub role_path: Option<String>,
    pub resume_command: Option<String>,
    pub model_provider: Option<String>,
    pub next_action: Option<String>,
    pub action: Option<String>,
    pub blocker: Option<String>,
    pub verified_resume: bool,
    pub role_available: bool,
    pub worktree_available: bool,
    pub blocker_known: bool,
    pub warnings: Vec<String>,
    pub body: String,
    pub head_sha: Option<String>,
    pub base_sha: Option<String>,
    pub author: Option<String>,
    pub ci: String,
    pub review_note: String,
    pub source: String,
}
pub struct Snapshot {
    pub tasks: Vec<Task>,
    pub source: String,
    pub error: Option<String>,
    pub complete: bool,
    pub retrieved: String,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self{tasks:vec![],source:"PR metadata not read; press R with configured --mops-repo and --mops-pr / --mops-pr-file".into(),error:None,complete:false,retrieved:"Not read".into()}
    }
}
impl Snapshot {
    pub fn read(input: &Inputs, roles: &[PathBuf], control: &provider::Control) -> Self {
        let mut tasks = Vec::new();
        let mut errors = Vec::new();
        let mut seen = HashSet::new();
        for path in &input.files {
            match read_file(path) {
                Ok(values) => {
                    for value in values {
                        push_task(
                            &mut tasks,
                            &mut errors,
                            &mut seen,
                            parse(&value, &input.repo, None, roles),
                        );
                    }
                }
                Err(e) => errors.push(e),
            }
        }
        for number in &input.prs {
            match provider::pull_request(control, &input.repo, *number) {
                Ok(value) => push_task(
                    &mut tasks,
                    &mut errors,
                    &mut seen,
                    parse(&value, &input.repo, Some(*number), roles),
                ),
                Err(e) => errors.push(format!("PR#{number}: {e}")),
            }
        }
        if input.prs.is_empty() && input.files.is_empty() {
            errors
                .push("No PRs supplied; configure --mops-pr NUMBER or --mops-pr-file PATH".into());
        }
        let mut provenance_records = Vec::new();
        for path in &input.provenance {
            match json(path) {
                Ok(value) => {
                    if let Some(records) = value["records"].as_array().filter(|a| a.len() <= 100) {
                        provenance_records.extend(records.iter().cloned());
                    } else {
                        errors.push("Invalid retrieval provenance records".into());
                    }
                }
                Err(error) => errors.push(error),
            }
        }
        let provenance = Some(serde_json::json!({"records":provenance_records}));
        let mut retrieved = Vec::new();
        for task in &mut tasks {
            for (path, kind) in input
                .reviews
                .iter()
                .filter(|(n, _)| *n == task.pr_number)
                .map(|(_, p)| (p, "reviews"))
                .chain(
                    input
                        .checks
                        .iter()
                        .filter(|(n, _)| *n == task.pr_number)
                        .map(|(_, p)| (p, "checks")),
                )
            {
                let evidence = match json(path) {
                    Ok(value) => value,
                    Err(error) => {
                        task.warnings.push(error);
                        continue;
                    }
                };
                if kind == "reviews" {
                    let attested = provenance
                        .as_ref()
                        .and_then(|p| provenance_record(p, path, task.head_sha.as_deref()));
                    if attested.is_none() {
                        task.warnings.push(
                            "Review snapshot has no matching PR-head provenance; not attached"
                                .into(),
                        );
                        continue;
                    }
                    if let Some(records) = evidence.as_array() {
                        task.review_note = format!(
                            "{} submitted GitHub review records (snapshot)",
                            records.len()
                        );
                        for review in records.iter().rev().take(32) {
                            if let Some(body) =
                                review["body"].as_str().filter(|s| !s.trim().is_empty())
                            {
                                task.feedback.push(clean(body));
                            }
                        }
                    } else {
                        task.warnings
                            .push("Reviews input is not a native JSON array".into());
                    }
                } else if let Some(records) = evidence["check_runs"].as_array() {
                    let records: Vec<_> = records
                        .iter()
                        .filter(|c| c["head_sha"].as_str() == task.head_sha.as_deref())
                        .collect();
                    if records.is_empty() {
                        task.warnings
                            .push("No supplied check run matches the PR head".into());
                    }
                    task.ci = records
                        .iter()
                        .map(|c| {
                            format!(
                                "{}: {} / {}",
                                c["name"].as_str().unwrap_or("Check"),
                                c["status"].as_str().unwrap_or("Unknown"),
                                c["conclusion"].as_str().unwrap_or("Unknown")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ");
                } else {
                    task.warnings
                        .push("Checks input lacks native check_runs array".into());
                }
            }
            if let Some(provenance) = &provenance {
                for path in &input.files {
                    if let Some(record) =
                        provenance_record(provenance, path, task.head_sha.as_deref())
                        && let Some(time) = record["retrieved_at_utc"].as_str()
                    {
                        retrieved.push(clean(time));
                        task.source = format!(
                            "imported GitHub snapshot • retrieved {} • {} • R rereads file only",
                            clean(time),
                            clean(&path.display().to_string())
                        );
                    }
                }
            }
        }
        retrieved.sort();
        retrieved.dedup();
        let retrieved = if retrieved.is_empty() {
            "Unknown (no matching retrieval provenance supplied)".into()
        } else {
            retrieved.join(" .. ")
        };
        Self {
            tasks,
            retrieved,
            source: format!(
                "{} • {} • explicit read only • no live session observation",
                if input.files.is_empty() {
                    "GitHub CLI metadata"
                } else {
                    "imported GitHub snapshot"
                },
                input.repo
            ),
            complete: errors.is_empty(),
            error: (!errors.is_empty()).then(|| errors.join("\n")),
        }
    }
    pub fn decisions(&self) -> Option<usize> {
        (self.complete && !self.tasks.is_empty() && self.tasks.iter().all(|t| t.action.is_some()))
            .then(|| {
                self.tasks
                    .iter()
                    .filter(|t| t.action.as_deref() == Some("Needs decision"))
                    .count()
            })
    }
    pub fn blocked(&self) -> Option<usize> {
        (self.complete && !self.tasks.is_empty() && self.tasks.iter().all(|t| t.blocker_known))
            .then(|| self.tasks.iter().filter(|t| t.blocker.is_some()).count())
    }
}
fn push_task(
    tasks: &mut Vec<Task>,
    errors: &mut Vec<String>,
    seen: &mut HashSet<u64>,
    result: Result<Task, String>,
) {
    if tasks.len() >= 20 {
        errors.push("MOPS PR record bound reached (20)".into());
        return;
    }
    match result {
        Ok(task) => {
            if seen.insert(task.pr_number) {
                tasks.push(task)
            } else {
                errors.push(format!("Duplicate PR#{} input", task.pr_number));
            }
        }
        Err(e) => errors.push(e),
    }
}
fn json(path: &Path) -> Result<Value, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.file_type().is_file() || meta.len() > MAX_JSON {
        return Err("PR input must be a regular JSON file <=4MiB".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_JSON + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON as usize {
        return Err("PR JSON exceeded4MiB".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid PR JSON: {e}"))?;
    Ok(value)
}
fn read_file(path: &Path) -> Result<Vec<Value>, String> {
    let value = json(path)?;
    match value {
        Value::Array(values) if values.len() <= 20 => Ok(values),
        Value::Object(_) => Ok(vec![value]),
        _ => Err("Expected native GitHub PR object or <=20 object array".into()),
    }
}

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
fn parse(
    value: &Value,
    repo: &str,
    requested: Option<u64>,
    roles: &[PathBuf],
) -> Result<Task, String> {
    let number = value["number"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("PR JSON has no valid number")?;
    if requested.is_some_and(|n| n != number) {
        return Err("PR response identity differs from requested number".into());
    }
    let url = value["html_url"]
        .as_str()
        .or_else(|| value["url"].as_str())
        .filter(|u| u.starts_with("https://github.com/"))
        .ok_or("PR JSON lacks canonical GitHub URL")?;
    let expected = format!("https://github.com/{repo}/pull/{number}");
    if !url.eq_ignore_ascii_case(&expected) {
        return Err(format!(
            "PR#{number} URL does not match configured repository"
        ));
    }
    let body = value["body"].as_str().unwrap_or("").to_string();
    let (fields, mut warnings) = fields(&body);
    let get = |key: &str| fields.get(key).and_then(|v| supplied(v));
    let harness = get("harness");
    let session_id = get("session id");
    let worktree = get("worktree");
    let resume_command = get("resume command");
    let resume_clean = resume_command
        .as_ref()
        .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control));
    let verified_resume = fields
        .get("resume verified")
        .is_some_and(|s| matches!(s.trim().to_ascii_lowercase().as_str(), "yes" | "true"))
        && resume_clean
        && harness.is_some()
        && session_id.is_some()
        && worktree.is_some()
        && warnings.is_empty();
    let worktree_target = worktree
        .as_ref()
        .filter(|p| !p.chars().any(char::is_control) && Path::new(p).is_absolute())
        .and_then(|p| Path::new(p).canonicalize().ok())
        .filter(|p| p.is_dir());
    let worktree_available = worktree_target.is_some();
    let worktree = worktree_target
        .map(|p| p.display().to_string())
        .or(worktree);
    let role_path = get("role source");
    let target = role_path
        .as_ref()
        .and_then(|raw| role_target(raw, worktree.as_deref(), roles));
    let role_available = target.is_some();
    let role_path = target.map(|p| p.display().to_string()).or(role_path);
    if role_path.is_some() && !role_available {
        warnings.push("Role source is not a discovered existing crew.list role path".into());
    }
    let action = get("your action").and_then(|a| {
        match a.to_ascii_lowercase().as_str() {
            "needs decision" => Some("Needs decision"),
            "on hold" => Some("On hold"),
            "pending" => Some("Pending"),
            "in production" => Some("In production"),
            "blocked" => Some("Blocked"),
            _ => None,
        }
        .map(str::to_string)
    });
    let blocker_known = fields
        .get("blocker")
        .is_some_and(|s| !s.trim().eq_ignore_ascii_case("unknown") && !s.trim().is_empty());
    let blocker = get("blocker");
    let feedback = feedback(value);
    let ci = ci(value);
    let review = if value["isDraft"]
        .as_bool()
        .or_else(|| value["draft"].as_bool())
        == Some(true)
    {
        "Draft"
    } else {
        match value["reviewDecision"].as_str() {
            Some("APPROVED") => "Approved",
            Some("CHANGES_REQUESTED") => "Changes requested",
            Some("REVIEW_REQUIRED") => "Awaiting review",
            _ => "Unknown",
        }
    }
    .into();
    Ok(Task {
        pr_number: number,
        pr_url: url.into(),
        name: get("task").or_else(|| text(value["title"].as_str())),
        issue: get("issue"),
        review,
        state: text(value["state"].as_str()),
        merged: value["merged"].as_bool(),
        feedback,
        department: get("department"),
        implementation_role: get("implementation role"),
        agent: get("agent"),
        harness,
        session_id,
        branch: text(native(value, "headRefName", &["head", "ref"])),
        worktree,
        role_path,
        resume_command,
        model_provider: get("model / provider"),
        next_action: get("next action"),
        action,
        blocker,
        verified_resume,
        role_available,
        worktree_available,
        blocker_known,
        warnings,
        body,
        head_sha: text(native(value, "headRefOid", &["head", "sha"])),
        base_sha: text(native(value, "baseRefOid", &["base", "sha"])),
        author: text(native(value, "authorLogin", &["author", "login"]))
            .or_else(|| text(value["user"]["login"].as_str())),
        ci,
        review_note: "Review records not supplied".into(),
        source: "GitHub metadata; retrieval provenance unknown".into(),
    })
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
    for line in body.lines() {
        let line = line.trim();
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
        if let Some((key, value)) = line.strip_prefix("- ").and_then(|s| s.split_once(':')) {
            let key = key.trim().to_ascii_lowercase();
            if fields
                .insert(key.clone(), value.trim().to_string())
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

fn provenance_record<'a>(value: &'a Value, path: &Path, sha: Option<&str>) -> Option<&'a Value> {
    let sha = sha?;
    let path = path.canonicalize().ok()?;
    value["records"]
        .as_array()?
        .iter()
        .filter(|record| {
            record["head_sha"]
                .as_str()
                .or_else(|| record["head"].as_str())
                == Some(sha)
                && record["path"]
                    .as_str()
                    .and_then(|p| Path::new(p).canonicalize().ok())
                    .as_ref()
                    == Some(&path)
        })
        .max_by_key(|record| record["retrieved_at_utc"].as_str().unwrap_or(""))
}

#[derive(Clone)]
pub struct PrScope {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub base: Option<String>,
    pub head: Option<String>,
    pub source: String,
    pub ci: String,
}
impl PrScope {
    pub fn matches(&self, base: Option<&str>, head: Option<&str>) -> bool {
        self.base.is_some()
            && self.head.is_some()
            && self.base.as_deref() == base
            && self.head.as_deref() == head
    }
    pub fn markdown(&self) -> String {
        format!(
            "# MAYHEM — PR-scoped audit evidence\n\nPR #{}: {}\nURL: {}\nNative base SHA: {}\nNative head SHA: {}\nSource: {}\n\nNative code findings: Unavailable. No baseline/candidate audit artifacts matching these exact PR revisions were supplied. Unrelated whole-project reports are not substituted.\n\nCI metadata (separate from code findings / approval): {}\n\nNo audit checker, agent or model was started. Use MOPS for ownership, review, blockers and recorded handoff metadata; missing session/resume fields remain Unknown.\n",
            self.number,
            self.title,
            self.url,
            self.base.as_deref().unwrap_or("Unknown"),
            self.head.as_deref().unwrap_or("Unknown"),
            self.source,
            self.ci
        )
    }
}
impl Task {
    pub fn audit_scope(&self) -> PrScope {
        PrScope {
            number: self.pr_number,
            title: self.name.clone().unwrap_or_else(|| "Unknown".into()),
            url: self.pr_url.clone(),
            base: self.base_sha.clone(),
            head: self.head_sha.clone(),
            source: self.source.clone(),
            ci: self.ci.clone(),
        }
    }
    pub fn audit_unavailable(&self) -> String {
        self.audit_scope().markdown()
    }
}
