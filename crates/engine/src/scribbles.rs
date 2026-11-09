//! Scribbles engine commands — thin dispatch to [`craft_scribbles`] (L4).
//!
//! Launches/focuses Google Flow's **Moppet Maker** (character animation: parts sheets, pose sheets,
//! sprite cycles) or **Storyboard & Animation Master Pack** (one-pass storyboard asset generation
//! & page recreation with `<CODE>__<slot>` ID markers), ingests downloaded packages, builds
//! animated looping SVGs + EffectCraft sprite cycles, and generates short AI video clips.

use crate::{EngineError, Result, Session};
use craft_scribbles::{
    DEFAULT_FLOW_PROJECT_URL, DEFAULT_MOPPET_MAKER_URL, DEFAULT_STORYBOARD_PACK_TOOL_URL, LOCKED_MOPPETS, StoryboardPackItem, VideoClipRequest,
    classify_animation_style,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Launch or focus Google Flow's Moppet Maker (`tool=moppet`) or Storyboard & Animation Master Pack
/// (`tool=storyboard` / `tool=pack`) in a dedicated Chrome `--app` window, and if a project is
/// currently open, automatically ingest any new assets in `~/Downloads`.
pub fn cmd_scribbles_open(s: &mut Session, p: &Value) -> Result<Value> {
    let custom_url = p["url"].as_str().or_else(|| {
        p["tool"].as_str().and_then(|t| match t.trim().to_ascii_lowercase().as_str() {
            "storyboard" | "pack" | "master" | "pages" => Some(DEFAULT_STORYBOARD_PACK_TOOL_URL),
            "moppet" | "animation" | "character" => Some(DEFAULT_MOPPET_MAKER_URL),
            _ => None,
        })
    });
    let mut out = craft_scribbles::open_moppet_maker_window(custom_url).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
    if p["ingest"].as_bool().unwrap_or(true)
        && s.project_path.is_some()
        && let Ok(ingested) = cmd_scribbles_ingest(s, p)
    {
        out["ingested"] = ingested;
    }
    Ok(out)
}

/// Scan `~/Downloads` (or `dir`) for Moppet Maker outputs (`handoff.json`, `character.png`,
/// `parts-sheet.png`, `pose-sheet.png`), copy & validate them into the open Magic Studio
/// project (`assets/characters/<id>/`), build an animated looping SVG if a pose sheet is present,
/// and register the assets in the active [`Session`].
pub fn cmd_scribbles_ingest(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before ingesting Moppet Maker assets".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let source_dir = p["dir"].as_str().filter(|d| !d.trim().is_empty()).map(PathBuf::from);
    let requested_char = p["character"].as_str().filter(|c| !c.trim().is_empty());

    let bundle =
        craft_scribbles::ingest_moppet_downloads(root, source_dir.as_deref(), requested_char).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let mut registered = Vec::new();
    let char_id = &bundle.character_id;

    if let Some(ref char_png) = bundle.character_png
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": char_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-character"),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    if let Some(ref parts_png) = bundle.parts_sheet_png
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": parts_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-parts-sheet"),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    if let Some(ref pose_png) = bundle.pose_sheet_png {
        if let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": pose_png,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-pose-sheet"),
                "load": false
            }),
        ) {
            registered.push(res);
        }
        let _ = s.execute(
            "character.sprite",
            json!({
                "character": char_id,
                "action": &bundle.action,
                "name": &bundle.action,
                "sheet": pose_png,
                "columns": bundle.grid.0,
                "rows": bundle.grid.1,
                "fps": 6
            }),
        );
    }

    if bundle.parts_json.is_some() && bundle.character_png.is_some() {
        let _ = s.execute(
            "character.cut",
            json!({
                "character": char_id,
                "view": &bundle.view,
                "rig": true
            }),
        );
    }

    if let Some(ref svg_path) = bundle.animated_svg
        && let Ok(res) = s.execute(
            "project.add_asset",
            json!({
                "path": svg_path,
                "kind": "image",
                "character": char_id,
                "name": format!("{char_id}-{}-svg", bundle.action),
                "load": false
            }),
        )
    {
        registered.push(res);
    }

    // Also ingest any ID-marked storyboard assets (`<CODE>__<slot>.png`) from `~/Downloads` or `storyboard/generated`
    // and auto-place them onto their exact storyboard pages and slots in a single batch!
    let mut id_placed = Vec::new();
    if let Ok(marked) = craft_scribbles::ingest_id_marked_assets(root, source_dir.as_deref())
        && !marked.is_empty()
    {
        id_placed = apply_id_marked_assets_to_storyboard(s, &marked);
    }

    Ok(json!({
        "ok": true,
        "character_id": bundle.character_id,
        "character_name": bundle.character_name,
        "view": bundle.view,
        "action": bundle.action,
        "grid": [bundle.grid.0, bundle.grid.1],
        "source_dir": bundle.source_dir,
        "dest_dir": bundle.dest_dir,
        "handoff_json": bundle.handoff_json,
        "parts_json": bundle.parts_json,
        "character_png": bundle.character_png,
        "parts_sheet_png": bundle.parts_sheet_png,
        "pose_sheet_png": bundle.pose_sheet_png,
        "animated_svg": bundle.animated_svg,
        "registered_count": registered.len(),
        "registered": registered,
        "storyboard_placed_count": id_placed.len(),
        "storyboard_placed": id_placed,
    }))
}

