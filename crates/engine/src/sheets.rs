//! Reviewable, project-owned asset sheets. A request draws complete sheets only.
//! Prompts and results stay in source-backed JSON; extraction and rigging are separate actions.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result, Session};

pub(crate) mod layout;
pub(crate) mod playback;
use layout::SheetLayout;

const KINDS: [&str; 3] = ["turnaround", "skeleton", "poses"];

/// Pixels within this distance of pure white still count as the flat `#ffffff` background.
const WHITE_TOLERANCE: u8 = 12;

/// The last `chars` of a stream, with the tail marker that makes a truncation visible.
fn tail(text: &str, chars: usize) -> String {
    let trimmed = text.trim();
    let out: String = trimmed
        .chars()
        .rev()
        .take(chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if trimmed.chars().count() > chars {
        format!("…{out}")
    } else {
        out
    }
}

/// What one agent invocation produced.
pub(crate) struct AgentRun {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) stopped_after_image: bool,
}

impl AgentRun {
    pub(crate) fn succeeded(&self) -> bool {
        !self.timed_out && self.code == Some(0)
    }

    /// The part of the agent's own report worth showing when it failed.
    pub(crate) fn failure_detail(&self) -> String {
        if !self.stderr.trim().is_empty() {
            tail(&self.stderr, 800)
        } else {
            tail(&self.stdout, 800)
        }
    }
}

/// Run the agent with a hard timeout, capturing both streams. A long run must not freeze the
/// app, so on timeout the agent is stopped and whatever it wrote is still checked.
pub(crate) fn run_agent(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
) -> Result<AgentRun> {
    run_agent_inner(program, args, cwd, timeout_s, false)
}

/// Image bytes are already staged when the native success receipt arrives. Stop this one-shot
/// agent before it can retry generation while trying to reconstruct/copy binary tool output.
pub(crate) fn run_image_agent(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
) -> Result<AgentRun> {
    run_agent_inner(program, args, cwd, timeout_s, true)
}

fn spawn_stdout_reader(pipe: Option<std::process::ChildStdout>) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = tx.send(buf);
        });
    }
    rx
}

fn spawn_stderr_reader(
    pipe: Option<std::process::ChildStderr>,
    stop_on_image: bool,
) -> (std::sync::mpsc::Receiver<Vec<u8>>, std::sync::mpsc::Receiver<()>) {
    let (tx_err, rx_err) = std::sync::mpsc::channel::<Vec<u8>>();
    let (image_tx, image_rx) = std::sync::mpsc::channel::<()>();
    if let Some(mut pipe) = pipe {
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            let mut reader = BufReader::new(&mut pipe);
            let mut buf = Vec::new();
            let mut line = Vec::new();
            let mut signalled = false;
            loop {
                line.clear();
                match reader.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                buf.extend_from_slice(&line);
                if stop_on_image
                    && !signalled
                    && crate::image_generation::native_image_success(&String::from_utf8_lossy(
                        &line,
                    ))
                {
                    signalled = true;
                    let _ = image_tx.send(());
                }
            }
            let _ = tx_err.send(buf);
        });
    }
    (rx_err, image_rx)
}

