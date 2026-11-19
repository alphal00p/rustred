//! vk5l-physrun level 2: the trace ops of a recorded request spilled to a
//! file as they are recorded (append-only, 8 B per op), and read back by
//! mmap for the replay, so that the forward pass and the replay hold no
//! in-memory copy of the trace. `RUSTRED_TRACE_SPILL_DIR=<dir>` enables it
//! (recorded requests on the sequential or pipelined scheduler, without
//! tail rewrites or layered mode);
//! the directory must be off snapshotted file systems (e.g. /common).
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use super::{Op, error};
use crate::reduction::ReductionError;

/// Ops flushed per block (64 MB; `RUSTRED_TRACE_SPILL_BLOCK_OPS` for tests).
pub(super) fn block_ops() -> usize {
    static BLOCK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BLOCK.get_or_init(|| {
        std::env::var("RUSTRED_TRACE_SPILL_BLOCK_OPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(8 << 20)
    })
}

pub(super) fn spill_dir() -> Option<PathBuf> {
    std::env::var_os("RUSTRED_TRACE_SPILL_DIR").map(PathBuf::from)
}

/// The writer of one request's spill file, created at the first flush and
/// removed when dropped unflushed or when its mapping is dropped.
pub(super) struct Spill {
    dir: PathBuf,
    file: Option<(File, PathBuf)>,
    pub flushed: usize,
    /// Resume mode: the file is the request's `ops.bin` and is kept.
    keep: bool,
    /// Resume mode: BLAKE3 of every byte written to `ops.bin` so far (the
    /// prefix a saved state covers), updated at every flush.
    hasher: Option<blake3::Hasher>,
    /// R-B2: the first write or sync error (with the file): every later
    /// flush, sync or finish refuses with it, so a short write can never be
    /// followed by a retried append (a duplicated prefix) that a later state
    /// would publish.
    failed: Option<String>,
}

// R-B2 test hooks: the next flush writes only this many bytes and fails
// (a short write); the next sync fails.
#[cfg(test)]
thread_local! {
    static SHORT_WRITE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static FAIL_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn write_ops(file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(n) = SHORT_WRITE.with(std::cell::Cell::take) {
        file.write_all(&bytes[..n.min(bytes.len())])?;
        return Err(std::io::Error::from_raw_os_error(libc::EFBIG));
    }
    file.write_all(bytes)
}

fn sync_ops(file: &mut File) -> std::io::Result<()> {
    file.flush()?;
    #[cfg(test)]
    if FAIL_SYNC.with(|f| f.replace(false)) {
        return Err(std::io::Error::from_raw_os_error(libc::ENOSPC));
    }
    file.sync_data()
}

/// Bytes of `ops` ops in the spill file.
fn op_bytes(ops: u64) -> u64 {
    ops * std::mem::size_of::<Op>() as u64
}

impl Spill {
    pub(super) fn create(dir: PathBuf) -> Result<Self, ReductionError> {
        Ok(Self {
            dir,
            file: None,
            flushed: 0,
            keep: false,
            hasher: None,
            failed: None,
        })
    }

    /// Resume mode: `<dir>/ops.bin`. Fresh (`resume` None): created empty.
    /// Continuing a saved state (`resume` = (ops, digest)): the file must
    /// EXIST and hold at least `ops` ops (a hard error names both lengths: it
    /// is never created or extended, so a missing or short trace can never be
    /// replayed as zero ops); the digest of its first `ops` ops is computed in
    /// one streaming pass and must equal the state's (states of format v1
    /// carry none: length check only, with a warning); then the file is cut
    /// back to exactly `ops` ops (the ops recorded after that save) and the
    /// digest continues from the verified prefix. kept on drop.
    pub(super) fn at(
        dir: &std::path::Path,
        resume: Option<(u64, Option<[u8; 32]>)>,
    ) -> Result<Self, ReductionError> {
        use std::io::{Read, Seek};
        std::fs::create_dir_all(dir).map_err(|e| error(format!("resume directory: {e}")))?;
        let path = dir.join("ops.bin");
        let Some((ops, digest)) = resume else {
            // 12d2: a trace without a saved state is never truncated silently
            // (it may be a finished record whose state was lost, or another
            // run's): refused unless RUSTRED_RESUME_DISCARD_TRACE=1.
            if let Ok(meta) = std::fs::metadata(&path)
                && meta.len() > 0
                && std::env::var("RUSTRED_RESUME_DISCARD_TRACE").as_deref() != Ok("1")
            {
                return Err(error(format!(
                    "resume refused: {} holds a trace of {} bytes but there is no saved state; it is kept \
                     (remove it, or set RUSTRED_RESUME_DISCARD_TRACE=1, to start this request from zero)",
                    path.display(),
                    meta.len()
                )));
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| error(format!("resume ops file: {e}")))?;
            return Ok(Self {
                dir: dir.to_path_buf(),
                file: Some((file, path)),
                flushed: 0,
                keep: true,
                hasher: Some(blake3::Hasher::new()),
                failed: None,
            });
        };
        let needed = op_bytes(ops);
        let have = match std::fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(e) => {
                return Err(error(format!(
                    "resume refused: the trace file {} cannot be read ({e}); the saved state needs {needed} bytes \
                     ({ops} ops)",
                    path.display()
                )));
            }
        };
        if have < needed {
            return Err(error(format!(
                "resume refused: the trace file {} holds {have} bytes ({} ops), the saved state needs {needed} bytes \
                 ({ops} ops)",
                path.display(),
                have / std::mem::size_of::<Op>() as u64
            )));
        }
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| error(format!("resume ops file: {e}")))?;
        // the digest of the prefix the state covers: one streaming pass
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0u8; 16 << 20];
        let mut left = needed;
        while left > 0 {
            let n = (left as usize).min(buffer.len());
            file.read_exact(&mut buffer[..n])
                .map_err(|e| error(format!("resume ops read: {e}")))?;
            hasher.update(&buffer[..n]);
            left -= n as u64;
        }
        match digest {
            Some(want) => {
                let got = hasher.finalize();
                if got.as_bytes() != &want {
                    return Err(error(format!(
                        "resume refused: the first {ops} ops of {} do not match the saved state (trace digest {} \
                         expected, {} found)",
                        path.display(),
                        blake3::Hash::from(want).to_hex(),
                        got.to_hex()
                    )));
                }
            }
            None => eprintln!(
                "rustred resume: WARNING: state without a trace digest (format v1): only the length of {} was \
                 checked ({have} bytes >= {needed}); its content is NOT verified",
                path.display()
            ),
        }
        file.set_len(needed)
            .map_err(|e| error(format!("resume ops cut: {e}")))?;
        file.seek(std::io::SeekFrom::End(0))
            .map_err(|e| error(format!("resume ops seek: {e}")))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            file: Some((file, path)),
            flushed: ops as usize,
            keep: true,
            hasher: Some(hasher),
            failed: None,
        })
    }

    /// Resume mode: the digest of every op written so far.
    pub(super) fn digest(&self) -> Option<[u8; 32]> {
        self.hasher.as_ref().map(|h| *h.finalize().as_bytes())
    }

    /// R-B2: refuse with the first earlier write or sync error.
    fn check_failed(&self) -> Result<(), ReductionError> {
        match &self.failed {
            Some(message) => Err(error(format!(
                "{message} (earlier error; the trace is not appended to again)"
            ))),
            None => Ok(()),
        }
    }

    /// R-B2: record a write or sync error on the spill file and return it
    /// (marked as a storage stop for batch callers).
    fn fail(&mut self, what: &str, e: &std::io::Error) -> ReductionError {
        let path = self
            .file
            .as_ref()
            .map_or_else(|| self.dir.clone(), |(_, path)| path.clone());
        let message = format!("trace spill {what} of {} failed: {e}", path.display());
        // a kept trace that no saved state covers yet is emptied (best
        // effort), so that a restart starts the request afresh instead of
        // refusing a trace without a state
        if self.keep
            && !self.dir.join("state.bin").exists()
            && let Some((file, _)) = &self.file
        {
            let _ = file.set_len(0);
        }
        super::super::resume::storage_stop(&message);
        self.failed = Some(message.clone());
        error(message)
    }

    /// Append `ops` and clear them. A failed (or short) write cuts the file
    /// back to the ops flushed before it (best effort) and fails this spill
    /// for good: nothing is appended after a partial write.
    pub(super) fn flush(&mut self, ops: &mut Vec<Op>) -> Result<(), ReductionError> {
        self.check_failed()?;
        if ops.is_empty() {
            return Ok(());
        }
        if self.file.is_none() {
            std::fs::create_dir_all(&self.dir)
                .map_err(|e| error(format!("trace spill directory: {e}")))?;
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = self.dir.join(format!("ops-{}-{n}.bin", std::process::id()));
            let file = File::create(&path).map_err(|e| error(format!("trace spill file: {e}")))?;
            self.file = Some((file, path));
        }
        let (file, _) = self.file.as_mut().expect("spill file");
        // SAFETY: Op is repr(C) of two u32 (8 bytes, no padding).
        let bytes = unsafe {
            std::slice::from_raw_parts(
                ops.as_ptr().cast::<u8>(),
                ops.len() * std::mem::size_of::<Op>(),
            )
        };
        if let Err(e) = write_ops(file, bytes) {
            use std::io::Seek;
            let before = op_bytes(self.flushed as u64);
            let _ = file
                .set_len(before)
                .and_then(|()| file.seek(std::io::SeekFrom::Start(before)));
            return Err(self.fail("write", &e));
        }
        if let Some(hasher) = &mut self.hasher {
            hasher.update(bytes);
        }
        self.flushed += ops.len();
        ops.clear();
        Ok(())
    }

    /// 12d2: the request directory when this is a kept (resume) trace.
    pub(super) fn kept_dir(&self) -> Option<PathBuf> {
        self.keep.then(|| self.dir.clone())
    }

    /// Make the written ops durable (resume checkpoints).
    /// A failed sync fails this spill for good (the page cache state after a
    /// failed fdatasync is unknown: never retried).
    pub(super) fn sync(&mut self) -> Result<(), ReductionError> {
        self.check_failed()?;
        if let Some((file, _)) = &mut self.file
            && let Err(e) = sync_ops(file)
        {
            return Err(self.fail("sync", &e));
        }
        Ok(())
    }

    /// The whole trace: the rest appended and the file mapped (or the rest
    /// itself when nothing was ever flushed).
    pub(super) fn finish(mut self, rest: &mut Vec<Op>) -> Result<OpStore, ReductionError> {
        self.check_failed()?;
        if self.file.is_none() || (self.keep && self.flushed == 0 && rest.len() < block_ops()) {
            // a kept (resume) file stays: its request removes it with the state
            if let Some((_, path)) = self.file.take()
                && !self.keep
            {
                let _ = std::fs::remove_file(path);
            }
            return Ok(OpStore::from(std::mem::take(rest)));
        }
        self.flush(rest)?;
        *rest = Vec::new();
        let (mut file, path) = self.file.take().expect("spill file");
        file.flush()
            .map_err(|e| error(format!("trace spill flush: {e}")))?;
        drop(file);
        OpStore::map(path, self.flushed, self.keep)
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if let Some((_, path)) = self.file.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The ops of a trace: in memory, or a read-only mapping of a spill file.
pub(super) enum OpStore {
    Memory(Vec<Op>),
    Mapped {
        ptr: *const Op,
        len: usize,
        path: PathBuf,
        /// 12d2: a resume trace, removed by its request once the result
        /// exists (never by the replay that maps it).
        keep: bool,
    },
}

// SAFETY: the mapping is read-only and lives as long as the store.
unsafe impl Send for OpStore {}
unsafe impl Sync for OpStore {}

impl OpStore {
    fn map(path: PathBuf, len: usize, keep: bool) -> Result<Self, ReductionError> {
        if len == 0 {
            if !keep {
                let _ = std::fs::remove_file(&path);
            }
            return Ok(Self::Memory(Vec::new()));
        }
        let file = File::open(&path).map_err(|e| error(format!("trace spill open: {e}")))?;
        let bytes = len * std::mem::size_of::<Op>();
        use std::os::fd::AsRawFd;
        // SAFETY: a fresh read-only private mapping of a file we wrote.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                bytes,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(error("trace spill mmap failed"));
        }
        unsafe { libc::madvise(ptr, bytes, libc::MADV_SEQUENTIAL) };
        Ok(Self::Mapped {
            ptr: ptr.cast::<Op>(),
            len,
            path,
            keep,
        })
    }
}

