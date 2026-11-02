//! Native storyboard template population and validation via DeckCraft MCP.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{EngineError, Result};

pub(crate) const VERSION: &str = "4.1";
pub(crate) const SIZE: [f64; 2] = [1080.0, 1600.0];
pub(crate) const TOC_PAGE_SIZE: usize = 20;
pub(crate) const SCENE_ART_REGION: [f64; 4] = [60.0, 220.0, 960.0, 540.0];
pub(crate) const INK: &str = "#F0EBDC";
pub(crate) const TEAL: &str = "#D7B46A";
pub(crate) const MUTED: &str = "#B7B4AB";

pub(crate) fn invalid(msg: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(msg.into())
}

pub(crate) fn dump(path: &Path, value: &Value) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| invalid(e.to_string()))?;
    text.push('\n');
    std::fs::write(path, text)?;
    Ok(())
}

pub(crate) fn sha(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub(crate) fn clock(t: f64) -> String {
    let n = (t * 1000.0).round() as i64;
    let n = n.max(0);
    format!("{:02}:{:02}.{:03}", n / 60000, (n / 1000) % 60, n % 1000)
}

pub(crate) fn norm(t: &str) -> String {
    t.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

pub(crate) fn panel_region(index: usize) -> [f64; 4] {
    [
        60.0 + ((index % 2) * 496) as f64,
        220.0 + ((index / 2) * 200) as f64,
        464.0,
        160.0,
    ]
}

pub(crate) fn fit_rect(width: f64, height: f64, region: [f64; 4]) -> Result<[f64; 4]> {
    if ![width, height, region[2], region[3]]
        .iter()
        .all(|v| v.is_finite() && *v > 0.0)
    {
        return Err(invalid("Picture and region dimensions must be positive"));
    }
    let [x, y, w, h] = region;
    let scale = (w / width).min(h / height);
    let (fw, fh) = (width * scale, height * scale);
    Ok([x + (w - fw) / 2.0, y + (h - fh) / 2.0, fw, fh])
}

pub(crate) fn path_to_file_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let mut out = String::from("file://");
    for b in raw.as_bytes() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(
                *b,
                b'/' | b'-'
                    | b'_'
                    | b'.'
                    | b'~'
                    | b':'
                    | b'@'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
            );
        if keep {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub(crate) fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path_part = rest.split(['?', '#']).next()?;
    let bytes = path_part.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            let val = u8::from_str_radix(hex, 16).ok()?;
            decoded.push(val);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok().map(PathBuf::from)
}

fn validate_single_word(
    w: &Value,
    previous: f64,
    duration: f64,
    allow_overrun: bool,
) -> Result<(String, f64)> {
    let a = w["start"].as_f64().unwrap_or(f64::NAN);
    let b = w["end"].as_f64().unwrap_or(f64::NAN);
    let text = w["text"].as_str().unwrap_or("").trim();
    let within = 0.0 <= a && a <= b && (b <= duration + 0.001 || (allow_overrun && a < duration));
    if !a.is_finite() || !b.is_finite() || !within || a < previous || text.is_empty() {
        return Err(invalid("Invalid or unordered source word timing"));
    }
    let id = w["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("Word occurrence IDs must be nonempty and unique"))?;
    Ok((id.to_owned(), a))
}

pub(crate) fn validate_words(words: &[Value], duration: f64, allow_overrun: bool) -> Result<()> {
    if words.is_empty() {
        return Err(invalid("Word occurrence IDs must be nonempty and unique"));
    }
    let mut ids = BTreeSet::new();
    let mut previous = -1.0;
    for w in words {
        let (id, start) = validate_single_word(w, previous, duration, allow_overrun)?;
        if !ids.insert(id) {
            return Err(invalid("Word occurrence IDs must be nonempty and unique"));
        }
        previous = start;
    }
    Ok(())
}

fn check_group_ids(groups: &[Value], words: &[Value]) -> Result<()> {
    let group_word_ids: Vec<&str> = groups
        .iter()
        .flat_map(|g| g["word_ids"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let source_ids: Vec<&str> = words.iter().filter_map(|w| w["id"].as_str()).collect();
    if group_word_ids != source_ids {
        return Err(invalid(
            "Grouping must preserve every word occurrence exactly once and in source order",
        ));
    }
    let unique_groups: BTreeSet<&str> = groups.iter().filter_map(|g| g["id"].as_str()).collect();
    if unique_groups.len() != groups.len() {
        return Err(invalid("Group targets must be unique"));
    }
    Ok(())
}

fn word_outside_span(word: &Value, group: &Value) -> bool {
    let start = word["start"].as_f64().unwrap_or(0.0);
    let end = word["end"].as_f64().unwrap_or(0.0);
    let span_start = group["span_start"].as_f64().unwrap_or(0.0);
    let span_end = group["span_end"].as_f64().unwrap_or(0.0);
    if start < span_start - 0.001 {
        return true;
    }
    if end > span_end + 0.001 {
        return group["word_assignment"].as_str()
            != Some("source-start timestamp; canonical scene cuts preserved");
    }
    false
}

fn check_group_spans(
    groups: &[Value],
    lookup: &BTreeMap<String, Value>,
    duration: f64,
) -> Result<()> {
    let mut previous = 0.0;
    for group in groups {
        let span_start = group["span_start"].as_f64().unwrap_or(-1.0);
        let span_end = group["span_end"].as_f64().unwrap_or(-1.0);
        if (span_start - previous).abs() > 0.001 || span_end <= span_start {
            return Err(invalid("Scene spans must be positive and contiguous"));
        }
        for id in group["word_ids"].as_array().into_iter().flatten() {
            let key = id.as_str().unwrap_or("");
            let word = lookup
                .get(key)
                .ok_or_else(|| invalid("Scene span excludes its source word timing"))?;
            if word_outside_span(word, group) {
                return Err(invalid("Scene span excludes its source word timing"));
            }
        }
        previous = span_end;
    }
    if (previous - duration).abs() > 0.001 {
        return Err(invalid("Scene spans do not cover the full recording"));
    }
    Ok(())
}

pub(crate) fn validate_groups(groups: &[Value], words: &[Value], duration: f64) -> Result<()> {
    check_group_ids(groups, words)?;
    let lookup: BTreeMap<String, Value> = words
        .iter()
        .filter_map(|w| Some((w["id"].as_str()?.to_owned(), w.clone())))
        .collect();
    check_group_spans(groups, &lookup, duration)
}

pub(crate) fn load_words(c: &Value) -> Result<Vec<Value>> {
    let path = c["transcript"]
        .as_str()
        .ok_or_else(|| invalid("Missing transcript path"))?;
    let text = std::fs::read_to_string(path)?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?;
    let list = doc
        .as_array()
        .or_else(|| doc["words"].as_array())
        .ok_or_else(|| invalid("Transcript has no words array"))?;
    Ok(list
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let mut item = w.clone();
            if item.get("id").and_then(Value::as_str).is_none() {
                item["id"] = json!(format!("w{i}"));
            }
            item
        })
        .collect())
}

fn recording_duration(c: &Value) -> Result<f64> {
    if let Some(d) = c.get("duration_sec").and_then(Value::as_f64) {
        return Ok(d);
    }
    let audiomap = c["audiomap"]
        .as_str()
        .ok_or_else(|| invalid("Missing duration_sec or audiomap"))?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(audiomap)?)
        .map_err(|e| invalid(e.to_string()))?;
    doc["audio"]["duration_sec"]
        .as_f64()
        .ok_or_else(|| invalid("Missing audio.duration_sec"))
}

fn parse_timestamp_token(token: &str) -> Option<f64> {
    let cleaned = token.trim();
    let mut parts = cleaned.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let sec_ms = parts.next()?;
    let (s_str, ms_str) = sec_ms.split_once([',', '.'])?;
    if !s_str.chars().all(|c| c.is_ascii_digit()) || !ms_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let s: f64 = s_str.parse().ok()?;
    let ms: f64 = ms_str.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s + ms / 1000.0)
}

fn parse_cue(block: &str, cues: &[Value], duration: f64, allow_overrun: bool) -> Result<Value> {
    let lines: Vec<&str> = block.lines().collect();
    let hit = lines
        .iter()
        .position(|l| l.contains("-->"))
        .ok_or_else(|| invalid("Malformed caption block"))?;
    let times: Vec<f64> = lines[hit]
        .split("-->")
        .filter_map(|seg| {
            seg.split_whitespace()
                .next()
                .and_then(parse_timestamp_token)
        })
        .collect();
    if times.len() != 2 {
        return Err(invalid("Malformed cue timestamp"));
    }
    let (a, b) = (times[0], times[1]);
    let prev_end = cues.last().and_then(|last| last["end"].as_f64());
    if a < 0.0
        || b < a
        || (b > duration + 0.001 && !allow_overrun)
        || prev_end.is_some_and(|pe| a < pe)
    {
        return Err(invalid("Caption timings overlap or exceed recording"));
    }
    Ok(json!({
        "id": format!("cue-{:03}", cues.len() + 1),
        "start": a,
        "end": b,
        "text": lines[hit + 1..].join(" "),
        "status": "needs review"
    }))
}

fn split_blank_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in text.trim().lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    blocks
}

