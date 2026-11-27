//! vk5l-physrun level 2: resumable record pass. With `RUSTRED_RESUME_DIR`
//! (and a modular arithmetic that spills its trace), the scheduler state is
//! saved at phase boundaries (at most every `RUSTRED_RESUME_SECONDS`,
//! default 1800) into `<dir>/<request fingerprint>/state.bin`, beside the
//! spilled ops (`ops.bin`). A later request with the same fingerprint
//! restores it and continues with the next phase; the spilled ops are cut
//! back to the saved count. `RUSTRED_RESUME_STOP_AFTER_PHASE=<n>` stops a
//! request after saving at phase n (tests). The sequential and the pipelined
//! scheduler (several threads) record the same trace and write the same state
//! file (pending keys in compact-form order), so a state saved at one thread
//! count resumes at any other.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::OrderedKernel;
use crate::algebra::Coefficient;

pub(crate) const RESUME_VERSION: &str = "rustred.resume-state.v1";
/// The leading bytes of a state file written since the trace digest (format
/// v2: a BLAKE3 digest of the trace prefix follows the op count). The request
/// fingerprint keeps using [`RESUME_VERSION`], so states of both formats are
/// found in the same request directory; v1 states are read with a length
/// check of the trace only (a warning says so).
/// 12d2 state format: magic, phase, recorder section (op count, trace
/// digest, ...), scheduler sections, BLAKE3 of the program identity, and last
/// the BLAKE3 of every preceding byte of the file (the state's own digest).
pub(crate) const STATE_FORMAT: &str = "rustred.resume-state.v3";

pub(crate) fn dir() -> Option<PathBuf> {
    std::env::var_os("RUSTRED_RESUME_DIR").map(PathBuf::from)
}

pub(crate) fn interval() -> f64 {
    std::env::var("RUSTRED_RESUME_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800.0)
}

static CALLER_COMMIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PENDING_COMMIT: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn set_caller_commit(on: bool) {
    CALLER_COMMIT.store(on, std::sync::atomic::Ordering::Relaxed);
}

fn caller_commit() -> bool {
    match std::env::var("RUSTRED_RESUME_COMMIT").as_deref() {
        Ok("caller") => true,
        Ok("engine") => false,
        _ => CALLER_COMMIT.load(std::sync::atomic::Ordering::Relaxed),
    }
}

fn remove_request(dir: &Path) {
    let _ = std::fs::remove_file(dir.join("state.bin"));
    let _ = std::fs::remove_file(dir.join("ops.bin"));
    // the resumable sampled replay's point store of this request
    let _ = std::fs::remove_file(dir.join("samples.jsonl"));
    let _ = std::fs::remove_dir(dir);
}

/// The result of the request in `dir` exists: remove its state and trace, or
/// keep them for [`commit`] (caller-commit mode).
pub(crate) fn result_ready(dir: PathBuf) {
    if caller_commit() {
        eprintln!(
            "rustred resume: result ready; state and trace in {} kept until the caller commits its output",
            dir.display()
        );
        PENDING_COMMIT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(dir);
    } else {
        remove_request(&dir);
    }
}

pub(crate) fn commit() -> usize {
    let pending = std::mem::take(
        &mut *PENDING_COMMIT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    for dir in &pending {
        remove_request(dir);
    }
    pending.len()
}

/// A save inside a phase is due: every `RUSTRED_RESUME_INPHASE_SECONDS`
/// (default: the phase-end interval, `RUSTRED_RESUME_SECONDS`), checked every
/// 4096 keys; `RUSTRED_RESUME_INPHASE_KEYS=<n>` (tests): every n keys instead.
/// `RUSTRED_RESUME_INPHASE=0` disables saves inside phases.
pub(crate) fn inphase_due(keys: usize, last_save: &std::time::Instant) -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    static KEYS: OnceLock<Option<usize>> = OnceLock::new();
    static SECONDS: OnceLock<f64> = OnceLock::new();
    if !*ON.get_or_init(|| std::env::var("RUSTRED_RESUME_INPHASE").as_deref() != Ok("0")) {
        return false;
    }
    if let Some(every) = *KEYS.get_or_init(|| {
        std::env::var("RUSTRED_RESUME_INPHASE_KEYS")
            .ok()
            .and_then(|v| v.parse().ok())
    }) {
        return every > 0 && keys.is_multiple_of(every);
    }
    keys.is_multiple_of(4096)
        && last_save.elapsed().as_secs_f64()
            >= *SECONDS.get_or_init(|| {
                std::env::var("RUSTRED_RESUME_INPHASE_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(interval)
            })
}

/// The resume directory's filesystem has less than `RUSTRED_RESUME_DISK_MIN_GB`
/// (default 50) GB available, or the host's disk guard raised
/// `RUSTRED_DISK_LOW_FLAG` (a file path; set by the lane's environment).
pub(crate) fn disk_low(dir: &Path) -> bool {
    use std::sync::OnceLock;
    static MIN: OnceLock<u64> = OnceLock::new();
    let min_gb = *MIN.get_or_init(|| {
        std::env::var("RUSTRED_RESUME_DISK_MIN_GB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50)
    });
    if std::env::var_os("RUSTRED_DISK_LOW_FLAG").is_some_and(|flag| Path::new(&flag).exists()) {
        return true;
    }
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: statvfs on a NUL-terminated path into a zeroed struct.
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return false;
    }
    let available = stat.f_bavail as u64 * stat.f_frsize as u64;
    available < min_gb.saturating_mul(1 << 30)
}

/// Test hook: stop right after the first save inside a phase.
pub(crate) fn stop_inphase() -> bool {
    std::env::var("RUSTRED_RESUME_STOP_INPHASE").as_deref() == Ok("1")
}

pub(crate) fn stop_after_phase() -> Option<usize> {
    std::env::var("RUSTRED_RESUME_STOP_AFTER_PHASE")
        .ok()
        .and_then(|v| v.parse().ok())
}

/// Write a coefficient (numerator, denominator) with the step-cache codec.
pub(crate) fn put_coefficient(w: &mut super::step_cache::codec::Writer, c: &Coefficient) {
    w.polynomial(&c.numerator);
    w.polynomial(&c.denominator);
}

pub(crate) fn get_coefficient(
    r: &mut super::step_cache::codec::Reader<'_>,
    template: &Coefficient,
) -> Result<Coefficient, String> {
    let numerator = r.polynomial(&template.numerator)?;
    let denominator = r.polynomial(&template.denominator)?;
    Ok(Coefficient {
        numerator,
        denominator,
    })
}

/// The directory of one request: blake3 of the version, the encoded input
/// keys with their coefficients and the conditions. `None` when a key has
/// no compact encoding (the request is then not resumable).
pub(crate) fn request_dir<K: OrderedKernel>(
    root: &Path,
    initial: &BTreeMap<K::Key, Coefficient>,
    conditions: &[Coefficient],
    selection: &[u8],
) -> Option<PathBuf> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RESUME_VERSION.as_bytes());
    let mut bytes = Vec::new();
    for (key, value) in initial {
        bytes.clear();
        if !K::tail_encode(key, &mut bytes) {
            return None;
        }
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        hasher.update(value.to_string().as_bytes());
    }
    for c in conditions {
        hasher.update(c.to_string().as_bytes());
    }
    // 12d2: a target selection (empty for a full run: the directory of a full
    // run is unchanged, so earlier full-run states stay reachable).
    if !selection.is_empty() {
        hasher.update(b"rustred.resume-selection");
        hasher.update(&(selection.len() as u64).to_le_bytes());
        hasher.update(selection);
    }
    Some(root.join(hasher.finalize().to_hex().to_string()))
}

