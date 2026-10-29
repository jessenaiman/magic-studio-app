//! Bounded repository observation only. Does not infer runtime project configuration.
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use time::OffsetDateTime;

pub struct FileRecord {
    pub name: String,
    pub group: &'static str,
    pub kind: &'static str,
    pub path: Option<PathBuf>,
    pub found: bool,
    pub modified: String,
    pub reason: String,
}
pub fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
impl FileRecord {
    pub fn location(&self) -> String {
        self.path
            .as_ref()
            .map(|p| clean(&p.display().to_string()))
            .unwrap_or_else(|| "Unavailable".into())
    }
    fn observe(
        name: String,
        group: &'static str,
        kind: &'static str,
        path: Option<PathBuf>,
    ) -> Self {
        let meta = path
            .as_ref()
            .and_then(|p| fs::symlink_metadata(p).ok())
            .filter(|m| m.is_file());
        let found = meta.is_some() && path.as_ref().is_some_and(|p| fs::File::open(p).is_ok());
        let modified = meta
            .and_then(|m| m.modified().ok())
            .map(OffsetDateTime::from)
            .map(|t| format!("{} {:02}:{:02} UTC", t.date(), t.hour(), t.minute()))
            .unwrap_or_else(|| "Unavailable".into());
        Self {
            name,
            group,
            kind,
            path,
            found,
            modified,
            reason: if found {
                "Repository source only; desktop association unavailable"
            } else {
                "Unavailable: authoritative runtime configuration not observed"
            }
            .into(),
        }
    }
}
pub struct Snapshot {
    pub files: Vec<FileRecord>,
    pub scope: String,
    pub projects_root: Option<PathBuf>,
    pub projects_found: bool,
    pub provider_detail: String,
}
impl Snapshot {
    pub fn read() -> Self {
        // Only recognize a checkout through its explicit current directory/ancestors;
        // never search the home directory, worktrees, processes or session stores.
        let root = std::env::current_dir().ok().and_then(|cwd| {
            cwd.ancestors()
                .find(|p| {
                    p.join("crates/engine/src/storyboard.rs").is_file()
                        && p.join("crew/departments").is_dir()
                })
                .map(Path::to_path_buf)
        });
        let mut files = vec![
            FileRecord::observe(
                "Storyboard template".into(),
                "Templates",
                "DeckCraft",
                root.as_ref()
                    .map(|r| r.join("pipeline/templates/studio-template.deckcraft")),
            ),
            FileRecord::observe(
                "Project settings".into(),
                "Project configuration",
                "Config",
                None,
            ),
        ];
        // These five paths are the current-main crew.list contract, not inferred roles.
        for id in ["creative", "animation", "production", "backstage", "review"] {
            let path = root
                .as_ref()
                .map(|r| r.join("crew/departments").join(id).join("AGENTS.md"));
            let title = path
                .as_ref()
                .and_then(|p| read_role(p).ok())
                .and_then(|s| {
                    let header = s.strip_prefix("---\n")?.split_once("\n---\n")?.0;
                    serde_json::from_str::<serde_json::Value>(header)
                        .ok()?
                        .get("title")?
                        .as_str()
                        .map(clean)
                })
                .unwrap_or_else(|| id.into());
            files.push(FileRecord::observe(
                format!("{title} AGENTS.md"),
                "OpenMontage agent roles",
                "Markdown",
                path,
            ));
        }
        Self {
            files,
            projects_root: None,
            projects_found: false,
            provider_detail: "Magic CLI observation pending".into(),
            scope: if root.is_some() {
                "REPOSITORY SOURCE • runtime config / cast unavailable"
            } else {
                "SOURCE UNAVAILABLE • run inside a Magic Studio checkout"
            }
            .into(),
        }
    }
}

fn read_role(path: &Path) -> std::io::Result<String> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.file_type().is_file() || meta.len() > 512 * 1024 {
        return Err(std::io::Error::other(
            "Role source is not a bounded regular file",
        ));
    }
    let mut text = String::new();
    fs::File::open(path)?
        .take(512 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 512 * 1024 {
        return Err(std::io::Error::other("Role source too large"));
    }
    Ok(text)
}

