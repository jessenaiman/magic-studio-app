//! Guarded, project-local replacement of one rig layer image.
//!
//! The candidate and prior image are retained under `.part-image-history`; the
//! rig document is never normalized or rewritten by this operation.

use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn safe_id(value: &str) -> String {
    crate::workspace::sanitize_asset_id(value)
}

fn project_root(s: &Session) -> Result<PathBuf> {
    let path = s
        .project_path
        .as_deref()
        .ok_or_else(|| invalid("Open a project before replacing a part image"))?;
    let root = path
        .parent()
        .ok_or_else(|| invalid("Project has no containing directory"))?;
    std::fs::canonicalize(root).map_err(EngineError::from)
}

fn resolve_project_file(root: &Path, value: &str, label: &str) -> Result<PathBuf> {
    let raw = PathBuf::from(value);
    let path = if raw.is_absolute() {
        raw
    } else {
        root.join(raw)
    };
    let canonical = std::fs::canonicalize(&path)
        .map_err(|e| invalid(format!("Cannot resolve {label} {}: {e}", path.display())))?;
    if !canonical.starts_with(root) {
        return Err(invalid(format!("{label} resolves outside the project")));
    }
    let meta = std::fs::symlink_metadata(&canonical)?;
    if !meta.file_type().is_file() {
        return Err(invalid(format!("{label} must be a regular file")));
    }
    Ok(canonical)
}

pub(crate) fn layer_file(plan: &Value, spec: &Path, part: &str) -> Result<PathBuf> {
    let base = spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?;
    if let Some(layers) = plan.get("layers").and_then(Value::as_object) {
        if !layers.contains_key(part) {
            return Err(invalid("Selected part is not in this rig"));
        }
        return Ok(base.join(format!("{part}.png")));
    }
    Err(invalid(
        "Image replacement currently supports keyed rigs with one PNG per stable part ID; other rig formats are read-only",
    ))
}

pub(crate) fn image_edit_capability(s: &Session, spec: &Path, raw: &Value, part: &str) -> Value {
    let checked: Result<()> = (|| {
        let root = project_root(s)?;
        let spec = resolve_project_file(&root, &spec.to_string_lossy(), "Rig manifest")?;
        let source = layer_file(raw, &spec, part)?;
        resolve_project_file(&root, &source.to_string_lossy(), "part image")?;
        Ok(())
    })();
    match checked {
        Ok(()) => json!({"supported":true,"source":"part image edit preflight"}),
        Err(error) => {
            json!({"supported":false,"source":"part image edit preflight","reason":error.to_string()})
        }
    }
}

/// The rig the session holds for this asset.
fn asset_rig<'a>(
    s: &'a Session,
    asset: &str,
) -> Result<&'a crate::session::LoadedCutoutRigPlan> {
    s.assets
        .get(asset)
        .and_then(|a| a.cutout_rig.as_ref())
        .ok_or_else(|| invalid("Load a layered rig first"))
}

/// The per-asset part-image history beside the rig manifest.
fn history_dir(spec: &Path, asset: &str) -> Result<PathBuf> {
    Ok(spec
        .parent()
        .ok_or_else(|| invalid("Rig has no parent directory"))?
        .join(".part-image-history")
        .join(safe_id(asset)))
}

fn canvas_of(raw: &Value) -> Result<[u32; 2]> {
    let values = raw["canvas"]
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| invalid("Keyed rig has no explicit canvas"))?;
    let canvas = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|n| *n > 0 && *n <= 8192)
                .map(|n| n as u32)
                .ok_or_else(|| invalid("Keyed rig canvas is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok([canvas[0], canvas[1]])
}

/// A layer's box as the rig stores it: [x, y, width, height].
fn layer_box(layer: &Value) -> Result<[u32; 4]> {
    let values = layer["box"]
        .as_array()
        .filter(|values| values.len() == 4)
        .ok_or_else(|| invalid("Keyed rig layer has no explicit box"))?;
    let b = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|n| *n <= 8192)
                .map(|n| n as u32)
                .ok_or_else(|| invalid("Keyed rig layer box is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    if b[2] == 0 || b[3] == 0 {
        return Err(invalid("Keyed rig layer box is outside the canvas"));
    }
    Ok([b[0], b[1], b[2], b[3]])
}

fn layer_pivot(layer: &Value) -> Result<[f32; 2]> {
    let values = layer["pivot"]
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or_else(|| invalid("Keyed rig layer has no explicit pivot"))?;
    let p = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|n| n.is_finite())
                .map(|n| n as f32)
                .ok_or_else(|| invalid("Keyed rig layer pivot is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok([p[0], p[1]])
}

/// One layer's box and pivot agree with the loaded geometry, inside the canvas.
fn check_layer(
    plan: &crate::session::LoadedCutoutRigPlan,
    layers: &serde_json::Map<String, Value>,
    id: &str,
    canvas: [u32; 2],
    index: usize,
) -> Result<()> {
    let layer = layers
        .get(id)
        .ok_or_else(|| invalid("Invalid keyed rig layer binding"))?;
    let b = layer_box(layer)?;
    if b[0] + b[2] > canvas[0] || b[1] + b[3] > canvas[1] {
        return Err(invalid("Keyed rig layer box is outside the canvas"));
    }
    let p = layer_pivot(layer)?;
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > canvas[0] as f32 || p[1] > canvas[1] as f32 {
        return Err(invalid("Keyed rig layer pivot lies outside the canvas"));
    }
    let part = &plan.parts[index];
    let expected_box = [b[0], b[1], b[0] + b[2], b[1] + b[3]];
    if part.id != id || part.crop_box != expected_box || part.pivot != [p[0], p[1]] {
        return Err(invalid(
            "Loaded rig geometry is stale; reload the asset before replacing",
        ));
    }
    Ok(())
}

fn keyed_geometry_matches(plan: &crate::session::LoadedCutoutRigPlan, raw: &Value) -> Result<()> {
    let layers = raw
        .get("layers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            invalid(
                "Image replacement currently supports keyed rigs with explicit boxes and pivots",
            )
        })?;
    let order = raw["layer_order"]
        .as_array()
        .ok_or_else(|| invalid("Keyed rig has no layer_order"))?;
    if order.len() != plan.parts.len() {
        return Err(invalid(
            "Loaded rig geometry is stale; reload the asset before replacing",
        ));
    }
    let canvas = canvas_of(raw)?;
    if canvas != plan.canvas_size {
        return Err(invalid(
            "Loaded rig canvas is stale; reload the asset before replacing",
        ));
    }
    for (index, id) in order.iter().enumerate() {
        let id = id
            .as_str()
            .ok_or_else(|| invalid("Invalid keyed rig layer ID"))?;
        check_layer(plan, layers, id, canvas, index)?;
    }
    Ok(())
}

