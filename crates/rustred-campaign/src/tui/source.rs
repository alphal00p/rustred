//! Where the dashboard's data comes from: the progress files of a campaign directory ([`DirSource`]) or values held in
//! memory ([`MemorySource`]). Sources never panic and never write; a file that cannot be read is reported, and the
//! last good copy of it is kept. Sources hand out RAW text (as read); the ONE sanitiser runs in `model` (audit C1).
//! A source is `Send + Sync`: the live dashboard reads it on a background thread (audit M1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use serde_json::Value;

use crate::progress::PROGRESS_DIR;

/// Size cap of one progress file (plan 3.P.2: viewer cap 16 MiB per file).
pub const MAX_FILE_BYTES: u64 = 16 << 20;
/// The progress schema major this dashboard understands.
pub const SCHEMA_MAJOR: u64 = 1;

/// One progress file as read.
#[derive(Clone, Debug, PartialEq)]
pub enum FileRead {
    Absent,
    /// Present but not usable (I/O error, too large, not JSON, not an object).
    Invalid(String),
    /// A schema this dashboard does not understand (another major, or not a campaign progress schema).
    UnknownSchema(String),
    Ok(Value),
}

impl FileRead {
    pub fn value(&self) -> Option<&Value> {
        match self {
            FileRead::Ok(v) => Some(v),
            _ => None,
        }
    }
}

/// Everything the summary screen needs, read at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct RawSnapshot {
    /// Human-readable location (directory path, or "memory").
    pub origin: String,
    pub campaign: FileRead,
    /// Node files by node id (from `campaign.json` `files.nodes`, else by listing `progress/nodes/`).
    pub nodes: BTreeMap<String, FileRead>,
    pub events: FileRead,
    /// Modification time of campaign.json (seconds since the epoch), a heartbeat of last resort.
    pub campaign_mtime_s: Option<f64>,
    /// Problems kept from earlier reads ("events.json unreadable since ...": last good copy in use).
    pub notes: Vec<String>,
    /// Optional progress files by key (`supervisor`, `jobs`, `attention`, `terminals`; names in
    /// `model::optional_files`): read when present, `Absent` otherwise.
    pub extra: BTreeMap<String, FileRead>,
}

impl RawSnapshot {
    pub fn empty(origin: &str) -> Self {
        RawSnapshot {
            origin: origin.into(),
            campaign: FileRead::Absent,
            nodes: BTreeMap::new(),
            events: FileRead::Absent,
            campaign_mtime_s: None,
            notes: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// One work item of a node (drill-down).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ItemRow {
    pub item: String,
    pub label: String,
    pub kind: String,
    pub rung: String,
    pub state: String,
    pub why: String,
    pub cpu_seconds: f64,
    pub jobs: usize,
    pub updated_seq: u64,
}

/// One running job.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct JobRow {
    pub job: String,
    pub node: String,
    pub item: String,
    pub rung: String,
    pub cpus: Option<u64>,
    pub rss_bytes: Option<u64>,
    pub cap_bytes: Option<u64>,
    pub elapsed_s: Option<f64>,
    pub budget_s: Option<f64>,
    pub heartbeat_age_s: Option<f64>,
    pub phase: Option<String>,
    pub pid: Option<u64>,
    pub run_dir: Option<String>,
}

/// The dashboard's data source (read on the dashboard's background reader thread).
pub trait ProgressSource: Send + Sync {
    /// Reads the progress files. Never panics.
    fn read(&self) -> RawSnapshot;
    /// Items of one node for the drill-down (`None`: not available from this source).
    fn node_items(&self, _node: &str) -> Result<Vec<ItemRow>, String> {
        Err("item lists are not available from this source".into())
    }
    /// `progress::item_detail` of one item.
    fn item_detail(&self, _item: &str) -> Result<Value, String> {
        Err("item detail is not available from this source".into())
    }
    /// Running attempts when `progress/jobs.json` is ABSENT (`None`: unknown). Shown labelled "from ledger
    /// (partial)"; never asked for when the progress files carry a job list.
    fn running_jobs(&self) -> Option<Vec<JobRow>> {
        None
    }
}

/// Parses and checks one progress document.
pub fn classify(bytes: &[u8]) -> FileRead {
    let v: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => return FileRead::Invalid(format!("not JSON ({e})")),
    };
    if !v.is_object() {
        return FileRead::Invalid("not a JSON object".into());
    }
    match v.get("schema").and_then(Value::as_str) {
        None => FileRead::Invalid("no schema field".into()),
        Some(s) => match schema_major(s) {
            Some(SCHEMA_MAJOR) => FileRead::Ok(v),
            Some(m) => FileRead::UnknownSchema(format!(
                "schema v{m} not understood (this dashboard reads v{SCHEMA_MAJOR}): {}",
                crate::tui::format::clean(s)
            )),
            None => FileRead::UnknownSchema(format!(
                "schema {} not understood",
                crate::tui::format::fit(&crate::tui::format::clean(s), 60)
            )),
        },
    }
}

