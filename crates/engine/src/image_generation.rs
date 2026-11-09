//! Image generation through Craft AI (`codex`, `agy`, and `flow` subscription/browser adapters).
//! Magic never sends prompts to an unconfigured paid image API endpoint.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidParams(message.into())
}

pub(crate) fn validated_png(path: &Path) -> Result<(u32, u32, String)> {
    let mut bytes = fs::read(path).map_err(|e| invalid(&format!("Cannot read image: {e}")))?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
        if let Ok(decoded) = image::load_from_memory(&bytes) {
            decoded.save_with_format(path, image::ImageFormat::Png).map_err(|e| invalid(&format!("Failed to normalize image to PNG: {e}")))?;
            bytes = fs::read(path).map_err(|e| invalid(&format!("Cannot read normalized PNG: {e}")))?;
        } else {
            return Err(invalid("Generated output is not a valid PNG"));
        }
    }
    let (w, h) = image::image_dimensions(path).map_err(|e| invalid(&format!("Cannot read image dimensions: {e}")))?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16_777_216 {
        return Err(invalid("PNG has invalid or unsupported dimensions"));
    }
    Ok((w, h, format!("{:x}", Sha256::digest(&bytes))))
}

fn prepare_output_dir(s: &Session) -> Result<(PathBuf, PathBuf)> {
    let project = s.project_path.clone().ok_or_else(|| invalid("Open a project before generating images, or pass project=<path>"))?;
    let root = project.parent().ok_or_else(|| invalid("Project has no parent directory"))?;
    let parent = root.join("assets/generated-images");
    fs::create_dir_all(&parent).map_err(|e| invalid(&format!("Failed to create asset dir: {e}")))?;

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| invalid(&e.to_string()))?.as_nanos();
    let dir = parent.join(format!("image-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| invalid(&format!("Failed to create asset dir: {e}")))?;

    Ok((dir.join("asset.png"), dir.join("generation.json")))
}

/// Every PNG under Codex's image cache (`$CODEX_HOME` or `~/.codex`, `generated_images/*/`).
fn codex_cached_images() -> Result<std::collections::HashSet<PathBuf>> {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".codex")))
        .ok_or_else(|| invalid("neither CODEX_HOME nor HOME is set, so the Codex image cache cannot be found"))?;
    let mut found = std::collections::HashSet::new();
    let Ok(entries) = fs::read_dir(home.join("generated_images")) else {
        return Ok(found);
    };
    for dir in entries.flatten().filter(|e| e.path().is_dir()) {
        for file in fs::read_dir(dir.path())?.flatten() {
            if file.path().extension().is_some_and(|e| e == "png") {
                found.insert(file.path());
            }
        }
    }
    Ok(found)
}

/// Every generated artifact image under AGY's brain cache (`~/.gemini/antigravity-cli/brain/*/`).
fn agy_cached_images() -> Result<std::collections::HashSet<PathBuf>> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| invalid("HOME is not set, so the AGY image cache cannot be found"))?;
    let brain = home.join(".gemini/antigravity-cli/brain");
    let mut found = std::collections::HashSet::new();
    let Ok(entries) = fs::read_dir(brain) else {
        return Ok(found);
    };
    for conv in entries.flatten().filter(|e| e.path().is_dir()) {
        let Ok(files) = fs::read_dir(conv.path()) else {
            continue;
        };
        for file in files.flatten() {
            let p = file.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
            {
                found.insert(p);
            }
        }
    }
    Ok(found)
}

