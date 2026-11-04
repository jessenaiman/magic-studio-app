//! Embed and invoke the one native-tool runner; do not interpret its findings.
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn root(target: &Path) -> PathBuf {
    let dir = if target.is_file() {
        target.parent().unwrap_or(target)
    } else {
        target
    };
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| dir.to_path_buf())
}

pub fn run(
    target: &Path,
    tools: &[String],
    dry_run: bool,
    base: &str,
    quick: bool,
) -> std::io::Result<Value> {
    let target = target.canonicalize()?;
    let params = json!({"root":root(&target),"target":target,"tools":tools,"dry_run":dry_run,"base":base,"quick":quick});
    let mut child = Command::new("node")
        .args(["--input-type=module", "-"])
        .arg(params.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("Runner stdin unavailable"))?
        .write_all(include_bytes!("../../../scripts/analyze.mjs"))?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(std::io::Error::other)
}

pub fn exit_code(report: &Value) -> i32 {
    report["exit_code"].as_i64().unwrap_or(2) as i32
}
