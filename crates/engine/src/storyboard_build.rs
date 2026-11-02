//! Build the editable Magic Studio storyboard from its native DeckCraft template.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::storyboard_layout::{
    Deck, SCENE_ART_REGION, SIZE, dump, fit_rect, generate, inspect_pages, invalid, norm, note,
    panel_region, sha,
};
use crate::{EngineError, Result};

pub(crate) struct BuildStoryboardArgs<'a> {
    pub transcript: &'a Path,
    pub source: Option<&'a Path>,
    pub out: &'a Path,
    pub title: &'a str,
    pub scene_map: &'a Path,
    pub allow_timing_overrun: bool,
    pub panels: Option<&'a str>,
    pub template: &'a Path,
}

fn line_lyric(line: &Value) -> String {
    for key in ["lyric", "text"] {
        if let Some(s) = line
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return s.to_owned();
        }
    }
    String::new()
}

fn canonical_scenes(scene_map: &Path) -> Result<Vec<Value>> {
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(scene_map)?)
        .map_err(|e| invalid(e.to_string()))?;
    let mut scenes = Vec::new();
    let mut previous = 0.0;
    let mut targets = BTreeSet::new();
    for packet in doc["scenes"].as_array().into_iter().flatten() {
        let start = packet["start"].as_f64().unwrap_or(f64::NAN);
        let end = packet["end"].as_f64().unwrap_or(f64::NAN);
        if !(start.is_finite()
            && end.is_finite()
            && 0.0 <= start
            && start < end
            && start >= previous - 0.001)
        {
            return Err(invalid(
                "Canonical scenes need finite ordered non-overlapping recording times",
            ));
        }
        let target = packet["target"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("Canonical scene missing target"))?;
        if !targets.insert(target.to_owned()) {
            return Err(invalid(format!(
                "Duplicate canonical scene target: {target}"
            )));
        }
        previous = end;
        let lyric_text = packet["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .map(line_lyric)
            .collect::<Vec<_>>()
            .join(" ");
        let mut item = packet.clone();
        item["id"] = json!(target);
        item["start_seconds"] = json!(start);
        item["end_seconds"] = json!(end);
        item["lyric_text"] = json!(lyric_text);
        scenes.push(item);
    }
    if scenes.is_empty() {
        return Err(invalid("No canonical scenes; run storyboard.init first"));
    }
    Ok(scenes)
}

const SCENE_FIELD_ALIASES: &[(&str, &[&str], &str)] = &[
    (
        "reference_prompt",
        &["reference_prompt", "image_prompt"],
        "image_prompt",
    ),
    (
        "animation_prompt",
        &["animation_prompt"],
        "animation_prompt",
    ),
    (
        "layer_stack",
        &["layer_stack", "continuity", "linked_assets"],
        "continuity",
    ),
    ("action", &["action", "action_staging", "staging"], "action"),
    ("animated_text", &["animated_text"], "animated_text"),
    (
        "drawing_cues",
        &["drawing_cues", "drawing", "teaching_cues"],
        "drawing",
    ),
];

const SCENE_NOTES_ONLY: &[&str] = &[
    "skills",
    "build_steps",
    "acceptance_checks",
    "plan_vs_actual",
    "visual_review",
];

fn normalize_key(label: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in label.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    out.trim_matches('_').to_owned()
}

fn resolve_scene_field(
    packet: &Value,
    normalized: &BTreeMap<String, (String, String)>,
    field: &str,
    names: &[&str],
    used: &mut BTreeSet<String>,
) -> (String, String) {
    if let Some((label, text)) = names
        .iter()
        .filter_map(|n| normalized.get(*n))
        .find(|(_, t)| !t.is_empty())
    {
        used.insert(label.clone());
        return (label.clone(), text.clone());
    }
    if field == "action" {
        let plans: Vec<&str> = packet["notes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|n| n.as_str()?.strip_prefix("Scene plan: "))
            .collect();
        return ("Notes / Scene plan".to_owned(), plans.join("\n"));
    }
    (field.to_owned(), String::new())
}

fn record_mapped_scene_field(
    report: &mut Value,
    scene_id: &str,
    field: &str,
    role: &str,
    label: &str,
    text: &str,
    sections: &mut serde_json::Map<String, Value>,
) {
    if text.is_empty() {
        if let Some(arr) = report["missing"].as_array_mut() {
            arr.push(json!({"scene": scene_id, "field": role}));
        }
        return;
    }
    sections.insert(field.to_owned(), json!(text));
    if let Some(arr) = report["mapped"].as_array_mut() {
        arr.push(json!({"scene": scene_id, "source": label, "target": role}));
    }
    let char_count = text.chars().count();
    if char_count > 220
        && let Some(arr) = report["fit_review"].as_array_mut()
    {
        arr.push(json!({
            "scene": scene_id,
            "field": role,
            "characters": char_count,
            "reason": "Full source wording retained; review native shrink-to-fit at viewing size",
        }));
    }
}

fn record_remaining_scene_notes(
    report: &mut Value,
    scene_id: &str,
    source: &serde_json::Map<String, Value>,
    used: &BTreeSet<String>,
) {
    for (label, val) in source {
        if used.contains(label) {
            continue;
        }
        let text = val.as_str().unwrap_or("");
        let key = normalize_key(label);
        if SCENE_NOTES_ONLY.contains(&key.as_str()) {
            if let Some(arr) = report["retained_in_slide_notes"].as_array_mut() {
                arr.push(json!({
                    "scene": scene_id,
                    "source": label,
                    "characters": text.chars().count(),
                }));
            }
        } else if let Some(arr) = report["unmapped"].as_array_mut() {
            arr.push(json!({"scene": scene_id, "source": label, "text": text}));
        }
    }
}

fn normalize_scene_content(packets: &[Value], expected_fields: &[String]) -> (Value, Value) {
    let mut report = json!({
        "expected_fields": expected_fields,
        "mapped": [],
        "missing": [],
        "unmapped": [],
        "retained_in_slide_notes": [],
        "fit_review": [],
    });
    let mut content = serde_json::Map::new();
    for packet in packets {
        let scene_id = packet["id"].as_str().unwrap_or("");
        let empty_map = serde_json::Map::new();
        let source = packet["sections"].as_object().unwrap_or(&empty_map);
        let mut sections = serde_json::Map::new();
        let mut used = BTreeSet::new();
        let normalized: BTreeMap<String, (String, String)> = source
            .iter()
            .map(|(k, v)| {
                (
                    normalize_key(k),
                    (k.clone(), v.as_str().unwrap_or("").to_owned()),
                )
            })
            .collect();
        for (field, names, target) in SCENE_FIELD_ALIASES {
            let role = format!("user.scene.{target}");
            if !expected_fields.contains(&role) {
                continue;
            }
            let (label, text) = resolve_scene_field(packet, &normalized, field, names, &mut used);
            record_mapped_scene_field(
                &mut report,
                scene_id,
                field,
                &role,
                &label,
                &text,
                &mut sections,
            );
        }
        record_remaining_scene_notes(&mut report, scene_id, source, &used);
        content.insert(
            scene_id.to_owned(),
            json!({
                "sections": sections,
                "notes": packet.get("notes").cloned().unwrap_or_else(|| json!([])),
                "source": packet.get("source").cloned().unwrap_or(Value::Null),
                "source_sections": source,
                "lines": packet.get("lines").cloned().unwrap_or_else(|| json!([])),
                "title": packet.get("title").cloned().unwrap_or(Value::Null),
            }),
        );
    }
    let has_unmapped = report["unmapped"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty());
    report["needs_correction"] = json!(has_unmapped);
    (Value::Object(content), report)
}

fn template_schema(template: &Path, out: &Path) -> Result<Value> {
    let parent = template
        .parent()
        .ok_or_else(|| invalid("Template has no parent directory"))?;
    let mut d = Deck::new(parent, out)?;
    d.cmd("file.open", json!({"path": template}))?;
    let mut schema = serde_json::Map::new();
    for sl in inspect_pages(&mut d)? {
        let Some(kind) = note(&sl)["kind"].as_str().map(str::to_owned) else {
            continue;
        };
        let fields: Vec<Value> = sl["shapes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|sh| {
                sh["altText"]
                    .as_str()
                    .is_some_and(|a| a.starts_with("user."))
            })
            .map(|sh| json!({"role": sh["altText"], "rect": sh["box"]}))
            .collect();
        schema.insert(kind, json!(fields));
    }
    Ok(Value::Object(schema))
}

