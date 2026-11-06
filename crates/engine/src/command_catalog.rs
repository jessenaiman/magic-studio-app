use crate::commands::CommandSpec;
use crate::commands::always;

// Import command handlers into this module so the `build()` array can reference them.
use crate::agent::{
    cmd_agent_config, cmd_agent_log, cmd_agent_models, cmd_agent_new_chat, cmd_agent_send,
    cmd_agent_session,
};
use crate::doctor::cmd_app_doctor;
use crate::project::{
    cmd_project_add_asset, cmd_project_close, cmd_project_list_assets, cmd_project_load_asset,
    cmd_project_new, cmd_project_open, cmd_project_remove_asset, cmd_project_save,
    cmd_project_update_asset,
};
use crate::workshop::{
    cmd_audio_analyze, cmd_project_search, cmd_query_list, cmd_video_init, cmd_video_server_start,
    cmd_video_server_stop, cmd_workshop_add, cmd_workshop_approve_draft, cmd_workshop_files,
    cmd_workshop_import_transcript, cmd_workshop_remove, cmd_workshop_set_audio,
    cmd_workshop_set_target, cmd_workshop_split_panels, cmd_workshop_status,
};

// ---------- CommandSpec + registry ----------

const fn is_long_running_id(id: &str) -> bool {
    matches!(
        id.as_bytes(),
        b"project.intake"
            | b"agent.send"
            | b"ui.eval"
            | b"words.design"
            | b"words.letter"
            | b"video.assemble"
            | b"video.init"
            | b"audio.analyze"
            | b"workshop.split_panels"
            | b"video.snapshot"
            | b"video.render"
            | b"image.generate"
            | b"video.frame"
            | b"video.export"
            | b"video.preview"
            | b"scribbles.generate"
            | b"scribbles.new"
            | b"scribbles.image"
            | b"scribbles.sheet"
            | b"scribbles.poses"
            | b"scribbles.actions"
            | b"scribbles.custom"
            | b"character.mark"
            | b"character.cut"
            | b"character.rig"
            | b"character.action"
            | b"character.frame"
            | b"character.render"
            | b"character.sprite"
            | b"character.shot"
            | b"character.set"
            | b"character.tripo.breakout"
            | b"character.tripo.rig"
            | b"animation.prepare_motion"
            | b"character.tripo.animate"
    )
}

macro_rules! cmd {
    ($id:literal, $label:literal, [$($m:literal),*], $sc:expr, $params:literal, $en:expr, $run:expr) => {
        CommandSpec { id: $id, label: $label, menu: &[$($m),*], shortcut: $sc, params: $params, help: "", example: "", enabled: $en, run: $run, journal: true, long_running: is_long_running_id($id) }
    };
}

macro_rules! query {
    ($id:literal, $label:literal, $params:literal, $run:expr) => {
        CommandSpec {
            id: $id,
            label: $label,
            menu: &[],
            shortcut: None,
            params: $params,
            help: "",
            example: "",
            enabled: always,
            run: $run,
            journal: false,
            long_running: false,
        }
    };
}

pub fn build() -> Vec<CommandSpec> {
    let mut specs = Vec::new();
    for section in [
        specs_skills_and_crew,
        specs_backstage,
        specs_ui_controls,
        specs_ui_inspection,
        specs_agent_and_sprocket,
        specs_project,
        specs_project_assets,
        specs_workshop_and_lyrics,
        specs_words_part1,
        specs_words_part2,
        specs_agent_and_image,
        specs_workshop_and_video,
        specs_video_and_intake,
        specs_character,
        specs_scribbles,
        specs_film,
        specs_storyboard,
        specs_diagnostics,
    ] {
        specs.extend(section());
    }
    specs
}

fn specs_skills_and_crew() -> Vec<CommandSpec> {
    vec![
        query!(
            "project.attach",
            "Attach Project Metadata Without Switching Studio",
            "{\"path\":\"existing .magic path\"}",
            crate::project::cmd_project_attach
        ),
        query!(
            "project.identity",
            "Canonical Project Identity",
            "{\"path\":\"existing .magic path\"}",
            crate::project::cmd_project_identity
        ),
        query!(
            "project.catalog",
            "Home Project Navigation History",
            "{}",
            crate::project_catalog::catalog
        ),
        cmd!(
            "project.root",
            "Inspect / Set Production Project Root",
            ["File"],
            None,
            "{\"root\":\"optional absolute production root; omitted to inspect\"}",
            always,
            crate::project_settings::command
        ),
        query!(
            "skills.list",
            "Registered knowledge skills (info blocks)",
            "{}",
            crate::skills_registry::cmd_skills_list
        ),
        query!(
            "crew.list",
            "Magic Studio MOPPETS and shared department AGENTS.md files",
            "{\"workspace\":\"optional workshop root\"}",
            crate::crew::cmd_list
        ),
        cmd!(
            "crew.save",
            "Save one department cast and shared instructions (revision guarded)",
            [],
            None,
            "{\"workspace\":\"workshop root from crew.list\",\"department\":\"creative|animation|production|backstage|review\",\"expected_revision\":\"from crew.list\",\"document\":\"department and instructions from crew.list\"}",
            always,
            crate::crew::cmd_save
        ),
        query!(
            "skills.info",
            "One registered skill's info block",
            "{\"id\":\"skill id\"}",
            crate::skills_registry::cmd_skills_info
        ),
        cmd!(
            "skills.limits",
            "Read / explicitly configure Learning block caps",
            [],
            None,
            "{\"max_words\":\"optional explicit cap\",\"max_steps\":\"optional explicit cap\"}",
            always,
            crate::skills_registry::cmd_skills_limits
        ),
        cmd!(
            "skills.diagram",
            "Archify view derived from Learning task records",
            [],
            None,
            "{\"kind\":\"production|correction\",\"render\":false}",
            always,
            crate::skills_registry::cmd_skills_diagram
        ),
        cmd!(
            "skills.edit",
            "Save Learning Markdown (revision guarded)",
            [],
            None,
            "{\"id\":\"block id\",\"markdown\":\"complete Markdown\",\"expected_revision\":\"from skills.info\"}",
            always,
            crate::skills_registry::cmd_skills_edit
        ),
    ]
}

