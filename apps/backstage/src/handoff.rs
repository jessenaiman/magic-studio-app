//! PR-owned handoff metadata. No session discovery, agent connection or command execution.
use crate::provider;
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_JSON: u64 = 4 * 1024 * 1024;
mod imports;
mod input;
mod pr;
pub use input::Inputs;

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
        let (mut tasks, mut errors) = collect_tasks(input, roles, control);
        let retrieved = imports::attach(&mut tasks, input, &mut errors);
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
fn collect_tasks(
    input: &Inputs,
    roles: &[PathBuf],
    control: &provider::Control,
) -> (Vec<Task>, Vec<String>) {
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
                        pr::parse(&value, &input.repo, None, roles),
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
                pr::parse(&value, &input.repo, Some(*number), roles),
            ),
            Err(e) => errors.push(format!("PR#{number}: {e}")),
        }
    }
    if input.prs.is_empty() && input.files.is_empty() {
        errors.push("No PRs supplied; configure --mops-pr NUMBER or --mops-pr-file PATH".into());
    }
    (tasks, errors)
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
            "# MAYHEM — PR-scoped audit evidence\n\nPR #{}: {}\nURL: {}\nNative base SHA: {}\nNative head SHA: {}\nSource: {}\n\nNative code findings: Unavailable. No baseline/candidate audit artifacts matching these exact PR revisions are loaded and verified. Unrelated whole-project reports are not substituted.\n\nCI metadata (separate from code findings / approval): {}\n\nNo audit checker, agent or model was started. Use MOPS for ownership, review, blockers and recorded handoff metadata; missing session/resume fields remain Unknown.\n",
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
