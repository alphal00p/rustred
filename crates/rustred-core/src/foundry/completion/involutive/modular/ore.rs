use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

use symbolica::domains::finite_field::FiniteFieldCore;

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};

use super::super::{ForwardShift, InvolutiveLimits, OreConsequence, OreOrderingAdapter};
use super::error::{checked_add, reserve_vec};
use super::work::ModularNormalFormWork;
use super::{
    CoeffRef, ModularCoefficientDag, ModularGuideError, ModularGuideLimits, ModularProbe,
    ModularZeroEvidence,
};

/// One structurally nonzero coefficient function at a sparse Ore shift.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModularOreTerm {
    shift: ForwardShift,
    coefficient: CoeffRef,
}

/// One field-independent row owned by a modular normal-form problem.
///
/// Sampled-zero terms deliberately remain in this structural row: translating
/// such a coefficient to another point on the Ore orbit can make it nonzero.
/// Only an exact DAG `KnownZero` is removed by row arithmetic.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModularOreRow {
    terms: Box<[ModularOreTerm]>,
    guards: Box<[CoeffRef]>,
}

#[derive(Debug)]
pub(super) struct SampledSupport {
    term_ordinals: Box<[usize]>,
    residues: Box<[u64]>,
}

impl SampledSupport {
    pub(super) fn len(&self) -> usize {
        self.term_ordinals.len()
    }

    pub(super) fn iter<'row>(
        &'row self,
        row: &'row ModularOreRow,
    ) -> impl DoubleEndedIterator<Item = &'row ModularOreTerm> + ExactSizeIterator + 'row {
        self.term_ordinals.iter().map(|&ordinal| {
            row.terms
                .get(ordinal)
                .expect("sampled support ordinal belongs to its immutable row")
        })
    }

    pub(super) fn entries<'row>(
        &'row self,
        row: &'row ModularOreRow,
    ) -> impl DoubleEndedIterator<Item = (&'row ModularOreTerm, u64)> + 'row {
        self.iter(row).zip(self.residues.iter().copied())
    }

    pub(super) fn residues(&self) -> &[u64] {
        &self.residues
    }

    pub(super) fn try_shifts(
        &self,
        row: &ModularOreRow,
    ) -> Result<Box<[ForwardShift]>, ModularGuideError> {
        let mut shifts = Vec::new();
        reserve_vec(
            &mut shifts,
            self.term_ordinals.len(),
            "modular sampled support shifts",
        )?;
        shifts.extend(self.iter(row).map(|term| term.shift.clone()));
        Ok(shifts.into_boxed_slice())
    }
}

impl ModularOreTerm {
    pub(super) fn shift(&self) -> &ForwardShift {
        &self.shift
    }

    pub(super) fn coefficient(&self) -> &CoeffRef {
        &self.coefficient
    }
}

impl ModularOreRow {
    pub(super) fn try_from_exact(
        consequence: &OreConsequence,
        dag: &mut ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        work: &mut ModularNormalFormWork,
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        work.observe_live_row(
            consequence.row().terms().len(),
            consequence.required_nonzero_guards().len(),
            limits,
        )?;
        let mut terms = Vec::new();
        reserve_vec(
            &mut terms,
            consequence.row().terms().len(),
            "modular exact-row terms",
        )?;
        for term in consequence.row().terms() {
            let coefficient = dag.try_exact_leaf(context, Arc::new(term.coefficient().clone()))?;
            if dag.is_known_zero(&coefficient)? {
                return Err(ModularGuideError::Invariant {
                    detail: "an authenticated exact Ore row retained a zero coefficient",
                });
            }
            terms.push(ModularOreTerm {
                shift: term.shift().clone(),
                coefficient,
            });
        }

        let mut guards = Vec::new();
        reserve_vec(
            &mut guards,
            consequence.required_nonzero_guards().len(),
            "modular exact-row guard references",
        )?;
        for guard in consequence.required_nonzero_guards() {
            guards.push(try_guard_leaf(dag, context, guard)?);
        }
        Ok(Self {
            terms: terms.into_boxed_slice(),
            guards: guards.into_boxed_slice(),
        })
    }

    pub(super) fn terms(&self) -> &[ModularOreTerm] {
        &self.terms
    }

    pub(super) fn guards(&self) -> &[CoeffRef] {
        &self.guards
    }

