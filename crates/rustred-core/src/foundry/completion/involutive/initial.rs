use std::cmp::Ordering;

use symbolica::domains::InternalOrdering;

use crate::algebra::IndexedCoefficientContext;
use crate::sector::ShiftComplexityKey;

use super::error::{check_limit, checked_add, checked_mul, try_vec};
use super::janet::{
    preflight_basis_coefficient_census, preflight_basis_coefficient_payload, preflight_basis_shape,
};
use super::limits::{InvolutiveWorkBudget, InvolutiveWorkCensus};
use super::{
    CoefficientPayloadCensus, ForwardShift, InvolutiveError, InvolutiveLimits, JanetBasisEpoch,
    LocalizationWitness, OreConsequence, OreOrderingAdapter,
};
use crate::foundry::completion::CompletionGeometryLimits;

/// Bounded accounting for deterministic equal-head elimination before Janet
/// masks or obligations are constructed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JanetInitialReductionCensus {
    input_rows: usize,
    retained_rows: usize,
    equal_head_eliminations: usize,
    zero_remainders: usize,
    nonzero_remainders: usize,
    cascading_collisions: usize,
    max_collision_chain: usize,
    max_head_class: usize,
    sort_comparisons: usize,
    sort_payload_visits: usize,
    pivot_head_comparisons: usize,
    pivot_head_coordinate_visits: usize,
    pivot_insertion_moves: usize,
}

impl JanetInitialReductionCensus {
    pub(crate) const fn input_rows(self) -> usize {
        self.input_rows
    }

    pub(crate) const fn retained_rows(self) -> usize {
        self.retained_rows
    }

    pub(crate) const fn equal_head_eliminations(self) -> usize {
        self.equal_head_eliminations
    }

    pub(crate) const fn zero_remainders(self) -> usize {
        self.zero_remainders
    }

    pub(crate) const fn nonzero_remainders(self) -> usize {
        self.nonzero_remainders
    }

    pub(crate) const fn cascading_collisions(self) -> usize {
        self.cascading_collisions
    }

    pub(crate) const fn max_collision_chain(self) -> usize {
        self.max_collision_chain
    }

    pub(crate) const fn max_head_class(self) -> usize {
        self.max_head_class
    }

    pub(crate) const fn sort_comparisons(self) -> usize {
        self.sort_comparisons
    }

    pub(crate) const fn sort_payload_visits(self) -> usize {
        self.sort_payload_visits
    }

    pub(crate) const fn pivot_head_comparisons(self) -> usize {
        self.pivot_head_comparisons
    }

    pub(crate) const fn pivot_head_coordinate_visits(self) -> usize {
        self.pivot_head_coordinate_visits
    }

    pub(crate) const fn pivot_insertion_moves(self) -> usize {
        self.pivot_insertion_moves
    }
}

/// One proposal-only, row-echelon initial module with distinct Janet heads.
///
/// Zero consequences do not enter the basis and are not retained in
/// production: their exact sparse provenance is maintained through the
/// cancellation, then their canonical localization is unioned before the
/// payload is released. Tests retain a bounded trace to verify that algebraic
/// provenance invariant directly.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct JanetInitialReduction {
    epoch: JanetBasisEpoch,
    #[cfg(test)]
    zero_remainders: Box<[OreConsequence]>,
    localization: LocalizationWitness,
    census: JanetInitialReductionCensus,
    work: InvolutiveWorkCensus,
}

impl JanetInitialReduction {
    pub(crate) fn epoch(&self) -> &JanetBasisEpoch {
        &self.epoch
    }

    #[cfg(test)]
    pub(crate) fn zero_remainders(&self) -> &[OreConsequence] {
        &self.zero_remainders
    }

    pub(crate) fn localization_witness(&self) -> &LocalizationWitness {
        &self.localization
    }

    pub(crate) const fn census(&self) -> JanetInitialReductionCensus {
        self.census
    }

    pub(crate) const fn work_census(&self) -> InvolutiveWorkCensus {
        self.work
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        JanetBasisEpoch,
        LocalizationWitness,
        JanetInitialReductionCensus,
    ) {
        (self.epoch, self.localization, self.census)
    }
}

