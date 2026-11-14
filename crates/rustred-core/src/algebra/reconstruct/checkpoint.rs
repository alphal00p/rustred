//! File-backed [`ProbeLog`]: one append-only file per prime.
//!
//! Layout of `<dir>/probes-<key>-<prime>.bin`:
//! - header: magic `RRPROBE1`, `u64` outputs, 32-byte key;
//! - records: `u64` x, `u8` status (1 = values follow), then `outputs`
//!   little-endian `u64` values.
//!
//! A torn last record (an interrupted write) is ignored on load and cut off
//! before the next append, so new records always start on a record
//! boundary. A header with another key or output count is an error: the
//! log belongs to another system or request.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::driver::ProbeLog;

const MAGIC: &[u8; 8] = b"RRPROBE1";

pub struct FileProbeLog {
    dir: PathBuf,
    key: [u8; 32],
    outputs: usize,
    /// Records loaded per prime, for reports.
    pub loaded: usize,
    pub written: usize,
}

impl FileProbeLog {
    /// `key` binds the log to one black box (system, request, settings).
    pub fn new(dir: impl AsRef<Path>, key: [u8; 32], outputs: usize) -> io::Result<Self> {
        std::fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
            key,
            outputs,
            loaded: 0,
            written: 0,
        })
    }

    fn path(&self, prime: u64) -> PathBuf {
        let key: String = self.key[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.dir.join(format!("probes-{key}-{prime}.bin"))
    }

    fn read_all(&self, prime: u64) -> io::Result<Vec<(u64, Option<Vec<u64>>)>> {
        Ok(self.read_complete(prime)?.0)
    }

    /// The complete records and the length in bytes of the file prefix
    /// they occupy (header included); a torn last record is not counted.
    #[allow(clippy::type_complexity)]
    fn read_complete(&self, prime: u64) -> io::Result<(Vec<(u64, Option<Vec<u64>>)>, u64)> {
        let path = self.path(prime);
        let mut bytes = Vec::new();
        match File::open(&path) {
            Ok(mut file) => {
                file.read_to_end(&mut bytes)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(error) => return Err(error),
        }
        if bytes.len() < 48 {
            return Ok((Vec::new(), 0));
        }
        if &bytes[..8] != MAGIC
            || u64::from_le_bytes(bytes[8..16].try_into().expect("8 bytes")) != self.outputs as u64
            || bytes[16..48] != self.key
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} belongs to another black box", path.display()),
            ));
        }
        let mut records = Vec::new();
        let mut at = 48;
        while at + 9 <= bytes.len() {
            let x = u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
            let ok = bytes[at + 8] == 1;
            let end = at + 9 + if ok { 8 * self.outputs } else { 0 };
            if end > bytes.len() {
                break;
            }
            let values = ok.then(|| {
                bytes[at + 9..end]
                    .chunks_exact(8)
                    .map(|chunk| u64::from_le_bytes(chunk.try_into().expect("8 bytes")))
                    .collect()
            });
            records.push((x, values));
            at = end;
        }
        Ok((records, at as u64))
    }
}

impl ProbeLog for FileProbeLog {
    fn load(&mut self, prime: u64) -> Vec<(u64, Option<Vec<u64>>)> {
        match self.read_all(prime) {
            Ok(records) => {
                self.loaded += records.len();
                records
            }
            // A foreign or unreadable log is not reused (never mixed in).
            Err(_) => Vec::new(),
        }
    }

    fn record(&mut self, prime: u64, probes: &[(u64, Option<Vec<u64>>)]) -> io::Result<()> {
        let path = self.path(prime);
        let fresh = !path.exists() || std::fs::metadata(&path)?.len() < 48;
        let mut bytes = Vec::new();
        let mut complete = 0u64;
        if fresh {
            bytes.extend_from_slice(MAGIC);
            bytes.extend_from_slice(&(self.outputs as u64).to_le_bytes());
            bytes.extend_from_slice(&self.key);
        } else {
            match self.read_complete(prime) {
                Ok((_, length)) => complete = length,
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{} belongs to another black box", path.display()),
                    ));
                }
            }
        }
        for (x, values) in probes {
            bytes.extend_from_slice(&x.to_le_bytes());
            match values {
                Some(values) => {
                    if values.len() != self.outputs {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "wrong output count",
                        ));
                    }
                    bytes.push(1);
                    for value in values {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
                None => bytes.push(0),
            }
        }
        let mut file = if fresh {
            File::create(&path)?
        } else {
            let file = OpenOptions::new().append(true).open(&path)?;
            // Cut off a torn last record (an interrupted earlier write):
            // appending after it would make the reader take the new records
            // for its missing values.
            if file.metadata()?.len() != complete {
                file.set_len(complete)?;
            }
            file
        };
        file.write_all(&bytes)?;
        file.sync_data()?;
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

    #[test]
    fn resumed_runs_replay_every_probe() {
        let dir = std::env::temp_dir().join(format!("rr-probe-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let options = UnivariateOptions::default();
        let calls = AtomicUsize::new(0);
        let black_box = Counting { calls: &calls };
        let mut log = FileProbeLog::new(&dir, [7; 32], 2).unwrap();
        let first =
            reconstruct_univariate(&black_box, &black_box, &options, Some(&mut log)).unwrap();
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
        assert!(other.load(first.report.primes[0].prime).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An interrupted append leaves a torn last record; it is ignored on
    /// load and must be dropped before the next append, otherwise the new
    /// records are read as the torn record's values (and misaligned).
    #[test]
    fn a_torn_record_is_dropped_before_the_next_append() {
        let dir = std::env::temp_dir().join(format!("rr-probe-log-torn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let prime = 1_000_003u64;
        let mut log = FileProbeLog::new(&dir, [3; 32], 2).unwrap();
        log.record(prime, &[(5, Some(vec![10, 20]))]).unwrap();
        // Interrupted write: x, status and only the first of two values.
        let mut file = OpenOptions::new()
            .append(true)
            .open(log.path(prime))
            .unwrap();
        file.write_all(&7u64.to_le_bytes()).unwrap();
        file.write_all(&[1]).unwrap();
        file.write_all(&30u64.to_le_bytes()).unwrap();
        drop(file);
        assert_eq!(log.load(prime), vec![(5, Some(vec![10, 20]))]);
        log.record(prime, &[(9, Some(vec![50, 60])), (11, None)])
            .unwrap();
        assert_eq!(
            log.load(prime),
            vec![(5, Some(vec![10, 20])), (9, Some(vec![50, 60])), (11, None)]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
