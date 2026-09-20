//! Owned scalar observations; never rule, coverage or publication authority.

use std::time::Duration;

use rustred::foundry::artifact::{SourcePortInstallEvent, SourcePortSuccessorSnapshot};
use rustred::solver::{SearchEvent, SectorEvent, SectorExecutionError, SectorPhase};

pub(in crate::application) type Observer<'a> =
    Option<&'a (dyn Fn(FamilyCloseProgress) + Send + Sync)>;

pub(in crate::application) fn emit(
    observer: Observer<'_>,
    event: impl FnOnce() -> FamilyCloseProgress,
) {
    if let Some(observer) = observer {
        observer(event());
    }
}

/// Live generation phase. No expression or source row is copied into progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyCloseGenerationStage {
    Case {
        pending: usize,
    },
    Discovery {
        depth: u32,
        seeds: usize,
        rows: usize,
    },
    ExactMaterialization,
    Canonicalization,
    GuardExtraction,
    ExceptionalGeometry,
    RuleFound {
        pending: usize,
    },
    Numerical {
        cases: usize,
    },
}

/// Lightweight live observations from complete family generation.
/// Candidate-only generation reuses preparation, generation and encoding
/// events, but never emits checking or installation events. No event grants
/// closure or provenance authority.
///
/// Sector masks use bit `i` for denominator coordinate `i`. Ordinals are
/// zero-based. `elapsed` is wall time since application entry, includes
/// observer work, and is never encoded in artifact bytes. Generation callbacks
/// may arrive concurrently and out of order; installation callbacks run on the
/// calling thread. A replay report is diagnostic, not a closure claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilyCloseProgress {
    /// Successor geometry only; never a claim that installation completed.
    SuccessorGeometry {
        sector: Option<u64>,
        snapshot: SourcePortSuccessorSnapshot,
        elapsed: Duration,
    },
    Preparing {
        arity: usize,
        elapsed: Duration,
    },
    Prepared {
        sectors: usize,
        /// Proved-zero sectors inside the explicit root domain.
        zero_sectors: usize,
        /// Global zero proofs also retained for translated-source replay.
        global_zero_sectors: usize,
        elapsed: Duration,
    },
    Generating {
        ordinal: usize,
        sector: u64,
        stage: FamilyCloseGenerationStage,
        elapsed: Duration,
    },
    GeneratedSector {
        ordinal: usize,
        sector: u64,
        rules: usize,
        finite_residuals: usize,
        elapsed: Duration,
    },
    /// A worker failed; other submitted jobs still run. This includes output
    /// failures after `GeneratedSector`, which does not imply durable storage.
    FailedSector {
        ordinal: usize,
        sector: u64,
        message: String,
        elapsed: Duration,
    },
    /// Structural reuse admission, not native algebra validation or closure.
    CheckpointPrepared {
        reused_sectors: usize,
        pending_sectors: usize,
        elapsed: Duration,
    },
    /// The completed candidate sector is durably stored. No rule certification.
    CheckpointedSector {
        ordinal: usize,
        sector: u64,
        bytes: usize,
        elapsed: Duration,
    },
    CheckingSector {
        ordinal: usize,
        sector: u64,
        rules: usize,
        elapsed: Duration,
    },
    /// Start exact checking of one proposed rule; not an admission result.
    CheckingRule {
        sector: u64,
        ordinal: usize,
        total: usize,
        elapsed: Duration,
    },
    CheckedSector {
        ordinal: usize,
        sector: u64,
        replayed_rules: usize,
        uncovered_boxes: usize,
        issues: usize,
        elapsed: Duration,
    },
    LoweringRule {
        sector: u64,
        ordinal: usize,
        total: usize,
        elapsed: Duration,
    },
    LoweredSector {
        sector: u64,
        cells: usize,
        elapsed: Duration,
    },
    Installing {
        sectors: usize,
        rule_cells: usize,
        terminals: usize,
        elapsed: Duration,
    },
    Installed {
        elapsed: Duration,
    },
    Encoding {
        elapsed: Duration,
    },
    Encoded {
        bytes: usize,
        elapsed: Duration,
    },
}

pub(in crate::application) fn generation_failure<const N: usize, E: std::fmt::Display>(
    error: &SectorExecutionError<N, E>,
    ordinal: usize,
    elapsed: Duration,
) -> FamilyCloseProgress {
    let message = match error {
        SectorExecutionError::Prepare { source, .. } => format!("preparation: {source}"),
        SectorExecutionError::Solve { source, .. } => format!("search: {source}"),
        SectorExecutionError::Consume { source, .. } => format!("output: {source}"),
    };
    FamilyCloseProgress::FailedSector {
        ordinal,
        sector: sector_mask(*error.sector()),
        message,
        elapsed,
    }
}

pub(in crate::application) fn sector_mask<const N: usize>(sector: [bool; N]) -> u64 {
    sector.iter().enumerate().fold(0, |mask, (axis, active)| {
        mask | (u64::from(*active) << axis)
    })
}

