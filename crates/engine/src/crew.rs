//! One shared AGENTS.md per department; the callboard and CLI edit the same guarded file.
use crate::{EngineError, Result, Session};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const DEPARTMENTS: &[&str] = &["creative", "animation", "production", "backstage", "review"];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    id: String,
    name: String,
    role: String,
    personality: String,
    status: String,
    model: Option<String>,
    capabilities: Vec<String>,
    lead_suggestion: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Department {
    id: String,
    title: String,
    purpose: String,
    lead: Option<String>,
    roles: Vec<String>,
    skills: Vec<String>,
    commands: Vec<String>,
    candidates: Vec<Candidate>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    department: Department,
    instructions: String,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn workspace(s: &Session, p: &Value) -> Result<PathBuf> {
    let path = p
        .get("workspace")
        .map(|v| {
            v.as_str()
                .map(PathBuf::from)
                .ok_or_else(|| invalid("workspace must be a path"))
        })
        .transpose()?
        .unwrap_or_else(|| crate::workspace::workspace_root(s));
    Ok(fs::canonicalize(path)?)
}
fn path(root: &Path, id: &str) -> Result<PathBuf> {
    if !DEPARTMENTS.contains(&id) {
        return Err(invalid("Unknown cast department"));
    }
    let mut path = root.to_path_buf();
    for segment in ["crew", "departments", id, "AGENTS.md"] {
        path.push(segment);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("Cast files and directories must not be symlinks"));
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
    }
    Ok(path)
}
fn validate_candidate_id<'a>(c: &'a Candidate, ids: &mut HashSet<&'a String>) -> Result<()> {
    if c.id.is_empty()
        || c.id.len() > 64
        || !c
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || !ids.insert(&c.id)
    {
        return Err(invalid(
            "Candidate ids must be unique lowercase names without path components",
        ));
    }
    Ok(())
}

fn validate_candidate(c: &Candidate, roles: &[String]) -> Result<()> {
    if c.name.trim().is_empty()
        || c.name.split_whitespace().count() != 1
        || c.personality.trim().is_empty()
        || !roles.contains(&c.role)
    {
        return Err(invalid(
            "Each candidate needs a one-word name, personality and allowed department role",
        ));
    }
    if !["candidate", "cast", "reserve", "wings"].contains(&c.status.as_str()) {
        return Err(invalid("Casting must be candidate, cast, reserve or wings"));
    }
    Ok(())
}

fn validate_lead(d: &Department, id: &str) -> Result<()> {
    if id == "review" {
        if d.lead.as_deref() != Some("human") {
            return Err(invalid("Jesse remains the human lead of the Review Desk"));
        }
    } else if let Some(lead) = &d.lead
        && !d
            .candidates
            .iter()
            .any(|c| &c.id == lead && c.status == "cast")
    {
        return Err(invalid("Choose a cast member as department lead"));
    }
    Ok(())
}

fn validate(doc: &Document, id: &str) -> Result<()> {
    let d = &doc.department;
    if d.id != id || d.title.trim().is_empty() || d.roles.is_empty() || d.candidates.len() > 64 {
        return Err(invalid(
            "Department id, title, roles and candidate count must be valid",
        ));
    }
    let mut ids = HashSet::new();
    for c in &d.candidates {
        validate_candidate_id(c, &mut ids)?;
        validate_candidate(c, &d.roles)?;
    }
    validate_lead(d, id)
}
fn parse(markdown: &str, id: &str) -> Result<Document> {
    let (header, body) = markdown
        .strip_prefix("---\n")
        .and_then(|v| v.split_once("\n---\n"))
        .ok_or_else(|| {
            invalid("Cast AGENTS.md needs its JSON frontmatter; original file preserved")
        })?;
    let doc = Document {
        department: serde_json::from_str(header)
            .map_err(|e| invalid(format!("Invalid cast profile: {e}; file preserved")))?,
        instructions: body.strip_prefix('\n').unwrap_or(body).into(),
    };
    validate(&doc, id)?;
    Ok(doc)
}
fn render(doc: &Document) -> Result<String> {
    Ok(format!(
        "---\n{}\n---\n\n{}",
        serde_json::to_string_pretty(&doc.department).map_err(|e| invalid(e.to_string()))?,
        doc.instructions
    ))
}
fn read(root: &Path, id: &str) -> Result<Value> {
    let file = path(root, id)?;
    let markdown = match fs::metadata(&file) {
        Ok(meta) => {
            if meta.len() > 512 * 1024 {
                return Err(invalid("Cast file exceeds 512 KiB; preserved for repair"));
            }
            fs::read_to_string(&file)?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(invalid(format!(
                "Department role file missing: {}; no embedded defaults are available",
                file.display()
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let document = parse(&markdown, id)?;
    Ok(
        json!({"path":file,"exists":true,"revision":digest(markdown.as_bytes()),"markdown":markdown,"document":document}),
    )
}
pub fn cmd_list(s: &mut Session, p: &Value) -> Result<Value> {
    let root = workspace(s, p)?;
    let departments = DEPARTMENTS
        .iter()
        .map(|&id| read(&root, id))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"workspace":root,"departments":departments}))
}
fn cast_lock(root: &Path) -> Result<std::fs::File> {
    let lock_path = root.join("crew/.cast.lock");
    fs::create_dir_all(root.join("crew"))?;
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(invalid("Cast lock must not be a symlink"));
    }
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.lock()?;
    Ok(lock)
}

fn write_cast_file(file: &Path, markdown: &str) -> Result<()> {
    fs::create_dir_all(
        file.parent()
            .ok_or_else(|| invalid("Missing cast directory"))?,
    )?;
    let tmp = file.with_extension(format!(
        "{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| -> std::io::Result<()> {
        let mut out = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        out.write_all(markdown.as_bytes())?;
        out.sync_all()?;
        fs::rename(&tmp, file)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(())
}

pub fn cmd_save(s: &mut Session, p: &Value) -> Result<Value> {
    let root = workspace(s, p)?;
    let id = p["department"]
        .as_str()
        .ok_or_else(|| invalid("department required"))?;
    let file = path(&root, id)?;
    let doc: Document =
        serde_json::from_value(p["document"].clone()).map_err(|e| invalid(e.to_string()))?;
    validate(&doc, id)?;
    let markdown = render(&doc)?;
    if markdown.len() > 512 * 1024 {
        return Err(invalid("Cast file exceeds 512 KiB"));
    }
    let _lock = cast_lock(&root)?;
    let current = read(&root, id)?;
    if p["expected_revision"].as_str() != current["revision"].as_str() {
        return Err(invalid(
            "Cast conflict: AGENTS.md changed; compare crew.list with the retained draft before saving",
        ));
    }
    write_cast_file(&file, &markdown)?;
    Ok(json!({"ok":true,"department":read(&root,id)?}))
}
