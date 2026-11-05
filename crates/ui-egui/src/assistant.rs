//! Fidget & Scribbles Craft AI Assistant Drawer (`crates/ui-egui/src/assistant.rs`).
//!
//! Exposes Magic Studio's two specialized Craft AI roles:
//! 1. **Fidget (`RoleId("fidget")`, `text.complete`)**: Prompt & Markdown Template Architect that
//!    reworks requests into YAML-frontmatter Markdown (`---` tags + `save_command`) and includes
//!    the mandatory single-line [`magic_engine::fidget::FIDGET_NOTE`].
//! 2. **Scribbles (`RoleId("scribbles")`, `image.generate`)**: Template-gated Codex / Craft AI
//!    image generator that requires a template (`new`, `image`, `sheet`, `poses`, `actions`,
//!    `storyboard_hero`, `storyboard_sketch`, `custom`), supports multi-item loops (`count` 1..=4),
//!    and executes the template's prescribed save command (`project.add_asset`, `storyboard.set`,
//!    `character.sprite`).

use egui::{CornerRadius, RichText, ScrollArea, Stroke, Ui};
use serde_json::{Value, json};

use crate::theme::{
    ACCENT, ACCENT_AMBER, ACCENT_GREEN, ACCENT_TEAL, ACCENT_VIOLET, BG_DEEP, BG_ELEVATED, BG_INPUT,
    BORDER_STRONG, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use crate::widgets::{
    card_frame, pill_toggle, primary_button, prop_row, secondary_button, section_header,
    status_badge,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantTab {
    Fidget,
    Scribbles,
    CraftAiSetup,
}

#[derive(Clone, Debug)]
pub struct TemplateEntry {
    pub id: String,
    pub label: String,
    pub canvas: [u64; 2],
    pub needs_reference: bool,
    pub save_command: String,
    pub save_slot: Option<String>,
    pub tags: Vec<String>,
    pub layout: String,
}

pub struct AssistantDrawer {
    pub open: bool,
    pub active_tab: AssistantTab,
    pub templates: Vec<TemplateEntry>,
    pub selected_template: String,
    pub selected_action: String,
    pub prompt_input: String,
    pub notes_input: String,
    pub reference_path: String,
    pub target_scene: usize,
    pub target_slot: String,
    pub loop_count: u64,
    pub prepared_markdown: String,
    pub prepared_fidget: Option<Value>,
    pub fidget_chat_input: String,
    pub last_receipt: Option<Value>,
    pub craft_ai_status: Value,
    pub agent_config: Value,
}

impl Default for AssistantDrawer {
    fn default() -> Self {
        Self {
            open: false,
            active_tab: AssistantTab::Scribbles,
            templates: Vec::new(),
            selected_template: "new".into(),
            selected_action: "walk".into(),
            prompt_input: String::new(),
            notes_input: String::new(),
            reference_path: String::new(),
            target_scene: 1,
            target_slot: "hero_image".into(),
            loop_count: 1,
            prepared_markdown: String::new(),
            prepared_fidget: None,
            fidget_chat_input: String::new(),
            last_receipt: None,
            craft_ai_status: json!({}),
            agent_config: json!({}),
        }
    }
}

impl AssistantDrawer {
    pub fn refresh_from_session(&mut self, session: &mut magic_engine::Session) {
        if let Ok(tpl) = session.execute("scribbles.templates", json!({})) {
            self.craft_ai_status = tpl.get("craft_ai").cloned().unwrap_or_else(|| json!({}));
            if let Some(kinds) = tpl["kinds"].as_object() {
                let order = [
                    "new",
                    "image",
                    "sheet",
                    "poses",
                    "actions",
                    "storyboard_hero",
                    "storyboard_sketch",
                    "custom",
                ];
                let mut list = Vec::new();
                for key in order {
                    if let Some(spec) = kinds.get(key) {
                        list.push(Self::parse_template(key, spec));
                    }
                }
                for (key, spec) in kinds {
                    if !order.contains(&key.as_str()) {
                        list.push(Self::parse_template(key, spec));
                    }
                }
                self.templates = list;
            }
        }
        if let Ok(cfg) = session.execute("agent.config", json!({})) {
            self.agent_config = cfg;
        }
    }

    fn parse_template(id: &str, spec: &Value) -> TemplateEntry {
        TemplateEntry {
            id: id.to_owned(),
            label: spec["label"].as_str().unwrap_or(id).to_owned(),
            canvas: [
                spec["canvas"][0].as_u64().unwrap_or(1024),
                spec["canvas"][1].as_u64().unwrap_or(1536),
            ],
            needs_reference: spec["needs_reference"].as_bool().unwrap_or(false),
            save_command: spec["save_command"]
                .as_str()
                .unwrap_or("project.add_asset")
                .to_owned(),
            save_slot: spec["save_slot"].as_str().map(str::to_owned),
            tags: spec["tags"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            layout: spec["layout"].as_str().unwrap_or_default().to_owned(),
        }
    }

    /// Open the Scribbles drawer pre-configured for a Storyboard scene & slot.
    pub fn open_for_storyboard_slot(&mut self, scene: usize, slot: &str, cue_hint: &str) {
        self.open = true;
        self.active_tab = AssistantTab::Scribbles;
        self.target_scene = scene.max(1);
        self.target_slot = slot.to_owned();
        self.selected_template = if slot == "hero_image" {
            "storyboard_hero".into()
        } else {
            "storyboard_sketch".into()
        };
        if self.prompt_input.trim().is_empty() && !cue_hint.trim().is_empty() {
            self.prompt_input = cue_hint.trim().to_owned();
        }
    }

    pub fn current_template(&self) -> Option<&TemplateEntry> {
        self.templates
            .iter()
            .find(|t| t.id == self.selected_template)
    }

    /// Render the right-side Assistant Drawer and return any engine command `(id, params)` queued
    /// by the user.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        project_assets: &[magic_engine::ProjectAsset],
        busy: bool,
    ) -> Option<(String, Value)> {
        let mut command_to_run: Option<(String, Value)> = None;

        // Top drawer header
        ui.horizontal(|ui| {
            if pill_toggle(
                ui,
                self.active_tab == AssistantTab::Fidget,
                "Fidget · Briefs",
                ACCENT_VIOLET,
            )
            .on_hover_text("Fidget (`text.complete`): reworks prompts into YAML-frontmatter Markdown templates")
            .clicked()
            {
                self.active_tab = AssistantTab::Fidget;
            }
            if pill_toggle(
                ui,
                self.active_tab == AssistantTab::Scribbles,
                "Scribbles · Draw",
                ACCENT_TEAL,
            )
            .on_hover_text("Scribbles (`image.generate`): template-gated image_gen loop with automatic save commands")
            .clicked()
            {
                self.active_tab = AssistantTab::Scribbles;
            }
            if pill_toggle(
                ui,
                self.active_tab == AssistantTab::CraftAiSetup,
                "Craft AI",
                ACCENT,
            )
            .on_hover_text("Inspect Craft AI roles (`craft-ai-client`) and local agent routing")
            .clicked()
            {
                self.active_tab = AssistantTab::CraftAiSetup;
            }
        });

        ui.add_space(4.0);
        // Mandatory working-template banner
        egui::Frame::new()
            .fill(BG_DEEP)
            .stroke(Stroke::new(1.0, ACCENT_AMBER.gamma_multiply(0.5)))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(magic_engine::fidget::FIDGET_NOTE)
                        .size(10.5)
                        .color(ACCENT_AMBER),
                );
            });
        ui.add_space(6.0);

        ScrollArea::vertical()
            .id_salt("assistant_drawer_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| match self.active_tab {
                AssistantTab::Fidget => {
                    if let Some(cmd) = self.show_fidget_tab(ui, project_assets, busy) {
                        command_to_run = Some(cmd);
                    }
                }
                AssistantTab::Scribbles => {
                    if let Some(cmd) = self.show_scribbles_tab(ui, project_assets, busy) {
                        command_to_run = Some(cmd);
                    }
                }
                AssistantTab::CraftAiSetup => {
                    self.show_craft_ai_tab(ui);
                }
            });

        command_to_run
    }

    fn show_template_picker(&mut self, ui: &mut Ui) {
        if section_header(ui, "1. Select Required Template", true) {
            ui.add_space(4.0);
            for tpl in &self.templates {
                let selected = self.selected_template == tpl.id;
                let border = if selected { ACCENT_TEAL } else { BORDER_SUBTLE };
                let fill = if selected {
                    ACCENT_TEAL.gamma_multiply(0.14)
                } else {
                    BG_ELEVATED
                };
                let resp = egui::Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, border))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&tpl.label)
                                    .size(12.0)
                                    .strong()
                                    .color(if selected { ACCENT_TEAL } else { TEXT_PRIMARY }),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    status_badge(ui, &tpl.save_command, ACCENT);
                                    if tpl.needs_reference {
                                        status_badge(ui, "needs ref", ACCENT_AMBER);
                                    }
                                    ui.label(
                                        RichText::new(format!(
                                            "{}×{}",
                                            tpl.canvas[0], tpl.canvas[1]
                                        ))
                                        .size(10.5)
                                        .color(TEXT_MUTED),
                                    );
                                },
                            );
                        });
                        if selected {
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(&tpl.layout)
                                    .size(10.5)
                                    .color(TEXT_SECONDARY),
                            );
                        }
                    })
                    .response
                    .interact(egui::Sense::click());
                if resp.clicked() {
                    self.selected_template = tpl.id.clone();
                    if let Some(slot) = &tpl.save_slot {
                        self.target_slot = slot.clone();
                    }
                }
                ui.add_space(3.0);
            }
        }
    }

    fn show_parameters_section(
        &mut self,
        ui: &mut Ui,
        project_assets: &[magic_engine::ProjectAsset],
    ) {
        if section_header(ui, "2. Template Parameters & Save Target", true) {
            ui.add_space(4.0);
            card_frame().show(ui, |ui| {
                ui.label(
                    RichText::new("Character / Scene Description (`prompt`)")
                        .size(11.0)
                        .color(TEXT_SECONDARY),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.prompt_input)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("e.g. a felt-plush clockwork owl wearing a brass lantern vest"),
                );

                ui.add_space(4.0);
                ui.label(
                    RichText::new("Revision / Styling Notes (`notes`, optional)")
                        .size(11.0)
                        .color(TEXT_SECONDARY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.notes_input)
                        .desired_width(f32::INFINITY)
                        .hint_text("Optional palette, posture, or Improvement notes"),
                );

                if self.selected_template == "actions" {
                    ui.add_space(4.0);
                    prop_row(ui, "Action Cycle", |ui| {
                        egui::ComboBox::from_id_salt("scribbles_action_combo")
                            .selected_text(&self.selected_action)
                            .show_ui(ui, |ui| {
                                for act in ["walk", "run", "jump", "wave", "turn"] {
                                    ui.selectable_value(
                                        &mut self.selected_action,
                                        act.to_owned(),
                                        act,
                                    );
                                }
                            });
                    });
                }

                ui.add_space(4.0);
                prop_row(ui, "Reference PNG", |ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    if secondary_button(ui, "Browse…").clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .add_filter("PNG Image", &["png"])
                            .pick_file()
                    {
                        self.reference_path = file.display().to_string();
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.reference_path)
                            .desired_width(130.0)
                            .hint_text("optional/ref.png"),
                    );
                });

                if !project_assets.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Project refs:").size(10.5).color(TEXT_MUTED));
                        for asset in project_assets.iter().take(6) {
                            if let Some(p) = &asset.path
                                && ui
                                    .small_button(&asset.name)
                                    .on_hover_text(p.display().to_string())
                                    .clicked()
                            {
                                self.reference_path = p.display().to_string();
                            }
                        }
                    });
                }

                if matches!(
                    self.selected_template.as_str(),
                    "storyboard_hero" | "storyboard_sketch"
                ) {
                    ui.add_space(4.0);
                    prop_row(ui, "Target Scene", |ui| {
                        let mut s = self.target_scene as f32;
                        if crate::widgets::hot_number(ui, &mut s, 1.0..=99.0, 0.2, "").changed() {
                            self.target_scene = (s.round() as usize).max(1);
                        }
                    });
                    prop_row(ui, "Target Slot", |ui| {
                        egui::ComboBox::from_id_salt("scribbles_slot_combo")
                            .selected_text(&self.target_slot)
                            .show_ui(ui, |ui| {
                                for slot in [
                                    "hero_image",
                                    "sketch_left",
                                    "sketch_right",
                                    "source_image",
                                    "result_image",
                                ] {
                                    ui.selectable_value(
                                        &mut self.target_slot,
                                        slot.to_owned(),
                                        slot,
                                    );
                                }
                            });
                    });
                }

                ui.add_space(4.0);
                prop_row(ui, "Image Loop", |ui| {
                    let mut c = self.loop_count as f32;
                    if crate::widgets::hot_number(ui, &mut c, 1.0..=4.0, 0.1, "x").changed() {
                        self.loop_count = (c.round() as u64).clamp(1, 4);
                    }
                });
            });
        }
    }

    fn build_request_params(&self) -> Value {
        let mut p = json!({
            "kind": self.selected_template,
            "prompt": self.prompt_input.trim(),
            "count": self.loop_count,
        });
        if !self.notes_input.trim().is_empty() {
            p["notes"] = json!(self.notes_input.trim());
        }
        if !self.reference_path.trim().is_empty() {
            p["reference"] = json!(self.reference_path.trim());
        }
        if self.selected_template == "actions" {
            p["action"] = json!(self.selected_action);
        }
        if matches!(
            self.selected_template.as_str(),
            "storyboard_hero" | "storyboard_sketch"
        ) {
            p["scene"] = json!(self.target_scene);
            p["slot"] = json!(self.target_slot);
        }
        if let Some(fidget) = &self.prepared_fidget
            && fidget["kind"].as_str() == Some(self.selected_template.as_str())
        {
            p["fidget"] = fidget.clone();
        }
        p
    }

    fn show_fidget_tab(
        &mut self,
        ui: &mut Ui,
        project_assets: &[magic_engine::ProjectAsset],
        busy: bool,
    ) -> Option<(String, Value)> {
        let mut cmd = None;
        self.show_template_picker(ui);
        ui.add_space(6.0);
        self.show_parameters_section(ui, project_assets);
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if primary_button(ui, "Prepare Frontmatter Markdown (Fidget)").clicked() && !busy {
                cmd = Some(("fidget.prepare".into(), self.build_request_params()));
            }
            if secondary_button(ui, "Send to Fidget Agent").clicked()
                && !busy
                && !self.prompt_input.trim().is_empty()
            {
                let save_cmd = self
                    .current_template()
                    .map(|t| t.save_command.as_str())
                    .unwrap_or("project.add_asset");
                cmd = Some((
                    "sprocket.send".into(),
                    json!({
                        "message": self.prompt_input.trim(),
                        "template": self.selected_template,
                        "save_command": save_cmd,
                        "new_chat": true,
                        "free_only": true,
                    }),
                ));
            }
        });

        ui.add_space(6.0);
        if section_header(ui, "3. Fidget Markdown Output (YAML Frontmatter + NOTE)", true) {
            ui.add_space(4.0);
            card_frame().show(ui, |ui| {
                if self.prepared_markdown.is_empty() {
                    ui.label(
                        RichText::new(
                            "Click 'Prepare Frontmatter Markdown' above to build the inspectable YAML-frontmatter Markdown brief.",
                        )
                        .size(11.5)
                        .color(TEXT_MUTED),
                    );
                } else {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.prepared_markdown)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);
                    if primary_button(ui, "Hand Off Brief to Scribbles →").clicked() {
                        self.active_tab = AssistantTab::Scribbles;
                    }
                }
            });
        }
        cmd
    }

    fn show_scribbles_tab(
        &mut self,
        ui: &mut Ui,
        project_assets: &[magic_engine::ProjectAsset],
        busy: bool,
    ) -> Option<(String, Value)> {
        let mut cmd = None;
        self.show_template_picker(ui);
        ui.add_space(6.0);
        self.show_parameters_section(ui, project_assets);
        ui.add_space(6.0);

        if let Some(tpl) = self.current_template().cloned() {
            card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    status_badge(ui, &format!("template: {}", tpl.id), ACCENT_TEAL);
                    status_badge(ui, &format!("save: {}", tpl.save_command), ACCENT);
                    if self.loop_count > 1 {
                        status_badge(ui, &format!("loop: {}x", self.loop_count), ACCENT_VIOLET);
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, "Preview Brief (Fidget)").clicked() && !busy {
                        cmd = Some(("fidget.prepare".into(), self.build_request_params()));
                    }
                    let label = if busy {
                        "Running Scribbles…"
                    } else if self.loop_count > 1 {
                        "Loop image_gen & Save (Scribbles)"
                    } else {
                        "Generate & Save (Scribbles)"
                    };
                    if primary_button(ui, label).clicked() && !busy {
                        cmd = Some(("scribbles.generate".into(), self.build_request_params()));
                    }
                });
            });
        }

        if !self.prepared_markdown.is_empty() {
            ui.add_space(6.0);
            if section_header(ui, "Active Fidget Markdown Brief", false) {
                card_frame().show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.prepared_markdown)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(7)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
        }

        if let Some(receipt) = &self.last_receipt {
            ui.add_space(6.0);
            if section_header(ui, "Last Scribbles Save Receipt", true) {
                card_frame().show(ui, |ui| {
                    if let Some(img) = receipt["image"].as_str() {
                        ui.label(RichText::new(format!("Saved PNG: {img}")).size(11.0).color(ACCENT_GREEN));
                    }
                    if let Some(save_cmd) = receipt["scribbles"]["save_command"].as_str() {
                        ui.label(
                            RichText::new(format!("Save Command: {save_cmd}"))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                        );
                    }
                });
            }
        }
        cmd
    }

    fn show_craft_ai_tab(&mut self, ui: &mut Ui) {
        if section_header(ui, "Craft AI Roles (`craft-ai-integrations`)", true) {
            ui.add_space(4.0);
            if let Some(roles) = self.craft_ai_status["roles"].as_array() {
                for role in roles {
                    let id = role["id"].as_str().unwrap_or_default();
                    let label = role["label"].as_str().unwrap_or(id);
                    let cap = role["capability"].as_str().unwrap_or_default();
                    let state = role["status"]["state"].as_str().unwrap_or("unavailable");
                    card_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(label).size(12.0).strong().color(TEXT_PRIMARY));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let color = if state == "ready" {
                                        ACCENT_GREEN
                                    } else {
                                        ACCENT_AMBER
                                    };
                                    status_badge(ui, state, color);
                                    status_badge(ui, cap, ACCENT);
                                },
                            );
                        });
                        if let Some(reason) = role["status"]["reason"].as_str() {
                            ui.label(
                                RichText::new(format!(
                                    "{reason} — falls back to local CLI adapter (`codex` / `opencode` / `omp`)"
                                ))
                                .size(11.0)
                                .color(TEXT_SECONDARY),
                            );
                        } else if let Some(model) = role["status"]["model_label"].as_str() {
                            ui.label(
                                RichText::new(format!("Bound model: {model}"))
                                    .size(11.0)
                                    .color(ACCENT_GREEN),
                            );
                        }
                    });
                    ui.add_space(4.0);
                }
            }
        }

        ui.add_space(6.0);
        if section_header(ui, "Local CLI Agent Fallback (`agent.config`)", true) {
            ui.add_space(4.0);
            card_frame().show(ui, |ui| {
                let active_agent = self.agent_config["agent"].as_str().unwrap_or("codex");
                let active_model = self.agent_config["model"].as_str().unwrap_or("(default)");
                prop_row(ui, "Active CLI", |ui| {
                    status_badge(ui, active_agent, ACCENT_TEAL);
                });
                prop_row(ui, "Model", |ui| {
                    ui.label(
                        RichText::new(if active_model.is_empty() {
                            "(subscription default)"
                        } else {
                            active_model
                        })
                        .size(11.5)
                        .color(TEXT_PRIMARY),
                    );
                });
                if let Some(agents) = self.agent_config["agents"].as_array() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for a in agents.iter().filter(|a| a["installed"] == true) {
                            if let Some(name) = a["name"].as_str() {
                                status_badge(
                                    ui,
                                    name,
                                    if name == active_agent {
                                        ACCENT_GREEN
                                    } else {
                                        BORDER_STRONG
                                    },
                                );
                            }
                        }
                    });
                }
            });
        }

        let _ = BG_INPUT;
    }
}