struct RankedInput {
    leading_shift: ForwardShift,
    leading_key: ShiftComplexityKey,
    sort_payload_weight: usize,
    consequence: OreConsequence,
}

struct InitialPivot {
    leading_shift: ForwardShift,
    consequence: OreConsequence,
    derived_by_elimination: bool,
    collision_multiplicity: usize,
}

/// Deterministically eliminate coincident leading monomials over the exact
/// Symbolica-backed rational-function field before constructing Janet masks.
///
/// This standalone entry point owns a fresh work ledger. Production completion
/// uses the sibling `*_with_budget` path so initialization, autoreduction, and
/// prolongation consume one cumulative budget.
pub(crate) fn try_preprocess_initial_basis(
    consequences: Vec<OreConsequence>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
) -> Result<JanetInitialReduction, InvolutiveError> {
    let mut work = InvolutiveWorkBudget::default();
    let mut result = try_preprocess_initial_basis_with_budget(
        consequences,
        ordering,
        context,
        limits,
        geometry_limits,
        &mut work,
    )?;
    result.work = work.census();
    Ok(result)
}

pub(super) fn try_preprocess_initial_basis_with_budget(
    consequences: Vec<OreConsequence>,
    ordering: &OreOrderingAdapter,
    context: &IndexedCoefficientContext,
    limits: InvolutiveLimits,
    geometry_limits: CompletionGeometryLimits,
    work: &mut InvolutiveWorkBudget,
) -> Result<JanetInitialReduction, InvolutiveError> {
    if consequences.is_empty() {
        return Err(InvolutiveError::EmptyInitialBasis);
    }
    preflight_basis_shape(consequences.len(), ordering.arity(), limits)?;
    for consequence in &consequences {
        consequence.try_validate(ordering, context, limits)?;
        if consequence.is_zero() {
            return Err(InvolutiveError::ZeroBasisRow);
        }
    }
    preflight_basis_coefficient_payload(consequences.iter(), limits)?;

    // Every retained buffer is allocated only after the complete untrusted
    // input shape, action, coefficient payload, and aggregate basis census
    // have passed their exact bounds.
    let input_rows = consequences.len();
    let mut ranked = try_vec("ranked initial Janet rows", input_rows)?;
    for consequence in consequences {
        let (leading, leading_key) = consequence
            .row()
            .try_leading_term(ordering)?
            .ok_or(InvolutiveError::ZeroBasisRow)?;
        ranked.push(RankedInput {
            leading_shift: leading.shift().clone(),
            leading_key,
            sort_payload_weight: consequence_sort_payload_weight(&consequence, ordering.arity())?,
            consequence,
        });
    }
    let (ranked, sort_comparisons, sort_payload_visits) = try_sort_ranked_inputs(ranked, limits)?;

    let pivot_index_bound = try_preflight_pivot_index_work(input_rows, ordering.arity(), limits)?;

    let mut pivots = try_vec("initial Janet head pivots", input_rows)?;
    #[cfg(test)]
    let mut zero_remainders = try_vec("initial Janet zero remainders", input_rows)?;
    let operator_zero = ForwardShift::try_zero(ordering.arity(), limits)?;
    let mut equal_head_eliminations = 0usize;
    let mut zero_count = 0usize;
    let mut nonzero_remainders = 0usize;
    let mut cascading_collisions = 0usize;
    let mut max_collision_chain = 0usize;
    let mut max_head_class = 1usize;
    let mut zero_localization = LocalizationWitness::default();
    let mut retained_coefficient_census = CoefficientPayloadCensus::default();
    let mut pivot_head_comparisons = 0usize;
    let mut pivot_head_coordinate_visits = 0usize;
    let mut pivot_insertion_moves = 0usize;

    for pending in ranked {
        let mut subject = pending.consequence;
        let mut previous_key = pending.leading_key;
        let mut collision_chain = 0usize;
        debug_assert_eq!(
            subject
                .row()
                .try_leading_term(ordering)?
                .map(|(term, _)| term.shift()),
            Some(&pending.leading_shift),
        );

        loop {
            let (leading_shift, leading_key) = {
                let (leading, key) = subject.row().try_leading_term(ordering)?.ok_or(
                    InvolutiveError::Invariant {
                        detail: "a nonzero initial subject lost its leading term",
                    },
                )?;
                (leading.shift().clone(), key)
            };
            if leading_key > previous_key {
                return Err(InvolutiveError::Invariant {
                    detail: "equal-head preprocessing increased an Ore leader",
                });
            }

            let pivot_position = try_find_pivot(
                &pivots,
                &leading_shift,
                ordering.arity(),
                limits,
                &mut pivot_head_comparisons,
                &mut pivot_head_coordinate_visits,
            )?;
            let Ok(pivot_position) = pivot_position else {
                let insertion = pivot_position.unwrap_err();
                let next_coefficient_census =
                    retained_coefficient_census.try_add(subject.coefficient_census())?;
                preflight_basis_coefficient_census(next_coefficient_census, limits)?;
                let moved = pivots.len() - insertion;
                let next_moves = checked_add(
                    "initial Janet pivot insertion moves",
                    pivot_insertion_moves,
                    moved,
                )?;
                check_limit(
                    "initial Janet pivot insertion moves",
                    next_moves,
                    limits.max_initial_pivot_insertion_moves,
                )?;
                pivots.insert(
                    insertion,
                    InitialPivot {
                        leading_shift,
                        consequence: subject,
                        derived_by_elimination: collision_chain != 0,
                        collision_multiplicity: 1,
                    },
                );
                pivot_insertion_moves = next_moves;
                retained_coefficient_census = next_coefficient_census;
                break;
            };

            work.charge_divisor_visit(limits)?;
            work.charge_normal_form_step(limits)?;
            work.charge_exact_coefficient_operations(2, limits)?;
            equal_head_eliminations = checked_add(
                "initial Janet equal-head eliminations",
                equal_head_eliminations,
                1,
            )?;
            collision_chain = checked_add("initial Janet collision chain", collision_chain, 1)?;
            let pivot_was_derived = pivots[pivot_position].derived_by_elimination;
            pivots[pivot_position].collision_multiplicity = checked_add(
                "initial Janet head-class multiplicity",
                pivots[pivot_position].collision_multiplicity,
                1,
            )?;
            max_head_class = max_head_class.max(pivots[pivot_position].collision_multiplicity);
            if collision_chain > 1 || pivot_was_derived {
                cascading_collisions = checked_add(
                    "initial Janet cascading collisions",
                    cascading_collisions,
                    1,
                )?;
            }
            max_collision_chain = max_collision_chain.max(collision_chain);

            let pivot = &pivots[pivot_position].consequence;
            let subject_leading =
                subject
                    .row()
                    .coefficient(&leading_shift)
                    .ok_or(InvolutiveError::Invariant {
                        detail: "an initial subject leader disappeared before cancellation",
                    })?;
            let pivot_leading =
                pivot
                    .row()
                    .coefficient(&leading_shift)
                    .ok_or(InvolutiveError::Invariant {
                        detail: "an initial pivot leader disappeared before cancellation",
                    })?;
            let required_nonzero =
                context.numerator_condition_from_bound(context.bind_sealed(pivot_leading)?)?;
            let quotient = context.div_bound_with_limits(
                context.bind_sealed(subject_leading)?,
                context.bind_sealed(pivot_leading)?,
                limits.indexed_algebra.exact_algebra,
            )?;
            let multiplier = context.neg_bound_with_limits(
                context.bind_sealed(&quotient)?,
                limits.indexed_algebra.exact_algebra,
            )?;
            subject = subject.try_left_axpy_sealed(
                &multiplier,
                &operator_zero,
                pivot,
                ordering,
                context,
                limits,
                work,
            )?;
            subject = subject
                .try_require_nonzero_guard(required_nonzero, context, limits)?
                .0;

            if subject.is_zero() {
                zero_count = checked_add("initial Janet zero remainders", zero_count, 1)?;
                check_limit("initial Janet zero remainders", zero_count, input_rows)?;
                zero_localization =
                    zero_localization.try_union(subject.localization_witness(), limits)?;
                #[cfg(test)]
                zero_remainders.push(subject);
                break;
            }
            nonzero_remainders =
                checked_add("initial Janet nonzero remainders", nonzero_remainders, 1)?;
            let next_key = subject
                .row()
                .try_leading_term(ordering)?
                .ok_or(InvolutiveError::Invariant {
                    detail: "a nonzero equal-head remainder has no leader",
                })?
                .1;
            if next_key >= leading_key {
                return Err(InvolutiveError::Invariant {
                    detail: "equal-head cancellation did not strictly lower the Ore leader",
                });
            }
            previous_key = next_key;
        }
    }

    let retained_rows = pivots.len();
    debug_assert!(pivot_head_comparisons <= pivot_index_bound.comparisons);
    debug_assert!(pivot_head_coordinate_visits <= pivot_index_bound.coordinate_visits);
    debug_assert!(pivot_insertion_moves <= pivot_index_bound.insertion_moves);
    let mut localization = zero_localization;
    for consequence in pivots.iter().map(|pivot| &pivot.consequence) {
        localization = localization.try_union(consequence.localization_witness(), limits)?;
    }
    let mut retained = try_vec("distinct initial Janet rows", retained_rows)?;
    for pivot in pivots {
        retained.push(pivot.consequence);
    }
    let epoch = JanetBasisEpoch::try_initial_sealed_with_budget(
        retained,
        ordering,
        context,
        limits,
        geometry_limits,
        work,
    )?;
    for element in epoch.elements() {
        localization =
            localization.try_union(element.consequence().localization_witness(), limits)?;
    }
    Ok(JanetInitialReduction {
        epoch,
        #[cfg(test)]
        zero_remainders: zero_remainders.into_boxed_slice(),
        localization,
        census: JanetInitialReductionCensus {
            input_rows,
            retained_rows,
            equal_head_eliminations,
            zero_remainders: zero_count,
            nonzero_remainders,
            cascading_collisions,
            max_collision_chain,
            max_head_class,
            sort_comparisons,
            sort_payload_visits,
            pivot_head_comparisons,
            pivot_head_coordinate_visits,
            pivot_insertion_moves,
        },
        work: work.census(),
    })
}

