//! Standard gzip transport of unchanged native payload bytes, in bounded files.
//! Decompression feeds the existing owner and terminal-map codecs directly.
use super::package::{Payload, authenticate, read_bounded, safe_child};
use super::*;
use crate::application::atomic_file::write_file_atomically;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, Read, Write};

const CHUNK_BYTES: usize = 32 * 1024 * 1024;
const ENCODING: &str = "gzip-chunks-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Chunk {
    path: String,
    bytes: u64,
    blake3: String,
}

pub(super) fn validate(payload: &Payload) -> Result<(), AppError> {
    safe_child(Path::new("."), &payload.path)?;
    match payload.encoding.as_deref() {
        None if payload.chunks.is_empty() => Ok(()),
        Some(ENCODING) if !payload.chunks.is_empty() => {
            let mut paths = BTreeSet::new();
            for chunk in &payload.chunks {
                safe_child(Path::new("."), &chunk.path)?;
                if chunk.bytes == 0
                    || chunk.bytes > CHUNK_BYTES as u64
                    || !paths.insert(&chunk.path)
                    || chunk.blake3.len() != 64
                    || !chunk.blake3.bytes().all(|c| c.is_ascii_hexdigit())
                {
                    return Err(AppError::input("invalid compressed owner chunk"));
                }
            }
            Ok(())
        }
        _ => Err(AppError::input("unsupported owner payload transport")),
    }
}

fn read_chunk(directory: &Path, chunk: &Chunk) -> Result<Vec<u8>, AppError> {
    let bytes = read_bounded(&safe_child(directory, &chunk.path)?, CHUNK_BYTES)?;
    if bytes.len() as u64 != chunk.bytes {
        return Err(AppError::input("compressed owner chunk size differs"));
    }
    authenticate(&bytes, &chunk.blake3)?;
    Ok(bytes)
}

struct ChunkReader<'a> {
    directory: &'a Path,
    chunks: std::slice::Iter<'a, Chunk>,
    current: std::io::Cursor<Vec<u8>>,
}
impl Read for ChunkReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let count = self.current.read(output)?;
            if count != 0 {
                return Ok(count);
            }
            let Some(chunk) = self.chunks.next() else {
                return Ok(0);
            };
            self.current =
                io::Cursor::new(read_chunk(self.directory, chunk).map_err(io::Error::other)?);
        }
    }
}

pub(super) fn decode(directory: &Path, payload: &Payload) -> Result<Vec<u8>, AppError> {
    validate(payload)?;
    let limit = usize::try_from(payload.bytes)
        .map_err(|_| AppError::limit("owner byte count exceeds address space"))?;
    if payload.encoding.is_none() {
        let bytes = read_bounded(&safe_child(directory, &payload.path)?, limit)?;
        if bytes.len() != limit {
            return Err(AppError::input("owner byte count differs"));
        }
        return Ok(bytes);
    }
    let reader = ChunkReader {
        directory,
        chunks: payload.chunks.iter(),
        current: io::Cursor::new(Vec::new()),
    };
    let mut decoder = flate2::bufread::GzDecoder::new(BufReader::new(reader));
    let mut bytes = Vec::new();
    decoder
        .by_ref()
        .take(
            payload
                .bytes
                .checked_add(1)
                .ok_or_else(|| AppError::limit("decoded owner limit overflow"))?,
        )
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() != limit {
        return Err(AppError::input(
            "decoded owner size differs from native manifest",
        ));
    }
    // Force the gzip trailer/CRC and reject concatenated streams or trailing
    // bytes. The owner loader subsequently hashes these decoded native bytes.
    let mut extra = [0u8; 1];
    if decoder.read(&mut extra).map_err(failure)? != 0
        || decoder.into_inner().read(&mut extra).map_err(failure)? != 0
    {
        return Err(AppError::input(
            "trailing bytes in compressed owner payload",
        ));
    }
    Ok(bytes)
}

