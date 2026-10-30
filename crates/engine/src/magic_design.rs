//! Asset design commands, grouped by workflow behind stable public entry points.

mod agent_prompt;
mod asset_extract_background;
mod asset_make_transparent;
mod cutout_rig;
mod images;
mod intake;
mod manifests;
mod paths;
mod presets;
mod reference_comparison;
mod sheet_extraction;

pub use agent_prompt::cmd_export_agent_prompt;
pub use asset_extract_background::cmd_asset_extract_background;
pub use asset_make_transparent::cmd_asset_make_transparent;
pub(crate) use asset_make_transparent::matte_image;
pub use cutout_rig::cmd_asset_build_cutout_rig;
pub use images::decontaminate_sprite_rgba;
pub use intake::{cmd_intake_three_question, cmd_intake_validate};
pub use manifests::cmd_asset_load_manifest;
pub use reference_comparison::cmd_intake_compare_reference;
pub use sheet_extraction::cmd_asset_cut_sheet;
