//! Opt-in G-split ([`super::CaseRefinement::graph_split`]): eliminate
//! coordinates that occur linearly with an affine-factored coefficient.
//!
//! For an equation `c(y) x + r(y)` of the current branch (`x` a free
//! coordinate, every factor `c_f` of `c` affine), every integer point has
//! either `c_f = 0` for some new factor, or an excluded factor zero (owned by
//! an earlier sibling), or `c != 0` and `x = -r/c`. The first kind become
//! ordinary children `{c_f = 0} AND branch`; the last is the remainder, which
//! keeps the defining equation and replaces every other equation `g` by the
//! denominator-free `c^deg(g) g(x = -r/c)` with every exclusion divided out.
//! On the remainder (every exclusion nonzero) both systems have the same
//! points, so the OR of children and remainder is the branch exactly.

use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::algebra::CoefficientPolynomial;

use super::super::super::Case;
use super::super::{CaseIntersectionBudget, CaseIntersectionFailure, GraphDependent, native};
use super::{Engine, WorkItem, checkpoint, spend};

/// G-split rounds that may replace equations along one path; afterwards a
/// branch only gains children (new factors) or is settled as it is.
const MAX_GRAPH_ROUNDS: u8 = 6;

pub(super) enum GraphOutcome {
    /// Children and/or a reduced remainder were queued; the current state is
    /// consumed.
    Pushed,
    /// Nothing new; the dependents describe the current branch as a graph
    /// (empty when no coordinate qualifies).
    Unchanged(Vec<GraphDependent>),
}

struct Elimination {
    dependents: Vec<GraphDependent>,
    defining: Vec<CoefficientPolynomial>,
    new_factors: Vec<CoefficientPolynomial>,
    base: Vec<CoefficientPolynomial>,
}

fn total_degree(polynomial: &CoefficientPolynomial) -> u32 {
    polynomial
        .exponents_iter()
        .map(|exponents| exponents.iter().map(|&power| u32::from(power)).sum())
        .max()
        .unwrap_or(0)
}

impl<const N: usize> Engine<'_, N> {
    /// `at_stuck`: called on a branch that refinement, S1 and S2 left
    /// unresolved. There only NEW coefficient factors count as progress (the
    /// remainder keeps its equations); before normalization a reduced
    /// remainder also counts, at most [`MAX_GRAPH_ROUNDS`] times per path.
    pub(super) fn graph_split(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
        at_stuck: bool,
    ) -> Result<GraphOutcome, CaseIntersectionFailure> {
        let Some(elimination) = self.eliminate()? else {
            return Ok(GraphOutcome::Unchanged(Vec::new()));
        };
        let mut remainder = elimination.defining.clone();
        remainder.extend(elimination.base.iter().cloned());
        native::canonicalize(&mut remainder);
        let replaces = remainder.as_slice() != self.current.equations.as_ref();
        let may_replace = !at_stuck && replaces && self.current.graph_rounds < MAX_GRAPH_ROUNDS;
        if elimination.new_factors.is_empty() && !may_replace {
            return Ok(GraphOutcome::Unchanged(elimination.dependents));
        }
        let children = elimination.new_factors.len() + 1;
        if self.limits.diagnostics_enabled
            && self
                .stats
                .work_items
                .checked_add(pending.len())
                .and_then(|count| count.checked_add(children))
                .is_none_or(|count| count > self.limits.max_work_items)
        {
            return Err(CaseIntersectionFailure::Budget {
                kind: CaseIntersectionBudget::WorkItems,
                limit: self.limits.max_work_items,
            });
        }
        let mut exclusions = self.current.exclusions.to_vec();
        let mut queued = Vec::with_capacity(children);
        for factor in &elimination.new_factors {
            let mut equations = self.current.equations.to_vec();
            equations.push(factor.clone());
            native::canonicalize(&mut equations);
            let mut owned = exclusions.clone();
            native::canonicalize(&mut owned);
            queued.push(WorkItem {
                parent: self.current.parent.clone(),
                equations: equations.into(),
                ancestry: self.current.ancestry.clone(),
                exclusions: owned.into(),
                graph_rounds: self.current.graph_rounds,
            });
            exclusions.push(factor.clone());
        }
        native::canonicalize(&mut exclusions);
        let (equations, graph_rounds): (Arc<[CoefficientPolynomial]>, u8) = if may_replace {
            self.check_original = true;
            (
                remainder.into(),
                self.current.graph_rounds.saturating_add(1),
            )
        } else {
            (self.current.equations.clone(), self.current.graph_rounds)
        };
        // The remainder is processed after the coefficient-zero children.
        pending.push(WorkItem {
            parent: self.current.parent.clone(),
            equations,
            ancestry: self.current.ancestry.clone(),
            exclusions: exclusions.into(),
            graph_rounds,
        });
        self.refinement_stats.graph_children = self
            .refinement_stats
            .graph_children
            .saturating_add(elimination.new_factors.len());
        self.refinement_stats.graph_splits = self.refinement_stats.graph_splits.saturating_add(1);
        pending.extend(queued.into_iter().rev());
        Ok(GraphOutcome::Pushed)
    }

    /// Greedy triangular elimination of linear coordinates (deterministic:
    /// lowest total degree and fewest terms of the defining equation, then
    /// fewest coefficient terms, then the lowest axis; the first candidate
    /// whose coefficient has only affine factors is taken).
    fn eliminate(&mut self) -> Result<Option<Elimination>, CaseIntersectionFailure> {
        let mut rest = self.current.equations.to_vec();
        let mut strip = self.current.exclusions.to_vec();
        let mut elimination = Elimination {
            dependents: Vec::new(),
            defining: Vec::new(),
            new_factors: Vec::new(),
            base: Vec::new(),
        };
        loop {
            checkpoint()?;
            if rest.iter().any(CoefficientPolynomial::is_constant) {
                break;
            }
            let mut candidates = Vec::new();
            for (position, equation) in rest.iter().enumerate() {
                for axis in 0..N {
                    let variable = self.indices[axis];
                    if equation.degree(variable) != 1 {
                        continue;
                    }
                    let mut coefficient = equation.zero();
                    let mut remainder = equation.zero();
                    for (part, power) in equation.to_univariate_polynomial_list(variable) {
                        if power == 1 {
                            coefficient = part;
                        } else {
                            remainder = part;
                        }
                    }
                    if coefficient.is_zero() {
                        continue;
                    }
                    let key = (
                        total_degree(equation),
                        equation.nterms(),
                        coefficient.nterms(),
                        axis,
                    );
                    candidates.push((key, position, axis, coefficient, remainder));
                }
            }
            candidates.sort_by_key(|candidate| candidate.0);
            let mut chosen = None;
            for (_, position, axis, coefficient, remainder) in candidates {
                let factors = if coefficient.is_constant() {
                    Vec::new()
                } else {
                    spend(
                        &mut self.stats.factorizations,
                        self.limits.diagnostic_limit(self.limits.max_factorizations),
                        CaseIntersectionBudget::Factorizations,
                    )?;
                    native::factors(&coefficient)?
                };
                if factors.iter().all(native::is_affine) {
                    chosen = Some((position, axis, coefficient, remainder, factors));
                    break;
                }
            }
            let Some((position, axis, denominator, remainder, factors)) = chosen else {
                break;
            };
            for factor in factors {
                if !strip.contains(&factor) {
                    elimination.new_factors.push(factor.clone());
                    strip.push(factor);
                }
            }
            let numerator = -remainder;
            let variable = self.indices[axis];
            let defining = rest.remove(position);
            let mut substituted = Vec::with_capacity(rest.len());
            for equation in rest {
                checkpoint()?;
                let equation = native::strip_factors(
                    native::substitute_quotient(&equation, variable, &numerator, &denominator),
                    &strip,
                );
                if !equation.is_zero() {
                    substituted.push(equation);
                }
            }
            native::canonicalize(&mut substituted);
            self.check_terms(&substituted)?;
            rest = substituted;
            elimination.defining.push(defining);
            elimination.dependents.push(GraphDependent {
                axis,
                numerator,
                denominator,
            });
        }
        if elimination.dependents.is_empty() {
            return Ok(None);
        }
        elimination.base = rest;
        Ok(Some(elimination))
    }
}