struct PivotIndexWorkBound {
    comparisons: usize,
    coordinate_visits: usize,
    insertion_moves: usize,
}

/// Admit the complete worst-case indexing work before allocating the pivot
/// vector. Each input performs one terminal lookup and every charged
/// equal-head cancellation can perform one further lookup. A binary lookup in
/// at most `input_rows` pivots performs at most `floor(log2(n)) + 1`
/// comparisons. Sorted-vector insertion moves at most the triangular number of
/// retained pivot slots.
fn try_preflight_pivot_index_work(
    input_rows: usize,
    arity: usize,
    limits: InvolutiveLimits,
) -> Result<PivotIndexWorkBound, InvolutiveError> {
    let lookups = checked_add(
        "initial Janet pivot lookups",
        input_rows,
        limits.max_normal_form_steps,
    )?;
    let comparisons_per_lookup = if input_rows == 0 {
        0
    } else {
        usize::BITS as usize - input_rows.leading_zeros() as usize
    };
    let comparisons = checked_mul(
        "initial Janet pivot head comparisons",
        lookups,
        comparisons_per_lookup,
    )?;
    check_limit(
        "initial Janet pivot head comparisons",
        comparisons,
        limits.max_initial_pivot_head_comparisons,
    )?;
    let coordinate_visits = checked_mul(
        "initial Janet pivot head coordinate visits",
        comparisons,
        arity,
    )?;
    check_limit(
        "initial Janet pivot head coordinate visits",
        coordinate_visits,
        limits.max_initial_pivot_head_coordinate_visits,
    )?;
    let insertion_moves = if input_rows <= 1 {
        0
    } else {
        checked_mul(
            "initial Janet pivot insertion moves",
            input_rows,
            input_rows - 1,
        )? / 2
    };
    check_limit(
        "initial Janet pivot insertion moves",
        insertion_moves,
        limits.max_initial_pivot_insertion_moves,
    )?;
    Ok(PivotIndexWorkBound {
        comparisons,
        coordinate_visits,
        insertion_moves,
    })
}