fn load_fish_linked_lyrics(line_source: &Path) -> BTreeMap<String, Value> {
    let Ok(raw) = std::fs::read_to_string(line_source) else {
        return BTreeMap::new();
    };
    let trimmed = raw.trim();
    if !trimmed.starts_with("window.FISH_DATA = ") {
        return BTreeMap::new();
    }
    let Some((_, rhs)) = trimmed.split_once('=') else {
        return BTreeMap::new();
    };
    let json_text = rhs.trim().trim_end_matches(';');
    let Ok(payload) = serde_json::from_str::<Value>(json_text) else {
        return BTreeMap::new();
    };
    payload["lyrics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l["id"].as_str()?.to_owned(), l.clone())))
        .collect()
}

fn packet_lyric_sections(original: &Value, sections: &BTreeMap<String, Value>) -> Vec<Value> {
    original["script_section_id"]
        .as_str()
        .unwrap_or("")
        .split(',')
        .filter_map(|k| sections.get(k))
        .filter(|s| {
            s.get("lyrics").and_then(Value::as_bool).unwrap_or(true)
                && s["text"].as_str() != Some("[instrumental]")
        })
        .cloned()
        .collect()
}

fn tag_packet_section_label(packet: &mut Value, first_lyric: &Value) {
    let label = first_lyric["label"]
        .as_str()
        .unwrap_or("")
        .split(" line")
        .next()
        .unwrap_or("");
    let lower = label.to_lowercase();
    if lower.starts_with("chorus") {
        let id = first_lyric["id"].as_str().unwrap_or("");
        let digits: String = id
            .strip_prefix("ch")
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        packet["kind"] = json!("chorus");
        packet["section_label"] = json!(if digits.is_empty() {
            "Chorus".to_owned()
        } else {
            format!("Chorus {digits}")
        });
    } else if lower.starts_with("verse") {
        packet["kind"] = json!("verse");
        packet["section_label"] = json!(label);
    }
}

fn find_line_onset_hits(
    words: &[Value],
    tokens: &[String],
    expected: &[String],
    prev_start: f64,
    curr_end: f64,
) -> Vec<usize> {
    if expected.is_empty() || words.len() < expected.len() {
        return Vec::new();
    }
    (0..=words.len() - expected.len())
        .filter(|&n| {
            let w_start = words[n]["start"].as_f64().unwrap_or(-1.0);
            tokens[n..n + expected.len()] == *expected
                && prev_start <= w_start
                && w_start < curr_end
        })
        .collect()
}

struct AlignContext<'a> {
    sections: BTreeMap<String, Value>,
    linked: BTreeMap<String, Value>,
    line_source: PathBuf,
    script: PathBuf,
    words: &'a [Value],
    tokens: Vec<String>,
}

fn record_alignment_evidence(
    evidence: Option<&mut Vec<Value>>,
    scene_id: &Value,
    source_line: &Value,
    hit_count: usize,
) {
    let Some(ev) = evidence else {
        return;
    };
    let reason = if hit_count == 1 {
        "Unique exact native-word match".to_owned()
    } else {
        format!("Expected one native-word match; found {hit_count}")
    };
    ev.push(json!({
        "scene_id": scene_id,
        "status": if hit_count == 1 { "assessed" } else { "unassessed" },
        "source_line": source_line,
        "reason": reason,
    }));
}

fn build_packet_source_lines(lyrics: &[Value], ctx: &AlignContext<'_>) -> Vec<Value> {
    lyrics
        .iter()
        .map(|l| {
            let id = l["id"].as_str().unwrap_or("");
            let chosen = ctx.linked.get(id).unwrap_or(l);
            let src = if ctx.linked.contains_key(id) {
                &ctx.line_source
            } else {
                &ctx.script
            };
            json!({"id": id, "text": chosen["text"], "source": src})
        })
        .collect()
}

fn align_single_packet(
    i: usize,
    packets: &mut [Value],
    original: &Value,
    ctx: &AlignContext<'_>,
    mut evidence: Option<&mut Vec<Value>>,
) -> Option<Value> {
    let lyrics = packet_lyric_sections(original, &ctx.sections);
    if lyrics.is_empty() {
        if let Some(ev) = evidence.as_deref_mut()
            && i > 0
        {
            ev.push(json!({
                "scene_id": packets[i]["id"],
                "status": "unassessed",
                "reason": "No supplied lyric section opening",
            }));
        }
        return None;
    }
    packets[i]["source_lines"] = json!(build_packet_source_lines(&lyrics, ctx));
    tag_packet_section_label(&mut packets[i], &lyrics[0]);
    if i == 0 {
        return None;
    }
    let expected: Vec<String> = lyrics[0]["text"]
        .as_str()
        .unwrap_or("")
        .split_whitespace()
        .map(norm)
        .collect();
    let prev_start = packets[i - 1]["start_seconds"].as_f64().unwrap_or(0.0);
    let curr_end = packets[i]["end_seconds"].as_f64().unwrap_or(0.0);
    let hits = find_line_onset_hits(ctx.words, &ctx.tokens, &expected, prev_start, curr_end);
    record_alignment_evidence(evidence, &packets[i]["id"], &lyrics[0]["id"], hits.len());
    let &[at] = hits.as_slice() else {
        return None;
    };
    let onset = ctx.words[at]["start"].as_f64().unwrap_or(0.0);
    let cut = packets[i]["start_seconds"].as_f64().unwrap_or(0.0);
    if onset >= cut {
        return None;
    }
    packets[i - 1]["end_seconds"] = json!(onset);
    packets[i]["start_seconds"] = json!(onset);
    Some(json!({
        "scene": packets[i]["id"],
        "previous_scene": packets[i - 1]["id"],
        "old_boundary": cut,
        "boundary": onset,
        "word": ctx.words[at]["id"],
        "source_line": lyrics[0]["id"],
        "convention": "next source line onset; half-open intervals",
    }))
}