fn wait_for_agent_child(
    child: &mut std::process::Child,
    deadline: Instant,
    stop_on_image: bool,
    image_rx: &std::sync::mpsc::Receiver<()>,
) -> (Option<i32>, bool, bool) {
    let mut timed_out = false;
    let mut stopped_after_image = false;
    let code = loop {
        if stop_on_image && image_rx.try_recv().is_ok() {
            stopped_after_image = true;
            let _ = child.kill();
            break child.wait().ok().and_then(|status| status.code());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child.wait().ok().and_then(|status| status.code());
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    (code, timed_out, stopped_after_image)
}

fn run_agent_inner(
    program: &str,
    args: &[String],
    cwd: &Path,
    timeout_s: u64,
    stop_on_image: bool,
) -> Result<AgentRun> {
    let mut command = std::process::Command::new(program);
    // `codex exec` defaults to error-only stderr. The native image importer needs
    // its INFO tool receipt to correlate staged bytes; never rely on the parent's log level.
    if stop_on_image {
        command.env("RUST_LOG", "error,codex_otel=info");
    }
    let mut child = command
        .current_dir(cwd)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(program.to_string(), e.to_string()))?;
    let rx_out = spawn_stdout_reader(child.stdout.take());
    let (rx_err, image_rx) = spawn_stderr_reader(child.stderr.take(), stop_on_image);
    let deadline = Instant::now() + Duration::from_secs(timeout_s.max(1));
    let (code, timed_out, stopped_after_image) =
        wait_for_agent_child(&mut child, deadline, stop_on_image, &image_rx);
    let stdout = rx_out
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_default();
    let stderr = rx_err
        .recv_timeout(Duration::from_secs(15))
        .unwrap_or_default();
    Ok(AgentRun {
        stdout: String::from_utf8_lossy(&stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
        code,
        timed_out,
        stopped_after_image,
    })
}

/// The file name the contract gives a sheet.
fn sheet_file(character: &str, kind: &str) -> String {
    format!("{character}-{kind}.png")
}

fn inspect_sheet_pixels(effective: &Path, reasons: &mut Vec<String>) {
    let Some(img) = image::open(effective).ok().map(|im| im.to_rgba8()) else {
        return;
    };
    let (w, h) = (img.width(), img.height());
    let low = WHITE_TOLERANCE;
    let transparent = img.pixels().filter(|px| px.0[3] < 250).count();
    if transparent > 0 {
        reasons.push(format!(
            "{transparent} pixels are not opaque: the contract wants a flat #ffffff background"
        ));
    }
    let mut edge = Vec::new();
    for x in 0..w {
        edge.extend([
            (x, 0),
            (x, 1.min(h.saturating_sub(1))),
            (x, h.saturating_sub(1)),
            (x, h.saturating_sub(2)),
        ]);
    }
    for y in 0..h {
        edge.extend([
            (0, y),
            (1.min(w.saturating_sub(1)), y),
            (w.saturating_sub(1), y),
            (w.saturating_sub(2), y),
        ]);
    }
    let edge_not_white = edge
        .into_iter()
        .filter(|&(x, y)| x < w && y < h)
        .filter(|&(x, y)| {
            let px = img.get_pixel(x, y);
            px.0[0] < 255 - low || px.0[1] < 255 - low || px.0[2] < 255 - low
        })
        .count();
    if edge_not_white > 0 {
        reasons.push(format!(
            "{edge_not_white} pixels on the canvas edge are not flat white: the background must \
             be #ffffff and no drawing may touch the edge"
        ));
    }
}

/// One sheet against the contract: the agent's drawing, the normalised sheet, and whether the
/// normalised sheet has the contract's canvas, a flat white background and no drawing on the
/// edge.
fn sheet_report(kind: &str, raw: &Path, normalised: &Path, canvas: (u32, u32)) -> Value {
    let mut reasons: Vec<String> = Vec::new();
    let raw_file = raw.to_string_lossy().to_string();
    let raw_dims = image::image_dimensions(raw).ok();
    if !raw.is_file() {
        reasons.push("the agent wrote no sheet at this path".to_string());
    } else if raw_dims.is_none() {
        reasons.push("the sheet the agent wrote is not a readable image".to_string());
    }
    // The sheet that counts is the normalised one when it exists; a drawing that already came
    // back at the contract size is the sheet itself.
    let raw_is_contract = raw_dims == Some(canvas);
    let effective = if normalised.is_file() || !raw_is_contract {
        normalised
    } else {
        raw
    };
    let file = effective.to_string_lossy().to_string();
    if !effective.is_file() {
        reasons.push(format!(
            "no sheet at the contract size: {file} is missing — normalise the drawing with \
             `image.image_size source={raw_file} out={file} width={w} height={h}`",
            w = canvas.0,
            h = canvas.1
        ));
        return json!({
            "kind": kind,
            "raw": {"file": raw_file, "exists": raw.is_file(), "width": raw_dims.map(|d| d.0).unwrap_or(0), "height": raw_dims.map(|d| d.1).unwrap_or(0)},
            "file": file,
            "normalised": normalised.to_string_lossy(),
            "exists": false,
            "ok": false,
            "reasons": reasons,
        });
    }
    let dims = image::image_dimensions(effective).ok();
    match dims {
        None => reasons.push("the sheet is not a readable image".to_string()),
        Some((w, h)) if (w, h) != canvas => reasons.push(format!(
            "{w}x{h} px, but the contract needs {}x{} px: normalise it with `image.image_size` \
             (the image tool returns its own size)",
            canvas.0, canvas.1
        )),
        Some(_) => {}
    }
    inspect_sheet_pixels(effective, &mut reasons);
    let ok = reasons.is_empty();
    json!({
        "kind": kind,
        "raw": {
            "file": raw_file,
            "exists": raw.is_file(),
            "width": raw_dims.map(|d| d.0).unwrap_or(0),
            "height": raw_dims.map(|d| d.1).unwrap_or(0),
        },
        "file": file,
        "normalised": normalised.to_string_lossy(),
        "exists": true,
        "width": dims.map(|d| d.0).unwrap_or(0),
        "height": dims.map(|d| d.1).unwrap_or(0),
        "canvas_ok": dims.map(|d| d == canvas).unwrap_or(false),
        "ok": ok,
        "reasons": reasons,
    })
}

// Review metadata and generated drafts belong to the selected project even when its
// source assets are referenced through a symlink or an external library.
fn ensure_project_output(s: &Session, destination: &Path) -> Result<()> {
    let project = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Open a project first".into()))?
        .canonicalize()?;
    let root = project
        .parent()
        .ok_or_else(|| EngineError::InvalidParams("Project has no folder".into()))?;
    let ancestor = destination
        .ancestors()
        .find(|p| p.exists())
        .ok_or_else(|| EngineError::InvalidParams("Output has no existing parent folder".into()))?
        .canonicalize()?;
    if !ancestor.starts_with(root) {
        return Err(EngineError::InvalidParams(
            "Sheet output must be inside the selected project, not a linked source library".into(),
        ));
    }
    Ok(())
}

/// One source asset, three independently reviewable sheet types. No generated sheet is
/// interpreted as rig parts until the user has reviewed its complete source image.
struct SheetPlan {
    asset: String,
    source: PathBuf,
    reference: PathBuf,
    source_prompt: Option<String>,
    suggested_kind: String,
    directory: PathBuf,
    sheets: Vec<Value>,
    motions: Value,
}
fn parse_sheet_kinds(params: &Value) -> Result<Vec<&str>> {
    let mut kinds: Vec<&str> = match &params["kinds"] {
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        Value::String(kind) => vec![kind],
        Value::Null => vec!["poses"],
        _ => {
            return Err(EngineError::InvalidParams(
                "kinds must be an array of sheet types".into(),
            ));
        }
    };
    if kinds.is_empty() || kinds.iter().any(|k| !KINDS.contains(k)) {
        return Err(EngineError::InvalidParams(
            "Choose turnaround, skeleton, or poses".into(),
        ));
    }
    kinds.dedup();
    Ok(kinds)
}

fn parse_sheet_motions(params: &Value) -> Result<Vec<&str>> {
    match params.get("motions") {
        Some(Value::Array(items)) if !items.is_empty() => items
            .iter()
            .map(|v| {
                v.as_str().ok_or_else(|| {
                    EngineError::InvalidParams(
                        "motions must be an ordered array of action IDs".into(),
                    )
                })
            })
            .collect(),
        None => Ok(vec![params["motion"].as_str().unwrap_or("sway")]),
        _ => Err(EngineError::InvalidParams(
            "motions must contain action IDs in the requested order".into(),
        )),
    }
}

fn build_sheet_entry(
    asset: &str,
    directory: &Path,
    params: &Value,
    previous: &Value,
    style_id: &str,
    style: &Value,
    kinds_len: usize,
    requests_len: usize,
    index: usize,
    kind: &str,
    action: &str,
) -> Result<Value> {
    let label = match kind {
        "turnaround" => "Turnaround",
        "skeleton" => "Parts and joints",
        _ => "Animation frames",
    };
    // Panels belong to the selected sheet type, never every sheet in a multi-sheet request.
    let mut options = if kinds_len == 1 || kind == "poses" {
        params.clone()
    } else {
        json!({})
    };
    options["motion"] = json!(action);
    let layout = SheetLayout::new(kind, &options)?;
    let key = if kind == "poses" {
        format!("poses-{}", layout.motion)
    } else {
        kind.to_string()
    };
    let direction = params["prompts"][&key]
        .as_str()
        .or(params["prompts"][kind].as_str())
        .filter(|p| !p.trim().is_empty())
        .unwrap_or("Preserve the character's identity, silhouette, proportions, distinctive features and palette.");
    let template_prompt = format!(
        "{}\n\n{}",
        style["prompt"].as_str().unwrap_or_default(),
        layout.prompt()
    );
    let mut prompt = format!("{direction}\n\n{template_prompt}");
    if let Some(note) = params["note"].as_str().filter(|note| !note.trim().is_empty()) {
        prompt.push_str(&format!("\nUser direction: {note}"));
    }
    let key = if requests_len > 1 {
        format!("{key}-{:02}", index + 1)
    } else {
        key
    };
    let existing = previous["sheets"].as_array().and_then(|v| {
        v.iter()
            .find(|v| v["kind"] == kind && v["layout"]["motion"] == layout.motion)
    });
    let file = existing
        .and_then(|v| v["file"].as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| directory.join(sheet_file(asset, &key)));
    let dimensions = image::image_dimensions(&file).ok();
    Ok(json!({
        "key":key,"kind":kind,"label":label,"prompt":prompt,"direction":direction,
        "art_style":style_id,"template_prompt":template_prompt,"layout":layout,
        "template_svg":layout.svg(),"file":file,"exists":dimensions.is_some(),
        "width":dimensions.map(|d|d.0),"height":dimensions.map(|d|d.1),"review":"pending"
    }))
}

fn resolve_sheet_reference(s: &Session, params: &Value, source: &Path, receipt: &Value) -> Result<PathBuf> {
    let mut reference = receipt["references"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["asset_id"].as_str())
        .find_map(|id| {
            s.project_assets
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| a.path.clone())
        })
        .unwrap_or_else(|| source.to_path_buf());
    if let Some(path) = params["reference"].as_str() {
        reference = crate::image_cmds::medium::project_reference(s, path)?;
    }
    Ok(reference)
}

fn resolve_sheet_asset_source<'a>(
    s: &'a Session,
    params: &'a Value,
) -> Result<(&'a str, PathBuf)> {
    let asset = params["character"]
        .as_str()
        .or(params["asset"].as_str())
        .or(s.active_asset.as_deref())
        .ok_or_else(|| EngineError::InvalidParams("Choose a project asset first".into()))?;
    let registered = s
        .project_assets
        .iter()
        .find(|a| a.id == asset)
        .ok_or_else(|| {
            EngineError::InvalidParams("Asset is not registered in this project".into())
        })?;
    let source = s
        .assets
        .get(asset)
        .map(|a| a.path.clone())
        .or_else(|| registered.path.clone())
        .ok_or_else(|| EngineError::InvalidParams("Asset has no source artwork".into()))?;
    Ok((asset, source))
}

