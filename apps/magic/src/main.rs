//! Native `eframe` desktop shell for Magic Studio (`apps/magic-gui`).
//!
//! Mounts [`magic_ui_egui::MagicApp`] over [`magic::Session`] and exposes both an auto-bound
//! Unix domain socket (`$XDG_RUNTIME_DIR/magic/<pid>-1.sock`) and an optional loopback
//! JSON-lines control channel (`magic --control 7985`) so `magic-cli` and `magic-mcp`
//! can inspect or drive a live window.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unimplemented,
        clippy::todo,
        clippy::unreachable
    )
)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

use clap::Parser;
use magic::Session;
use magic_ui_egui::{ControlRequest, MagicApp, ViewMode};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

#[cfg(feature = "lyrics-webview")]
mod lyrics;

type WakeHandle = Arc<Mutex<Option<egui::Context>>>;

#[derive(Parser)]
#[command(
    name = "magic",
    about = "Magic Studio desktop app (Craft egui + Tauri edition)"
)]
struct Cli {
    /// Open this `.magic` project at launch
    #[arg(long)]
    project: Option<String>,
    /// Positional `.magic` project path (used by `.desktop` launcher `magic %f`)
    #[arg()]
    file: Option<String>,
    /// Start on a specific view (`projects`, `storyboard`, `lyrics`, `animation`, `studio`)
    #[arg(long)]
    view: Option<String>,
    /// Open the Tauri Kinetic Lyrics Designer window directly
    #[arg(long)]
    lyrics: bool,
    /// Loopback TCP port or `127.0.0.1:PORT` for the live `--control` JSON-lines bridge
    #[arg(long)]
    control: Option<String>,
    /// Optional bearer token required by the `--control` channel
    #[arg(long, env = "MAGIC_CONTROL_TOKEN")]
    control_token: Option<String>,
}

fn handle_control_stream<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    expected_token: Option<&str>,
    tx: &Sender<ControlRequest>,
    wake: &WakeHandle,
) {
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(
                    writer,
                    "{}",
                    json!({ "id": null, "ok": false, "error": format!("parse error: {e}") })
                );
                let _ = writer.flush();
                continue;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        if let Some(exp) = expected_token {
            let given = msg["token"].as_str().unwrap_or_default();
            let ok: bool = exp.as_bytes().ct_eq(given.as_bytes()).into();
            if !ok {
                let _ = writeln!(
                    writer,
                    "{}",
                    json!({ "id": id, "ok": false, "error": "unauthorized control token" })
                );
                let _ = writer.flush();
                continue;
            }
        }
        let method = msg["method"]
            .as_str()
            .or_else(|| msg["command"].as_str())
            .unwrap_or_default()
            .to_owned();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let (reply_tx, reply_rx) = channel();
        if tx
            .send(ControlRequest {
                method,
                params,
                reply: reply_tx,
            })
            .is_err()
        {
            break;
        }
        if let Ok(guard) = wake.lock()
            && let Some(ctx) = guard.as_ref()
        {
            ctx.request_repaint();
        }
        let out = match reply_rx.recv() {
            Ok(Ok(result)) => json!({ "id": id, "ok": true, "result": result }),
            Ok(Err(error)) => json!({ "id": id, "ok": false, "error": error }),
            Err(_) => json!({ "id": id, "ok": false, "error": "window closed" }),
        };
        let _ = writeln!(writer, "{out}");
        let _ = writer.flush();
    }
}

fn spawn_control_listener(
    spec: &str,
    expected_token: Option<String>,
    tx: Sender<ControlRequest>,
    wake: WakeHandle,
) -> Result<String, String> {
    let addr = if spec.contains(':') {
        spec.to_owned()
    } else {
        format!("127.0.0.1:{spec}")
    };
    let listener = TcpListener::bind(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
    let local = listener
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| addr.clone());
    std::thread::spawn(move || {
        for incoming in listener.incoming().flatten() {
            let tx = tx.clone();
            let expected = expected_token.clone();
            let wake = Arc::clone(&wake);
            std::thread::spawn(move || {
                let Ok(writer_stream) = incoming.try_clone() else {
                    return;
                };
                handle_control_stream(
                    BufReader::new(incoming),
                    std::io::BufWriter::new(writer_stream),
                    expected.as_deref(),
                    &tx,
                    &wake,
                );
            });
        }
    });
    Ok(local)
}

#[cfg(unix)]
fn spawn_unix_control_socket(tx: Sender<ControlRequest>, wake: WakeHandle) {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("magic");
    let _ = std::fs::create_dir_all(&dir);
    let sock_path = dir.join(format!("{}-1.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock_path);
    let Ok(listener) = std::os::unix::net::UnixListener::bind(&sock_path) else {
        return;
    };
    std::thread::spawn(move || {
        for incoming in listener.incoming().flatten() {
            let tx = tx.clone();
            let wake = Arc::clone(&wake);
            std::thread::spawn(move || {
                let Ok(writer_stream) = incoming.try_clone() else {
                    return;
                };
                handle_control_stream(
                    BufReader::new(incoming),
                    std::io::BufWriter::new(writer_stream),
                    None,
                    &tx,
                    &wake,
                );
            });
        }
    });
}

fn main() -> eframe::Result {
    let cli = Cli::parse();

    let mut session = Session::user_session();
    if let Some(path) = cli.project.as_deref().or(cli.file.as_deref()) {
        let _ = session.execute("project.open", json!({ "path": path }));
    }

    #[cfg(feature = "lyrics-webview")]
    if cli.lyrics || cli.view.as_deref() == Some("lyrics") {
        return lyrics::run(session).map_err(|error| eframe::Error::AppCreation(Box::new(error)));
    }

    let (tx, control_rx) = channel();
    let wake: WakeHandle = Arc::new(Mutex::new(None));

    #[cfg(unix)]
    spawn_unix_control_socket(tx.clone(), Arc::clone(&wake));

    if let Some(spec) = &cli.control {
        match spawn_control_listener(spec, cli.control_token.clone(), tx, Arc::clone(&wake)) {
            Ok(bound) => eprintln!("magic: control listening on {bound}"),
            Err(err) => eprintln!("magic: failed to bind --control {spec}: {err}"),
        }
    }

    let initial_view = cli.view.as_deref().and_then(ViewMode::from_name);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Magic Studio")
            .with_app_id("ai.storyteller.magic")
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Magic Studio",
        native_options,
        Box::new(move |cc| {
            if let Ok(mut guard) = wake.lock() {
                *guard = Some(cc.egui_ctx.clone());
            }
            magic_ui_egui::theme::apply(&cc.egui_ctx);
            let mut app = MagicApp::new(session).with_control(control_rx);
            if let Some(view) = initial_view {
                app.view = view;
            }
            Ok(Box::new(app))
        }),
    )
}
