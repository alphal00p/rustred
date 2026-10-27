//! One authenticated gzip member around one native cache chunk/table.
//!
//! This trusted-generated transport never certifies a reduction identity. The
//! solver still checks entry policy, algebra and cache admission at every hit.
//! File identity is supplied from an independently authenticated manifest; the
//! embedded native binding includes the entire campaign and solver layer.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use flate2::{Compression, GzBuilder, bufread::GzDecoder};
use rustred::{
    algebra::CoefficientContext,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheIoLimits, ApplicationCacheKey,
        ApplicationCacheRecord, OwnedApplicationCacheChunk, encode_application_cache_chunk,
    },
};
use serde::{Deserialize, Serialize};

use super::cache_pack::{io_error, resource_allocation, resource_checkpoint};
use crate::AppError;

#[cfg(test)]
#[path = "cache_file_tests.rs"]
mod tests;

#[path = "cache_file/stream.rs"]
mod stream;
pub use stream::SavedApplicationCacheFileWriter;

const MAGIC: &[u8; 16] = b"RR-CACHE-FILE-01";
const HEADER_BYTES: usize = MAGIC.len() + 8 + 32;
const MAX_COMPRESSED_BYTES: usize = 95 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 1024 * 1024 * 1024;

/// External content identity. Hashes use BLAKE3; decoded bytes/hash cover the
/// complete uncompressed framing plus native chunk, not only coefficients.
/// A manifest containing this identity must itself be authenticated by callers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedApplicationCacheFileIdentity {
    pub compressed_bytes: u64,
    pub compressed_blake3: [u8; 32],
    pub decoded_bytes: u64,
    pub decoded_blake3: [u8; 32],
}

/// Caller-owned transport admission. Defaults retain the compact distribution
/// policy (95 MiB compressed, 1 GiB decoded); explicitly larger grants support
/// independently hosted caches. Index/payload grants do not bound scratch RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedApplicationCacheFileLimits {
    pub chunk: ApplicationCacheIoLimits,
    pub max_compressed_bytes: usize,
}
impl Default for SavedApplicationCacheFileLimits {
    fn default() -> Self {
        let mut chunk = ApplicationCacheIoLimits::default();
        chunk.binary.max_program_bytes = MAX_DECODED_BYTES - HEADER_BYTES;
        chunk.binary.max_total_atom_bytes = MAX_DECODED_BYTES - HEADER_BYTES;
        chunk.max_records = 1_000_000;
        Self {
            chunk,
            max_compressed_bytes: MAX_COMPRESSED_BYTES,
        }
    }
}
impl SavedApplicationCacheFileLimits {
    /// Maximum total decoded frame size, including the native file header.
    pub fn max_decoded_bytes(self) -> usize {
        self.chunk
            .binary
            .max_program_bytes
            .saturating_add(HEADER_BYTES)
    }

    pub fn with_decoded_bytes(mut self, bytes: usize) -> Result<Self, AppError> {
        let payload = bytes
            .checked_sub(HEADER_BYTES)
            .ok_or_else(|| fail("decoded grant must include the56-byte native header"))?;
        self.chunk.binary.max_program_bytes = payload;
        self.chunk.binary.max_total_atom_bytes = payload;
        Ok(self)
    }

    fn decoded_limit(self) -> usize {
        self.chunk
            .binary
            .max_program_bytes
            .saturating_add(HEADER_BYTES)
    }
}

/// One native file codec with either retained memory or a private decoded file
/// reader. The latter needs decoded disk space but does not retain full payload
/// copies in RAM. Reader forks share its lifetime, indices and lazy state map.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationCacheFileOptions {
    pub limits: SavedApplicationCacheFileLimits,
    pub decoded_directory: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedApplicationCacheFileStatistics {
    pub records: usize,
    pub compressed_bytes: u64,
    pub decoded_bytes: u64,
    pub index_owned_bytes: usize,
    /// Shared across forked readers; increases only on first coefficient access.
    pub decoded_coefficients: usize,
    pub decoded_coefficient_terms: usize,
    pub decoded_coefficient_bytes: usize,
    /// Successful StateMap imports retained by the shared lazy table (0 or 1).
    pub state_imports: usize,
    /// Reader-local successful lookup counters.
    pub hits: usize,
    pub misses: usize,
}

