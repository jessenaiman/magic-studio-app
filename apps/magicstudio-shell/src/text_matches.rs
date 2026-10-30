use serde_json::{Value, json};
use std::{collections::HashSet, process::Command, time::SystemTime};

#[tauri::command]
pub fn text_matches() -> Result<Value, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(&root)
            .args(args)
            .output()
            .map_err(|e| e.to_string())
    };
    let head = git(&["rev-parse", "HEAD"])?;
    if !head.status.success() {
        return Err(String::from_utf8_lossy(&head.stderr).into_owned());
    }
    let terms =
        std::fs::read_to_string(root.join(".backstage-text-terms")).map_err(|e| e.to_string())?;
    let scope =
        std::fs::read_to_string(root.join(".backstage-text-scope")).map_err(|e| e.to_string())?;
    let paths: Vec<&str> = scope
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
        .collect();
    if paths.is_empty() || terms.lines().all(|s| s.is_empty() || s.starts_with('#')) {
        return Err("Terms or source scope is empty; not measured".into());
    }
    let mut report = String::new();
    for term in terms
        .lines()
        .filter(|s| !s.is_empty() && !s.starts_with('#'))
    {
        let mut args = vec![
            "--no-pager",
            "grep",
            "-z",
            "-n",
            "-o",
            "-I",
            "-P",
            "-e",
            term,
            "--",
        ];
        args.extend_from_slice(&paths);
        let output = git(&args)?;
        if !matches!(output.status.code(), Some(0 | 1)) {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        let rows: Vec<&str> = text.lines().collect();
        let files: HashSet<&str> = rows
            .iter()
            .filter_map(|s| s.split_once('\0').map(|v| v.0))
            .collect();
        report.push_str(&format!(
            "{term}: {} occurrences in {} files\n{}\n",
            rows.len(),
            files.len(),
            text.replace('\0', ":")
        ));
    }
    Ok(
        json!({"root":root,"revision":String::from_utf8_lossy(&head.stdout).trim(),"refreshed_unix_seconds":SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),"scope":scope,"terms_file":root.join(".backstage-text-terms"),"report":report,"source":"git grep -z -n -o -I -P; tracked files; one output record per occurrence"}),
    )
}
