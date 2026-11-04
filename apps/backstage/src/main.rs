//! Backstage: Magic Studio's separate Rust/Ratatui production and engineering observer.
//!
//! Workshop shows saved project references, MOPS shows PR-owned handoffs, Mayhem
//! shows worktrees and existing native audit evidence, and Settings shows bounded
//! observations of production paths and crew documents. Unavailable, stale, and
//! imported observations remain labeled; they are not inferred live session state.
//!
//! # Read the implementation
//!
//! - [`App`] coordinates view state and completion events through [`Wake`].
//! - [`provider`] bounds supported observation processes and their lifecycle.
//! - [`project_data`] reads saved references; [`data`] observes repository files.
//! - [`handoff`] interprets explicitly selected PR metadata without resuming agents.
//! - [`audit`] reads supplied native report artifacts without running a checker.
//! - [`workshop`], [`mops`], [`mayhem`], and [`ui`] render observed evidence.
//!
//! This is a binary implementation reference: Rustdoc includes its private items,
//! not a stable public library API. Backstage has no engine linkage or model client.
//! Selection/navigation does not contact agents; explicit refresh and Open actions
//! have the read/process boundaries described in the
//! [observer guide](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/apps/backstage/README.md).
//! See the repository's
//! [architecture guide](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/docs/architecture/README.md)
//! for its separation from the desktop shell and shared engine.
//! Repository links track the default branch; new guides appear after this
//! documentation change merges.
mod audit;
mod data;
mod handoff;
mod mayhem;
mod mops;
mod project_data;
mod provider;
mod ui;
mod widgets;
mod workshop;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
};
use ratatui::layout::Rect;
use std::{io, path::Path, process::Command};

enum Wake {
    Shutdown,
    MopsObserved(Box<handoff::Snapshot>),
    AuditObserved(Result<Box<audit::Reports>, String>),
    Terminal(io::Result<Event>),
    Observed(Box<data::Snapshot>, Box<project_data::ProjectSnapshot>),
}

