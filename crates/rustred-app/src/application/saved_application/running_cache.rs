//! Bounded completed-record I/O, independent of a prepared reducer's lock.
//!
//! The worker owns only native records, a coefficient context and one pack
//! writer. Accepted records are not durable until their manifest checkpoint.
//! Call `flush`, `snapshot`, or `close` to obtain a publication barrier; dropping
//! a handle cannot report I/O errors and is not a durability acknowledgement.
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    time::{Duration, Instant},
};

use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
    reduction::{
        GuardedCacheBacking, GuardedCacheBackingError, GuardedCacheBackingErrorKind,
        GuardedIntermediateCapture,
    },
};
use symbolica::utils::resources::{ResourceBudget, checkpoint};

use super::{
    NativePackSnapshot, SavedApplicationCacheBundle, SavedApplicationCacheFile,
    SavedApplicationCachePack, SavedApplicationCachePackLimits, SavedApplicationCacheWriter,
};
use crate::{AppError, AppErrorKind};

#[cfg(test)]
mod closed_tests;
#[cfg(test)]
mod path_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod timer_tests;

/// An immutable base reader. Bases are searched in the supplied order after
/// the mutable delta. Opening/authenticating these readers remains explicit.
pub enum SavedApplicationRunningCacheBase {
    File(SavedApplicationCacheFile),
    Pack(SavedApplicationCachePack),
}
impl SavedApplicationRunningCacheBase {
    pub fn from_bundle(
        bundle: &SavedApplicationCacheBundle,
        binding: &ApplicationCacheBinding,
    ) -> Result<Self, AppError> {
        Ok(Self::Pack(bundle.open_reader(binding)?))
    }
    fn binding(&self) -> &ApplicationCacheBinding {
        match self {
            Self::File(reader) => reader.binding(),
            Self::Pack(reader) => reader.binding(),
        }
    }
    /// Share immutable native tables/directories, preserving a frozen pack's
    /// exact snapshot. This acquires no physical writer lease.
    pub fn fork_reader(&self) -> Self {
        match self {
            Self::File(reader) => Self::File(reader.fork_reader()),
            Self::Pack(reader) => Self::Pack(reader.fork_reader()),
        }
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        match self {
            Self::File(reader) => reader.lookup(key, context),
            Self::Pack(reader) => reader.lookup(key, context),
        }
    }
}

