//! vk5l-physrun level 2: the trace ops of a recorded request spilled to a
//! file as they are recorded (append-only, 8 B per op), and read back by
//! mmap for the replay, so that the forward pass and the replay hold no
//! in-memory copy of the trace. `RUSTRED_TRACE_SPILL_DIR=<dir>` enables it
//! (one-thread recorded requests without tail rewrites or layered mode);
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
}

impl Spill {
    pub(super) fn create(dir: PathBuf) -> Result<Self, ReductionError> {
        Ok(Self {
            dir,
            file: None,
            flushed: 0,
            keep: false,
        })
    }

    /// Resume mode: `<dir>/ops.bin`, cut back to `ops` ops when continuing
    /// a saved state (else created empty); kept on drop.
    pub(super) fn at(dir: &std::path::Path, ops: Option<u64>) -> Result<Self, ReductionError> {
        std::fs::create_dir_all(dir).map_err(|e| error(format!("resume directory: {e}")))?;
        let path = dir.join("ops.bin");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(ops.is_none())
            .open(&path)
            .map_err(|e| error(format!("resume ops file: {e}")))?;
        let flushed = ops.unwrap_or(0) as usize;
        file.set_len((flushed * std::mem::size_of::<Op>()) as u64)
            .map_err(|e| error(format!("resume ops cut: {e}")))?;
        use std::io::Seek;
        let mut file = file;
        file.seek(std::io::SeekFrom::End(0))
            .map_err(|e| error(format!("resume ops seek: {e}")))?;
        Ok(Self {
            dir: dir.to_path_buf(),
            file: Some((file, path)),
            flushed,
            keep: true,
        })
    }

    /// Append `ops` and clear them.
    pub(super) fn flush(&mut self, ops: &mut Vec<Op>) -> Result<(), ReductionError> {
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
        file.write_all(bytes)
            .map_err(|e| error(format!("trace spill write: {e}")))?;
        self.flushed += ops.len();
        ops.clear();
        Ok(())
    }

    /// Make the written ops durable (resume checkpoints).
    pub(super) fn sync(&mut self) -> Result<(), ReductionError> {
        if let Some((file, _)) = &mut self.file {
            file.flush()
                .map_err(|e| error(format!("trace spill flush: {e}")))?;
            file.sync_data()
                .map_err(|e| error(format!("trace spill sync: {e}")))?;
        }
        Ok(())
    }

    /// The whole trace: the rest appended and the file mapped (or the rest
    /// itself when nothing was ever flushed).
    pub(super) fn finish(mut self, rest: &mut Vec<Op>) -> Result<OpStore, ReductionError> {
        if self.file.is_none() || (self.keep && self.flushed == 0 && rest.len() < block_ops()) {
            if let Some((_, path)) = self.file.take() {
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
        OpStore::map(path, self.flushed)
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
    },
}

// SAFETY: the mapping is read-only and lives as long as the store.
unsafe impl Send for OpStore {}
unsafe impl Sync for OpStore {}

impl OpStore {
    fn map(path: PathBuf, len: usize) -> Result<Self, ReductionError> {
        if len == 0 {
            let _ = std::fs::remove_file(&path);
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
        })
    }
}

impl Drop for OpStore {
    fn drop(&mut self) {
        if let Self::Mapped { ptr, len, path } = self {
            unsafe { libc::munmap((*ptr).cast_mut().cast(), *len * std::mem::size_of::<Op>()) };
            let _ = std::fs::remove_file(&*path);
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
}
