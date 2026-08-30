use crate::algebra::{IndexedAlgebraLimits, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
use crate::identity::{
    IntegralShift, ParametricIbpConfig, ParametricIbpGenerator, TranslatedSourceLimits,
};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};
use symbolica::prelude::Integer;

use super::super::canonical_family;
use super::support::complete_ordinary_sources;

const ORDINARY_SOURCE_COUNT: usize = 9;
const TOP_SECTOR: [i64; 6] = [1; 6];
const ANCHOR: [i64; 6] = [2; 6];
const TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 0, 1];

/// Derive one exact, sector-monotone top-sector recurrence from all nine
/// generic ordinary sources. This is deliberately test-only and does not
/// imply that the `K = 6` family is closed.
pub(super) fn derive_top_cell() -> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let family = canonical_family()?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, source_count) = complete_ordinary_sources(&generator)?;
    let translated = generator.translate_completed_source_rows(
        &completed,
        [IntegralShift::try_new([0; 6])?],
        TranslatedSourceLimits::default(),
    )?;
    let source_ordinals = (0..source_count).collect::<Vec<_>>();
    let sources =
        SourceViewBatch::try_select(translated, &source_ordinals, RuleCellLimits::default())?;
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &ANCHOR,
        &TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let application = top_application_domain(&rule)?;
    let context = generator.context().clone();
    let cell = RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        [],
        [],
        RuleCellLimits::default(),
    )?;
    drop(generator);
    Ok((context, cell))
}

fn top_application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_maximal_for_rule(
        Mask::try_from_indices(&TOP_SECTOR)?,
        rule.pivot().values(),
        &rhs,
    )?)
}

#[test]
fn exact_top_rule_is_generated_from_the_complete_ordinary_source_span() {
    let (_context, cell) = derive_top_cell().unwrap();
    let rule = cell.rule();
    assert_eq!(rule.anchor().powers(), ANCHOR);
    assert_eq!(rule.sector().active_bits(), [true; 6]);
    assert_eq!(rule.pivot().values(), TARGET_SHIFT);
    assert_eq!(
        rule.right_hand_side()
            .iter()
            .map(|term| term.shift().values())
            .collect::<Vec<_>>(),
        [
            &[0, 0, 1, 0, 0, -1][..],
            &[0, 0, 1, -1, 0, 0],
            &[0, 0, 0, 1, 0, -1],
            &[0, 0, 0, 1, -1, 0],
            &[0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, -1, 1],
            &[0, 0, 0, -1, 0, 1],
            &[0, 0, -1, 1, 0, 0],
            &[0, 0, -1, 0, 0, 1],
            &[0, -1, 1, 0, 0, 0],
            &[0, -1, 0, 0, 0, 1],
            &[-1, 0, 1, 0, 0, 0],
            &[-1, 0, 0, 1, 0, 0],
        ]
    );

    // The full nine-row source basis enters the target-directed derivation.
    // Exact elimination happens to need only its three k_i . d/dk_3 rows;
    // that sparse outcome is generated, not authored.
    assert_eq!(cell.sources().len(), ORDINARY_SOURCE_COUNT);
    assert_eq!(
        cell.sources()
            .provenance()
            .iter()
            .map(|source| source.translated().source_row().stable_string())
            .collect::<Vec<_>>(),
        [
            "ordinary-ibp:0:0",
            "ordinary-ibp:0:1",
            "ordinary-ibp:0:2",
            "ordinary-ibp:1:0",
            "ordinary-ibp:1:1",
            "ordinary-ibp:1:2",
            "ordinary-ibp:2:0",
            "ordinary-ibp:2:1",
            "ordinary-ibp:2:2",
        ]
    );
    assert_eq!(
        rule.source_combination()
            .iter()
            .map(|source| (source.source_ordinal(), source.row_id().stable_string()))
            .collect::<Vec<_>>(),
        [
            (2, "ordinary-ibp:0:2".to_owned()),
            (5, "ordinary-ibp:1:2".to_owned()),
            (8, "ordinary-ibp:2:2".to_owned()),
        ]
    );
    assert_eq!(rule.replay().source_rows_used(), 3);
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(cell.terms().len(), rule.right_hand_side().len());
    assert!(cell.terms().iter().all(|term| term.descent().verify()));
}

#[test]
fn exact_guards_and_application_box_require_excess_in_the_selected_edge() {
    let (context, cell) = derive_top_cell().unwrap();
    let expected_guards = [
        (-2, 0),
        (-1, 1),
        (-1, 2),
        (1, 3),
        (-3, 4),
        (2, 0),
        (4, 5),
        (-1, 0),
        (3, 1),
    ];
    assert_eq!(cell.rule().nonzero_guards().len(), expected_guards.len());
    let base_variables = context.base().parameter_names().len();
    for (guard, (expected_multiplier, expected_index)) in
        cell.rule().nonzero_guards().iter().zip(expected_guards)
    {
        let polynomial = guard.polynomial().raw();
        assert_eq!(polynomial.nterms(), 1);
        assert_eq!(
            polynomial.coefficients,
            [Integer::from(expected_multiplier)]
        );
        let exponents = polynomial.exponents_iter().next().unwrap();
        assert!(exponents[..base_variables].iter().all(|&power| power == 0));
        assert!(
            exponents[base_variables..]
                .iter()
                .enumerate()
                .all(|(index, &power)| power == u16::from(index == expected_index))
        );
    }
    assert_eq!(cell.guards().len(), 9);
    assert_eq!(
        cell.application_domain().bounds(),
        [
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
        ]
    );

    let ordinary_target = IntegralKey::try_new([1, 1, 1, 1, 1, 2]).unwrap();
    let assignment = cell
        .assignment_for_target(&ordinary_target)
        .unwrap()
        .expect("the selected sixth edge has one unit of reducible excess");
    assert_eq!(assignment, [1; 6]);
    assert!(cell.guards().iter().all(|guard| {
        !context
            .specialize_polynomial(
                guard.polynomial(),
                &assignment,
                IndexedAlgebraLimits::default(),
            )
            .unwrap()
            .is_zero()
    }));

    // The undotted scalar parent has no excess in the chosen edge. It is a
    // prospective master/boundary obligation, not an application point for
    // this recurrence.
    assert!(
        cell.assignment_for_target(&IntegralKey::try_new([1; 6]).unwrap())
            .unwrap()
            .is_none()
    );
    // Nor may this fixed top-sector cell leak into a pinch.
    assert!(
        cell.assignment_for_target(&IntegralKey::try_new([0, 1, 1, 1, 1, 2]).unwrap())
            .unwrap()
            .is_none()
    );
    // This coordinate is deliberately capped because two generated children
    // carry a +1 shift there; accepting it would overflow the i64 carrier.
    assert!(
        cell.assignment_for_target(&IntegralKey::try_new([1, 1, i64::MAX, 1, 1, 2]).unwrap())
            .unwrap()
            .is_none()
    );
}
