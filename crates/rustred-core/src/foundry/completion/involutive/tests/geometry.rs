use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
use crate::identity::{CompletedIbpSourceRows, ParametricIbpGenerator};
use crate::sector::{Mask, OrderingPolicy};

use super::super::super::CompletionGeometryLimits;
use super::super::divisor_index::{JanetDivisorIndex, JanetMonomialView};
use super::super::janet::{
    EpochId, JanetDivisionEpoch, JanetDivisionGeometry, JanetMultiplicativeMask,
    geometry_authority, try_build_completion_geometry,
    try_compute_multiplicative_masks_from_geometry,
};
use super::super::limits::InvolutiveWorkBudget;
use super::super::selection::try_select_janet_reduction;
use super::super::*;
use super::support::*;

#[derive(Clone)]
struct DetachedJanetMonomial {
    ordinal: usize,
    leading_shift: ForwardShift,
    multiplicative: JanetMultiplicativeMask,
}

struct DetachedJanetGeometry {
    epoch: EpochId,
    action: OreActionIdentity,
    arity: usize,
    monomials: Box<[DetachedJanetMonomial]>,
    divisor_index: JanetDivisorIndex,
}

impl DetachedJanetGeometry {
    fn try_from_exact(
        division: &JanetDivisionEpoch,
        limits: InvolutiveLimits,
        work: &mut InvolutiveWorkBudget,
    ) -> Result<Self, InvolutiveError> {
        let epoch = division.geometry_epoch().clone();
        let action = division.geometry_action().clone();
        let arity = division.geometry_arity();
        let mut monomials = Vec::new();
        monomials
            .try_reserve_exact(division.geometry_element_count())
            .map_err(|_| InvolutiveError::AllocationFailure {
                resource: "detached Janet test monomials",
                requested: division.geometry_element_count(),
            })?;
        for ordinal in 0..division.geometry_element_count() {
            let monomial =
                division
                    .geometry_monomial(ordinal)
                    .ok_or(InvolutiveError::Invariant {
                        detail: "exact epoch omitted a detached Janet test monomial",
                    })?;
            monomials.push(DetachedJanetMonomial {
                ordinal: monomial.ordinal(),
                leading_shift: monomial.leading_shift().clone(),
                multiplicative: monomial.multiplicative().clone(),
            });
        }
        let divisor_index = JanetDivisorIndex::try_new_from_geometry(
            &epoch,
            arity,
            monomials.len(),
            monomials.iter().map(|monomial| {
                JanetMonomialView::new(
                    monomial.ordinal,
                    &monomial.leading_shift,
                    &monomial.multiplicative,
                )
            }),
            limits,
            work,
        )?;
        Ok(Self {
            epoch,
            action,
            arity,
            monomials: monomials.into_boxed_slice(),
            divisor_index,
        })
    }
}

impl geometry_authority::Sealed for DetachedJanetGeometry {}

impl JanetDivisionGeometry for DetachedJanetGeometry {
    fn geometry_epoch(&self) -> &EpochId {
        &self.epoch
    }

    fn geometry_action(&self) -> &OreActionIdentity {
        &self.action
    }

    fn geometry_arity(&self) -> usize {
        self.arity
    }

    fn geometry_element_count(&self) -> usize {
        self.monomials.len()
    }

    fn geometry_monomial(&self, ordinal: usize) -> Option<JanetMonomialView<'_>> {
        self.monomials.get(ordinal).map(|monomial| {
            JanetMonomialView::new(
                monomial.ordinal,
                &monomial.leading_shift,
                &monomial.multiplicative,
            )
        })
    }

    fn geometry_divisor_index(&self) -> &JanetDivisorIndex {
        &self.divisor_index
    }
}

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn flat_divisor(
    basis: &JanetBasisEpoch,
    target: &ForwardShift,
    excluded: Option<usize>,
) -> Option<usize> {
    basis.elements().iter().find_map(|element| {
        (excluded != Some(element.ordinal())
            && element
                .multiplicative()
                .janet_divides(element.leading_shift(), target))
        .then_some(element.ordinal())
    })
}

