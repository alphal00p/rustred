//! Pid numbers that may be TARGETED: range-checked newtypes (audit A m1, audit B C1/C2).
//!
//! Every pid that comes from a file (ledger record, serde), a scan or a child handle passes through [`ValidPid`] before
//! it may be named in a signal. The constructor rejects:
//! - pid <= 1: 0 is "my own group" for kill(2) and "session 0" (kernel threads) for scans; 1 is init and, negated,
//!   `kill(-1)` = every process of the user;
//! - pid > `/proc/sys/kernel/pid_max` (default 4,194,304 on 64-bit), which also excludes every value whose `as pid_t`
//!   cast would turn negative;
//! - for signalling ([`ValidPid::for_signal`]): this process, its parent, and the leaders of its own session and process
//!   group (the user's shell / pane).
//!
//! The number inside a `ValidPid` is therefore always in 2..=pid_max and can never be negated into a broadcast.

use serde::Serialize;
use std::fmt;
use std::sync::OnceLock;

/// PID_MAX_LIMIT on 64-bit Linux: the largest value pid_max can be set to.
pub const PID_MAX_LIMIT: u32 = 4_194_304;

/// `/proc/sys/kernel/pid_max`, read once (PID_MAX_LIMIT when unreadable or out of range).
pub fn pid_max() -> u32 {
    static MAX: OnceLock<u32> = OnceLock::new();
    *MAX.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/pid_max")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .filter(|&m| (2..=PID_MAX_LIMIT).contains(&m))
            .unwrap_or(PID_MAX_LIMIT)
    })
}

/// A pid number refused as a target. Never carries anything but the number.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvalidPid {
    pub pid: i64,
    pub why: &'static str,
}

impl fmt::Display for InvalidPid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pid {} refused as a target: {}", self.pid, self.why)
    }
}

impl std::error::Error for InvalidPid {}

impl InvalidPid {
    /// As an `io::Error` of kind InvalidInput whose payload downcasts to `InvalidPid`.
    pub fn into_io(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, self)
    }
}

/// A pid in 2..=pid_max. Copy, comparable; the only pid type the signal wrapper accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ValidPid(u32);

impl ValidPid {
    /// Range check only (2..=pid_max). For records and scans.
    pub fn new(pid: i64) -> Result<Self, InvalidPid> {
        if pid <= 1 {
            return Err(InvalidPid {
                pid,
                why: "pid <= 1 (0 = own group / session 0, 1 = init; negated: every process)",
            });
        }
        if pid > i64::from(pid_max()) {
            return Err(InvalidPid {
                pid,
                why: "pid above /proc/sys/kernel/pid_max",
            });
        }
        Ok(Self(pid as u32))
    }

    /// Range check plus: not this process, not its parent, not the leader of its own session or process group.
    pub fn for_signal(pid: i64) -> Result<Self, InvalidPid> {
        let v = Self::new(pid)?;
        // SAFETY: getpid/getppid/getsid(0)/getpgrp take no pointers and cannot fail for the calling process.
        let (me, parent, sid, pgrp) = unsafe {
            (
                libc::getpid(),
                libc::getppid(),
                libc::getsid(0),
                libc::getpgrp(),
            )
        };
        let p = v.0 as libc::pid_t;
        if p == me {
            return Err(InvalidPid {
                pid,
                why: "this process",
            });
        }
        if p == parent {
            return Err(InvalidPid {
                pid,
                why: "the parent of this process",
            });
        }
        if p == sid || p == pgrp {
            return Err(InvalidPid {
                pid,
                why: "the leader of this process's own session or group",
            });
        }
        Ok(v)
    }

    pub fn get(self) -> u32 {
        self.0
    }

    /// As `pid_t`: always in 2..=pid_max, hence positive.
    pub fn as_pid_t(self) -> libc::pid_t {
        self.0 as libc::pid_t
    }
}

impl fmt::Display for ValidPid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_and_self_rules() {
        for bad in [-5i64, -1, 0, 1, i64::from(pid_max()) + 1, 1 << 31, 1 << 32] {
            assert!(ValidPid::new(bad).is_err(), "{bad}");
            assert!(ValidPid::for_signal(bad).is_err(), "{bad}");
        }
        assert_eq!(ValidPid::new(2).unwrap().get(), 2);
        let me = i64::from(std::process::id());
        assert!(ValidPid::new(me).is_ok());
        assert!(ValidPid::for_signal(me).is_err());
        // SAFETY: getppid cannot fail.
        let parent = i64::from(unsafe { libc::getppid() });
        if parent > 1 {
            assert!(ValidPid::for_signal(parent).is_err());
        }
    }
}
