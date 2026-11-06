use std::path::Path;
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use crate::MagicApp;

fn modified(path: &Path) -> Option<u128> {
    let time = path.metadata().ok()?.modified().ok()?;
    Some(time.duration_since(UNIX_EPOCH).ok()?.as_nanos())
}

fn field<'a>(page: &'a Value, label: &str) -> Option<&'a str> {
    page["slots"]
        .as_array()?
        .iter()
        .find(|slot| slot["label"].as_str() == Some(label))?["text"]
        .as_str()
}

impl MagicApp {
    pub(crate) fn storyboard_workspace_command(&mut self, params: &Value) -> Result<Value, String> {
        let project = self
            .session
            .project_path
            .clone()
            .ok_or("Open a Magic project first.")?;
        let path = project.with_file_name("storyboard.designcraft");
        let action = params["action"].as_str().unwrap_or("state");
        if action != "state" && params["project"].as_str() != project.to_str() {
            return Err("The open Magic project changed.".into());
        }

        match action {
            "state" => {}
            "select" => {
                let page = params["page"]
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .filter(|index| *index < self.storyboard_pages.len())
                    .ok_or("Choose a storyboard page.")?;
                self.selected_page = page;
                self.load_selected_page_slots();
                self.preview_image_path = None;
                self.render_selected_page_preview();
            }
            "refresh" => self.refresh_storyboard(),
            "new" | "add" | "save" | "export" => {
                if action == "save"
                    && (params["page"].as_u64() != Some(self.selected_page as u64)
                        || params["revision"].as_str()
                            != modified(&path).map(|value| value.to_string()).as_deref())
                {
                    return Err(
                        "The storyboard changed. Refresh the page before applying edits.".into(),
                    );
                }
                let (command, fields) = match action {
                    "new" => ("storyboard.new", json!({})),
                    "add" => ("storyboard.add", json!({"archetype":"scene"})),
                    "save" => (
                        "storyboard.set",
                        json!({"page":self.selected_page, "fields":params["fields"], "images":params["images"]}),
                    ),
                    _ => (
                        "storyboard.export",
                        json!({"pdf":project.with_file_name("storyboard.pdf")}),
                    ),
                };
                let result = self.execute_session_command(command, fields)?;
                if action == "add"
                    && let Some(page) = result["page"]
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                {
                    self.selected_page = page;
                }
                self.handle_command_success(command, result);
            }
            "open" => {
                self.execute_session_command("workspace.open", json!({"path":path}))?;
            }
            "scribbles" => {
                let label = params["label"].as_str().ok_or("Choose an image field.")?;
                let page = self
                    .storyboard_pages
                    .get(self.selected_page)
                    .ok_or("Choose a storyboard page.")?;
                if !self.slot_images.contains_key(label) || page["archetype"] != "scene" {
                    return Err("Choose a scene image field.".into());
                }
                let scene = self.storyboard_pages[..=self.selected_page]
                    .iter()
                    .filter(|page| page["archetype"] == "scene")
                    .count();
                self.assistant.open_for_storyboard_slot(
                    scene,
                    label,
                    field(page, "cue").unwrap_or_default(),
                );
            }
            _ => return Err(format!("Unknown Storyboard action `{action}`.")),
        }

        let pages: Vec<Value> = self
            .storyboard_pages
            .iter()
            .map(|page| {
                let title = ["title", "cover_title", "world_heading", "char_heading"]
                    .into_iter()
                    .filter_map(|label| field(page, label))
                    .find(|text| !text.is_empty() && !text.starts_with('['));
                json!({"page":page["page"], "archetype":page["archetype"], "title":title,
                "scene_id":field(page,"scene_id"), "cue":field(page,"cue")})
            })
            .collect();
        let selected = self.storyboard_pages.get(self.selected_page);
        let revision = modified(&path).map(|value| value.to_string());
        let preview_revision = self
            .preview_image_path
            .as_deref()
            .and_then(modified)
            .map(|value| value.to_string());
        Ok(
            json!({"project":project,"path":path,"pages":pages,"page":self.selected_page,
            "slots":selected.map(|page| &page["slots"]), "revision":revision,
            "preview":self.preview_image_path,"preview_revision":preview_revision}),
        )
    }
}
