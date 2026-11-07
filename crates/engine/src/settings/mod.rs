//! Settings: storage + schema + commands. Copied from FilmCraft `settings/`, refactored for Magic.
//! Storage reuses `cmd::project::settings::stored_settings()` (settings.json).

pub mod schema;
pub mod prefs;
pub mod commands;

pub use schema::{CATEGORIES, Kind, fields_with_cat};

pub fn schema_json(session: &crate::Session, category: Option<&str>) -> serde_json::Value {
    use serde_json::json;
    let stored = crate::cmd::project::settings::stored_settings();
    let flat = |k: &str| stored.get(k).cloned().unwrap_or(serde_json::Value::Null);
    let cats: Vec<_> = CATEGORIES.iter()
        .filter(|c| category.is_none_or(|w| w == c.id))
        .map(|c| {
            let fields: Vec<_> = fields_with_cat().into_iter()
                .filter(|(cat, _)| *cat == c.id)
                .map(|(_, f)| {
                    let kind = match f.kind {
                        Kind::Bool => json!({"type":"bool"}),
                        Kind::Int{min,max,unit} => json!({"type":"int","min":min,"max":max,"unit":unit}),
                        Kind::Float{min,max,decimals,unit} => json!({"type":"float","min":min,"max":max,"decimals":decimals,"unit":unit}),
                        Kind::Choice(o) => json!({"type":"choice","choices":o.iter().map(|(v,l)| json!({"value":v,"label":l})).collect::<Vec<_>>()}),
                        Kind::Text => json!({"type":"text"}),
                        Kind::Path => json!({"type":"path"}),
                    };
                    json!({"key":f.key,"label":f.label,"kind":kind,"wired":f.wired,"enabledBy":f.enabled_by,"value":flat(f.key)})
                }).collect();
            json!({"id":c.id,"title":c.title,"command":format!("prefs.{}",c.id),"fields":fields})
        }).collect();
    json!({"categories":cats})
}