/// Draw with Codex's built-in image tool, attaching `references` as images. Only a PNG this run
/// created counts: when Codex finishes without drawing, that is an error, never an older image.
pub(crate) fn generate_via_guarded_codex(model: &str, prompt: &str, references: &[PathBuf], dir: &Path, out: &Path) -> Result<()> {
    let before = codex_cached_images()?;
    let mut cmd = Command::new("codex");
    cmd.args(["exec", "--ephemeral", "--skip-git-repo-check", "--sandbox", "workspace-write", "--model", model, "--cd"])
        .arg(dir)
        .args(["--config", "model_reasoning_effort=\"low\""]);
    for reference in references {
        cmd.arg("--image").arg(reference);
    }
    let attached = if references.is_empty() { String::new() } else { " Match the attached reference image(s) for character design and finish.".into() };
    cmd.arg("--")
        .arg(format!("Use your native built-in image_gen.imagegen tool exactly once to generate: {prompt}.{attached} Do not retry or run other tools."));

    crate::workspace::scrub_inherited_credentials(&mut cmd);
    let output = cmd.output().map_err(|e| EngineError::ExternalTool("codex".into(), format!("Failed to launch codex: {e}")))?;
    if !output.status.success() {
        return Err(invalid(&format!("Codex image execution failed (exit {:?}): {}", output.status.code(), String::from_utf8_lossy(&output.stderr))));
    }
    let mut made: Vec<PathBuf> = codex_cached_images()?.difference(&before).cloned().collect();
    made.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    let src = made.pop().ok_or_else(|| {
        invalid(&format!("Codex finished without drawing an image: {}", String::from_utf8_lossy(&output.stdout).lines().last().unwrap_or("no output")))
    })?;
    fs::copy(&src, out).map_err(|e| invalid(&format!("Failed to copy PNG: {e}")))?;
    Ok(())
}

/// Draw with AGY (`gemini-3.8-flash-medium` / Flash) using its built-in `image-generator` subagent,
/// attaching `references` as reference paths and converting the newly generated artifact to PNG at `out`.
pub(crate) fn generate_via_guarded_agy(model: &str, prompt: &str, references: &[PathBuf], dir: &Path, out: &Path) -> Result<()> {
    let before = agy_cached_images()?;
    let cli = crate::workspace::find_executable("agy").unwrap_or_else(|_| PathBuf::from("agy"));
    let effective_model = if model.trim().is_empty() || model == "gpt-6-luna" || model.starts_with("nano-banana") { "gemini-3.8-flash-medium" } else { model };
    let refs_clause = if references.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = references.iter().map(|p| format!("`{}`", p.display())).collect();
        format!(" Pass these reference image paths in ImagePaths to match character design and style: {}.", list.join(", "))
    };
    let instruction = format!(
        "Invoke the built-in `image-generator` subagent exactly once to generate a single image for this brief: {prompt}.{refs_clause} Copy the generated image to `{}` and reply with its path.",
        out.display()
    );
    let mut cmd = Command::new(&cli);
    cmd.args(["--model", effective_model, "--dangerously-skip-permissions", "--disable-slash-commands", "--add-dir"]).arg(dir).args(["-p", &instruction]);
    for reference in references {
        if let Some(parent) = reference.parent() {
            cmd.arg("--add-dir").arg(parent);
        }
    }
    crate::workspace::scrub_inherited_credentials(&mut cmd);
    let output = cmd.output().map_err(|e| EngineError::ExternalTool("agy".into(), format!("Failed to launch agy: {e}")))?;
    if !output.status.success() && !out.is_file() {
        return Err(invalid(&format!("AGY image execution failed (exit {:?}): {}", output.status.code(), String::from_utf8_lossy(&output.stderr))));
    }
    if !out.is_file() {
        let mut made: Vec<PathBuf> = agy_cached_images()?.difference(&before).cloned().collect();
        made.sort_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
        let src = made.pop().ok_or_else(|| {
            invalid(&format!("AGY finished without drawing an image: {}", String::from_utf8_lossy(&output.stdout).lines().last().unwrap_or("no output")))
        })?;
        fs::copy(&src, out).map_err(|e| invalid(&format!("Failed to copy AGY image: {e}")))?;
    }
    let _ = validated_png(out)?;
    Ok(())
}

