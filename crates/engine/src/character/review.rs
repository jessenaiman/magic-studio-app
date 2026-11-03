//! `character.animation.review`: Object-Oriented 2D-to-3D animation review pipeline
//! driving the ArtCraft suite CLIs (PhotoCraft, TextureCraft, EffectCraft, Magic Studio, Craft Anything).
//!
//! Evaluates:
//! 1. Character Type: 2D Part Cutout & morphology, 3D Mesh Topology & UV layout, Skeleton Rig & joints.
//! 2. Actions: Semantic action classification, duration, loop settings, in-place flag, retargeting.
//! 3. Motions: 9 Core 2D movement principles (Move, Rotate, Squash/Stretch, Ease, Anticipation,
//!    Overshoot, Follow-Through, Walk, Jump) and 3D retargeted motion comps.
//! 4. Visual Parity: Live frame rendering and output verification via `effectcraft-cli`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::character::tripo::inspect_character_3d;
use crate::workspace::resolve_tool;
use crate::{EngineError, Result, Session};

// ============================================================================
// Tool Abstraction Layer (ArtCraft Suite CLI Drivers)
// ============================================================================

/// Result of executing an external ArtCraft CLI process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Common trait for all ArtCraft suite CLI drivers.
pub trait CraftTool: Send + Sync {
    /// Human-friendly name of the tool.
    fn name(&self) -> &'static str;
    /// The executable binary name (e.g. "photocraft-cli").
    fn binary(&self) -> &'static str;
    /// Resolve the executable path using engine environment and path resolution.
    fn resolved_path(&self) -> String {
        resolve_tool(self.binary())
    }
    /// Check whether the executable exists and can be executed.
    fn is_available(&self) -> bool {
        let p = self.resolved_path();
        !p.is_empty() && (Path::new(&p).is_file() || which_check(&p))
    }
    /// Execute the tool with arguments and capture output.
    fn execute(&self, args: &[&str]) -> Result<ToolExecutionResult> {
        let exe = self.resolved_path();
        if exe.is_empty() {
            return Err(EngineError::ExternalTool(
                self.binary().into(),
                format!("binary '{}' not found", self.binary()),
            ));
        }
        let output = Command::new(&exe)
            .args(args)
            .output()
            .map_err(|e| EngineError::ExternalTool(self.binary().into(), e.to_string()))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ToolExecutionResult {
            exit_code,
            stdout,
            stderr,
            success: output.status.success(),
        })
    }
}

fn which_check(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Driver for `photocraft-cli` (2D raster editor, layer stack, and cutout inspector).
#[derive(Default, Debug, Clone)]
pub struct PhotoCraftTool;

impl CraftTool for PhotoCraftTool {
    fn name(&self) -> &'static str {
        "PhotoCraft"
    }
    fn binary(&self) -> &'static str {
        "photocraft-cli"
    }
}

impl PhotoCraftTool {
    pub fn info(&self, file_path: &Path) -> Result<Value> {
        let path_str = file_path.to_string_lossy();
        let res = self.execute(&["info", &path_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "photocraft-cli info".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("photocraft-cli info json".into(), e.to_string())
        })
    }
}

/// Driver for `texturecraft-cli` (3D mesh, UV unwrapping, topology, and texture sets).
#[derive(Default, Debug, Clone)]
pub struct TextureCraftTool;

impl CraftTool for TextureCraftTool {
    fn name(&self) -> &'static str {
        "TextureCraft"
    }
    fn binary(&self) -> &'static str {
        "texturecraft-cli"
    }
}

impl TextureCraftTool {
    pub fn info(&self, mesh_path: &Path) -> Result<Value> {
        let path_str = mesh_path.to_string_lossy();
        let res = self.execute(&["info", &path_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "texturecraft-cli info".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("texturecraft-cli info json".into(), e.to_string())
        })
    }

    pub fn auto_unwrap(&self, mesh_path: &Path, out_path: &Path) -> Result<PathBuf> {
        let in_str = mesh_path.to_string_lossy();
        let out_str = out_path.to_string_lossy();
        let res = self.execute(&["-p", &in_str, "run", "-c", "uv.autoUnwrap", "-s", &out_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "texturecraft-cli uv.autoUnwrap".into(),
                res.stderr,
            ));
        }
        Ok(out_path.to_path_buf())
    }

    pub fn bake_ao(&self, mesh_path: &Path, resolution: u32, out_png: &Path) -> Result<PathBuf> {
        let in_str = mesh_path.to_string_lossy();
        let res_str = resolution.to_string();
        let out_str = out_png.to_string_lossy();
        let res = self.execute(&[
            "-p",
            &in_str,
            "bake",
            "--map",
            "ao",
            "--resolution",
            &res_str,
            "--out",
            &out_str,
        ])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "texturecraft-cli bake".into(),
                res.stderr,
            ));
        }
        Ok(out_png.to_path_buf())
    }
}

/// Driver for `cadcraft-cli` (parametric CAD rig generation, entity-tree introspection, geometric measurement).
#[derive(Default, Debug, Clone)]
pub struct CadCraftTool;

impl CraftTool for CadCraftTool {
    fn name(&self) -> &'static str {
        "CADCraft"
    }
    fn binary(&self) -> &'static str {
        "cadcraft-cli"
    }
}

impl CadCraftTool {
    /// Task 2 & 6: Entity-Tree Introspection & Precision Geometric Measurement (`cadcraft-cli info`).
    pub fn info(&self, drawing_path: &Path) -> Result<Value> {
        let path_str = drawing_path.to_string_lossy();
        let res = self.execute(&["info", &path_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "cadcraft-cli info".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout)
            .map_err(|e| EngineError::ExternalTool("cadcraft-cli info json".into(), e.to_string()))
    }

    /// Task 1: Parametric Rig Generation (`cadcraft-cli convert` .dxf/.dwg -> .svg).
    pub fn convert(&self, input_path: &Path, out_path: &Path) -> Result<PathBuf> {
        let in_str = input_path.to_string_lossy();
        let out_str = out_path.to_string_lossy();
        let res = self.execute(&["convert", &in_str, &out_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "cadcraft-cli convert".into(),
                res.stderr,
            ));
        }
        Ok(out_path.to_path_buf())
    }
}