/// Queue/batch grants are independent of the codec, disk and ambient resource
/// grants. Bytes charge coefficients, guards, keys and conservative map/index
/// metadata, excluding allocator overhead and temporary codec scratch.
#[derive(Clone, Copy, Debug)]
pub struct SavedApplicationRunningCacheOptions {
    pub storage: SavedApplicationCachePackLimits,
    pub max_pending_records: usize,
    pub max_pending_bytes: usize,
    pub batch_records: usize,
    pub batch_bytes: usize,
    pub checkpoint_interval: Duration,
}
impl Default for SavedApplicationRunningCacheOptions {
    fn default() -> Self {
        Self {
            storage: Default::default(),
            max_pending_records: 128,
            max_pending_bytes: 256 * 1024 * 1024,
            batch_records: 64,
            batch_bytes: 64 * 1024 * 1024,
            checkpoint_interval: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SavedApplicationRunningCacheReport {
    /// Records admitted to this worker, excluding duplicate pending keys.
    pub accepted_records: u64,
    /// Accepted records covered by successful manifest checkpoints this session.
    pub committed_records: u64,
    pub pending_records: usize,
    pub pending_bytes: usize,
    /// Last acknowledged manifest generation, including a resumed base delta.
    pub generation: u64,
    /// Age since this session acknowledged the generation. On resume this
    /// starts at attachment, not at the historical filesystem checkpoint.
    pub checkpoint_age: Option<Duration>,
    pub last_error: Option<AppError>,
    /// First terminal worker failure; later admission errors cannot replace it.
    pub writer_error: Option<AppError>,
    pub capture_skipped_records: u64,
    pub capture_metadata_exhausted: bool,
    pub closed: bool,
}
impl SavedApplicationRunningCacheReport {
    pub fn to_document(&self) -> serde_json::Value {
        serde_json::json!({
            "schema":"rustred.running-application-cache.v1",
            "accepted_records":self.accepted_records,"committed_records":self.committed_records,
            "pending_records":self.pending_records,"pending_bytes":self.pending_bytes,
            "generation":self.generation,"checkpoint_age_seconds":self.checkpoint_age.map(|age| age.as_secs_f64()),
            "checkpoint_age_scope":"session_acknowledgement",
            "last_error":self.last_error.as_ref().map(|error| serde_json::json!({
                "kind":error.kind().as_str(),"message":error.to_string(),
                "resource_limit":error.resource_limit_error().map(|cause| cause.to_string()),
            })),
            "writer_error":self.writer_error.as_ref().map(|error| serde_json::json!({
                "kind":error.kind().as_str(),"message":error.to_string(),
                "resource_limit":error.resource_limit_error().map(|cause| cause.to_string()),
            })),
            "capture_skipped_records":self.capture_skipped_records,
            "capture_metadata_exhausted":self.capture_metadata_exhausted,
            "closed":self.closed,"writer_mode":"thread",
        })
    }
}

/// A small stable delta descriptor. Immutable bases remain the caller's
/// explicit configuration; this operation never copies their payloads.
#[derive(Clone, Debug)]
pub struct SavedApplicationRunningCacheSnapshot {
    pub directory: PathBuf,
    pub binding: ApplicationCacheBinding,
    pub delta: NativePackSnapshot,
}
impl SavedApplicationRunningCacheSnapshot {
    /// Parse the small descriptor only. Opening its frozen native manifest
    /// separately verifies the binding, exact generation, length and digest.
    /// This performs no filesystem access or coefficient import.
    pub fn from_document(document: &serde_json::Value) -> Result<Self, AppError> {
        use serde_json::Value;
        fn object<'a>(
            value: &'a Value,
            fields: &[&str],
        ) -> Result<&'a serde_json::Map<String, Value>, AppError> {
            let object = value
                .as_object()
                .ok_or_else(|| AppError::input("cache snapshot descriptor must be an object"))?;
            if object.len() != fields.len()
                || fields.iter().any(|field| !object.contains_key(*field))
            {
                return Err(AppError::input(
                    "cache snapshot descriptor fields differ from schema",
                ));
            }
            Ok(object)
        }
        fn string(value: &Value) -> Result<&str, AppError> {
            value
                .as_str()
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    AppError::input("cache snapshot descriptor requires a nonempty string")
                })
        }
        fn number(value: &Value) -> Result<u64, AppError> {
            value.as_u64().ok_or_else(|| {
                AppError::input("cache snapshot descriptor requires an unsigned integer")
            })
        }
        fn digest(value: &Value) -> Result<[u8; 32], AppError> {
            let text = string(value)?;
            if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(AppError::input(
                    "cache snapshot digest must have exactly 64 hexadecimal digits",
                ));
            }
            let mut result = [0u8; 32];
            for (offset, byte) in result.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&text[offset * 2..offset * 2 + 2], 16)
                    .map_err(|_| AppError::input("invalid cache snapshot digest"))?;
            }
            Ok(result)
        }
        fn path(value: &Value) -> Result<PathBuf, AppError> {
            let path = PathBuf::from(string(value)?);
            if !path.is_absolute() {
                return Err(AppError::input("cache snapshot paths must be absolute"));
            }
            Ok(path)
        }
        let root = object(document, &["schema", "directory", "binding", "delta"])?;
        if string(&root["schema"])? != "rustred.running-application-cache-snapshot.v1" {
            return Err(AppError::input(
                "unsupported cache snapshot descriptor schema",
            ));
        }
        let binding = object(
            &root["binding"],
            &[
                "artifact_digest",
                "family_fingerprint",
                "physical_arity",
                "layer",
            ],
        )?;
        let binding = ApplicationCacheBinding {
            artifact_digest: digest(&binding["artifact_digest"])?,
            family_fingerprint: string(&binding["family_fingerprint"])?.to_owned(),
            physical_arity: usize::try_from(number(&binding["physical_arity"])?)
                .map_err(|_| AppError::input("cache snapshot arity exceeds address space"))?,
            layer: string(&binding["layer"])?.to_owned(),
        };
        binding.validate(Default::default()).map_err(binary_error)?;
        let delta = object(
            &root["delta"],
            &[
                "manifest_path",
                "generation",
                "manifest_bytes",
                "manifest_digest",
            ],
        )?;
        let manifest_bytes = number(&delta["manifest_bytes"])?;
        if manifest_bytes == 0 {
            return Err(AppError::input("cache snapshot manifest cannot be empty"));
        }
        Ok(Self {
            directory: path(&root["directory"])?,
            binding,
            delta: NativePackSnapshot {
                manifest_path: path(&delta["manifest_path"])?,
                generation: number(&delta["generation"])?,
                manifest_bytes,
                manifest_digest: digest(&delta["manifest_digest"])?,
            },
        })
    }
    pub fn to_document(&self) -> serde_json::Value {
        serde_json::json!({
            "schema":"rustred.running-application-cache-snapshot.v1",
            "directory":self.directory,"binding":{
                "artifact_digest":blake3::Hash::from_bytes(self.binding.artifact_digest).to_hex().to_string(),
                "family_fingerprint":self.binding.family_fingerprint,
                "physical_arity":self.binding.physical_arity,"layer":self.binding.layer,
            },"delta":{
                "manifest_path":self.delta.manifest_path,"generation":self.delta.generation,
                "manifest_bytes":self.delta.manifest_bytes,
                "manifest_digest":blake3::Hash::from_bytes(self.delta.manifest_digest).to_hex().to_string(),
            },
        })
    }
}

