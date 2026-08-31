use std::fmt;

use crate::foundry::completion::stratum::StratumRegistryError;

use super::SourceDiscoveryError;

/// Typed fail-closed reasons at sampled-dual admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SampledDeclaredModuleDualError {
    IncidenceVerification(SourceDiscoveryError),
    IncidenceTaskScopeMismatch {
        detail: &'static str,
    },
    GuardedStratumRequiresSampleWitness {
        guard_count: usize,
    },
    QueryIsModularHit,
    PartitionVerification(StratumRegistryError),
    PartitionNotVerified,
    PartitionPlanMismatch,
    SamplePlanMismatch,
    ObstructionPlanMismatch,
    ObstructionSampleMismatch,
    ObstructionPartitionMismatch,
    TargetColumnOutOfRange,
    TargetColumnMismatch,
    TargetShiftMismatch,
    FixedStratumMismatch,
    FixedOrderingMismatch,
    FixedOwnerSnapshotMismatch,
    MaterializedSourceChronologyMismatch,
    NominationIncidenceMismatch,
    NominationIsTargetUnit,
    NominationObstructionMismatch,
    ResidualNominationMismatch,
    ResidualIncidenceMismatch,
    ResidualPlanMismatch,
    ResidualObstructionMismatch,
    ResidualSampleMismatch,
    NominationVerification(SourceDiscoveryError),
    IncompleteNominationCensus,
    ResidualTelemetryMismatch,
    ResidualPairingShiftOverflow {
        candidate_ordinal: usize,
        term_ordinal: usize,
        position: usize,
        offset: i64,
        source_shift: i64,
    },
    CuttingResiduals {
        count: usize,
    },
    RawObstructionMismatch,
    RankDiagnosticsMismatch {
        detail: &'static str,
    },
    Retention(SourceDiscoveryError),
}

impl fmt::Display for SampledDeclaredModuleDualError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncidenceVerification(error) => {
                write!(formatter, "sampled-dual incidence verification failed: {error}")
            }
            Self::IncidenceTaskScopeMismatch { detail } => {
                write!(formatter, "sampled-dual task scope mismatch: {detail}")
            }
            Self::GuardedStratumRequiresSampleWitness { guard_count } => write!(
                formatter,
                "sampled declared-module dual cannot cover {guard_count} opaque decorated-stratum guards without an exact sample-bound predicate witness"
            ),
            Self::QueryIsModularHit => formatter.write_str(
                "sampled declared-module dual requires a checked no-hit obstruction",
            ),
            Self::PartitionVerification(error) => {
                write!(formatter, "sampled-dual partition verification failed: {error}")
            }
            Self::PartitionNotVerified => {
                formatter.write_str("sampled-dual target partition failed cold verification")
            }
            Self::PartitionPlanMismatch => formatter.write_str(
                "sampled-dual partition is not bound to the exact fresh task plan",
            ),
            Self::SamplePlanMismatch => formatter
                .write_str("sampled-dual modular frame is not bound to the fresh task plan"),
            Self::ObstructionPlanMismatch => formatter
                .write_str("sampled-dual obstruction is not bound to the fresh task plan"),
            Self::ObstructionSampleMismatch => formatter.write_str(
                "sampled-dual obstruction and modular frame do not share one sample owner",
            ),
            Self::ObstructionPartitionMismatch => formatter.write_str(
                "sampled-dual obstruction projection is not exactly the exhaustive forbidden partition followed by target",
            ),
            Self::TargetColumnOutOfRange => {
                formatter.write_str("sampled-dual target column is outside its fresh plan")
            }
            Self::TargetColumnMismatch => formatter.write_str(
                "sampled-dual partition target differs from the fixed fresh-task target",
            ),
            Self::TargetShiftMismatch => formatter.write_str(
                "sampled-dual target column does not recover the fixed raw target shift",
            ),
            Self::FixedStratumMismatch => formatter.write_str(
                "sampled-dual partition differs from the fixed decorated stratum",
            ),
            Self::FixedOrderingMismatch => {
                formatter.write_str("sampled-dual partition uses a different ordering policy")
            }
            Self::FixedOwnerSnapshotMismatch => formatter.write_str(
                "sampled-dual partition uses a different immutable owner snapshot",
            ),
            Self::MaterializedSourceChronologyMismatch => formatter.write_str(
                "sampled-dual plan rows differ from its accumulated declared-source requests",
            ),
            Self::NominationIncidenceMismatch => formatter.write_str(
                "sampled-dual nominations belong to a different exact incidence index",
            ),
            Self::NominationIsTargetUnit => formatter.write_str(
                "target-unit bootstrap nominations cannot certify a sampled module dual",
            ),
            Self::NominationObstructionMismatch => formatter.write_str(
                "sampled-dual nominations belong to a different checked obstruction",
            ),
            Self::ResidualNominationMismatch => formatter.write_str(
                "sampled-dual residuals were not evaluated from these sealed nominations",
            ),
            Self::ResidualIncidenceMismatch => formatter.write_str(
                "sampled-dual residuals were evaluated against a different incidence index",
            ),
            Self::ResidualPlanMismatch => formatter.write_str(
                "sampled-dual residuals were evaluated against a different physical plan",
            ),
            Self::ResidualObstructionMismatch => formatter.write_str(
                "sampled-dual residuals were evaluated against a different obstruction",
            ),
            Self::ResidualSampleMismatch => formatter.write_str(
                "sampled-dual residuals were evaluated at a different modular sample",
            ),
            Self::NominationVerification(error) => {
                write!(formatter, "sampled-dual exhaustive census failed: {error}")
            }
            Self::IncompleteNominationCensus => formatter.write_str(
                "sampled-dual nominations omit or alter a structurally incident unseen row",
            ),
            Self::ResidualTelemetryMismatch => formatter.write_str(
                "sampled-dual residual telemetry does not census every exact nominated row",
            ),
            Self::ResidualPairingShiftOverflow {
                candidate_ordinal,
                term_ordinal,
                position,
                offset,
                source_shift,
            } => write!(
                formatter,
                "sampled-dual exact pairing shift {offset}+{source_shift} overflowed at candidate {candidate_ordinal}, term {term_ordinal}, component {position}"
            ),
            Self::CuttingResiduals { count } => write!(
                formatter,
                "sampled-dual census retained {count} nonzero cutting residuals"
            ),
            Self::RawObstructionMismatch => formatter.write_str(
                "sampled-dual obstruction could not be retained exactly by raw integral key",
            ),
            Self::RankDiagnosticsMismatch { detail } => write!(
                formatter,
                "sampled-dual plan-local rank diagnostics are inconsistent: {detail}"
            ),
            Self::Retention(error) => {
                write!(formatter, "sampled-dual owned evidence retention failed: {error}")
            }
        }
    }
}

impl std::error::Error for SampledDeclaredModuleDualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PartitionVerification(error) => Some(error),
            Self::IncidenceVerification(error)
            | Self::NominationVerification(error)
            | Self::Retention(error) => Some(error),
            _ => None,
        }
    }
}
