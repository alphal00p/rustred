//! Independent adversarial coverage of the optional native coefficient cache.

use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::reduction::terminal_normalization::TerminalAliasPlan;
use crate::reduction::{ReductionError, ReductionLimits};
use crate::solver::{CandidateCacheRepresentation, CandidateReductionError, Integral};

use super::{affine_test_owner, candidate, from_one_rule, index_offset, key, one_rule};

#[test]
fn factorized_cache_preserves_exact_maps_aliases_and_mass_for_varied_targets() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    for normalize in [false, true] {
        let mut ordinary = candidate::<3>(family, ReductionLimits::default());
        let mut factorized = candidate::<3>(family, ReductionLimits::default());
        assert_eq!(
            ordinary.cache_representation(),
            CandidateCacheRepresentation::Sparse
        );
        assert_eq!(
            factorized.cache_representation(),
            CandidateCacheRepresentation::Sparse
        );
        factorized
            .set_cache_representation(CandidateCacheRepresentation::Factorized)
            .unwrap();
        if normalize {
            let plan = TerminalAliasPlan::vacuum_parametric_equivalences(
                family,
                ordinary.terminals(),
                ordinary.ordering(),
                Default::default(),
            )
            .unwrap();
            assert!(!plan.aliases().is_empty());
            ordinary.install_terminal_aliases(plan.clone()).unwrap();
            factorized.install_terminal_aliases(plan).unwrap();
        }
        let targets = ordinary
            .terminals()
            .iter()
            .cloned()
            .chain([
                key([2, 1, 1]),
                key([1, 2, 3]),
                key([0, 2, 3]),
                key([-1, 2, 1]),
                key([3, 0, 2]),
                key([2, 2, -2]),
                key([0, 0, 0]),
            ])
            .collect::<Vec<_>>();
        for target in targets {
            let expected = ordinary.reduce_unit_mass(&target).unwrap();
            let actual = factorized.reduce_unit_mass(&target).unwrap();
            assert_eq!(actual, expected, "target {:?}", target.powers());
            for (terminal, coefficient) in actual.terms() {
                let reference = &expected.terms()[terminal];
                assert_eq!(
                    coefficient.numerator.variables(),
                    reference.numerator.variables()
                );
                assert_eq!(
                    coefficient.denominator.variables(),
                    reference.denominator.variables()
                );
                assert_eq!(
                    coefficient.numerator.variables(),
                    family.coefficient_context().variables()
                );
                assert_eq!(
                    coefficient.denominator.variables(),
                    family.coefficient_context().variables()
                );
                assert_eq!(
                    actual.common_mass_squared_power(terminal).unwrap(),
                    expected.common_mass_squared_power(terminal).unwrap()
                );
            }
            let before = factorized.statistics();
            assert_eq!(factorized.reduce_unit_mass(&target).unwrap(), actual);
            let after = factorized.statistics();
            assert_eq!(after.rule_applications(), before.rule_applications());
            assert_eq!(after.coalescing_additions(), before.coalescing_additions());
            assert_eq!(after.cache_hits(), before.cache_hits() + 1);
            assert_eq!(after.cached_integrals(), before.cached_integrals());
            assert_eq!(
                after.cached_coefficient_terms(),
                before.cached_coefficient_terms()
            );
            assert_eq!(
                after.cached_coefficient_bytes(),
                before.cached_coefficient_bytes()
            );
        }
        assert_eq!(factorized.terminals(), ordinary.terminals());
        assert_eq!(
            factorized.canonical_terminals(),
            ordinary.canonical_terminals()
        );
        assert_eq!(
            factorized.statistics().rule_applications(),
            ordinary.statistics().rule_applications()
        );
        assert_eq!(
            factorized.statistics().coalescing_additions(),
            ordinary.statistics().coalescing_additions()
        );
        assert_eq!(
            factorized.statistics().cached_integrals(),
            ordinary.statistics().cached_integrals()
        );
    }
}

