use super::super::{OwnerDomainMatchDisposition, OwnerDomainMatchPiece};
use super::{
    algebra::{self, Zero},
    geometry,
    model::*,
};
use crate::algebra::IndexedCoefficient;
use crate::foundry::artifact::prove_wide_descent_with_limits;
use crate::foundry::completion::{CompletionGeometryLimits, LatticeBox};
use crate::solver::candidate_reduction::owners::CandidateOwnerPrograms;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct Budget<'a> {
    pub limits: OwnerAppliedLimits,
    pub stats: OwnerAppliedStats,
    pub cancel: &'a AtomicBool,
}
fn charge(
    value: &mut usize,
    addition: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerAppliedFailure> {
    let requested = value
        .checked_add(addition)
        .ok_or(OwnerAppliedFailure::CountOverflow { resource })?;
    if requested > limit {
        return Err(OwnerAppliedFailure::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *value = requested;
    Ok(())
}
impl Budget<'_> {
    pub fn cancelled(&self) -> Result<(), OwnerAppliedFailure> {
        if self.cancel.load(Ordering::Acquire) {
            Err(OwnerAppliedFailure::Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn check(
        &self,
        requested: usize,
        limit: usize,
        resource: &'static str,
    ) -> Result<(), OwnerAppliedFailure> {
        if requested > limit {
            Err(OwnerAppliedFailure::ResourceLimit {
                resource,
                requested,
                limit,
            })
        } else {
            Ok(())
        }
    }
    pub fn native(&mut self) -> Result<(), OwnerAppliedFailure> {
        self.cancelled()?;
        charge(
            &mut self.stats.native_operations,
            1,
            self.limits.max_native_operations,
            "native operations",
        )
    }
    pub fn sign_splits(&mut self, n: usize) -> Result<(), OwnerAppliedFailure> {
        charge(
            &mut self.stats.sign_splits,
            n,
            self.limits.max_sign_splits,
            "sign splits",
        )
    }
    pub fn boundary(&mut self) -> Result<(), OwnerAppliedFailure> {
        charge(
            &mut self.stats.boundary_cells,
            1,
            self.limits.max_boundary_cells,
            "boundary cells",
        )
    }
    /// Commit total/stage counters together, before later event/cancel checks.
    /// Returns whether this is the first refusal of this phase in the query.
    pub(super) fn optional_refusal(&mut self, original: bool) -> Result<bool, OwnerAppliedFailure> {
        let next = |value: usize, resource| {
            value
                .checked_add(1)
                .ok_or(OwnerAppliedFailure::CountOverflow { resource })
        };
        let total = next(
            self.stats.optional_coefficient_refusals,
            "optional coefficient refusals",
        )?;
        let (original_count, coalesced_count) = if original {
            (
                next(
                    self.stats.optional_original_refusals,
                    "optional original coefficient refusals",
                )?,
                self.stats.optional_coalesced_refusals,
            )
        } else {
            (
                self.stats.optional_original_refusals,
                next(
                    self.stats.optional_coalesced_refusals,
                    "optional coalesced coefficient refusals",
                )?,
            )
        };
        self.stats.optional_coefficient_refusals = total;
        self.stats.optional_original_refusals = original_count;
        self.stats.optional_coalesced_refusals = coalesced_count;
        Ok(if original {
            original_count == 1
        } else {
            coalesced_count == 1
        })
    }
    fn emit<const N: usize>(
        &mut self,
        visit: &mut impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
        event: OwnerAppliedEvent<'_, N>,
    ) -> Result<(), OwnerAppliedFailure> {
        self.cancelled()?;
        charge(&mut self.stats.events, 1, self.limits.max_events, "events")?;
        if visit(event).is_break() {
            Err(OwnerAppliedFailure::StoppedByConsumer)
        } else {
            Ok(())
        }
    }
    fn problem<const N: usize>(
        &mut self,
        visit: &mut impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
        problem: OwnerAppliedProblem<'_, N>,
    ) -> Result<(), OwnerAppliedFailure> {
        charge(&mut self.stats.problems, 1, usize::MAX, "problems")?;
        self.emit(visit, OwnerAppliedEvent::Problem(problem))
    }
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Match and inspect local symbolic RHSs on this immutable owner snapshot.
    /// Successful inspection can contain problems or conditional successors;
    /// neither is a reached missing-rule verdict or recursive closure.
    pub fn visit_owner_applied_successors(
        &self,
        owner: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        rank: Option<u32>,
        limits: OwnerAppliedLimits,
        cancellation: &AtomicBool,
        mut visit: impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
    ) -> Result<OwnerAppliedStats, OwnerAppliedError> {
        let mut budget = Budget {
            limits,
            stats: Default::default(),
            cancel: cancellation,
        };
        let mut failure = None;
        let matched = self.visit_owner_domain_matches(
            owner,
            lower,
            upper,
            rank,
            limits.matching,
            cancellation,
            |piece| match self.apply_piece(&piece, &mut budget, &mut visit) {
                Ok(()) => ControlFlow::Continue(()),
                Err(error) => {
                    failure = Some(error);
                    ControlFlow::Break(())
                }
            },
        );
        match matched {
            Ok(stats) => budget.stats.matching = stats,
            Err(error) => {
                budget.stats.matching = error.stats;
                if failure.is_none() {
                    failure = Some(OwnerAppliedFailure::Matching(error.failure));
                }
            }
        }
        if failure.is_none() {
            failure = budget.cancelled().err();
        }
        match failure {
            Some(failure) => Err(OwnerAppliedError {
                failure,
                stats: budget.stats,
            }),
            None => Ok(budget.stats),
        }
    }

    fn apply_piece(
        &self,
        piece: &OwnerDomainMatchPiece<N>,
        budget: &mut Budget<'_>,
        visit: &mut impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
    ) -> Result<(), OwnerAppliedFailure> {
        budget.emit(visit, OwnerAppliedEvent::Classified(piece))?;
        let OwnerDomainMatchDisposition::SelectedRule { batch, rule } = piece.disposition() else {
            return Ok(());
        };
        let prepared = &self.owners[piece.owner()].batches[batch];
        let rule = prepared.rules.iter().find(|r| r.ordinal == rule).ok_or(
            OwnerAppliedFailure::InternalInvariant("matched rule absent from same programs"),
        )?;
        charge(
            &mut budget.stats.selected_pieces,
            1,
            usize::MAX,
            "selected pieces",
        )?;
        budget.check(
            rule.rhs.len(),
            budget.limits.max_scratch_terms,
            "staged term indices",
        )?;
        budget.check(12, budget.limits.max_scratch_boxes, "scratch boxes")?;
        let coordinates = N
            .checked_mul(24)
            .ok_or(OwnerAppliedFailure::CountOverflow {
                resource: "scratch coordinate cells",
            })?;
        budget.check(
            coordinates,
            budget.limits.max_scratch_coordinate_cells,
            "scratch coordinate cells",
        )?;
        let mut indices = Vec::new();
        indices.try_reserve_exact(rule.rhs.len()).map_err(|_| {
            OwnerAppliedFailure::AllocationFailure {
                resource: "staged term indices",
            }
        })?;
        indices.extend(0..rule.rhs.len());
        indices
            .sort_unstable_by(|&a, &b| rule.rhs[a].shift.cmp(&rule.rhs[b].shift).then(a.cmp(&b)));
        let source = geometry::copy_box(piece.lower(), piece.upper())?;
        let before_successors = budget.stats.successors;
        let before_problems = budget.stats.problems;
        let mut start = 0;
        while start < indices.len() {
            budget.cancelled()?;
            let shift = &rule.rhs[indices[start]].shift;
            let mut end = start + 1;
            while end < indices.len() && rule.rhs[indices[end]].shift == *shift {
                end += 1;
            }
            charge(
                &mut budget.stats.shift_groups,
                1,
                budget.limits.max_shift_groups,
                "shift groups",
            )?;
            for sign in geometry::sign_cells(&source, piece.owner(), shift, budget)? {
                if geometry::rank_empty(&sign, piece.owner(), piece.max_numerator_rank()) {
                    continue;
                }
                let mut boundary = geometry::Boundaries::new(&sign, piece.owner(), shift, budget)?;
                while let Some(cell) = boundary.next(budget)? {
                    if !geometry::rank_empty(&cell, piece.owner(), piece.max_numerator_rank()) {
                        self.apply_group(
                            piece,
                            &cell,
                            rule,
                            &indices[start..end],
                            shift,
                            budget,
                            visit,
                        )?;
                    }
                }
            }
            start = end;
        }
        budget.emit(
            visit,
            OwnerAppliedEvent::RuleFinished {
                source: piece,
                successors: budget.stats.successors - before_successors,
                problems: budget.stats.problems - before_problems,
            },
        )
    }

    fn classify_coefficient(
        &self,
        piece: &OwnerDomainMatchPiece<N>,
        cell: &LatticeBox,
        shift: &[i64; N],
        original_term_ordinal: Option<usize>,
        coefficient: &IndexedCoefficient,
        budget: &mut Budget<'_>,
        visit: &mut impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
    ) -> Result<Zero, OwnerAppliedFailure> {
        let classification = algebra::coefficient(
            &self.context.shared.context,
            coefficient,
            cell,
            piece.owner(),
            piece.max_numerator_rank(),
            self.context.limits.indexed_algebra,
            budget,
        )?;
        if let Some(failure) = classification.optional_refusal {
            if budget.optional_refusal(original_term_ordinal.is_some())? {
                budget.emit(
                    visit,
                    OwnerAppliedEvent::OptionalCoefficientRefusal {
                        source: piece,
                        source_lower: cell.lower(),
                        source_upper: cell.upper(),
                        shift,
                        original_term_ordinal,
                        failure: &failure,
                    },
                )?;
            }
        }
        budget.cancelled()?;
        Ok(classification.zero)
    }

    fn apply_group(
        &self,
        piece: &OwnerDomainMatchPiece<N>,
        cell: &LatticeBox,
        rule: &crate::solver::candidate_reduction::model::PreparedRule<N>,
        indices: &[usize],
        shift: &[i64; N],
        budget: &mut Budget<'_>,
        visit: &mut impl FnMut(OwnerAppliedEvent<'_, N>) -> ControlFlow<()>,
    ) -> Result<(), OwnerAppliedFailure> {
        let problem = |kind, ordinal, coefficient, nonzero| OwnerAppliedProblem {
            source: piece,
            source_lower: cell.lower(),
            source_upper: cell.upper(),
            shift,
            original_term_ordinal: ordinal,
            coefficient,
            coefficient_nonzero: nonzero,
            kind,
        };
        let fixed = match geometry::fixed(cell, piece.owner(), piece.max_numerator_rank()) {
            Ok(fixed) => fixed,
            Err(axis) => {
                return budget.problem(
                    visit,
                    problem(
                        OwnerAppliedProblemKind::UnresolvedFixedCoordinate { axis },
                        None,
                        None,
                        OwnerAppliedNonzero::Conditional,
                    ),
                );
            }
        };
        let target: [bool; N] = std::array::from_fn(|axis| {
            let x = i128::from(cell.lower()[axis]);
            (if piece.owner()[axis] { x + 1 } else { -x }) + i128::from(shift[axis]) > 0
        });
        let context = &self.context.shared.context;
        let algebra_limits = self.context.limits.indexed_algebra;
        let mut sum: Option<IndexedCoefficient> = None;
        let mut valid = false;
        let mut zero_sector = false;
        for &ordinal in indices {
            budget.cancelled()?;
            charge(
                &mut budget.stats.term_visits,
                1,
                budget.limits.max_term_visits,
                "RHS term visits",
            )?;
            budget.native()?;
            let (coefficient, _original_denominator) = context
                .specialize_fixed_indices_sealed(
                    &rule.rhs[ordinal].coefficient,
                    &fixed,
                    algebra_limits,
                )
                .map_err(OwnerAppliedFailure::Algebra)?;
            let nonzero = self.classify_coefficient(
                piece,
                cell,
                shift,
                Some(ordinal),
                &coefficient,
                budget,
                visit,
            )?;
            if nonzero == Zero::Yes {
                budget.stats.zero_terms += 1;
                continue;
            }
            if !valid {
                if target
                    .iter()
                    .zip(self.owners[piece.owner()].root)
                    .any(|(&active, root)| active && !root)
                {
                    return budget.problem(
                        visit,
                        problem(
                            OwnerAppliedProblemKind::InvalidChildRoot,
                            Some(ordinal),
                            Some(&coefficient),
                            nonzero.nonzero(),
                        ),
                    );
                }
                for (condition_ordinal, condition) in
                    self.context.shared.source_conditions.iter().enumerate()
                {
                    budget.native()?;
                    let shifted = context
                        .translate_polynomial_sealed(condition, shift, algebra_limits)
                        .map_err(OwnerAppliedFailure::Algebra)?;
                    budget.native()?;
                    let restricted = context
                        .specialize_fixed_polynomial_sealed(&shifted, &fixed, algebra_limits)
                        .map_err(OwnerAppliedFailure::Algebra)?;
                    let kind = match algebra::polynomial(
                        context,
                        &restricted,
                        cell,
                        piece.owner(),
                        piece.max_numerator_rank(),
                        algebra_limits,
                        budget,
                    )? {
                        Zero::No => continue,
                        Zero::Yes => OwnerAppliedProblemKind::InvalidChildSourceCondition {
                            ordinal: condition_ordinal,
                        },
                        Zero::Unknown => OwnerAppliedProblemKind::UnresolvedChildSourceCondition {
                            ordinal: condition_ordinal,
                        },
                    };
                    return budget.problem(
                        visit,
                        problem(kind, Some(ordinal), Some(&coefficient), nonzero.nonzero()),
                    );
                }
                zero_sector = self.context.shared.zero_sectors.contains(&target);
                if !zero_sector {
                    if let Err(error) = prove_wide_descent_with_limits(
                        [(shift.as_slice(), &())],
                        std::slice::from_ref(cell),
                        piece.owner(),
                        self.owners[piece.owner()].ordering,
                        CompletionGeometryLimits {
                            max_uncovered_boxes: 1,
                            max_uncovered_box_coordinate_cells: budget
                                .limits
                                .max_scratch_coordinate_cells,
                            max_split_operations: 0,
                            ..Default::default()
                        },
                        |_, _| Ok(false),
                    ) {
                        return budget.problem(
                            visit,
                            problem(
                                OwnerAppliedProblemKind::DescentNotEstablished {
                                    detail: error.to_string(),
                                },
                                Some(ordinal),
                                Some(&coefficient),
                                nonzero.nonzero(),
                            ),
                        );
                    }
                }
                valid = true;
            }
            if zero_sector {
                continue;
            }
            sum = Some(match sum {
                None => coefficient,
                Some(previous) => {
                    budget.native()?;
                    budget.stats.coalescing_additions += 1;
                    context
                        .add_with_limits(&previous, &coefficient, algebra_limits.exact_algebra)
                        .map_err(OwnerAppliedFailure::Algebra)?
                }
            });
        }
        if zero_sector {
            budget.stats.zero_sector_groups += 1;
            return Ok(());
        }
        let Some(coefficient) = sum else {
            return Ok(());
        };
        let nonzero =
            self.classify_coefficient(piece, cell, shift, None, &coefficient, budget, visit)?;
        if nonzero == Zero::Yes {
            budget.stats.cancelled_groups += 1;
            return Ok(());
        }
        let image = match geometry::image(cell, piece.owner(), shift, piece.max_numerator_rank()) {
            Ok(image) => image,
            Err(detail) => {
                return budget.problem(
                    visit,
                    problem(
                        OwnerAppliedProblemKind::UnresolvedImage { detail },
                        None,
                        Some(&coefficient),
                        nonzero.nonzero(),
                    ),
                );
            }
        };
        charge(&mut budget.stats.successors, 1, usize::MAX, "successors")?;
        if nonzero == Zero::Unknown {
            budget.stats.conditional_successors += 1;
        }
        budget.emit(
            visit,
            OwnerAppliedEvent::Successor(OwnerAppliedSuccessor {
                source: piece,
                source_lower: cell.lower(),
                source_upper: cell.upper(),
                target_sector: &image.sector,
                target_lower: &image.lower,
                target_upper: &image.upper,
                target_rank_limit: image.rank,
                shift,
                coefficient: &coefficient,
                coefficient_nonzero: nonzero.nonzero(),
                has_installed_target_owner: self.owners.contains_key(&image.sector),
            }),
        )
    }
}