/// `rustred.campaign.<name>.v<major>` -> major.
pub fn schema_major(s: &str) -> Option<u64> {
    let rest = s.strip_prefix("rustred.campaign.")?;
    let (_, v) = rest.rsplit_once(".v")?;
    v.parse().ok()
}

/// Memory-backed source (tests; or a supervisor handing over its derived progress set).
#[derive(Clone, Debug, Default)]
pub struct MemorySource {
    /// Relative paths as in the progress directory: `campaign.json`, `events.json`, `nodes/<T>.json`.
    pub files: BTreeMap<String, Value>,
    pub items: BTreeMap<String, Vec<ItemRow>>,
    pub details: BTreeMap<String, Value>,
    pub jobs: Option<Vec<JobRow>>,
    pub mtime_s: Option<f64>,
}

impl MemorySource {
    pub fn new(files: BTreeMap<String, Value>) -> Self {
        MemorySource {
            files,
            ..Default::default()
        }
    }
}

fn doc(v: Option<&Value>) -> FileRead {
    match v {
        None => FileRead::Absent,
        Some(v) => classify(&serde_json::to_vec(v).unwrap_or_default()),
    }
}

impl ProgressSource for MemorySource {
    fn read(&self) -> RawSnapshot {
        let mut snap = RawSnapshot::empty("memory");
        snap.campaign = doc(self.files.get("campaign.json"));
        snap.events = doc(self.files.get("events.json"));
        snap.campaign_mtime_s = self.mtime_s;
        for (key, rel) in crate::tui::model::optional_files(snap.campaign.value()) {
            if !snap.extra.get(&key).is_some_and(|r| *r != FileRead::Absent) {
                snap.extra.insert(key, doc(self.files.get(&rel)));
            }
        }
        for (k, v) in &self.files {
            if let Some(id) = k
                .strip_prefix("nodes/")
                .and_then(|r| r.strip_suffix(".json"))
            {
                snap.nodes.insert(id.to_string(), doc(Some(v)));
            }
        }
        snap
    }
    fn node_items(&self, node: &str) -> Result<Vec<ItemRow>, String> {
        self.items
            .get(node)
            .cloned()
            .ok_or_else(|| format!("no item list for {node} in this source"))
    }
    fn item_detail(&self, item: &str) -> Result<Value, String> {
        self.details
            .get(item)
            .cloned()
            .ok_or_else(|| format!("no detail for {item} in this source"))
    }
    fn running_jobs(&self) -> Option<Vec<JobRow>> {
        self.jobs.clone()
    }
}

/// Reduced ledger kept for the drill-down.
struct LedgerCache {
    seq: u64,
    at: Instant,
    state: Option<crate::state::CampaignState>,
    error: Option<String>,
}

/// Reads `CDIR/progress/` (read-only; works on a directory without write permission).
pub struct DirSource {
    cdir: PathBuf,
    /// Shown instead of the path (tests: a fixed label, so that screens do not depend on the checkout path).
    display: Option<String>,
    /// Drill-down by reducing the ledger only while the ledger segments are at most this large.
    pub ledger_cap_bytes: u64,
    /// The running-job FALLBACK (no job list in the progress files) reduces the ledger only while it is at most this
    /// large, ...
    pub job_fallback_cap_bytes: u64,
    /// ... and is switched off for the session after one reduction took longer than this.
    pub job_fallback_max: Duration,
    /// Minimum interval between two ledger reductions.
    pub ledger_min_interval: Duration,
    /// Node files read at most (a note says when more exist).
    pub max_node_files: usize,
    last_good: Mutex<BTreeMap<String, (Value, String)>>,
    ledger: Mutex<Option<LedgerCache>>,
    job_fallback_off: Mutex<Option<String>>,
}

