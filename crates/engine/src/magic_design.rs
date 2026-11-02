//! Asset design commands, grouped by workflow behind stable public entry points.

mod agent_prompt;
mod asset_extract_background;
mod cutout_rig;
mod manifests;
mod paths;

pub use agent_prompt::cmd_export_agent_prompt;
pub use asset_extract_background::cmd_asset_extract_background;
pub use cutout_rig::{cmd_asset_build_cutout_rig, cmd_asset_cut_sheet, cmd_asset_make_transparent};
pub(crate) use cutout_rig::matte_image;
pub use manifests::cmd_asset_load_manifest;

