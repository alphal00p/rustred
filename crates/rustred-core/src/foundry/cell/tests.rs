use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_monotone_rule_for_target};
use crate::identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    FixedIndexRestriction, RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch,
};
use super::{RuleCellError, build::validate_guard_on_bounds};

fn sunset_family() -> IntegralFamily {
    let base = CoefficientContext::new(["d"]);
    let zero = base.zero();
    let one = base.one();
    IntegralFamily::new(
        "rule-cell-sunset",
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                base.integer(-1),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                base.integer(-1),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(base.integer(-1), vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

#[test]
fn generated_rule_retains_sources_and_separate_application_proof() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let sources =
        SourceViewBatch::try_select(translated, &[0, 1, 2, 3], Default::default()).unwrap();
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &[1, 1, 1],
        &[0, 0, 1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let application = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&[1, 1, 1]).unwrap(),
        [
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ],
        rule.pivot().values(),
        &rhs,
    )
    .unwrap();
    let cell = RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        [],
        [],
        RuleCellLimits::default(),
    )
    .unwrap();
    assert_eq!(
        cell.domain_proof(),
        RuleCellDomainProof::ReprovedSectorMonotone
    );
    assert_eq!(cell.sources().len(), 4);
    assert_eq!(cell.terms().len(), cell.rule().right_hand_side().len());
    assert!(cell.terms().iter().all(|term| term.descent().verify()));
    assert_eq!(
        cell.assignment_for_target(&IntegralKey::try_new([1, 1, 2]).unwrap())
            .unwrap()
            .unwrap(),
        [1, 1, 1]
    );
}

#[test]
fn fixed_boundary_pruning_requires_an_identically_dead_coefficient() {
    let family = sunset_family();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0, 0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let sources =
        SourceViewBatch::try_select(translated, &[0, 1, 2, 3], Default::default()).unwrap();
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &[1, 1, 1],
        &[0, 0, 1],
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )
    .unwrap();
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let application = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&[1, 1, 1]).unwrap(),
        [
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ],
        rule.pivot().values(),
        &rhs,
    )
    .unwrap();
    let error = RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        [FixedIndexRestriction::new(0, 1)],
        [0],
        RuleCellLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        super::RuleCellError::PrunedTermNotZero { ordinal: 0 }
    ));
}

#[test]
fn guard_domains_are_proved_exactly_or_rejected_before_cell_installation() {
    let base = CoefficientContext::new(["d"]);
    let context =
        crate::algebra::IndexedCoefficientContext::try_new(&base, "rule-cell-guard-domain", 2)
            .unwrap();
    let d = context.lift(&base.parameter("d").unwrap()).unwrap();
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let one = context.one();
    let guard = |coefficient| {
        context
            .numerator_condition_with_limits(&coefficient, Default::default())
            .unwrap()
    };

    let root_at_one = guard(context.sub(&n0, &one).unwrap());
    assert_eq!(
        validate_guard_on_bounds(
            &context,
            7,
            &root_at_one,
            &[InteriorBounds::new(1, 4), InteriorBounds::new(1, 4)],
            Default::default(),
            Default::default(),
        ),
        Err(RuleCellError::GuardVanishesInApplicationDomain {
            ordinal: 7,
            position: 0,
            value: 1,
        })
    );
    validate_guard_on_bounds(
        &context,
        7,
        &root_at_one,
        &[InteriorBounds::new(2, 4), InteriorBounds::new(1, 4)],
        Default::default(),
        Default::default(),
    )
    .unwrap();

    let beyond_i64 = context
        .mul(&context.integer(i64::MAX), &context.integer(2))
        .unwrap();
    let root_beyond_i64 = guard(context.sub(&n0, &beyond_i64).unwrap());
    validate_guard_on_bounds(
        &context,
        9,
        &root_beyond_i64,
        &[
            InteriorBounds::new(i64::MIN, i64::MAX),
            InteriorBounds::new(i64::MIN, i64::MAX),
        ],
        Default::default(),
        Default::default(),
    )
    .unwrap();

    let multivariate = guard(context.add(&context.mul(&d, &n0).unwrap(), &n1).unwrap());
    assert_eq!(
        validate_guard_on_bounds(
            &context,
            11,
            &multivariate,
            &[InteriorBounds::new(1, 4), InteriorBounds::new(1, 4)],
            Default::default(),
            Default::default(),
        ),
        Err(RuleCellError::UnsupportedMultivariateGuardLocus { ordinal: 11 })
    );

    let identically_zero = guard(context.zero());
    assert_eq!(
        validate_guard_on_bounds(
            &context,
            12,
            &identically_zero,
            &[InteriorBounds::new(1, 4), InteriorBounds::new(1, 4)],
            Default::default(),
            Default::default(),
        ),
        Err(RuleCellError::GuardIdenticallyZero { ordinal: 12 })
    );

    let universally_nonzero = guard(context.add(&context.mul(&d, &d).unwrap(), &n0).unwrap());
    validate_guard_on_bounds(
        &context,
        13,
        &universally_nonzero,
        &[
            InteriorBounds::new(i64::MIN, i64::MAX),
            InteriorBounds::new(i64::MIN, i64::MAX),
        ],
        Default::default(),
        Default::default(),
    )
    .unwrap();

    for (ordinal, endpoint) in [(14, i64::MIN), (15, i64::MAX)] {
        let endpoint_root = guard(context.sub(&n0, &context.integer(endpoint)).unwrap());
        assert_eq!(
            validate_guard_on_bounds(
                &context,
                ordinal,
                &endpoint_root,
                &[
                    InteriorBounds::new(endpoint, endpoint),
                    InteriorBounds::new(1, 1),
                ],
                Default::default(),
                Default::default(),
            ),
            Err(RuleCellError::GuardVanishesInApplicationDomain {
                ordinal,
                position: 0,
                value: endpoint,
            })
        );
    }

    assert!(matches!(
        validate_guard_on_bounds(
            &context,
            17,
            &root_at_one,
            &[InteriorBounds::new(2, 4), InteriorBounds::new(1, 4)],
            Default::default(),
            crate::algebra::IndexedGuardLimits {
                max_input_terms: 0,
                ..Default::default()
            },
        ),
        Err(RuleCellError::GuardAlgebra {
            ordinal: 17,
            source: crate::algebra::IndexedAlgebraError::ResourceLimit {
                resource: "guard coefficient split input terms",
                requested: 2,
                limit: 0,
            },
        })
    ));
    assert!(matches!(
        validate_guard_on_bounds(
            &context,
            13,
            &universally_nonzero,
            &[InteriorBounds::new(1, 1)],
            Default::default(),
            Default::default(),
        ),
        Err(RuleCellError::WrongApplicationArity {
            expected: 2,
            actual: 1,
        })
    ));
}
