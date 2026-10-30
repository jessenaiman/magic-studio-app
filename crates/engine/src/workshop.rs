//! The song workshop: four stages (song, transcript, reference, animate), stage status,
//! studio intent, and unified project search.
//!
//! Related workshop submodules:
//! - [`crate::workshop_intake`]: Audio/target linking, file intake, panel splitting, draft approval, and file catalog
//! - [`crate::workshop_transcript`]: Stage 2 transcript validation, SRT captions, and HyperFrames audio/beat analysis
//! - [`crate::workshop_fixlist`]: Artwork layer assessment (`assets.assess`) and the asset fix list
//! - [`crate::storyboard`]: Storyboard chapters and DeckCraft PDF generation
//! - [`crate::video_target`]: Video target scaffolding and preview servers

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{EngineError, Result, Session};

use crate::control_bindings::CONTROLS;
use crate::storyboard::{SceneDoc, clock, panel_spans, read_scene_doc, scene_doc_path, scenes};
use crate::video_target::{
    target_project_chip, target_project_path, video_server_info,
};
use crate::workspace::{has_extension, run_tool_in, workspace_root};

// Re-export storyboard, video target, intake, transcript, and fix-list handlers so `crate::workshop::*` callers continue to work.
pub use crate::storyboard::{
    cmd_storyboard_approve, cmd_storyboard_edit, cmd_storyboard_import_magicstudio,
    cmd_storyboard_init, cmd_storyboard_read, cmd_storyboard_toc, cmd_workshop_build_storyboard,
    cmd_workshop_storyboard,
};
pub use crate::video_target::{
    cmd_video_init, cmd_video_place_track, cmd_video_render, cmd_video_server_start,
    cmd_video_server_stop, cmd_video_snapshot,
};
pub use crate::workshop_fixlist::{
    cmd_assets_assess, cmd_assets_fixlist, cmd_assets_fixlist_add, cmd_assets_request_sheet,
};
pub use crate::workshop_intake::{
    cmd_workshop_add, cmd_workshop_approve_draft, cmd_workshop_files, cmd_workshop_remove,
    cmd_workshop_set_audio, cmd_workshop_set_target, cmd_workshop_split_panels,
};
pub(crate) use crate::workshop_transcript::{transcript_document, transcript_problems};
pub use crate::workshop_transcript::{cmd_audio_analyze, cmd_workshop_import_transcript};

/// Image file extensions recognized by the workshop.
pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
/// Storyboard template inside a song folder: `storyboard/panel-NN.png (+ .prompt.md)`, `storyboard.pdf`.
pub(crate) const STORYBOARD_DIR: &str = "storyboard";
pub(crate) const STORYBOARD_PDF: &str = "storyboard.pdf";
/// Static images for video tracks (characters, props, backgrounds), ideally transparent.
pub(crate) const ASSETS_DIR: &str = "assets";
/// Video targets the song can be edited in; Remotion is the default.
pub const VIDEO_TARGETS: &[&str] = &["remotion", "filmcraft", "hyperframes"];
/// FilmCraft time base: ticks per second.
pub(crate) const SEQUENCE_TICKS_PER_SECOND: f64 = 254_016_000_000.0;
/// Asset fix list: one decision per problem asset ("clean" or "recreate").
pub(crate) const FIXLIST: &str = "asset-fixes.json";

