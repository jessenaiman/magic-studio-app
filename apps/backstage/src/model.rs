use serde_json::Value;

pub fn rows(catalog: &Value, query: &str) -> Vec<Value> {
    let query = query.to_lowercase();
    catalog["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            p["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .contains(&query)
        })
        .cloned()
        .collect()
}

pub fn text(v: &Value) -> String {
    match v {
        Value::Null => "Not reported".into(),
        Value::String(s) => s
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect(),
        _ => v.to_string(),
    }
}

pub struct App {
    pub tab: usize,
    pub selected: usize,
    pub scroll: u16,
    pub query: String,
    pub searching: bool,
    pub catalog: Value,
    pub report: Value,
    pub changes: Value,
    pub notification_status: String,
    pub last_token: Option<(String, String)>,
    pub project_report: Value,
    pub project_error: String,
    pub graphics_status: String,
    pub agents: Vec<Value>,
    pub agent_selected: usize,
    pub agents_observed_at: Option<u64>,
    pub help: bool,
    pub detail: bool,
    pub detail_scroll: u16,
    pub status: String,
    pub observed: Option<std::time::Instant>,
    pub agent_status: String,
    pub effect_until: std::time::Instant,
    pub reduce_motion: bool,
    pub busy: bool,
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub worker: Option<std::thread::JoinHandle<()>>,
    pub pending: Option<crate::ReadRequest>,
    pub refresh_needed: bool,
    pub project_refresh_needed: bool,
}
impl Default for App {
    fn default() -> Self {
        Self {
            tab: 0,
            selected: 0,
            scroll: 0,
            query: String::new(),
            searching: false,
            catalog: Value::Null,
            report: Value::Null,
            changes: Value::Null,
            notification_status: "Change notifications not discovered".into(),
            last_token: None,
            project_report: Value::Null,
            project_error: String::new(),
            graphics_status: "Terminal graphics not queried".into(),
            agents: vec![],
            agent_selected: 0,
            agents_observed_at: None,
            help: false,
            detail: false,
            detail_scroll: 0,
            status: "Connecting to Magic…".into(),
            observed: None,
            agent_status: "Agent observations unavailable".into(),
            effect_until: std::time::Instant::now(),
            reduce_motion: false,
            busy: false,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            worker: None,
            pending: None,
            refresh_needed: false,
            project_refresh_needed: false,
        }
    }
}
impl App {
    pub fn move_agent(&mut self, delta: isize) {
        let count = self.agents.len();
        if count > 0 {
            self.agent_selected =
                (self.agent_selected as isize + delta).rem_euclid(count as isize) as usize;
        }
    }
    pub fn projects(&self) -> Vec<Value> {
        rows(&self.catalog, &self.query)
    }
    pub fn project(&self) -> Option<Value> {
        self.projects().get(self.selected).cloned()
    }
    pub fn path(&self) -> Option<String> {
        self.project()
            .and_then(|p| p["path"].as_str().map(str::to_owned))
    }
    pub fn move_selection(&mut self, delta: isize) {
        let n = self.projects().len();
        if n > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(n as isize) as usize;
        }
        self.scroll = 0;
        self.project_report = Value::Null;
        self.project_error.clear();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