fn specs_backstage() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "skills.import",
            "Import a cited draft reference (never execute)",
            [],
            None,
            "{\"id\":\"new block id\",\"source\":\"https URL or existing skill/package path\",\"category\":\"optional\"}",
            always,
            crate::skills_registry::cmd_skills_import
        ),
        cmd!(
            "skills.remove",
            "Remove Learning block reversibly",
            [],
            None,
            "{\"id\":\"block id\",\"expected_revision\":\"from skills.info\"}",
            always,
            crate::skills_registry::cmd_skills_remove
        ),
        cmd!(
            "skills.undo",
            "Restore removed Learning block",
            [],
            None,
            "{\"id\":\"block id\",\"expected_revision\":\"from remove receipt\"}",
            always,
            crate::skills_registry::cmd_skills_undo
        ),
        cmd!(
            "skills.add",
            "Register / update one knowledge skill",
            [],
            None,
            "{\"id\":\"skill id\",\"name\":\"display name\",\"description\":\"what it does\",\"status\":\"researching|implemented|not_implemented\",\"source\":\"url or reference\",\"implemented_at\":[\"command ids\"],\"implemented_in\":[\"file paths\"],\"integration_notes\":\"where it plugs in\"}",
            always,
            crate::skills_registry::cmd_skills_add
        ),
        query!(
            "backstage.changes",
            "Discover Saved-Project Change Notifications",
            "{}",
            crate::change_marker::status
        ),
        query!(
            "backstage.events",
            "Recent Activity Receipts",
            "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}",
            crate::activity::events
        ),
        query!(
            "booth.events",
            "Recent Activity Receipts (alias of backstage.events)",
            "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}",
            crate::activity::events
        ),
        query!(
            "center.events",
            "Recent Activity Receipts (alias of backstage.events)",
            "{\"after\":0,\"limit\":20,\"project\":\"optional absolute project filter\"}",
            crate::activity::events
        ),
    ]
}

fn requires_ui(_: &mut crate::Session, _: &serde_json::Value) -> crate::Result<serde_json::Value> {
    Err(crate::EngineError::InvalidParams(
        "requires a live Magic Studio window".into(),
    ))
}

fn specs_ui_controls() -> Vec<CommandSpec> {
    vec![
        query!("ui.state", "Live window state", "{}", requires_ui),
        query!(
            "ui.elements",
            "Visible controls",
            "{\"region\":\"optional CSS selector\"}",
            requires_ui
        ),
        query!(
            "ui.unconfigured",
            "Unconfigured controls",
            "{}",
            requires_ui
        ),
        cmd!(
            "ui.click",
            "Click a control",
            [],
            None,
            "{\"id\":\"element ID\",\"instance\":0,\"files\":[]}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.set",
            "Set a control",
            [],
            None,
            "{\"id\":\"element ID\",\"value\":\"value\",\"instance\":0}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.text_edit",
            "Edit the focused text field using native WebKit (Linux)",
            [],
            None,
            "{\"action\":\"edit.undo, edit.redo, edit.cut, edit.copy, edit.paste or edit.select_all\"}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.menu",
            "Open a shared File or text context menu",
            [],
            None,
            "{\"menu\":\"file or text\",\"x\":0,\"y\":48}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.key",
            "Press a key",
            [],
            None,
            "{\"key\":\"key name\",\"id\":\"optional element ID\"}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.drag",
            "Drag a control",
            [],
            None,
            "{\"id\":\"optional element ID\",\"from\":[],\"to\":[]}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.look",
            "Capture window and state",
            [],
            None,
            "{\"output\":\"optional PNG path\",\"region\":\"optional CSS selector\",\"click\":\"optional element ID\"}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.screenshot",
            "Capture window",
            [],
            None,
            "{\"output\":\"optional PNG path\"}",
            always,
            requires_ui
        ),
    ]
}

fn specs_ui_inspection() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "ui.focus",
            "Focus window",
            [],
            None,
            "{\"animation_only\":\"optional true: also show only the Animation workspace\"}",
            always,
            requires_ui
        ),
        cmd!(
            "ui.eval",
            "Evaluate window JavaScript",
            [],
            None,
            "{\"js\":\"JavaScript body\"}",
            always,
            requires_ui
        ),
        cmd!(
            "app.quit",
            "Guarded window close",
            [],
            None,
            "{}",
            always,
            requires_ui
        ),
        query!(
            "ui.feedback",
            "Live Studio Feedback",
            "{\"project\":\"optional canonical project path\"}",
            requires_ui
        ),
        query!(
            "ui.components",
            "Rendered Studio Components",
            "{\"region\":\"#view-project\",\"filter\":\"optional ID filter\",\"limit\":30}",
            requires_ui
        ),
        query!(
            "ui.controls",
            "Studio control telemetry",
            "{\"region\":\"optional region\",\"offset\":0,\"limit\":64}",
            requires_ui
        ),
        cmd!(
            "ui.attention",
            "Request attention at a Studio panel",
            ["Window"],
            None,
            "{\"project\":\"exact open .magic path\",\"id\":\"registered panel id\",\"reason\":\"what needs attention\",\"clear\":false}",
            always,
            requires_ui
        ),
        query!(
            "studio.cues",
            "Cues at a Song Time / Uses of a Clip (read-only)",
            "{\"project\":\"existing .magic path\",\"at\":43.2,\"clip\":\"clip id; give at or clip\"}",
            crate::studio_cues::cues
        ),
        cmd!(
            "project.migrate",
            "Migrate a Legacy .acproj to .magic",
            ["File"],
            None,
            "{\"path\":\"legacy .acproj file\"}",
            always,
            crate::project::cmd_project_migrate
        ),
        cmd!(
            "workspace.open",
            "Open a File in Its Craft Editor",
            ["File"],
            None,
            "{\"path\":\"file path\"}",
            always,
            crate::workspace::cmd_workspace_open
        ),
        cmd!(
            "photo.exec",
            "Run PhotoCraft Commands In-Process (.pcraft, .psd, .png, .webp)",
            ["File"],
            None,
            "{\"path\":\"image or .pcraft/.psd\",\"id\":\"document.inspect|image.imageRotation.*|image.adjustments.*|layer.*\",\"params\":{},\"save\":\"optional output path\"}",
            always,
            crate::workspace::cmd_photo_exec
        ),
        cmd!(
            "effect.exec",
            "Run EffectCraft Commands In-Process (.ecproj)",
            ["Animation"],
            None,
            "{\"path\":\".ecproj path\",\"id\":\"comp.settings|comp.workArea|layer.newText|layer.newShape|edit.splitLayer|edit.history.list\",\"params\":{},\"save\":true}",
            always,
            crate::workspace::cmd_effect_exec
        ),
        cmd!(
            "vector.exec",
            "Run VectorCraft Info, Convert, or Commands (.svg, .vectorcraft, .ai, .pdf, .png)",
            ["File"],
            None,
            "{\"path\":\"asset path\",\"mode\":\"info|convert|run\",\"out\":\"optional output path\",\"scale\":2,\"outline_text\":false,\"id\":\"optional command\",\"params\":{}}",
            always,
            crate::workspace::cmd_vector_exec
        ),
    ]
}

