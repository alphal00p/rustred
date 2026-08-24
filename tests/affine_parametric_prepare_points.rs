use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::{
    AFFINE_PREPARE_POINT_LAYER_V1_SCHEMA, AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA,
    AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
    AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA, AffineParametricOrderingError,
    AffineParametricOrderingLimits, AffinePreparePointError, AffinePreparePointLayer,
    AffinePreparePointLimits, AffinePreparePointScheduleCertificate,
    AffinePreparePointScheduleError, AffinePreparePointScheduleLimits,
    AffineStartParametricEliminationOrdering, CoefficientContext,
    CoordinateEqualityLocusCertificate, CoordinateEqualityLocusExtractor,
    CoordinateEqualityLocusLimits, IndexShift, IntegralOrderingPolicy, ParametricCoefficient,
    ParametricCoefficientContext, RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
    ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapLimits, SectorMask,
    SymbolicPolynomialPredicateKind, SymbolicSectorCaseLimits, SymbolicSectorCasePartitionBuilder,
};
use symbolica::prelude::Integer;

fn context(scope: &str, arity: usize) -> ParametricCoefficientContext {
    ParametricCoefficientContext::try_new(&CoefficientContext::new(["d"]), scope, arity).unwrap()
}

fn dependent_equality_ordinal(source: &CoordinateEqualityLocusCertificate) -> usize {
    source
        .unresolved_predicates()
        .iter()
        .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
        .expect("the synthetic dependent equality must remain unresolved")
        .predicate_ordinal()
}

fn compile_map(
    context: &ParametricCoefficientContext,
    sector: SectorMask,
    literals: &[(usize, i64)],
    preliminary_nonzero: Option<&ParametricCoefficient>,
    predicate: &ParametricCoefficient,
    bound_position: usize,
) -> Arc<ResidualUnitAffineIndexMapCertificate> {
    let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
        context,
        sector,
        SymbolicSectorCaseLimits::default(),
    )
    .unwrap();
    let mut leaf = builder.root_case();
    for &(position, value) in literals {
        let literal = context
            .sub(&context.index(position).unwrap(), &context.integer(value))
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                context,
                leaf,
                context.numerator_condition(&literal).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
    }
    if let Some(nonzero) = preliminary_nonzero {
        leaf = builder
            .split_on_bad_polynomial(context, leaf, context.numerator_condition(nonzero).unwrap())
            .unwrap()
            .nonzero_case();
    }
    leaf = builder
        .split_on_bad_polynomial(
            context,
            leaf,
            context.numerator_condition(predicate).unwrap(),
        )
        .unwrap()
        .equal_zero_case();
    let partition = builder.finish(context).unwrap();
    let source = Arc::new(
        CoordinateEqualityLocusExtractor::extract(
            context,
            &partition,
            leaf,
            CoordinateEqualityLocusLimits::default(),
        )
        .unwrap(),
    );
    let ordinal = dependent_equality_ordinal(&source);
    Arc::new(
        ResidualUnitAffineIndexMapCertificate::compile(
            context,
            source,
            ordinal,
            bound_position,
            ResidualUnitAffineIndexMapLimits::default(),
        )
        .unwrap(),
    )
}

fn three_index_map(
    context: &ParametricCoefficientContext,
    with_extra_source_predicate: bool,
) -> Arc<ResidualUnitAffineIndexMapCertificate> {
    let sum = context
        .add(&context.index(0).unwrap(), &context.index(1).unwrap())
        .unwrap();
    let predicate = context.sub(&sum, &context.integer(3)).unwrap();
    let exclusion = context
        .sub(&context.index(1).unwrap(), &context.integer(1))
        .unwrap();
    compile_map(
        context,
        SectorMask::try_new([true, true, true]).unwrap(),
        &[(2, 2)],
        with_extra_source_predicate.then_some(&exclusion),
        &predicate,
        0,
    )
}

