//! `character.mark`: Scribbles (Codex) looks at one drawing and writes `parts.json`: each part
//! of the shared skeleton as an outline and the joint it turns on. The marks are a draft for
//! review on the Studio canvas before `character.cut`.

use serde_json::{Value, json};

use super::{POSES, View, bad, part_list, path_arg, point, root_part};
use crate::{EngineError, Result, Session};

const PARENTS: [(&str, &str); 8] = [
    ("head", "torso"),
    ("ear_L", "head"),
    ("ear_R", "head"),
    ("arm_L", "torso"),
    ("arm_R", "torso"),
    ("leg_L", "torso"),
    ("leg_R", "torso"),
    ("tail", "torso"),
];

fn instruction(view: &str, size: (u32, u32), file: &str) -> String {
    let parents: Vec<String> = PARENTS.iter().map(|(c, p)| format!("{c}->{p}")).collect();
    format!(
        "You are marking a character drawing for a cut-out puppet rig, as a designer does before cutting \
         layers in Photoshop. Look at the attached image ({w}x{h} px, {view} view). Write ONLY the file \
         {file} in the current directory, as JSON: {{\"image\": <the image file name>, \"view\": \"{view}\", \
         \"size\": [{w}, {h}], \"parts\": [...]}}. List the parts back to front (furthest from the viewer first). \
         Use these ids only, and only for parts that are visible: torso, head, ear_L, ear_R, arm_L, arm_R, \
         leg_L, leg_R, tail (_L/_R are the character's own left and right). torso has \"parent\": null; the \
         others use these parents: {parents}. Each part: {{\"id\", \"parent\", \"joint\": [x, y], \"outline\": \
         [[x, y], ...]}}, in image pixels from the top-left. joint is the centre of the hinge where the part \
         turns on its parent (shoulder, hip, neck, ear root, tail root; for torso, the hips centre). outline \
         is a closed polygon of 8 to 40 points tracing that part's visible silhouette, including clothing \
         that moves with it (a sleeve with its arm, a trouser leg with its leg). Outlines may overlap a \
         little at joints. Do not create images, run tools that edit images, or change any other file.",
        w = size.0,
        h = size.1,
        parents = parents.join(", "),
    )
}

fn codex_args(view: &View, image: &std::path::Path, size: (u32, u32)) -> Vec<String> {
    vec![
        "exec".into(),
        "--ephemeral".into(),
        "--skip-git-repo-check".into(),
        "--sandbox".into(),
        "workspace-write".into(),
        "--cd".into(),
        path_arg(&view.dir),
        "--image".into(),
        path_arg(image),
        "--".into(),
        instruction(&view.view, size, "parts.json"),
    ]
}

/// The drawing, copied beside the marks so every later step reads one folder.
fn stage_image(view: &View, p: &Value, s: &Session) -> Result<(std::path::PathBuf, (u32, u32))> {
    let source = p["image"]
        .as_str()
        .filter(|i| !i.trim().is_empty())
        .map(|i| crate::workspace::resolve_workspace_path(s, i))
        .filter(|i| i.is_file())
        .or_else(|| view.dir.parent().and_then(super::find_initial_cutout))
        .ok_or_else(|| bad("image=<drawing.png> is required"))?;
    std::fs::create_dir_all(&view.dir)?;
    let staged = view.dir.join(format!("{}-{}-drawing.png", view.id, view.view));
    if source != staged {
        std::fs::copy(&source, &staged)?;
    }
    let dims = super::cut::photocraft_dimensions(&staged).or_else(|_| image::image_dimensions(&staged).map_err(|e| bad(e.to_string())))?;
    Ok((staged, dims))
}

fn check_marks(view: &View, staged: &std::path::Path) -> Result<Value> {
    let mut parts = view.parts()?;
    let list = part_list(&parts)?;
    root_part(list)?;
    let skeleton: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let known = skeleton["parts"].as_array().cloned().unwrap_or_default();
    for part in list {
        if !known.contains(&part["id"]) {
            return Err(bad(format!("Scribbles marked an unknown part {}", part["id"])));
        }
    }
    parts["image"] = json!(staged.file_name().map(|n| n.to_string_lossy().into_owned()));
    parts["skeleton"] = skeleton["skeleton"].clone();
    parts["status"] = json!("draft; review on the canvas before character.cut");
    view.write_parts(&parts)?;
    Ok(parts)
}

