//! The recorder's dictionary of distinct coefficients, with an optional
//! spill of large entries to disk (stale-memory work, T277 / T278 14-line
//! records: in their heavy stretches every key interns ~1,400 new, large and
//! rarely re-used coefficients; held in RAM they took the records past
//! 140 GiB).
//!
//! `RUSTRED_DICTIONARY_SPILL_BYTES=<n>` (off by default; needs a spill or
//! resume directory): a new entry whose heap size exceeds `n` bytes is written
//! to `<dir>/dict-<pid>-<seq>.bin` (step-cache codec) and only its 128-bit
//! digest stays in RAM. Small entries stay in RAM (the fast path: most
//! lookups are hits on small, frequent coefficients). Equality against a
//! spilled entry compares digests, computed for the looked-up coefficient only
//! when its 64-bit hash already matches a spilled entry. Reads (end of the
//! record, state saves) stream the spilled entries back. The file is a cache:
//! a resumed request rebuilds it from the saved state, and it is removed when
//! the dictionary is dropped.
use std::borrow::Cow;
use std::cell::Cell;
use std::io::Write;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use crate::algebra::Coefficient;
use crate::reduction::ReductionError;

use super::super::resume;
use super::super::step_cache::codec::{Reader, Writer};
use super::error;

struct Spilled {
    offset: u64,
    len: u32,
    digest: u128,
}

struct DictSpill {
    file: std::fs::File,
    path: PathBuf,
    end: u64,
    threshold: usize,
    template: Coefficient,
    /// Spilled entries by index.
    at: std::collections::HashMap<u32, Spilled>,
}

impl Drop for DictSpill {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Default)]
pub(super) struct Entries {
    mem: Vec<Option<Coefficient>>,
    spill: Option<DictSpill>,
}

fn spill_threshold() -> Option<usize> {
    static N: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("RUSTRED_DICTIONARY_SPILL_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
    })
}

fn encode(coefficient: &Coefficient) -> Vec<u8> {
    let mut w = Writer { bytes: Vec::new() };
    resume::put_coefficient(&mut w, coefficient);
    w.bytes
}

fn digest(bytes: &[u8]) -> u128 {
    let hash = blake3::hash(bytes);
    u128::from_le_bytes(hash.as_bytes()[..16].try_into().expect("16 bytes"))
}

/// A clone holds every entry in RAM (spilled entries read back): clones are
/// rare (channel replay copies) and must not share the spill file.
impl Clone for Entries {
    fn clone(&self) -> Self {
        Self {
            mem: (0..self.mem.len())
                .map(|i| {
                    Some(
                        self.get(i)
                            .expect("dictionary spill readable while cloning")
                            .into_owned(),
                    )
                })
                .collect(),
            spill: None,
        }
    }
}

impl Entries {
    /// Turn the spill on for this dictionary (when the switch is set), with
    /// its file in `dir`; existing entries stay where they are.
    pub(super) fn enable_spill(&mut self, dir: &Path, template: Coefficient) {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let Some(threshold) = spill_threshold() else {
            return;
        };
        if self.spill.is_some() {
            return;
        }
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = dir.join(format!("dict-{}-{seq}.bin", std::process::id()));
        let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
        else {
            eprintln!(
                "rustred: dictionary spill disabled (cannot create {})",
                path.display()
            );
            return;
        };
        self.spill = Some(DictSpill {
            file,
            path,
            end: 0,
            threshold,
            template,
            at: Default::default(),
        });
    }

    /// A RAM-only dictionary of `entries` (channel replay copies).
    pub(super) fn from_vec(entries: Vec<Coefficient>) -> Self {
        Self {
            mem: entries.into_iter().map(Some).collect(),
            spill: None,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.mem.len()
    }

    /// Append a new distinct entry (spilled when large and the spill is on).
    pub(super) fn push(&mut self, coefficient: &Coefficient) -> Result<(), ReductionError> {
        let index = self.mem.len() as u32;
        if let Some(spill) = &mut self.spill
            && super::super::coefficient_heap_bytes(coefficient) > spill.threshold
        {
            let bytes = encode(coefficient);
            let mut file = &spill.file;
            file.write_all(&bytes).map_err(|e| {
                let message = format!("dictionary spill write: {e}");
                super::super::resume::storage_stop(&message);
                error(message)
            })?;
            spill.at.insert(
                index,
                Spilled {
                    offset: spill.end,
                    len: bytes.len() as u32,
                    digest: digest(&bytes),
                },
            );
            spill.end += bytes.len() as u64;
            self.mem.push(None);
            return Ok(());
        }
        self.mem.push(Some(coefficient.clone()));
        Ok(())
    }

    /// Entry `index` (borrowed from RAM or read back from the spill file).
    pub(super) fn get(&self, index: usize) -> Result<Cow<'_, Coefficient>, ReductionError> {
        if let Some(Some(coefficient)) = self.mem.get(index) {
            return Ok(Cow::Borrowed(coefficient));
        }
        let spill = self
            .spill
            .as_ref()
            .ok_or_else(|| error("a spilled dictionary entry without its spill"))?;
        let at = spill
            .at
            .get(&(index as u32))
            .ok_or_else(|| error("a dictionary entry is missing"))?;
        let mut bytes = vec![0u8; at.len as usize];
        spill
            .file
            .read_exact_at(&mut bytes, at.offset)
            .map_err(|e| error(format!("dictionary spill read: {e}")))?;
        if digest(&bytes) != at.digest {
            return Err(error(
                "dictionary spill: an entry does not match its digest",
            ));
        }
        let mut r = Reader::new(&bytes);
        resume::get_coefficient(&mut r, &spill.template)
            .map(Cow::Owned)
            .map_err(|e| error(format!("dictionary spill decode: {e}")))
    }

    /// Whether entry `index` equals `coefficient`; `cache` holds the digest
    /// of `coefficient` once computed (for a spilled entry only).
    pub(super) fn eq_at(
        &self,
        index: u32,
        coefficient: &Coefficient,
        cache: &Cell<Option<u128>>,
    ) -> bool {
        match self.mem.get(index as usize) {
            Some(Some(entry)) => entry == coefficient,
            Some(None) => {
                let Some(at) = self.spill.as_ref().and_then(|s| s.at.get(&index)) else {
                    return false;
                };
                let digest = cache.get().unwrap_or_else(|| {
                    let d = digest(&encode(coefficient));
                    cache.set(Some(d));
                    d
                });
                at.digest == digest
            }
            None => false,
        }
    }

    /// Heap bytes held in RAM, entries spilled, bytes on disk (statistics).
    pub(super) fn census(&self) -> (usize, usize, u64) {
        let heap = self
            .mem
            .iter()
            .flatten()
            .map(super::super::coefficient_heap_bytes)
            .sum();
        let (spilled, bytes) = self.spill.as_ref().map_or((0, 0), |s| (s.at.len(), s.end));
        (heap, spilled, bytes)
    }

    pub(super) fn vec_bytes(&self) -> usize {
        self.mem.capacity() * std::mem::size_of::<Option<Coefficient>>()
    }
}
