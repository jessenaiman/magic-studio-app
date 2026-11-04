//! Read the runner's manifest and original reports. No source analysis or grading.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
pub struct Inputs {
    pub baseline: Option<PathBuf>,
    pub candidate: Option<PathBuf>,
    pub baseline_sha: Option<String>,
    pub candidate_sha: Option<String>,
    pub tool_filter: Option<String>,
}
pub struct Report {
    pub path: PathBuf,
    pub sha: Option<String>,
    pub native: Value,
}
pub struct Reports {
    pub baseline: Report,
    pub candidate: Report,
}

pub fn default_analytics_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    Some(
        cwd.ancestors()
            .find(|p| p.join(".git").exists())?
            .join("target/analytics"),
    )
}
pub fn default_analytics_path(name: &str) -> Option<PathBuf> {
    let path = default_analytics_dir()?.join(name);
    path.is_file().then_some(path)
}
impl Inputs {
    pub fn from_saved_analytics() -> Option<Self> {
        Some(Self {
            candidate: Some(default_analytics_path("analyzers.json")?),
            ..Self::default()
        })
    }
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut input = Self::default();
        let mut args = args.iter();
        while let Some(flag) = args.next() {
            if !flag.starts_with("--audit-") {
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{flag} requires a value"))?;
            match flag.as_str() {
                "--audit-report" | "--audit-candidate" => input.candidate = Some(value.into()),
                "--audit-baseline" => input.baseline = Some(value.into()),
                "--audit-baseline-sha" => input.baseline_sha = Some(value.clone()),
                "--audit-candidate-sha" => input.candidate_sha = Some(value.clone()),
                "--audit-tool" => input.tool_filter = Some(value.clone()),
                _ => {
                    return Err(format!(
                        "Unsupported audit option {flag}; use --audit-report PATH"
                    ));
                }
            }
        }
        Ok(input)
    }
    pub fn load(&self) -> Result<Reports, String> {
        let load = |path: Option<&Path>, expected: Option<&str>| -> Result<Report, String> {
            let Some(path) = path else {
                return Ok(Report {
                    path: PathBuf::new(),
                    sha: None,
                    native: Value::Null,
                });
            };
            let metadata = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
            if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
                return Err(format!(
                    "{}: expected a regular report file up to 32 MiB",
                    path.display()
                ));
            }
            let native: Value = serde_json::from_slice(
                &fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
            )
            .map_err(|e| e.to_string())?;
            let sha = (native["schema"] == "magic.analyzers/1" && native["working_tree"] == "")
                .then(|| native["revision"].as_str().map(str::to_owned))
                .flatten();
            if expected.is_some_and(|wanted| sha.as_deref() != Some(wanted)) {
                return Err(format!(
                    "{}: recorded clean revision does not match requested SHA",
                    path.display()
                ));
            }
            Ok(Report {
                path: path.to_path_buf(),
                sha,
                native,
            })
        };
        let candidate = self
            .candidate
            .clone()
            .or_else(|| default_analytics_path("analyzers.json"))
            .ok_or("No native reports. Run magic-cli audit, or pass --audit-report PATH")?;
        let result = Reports {
            baseline: load(self.baseline.as_deref(), self.baseline_sha.as_deref())?,
            candidate: load(Some(&candidate), self.candidate_sha.as_deref())?,
        };
        if let Some(id) = &self.tool_filter {
            result.validate_tool(id)?;
        }
        Ok(result)
    }
}
impl Reports {
    pub fn tool_ids(&self) -> Vec<String> {
        self.candidate.native["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| t["tool"].as_str().map(str::to_owned))
            .collect()
    }
    pub fn validate_tool(&self, id: &str) -> Result<(), String> {
        if self.tool_ids().iter().any(|t| t == id) {
            Ok(())
        } else {
            Err(format!("No recorded analyzer named {id}"))
        }
    }
    pub fn detail(&self, index: usize) -> String {
        let Some(tool) = self.candidate.native["tools"].get(index) else {
            return serde_json::to_string_pretty(&self.candidate.native).unwrap_or_default();
        };
        let body = [
            tool["error"].as_str(),
            tool["stdout"].as_str(),
            tool["stderr"].as_str(),
            tool["native_report"]["contents"].as_str(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");
        let invocation = serde_json::json!({"scope":tool["scope"],"executable":tool["executable"],"args":tool["args"],"exit_code":tool["exit_code"],"signal":tool["signal"]});
        format!(
            "{}\n\nInvocation and scope:\n{}",
            if body.is_empty() {
                "No stdout or stderr was produced."
            } else {
                &body
            },
            serde_json::to_string_pretty(&invocation).unwrap_or_default()
        )
    }
    pub fn detail_count(&self, index: usize) -> usize {
        self.detail(index).lines().count()
    }
    pub fn artifact(&self, index: usize) -> Option<PathBuf> {
        let tool = self.candidate.native["tools"].get(index)?["tool"].as_str()?;
        if !tool.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        let path =
            Path::new(self.candidate.native["artifacts"].as_str()?).join(format!("{tool}.stdout"));
        path.is_file().then_some(path)
    }
    pub fn markdown(&self) -> String {
        let mut text = format!(
            "# Native analyzer reports\n\nSource: {}\n\n",
            self.candidate.path.display()
        );
        if self.candidate.native["schema"] != "magic.analyzers/1" {
            return text + "Legacy JSON (not regraded):\n\n" + &self.detail(0);
        }
        for (index, id) in self.tool_ids().iter().enumerate() {
            text += &format!("## {id}\n\n{}\n\n", self.detail(index));
        }
        text
    }
    pub fn markdown_tool(&self, id: &str) -> Result<String, String> {
        self.validate_tool(id)?;
        Ok(self.detail(self.tool_ids().iter().position(|t| t == id).unwrap_or(0)))
    }
}