fn specs_agent_and_sprocket() -> Vec<CommandSpec> {
    vec![
        query!(
            "agent.session",
            "Current Chat Session",
            "{}",
            cmd_agent_session
        ),
        cmd!(
            "sprocket.send",
            "Send a message to Fidget (the app's in-app agent)",
            ["Agent"],
            None,
            "{\"message\":\"text\",\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":true}",
            always,
            crate::agent::cmd_sprocket_send
        ),
        query!(
            "sprocket.status",
            "Fidget Session Status",
            "{}",
            cmd_agent_session
        ),
        cmd!(
            "agent.persist",
            "Persist the Active Chat Selection to Shared Settings",
            ["Agent"],
            None,
            "{}",
            always,
            crate::agent::cmd_agent_persist
        ),
        query!(
            "sprocket.models",
            "Fidget Model Catalog (local first, then free)",
            "{}",
            crate::agent::cmd_sprocket_models
        ),
        cmd!(
            "sprocket.use",
            "Switch Fidget's Model",
            ["Agent"],
            None,
            "{\"model\":\"provider/id, e.g. ollama/llama3.2:latest\"}",
            always,
            crate::agent::cmd_sprocket_use
        ),
        query!(
            "sprocket.log",
            "Fidget Transcript / Reply Retrieval",
            "{\"refresh\":false}",
            cmd_agent_log
        ),
        query!(
            "agent.config",
            "Current Window Chat Selection",
            "{\"selection\":{\"model\":\"optional active model; shared defaults unchanged\"}}",
            cmd_agent_config
        ),
        query!(
            "agent.models",
            "Available Chat Models",
            "{\"agent\":\"optional catalog adapter: opencode\"}",
            cmd_agent_models
        ),
        cmd!(
            "agent.new_chat",
            "New Agent Chat",
            ["Agent"],
            None,
            "{\"free_only\":false,\"image_task\":false}",
            always,
            cmd_agent_new_chat
        ),
    ]
}

fn specs_project() -> Vec<CommandSpec> {
    vec![
        query!(
            "project.list_assets",
            "List Project Assets",
            "{}",
            cmd_project_list_assets
        ),
        cmd!(
            "project.new",
            "New Project",
            ["File"],
            Some("Cmd+N"),
            "{\"path\":\"folder/name.magic\",\"mode\":\"project (song, default) | animation (character study: Animation only, reusable in projects)\",\"audio\":\"optional song audio\",\"overwrite\":false}",
            always,
            cmd_project_new
        ),
        cmd!(
            "project.open",
            "Open Project",
            ["File"],
            Some("Cmd+O"),
            "{\"path\":\"folder/name.magic\"}",
            always,
            cmd_project_open
        ),
        cmd!(
            "project.save",
            "Save Project",
            ["File"],
            Some("Cmd+S"),
            "{\"path\":\"optional_save_as.magic\"}",
            always,
            cmd_project_save
        ),
        cmd!(
            "project.close",
            "Close Project",
            ["File"],
            None,
            "{}",
            always,
            cmd_project_close
        ),
    ]
}

fn specs_project_assets() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "project.add_asset",
            "Connect Project Asset",
            ["Project Assets"],
            None,
            "{\"path\":\"image_or_manifest\",\"name\":\"optional\",\"id\":\"optional\",\"kind\":\"auto|image|poses|turnaround|cutout_rig\",\"character\":\"name (outside image -> assets/characters/<name>/parts/)\",\"background\":\"name (outside image -> assets/backgrounds/<name>/)\",\"image\":\"name (outside image -> assets/images/<name>/)\",\"remove_background\":true}",
            always,
            cmd_project_add_asset
        ),
        cmd!(
            "project.remove_asset",
            "Remove Project Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\"}",
            always,
            cmd_project_remove_asset
        ),
        cmd!(
            "project.update_asset",
            "Rename / Reorder Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\",\"name\":\"optional\",\"index\":0}",
            always,
            cmd_project_update_asset
        ),
        cmd!(
            "project.load_asset",
            "Load Connected Asset",
            ["Project Assets"],
            None,
            "{\"id\":\"asset_id\",\"reload\":false}",
            always,
            cmd_project_load_asset
        ),
    ]
}

fn specs_workshop_and_lyrics() -> Vec<CommandSpec> {
    vec![
        query!(
            "workshop.status",
            "Workshop Stages",
            "{}",
            cmd_workshop_status
        ),
        query!(
            "song.lyrics",
            "Song Lyrics (Artist Sheet or Transcript)",
            "{}",
            crate::cmd::words::lyrics::cmd_song_lyrics
        ),
        cmd!(
            "song.save",
            "Save Song Details",
            ["Song"],
            None,
            "{\"title\":\"optional text\",\"album\":\"optional text\",\"track_number\":\"optional positive integer or null to clear\",\"lyrics\":\"optional plain text, separate from timed or artist lyrics\",\"notes\":\"optional text\"}",
            always,
            crate::song_metadata::save
        ),
    ]
}

