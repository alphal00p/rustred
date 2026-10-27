//! Streaming transport for the existing single-file native codec.
use super::*;
use rustred::persistence::application_cache::{
    ApplicationCacheChunkWriteStatistics, ApplicationCacheChunkWriter,
};
use std::io::{BufReader, BufWriter, Seek, SeekFrom};

/// Incremental ordered-record export. The native file is spooled once, then
/// streamed into one gzip member. Only the unique coefficient interner remains
/// resident; its measured payload/index floor is returned at completion.
pub struct SavedApplicationCacheFileWriter<'a> {
    native: ApplicationCacheChunkWriter<'a, TemporaryFile>,
    limits: SavedApplicationCacheFileLimits,
    directory: PathBuf,
    unordered: bool,
}

impl<'a> SavedApplicationCacheFileWriter<'a> {
    pub fn new(
        binding: &ApplicationCacheBinding,
        context: &'a CoefficientContext,
        limits: SavedApplicationCacheFileLimits,
        temporary_directory: &Path,
    ) -> Result<Self, AppError> {
        let spool = TemporaryFile::new(temporary_directory)?;
        Ok(Self {
            native: ApplicationCacheChunkWriter::new(spool, binding, context, limits.chunk)
                .map_err(binary_error)?,
            limits,
            directory: temporary_directory.to_owned(),
            unordered: false,
        })
    }

    /// Existing completed-record visitors emit deterministic scan order. This
    /// mode spools their rows, deduplicates identical complete records and
    /// rejects conflicting coefficients or ordered guards before publication.
    pub fn new_unordered(
        binding: &ApplicationCacheBinding,
        context: &'a CoefficientContext,
        limits: SavedApplicationCacheFileLimits,
        temporary_directory: &Path,
    ) -> Result<Self, AppError> {
        let spool = TemporaryFile::new(temporary_directory)?;
        Ok(Self {
            native: ApplicationCacheChunkWriter::new_unordered(
                spool,
                binding,
                context,
                limits.chunk,
            )
            .map_err(binary_error)?,
            limits,
            directory: temporary_directory.to_owned(),
            unordered: true,
        })
    }

    pub fn push(&mut self, record: &ApplicationCacheRecord) -> Result<(), AppError> {
        self.native.push(record).map_err(binary_error)
    }

    pub fn push_unordered(&mut self, record: &ApplicationCacheRecord) -> Result<(), AppError> {
        self.native.push_unordered(record).map_err(binary_error)
    }

    /// Finalize into a caller-owned destination. Failure may leave an output
    /// prefix; the caller owns atomic publication and must not expose it.
    pub fn finish<W: Write>(
        self,
        output: W,
    ) -> Result<
        (
            W,
            SavedApplicationCacheFileIdentity,
            ApplicationCacheChunkWriteStatistics,
        ),
        AppError,
    > {
        let (mut spool, statistics) = if self.unordered {
            self.native
                .finish_sorted_into(TemporaryFile::new(&self.directory)?)
                .map_err(binary_error)?
        } else {
            self.native.finish().map_err(binary_error)?
        };
        spool.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let mut native_hasher = blake3::Hasher::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            resource_checkpoint()?;
            let count = spool.read(&mut buffer).map_err(io_error)?;
            if count == 0 {
                break;
            }
            native_hasher.update(&buffer[..count]);
        }
        let mut header = [0u8; HEADER_BYTES];
        header[..16].copy_from_slice(MAGIC);
        header[16..24].copy_from_slice(&statistics.native_bytes.to_le_bytes());
        header[24..].copy_from_slice(native_hasher.finalize().as_bytes());
        let decoded_bytes = statistics
            .native_bytes
            .checked_add(HEADER_BYTES as u64)
            .ok_or_else(|| fail("decoded cache frame overflow"))?;
        let mut decoded_hasher = blake3::Hasher::new();
        decoded_hasher.update(&header);
        spool.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let mut output = CountedWriter::new(output, self.limits.max_compressed_bytes);
        {
            let mut encoder = GzBuilder::new()
                .mtime(0)
                .operating_system(255)
                .write(&mut output, Compression::best());
            encoder.write_all(&header).map_err(io_error)?;
            loop {
                resource_checkpoint()?;
                let count = spool.read(&mut buffer).map_err(io_error)?;
                if count == 0 {
                    break;
                }
                encoder.write_all(&buffer[..count]).map_err(io_error)?;
                decoded_hasher.update(&buffer[..count]);
            }
            encoder.finish().map_err(io_error)?;
        }
        let identity = SavedApplicationCacheFileIdentity {
            compressed_bytes: output.bytes,
            compressed_blake3: *output.hasher.finalize().as_bytes(),
            decoded_bytes,
            decoded_blake3: *decoded_hasher.finalize().as_bytes(),
        };
        resource_checkpoint()?;
        Ok((output.inner, identity, statistics))
    }
}