fn parse_cues(text: &str, duration: f64, allow_overrun: bool) -> Result<Vec<Value>> {
    let mut cues = Vec::new();
    for block in split_blank_blocks(text) {
        let cue = parse_cue(&block, &cues, duration, allow_overrun)?;
        cues.push(cue);
    }
    Ok(cues)
}

fn native_cues(words: &[Value]) -> Vec<Value> {
    words
        .iter()
        .map(|w| {
            let mut item = w.clone();
            item["status"] = json!("native word timing; no supplied phrase captions");
            item
        })
        .collect()
}

fn check_caption_words(cues: &[Value], words: &[Value]) -> Result<()> {
    let cue_tokens: Vec<String> = cues
        .iter()
        .flat_map(|x| x["text"].as_str().unwrap_or("").split_whitespace())
        .map(norm)
        .collect();
    let word_tokens: Vec<String> = words
        .iter()
        .flat_map(|w| w["text"].as_str().unwrap_or("").split_whitespace())
        .map(norm)
        .collect();
    if cue_tokens != word_tokens {
        return Err(invalid(
            "Caption text does not preserve the word transcript occurrence sequence",
        ));
    }
    Ok(())
}

fn populate_group_text_and_timing(g: &mut Value, lookup: &BTreeMap<String, Value>) {
    let text = if let Some(lines) = g.get("lyric_lines").and_then(Value::as_array)
        && !lines.is_empty()
    {
        lines
            .iter()
            .filter_map(|l| l["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        g["word_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| lookup.get(id.as_str()?)?["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    g["text"] = json!(text);
    if let Some(ids) = g["word_ids"].as_array()
        && let (Some(first), Some(last)) = (
            ids.first().and_then(Value::as_str),
            ids.last().and_then(Value::as_str),
        )
        && let (Some(w0), Some(w1)) = (lookup.get(first), lookup.get(last))
    {
        g["lyric_start"] = w0["start"].clone();
        g["lyric_end"] = w1["end"].clone();
    }
}

fn load_groups(grouping: Option<&Path>, words: &[Value], duration: f64) -> Result<Vec<Value>> {
    let path = grouping.ok_or_else(|| invalid("Grouping path is required"))?;
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path)?)
        .map_err(|e| invalid(e.to_string()))?;
    let mut groups = doc["groups"]
        .as_array()
        .cloned()
        .ok_or_else(|| invalid("Grouping file has no groups"))?;
    let lookup: BTreeMap<String, Value> = words
        .iter()
        .filter_map(|w| Some((w["id"].as_str()?.to_owned(), w.clone())))
        .collect();
    for g in &mut groups {
        populate_group_text_and_timing(g, &lookup);
    }
    validate_groups(&groups, words, duration)?;
    Ok(groups)
}

fn check_scene_images(c: &Value, groups: &[Value]) -> Result<()> {
    let Some(map_val) = c.get("scene_images").filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let Some(image_map) = map_val.as_object() else {
        return Err(invalid(
            "scene_images keys must identify actual grouping IDs",
        ));
    };
    let valid_ids: BTreeSet<&str> = groups.iter().filter_map(|g| g["id"].as_str()).collect();
    if image_map.keys().any(|k| !valid_ids.contains(k.as_str())) {
        return Err(invalid(
            "scene_images keys must identify actual grouping IDs",
        ));
    }
    if image_map.values().any(|r| {
        r.get("file")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
    }) {
        return Err(invalid(
            "Each scene_images record requires an explicit file path",
        ));
    }
    Ok(())
}

type InputsTuple = (Value, Vec<Value>, Vec<Value>, Vec<Value>, f64);

fn read_inputs(config: &Path, grouping: Option<&Path>) -> Result<InputsTuple> {
    let c: Value = serde_json::from_str(&std::fs::read_to_string(config)?)
        .map_err(|e| invalid(e.to_string()))?;
    if c["empty_template"] == true {
        let dur = c["duration_sec"].as_f64().unwrap_or(0.0);
        return Ok((c, Vec::new(), Vec::new(), Vec::new(), dur));
    }
    let words = load_words(&c)?;
    let duration = recording_duration(&c)?;
    let allow_overrun = c["allow_timing_overrun"] == true;
    validate_words(&words, duration, allow_overrun)?;
    let cues = if let Some(cap_path) = c["captions"].as_str().filter(|s| !s.is_empty()) {
        parse_cues(&std::fs::read_to_string(cap_path)?, duration, allow_overrun)?
    } else {
        native_cues(&words)
    };
    check_caption_words(&cues, &words)?;
    let groups = load_groups(grouping, &words, duration)?;
    check_scene_images(&c, &groups)?;
    Ok((c, words, cues, groups, duration))
}

pub(crate) struct Deck {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    serial: u64,
    queue: Vec<Value>,
}

impl Deck {
    pub(crate) fn new(template_root: &Path, output_root: &Path) -> Result<Self> {
        let bin = crate::workspace::resolve_tool("deckcraft-cli");
        let mut child = Command::new(&bin)
            .args([
                "mcp",
                "--automation-read-root",
                &template_root.to_string_lossy(),
                "--automation-write-root",
                &output_root.to_string_lossy(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| EngineError::ExternalTool("deckcraft-cli".into(), e.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| invalid("Failed to open DeckCraft stdin"))?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| invalid("Failed to open DeckCraft stdout"))?,
        );
        let mut deck = Self {
            child,
            stdin,
            stdout,
            serial: 0,
            queue: Vec::new(),
        };
        deck.rpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "project-local-storyboard", "version": VERSION}
            }),
        )?;
        Ok(deck)
    }

    fn parse_rpc_response(line: &str) -> Result<Value> {
        let r: Value = serde_json::from_str(line).map_err(|e| invalid(e.to_string()))?;
        if r.get("error").is_some() || r["result"]["isError"] == true {
            return Err(invalid(format!("{r}")));
        }
        if let Some(blocks) = r["result"]["content"].as_array()
            && let Some(first) = blocks.first()
            && first["type"] == "text"
            && let Some(text) = first["text"].as_str()
        {
            return Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into())));
        }
        Ok(r.get("result").cloned().unwrap_or_else(|| json!({})))
    }

    pub(crate) fn rpc(&mut self, method: &str, params: Value) -> Result<Value> {
        self.serial += 1;
        let req = json!({
            "jsonrpc": "2.0",
            "id": self.serial,
            "method": method,
            "params": params
        });
        let mut payload = serde_json::to_string(&req).map_err(|e| invalid(e.to_string()))?;
        payload.push('\n');
        self.stdin.write_all(payload.as_bytes())?;
        self.stdin.flush()?;
        let mut line = String::new();
        let n = self.stdout.read_line(&mut line)?;
        if n == 0 {
            let mut err_text = String::new();
            if let Some(mut stderr) = self.child.stderr.take() {
                let _ = stderr.read_to_string(&mut err_text);
            }
            return Err(invalid(format!("DeckCraft stopped: {err_text}")));
        }
        Self::parse_rpc_response(&line)
    }

    pub(crate) fn cmd(&mut self, c: &str, params: Value) -> Result<Value> {
        self.rpc(
            "tools/call",
            json!({"name": "run_command", "arguments": {"command": c, "params": params}}),
        )
    }

    pub(crate) fn q(&mut self, c: &str, params: Value) {
        self.queue.push(json!({"command": c, "params": params}));
    }

    pub(crate) fn flush(&mut self) -> Result<()> {
        if !self.queue.is_empty() {
            let commands = std::mem::take(&mut self.queue);
            self.rpc(
                "tools/call",
                json!({"name": "batch", "arguments": {"commands": commands}}),
            )?;
        }
        Ok(())
    }

    pub(crate) fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    pub(crate) fn text(
        &mut self,
        t: &str,
        rect: [f64; 4],
        size: f64,
        color: &str,
        bold: bool,
        role: Option<&str>,
    ) {
        self.q("insert.textBox", json!({"rect": rect, "text": t}));
        self.q("format.font", json!({"family": "DejaVu Sans"}));
        self.q("format.size", json!({"size": size}));
        self.q("format.color", json!({"color": color}));
        self.q("format.bold", json!({"on": bold}));
        self.q(
            "format.margins",
            json!({"left": 0, "right": 0, "top": 0, "bottom": 0}),
        );
        if let Some(r) = role {
            self.q("shape.altText", json!({"text": r}));
            self.q("shape.rename", json!({"name": r}));
        }
    }
}

