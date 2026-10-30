//! One explicitly launched Backstage PTY per desktop process. No shell or network transport.
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

type Output = Arc<dyn Fn(Value) -> Result<(), String> + Send + Sync>;
type Completion = Arc<(Mutex<bool>, Condvar)>;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
const OUTPUT_DEADLINE: Duration = Duration::from_secs(5);
#[derive(Default)]
pub struct TerminalState(Mutex<Option<Session>>);
#[derive(Default)]
struct Flow {
    cancelled: bool,
    pending: Option<u64>,
}
type FlowControl = Arc<(Mutex<Flow>, Condvar)>;
fn cancel(flow: &FlowControl) {
    {
        let mut state = flow.0.lock().unwrap_or_else(|e| e.into_inner());
        state.cancelled = true;
        state.pending = None;
    }
    flow.1.notify_all();
}
struct Session {
    id: u64,
    master: Box<dyn MasterPty + Send>,
    input: mpsc::SyncSender<Vec<u8>>,
    stop: mpsc::SyncSender<()>,
    flow: FlowControl,
    finished: Arc<(Mutex<bool>, Condvar)>,
    owner: Option<std::thread::JoinHandle<()>>,
}
impl Session {
    fn finished(&self) -> bool {
        *self.finished.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn stop(&mut self) -> Result<(), String> {
        if !self.finished() {
            cancel(&self.flow);
            let _ = self.stop.try_send(());
            let (lock, wake) = &*self.finished;
            let (done, _) = wake
                .wait_timeout_while(
                    lock.lock().map_err(|e| e.to_string())?,
                    Duration::from_secs(5),
                    |done| !*done,
                )
                .map_err(|e| e.to_string())?;
            if !*done {
                return Err(
                    "Backstage shutdown is still pending; another session cannot start.".into(),
                );
            }
        }
        if let Some(owner) = self.owner.take() {
            owner
                .join()
                .map_err(|_| "Backstage owner thread failed".to_string())?;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn size(cols: u16, rows: u16) -> Result<PtySize, String> {
    if !(2..=500).contains(&cols) || !(2..=200).contains(&rows) {
        return Err("Terminal dimensions must be 2–500 columns and 2–200 rows.".into());
    }
    Ok(PtySize {
        cols,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    })
}
pub fn start(
    state: &TerminalState,
    cols: u16,
    rows: u16,
    output: impl Fn(Value) -> Result<(), String> + Send + Sync + 'static,
) -> Result<u64, String> {
    // Backstage's bounded provider-group shutdown is currently implemented on Linux.
    // In particular, do not imply ConPTY teardown support by offering a partial session.
    if !cfg!(target_os = "linux") {
        return Err("Embedded Backstage currently supports Linux only; use standalone Backstage on this platform.".into());
    }
    let output: Output = Arc::new(output);
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if slot.as_ref().is_some_and(|s| !s.finished()) {
        return Err("Backstage is already running. Stop the existing terminal first.".into());
    }
    if let Some(mut old) = slot.take() {
        old.stop()?;
    }
    // Only the companion beside this exact shell; never PATH, frontend paths/arguments,
    // cargo run, a shell string, or a generic command dispatcher.
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("backstage");
    if !executable.is_file() {
        return Err(format!(
            "Backstage companion missing: {}. Build with cargo build -p backstage -p magicstudio-shell and launch that shell.",
            executable.display()
        ));
    }
    let pair = native_pty_system()
        .openpty(size(cols, rows)?)
        .map_err(|e| e.to_string())?;
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut command = CommandBuilder::new(&executable);
    command.arg("--color=always");
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.cwd(std::env::current_dir().map_err(|e| e.to_string())?);
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("Could not launch Backstage: {e}"))?;
    drop(pair.slave);
    let id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
    let (stop, requests) = mpsc::sync_channel(1);
    // Four queued chunks plus the writer's one chunk: at most 40 KiB input retained.
    let (input, inputs) = mpsc::sync_channel::<Vec<u8>>(4);
    let flow: FlowControl = Arc::default();
    let writer_thread = spawn_writer(writer, inputs, flow.clone(), stop.clone());
    let finished: Completion = Arc::default();
    let reader_thread = spawn_reader(reader, id, output.clone(), flow.clone(), stop.clone());
    let owner = std::thread::spawn({
        let flow = flow.clone();
        let completion = finished.clone();
        move || {
            own_child(
                child,
                requests,
                id,
                output,
                flow,
                completion,
                (reader_thread, writer_thread),
            )
        }
    });
    *slot = Some(Session {
        id,
        master: pair.master,
        input,
        stop,
        flow,
        finished,
        owner: Some(owner),
    });
    Ok(id)
}

fn spawn_writer(
    mut writer: Box<dyn Write + Send>,
    inputs: mpsc::Receiver<Vec<u8>>,
    flow: FlowControl,
    stop: mpsc::SyncSender<()>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            if flow.0.lock().unwrap_or_else(|e| e.into_inner()).cancelled {
                break;
            }
            match inputs.recv_timeout(Duration::from_millis(50)) {
                Ok(bytes) => {
                    if writer
                        .write_all(&bytes)
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        let _ = stop.try_send(());
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    })
}
fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    id: u64,
    output: Output,
    flow: FlowControl,
    stop: mpsc::SyncSender<()>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut bytes = [0; 8192];
        let mut sequence = 0_u64;
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    // Continue draining during shutdown so a full output buffer cannot
                    // block Backstage before it handles HUP and cancels its providers.
                    if flow.0.lock().unwrap_or_else(|e| e.into_inner()).cancelled {
                        continue;
                    }
                    sequence += 1;
                    let (lock, wake) = &*flow;
                    lock.lock().unwrap_or_else(|e| e.into_inner()).pending = Some(sequence);
                    if output(json!({"kind":"data", "session":id, "sequence":sequence, "bytes": &bytes[..count]})).is_err() {
                        cancel(&flow);
                        let _ = stop.try_send(()); continue;
                    }
                    // Only one 8 KiB message may be in IPC/xterm at once. xterm's parse
                    // callback returns the credit; a lost renderer terminates this session.
                    let (credit, _) = wake
                        .wait_timeout_while(
                            lock.lock().unwrap_or_else(|e| e.into_inner()),
                            OUTPUT_DEADLINE,
                            |flow| flow.pending.is_some() && !flow.cancelled,
                        )
                        .unwrap_or_else(|e| e.into_inner());
                    if credit.cancelled {
                        continue;
                    }
                    if credit.pending.is_some() {
                        let _ = output(
                            json!({"kind":"error", "session":id, "message":"Backstage stopped: terminal output acknowledgement timed out."}),
                        );
                        drop(credit);
                        cancel(&flow);
                        let _ = stop.try_send(());
                    }
                }
                Err(error) => {
                    let _ = output(
                        json!({"kind":"error", "session":id, "message":format!("Backstage PTY read failed: {error}")}),
                    );
                    let _ = stop.try_send(());
                    break;
                }
            }
        }
    })
}
fn own_child(
    mut child: Box<dyn Child + Send + Sync>,
    requests: mpsc::Receiver<()>,
    id: u64,
    output: Output,
    flow: FlowControl,
    completion: Completion,
    workers: (std::thread::JoinHandle<()>, std::thread::JoinHandle<()>),
) {
    let (reader_thread, writer_thread) = workers;
    // Sample only our owned child's exit, never agents/external sessions.
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error);
            }
            Ok(None) => {}
        }
        match requests.recv_timeout(Duration::from_millis(50)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                // HUP lets Backstage cancel its isolated observer groups; portable-pty
                // escalates only this child if necessary, and we always reap it.
                let killed = child.kill();
                break match killed {
                    Ok(()) => child.wait(),
                    Err(error) => {
                        let _ = child.wait();
                        Err(error)
                    }
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    };
    cancel(&flow);
    let _ = writer_thread.join();
    let _ = reader_thread.join();
    *completion.0.lock().unwrap_or_else(|e| e.into_inner()) = true;
    completion.1.notify_all();
    let message = match result {
        Ok(status) => {
            json!({"kind":"exit", "session":id, "success":status.success(), "code":status.exit_code()})
        }
        Err(error) => {
            json!({"kind":"exit", "session":id, "success":false, "code":null, "error":format!("Backstage lifecycle failed: {error}")})
        }
    };
    let _ = output(message);
}