impl SavedApplicationCacheFile {
    pub fn open_with_options(
        path: &Path,
        expected: &ApplicationCacheBinding,
        identity: Option<&SavedApplicationCacheFileIdentity>,
        options: &SavedApplicationCacheFileOptions,
    ) -> Result<Self, AppError> {
        resource_checkpoint()?;
        if let Some(identity) = identity {
            validate_identity(identity, options.limits)?;
        }
        if !std::fs::metadata(path).map_err(io_error)?.is_file() {
            return Err(fail("cache path is not a regular file"));
        }
        let file = File::open(path).map_err(io_error)?;
        let metadata = file.metadata().map_err(io_error)?;
        if !metadata.is_file() {
            return Err(fail("cache path is not a regular file"));
        }
        if metadata.len() > options.limits.max_compressed_bytes as u64 {
            return Err(AppError::limit(
                "compressed application cache file exceeds allowance",
            ));
        }
        if identity.is_some_and(|identity| identity.compressed_bytes != metadata.len()) {
            return Err(fail("compressed application cache file length mismatch"));
        }
        Self::decode_reader(file, expected, identity, options)
    }

    pub(super) fn decode_reader(
        encoded: impl Read,
        expected: &ApplicationCacheBinding,
        expected_identity: Option<&SavedApplicationCacheFileIdentity>,
        options: &SavedApplicationCacheFileOptions,
    ) -> Result<Self, AppError> {
        let limits = options.limits;
        if let Some(identity) = expected_identity {
            validate_identity(identity, limits)?;
        }
        let encoded = CountedReader::new(encoded, limits.max_compressed_bytes);
        let mut decoder = GzDecoder::new(BufReader::new(encoded));
        let mut header = [0u8; HEADER_BYTES];
        decoder.read_exact(&mut header).map_err(io_error)?;
        if &header[..16] != MAGIC {
            return Err(fail("application cache file framing version mismatch"));
        }
        let native_length = usize::try_from(u64::from_le_bytes(
            header[16..24].try_into().expect("header"),
        ))
        .map_err(fail)?;
        if native_length > limits.chunk.binary.max_program_bytes {
            return Err(AppError::limit(
                "decoded application cache file exceeds allowance",
            ));
        }
        let decoded_length = native_length
            .checked_add(HEADER_BYTES)
            .ok_or_else(|| fail("decoded cache frame overflow"))?;
        if expected_identity.is_some_and(|identity| identity.decoded_bytes != decoded_length as u64)
        {
            return Err(fail("decoded application cache file length mismatch"));
        }
        let mut native_hasher = blake3::Hasher::new();
        let mut decoded_hasher = blake3::Hasher::new();
        decoded_hasher.update(&header);
        let mut store = DecodedStore::new(native_length, options.decoded_directory.as_deref())?;
        let mut remaining = native_length;
        let mut buffer = [0u8; 64 * 1024];
        while remaining != 0 {
            resource_checkpoint()?;
            let requested = remaining.min(buffer.len());
            let count = decoder.read(&mut buffer[..requested]).map_err(io_error)?;
            if count == 0 {
                return Err(fail("truncated decoded application cache file"));
            }
            store.write_all(&buffer[..count]).map_err(io_error)?;
            native_hasher.update(&buffer[..count]);
            decoded_hasher.update(&buffer[..count]);
            remaining -= count;
        }
        // This forces CRC/trailer validation without allocating overrun bytes.
        if decoder.read(&mut buffer[..1]).map_err(io_error)? != 0 {
            return Err(fail(
                "decoded application cache file exceeds declared length",
            ));
        }
        let mut encoded = decoder.into_inner();
        if encoded.read(&mut buffer[..1]).map_err(io_error)? != 0 {
            return Err(fail(
                "trailing gzip member or bytes in application cache file",
            ));
        }
        let encoded = encoded.into_inner();
        if native_hasher.finalize().as_bytes() != &header[24..] {
            return Err(fail("native application cache chunk digest mismatch"));
        }
        let identity = SavedApplicationCacheFileIdentity {
            compressed_bytes: encoded.bytes,
            compressed_blake3: *encoded.hasher.finalize().as_bytes(),
            decoded_bytes: decoded_length as u64,
            decoded_blake3: *decoded_hasher.finalize().as_bytes(),
        };
        if expected_identity.is_some_and(|expected| *expected != identity) {
            return Err(fail("application cache file identity mismatch"));
        }
        let native_digest = header[24..].try_into().expect("native digest");
        let chunk = store.inspect(expected, limits.chunk, &native_digest)?;
        resource_checkpoint()?;
        Ok(Self {
            binding: expected.clone(),
            identity,
            chunk: Arc::new(chunk),
            hits: 0,
            misses: 0,
        })
    }
}