impl Drop for Deck {
    fn drop(&mut self) {
        self.close();
    }
}

pub(crate) fn inspect_pages(d: &mut Deck) -> Result<Vec<Value>> {
    let info = d.cmd("document.inspect", json!({}))?;
    let count = info["slides"].as_array().map_or(0, Vec::len);
    let mut slides = Vec::with_capacity(count);
    for i in 0..count {
        slides.push(d.cmd("slide.inspect", json!({"index": i}))?);
    }
    Ok(slides)
}

pub(crate) fn note(slide: &Value) -> Value {
    slide["notes"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| json!({}))
}

fn snapshot(d: &mut Deck) -> Result<Vec<Value>> {
    Ok(inspect_pages(d)?
        .into_iter()
        .map(|s| json!({"id": s["id"], "notes": s["notes"], "shapes": s["shapes"]}))
        .collect())
}

fn source_fingerprint(c: &Value, groups: &[Value]) -> Result<String> {
    let mut inputs = serde_json::Map::new();
    for k in ["transcript", "captions", "audiomap"] {
        if let Some(p) = c.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()) {
            inputs.insert(k.to_owned(), json!(sha(Path::new(p))?));
        }
    }
    let payload = json!({
        "config": c,
        "inputs": inputs,
        "groups": groups,
    });
    let bytes = serde_json::to_vec(&payload).map_err(|e| invalid(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn toc_entries(pages: &[Value]) -> Vec<(usize, Value)> {
    pages
        .iter()
        .enumerate()
        .filter(|(_, p)| p["kind"] != "toc")
        .map(|(i, p)| (i, p.clone()))
        .collect()
}

fn sheet_pages(kind: &str, records: &[Value], label: &str) -> Vec<Value> {
    let fallback = [json!({"id": "inventory", "name": "No linked artwork yet"})];
    let source = if records.is_empty() {
        &fallback[..]
    } else {
        records
    };
    let mut categories: Vec<(String, Vec<Value>)> = Vec::new();
    for record in source {
        let cat = record
            .get("category")
            .and_then(Value::as_str)
            .unwrap_or(label);
        if let Some((_, items)) = categories.iter_mut().find(|(c, _)| c == cat) {
            items.push(record.clone());
        } else {
            categories.push((cat.to_owned(), vec![record.clone()]));
        }
    }
    let prefix = if kind == "assets" {
        "asset:"
    } else {
        "location:"
    };
    let mut pages = Vec::new();
    for (category_index, (category, items)) in categories.into_iter().enumerate() {
        for (sheet_idx, chunk) in items.chunks(4).enumerate() {
            pages.push(json!({
                "kind": kind,
                "key": format!("{prefix}{}:{}", category_index + 1, sheet_idx + 1),
                "title": format!("{category} / sheet {}", sheet_idx + 1),
                "sheet": chunk,
            }));
        }
    }
    pages
}

pub(crate) fn build_plan(c: &Value, groups: &[Value], representative: bool) -> Vec<Value> {
    if representative && let Some(first) = groups.first() {
        return vec![json!({
            "kind": "scene",
            "key": format!("scene:{}", first["id"].as_str().unwrap_or_default()),
            "title": "Scene 01",
            "group": first,
        })];
    }
    let mut pages = vec![json!({
        "kind": "overview",
        "key": "overview",
        "title": "Song story & backgrounds"
    })];
    let assets = c["assets"].as_array().map_or(&[][..], Vec::as_slice);
    let locations = c["locations"].as_array().map_or(&[][..], Vec::as_slice);
    pages.extend(sheet_pages("assets", assets, "Characters & props"));
    pages.extend(sheet_pages(
        "locations",
        locations,
        "Backgrounds & locations",
    ));
    for (i, x) in groups.iter().enumerate() {
        let prefix = x["section_label"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| format!("{s} / "))
            .unwrap_or_default();
        pages.push(json!({
            "kind": "scene",
            "key": format!("scene:{}", x["id"].as_str().unwrap_or_default()),
            "title": format!("{prefix}Scene {:02}", i + 1),
            "group": x,
        }));
    }
    let count = pages.len().div_ceil(TOC_PAGE_SIZE);
    let mut out = vec![pages[0].clone()];
    for i in 0..count {
        let title = if count == 1 {
            "Contents".to_owned()
        } else {
            format!("Contents {}/{count}", i + 1)
        };
        out.push(json!({"kind": "toc", "key": format!("toc:{}", i + 1), "title": title}));
    }
    out.extend_from_slice(&pages[1..]);
    out
}

fn role_text_color(role: &str) -> &'static str {
    if role == "auto.scene.lyrics" {
        "#211E18"
    } else if role.starts_with("auto.sheet.")
        || role.starts_with("auto.toc.meta.")
        || role == "state.scene.reference"
    {
        TEAL
    } else if role == "auto.subtitle" {
        MUTED
    } else {
        INK
    }
}

fn box_coords(val: &Value) -> Option<[f64; 4]> {
    if let Some(arr) = val.as_array()
        && arr.len() == 4
    {
        return Some([
            arr[0].as_f64()?,
            arr[1].as_f64()?,
            arr[2].as_f64()?,
            arr[3].as_f64()?,
        ]);
    }
    Some([
        val.get("x")?.as_f64()?,
        val.get("y")?.as_f64()?,
        val.get("w")?.as_f64()?,
        val.get("h")?.as_f64()?,
    ])
}

fn fit_text_to_card(d: &mut Deck, slide: &Value, sh: &Value, role: &str) {
    let suffix = role.get(5..).unwrap_or("");
    let target_alt = format!("layout.card.{suffix}");
    let Some(card) = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["altText"].as_str() == Some(&target_alt))
    else {
        return;
    };
    let Some([x, y, w, h]) = box_coords(&card["box"]) else {
        return;
    };
    let mut height = h - 76.0;
    if role == "user.scene.continuity" {
        height = height.min(1130.0 - (y + 56.0) - 12.0);
    }
    d.q("edit.select", json!({"ids": [sh["id"]]}));
    d.q("format.autofit", json!({"mode": "shrink"}));
    d.q(
        "shape.setBounds",
        json!({"ids": [sh["id"]], "x": x + 18.0, "y": y + 56.0, "w": w - 36.0, "h": height}),
    );
}

fn fill_text(d: &mut Deck, slide: &Value, role: &str, value: &str, size: Option<f64>, fit: bool) {
    let Some(sh) = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["altText"].as_str() == Some(role))
    else {
        return;
    };
    d.q("text.set", json!({"id": sh["id"], "text": value}));
    if !value.is_empty() {
        d.q("edit.select", json!({"ids": [sh["id"]]}));
        d.q("format.color", json!({"color": role_text_color(role)}));
    }
    if let Some(sz) = size {
        d.q("edit.select", json!({"ids": [sh["id"]]}));
        d.q("format.size", json!({"size": sz}));
    }
    if fit {
        fit_text_to_card(d, slide, sh, role);
    }
}

