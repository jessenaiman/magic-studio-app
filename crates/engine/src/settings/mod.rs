//! Settings: `craft-settings` (`config.toml`) storage + schema + commands.
//!
//! Reference:
//! - <https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/engine/src/settings/commands.rs>
//! - <https://github.com/storytold/craft-libs/blob/e653048a187036d30fd912f35f14b32ab4084e5c/docs/settings-storage.md>

pub mod commands;
pub mod prefs;
pub mod schema;

pub use prefs::MagicPrefs;
pub use schema::{CATEGORIES, Kind, fields_with_cat};

pub fn schema_json(_session: &crate::Session, category: Option<&str>) -> serde_json::Value {
    schema_json_for(prefs::open_store().get(), category)
}

pub fn schema_json_for(prefs: &MagicPrefs, category: Option<&str>) -> serde_json::Value {
    use serde_json::json;
    let cats: Vec<_> = CATEGORIES
        .iter()
        .filter(|c| category.is_none_or(|w| w == c.id))
        .map(|c| {
            let fields: Vec<_> = fields_with_cat()
                .into_iter()
                .filter(|(cat, _)| *cat == c.id)
                .map(|(_, f)| {
                    let kind = match f.kind {
                        Kind::Bool => json!({"type":"bool"}),
                        Kind::Int { min, max, unit } => json!({"type":"int","min":min,"max":max,"unit":unit}),
                        Kind::Float { min, max, decimals, unit } => json!({"type":"float","min":min,"max":max,"decimals":decimals,"unit":unit}),
                        Kind::Choice(o) => json!({"type":"choice","choices":o.iter().map(|(v,l)| json!({"value":v,"label":l})).collect::<Vec<_>>()}),
                        Kind::Text => json!({"type":"text"}),
                        Kind::Path => json!({"type":"path"}),
                    };
                    json!({"key":f.key,"label":f.label,"kind":kind,"wired":f.wired,"enabledBy":f.enabled_by,"value":prefs.get_value(f.key)})
                })
                .collect();
            json!({"id":c.id,"title":c.title,"command":format!("prefs.{}",c.id),"fields":fields})
        })
        .collect();
    json!({"categories":cats})
}