struct CountedReader<R> {
    inner: R,
    bytes: u64,
    limit: u64,
    hasher: blake3::Hasher,
}
impl<R> CountedReader<R> {
    fn new(inner: R, limit: usize) -> Self {
        Self {
            inner,
            bytes: 0,
            limit: limit as u64,
            hasher: blake3::Hasher::new(),
        }
    }
}
impl<R: Read> Read for CountedReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        resource_checkpoint().map_err(io::Error::other)?;
        let allowed = self.limit.saturating_sub(self.bytes).saturating_add(1);
        let requested = bytes
            .len()
            .min(usize::try_from(allowed).unwrap_or(usize::MAX));
        let count = self.inner.read(&mut bytes[..requested])?;
        self.bytes = self
            .bytes
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("cache byte count overflow"))?;
        if self.bytes > self.limit {
            return Err(io::Error::other(AppError::limit(
                "compressed application cache file exceeds allowance",
            )));
        }
        self.hasher.update(&bytes[..count]);
        Ok(count)
    }
}
struct CountedWriter<W> {
    inner: W,
    bytes: u64,
    limit: u64,
    hasher: blake3::Hasher,
}
impl<W> CountedWriter<W> {
    fn new(inner: W, limit: usize) -> Self {
        Self {
            inner,
            bytes: 0,
            limit: limit as u64,
            hasher: blake3::Hasher::new(),
        }
    }
}
impl<W: Write> Write for CountedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        resource_checkpoint().map_err(io::Error::other)?;
        if (bytes.len() as u64) > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other(AppError::limit(
                "compressed application cache file exceeds allowance",
            )));
        }
        let count = self.inner.write(bytes)?;
        self.hasher.update(&bytes[..count]);
        self.bytes += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