fn align_imported_sections(
    packets: &mut [Value],
    words: &[Value],
    source: Option<&Path>,
    mut evidence: Option<&mut Vec<Value>>,
) -> Vec<Value> {
    let Some(root) = source else {
        return Vec::new();
    };
    let plan = root.join("artifacts/scene_plan.json");
    let script = root.join("artifacts/script.json");
    if !plan.is_file() || !script.is_file() {
        return Vec::new();
    }
    let Ok(plan_doc) = std::fs::read_to_string(&plan)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .ok_or(())
    else {
        return Vec::new();
    };
    let scenes = plan_doc["scenes"].as_array().map_or(&[][..], Vec::as_slice);
    if scenes.len() != packets.len() {
        if let Some(ev) = evidence {
            ev.push(json!({
                "status": "unassessed",
                "reason": "Supplied scene plan and storyboard scene counts differ",
            }));
        }
        return Vec::new();
    }
    let script_doc = std::fs::read_to_string(&script)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or_else(|| json!({}));
    let sections: BTreeMap<String, Value> = script_doc["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["id"].as_str()?.to_owned(), s.clone())))
        .collect();
    let line_source = root.join("hyperframes/film-data.js");
    let ctx = AlignContext {
        sections,
        linked: load_fish_linked_lyrics(&line_source),
        line_source,
        script,
        words,
        tokens: words
            .iter()
            .map(|w| norm(w["text"].as_str().unwrap_or("")))
            .collect(),
    };
    let mut changes = Vec::new();
    for (i, original) in scenes.iter().enumerate() {
        if let Some(change) =
            align_single_packet(i, packets, original, &ctx, evidence.as_deref_mut())
        {
            changes.push(change);
        }
    }
    changes
}

fn source_line_breaks(words: &[Value], lines: &[Value]) -> Vec<Value> {
    let tokens: Vec<String> = words
        .iter()
        .map(|w| {
            w["text"]
                .as_str()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .map(norm)
                .unwrap_or_default()
        })
        .collect();
    let mut mapped = Vec::new();
    let mut used = Vec::new();
    for line in lines {
        let expected: Vec<String> = line["text"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .map(norm)
            .collect();
        if expected.is_empty() || words.len() < expected.len() {
            return Vec::new();
        }
        let hits: Vec<usize> = (0..=words.len() - expected.len())
            .filter(|&i| tokens[i..i + expected.len()] == expected)
            .collect();
        if hits.len() != 1 {
            return Vec::new();
        }
        let selected = &words[hits[0]..hits[0] + expected.len()];
        used.extend(selected.iter().map(|w| w["id"].clone()));
        mapped.push(json!({
            "id": line["id"],
            "source": line["source"],
            "text": selected.iter().filter_map(|w| w["text"].as_str()).collect::<Vec<_>>().join(" "),
            "word_ids": selected.iter().map(|w| w["id"].clone()).collect::<Vec<_>>(),
            "start": selected.first().map(|w| w["start"].clone()).unwrap_or(Value::Null),
            "end": selected.last().map(|w| w["end"].clone()).unwrap_or(Value::Null),
        }));
    }
    let all_ids: Vec<Value> = words.iter().map(|w| w["id"].clone()).collect();
    if used == all_ids { mapped } else { Vec::new() }
}

fn chorus_phrases(words: &[Value]) -> Result<Vec<Value>> {
    let tokens: Vec<String> = words
        .iter()
        .map(|w| norm(w["text"].as_str().unwrap_or("")))
        .collect();
    let mut candidates = Vec::new();
    if words.len() >= 5 {
        for repeat in 2..words.len() - 2 {
            let mut prefix = 0;
            while prefix < repeat
                && repeat + prefix < words.len()
                && tokens[prefix] == tokens[repeat + prefix]
            {
                prefix += 1;
            }
            if prefix >= 2 && prefix < repeat && repeat + prefix < words.len() {
                candidates.push((prefix, repeat));
            }
        }
    }
    let Some(&(prefix, repeat)) = candidates.iter().max() else {
        return Err(invalid(
            "Four chorus panels require an unambiguous repeated opening in the native words",
        ));
    };
    let cuts = [0, prefix, repeat, repeat + prefix, words.len()];
    Ok(cuts
        .windows(2)
        .map(|w| {
            let slice = &words[w[0]..w[1]];
            json!({
                "text": slice.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join(" "),
                "word_ids": slice.iter().map(|x| x["id"].clone()).collect::<Vec<_>>(),
                "start": slice.first().map(|x| x["start"].clone()).unwrap_or(Value::Null),
                "end": slice.last().map(|x| x["end"].clone()).unwrap_or(Value::Null),
            })
        })
        .collect())
}

fn build_doctor_task(
    fix: &Value,
    pages: &[(usize, Value)],
    words: &[Value],
    script_path: &Path,
    source_transcript: &Value,
) -> Value {
    let prev_scene = fix["previous_scene"].as_str().unwrap_or("");
    let next_scene = fix["scene"].as_str().unwrap_or("");
    let page = pages
        .iter()
        .find(|(_, g)| g["id"].as_str() == Some(prev_scene))
        .map(|(i, _)| *i);
    let boundary = fix["boundary"].as_f64().unwrap_or(0.0);
    let old_boundary = fix["old_boundary"].as_f64().unwrap_or(0.0);
    let affected: Vec<&Value> = words
        .iter()
        .filter(|w| {
            let s = w["start"].as_f64().unwrap_or(-1.0);
            boundary <= s && s < old_boundary
        })
        .collect();
    let source_range = match (affected.first(), affected.last()) {
        (Some(first), Some(last)) => json!([first["start"], last["end"]]),
        _ => json!([]),
    };
    let page_display = page.map_or_else(|| "None".to_owned(), |p| p.to_string());
    json!({
        "id": format!("section-boundary:{prev_scene}:{next_scene}"),
        "priority": 1,
        "scene_id": prev_scene,
        "page_id": page,
        "next_scene_id": next_scene,
        "suspected_flaw": "Scene includes the opening of the next supplied section",
        "source_word_ids": affected.iter().map(|w| w["id"].clone()).collect::<Vec<_>>(),
        "affected_lyrics": affected.iter().filter_map(|w| w["text"].as_str()).collect::<Vec<_>>().join(" "),
        "source_range": source_range,
        "expected": {"boundary": fix["boundary"], "convention": fix["convention"]},
        "actual": {"boundary": fix["old_boundary"]},
        "evidence": {
            "script": script_path,
            "section_line": fix["source_line"],
            "native_transcript": source_transcript,
            "onset_word": fix["word"],
        },
        "confidence": "Exact native-word match to a supplied section line; not a musical inference",
        "proposed_fix": format!("Review {prev_scene} / page {page_display}, then set its end and {next_scene} start to {}s; preserve every native word/timestamp", fix["boundary"]),
    })
}

fn doctor_health_and_status(result: &Value) -> (Value, &'static str) {
    if result["tasks"].as_array().is_some_and(|t| !t.is_empty()) {
        return (json!(false), "issues_found");
    }
    let has_unassessed = result["unassessed"]
        .as_array()
        .is_some_and(|u| !u.is_empty());
    let no_assessed = result["assessed"].as_array().is_none_or(Vec::is_empty);
    if has_unassessed || no_assessed {
        return (Value::Null, "insufficient_source_evidence");
    }
    (json!(true), "no_supported_issues")
}

pub(crate) fn doctor(manifest: &Path, source: &Path) -> Result<Value> {
    let board: Value = serde_json::from_str(&std::fs::read_to_string(manifest)?)
        .map_err(|e| invalid(e.to_string()))?;
    let mut result = json!({
        "ok": true,
        "project": board.get("project").cloned().unwrap_or(Value::Null),
        "storyboard": manifest,
        "healthy": Value::Null,
        "status": "insufficient_source_evidence",
        "tasks": [],
        "assessed": [],
        "unassessed": [],
        "scope_assessed": "Scene cuts crossing an exact supplied section opening line",
        "scope_unassessed": [
            "Musical structure without supplied section labels",
            "Timing accuracy in the recording",
            "Rendered typography, artwork, template and animation quality",
        ],
        "diagnostic_only": true,
    });
    let needed = [
        source.join("artifacts/scene_plan.json"),
        source.join("artifacts/script.json"),
    ];
    let missing: Vec<String> = needed
        .iter()
        .filter(|p| !p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !missing.is_empty() {
        result["unassessed"] = json!([{
            "reason": "No supplied section plan/script",
            "missing": missing,
        }]);
        return Ok(result);
    }
    let words = crate::storyboard_layout::load_words(&json!({
        "transcript": board["source_transcript"]
    }))?;
    let pages: Vec<(usize, Value)> = board["page_plan"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, p)| {
            let g = p.get("group")?.clone();
            g.is_object().then_some((i + 1, g))
        })
        .collect();
    let mut packets: Vec<Value> = pages
        .iter()
        .map(|(_, g)| {
            json!({
                "id": g["id"],
                "start_seconds": g["span_start"],
                "end_seconds": g["span_end"],
            })
        })
        .collect();
    let mut evidence = Vec::new();
    let proposals =
        align_imported_sections(&mut packets, &words, Some(source), Some(&mut evidence));
    result["assessed"] = json!(
        evidence
            .iter()
            .filter(|e| e["status"] == "assessed")
            .cloned()
            .collect::<Vec<_>>()
    );
    result["unassessed"] = json!(
        evidence
            .iter()
            .filter(|e| e["status"] != "assessed")
            .cloned()
            .collect::<Vec<_>>()
    );
    result["tasks"] = json!(
        proposals
            .iter()
            .map(|fix| build_doctor_task(
                fix,
                &pages,
                &words,
                &needed[1],
                &board["source_transcript"]
            ))
            .collect::<Vec<_>>()
    );
    let (healthy, status) = doctor_health_and_status(&result);
    result["healthy"] = healthy;
    result["status"] = json!(status);
    Ok(result)
}

const TEMPLATE_PLACEHOLDERS: &[&str] = &[
    "Describe the story, characters",
    "Name this location and place",
    "Name the cast and reusable props",
    "Register existing artwork",
    "Place the scene reference",
    "See the four panels",
    "Describe the visible action",
    "Link cast, props and locations",
    "Describe the reference image",
    "Describe movement, timing",
    "Text, word IDs, reveal timing",
    "What is drawn, when it appears",
];

fn is_authored_text(val: Option<&str>) -> bool {
    let Some(s) = val.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    !TEMPLATE_PLACEHOLDERS.iter().any(|p| s.starts_with(p))
        && !s.contains("\nRegister existing artwork")
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp"];

fn is_image_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn title_case_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_words_inventory(source_root: &Path) -> Vec<(String, String)> {
    let words_path = source_root.join("design/words.json");
    let Ok(text) = std::fs::read_to_string(words_path) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let mut lyric_bgs: Vec<(String, String)> = Vec::new();
    for anim in doc["animations"].as_array().into_iter().flatten() {
        let Some(bg) = anim["background_asset"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        let anim_id = anim["animation_id"].as_str().unwrap_or("");
        let sec = anim["scope"]["section"].as_str().unwrap_or(anim_id);
        let label = format!("{anim_id} ({sec})");
        if let Some((_, existing)) = lyric_bgs.iter_mut().find(|(k, _)| k == bg) {
            *existing = label;
        } else {
            lyric_bgs.push((bg.to_owned(), label));
        }
    }
    lyric_bgs
}

fn sorted_dir_entries(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|v| v.path())).collect();
    entries.sort();
    entries
}

fn scan_background_files(bg_root: &Path) -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    for entry in sorted_dir_entries(bg_root) {
        if entry.is_dir() {
            let Some(name) = entry.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(img) = sorted_dir_entries(&entry)
                .into_iter()
                .find(|p| is_image_file(p))
            {
                found.push((name.to_owned(), img));
            }
        } else if is_image_file(&entry)
            && let Some(stem) = entry.file_stem().and_then(|s| s.to_str())
        {
            found.push((stem.to_owned(), entry));
        }
    }
    found
}

fn discover_locations(source_root: &Path, lyric_bgs: &[(String, String)]) -> Vec<Value> {
    let found = scan_background_files(&source_root.join("assets/backgrounds"));
    let found_map: BTreeMap<String, PathBuf> = found.iter().cloned().collect();
    let mut ordered: Vec<String> = lyric_bgs
        .iter()
        .filter(|(k, _)| found_map.contains_key(k))
        .map(|(k, _)| k.clone())
        .collect();
    for (k, _) in &found {
        if !ordered.contains(k) {
            ordered.push(k.clone());
        }
    }
    ordered
        .into_iter()
        .take(4)
        .filter_map(|bg_id| {
            let img = found_map.get(&bg_id)?;
            let resolved = img.canonicalize().unwrap_or_else(|_| img.clone());
            let file_name = resolved.file_name()?.to_str()?;
            let note = lyric_bgs
                .iter()
                .find(|(k, _)| k == &bg_id)
                .map(|(_, v)| format!("Mapped lyric backdrop for {v}"))
                .unwrap_or_else(|| format!("Storyboard background plate ({bg_id})"));
            Some(json!({
                "id": bg_id,
                "name": title_case_slug(&bg_id),
                "file": resolved,
                "notes": format!("{note}\n{file_name}"),
                "notes_exact": true,
            }))
        })
        .collect()
}

fn first_character_image(entry: &Path, slug: &str) -> Option<PathBuf> {
    let assembled = entry.join(format!("{slug}-assembled.png"));
    if assembled.is_file() {
        return Some(assembled);
    }
    if let Some(img) = sorted_dir_entries(entry)
        .into_iter()
        .find(|p| is_image_file(p))
    {
        return Some(img);
    }
    sorted_dir_entries(&entry.join("parts"))
        .into_iter()
        .find(|p| is_image_file(p))
}

fn discover_assets(source_root: &Path) -> Vec<Value> {
    let mut assets = Vec::new();
    for entry in sorted_dir_entries(&source_root.join("assets/characters")) {
        let found = if entry.is_dir() {
            entry
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|slug| first_character_image(&entry, slug).map(|p| (slug.to_owned(), p)))
        } else if is_image_file(&entry) {
            entry
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|stem| (stem.to_owned(), entry.clone()))
        } else {
            None
        };
        let Some((id, img)) = found else {
            continue;
        };
        let resolved = img.canonicalize().unwrap_or(img);
        let fname = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assets.push(json!({
            "id": id,
            "name": title_case_slug(&id),
            "file": resolved,
            "notes": format!("Character asset ({id})\n{fname}"),
            "notes_exact": true,
        }));
    }
    for entry in sorted_dir_entries(&source_root.join("assets/references")) {
        if !is_image_file(&entry) {
            continue;
        }
        let Some(stem) = entry
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let id = stem.strip_suffix("-v1").unwrap_or(&stem);
        let resolved = entry.canonicalize().unwrap_or(entry);
        let fname = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("");
        assets.push(json!({
            "id": id,
            "name": title_case_slug(id),
            "file": resolved,
            "notes": format!("Reference sheet ({id})\n{fname}"),
            "notes_exact": true,
        }));
    }
    assets.truncate(4);
    assets
}