fn checked_dir(root: &Path, path: &Path, create: bool) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid("Part-image history resolves outside the project"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid("Invalid history path"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("Part-image history cannot use symlinked paths"));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(invalid("Part-image history path contains a non-directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
                std::fs::create_dir(&current)?
            }
            Err(error) => return Err(EngineError::from(error)),
        }
    }
    Ok(())
}

fn preflight_cache_path(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid("Rig preview cache resolves outside the project"));
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| invalid("Invalid preview cache path"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("Rig preview cache cannot use symlinked paths"));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(invalid("Rig preview cache path contains a non-directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(EngineError::from(error)),
        }
    }
    Ok(())
}

fn preflight_cache_entries(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let kind = entry?.file_type()?;
        if !kind.is_file() || kind.is_symlink() {
            return Err(invalid(
                "Rig preview cache contains a non-regular file; refusing to load",
            ));
        }
    }
    Ok(())
}

fn check_regular_or_missing(root: &Path, path: &Path) -> Result<()> {
    if !path.starts_with(root) {
        return Err(invalid(
            "Part-image history file resolves outside the project",
        ));
    }
    if let Some(parent) = path.parent() {
        checked_dir(root, parent, false)?;
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(invalid("Part-image history cannot use symlinked files"))
        }
        Ok(meta) if !meta.is_file() => {
            Err(invalid("Part-image history entry must be a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(EngineError::from(error)),
    }
}

type KeyedCacheSnapshot = Vec<(PathBuf, Vec<u8>)>;
fn snapshot_keyed_cache(root: &Path, spec: &Path) -> Result<KeyedCacheSnapshot> {
    let Some(base) = spec.parent() else {
        return Err(invalid("Rig has no parent directory"));
    };
    let dir = base.join(".layered_preview_cache/keyed");
    if !dir.exists() {
        checked_dir(root, &dir, true)?;
    } else {
        checked_dir(root, &dir, false)?;
    }
    let mut snapshot = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let meta = entry.file_type()?;
        if !meta.is_file() {
            return Err(invalid(
                "Keyed preview cache contains a non-file entry; refusing replacement",
            ));
        }
        snapshot.push((entry.path(), std::fs::read(entry.path())?));
    }
    Ok(snapshot)
}
fn restore_keyed_cache(root: &Path, spec: &Path, snapshot: &KeyedCacheSnapshot) -> Result<()> {
    let Some(base) = spec.parent() else {
        return Err(invalid("Rig has no parent directory"));
    };
    let dir = base.join(".layered_preview_cache/keyed");
    if !dir.exists() {
        return Ok(());
    }
    checked_dir(root, &dir, false)?;
    let saved: std::collections::BTreeSet<_> =
        snapshot.iter().map(|(path, _)| path.clone()).collect();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !saved.contains(&path) {
            std::fs::remove_file(path)?;
        }
    }
    for (path, bytes) in snapshot {
        atomic_write(path, bytes, "cache-restore")?;
    }
    Ok(())
}

fn decoded_png(bytes: &[u8], label: &str) -> Result<(u32, u32)> {
    if bytes.len() > 50_000_000 {
        return Err(invalid(format!("{label} exceeds the 50 MB image limit")));
    }
    if image::guess_format(bytes).ok() != Some(image::ImageFormat::Png) {
        return Err(invalid(format!("{label} must be a PNG")));
    }
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| invalid(format!("{label} failed full PNG decode: {e}")))?;
    if image.width() == 0
        || image.height() == 0
        || u64::from(image.width()) * u64::from(image.height()) > 16_777_216
        || !image.color().has_alpha()
    {
        return Err(invalid(format!(
            "{label} must have nonzero dimensions and an alpha channel"
        )));
    }
    Ok((image.width(), image.height()))
}

fn atomic_write(path: &Path, bytes: &[u8], suffix: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Target file has no parent directory"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("Target file name is not UTF-8"))?;
    let stage = parent.join(format!(".{name}.{suffix}.tmp"));
    let staged = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)?;
        use std::io::Write;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = std::fs::remove_file(&stage);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&stage, path) {
        let _ = std::fs::remove_file(&stage);
        return Err(EngineError::from(error));
    }
    Ok(())
}

fn reload(s: &mut Session, asset: &str) -> Result<()> {
    crate::project::cmd_project_load_asset(s, &json!({"id":asset,"reload":true}))?;
    Ok(())
}

