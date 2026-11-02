//! Bounded Blender jobs. Studio reads metadata; evaluation and rendering belong to the worker.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const UNIT: &str = "magicstudio-animation-worker.service";
const JSON_LIMIT: u64 = 16 * 1024 * 1024;
const PROPERTIES: &[&str] = &[
    "LoadState",
    "ActiveState",
    "SubState",
    "Description",
    "InvocationID",
    "Result",
    "ExecMainStatus",
    "ExecMainCode",
    "MemoryCurrent",
    "MemoryPeak",
    "MemoryMax",
    "MemorySwapCurrent",
    "MemorySwapMax",
    "CPUUsageNSec",
    "MainPID",
    "ExecMainStartTimestamp",
];
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    // Limit the read itself, including a file that grows after metadata inspection.
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(JSON_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > JSON_LIMIT {
        return Err(invalid("Animation metadata exceeds 16 MiB"));
    }
    Ok(bytes)
}
fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&read_bytes(path)?).map_err(|e| invalid(e.to_string()))
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?,
    )?;
    fs::rename(temporary, path)?;
    Ok(())
}
fn worker_lock() -> Result<fs::File> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| invalid("A user systemd runtime is required"))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(PathBuf::from(runtime).join("magicstudio-animation-worker.lock"))?;
    file.try_lock().map_err(|_| {
        invalid("Another animation command is submitting or cancelling; retry shortly")
    })?;
    Ok(file)
}
fn service() -> Result<Value> {
    let output = Command::new("systemctl")
        .args(["--user", "show", UNIT, "--timestamp=unix"])
        .arg(format!("--property={}", PROPERTIES.join(",")))
        .output()?;
    let mut state = json!({});
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((key, value)) = line.split_once('=') {
            state[key] = json!(value);
        }
    }
    if state["LoadState"].is_null()
        || (!output.status.success() && state["LoadState"] != "not-found")
    {
        return Err(EngineError::ExternalTool(
            "systemctl".into(),
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }
    Ok(state)
}
fn occupied(state: &Value) -> bool {
    !matches!(state["ActiveState"].as_str(), Some("inactive" | "failed"))
}
fn number(state: &Value, key: &str) -> Value {
    state[key]
        .as_str()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v != u64::MAX)
        .map_or(Value::Null, |v| json!(v))
}
fn telemetry(state: &Value) -> Value {
    json!({"memory_current_bytes":number(state,"MemoryCurrent"),"memory_peak_bytes":number(state,"MemoryPeak"),
        "memory_max_bytes":number(state,"MemoryMax"),"swap_current_bytes":number(state,"MemorySwapCurrent"),
        "swap_max_bytes":number(state,"MemorySwapMax"),"cpu_usage_ns":number(state,"CPUUsageNSec")})
}
// Runtime identity is published under the same lock as start/cancel. Reads never select
// an owner from project history; the observed unit must identify this exact job.
fn owner_path() -> Result<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|p| PathBuf::from(p).join("magicstudio-animation-worker-owner.json"))
        .ok_or_else(|| invalid("User runtime directory is unavailable"))
}
fn started_at(state: &Value) -> Value {
    state["ExecMainStartTimestamp"]
        .as_str()
        .and_then(|s| s.strip_prefix('@'))
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|s| s.is_finite() && *s > 0.0)
        .map_or(Value::Null, |s| json!(((s * 1000.0) as u64).to_string()))
}
fn job_details(dir: &Path) -> Result<Value> {
    let mut details = read_json(&dir.join("job.json"))?;
    for key in [
        "backend",
        "limits",
        "submitted_at",
        "started_at",
        "cancellation_requested_at",
        "source_id",
        "source_revision",
    ] {
        if details.get(key).is_none() {
            details[key] = Value::Null;
        }
    }
    if let Ok(marker) = read_json(&dir.join("cancel-requested")) {
        details["cancellation_requested_at"] = marker
            .get("cancellation_requested_at")
            .unwrap_or(&marker["updated_at"])
            .clone();
    }
    Ok(details)
}
fn add_job_details(value: &mut Value, details: &Value) {
    for key in [
        "backend",
        "limits",
        "submitted_at",
        "started_at",
        "cancellation_requested_at",
        "source_id",
        "source_revision",
    ] {
        if value.get(key).is_none() {
            value[key] = details[key].clone();
        }
    }
}
fn owner_observation(state: &Value, details: Option<&Value>, error: Option<String>) -> Value {
    let owner = if occupied(state) {
        let id = &state["Description"];
        let matching = details.filter(|d| {
            d["job"] == *id
                && (!d["invocation_id"].is_string() || d["invocation_id"] == state["InvocationID"])
        });
        let mut start = started_at(state);
        if start.is_null() {
            start = matching.map_or(Value::Null, |d| d["started_at"].clone());
        }
        json!({"job":id,"job_id":id,"project":matching.map(|d|&d["project"]),"unit":UNIT,
            "backend":matching.map(|d|&d["backend"]),"invocation_id":state["InvocationID"],"updated_at":now(),"service_state":state["ActiveState"],
            "state":if state["ActiveState"]=="deactivating" || matching.is_some_and(|d|!d["cancellation_requested_at"].is_null()){"cancelling"}else{"running"},
            "limits":matching.map(|d|&d["limits"]),"submitted_at":matching.map(|d|&d["submitted_at"]),
            "started_at":start,"cancellation_requested_at":matching.map(|d|&d["cancellation_requested_at"])})
    } else {
        Value::Null
    };
    json!({"worker_owner":owner,"worker_owner_status":if owner.is_null(){"idle"}else{"observed"},"worker_owner_error":error})
}
/// A unit the app itself started: the receiving state and the job name prefix must agree.
fn names_worker_job(state: &Value) -> bool {
    matches!(
        state["ActiveState"].as_str(),
        Some("active" | "activating" | "deactivating" | "reloading")
    ) && state["Description"].as_str().is_some_and(|id| {
        id.starts_with("magicstudio-blender-") || id.starts_with("magicstudio-native-")
    })
}