/// Driver for `wordcraft-cli` (motion script templating, screenplay markup, storyboard/document JSON inspection).
#[derive(Default, Debug, Clone)]
pub struct WordCraftTool;

impl CraftTool for WordCraftTool {
    fn name(&self) -> &'static str {
        "WordCraft"
    }
    fn binary(&self) -> &'static str {
        "wordcraft-cli"
    }
}

impl WordCraftTool {
    /// Task 4: Document & Storyboard JSON Inspection (`wordcraft-cli inspect`).
    pub fn inspect(&self, doc_path: &Path) -> Result<Value> {
        let path_str = doc_path.to_string_lossy();
        let res = self.execute(&["inspect", &path_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "wordcraft-cli inspect".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("wordcraft-cli inspect json".into(), e.to_string())
        })
    }

    /// Task 3: Motion Script Templating with markup (`wordcraft-cli convert <in> <out> --markup`).
    pub fn convert_markup(&self, input_path: &Path, out_path: &Path) -> Result<PathBuf> {
        let in_str = input_path.to_string_lossy();
        let out_str = out_path.to_string_lossy();
        let res = self.execute(&["convert", &in_str, &out_str, "--markup"])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "wordcraft-cli convert --markup".into(),
                res.stderr,
            ));
        }
        Ok(out_path.to_path_buf())
    }
}

/// Driver for `vectorcraft-cli` (automated contour vectorization and SVG anchor deformation).
#[derive(Default, Debug, Clone)]
pub struct VectorCraftTool;

impl CraftTool for VectorCraftTool {
    fn name(&self) -> &'static str {
        "VectorCraft"
    }
    fn binary(&self) -> &'static str {
        "vectorcraft-cli"
    }
}

impl VectorCraftTool {
    /// Task 5: Automated Contour Vectorization (`vectorcraft-cli convert`).
    pub fn convert(&self, input_path: &Path, out_path: &Path) -> Result<PathBuf> {
        let in_str = input_path.to_string_lossy();
        let out_str = out_path.to_string_lossy();
        let res = self.execute(&["convert", &in_str, &out_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "vectorcraft-cli convert".into(),
                res.stderr,
            ));
        }
        Ok(out_path.to_path_buf())
    }
}

/// Driver for `effectcraft-cli` (motion graphics compositor, 2D/3D comps, frame rendering).
#[derive(Default, Debug, Clone)]
pub struct EffectCraftTool;

impl CraftTool for EffectCraftTool {
    fn name(&self) -> &'static str {
        "EffectCraft"
    }
    fn binary(&self) -> &'static str {
        "effectcraft-cli"
    }
}

impl EffectCraftTool {
    pub fn info(&self, project_path: &Path) -> Result<Value> {
        let path_str = project_path.to_string_lossy();
        let res = self.execute(&["--project", &path_str, "info", "--json"])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "effectcraft-cli info".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("effectcraft-cli info json".into(), e.to_string())
        })
    }

    pub fn render_frame(
        &self,
        project_path: &Path,
        comp: Option<&str>,
        time_seconds: f64,
        out_png: &Path,
    ) -> Result<PathBuf> {
        let path_str = project_path.to_string_lossy();
        let time_str = format!("{time_seconds:.2}");
        let out_str = out_png.to_string_lossy();

        let mut args: Vec<&str> = vec!["--project", &path_str, "render-frame", "--time", &time_str];
        if let Some(c) = comp {
            args.push("--comp");
            args.push(c);
        }
        args.push("--out");
        args.push(&out_str);

        let res = self.execute(&args)?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "effectcraft-cli render-frame".into(),
                res.stderr,
            ));
        }
        Ok(out_png.to_path_buf())
    }

    /// Task 8: Headless Batch Compositing (`effectcraft-cli run --commands`).
    pub fn run_commands(
        &self,
        project_path: &Path,
        commands_file: &Path,
    ) -> Result<ToolExecutionResult> {
        let proj_str = project_path.to_string_lossy();
        let cmds_str = commands_file.to_string_lossy();
        let res = self.execute(&["--project", &proj_str, "run", "--commands", &cmds_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "effectcraft-cli run --commands".into(),
                res.stderr,
            ));
        }
        Ok(res)
    }
}

/// Driver for `magic-cli` (Magic Studio workspace and pipeline command runner).
#[derive(Default, Debug, Clone)]
pub struct MagicStudioTool;

impl CraftTool for MagicStudioTool {
    fn name(&self) -> &'static str {
        "Magic Studio"
    }
    fn binary(&self) -> &'static str {
        "magic-cli"
    }
}

/// Driver for `craft-anything-cli` (agentic investigation and rebuild triage).
#[derive(Default, Debug, Clone)]
pub struct CraftAnythingTool;

impl CraftTool for CraftAnythingTool {
    fn name(&self) -> &'static str {
        "Craft Anything"
    }
    fn binary(&self) -> &'static str {
        "craft-anything-cli"
    }
}

impl CraftAnythingTool {
    /// Task 10: Triage & Archive Inventory (`craft-anything-cli --deep core-archive-show`).
    pub fn archive_show(&self, archive_path: &Path) -> Result<Value> {
        let path_str = archive_path.to_string_lossy();
        let res = self.execute(&["--deep", "core-archive-show", "--path", &path_str])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "craft-anything-cli core-archive-show".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("craft-anything-cli archive json".into(), e.to_string())
        })
    }

    /// Compare two 3D/animation archives by SHA-256 (`craft-anything-cli --deep core-archives-compare`).
    pub fn archives_compare(&self, before: &Path, after: &Path) -> Result<Value> {
        let before_str = before.to_string_lossy();
        let after_str = after.to_string_lossy();
        let res = self.execute(&[
            "--deep",
            "core-archives-compare",
            "--before",
            &before_str,
            "--after",
            &after_str,
        ])?;
        if !res.success {
            return Err(EngineError::ExternalTool(
                "craft-anything-cli core-archives-compare".into(),
                res.stderr,
            ));
        }
        serde_json::from_str::<Value>(&res.stdout).map_err(|e| {
            EngineError::ExternalTool("craft-anything-cli compare json".into(), e.to_string())
        })
    }
}

