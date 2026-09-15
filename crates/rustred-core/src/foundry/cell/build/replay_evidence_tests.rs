use crate::foundry::artifact::{
    ClosedArtifact, derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::foundry::cell::FixedIndexSpecializationEvidence;
use crate::identity::{IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits};

use super::*;

fn construct_tadpole(
    artifact: &ClosedArtifact,
    constructor: usize,
    combined: bool,
    fixed: bool,
) -> Result<RuleCell, RuleCellError> {
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len()).map(|i| prepared.generate(i)).collect();
    let completed = prepared.complete(rows).unwrap();
    let translated = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let mut sources = SourceViewBatch::try_select(translated, &[0], Default::default()).unwrap();
    let mut rule = artifact.rules()[0].clone();
    let fixed = if fixed {
        vec![FixedIndexRestriction::new(0, 2)]
    } else {
        Vec::new()
    };
    if !fixed.is_empty() {
        // The underlying exact identity is valid before and after n=2.
        // These test-owned unchanged source rows have the same quotient
        // metadata as the fixed-source producer; the positive control below
        // establishes the cell is valid before swapping its replay kind.
        sources.construction =
            SourceViewConstruction::FixedIndexSpecialization(FixedIndexSpecializationEvidence {
                fixed: fixed.clone().into_boxed_slice(),
            });
    }
    let bounds = if fixed.is_empty() {
        rule.domain().bounds().to_vec()
    } else {
        vec![InteriorBounds::new(2, 2)]
    };
    let interior = SectorInteriorDomain::try_new(rule.sector().clone(), bounds.clone()).unwrap();
    let rhs = try_rhs_shifts(&rule, usize::MAX).unwrap();
    let application = SectorMonotoneDomain::try_new_for_rule(
        rule.sector().clone(),
        bounds,
        rule.pivot().values(),
        &rhs,
    )
    .unwrap();
    if combined {
        rule.replace_replay_with_uncertified_combined_domain_for_test();
    }
    match constructor {
        0 => RuleCell::try_tightened(
            generator.context(),
            rule,
            sources,
            interior,
            Default::default(),
        ),
        1 => RuleCell::try_refined(
            generator.context(),
            rule,
            sources,
            application,
            fixed,
            [],
            Default::default(),
        ),
        2 => RuleCell::try_refined_replay_authorized_pointwise_guards(
            generator.context(),
            rule,
            sources,
            application,
            fixed,
            [],
            Default::default(),
        ),
        _ => unreachable!(),
    }
}

#[test]
fn every_cell_constructor_rejects_only_the_changed_replay_kind() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    for constructor in 0..3 {
        construct_tadpole(&artifact, constructor, false, false).unwrap();
        assert!(matches!(
            construct_tadpole(&artifact, constructor, true, false),
            Err(RuleCellError::UnsupportedReplayEvidence)
        ));
    }
}

#[test]
fn combined_replay_cannot_inherit_fixed_or_residual_source_authority() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    for constructor in 1..3 {
        construct_tadpole(&artifact, constructor, false, true).unwrap();
        assert!(matches!(
            construct_tadpole(&artifact, constructor, true, true),
            Err(RuleCellError::UnsupportedReplayEvidence)
        ));
    }
    let sunset = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(sunset.family()).unwrap();
    let cell = sunset
        .rule_cells()
        .iter()
        .find(|cell| {
            matches!(
                cell.sources().construction(),
                SourceViewConstruction::ResidualProjection(_)
            )
        })
        .expect("the sealed sunset contains an authentic residual-projection cell");
    validate_bindings(generator.context(), cell.rule(), cell.sources()).unwrap();
    let mut rule = cell.rule().clone();
    rule.replace_replay_with_uncertified_combined_domain_for_test();
    assert_eq!(
        validate_bindings(generator.context(), &rule, cell.sources()),
        Err(RuleCellError::UnsupportedReplayEvidence)
    );
}

#[test]
fn anchor_directed_guard_split_rejects_combined_even_without_guards() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let mut rule = artifact.rules()[0].clone();
    rule.clear_nonzero_guards_for_test();
    let rhs = try_rhs_shifts(&rule, usize::MAX).unwrap();
    let application = SectorMonotoneDomain::try_new_for_rule(
        rule.sector().clone(),
        rule.domain().bounds().iter().copied(),
        rule.pivot().values(),
        &rhs,
    )
    .unwrap();
    assert!(
        try_single_guard_domain_split(
            generator.context(),
            &rule,
            &application,
            &[],
            Default::default(),
        )
        .unwrap()
        .is_none()
    );
    rule.replace_replay_with_uncertified_combined_domain_for_test();
    assert_eq!(
        try_single_guard_domain_split(
            generator.context(),
            &rule,
            &application,
            &[],
            Default::default(),
        ),
        Err(RuleCellError::UnsupportedReplayEvidence)
    );
}
