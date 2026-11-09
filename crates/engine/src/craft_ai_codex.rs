//! Codex as a Craft AI connection: Scribbles draws through `AiClient` (`status`, `plan`, consent,
//! `start`, `Job`), and this module is the adapter the client runs (docs/adopting/bring-a-service.md
//! in `storytold/craft-ai-integrations`).

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

const PROVIDER: &str = "codex";
const MODEL: &str = "gpt-6-luna";
const MAX_REFERENCES: u32 = 5;
const POLL: Duration = Duration::from_secs(1);
const LIMIT: Duration = Duration::from_secs(300);

fn engine(e: impl std::fmt::Display) -> EngineError {
    EngineError::CommandFailed("craft-ai".into(), e.to_string())
}

fn internal(e: impl std::fmt::Display) -> AiError {
    AiError::new(ErrorKind::Internal, e.to_string())
}

fn model_info() -> std::result::Result<ModelInfo, AiError> {
    let capability = CapabilityId::parse("image.generate").map_err(internal)?;
    let spec = CapabilitySpec::new(capability).accepting(InputRole::Prompt, 1).accepting(InputRole::Reference, MAX_REFERENCES).requiring(InputRole::Prompt);
    Ok(ModelInfo::new(ModelId::new(MODEL).map_err(internal)?, "Codex Luna").with(spec))
}

enum Phase {
    Running,
    Ready(Vec<u8>),
    Failed(AiError),
    Delivered,
}

struct CodexProvider {
    model: ModelInfo,
    jobs: Arc<Mutex<HashMap<String, Phase>>>,
    next: AtomicU32,
}

fn lock(jobs: &Mutex<HashMap<String, Phase>>) -> std::result::Result<MutexGuard<'_, HashMap<String, Phase>>, AiError> {
    jobs.lock().map_err(|_| AiError::new(ErrorKind::Internal, "the job list is unavailable"))
}

/// Writes the reference images beside the job and runs Codex on them.
fn run_codex(model: &str, prompt: &str, references: &[Vec<u8>]) -> Result<Vec<u8>> {
    let dir = ScopedTempDir::new("magic-codex-job")?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for (index, data) in references.iter().enumerate() {
        let format = image::guess_format(data).map_err(engine)?;
        let extension = format.extensions_str().first().copied().unwrap_or("png");
        let path = dir.path().join(format!("reference-{index}.{extension}"));
        std::fs::write(&path, data)?;
        paths.push(path);
    }
    let out = dir.path().join("result.png");
    crate::image_generation::generate_via_guarded_codex(model, prompt, &paths, dir.path(), &out)?;
    Ok(std::fs::read(&out)?)
}

impl Provider for CodexProvider {
    fn models(&self) -> std::result::Result<Vec<ModelInfo>, AiError> {
        Ok(vec![self.model.clone()])
    }

