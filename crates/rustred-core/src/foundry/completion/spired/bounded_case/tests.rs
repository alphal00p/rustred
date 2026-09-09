use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::completion::stratum::{
    DecoratedStratum, GuardBranch, GuardBranchIdentity, StratumRegistryLimits,
};
use crate::identity::{CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator};
use crate::sector::{InteriorBounds, Mask, SectorMonotoneDomain};

use super::{
    SpiredBoundedCaseAxisDisposition, SpiredBoundedCaseEnvelopeError,
    SpiredBoundedCaseEnvelopeLimits, try_build_spired_bounded_case_envelope,
};
use crate::foundry::completion::spired::SpiredCoordinateCaseObligation;

fn unit_tadpole(name: &str) -> IntegralFamily {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        Vec::new(),
        vec![base.zero()],
    )
    .unwrap()
}

fn equal_mass_sunset(name: &str) -> IntegralFamily {
    let base = CoefficientContext::try_new(["d", "s"]).unwrap();
    let zero = base.zero();
    let one = base.one();
    let minus_s = base
        .try_neg(&base.parameter("s").unwrap(), Default::default())
        .unwrap();
    IntegralFamily::new(
        name,
        vec!["k1".into(), "k2".into()],
        Vec::new(),
        base.clone(),
        base.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(
                minus_s.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_s.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_s, vec![one.clone(), base.integer(2), one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
    )
    .unwrap()
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn parent(
    sources: &CompletedIbpSourceRows,
    active: &[bool],
    bounds: &[(i64, i64)],
) -> SpiredCoordinateCaseObligation {
    SpiredCoordinateCaseObligation::try_new_root(parent_stratum_with_guards(
        sources,
        active,
        bounds,
        [],
    ))
    .unwrap()
}

fn parent_stratum_with_guards(
    sources: &CompletedIbpSourceRows,
    active: &[bool],
    bounds: &[(i64, i64)],
    guards: impl IntoIterator<Item = GuardBranchIdentity>,
) -> DecoratedStratum {
    assert_eq!(active.len(), bounds.len());
    let zero = vec![0_i64; active.len()];
    let domain = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_new(active.iter().copied()).unwrap(),
        bounds
            .iter()
            .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
        &zero,
        &[] as &[&[i64]],
    )
    .unwrap();
    DecoratedStratum::try_new(
        sources.family_fingerprint(),
        sources.context_fingerprint(),
        domain,
        guards,
        StratumRegistryLimits::default(),
    )
    .unwrap()
}

fn target(values: impl IntoIterator<Item = i64>) -> IntegralShift {
    IntegralShift::try_new(values).unwrap()
}

fn bounds(stratum: &DecoratedStratum) -> Vec<(i64, i64)> {
    stratum
        .domain()
        .bounds()
        .iter()
        .map(|bound| (bound.lower(), bound.upper()))
        .collect()
}

fn contains(stratum: &DecoratedStratum, point: &[i64]) -> bool {
    stratum.domain().contains(point).unwrap()
}

#[test]
fn depth_zero_and_one_have_exact_target_relative_envelopes() {
    let family = unit_tadpole("bounded-case-depth");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent_case = parent(&sources, &[false], &[(-4, 0)]);

    let depth_zero = try_build_spired_bounded_case_envelope(
        &parent_case,
        &sources,
        &target([0]),
        0,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        depth_zero.declared_carrier(),
        parent_case.declared_carrier().id()
    );
    assert_eq!(depth_zero.parent(), parent_case.stratum().id());
    assert_eq!(bounds(depth_zero.bulk().stratum()), vec![(-4, -1)]);
    assert_eq!(
        depth_zero
            .equality_faces()
            .iter()
            .map(|face| (face.position(), face.value()))
            .collect::<Vec<_>>(),
        vec![(0, 0)]
    );
    let axis = depth_zero.shift_envelope().axes()[0];
    assert_eq!(axis.max_positive_relative_shift(), 1);
    assert_eq!(axis.retained_bulk_bounds(), InteriorBounds::new(-4, -1));
    assert_eq!(
        axis.excluded_boundary_bounds(),
        Some(InteriorBounds::new(0, 0))
    );

    let depth_one = try_build_spired_bounded_case_envelope(
        &parent_case,
        &sources,
        &target([0]),
        1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(depth_one.shift_envelope().max_source_depth(), 1);
    assert_eq!(
        depth_one.shift_envelope().axes()[0].max_positive_relative_shift(),
        2
    );
    assert_eq!(bounds(depth_one.bulk().stratum()), vec![(-4, -2)]);
    assert_eq!(
        depth_one
            .equality_faces()
            .iter()
            .map(|face| face.value())
            .collect::<Vec<_>>(),
        vec![-1, 0]
    );
    assert_eq!(depth_one.census().split_axes(), 1);
    assert_eq!(depth_one.census().equality_faces(), 2);
    assert_eq!(depth_one.census().retained_domains(), 3);
}

#[test]
fn target_shift_is_subtracted_before_the_positive_envelope_is_taken() {
    let family = unit_tadpole("bounded-case-target-relative");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent = parent(&sources, &[false], &[(-4, -1)]);

    let envelope = try_build_spired_bounded_case_envelope(
        &parent,
        &sources,
        &target([1]),
        0,
        Default::default(),
    )
    .unwrap();
    let axis = envelope.shift_envelope().axes()[0];
    assert_eq!(axis.max_positive_relative_shift(), 0);
    assert_eq!(axis.base_safe_upper(), Some(-1));
    assert_eq!(
        axis.disposition(),
        SpiredBoundedCaseAxisDisposition::FullParentBulk
    );
    assert_eq!(bounds(envelope.bulk().stratum()), vec![(-4, -1)]);
    assert!(envelope.equality_faces().is_empty());
}

#[test]
fn positive_and_negative_target_shifts_move_the_base_coordinate_cutoff() {
    let family = unit_tadpole("bounded-case-target-origin");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent = parent(&sources, &[false], &[(-4, 0)]);

    // q_max=+1. For p=+2 the target-relative envelope is zero, but the base
    // index must still satisfy n+p<=0. Using n<=-M would wrongly retain all of
    // -4..0; the physical-coordinate cutoff is n<=-p-M=-2.
    let positive = try_build_spired_bounded_case_envelope(
        &parent,
        &sources,
        &target([2]),
        0,
        Default::default(),
    )
    .unwrap();
    let axis = positive.shift_envelope().axes()[0];
    assert_eq!(axis.max_positive_relative_shift(), 0);
    assert_eq!(axis.base_safe_upper(), Some(-2));
    assert_eq!(bounds(positive.bulk().stratum()), vec![(-4, -2)]);
    assert_eq!(
        positive
            .equality_faces()
            .iter()
            .map(|face| face.value())
            .collect::<Vec<_>>(),
        vec![-1, 0]
    );

    // For p=-1, M=max(0,+1-(-1))=2, hence n<=-p-M=-1. The
    // target-relative-only cutoff n<=-M=-2 would be needlessly strict.
    let negative = try_build_spired_bounded_case_envelope(
        &parent,
        &sources,
        &target([-1]),
        0,
        Default::default(),
    )
    .unwrap();
    let axis = negative.shift_envelope().axes()[0];
    assert_eq!(axis.max_positive_relative_shift(), 2);
    assert_eq!(axis.base_safe_upper(), Some(-1));
    assert_eq!(bounds(negative.bulk().stratum()), vec![(-4, -1)]);
    assert_eq!(
        negative
            .equality_faces()
            .iter()
            .map(|face| face.value())
            .collect::<Vec<_>>(),
        vec![0]
    );
}

#[test]
fn working_faces_require_explicit_logical_authorization_before_recursion() {
    let family = equal_mass_sunset("bounded-case-multiple-axes");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent = parent(&sources, &[false, false, true], &[(-2, 0), (-2, 0), (1, 1)]);

    let envelope = try_build_spired_bounded_case_envelope(
        &parent,
        &sources,
        &target([0, 0, 0]),
        0,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        bounds(envelope.bulk().stratum()),
        vec![(-2, -1), (-2, -1), (1, 1)]
    );
    assert_eq!(
        envelope
            .equality_faces()
            .iter()
            .map(|face| {
                (
                    face.position(),
                    face.value(),
                    bounds(face.working_case().stratum()),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (0, 0, vec![(0, 0), (-2, 0), (1, 1)]),
            (1, 0, vec![(-2, 0), (0, 0), (1, 1)]),
        ]
    );
    assert_eq!(envelope.census().split_axes(), 2);

    // The envelope does not itself return a logical obligation. Simulate the
    // separate exact authorization boundary explicitly before reprocessing
    // the first face; x=0 then remains fixed while y is partitioned.
    let first_face = SpiredCoordinateCaseObligation::try_new_child(
        &parent,
        envelope.equality_faces()[0]
            .working_case()
            .stratum()
            .clone(),
    )
    .unwrap();
    let child = try_build_spired_bounded_case_envelope(
        &first_face,
        &sources,
        &target([0, 0, 0]),
        0,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        child.shift_envelope().axes()[0].disposition(),
        SpiredBoundedCaseAxisDisposition::FixedInactive
    );
    assert_eq!(
        bounds(child.bulk().stratum()),
        vec![(0, 0), (-2, -1), (1, 1)]
    );
    assert_eq!(
        child
            .equality_faces()
            .iter()
            .map(|face| (face.position(), face.value()))
            .collect::<Vec<_>>(),
        vec![(1, 0)]
    );
}

#[test]
fn fixed_and_no_bulk_inactive_axes_remain_unsplit() {
    let family = unit_tadpole("bounded-case-unsplit");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);

    let fixed_parent = parent(&sources, &[false], &[(0, 0)]);
    let fixed = try_build_spired_bounded_case_envelope(
        &fixed_parent,
        &sources,
        &target([0]),
        0,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        fixed.shift_envelope().axes()[0].disposition(),
        SpiredBoundedCaseAxisDisposition::FixedInactive
    );
    assert_eq!(bounds(fixed.bulk().stratum()), vec![(0, 0)]);
    assert!(fixed.equality_faces().is_empty());
    assert_eq!(fixed.census().fixed_inactive_axes(), 1);

    let shallow_parent = parent(&sources, &[false], &[(-1, 0)]);
    let no_bulk = try_build_spired_bounded_case_envelope(
        &shallow_parent,
        &sources,
        &target([0]),
        1,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        no_bulk.shift_envelope().axes()[0].disposition(),
        SpiredBoundedCaseAxisDisposition::NoNonEmptyBulk
    );
    assert_eq!(bounds(no_bulk.bulk().stratum()), vec![(-1, 0)]);
    assert!(no_bulk.equality_faces().is_empty());
    assert_eq!(no_bulk.census().no_nonempty_bulk_axes(), 1);
}

#[test]
fn scope_layout_arity_and_guard_fail_closed() {
    let family = unit_tadpole("bounded-case-validation");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent = parent(&sources, &[false], &[(-3, 0)]);

    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent,
            &sources,
            &target([0, 0]),
            0,
            Default::default(),
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::WrongTargetArity {
            expected: 1,
            actual: 2,
        }
    );

    let guard = GuardBranchIdentity::try_new(
        "bounded-case-guard",
        GuardBranch::Zero,
        StratumRegistryLimits::default(),
    )
    .unwrap();
    let guarded = parent_stratum_with_guards(&sources, &[false], &[(-3, 0)], [guard]);
    assert!(matches!(
        SpiredCoordinateCaseObligation::try_new_root(guarded),
        Err(
            super::super::SpiredCoordinateCaseWorklistError::GuardedDiscoveryCase {
                guard_branches: 1
            }
        )
    ));

    let external = generator.prepare_external_ibp_sources().unwrap();
    let external = external.complete(Vec::new()).unwrap();
    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent,
            &external,
            &target([0]),
            0,
            Default::default(),
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::WrongSourceLayout {
            actual: "external-contraction IBP source",
        }
    );

    let mut foreign_family = complete_ordinary(&generator);
    foreign_family.replace_family_fingerprint_for_test("foreign-family");
    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent,
            &foreign_family,
            &target([0]),
            0,
            Default::default(),
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::WrongSourceFamily
    );

    let mut foreign_context = complete_ordinary(&generator);
    foreign_context.replace_context_fingerprint_for_test("foreign-context");
    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent,
            &foreign_context,
            &target([0]),
            0,
            Default::default(),
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::WrongSourceContext
    );
}