fn upgrade_assembled_character_cards(source_root: &Path, assets: &mut [Value]) {
    for item in assets {
        let Some(id) = item["id"].as_str() else {
            continue;
        };
        let candidate = source_root
            .join("assets/characters")
            .join(id)
            .join(format!("{id}-assembled.png"));
        if candidate.is_file()
            && let Ok(resolved) = candidate.canonicalize()
        {
            item["file"] = json!(resolved);
        }
    }
}

fn discover_project_inventory(source_root: Option<&Path>, config: &mut Value) {
    let Some(root) = source_root.filter(|p| p.is_dir()) else {
        return;
    };
    let lyric_bgs = read_words_inventory(root);
    if config
        .get("locations")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty)
    {
        let locations = discover_locations(root, &lyric_bgs);
        if !locations.is_empty() {
            config["locations"] = json!(locations);
        }
    }
    if let Some(assets) = config.get_mut("assets").and_then(Value::as_array_mut)
        && !assets.is_empty()
    {
        upgrade_assembled_character_cards(root, assets);
    } else {
        let assets = discover_assets(root);
        if !assets.is_empty() {
            config["assets"] = json!(assets);
        }
    }
}

fn resolve_source_root(source: Option<&Path>, metadata: &Value) -> Option<PathBuf> {
    if let Some(s) = source {
        return Some(s.to_path_buf());
    }
    metadata["project"]
        .as_str()
        .and_then(|p| Path::new(p).parent().map(Path::to_path_buf))
}

