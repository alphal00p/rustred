//! Lazy Custom payloads and the program budget (p5l-library session 2).
//!
//! A 5-loop library carries ~10^2 GB of Custom payloads; reading and decoding
//! them all at load does not fit a machine. Two pieces make a package load
//! independent of its payload bytes:
//!
//! - [`PPayloadFile`]: a payload that stays on disk until its owner's program
//!   is first built. The first read checks its length and the loader's
//!   structural check (the record's meta, residuals and family) and keeps the
//!   BLAKE3 digest of the verified bytes; every later read (after an eviction)
//!   must give the same digest. Values never depend on when a payload is read.
//! - [`PProgramBudget`] with [`PEvictableStep`]: the program of a Custom owner
//!   with a lazy payload is built on its first step and counted against a
//!   memory budget (an estimate: payload bytes times a decode factor); when the
//!   resident estimate exceeds the budget, the least recently used programs
//!   that no step is using are dropped and rebuilt from their payload on their
//!   next use. A rebuilt program is the same program (the build is a pure
//!   function of the verified bytes, the presentation, the meta and the rule
//!   order), so eviction changes time and memory, never a value.
//!
//! Settings (read once per library build): `RUSTRED_P_PROGRAM_BUDGET_GB`
//! (`0`: no eviction; default 32 for a library whose scope reaches 5 loops, no
//! eviction otherwise) and `RUSTRED_P_PROGRAM_WEIGHT` (decode factor, default 4:
//! the resident program of a payload of `b` bytes is counted as `4 b`).
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};

use super::error::PError;
use super::lane::OwnerLane;
use super::step::{PKeyView, PLaneStep, POrderKey, PStepContext, PStepOut};

/// The check of a payload's bytes on its first read (the loader's structural
/// check against its record).
pub type PPayloadCheck = Arc<dyn Fn(&[u8]) -> Result<(), PError> + Send + Sync>;

/// The decoding of a stored payload file into the payload bytes (a
/// compressed package, p5l-library session 2), applied on every read.
pub type PPayloadDecode = Arc<dyn Fn(Vec<u8>) -> Result<Vec<u8>, PError> + Send + Sync>;

/// A Custom payload on disk, read on use.
pub struct PPayloadFile {
    path: PathBuf,
    bytes: u64,
    check: Option<PPayloadCheck>,
    decode: Option<PPayloadDecode>,
    digest: OnceLock<[u8; 32]>,
    reads: AtomicU64,
}

impl fmt::Debug for PPayloadFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PPayloadFile")
            .field("path", &self.path)
            .field("bytes", &self.bytes)
            .field("verified", &self.digest.get().is_some())
            .finish()
    }
}

impl PPayloadFile {
    /// The payload `path` of exactly `bytes` bytes, checked by `check` on its
    /// first read.
    pub fn new(path: PathBuf, bytes: u64, check: Option<PPayloadCheck>) -> Self {
        Self {
            path,
            bytes,
            check,
            decode: None,
            digest: OnceLock::new(),
            reads: AtomicU64::new(0),
        }
    }

    /// [`Self::new`] of a stored file of `bytes` bytes that `decode` turns
    /// into the payload (the digest and the check see the decoded payload).
    pub fn with_decode(
        path: PathBuf,
        bytes: u64,
        check: Option<PPayloadCheck>,
        decode: PPayloadDecode,
    ) -> Self {
        Self {
            decode: Some(decode),
            ..Self::new(path, bytes, check)
        }
    }

