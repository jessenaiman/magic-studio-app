//! Craft AI [`Provider`] adapters for Magic Studio's three image backends (`docs/adopting/bring-a-service.md`
//! in `storytold/craft-ai-integrations`):
//! - `codex`: Codex CLI (`gpt-6-luna` / GPT-6 Luna)
//! - `agy`: Antigravity CLI (`gemini-3.8-flash-medium` / Gemini 3.8 Flash `image-generator`)
//! - `flow`: Google Flow Browser-Agent (`flow-clip --mode image` via `agent-browser`, `nano-banana-2.1` / `nano-banana-pro`)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use craft_ai_client::{AiClient, ProviderFactory, RoleStatus, ToolDesk};
use craft_ai_core::payload::Limits;
use craft_ai_core::{
    AiError, AppId, CapabilityId, CapabilitySpec, Connection, ConsentLedger, ConsentTerm, ErrorKind, Event, InputRole, JobId, JobSnapshot, JobState, ModelId,
    ModelInfo, Output, Payload, Provider, Request,
};
use craft_ai_native::shared::SharedStore;
use serde_json::{Value, json};

use crate::craft_hub::ScopedTempDir;
use crate::{EngineError, Result, Session};

pub const CODEX_PROVIDER: &str = "codex";
pub const CODEX_MODEL: &str = "gpt-6-luna";

pub const AGY_PROVIDER: &str = "agy";
pub const AGY_MODEL: &str = "gemini-3.8-flash-medium";

pub const FLOW_PROVIDER: &str = "flow";
pub const FLOW_MODEL: &str = "nano-banana-2.1";

const MAX_REFERENCES: u32 = 5;
const POLL: Duration = Duration::from_secs(1);
const LIMIT: Duration = Duration::from_secs(600);

fn engine(e: impl std::fmt::Display) -> EngineError {
    EngineError::CommandFailed("craft-ai".into(), e.to_string())
}

fn internal(e: impl std::fmt::Display) -> AiError {
    AiError::new(ErrorKind::Internal, e.to_string())
}

pub fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "agy" => AGY_MODEL,
        "flow" => FLOW_MODEL,
        _ => CODEX_MODEL,
    }
}

fn label_for_provider(provider: &str) -> &'static str {
    match provider {
        "agy" => "AGY (Gemini 3.8 Flash)",
        "flow" => "Google Flow (Browser-Agent)",
        _ => "Codex",
    }
}

fn image_spec() -> std::result::Result<CapabilitySpec, AiError> {
    let capability = CapabilityId::parse("image.generate").map_err(internal)?;
    Ok(CapabilitySpec::new(capability).accepting(InputRole::Prompt, 1).accepting(InputRole::Reference, MAX_REFERENCES).requiring(InputRole::Prompt))
}

fn models_for_provider(provider: &str) -> std::result::Result<Vec<ModelInfo>, AiError> {
    let spec = image_spec()?;
    let pairs: &[(&str, &str)] = match provider {
        "agy" => &[
            ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium)"),
            ("gemini-3.8-flash-high", "Gemini 3.8 Flash (High)"),
            ("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low)"),
        ],
        "flow" => &[("nano-banana-2.1", "Nano Banana 2.1"), ("nano-banana-pro", "Nano Banana Pro")],
        _ => &[(CODEX_MODEL, "Codex Luna")],
    };
    let mut out = Vec::with_capacity(pairs.len());
    for (id, label) in pairs {
        out.push(ModelInfo::new(ModelId::new(*id).map_err(internal)?, *label).with(spec.clone()));
    }
    Ok(out)
}

enum Phase {
    Running,
    Ready(Vec<u8>),
    Failed(AiError),
    Delivered,
}

struct CliImageProvider {
    provider_id: &'static str,
    models: Vec<ModelInfo>,
    jobs: Arc<Mutex<HashMap<String, Phase>>>,
    next: AtomicU32,
}

fn lock(jobs: &Mutex<HashMap<String, Phase>>) -> std::result::Result<MutexGuard<'_, HashMap<String, Phase>>, AiError> {
    jobs.lock().map_err(|_| AiError::new(ErrorKind::Internal, "the job list is unavailable"))
}

