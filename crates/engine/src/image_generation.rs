//! Image generation through the Codex subscription service (`codex exec` and its built-in image
//! tool). There is no image API path: Magic never sends prompts to a paid image endpoint.
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

fn validated_png(path: &Path) -> Result<(u32, u32, String)> {
    let bytes = fs::read(path).map_err(|e| invalid(&format!("Cannot read image: {e}")))?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
        return Err(invalid("Generated output is not a valid PNG"));
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
    fs::create_dir_all(&dir).map_err(|e| invalid(&format!("Failed to create image dir: {e}")))?;

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

/// Draw with Codex's built-in image tool, attaching `references` as images. Only a PNG this run
/// created counts: when Codex finishes without drawing, that is an error, never an older image.
fn generate_via_guarded_codex(prompt: &str, references: &[PathBuf], dir: &Path, out: &Path) -> Result<()> {
    let before = codex_cached_images()?;
    let mut cmd = Command::new("codex");
    cmd.args(["exec", "--ephemeral", "--skip-git-repo-check", "--sandbox", "workspace-write", "--cd"])
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

/// Public engine entrypoint to generate a single image and register it in the session.
pub fn generate(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p["prompt"].as_str().map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| invalid("image.generate needs prompt=<description>"))?;

    let (out_png, receipt_path) = prepare_output_dir(s)?;
    let work_dir = out_png.parent().ok_or_else(|| invalid("invalid output image path"))?;

    let settings = crate::project_settings::stored_settings();
    let img_settings = &settings["image"];

    // The provider is the request's or the image.provider setting's; there is no implicit one.
    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .or_else(|| img_settings["provider"].as_str())
        .ok_or_else(|| invalid("no image provider is chosen: pass `provider`, or set it with `magic-cli exec prefs.set key=image.provider value=codex`"))?;
    let references: Vec<PathBuf> = p["references"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            let path = r.as_str().map(|raw| crate::workspace::resolve_workspace_path(s, raw)).ok_or_else(|| invalid("`references` are image paths"))?;
            if path.is_file() { Ok(path) } else { Err(invalid(&format!("reference image {} does not exist", path.display()))) }
        })
        .collect::<Result<_>>()?;

    match provider {
        "codex" => generate_via_guarded_codex(prompt, &references, work_dir, &out_png)?,
        // Images come from the Codex subscription service only, never a paid image API.
        other => return Err(invalid(&format!("unknown image provider `{other}`: images are drawn through the Codex service (provider=codex)"))),
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
