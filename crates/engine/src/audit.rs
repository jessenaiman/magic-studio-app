//! The audit is Codacy's real JSON report, shared by the application registry.
use crate::{EngineError, Result, Session};
use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

struct ReportDir(PathBuf);
impl Drop for ReportDir {
    fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); }
}

pub fn run(s: &mut Session, params: &Value) -> Result<Value> {
    let allowed=["path","_origin","_client","_request_id","_control"];
    if params.as_object().is_none_or(|v|v.keys().any(|k|!allowed.contains(&k.as_str()))) {
        return Err(EngineError::InvalidParams("audit accepts only an optional source repository path".into()));
    }
    let root=if let Some(path)=params.get("path") {
        let path=path.as_str().filter(|v|!v.is_empty()).ok_or_else(||EngineError::InvalidParams("audit path must be a repository directory".into()))?;
        fs::canonicalize(path)?
    } else { crate::workspace::workspace_root(s) };
    if !root.is_dir() { return Err(EngineError::InvalidParams("audit path must be a repository directory".into())); }
    if !root.join(".codacy/codacy.config.json").is_file() {
        return Err(EngineError::InvalidParams(format!("Codacy configuration is missing in {}. Configure this repository with the documented Codacy setup before auditing.",root.display())));
    }
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    let dir=ReportDir(std::env::temp_dir().join(format!("magic-codacy-{}-{stamp}",std::process::id())));
    let mut builder=fs::DirBuilder::new();
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(&dir.0)?;
    let report=dir.0.join("report.json");
    // The CLI owns scanner execution and the report schema; never install, upload, or
    // replace missing findings with a made-up empty result during an ordinary audit.
    let output=Command::new("codacy-analysis").current_dir(&root)
        .args(["analyze","--output-format","json","--output"])
        .arg(&report).args(["--no-log","--no-update-notifier"]).arg(&root)
        .output().map_err(|e|EngineError::InvalidParams(format!("Cannot run codacy-analysis: {e}")))?;
    let file=fs::File::open(&report).map_err(|_|EngineError::InvalidParams(format!(
        "Codacy returned no report (exit {}): {}",output.status, String::from_utf8_lossy(&output.stderr).chars().take(2000).collect::<String>()
    )))?;
    use std::io::Read;
    let mut bytes=Vec::new();file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len()>64 * 1024 * 1024 {return Err(EngineError::InvalidParams("Codacy report exceeds 64 MiB".into()));}
    let report:Value=serde_json::from_slice(&bytes).map_err(|e|EngineError::InvalidParams(format!("Invalid Codacy JSON report: {e}")))?;
    if !report["issues"].is_array() || !report["toolResults"].is_array() || !report["errors"].is_array() || report["metadata"]["executionMode"]=="inspect" {
        return Err(EngineError::InvalidParams("Codacy did not return an analysis report".into()));
    }
    if !report["toolResults"].as_array().is_some_and(|tools|tools.iter().any(|tool|
        matches!(tool["status"].as_str(),Some("success" | "partial"))
            && tool["filesAnalyzed"].as_u64().unwrap_or(0)>0
    )) {
        return Err(EngineError::InvalidParams("Codacy analyzed no files; the audit is unavailable, not clean".into()));
    }
    Ok(report)
}