fn ordering(
    context: &ParametricCoefficientContext,
    map: Arc<ResidualUnitAffineIndexMapCertificate>,
    limits: AffineParametricOrderingLimits,
) -> AffineStartParametricEliminationOrdering {
    AffineStartParametricEliminationOrdering::try_new(
        context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_new([true, true, true]).unwrap(),
        map,
        limits,
    )
    .unwrap()
}

#[test]
fn full_matrix_rows_drive_formal_classification_and_signed_keys() {
    let context = context("affine-prepare-formal-rows", 3);
    let map = three_index_map(&context, false);
    let ordering = ordering(&context, map, AffineParametricOrderingLimits::default());
    ordering.replay(&context).unwrap();

    // F(t)=(3-t,t,2): the dependent bound n0 is not a literal, but it is
    // still symbolic because its complete A row is (-1). Only n2 is constant.
    assert_eq!(ordering.free_positions(), &[1]);
    assert_eq!(ordering.constant_positions(), &[2]);
    assert_eq!(ordering.symbolic_positions(), &[0, 1]);
    assert_eq!(ordering.constant_start_value(2), Some(&Integer::from(2)));
    assert_eq!(ordering.constant_start_value(0), None);
    assert_eq!(ordering.stats().matrix_entries_inspected(), 3);

    let shift = IndexShift::try_new([-10, 0, -2], 3).unwrap();
    let key = ordering.key_for_shift(&shift).unwrap();
    ordering.replay_key(&key).unwrap();
    assert_eq!(key.schema(), AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA);
    assert_eq!(
        key.key_schema(),
        RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA
    );
    assert_eq!(key.propagators(), 2);
    assert_eq!(key.formal_sector().active_bits(), &[true, true, false]);
    assert_eq!(key.corner_distance_offset(), &Integer::from(-10));
    assert_eq!(key.dots_offset(), &Integer::from(-10));
    assert_eq!(key.numerators_offset(), &Integer::from(0));
    assert_eq!(
        key.signed_index_excess(),
        &[Integer::from(-10), Integer::from(0), Integer::from(0)]
    );
    assert_eq!(key.retained_integer_bits(), 12);
    assert!(
        key.try_to_stable_string()
            .unwrap()
            .contains("integer-bits=12")
    );
}

