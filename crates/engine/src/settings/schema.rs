//! Settings schema copied from FilmCraft `engine/src/settings/schema.rs`, refactored for Magic.
//! One schema drives UI, CLI, MCP and --control: fields() is the single list.

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    Int { min: f64, max: f64, unit: &'static str },
    Float { min: f64, max: f64, decimals: usize, unit: &'static str },
    Choice(&'static [(&'static str, &'static str)]),
    Text,
    Path,
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub wired: bool,
    pub enabled_by: Option<&'static str>,
}

pub struct Category {
    pub id: &'static str,
    pub title: &'static str,
}

pub const CATEGORIES: &[Category] = &[
    Category { id: "general", title: "General" },
    Category { id: "project", title: "Project" },
    Category { id: "image", title: "Image / Scribbles" },
    Category { id: "lyrics", title: "Lyrics" },
    Category { id: "animation", title: "Animation" },
    Category { id: "appearance", title: "Appearance" },
    Category { id: "autosave", title: "AutoSave" },
    Category { id: "mediaCache", title: "Media Cache" },
    Category { id: "playback", title: "Playback" },
    Category { id: "timeline", title: "Timeline" },
    Category { id: "audio", title: "Audio" },
    Category { id: "media", title: "Media" },
    Category { id: "labels", title: "Labels" },
];

pub fn fields() -> &'static [(Category, Field)] {
    // Category is not Copy-friendly in slice; use index via consts below.
    // This fn is replaced by fields_with_cat() - kept for API compat.
    &[]
}

pub fn fields_with_cat() -> Vec<(&'static str, Field)> {
    vec![
        ("project", Field { key: "project.root", label: "Production project root", kind: Kind::Path, wired: true, enabled_by: None }),
        ("project", Field { key: "project.characters_root", label: "Characters root", kind: Kind::Path, wired: true, enabled_by: None }),
        (
            "image",
            Field { key: "image.provider", label: "Image provider", kind: Kind::Choice(&[("codex", "Codex (subscription)")]), wired: true, enabled_by: None },
        ),
        ("image", Field { key: "scribbles.template", label: "Scribbles template", kind: Kind::Text, wired: true, enabled_by: None }),
        (
            "image",
            Field {
                key: "scribbles.provider",
                label: "Scribbles provider",
                kind: Kind::Choice(&[("codex", "Codex (subscription)")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("image", Field { key: "scribbles.model", label: "Scribbles model", kind: Kind::Text, wired: true, enabled_by: None }),
        (
            "lyrics",
            Field {
                key: "lyrics.default_size",
                label: "Default lyric size",
                kind: Kind::Int { min: 8.0, max: 600.0, unit: "px" },
                wired: true,
                enabled_by: None,
            },
        ),
        ("lyrics", Field { key: "lyrics.outline", label: "Lyric outline", kind: Kind::Bool, wired: true, enabled_by: None }),
        ("animation", Field { key: "animation.loop_preview", label: "Loop preview", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "animation",
            Field {
                key: "animation.duration",
                label: "Default clip duration",
                kind: Kind::Float { min: 0.1, max: 30.0, decimals: 1, unit: "s" },
                wired: true,
                enabled_by: None,
            },
        ),
        (
            "appearance",
            Field {
                key: "appearance.theme",
                label: "Theme",
                kind: Kind::Choice(&[("system", "System"), ("dark", "Dark"), ("light", "Light")]),
                wired: false,
                enabled_by: None,
            },
        ),
        (
            "general",
            Field {
                key: "general.at_startup",
                label: "At Startup",
                kind: Kind::Choice(&[("showHome", "Show Home"), ("openMostRecent", "Open Most Recent"), ("emptyProject", "Empty Project")]),
                wired: true,
                enabled_by: None,
            },
        ),
        (
            "general",
            Field {
                key: "general.when_opening_project",
                label: "When Opening a Project",
                kind: Kind::Choice(&[("showOpenDialog", "Show Open Dialog"), ("showHome", "Show Home")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("general", Field { key: "general.show_tooltips", label: "Show Tool Tips", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "general",
            Field { key: "general.show_rich_tooltips", label: "Show Rich Tool Tips", kind: Kind::Bool, wired: true, enabled_by: Some("general.show_tooltips") },
        ),
        ("autosave", Field { key: "autosave.enabled", label: "AutoSave Enabled", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "autosave",
            Field {
                key: "autosave.interval_minutes",
                label: "AutoSave Interval",
                kind: Kind::Int { min: 1.0, max: 60.0, unit: "min" },
                wired: true,
                enabled_by: Some("autosave.enabled"),
            },
        ),
        (
            "mediaCache",
            Field {
                key: "mediaCache.max_mb",
                label: "Media Cache Max",
                kind: Kind::Int { min: 100.0, max: 100000.0, unit: "MB" },
                wired: true,
                enabled_by: None,
            },
        ),
        ("mediaCache", Field { key: "mediaCache.location", label: "Media Cache Location", kind: Kind::Path, wired: true, enabled_by: None }),
        (
            "playback",
            Field {
                key: "playback.resolution",
                label: "Playback Resolution",
                kind: Kind::Choice(&[("full", "Full"), ("half", "Half"), ("quarter", "Quarter")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("playback", Field { key: "playback.draft_decode", label: "Draft Decode", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "timeline",
            Field {
                key: "timeline.still_image_duration",
                label: "Still Image Duration",
                kind: Kind::Float { min: 0.5, max: 30.0, decimals: 1, unit: "s" },
                wired: true,
                enabled_by: None,
            },
        ),
        (
            "timeline",
            Field {
                key: "timeline.transition_duration",
                label: "Transition Duration",
                kind: Kind::Float { min: 0.0, max: 10.0, decimals: 2, unit: "s" },
                wired: true,
                enabled_by: None,
            },
        ),
        ("audio", Field { key: "audio.edit_location", label: "Edit in Audition Location", kind: Kind::Path, wired: false, enabled_by: None }),
        ("audio", Field { key: "audio.output_device", label: "Audio Output Device", kind: Kind::Text, wired: false, enabled_by: None }),
        ("appearance", Field { key: "appearance.accessible_contrast", label: "Accessible Contrast", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "appearance",
            Field {
                key: "appearance.ui_scale",
                label: "UI Scale",
                kind: Kind::Float { min: 0.5, max: 2.0, decimals: 2, unit: "x" },
                wired: true,
                enabled_by: None,
            },
        ),
        ("labels", Field { key: "labels.preset", label: "Label Preset", kind: Kind::Text, wired: true, enabled_by: None }),
        ("media", Field { key: "media.analyze_on_import", label: "Analyze Imported Media", kind: Kind::Bool, wired: true, enabled_by: None }),
        ("general", Field { key: "general.recent_projects", label: "Recent Projects", kind: Kind::Text, wired: false, enabled_by: None }),
    ]
}