/// The asset this call names: the param, the active asset, else the first project asset.
fn chosen_asset(s: &Session, params: &Value) -> Result<String> {
    params
        .get("asset")
        .or_else(|| params.get("character"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| s.active_asset.clone())
        .or_else(|| s.project_assets.first().map(|asset| asset.id.clone()))
        .ok_or_else(|| invalid("Choose a project asset"))
}

/// The asset's source path, canonicalised and proven to live inside the project.
fn asset_source(s: &Session, root: &Path, id: &str) -> Result<PathBuf> {
    let entry = s.project_assets.iter().find(|asset| asset.id == id);
    let source = entry
        .and_then(|asset| asset.path.as_ref())
        .cloned()
        .or_else(|| s.assets.get(id).map(|asset| asset.path.clone()))
        .ok_or_else(|| invalid("Selected project asset has no source path"))?;
    let source = if source.is_absolute() {
        source
    } else {
        root.join(source)
    };
    let source = std::fs::canonicalize(source).map_err(EngineError::from)?;
    if !source.starts_with(root) {
        return Err(invalid(
            "Selected project asset resolves outside the project",
        ));
    }
    Ok(source)
}

fn is_rig_asset(s: &Session, id: &str) -> bool {
    s.project_assets
        .iter()
        .find(|asset| asset.id == id)
        .is_some_and(|asset| asset.kind == "cutout_rig")
}

/// A rig manifest must be keyed, project-local and free of stale derived caches.
fn preflight_manifest(root: &Path, manifest: &Path, is_rig: bool) -> Result<()> {
    let manifest = std::fs::canonicalize(manifest).map_err(EngineError::from)?;
    if !manifest.starts_with(root) || !manifest.is_file() {
        return Err(invalid("Rig manifest must be a regular project-local file"));
    }
    let raw: Value = serde_json::from_slice(&std::fs::read(&manifest)?)
        .map_err(|error| invalid(format!("Invalid project asset JSON: {error}")))?;
    let rig_like = is_rig
        || raw.get("rig_type").is_some()
        || (raw.get("layers").is_some() && raw.get("canvas").is_some());
    if rig_like && raw.get("layers").and_then(Value::as_object).is_none() {
        return Err(invalid(
            "Image replacement currently supports keyed rigs with one PNG per stable part ID; array/component rigs remain read-only",
        ));
    }
    let parent = manifest
        .parent()
        .ok_or_else(|| invalid("Rig manifest has no parent"))?;
    let cache = parent.join(".layered_preview_cache/keyed");
    preflight_cache_path(root, &cache)?;
    preflight_cache_entries(&cache)
}

fn asset_id(s: &mut Session, params: &Value) -> Result<String> {
    // Loading a keyed rig writes derived PNG caches beside its manifest.
    let root = project_root(s)?;
    let id = chosen_asset(s, params)?;
    let source = asset_source(s, &root, &id)?;
    let manifest = if source.is_dir() {
        source.join("manifest.json")
    } else {
        source.clone()
    };
    let is_rig = is_rig_asset(s, &id);
    if is_rig
        || manifest
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        preflight_manifest(&root, &manifest, is_rig)?;
    }
    crate::project::cmd_project_load_asset(s, &json!({"id":id}))?;
    Ok(id)
}

fn rig_snapshot(s: &Session, asset: &str) -> Result<(PathBuf, Value, String)> {
    let rig = s
        .assets
        .get(asset)
        .and_then(|a| a.cutout_rig.as_ref())
        .ok_or_else(|| invalid("Load a layered rig first"))?;
    let spec = std::fs::canonicalize(&rig.plan_path)?;
    let root = project_root(s)?;
    if !spec.starts_with(&root) {
        return Err(invalid("Rig manifest resolves outside the project"));
    }
    let bytes = std::fs::read(&spec)?;
    let raw: Value =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("Invalid rig JSON: {e}")))?;
    Ok((spec, raw, sha(&bytes)))
}

fn state_path(history: &Path) -> PathBuf {
    history.join("state.json")
}

fn read_state(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({"revision":0,"last_token":null}));
    }
    let state: Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| invalid(format!("Invalid part-image history state: {e}")))?;
    let revision = state
        .get("revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid("Invalid part-image history state: revision must be a nonnegative integer")
        })?;
    let token_valid = state
        .get("last_token")
        .is_some_and(|token| token.is_null() || token.as_str().is_some_and(|v| !v.is_empty()));
    if !token_valid {
        return Err(invalid(
            "Invalid part-image history state: last_token must be null or a nonempty token",
        ));
    }
    if revision == 0 && !state["last_token"].is_null() {
        return Err(invalid(
            "Invalid part-image history state: revision and last_token disagree",
        ));
    }
    Ok(state)
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| invalid(e.to_string()))?;
    atomic_write(path, &bytes, "metadata")
}

fn requested_part(params: &Value) -> Result<&str> {
    params["part"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("Choose a stable part ID"))
}

fn check_rig_hash(params: &Value, rig_hash: &str) -> Result<()> {
    if params["expected_rig_sha256"].as_str() != Some(rig_hash) {
        return Err(invalid(
            "Rig changed since this candidate was reviewed; reload and compare again",
        ));
    }
    Ok(())
}

fn require_part(plan: &crate::session::LoadedCutoutRigPlan, part: &str) -> Result<()> {
    if plan.parts.iter().any(|p| p.id == part) {
        return Ok(());
    }
    Err(invalid("Selected part is not in the loaded rig"))
}

/// The part's bound PNG, resolved inside the project without judging the binding.
fn bound_target(root: &Path, raw: &Value, spec: &Path, part: &str) -> Result<PathBuf> {
    let bound = layer_file(raw, spec, part)?;
    resolve_project_file(
        root,
        bound
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )
}

