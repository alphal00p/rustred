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
//! 4. Every returned rule must use only residuals, and, with preferred masters
//!    mapped back to their original reductions, reduce to zero through the
//!    derived rules.
//! 5. Every requested integral must have a rule or be a residual.
//!
//! This verifies the reduction. It does not check that the nonzero conditions
//! cover every pole, and it says nothing about whether the residuals are a
//! minimal or independent master basis.

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
use crate::solver::{Integral, SolverError};

use super::combination::{Combination, NumericOrder, add, sector_of, unit, values};
use super::{
    DynamicRule, DynamicSolution, RuleOrigin, array, certify_laporta_with_capacity,
    cut_restrictions, is_excluded, unsupported_runtime_arity, validate_family_arity, zero_census,
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
/// A failed check is [`SolverError::Certification`]; a family or cut that does
/// not match the solution can also be [`SolverError::InvalidInput`].
pub fn certify_laporta(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    dispatch_solver_capacity!(
        family.denominator_count(),
        certify_laporta_with_capacity(family, cuts, solution, include_lorentz),
        arity => Err(unsupported_runtime_arity(arity))
    )
}

/// Independently certify a Laporta solution at an explicitly compiled arity.
///
/// This has the same certificate scope and checks as [`certify_laporta`]; it
/// does not prove master independence or pole completeness. `N` must match the
/// family and be positive, independently of the runtime registry.
pub fn certify_laporta_for<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    validate_family_arity::<N>(family)?;
    certify::<N>(family, cuts, solution, include_lorentz)
}

fn fail(message: String) -> SolverError {
    SolverError::Certification(message)
}

// DynamicSolution is public and mutable. Validate all keys before projecting
// DynamicPower to numeric values or calling the compact order's infallible
// internal constructors; a symbolic rule is not a numeric Laporta identity.
fn validate_numeric_key<const N: usize>(powers: &[i16]) -> Result<(), SolverError> {
    let powers: [i16; N] = powers.try_into().map_err(|_| {
        fail(format!(
            "certificate integral has arity {}, expected {N}",
            powers.len()
        ))
    })?;
    Integral::numeric(powers)
        .map_err(|error| fail(format!("invalid certificate integral: {error}")))?;
    Ok(())
}

fn validate_rule<const N: usize>(rule: &DynamicRule) -> Result<(), SolverError> {
    for powers in std::iter::once(&rule.target).chain(rule.rhs.iter().map(|term| &term.powers)) {
        if powers.iter().any(|power| power.symbolic) {
            return Err(fail("a Laporta certificate requires numeric powers".into()));
        }
        validate_numeric_key::<N>(&values(powers))?;
    }
    if rule.sector.len() != N
        || !rule
            .sector
            .iter()
            .copied()
            .eq(rule.target.iter().map(|power| power.value > 0))
    {
        return Err(fail(
            "certificate rule sector does not match its numeric target".into(),
        ));
    }
    Ok(())
}

fn validate_solution<const N: usize>(solution: &DynamicSolution) -> Result<(), SolverError> {
    for key in solution.requested.iter().chain(&solution.residuals) {
        validate_numeric_key::<N>(key)?;
    }
    for rule in &solution.rules {
        validate_rule::<N>(rule)?;
    }
    for derived in &solution.derivation {
        validate_rule::<N>(&derived.rule)?;
        if let RuleOrigin::Identities { seeds } = &derived.origin {
            for seed in seeds {
                validate_numeric_key::<N>(seed)?;
            }
        }
    }
    if let Some(change) = &solution.basis_change {
        for key in change
            .replaced
            .iter()
            .chain(change.preferred.iter().map(|master| &master.integral))
        {
            validate_numeric_key::<N>(key)?;
        }
        for rule in &change.original {
            validate_rule::<N>(rule)?;
        }
    }
    Ok(())
}