#[test]
fn a_cached_zero_still_prevents_representation_changes_until_explicit_clear() {
    let family = crate::solver::tests::tadpole();
    let mut owner = candidate::<1>(&family, ReductionLimits::default());
    owner
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    assert!(owner.reduce_unit_mass(&key([0])).unwrap().is_zero());
    let before = owner.statistics();
    assert_eq!(before.cached_integrals(), 1);
    assert_eq!(before.cached_coefficient_terms(), 0);
    assert_eq!(before.cached_coefficient_bytes(), 0);
    for requested in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        assert!(owner.set_cache_representation(requested).is_err());
        assert_eq!(
            owner.cache_representation(),
            CandidateCacheRepresentation::Factorized
        );
        assert_eq!(owner.statistics(), before);
    }
    owner.clear_cache().unwrap();
    owner
        .set_cache_representation(CandidateCacheRepresentation::Sparse)
        .unwrap();
    assert_eq!(
        owner.cache_representation(),
        CandidateCacheRepresentation::Sparse
    );
    assert_eq!(owner.statistics().cached_integrals(), 0);
    assert!(owner.reduce_unit_mass(&key([0])).unwrap().is_zero());
}

#[test]
fn partial_factorized_cache_failure_preserves_weights_and_blocks_reconfiguration() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut owner = candidate::<3>(
        family,
        ReductionLimits {
            max_cached_integrals: 1,
            ..Default::default()
        },
    );
    owner
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let plan = TerminalAliasPlan::vacuum_parametric_equivalences(
        family,
        owner.terminals(),
        owner.ordering(),
        Default::default(),
    )
    .unwrap();
    assert!(matches!(
        owner.reduce_unit_mass(&key([3, 0, 1])),
        Err(CandidateReductionError::Application(
            ReductionError::CacheLimit { limit: 1, .. }
        ))
    ));
    let partial = owner.statistics();
    assert_eq!(partial.cached_integrals(), 1);
    assert!(partial.cached_coefficient_terms() > 0);
    assert!(partial.cached_coefficient_bytes() > 0);
    assert!(
        owner
            .set_cache_representation(CandidateCacheRepresentation::Sparse)
            .is_err()
    );
    assert!(owner.install_terminal_aliases(plan.clone()).is_err());
    assert!(owner.terminal_aliases().is_none());
    assert_eq!(owner.statistics(), partial);
    owner.clear_cache().unwrap();
    assert_eq!(owner.statistics().cached_integrals(), 0);
    assert_eq!(owner.statistics().cached_coefficient_terms(), 0);
    assert_eq!(owner.statistics().cached_coefficient_bytes(), 0);
    owner.install_terminal_aliases(plan.clone()).unwrap();
    owner
        .set_cache_representation(CandidateCacheRepresentation::Sparse)
        .unwrap();
    let source = plan.aliases().keys().next().unwrap();
    let result = owner.reduce_unit_mass(source).unwrap();
    assert_eq!(result.terms().len(), 1);
    assert!(
        result
            .terms()
            .contains_key(plan.representative(family, source).unwrap())
    );
}

#[test]
fn factorized_cached_requests_recheck_source_conditions_arity_and_scope() {
    let family = crate::solver::tests::tadpole();
    let mut owner = candidate::<1>(&family, ReductionLimits::default());
    owner
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    owner.reduce_unit_mass(&key([2])).unwrap();
    let context = owner.coefficient_context().clone();
    owner.source_conditions.push(
        context
            .admit_native_polynomial_result_with_limits(
                index_offset(&context, 0, 2),
                Default::default(),
            )
            .unwrap(),
    );
    let before = owner.statistics();
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::SourceConditionVanished { .. })
    ));
    assert!(matches!(
        owner.reduce_unit_mass(&key([1, 2])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    owner.root_sector = [false];
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::OutsideRoot { .. })
    ));
    assert_eq!(owner.statistics(), before);
}

