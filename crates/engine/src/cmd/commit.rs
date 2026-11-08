//! `shot.commit`: persist an animation's three documents in one call — the lettering EffectCraft
//! project, the FilmCraft project and the `.magic` project — and report success only after all
//! three saved. The first failure is returned as an error, never as an `ok`.

use serde_json::{Value, json};

use crate::{Result, Session};

/// Save `lettering/lyrics.ecproj` (`effect.save`), then the film `.fcproj` (`film.save`), then
/// the `.magic` (`project.save`). The reply's `effect`, `film` and `magic` are the three saved
/// paths; `ok` is reported only after every one of them saved.
pub fn cmd_shot_commit(s: &mut Session, _p: &Value) -> Result<Value> {
    let effect = crate::cmd::words::native_motion::cmd_effect_save(s, &json!({}))?;
    let film = crate::cmd::film::cmd_film_save(s, &json!({}))?;
    let magic = crate::project::cmd_project_save(s, &json!({}))?;

    Ok(json!({
        "ok": true,
        "magic": magic["project"],
        "effect": effect["project"],
        "film": film["project"],
    }))
}
