//! The existing explicit auto-fix command, separate from read-only analyzer runs.
use magicstudio_engine::analyzers::root;
use std::{path::Path, process::Command};

/// Existing explicit auto-fix entrypoint; it does not run while reading or scanning.
pub fn run(target: &Path, dry_run: bool) -> i32 {
    let root = root(target);
    let mut status = 0;
    for (name, args) in [
        ("cargo", vec!["fmt".into(), "--all".into()]),
        ("eslint", vec!["--fix".into(), target.display().to_string()]),
        (
            "stylelint",
            vec![
                "--fix".into(),
                if target.is_dir() {
                    target.join("**/*.css").display().to_string()
                } else {
                    target.display().to_string()
                },
            ],
        ),
    ] {
        let local = root.join("node_modules/.bin").join(name);
        let executable = if local.is_file() { local } else { name.into() };
        eprintln!("{:?} {:?}", executable, args);
        if !dry_run {
            let code = Command::new(executable)
                .args(args)
                .current_dir(&root)
                .status()
                .map(|s| s.code().unwrap_or(2))
                .unwrap_or(2);
            if status == 0 {
                status = code;
            }
        }
    }
    status
}