/// The recorded owner, when its job, directory and invocation agree with the observed unit.
fn owner_details(state: &Value) -> Result<Value> {
    let owner = read_json(&owner_path()?)?;
    if owner["job"] != state["Description"] {
        return Err(invalid("Runtime owner does not match observed worker"));
    }
    let dir = Path::new(
        owner["directory"]
            .as_str()
            .ok_or_else(|| invalid("Owner directory unavailable"))?,
    );
    let details = job_details(dir)?;
    if details["job"] != state["Description"]
        || (details["invocation_id"].is_string()
            && details["invocation_id"] != state["InvocationID"])
    {
        return Err(invalid(
            "Owner job/invocation does not match observed worker",
        ));
    }
    Ok(details)
}

fn worker_owner(observed: &Result<Value>) -> Value {
    let state = match observed {
        Ok(state) => state,
        Err(error) => {
            return json!({"worker_owner":Value::Null,"worker_owner_status":"unavailable","worker_owner_error":error.to_string()})
        }
    };
    if !occupied(state) {
        return owner_observation(state, None, None);
    }
    if !names_worker_job(state) {
        return json!({"worker_owner":Value::Null,"worker_owner_status":"unavailable","worker_owner_error":"No recognizable worker job identity/state was observed"});
    }
    match owner_details(state) {
        Ok(details) => owner_observation(state, Some(&details), None),
        Err(error) => owner_observation(state, None, Some(error.to_string())),
    }
}
fn add_owner(value: &mut Value, owner: &Value) {
    for key in ["worker_owner", "worker_owner_status", "worker_owner_error"] {
        value[key] = owner[key].clone();
    }
}
fn root(s: &Session) -> Result<PathBuf> {
    s.project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("animation/blender-jobs"))
        .ok_or_else(|| invalid("Open a .magic project first"))
}
pub(crate) fn job(s: &Session, p: &Value) -> Result<(String, PathBuf)> {
    let id = p["job"]
        .as_str()
        .ok_or_else(|| invalid("job is required"))?;
    if !(id.starts_with("magicstudio-blender-") || id.starts_with("magicstudio-native-"))
        || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err(invalid("Invalid animation job id"));
    }
    let dir = root(s)?.join(id);
    if !dir.join("request.json").is_file() {
        return Err(invalid("Job not found in this project"));
    }
    let request = read_json(&dir.join("request.json"))?;
    if request["project"].is_string() && request["project"] != json!(s.project_path) {
        return Err(invalid("Job belongs to another project"));
    }
    Ok((id.into(), dir))
}
fn bounded(p: &Value, key: &str, default: u64, min: u64, max: u64) -> Result<u64> {
    let value = match p.get(key) {
        None => default,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| invalid(format!("{key} must be an integer")))?,
    };
    if !(min..=max).contains(&value) {
        return Err(invalid(format!("{key} must be {min}–{max}")));
    }
    Ok(value)
}
fn entry(s: &Session, p: &Value) -> Result<crate::ProjectAsset> {
    let id = p["character"]
        .as_str()
        .ok_or_else(|| invalid("character is required for legacy migration"))?;
    let mut asset = s
        .project_assets
        .iter()
        .find(|a| a.id == id)
        .cloned()
        .ok_or_else(|| invalid("Character is not registered"))?;
    if let Some(loaded) = s.assets.get(id) {
        asset.parts = loaded.parts.clone();
    }
    let path = asset
        .path
        .as_ref()
        .ok_or_else(|| invalid("Migration requires a saved rig file"))?;
    asset.path =
        Some(crate::workspace::resolve_workspace_path(s, &path.to_string_lossy()).canonicalize()?);
    Ok(asset)
}
pub fn cmd_animation_actions(s: &mut Session, p: &Value) -> Result<Value> {
    let asset = entry(s, p)?;
    let path = asset
        .path
        .as_ref()
        .ok_or_else(|| invalid("Missing rig path"))?;
    let raw = read_json(path)?;
    let spec = if raw["tracks"].is_array() {
        &raw
    } else {
        &raw["effectcraft"]
    };
    let mut tracks: Vec<Value> = Vec::new();
    if let Some(source) = spec["tracks"].as_array() {
        if source.len() > 1024 {
            return Err(invalid("Too many animation tracks"));
        }
        tracks.extend(source.iter().map(|t| {
            json!({"id":t["id"],"duration":t["duration"],
                "loop":t["loop"].as_bool().unwrap_or(false),"standard":false})
        }));
    }
    let meta = crate::motion::cmd_motion_tracks(s, &json!({"character": asset.id}))?;
    if let Some(meta_tracks) = meta["tracks"].as_array() {
        for mt in meta_tracks {
            let id = mt["id"].as_str().unwrap_or("");
            if !id.is_empty() && !tracks.iter().any(|t| t["id"].as_str() == Some(id)) {
                tracks.push(json!({
                    "id": id,
                    "label": mt["label"],
                    "duration": mt["duration"],
                    "loop": mt["loop"].as_bool().unwrap_or(false),
                    "standard": mt["standard"].as_bool().unwrap_or(false),
                }));
            }
        }
    }
    Ok(
        json!({"character":asset.id,"tracks":tracks,"layers":[],"canvas":spec.get("canvas").unwrap_or(&raw["canvas_size"]),"spec":path}),
    )
}
/// Helpers must validate the complete request's job against their actual cgroup invocation.
pub(crate) fn require_worker_invocation(request: &Value) -> Result<()> {
    let active = service()?;
    if active["Description"] != request["job"]
        || active["InvocationID"].as_str() != std::env::var("INVOCATION_ID").ok().as_deref()
        || !occupied(&active)
    {
        return Err(invalid(
            "Native preparation must run inside the selected bounded worker service",
        ));
    }
    Ok(())
}