fn native_picture_dims(native: &Value) -> Result<(f64, f64)> {
    let w = native["w"]
        .as_f64()
        .or_else(|| native.get(2).and_then(Value::as_f64))
        .ok_or_else(|| invalid("Missing native picture width"))?;
    let h = native["h"]
        .as_f64()
        .or_else(|| native.get(3).and_then(Value::as_f64))
        .ok_or_else(|| invalid("Missing native picture height"))?;
    Ok((w, h))
}

fn style_scene_picture(
    d: &mut Deck,
    pid: &Value,
    fitted: [f64; 4],
    group_id: &str,
    label: &str,
    path: &Path,
) -> Result<()> {
    for params in [
        json!({"ids": [pid], "x": fitted[0], "y": fitted[1], "w": fitted[2], "h": fitted[3], "lockAspect": false}),
        json!({"ids": [pid], "lockAspect": true}),
    ] {
        d.cmd("shape.sizePosition", params)?;
    }
    d.cmd("edit.select", json!({"ids": [pid]}))?;
    d.cmd(
        "shape.altText",
        json!({"text": format!("user.picture.scene.{group_id}")}),
    )?;
    d.cmd(
        "shape.rename",
        json!({"name": format!("Reference only · {label}")}),
    )?;
    d.cmd(
        "insert.hyperlink",
        json!({"url": path_to_file_uri(path), "tooltip": "Reference source; not an approved production asset"}),
    )?;
    Ok(())
}

fn place_scene_picture(
    d: &mut Deck,
    slide: &Value,
    record: &Value,
    group_id: &str,
    region: Option<[f64; 4]>,
) -> Result<Value> {
    let raw_file = record["file"]
        .as_str()
        .ok_or_else(|| invalid("Scene image missing file path"))?;
    let path = Path::new(raw_file)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(raw_file));
    if !path.is_file() {
        return Err(invalid(format!(
            "Configured scene reference does not exist: {}",
            path.display()
        )));
    }
    d.flush()?;
    let name = record["name"].as_str().unwrap_or("Reference only");
    let picture = d.cmd("insert.picture", json!({"path": path, "name": name}))?;
    let pid = picture["id"].clone();
    let inspected = d.cmd("shape.inspect", json!({"id": pid}))?;
    let (nw, nh) = native_picture_dims(&inspected["box"])?;
    let fitted = fit_rect(nw, nh, region.unwrap_or(SCENE_ART_REGION))?;
    let fallback_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("reference");
    let label = record["name"].as_str().unwrap_or(fallback_name);
    style_scene_picture(d, &pid, fitted, group_id, label, &path)?;
    fill_text(d, slide, "user.scene.reference", "", None, false);
    fill_text(d, slide, "state.scene.reference", "", None, false);
    d.flush()?;
    Ok(json!({
        "file": path,
        "status": "reference only / needs review",
        "region": [60, 220, 960, 540],
        "picture_id": pid,
        "rect": fitted,
    }))
}

fn shrink_panels(d: &mut Deck, slide: &Value, compact: f64) -> Result<()> {
    for shape in slide["shapes"].as_array().into_iter().flatten() {
        let Some([x, y, w, h]) = box_coords(&shape["box"]) else {
            continue;
        };
        if y >= 790.0 {
            d.q(
                "shape.setBounds",
                json!({"ids": [shape["id"]], "x": x, "y": y - compact, "w": w, "h": h}),
            );
        }
        if shape["altText"].as_str() == Some("layout.scene.picturewell") {
            d.q(
                "shape.setBounds",
                json!({"ids": [shape["id"]], "x": x, "y": y, "w": w, "h": 400}),
            );
        }
    }
    d.flush()
}

fn contents_nav(
    d: &mut Deck,
    slide: &Value,
    representative: bool,
    kind: &str,
    all_pages: &[Value],
) {
    let Some(nav) = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["altText"].as_str() == Some("nav.contents"))
    else {
        return;
    };
    let plain = representative || kind == "toc";
    if plain {
        d.q(
            "shape.action",
            json!({"ids": [nav["id"]], "action": "none"}),
        );
    } else if let Some(toc_idx) = all_pages.iter().position(|p| p["kind"] == "toc") {
        d.q(
            "shape.action",
            json!({"ids": [nav["id"]], "action": "slide", "slide": toc_idx}),
        );
    }
}

fn fill_toc_slot(d: &mut Deck, slide: &Value, slot: usize, entry: Option<&(usize, Value)>) {
    let row_role = format!("layout.toc.row.{slot}");
    let label_role = format!("auto.toc.label.{slot}");
    let meta_role = format!("auto.toc.meta.{slot}");
    let Some((index, target)) = entry else {
        let ids: Vec<Value> = slide["shapes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|x| {
                x["altText"]
                    .as_str()
                    .is_some_and(|a| a == row_role || a == label_role || a == meta_role)
            })
            .map(|x| x["id"].clone())
            .collect();
        d.q("edit.delete", json!({"ids": ids}));
        return;
    };
    fill_text(
        d,
        slide,
        &label_role,
        target["title"].as_str().unwrap_or(""),
        Some(22.0),
        false,
    );
    fill_text(
        d,
        slide,
        &meta_role,
        &format!("{:02}", index + 1),
        Some(18.0),
        false,
    );
    if let Some(sh) = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["altText"].as_str() == Some(&label_role))
    {
        d.q(
            "shape.action",
            json!({"ids": [sh["id"]], "action": "slide", "slide": *index}),
        );
    }
}

fn fill_toc(d: &mut Deck, slide: &Value, page: &Value, all_pages: &[Value]) {
    let at = page["key"]
        .as_str()
        .and_then(|k| k.split(':').nth(1))
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let entries = toc_entries(all_pages);
    for slot in 0..TOC_PAGE_SIZE {
        let entry_idx = at * TOC_PAGE_SIZE + slot;
        fill_toc_slot(d, slide, slot, entries.get(entry_idx));
    }
}

fn fill_overview_links(d: &mut Deck, c: &Value) {
    for (i, (label, key)) in [
        ("Audio", "audio"),
        ("Transcript", "transcript"),
        ("Captions", "captions"),
        ("Project", "project"),
    ]
    .iter()
    .enumerate()
    {
        let Some(raw) = c
            .get(*key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let path = Path::new(raw);
        if !path.exists() {
            continue;
        }
        let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let role = format!("source.{i}");
        d.text(
            &format!("{label} ↗"),
            [60.0 + (i * 240) as f64, 1470.0, 220.0, 30.0],
            18.0,
            TEAL,
            true,
            Some(&role),
        );
        d.q(
            "insert.hyperlink",
            json!({"url": path_to_file_uri(&resolved), "tooltip": label}),
        );
    }
}

fn fill_overview(
    d: &mut Deck,
    slide: &Value,
    c: &Value,
    words: &[Value],
    groups: &[Value],
    duration: f64,
    duration_label: &str,
) {
    for (key, role, sz) in [
        ("story", "user.story", 21.0),
        ("background_main", "user.background.main", 18.0),
        ("background_alternate", "user.background.alternate", 18.0),
        ("cast", "user.cast", 18.0),
    ] {
        if let Some(val) = c.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()) {
            fill_text(d, slide, role, val, Some(sz), true);
        }
    }
    let warning = c
        .get("timing_warning")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|w| format!("\nTIMING WARNING: {w}"))
        .unwrap_or_default();
    let source_note = if c["empty_template"] == true {
        "Audio linked; lyrics, scene structure and artwork await your choices."
    } else {
        "Existing transcript words/timing consumed unchanged."
    };
    let source_text = format!(
        "{duration_label}: {}\n{} timed words / {} scenes\n{source_note}{warning}",
        clock(duration),
        words.len(),
        groups.len()
    );
    fill_text(d, slide, "user.source", &source_text, Some(20.0), true);
    fill_overview_links(d, c);
}