/// Route ID-marked files (`<CODE>__<slot>.png`) to `project.add_asset` and `storyboard.set` in one batch.
fn apply_id_marked_assets_to_storyboard(s: &mut Session, marked: &[(String, String, String, PathBuf)]) -> Vec<Value> {
    let mut page_images: BTreeMap<usize, serde_json::Map<String, Value>> = BTreeMap::new();
    let mut scene_images: BTreeMap<usize, serde_json::Map<String, Value>> = BTreeMap::new();
    let mut placed_summary = Vec::new();

    // Inspect storyboard pages so we can map `COVER`, `L01`..`L03`, `C01`..`C03`, `S01`..`S99`, or `P<n>` to exact page indices
    let pages_val = s.execute("storyboard.pages", json!({ "full": true })).unwrap_or_default();
    let pages_arr = pages_val["pages"].as_array().cloned().unwrap_or_default();
    let total_pages = pages_arr.len();

    for (id_marker, code, slot, path) in marked {
        let path_str = path.to_string_lossy().to_string();
        let asset_name = craft_scribbles::sanitize_id(id_marker);
        let _ = s.execute(
            "project.add_asset",
            json!({
                "path": path_str,
                "kind": "image",
                "name": asset_name,
                "load": false
            }),
        );

        if code == "COVER" {
            page_images.entry(0).or_default().insert(slot.clone(), json!(path_str));
        } else if let Some(num_str) = code.strip_prefix('P')
            && let Ok(p_idx) = num_str.parse::<usize>()
        {
            page_images.entry(p_idx).or_default().insert(slot.clone(), json!(path_str));
        } else if let Some(num_str) = code.strip_prefix('L')
            && let Ok(loc_no) = num_str.parse::<usize>()
        {
            // Place on the primary world page (`page=1`) into `world_stage_image` (L01), `world_source_image` (L02), `world_result_image` (L03)
            let target_slot = if slot.starts_with("world_") {
                slot.clone()
            } else {
                match loc_no {
                    1 => "world_stage_image".to_string(),
                    2 => "world_source_image".to_string(),
                    _ => "world_result_image".to_string(),
                }
            };
            page_images.entry(1).or_default().insert(target_slot.clone(), json!(path_str));
            if (1..=3).contains(&loc_no) {
                page_images.entry(0).or_default().insert(format!("cover_reg_thumb_{loc_no}"), json!(path_str));
            }
        } else if let Some(num_str) = code.strip_prefix('C')
            && let Ok(char_no) = num_str.parse::<usize>()
        {
            let target_slot = if slot.starts_with("char_") { slot.clone() } else { "char_original_image".to_string() };
            page_images.entry(2).or_default().insert(target_slot, json!(path_str));
            if (1..=3).contains(&char_no) {
                page_images.entry(0).or_default().insert(format!("cover_reg_thumb_{}", char_no + 3), json!(path_str));
            }
        } else if let Some(num_str) = code.strip_prefix('S')
            && let Ok(scene_no) = num_str.parse::<usize>()
        {
            let entry = scene_images.entry(scene_no).or_default();
            entry.insert(slot.clone(), json!(path_str));
            if slot == "hero_image" {
                entry.entry("result_image".to_string()).or_insert_with(|| json!(path_str));
                if (1..=5).contains(&scene_no) {
                    page_images.entry(0).or_default().insert(format!("cover_songmap_{scene_no}"), json!(path_str));
                }
            }
        }

        placed_summary.push(json!({
            "id_marker": id_marker,
            "code": code,
            "slot": slot,
            "path": path,
        }));
    }

    let mut batch_pages = Vec::new();
    for (p_idx, imgs) in page_images {
        if p_idx < total_pages || total_pages == 0 {
            batch_pages.push(json!({
                "page": p_idx,
                "images": Value::Object(imgs),
            }));
        }
    }
    for (scene_no, imgs) in scene_images {
        batch_pages.push(json!({
            "scene": scene_no,
            "images": Value::Object(imgs),
        }));
    }
    if !batch_pages.is_empty() {
        let _ = s.execute("storyboard.set", json!({ "pages": batch_pages }));
    }
    placed_summary
}

/// Request keys that say where a drawing is saved (`storyboard.set` page, scene, slot, ...).
const SAVE_TARGET_KEYS: &[&str] = &["path", "page", "scene", "slot", "label"];

fn resolve_template_request(p: &Value) -> Result<(Value, Value)> {
    let all = crate::fidget::templates()?;
    let kinds = &all["kinds"];

    // 1. Already prepared by `fidget.prepare`
    if let Some(fidget) = p.get("fidget").filter(|v| v.is_object()) {
        let kind_name = fidget["kind"]
            .as_str()
            .filter(|k| kinds.get(*k).is_some())
            .ok_or_else(|| EngineError::InvalidParams("Scribbles requires a valid template in `fidget.kind`; inspect `scribbles.templates`".into()))?;
        let spec = &kinds[kind_name];
        let mut meta = fidget.clone();
        if meta.get("save_command").is_none() {
            meta["save_command"] = spec["save_command"].clone();
        }
        if meta.get("save_kind").is_none() {
            meta["save_kind"] = spec["save_kind"].clone();
        }
        if meta.get("save_slot").is_none() {
            meta["save_slot"] = spec["save_slot"].clone();
        }
        if meta.get("note").is_none() {
            meta["note"] = json!(crate::fidget::FIDGET_NOTE);
        }
        return Ok((p.clone(), meta));
    }

    // 2. Explicit `kind` or `template` parameter -> prepare via Fidget
    if let Some(kind_name) = p["kind"].as_str().or_else(|| p["template"].as_str()) {
        let prepared = crate::fidget::PreparedDrawing::prepare(p, kind_name)?;
        let mut request = prepared.request;
        // Where the drawing is saved belongs to the caller, not the template: keep it.
        for key in SAVE_TARGET_KEYS {
            if let Some(v) = p.get(*key) {
                request[*key] = v.clone();
            }
        }
        return Ok((request, prepared.fidget));
    }

    // 3. Prompt or markdown carries Fidget YAML frontmatter (`---\n...template: <kind>...`)
    let frontmatter_kind = p["markdown"]
        .as_str()
        .and_then(crate::fidget::parse_frontmatter_template)
        .or_else(|| p["prompt"].as_str().and_then(crate::fidget::parse_frontmatter_template));
    if let Some(kind_name) = frontmatter_kind {
        let spec = kinds.get(&kind_name).ok_or_else(|| {
            EngineError::InvalidParams(format!("Unknown Scribbles template `{kind_name}` in Markdown frontmatter; inspect `scribbles.templates`"))
        })?;
        let meta = json!({
            "role": "fidget",
            "kind": kind_name,
            "label": spec["label"],
            "reference": spec["reference"],
            "canvas": spec["canvas"],
            "tags": spec["tags"],
            "save_command": spec["save_command"],
            "save_kind": spec["save_kind"],
            "save_slot": spec["save_slot"],
            "note": crate::fidget::FIDGET_NOTE,
        });
        return Ok((p.clone(), meta));
    }

    // 4. Direct prompt without an explicit template defaults to `custom` so `scribbles.generate prompt=...` works across codex, agy, and flow.
    if p["prompt"].as_str().is_some_and(|v| !v.trim().is_empty()) {
        let prepared = crate::fidget::PreparedDrawing::prepare(p, "custom")?;
        let mut request = prepared.request;
        for key in SAVE_TARGET_KEYS {
            if let Some(v) = p.get(*key) {
                request[*key] = v.clone();
            }
        }
        return Ok((request, prepared.fidget));
    }

    Err(EngineError::InvalidParams(
        "Scribbles requires a template (`kind`, `template`, or `fidget` from `fidget.prepare`); inspect `scribbles.templates`".into(),
    ))
}