    /// The stored file is decoded on read (a compressed payload).
    pub fn is_encoded(&self) -> bool {
        self.decode.is_some()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The recorded length in bytes of the stored file.
    pub fn len(&self) -> u64 {
        self.bytes
    }

    pub fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    /// How often the file was read.
    pub fn reads(&self) -> u64 {
        self.reads.load(Ordering::Relaxed)
    }

    /// The bytes: the recorded length, checked on the first read, and the
    /// digest of the first read on every later one.
    pub fn read(&self) -> Result<Arc<[u8]>, PError> {
        let failed = |detail: String| {
            PError::MalformedLibrary(format!("payload {}: {detail}", self.path.display()))
        };
        let data = std::fs::read(&self.path).map_err(|error| failed(error.to_string()))?;
        self.reads.fetch_add(1, Ordering::Relaxed);
        if data.len() as u64 != self.bytes {
            return Err(failed(format!(
                "{} bytes, the library records {}",
                data.len(),
                self.bytes
            )));
        }
        let data = match &self.decode {
            Some(decode) => decode(data)?,
            None => data,
        };
        let digest: [u8; 32] = blake3::hash(&data).into();
        match self.digest.get() {
            Some(first) if *first != digest => {
                return Err(failed("changed since its first read".to_owned()));
            }
            Some(_) => {}
            None => {
                if let Some(check) = &self.check {
                    check(&data)?;
                }
                let _ = self.digest.set(digest);
            }
        }
        Ok(Arc::from(data))
    }
}

/// What the budget did, for reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PProgramBudgetStats {
    pub limit_bytes: u64,
    pub resident_bytes: u64,
    pub peak_resident_bytes: u64,
    pub builds: u64,
    pub rebuilds: u64,
    pub evictions: u64,
    pub build_seconds_millis: u64,
}

/// The memory budget of the evictable programs of one library.
pub struct PProgramBudget {
    limit: u64,
    factor: u64,
    resident: AtomicU64,
    peak: AtomicU64,
    clock: AtomicU64,
    builds: AtomicU64,
    rebuilds: AtomicU64,
    evictions: AtomicU64,
    build_millis: AtomicU64,
    entries: Mutex<Vec<Weak<PEvictableStep>>>,
}

impl fmt::Debug for PProgramBudget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "PProgramBudget({:?})", self.stats())
    }
}

/// `RUSTRED_P_PROGRAM_BUDGET_GB` default of a library reaching 5 loops.
pub const P_PROGRAM_BUDGET_DEFAULT_GB_5L: f64 = 32.0;
/// `RUSTRED_P_PROGRAM_WEIGHT` default: resident program bytes per payload byte.
pub const P_PROGRAM_WEIGHT_DEFAULT: u64 = 4;

impl PProgramBudget {
    /// A budget of `limit` bytes; a program of a payload of `b` bytes weighs
    /// `factor * b`.
    pub fn new(limit: u64, factor: u64) -> Arc<Self> {
        Arc::new(Self {
            limit,
            factor: factor.max(1),
            resident: AtomicU64::new(0),
            peak: AtomicU64::new(0),
            clock: AtomicU64::new(0),
            builds: AtomicU64::new(0),
            rebuilds: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            build_millis: AtomicU64::new(0),
            entries: Mutex::new(Vec::new()),
        })
    }

    /// The budget of a library of `max_loops`: `RUSTRED_P_PROGRAM_BUDGET_GB`
    /// (`0` = none) or the default (32 GB at 5 loops, none below).
    pub fn from_env(max_loops: u8) -> Option<Arc<Self>> {
        let gigabytes = match std::env::var("RUSTRED_P_PROGRAM_BUDGET_GB") {
            Ok(text) => text.trim().parse::<f64>().ok()?,
            Err(_) if max_loops >= 5 => P_PROGRAM_BUDGET_DEFAULT_GB_5L,
            Err(_) => return None,
        };
        if gigabytes <= 0.0 {
            return None;
        }
        let factor = std::env::var("RUSTRED_P_PROGRAM_WEIGHT")
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok())
            .unwrap_or(P_PROGRAM_WEIGHT_DEFAULT);
        Some(Self::new((gigabytes * 1e9) as u64, factor))
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// The weight of the program of a payload of `bytes` bytes.
    pub fn weight(&self, bytes: u64) -> u64 {
        bytes.saturating_mul(self.factor)
    }

