//! Regenerate a compact source trace and recover its exact normalized rule.
//!
//! Symbolica reduces an augmented [physical | source identity] matrix. The
//! identity columns retain direct source weights without an independent CAS
//! kernel or a transitive-combination arena in the search hot path.

use std::cmp::Ordering;

use symbolica::domains::rational_polynomial::{
    FromNumeratorAndDenominator, RationalPolynomialField,
};
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;
use crate::foundry::completion::LatticeBox;
use crate::solver::{
    Case, ExactRow, IntegralOrder, PolynomialRow, RuleCandidate, SectorRule, SourceSystem, Term,
    canonicalize_source_port, extract_exceptions, instantiate_source_port,
};

use super::{SourcePortAuditError, error, geometry, ordinary};

#[cfg(test)]
mod tests;

pub(super) struct Replay<const N: usize> {
    pub additional_exceptions: Vec<Case<N>>,
    /// Retain with the rule and exact domain at the next installation boundary.
    pub ordinary: super::certificate::OriginalSourceReplay<N>,
}

pub(super) fn replay_rule<const N: usize>(
    system: &SourceSystem<N>,
    original_row_ids: &[crate::identity::RowId],
    original_sources: &super::normalization::OriginalSourceCorpus,
    basis: &[PolynomialRow<N>],
    order: &IntegralOrder<N>,
    zero_sectors: &[[bool; N]],
    rule: &SectorRule<N>,
    boxes: &[LatticeBox],
) -> Result<Replay<N>, SourcePortAuditError> {
    let candidate = &rule.candidate;
    if candidate.target != candidate.case.integral() || !candidate.case.is_in_sector(order.sector())
    {
        return Err(error(
            "cold bridge requires a canonical target in its declared sector",
        ));
    }
    let template = system
        .rows()
        .iter()
        .flatten()
        .next()
        .ok_or_else(|| error("regenerated source corpus has no coefficient map"))?
        .coefficient
        .clone();
    if candidate.rhs.iter().any(|term| {
        term.coefficient.numerator.variables() != template.variables()
            || term.coefficient.denominator.variables() != template.variables()
    }) {
        return Err(error(
            "candidate coefficient map differs from regenerated sources",
        ));
    }
    let affine = candidate.case.affine();
    if let Some(affine) = affine {
        if affine.index_variables() != system.index_variables() {
            return Err(error("affine replay uses a different index-variable map"));
        }
        // This validates the chart/map used by the trusted instantiator below.
        // Reject poles on the WHOLE case before any rational cancellation.
        affine.restrict_polynomial_value(&template).map_err(error)?;
        for term in &candidate.rhs {
            affine
                .restrict_coefficient(&term.coefficient)
                .map_err(error)?;
        }
    }
    // Affine discovery deliberately retained the full physical source span;
    // replay must use the same row projection.  The rectangular zero-sector
    // census cannot infer signs on coupled charts and pruning here would make
    // the authenticated source trace differ from the discovered one.
    let replay_zero_sectors: &[[bool; N]] = if affine.is_some() { &[] } else { zero_sectors };
    let mut rows = Vec::with_capacity(candidate.sources.len());
    for source in &candidate.sources {
        // Seed fields are public search transport, not certificate authority.
        // For a canonical target, the symbolic base is exactly n;
        // coefficient and physical-index translations must therefore agree.
        for axis in 0..N {
            let seed_power = source.seed.integral[axis];
            if seed_power.is_symbolic() != candidate.target[axis].is_symbolic() {
                return Err(error(format!(
                    "source seed symbolic pattern differs from the target case at axis {axis}"
                )));
            }
            let coefficient_shift = source.seed.shifts[axis];
            if (seed_power.is_symbolic() && seed_power.value() != coefficient_shift)
                || (!seed_power.is_symbolic() && coefficient_shift != 0)
            {
                return Err(error(format!(
                    "source seed coefficient translation differs from its physical integral at axis {axis}"
                )));
            }
        }
        let row = basis
            .get(source.basis_row)
            .ok_or_else(|| error("selected preconditioned source ordinal is out of range"))?;
        rows.push(
            instantiate_source_port(
                row,
                &source.seed,
                system.index_variables(),
                system.fixed(),
                order,
                replay_zero_sectors,
                affine,
            )
            .map_err(error)?,
        );
    }
    if rows.is_empty() {
        return Err(error("candidate has no retained source provenance"));
    }
    let mut columns: Vec<_> = rows.iter().flatten().map(|term| term.integral).collect();
    columns.sort_unstable_by(|left, right| order.compare(left, right));
    columns.dedup();
    if rows.iter().any(|row| {
        row.windows(2)
            .any(|terms| order.compare(&terms[0].integral, &terms[1].integral) != Ordering::Less)
    }) {
        return Err(error("regenerated source trace is not canonical"));
    }
    let native_columns = columns
        .len()
        .checked_add(rows.len())
        .and_then(|n| n.checked_add(1))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| error("exact replay column count overflow"))?;
    let field = RationalPolynomialField::<IntegerRing, u16>::new(Z);
    let mut reducer = SparseRowReducer::new(native_columns, field, LuLMode::None);
    let one: Coefficient = template.one().into();
    for (source_ordinal, row) in rows.iter().enumerate() {
        let mut values: Vec<_> = row.iter().map(|term| term.coefficient.clone()).collect();
        let mut indices: Vec<_> = row
            .iter()
            .map(|term| {
                columns
                    .binary_search_by(|column| order.compare(column, &term.integral))
                    .expect("the physical column union contains its own source term")
                    as u32
            })
            .collect();
        values.push(one.clone());
        indices.push((columns.len() + source_ordinal) as u32);
        let Some(pivot) = reducer.add_row(&values, &indices) else {
            continue;
        };
        let Some(raw_target) = columns.get(pivot as usize) else {
            continue;
        };
        if !candidate.case.matches(raw_target) {
            continue;
        }
        let u = reducer.u();
        let row_index = u.nrows() as usize - 1;
        let start = u.row_ptrs()[row_index];
        let end = u.row_ptrs()[row_index + 1];
        let entries = u.col_idcs()[start..end].iter().zip(&u.values()[start..end]);
        let mut physical: ExactRow<N> = Vec::new();
        let mut weights = Vec::new();
        for (&column, coefficient) in entries {
            if let Some(integral) = columns.get(column as usize) {
                physical.push(Term {
                    integral: *integral,
                    coefficient: coefficient.clone(),
                });
            } else {
                weights.push((column as usize - columns.len(), coefficient.clone()));
            }
        }
        let (target, rhs) =
            canonicalize_source_port(physical.clone(), system.index_variables()).map_err(error)?;
        if target != candidate.target || rhs != candidate.rhs {
            continue;
        }

        // Independently multiply the retained original trace by the recovered
        // weights; native reduction success alone is not the equality check.
        let zero: Coefficient = template.zero().into();
        let mut check = vec![zero; columns.len()];
        for (row_ordinal, weight) in &weights {
            let source = rows
                .get(*row_ordinal)
                .ok_or_else(|| error("native source identity column is out of range"))?;
            for term in source {
                let column = columns
                    .binary_search_by(|key| order.compare(key, &term.integral))
                    .expect("replayed source term belongs to the physical union");
                check[column] = &check[column] + &(weight * &term.coefficient);
            }
        }
        for term in &physical {
            let column = columns
                .binary_search_by(|key| order.compare(key, &term.integral))
                .expect("native U term belongs to the physical union");
            check[column] = &check[column] - &term.coefficient;
        }
        if check.iter().any(|coefficient| !coefficient.is_zero()) {
            return Err(error(
                "exact native source-weight replay left a nonzero physical coefficient",
            ));
        }
        let shifts: [i16; N] = std::array::from_fn(|axis| {
            if raw_target[axis].is_symbolic() {
                -raw_target[axis].value()
            } else {
                0
            }
        });
        // The regenerated polynomial preconditioner itself introduces no
        // rational divisors. Nevertheless this diagnostic also obtains an
        // independent certificate directly from the original ordinary rows,
        // without trusting the search's zero-sector projection.
        let existing = rule
            .exceptional_cases(system.index_variables(), order.sector())
            .map_err(error)?;
        let mut additional_exceptions = Vec::new();
        // Do not authenticate a caller's serialized exclusions merely by
        // interpreting them: recompute RHS poles and activation conditions.
        let rhs_exceptions =
            extract_exceptions(candidate, system.index_variables(), order.sector())
                .map_err(error)?;
        append_exceptions(
            candidate,
            rhs_exceptions,
            &existing,
            &mut additional_exceptions,
            system.index_variables(),
            order.sector(),
        )?;
        let tightened;
        // An affine target carries an exact chart, but its coordinate-face
        // box is still a sound conservative prefilter for replay: proving a
        // discarded physical column zero on that larger box also proves it
        // on the affine subset.  The box is not used as an ownership or
        // publication certificate; the affine bridge remains fail-closed.
        let applicable = if additional_exceptions.is_empty() {
            boxes
        } else {
            tightened = geometry::application_partition(
                rule,
                system.index_variables(),
                order.sector(),
                &additional_exceptions,
            )?;
            &tightened.boxes
        };
        let ordinary = ordinary::weights(
            system,
            original_row_ids,
            order,
            zero_sectors,
            rule,
            shifts,
            applicable,
        )?;
        let ordinary = original_sources.normalize(ordinary, &candidate.case)?;
        let mut normalized_weights: Vec<_> = ordinary
            .contributions
            .iter()
            .map(|contribution| Term {
                integral: candidate.target,
                coefficient: contribution.weight.clone(),
            })
            .collect();
        // Index-dependent original-source poles take the same exact geometry
        // path as converted-weight poles. Parameter-only conditions remain
        // attached to the retained record even when that extractor omits them.
        normalized_weights.extend(ordinary.source_conditions.iter().map(|condition| Term {
            integral: candidate.target,
            coefficient: Coefficient::from_num_den(condition.one(), condition.clone(), &Z, false),
        }));
        let weight_candidate = RuleCandidate {
            case: candidate.case.clone(),
            target: candidate.target,
            rhs: normalized_weights,
            sources: Vec::new(),
            stats: Default::default(),
        };
        let exceptions =
            extract_exceptions(&weight_candidate, system.index_variables(), order.sector())
                .map_err(error)?;
        append_exceptions(
            candidate,
            exceptions,
            &existing,
            &mut additional_exceptions,
            system.index_variables(),
            order.sector(),
        )?;
        return Ok(Replay {
            additional_exceptions,
            ordinary,
        });
    }
    Err(error(
        "regenerated selected source frame has no pivot reproducing the canonical candidate",
    ))
}

fn append_exceptions<const N: usize>(
    candidate: &RuleCandidate<N>,
    exceptions: crate::solver::ExceptionalConditions,
    existing: &[Case<N>],
    additional: &mut Vec<Case<N>>,
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<(), SourcePortAuditError> {
    for branch in exceptions.branches {
        let Some(case) = candidate
            .case
            .intersect(&branch, indices, sector)
            .map_err(error)?
        else {
            continue;
        };
        let mut already_excluded = false;
        for excluded in existing.iter().chain(additional.iter()) {
            if excluded.contains(&case).map_err(error)? {
                already_excluded = true;
                break;
            }
        }
        if !already_excluded {
            additional.push(case);
        }
    }
    Ok(())
}
