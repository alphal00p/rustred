use super::*;
use crate::reduction::ReductionStatistics;

fn filled(value: usize) -> GuardedRootCacheStatistics {
    GuardedRootCacheStatistics {
        ram_hits: value,
        backing_hits: value,
        descendant_ram_hits: value,
        descendant_backing_hits: value,
        completed_reductions: value,
        payload_validations: value,
        evicted_entries: value,
        evicted_coefficient_terms: value,
        evicted_coefficient_bytes: value,
        cached_integrals: value,
        cached_coefficient_terms: value,
        cached_coefficient_bytes: value,
        work: ReductionStatistics::default(),
    }
}

#[test]
fn telemetry_merge_saturates_every_counter_without_importing_work_census() {
    let mut work = ReductionStatistics::default();
    work.record_cache_hit();
    work.record_rule_application();
    work.record_coalescing_additions(1);
    for _ in 0..usize::BITS {
        work.merge_work(work);
    }
    assert_eq!(work.cache_hits(), usize::MAX);
    work.set_cache_census(100, 200, 300);
    let mut total = filled(usize::MAX - 1);
    total.work.set_cache_census(7, 8, 9);
    let mut incoming = filled(2);
    incoming.work = work;
    total.saturating_merge(incoming);
    let mut expected = filled(usize::MAX);
    expected.work = work;
    expected.work.set_cache_census(7, 8, 9);
    assert_eq!(total, expected);
    total.saturating_merge(incoming);
    assert_eq!(total, expected);
}

#[test]
fn telemetry_merge_sums_current_outer_roots_across_policies_not_transient_work_peaks() {
    let artifact = derive_one_loop_unit_mass_tadpole().unwrap();
    let program = ClosedUnitMassProgram::new(&artifact, [33; 32]);
    let mut total = GuardedRootCacheStatistics::default();
    let mut expected_bytes = 0;
    let mut expected_terms = 0;
    let mut expected_rules = 0;
    for policy in [
        CompletedCachePolicy::EvictLeastRecentlyUsed,
        CompletedCachePolicy::EvictAgedLeastFrequentlyUsed,
    ] {
        let mut cache = GuardedRootCache::with_policy(&program, Default::default(), policy);
        let result = program
            .reduce_guarded_root(&key(4), &mut cache, Default::default(), None)
            .unwrap();
        assert!(result.work.cached_integrals() > cache.statistics().cached_integrals);
        let snapshot = cache.statistics();
        expected_bytes += snapshot.cached_coefficient_bytes;
        expected_terms += snapshot.cached_coefficient_terms;
        expected_rules += snapshot.work.rule_applications();
        total.saturating_merge(snapshot);
        assert_eq!(cache.statistics(), snapshot);
        assert_eq!(cache.policy(), policy);
    }
    assert_eq!(total.cached_integrals, 2);
    assert_eq!(total.cached_coefficient_terms, expected_terms);
    assert_eq!(total.cached_coefficient_bytes, expected_bytes);
    assert_eq!(total.work.rule_applications(), expected_rules);
    assert_eq!(total.work.cached_integrals(), 0);
    assert_eq!(total.work.cached_coefficient_terms(), 0);
    assert_eq!(total.work.cached_coefficient_bytes(), 0);
}
