//! Workspace and external CLI tool resolution.
//!
//! Resolves the source checkout or installed runtime, installed CLI executables
//! on `PATH` / `~/.local/bin` (with system settings and environment overrides),
//! builds exact CLI argument vectors, and runs subprocess tools. Pure path logic:
//! no session state is mutated here.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::{EngineError, Result};

/// Return the canonical current Magic Studio workspace, independent of the caller working directory.
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
        root.join("Cargo.toml").is_file() && root.join("apps/magic/Cargo.toml").is_file()
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

/// The user's portable data directory; relative XDG values are not valid roots.
pub(crate) fn user_data_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/share"))
        })
}

/// Resolve a program on the caller's PATH before handing it to a background service.
pub(crate) fn find_executable(name: &str) -> Result<PathBuf> {
    let executable = |path: &Path| {
        if !path.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            path.metadata()
                .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            true
        }
    };
    let path = Path::new(name);
    let found = if path.components().count() > 1 {
        executable(path).then(|| path.to_path_buf())
    } else {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(name))
                .find(|path| executable(path))
        })
    };
    found
        .and_then(|path| std::path::absolute(path).ok())
        .ok_or_else(|| {
            EngineError::ExternalTool(name.into(), "executable not found on PATH".into())
        })
}

/// Hand off a document for editing. The editor owns its window and unsaved changes.
pub(crate) fn open_editor(app: &str, document: &Path) -> Result<()> {
    use std::process::{Command, Stdio};
    let mut editor = Command::new(resolve_tool(app))
        .arg(document)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(app.to_owned(), e.to_string()))?;
    std::thread::spawn(move || editor.wait());
    Ok(())
}

/// Open a project file in the Craft app that owns its format, or the system opener.
pub fn cmd_workspace_open(s: &mut crate::Session, params: &Value) -> Result<Value> {
    let raw = params["path"]
        .as_str()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let path = resolve_workspace_path(s, raw);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let app = match ext.as_str() {
        "deckcraft" | "potx" | "pptx" => "deckcraft",
        "fcproj" => "filmcraft",
        "ecproj" => "effectcraft",
        "pcraft" | "psd" => "photocraft",
        "vcraft" | "svg" => "vectorcraft",
        "tcraft" => "texturecraft",
        "mcraft" => "materialcraft",
        "blend" => "blender",
        _ => "xdg-open",
    };
    open_editor(app, &path)?;
    Ok(serde_json::json!({"ok": true, "app": app, "path": path}))
}

/// Resolve an installed tool executable from `<TOOL_NAME>` env override,
/// `~/.config/magic/settings.json` (`tools.<name>`), `PATH`, or `~/.local/bin/<name>`.
pub fn resolve_tool(name: &str) -> String {
    if let Ok(env_val) = std::env::var(name.to_uppercase().replace('-', "_")) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let stored = crate::project_settings::stored_settings();
    if let Some(cfg_val) = stored
        .pointer(&format!("/tools/{name}"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return cfg_val.to_string();
    }
    if let Some(installed) =
        std::env::var_os("HOME").and_then(|h| launcher_app(Path::new(&h), name))
    {
        return installed;
    }
    if let Ok(found) = find_executable(name) {
        return found.to_string_lossy().into_owned();
    }
    if let Some(local_bin) = std::env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/bin").join(name))
        .filter(|p| p.is_file())
    {
        return local_bin.to_string_lossy().into_owned();
    }
    name.to_string()
}

/// The installed desktop app as the ArtCraft launcher registered it: the `Exec` program of
/// `~/.local/share/applications/ai.storyteller.<name>.desktop`. The launcher rewrites that entry
/// on every update, so this always names the current build; no version lives in this code.
fn launcher_app(home: &Path, name: &str) -> Option<String> {
    let entry = home
        .join(".local/share/applications")
        .join(format!("ai.storyteller.{name}.desktop"));
    let text = std::fs::read_to_string(entry).ok()?;
    let exec = text.lines().find_map(|l| l.strip_prefix("Exec="))?.trim();
    let program = match exec.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => exec.split_whitespace().next()?,
    };
    Path::new(program).is_file().then(|| program.to_string())
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
    let executable = if !tool.contains('/') {
        resolve_tool(tool)
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
mod tests {
    use super::launcher_app;

    /// The launcher's desktop entry names the installed build; a stale link elsewhere is ignored.
    #[test]
    fn desktop_apps_resolve_through_the_launcher_entry() {
        let home = std::env::temp_dir().join(format!("magic-launcher-test-{}", std::process::id()));
        let apps = home.join(".local/share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        let build = home.join("photocraft-9.9.9.AppImage");
        std::fs::write(&build, b"").unwrap();
        std::fs::write(
            apps.join("ai.storyteller.photocraft.desktop"),
            format!("[Desktop Entry]\nExec=\"{}\" %F\n", build.display()),
        )
        .unwrap();
        let found = launcher_app(&home, "photocraft");
        let missing = launcher_app(&home, "vectorcraft");
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(found.as_deref(), Some(build.to_str().unwrap()));
        assert_eq!(missing, None);
    }
}
