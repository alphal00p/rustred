use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

use super::super::{OwnerAppliedEvent, OwnerDomainMatchPiece, applied};
use super::model::*;
use crate::solver::candidate_reduction::owners::CandidateOwnerPrograms;

fn charge(
    value: &mut usize,
    count: usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), OwnerGuardedFailure> {
    let requested = value
        .checked_add(count)
        .ok_or(OwnerGuardedFailure::CountOverflow { resource })?;
    if requested > limit {
        return Err(OwnerGuardedFailure::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    *value = requested;
    Ok(())
}
fn cancelled(cancel: &AtomicBool) -> Result<(), OwnerGuardedFailure> {
    if cancel.load(Ordering::Acquire) {
        Err(OwnerGuardedFailure::Cancelled)
    } else {
        Ok(())
    }
}
fn emit<const N: usize>(
    event: OwnerGuardedEvent<'_, N>,
    stats: &mut OwnerGuardedStats,
    limits: OwnerGuardedLimits,
    cancel: &AtomicBool,
    visit: &mut impl FnMut(OwnerGuardedEvent<'_, N>) -> ControlFlow<()>,
) -> Result<(), OwnerGuardedFailure> {
    cancelled(cancel)?;
    charge(&mut stats.events, 1, limits.max_events, "guarded events")?;
    if matches!(event, OwnerGuardedEvent::Residual { .. }) {
        charge(&mut stats.residuals, 1, usize::MAX, "guarded residuals")?;
    }
    if matches!(event, OwnerGuardedEvent::Successor(_)) {
        charge(&mut stats.successors, 1, usize::MAX, "guarded successors")?;
    }
    if visit(event).is_break() {
        return Err(OwnerGuardedFailure::StoppedByConsumer);
    }
    cancelled(cancel)
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Inspect one actual saved candidate on its exact guarded subdomain.
    /// `batch`/`rule_ordinal` select immutable data, not applicability authority.
    /// This method constructs all own-case/source/exclusion/pole predicates and
    /// retains the incoming complement. It does NOT run first-priority dispatch,
    /// assert nonempty integer feasibility, use later batch terminals, install
    /// rules, or insert these descriptors into the box-only campaign queue.
    /// Successors remain provisional until RuleFinished without problems, and
    /// even then establish only this guarded one-hop identity, never closure.
    pub fn visit_owner_guarded_rule_successors(
        &self,
        owner: [bool; N],
        batch: usize,
        rule_ordinal: usize,
        lower: &[u64],
        upper: &[Option<u64>],
        rank: Option<u32>,
        limits: OwnerGuardedLimits,
        cancellation: &AtomicBool,
        mut visit: impl FnMut(OwnerGuardedEvent<'_, N>) -> ControlFlow<()>,
    ) -> Result<OwnerGuardedStats, OwnerGuardedError> {
        let mut stats = OwnerGuardedStats::default();
        let mut work = applied::Budget {
            limits: limits.applied,
            stats: Default::default(),
            cancel: cancellation,
        };
        let result = (|| {
            cancelled(cancellation)?;
            if N == 0
                || N > 4096
                || lower.len() != N
                || upper.len() != N
                || lower
                    .iter()
                    .zip(upper)
                    .any(|(&l, u)| u.is_some_and(|u| u < l))
            {
                return Err(OwnerGuardedFailure::InvalidInput("guarded owner/box shape"));
            }
            let prepared = self
                .owners
                .get(&owner)
                .ok_or(OwnerGuardedFailure::UnknownOwner)?;
            let rule = prepared
                .batches
                .get(batch)
                .and_then(|b| b.rules.iter().find(|r| r.ordinal == rule_ordinal))
                .ok_or(OwnerGuardedFailure::UnknownCandidate)?;
            // Outer lo/hi, domain, fixed values, private piece and inverse-image
            // array coexist with inner RHS scratch. Reserve conservatively
            // before even early-residual allocations; inner checks get only
            // the remaining shared peak allowance.
            let outer_coordinates =
                N.checked_mul(10)
                    .ok_or(OwnerGuardedFailure::CountOverflow {
                        resource: "guarded coordinates",
                    })?;
            work.check(
                5,
                work.limits.max_scratch_boxes,
                "guarded outer scratch boxes",
            )
            .map_err(OwnerGuardedFailure::Applied)?;
            work.check(
                outer_coordinates,
                work.limits.max_scratch_coordinate_cells,
                "guarded coordinates",
            )
            .map_err(OwnerGuardedFailure::Applied)?;
            work.limits.max_scratch_boxes -= 5;
            work.limits.max_scratch_coordinate_cells -= outer_coordinates;
            let mut lo = lower.to_vec();
            let mut hi = upper.to_vec();
            for (axis, value) in rule.fixed.iter().enumerate() {
                if let Some(value) = value {
                    let x = if owner[axis] {
                        (i64::from(*value) - 1) as u64
                    } else {
                        i64::from(*value).unsigned_abs()
                    };
                    if x < lo[axis] || hi[axis].is_some_and(|u| x > u) {
                        return emit(
                            OwnerGuardedEvent::Residual {
                                kind: OwnerGuardedResidualKind::EmptyFixedFace,
                                requested_lower: lower,
                                requested_upper: upper,
                                requested_rank: rank,
                                domain: None,
                            },
                            &mut stats,
                            limits,
                            cancellation,
                            &mut visit,
                        );
                    }
                    lo[axis] = x;
                    hi[axis] = Some(x);
                }
            }
            let cell = applied::copy_box(&lo, &hi).map_err(OwnerGuardedFailure::Applied)?;
            let domain = OwnerGuardedDomain {
                owner,
                batch,
                rule,
                cell,
                rank,
                source_conditions: &self.context.shared.source_conditions,
            };
            let residual = |kind| OwnerGuardedEvent::Residual {
                kind,
                requested_lower: lower,
                requested_upper: upper,
                requested_rank: rank,
                domain: Some(&domain),
            };
            if applied::rank_empty(&domain.cell, &owner, rank) {
                return emit(
                    residual(OwnerGuardedResidualKind::RankEmpty),
                    &mut stats,
                    limits,
                    cancellation,
                    &mut visit,
                );
            }
            // Bound the complete borrowed inventory before native work. Whole
            // conjunction boundaries stay in rule.exceptions, never flattened.
            for p in rule
                .equalities
                .iter()
                .chain(rule.exceptions.iter().flatten())
                .chain(domain.source_conditions.iter())
                .chain(rule.rhs.iter().map(|t| &t.denominator))
            {
                cancelled(cancellation)?;
                let mut predicates = stats.predicates;
                let mut terms = stats.predicate_terms;
                charge(
                    &mut predicates,
                    1,
                    limits.max_predicates,
                    "guarded predicates",
                )?;
                charge(
                    &mut terms,
                    p.raw().nterms(),
                    limits.max_predicate_terms,
                    "guarded predicate terms",
                )?;
                stats.predicates = predicates;
                stats.predicate_terms = terms;
            }
            let fixed = match applied::fixed(&domain.cell, &owner, rank) {
                Ok(fixed) => fixed,
                Err(axis) => {
                    return emit(
                        residual(OwnerGuardedResidualKind::UnresolvedFixedCoordinate { axis }),
                        &mut stats,
                        limits,
                        cancellation,
                        &mut visit,
                    );
                }
            };
            // Preserve off-case input even if the candidate domain below is
            // proved empty by an original pole/source/exclusion predicate.
            emit(
                residual(OwnerGuardedResidualKind::IncomingComplement),
                &mut stats,
                limits,
                cancellation,
                &mut visit,
            )?;
            let context = &self.context.shared.context;
            let algebra = self.context.limits.indexed_algebra;
            let affine = rule.case.affine();
            // Native exact zero checks can reject an identically invalid
            // candidate. Every other predicate remains an EXACT domain guard,
            // not an assumed uniform fact about the enclosing box.
            let mut zero = |p| {
                work.native().map_err(OwnerGuardedFailure::Applied)?;
                applied::restriction::polynomial(context, p, &fixed, affine, algebra, &mut work)
                    .map(|p| p.is_zero())
                    .map_err(OwnerGuardedFailure::Applied)
            };
            for (ordinal, condition) in domain.source_conditions.iter().enumerate() {
                if zero(condition)? {
                    return emit(
                        residual(OwnerGuardedResidualKind::InvalidSourceCondition { ordinal }),
                        &mut stats,
                        limits,
                        cancellation,
                        &mut visit,
                    );
                }
            }
            for (branch, conjunction) in rule.exceptions.iter().enumerate() {
                let mut all_zero = true;
                for condition in conjunction {
                    // Check every original atom's admission even after finding
                    // a nonzero polynomial. Its whole AND remains retained.
                    all_zero &= zero(condition)?;
                }
                if all_zero {
                    return emit(
                        residual(OwnerGuardedResidualKind::ExcludedConjunction { branch }),
                        &mut stats,
                        limits,
                        cancellation,
                        &mut visit,
                    );
                }
            }
            for (term, rhs) in rule.rhs.iter().enumerate() {
                if zero(&rhs.denominator)? {
                    return emit(
                        residual(OwnerGuardedResidualKind::OriginalDenominatorZero { term }),
                        &mut stats,
                        limits,
                        cancellation,
                        &mut visit,
                    );
                }
            }
            emit(
                OwnerGuardedEvent::Admitted(&domain),
                &mut stats,
                limits,
                cancellation,
                &mut visit,
            )?;
            let piece = OwnerDomainMatchPiece::guarded_candidate(
                owner,
                applied::copy_box(domain.lower(), domain.upper())
                    .map_err(OwnerGuardedFailure::Applied)?,
                rank,
                batch,
                rule_ordinal,
            );
            let mut callback_failure = None;
            let applied = self.apply_piece(&piece, affine, &mut work, &mut |event| {
                let mapped = match event {
                    // This private synthetic selection is never exposed as
                    // ordinary first-priority matcher/box authority.
                    OwnerAppliedEvent::Classified(_) => return ControlFlow::Continue(()),
                    OwnerAppliedEvent::Successor(child) => {
                        let inverse = inverse_shift(child.shift);
                        OwnerGuardedEvent::Successor(OwnerGuardedSuccessor {
                            image: OwnerGuardedImage {
                                source: &domain,
                                source_lower: child.source_lower,
                                source_upper: child.source_upper,
                                argument_shift: inverse,
                            },
                            target_sector: child.target_sector,
                            target_lower: child.target_lower,
                            target_upper: child.target_upper,
                            target_rank_limit: child.target_rank_limit,
                            coefficient: child.coefficient,
                            coefficient_nonzero: child.coefficient_nonzero,
                            has_installed_target_owner: child.has_installed_target_owner,
                        })
                    }
                    OwnerAppliedEvent::Problem(problem) => OwnerGuardedEvent::Problem {
                        domain: &domain,
                        problem,
                    },
                    OwnerAppliedEvent::OptionalCoefficientRefusal {
                        source_lower,
                        source_upper,
                        shift,
                        original_term_ordinal,
                        failure,
                        ..
                    } => OwnerGuardedEvent::OptionalCoefficientRefusal {
                        domain: &domain,
                        source_lower,
                        source_upper,
                        shift,
                        original_term_ordinal,
                        failure,
                    },
                    OwnerAppliedEvent::RuleFinished {
                        successors,
                        problems,
                        ..
                    } => OwnerGuardedEvent::RuleFinished {
                        domain: &domain,
                        successors,
                        problems,
                    },
                };
                match emit(mapped, &mut stats, limits, cancellation, &mut visit) {
                    Ok(()) => ControlFlow::Continue(()),
                    Err(e) => {
                        callback_failure = Some(e);
                        ControlFlow::Break(())
                    }
                }
            });
            if let Some(failure) = callback_failure {
                return Err(failure);
            }
            applied.map_err(OwnerGuardedFailure::Applied)?;
            cancelled(cancellation)
        })();
        stats.applied = work.stats;
        result
            .map(|()| stats)
            .map_err(|failure| OwnerGuardedError { failure, stats })
    }
}

pub(super) fn inverse_shift<const N: usize>(shift: &[i64; N]) -> [i128; N] {
    std::array::from_fn(|axis| -i128::from(shift[axis]))
}