#[test]
fn resource_limits_and_arithmetic_overflow_are_typed() {
    let family = unit_tadpole("bounded-case-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent_case = parent(&sources, &[false], &[(-4, 0)]);
    let defaults = SpiredBoundedCaseEnvelopeLimits::default();

    for (limits, expected_resource, requested, limit) in [
        (
            SpiredBoundedCaseEnvelopeLimits {
                max_arity: 0,
                ..defaults
            },
            "case-envelope arity",
            1,
            0,
        ),
        (
            SpiredBoundedCaseEnvelopeLimits {
                max_source_rows: 0,
                ..defaults
            },
            "ordinary source rows",
            1,
            0,
        ),
        (
            SpiredBoundedCaseEnvelopeLimits {
                max_equality_faces: 0,
                ..defaults
            },
            "equality faces",
            1,
            0,
        ),
        (
            SpiredBoundedCaseEnvelopeLimits {
                max_retained_domains: 1,
                ..defaults
            },
            "retained domains",
            2,
            1,
        ),
    ] {
        assert_eq!(
            try_build_spired_bounded_case_envelope(
                &parent_case,
                &sources,
                &target([0]),
                0,
                limits,
            )
            .unwrap_err(),
            SpiredBoundedCaseEnvelopeError::ResourceLimit {
                resource: expected_resource,
                requested,
                limit,
            }
        );
    }

    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent_case,
            &sources,
            &target([0]),
            1,
            SpiredBoundedCaseEnvelopeLimits {
                max_source_depth: 0,
                ..defaults
            },
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::ResourceLimit {
            resource: "signed-L1 source depth",
            requested: 1,
            limit: 0,
        }
    );

    assert_eq!(
        try_build_spired_bounded_case_envelope(
            &parent_case,
            &sources,
            &target([0]),
            usize::MAX,
            SpiredBoundedCaseEnvelopeLimits {
                max_source_depth: usize::MAX,
                ..defaults
            },
        )
        .unwrap_err(),
        SpiredBoundedCaseEnvelopeError::DepthNotRepresentable { depth: usize::MAX }
    );

    let relative_overflow_parent = parent(&sources, &[false], &[(-4, 0)]);
    assert!(matches!(
        try_build_spired_bounded_case_envelope(
            &relative_overflow_parent,
            &sources,
            &target([i64::MIN]),
            0,
            Default::default(),
        ),
        Err(
            SpiredBoundedCaseEnvelopeError::RelativeShiftEnvelopeOverflow {
                target_shift: i64::MIN,
                ..
            }
        )
    ));

    assert!(matches!(
        try_build_spired_bounded_case_envelope(
            &parent_case,
            &sources,
            &target([0]),
            i64::MAX as usize,
            SpiredBoundedCaseEnvelopeLimits {
                max_source_depth: usize::MAX,
                ..defaults
            },
        ),
        Err(SpiredBoundedCaseEnvelopeError::TranslatedShiftOverflow {
            source_shift: 1,
            depth,
            ..
        }) if depth == i64::MAX as usize
    ));
}

