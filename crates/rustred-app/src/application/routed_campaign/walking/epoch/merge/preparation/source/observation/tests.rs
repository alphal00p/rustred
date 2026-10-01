use super::*;
use crate::application::routed_campaign::walking::queue::Phase;
use rustred::solver::DomainPowerBounds;

fn image(point: u64) -> CompactDomain<2> {
    CompactDomain::try_from_parts(
        Phase::Apply,
        [true, false],
        &[point, 0],
        &[Some(point), Some(0)],
        None,
        DomainPowerBounds::default(),
    )
    .unwrap()
}

fn unprobed() -> Context {
    Context::new(None, None, 7, 20)
}

fn observe(o: &mut Observer<2>, entry: usize, point: u64, context: Context) {
    let image = image(point);
    o.record(
        entry,
        image,
        image.digest().0,
        context,
        context.target.is_some(),
    );
}

#[test]
fn cross_entry_observation_distinguishes_geometry_and_validation_context() {
    let mut o = Observer::new(7, 20, 8, 8);
    let contexts = [
        Context::new(Some(1), Some((6, 10)), 7, 20),
        Context::new(Some(2), Some((6, 10)), 7, 20),
        Context::new(Some(1), Some((6, 11)), 7, 20),
        Context::new(Some(1), Some((5, 10)), 7, 20),
        Context::new(None, Some((7, 20)), 7, 20),
        Context::new(None, Some((6, 20)), 7, 20),
        unprobed(),
        Context::new(Some(1), Some((6, 10)), 7, 20),
    ];
    for (entry, context) in contexts.into_iter().enumerate() {
        observe(&mut o, entry, 1, context);
    }
    let s = o.finish();
    assert_eq!(s.unique_images, 1);
    assert_eq!(s.unique_contexts, 7);
    assert_eq!(s.cross_entry_geometry_repeats, 7);
    assert_eq!(s.cross_entry_same_context_repeats, 1);
    assert_eq!(s.stored_proposal.rows, 5);
    assert_eq!(s.stored_proposal.cross_entry_same_context_repeats, 1);
    assert_eq!(s.current_view_miss.rows, 1);
    assert_eq!(s.stale_or_unprobed_miss.rows, 2);
    assert!(!s.truncated);
}

#[test]
fn cross_entry_observation_same_entry_and_digest_collisions_are_not_cross_repeats() {
    let mut o = Observer::new(0, 2, 2, 5);
    o.record(0, image(1), 17, unprobed(), false);
    o.record(0, image(1), 17, unprobed(), false);
    o.record(1, image(2), 17, unprobed(), false);
    o.record(1, image(1), 17, unprobed(), false);
    o.record(1, image(1), 17, unprobed(), false);
    let s = o.finish();
    assert_eq!(s.observed_entries, 2);
    assert_eq!(s.unique_images, 2);
    assert_eq!(s.same_entry_geometry_repeats, 1);
    assert_eq!(s.cross_entry_geometry_repeats, 2);
    assert_eq!(s.cross_entry_same_context_repeats, 2);
    assert_eq!(s.comparison_count, 6);
}

#[test]
fn cross_entry_observation_small_and_large_census_are_only_sample_counts() {
    for extra in [0, 600] {
        let mut o = Observer::new(0, 3, 2, 8 + extra);
        for point in 0..5 + extra {
            observe(&mut o, 0, point as u64, unprobed());
        }
        for point in [0, 2, 10_000] {
            observe(&mut o, 1, point, unprobed());
        }
        let s = o.finish();
        assert_eq!(s.observed_rows, 8 + extra);
        assert_eq!(s.unique_images, 6 + extra);
        assert_eq!(s.cross_entry_geometry_repeats, 2);
        assert_eq!(s.cross_entry_same_context_repeats, 2);
        assert!(!s.truncated);
        assert!(s.allocated_capacity_bytes <= MAX_CAPACITY_BYTES);
    }
}

#[test]
fn cross_entry_observation_limits_censor_only_the_diagnostic() {
    let mut o = Observer::new(0, 1, 2, MAX_ROWS + 1);
    for _ in 0..MAX_ROWS {
        observe(&mut o, 0, 1, unprobed());
    }
    observe(&mut o, 1, 1, unprobed());
    let s = o.finish();
    assert_eq!(s.observed_rows, MAX_ROWS);
    assert_eq!(s.observed_entries, 1);
    assert_eq!(s.skipped_rows, 1);
    assert_eq!(s.cross_entry_geometry_repeats, 0);
    assert_eq!(s.stop_reason, Some("source_prefix_rows"));
    assert!(
        s.truncated,
        "prefix domination cannot establish no cross-entry repeats"
    );

    let mut o = Observer::new(0, 1, 2, 2);
    observe(&mut o, 0, 1, unprobed());
    o.sample.comparison_count = MAX_COMPARISONS;
    observe(&mut o, 1, 1, unprobed());
    let s = o.finish();
    assert_eq!(s.stop_reason, Some("comparison_budget"));
    assert_eq!(s.observed_rows, 1);
    assert_eq!(s.skipped_rows, 1);

    let mut o = Observer::new(0, 1, 2, 2);
    o.stop("allocation_refused");
    observe(&mut o, 0, 1, unprobed());
    assert_eq!(o.finish().observed_rows, 0);
}

#[test]
fn cross_entry_observation_off_and_global_version_sampling() {
    assert!(Observer::<2>::for_cut(false, 0, 1, 1, 1).is_none());
    assert!(Observer::<2>::for_cut(true, 9, 1, 1, 1).is_none());
    assert!(Observer::<2>::for_cut(true, 63, 1, 1, 1).is_none());
    assert!(Observer::<2>::for_cut(true, 64, 1, 1, 1).is_some());
    assert!(Observer::<2>::for_cut(true, 7, 1, 1, 1).is_some());
    let empty = Observer::<2>::new(0, 1, 0, 0).finish();
    assert_eq!(empty.allocated_capacity_bytes, 0);
    assert!(!empty.truncated);
}

#[test]
fn cross_entry_observation_totals_retain_only_bounded_non_authoritative_samples() {
    let mut totals = Totals::default();
    for version in 0..40 {
        totals.add(&Sample {
            epoch_version: version,
            observed_rows: 1,
            stored_proposal: Stratum {
                rows: 1,
                resolved_stored: 1,
                ..Stratum::default()
            },
            current_view_miss: Stratum {
                cross_entry_geometry_repeats: 2,
                ..Stratum::default()
            },
            stale_or_unprobed_miss: Stratum {
                cross_entry_same_context_repeats: 3,
                ..Stratum::default()
            },
            ..Sample::default()
        });
    }
    assert_eq!(totals.sampled_cuts, 40);
    assert_eq!(totals.observed_rows, 40);
    assert_eq!(totals.stored_proposal.rows, 40);
    assert_eq!(totals.stored_proposal.resolved_stored, 40);
    assert_eq!(totals.current_view_miss.cross_entry_geometry_repeats, 80);
    assert_eq!(
        totals
            .stale_or_unprobed_miss
            .cross_entry_same_context_repeats,
        120
    );
    assert_eq!(
        totals.retained_samples[0].as_ref().unwrap().epoch_version,
        24
    );
    assert_eq!(
        totals.retained_samples[15].as_ref().unwrap().epoch_version,
        39
    );
    assert!(totals.scope.contains("not necessarily published"));
}