struct ChunkWriter<'a> {
    directory: &'a Path,
    digest: &'a str,
    capacity: usize,
    buffer: Vec<u8>,
    chunks: Vec<Chunk>,
}
impl ChunkWriter<'_> {
    fn emit(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let path = format!(
            "objects/{}.{:05}.rrbin.gz.part",
            self.digest,
            self.chunks.len()
        );
        write_file_atomically(&self.directory.join(&path), &self.buffer, false)
            .map_err(io::Error::other)?;
        self.chunks.push(Chunk {
            path,
            bytes: self.buffer.len() as u64,
            blake3: blake3::hash(&self.buffer).to_hex().to_string(),
        });
        self.buffer.clear();
        Ok(())
    }
}
impl Write for ChunkWriter<'_> {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let total = bytes.len();
        while !bytes.is_empty() {
            let count = bytes.len().min(self.capacity - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.buffer.len() == self.capacity {
                self.emit()?;
            }
        }
        Ok(total)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn export(source: &Path, output: &Path, payload: &Payload) -> Result<Payload, AppError> {
    validate(payload)?;
    if payload.encoding.is_some() {
        for chunk in &payload.chunks {
            let bytes = read_chunk(source, chunk)?;
            write_file_atomically(&safe_child(output, &chunk.path)?, &bytes, false)
                .map_err(failure)?;
        }
        return Ok(payload.clone());
    }
    let mut input = File::open(safe_child(source, &payload.path)?).map_err(failure)?;
    compress(&mut input, output, payload, CHUNK_BYTES)
}

/// Package an existing native encoding without changing its bytes or identity.
/// `path` remains the logical payload name; only its compressed chunks are
/// written. Chunk size bounds individual transport files, not native data.
pub(super) fn export_bytes(bytes: &[u8], output: &Path, path: &str) -> Result<Payload, AppError> {
    let payload = Payload {
        path: path.to_owned(),
        bytes: bytes.len() as u64,
        blake3: blake3::hash(bytes).to_hex().to_string(),
        encoding: None,
        chunks: vec![],
    };
    validate(&payload)?;
    let mut input = bytes;
    compress(&mut input, output, &payload, CHUNK_BYTES)
}

fn compress(
    input: &mut impl Read,
    output: &Path,
    payload: &Payload,
    capacity: usize,
) -> Result<Payload, AppError> {
    let writer = ChunkWriter {
        directory: output,
        digest: &payload.blake3,
        capacity,
        buffer: Vec::new(),
        chunks: Vec::new(),
    };
    let mut encoder = flate2::write::GzEncoder::new(writer, flate2::Compression::fast());
    let mut hash = blake3::Hasher::new();
    let mut count = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let n = input.read(&mut buffer).map_err(failure)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or_else(|| AppError::limit("owner export byte count overflow"))?;
        if count > payload.bytes {
            return Err(AppError::input("owner grew after preparation"));
        }
        hash.update(&buffer[..n]);
        encoder.write_all(&buffer[..n]).map_err(failure)?;
    }
    if count != payload.bytes || hash.finalize().to_hex().as_str() != payload.blake3 {
        return Err(AppError::input("native owner changed after preparation"));
    }
    let mut writer = encoder.finish().map_err(failure)?;
    writer.emit().map_err(failure)?;
    let mut result = payload.clone();
    result.encoding = Some(ENCODING.to_owned());
    result.chunks = writer.chunks;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gzip_chunk_boundary_corruption_truncation_and_decoded_size_are_checked() {
        let scratch = super::super::tests::Scratch::new();
        std::fs::create_dir(scratch.0.join("objects")).unwrap();
        let bytes: Vec<u8> = (0..65536)
            .map(|i| ((i * 67 + i / 13) % 256) as u8)
            .collect();
        let raw = Payload {
            path: "objects/native.rrbin".into(),
            bytes: bytes.len() as u64,
            blake3: blake3::hash(&bytes).to_hex().to_string(),
            encoding: None,
            chunks: vec![],
        };
        let packed = compress(&mut bytes.as_slice(), &scratch.0, &raw, 64).unwrap();
        assert!(packed.chunks.len() > 1);
        assert_eq!(decode(&scratch.0, &packed).unwrap(), bytes);
        let mut short = packed.clone();
        short.chunks.pop();
        assert!(decode(&scratch.0, &short).is_err());
        let mut oversized = packed.clone();
        oversized.bytes -= 1;
        assert!(decode(&scratch.0, &oversized).is_err());
        std::fs::write(scratch.0.join(&packed.chunks[0].path), b"corrupt").unwrap();
        assert!(decode(&scratch.0, &packed).is_err());
    }

    #[test]
    fn compressed_transport_rejects_missing_chunks_paths_and_trailing_streams() {
        let scratch = super::super::tests::Scratch::new();
        std::fs::create_dir(scratch.0.join("objects")).unwrap();
        let bytes = b"unchanged terminal-map bytes";
        let packed = export_bytes(bytes, &scratch.0, "terminal-maps.rrbin").unwrap();
        assert!(!scratch.0.join("terminal-maps.rrbin").exists());

        let mut invalid = packed.clone();
        invalid.chunks[0].path = "objects/missing.rrbin.gz.part".into();
        assert!(decode(&scratch.0, &invalid).is_err());
        invalid.chunks[0].path = "../outside".into();
        assert!(validate(&invalid).is_err());
        invalid = packed.clone();
        invalid.path = "/absolute".into();
        assert!(validate(&invalid).is_err());
        invalid = packed.clone();
        invalid.encoding = Some("unknown".into());
        assert!(validate(&invalid).is_err());
        invalid = packed.clone();
        invalid.chunks.push(invalid.chunks[0].clone());
        assert!(validate(&invalid).is_err());

        let path = scratch.0.join(&packed.chunks[0].path);
        let compressed = std::fs::read(&path).unwrap();
        for suffix in [b"trailing bytes".as_slice(), compressed.as_slice()] {
            let mut extended = compressed.clone();
            extended.extend_from_slice(suffix);
            std::fs::write(&path, &extended).unwrap();
            let mut trailing = packed.clone();
            trailing.chunks[0].bytes = extended.len() as u64;
            trailing.chunks[0].blake3 = blake3::hash(&extended).to_hex().to_string();
            assert!(decode(&scratch.0, &trailing).is_err());
        }

        let truncated = &compressed[..compressed.len() - 4];
        std::fs::write(&path, truncated).unwrap();
        let mut short = packed;
        short.chunks[0].bytes = truncated.len() as u64;
        short.chunks[0].blake3 = blake3::hash(truncated).to_hex().to_string();
        assert!(decode(&scratch.0, &short).is_err());
    }
}
