//! Settings-only observer. No engine linkage, agent connections, sockets or model clients.
mod audit;
mod data;
mod handoff;
mod mayhem;
mod mops;
mod project_data;
mod provider;
mod ui;
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
    MopsObserved(Box<handoff::Snapshot>),
    AuditObserved(Result<Box<audit::Comparison>, String>),
    Terminal(io::Result<Event>),
    Observed(Box<data::Snapshot>, Box<project_data::ProjectSnapshot>),
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
    audit: Option<audit::Comparison>,
    audit_error: Option<String>,
    audit_inputs: Option<audit::Inputs>,
    audit_selected: usize,
    audit_finding_selected: usize,
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
    group_rows: Vec<(Rect, usize)>,
    project_buttons: Vec<(Rect, std::path::PathBuf)>,
    tabs: Vec<(Rect, View)>,
    data: data::Snapshot,
    refreshing: bool,
    selected: usize,
    offset: usize,
    message: String,
    details: bool,
    detail_scroll: u16,
    outlined_controls: bool,
    opener_result: Option<std::sync::mpsc::Receiver<String>>,
    buttons: Vec<(Rect, usize)>,
    rows: Vec<(Rect, usize)>,
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
            audit_finding_selected: 0,
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
            group_rows: vec![],
            project_buttons: vec![],
            tabs: vec![],
            data: data::Snapshot::read(),
            refreshing: false,
            selected: 0,
            offset: 0,
            message: String::new(),
            details: false,
            detail_scroll: 0,
            opener_result: None,
            outlined_controls: true,
            buttons: vec![],
            rows: vec![],
        }
    }
    fn refresh(&mut self, control: provider::Control, sender: std::sync::mpsc::Sender<Wake>) {
        if self.refreshing {
            self.message = "Passive observation already in progress".into();
            return;
        }
        if self.view == View::Mops {
            let Some(inputs) = self.mops_inputs.clone() else {
                self.mops.error=Some("Configure --mops-repo OWNER/REPO and explicit --mops-pr NUMBER / --mops-pr-file PATH".into());
                return;
            };
            let roles = self
                .data
                .files
                .iter()
                .filter(|f| f.group == "OpenMontage agent roles" && f.found)
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>();
            self.refreshing = true;
            std::thread::spawn(move || {
                let snapshot = handoff::Snapshot::read(&inputs, &roles, &control);
                let _ = sender.send(Wake::MopsObserved(Box::new(snapshot)));
            });
            return;
        }
        if self.view == View::Mayhem
            && let Some(scope) = &self.audit_pr_context
        {
            let matches = self.audit_inputs.as_ref().is_some_and(|input| {
                scope.matches(
                    input.baseline_sha.as_deref(),
                    input.candidate_sha.as_deref(),
                )
            });
            if !matches {
                self.audit = None;
                self.audit_error = Some(scope.markdown());
                self.message = "No audit artifacts matching this exact PR base/head".into();
                return;
            }
        }
        self.refreshing = true;
        if let Some(inputs) = self.audit_inputs.clone() {
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
    fn mops_action(&mut self, action: MopsAction) {
        let Some(task) = self.mops.tasks.get(self.mops_selected) else {
            self.message = "No selected PR handoff".into();
            return;
        };
        match action {
            MopsAction::OpenPr => {
                let url = task.pr_url.clone();
                self.message = match open_url(&url) {
                    Ok(receiver) => {
                        self.opener_result = Some(receiver);
                        "Browser opener requested; outcome unknown".into()
                    }
                    Err(error) => error.to_string(),
                };
            }
            MopsAction::OpenRole => {
                let path = task
                    .role_path
                    .as_ref()
                    .filter(|_| task.role_available)
                    .map(std::path::PathBuf::from);
                if let Some(path) = path {
                    self.launch(&path)
                } else {
                    self.message = "Role path unavailable / not verified against crew.list".into();
                }
            }
            MopsAction::OpenWorktree => {
                let path = task
                    .worktree
                    .as_ref()
                    .filter(|_| task.worktree_available)
                    .map(std::path::PathBuf::from);
                if let Some(path) = path {
                    self.launch(&path)
                } else {
                    self.message = "Recorded worktree unavailable".into();
                }
            }
            MopsAction::CopyResume => {
                let command = task
                    .resume_command
                    .as_ref()
                    .filter(|_| task.verified_resume)
                    .cloned();
                if let Some(command) = command {
                    self.message = copy_text(&command)
                } else {
                    self.message =
                        "No author-verified resume command; nothing copied or executed".into();
                }
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
        if self.view == View::Mayhem
            && (self.audit_inputs.is_some() || self.audit_pr_context.is_some())
        {
            let path = self
                .audit
                .as_ref()
                .and_then(|a| a.finding_link(self.audit_selected, self.audit_finding_selected))
                .map(Path::to_path_buf);
            if let Some(path) = path {
                self.launch(&path)
            } else {
                self.message =
                    "Native finding source link unavailable/unverified; report remains inspectable"
                        .into();
            }
            return;
        }
        if self.view == View::Settings {
            self.open(if folder { usize::MAX } else { self.selected });
            return;
        }
        let path = if self.view == View::Mayhem && self.focus_projects {
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
        } else if self.view == View::Mayhem {
            self.project_data
                .worktrees
                .get(self.worktree_selected)
                .filter(|w| w.available)
                .map(|w| w.path.clone())
        } else if folder && !self.focus_projects {
            self.project_data
                .projects
                .get(self.project_selected)
                .and_then(|p| p.files.get(self.project_file_selected))
                .filter(|f| f.available)
                .and_then(|f| f.path.parent())
                .map(Path::to_path_buf)
        } else if folder {
            self.project_data
                .projects
                .get(self.project_selected)
                .filter(|p| p.exists)
                .map(|p| p.path.clone())
        } else {
            self.project_data
                .projects
                .get(self.project_selected)
                .and_then(|p| p.files.get(self.project_file_selected))
                .filter(|f| f.available)
                .map(|f| f.path.clone())
        };
        if let Some(path) = path {
            self.launch(&path)
        } else {
            self.message = "Unavailable: no observed accessible target".into();
        }
    }
    fn move_selection(&mut self, down: bool) {
        if self.view == View::Mops {
            self.mops_selected = if down {
                self.mops_selected
                    .saturating_add(1)
                    .min(self.mops.tasks.len().saturating_sub(1))
            } else {
                self.mops_selected.saturating_sub(1)
            };
            return;
        }
        if self.view == View::Mayhem
            && (self.audit_inputs.is_some() || self.audit_pr_context.is_some())
        {
            let len = self.audit.as_ref().map_or(0, |a| a.tool_ids().len());
            self.audit_selected = if down {
                self.audit_selected
                    .saturating_add(1)
                    .min(len.saturating_sub(1))
            } else {
                self.audit_selected.saturating_sub(1)
            };
            self.audit_finding_selected = 0;
            return;
        }
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
        *selected = if down {
            selected.saturating_add(1).min(len.saturating_sub(1))
        } else {
            selected.saturating_sub(1)
        };
        if self.focus_projects {
            self.project_file_selected = 0;
        } else if self.view == View::Workshop
            && let Some(file) = self
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
fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(index) = args.iter().position(|a| a == "--mayhem-pr-report") {
        let number = args
            .get(index + 1)
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| io::Error::other("--mayhem-pr-report requires a PR number"))?;
        let inputs = handoff::Inputs::parse(&args)
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
        return Ok(());
    }
    if args.iter().any(|a| a == "--mayhem-report") {
        let input = audit::Inputs::parse(&args).map_err(io::Error::other)?;
        let report = input.load().map_err(io::Error::other)?;
        let output = match input.tool_filter.as_deref() {
            Some(tool) => report.markdown_tool(tool).map_err(io::Error::other)?,
            None => report.markdown(),
        };
        print!("{output}");
        return Ok(());
    }

    match std::env::args()
        .find(|a| a.starts_with("--color="))
        .as_deref()
    {
        Some("--color=always") => crossterm::style::force_color_output(true),
        Some("--color=never") => crossterm::style::force_color_output(false),
        _ => {}
    }
    let audit_inputs = if args.iter().any(|a| a.starts_with("--audit-")) {
        Some(audit::Inputs::parse(&args).map_err(io::Error::other)?)
    } else {
        None
    };
    let mops_inputs = handoff::Inputs::parse(&args).map_err(io::Error::other)?;
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
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let control = provider::Control::default();
    let _lifecycle = provider::Owner(control.clone());
    let (sender, receiver) = std::sync::mpsc::channel();
    let input_sender = sender.clone();
    std::thread::spawn(move || {
        loop {
            let input = event::read();
            let failed = input.is_err();
            if input_sender.send(Wake::Terminal(input)).is_err() || failed {
                break;
            }
        }
    });
    if app.view != View::Mops {
        app.refresh(control.clone(), sender.clone());
    }
    let result = (|| loop {
        // Inspect only our explicit opener result on terminal input, never on a timer.
        if let Some(receiver) = &app.opener_result
            && let Ok(message) = receiver.try_recv()
        {
            app.message = message;
            app.opener_result = None;
        }
        terminal.draw(|f| ui::draw(f, &mut app))?;
        let input = match receiver
            .recv()
            .map_err(|_| io::Error::other("UI event channel closed"))?
        {
            Wake::MopsObserved(snapshot) => {
                let number = app.mops.tasks.get(app.mops_selected).map(|t| t.pr_number);
                let mut snapshot = *snapshot;
                if snapshot.tasks.is_empty()
                    && !app.mops.tasks.is_empty()
                    && snapshot.error.is_some()
                {
                    snapshot.tasks = std::mem::take(&mut app.mops.tasks);
                    snapshot.source = format!(
                        "STALE: previous imported/observed metadata retained; {}",
                        snapshot.source
                    );
                }
                app.mops = snapshot;
                app.mops_selected = number
                    .and_then(|n| app.mops.tasks.iter().position(|t| t.pr_number == n))
                    .unwrap_or(0);
                app.refreshing = false;
                app.message = app
                    .mops
                    .error
                    .clone()
                    .unwrap_or_else(|| "PR metadata read; no harness contacted".into());
                continue;
            }
            Wake::AuditObserved(result) => {
                app.refreshing = false;
                if let Some(scope) = &app.audit_pr_context {
                    let matches = result.as_ref().is_ok_and(|report| {
                        scope.matches(
                            report.baseline.sha.as_deref(),
                            report.candidate.sha.as_deref(),
                        )
                    });
                    if !matches {
                        app.audit = None;
                        app.audit_error = Some(scope.markdown());
                        app.message="Discarded artifact result: it does not match the selected PR base/head".into();
                        continue;
                    }
                }
                match result {
                    Ok(report) => {
                        if let Some(requested) = app
                            .audit_inputs
                            .as_ref()
                            .and_then(|input| input.tool_filter.as_deref())
                            && let Err(error) = report.validate_tool(requested)
                        {
                            app.audit_error = Some(error.clone());
                            app.message = error;
                            continue;
                        }
                        let id = app
                            .audit
                            .as_ref()
                            .and_then(|a| a.tool_ids().get(app.audit_selected).cloned());
                        let id = id.or_else(|| {
                            app.audit_inputs
                                .as_ref()
                                .and_then(|input| input.tool_filter.clone())
                        });
                        app.audit_selected = id
                            .and_then(|id| report.tool_ids().iter().position(|t| *t == id))
                            .unwrap_or_else(|| {
                                (0..report.tool_ids().len())
                                    .find(|i| report.finding_count(*i) > 0)
                                    .unwrap_or(0)
                            });
                        app.audit_finding_selected = app
                            .audit_finding_selected
                            .min(report.finding_count(app.audit_selected).saturating_sub(1));
                        app.audit = Some(*report);
                        app.audit_error = None;
                        app.message = "Native audit artifacts read; no checks run".into();
                    }
                    Err(error) => {
                        app.audit_error = Some(error.clone());
                        app.message = error;
                    }
                }
                continue;
            }
            Wake::Observed(snapshot, projects) => {
                let project_identity = app
                    .project_data
                    .projects
                    .get(app.project_selected)
                    .map(|p| p.identity.clone());
                let file_identity = app
                    .project_data
                    .projects
                    .get(app.project_selected)
                    .and_then(|p| p.files.get(app.project_file_selected))
                    .map(|f| f.path.clone());
                let worktree_identity = app
                    .project_data
                    .worktrees
                    .get(app.worktree_selected)
                    .map(|w| w.path.clone());
                app.project_data = *projects;
                app.project_selected = project_identity
                    .and_then(|id| {
                        app.project_data
                            .projects
                            .iter()
                            .position(|p| p.identity == id)
                    })
                    .unwrap_or(
                        app.project_selected
                            .min(app.project_data.projects.len().saturating_sub(1)),
                    );
                let files = app
                    .project_data
                    .projects
                    .get(app.project_selected)
                    .map(|p| p.files.as_slice())
                    .unwrap_or(&[]);
                app.project_file_selected = file_identity
                    .and_then(|id| files.iter().position(|f| f.path == id))
                    .unwrap_or(app.project_file_selected.min(files.len().saturating_sub(1)));
                app.worktree_selected = worktree_identity
                    .and_then(|id| app.project_data.worktrees.iter().position(|w| w.path == id))
                    .unwrap_or(
                        app.worktree_selected
                            .min(app.project_data.worktrees.len().saturating_sub(1)),
                    );
                let name = app.data.files.get(app.selected).map(|f| f.name.clone());
                app.data = *snapshot;
                app.selected = name
                    .and_then(|name| app.data.files.iter().position(|f| f.name == name))
                    .unwrap_or(app.selected.min(app.data.files.len().saturating_sub(1)));
                app.refreshing = false;
                app.message = "Passive observation complete • d details".into();
                continue;
            }
            Wake::Terminal(input) => input?,
        };
        match input {
            Event::Key(k) if k.kind == KeyEventKind::Press => match k.code {
                KeyCode::Esc if app.details => app.details = false,
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::PageDown if app.details => {
                    app.detail_scroll = app.detail_scroll.saturating_add(5)
                }
                KeyCode::PageUp if app.details => {
                    app.detail_scroll = app.detail_scroll.saturating_sub(5)
                }
                _ if app.details && !matches!(k.code, KeyCode::Char('d')) => {}
                KeyCode::Down | KeyCode::Char('j') => app.move_selection(true),
                KeyCode::Up | KeyCode::Char('k') => app.move_selection(false),
                KeyCode::Char(c @ ('B' | 'C'))
                    if app.view == View::Mayhem && app.audit_inputs.is_some() =>
                {
                    let path = app.audit.as_ref().map(|a| {
                        if c == 'B' {
                            a.baseline.path.clone()
                        } else {
                            a.candidate.path.clone()
                        }
                    });
                    if let Some(path) = path {
                        app.launch(&path);
                    }
                }
                KeyCode::Char('[') if app.view == View::Mayhem && app.audit_inputs.is_some() => {
                    app.audit_finding_selected = app.audit_finding_selected.saturating_sub(1)
                }
                KeyCode::Char(']') if app.view == View::Mayhem && app.audit_inputs.is_some() => {
                    let n = app
                        .audit
                        .as_ref()
                        .map_or(0, |a| a.finding_count(app.audit_selected));
                    app.audit_finding_selected = app
                        .audit_finding_selected
                        .saturating_add(1)
                        .min(n.saturating_sub(1));
                }
                KeyCode::Tab => app.focus_projects = !app.focus_projects,
                KeyCode::Enter | KeyCode::Char('o') if app.view == View::Mops => {
                    app.mops_action(MopsAction::OpenPr)
                }
                KeyCode::Char('c') if app.view == View::Mops => {
                    app.mops_action(MopsAction::CopyResume)
                }
                KeyCode::Char('f') if app.view == View::Mops => {
                    app.mops_action(MopsAction::CopyFeedback)
                }
                KeyCode::Char('p') if app.view == View::Mops => {
                    app.mops_action(MopsAction::OpenWorktree)
                }
                KeyCode::Char('l') if app.view == View::Mops => {
                    app.mops_action(MopsAction::OpenRole)
                }
                KeyCode::Char('m') if app.view == View::Mops => {
                    app.mops_action(MopsAction::ViewMayhem)
                }
                KeyCode::Enter | KeyCode::Char('o') => app.open_current(false),
                KeyCode::Char('p') => app.open_current(true),
                KeyCode::Char(c @ '1'..='4') if app.view == View::Workshop => {
                    let i = c as usize - '1' as usize;
                    app.collapsed[i] = !app.collapsed[i];
                }
                KeyCode::Char('r') => {
                    app.refresh(control.clone(), sender.clone());
                }
                KeyCode::F(1) => app.view = View::Workshop,
                KeyCode::F(4) => app.view = View::Mayhem,
                KeyCode::F(5) => app.view = View::Settings,
                KeyCode::F(3) => app.view = View::Mops,
                KeyCode::F(2) => app.message = "Booth is not implemented".into(),
                KeyCode::Char('d') => {
                    app.details = !app.details;
                    app.detail_scroll = 0;
                }
                _ => {}
            },
            Event::Mouse(m)
                if !app.details && m.kind == MouseEventKind::Down(MouseButton::Left) =>
            {
                let p = ratatui::layout::Position::new(m.column, m.row);
                if let Some((_, action)) = app.mops_actions.iter().find(|(r, _)| r.contains(p)) {
                    app.mops_action(*action);
                } else if let Some((_, i)) = app.mops_rows.iter().find(|(r, _)| r.contains(p)) {
                    app.mops_selected = *i;
                } else if let Some((_, i)) = app.audit_rows.iter().find(|(r, _)| r.contains(p)) {
                    app.audit_selected = *i;
                    app.audit_finding_selected = 0;
                } else if let Some((_, view)) = app.tabs.iter().find(|(r, _)| r.contains(p)) {
                    app.view = *view;
                } else if let Some((_, path)) =
                    app.project_buttons.iter().find(|(r, _)| r.contains(p))
                {
                    let path = path.clone();
                    app.launch(&path);
                } else if let Some((_, i)) = app.group_rows.iter().find(|(r, _)| r.contains(p)) {
                    app.collapsed[*i] = !app.collapsed[*i];
                } else if let Some((_, i)) = app.worktree_rows.iter().find(|(r, _)| r.contains(p)) {
                    app.worktree_selected = *i;
                    app.focus_projects = false;
                } else if let Some((_, i)) =
                    app.project_file_rows.iter().find(|(r, _)| r.contains(p))
                {
                    app.project_file_selected = *i;
                    app.focus_projects = false;
                } else if let Some((_, i)) = app.project_rows.iter().find(|(r, _)| r.contains(p)) {
                    app.project_selected = *i;
                    app.project_file_selected = 0;
                    app.focus_projects = true;
                } else if let Some((_, i)) = app.buttons.iter().find(|(r, _)| r.contains(p)) {
                    app.open(*i);
                } else if let Some((_, i)) = app.rows.iter().find(|(r, _)| r.contains(p)) {
                    app.selected = *i;
                }
            }
            _ => {}
        }
    })();
    control.cancel();
    let cleanup = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result.and(cleanup)
}