fn specs_words_part1() -> Vec<CommandSpec> {
    vec![
        query!(
            "words.align",
            "Artist Words vs Timing Reference",
            "{}",
            crate::cmd::words::cmd_words_align
        ),
        query!(
            "words.read",
            "Words Animations",
            "{\"animation_id\":\"optional\"}",
            crate::cmd::words::cmd_words_read
        ),
        query!(
            "words.studio_track",
            "Lyric Clips in the Video",
            "{}",
            crate::cmd::words::cmd_words_studio_track
        ),
        cmd!(
            "words.lyrics_view",
            "Stage, Lyrics-Only or Scene-Only View of the Video",
            ["Design"],
            None,
            "{\"show\":\"still (one saved scene image plus audio and lyrics; requires animation_id), stage, lyrics or scene\"}",
            always,
            crate::cmd::words::cmd_words_lyrics_view
        ),
        query!(
            "words.effects",
            "Effects from the HyperFrames Registry",
            "{\"kind\":\"optional: text, drawing or background\"}",
            crate::cmd::words::cmd_words_effects
        ),
        cmd!(
            "words.effect_install",
            "Install a Registry Effect",
            ["Design"],
            None,
            "{\"name\":\"per-word-rise\"}",
            always,
            crate::cmd::words::cmd_words_effect_install
        ),
        cmd!(
            "words.captions",
            "Captions by the Artist's Lines",
            ["Design"],
            None,
            "{\"by\":\"section (default) or line\",\"out\":\"optional .srt path; default lyrics.captions.srt in the project\",\"to\":\"video: the video's caption track shows the printed lines (earlier source kept as captions.vN.html)\"}",
            always,
            crate::cmd::words::cmd_words_captions
        ),
        cmd!(
            "words.track",
            "Lyric Line in the Video",
            ["Design"],
            None,
            "{}",
            always,
            crate::cmd::words::cmd_words_track
        ),
        cmd!(
            "words.design",
            "Design Every Sung Line",
            ["Design"],
            None,
            "{\"recipe\":\"optional; default lyrics-design.json in the project (palette, band, hero words and their moves)\",\"apply\":\"true (default) uses each section in the video\",\"section\":\"optional animation id\",\"layout\":\"optional kinetic layout\",\"entrance\":\"optional kinetic entrance\",\"overlay\":\"optional overlay layer\",\"seed\":\"varies layouts and entrances per section\",\"kinetic\":\"false keeps static line designs\"}",
            always,
            crate::cmd::words::cmd_words_design
        ),
    ]
}

fn specs_words_part2() -> Vec<CommandSpec> {
    let mut out = vec![
        cmd!(
            "video.assemble",
            "Assemble and Export the Film (editor)",
            ["Studio"],
            None,
            "{\"backgrounds\":\"optional [{\\\"asset\\\":\\\"registered image id (or path)\\\",\\\"from\\\":0}]; defaults to storyboard scene backgrounds\",\"characters\":\"optional [{\\\"clip\\\":\\\"character.shot .mov\\\",\\\"from\\\":seconds}] on V2\",\"lettering\":\"optional; default lettering/lyrics.mov (V3)\",\"fps\":30,\"export\":\"true (default) writes exports/<song>.mp4\"}",
            always,
            crate::assembly::cmd_video_assemble
        ),
        cmd!(
            "words.letter",
            "Letter and Animate the Lyrics (effects app)",
            ["Design"],
            None,
            "{\"recipe\":\"optional; default lyrics-design.json in the project\",\"render\":\"true (default) renders lettering/lyrics.mov with transparency\"}",
            always,
            crate::cmd::words::lettering::cmd_words_letter
        ),
        cmd!(
            "words.apply",
            "Use Designed Lyrics in the Video",
            ["Design"],
            None,
            "{\"animation_id\":\"words-chorus-1\",\"use\":true}",
            always,
            crate::cmd::words::cmd_words_apply
        ),
        cmd!(
            "edit.undo",
            "Undo",
            ["Edit"],
            Some("Cmd+Z"),
            "{}",
            always,
            crate::session::undo::cmd_edit_undo
        ),
        cmd!(
            "edit.redo",
            "Redo",
            ["Edit"],
            Some("Cmd+Shift+Z"),
            "{}",
            always,
            crate::session::undo::cmd_edit_redo
        ),
        cmd!(
            "words.save",
            "Save Words Animation",
            ["Design"],
            None,
            "{\"animation_id\":\"words-chorus-1\",\"expected_source_revision\":0,\"style\":{\"size\":120},\"words\":[{\"word_id\":\"ch1-l1/w5\",\"action\":\"cry\",\"emphasis\":true}]}",
            always,
            crate::cmd::words::cmd_words_save
        ),
        cmd!(
            "words.link",
            "Link Drawing to a Lyric Word",
            ["Design"],
            None,
            "{\"animation_id\":\"source section\",\"index\":0,\"word_id\":\"target word ID\",\"target_animation_id\":\"optional target section\",\"expected_source_revision\":0,\"expected_target_revision\":0}",
            always,
            crate::cmd::words::cmd_words_link
        ),
        cmd!(
            "words.compose",
            "Compose Words Track (HyperFrames)",
            ["Design"],
            None,
            "{\"animation_id\":\"words-chorus-1\",\"instance_id\":\"optional: isolated composition for a repeated placement\"}",
            always,
            crate::cmd::words::cmd_words_compose
        ),
    ];
    out.extend(specs_workshop_queries());
    out
}

fn specs_workshop_queries() -> Vec<CommandSpec> {
    vec![
        query!(
            "workshop.files",
            "Workshop Files",
            "{\"filter\": \"optional substring\", \"group\": \"optional audio|liner_notes|transcript|storyboard|assets|other\"}",
            cmd_workshop_files
        ),
        query!(
            "project.search",
            "Search Project, Files, Storyboard & Commands",
            "{\"query\": \"search text\"}",
            cmd_project_search
        ),
    ]
}

