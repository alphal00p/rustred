//! File-backed [`ProbeLog`]: one append-only file per prime.
//!
//! Layout of `<dir>/probes-<key: 16 bytes hex>-<prime>.bin` (format 2):
//! - header (64 bytes): magic `RRPROBE2`, `u64` outputs, `u64` prime, the
//!   32-byte key, and the first 8 bytes of the blake3 hash of the preceding
//!   56 bytes;
//! - records: `u32` payload length, the payload (`u64` x, `u8` status: 1 =
//!   `outputs` `u64` values follow, 0 = failed point), and the first 8 bytes
//!   of the blake3 hash of the payload. All integers little-endian.
//!
//! On load every record is checked: its length matches its status, the
//! checksum matches, `0 < x < prime` and every value is `< prime`. An
//! incomplete or bad LAST record is a torn write (an interrupted append): it
//! is ignored and cut before the next append, so new records always start on
//! a record boundary. A bad record that is followed by others is corruption:
//! the file is refused (`InvalidData`), never read past. A header with
//! another key, output count or prime is an error too: the file belongs to
//! another black box or prime.
//!
//! One writer per directory and key: [`FileProbeLog::new`] takes an exclusive
//! lock on `<dir>/probes-<key>.lock` for the lifetime of the log, so a second
//! process (or a second log in the same process) fails instead of
//! interleaving appends.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::driver::ProbeLog;

const MAGIC: &[u8; 8] = b"RRPROBE2";
const HEADER: usize = 64;
const CHECKSUM: usize = 8;

/// A record: the point and its values (`None` for a failed point).
type Record = (u64, Option<Vec<u64>>);

fn checksum(bytes: &[u8]) -> [u8; CHECKSUM] {
    let hash = blake3::hash(bytes);
    hash.as_bytes()[..CHECKSUM].try_into().expect("8 bytes")
}

fn invalid(path: &Path, message: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: {message}", path.display()),
    )
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"))
}

pub struct FileProbeLog {
    dir: PathBuf,
    key: [u8; 32],
    outputs: usize,
    /// Held for the lifetime of the log: one writer per directory and key.
    _lock: File,
    /// Length of the complete-record prefix of each prime's file, as last
    /// read or written, so an append re-reads the file only when it changed.
    complete: HashMap<u64, u64>,
    /// Records loaded, for reports.
    pub loaded: usize,
    pub written: usize,
}

