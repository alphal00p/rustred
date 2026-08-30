use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::foundry::parametric::{ParametricRuleLimits, derive_sector_monotone_rule_for_target};
use crate::identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorMonotoneDomain};

use super::{
    FixedIndexRestriction, RuleCell, RuleCellDomainProof, RuleCellLimits, SourceViewBatch,
};

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