    /// Make the only mutable per-probe row copy through a fallible,
    /// preflighted allocation boundary. Frozen basis rows are never copied.
    pub(super) fn try_copy(
        &self,
        work: &mut ModularNormalFormWork,
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        work.observe_live_row(self.terms.len(), self.guards.len(), limits)?;
        let mut terms = Vec::new();
        reserve_vec(&mut terms, self.terms.len(), "modular subject-copy terms")?;
        terms.extend(self.terms.iter().cloned());
        let mut guards = Vec::new();
        reserve_vec(
            &mut guards,
            self.guards.len(),
            "modular subject-copy guard references",
        )?;
        guards.extend(self.guards.iter().cloned());
        Ok(Self {
            terms: terms.into_boxed_slice(),
            guards: guards.into_boxed_slice(),
        })
    }

    pub(super) fn coefficient(&self, shift: &ForwardShift) -> Option<&CoeffRef> {
        self.terms
            .binary_search_by(|term| term.shift.cmp(shift))
            .ok()
            .map(|ordinal| &self.terms[ordinal].coefficient)
    }

    pub(super) fn contains_shift(&self, shift: &ForwardShift) -> bool {
        self.terms
            .binary_search_by(|term| term.shift.cmp(shift))
            .is_ok()
    }

    pub(super) fn try_sampled_support(
        &self,
        dag: &ModularCoefficientDag,
        probe: &mut ModularProbe,
        work: &mut ModularNormalFormWork,
        limits: ModularGuideLimits,
    ) -> Result<SampledSupport, ModularGuideError> {
        let mut coefficients = Vec::new();
        reserve_vec(
            &mut coefficients,
            self.terms.len(),
            "modular sampled-row coefficient references",
        )?;
        coefficients.extend(self.terms.iter().map(|term| term.coefficient.clone()));
        let images = probe.try_evaluate_retained_batch(dag, &coefficients)?;
        let mut term_ordinals = Vec::new();
        reserve_vec(
            &mut term_ordinals,
            self.terms.len(),
            "modular sampled support",
        )?;
        let mut residues = Vec::new();
        reserve_vec(
            &mut residues,
            self.terms.len(),
            "modular sampled support residues",
        )?;
        let mut sampled_zeros = 0usize;
        for (ordinal, image) in images.iter().enumerate() {
            match image.zero_evidence() {
                ModularZeroEvidence::KnownZero => {
                    return Err(ModularGuideError::Invariant {
                        detail: "a modular structural row retained a known-zero coefficient",
                    });
                }
                ModularZeroEvidence::SampledZero => {
                    sampled_zeros = sampled_zeros.checked_add(1).ok_or(
                        ModularGuideError::ResourceCountOverflow {
                            resource: "modular sampled-zero observations",
                        },
                    )?;
                }
                ModularZeroEvidence::Nonzero => {
                    term_ordinals.push(ordinal);
                    residues.push(probe.field().from_element(image.value()));
                }
            }
        }
        work.charge_sampled_terms(self.terms.len(), sampled_zeros, limits)?;
        Ok(SampledSupport {
            term_ordinals: term_ordinals.into_boxed_slice(),
            residues: residues.into_boxed_slice(),
        })
    }

    pub(super) fn try_require_guards(
        &self,
        dag: &ModularCoefficientDag,
        probe: &mut ModularProbe,
    ) -> Result<(), ModularGuideError> {
        let images = probe.try_evaluate_retained_batch(dag, &self.guards)?;
        for image in &images {
            match image.zero_evidence() {
                ModularZeroEvidence::Nonzero => {}
                ModularZeroEvidence::SampledZero => {
                    probe.reject();
                    return Err(ModularGuideError::SampledZeroLocalizationGuard);
                }
                ModularZeroEvidence::KnownZero => {
                    probe.reject();
                    return Err(ModularGuideError::Invariant {
                        detail: "a modular row retained a structurally zero localization guard",
                    });
                }
            }
        }
        Ok(())
    }

