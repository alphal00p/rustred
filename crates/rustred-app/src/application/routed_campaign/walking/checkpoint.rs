//! Durable coordinator state, not a completed reduction artifact.
//! The binary envelope contains bounded UTF-8 serde chunks. Serialization is
//! streaming: no full queue/report clone or full serialized byte buffer exists.
pub(super) mod codec;
use super::{OwnerDomainWalkRequest, execution::State};
use crate::application::atomic_file::{write_file_atomically, write_file_atomically_with};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerDomainWalkCheckpointOptions {
    pub directory: PathBuf,
    pub resume: bool,
    pub interval_seconds: u64,
}
impl OwnerDomainWalkCheckpointOptions {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            resume: false,
            interval_seconds: 3600,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    kind: String,
    generation: u64,
    state_file: String,
    bytes: u64,
    digest: String,
    request: String,
    executable: String,
    owners: Vec<String>,
    metadata: Value,
}
pub(super) struct Store {
    schema: u32,
    options: OwnerDomainWalkCheckpointOptions,
    _lock: File,
    manifest: Option<Manifest>,
    request: String,
    executable: String,
    owners: Vec<String>,
    last: Instant,
}
fn file_digest(path: &Path) -> Result<(u64, String), String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = f.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes = bytes
            .checked_add(n as u64)
            .ok_or("checkpoint byte overflow")?;
    }
    Ok((bytes, hash.finalize().to_hex().to_string()))
}
fn binding(request: &OwnerDomainWalkRequest) -> String {
    // Checkpoint location, interval and resume mode are transport, not policy.
    let value = json!({"selection":request.matching.selection_json,"queries":request.matching.queries_json,
        "limits":super::limits_json(request),"reduction":format!("{:?}",request.matching.reduction_limits),
        "workers":request.workers,"inspection_workers":request.inspection_workers,
        "publication":format!("{:?}",request.publication_policy),"scheduling":format!("{:?}",request.scheduling_policy),
        "reuse_initial_d_bands":request.reuse_initial_d_bands,"max_domains":request.max_domains,
        "max_events":request.max_events,"max_frontiers":request.max_frontiers,
        "max_containment_checks":request.max_containment_checks,"route_domain_overcover":request.route_domain_overcover,
        "max_route_masks":request.max_route_masks,"subdivision":request.apply_subdivision,
        "max_queries":request.matching.max_queries,"max_query_bytes":request.matching.max_query_bytes});
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}
impl Store {
    pub(super) fn open(request: &OwnerDomainWalkRequest) -> Result<Option<Self>, String> {
        let Some(options) = request.checkpoint.clone() else {
            return Ok(None);
        };
        if options.resume {
            if !options.directory.is_dir() {
                return Err("resume checkpoint directory does not exist".into());
            }
        } else if options.directory.exists() {
            if fs::read_dir(&options.directory)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err("new checkpoint directory must be empty".into());
            }
        } else {
            fs::create_dir(&options.directory)
                .map_err(|e| format!("cannot create checkpoint directory: {e}"))?;
        }
        if fs::symlink_metadata(&options.directory)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("checkpoint directory may not be a symlink".into());
        }
        if !options.resume {
            // The generation writer syncs this directory's contents. Its own
            // entry in the parent must also survive a crash after first save.
            let parent = options
                .directory
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("cannot sync checkpoint directory parent: {error}"))?;
        }
        let lock_path = options.directory.join("checkpoint.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("checkpoint lock may not be a symlink".into());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("checkpoint is in use: {e}"))?;
        let request_binding = binding(request);
        let schema = if request.publication_policy == super::OwnerDomainWalkPublicationPolicy::Ready
        {
            2
        } else {
            1
        };
        let executable = file_digest(&std::env::current_exe().map_err(|e| e.to_string())?)?.1;
        let manifest = if options.resume {
            let path = options.directory.join("latest.json");
            let m: Manifest = serde_json::from_reader(
                File::open(path)
                    .map_err(|e| format!("cannot open checkpoint manifest: {e}"))?
                    .take(1024 * 1024),
            )
            .map_err(|e| format!("invalid checkpoint manifest: {e}"))?;
            if m.schema != schema
                || !matches!(m.kind.as_str(), "bootstrap" | "state")
                || (m.kind == "state" && m.owners.is_empty())
                || m.state_file != format!("state-{:020}.bin", m.generation)
            {
                return Err("unsupported checkpoint generation".into());
            }
            if m.request != request_binding {
                return Err("checkpoint request or policy differs; refusing to restart".into());
            }
            if m.executable != executable {
                return Err("checkpoint executable bytes differ; use the original binary".into());
            }
            let state_path = options.directory.join(&m.state_file);
            if file_digest(&state_path)? != (m.bytes, m.digest.clone()) {
                return Err("checkpoint state checksum or length mismatch".into());
            }
            Some(m)
        } else {
            None
        };
        Ok(Some(Self {
            schema,
            options,
            _lock: lock,
            manifest,
            request: request_binding,
            executable,
            owners: Vec::new(),
            last: Instant::now(),
        }))
    }
    pub(super) fn bind_owners(&mut self, owners: Vec<String>) -> Result<(), String> {
        if self
            .manifest
            .as_ref()
            .is_some_and(|m| m.kind == "state" && m.owners != owners)
        {
            return Err("checkpoint immutable owner payload bytes differ".into());
        }
        self.owners = owners;
        Ok(())
    }
    pub(super) fn resume<const N: usize>(&self) -> Result<Option<codec::Restored<N>>, String> {
        let Some(m) = &self.manifest else {
            return Ok(None);
        };
        if !self.options.resume || m.kind == "bootstrap" {
            return Ok(None);
        }
        let f =
            File::open(self.options.directory.join(&m.state_file)).map_err(|e| e.to_string())?;
        codec::read(BufReader::new(f)).map(Some)
    }
    pub(super) fn metadata(&self) -> Option<&Value> {
        self.manifest.as_ref().map(|m| &m.metadata)
    }
    pub(super) fn bootstrap(&mut self) -> Result<Option<Value>, String> {
        if self.manifest.is_some() {
            return Ok(None);
        }
        let generation = 1;
        let state_file = format!("state-{generation:020}.bin");
        let path = self.options.directory.join(&state_file);
        write_file_atomically(&path, b"RUSTRED-WALK-BOOTSTRAP-1\n", false)?;
        let (bytes, digest) = file_digest(&path)?;
        let metadata = json!({"state":"saved","directory":self.options.directory,"generation":generation,"state_path":path,
            "saved_unix_time":SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),
            "committed_domains":0,"pending_domains":0,"completed_native_inspections":0,"committed_events":0,
            "paused":false,"bootstrap":true,"preparation_must_restart":true,"bytes":bytes});
        let manifest = Manifest {
            schema: self.schema,
            kind: "bootstrap".into(),
            generation,
            state_file,
            bytes,
            digest,
            request: self.request.clone(),
            executable: self.executable.clone(),
            owners: Vec::new(),
            metadata: metadata.clone(),
        };
        self.publish(manifest)?;
        Ok(Some(
            json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false}),
        ))
    }
    fn publish(&mut self, manifest: Manifest) -> Result<(), String> {
        // Retain the complete preceding authority, not merely its state bytes.
        // Recovery is explicit: a corrupt latest generation is never silently
        // replaced by an older prefix during normal --resume admission.
        if let Some(previous) = &self.manifest {
            write_file_atomically(
                &self.options.directory.join("previous.json"),
                &serde_json::to_vec(previous).map_err(|e| e.to_string())?,
                true,
            )?;
        }
        write_file_atomically(
            &self.options.directory.join("latest.json"),
            &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
            true,
        )?;
        let previous = self.manifest.as_ref().map(|m| m.generation);
        self.manifest = Some(manifest);
        self.last = Instant::now();
        // Only explicit validated filenames older than the previous good
        // generation are ours to remove. A failed publication keeps both old
        // authority and the newly written orphan; never delete on save failure.
        if let Some(previous) = previous {
            for entry in fs::read_dir(&self.options.directory).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let Some(number) = name
                    .strip_prefix("state-")
                    .and_then(|s| s.strip_suffix(".bin"))
                else {
                    continue;
                };
                if number.len() != 20 || !number.bytes().all(|b| b.is_ascii_digit()) {
                    continue;
                }
                let Ok(generation) = number.parse::<u64>() else {
                    continue;
                };
                if generation < previous && entry.file_type().map_err(|e| e.to_string())?.is_file()
                {
                    fs::remove_file(entry.path()).map_err(|e| {
                        format!("checkpoint saved, old generation cleanup failed: {e}")
                    })?;
                }
            }
            File::open(&self.options.directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub(super) fn save<const N: usize>(
        &mut self,
        state: &State<N>,
        inputs: &[Value],
        frontiers: &[Value],
        force: bool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Value>, String> {
        if !force && self.last.elapsed().as_secs() < self.options.interval_seconds {
            return Ok(None);
        }
        if state.error.is_some() {
            return Err("refusing to checkpoint a failed publication prefix".into());
        }
        if self.owners.is_empty() {
            return Err("checkpoint owner binding is unavailable".into());
        }
        let started = Instant::now();
        let started_unix_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let mut generation = self
            .manifest
            .as_ref()
            .map_or(Some(1), |m| m.generation.checked_add(1))
            .ok_or("checkpoint generation overflow")?;
        while self
            .options
            .directory
            .join(format!("state-{generation:020}.bin"))
            .exists()
        {
            generation = generation
                .checked_add(1)
                .ok_or("checkpoint generation overflow")?;
        }
        let state_file = format!("state-{generation:020}.bin");
        let path = self.options.directory.join(&state_file);
        observer(
            json!({"event":"checkpoint_started","operation":"owner_domain_walk",
            "checkpoint_write":{"state":"writing","directory":self.options.directory,"generation":generation,"state_path":path,"started_unix_time":started_unix_time},
            "committed_domains":state.published_count(),"contiguous_publication_watermark":state.queue.next,"committed_events":state.events,"family_closure_claim":false}),
        );
        write_file_atomically_with(&path, false, |f| codec::write(f, state, inputs, frontiers))?;
        let (bytes, digest) = file_digest(&path)?;
        let metadata = json!({"state":"saved","directory":self.options.directory,"generation":generation,"state_path":path,
            "saved_unix_time":SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_secs(),
            "committed_domains":state.published_count(),"pending_domains":state.queue.domains.len()-state.published_count(),
            "contiguous_publication_watermark":state.queue.next,
            "completed_native_inspections":state.completed,"committed_events":state.events,"paused":state.checkpoint_paused,
            "bytes":bytes,"started_unix_time":started_unix_time,"save_seconds":started.elapsed().as_secs_f64(),"duration_seconds":started.elapsed().as_secs_f64()});
        let manifest = Manifest {
            schema: self.schema,
            kind: "state".into(),
            generation,
            state_file,
            bytes,
            digest,
            request: self.request.clone(),
            executable: self.executable.clone(),
            owners: self.owners.clone(),
            metadata: metadata.clone(),
        };
        self.publish(manifest)?;
        let mut metadata = metadata;
        metadata["duration_seconds"] = json!(started.elapsed().as_secs_f64());
        metadata["saved_unix_time"] = json!(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs()
        );
        if let Some(manifest) = self.manifest.as_mut() {
            manifest.metadata = metadata.clone();
        }
        Ok(Some(
            json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false}),
        ))
    }
}