/// Immutable cache payload/table shared by reader forks, plus local counters.
/// Opening authenticates and decompresses once. Lookups decode only the selected
/// record through a shared lazy table and never import all coefficients afresh.
pub struct SavedApplicationCacheFile {
    binding: ApplicationCacheBinding,
    identity: SavedApplicationCacheFileIdentity,
    chunk: Arc<OwnedApplicationCacheChunk>,
    hits: usize,
    misses: usize,
}
impl SavedApplicationCacheFile {
    /// Deterministic gzip (mtime zero, no filename) containing exactly one native
    /// coefficient table. Records must be ordered/unique completed native nodes.
    pub fn encode_generated(
        binding: &ApplicationCacheBinding,
        records: &[ApplicationCacheRecord],
        context: &CoefficientContext,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<(Vec<u8>, SavedApplicationCacheFileIdentity), AppError> {
        resource_checkpoint()?;
        let native = encode_application_cache_chunk(binding, records, context, limits.chunk)
            .map_err(binary_error)?;
        let native_digest = blake3::hash(&native);
        let decoded_bytes = HEADER_BYTES
            .checked_add(native.len())
            .ok_or_else(|| fail("decoded cache frame overflow"))?;
        let mut header = [0u8; HEADER_BYTES];
        header[..16].copy_from_slice(MAGIC);
        header[16..24].copy_from_slice(&(native.len() as u64).to_le_bytes());
        header[24..].copy_from_slice(native_digest.as_bytes());
        let mut decoded_hasher = blake3::Hasher::new();
        decoded_hasher.update(&header);
        decoded_hasher.update(&native);
        let mut output = BoundedBytes::new(limits.max_compressed_bytes);
        let compressed = (|| {
            let mut encoder = GzBuilder::new()
                .mtime(0)
                .operating_system(255)
                .write(&mut output, Compression::best());
            encoder.write_all(&header)?;
            encoder.write_all(&native)?;
            encoder.finish()?;
            Ok::<(), io::Error>(())
        })();
        if output.exceeded {
            return Err(AppError::limit(
                "compressed application cache file exceeds allowance",
            ));
        }
        compressed.map_err(io_error)?;
        let encoded = output.bytes;
        let identity = SavedApplicationCacheFileIdentity {
            compressed_bytes: encoded.len() as u64,
            compressed_blake3: *blake3::hash(&encoded).as_bytes(),
            decoded_bytes: decoded_bytes as u64,
            decoded_blake3: *decoded_hasher.finalize().as_bytes(),
        };
        resource_checkpoint()?;
        Ok((encoded, identity))
    }

    /// Open an explicit trusted-generated file without an external manifest.
    /// The native campaign binding, embedded decoded digest, gzip CRC and exact
    /// one-member framing are checked. The returned whole-file identity can be
    /// recorded by the caller; authentication of its origin remains the caller's
    /// responsibility. Official distribution should use `open_generated`.
    pub fn open(
        path: &Path,
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        Self::open_with_options(
            path,
            expected,
            None,
            &SavedApplicationCacheFileOptions {
                limits,
                ..Default::default()
            },
        )
    }

    pub fn open_generated(
        path: &Path,
        expected: &ApplicationCacheBinding,
        identity: &SavedApplicationCacheFileIdentity,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        Self::open_with_options(
            path,
            expected,
            Some(identity),
            &SavedApplicationCacheFileOptions {
                limits,
                ..Default::default()
            },
        )
    }

    /// Explicit trusted-generated byte import; see `open` for authenticity.
    pub fn decode(
        encoded: &[u8],
        expected: &ApplicationCacheBinding,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        Self::decode_inner(encoded, expected, None, limits)
    }

    /// Verify both external digests/lengths and the entire native binding before
    /// exposing the file. Extra gzip members/trailing bytes are always rejected.
    pub fn decode_generated(
        encoded: &[u8],
        expected: &ApplicationCacheBinding,
        identity: &SavedApplicationCacheFileIdentity,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        Self::decode_inner(encoded, expected, Some(identity), limits)
    }

    fn decode_inner(
        encoded: &[u8],
        expected: &ApplicationCacheBinding,
        expected_identity: Option<&SavedApplicationCacheFileIdentity>,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        Self::decode_reader(
            encoded,
            expected,
            expected_identity,
            &SavedApplicationCacheFileOptions {
                limits,
                ..Default::default()
            },
        )
    }

    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    pub fn identity(&self) -> &SavedApplicationCacheFileIdentity {
        &self.identity
    }
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &ApplicationCacheKey> + Clone {
        self.chunk.keys()
    }
    pub fn statistics(&self) -> Result<SavedApplicationCacheFileStatistics, AppError> {
        let (decoded_coefficient_terms, decoded_coefficient_bytes) = self
            .chunk
            .decoded_coefficient_census()
            .map_err(binary_error)?;
        Ok(SavedApplicationCacheFileStatistics {
            records: self.chunk.len(),
            compressed_bytes: self.identity.compressed_bytes,
            decoded_bytes: self.identity.decoded_bytes,
            index_owned_bytes: self.chunk.index_owned_bytes(),
            decoded_coefficients: self
                .chunk
                .decoded_coefficient_count()
                .map_err(binary_error)?,
            decoded_coefficient_terms,
            decoded_coefficient_bytes,
            state_imports: self.chunk.state_import_count().map_err(binary_error)?,
            hits: self.hits,
            misses: self.misses,
        })
    }
    pub fn fork_reader(&self) -> Self {
        Self {
            binding: self.binding.clone(),
            identity: self.identity,
            chunk: Arc::clone(&self.chunk),
            hits: 0,
            misses: 0,
        }
    }
    pub fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, AppError> {
        let record = self
            .chunk
            .decode_record(key, context)
            .map_err(binary_error)?;
        if record.is_some() {
            self.hits = self.hits.saturating_add(1);
        } else {
            self.misses = self.misses.saturating_add(1);
        }
        resource_checkpoint()?;
        Ok(record)
    }
}

fn binary_error(error: rustred::persistence::BinaryIoError) -> AppError {
    match error {
        rustred::persistence::BinaryIoError::Resource(error) => AppError::resource(error),
        rustred::persistence::BinaryIoError::Limit { .. } => {
            AppError::limit(format!("application cache file: {error}"))
        }
        _ => fail(error),
    }
}

fn validate_identity(
    identity: &SavedApplicationCacheFileIdentity,
    limits: SavedApplicationCacheFileLimits,
) -> Result<(usize, usize), AppError> {
    let encoded = usize::try_from(identity.compressed_bytes).map_err(fail)?;
    let decoded = usize::try_from(identity.decoded_bytes).map_err(fail)?;
    if encoded > limits.max_compressed_bytes || decoded > limits.decoded_limit() {
        return Err(AppError::limit(
            "application cache file transport allowance exceeded",
        ));
    }
    if encoded == 0 || decoded < HEADER_BYTES {
        return Err(fail("application cache file identity has invalid length"));
    }
    Ok((encoded, decoded))
}
fn fail(error: impl std::fmt::Display) -> AppError {
    AppError::input(format!("application cache file: {error}"))
}
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}
impl BoundedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self.bytes.len().checked_add(bytes.len());
        if length.is_none_or(|length| length > self.limit) {
            self.exceeded = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "compressed cache output allowance exceeded",
            ));
        }
        resource_checkpoint().map_err(io::Error::other)?;
        let additional = bytes
            .len()
            .saturating_sub(self.bytes.capacity() - self.bytes.len());
        resource_allocation(additional).map_err(io::Error::other)?;
        self.bytes.try_reserve_exact(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "compressed cache allocation failed",
            )
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl rustred::reduction::GuardedCacheBacking for SavedApplicationCacheFile {
    fn binding(&self) -> &ApplicationCacheBinding {
        self.binding()
    }
    fn lookup(
        &mut self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, rustred::reduction::GuardedCacheBackingError> {
        SavedApplicationCacheFile::lookup(self, key, context).map_err(|error| {
            use rustred::reduction::GuardedCacheBackingErrorKind;
            if let Some(resource) = error.resource_limit_error() {
                return rustred::reduction::GuardedCacheBackingError::resource(resource.clone());
            }
            rustred::reduction::GuardedCacheBackingError {
                kind: if error.kind() == crate::AppErrorKind::Limit {
                    GuardedCacheBackingErrorKind::Limit
                } else {
                    GuardedCacheBackingErrorKind::Invalid
                },
                message: error.to_string(),
            }
        })
    }
    fn store_completed(
        &mut self,
        _: &ApplicationCacheRecord,
        _: &CoefficientContext,
    ) -> Result<(), rustred::reduction::GuardedCacheBackingError> {
        // The application only calls lookup on an attached reader. Silently
        // accepting an export here would falsely report discarded records.
        Err(rustred::reduction::GuardedCacheBackingError {
            kind: rustred::reduction::GuardedCacheBackingErrorKind::Invalid,
            message: "application cache file is read-only; export requires a writer".into(),
        })
    }
}