impl DirSource {
    pub fn new(cdir: impl Into<PathBuf>) -> Self {
        DirSource {
            cdir: cdir.into(),
            display: None,
            ledger_cap_bytes: 256 << 20,
            job_fallback_cap_bytes: 64 << 20,
            job_fallback_max: Duration::from_secs(5),
            ledger_min_interval: Duration::from_secs(10),
            max_node_files: 20_000,
            last_good: Mutex::new(BTreeMap::new()),
            ledger: Mutex::new(None),
            job_fallback_off: Mutex::new(None),
        }
    }

    /// Shows `label` instead of the directory path (golden screens, or a caller that prefers a short name).
    pub fn with_display(mut self, label: impl Into<String>) -> Self {
        self.display = Some(label.into());
        self
    }

    pub fn cdir(&self) -> &Path {
        &self.cdir
    }

    /// Why the running-job fallback is off (None: available).
    pub fn job_fallback_off(&self) -> Option<String> {
        self.job_fallback_off
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Opens without blocking (a FIFO swapped in cannot hang the reader: audit m10), checks the OPENED file is a
    /// regular file (fstat after open), reads at most [`MAX_FILE_BYTES`].
    fn read_raw(path: &Path) -> FileRead {
        use std::io::Read;
        use std::os::unix::fs::OpenOptionsExt;
        let f = match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return FileRead::Absent,
            Err(e) => return FileRead::Invalid(format!("cannot open: {e}")),
        };
        match f.metadata() {
            Ok(m) if !m.is_file() => return FileRead::Invalid("not a regular file".into()),
            Ok(m) if m.len() > MAX_FILE_BYTES => {
                return FileRead::Invalid(format!(
                    "too large ({} bytes > {MAX_FILE_BYTES})",
                    m.len()
                ));
            }
            Ok(_) => {}
            Err(e) => return FileRead::Invalid(format!("cannot stat: {e}")),
        }
        let mut b = Vec::new();
        match f.take(MAX_FILE_BYTES + 1).read_to_end(&mut b) {
            Ok(_) if b.len() as u64 > MAX_FILE_BYTES => {
                FileRead::Invalid(format!("too large (> {MAX_FILE_BYTES} bytes)"))
            }
            Ok(_) => classify(&b),
            Err(e) => FileRead::Invalid(format!("cannot read: {e}")),
        }
    }

    /// Reads one file, retrying once after 50 ms on an unusable read, falling back to the last good copy.
    fn read_file(&self, rel: &str, notes: &mut Vec<String>) -> FileRead {
        let path = self.cdir.join(PROGRESS_DIR).join(rel);
        let mut r = Self::read_raw(&path);
        if matches!(r, FileRead::Invalid(_)) {
            std::thread::sleep(Duration::from_millis(50));
            r = Self::read_raw(&path);
        }
        let mut good = self.last_good.lock().unwrap_or_else(|p| p.into_inner());
        match &r {
            FileRead::Ok(v) => {
                good.insert(rel.to_string(), (v.clone(), now_hms()));
                r
            }
            FileRead::Invalid(why) => {
                if let Some((v, since)) = good.get(rel) {
                    notes.push(format!(
                        "{rel} unreadable ({why}); showing the copy read at {since}"
                    ));
                    FileRead::Ok(v.clone())
                } else {
                    r
                }
            }
            _ => r,
        }
    }