/// Immutable bases only, including explicitly frozen pack readers. This opens
/// no writer and captures no records; native reduction still validates hits.
/// A validated completed-root store is a checked no-op, so Closed-program
/// misses can compute normally without training this read-only cache.
pub struct SavedApplicationReadOnlyCache {
    binding: ApplicationCacheBinding,
    context: CoefficientContext,
    bases: Vec<SavedApplicationRunningCacheBase>,
}
impl SavedApplicationReadOnlyCache {
    pub fn new(
        binding: ApplicationCacheBinding,
        context: CoefficientContext,
        bases: Vec<SavedApplicationRunningCacheBase>,
    ) -> Result<Self, AppError> {
        if bases.iter().any(|base| base.binding() != &binding) {
            return Err(AppError::input("read-only cache base binding mismatch"));
        }
        Ok(Self {
            binding,
            context,
            bases,
        })
    }
    pub fn fork_reader(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            context: self.context.clone(),
            bases: self
                .bases
                .iter()
                .map(SavedApplicationRunningCacheBase::fork_reader)
                .collect(),
        }
    }
}
impl GuardedCacheBacking for SavedApplicationReadOnlyCache {
    fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        let result = (|| {
            check_context(&self.context, context)?;
            for base in &mut self.bases {
                if let Some(record) = base.lookup(key, context)? {
                    return Ok(Some(record));
                }
            }
            Ok(None)
        })();
        result.map_err(backing_error)
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        check_store_boundary(&self.binding, &self.context, record, context).map_err(backing_error)
    }
}

struct State {
    accepted: u64,
    committed: u64,
    pending: usize,
    bytes: usize,
    reserved: usize,
    reserved_bytes: usize,
    keys: BTreeSet<ApplicationCacheKey>,
    generation: u64,
    checkpoint: Instant,
    last_error: Option<AppError>,
    failure: Option<AppError>,
    capture_skipped_records: u64,
    capture_metadata_exhausted: bool,
    closing: bool,
    closed: bool,
}
impl State {
    fn report(&self) -> SavedApplicationRunningCacheReport {
        SavedApplicationRunningCacheReport {
            accepted_records: self.accepted,
            committed_records: self.committed,
            pending_records: self.pending,
            pending_bytes: self.bytes,
            generation: self.generation,
            checkpoint_age: Some(self.checkpoint.elapsed()),
            last_error: self.last_error.clone(),
            writer_error: self.failure.clone(),
            capture_skipped_records: self.capture_skipped_records,
            capture_metadata_exhausted: self.capture_metadata_exhausted,
            closed: self.closed,
        }
    }
    fn healthy(&self) -> Result<(), AppError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.closed {
            return Err(AppError::execution("running cache is closed"));
        }
        if self.closing {
            return Err(AppError::execution("running cache is closing"));
        }
        Ok(())
    }
}
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
impl Shared {
    fn lock(&self) -> Result<MutexGuard<'_, State>, AppError> {
        self.state
            .lock()
            .map_err(|_| AppError::internal_invariant("running cache state lock poisoned"))
    }
    fn record_error(&self, error: &AppError, terminal: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.last_error = Some(error.clone());
            if terminal && state.failure.is_none() {
                state.failure = Some(error.clone());
            }
        }
        self.changed.notify_all();
    }
}

struct Queued {
    record: ApplicationCacheRecord,
    bytes: usize,
    budget: ResourceBudget,
}
type Reply<T> = mpsc::Sender<Result<T, AppError>>;
enum Message {
    Record(Queued),
    Flush(ResourceBudget, Reply<SavedApplicationRunningCacheReport>),
    Snapshot(
        PathBuf,
        ResourceBudget,
        Reply<SavedApplicationRunningCacheSnapshot>,
    ),
    Close(ResourceBudget, Reply<SavedApplicationRunningCacheReport>),
}