/// The part's bound PNG, refusing a symlinked binding and resolving it inside the project.
fn part_target(root: &Path, raw: &Value, spec: &Path, part: &str) -> Result<PathBuf> {
    let bound = layer_file(raw, spec, part)?;
    if std::fs::symlink_metadata(&bound)?.file_type().is_symlink() {
        return Err(invalid(
            "Symlink-bound part images are read-only; use a regular project PNG",
        ));
    }
    resolve_project_file(
        root,
        bound
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )
}

fn candidate_path(root: &Path, params: &Value, target: &Path) -> Result<PathBuf> {
    let raw = params["candidate"]
        .as_str()
        .ok_or_else(|| invalid("Choose a generated candidate"))?;
    let candidate = resolve_project_file(root, raw, "candidate")?;
    if candidate == target {
        return Err(invalid("Candidate must be a separate project-local image"));
    }
    Ok(candidate)
}

fn symlink_refusal(undo: bool) -> &'static str {
    if undo {
        "Symlink-bound part images are read-only; undo refused"
    } else {
        "Symlink-bound part images are read-only; use regular project PNGs"
    }
}

fn shared_refusal(undo: bool) -> &'static str {
    if undo {
        "Part image is shared with another rig part; undo refused"
    } else {
        "Selected image is shared with another rig part; replacement is unsafe"
    }
}

/// One layer's binding: not a symlink, not another part's copy of the target, not the candidate.
fn check_binding(
    root: &Path,
    base: &Path,
    layer: &str,
    part: &str,
    target: &Path,
    candidate: Option<&Path>,
    undo: bool,
) -> Result<()> {
    let binding = base.join(format!("{layer}.png"));
    if std::fs::symlink_metadata(&binding)?
        .file_type()
        .is_symlink()
    {
        return Err(invalid(symlink_refusal(undo)));
    }
    let other = resolve_project_file(root, &binding.to_string_lossy(), "part image")?;
    if layer != part && other == target {
        return Err(invalid(shared_refusal(undo)));
    }
    if candidate == Some(other.as_path()) {
        return Err(invalid(
            "Candidate cannot be another rig part's bound image",
        ));
    }
    Ok(())
}

/// No other rig part may be bound to the target, nor to the candidate being installed.
fn ensure_unshared(
    root: &Path,
    spec: &Path,
    raw: &Value,
    part: &str,
    target: &Path,
    candidate: Option<&Path>,
    undo: bool,
) -> Result<()> {
    let base = spec
        .parent()
        .ok_or_else(|| invalid("Part specification has no parent directory"))?;
    for layer in raw["layers"]
        .as_object()
        .into_iter()
        .flat_map(|layers| layers.keys())
    {
        check_binding(root, base, layer, part, target, candidate, undo)?;
    }
    Ok(())
}

fn verify_hashes(params: &Value, old_hash: &str, candidate_hash: &str) -> Result<()> {
    if params["expected_part_sha256"].as_str() != Some(old_hash) {
        return Err(invalid(
            "Part image changed since comparison; reload before replacing",
        ));
    }
    if params["expected_candidate_sha256"].as_str() != Some(candidate_hash) {
        return Err(invalid(
            "Candidate image changed since comparison; compare it again",
        ));
    }
    Ok(())
}

/// Replacement keeps the existing box, so the candidate must share the part's size.
fn checked_size(target_bytes: &[u8], candidate_bytes: &[u8]) -> Result<(u32, u32)> {
    let old_size = decoded_png(target_bytes, "Current part image")?;
    let new_size = decoded_png(candidate_bytes, "Candidate")?;
    if old_size != new_size {
        return Err(invalid(format!(
            "Candidate is {}×{} but this part requires {}×{}; replacement keeps the existing box and does not resize artwork",
            new_size.0, new_size.1, old_size.0, old_size.1
        )));
    }
    Ok(old_size)
}

fn next_revision(state: &Value) -> Result<u64> {
    let revision = state["revision"].as_u64().unwrap_or(0);
    revision
        .checked_add(1)
        .filter(|_| revision.checked_add(2).is_some())
        .ok_or_else(|| {
            invalid("Part-image history revision is exhausted; replacement cannot be undone safely")
        })
}

fn check_revision(params: &Value, revision: u64) -> Result<()> {
    if params["expected_revision"].as_u64() != Some(revision) {
        return Err(invalid(
            "Part-image history changed since comparison; reload the asset",
        ));
    }
    Ok(())
}

/// Undo the write and the derived cache together, keeping the caller's error if all succeed.
#[allow(clippy::too_many_arguments)]
fn rollback(
    s: &mut Session,
    asset: &str,
    target: &Path,
    restore: &[u8],
    tag: &str,
    root: &Path,
    spec: &Path,
    cache_before: &KeyedCacheSnapshot,
    error: EngineError,
    reload_asset: bool,
) -> EngineError {
    let rollback = atomic_write(target, restore, tag);
    if reload_asset {
        let _ = reload(s, asset);
    }
    let cache_rollback = restore_keyed_cache(root, spec, cache_before);
    rollback
        .err()
        .or_else(|| cache_rollback.err())
        .unwrap_or(error)
}

/// The request's own identity and the paths it may write, before any image is read.
struct Guard {
    asset: String,
    part: String,
    root: PathBuf,
    spec: PathBuf,
    rig_hash: String,
    target: PathBuf,
    candidate: PathBuf,
}