impl SheetPlan {
    fn new(s: &Session, params: &Value) -> Result<Self> {
        let dir = crate::workshop::project_dir(s)?;
        let (asset, source) = resolve_sheet_asset_source(s, params)?;
        let receipt: Value = std::fs::read(source.with_file_name("generation.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
        let source_prompt = receipt["effective_prompt"]
            .as_str()
            .or(receipt["prompt"].as_str())
            .map(str::to_owned);
        let suggested_kind = if receipt["rig_sheet"].is_object() {
            "skeleton"
        } else {
            "poses"
        }
        .to_string();
        let reference = resolve_sheet_reference(s, params, &source, &receipt)?;
        let kinds = parse_sheet_kinds(params)?;
        let directory = dir.join("assets/sheets");
        let style_id = params["art_style"].as_str().unwrap_or("original");
        let styles = layout::styles();
        let style = styles
            .as_array()
            .and_then(|items| items.iter().find(|item| item["id"] == style_id))
            .ok_or_else(|| {
                EngineError::InvalidParams(
                    "Choose an art_style from assets.sheet_plan styles".into(),
                )
            })?;
        let previous: Value = std::fs::read(directory.join(format!("{asset}-request.json")))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Value::Null);
        let motions = parse_sheet_motions(params)?;
        let requests: Vec<(&str, &str)> = kinds
            .iter()
            .flat_map(|kind| {
                if *kind == "poses" {
                    motions.iter().map(|action| (*kind, *action)).collect()
                } else {
                    vec![(*kind, "sway")]
                }
            })
            .collect();
        let sheets = requests
            .iter()
            .enumerate()
            .map(|(index, (kind, action))| {
                build_sheet_entry(
                    asset,
                    &directory,
                    params,
                    &previous,
                    style_id,
                    style,
                    kinds.len(),
                    requests.len(),
                    index,
                    kind,
                    action,
                )
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            asset: asset.into(),
            source,
            reference,
            source_prompt,
            suggested_kind,
            directory,
            sheets,
            motions: layout::motions()?,
        })
    }
    fn value(&self) -> Value {
        json!({"ok":true,"asset":self.asset,"source":self.source,"reference":self.reference,"source_prompt":self.source_prompt,"suggested_kind":self.suggested_kind,"sheets":self.sheets,
            "motions":self.motions,"styles":layout::styles(),"sheet_types":[{"id":"poses","label":"Action frames"},{"id":"turnaround","label":"Turnaround · 3D views"},{"id":"skeleton","label":"Body and limb parts"}],
            "default_motion":"sway","default_action_rows":["walk","jump","lie_down","sway"],"review_required":true,"generation_only":true,"extraction_started":false})
    }
}

/// Inspect editable prompts and existing complete sheets without starting a job or writing files.
/// Example: `magic-cli exec assets.sheet_plan asset=ship`.
pub fn cmd_assets_sheet_plan(s: &mut Session, params: &Value) -> Result<Value> {
    Ok(SheetPlan::new(s, params)?.value())
}

fn write_sheet_guides(s: &Session, plan: &mut SheetPlan, output: &Path) -> Result<()> {
    let root = crate::workspace::workspace_root(s);
    for sheet in &mut plan.sheets {
        sheet["file"] = json!(output.join(sheet_file(
            &plan.asset,
            sheet["key"].as_str().unwrap_or_default()
        )));
        let layout: SheetLayout = serde_json::from_value(sheet["layout"].clone())
            .map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        if layout.kind == "skeleton" && layout.panels.len() < 2 {
            return Err(EngineError::InvalidParams(
                "Name the existing body and limb parts before generating a parts sheet".into(),
            ));
        }
        let file = PathBuf::from(sheet["file"].as_str().unwrap_or_default());
        let guide = file.with_extension("guide.png");
        layout.write_guide(&root, &guide)?;
        std::fs::write(
            file.with_extension("layout.json"),
            serde_json::to_vec_pretty(&layout)
                .map_err(|e| EngineError::InvalidParams(e.to_string()))?,
        )?;
        sheet["template_reference"] = json!(guide);
    }
    Ok(())
}

fn generate_planned_sheets(s: &mut Session, plan: &mut SheetPlan, params: &Value) -> Result<()> {
    for sheet in &mut plan.sheets {
        let mut request = json!({
            "prompt":sheet["prompt"],
            "references":[plan.reference,sheet["template_reference"]],
            "name":format!("{} {}",plan.asset,sheet["key"].as_str().unwrap_or("sheet")),
            "transparent_background":false
        });
        for key in ["model", "provider", "quality", "timeout_s"] {
            if let Some(value) = params.get(key) {
                request[key] = value.clone();
            }
        }
        let generated = crate::image_generation::generate(s, &request)?;
        let source = generated["image"]
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("Gaffer returned no image".into()))?;
        let destination = PathBuf::from(sheet["file"].as_str().unwrap_or_default());
        std::fs::copy(source, &destination)?;
        std::fs::write(
            destination.with_extension("prompt.txt"),
            sheet["prompt"].as_str().unwrap_or_default(),
        )?;
        sheet["generation"] = generated;
    }
    Ok(())
}

