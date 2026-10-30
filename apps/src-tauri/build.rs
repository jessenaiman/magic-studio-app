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
        {
            if output.status.success() {
                println!(
                    "cargo:rerun-if-changed={}",
                    String::from_utf8_lossy(&output.stdout).trim()
                );
            }
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
        "frontend/theatre-frame.js",
        "frontend/backstage-terminal.js",
        "frontend/project-id.mjs",
        "frontend/package.json",
        "frontend/bun.lock",
    ];
    for source in sources {
        println!("cargo:rerun-if-changed={source}");
    }
    let modified = |path: &str| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let bundle = modified("ui/vendor/theatre.bundle.js");
    if bundle.is_some()
        && modified("ui/vendor/backstage/backstage-terminal.js").is_some()
        && sources.iter().all(|s| {
            modified(s) <= modified("ui/vendor/backstage/backstage-terminal.js")
                && modified(s) <= bundle
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
    {
        bun(&["install"]);
    }
    bun(&["run", "build"]);
}

/// Every `ui/*.ts` is the source of a classic script: `ui/<name>.js` is generated from it with
/// Bun's transpiler (types stripped, no bundling, so index.html loads it unchanged). The `.js`
/// files are gitignored, rebuilt when missing or older than their source, like the Theatre bundle.
fn ui_typescript() {
    let Ok(entries) = std::fs::read_dir("ui") else { return };
    let modified = |path: &std::path::Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    for source in entries.flatten().map(|e| e.path()) {
        if source.extension().and_then(|e| e.to_str()) != Some("ts") {
            continue;
        }
        println!("cargo:rerun-if-changed={}", source.display());
        let output = source.with_extension("js");
        if output.exists() && modified(&source) <= modified(&output) {
            continue;
        }
        let status = std::process::Command::new("bun")
            .args(["build", "--no-bundle", "--target=browser"])
            .arg(&source)
            .arg("--outfile")
            .arg(&output)
            .status()
            .unwrap_or_else(|e| panic!("Compiling the UI TypeScript needs Bun on PATH: {e}"));
        assert!(status.success(), "`bun build` failed for {}", source.display());
    }
}