/// eng-mem: the staged bytes of a [`StateWriter`] are moved to the file
/// once they reach this size.
const STATE_DRAIN: usize = 1 << 20;

/// eng-mem: a state file written as it is produced (no whole-state buffer):
/// the codec primitives write into a small staging [`Writer`] (`Deref`),
/// which [`Self::drain`] moves into the file and into the BLAKE3 of the
/// file's bytes (the state's own digest, appended by [`write_state`]). The
/// first I/O error is kept and returned by [`write_state`]; later bytes
/// are dropped.
pub(crate) struct StateWriter {
    staged: super::step_cache::codec::Writer,
    file: std::fs::File,
    hasher: blake3::Hasher,
    written: u64,
    error: Option<std::io::Error>,
}

impl StateWriter {
    fn new(file: std::fs::File) -> Self {
        Self {
            staged: super::step_cache::codec::Writer {
                bytes: Vec::with_capacity(STATE_DRAIN + (STATE_DRAIN >> 2)),
            },
            file,
            hasher: blake3::Hasher::new(),
            written: 0,
            error: None,
        }
    }

    /// Move the staged bytes to the file when they reach [`STATE_DRAIN`].
    #[inline]
    pub(crate) fn drain(&mut self) {
        if self.staged.bytes.len() >= STATE_DRAIN {
            self.flush_staged();
        }
    }

    fn flush_staged(&mut self) {
        use std::io::Write;
        let bytes = &self.staged.bytes;
        self.hasher.update(bytes);
        if self.error.is_none()
            && let Err(error) = self.file.write_all(bytes)
        {
            self.error = Some(error);
        }
        self.written += bytes.len() as u64;
        self.staged.bytes.clear();
    }

    /// The state's own digest appended (BLAKE3 of every byte before it),
    /// everything written and the file synced: its size.
    fn finish(mut self) -> std::io::Result<u64> {
        self.flush_staged();
        let own = self.hasher.finalize();
        self.staged.bytes.extend_from_slice(own.as_bytes());
        self.flush_staged();
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        self.file.sync_all()?;
        Ok(self.written)
    }
}

impl std::ops::Deref for StateWriter {
    type Target = super::step_cache::codec::Writer;
    fn deref(&self) -> &Self::Target {
        &self.staged
    }
}

impl std::ops::DerefMut for StateWriter {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.staged
    }
}

/// eng-mem: why a streamed state was not written.
pub(crate) enum StateFailure<E> {
    /// The content could not be produced (`body`'s hard error).
    Content(E),
    /// Creating, writing, syncing or publishing the file failed.
    Write(std::io::Error),
}