fn fill_lyrics(d: &mut Deck, slide: &Value, group: &Value, compact: f64) {
    let has_ids = group["word_ids"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty());
    let text = group["text"].as_str().unwrap_or("");
    let raw = if !text.is_empty() {
        text
    } else if !has_ids {
        "[Instrumental]"
    } else {
        ""
    };
    fill_text(d, slide, "auto.scene.lyrics", raw, Some(27.0), false);
    let has_lines = group["lyric_lines"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty());
    if let Some(lyric_shape) = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|x| x["altText"].as_str() == Some("auto.scene.lyrics"))
    {
        d.q("edit.select", json!({"ids": [lyric_shape["id"]]}));
        d.q("format.autofit", json!({"mode": "shrink"}));
        d.q(
            "shape.setBounds",
            json!({
                "ids": [lyric_shape["id"]],
                "x": if has_lines { 160.0 } else { 80.0 },
                "y": 838.0 - compact,
                "w": if has_lines { 840.0 } else { 920.0 },
                "h": 90.0
            }),
        );
    }
    for (i, line) in group["lyric_lines"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let line_id = line["id"].as_str().unwrap_or("");
        let role = format!("auto.scene.lyric_id.{line_id}");
        d.text(
            line_id,
            [80.0, 838.0 - compact + (i as f64) * 32.4, 76.0, 24.0],
            13.0,
            TEAL,
            false,
            Some(&role),
        );
    }
}

fn fill_scene_sections(d: &mut Deck, slide: &Value, group: &Value, c: &Value) {
    let gid = group["id"].as_str().unwrap_or("");
    let sections = &c["scene_content"][gid]["sections"];
    let fit = c["fit_imported_text"] == true;
    for (source_role, target_role) in [
        ("reference_prompt", "user.scene.image_prompt"),
        ("animation_prompt", "user.scene.animation_prompt"),
        ("layer_stack", "user.scene.continuity"),
        ("action", "user.scene.action"),
        ("animated_text", "user.scene.animated_text"),
        ("drawing_cues", "user.scene.drawing"),
    ] {
        if let Some(val) = sections.get(source_role).and_then(Value::as_str)
            && !val.is_empty()
        {
            fill_text(d, slide, target_role, val, Some(21.0), fit);
        }
    }
}

fn fill_scene(
    d: &mut Deck,
    slide: &Value,
    group: &Value,
    c: &Value,
    compact: f64,
    all_pages: &[Value],
) {
    fill_lyrics(d, slide, group, compact);
    fill_scene_sections(d, slide, group, c);
    let span_start = group["span_start"].as_f64().unwrap_or(0.0);
    let span_end = group["span_end"].as_f64().unwrap_or(0.0);
    let mut t = format!("Scene span {}–{}", clock(span_start), clock(span_end));
    if let Some(ids) = group["word_ids"].as_array()
        && let (Some(first), Some(last)) = (
            ids.first().and_then(Value::as_str),
            ids.last().and_then(Value::as_str),
        )
    {
        let ls = group["lyric_start"].as_f64().unwrap_or(span_start);
        let le = group["lyric_end"].as_f64().unwrap_or(span_end);
        t.push_str(&format!(
            " / Words {}–{} / {first}…{last}",
            clock(ls),
            clock(le)
        ));
    }
    fill_text(d, slide, "auto.subtitle", &t, Some(18.0), false);
    for (label, targetkey, x) in [
        ("Assets", "asset:", 574.0),
        ("Locations", "location:", 780.0),
    ] {
        if let Some(target) = all_pages
            .iter()
            .position(|p| p["key"].as_str().is_some_and(|k| k.starts_with(targetkey)))
        {
            let role = format!("nav.{targetkey}");
            d.text(
                label,
                [x, 1130.0, 140.0, 24.0],
                16.0,
                TEAL,
                true,
                Some(&role),
            );
            d.q(
                "shape.action",
                json!({"action": "slide", "slide": target, "tooltip": label}),
            );
        }
    }
}

fn place_sheet_picture(d: &mut Deck, slide: &Value, record: &Value, slot: usize) -> Result<()> {
    let Some(raw_file) = record
        .get("file")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let path = Path::new(raw_file)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(raw_file));
    if !path.is_file() {
        return Err(invalid(format!(
            "Configured asset does not exist: {}",
            path.display()
        )));
    }
    let pic_role = format!("layout.sheet.{slot}.picture");
    let region = slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|sh| sh["altText"].as_str() == Some(&pic_role))
        .and_then(|sh| box_coords(&sh["box"]))
        .ok_or_else(|| invalid("Missing sheet picture slot"))?;
    d.flush()?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("asset");
    let name = record["name"].as_str().unwrap_or(stem);
    let picture = d.cmd("insert.picture", json!({"path": path, "name": name}))?;
    let pid = picture["id"].clone();
    let inspected = d.cmd("shape.inspect", json!({"id": pid}))?;
    let (nw, nh) = native_picture_dims(&inspected["box"])?;
    let rect = fit_rect(nw, nh, region)?;
    d.cmd(
        "shape.sizePosition",
        json!({"ids": [pid], "x": rect[0], "y": rect[1], "w": rect[2], "h": rect[3], "lockAspect": false}),
    )?;
    d.cmd("edit.select", json!({"ids": [pid]}))?;
    let id = record["id"].as_str().unwrap_or("item");
    d.cmd(
        "shape.altText",
        json!({"text": format!("user.picture.{id}")}),
    )?;
    d.cmd(
        "insert.hyperlink",
        json!({"url": path_to_file_uri(&path), "tooltip": "Existing asset; original pixels and alpha unchanged"}),
    )?;
    Ok(())
}

fn fill_sheet_slot(d: &mut Deck, slide: &Value, record: &Value, slot: usize) -> Result<()> {
    let id = record["id"].as_str().unwrap_or("");
    let title = record["name"].as_str().unwrap_or(id);
    fill_text(
        d,
        slide,
        &format!("auto.sheet.{slot}.title"),
        title,
        Some(20.0),
        false,
    );
    let notes = if record["notes_exact"] == true {
        record["notes"].as_str().unwrap_or("").to_owned()
    } else {
        let base = record["notes"].as_str().unwrap_or(id);
        let suffix = record["file"]
            .as_str()
            .and_then(|f| Path::new(f).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Register existing artwork to populate this slot.");
        format!("{base}\n{suffix}")
    };
    let notes_role = format!("user.sheet.{slot}.notes");
    let card_role = format!("layout.sheet.{slot}.card");
    fill_text(d, slide, &notes_role, &notes, Some(16.0), false);
    let shapes = slide["shapes"].as_array().into_iter().flatten();
    let notes_sh = shapes
        .clone()
        .find(|sh| sh["altText"].as_str() == Some(&notes_role));
    let card_sh = shapes
        .clone()
        .find(|sh| sh["altText"].as_str() == Some(&card_role));
    if let (Some(nsh), Some(csh)) = (notes_sh, card_sh)
        && let Some([cx, cy, _, _]) = box_coords(&csh["box"])
    {
        d.q("edit.select", json!({"ids": [nsh["id"]]}));
        d.q("format.autofit", json!({"mode": "shrink"}));
        d.q(
            "shape.setBounds",
            json!({"ids": [nsh["id"]], "x": cx + 18.0, "y": cy + 390.0, "w": 428.0, "h": 155.0}),
        );
    }
    place_sheet_picture(d, slide, record, slot)
}

fn fill_sheet(d: &mut Deck, slide: &Value, page: &Value) -> Result<()> {
    let sheet = page["sheet"].as_array().map_or(&[][..], Vec::as_slice);
    for slot in 0..4 {
        if slot >= sheet.len() {
            let p1 = format!("layout.sheet.{slot}.");
            let p2 = format!("auto.sheet.{slot}.");
            let p3 = format!("user.sheet.{slot}.");
            let ids: Vec<Value> = slide["shapes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|sh| {
                    sh["altText"].as_str().is_some_and(|a| {
                        a.starts_with(&p1) || a.starts_with(&p2) || a.starts_with(&p3)
                    })
                })
                .map(|sh| sh["id"].clone())
                .collect();
            if !ids.is_empty() {
                d.q("edit.delete", json!({"ids": ids}));
            }
            continue;
        }
        fill_sheet_slot(d, slide, &sheet[slot], slot)?;
    }
    Ok(())
}