/// Cloneable I/O control. Its locks cover only queue/report metadata; it never
/// owns, waits for, or borrows a prepared application or reduction engine.
#[derive(Clone)]
pub struct SavedApplicationRunningCacheControl {
    sender: mpsc::SyncSender<Message>,
    shared: Arc<Shared>,
}
impl SavedApplicationRunningCacheControl {
    pub fn report(&self) -> Result<SavedApplicationRunningCacheReport, AppError> {
        Ok(self.shared.lock()?.report())
    }
    pub fn flush(&self) -> Result<SavedApplicationRunningCacheReport, AppError> {
        let (send, receive) = mpsc::channel();
        self.command(Message::Flush(ResourceBudget::capture_current(), send))?;
        self.receive(receive)
    }
    pub fn snapshot(
        &self,
        sidecar: &Path,
    ) -> Result<SavedApplicationRunningCacheSnapshot, AppError> {
        let (send, receive) = mpsc::channel();
        self.command(Message::Snapshot(
            sidecar.to_path_buf(),
            ResourceBudget::capture_current(),
            send,
        ))?;
        self.receive(receive)
    }
    /// Flush and release the writer lock. All clones then report closed. An
    /// error is explicit: accepted-but-uncommitted records were not saved.
    pub fn close(&self) -> Result<SavedApplicationRunningCacheReport, AppError> {
        let (send, receive) = mpsc::channel();
        self.command(Message::Close(ResourceBudget::capture_current(), send))?;
        self.receive(receive)
    }
    fn command(&self, mut message: Message) -> Result<(), AppError> {
        let closes = matches!(&message, Message::Close(..));
        loop {
            checkpoint().map_err(AppError::resource)?;
            let mut state = self.shared.lock()?;
            // Close must also release a writer whose I/O has already failed.
            if state.closed {
                return Err(AppError::execution("running cache is closed"));
            }
            if state.closing {
                return Err(AppError::execution("running cache is closing"));
            }
            match self.sender.try_send(message) {
                Ok(()) => {
                    if closes {
                        // The queue and accepted-record counters share this
                        // mutex. Seal admission only after Close is enqueued,
                        // so every accepted record precedes it in the queue.
                        state.closing = true;
                        self.shared.changed.notify_all();
                    }
                    return Ok(());
                }
                Err(mpsc::TrySendError::Full(returned)) => {
                    message = returned;
                    drop(
                        self.shared
                            .changed
                            .wait_timeout(state, Duration::from_millis(50))
                            .map_err(|_| {
                                AppError::internal_invariant("running cache wait lock poisoned")
                            })?,
                    );
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err(state.failure.clone().unwrap_or_else(|| {
                        AppError::execution("running cache writer disconnected")
                    }));
                }
            }
        }
    }
    fn receive<T>(&self, receive: mpsc::Receiver<Result<T, AppError>>) -> Result<T, AppError> {
        loop {
            match receive.recv_timeout(Duration::from_millis(50)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => checkpoint().map_err(AppError::resource)?,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(self.shared.lock()?.failure.clone().unwrap_or_else(|| {
                        AppError::execution("running cache writer disconnected")
                    }));
                }
            }
        }
    }
}

