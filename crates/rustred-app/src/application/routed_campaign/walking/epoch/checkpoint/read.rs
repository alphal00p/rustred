//! Fixed-scratch authenticated section reads for private restore assembly.
//! A successful decode is provisional until `finish` verifies the entire
//! length/digest. No worker may observe a partially decoded section.
use super::{BUFFER_BYTES, invalid};
use std::cell::Cell;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path};

/// A serde value budget, reset only at a caller-known schema boundary.
/// Large diagnostic values may use an unlimited budget with IgnoredAny;
/// known bounded fields cannot borrow another value's byte allowance.
pub(super) struct Budget<'a> {
    pub input: &'a mut CheckedRead,
    pub remaining: &'a Cell<u64>,
    pub reason: &'static str,
}

impl Read for Budget<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let remaining = self.remaining.get();
        if remaining == 0 {
            return Err(invalid(self.reason));
        }
        let end = bytes
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let read = self.input.read(&mut bytes[..end])?;
        self.remaining.set(remaining - read as u64);
        Ok(read)
    }
}

pub(super) struct CheckedRead {
    file: File,
    buffer: [u8; BUFFER_BYTES],
    at: usize,
    end: usize,
    remaining: u64,
    hash: blake3::Hasher,
    expected: [u8; 32],
}

impl CheckedRead {
    pub fn open(directory: &Path, name: &str, bytes: u64, expected: [u8; 32]) -> io::Result<Self> {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(invalid("epoch section path is not one local filename"));
        }
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != bytes {
            return Err(invalid("epoch section type or length differs"));
        }
        let file = File::open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || opened.len() != bytes {
            return Err(invalid("epoch section changed before open"));
        }
        Ok(Self {
            file,
            buffer: [0; BUFFER_BYTES],
            at: 0,
            end: 0,
            remaining: bytes,
            hash: blake3::Hasher::new(),
            expected,
        })
    }

    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    pub fn u8(&mut self) -> io::Result<u8> {
        let mut bytes = [0; 1];
        self.read_exact(&mut bytes)?;
        Ok(bytes[0])
    }
    pub fn u32(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 4];
        self.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    pub fn u64(&mut self) -> io::Result<u64> {
        let mut bytes = [0; 8];
        self.read_exact(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub fn finish(mut self) -> io::Result<()> {
        if self.remaining != 0 || self.at != self.end {
            return Err(invalid("epoch section was not completely decoded"));
        }
        let mut extra = [0; 1];
        if self.file.read(&mut extra)? != 0 || self.hash.finalize().as_bytes() != &self.expected {
            return Err(invalid("epoch section trailing bytes or digest differs"));
        }
        Ok(())
    }
}

impl Read for CheckedRead {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        if self.at == self.end {
            let want = self.remaining.min(BUFFER_BYTES as u64) as usize;
            self.end = self.file.read(&mut self.buffer[..want])?;
            self.at = 0;
            if self.end == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            // Hash each file chunk once, not each scalar word decoded from it.
            self.hash.update(&self.buffer[..self.end]);
        }
        let n = out.len().min(self.end - self.at);
        out[..n].copy_from_slice(&self.buffer[self.at..self.at + n]);
        self.at += n;
        self.remaining -= n as u64;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Directory;
    use super::*;
    use std::io::Write;

    #[test]
    fn authenticated_read_is_chunk_bounded_and_requires_complete_consumption() {
        let directory = Directory::new();
        let bytes = vec![0x39; BUFFER_BYTES * 3 + 7];
        fs::write(directory.0.join("section"), &bytes).unwrap();
        let expected = *blake3::hash(&bytes).as_bytes();
        let mut reader =
            CheckedRead::open(&directory.0, "section", bytes.len() as u64, expected).unwrap();
        assert_eq!(reader.u64().unwrap(), 0x3939393939393939);
        assert!(reader.finish().is_err());
        let mut reader =
            CheckedRead::open(&directory.0, "section", bytes.len() as u64, expected).unwrap();
        let mut copied = Vec::new();
        reader.read_to_end(&mut copied).unwrap();
        assert_eq!(reader.remaining(), 0);
        reader.finish().unwrap();
        assert_eq!(copied, bytes);
    }

    #[test]
    fn section_length_digest_growth_and_local_path_mutations_fail() {
        let directory = Directory::new();
        let path = directory.0.join("section");
        fs::write(&path, b"abcdefgh").unwrap();
        let digest = *blake3::hash(b"abcdefgh").as_bytes();
        for name in ["../section", "/section", "", ".", "sub/section"] {
            assert!(CheckedRead::open(&directory.0, name, 8, digest).is_err());
        }
        assert!(CheckedRead::open(&directory.0, "section", 7, digest).is_err());
        let mut reader = CheckedRead::open(&directory.0, "section", 8, [0; 32]).unwrap();
        reader.u64().unwrap();
        assert!(reader.finish().is_err());
        let mut reader = CheckedRead::open(&directory.0, "section", 8, digest).unwrap();
        reader.u64().unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"!")
            .unwrap();
        assert!(reader.finish().is_err());
    }
}
