//! Shared byte ranges for the existing lazy native coefficient table.
use std::{
    borrow::Cow,
    io::{Read, Seek, SeekFrom},
    ops::Range,
    sync::{Arc, Mutex},
};

use super::{BinaryIoError, native_io_error, resource_allocation};

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

enum Bytes {
    Memory(Vec<u8>),
    Shared(Arc<[u8]>),
    Reader(Mutex<Reader>),
}

const BLOCK_BYTES: usize = 64 * 1024;

struct Reader {
    input: Box<dyn ReadSeek>,
    hashes: Vec<[u8; 32]>,
    length: usize,
    block: Vec<u8>,
    loaded: Option<usize>,
}

impl Reader {
    fn new(
        mut input: impl Read + Seek + Send + 'static,
        length: usize,
        digest: &[u8; 32],
        max_index_bytes: usize,
    ) -> Result<Self, BinaryIoError> {
        let actual = input.seek(SeekFrom::End(0)).map_err(native_io_error)?;
        if actual != length as u64 {
            return Err(BinaryIoError::Invalid(
                "native backing file length mismatch",
            ));
        }
        input.seek(SeekFrom::Start(0)).map_err(native_io_error)?;
        let count = length.div_ceil(BLOCK_BYTES);
        let index_bytes = count.checked_mul(32).ok_or(BinaryIoError::Invalid(
            "native backing digest index overflow",
        ))?;
        super::error::check_limit(
            "native backing digest index bytes",
            index_bytes,
            max_index_bytes,
        )?;
        super::resource_array::<[u8; 32]>(count)?;
        let mut hashes = Vec::new();
        hashes
            .try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "native backing digest index",
                requested: index_bytes,
            })?;
        resource_allocation(BLOCK_BYTES)?;
        let mut block = Vec::new();
        block
            .try_reserve_exact(BLOCK_BYTES)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "native backing verification block",
                requested: BLOCK_BYTES,
            })?;
        let mut hasher = blake3::Hasher::new();
        let mut remaining = length;
        while remaining != 0 {
            super::resource_checkpoint()?;
            let count = remaining.min(BLOCK_BYTES);
            block.resize(count, 0);
            input.read_exact(&mut block).map_err(native_io_error)?;
            hashes.push(*blake3::hash(&block).as_bytes());
            hasher.update(&block);
            remaining -= count;
        }
        let mut probe = [0; 1];
        if input.read(&mut probe).map_err(native_io_error)? != 0
            || hasher.finalize().as_bytes() != digest
        {
            return Err(BinaryIoError::Invalid(
                "native backing file digest mismatch",
            ));
        }
        block.clear();
        Ok(Self {
            input: Box::new(input),
            hashes,
            length,
            block,
            loaded: None,
        })
    }

    fn read_range(
        &mut self,
        range: Range<usize>,
        output: &mut Vec<u8>,
    ) -> Result<(), BinaryIoError> {
        let mut position = range.start;
        while position < range.end {
            super::resource_checkpoint()?;
            let index = position / BLOCK_BYTES;
            if self.loaded != Some(index) {
                self.loaded = None;
                let start = index * BLOCK_BYTES;
                let count = (self.length - start).min(BLOCK_BYTES);
                self.block.resize(count, 0);
                self.input
                    .seek(SeekFrom::Start(start as u64))
                    .map_err(native_io_error)?;
                self.input
                    .read_exact(&mut self.block)
                    .map_err(native_io_error)?;
                if blake3::hash(&self.block).as_bytes() != &self.hashes[index] {
                    return Err(BinaryIoError::Invalid(
                        "native backing bytes changed after verification",
                    ));
                }
                self.loaded = Some(index);
            }
            let start = position % BLOCK_BYTES;
            let count = (range.end - position).min(self.block.len() - start);
            output.extend_from_slice(&self.block[start..start + count]);
            position += count;
        }
        Ok(())
    }
}

/// Storage is private and owned. Reading a file range allocates only that range;
/// memory ranges borrow the original Vec/Arc without copying its payload.
#[derive(Clone)]
pub(crate) struct SharedCoefficientBytes {
    owner: Arc<Bytes>,
    range: Range<usize>,
}

impl SharedCoefficientBytes {
    pub(crate) fn from_arc(bytes: Arc<[u8]>) -> Self {
        let len = bytes.len();
        Self {
            owner: Arc::new(Bytes::Shared(bytes)),
            range: 0..len,
        }
    }

    pub(crate) fn from_vec(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        Self {
            owner: Arc::new(Bytes::Memory(bytes)),
            range: 0..len,
        }
    }

