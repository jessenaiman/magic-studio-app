//! Backstage reads existing native analyzer reports. It never runs analyzers or edits projects.
//! Start here: Action → input_action → update → draw. Activity uses the existing CLI adapter.
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind},
    execute,
};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Paragraph, Row, Table},
};
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::mpsc,
};

// One action vocabulary and one dispatcher for every keyboard and mouse control.
#[derive(Clone, Copy)]
enum Action {
    Quit,
    Close,
    Select(usize),
    Move(bool),
    Pane(usize),
    Scroll(bool),
    Pan(bool),
    Details,
    Refresh,
}
const PANES: [&str; 5] = ["1 Output", "2 Command", "3 Run", "4 Files", "5 Activity"];
const BUTTONS: [(&str, Action); 3] = [
    ("Expand/close", Action::Details),
    ("Reread", Action::Refresh),
    ("Quit", Action::Quit),
];
type Hits = Vec<(Rect, Action)>;
enum Wake {
    Input(io::Result<Event>),
    Activity(Result<Value, String>),
    Quit,
}
struct App {
    path: PathBuf,
    report: Value,
    selected: usize,
    pane: usize,
    scroll: usize,
    column: usize,
    full: bool,
    activity: Value,
    reading: bool,
    message: String,
}

fn input_action(app: &App, event: Event, hits: &Hits) -> Option<Action> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Esc => Some(if app.full {
                Action::Close
            } else {
                Action::Quit
            }),
            KeyCode::Enter | KeyCode::Char('d') => Some(Action::Details),
            KeyCode::Down | KeyCode::Char('j') => Some(Action::Move(true)),
            KeyCode::Up | KeyCode::Char('k') => Some(Action::Move(false)),
            KeyCode::PageDown | KeyCode::Char(']') => Some(Action::Scroll(true)),
            KeyCode::PageUp | KeyCode::Char('[') => Some(Action::Scroll(false)),
            KeyCode::Right => Some(Action::Pan(true)),
            KeyCode::Left => Some(Action::Pan(false)),
            KeyCode::Char('r' | 'R') => Some(Action::Refresh),
            KeyCode::Tab => Some(Action::Pane((app.pane + 1) % PANES.len())),
            KeyCode::Char(c @ '1'..='5') => Some(Action::Pane(c as usize - '1' as usize)),
            KeyCode::F(n @ 1..=5) => Some(Action::Pane(n as usize - 1)),
            _ => None,
        },
        Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains((mouse.column, mouse.row).into()))
            .map(|(_, a)| *a),
        _ => None,
    }
}

fn update(
    app: &mut App,
    action: Action,
    control: &activity::Control,
    sender: &mpsc::Sender<Wake>,
) -> bool {
    let reset = matches!(
        action,
        Action::Select(_) | Action::Move(_) | Action::Pane(_)
    );
    match action {
        Action::Quit => return true,
        Action::Close => app.full = false,
        Action::Details => app.full = !app.full,
        Action::Select(i) => app.selected = i,
        Action::Move(down) => {
            app.selected = if down {
                (app.selected + 1).min(app.tools().len().saturating_sub(1))
            } else {
                app.selected.saturating_sub(1)
            }
        }
        Action::Pane(i) => app.pane = i,
        Action::Scroll(down) => {
            app.scroll = if down {
                (app.scroll + 10).min(app.content().lines().count().saturating_sub(1))
            } else {
                app.scroll.saturating_sub(10)
            }
        }
        Action::Pan(right) => {
            app.column = if right {
                app.column.saturating_add(8)
            } else {
                app.column.saturating_sub(8)
            }
        }
        Action::Refresh if app.pane == 4 => {
            if !app.reading {
                app.reading = true;
                let control = control.clone();
                let sender = sender.clone();
                std::thread::spawn(move || {
                    let _ = sender.send(Wake::Activity(activity::read(&control)));
                });
            }
        }
        Action::Refresh => match read_report(&app.path) {
            Ok(report) => {
                let tool = app.tool()["tool"].as_str().map(str::to_owned);
                app.report = report;
                app.selected = tool
                    .and_then(|id| app.tools().iter().position(|t| t["tool"] == id))
                    .unwrap_or(0);
                app.scroll = 0;
                app.column = 0;
                app.message = "Saved report reread; no analyzers run".into();
            }
            Err(error) => app.message = format!("Read failed; previous report retained: {error}"),
        },
    }
    if reset {
        app.scroll = 0;
        app.column = 0;
    }
    false
}