/// Opt-in refinement: an exact proof that the integer points of `chart` miss
/// the sector (active coordinates >= 1, inactive <= 0). Gauss-Jordan
/// elimination of the chart's primitive integer rows in REVERSE axis order;
/// every row produced is an exact consequence of the chart equalities, and a
/// row whose left-hand side is bounded away from its right-hand side on the
/// sector proves emptiness. `false` proves nothing.
pub(super) fn chart_misses_sector<const N: usize>(chart: &Case<N>, sector: &[bool; N]) -> bool {
    let Some(affine) = chart.affine() else {
        return false;
    };
    let fixed = chart.fixed();
    let mut rows: Vec<Vec<Integer>> = affine
        .primitive_matrix()
        .row_iter()
        .map(<[Integer]>::to_vec)
        .collect();
    if rows.iter().any(|row| row.len() != N + 1) {
        return false;
    }
    if rows.iter().any(|row| row_misses_sector(row, fixed, sector)) {
        return true;
    }
    let mut used = vec![false; rows.len()];
    for column in (0..N).rev() {
        if fixed[column].is_some() {
            continue;
        }
        let Some(pivot) = (0..rows.len()).find(|&row| !used[row] && !rows[row][column].is_zero())
        else {
            continue;
        };
        used[pivot] = true;
        let pivot_row = rows[pivot].clone();
        for (index, row) in rows.iter_mut().enumerate() {
            if index == pivot || row[column].is_zero() {
                continue;
            }
            let factor = row[column].clone();
            let scale = pivot_row[column].clone();
            let combined: Vec<Integer> = row
                .iter()
                .zip(&pivot_row)
                .map(|(value, pivot_value)| &(value * &scale) - &(pivot_value * &factor))
                .collect();
            if row_misses_sector(&combined, fixed, sector) {
                return true;
            }
            *row = combined;
        }
    }
    false
}

/// `sum_i row[i] n_i = row[N]` has no solution with fixed coordinates at their
/// values, active free coordinates >= 1 and inactive free coordinates <= 0.
fn row_misses_sector<const N: usize>(
    row: &[Integer],
    fixed: &[Option<i16>; N],
    sector: &[bool; N],
) -> bool {
    let mut lower = Some(Integer::zero());
    let mut upper = Some(Integer::zero());
    for (axis, coefficient) in row[..N].iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        if let Some(value) = fixed[axis] {
            let contribution = coefficient * &Integer::from(value);
            if let Some(bound) = &mut lower {
                *bound += &contribution;
            }
            if let Some(bound) = &mut upper {
                *bound += &contribution;
            }
        } else if sector[axis] != coefficient.is_negative() {
            // Unbounded above; an active axis contributes coefficient * 1 below.
            upper = None;
            if sector[axis]
                && let Some(bound) = &mut lower
            {
                *bound += coefficient;
            }
        } else {
            lower = None;
            if sector[axis]
                && let Some(bound) = &mut upper
            {
                *bound += coefficient;
            }
        }
    }
    lower.as_ref().is_some_and(|bound| row[N] < *bound)
        || upper.as_ref().is_some_and(|bound| row[N] > *bound)
}
