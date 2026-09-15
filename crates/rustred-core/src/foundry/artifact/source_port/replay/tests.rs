//! Exact affine replay is deliberately not affine publication/coverage.

use super::super::SourcePortAudit;
use super::*;
use crate::solver::{SearchOptions, SectorConfig, SectorSolver};

fn coupled_sunset() -> (SourcePortAudit<3>, SectorRule<3>) {
    let family = crate::foundry::artifact::two_loop::canonical_family(Default::default()).unwrap();
    let audit = SourcePortAudit::try_new(&family, std::sync::Arc::from([])).unwrap();
    let context = audit.original_sources.context();
    let a = context.index(0).unwrap().raw().numerator.clone();
    let b = context.index(1).unwrap().raw().numerator.clone();
    let equality = &(&a - &b.mul_coeff(2.into())) + &a.one();
    let case = Case::generic()
        .intersect(&[equality], audit.sources.index_variables(), &[true; 3])
        .unwrap()
        .unwrap();
    assert!(case.affine().is_some());
    let solver = SectorSolver::new(&audit.sources, [true; 3], SectorConfig::default()).unwrap();
    let candidate = solver
        .solve_case(
            case,
            SearchOptions {
                max_depth: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let exceptions =
        extract_exceptions(&candidate, audit.sources.index_variables(), &[true; 3]).unwrap();
    (
        audit,
        SectorRule {
            candidate,
            exceptions,
        },
    )
}

fn replay(
    audit: &SourcePortAudit<3>,
    rule: &SectorRule<3>,
) -> Result<Replay<3>, SourcePortAuditError> {
    let solver = SectorSolver::new(&audit.sources, [true; 3], SectorConfig::default()).unwrap();
    replay_rule(
        &audit.sources,
        &audit.original_row_ids,
        &audit.original_sources,
        solver.basis(),
        solver.ordering(),
        &[],
        rule,
        &[],
    )
}

#[test]
fn affine_sunset_replays_against_original_ordinary_rows_but_cannot_publish() {
    let (audit, rule) = coupled_sunset();
    let result = replay(&audit, &rule).unwrap();
    assert!(!result.ordinary.contributions.is_empty());
    assert_eq!(
        result.ordinary.normalization,
        super::super::certificate::OriginalRowNormalization::OriginalGeneratorOrdinaryV1
    );
    assert!(result
        .ordinary
        .contributions
        .iter()
        .any(|source| source.offset != [0; 3]));
    assert!(matches!(
        geometry::application_boxes(&rule, audit.sources.index_variables(), &[true; 3], &[]),
        Err(SourcePortAuditError::UnsupportedAffineOwnership { .. })
    ));
}

#[test]
fn affine_sunset_replay_rejects_seed_corruption_and_identically_zero_poles() {
    let (audit, mut rule) = coupled_sunset();
    rule.candidate.sources[0].seed.shifts[0] += 1;
    assert!(replay(&audit, &rule)
        .err()
        .unwrap()
        .to_string()
        .contains("seed coefficient translation"));

    let (audit, mut rule) = coupled_sunset();
    assert!(!rule.candidate.rhs.is_empty());
    rule.candidate.rhs[0].coefficient.denominator =
        rule.candidate.case.affine().unwrap().equations()[0].clone();
    assert!(replay(&audit, &rule).is_err());
}