    fn ledger_bytes(&self) -> Option<u64> {
        let dir = crate::ledger::ledger_dir(&self.cdir);
        let rd = std::fs::read_dir(&dir).ok()?;
        let mut total = 0u64;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("segment-")
                && let Ok(m) = e.metadata()
            {
                total += m.len();
            }
        }
        Some(total)
    }

    /// Runs `f` on a read-only reduction of the ledger (cached per ledger seq, rate-limited, size-capped).
    fn with_state<T>(
        &self,
        f: impl FnOnce(&crate::state::CampaignState) -> T,
    ) -> Result<T, String> {
        let mut guard = self.ledger.lock().unwrap_or_else(|p| p.into_inner());
        let want_seq = match Self::read_raw(&self.cdir.join(PROGRESS_DIR).join("campaign.json")) {
            FileRead::Ok(v) => v
                .pointer("/ledger/seq")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            _ => 0,
        };
        let fresh = match guard.as_ref() {
            Some(c) => c.seq == want_seq || c.at.elapsed() < self.ledger_min_interval,
            None => false,
        };
        if !fresh {
            let cache = match self.ledger_bytes() {
                None => LedgerCache {
                    seq: want_seq,
                    at: Instant::now(),
                    state: None,
                    error: Some("no ledger in this directory".into()),
                },
                Some(b) if b > self.ledger_cap_bytes => LedgerCache {
                    seq: want_seq,
                    at: Instant::now(),
                    state: None,
                    error: Some(format!(
                        "ledger too large for live drill-down ({}): use rr_campaign item / why",
                        crate::tui::format::bytes(Some(b))
                    )),
                },
                // The READ-ONLY open (no lock, never repairs; snapshot + active segment, every line checked).
                Some(_) => match crate::ledger::open_read_only(&self.cdir) {
                    Ok(view) => LedgerCache {
                        seq: want_seq,
                        at: Instant::now(),
                        state: Some(view.state),
                        error: None,
                    },
                    Err(e) => LedgerCache {
                        seq: want_seq,
                        at: Instant::now(),
                        state: None,
                        error: Some(format!("ledger unreadable: {e}")),
                    },
                },
            };
            *guard = Some(cache);
        }
        let c = guard.as_ref().expect("cache set above");
        match &c.state {
            Some(st) => Ok(f(st)),
            None => Err(c.error.clone().unwrap_or_else(|| "no ledger".into())),
        }
    }
}

fn now_hms() -> String {
    let ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    crate::tui::format::hms(&crate::clock::UtcMillis(ms).to_iso())
}

fn mtime_s(path: &Path) -> Option<f64> {
    let m = std::fs::metadata(path).ok()?.modified().ok()?;
    m.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs_f64())
}

/// Item rows of one node from a reduced state (shared by [`DirSource`] and supervisors holding a state).
pub fn items_of(state: &crate::state::CampaignState, node: &str) -> Vec<ItemRow> {
    state
        .items
        .iter()
        .filter(|(_, it)| it.node == node)
        .map(|(id, it)| ItemRow {
            item: id.clone(),
            label: it.label.clone(),
            kind: it.kind.clone(),
            rung: it.rung.to_string(),
            state: it.phase.tag().to_string(),
            why: crate::progress::item_detail(state, id)
                .and_then(|d| d.get("why").and_then(Value::as_str).map(String::from))
                .unwrap_or_default(),
            cpu_seconds: it.cpu_ms as f64 / 1000.0,
            jobs: it.jobs.len(),
            updated_seq: it.updated_seq,
        })
        .collect()
}

/// Running attempts of a reduced state.
pub fn running_of(state: &crate::state::CampaignState) -> Vec<JobRow> {
    use crate::state::AttemptState;
    state
        .jobs
        .iter()
        .filter(|(_, j)| matches!(j.state, AttemptState::Running | AttemptState::Launching))
        .map(|(id, j)| JobRow {
            job: id.clone(),
            node: state
                .items
                .get(&j.item)
                .map(|i| i.node.clone())
                .unwrap_or_default(),
            item: state
                .items
                .get(&j.item)
                .map(|i| i.label.clone())
                .unwrap_or_else(|| j.item.clone()),
            rung: j.rung.to_string(),
            cap_bytes: (j.reserved_bytes > 0).then_some(j.reserved_bytes),
            pid: j.pid.map(u64::from),
            run_dir: j.run_dir.clone(),
            phase: Some(j.state.tag().to_string()),
            ..Default::default()
        })
        .collect()
}

