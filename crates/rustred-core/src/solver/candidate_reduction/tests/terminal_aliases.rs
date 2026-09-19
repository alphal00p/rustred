use std::collections::BTreeMap;

use crate::foundry::artifact::{
    derive_one_loop_unit_mass_tadpole, derive_two_loop_unit_mass_sunset,
};
use crate::reduction::ReductionLimits;
use crate::reduction::terminal_normalization::TerminalAliasPlan;
use crate::sector::OrderingPolicy;

use super::{candidate, key};

#[test]
fn aliases_coalesce_exact_outputs_before_memoization_and_preserve_mass_factors() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let limits = ReductionLimits::default();
    let mut raw = candidate::<3>(family, limits);
    let mut normalized = candidate::<3>(family, limits);
    let plan =
        TerminalAliasPlan::independent_tadpole_products(family, raw.terminals(), raw.ordering())
            .unwrap();
    assert!(plan.statistics().verified_aliases > 0);
    assert!(raw.terminal_aliases().is_none());
    assert_eq!(raw.canonical_terminals(), raw.terminals());
    normalized.install_terminal_aliases(plan.clone()).unwrap();
    assert_eq!(normalized.terminals(), raw.terminals());
    assert_eq!(normalized.canonical_terminals(), plan.canonical_terminals());

    // Include all raw terminals themselves, raised powers, a numerator, zero,
    // and targets for which several distinct raw products can accumulate.
    let targets = raw
        .terminals()
        .iter()
        .cloned()
        .chain([
            key([1, 1, 1]),
            key([2, 1, 1]),
            key([1, 2, 3]),
            key([0, 2, 3]),
            key([-1, 2, 1]),
            key([3, 0, 2]),
            key([2, 2, -2]),
            key([0, 0, 0]),
        ])
        .collect::<Vec<_>>();
    let context = raw.coefficient_context().base();
    let context = context.clone();
    for target in targets {
        let before = raw.reduce_unit_mass(&target).unwrap();
        let actual = normalized.reduce_unit_mass(&target).unwrap();
        let mut expected = BTreeMap::new();
        for (terminal, coefficient) in before.terms() {
            let representative = plan.representative(family, terminal).unwrap();
            let old = expected
                .remove(representative)
                .unwrap_or_else(|| context.zero());
            let sum = context
                .try_add(&old, coefficient, limits.exact_algebra)
                .unwrap();
            if !sum.is_zero() {
                expected.insert(representative.clone(), sum);
            }
            if actual.terms().contains_key(representative) {
                assert_eq!(
                    before.common_mass_squared_power(terminal).unwrap(),
                    actual.common_mass_squared_power(representative).unwrap()
                );
            }
        }
        assert_eq!(actual.terms(), &expected, "target {:?}", target.powers());
        assert_eq!(actual.target(), &target);
        assert!(
            actual
                .terms()
                .keys()
                .all(|key| normalized.canonical_terminals().contains(key))
        );
        let statistics = normalized.statistics();
        assert_eq!(normalized.reduce_unit_mass(&target).unwrap(), actual);
        assert_eq!(
            normalized.statistics().rule_applications(),
            statistics.rule_applications()
        );
        assert_eq!(
            normalized.statistics().cache_hits(),
            statistics.cache_hits() + 1
        );
    }
    assert!(
        normalized.statistics().cached_coefficient_terms()
            <= raw.statistics().cached_coefficient_terms()
    );
    normalized.clear_cache().unwrap();
    assert_eq!(normalized.statistics().cached_integrals(), 0);
    assert_eq!(normalized.terminals(), raw.terminals());
    assert_eq!(normalized.canonical_terminals(), plan.canonical_terminals());
}

#[test]
fn alias_installation_rejects_foreign_family_order_and_terminal_sets_atomically() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut reducer = candidate::<3>(family, ReductionLimits::default());
    let valid = TerminalAliasPlan::independent_tadpole_products(
        family,
        reducer.terminals(),
        reducer.ordering(),
    )
    .unwrap();
    reducer.install_terminal_aliases(valid.clone()).unwrap();

    let other = derive_one_loop_unit_mass_tadpole().unwrap();
    let foreign = TerminalAliasPlan::independent_tadpole_products(
        other.family(),
        &std::collections::BTreeSet::from([key([1])]),
        reducer.ordering(),
    )
    .unwrap();
    let wrong_order = TerminalAliasPlan::independent_tadpole_products(
        family,
        reducer.terminals(),
        OrderingPolicy::RustRedUnshiftedV1,
    )
    .unwrap();
    let mut subset = reducer.terminals().clone();
    subset.pop_first();
    let wrong_set =
        TerminalAliasPlan::independent_tadpole_products(family, &subset, reducer.ordering())
            .unwrap();
    for plan in [foreign, wrong_order, wrong_set] {
        assert!(reducer.install_terminal_aliases(plan).is_err());
        assert_eq!(
            reducer.terminal_aliases().unwrap().raw_terminals(),
            valid.raw_terminals()
        );
        assert_eq!(reducer.canonical_terminals(), valid.canonical_terminals());
    }
}

#[test]
fn alias_installation_requires_explicit_cache_clear() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut reducer = candidate::<3>(family, ReductionLimits::default());
    let plan = TerminalAliasPlan::independent_tadpole_products(
        family,
        reducer.terminals(),
        reducer.ordering(),
    )
    .unwrap();
    let target = plan.aliases().keys().next().unwrap().clone();
    let raw = reducer.reduce_unit_mass(&target).unwrap();
    let statistics = reducer.statistics();
    assert!(reducer.install_terminal_aliases(plan.clone()).is_err());
    assert!(reducer.terminal_aliases().is_none());
    assert_eq!(reducer.statistics(), statistics);
    assert_eq!(reducer.reduce_unit_mass(&target).unwrap(), raw);
    reducer.clear_cache().unwrap();
    reducer.install_terminal_aliases(plan.clone()).unwrap();
    let normalized = reducer.reduce_unit_mass(&target).unwrap();
    assert_eq!(normalized.terms().len(), 1);
    assert!(
        normalized
            .terms()
            .contains_key(plan.representative(family, &target).unwrap())
    );
    assert!(!normalized.terms().contains_key(&target));
}