pub(super) fn certify<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    solution: &DynamicSolution,
    include_lorentz: bool,
) -> Result<ReductionCertificate, SolverError> {
    validate_solution::<N>(solution)?;
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
    // One over the identities' variable map, which the solver's rules share.
    let one = relations
        .iter()
        .flat_map(|relation| relation.terms().values())
        .next()
        .map(|coefficient| unit(coefficient.raw()))
        .ok_or_else(|| fail("the family has no identities".into()))?;
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
        // target - sum_j c_j I_j, which is just the target for a zero rule.
        let desired: Vec<(Vec<i16>, Combination)> = rules
            .iter()
            .map(|rule| {
                let target = values(&rule.target);
                let mut row = Combination::new();
                add(&mut row, target.clone(), one.clone());
                for term in &rule.rhs {
                    add(&mut row, values(&term.powers), -term.coefficient.clone());
                }
                (target, row)
            })
            .collect();
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

    // The search residuals: before any basis change, the replaced residuals
    // were residuals and the replaced preferred masters had rules.
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

    // Reduce every derived target once, easiest first; strict descent makes
    // every right-hand side already reduced or a search residual.
    let mut reduced = BTreeMap::<Vec<i16>, Combination>::new();
    let mut targets: Vec<&Vec<i16>> = derived
        .keys()
        .filter(|target| !search_residuals.contains(*target))
        .collect();
    targets.sort_by(|left, right| compare(right, left));
    for target in targets {
        let mut combination = Combination::new();
        for term in &derived[target].rhs {
            let powers = values(&term.powers);
            if search_residuals.contains(&powers) {
                add(&mut combination, powers, term.coefficient.clone());
            } else if let Some(inner) = reduced.get(&powers) {
                for (integral, coefficient) in inner {
                    add(
                        &mut combination,
                        integral.clone(),
                        &term.coefficient * coefficient,
                    );
                }
            } else {
                return Err(fail(format!(
                    "{powers:?} in the rule for {target:?} is neither derived nor a residual"
                )));
            }
        }
        reduced.insert(target.clone(), combination);
    }
    let expand = |row: &mut Combination, integral: Vec<i16>, coefficient: Coefficient| {
        if search_residuals.contains(&integral) {
            add(row, integral, coefficient);
        } else if let Some(inner) = reduced.get(&integral) {
            for (key, value) in inner {
                add(row, key.clone(), &coefficient * value);
            }
        } else {
            return Err(fail(format!(
                "{integral:?} is neither derived nor a residual"
            )));
        }
        Ok(())
    };

    // Every returned rule, with replaced preferred masters standing for their
    // original reductions, must vanish once expressed in search residuals.
    let residuals: BTreeSet<&Vec<i16>> = solution.residuals.iter().collect();
    for rule in &solution.rules {
        let target = values(&rule.target);
        if residuals.contains(&target) {
            return Err(fail(format!(
                "{target:?} is both a residual and a rule target"
            )));
        }
        if let Some(term) = rule
            .rhs
            .iter()
            .find(|term| !residuals.contains(&values(&term.powers)))
        {
            return Err(fail(format!(
                "the rule for {target:?} uses {:?}, which is not a residual",
                values(&term.powers)
            )));
        }
    }
    for rule in solution.rules.iter().chain(back.values().copied()) {
        let target = values(&rule.target);
        let mut row = Combination::new();
        expand(&mut row, target.clone(), one.clone())?;
        for term in &rule.rhs {
            let powers = values(&term.powers);
            match back.get(&powers) {
                Some(original) => {
                    for inner in &original.rhs {
                        expand(
                            &mut row,
                            values(&inner.powers),
                            -(&term.coefficient * &inner.coefficient),
                        )?;
                    }
                }
                None => expand(&mut row, powers, -term.coefficient.clone())?,
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
            .enumerate()
            .map(|(axis, &seed)| {
                i16::try_from(i64::from(seed) + shift.values().get(axis).copied().unwrap_or(0))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| fail("an instantiated power overflows".into()))?;
        validate_numeric_key::<N>(&powers)?;
        if is_zero(&powers) {
            continue;
        }
        let raw = coefficient.raw();
        let (mut numerator, mut denominator) = (raw.numerator.clone(), raw.denominator.clone());
        for (axis, &value) in seed.iter().take(shift.values().len()).enumerate() {
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