impl FileProbeLog {
    /// `key` binds the log to one black box (system, request, settings).
    /// Fails with `WouldBlock` when another log holds the directory's lock
    /// for this key.
    pub fn new(dir: impl AsRef<Path>, key: [u8; 32], outputs: usize) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(format!("probes-{}.lock", Self::key_name(&key))))?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                format!("{} is in use by another probe log", dir.display()),
            ),
            std::fs::TryLockError::Error(error) => error,
        })?;
        Ok(Self {
            dir,
            key,
            outputs,
            _lock: lock,
            complete: HashMap::new(),
            loaded: 0,
            written: 0,
        })
    }

    fn key_name(key: &[u8; 32]) -> String {
        key[..16].iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn path(&self, prime: u64) -> PathBuf {
        self.dir
            .join(format!("probes-{}-{prime}.bin", Self::key_name(&self.key)))
    }

    fn header(&self, prime: u64) -> Vec<u8> {
        let mut header = Vec::with_capacity(HEADER);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&(self.outputs as u64).to_le_bytes());
        header.extend_from_slice(&prime.to_le_bytes());
        header.extend_from_slice(&self.key);
        let sum = checksum(&header);
        header.extend_from_slice(&sum);
        header
    }

    /// The complete records and the length in bytes of the file prefix they
    /// occupy (header included; 0 when there is no file or only a torn
    /// header).
    fn read_complete(&self, prime: u64) -> io::Result<(Vec<Record>, u64)> {
        let path = self.path(prime);
        let mut bytes = Vec::new();
        match File::open(&path) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(error) => return Err(error),
        }
        if bytes.len() < HEADER {
            // A torn header: the first append rewrites the file.
            return Ok((Vec::new(), 0));
        }
        if bytes[..HEADER] != self.header(prime)[..] {
            return Err(invalid(
                &path,
                "header of another black box, output count or prime, or corrupted",
            ));
        }
        let mut records = Vec::new();
        let mut at = HEADER;
        // A torn append only truncates: its length prefix, once complete, is
        // intact. So an impossible length is corruption wherever it is; an
        // incomplete record, or a bad checksum on the very last record, is a
        // torn write.
        while at < bytes.len() {
            let rest = &bytes[at..];
            if rest.len() < 4 {
                break;
            }
            let length = Self::declared(rest);
            if length != 9 && length != 9 + 8 * self.outputs {
                return Err(invalid(
                    &path,
                    format!(
                        "corrupted record {} at byte {at}: length {length}",
                        records.len()
                    ),
                ));
            }
            let total = 4 + length + CHECKSUM;
            if rest.len() < total {
                break;
            }
            match self.parse_record(&rest[..total], prime) {
                Ok(record) => {
                    records.push(record);
                    at += total;
                }
                Err(_) if at + total == bytes.len() => break,
                Err(message) => {
                    return Err(invalid(
                        &path,
                        format!("corrupted record {} at byte {at}: {message}", records.len()),
                    ));
                }
            }
        }
        Ok((records, at as u64))
    }

    /// Declared payload length of the record at the start of `bytes` (0 if
    /// the length field itself is incomplete).
    fn declared(bytes: &[u8]) -> usize {
        bytes.get(..4).map_or(0, |b| {
            u32::from_le_bytes(b.try_into().expect("4 bytes")) as usize
        })
    }

    /// One complete record of a valid length (`bytes` is exactly the
    /// record); `Err` when its checksum, point, status or values are bad.
    fn parse_record(&self, bytes: &[u8], prime: u64) -> Result<Record, String> {
        let length = Self::declared(bytes);
        let payload = &bytes[4..4 + length];
        if bytes[4 + length..] != checksum(payload) {
            return Err("checksum mismatch".into());
        }
        let x = u64_at(payload, 0);
        if x == 0 || x >= prime {
            return Err(format!("point {x} outside 1..{prime}"));
        }
        let values = match payload[8] {
            0 if length == 9 => None,
            1 if length == 9 + 8 * self.outputs => {
                let values: Vec<u64> = (0..self.outputs)
                    .map(|index| u64_at(payload, 9 + 8 * index))
                    .collect();
                if let Some(value) = values.iter().find(|&&value| value >= prime) {
                    return Err(format!("value {value} not below {prime}"));
                }
                Some(values)
            }
            status => return Err(format!("status {status} with a {length}-byte payload")),
        };
        Ok((x, values))
    }

    fn encode(&self, prime: u64, probes: &[Record], bytes: &mut Vec<u8>) -> io::Result<()> {
        for (x, values) in probes {
            if *x == 0 || *x >= prime {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("point {x} outside 1..{prime}"),
                ));
            }
            let mut payload = Vec::with_capacity(9 + 8 * self.outputs);
            payload.extend_from_slice(&x.to_le_bytes());
            match values {
                Some(values) => {
                    if values.len() != self.outputs || values.iter().any(|&value| value >= prime) {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "wrong output count or a value not reduced modulo the prime",
                        ));
                    }
                    payload.push(1);
                    for value in values {
                        payload.extend_from_slice(&value.to_le_bytes());
                    }
                }
                None => payload.push(0),
            }
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&payload);
            bytes.extend_from_slice(&checksum(&payload));
        }
        Ok(())
    }
}

impl ProbeLog for FileProbeLog {
    fn load(&mut self, prime: u64) -> io::Result<Vec<Record>> {
        match self.read_complete(prime) {
            Ok((records, complete)) => {
                self.complete.insert(prime, complete);
                self.loaded += records.len();
                Ok(records)
            }
            Err(error) => {
                // Never append to a file that failed validation.
                self.complete.remove(&prime);
                Err(error)
            }
        }
    }

