//! Coalescing invalidation for the explicitly listed Magic-owned file writes.
//! Readers never create, clear or acknowledge the shared marker.
use crate::{EngineError, Result, Session};
use serde_json::{Value, json};
use std::{fs, io::Write, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const COVERAGE: &[&str] = &["user_session_project_file_write_attempt", "recent_projects_write_attempt"];

fn directory() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".local/state")))
        .ok_or_else(|| EngineError::InvalidParams("No runtime or state directory available for change notifications".into()))?;
    if !base.is_absolute() { return Err(EngineError::InvalidParams("Change notification directory must be absolute".into())); }
    Ok(base.join("magicstudio"))
}

/// Best-effort publication must not turn a completed project save into an apparent failure.
pub fn invalidate() {
    if let Err(error) = publish() { eprintln!("Magic change notification unavailable: {error}"); }
}

fn publish() -> Result<()> {
    let dir=directory()?;
    let mut builder=fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(&dir)?;
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        let metadata=fs::symlink_metadata(&dir)?;
        let parent=fs::metadata(dir.parent().ok_or_else(||EngineError::InvalidParams("Notification parent unavailable".into()))?)?;
        if !metadata.file_type().is_dir() || metadata.uid()!=rustix::process::getuid().as_raw()
            || parent.uid()!=rustix::process::getuid().as_raw()
            || (metadata.mode() & 0o077 != 0 && parent.mode() & 0o077 != 0) {
            return Err(EngineError::InvalidParams("Notification directory must be private to the current user".into()));
        }
    }
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let token=format!("{}-{now}-{}",std::process::id(),SEQUENCE.fetch_add(1,Ordering::Relaxed));
    let temp=dir.join(format!(".changes-{token}.tmp"));
    let mut options=fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut file=options.open(&temp)?;
    let result=(|| -> Result<()> {
        file.write_all(serde_json::to_string(&json!({"version":1,"token":token})).map_err(|e|EngineError::InvalidParams(e.to_string()))?.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp,dir.join("changes.json"))?;
        Ok(())
    })();
    if result.is_err() { let _=fs::remove_file(&temp); }
    result
}

/// Discover the resolved path and current token without creating or modifying any file.
pub fn status(_: &mut Session, params: &Value) -> Result<Value> {
    let allowed=["_origin","_client","_request_id","_control"];
    if params.as_object().is_none_or(|p|p.keys().any(|k|!allowed.contains(&k.as_str()))) {
        return Err(EngineError::InvalidParams("app.changes accepts no options".into()));
    }
    let path=directory()?.join("changes.json");
    let read=(|| -> std::result::Result<Value,&'static str> {
        let file=crate::read_only_file::open_regular(&path).map_err(|e|match e {
            EngineError::Io(e) if e.kind()==std::io::ErrorKind::NotFound => "marker_missing",
            EngineError::InvalidParams(_) => "marker_invalid",
            _ => "marker_unreadable",
        })?;
        let meta=file.metadata().map_err(|_|"marker_unreadable")?;
        if meta.len()>4096 { return Err("marker_invalid"); }
        #[cfg(unix)] {
            use std::os::unix::fs::MetadataExt;
            if meta.uid()!=rustix::process::getuid().as_raw() || meta.mode() & 0o077 != 0 { return Err("marker_not_private"); }
        }
        use std::io::Read;
        let mut bytes=Vec::new();
        file.take(4097).read_to_end(&mut bytes).map_err(|_|"marker_unreadable")?;
        if bytes.len()>4096 { return Err("marker_invalid"); }
        let marker:Value=serde_json::from_slice(&bytes).map_err(|_|"marker_invalid")?;
        if marker["version"]!=1 || marker["token"].as_str().is_none_or(str::is_empty) { return Err("marker_invalid"); }
        Ok(marker)
    })();
    let token=read.as_ref().ok().and_then(|v|v["token"].as_str());
    Ok(json!({"ok":true,"path":path,"available":token.is_some(),"token":token,"reason":read.as_ref().err(),
        "coverage":COVERAGE,"excluded":["non_user_session_project_writes","unsaved_ui","external_file_changes","asynchronous_completions","other_project_files"],
        "semantics":"opaque coalescing invalidation; readers never clear the marker"}))
}
