//! Independent re-derivation of a Laporta solution from the original identities.
//!
//! The search combines preconditioned identities, prunes zero sectors at
//! instantiation, back-substitutes, and may change to preferred masters. The
//! certificate repeats none of that machinery:
//!
//! 1. It recomputes the zero census, so every zero rule must lie in a proved
//!    zero sector or outside the cut.
//! 2. It regenerates the family's original IBP (and, if used, Lorentz)
//!    identities, instantiates them at each derived rule's recorded seeds,
//!    drops integrals of zero sectors, and requires the rule to lie in their
//!    exact span over the coefficient field.
//! 3. Every derived rule must strictly descend in the search order.
//! 4. Every returned rule, with preferred masters mapped back to their
//!    original reductions, must reduce to zero through the derived rules.
//! 5. Every requested integral must have a rule or be a residual.
//!
//! This verifies the reduction; it says nothing about whether the residuals
//! are a minimal or independent master basis.

use std::collections::{BTreeMap, BTreeSet};

use symbolica::domains::{
    integer::{Integer, Z},
    rational_polynomial::{FromNumeratorAndDenominator, RationalPolynomialField},
};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;
use crate::identity::{ParametricIbpGenerator, ParametricRelation};
use crate::sector::CutConstraint;
use crate::solver::SolverError;

use super::combination::{Combination, NumericOrder, add, sector_of, unit, values};
use super::{
    DynamicRule, DynamicSolution, RuleOrigin, array, cut_restrictions, dispatch, is_excluded,
    zero_census,
};

/// What a successful certificate checked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductionCertificate {
    /// Derived rules re-derived from the original identities.
    pub replayed_rules: usize,
    /// Zero rules justified by a zero-sector proof or the cut.
    pub zero_rules: usize,
    /// Instantiated original identities used by the replay.
    pub identities: usize,
    /// Returned rules shown to follow from the derived rules.
    pub returned_rules: usize,
}

/// Check a [`super::solve_laporta`] solution; see the module documentation.
/// Any failure is [`SolverError::ExactReplay`].
pub fn certify_laporta(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    dispatch!(
        family.denominator_count(),
        certify,
        family,
        cuts,
        solution,
        include_lorentz
    )
}

fn fail(message: String) -> SolverError {
    SolverError::ExactReplay(format!("certification failed: {message}"))
}