/// Writes the reference images beside the job and runs the chosen backend (`codex`, `agy`, or `flow`) on them.
fn run_backend(provider_id: &str, model: &str, prompt: &str, references: &[Vec<u8>]) -> Result<Vec<u8>> {
    let dir = ScopedTempDir::new(&format!("magic-{provider_id}-job"))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for (index, data) in references.iter().enumerate() {
        let format = image::guess_format(data).map_err(engine)?;
        let extension = format.extensions_str().first().copied().unwrap_or("png");
        let path = dir.path().join(format!("reference-{index}.{extension}"));
        std::fs::write(&path, data)?;
        paths.push(path);
    }
    let out = dir.path().join("result.png");
    match provider_id {
        "agy" => crate::image_generation::generate_via_guarded_agy(model, prompt, &paths, dir.path(), &out)?,
        "flow" => crate::image_generation::generate_via_flow(prompt, &paths, Some(model), &out)?,
        _ => crate::image_generation::generate_via_guarded_codex(model, prompt, &paths, dir.path(), &out)?,
    }
    Ok(std::fs::read(&out)?)
}

impl Provider for CliImageProvider {
    fn models(&self) -> std::result::Result<Vec<ModelInfo>, AiError> {
        Ok(self.models.clone())
    }

    fn submit(&self, model: &ModelId, request: &Request) -> std::result::Result<JobId, AiError> {
        let model_info = self.models.iter().find(|m| m.id == *model).ok_or_else(|| AiError::unsupported("this service has no such model"))?;
        let spec = model_info.spec_for(&request.capability).ok_or_else(|| AiError::unsupported(format!("this model cannot do {}", request.capability)))?;
        request.validate(spec, &Limits::default())?;
        let mut prompt = String::new();
        let mut references = Vec::new();
        for input in &request.inputs {
            match (&input.role, &input.payload) {
                (InputRole::Prompt, Payload::Text { text }) => prompt.clone_from(text),
                (InputRole::Reference, Payload::Inline { data, .. }) => references.push(data.clone()),
                _ => return Err(AiError::invalid("Scribbles takes a text prompt and inline reference images")),
            }
        }
        let provider_id = self.provider_id;
        let name = format!("{provider_id}-{}", self.next.fetch_add(1, Ordering::SeqCst));
        let job = JobId::new(&name).map_err(internal)?;
        lock(&self.jobs)?.insert(name.clone(), Phase::Running);
        let jobs = Arc::clone(&self.jobs);
        let model_str = model.as_str().to_owned();
        std::thread::spawn(move || {
            let phase = match run_backend(provider_id, &model_str, &prompt, &references) {
                Ok(png) => Phase::Ready(png),
                Err(e) => Phase::Failed(AiError::new(ErrorKind::Provider, e.to_string())),
            };
            if let Ok(mut held) = jobs.lock() {
                held.insert(name, phase);
            }
        });
        Ok(job)
    }

    fn poll(&self, job: &JobId) -> std::result::Result<JobSnapshot, AiError> {
        let mut jobs = lock(&self.jobs)?;
        let phase = jobs.get_mut(job.as_str()).ok_or_else(|| AiError::invalid("that job does not exist"))?;
        Ok(match std::mem::replace(phase, Phase::Delivered) {
            Phase::Running => {
                *phase = Phase::Running;
                JobSnapshot::new(JobState::Running { percent: None })
            }
            Phase::Failed(error) => {
                *phase = Phase::Failed(error.clone());
                JobSnapshot::new(JobState::Failed { error })
            }
            Phase::Ready(png) => {
                let mut done = JobSnapshot::new(JobState::Succeeded);
                done.events.push(Event::Output(Output { payload: Payload::inline("image/png", png)?, label: None }));
                done
            }
            Phase::Delivered => JobSnapshot::new(JobState::Succeeded),
        })
    }

    fn cancel(&self, job: &JobId) -> std::result::Result<(), AiError> {
        let jobs = lock(&self.jobs)?;
        match jobs.get(job.as_str()) {
            None => Err(AiError::invalid("that job does not exist")),
            Some(Phase::Running) => Err(AiError::unsupported("cannot stop a drawing once it has started")),
            Some(_) => Ok(()),
        }
    }
}

/// The standard Craft AI adapters plus `codex` (Luna), `agy` (Flash), and `flow` (Browser-Agent).
pub(crate) fn factory() -> ProviderFactory {
    let mut factory = ProviderFactory::standard();
    for provider_id in [CODEX_PROVIDER, AGY_PROVIDER, FLOW_PROVIDER] {
        factory.register(provider_id, move |_, _| {
            Ok(Arc::new(CliImageProvider {
                provider_id,
                models: models_for_provider(provider_id)?,
                jobs: Arc::new(Mutex::new(HashMap::new())),
                next: AtomicU32::new(1),
            }) as Arc<dyn Provider>)
        });
    }
    factory
}

fn client() -> Result<AiClient> {
    Ok(AiClient::standard(AppId::new("magic").map_err(engine)?).map_err(engine)?.with_factory(factory()))
}