fn scene_panels(
    d: &mut Deck,
    slide: &Value,
    page: &Value,
    c: &Value,
    record: &Value,
) -> Result<Value> {
    let background = c["locations"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|r| {
            r["id"]
                .as_str()
                .is_some_and(|id| id.to_lowercase().contains("backdrop"))
        })
        .ok_or_else(|| {
            invalid("Four chorus panels need an existing registered backdrop; no background is generated")
        })?;
    let mut panels = Vec::new();
    let gid = page["group"]["id"].as_str().unwrap_or("");
    for (i, line) in page["group"]["panels"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let image = if i == 0 || i == 3 { record } else { background };
        let rect = panel_region(i);
        let placed = place_scene_picture(
            d,
            slide,
            image,
            &format!("{gid}.panel.{}", i + 1),
            Some(rect),
        )?;
        let label = match i {
            0 => "START".to_owned(),
            3 => "END".to_owned(),
            _ => (i + 1).to_string(),
        };
        let line_id = line["line_id"].as_str().unwrap_or("");
        let start = clock(line["start"].as_f64().unwrap_or(0.0));
        let end = clock(line["end"].as_f64().unwrap_or(0.0));
        let text = line["text"].as_str().unwrap_or("");
        let role = format!("user.scene.panel.{}", i + 1);
        d.text(
            &format!("{label}  {line_id}  {start}–{end}  {text}"),
            [rect[0], rect[1] + 160.0, 464.0, 40.0],
            14.0,
            INK,
            false,
            Some(&role),
        );
        let mut item = line.clone();
        item["reference"] = placed;
        item["staging"] = json!(if i == 0 || i == 3 {
            "Original cast reference"
        } else {
            "Existing background; animated subjects pending"
        });
        panels.push(item);
    }
    Ok(json!({"panels": panels, "original": record["file"]}))
}

fn scene_picture(d: &mut Deck, slide: &Value, page: &Value, c: &Value) -> Result<Option<Value>> {
    if page["kind"] != "scene" {
        return Ok(None);
    }
    let gid = page["group"]["id"].as_str().unwrap_or("");
    let Some(record) = c["scene_images"].get(gid).filter(|v| v.is_object()) else {
        return Ok(None);
    };
    if page["group"]
        .get("panels")
        .and_then(Value::as_array)
        .is_some_and(|p| !p.is_empty())
    {
        let reference = scene_panels(d, slide, page, c, record)?;
        fill_text(d, slide, "state.scene.reference", "", None, false);
        return Ok(Some(reference));
    }
    Ok(Some(place_scene_picture(d, slide, record, gid, None)?))
}

fn page_meta(page: &Value, kind: &str, c: &Value, reference: Option<&Value>) -> Value {
    let mut meta = json!({
        "sb_generator": VERSION,
        "page_key": page["key"],
        "kind": kind,
        "status": "needs review",
        "group": page.get("group").cloned().unwrap_or(Value::Null),
        "source_transcript": c["transcript"],
        "source_captions": c.get("captions").cloned().unwrap_or(Value::Null),
        "sheet": page.get("sheet").cloned().unwrap_or(Value::Null),
    });
    if kind == "scene" {
        let gid = page["group"]["id"].as_str().unwrap_or("");
        meta["source_packet"] = c["scene_content"]
            .get(gid)
            .cloned()
            .unwrap_or_else(|| json!({}));
        meta["asset_links"] = c.get("asset_links").cloned().unwrap_or_else(|| json!([]));
        meta["official_lyrics_source"] = c
            .get("official_lyrics_source")
            .cloned()
            .unwrap_or(Value::Null);
    }
    if let Some(r) = reference {
        meta["reference_image"] = r.clone();
    }
    meta
}

#[allow(clippy::too_many_arguments)]
fn fill_page(
    d: &mut Deck,
    mut slide: Value,
    page: &Value,
    c: &Value,
    words: &[Value],
    groups: &[Value],
    duration: f64,
    all_pages: &[Value],
    representative: bool,
) -> Result<()> {
    let kind = page["kind"].as_str().unwrap_or("");
    let has_panels = kind == "scene"
        && page["group"]
            .get("panels")
            .and_then(Value::as_array)
            .is_some_and(|p| !p.is_empty());
    let compact = if has_panels { 140.0 } else { 0.0 };
    if compact > 0.0 {
        shrink_panels(d, &slide, compact)?;
        slide = d.cmd("slide.inspect", json!({"index": slide["index"]}))?;
    }
    let duration_label = if c["recording_duration_verified"] == true {
        "Measured recording"
    } else {
        "Supplied timing extent; recording duration unknown"
    };
    let page_title = page["title"].as_str().unwrap_or("");
    let doc_title = c["title"].as_str().unwrap_or("");
    fill_text(
        d,
        &slide,
        "auto.title",
        &format!("{doc_title} / {page_title}"),
        Some(36.0),
        false,
    );
    fill_text(
        d,
        &slide,
        "auto.subtitle",
        &format!("{} / {page_title}", clock(duration)),
        Some(18.0),
        false,
    );
    contents_nav(d, &slide, representative, kind, all_pages);
    match kind {
        "toc" => fill_toc(d, &slide, page, all_pages),
        "overview" => fill_overview(d, &slide, c, words, groups, duration, duration_label),
        "scene" => fill_scene(d, &slide, &page["group"], c, compact, all_pages),
        "assets" | "locations" => fill_sheet(d, &slide, page)?,
        _ => {}
    }
    let reference = scene_picture(d, &slide, page, c)?;
    let key = page["key"].as_str().unwrap_or("");
    for field in c["page_content"][key].as_array().into_iter().flatten() {
        let role = field["role"].as_str().unwrap_or("");
        let val = field["value"].as_str().unwrap_or("");
        fill_text(d, &slide, role, val, field["font_size"].as_f64(), true);
    }
    let meta = page_meta(page, kind, c, reference.as_ref());
    let notes_str = serde_json::to_string_pretty(&meta).map_err(|e| invalid(e.to_string()))?;
    d.q("slide.notes", json!({"text": notes_str}));
    d.q("slide.rename", json!({"name": key}));
    d.flush()
}

fn slide_cards(slide: &Value) -> BTreeMap<String, [f64; 4]> {
    slide["shapes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| {
            let role = x["altText"].as_str()?.strip_prefix("layout.card.")?;
            Some((role.to_owned(), box_coords(&x["box"])?))
        })
        .collect()
}

fn shape_overflow(index: usize, shape: &Value, cards: &BTreeMap<String, [f64; 4]>) -> Vec<Value> {
    let Some([x, y, w, h]) = box_coords(&shape["box"]) else {
        return Vec::new();
    };
    let role = shape["altText"].as_str().unwrap_or("");
    let mut out = Vec::new();
    if x < -0.1 || y < -0.1 || x + w > SIZE[0] + 1.0 || y + h > SIZE[1] + 1.0 {
        out.push(json!({"page": index + 1, "role": role, "box": shape["box"]}));
    }
    if let Some(card_key) = role.strip_prefix("user.")
        && let Some(card) = cards.get(card_key)
        && y + h > card[1] + card[3] - 12.0
    {
        out.push(json!({
            "page": index + 1,
            "role": role,
            "reason": "Text collides with its card status; shorten or enlarge the editable card",
            "box": shape["box"],
        }));
    }
    if role == "auto.scene.lyrics" && y + h > 940.0 {
        out.push(json!({
            "page": index + 1,
            "role": role,
            "reason": "Lyric block exceeds its allocated area",
            "box": shape["box"],
        }));
    }
    out
}

fn link_target(role: &str, page: &Value, pages: &[Value], representative: bool) -> Option<usize> {
    if role == "nav.contents" && !representative && page["kind"] != "toc" {
        return pages.iter().position(|p| p["kind"] == "toc");
    }
    if let Some(slot_str) = role.strip_prefix("auto.toc.label.") {
        let toc_num: usize = page["key"].as_str()?.split(':').nth(1)?.parse().ok()?;
        let slot: usize = slot_str.parse().ok()?;
        let at = (toc_num - 1) * TOC_PAGE_SIZE + slot;
        let entries = toc_entries(pages);
        return entries.get(at).map(|(idx, _)| *idx);
    }
    if matches!(role, "nav.asset:" | "nav.location:") {
        let prefix = &role[4..];
        return pages
            .iter()
            .position(|p| p["key"].as_str().is_some_and(|k| k.starts_with(prefix)));
    }
    None
}