/// One process delta plus immutable bases. The writer's native lock rejects a
/// second writer to the same directory; use a separate delta in each process.
pub struct SavedApplicationRunningCache {
    binding: ApplicationCacheBinding,
    context: CoefficientContext,
    delta: SavedApplicationCachePack,
    bases: Vec<SavedApplicationRunningCacheBase>,
    reader_generation: u64,
    control: SavedApplicationRunningCacheControl,
    options: SavedApplicationRunningCacheOptions,
}
impl SavedApplicationRunningCache {
    pub fn open(
        binding: ApplicationCacheBinding,
        context: CoefficientContext,
        delta_directory: &Path,
        bases: Vec<SavedApplicationRunningCacheBase>,
        options: SavedApplicationRunningCacheOptions,
    ) -> Result<Self, AppError> {
        if options.max_pending_records == 0
            || options.max_pending_bytes == 0
            || options.batch_records == 0
            || options.batch_bytes == 0
            || options.checkpoint_interval.is_zero()
            || options.batch_records > options.max_pending_records
            || options.batch_bytes > options.max_pending_bytes
        {
            return Err(AppError::input(
                "running cache requires positive queue/batch grants and interval, with batches within queue grants",
            ));
        }
        binding
            .validate(options.storage.chunk)
            .map_err(binary_error)?;
        if bases.iter().any(|base| base.binding() != &binding) {
            return Err(AppError::input("running cache base binding mismatch"));
        }
        // Resolve before acquiring the writer lease: the worker can outlive a
        // caller's current working directory and must keep writing at its lock.
        let directory = if delta_directory.exists() {
            std::fs::canonicalize(delta_directory).map_err(|e| AppError::input(e.to_string()))?
        } else {
            let parent = delta_directory
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let basename = delta_directory
                .file_name()
                .ok_or_else(|| AppError::input("running cache delta requires a directory name"))?;
            std::fs::canonicalize(parent)
                .map_err(|e| AppError::input(e.to_string()))?
                .join(basename)
        };
        let writer = if directory.exists() {
            SavedApplicationCacheWriter::resume(&directory, &binding, options.storage)?
        } else {
            SavedApplicationCacheWriter::create(&directory, binding.clone(), options.storage)?
        };
        let delta =
            SavedApplicationCachePack::open_generated(&directory, &binding, options.storage)?;
        let generation = delta.generation();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                accepted: 0,
                committed: 0,
                pending: 0,
                bytes: 0,
                reserved: 0,
                reserved_bytes: 0,
                keys: BTreeSet::new(),
                generation,
                checkpoint: Instant::now(),
                last_error: None,
                failure: None,
                capture_skipped_records: 0,
                capture_metadata_exhausted: false,
                closing: false,
                closed: false,
            }),
            changed: Condvar::new(),
        });
        let capacity = options
            .max_pending_records
            .checked_add(1)
            .ok_or_else(|| AppError::limit("running cache channel capacity overflow"))?;
        let queue_bytes = capacity
            .checked_mul(std::mem::size_of::<Message>())
            .ok_or_else(|| AppError::limit("running cache channel byte count overflow"))?;
        symbolica::utils::resources::check_allocation(queue_bytes as u64)
            .map_err(AppError::resource)?;
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let control = SavedApplicationRunningCacheControl {
            sender,
            shared: shared.clone(),
        };
        let worker = Worker {
            writer,
            directory,
            binding: binding.clone(),
            context: context.clone(),
            options,
            receiver,
            shared: shared.clone(),
            resources: ResourceBudget::capture_current(),
            records: Vec::new(),
            budgets: Vec::new(),
            bytes: 0,
        };
        // Owned records/context are naturally Send. No engine, borrowed CAS
        // object or unsafe Send/Sync assertion crosses this boundary.
        let _worker = std::thread::Builder::new()
            .name("rustred-cache-writer".into())
            .spawn(move || {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| worker.run())).is_err()
                {
                    shared
                        .record_error(&AppError::execution("running cache writer panicked"), true);
                    if let Ok(mut state) = shared.state.lock() {
                        state.closed = true;
                    }
                    shared.changed.notify_all();
                }
            })
            .map_err(|e| AppError::execution(format!("cannot start running cache writer: {e}")))?;
        Ok(Self {
            binding,
            context,
            delta,
            bases,
            reader_generation: generation,
            control,
            options,
        })
    }
    pub fn control(&self) -> SavedApplicationRunningCacheControl {
        self.control.clone()
    }
    /// Transfer settings to another session view without reopening the delta
    /// writer. Both views share its one queue and physical writer lease.
    pub fn fork(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            context: self.context.clone(),
            delta: self.delta.fork_reader(),
            bases: self
                .bases
                .iter()
                .map(SavedApplicationRunningCacheBase::fork_reader)
                .collect(),
            reader_generation: self.reader_generation,
            control: self.control.clone(),
            options: self.options,
        }
    }
    /// Short split borrows over the existing native reduction call. A native
    /// mathematical failure keeps precedence over a simultaneous I/O failure.
    pub fn with_io<R>(
        &mut self,
        run: impl FnOnce(
            &mut dyn GuardedCacheBacking,
            &mut dyn GuardedIntermediateCapture,
        ) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        self.control.shared.lock()?.healthy()?;
        let mut backing = Backing {
            binding: &self.binding,
            context: &self.context,
            delta: &mut self.delta,
            bases: &mut self.bases,
            generation: &mut self.reader_generation,
            control: self.control.clone(),
            options: self.options,
        };
        let mut capture = Capture {
            context: &self.context,
            control: self.control.clone(),
            options: self.options,
        };
        let result = run(&mut backing, &mut capture);
        self.finish_call(result)
    }
    /// Productive capture without consulting any backing cache.
    pub fn with_capture<R>(
        &mut self,
        run: impl FnOnce(&mut dyn GuardedIntermediateCapture) -> Result<R, AppError>,
    ) -> Result<R, AppError> {
        self.control.shared.lock()?.healthy()?;
        let mut capture = Capture {
            context: &self.context,
            control: self.control.clone(),
            options: self.options,
        };
        let result = run(&mut capture);
        self.finish_call(result)
    }
    fn finish_call<R>(&self, result: Result<R, AppError>) -> Result<R, AppError> {
        let flushed = self.control.flush();
        if let Err(error) = &flushed {
            self.control.shared.record_error(error, false);
        }
        match result {
            Err(error) => Err(error),
            Ok(value) => flushed.map(|_| value),
        }
    }
}

