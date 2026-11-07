//! `character.mark`: Scribbles (Codex) looks at one drawing and writes `parts.json`: each part
//! of the shared skeleton as an outline and the joint it turns on. The marks are a draft for
//! review on the Studio canvas before `character.cut`.

use serde_json::{Value, json};

use super::{POSES, View, bad, part_list, path_arg, root_part};
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
        .map(|i| crate::workspace::resolve_workspace_path(s, i))
        .filter(|i| i.is_file())
        .ok_or_else(|| bad("image=<drawing.png> is required"))?;
    std::fs::create_dir_all(&view.dir)?;
    let staged = view.dir.join(format!("{}-{}-drawing.png", view.id, view.view));
    if source != staged {
        std::fs::copy(&source, &staged)?;
    }
    Ok((staged.clone(), image::image_dimensions(&staged)?))
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

pub fn cmd_character_mark(s: &mut Session, p: &Value) -> Result<Value> {
    let view = View::from_params(s, p)?;
    let (staged, size) = stage_image(&view, p, s)?;
    let timeout = p["timeout_s"].as_u64().unwrap_or(600);
    let codex = crate::workspace::resolve_tool("codex");
    let run = crate::agent_run::run_agent(&codex, &codex_args(&view, &staged, size), &view.dir, timeout)?;
    if !run.succeeded() {
        return Err(EngineError::ExternalTool("codex".into(), format!("Scribbles did not finish marking: {}", run.failure_detail())));
    }
    let parts = check_marks(&view, &staged)?;
    Ok(json!({"ok": true, "parts": view.dir.join("parts.json"), "marked": parts["parts"],
        "next": format!("review the marks, then character.cut character={} view={}", view.id, view.view)}))
}
