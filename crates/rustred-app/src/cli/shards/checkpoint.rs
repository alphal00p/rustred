use super::{CliError, bad, config::Config, io_error, plan::Shard};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) fn read_json<T: DeserializeOwned>(path: &Path, maximum: u64) -> Result<T, CliError> {
    let file = File::open(path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    if file.metadata().map_err(io_error)?.len() > maximum {
        return Err(bad(format!(
            "{} exceeds {maximum} byte limit",
            path.display()
        )));
    }
    serde_json::from_reader(file.take(maximum + 1))
        .map_err(|e| bad(format!("{}: {e}", path.display())))
}
pub(super) fn write_json(
    path: &Path,
    value: &impl Serialize,
    replace: bool,
) -> Result<(), CliError> {
    crate::application::atomic_file::write_file_atomically_with(path, replace, |file| {
        serde_json::to_writer_pretty(&mut *file, value).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())
    })
    .map_err(io_error)
}
pub(super) fn digest(path: &Path) -> Result<(u64, String), CliError> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut hash = blake3::Hasher::new();
    let mut size = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        size += n as u64;
    }
    Ok((size, hash.finalize().to_hex().to_string()))
}
pub(super) fn immutable(path: &Path, executable: bool) -> Result<(), CliError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o555 } else { 0o444 }),
        )
        .map_err(io_error)?;
    }
    #[cfg(not(unix))]
    {
        let _ = executable;
        let mut p = fs::metadata(path).map_err(io_error)?.permissions();
        p.set_readonly(true);
        fs::set_permissions(path, p).map_err(io_error)?;
    }
    Ok(())
}
pub(super) fn copy_frozen(
    source: &Path,
    target: &Path,
    executable: bool,
) -> Result<FileIdentity, CliError> {
    let before = digest(source)?;
    let mut incoming = File::open(source).map_err(io_error)?;
    let mut outgoing = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(io_error)?;
    std::io::copy(&mut incoming, &mut outgoing).map_err(io_error)?;
    outgoing.sync_all().map_err(io_error)?;
    if digest(target)? != before || digest(source)? != before {
        return Err(bad(format!(
            "input changed while freezing: {}",
            source.display()
        )));
    }
    immutable(target, executable)?;
    Ok(FileIdentity {
        path: target.to_owned(),
        bytes: before.0,
        blake3: before.1,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileIdentity {
    pub path: PathBuf,
    pub bytes: u64,
    pub blake3: String,
}
impl FileIdentity {
    pub fn record(path: &Path) -> Result<Self, CliError> {
        let (bytes, blake3) = digest(path)?;
        Ok(Self {
            path: path.to_owned(),
            bytes,
            blake3,
        })
    }
    pub fn verify(&self, base: &Path) -> Result<(), CliError> {
        if digest(&base.join(&self.path))? != (self.bytes, self.blake3.clone()) {
            return Err(bad(format!(
                "frozen input or receipt changed: {}",
                self.path.display()
            )));
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub schema: String,
    pub config: Config,
    pub shards: Vec<Shard>,
    pub files: Vec<FileIdentity>,
    pub source_files: Vec<FileIdentity>,
    pub root_count: usize,
    pub query_count: usize,
}
impl Snapshot {
    pub fn verify(&self, directory: &Path) -> Result<(), CliError> {
        if self.schema != "rustred.independent-root-snapshot.v1" {
            return Err(bad("unsupported snapshot schema"));
        }
        self.config.validate()?;
        for file in &self.files {
            file.verify(directory)?;
        }
        let selection: Value =
            read_json(&directory.join("artifact/selection.json"), 16 * 1024 * 1024)?;
        let queries: Value = read_json(&directory.join("artifact/queries.json"), 64 * 1024 * 1024)?;
        let plan = super::plan::partition(&selection, &queries, self.config.shards)?;
        if plan != self.shards
            || self.config.jobs > plan.len()
            || self.root_count != plan.iter().map(|s| s.owners.len()).sum::<usize>()
            || self.query_count != plan.iter().map(|s| s.query_indices.len()).sum::<usize>()
        {
            return Err(bad(
                "immutable shard plan does not partition the original owner queries",
            ));
        }
        for shard in &plan {
            let actual: Value = read_json(
                &directory.join(format!("jobs/{:04}/queries.json", shard.id)),
                64 * 1024 * 1024,
            )?;
            if actual != super::plan::shard_queries(&queries, shard) {
                return Err(bad(
                    "a shard's frozen queries differ from the exact original partition",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JobState {
    pub id: usize,
    pub state: String,
    pub attempt: usize,
    pub pid: Option<u32>,
    pub start_ticks: Option<u64>,
    pub receipt: Option<FileIdentity>,
    pub last_error: Option<String>,
    pub native_summary: Option<Value>,
    pub saved_checkpoint: Option<Value>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MasterState {
    pub schema: String,
    pub state: String,
    pub jobs: Vec<JobState>,
}
impl MasterState {
    pub fn new(shards: &[Shard]) -> Self {
        Self {
            schema: "rustred.independent-root-state.v1".into(),
            state: "prepared".into(),
            jobs: shards
                .iter()
                .map(|s| JobState {
                    id: s.id,
                    state: "queued".into(),
                    attempt: 0,
                    pid: None,
                    start_ticks: None,
                    receipt: None,
                    last_error: None,
                    native_summary: None,
                    saved_checkpoint: None,
                })
                .collect(),
        }
    }
    pub fn save(&self, directory: &Path) -> Result<(), CliError> {
        write_json(&directory.join("state.json"), self, true)
    }
}

pub(super) fn acquire_lock(directory: &Path) -> Result<File, CliError> {
    let path = directory.join("campaign.lock");
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(bad("campaign lock cannot be a symlink"));
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(io_error)?;
    lock.try_lock()
        .map_err(|e| bad(format!("campaign already has a supervisor: {e}")))?;
    Ok(lock)
}

/// All completion evidence is native, emitted by the frozen executable. The
/// parent only validates/combines it; it never discharges an obligation.
pub(super) fn validate_completion(value: &Value) -> Result<(), CliError> {
    if value["event"] != "finished"
        || value["operation"] != "owner_domain_walk"
        || value["status"] != "locally_resolved"
        || value["all_scheduled_domains_resolved"] != true
        || value["recursive_worklist_exhausted"] != true
        || value.get("error").is_some_and(|v| !v.is_null())
        || ["queued_nodes", "failed_nodes", "frontiers"]
            .iter()
            .any(|k| value[*k].as_u64() != Some(0))
    {
        return Err(bad(
            "native final event does not establish error-free recursive exhaustion",
        ));
    }
    if let Some(parallel) = value.get("parallel").filter(|v| v.is_object()) {
        for key in [
            "active_workers",
            "in_flight_domains",
            "finished_uncommitted_domains",
            "worker_buffered_events",
            "completed_escrow_entries",
        ] {
            if parallel.get(key).is_some_and(|v| v.as_u64() != Some(0)) {
                return Err(bad(format!(
                    "native completion has nonzero or malformed parallel {key}"
                )));
            }
        }
    }
    if let Some(delegation) = value.get("delegation").filter(|v| !v.is_null()) {
        if delegation["all_ledger_obligations_discharged"] != true {
            return Err(bad("native delegation ledger is not discharged"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn complete() -> Value {
        json!({"event":"finished","operation":"owner_domain_walk","status":"locally_resolved","all_scheduled_domains_resolved":true,"recursive_worklist_exhausted":true,"queued_nodes":0,"failed_nodes":0,"frontiers":0})
    }
    #[test]
    fn completion_fails_closed() {
        validate_completion(&complete()).unwrap();
        for key in ["queued_nodes", "failed_nodes", "frontiers"] {
            let mut v = complete();
            v[key] = json!(1);
            assert!(validate_completion(&v).is_err());
        }
        let mut v = complete();
        v["error"] = json!("bad");
        assert!(validate_completion(&v).is_err());
        let mut v = complete();
        v["parallel"] = json!({"finished_uncommitted_domains":1});
        assert!(validate_completion(&v).is_err());
        let mut v = complete();
        v["delegation"] = json!({"all_ledger_obligations_discharged":false});
        assert!(validate_completion(&v).is_err());
        assert!(validate_completion(&json!({"all_scheduled_domains_resolved":true})).is_err());
    }
}