impl Drop for OpStore {
    fn drop(&mut self) {
        if let Self::Mapped {
            ptr,
            len,
            path,
            keep,
        } = self
        {
            unsafe { libc::munmap((*ptr).cast_mut().cast(), *len * std::mem::size_of::<Op>()) };
            if !*keep {
                let _ = std::fs::remove_file(&*path);
            }
        }
    }
}

impl OpStore {
    /// batch 12t: mutable ops (the t-adic replay re-points the recorded input
    /// ops per weight vector); a mapped store is first copied into memory
    /// (the mapping and its spill file are released).
    pub(super) fn to_mut(&mut self) -> &mut Vec<Op> {
        if let Self::Mapped { .. } = self {
            let ops: Vec<Op> = std::ops::Deref::deref(self).to_vec();
            *self = Self::Memory(ops);
        }
        match self {
            Self::Memory(ops) => ops,
            Self::Mapped { .. } => unreachable!("copied into memory above"),
        }
    }
}

impl std::ops::Deref for OpStore {
    type Target = [Op];
    fn deref(&self) -> &[Op] {
        match self {
            Self::Memory(ops) => ops,
            // SAFETY: `len` ops are mapped at `ptr` for the life of `self`.
            Self::Mapped { ptr, len, .. } => unsafe { std::slice::from_raw_parts(*ptr, *len) },
        }
    }
}

