//! One panel contract for the drawing guide, Doodle's prompt, and extraction.
use crate::{EngineError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Panel {
    pub label: String,
    pub description: String,
    pub cell: [u32; 4],
    pub box_px: [u32; 4],
    pub baseline: u32,
    pub hold_frames: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct SheetLayout {
    pub schema_version: u32,
    pub kind: String,
    pub motion: String,
    pub canvas: [u32; 2],
    pub columns: u32,
    pub rows: u32,
    pub fps: u32,
    pub looping: bool,
    pub panels: Vec<Panel>,
    #[serde(default)]
    pub action_rows: Vec<Value>,
}

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidParams(message.into())
}

#[derive(Deserialize)]
struct ActionTemplate {
    id: String,
    label: String,
    looping: bool,
    hold_frames: u32,
    panels: Vec<String>,
}

fn actions() -> Result<Vec<ActionTemplate>> {
    serde_json::from_str(include_str!("actions.json"))
        .map_err(|e| invalid(&format!("Cannot read action templates: {e}")))
}

pub(super) fn motions() -> Result<Value> {
    Ok(json!(
        actions()?
            .iter()
            .map(|a| json!({"id":a.id,"label":a.label}))
            .collect::<Vec<_>>()
    ))
}

pub(super) fn styles() -> Value {
    json!([
        {"id":"original","label":"Original","prompt":"Preserve the reference's existing art style and materials."},
        {"id":"flat_2d","label":"Flat 2D illustration","prompt":"Render as clean flat 2D animation artwork with simple colour shapes, restrained outlines and minimal shading. Simplify fine textures and reflections while preserving identity, silhouette, proportions and distinctive features."},
        {"id":"cutout","label":"Paper cutout","prompt":"Render as simple layered coloured-paper shapes with crisp edges and restrained texture. Preserve character identity and proportions."},
        {"id":"watercolor","label":"Watercolour","prompt":"Render as a soft watercolour illustration with controlled washes and clear silhouettes. Preserve character identity and proportions."},
        {"id":"pencil","label":"Coloured pencil","prompt":"Render as a coloured-pencil illustration with clear shapes and restrained strokes. Preserve character identity and proportions."},
        {"id":"oil","label":"Painted","prompt":"Render as a simplified painted illustration with broad brushwork. Preserve character identity and proportions."}
    ])
}

/// The panel lines a kind starts from before any caller description replaces them.
fn default_panels(kind: &str, action: &ActionTemplate) -> Vec<String> {
    match kind {
        "turnaround" => vec![
            "Front | Front view".into(),
            "Right profile | Facing right".into(),
            "Back | Back view".into(),
            "Left profile | Facing left".into(),
        ],
        "skeleton" => vec!["Assembled reference | Complete asset in the reference pose".into()],
        _ => action.panels.clone(),
    }
}

/// `label | drawing direction`, split and trimmed; a line without `|` describes itself.
fn split_label(text: &str) -> (String, String) {
    let (label, description) = text.split_once('|').unwrap_or((text, text));
    (label.trim().to_string(), description.trim().to_string())
}

/// One caller-supplied panel line, rejected when it is empty, too long or half empty.
fn parse_panel_line(item: &Value) -> Result<(String, String)> {
    let text = item
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 400)
        .ok_or_else(|| {
            invalid("Each panel needs 1 to 400 bytes: label | drawing direction")
        })?;
    let (label, description) = split_label(text);
    if label.is_empty() || description.is_empty() {
        return Err(invalid(
            "Panel labels and drawing directions cannot be empty",
        ));
    }
    Ok((label, description))
}

/// The ordered panel descriptions for this sheet, from the caller or the action template.
fn panel_definitions(
    kind: &str,
    action: &ActionTemplate,
    p: &Value,
) -> Result<Vec<(String, String)>> {
    let defaults = default_panels(kind, action);
    let definitions: Vec<(String, String)> = match p.get("panels") {
        Some(Value::Array(items)) => items
            .iter()
            .map(parse_panel_line)
            .collect::<Result<_>>()?,
        None | Some(Value::Null) => defaults.iter().map(|text| split_label(text)).collect(),
        _ => return Err(invalid("panels must be an ordered array of descriptions")),
    };
    if definitions.is_empty() || definitions.len() > 16 {
        return Err(invalid(
            "Specify 1 to 16 ordered panels; custom motion needs your frame descriptions",
        ));
    }
    Ok(definitions)
}