fn apply_template_save_command(s: &mut Session, request: &Value, metadata: &Value, generated: &Value) -> Result<Value> {
    let save_cmd = metadata["save_command"].as_str().ok_or_else(|| EngineError::InvalidParams("the Scribbles template names no save_command".into()))?;
    let image_path = generated["image"].as_str().unwrap_or_default();
    if save_cmd == "storyboard.set" && !image_path.is_empty() && (request.get("scene").is_some() || request.get("page").is_some()) {
        let slot = request["slot"]
            .as_str()
            .or_else(|| metadata["save_slot"].as_str())
            .ok_or_else(|| EngineError::InvalidParams("pass `slot`: this Scribbles template names no storyboard slot".into()))?;
        let mut set_params = json!({
            "images": { slot: image_path }
        });
        for key in SAVE_TARGET_KEYS.iter().filter(|k| **k != "slot") {
            if let Some(v) = request.get(*key) {
                set_params[*key] = v.clone();
            }
        }
        let receipt = s.execute("storyboard.set", set_params)?;
        return Ok(json!({
            "command": "storyboard.set",
            "slot": slot,
            "receipt": receipt,
        }));
    }
    if save_cmd == "character.sprite"
        && !image_path.is_empty()
        && let Some(char_id) = request["character"].as_str().filter(|v| !v.trim().is_empty())
    {
        let action_name = request["action"].as_str().unwrap_or("walk");
        if let Ok(receipt) = s.execute(
            "character.sprite",
            json!({
                "character": char_id,
                "name": action_name,
                "action": action_name,
                "sheet": image_path,
                "cols": 4,
                "rows": 2,
            }),
        ) {
            return Ok(json!({
                "command": "character.sprite",
                "character": char_id,
                "action": action_name,
                "receipt": receipt,
            }));
        }
    }
    Ok(json!({
        "command": save_cmd,
        "asset_id": generated["asset_id"],
        "image": generated["image"],
    }))
}

/// Resolve the active Scribbles provider from the request, `scribbles.provider`, or `image.provider`.
fn resolve_scribbles_provider(request: &Value) -> Result<String> {
    if let Some(p) = request["provider"].as_str().map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(p.to_owned());
    }
    let settings = crate::project_settings::stored_settings();
    if let Some(p) = settings["scribbles"]["provider"].as_str().map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(p.to_owned());
    }
    if let Some(p) = settings["image"]["provider"].as_str().map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(p.to_owned());
    }
    Err(EngineError::InvalidParams(
        "no Scribbles provider is chosen: pass `provider` (codex, agy, or flow), or set it with `magic-cli exec scribbles.connect provider=codex`".into(),
    ))
}

/// Generate a final prompt gated by a Scribbles template (or open Flow Moppet Maker when called
/// with `open=true` or without a prompt/template). Supports single or looped (`count` 1..=4)
/// execution across `codex` (Luna), `agy` (Flash), and `flow` (Browser-Agent).
pub fn cmd_scribbles_generate(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_object() {
        return Err(EngineError::InvalidParams("Scribbles needs a JSON object with a template and prompt".into()));
    }
    if p["open"].as_bool() == Some(true)
        || (p["prompt"].as_str().is_none_or(|v| v.trim().is_empty())
            && p.get("fidget").is_none()
            && p.get("kind").is_none()
            && p.get("template").is_none()
            && p["markdown"].as_str().is_none_or(|v| v.trim().is_empty()))
    {
        return cmd_scribbles_open(s, p);
    }
    let kind_hint = p["kind"].as_str().or_else(|| p["template"].as_str()).or_else(|| p["fidget"]["kind"].as_str());
    if kind_hint == Some("moppet") {
        return cmd_scribbles_moppet(s, p);
    }
    if p.get("poses").is_some() || p.get("rig_parts").is_some() {
        return Err(EngineError::InvalidParams("Prepare pose/rig instructions in the final prompt before calling Scribbles".into()));
    }
    let (mut request, mut metadata) = resolve_template_request(p)?;
    if request["prompt"].as_str().is_none_or(|v| v.trim().is_empty()) {
        return Err(EngineError::InvalidParams("Scribbles needs a nonempty final prompt".into()));
    }
    let provider = resolve_scribbles_provider(&request)?;
    request["provider"] = json!(provider);

    // A template that saves into the storyboard needs to know where, before anything is drawn.
    if metadata["save_command"] == "storyboard.set" && request.get("page").is_none() && request.get("scene").is_none() {
        return Err(EngineError::InvalidParams(
            "this Scribbles template saves into the storyboard: pass `page` or `scene` (and `slot` to override the template's slot)".into(),
        ));
    }
    let count = request.get("count").and_then(Value::as_u64).unwrap_or(1);
    if !(1..=4).contains(&count) {
        return Err(EngineError::InvalidParams("scribbles count must be between 1 and 4".into()));
    }
    let mut runs = Vec::new();
    let mut last_result = Value::Null;
    for index in 0..count {
        let mut iter_req = request.clone();
        if count > 1 {
            let base_name = request["name"].as_str().unwrap_or("scribbles-item");
            iter_req["name"] = json!(format!("{base_name}-{}", index + 1));
        }
        let generated = crate::image_generation::generate(s, &iter_req)?;
        let save_receipt = apply_template_save_command(s, &iter_req, &metadata, &generated)?;
        let mut entry = generated.clone();
        entry["save_receipt"] = save_receipt;
        runs.push(entry);
        last_result = generated;
    }
    let canvas = &metadata["canvas"];
    metadata["canvas_matches"] = json!(last_result["width"] == canvas[0] && last_result["height"] == canvas[1]);
    metadata["loop_count"] = json!(count);
    if let Some(first) = runs.first() {
        metadata["save_receipt"] = first["save_receipt"].clone();
    }
    if count > 1 {
        metadata["items"] = json!(runs);
    }
    last_result["scribbles"] = metadata;
    Ok(last_result)
}

