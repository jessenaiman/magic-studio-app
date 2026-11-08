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
        if runtime.join("installation.json").is_file() {
            return runtime;
        }
    }
    let current_root = |root: &Path| root.join("Cargo.toml").is_file() && root.join("crates/engine/Cargo.toml").is_file();
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
    if let Ok(resolved) = craft_docpath::resolve_absolute(raw, s.project_path.as_deref())
        && resolved.exists()
    {
        return resolved;
    }
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
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from).filter(|path| path.is_absolute()).map(|home| home.join(".local/share")))
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
            path.metadata().is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
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
        std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|dir| dir.join(name)).find(|path| executable(path)))
    };
    found.and_then(|path| std::path::absolute(path).ok()).ok_or_else(|| EngineError::ExternalTool(name.into(), "executable not found on PATH".into()))
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

/// Launch a Craft desktop app directly when no specific document path is given.
pub(crate) fn open_app(app: &str) -> Result<()> {
    use std::process::{Command, Stdio};
    let mut editor = Command::new(resolve_tool(app))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| EngineError::ExternalTool(app.to_owned(), e.to_string()))?;
    std::thread::spawn(move || editor.wait());
    Ok(())
}

fn find_first_with_ext(dir: &Path, exts: &[&str], depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.') || n == "node_modules" || n == "frames"))
        .collect();
    entries.sort();
    for entry in &entries {
        if entry.is_file() && has_extension(entry, exts) {
            return Some(entry.clone());
        }
    }
    for entry in &entries {
        if entry.is_dir()
            && let Some(found) = find_first_with_ext(entry, exts, depth - 1)
        {
            return Some(found);
        }
    }
    None
}

fn find_project_craft_doc(s: &crate::Session, app: &str) -> Option<PathBuf> {
    let dir = s.project_path.as_deref()?.parent()?;
    match app {
        "designcraft" => [dir.join("storyboard.designcraft"), dir.join("storyboard/storyboard.designcraft")]
            .into_iter()
            .find(|p| p.is_file())
            .or_else(|| find_first_with_ext(dir, &["designcraft"], 3)),
        "effectcraft" => {
            let lyrics = dir.join("lettering/lyrics.ecproj");
            lyrics.is_file().then_some(lyrics).or_else(|| find_first_with_ext(dir, &["ecproj"], 3))
        }
        "filmcraft" => {
            let stem = s.project_path.as_deref().and_then(|p| p.file_stem()).and_then(|n| n.to_str()).unwrap_or("Film");
            let primary = dir.join("film").join(format!("{stem}.fcproj"));
            primary
                .is_file()
                .then_some(primary)
                .or_else(|| crate::cmd::project::workshop::video_project_path(dir))
                .or_else(|| find_first_with_ext(dir, &["fcproj"], 3))
        }
        "photocraft" => find_first_with_ext(dir, &["pcraft", "psd"], 4),
        "vectorcraft" => find_first_with_ext(dir, &["vectorcraft", "vcraft", "drawcraft"], 4),
        "texturecraft" => find_first_with_ext(dir, &["tcraft", "mcraft"], 4),
        _ => None,
    }
}

fn craft_doc_info(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    let info = craft_doc::info(&bytes, &craft_doc::Limits::default()).ok()?;
    if !info.supported {
        return Some(serde_json::json!({
            "kind": info.kind.extension(),
            "version": info.version,
            "generator": info.generator,
            "supported": false,
            "error": "document version is newer than supported craft-doc schema",
        }));
    }
    Some(serde_json::json!({
        "kind": info.kind.extension(),
        "version": info.version,
        "generator": info.generator,
        "supported": info.supported,
    }))
}