#[cfg(unix)]
struct ShutdownSignals {
    handle: signal_hook::iterator::Handle,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[cfg(unix)]
impl ShutdownSignals {
    fn new(sender: std::sync::mpsc::Sender<Wake>) -> io::Result<Self> {
        let mut signals = signal_hook::iterator::Signals::new([
            signal_hook::consts::SIGHUP,
            signal_hook::consts::SIGTERM,
        ])?;
        let handle = signals.handle();
        let thread = std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                let _ = sender.send(Wake::Shutdown);
            }
        });
        Ok(Self {
            handle,
            thread: Some(thread),
        })
    }
}
#[cfg(unix)]
impl Drop for ShutdownSignals {
    fn drop(&mut self) {
        self.handle.close();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Workshop,
    Mayhem,
    Mops,
    Settings,
}
#[derive(Clone, Copy)]
enum MopsAction {
    OpenPr,
    OpenRole,
    OpenWorktree,
    CopyResume,
    CopyFeedback,
    ViewMayhem,
}
struct App {
    clipboard_supported: bool,
    mops: handoff::Snapshot,
    mops_inputs: Option<handoff::Inputs>,
    mops_selected: usize,
    mops_rows: Vec<(Rect, usize)>,
    mops_actions: Vec<(Rect, MopsAction)>,
    audit_pr_context: Option<handoff::PrScope>,
    audit: Option<audit::Reports>,
    audit_error: Option<String>,
    audit_inputs: Option<audit::Inputs>,
    audit_selected: usize,
    audit_scroll: usize,
    audit_rows: Vec<(Rect, usize)>,
    view: View,
    project_data: project_data::ProjectSnapshot,
    project_selected: usize,
    project_file_selected: usize,
    worktree_selected: usize,
    focus_projects: bool,
    collapsed: [bool; 4],
    project_rows: Vec<(Rect, usize)>,
    project_file_rows: Vec<(Rect, usize)>,
    worktree_rows: Vec<(Rect, usize)>,
    open_button: Option<Rect>,
    tabs: Vec<(Rect, View)>,
    data: data::Snapshot,
    refreshing: bool,
    selected: usize,
    message: String,
    details: bool,
    detail_scroll: u16,
    opener_result: Option<std::sync::mpsc::Receiver<String>>,
    rows: Vec<(Rect, usize)>,
}
struct ProjectSelection {
    project: Option<std::path::PathBuf>,
    file: Option<std::path::PathBuf>,
    worktree: Option<std::path::PathBuf>,
}
impl App {
    fn new(audit_inputs: Option<audit::Inputs>) -> Self {
        Self {
            clipboard_supported: std::env::var("TERM")
                .is_ok_and(|term| !term.is_empty() && !matches!(term.as_str(), "dumb" | "linux")),
            mops: handoff::Snapshot::default(),
            mops_inputs: None,
            mops_selected: 0,
            mops_rows: vec![],
            mops_actions: vec![],
            audit_pr_context: None,
            view: if audit_inputs.is_some() {
                View::Mayhem
            } else {
                View::Workshop
            },
            audit: None,
            audit_error: None,
            audit_inputs,
            audit_selected: 0,
            audit_scroll: 0,
            audit_rows: vec![],
            project_data: project_data::ProjectSnapshot::default(),
            project_selected: 0,
            project_file_selected: 0,
            worktree_selected: 0,
            focus_projects: false,
            collapsed: [false; 4],
            project_rows: vec![],
            project_file_rows: vec![],
            worktree_rows: vec![],
            open_button: None,
            tabs: vec![],
            data: data::Snapshot::read(),
            refreshing: false,
            selected: 0,
            message: String::new(),
            details: false,
            detail_scroll: 0,
            opener_result: None,
            rows: vec![],
        }
    }
    // Completion transitions preserve identity independently of terminal dispatch.
    fn mops_observed(&mut self, mut snapshot: handoff::Snapshot) {
        let number = self
            .mops
            .tasks
            .get(self.mops_selected)
            .map(|task| task.pr_number);
        if snapshot.tasks.is_empty() && !self.mops.tasks.is_empty() && snapshot.error.is_some() {
            snapshot.tasks = std::mem::take(&mut self.mops.tasks);
            snapshot.source = format!(
                "STALE: previous imported/observed metadata retained; {}",
                snapshot.source
            );
        }
        self.mops = snapshot;
        self.mops_selected = number
            .and_then(|number| {
                self.mops
                    .tasks
                    .iter()
                    .position(|task| task.pr_number == number)
            })
            .unwrap_or(0);
        self.refreshing = false;
        self.message = self
            .mops
            .error
            .clone()
            .unwrap_or_else(|| "PR metadata read; no harness contacted".into());
    }
    fn audit_observed(&mut self, result: Result<Box<audit::Reports>, String>) {
        self.refreshing = false;
        if !self.audit_completion_matches(&result) {
            return;
        }
        match result {
            Ok(report) => {
                if self.audit_tool_accepted(&report) {
                    self.install_audit(*report);
                }
            }
            Err(error) => {
                self.audit_error = Some(error.clone());
                self.message = error;
            }
        }
    }
    fn audit_completion_matches(&mut self, result: &Result<Box<audit::Reports>, String>) -> bool {
        let Some(scope) = &self.audit_pr_context else {
            return true;
        };
        if result.as_ref().is_ok_and(|report| {
            scope.matches(
                report.baseline.sha.as_deref(),
                report.candidate.sha.as_deref(),
            )
        }) {
            return true;
        }
        self.audit = None;
        self.audit_error = Some(scope.markdown());
        self.message =
            "Discarded artifact result: it does not match the selected PR base/head".into();
        false
    }
    fn audit_tool_accepted(&mut self, report: &audit::Reports) -> bool {
        let Some(requested) = self
            .audit_inputs
            .as_ref()
            .and_then(|input| input.tool_filter.as_deref())
        else {
            return true;
        };
        if let Err(error) = report.validate_tool(requested) {
            self.audit_error = Some(error.clone());
            self.message = error;
            return false;
        }
        true
    }
    fn install_audit(&mut self, report: audit::Reports) {
        self.audit_selected = self.preferred_audit_tool(&report);
        self.audit_scroll = self
            .audit_scroll
            .min(report.detail_count(self.audit_selected).saturating_sub(1));
        self.audit = Some(report);
        self.audit_error = None;
        self.message = "Native audit artifacts read; no checks run".into();
    }
    fn preferred_audit_tool(&self, report: &audit::Reports) -> usize {
        let id = self
            .audit
            .as_ref()
            .and_then(|audit| audit.tool_ids().get(self.audit_selected).cloned());
        let id = id.or_else(|| {
            self.audit_inputs
                .as_ref()
                .and_then(|input| input.tool_filter.clone())
        });
        id.and_then(|id| report.tool_ids().iter().position(|tool| *tool == id))
            .unwrap_or_else(|| {
                (0..report.tool_ids().len())
                    .find(|index| report.detail_count(*index) > 0)
                    .unwrap_or(0)
            })
    }
    fn observed(&mut self, snapshot: data::Snapshot, projects: project_data::ProjectSnapshot) {
        let selection = self.project_selection();
        self.project_data = projects;
        self.restore_project_selection(selection);
        self.settings_observed(snapshot);
        self.refreshing = false;
        self.message = "Passive observation complete • d details".into();
    }
    fn project_selection(&self) -> ProjectSelection {
        let project = self.project_data.projects.get(self.project_selected);
        ProjectSelection {
            project: project.map(|project| project.identity.clone()),
            file: project
                .and_then(|project| project.files.get(self.project_file_selected))
                .map(|file| file.path.clone()),
            worktree: self
                .project_data
                .worktrees
                .get(self.worktree_selected)
                .map(|worktree| worktree.path.clone()),
        }
    }
    fn restore_project_selection(&mut self, selection: ProjectSelection) {
        self.project_selected = selection
            .project
            .and_then(|id| {
                self.project_data
                    .projects
                    .iter()
                    .position(|project| project.identity == id)
            })
            .unwrap_or(
                self.project_selected
                    .min(self.project_data.projects.len().saturating_sub(1)),
            );
        let files = self
            .project_data
            .projects
            .get(self.project_selected)
            .map(|project| project.files.as_slice())
            .unwrap_or(&[]);
        self.project_file_selected = selection
            .file
            .and_then(|id| files.iter().position(|file| file.path == id))
            .unwrap_or(
                self.project_file_selected
                    .min(files.len().saturating_sub(1)),
            );
        self.worktree_selected = selection
            .worktree
            .and_then(|id| {
                self.project_data
                    .worktrees
                    .iter()
                    .position(|worktree| worktree.path == id)
            })
            .unwrap_or(
                self.worktree_selected
                    .min(self.project_data.worktrees.len().saturating_sub(1)),
            );
    }
    fn settings_observed(&mut self, snapshot: data::Snapshot) {
        let name = self
            .data
            .files
            .get(self.selected)
            .map(|file| file.name.clone());
        self.data = snapshot;
        self.selected = name
            .and_then(|name| self.data.files.iter().position(|file| file.name == name))
            .unwrap_or(self.selected.min(self.data.files.len().saturating_sub(1)));
    }
    fn refresh(&mut self, control: provider::Control, sender: std::sync::mpsc::Sender<Wake>) {
        if self.refreshing {
            self.message = "Passive observation already in progress".into();
            return;
        }
        if self.view == View::Mops {
            self.refresh_mops(control, sender);
            return;
        }
        if !self.audit_scope_ready() {
            return;
        }
        self.refreshing = true;
        if let Some(inputs) = self
            .audit_inputs
            .clone()
            .filter(|_| self.view == View::Mayhem)
        {
            std::thread::spawn(move || {
                let _ = sender.send(Wake::AuditObserved(inputs.load().map(Box::new)));
            });
            return;
        }
        std::thread::spawn(move || {
            let mut snapshot = data::Snapshot::read();
            snapshot.apply(provider::read(&control));
            let projects = project_data::ProjectSnapshot::read(&control);
            let _ = sender.send(Wake::Observed(Box::new(snapshot), Box::new(projects)));
        });
    }
    // A PR-selected view may only load artifacts attributed to that exact base/head.
    fn audit_scope_ready(&mut self) -> bool {
        let Some(scope) = self
            .audit_pr_context
            .as_ref()
            .filter(|_| self.view == View::Mayhem)
        else {
            return true;
        };
        if self.audit_inputs.as_ref().is_some_and(|input| {
            scope.matches(
                input.baseline_sha.as_deref(),
                input.candidate_sha.as_deref(),
            )
        }) {
            return true;
        }
        self.audit = None;
        self.audit_error = Some(scope.markdown());
        self.message = "No audit artifacts matching this exact PR base/head".into();
        false
    }
    fn refresh_mops(&mut self, control: provider::Control, sender: std::sync::mpsc::Sender<Wake>) {
        let Some(inputs) = self.mops_inputs.clone() else {
            self.mops.error=Some("Configure --mops-repo OWNER/REPO and explicit --mops-pr NUMBER / --mops-pr-file PATH".into());
            return;
        };
        let roles = self
            .data
            .files
            .iter()
            .filter(|f| f.group == "MagicStudio agent roles" && f.found)
            .filter_map(|f| f.path.clone())
            .collect::<Vec<_>>();
        self.refreshing = true;
        std::thread::spawn(move || {
            let snapshot = handoff::Snapshot::read(&inputs, &roles, &control);
            let _ = sender.send(Wake::MopsObserved(Box::new(snapshot)));
        });
    }
    fn mops_action(&mut self, action: MopsAction) {
        let Some(task) = self.mops.tasks.get(self.mops_selected) else {
            self.message = "No selected PR handoff".into();
            return;
        };
        if matches!(action, MopsAction::CopyResume | MopsAction::CopyFeedback)
            && !self.clipboard_supported
        {
            self.message = "Clipboard unavailable: terminal OSC52 support required".into();
            return;
        }
        match action {
            MopsAction::OpenPr => {
                let url = task.pr_url.clone();
                self.open_pr_url(&url);
            }
            MopsAction::OpenRole => {
                let path = task
                    .role_path
                    .as_ref()
                    .filter(|_| task.role_available)
                    .map(std::path::PathBuf::from);
                self.open_recorded_path(
                    path,
                    "Role path unavailable / not verified against crew.list",
                );
            }
            MopsAction::OpenWorktree => {
                let path = task
                    .worktree
                    .as_ref()
                    .filter(|_| task.worktree_available)
                    .map(std::path::PathBuf::from);
                self.open_recorded_path(path, "Recorded worktree unavailable");
            }
            MopsAction::CopyResume => {
                self.message = copy_resume(task);
            }
            MopsAction::CopyFeedback => {
                let feedback = task.feedback.join("\n");
                if feedback.is_empty() {
                    self.message = "No review feedback supplied".into()
                } else {
                    self.message = copy_text(&feedback)
                }
            }
            MopsAction::ViewMayhem => {
                let scope = task.audit_scope();
                let message = scope.markdown();
                self.audit_pr_context = Some(scope);
                self.audit = None;
                self.audit_error = Some(message);
                self.view = View::Mayhem;
            }
        }
    }
    fn open_recorded_path(&mut self, path: Option<std::path::PathBuf>, unavailable: &str) {
        match path {
            Some(path) => self.launch(&path),
            None => self.message = unavailable.into(),
        }
    }
    fn open_pr_url(&mut self, url: &str) {
        self.message = match open_url(url) {
            Ok(receiver) => {
                self.opener_result = Some(receiver);
                "Browser opener requested; outcome unknown".into()
            }
            Err(error) => error.to_string(),
        };
    }
    fn launch(&mut self, path: &Path) {
        self.message = match open_external(path) {
            Ok(receiver) => {
                self.opener_result = Some(receiver);
                "External opener started; application outcome unknown".into()
            }
            Err(error) => format!("External opener failed: {error}"),
        };
    }
    fn open_current(&mut self, folder: bool) {
        if self.view == View::Mops {
            self.mops_action(if folder {
                MopsAction::OpenWorktree
            } else {
                MopsAction::OpenPr
            });
            return;
        }
        if self.is_audit_mayhem() {
            self.open_audit_artifact();
            return;
        }
        if self.view == View::Settings {
            self.open(if folder { usize::MAX } else { self.selected });
            return;
        }
        if let Some(path) = self.current_target_path(folder) {
            self.launch(&path);
        } else {
            self.message = "Unavailable: no observed accessible target".into();
        }
    }
    fn is_audit_mayhem(&self) -> bool {
        self.view == View::Mayhem
            && (self.audit_inputs.is_some() || self.audit_pr_context.is_some())
    }
    fn open_audit_artifact(&mut self) {
        let path = self
            .audit
            .as_ref()
            .and_then(|a| a.artifact(self.audit_selected));
        if let Some(path) = path {
            self.launch(&path);
        } else {
            self.message = "Native artifact unavailable; press d for report details".into();
        }
    }
    fn current_target_path(&self, folder: bool) -> Option<std::path::PathBuf> {
        match self.view {
            View::Workshop => self.workshop_target_path(folder),
            View::Mayhem => self.mayhem_target_path(),
            View::Settings => self
                .data
                .files
                .get(self.selected)
                .filter(|file| file.found)
                .and_then(|file| file.path.clone())
                .filter(|p| p.is_file()),
            View::Mops => None,
        }
    }
    fn mayhem_target_path(&self) -> Option<std::path::PathBuf> {
        if self.focus_projects {
            self.project_data
                .projects
                .get(self.project_selected)
                .filter(|p| p.exists)
                .map(|p| p.path.clone())
                .or_else(|| {
                    self.project_data
                        .worktrees
                        .first()
                        .filter(|w| w.available)
                        .map(|w| w.path.clone())
                })
        } else {
            self.project_data
                .worktrees
                .get(self.worktree_selected)
                .filter(|w| w.available)
                .map(|w| w.path.clone())
        }
    }
    fn workshop_target_path(&self, folder: bool) -> Option<std::path::PathBuf> {
        let project = self.project_data.projects.get(self.project_selected)?;
        if folder && self.focus_projects {
            return project.exists.then(|| project.path.clone());
        }
        let file = project
            .files
            .get(self.project_file_selected)
            .filter(|f| f.available)?;
        if folder {
            file.path.parent().map(Path::to_path_buf)
        } else {
            Some(file.path.clone())
        }
    }
    fn move_selection(&mut self, down: bool) {
        if self.view == View::Mops {
            step_index(&mut self.mops_selected, self.mops.tasks.len(), down);
            return;
        }
        if self.is_audit_mayhem() {
            let len = self.audit.as_ref().map_or(0, |a| a.tool_ids().len());
            step_index(&mut self.audit_selected, len, down);
            self.audit_scroll = 0;
            return;
        }
        self.step_primary_selection(down);
        if self.focus_projects {
            self.project_file_selected = 0;
        } else if self.view == View::Workshop {
            self.expand_selected_workshop_group();
        }
    }
    fn step_primary_selection(&mut self, down: bool) {
        let (selected, len) = if self.view == View::Settings {
            (&mut self.selected, self.data.files.len())
        } else if self.focus_projects {
            (&mut self.project_selected, self.project_data.projects.len())
        } else if self.view == View::Mayhem {
            (
                &mut self.worktree_selected,
                self.project_data.worktrees.len(),
            )
        } else {
            (
                &mut self.project_file_selected,
                self.project_data
                    .projects
                    .get(self.project_selected)
                    .map_or(0, |p| p.files.len()),
            )
        };
        step_index(selected, len, down);
    }
    fn expand_selected_workshop_group(&mut self) {
        if let Some(file) = self
            .project_data
            .projects
            .get(self.project_selected)
            .and_then(|p| p.files.get(self.project_file_selected))
            && let Some(group) = ["Storyboard", "Animations", "Lyrics", "Studio"]
                .iter()
                .position(|g| *g == file.group)
        {
            self.collapsed[group] = false;
        }
    }
    fn open(&mut self, index: usize) {
        if data::inspect_row(index).is_some() {
            self.details = true;
            self.detail_scroll = 0;
            return;
        }
        if index == usize::MAX {
            let path = self.data.projects_root.as_deref().filter(|p| p.is_dir());
            self.message = match path {
                Some(path) => match open_external(path) {
                    Ok(receiver) => {
                        self.opener_result = Some(receiver);
                        "External folder opener started; application outcome unknown".into()
                    }
                    Err(e) => format!("External opener failed: {e}"),
                },
                None => "Project root unavailable".into(),
            };
            return;
        }
        let Some(file) = self.data.files.get(index) else {
            return;
        };
        let Some(path) = file.path.as_deref().filter(|p| {
            file.found && std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_file())
        }) else {
            self.message = "Unavailable: no accessible external target".into();
            return;
        };
        self.message = match open_external(path) {
            Ok(receiver) => {
                self.opener_result = Some(receiver);
                "External opener started; application outcome unknown".into()
            }
            Err(e) => format!("External opener failed: {e}"),
        };
    }
}
fn open_external(path: &Path) -> io::Result<std::sync::mpsc::Receiver<String>> {
    // Canonical absolute filesystem arguments cannot become shell commands or URL schemes.
    let path = path.canonicalize()?;
    spawn_opener(path.as_os_str())
}
fn open_url(url: &str) -> io::Result<std::sync::mpsc::Receiver<String>> {
    if !url.starts_with("https://github.com/") || url.chars().any(char::is_control) {
        return Err(io::Error::other(
            "Only validated GitHub HTTPS links are supported",
        ));
    }
    spawn_opener(std::ffi::OsStr::new(url))
}
fn copy_text(text: &str) -> String {
    if text.len() > 65536 {
        return "Clipboard payload exceeds64KiB; inspect full native details".into();
    }
    if std::env::var("TERM").is_ok_and(|t| matches!(t.as_str(), "dumb" | "linux")) {
        return "Clipboard unavailable: terminal OSC52 support required".into();
    }
    use crossterm::clipboard::{ClipboardSelection, ClipboardType, CopyToClipboard};
    match execute!(
        io::stdout(),
        CopyToClipboard {
            content: text,
            destination: ClipboardSelection(vec![ClipboardType::Clipboard])
        }
    ) {
        Ok(()) => {
            "OSC52 copy requested; terminal permission/support not verified. Nothing executed."
                .into()
        }
        Err(error) => format!("Clipboard request failed: {error}"),
    }
}
fn spawn_opener(target: &std::ffi::OsStr) -> io::Result<std::sync::mpsc::Receiver<String>> {
    #[cfg(target_os = "linux")]
    let mut cmd = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer.exe");
        c.arg("/select,");
        c
    };
    let mut child = cmd
        .arg(target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Reap only this explicit desktop-opener child; no scan, timer or UI blocking.
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let message = match child.wait() {
            Ok(status) if status.success() => {
                "External opener exited successfully; application outcome unknown".into()
            }
            Ok(status) => format!("External opener failed: {status}"),
            Err(error) => format!("External opener wait failed: {error}"),
        };
        let _ = sender.send(message);
    });
    Ok(receiver)
}
fn copy_resume(task: &handoff::Task) -> String {
    match task.copyable_resume() {
        Some(command) => copy_text(command),
        None => "No copyable recorded resume command; nothing copied or executed".into(),
    }
}
fn print_pr_report(args: &[String], index: usize) -> io::Result<()> {
    let number = args
        .get(index + 1)
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| io::Error::other("--mayhem-pr-report requires a PR number"))?;
    let inputs = handoff::Inputs::parse(args)
        .map_err(io::Error::other)?
        .ok_or_else(|| io::Error::other("PR inputs required"))?;
    let control = provider::Control::default();
    let _lifecycle = provider::Owner(control.clone());
    let snapshot = handoff::Snapshot::read(&inputs, &[], &control);
    let task = snapshot
        .tasks
        .iter()
        .find(|t| t.pr_number == number)
        .ok_or_else(|| {
            io::Error::other(
                snapshot
                    .error
                    .unwrap_or_else(|| "Requested PR unavailable".into()),
            )
        })?;
    print!("{}", task.audit_unavailable());
    Ok(())
}
fn configured_app(args: &[String]) -> io::Result<App> {
    let audit_inputs = if args.iter().any(|a| a.starts_with("--audit-")) {
        Some(audit::Inputs::parse(args).map_err(io::Error::other)?)
    } else {
        None
    };
    let mops_inputs = handoff::Inputs::parse(args).map_err(io::Error::other)?;
    let mut app = App::new(audit_inputs);
    if mops_inputs.is_some() {
        app.view = View::Mops;
        app.mops_inputs = mops_inputs;
    }
    if app.audit_inputs.is_some() {
        app.project_data.source =
            "Audit artifact session; native project observation not requested".into();
        app.project_data.error = Some("Launch without audit flags for project observation".into());
        app.data.provider_detail =
            "Audit artifact session; configuration query not requested".into();
    }
    Ok(app)
}