struct Backing<'a> {
    binding: &'a ApplicationCacheBinding,
    context: &'a CoefficientContext,
    delta: &'a mut SavedApplicationCachePack,
    bases: &'a mut [SavedApplicationRunningCacheBase],
    generation: &'a mut u64,
    control: SavedApplicationRunningCacheControl,
    options: SavedApplicationRunningCacheOptions,
}
impl GuardedCacheBacking for Backing<'_> {
    fn binding(&self) -> &ApplicationCacheBinding {
        self.binding
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheBackingError> {
        self.lookup_inner(key, context).map_err(backing_error)
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        check_store_boundary(self.binding, self.context, record, context).map_err(backing_error)?;
        // Closed programs publish their already validated root through the
        // backing trait. Reuse the same bounded queue and physical writer as
        // intermediate capture; a pending duplicate is acknowledged once.
        Capture {
            context: self.context,
            control: self.control.clone(),
            options: self.options,
        }
        .store_completed(record, context)
    }
}
impl Backing<'_> {
    fn lookup_inner(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        check_context(self.context, context)?;
        let latest = {
            let state = self.control.shared.lock()?;
            state.healthy()?;
            state.generation
        };
        if latest != *self.generation {
            self.delta.refresh()?;
            *self.generation = self.delta.generation();
        }
        if let Some(record) = self.delta.lookup(key, context)? {
            return Ok(Some(record));
        }
        for base in self.bases.iter_mut() {
            if let Some(record) = base.lookup(key, context)? {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }
}

struct Capture<'a> {
    context: &'a CoefficientContext,
    control: SavedApplicationRunningCacheControl,
    options: SavedApplicationRunningCacheOptions,
}
struct Reservation<'a> {
    shared: &'a Shared,
    key: &'a ApplicationCacheKey,
    bytes: usize,
    active: bool,
}
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        if self.active {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.reserved -= 1;
            state.reserved_bytes -= self.bytes;
            state.keys.remove(self.key);
            self.shared.changed.notify_all();
        }
    }
}
impl GuardedIntermediateCapture for Capture<'_> {
    fn capture_backing_hits(&self) -> bool {
        false
    }
    fn fail_on_materialization_limit(&self) -> bool {
        true
    }
    fn wants(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<bool, GuardedCacheBackingError> {
        let result = (|| {
            check_context(self.context, context)?;
            checkpoint().map_err(AppError::resource)?;
            let state = self.control.shared.lock()?;
            state.healthy()?;
            Ok(!state.keys.contains(key))
        })();
        result.map_err(|error| {
            self.control.shared.record_error(&error, false);
            backing_error(error)
        })
    }
    fn store_completed(
        &mut self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), GuardedCacheBackingError> {
        let result = self.enqueue(record, context);
        result.map_err(|error| {
            self.control.shared.record_error(&error, false);
            backing_error(error)
        })
    }
    fn metadata_limit_bytes(&self) -> Option<usize> {
        // Queue grants govern owned transport records, not native guard
        // provenance. Retain the engine's mandatory native allowance.
        None
    }
    fn report_statistics(&mut self, statistics: rustred::reduction::GuardedCaptureStatistics) {
        if let Ok(mut state) = self.control.shared.state.lock() {
            state.capture_skipped_records = state
                .capture_skipped_records
                .saturating_add(statistics.skipped_records as u64);
            state.capture_metadata_exhausted |= statistics.metadata_exhausted;
        }
    }
}
impl Capture<'_> {
    fn enqueue(
        &self,
        record: &ApplicationCacheRecord,
        context: &CoefficientContext,
    ) -> Result<(), AppError> {
        check_context(self.context, context)?;
        let bytes = record_bytes(record)?;
        if bytes > self.options.max_pending_bytes {
            return Err(AppError::limit(
                "completed record exceeds running cache pending-byte grant",
            ));
        }
        let key = &record.key;
        loop {
            checkpoint().map_err(AppError::resource)?;
            let mut state = self.control.shared.lock()?;
            state.healthy()?;
            if state.keys.contains(key) {
                return Ok(());
            }
            let count = state
                .pending
                .checked_add(state.reserved)
                .and_then(|n| n.checked_add(1));
            let total = state
                .bytes
                .checked_add(state.reserved_bytes)
                .and_then(|n| n.checked_add(bytes));
            if count.is_some_and(|n| n <= self.options.max_pending_records)
                && total.is_some_and(|n| n <= self.options.max_pending_bytes)
            {
                state.reserved += 1;
                state.reserved_bytes += bytes;
                state.keys.insert(key.clone());
                break;
            }
            drop(
                self.control
                    .shared
                    .changed
                    .wait_timeout(state, Duration::from_millis(50))
                    .map_err(|_| {
                        AppError::internal_invariant("running cache backpressure lock poisoned")
                    })?,
            );
        }
        let mut reservation = Reservation {
            shared: &self.control.shared,
            key,
            bytes,
            active: true,
        };
        let cloned = record.try_clone_with_resources().map_err(binary_error)?;
        let mut message = Message::Record(Queued {
            record: cloned,
            bytes,
            budget: ResourceBudget::capture_current(),
        });
        loop {
            checkpoint().map_err(AppError::resource)?;
            let mut state = self.control.shared.lock()?;
            state.healthy()?;
            let accepted = state
                .accepted
                .checked_add(1)
                .ok_or_else(|| AppError::limit("running cache accepted count overflow"))?;
            match self.control.sender.try_send(message) {
                Ok(()) => {
                    state.reserved -= 1;
                    state.reserved_bytes -= bytes;
                    reservation.active = false;
                    state.pending += 1;
                    state.bytes += bytes;
                    state.accepted = accepted;
                    return Ok(());
                }
                Err(mpsc::TrySendError::Full(returned)) => {
                    message = returned;
                    drop(
                        self.control
                            .shared
                            .changed
                            .wait_timeout(state, Duration::from_millis(50))
                            .map_err(|_| {
                                AppError::internal_invariant("running cache enqueue lock poisoned")
                            })?,
                    );
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    let error = state.failure.clone().unwrap_or_else(|| {
                        AppError::execution("running cache writer disconnected")
                    });
                    return Err(error);
                }
            }
        }
    }
}