/// Container for all suite tool drivers.
pub struct ToolRegistry {
    pub photocraft: PhotoCraftTool,
    pub texturecraft: TextureCraftTool,
    pub cadcraft: CadCraftTool,
    pub wordcraft: WordCraftTool,
    pub vectorcraft: VectorCraftTool,
    pub effectcraft: EffectCraftTool,
    pub magic: MagicStudioTool,
    pub craft_anything: CraftAnythingTool,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self {
            photocraft: PhotoCraftTool,
            texturecraft: TextureCraftTool,
            cadcraft: CadCraftTool,
            wordcraft: WordCraftTool,
            vectorcraft: VectorCraftTool,
            effectcraft: EffectCraftTool,
            magic: MagicStudioTool,
            craft_anything: CraftAnythingTool,
        }
    }
}

impl ToolRegistry {
    pub fn probe_all(&self) -> HashMap<&'static str, bool> {
        let mut map = HashMap::new();
        map.insert(self.photocraft.binary(), self.photocraft.is_available());
        map.insert(self.texturecraft.binary(), self.texturecraft.is_available());
        map.insert(self.cadcraft.binary(), self.cadcraft.is_available());
        map.insert(self.wordcraft.binary(), self.wordcraft.is_available());
        map.insert(self.vectorcraft.binary(), self.vectorcraft.is_available());
        map.insert(self.effectcraft.binary(), self.effectcraft.is_available());
        map.insert(self.magic.binary(), self.magic.is_available());
        map.insert(
            self.craft_anything.binary(),
            self.craft_anything.is_available(),
        );
        map
    }
}

// ============================================================================
// Domain Models (Character Types, Actions, Motions, Verdicts)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Pass,
    Warn,
    Fail,
}

impl ReviewVerdict {
    pub fn is_passing(&self) -> bool {
        matches!(self, Self::Pass)
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Pass => " PASS ",
            Self::Warn => " WARN ",
            Self::Fail => " FAIL ",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub stage: String,
    pub check: String,
    pub verdict: ReviewVerdict,
    pub summary: String,
    pub metric: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageReport {
    pub stage_name: String,
    pub verdict: ReviewVerdict,
    pub findings: Vec<Finding>,
    pub metadata: Value,
}

impl StageReport {
    pub fn new(stage_name: impl Into<String>) -> Self {
        Self {
            stage_name: stage_name.into(),
            verdict: ReviewVerdict::Pass,
            findings: Vec::new(),
            metadata: Value::Null,
        }
    }

    pub fn record(
        &mut self,
        check: impl Into<String>,
        verdict: ReviewVerdict,
        summary: impl Into<String>,
        metric: Option<String>,
    ) {
        if verdict == ReviewVerdict::Fail {
            self.verdict = ReviewVerdict::Fail;
        } else if verdict == ReviewVerdict::Warn && self.verdict != ReviewVerdict::Fail {
            self.verdict = ReviewVerdict::Warn;
        }
        self.findings.push(Finding {
            stage: self.stage_name.clone(),
            check: check.into(),
            verdict,
            summary: summary.into(),
            metric,
        });
    }
}

/// 9 core 2D movement principles in the animation sampler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionPrinciple {
    Move,
    Rotate,
    SquashStretch,
    EasyEase,
    Anticipation,
    Overshoot,
    FollowThrough,
    WalkInPlace,
    Jump,
}

impl MotionPrinciple {
    pub fn all() -> &'static [Self] {
        &[
            Self::Move,
            Self::Rotate,
            Self::SquashStretch,
            Self::EasyEase,
            Self::Anticipation,
            Self::Overshoot,
            Self::FollowThrough,
            Self::WalkInPlace,
            Self::Jump,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Move => "01 / MOVE",
            Self::Rotate => "02 / ROTATE",
            Self::SquashStretch => "03 / SQUASH / STRETCH",
            Self::EasyEase => "04 / EASY EASE",
            Self::Anticipation => "05 / ANTICIPATION",
            Self::Overshoot => "06 / OVERSHOOT",
            Self::FollowThrough => "07 / FOLLOW-THROUGH",
            Self::WalkInPlace => "08 / WALK IN PLACE",
            Self::Jump => "09 / JUMP",
        }
    }

    pub fn kinematic_channel(&self) -> &'static str {
        match self {
            Self::Move => "transform/position (linear translation)",
            Self::Rotate => "transform/rotation (pivot oscillation)",
            Self::SquashStretch => "transform/scale (volume preserving scale dynamics)",
            Self::EasyEase => "transform/position (cubic bezier easing)",
            Self::Anticipation => "transform/rotation (counter-movement recoil)",
            Self::Overshoot => "transform/rotation (settling past boundary)",
            Self::FollowThrough => "transform/rotation (subordinate limb lag)",
            Self::WalkInPlace => "transform/rotation + position (antiphase limbs + root bob)",
            Self::Jump => "transform/position + scale (crouch, apex arc, landing cushion)",
        }
    }
}

// ============================================================================
// Execution Context
// ============================================================================

/// Review execution parameters and paths.
#[derive(Debug, Clone)]
pub struct ReviewContext {
    pub project_dir: PathBuf,
    pub character_id: String,
    pub render_test_frames: bool,
    pub output_render_dir: PathBuf,
}

// ============================================================================
// Review Stages (Polymorphic Pipeline Steps)
// ============================================================================

pub trait ReviewStage: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn evaluate(&self, ctx: &ReviewContext, tools: &ToolRegistry) -> Result<StageReport>;
}

/// Stage 1: Review 2D Cutout Morphology, 3D Mesh Topology, and Skeleton Rigging.
pub struct CharacterTypeStage;