fn step_index(selected: &mut usize, len: usize, down: bool) {
    *selected = if down {
        selected.saturating_add(1).min(len.saturating_sub(1))
    } else {
        selected.saturating_sub(1)
    };
}

/// A background observation requests an immediate redraw; it never waits for a key.
enum UiEvent {
    Input(Event),
    Redraw,
    Shutdown,
}
fn receive_ui_event(
    receiver: &std::sync::mpsc::Receiver<Wake>,
    app: &mut App,
) -> io::Result<UiEvent> {
    match receiver
        .recv()
        .map_err(|_| io::Error::other("UI event channel closed"))?
    {
        Wake::Shutdown => Ok(UiEvent::Shutdown),
        Wake::Terminal(input) => input.map(UiEvent::Input),
        Wake::MopsObserved(snapshot) => {
            app.mops_observed(*snapshot);
            Ok(UiEvent::Redraw)
        }
        Wake::AuditObserved(result) => {
            app.audit_observed(result);
            Ok(UiEvent::Redraw)
        }
        Wake::Observed(snapshot, projects) => {
            app.observed(*snapshot, *projects);
            Ok(UiEvent::Redraw)
        }
    }
}

impl App {
    fn poll_opener_result(&mut self) {
        // Inspect only our explicit opener result on terminal input, never on a timer.
        if let Some(receiver) = &self.opener_result
            && let Ok(message) = receiver.try_recv()
        {
            self.message = message;
            self.opener_result = None;
        }
    }
    fn handle_input(
        &mut self,
        input: Event,
        control: &provider::Control,
        sender: &std::sync::mpsc::Sender<Wake>,
    ) -> bool {
        match input {
            Event::Key(k) => {
                if k.kind == KeyEventKind::Press {
                    return self.handle_key_press(k.code, control, sender);
                }
                false
            }
            Event::Mouse(m) => {
                if !self.details && m.kind == MouseEventKind::Down(MouseButton::Left) {
                    self.handle_mouse_click(ratatui::layout::Position::new(m.column, m.row));
                }
                false
            }
            _ => false,
        }
    }
    fn handle_key_press(
        &mut self,
        code: KeyCode,
        control: &provider::Control,
        sender: &std::sync::mpsc::Sender<Wake>,
    ) -> bool {
        if let Some(quit) = self.handle_details_key(code) {
            return quit;
        }
        if code == KeyCode::F(4) {
            self.view = View::Mayhem;
            if self.audit_inputs.is_none() {
                self.audit_inputs = audit::Inputs::from_saved_analytics();
            }
            if self.audit_inputs.is_some() {
                self.refresh(control.clone(), sender.clone());
            }
            return false;
        }
        if self.handle_navigation_key(code)
            || self.handle_audit_key(code)
            || self.handle_mops_key(code)
        {
            return false;
        }
        self.handle_action_key(code, control, sender)
    }
    fn handle_details_key(&mut self, code: KeyCode) -> Option<bool> {
        if !self.details && !matches!(code, KeyCode::Char('d' | 'q')) {
            return None;
        }
        match code {
            KeyCode::Esc => self.details = false,
            KeyCode::Char('d') => self.toggle_details(),
            KeyCode::Char('q') => return Some(true),
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            _ => {}
        }
        Some(false)
    }
    fn handle_navigation_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(true),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(false),
            KeyCode::Char(c @ '1'..='4') => self.toggle_workshop_group(c),
            KeyCode::Tab => self.focus_projects = !self.focus_projects,
            KeyCode::F(1) => self.view = View::Workshop,
            KeyCode::F(2) => self.message = "Booth is not implemented".into(),
            KeyCode::F(3) => self.view = View::Mops,
            KeyCode::F(4) => self.view = View::Mayhem,
            KeyCode::F(5) => self.view = View::Settings,
            _ => return false,
        }
        true
    }
    fn handle_audit_key(&mut self, code: KeyCode) -> bool {
        if self.view != View::Mayhem || self.audit_inputs.is_none() {
            return false;
        }
        match code {
            KeyCode::Char('B') => self.open_audit_json(true),
            KeyCode::Char('C') => self.open_audit_json(false),
            KeyCode::PageUp | KeyCode::Char('[') => {
                self.audit_scroll = self.audit_scroll.saturating_sub(1);
            }
            KeyCode::PageDown | KeyCode::Char(']') => {
                let n = self
                    .audit
                    .as_ref()
                    .map_or(0, |a| a.detail_count(self.audit_selected));
                step_index(&mut self.audit_scroll, n, true);
            }
            _ => return false,
        }
        true
    }
    fn open_audit_json(&mut self, baseline: bool) {
        let path = self.audit.as_ref().map(|a| {
            if baseline {
                a.baseline.path.clone()
            } else {
                a.candidate.path.clone()
            }
        });
        if let Some(path) = path.filter(|p| p.is_file()) {
            self.launch(&path);
        }
    }
    fn handle_mops_key(&mut self, code: KeyCode) -> bool {
        if self.view != View::Mops {
            return false;
        }
        let action = match code {
            KeyCode::Enter | KeyCode::Char('o') => MopsAction::OpenPr,
            KeyCode::Char('c') => MopsAction::CopyResume,
            KeyCode::Char('f') => MopsAction::CopyFeedback,
            KeyCode::Char('p') => MopsAction::OpenWorktree,
            KeyCode::Char('l') => MopsAction::OpenRole,
            KeyCode::Char('m') => MopsAction::ViewMayhem,
            _ => return false,
        };
        self.mops_action(action);
        true
    }
    fn handle_action_key(
        &mut self,
        code: KeyCode,
        control: &provider::Control,
        sender: &std::sync::mpsc::Sender<Wake>,
    ) -> bool {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Enter | KeyCode::Char('o') => self.open_current(false),
            KeyCode::Char('p') => self.open_current(true),
            KeyCode::Char('r' | 'R') => self.refresh(control.clone(), sender.clone()),
            KeyCode::Char('a') if self.view == View::Mayhem => {
                self.toggle_saved_audit(control, sender);
            }
            _ => {}
        }
        false
    }
    fn toggle_saved_audit(
        &mut self,
        control: &provider::Control,
        sender: &std::sync::mpsc::Sender<Wake>,
    ) {
        if self.audit_inputs.is_some() {
            self.audit_inputs = None;
            self.audit = None;
            self.audit_error = None;
            return;
        }
        if let Some(inputs) = audit::Inputs::from_saved_analytics() {
            self.audit_inputs = Some(inputs);
            self.refresh(control.clone(), sender.clone());
        } else {
            self.message = "No saved audit report; run magic-cli audit first".into();
        }
    }
    fn toggle_workshop_group(&mut self, c: char) {
        if self.view == View::Workshop
            && let Some(digit) = c.to_digit(10)
        {
            let i = (digit - 1) as usize;
            self.collapsed[i] = !self.collapsed[i];
        }
    }
    fn toggle_details(&mut self) {
        self.details = !self.details;
        self.detail_scroll = 0;
    }
    fn handle_mouse_click(&mut self, p: ratatui::layout::Position) {
        if self.details {
            return;
        }
        if self.handle_overlay_or_tab_click(p) {
            return;
        }
        self.handle_row_or_button_click(p);
    }
    fn handle_overlay_or_tab_click(&mut self, p: ratatui::layout::Position) -> bool {
        if let Some((_, action)) = self.mops_actions.iter().find(|(r, _)| r.contains(p)) {
            self.mops_action(*action);
        } else if let Some((_, i)) = self.mops_rows.iter().find(|(r, _)| r.contains(p)) {
            self.mops_selected = *i;
        } else if let Some((_, i)) = self.audit_rows.iter().find(|(r, _)| r.contains(p)) {
            self.audit_selected = *i;
            self.audit_scroll = 0;
        } else if let Some((_, view)) = self.tabs.iter().find(|(r, _)| r.contains(p)) {
            self.view = *view;
        } else if self.open_button.is_some_and(|r| r.contains(p)) {
            self.open_current(false);
        } else {
            return false;
        }
        true
    }
    fn handle_row_or_button_click(&mut self, p: ratatui::layout::Position) {
        if let Some((_, i)) = self.worktree_rows.iter().find(|(r, _)| r.contains(p)) {
            self.worktree_selected = *i;
            self.focus_projects = false;
        } else if let Some((_, i)) = self.project_file_rows.iter().find(|(r, _)| r.contains(p)) {
            self.project_file_selected = *i;
            self.focus_projects = false;
        } else if let Some((_, i)) = self.project_rows.iter().find(|(r, _)| r.contains(p)) {
            self.project_selected = *i;
            self.project_file_selected = 0;
            self.focus_projects = true;
        } else if let Some((_, i)) = self.rows.iter().find(|(r, _)| r.contains(p)) {
            self.selected = *i;
        }
    }
}

