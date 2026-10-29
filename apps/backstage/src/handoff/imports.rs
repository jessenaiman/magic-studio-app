//! Attach supplied review/check snapshots only under the existing PR-head provenance rules.
use super::{Inputs, Task, json};
use crate::data::clean;
use serde_json::Value;
use std::path::Path;

pub(super) fn attach(tasks: &mut [Task], input: &Inputs, errors: &mut Vec<String>) -> String {
    let provenance = read_provenance(input, errors);
    let mut retrieved = Vec::new();
    for task in tasks {
        attach_auxiliary(task, input, &provenance);
        attach_retrieval(task, input, &provenance, &mut retrieved);
    }
    retrieved.sort();
    retrieved.dedup();
    if retrieved.is_empty() {
        "Unknown (no matching retrieval provenance supplied)".into()
    } else {
        retrieved.join(" .. ")
    }
}
fn read_provenance(input: &Inputs, errors: &mut Vec<String>) -> Value {
    let mut records = Vec::new();
    for path in &input.provenance {
        match json(path) {
            Ok(value) => match value["records"]
                .as_array()
                .filter(|records| records.len() <= 100)
            {
                Some(values) => records.extend(values.iter().cloned()),
                None => errors.push("Invalid retrieval provenance records".into()),
            },
            Err(error) => errors.push(error),
        }
    }
    serde_json::json!({"records":records})
}
fn attach_auxiliary(task: &mut Task, input: &Inputs, provenance: &Value) {
    let pr_number = task.pr_number;
    for (_, path) in input
        .reviews
        .iter()
        .filter(|(number, _)| *number == pr_number)
    {
        match json(path) {
            Ok(evidence) => attach_reviews(task, path, &evidence, provenance),
            Err(error) => task.warnings.push(error),
        }
    }
    for (_, path) in input
        .checks
        .iter()
        .filter(|(number, _)| *number == pr_number)
    {
        match json(path) {
            Ok(evidence) => attach_checks(task, &evidence),
            Err(error) => task.warnings.push(error),
        }
    }
}
fn attach_reviews(task: &mut Task, path: &Path, evidence: &Value, provenance: &Value) {
    if provenance_record(provenance, path, task.head_sha.as_deref()).is_none() {
        task.warnings
            .push("Review snapshot has no matching PR-head provenance; not attached".into());
        return;
    }
    let Some(records) = evidence.as_array() else {
        task.warnings
            .push("Reviews input is not a native JSON array".into());
        return;
    };
    task.review_note = format!(
        "{} submitted GitHub review records (snapshot)",
        records.len()
    );
    for review in records.iter().rev().take(32) {
        if let Some(body) = review["body"]
            .as_str()
            .filter(|body| !body.trim().is_empty())
        {
            task.feedback.push(clean(body));
        }
    }
}
fn attach_checks(task: &mut Task, evidence: &Value) {
    let Some(records) = evidence["check_runs"].as_array() else {
        task.warnings
            .push("Checks input lacks native check_runs array".into());
        return;
    };
    let records: Vec<_> = records
        .iter()
        .filter(|check| check["head_sha"].as_str() == task.head_sha.as_deref())
        .collect();
    if records.is_empty() {
        task.warnings
            .push("No supplied check run matches the PR head".into());
    }
    task.ci = records
        .iter()
        .map(|check| {
            format!(
                "{}: {} / {}",
                check["name"].as_str().unwrap_or("Check"),
                check["status"].as_str().unwrap_or("Unknown"),
                check["conclusion"].as_str().unwrap_or("Unknown")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
}
fn attach_retrieval(
    task: &mut Task,
    input: &Inputs,
    provenance: &Value,
    retrieved: &mut Vec<String>,
) {
    for path in &input.files {
        if let Some(record) = provenance_record(provenance, path, task.head_sha.as_deref())
            && let Some(time) = record["retrieved_at_utc"].as_str()
        {
            retrieved.push(clean(time));
            task.source = format!(
                "imported GitHub snapshot • retrieved {} • {} • R rereads file only",
                clean(time),
                clean(&path.display().to_string())
            );
        }
    }
}
fn provenance_record<'a>(value: &'a Value, path: &Path, sha: Option<&str>) -> Option<&'a Value> {
    let sha = sha?;
    let path = path.canonicalize().ok()?;
    value["records"]
        .as_array()?
        .iter()
        .filter(|record| {
            record["head_sha"]
                .as_str()
                .or_else(|| record["head"].as_str())
                == Some(sha)
                && record["path"]
                    .as_str()
                    .and_then(|p| Path::new(p).canonicalize().ok())
                    .as_ref()
                    == Some(&path)
        })
        .max_by_key(|record| record["retrieved_at_utc"].as_str().unwrap_or(""))
}