struct Worker {
    writer: SavedApplicationCacheWriter,
    directory: PathBuf,
    binding: ApplicationCacheBinding,
    context: CoefficientContext,
    options: SavedApplicationRunningCacheOptions,
    receiver: mpsc::Receiver<Message>,
    shared: Arc<Shared>,
    resources: ResourceBudget,
    records: Vec<ApplicationCacheRecord>,
    budgets: Vec<ResourceBudget>,
    bytes: usize,
}
impl Worker {
    fn run(mut self) {
        loop {
            let (elapsed, failed) = self
                .shared
                .lock()
                .map(|state| (state.checkpoint.elapsed(), state.failure.is_some()))
                .unwrap_or((Duration::ZERO, true));
            if !self.records.is_empty() && !failed && elapsed >= self.options.checkpoint_interval {
                // A zero receive timeout can still return a ready record.
                // Publish an overdue batch before consuming more ingress.
                let _ = self.commit(None);
                continue;
            }
            let message = if self.records.is_empty() || failed {
                self.receiver
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            } else {
                self.receiver
                    .recv_timeout(self.options.checkpoint_interval.saturating_sub(elapsed))
            };
            self.shared.changed.notify_all();
            match message {
                Ok(Message::Record(queued)) => {
                    if !self.records.is_empty()
                        && self
                            .bytes
                            .checked_add(queued.bytes)
                            .is_none_or(|n| n > self.options.batch_bytes)
                    {
                        let _ = self.commit(None);
                    }
                    self.bytes += queued.bytes;
                    self.records.push(queued.record);
                    self.budgets.push(queued.budget);
                    if self.records.len() >= self.options.batch_records
                        || self.bytes >= self.options.batch_bytes
                    {
                        let _ = self.commit(None);
                    }
                }
                Ok(Message::Flush(budget, reply)) => {
                    let result = self
                        .commit(Some(&budget))
                        .and_then(|()| Ok(self.shared.lock()?.report()));
                    let _ = reply.send(result);
                }
                Ok(Message::Snapshot(path, budget, reply)) => {
                    let result = self
                        .commit(Some(&budget))
                        .and_then(|()| {
                            self.resources
                                .run_captured(|| budget.run(|| self.writer.freeze_snapshot(&path)))
                        })
                        .map(|delta| SavedApplicationRunningCacheSnapshot {
                            directory: self.directory.clone(),
                            binding: self.binding.clone(),
                            delta,
                        });
                    if let Err(error) = &result {
                        self.shared.record_error(error, false);
                    }
                    let _ = reply.send(result);
                }
                Ok(Message::Close(budget, reply)) => {
                    let result = self.commit(Some(&budget));
                    let report = self.shared.lock().map(|mut state| {
                        state.closed = true;
                        state.report()
                    });
                    drop(self.writer);
                    self.shared.changed.notify_all();
                    let _ = reply.send(result.and(report));
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = self.commit(None);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = self.commit(None);
                    if let Ok(mut state) = self.shared.lock() {
                        state.closed = true;
                    }
                    self.shared.changed.notify_all();
                    return;
                }
            }
        }
    }
    fn commit(&mut self, extra: Option<&ResourceBudget>) -> Result<(), AppError> {
        {
            let state = self.shared.lock()?;
            if let Some(error) = &state.failure {
                return Err(error.clone());
            }
        }
        if self.records.is_empty() {
            return Ok(());
        }
        self.records.sort_by(|a, b| a.key.cmp(&b.key));
        let generation = self
            .shared
            .lock()?
            .generation
            .checked_add(1)
            .ok_or_else(|| AppError::limit("running cache generation overflow"))?;
        let result = self.resources.run_captured(|| {
            let mut operation = || {
                under_budgets(&self.budgets, || {
                    self.writer.append_completed(&self.records, &self.context)?;
                    self.writer.checkpoint()
                })
            };
            match extra {
                Some(budget) => budget.run(operation),
                None => operation(),
            }
        });
        if let Err(error) = result {
            self.shared.record_error(&error, true);
            return Err(error);
        }
        {
            let mut state = self.shared.lock()?;
            for record in &self.records {
                state.keys.remove(&record.key);
            }
            state.pending -= self.records.len();
            state.bytes -= self.bytes;
            state.committed += self.records.len() as u64;
            state.generation = generation;
            state.checkpoint = Instant::now();
        }
        self.records.clear();
        self.budgets.clear();
        self.bytes = 0;
        self.shared.changed.notify_all();
        Ok(())
    }
}

