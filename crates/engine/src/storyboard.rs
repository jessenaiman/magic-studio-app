//! The storyboard is a DeckCraft deck. `storyboard.create` fills the template from a scene plan
//! and the project's transcription file; `storyboard.render` draws its pages for the Storyboard
//! view. DeckCraft owns the typography and style: Magic only sets the template's own fields and
//! inserts pictures. DeckCraft owns saving and closing its hand-editing window.
use crate::{EngineError, Result, Session};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

// ---- The native engine, the template and the project files this file uses ----------------------------

/// Deck operations use the published DeckCraft Rust session and renderer.
/// Pages: 1 overview, 2 sequence, 3 the scene page (duplicated once per scene), last handoff.
const TEMPLATE: &[u8] = include_bytes!("../../../references/storyboard/Storyboard.potx");
const SCENE_PAGE: usize = 3;
/// In the project folder.
const DECK: &str = "storyboard/storyboard.deckcraft";
const PAGES: &str = "storyboard/pages";
const MANIFEST: &str = "storyboard/movie-board.json";

// ---- Deck operations ------------------------------------------------------------------------

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::InvalidParams(message.into())
}

fn open_deck(path: &Path) -> Result<deckcraft_engine::Session> {
    let path = std::fs::canonicalize(path)?;
    let mut session = deckcraft_engine::Session::new();
    session
        .execute("file.open", &json!({"path": path}))
        .map_err(|e| bad(format!("DeckCraft open: {e}")))?;
    Ok(session)
}

/// Run DeckCraft commands on `input`: save the result to `save`, or return each command's answer.
fn run(
    _dir: &Path,
    input: &Path,
    commands: &[(&str, Value)],
    save: Option<&Path>,
) -> Result<Vec<Value>> {
    let mut session = open_deck(input)?;
    let mut results = Vec::with_capacity(commands.len());
    for (id, params) in commands {
        results.push(
            session
                .execute(id, params)
                .map_err(|e| bad(format!("DeckCraft {id}: {e}")))?,
        );
    }
    if let Some(path) = save {
        session
            .execute("file.saveAs", &json!({"path": path}))
            .map_err(|e| bad(format!("DeckCraft save: {e}")))?;
    }
    Ok(results)
}