/// Small-fixture test seam; production always streams directly to a file.
#[cfg(test)]
pub(super) fn round_trip_state<const N: usize>(state: &State<N>) -> Result<State<N>, String> {
    let mut bytes = Vec::new();
    codec::write(&mut bytes, state, &[], &[])?;
    Ok(codec::read(bytes.as_slice())?.state)
}

#[cfg(test)]
fn test_directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../TMP")
        .join(format!(
            "checkpoint-store-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
    fs::create_dir(&path).unwrap();
    path
}

/// Uses actual fsync/atomic store and reopens it, for small native fixtures.
/// Fixture programs already reside in memory; the synthetic owner digest is
/// explicitly a test binding, not a claim about a production owner bundle.
#[cfg(test)]
pub(super) fn round_trip_state_on_disk<const N: usize>(
    state: &State<N>,
    request: &OwnerDomainWalkRequest,
) -> Result<State<N>, String> {
    let path = test_directory();
    let result = (|| {
        let mut request = request.clone();
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&path));
        let mut store = Store::open(&request)?.ok_or("test checkpoint store missing")?;
        store.bootstrap()?;
        store.bind_owners(vec!["test-only immutable in-memory native fixture".into()])?;
        store.save(state, &[], &[], true, &|_| {})?;
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        let mut store = Store::open(&request)?.ok_or("test resume store missing")?;
        store.bind_owners(vec!["test-only immutable in-memory native fixture".into()])?;
        Ok(store
            .resume::<N>()?
            .ok_or("test stateful checkpoint missing")?
            .state)
    })();
    fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    result
}

