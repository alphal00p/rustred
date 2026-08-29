use std::fmt;

use crate::campaign::{CampaignBytes, CampaignEstimatorRevision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignExecutionWidthError {
    ZeroRequestedCoreCeiling,
    ZeroEnclosingMemoryLimit,
    ZeroOperationalMemoryLimit,
    OperationalMemoryNotBelowEnclosing {
        operational: CampaignBytes,
        enclosing: CampaignBytes,
    },
    MinimumTaskEstimatorRevisionMismatch {
        expected: CampaignEstimatorRevision,
        actual: CampaignEstimatorRevision,
    },
    MinimumTaskMustUseOneCore {
        actual: usize,
    },
    ByteCountOverflow {
        operation: &'static str,
    },
    CoreCountDoesNotFitByteArithmetic {
        operation: &'static str,
        count: usize,
    },
}

impl fmt::Display for CampaignExecutionWidthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRequestedCoreCeiling => {
                formatter.write_str("campaign requested core ceiling must be positive")
            }
            Self::ZeroEnclosingMemoryLimit => {
                formatter.write_str("campaign enclosing memory limit must be positive")
            }
            Self::ZeroOperationalMemoryLimit => {
                formatter.write_str("campaign operational memory limit must be positive")
            }
            Self::OperationalMemoryNotBelowEnclosing {
                operational,
                enclosing,
            } => write!(
                formatter,
                "campaign operational memory limit {operational} must be strictly below enclosing memory limit {enclosing}"
            ),
            Self::MinimumTaskEstimatorRevisionMismatch { expected, actual } => write!(
                formatter,
                "minimum runnable task uses estimator revision {}, expected {}",
                actual.get(),
                expected.get()
            ),
            Self::MinimumTaskMustUseOneCore { actual } => write!(
                formatter,
                "minimum runnable task requests {actual} cores; pre-pool planning requires exactly one"
            ),
            Self::ByteCountOverflow { operation } => {
                write!(formatter, "{operation} overflowed u64")
            }
            Self::CoreCountDoesNotFitByteArithmetic { operation, count } => write!(
                formatter,
                "{operation} cannot represent core/thread count {count} as u64"
            ),
        }
    }
}

impl std::error::Error for CampaignExecutionWidthError {}