fn check_native_link(d: &mut Deck, shape: &Value, target: usize, slides: &[Value]) -> Result<()> {
    let full = d.cmd("shape.inspect", json!({"id": shape["id"]}))?;
    let action = &full["shape"]["click"]["action"];
    let expected = json!({"kind": "slide", "slide": slides[target]["id"]});
    if action != &expected {
        return Err(invalid("Native TOC target mismatch"));
    }
    Ok(())
}

fn check_source_link(d: &mut Deck, shape: &Value) -> Result<()> {
    let full = d.cmd("shape.inspect", json!({"id": shape["id"]}))?;
    let action = &full["shape"]["click"]["action"];
    let url = action["url"].as_str().unwrap_or("");
    let resolves = action["kind"] == "url" && file_uri_to_path(url).is_some_and(|p| p.is_file());
    if !resolves {
        return Err(invalid("Source file link does not resolve"));
    }
    Ok(())
}

fn page_links(
    d: &mut Deck,
    slide: &Value,
    page: &Value,
    index: usize,
    pages: &[Value],
    slides: &[Value],
    representative: bool,
) -> Result<(usize, Vec<Value>, usize)> {
    let (mut links, mut regions, mut files) = (0, Vec::new(), 0);
    for shape in slide["shapes"].as_array().into_iter().flatten() {
        let role = shape["altText"].as_str().unwrap_or("");
        if let Some(target) = link_target(role, page, pages, representative) {
            check_native_link(d, shape, target, slides)?;
            links += 1;
            if let Some([x, y, w, h]) = box_coords(&shape["box"]) {
                regions.push(
                    json!({"page": index + 1, "to_page": target + 1, "rect": [x, y, x + w, y + h]}),
                );
            }
        }
        if role.starts_with("source.") {
            check_source_link(d, shape)?;
            files += 1;
        }
    }
    Ok((links, regions, files))
}

fn check_structure(
    pages: &[Value],
    words: &[Value],
    empty_template: bool,
    representative: bool,
    actual_cues: &[Value],
) -> Result<()> {
    if representative {
        return Ok(());
    }
    let expected_cues: Vec<Value> = words.iter().map(|w| w["id"].clone()).collect();
    if actual_cues != expected_cues {
        return Err(invalid("Original word occurrence lost from scenes"));
    }
    let kinds: BTreeSet<&str> = pages.iter().filter_map(|x| x["kind"].as_str()).collect();
    for req in ["toc", "overview", "assets", "locations"] {
        if !kinds.contains(req) {
            return Err(invalid("Missing required storyboard page kind"));
        }
    }
    if !empty_template && !kinds.contains("scene") {
        return Err(invalid("Missing scene page kind"));
    }
    if kinds.contains("lyrics") || kinds.contains("qc") {
        return Err(invalid("Obsolete page kind in storyboard"));
    }
    Ok(())
}

fn validate_deck(
    d: &mut Deck,
    pages: &[Value],
    words: &[Value],
    groups: &[Value],
    duration: f64,
    representative: bool,
    empty_template: bool,
) -> Result<Value> {
    let slides = inspect_pages(d)?;
    if slides.len() != pages.len() {
        return Err(invalid("Page count mismatch"));
    }
    let keys: Vec<Option<String>> = slides
        .iter()
        .map(|s| note(s)["page_key"].as_str().map(str::to_owned))
        .collect();
    let expected_keys: Vec<Option<String>> = pages
        .iter()
        .map(|p| p["key"].as_str().map(str::to_owned))
        .collect();
    if keys != expected_keys {
        return Err(invalid("Page mapping mismatch"));
    }
    let (mut overflow, mut link_regions, mut actual_cues) = (Vec::new(), Vec::new(), Vec::new());
    let (mut links, mut file_links, mut creative_instructions) = (0, 0, 0);
    for (i, (sl, page)) in slides.iter().zip(pages.iter()).enumerate() {
        d.cmd("slide.go", json!({"index": i}))?;
        let cards = slide_cards(sl);
        for sh in sl["shapes"].as_array().into_iter().flatten() {
            overflow.extend(shape_overflow(i, sh, &cards));
            if sh["altText"]
                .as_str()
                .is_some_and(|a| a.starts_with("user."))
            {
                creative_instructions += 1;
            }
        }
        let (fl, reg, files) = page_links(d, sl, page, i, pages, &slides, representative)?;
        links += fl;
        link_regions.extend(reg);
        file_links += files;
        if page["kind"] == "scene"
            && let Some(wids) = page["group"]["word_ids"].as_array()
        {
            actual_cues.extend(wids.iter().cloned());
        }
    }
    check_structure(pages, words, empty_template, representative, &actual_cues)?;
    if !empty_template {
        validate_groups(groups, words, duration)?;
    }
    let page_summary: Vec<Value> = pages
        .iter()
        .zip(slides.iter())
        .map(|(p, s)| json!({"key": p["key"], "id": s["id"], "kind": p["kind"]}))
        .collect();
    Ok(json!({
        "ok": overflow.is_empty(), "slides": slides.len(), "native_links": links,
        "link_regions": link_regions, "resolving_source_links": file_links,
        "caption_occurrences": actual_cues.len(), "source_words": words.len(),
        "scene_spans": groups.len(), "duration": duration, "source_sequence_preserved": true,
        "provisional_grouping": true, "overflow": overflow,
        "editable_fields": creative_instructions, "approval": "needs review", "pages": page_summary,
    }))
}

fn check_existing(output: &Path, state: &Path, fingerprint: &str) -> Result<()> {
    if output.exists() && !state.exists() {
        return Err(invalid(
            "Existing deck has no generator state; refusing to overwrite authored work",
        ));
    }
    if state.exists() {
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(state)?)
            .map_err(|e| invalid(e.to_string()))?;
        if doc["fingerprint"].as_str() != Some(fingerprint) {
            return Err(invalid(
                "Source/grouping changed. Generate to a new output revision; existing authored work is preserved.",
            ));
        }
    }
    Ok(())
}

fn stage_template(template: &Path, c: &Value, output: &Path) -> Result<(PathBuf, PathBuf)> {
    let read_root = c
        .get("project")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .and_then(|p| Path::new(p).canonicalize().ok())
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .or_else(|| template.parent().map(Path::to_path_buf))
        .ok_or_else(|| invalid("Cannot resolve template read root"))?;
    if template.starts_with(&read_root) {
        return Ok((template.to_path_buf(), read_root));
    }
    let out_parent = output
        .parent()
        .ok_or_else(|| invalid("Output has no parent directory"))?;
    let runtime = out_parent.join("source-template.deckcraft");
    if !runtime.exists() {
        std::fs::copy(template, &runtime)?;
    }
    if sha(&runtime)? != sha(template)? {
        return Err(invalid("Staged template differs from canonical template"));
    }
    Ok((runtime, read_root))
}

fn open_existing(d: &mut Deck, output: &Path) -> Result<Option<Vec<Value>>> {
    if !output.exists() {
        return Ok(None);
    }
    d.cmd("file.open", json!({"path": output}))?;
    Ok(Some(snapshot(d)?))
}