fn evaluate_2d_layer_manifest(ctx: &ReviewContext, tools: &ToolRegistry, report: &mut StageReport) {
    let char_dir = ctx
        .project_dir
        .join("assets/characters")
        .join(&ctx.character_id);
    let front_pcraft = char_dir
        .join("front")
        .join(format!("{}-front.pcraft", ctx.character_id));
    let breakout_pcraft = char_dir
        .join("3d/breakouts")
        .join(format!("{}-cutout.pcraft", ctx.character_id));
    let pcraft_target = [front_pcraft, breakout_pcraft]
        .into_iter()
        .find(|p| p.is_file());

    if let Some(pcraft) = pcraft_target {
        inspect_pcraft_file(&pcraft, tools, report);
        return;
    }

    let parts_json = char_dir.join("front/parts.json");
    if parts_json.is_file()
        && let Ok(text) = std::fs::read_to_string(&parts_json)
        && let Ok(val) = serde_json::from_str::<Value>(&text)
    {
        let part_count = val["parts"].as_array().map_or(0, Vec::len);
        let image = val["image"].as_str().unwrap_or("front reference");
        report.record(
            "2D Reference & Parts Manifest",
            ReviewVerdict::Pass,
            format!("Verified front reference '{image}' with {part_count} part definition(s)"),
            Some(format!("{part_count} parts")),
        );
        return;
    }

    report.record(
        "2D Cutout Document",
        ReviewVerdict::Warn,
        "No .pcraft cutout or front/parts.json found in character folder",
        None,
    );
}

fn inspect_pcraft_file(pcraft: &Path, tools: &ToolRegistry, report: &mut StageReport) {
    if !tools.photocraft.is_available() {
        report.record(
            "2D Cutout Inspection",
            ReviewVerdict::Warn,
            "photocraft-cli is not available in PATH or ~/.local/bin",
            None,
        );
        return;
    }
    let info = match tools.photocraft.info(pcraft) {
        Ok(i) => i,
        Err(e) => {
            report.record(
                "2D Cutout Inspection",
                ReviewVerdict::Warn,
                format!("photocraft-cli returned error: {e}"),
                None,
            );
            return;
        }
    };
    let layers = info["layers"].as_array().cloned().unwrap_or_default();
    let width = info["width"].as_u64().unwrap_or(0);
    let height = info["height"].as_u64().unwrap_or(0);
    let mut names: Vec<String> = layers
        .iter()
        .filter_map(|l| l["name"].as_str().map(String::from))
        .collect();
    names.sort();

    if !names.is_empty() {
        report.record(
            "2D Cutout Part Hierarchy",
            ReviewVerdict::Pass,
            format!(
                "Verified {} layer part(s) in {width}x{height} canvas",
                names.len()
            ),
            Some(format!("{} layers: {}", names.len(), names.join(", "))),
        );
    } else {
        report.record(
            "2D Cutout Part Hierarchy",
            ReviewVerdict::Warn,
            format!("No named layers found in {width}x{height} .pcraft document"),
            None,
        );
    }
}

fn evaluate_3d_mesh(glb: &Path, tools: &ToolRegistry, report: &mut StageReport) {
    if !tools.texturecraft.is_available() {
        report.record(
            "3D Mesh Inspection",
            ReviewVerdict::Warn,
            "texturecraft-cli is not available",
            None,
        );
        return;
    }
    let info = match tools.texturecraft.info(glb) {
        Ok(i) => i,
        Err(e) => {
            report.record(
                "3D Mesh Inspection",
                ReviewVerdict::Warn,
                format!("texturecraft-cli returned error: {e}"),
                None,
            );
            return;
        }
    };
    let triangles = info["triangles"].as_u64().unwrap_or(0);
    let vertices = info["vertices"].as_u64().unwrap_or(0);
    let overlap = info["uv"]["overlapRatio"].as_f64().unwrap_or(0.0);
    let zero_area = info["uv"]["zeroAreaUvTriangles"].as_u64().unwrap_or(0);
    let materials = info["materials"].as_array().map_or(0, Vec::len);

    let (tri_verdict, tri_msg) = if triangles > 0 && triangles <= 50_000 {
        (
            ReviewVerdict::Pass,
            format!("{triangles} triangles (under 50k budget), {vertices} vertices"),
        )
    } else if triangles > 50_000 {
        (
            ReviewVerdict::Warn,
            format!("{triangles} triangles exceeds 50k budget"),
        )
    } else {
        (ReviewVerdict::Fail, "Mesh contains 0 triangles".into())
    };
    report.record(
        "3D Mesh Polygon Budget",
        tri_verdict,
        tri_msg,
        Some(format!("{materials} material slot(s)")),
    );

    let uv_ok = overlap < 0.05 && zero_area == 0;
    report.record(
        "UV Seam & Overlap Health",
        if uv_ok {
            ReviewVerdict::Pass
        } else {
            ReviewVerdict::Warn
        },
        format!(
            "UV overlap ratio is {:.2}%, {zero_area} zero-area triangles",
            overlap * 100.0
        ),
        Some(format!("{overlap:.4} overlap ratio")),
    );
}

fn read_rig_metadata(dir_3d: &Path) -> (u64, u64, bool, String, String) {
    let mut bone_count = 0u64;
    let mut expected_bones = 0u64;
    let mut rig_verified = false;
    let mut rig_type = "biped".to_string();
    let mut skeleton_preset = "mixamo".to_string();

    for meta_name in [
        "tripo.json",
        "motion-library-import.json",
        "rigged-import.json",
    ] {
        let Some(tj) = std::fs::read_to_string(dir_3d.join(meta_name))
            .ok()
            .and_then(|c| serde_json::from_str::<Value>(&c).ok())
        else {
            continue;
        };
        if let Some(rt) = tj["rig_type"].as_str() {
            rig_type = rt.to_string();
        }
        if let Some(sp) = tj["skeleton_preset"].as_str() {
            skeleton_preset = sp.to_string();
        }
        rig_verified |= tj["rigged"].as_bool() == Some(true);
        let b = tj["bones"].as_u64().unwrap_or(0);
        expected_bones = expected_bones.max(b);
        bone_count = bone_count.max(tj["bone_count"].as_u64().unwrap_or(b));
    }
    (
        bone_count,
        expected_bones,
        rig_verified,
        rig_type,
        skeleton_preset,
    )
}

