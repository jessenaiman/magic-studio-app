//! Adapter for existing native reports only. Never invokes a checker or writes an archive.
use crate::data::clean;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    io::Read,
    path::PathBuf,
};
const MAX: u64 = 32 * 1024 * 1024;
#[derive(Clone, Default)]
pub struct Inputs {
    pub baseline: Option<PathBuf>,
    pub candidate: Option<PathBuf>,
    pub baseline_supplement: Option<PathBuf>,
    pub candidate_supplement: Option<PathBuf>,
    pub baseline_sha: Option<String>,
    pub candidate_sha: Option<String>,
    pub tool_filter: Option<String>,
}
pub struct Report {
    pub path: PathBuf,
    pub sha: Option<String>,
    pub native: Value,
    pub source_links: BTreeMap<String, Option<PathBuf>>,
    pub links_partial: bool,
}
pub struct Comparison {
    pub baseline: Report,
    pub candidate: Report,
    pub baseline_supplement: Option<Report>,
    pub candidate_supplement: Option<Report>,
}
impl Inputs {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut input = Self::default();
        let mut i = 0;
        while i < args.len() {
            let slot = match args[i].as_str() {
                "--audit-baseline" => Some(0),
                "--audit-candidate" => Some(1),
                "--audit-baseline-supplement" => Some(2),
                "--audit-candidate-supplement" => Some(3),
                "--audit-baseline-sha" => Some(4),
                "--audit-candidate-sha" => Some(5),
                "--audit-tool" => Some(6),
                _ => None,
            };
            if let Some(slot) = slot {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| format!("{} requires a value", args[i - 1]))?;
                if matches!(slot, 4 | 5)
                    && !(value.len() >= 7
                        && value.len() <= 64
                        && value.chars().all(|c| c.is_ascii_hexdigit()))
                {
                    return Err("Source SHA must be 7–64 hexadecimal characters".into());
                }
                match slot {
                    0 => input.baseline = Some(value.into()),
                    1 => input.candidate = Some(value.into()),
                    2 => input.baseline_supplement = Some(value.into()),
                    3 => input.candidate_supplement = Some(value.into()),
                    4 => input.baseline_sha = Some(value.clone()),
                    5 => input.candidate_sha = Some(value.clone()),
                    _ => input.tool_filter = Some(value.clone()),
                };
            }
            i += 1;
        }
        Ok(input)
    }
    pub fn load(&self) -> Result<Comparison, String> {
        Ok(Comparison {
            baseline: load(
                self.baseline
                    .as_ref()
                    .ok_or("--audit-baseline is required")?,
                self.baseline_sha.clone(),
            )?,
            candidate: load(
                self.candidate
                    .as_ref()
                    .ok_or("--audit-candidate is required")?,
                self.candidate_sha.clone(),
            )?,
            baseline_supplement: self
                .baseline_supplement
                .as_ref()
                .map(|p| load(p, self.baseline_sha.clone()))
                .transpose()?,
            candidate_supplement: self
                .candidate_supplement
                .as_ref()
                .map(|p| load(p, self.candidate_sha.clone()))
                .transpose()?,
        })
    }
}
fn load(path: &PathBuf, sha: Option<String>) -> Result<Report, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.len() > MAX {
        return Err(format!(
            "{} must be a regular JSON file <=32MiB",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX as usize {
        return Err("Audit report grew beyond32MiB".into());
    }
    let native: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{}: invalid native JSON: {e}", path.display()))?;
    for key in ["issues", "errors", "toolResults"] {
        if !native[key].is_array() {
            return Err(format!("{}: missing native {key} array", path.display()));
        }
    }
    if native
        .get("skippedFiles")
        .is_some_and(|value| !value.is_array())
    {
        return Err(format!(
            "{}: native skippedFiles must be an array when supplied",
            path.display()
        ));
    }
    if !native["metadata"].is_object() {
        return Err(format!(
            "{}: missing native metadata object",
            path.display()
        ));
    }
    let mut source_links = BTreeMap::new();
    let mut links_partial = false;
    if let Some(root) = native["metadata"]["repositoryRoot"]
        .as_str()
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok())
    {
        let started = std::time::Instant::now();
        for issue in native["issues"].as_array().unwrap() {
            if source_links.len() >= 2000
                || started.elapsed() > std::time::Duration::from_millis(500)
            {
                links_partial = true;
                break;
            }
            let Some(raw) = issue["filePath"].as_str() else {
                continue;
            };
            if source_links.contains_key(raw) {
                continue;
            }
            let relative = std::path::Path::new(raw);
            let link = if raw.chars().any(char::is_control)
                || relative.is_absolute()
                || relative
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                None
            } else {
                root.join(relative)
                    .canonicalize()
                    .ok()
                    .filter(|p| p.starts_with(&root) && p.is_file())
            };
            source_links.insert(raw.to_string(), link);
        }
    }
    Ok(Report {
        source_links,
        links_partial,
        path: path.canonicalize().unwrap_or_else(|_| path.clone()),
        sha,
        native,
    })
}
fn cell(v: impl AsRef<str>) -> String {
    clean(v.as_ref()).replace('|', "\\|").replace('`', "'")
}
fn field(v: &Value, key: &str) -> String {
    v.get(key)
        .map(|v| v.as_str().map(cell).unwrap_or_else(|| v.to_string()))
        .unwrap_or_else(|| "Unavailable".into())
}
fn number(v: &Value, key: &str) -> Option<i64> {
    v[key].as_i64()
}
fn n(v: Option<i64>) -> String {
    v.map(|n| n.to_string())
        .unwrap_or_else(|| "Unavailable".into())
}
fn delta(a: Option<i64>, b: Option<i64>) -> String {
    match (a, b) {
        (Some(a), Some(b)) => format!("{:+}", b - a),
        _ => "Unavailable".into(),
    }
}
impl Report {
    pub fn issues(&self) -> usize {
        self.native["issues"].as_array().map_or(0, Vec::len)
    }
    pub fn errors(&self) -> usize {
        self.native["errors"].as_array().map_or(0, Vec::len)
    }
    pub fn tools(&self) -> &[Value] {
        self.native["toolResults"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}
impl Comparison {
    pub fn validate_tool(&self, id: &str) -> Result<(), String> {
        if self.tool_ids().iter().any(|tool| tool == id) {
            Ok(())
        } else {
            Err(format!(
                "Native toolId not present in supplied reports: {}",
                cell(id)
            ))
        }
    }
    pub fn tool_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for tool in self.baseline.tools().iter().chain(self.candidate.tools()) {
            if let Some(id) = tool["toolId"].as_str()
                && !ids.iter().any(|s| s == id)
            {
                ids.push(id.to_string());
            }
        }
        ids
    }
    pub fn finding(&self, tool: usize, index: usize) -> Option<&Value> {
        let ids = self.tool_ids();
        let id = ids.get(tool)?;
        self.candidate.native["issues"]
            .as_array()?
            .iter()
            .filter(|i| i["toolId"] == id.as_str())
            .nth(index)
    }
    pub fn finding_count(&self, tool: usize) -> usize {
        let ids = self.tool_ids();
        let Some(id) = ids.get(tool) else { return 0 };
        self.candidate.native["issues"].as_array().map_or(0, |a| {
            a.iter().filter(|i| i["toolId"] == id.as_str()).count()
        })
    }
    pub fn finding_link(&self, tool: usize, index: usize) -> Option<&std::path::Path> {
        let raw = self.finding(tool, index)?["filePath"].as_str()?;
        self.candidate.source_links.get(raw)?.as_deref()
    }
    pub fn markdown(&self) -> String {
        let a = &self.baseline;
        let b = &self.candidate;
        let mut out = String::new();
        writeln!(out,"# MAYHEM — native audit comparison\n\nRead-only output from Backstage. No audits, agents, models or inference were run to produce this view.\n").unwrap();
        writeln!(out,"## Sources and scope\n\n| Native field / source | Baseline | Candidate |\n|---|---|---|").unwrap();
        writeln!(
            out,
            "| Report | `{}` | `{}` |",
            cell(a.path.display().to_string()),
            cell(b.path.display().to_string())
        )
        .unwrap();
        for key in [
            "repositoryRoot",
            "startedAt",
            "completedAt",
            "executionMode",
        ] {
            writeln!(
                out,
                "| metadata.{key} | {} | {} |",
                field(&a.native["metadata"], key),
                field(&b.native["metadata"], key)
            )
            .unwrap();
        }
        writeln!(
            out,
            "| Source SHA (caller-supplied; not embedded in native JSON) | {} | {} |",
            a.sha.as_deref().unwrap_or("Unavailable"),
            b.sha.as_deref().unwrap_or("Unavailable")
        )
        .unwrap();
        writeln!(out,"| issues records | {} | {} |\n| errors records | {} | {} |\n| skippedFiles records | {} | {} |",a.issues(),b.issues(),a.errors(),b.errors(),a.native["skippedFiles"].as_array().map(|v|v.len().to_string()).unwrap_or_else(||"Unavailable".into()),b.native["skippedFiles"].as_array().map(|v|v.len().to_string()).unwrap_or_else(||"Unavailable".into())).unwrap();
        writeln!(
            out,
            "\n**Primary issues: {} → {} (Δ {:+}). Primary errors: {} → {} (Δ {:+}).**",
            a.issues(),
            b.issues(),
            b.issues() as i64 - a.issues() as i64,
            a.errors(),
            b.errors(),
            b.errors() as i64 - a.errors() as i64
        )
        .unwrap();
        writeln!(out,"\nCounts are native report records, not a quality score. Scope differs where filesAnalyzed differs; fewer findings alone does not prove that all removed findings were fixed. Native errors and skipped files remain unresolved evidence.\n").unwrap();
        tools_table(&mut out, "Native toolResults", a, b);
        writeln!(out,"\n## Native errors (grouped without renaming toolId / phase / kind)\n\n| toolId | phase | kind | Baseline records | Candidate records |\n|---|---|---|---:|---:|").unwrap();
        let errors_a = errors(a);
        let errors_b = errors(b);
        let keys: BTreeSet<_> = errors_a.keys().chain(errors_b.keys()).collect();
        for key in keys {
            writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                cell(&key.0),
                cell(&key.1),
                cell(&key.2),
                errors_a.get(key).copied().unwrap_or(0),
                errors_b.get(key).copied().unwrap_or(0)
            )
            .unwrap();
        }
        writeln!(out,"\n## Native patternId changes\n\nLargest absolute changes in issue-record counts; no rule names or severity fields are rewritten.\n\n| toolId | patternId | Baseline records | Candidate records | Δ |\n|---|---|---:|---:|---:|").unwrap();
        let rules_a = rules(a);
        let rules_b = rules(b);
        let keys: BTreeSet<_> = rules_a.keys().chain(rules_b.keys()).collect();
        let mut changes: Vec<_> = keys
            .into_iter()
            .map(|key| {
                let av = rules_a.get(key).copied().unwrap_or(0);
                let bv = rules_b.get(key).copied().unwrap_or(0);
                (key, av, bv)
            })
            .collect();
        changes.sort_by_key(|(_, a, b)| std::cmp::Reverse((*b - *a).abs()));
        for (key, av, bv) in changes.into_iter().filter(|(_, a, b)| a != b).take(10) {
            writeln!(
                out,
                "| {} | {} | {} | {} | {:+} |",
                cell(&key.0),
                cell(&key.1),
                av,
                bv,
                bv - av
            )
            .unwrap();
        }
        writeln!(
            out,
            "\n## Supplemental reports — separate from primary execution"
        )
        .unwrap();
        match (&self.baseline_supplement, &self.candidate_supplement) {
            (Some(sa), Some(sb)) => {
                writeln!(out,"\nBaseline: `{}`  \nCandidate: `{}`\n\nissues records: {} → {}; errors records: {} → {}. These do not erase primary execution errors.",cell(sa.path.display().to_string()),cell(sb.path.display().to_string()),sa.issues(),sb.issues(),sa.errors(),sb.errors()).unwrap();
                tools_table(&mut out, "Supplement native toolResults", sa, sb);
                if supplement_safe(a, sa) && supplement_safe(b, sb) {
                    writeln!(out,"\nPrimary + separate Trivy retry issue records: **{} → {} (Δ {:+})**. Addition is shown because primary Trivy issueCount is 0 in both reports; no cross-tool deduplication or completeness claim is made.",a.issues()+sa.issues(),b.issues()+sb.issues(),(b.issues()+sb.issues()) as i64-(a.issues()+sa.issues()) as i64).unwrap();
                }
                for (label, r) in [("Baseline supplement", sa), ("Candidate supplement", sb)] {
                    for error in r.native["errors"].as_array().unwrap() {
                        writeln!(
                            out,
                            "\n- {label}: toolId `{}`, phase `{}`, kind `{}`, level `{}` — {}",
                            field(error, "toolId"),
                            field(error, "phase"),
                            field(error, "kind"),
                            field(error, "level"),
                            field(error, "message")
                        )
                        .unwrap();
                    }
                }
            }
            _ => {
                writeln!(out,"\nUnavailable: both supplemental paths must be supplied for a paired comparison. No supplemental counts are invented.").unwrap();
            }
        }
        writeln!(out,"\n## Incomplete / not run\n\n- Primary errors are retained; a tool's native status `success` does not clear its nonzero errorCount.\n- {}\n- Source SHAs are unavailable unless explicitly supplied; current repository HEAD is not substituted for report provenance.\n- Full native issue/error details and source filePath/line/patternId remain in the report files listed above. No source files or audit reports were modified.\n",agent_note(a,b)).unwrap();
        out
    }
}
fn tools_table(out: &mut String, title: &str, a: &Report, b: &Report) {
    writeln!(out,"\n## {title}\n\n| Tool / native toolId | issueCount before → after | Δ | errorCount before → after |\n|---|---:|---:|---:|").unwrap();
    let mut ids = Vec::new();
    for tool in a.tools().iter().chain(b.tools()) {
        if let Some(id) = tool["toolId"].as_str()
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    for id in &ids {
        let av = a
            .tools()
            .iter()
            .find(|t| t["toolId"] == *id)
            .unwrap_or(&Value::Null);
        let bv = b
            .tools()
            .iter()
            .find(|t| t["toolId"] == *id)
            .unwrap_or(&Value::Null);
        writeln!(
            out,
            "| {} | {} → {} | {} | {} → {} |",
            tool_cell(id),
            n(number(av, "issueCount")),
            n(number(bv, "issueCount")),
            delta(number(av, "issueCount"), number(bv, "issueCount")),
            n(number(av, "errorCount")),
            n(number(bv, "errorCount"))
        )
        .unwrap();
    }
    writeln!(out,"\nNative errorCount remains visible; status success is not a completeness claim. {}\n\n### {title} — coverage and status\n\n| Tool / native toolId | filesAnalyzed before → after | status before → after |\n|---|---:|---|",agent_note(a,b)).unwrap();
    for id in ids {
        let av = a
            .tools()
            .iter()
            .find(|t| t["toolId"] == id)
            .unwrap_or(&Value::Null);
        let bv = b
            .tools()
            .iter()
            .find(|t| t["toolId"] == id)
            .unwrap_or(&Value::Null);
        writeln!(
            out,
            "| {} | {} → {} | {} → {} |",
            tool_cell(id),
            n(number(av, "filesAnalyzed")),
            n(number(bv, "filesAnalyzed")),
            field(av, "status"),
            field(bv, "status")
        )
        .unwrap();
    }
}

fn errors(report: &Report) -> BTreeMap<(String, String, String), usize> {
    let mut counts = BTreeMap::new();
    for e in report.native["errors"].as_array().unwrap() {
        *counts
            .entry((field(e, "toolId"), field(e, "phase"), field(e, "kind")))
            .or_default() += 1;
    }
    counts
}
fn rules(report: &Report) -> BTreeMap<(String, String), i64> {
    let mut counts = BTreeMap::new();
    for e in report.native["issues"].as_array().unwrap() {
        *counts
            .entry((field(e, "toolId"), field(e, "patternId")))
            .or_default() += 1;
    }
    counts
}
fn supplement_safe(primary: &Report, supplement: &Report) -> bool {
    primary.native["metadata"]["repositoryRoot"] == supplement.native["metadata"]["repositoryRoot"]
        && primary
            .tools()
            .iter()
            .any(|t| t["toolId"] == "Trivy" && t["issueCount"] == 0)
        && supplement.native["issues"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["toolId"] == "Trivy")
}

fn agent_note(a: &Report, b: &Report) -> &'static str {
    if a.tools().iter().chain(b.tools()).any(|t| {
        t["toolId"]
            .as_str()
            .is_some_and(|id| id.eq_ignore_ascii_case("agentlinter"))
    }) {
        "Agentlinter native results are included; no checker was run by Backstage."
    } else {
        "Agentlinter: not present in supplied native toolResults; not run or verified by Backstage."
    }
}