fn draw(s: &mut Session, p: &Value, name: &str) -> Result<Value> {
    let prepared = crate::fidget::PreparedDrawing::prepare(p, name)?;
    let mut request = prepared.request;
    for key in SAVE_TARGET_KEYS {
        if let Some(v) = p.get(*key) {
            request[*key] = v.clone();
        }
    }
    request["fidget"] = prepared.fidget;
    cmd_scribbles_generate(s, &request)
}

macro_rules! kind_command {
    ($fn:ident, $kind:literal) => {
        pub fn $fn(s: &mut Session, p: &Value) -> Result<Value> {
            draw(s, p, $kind)
        }
    };
}

kind_command!(cmd_scribbles_new, "new");
kind_command!(cmd_scribbles_image, "image");
kind_command!(cmd_scribbles_sheet, "sheet");
kind_command!(cmd_scribbles_poses, "poses");
kind_command!(cmd_scribbles_poses_2x2, "poses_2x2");
kind_command!(cmd_scribbles_parts_chroma, "parts_chroma");
kind_command!(cmd_scribbles_actions, "actions");
kind_command!(cmd_scribbles_layers, "layers");
kind_command!(cmd_scribbles_custom, "custom");

/// Local Moppet Maker 3-part bundle runner (`character.png` -> `#00FF00` `parts-sheet.png` -> 2x2
/// `pose-sheet.png`) with reference chaining, auto-ingest into `characters/<id>/`, animated SVG
/// generation (`character.svg`), and optional `#00FF00` cutout layer reassembly + EffectCraft rig.
/// Works identically across `codex` (Luna), `agy` (Flash), and `flow` (Browser-Agent).
pub fn cmd_scribbles_moppet(s: &mut Session, p: &Value) -> Result<Value> {
    if p["open"].as_bool() == Some(true) {
        return cmd_scribbles_open(s, p);
    }
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before running scribbles.moppet".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_raw = p["character"].as_str().or_else(|| p["name"].as_str()).filter(|v| !v.trim().is_empty()).unwrap_or("moppet");
    let char_id = crate::workspace::sanitize_asset_id(char_raw);
    let prompt = p["prompt"].as_str().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(char_raw);
    let provider = resolve_scribbles_provider(p)?;
    let model = p["model"].as_str().filter(|v| !v.trim().is_empty());

    let staging_dir = root.join("assets/characters").join(&char_id).join("scribbles_moppet_bundle");
    std::fs::create_dir_all(&staging_dir)?;

    // 1. Character base image (`character.png`)
    let char_png = staging_dir.join("character.png");
    let existing_ref =
        p["reference"].as_str().filter(|v| !v.trim().is_empty()).map(|raw| crate::workspace::resolve_workspace_path(s, raw)).filter(|pb| pb.is_file());
    let char_gen = if let Some(ref ref_path) = existing_ref
        && p["reuse_reference"].as_bool().unwrap_or(false)
    {
        std::fs::copy(ref_path, &char_png)?;
        json!({ "image": char_png, "reused_reference": true })
    } else {
        let mut req1 = json!({
            "prompt": prompt,
            "character": char_id,
            "provider": provider,
            "name": format!("{char_id}-character"),
        });
        if let Some(m) = model {
            req1["model"] = json!(m);
        }
        if let Some(notes) = p["notes"].as_str() {
            req1["notes"] = json!(notes);
        }
        let step1 = if let Some(ref ref_path) = existing_ref {
            req1["reference"] = json!(ref_path);
            draw(s, &req1, "image")?
        } else {
            draw(s, &req1, "new")?
        };
        let drawn_path = step1["image"].as_str().ok_or_else(|| EngineError::InvalidParams("character draw produced no image path".into()))?;
        std::fs::copy(drawn_path, &char_png)?;
        step1
    };

    // 2. Chroma-green parts sheet (`parts-sheet.png` on `#00FF00`) referencing `character.png`
    let parts_png = staging_dir.join("parts-sheet.png");
    let mut req2 = json!({
        "prompt": prompt,
        "reference": char_png,
        "character": char_id,
        "provider": provider,
        "name": format!("{char_id}-parts-sheet"),
    });
    if let Some(m) = model {
        req2["model"] = json!(m);
    }
    let step2 = draw(s, &req2, "parts_chroma")?;
    let drawn_parts = step2["image"].as_str().ok_or_else(|| EngineError::InvalidParams("parts_chroma draw produced no image path".into()))?;
    std::fs::copy(drawn_parts, &parts_png)?;
    let root_parts_png = root.join("assets/characters").join(&char_id).join("parts-sheet.png");
    let _ = std::fs::copy(&parts_png, &root_parts_png);

    // 3. 2x2 Moppet pose sheet (`pose-sheet.png`) referencing `character.png`
    let poses_png = staging_dir.join("pose-sheet.png");
    let mut req3 = json!({
        "prompt": prompt,
        "reference": char_png,
        "character": char_id,
        "provider": provider,
        "name": format!("{char_id}-pose-sheet"),
    });
    if let Some(m) = model {
        req3["model"] = json!(m);
    }
    let step3 = draw(s, &req3, "poses_2x2")?;
    let drawn_poses = step3["image"].as_str().ok_or_else(|| EngineError::InvalidParams("poses_2x2 draw produced no image path".into()))?;
    std::fs::copy(drawn_poses, &poses_png)?;

    // 4. Write `handoff.json` and run `ingest_folder_and_register` (slices 2x2 poses, builds animated SVG, registers views/front)
    let handoff_doc = json!({
        "name": char_raw,
        "id": char_id,
        "provider": provider,
        "prompt": prompt,
        "character_image": "character.png",
        "parts_sheet": "parts-sheet.png",
        "pose_sheet": "pose-sheet.png",
    });
    craft_atomic::atomic_write(&staging_dir.join("handoff.json"), &serde_json::to_vec_pretty(&handoff_doc).unwrap_or_default())?;
    let ingested = cmd_scribbles_ingest(s, &json!({ "dir": staging_dir, "character": char_id }))?;

    // 5. Reassemble `#00FF00` parts sheet into cut layers + rig if requested (default true)
    let reassembled = if p["reassemble"].as_bool().unwrap_or(true) {
        cmd_scribbles_reassemble(
            s,
            &json!({
                "character": char_id,
                "view": "front",
                "sheet": parts_png,
                "rig": p["rig"].as_bool().unwrap_or(true),
            }),
        )
        .ok()
    } else {
        None
    };

    Ok(json!({
        "ok": true,
        "format": "magic.scribbles-moppet-pack",
        "character": char_id,
        "provider": provider,
        "image": char_png,
        "character_image": char_png,
        "parts_sheet": parts_png,
        "pose_sheet": poses_png,
        "steps": {
            "character": char_gen,
            "parts_sheet": step2,
            "pose_sheet": step3,
        },
        "ingested": ingested,
        "reassembled": reassembled,
        "scribbles": {
            "kind": "moppet",
            "prompt": prompt,
            "canvas": [1024, 1024],
            "canvas_matches": true,
        }
    }))
}