/// Only the headless helper evaluates legacy motion; never attach a Studio project here.
fn require_headless_session(s: &Session) -> Result<()> {
    if s.project_path.is_some() {
        return Err(invalid(
            "Preparation requires a separate headless worker session",
        ));
    }
    Ok(())
}

fn request_path(p: &Value) -> Result<PathBuf> {
    Ok(PathBuf::from(
        p["request"]
            .as_str()
            .ok_or_else(|| invalid("request is required"))?,
    ))
}

/// The rig file, its hash and its parsed contents.
fn read_rig(spec: &Path) -> Result<(Vec<u8>, Value, String)> {
    let bytes = read_bytes(spec)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let raw: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    Ok((bytes, raw, hash))
}

/// A rig path field is relative to the rig spec unless it is already absolute.
fn resolve_relative(base: &Path, value: &mut Value) {
    let Some(text) = value.as_str().filter(|v| !v.is_empty()) else {
        return;
    };
    let original = PathBuf::from(text);
    *value = json!(if original.is_absolute() {
        original
    } else {
        base.join(original)
    });
}

/// Snapshot reads the rig from beside the spec, so keep every media path absolute.
fn resolve_rig_paths(raw: &mut Value, base: &Path) {
    for key in [
        "source",
        "source_image",
        "photocraft_psd",
        "cutout_asset_sheet",
    ] {
        resolve_relative(base, &mut raw[key]);
    }
    if let Some(layers) = raw["layers"].as_array_mut() {
        for layer in layers {
            resolve_relative(base, &mut layer["file"]);
        }
    }
    if let Some(parts) = raw["parts"].as_array_mut() {
        for part in parts {
            for key in ["cropped_path", "full_layer_path"] {
                resolve_relative(base, &mut part[key]);
            }
        }
    }
}

