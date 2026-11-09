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

/// Generate an image through the request's provider or the `image.provider` setting,
/// or open Moppet Maker when called without a prompt.
pub fn cmd_scribbles_generate(s: &mut Session, p: &Value) -> Result<Value> {
    if p["prompt"].as_str().is_none_or(|v| v.trim().is_empty()) {
        return cmd_scribbles_open(s, p);
    }
    crate::image_generation::generate(s, p)
}

/// Inspect Scribbles / Google Flow Moppet Maker and Storyboard Master Pack status and URLs.
pub fn cmd_scribbles_templates(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({
        "ok": true,
        "provider": "flow",
        "tool": "Moppet Maker",
        "url": DEFAULT_MOPPET_MAKER_URL,
        "storyboard_pack_tool": "Storyboard & Animation Master Pack",
        "storyboard_pack_url": DEFAULT_STORYBOARD_PACK_TOOL_URL,
        "project_url": DEFAULT_FLOW_PROJECT_URL,
        "outputs": ["character.png", "parts-sheet.png", "pose-sheet.png", "handoff.json", "<CODE>__<slot>.png", "storyboard-pack.json"],
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