fn active(slot: &Option<Session>, id: u64) -> Result<&Session, String> {
    slot.as_ref()
        .filter(|s| s.id == id && !s.finished())
        .ok_or_else(|| "Backstage session ended or changed".into())
}
pub fn current(state: &TerminalState) -> Result<Option<u64>, String> {
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    Ok(slot.as_ref().filter(|s| !s.finished()).map(|s| s.id))
}
pub fn write(state: &TerminalState, id: u64, bytes: Vec<u8>) -> Result<(), String> {
    if bytes.len() > 8192 {
        return Err("Terminal input exceeds 8192 bytes.".into());
    }
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    active(&slot, id)?
        .input
        .try_send(bytes)
        .map_err(|_| "Backstage input is busy or closed; input was not queued.".into())
}
pub fn acknowledge(state: &TerminalState, id: u64, sequence: u64) -> Result<(), String> {
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    let session = active(&slot, id)?;
    let mut flow = session.flow.0.lock().map_err(|e| e.to_string())?;
    if flow.pending != Some(sequence) {
        return Err("Backstage output credit is stale".into());
    }
    flow.pending = None;
    session.flow.1.notify_all();
    Ok(())
}
pub fn resize(state: &TerminalState, id: u64, cols: u16, rows: u16) -> Result<(), String> {
    let size = size(cols, rows)?;
    let slot = state.0.lock().map_err(|e| e.to_string())?;
    active(&slot, id)?
        .master
        .resize(size)
        .map_err(|e| e.to_string())
}
pub fn stop(state: &TerminalState, id: Option<u64>) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(session) = slot.as_mut() {
        if id.is_some_and(|id| id != session.id) {
            return Err("Backstage session changed; stale Stop ignored.".into());
        }
        session.stop()?;
    }
    slot.take();
    Ok(())
}
