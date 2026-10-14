//! Compact negative lookup metadata, not integral or coefficient authority.
//!
//! The index checksum authenticates these bytes in the existing trusted-native
//! snapshot boundary. A coherent malicious index rewrite is outside that trust
//! boundary, just as for the first/last ranges. Actual chunk reads cross-check
//! the entire filter against the native key census. Hash collisions only cause
//! extra reads; full native key equality is still required for every hit.
use std::io::{self, Write};

use rustred::persistence::application_cache::ApplicationCacheKey;

use super::{AppError, fail};

pub(crate) fn fingerprint(key: &ApplicationCacheKey) -> Result<u64, AppError> {
    let mut writer = HashWriter(blake3::Hasher::new());
    writer.0.update(b"RustRed cache key fingerprint v1\0");
    bincode::encode_into_std_write(key, &mut writer, bincode::config::standard()).map_err(fail)?;
    let hash = writer.0.finalize();
    Ok(u64::from_le_bytes(
        hash.as_bytes()[..8].try_into().expect("eight bytes"),
    ))
}

pub(crate) fn collect<'a>(
    keys: impl ExactSizeIterator<Item = &'a ApplicationCacheKey>,
) -> Result<Vec<u64>, AppError> {
    let mut fingerprints = Vec::new();
    fingerprints.try_reserve_exact(keys.len()).map_err(fail)?;
    for key in keys {
        fingerprints.push(fingerprint(key)?);
    }
    // Keep collisions so records.len() remains an independently checked bound.
    fingerprints.sort_unstable();
    Ok(fingerprints)
}

pub(crate) fn may_contain(fingerprints: &[u64], fingerprint: u64) -> bool {
    fingerprints.binary_search(&fingerprint).is_ok()
}

struct HashWriter(blake3::Hasher);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