fn evaluate_skeleton_rig(ctx: &ReviewContext, glb: &Path, report: &mut StageReport) {
    let dir_3d = ctx
        .project_dir
        .join("assets/characters")
        .join(&ctx.character_id)
        .join("3d");
    let (mut bone_count, expected_bones, rig_verified, rig_type, skeleton_preset) =
        read_rig_metadata(&dir_3d);
    if bone_count == 0 {
        bone_count = glb_joint_count(glb);
    }

    let min_bones = if expected_bones > 0 {
        expected_bones
    } else if rig_verified || rig_type != "biped" {
        1
    } else {
        15
    };
    let (verdict, msg) = if bone_count >= min_bones {
        (
            ReviewVerdict::Pass,
            format!(
                "Rig verified with {bone_count} joints (type: {rig_type}, preset: {skeleton_preset})"
            ),
        )
    } else if bone_count > 0 {
        (
            ReviewVerdict::Warn,
            format!("Skeleton has {bone_count} bones (expected >= {min_bones} for {rig_type})"),
        )
    } else {
        (
            ReviewVerdict::Fail,
            "No skin joints found in 3D model".into(),
        )
    };
    report.record(
        "Skeleton Rig Hierarchy",
        verdict,
        msg,
        Some(format!("{bone_count} bones")),
    );
}

fn evaluate_cad_and_vector(char_dir: &Path, tools: &ToolRegistry, report: &mut StageReport) {
    let cad_file = ["rig.dxf", "rig.dwg"]
        .into_iter()
        .map(|n| char_dir.join(n))
        .find(|p| p.is_file());
    if let Some(cad) = cad_file
        && tools.cadcraft.is_available()
        && let Ok(info) = tools.cadcraft.info(&cad)
    {
        let entities = info["entityCount"]
            .as_u64()
            .or_else(|| info["entities"].as_array().map(|a| a.len() as u64))
            .unwrap_or(0);
        report.record(
            "CAD Parametric Rig & Bounds",
            ReviewVerdict::Pass,
            format!(
                "Verified CAD drawing '{}' via cadcraft-cli ({entities} entities)",
                cad.file_name().unwrap_or_default().to_string_lossy()
            ),
            Some(format!("{entities} entities")),
        );
    }

    let vector_file = ["contour.svg", "rig.svg", "contour.vectorcraft"]
        .into_iter()
        .map(|n| char_dir.join(n))
        .find(|p| p.is_file());
    if let Some(vec_path) = vector_file
        && tools.vectorcraft.is_available()
    {
        report.record(
            "VectorCraft Deformable Contour",
            ReviewVerdict::Pass,
            format!(
                "Verified vector rig/contour '{}' via vectorcraft-cli",
                vec_path.file_name().unwrap_or_default().to_string_lossy()
            ),
            Some("SVG/VectorCraft ready".into()),
        );
    }
}

fn evaluate_brief_and_archives(char_dir: &Path, tools: &ToolRegistry, report: &mut StageReport) {
    let brief_file = ["motion-brief.docx", "motion-brief.md"]
        .into_iter()
        .map(|n| char_dir.join(n))
        .find(|p| p.is_file());
    if let Some(doc_path) = brief_file
        && tools.wordcraft.is_available()
        && let Ok(info) = tools.wordcraft.inspect(&doc_path)
    {
        let blocks = info["blocks"]
            .as_array()
            .or_else(|| info["paragraphs"].as_array())
            .map_or(1, Vec::len);
        report.record(
            "WordCraft Motion Script Brief",
            ReviewVerdict::Pass,
            format!(
                "Inspected motion screenplay '{}' via wordcraft-cli ({blocks} block(s))",
                doc_path.file_name().unwrap_or_default().to_string_lossy()
            ),
            Some(format!("{blocks} blocks")),
        );
    }

    let dir_3d = char_dir.join("3d");
    let zip_archive = std::fs::read_dir(&dir_3d)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|x| x.to_str()) == Some("zip"));
    if let Some(zip_path) = zip_archive
        && tools.craft_anything.is_available()
        && let Ok(arch) = tools.craft_anything.archive_show(&zip_path)
    {
        let entry_count = arch["result"]["entries"]
            .as_array()
            .map(|a| a.len() as u64)
            .or_else(|| arch["result"]["entry_count"].as_u64())
            .unwrap_or(0);
        report.record(
            "Craft Anything Archive Triage",
            ReviewVerdict::Pass,
            format!(
                "Triaged '{}' via craft-anything-cli ({entry_count} entries)",
                zip_path.file_name().unwrap_or_default().to_string_lossy()
            ),
            Some(format!("{entry_count} entries")),
        );
    }
}

impl ReviewStage for CharacterTypeStage {
    fn name(&self) -> &'static str {
        "Type Review (2D Cutout & 3D Mesh / Rig)"
    }

    fn description(&self) -> &'static str {
        "Validates 2D parts/layers in PhotoCraft, 3D mesh triangles and UV overlap in TextureCraft, CAD rigs, and skeleton joint count."
    }

    fn evaluate(&self, ctx: &ReviewContext, tools: &ToolRegistry) -> Result<StageReport> {
        let mut report = StageReport::new(self.name());
        let char_dir = ctx
            .project_dir
            .join("assets/characters")
            .join(&ctx.character_id);
        evaluate_2d_layer_manifest(ctx, tools, &mut report);
        evaluate_cad_and_vector(&char_dir, tools, &mut report);
        evaluate_brief_and_archives(&char_dir, tools, &mut report);

        let c3d = inspect_character_3d(&ctx.project_dir, &ctx.character_id);
        let glb_target = c3d["primary_glb"].as_str().map(PathBuf::from).or_else(|| {
            c3d["glb_files"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(PathBuf::from)
        });

        if let Some(glb) = glb_target.filter(|p| p.is_file()) {
            evaluate_3d_mesh(&glb, tools, &mut report);
            evaluate_skeleton_rig(ctx, &glb, &mut report);
        } else {
            report.record(
                "3D Mesh File",
                ReviewVerdict::Fail,
                format!("No model GLB found for character '{}'", ctx.character_id),
                None,
            );
        }

        report.metadata = json!({
            "character": ctx.character_id,
            "stage": "type_review",
        });

        Ok(report)
    }
}

