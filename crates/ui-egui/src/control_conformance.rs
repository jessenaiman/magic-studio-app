//! Control-protocol conformance: every method in `docs/control-protocol.md`'s method table is sent
//! to the app's real control handler. A method the app answers `unknown command` for is missing.
//! Run by `cargo xtask ci`; fails when fewer methods are answered than `ANSWERED_MIN`.

use serde_json::json;

use crate::MagicApp;

const PROTOCOL: &str = include_str!("../../../docs/control-protocol.md");

/// Methods answered when this check was added; raise it as methods are implemented.
const ANSWERED_MIN: usize = 9;

/// Method names from the first column of the "Methods" table.
fn documented_methods() -> Vec<String> {
    let table = PROTOCOL
        .split_once("Methods (handlers in")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let mut methods = Vec::new();
    for line in table.lines().skip_while(|l| !l.starts_with('|')) {
        if !line.starts_with('|') {
            break;
        }
        let first = line.split('|').nth(1).unwrap_or_default();
        for name in first.split('`').skip(1).step_by(2) {
            if name.contains('.') && !methods.iter().any(|m| m == name) {
                methods.push(name.to_string());
            }
        }
    }
    methods
}

#[test]
#[ignore = "run by `cargo xtask ci` with --nocapture so the report is shown"]
fn control_protocol_methods_are_answered() {
    let methods = documented_methods();
    assert!(
        !methods.is_empty(),
        "no methods found in docs/control-protocol.md"
    );
    let ctx = egui::Context::default();
    let mut app = MagicApp::new(magic::Session::default());
    let mut missing = Vec::new();
    for method in &methods {
        let reply = app.handle_control_request(&ctx, method, json!({}));
        let unknown = matches!(&reply, Err(e) if e.starts_with("unknown command"));
        if unknown {
            missing.push(method.as_str());
        }
        println!(
            "{:<8} {method}",
            if unknown { "missing" } else { "answered" }
        );
    }
    let answered = methods.len() - missing.len();
    println!(
        "control protocol: {answered}/{} methods answered",
        methods.len()
    );
    assert!(
        answered >= ANSWERED_MIN,
        "control protocol regressed: {answered} answered, at least {ANSWERED_MIN} expected"
    );
}