impl ProgressSource for DirSource {
    fn read(&self) -> RawSnapshot {
        let origin = self
            .display
            .clone()
            .unwrap_or_else(|| self.cdir.display().to_string());
        let mut snap = RawSnapshot::empty(&origin);
        let mut notes = Vec::new();
        snap.campaign = self.read_file("campaign.json", &mut notes);
        snap.campaign_mtime_s = mtime_s(&self.cdir.join(PROGRESS_DIR).join("campaign.json"));
        snap.events = self.read_file("events.json", &mut notes);
        for (key, rel) in crate::tui::model::optional_files(snap.campaign.value()) {
            if rel.contains("..")
                || rel.starts_with('/')
                || snap.extra.get(&key).is_some_and(|r| *r != FileRead::Absent)
            {
                continue;
            }
            let r = self.read_file(&rel, &mut notes);
            snap.extra.insert(key, r);
        }
        // node files: named by campaign.json, else listed
        let mut names: BTreeMap<String, String> = BTreeMap::new();
        if let Some(m) = snap
            .campaign
            .value()
            .and_then(|v| v.pointer("/files/nodes"))
            .and_then(Value::as_object)
        {
            if m.len() > self.max_node_files {
                notes.push(format!(
                    "{} node files listed; only the first {} are read",
                    m.len(),
                    self.max_node_files
                ));
            }
            for (id, rel) in m.iter().take(self.max_node_files) {
                if let Some(rel) = rel.as_str()
                    && !rel.contains("..")
                    && !rel.starts_with('/')
                {
                    names.insert(id.clone(), rel.to_string());
                }
            }
        }
        if names.is_empty()
            && let Ok(rd) = std::fs::read_dir(self.cdir.join(PROGRESS_DIR).join("nodes"))
        {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if !n.starts_with('.')
                    && let Some(id) = n.strip_suffix(".json")
                    && !id.ends_with(".items")
                {
                    if names.len() >= self.max_node_files {
                        notes.push(format!(
                            "more than {} node files; the rest are not read",
                            self.max_node_files
                        ));
                        break;
                    }
                    names.insert(id.to_string(), format!("nodes/{n}"));
                }
            }
        }
        for (id, rel) in names {
            let r = self.read_file(&rel, &mut notes);
            snap.nodes.insert(id, r);
        }
        snap.notes = notes;
        snap
    }

    fn node_items(&self, node: &str) -> Result<Vec<ItemRow>, String> {
        self.with_state(|st| items_of(st, node))
    }

    fn item_detail(&self, item: &str) -> Result<Value, String> {
        self.with_state(|st| crate::progress::item_detail(st, item))?
            .ok_or_else(|| format!("no item {item} in the ledger"))
    }

    /// The ledger FALLBACK for running jobs (no job list in the progress files): bounded in size
    /// (`job_fallback_cap_bytes`) and time (switched off after one reduction slower than `job_fallback_max`).
    fn running_jobs(&self) -> Option<Vec<JobRow>> {
        if self.job_fallback_off().is_some() {
            return None;
        }
        match self.ledger_bytes() {
            None => return None,
            Some(b) if b > self.job_fallback_cap_bytes => {
                *self
                    .job_fallback_off
                    .lock()
                    .unwrap_or_else(|p| p.into_inner()) = Some(format!(
                    "ledger {} above the {} bound of the job-list fallback",
                    crate::tui::format::bytes(Some(b)),
                    crate::tui::format::bytes(Some(self.job_fallback_cap_bytes))
                ));
                return None;
            }
            Some(_) => {}
        }
        let t0 = Instant::now();
        let r = self.with_state(running_of).ok();
        if t0.elapsed() > self.job_fallback_max {
            *self
                .job_fallback_off
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = Some(format!(
                "one ledger reduction took {:.1} s (bound {:.0} s)",
                t0.elapsed().as_secs_f64(),
                self.job_fallback_max.as_secs_f64()
            ));
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_classification() {
        assert_eq!(schema_major("rustred.campaign.progress.v1"), Some(1));
        assert_eq!(schema_major("rustred.campaign.summary.v2"), Some(2));
        assert_eq!(schema_major("other.v1"), None);
        assert!(matches!(
            classify(b"{\"schema\":\"rustred.campaign.progress.v1\"}"),
            FileRead::Ok(_)
        ));
        assert!(matches!(
            classify(b"{\"schema\":\"rustred.campaign.progress.v2\"}"),
            FileRead::UnknownSchema(_)
        ));
        assert!(matches!(
            classify(b"{\"schema\":\"rustred.campaign.prog"),
            FileRead::Invalid(_)
        ));
        assert!(matches!(classify(b"[1,2]"), FileRead::Invalid(_)));
        assert!(matches!(classify(b"{}"), FileRead::Invalid(_)));
    }
}