    fn submit(&self, model: &ModelId, request: &Request) -> std::result::Result<JobId, AiError> {
        if *model != self.model.id {
            return Err(AiError::unsupported("this service has no such model"));
        }
        let spec = self.model.spec_for(&request.capability).ok_or_else(|| AiError::unsupported(format!("this model cannot do {}", request.capability)))?;
        request.validate(spec, &Limits::default())?;
        let mut prompt = String::new();
        let mut references = Vec::new();
        for input in &request.inputs {
            match (&input.role, &input.payload) {
                (InputRole::Prompt, Payload::Text { text }) => prompt.clone_from(text),
                (InputRole::Reference, Payload::Inline { data, .. }) => references.push(data.clone()),
                _ => return Err(AiError::invalid("Codex takes a text prompt and inline reference images")),
            }
        }
        let name = format!("codex-{}", self.next.fetch_add(1, Ordering::SeqCst));
        let job = JobId::new(&name).map_err(internal)?;
        lock(&self.jobs)?.insert(name.clone(), Phase::Running);
        let jobs = Arc::clone(&self.jobs);
        let model = model.as_str().to_owned();
        std::thread::spawn(move || {
            let phase = match run_codex(&model, &prompt, &references) {
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
            Some(Phase::Running) => Err(AiError::unsupported("Codex cannot stop a drawing once it has started")),
            Some(_) => Ok(()),
        }
    }
}

/// The standard adapters plus Codex.
pub(crate) fn factory() -> ProviderFactory {
    let mut factory = ProviderFactory::standard();
    factory.register(PROVIDER, |_, _| {
        Ok(Arc::new(CodexProvider { model: model_info()?, jobs: Arc::new(Mutex::new(HashMap::new())), next: AtomicU32::new(1) }) as Arc<dyn Provider>)
    });
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

/// Draws through the person's chosen connection and writes the result to `out`.
pub(crate) fn draw(prompt: &str, references: &[PathBuf], out: &Path) -> Result<()> {
    let role = crate::fidget::scribbles_role()?;
    let client = client()?;
    if let RoleStatus::Unavailable { reason } = client.status(&role) {
        return Err(EngineError::InvalidParams(format!("Scribbles cannot draw: {reason}. Run `magic-cli exec scribbles.connect` to choose Codex.")));
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

/// `scribbles.connect`: adds Codex to the shared connections and chooses it for Scribbles.
pub fn cmd_scribbles_connect(_: &mut Session, _: &Value) -> Result<Value> {
    let model = model_info().map_err(engine)?;
    let connection: Connection = serde_json::from_value(json!({
        "id": PROVIDER, "label": "Codex", "provider": PROVIDER, "locality": "remote",
        "auth": {"kind": "none"}, "billing": "subscription_allowance", "models": [model.clone()],
    }))
    .map_err(engine)?;
    let shared = SharedStore::at_default_location().map_err(engine)?;
    let id = connection.id.clone();
    shared
        .update(|store| {
            store.connections.retain(|existing| existing.id != connection.id);
            store.connections.push(connection);
        })
        .map_err(engine)?;
    let role = crate::fidget::scribbles_role()?;
    ToolDesk::new(client()?, shared, None).choose(&role, &id, &model.id).map_err(engine)?;
    Ok(json!({"ok": true, "connection": PROVIDER, "model": MODEL, "craft_ai": crate::fidget::craft_ai_status()}))
}

/// `scribbles.consent`: records the person's agreement to send drawing requests to Codex.
pub fn cmd_scribbles_consent(_: &mut Session, p: &Value) -> Result<Value> {
    if p.get("agree").and_then(Value::as_bool) != Some(true) && p.get("agree").and_then(Value::as_str) != Some("true") {
        return Err(EngineError::InvalidParams("pass agree=true to agree to send drawing requests to Codex".into()));
    }
    let wanted = p.get("term").and_then(Value::as_str).unwrap_or_default();
    let term = ConsentTerm::ALL.into_iter().find(|term| term.id() == wanted).ok_or_else(|| {
        let names: Vec<&str> = ConsentTerm::ALL.iter().map(|term| term.id()).collect();
        EngineError::InvalidParams(format!("pass term= one of {}", names.join(", ")))
    })?;
    let role = crate::fidget::scribbles_role()?;
    let client = client()?;
    // A prompt and one image: the two kinds of data a drawing request can carry.
    let request = Request::new(role.capability.clone())
        .with_input(InputRole::Prompt, Payload::text("consent"))
        .with_input(InputRole::Reference, Payload::inline("image/png", vec![0x89, b'P', b'N', b'G']).map_err(engine)?);
    let plan = client.plan(&role, request).map_err(engine)?;
    let file = client.consent_file().ok_or_else(|| engine("this system has no folder for the consent file"))?;
    let mut ledger: ConsentLedger = file.load_or_empty();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(engine)?.as_secs();
    ledger.agree(plan.disclosure(), term, now);
    if term.remembers() {
        file.save(&ledger).map_err(engine)?;
    }
    Ok(json!({"ok": true, "destination": plan.disclosure().destination, "term": term.id(), "remembered": term.remembers()}))
}
