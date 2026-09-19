//! Independent application tests for the opt-in terminal normalization hook.

use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::reduction::terminal_normalization::TerminalAliasPlan;
use crate::reduction::{ReductionError, ReductionLimits};
use crate::solver::CandidateReductionError;

use super::{candidate, key};

#[test]
fn partial_cache_after_a_failed_request_prevents_alias_installation() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut reducer = candidate::<3>(
        family,
        ReductionLimits {
            max_cached_integrals: 1,
            ..Default::default()
        },
    );
    let plan = TerminalAliasPlan::independent_tadpole_products(
        family,
        reducer.terminals(),
        reducer.ordering(),
    )
    .unwrap();
    assert!(!plan.aliases().is_empty());
    // A raised product reduces down to a raw terminal before retaining its
    // ancestors. The deliberately tiny cache fails after that first insertion.
    assert!(matches!(
        reducer.reduce_unit_mass(&key([3, 0, 1])),
        Err(CandidateReductionError::Application(
            ReductionError::CacheLimit { limit: 1, .. }
        ))
    ));
    let partial_statistics = reducer.statistics();
    assert_eq!(partial_statistics.cached_integrals(), 1);
    assert!(reducer.install_terminal_aliases(plan.clone()).is_err());
    assert!(reducer.terminal_aliases().is_none());
    assert_eq!(reducer.statistics(), partial_statistics);
    assert_eq!(reducer.canonical_terminals(), reducer.terminals());

    reducer.clear_cache().unwrap();
    assert_eq!(reducer.statistics().cached_integrals(), 0);
    reducer.install_terminal_aliases(plan.clone()).unwrap();
    let source = plan.aliases().keys().next().unwrap().clone();
    let output = reducer.reduce_unit_mass(&source).unwrap();
    assert_eq!(output.target(), &source);
    assert_eq!(output.terms().len(), 1);
    assert!(
        output
            .terms()
            .contains_key(plan.representative(family, &source).unwrap())
    );
    assert_eq!(
        output
            .common_mass_squared_power(output.terms().keys().next().unwrap())
            .unwrap(),
        0
    );
}

#[test]
fn preparing_a_plan_does_not_change_default_reducer_outputs() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut reducer = candidate::<3>(family, ReductionLimits::default());
    let original_raw = reducer.terminals().clone();
    let original_statistics = reducer.statistics();
    let plan = TerminalAliasPlan::independent_tadpole_products(
        family,
        reducer.terminals(),
        reducer.ordering(),
    )
    .unwrap();
    assert!(!plan.aliases().is_empty());
    assert_eq!(reducer.statistics(), original_statistics);
    assert!(reducer.terminal_aliases().is_none());
    assert_eq!(reducer.terminals(), &original_raw);
    assert_eq!(reducer.canonical_terminals(), &original_raw);
    let source = plan.aliases().keys().next().unwrap().clone();
    let output = reducer.reduce_unit_mass(&source).unwrap();
    assert_eq!(output.terms().len(), 1);
    assert_eq!(output.terms()[&source], family.coefficient_context().one());
    assert!(
        !output
            .terms()
            .contains_key(plan.representative(family, &source).unwrap())
    );
    assert!(reducer.terminal_aliases().is_none());
    assert_eq!(reducer.canonical_terminals(), &original_raw);
}