enum DecodedStore {
    Memory(Vec<u8>),
    File(TemporaryFile),
}
impl DecodedStore {
    fn new(length: usize, directory: Option<&Path>) -> Result<Self, AppError> {
        if let Some(directory) = directory {
            return TemporaryFile::new(directory).map(Self::File);
        }

        resource_allocation(length)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(fail)?;
        Ok(Self::Memory(bytes))
    }
    fn inspect(
        self,
        binding: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
        native_digest: &[u8; 32],
    ) -> Result<OwnedApplicationCacheChunk, AppError> {
        match self {
            Self::Memory(bytes) => {
                OwnedApplicationCacheChunk::inspect_owned(bytes, binding, limits)
                    .map_err(binary_error)
            }
            Self::File(mut file) => {
                file.flush().map_err(io_error)?;
                let length = usize::try_from(file.file().metadata().map_err(io_error)?.len())
                    .map_err(fail)?;
                OwnedApplicationCacheChunk::inspect_reader(
                    file,
                    length,
                    native_digest,
                    binding,
                    limits,
                )
                .map_err(binary_error)
            }
        }
    }
}

impl Write for DecodedStore {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Memory(output) => {
                output.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            Self::File(file) => file.write(bytes),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Memory(_) => Ok(()),
            Self::File(file) => file.flush(),
        }
    }
}

struct TemporaryFile {
    file: Option<BufWriter<File>>,
    path: Option<PathBuf>,
}
impl TemporaryFile {
    fn new(directory: &Path) -> Result<Self, AppError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = directory.join(format!(
            ".rustred-native-{}-{}.pending",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        resource_allocation(64 * 1024)?;
        let file = options.open(&path).map_err(io_error)?;
        let mut temporary = Self {
            file: Some(BufWriter::with_capacity(64 * 1024, file)),
            path: Some(path),
        };
        // Keep only the owned descriptor. No decoded or writer-spool pathname
        // remains available for ordinary caller mutation during long operations.
        temporary.unlink()?;
        Ok(temporary)
    }
    fn unlink(&mut self) -> Result<(), AppError> {
        if let Some(path) = self.path.as_ref() {
            std::fs::remove_file(path).map_err(io_error)?;
            self.path = None;
        }
        Ok(())
    }
    fn file(&self) -> &File {
        self.file.as_ref().expect("owned temporary file").get_ref()
    }
    fn file_mut(&mut self) -> &mut BufWriter<File> {
        self.file.as_mut().expect("owned temporary file")
    }
}
impl Read for TemporaryFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let file = self.file_mut();
        file.flush()?;
        file.get_mut().read(out)
    }
}
impl Write for TemporaryFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file_mut().write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file_mut().flush()
    }
}
impl Seek for TemporaryFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file_mut().seek(pos)
    }
}
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        drop(self.file.take());
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffered_spool_flushes_on_write_read_and_seek_boundaries() {
        let scratch = super::super::super::tests::Scratch::new();
        let mut spool = TemporaryFile::new(&scratch.0).unwrap();
        assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 0);
        spool.write_all(b"header").unwrap();
        assert_eq!(
            spool.file().metadata().unwrap().len(),
            0,
            "small fields stay buffered"
        );
        assert_eq!(spool.stream_position().unwrap(), 6);
        spool.seek(SeekFrom::Start(0)).unwrap();
        let mut prefix = [0; 3];
        spool.read_exact(&mut prefix).unwrap();
        assert_eq!(&prefix, b"hea");
        spool.write_all(b"XYZ").unwrap();
        spool.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        spool.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"heaXYZ");
        spool.write_all(b"tail").unwrap();
        spool.flush().unwrap();
        assert_eq!(spool.file().metadata().unwrap().len(), 10);
        drop(spool);
        assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[test]
    fn buffered_spool_preserves_payloads_larger_than_its_buffer() {
        let scratch = super::super::super::tests::Scratch::new();
        let mut spool = TemporaryFile::new(&scratch.0).unwrap();
        let expected = (0..150_000).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        for field in expected.chunks(7) {
            spool.write_all(field).unwrap();
        }
        spool.seek(SeekFrom::Start(0)).unwrap();
        let mut actual = Vec::new();
        spool.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            spool.file().metadata().unwrap().len(),
            expected.len() as u64
        );
        drop(spool);
        assert_eq!(std::fs::read_dir(&scratch.0).unwrap().count(), 0);
    }
}