fn collect_sheet_reports(plan: &SheetPlan) -> Vec<Value> {
    let mut reports = Vec::new();
    for sheet in &plan.sheets {
        let file = Path::new(sheet["file"].as_str().unwrap_or_default());
        let canvas = (
            sheet["layout"]["canvas"][0].as_u64().unwrap_or(2048) as u32,
            sheet["layout"]["canvas"][1].as_u64().unwrap_or(2048) as u32,
        );
        let mut report = sheet_report(
            sheet["kind"].as_str().unwrap_or_default(),
            file,
            file,
            canvas,
        );
        report["prompt"] = sheet["prompt"].clone();
        report["layout"] = sheet["layout"].clone();
        report["key"] = sheet["key"].clone();
        report["template_reference"] = sheet["template_reference"].clone();
        report["generation"] = sheet["generation"].clone();
        report["review"] = json!("pending");
        reports.push(report);
    }
    reports
}

/// Generate complete sheet drafts only when `run=true`. With `run=false`, return the
/// editable plan without writes. Returned technical checks never approve artwork or start extraction.
pub fn cmd_assets_agent_sheets(s: &mut Session, params: &Value) -> Result<Value> {
    let mut plan = SheetPlan::new(s, params)?;
    let run = params.get("run").and_then(Value::as_bool).unwrap_or(false);
    if !run {
        return Ok(plan.value());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| EngineError::InvalidParams(e.to_string()))?
        .as_nanos();
    let output = plan.directory.join(format!("{}-{stamp}", plan.asset));
    ensure_project_output(s, &output)?;
    std::fs::create_dir_all(&output)?;
    write_sheet_guides(s, &mut plan, &output)?;
    let brief = format!(
        "Draw only the complete sheets in this JSON request. Attach BOTH the artwork `reference` and each sheet's `template_reference` PNG to the native image generation tool. The artwork controls identity; the numbered guide controls layout, frame order and drawing bounds. Keep dividers and captions outside drawing bounds. Follow every panel description. Report actual returned dimensions. Copy each whole result to its exact file path and leave its .layout.json intact. Save the exact image prompt beside it as .prompt.txt. Stop after drawing. Human review is required before resizing, cutting, registration, or rigging. Do not run preparation commands and do not create Markdown documents.\n{}",
        plan.value()
    );
    let brief_path = output.join("request.txt");
    std::fs::write(&brief_path, &brief)?;
    let manifest = plan.directory.join(format!("{}-request.json", plan.asset));
    std::fs::write(&manifest, plan.value().to_string())?;
    std::fs::write(output.join("request.json"), plan.value().to_string())?;

    generate_planned_sheets(s, &mut plan, params)?;

    // Never resize or cut a generated result before its full-sheet review.
    let reports = collect_sheet_reports(&plan);
    let mut result = plan.value();
    result["ok"] = json!(reports.iter().all(|r| r["raw"]["exists"] == true));
    result["sheets"] = json!(reports);
    result["agent"] = json!("image.generate");
    result["request"] = json!(manifest);
    std::fs::write(output.join("result.json"), result.to_string())?;
    result["next"] = json!([
        "Review each complete source sheet in the app before preparing any individual assets."
    ]);
    Ok(result)
}