    /// Return the projectively monic coefficient-function row at this lane's
    /// greatest structural term.
    ///
    /// Only that structural leader is evaluated before pivoting. If it
    /// vanishes at this probe, the complete lane is rejected rather than
    /// silently promoting a lower sampled term. Latent sampled-zero lower
    /// terms remain structurally present, and a successful old leader is
    /// retained as a localization guard.
    pub(super) fn try_monic(
        self,
        ordering: &OreOrderingAdapter,
        dag: &mut ModularCoefficientDag,
        probe: &mut ModularProbe,
        work: &mut ModularNormalFormWork,
        limits: ModularGuideLimits,
    ) -> Result<(Self, Option<ForwardShift>), ModularGuideError> {
        self.try_require_guards(dag, probe)?;
        let mut leading: Option<(&ModularOreTerm, crate::sector::ShiftComplexityKey)> = None;
        for term in &self.terms {
            let key = ordering.try_key(&term.shift)?;
            if leading.as_ref().is_none_or(|(_, current)| key > *current) {
                leading = Some((term, key));
            }
        }
        let Some((leading, _)) = leading else {
            return Ok((self, None));
        };
        let leading_shift = leading.shift.clone();
        let leading_coefficient = leading.coefficient.clone();
        let images =
            probe.try_evaluate_retained_batch(dag, std::slice::from_ref(&leading_coefficient))?;
        let [image] = images.as_ref() else {
            probe.reject();
            return Err(ModularGuideError::Invariant {
                detail: "a one-coefficient modular leader batch did not return exactly one image",
            });
        };
        match image.zero_evidence() {
            ModularZeroEvidence::KnownZero => {
                probe.reject();
                return Err(ModularGuideError::Invariant {
                    detail: "a modular structural leader was a known-zero DAG expression",
                });
            }
            ModularZeroEvidence::SampledZero => {
                let charge = work.charge_sampled_terms(1, 1, limits);
                probe.reject();
                charge?;
                return Err(ModularGuideError::SampledZeroMonicLeader);
            }
            ModularZeroEvidence::Nonzero => work.charge_sampled_terms(1, 0, limits)?,
        }
        if leading_coefficient == dag.one() {
            return Ok((self, Some(leading_shift)));
        }

        work.charge_monic(self.terms.len(), limits)?;
        let inverse = dag.try_inv(&leading_coefficient)?;
        let mut terms = Vec::new();
        reserve_vec(&mut terms, self.terms.len(), "modular monic row terms")?;
        for term in self.terms {
            let coefficient = if term.shift == leading_shift {
                dag.one()
            } else {
                dag.try_mul(&inverse, &term.coefficient)?
            };
            if !dag.is_known_zero(&coefficient)? {
                terms.push(ModularOreTerm {
                    shift: term.shift,
                    coefficient,
                });
            }
        }
        let mut incoming_guard = Vec::new();
        reserve_vec(&mut incoming_guard, 1, "modular monic localization guard")?;
        incoming_guard.push(leading_coefficient);
        let guards = merge_guards(
            self.guards.into_vec(),
            incoming_guard,
            limits.max_live_guard_references,
        )?;
        let result = Self {
            terms: terms.into_boxed_slice(),
            guards,
        };
        work.observe_live_row(result.terms.len(), result.guards.len(), limits)?;
        result.try_require_guards(dag, probe)?;
        Ok((result, Some(leading_shift)))
    }

    /// Consume the accumulator and add
    /// `multiplier * E^operator_shift * source` in the coefficient DAG.
    pub(super) fn try_left_axpy(
        self,
        multiplier: &CoeffRef,
        operator_shift: &ForwardShift,
        source: &Self,
        ordering: &OreOrderingAdapter,
        dag: &mut ModularCoefficientDag,
        probe: &mut ModularProbe,
        exact_limits: InvolutiveLimits,
        work: &mut ModularNormalFormWork,
        limits: ModularGuideLimits,
    ) -> Result<Self, ModularGuideError> {
        if dag.is_known_zero(multiplier)? {
            return Ok(self);
        }
        let input_terms = checked_add(
            "modular Ore AXPY input terms",
            self.terms.len(),
            source.terms.len(),
        )?;
        let transformed_entries = checked_add(
            "modular Ore AXPY transformed entries",
            source.terms.len(),
            source.guards.len(),
        )?;
        work.charge_axpy(
            input_terms,
            transformed_entries,
            operator_shift.arity(),
            limits,
        )?;

        let physical_translation = ordering.try_physical_translation(operator_shift)?;
        let mut transformed = Vec::new();
        reserve_vec(
            &mut transformed,
            source.terms.len(),
            "modular transformed Ore terms",
        )?;
        for term in &source.terms {
            let translated =
                dag.try_translate_physical(&term.coefficient, &physical_translation)?;
            let coefficient = dag.try_mul(multiplier, &translated)?;
            if !dag.is_known_zero(&coefficient)? {
                transformed.push(ModularOreTerm {
                    shift: operator_shift.try_checked_add(&term.shift, exact_limits)?,
                    coefficient,
                });
            }
        }
        let terms = merge_terms(
            self.terms.into_vec(),
            transformed,
            dag,
            input_terms,
            limits.max_live_row_terms,
        )?;

        let mut translated_guards = Vec::new();
        reserve_vec(
            &mut translated_guards,
            source.guards.len(),
            "modular translated guard references",
        )?;
        for guard in &source.guards {
            translated_guards.push(dag.try_translate_physical(guard, &physical_translation)?);
        }
        let guard_images = probe.try_evaluate_retained_batch(dag, &translated_guards)?;
        for image in &guard_images {
            match image.zero_evidence() {
                ModularZeroEvidence::Nonzero => {}
                ModularZeroEvidence::SampledZero => {
                    probe.reject();
                    return Err(ModularGuideError::SampledZeroLocalizationGuard);
                }
                ModularZeroEvidence::KnownZero => {
                    probe.reject();
                    return Err(ModularGuideError::Invariant {
                        detail: "an Ore translation produced a structurally zero localization guard",
                    });
                }
            }
        }
        let guards = merge_guards(
            self.guards.into_vec(),
            translated_guards,
            limits.max_live_guard_references,
        )?;
        let result = Self { terms, guards };
        work.observe_live_row(result.terms.len(), result.guards.len(), limits)?;
        Ok(result)
    }
}

