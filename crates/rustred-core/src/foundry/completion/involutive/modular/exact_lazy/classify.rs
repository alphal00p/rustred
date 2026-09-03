use std::collections::{HashMap, HashSet};

use crate::algebra::IndexedCoefficientContext;

use super::super::{
    ModularGuardQuery, ModularGuideError, NonzeroCertification, try_certify_typed_batch,
};
use super::{
    AccountedProbeOutcome, ClassifiedLazyOreRow, ExactFallbackNonzeroProof,
    ExactFallbackZeroAuthority, ExactLazyError, ExactLazyProbeSchedule, ExactLazySupportBudget,
    ExactLazyTransaction, ExactNonzeroProof, ExactZeroProof, GuardProbeRequirement, LazyCoeff,
    LazyOreTerm, ModularNonzeroProof, StructuralZeroProof, UnclassifiedLazyOreRow,
};

/// Unforgeable authority that the classifier audited one complete consumed
/// unclassified transition. Only this module can mint the private field.
pub(super) struct SupportClassificationSeal {
    _private: (),
}

/// Resolve the complete exact support of one post-AXPY row.
///
/// The row is consumed and no classified prefix can escape. Unchanged terms
/// retain their prior exact authority. Changed roots are queried once in
/// canonical shift order at each deterministic probe ordinal; sampled zeros
/// remain unresolved. Every residual root is then materialized in one bounded
/// Symbolica batch, producing exact zero or nonzero authority before the row
/// can enter scheduler-visible `ClassifiedLazyOreRow` state.
pub(super) fn try_classify_support(
    transaction: &ExactLazyTransaction<'_, '_>,
    context: &IndexedCoefficientContext,
    guards: &[GuardProbeRequirement],
    row: UnclassifiedLazyOreRow,
    schedule: &ExactLazyProbeSchedule,
    budget: &mut ExactLazySupportBudget,
) -> Result<ClassifiedLazyOreRow, ExactLazyError> {
    schedule.require_owner(transaction.owner())?;
    if !transaction.coefficient_dag().owns_context(context) {
        return Err(ExactLazyError::WrongIndexedContext);
    }
    let requested_roots = row.pending_term_count().checked_add(guards.len()).ok_or(
        ExactLazyError::ResourceCountOverflow {
            resource: "exact-lazy support-classification roots",
        },
    )?;
    budget.try_start_classification(transaction.owner(), requested_roots)?;
    let dag = transaction.coefficient_dag();
    let guard_queries = canonical_guard_queries(transaction, guards)?;

    let (pending, structural_zero_elisions) = row.into_parts();
    let zero_capacity = structural_zero_elisions
        .len()
        .checked_add(pending.len())
        .ok_or(ExactLazyError::ResourceCountOverflow {
            resource: "exact-lazy classified zero proofs",
        })?;
    let mut zero_elisions = Vec::new();
    zero_elisions
        .try_reserve_exact(zero_capacity)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: "exact-lazy classified zero proofs",
            requested: zero_capacity,
        })?;
    zero_elisions.extend(
        structural_zero_elisions
            .into_vec()
            .into_iter()
            .map(ExactZeroProof::Structural),
    );

    let mut expected_transition =
        try_vec_capacity("exact-lazy support-transition manifest", zero_capacity)?;
    expected_transition.extend(
        zero_elisions
            .iter()
            .map(|proof| (proof.shift().clone(), proof.root().clone())),
    );

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(pending.len())
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: "exact-lazy support-classification slots",
            requested: pending.len(),
        })?;
    let mut unique_roots = Vec::new();
    unique_roots.try_reserve_exact(pending.len()).map_err(|_| {
        ExactLazyError::AllocationFailure {
            resource: "exact-lazy unique changed coefficient roots",
            requested: pending.len(),
        }
    })?;
    let mut root_lookup = HashMap::new();
    root_lookup
        .try_reserve(pending.len())
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: "exact-lazy changed-root lookup",
            requested: pending.len(),
        })?;
    let mut previous_shift = None;
    for term in pending.into_vec() {
        if previous_shift
            .as_ref()
            .is_some_and(|previous| previous >= term.shift())
        {
            return Err(ExactLazyError::InvalidSupport {
                detail: "unclassified Ore support is not strictly shift sorted",
            });
        }
        previous_shift = Some(term.shift().clone());
        expected_transition.push((term.shift().clone(), term.coefficient().clone()));
        let (shift, coefficient, prior_proof) = term.into_parts();
        transaction.require_lazy_coefficient(&coefficient)?;
        if let Some(proof) = prior_proof {
            slots.push(ClassificationSlot {
                shift,
                coefficient,
                resolution: SlotResolution::Nonzero(proof),
            });
        } else if transaction.try_is_structural_zero(&coefficient)? {
            let proof = StructuralZeroProof::try_new(transaction, shift.clone(), &coefficient)?;
            slots.push(ClassificationSlot {
                shift,
                coefficient,
                resolution: SlotResolution::Zero(ExactZeroProof::Structural(proof)),
            });
        } else {
            let root_index = if let Some(&index) = root_lookup.get(coefficient.root()) {
                index
            } else {
                let index = unique_roots.len();
                root_lookup.insert(coefficient.root().clone(), index);
                unique_roots.push(coefficient.clone());
                index
            };
            slots.push(ClassificationSlot {
                shift,
                coefficient,
                resolution: SlotResolution::Changed(root_index),
            });
        }
    }

    let mut root_proofs =
        try_vec_capacity("exact-lazy changed-root support proofs", unique_roots.len())?;
    root_proofs.resize_with(unique_roots.len(), || None);
    let mut exact_zero_roots =
        try_vec_capacity("exact-lazy exact-zero root authorities", unique_roots.len())?;
    exact_zero_roots.resize_with(unique_roots.len(), || None);
    let mut guard_roots = HashSet::new();
    guard_roots.try_reserve(guard_queries.len()).map_err(|_| {
        ExactLazyError::AllocationFailure {
            resource: "exact-lazy canonical guard-root set",
            requested: guard_queries.len(),
        }
    })?;
    guard_roots.extend(guard_queries.iter().map(|guard| guard.root().clone()));

    for spec in schedule.specs() {
        let mut unresolved_indices = try_vec_capacity(
            "exact-lazy unresolved modular root indices",
            unique_roots.len(),
        )?;
        unresolved_indices.extend(unique_roots.iter().enumerate().filter_map(|(index, root)| {
            // Certificate layouts are globally unique. If this root is also
            // a point-admissibility guard, leave its support decision to the
            // exact batch. Sound now; sharing that guard image is a possible
            // later optimization.
            (root_proofs[index].is_none() && !guard_roots.contains(root.root())).then_some(index)
        }));
        if unresolved_indices.is_empty() {
            break;
        }
        let mut coefficient_roots = try_vec_capacity(
            "exact-lazy modular coefficient queries",
            unresolved_indices.len(),
        )?;
        coefficient_roots.extend(
            unresolved_indices
                .iter()
                .map(|&index| unique_roots[index].root().clone()),
        );
        let raw = try_certify_typed_batch(
            dag,
            context,
            &guard_queries,
            &coefficient_roots,
            spec.ordinal(),
            spec.modulus(),
            spec.full_integer_point(),
            transaction.owner().limits().coefficient,
        );
        match budget.try_account_probe(transaction.owner(), raw)? {
            AccountedProbeOutcome::Complete(batch) => {
                if !batch.owns_typed(dag, context, &guard_queries, &coefficient_roots) {
                    return Err(ExactLazyError::InvalidProof {
                        detail: "an accounted support batch lost its canonical owner/layout binding",
                    });
                }
                let outcomes = batch.into_outcomes();
                if outcomes.len() != unresolved_indices.len() {
                    return Err(ExactLazyError::InvalidSupport {
                        detail: "support batch outcome count disagrees with unresolved roots",
                    });
                }
                for ((outcome, root_index), expected_root) in outcomes
                    .into_vec()
                    .into_iter()
                    .zip(unresolved_indices)
                    .zip(coefficient_roots)
                {
                    match outcome {
                        NonzeroCertification::Certified(certificate) => {
                            let root = unique_roots[root_index].clone();
                            if root.root() != &expected_root {
                                return Err(ExactLazyError::InvalidSupport {
                                    detail: "support certificate position changed after batch issuance",
                                });
                            }
                            root_proofs[root_index] =
                                Some(ExactNonzeroProof::Modular(ModularNonzeroProof::try_new(
                                    transaction.owner(),
                                    dag,
                                    context,
                                    root,
                                    certificate,
                                )?));
                        }
                        NonzeroCertification::Unresolved(sampled_zero) => {
                            if !sampled_zero.owns(dag, context, &expected_root) {
                                return Err(ExactLazyError::InvalidProof {
                                    detail: "sampled-zero evidence lost its root/layout binding",
                                });
                            }
                        }
                    }
                }
            }
            AccountedProbeOutcome::Rejected(error) if is_point_rejection(&error) => {}
            AccountedProbeOutcome::Rejected(error) => return Err(error.into()),
        }
    }

    let mut fallback_indices =
        try_vec_capacity("exact-lazy exact-fallback root indices", unique_roots.len())?;
    fallback_indices.extend(
        root_proofs
            .iter()
            .enumerate()
            .filter_map(|(index, proof)| proof.is_none().then_some(index)),
    );
    if !fallback_indices.is_empty() {
        let mut fallback_roots = try_vec_capacity(
            "exact-lazy exact-fallback coefficient roots",
            fallback_indices.len(),
        )?;
        fallback_roots.extend(
            fallback_indices
                .iter()
                .map(|&index| unique_roots[index].root().clone()),
        );
        let exact =
            budget.try_materialize_fallback(transaction.owner(), dag, context, &fallback_roots)?;
        if !exact.owns(dag, context, &fallback_roots) {
            return Err(ExactLazyError::InvalidProof {
                detail: "an exact fallback batch lost its canonical root binding",
            });
        }
        let materializations = exact.into_materializations();
        if materializations.len() != fallback_indices.len() {
            return Err(ExactLazyError::InvalidSupport {
                detail: "exact fallback outcome count disagrees with unresolved roots",
            });
        }
        for (materialization, root_index) in materializations
            .into_vec()
            .into_iter()
            .zip(fallback_indices)
        {
            let root = unique_roots[root_index].clone();
            if materialization.value().is_zero() {
                exact_zero_roots[root_index] = Some(ExactFallbackZeroAuthority::try_new(
                    transaction.owner(),
                    dag,
                    context,
                    root,
                    materialization,
                )?);
            } else {
                root_proofs[root_index] = Some(ExactNonzeroProof::ExactFallback(
                    ExactFallbackNonzeroProof::try_new(
                        transaction.owner(),
                        dag,
                        context,
                        root,
                        materialization,
                    )?,
                ));
            }
        }
    }

    let mut classified = Vec::new();
    classified
        .try_reserve_exact(slots.len())
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: "exact-lazy classified Ore terms",
            requested: slots.len(),
        })?;
    for slot in slots {
        let proof = match slot.resolution {
            SlotResolution::Nonzero(proof) => Some(proof),
            SlotResolution::Zero(zero) => {
                zero_elisions.push(zero);
                None
            }
            SlotResolution::Changed(root_index) => {
                if let Some(authority) = exact_zero_roots[root_index].as_ref() {
                    zero_elisions.push(ExactZeroProof::ExactFallback(
                        authority.bind_shift(slot.shift.clone()),
                    ));
                    None
                } else {
                    root_proofs[root_index].clone()
                }
            }
        };
        if let Some(proof) = proof {
            classified.push(LazyOreTerm::try_new(
                transaction,
                slot.shift,
                slot.coefficient,
                proof,
            )?);
        }
    }
    audit_complete_transition(&mut expected_transition, &classified, &zero_elisions)?;
    ClassifiedLazyOreRow::try_from_classification(
        transaction,
        classified,
        zero_elisions,
        SupportClassificationSeal { _private: () },
    )
}