/// Draw through the signed-in Google Flow tab (`flow-clip --mode image` via `agent-browser`).
pub(crate) fn generate_via_flow(prompt: &str, references: &[PathBuf], model: Option<&str>, out: &Path) -> Result<()> {
    let cli = crate::workspace::find_executable("flow-clip").map_err(|e| EngineError::ExternalTool("flow-clip".into(), e.to_string()))?;
    let mut cmd = Command::new(&cli);
    cmd.args(["--mode", "image", "--prompt", prompt, "--out"]).arg(out);
    if let Some(m) = model.filter(|s| !s.trim().is_empty() && *s != "gpt-6-luna" && !s.starts_with("gemini-")) {
        cmd.args(["--model", m]);
    }
    for reference in references {
        cmd.arg("--ingredient").arg(reference);
    }
    crate::workspace::scrub_inherited_credentials(&mut cmd);
    let output = cmd.output().map_err(|e| EngineError::ExternalTool("flow-clip".into(), format!("Failed to launch flow-clip: {e}")))?;
    if !output.status.success() {
        return Err(invalid(&format!(
            "Flow image generation failed (exit {:?}): {}{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        )));
    }
    Ok(())
}

/// Public engine entrypoint to generate a single image and register it in the session.
pub fn generate(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p["prompt"].as_str().map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| invalid("image.generate needs prompt=<description>"))?;

    let (out_png, receipt_path) = prepare_output_dir(s)?;

    let settings = crate::project_settings::stored_settings();
    let img_settings = &settings["image"];
    let scrib_settings = &settings["scribbles"];

    // The provider is the request's, image.provider's, or scribbles.provider's.
    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .or_else(|| img_settings["provider"].as_str().filter(|v| !v.trim().is_empty()))
        .or_else(|| scrib_settings["provider"].as_str().filter(|v| !v.trim().is_empty()))
        .ok_or_else(|| {
            invalid("no image provider is chosen: pass `provider` (codex, agy, or flow), or set it with `magic-cli exec scribbles.connect provider=codex`")
        })?;

    let raw_refs: Vec<&str> = if let Some(arr) = p["references"].as_array() {
        arr.iter().filter_map(Value::as_str).collect()
    } else if let Some(one) = p["reference"].as_str().filter(|v| !v.trim().is_empty()) {
        vec![one]
    } else {
        Vec::new()
    };
    let references: Vec<PathBuf> = raw_refs
        .into_iter()
        .map(|raw| {
            let path = crate::workspace::resolve_workspace_path(s, raw);
            if path.is_file() { Ok(path) } else { Err(invalid(&format!("reference image {} does not exist", path.display()))) }
        })
        .collect::<Result<_>>()?;

    let model_override = p["model"].as_str().filter(|v| !v.trim().is_empty());
    match provider {
        "codex" | "agy" | "flow" => crate::craft_ai_codex::draw_with(Some(provider), model_override, prompt, &references, &out_png)?,
        other => {
            return Err(invalid(&format!(
                "unknown image provider `{other}`: images are drawn through `codex` (Luna), `agy` (Flash), or `flow` (Browser-Agent)"
            )));
        }
    }

    let (width, height, sha256) = validated_png(&out_png)?;

    let attached = s.execute(
        "project.add_asset",
        json!({
            "path": out_png,
            "kind": "image",
            "remove_background": false,
            "name": p["name"].as_str().unwrap_or("Generated image"),
            "load": false
        }),
    )?;

    let record = json!({
        "ok": true,
        "format": "magic.image-generation",
        "provider": provider,
        "prompt": prompt,
        "image": out_png,
        "asset_id": attached["asset_id"],
        "width": width,
        "height": height,
        "sha256": sha256,
        "status": "ready"
    });

    let _ = crate::doc_state::write_file(&receipt_path, serde_json::to_vec_pretty(&record).unwrap_or_default());

    Ok(record)
}
