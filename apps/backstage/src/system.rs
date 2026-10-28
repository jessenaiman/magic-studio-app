//! Linux measurements, sampled exclusively by the visible MAYHEM view.
use std::{collections::VecDeque, fs, time::Instant};

#[derive(Default)]
pub struct Metrics {
    pub cpu: Option<u64>,
    pub cores: Vec<u64>,
    previous_cores: Vec<(u64, u64)>,
    pub memory_used: u64,
    pub memory_total: u64,
    pub history: VecDeque<u64>,
    pub network: Option<(u64, u64)>,
    pub load: String,
    pub processes: Vec<(u32, String, u64)>,
    pub error: Option<String>,
    previous: Option<(u64, u64, u64, u64, Instant)>,
}
pub fn cpu_ticks(raw: &str) -> Option<(u64, u64)> {
    let v: Vec<u64> = raw
        .lines()
        .next()?
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if v.len() < 4 {
        return None;
    }
    Some((v.iter().sum(), v[3] + v.get(4).copied().unwrap_or(0)))
}
impl Metrics {
    pub fn pause(&mut self) {
        self.previous = None;
        self.previous_cores.clear();
        self.cores.clear();
        self.cpu = None;
        self.network = None;
    }
    pub fn sample(&mut self) {
        let raw = match fs::read_to_string("/proc/stat") {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let Some((total, idle)) = cpu_ticks(&raw) else {
            self.error = Some("CPU counters unavailable".into());
            return;
        };
        let cores: Vec<(u64, u64)> = raw
            .lines()
            .skip(1)
            .take_while(|l| l.starts_with("cpu"))
            .take(16)
            .filter_map(cpu_ticks)
            .collect();
        self.cores = cores
            .iter()
            .zip(&self.previous_cores)
            .map(|((total, idle), (old, old_idle))| {
                let delta = total.saturating_sub(*old);
                (100 * delta.saturating_sub(idle.saturating_sub(*old_idle)))
                    .checked_div(delta)
                    .unwrap_or(0)
            })
            .collect();
        self.previous_cores = cores;
        let mem = fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let field = |name: &str| {
            mem.lines()
                .find(|l| l.starts_with(name))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };
        self.memory_total = field("MemTotal:") * 1024;
        self.memory_used = self
            .memory_total
            .saturating_sub(field("MemAvailable:") * 1024);
        let mut rx = 0;
        let mut tx = 0;
        for line in fs::read_to_string("/proc/net/dev")
            .unwrap_or_default()
            .lines()
            .skip(2)
        {
            if let Some((name, counts)) = line.split_once(':') {
                if name.trim() == "lo" {
                    continue;
                }
                let n: Vec<u64> = counts
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if n.len() > 8 {
                    rx += n[0];
                    tx += n[8];
                }
            }
        }
        let now = Instant::now();
        if let Some((old, old_idle, old_rx, old_tx, at)) = self.previous {
            let ticks = total.saturating_sub(old);
            if let Some(cpu) =
                (100 * ticks.saturating_sub(idle.saturating_sub(old_idle))).checked_div(ticks)
            {
                self.cpu = Some(cpu);
                self.history.push_back(cpu);
                if self.history.len() > 120 {
                    self.history.pop_front();
                }
            }
            let secs = now.duration_since(at).as_secs_f64();
            if secs > 0.0 {
                self.network = Some((
                    (rx.saturating_sub(old_rx) as f64 / secs) as u64,
                    (tx.saturating_sub(old_tx) as f64 / secs) as u64,
                ));
            }
        }
        self.previous = Some((total, idle, rx, tx, now));
        self.load = fs::read_to_string("/proc/loadavg")
            .unwrap_or_default()
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join("  ");
        self.processes.clear();
        if let Ok(entries) = fs::read_dir("/proc") {
            for e in entries.flatten() {
                let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                    continue;
                };
                let Ok(s) = fs::read_to_string(e.path().join("status")) else {
                    continue;
                };
                let name = s
                    .lines()
                    .find_map(|l| l.strip_prefix("Name:\t"))
                    .unwrap_or("?")
                    .chars()
                    .filter(|c| !c.is_control())
                    .collect();
                let rss = s
                    .lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                self.processes.push((pid, name, rss));
            }
        }
        self.processes.sort_by_key(|p| std::cmp::Reverse(p.2));
        self.processes.truncate(12);
        self.error = None;
    }
}