fn safe_layer_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Keyed layers are copied by name, so the name must be one that cannot escape the snapshot.
fn copy_keyed_layers(raw: &Value, base: &Path, snapshot: &Path) -> Result<()> {
    let Some(layers) = raw["layers"].as_object() else {
        return Ok(());
    };
    for id in layers.keys() {
        if !safe_layer_id(id) {
            return Err(invalid("Unsafe keyed layer id"));
        }
        fs::copy(
            base.join(format!("{id}.png")),
            snapshot.join(format!("{id}.png")),
        )?;
    }
    Ok(())
}

/// Evaluate the motion against the isolated snapshot, never against the live project.
fn write_sample(
    path: &Path,
    isolated: &mut Session,
    motion: &Value,
    hash: &str,
    spec: &Path,
) -> Result<()> {
    let mut meta = crate::motion::cmd_motion_tracks(isolated, motion)?;
    let sample = crate::motion::cmd_motion_sample(isolated, motion)?;
    meta["rig_sha256"] = json!(hash);
    meta["spec"] = json!(spec);
    write_json(
        &path.with_file_name("sample.json"),
        &json!({"meta":meta,"sample":sample}),
    )
}

/// Snapshot the rig beside the request, then evaluate the motion against that copy.
fn prepare_rig(path: &Path, request: &Value, mut asset: crate::ProjectAsset) -> Result<()> {
    let spec = asset
        .path
        .clone()
        .ok_or_else(|| invalid("Missing rig path"))?;
    let (bytes, mut raw, hash) = read_rig(&spec)?;
    let base = spec
        .parent()
        .ok_or_else(|| invalid("Missing rig directory"))?;
    let snapshot = path.with_file_name("rig-snapshot");
    fs::create_dir_all(&snapshot)?;
    fs::write(snapshot.join("original-rig.json"), bytes)?;
    resolve_rig_paths(&mut raw, base);
    copy_keyed_layers(&raw, base, &snapshot)?;
    let snapshot_path = snapshot.join("rig.json");
    write_json(&snapshot_path, &raw)?;
    asset.path = Some(snapshot_path);
    let mut isolated = Session {
        project_assets: vec![asset],
        ..Session::default()
    };
    write_sample(path, &mut isolated, &request["motion"], &hash, &spec)
}

pub fn cmd_prepare_animation_job(s: &mut Session, p: &Value) -> Result<Value> {
    require_headless_session(s)?;
    let path = request_path(p)?;
    let request = read_json(&path)?;
    require_worker_invocation(&request)?;
    let asset: crate::ProjectAsset =
        serde_json::from_value(request["asset"].clone()).map_err(|e| invalid(e.to_string()))?;
    prepare_rig(&path, &request, asset)?;
    Ok(json!({"ok":true,"prepared":true}))
}

fn scene_request(s: &Session, p: &Value) -> Result<Value> {
    Ok(json!({"resolution_percent":bounded(p,"resolution_percent",50,10,100)?,
        "max_frames":bounded(p,"max_frames",90,1,18000)?,"unit":UNIT,"project":s.project_path,"character":p["character"],"track":p["track"]}))
}

/// One scene source per job: a blend, a rig plan or a volume plan.
fn validate_scene_source(p: &Value) -> Result<()> {
    let chosen = ["blend", "rig_plan", "volume_plan"]
        .iter()
        .filter(|key| p[**key].is_string())
        .count();
    if chosen > 1 {
        return Err(invalid("Choose one of blend, rig_plan or volume_plan"));
    }
    Ok(())
}

/// A plan file the job carries: its resolved path, bytes and parsed contents.
fn read_plan(s: &Session, p: &Value, key: &str) -> Result<(PathBuf, Vec<u8>, Value)> {
    let text = p[key]
        .as_str()
        .ok_or_else(|| invalid("A plan path is required"))?;
    let path = crate::workspace::resolve_workspace_path(s, text).canonicalize()?;
    let bytes = read_bytes(&path)?;
    let plan: Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    Ok((path, bytes, plan))
}

fn volume_plan_request(s: &Session, p: &Value, request: &mut Value) -> Result<()> {
    let (path, bytes, plan) = read_plan(s, p, "volume_plan")?;
    if plan["slug"] != p["character"] || !p["character"].is_string() {
        return Err(invalid("Volume construction requires character matching slug"));
    }
    let track = p["track"]
        .as_str()
        .ok_or_else(|| invalid("Choose turntable or an authored skeleton track"))?;
    if track != "turntable" && !plan["skeleton"]["tracks"][track].is_array() {
        return Err(invalid("Choose turntable or an authored skeleton track"));
    }
    request["volume_plan"] = json!({"path":path,"sha256":format!("{:x}",Sha256::digest(&bytes)),"data":plan});
    request["duration"] = json!(bounded(p,"duration",4,1,600)?);
    request["voxel_resolution"] = json!(bounded(p,"voxel_resolution",144,16,256)?);
    Ok(())
}