fn certify<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    if solution.requested.is_empty() && !solution.rules.is_empty() {
        return Err(fail("the solution records no requested integrals".into()));
    }
    let restrictions = cut_restrictions(family, cuts)?;
    let order = NumericOrder::<N>::new();
    let compare = |left: &[i16], right: &[i16]| order.integrals(left, right);
    let mut requested_sectors = Vec::new();
    for integral in &solution.requested {
        let sector = sector_of(&array::<_, N>(integral, "requested integral")?);
        if !is_excluded(&restrictions, sector)? {
            requested_sectors.push(sector);
        }
    }
    let (zero_sectors, _) = zero_census(family, &restrictions, &requested_sectors)?;
    let is_zero = |powers: &[i16]| zero_sectors.contains(&sector_of::<N>(powers));

    let mut certificate = ReductionCertificate::default();
    let mut derived = BTreeMap::<Vec<i16>, &DynamicRule>::new();
    let mut by_seeds = BTreeMap::<&[Vec<i16>], Vec<&DynamicRule>>::new();
    for derivation in &solution.derivation {
        let rule = &derivation.rule;
        let target = values(&rule.target);
        if derived.insert(target.clone(), rule).is_some() {
            return Err(fail(format!("{target:?} is derived twice")));
        }
        match &derivation.origin {
            RuleOrigin::OutsideCut | RuleOrigin::ProvedZero => {
                let justified = match derivation.origin {
                    RuleOrigin::OutsideCut => is_excluded(&restrictions, sector_of::<N>(&target))?,
                    _ => is_zero(&target),
                };
                if !rule.rhs.is_empty() || !justified {
                    return Err(fail(format!(
                        "the zero rule for {target:?} is not justified"
                    )));
                }
                certificate.zero_rules += 1;
            }
            RuleOrigin::Identities { seeds } => {
                if seeds.is_empty() {
                    return Err(fail(format!("the rule for {target:?} records no seeds")));
                }
                for term in &rule.rhs {
                    let powers = values(&term.powers);
                    if compare(&target, &powers) != std::cmp::Ordering::Less || is_zero(&powers) {
                        return Err(fail(format!(
                            "the rule for {target:?} does not strictly descend to {powers:?}"
                        )));
                    }
                }
                by_seeds.entry(seeds.as_slice()).or_default().push(rule);
            }
        }
    }

    let (relations, offset) = original_relations(family, include_lorentz)?;
    let field = RationalPolynomialField::new(Z);
    for (seeds, rules) in by_seeds {
        let mut rows: Vec<Combination> = Vec::new();
        for seed in seeds {
            let seed = array::<_, N>(seed, "seed")?;
            for relation in &relations {
                if let Some(row) = instantiate(relation, &seed, offset, &is_zero)? {
                    rows.push(row);
                }
            }
        }
        certificate.identities += rows.len();
        let desired: Vec<(Vec<i16>, Combination)> = rules
            .iter()
            .map(|rule| {
                let target = values(&rule.target);
                let mut row = Combination::new();
                if let Some(term) = rule.rhs.first() {
                    add(&mut row, target.clone(), unit(&term.coefficient));
                } else {
                    return Err(fail(format!("the rule for {target:?} has no terms")));
                }
                for term in &rule.rhs {
                    add(&mut row, values(&term.powers), -term.coefficient.clone());
                }
                Ok((target, row))
            })
            .collect::<Result<_, _>>()?;
        let mut columns: Vec<&Vec<i16>> = rows
            .iter()
            .chain(desired.iter().map(|(_, row)| row))
            .flat_map(|row| row.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        columns.sort_by(|left, right| compare(left, right));
        let index: BTreeMap<&Vec<i16>, u32> = columns
            .iter()
            .enumerate()
            .map(|(position, key)| (*key, position as u32))
            .collect();
        let width = u32::try_from(columns.len()).map_err(|_| fail("too many columns".into()))?;
        let mut reducer = SparseRowReducer::new(width, field.clone(), LuLMode::None);
        let sparse = |row: &Combination| {
            let mut entries: Vec<(u32, Coefficient)> = row
                .iter()
                .map(|(key, coefficient)| (index[key], coefficient.clone()))
                .collect();
            entries.sort_by_key(|(column, _)| *column);
            entries.into_iter().unzip::<_, _, Vec<_>, Vec<_>>()
        };
        for row in &rows {
            let (indices, values) = sparse(row);
            reducer.add_row(&values, &indices);
        }
        // A desired row in the span adds no pivot, so earlier checks stay valid.
        for (target, row) in &desired {
            let (indices, values) = sparse(row);
            if reducer.add_row(&values, &indices).is_some() {
                return Err(fail(format!(
                    "the rule for {target:?} does not follow from the original identities at its seeds"
                )));
            }
            certificate.replayed_rules += 1;
        }
    }

    // Returned rules: map preferred masters back, then eliminate the hardest
    // derived target until only search residuals remain.
    let (back, search_residuals) = match &solution.basis_change {
        Some(change) => {
            let back: BTreeMap<Vec<i16>, &DynamicRule> = change
                .original
                .iter()
                .map(|rule| (values(&rule.target), rule))
                .collect();
            let mut residuals: BTreeSet<Vec<i16>> = solution
                .residuals
                .iter()
                .filter(|residual| !back.contains_key(*residual))
                .cloned()
                .collect();
            residuals.extend(change.replaced.iter().cloned());
            (back, residuals)
        }
        None => (
            BTreeMap::new(),
            solution.residuals.iter().cloned().collect(),
        ),
    };
    let mapped_rules = solution.rules.iter().chain(back.values().copied());
    for rule in mapped_rules {
        let target = values(&rule.target);
        let mut row = Combination::new();
        let unit = match rule.rhs.first() {
            Some(term) => unit(&term.coefficient),
            None => {
                // A zero rule must be derived as zero.
                match derived.get(&target) {
                    Some(origin) if origin.rhs.is_empty() => {
                        certificate.returned_rules += 1;
                        continue;
                    }
                    _ => return Err(fail(format!("the zero rule for {target:?} is not derived"))),
                }
            }
        };
        add(&mut row, target.clone(), unit);
        for term in &rule.rhs {
            let powers = values(&term.powers);
            // A replaced preferred master stands for its original reduction.
            match back.get(&powers) {
                Some(original) => {
                    for inner in &original.rhs {
                        add(
                            &mut row,
                            values(&inner.powers),
                            -(&term.coefficient * &inner.coefficient),
                        );
                    }
                }
                None => add(&mut row, powers, -term.coefficient.clone()),
            }
        }
        loop {
            let hardest = row
                .keys()
                .filter(|key| !search_residuals.contains(*key))
                .min_by(|left, right| compare(left, right))
                .cloned();
            let Some(key) = hardest else {
                break;
            };
            let Some(derivation) = derived.get(&key) else {
                return Err(fail(format!(
                    "{key:?} in the rule for {target:?} is neither derived nor a residual"
                )));
            };
            let factor = row.remove(&key).expect("the hardest key is present");
            for term in &derivation.rhs {
                add(&mut row, values(&term.powers), &factor * &term.coefficient);
            }
        }
        if !row.is_empty() {
            return Err(fail(format!(
                "the rule for {target:?} does not follow from the derived rules"
            )));
        }
        certificate.returned_rules += 1;
    }

    let solved: BTreeSet<Vec<i16>> = solution
        .rules
        .iter()
        .map(|rule| values(&rule.target))
        .collect();
    let residuals: BTreeSet<&Vec<i16>> = solution.residuals.iter().collect();
    for integral in &solution.requested {
        if !solved.contains(integral) && !residuals.contains(integral) {
            return Err(fail(format!(
                "{integral:?} has neither a rule nor a residual"
            )));
        }
    }
    Ok(certificate)
}

