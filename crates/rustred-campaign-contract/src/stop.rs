//! Stop codes, stop sources, job status and case results (child-job.md 3.5, 3.7).

use crate::exit::ExitCode;
use serde::{Deserialize, Serialize};

/// Stop codes, closed list v1 (`OUTCOME.job.stop.code`; `null` = no stop, serialised as `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopCode {
    MaxCases,
    LiftRefused,
    GuardTimeout,
    OpenRegions,
    OpenObligations,
    BeforeNumeric,
    NumericSearchRefused,
    NotLower,
    BudgetWork,
    BudgetWall,
    BudgetMemory,
    Interrupted,
    InputError,
    IoError,
    Nospace,
    InternalError,
    Panic,
    Refused,
}

impl StopCode {
    pub const ALL: [StopCode; 18] = [
        StopCode::MaxCases,
        StopCode::LiftRefused,
        StopCode::GuardTimeout,
        StopCode::OpenRegions,
        StopCode::OpenObligations,
        StopCode::BeforeNumeric,
        StopCode::NumericSearchRefused,
        StopCode::NotLower,
        StopCode::BudgetWork,
        StopCode::BudgetWall,
        StopCode::BudgetMemory,
        StopCode::Interrupted,
        StopCode::InputError,
        StopCode::IoError,
        StopCode::Nospace,
        StopCode::InternalError,
        StopCode::Panic,
        StopCode::Refused,
    ];

    /// The engine's domain stops (exit 10, INCOMPLETE, for producing kinds).
    pub const fn is_domain(self) -> bool {
        matches!(
            self,
            StopCode::MaxCases
                | StopCode::LiftRefused
                | StopCode::GuardTimeout
                | StopCode::OpenRegions
                | StopCode::OpenObligations
                | StopCode::BeforeNumeric
                | StopCode::NumericSearchRefused
                | StopCode::NotLower
        )
    }

    /// The exit code a producing job (generate, witness, fake, table-dump) ends with for this stop (3.6).
    /// `refused` maps to 42; usage errors (40) carry stop `refused` too, see [`ExitCode::Usage`].
    pub const fn exit_code(self) -> ExitCode {
        match self {
            s if s.is_domain() => ExitCode::Incomplete,
            StopCode::BudgetWork => ExitCode::BudgetWork,
            StopCode::BudgetWall => ExitCode::BudgetWall,
            StopCode::BudgetMemory => ExitCode::BudgetMemory,
            StopCode::Interrupted => ExitCode::Interrupted,
            StopCode::InputError => ExitCode::Input,
            StopCode::IoError => ExitCode::Io,
            StopCode::Nospace => ExitCode::NoSpace,
            StopCode::InternalError | StopCode::Panic => ExitCode::Internal,
            _ => ExitCode::Refused,
        }
    }

    /// Whether `code` is a consistent exit code for this stop (USAGE also carries `refused`).
    pub fn consistent_with(self, code: ExitCode) -> bool {
        self.exit_code() == code || (self == StopCode::Refused && code == ExitCode::Usage)
    }
}

/// `OUTCOME.job.status` (closed list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Complete,
    Incomplete,
    DecidedNegative,
    Undecided,
    Budget,
    Interrupted,
    Error,
    Refused,
}

impl JobStatus {
    /// The status that goes with an exit code (the document's status list fixed per code by package contract):
    /// 0 complete, 10 incomplete, 11 decided_negative, 12 undecided, 20/21/22 budget, 30 interrupted,
    /// 40/42 refused, 41/50/51/70 error.
    pub const fn for_exit(code: ExitCode) -> JobStatus {
        match code {
            ExitCode::Complete => JobStatus::Complete,
            ExitCode::Incomplete => JobStatus::Incomplete,
            ExitCode::DecidedNegative => JobStatus::DecidedNegative,
            ExitCode::Undecided => JobStatus::Undecided,
            ExitCode::BudgetWork | ExitCode::BudgetWall | ExitCode::BudgetMemory => JobStatus::Budget,
            ExitCode::Interrupted => JobStatus::Interrupted,
            ExitCode::Usage | ExitCode::Refused => JobStatus::Refused,
            ExitCode::Input | ExitCode::NoSpace | ExitCode::Io | ExitCode::Internal => JobStatus::Error,
        }
    }
}

/// Source of a stop request (event `stop_requested`, `OUTCOME.job.stop.source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StopSource {
    #[serde(rename = "SIGTERM")]
    Sigterm,
    #[serde(rename = "SIGINT")]
    Sigint,
    #[serde(rename = "SIGHUP")]
    Sighup,
    #[serde(rename = "stopfile")]
    StopFile,
    #[serde(rename = "budget")]
    Budget,
    #[serde(rename = "memory")]
    Memory,
}

/// Result of one case (event `case_finished`, `OUTCOME.job.per_case[].result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseResult {
    Rule,
    Covered,
    Empty,
    Finite,
    Refused,
    Budget,
    DeclaredTerminal,
    RegistryCovered,
    Error,
}

impl CaseResult {
    /// Results meaning "no rule needed" (row 3 of the decision table 6.5).
    pub const fn needs_no_rule(self) -> bool {
        matches!(self, CaseResult::Covered | CaseResult::Empty | CaseResult::Finite | CaseResult::DeclaredTerminal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_exit_map() {
        for s in StopCode::ALL {
            let e = s.exit_code();
            assert!(s.consistent_with(e));
            if s.is_domain() {
                assert_eq!(e, ExitCode::Incomplete);
            }
        }
        assert_eq!(StopCode::BudgetWall.exit_code(), ExitCode::BudgetWall);
        assert_eq!(StopCode::Panic.exit_code(), ExitCode::Internal);
        assert_eq!(StopCode::Nospace.exit_code(), ExitCode::NoSpace);
        assert!(StopCode::Refused.consistent_with(ExitCode::Usage));
        assert_eq!(serde_json::to_string(&StopCode::NumericSearchRefused).unwrap(), "\"numeric_search_refused\"");
        assert_eq!(serde_json::to_string(&StopSource::Sigterm).unwrap(), "\"SIGTERM\"");
        assert_eq!(serde_json::to_string(&StopSource::StopFile).unwrap(), "\"stopfile\"");
    }

    #[test]
    fn status_per_exit() {
        assert_eq!(JobStatus::for_exit(ExitCode::BudgetWall), JobStatus::Budget);
        assert_eq!(JobStatus::for_exit(ExitCode::Usage), JobStatus::Refused);
        assert_eq!(JobStatus::for_exit(ExitCode::NoSpace), JobStatus::Error);
    }
}