#[test]
fn formal_order_matches_concrete_v1_at_multiple_same_chamber_specializations() {
    let context = context("affine-prepare-concrete-order-oracle", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let shifts = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [1, 1, 0],
        [2, 0, 0],
    ]
    .map(|values| IndexShift::try_new(values, 3).unwrap());

    // F(1)=(2,1,2) and F(2)=(1,2,2). Nonnegative translations keep both
    // symbolic rows in the source chamber at both specializations.
    for free_coordinate in [1i64, 2] {
        let start = [3 - free_coordinate, free_coordinate, 2];
        for left in &shifts {
            for right in &shifts {
                let concrete = |shift: &IndexShift| {
                    start
                        .iter()
                        .zip(shift.values())
                        .map(|(&value, &offset)| value.checked_add(offset).unwrap())
                        .collect::<Vec<_>>()
                };
                let concrete_left = concrete(left);
                let concrete_right = concrete(right);
                assert!(concrete_left.iter().all(|&value| value >= 1));
                assert!(concrete_right.iter().all(|&value| value >= 1));
                assert_eq!(
                    ordering.compare_shifts(left, right).unwrap(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1
                        .compare(&concrete_left, &concrete_right)
                        .unwrap(),
                    "t={free_coordinate}, left={:?}, right={:?}",
                    left.values(),
                    right.values(),
                );
            }
        }
    }
}

#[test]
fn extreme_signed_rustred_offsets_remain_exact_and_injective() {
    let context = context("affine-prepare-extreme-offsets", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let extremes = IndexShift::try_new([i64::MIN, i64::MAX, i64::MIN], 3).unwrap();
    let key = ordering.key_for_shift(&extremes).unwrap();
    ordering.replay_key(&key).unwrap();
    assert_eq!(key.formal_sector().active_bits(), &[true, true, false]);
    assert_eq!(
        key.signed_index_excess(),
        &[
            Integer::from(i64::MIN),
            Integer::from(i64::MAX),
            Integer::from(i64::MAX - 1),
        ]
    );
    let other = IndexShift::try_new([i64::MIN, i64::MAX, i64::MAX], 3).unwrap();
    assert_ne!(
        ordering.key_for_shift(&extremes).unwrap(),
        ordering.key_for_shift(&other).unwrap()
    );
    assert_ne!(
        ordering.compare_shifts(&extremes, &other).unwrap(),
        Ordering::Equal
    );
}

#[test]
fn ordering_binds_complete_source_identity_and_rejects_before_unbounded_work() {
    let context = context("affine-prepare-map-identity", 3);
    let plain_map = three_index_map(&context, false);
    let refined_map = three_index_map(&context, true);
    // The local certificate also binds case/predicate ordinals, while its
    // affine payload is the same. The complete source identity must still be
    // independently present and differ after source refinement.
    assert!(plain_map.local_manifest().contains("|b=3,0,2|A=-1,1,0"));
    assert!(refined_map.local_manifest().contains("|b=3,0,2|A=-1,1,0"));
    assert_ne!(
        plain_map.source_partition_identity(),
        refined_map.source_partition_identity()
    );
    let plain = ordering(
        &context,
        plain_map.clone(),
        AffineParametricOrderingLimits::default(),
    );
    assert!(Arc::ptr_eq(plain.affine_map().unwrap(), &plain_map));
    let refined = ordering(
        &context,
        refined_map,
        AffineParametricOrderingLimits::default(),
    );
    assert_ne!(plain.stable_manifest(), refined.stable_manifest());
    assert_ne!(plain, refined);

    let mut limits = AffineParametricOrderingLimits::default();
    limits.max_arity = 2;
    assert!(matches!(
        AffineStartParametricEliminationOrdering::try_new(
            &context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            SectorMask::try_new([true, true, true]).unwrap(),
            plain_map.clone(),
            limits,
        ),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine ordering arity",
            requested: 3,
            limit: 2,
        })
    ));
    assert!(matches!(
        AffineStartParametricEliminationOrdering::try_new(
            &context,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            SectorMask::try_new([false, true, true]).unwrap(),
            plain_map,
            AffineParametricOrderingLimits::default(),
        ),
        Err(AffineParametricOrderingError::SourceSectorMismatch)
    ));
}

