//! A lock-held, durable high-water mark, independent of the last saved merge.
//! A crash after reservation but before the next checkpoint burns that session;
//! saved_session+1 alone would reuse it. No permit or CAS state is transferred.
use super::{SESSION_LIMIT, invalid};
use crate::application::atomic_file::write_file_atomically;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

const FILE: &str = "epoch-session.bin";
const MAGIC: &[u8; 8] = b"EPCS0001";

/// Constructor is private: only a successful durable reservation produces it.
/// Intentionally neither Copy nor Clone, so one reservation starts one Dispatch.
pub(in super::super) struct Session(u64);
impl Session {
    pub fn number(&self) -> u64 {
        self.0
    }
}

fn write(directory: &Path, number: u64, replace: bool) -> io::Result<Session> {
    if number == 0 || number >= SESSION_LIMIT {
        return Err(invalid("epoch session sequence exhausted"));
    }
    let mut bytes = [0u8; 48];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..16].copy_from_slice(&number.to_le_bytes());
    let digest = *blake3::hash(&bytes[..16]).as_bytes();
    bytes[16..].copy_from_slice(&digest);
    // Error after rename but before directory fsync is publication-uncertain:
    // do not return the capability or issue a job, and never roll the file back.
    write_file_atomically(&directory.join(FILE), &bytes, replace).map_err(io::Error::other)?;
    Ok(Session(number))
}

pub(super) fn initial(directory: &Path) -> io::Result<Session> {
    write(directory, 1, false)
}

pub(super) fn read(directory: &Path) -> io::Result<u64> {
    let path = directory.join(FILE);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != 48 {
        return Err(invalid("epoch session journal file shape"));
    }
    let mut input = File::open(path)?;
    let mut bytes = [0; 48];
    input.read_exact(&mut bytes)?;
    if input.read(&mut [0; 1])? != 0
        || &bytes[..8] != MAGIC
        || &bytes[16..] != blake3::hash(&bytes[..16]).as_bytes()
    {
        return Err(invalid("epoch session journal bytes or digest"));
    }
    let number = u64::from_le_bytes(bytes[8..16].try_into().expect("fixed width"));
    if number == 0 || number >= SESSION_LIMIT {
        return Err(invalid("epoch session journal range"));
    }
    Ok(number)
}

/// Caller holds the checkpoint store lock throughout read, reserve and use.
pub(super) fn reserve(directory: &Path, saved: u64) -> io::Result<Session> {
    let previous = read(directory)?;
    if saved == 0 || saved > previous {
        return Err(invalid("epoch saved session exceeds durable reservation"));
    }
    let next = previous
        .checked_add(1)
        .filter(|&n| n < SESSION_LIMIT)
        .ok_or_else(|| invalid("epoch session sequence exhausted"))?;
    write(directory, next, true)
}

#[cfg(test)]
mod tests {
    use super::super::tests::Directory;
    use super::*;

    #[test]
    fn crashes_before_a_merge_save_cannot_reuse_sessions() {
        let directory = Directory::new();
        assert_eq!(initial(&directory.0).unwrap().number(), 1);
        assert!(initial(&directory.0).is_err());
        assert_eq!(reserve(&directory.0, 1).unwrap().number(), 2);
        // No checkpoint save happened; restart still sees saved session1.
        assert_eq!(reserve(&directory.0, 1).unwrap().number(), 3);
        assert_eq!(read(&directory.0).unwrap(), 3);
        assert!(reserve(&directory.0, 4).is_err());
        assert!(reserve(&directory.0, 0).is_err());
    }

    #[test]
    fn corrupt_missing_and_exhausted_session_journals_fail_closed() {
        let directory = Directory::new();
        assert!(read(&directory.0).is_err());
        write(&directory.0, SESSION_LIMIT - 1, false).unwrap();
        assert!(reserve(&directory.0, 1).is_err());
        assert_eq!(read(&directory.0).unwrap(), SESSION_LIMIT - 1);
        for length in [0, 47, 48, 49] {
            fs::write(directory.0.join(FILE), vec![0; length]).unwrap();
            assert!(read(&directory.0).is_err());
        }
        assert!(write(&directory.0, SESSION_LIMIT, true).is_err());
    }
}