pub(in crate::application) fn generation_stage<const N: usize>(
    event: SectorEvent<'_, N>,
) -> FamilyCloseGenerationStage {
    match event {
        SectorEvent::CaseStarted { pending, .. } => FamilyCloseGenerationStage::Case { pending },
        SectorEvent::Search { event, .. } => match event {
            SearchEvent::DiscoveryProgress {
                depth, seeds, rows, ..
            } => FamilyCloseGenerationStage::Discovery { depth, seeds, rows },
            SearchEvent::ExactStarted { .. } | SearchEvent::ExactProgress(_) => {
                FamilyCloseGenerationStage::ExactMaterialization
            }
            SearchEvent::CanonicalizationStarted { .. } => {
                FamilyCloseGenerationStage::Canonicalization
            }
        },
        SectorEvent::PhaseStarted { phase, .. } => match phase {
            SectorPhase::GuardExtraction => FamilyCloseGenerationStage::GuardExtraction,
            SectorPhase::ExceptionalGeometry => FamilyCloseGenerationStage::ExceptionalGeometry,
        },
        SectorEvent::RuleFound { pending, .. } => FamilyCloseGenerationStage::RuleFound { pending },
        SectorEvent::NumericalStarted { cases } => {
            FamilyCloseGenerationStage::Numerical { cases: cases.len() }
        }
    }
}

pub(in crate::application) fn installation_event<const N: usize>(
    event: SourcePortInstallEvent<'_, N>,
    elapsed: Duration,
) -> FamilyCloseProgress {
    match event {
        SourcePortInstallEvent::SuccessorGeometry {
            sector, snapshot, ..
        } => FamilyCloseProgress::SuccessorGeometry {
            sector: sector.and_then(|bits| {
                bits.iter()
                    .enumerate()
                    .try_fold(0_u64, |mask, (axis, &active)| {
                        u64::from(active)
                            .checked_shl(u32::try_from(axis).ok()?)
                            .map(|bit| mask | bit)
                    })
            }),
            snapshot: *snapshot,
            elapsed,
        },
        SourcePortInstallEvent::CheckingSector {
            ordinal,
            sector,
            rules,
            ..
        } => FamilyCloseProgress::CheckingSector {
            ordinal,
            sector: sector_mask(sector),
            rules,
            elapsed,
        },
        SourcePortInstallEvent::CheckingRule {
            sector,
            ordinal,
            total,
            ..
        } => FamilyCloseProgress::CheckingRule {
            sector: sector_mask(sector),
            ordinal,
            total,
            elapsed,
        },
        SourcePortInstallEvent::CheckedSector {
            ordinal, report, ..
        } => FamilyCloseProgress::CheckedSector {
            ordinal,
            sector: sector_mask(report.sector),
            replayed_rules: report.exact_replayed_rules,
            uncovered_boxes: report.checked_rule_uncovered_boxes,
            issues: report.issues.len(),
            elapsed,
        },
        SourcePortInstallEvent::LoweringRule {
            sector,
            ordinal,
            total,
            ..
        } => FamilyCloseProgress::LoweringRule {
            sector: sector_mask(sector),
            ordinal,
            total,
            elapsed,
        },
        SourcePortInstallEvent::LoweredSector { sector, cells, .. } => {
            FamilyCloseProgress::LoweredSector {
                sector: sector_mask(sector),
                cells,
                elapsed,
            }
        }
        SourcePortInstallEvent::Installing {
            sectors,
            rule_cells,
            terminals,
            ..
        } => FamilyCloseProgress::Installing {
            sectors,
            rule_cells,
            terminals,
            elapsed,
        },
        SourcePortInstallEvent::Installed { .. } => FamilyCloseProgress::Installed { elapsed },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::foundry::artifact::{SourcePortSuccessorCounts, SourcePortSuccessorStage};

    #[test]
    fn successor_progress_copies_only_scalars_and_preserves_stage_and_failure() {
        let snapshot = SourcePortSuccessorSnapshot {
            stage: SourcePortSuccessorStage::ActualCells,
            sector_ordinal: None,
            rule_ordinal: Some(2),
            rhs_ordinal: Some(1),
            application_ordinal: Some(0),
            completed_sectors: 0,
            completed_cells: 2,
            consumed: SourcePortSuccessorCounts {
                boxes: 3,
                coordinate_cells: 18,
                work: 21,
            },
            limits: SourcePortSuccessorCounts {
                boxes: 4,
                coordinate_cells: 18,
                work: 21,
            },
            max_arity: 4096,
            max_uncovered_boxes: 4,
            max_uncovered_coordinate_cells: 18,
            failed_attempt: None,
            partition_requests: 3,
            singleton_partitions: 3,
            split_partitions: 0,
            total_pieces: 3,
            maximum_pieces: 1,
            source_degree_probes: 3,
            piece_degree_probes: 0,
            skipped_singleton_probes: 3,
            source_key_builds: 1,
            child_key_builds: 2,
            telemetry_overflow: false,
            arithmetic_overflow: false,
            failed: true,
            traversal_complete: false,
        };
        let elapsed = Duration::from_secs(9);
        assert_eq!(
            installation_event::<3>(
                SourcePortInstallEvent::SuccessorGeometry {
                    sector: Some(&[true, false, true]),
                    snapshot: &snapshot,
                    elapsed: Duration::ZERO,
                },
                elapsed
            ),
            FamilyCloseProgress::SuccessorGeometry {
                sector: Some(5),
                snapshot,
                elapsed
            }
        );
        // The core's borrowed sector does not acquire an app-width restriction
        // or a shift panic. An unrepresentable display mask remains absent.
        assert_eq!(
            installation_event::<65>(
                SourcePortInstallEvent::SuccessorGeometry {
                    sector: Some(&[true; 65]),
                    snapshot: &snapshot,
                    elapsed: Duration::ZERO,
                },
                elapsed
            ),
            FamilyCloseProgress::SuccessorGeometry {
                sector: None,
                snapshot,
                elapsed
            }
        );
    }
}
