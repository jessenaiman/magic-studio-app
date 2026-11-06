//! Native image generation using direct HTTP (OpenAI / standard image endpoints) or guarded Codex.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn validated_png(path: &Path) -> Result<(u32, u32, String)> {
    let bytes = fs::read(path).map_err(|e| invalid(&format!("Cannot read image: {e}")))?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
        return Err(invalid("Generated output is not a valid PNG"));
    }
    let (w, h) = image::image_dimensions(path)
        .map_err(|e| invalid(&format!("Cannot read image dimensions: {e}")))?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16_777_216 {
        return Err(invalid("PNG has invalid or unsupported dimensions"));
    }
    Ok((w, h, format!("{:x}", Sha256::digest(&bytes))))
}

fn prepare_output_dir(s: &Session) -> Result<(PathBuf, PathBuf)> {
    let project = s.project_path.clone().ok_or_else(|| {
        invalid("Open a project before generating images, or pass project=<path>")
    })?;
    let root = project
        .parent()
        .ok_or_else(|| invalid("Project has no parent directory"))?;
    let parent = root.join("assets/generated-images");
    fs::create_dir_all(&parent)
        .map_err(|e| invalid(&format!("Failed to create asset dir: {e}")))?;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| invalid(&e.to_string()))?
        .as_nanos();
    let dir = parent.join(format!("image-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| invalid(&format!("Failed to create image dir: {e}")))?;

    Ok((dir.join("asset.png"), dir.join("generation.json")))
}

fn generate_via_openai_api(
    prompt: &str,
    key: &str,
    endpoint: Option<&str>,
    out: &Path,
) -> Result<()> {
    let url = endpoint.unwrap_or("https://api.openai.com/v1/images/generations");
    let payload = json!({
        "model": "dall-e-3",
        "prompt": prompt,
        "n": 1,
        "size": "1024x1024",
        "response_format": "b64_json"
    });

    let resp = ureq::post(url)
        .header("Authorization", &format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .config()
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .send_json(&payload)
        .map_err(|e| invalid(&format!("Direct API request failed: {e}")))?;

    let body: Value = serde_json::from_reader(resp.into_body().into_reader())
        .map_err(|e| invalid(&format!("Failed to parse API JSON: {e}")))?;

    let b64 = body["data"][0]["b64_json"]
        .as_str()
        .ok_or_else(|| invalid("API response missing b64_json image data"))?;

    let cleaned: String = b64.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = craft_ai_core::payload::base64_decode(&cleaned)
        .ok_or_else(|| invalid("Failed to decode base64 image data from API"))?;

    fs::write(out, bytes).map_err(|e| invalid(&format!("Failed to write PNG: {e}")))?;
    Ok(())
}

fn generate_via_guarded_codex(prompt: &str, dir: &Path, out: &Path) -> Result<()> {
    let mut cmd = Command::new("codex");
    cmd.args([
        "exec",
        "--ephemeral",
        "--skip-git-repo-check",
        "--sandbox",
        "workspace-write",
        "--cd",
    ])
    .arg(dir)
    .args(["--config", "model_reasoning_effort=\"low\""])
    .arg("--")
    .arg(format!(
        "Use your native built-in image_gen.imagegen tool exactly once to generate: {prompt}. Do not retry or run other tools."
    ));

    let output = cmd.output().map_err(|e| {
        EngineError::ExternalTool("codex".into(), format!("Failed to launch codex: {e}"))
    })?;

    if !output.status.success() {
        return Err(invalid(&format!(
            "Codex image execution failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // Check if staged in codex cache or output dir
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".codex")));

    if let Some(h) = home {
        // Look for newest generated image in ~/.codex/generated_images
        let gen_dir = h.join("generated_images");
        if let Ok(entries) = fs::read_dir(&gen_dir) {
            let mut newest: Option<(PathBuf, SystemTime)> = None;
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata()
                    && meta.is_dir()
                    && let Ok(files) = fs::read_dir(entry.path())
                {
                    for f in files.flatten() {
                        if f.path().extension().is_some_and(|e| e == "png")
                            && let Ok(f_meta) = f.metadata()
                            && let Ok(time) = f_meta.modified()
                            && newest.as_ref().is_none_or(|(_, t)| time > *t)
                        {
                            newest = Some((f.path(), time));
                        }
                    }
                }
            }
            if let Some((src, _)) = newest {
                fs::copy(&src, out).map_err(|e| invalid(&format!("Failed to copy PNG: {e}")))?;
                return Ok(());
            }
        }
    }

    Err(invalid(
        "Codex completed but no generated image was found in output cache",
    ))
}

/// Public engine entrypoint to generate a single image and register it in the session.
pub fn generate(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p["prompt"]
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid("image.generate needs prompt=<description>"))?;

    let (out_png, receipt_path) = prepare_output_dir(s)?;
    let work_dir = out_png
        .parent()
        .ok_or_else(|| invalid("invalid output image path"))?;

    let settings = crate::project_settings::stored_settings();
    let img_settings = &settings["image"];

    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .or_else(|| img_settings["api_key"].as_str().map(str::to_string))
        .or_else(|| {
            img_settings["api_key_env"]
                .as_str()
                .and_then(|env| std::env::var(env).ok())
        });

    let endpoint = std::env::var("magic_IMAGE_ENDPOINT")
        .ok()
        .or_else(|| img_settings["endpoint"].as_str().map(str::to_string));

    let provider = p
        .get("provider")
        .and_then(Value::as_str)
        .or_else(|| img_settings["provider"].as_str())
        .unwrap_or_else(|| if api_key.is_some() { "openai" } else { "codex" });

    if let Some(ref key) = api_key {
        generate_via_openai_api(prompt, key, endpoint.as_deref(), &out_png)?;
    } else if provider == "codex" {
        generate_via_guarded_codex(prompt, work_dir, &out_png)?;
    } else {
        return Err(invalid(
            "No OPENAI_API_KEY set and provider is not codex. Set OPENAI_API_KEY or configure image provider in settings.json.",
        ));
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

    let _ = fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&record).unwrap_or_default(),
    );

    Ok(record)
}
