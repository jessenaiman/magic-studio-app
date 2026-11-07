//! Validation receipts bind FilmCraft exports to their settings and native sources.
use super::{EngineError, Film, Path, PathBuf, Result, Value, bad, export_params, json, open_filmcraft};

fn receipt_path(f: &Film) -> PathBuf {
    f.dir.join("exports/.magic-export-receipt.json")
}

fn read_receipt(f: &Film) -> Value {
    std::fs::read(receipt_path(f)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

pub(super) fn destination(f: &Film, p: &Value) -> (PathBuf, Value) {
    let receipt = read_receipt(f);
    let mut options = receipt["options"].as_object().cloned().unwrap_or_default();
    // The shared native exporter allowlist drops request metadata and UI-only fields.
    let allowed = export_params(Path::new(""), false, p);
    for (key, value) in allowed.as_object().into_iter().flatten() {
        if !matches!(key.as_str(), "path" | "wait") && (p.get(key).is_some() || !options.contains_key(key)) {
            options.insert(key.clone(), value.clone());
        }
    }
    let ext = match options.get("format").and_then(Value::as_str) {
        Some("prores" | "dnxhr") => "mov",
        Some("gif") => "gif",
        Some("wav") => "wav",
        Some("flac") => "flac",
        Some("aac") => "aac",
        Some("webm") => "webm",
        _ => "mp4",
    };
    let default = if ext == "mp4" { f.export() } else { f.dir.join("exports").join(format!("{}.{ext}", f.name)) };
    let out = p["path"].as_str().map(PathBuf::from).or_else(|| receipt["path"].as_str().map(PathBuf::from)).unwrap_or(default);
    let out = if out.is_absolute() { out } else { f.dir.join(out) };
    (out, Value::Object(options))
}

fn signature(path: &Path, hash: bool) -> Result<Value> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({"path":path,"exists":false})),
        Err(e) => return Err(e.into()),
    };
    let modified = metadata.modified()?.duration_since(std::time::UNIX_EPOCH).map_err(|e| bad(e.to_string()))?.as_nanos().to_string();
    let mut result = json!({"path":path,"exists":true,"bytes":metadata.len(),"modified_ns":modified});
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        result["inode"] = json!(metadata.ino());
        result["changed_ns"] = json!([metadata.ctime(), metadata.ctime_nsec()]);
    }
    if hash {
        use sha2::{Digest, Sha256};
        result["sha256"] = json!(format!("{:x}", Sha256::digest(std::fs::read(path)?)));
    }
    Ok(result)
}

fn native(session: &mut filmcraft_engine::Session, id: &str, params: Value) -> Result<Value> {
    session.execute(id, params).map_err(|e| EngineError::ExternalTool("filmcraft-engine".into(), format!("{id}: {e}")))
}

fn check_live_timeline(project: &Path, report: &Value) -> Result<()> {
    let matches = report["path"]
        .as_str()
        .is_some_and(|path| Path::new(path) == project || matches!((Path::new(path).canonicalize(),project.canonicalize()),(Ok(a),Ok(b)) if a == b));
    if !matches {
        return Err(bad("FilmCraft changed its active timeline during the export check; retry after selecting the saved timeline"));
    }
    match report["dirty"].as_bool() {
        Some(false) => Ok(()),
        Some(true) => Err(bad("Save the open FilmCraft timeline before exporting or validating its final output")),
        None => Err(bad("Cannot verify whether the open FilmCraft timeline is saved; update the native integration before exporting")),
    }
}

/// A matching live editor owns its unsaved edits. Never silently validate a
/// headless export of an older saved file while that editor is dirty.
pub(super) fn require_clean_live(project: &Path) -> Result<()> {
    if let Some(report) = crate::CraftHub::try_live(crate::CraftTarget::Film, project, "project.inspect", &json!({}), false) {
        check_live_timeline(project, &report?)?;
    }
    Ok(())
}

