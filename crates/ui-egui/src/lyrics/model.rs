//! The data `words.read` returns, typed for the Lyrics view.
//!
//! Every type keeps the fields it does not name in `extra`, so a design read here and saved
//! back with `words.save` carries everything the engine wrote, not only what this view edits.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WordsRead {
    pub document: Document,
    #[serde(default)]
    pub fonts: Vec<SongFont>,
    #[serde(default)]
    pub images: Vec<Backdrop>,
    /// The timing reference changed since the document was saved: words may have moved.
    #[serde(default)]
    pub reference_changed: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Document {
    #[serde(default)]
    pub animations: Vec<Animation>,
}

/// One designed section of the song (a verse or chorus occurrence).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Animation {
    pub animation_id: String,
    #[serde(default)]
    pub source_revision: u64,
    #[serde(default)]
    pub background_asset: Option<String>,
    /// Saved line designs by line id; `null` means "no design yet".
    #[serde(default)]
    pub design: BTreeMap<String, Option<LineDesign>>,
    #[serde(default)]
    pub words: Vec<Word>,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub kinetic: Kinetic,
    #[serde(default)]
    pub style: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Scope {
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub occurrence: Option<u32>,
}

/// The section's kinetic choices: which act each meaning word performs, and the lettering.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Kinetic {
    #[serde(default)]
    pub acts: BTreeMap<String, String>,
    #[serde(default)]
    pub style: Map<String, Value>,
}

/// One sung word. Its text and timing come from the artist's sheet and the timing reference;
/// this view reads them and never changes them.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Word {
    pub word_id: String,
    pub line_id: String,
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

/// How one sung line looks: lettered items and hand-drawn strokes.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LineDesign {
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub draws: Vec<Draw>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A group of words lettered together at one place on the 1920×1080 stage.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Item {
    #[serde(default)]
    pub word_ids: Vec<String>,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default = "default_size")]
    pub size: f32,
    /// `song`, `serif` or `sans`.
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub italic: bool,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<String>,
    /// Degrees.
    #[serde(default)]
    pub rotate: f32,
    /// `quiet`, `pop`, `settle`, `fall`, `rise`, `east`, `west`, `loop`, `sleep`.
    #[serde(default = "default_motion")]
    pub motion: String,
    /// The word whose time starts this item's motion.
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A hand-drawn stroke group, placed in stage pixels and drawn on over its paths' from/to.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Draw {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub w: f32,
    #[serde(default)]
    pub h: f32,
    #[serde(default)]
    pub view_box: String,
    #[serde(default)]
    pub paths: Vec<DrawPath>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DrawPath {
    pub d: String,
    /// Line-local seconds when the stroke starts and finishes drawing on.
    #[serde(default)]
    pub from: f32,
    #[serde(default)]
    pub to: f32,
    #[serde(default = "default_stroke")]
    pub stroke: String,
    #[serde(default = "default_stroke_width")]
    pub width: f32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `words.studio_track`: the video's lyric track. Where it has a clip for a word, the clip's
/// start is the word's time, and a section's span is the track section's span.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct StudioTrack {
    #[serde(default)]
    pub sections: Vec<TrackSection>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackSection {
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub occurrence: Option<u32>,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
    #[serde(default)]
    pub clips: Vec<TrackClip>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackClip {
    pub word_id: String,
    pub start: f64,
}

impl StudioTrack {
    pub fn section_of(&self, anim: &Animation) -> Option<&TrackSection> {
        self.sections
            .iter()
            .find(|s| s.section == anim.scope.section && s.occurrence == anim.scope.occurrence)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SongFont {
    pub family: String,
    pub path: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Backdrop {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub path: String,
}

fn default_size() -> f32 {
    58.0
}
fn default_font() -> String {
    "song".into()
}
fn default_weight() -> u32 {
    600
}
fn default_color() -> String {
    "#ffffff".into()
}
fn default_motion() -> String {
    "quiet".into()
}
fn default_stroke() -> String {
    "#b8442a".into()
}
fn default_stroke_width() -> f32 {
    6.0
}

impl Animation {
    /// "Verse 1", "Chorus 2" … from the section scope, as the web UI labels it: the section name
    /// with underscores as spaces, plus the occurrence for a chorus or a repeated section.
    pub fn label(&self) -> String {
        let section = self.scope.section.as_deref().unwrap_or("lyrics");
        let mut name: String = section.replace('_', " ");
        if let Some(first) = name.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        match self.scope.occurrence {
            Some(n) if section == "chorus" || n > 1 => format!("{name} {n}"),
            _ => name,
        }
    }

    /// The section's chorus/verse kind, for colouring its cards.
    pub fn is_chorus(&self) -> bool {
        self.scope.section.as_deref().is_some_and(|s| s.starts_with("chorus"))
    }

    pub fn hero_color(&self) -> Option<&str> {
        self.kinetic.style.get("hero_color").and_then(Value::as_str)
    }

    pub fn base_color(&self) -> Option<&str> {
        self.style.get("color").and_then(Value::as_str)
    }
}