fn tool_cell(id: &str) -> String {
    format!("{} · `{}`", cell(tool_label(id)), cell(id))
}
/// Display-only labels; native JSON keys and counts are never rewritten.
pub fn tool_label(id: &str) -> &str {
    match id {
        "ESLint9" => "JavaScript quality",
        "Ruff" => "Python style",
        "markdownlint" => "Markdown style",
        "shellcheck" => "Shell review",
        "Trivy" => "Package security",
        "Semgrep" => "Security patterns",
        "Stylelint" => "Stylesheet quality",
        "spectral" => "API schema",
        "Bandit" => "Python security",
        "PyLintPython3" => "Python quality",
        "Checkov" => "Config security",
        "Lizard" => "Code complexity",
        "PMD7" => "Java review",
        "Prospector" => "Python review",
        "jackson" => "JSON quality",
        _ => id,
    }
}
pub fn rule_label(id: &str) -> &str {
    match id {
        "ESLint9_no-undef" => "Unknown names",
        "ESLint9_react_prop-types" => "Prop validation",
        "ESLint9_react_jsx-sort-props" | "ESLint9_@stylistic_jsx-sort-props" => "Property order",
        "ESLint9_security_detect-object-injection" => "Dynamic keys",
        "ESLint9_react_react-in-jsx-scope" => "React scope",
        "ESLint9_react-perf_jsx-no-new-object-as-prop" => "Inline objects",
        "ESLint9_react_jsx-newline" | "ESLint9_@stylistic_jsx-newline" => "JSX spacing",
        "ESLint9_react_forbid-component-props" => "Restricted props",
        "ESLint9_react_jsx-indent" | "ESLint9_@stylistic_jsx-indent" => "JSX indentation",
        "ESLint9_react_jsx-indent-props" | "ESLint9_@stylistic_jsx-indent-props" => {
            "JSX prop indent"
        }
        "Ruff_E501_line-too-long" => "Long lines",
        "markdownlint_MD032" => "Blank lines",
        _ => id,
    }
}
fn rule_doc(id: &str) -> Option<String> {
    if id == "ESLint9_no-undef" {
        return Some("https://eslint.org/docs/latest/rules/no-undef".into());
    }
    if id == "ESLint9_security_detect-object-injection" {
        return Some("https://github.com/eslint-community/eslint-plugin-security/blob/main/docs/rules/detect-object-injection.md".into());
    }
    if id == "ESLint9_react-perf_jsx-no-new-object-as-prop" {
        return Some("https://github.com/cvazac/eslint-plugin-react-perf/blob/main/docs/rules/jsx-no-new-object-as-prop.md".into());
    }
    match id {
        "ESLint9_react_prop-types"
        | "ESLint9_react_jsx-sort-props"
        | "ESLint9_react_react-in-jsx-scope"
        | "ESLint9_react_jsx-newline"
        | "ESLint9_react_forbid-component-props"
        | "ESLint9_react_jsx-indent"
        | "ESLint9_react_jsx-indent-props" => Some(format!(
            "https://github.com/jsx-eslint/eslint-plugin-react/blob/master/docs/rules/{}.md",
            id.strip_prefix("ESLint9_react_").unwrap()
        )),
        "ESLint9_@stylistic_jsx-sort-props"
        | "ESLint9_@stylistic_jsx-newline"
        | "ESLint9_@stylistic_jsx-indent"
        | "ESLint9_@stylistic_jsx-indent-props" => Some(format!(
            "https://v4.eslint.style/rules/jsx/{}",
            id.strip_prefix("ESLint9_@stylistic_").unwrap()
        )),
        _ => None,
    }
}
impl Comparison {
    pub fn markdown_tool(&self, id: &str) -> Result<String, String> {
        self.validate_tool(id)?;
        let mut out = String::new();
        writeln!(
            out,
            "# MAYHEM — remaining native findings\n\n**{}**  \nNative toolId: `{}`\n",
            cell(tool_label(id)),
            cell(id)
        )
        .unwrap();
        writeln!(
            out,
            "Candidate report: `{}`  \nCaller-supplied candidate SHA: `{}`\n",
            cell(self.candidate.path.display().to_string()),
            self.candidate.sha.as_deref().unwrap_or("Unavailable")
        )
        .unwrap();
        let before = self
            .baseline
            .tools()
            .iter()
            .find(|t| t["toolId"] == id)
            .unwrap_or(&Value::Null);
        let after = self
            .candidate
            .tools()
            .iter()
            .find(|t| t["toolId"] == id)
            .unwrap_or(&Value::Null);
        writeln!(out,"Native issueCount: {} → {}; errorCount: {} → {}; filesAnalyzed: {} → {}; status: {} → {}.\n",n(number(before,"issueCount")),n(number(after,"issueCount")),n(number(before,"errorCount")),n(number(after,"errorCount")),n(number(before,"filesAnalyzed")),n(number(after,"filesAnalyzed")),field(before,"status"),field(after,"status")).unwrap();
        let a = rules(&self.baseline);
        let b = rules(&self.candidate);
        let mut remaining: Vec<_> = b
            .iter()
            .filter(|((tool, _), count)| tool == id && **count > 0)
            .collect();
        remaining.sort_by(|(ak, av), (bk, bv)| bv.cmp(av).then(ak.cmp(bk)));
        writeln!(out,"Showing {} of {} remaining native patternId groups, sorted by candidate issue-record count (descending). Unchanged rules remain visible; these are not independent bug counts.\n\n| Meaning | Native patternId | Candidate records | Baseline records | Δ |\n|---|---|---:|---:|---:|",remaining.len().min(10),remaining.len()).unwrap();
        for (key, count) in remaining.iter().take(10) {
            let prior = a.get(*key).copied().unwrap_or(0);
            let label = cell(rule_label(&key.1));
            let meaning = rule_doc(&key.1)
                .map(|url| format!("[{label}]({url})"))
                .unwrap_or(label);
            writeln!(
                out,
                "| {} | `{}` | {} | {} | {:+} |",
                meaning,
                cell(&key.1),
                count,
                prior,
                **count - prior
            )
            .unwrap();
        }
        writeln!(out,"\nFriendly labels are display-only; native toolId, patternId and counts are unchanged. Unknown IDs fall back to their raw name.\n").unwrap();
        if id == "ESLint9" {
            writeln!(out,"- Unknown names: review declarations **and lint scope/configuration**; shared/browser globals can cause findings without proving a runtime bug.\n- React scope: review JSX runtime configuration; automatic runtimes may not require a React variable in scope.\n- Dynamic keys: review key provenance; a finding alone does not prove injection.\n- Property order / JSX spacing: react and @stylistic may report the same location separately. Their native records remain separate, not silently deduplicated or called separate bugs.\n- Inline objects is a performance heuristic; restricted props is a policy check. Neither is automatically a runtime failure.\n").unwrap();
        }
        writeln!(out,"Primary report errors remain {} → {}. {} No audits, agents, models or inference were run by this read-only view.",self.baseline.errors(),self.candidate.errors(),agent_note(&self.baseline,&self.candidate)).unwrap();
        Ok(out)
    }
}