pub(crate) fn song_seconds(dir: &Path) -> Option<f64> {
    if let Some(duration) = std::fs::read(dir.join("transcript-provenance.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["recording_duration_sec"].as_f64())
        .filter(|v| v.is_finite() && *v > 0.0)
    {
        return Some(duration);
    }
    let fcproj = video_project_path(dir)?;
    let out = run_tool_in(
        dir,
        "filmcraft-cli",
        &[
            "--project".into(),
            fcproj.to_string_lossy().into_owned(),
            "inspect".into(),
            "sequence".into(),
        ],
    )
    .ok()?;
    let seq: Value = serde_json::from_str(&out).ok()?;
    seq.get("duration")
        .and_then(Value::as_f64)
        .map(|t| t / SEQUENCE_TICKS_PER_SECOND)
}

/// The studio's own words, kept in data the user edits (never in code).
#[derive(Clone, Default)]
pub struct Studio {
    pub name: Option<String>,
    pub intent: Option<String>,
    pub source: Option<PathBuf>,
}

/// Read the studio intent from `intent.md` in the song folder, else `STUDIO.md` beside projects.
pub fn studio(dir: &Path) -> Studio {
    let candidates = [
        Some(dir.join("intent.md")),
        dir.parent().map(|p| p.join("STUDIO.md")),
    ];
    for path in candidates.into_iter().flatten() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let mut name = None;
            let mut body = Vec::new();
            for line in text.lines() {
                match line.strip_prefix("# ") {
                    Some(h) if name.is_none() => name = Some(h.trim().to_string()),
                    _ => body.push(line),
                }
            }
            let intent = body.join("\n").trim().to_string();
            return Studio {
                name,
                intent: (!intent.is_empty()).then_some(intent),
                source: Some(path),
            };
        }
    }
    Studio {
        name: None,
        intent: None,
        source: None,
    }
}

/// Skill files an agent should read for a stage.
fn skill_hints(s: &Session, dir: &Path, names: &[&str]) -> String {
    let roots = [
        Some(workspace_root(s).join(".agents/skills")),
        dir.parent()
            .and_then(Path::parent)
            .map(|p| p.join(".agents/skills")),
    ];
    let found: Vec<String> = names
        .iter()
        .filter_map(|n| {
            roots
                .iter()
                .flatten()
                .map(|r| r.join(n).join("SKILL.md"))
                .find(|p| p.is_file())
        })
        .map(|p| p.display().to_string())
        .collect();
    if found.is_empty() {
        String::new()
    } else {
        format!("\nSkills: {}", found.join(", "))
    }
}

/// What an agent needs to start a stage.
fn stage_brief(s: &Session, dir: &Path, stage: &str) -> String {
    format!("{}\n{}", brief_header(dir), stage_body(s, dir, stage))
}

pub(crate) fn brief_header(dir: &Path) -> String {
    let project = dir.display();
    let studio = studio(dir);
    let intent = match (&studio.intent, &studio.source) {
        (Some(intent), Some(src)) => format!("{intent}\n(from {})\n", src.display()),
        _ => format!(
            "No studio intent yet: write it in {project}/intent.md or STUDIO.md beside the projects.\n"
        ),
    };
    format!(
        "{intent}Project folder: {project}\nWorkshop state: magic-cli exec workshop.status (run project.open path=<the .magic> first)\nStoryboard (read it first): {project}/{STORYBOARD_DIR}/{STORYBOARD_PDF} — pdfcraft-cli text <pdf> --page N\n"
    )
}

pub(crate) fn stage_body(s: &Session, dir: &Path, stage: &str) -> String {
    let target = s.video_target.as_str();
    match stage {
        "song" => "Stage 1 — Song. Link the song audio (scaffolds hyperframes/ and tags data-timeline-role=\"music\") and optional liner notes.\n  magic-cli exec workshop.set_audio path=<song>\n  magic-cli exec workshop.add kind=liner_notes paths='[\"<file>\"]'".to_string(),
        "transcript" => format!(
            "Stage 2 — Transcription & music analysis. Transcribe word timings, export captions, and analyze sections/beats.\n  magic-cli exec audio.analyze\nAlready have a transcript? magic-cli exec workshop.import_transcript path=<file> captions=<srt>\nVideo target: {target}.{}",
            skill_hints(s, dir, &[match target { "hyperframes" => "hyperframes-cli", "filmcraft" => "filmcraft", _ => "remotion-best-practices" }])
        ),
        "reference" => format!(
            "Stage 3 — Storyboard & scene art. Initialize and populate the DeckCraft storyboard and 16:9 scene plates (`storyboard/scene-N.md` + `assets/backgrounds/`).\n  magic-cli exec storyboard.init\n  magic-cli exec storyboard.read scene=1\n  magic-cli exec storyboard.edit scene=1 field=action value=\"...\"\n  magic-cli exec workshop.build_storyboard\n  magic-cli exec words.apply{}",
            skill_hints(s, dir, &["storyboard"])
        ),
        _ => format!(
            "Stage 4 — Animate assets. One animated subject at a time over its background; not every asset animates.\nExport layered files for the video target ({target}); never render or encode video.{}",
            skill_hints(s, dir, &["layered-motion", "character-rigging", "image-to-animation"])
        ),
    }
}