fn image_request(role: &craft_ai_core::Role, prompt: &str, references: &[PathBuf]) -> Result<Request> {
    let mut request = Request::new(role.capability.clone()).with_input(InputRole::Prompt, Payload::text(prompt));
    for path in references {
        let data = std::fs::read(path)?;
        let media = image::guess_format(&data).map_err(engine)?.to_mime_type();
        request = request.with_input(InputRole::Reference, Payload::inline(media, data).map_err(engine)?);
    }
    Ok(request)
}

/// Ensure `codex`, `agy`, and `flow` connections exist in `SharedStore` and choose `provider` + `model`
/// for the `scribbles` role in `ToolDesk`.
pub fn ensure_connections_and_choice(provider: &str, model_override: Option<&str>) -> Result<(String, String)> {
    let canonical_provider = match provider.trim() {
        "agy" | "flash" | "gemini-flash" => AGY_PROVIDER,
        "flow" | "browser" | "browser-agent" => FLOW_PROVIDER,
        "codex" | "luna" | "" => CODEX_PROVIDER,
        other => {
            return Err(EngineError::InvalidParams(format!("unknown Scribbles provider `{other}`: expected `codex`, `agy`, or `flow`")));
        }
    };
    let chosen_model = model_override
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .filter(|m| match canonical_provider {
            "codex" => !m.starts_with("gemini-") && !m.starts_with("nano-banana"),
            "agy" => *m != CODEX_MODEL && !m.starts_with("nano-banana"),
            "flow" => *m != CODEX_MODEL && !m.starts_with("gemini-"),
            _ => true,
        })
        .unwrap_or_else(|| default_model_for_provider(canonical_provider));

    let shared = SharedStore::at_default_location().map_err(engine)?;
    let mut connections_to_save = Vec::new();
    for p_id in [CODEX_PROVIDER, AGY_PROVIDER, FLOW_PROVIDER] {
        let models = models_for_provider(p_id).map_err(engine)?;
        let conn: Connection = serde_json::from_value(json!({
            "id": p_id,
            "label": label_for_provider(p_id),
            "provider": p_id,
            "locality": "remote",
            "auth": { "kind": "none" },
            "billing": "subscription_allowance",
            "models": models,
        }))
        .map_err(engine)?;
        connections_to_save.push(conn);
    }
    shared
        .update(|store| {
            for conn in connections_to_save {
                store.connections.retain(|existing| existing.id != conn.id);
                store.connections.push(conn);
            }
        })
        .map_err(engine)?;

    let role = crate::fidget::scribbles_role()?;
    let active_conn_id = craft_ai_core::ConnectionId::new(canonical_provider).map_err(engine)?;
    let active_model_id = ModelId::new(chosen_model).map_err(engine)?;
    ToolDesk::new(client()?, shared, None).choose(&role, &active_conn_id, &active_model_id).map_err(engine)?;

    Ok((canonical_provider.to_owned(), chosen_model.to_owned()))
}

/// Draws through [`AiClient`] (optionally switching to `provider_override` / `model_override`) and writes the PNG to `out`.
pub(crate) fn draw_with(provider_override: Option<&str>, model_override: Option<&str>, prompt: &str, references: &[PathBuf], out: &Path) -> Result<()> {
    if let Some(prov) = provider_override.map(str::trim).filter(|s| !s.is_empty()) {
        let _ = ensure_connections_and_choice(prov, model_override)?;
    }
    let role = crate::fidget::scribbles_role()?;
    let client = client()?;
    if let RoleStatus::Unavailable { reason } = client.status(&role) {
        return Err(EngineError::InvalidParams(format!(
            "Scribbles cannot draw: {reason}. Run `magic-cli exec scribbles.connect provider=codex` (or `provider=agy` / `provider=flow`)."
        )));
    }
    let plan = client.plan(&role, image_request(&role, prompt, references)?).map_err(engine)?;
    let ledger = client.consent_file().map(|file| file.load_or_empty()).unwrap_or_default();
    if plan.needs_consent(&ledger) {
        return Err(EngineError::InvalidParams(format!(
            "sending prompts and reference images to {} needs your agreement: run `magic-cli exec scribbles.consent agree=true term=one_hour` (or one_day, until_revoked)",
            plan.disclosure().destination
        )));
    }
    let finished = client.start(&plan, &ledger).map_err(engine)?.wait(POLL, LIMIT).map_err(engine)?;
    if let JobState::Failed { error } = &finished.state {
        return Err(engine(error));
    }
    let png = finished.events.iter().find_map(|event| match event {
        Event::Output(Output { payload: Payload::Inline { data, .. }, .. }) => Some(data),
        _ => None,
    });
    let png = png.ok_or_else(|| engine("the job finished without an image"))?;
    std::fs::write(out, png)?;
    Ok(())
}