#[test]
fn factorization_cannot_erase_an_original_rule_pole() {
    let family = crate::solver::tests::tadpole();
    let mut owner = from_one_rule(&family, one_rule(&family));
    owner
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let context = owner.coefficient_context().clone();
    // Model a prepared rule whose simplified coefficient is regular but whose
    // original denominator still excludes this integer target. Its independent
    // pole record must remain authoritative before cache arithmetic is entered.
    let pole = context
        .admit_native_polynomial_result_with_limits(
            index_offset(&context, 0, 2),
            Default::default(),
        )
        .unwrap();
    let rules = owner.rules.get_mut(&[true]).unwrap();
    rules.truncate(1);
    rules[0].rhs[0].denominator = pole;
    assert!(matches!(
        owner.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert_eq!(owner.statistics().cached_integrals(), 0);
}

#[test]
fn factorized_mode_keeps_affine_equalities_and_exception_conjunctions() {
    let mut admitted = affine_test_owner(false);
    admitted
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let output = admitted.reduce_unit_mass(&key([2, 2, 1])).unwrap();
    assert_eq!(output.terms().len(), 1);
    assert!(output.terms().contains_key(&key([1, 2, 1])));
    for target in [[2, 3, 1], [2, 2, 2]] {
        assert!(matches!(
            admitted.reduce_unit_mass(&key(target)),
            Err(CandidateReductionError::Uncovered { .. })
        ));
    }
    let mut blocked = affine_test_owner(true);
    blocked
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    assert!(matches!(
        blocked.reduce_unit_mass(&key([2, 2, 1])),
        Err(CandidateReductionError::Uncovered { .. })
    ));
    assert_eq!(blocked.statistics().cached_integrals(), 0);
}

#[test]
fn factorized_mode_keeps_descent_and_checked_index_shifts() {
    let family = crate::solver::tests::tadpole();
    for target in [2, i64::MAX] {
        let mut solution = one_rule(&family);
        solution.rules.truncate(1);
        solution.rules[0].candidate.rhs[0].integral = Integral::symbolic([1]).unwrap();
        let mut owner = from_one_rule(&family, solution);
        owner
            .set_cache_representation(CandidateCacheRepresentation::Factorized)
            .unwrap();
        let error = owner.reduce_unit_mass(&key([target])).unwrap_err();
        if target == 2 {
            assert!(matches!(
                error,
                CandidateReductionError::NonDescending { .. }
            ));
        } else {
            assert!(matches!(
                error,
                CandidateReductionError::IndexOverflow { .. }
            ));
        }
        assert_eq!(owner.statistics().cached_integrals(), 0);
    }
}

#[test]
fn factorized_cache_term_and_byte_boundaries_fail_atomically() {
    let family = crate::solver::tests::tadpole();
    let mut measured = candidate::<1>(&family, ReductionLimits::default());
    measured
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    let expected = measured.reduce_unit_mass(&key([1])).unwrap();
    let terms = measured.statistics().cached_coefficient_terms();
    let bytes = measured.statistics().cached_coefficient_bytes();
    assert!(terms > 0 && bytes > 0);
    for (term_limit, byte_limit, succeeds) in [
        (terms, bytes, true),
        (terms - 1, bytes, false),
        (terms, bytes - 1, false),
    ] {
        let mut owner = candidate::<1>(
            &family,
            ReductionLimits {
                max_cached_coefficient_terms: term_limit,
                max_cached_coefficient_bytes: byte_limit,
                ..Default::default()
            },
        );
        owner
            .set_cache_representation(CandidateCacheRepresentation::Factorized)
            .unwrap();
        let result = owner.reduce_unit_mass(&key([1]));
        if succeeds {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(owner.statistics().cached_coefficient_terms(), terms);
            assert_eq!(owner.statistics().cached_coefficient_bytes(), bytes);
        } else {
            assert!(matches!(
                result,
                Err(CandidateReductionError::Application(
                    ReductionError::CacheCoefficientTermLimit { .. }
                        | ReductionError::CacheCoefficientByteLimit { .. }
                ))
            ));
            assert_eq!(owner.statistics().cached_integrals(), 0);
            assert_eq!(owner.statistics().cached_coefficient_terms(), 0);
            assert_eq!(owner.statistics().cached_coefficient_bytes(), 0);
            owner
                .set_cache_representation(CandidateCacheRepresentation::Sparse)
                .unwrap();
        }
    }
}

#[test]
fn finite_scope_reports_match_after_clearing_either_cache_representation() {
    let family = crate::solver::tests::tadpole();
    let mut ordinary = candidate::<1>(&family, ReductionLimits::default());
    let mut factorized = candidate::<1>(&family, ReductionLimits::default());
    factorized
        .set_cache_representation(CandidateCacheRepresentation::Factorized)
        .unwrap();
    ordinary.reduce_unit_mass(&key([8])).unwrap();
    factorized.reduce_unit_mass(&key([8])).unwrap();
    let targets = [key([-3]), key([0]), key([2]), key([2])];
    let expected = ordinary.check_targets(targets.clone()).unwrap();
    let actual = factorized.check_targets(targets).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.requested_targets(), 3);
    assert_eq!(actual.reachable_terminals(), 1);
    assert_eq!(actual.max_positive_power_sum(), 2);
    assert_eq!(actual.max_negative_index_degree(), 3);
    assert_eq!(
        factorized.statistics().cached_integrals(),
        ordinary.statistics().cached_integrals()
    );
    assert_eq!(
        factorized.cache_representation(),
        CandidateCacheRepresentation::Factorized
    );
}
