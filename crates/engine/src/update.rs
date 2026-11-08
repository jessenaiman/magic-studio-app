//! `app.update`: is there a newer published Magic Studio release than this build?
//!
//! Shares `craft-update`'s HTTP client (user agent, proxy, GitHub token, rate-limit handling)
//! with the Craft-app checks in [`crate::doctor`], so one credential and one policy cover both.
//! Nothing is downloaded or installed here: a build that was installed by hand must not rewrite
//! itself. The reply is what a surface needs to tell a person a release exists.

use crate::{EngineError, Result, Session};
use craft_update::http::{Fetched, Http};
use craft_update::{release, version};
use serde_json::{Value, json};

/// The repository that publishes Magic Studio releases.
const REPO: &str = "jessenaiman/magic-studio-app";
const RELEASES_API: &str = "https://api.github.com/repos/jessenaiman/magic-studio-app/releases?per_page=10";
const RELEASES_PAGE: &str = "https://github.com/jessenaiman/magic-studio-app/releases";

fn failed(message: impl Into<String>) -> EngineError {
    EngineError::ExternalTool("update".into(), message.into())
}

fn no_release(installed: &str, reason: &str) -> Value {
    json!({
        "installed": installed,
        "latest": Value::Null,
        "update_available": false,
        "state": reason,
        "releases": RELEASES_PAGE,
    })
}

/// Checks the newest GitHub release against this build's version. Read-only and never fatal:
/// a machine without network gets a `state` instead of an error the caller must handle.
pub fn cmd_app_update(_s: &mut Session, _p: &Value) -> Result<Value> {
    let installed_text = env!("CARGO_PKG_VERSION");
    let installed = version::parse(installed_text);
    let token = ["GH_TOKEN", "GITHUB_TOKEN"].iter().find_map(|var| std::env::var(var).ok().filter(|t| !t.trim().is_empty()));
    let http =
        Http::new(concat!("MagicStudio/", env!("CARGO_PKG_VERSION"))).map_err(|e| failed(format!("cannot check for updates: {e}")))?.with_github_token(token);
    match http.get_api(RELEASES_API, None) {
        Ok(Fetched::Body { text, .. }) => {
            let latest = release::parse_github_release_list(&text, REPO, true).map_err(|e| failed(format!("cannot read the release list: {e}")))?;
            let Some(latest) = latest else {
                return Ok(no_release(installed_text, "no published release yet"));
            };
            let available = match (&latest.version, &installed) {
                (Some(latest), Some(installed)) => latest > installed,
                _ => false,
            };
            Ok(json!({
                "installed": installed_text,
                "latest": latest.tag,
                "latest_version": latest.version.as_ref().map(ToString::to_string),
                "update_available": available,
                "state": if available { "a newer release is published" } else { "this build is current" },
                "prerelease": latest.prerelease,
                "published_at": latest.published_at,
                "name": latest.name,
                "url": latest.html_url,
                "releases": RELEASES_PAGE,
            }))
        }
        Ok(Fetched::NotFound) => Ok(no_release(installed_text, "no published release yet")),
        Ok(Fetched::RateLimited { reset }) => Ok(json!({
            "installed": installed_text,
            "latest": Value::Null,
            "update_available": false,
            "state": "GitHub rate limit reached; try again later",
            "retry_at": reset,
            "releases": RELEASES_PAGE,
        })),
        Ok(Fetched::NotModified) => Ok(no_release(installed_text, "no change since the last check")),
        Err(error) => Ok(json!({
            "installed": installed_text,
            "latest": Value::Null,
            "update_available": false,
            "state": format!("could not check for updates: {error}"),
            "releases": RELEASES_PAGE,
        })),
    }
}