fn try_find_pivot(
    pivots: &[InitialPivot],
    leading_shift: &ForwardShift,
    arity: usize,
    limits: InvolutiveLimits,
    comparisons: &mut usize,
    coordinate_visits: &mut usize,
) -> Result<Result<usize, usize>, InvolutiveError> {
    let mut left = 0usize;
    let mut right = pivots.len();
    while left < right {
        let middle = left + (right - left) / 2;
        let next_comparisons =
            checked_add("initial Janet pivot head comparisons", *comparisons, 1)?;
        check_limit(
            "initial Janet pivot head comparisons",
            next_comparisons,
            limits.max_initial_pivot_head_comparisons,
        )?;
        let next_coordinate_visits = checked_add(
            "initial Janet pivot head coordinate visits",
            *coordinate_visits,
            arity,
        )?;
        check_limit(
            "initial Janet pivot head coordinate visits",
            next_coordinate_visits,
            limits.max_initial_pivot_head_coordinate_visits,
        )?;
        *comparisons = next_comparisons;
        *coordinate_visits = next_coordinate_visits;
        match pivots[middle].leading_shift.cmp(leading_shift) {
            Ordering::Less => left = middle + 1,
            Ordering::Greater => right = middle,
            Ordering::Equal => return Ok(Ok(middle)),
        }
    }
    Ok(Err(left))
}