fn duplicate_template_slides(d: &mut Deck, pages: &[Value], originals: &[Value]) -> Result<()> {
    let sources: BTreeMap<String, Value> = originals
        .iter()
        .filter_map(|s| Some((note(s)["kind"].as_str()?.to_owned(), s["id"].clone())))
        .collect();
    for page in pages {
        let kind = page["kind"].as_str().unwrap_or("");
        let source_id = sources
            .get(kind)
            .ok_or_else(|| invalid(format!("Template missing slide kind: {kind}")))?;
        d.cmd("slide.go", json!({"id": source_id}))?;
        d.cmd("slide.duplicate", json!({}))?;
        let info = d.cmd("document.inspect", json!({}))?;
        let active = d.cmd("slide.inspect", json!({}))?;
        let total = info["slides"].as_array().map_or(1, Vec::len);
        d.cmd(
            "slide.move",
            json!({"from": active["index"], "to": total.saturating_sub(1)}),
        )?;
    }
    for i in (0..originals.len()).rev() {
        d.cmd("slide.delete", json!({"index": i}))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_pages(
    d: &mut Deck,
    runtime_template: &Path,
    pages: &[Value],
    c: &Value,
    words: &[Value],
    groups: &[Value],
    duration: f64,
    representative: bool,
) -> Result<()> {
    d.cmd("file.open", json!({"path": runtime_template}))?;
    let originals = inspect_pages(d)?;
    duplicate_template_slides(d, pages, &originals)?;
    for (i, page) in pages.iter().enumerate() {
        d.cmd("slide.go", json!({"index": i}))?;
        let slide = d.cmd("slide.inspect", json!({"index": i}))?;
        fill_page(
            d,
            slide,
            page,
            c,
            words,
            groups,
            duration,
            pages,
            representative,
        )?;
    }
    Ok(())
}

fn commit(
    d: &mut Deck,
    output: &Path,
    state: &Path,
    payload: &Value,
    previous_hash: Option<&str>,
) -> Result<()> {
    let parent = output
        .parent()
        .ok_or_else(|| invalid("Output has no parent directory"))?;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("movie-board");
    if let Some(prev) = previous_hash {
        let history = parent.join("history");
        std::fs::create_dir_all(&history)?;
        let short = &prev[..prev.len().min(12)];
        let ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("deckcraft");
        let backup = history.join(format!("{stem}-{short}.{ext}"));
        if !backup.exists() {
            std::fs::copy(output, &backup)?;
        }
    }
    let pending = output.with_file_name(format!("{stem}.pending.deckcraft"));
    d.cmd("file.saveAs", json!({"path": pending}))?;
    std::fs::rename(&pending, output)?;
    dump(state, payload)
}

fn export_pages(
    d: &mut Deck,
    output: &Path,
    pages: &[Value],
    representative: bool,
    fresh: bool,
) -> Result<()> {
    let pdf = output.with_extension("pdf");
    if fresh || !pdf.is_file() {
        d.cmd("file.export", json!({"path": pdf, "format": "pdf"}))?;
    }
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("movie-board");
    let scene = pages.iter().position(|p| p["kind"] == "scene").unwrap_or(0);
    d.cmd("file.export", json!({"path": output.with_file_name(format!("{stem}-scene.png")), "format": "png", "slide": scene, "scale": 1}))?;
    if representative {
        return Ok(());
    }
    for kind in ["toc", "overview", "assets", "locations"] {
        if let Some(i) = pages.iter().position(|p| p["kind"] == kind) {
            d.cmd("file.export", json!({"path": output.with_file_name(format!("{stem}-{kind}.png")), "format": "png", "slide": i, "scale": 1}))?;
        }
    }
    Ok(())
}

struct LockGuard(PathBuf);
impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn verify_existing_invariants(
    d: &mut Deck,
    output: &Path,
    before: Option<&[Value]>,
    previous_hash: Option<&str>,
) -> Result<()> {
    let Some(prev_snap) = before else {
        return Ok(());
    };
    if snapshot(d)?.as_slice() != prev_snap {
        return Err(invalid("Regeneration altered authored objects"));
    }
    if Some(sha(output)?.as_str()) != previous_hash {
        return Err(invalid(
            "Concurrent disk edit detected; refusing to replace current deck",
        ));
    }
    Ok(())
}

fn prepare_output_path(output: &Path) -> Result<(PathBuf, PathBuf, LockGuard)> {
    let out_parent = output
        .parent()
        .ok_or_else(|| invalid("Output has no parent directory"))?;
    std::fs::create_dir_all(out_parent)?;
    let canonical = out_parent.canonicalize()?.join(
        output
            .file_name()
            .ok_or_else(|| invalid("Invalid output file"))?,
    );
    let state = canonical.with_extension("state.json");
    let lock = canonical.with_extension("lock");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)?;
    Ok((canonical, state, LockGuard(lock)))
}

fn build_result_summary(
    output: &Path,
    pages: Vec<Value>,
    reopened_existing: bool,
    validation: Value,
) -> Value {
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("movie-board");
    json!({
        "ok": true, "page_plan": pages, "deck": output,
        "pdf": output.with_extension("pdf"), "proof": output.with_file_name(format!("{stem}-scene.png")),
        "reopened_existing": reopened_existing, "validation": validation,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_and_verify(
    d: &mut Deck,
    output: &Path,
    pages: &[Value],
    words: &[Value],
    groups: &[Value],
    duration: f64,
    representative: bool,
    empty_template: bool,
    before: Option<&[Value]>,
    previous_hash: Option<&str>,
) -> Result<Value> {
    let validation = validate_deck(
        d,
        pages,
        words,
        groups,
        duration,
        representative,
        empty_template,
    )?;
    if validation["ok"] != true {
        return Err(invalid(format!(
            "Layout overflow: {}",
            validation["overflow"]
        )));
    }
    verify_existing_invariants(d, output, before, previous_hash)?;
    Ok(validation)
}

#[allow(clippy::too_many_arguments)]
fn write_deck_artifacts(
    d: &mut Deck,
    output: &Path,
    state: &Path,
    template: &Path,
    config: &Path,
    fingerprint: &str,
    pages: &[Value],
    groups: &[Value],
    representative: bool,
    reopened_existing: bool,
    previous_hash: Option<&str>,
    mut validation: Value,
) -> Result<Value> {
    let state_payload = json!({
        "version": VERSION, "fingerprint": fingerprint, "template": template,
        "template_sha256": sha(template)?, "source_config": config.canonicalize().unwrap_or_else(|_| config.to_path_buf()),
        "representative": representative,
    });
    commit(d, output, state, &state_payload, previous_hash)?;
    dump(
        &output.with_extension("grouping.json"),
        &json!({"status": "provisional / editable", "groups": groups}),
    )?;
    export_pages(d, output, pages, representative, !reopened_existing)?;
    validation["authored_edits_preserved"] = json!(reopened_existing);
    dump(&output.with_extension("validation.json"), &validation)?;
    Ok(validation)
}

#[allow(clippy::too_many_arguments)]
fn open_or_build_deck(
    template: &Path,
    c: &Value,
    output: &Path,
    pages: &[Value],
    words: &[Value],
    groups: &[Value],
    duration: f64,
    representative: bool,
) -> Result<(Deck, Option<Vec<Value>>)> {
    let (runtime_template, read_root) = stage_template(template, c, output)?;
    let parent_dir = output.parent().unwrap_or_else(|| Path::new("."));
    let mut d = Deck::new(&read_root, parent_dir)?;
    let before = open_existing(&mut d, output)?;
    if before.is_none() {
        build_pages(
            &mut d,
            &runtime_template,
            pages,
            c,
            words,
            groups,
            duration,
            representative,
        )?;
    }
    Ok((d, before))
}

pub(crate) fn generate(
    template: &Path,
    config: &Path,
    output: &Path,
    grouping: Option<&Path>,
    representative: bool,
) -> Result<Value> {
    let template = template.canonicalize()?;
    let (output, state, _lock_guard) = prepare_output_path(output)?;
    let (c, words, _cues, groups, duration) = read_inputs(config, grouping)?;
    let fingerprint = source_fingerprint(&c, &groups)?;
    let pages = build_plan(&c, &groups, representative);
    let previous_hash = output.exists().then(|| sha(&output)).transpose()?;
    check_existing(&output, &state, &fingerprint)?;
    let (mut d, before) = open_or_build_deck(
        &template,
        &c,
        &output,
        &pages,
        &words,
        &groups,
        duration,
        representative,
    )?;
    let validation = validate_and_verify(
        &mut d,
        &output,
        &pages,
        &words,
        &groups,
        duration,
        representative,
        c["empty_template"] == true,
        before.as_deref(),
        previous_hash.as_deref(),
    )?;
    let validation = write_deck_artifacts(
        &mut d,
        &output,
        &state,
        &template,
        config,
        &fingerprint,
        &pages,
        &groups,
        representative,
        before.is_some(),
        previous_hash.as_deref(),
        validation,
    )?;
    Ok(build_result_summary(
        &output,
        pages,
        before.is_some(),
        validation,
    ))
}
