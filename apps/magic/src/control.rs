//! Bounded JSON-lines transport, independent of the desktop renderer.
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use subtle::ConstantTimeEq;

const MAX_LINE: usize = 4 * 1024 * 1024;
const MAX_CONNECTIONS: usize = 16;
pub type Dispatch = Arc<dyn Fn(&str, Value) -> Result<Value, String> + Send + Sync>;

pub fn start(spec: &str, token: &str, dispatch: Dispatch) -> Result<SocketAddr, String> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("MAGIC_CONTROL_TOKEN must contain 64 hexadecimal characters".into());
    }
    let addr: SocketAddr =
        if spec.contains(':') { spec.to_owned() } else { format!("127.0.0.1:{spec}") }.parse().map_err(|_| "--control requires a port or loopback IP:port")?;
    if !addr.ip().is_loopback() {
        return Err("--control must bind to loopback".into());
    }
    let listener = TcpListener::bind(addr).map_err(|e| format!("control bind: {e}"))?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?;
    let token = token.to_owned();
    std::thread::Builder::new()
        .name("magic-control".into())
        .spawn(move || {
            let active = Arc::new(AtomicUsize::new(0));
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));
                if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                    active.fetch_sub(1, Ordering::SeqCst);
                    let _ = writeln!(stream, "{}", json!({"ok":false,"error":"control connection limit reached"}));
                    continue;
                }
                let active_worker = active.clone();
                let token = token.clone();
                let dispatch = dispatch.clone();
                if std::thread::Builder::new()
                    .name("magic-control-client".into())
                    .spawn(move || {
                        if let Ok(reader) = stream.try_clone() {
                            serve_lines(BufReader::new(reader), &mut stream, &token, |m, p| dispatch(m, p));
                        }
                        active_worker.fetch_sub(1, Ordering::SeqCst);
                    })
                    .is_err()
                {
                    active.fetch_sub(1, Ordering::SeqCst);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(bound)
}

fn serve_lines<R: BufRead, W: Write>(mut reader: R, mut writer: W, token: &str, dispatch: impl Fn(&str, Value) -> Result<Value, String>) {
    loop {
        let mut line = Vec::new();
        let read = reader.by_ref().take((MAX_LINE + 1) as u64).read_until(b'\n', &mut line);
        match read {
            Ok(0) | Err(_) => break,
            _ => {}
        }
        let parsed = if line.len() > MAX_LINE || !line.ends_with(b"\n") {
            Err("oversized or incomplete control request")
        } else if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        } else {
            serde_json::from_slice::<Value>(&line).map_err(|_| "invalid JSON request").and_then(|v| {
                if !v.is_object() || v["method"].as_str().is_none() {
                    return Err("request requires a method");
                }
                if !bool::from(v["token"].as_str().unwrap_or_default().as_bytes().ct_eq(token.as_bytes())) {
                    return Err("unauthorized control token");
                }
                if v.get("params").is_some_and(|p| !p.is_object()) {
                    return Err("params must be an object");
                }
                Ok(v)
            })
        };
        let request = match parsed {
            Ok(v) => v,
            Err(error) => {
                let _ = writeln!(writer, "{}", json!({"id":null,"ok":false,"error":error}));
                let _ = writer.flush();
                break;
            }
        };
        let method = request["method"].as_str().unwrap_or_default();
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let reply = match dispatch(method, params) {
            Ok(result) => json!({"id":request["id"],"ok":true,"result":result}),
            Err(error) => json!({"id":request["id"],"ok":false,"error":error}),
        };
        if writeln!(writer, "{reply}").and_then(|_| writer.flush()).is_err() {
            break;
        }
    }
}