    pub(crate) fn from_reader(
        reader: impl Read + Seek + Send + 'static,
        len: usize,
        digest: &[u8; 32],
        max_index_bytes: usize,
    ) -> Result<Self, BinaryIoError> {
        Ok(Self {
            owner: Arc::new(Bytes::Reader(Mutex::new(Reader::new(
                reader,
                len,
                digest,
                max_index_bytes,
            )?))),
            range: 0..len,
        })
    }

    pub(crate) fn index_owned_bytes(&self) -> Result<usize, BinaryIoError> {
        match self.owner.as_ref() {
            Bytes::Reader(reader) => reader
                .lock()
                .map_err(|_| BinaryIoError::Native("native byte-reader lock poisoned".into()))?
                .hashes
                .capacity()
                .checked_mul(32)
                .ok_or(BinaryIoError::Invalid(
                    "native backing digest index overflow",
                )),
            _ => Ok(0),
        }
    }

    pub(crate) fn slice(&self, range: Range<usize>) -> Result<Self, BinaryIoError> {
        if range.start > range.end || range.end > self.len() {
            return Err(BinaryIoError::Invalid(
                "native byte range is outside its owner",
            ));
        }
        Ok(Self {
            owner: self.owner.clone(),
            range: self.range.start + range.start..self.range.start + range.end,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.range.len()
    }

    pub(crate) fn read(&self) -> Result<Cow<'_, [u8]>, BinaryIoError> {
        match self.owner.as_ref() {
            Bytes::Memory(bytes) => Ok(Cow::Borrowed(&bytes[self.range.clone()])),
            Bytes::Shared(bytes) => Ok(Cow::Borrowed(&bytes[self.range.clone()])),
            Bytes::Reader(reader) => {
                resource_allocation(self.len())?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(self.len())
                    .map_err(|_| BinaryIoError::Allocation {
                        resource: "native selected byte range",
                        requested: self.len(),
                    })?;
                let mut reader = reader.lock().map_err(|_| {
                    BinaryIoError::Native("native byte-reader lock poisoned".into())
                })?;
                reader.read_range(self.range.clone(), &mut bytes)?;
                Ok(Cow::Owned(bytes))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum CursorSource<'a> {
    Borrowed(&'a [u8]),
    Shared(&'a SharedCoefficientBytes),
}

pub(crate) struct ByteCursor<'a> {
    source: CursorSource<'a>,
    position: usize,
}

impl<'a> ByteCursor<'a> {
    pub(crate) fn borrowed(bytes: &'a [u8]) -> Self {
        Self {
            source: CursorSource::Borrowed(bytes),
            position: 0,
        }
    }
    pub(crate) fn shared(bytes: &'a SharedCoefficientBytes) -> Self {
        Self {
            source: CursorSource::Shared(bytes),
            position: 0,
        }
    }
    pub(crate) fn position(&self) -> usize {
        self.position
    }
    pub(crate) fn remaining(&self) -> usize {
        (match &self.source {
            CursorSource::Borrowed(bytes) => bytes.len(),
            CursorSource::Shared(bytes) => bytes.len(),
        }) - self.position
    }
    pub(crate) fn skip(&mut self, count: usize) -> Result<Range<usize>, BinaryIoError> {
        if count > self.remaining() {
            return Err(BinaryIoError::Invalid("truncated native byte range"));
        }
        let start = self.position;
        self.position += count;
        Ok(start..self.position)
    }
    pub(crate) fn take(&mut self, count: usize) -> Result<Cow<'a, [u8]>, BinaryIoError> {
        let range = self.skip(count)?;
        match self.source {
            CursorSource::Borrowed(bytes) => Ok(Cow::Borrowed(&bytes[range])),
            CursorSource::Shared(bytes) => {
                // The range owner remains borrowed by this cursor. File data
                // is returned owned; memory keeps the original source lifetime.
                match bytes.owner.as_ref() {
                    Bytes::Memory(data) => Ok(Cow::Borrowed(
                        &data[bytes.range.start + range.start..bytes.range.start + range.end],
                    )),
                    Bytes::Shared(data) => Ok(Cow::Borrowed(
                        &data[bytes.range.start + range.start..bytes.range.start + range.end],
                    )),
                    Bytes::Reader(_) => Ok(Cow::Owned(bytes.slice(range)?.read()?.into_owned())),
                }
            }
        }
    }
    pub(crate) fn length(&mut self) -> Result<usize, BinaryIoError> {
        super::resource_checkpoint()?;
        let bytes = self.take(8)?;
        usize::try_from(u64::from_le_bytes(
            bytes.as_ref().try_into().expect("length bytes"),
        ))
        .map_err(|_| BinaryIoError::Invalid("native length exceeds address space"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[derive(Clone)]
    struct MutableInput(Arc<Mutex<Cursor<Vec<u8>>>>);
    impl Read for MutableInput {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            assert!(
                out.len() <= BLOCK_BYTES,
                "native file verification must use bounded blocks"
            );
            self.0.lock().unwrap().read(out)
        }
    }
    impl Seek for MutableInput {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.0.lock().unwrap().seek(position)
        }
    }

    #[test]
    fn owned_memory_ranges_borrow_without_copy_and_check_boundaries() {
        let bytes = vec![1, 2, 3, 4];
        let pointer = bytes.as_ptr();
        let source = SharedCoefficientBytes::from_vec(bytes);
        let range = source.slice(1..3).unwrap();
        let selected = range.read().unwrap();
        assert!(matches!(selected, Cow::Borrowed(_)));
        assert_eq!(selected.as_ptr(), pointer.wrapping_add(1));
        assert_eq!(selected.as_ref(), &[2, 3]);
        assert!(source.slice(0..usize::MAX).is_err());
        assert!(source.slice(std::ops::Range { start: 3, end: 2 }).is_err());
        assert_eq!(source.index_owned_bytes().unwrap(), 0);
    }

    #[test]
    fn file_ranges_authenticate_blocks_share_owner_and_detect_mutation() {
        let bytes = (0..BLOCK_BYTES * 3 + 17)
            .map(|i| (i % 251) as u8)
            .collect::<Vec<_>>();
        let digest = *blake3::hash(&bytes).as_bytes();
        let input = MutableInput(Arc::new(Mutex::new(Cursor::new(bytes.clone()))));
        let owner =
            SharedCoefficientBytes::from_reader(input.clone(), bytes.len(), &digest, usize::MAX)
                .unwrap();
        assert_eq!(owner.index_owned_bytes().unwrap(), 4 * 32);
        let crossing = owner.slice(BLOCK_BYTES - 7..BLOCK_BYTES + 9).unwrap();
        assert_eq!(
            crossing.read().unwrap().as_ref(),
            &bytes[BLOCK_BYTES - 7..BLOCK_BYTES + 9]
        );
        let fork = owner.clone();
        let first = fork.slice(0..3).unwrap();
        assert_eq!(first.read().unwrap().as_ref(), &bytes[..3]);
        // Cached bytes still represent the authenticated immutable snapshot.
        input.0.lock().unwrap().get_mut()[0] ^= 1;
        assert_eq!(first.read().unwrap().as_ref(), &bytes[..3]);
        let other = owner.slice(BLOCK_BYTES * 2..BLOCK_BYTES * 2 + 1).unwrap();
        other.read().unwrap();
        assert!(
            first.read().is_err(),
            "a changed backing block must never supply different coefficients"
        );
        input.0.lock().unwrap().get_mut().truncate(BLOCK_BYTES);
        let last = owner.slice(BLOCK_BYTES * 3..bytes.len()).unwrap();
        assert!(last.read().is_err());
        drop(owner);
        assert_eq!(fork.len(), bytes.len());
    }

    #[test]
    fn file_source_admits_digest_index_and_authenticates_before_use() {
        let bytes = vec![7; BLOCK_BYTES + 1];
        let digest = *blake3::hash(&bytes).as_bytes();
        assert!(matches!(
            SharedCoefficientBytes::from_reader(
                Cursor::new(bytes.clone()),
                bytes.len(),
                &digest,
                63
            ),
            Err(BinaryIoError::Limit { .. })
        ));
        assert!(
            SharedCoefficientBytes::from_reader(
                Cursor::new(bytes.clone()),
                bytes.len(),
                &[0; 32],
                64
            )
            .is_err()
        );
        assert!(
            SharedCoefficientBytes::from_reader(
                Cursor::new(bytes.clone()),
                bytes.len() + 1,
                &digest,
                64
            )
            .is_err()
        );
        let source = SharedCoefficientBytes::from_reader(
            Cursor::new(bytes.clone()),
            bytes.len(),
            &digest,
            64,
        )
        .unwrap();
        assert_eq!(
            source
                .slice(bytes.len() - 1..bytes.len())
                .unwrap()
                .read()
                .unwrap()
                .as_ref(),
            &[7]
        );
    }
}