impl Snapshot {
    pub fn apply(&mut self, results: crate::provider::Results) {
        let mut details = Vec::new();
        let template = results.template.and_then(|v| response_path(&v, "path"));
        match template {
            Ok(path) => {
                self.files[0] = FileRecord::observe(
                    "Storyboard template".into(),
                    "Templates",
                    "DeckCraft",
                    Some(path),
                );
                self.files[0].reason =
                    "Observed storyboard.template; local metadata; desktop association unavailable"
                        .into();
                details.push("storyboard.template: observed".into());
            }
            Err(e) => {
                self.files[0].reason = format!("Repository fallback; storyboard.template: {e}");
                details.push(format!("storyboard.template: {e}"));
            }
        }
        match results.root.and_then(|v| {
            Ok((
                response_path(&v, "root")?,
                response_path(&v, "config")?,
                v["source"]
                    .as_str()
                    .map(clean)
                    .unwrap_or_else(|| "Unknown".into()),
            ))
        }) {
            Ok((root, config, source)) => {
                self.projects_found = root.is_dir();
                self.projects_root = Some(root);
                self.files[1] = FileRecord::observe(
                    "Project settings".into(),
                    "Project configuration",
                    "Config",
                    Some(config),
                );
                self.files[1].reason = format!(
                    "Observed project.root ({source}); local metadata; desktop association unavailable"
                );
                details.push(format!("project.root: observed ({source})"));
            }
            Err(e) => {
                self.files[1].reason = format!("project.root: {e}");
                details.push(format!("project.root: {e}"));
            }
        }
        match results.crew.and_then(role_records) {
            Ok(records) => {
                self.files.truncate(2);
                self.files.extend(records);
                details.push("crew.list: observed department files (not live cast)".into());
            }
            Err(e) => {
                for file in self.files.iter_mut().skip(2) {
                    file.reason = format!("Repository fallback; crew.list: {e}");
                }
                details.push(format!("crew.list: {e}"));
            }
        }
        self.provider_detail = details.join("\n");
        self.scope =
            "PER-RECORD CLI OBSERVATION / REPOSITORY FALLBACK • runtime cast unavailable".into();
    }
}
fn response_path(value: &serde_json::Value, key: &str) -> Result<PathBuf, String> {
    let text = value[key]
        .as_str()
        .ok_or_else(|| format!("Provider schema error: missing {key}"))?;
    let path = PathBuf::from(text);
    if !path.is_absolute() || text.chars().any(char::is_control) {
        return Err(format!(
            "Provider schema error: {key} must be a clean absolute path"
        ));
    }
    Ok(path)
}
fn role_records(value: serde_json::Value) -> Result<Vec<FileRecord>, String> {
    let departments = value["departments"]
        .as_array()
        .filter(|a| a.len() == 5)
        .ok_or("Provider schema error: expected five departments")?;
    let mut seen = std::collections::HashSet::new();
    let mut records = Vec::new();
    for department in departments {
        let id = department["document"]["department"]["id"]
            .as_str()
            .ok_or("Provider schema error: department id")?;
        if !["creative", "animation", "production", "backstage", "review"].contains(&id)
            || !seen.insert(id)
        {
            return Err("Provider schema error: department identity".into());
        }
        let title = department["document"]["department"]["title"]
            .as_str()
            .ok_or("Provider schema error: department title")?;
        let path = response_path(department, "path")?;
        let exists = department["exists"]
            .as_bool()
            .ok_or("Provider schema error: exists")?;
        let mut record = FileRecord::observe(
            format!("{} AGENTS.md", clean(title)),
            "OpenMontage agent roles",
            "Markdown",
            Some(path),
        );
        record.found &= exists;
        record.reason = if exists {
            "Observed crew.list path; local metadata; not live cast"
        } else {
            "crew.list reports embedded fallback only, not an existing file"
        }
        .into();
        records.push(record);
    }
    Ok(records)
}
