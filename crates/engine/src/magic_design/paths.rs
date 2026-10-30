use crate::Session;
use std::path::{Path, PathBuf};

/// Where a step writes its results: beside the source when it lives in the open project
/// (its destination folder), else the workspace's `exports/pose_and_parts/<id>/`.
pub(super) fn results_dir(s: &Session, source: &Path, root: &Path, id: &str) -> PathBuf {
    match (
        s.project_path.as_ref().and_then(|p| p.parent()),
        source.parent(),
    ) {
        (Some(dir), Some(parent)) if source.starts_with(dir) => parent.to_path_buf(),
        _ => root.join("exports/pose_and_parts").join(id),
    }
}
