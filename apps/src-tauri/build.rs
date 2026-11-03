fn main() {
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".into());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!(
        "cargo:rustc-env=MAGICSTUDIO_BUILD_ID={}-{}",
        revision.trim(),
        now
    );
    // Recompute runtime identity after local commits, including named worktrees.
    let branch = std::process::Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    println!(
        "cargo:rustc-env=MAGICSTUDIO_BUILD_BRANCH={}",
        branch
            .trim()
            .strip_prefix("refs/heads/")
            .unwrap_or("detached")
    );
    for name in ["HEAD", "packed-refs", branch.trim()]
        .into_iter()
        .filter(|p| !p.is_empty())
    {
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--git-path", name])
            .output()
            && output.status.success()
        {
            println!(
                "cargo:rerun-if-changed={}",
                String::from_utf8_lossy(&output.stdout).trim()
            );
        }
    }
    println!("cargo:rerun-if-changed=ui");
    theatre_bundle();
    ui_typescript();
    tauri_build::build();
}

/// ui/vendor/theatre.bundle.js is generated (gitignored) from frontend/, so a fresh checkout
/// builds it here instead of needing a manual `bun run build`. It is rebuilt only when missing
/// or older than its sources, which keeps the `ui` change watch above from looping.
fn theatre_bundle() {
    let sources = [
        "frontend/theatre-frame.ts",
        "frontend/rig-review-stage.ts",
        "frontend/stage-canvas.ts",
        "frontend/backstage-terminal.ts",
        "frontend/project-id.ts",
        "frontend/analytics-charts.ts",
        "frontend/package.json",
        "frontend/bun.lock",
    ];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    let modified = |path: &str| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let bundle = modified("ui/vendor/theatre.bundle.js");
    if modified("ui/vendor/rig-review.bundle.js").is_some()
        && bundle.is_some()
        && modified("ui/vendor/konva.bundle.js").is_some()
        && modified("ui/vendor/backstage/backstage-terminal.js").is_some()
        && modified("ui/vendor/mayhem-charts.bundle.js").is_some()
        && sources.iter().all(|s| {
            modified(s) <= modified("ui/vendor/rig-review.bundle.js")
                && modified(s) <= modified("ui/vendor/backstage/backstage-terminal.js")
                && modified(s) <= bundle
                && modified(s) <= modified("ui/vendor/mayhem-charts.bundle.js")
        })
    {
        return;
    }
    let bun = |args: &[&str]| {
        let status = std::process::Command::new("bun")
            .args(args)
            .current_dir("frontend")
            .status()
            .unwrap_or_else(|e| panic!("Building the Theatre panel needs Bun on PATH: {e}"));
        assert!(
            status.success(),
            "`bun {}` failed in apps/src-tauri/frontend",
            args.join(" ")
        );
    };
    if !std::path::Path::new("frontend/node_modules/@theatre/studio").exists()
        || !std::path::Path::new("frontend/node_modules/@xterm/xterm").exists()
        || !std::path::Path::new("frontend/node_modules/konva").exists()
        || !std::path::Path::new("frontend/node_modules/echarts").exists()
    {
        bun(&["install"]);
    }
    bun(&["run", "build"]);
}

fn find_tsc(repo_root: &std::path::Path) -> std::path::PathBuf {
    // The tsc process runs with the repository root as its working directory, so a relative
    // `../../node_modules/.bin/tsc` (relative to this crate) would not resolve there.
    let local = repo_root.join("node_modules/.bin/tsc");
    if local.is_file() {
        return local.canonicalize().unwrap_or(local);
    }
    if let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(repo_root)
        .output()
        && out.status.success()
    {
        let raw = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let common = std::path::Path::new(&raw);
        let resolved = if common.is_absolute() {
            common.to_path_buf()
        } else {
            repo_root.join(common)
        };
        if let Some(main_root) = resolved.parent() {
            let shared = main_root.join("node_modules/.bin/tsc");
            if shared.is_file() {
                return shared.canonicalize().unwrap_or(shared);
            }
        }
    }
    std::path::PathBuf::from("tsc")
}

/// Every `ui/*.ts` is the source of a classic script: `ui/<name>.js` is generated from it with
/// Bun's transpiler and typechecked with `tsc -p .` so type errors fail the build.
fn ui_typescript() {
    println!("cargo:rerun-if-changed=../../tsconfig.json");
    println!("cargo:rerun-if-changed=types");
    let Ok(entries) = std::fs::read_dir("ui") else {
        return;
    };
    let modified = |path: &std::path::Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let mut rebuilt = false;
    for source in entries.flatten().map(|e| e.path()) {
        if source.extension().and_then(|e| e.to_str()) != Some("ts") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", source.display());
        let output = source.with_extension("js");
        if output.exists() && modified(&source) <= modified(&output) {
            continue;
        }
        rebuilt = true;
        let status = std::process::Command::new("bun")
            .args(["build", "--no-bundle", "--target=browser"])
            .arg(&source)
            .arg("--outfile")
            .arg(&output)
            .status()
            .unwrap_or_else(|e| panic!("Compiling the UI TypeScript needs Bun on PATH: {e}"));
        assert!(
            status.success(),
            "`bun build` failed for {}",
            source.display()
        );
    }
    if rebuilt {
        let repo_root = std::path::Path::new("../..");
        let tsc = find_tsc(repo_root);
        let status = std::process::Command::new(&tsc)
            .args(["-p", "."])
            .current_dir(repo_root)
            .status()
            .unwrap_or_else(|e| {
                panic!(
                    "Typechecking UI TypeScript needs tsc ({}): {e}",
                    tsc.display()
                )
            });
        assert!(
            status.success(),
            "`tsc -p .` failed; fix TypeScript errors before building"
        );
    }
}