/// The named shapes on `page` (from 1).
fn slots(dir: &Path, deck: &Path, page: usize) -> Result<Map<String, Value>> {
    let inspected = run(
        dir,
        deck,
        &[("slide.inspect", json!({"index": page - 1}))],
        None,
    )?;
    Ok(inspected
        .first()
        .and_then(|p| p["shapes"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|s| Some((s["name"].as_str()?.to_owned(), s.clone())))
        .collect())
}

/// Commands that set `page`'s text fields and place its pictures (paths from `root`).
fn fill(
    dir: &Path,
    deck: &Path,
    page: usize,
    card: &Value,
    root: &Path,
) -> Result<Vec<(&'static str, Value)>> {
    let slots = slots(dir, deck, page)?;
    let mut ops = vec![("slide.go", json!({"index": page - 1}))];
    for (name, text) in card["fields"].as_object().into_iter().flatten() {
        let slot = slots
            .get(name)
            .filter(|s| s["kind"] == "shape")
            .ok_or_else(|| bad(format!("Page {page} has no text field {name}")))?;
        ops.push(("text.set", json!({"id": slot["id"], "text": text})));
    }
    for (name, image) in card["images"].as_object().into_iter().flatten() {
        let slot = slots
            .get(name)
            .ok_or_else(|| bad(format!("Page {page} has no picture {name}")))?;
        let path = root.join(image.as_str().unwrap_or_default());
        ops.push((
            "insert.picture",
            json!({"name": format!("{name} artwork"), "rect": slot["box"], "path": path}),
        ));
    }
    Ok(ops)
}

fn project_dir(s: &Session) -> Result<PathBuf> {
    crate::workshop::project_dir(s)
}

// ---- Commands -------------------------------------------------------------------------------

/// The project's transcription file as timed lines. It was checked once against the artist's
/// lyrics when it was added and is used as it is: no further spelling or wording changes.
fn transcript_lines(dir: &Path) -> Result<Vec<Value>> {
    let file = crate::workshop::transcript_files(dir)
        .into_iter()
        .next()
        .ok_or_else(|| bad("No transcription file; add or transcribe the song first"))?;
    let doc = crate::workshop::transcript_document(&file)?;
    Ok(doc["paragraphs"].as_array().cloned().unwrap_or_default())
}

/// `storyboard.create plan=<scene plan .json>`: a new deck from the template. The plan fills the
/// overview, sequence and handoff cards and one card per scene; scenes cover every transcript
/// line once, in order (`"lines": [first, last]`, from 1).
pub fn cmd_storyboard_create(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = project_dir(s)?;
    let deck = dir.join(DECK);
    if deck.exists() {
        return Err(bad(format!(
            "{} exists; edit it in DeckCraft",
            deck.display()
        )));
    }
    let plan_path = PathBuf::from(
        p["plan"]
            .as_str()
            .ok_or_else(|| bad("plan=<scene plan .json> is required"))?,
    );
    let plan: Value = serde_json::from_slice(&std::fs::read(&plan_path)?)
        .map_err(|e| bad(format!("{}: {e}", plan_path.display())))?;
    let root = plan_path.parent().unwrap_or(&dir).to_path_buf();
    let lines = transcript_lines(&dir)?;
    let scenes = plan["scenes"].as_array().cloned().unwrap_or_default();
    if scenes.is_empty() {
        return Err(bad("The plan needs at least one scene"));
    }
    std::fs::create_dir_all(dir.join("storyboard"))?;
    let template =
        std::env::temp_dir().join(format!("magic-storyboard-{}.potx", std::process::id()));
    std::fs::write(&template, TEMPLATE)?;
    let layout = dir.join("storyboard/.layout.deckcraft");
    let copies: Vec<(&str, Value)> = (1..scenes.len())
        .map(|_| ("slide.duplicate", json!({"index": SCENE_PAGE - 1})))
        .collect();
    let made = run(&dir, &template, &copies, Some(&layout));
    let _ = std::fs::remove_file(&template);
    made?;
    let handoff = SCENE_PAGE + scenes.len();
    let mut ops = Vec::new();
    for (page, key) in [(1, "overview"), (2, "sequence"), (handoff, "handoff")] {
        ops.extend(fill(&dir, &layout, page, &plan[key], &root)?);
    }
    let (mut next, mut cues) = (1, vec![Vec::new(), Vec::new()]);
    for (i, scene) in scenes.iter().enumerate() {
        let (first, last) = (
            scene["lines"][0].as_u64().unwrap_or(0) as usize,
            scene["lines"][1].as_u64().unwrap_or(0) as usize,
        );
        if first != next || last < first || last > lines.len() {
            return Err(bad(format!(
                "Scene {}: lines must continue at {next}, within 1–{}",
                i + 1,
                lines.len()
            )));
        }
        next = last + 1;
        let said = &lines[first - 1..last];
        let mut card = scene.clone();
        let fields = card["fields"]
            .as_object_mut()
            .ok_or_else(|| bad(format!("Scene {}: fields", i + 1)))?;
        fields.insert("scene_id".into(), json!(format!("S{:02}", i + 1)));
        fields.insert(
            "cue".into(),
            json!(
                said.iter()
                    .filter_map(|l| l["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        );
        if let (Some(start), Some(end)) = (
            said[0]["start"].as_f64(),
            said[said.len() - 1]["end"].as_f64(),
        ) {
            fields
                .entry("time")
                .or_insert_with(|| json!(format!("{start:.2}–{end:.2} s")));
        }
        cues.push(said.iter().map(|l| json!({"text": l["text"]})).collect());
        ops.extend(fill(&dir, &layout, SCENE_PAGE + i, &card, &root)?);
    }
    if next != lines.len() + 1 {
        return Err(bad("The scenes must cover every transcript line"));
    }
    cues.push(Vec::new());
    let saved = run(&dir, &layout, &ops, Some(&deck));
    let _ = std::fs::remove_file(&layout);
    saved?;
    render(&dir, Some(json!(cues)))
}

/// `storyboard.render`: draw the deck's pages for the Storyboard view (after editing in DeckCraft).
pub fn cmd_storyboard_render(s: &mut Session, _: &Value) -> Result<Value> {
    render(&project_dir(s)?, None)
}

fn render(dir: &Path, cues: Option<Value>) -> Result<Value> {
    let deck = dir.join(DECK);
    if !deck.is_file() {
        return Err(bad("No storyboard yet; run storyboard.create"));
    }
    let pages = dir.join(PAGES);
    let _ = std::fs::remove_dir_all(&pages);
    std::fs::create_dir_all(&pages)?;
    let mut session = open_deck(&deck)?;
    let document = &session.doc().map_err(|e| bad(e.to_string()))?.doc;
    for index in 0..document.slides.len() {
        let (png, _, _) =
            deckcraft_engine::cmd::file::render_png_with(document, index, 2.0, false, false);
        std::fs::write(pages.join(format!("slide-{:02}.png", index + 1)), png)?;
    }
    let mut previews: Vec<PathBuf> = std::fs::read_dir(&pages)?
        .flatten()
        .map(|e| e.path())
        .collect();
    previews.sort();
    let info = session
        .execute("document.inspect", &json!({}))
        .map_err(|e| bad(format!("DeckCraft inspect: {e}")))?;
    let inspect: Vec<(&str, Value)> = (0..previews.len())
        .map(|i| ("slide.inspect", json!({"index": i})))
        .collect();
    let titles: Vec<Value> = run(dir, &deck, &inspect, None)?
        .iter()
        .map(|page| {
            let named = |n: &str| {
                page["shapes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|s| s["name"] == n)
                    .and_then(|s| s["text"].as_str())
            };
            json!(match (named("scene_id"), named("title")) {
                (Some(id), Some(title)) => format!("{id} · {title}"),
                (_, title) => title.unwrap_or_default().to_owned(),
            })
        })
        .collect();
    let manifest = dir.join(MANIFEST);
    let earlier: Value = std::fs::read(&manifest)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let board = json!({"deck": DECK, "previews": previews, "page_titles": titles,
        "page_size": info["size"], "page_cues": cues.unwrap_or_else(|| earlier["page_cues"].clone())});
    std::fs::write(
        &manifest,
        serde_json::to_vec_pretty(&board).unwrap_or_default(),
    )?;
    Ok(json!({"ok": true, "deck": deck, "pages": previews.len(), "manifest": manifest}))
}