fn columns_for(kind: &str, panels: usize) -> u32 {
    if kind == "turnaround" {
        4
    } else {
        (panels as u32).min(4)
    }
}

fn checked_fps(p: &Value) -> Result<u64> {
    let fps = p["fps"].as_u64().unwrap_or(12);
    if !(1..=60).contains(&fps) {
        return Err(invalid("fps must be 1 to 60"));
    }
    Ok(fps)
}

fn checked_hold(p: &Value, action: &ActionTemplate) -> Result<u64> {
    let hold = p["hold_frames"]
        .as_u64()
        .unwrap_or(u64::from(action.hold_frames));
    if !(1..=120).contains(&hold) {
        return Err(invalid("hold_frames must be 1 to 120"));
    }
    Ok(hold)
}

/// Lay the descriptions out left to right, top to bottom, inside the 2048 canvas.
fn build_panels(
    definitions: Vec<(String, String)>,
    columns: u32,
    rows: u32,
    hold: u64,
) -> Vec<Panel> {
    definitions
        .into_iter()
        .enumerate()
        .map(|(i, (label, description))| {
            let x = 48 + i as u32 % columns * (1952 / columns);
            let y = 48 + i as u32 / columns * (1952 / rows);
            let cell = [x, y, x + 1952 / columns, y + 1952 / rows];
            let box_px = [x + 24, y + 88, cell[2] - 24, cell[3] - 32];
            Panel {
                label,
                description,
                cell,
                box_px,
                baseline: box_px[3] - 24,
                hold_frames: hold as u32,
            }
        })
        .collect()
}

const GUIDE_LINE: image::Rgb<u8> = image::Rgb([82, 101, 121]);
const GUIDE_DASH: image::Rgb<u8> = image::Rgb([216, 224, 230]);

/// A three pixel border inside the panel cell.
fn draw_cell_border(image: &mut image::RgbImage, [x, y, r, b]: [u32; 4]) {
    for xx in x..r {
        for d in 0..3 {
            image.put_pixel(xx, y + d, GUIDE_LINE);
            image.put_pixel(xx, b - 1 - d, GUIDE_LINE);
        }
    }
    for yy in y..b {
        for d in 0..3 {
            image.put_pixel(x + d, yy, GUIDE_LINE);
            image.put_pixel(r - 1 - d, yy, GUIDE_LINE);
        }
    }
}

/// A dashed safe-area box: eight on, eight off.
fn draw_dashed_box(image: &mut image::RgbImage, [l, t, rr, bb]: [u32; 4]) {
    for xx in (l..rr).filter(|xx| (xx - l) % 16 < 8) {
        image.put_pixel(xx, t, GUIDE_DASH);
        image.put_pixel(xx, bb, GUIDE_DASH);
    }
    for yy in (t..bb).filter(|yy| (yy - t) % 16 < 8) {
        image.put_pixel(l, yy, GUIDE_DASH);
        image.put_pixel(rr, yy, GUIDE_DASH);
    }
}

/// The short baseline ticks either side of the text box.
fn draw_baseline_marks(image: &mut image::RgbImage, [l, _, r, _]: [u32; 4], baseline: u32) {
    for xx in (l - 20..l - 4).chain(r + 4..r + 20) {
        for d in 0..3 {
            image.put_pixel(xx, baseline + d, GUIDE_LINE);
        }
    }
}

/// One `type.create` step per panel, on the blank guide, written to `png`.
fn label_args(panels: &[Panel], blank: &Path, png: &Path) -> Vec<String> {
    let mut args = vec!["run".into(), blank.to_string_lossy().into_owned()];
    for (i, p) in panels.iter().enumerate() {
        args.extend(["--cmd".into(),"type.create".into(),"--params".into(),json!({
            "text":format!("{}. {}",i+1,p.label),"box":[p.cell[0]+12,p.cell[1]+8,p.cell[2]-p.cell[0]-24,64],
            "size":24,"color":"#263b50"}).to_string()]);
    }
    args.extend(["--out".into(), png.to_string_lossy().into_owned()]);
    args
}