fn try_guard_leaf(
    dag: &mut ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    guard: &Arc<IndexedPolynomial>,
) -> Result<CoeffRef, ModularGuideError> {
    let coefficient = context.coefficient_from_polynomial_sealed(guard)?;
    let reference = dag.try_exact_leaf(context, Arc::new(coefficient))?;
    if dag.is_known_zero(&reference)? {
        return Err(ModularGuideError::Invariant {
            detail: "an exact localization witness retained the zero polynomial",
        });
    }
    Ok(reference)
}

fn merge_terms(
    left: Vec<ModularOreTerm>,
    right: Vec<ModularOreTerm>,
    dag: &mut ModularCoefficientDag,
    capacity: usize,
    live_limit: usize,
) -> Result<Box<[ModularOreTerm]>, ModularGuideError> {
    let mut result = Vec::new();
    reserve_vec(
        &mut result,
        capacity.min(live_limit),
        "modular merged Ore terms",
    )?;
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    while left.peek().is_some() || right.peek().is_some() {
        match (left.peek(), right.peek()) {
            (Some(left_term), Some(right_term)) => match left_term.shift.cmp(&right_term.shift) {
                Ordering::Less => try_push_merged(
                    &mut result,
                    left.next().expect("peeked left Ore term"),
                    live_limit,
                )?,
                Ordering::Greater => try_push_merged(
                    &mut result,
                    right.next().expect("peeked right Ore term"),
                    live_limit,
                )?,
                Ordering::Equal => {
                    let left_term = left.next().expect("peeked left Ore term");
                    let right_term = right.next().expect("peeked right Ore term");
                    let coefficient =
                        dag.try_add(&left_term.coefficient, &right_term.coefficient)?;
                    if !dag.is_known_zero(&coefficient)? {
                        try_push_merged(
                            &mut result,
                            ModularOreTerm {
                                shift: left_term.shift,
                                coefficient,
                            },
                            live_limit,
                        )?;
                    }
                }
            },
            (Some(_), None) => try_push_merged(
                &mut result,
                left.next().expect("peeked left Ore term"),
                live_limit,
            )?,
            (None, Some(_)) => try_push_merged(
                &mut result,
                right.next().expect("peeked right Ore term"),
                live_limit,
            )?,
            (None, None) => break,
        }
    }
    Ok(result.into_boxed_slice())
}

fn try_push_merged(
    result: &mut Vec<ModularOreTerm>,
    term: ModularOreTerm,
    live_limit: usize,
) -> Result<(), ModularGuideError> {
    let requested = checked_add("modular live row terms", result.len(), 1)?;
    if requested > live_limit {
        return Err(ModularGuideError::ResourceLimit {
            resource: "modular live row terms",
            requested,
            limit: live_limit,
        });
    }
    result.push(term);
    Ok(())
}

fn merge_guards(
    retained: Vec<CoeffRef>,
    incoming: Vec<CoeffRef>,
    limit: usize,
) -> Result<Box<[CoeffRef]>, ModularGuideError> {
    let total = checked_add("modular guard references", retained.len(), incoming.len())?;
    let retained_capacity = total.min(limit);
    let mut guards = Vec::new();
    reserve_vec(&mut guards, retained_capacity, "modular guard references")?;
    let mut seen = HashSet::new();
    seen.try_reserve(retained_capacity)
        .map_err(|_| ModularGuideError::AllocationFailure {
            resource: "modular guard-reference set",
            requested: retained_capacity,
        })?;
    for guard in retained.into_iter().chain(incoming) {
        if !seen.contains(&guard) {
            let requested = checked_add("modular guard references", guards.len(), 1)?;
            if requested > limit {
                return Err(ModularGuideError::ResourceLimit {
                    resource: "modular live guard references",
                    requested,
                    limit,
                });
            }
            seen.insert(guard.clone());
            guards.push(guard);
        }
    }
    Ok(guards.into_boxed_slice())
}