pub(super) fn context(f: &Film, out: &Path, options: &Value) -> Result<Value> {
    let mut session = open_filmcraft(f.require()?)?;
    let resolved = native(&mut session, "export.resolve", export_params(out, false, options))?;
    let sequence = native(&mut session, "sequence.inspect", json!({}))?;
    let media = native(&mut session, "media.status", json!({}))?;
    let mut ids = std::collections::BTreeSet::new();
    for kind in ["video", "audio"] {
        for track in sequence[kind].as_array().into_iter().flatten() {
            for clip in track["items"].as_array().into_iter().flatten() {
                if let Some(id) = clip["item"].as_u64() {
                    ids.insert(id);
                }
            }
        }
    }
    let mut sources = vec![signature(&f.project, true)?];
    for file in ["design/words.json", "lettering/lyrics.ecproj", "lyrics-design.json"] {
        sources.push(signature(&f.dir.join(file), true)?);
    }
    for item in media.as_array().into_iter().flatten() {
        let referenced = item["item"].as_u64().is_some_and(|id| ids.contains(&id));
        if referenced && matches!(item["status"].as_str(), Some("missing" | "offline" | "unreadable")) {
            return Err(bad(format!("Source media is unavailable: {}", item["path"])));
        }
        if let Some(path) = item["path"].as_str() {
            let path = Path::new(path);
            let path = if path.is_absolute() { path.to_path_buf() } else { f.project.parent().unwrap_or(&f.dir).join(path) };
            let sig = signature(&path, false)?;
            if referenced && sig["exists"] != true {
                return Err(bad(format!("Source media is missing: {}", path.display())));
            }
            // A fresh final movie cannot validate stale lettering against newer authored design.
            if sig["exists"] == true && path == f.dir.join("lettering/lyrics.mov") {
                let movie_time = std::fs::metadata(&path)?.modified()?;
                for dependency in ["design/words.json", "lettering/lyrics.ecproj"] {
                    if std::fs::metadata(f.dir.join(dependency)).and_then(|m| m.modified()).is_ok_and(|time| time > movie_time) {
                        return Err(bad("Lettering source changed after its render; regenerate lettering before final export"));
                    }
                }
            }
            sources.push(sig);
        }
    }
    Ok(json!({"path":out,"options":options,"sources":sources,"resolved":resolved}))
}

fn probe(out: &Path, resolved: &Value) -> Result<Value> {
    let session = filmcraft_engine::Session::default();
    let source = session.media.probe_file(&out.to_string_lossy(), &*session.services).map_err(|e| bad(format!("Export probe failed: {e}")))?;
    let info = source.info();
    let seconds = info.duration.seconds();
    let expected = resolved["range"]["seconds"].as_f64().ok_or_else(|| bad("Native export settings omit expected duration"))?;
    if !seconds.is_finite() || seconds <= 0.0 || (seconds - expected).abs() > 0.15 {
        return Err(bad(format!("Export duration {seconds:.3}s differs from native range {expected:.3}s")));
    }
    let format = resolved["settings"]["format"].as_str().unwrap_or("h264");
    let audio_only = matches!(format, "wav" | "flac" | "aac" | "mp3" | "aiff");
    if !audio_only {
        let video = info.video.as_ref().ok_or_else(|| bad("Export has no video stream"))?;
        if Some(u64::from(video.width)) != resolved["output"]["width"].as_u64() || Some(u64::from(video.height)) != resolved["output"]["height"].as_u64() {
            return Err(bad("Export dimensions differ from resolved FilmCraft settings"));
        }
    }
    if (audio_only || resolved["settings"]["includeAudio"] == true) && format != "gif" && info.audio_streams.is_empty() {
        return Err(bad("Export has no expected audio stream"));
    }
    // Decode a few native samples; container metadata alone does not prove frames
    // are readable. This is a technical validation, not a visual-quality review.
    if !audio_only {
        for time in [0.0, seconds / 2.0, (seconds - 0.1).max(0.0)] {
            source
                .video_frame(filmcraft_media::FrameRequest { time: filmcraft_time::Tick::from_seconds_f64(time), scale: 0.125 })
                .map_err(|e| bad(format!("Export frame at {time:.3}s cannot decode: {e}")))?;
        }
    }
    if let Some(audio) = info.audio_streams.first() {
        source.audio(0, 1024, audio.sample_rate).map_err(|e| bad(format!("Export audio cannot decode: {e}")))?;
    }
    Ok(json!({"valid":true,"duration_seconds":seconds,"expected_duration_seconds":expected,"format":format,"media":info}))
}

fn receipt_matches(receipt: &Value, current: &Value) -> std::result::Result<(), String> {
    if receipt["probe"]["valid"] != true {
        return Err("No successfully validated export receipt".into());
    }
    if current["output"]["exists"] != true || current["output"]["bytes"].as_u64().unwrap_or(0) == 0 {
        return Err("Export output is missing or empty".into());
    }
    for field in ["path", "options", "sources", "resolved", "output"] {
        if receipt[field] != current[field] {
            return Err(format!("Export is stale: {field} changed"));
        }
    }
    Ok(())
}

pub(super) fn record(f: &Film, out: &Path, options: &Value, before: &Value) -> Result<Value> {
    let mut current = context(f, out, options)?;
    if &current != before {
        return Err(bad("Export sources or settings changed while FilmCraft rendered; export again"));
    }
    let report = probe(out, &current["resolved"])?;
    current["output"] = signature(out, false)?;
    current["probe"] = report.clone();
    std::fs::create_dir_all(f.dir.join("exports"))?;
    craft_atomic::atomic_write(&receipt_path(f), &serde_json::to_vec_pretty(&current).map_err(|e| bad(e.to_string()))?)?;
    Ok(
        json!({"valid":true,"path":out,"receipt":receipt_path(f),"probe":report,"checks":["native media probe","duration","streams","dimensions","unchanged settings and sources"]}),
    )
}