fn replacement_guard(s: &mut Session, params: &Value) -> Result<Guard> {
    let asset = asset_id(s, params)?;
    let part = requested_part(params)?.to_owned();
    let root = project_root(s)?;
    let (spec, raw, rig_hash) = rig_snapshot(s, &asset)?;
    check_rig_hash(params, &rig_hash)?;
    let plan = asset_rig(s, &asset)?;
    keyed_geometry_matches(plan, &raw)?;
    require_part(plan, &part)?;
    let target = part_target(&root, &raw, &spec, &part)?;
    let candidate = candidate_path(&root, params, &target)?;
    ensure_unshared(&root, &spec, &raw, &part, &target, Some(&candidate), false)?;
    Ok(Guard {
        asset,
        part,
        root,
        spec,
        rig_hash,
        target,
        candidate,
    })
}

/// The two images the call compares, with the digests and size the request pinned.
struct Images {
    target: Vec<u8>,
    candidate: Vec<u8>,
    old_hash: String,
    candidate_hash: String,
    size: (u32, u32),
}

fn verified_images(guard: &Guard, params: &Value) -> Result<Images> {
    let target = std::fs::read(&guard.target)?;
    let candidate = std::fs::read(&guard.candidate)?;
    let old_hash = sha(&target);
    let candidate_hash = sha(&candidate);
    verify_hashes(params, &old_hash, &candidate_hash)?;
    let size = checked_size(&target, &candidate)?;
    Ok(Images {
        target,
        candidate,
        old_hash,
        candidate_hash,
        size,
    })
}

/// The history directory, its state file, the parsed state and the revision this call may write.
fn history_state(guard: &Guard, params: &Value) -> Result<(PathBuf, PathBuf, Value, u64)> {
    let history = history_dir(&guard.spec, &guard.asset)?;
    checked_dir(
        &guard.root,
        history
            .parent()
            .ok_or_else(|| invalid("Invalid history directory"))?,
        true,
    )?;
    checked_dir(&guard.root, &history, true)?;
    let state_file = state_path(&history);
    check_regular_or_missing(&guard.root, &state_file)?;
    let state = read_state(&state_file)?;
    let revision = state["revision"].as_u64().unwrap_or(0);
    let next_revision = next_revision(&state)?;
    check_revision(params, revision)?;
    Ok((history, state_file, state, next_revision))
}

/// One replacement attempt's files, all inside the asset's history directory.
struct Operation {
    token: String,
    backup: PathBuf,
    candidate: PathBuf,
    receipt: PathBuf,
}

fn begin_operation(
    guard: &Guard,
    images: &Images,
    history: &Path,
) -> Result<Operation> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| invalid(e.to_string()))?
        .as_nanos();
    let token = format!("{}-{stamp}-{}", safe_id(&guard.asset), std::process::id());
    checked_dir(&guard.root, &history.join(&token), true)?;
    let operation = Operation {
        backup: history.join(&token).join("before.png"),
        candidate: history.join(&token).join("candidate.png"),
        receipt: history.join(&token).join("receipt.json"),
        token,
    };
    for file in [&operation.backup, &operation.candidate, &operation.receipt] {
        check_regular_or_missing(&guard.root, file)?;
    }
    std::fs::write(&operation.backup, &images.target)?;
    std::fs::write(&operation.candidate, &images.candidate)?;
    Ok(operation)
}

/// Install the candidate; a failed reload puts the previous bytes and cache back.
fn apply_candidate(
    s: &mut Session,
    guard: &Guard,
    images: &Images,
    cache_before: &KeyedCacheSnapshot,
    operation: &Operation,
) -> Result<()> {
    atomic_write(&guard.target, &images.candidate, &operation.token)?;
    if let Err(error) = reload(s, &guard.asset) {
        return Err(rollback(
            s,
            &guard.asset,
            &guard.target,
            &images.target,
            &format!("rollback-{}", operation.token),
            &guard.root,
            &guard.spec,
            cache_before,
            error,
            false,
        ));
    }
    Ok(())
}

/// Write one record, undoing the image and cache if the write cannot stand.
#[allow(clippy::too_many_arguments)]
fn write_or_rollback(
    s: &mut Session,
    guard: &Guard,
    images: &Images,
    cache_before: &KeyedCacheSnapshot,
    operation: &Operation,
    path: &Path,
    value: &Value,
) -> Result<()> {
    if let Err(error) = write_json(path, value) {
        return Err(rollback(
            s,
            &guard.asset,
            &guard.target,
            &images.target,
            &format!("rollback-{}", operation.token),
            &guard.root,
            &guard.spec,
            cache_before,
            error,
            true,
        ));
    }
    Ok(())
}

/// Replace one part image. Required parameters: asset, part, candidate,
/// expected_rig_sha256, expected_part_sha256, expected_candidate_sha256,
/// and expected_revision (0 for an asset without prior replacements).
pub fn cmd_part_replace_image(s: &mut Session, params: &Value) -> Result<Value> {
    let guard = replacement_guard(s, params)?;
    let images = verified_images(&guard, params)?;
    let (history, state_file, mut state, next_revision) = history_state(&guard, params)?;
    let cache_before = snapshot_keyed_cache(&guard.root, &guard.spec)?;
    let operation = begin_operation(&guard, &images, &history)?;
    apply_candidate(s, &guard, &images, &cache_before, &operation)?;
    let record = json!({"asset":guard.asset,"part":guard.part,"token":operation.token,"target":guard.target,"backup":operation.backup,"candidate":operation.candidate,
        "before_sha256":images.old_hash,"candidate_sha256":images.candidate_hash,"rig_sha256":guard.rig_hash,"revision_before":state["revision"].as_u64().unwrap_or(0),
        "revision_after":next_revision,"status":"applied","width":images.size.0,"height":images.size.1});
    write_or_rollback(s, &guard, &images, &cache_before, &operation, &operation.receipt, &record)?;
    state["revision"] = json!(next_revision);
    state["last_token"] = json!(operation.token);
    write_or_rollback(s, &guard, &images, &cache_before, &operation, &state_file, &state)?;
    Ok(
        json!({"ok":true,"asset":guard.asset,"part":guard.part,"undo":operation.token,"revision":next_revision,"before_sha256":images.old_hash,"candidate_sha256":images.candidate_hash,"width":images.size.0,"height":images.size.1}),
    )
}

