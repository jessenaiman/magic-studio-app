//! Mesh inspection using the same native importer as TextureCraft's CLI.
use crate::{EngineError, Result};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) fn mesh_info(path: &Path) -> Result<Value> {
    let imported = texturecraft_formats::import_mesh(path)
        .map_err(|e| EngineError::InvalidParams(format!("TextureCraft mesh import: {e}")))?;
    let mesh = &imported.mesh;
    let report = &imported.report;
    Ok(json!({
        "file": path, "kind": "mesh", "triangles": mesh.triangle_count(),
        "vertices": mesh.vertex_count(), "materials": mesh.materials, "textureSets": mesh.materials,
        "uv": {"triangles": report.triangles, "vertices": report.vertices, "materials": report.materials,
            "uvsGenerated": report.uvs_generated, "uvOutOfRange": report.uv_out_of_range,
            "overlapRatio": report.overlap_ratio, "zeroAreaUvTriangles": report.zero_area_uv_triangles,
            "warnings": report.warnings}
    }))
}