/// Files in `dir` (not recursive), sorted, optionally limited to some extensions.
pub(crate) fn files_in(dir: &Path, only: Option<&[&str]>) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && !p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with('.'))
        })
        .filter(|p| only.is_none_or(|exts| has_extension(p, exts)))
        .collect();
    files.sort();
    files
}

pub(crate) fn transcript_files(dir: &Path) -> Vec<PathBuf> {
    let native = dir.join("hyperframes/transcript.json");
    if native.is_file() {
        return vec![native];
    }
    files_in(dir, Some(&["json"]))
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".transcript.json"))
        })
        .collect()
}

/// Storyboard panels, in order: `storyboard/panel-NN.`.
pub(crate) fn panel_files(dir: &Path) -> Vec<PathBuf> {
    files_in(&dir.join(STORYBOARD_DIR), Some(IMAGE_EXTENSIONS))
        .into_iter()
        .filter(|p| {
            p.file_stem()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("panel-"))
        })
        .collect()
}

pub(crate) fn video_project_path(dir: &Path) -> Option<PathBuf> {
    transcript_files(dir)
        .iter()
        .find_map(|t| {
            let name = t
                .file_name()?
                .to_str()?
                .replace(".transcript.json", ".fcproj");
            let fcproj = t.with_file_name(name);
            fcproj.is_file().then_some(fcproj)
        })
        .or_else(|| files_in(dir, Some(&["fcproj"])).into_iter().next())
}

/// Static and cutout image assets in the song project folder.
pub(crate) fn workshop_asset_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut push_unique = |paths: Vec<PathBuf>| {
        for p in paths {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    };
    push_unique(files_in(&dir.join(ASSETS_DIR), Some(IMAGE_EXTENSIONS)));
    push_unique(files_in(
        &dir.join(ASSETS_DIR).join("images"),
        Some(IMAGE_EXTENSIONS),
    ));
    push_unique(files_in(
        &dir.join(ASSETS_DIR).join("characters/approved-crops"),
        Some(IMAGE_EXTENSIONS),
    ));
    if let Ok(chars) = std::fs::read_dir(dir.join(ASSETS_DIR).join("characters")) {
        let mut char_dirs: Vec<PathBuf> = chars
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        char_dirs.sort();
        for cdir in char_dirs {
            push_unique(files_in(&cdir.join("parts"), Some(IMAGE_EXTENSIONS)));
        }
    }
    if let Ok(bgs) = std::fs::read_dir(dir.join(ASSETS_DIR).join("backgrounds")) {
        let mut bg_dirs: Vec<PathBuf> = bgs
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        bg_dirs.sort();
        for bdir in bg_dirs {
            push_unique(files_in(&bdir, Some(IMAGE_EXTENSIONS)));
        }
    }
    push_unique(files_in(&dir.join("layer-sources"), Some(IMAGE_EXTENSIONS)));
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut subdirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_dir()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with("magicstudio-"))
            })
            .collect();
        subdirs.sort();
        for sub in subdirs {
            push_unique(files_in(&sub.join("parts"), Some(IMAGE_EXTENSIONS)));
            push_unique(files_in(&sub.join("reference"), Some(IMAGE_EXTENSIONS)));
        }
    }
    out
}

fn file_chip(path: &Path) -> Value {
    json!({"name": path.file_name().and_then(|n| n.to_str()), "path": path, "exists": path.exists()})
}

/// The open project's directory; errors when no project is open.
pub(crate) fn project_dir(s: &Session) -> Result<PathBuf> {
    s.project_path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            EngineError::InvalidParams("no project is open; use project.new or project.open".into())
        })
}