#[test]
fn every_ordering_budget_rejects_one_below_its_complete_census() {
    let context = context("affine-prepare-ordering-budget-table", 3);
    let map = three_index_map(&context, false);
    let baseline = ordering(
        &context,
        map.clone(),
        AffineParametricOrderingLimits::default(),
    );
    let stats = baseline.stats();
    let sector = SectorMask::try_new([true, true, true]).unwrap();

    macro_rules! construction_budget {
        ($field:ident, $used:expr, $resource:literal) => {{
            let used = $used;
            assert!(used > 0, "{} must have a nonzero census", $resource);
            let mut limits = AffineParametricOrderingLimits::default();
            limits.$field = used - 1;
            assert!(matches!(
                AffineStartParametricEliminationOrdering::try_new(
                    &context,
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    sector.clone(),
                    map.clone(),
                    limits,
                ),
                Err(AffineParametricOrderingError::ResourceLimit { resource, .. })
                    if resource == $resource
            ));
        }};
    }

    construction_budget!(max_arity, stats.ambient_arity(), "affine ordering arity");
    construction_budget!(
        max_free_positions,
        stats.free_positions(),
        "affine ordering free positions"
    );
    construction_budget!(
        max_constant_positions,
        stats.constant_positions(),
        "constant affine positions"
    );
    construction_budget!(
        max_symbolic_positions,
        stats.symbolic_positions(),
        "symbolic affine positions"
    );
    construction_budget!(
        max_matrix_entries_inspected,
        stats.matrix_entries_inspected(),
        "affine matrix entries inspected"
    );
    construction_budget!(max_key_components, 3 * 3 + 5, "affine order-key components");
    construction_budget!(
        max_affine_integer_bits,
        stats.largest_affine_integer_bits(),
        "affine ordering integer bits"
    );
    construction_budget!(
        max_map_identity_bytes,
        stats.map_identity_bytes(),
        "affine map identity bytes"
    );
    let mut manifest_limit = 10_000usize;
    loop {
        let mut limits = AffineParametricOrderingLimits::default();
        limits.max_manifest_bytes = manifest_limit;
        let candidate = ordering(&context, map.clone(), limits);
        let rendered = candidate.stats().manifest_bytes();
        if rendered == manifest_limit {
            break;
        }
        manifest_limit = rendered;
    }
    construction_budget!(
        max_manifest_bytes,
        manifest_limit,
        "affine ordering manifest bytes"
    );

    let zero = IndexShift::try_new([0, 0, 0], 3).unwrap();
    let zero_key = baseline.key_for_shift(&zero).unwrap();
    let mut limits = AffineParametricOrderingLimits::default();
    limits.max_key_integer_bits = 2;
    let bounded = ordering(&context, map.clone(), limits);
    assert!(matches!(
        bounded.key_for_shift(&zero),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine key integer bits",
            requested: 3,
            limit: 2,
        })
    ));

    let mut limits = AffineParametricOrderingLimits::default();
    limits.max_key_total_integer_bits = zero_key.retained_integer_bits() - 1;
    let bounded = ordering(&context, map.clone(), limits);
    assert!(matches!(
        bounded.key_for_shift(&zero),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine key total integer bits",
            ..
        })
    ));

    // Find the exact complete diagnostic size at a stable decimal width, then
    // prove that its one-below byte budget fails before growing the String.
    let mut diagnostic_limit = 10_000usize;
    loop {
        let mut limits = AffineParametricOrderingLimits::default();
        limits.max_key_diagnostic_bytes = diagnostic_limit;
        let candidate = ordering(&context, map.clone(), limits);
        let rendered = candidate
            .key_for_shift(&zero)
            .unwrap()
            .try_to_stable_string()
            .unwrap();
        if rendered.len() == diagnostic_limit {
            break;
        }
        diagnostic_limit = rendered.len();
    }
    let mut limits = AffineParametricOrderingLimits::default();
    limits.max_key_diagnostic_bytes = diagnostic_limit - 1;
    let bounded = ordering(&context, map, limits);
    assert!(matches!(
        bounded.key_for_shift(&zero).unwrap().try_to_stable_string(),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine key diagnostic bytes",
            ..
        })
    ));
}

#[test]
fn folded_dependent_constants_are_arbitrary_precision_and_not_literal_only() {
    let context = context("affine-prepare-large-folded-constant", 2);
    let mut huge = context.integer(2);
    for _ in 0..8 {
        huge = context.mul(&huge, &huge).unwrap();
    }
    let huge_times_n1 = context.mul(&huge, &context.index(1).unwrap()).unwrap();
    let predicate = context
        .sub(&context.index(0).unwrap(), &huge_times_n1)
        .unwrap();
    let map = compile_map(
        &context,
        SectorMask::try_new([true, true]).unwrap(),
        &[(1, 2)],
        None,
        &predicate,
        0,
    );
    assert!(map.free_positions().is_empty());
    assert_eq!(map.literal_positions(), &[1]);
    assert!(matches!(map.constant(0), Some(Integer::Large(_))));

    let ordering = AffineStartParametricEliminationOrdering::try_new(
        &context,
        IntegralOrderingPolicy::RustRedUnshiftedV1,
        SectorMask::try_new([true, true]).unwrap(),
        map,
        AffineParametricOrderingLimits::default(),
    )
    .unwrap();
    // n0 is the dependent row folded through n1=2. It is constant even
    // though only n1 appeared in the extractor's literal-position table.
    assert_eq!(ordering.constant_positions(), &[0, 1]);
    assert!(ordering.symbolic_positions().is_empty());
    assert!(ordering.stats().largest_affine_integer_bits() > i128::BITS as usize);
    ordering.replay(&context).unwrap();
}

