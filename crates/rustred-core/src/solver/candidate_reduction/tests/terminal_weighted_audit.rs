//! Independent failure-path checks for the opt-in weighted terminal leaf.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::reduction::terminal_normalization::TerminalNormalizationPlan;
use crate::reduction::{ReductionError, ReductionLimits};
use crate::sector::OrderingPolicy;
use crate::solver::{
    CandidateCacheRepresentation, CandidateReducer, CandidateReductionError, Integral,
    SectorSolution,
};

use super::{index_offset, key};

pub(super) fn fixture() -> (IntegralFamily, BTreeSet<IntegralKey>, IntegralKey) {
    let context = CoefficientContext::new(["d"]);
    let rows = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 2, 2, 1, 2, 1],
        [1, 2, 0, 1, 0, 0],
        [1, 0, 2, 0, 0, 1],
    ];
    let family = IntegralFamily::new(
        "independent_weighted_terminal_failure_paths",
        vec!["k1".into(), "k2".into(), "k3".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        rows.into_iter()
            .map(|row| {
                AffineDenominator::new(
                    context.integer(-1),
                    row.into_iter().map(|n| context.integer(n)).collect(),
                )
            })
            .collect(),
        vec![],
        vec![context.zero(); 6],
    )
    .unwrap();
    let target = key([1, 1, 1, 1, -1, 0]);
    let raw = BTreeSet::from([
        target.clone(),
        key([1, 1, 1, 1, 0, 0]),
        key([0, 1, 1, 1, 0, 0]),
        key([1, 0, 1, 1, 0, 0]),
        key([1, 1, 0, 1, 0, 0]),
        key([1, 1, 1, 0, 0, 0]),
    ]);
    (family, raw, target)
}

fn owner(
    family: &IntegralFamily,
    raw: &BTreeSet<IntegralKey>,
    plan: &TerminalNormalizationPlan,
    representation: CandidateCacheRepresentation,
) -> CandidateReducer<6> {
    let mut sectors = BTreeMap::<[bool; 6], Vec<Integral<6>>>::new();
    for key in raw {
        sectors
            .entry(std::array::from_fn(|i| key.powers()[i] > 0))
            .or_default()
            .push(
                Integral::numeric(std::array::from_fn(|i| {
                    i16::try_from(key.powers()[i]).unwrap()
                }))
                .unwrap(),
            );
    }
    let mut owner = CandidateReducer::try_new(
        family,
        [true; 6],
        OrderingPolicy::SpiredUncutV1,
        sectors.into_iter().map(|(sector, finite_residuals)| {
            (
                sector,
                SectorSolution {
                    order: crate::solver::IntegralOrder::new(sector, [false; 6]),
                    finite_case_policy: Default::default(),
                    max_numerator_rank: None,
                    rules: vec![],
                    finite_residuals,
                    stats: Default::default(),
                },
            )
        }),
        vec![],
        ReductionLimits::default(),
    )
    .unwrap();
    owner.set_cache_representation(representation).unwrap();
    owner.install_terminal_normalization(plan.clone()).unwrap();
    owner
}

#[test]
fn isolated_session_shares_weighted_convention_without_sharing_completed_cache() {
    let (family, raw, target) = fixture();
    let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.terms()[&target].len(), 2);
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut original = owner(&family, &raw, &plan, representation);
        let expected = original
            .reduce_unit_mass_guarded(&target, Default::default())
            .unwrap();
        let before = original.statistics();
        let mut isolated = original.fork_empty();
        assert!(std::sync::Arc::ptr_eq(&original.program, &isolated.program));
        assert!(std::sync::Arc::ptr_eq(
            original.terminal_normalization.as_ref().unwrap(),
            isolated.terminal_normalization.as_ref().unwrap(),
        ));
        assert_eq!(isolated.statistics(), Default::default());
        let result = isolated
            .reduce_unit_mass_guarded(&target, Default::default())
            .unwrap();
        assert_eq!(result.0, expected.0);
        assert_eq!(result.1, expected.1);
        assert_eq!(original.statistics(), before);
        isolated.clear_cache().unwrap();
        assert_eq!(original.statistics(), before);
    }
}