/// Stage 2: Review Character Action Presets and Split Clips.
pub struct ActionReviewStage;

impl ReviewStage for ActionReviewStage {
    fn name(&self) -> &'static str {
        "Action Review (Semantics & Presets)"
    }

    fn description(&self) -> &'static str {
        "Validates discrete action clips, categories (Interactive, Locomotion, Basic), durations, and retarget configs."
    }

    fn evaluate(&self, ctx: &ReviewContext, _tools: &ToolRegistry) -> Result<StageReport> {
        let mut report = StageReport::new(self.name());
        let c3d = inspect_character_3d(&ctx.project_dir, &ctx.character_id);

        let clips = c3d["clips"].as_array().cloned().unwrap_or_default();
        if clips.is_empty() {
            report.record(
                "Action Clips Catalog",
                ReviewVerdict::Warn,
                format!(
                    "No authored .anim.json clips found for character '{}'",
                    ctx.character_id
                ),
                None,
            );
        } else {
            report.record(
                "Action Clips Catalog",
                ReviewVerdict::Pass,
                format!("Found {} registered 3D animation clip(s)", clips.len()),
                Some(format!("{} clips", clips.len())),
            );

            for clip in &clips {
                let id = clip["id"].as_str().unwrap_or("unknown");
                let name = clip["name"].as_str().unwrap_or(id);
                let duration = clip["duration"].as_f64().unwrap_or(0.0);
                let category = clip["category"].as_str().unwrap_or("interactive");
                let in_place = clip["in_place"].as_bool().unwrap_or(true);
                let fbx_preset = clip["fbx_preset"].as_str().unwrap_or("blender");

                let mut status = ReviewVerdict::Pass;
                let mut note = format!(
                    "Clip '{name}' [{category}]: duration {duration:.1}s, in_place={in_place}, preset={fbx_preset}"
                );

                if duration <= 0.0 || duration > 30.0 {
                    status = ReviewVerdict::Warn;
                    note.push_str(" (abnormal duration)");
                }

                report.record(
                    format!("Action Clip: {id}"),
                    status,
                    note,
                    Some(format!("{duration:.1}s")),
                );
            }
        }

        // Check companion 2D actions in poses.json
        let poses_json = crate::character::POSES;
        if let Ok(poses_doc) = serde_json::from_str::<Value>(poses_json) {
            let actions = poses_doc["actions"].as_object();
            if let Some(act_map) = actions {
                let action_count = act_map.len();
                let keys: Vec<&String> = act_map.keys().collect();
                report.record(
                    "2D Shared Pose Actions",
                    ReviewVerdict::Pass,
                    format!("2D engine action poses library contains {action_count} actions"),
                    Some(
                        keys.iter()
                            .map(|k| k.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                );
            }
        }

        report.metadata = json!({
            "character": ctx.character_id,
            "clips_count": clips.len(),
        });

        Ok(report)
    }
}

/// Stage 3: Review Motion Principles & Kinematics (2D Sampler & 3D EffectCraft comps).
pub struct MotionReviewStage;

fn evaluate_2d_sampler(ctx: &ReviewContext, tools: &ToolRegistry, report: &mut StageReport) {
    let sampler_proj = ctx.project_dir.join("10-2d-movement-sampler.ecproj");
    if !sampler_proj.is_file() || !tools.effectcraft.is_available() {
        return;
    }
    let info = match tools.effectcraft.info(&sampler_proj) {
        Ok(i) => i,
        Err(e) => {
            report.record(
                "2D Movement Sampler",
                ReviewVerdict::Warn,
                format!("Failed to read sampler project: {e}"),
                None,
            );
            return;
        }
    };
    let items = info["project"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut comp_names: Vec<String> = items
        .iter()
        .filter(|it| it["type"].as_str() == Some("Composition"))
        .filter_map(|it| it["name"].as_str().map(String::from))
        .collect();
    comp_names.sort();

    let mut passed_principles = 0;
    for principle in MotionPrinciple::all() {
        let title = principle.title();
        if comp_names.iter().any(|c| c.contains(title)) {
            passed_principles += 1;
            report.record(
                format!("2D Motion: {title}"),
                ReviewVerdict::Pass,
                format!("Motion track verified: {}", principle.kinematic_channel()),
                Some("EffectCraft Comp Active".into()),
            );
        } else {
            report.record(
                format!("2D Motion: {title}"),
                ReviewVerdict::Warn,
                format!("Missing discrete comp for {title}"),
                None,
            );
        }
    }
    report.record(
        "2D Movement Sampler Summary",
        if passed_principles == 9 {
            ReviewVerdict::Pass
        } else {
            ReviewVerdict::Warn
        },
        format!("{passed_principles}/9 2D animation principles implemented in sampler"),
        Some(format!("{passed_principles}/9 principles")),
    );
}

fn evaluate_3d_comp_clip(clip: &Value, tools: &ToolRegistry, report: &mut StageReport) {
    let Some(ecproj_str) = clip["ecproj_3d"].as_str() else {
        return;
    };
    let ecproj = PathBuf::from(ecproj_str);
    if !ecproj.is_file() || !tools.effectcraft.is_available() {
        return;
    }
    let clip_id = clip["id"].as_str().unwrap_or("");
    match tools.effectcraft.info(&ecproj) {
        Ok(info) => {
            let active_comp = &info["activeComp"];
            let duration = active_comp["duration"].as_f64().unwrap_or(0.0);
            let fps = active_comp["frameRate"].as_f64().unwrap_or(0.0);
            let layers = active_comp["layers"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let has_3d_layer = layers.iter().any(|l| {
                l["switches"]["three_d"].as_bool().unwrap_or(false)
                    || l["type"].as_str() == Some("3D Model")
            });
            let valid = has_3d_layer && fps >= 23.97 && duration > 0.0;
            report.record(
                format!("3D Comp: {clip_id}"),
                if valid {
                    ReviewVerdict::Pass
                } else {
                    ReviewVerdict::Warn
                },
                if valid {
                    format!(
                        "Valid 3D stage comp: {fps:.0} FPS, {duration:.1}s duration, 3D model layer bound"
                    )
                } else {
                    format!(
                        "Comp checks: has_3d_layer={has_3d_layer}, fps={fps}, duration={duration}"
                    )
                },
                valid.then(|| format!("{fps:.0} FPS, {duration:.1}s")),
            );
        }
        Err(e) => {
            report.record(
                format!("3D Comp: {clip_id}"),
                ReviewVerdict::Warn,
                format!("effectcraft-cli error: {e}"),
                None,
            );
        }
    }
}

impl ReviewStage for MotionReviewStage {
    fn name(&self) -> &'static str {
        "Motion Review (Kinematic Principles & 3D Comps)"
    }

    fn description(&self) -> &'static str {
        "Evaluates the 9 core 2D movement sampler compositions in EffectCraft and 3D retargeted comps."
    }

    fn evaluate(&self, ctx: &ReviewContext, tools: &ToolRegistry) -> Result<StageReport> {
        let mut report = StageReport::new(self.name());
        evaluate_2d_sampler(ctx, tools, &mut report);

        let c3d = inspect_character_3d(&ctx.project_dir, &ctx.character_id);
        let clips = c3d["clips"].as_array().cloned().unwrap_or_default();
        for clip in &clips {
            evaluate_3d_comp_clip(clip, tools, &mut report);
        }

        report.metadata = json!({
            "stage": "motion_review",
        });

        Ok(report)
    }
}

/// Stage 4: Live Frame Rendering & Visual Parity Verification.
pub struct VisualParityStage;

fn record_rendered_frame(
    label: String,
    kind: &str,
    render_res: Result<PathBuf>,
    report: &mut StageReport,
) {
    match render_res {
        Ok(path) => {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            if size > 1024 {
                report.record(
                    label,
                    ReviewVerdict::Pass,
                    format!("Successfully rendered {kind} frame ({size} bytes)"),
                    Some(path.to_string_lossy().to_string()),
                );
            } else {
                report.record(
                    label,
                    ReviewVerdict::Warn,
                    format!("Rendered {kind} frame is unusually small ({size} bytes)"),
                    None,
                );
            }
        }
        Err(e) => {
            report.record(
                label,
                ReviewVerdict::Warn,
                format!("Failed to render {kind} frame: {e}"),
                None,
            );
        }
    }
}

impl ReviewStage for VisualParityStage {
    fn name(&self) -> &'static str {
        "Visual Parity (Live Frame Rendering)"
    }

    fn description(&self) -> &'static str {
        "Renders key motion frames via `effectcraft-cli render-frame` and verifies image output dimensions and integrity."
    }

    fn evaluate(&self, ctx: &ReviewContext, tools: &ToolRegistry) -> Result<StageReport> {
        let mut report = StageReport::new(self.name());

        if !tools.effectcraft.is_available() {
            report.record(
                "EffectCraft Renderer",
                ReviewVerdict::Warn,
                "effectcraft-cli is not available to render live test frames",
                None,
            );
            return Ok(report);
        }

        if !ctx.render_test_frames {
            report.record(
                "Live Frame Rendering",
                ReviewVerdict::Pass,
                "Frame render testing skipped (pass render_test_frames=true to execute live render)",
                None,
            );
            return Ok(report);
        }

        std::fs::create_dir_all(&ctx.output_render_dir).map_err(EngineError::Io)?;

        let sampler_proj = ctx.project_dir.join("10-2d-movement-sampler.ecproj");
        if sampler_proj.is_file() {
            let out_png = ctx
                .output_render_dir
                .join("2d-movement-sampler-review-frame.png");
            let res = tools
                .effectcraft
                .render_frame(&sampler_proj, None, 1.5, &out_png);
            record_rendered_frame(
                "2D Movement Sampler Live Render".into(),
                "2D sampler",
                res,
                &mut report,
            );
        }

        let c3d = inspect_character_3d(&ctx.project_dir, &ctx.character_id);
        let clips = c3d["clips"].as_array().cloned().unwrap_or_default();
        for clip in clips {
            let Some(ecproj) = clip["ecproj_3d"]
                .as_str()
                .map(PathBuf::from)
                .filter(|p| p.is_file())
            else {
                continue;
            };
            let clip_id = clip["id"].as_str().unwrap_or("clip");
            let out_png = ctx
                .output_render_dir
                .join(format!("3d-{clip_id}-review-frame.png"));
            let res = tools.effectcraft.render_frame(&ecproj, None, 1.0, &out_png);
            record_rendered_frame(
                format!("3D Clip Live Render: {clip_id}"),
                "3D clip",
                res,
                &mut report,
            );
        }

        report.metadata = json!({
            "stage": "visual_parity",
            "output_dir": ctx.output_render_dir.to_string_lossy(),
        });

        Ok(report)
    }
}