impl App {
    fn tools(&self) -> &[Value] {
        self.report["tools"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    fn tool(&self) -> &Value {
        self.tools().get(self.selected).unwrap_or(&Value::Null)
    }
    fn content(&self) -> String {
        let tool = self.tool();
        match self.pane {
            0 => [tool["error"].as_str(), tool["stdout"].as_str(), tool["stderr"].as_str()].into_iter().flatten()
                .filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
            1 => pretty(&json!({"tool":tool["tool"], "cwd":tool["cwd"], "scope":tool["scope"], "executable":tool["executable"], "args":tool["args"], "exit_code":tool["exit_code"], "signal":tool["signal"], "error":tool["error"]})),
            2 => { let mut run = self.report.clone(); if let Some(object) = run.as_object_mut() { object.remove("tools"); } pretty(&run) },
            3 => format!("Report: {}\nArtifacts: {}\n\n{}", self.path.display(), self.report["artifacts"].as_str().unwrap_or("Unknown"),
                tool.get("native_report").map(|v| v["contents"].as_str().map(str::to_owned).unwrap_or_else(|| pretty(v))).unwrap_or_else(|| "No additional native report file recorded for this tool. stdout/stderr files remain in the artifact directory.".into())),
            _ => pretty(&self.activity),
        }
    }
}
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}
fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .collect()
}
fn read_report(path: &Path) -> Result<Value, String> {
    if !std::fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(format!(
            "Expected a regular report file at {}",
            path.display()
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= 32 * 1024 * 1024)
    {
        return Err("Expected a regular report up to 32 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Report grew beyond 32 MiB".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value["schema"] != "magic.analyzers/1" || !value["tools"].is_array() {
        return Err(
            "Expected an existing magic.analyzers/1 report; run magic-cli audit first".into(),
        );
    }
    Ok(value)
}

// The renderer reads state and returns fresh Action targets; it never changes selection.
fn draw(frame: &mut ratatui::Frame, app: &App) -> Hits {
    let mut hits = Vec::new();
    let area = frame.area();
    let gold = Color::Rgb(245, 186, 56);
    let ink = Color::Rgb(24, 15, 27);
    let ivory = Color::Rgb(242, 222, 188);
    frame.render_widget(
        Block::default().style(Style::default().bg(ink).fg(ivory)),
        area,
    );
    if area.width < 45 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("Backstage • resize to 45×12 • q quits"),
            area,
        );
        return hits;
    }
    let bands = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(3),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(format!(
            "BACKSTAGE / NATIVE REPORTS\n{} • revision {}",
            clean(&app.path.display().to_string()),
            clean(app.report["revision"].as_str().unwrap_or("Unknown"))
        ))
        .style(Style::default().fg(gold)),
        bands[0],
    );
    let tabs = Layout::horizontal([Constraint::Percentage(20); 5]).split(bands[1]);
    for (i, title) in PANES.iter().enumerate() {
        frame.render_widget(
            Block::bordered()
                .title(*title)
                .border_style(Style::default().fg(if i == app.pane { gold } else { ivory })),
            tabs[i],
        );
        hits.push((tabs[i], Action::Pane(i)));
    }
    let body = if app.full {
        bands[2]
    } else {
        let columns = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(bands[2]);
        let capacity = columns[0].height.saturating_sub(3) as usize;
        let start = app.selected.saturating_sub(capacity / 2);
        let rows = app
            .tools()
            .iter()
            .enumerate()
            .skip(start)
            .take(capacity)
            .map(|(i, tool)| {
                let outcome = tool["exit_code"]
                    .as_i64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "Unknown".into());
                Row::new(vec![
                    clean(tool["tool"].as_str().unwrap_or("Unknown")),
                    outcome,
                ])
                .style(if i == app.selected {
                    Style::default().fg(gold)
                } else {
                    Style::default().fg(ivory)
                })
            });
        frame.render_widget(
            Table::new(
                rows,
                [Constraint::Percentage(65), Constraint::Percentage(35)],
            )
            .header(Row::new(["Tool", "Exit"]).style(Style::default().fg(gold)))
            .block(Block::bordered().title("Native outcomes")),
            columns[0],
        );
        for (row, i) in (start..app.tools().len().min(start + capacity)).enumerate() {
            hits.push((
                Rect::new(
                    columns[0].x + 1,
                    columns[0].y + 2 + row as u16,
                    columns[0].width.saturating_sub(2),
                    1,
                ),
                Action::Select(i),
            ));
        }
        columns[1]
    };
    let content = app.content();
    let visible = content
        .lines()
        .skip(app.scroll)
        .take(body.height.saturating_sub(2) as usize)
        .map(|s| clean(s).chars().skip(app.column).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    frame.render_widget(
        Paragraph::new(if content.is_empty() {
            "No output recorded.".into()
        } else {
            visible
        })
        .block(Block::bordered().title(format!(
            "{} • line {}",
            PANES[app.pane],
            app.scroll + 1
        ))),
        body,
    );
    let footer = Layout::vertical([Constraint::Length(1); 3]).split(bands[3]);
    frame.render_widget(
        Paragraph::new(clean(if app.reading {
            "Reading native Herdr activity…"
        } else {
            &app.message
        }))
        .style(Style::default().fg(gold)),
        footer[0],
    );
    frame.render_widget(
        Paragraph::new(
            "↑↓ tool · 1–5/Tab views · PgUp/PgDn/←→ scroll · d/Enter expand · r reread · q quit",
        ),
        footer[1],
    );
    let buttons = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ])
    .split(footer[2]);
    for (rect, (label, action)) in buttons.iter().zip(BUTTONS) {
        frame.render_widget(
            Paragraph::new(format!("[ {label} ]")).style(Style::default().fg(gold)),
            *rect,
        );
        hits.push((*rect, action));
    }
    hits
}