pub(super) fn status(f: &Film, out: &Path, options: &Value, reprobe: bool) -> Value {
    match require_clean_live(&f.project) {
        Ok(()) => saved_status(f, out, options, reprobe),
        Err(error) => json!({"valid":false,"path":out,"error":error.to_string(),"receipt":receipt_path(f)}),
    }
}

fn saved_status(f: &Film, out: &Path, options: &Value, reprobe: bool) -> Value {
    let check = || -> Result<Value> {
        let receipt = read_receipt(f);
        if receipt.is_null() {
            return Err(bad("No successfully validated export receipt"));
        }
        let mut current = context(f, out, options)?;
        current["output"] = signature(out, false)?;
        receipt_matches(&receipt, &current).map_err(bad)?;
        let report = if reprobe { probe(out, &current["resolved"])? } else { receipt["probe"].clone() };
        Ok(json!({"valid":true,"path":out,"receipt":receipt_path(f),"probe":report}))
    };
    check().unwrap_or_else(|e| json!({"valid":false,"path":out,"error":e.to_string(),"receipt":receipt_path(f)}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_timeline_policy_requires_matching_saved_document() {
        let project = Path::new("/tmp/owned-film.fcproj");
        assert!(check_live_timeline(project, &json!({"path":project,"dirty":false})).is_ok());
        for report in [json!({"path":project,"dirty":true}), json!({"path":project}), json!({"path":"/tmp/other-film.fcproj","dirty":false})] {
            assert!(check_live_timeline(project, &report).is_err(), "{report}");
        }
    }

    #[test]
    fn native_export_receipt_tracks_output_settings_and_authored_design() {
        let dir = std::env::temp_dir().join(format!("magic-export-receipt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = Film { dir: dir.clone(), name: "Fixture".into(), project: dir.join("Fixture.fcproj") };
        let mut native = filmcraft_engine::Session::default();
        native.execute("file.openDemoProject", json!({})).unwrap();
        native.execute("file.saveAs", json!({"path":f.project})).unwrap();
        let options = json!({"format":"wav","range":"custom","startSeconds":0,"endSeconds":0.25});
        let out = dir.join("fixture.wav");
        assert_eq!(saved_status(&f, &out, &options, false)["valid"], false);
        let before = context(&f, &out, &options).unwrap();
        super::super::export_with_options(&f.project, &out, false, &options).unwrap();
        record(&f, &out, &options, &before).unwrap();
        assert_eq!(saved_status(&f, &out, &options, true)["valid"], true);
        let changed = json!({"format":"wav","range":"custom","startSeconds":0,"endSeconds":0.2});
        assert_eq!(saved_status(&f, &out, &changed, false)["valid"], false);
        let (saved_out, saved_options) = destination(&f, &json!({"_origin":"test","_expected_project":"ignored"}));
        assert_eq!(saved_out, out);
        assert_eq!(saved_options, options);
        let (_, cleared) = destination(&f, &json!({"captionSidecar":null}));
        assert_eq!(cleared["captionSidecar"], "none");
        assert!(cleared.get("_origin").is_none());
        assert!(cleared.get("_expected_project").is_none());
        std::fs::create_dir_all(dir.join("design")).unwrap();
        std::fs::write(dir.join("design/words.json"), b"authored design changed").unwrap();
        assert_eq!(saved_status(&f, &out, &options, false)["valid"], false);
        std::fs::remove_file(dir.join("design/words.json")).unwrap();
        std::fs::remove_file(&out).unwrap();
        assert_eq!(saved_status(&f, &out, &options, false)["valid"], false);
        let video_options = json!({"format":"h264","range":"custom","startSeconds":0,"endSeconds":0.25,"scale":0.08333333333333333,"audio":false});
        let movie = dir.join("fixture.mp4");
        let before = context(&f, &movie, &video_options).unwrap();
        super::super::export_with_options(&f.project, &movie, false, &video_options).unwrap();
        record(&f, &movie, &video_options, &before).unwrap();
        assert_eq!(saved_status(&f, &movie, &video_options, true)["valid"], true);
        std::fs::write(&movie, b"corrupt output").unwrap();
        assert_eq!(saved_status(&f, &movie, &video_options, true)["valid"], false);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn receipt_rejects_missing_output_and_changed_settings_or_sources() {
        let base =
            json!({"path":"/tmp/final.mp4","options":{"format":"h264"},"sources":{"timeline":"a"},"output":{"exists":true,"bytes":100},"probe":{"valid":true}});
        assert!(receipt_matches(&base, &base).is_ok());
        for field in ["output", "options", "sources", "path"] {
            let mut changed = base.clone();
            changed[field] = json!("changed");
            assert!(receipt_matches(&base, &changed).is_err(), "{field}");
        }
        let mut missing = base.clone();
        missing["output"] = json!({"exists":false});
        assert!(receipt_matches(&missing, &missing).is_err());
    }
}