#[cfg(test)]
mod tests {
    use super::super::{OwnerDomainMatchRequest, queue::Queue};
    use super::*;
    use std::io::Write;
    fn directory() -> PathBuf {
        test_directory()
    }
    #[test]
    fn application_refinement_is_exact_checkpoint_policy() {
        use rustred::solver::OwnerAppliedCellRefinement;
        let path = directory();
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "selection".into(),
            "queries".into(),
        ));
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&path));
        let off = binding(&request);
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        assert_eq!(binding(&request), off);
        let resumed = Store::open(&request).unwrap().unwrap();
        drop(resumed);
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality: std::num::NonZeroUsize::new(2).unwrap(),
        };
        assert_ne!(binding(&request), off);
        assert!(Store::open(&request).is_err());
        // A new On checkpoint binds the full threshold, not just an enabled bit.
        fs::remove_dir_all(&path).unwrap();
        fs::create_dir(&path).unwrap();
        request.checkpoint.as_mut().unwrap().resume = false;
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        drop(Store::open(&request).unwrap().unwrap());
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality: std::num::NonZeroUsize::new(3).unwrap(),
        };
        assert!(Store::open(&request).is_err());
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::Off;
        assert!(Store::open(&request).is_err());
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn bootstrap_atomic_generations_owner_binding_and_corruption_rejection() {
        let path = directory();
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "selection".into(),
            "queries".into(),
        ));
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&path));
        let mut store = Store::open(&request).unwrap().unwrap();
        assert_eq!(request.checkpoint.as_ref().unwrap().interval_seconds, 3600);
        store.bootstrap().unwrap();
        assert_eq!(store.metadata().unwrap()["bootstrap"], true);
        assert!(Store::open(&request).is_err());
        store
            .bind_owners(vec!["actual payload digest".into()])
            .unwrap();
        let state = State::<1>::new(Queue::new(8, None), 0, None);
        for _ in 0..3 {
            store.save(&state, &[], &[], true, &|_| {}).unwrap();
        }
        assert!(!path.join("state-00000000000000000001.bin").exists());
        assert!(!path.join("state-00000000000000000002.bin").exists());
        assert!(path.join("state-00000000000000000003.bin").exists());
        assert!(path.join("state-00000000000000000004.bin").exists());
        let previous: Manifest =
            serde_json::from_reader(File::open(path.join("previous.json")).unwrap()).unwrap();
        assert_eq!(previous.generation, 3);
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        let mut resumed = Store::open(&request).unwrap().unwrap();
        assert!(resumed.bind_owners(vec!["changed payload".into()]).is_err());
        resumed
            .bind_owners(vec!["actual payload digest".into()])
            .unwrap();
        assert_eq!(resumed.resume::<1>().unwrap().unwrap().state.events, 0);
        drop(resumed);
        request.max_events += 1;
        assert!(Store::open(&request).is_err());
        request.max_events -= 1;
        let state_path = path.join("state-00000000000000000004.bin");
        OpenOptions::new()
            .append(true)
            .open(&state_path)
            .unwrap()
            .write_all(b"corruption")
            .unwrap();
        assert!(
            Store::open(&request)
                .err()
                .unwrap()
                .contains("checksum or length")
        );
        // Unique test-owned directory, never a caller-supplied campaign path.
        fs::remove_dir_all(path).unwrap();
    }
}