    pub fn stats(&self) -> PProgramBudgetStats {
        PProgramBudgetStats {
            limit_bytes: self.limit,
            resident_bytes: self.resident.load(Ordering::Relaxed),
            peak_resident_bytes: self.peak.load(Ordering::Relaxed),
            builds: self.builds.load(Ordering::Relaxed),
            rebuilds: self.rebuilds.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            build_seconds_millis: self.build_millis.load(Ordering::Relaxed),
        }
    }

    /// Drop every resident program that no step holds, whatever the limit,
    /// and return the weight freed. Called when a request's forward pass is
    /// over (p5l-library-3): the modular replay that follows never steps, so
    /// the programs would only hold memory beside its weights; a later
    /// request rebuilds what it uses.
    pub fn release_unheld(&self) -> u64 {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let steps: Vec<Arc<PEvictableStep>> = entries.iter().filter_map(Weak::upgrade).collect();
        drop(entries);
        let mut freed = 0;
        for step in steps {
            let Ok(mut slot) = step.slot.try_write() else {
                continue;
            };
            if slot
                .as_ref()
                .is_some_and(|program| Arc::strong_count(program) == 1)
            {
                *slot = None;
                drop(slot);
                self.resident.fetch_sub(step.weight, Ordering::Relaxed);
                self.evictions.fetch_add(1, Ordering::Relaxed);
                freed += step.weight;
            }
        }
        freed
    }

    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    fn register(&self, step: &Arc<PEvictableStep>) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|entry| entry.strong_count() > 0);
        entries.push(Arc::downgrade(step));
    }

    /// Drop the least recently used resident programs that no step holds
    /// (never `keep`) until the resident estimate fits the limit.
    fn enforce(&self, keep: &PEvictableStep) {
        if self.resident.load(Ordering::Relaxed) <= self.limit {
            return;
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut candidates: Vec<(u64, Arc<PEvictableStep>)> = entries
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|step| !std::ptr::eq(Arc::as_ptr(step), keep))
            .map(|step| (step.last.load(Ordering::Relaxed), step))
            .collect();
        drop(entries);
        candidates.sort_unstable_by_key(|(last, _)| *last);
        for (_, step) in candidates {
            if self.resident.load(Ordering::Relaxed) <= self.limit {
                break;
            }
            let Ok(mut slot) = step.slot.try_write() else {
                continue;
            };
            // In use: a step holds a clone of the program.
            if slot
                .as_ref()
                .is_some_and(|program| Arc::strong_count(program) == 1)
            {
                *slot = None;
                drop(slot);
                self.resident.fetch_sub(step.weight, Ordering::Relaxed);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

type PProgramBuild = Box<dyn Fn() -> Result<Arc<dyn PLaneStep>, PError> + Send + Sync>;

/// The program of an owner with a lazy payload, built on use and evictable
/// under its library's [`PProgramBudget`].
pub struct PEvictableStep {
    lane: OwnerLane,
    weight: u64,
    build: PProgramBuild,
    slot: RwLock<Option<Arc<dyn PLaneStep>>>,
    built: AtomicU64,
    last: AtomicU64,
    budget: Arc<PProgramBudget>,
}

impl fmt::Debug for PEvictableStep {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PEvictableStep")
            .field("lane", &self.lane)
            .field("weight", &self.weight)
            .field("builds", &self.built.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl PEvictableStep {
    /// The program of lane `lane`, built by `build`, weighing `weight` bytes
    /// against `budget`.
    pub fn new(
        lane: OwnerLane,
        weight: u64,
        budget: Arc<PProgramBudget>,
        build: PProgramBuild,
    ) -> Arc<Self> {
        let step = Arc::new(Self {
            lane,
            weight,
            build,
            slot: RwLock::new(None),
            built: AtomicU64::new(0),
            last: AtomicU64::new(0),
            budget,
        });
        step.budget.register(&step);
        step
    }

    /// How often the program was built.
    pub fn builds(&self) -> u64 {
        self.built.load(Ordering::Relaxed)
    }

    /// The program, built (again) if it is not resident.
    pub fn program(&self) -> Result<Arc<dyn PLaneStep>, PError> {
        self.last.store(self.budget.tick(), Ordering::Relaxed);
        if let Some(program) = self.slot.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
            return Ok(Arc::clone(program));
        }
        let program = {
            let mut slot = self.slot.write().unwrap_or_else(|e| e.into_inner());
            if let Some(program) = slot.as_ref() {
                return Ok(Arc::clone(program));
            }
            let started = std::time::Instant::now();
            let program = (self.build)()?;
            if program.lane() != self.lane {
                return Err(PError::MalformedLibrary(format!(
                    "an evictable program of lane {:?} built {:?}",
                    self.lane,
                    program.lane()
                )));
            }
            *slot = Some(Arc::clone(&program));
            let budget = &self.budget;
            budget
                .build_millis
                .fetch_add(started.elapsed().as_millis() as u64, Ordering::Relaxed);
            if self.built.fetch_add(1, Ordering::Relaxed) > 0 {
                budget.rebuilds.fetch_add(1, Ordering::Relaxed);
            }
            budget.builds.fetch_add(1, Ordering::Relaxed);
            let resident = budget.resident.fetch_add(self.weight, Ordering::Relaxed) + self.weight;
            budget.peak.fetch_max(resident, Ordering::Relaxed);
            program
        };
        self.budget.enforce(self);
        Ok(program)
    }
}

impl PLaneStep for PEvictableStep {
    fn lane(&self) -> OwnerLane {
        self.lane
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        self.program()?.step(cx, key)
    }

    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        self.program()?.order_key(cx, powers)
    }

    /// vk5l WP1: a lazy boxed owner keeps its box (built on use).
    fn box_contains(&self, powers: &[i64]) -> Option<bool> {
        self.program().ok()?.box_contains(powers)
    }

    fn boxed_record(&self) -> Option<Arc<super::table_import::BoxedNodeRecord>> {
        self.program().ok()?.boxed_record()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Fake;

    impl PLaneStep for Fake {
        fn lane(&self) -> OwnerLane {
            OwnerLane::Custom
        }

        fn step(&self, _: &PStepContext<'_>, _: &PKeyView<'_>) -> Result<PStepOut, PError> {
            Ok(PStepOut::default())
        }
    }

    fn fake(budget: &Arc<PProgramBudget>, weight: u64) -> Arc<PEvictableStep> {
        PEvictableStep::new(
            OwnerLane::Custom,
            weight,
            Arc::clone(budget),
            Box::new(|| Ok(Arc::new(Fake) as Arc<dyn PLaneStep>)),
        )
    }

    #[test]
    fn the_coldest_unused_program_is_evicted_and_rebuilt_on_use() {
        let budget = PProgramBudget::new(25, 1);
        let a = fake(&budget, 10);
        let b = fake(&budget, 10);
        let c = fake(&budget, 10);
        a.program().unwrap();
        b.program().unwrap();
        // In use: c's build may not evict a held program.
        let held = a.program().unwrap();
        c.program().unwrap();
        let stats = budget.stats();
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.resident_bytes, 20);
        assert_eq!(b.builds(), 1);
        drop(held);
        b.program().unwrap();
        assert_eq!(b.builds(), 2);
        assert_eq!(budget.stats().rebuilds, 1);
        assert!(budget.stats().resident_bytes <= 25);
    }

    #[test]
    fn a_payload_file_that_changes_after_its_first_read_is_refused() {
        let dir = std::env::temp_dir().join(format!("rr-payload-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.rrbin");
        std::fs::write(&path, b"abcd").unwrap();
        let file = PPayloadFile::new(path.clone(), 4, None);
        assert_eq!(&*file.read().unwrap(), b"abcd");
        std::fs::write(&path, b"abce").unwrap();
        assert!(file.read().is_err());
        std::fs::write(&path, b"abc").unwrap();
        assert!(file.read().is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