/// Full-sheet review is tied to source bytes, asset identity, and the selected project.
/// It approves no inferred anatomy and never starts extraction or generation.
fn sheet_identity(s: &Session, asset: &str, sheet: &Path) -> Result<(PathBuf, String)> {
    let project = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Open a project first".into()))?
        .canonicalize()?;
    let source = sheet.canonicalize()?;
    let registered = s
        .project_assets
        .iter()
        .find(|a| a.id == asset)
        .ok_or_else(|| EngineError::InvalidParams("Choose a registered project asset".into()))?;
    let same_source = s
        .project_assets
        .iter()
        .filter_map(|a| a.path.as_ref())
        .filter_map(|p| p.canonicalize().ok())
        .any(|p| p == source)
        || s.assets
            .get(asset)
            .and_then(|a| a.path.canonicalize().ok())
            .is_some_and(|p| p == source)
        || registered
            .path
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .is_some_and(|p| p == source);
    if !same_source
        && !project
            .parent()
            .is_some_and(|root| source.starts_with(root))
    {
        return Err(EngineError::InvalidParams(
            "The sheet must belong to the selected project asset or project folder".into(),
        ));
    }
    image::image_dimensions(&source)?;
    let mut hash = Sha256::new();
    std::io::copy(&mut std::fs::File::open(&source)?, &mut hash)?;
    Ok((project, format!("{:x}", hash.finalize())))
}
fn review_file(project: &Path, asset: &str, hash: &str) -> Result<PathBuf> {
    Ok(project
        .parent()
        .ok_or_else(|| EngineError::InvalidParams("Project has no folder".into()))?
        .join("assets/sheets/reviews")
        .join(format!(
            "{:x}.json",
            Sha256::digest(format!("{asset}\0{hash}").as_bytes())
        )))
}
fn review(s: &Session, asset: &str, sheet: &Path) -> Result<Value> {
    let (project, hash) = sheet_identity(s, asset, sheet)?;
    let stored: Value = std::fs::read(review_file(&project, asset, &hash)?)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let approved = stored["approved"] == true
        && stored["asset"] == asset
        && stored["project"] == json!(project);
    let (width, height) = image::image_dimensions(sheet)?;
    Ok(
        json!({"asset":asset,"file":sheet,"sha256":hash,"width":width,"height":height,
        "approved":approved,"review":if approved{"approved"}else{"pending"}}),
    )
}
/// Inspect the complete sheet and its exact review state without modifying source pixels.
pub fn cmd_assets_sheet_info(s: &mut Session, p: &Value) -> Result<Value> {
    let asset = p["asset"]
        .as_str()
        .ok_or_else(|| EngineError::InvalidParams("asset is required".into()))?;
    let sheet = crate::workspace::resolve_workspace_path(
        s,
        p["sheet"]
            .as_str()
            .ok_or_else(|| EngineError::InvalidParams("sheet is required".into()))?,
    );
    review(s, asset, &sheet)
}
fn write_sheet_review_record(s: &Session, project: &Path, result: &Value) -> Result<()> {
    let file = review_file(
        project,
        result["asset"].as_str().unwrap_or_default(),
        result["sha256"].as_str().unwrap_or_default(),
    )?;
    ensure_project_output(s, &file)?;
    std::fs::create_dir_all(
        file.parent()
            .ok_or_else(|| EngineError::InvalidParams("Review folder missing".into()))?,
    )?;
    std::fs::write(&file, result.to_string())?;
    Ok(())
}