/// Templates and provider options (local `codex` Luna, local `agy` Flash, and `flow` Browser-Agent)
/// remain discoverable at `scribbles.templates`.
pub fn cmd_scribbles_templates(_: &mut Session, _: &Value) -> Result<Value> {
    let all = crate::fidget::templates()?;
    Ok(json!({
        "ok": true,
        "model": all["model"],
        "note": all["note"],
        "kinds": all["kinds"],
        "providers": [
            {
                "id": crate::craft_ai_codex::CODEX_PROVIDER,
                "label": "Codex",
                "models": crate::craft_ai_codex::models_for_provider(crate::craft_ai_codex::CODEX_PROVIDER).map(|v| v.iter().map(|m| m.id.as_str().to_owned()).collect::<Vec<_>>()).unwrap_or_default(),
                "mode": "local"
            },
            {
                "id": crate::craft_ai_codex::AGY_PROVIDER,
                "label": "AGY (Gemini Flash)",
                "models": crate::craft_ai_codex::models_for_provider(crate::craft_ai_codex::AGY_PROVIDER).map(|v| v.iter().map(|m| m.id.as_str().to_owned()).collect::<Vec<_>>()).unwrap_or_default(),
                "mode": "local"
            },
            {
                "id": crate::craft_ai_codex::FLOW_PROVIDER,
                "label": "Google Flow (Browser-Agent)",
                "models": crate::craft_ai_codex::models_for_provider(crate::craft_ai_codex::FLOW_PROVIDER).map(|v| v.iter().map(|m| m.id.as_str().to_owned()).collect::<Vec<_>>()).unwrap_or_default(),
                "mode": "browser-agent"
            }
        ],
        "flow": {
            "provider": "flow",
            "tool": "Moppet Maker",
            "url": DEFAULT_MOPPET_MAKER_URL,
            "storyboard_pack_tool": "Storyboard & Animation Master Pack",
            "storyboard_pack_url": DEFAULT_STORYBOARD_PACK_TOOL_URL,
            "project_url": DEFAULT_FLOW_PROJECT_URL,
            "outputs": ["character.png", "parts-sheet.png", "pose-sheet.png", "handoff.json"],
        },
        "craft_ai": crate::fidget::craft_ai_status(),
    }))
}

/// Generate a short AI video clip through Google Flow (`flow-clip --mode video`) and register it in the project.
pub fn cmd_scribbles_video(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p["prompt"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("scribbles.video needs prompt=<motion description>".into()))?;
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before generating video clips".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let clips_dir = root.join("assets/clips");

    let name =
        p["name"].as_str().map(crate::workspace::sanitize_asset_id).filter(|v| !v.is_empty()).unwrap_or_else(|| format!("flow-clip-{}", std::process::id()));
    let out_mp4 = clips_dir.join(format!("{name}.mp4"));

    let req = VideoClipRequest {
        prompt,
        out_mp4: &out_mp4,
        model: p["model"].as_str(),
        aspect: p["aspect"].as_str(),
        duration: p["duration"].as_str(),
        resolution: p["resolution"].as_str(),
        ingredient: p["ingredient"].as_str().or_else(|| p["reference"].as_str()),
    };
    craft_scribbles::generate_video_clip(&req).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let attached = s.execute(
        "project.add_asset",
        json!({
            "path": out_mp4,
            "kind": "video",
            "name": p["name"].as_str().unwrap_or("Flow video clip"),
            "load": false
        }),
    )?;

    Ok(json!({
        "ok": true,
        "format": "magic.flow-video",
        "provider": "flow",
        "prompt": prompt,
        "video": out_mp4,
        "asset_id": attached["asset_id"],
        "status": "ready"
    }))
}

/// Slice a 2×2 chroma-green pose sheet into an animated looping SVG (`flow-clip --mode svg`)
/// and build the EffectCraft `.ecproj` sprite cycle via `character.sprite`.
pub fn cmd_scribbles_svg(s: &mut Session, p: &Value) -> Result<Value> {
    let sheet_raw = p["sheet"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| EngineError::InvalidParams("scribbles.svg needs sheet=<2x2 pose sheet PNG>".into()))?;
    let sheet = crate::workspace::resolve_workspace_path(s, sheet_raw);
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before building animated SVGs".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_else(|| "moppet".into());
    let action = p["action"].as_str().unwrap_or("wave");
    let out_svg = root.join("assets/characters").join(&char_id).join("sprites").join(action).join(format!("{char_id}-{action}.svg"));
    let fps = p["fps"].as_u64().unwrap_or(6);

    craft_scribbles::build_animated_svg(&sheet, &out_svg, fps).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    let sprite_res = s
        .execute(
            "character.sprite",
            json!({
                "character": char_id,
                "action": action,
                "name": action,
                "sheet": sheet,
                "columns": 2,
                "rows": 2,
                "fps": fps
            }),
        )
        .ok();

    let attached = s.execute(
        "project.add_asset",
        json!({
            "path": out_svg,
            "kind": "image",
            "character": char_id,
            "name": format!("{char_id}-{action}-svg"),
            "load": false
        }),
    )?;

    Ok(json!({
        "ok": true,
        "format": "magic.animated-svg",
        "svg": out_svg,
        "sheet": sheet,
        "character": char_id,
        "action": action,
        "fps": fps,
        "asset_id": attached["asset_id"],
        "sprite_project": sprite_res.and_then(|v| v.get("effect_project").cloned()),
    }))
}