fn specs_agent_and_image() -> Vec<CommandSpec> {
    vec![
        query!(
            "agent.log",
            "Agent Transcript / Reply Retrieval",
            "{\"refresh\":false}",
            cmd_agent_log
        ),
        cmd!(
            "agent.send",
            "Send to the app agent",
            ["Agent"],
            None,
            "{\"message\":\"text\",\"attachments\":[\"png\"],\"new_chat\":true,\"session\":\"optional expected existing session; requires new_chat=false\",\"wait\":300,\"free_only\":false,\"image_task\":false}",
            always,
            cmd_agent_send
        ),
        cmd!(
            "image.generate",
            "Generate Image (Codex)",
            ["Project Assets"],
            None,
            "{\"prompt\":\"image description\",\"provider\":\"codex (subscription default) or http (from settings.json / magic_IMAGE_ENDPOINT)\",\"model\":\"optional model override (omit to use Codex subscription default)\",\"quality\":\"auto\",\"reference_asset\":\"optional registered reference PNG asset ID\",\"references\":[\"project-relative or absolute project PNG; max 5 including reference_asset\"],\"poses\":[\"optional 2 to 8 ordered pose descriptions; requires a reference\"],\"rig_parts\":[{\"id\":\"part id\",\"description\":\"what the part is\",\"parent\":\"optional part id\",\"pivot\":\"optional [u,v] in the part, 0-1\"}],\"character\":\"optional registered source character ID\",\"scene\":1,\"animation\":\"optional intended action name\",\"transparent_background\":true,\"name\":\"optional asset name\",\"timeout_s\":300,\"project\":\"optional existing .magic for CLI targeting\"}",
            always,
            crate::image_generation::generate
        ),
    ]
}

fn specs_workshop_and_video() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "workshop.set_audio",
            "Set Song Audio",
            ["Workshop"],
            None,
            "{\"path\": \"song audio file; creates the HyperFrames project and starts Studio\", \"resolve_video_cli\": \"optional absolute executable\"}",
            always,
            cmd_workshop_set_audio
        ),
        cmd!(
            "video.place_track",
            "Place a rendered track in HyperFrames",
            ["Video"],
            None,
            "{\"id\":\"clip id\",\"path\":\"video file\",\"start\":0,\"duration\":2,\"rect\":[0,0,300,500],\"scene\":14,\"generation_receipt\":\"optional native image generation.json; links source poses and character\",\"character\":\"optional registered source motion asset; requires track\",\"track\":\"optional source motion track; requires character\",\"playback_rate\":1}",
            always,
            crate::video_target::cmd_video_place_track
        ),
        cmd!(
            "workshop.set_target",
            "Set Video Target",
            ["Workshop"],
            None,
            "{\"target\": \"hyperframes\"}",
            always,
            cmd_workshop_set_target
        ),
        cmd!(
            "workshop.add",
            "Add Liner Notes / Panels / Assets",
            ["Workshop"],
            None,
            "{\"kind\": \"liner_notes|panels|assets\", \"paths\": [\"file\"]}",
            always,
            cmd_workshop_add
        ),
        cmd!(
            "workshop.split_panels",
            "Split Storyboard Grid (PhotoCraft)",
            ["Workshop"],
            None,
            "{\"image\": \"storyboard grid\", \"columns\": 3, \"rows\": 2, \"inset\": 8, \"prompts\": \"optional prompt.md with a numbered panel list\"}",
            always,
            cmd_workshop_split_panels
        ),
        cmd!(
            "workshop.import_transcript",
            "Import Transcript",
            ["Workshop"],
            None,
            "{\"path\": \"transcript.json\", \"captions\": \"optional original SRT; without it use provisional timed-word fallback\", \"audiomap\": \"optional existing audiomap.json\", \"replace\": false, \"filmcraft_project\": \"optional .fcproj\"}",
            always,
            cmd_workshop_import_transcript
        ),
        cmd!(
            "workshop.approve_draft",
            "Approve Storyboard",
            ["Workshop"],
            None,
            "{\"approved\": true}",
            always,
            cmd_workshop_approve_draft
        ),
        cmd!(
            "workshop.remove",
            "Remove Workshop File",
            ["Workshop"],
            None,
            "{\"path\": \"file in the project folder (moved to trash), or the song audio (unlinked only)\"}",
            always,
            cmd_workshop_remove
        ),
    ]
}

fn specs_video_and_intake() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "video.init",
            "Initialize Target Video Project",
            ["Workshop"],
            None,
            "{\"target\": \"optional hyperframes\", \"resolve_video_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH, no npx install\", \"captions\": true, \"assemble\": false, \"audiomap\": \"legacy parameter; retired assembly is rejected without changing the project\"}",
            always,
            cmd_video_init
        ),
        cmd!(
            "video.server_start",
            "Start Video Editor Web Server",
            ["Workshop"],
            None,
            "{\"target\": \"optional hyperframes\", \"port\": \"optional; HyperFrames selects an available port by default\", \"resolve_video_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}",
            always,
            cmd_video_server_start
        ),
        cmd!(
            "video.server_stop",
            "Stop Video Editor Web Server",
            ["Workshop"],
            None,
            "{}",
            always,
            cmd_video_server_stop
        ),
        cmd!(
            "video.snapshot",
            "Take one exact frame from HyperFrames",
            ["Video"],
            None,
            "{\"target\": \"optional hyperframes\", \"at\": \"seconds or array of seconds\", \"out\": \"output PNG path for one time; directory for multiple times\", \"resolve_video_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}",
            always,
            crate::video_target::cmd_video_snapshot
        ),
        cmd!(
            "video.render",
            "Render HyperFrames composition",
            ["Video"],
            None,
            "{\"target\": \"optional hyperframes\", \"out\": \"output path\", \"fps\": \"frames per second (default 24)\", \"workers\": \"bounded worker count (default 4, max 16)\", \"start/end\": \"unsupported by official HyperFrames CLI; any supplied value is rejected\", \"resolve_video_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH\"}",
            always,
            crate::video_target::cmd_video_render
        ),
        cmd!(
            "audio.analyze",
            "Analyze Song (HyperFrames)",
            ["File"],
            None,
            "{\"audio\": \"optional, default: the project's song\", \"overwrite\": false, \"analyze_beats\": true, \"model\": \"large-v3\", \"resolve_video_cli\": \"optional absolute official executable; otherwise HYPERFRAMES_CLI or PATH; no npx install\"}",
            always,
            cmd_audio_analyze
        ),
    ]
}

