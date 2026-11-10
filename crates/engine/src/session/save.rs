//! Save: the only place that decides `has_unsaved_changes()` and `save_all()`.
//!
//! Craft Sessions pattern (see `craft-doc` README, `craft-atomic`, `craft-quit`):
//! each document owns its dirty bit + stamp; the session only asks
//! "anything dirty?" then writes. No counters (`edits_since_save`/`project_saves`),
//! no autosave side-effects in `execute`/`headless`, no React drafts read here.
//! Saving is explicit: the keymap, CLI and MCP all call this one function.

use serde_json::{Value, json};

use crate::{CraftHub, CraftTarget, Result, Session};

impl Session {
    /// True when any open DocState is dirty or the in-memory `.magic` manifest
    /// differs from disk. The single dirty check — UI, quit guard and
    /// `project.save` all read this.
    pub fn has_unsaved_changes(&self) -> bool {
        if self.docs.iter().any(|d| d.dirty) {
            return true;
        }
        let Some(path) = &self.project_path else {
            return false;
        };
        match std::fs::read(path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
            Some(saved) => crate::project::project_is_dirty(self, &saved),
            None => true,
        }
    }

    /// Look for unsaved files and save them. Returns save receipts:
    /// `documents` (dirty DocStates written), `native` (per-app Craft saves)
    /// and `directories` (project layout ensured). Craft satellite files
    /// (`.ecproj`/`.fcproj`/`.designcraft`) are saved by their owning app's
    /// `<app>-cli` / `--control` `file.save` via `CraftHub::execute_and_save`
    /// — the one fan-out, only here in the sessions crate.
    pub fn save_all(&mut self) -> Result<Value> {
        let mut saved = Vec::new();
        for doc in &mut self.docs {
            if doc.dirty {
                doc.save()?;
                saved.push(json!({
                    "kind": doc.kind,
                    "path": doc.path,
                    "revision": doc.revision,
                }));
            }
        }
        crate::project::write_project_file(self)?;
        let mut native = Vec::new();
        if let Some(project) = self.project_path.clone() {
            let dir = project.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            let sat = crate::cmd::film::film_path(self).ok();
            let mut targets: Vec<(CraftTarget, std::path::PathBuf)> = Vec::new();
            if let Some(fc) = sat.filter(|p| p.is_file()) {
                targets.push((CraftTarget::Film, fc));
            }
            let ec = dir.join("lettering/lyrics.ecproj");
            if ec.is_file() {
                // effect.save would re-letter; file.save alone is the per-app save.
                targets.push((CraftTarget::Effect, ec));
            }
            let sb = dir.join("storyboard.designcraft");
            if sb.is_file() {
                targets.push((CraftTarget::Design, sb));
            }
            for (t, path) in targets {
                let app = t.cli_name();
                match CraftHub::execute_and_save(t, &path, "file.save", &json!({})) {
                    Ok(_) => native.push(json!({"app": app, "path": path, "ok": true})),
                    Err(e) => native.push(json!({"app": app, "path": path, "ok": false, "error": e.to_string()})),
                }
            }
        }
        let directories = self.project_path.as_deref().and_then(|p| p.parent()).map(crate::project::ensure_project_layout).transpose()?.unwrap_or_default();
        Ok(json!({
            "ok": true,
            "project": self.project_path,
            "saved": self.project_path,
            "documents": saved,
            "native": native,
            "derivatives": [],
            "relinked": [],
            "warnings": [],
            "directories": directories,
        }))
    }
}