impl<'a> IntoIterator for &'a OpStore {
    type Item = &'a Op;
    type IntoIter = std::slice::Iter<'a, Op>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl From<Vec<Op>> for OpStore {
    fn from(ops: Vec<Op>) -> Self {
        Self::Memory(ops)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CoefRef, Op, OpTag};
    use super::*;

    /// Ops flushed in blocks and finished come back from the mapping in order;
    /// the file is removed with the mapping.
    #[test]
    fn spilled_ops_map_back_in_order_and_leave_no_file() {
        let dir = std::env::temp_dir().join(format!("rustred-spill-test-{}", std::process::id()));
        let mut spill = Spill::create(dir.clone()).unwrap();
        let all: Vec<Op> = (0..1000u32)
            .map(|i| Op::new(OpTag::Add, i % 7, CoefRef(i)))
            .collect();
        let mut buffer = all[..600].to_vec();
        spill.flush(&mut buffer).unwrap();
        assert!(buffer.is_empty());
        let mut rest = all[600..].to_vec();
        let store = spill.finish(&mut rest).unwrap();
        assert!(matches!(store, OpStore::Mapped { .. }));
        assert_eq!(store.len(), all.len());
        for (a, b) in store.iter().zip(&all) {
            assert_eq!(a.tag(), b.tag());
            assert_eq!(a.coefficient, b.coefficient);
        }
        drop(store);
        assert!(std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0);
        let _ = std::fs::remove_dir(&dir);
    }