fn compare_ranked_inputs(left: &RankedInput, right: &RankedInput) -> Ordering {
    right
        .leading_key
        .cmp(&left.leading_key)
        .then_with(|| right.leading_shift.cmp(&left.leading_shift))
        .then_with(|| compare_consequences(&left.consequence, &right.consequence))
}

fn consequence_sort_payload_weight(
    consequence: &OreConsequence,
    arity: usize,
) -> Result<usize, InvolutiveError> {
    let coefficient = consequence.coefficient_census();
    let localization = consequence.localization_witness().census();
    let sparse_terms = checked_add(
        "initial Janet sort payload visits",
        consequence.row().terms().len(),
        consequence.provenance().terms().len(),
    )?;
    let coordinate_cells = checked_mul("initial Janet sort payload visits", sparse_terms, arity)?;
    let mut weight = checked_add(
        "initial Janet sort payload visits",
        coordinate_cells,
        coefficient.retained_bytes(),
    )?;
    weight = checked_add(
        "initial Janet sort payload visits",
        weight,
        localization.retained_bytes(),
    )?;
    checked_add(
        "initial Janet sort payload visits",
        weight,
        localization.count(),
    )
}

/// Fallible deterministic merge sort whose exact comparator count and a
/// conservative full-payload scan bound are admitted before scratch storage is
/// allocated. This avoids relying on an implementation-specific standard-sort
/// comparison bound for untrusted symbolic input.
fn try_sort_ranked_inputs(
    ranked: Vec<RankedInput>,
    limits: InvolutiveLimits,
) -> Result<(Vec<RankedInput>, usize, usize), InvolutiveError> {
    let rows = ranked.len();
    let rounds = if rows <= 1 {
        0
    } else {
        usize::BITS as usize - (rows - 1).leading_zeros() as usize
    };
    let comparison_bound = checked_mul("initial Janet sort comparisons", rows, rounds)?;
    check_limit(
        "initial Janet sort comparisons",
        comparison_bound,
        limits.max_initial_sort_comparisons,
    )?;
    let maximum_weight = ranked
        .iter()
        .map(|entry| entry.sort_payload_weight)
        .max()
        .unwrap_or(0);
    let payload_bound = checked_mul(
        "initial Janet sort payload visits",
        checked_mul("initial Janet sort payload visits", comparison_bound, 2)?,
        maximum_weight,
    )?;
    check_limit(
        "initial Janet sort payload visits",
        payload_bound,
        limits.max_initial_sort_payload_visits,
    )?;

    let mut slots = try_vec("initial Janet sort slots", rows)?;
    for entry in ranked {
        slots.push(Some(entry));
    }
    let mut order = try_vec("initial Janet sort order", rows)?;
    order.extend(0..rows);
    let mut scratch = try_vec("initial Janet sort scratch", rows)?;
    scratch.resize(rows, 0);
    let mut comparisons = 0usize;
    let mut payload_visits = 0usize;
    let mut width = 1usize;
    while width < rows {
        let mut left = 0usize;
        while left < rows {
            let middle = left.saturating_add(width).min(rows);
            let right = middle.saturating_add(width).min(rows);
            let mut first = left;
            let mut second = middle;
            for output in left..right {
                let choose_first = if second == right {
                    true
                } else if first == middle {
                    false
                } else {
                    comparisons = checked_add("initial Janet sort comparisons", comparisons, 1)?;
                    let left_entry =
                        slots[order[first]]
                            .as_ref()
                            .ok_or(InvolutiveError::Invariant {
                                detail: "initial Janet sort lost a left input row",
                            })?;
                    let right_entry =
                        slots[order[second]]
                            .as_ref()
                            .ok_or(InvolutiveError::Invariant {
                                detail: "initial Janet sort lost a right input row",
                            })?;
                    payload_visits = checked_add(
                        "initial Janet sort payload visits",
                        payload_visits,
                        checked_add(
                            "initial Janet sort payload visits",
                            left_entry.sort_payload_weight,
                            right_entry.sort_payload_weight,
                        )?,
                    )?;
                    compare_ranked_inputs(left_entry, right_entry) != Ordering::Greater
                };
                scratch[output] = if choose_first {
                    let selected = order[first];
                    first += 1;
                    selected
                } else {
                    let selected = order[second];
                    second += 1;
                    selected
                };
            }
            left = right;
        }
        std::mem::swap(&mut order, &mut scratch);
        width = width.checked_mul(2).unwrap_or(rows);
    }
    debug_assert!(comparisons <= comparison_bound);
    debug_assert!(payload_visits <= payload_bound);

    let mut sorted = try_vec("sorted initial Janet rows", rows)?;
    for position in order {
        sorted.push(slots[position].take().ok_or(InvolutiveError::Invariant {
            detail: "initial Janet sort selected one input row twice",
        })?);
    }
    Ok((sorted, comparisons, payload_visits))
}