#[test]
fn exact_l1_shell_filters_only_constant_rows_and_is_stably_ordered() {
    let context = context("affine-prepare-exact-shell", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let layer = AffinePreparePointLayer::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointLimits::default(),
    )
    .unwrap();
    layer.replay(&context).unwrap();
    assert_eq!(layer.schema(), AFFINE_PREPARE_POINT_LAYER_V1_SCHEMA);
    assert_eq!(layer.stats().enumerated_offsets(), 18);
    assert_eq!(layer.stats().constant_sector_checks(), 18);
    assert_eq!(layer.stats().rejected_constant_sector_offsets(), 1);
    assert_eq!(layer.stats().retained_points(), 17);
    let values = layer
        .ordered_translations()
        .iter()
        .map(|shift| shift.values().to_vec())
        .collect::<Vec<_>>();
    assert!(!values.contains(&vec![0, 0, -2]));
    // n0 is a nonconstant affine row. It remains formal and is not rejected
    // by pretending that a particular t value represents the whole locus.
    assert!(values.contains(&vec![-2, 0, 0]));
    assert!(
        values
            .iter()
            .all(|values| { values.iter().map(|value| value.unsigned_abs()).sum::<u64>() == 2 })
    );
    assert_eq!(values.iter().cloned().collect::<BTreeSet<_>>().len(), 17);
    assert!(
        layer.ordered_translations().windows(2).all(|pair| {
            ordering.compare_shifts(&pair[0], &pair[1]).unwrap() != Ordering::Greater
        })
    );
}

#[test]
fn integer_key_payload_limits_apply_per_key_layer_and_diagnostic() {
    let context = context("affine-prepare-key-bounds", 3);
    let map = three_index_map(&context, false);
    let shift = IndexShift::try_new([-10, 0, -2], 3).unwrap();
    let baseline = ordering(
        &context,
        map.clone(),
        AffineParametricOrderingLimits::default(),
    );
    let key = baseline.key_for_shift(&shift).unwrap();

    let mut ordering_limits = AffineParametricOrderingLimits::default();
    ordering_limits.max_key_total_integer_bits = key.retained_integer_bits() - 1;
    let bounded = ordering(&context, map.clone(), ordering_limits);
    assert!(matches!(
        bounded.key_for_shift(&shift),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine key total integer bits",
            ..
        })
    ));

    let mut diagnostic_limits = AffineParametricOrderingLimits::default();
    diagnostic_limits.max_key_diagnostic_bytes = 32;
    let diagnostic_bounded = ordering(&context, map, diagnostic_limits);
    let diagnostic_key = diagnostic_bounded.key_for_shift(&shift).unwrap();
    assert!(matches!(
        diagnostic_key.try_to_stable_string(),
        Err(AffineParametricOrderingError::ResourceLimit {
            resource: "affine key diagnostic bytes",
            ..
        })
    ));

    let layer = AffinePreparePointLayer::compile(
        &context,
        baseline.clone(),
        2,
        AffinePreparePointLimits::default(),
    )
    .unwrap();
    assert!(layer.stats().order_key_integer_bits() > 0);
    let mut layer_limits = AffinePreparePointLimits::default();
    layer_limits.max_order_key_integer_bits = layer.stats().order_key_integer_bits() - 1;
    assert!(matches!(
        AffinePreparePointLayer::compile(&context, baseline, 2, layer_limits),
        Err(AffinePreparePointError::ResourceLimit {
            resource: "prepare-point order-key integer bits",
            ..
        })
    ));
}