#[test]
fn bulk_and_overlapping_faces_have_exact_union_on_a_finite_box() {
    let family = equal_mass_sunset("bounded-case-exact-union");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let sources = complete_ordinary(&generator);
    let parent = parent(&sources, &[false, false, true], &[(-3, 0), (-3, 0), (1, 1)]);
    let target = target([0, 0, 0]);
    let envelope =
        try_build_spired_bounded_case_envelope(&parent, &sources, &target, 1, Default::default())
            .unwrap();

    assert_eq!(
        bounds(envelope.bulk().stratum()),
        vec![(-3, -2), (-3, -2), (1, 1)]
    );
    assert_eq!(envelope.equality_faces().len(), 4);
    for first in -3..=0 {
        for second in -3..=0 {
            let point = [first, second, 1];
            let covered = contains(envelope.bulk().stratum(), &point)
                || envelope
                    .equality_faces()
                    .iter()
                    .any(|face| contains(face.working_case().stratum(), &point));
            assert_eq!(covered, parent.stratum().domain().contains(&point).unwrap());
        }
    }
    assert_eq!(
        envelope
            .equality_faces()
            .iter()
            .filter(|face| contains(face.working_case().stratum(), &[0, 0, 1]))
            .count(),
        2,
        "broad coordinate faces are intentionally allowed to overlap"
    );

    // Exhaustively confirm the coordinate envelope against every ordinary
    // source term and every signed-L1 offset through depth one.
    let offsets = [[-1, 0, 0], [0, -1, 0], [0, 0, 0], [0, 1, 0], [1, 0, 0]];
    for first in -3..=-2 {
        for second in -3..=-2 {
            let point = [first, second, 1];
            for relation in sources.relations() {
                for source_shift in relation.terms().keys() {
                    for offset in offsets {
                        for position in 0..2 {
                            let relative = source_shift.values()[position] + offset[position]
                                - target.values()[position];
                            assert!(point[position] + relative <= 0);
                        }
                    }
                }
            }
        }
    }
    assert_eq!(envelope.census().source_rows(), 4);
    assert!(envelope.census().source_terms() >= 2);
    assert_eq!(
        envelope.census().source_coordinate_cells(),
        envelope.census().source_terms() * 3
    );
    assert_eq!(envelope.census().retained_domain_bound_cells(), 15);
}