fn audit_complete_transition(
    expected: &mut Vec<(super::super::super::ForwardShift, LazyCoeff)>,
    retained: &[LazyOreTerm],
    zero_elisions: &[ExactZeroProof],
) -> Result<(), ExactLazyError> {
    expected.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if expected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(ExactLazyError::InvalidSupport {
            detail: "unclassified support transition contains a duplicate shift",
        });
    }
    let actual_capacity = retained.len().checked_add(zero_elisions.len()).ok_or(
        ExactLazyError::ResourceCountOverflow {
            resource: "exact-lazy classified support-transition manifest",
        },
    )?;
    let mut actual = try_vec_capacity(
        "exact-lazy classified support-transition manifest",
        actual_capacity,
    )?;
    actual.extend(
        retained
            .iter()
            .map(|term| (term.shift().clone(), term.coefficient().clone())),
    );
    actual.extend(
        zero_elisions
            .iter()
            .map(|proof| (proof.shift().clone(), proof.root().clone())),
    );
    actual.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if actual.windows(2).any(|pair| pair[0].0 == pair[1].0) || actual != *expected {
        return Err(ExactLazyError::InvalidSupport {
            detail: "classified support did not account for the complete input transition",
        });
    }
    Ok(())
}

fn canonical_guard_queries(
    transaction: &ExactLazyTransaction<'_, '_>,
    guards: &[GuardProbeRequirement],
) -> Result<Vec<ModularGuardQuery>, ExactLazyError> {
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(guards.len())
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource: "exact-lazy canonical guard queries",
            requested: guards.len(),
        })?;
    for guard in guards {
        transaction.require_lazy_coefficient(guard_root(guard))?;
        canonical.push(guard.clone());
    }
    canonical.sort_unstable_by(|left, right| {
        guard_root(left)
            .root()
            .raw
            .cmp(&guard_root(right).root().raw)
            .then_with(|| guard_rank(left).cmp(&guard_rank(right)))
    });
    canonical.dedup_by(|later, earlier| guard_root(later).root() == guard_root(earlier).root());
    let mut queries = try_vec_capacity("exact-lazy modular guard queries", canonical.len())?;
    queries.extend(canonical.into_iter().map(|guard| match guard {
        GuardProbeRequirement::Nonzero(root) => ModularGuardQuery::Nonzero(root.root().clone()),
        GuardProbeRequirement::Defined(root) => ModularGuardQuery::Defined(root.root().clone()),
    }));
    Ok(queries)
}

fn guard_root(guard: &GuardProbeRequirement) -> &LazyCoeff {
    match guard {
        GuardProbeRequirement::Nonzero(root) | GuardProbeRequirement::Defined(root) => root,
    }
}

const fn guard_rank(guard: &GuardProbeRequirement) -> u8 {
    match guard {
        GuardProbeRequirement::Nonzero(_) => 0,
        GuardProbeRequirement::Defined(_) => 1,
    }
}

fn is_point_rejection(error: &ModularGuideError) -> bool {
    matches!(
        error,
        ModularGuideError::SampledZeroLocalizationGuard
            | ModularGuideError::SingularExactLeaf { .. }
            | ModularGuideError::SingularInverse { .. }
    )
}

struct ClassificationSlot {
    shift: super::super::super::ForwardShift,
    coefficient: LazyCoeff,
    resolution: SlotResolution,
}

enum SlotResolution {
    Nonzero(ExactNonzeroProof),
    Changed(usize),
    Zero(ExactZeroProof),
}

fn try_vec_capacity<T>(resource: &'static str, capacity: usize) -> Result<Vec<T>, ExactLazyError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ExactLazyError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}