/// Canonical total tie-break over already authenticated sparse Symbolica
/// values. Provenance comes first so exact source chronology remains stable;
/// row and localization payloads make derived/repeated-source consequences a
/// true total order without trusting a hash collision assumption.
fn compare_consequences(left: &OreConsequence, right: &OreConsequence) -> Ordering {
    compare_provenance(left, right)
        .then_with(|| compare_rows(left, right))
        .then_with(|| compare_localization(left, right))
}

fn compare_provenance(left: &OreConsequence, right: &OreConsequence) -> Ordering {
    for (left, right) in left
        .provenance()
        .terms()
        .iter()
        .zip(right.provenance().terms())
    {
        let ordering = left
            .source_ordinal()
            .cmp(&right.source_ordinal())
            .then_with(|| left.left_shift().cmp(right.left_shift()))
            .then_with(|| {
                left.left_coefficient()
                    .raw()
                    .internal_cmp(right.left_coefficient().raw())
            });
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.provenance()
        .terms()
        .len()
        .cmp(&right.provenance().terms().len())
}

fn compare_rows(left: &OreConsequence, right: &OreConsequence) -> Ordering {
    for (left, right) in left.row().terms().iter().zip(right.row().terms()) {
        let ordering = left.shift().cmp(right.shift()).then_with(|| {
            left.coefficient()
                .raw()
                .internal_cmp(right.coefficient().raw())
        });
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.row().terms().len().cmp(&right.row().terms().len())
}

fn compare_localization(left: &OreConsequence, right: &OreConsequence) -> Ordering {
    for (left, right) in left
        .localization_witness()
        .guards()
        .iter()
        .zip(right.localization_witness().guards())
    {
        let ordering = left.raw().internal_cmp(right.raw());
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.localization_witness()
        .guards()
        .len()
        .cmp(&right.localization_witness().guards().len())
}