/// `scribbles.connect`: registers `codex` (Luna), `agy` (Flash), and `flow` (Browser-Agent) in the
/// shared Craft AI store, chooses the requested `provider` and `model` for Scribbles, and syncs preferences.
pub fn cmd_scribbles_connect(s: &mut Session, p: &Value) -> Result<Value> {
    let settings = crate::project_settings::stored_settings();
    let requested_provider = p["provider"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| settings["scribbles"]["provider"].as_str().filter(|v| !v.trim().is_empty()))
        .or_else(|| settings["image"]["provider"].as_str().filter(|v| !v.trim().is_empty()))
        .unwrap_or(CODEX_PROVIDER);
    let requested_model = p["model"].as_str().filter(|v| !v.trim().is_empty());

    let (provider, model) = ensure_connections_and_choice(requested_provider, requested_model)?;

    let _ = s.execute("prefs.set", json!({ "key": "scribbles.provider", "value": provider }));
    let _ = s.execute("prefs.set", json!({ "key": "scribbles.model", "value": model }));
    let _ = s.execute("prefs.set", json!({ "key": "image.provider", "value": provider }));

    Ok(json!({
        "ok": true,
        "connection": provider,
        "provider": provider,
        "model": model,
        "available_providers": [
            { "id": CODEX_PROVIDER, "label": label_for_provider(CODEX_PROVIDER), "default_model": CODEX_MODEL },
            { "id": AGY_PROVIDER, "label": label_for_provider(AGY_PROVIDER), "default_model": AGY_MODEL },
            { "id": FLOW_PROVIDER, "label": label_for_provider(FLOW_PROVIDER), "default_model": FLOW_MODEL },
        ],
        "craft_ai": crate::fidget::craft_ai_status(),
    }))
}

/// `scribbles.consent`: records the person's agreement to send drawing requests to Scribbles
/// (`codex`, `agy`, and `flow`).
pub fn cmd_scribbles_consent(_: &mut Session, p: &Value) -> Result<Value> {
    if p.get("agree").and_then(Value::as_bool) != Some(true) && p.get("agree").and_then(Value::as_str) != Some("true") {
        return Err(EngineError::InvalidParams("pass agree=true to agree to send drawing requests to Scribbles".into()));
    }
    let wanted = p.get("term").and_then(Value::as_str).unwrap_or("until_revoked");
    let term = ConsentTerm::ALL.into_iter().find(|term| term.id() == wanted).ok_or_else(|| {
        let names: Vec<&str> = ConsentTerm::ALL.iter().map(|term| term.id()).collect();
        EngineError::InvalidParams(format!("pass term= one of {}", names.join(", ")))
    })?;

    let settings = crate::project_settings::stored_settings();
    let current_provider = settings["scribbles"]["provider"].as_str().or_else(|| settings["image"]["provider"].as_str()).unwrap_or(CODEX_PROVIDER).to_owned();
    let current_model = settings["scribbles"]["model"].as_str().map(str::to_owned);

    let target_providers: Vec<&str> =
        if let Some(prov) = p["provider"].as_str().filter(|s| !s.trim().is_empty()) { vec![prov] } else { vec![CODEX_PROVIDER, AGY_PROVIDER, FLOW_PROVIDER] };

    let role = crate::fidget::scribbles_role()?;
    let client = client()?;
    let file = client.consent_file().ok_or_else(|| engine("this system has no folder for the consent file"))?;
    let mut ledger: ConsentLedger = file.load_or_empty();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(engine)?.as_secs();
    let mut destinations = Vec::new();

    for prov in target_providers {
        let _ = ensure_connections_and_choice(prov, None)?;
        let request = Request::new(role.capability.clone())
            .with_input(InputRole::Prompt, Payload::text("consent"))
            .with_input(InputRole::Reference, Payload::inline("image/png", vec![0x89, b'P', b'N', b'G']).map_err(engine)?);
        let plan = client.plan(&role, request).map_err(engine)?;
        destinations.push(plan.disclosure().destination.clone());
        ledger.agree(plan.disclosure(), term, now);
    }
    if term.remembers() {
        file.save(&ledger).map_err(engine)?;
    }
    // Restore active provider choice
    let _ = ensure_connections_and_choice(&current_provider, current_model.as_deref());

    Ok(json!({
        "ok": true,
        "destination": destinations.first().cloned().unwrap_or_default(),
        "destinations": destinations,
        "term": term.id(),
        "remembered": term.remembers(),
    }))
}