fn build_geometry_index<'a>(
    division: &JanetDivisionEpoch,
    element_count: usize,
    monomials: impl ExactSizeIterator<Item = JanetMonomialView<'a>> + Clone,
    limits: InvolutiveLimits,
) -> Result<JanetDivisorIndex, InvolutiveError> {
    JanetDivisorIndex::try_new_from_geometry(
        division.epoch(),
        division.arity(),
        element_count,
        monomials,
        limits,
        &mut InvolutiveWorkBudget::default(),
    )
}

fn legacy_mask_oracle(leaders: &[&ForwardShift], variable_sequence: &[usize]) -> Vec<Vec<bool>> {
    leaders
        .iter()
        .map(|leader| {
            let mut mask = vec![false; variable_sequence.len()];
            for (sequence_position, &variable) in variable_sequence.iter().enumerate() {
                let prefix = &variable_sequence[..sequence_position];
                let maximum = leaders
                    .iter()
                    .filter(|candidate| {
                        prefix.iter().all(|&prefix_variable| {
                            candidate.values()[prefix_variable] == leader.values()[prefix_variable]
                        })
                    })
                    .map(|candidate| candidate.values()[variable])
                    .max()
                    .expect("the leader itself belongs to its Janet prefix class");
                mask[variable] = leader.values()[variable] == maximum;
            }
            mask
        })
        .collect()
}