fn maybe_run_cli_report(args: &[String]) -> io::Result<bool> {
    if let Some(index) = args.iter().position(|a| a == "--mayhem-pr-report") {
        print_pr_report(args, index)?;
        return Ok(true);
    }
    if args.iter().any(|a| a == "--mayhem-report") {
        let input = audit::Inputs::parse(args).map_err(io::Error::other)?;
        let report = input.load().map_err(io::Error::other)?;
        let output = match input.tool_filter.as_deref() {
            Some(tool) => report.markdown_tool(tool).map_err(io::Error::other)?,
            None => report.markdown(),
        };
        print!("{output}");
        return Ok(true);
    }
    Ok(false)
}
fn apply_color_flag(args: &[String]) {
    match args
        .iter()
        .find(|a| a.starts_with("--color="))
        .map(String::as_str)
    {
        Some("--color=always") => crossterm::style::force_color_output(true),
        Some("--color=never") => crossterm::style::force_color_output(false),
        _ => {}
    }
}
fn spawn_terminal_reader(input_sender: std::sync::mpsc::Sender<Wake>) {
    std::thread::spawn(move || {
        loop {
            let input = event::read();
            let failed = input.is_err();
            if input_sender.send(Wake::Terminal(input)).is_err() || failed {
                break;
            }
        }
    });
}
fn run_event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    control: &provider::Control,
    sender: &std::sync::mpsc::Sender<Wake>,
    receiver: &std::sync::mpsc::Receiver<Wake>,
) -> io::Result<()> {
    loop {
        app.poll_opener_result();
        terminal.draw(|f| ui::draw(f, app))?;
        let input = match receive_ui_event(receiver, app)? {
            UiEvent::Input(input) => input,
            UiEvent::Redraw => continue,
            UiEvent::Shutdown => break Ok(()),
        };
        if app.handle_input(input, control, sender) {
            break Ok(());
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if maybe_run_cli_report(&args)? {
        return Ok(());
    }
    apply_color_flag(&args);
    let mut app = configured_app(&args)?;
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let control = provider::Control::default();
    let _lifecycle = provider::Owner(control.clone());
    let (sender, receiver) = std::sync::mpsc::channel();
    // Drop closes the signal listener after normal exit or an early UI error.
    #[cfg(unix)]
    let _shutdown = ShutdownSignals::new(sender.clone())?;
    spawn_terminal_reader(sender.clone());
    if app.view != View::Mops {
        app.refresh(control.clone(), sender.clone());
    }
    let result = run_event_loop(&mut terminal, &mut app, &control, &sender, &receiver);
    control.cancel();
    let cleanup = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result.and(cleanup)
}