fn rig_plan_request(s: &Session, p: &Value, request: &mut Value) -> Result<()> {
    let (path, bytes, plan) = read_plan(s, p, "rig_plan")?;
    if plan["character_id"] != p["character"] || !p["character"].is_string() {
        return Err(invalid("character must match the rig plan's character_id"));
    }
    if !p["track"].is_string() || !plan["pose_library"]["poses"].is_object() {
        return Err(invalid(
            "Choose a track and include the authored pose_library in the rig plan",
        ));
    }
    request["rig_plan"] = json!({"path":path,"sha256":format!("{:x}",Sha256::digest(&bytes)),"data":plan});
    request["duration"] = json!(bounded(p,"duration",3,1,600)?);
    Ok(())
}

fn blend_request(s: &Session, p: &Value, request: &mut Value) -> Result<()> {
    let text = p["blend"]
        .as_str()
        .ok_or_else(|| invalid("blend must be an existing .blend file"))?;
    let path = crate::workspace::resolve_workspace_path(s, text).canonicalize()?;
    if path.extension().is_none_or(|e| e != "blend") {
        return Err(invalid("blend must be an existing .blend file"));
    }
    request["blend"] = json!(path);
    Ok(())
}

/// Legacy migration evaluates the project's saved motion, so it needs the theatre state
/// to agree with disk and the same magic-cli binary beside the app.
fn legacy_request(s: &mut Session, p: &Value, request: &mut Value) -> Result<()> {
    let character = p["character"]
        .as_str()
        .ok_or_else(|| invalid("character is required for legacy migration"))?;
    crate::motion_authoring::theatre_guard(
        s,
        character,
        p["track"].as_str().filter(|id| !id.is_empty()),
    )?;
    let actions = cmd_animation_actions(s, p)?;
    if !p["track"].is_string()
        || !actions["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|t| t["id"] == p["track"])
    {
        return Err(invalid("Choose a saved animation track"));
    }
    request["asset"] = serde_json::to_value(entry(s, p)?).map_err(|e| invalid(e.to_string()))?;
    request["motion"] = json!({"character":p["character"],"track":p["track"],"fps":p["fps"].as_f64().unwrap_or(30.0)});
    // Use the rig's authored rate unless explicitly overridden.
    if p.get("fps").is_none()
        && let Some(motion) = request["motion"].as_object_mut()
    {
        motion.remove("fps");
    }
    let exe = std::env::current_exe()?;
    let executable = [exe.with_file_name("magic-cli")]
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| {
            invalid("Install magic beside Magic Studio to prepare legacy animations")
        })?;
    request["magic_cli"] = json!(executable);
    Ok(())
}

pub fn cmd_start_scene_job(s: &mut Session, p: &Value) -> Result<Value> {
    let mut request = scene_request(s, p)?;
    validate_scene_source(p)?;
    if p["volume_plan"].is_string() {
        volume_plan_request(s, p, &mut request)?;
    } else if p["rig_plan"].is_string() {
        rig_plan_request(s, p, &mut request)?;
    } else if p["blend"].is_string() {
        blend_request(s, p, &mut request)?;
    } else {
        legacy_request(s, p, &mut request)?;
    }
    submit(s, p, request, "blender")
}

/// All native backends use the same slot, lock, limits, receipts and cancellation.
/// Keep the historical job directory so existing Blender receipts remain readable.
/// What the submission names, sizes and describes about one job.
struct JobPlan<'a> {
    backend: &'a str,
    id: String,
    stamp: u128,
    memory: u64,
    seconds: u64,
}

fn job_stamp() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| invalid(e.to_string()))?
        .as_nanos())
}

fn fill_request(request: &mut Value, s: &Session, p: &Value, plan: &JobPlan) {
    request["unit"] = json!(UNIT);
    request["project"] = json!(s.project_path);
    request["backend"] = json!(plan.backend);
    request["source_id"] = p["source_id"].clone();
    request["source_revision"] = p["source_revision"].clone();
    request["job"] = json!(plan.id);
    request["limits"] = json!({"memory_max_bytes":plan.memory*1024*1024,"swap_max_bytes":0,"cpu_quota_percent":200,"timeout_seconds":plan.seconds});
    request["submitted_at"] = json!((plan.stamp / 1_000_000).to_string());
}

