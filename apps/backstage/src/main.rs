//! Settings-only observer. No engine linkage, agent connections, sockets or model clients.
mod data;
mod provider;
mod ui;
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
    Terminal(io::Result<Event>),
    Observed(Box<data::Snapshot>),
}
struct App {
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
    fn new() -> Self {
        Self {
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
        self.refreshing = true;
        std::thread::spawn(move || {
            let mut snapshot = data::Snapshot::read();
            snapshot.apply(provider::read(&control));
            let _ = sender.send(Wake::Observed(Box::new(snapshot)));
        });
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
        .arg(path)
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
    let mut app = App::new();
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
    app.refresh(control.clone(), sender.clone());
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
            Wake::Observed(snapshot) => {
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
                KeyCode::Down | KeyCode::Char('j') => {
                    app.selected = (app.selected + 1).min(app.data.files.len().saturating_sub(1))
                }
                KeyCode::Up | KeyCode::Char('k') => app.selected = app.selected.saturating_sub(1),
                KeyCode::Enter | KeyCode::Char('o') => app.open(app.selected),
                KeyCode::Char('p') => app.open(usize::MAX),
                KeyCode::Char('r') => {
                    app.refresh(control.clone(), sender.clone());
                }
                KeyCode::F(1..=4) => {
                    app.message = "Not implemented: Settings is the only available view".into()
                }
                KeyCode::F(5) => app.message.clear(),
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
                if let Some((_, i)) = app.buttons.iter().find(|(r, _)| r.contains(p)) {
                    app.open(*i);
                } else if let Some((_, i)) = app.rows.iter().find(|(r, _)| r.contains(p)) {
                    app.selected = *i;
                } else if m.row >= 2 && m.row < 5 {
                    app.message = "Only Settings is implemented; other tabs are disabled".into();
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