fn under_budgets<R>(budgets: &[ResourceBudget], run: impl FnOnce() -> R) -> R {
    let mut merged = ResourceBudget::capture_current();
    for budget in budgets {
        merged = merged.run_captured(|| budget.run(ResourceBudget::capture_current));
    }
    merged.run_captured(run)
}
fn check_context(
    expected: &CoefficientContext,
    actual: &CoefficientContext,
) -> Result<(), AppError> {
    if !expected.has_same_variable_map(actual) {
        Err(AppError::input(
            "running cache coefficient context mismatch",
        ))
    } else {
        Ok(())
    }
}
fn check_store_boundary(
    binding: &ApplicationCacheBinding,
    expected: &CoefficientContext,
    record: &ApplicationCacheRecord,
    actual: &CoefficientContext,
) -> Result<(), AppError> {
    checkpoint().map_err(AppError::resource)?;
    check_context(expected, actual)?;
    binding.validate(Default::default()).map_err(binary_error)?;
    record
        .key
        .validate(binding.physical_arity)
        .map_err(binary_error)?;
    for (target, coefficient) in &record.terms {
        if target.powers().len() != binding.physical_arity {
            return Err(AppError::input(
                "completed cache term arity differs from binding",
            ));
        }
        expected
            .validate_with_limits(coefficient, Default::default())
            .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))?;
    }
    for guard in &record.nonzero_conditions {
        expected
            .validate_with_limits(guard, Default::default())
            .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))?;
    }
    checkpoint().map_err(AppError::resource)
}
fn record_bytes(record: &ApplicationCacheRecord) -> Result<usize, AppError> {
    let (_, coefficients) = record.coefficient_census().map_err(binary_error)?;
    let overflow = || AppError::limit("running cache record byte count overflow");
    let mut bytes = coefficients
        .checked_add(std::mem::size_of::<ApplicationCacheRecord>())
        .and_then(|n| n.checked_add(record.key.owned_heap_bytes().saturating_mul(2)))
        .and_then(|n| n.checked_add(std::mem::size_of::<ApplicationCacheKey>() + 128))
        .ok_or_else(overflow)?;
    for target in record.terms.keys() {
        bytes = bytes
            .checked_add(
                target
                    .powers()
                    .len()
                    .checked_mul(std::mem::size_of::<i64>())
                    .ok_or_else(overflow)?,
            )
            .and_then(|n| n.checked_add(128))
            .ok_or_else(overflow)?;
    }
    Ok(bytes)
}
fn binary_error(error: rustred::persistence::BinaryIoError) -> AppError {
    AppError::from_native(error, AppErrorKind::Limit)
}
fn backing_error(error: AppError) -> GuardedCacheBackingError {
    if let Some(resource) = error.resource_limit_error() {
        return GuardedCacheBackingError::resource(resource.clone());
    }
    GuardedCacheBackingError {
        kind: if error.kind() == AppErrorKind::Limit {
            GuardedCacheBackingErrorKind::Limit
        } else {
            GuardedCacheBackingErrorKind::Invalid
        },
        message: error.to_string(),
    }
}