impl SheetLayout {
    pub fn new(kind: &str, p: &Value) -> Result<Self> {
        if kind == "poses" && p.get("action_rows").is_some() {
            return Self::action_sheet(p);
        }
        let motion = p["motion"].as_str().unwrap_or("sway");
        let action = actions()?
            .into_iter()
            .find(|a| a.id == motion)
            .ok_or_else(|| {
                invalid("Choose a motion from assets.sheet_plan motions, or use custom")
            })?;
        let definitions = panel_definitions(kind, &action, p)?;
        let columns = columns_for(kind, definitions.len());
        let rows = (definitions.len() as u32).div_ceil(columns);
        let fps = checked_fps(p)?;
        let hold = checked_hold(p, &action)?;
        let panels = build_panels(definitions, columns, rows, hold);
        Ok(Self {
            schema_version: 1,
            kind: kind.into(),
            motion: motion.into(),
            canvas: [2048, 2048],
            columns,
            rows,
            fps: fps as u32,
            looping: p["loop"].as_bool().unwrap_or(action.looping),
            panels,
            action_rows: Vec::new(),
        })
    }

    fn action_sheet(p: &Value) -> Result<Self> {
        let actions = p["action_rows"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 4)
            .ok_or_else(|| invalid("action_rows needs one to four ordered action IDs"))?;
        let mut rows = Vec::new();
        for action in actions {
            let name = action
                .as_str()
                .ok_or_else(|| invalid("Each action row names an action"))?;
            let mut request = p.clone();
            request
                .as_object_mut()
                .ok_or_else(|| invalid("Expected sheet options"))?
                .remove("action_rows");
            request["motion"] = json!(name);
            rows.push(Self::new("poses", &request)?);
        }
        let columns = rows.iter().map(|r| r.panels.len()).max().unwrap_or(8) as u32;
        let row_count = rows.len() as u32;
        let canvas = [4096, 2048];
        let mut panels = Vec::new();
        let mut action_rows = Vec::new();
        for (row, mut layout) in rows.into_iter().enumerate() {
            let first = panels.len();
            for (col, panel) in layout.panels.iter_mut().enumerate() {
                let x = 48 + col as u32 * (canvas[0] - 96) / columns;
                let y = 48 + row as u32 * (canvas[1] - 96) / row_count;
                panel.cell = [
                    x,
                    y,
                    x + (canvas[0] - 96) / columns,
                    y + (canvas[1] - 96) / row_count,
                ];
                panel.box_px = [x + 24, y + 72, panel.cell[2] - 24, panel.cell[3] - 24];
                panel.baseline = panel.box_px[3] - 24;
                panel.label = format!("{} · {}", layout.motion, panel.label);
            }
            let indices = (first..first + layout.panels.len()).collect::<Vec<_>>();
            action_rows.push(json!({"id":layout.motion,"row":row,"frames":indices,"fps":layout.fps,"loop":layout.looping,
                "mirror_variant":if layout.motion=="walk"{Some("walk_left")}else{None}}));
            panels.extend(layout.panels);
        }
        Ok(Self {
            schema_version: 1,
            kind: "poses".into(),
            motion: "actions".into(),
            canvas,
            columns,
            rows: row_count,
            fps: p["fps"].as_u64().unwrap_or(12) as u32,
            looping: false,
            panels,
            action_rows,
        })
    }

    pub fn prompt(&self) -> String {
        let panels = self.panels.iter().enumerate().map(|(i,p)| format!(
            "Panel {}: {} — {}. Drawing bounds [left,top,right,bottom]={:?}; ground baseline y={}. Hold {} frames.",
            i+1,p.label,p.description,p.box_px,p.baseline,p.hold_frames)).collect::<Vec<_>>().join("\n");
        let content = if self.kind == "turnaround" {
            "Each panel shows the named viewpoint of the same complete asset. Keep physical scale, lighting, ground baseline and design consistent across front, profile and back views. These views are reference drawings for later 3D construction."
        } else if self.kind == "skeleton" {
            "The first panel is the assembled reference. Every subsequent panel is exactly the named existing part, complete with hidden overlap painted in its own material. Use one physical scale and orientation for all parts; never enlarge small parts to fill a panel. Do not invent anatomy."
        } else {
            "Each panel contains one complete asset. Keep camera, facing, physical scale and registration fixed. Preserve airborne height in jumping frames. For walking, use only the limbs in the reference and walk in place; for an object without legs, ask for an appropriate movement rather than inventing legs."
        };
        let row_instructions = if self.action_rows.is_empty() {
            String::new()
        } else {
            format!(
                "Each horizontal row is one complete ordered action. Do not mix frames between rows. Row playback definitions: {}. Walking faces right and can be mirrored during playback for walking left; do not draw a fifth row.",
                json!(self.action_rows)
            )
        };
        format!(
            "Use the supplied panel guide as the layout reference and the artwork as the identity reference. Canvas {} × {} px, {} columns × {} rows, ordered left-to-right then top-to-bottom. Keep the visible panel borders and numbered captions OUTSIDE the drawing bounds. Inside each drawing area use flat #ffffff, with no text, grid lines, shadows, or neighbouring artwork. Leave unused panels empty. {content}\n{panels}\nPlayback: {} fps, loop={}. Return the entire uncropped sheet for review. Exact panel geometry is stored in the layout JSON; do not rearrange panels.",
            self.canvas[0], self.canvas[1], self.columns, self.rows, self.fps, self.looping
        ) + "\n"
            + &row_instructions
    }