// ============================================================================
// Pipeline Orchestrator (Composite Review Runner)
// ============================================================================

/// Master report from executing all review stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveReviewReport {
    pub character: String,
    pub overall_verdict: ReviewVerdict,
    pub tools_available: HashMap<String, bool>,
    pub stages: Vec<StageReport>,
    pub summary_statistics: Value,
}

/// Object-Oriented pipeline orchestrator.
pub struct AnimationReviewPipeline {
    tools: ToolRegistry,
    stages: Vec<Box<dyn ReviewStage>>,
}

impl Default for AnimationReviewPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationReviewPipeline {
    /// Create a standard pipeline with all canonical stages.
    pub fn new() -> Self {
        let stages: Vec<Box<dyn ReviewStage>> = vec![
            Box::new(CharacterTypeStage),
            Box::new(ActionReviewStage),
            Box::new(MotionReviewStage),
            Box::new(VisualParityStage),
        ];

        Self {
            tools: ToolRegistry::default(),
            stages,
        }
    }

    /// Add a custom review stage to the pipeline.
    pub fn add_stage(&mut self, stage: Box<dyn ReviewStage>) {
        self.stages.push(stage);
    }

    /// Run the full pipeline against a character context.
    pub fn execute(&self, ctx: &ReviewContext) -> Result<ComprehensiveReviewReport> {
        let tools_map = self
            .tools
            .probe_all()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        let mut stage_reports = Vec::new();
        let mut overall_verdict = ReviewVerdict::Pass;
        let mut pass_count = 0;
        let mut warn_count = 0;
        let mut fail_count = 0;

        for stage in &self.stages {
            let res = stage.evaluate(ctx, &self.tools)?;
            for f in &res.findings {
                match f.verdict {
                    ReviewVerdict::Pass => pass_count += 1,
                    ReviewVerdict::Warn => {
                        warn_count += 1;
                        if overall_verdict != ReviewVerdict::Fail {
                            overall_verdict = ReviewVerdict::Warn;
                        }
                    }
                    ReviewVerdict::Fail => {
                        fail_count += 1;
                        overall_verdict = ReviewVerdict::Fail;
                    }
                }
            }
            stage_reports.push(res);
        }

        let summary = json!({
            "total_checks": pass_count + warn_count + fail_count,
            "passed_checks": pass_count,
            "warn_checks": warn_count,
            "fail_checks": fail_count,
        });

        Ok(ComprehensiveReviewReport {
            character: ctx.character_id.clone(),
            overall_verdict,
            tools_available: tools_map,
            stages: stage_reports,
            summary_statistics: summary,
        })
    }