fn main() -> io::Result<()> {
    let mut path = std::env::current_dir()?
        .ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(Path::new("."))
        .join("target/analytics/analyzers.json");
    let mut plain = false;
    let mut tool = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "backstage [--audit-report PATH] [--audit-tool NAME] [--mayhem-report] [--color=always|never]\nReads existing reports. 1–5 select Output, Command, Run, Files, Activity. r in Activity queries the existing native adapter; other views only reread the report."
                );
                return Ok(());
            }
            "--audit-report" | "--audit-candidate" => {
                path = args
                    .next()
                    .ok_or_else(|| io::Error::other("Report path required"))?
                    .into()
            }
            "--audit-tool" => {
                tool = Some(
                    args.next()
                        .ok_or_else(|| io::Error::other("Tool name required"))?,
                )
            }
            "--mayhem-report" => plain = true,
            "--color=always" => crossterm::style::force_color_output(true),
            "--color=never" => crossterm::style::force_color_output(false),
            _ => {
                return Err(io::Error::other(format!(
                    "Unsupported option {arg}; use --help. Backstage now only displays native analyzer reports and scoped activity."
                )));
            }
        }
    }
    let report = read_report(&path).map_err(io::Error::other)?;
    let filtered = tool.is_some();
    let selected = match tool {
        Some(tool) => report["tools"]
            .as_array()
            .and_then(|tools| tools.iter().position(|t| t["tool"] == tool))
            .ok_or_else(|| io::Error::other("Requested tool is not in this report"))?,
        None => report["tools"]
            .as_array()
            .and_then(|tools| tools.iter().position(|t| t["exit_code"] != 0))
            .unwrap_or(0),
    };
    if plain {
        println!(
            "{}",
            pretty(if filtered {
                &report["tools"][selected]
            } else {
                &report
            })
        );
        return Ok(());
    }
    let mut app = App {
        path,
        report,
        selected,
        pane: 0,
        scroll: 0,
        column: 0,
        full: false,
        activity: json!({"available":false,"error":"Press r in Activity to read the native Herdr snapshot. No session has been queried."}),
        reading: false,
        message: "Existing report loaded; no analyzers run. Activity is explicit and read-only."
            .into(),
    };
    let control = activity::Control::default();
    let _owner = activity::Owner(control.clone());
    let (sender, receiver) = mpsc::channel();
    #[cfg(unix)]
    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGTERM,
    ])?;
    #[cfg(unix)]
    let signal_handle = signals.handle();
    #[cfg(unix)]
    let signal_thread = {
        let sender = sender.clone();
        std::thread::spawn(move || {
            if signals.forever().next().is_some() {
                let _ = sender.send(Wake::Quit);
            }
        })
    };
    let mut terminal = ratatui::init();
    let result = (|| -> io::Result<()> {
        execute!(io::stdout(), event::EnableMouseCapture)?;
        let input = sender.clone();
        std::thread::spawn(move || {
            loop {
                let event = event::read();
                let failed = event.is_err();
                if input.send(Wake::Input(event)).is_err() || failed {
                    break;
                }
            }
        });
        loop {
            let mut hits = Vec::new();
            terminal.draw(|frame| hits = draw(frame, &app))?;
            match receiver
                .recv()
                .map_err(|_| io::Error::other("Event channel closed"))?
            {
                Wake::Quit => break Ok(()),
                Wake::Input(event) => {
                    if input_action(&app, event?, &hits)
                        .is_some_and(|action| update(&mut app, action, &control, &sender))
                    {
                        break Ok(());
                    }
                }
                Wake::Activity(result) => {
                    app.reading = false;
                    app.activity =
                        result.unwrap_or_else(|error| json!({"available":false,"error":error}));
                    app.message =
                        "Native activity response read; unavailable fields remain unknown".into();
                }
            }
        }
    })();
    control.cancel();
    let cleanup = execute!(io::stdout(), event::DisableMouseCapture);
    ratatui::restore();
    #[cfg(unix)]
    {
        signal_handle.close();
        let _ = signal_thread.join();
    }
    result.and(cleanup)
}

