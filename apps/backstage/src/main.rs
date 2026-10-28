mod model;
mod observe;
mod source;
mod system;
mod theme;
mod ui;

use crossterm::{
    event::{self, Event as TerminalEvent, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use model::App;
use ratatui::{
    Terminal,
    backend::{CrosstermBackend, TestBackend},
};
use serde_json::Value;
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use theme::Theme;

#[derive(Clone)]
pub enum ReadRequest {
    Catalog,
    Audit,
    Project(String),
}
pub enum Event {
    Input(TerminalEvent),
    Changed,
    Notice(String),
    Catalog(Result<Value, String>),
    Audit(Result<Value, String>),
    Agents(Result<Vec<Value>, String>),
    Project(String, Result<Value, String>),
}
struct Screen;
impl Drop for Screen {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            event::DisableMouseCapture,
            crossterm::cursor::Show
        );
    }
}

fn request(a: &mut App, tx: &SyncSender<Event>, request: ReadRequest) {
    if a.busy {
        if a.pending.is_none() || !matches!(request, ReadRequest::Catalog) {
            a.pending = Some(request);
        }
        return;
    }
    if let Some(previous) = a.worker.take() {
        let _ = previous.join();
    }
    a.busy = true;
    let tx = tx.clone();
    let cancel = a.cancel.clone();
    a.worker = Some(thread::spawn(move || {
        let event = match request {
            ReadRequest::Catalog => Event::Catalog(source::catalog(&cancel)),
            ReadRequest::Audit => Event::Audit(source::audit(&cancel)),
            ReadRequest::Project(path) => {
                let result = source::inspect(&path, &cancel);
                Event::Project(path, result)
            }
        };
        let mut event = event;
        while !cancel.load(Ordering::Acquire) {
            match tx.try_send(event) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(e)) => {
                    event = e;
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    }));
}
fn inspect(a: &mut App, tx: &SyncSender<Event>) {
    if let Some(path) = a.path() {
        request(a, tx, ReadRequest::Project(path));
    }
}
fn thumbnail(a: &App) -> Option<image::RgbImage> {
    let p = a.project()?;
    let path = p["thumb"].as_str()?;
    if std::fs::metadata(path).ok()?.len() > 20 * 1024 * 1024 {
        return None;
    }
    let mut reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Some(reader.decode().ok()?.thumbnail(160, 100).to_rgb8())
}
fn main() {
    if let Err(e) = start() {
        eprintln!("backstage: {e}");
        std::process::exit(1);
    }
}
fn start() -> Result<(), Box<dyn std::error::Error>> {
    let mut name = "daylight".to_owned();
    let mut snapshot = None;
    let mut watch = None;
    let mut herdr = None;
    let mut motion = true;
    let mut print_theme = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "backstage — a production observer\n\nUsage: backstage [--theme daylight|midnight|omarchy|herdr|PATH]\n                 [--herdr-socket PATH] [--watch PATH] [--reduce-motion]\n                 [--snapshot overview|booth|mops|code|mayhem]\n\nA requests Magic's audit; R refreshes the catalog; / filters videos.\n1–5 or Tab selects views; arrows select; PgUp/PgDn scroll audit; Q exits.\nT toggles the bundled Omarchy-format Backstage themes.\n--watch watches a supplied change-marker path; no marker writer is installed.\n--herdr-socket enables read-only agent event subscriptions; outside Herdr this is opt-in.\n--snapshot prints a deterministic text view without contacting any service.\nNo production controls or agent messaging are available."
                );
                return Ok(());
            }
            "--theme" => name = args.next().ok_or("--theme needs a value")?,
            "--snapshot" => snapshot = Some(args.next().ok_or("--snapshot needs a view")?),
            "--watch" => watch = Some(PathBuf::from(args.next().ok_or("--watch needs a path")?)),
            "--herdr-socket" => {
                herdr = Some(PathBuf::from(
                    args.next().ok_or("--herdr-socket needs a path")?,
                ))
            }
            "--reduce-motion" => motion = false,
            "--print-herdr-theme" => print_theme = true,
            _ => return Err(format!("Unknown option: {arg}; use --help").into()),
        }
    }
    let mut t = Theme::load(&name)?;
    if print_theme {
        print!("{}", t.herdr_config());
        return Ok(());
    }
    let mut a = App::default();
    if !motion {
        a.reduce_motion = true;
    }
    let mut m = system::Metrics::default();
    if let Some(view) = snapshot {
        a.tab = ui::TABS
            .iter()
            .position(|s| s.eq_ignore_ascii_case(&view))
            .ok_or("Unknown view")?;
        a.status = "Offline layout inspection · no service queried".into();
        let mut term = Terminal::new(TestBackend::new(120, 38))?;
        term.draw(|f| ui::draw(f, &a, &t, &m, None))?;
        let b = term.backend().buffer();
        for y in 0..38 {
            let line = (0..120).map(|x| b[(x, y)].symbol()).collect::<String>();
            println!("{}", line.trim_end());
        }
        return Ok(());
    }
    if !io::stdout().is_terminal() {
        return Err(
            "An interactive terminal is required; use --snapshot for offline inspection".into(),
        );
    }
    let (tx, rx) = mpsc::sync_channel(64);
    let keys = tx.clone();
    thread::spawn(move || {
        loop {
            match event::read() {
                Ok(e) => {
                    if keys.send(Event::Input(e)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = keys.send(Event::Notice(e.to_string()));
                    break;
                }
            }
        }
    });
    let mut paths = vec![
        theme::config_home().join("herdr/config.toml"),
        theme::state_home().join("omarchy/current/theme/colors.toml"),
    ];
    if let Some(p) = watch {
        if !p.parent().is_some_and(|p| p.is_dir()) {
            return Err("The marker parent directory must exist".into());
        }
        paths.push(p);
    }
    paths.push(theme::state_home().join("omarchy/current/theme"));
    let dirty = Arc::new(AtomicBool::new(false));
    let _watcher = observe::watch(&paths, tx.clone(), dirty.clone())
        .map_err(|e| format!("Cannot watch changes: {e}"))?;
    if herdr.is_none() && std::env::var("HERDR_ENV").as_deref() == Ok("1") {
        herdr = Some(
            std::env::var_os("HERDR_SOCKET_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| theme::config_home().join("herdr/herdr.sock")),
        );
    }
    if let Some(path) = herdr {
        observe::agents(path, tx.clone());
    }
    enable_raw_mode()?;
    let _screen = Screen;
    execute!(io::stdout(), EnterAlternateScreen, crossterm::cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    request(&mut a, &tx, ReadRequest::Catalog);
    let mut thumb = None;
    let mut sample_at = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(f, &a, &t, &m, thumb.as_ref()))?;
        let effect = a.tab == 2 && !a.reduce_motion && Instant::now() < a.effect_until;
        let event = if a.tab == 4 || effect {
            let timeout = if effect {
                Duration::from_millis(180)
            } else {
                sample_at.saturating_duration_since(Instant::now())
            };
            match rx.recv_timeout(timeout) {
                Ok(e) => Some(e),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(_) => break,
            }
        } else {
            Some(rx.recv()?)
        };
        if a.tab == 4 && Instant::now() >= sample_at {
            m.sample();
            sample_at = Instant::now() + Duration::from_secs(1);
        }
        if dirty.swap(false, Ordering::AcqRel) {
            request(&mut a, &tx, ReadRequest::Catalog);
        }
        match event {
            Some(Event::Input(TerminalEvent::Key(k))) if k.kind != event::KeyEventKind::Release => {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                if a.searching {
                    match k.code {
                        KeyCode::Esc => {
                            a.searching = false;
                            a.query.clear();
                        }
                        KeyCode::Enter => a.searching = false,
                        KeyCode::Backspace => {
                            a.query.pop();
                        }
                        KeyCode::Char(c) => a.query.push(c),
                        _ => {}
                    }
                    a.project_report = Value::Null;
                    a.project_error.clear();
                    a.selected = 0;
                    thumb = thumbnail(&a);
                    continue;
                }
                let old_tab = a.tab;
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => a.tab = (a.tab + 1) % 5,
                    KeyCode::BackTab => a.tab = (a.tab + 4) % 5,
                    KeyCode::Char(c @ '1'..='5') => a.tab = (c as u8 - b'1') as usize,
                    KeyCode::Char('/') => a.searching = true,
                    KeyCode::Down | KeyCode::Char('j') => {
                        if a.tab == 0 {
                            a.move_selection(1);
                            thumb = thumbnail(&a);
                        } else {
                            a.scroll = a.scroll.saturating_add(1);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if a.tab == 0 {
                            a.move_selection(-1);
                            thumb = thumbnail(&a);
                        } else {
                            a.scroll = a.scroll.saturating_sub(1);
                        }
                    }
                    KeyCode::PageDown => a.scroll = a.scroll.saturating_add(10),
                    KeyCode::PageUp => a.scroll = a.scroll.saturating_sub(10),
                    KeyCode::Enter => {
                        if a.tab == 0 {
                            a.tab = 1;
                        }
                    }
                    KeyCode::Char('a' | 'A') => request(&mut a, &tx, ReadRequest::Audit),
                    KeyCode::Char('r' | 'R') => {
                        if a.tab == 1 {
                            inspect(&mut a, &tx);
                        } else {
                            request(&mut a, &tx, ReadRequest::Catalog);
                        }
                    }
                    KeyCode::Char('t' | 'T') => {
                        name = if name == "daylight" {
                            "midnight"
                        } else {
                            "daylight"
                        }
                        .into();
                        t = Theme::load(&name)?;
                    }
                    _ => {}
                }
                if a.tab != old_tab {
                    a.scroll = 0;
                    if a.tab == 1 {
                        inspect(&mut a, &tx);
                    }
                    if a.tab == 4 {
                        sample_at = Instant::now();
                    } else if old_tab == 4 {
                        m.pause();
                    }
                }
            }
            Some(Event::Catalog(result)) => {
                a.busy = false;
                let selected = a.path();
                match result {
                    Ok(v) => {
                        a.catalog = v;
                        a.selected = a
                            .projects()
                            .iter()
                            .position(|p| p["path"].as_str() == selected.as_deref())
                            .unwrap_or(0);
                        a.status="Magic catalog observed · A retrieves audit · production continues in Magic".into();
                        if a.path() != selected {
                            a.project_report = Value::Null;
                            a.project_error.clear();
                        }
                        thumb = thumbnail(&a);
                    }
                    Err(e) => {
                        a.project_report = Value::Null;
                        a.project_error.clear();
                        a.catalog = Value::Null;
                        thumb = None;
                        a.status = e;
                    }
                }
                if let Some(audit) = a.pending.take() {
                    request(&mut a, &tx, audit);
                }
            }
            Some(Event::Audit(result)) => {
                a.busy = false;
                match result {
                    Ok(v) => {
                        a.report = v;

                        a.observed = Some(Instant::now());
                        a.status = "Magic audit received · displayed as reported".into();
                        a.tab = 3;
                        m.pause();
                    }
                    Err(e) => {
                        a.report = Value::Null;
                        a.observed = None;
                        a.status = format!("Audit unavailable: {e}");
                    }
                }
                if let Some(audit) = a.pending.take() {
                    request(&mut a, &tx, audit);
                }
            }
            Some(Event::Project(path, result)) => {
                a.busy = false;
                if a.path().as_deref() == Some(&path) {
                    match result {
                        Ok(v) => {
                            a.project_report = v;
                            a.project_error.clear();
                            a.status = "Disk observation received; Studio was not switched".into();
                        }
                        Err(e) => {
                            a.project_report = Value::Null;
                            a.project_error = e;
                        }
                    }
                }
                if let Some(audit) = a.pending.take() {
                    request(&mut a, &tx, audit);
                }
            }
            Some(Event::Agents(result)) => {
                match result {
                    Ok(v) => {
                        a.agents = v;
                        a.agent_status="Herdr event subscription · observed host only · title is not task proof".into();
                        a.effect_until = Instant::now() + Duration::from_millis(900);
                    }
                    Err(e) => {
                        a.agents.clear();
                        a.agent_status = e;
                    }
                }
            }
            Some(Event::Changed) => {
                if matches!(name.as_str(), "omarchy" | "herdr") {
                    match Theme::load(&name) {
                        Ok(v) => t = v,
                        Err(e) => a.status = format!("Theme: {e}"),
                    }
                }
            }
            Some(Event::Notice(e)) => a.status = e,
            _ => {}
        }
    }
    Ok(())
}
