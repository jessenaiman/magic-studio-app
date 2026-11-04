//! Saved native project references only; never attach/open an engine project or scan folders.
use crate::{data::clean, provider};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use time::OffsetDateTime;
pub struct ProjectFile {
    pub path: PathBuf,
    pub name: String,
    pub group: String,
    pub kind: String,
    pub bytes: Option<u64>,
    pub modified: String,
    pub available: bool,
}
pub struct ProjectRecord {
    pub identity: PathBuf,
    pub name: String,
    pub path: PathBuf,
    pub files: Vec<ProjectFile>,
    pub error: Option<String>,
    pub complete: bool,
    pub exists: bool,
}
pub struct ProjectSnapshot {
    pub activity: Value,
    pub projects: Vec<ProjectRecord>,
    pub catalog_observed: bool,
    pub source: String,
    pub error: Option<String>,
    pub worktrees: Vec<WorktreeRecord>,
    pub worktree_source: String,
    pub worktree_error: Option<String>,
}
impl Default for ProjectSnapshot {
    fn default() -> Self {
        Self {
            activity: serde_json::json!({"available":false,"error":"Herdr activity not observed"}),
            projects: vec![],
            catalog_observed: false,
            source: "Native saved-project observation pending".into(),
            error: None,
            worktrees: vec![],
            worktree_source: "Git observation pending".into(),
            worktree_error: None,
        }
    }
}
impl ProjectSnapshot {
    pub fn read(control: &provider::Control) -> Self {
        let mut snapshot = Self::read_projects(control);
        snapshot.load_worktrees(control);
        snapshot.activity = provider::activity(control)
            .map(|v| v["activity"].clone())
            .unwrap_or_else(|error| serde_json::json!({"available":false,"error":error}));
        snapshot
    }
    fn read_projects(control: &provider::Control) -> Self {
        let catalog = match provider::catalog(control) {
            Ok(v) => v,
            Err(error) => {
                return Self {
                    projects: vec![],
                    source: "Magic project.catalog unavailable".into(),
                    error: Some(error),
                    ..Self::default()
                };
            }
        };
        let Some(items) = catalog["projects"].as_array().filter(|a| a.len() <= 10) else {
            return Self {
                projects: vec![],
                source: "Magic project.catalog".into(),
                error: Some("Invalid native catalog response (expected newest-ten history)".into()),
                ..Self::default()
            };
        };
        let mut projects = Vec::new();
        let mut warnings = Vec::new();
        for (index, item) in items.iter().enumerate() {
            match read_project_item(control, index, item) {
                Ok(record) => projects.push(record),
                Err(warning) => warnings.push(warning),
            }
        }
        Self {
            projects,
            catalog_observed: true,
            source: "Magic project.catalog: newest-ten navigation history; saved references, not directory inventory".into(),
            error: (!warnings.is_empty()).then(|| warnings.join("; ")),
            ..Self::default()
        }
    }
}
fn read_project_item(
    control: &provider::Control,
    index: usize,
    item: &Value,
) -> Result<ProjectRecord, String> {
    let Some(path) = path(&item["path"]) else {
        return Err(format!(
            "Catalog entry {} has invalid/missing absolute project path",
            index + 1
        ));
    };
    let name = item["name"]
        .as_str()
        .map(clean)
        .unwrap_or_else(|| "Unnamed saved project".into());
    let mut record = ProjectRecord {
        identity: path.canonicalize().unwrap_or_else(|_| path.clone()),
        name,
        path: path.parent().unwrap_or(Path::new("/")).to_path_buf(),
        files: vec![],
        error: None,
        complete: false,
        exists: path.parent().is_some_and(|p| p.is_dir()),
    };
    match provider::observe(control, &path) {
        Err(error) => record.error = Some(error),
        Ok(observed) => apply_observed_project(&mut record, &path, &observed),
    }
    Ok(record)
}
fn apply_observed_project(record: &mut ProjectRecord, path: &Path, observed: &Value) {
    let requested = path.canonicalize().ok();
    let identity = observed["project"].as_str().map(PathBuf::from);
    if observed["ok"] != true
        || observed["observation"]["source"] != "saved_project_file"
        || requested.is_none()
        || identity != requested
    {
        record.error = Some("Invalid native saved-project observation identity/source".into());
        return;
    }
    if let Some(name) = observed["name"].as_str() {
        record.name = clean(name);
    }
    let base = path.parent().unwrap_or(Path::new("/"));
    let mut seen = HashSet::new();
    add_file(
        record,
        &mut seen,
        path.to_path_buf(),
        "Studio",
        "Magic project",
    );
    add_workshop_files(record, &mut seen, base, &observed["workshop"]);
    let assets_len = add_asset_files(record, &mut seen, base, &observed["assets"]);
    record.complete = assets_len <= 256;
    if assets_len > 256 {
        record.error = Some("Partial: saved asset references capped at256".into());
    }
    record.files.sort_by(|a, b| {
        (!a.available)
            .cmp(&(!b.available))
            .then(b.modified.cmp(&a.modified))
            .then(a.name.cmp(&b.name))
    });
}
fn resolve_clean_path(base: &Path, raw: Option<&str>) -> Option<PathBuf> {
    let raw = raw.filter(|p| !p.chars().any(char::is_control) && !p.is_empty())?;
    let path = PathBuf::from(raw);
    Some(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}
fn add_workshop_files(
    record: &mut ProjectRecord,
    seen: &mut HashSet<PathBuf>,
    base: &Path,
    workshop: &Value,
) {
    for (key, kind) in [("audio", "Audio"), ("video_project", "Video project")] {
        if let Some(target) = resolve_clean_path(base, workshop[key].as_str()) {
            add_file(record, seen, target, "Studio", kind);
        }
    }
}
fn add_asset_files(
    record: &mut ProjectRecord,
    seen: &mut HashSet<PathBuf>,
    base: &Path,
    assets: &Value,
) -> usize {
    let Some(assets) = assets.as_array() else {
        return 0;
    };
    for asset in assets.iter().take(256) {
        if let Some(target) = resolve_clean_path(base, asset["path"].as_str()) {
            let kind = asset["kind"].as_str().unwrap_or("Saved asset");
            add_file(record, seen, target, "Unclassified", kind);
        }
    }
    assets.len()
}
fn path(value: &Value) -> Option<PathBuf> {
    let s = value.as_str()?;
    let p = PathBuf::from(s);
    (p.is_absolute() && !s.chars().any(char::is_control)).then_some(p)
}
fn add_file(
    record: &mut ProjectRecord,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
    group: &str,
    kind: &str,
) {
    if !seen.insert(path.clone()) {
        return;
    }
    let meta = fs::symlink_metadata(&path)
        .ok()
        .filter(|m| m.file_type().is_file());
    let modified = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(OffsetDateTime::from)
        .map(|t| format!("{} {:02}:{:02} UTC", t.date(), t.hour(), t.minute()))
        .unwrap_or_else(|| "Unavailable".into());
    let name = path
        .file_name()
        .map(|n| clean(&n.to_string_lossy()))
        .unwrap_or_else(|| "Unknown file".into());
    let kind = file_kind(&path, kind);
    record.files.push(ProjectFile {
        name,
        path,
        group: group.into(),
        kind,
        available: meta.is_some(),
        bytes: meta.map(|m| m.len()),
        modified,
    });
}

pub struct FolderRecord {
    pub name: String,
    pub path: PathBuf,
    pub bytes: Option<u64>,
    pub files: usize,
    pub available: bool,
}
pub struct WorktreeRecord {
    pub error: Option<String>,
    pub project: String,
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub files: Option<usize>,
    pub bytes: Option<u64>,
    pub partial: bool,
    pub folders: Vec<FolderRecord>,
    pub available: bool,
}
impl ProjectSnapshot {
    fn load_worktrees(&mut self, control: &provider::Control) {
        let Some(root) = current_git_root() else {
            self.worktree_error = Some("No explicit current-directory Git repository".into());
            return;
        };
        self.worktree_source = "Native Git worktree list + tracked-file metadata; 3000 refs / 200ms per path; excludes .git, symlinks, untracked; no cross-worktree grand total".into();
        let text = match read_worktree_text(control, &root) {
            Ok(text) => text,
            Err(error) => {
                self.worktree_error = Some(error);
                return;
            }
        };
        let project = worktree_project_name(&text, &root);
        self.collect_worktree_records(control, &text, &project);
        self.worktrees.sort_by(|a, b| {
            (b.name == "bunzen")
                .cmp(&(a.name == "bunzen"))
                .then_with(|| b.files.unwrap_or(0).cmp(&a.files.unwrap_or(0)))
                .then_with(|| b.bytes.unwrap_or(0).cmp(&a.bytes.unwrap_or(0)))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    fn collect_worktree_records(&mut self, control: &provider::Control, text: &str, project: &str) {
        let mut path = None;
        let mut branch = String::new();
        for field in text.split('\0') {
            if let Some(raw) = field.strip_prefix("worktree ") {
                path = Some(PathBuf::from(raw));
                branch = "detached".into();
            } else if let Some(raw) = field.strip_prefix("branch ") {
                branch = clean(raw.strip_prefix("refs/heads/").unwrap_or(raw));
            } else if field.is_empty()
                && let Some(path) = path.take()
            {
                if self.worktrees.len() >= 32 {
                    self.worktree_error = Some("Partial: worktree list capped at32 paths".into());
                    break;
                }
                if let Some(record) = build_worktree_record(control, project, path, &branch) {
                    self.worktrees.push(record);
                }
            }
        }
    }
}
fn current_git_root() -> Option<PathBuf> {
    std::env::current_dir().ok().and_then(|cwd| {
        cwd.ancestors()
            .find(|p| p.join(".git").exists())
            .map(Path::to_path_buf)
    })
}
fn read_worktree_text(control: &provider::Control, root: &Path) -> Result<String, String> {
    let raw = provider::git_worktrees(control, root)?;
    String::from_utf8(raw).map_err(|_| "Git worktree response is not UTF-8".into())
}
fn worktree_project_name(text: &str, root: &Path) -> String {
    text.split('\0')
        .find_map(|f| f.strip_prefix("worktree "))
        .map(PathBuf::from)
        .as_deref()
        .unwrap_or(root)
        .file_name()
        .map(|n| clean(&n.to_string_lossy()))
        .unwrap_or_else(|| "Repository".into())
}
fn build_worktree_record(
    control: &provider::Control,
    project: &str,
    path: PathBuf,
    branch: &str,
) -> Option<WorktreeRecord> {
    if !path.is_absolute() || path.to_string_lossy().chars().any(char::is_control) {
        return None;
    }
    let name = path
        .file_name()
        .map(|n| clean(&n.to_string_lossy()))
        .unwrap_or_else(|| "Worktree".into());
    let path = path.canonicalize().unwrap_or(path);
    let available = path.is_dir();
    let mut record = WorktreeRecord {
        error: None,
        project: project.into(),
        name,
        path,
        branch: branch.into(),
        files: None,
        bytes: None,
        partial: true,
        folders: vec![],
        available,
    };
    if available {
        measure_tracked(control, &mut record);
    }
    Some(record)
}
fn measure_tracked(control: &provider::Control, record: &mut WorktreeRecord) {
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };
    let text = match read_tracked_text(control, &record.path) {
        Ok(text) => text,
        Err(error) => {
            record.error = Some(error);
            return;
        }
    };
    let start = Instant::now();
    let mut count = 0;
    let mut bytes = 0u64;
    let mut seen = HashSet::new();
    let mut folders = BTreeMap::<String, (u64, usize)>::new();
    let mut partial = false;
    for item in text.split('\0').filter(|s| !s.is_empty()) {
        if count >= 3000 || start.elapsed() > Duration::from_millis(200) {
            partial = true;
            break;
        }
        match inspect_tracked_item(&record.path, item, &mut seen) {
            Err(()) => partial = true,
            Ok(None) => {}
            Ok(Some((key, amount))) => {
                count += 1;
                bytes = bytes.saturating_add(amount);
                let entry = folders.entry(key).or_default();
                entry.0 = entry.0.saturating_add(amount);
                entry.1 += 1;
            }
        }
    }
    record.files = Some(count);
    record.bytes = Some(bytes);
    record.partial = partial;
    if partial {
        record.error = Some(
            "Partial tracked metadata: bound reached, missing/inaccessible/unsafe references; inspect source scope".into(),
        );
    }
    record.folders = build_folder_records(&record.path, folders);
}
fn read_tracked_text(control: &provider::Control, root: &Path) -> Result<String, String> {
    let raw = provider::git_tracked(control, root)?;
    String::from_utf8(raw).map_err(|error| format!("Tracked Git paths are not UTF-8: {error}"))
}
#[cfg(unix)]
type FileIdentity = (u64, u64);
#[cfg(not(unix))]
type FileIdentity = PathBuf;
fn inspect_tracked_item(
    root: &Path,
    item: &str,
    seen: &mut HashSet<FileIdentity>,
) -> Result<Option<(String, u64)>, ()> {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    if item.chars().any(char::is_control) {
        return Err(());
    }
    let relative = Path::new(item);
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(());
    }
    let target = root.join(relative);
    let canonical = target.canonicalize().map_err(|_| ())?;
    if !canonical.starts_with(root) {
        return Err(());
    }
    let meta = fs::symlink_metadata(&target).map_err(|_| ())?;
    if !meta.file_type().is_file() {
        return Ok(None);
    }
    #[cfg(unix)]
    let identity = (meta.dev(), meta.ino());
    #[cfg(not(unix))]
    let identity = canonical;
    let amount = if seen.insert(identity) { meta.len() } else { 0 };
    Ok(Some((tracked_folder_key(relative), amount)))
}
fn tracked_folder_key(relative: &Path) -> String {
    let first = relative
        .components()
        .next()
        .map(|c| clean(&c.as_os_str().to_string_lossy()))
        .unwrap_or_default();
    if relative.components().count() > 1 {
        first
    } else {
        "(root files)".into()
    }
}
fn build_folder_records(
    root: &Path,
    folders: std::collections::BTreeMap<String, (u64, usize)>,
) -> Vec<FolderRecord> {
    folders
        .into_iter()
        .map(|(name, (bytes, files))| FolderRecord {
            path: if name == "(root files)" {
                root.to_path_buf()
            } else {
                root.join(&name)
            },
            name,
            bytes: Some(bytes),
            files,
            available: true,
        })
        .collect()
}

fn file_kind(path: &Path, fallback: &str) -> String {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("magic") => "Magic project",
        Some("deckcraft") => "DeckCraft",
        Some("blend") => "Blender",
        Some("png") => "PNG image",
        Some("jpg" | "jpeg") => "JPEG image",
        Some("wav") => "WAV audio",
        Some("mp3") => "MP3 audio",
        Some("lrc") => "LRC",
        Some("json") => "JSON",
        Some("txt") => "Text",
        Some("md") => "Markdown",
        Some("zip") => "ZIP archive",
        _ => fallback,
    }
    .into()
}