    /// Resume trace binding (correctness fix): a continued state needs its trace file to EXIST, to hold at least the
    /// saved ops and (format v2) to match the saved digest; missing, short by one op, short by a block and one flipped
    /// byte are hard errors (never a created or zero-extended file); an intact or longer file continues, cut back to
    /// the saved ops, with the digest continuing from the verified prefix.
    #[test]
    fn a_resumed_trace_must_exist_hold_the_saved_ops_and_match_its_digest() {
        let root =
            std::env::temp_dir().join(format!("rustred-resume-bind-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ops = |from: u32, n: u32| -> Vec<Op> {
            (from..from + n)
                .map(|i| Op::new(OpTag::Add, i % 5, CoefRef(i)))
                .collect()
        };
        // a fresh run: 3 flushes of 10 ops, digest at the save
        let mut spill = Spill::at(&root, None).unwrap();
        for k in 0..3 {
            let mut buffer = ops(10 * k, 10);
            spill.flush(&mut buffer).unwrap();
        }
        spill.sync().unwrap();
        let (saved, digest) = (spill.flushed as u64, spill.digest().unwrap());
        // ops recorded after the save (lost on resume: cut back)
        let mut buffer = ops(30, 7);
        spill.flush(&mut buffer).unwrap();
        drop(spill);
        let path = root.join("ops.bin");
        let original = std::fs::read(&path).unwrap();
        assert_eq!(original.len() as u64, (saved + 7) * 8);
        let restore = |bytes: Option<&[u8]>| {
            match bytes {
                Some(b) => std::fs::write(&path, b).unwrap(),
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
            Spill::at(&root, Some((saved, Some(digest))))
        };
        let refused = |r: Result<Spill, ReductionError>, what: &str| {
            let e = r
                .err()
                .unwrap_or_else(|| panic!("{what}: accepted"))
                .to_string();
            assert!(e.contains("resume refused"), "{what}: {e}");
            e
        };
        let e = refused(restore(None), "missing");
        assert!(e.contains("240 bytes") && e.contains("30 ops"), "{e}");
        assert!(!path.exists(), "a missing trace is never created");
        let e = refused(
            restore(Some(&original[..(saved as usize - 1) * 8])),
            "short by one op",
        );
        assert!(
            e.contains("holds 232 bytes") && e.contains("needs 240 bytes"),
            "{e}"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            232,
            "a short trace is never extended"
        );
        refused(
            restore(Some(&original[..(saved as usize - 10) * 8])),
            "short by a block",
        );
        let mut flipped = original.clone();
        flipped[17] ^= 0x01;
        let e = refused(restore(Some(&flipped)), "one flipped byte");
        assert!(e.contains("do not match"), "{e}");
        // intact (with the 7 ops after the save): continues, cut back to the saved ops, digest continues
        let mut spill = restore(Some(&original)).unwrap();
        assert_eq!(spill.flushed as u64, saved);
        assert_eq!(spill.digest().unwrap(), digest);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), saved * 8);
        let mut buffer = ops(100, 4);
        spill.flush(&mut buffer).unwrap();
        let whole = std::fs::read(&path).unwrap();
        assert_eq!(spill.digest().unwrap(), *blake3::hash(&whole).as_bytes());
        drop(spill);
        // a v1 state (no digest): the length check only
        std::fs::write(&path, &flipped).unwrap();
        assert!(
            Spill::at(&root, Some((saved, None))).is_ok(),
            "v1: length only"
        );
        std::fs::write(&path, &original[..8]).unwrap();
        refused(Spill::at(&root, Some((saved, None))), "v1 short");
        // 12d2: a trace without a saved state (a finished record whose state is gone) is never truncated by a
        // fresh launch; an empty one is reused
        std::fs::write(&path, &original).unwrap();
        let e = refused(Spill::at(&root, None), "trace without a state");
        assert!(
            e.contains("no saved state") && e.contains("RUSTRED_RESUME_DISCARD_TRACE"),
            "{e}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "the trace is untouched"
        );
        std::fs::write(&path, b"").unwrap();
        assert!(
            Spill::at(&root, None).is_ok(),
            "an empty trace file is reused"
        );
        // 12d2: a kept (resume) trace survives its mapping and the replay that reads it
        let mut spill = Spill::at(&root, None).unwrap();
        let mut buffer = ops(0, 9);
        spill.flush(&mut buffer).unwrap();
        assert_eq!(spill.kept_dir().as_deref(), Some(root.as_path()));
        let mut rest = ops(9, 3);
        let store = spill.finish(&mut rest).unwrap();
        assert_eq!(store.len(), 12);
        drop(store);
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            12 * 8,
            "kept after the replay's mapping"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
    /// R-B2 (Astra's partial-write witness): a short trace write after a saved prefix is an error naming the file;
    /// the file is cut back to the saved prefix, the spill refuses every later flush, sync and finish (no retried
    /// append can duplicate the partial bytes), the error is a storage stop, and a fresh process resumes from the
    /// saved (count, digest).
    /// The two R-B2 tests read the process-wide storage stop: one at a time.
    static STOP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_short_trace_write_fails_the_spill_and_keeps_the_saved_prefix() {
        let _serial = STOP_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root =
            std::env::temp_dir().join(format!("rustred-short-write-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let ops = |from: u32, n: u32| -> Vec<Op> {
            (from..from + n)
                .map(|i| Op::new(OpTag::Add, i % 5, CoefRef(i)))
                .collect()
        };
        let path = root.join("ops.bin");
        let mut spill = Spill::at(&root, None).unwrap();
        let mut buffer = ops(0, 20);
        spill.flush(&mut buffer).unwrap();
        spill.sync().unwrap();
        let (saved, digest) = (spill.flushed as u64, spill.digest().unwrap());
        // the state that covers the prefix (its content is not read here)
        std::fs::write(root.join("state.bin"), b"state").unwrap();
        let _ = super::super::super::resume::take_storage_stop();
        // a short write: 3 ops and 4 bytes of the next 10 reach the file, then EFBIG
        SHORT_WRITE.with(|c| c.set(Some(28)));
        let mut buffer = ops(20, 10);
        let e = spill.flush(&mut buffer).unwrap_err().to_string();
        assert!(
            e.contains("trace spill write") && e.contains("ops.bin"),
            "{e}"
        );
        assert_eq!(buffer.len(), 10, "the unwritten ops are not dropped");
        assert_eq!(spill.flushed as u64, saved);
        assert_eq!(spill.digest().unwrap(), digest);
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            saved * 8,
            "cut back"
        );
        let stop = super::super::super::resume::take_storage_stop().expect("a storage stop");
        assert!(stop.contains("ops.bin"), "{stop}");
        // a retry in this process is refused (the writer never appends again) ...
        let e = spill.flush(&mut buffer).unwrap_err().to_string();
        assert!(e.contains("earlier error"), "{e}");
        assert!(spill.sync().is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().len(), saved * 8);
        let mut rest = Vec::new();
        assert!(spill.finish(&mut rest).is_err());
        // ... and a fresh process continues from the saved prefix
        let mut spill = Spill::at(&root, Some((saved, Some(digest)))).unwrap();
        let mut buffer = ops(20, 10);
        spill.flush(&mut buffer).unwrap();
        let whole = std::fs::read(&path).unwrap();
        assert_eq!(whole.len(), 30 * 8);
        assert_eq!(spill.digest().unwrap(), *blake3::hash(&whole).as_bytes());
        drop(spill);
        std::fs::remove_dir_all(&root).unwrap();
    }

    /// R-B2 (Astra's persistent-sync witness): a failed trace sync is an error naming the file and a storage stop;
    /// it is never retried, no later flush appends, and a trace no saved state covers is emptied.
    #[test]
    fn a_failed_trace_sync_fails_the_spill() {
        let _serial = STOP_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root =
            std::env::temp_dir().join(format!("rustred-sync-fail-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut spill = Spill::at(&root, None).unwrap();
        let mut buffer: Vec<Op> = (0..8u32)
            .map(|i| Op::new(OpTag::Add, i, CoefRef(i)))
            .collect();
        spill.flush(&mut buffer).unwrap();
        let _ = super::super::super::resume::take_storage_stop();
        FAIL_SYNC.with(|f| f.set(true));
        let e = spill.sync().unwrap_err().to_string();
        assert!(
            e.contains("trace spill sync") && e.contains("ops.bin"),
            "{e}"
        );
        assert!(super::super::super::resume::take_storage_stop().is_some());
        assert!(spill.sync().is_err(), "a failed sync is not retried");
        let mut buffer = vec![Op::new(OpTag::Add, 0, CoefRef(0))];
        assert!(spill.flush(&mut buffer).is_err());
        // no saved state covers the trace: emptied, so a restart starts afresh
        assert_eq!(std::fs::metadata(root.join("ops.bin")).unwrap().len(), 0);
        drop(spill);
        assert!(Spill::at(&root, None).is_ok(), "an empty trace is reused");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