/// The undo token, refused when it tries to leave the history directory.
fn undo_token(params: &Value) -> Result<&str> {
    let token = params["undo"]
        .as_str()
        .ok_or_else(|| invalid("Missing part-image undo token"))?;
    if token.contains('/') || token.contains('\\') || token.contains("..") {
        return Err(invalid("Invalid part-image undo token"));
    }
    Ok(token)
}

/// The receipt this token names, proven to belong to this asset and still un-consumed.
fn undo_record(
    root: &Path,
    history: &Path,
    asset: &str,
    token: &str,
) -> Result<(Value, PathBuf, Value, PathBuf)> {
    checked_dir(root, history, false)?;
    let state_file = state_path(history);
    check_regular_or_missing(root, &state_file)?;
    let state = read_state(&state_file)?;
    let operation = history.join(token);
    checked_dir(root, &operation, false)?;
    let receipt_path = operation.join("receipt.json");
    check_regular_or_missing(root, &receipt_path)?;
    let record: Value = serde_json::from_slice(
        &std::fs::read(&receipt_path)
            .map_err(|e| invalid(format!("Unknown or unavailable undo token: {e}")))?,
    )
    .map_err(|e| invalid(format!("Invalid undo receipt: {e}")))?;
    if record["asset"].as_str() != Some(asset) || record["token"].as_str() != Some(token) {
        return Err(invalid("Undo token belongs to another asset"));
    }
    if record["status"] != "applied" {
        return Err(invalid("This part-image undo token was already consumed"));
    }
    if state["last_token"].as_str() != Some(token) || state["revision"] != record["revision_after"]
    {
        return Err(invalid(
            "A later part-image operation exists; this undo token is stale",
        ));
    }
    Ok((record, receipt_path, state, state_file))
}

/// The part the receipt names, its bound target and the bytes now on disk.
fn undo_target(
    root: &Path,
    raw: &Value,
    spec: &Path,
    record: &Value,
    rig_hash: &str,
    params: &Value,
) -> Result<(String, PathBuf, Vec<u8>)> {
    // A symlinked part is refused before the rig-hash comparison: the hash covers layer bytes, so a
    // changed alias would otherwise report "rig changed" and hide the real, unsafe binding.
    let part = record["part"]
        .as_str()
        .ok_or_else(|| invalid("Undo receipt has no part ID"))?
        .to_owned();
    let bound_target = layer_file(raw, spec, &part)?;
    if std::fs::symlink_metadata(&bound_target)?
        .file_type()
        .is_symlink()
    {
        return Err(invalid(
            "Symlink-bound part images are read-only; undo refused",
        ));
    }
    if params
        .get("expected_rig_sha256")
        .and_then(Value::as_str)
        .is_some_and(|expected| expected != rig_hash)
        || record["rig_sha256"].as_str() != Some(rig_hash)
    {
        return Err(invalid(
            "Rig changed since replacement; reload and review before undo",
        ));
    }
    let target = resolve_project_file(
        root,
        bound_target
            .to_str()
            .ok_or_else(|| invalid("Layer path is not UTF-8"))?,
        "part image",
    )?;
    if record["target"].as_str() != target.to_str() {
        return Err(invalid(
            "Part image binding changed since replacement; undo refused",
        ));
    }
    let current = std::fs::read(&target)?;
    if sha(&current) != record["candidate_sha256"].as_str().unwrap_or_default() {
        return Err(invalid(
            "Part image changed after replacement; undo would overwrite newer work",
        ));
    }
    Ok((part, target, current))
}

/// The recorded backup, verified against the digest the receipt kept.
fn undo_original(root: &Path, record: &Value) -> Result<Vec<u8>> {
    let backup = PathBuf::from(
        record["backup"]
            .as_str()
            .ok_or_else(|| invalid("Undo receipt has no original image"))?,
    );
    let backup = resolve_project_file(
        root,
        backup
            .to_str()
            .ok_or_else(|| invalid("Backup path is not UTF-8"))?,
        "original part backup",
    )?;
    let original = std::fs::read(&backup)?;
    if sha(&original) != record["before_sha256"].as_str().unwrap_or_default() {
        return Err(invalid(
            "Original part backup failed integrity verification",
        ));
    }
    Ok(original)
}

/// The asset, token and rig this undo names, before any history is read.
struct UndoGuard {
    asset: String,
    token: String,
    root: PathBuf,
    spec: PathBuf,
    raw: Value,
    rig_hash: String,
    history: PathBuf,
}

fn undo_guard(s: &mut Session, params: &Value) -> Result<UndoGuard> {
    let asset = asset_id(s, params)?;
    let token = undo_token(params)?.to_owned();
    let (spec, raw, rig_hash) = rig_snapshot(s, &asset)?;
    keyed_geometry_matches(asset_rig(s, &asset)?, &raw)?;
    let root = project_root(s)?;
    let history = history_dir(&spec, &asset)?;
    Ok(UndoGuard {
        asset,
        token,
        root,
        spec,
        raw,
        rig_hash,
        history,
    })
}