fn workshop_transcript_summary(
    dir: &Path,
    video_target: &str,
    transcripts: &[PathBuf],
    loaded_board: &Value,
    target_chip: &Value,
) -> (Value, Vec<String>, bool, String, Vec<Value>) {
    let source = transcripts.first().cloned().or_else(|| {
        loaded_board["source_transcript"].as_str().map(PathBuf::from)
    });
    let doc = source
        .as_ref()
        .and_then(|p| transcript_document(p).ok())
        .unwrap_or(Value::Null);
    let mut errors = transcript_problems(&doc);
    if loaded_board["timing_valid"] == false {
        errors.push(
            loaded_board["timing_warning"]
                .as_str()
                .unwrap_or("Storyboard timing has not passed validation")
                .to_string(),
        );
    }
    let valid = errors.is_empty();
    let why = if source.is_none() {
        "Import existing timed lyrics, or transcribe the linked song.".to_string()
    } else if !valid {
        format!("Transcript needs repair: {}. Original inputs are retained.", errors.join("; "))
    } else if let Some(src) = loaded_board["source_transcript"].as_str() {
        let label = Path::new(src).parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("unknown");
        format!(
            "Storyboard uses native output {src} directly ({} words); no transcription rerun. Archive label: {label} (producer executable/version not recorded). {}",
            loaded_board["source_words"],
            loaded_board["timing_warning"].as_str().unwrap_or("")
        )
    } else if let Some(tool) = doc["source"].as_str() {
        format!("Reused native transcript; recorded tool: {tool}; recorded model: {}. No transcription run in this import.", doc["model"].as_str().unwrap_or("unknown"))
    } else {
        "Transcript ready; producer unknown because the supplied file did not record it.".to_string()
    };
    let mut outputs: Vec<Value> = transcripts.iter().map(|p| file_chip(p)).collect();
    for name in ["transcript.json", "transcript.srt", "audio-analysis.json", "audiomap.json", "STORYBOARD.md"] {
        let file = dir.join(video_target).join(name);
        if file.is_file() {
            outputs.push(file_chip(&file));
        }
    }
    outputs.push(target_chip.clone());
    (doc, errors, valid, why, outputs)
}

fn workshop_animated_assets(s: &Session) -> Vec<Value> {
    s.project_assets
        .iter()
        .map(|a| {
            let rig = s.assets.get(&a.id).and_then(|x| x.cutout_rig.as_ref());
            let parts = rig.map(|r| r.parts.len());
            let tracks = rig.and_then(|r| r.effectcraft.get("tracks")).and_then(Value::as_array).map(|t| t.len());
            let controls: Vec<&str> = rig
                .map(|r| CONTROLS.iter().copied().filter(|c| r.control_bindings.supports(c)).collect())
                .unwrap_or_default();
            json!({"id": a.id, "name": a.name, "kind": a.kind, "path": a.path, "parts": parts, "tracks": tracks, "controls": controls, "animatable": rig.is_some_and(|r| r.parts.len() >= 2)})
        })
        .collect()
}

fn workshop_reference_state(s: &Session, dir: &Path, loaded_board: &Value, panels: &[PathBuf]) -> (PathBuf, bool, bool, &'static str) {
    let pdf = loaded_board["pdf"]
        .as_str()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .unwrap_or_else(|| dir.join(STORYBOARD_DIR).join(STORYBOARD_PDF));
    let scene_docs: Vec<SceneDoc> = (1..)
        .map(|n| (n, scene_doc_path(dir, n)))
        .take_while(|(_, p)| p.is_file())
        .map(|(n, _)| read_scene_doc(dir, n))
        .collect();
    let has_ref_art = !panels.is_empty() || !scene_docs.is_empty();
    let approved = pdf.is_file()
        && (s.draft_approved || (!scene_docs.is_empty() && scene_docs.iter().all(|d| d.status == "approved")));
    let why = match (has_ref_art, pdf.is_file(), approved) {
        (false, _, _) => "Initialize the storyboard (`storyboard.init`) or add scene art.",
        (true, false, _) => "Build the storyboard PDF (`workshop.build_storyboard`).",
        (true, true, false) => "Review the storyboard and scene plates, then approve (`workshop.approve_draft`).",
        (true, true, true) => "Storyboard approved.",
    };
    (pdf, has_ref_art, approved, why)
}