/// Optional Scribbles body-plate redraw when `redraw=true` is requested and `parts/source/body-plate_v1.png`
/// does not exist yet: asks `image_generation::generate` to redraw the character's torso/body plate
/// without overlapping front fins/eyes, then resizes it to the character's exact canvas in PhotoCraft.
fn ensure_redrawn_body_plate(s: &mut Session, view: &View, size: (u32, u32), p: &Value) -> Result<Option<std::path::PathBuf>> {
    let Some(char_dir) = view.dir.parent() else {
        return Ok(None);
    };
    let plate = char_dir.join("parts/source/body-plate_v1.png");
    if plate.is_file() && p["force_redraw"] != true {
        return Ok(Some(plate));
    }
    if p["redraw"] != true {
        return Ok(plate.is_file().then_some(plate));
    }
    let prompt = p["redraw_prompt"].as_str().map(str::to_string).unwrap_or_else(|| {
        format!(
            "Clean felt and cut-paper body-plate infill for character '{}' ({}x{} px). Keep the exact body silhouette, \
             felt texture, and polka-dot/scale pattern, but omit the overlapping front pectoral fin and eye so the body \
             underneath is uninterrupted when the fin and eye layers rotate.",
            view.id, size.0, size.1
        )
    });
    let generated = crate::image_generation::generate(s, &json!({"prompt": prompt, "name": format!("{}-body-plate", view.id)}))?;
    if let Some(raw_img) = generated["image"].as_str() {
        let src = std::path::Path::new(raw_img);
        if src.is_file() {
            if let Some(parent) = plate.parent() {
                std::fs::create_dir_all(parent)?;
            }
            super::cut::photocraft_run(
                src,
                &[("image.imageSize", json!({"width": size.0, "height": size.1})), ("file.saveACopy", json!({"path": &plate, "overwrite": true}))],
                None,
            )?;
            return Ok(Some(plate));
        }
    }
    Ok(plate.is_file().then_some(plate))
}

fn box_polygon(b: &[Value], size: (u32, u32)) -> Option<Value> {
    if b.len() != 4 {
        return None;
    }
    let x = b[0].as_f64()?.round() as i64;
    let y = b[1].as_f64()?.round() as i64;
    let w = b[2].as_f64()?.round() as i64;
    let h = b[3].as_f64()?.round() as i64;
    let x1 = (x + w).clamp(0, i64::from(size.0));
    let y1 = (y + h).clamp(0, i64::from(size.1));
    let x0 = x.clamp(0, x1);
    let y0 = y.clamp(0, y1);
    Some(json!([[x0, y0], [x1, y0], [x1, y1], [x0, y1]]))
}

/// Curated outlines for `disc-parent-coral` (`340x353`) so its dorsal fin, tail, round felt disc + head
/// body plate, eye, and pectoral fin separate cleanly along the craft seams without clipping fins.
fn disc_parent_coral_outline(id: &str) -> Option<Value> {
    match id {
        "fin_dorsal" => Some(json!([[70, 148], [69, 90], [88, 22], [115, 2], [156, 2], [202, 36], [228, 69], [200, 74], [145, 92], [102, 128]])),
        "tail" => Some(json!([[2, 114], [48, 138], [86, 168], [92, 187], [84, 205], [38, 250], [2, 250]])),
        "body" => Some(json!([
            [64, 174],
            [85, 135],
            [128, 92],
            [182, 69],
            [225, 65],
            [262, 74],
            [305, 102],
            [332, 145],
            [339, 178],
            [332, 215],
            [306, 260],
            [265, 286],
            [240, 322],
            [210, 322],
            [218, 290],
            [168, 325],
            [110, 351],
            [86, 344],
            [74, 280],
            [75, 224],
            [64, 196]
        ])),
        "eye" => Some(json!([[266, 140], [283, 134], [300, 140], [307, 157], [300, 174], [283, 181], [266, 174], [259, 157]])),
        "fin_pectoral_near" => Some(json!([[175, 195], [208, 190], [235, 194], [244, 206], [234, 226], [205, 258], [176, 262], [160, 244], [156, 218]])),
        _ => None,
    }
}

