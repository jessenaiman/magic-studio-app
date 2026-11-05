use serde_json::json;

#[test]
fn craft_cli_rejects_unknown_apps_and_non_string_arguments() {
    let mut session = magic::Session::default();
    for params in [
        json!({"app":"bash", "args":["--help"]}),
        json!({"app":"filmcraft", "args":[42]}),
        json!({"app":"filmcraft", "args":[]}),
    ] {
        let error = session
            .execute("craft.cli", params)
            .unwrap_err()
            .to_string();
        assert!(error.contains("app") || error.contains("args"), "{error}");
        assert!(
            !error.contains("unknown command"),
            "command must be registered: {error}"
        );
    }
}