/// The job record and its starting progress state, both written before the unit starts.
fn write_job_records(
    dir: &Path,
    s: &Session,
    p: &Value,
    request: &Value,
    plan: &JobPlan,
) -> Result<()> {
    write_json(
        &dir.join("job.json"),
        &json!({"job":plan.id,"job_id":plan.id,"unit":UNIT,"project":s.project_path,
        "backend":plan.backend,"source_id":p["source_id"],"source_revision":p["source_revision"],"created_ns":plan.stamp.to_string(),"character":p["character"],"track":p["track"],"source_blend":request["blend"],
        "limits":request["limits"],"submitted_at":request["submitted_at"],"started_at":Value::Null,"cancellation_requested_at":Value::Null}),
    )?;
    write_json(
        &dir.join("progress.json"),
        &json!({"state":if request["blend"].is_string(){"loading"}else{"preparing"}}),
    )
}

/// The worker script, plus the pipeline modules a rig or volume plan runs beside it.
fn write_worker_payloads(dir: &Path, request: &Value) -> Result<()> {
    fs::write(
        dir.join("worker.py"),
        include_str!("../../../pipeline/animation_job_worker.py"),
    )?;
    if request["rig_plan"].is_object() {
        fs::create_dir_all(dir.join("pipeline"))?;
        fs::write(dir.join("pipeline/__init__.py"), "")?;
        fs::write(dir.join("pipeline/rig_motion.py"), include_str!("../../../pipeline/rig_motion.py"))?;
        fs::write(dir.join("pipeline/build_cutout_skeleton.py"), include_str!("../../../pipeline/build_cutout_skeleton.py"))?;
    }
    if request["volume_plan"].is_object() {
        fs::write(dir.join("volume_reconstruction.py"), include_str!("../../../pipeline/volume_reconstruction.py"))?;
        fs::write(dir.join("build_volume_scene.py"), include_str!("../../../pipeline/build_volume_scene.py"))?;
        fs::write(dir.join("bind_volume_skeleton.py"), include_str!("../../../pipeline/bind_volume_skeleton.py"))?;
    }
    Ok(())
}

fn worker_command(backend: &str, worker: &Path, request_path: &Path) -> Result<Vec<String>> {
    if backend != "blender" {
        return Err(EngineError::InvalidParams(format!(
            "Unsupported animation backend: {backend}"
        )));
    }
    Ok(vec![
        crate::workspace::find_executable(
            &std::env::var("BLENDER_BIN").unwrap_or_else(|_| "blender".into()),
        )?.to_string_lossy().into_owned(),
        "--background".into(),
        "--factory-startup".into(),
        "--disable-autoexec".into(),
        "--threads".into(),
        "2".into(),
        "--python-exit-code".into(),
        "1".into(),
        "--python".into(),
        worker.to_string_lossy().into_owned(),
        "--".into(),
        request_path.to_string_lossy().into_owned(),
    ])
}

/// The unit that runs the worker: limits, log capture and the finalizing receipt hook.
fn systemd_command(
    dir: &Path,
    plan: &JobPlan,
    worker: &Path,
    request_path: &Path,
    command: &[String],
) -> Result<Command> {
    let python = crate::workspace::find_executable("python3")?;
    let mut cmd = Command::new("systemd-run");
    cmd.args(["--user", "--quiet", "--collect"])
        .arg(format!("--unit={UNIT}"))
        .arg(format!("--description={}", plan.id))
        .arg(format!("--property=MemoryMax={}M", plan.memory))
        .args([
            "--property=MemorySwapMax=0",
            "--property=CPUQuota=200%",
            "--property=TimeoutStopSec=5",
            "--property=KillMode=control-group",
            "--property=MemoryAccounting=yes",
            "--property=CPUAccounting=yes",
        ])
        .arg(format!("--property=RuntimeMaxSec={}", plan.seconds))
        .arg(format!(
            "--property=StandardOutput=append:{}",
            dir.join("worker.log").display()
        ))
        .arg(format!(
            "--property=StandardError=append:{}",
            dir.join("worker.log").display()
        ))
        .arg(format!(
            "--property=ExecStopPost={} {} --finalize {}",
            systemd_quote(&python),
            systemd_quote(worker),
            systemd_quote(request_path)
        ))
        .arg("--")
        .args(command);
    Ok(cmd)
}