// Bounded native activity call: existing process-group cancellation and output limits.
mod activity {
    use serde_json::Value;
    use std::{
        collections::HashSet,
        io::Read,
        process::{Command, Stdio},
        sync::{Arc, Mutex, mpsc},
        time::Duration,
    };
    const LIMIT: u64 = 1024 * 1024;
    const DEADLINE: Duration = Duration::from_secs(10);
    #[derive(Default)]
    struct State {
        closed: bool,
        groups: HashSet<i32>,
    }
    #[derive(Clone, Default)]
    pub struct Control(Arc<Mutex<State>>);
    pub struct Owner(pub Control);
    impl Drop for Owner {
        fn drop(&mut self) {
            self.0.cancel();
        }
    }
    impl Control {
        pub fn cancel(&self) {
            let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
            state.closed = true;
            for group in state.groups.drain() {
                stop(group);
            }
        }
    }
    struct Group {
        control: Control,
        id: i32,
    }
    impl Group {
        fn stop(&self) {
            let mut state = self.control.0.lock().unwrap_or_else(|e| e.into_inner());
            if state.groups.remove(&self.id) {
                stop(self.id);
            }
        }
    }
    impl Drop for Group {
        fn drop(&mut self) {
            self.stop();
        }
    }
    #[cfg(target_os = "linux")]
    fn stop(group: i32) {
        if group > 1 {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(group),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    #[cfg(not(target_os = "linux"))]
    fn stop(_: i32) {}

    #[cfg(not(target_os = "linux"))]
    pub fn read(_: &Control) -> Result<Value, String> {
        Err("Scoped native activity observation currently supports Linux only".into())
    }
    #[cfg(target_os = "linux")]
    pub fn read(control: &Control) -> Result<Value, String> {
        use std::os::unix::fs::FileTypeExt;
        if !std::fs::symlink_metadata("/dev/null").is_ok_and(|m| m.file_type().is_char_device()) {
            return Err("/dev/null isolation unavailable".into());
        }
        let sibling = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("magic-cli")))
            .filter(|p| p.is_file());
        let mut command = Command::new(sibling.unwrap_or_else(|| "magic-cli".into()));
        command.args([
            "--session-file",
            "/dev/null",
            "exec",
            "mayhem.evidence",
            "--params",
            "{\"activity_only\":true}",
        ]);
        let bytes = run(control, command)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid native activity JSON: {e}"))?;
        value
            .get("activity")
            .filter(|v| v.is_object())
            .cloned()
            .ok_or_else(|| "Missing native activity object".into())
    }
    #[cfg(target_os = "linux")]
    fn spawn_grouped(
        control: &Control,
        mut command: Command,
    ) -> Result<(std::process::Child, Group), String> {
        use std::os::unix::process::CommandExt;
        // Hold the lifecycle lock through spawn+registration: quit cannot miss a child.
        let mut state = control.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err("Provider cancelled".into());
        }
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "Command absent: native executable is not installed".into()
                } else {
                    format!("Provider launch error: {e}")
                }
            })?;
        let group = i32::try_from(child.id()).map_err(|_| "Provider process id out of range")?;
        state.groups.insert(group);
        drop(state);
        Ok((
            child,
            Group {
                control: control.clone(),
                id: group,
            },
        ))
    }
    #[cfg(target_os = "linux")]
    fn format_failure(status: std::process::ExitStatus, stderr: &[u8]) -> String {
        let detail = String::from_utf8_lossy(stderr).replace(char::is_control, " ");
        let missing_backend = detail.contains("magic-studio")
            && (detail.contains("not found")
                || detail.contains("Could not find")
                || detail.contains("not installed"));
        let category = if missing_backend {
            "Native backend absent"
        } else {
            "Provider error"
        };
        format!(
            "{category}: {status}: {}",
            detail.chars().take(1000).collect::<String>()
        )
    }
    #[cfg(target_os = "linux")]
    fn validate_output(
        status: std::process::ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        if stdout.len() > LIMIT as usize || stderr.len() > LIMIT as usize {
            return Err("Provider output exceeded 1 MiB bound".into());
        }
        if !status.success() {
            return Err(format_failure(status, &stderr));
        }
        Ok(stdout)
    }
    #[cfg(target_os = "linux")]
    fn run(control: &Control, command: Command) -> Result<Vec<u8>, String> {
        let (mut child, group_guard) = spawn_grouped(control, command)?;
        let group = group_guard.id;
        let stdout = child.stdout.take().ok_or("Provider stdout unavailable")?;
        let stderr = child.stderr.take().ok_or("Provider stderr unavailable")?;
        let out = read_pipe(stdout);
        let err = read_pipe(stderr);
        let (tx, rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (reaped_tx, reaped_rx) = mpsc::channel();
        std::thread::spawn(move || {
            use nix::sys::wait::{Id, WaitPidFlag, waitid};
            // Observe completion without reaping: the zombie leader reserves its PID/PGID
            // until group cleanup+unregistration, so a reused unrelated group cannot be hit.
            let observed = waitid(
                Id::Pid(nix::unistd::Pid::from_raw(group)),
                WaitPidFlag::WEXITED | WaitPidFlag::WNOWAIT,
            );
            let _ = tx.send(observed);
            let _ = release_rx.recv();
            let _ = reaped_tx.send(child.wait());
        });
        let observed = rx.recv_timeout(DEADLINE);
        group_guard.stop();
        let _ = release_tx.send(());
        let reaped = reaped_rx.recv_timeout(Duration::from_secs(2));
        let stdout = out
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "Provider stdout cleanup deadline exceeded")??;
        let stderr = err
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "Provider stderr cleanup deadline exceeded")??;
        observed
            .map_err(|_| "Provider timeout: isolated CLI/native process group stopped")?
            .map_err(|e| format!("Provider completion observation failed: {e}"))?;
        let status = reaped
            .map_err(|_| "Provider reap deadline exceeded")?
            .map_err(|e| format!("Provider wait error: {e}"))?;
        validate_output(status, stdout, stderr)
    }

    fn bounded(input: impl Read) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        input
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Provider pipe error: {e}"))?;
        Ok(bytes)
    }

    fn read_pipe(input: impl Read + Send + 'static) -> mpsc::Receiver<Result<Vec<u8>, String>> {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(bounded(input));
        });
        receiver
    }
}