fn specs_diagnostics() -> Vec<CommandSpec> {
    vec![
        query!(
            "mayhem.evidence",
            "Mayhem · Read saved analyzer reports and scoped Herdr activity",
            "{\"path\":\"optional source checkout\",\"workspace_id\":\"optional verified Herdr workspace ID\"}",
            crate::mayhem::evidence
        ),
        cmd!(
            "resources.workflow",
            "Shared Creation Loop",
            ["Resources"],
            None,
            "steps=true for terminal steps; section=production|revision; edit with id, field, previous and value",
            always,
            crate::creation_loop::workflow
        ),
        query!(
            "app.doctor",
            "Doctor: CLI Availability and Video Server Metadata",
            "{\"clean\": false}",
            cmd_app_doctor
        ),
        query!(
            "app.errors",
            "Recorded Runtime Errors",
            "{\"project\":\"optional absolute .magic path; defaults to selected project\",\"after\":0,\"limit\":50}",
            crate::activity::errors
        ),
        query!(
            "command.list",
            "List Commands",
            "{\"filter\":\"optional command ID or search text\"}",
            cmd_query_list
        ),
    ]
}

/// Characters: Scribbles marks, PhotoCraft cuts, EffectCraft rigs, animates and renders.
fn specs_character() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "character.shot",
            "Place a Sprite in a Scene Shot (EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"sprite\":\"sprite name\",\"name\":\"optional shot name\",\"duration\":4,\"from\":[960,780],\"to\":[960,780],\"scale\":100,\"mirror\":false,\"hop_period\":\"optional seconds per jump (the sprite cycle)\",\"height\":140}",
            always,
            crate::character::cmd_character_shot
        ),
        cmd!(
            "character.sprite",
            "Sprite Cycle from a Scribbles Sheet (PhotoCraft, EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"action\":\"walk|run|jump|wave|turn\",\"sheet\":\"Scribbles action sheet PNG\",\"name\":\"optional sprite name; default the action\",\"columns\":4,\"rows\":2,\"fps\":12,\"duration\":4,\"mirror\":false}",
            always,
            crate::character::cmd_character_sprite
        ),
        query!(
            "animation.motion_options",
            "Single-Subject EffectCraft Motion Options",
            "{}",
            crate::character::review::cmd_motion_options
        ),
        cmd!(
            "animation.prepare_motion",
            "Prepare One Character in EffectCraft",
            ["Animation"],
            None,
            r#"{"character":"id","clip":"exact GLB clip","option":"baseline|paced","duration":3}"#,
            always,
            crate::character::review::cmd_prepare_motion
        ),
        query!(
            "motion.theatre.read",
            "Read project timeline",
            r#"{"expected_project":"path"}"#,
            crate::project::theatre_state
        ),
        cmd!(
            "motion.theatre.save",
            "Save project timeline",
            [],
            None,
            r#"{"expected_project":"path","state":{}}"#,
            always,
            crate::project::save_theatre_state
        ),
        query!(
            "animation.review",
            "Animation Review Models and Saved Revisions",
            "{}",
            crate::character::review::cmd_review
        ),
        cmd!(
            "animation.save_revision",
            "Save Animation Revision",
            ["Animation"],
            None,
            r#"{"expected_project":"absolute .magic path","expected_revision":0,"character":"id","clip":"clip name","parent":null,"draft":{"trim_start":0,"trim_end":1,"speed":1,"loop":false,"in_place":true,"keys":[],"issues":[],"notes":"","status":"needs-visual-review"}}"#,
            always,
            crate::character::review::cmd_save_revision
        ),
        query!(
            "character.meta",
            "Character Views, Parts and Actions",
            "{\"character\":\"id\"}",
            crate::character::cmd_character_meta
        ),
        cmd!(
            "character.save_marks",
            "Save Adjusted Marks",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"parts\":[{\"id\":\"arm_L\",\"parent\":\"torso\",\"joint\":[0,0],\"outline\":[[0,0]]}]}",
            always,
            crate::character::cmd_character_save_marks
        ),
        query!(
            "character.list",
            "Characters and Their App Files",
            "{}",
            crate::character::cmd_character_list
        ),
        query!(
            "character.actions",
            "Actions in the Shared Pose Table",
            "{}",
            crate::character::cmd_character_actions
        ),
        cmd!(
            "character.mark",
            "Mark Parts and Joints (Scribbles)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"image\":\"drawing.png\",\"timeout_s\":600}",
            always,
            crate::character::cmd_character_mark
        ),
        cmd!(
            "character.cut",
            "Cut Parts into Layers (PhotoCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\"}",
            always,
            crate::character::cmd_character_cut
        ),
        cmd!(
            "character.rig",
            "Rig the Puppet (EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\"}",
            always,
            crate::character::cmd_character_rig
        ),
        cmd!(
            "character.action",
            "Key an Action (EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"action\":\"idle|walk_down|walk_up|walk_left|walk_right|jump|turn|wave\",\"at\":0,\"times\":1}",
            always,
            crate::character::cmd_character_action
        ),
        cmd!(
            "character.render",
            "Render Frames or a Clip (EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"sprite\":\"optional sprite name instead of view\",\"start\":0,\"end\":1,\"fps\":24,\"format\":\"frames|clip\"}",
            always,
            crate::character::cmd_character_render
        ),
        query!(
            "character.frame",
            "Render a Frame (EffectCraft)",
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"time\":0,\"max_side\":900}",
            crate::character::cmd_character_frame
        ),
        cmd!(
            "character.set",
            "Key One Transform (EffectCraft)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"part\":\"layer\",\"prop\":\"position|rotation|scale|anchor|opacity\",\"value\":0,\"time\":0}",
            always,
            crate::character::cmd_character_set
        ),
        query!(
            "character.open",
            "Open in EffectCraft, PhotoCraft or Blender",
            "{\"character\":\"id\",\"view\":\"front|back|side\",\"app\":\"effectcraft|photocraft|blender\"}",
            crate::character::cmd_character_open
        ),
        query!(
            "character.tripo.catalog",
            "Tripo 3D Breakout Templates, Rig Specs and 101 Animation Presets",
            "{\"category\":\"optional basic|interactive|scene_specific|emotional|combat\"}",
            crate::character::cmd_tripo_catalog
        ),
        cmd!(
            "character.tripo.breakout",
            "Tripo Post-Image Breakout (Template, Auto-Split Cutout, Multiview or 3D Segmentation)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"kind\":\"character_extraction|asset_extraction|t_pose|character_completion|head_extraction|3d_enhance|split_image|multiview_images|upscale_image|ai_segmentation|mesh_completion\",\"provider\":\"tripo|google_flow|codex_luna\",\"reference\":\"optional source image\",\"prompt\":\"optional prompt\",\"granularity\":\"low|medium|high\",\"completion_mode\":\"quick_cap|ai_completion\"}",
            always,
            crate::character::cmd_tripo_breakout
        ),
        cmd!(
            "character.tripo.rig",
            "Tripo 3D Pre-Rig Check and Auto-Rigging Setup",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"rig_type\":\"biped|quadruped|hexapod|octopod|avian|serpentine|aquatic|others\",\"skeleton_preset\":\"mixamo|actorcore|unreal|vrm|unity\",\"topology\":\"quad|triangle\",\"face_count\":10000,\"t_pose_ready\":true,\"model\":\"optional .glb path\"}",
            always,
            crate::character::cmd_tripo_rig
        ),
        cmd!(
            "character.tripo.animate",
            "Tripo 3D Retarget Preset or Multi-Stage Motion (.glb + .anim/.fbx)",
            ["Animation"],
            None,
            "{\"character\":\"id\",\"preset\":\"optional preset name (e.g. wave_goodbye_01, idle, walk, bow)\",\"prompt\":\"optional AI text-to-motion prompt\",\"stages\":[{\"name\":\"Start\",\"prompt\":\"action\",\"duration_seconds\":2,\"waypoints\":[[0,0],[0,0]]}],\"name\":\"optional clip name\",\"rig_type\":\"biped\",\"animate_in_place\":true,\"fbx_preset\":\"blender|3dsmax|maya|unreal|unity\",\"format\":\"fbx|glb\",\"texture_size\":\"2K\",\"glb\":\"optional .glb\",\"fbx\":\"optional .fbx\",\"blend\":\"optional .blend\",\"preview\":\"optional .mp4\"}",
            always,
            crate::character::cmd_tripo_animate
        ),
    ]
}

