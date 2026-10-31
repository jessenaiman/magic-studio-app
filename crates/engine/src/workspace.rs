//! Workspace & external Craft CLI resolution.
//!
//! Resolves the source checkout or installed runtime, the installed Craft
//! CLIs (`photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`), builds exact
//! CLI argument vectors, and runs subprocess tools. Pure path logic: no session
//! state is mutated here.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{EngineError, Result};

/// Return the canonical current Tauri/engine workspace, independent of the caller working directory.
pub fn workspace_root(s: &crate::Session) -> PathBuf {
    // Installed binaries resolve their bundled runtime before any development checkout.
    if let Ok(exe) = std::env::current_exe()
        && let Some(home) = exe.parent().and_then(|p| p.parent())
    {
        let runtime = home.join("runtime");
        if runtime.join("installation.json").is_file()
            && runtime
                .join("pipeline/templates/studio-template.deckcraft")
                .is_file()
        {
            return runtime;
        }
    }
    let current_root = |root: &Path| {
        root.join("Cargo.toml").is_file() && root.join("apps/src-tauri/Cargo.toml").is_file()
    };
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Ok(canon) = manifest_root.canonicalize()
        && current_root(&canon)
    {
        return canon;
    }
    // Installed builds without their source tree can still use an explicit workspace cwd.
    s.cwd.clone()
}

/// Source measurements must never use the installed runtime-resource bundle.
pub fn audit_source_root(s: &crate::Session) -> Option<PathBuf> {
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    resolve_audit_source(&workspace_root(s), &manifest_root, &s.cwd)
}

fn source_checkout(root: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    (root.join("Cargo.toml").is_file()
        && root.join("apps/src-tauri/Cargo.toml").is_file()
        && root.join("crates/engine/Cargo.toml").is_file())
    .then_some(root)
}

fn resolve_audit_source(runtime: &Path, built_from: &Path, cwd: &Path) -> Option<PathBuf> {
    let installed = std::fs::read(runtime.join("installation.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    if let Some(recorded) = installed.as_ref().and_then(|v| v.get("source_root")) {
        // A moved/malformed recorded checkout must not silently select another repository.
        let recorded = Path::new(recorded.as_str()?);
        return recorded
            .is_absolute()
            .then(|| source_checkout(recorded))
            .flatten();
    }
    // Legacy installations did not record their source path. Retain development use
    // without ever treating the bundled runtime as source.
    source_checkout(built_from).or_else(|| source_checkout(cwd))
}

/// Resolve a potentially relative path against the session workspace root.
pub fn resolve_workspace_path(s: &crate::Session, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    let from_cwd = s.cwd.join(p);
    if from_cwd.exists() {
        return from_cwd;
    }
    workspace_root(s).join(p)
}

/// Resolve an installed Craft CLI binary (`photocraft-cli`, `effectcraft-cli`, `filmcraft-cli`),
/// preferring an explicit `<TOOL_NAME>` env override, then `~/.local/share/artcraft-launcher/installed.json`, then `PATH`.
pub fn resolve_craft_cli(name: &str) -> String {
    std::env::var(name.to_uppercase().replace('-', "_"))
        .ok()
        .filter(|p| !p.is_empty())
        .or_else(|| resolve_installed_cli(name))
        .unwrap_or_else(|| name.to_string())
}

fn resolve_installed_cli(name: &str) -> Option<String> {
    let is_cli = name.ends_with("-cli");
    let app = name.strip_suffix("-cli").unwrap_or(name);
    let home = std::env::var("HOME").ok()?;
    let installed_path = Path::new(&home).join(".local/share/artcraft-launcher/installed.json");
    let text = std::fs::read_to_string(installed_path).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    let app_entry = &parsed["apps"][app];
    if !is_cli && let Some(appimage) = app_entry["path"].as_str() {
        let p = Path::new(appimage);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    let ver = app_entry["version"].as_str()?;
    let cli_name = format!("{app}-cli");
    let extracted = Path::new(&home)
        .join(".local/share")
        .join(app)
        .join(ver)
        .join(&cli_name);
    if extracted.is_file() {
        return Some(extracted.to_string_lossy().into_owned());
    }
    None
}

/// Build the exact CLI argument vector for `photocraft-cli run <input> --cmd <id> [--params <json>] --out <output>`.
pub fn image_batch_args(
    input: &str,
    cmd_id: &str,
    params_json: Option<&str>,
    output: &str,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        input.to_string(),
        "--cmd".to_string(),
        cmd_id.to_string(),
    ];
    if let Some(p) = params_json {
        args.push("--params".to_string());
        args.push(p.to_string());
    }
    args.push("--out".to_string());
    args.push(output.to_string());
    args
}

pub(crate) struct ToolSlotGuard;
impl Drop for ToolSlotGuard {
    fn drop(&mut self) {
        let (lock, cvar) = tool_limiter();
        if let Ok(mut active) = lock.lock() {
            *active = active.saturating_sub(1);
            cvar.notify_one();
        }
    }
}

fn tool_limiter() -> &'static (std::sync::Mutex<usize>, std::sync::Condvar) {
    static LIMITER: std::sync::OnceLock<(std::sync::Mutex<usize>, std::sync::Condvar)> =
        std::sync::OnceLock::new();
    LIMITER.get_or_init(|| (std::sync::Mutex::new(0), std::sync::Condvar::new()))
}