/// Convert a Studio `parts/rig.json` into `parts.json` marks without needing Codex.
fn import_rig_marks(s: &mut Session, view: &View, p: &Value) -> Result<Option<Value>> {
    let Some(char_dir) = view.dir.parent() else {
        return Ok(None);
    };
    let rig_path = char_dir.join("parts/rig.json");
    if !rig_path.is_file() || p["use_codex"] == true {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&rig_path)?;
    let rig: Value = serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", rig_path.display())))?;
    let (staged, size) = stage_image(view, p, s)?;
    let plate = ensure_redrawn_body_plate(s, view, size, p)?;
    let anim_dir = char_dir.join("parts/animation-layers");
    let has_clean_anim_layers =
        anim_dir.join("scale_row_01.png").is_file() || (plate.is_none() && anim_dir.join("body.png").is_file() && view.id != "disc_parent_coral");
    let layers = rig["layers"].as_array().cloned().unwrap_or_default();
    let seg_layers = &rig["segmented_source"]["layers"];
    let skeleton: Value = serde_json::from_str(POSES).map_err(|e| bad(e.to_string()))?;
    let known = skeleton["parts"].as_array().cloned().unwrap_or_default();
    let mut parts_arr = Vec::new();
    for layer in &layers {
        let Some(id) = layer["id"].as_str() else { continue };
        if !known.contains(&json!(id)) {
            continue;
        }
        if !has_clean_anim_layers && id == "pearl_fibres" {
            continue;
        }
        let parent = layer.get("parent").cloned().unwrap_or(Value::Null);
        let joint = point(&layer["pivot"]).or_else(|| point(&seg_layers[id]["pivot"])).unwrap_or([f64::from(size.0) / 2.0, f64::from(size.1) / 2.0]);
        let mut entry = json!({
            "id": id,
            "parent": parent,
            "joint": [joint[0].round(), joint[1].round()],
        });
        if has_clean_anim_layers {
            let candidate = if id == "body" && anim_dir.join("body-fabric-v2.png").is_file() {
                anim_dir.join("body-fabric-v2.png")
            } else {
                anim_dir.join(format!("{id}.png"))
            };
            if candidate.is_file() {
                entry["source"] = json!(candidate);
            }
        }
        let outline = if view.id == "disc_parent_coral" { disc_parent_coral_outline(id) } else { None }
            .or_else(|| seg_layers[id]["box"].as_array().and_then(|b| box_polygon(b, size)))
            .unwrap_or_else(|| json!([[0, 0], [size.0, 0], [size.0, size.1], [0, size.1]]));
        entry["outline"] = outline;
        parts_arr.push(entry);
    }
    if parts_arr.is_empty() {
        return Ok(None);
    }
    let mut doc = json!({
        "image": staged.file_name().map(|n| n.to_string_lossy().into_owned()),
        "view": view.view,
        "size": [size.0, size.1],
        "skeleton": skeleton["skeleton"],
        "status": "imported from parts/rig.json; ready for character.cut",
        "parts": parts_arr,
    });
    if let Some(plate_path) = plate {
        doc["body_plate"] = json!(plate_path);
    }
    view.write_parts(&doc)?;
    Ok(Some(doc))
}

pub fn cmd_character_mark(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let parts = if let Some(imported) = import_rig_marks(s, &view, p)? {
        imported
    } else {
        let (staged, size) = stage_image(&view, p, s)?;
        let _ = ensure_redrawn_body_plate(s, &view, size, p)?;
        let timeout = p["timeout_s"].as_u64().unwrap_or(600);
        let codex = crate::workspace::resolve_tool("codex");
        let run = crate::agent_run::run_agent(&codex, &codex_args(&view, &staged, size), &view.dir, timeout)?;
        if !run.succeeded() {
            return Err(EngineError::ExternalTool("codex".into(), format!("Scribbles did not finish marking: {}", run.failure_detail())));
        }
        check_marks(&view, &staged)?
    };
    let mut reply = json!({
        "ok": true,
        "parts": view.dir.join("parts.json"),
        "marked": parts["parts"],
        "body_plate": parts.get("body_plate"),
        "next": format!("review the marks, then character.cut character={} view={}", view.id, view.view),
    });
    if p["cut"] == true || p["rig"] == true {
        let mut cut_params = p.clone();
        if let Some(obj) = cut_params.as_object_mut() {
            obj.remove("parts");
        }
        let cut_res = super::cmd_character_cut(s, &cut_params)?;
        reply["psd"] = cut_res["psd"].clone();
        reply["pcraft"] = cut_res["pcraft"].clone();
        reply["ecproj"] = cut_res["ecproj"].clone();
        reply["next"] = cut_res["next"].clone();
    }
    Ok(reply)
}