/// A start that systemd-run refused still leaves a receipt, so a later status can report it.
fn write_failure_receipt(
    dir: &Path,
    s: &Session,
    request: &Value,
    id: &str,
    error: &str,
) -> Result<()> {
    write_json(
        &dir.join("result.json"),
        &json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,
        "invocation_id":Value::Null,"state":"failed","termination_reason":"submission","updated_at":now(),
        "telemetry":telemetry(&json!({})),"limits":request["limits"],"submitted_at":request["submitted_at"],
        "started_at":Value::Null,"cancellation_requested_at":Value::Null,"error":error}),
    )
}

/// systemd-run's own failure text, if the unit did not start.
fn submission_error(output: std::io::Result<std::process::Output>) -> Option<String> {
    match output {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(String::from_utf8_lossy(&output.stderr).into_owned()),
        Err(error) => Some(error.to_string()),
    }
}

/// The job directory as the worker will read it: request, records, payloads.
fn prepare_job_dir(
    dir: &Path,
    s: &Session,
    p: &Value,
    request: &Value,
    plan: &JobPlan,
) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(dir)?;
    write_json(&dir.join("request.json"), request)?;
    write_job_records(dir, s, p, request, plan)?;
    write_worker_payloads(dir, request)?;
    Ok((dir.join("worker.py"), dir.join("request.json")))
}

fn record_owner(dir: &Path, plan: &JobPlan) -> Result<()> {
    write_json(&owner_path()?, &json!({"job":plan.id,"directory":dir}))
}

pub(crate) fn submit(s: &Session, p: &Value, mut request: Value, backend: &str) -> Result<Value> {
    let _slot = worker_lock()?;
    let active = service()?;
    if occupied(&active) {
        return Err(invalid(format!(
            "Animation worker is busy with {}; finish or cancel it first",
            active["Description"]
        )));
    }
    let memory = bounded(p, "memory_mb", 4096, 512, 8192)?;
    let seconds = bounded(p, "timeout_seconds", 300, 10, 3600)?;
    let stamp = job_stamp()?;
    let kind = if backend == "blender" { "blender" } else { "native" };
    let id = format!("magicstudio-{kind}-{}-{stamp}", std::process::id());
    let plan = JobPlan { backend, id, stamp, memory, seconds };
    fill_request(&mut request, s, p, &plan);
    let dir = root(s)?.join(&plan.id);
    let (worker, request_path) = prepare_job_dir(&dir, s, p, &request, &plan)?;
    let command = worker_command(backend, &worker, &request_path)?;
    // systemd-run waits for the start transaction while holding the start/cancel lock.
    let mut unit = systemd_command(&dir, &plan, &worker, &request_path, &command)?;
    record_owner(&dir, &plan)?;
    if let Some(error) = submission_error(unit.output()) {
        write_failure_receipt(&dir, s, &request, &plan.id, &error)?;
        return Err(EngineError::ExternalTool("systemd-run".into(), error));
    }
    Ok(
        json!({"ok":true,"job":plan.id,"state":"submitted","directory":dir,"memory_limit_mb":plan.memory,"timeout_seconds":plan.seconds,"limits":request["limits"],"submitted_at":request["submitted_at"]}),
    )
}

