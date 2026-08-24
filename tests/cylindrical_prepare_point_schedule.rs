use std::collections::BTreeSet;

use rustred::{
    CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA, CylindricalOrderingLimits,
    CylindricalParametricEliminationOrdering, CylindricalPreparePointScheduleCertificate,
    CylindricalPreparePointScheduleError, CylindricalPreparePointScheduleLimits,
    IntegralOrderingPolicy, PartialIndexAssignment, SectorMask,
};

fn ordering(
    sector: &str,
    assignment: impl IntoIterator<Item = (usize, i64)>,
) -> CylindricalParametricEliminationOrdering {
    let sector = SectorMask::try_from_bit_string(sector).unwrap();
    let assignment =
        PartialIndexAssignment::try_new(assignment, sector.arity(), sector.arity()).unwrap();
    CylindricalParametricEliminationOrdering::try_new(
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        sector,
        assignment,
        CylindricalOrderingLimits::default(),
    )
    .unwrap()
}

#[derive(Default)]
struct Census {
    enumeration_steps: usize,
    enumerated_offsets: usize,
    enumerated_components: usize,
    fixed_sector_checks: usize,
    rejected: usize,
    retained_points: usize,
    retained_components: usize,
    order_key_components: usize,
    order_comparisons: usize,
}

impl Census {
    fn add(&mut self, stats: rustred::CylindricalPreparePointStats) {
        self.enumeration_steps += stats.enumeration_steps();
        self.enumerated_offsets += stats.enumerated_offsets();
        self.enumerated_components += stats.enumerated_components();
        self.fixed_sector_checks += stats.fixed_sector_checks();
        self.rejected += stats.rejected_fixed_sector_offsets();
        self.retained_points += stats.retained_points();
        self.retained_components += stats.retained_components();
        self.order_key_components += stats.order_key_components();
        self.order_comparisons += stats.order_comparisons();
    }
}

#[test]
fn every_depth_is_bound_to_one_ordering_and_exact_remaining_allowance() {
    let ordering = ordering("10", [(0, 3)]);
    let limits = CylindricalPreparePointScheduleLimits::default();
    let schedule =
        CylindricalPreparePointScheduleCertificate::compile(ordering.clone(), 3, limits).unwrap();
    schedule.replay().unwrap();

    assert_eq!(
        schedule.schema(),
        CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA
    );
    assert_eq!(schedule.ordering(), &ordering);
    assert_eq!(schedule.through_depth(), 3);
    assert_eq!(schedule.layers().len(), 4);
    assert_eq!(schedule.stats().layer_count(), 4);

    let mut prior = Census::default();
    let mut all_translations = BTreeSet::new();
    for (depth, layer) in schedule.layers().iter().enumerate() {
        assert_eq!(layer.depth(), depth);
        assert_eq!(layer.ordering(), &ordering);
        let layer_limits = layer.limits();
        assert_eq!(layer_limits.max_depth, limits.max_depth);
        assert_eq!(
            layer_limits.max_enumeration_steps,
            limits.max_enumeration_steps - prior.enumeration_steps
        );
        assert_eq!(
            layer_limits.max_enumerated_offsets,
            limits.max_enumerated_offsets - prior.enumerated_offsets
        );
        assert_eq!(
            layer_limits.max_enumerated_components,
            limits.max_enumerated_components - prior.enumerated_components
        );
        assert_eq!(
            layer_limits.max_fixed_sector_checks,
            limits.max_fixed_sector_checks - prior.fixed_sector_checks
        );
        assert_eq!(
            layer_limits.max_retained_points,
            limits.max_retained_points - prior.retained_points
        );
        assert_eq!(
            layer_limits.max_retained_components,
            limits.max_retained_components - prior.retained_components
        );
        assert_eq!(
            layer_limits.max_order_key_components,
            limits.max_order_key_components - prior.order_key_components
        );
        assert_eq!(
            layer_limits.max_order_comparisons,
            limits.max_order_comparisons - prior.order_comparisons
        );

        for translation in layer.ordered_translations() {
            assert_eq!(
                translation
                    .values()
                    .iter()
                    .map(|value| value.unsigned_abs() as usize)
                    .sum::<usize>(),
                depth
            );
            assert!(all_translations.insert(translation.clone()));
            let fixed = 3i64.checked_add(translation.values()[0]).unwrap();
            assert!(fixed >= 1);
        }
        prior.add(layer.stats());
    }

    let stats = schedule.stats();
    assert_eq!(stats.enumeration_steps(), prior.enumeration_steps);
    assert_eq!(stats.enumerated_offsets(), prior.enumerated_offsets);
    assert_eq!(stats.enumerated_components(), prior.enumerated_components);
    assert_eq!(stats.fixed_sector_checks(), prior.fixed_sector_checks);
    assert_eq!(stats.rejected_fixed_sector_offsets(), prior.rejected);
    assert_eq!(stats.retained_points(), prior.retained_points);
    assert_eq!(stats.retained_components(), prior.retained_components);
    assert_eq!(stats.order_key_components(), prior.order_key_components);
    assert_eq!(stats.order_comparisons(), prior.order_comparisons);
}