/// eng-mem: the state that `body` writes, durably at `path` (R-B2), as a
/// whole-state buffer was before (`write_atomic`): streamed into
/// `<path>.tmp` with the state's own digest last, fsynced, renamed over
/// `path`, and the directory fsynced. On any failure, and when `body`
/// declines (`Ok(Err(reason))`: no state), the temporary file is removed and
/// the previous `path` is untouched, so a partial write is never published.
/// Returns the size of the published state.
pub(crate) fn write_state<E>(
    path: &Path,
    body: impl FnOnce(&mut StateWriter) -> Result<Result<(), String>, E>,
) -> Result<Result<u64, String>, StateFailure<E>> {
    let tmp = path.with_extension("tmp");
    let file = std::fs::File::create(&tmp).map_err(StateFailure::Write)?;
    let mut w = StateWriter::new(file);
    let produced = body(&mut w);
    let written = match produced {
        Ok(Ok(())) => w.finish(),
        Ok(Err(reason)) => {
            drop(w);
            let _ = std::fs::remove_file(&tmp);
            return Ok(Err(reason));
        }
        Err(error) => {
            drop(w);
            let _ = std::fs::remove_file(&tmp);
            return Err(StateFailure::Content(error));
        }
    };
    let publish = written.and_then(|size| {
        std::fs::rename(&tmp, path)?;
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(size)
    });
    match publish {
        Ok(size) => Ok(Ok(size)),
        Err(error) => {
            let _ = std::fs::remove_file(&tmp);
            Err(StateFailure::Write(error))
        }
    }
}

static STORAGE_STOP: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// R-B2: a request stopped for a storage reason (a low-disk pause, a failed
/// checkpoint or trace / dictionary spill write or sync, a refused resume, a
/// test stop): its error is not a result. Recorded (first message kept) for
/// batch callers ([`take_storage_stop`]), which must leave the request
/// retryable instead of recording the error as its output.
pub(crate) fn storage_stop(message: &str) {
    let mut stop = STORAGE_STOP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.is_none() {
        *stop = Some(message.to_owned());
    }
}

/// Tests that raise or read the process-wide storage stop: one at a time.
#[cfg(test)]
pub(crate) static STORAGE_STOP_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// R-B2: the storage stop recorded since the last call, cleared.
pub(crate) fn take_storage_stop() -> Option<String> {
    STORAGE_STOP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::reduction::ordered_reduction::step_cache::codec::{Reader, Writer};

    /// A coefficient of the resume state comes back equal.
    #[test]
    fn resume_coefficients_round_trip() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let d = context.parameter("d").unwrap();
        let limits = crate::algebra::ExactAlgebraLimits::default();
        let value = context
            .try_div(
                &context.try_add(&d, &context.integer(3), limits).unwrap(),
                &context.try_mul(&d, &d, limits).unwrap(),
                limits,
            )
            .unwrap();
        let mut w = Writer { bytes: Vec::new() };
        put_coefficient(&mut w, &value);
        put_coefficient(&mut w, &context.integer(-7));
        let mut r = Reader::new(&w.bytes);
        let template = context.zero();
        assert_eq!(get_coefficient(&mut r, &template).unwrap(), value);
        assert_eq!(
            get_coefficient(&mut r, &template).unwrap(),
            context.integer(-7)
        );
        assert!(r.is_empty());
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rustred-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// eng-mem: a streamed state is published only whole: its bytes are
    /// what the body wrote followed by their BLAKE3; a body that declines or
    /// fails leaves the previous state untouched and no temporary file.
    #[test]
    fn a_streamed_state_is_published_whole_or_not_at_all() {
        let dir = scratch_dir("state-writer-test");
        let path = dir.join("state.bin");
        let size = write_state::<()>(&path, |w| {
            for i in 0..(3 * STATE_DRAIN as u64 / 2) {
                w.varint(i);
                w.drain();
            }
            Ok(Ok(()))
        });
        let file = std::fs::read(&path).unwrap();
        assert!(matches!(size, Ok(Ok(n)) if n == file.len() as u64));
        let mut reference = Writer::default();
        for i in 0..(3 * STATE_DRAIN as u64 / 2) {
            reference.varint(i);
        }
        let body = file.len() - 32;
        assert_eq!(file[..body], reference.bytes[..]);
        assert_eq!(file[body..], *blake3::hash(&reference.bytes).as_bytes());
        assert!(!dir.join("state.tmp").exists());
        let declined = write_state::<()>(&path, |w| {
            w.raw(b"half a state");
            Ok(Err("no compact form".to_owned()))
        });
        assert!(matches!(declined, Ok(Err(reason)) if reason == "no compact form"));
        let failed = write_state::<&str>(&path, |w| {
            w.raw(&[7; 3 * STATE_DRAIN]);
            w.drain();
            Err("trace sync")
        });
        assert!(matches!(failed, Err(StateFailure::Content("trace sync"))));
        assert_eq!(std::fs::read(&path).unwrap(), file);
        assert!(!dir.join("state.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

}
