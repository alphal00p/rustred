use std::ops::{Bound, ControlFlow};
use std::sync::atomic::{AtomicBool, Ordering};

use super::geometry::{Budget, charge, geometry, minimum_rank};
use super::guards::{self, Resolution};
use super::model::*;
use crate::algebra::IndexedPolynomial;
use crate::family::IntegralKey;
use crate::foundry::completion::LatticeBox;
use crate::solver::candidate_reduction::owners::CandidateOwnerPrograms;

#[derive(Clone, Copy)]
enum RuleStage {
    Fixed,
    Equality(usize),
    Exception(usize, usize),
    Denominator(usize),
}
/// A refinement retries exactly the predicate that could not be classified,
/// never a later rule. Keeping this cursor separate avoids recursive Phase
/// storage and keeps arbitrarily long positive tails entirely symbolic.
#[derive(Clone, Copy)]
enum PredicateResume {
    Source(usize),
    Rule {
        batch: usize,
        index: usize,
        stage: RuleStage,
    },
}
impl PredicateResume {
    fn phase<'a>(self) -> Phase<'a> {
        match self {
            Self::Source(ordinal) => Phase::Source(ordinal),
            Self::Rule {
                batch,
                index,
                stage,
            } => Phase::Rule {
                batch,
                index,
                stage,
            },
        }
    }
}
#[derive(Clone, Copy)]
enum Phase<'a> {
    Source(usize),
    Terminals {
        batch: usize,
        after: Option<&'a IntegralKey>,
    },
    Rule {
        batch: usize,
        index: usize,
        stage: RuleStage,
    },
    /// All faces are already charged. Keep only the next face and one
    /// continuation live rather than publishing a width-sized work stack.
    Refinement {
        axis: usize,
        next: u64,
        upper: u64,
        resume: PredicateResume,
    },
    Emit(OwnerDomainMatchDisposition),
}
struct Task<'a> {
    cell: LatticeBox,
    phase: Phase<'a>,
}
struct Matcher<'a, 'v, const N: usize, F> {
    programs: &'a CandidateOwnerPrograms<N>,
    owner: [bool; N],
    rank: Option<u32>,
    budget: Budget,
    cancel: &'v AtomicBool,
    visit: &'v mut F,
    pending: Vec<Task<'a>>,
}

impl<const N: usize> CandidateOwnerPrograms<N> {
    /// Match an explicit local box against ordered installed-program guards.
    /// Each result retains the exact requested inactive-rank simplex, which may
    /// exceed the saved entry scope. Unsupported predicates stop fall-through
    /// on their piece. Successful exhaustion may include unresolved/invalid
    /// pieces, and never implies RHS success, recursive closure, or that an
    /// externally supplied box is the exact image of an earlier dependency.
    /// Previously emitted pieces remain an incomplete prefix on error/cancel.
    pub fn visit_owner_domain_matches(
        &self,
        owner: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        max_numerator_rank: Option<u32>,
        limits: OwnerDomainMatchLimits,
        cancellation: &AtomicBool,
        mut visit: impl FnMut(OwnerDomainMatchPiece<N>) -> ControlFlow<()>,
    ) -> Result<OwnerDomainMatchStats, OwnerDomainMatchError> {
        let mut matcher = Matcher {
            programs: self,
            owner,
            rank: max_numerator_rank,
            budget: Budget {
                limits,
                stats: Default::default(),
            },
            cancel: cancellation,
            visit: &mut visit,
            pending: Vec::new(),
        };
        let result = matcher.run(lower, upper);
        result
            .map(|()| matcher.budget.stats)
            .map_err(|failure| OwnerDomainMatchError {
                failure,
                stats: matcher.budget.stats,
            })
    }
}

