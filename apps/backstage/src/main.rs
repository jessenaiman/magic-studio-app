mod art;
mod model;
mod observe;
mod source;
mod system;
mod theme;
mod ui;

use crossterm::{
    event::{self, Event as TerminalEvent, KeyCode, KeyModifiers, MouseButton, MouseEventKind},
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
    Marker {
        before: Result<Value, String>,
        after: Result<Value, String>,
        read_ok: bool,
    },
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
    if matches!(request, ReadRequest::Catalog) {
        a.refresh_needed = true;
    }
    if a.busy {
        // Explicit reads and refresh demand are independent: neither can erase the other.
        if !matches!(request, ReadRequest::Catalog) {
            a.pending = Some(request);
        }
        return;
    }
    if matches!(request, ReadRequest::Catalog) {
        // Consume demand only when its read is scheduled. Changes during that read
        // set it again; completing an older read must never clear newer demand.
        a.refresh_needed = false;
    }
    if let Some(previous) = a.worker.take() {
        let _ = previous.join();
    }
    a.busy = true;
    let tx = tx.clone();
    let cancel = a.cancel.clone();
    a.worker = Some(thread::spawn(move || {
        let before = source::changes(&cancel);
        let event = match request {
            ReadRequest::Catalog => Event::Catalog(source::catalog(&cancel)),
            ReadRequest::Audit => Event::Audit(source::audit(&cancel)),
            ReadRequest::Project(path) => {
                let result = source::inspect(&path, &cancel);
                Event::Project(path, result)
            }
        };
        let read_ok = matches!(&event, Event::Catalog(Ok(_)) | Event::Project(_, Ok(_)));
        let after = source::changes(&cancel);
        for mut event in [
            Event::Marker {
                before,
                after,
                read_ok,
            },
            event,
        ] {
            while !cancel.load(Ordering::Acquire) {
                match tx.try_send(event) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Full(e)) => {
                        event = e;
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            }
        }
    }));
}
fn next_read(a: &mut App, tx: &SyncSender<Event>) {
    if let Some(explicit) = a.pending.take() {
        request(a, tx, explicit);
    } else if a.refresh_needed {
        request(a, tx, ReadRequest::Catalog);
    } else if a.project_refresh_needed {
        a.project_refresh_needed = false;
        inspect(a, tx);
    }
}
fn inspect(a: &mut App, tx: &SyncSender<Event>) {
    if let Some(path) = a.path() {
        request(a, tx, ReadRequest::Project(path));
    }
}
fn thumbnail(
    a: &App,
    picker: &Option<ratatui_image::picker::Picker>,
) -> Option<ratatui_image::protocol::StatefulProtocol> {
    let picker = picker.as_ref()?;
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
    Some(picker.new_resize_protocol(reader.decode().ok()?.thumbnail(1600, 900)))
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
                    "backstage — a production observer\n\nUsage: backstage [--theme daylight|midnight|omarchy|herdr|PATH]\n                 [--herdr-socket PATH] [--watch PATH] [--reduce-motion]\n                 [--snapshot overview|booth|mops|code|mayhem]\n\nA requests Magic's audit; R refreshes the catalog; / filters videos.\nClick tabs, use Left/Right, F1–F5 or Tab for views; Up/Down selects; PgUp/PgDn scroll audit; Q exits.\nT toggles the bundled Omarchy-format Backstage themes.\n--watch watches a supplied change-marker path; no marker writer is installed.\n--herdr-socket enables read-only agent event subscriptions; outside Herdr this is opt-in.\n--snapshot prints a deterministic text view without contacting any service.\nNo production controls or agent messaging are available."
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
        term.draw(|f| ui::draw(f, &a, &t, &m, None, &mut ui::HitMap::default()))?;
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
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        event::EnableMouseCapture,
        crossterm::cursor::Hide
    )?;
    // Capability negotiation owns stdin briefly: do this before the key reader.
    let picker = match ratatui_image::picker::Picker::from_query_stdio() {
        Ok(picker) if picker.protocol_type() != ratatui_image::picker::ProtocolType::Halfblocks => {
            a.graphics_status = format!("{:?} graphics · cached image", picker.protocol_type());
            Some(picker)
        }
        Ok(_) => {
            a.graphics_status =
                "No image graphics protocol detected; open Kitty and relaunch Backstage".into();
            None
        }
        Err(e) => {
            a.graphics_status = format!("Terminal graphics negotiation failed: {e}");
            None
        }
    };
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
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    request(&mut a, &tx, ReadRequest::Catalog);
    let mut thumb = None;
    let mut sample_at = Instant::now();
    let mut hits = ui::HitMap::default();
    let mut marker_watch: Option<(PathBuf, PathBuf, notify::RecommendedWatcher)> = None;
    loop {
        terminal.draw(|f| ui::draw(f, &a, &t, &m, thumb.as_mut(), &mut hits))?;
        let effect = !a.reduce_motion
            && (a.busy
                || (a.tab == 2 && a.agents.iter().any(|p| p["agent_status"] == "working"))
                || Instant::now() < a.effect_until);
        let event = if a.tab == 4 || effect {
            let timeout = if effect {
                Duration::from_millis(250)
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
            // Notifications may include replacement of the watched directory itself.
            // Rearm even when its pathname is unchanged; the old watch may refer
            // to an unlinked inode. Installation requires another bracketed read.
            marker_watch = None;
            request(&mut a, &tx, ReadRequest::Catalog);
        }
        let old_tab = a.tab;
        match event {
            Some(Event::Input(TerminalEvent::Key(k))) if k.kind != event::KeyEventKind::Release => {
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    break;
                }
                if a.help || a.detail {
                    match k.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            a.help = false;
                            a.detail = false;
                            a.detail_scroll = 0;
                        }
                        KeyCode::Down | KeyCode::PageDown => {
                            a.detail_scroll = a.detail_scroll.saturating_add(5)
                        }
                        KeyCode::Up | KeyCode::PageUp => {
                            a.detail_scroll = a.detail_scroll.saturating_sub(5)
                        }
                        _ => {}
                    }
                    continue;
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
                    thumb = thumbnail(&a, &picker);
                    continue;
                }
                match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab | KeyCode::Right => a.tab = (a.tab + 1) % 5,
                    KeyCode::BackTab | KeyCode::Left => a.tab = (a.tab + 4) % 5,
                    KeyCode::F(n @ 1..=5) => a.tab = usize::from(n - 1),
                    KeyCode::Char(c @ '1'..='5') => a.tab = (c as u8 - b'1') as usize,
                    KeyCode::Char('/') => a.searching = true,
                    KeyCode::Char('?') => {
                        a.help = true;
                        a.detail_scroll = 0;
                    }
                    KeyCode::Char('d' | 'D') => {
                        a.detail = true;
                        a.detail_scroll = 0;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if a.tab == 0 {
                            a.move_selection(1);
                            thumb = thumbnail(&a, &picker);
                        } else if a.tab == 2 {
                            a.move_agent(1);
                        } else {
                            a.scroll = a.scroll.saturating_add(1);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if a.tab == 0 {
                            a.move_selection(-1);
                            thumb = thumbnail(&a, &picker);
                        } else if a.tab == 2 {
                            a.move_agent(-1);
                        } else {
                            a.scroll = a.scroll.saturating_sub(1);
                        }
                    }
                    KeyCode::PageDown => a.scroll = a.scroll.saturating_add(10),
                    KeyCode::PageUp => a.scroll = a.scroll.saturating_sub(10),
                    KeyCode::Enter => {
                        if a.tab == 0 {
                            a.tab = 1;
                        } else {
                            a.detail = true;
                            a.detail_scroll = 0;
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
            }
            Some(Event::Input(TerminalEvent::Mouse(mouse))) => {
                let point = ratatui::layout::Position::new(mouse.column, mouse.row);
                if a.help || a.detail {
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            a.help = false;
                            a.detail = false;
                        }
                        MouseEventKind::ScrollDown => {
                            a.detail_scroll = a.detail_scroll.saturating_add(3)
                        }
                        MouseEventKind::ScrollUp => {
                            a.detail_scroll = a.detail_scroll.saturating_sub(3)
                        }
                        _ => {}
                    }
                    continue;
                }

                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some((_, index)) =
                            hits.tabs.iter().find(|(rect, _)| rect.contains(point))
                        {
                            a.tab = *index;
                            a.searching = false;
                        } else if a.tab == 2
                            && let Some((_, index)) =
                                hits.agents.iter().find(|(rect, _)| rect.contains(point))
                        {
                            a.agent_selected = *index;
                        } else if a.tab == 0
                            && let Some((_, index)) =
                                hits.projects.iter().find(|(rect, _)| rect.contains(point))
                        {
                            a.move_selection(*index as isize - a.selected as isize);
                            thumb = thumbnail(&a, &picker);
                        }
                    }
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                        let delta = if mouse.kind == MouseEventKind::ScrollDown {
                            1
                        } else {
                            -1
                        };
                        if a.tab == 0 {
                            a.move_selection(delta);
                            thumb = thumbnail(&a, &picker);
                        } else if a.tab == 2 {
                            a.move_agent(delta);
                        } else {
                            a.scroll = a.scroll.saturating_add_signed(delta as i16 * 3);
                        }
                    }
                    _ => {}
                }
            }
            Some(Event::Marker {
                before,
                after,
                read_ok,
            }) => {
                let before_token = before.as_ref().ok().and_then(observe::token);
                let after_token = after.as_ref().ok().and_then(observe::token);
                if before_token != after_token {
                    a.refresh_needed = true;
                }
                let mut installed_now = false;
                match after {
                    Ok(status) if status["ok"] == true => {
                        a.changes = status;
                        if let Some(path) = a.changes["path"]
                            .as_str()
                            .filter(|p| std::path::Path::new(p).is_absolute())
                            .map(PathBuf::from)
                        {
                            match observe::watch_directory(&path) {
                                Ok(directory) => {
                                    let replace =
                                        marker_watch.as_ref().is_none_or(|(old, dir, _)| {
                                            old != &path || dir != &directory
                                        });
                                    if replace {
                                        match observe::watch(
                                            std::slice::from_ref(&path),
                                            tx.clone(),
                                            dirty.clone(),
                                        ) {
                                            Ok(watcher) => {
                                                marker_watch = Some((
                                                    path.clone(),
                                                    directory.clone(),
                                                    watcher,
                                                ));
                                                installed_now = true;
                                                a.refresh_needed = true;
                                            }
                                            Err(e) => {
                                                marker_watch = None;
                                                a.notification_status =
                                                    format!("Change watcher unavailable: {e}");
                                            }
                                        }
                                    }
                                    if marker_watch.is_some() {
                                        a.notification_status = if path.parent()
                                            != Some(directory.as_path())
                                        {
                                            "Marker parent missing; watching ancestor for creation"
                                                .into()
                                        } else if after_token.is_some() {
                                            "Change token observed · limited, best-effort coverage"
                                                .into()
                                        } else {
                                            format!(
                                                "Change token unavailable: {}",
                                                model::text(&a.changes["reason"])
                                            )
                                        };
                                    }
                                }
                                Err(e) => {
                                    marker_watch = None;
                                    a.notification_status = e;
                                }
                            }
                        } else {
                            marker_watch = None;
                            a.notification_status =
                                "Provider returned no absolute change-marker path".into();
                        }
                    }
                    Ok(_) => {
                        a.changes = Value::Null;
                        marker_watch = None;
                        a.notification_status = "Change provider returned no usable status".into();
                    }
                    Err(e) => {
                        a.changes = Value::Null;
                        marker_watch = None;
                        a.notification_status = format!("Change provider unavailable: {e}");
                    }
                }
                // A fresh watch always requires a subsequent bracketed read; a failed
                // observation or changing/unavailable token must not be acknowledged.
                if read_ok
                    && !installed_now
                    && before_token == after_token
                    && marker_watch.is_some()
                {
                    a.last_token = after_token;
                }
            }
            Some(Event::Catalog(result)) => {
                a.busy = false;
                let selected = a.path();
                match result {
                    Ok(v) => {
                        // A catalog refresh does not refresh saved project evidence.
                        // Keep that demand separate from explicit user requests.
                        a.project_refresh_needed = a.tab == 1 || !a.project_report.is_null();
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
                        thumb = thumbnail(&a, &picker);
                    }
                    Err(e) => {
                        a.project_report = Value::Null;
                        a.project_error.clear();
                        a.catalog = Value::Null;
                        thumb = None;
                        a.status = e;
                    }
                }
                next_read(&mut a, &tx);
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
                next_read(&mut a, &tx);
            }
            Some(Event::Project(path, result)) => {
                a.busy = false;
                if a.path().as_deref() == Some(&path) {
                    match result {
                        Ok(v) => {
                            a.project_report = v;
                            a.project_error.clear();
                            a.status = "Saved-file observation received · live/unsaved state remains unknown".into();
                        }
                        Err(e) => {
                            a.project_report = Value::Null;
                            a.project_error = e;
                        }
                    }
                }
                next_read(&mut a, &tx);
            }
            Some(Event::Agents(result)) => {
                match result {
                    Ok(v) => {
                        let selected = a.agents.get(a.agent_selected).map(|p| p["pane_id"].clone());
                        a.agent_selected = v
                            .iter()
                            .position(|p| Some(&p["pane_id"]) == selected.as_ref())
                            .unwrap_or(0);
                        a.agents = v;
                        a.agents_observed_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs());
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
            Some(Event::Notice(e)) => {
                if e.starts_with("Watcher:") {
                    marker_watch = None;
                    a.notification_status = e.clone();
                }
                a.status = e;
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
    Ok(())
}