#[test]
fn layer_and_schedule_push_their_remaining_bit_budget_into_key_construction() {
    let context = context("affine-prepare-internal-key-budget", 3);
    let mut ordering_limits = AffineParametricOrderingLimits::default();
    ordering_limits.max_key_total_integer_bits = 1_000_000;
    let ordering = ordering(&context, three_index_map(&context, false), ordering_limits);
    assert!(ordering.limits().max_key_total_integer_bits > 1);

    // At q=0 the exact retained key needs three magnitude bits. With only one
    // layer bit left, bounded key construction first charges the two one-bit
    // totals and fails at requested=2. A post-construction check would instead
    // report the already-materialized complete key size (3).
    let mut layer_limits = AffinePreparePointLimits::default();
    layer_limits.max_order_key_integer_bits = 1;
    assert_eq!(
        AffinePreparePointLayer::compile(&context, ordering.clone(), 0, layer_limits),
        Err(AffinePreparePointError::ResourceLimit {
            resource: "prepare-point order-key integer bits",
            requested: 2,
            limit: 1,
        })
    );

    let depth_zero = AffinePreparePointLayer::compile(
        &context,
        ordering.clone(),
        0,
        AffinePreparePointLimits::default(),
    )
    .unwrap();
    let consumed_at_depth_zero = depth_zero.stats().order_key_integer_bits();
    assert_eq!(consumed_at_depth_zero, 3);

    // The cumulative schedule leaves exactly one bit after depth zero. The
    // depth-one layer receives that exact remainder and the same in-key charge
    // is translated into cumulative typed fields without first retaining the
    // six-bit q=(0,0,+1) key.
    let mut schedule_limits = AffinePreparePointScheduleLimits::default();
    schedule_limits.max_order_key_integer_bits = consumed_at_depth_zero + 1;
    assert_eq!(
        AffinePreparePointScheduleCertificate::compile(&context, ordering, 1, schedule_limits,),
        Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
            depth: 1,
            resource: "prepare-point order-key integer bits",
            consumed_before_layer: consumed_at_depth_zero,
            requested_in_layer: 2,
            cumulative_requested: consumed_at_depth_zero + 2,
            cumulative_limit: consumed_at_depth_zero + 1,
        })
    );
}

