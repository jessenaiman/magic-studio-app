//! Native `eframe` desktop shell for Magic Studio (`apps/magic-gui`).
//!
//! Mounts [`magic_ui_egui::MagicApp`] over [`magic::Session`] and exposes an optional
//! loopback JSON-lines control channel (`magic --control 7985`) so `magic-cli` and `magic-mcp`
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

use clap::Parser;
use magic::Session;
use magic_ui_egui::{ControlRequest, MagicApp, ViewMode};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

#[derive(Parser)]
#[command(name = "magic", about = "Magic Studio desktop app (Craft egui edition)")]
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
    /// Loopback TCP port or `127.0.0.1:PORT` for the live `--control` JSON-lines bridge
    #[arg(long)]
    control: Option<String>,
    /// Optional bearer token required by the `--control` channel
    #[arg(long, env = "MAGIC_CONTROL_TOKEN")]
    control_token: Option<String>,
}

fn spawn_control_listener(
    spec: &str,
    expected_token: Option<String>,
    tx: Sender<ControlRequest>,
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
            std::thread::spawn(move || {
                let Ok(writer_stream) = incoming.try_clone() else {
                    return;
                };
                let mut writer = std::io::BufWriter::new(writer_stream);
                let reader = BufReader::new(incoming);
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
                    if let Some(exp) = &expected {
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
                    let out = match reply_rx.recv() {
                        Ok(Ok(result)) => json!({ "id": id, "ok": true, "result": result }),
                        Ok(Err(error)) => json!({ "id": id, "ok": false, "error": error }),
                        Err(_) => json!({ "id": id, "ok": false, "error": "window closed" }),
                    };
                    let _ = writeln!(writer, "{out}");
                    let _ = writer.flush();
                }
            });
        }
    });
    Ok(local)
}

fn main() -> eframe::Result {
    let cli = Cli::parse();

    let mut session = Session::user_session();
    if let Some(path) = cli.project.as_deref().or(cli.file.as_deref()) {
        let _ = session.execute("project.open", json!({ "path": path }));
    }

    let control_rx = if let Some(spec) = &cli.control {
        let (tx, rx) = channel();
        match spawn_control_listener(spec, cli.control_token.clone(), tx) {
            Ok(bound) => {
                eprintln!("magic: control listening on {bound}");
                Some(rx)
            }
            Err(err) => {
                eprintln!("magic: failed to bind --control {spec}: {err}");
                None
            }
        }
    } else {
        None
    };

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
            magic_ui_egui::theme::apply(&cc.egui_ctx);
            let mut app = MagicApp::new(session);
            if let Some(rx) = control_rx {
                app = app.with_control(rx);
            }
            if let Some(view) = initial_view {
                app.view = view;
            }
            Ok(Box::new(app))
        }),
    )
}