fn systemd_quote(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    )
}
/// Job directories this session may report: none without a project, and only real job folders.
fn project_job_dirs(s: &Session) -> Result<Vec<PathBuf>> {
    if s.project_path.is_none() {
        return Ok(Vec::new());
    }
    let dir = root(s)?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.join("job.json").is_file() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

/// A job without a project field belongs to whoever asks; one with a project must match.
fn same_project(s: &Session, value: &Value) -> bool {
    !value["project"].is_string() || value["project"] == json!(s.project_path)
}

pub fn cmd_animation_list(s: &mut Session, _: &Value) -> Result<Value> {
    let owner = worker_owner(&service());
    let mut jobs = Vec::new();
    for dir in project_job_dirs(s)? {
        let value = job_details(&dir)?;
        if same_project(s, &value) {
            jobs.push(value);
        }
    }
    jobs.sort_by(|a, b| b["created_ns"].as_str().cmp(&a["created_ns"].as_str()));
    let mut result = json!({"jobs":jobs});
    add_owner(&mut result, &owner);
    Ok(result)
}

/// The unit is gone but left a terminal receipt: report that receipt, flagged as one.
fn terminal_receipt(
    dir: &Path,
    id: &str,
    details: &Value,
    owner: &Value,
    error: crate::EngineError,
) -> Result<Value> {
    let result = dir.join("result.json");
    if result.is_file() {
        let mut receipt = read_json(&result)?;
        if matches!(
            receipt["state"].as_str(),
            Some("completed" | "cancelled" | "failed")
        ) {
            receipt["job"] = json!(id);
            receipt["state_source"] = json!("terminal_receipt");
            receipt["telemetry_source"] = json!("terminal_receipt");
            receipt["telemetry_stale"] = json!(true);
            receipt["service_state"] = Value::Null;
            receipt["service_error"] = json!(error.to_string());
            add_job_details(&mut receipt, details);
            add_owner(&mut receipt, owner);
            return Ok(receipt);
        }
    }
    Err(error)
}

/// The unit belongs to this job when its description is the job id and the invocation agrees.
fn service_matches(service: &Value, id: &str, invocation: Option<&Value>) -> bool {
    service["Description"] == id
        && invocation.is_none_or(|i| i["invocation_id"] == service["InvocationID"])
}

/// The live report for a running unit, including its progress telemetry.
fn running_value(
    s: &Session,
    id: &str,
    dir: &Path,
    service: &Value,
    details: &Value,
    owner: &Value,
) -> Value {
    let progress = read_json(&dir.join("progress.json")).ok();
    let mut value = json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,"invocation_id":service["InvocationID"],"updated_at":now(),
        "state":if dir.join("cancel-requested").exists() || service["ActiveState"]=="deactivating"{"cancelling"}else{"running"},
        "phase":progress.as_ref().map(|v|&v["state"]),"frames_completed":progress.as_ref().map(|v|&v["completed"]),"frames_total":progress.as_ref().map(|v|&v["total"]),
        "progress":progress,"service_state":service["ActiveState"],"main_pid":number(service,"MainPID"),"telemetry":telemetry(service),
        "service_result":service["Result"],"exec_main_status":number(service,"ExecMainStatus"),"termination_reason":Value::Null,"log":dir.join("worker.log")});
    add_job_details(&mut value, details);
    let started = started_at(service);
    if !started.is_null() {
        value["started_at"] = started;
    }
    add_owner(&mut value, owner);
    value
}

fn receipt_value(id: &str, dir: &Path, details: &Value, owner: &Value) -> Result<Value> {
    let mut value = read_json(&dir.join("result.json"))?;
    value["job"] = json!(id);
    add_job_details(&mut value, details);
    add_owner(&mut value, owner);
    Ok(value)
}

/// Neither running nor receipted: say so instead of guessing success or usage.
fn missing_receipt(
    s: &Session,
    id: &str,
    dir: &Path,
    invocation: Option<Value>,
    details: &Value,
    owner: &Value,
) -> Value {
    let mut value = json!({"job":id,"job_id":id,"project":s.project_path,"unit":UNIT,"invocation_id":invocation.map(|v|v["invocation_id"].clone()),
        "updated_at":now(),"state":"failed","termination_reason":"missing_receipt","telemetry":telemetry(&json!({})),
        "detail":"Worker is no longer active and left no terminal receipt; success and resource usage are unknown","log":dir.join("worker.log")});
    add_job_details(&mut value, details);
    add_owner(&mut value, owner);
    value
}

pub fn cmd_animation_status(s: &mut Session, p: &Value) -> Result<Value> {
    let (id, dir) = job(s, p)?;
    let details = job_details(&dir)?;
    let observed = service();
    let owner = worker_owner(&observed);
    let service = match observed {
        Ok(service) => service,
        Err(error) => return terminal_receipt(&dir, &id, &details, &owner, error),
    };
    let invocation = read_json(&dir.join("invocation.json")).ok();
    if service_matches(&service, &id, invocation.as_ref()) && occupied(&service) {
        return Ok(running_value(s, &id, &dir, &service, &details, &owner));
    }
    if dir.join("result.json").is_file() {
        return receipt_value(&id, &dir, &details, &owner);
    }
    Ok(missing_receipt(s, &id, &dir, invocation, &details, &owner))
}

pub fn cmd_animation_cancel(s: &mut Session, p: &Value) -> Result<Value> {
    let _slot = worker_lock()?;
    let (id, dir) = job(s, p)?;
    let active = service()?;
    if active["Description"] != id || !occupied(&active) {
        return cmd_animation_status(s, p);
    }
    let requested_at = now();
    write_json(
        &dir.join("cancel-requested"),
        &json!({"updated_at":requested_at,"cancellation_requested_at":requested_at}),
    )?;
    let output = Command::new("systemctl")
        .args(["--user", "stop", "--no-block", UNIT])
        .output()?;
    if !output.status.success() {
        fs::remove_file(dir.join("cancel-requested"))?;
        return Err(EngineError::ExternalTool(
            "systemctl".into(),
            String::from_utf8_lossy(&output.stderr).into(),
        ));
    }
    cmd_animation_status(s, p)
}