#[test]
fn every_layer_budget_rejects_one_below_its_complete_census() {
    let context = context("affine-prepare-layer-budget-table", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let baseline = AffinePreparePointLayer::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();

    let limits = AffinePreparePointLimits {
        max_depth: 1,
        ..AffinePreparePointLimits::default()
    };
    assert!(matches!(
        AffinePreparePointLayer::compile(&context, ordering.clone(), 2, limits),
        Err(AffinePreparePointError::DepthTooLarge {
            requested: 2,
            limit: 1,
        })
    ));

    macro_rules! layer_budget {
        ($field:ident, $getter:ident, $resource:literal) => {{
            let used = stats.$getter();
            assert!(used > 0, "{} must have a nonzero census", $resource);
            let mut limits = AffinePreparePointLimits::default();
            limits.$field = used - 1;
            assert!(matches!(
                AffinePreparePointLayer::compile(&context, ordering.clone(), 2, limits),
                Err(AffinePreparePointError::ResourceLimit { resource, .. })
                    if resource == $resource
            ));
        }};
    }
    layer_budget!(
        max_enumeration_steps,
        enumeration_steps,
        "prepare-point enumeration steps"
    );
    layer_budget!(
        max_enumerated_offsets,
        enumerated_offsets,
        "enumerated prepare-point offsets"
    );
    layer_budget!(
        max_enumerated_components,
        enumerated_components,
        "enumerated prepare-point components"
    );
    layer_budget!(
        max_constant_sector_checks,
        constant_sector_checks,
        "constant-row sector checks"
    );
    layer_budget!(
        max_retained_points,
        retained_points,
        "retained prepare points"
    );
    layer_budget!(
        max_retained_components,
        retained_components,
        "retained prepare-point components"
    );
    layer_budget!(
        max_order_key_components,
        order_key_components,
        "prepare-point order-key components"
    );
    layer_budget!(
        max_order_key_integer_bits,
        order_key_integer_bits,
        "prepare-point order-key integer bits"
    );
    layer_budget!(
        max_order_comparisons,
        order_comparisons,
        "prepare-point order comparisons"
    );
    layer_budget!(
        max_order_comparison_integer_bit_work,
        order_comparison_integer_bit_work,
        "prepare-point order-comparison integer bit work"
    );
}

#[test]
fn cumulative_schedule_replays_exact_shells_and_enforces_remaining_budgets() {
    let context = context("affine-prepare-cumulative-schedule", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let schedule = AffinePreparePointScheduleCertificate::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointScheduleLimits::default(),
    )
    .unwrap();
    schedule.replay(&context).unwrap();
    assert_eq!(schedule.schema(), AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA);
    assert_eq!(schedule.through_depth(), 2);
    assert_eq!(schedule.layers().len(), 3);
    assert_eq!(
        schedule
            .layers()
            .iter()
            .map(|layer| layer.depth())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(schedule.stats().layer_count(), 3);
    assert_eq!(schedule.stats().enumerated_offsets(), 25);
    assert_eq!(schedule.stats().constant_sector_checks(), 25);
    assert_eq!(schedule.stats().rejected_constant_sector_offsets(), 1);
    assert_eq!(schedule.stats().retained_points(), 24);
    assert_eq!(
        schedule.stats().order_key_integer_bits(),
        schedule
            .layers()
            .iter()
            .map(|layer| layer.stats().order_key_integer_bits())
            .sum::<usize>()
    );
    assert_eq!(
        schedule.stats().order_comparison_integer_bit_work(),
        schedule
            .layers()
            .iter()
            .map(|layer| layer.stats().order_comparison_integer_bit_work())
            .sum::<usize>()
    );

    let mut limits = AffinePreparePointScheduleLimits::default();
    limits.max_order_key_integer_bits = schedule.stats().order_key_integer_bits() - 1;
    assert!(matches!(
        AffinePreparePointScheduleCertificate::compile(&context, ordering.clone(), 2, limits),
        Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
            resource: "prepare-point order-key integer bits",
            ..
        })
    ));

    let mut limits = AffinePreparePointScheduleLimits::default();
    limits.max_retained_points = schedule.stats().retained_points() - 1;
    assert!(matches!(
        AffinePreparePointScheduleCertificate::compile(&context, ordering, 2, limits),
        Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
            resource: "retained prepare points",
            ..
        })
    ));
}

#[test]
fn every_schedule_budget_rejects_one_below_its_cumulative_census() {
    let context = context("affine-prepare-schedule-budget-table", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );
    let baseline = AffinePreparePointScheduleCertificate::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointScheduleLimits::default(),
    )
    .unwrap();
    let stats = baseline.stats();

    let limits = AffinePreparePointScheduleLimits {
        max_depth: 1,
        ..AffinePreparePointScheduleLimits::default()
    };
    assert!(matches!(
        AffinePreparePointScheduleCertificate::compile(&context, ordering.clone(), 2, limits),
        Err(AffinePreparePointScheduleError::DepthTooLarge {
            requested: 2,
            limit: 1,
        })
    ));

    macro_rules! schedule_budget {
        ($field:ident, $getter:ident, $resource:literal) => {{
            let used = stats.$getter();
            assert!(used > 0, "{} must have a nonzero census", $resource);
            let mut limits = AffinePreparePointScheduleLimits::default();
            limits.$field = used - 1;
            assert!(matches!(
                AffinePreparePointScheduleCertificate::compile(
                    &context,
                    ordering.clone(),
                    2,
                    limits,
                ),
                Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
                    resource,
                    ..
                }) if resource == $resource
            ));
        }};
    }
    schedule_budget!(
        max_enumeration_steps,
        enumeration_steps,
        "prepare-point enumeration steps"
    );
    schedule_budget!(
        max_enumerated_offsets,
        enumerated_offsets,
        "enumerated prepare-point offsets"
    );
    schedule_budget!(
        max_enumerated_components,
        enumerated_components,
        "enumerated prepare-point components"
    );
    schedule_budget!(
        max_constant_sector_checks,
        constant_sector_checks,
        "constant-row sector checks"
    );
    schedule_budget!(
        max_retained_points,
        retained_points,
        "retained prepare points"
    );
    schedule_budget!(
        max_retained_components,
        retained_components,
        "retained prepare-point components"
    );
    schedule_budget!(
        max_order_key_components,
        order_key_components,
        "prepare-point order-key components"
    );
    schedule_budget!(
        max_order_key_integer_bits,
        order_key_integer_bits,
        "prepare-point order-key integer bits"
    );
    schedule_budget!(
        max_order_comparisons,
        order_comparisons,
        "prepare-point order comparisons"
    );
    schedule_budget!(
        max_order_comparison_integer_bit_work,
        order_comparison_integer_bit_work,
        "prepare-point order-comparison integer bit work"
    );
}