fn assert_geometry_path_matches_exact_epoch(
    basis: &JanetBasisEpoch,
    ordering: &OreOrderingAdapter,
    targets: &[ForwardShift],
    limits: InvolutiveLimits,
) {
    let division = basis.division();
    let elements = basis.elements();
    let leaders = elements
        .iter()
        .map(JanetBasisElement::leading_shift)
        .collect::<Vec<_>>();
    let mask_bits = try_compute_multiplicative_masks_from_geometry(
        leaders.len(),
        |ordinal| leaders.get(ordinal).copied(),
        ordering.variable_sequence(),
        limits,
    )
    .unwrap();
    assert_eq!(
        mask_bits,
        legacy_mask_oracle(&leaders, ordering.variable_sequence())
    );
    for (element, bits) in elements.iter().zip(&mask_bits) {
        assert_eq!(element.multiplicative().bits(), bits);
    }
    let masks = mask_bits
        .into_iter()
        .map(JanetMultiplicativeMask::from_sealed_bits)
        .collect::<Vec<_>>();

    let mut exact_geometry_work = InvolutiveWorkBudget::default();
    let exact_geometry_index = JanetDivisorIndex::try_new_from_geometry(
        division.epoch(),
        division.arity(),
        elements.len(),
        elements.iter().map(|element| {
            JanetMonomialView::new(
                element.ordinal(),
                element.leading_shift(),
                element.multiplicative(),
            )
        }),
        limits,
        &mut exact_geometry_work,
    )
    .unwrap();
    let mut independent_geometry_work = InvolutiveWorkBudget::default();
    let independent_geometry_index = JanetDivisorIndex::try_new_from_geometry(
        division.epoch(),
        division.arity(),
        leaders.len(),
        leaders
            .iter()
            .zip(&masks)
            .enumerate()
            .map(|(ordinal, (&leader, mask))| JanetMonomialView::new(ordinal, leader, mask)),
        limits,
        &mut independent_geometry_work,
    )
    .unwrap();

    assert_eq!(exact_geometry_index, independent_geometry_index);
    assert_eq!(division.divisor_index(), &exact_geometry_index);
    assert_eq!(
        exact_geometry_work.census(),
        independent_geometry_work.census()
    );
    assert_eq!(
        exact_geometry_index.retained_bytes(),
        basis.divisor_index_retained_bytes()
    );

    let mut detached_build_work = InvolutiveWorkBudget::default();
    let detached =
        DetachedJanetGeometry::try_from_exact(division, limits, &mut detached_build_work).unwrap();
    assert_eq!(detached.divisor_index, exact_geometry_index);
    assert_eq!(detached_build_work.census(), exact_geometry_work.census());

    let detached_completion = try_build_completion_geometry(
        &detached,
        ordering,
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();
    assert_eq!(detached_completion.epoch(), basis.epoch());
    assert!(detached_completion.action().belongs_to(ordering.identity()));
    assert_eq!(detached_completion.arity(), basis.arity());
    assert_eq!(detached_completion.prolongations(), basis.prolongations());
    assert_eq!(detached_completion.leading_ideal(), basis.leading_ideal());
    assert_eq!(
        detached_completion.uncovered_partition(),
        basis.uncovered_partition()
    );
    assert_eq!(
        detached_completion.pure_power_coverage(),
        basis.pure_power_coverage()
    );
    let blind =
        BlindDomainSchedule::try_from_partition(basis.uncovered_partition(), ordering, limits)
            .unwrap();
    let exact_priority = blind
        .try_rank_prolongation_ordinals(division, basis.prolongations(), ordering, limits)
        .unwrap();
    let detached_priority = blind
        .try_rank_prolongation_ordinals(
            &detached,
            detached_completion.prolongations(),
            ordering,
            limits,
        )
        .unwrap();
    assert_eq!(detached_priority, exact_priority);

    let mut epoch_scratch = basis.try_divisor_scratch(limits).unwrap();
    let mut exact_geometry_scratch = exact_geometry_index.try_scratch(limits).unwrap();
    let mut independent_geometry_scratch = independent_geometry_index.try_scratch(limits).unwrap();
    assert_eq!(
        epoch_scratch.retained_bytes(),
        exact_geometry_scratch.retained_bytes()
    );
    assert_eq!(
        exact_geometry_scratch.retained_bytes(),
        independent_geometry_scratch.retained_bytes()
    );
    let mut epoch_work = InvolutiveWorkBudget::default();
    let mut exact_geometry_query_work = InvolutiveWorkBudget::default();
    let mut independent_geometry_query_work = InvolutiveWorkBudget::default();
    for target in targets {
        for excluded in std::iter::once(None).chain((0..elements.len()).map(Some)) {
            let expected = flat_divisor(basis, target, excluded);
            let epoch_result = basis
                .try_janet_divisor_with_scratch(
                    target,
                    excluded,
                    &mut epoch_scratch,
                    limits,
                    &mut epoch_work,
                )
                .unwrap();
            let exact_geometry_result = exact_geometry_index
                .try_first_divisor(
                    division.epoch(),
                    target,
                    excluded,
                    &mut exact_geometry_scratch,
                    limits,
                    &mut exact_geometry_query_work,
                )
                .unwrap();
            let independent_geometry_result = independent_geometry_index
                .try_first_divisor(
                    division.epoch(),
                    target,
                    excluded,
                    &mut independent_geometry_scratch,
                    limits,
                    &mut independent_geometry_query_work,
                )
                .unwrap();
            assert_eq!(epoch_result, expected);
            assert_eq!(exact_geometry_result, expected);
            assert_eq!(independent_geometry_result, expected);
        }
    }
    assert_eq!(epoch_work.census(), exact_geometry_query_work.census());
    assert_eq!(
        exact_geometry_query_work.census(),
        independent_geometry_query_work.census()
    );

    for excluded in std::iter::once(None).chain((0..elements.len()).map(Some)) {
        let mut epoch_scratch = basis.try_divisor_scratch(limits).unwrap();
        let mut detached_scratch = detached.try_geometry_divisor_scratch(limits).unwrap();
        let mut epoch_visits = 0;
        let mut detached_visits = 0;
        let mut epoch_work = InvolutiveWorkBudget::default();
        let mut detached_work = InvolutiveWorkBudget::default();
        let exact = try_select_janet_reduction(
            division,
            targets.iter(),
            excluded,
            ordering,
            limits,
            &mut epoch_visits,
            &mut epoch_scratch,
            &mut epoch_work,
        )
        .unwrap();
        let coefficient_free = try_select_janet_reduction(
            &detached,
            targets.iter(),
            excluded,
            ordering,
            limits,
            &mut detached_visits,
            &mut detached_scratch,
            &mut detached_work,
        )
        .unwrap();
        assert_eq!(coefficient_free, exact);
        assert_eq!(detached_visits, epoch_visits);
        assert_eq!(detached_work.census(), epoch_work.census());
    }
}

#[test]
fn coefficient_free_geometry_matches_synthetic_exact_epoch_byte_for_byte() {
    let limits = InvolutiveLimits::default();
    let context = context(3);
    let ordering = active_ordering(3, limits);
    let basis = epoch(
        &[&[0, 0, 3], &[0, 2, 1], &[1, 0, 2], &[1, 1, 0], &[2, 0, 0]],
        &context,
        &ordering,
        limits,
    );
    let targets = (0..=3)
        .flat_map(|x| (0..=3).flat_map(move |y| (0..=3).map(move |z| shift(&[x, y, z], limits))))
        .collect::<Vec<_>>();
    assert_geometry_path_matches_exact_epoch(&basis, &ordering, &targets, limits);
}

#[test]
fn coefficient_free_geometry_matches_generated_k3_sunset_epoch() {
    let artifact = derive_two_loop_unit_mass_sunset().unwrap();
    let generator = ParametricIbpGenerator::try_new(artifact.family()).unwrap();
    let completed = complete_ordinary(&generator);
    let limits = InvolutiveLimits::default();
    let ordering = OreOrderingAdapter::try_new_for_completed(
        OrderingPolicy::default(),
        Mask::try_new([true, false, true]).unwrap(),
        &completed,
        limits,
    )
    .unwrap();
    let lifted = try_lift_completed_ordinary_sources(
        &completed,
        &ordering,
        generator.context(),
        OrdinaryChartLiftLimits {
            involutive: limits,
            ..OrdinaryChartLiftLimits::default()
        },
    )
    .unwrap();
    let consequences = lifted
        .try_into_consequences(&completed, &ordering, generator.context(), limits)
        .unwrap();
    let basis = JanetBasisEpoch::try_initial(
        consequences.into_vec(),
        &ordering,
        generator.context(),
        limits,
        CompletionGeometryLimits::default(),
    )
    .unwrap();

    // Stable generated K=3 completion-geometry census. This pins the exact
    // ELC0 queue/complement trajectory while the coefficient owner changes.
    assert_eq!(
        (
            basis.elements().len(),
            basis.epoch().revision(),
            basis.prolongations().len(),
            basis.uncovered_partition().boxes().len(),
            basis.uncovered_partition().is_finite(),
            basis.pure_power_coverage().is_complete(),
        ),
        (4, 0, 5, 3, false, false),
    );

    let mut targets = basis
        .elements()
        .iter()
        .map(|element| element.leading_shift().clone())
        .collect::<Vec<_>>();
    for element in basis.elements() {
        for coordinate in 0..basis.arity() {
            targets.push(
                element
                    .leading_shift()
                    .try_checked_add(
                        &ForwardShift::try_unit(basis.arity(), coordinate, limits).unwrap(),
                        limits,
                    )
                    .unwrap(),
            );
        }
    }
    assert_geometry_path_matches_exact_epoch(&basis, &ordering, &targets, limits);
}

#[test]
fn geometry_boundaries_reject_missing_noncanonical_and_wrong_arity_descriptors() {
    let limits = InvolutiveLimits::default();
    let context = context(2);
    let ordering = active_ordering(2, limits);
    let basis = epoch(&[&[2, 0], &[0, 3]], &context, &ordering, limits);
    let division = basis.division();
    let elements = basis.elements();
    assert_eq!(
        build_geometry_index(
            division,
            elements.len(),
            elements.iter().enumerate().map(|(ordinal, element)| {
                JanetMonomialView::new(
                    if ordinal == 0 { 1 } else { ordinal },
                    element.leading_shift(),
                    element.multiplicative(),
                )
            }),
            limits
        ),
        Err(InvolutiveError::Invariant {
            detail: "Janet divisor index saw a noncanonical basis ordinal",
        })
    );
    assert_eq!(
        build_geometry_index(
            division,
            elements.len(),
            elements.iter().take(1).map(|element| {
                JanetMonomialView::new(
                    element.ordinal(),
                    element.leading_shift(),
                    element.multiplicative(),
                )
            }),
            limits
        ),
        Err(InvolutiveError::Invariant {
            detail: "Janet divisor index geometry omitted a basis ordinal",
        })
    );

    let wrong_leader = shift(&[1], limits);
    assert_eq!(
        build_geometry_index(
            division,
            elements.len(),
            elements.iter().enumerate().map(|(ordinal, element)| {
                JanetMonomialView::new(
                    ordinal,
                    if ordinal == 0 {
                        &wrong_leader
                    } else {
                        element.leading_shift()
                    },
                    element.multiplicative(),
                )
            }),
            limits
        ),
        Err(InvolutiveError::WrongArity {
            object: "Janet divisor index element",
            expected: 2,
            actual: 1,
        })
    );

    let wrong_mask = JanetMultiplicativeMask::from_sealed_bits(vec![true]);
    assert_eq!(
        build_geometry_index(
            division,
            elements.len(),
            elements.iter().enumerate().map(|(ordinal, element)| {
                JanetMonomialView::new(
                    ordinal,
                    element.leading_shift(),
                    if ordinal == 0 {
                        &wrong_mask
                    } else {
                        element.multiplicative()
                    },
                )
            }),
            limits
        ),
        Err(InvolutiveError::WrongArity {
            object: "Janet divisor index mask",
            expected: 2,
            actual: 1,
        })
    );

    assert_eq!(
        try_compute_multiplicative_masks_from_geometry(
            2,
            |ordinal| (ordinal == 0).then_some(elements[0].leading_shift()),
            ordering.variable_sequence(),
            limits,
        ),
        Err(InvolutiveError::Invariant {
            detail: "Janet mask geometry omitted a leader ordinal",
        })
    );
    assert_eq!(
        try_compute_multiplicative_masks_from_geometry(
            1,
            |_| Some(&wrong_leader),
            ordering.variable_sequence(),
            limits,
        ),
        Err(InvolutiveError::WrongArity {
            object: "Janet mask geometry leader",
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
fn descriptor_backed_index_preserves_exact_build_caps_and_admission_order() {
    let defaults = InvolutiveLimits::default();
    let context = context(3);
    let ordering = active_ordering(3, defaults);
    let basis = epoch(
        &[&[0, 0, 3], &[0, 2, 1], &[1, 0, 2], &[1, 1, 0], &[2, 0, 0]],
        &context,
        &ordering,
        defaults,
    );
    let division = basis.division();
    let elements = basis.elements();
    let build = |limits, work: &mut InvolutiveWorkBudget| {
        JanetDivisorIndex::try_new_from_geometry(
            division.epoch(),
            division.arity(),
            elements.len(),
            elements.iter().map(|element| {
                JanetMonomialView::new(
                    element.ordinal(),
                    element.leading_shift(),
                    element.multiplicative(),
                )
            }),
            limits,
            work,
        )
    };
    let mut work = InvolutiveWorkBudget::default();
    let index = build(defaults, &mut work).unwrap();
    let operations = work.census().divisor_index_build_operations();
    assert!(operations > 0);
    let operation_cap = InvolutiveLimits {
        max_divisor_index_build_operations: operations - 1,
        ..defaults
    };
    assert_eq!(
        build(operation_cap, &mut InvolutiveWorkBudget::default()),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index build operations",
            requested: operations,
            limit: operations - 1,
        })
    );

    let retained_bytes = index.retained_bytes();
    let retained_cap = InvolutiveLimits {
        max_divisor_index_retained_bytes: retained_bytes - 1,
        ..defaults
    };
    assert_eq!(
        build(retained_cap, &mut InvolutiveWorkBudget::default()),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index retained bytes",
            requested: retained_bytes,
            limit: retained_bytes - 1,
        })
    );

    let scratch_bytes = 2 * elements.len() * std::mem::size_of::<(u64, usize)>();
    let scratch_cap = InvolutiveLimits {
        max_divisor_index_build_scratch_bytes: scratch_bytes - 1,
        ..defaults
    };
    assert_eq!(
        build(scratch_cap, &mut InvolutiveWorkBudget::default()),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet divisor index build scratch bytes",
            requested: scratch_bytes,
            limit: scratch_bytes - 1,
        })
    );

    let mask_cells = elements.len() * division.arity();
    let mask_cap = InvolutiveLimits {
        max_basis_coordinate_cells: mask_cells - 1,
        ..defaults
    };
    let called = std::cell::Cell::new(false);
    assert_eq!(
        try_compute_multiplicative_masks_from_geometry(
            elements.len(),
            |_| {
                called.set(true);
                None
            },
            ordering.variable_sequence(),
            mask_cap,
        ),
        Err(InvolutiveError::ResourceLimit {
            resource: "Janet multiplicative-mask cells",
            requested: mask_cells,
            limit: mask_cells - 1,
        })
    );
    assert!(
        !called.get(),
        "mask allocation must be preflighted before geometry access"
    );
}