pub(crate) fn acquire_tool_slot() -> ToolSlotGuard {
    let (lock, cvar) = tool_limiter();
    if let Ok(mut active) = lock.lock() {
        while *active >= 4 {
            active = match cvar.wait(active) {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
        }
        *active += 1;
    }
    ToolSlotGuard
}

/// Run an external command-line tool in `cwd`, capturing stdout as a String.
pub(crate) fn run_tool_in(cwd: &Path, tool: &str, args: &[String]) -> Result<String> {
    let _slot = acquire_tool_slot();
    let executable = if tool.ends_with("-cli") && !tool.contains('/') {
        resolve_craft_cli(tool)
    } else {
        tool.to_string()
    };
    let output = std::process::Command::new(&executable)
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| EngineError::ExternalTool(tool.to_string(), e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        Err(EngineError::ExternalTool(
            tool.to_string(),
            format!("exit {:?}: {detail}", output.status.code()),
        ))
    }
}

/// Run an existing pipeline module directly and require its JSON result.
pub(crate) fn run_python_json(s: &crate::Session, module: &str, args: &[String]) -> Result<Value> {
    let mut command = vec!["-m".to_owned(), module.to_owned()];
    command.extend_from_slice(args);
    let stdout = run_tool_in(&workspace_root(s), "python3", &command)?;
    serde_json::from_str(&stdout)
        .map_err(|error| EngineError::ExternalTool(module.into(), format!("invalid JSON: {error}")))
}

/// Audio file extensions recognized by the workshop.
pub(crate) const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "m4a", "aac", "ogg"];

/// True when `path` has one of `allowed` extensions (case-insensitive).
pub(crate) fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| allowed.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Validate and resolve an audio path for the workshop song.
pub(crate) fn song_audio_path(s: &crate::Session, raw: &str) -> Result<PathBuf> {
    let path = resolve_workspace_path(s, raw);
    if !path.is_file() || !has_extension(&path, AUDIO_EXTENSIONS) {
        return Err(EngineError::InvalidParams(format!(
            "not an audio file: {}",
            path.display()
        )));
    }
    Ok(path)
}

pub(crate) fn try_sanitize_asset_id(raw: &str) -> Option<String> {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '_' || ch == '-' || ch == ' ') && !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Sanitize a user-supplied or file-stem name into a stable asset id.
pub fn sanitize_asset_id(raw: &str) -> String {
    try_sanitize_asset_id(raw).unwrap_or_else(|| "asset".to_string())
}

/// Derive a sanitized asset id from an optional user-typed name, falling back to the file stem
/// only when no name is typed.
pub fn asset_id_for_import(typed_name: Option<&str>, path: &Path) -> String {
    typed_name
        .and_then(try_sanitize_asset_id)
        .or_else(|| {
            path.file_stem()
                .and_then(|st| st.to_str())
                .and_then(try_sanitize_asset_id)
        })
        .unwrap_or_else(|| "asset".to_string())
}

#[cfg(test)]
mod audit_source_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "magic-audit-source-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn checkout(&self, name: &str) -> PathBuf {
            let root = self.0.join(name);
            for path in [
                "Cargo.toml",
                "apps/src-tauri/Cargo.toml",
                "crates/engine/Cargo.toml",
            ] {
                let path = root.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, "").unwrap();
            }
            root
        }
        fn runtime(&self, metadata: Value) -> PathBuf {
            let root = self.0.join("runtime");
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("installation.json"), metadata.to_string()).unwrap();
            root
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn installed_audit_uses_recorded_source_not_runtime_or_cwd() {
        let f = Fixture::new();
        let source = f.checkout("source");
        let other = f.checkout("other");
        let runtime = f.runtime(serde_json::json!({"source_root": source}));
        assert_eq!(
            resolve_audit_source(&runtime, &other, &other),
            source_checkout(&source)
        );
        assert!(runtime.join("installation.json").is_file());
    }

    #[test]
    fn moved_or_invalid_recorded_source_does_not_fall_back() {
        let f = Fixture::new();
        let fallback = f.checkout("fallback");
        for value in [
            serde_json::json!(f.0.join("moved")),
            serde_json::json!("relative"),
            Value::Null,
        ] {
            let runtime = f.runtime(serde_json::json!({"source_root": value}));
            assert_eq!(resolve_audit_source(&runtime, &fallback, &fallback), None);
        }
    }

    #[test]
    fn legacy_install_uses_existing_build_checkout() {
        let f = Fixture::new();
        let source = f.checkout("source");
        let runtime = f.runtime(serde_json::json!({"source_revision": "legacy"}));
        assert_eq!(
            resolve_audit_source(&runtime, &source, &f.0),
            source_checkout(&source)
        );
    }

    #[test]
    fn source_unavailable_never_audits_runtime_bundle() {
        let f = Fixture::new();
        let runtime = f.runtime(serde_json::json!({}));
        assert_eq!(
            resolve_audit_source(&runtime, &f.0.join("gone"), &runtime),
            None
        );
    }
}