/// Open a project file in the Craft app that owns its format, launch a Craft app directly, or invoke the system opener.
pub fn cmd_workspace_open(s: &mut crate::Session, params: &Value) -> Result<Value> {
    if params.get("path").and_then(Value::as_str).is_none_or(|p| p.trim().is_empty())
        && let Some(app) = params.get("app").and_then(Value::as_str).map(str::trim).filter(|a| !a.is_empty())
    {
        let allowed = ["photocraft", "vectorcraft", "designcraft", "effectcraft", "filmcraft", "texturecraft"];
        if !allowed.contains(&app) {
            return Err(EngineError::InvalidParams(format!("unsupported Craft app '{app}'")));
        }
        if let Some(doc) = find_project_craft_doc(s, app) {
            open_editor(app, &doc)?;
            return Ok(serde_json::json!({"ok": true, "app": app, "path": doc}));
        }
        open_app(app)?;
        return Ok(serde_json::json!({"ok": true, "app": app, "path": Value::Null}));
    }
    let raw = params["path"].as_str().filter(|p| !p.trim().is_empty()).ok_or_else(|| EngineError::InvalidParams("missing 'path'".into()))?;
    let path = resolve_workspace_path(s, raw);
    let requested_app = params.get("app").and_then(Value::as_str).map(str::trim).filter(|a| !a.is_empty());
    if let Some(app) = requested_app {
        let allowed = ["photocraft", "vectorcraft", "designcraft", "effectcraft", "filmcraft", "texturecraft", "blender"];
        if !allowed.contains(&app) {
            return Err(EngineError::InvalidParams(format!("unsupported Craft app '{app}'")));
        }
        let target = if app == "photocraft" && path.with_extension("pcraft").is_file() {
            path.with_extension("pcraft")
        } else if app == "vectorcraft" && path.with_extension("vectorcraft").is_file() {
            path.with_extension("vectorcraft")
        } else {
            path
        };
        open_editor(app, &target)?;
        return Ok(serde_json::json!({"ok": true, "app": app, "path": target}));
    }
    let sniffed = std::fs::read(&path).ok().and_then(|bytes| craft_doc::sniff(&bytes)).map(|kind| match kind {
        craft_doc::Kind::PhotoCraft => "photocraft",
        craft_doc::Kind::VectorCraft => "vectorcraft",
        craft_doc::Kind::EffectCraft => "effectcraft",
        _ => "xdg-open",
    });
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let app = sniffed.unwrap_or(match ext.as_str() {
        "cadcraft" | "dxf" | "dwg" => "cadcraft",
        "designcraft" | "idml" => "designcraft",
        "ecproj" => "effectcraft",
        "fcproj" => "filmcraft",
        "mcraft" => "materialcraft",
        "pcraft" | "psd" => "photocraft",
        "solidcraft" | "step" | "stp" => "solidcraft",
        "soundcraft" => "soundcraft",
        "tcraft" => "texturecraft",
        "vectorcraft" | "drawcraft" | "vcraft" | "svg" => "vectorcraft",
        "wordcraft" | "docx" => "wordcraft",
        "blend" => "blender",
        _ => "xdg-open",
    });
    open_editor(app, &path)?;
    Ok(serde_json::json!({"ok": true, "app": app, "path": path}))
}