/// The receipt, history state and bound image this undo would restore.
struct UndoHistory {
    record: Value,
    receipt_path: PathBuf,
    state: Value,
    state_file: PathBuf,
    part: String,
    target: PathBuf,
    current: Vec<u8>,
}

fn undo_history(guard: &UndoGuard, params: &Value) -> Result<UndoHistory> {
    let (record, receipt_path, state, state_file) =
        undo_record(&guard.root, &guard.history, &guard.asset, &guard.token)?;
    let (part, target, current) = undo_target(
        &guard.root,
        &guard.raw,
        &guard.spec,
        &record,
        &guard.rig_hash,
        params,
    )?;
    ensure_unshared(&guard.root, &guard.spec, &guard.raw, &part, &target, None, true)?;
    Ok(UndoHistory {
        record,
        receipt_path,
        state,
        state_file,
        part,
        target,
        current,
    })
}

/// Undo only the latest part-image replacement for this asset. The token is
/// single-use and guarded by the project-local revision plus current image hash.
pub fn cmd_part_undo_image(s: &mut Session, params: &Value) -> Result<Value> {
    let guard = undo_guard(s, params)?;
    let mut plan = undo_history(&guard, params)?;
    let original = undo_original(&guard.root, &plan.record)?;
    let revision = plan.state["revision"].as_u64().unwrap_or(0);
    let next_revision = revision.checked_add(1).ok_or_else(|| {
        invalid("Part-image history revision is exhausted; undo cannot safely advance it")
    })?;
    let cache_before = snapshot_keyed_cache(&guard.root, &guard.spec)?;
    atomic_write(&plan.target, &original, &format!("undo-{}", guard.token))?;
    if let Err(error) = reload(s, &guard.asset) {
        return Err(rollback(
            s,
            &guard.asset,
            &plan.target,
            &plan.current,
            &format!("rollback-undo-{}", guard.token),
            &guard.root,
            &guard.spec,
            &cache_before,
            error,
            false,
        ));
    }
    plan.record["status"] = json!("undone");
    plan.record["revision_undone"] = json!(next_revision);
    if let Err(error) = write_json(&plan.receipt_path, &plan.record) {
        return Err(rollback(
            s,
            &guard.asset,
            &plan.target,
            &plan.current,
            &format!("rollback-undo-{}", guard.token),
            &guard.root,
            &guard.spec,
            &cache_before,
            error,
            true,
        ));
    }
    plan.state["revision"] = json!(next_revision);
    plan.state["last_token"] = Value::Null;
    if let Err(error) = write_json(&plan.state_file, &plan.state) {
        plan.record["status"] = json!("applied");
        plan.record
            .as_object_mut()
            .map(|map| map.remove("revision_undone"));
        let _ = write_json(&plan.receipt_path, &plan.record);
        return Err(rollback(
            s,
            &guard.asset,
            &plan.target,
            &plan.current,
            &format!("rollback-undo-{}", guard.token),
            &guard.root,
            &guard.spec,
            &cache_before,
            error,
            true,
        ));
    }
    Ok(
        json!({"ok":true,"asset":guard.asset,"part":plan.part,"revision":next_revision,"restored_sha256":plan.record["before_sha256"]}),
    )
}

/// Resolve an installed Craft binary the way `resolve_craft_cli` names it: an
/// explicit path must be a file, a bare name must be on PATH.
fn installed_craft_binary(name: &str) -> Option<PathBuf> {
    let resolved = PathBuf::from(crate::workspace::resolve_craft_cli(name));
    if resolved.components().count() > 1 {
        return resolved.is_file().then_some(resolved);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(&resolved))
            .find(|path| path.is_file())
    })
}

/// The PhotoCraft app's desktop entry (`gio launch` runs whatever version it points at).
fn image_editor_entry() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    let data_dirs =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    data_home
        .into_iter()
        .chain(std::env::split_paths(&data_dirs))
        .map(|dir| dir.join("applications/ai.storyteller.photocraft.desktop"))
        .find(|path| path.is_file())
}

/// A project-local copy of `source` for PhotoCraft to edit, so saving there never
/// bypasses `part.replace_image`. An existing draft is kept: it may hold unsaved work.
fn draft_path(history: &Path, part: &str) -> PathBuf {
    history
        .join("drafts")
        .join(format!("{}.png", safe_id(part)))
}

fn png_size(bytes: &[u8]) -> Result<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()
        .map_err(|e| invalid(format!("Cannot read part image dimensions: {e}")))
}

fn edit_draft(root: &Path, history: &Path, part: &str, source: &Path) -> Result<(PathBuf, bool)> {
    checked_dir(root, &history.join("drafts"), true)?;
    let draft = draft_path(history, part);
    check_regular_or_missing(root, &draft)?;
    if draft.exists() {
        return Ok((draft, true));
    }
    std::fs::copy(source, &draft)?;
    Ok((draft, false))
}

/// The earliest receipt for this part, as (revision, backup path, digest).
fn earliest_receipt(root: &Path, history: &Path, part: &str) -> Result<Option<(u64, PathBuf, String)>> {
    let mut earliest: Option<(u64, PathBuf, String)> = None;
    for entry in std::fs::read_dir(history)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let receipt_path = entry.path().join("receipt.json");
        check_regular_or_missing(root, &receipt_path)?;
        let Ok(bytes) = std::fs::read(&receipt_path) else {
            continue;
        };
        let record: Value = serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("Invalid part-image receipt: {e}")))?;
        if record["part"].as_str() != Some(part) {
            continue;
        }
        let revision = record["revision_before"]
            .as_u64()
            .ok_or_else(|| invalid("Part-image receipt has no revision"))?;
        let backup = record["backup"]
            .as_str()
            .ok_or_else(|| invalid("Part-image receipt has no original image"))?;
        let digest = record["before_sha256"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if earliest.as_ref().is_none_or(|(r, _, _)| revision < *r) {
            earliest = Some((revision, PathBuf::from(backup), digest));
        }
    }
    Ok(earliest)
}

