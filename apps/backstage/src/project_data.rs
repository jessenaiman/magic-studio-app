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
            let Some(path) = path(&item["path"]) else {
                warnings.push(format!(
                    "Catalog entry {} has invalid/missing absolute project path",
                    index + 1
                ));
                continue;
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
                Ok(observed) => {
                    let requested = path.canonicalize().ok();
                    let identity = observed["project"].as_str().map(PathBuf::from);
                    if observed["ok"] != true
                        || observed["observation"]["source"] != "saved_project_file"
                        || requested.is_none()
                        || identity != requested
                    {
                        record.error =
                            Some("Invalid native saved-project observation identity/source".into());
                        projects.push(record);
                        continue;
                    }

                    if let Some(name) = observed["name"].as_str() {
                        record.name = clean(name);
                    }
                    let base = path.parent().unwrap_or(Path::new("/"));
                    let mut seen = HashSet::new();
                    add_file(
                        &mut record,
                        &mut seen,
                        path.clone(),
                        "Studio",
                        "Magic project",
                    );
                    for (key, kind) in [("audio", "Audio"), ("video_project", "Video project")] {
                        if let Some(raw) = observed["workshop"][key]
                            .as_str()
                            .filter(|p| !p.chars().any(char::is_control) && !p.is_empty())
                        {
                            let raw = PathBuf::from(raw);
                            let target = if raw.is_absolute() {
                                raw
                            } else {
                                base.join(raw)
                            };
                            add_file(&mut record, &mut seen, target, "Studio", kind);
                        }
                    }
                    if let Some(assets) = observed["assets"].as_array() {
                        for asset in assets.iter().take(256) {
                            if let Some(raw) = asset["path"]
                                .as_str()
                                .filter(|p| !p.chars().any(char::is_control) && !p.is_empty())
                            {
                                let raw = PathBuf::from(raw);
                                let target = if raw.is_absolute() {
                                    raw
                                } else {
                                    base.join(raw)
                                };
                                let kind = asset["kind"].as_str().unwrap_or("Saved asset");
                                add_file(&mut record, &mut seen, target, "Unclassified", kind);
                            }
                        }
                        if assets.len() > 256 {
                            record.error =
                                Some("Partial: saved asset references capped at256".into());
                        }
                    }
                    record.files.sort_by(|a, b| {
                        (!a.available)
                            .cmp(&(!b.available))
                            .then(b.modified.cmp(&a.modified))
                            .then(a.name.cmp(&b.name))
                    });
                }
            }
            projects.push(record);
        }
        Self{projects,catalog_observed:true,source:"Magic project.catalog: newest-ten navigation history; saved references, not directory inventory".into(),error:(!warnings.is_empty()).then(||warnings.join("; ")),..Self::default()}
    }
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
        let Some(root) = std::env::current_dir().ok().and_then(|cwd| {
            cwd.ancestors()
                .find(|p| p.join(".git").exists())
                .map(Path::to_path_buf)
        }) else {
            self.worktree_error = Some("No explicit current-directory Git repository".into());
            return;
        };
        self.worktree_source="Native Git worktree list + tracked-file metadata; 3000 refs / 200ms per path; excludes .git, symlinks, untracked; no cross-worktree grand total".into();
        let raw = match provider::git_worktrees(control, &root) {
            Ok(raw) => raw,
            Err(error) => {
                self.worktree_error = Some(error);
                return;
            }
        };
        let text = match String::from_utf8(raw) {
            Ok(text) => text,
            Err(_) => {
                self.worktree_error = Some("Git worktree response is not UTF-8".into());
                return;
            }
        };
        let project = text
            .split('\0')
            .find_map(|f| f.strip_prefix("worktree "))
            .map(PathBuf::from)
            .as_ref()
            .unwrap_or(&root)
            .file_name()
            .map(|n| clean(&n.to_string_lossy()))
            .unwrap_or_else(|| "Repository".into());
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
                if self.worktrees.len() >= 16 {
                    self.worktree_error = Some("Partial: worktree list capped at16 paths".into());
                    break;
                }
                if !path.is_absolute() || path.to_string_lossy().chars().any(char::is_control) {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| clean(&n.to_string_lossy()))
                    .unwrap_or_else(|| "Worktree".into());
                let path = path.canonicalize().unwrap_or(path);
                let available = path.is_dir();
                let mut record = WorktreeRecord {
                    error: None,
                    project: project.clone(),
                    name,
                    path,
                    branch: branch.clone(),
                    files: None,
                    bytes: None,
                    partial: true,
                    folders: vec![],
                    available,
                };
                if available {
                    measure_tracked(control, &mut record);
                }
                self.worktrees.push(record);
            }
        }
    }
}
fn measure_tracked(control: &provider::Control, record: &mut WorktreeRecord) {
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    use std::{
        collections::BTreeMap,
        time::{Duration, Instant},
    };
    let raw = match provider::git_tracked(control, &record.path) {
        Ok(raw) => raw,
        Err(error) => {
            record.error = Some(error);
            return;
        }
    };
    let text = match String::from_utf8(raw) {
        Ok(text) => text,
        Err(error) => {
            record.error = Some(format!("Tracked Git paths are not UTF-8: {error}"));
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
        if item.chars().any(char::is_control) {
            partial = true;
            continue;
        }
        let relative = Path::new(item);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            partial = true;
            continue;
        }
        let target = record.path.join(relative);
        let Ok(canonical) = target.canonicalize() else {
            partial = true;
            continue;
        };
        if !canonical.starts_with(&record.path) {
            partial = true;
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&target) else {
            partial = true;
            continue;
        };
        if !meta.file_type().is_file() {
            continue;
        }
        count += 1;
        #[cfg(unix)]
        let identity = (meta.dev(), meta.ino());
        #[cfg(not(unix))]
        let identity = canonical.clone();
        let amount = if seen.insert(identity) { meta.len() } else { 0 };
        bytes = bytes.saturating_add(amount);
        let first = relative
            .components()
            .next()
            .map(|c| clean(&c.as_os_str().to_string_lossy()))
            .unwrap_or_default();
        let key = if relative.components().count() > 1 {
            first
        } else {
            "(root files)".into()
        };
        let entry = folders.entry(key).or_default();
        entry.0 = entry.0.saturating_add(amount);
        entry.1 += 1;
    }
    record.files = Some(count);
    record.bytes = Some(bytes);
    record.partial = partial;
    if partial {
        record.error=Some("Partial tracked metadata: bound reached, missing/inaccessible/unsafe references; inspect source scope".into());
    }
    record.folders = folders
        .into_iter()
        .map(|(name, (bytes, files))| FolderRecord {
            path: if name == "(root files)" {
                record.path.clone()
            } else {
                record.path.join(&name)
            },
            name,
            bytes: Some(bytes),
            files,
            available: true,
        })
        .collect();
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
