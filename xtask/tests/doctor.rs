use std::process::Command;

/// A failed Cargo invocation must block the commit even without a package name in its error.
#[test]
fn doctor_fails_when_cargo_check_has_no_package_breakdown() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let target = std::env::temp_dir().join(format!("magic-doctor-target-{unique}"));
    std::fs::write(&target, b"not a directory").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("doctor")
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    std::fs::remove_file(target).unwrap();

    let report = String::from_utf8_lossy(&output.stdout);
    assert!(
        report.contains("cargo check --workspace: FAILED"),
        "{report}"
    );
    assert!(
        !output.status.success(),
        "doctor accepted a failed check:\n{report}"
    );
}