/// The retained original, verified against the digest its receipt recorded.
fn verified_backup(root: &Path, backup: &Path, digest: &str) -> Result<PathBuf> {
    let path = resolve_project_file(
        root,
        backup
            .to_str()
            .ok_or_else(|| invalid("Backup path is not UTF-8"))?,
        "original part backup",
    )?;
    if sha(&std::fs::read(&path)?) != digest {
        return Err(invalid(
            "Original part backup failed integrity verification",
        ));
    }
    Ok(path)
}

/// The earliest retained `before.png` for this part: the image the part had
/// before its first replacement. Verified against its receipt digest.
fn original_backup(root: &Path, history: &Path, part: &str) -> Result<Option<PathBuf>> {
    if !history.exists() {
        return Ok(None);
    }
    checked_dir(root, history, false)?;
    let Some((_, backup, digest)) = earliest_receipt(root, history, part)? else {
        return Ok(None);
    };
    Ok(Some(verified_backup(root, &backup, &digest)?))
}

/// `which` selects current or original; `spawn` asks for the editor window.
fn editor_choice(params: &Value) -> Result<(&str, bool)> {
    let which = params["which"].as_str().unwrap_or("current");
    if which != "current" && which != "original" {
        return Err(invalid("`which` must be \"current\" or \"original\""));
    }
    Ok((which, params["spawn"].as_bool().unwrap_or(false)))
}

/// Open the part's draft in the installed editor; reports whether the draft already existed.
fn spawn_editor(root: &Path, history: &Path, part: &str, file: &Path) -> Result<bool> {
    // photocraft-cli is headless; only the PhotoCraft app opens a window.
    // An explicit `photocraft` binary wins, else the installed desktop entry.
    let mut launch = match installed_craft_binary("photocraft") {
        Some(app) => std::process::Command::new(app),
        None => {
            let entry = image_editor_entry().ok_or_else(|| {
                invalid("PhotoCraft app is not installed (no `photocraft` binary or ai.storyteller.photocraft.desktop)")
            })?;
            let mut gio = std::process::Command::new("gio");
            gio.arg("launch").arg(entry);
            gio
        }
    };
    let (path, existed) = edit_draft(root, history, part, file)?;
    launch.arg(&path).spawn()?;
    Ok(existed)
}

/// A draft saved in the editor is the part's candidate until it matches the current image.
fn draft_report(root: &Path, history: &Path, part: &str, current_sha: &str) -> Result<Value> {
    let draft_file = draft_path(history, part);
    if !draft_file.exists() {
        return Ok(Value::Null);
    }
    check_regular_or_missing(root, &draft_file)?;
    let bytes = std::fs::read(&draft_file)?;
    let (width, height) = png_size(&bytes)?;
    let digest = sha(&bytes);
    Ok(json!({"file":draft_file,"reused":false,"width":width,"height":height,
        "matches_current":digest == current_sha,"sha256":digest}))
}

/// The asset, part and files this editor request names.
struct EditorRequest {
    part: String,
    which: String,
    spawn: bool,
    root: PathBuf,
    history: PathBuf,
    current: PathBuf,
    original: Option<PathBuf>,
}

fn editor_request(s: &mut Session, params: &Value) -> Result<EditorRequest> {
    let asset = asset_id(s, params)?;
    let part = requested_part(params)?.to_owned();
    let (which, spawn) = editor_choice(params)?;
    let root = project_root(s)?;
    let (spec, raw, _) = rig_snapshot(s, &asset)?;
    let current = bound_target(&root, &raw, &spec, &part)?;
    let history = history_dir(&spec, &asset)?;
    let original = original_backup(&root, &history, &part)?;
    Ok(EditorRequest {
        part,
        which: which.to_string(),
        spawn,
        root,
        history,
        current,
        original,
    })
}

/// Resolve one rig part's PNG (current, or its retained original) and report
/// what PhotoCraft would open. `spawn: true` launches the PhotoCraft app on an
/// edit draft of it; the saved draft is the candidate for `part.replace_image`.
pub fn cmd_part_open_image_editor(s: &mut Session, params: &Value) -> Result<Value> {
    let request = editor_request(s, params)?;
    let current_sha = sha(&std::fs::read(&request.current)?);
    let file = match (request.which.as_str(), &request.original) {
        ("original", Some(backup)) => backup.clone(),
        _ => request.current.clone(),
    };
    let bytes = std::fs::read(&file)?;
    let (width, height) = png_size(&bytes)?;
    let photocraft_cli = installed_craft_binary("photocraft-cli");
    let reused = if request.spawn {
        spawn_editor(&request.root, &request.history, &request.part, &file)?
    } else {
        true
    };
    let mut draft = draft_report(&request.root, &request.history, &request.part, &current_sha)?;
    if let Some(draft) = draft.as_object_mut() {
        draft.insert("reused".into(), json!(reused));
    }
    Ok(
        json!({"ok":true,"command":"part.edit_image","part":request.part,"which":request.which,"file":file,
        "original_file":request.original,"width":width,"height":height,"sha256":sha(&bytes),
        "photocraft_cli":photocraft_cli,"spawned":request.spawn,"draft":draft}),
    )
}
