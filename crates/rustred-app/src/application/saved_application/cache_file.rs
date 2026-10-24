//! One authenticated gzip member around one native cache chunk/table.
//!
//! This trusted-generated transport never certifies a reduction identity. The
//! solver still checks entry policy, algebra and cache admission at every hit.
//! File identity is supplied from an independently authenticated manifest; the
//! embedded native binding includes the entire campaign and solver layer.

use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
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

/// Explicit resource admission, independently capped at 95 MiB compressed and
/// 1 GiB decompressed. Index/payload allowances do not bound native scratch RSS.
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
    fn effective(self) -> Self {
        let mut limits = self;
        limits.max_compressed_bytes = limits.max_compressed_bytes.min(MAX_COMPRESSED_BYTES);
        limits.chunk.binary.max_program_bytes = limits
            .chunk
            .binary
            .max_program_bytes
            .min(MAX_DECODED_BYTES - HEADER_BYTES);
        limits
    }
    fn decoded_limit(self) -> usize {
        self.chunk.binary.max_program_bytes + HEADER_BYTES
    }
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
        let limits = limits.effective();
        let native = encode_application_cache_chunk(binding, records, context, limits.chunk)
            .map_err(binary_error)?;
        let native_digest = blake3::hash(&native);
        let mut decoded = Vec::new();
        let decoded_bytes = HEADER_BYTES
            .checked_add(native.len())
            .ok_or_else(|| fail("decoded cache frame overflow"))?;
        resource_allocation(decoded_bytes)?;
        decoded.try_reserve_exact(decoded_bytes).map_err(fail)?;
        decoded.extend_from_slice(MAGIC);
        decoded.extend_from_slice(&(native.len() as u64).to_le_bytes());
        decoded.extend_from_slice(native_digest.as_bytes());
        decoded.extend_from_slice(&native);
        drop(native);
        let mut output = BoundedBytes::new(limits.max_compressed_bytes);
        let compressed = (|| {
            let mut encoder = GzBuilder::new()
                .mtime(0)
                .operating_system(255)
                .write(&mut output, Compression::best());
            encoder.write_all(&decoded)?;
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
            decoded_bytes: decoded.len() as u64,
            decoded_blake3: *blake3::hash(&decoded).as_bytes(),
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
        let encoded = read_file(path, None, limits.effective())?;
        Self::decode(&encoded, expected, limits)
    }

    pub fn open_generated(
        path: &Path,
        expected: &ApplicationCacheBinding,
        identity: &SavedApplicationCacheFileIdentity,
        limits: SavedApplicationCacheFileLimits,
    ) -> Result<Self, AppError> {
        resource_checkpoint()?;
        let limits = limits.effective();
        validate_identity(identity, limits)?;
        let encoded = read_file(path, Some(identity.compressed_bytes), limits)?;
        Self::decode_generated(&encoded, expected, identity, limits)
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
        resource_checkpoint()?;
        let limits = limits.effective();
        if encoded.len() > limits.max_compressed_bytes {
            return Err(AppError::limit(
                "compressed application cache file exceeds allowance",
            ));
        }
        let encoded_hash = *blake3::hash(encoded).as_bytes();
        if let Some(identity) = expected_identity {
            let (encoded_length, _) = validate_identity(identity, limits)?;
            if encoded.len() != encoded_length || encoded_hash != identity.compressed_blake3 {
                return Err(fail("compressed application cache file identity mismatch"));
            }
        }
        // A bufread decoder over a slice preserves the unconsumed suffix after
        // its first member. A Read decoder can swallow trailing bytes.
        let mut decoder = GzDecoder::new(encoded);
        let mut header = [0u8; HEADER_BYTES];
        decoder.read_exact(&mut header).map_err(fail)?;
        if &header[..16] != MAGIC {
            return Err(fail("application cache file framing version mismatch"));
        }
        let native_length = usize::try_from(u64::from_le_bytes(
            header[16..24].try_into().expect("fixed header"),
        ))
        .map_err(fail)?;
        if native_length > limits.chunk.binary.max_program_bytes {
            return Err(AppError::limit(
                "decoded application cache file exceeds allowance",
            ));
        }
        let decoded_length = native_length + HEADER_BYTES;
        if expected_identity.is_some_and(|identity| identity.decoded_bytes != decoded_length as u64)
        {
            return Err(fail("decoded application cache file length mismatch"));
        }
        let mut native = Vec::new();
        resource_allocation(native_length)?;
        native.try_reserve_exact(native_length).map_err(fail)?;
        let mut buffer = [0u8; 16 * 1024];
        while native.len() < native_length {
            resource_checkpoint()?;
            let requested = buffer.len().min(native_length - native.len());
            let count = decoder.read(&mut buffer[..requested]).map_err(fail)?;
            if count == 0 {
                return Err(fail("truncated decoded application cache file"));
            }
            native.extend_from_slice(&buffer[..count]);
        }
        // Force CRC/trailer validation with an allocation-free overrun probe.
        if decoder.read(&mut buffer[..1]).map_err(fail)? != 0 {
            return Err(fail(
                "decoded application cache file exceeds declared length",
            ));
        }
        if !decoder.into_inner().is_empty() {
            return Err(fail(
                "trailing gzip member or bytes in application cache file",
            ));
        }
        if blake3::hash(&native).as_bytes() != &header[24..56] {
            return Err(fail("native application cache chunk digest mismatch"));
        }
        let mut decoded_hasher = blake3::Hasher::new();
        decoded_hasher.update(&header);
        decoded_hasher.update(&native);
        let identity = SavedApplicationCacheFileIdentity {
            compressed_bytes: encoded.len() as u64,
            compressed_blake3: encoded_hash,
            decoded_bytes: decoded_length as u64,
            decoded_blake3: *decoded_hasher.finalize().as_bytes(),
        };
        if expected_identity.is_some_and(|expected| *expected != identity) {
            return Err(fail("decoded application cache file digest mismatch"));
        }
        resource_allocation(native.len())?;
        let chunk = OwnedApplicationCacheChunk::inspect(Arc::from(native), expected, limits.chunk)
            .map_err(binary_error)?;
        Ok(Self {
            binding: expected.clone(),
            identity,
            chunk: Arc::new(chunk),
            hits: 0,
            misses: 0,
        })
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

fn read_file(
    path: &Path,
    expected_bytes: Option<u64>,
    limits: SavedApplicationCacheFileLimits,
) -> Result<Vec<u8>, AppError> {
    if !std::fs::metadata(path).map_err(fail)?.is_file() {
        return Err(fail("application cache path is not a regular file"));
    }
    let mut file = File::open(path).map_err(|e| fail(format!("{}: {e}", path.display())))?;
    let metadata = file.metadata().map_err(fail)?;
    if !metadata.is_file() {
        return Err(fail("application cache path changed to a nonregular file"));
    }
    let length = metadata.len();
    if length > limits.max_compressed_bytes as u64 {
        return Err(AppError::limit(
            "compressed application cache file exceeds allowance",
        ));
    }
    if expected_bytes.is_some_and(|expected| length != expected) {
        return Err(fail("compressed application cache file length mismatch"));
    }
    let mut encoded = Vec::new();
    let length_usize = usize::try_from(length).map_err(fail)?;
    resource_allocation(length_usize)?;
    encoded.try_reserve_exact(length_usize).map_err(fail)?;
    Read::by_ref(&mut file)
        .take(length)
        .read_to_end(&mut encoded)
        .map_err(fail)?;
    let mut probe = [0u8; 1];
    if encoded.len() != length as usize || file.read(&mut probe).map_err(fail)? != 0 {
        return Err(fail("application cache file changed during read"));
    }
    resource_checkpoint()?;
    Ok(encoded)
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
