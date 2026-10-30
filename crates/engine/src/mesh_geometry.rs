//! Shared silhouette geometry used by the native preview and mesh export.
use crate::session::Asset;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeSettings {
    pub inflation_depth_pct: u32,
    pub relief_detail_pct: u32,
    pub mesh_res: u32,
    pub proj_blend_pct: u32,
    pub bump_intensity: u32,
}
#[derive(Clone, Debug, Default)]
pub struct MeshGeometry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
/// Same source priority and cleanup as mesh.build_4dir. Called only when source
/// metadata changes, never for each slider movement.
pub fn load_views(asset: &Asset) -> Vec<(String, image::RgbaImage)> {
    let mut views = Vec::new();
    if let Some(manifest) = &asset.turnaround_manifest {
        for frame in &manifest.frames {
            if let Ok(img) = image::open(&frame.resolved_path) {
                views.push((
                    frame.label.clone(),
                    crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), None).0,
                ));
            }
        }
    }
    if views.is_empty()
        && let Some(manifest) = &asset.pose_manifest
    {
        for frame in &manifest.frames {
            if let Ok(img) = image::open(&frame.resolved_path) {
                let target = asset
                    .turnaround_registration
                    .as_ref()
                    .map(|r| r.target_height_px)
                    .filter(|h| *h > 0);
                views.push((
                    frame.label.clone(),
                    crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), target).0,
                ));
                break;
            }
        }
    }
    if views.is_empty()
        && let Ok(img) = image::open(&asset.path)
    {
        views.push((
            "front".into(),
            crate::commands::decontaminate_sprite_rgba(&img.into_rgba8(), None).0,
        ));
    }
    views.retain(|(_, image)| image.pixels().any(|pixel| pixel[3] >= 96));
    views
}

/// The first view whose label contains `needle`, else the fallback view.
fn find_view<'a>(
    views: &'a [(String, image::RgbaImage)],
    needle: &str,
    fallback: &'a image::RgbaImage,
) -> &'a image::RgbaImage {
    views
        .iter()
        .find(|(lbl, _)| lbl.to_ascii_lowercase().contains(needle))
        .map(|(_, img)| img)
        .unwrap_or(fallback)
}

/// Side views carry either label.
fn find_side_view<'a>(
    views: &'a [(String, image::RgbaImage)],
    fallback: &'a image::RgbaImage,
) -> &'a image::RgbaImage {
    views
        .iter()
        .find(|(label, _)| {
            let label = label.to_ascii_lowercase();
            label.contains("side") || label.contains("profile")
        })
        .map(|(_, image)| image)
        .unwrap_or(fallback)
}

/// The four silhouettes a ring samples, and the opaque bounds of each.
struct Silhouettes<'a> {
    front: &'a image::RgbaImage,
    quarter: &'a image::RgbaImage,
    side: &'a image::RgbaImage,
    back: &'a image::RgbaImage,
    bounds: [[u32; 4]; 4],
}

/// Extrusion scale and shading terms derived from the settings percentages.
struct Surface {
    slices: usize,
    rings: usize,
    depth_scale: f32,
    relief_amp: f32,
    blend_w: f32,
}

fn surface(settings: ShapeSettings) -> Surface {
    let ShapeSettings {
        inflation_depth_pct,
        relief_detail_pct,
        mesh_res,
        proj_blend_pct,
        bump_intensity,
    } = settings;
    let res = mesh_res.clamp(12, 64) as usize;
    Surface {
        slices: res,
        rings: res,
        depth_scale: (inflation_depth_pct as f32 / 58.0).clamp(0.25, 1.85),
        relief_amp: (relief_detail_pct as f32 / 100.0) * 0.035
            + (bump_intensity as f32 / 100.0) * 0.02,
        blend_w: (proj_blend_pct as f32 / 100.0).clamp(0.0, 1.0),
    }
}

/// Front width blended toward the quarter width, and side depth scaled for inflation.
fn ring_radii(front: f32, quarter: f32, side: f32, surface: &Surface) -> (f32, f32) {
    let rx = (front * (1.0 - 0.25 * surface.blend_w) + quarter * (0.25 * surface.blend_w))
        .max(0.025);
    let rz = (side * surface.depth_scale).max(0.025);
    (rx, rz)
}