/// Scribbles: structured drawing requests, one template per kind of animation asset.
fn specs_scribbles() -> Vec<CommandSpec> {
    vec![
        query!(
            "fidget.prepare",
            "Prepare Final Drawing Prompt and References (Fidget)",
            r#"{"kind":"new|image|sheet|poses|actions|custom (default custom)","prompt":"character or drawing","reference":"image path or array","notes":"optional","action":"walk|run|jump|wave|turn","provider":"optional","model":"optional","character":"optional id","project":"optional .magic","name":"optional","timeout_s":900}"#,
            crate::fidget::cmd_fidget_prepare
        ),
        cmd!(
            "scribbles.generate",
            "Generate Final Drawing (Scribbles)",
            ["Scribbles"],
            None,
            r#"{"prompt":"final drawing instructions (unchanged)","references":["optional project PNG paths; max 5"],"provider":"optional; settings scribbles.provider or image provider","model":"optional; settings scribbles.model or gpt-6-luna","project":"optional .magic","character":"optional id","transparent_background":true,"timeout_s":900,"name":"optional","fidget":"optional preparation metadata"}"#,
            always,
            crate::scribbles::cmd_scribbles_generate
        ),
        query!(
            "scribbles.templates",
            "Scribbles Templates",
            "{}",
            crate::scribbles::cmd_scribbles_templates
        ),
        cmd!(
            "scribbles.new",
            "New Character (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\"}",
            always,
            crate::scribbles::cmd_scribbles_new
        ),
        cmd!(
            "scribbles.image",
            "Redraw for Animation (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\"}",
            always,
            crate::scribbles::cmd_scribbles_image
        ),
        cmd!(
            "scribbles.sheet",
            "Turnaround Sheet (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\"}",
            always,
            crate::scribbles::cmd_scribbles_sheet
        ),
        cmd!(
            "scribbles.poses",
            "Pose Sheet (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\"}",
            always,
            crate::scribbles::cmd_scribbles_poses
        ),
        cmd!(
            "scribbles.actions",
            "Action Frames (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\",\"action\":\"walk|run|jump|wave|turn\"}",
            always,
            crate::scribbles::cmd_scribbles_actions
        ),
        cmd!(
            "scribbles.custom",
            "Custom Drawing (Scribbles)",
            ["Scribbles"],
            None,
            "{\"reference\":\"character image (needed except for new/custom)\",\"prompt\":\"the character (new) or the drawing (custom)\",\"notes\":\"optional\",\"model\":\"optional; default gpt-6-luna or settings scribbles.model\",\"character\":\"optional id\",\"name\":\"optional\"}",
            always,
            crate::scribbles::cmd_scribbles_custom
        ),
    ]
}

