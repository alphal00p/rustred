//! Shared immutable candidate programs with independent application state.
use super::*;
use crate::reduction::terminal_normalization::TerminalAliasPlan;
use crate::solver::CandidateCacheRepresentation;

#[test]
fn empty_sessions_share_rules_but_not_cache_budgets_or_work() {
    let family = crate::solver::tests::tadpole();
    for representation in [
        CandidateCacheRepresentation::Sparse,
        CandidateCacheRepresentation::Factorized,
    ] {
        let mut original = candidate::<1>(&family, ReductionLimits::default());
        original.set_cache_representation(representation).unwrap();
        let expected = original
            .reduce_unit_mass_guarded(&key([4]), Default::default())
            .unwrap();
        let before = original.statistics();
        let mut isolated = original.fork_empty();
        assert!(Arc::ptr_eq(&original.program, &isolated.program));
        assert_eq!(isolated.cache_representation(), representation);
        assert_eq!(isolated.statistics(), Default::default());
        assert_eq!(isolated.limits(), original.limits());
        let result = isolated
            .reduce_unit_mass_guarded(&key([4]), Default::default())
            .unwrap();
        assert_eq!(result.0, expected.0);
        assert_eq!(result.1, expected.1);
        assert!(result.2.rule_applications() > 0);
        assert_eq!(original.statistics(), before);
        let isolated_before = isolated.statistics();
        original.clear_cache().unwrap();
        assert_eq!(isolated.statistics(), isolated_before);
        let warm = isolated
            .reduce_unit_mass_guarded(
                &key([4]),
                ReductionLimits {
                    max_rule_applications: 0,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(warm.0, expected.0);
        assert_eq!(warm.1, expected.1);
        assert_eq!(warm.2.rule_applications(), 0);
        assert_eq!(original.statistics().cached_integrals(), 0);
    }
}

#[test]
fn explicit_new_session_limits_do_not_change_source_or_entry_admission() {
    let family = crate::solver::tests::tadpole();
    let mut original = candidate::<1>(&family, ReductionLimits::default());
    let expected = original.reduce_unit_mass(&key([2])).unwrap();
    let before = original.statistics();
    let limits = ReductionLimits {
        max_rule_applications: 0,
        ..Default::default()
    };
    let mut limited = original.fork_empty_with_limits(limits);
    assert_eq!(limited.limits(), limits);
    assert!(matches!(
        limited.reduce_unit_mass(&key([2])),
        Err(CandidateReductionError::Application(
            ReductionError::RuleApplicationLimit { .. }
        ))
    ));
    assert!(matches!(
        limited.reduce_unit_mass(&key([2, 1])),
        Err(CandidateReductionError::Application(
            ReductionError::WrongArity { .. }
        ))
    ));
    assert_eq!(original.limits(), ReductionLimits::default());
    assert_eq!(original.statistics(), before);
    assert_eq!(original.reduce_unit_mass(&key([2])).unwrap(), expected);
}

#[test]
fn terminal_convention_changes_are_session_local_without_rule_cloning() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let family = artifact.family();
    let mut original = candidate::<3>(family, ReductionLimits::default());
    let mut raw_session = original.fork_empty();
    let plan = TerminalAliasPlan::independent_tadpole_products(
        family,
        original.terminals(),
        original.ordering(),
    )
    .unwrap();
    let target = plan.aliases().keys().next().unwrap().clone();
    let raw = raw_session.reduce_unit_mass(&target).unwrap();
    original.install_terminal_aliases(plan).unwrap();
    let mut normalized_session = original.fork_empty();
    assert!(Arc::ptr_eq(&original.program, &raw_session.program));
    assert!(Arc::ptr_eq(&original.program, &normalized_session.program));
    assert!(raw_session.terminal_aliases().is_none());
    assert!(Arc::ptr_eq(
        original.terminal_aliases.as_ref().unwrap(),
        normalized_session.terminal_aliases.as_ref().unwrap(),
    ));
    let normalized = original.reduce_unit_mass(&target).unwrap();
    let independent = normalized_session.reduce_unit_mass(&target).unwrap();
    assert_eq!(independent, normalized);
    assert_eq!(raw_session.reduce_unit_mass(&target).unwrap(), raw);
    assert_ne!(normalized.terms(), raw.terms());
}