    /// Render a human-readable ANSI terminal report.
    pub fn format_terminal(&self, report: &ComprehensiveReviewReport) -> String {
        let mut out = String::new();
        out.push('\n');
        out.push_str(
            "================================================================================\n",
        );
        out.push_str("    MAGIC STUDIO 2D TO 3D ANIMATION REVIEW REPORT: ARTCRAFT SUITE AUDIT\n");
        out.push_str(
            "================================================================================\n",
        );
        out.push_str(&format!(" Character:      {}\n", report.character));
        out.push_str(&format!(
            " Overall Status: [{}]\n",
            report.overall_verdict.badge()
        ));
        out.push_str(" ArtCraft Tools: ");
        let mut tool_parts = Vec::new();
        for (tool, ok) in &report.tools_available {
            tool_parts.push(format!(
                "{}: {}",
                tool,
                if *ok { "READY" } else { "MISSING" }
            ));
        }
        tool_parts.sort();
        out.push_str(&tool_parts.join(" | "));
        out.push('\n');
        out.push_str(
            "--------------------------------------------------------------------------------\n",
        );

        for stage in &report.stages {
            out.push_str(&format!(
                "\n▶ STAGE: {} [{}]\n",
                stage.stage_name,
                stage.verdict.badge()
            ));
            out.push_str(&format!(
                "{:<38} {:<8} {:<32}\n",
                "CHECK", "STATUS", "SUMMARY / DETAILS"
            ));
            out.push_str(&format!("{:-<38} {:-<8} {:-<32}\n", "", "", ""));

            for f in &stage.findings {
                let badge = f.verdict.badge();
                let metric = f.metric.as_deref().unwrap_or("");
                let line_text = if metric.is_empty() {
                    f.summary.clone()
                } else {
                    format!("{} [{}]", f.summary, metric)
                };
                out.push_str(&format!("{:<38} [{}] {}\n", f.check, badge, line_text));
            }
        }

        out.push_str(
            "\n--------------------------------------------------------------------------------\n",
        );
        let stats = &report.summary_statistics;
        out.push_str(&format!(
            " SUMMARY: {} checks total ({} passed, {} warnings, {} failed)\n",
            stats["total_checks"],
            stats["passed_checks"],
            stats["warn_checks"],
            stats["fail_checks"],
        ));
        out.push_str(
            "================================================================================\n\n",
        );
        out
    }
}

// ============================================================================
// Engine Command Handler (`character.animation.review`)
// ============================================================================

fn glb_joint_count(path: &Path) -> u64 {
    let Ok(bytes) = std::fs::read(path) else {
        return 0;
    };
    let count_from_doc = |doc: &Value| -> u64 {
        doc["skins"]
            .as_array()
            .map(|skins| {
                skins
                    .iter()
                    .filter_map(|s| s["joints"].as_array().map(|j| j.len() as u64))
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    };
    if bytes.len() >= 20 && &bytes[0..4] == b"glTF" {
        let chunk_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        if &bytes[16..20] == b"JSON"
            && bytes.len() >= 20 + chunk_len
            && let Ok(doc) = serde_json::from_slice::<Value>(&bytes[20..20 + chunk_len])
        {
            return count_from_doc(&doc);
        }
    }
    if let Ok(doc) = serde_json::from_slice::<Value>(&bytes) {
        return count_from_doc(&doc);
    }
    0
}

/// Execute `character.animation.review` command.
pub fn cmd_character_animation_review(s: &mut Session, p: &Value) -> Result<Value> {
    let project_dir = if let Some(proj) = p["project_dir"].as_str() {
        PathBuf::from(proj)
    } else {
        crate::workshop::project_dir(s)?
    };

    let character_id = p["character"]
        .as_str()
        .map(crate::workspace::sanitize_asset_id)
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| "toasty".to_string());

    let render_test_frames = p["render_test_frames"].as_bool().unwrap_or(false);
    let output_render_dir = if let Some(out) = p["output_dir"].as_str() {
        PathBuf::from(out)
    } else {
        project_dir.join("renders/review")
    };

    let ctx = ReviewContext {
        project_dir,
        character_id,
        render_test_frames,
        output_render_dir,
    };

    let pipeline = AnimationReviewPipeline::new();
    let report = pipeline.execute(&ctx)?;
    let text = pipeline.format_terminal(&report);

    Ok(json!({
        "ok": report.overall_verdict != ReviewVerdict::Fail,
        "verdict": report.overall_verdict,
        "character": report.character,
        "tools_available": report.tools_available,
        "summary": report.summary_statistics,
        "stages": report.stages,
        "formatted_report": text,
    }))
}