/// Colour at the angle between front, back and the two side views.
fn ring_color(
    front: [f32; 3],
    back: [f32; 3],
    side: [f32; 3],
    quarter: [f32; 3],
    cos_t: f32,
    sin_t: f32,
) -> [f32; 4] {
    let w_front = cos_t.max(0.0);
    let w_back = (-cos_t).max(0.0);
    let w_side = sin_t.abs();
    let w_sum = (w_front + w_back + w_side).max(1e-4);
    let channel = |f: f32, b: f32, s: f32, q: f32| {
        (f * w_front + b * w_back + (s * 0.6 + q * 0.4) * w_side) / w_sum
    };
    [
        channel(front[0], back[0], side[0], quarter[0]),
        channel(front[1], back[1], side[1], quarter[1]),
        channel(front[2], back[2], side[2], quarter[2]),
        1.0,
    ]
}

/// The vertex, normal, uv, colour and index streams of one mesh.
struct MeshBuffers {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuffers {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn vertex(&mut self, position: [f32; 3], normal: [f32; 3], uv: [f32; 2], color: [f32; 4]) {
        self.positions.push(position);
        self.normals.push(normal);
        self.uvs.push(uv);
        self.colors.push(color);
    }

    fn quads(&mut self, slices: usize, rings: usize) {
        let stride = (rings + 1) as u32;
        for iz in 0..slices as u32 {
            for it in 0..rings as u32 {
                let i00 = iz * stride + it;
                let i10 = (iz + 1) * stride + it;
                let i01 = iz * stride + it + 1;
                let i11 = (iz + 1) * stride + it + 1;
                self.indices.extend_from_slice(&[i00, i01, i11, i00, i11, i10]);
            }
        }
    }

    /// Bottom sole center cap at y = 0.0
    fn bottom_cap(&mut self, rings: usize) {
        let center = self.positions.len() as u32;
        self.vertex([0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.5, 1.0], [0.35, 0.30, 0.28, 1.0]);
        for it in 0..rings as u32 {
            self.indices.extend_from_slice(&[center, it + 1, it]);
        }
    }

    /// Top crown cap at y = 1.0
    fn top_cap(&mut self, slices: usize, rings: usize) {
        let stride = (rings + 1) as u32;
        let center = self.positions.len() as u32;
        self.vertex([0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.5, 0.0], [0.75, 0.62, 0.50, 1.0]);
        let ring_start = (slices as u32) * stride;
        for it in 0..rings as u32 {
            self.indices
                .extend_from_slice(&[center, ring_start + it, ring_start + it + 1]);
        }
    }