/// Record an explicit full-sheet approval or request changes. The exact bytes must match
/// the source displayed by the caller. Example: `assets.sheet_review asset=ship sheet=... sha256=... approved=true expected_project=...`.
pub fn cmd_assets_sheet_review(s: &mut Session, p: &Value) -> Result<Value> {
    crate::motion_authoring::project_guard(s, p)?;
    let mut result = cmd_assets_sheet_info(s, p)?;
    if result["sha256"] != p["sha256"] {
        return Err(EngineError::InvalidParams(
            "Sheet changed since review; open its full image again".into(),
        ));
    }
    let approved = p["approved"]
        .as_bool()
        .ok_or_else(|| EngineError::InvalidParams("approved must be true or false".into()))?;
    let project = s
        .project_path
        .as_ref()
        .ok_or_else(|| EngineError::InvalidParams("Open a project first".into()))?
        .canonicalize()?;
    result["project"] = json!(project);
    result["approved"] = json!(approved);
    result["review"] = json!(if approved {
        "approved"
    } else {
        "changes_requested"
    });
    result["note"] = p["note"].clone();
    write_sheet_review_record(s, &project, &result)?;
    Ok(result)
}
/// Preparation consumes only a reviewed source revision; changed bytes need another review.
pub(crate) fn require_sheet_review(s: &Session, asset: &str, sheet: &Path) -> Result<()> {
    if review(s, asset, sheet)?["approved"] != true {
        return Err(EngineError::InvalidParams("Review the complete source sheet in Asset sheets and approve it before extracting individual assets".into()));
    }
    Ok(())
}