/// Workshop status: what each stage needs, whether complete, asset riggability, server state.
pub(crate) fn workshop_status(s: &Session) -> Result<Value> {
    let dir = project_dir(s)?;
    let target = s.video_target.as_str();
    let audio_ok = s.song_audio.as_ref().is_some_and(|p| p.is_file());
    let liner = files_in(&dir.join("liner-notes"), None);
    let transcripts = transcript_files(&dir);
    let panels = panel_files(&dir);
    let assets = workshop_asset_files(&dir);
    let loaded_board = std::fs::read(dir.join("storyboard/movie-board.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .unwrap_or(Value::Null);
    let (pdf, has_ref_art, approved, reference_why) = workshop_reference_state(s, &dir, &loaded_board, &panels);
    let panel_timing = transcripts
        .first()
        .and_then(|t| transcript_document(t).ok())
        .map(|t| panel_spans(&scenes(&t, None), panels.len()));
    let song_why = match (audio_ok, liner.is_empty()) {
        (false, _) => "Choose the song's audio.",
        (true, true) => "Song audio linked and ready (liner notes optional).",
        (true, false) => "Song and liner notes are in place.",
    };
    let active_target_path = target_project_path(s, &dir, target);
    let target_chip = target_project_chip(s, &dir, target);
    let target_project = json!({"target": target, "path": active_target_path, "ready": active_target_path.is_some(), "chip": target_chip});
    let targets: Vec<Value> = VIDEO_TARGETS
        .iter()
        .map(|t| {
            let path = target_project_path(s, &dir, t);
            json!({"target": t, "selected": *t == target, "path": path, "exists": path.is_some()})
        })
        .collect();
    let (supplied_transcript, transcript_errors, transcript_valid, transcript_why, transcript_outputs) =
        workshop_transcript_summary(&dir, target, &transcripts, &loaded_board, &target_chip);
    let animated = workshop_animated_assets(s);
    let stages = vec![
        json!({"id": "song", "title": "Song", "done": audio_ok, "why": song_why,
            "inputs": s.song_audio.iter().map(|p| file_chip(p)).chain(liner.iter().map(|p| file_chip(p))).collect::<Vec<_>>(),
            "outputs": vec![target_chip], "brief": stage_brief(s, &dir, "song")}),
        json!({"id": "transcript", "title": "Transcription & transcoding", "done": transcript_valid, "why": transcript_why,
            "inputs": s.song_audio.iter().map(|p| file_chip(p)).collect::<Vec<_>>(), "outputs": transcript_outputs,
            "document": supplied_transcript, "problems": transcript_errors, "target": target_project, "brief": stage_brief(s, &dir, "transcript")}),
        json!({"id": "reference", "title": "Reference art", "done": has_ref_art && approved, "why": reference_why,
            "inputs": panels.iter().enumerate().map(|(i, p)| {
                let mut chip = file_chip(p);
                if let Some(spans) = panel_timing.as_ref().and_then(|t| t.get(i)) {
                    chip["begins"] = json!(spans.iter().map(|(a, _)| clock(*a)).collect::<Vec<_>>());
                }
                chip
            }).collect::<Vec<_>>(),
            "outputs": std::iter::once(file_chip(&pdf)).chain(assets.iter().map(|p| file_chip(p))).collect::<Vec<_>>(),
            "approved": approved, "pdf": pdf, "brief": stage_brief(s, &dir, "reference")}),
        json!({"id": "animate", "title": "Animate assets", "done": !animated.is_empty(), "optional": true,
            "why": if animated.is_empty() { "Drag an asset into Stage 4 to create an animated subject (static scenes need none)." } else { "Pick an animated subject to rig or preview; static assets stay on video tracks." },
            "inputs": assets.iter().map(|p| file_chip(p)).collect::<Vec<_>>(), "assets": animated, "brief": stage_brief(s, &dir, "animate")}),
    ];
    let complete = stages.iter().take(3).all(|st| st["done"] == json!(true));
    let studio_info = studio(&dir);
    let effective_video_project = s.video_project_path.clone().or_else(|| active_target_path.clone());
    let diag = crate::storyboard::diagnose_storyboard_and_workshop(s, &dir);
    Ok(json!({
        "ok": true,
        "project_dir": dir,
        "complete": complete,
        "video_target": target,
        "targets": targets,
        "video_server": video_server_info(&dir, target),
        "asset_files": assets.iter().map(|p| file_chip(p)).collect::<Vec<_>>(),
        "studio": {"name": studio_info.name, "intent": studio_info.intent, "source": studio_info.source},
        "video_project_path": effective_video_project,
        "video_project_exists": effective_video_project.as_ref().is_some_and(|p| p.exists()),
        "diagnostics": diag,
        "next_steps": diag["next_steps"],
        "stages": stages
    }))
}

/// List every registered command, optionally filtered by id/label/params substring.
pub fn cmd_query_list(_s: &mut Session, params: &Value) -> Result<Value> {
    let f = params
        .get("filter")
        .or_else(|| params.get("query"))
        .and_then(Value::as_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let list: Vec<Value> = crate::commands::command_specs()
        .iter()
        .filter(|c| {
            f.is_empty()
                || c.id.to_ascii_lowercase().contains(&f)
                || c.label.to_ascii_lowercase().contains(&f)
                || c.params.to_ascii_lowercase().contains(&f)
        })
        .map(|c| c.describe())
        .collect();
    Ok(json!({"count": list.len(), "commands": list}))
}

/// Unified CLI search across commands, connected subjects, workshop files, and storyboard scenes.
pub fn cmd_project_search(s: &mut Session, params: &Value) -> Result<Value> {
    let q = params
        .get("query")
        .or_else(|| params.get("filter"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    let q_lower = q.to_ascii_lowercase();

    let commands: Vec<Value> = crate::commands::command_specs()
        .iter()
        .filter(|c| {
            q_lower.is_empty()
                || c.id.to_ascii_lowercase().contains(&q_lower)
                || c.label.to_ascii_lowercase().contains(&q_lower)
                || c.params.to_ascii_lowercase().contains(&q_lower)
        })
        .map(|c| json!({"id": c.id, "label": c.label, "params": c.params}))
        .collect();

    let subjects: Vec<Value> = s
        .project_assets
        .iter()
        .filter(|a| {
            q_lower.is_empty()
                || a.id.to_ascii_lowercase().contains(&q_lower)
                || a.name.to_ascii_lowercase().contains(&q_lower)
                || a.path.as_ref().is_some_and(|p| {
                    p.display()
                        .to_string()
                        .to_ascii_lowercase()
                        .contains(&q_lower)
                })
        })
        .map(|a| {
            let parts: Vec<String> = s
                .assets
                .get(&a.id)
                .map(|loaded| loaded.parts.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();
            json!({
                "id": a.id,
                "name": a.name,
                "kind": a.kind,
                "path": a.path,
                "parts": parts,
            })
        })
        .collect();

    let files = cmd_workshop_files(s, &json!({"filter": q}))
        .ok()
        .and_then(|v| v.get("files").cloned())
        .unwrap_or_else(|| json!([]));

    let storyboard_hits = if !q.is_empty() {
        crate::storyboard_crud::search(s, q)
            .ok()
            .and_then(|v| v.get("hits").cloned())
            .unwrap_or_else(|| json!([]))
    } else {
        cmd_storyboard_toc(s, &json!({}))
            .ok()
            .and_then(|v| v.get("chapters").cloned())
            .unwrap_or_else(|| json!([]))
    };

    Ok(json!({
        "ok": true,
        "query": q,
        "project": s.project_path,
        "active_asset": s.active_asset,
        "subjects": subjects,
        "files": files,
        "storyboard": storyboard_hits,
        "commands": commands,
    }))
}

pub fn cmd_workshop_status(s: &mut Session, _: &Value) -> Result<Value> {
    workshop_status(s)
}
