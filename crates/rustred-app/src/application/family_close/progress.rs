//! Owned scalar observations; never rule, coverage or publication authority.

use std::time::Duration;

use rustred::foundry::artifact::SourcePortInstallEvent;
use rustred::solver::{SearchEvent, SectorEvent, SectorPhase};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamilyCloseProgress {
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

pub(super) fn installation_event<const N: usize>(
    event: SourcePortInstallEvent<'_, N>,
    elapsed: Duration,
) -> FamilyCloseProgress {
    match event {
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
