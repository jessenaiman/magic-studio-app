//! Settings schema for Magic (per docs/settings-storage.md). One schema drives
//! UI, CLI, MCP and --control: fields() is the single list. Only settings
//! people might edit and that the engine reads belong here; window/UI state
//! belongs in ui.ron/app.ron, caches in cache_dir, recovery in data_dir,
//! and API keys in the OS keychain.

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

pub const CATEGORIES: &[Category] = &[Category { id: "project", title: "Project" }, Category { id: "image", title: "Image / Scribbles" }];

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
            "project",
            Field {
                key: "project.storyboard_template",
                label: "Storyboard template",
                kind: Kind::Choice(&[("magic", "Magic Studio Canonical Storyboard")]),
                wired: true,
                enabled_by: None,
            },
        ),
        (
            "project",
            Field {
                key: "project.storyboard_format",
                label: "Storyboard document format",
                kind: Kind::Choice(&[("designcraft", "DesignCraft (.designcraft + .pdf + .pptx)")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("project", Field { key: "project.storyboard_locked", label: "Lock canonical storyboard template", kind: Kind::Bool, wired: true, enabled_by: None }),
        (
            "image",
            Field {
                key: "image.provider",
                label: "Image provider",
                kind: Kind::Choice(&[("codex", "Codex (GPT-6 Luna)"), ("agy", "AGY (Gemini 3.8 Flash)"), ("flow", "Google Flow (Browser-Agent)")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("image", Field { key: "scribbles.template", label: "Scribbles template", kind: Kind::Text, wired: true, enabled_by: None }),
        (
            "image",
            Field {
                key: "scribbles.provider",
                label: "Scribbles provider",
                kind: Kind::Choice(&[("codex", "Codex (GPT-6 Luna)"), ("agy", "AGY (Gemini 3.8 Flash)"), ("flow", "Google Flow (Browser-Agent)")]),
                wired: true,
                enabled_by: None,
            },
        ),
        ("image", Field { key: "scribbles.model", label: "Scribbles model", kind: Kind::Text, wired: true, enabled_by: None }),
    ]
}