/// Reassemble an exploded `#00FF00` chroma-green `parts-sheet.png` in Rust into separate transparent
/// layers + `parts.json` (`plush-9` skeleton + Puppet Pins), then run `character.cut` + `character.rig`
/// + `character.action` so the character is immediately rigged and animated in Magic Studio.
pub fn cmd_scribbles_reassemble(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before reassembling a parts sheet".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let char_id = p["character"].as_str().map(crate::workspace::sanitize_asset_id).unwrap_or_else(|| "label-pangolin".into());
    let view = p["view"].as_str().unwrap_or("front");
    let char_dir = root.join("assets/characters").join(&char_id);
    let sheet = p["sheet"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .map(|raw| crate::workspace::resolve_workspace_path(s, raw))
        .unwrap_or_else(|| char_dir.join("parts-sheet.png"));
    if !sheet.is_file() {
        return Err(EngineError::InvalidParams(format!("parts sheet {} does not exist", sheet.display())));
    }
    let view_dir = char_dir.join(view);
    let mut out = craft_scribbles::reassemble_parts_sheet(&sheet, &view_dir, &char_id, view).map_err(|e| EngineError::InvalidParams(e.to_string()))?;

    if p["rig"].as_bool().unwrap_or(true) {
        if let Ok(cut_res) = s.execute(
            "character.cut",
            json!({
                "character": char_id,
                "view": view,
                "rig": true
            }),
        ) {
            out["cut_and_rig"] = cut_res;
        }
        let action = p["action"].as_str().unwrap_or("idle");
        if let Ok(act_res) = s.execute(
            "character.action",
            json!({
                "character": char_id,
                "view": view,
                "action": action
            }),
        ) {
            out["animated_action"] = act_res;
        }
    }
    Ok(out)
}

/// Build the Master Storyboard & Animation Pack (`storyboard/storyboard-pack.json`) by reading
/// every page (or a requested `page` / `scene`) of `storyboard.designcraft`, extracting its
/// `exact_prompt` / `world_location_prompt` / `char_exact_prompt`, reference plates (`location_ref`,
/// `character_lead`, `character_shared`), animation style, and deterministic ID name markers (`<CODE>__<slot>`).
pub fn cmd_scribbles_pack(s: &mut Session, p: &Value) -> Result<Value> {
    let project = s.project_path.clone().ok_or_else(|| EngineError::InvalidParams("Open a project before building a storyboard pack".into()))?;
    let root = project.parent().ok_or_else(|| EngineError::InvalidParams("Project has no parent directory".into()))?;
    let pages_val = s.execute("storyboard.pages", json!({ "full": true }))?;
    let pages = pages_val["pages"].as_array().cloned().unwrap_or_default();

    let filter_page = p.get("page").and_then(Value::as_u64).map(|n| n as usize);
    let filter_scene = p.get("scene").and_then(Value::as_u64).map(|n| n as usize);

    let slot_text = |slots: &[Value], label: &str| -> String {
        slots.iter().find(|sl| sl["label"].as_str() == Some(label)).and_then(|sl| sl["text"].as_str()).unwrap_or_default().trim().to_string()
    };
    let slot_image = |slots: &[Value], label: &str| -> Option<String> {
        slots.iter().find(|sl| sl["label"].as_str() == Some(label)).and_then(|sl| sl["image"].as_str()).map(str::to_string)
    };

    let workspace_root = std::env::current_dir().unwrap_or_else(|_| root.to_path_buf());
    let mut cast_moppets: Vec<Value> = Vec::new();
    let mut moppet_lookup: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();
    for (id, name, rel_img, locked_desc) in LOCKED_MOPPETS {
        let abs_img = workspace_root.join(rel_img);
        moppet_lookup.insert(id.to_ascii_lowercase(), (abs_img, (*locked_desc).to_string()));
        moppet_lookup.insert(name.to_ascii_lowercase(), (workspace_root.join(rel_img), (*locked_desc).to_string()));
    }

    let (core_style, master_prompt, master_reference_register, cover_key_art_image) = if let Some(cover_pg) = pages.first() {
        let sl = cover_pg["slots"].as_array().cloned().unwrap_or_default();
        let sub = slot_text(&sl, "cover_subtitle");
        let design = slot_text(&sl, "cover_overall_design");
        let key_art = slot_image(&sl, "cover_key_art");
        let mut reg = Vec::new();
        for idx in 1..=6 {
            let code = slot_text(&sl, &format!("cover_reg_id_{idx}"));
            let name = slot_text(&sl, &format!("cover_reg_name_{idx}"));
            let where_used = slot_text(&sl, &format!("cover_reg_where_{idx}"));
            let thumb = slot_image(&sl, &format!("cover_reg_thumb_{idx}"));
            if !code.is_empty() {
                let lower_name = name.to_ascii_lowercase();
                for (m_id, m_name, rel_img, locked_desc) in LOCKED_MOPPETS {
                    if lower_name.contains(m_id) && !cast_moppets.iter().any(|v| v["id"].as_str() == Some(*m_id)) {
                        cast_moppets.push(json!({
                            "code": code,
                            "id": m_id,
                            "name": m_name,
                            "locked": true,
                            "reference_image": workspace_root.join(rel_img),
                            "locked_design": locked_desc,
                        }));
                    }
                }
                reg.push(json!({
                    "index": idx,
                    "code": code,
                    "name": name,
                    "where": where_used,
                    "slot": format!("cover_reg_thumb_{idx}"),
                    "image": thumb,
                }));
            }
        }
        let core = if sub.is_empty() {
            "CORE STYLE: torn notebook paper, painted ink, cardboard, clay, foil; front 3/4 only, quiet title zone, no text in image.".to_string()
        } else {
            sub
        };
        let master = if design.is_empty() { core.clone() } else { format!("{core} | {design}") };
        (core, master, reg, key_art)
    } else {
        (
            "CORE STYLE: torn notebook paper, painted ink, cardboard, clay, foil; front 3/4 only, quiet title zone, no text in image.".to_string(),
            String::new(),
            Vec::new(),
            None,
        )
    };

    let mut items: Vec<StoryboardPackItem> = Vec::new();
    let mut scene_counter = 0usize;

    for pg in &pages {
        let page_idx = pg["page"].as_u64().unwrap_or(0) as usize;
        let archetype = pg["archetype"].as_str().unwrap_or("scene").to_string();
        let title = pg["title"].as_str().unwrap_or_default().to_string();
        let slots = pg["slots"].as_array().cloned().unwrap_or_default();

        if archetype == "scene" {
            scene_counter += 1;
        }
        if let Some(fp) = filter_page
            && fp != page_idx
        {
            continue;
        }
        if let Some(fs) = filter_scene
            && (archetype != "scene" || scene_counter != fs)
        {
            continue;
        }

        match archetype.as_str() {
            "cover" => {
                let prompt = format!("{core_style} | {}", slot_text(&slots, "cover_overall_design"));
                items.push(StoryboardPackItem {
                    id_marker: "COVER__cover_key_art".into(),
                    code: "COVER".into(),
                    page: page_idx,
                    archetype: archetype.clone(),
                    slot: "cover_key_art".into(),
                    kind: "cover".into(),
                    title: title.clone(),
                    prompt,
                    animation_style: "collage_key_art".into(),
                    references: vec!["L01".into(), "L02".into(), "C01".into()],
                    reference_image: None,
                    existing_image: slot_image(&slots, "cover_key_art"),
                });
            }
            "world" if page_idx == 1 => {
                let base_prompt = slot_text(&slots, "world_location_prompt");
                let l01_cap = slot_text(&slots, "world_stage_caption");
                let l02_cap = slot_text(&slots, "world_source_image_caption");
                let l03_cap = slot_text(&slots, "world_result_image_caption");
                let l01_desc = if !l01_cap.is_empty() {
                    format!("L01 {l01_cap}")
                } else {
                    "L01 Blank Paper Wall (verse): blank ruled notebook paper wall + hand-drawn ink doodle arrows".to_string()
                };
                let l02_desc = if !l02_cap.is_empty() {
                    format!("L02 {l02_cap}")
                } else {
                    "L02 Cardboard Box (chorus mock): open corrugated cardboard box + empty tray, front 3/4".to_string()
                };
                let l03_desc = if !l03_cap.is_empty() {
                    format!("L03 {l03_cap}")
                } else {
                    "L03 Clay Room (AI plate outro): empty matte clay room + crumpled foil spotlight cone, no text".to_string()
                };

                for (code, slot, desc) in
                    [("L01", "world_stage_image", l01_desc), ("L02", "world_source_image", l02_desc), ("L03", "world_result_image", l03_desc)]
                {
                    items.push(StoryboardPackItem {
                        id_marker: format!("{code}__{slot}"),
                        code: code.into(),
                        page: page_idx,
                        archetype: archetype.clone(),
                        slot: slot.into(),
                        kind: "location".into(),
                        title: desc.clone(),
                        prompt: format!("{base_prompt} | FOCUS: {desc}"),
                        animation_style: "location_plate".into(),
                        references: Vec::new(),
                        reference_image: None,
                        existing_image: slot_image(&slots, slot),
                    });
                }
            }
            "char" => {
                let base_prompt = slot_text(&slots, "char_exact_prompt");
                let reg_name_for = |code: &str| -> String {
                    master_reference_register
                        .iter()
                        .find(|r| r["code"].as_str() == Some(code))
                        .map(|r| format!("{} {} ({})", code, r["name"].as_str().unwrap_or_default(), r["where"].as_str().unwrap_or_default()))
                        .unwrap_or_else(|| code.to_string())
                };
                let c01_cap = slot_text(&slots, "char_original_image_caption");
                let c02_cap = slot_text(&slots, "char_pose_image_caption");
                let c03_cap = slot_text(&slots, "char_hand_note");
                let c01_default =
                    if !c01_cap.is_empty() { format!("C01 {c01_cap}") } else { "C01 Creator paper cutout character with crayon shrug, front 3/4".to_string() };
                let c02_default = if !c02_cap.is_empty() {
                    format!("C02 {c02_cap}")
                } else {
                    "C02 Clay Mic with crumpled silver foil grille on clay stand, front 3/4".to_string()
                };
                let c03_default = if !c03_cap.is_empty() {
                    format!("C03 {c03_cap}")
                } else {
                    "C03 Paper Hand cutout reaching with cursor physics energy, front 3/4".to_string()
                };

                for (code, slot, default_desc) in
                    [("C01", "char_original_image", c01_default), ("C02", "char_pose_image", c02_default), ("C03", "char_geometry_pose", c03_default)]
                {
                    let reg_label = reg_name_for(code);
                    let matched_moppet = LOCKED_MOPPETS.iter().find(|(m_id, _, _, _)| reg_label.to_ascii_lowercase().contains(*m_id));
                    let (desc, locked_ref) = if let Some((_, _, rel_img, locked_desc)) = matched_moppet {
                        let p = workspace_root.join(rel_img);
                        (format!("LOCKED MOPPET DESIGN: {locked_desc}"), p.is_file().then(|| p.to_string_lossy().to_string()))
                    } else {
                        (default_desc.clone(), None)
                    };
                    items.push(StoryboardPackItem {
                        id_marker: format!("{code}__{slot}"),
                        code: code.into(),
                        page: page_idx,
                        archetype: archetype.clone(),
                        slot: slot.into(),
                        kind: "character".into(),
                        title: desc.clone(),
                        prompt: format!("{base_prompt} | FOCUS: {desc}"),
                        animation_style: "cutout_rig".into(),
                        references: Vec::new(),
                        reference_image: locked_ref,
                        existing_image: slot_image(&slots, slot),
                    });
                }
            }
            "scene" => {
                let code = format!("S{scene_counter:02}");
                let design = slot_text(&slots, "overall_design");
                let exact_prompt = slot_text(&slots, "exact_prompt");
                let loc_ref = slot_text(&slots, "location_ref");
                let char_lead = slot_text(&slots, "character_lead");
                let char_shared = slot_text(&slots, "character_shared");
                let mut refs = Vec::new();
                for r in [&loc_ref, &char_lead, &char_shared] {
                    if !r.is_empty() && !r.starts_with('[') {
                        refs.push(r.clone());
                    }
                }
                let anim_style = classify_animation_style(&design, &exact_prompt);
                items.push(StoryboardPackItem {
                    id_marker: format!("{code}__hero_image"),
                    code,
                    page: page_idx,
                    archetype: archetype.clone(),
                    slot: "hero_image".into(),
                    kind: "scene".into(),
                    title: format!("SCENE {scene_counter:02} — {}", slot_text(&slots, "title")),
                    prompt: if exact_prompt.is_empty() { format!("{core_style} | {design}") } else { exact_prompt },
                    animation_style: anim_style,
                    references: refs,
                    reference_image: slot_image(&slots, "source_image"),
                    existing_image: slot_image(&slots, "hero_image"),
                });
            }
            _ => {}
        }
    }

    // Build the Master World Composite (`storyboard/composites/MASTER_WORLD__composite.png`) by flattening
    // only canonical Location (`L01`..`L03`) and canonical Character (`C01`, `C02`) plates (plus locked
    // canonical Moppet references when cast) — never using one-off/cardboard joke variants (`C03`) as a reference!
    let mut code_to_img: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut master_world_sources: Vec<PathBuf> = Vec::new();
    for item in &items {
        if let Some(ref img) = item.existing_image {
            let pb = PathBuf::from(img);
            if pb.is_file() {
                if item.code == "C03" {
                    // Keep C03 (e.g. cardboard Moppet gag) as a scene/character asset, never as an upstream reference
                    continue;
                }
                code_to_img.insert(item.code.clone(), pb.clone());
                if (item.code.starts_with('L') || item.code == "C01" || item.code == "C02") && !master_world_sources.contains(&pb) {
                    master_world_sources.push(pb);
                }
            }
        }
    }
    let comp_dir = root.join("storyboard/composites");
    let master_world_composite = if !master_world_sources.is_empty() {
        let out_master = comp_dir.join("MASTER_WORLD__composite.png");
        craft_scribbles::build_composite_reference(&master_world_sources, &out_master).ok()
    } else {
        None
    };
    if let Some(ref mw) = master_world_composite {
        code_to_img.insert("COVER".into(), mw.clone());
        code_to_img.insert("MASTER_WORLD".into(), mw.clone());
    }
    for (id, _, rel_img, _) in LOCKED_MOPPETS {
        let p = workspace_root.join(rel_img);
        if p.is_file() {
            code_to_img.insert(format!("{}_LOCKED", id.to_ascii_uppercase()), p);
        }
    }
    let mut source_updates = Vec::new();
    if let Some(ref mw) = master_world_composite {
        source_updates.push(json!({
            "page": 0,
            "images": { "cover_key_art": mw }
        }));
    }
    for item in &mut items {
        let mut ref_files: Vec<PathBuf> = Vec::new();
        for r in &item.references {
            let prefix = r.split_whitespace().next().unwrap_or(r).trim().to_ascii_uppercase();
            if let Some(pb) = code_to_img.get(&prefix)
                && !ref_files.contains(pb)
            {
                ref_files.push(pb.clone());
            }
        }
        if ref_files.len() == 1 {
            item.reference_image = Some(ref_files[0].to_string_lossy().to_string());
        } else if ref_files.len() > 1 {
            let out_comp = comp_dir.join(format!("{}__reference.png", item.code));
            if let Ok(built) = craft_scribbles::build_composite_reference(&ref_files, &out_comp) {
                let built_str = built.to_string_lossy().to_string();
                item.reference_image = Some(built_str.clone());
                if item.archetype == "scene" {
                    source_updates.push(json!({
                        "page": item.page,
                        "images": { "source_image": built_str }
                    }));
                }
            }
        } else if let Some(ref mw) = master_world_composite {
            item.reference_image = Some(mw.to_string_lossy().to_string());
        }
    }
    if !source_updates.is_empty() && p["sync_sources"].as_bool().unwrap_or(true) {
        let _ = s.execute("storyboard.set", json!({ "pages": source_updates }));
    }

    let missing_count = items.iter().filter(|i| i.existing_image.is_none()).count();
    let mut pack_doc = json!({
        "ok": true,
        "format": "magic.storyboard-pack.v1",
        "project": project,
        "flow_tool_url": DEFAULT_STORYBOARD_PACK_TOOL_URL,
        "moppet_maker_url": DEFAULT_MOPPET_MAKER_URL,
        "core_style": core_style,
        "master_prompt": master_prompt,
        "master_world_composite": master_world_composite,
        "cover_key_art_image": master_world_composite.as_ref().map(|p| p.to_string_lossy().to_string()).or(cover_key_art_image),
        "master_reference_register": master_reference_register,
        "total_items": items.len(),
        "missing_count": missing_count,
        "items": items,
    });
    if !cast_moppets.is_empty() || p["include_moppets"].as_bool().unwrap_or(false) {
        let locked_moppets: Vec<Value> = if !cast_moppets.is_empty() {
            cast_moppets
        } else {
            LOCKED_MOPPETS
                .iter()
                .map(|(id, name, rel_img, locked_desc)| {
                    json!({
                        "id": id,
                        "name": name,
                        "locked": true,
                        "reference_image": workspace_root.join(rel_img),
                        "locked_design": locked_desc,
                    })
                })
                .collect()
        };
        pack_doc["moppets_lore"] = json!("Electronic devices with AI hearts and brains that act like autonomous puppets.");
        pack_doc["moppets_roster_image"] = json!(workspace_root.join("assets/characters/moppets/moppets-roster.jpg"));
        pack_doc["locked_moppets"] = json!(locked_moppets);
    }

    let sb_dir = root.join("storyboard");
    std::fs::create_dir_all(&sb_dir)?;
    let out_path = sb_dir.join("storyboard-pack.json");
    if let Ok(bytes) = serde_json::to_vec_pretty(&pack_doc) {
        craft_atomic::atomic_write(&out_path, &bytes)?;
    }

    if p["open"].as_bool().unwrap_or(false) {
        let _ = craft_scribbles::open_moppet_maker_window(Some(DEFAULT_STORYBOARD_PACK_TOOL_URL));
    }

    let mut res = pack_doc;
    res["pack_path"] = json!(out_path);
    Ok(res)
}