    fn finish(self) -> MeshGeometry {
        MeshGeometry {
            positions: self.positions,
            normals: self.normals,
            uvs: self.uvs,
            colors: self.colors,
            indices: self.indices,
        }
    }
}

/// One ring of vertices per height slice, each sampled from the four silhouettes.
fn push_rings(buffers: &mut MeshBuffers, views: &Silhouettes, surface: &Surface) {
    for iz in 0..=surface.slices {
        // z_y runs strictly from 0.0 (feet planted on ground plane) to 1.0 (crown)
        let u_height = iz as f32 / surface.slices as f32;
        let v_img = 1.0 - u_height;
        let [bounds_f, bounds_q, bounds_s, bounds_b] = views.bounds;
        let (cx_f, rx_f, col_f) = sample_row_span(views.front, v_img, bounds_f);
        let (_cx_q, rx_q, col_q) = sample_row_span(views.quarter, v_img, bounds_q);
        let (cz_s, rz_s_raw, col_s) = sample_row_span(views.side, v_img, bounds_s);
        let (_cx_b, _rx_b, col_b) = sample_row_span(views.back, v_img, bounds_b);
        let (rx, rz) = ring_radii(rx_f, rx_q, rz_s_raw, surface);
        // Taper very bottom and top rings slightly for clean manifold caps while keeping z >= 0.0
        let cap_taper = if iz == 0 || iz == surface.slices {
            0.55
        } else {
            1.0
        };
        for it in 0..=surface.rings {
            let v_theta = it as f32 / surface.rings as f32;
            let theta = v_theta * std::f32::consts::TAU;
            let cos_t = theta.cos();
            let sin_t = theta.sin();
            let relief = 1.0
                + surface.relief_amp
                    * ((3.0 * theta + u_height * std::f32::consts::TAU).cos() * 0.5);
            let x = cx_f + rx * cap_taper * relief * cos_t;
            let z = cz_s + rz * cap_taper * relief * sin_t;
            let y = u_height.max(0.0); // Ground plane strictly at y = 0.0 (glTF Y-up)
            let nx = cos_t / rx.max(0.01);
            let nz = sin_t / rz.max(0.01);
            let n_len = (nx * nx + nz * nz).sqrt().max(1e-5);
            buffers.vertex(
                [x, y, z],
                [nx / n_len, 0.0, nz / n_len],
                [v_theta, v_img],
                ring_color(col_f, col_b, col_s, col_q, cos_t, sin_t),
            );
        }
    }
}

pub fn build(views: &[(String, image::RgbaImage)], settings: ShapeSettings) -> MeshGeometry {
    let Some((_, fallback_img)) = views.first() else {
        return MeshGeometry::default();
    };
    let fallback_img = fallback_img.clone();
    let front = find_view(views, "front", &fallback_img);
    let quarter = find_view(views, "quarter", front);
    let side = find_side_view(views, &fallback_img);
    let back = find_view(views, "back", &fallback_img);
    let views = Silhouettes {
        front,
        quarter,
        side,
        back,
        bounds: [
            image_bounds(front),
            image_bounds(quarter),
            image_bounds(side),
            image_bounds(back),
        ],
    };
    let surface = surface(settings);
    let mut buffers = MeshBuffers::new();
    push_rings(&mut buffers, &views, &surface);
    buffers.quads(surface.slices, surface.rings);
    buffers.bottom_cap(surface.rings);
    buffers.top_cap(surface.slices, surface.rings);
    buffers.finish()
}

/// Running extent of the opaque pixels seen so far.
struct Extent {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

impl Extent {
    fn of(w: u32, h: u32) -> Self {
        Self {
            min_x: w,
            min_y: h,
            max_x: 0,
            max_y: 0,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn finish(self) -> [u32; 4] {
        [self.min_x, self.min_y, self.max_x, self.max_y]
    }
}

fn image_bounds(img: &image::RgbaImage) -> [u32; 4] {
    let mut extent = Extent::of(img.width().max(1), img.height().max(1));
    for (x, y, pixel) in img.enumerate_pixels() {
        if pixel[3] >= 96 {
            extent.include(x, y);
        }
    }
    extent.finish()
}

/// Opaque pixels of the three rows around one height: their x range, colour sums and count.
struct RowSample {
    x0: u32,
    x1: u32,
    red: u64,
    green: u64,
    blue: u64,
    count: u64,
}

impl RowSample {
    fn mean_color(&self) -> [f32; 3] {
        let count = self.count as f32;
        [
            (self.red as f32 / count) / 255.0,
            (self.green as f32 / count) / 255.0,
            (self.blue as f32 / count) / 255.0,
        ]
    }
}

fn scan_row(img: &image::RgbaImage, y: u32) -> RowSample {
    let w = img.width();
    let h = img.height();
    let mut row = RowSample {
        x0: w,
        x1: 0,
        red: 0,
        green: 0,
        blue: 0,
        count: 0,
    };
    for dy in -1i32..=1i32 {
        let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
        for x in 0..w {
            let p = img.get_pixel(x, sy);
            if p[3] >= 96 {
                row.x0 = row.x0.min(x);
                row.x1 = row.x1.max(x);
                row.red += u64::from(p[0]);
                row.green += u64::from(p[1]);
                row.blue += u64::from(p[2]);
                row.count += 1;
            }
        }
    }
    row
}

/// The pixel row a normalised height maps to inside the opaque body.
fn body_row(v_norm: f32, min_y: u32, max_y: u32) -> u32 {
    let body_h = (max_y - min_y).max(1) as f32;
    (min_y as f32 + v_norm.clamp(0.0, 1.0) * body_h)
        .round()
        .clamp(min_y as f32, max_y as f32) as u32
}

fn sample_row_span(img: &image::RgbaImage, v_norm: f32, bounds: [u32; 4]) -> (f32, f32, [f32; 3]) {
    let [min_x, min_y, max_x, max_y] = bounds;
    if max_y <= min_y || max_x <= min_x {
        return (0.0, 0.18, [0.78, 0.62, 0.48]);
    }
    let body_h = (max_y - min_y).max(1) as f32;
    let y = body_row(v_norm, min_y, max_y);
    let row = scan_row(img, y);
    let global_cx = 0.5 * (min_x + max_x) as f32;
    if row.x1 <= row.x0 || row.count == 0 {
        return (0.0, 0.06, [0.72, 0.58, 0.46]);
    }
    let row_cx = 0.5 * (row.x0 + row.x1) as f32;
    let center_offset = (row_cx - global_cx) / body_h;
    let half_span = (0.5 * (row.x1 - row.x0) as f32 / body_h).clamp(0.035, 0.48);
    (center_offset, half_span, row.mean_color())
}