fn build_scene_group(packet: &Value, words: &[Value], panels: Option<&str>) -> Result<Value> {
    let start_sec = packet["start_seconds"].as_f64().unwrap_or(0.0);
    let end_sec = packet["end_seconds"].as_f64().unwrap_or(0.0);
    let selected: Vec<Value> = words
        .iter()
        .filter(|w| {
            let s = w["start"].as_f64().unwrap_or(-1.0);
            start_sec - 0.001 <= s && s < end_sec - 0.001
        })
        .cloned()
        .collect();
    let source_lines = packet["source_lines"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let lyric_lines = source_line_breaks(&selected, source_lines);
    let id = packet["id"].as_str().unwrap_or("");
    let kind = packet["kind"].as_str().unwrap_or("provisional scene");
    let mut group = json!({
        "id": id,
        "word_ids": selected.iter().map(|w| w["id"].clone()).collect::<Vec<_>>(),
        "span_start": start_sec,
        "span_end": end_sec,
        "kind": kind,
        "word_assignment": "source-start timestamp; canonical scene cuts preserved",
        "status": packet["status"].as_str().unwrap_or("needs review"),
        "notes": packet.get("notes").cloned().unwrap_or_else(|| json!([])),
        "reason": "Canonical CLI scene span; review grouping without forced verse taxonomy.",
        "section_label": packet.get("section_label").cloned().unwrap_or(Value::Null),
        "lyric_lines": lyric_lines,
    });
    let has_saved_panels = packet
        .get("panels")
        .and_then(Value::as_array)
        .is_some_and(|p| !p.is_empty());
    if panels == Some(id) || has_saved_panels {
        if kind != "chorus" {
            return Err(invalid(
                "Four-line panel layout requires a source-labelled chorus",
            ));
        }
        let mut built_panels = chorus_phrases(&selected)?;
        for panel in &mut built_panels {
            let p_ids: BTreeSet<&str> = panel["word_ids"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let line_id = lyric_lines.iter().find_map(|l| {
                let l_ids: BTreeSet<&str> = l["word_ids"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                p_ids.is_subset(&l_ids).then(|| l["id"].clone())
            });
            panel["line_id"] = line_id.unwrap_or(Value::Null);
        }
        group["panels"] = json!(built_panels);
    }
    Ok(group)
}

const EDITABLE_SCENE_FIELDS: &[&str] = &[
    "action",
    "continuity",
    "image_prompt",
    "animation_prompt",
    "animated_text",
    "drawing",
];

fn field_to_section_key(field_name: &str) -> &str {
    match field_name {
        "continuity" => "layer_stack",
        "image_prompt" => "reference_prompt",
        "drawing" => "drawing_cues",
        other => other,
    }
}

fn apply_single_saved_field(scene_id: &str, f: &Value, config: &mut Value, mapping: &mut Value) {
    let role = f["role"].as_str().unwrap_or("");
    let field_name = f["field"].as_str().unwrap_or("");
    if !role.starts_with("user.scene.") || !EDITABLE_SCENE_FIELDS.contains(&field_name) {
        return;
    }
    let key = field_to_section_key(field_name);
    let existing = config["scene_content"][scene_id]["sections"][key]
        .as_str()
        .unwrap_or("");
    let value = f["value"].as_str();
    if existing.is_empty()
        && is_authored_text(value)
        && let Some(val_str) = value
    {
        config["scene_content"][scene_id]["sections"][key] = json!(val_str);
        let target_role = format!("user.scene.{field_name}");
        if let Some(missing) = mapping["missing"].as_array_mut() {
            missing.retain(|m| {
                !(m["scene"].as_str() == Some(scene_id)
                    && m["field"].as_str() == Some(&target_role))
            });
        }
    }
}

fn apply_saved_scene_text_fields(packets: &[Value], config: &mut Value, mapping: &mut Value) {
    for packet in packets {
        let scene_id = packet["id"].as_str().unwrap_or("");
        for f in packet["text_fields"].as_array().into_iter().flatten() {
            apply_single_saved_field(scene_id, f, config, mapping);
        }
    }
}

fn compute_timing_conflicts(groups: &[Value], words: &[Value]) -> Vec<Value> {
    let mut conflicts = Vec::new();
    for g in groups {
        let span_end = g["span_end"].as_f64().unwrap_or(0.0);
        let ids: BTreeSet<&str> = g["word_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        for w in words {
            let wid = w["id"].as_str().unwrap_or("");
            let w_end = w["end"].as_f64().unwrap_or(0.0);
            if ids.contains(wid) && w_end > span_end + 0.001 {
                conflicts.push(json!({
                    "scene": g["id"],
                    "word": w["id"],
                    "text": w["text"],
                    "start": w["start"],
                    "end": w["end"],
                    "scene_end": span_end,
                    "resolution": "Source timing preserved; word occurrence assigned by its original start",
                }));
            }
        }
    }
    conflicts
}

fn retained_page_content(previous: &Value, content: &[Value], has_backgrounds: bool) -> Value {
    let mut map = serde_json::Map::new();
    for (i, page) in previous["page_plan"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let kind = page["kind"].as_str().unwrap_or("");
        if kind == "scene" || (kind == "locations" && has_backgrounds) {
            continue;
        }
        let Some(key) = page["key"].as_str() else {
            continue;
        };
        let page_num = (i + 1) as u64;
        let fields: Vec<Value> = content
            .iter()
            .filter(|f| {
                f["page"].as_u64() == Some(page_num)
                    && f["role"].as_str().is_some_and(|r| r.starts_with("user."))
                    && is_authored_text(f["value"].as_str())
            })
            .cloned()
            .collect();
        map.insert(key.to_owned(), json!(fields));
    }
    Value::Object(map)
}

fn retain_inventory_sheets(previous: &Value, config: &mut Value) {
    for (kind, key) in [("assets", "assets"), ("locations", "locations")] {
        let retained: Vec<Value> = previous["page_plan"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|page| page["kind"].as_str() == Some(kind))
            .flat_map(|page| page["sheet"].as_array().into_iter().flatten())
            .filter(|record| {
                record["id"].as_str() != Some("inventory")
                    && record["file"].as_str().is_some_and(|s| !s.is_empty())
            })
            .cloned()
            .collect();
        if !retained.is_empty() {
            config[key] = json!(retained);
        }
    }
}

fn merge_location_notes_for_record(record: &mut Value, previous: &Value, content: &[Value]) {
    let rec_id = record["id"].as_str().unwrap_or("").to_owned();
    for (i, page) in previous["page_plan"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if page["kind"].as_str() != Some("locations") {
            continue;
        }
        let page_num = (i + 1) as u64;
        for (slot, old) in page["sheet"].as_array().into_iter().flatten().enumerate() {
            if old["id"].as_str() != Some(rec_id.as_str()) {
                continue;
            }
            let target_field = format!("sheet.{slot}.notes");
            if let Some(field) = content.iter().find(|f| {
                f["page"].as_u64() == Some(page_num) && f["field"].as_str() == Some(&target_field)
            }) && let Some(val) = field["value"]
                .as_str()
                .filter(|v| is_authored_text(Some(v)))
            {
                record["notes"] = json!(val);
                record["notes_exact"] = json!(true);
            }
        }
    }
}

fn restore_previous_inventory(metadata: &Value, config: &mut Value) {
    let previous = &metadata["previous_board"];
    let content = metadata["page_content"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    let has_bgs = !metadata["backgrounds"].is_null();
    config["page_content"] = retained_page_content(previous, content, has_bgs);
    retain_inventory_sheets(previous, config);
    if has_bgs && let Some(bgs) = metadata["backgrounds"].as_array() {
        let mut locs = bgs.clone();
        for record in &mut locs {
            merge_location_notes_for_record(record, previous, content);
        }
        config["locations"] = json!(locs);
    }
}

fn finalize_mapping_and_write_inputs(
    out: &Path,
    config: &mut Value,
    mapping: &mut Value,
    groups: &[Value],
    packets: &[Value],
    words: &[Value],
    metadata: &Value,
    template: &Path,
) -> Result<(PathBuf, PathBuf, f64)> {
    let max_end = words
        .iter()
        .filter_map(|w| w["end"].as_f64())
        .fold(0.0_f64, f64::max);
    let dur = metadata["duration_sec"].as_f64().unwrap_or(0.0);
    let overrun = max_end - dur;
    if overrun > 0.001 {
        let warn = format!(
            "Source word end {max_end:.6}s exceeds measured audio {dur:.6}s by {overrun:.6}s. Raw words/timestamps retained; timing is not validated."
        );
        config["timing_warning"] = json!(warn);
        mapping["timing_warning"] = json!(warn);
    }
    let has_unmapped = mapping["unmapped"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty());
    mapping["needs_correction"] = json!(has_unmapped);
    mapping["template_file"] = json!(template);
    mapping["line_mapping"] = json!(
        groups
            .iter()
            .map(|g| {
                let lines = g["lyric_lines"].as_array().map_or(&[][..], Vec::as_slice);
                json!({
                    "scene": g["id"],
                    "line_ids": lines.iter().map(|l| l["id"].clone()).collect::<Vec<_>>(),
                    "status": if !lines.is_empty() {
                        "exact source-word sequence"
                    } else {
                        "unmapped native words retained; no line IDs invented"
                    },
                })
            })
            .collect::<Vec<_>>()
    );
    mapping["import_alignment"] = json!([]);
    mapping["source_words"] = json!(words.len());
    mapping["scene_spans"] = json!(
        packets
            .iter()
            .map(|p| json!([p["start_seconds"], p["end_seconds"]]))
            .collect::<Vec<_>>()
    );
    config["mapping"] = mapping.clone();
    dump(&out.join("mapping.json"), mapping)?;
    let config_path = out.join("template-inputs.json");
    dump(&config_path, config)?;
    let grouping = out.join("scene-map-input.json");
    dump(&grouping, &json!({"groups": groups}))?;
    Ok((config_path, grouping, overrun))
}

fn render_slide_previews(out: &Path, pdf_path: &Path, page_count: usize) -> Result<Vec<String>> {
    let pages = out.join("pages");
    std::fs::create_dir_all(&pages)?;
    let mut rendered = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let out_png = pages.join(format!("slide-{:03}.png", i + 1));
        crate::workspace::run_tool_in(
            out,
            "pdfcraft-cli",
            &[
                "render".into(),
                pdf_path.to_string_lossy().into_owned(),
                "--page".into(),
                (i + 1).to_string(),
                "--dpi".into(),
                "72".into(),
                "--out".into(),
                out_png.to_string_lossy().into_owned(),
            ],
        )?;
        rendered.push(out_png.to_string_lossy().into_owned());
    }
    rendered.sort();
    Ok(rendered)
}

fn remap_scene_placements(
    packets: &[Value],
    page_plan: &[Value],
    panels: Option<&str>,
) -> Result<Vec<Value>> {
    let mut placements = Vec::new();
    for packet in packets {
        let pid = packet["id"].as_str().unwrap_or("");
        let Some(page) = page_plan
            .iter()
            .position(|p| p["group"]["id"].as_str() == Some(pid))
            .map(|i| i + 1)
        else {
            continue;
        };
        let has_saved_panels = packet
            .get("panels")
            .and_then(Value::as_array)
            .is_some_and(|p| !p.is_empty());
        for original in packet["saved_placements"].as_array().into_iter().flatten() {
            let mut item = original.clone();
            item["page"] = json!(page);
            if panels == Some(pid)
                && !has_saved_panels
                && let Some(r) = item["rect"].as_array()
                && r.len() == 4
            {
                let target = fit_rect(SCENE_ART_REGION[2], SCENE_ART_REGION[3], panel_region(3))?;
                let scale = target[2] / SCENE_ART_REGION[2];
                let rx = r[0].as_f64().unwrap_or(0.0);
                let ry = r[1].as_f64().unwrap_or(0.0);
                let rw = r[2].as_f64().unwrap_or(0.0);
                let rh = r[3].as_f64().unwrap_or(0.0);
                item["rect"] = json!([
                    target[0] + (rx - 60.0) * scale,
                    target[1] + (ry - 220.0) * scale,
                    rw * scale,
                    rh * scale,
                ]);
            }
            placements.push(item);
        }
    }
    Ok(placements)
}

fn remap_inventory_placements(
    previous: &Value,
    page_plan: &[Value],
    has_backgrounds: bool,
) -> Vec<Value> {
    let mut placements = Vec::new();
    let old_plan = previous["page_plan"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    for original in previous["design"]["placements"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let Some(old_page_num) = original["page"].as_u64().filter(|&n| n > 0) else {
            continue;
        };
        let Some(old_page) = old_plan.get((old_page_num - 1) as usize) else {
            continue;
        };
        let kind = old_page["kind"].as_str().unwrap_or("");
        if kind == "scene" || (kind == "locations" && has_backgrounds) {
            continue;
        }
        let key = old_page["key"].as_str().unwrap_or("");
        if let Some(page) = page_plan
            .iter()
            .position(|p| p["key"].as_str() == Some(key))
            .map(|i| i + 1)
        {
            let mut item = original.clone();
            item["page"] = json!(page);
            placements.push(item);
        }
    }
    placements
}

fn initial_board_config(
    args: &BuildStoryboardArgs<'_>,
    metadata: &Value,
    packets: &[Value],
) -> Value {
    let mut scene_content = serde_json::Map::new();
    for p in packets {
        let id = p["id"].as_str().unwrap_or("");
        scene_content.insert(
            id.to_owned(),
            json!({
                "sections": p.get("sections").cloned().unwrap_or_else(|| json!({})),
                "notes": p.get("notes").cloned().unwrap_or_else(|| json!([])),
                "source": p.get("source").cloned().unwrap_or(Value::Null),
                "lines": p.get("lines").cloned().unwrap_or_else(|| json!([])),
            }),
        );
    }
    json!({
        "title": args.title,
        "transcript": args.transcript,
        "captions": metadata.get("captions").cloned().unwrap_or(Value::Null),
        "allow_timing_overrun": args.allow_timing_overrun,
        "duration_sec": metadata["duration_sec"],
        "recording_duration_verified": metadata["recording_duration_verified"] == true,
        "audio": metadata.get("audio").cloned().unwrap_or(Value::Null),
        "project": metadata.get("project").cloned().unwrap_or(Value::Null),
        "scene_content": scene_content,
    })
}

#[allow(clippy::too_many_arguments)]
fn populate_board_result(
    result: &mut Value,
    args: &BuildStoryboardArgs<'_>,
    template: &Path,
    mapping: &Value,
    packets: &[Value],
    previews: &[String],
    deck: &Path,
    word_count: usize,
    overrun: f64,
    timing_warning: Option<&Value>,
) -> Result<()> {
    let first_scene = result["validation"]["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .position(|p| p["kind"] == "scene")
        .unwrap_or(0);
    let page_titles: Vec<Value> = result["page_plan"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| p["title"].clone())
        .collect();
    let scene_titles: Vec<String> = packets
        .iter()
        .map(|p| {
            format!(
                "{} · {:.3}–{:.3}",
                p["id"].as_str().unwrap_or(""),
                p["start_seconds"].as_f64().unwrap_or(0.0),
                p["end_seconds"].as_f64().unwrap_or(0.0)
            )
        })
        .collect();
    let producer_label = args
        .transcript
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let deck_str = crate::command_center::shell_quote(&deck.to_string_lossy());
    let inspect_arg =
        crate::command_center::shell_quote(&format!("slide.inspect={{\"index\":{first_scene}}}"));
    result["mapping"] = mapping.clone();
    result["template"] = json!("deckcraft");
    result["template_file"] = json!(template);
    result["scene_map"] = json!(args.scene_map);
    result["scene_targets"] = json!(packets.iter().map(|p| p["id"].clone()).collect::<Vec<_>>());
    result["scene_count"] = json!(packets.len());
    result["first_scene_page"] = json!(first_scene + 1);
    result["scene_titles"] = json!(scene_titles);
    result["previews"] = json!(previews);
    result["page_size"] = json!([SIZE[0] as i64, SIZE[1] as i64]);
    result["stage_rect"] = json!([
        SCENE_ART_REGION[0] as i64,
        SCENE_ART_REGION[1] as i64,
        SCENE_ART_REGION[2] as i64,
        SCENE_ART_REGION[3] as i64
    ]);
    result["links"] = result["validation"]["link_regions"].clone();
    result["page_titles"] = json!(page_titles);
    result["title"] = json!(args.title);
    result["editor"] = json!("DeckCraft native document; PDF export");
    result["source_transcript"] = json!(args.transcript);
    result["source_transcript_sha256"] = json!(sha(args.transcript)?);
    result["source_producer_label"] = json!(producer_label);
    result["producer_executable_version"] = json!("not recorded by native array");
    result["source_words"] = json!(word_count);
    result["timing_valid"] = json!(overrun <= 0.001);
    result["timing_warning"] = timing_warning.cloned().unwrap_or(Value::Null);
    result["next_commands"] = json!([
        format!("deckcraft-cli info {deck_str}"),
        format!("deckcraft-cli run --in {deck_str} --print --cmd {inspect_arg}"),
    ]);
    Ok(())
}

fn scene_images_from_packets(packets: &[Value]) -> Value {
    let mut map = serde_json::Map::new();
    for p in packets {
        let Some(id) = p["id"].as_str() else {
            continue;
        };
        let Some(r) = p["reference"].as_str().filter(|s| Path::new(s).is_file()) else {
            continue;
        };
        let name = Path::new(r)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("reference");
        map.insert(id.to_owned(), json!({"file": r, "name": name}));
    }
    Value::Object(map)
}

fn prepare_board_inputs(
    args: &BuildStoryboardArgs<'_>,
    metadata: &Value,
    packets: &[Value],
    words: &[Value],
    groups: &[Value],
    template: &Path,
) -> Result<(PathBuf, PathBuf, f64, Value, Value)> {
    let source_root = resolve_source_root(args.source, metadata);
    let mut config = initial_board_config(args, metadata, packets);
    let schema = template_schema(template, args.out)?;
    let expected_fields: Vec<String> = schema["scene"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["role"].as_str().map(str::to_owned))
        .collect();
    let (scene_content, mut mapping) = normalize_scene_content(packets, &expected_fields);
    config["scene_content"] = scene_content;
    mapping["template_schema"] = schema;
    apply_saved_scene_text_fields(packets, &mut config, &mut mapping);
    config["fit_imported_text"] = json!(true);
    config["scene_images"] = scene_images_from_packets(packets);
    if let Some(official) = source_root
        .as_ref()
        .map(|r| r.join("artifacts/official_lyrics.json"))
        .filter(|p| p.is_file())
    {
        config["official_lyrics_source"] = json!(official);
    }
    mapping["timing_conflicts"] = json!(compute_timing_conflicts(groups, words));
    config["asset_links"] = metadata.get("assets").cloned().unwrap_or_else(|| json!([]));
    restore_previous_inventory(metadata, &mut config);
    discover_project_inventory(source_root.as_deref(), &mut config);
    let (config_path, grouping, overrun) = finalize_mapping_and_write_inputs(
        args.out,
        &mut config,
        &mut mapping,
        groups,
        packets,
        words,
        metadata,
        template,
    )?;
    Ok((config_path, grouping, overrun, config, mapping))
}

fn attach_remapped_placements(
    result: &mut Value,
    packets: &[Value],
    metadata: &Value,
    panels: Option<&str>,
) -> Result<()> {
    let page_plan = result["page_plan"].as_array().cloned().unwrap_or_default();
    let mut placements = remap_scene_placements(packets, &page_plan, panels)?;
    placements.extend(remap_inventory_placements(
        &metadata["previous_board"],
        &page_plan,
        !metadata["backgrounds"].is_null(),
    ));
    if !placements.is_empty() {
        result["design"] = json!({"placements": placements, "dirty": false});
    }
    Ok(())
}

pub(crate) fn build(args: BuildStoryboardArgs<'_>) -> Result<Value> {
    std::fs::create_dir_all(args.out)?;
    let packets = canonical_scenes(args.scene_map)?;
    let metadata: Value = serde_json::from_str(&std::fs::read_to_string(args.scene_map)?)
        .map_err(|e| invalid(e.to_string()))?;
    let words = crate::storyboard_layout::load_words(&json!({"transcript": args.transcript}))?;
    let groups: Vec<Value> = packets
        .iter()
        .map(|p| build_scene_group(p, &words, args.panels))
        .collect::<Result<_>>()?;
    let template = args.template.canonicalize()?;
    let (config_path, grouping, overrun, config, mapping) =
        prepare_board_inputs(&args, &metadata, &packets, &words, &groups, &template)?;
    let deck = args.out.join("movie-board.deckcraft");
    let mut result = match generate(&template, &config_path, &deck, Some(&grouping), false) {
        Ok(res) => res,
        Err(EngineError::InvalidParams(msg)) => {
            return Ok(json!({
                "ok": false,
                "status": "draft blocked; active storyboard unchanged",
                "error": msg,
                "template_file": template,
                "mapping": mapping,
                "config": config_path,
                "scene_map": args.scene_map,
                "correction": "Resolve the reported source/template mismatch, then rerun the same CLI command; original data is retained",
            }));
        }
        Err(other) => return Err(other),
    };
    let page_count = result["page_plan"].as_array().map_or(0, Vec::len);
    let pdf_path = PathBuf::from(result["pdf"].as_str().unwrap_or(""));
    let previews = render_slide_previews(args.out, &pdf_path, page_count)?;
    populate_board_result(
        &mut result,
        &args,
        &template,
        &mapping,
        &packets,
        &previews,
        &deck,
        words.len(),
        overrun,
        config.get("timing_warning"),
    )?;
    attach_remapped_placements(&mut result, &packets, &metadata, args.panels)?;
    dump(&args.out.join("manifest.json"), &result)?;
    Ok(result)
}
