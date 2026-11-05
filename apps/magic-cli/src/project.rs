//! Turn what a caller typed into one `.magic` project file: a file, its folder, the
//! project's `hyperframes/index.html`, or the name of a recently opened project.
use std::path::{Path, PathBuf};

fn magic_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("magic"))
        })
        .collect();
    found.sort();
    found
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn existing_file(path: &Path) -> Result<PathBuf, String> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("magic"))
    {
        return canonical(path);
    }
    let site = path.parent().filter(|p| p.ends_with("hyperframes"));
    if path.file_name().is_some_and(|n| n == "index.html")
        && let Some(project_dir) = site.and_then(Path::parent)
        && let [only] = magic_files(project_dir).as_slice()
    {
        return canonical(only);
    }
    Err(format!("{} is not a .magic project file", path.display()))
}

fn existing_dir(path: &Path) -> Result<PathBuf, String> {
    for dir in path.ancestors().take(3) {
        match magic_files(dir).as_slice() {
            [] => {}
            [only] => return canonical(only),
            many => {
                return Err(format!(
                    "{} has {} .magic files; specify the file path",
                    dir.display(),
                    many.len()
                ));
            }
        }
    }
    Err(format!(
        "No .magic project file found in {}",
        path.display()
    ))
}

fn recent(token: &str) -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    let text = std::fs::read_to_string(config.join("magic/recent_projects.json")).ok()?;
    let entries: Vec<PathBuf> = serde_json::from_str(&text).ok()?;
    let token = token.to_lowercase();
    entries.into_iter().find(|p| {
        p.is_file()
            && [p.file_stem(), p.file_name()]
                .into_iter()
                .flatten()
                .any(|n| n.to_string_lossy().to_lowercase() == token)
    })
}

fn expand_home(raw: &str) -> PathBuf {
    match (raw.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(raw),
    }
}

/// The project file a caller means. A path that does not exist yet keeps its `.magic` name
/// so `project.new` can create it.
pub fn resolve(raw: &str) -> Result<PathBuf, String> {
    let path = expand_home(raw);
    if path.is_file() {
        return existing_file(&path);
    }
    if path.is_dir() {
        return existing_dir(&path);
    }
    if let Some(found) = recent(raw.trim()) {
        return canonical(&found);
    }
    let path = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("magic"))
    {
        path
    } else {
        path.with_extension("magic")
    };
    if path.exists() {
        canonical(&path)
    } else {
        std::path::absolute(&path).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magic-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("hyperframes")).unwrap();
        dir
    }

    #[test]
    fn a_folder_or_its_website_resolves_to_its_one_project_file() {
        let dir = scratch("one");
        std::fs::write(dir.join("Song.magic"), "{}").unwrap();
        std::fs::write(dir.join("hyperframes/index.html"), "").unwrap();
        let want = canonical(&dir.join("Song.magic")).unwrap();
        assert_eq!(resolve(dir.to_str().unwrap()).unwrap(), want);
        assert_eq!(
            resolve(dir.join("hyperframes/index.html").to_str().unwrap()).unwrap(),
            want
        );
        assert_eq!(
            resolve(dir.join("hyperframes").to_str().unwrap()).unwrap(),
            want
        );
    }

    #[test]
    fn two_project_files_in_one_folder_is_an_error_not_a_guess() {
        let dir = scratch("two");
        std::fs::write(dir.join("A.magic"), "{}").unwrap();
        std::fs::write(dir.join("B.magic"), "{}").unwrap();
        assert!(resolve(dir.to_str().unwrap()).is_err());
    }

    #[test]
    fn a_new_name_gets_the_magic_extension() {
        let dir = scratch("new");
        let got = resolve(dir.join("Fresh").to_str().unwrap()).unwrap();
        assert_eq!(got.extension().unwrap(), "magic");
    }
}