/// Run PhotoCraft commands (`document.inspect`, `image.imageRotation.*`, `image.adjustments.*`,
/// `layer.*`) directly in-process via `photocraft_engine::Session`. Edited images default to
/// saving a native `.pcraft` project file alongside the source image.
pub fn cmd_photo_exec(s: &mut crate::Session, p: &Value) -> Result<Value> {
    let raw = p["path"].as_str().ok_or_else(|| EngineError::InvalidParams("path is required".into()))?;
    let path = resolve_workspace_path(s, raw);
    let steps: Vec<Value> = if let Some(arr) = p["steps"].as_array() {
        arr.clone()
    } else if let Some(id) = p["id"].as_str().or_else(|| p["command"].as_str()) {
        vec![serde_json::json!({"id": id, "params": p.get("params").cloned().unwrap_or_else(|| serde_json::json!({}))})]
    } else {
        vec![serde_json::json!({"id": "document.inspect", "params": {}})]
    };
    let is_query = steps.len() == 1 && matches!(steps[0]["id"].as_str(), Some("document.inspect" | "session.inspect" | "command.list"));
    let save = p["save"].as_str().map(|out| resolve_workspace_path(s, out)).or_else(|| (!is_query).then(|| path.with_extension("pcraft")));
    let mut pairs: Vec<(&str, Value)> = Vec::with_capacity(steps.len());
    for step in &steps {
        let id = step["id"].as_str().or_else(|| step["command"].as_str()).ok_or_else(|| EngineError::InvalidParams("step missing command id".into()))?;
        let params = step.get("params").cloned().unwrap_or_else(|| serde_json::json!({}));
        pairs.push((id, params));
    }
    // Via installed photocraft-cli (system app pattern, no in-process linking)
    let cli = find_executable("photocraft-cli").map_err(|e| EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    let mut cmd = std::process::Command::new(&cli);
    cmd.arg("run").arg(&path);
    for (id, params) in &pairs {
        cmd.arg("--cmd").arg(id).arg("--params").arg(params.to_string());
    }
    if let Some(ref out) = save {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        cmd.arg("--out").arg(out);
    }
    let out = cmd.output().map_err(|e| EngineError::ExternalTool("photocraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool("photocraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    let info = save.as_deref().or(Some(path.as_path())).and_then(craft_doc_info);
    Ok(serde_json::json!({"ok": true, "path": path, "pcraft": save, "info": info, "via": "photocraft-cli"}))
}

/// Run EffectCraft via installed `effectcraft-cli` (system app pattern).
pub fn cmd_effect_exec(s: &mut crate::Session, p: &Value) -> Result<Value> {
    let raw = p["path"].as_str().ok_or_else(|| EngineError::InvalidParams("path=<.ecproj> is required".into()))?;
    let path = resolve_workspace_path(s, raw);
    let steps: Vec<Value> = if let Some(arr) = p["steps"].as_array() {
        arr.clone()
    } else if let Some(id) = p["id"].as_str().or_else(|| p["command"].as_str()) {
        vec![serde_json::json!({"id": id, "params": p.get("params").cloned().unwrap_or_else(|| serde_json::json!({}))})]
    } else {
        return Err(EngineError::InvalidParams("id=<effectcraft command> or steps=[...] is required".into()));
    };
    let cli = find_executable("effectcraft-cli").map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    let mut cmd = std::process::Command::new(&cli);
    cmd.arg("run").arg("--project").arg(&path).arg("--commands").arg("-");
    if p["save"] == true {
        cmd.arg("--save");
    }
    cmd.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let bytes = serde_json::to_vec(&steps).map_err(|e| EngineError::InvalidParams(e.to_string()))?;
        stdin.write_all(&bytes)?;
    }
    let out = child.wait_with_output().map_err(|e| EngineError::ExternalTool("effectcraft-cli".into(), e.to_string()))?;
    if !out.status.success() {
        return Err(EngineError::ExternalTool("effectcraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    let results: Vec<Value> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str(l.trim()).ok()).collect();
    let info = craft_doc_info(&path);
    Ok(serde_json::json!({"ok": true, "path": path, "info": info, "results": results, "via": "effectcraft-cli"}))
}

/// Run VectorCraft via installed `vectorcraft-cli` (system app, MCP/CLI pattern).
/// No in-process engine linking: Magic drives the installed app on PATH,
/// same as all Craft apps (--control / mcp / run commands).
pub fn cmd_vector_exec(s: &mut crate::Session, p: &Value) -> Result<Value> {
    let raw = p["path"].as_str().ok_or_else(|| EngineError::InvalidParams("path is required".into()))?;
    let path = resolve_workspace_path(s, raw);
    let steps: Vec<Value> = if let Some(arr) = p["steps"].as_array() {
        arr.clone()
    } else if let Some(id) = p["id"].as_str().or_else(|| p["command"].as_str()) {
        vec![serde_json::json!({"id": id, "params": p.get("params").cloned().unwrap_or_else(|| serde_json::json!({}))})]
    } else {
        vec![serde_json::json!({"id": "document.inspect", "params": {}})]
    };
    let is_query = steps.len() == 1 && matches!(steps[0]["id"].as_str(), Some("document.inspect" | "session.inspect" | "command.list"));
    let save = p["save"]
        .as_str()
        .or_else(|| p["out"].as_str())
        .map(|o| resolve_workspace_path(s, o))
        .or_else(|| (!is_query).then(|| path.with_extension("vectorcraft")));
    let cli = find_executable("vectorcraft-cli")
        .or_else(|_| find_executable("vectorcraft"))
        .map_err(|e| EngineError::ExternalTool("vectorcraft-cli".into(), e.to_string()))?;
    let tmp = std::env::temp_dir().join(format!("magic-vector-{}.json", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec(&steps).unwrap_or_default())?;
    let mut cmd = std::process::Command::new(&cli);
    cmd.arg("run").arg("--commands").arg(&tmp);
    if path.is_file() {
        cmd.arg("--in").arg(&path);
    }
    if let Some(ref out) = save {
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        cmd.arg("--export").arg(out);
    }
    let out = cmd.output().map_err(|e| EngineError::ExternalTool("vectorcraft-cli".into(), e.to_string()))?;
    let _ = std::fs::remove_file(&tmp);
    if !out.status.success() {
        return Err(EngineError::ExternalTool("vectorcraft-cli".into(), format!("exit {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr))));
    }
    let results: Vec<Value> = String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str(l.trim()).ok()).collect();
    let info = save.as_deref().or(Some(path.as_path())).and_then(craft_doc_info);
    Ok(serde_json::json!({"ok": true, "path": path, "vectorcraft": save, "info": info, "results": results, "via": "vectorcraft-cli"}))
}

/// Resolve an installed tool executable from `<TOOL_NAME>` env override,
/// `~/.config/magic/config.toml` (`tools.<name>`), `PATH`, or `~/.local/bin/<name>`.
pub fn resolve_tool(name: &str) -> String {
    if let Ok(env_val) = std::env::var(name.to_uppercase().replace('-', "_")) {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let stored = crate::project_settings::stored_settings();
    if let Some(cfg_val) = stored.pointer(&format!("/tools/{name}")).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        return cfg_val.to_string();
    }
    if let Some(installed) = std::env::var_os("HOME").and_then(|h| launcher_app(Path::new(&h), name)) {
        return installed;
    }
    if let Ok(found) = find_executable(name) {
        return found.to_string_lossy().into_owned();
    }
    if let Some(local_bin) = std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/bin").join(name)).filter(|p| p.is_file()) {
        return local_bin.to_string_lossy().into_owned();
    }
    name.to_string()
}

/// The installed desktop app as the ArtCraft launcher registered it: the `Exec` program of
/// `~/.local/share/applications/ai.storyteller.<name>.desktop`. The launcher rewrites that entry
/// on every update, so this always names the current build; no version lives in this code.
fn launcher_app(home: &Path, name: &str) -> Option<String> {
    let entry = home.join(".local/share/applications").join(format!("ai.storyteller.{name}.desktop"));
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
    static LIMITER: std::sync::OnceLock<(std::sync::Mutex<usize>, std::sync::Condvar)> = std::sync::OnceLock::new();
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
    let executable = if !tool.contains('/') { resolve_tool(tool) } else { tool.to_string() };
    let output =
        std::process::Command::new(&executable).current_dir(cwd).args(args).output().map_err(|e| EngineError::ExternalTool(tool.to_string(), e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() { stdout.trim().to_string() } else { stderr.trim().to_string() };
        Err(EngineError::ExternalTool(tool.to_string(), format!("exit {:?}: {detail}", output.status.code())))
    }
}

/// Audio file extensions recognized by the workshop.
pub(crate) const AUDIO_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "m4a", "aac", "ogg"];

/// True when `path` has one of `allowed` extensions (case-insensitive).
pub(crate) fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| allowed.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

/// Validate and resolve an audio path for the workshop song.
pub(crate) fn song_audio_path(s: &crate::Session, raw: &str) -> Result<PathBuf> {
    let path = resolve_workspace_path(s, raw);
    if !path.is_file() || !has_extension(&path, AUDIO_EXTENSIONS) {
        return Err(EngineError::InvalidParams(format!("not an audio file: {}", path.display())));
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
        .or_else(|| path.file_stem().and_then(|st| st.to_str()).and_then(try_sanitize_asset_id))
        .unwrap_or_else(|| "asset".to_string())
}
