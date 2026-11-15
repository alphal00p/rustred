//! Exit codes of every program in job mode (child-job.md 3.6). Numeric values are stable: never renumber.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The exit-code table. Anything not listed (101 = uncaught panic, death by a signal, unknown codes) is CRASHED and
/// the files decide (6.5); such a status is not representable here on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum ExitCode {
    /// Goal reached (chart closed; checker PASS). OUTCOME sealed.
    Complete = 0,
    /// Engine stopped with a domain reason (max_cases, lift_refused, ...). Sealed partial table.
    Incomplete = 10,
    /// Checker decided FAIL.
    DecidedNegative = 11,
    /// Checker could not decide.
    Undecided = 12,
    /// A work budget stopped the job (deterministic).
    BudgetWork = 20,
    /// A wall budget stopped the job (time-decided; table unsealed).
    BudgetWall = 21,
    /// The soft memory limit stopped the job.
    BudgetMemory = 22,
    /// Signal or STOP file; `stop.graceful` says how.
    Interrupted = 30,
    /// Spec or args invalid (a supervisor bug).
    Usage = 40,
    /// Input missing, unreadable, digest mismatch, unparsable or unsupported family.
    Input = 41,
    /// Contract refusal (dirty run dir, OUTCOME exists, forbidden option, env var, feature unsupported).
    Refused = 42,
    /// ENOSPC / EDQUOT on an output.
    NoSpace = 50,
    /// Other output I/O error.
    Io = 51,
    /// Engine error after preparation, caught panic.
    Internal = 70,
}

/// How a code may be resumed (column "resumable" of 3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resumable {
    NotApplicable,
    /// Through the open list of the sealed outcome (RESUME.pending).
    ViaOpenList,
    Yes,
    /// Yes, once free space is back.
    AfterSpace,
    No,
}

impl ExitCode {
    /// Every code, in table order.
    pub const ALL: [ExitCode; 14] = [
        ExitCode::Complete,
        ExitCode::Incomplete,
        ExitCode::DecidedNegative,
        ExitCode::Undecided,
        ExitCode::BudgetWork,
        ExitCode::BudgetWall,
        ExitCode::BudgetMemory,
        ExitCode::Interrupted,
        ExitCode::Usage,
        ExitCode::Input,
        ExitCode::Refused,
        ExitCode::NoSpace,
        ExitCode::Io,
        ExitCode::Internal,
    ];

    /// The numeric process exit status.
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// The table entry for a numeric status, `None` for anything outside the table (= CRASHED).
    pub fn from_code(c: i32) -> Option<ExitCode> {
        ExitCode::ALL.into_iter().find(|e| e.code() == c)
    }

    /// The upper-case name of the table.
    pub const fn name(self) -> &'static str {
        match self {
            ExitCode::Complete => "COMPLETE",
            ExitCode::Incomplete => "INCOMPLETE",
            ExitCode::DecidedNegative => "DECIDED_NEGATIVE",
            ExitCode::Undecided => "UNDECIDED",
            ExitCode::BudgetWork => "BUDGET_WORK",
            ExitCode::BudgetWall => "BUDGET_WALL",
            ExitCode::BudgetMemory => "BUDGET_MEMORY",
            ExitCode::Interrupted => "INTERRUPTED",
            ExitCode::Usage => "USAGE",
            ExitCode::Input => "INPUT",
            ExitCode::Refused => "REFUSED",
            ExitCode::NoSpace => "NOSPACE",
            ExitCode::Io => "IO",
            ExitCode::Internal => "INTERNAL",
        }
    }

    /// Column "resumable".
    pub const fn resumable(self) -> Resumable {
        match self {
            ExitCode::Complete => Resumable::NotApplicable,
            ExitCode::Incomplete => Resumable::ViaOpenList,
            ExitCode::BudgetWork
            | ExitCode::BudgetWall
            | ExitCode::BudgetMemory
            | ExitCode::Interrupted
            | ExitCode::Io => Resumable::Yes,
            ExitCode::NoSpace => Resumable::AfterSpace,
            ExitCode::DecidedNegative
            | ExitCode::Undecided
            | ExitCode::Usage
            | ExitCode::Input
            | ExitCode::Refused
            | ExitCode::Internal => Resumable::No,
        }
    }

    /// Whether the child is required to have written OUTCOME.json for this code (column "OUTCOME"): every code
    /// except USAGE (only "if the run dir is known") and NOSPACE / IO ("best effort").
    pub const fn outcome_required(self) -> bool {
        !matches!(self, ExitCode::Usage | ExitCode::NoSpace | ExitCode::Io)
    }
}

impl From<ExitCode> for i32 {
    fn from(e: ExitCode) -> i32 {
        e.code()
    }
}

/// Error of [`ExitCode::try_from`]: the status is outside the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotInTable(pub i32);

impl fmt::Display for NotInTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "exit status {} is not in the child-job exit-code table",
            self.0
        )
    }
}
impl std::error::Error for NotInTable {}

impl TryFrom<i32> for ExitCode {
    type Error = NotInTable;
    fn try_from(c: i32) -> Result<Self, Self::Error> {
        ExitCode::from_code(c).ok_or(NotInTable(c))
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.code(), self.name())
    }
}

impl Serialize for ExitCode {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(self.code())
    }
}

impl<'de> Deserialize<'de> for ExitCode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let c = i32::deserialize(d)?;
        ExitCode::from_code(c).ok_or_else(|| serde::de::Error::custom(NotInTable(c)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_numeric_values() {
        let v: Vec<i32> = ExitCode::ALL.iter().map(|e| e.code()).collect();
        assert_eq!(v, [0, 10, 11, 12, 20, 21, 22, 30, 40, 41, 42, 50, 51, 70]);
    }

    #[test]
    fn round_trip_and_outside_table() {
        for e in ExitCode::ALL {
            assert_eq!(ExitCode::try_from(e.code()), Ok(e));
            let j = serde_json::to_string(&e).unwrap();
            assert_eq!(j, e.code().to_string());
            assert_eq!(serde_json::from_str::<ExitCode>(&j).unwrap(), e);
        }
        for c in [1, 2, 9, 101, 137, -1, 255] {
            assert_eq!(ExitCode::from_code(c), None);
            assert!(serde_json::from_str::<ExitCode>(&c.to_string()).is_err());
        }
    }

    #[test]
    fn names_unique() {
        let mut n: Vec<_> = ExitCode::ALL.iter().map(|e| e.name()).collect();
        n.sort();
        n.dedup();
        assert_eq!(n.len(), ExitCode::ALL.len());
    }
}