#[test]
fn comparison_integer_bit_work_accepts_exact_limits_and_rejects_one_below() {
    let context = context("affine-prepare-comparison-bit-work-boundaries", 3);
    let ordering = ordering(
        &context,
        three_index_map(&context, false),
        AffineParametricOrderingLimits::default(),
    );

    let baseline_layer = AffinePreparePointLayer::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointLimits::default(),
    )
    .unwrap();
    let layer_work = baseline_layer.stats().order_comparison_integer_bit_work();
    assert!(layer_work > 0);

    let mut exact_layer_limits = AffinePreparePointLimits::default();
    exact_layer_limits.max_order_comparison_integer_bit_work = layer_work;
    let exact_layer =
        AffinePreparePointLayer::compile(&context, ordering.clone(), 2, exact_layer_limits)
            .unwrap();
    assert_eq!(
        exact_layer.stats().order_comparison_integer_bit_work(),
        layer_work
    );

    let mut one_below_layer_limits = AffinePreparePointLimits::default();
    one_below_layer_limits.max_order_comparison_integer_bit_work = layer_work - 1;
    assert!(matches!(
        AffinePreparePointLayer::compile(
            &context,
            ordering.clone(),
            2,
            one_below_layer_limits,
        ),
        Err(AffinePreparePointError::ResourceLimit {
            resource: "prepare-point order-comparison integer bit work",
            requested,
            limit,
        }) if requested == layer_work && limit == layer_work - 1
    ));

    let baseline_schedule = AffinePreparePointScheduleCertificate::compile(
        &context,
        ordering.clone(),
        2,
        AffinePreparePointScheduleLimits::default(),
    )
    .unwrap();
    let schedule_work = baseline_schedule
        .stats()
        .order_comparison_integer_bit_work();
    assert!(schedule_work >= layer_work);

    let mut exact_schedule_limits = AffinePreparePointScheduleLimits::default();
    exact_schedule_limits.max_order_comparison_integer_bit_work = schedule_work;
    let exact_schedule = AffinePreparePointScheduleCertificate::compile(
        &context,
        ordering.clone(),
        2,
        exact_schedule_limits,
    )
    .unwrap();
    assert_eq!(
        exact_schedule.stats().order_comparison_integer_bit_work(),
        schedule_work
    );

    let mut one_below_schedule_limits = AffinePreparePointScheduleLimits::default();
    one_below_schedule_limits.max_order_comparison_integer_bit_work = schedule_work - 1;
    assert!(matches!(
        AffinePreparePointScheduleCertificate::compile(
            &context,
            ordering,
            2,
            one_below_schedule_limits,
        ),
        Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
            resource: "prepare-point order-comparison integer bit work",
            cumulative_requested,
            cumulative_limit,
            ..
        }) if cumulative_requested == schedule_work && cumulative_limit == schedule_work - 1
    ));
}

#[test]
fn public_schema_constants_match_certificate_payloads() {
    assert_eq!(
        AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA,
        "rustred-affine-start-parametric-elimination-ordering-v1"
    );
    assert_eq!(
        AFFINE_PREPARE_POINT_LAYER_V1_SCHEMA,
        "rustred-affine-prepare-point-layer-v1"
    );
    assert_eq!(
        AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA,
        "rustred-affine-prepare-point-schedule-v1"
    );
}