impl<'a, const N: usize, F: FnMut(OwnerDomainMatchPiece<N>) -> ControlFlow<()>>
    Matcher<'a, '_, N, F>
{
    fn cancelled(&self) -> Result<(), OwnerDomainMatchFailure> {
        if self.cancel.load(Ordering::Acquire) {
            Err(OwnerDomainMatchFailure::Cancelled)
        } else {
            Ok(())
        }
    }
    fn push(&mut self, cell: LatticeBox, phase: Phase<'a>) -> Result<(), OwnerDomainMatchFailure> {
        self.cancelled()?;
        if self
            .rank
            .is_some_and(|r| minimum_rank(&cell, &self.owner) > u128::from(r))
        {
            charge(
                &mut self.budget.stats.rank_empty_cells,
                1,
                usize::MAX,
                "rank empty cells",
            )?;
            return Ok(());
        }
        self.pending
            .try_reserve(1)
            .map_err(|_| OwnerDomainMatchFailure::AllocationFailure {
                resource: "pending match cells",
            })?;
        self.pending.push(Task { cell, phase });
        Ok(())
    }
    fn run(&mut self, lower: &[u64], upper: &[Option<u64>]) -> Result<(), OwnerDomainMatchFailure> {
        self.cancelled()?;
        if N == 0 || N > 4096 || lower.len() != N || upper.len() != N {
            return Err(OwnerDomainMatchFailure::InvalidInput(
                "owner/box arity must match and lie in 1..=4096".into(),
            ));
        }
        if !self.programs.owners.contains_key(&self.owner)
            && !self
                .programs
                .context
                .shared
                .zero_sectors
                .contains(&self.owner)
        {
            return Err(OwnerDomainMatchFailure::UnknownOwner);
        }
        if lower
            .iter()
            .zip(upper)
            .any(|(&l, &u)| u.is_some_and(|u| u < l))
        {
            return Err(OwnerDomainMatchFailure::InvalidInput(
                "lower endpoint exceeds upper endpoint".into(),
            ));
        }
        self.budget.cells::<N>(1)?;
        let cell =
            LatticeBox::try_new(lower.iter().copied(), upper.iter().copied()).map_err(geometry)?;
        self.push(cell, Phase::Source(0))?;
        while let Some(Task { cell, phase }) = self.pending.pop() {
            self.cancelled()?;
            self.advance(cell, phase)?;
        }
        self.cancelled()
    }
    fn emit(
        &mut self,
        cell: LatticeBox,
        disposition: OwnerDomainMatchDisposition,
    ) -> Result<(), OwnerDomainMatchFailure> {
        self.cancelled()?;
        charge(
            &mut self.budget.stats.pieces,
            1,
            self.budget.limits.max_pieces,
            "pieces",
        )?;
        let piece = OwnerDomainMatchPiece {
            owner: self.owner,
            cell,
            rank: self.rank,
            disposition,
        };
        if (self.visit)(piece).is_break() {
            return Err(OwnerDomainMatchFailure::StoppedByConsumer);
        }
        Ok(())
    }
    fn advance(
        &mut self,
        cell: LatticeBox,
        phase: Phase<'a>,
    ) -> Result<(), OwnerDomainMatchFailure> {
        let programs = self.programs;
        match phase {
            Phase::Emit(disposition) => self.emit(cell, disposition),
            Phase::Refinement {
                axis,
                next,
                upper,
                resume,
            } => {
                // The complete interval was admitted before any child was
                // published. This native box construction spends prepaid cells.
                let face = LatticeBox::try_new(
                    cell.lower()
                        .iter()
                        .enumerate()
                        .map(|(i, &x)| if i == axis { next } else { x }),
                    cell.upper()
                        .iter()
                        .enumerate()
                        .map(|(i, &x)| if i == axis { Some(next) } else { x }),
                )
                .map_err(geometry)?;
                if next < upper {
                    self.push(
                        cell,
                        Phase::Refinement {
                            axis,
                            next: next + 1,
                            upper,
                            resume,
                        },
                    )?;
                }
                self.push(face, resume.phase())
            }
            Phase::Source(ordinal) => {
                if let Some(polynomial) = programs.context.shared.source_conditions.get(ordinal) {
                    self.predicate(
                        cell,
                        polynomial,
                        OwnerDomainPredicate::SourceCondition { ordinal },
                        Phase::Emit(OwnerDomainMatchDisposition::InvalidSourceCondition {
                            ordinal,
                        }),
                        Phase::Source(ordinal + 1),
                        PredicateResume::Source(ordinal),
                    )
                } else if programs.context.shared.zero_sectors.contains(&self.owner) {
                    self.emit(cell, OwnerDomainMatchDisposition::ExactZeroSector)
                } else {
                    self.push(
                        cell,
                        Phase::Terminals {
                            batch: 0,
                            after: None,
                        },
                    )
                }
            }
            Phase::Terminals { batch, after } => {
                let owner = &programs.owners[&self.owner];
                let Some(prepared) = owner.batches.get(batch) else {
                    return self.emit(cell, OwnerDomainMatchDisposition::ExactGap);
                };
                let terminal = match after {
                    None => prepared.terminals.iter().next(),
                    Some(key) => prepared
                        .terminals
                        .range((Bound::Excluded(key), Bound::Unbounded))
                        .next(),
                };
                let Some(key) = terminal else {
                    return self.push(
                        cell,
                        Phase::Rule {
                            batch,
                            index: 0,
                            stage: RuleStage::Fixed,
                        },
                    );
                };
                charge(
                    &mut self.budget.stats.terminal_checks,
                    1,
                    self.budget.limits.max_terminal_checks,
                    "terminal checks",
                )?;
                let next = Phase::Terminals {
                    batch,
                    after: Some(key),
                };
                let fixed = key.powers().iter().enumerate().map(|(i, &n)| {
                    (
                        i,
                        if self.owner[i] {
                            (n - 1) as u64
                        } else {
                            n.unsigned_abs()
                        },
                    )
                });
                let cut = self.budget.restrict::<N>(&cell, fixed)?;
                self.cut(
                    cell,
                    cut,
                    Phase::Emit(OwnerDomainMatchDisposition::Terminal { batch }),
                    next,
                )
            }
            Phase::Rule {
                batch,
                index,
                stage,
            } => {
                let prepared = &programs.owners[&self.owner].batches[batch];
                let Some(rule) = prepared.rules.get(index) else {
                    return self.push(
                        cell,
                        Phase::Terminals {
                            batch: batch + 1,
                            after: None,
                        },
                    );
                };
                let next = Phase::Rule {
                    batch,
                    index: index + 1,
                    stage: RuleStage::Fixed,
                };
                let at = |stage| Phase::Rule {
                    batch,
                    index,
                    stage,
                };
                let resume = PredicateResume::Rule {
                    batch,
                    index,
                    stage,
                };
                match stage {
                    RuleStage::Fixed => {
                        charge(
                            &mut self.budget.stats.rules,
                            1,
                            self.budget.limits.max_rules,
                            "rules",
                        )?;
                        let fixed = rule.fixed.iter().enumerate().filter_map(|(i, n)| {
                            n.map(|n| {
                                (
                                    i,
                                    if self.owner[i] {
                                        (i64::from(n) - 1) as u64
                                    } else {
                                        i64::from(n).unsigned_abs()
                                    },
                                )
                            })
                        });
                        let cut = self.budget.restrict::<N>(&cell, fixed)?;
                        self.cut(cell, cut, at(RuleStage::Equality(0)), next)
                    }
                    RuleStage::Equality(ordinal) => {
                        if let Some(p) = rule.equalities.get(ordinal) {
                            self.predicate(
                                cell,
                                p,
                                OwnerDomainPredicate::Equality {
                                    batch,
                                    rule: rule.ordinal,
                                    ordinal,
                                },
                                at(RuleStage::Equality(ordinal + 1)),
                                next,
                                resume,
                            )
                        } else {
                            self.push(cell, at(RuleStage::Exception(0, 0)))
                        }
                    }
                    RuleStage::Exception(branch, ordinal) => {
                        let Some(conjunction) = rule.exceptions.get(branch) else {
                            return self.push(cell, at(RuleStage::Denominator(0)));
                        };
                        if let Some(p) = conjunction.get(ordinal) {
                            self.predicate(
                                cell,
                                p,
                                OwnerDomainPredicate::ExcludedConjunction {
                                    batch,
                                    rule: rule.ordinal,
                                    branch,
                                    ordinal,
                                },
                                at(RuleStage::Exception(branch, ordinal + 1)),
                                at(RuleStage::Exception(branch + 1, 0)),
                                resume,
                            )
                        } else {
                            self.push(cell, next)
                        } // the WHOLE AND vanished
                    }
                    RuleStage::Denominator(term) => {
                        if let Some(rhs) = rule.rhs.get(term) {
                            self.predicate(
                                cell,
                                &rhs.denominator,
                                OwnerDomainPredicate::OriginalDenominator {
                                    batch,
                                    rule: rule.ordinal,
                                    term,
                                },
                                next,
                                at(RuleStage::Denominator(term + 1)),
                                resume,
                            )
                        } else {
                            self.emit(
                                cell,
                                OwnerDomainMatchDisposition::SelectedRule {
                                    batch,
                                    rule: rule.ordinal,
                                },
                            )
                        }
                    }
                }
            }
        }
    }
    /// The cut is already an intersection, so BoxCover yields its exact
    /// complement in this cell. Insertion order is deterministic, not a claim
    /// that callbacks are globally sorted by endpoints or dispatch ordinal.
    fn cut(
        &mut self,
        cell: LatticeBox,
        cut: Option<LatticeBox>,
        inside: Phase<'a>,
        outside: Phase<'a>,
    ) -> Result<(), OwnerDomainMatchFailure> {
        let Some(cut) = cut else {
            return self.push(cell, outside);
        };
        if cell == cut {
            return self.push(cell, inside);
        }
        for remainder in self.budget.subtract::<N>(cell, &cut)?.into_iter().rev() {
            self.push(remainder, outside)?;
        }
        self.push(cut, inside)
    }
    fn predicate(
        &mut self,
        cell: LatticeBox,
        p: &IndexedPolynomial,
        identity: OwnerDomainPredicate,
        zero: Phase<'a>,
        nonzero: Phase<'a>,
        resume: PredicateResume,
    ) -> Result<(), OwnerDomainMatchFailure> {
        self.cancelled()?;
        let resolution = guards::resolve(
            &self.programs.context.shared.context,
            p,
            &cell,
            &self.owner,
            self.rank,
            self.programs.context.limits.indexed_algebra,
            &mut self.budget,
        )?;
        self.cancelled()?;
        match resolution {
            Resolution::Zero => self.push(cell, zero),
            Resolution::Nonzero => self.push(cell, nonzero),
            Resolution::Unknown => self.refine_or_unresolved(cell, p, identity, resume),
            Resolution::Planes { roots, exact } => {
                let mut remaining = vec![cell];
                // Subtract each root from the residual BEFORE processing the
                // next root. Overlapping hyperplanes cannot duplicate output.
                for (axis, local) in roots {
                    let mut next = Vec::new();
                    for cell in remaining {
                        self.cancelled()?;
                        match self.budget.restrict::<N>(&cell, [(axis, local)])? {
                            None => {
                                next.try_reserve(1).map_err(|_| {
                                    OwnerDomainMatchFailure::AllocationFailure {
                                        resource: "guard residual cells",
                                    }
                                })?;
                                next.push(cell);
                            }
                            Some(cut) if cut == cell => {
                                if exact {
                                    self.push(cell, zero)?;
                                } else {
                                    self.refine_or_unresolved(cell, p, identity, resume)?;
                                }
                            }
                            Some(cut) => {
                                let residual = self.budget.subtract::<N>(cell, &cut)?;
                                next.try_reserve(residual.len()).map_err(|_| {
                                    OwnerDomainMatchFailure::AllocationFailure {
                                        resource: "guard residual cells",
                                    }
                                })?;
                                next.extend(residual);
                                if exact {
                                    self.push(cut, zero)?;
                                } else {
                                    self.refine_or_unresolved(cut, p, identity, resume)?;
                                }
                            }
                        }
                    }
                    remaining = next;
                }
                for cell in remaining.into_iter().rev() {
                    self.push(cell, nonzero)?;
                }
                Ok(())
            }
        }
    }

    fn refine_or_unresolved(
        &mut self,
        cell: LatticeBox,
        polynomial: &IndexedPolynomial,
        identity: OwnerDomainPredicate,
        resume: PredicateResume,
    ) -> Result<(), OwnerDomainMatchFailure> {
        self.cancelled()?;
        let unresolved = Phase::Emit(OwnerDomainMatchDisposition::Unresolved {
            predicate: identity,
        });
        // Conservative hyperplane subtraction can create rank-empty rectangle
        // intersections before they reach push(). Discard them through the same
        // exact simplex gate before computing any residual-rank subtraction.
        let min_rank = minimum_rank(&cell, &self.owner);
        if self.rank.is_some_and(|rank| min_rank > u128::from(rank)) {
            return self.push(cell, unresolved);
        }
        let remaining = self
            .budget
            .limits
            .max_bounded_refinement_cells
            .saturating_sub(self.budget.stats.refinement_cells);
        if remaining == 0 {
            return self.push(cell, unresolved);
        }

        // Pinned Symbolica's public `contains(variable)` is an allocation-free
        // exponent-support query. IndexedPolynomial authenticates the variable
        // map: base parameters precede original physical index coordinates.
        // Original support is a conservative choice after specialization; it
        // can waste refinement but cannot exclude a required predicate axis.
        let base = self
            .programs
            .context
            .coefficient_context()
            .base()
            .variables()
            .len();
        let mut best: Option<(u128, usize, u64)> = None;
        for axis in 0..N {
            if self.owner[axis]
                || cell.upper()[axis] == Some(cell.lower()[axis])
                || !polynomial.raw().contains(base + axis)
            {
                continue;
            }
            let rank_upper = self.rank.map(|rank| {
                let others = min_rank - u128::from(cell.lower()[axis]);
                // Every queued cell already passes the exact minimum-rank test.
                (u128::from(rank) - others) as u64
            });
            let upper = match (cell.upper()[axis], rank_upper) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => continue,
            };
            let width = u128::from(upper) - u128::from(cell.lower()[axis]) + 1;
            let candidate = (width, axis, upper);
            if best.is_none_or(|old| candidate < old) {
                best = Some(candidate);
            }
        }
        let Some((width, axis, upper)) = best else {
            return self.push(cell, unresolved);
        };
        let Ok(width) = usize::try_from(width) else {
            return self.push(cell, unresolved);
        };
        if width > remaining {
            return self.push(cell, unresolved);
        }

        // This refinement is optional: reserve its entire geometry and counter
        // envelope transactionally before any singleton face can be published.
        // Failure to fit keeps the ORIGINAL unresolved cell, not a partial cover.
        let mut prospective = Budget {
            limits: self.budget.limits,
            stats: self.budget.stats,
        };
        if prospective.cells::<N>(width).is_err() {
            return self.push(cell, unresolved);
        }
        charge(
            &mut prospective.stats.refinement_cells,
            width,
            prospective.limits.max_bounded_refinement_cells,
            "bounded refinement cells",
        )?;
        charge(
            &mut prospective.stats.refinement_steps,
            1,
            usize::MAX,
            "bounded refinement steps",
        )?;
        self.cancelled()?;
        self.budget = prospective;
        let next = cell.lower()[axis];
        self.push(
            cell,
            Phase::Refinement {
                axis,
                next,
                upper,
                resume,
            },
        )
    }
}