#[test]
fn cached_candidate_preserves_weighted_rows_guards_and_canonical_leaf_authority() {
    use crate::reduction::{CompletedCachePolicy, GuardedRootSource};
    use crate::solver::CandidateUnitMassProgram;
    let (family, raw, target) = fixture();
    let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.terms()[&target].len(), 2);
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut source = owner(&family, &raw, &plan, representation);
        let d = source.coefficient_context().base().parameter("d").unwrap();
        let source_guard = source
            .coefficient_context()
            .lift_base_polynomial(&d.numerator)
            .unwrap();
        std::sync::Arc::get_mut(&mut source.program)
            .unwrap()
            .source_conditions
            .push(source_guard);
        let expected = source
            .reduce_unit_mass_guarded(&target, Default::default())
            .unwrap();
        let program = CandidateUnitMassProgram::new(&source, [42; 32]);
        assert!(std::sync::Arc::strong_count(source.terminal_normalization.as_ref().unwrap()) >= 2);
        let mut cache = program.new_cache(
            Default::default(),
            CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
        );
        let (cold, unit) = program
            .reduce_guarded_root_with_decomposition(&target, &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(unit, expected.0);
        assert_eq!(cold.record.nonzero_conditions, expected.1);
        assert!(cold.record.nonzero_conditions.contains(&d));
        assert_eq!(cold.record.terms, plan.terms()[&target]);
        let warm = program
            .reduce_guarded_root(&target, &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(warm.source, GuardedRootSource::Ram);
        assert_eq!(warm.record, cold.record);
        let mut corrupted = cold.record.clone();
        corrupted
            .terms
            .remove(corrupted.terms.keys().next().cloned().as_ref().unwrap());
        assert!(
            program
                .validate_completed_record(&corrupted, Default::default())
                .is_err()
        );
        let canonical = plan.canonical_terminals().iter().next().unwrap();
        let leaf = program
            .reduce_guarded_root(canonical, &mut cache, Default::default(), None)
            .unwrap();
        assert_eq!(
            leaf.record.terms,
            BTreeMap::from([(canonical.clone(), program.context().one())])
        );
        let mut corrupted = leaf.record;
        corrupted
            .terms
            .insert(canonical.clone(), program.context().integer(2));
        assert!(
            program
                .validate_completed_record(&corrupted, Default::default())
                .is_err()
        );
    }
}

#[test]
fn weighted_terminal_and_cached_output_cannot_bypass_an_inherited_source_pole() {
    let (family, raw, target) = fixture();
    let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.terms()[&target].len(), 2);
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        for cached in [false, true] {
            let mut owner = owner(&family, &raw, &plan, representation);
            if cached {
                assert_eq!(
                    owner.reduce_unit_mass(&target).unwrap().terms(),
                    &plan.terms()[&target]
                );
            }
            let context = owner.coefficient_context();
            let condition = context
                .admit_native_polynomial_result_with_limits(
                    index_offset(context, 4, -1),
                    Default::default(),
                )
                .unwrap();
            std::sync::Arc::get_mut(&mut owner.program)
                .unwrap()
                .source_conditions
                .push(condition);
            let before = owner.statistics();
            assert!(matches!(
                owner.reduce_unit_mass(&target),
                Err(CandidateReductionError::SourceConditionVanished { .. })
            ));
            assert_eq!(owner.statistics(), before);
            assert_eq!(owner.cache.contains_key(&target), cached);
            assert_eq!(owner.terminals(), &raw);
            assert!(owner.terminal_normalization().is_some());
        }
    }
}

#[test]
fn weighted_multiterm_cache_budget_failure_preserves_existing_cache_and_accounting() {
    let (family, raw, target) = fixture();
    let plan = TerminalNormalizationPlan::vacuum_quadratic_numerators(
        &family,
        &raw,
        OrderingPolicy::SpiredUncutV1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.terms()[&target].len(), 2);
    let retained = plan.canonical_terminals().iter().next().unwrap();
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut measured = owner(&family, &raw, &plan, representation);
        let expected = measured.reduce_unit_mass(&target).unwrap();
        let row_terms = measured.statistics().cached_coefficient_terms();
        let row_bytes = measured.statistics().cached_coefficient_bytes();
        assert!(row_terms > 0 && row_bytes > 0);
        for restrict_terms in [true, false] {
            let mut owner = owner(&family, &raw, &plan, representation);
            let retained_result = owner.reduce_unit_mass(retained).unwrap();
            let before = owner.statistics();
            if restrict_terms {
                owner.limits.max_cached_coefficient_terms =
                    before.cached_coefficient_terms() + row_terms - 1;
            } else {
                owner.limits.max_cached_coefficient_bytes =
                    before.cached_coefficient_bytes() + row_bytes - 1;
            }
            let error = owner.reduce_unit_mass(&target).unwrap_err();
            assert!(matches!(
                error,
                CandidateReductionError::Application(
                    ReductionError::CacheCoefficientTermLimit { .. }
                        | ReductionError::CacheCoefficientByteLimit { .. }
                )
            ));
            assert_eq!(owner.statistics(), before);
            assert!(!owner.cache.contains_key(&target));
            assert!(owner.cache.contains_key(retained));
            assert!(owner.install_terminal_normalization(plan.clone()).is_err());
            owner.limits = ReductionLimits::default();
            assert_eq!(owner.reduce_unit_mass(retained).unwrap(), retained_result);
            assert_eq!(owner.reduce_unit_mass(&target).unwrap(), expected);
            assert_eq!(owner.statistics().cached_integrals(), 2);
            assert_eq!(
                owner.statistics().cached_coefficient_terms(),
                before.cached_coefficient_terms() + row_terms
            );
            assert_eq!(
                owner.statistics().cached_coefficient_bytes(),
                before.cached_coefficient_bytes() + row_bytes
            );
        }
    }
}