/// The family's original identities, before any preconditioning, and the
/// position of the first index variable in their coefficients.
fn original_relations(
    family: &IntegralFamily,
    include_lorentz: bool,
) -> Result<(Vec<ParametricRelation>, usize), SolverError> {
    let invalid = |error: &dyn std::fmt::Display| SolverError::InvalidInput(error.to_string());
    let generator = ParametricIbpGenerator::try_new(family).map_err(|error| invalid(&error))?;
    let offset = generator.context().base().parameter_names().len();
    let batch = generator
        .prepare_ordinary_ibp()
        .map_err(|error| invalid(&error))?;
    let generated = (0..batch.len())
        .map(|ordinal| batch.generate(ordinal))
        .collect();
    let completed = batch.complete(generated).map_err(|error| invalid(&error))?;
    let lorentz = if include_lorentz {
        let batch = generator
            .prepare_lorentz_invariance(&completed)
            .map_err(|error| invalid(&error))?;
        (0..batch.len())
            .map(|ordinal| batch.generate(ordinal))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| invalid(&error))?
    } else {
        Vec::new()
    };
    let mut relations = completed.into_relations();
    relations.extend(lorentz);
    Ok((relations, offset))
}

/// Instantiate one relation at a concrete seed, dropping integrals of zero
/// sectors. A relation whose coefficient is singular at the seed is unusable.
fn instantiate<const N: usize>(
    relation: &ParametricRelation,
    seed: &[i16; N],
    offset: usize,
    is_zero: &impl Fn(&[i16]) -> bool,
) -> Result<Option<Combination>, SolverError> {
    let mut row = Combination::new();
    for (shift, coefficient) in relation.terms() {
        let powers = seed
            .iter()
            .zip(shift.values())
            .map(|(&seed, &shift)| i16::try_from(i64::from(seed) + shift))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| fail("an instantiated power overflows".into()))?;
        if is_zero(&powers) {
            continue;
        }
        let raw = coefficient.raw();
        let (mut numerator, mut denominator) = (raw.numerator.clone(), raw.denominator.clone());
        for (axis, &value) in seed.iter().enumerate() {
            let value = Integer::from(i64::from(value));
            numerator = numerator.replace(offset + axis, &value);
            denominator = denominator.replace(offset + axis, &value);
        }
        if denominator.is_zero() {
            return Ok(None);
        }
        add(
            &mut row,
            powers,
            Coefficient::from_num_den(numerator, denominator, &Z, true),
        );
    }
    Ok(Some(row))
}