    pub fn svg(&self) -> String {
        let [width, height] = self.canvas;
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"{width}\" height=\"{height}\" fill=\"white\"/>"
        );
        for (i, p) in self.panels.iter().enumerate() {
            let [x, y, r, b] = p.cell;
            let [l, t, rr, bb] = p.box_px;
            let label = p
                .label
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");
            svg.push_str(&format!("<rect x=\"{x}\" y=\"{y}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#526579\" stroke-width=\"4\"/><text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"24\" fill=\"#263b50\">{}. {label}</text><rect x=\"{l}\" y=\"{t}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#d8e0e6\" stroke-dasharray=\"8 8\"/><path d=\"M{} {}h16 M{} {}h16\" stroke=\"#526579\" stroke-width=\"3\"/>",r-x,b-y,x+12,y+38,i+1,rr-l,bb-t,l-20,p.baseline,rr+4,p.baseline));
        }
        svg.push_str("</svg>");
        svg
    }

    /// Raster guide for image agents, typeset with the installed PhotoCraft CLI.
    pub fn write_guide(&self, root: &Path, png: &Path) -> Result<()> {
        let mut image = image::RgbImage::from_pixel(
            self.canvas[0],
            self.canvas[1],
            image::Rgb([255, 255, 255]),
        );
        for p in &self.panels {
            draw_cell_border(&mut image, p.cell);
            draw_dashed_box(&mut image, p.box_px);
            draw_baseline_marks(&mut image, p.box_px, p.baseline);
        }
        let blank = png.with_extension("grid.png");
        image.save(&blank)?;
        let args = label_args(&self.panels, &blank, png);
        crate::workspace::run_tool_in(root, "photocraft-cli", &args)?;
        std::fs::remove_file(blank)?;
        Ok(())
    }

    pub fn cutter_layout(&self, width: u32, height: u32) -> Value {
        let frames: Vec<Value> = self.panels.iter().enumerate().map(|(i,p)| {
            let [l,t,r,b] = p.box_px;
            json!({"label":p.label,"index":i,"box":[(l+4)*width/self.canvas[0],(t+4)*height/self.canvas[1],(r-4)*width/self.canvas[0],(b-4)*height/self.canvas[1]],
                "gridRow":((p.cell[1]-48) as f64*f64::from(self.rows)/f64::from(self.canvas[1]-96)).round() as u32,
                "gridColumn":((p.cell[0]-48) as f64*f64::from(self.columns)/f64::from(self.canvas[0]-96)).round() as u32,
                "labelSource":"sheet_template","holdFrames":p.hold_frames})
        }).collect();
        json!({"sheetType":match self.kind.as_str(){"turnaround"=>"turnaround","skeleton"=>"exploded",_=>"action"},"grid":{"columns":self.columns,"rows":self.rows},"frames":frames,"preserveRegistration":true,"animation":{"template":self.motion,
            "fps":self.fps,"loop":self.looping,"rows":self.action_rows,"continuousCycle":self.looping,"frameOrder":(0..self.panels.len()).collect::<Vec<_>>()}})
    }
}

/// The saved layout travels with this exact sheet; selecting another motion cannot retarget it.
pub(crate) fn for_sheet(sheet: &Path) -> Result<Option<Value>> {
    let file = sheet.with_extension("layout.json");
    if !file.is_file() {
        return Ok(None);
    }
    let layout: SheetLayout = serde_json::from_slice(&std::fs::read(file)?)
        .map_err(|e| invalid(&format!("Cannot read sheet panel layout: {e}")))?;
    let (width, height) = image::image_dimensions(sheet)?;
    if u64::from(width) * u64::from(layout.canvas[1])
        != u64::from(height) * u64::from(layout.canvas[0])
    {
        return Err(invalid(
            "The returned sheet aspect ratio differs from its panel guide; review the canvas before extraction",
        ));
    }
    Ok(Some(layout.cutter_layout(width, height)))
}