/// The film in FilmCraft: watch a preview in Studio, check frames, export, or edit in FilmCraft.
fn specs_storyboard() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "storyboard.new",
            "New Storyboard from the Neutral Template",
            ["Storyboard"],
            None,
            "{\"path\":\"/absolute/storyboard.designcraft; default storyboard.designcraft beside the open project\",\"overwrite\":false}",
            always,
            crate::cmd::storyboard::cmd_storyboard_new
        ),
        query!(
            "storyboard.scenes",
            "Storyboard Scenes and Their Text Fields",
            "{\"path\":\"optional storyboard.designcraft\"}",
            crate::cmd::storyboard::cmd_storyboard_scenes
        ),
        query!(
            "storyboard.cues",
            "The Song's Verse and Chorus, With Each Line's Timing",
            "{\"lyrics\":\"canonical lyrics sheet .json\",\"transcript\":\"optional transcript .json for timings\",\"sections\":\"optional comma list; default every verse* and chorus* section\",\"per\":1}",
            crate::cmd::storyboard::cmd_storyboard_cues
        ),
        cmd!(
            "storyboard.lyrics",
            "Insert the Song's Lyrics Into the Scene Pages, Locked",
            ["Storyboard"],
            None,
            "{\"path\":\"optional storyboard.designcraft\",\"lyrics\":\"canonical lyrics sheet .json\",\"transcript\":\"optional transcript .json for timings\",\"sections\":\"optional comma list; default every verse* and chorus* section\",\"per\":1}",
            always,
            crate::cmd::storyboard::cmd_storyboard_lyrics
        ),
        cmd!(
            "storyboard.run",
            "Run a Command Script Against the Storyboard",
            ["Storyboard"],
            None,
            "{\"path\":\"optional storyboard.designcraft\",\"script\":\"script text\",\"script_file\":\"optional script file\"}",
            always,
            crate::cmd::storyboard::cmd_storyboard_run
        ),
        query!(
            "storyboard.check",
            "Fields Still Holding the Template Default, With the Command to Fill Each",
            "{\"path\":\"optional storyboard.designcraft\"}",
            crate::cmd::storyboard::cmd_storyboard_check
        ),
        query!(
            "storyboard.pages",
            "Storyboard Pages, Their Archetype, Section and Slots",
            "{\"path\":\"optional storyboard.designcraft\"}",
            crate::cmd::storyboard::cmd_storyboard_pages
        ),
        query!(
            "storyboard.slots",
            "The Labelled Slots of One Storyboard Page",
            "{\"path\":\"optional\",\"page\":0,\"label\":\"optional slot name\",\"scene\":1}",
            crate::cmd::storyboard::cmd_storyboard_slots
        ),
        cmd!(
            "storyboard.set",
            "Edit a Page: Text, Images, Contact Strip",
            ["Storyboard"],
            None,
            "{\"path\":\"optional\",\"page\":0,\"label\":\"optional slot name\",\"scene\":1,\"fields\":{\"label\":\"text for the slot, e.g. running_title, cue, world_heading\"},\"images\":{\"label\":\"file for hero_image, source_image, result_image, sketch_left, sketch_right\"},\"refs\":[\"1 to 12 image files for the contact strip\"]}",
            always,
            crate::cmd::storyboard::cmd_storyboard_set
        ),
        cmd!(
            "storyboard.add",
            "Add a Page (Copy of a Template Page)",
            ["Storyboard"],
            None,
            "{\"path\":\"optional\",\"archetype\":\"scene\",\"at\":0,\"fields\":{},\"images\":{},\"refs\":[]}",
            always,
            crate::cmd::storyboard::cmd_storyboard_add
        ),
        cmd!(
            "storyboard.section",
            "Add a Section (Parent Page) and Apply It",
            ["Storyboard"],
            None,
            "{\"path\":\"optional\",\"name\":\"B\",\"based_on\":\"A-Parent\",\"pages\":[0,1]}",
            always,
            crate::cmd::storyboard::cmd_storyboard_section
        ),
        cmd!(
            "storyboard.overlay",
            "Overlay New Art, Keeping the Original Underneath",
            ["Storyboard"],
            None,
            "{\"path\":\"optional\",\"page\":0,\"slot\":\"hero_image\",\"image\":\"/absolute/new.png\",\"layer\":\"Revision\",\"hide_original\":true}",
            always,
            crate::cmd::storyboard::cmd_storyboard_overlay
        ),
        query!(
            "storyboard.thumbnail",
            "Render One Storyboard Page to PNG",
            "{\"path\":\"optional\",\"page\":0,\"scene\":1,\"out\":\"/absolute/scene.png\",\"scale\":0.5}",
            crate::cmd::storyboard::cmd_storyboard_thumbnail
        ),
        query!(
            "storyboard.export",
            "Export Storyboard to PDF and/or PNG Spreads",
            "{\"path\":\"optional\",\"pdf\":\"/absolute/storyboard.pdf\",\"png_dir\":\"/absolute/dir\",\"scale\":1.0}",
            crate::cmd::storyboard::cmd_storyboard_export
        ),
    ]
}

fn specs_film() -> Vec<CommandSpec> {
    vec![
        query!(
            "video.film",
            "Film Timeline, Preview and Export Status",
            "{}",
            crate::film::cmd_video_film
        ),
        cmd!(
            "video.preview",
            "Render a Film Preview (FilmCraft)",
            ["Studio"],
            None,
            "{}",
            always,
            crate::film::cmd_video_preview
        ),
        cmd!(
            "video.export",
            "Export the Film (FilmCraft)",
            ["Studio"],
            None,
            "{}",
            always,
            crate::film::cmd_video_export
        ),
        query!(
            "video.frame",
            "Render One Film Frame (FilmCraft)",
            "{\"seconds\":0}",
            crate::film::cmd_video_frame
        ),
        query!(
            "video.open",
            "Open the Film in FilmCraft",
            "{}",
            crate::film::cmd_video_open
        ),
        query!(
            "video.timeline",
            "Film Tracks and Clips (FilmCraft)",
            "{}",
            crate::film::cmd_video_timeline
        ),
        cmd!(
            "video.place_clip",
            "Place a Clip on the Film (FilmCraft)",
            ["Studio"],
            None,
            "{\"clip\":\"rendered .mov\",\"from\":0,\"track\":\"V2\",\"width\":\"animation study's first clip starts the film: 1920\",\"height\":1080,\"fps\":30}",
            always,
            crate::film::cmd_video_place_clip
        ),
        cmd!(
            "video.exec",
            "Execute Any FilmCraft Command on the Linked .fcproj",
            ["Studio"],
            None,
            "{\"id\":\"file.exportInterchange|file.projectManager|graphics.newFromFile|sequence.inspect|project.items\",\"params\":{},\"save\":true}",
            always,
            crate::film::cmd_video_exec
        ),
    ]
}