    fn record(&mut self, prime: u64, probes: &[Record]) -> io::Result<()> {
        let path = self.path(prime);
        let size = match std::fs::metadata(&path) {
            Ok(metadata) => Some(metadata.len()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        // The complete prefix: from the cache when the file still has the
        // size we left it at, else from one read (and validation).
        let complete = match (size, self.complete.get(&prime)) {
            (None, _) => 0,
            (Some(size), Some(&complete)) if size == complete && complete >= HEADER as u64 => {
                complete
            }
            _ => self.read_complete(prime)?.1,
        };
        let mut bytes = Vec::new();
        if complete == 0 {
            bytes.extend_from_slice(&self.header(prime));
        }
        self.encode(prime, probes, &mut bytes)?;
        let mut file = if complete == 0 {
            File::create(&path)?
        } else {
            let file = OpenOptions::new().append(true).open(&path)?;
            // Cut a torn last record (an interrupted earlier write):
            // appending after it would misalign the records.
            if file.metadata()?.len() != complete {
                file.set_len(complete)?;
            }
            file
        };
        file.write_all(&bytes)?;
        file.sync_data()?;
        self.complete.insert(
            prime,
            if complete == 0 {
                bytes.len() as u64
            } else {
                complete + bytes.len() as u64
            },
        );
        self.written += probes.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::reconstruct::driver::{
        BlackBox, PrimeBlackBox, ProbeResult, UnivariateOptions, reconstruct_univariate,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
    use symbolica::domains::{Field, RingOps};
    use symbolica::prelude::Ring;

    struct Counting<'a> {
        calls: &'a AtomicUsize,
    }
    struct CountingPrime<'a> {
        field: Zp64,
        calls: &'a AtomicUsize,
    }
    impl<'a> BlackBox for Counting<'a> {
        type Prime = CountingPrime<'a>;
        fn outputs(&self) -> usize {
            2
        }
        fn at_prime(&self, prime: u64) -> Option<CountingPrime<'a>> {
            Some(CountingPrime {
                field: Zp64::new(prime),
                calls: self.calls,
            })
        }
    }
    impl PrimeBlackBox for CountingPrime<'_> {
        fn probe(&self, xs: &[u64]) -> Vec<ProbeResult> {
            self.calls.fetch_add(xs.len(), Ordering::Relaxed);
            let field = &self.field;
            xs.iter()
                .map(|&x| {
                    let x = field.to_element(x);
                    let f = field.div(
                        &field.add(&x, &field.one()),
                        &field.add(&field.mul(&x, &x), &field.one()),
                    );
                    Ok(vec![
                        field.from_element(&f),
                        field.from_element(&field.mul(&x, &x)),
                    ])
                })
                .collect()
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rr-probe-log-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn resumed_runs_replay_every_probe() {
        let dir = temp_dir("resume");
        let options = UnivariateOptions::default();
        let calls = AtomicUsize::new(0);
        let black_box = Counting { calls: &calls };
        let mut log = FileProbeLog::new(&dir, [7; 32], 2).unwrap();
        let first =
            reconstruct_univariate(&black_box, &black_box, &options, Some(&mut log)).unwrap();
        drop(log);
        let first_calls = calls.swap(0, Ordering::Relaxed);
        let mut log = FileProbeLog::new(&dir, [7; 32], 2).unwrap();
        let second =
            reconstruct_univariate(&black_box, &black_box, &options, Some(&mut log)).unwrap();
        // Only the verification probes run again.
        assert_eq!(calls.load(Ordering::Relaxed), options.verify_points);
        assert!(first_calls > options.verify_points);
        assert_eq!(first.functions, second.functions);
        // Another key does not reuse the log.
        let mut other = FileProbeLog::new(&dir, [8; 32], 2).unwrap();
        assert!(other.load(first.report.primes[0].prime).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    const PRIME: u64 = 1_000_003;

    fn written(log: &mut FileProbeLog) {
        log.record(
            PRIME,
            &[(5, Some(vec![10, 20])), (6, None), (7, Some(vec![1, 2]))],
        )
        .unwrap();
    }

    fn flip(path: &Path, offset: u64) {
        let mut bytes = std::fs::read(path).unwrap();
        bytes[offset as usize] ^= 0x5a;
        std::fs::write(path, bytes).unwrap();
    }

    /// An interrupted append leaves a torn last record; it is ignored on
    /// load and cut before the next append, otherwise the new records would
    /// be misaligned.
    #[test]
    fn a_torn_record_is_dropped_before_the_next_append() {
        let dir = temp_dir("torn");
        let mut log = FileProbeLog::new(&dir, [3; 32], 2).unwrap();
        written(&mut log);
        let path = log.path(PRIME);
        let full = std::fs::metadata(&path).unwrap().len();
        // Interrupted write: a length prefix and part of the payload.
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&25u32.to_le_bytes()).unwrap();
        file.write_all(&9u64.to_le_bytes()).unwrap();
        drop(file);
        let expected = vec![(5, Some(vec![10, 20])), (6, None), (7, Some(vec![1, 2]))];
        assert_eq!(log.load(PRIME).unwrap(), expected);
        log.record(PRIME, &[(9, Some(vec![50, 60])), (11, None)])
            .unwrap();
        let mut more = expected.clone();
        more.extend([(9, Some(vec![50, 60])), (11, None)]);
        assert_eq!(log.load(PRIME).unwrap(), more);
        assert!(std::fs::metadata(&path).unwrap().len() > full);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review #24 finding 3: a corrupted MIDDLE record (a value byte, the
    /// status byte, the length) is refused; the same damage in the LAST
    /// record is a torn write and is dropped.
    #[test]
    fn a_corrupted_middle_record_is_refused() {
        // Record layout: 4-byte length, 8-byte x, 1-byte status, values, 8-byte sum.
        let first = HEADER as u64;
        let first_len = 4 + 9 + 16 + 8;
        for (label, offset) in [
            ("value", 4 + 9 + 3),
            ("status", 4 + 8),
            ("length", 0),
            ("sum", 4 + 25),
        ] {
            let dir = temp_dir(&format!("mid-{label}"));
            let mut log = FileProbeLog::new(&dir, [4; 32], 2).unwrap();
            written(&mut log);
            let path = log.path(PRIME);
            flip(&path, first + offset);
            let error = log.load(PRIME).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{label}: {error}");
            // A record cannot be appended to a corrupted file either.
            assert!(log.record(PRIME, &[(13, None)]).is_err(), "{label}");
            drop(log);
            let _ = std::fs::remove_dir_all(&dir);
        }
        // The last record (x = 7) damaged in a value: a torn write.
        let dir = temp_dir("last");
        let mut log = FileProbeLog::new(&dir, [4; 32], 2).unwrap();
        written(&mut log);
        let path = log.path(PRIME);
        let last = first + first_len + (4 + 9 + 8);
        flip(&path, last + 4 + 9 + 1);
        assert_eq!(
            log.load(PRIME).unwrap(),
            vec![(5, Some(vec![10, 20])), (6, None)]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review #24 finding 7: the prime is bound by the header, so a file of
    /// another prime (renamed or copied) is refused.
    #[test]
    fn a_file_of_another_prime_is_refused() {
        let dir = temp_dir("rename");
        let mut log = FileProbeLog::new(&dir, [5; 32], 2).unwrap();
        written(&mut log);
        let other = 999_983;
        std::fs::rename(log.path(PRIME), log.path(other)).unwrap();
        assert_eq!(
            log.load(other).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review #24 finding 6: one writer per directory and key; records are
    /// range-checked on write.
    #[test]
    fn a_second_writer_and_unreduced_records_are_refused() {
        let dir = temp_dir("writers");
        let log = FileProbeLog::new(&dir, [6; 32], 2).unwrap();
        let second = FileProbeLog::new(&dir, [6; 32], 2);
        assert_eq!(
            second.err().map(|e| e.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
        drop(log);
        let mut log = FileProbeLog::new(&dir, [6; 32], 2).unwrap();
        assert!(log.record(PRIME, &[(PRIME, None)]).is_err());
        assert!(log.record(PRIME, &[(3, Some(vec![PRIME, 1]))]).is_err());
        assert!(log.record(PRIME, &[(3, Some(vec![1]))]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