#[test]
fn every_work_and_payload_limit_is_truly_cumulative_one_below() {
    // The fixed coordinate is sufficiently interior that every shell through
    // depth two survives.  Thus each counter has work in more than one layer.
    let make_ordering = || ordering("10", [(0, 5)]);
    let baseline = CylindricalPreparePointScheduleCertificate::compile(
        make_ordering(),
        2,
        CylindricalPreparePointScheduleLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();
    let cases = [
        ("prepare-point enumeration steps", stats.enumeration_steps()),
        (
            "enumerated prepare-point offsets",
            stats.enumerated_offsets(),
        ),
        (
            "enumerated prepare-point components",
            stats.enumerated_components(),
        ),
        (
            "fixed-coordinate sector checks",
            stats.fixed_sector_checks(),
        ),
        ("retained prepare points", stats.retained_points()),
        (
            "retained prepare-point components",
            stats.retained_components(),
        ),
        (
            "prepare-point order-key components",
            stats.order_key_components(),
        ),
        ("prepare-point order comparisons", stats.order_comparisons()),
    ];

    for (resource, observed) in cases {
        assert!(observed > 1, "fixture needs cumulative {resource} work");
        let one_below = observed - 1;
        let maximum_single_layer = baseline
            .layers()
            .iter()
            .map(|layer| match resource {
                "prepare-point enumeration steps" => layer.stats().enumeration_steps(),
                "enumerated prepare-point offsets" => layer.stats().enumerated_offsets(),
                "enumerated prepare-point components" => layer.stats().enumerated_components(),
                "fixed-coordinate sector checks" => layer.stats().fixed_sector_checks(),
                "retained prepare points" => layer.stats().retained_points(),
                "retained prepare-point components" => layer.stats().retained_components(),
                "prepare-point order-key components" => layer.stats().order_key_components(),
                "prepare-point order comparisons" => layer.stats().order_comparisons(),
                _ => unreachable!(),
            })
            .max()
            .unwrap();
        assert!(
            one_below >= maximum_single_layer,
            "each isolated layer must fit while cumulative {resource} fails"
        );

        let mut limits = CylindricalPreparePointScheduleLimits::default();
        match resource {
            "prepare-point enumeration steps" => limits.max_enumeration_steps = one_below,
            "enumerated prepare-point offsets" => limits.max_enumerated_offsets = one_below,
            "enumerated prepare-point components" => limits.max_enumerated_components = one_below,
            "fixed-coordinate sector checks" => limits.max_fixed_sector_checks = one_below,
            "retained prepare points" => limits.max_retained_points = one_below,
            "retained prepare-point components" => limits.max_retained_components = one_below,
            "prepare-point order-key components" => limits.max_order_key_components = one_below,
            "prepare-point order comparisons" => limits.max_order_comparisons = one_below,
            _ => unreachable!(),
        }
        assert!(matches!(
            CylindricalPreparePointScheduleCertificate::compile(make_ordering(), 2, limits),
            Err(CylindricalPreparePointScheduleError::CumulativeResourceLimit {
                depth,
                resource: actual,
                consumed_before_layer,
                cumulative_requested,
                cumulative_limit,
                ..
            }) if depth > 0
                && actual == resource
                && consumed_before_layer > 0
                && cumulative_requested == observed
                && cumulative_limit == one_below
        ));
    }
}

#[test]
fn depth_zero_is_retained_and_depth_ceiling_fails_before_any_layer() {
    let zero = CylindricalPreparePointScheduleCertificate::compile(
        ordering("1", []),
        0,
        CylindricalPreparePointScheduleLimits::default(),
    )
    .unwrap();
    assert_eq!(zero.layers().len(), 1);
    assert_eq!(zero.layers()[0].depth(), 0);
    assert_eq!(zero.stats().layer_count(), 1);

    let limits = CylindricalPreparePointScheduleLimits {
        max_depth: 1,
        ..CylindricalPreparePointScheduleLimits::default()
    };
    assert_eq!(
        CylindricalPreparePointScheduleCertificate::compile(ordering("1", []), 2, limits),
        Err(CylindricalPreparePointScheduleError::DepthTooLarge {
            requested: 2,
            limit: 1,
        })
    );
}

#[test]
fn schema_name_records_schedule_not_solver_or_grouped_numeric_claims() {
    assert_eq!(
        CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA,
        "rustred-cylindrical-prepare-point-schedule-v1"
    );
}
