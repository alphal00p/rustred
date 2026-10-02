//! Exact change from a Laporta solution's residuals to preferred masters.
//!
//! The search reduces every preferred integral `p` it can: `p = sum_r c_r r`
//! over the residuals `r`. Each such `p` replaces one residual of its own
//! sector. Per sector, the rows `p - sum_r c_r r` are brought to reduced
//! echelon form over the sector's remaining residuals, hardest column first,
//! and every pivot residual is then expressed through the preferred integrals
//! and the other residuals. Lower sectors are processed first, so a higher
//! sector's rows already use their replacements. Dividing by a pivot adds its
//! numerator to the nonzero conditions of every rule that uses the pivot's
//! replacement. Mapping each replaced preferred integral back to its original
//! reduction must restore every rule of the search exactly; this is checked.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::sector::Restrictions;
use crate::solver::{CoordinateCase, SolverError};

use super::combination::{Combination, NumericOrder, add, combination, sector_of, unit, values};
use super::{
    DynamicPower, DynamicRule, DynamicSolution, DynamicTerm, array, extend_conditions, is_excluded,
};

/// What the basis change did with one preferred master.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreferredStatus {
    /// The search reduced it, and it replaced a residual of its own sector.
    Replaced,
    /// The search left it unreduced, so it already was a residual. Whether
    /// that basis is minimal is a question about the search, not the change.
    Residual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreferredMaster {
    pub integral: Vec<i16>,
    pub status: PreferredStatus,
}

/// A preferred-master basis change applied to a Laporta solution.
#[derive(Clone, Debug, Default)]
pub struct BasisChange {
    /// The preferred masters, in request order.
    pub preferred: Vec<PreferredMaster>,
    /// The search residuals that the preferred masters replaced.
    pub replaced: Vec<Vec<i16>>,
    /// The search's own rules for the replaced preferred masters, through
    /// which every returned rule maps back to the search result.
    pub original: Vec<DynamicRule>,
    /// Numerators of the pivots divided by during the change.
    pub conditions: Vec<CoefficientPolynomial>,
}

/// Check each preferred master's arity and range, reject duplicates, and
/// reject integrals that vanish outside the cut.
pub(super) fn validate<const N: usize>(
    preferred: &[Vec<i16>],
    restrictions: &Restrictions,
) -> Result<Vec<[i16; N]>, SolverError> {
    let mut seen = BTreeSet::new();
    let mut validated = Vec::with_capacity(preferred.len());
    for master in preferred {
        let powers = array::<_, N>(master, "preferred master")?;
        CoordinateCase::new(powers.map(Some))?;
        if !seen.insert(powers) {
            return Err(SolverError::InvalidInput(format!(
                "preferred master {master:?} is listed twice"
            )));
        }
        if is_excluded(restrictions, sector_of::<N>(&powers))? {
            return Err(SolverError::InvalidInput(format!(
                "preferred master {master:?} lies outside the cut; it vanishes"
            )));
        }
        validated.push(powers);
    }
    Ok(validated)
}

/// Replace residuals by the preferred masters; see the module documentation.
pub(super) fn prefer<const N: usize>(
    result: &mut DynamicSolution,
    preferred: &[[i16; N]],
) -> Result<(), SolverError> {
    let order = NumericOrder::<N>::new();
    let residuals: BTreeSet<Vec<i16>> = result.residuals.iter().cloned().collect();
    let preferred_keys: BTreeSet<Vec<i16>> =
        preferred.iter().map(|master| master.to_vec()).collect();
    let rule_of: BTreeMap<Vec<i16>, usize> = result
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (values(&rule.target), index))
        .collect();

    let mut statuses = Vec::with_capacity(preferred.len());
    let mut reduced = Vec::new();
    for master in preferred {
        let key = master.to_vec();
        if residuals.contains(&key) {
            statuses.push(PreferredMaster {
                integral: key,
                status: PreferredStatus::Residual,
            });
            continue;
        }
        let index = *rule_of.get(&key).ok_or_else(|| {
            SolverError::ExactReplay(format!(
                "preferred master {key:?} has neither a rule nor a residual"
            ))
        })?;
        let rule = &result.rules[index];
        if rule.rhs.is_empty() {
            return Err(SolverError::InvalidInput(format!(
                "preferred master {key:?} reduces to zero"
            )));
        }
        let own = sector_of(master);
        for term in &rule.rhs {
            let powers = values(&term.powers);
            // Search rules strictly descend, so no term can lie in a later sector.
            if order.sectors(&sector_of(&powers), &own) == Ordering::Less {
                return Err(SolverError::ExactReplay(format!(
                    "the rule for preferred master {key:?} uses {powers:?} from a later sector"
                )));
            }
        }
        if !rule
            .rhs
            .iter()
            .any(|term| sector_of(&values(&term.powers)) == own)
        {
            let lower: Vec<_> = rule.rhs.iter().map(|term| values(&term.powers)).collect();
            return Err(SolverError::InvalidInput(format!(
                "preferred master {key:?} reduces to lower-sector integrals {lower:?}; it cannot replace a master of its sector"
            )));
        }
        statuses.push(PreferredMaster {
            integral: key,
            status: PreferredStatus::Replaced,
        });
        reduced.push((*master, index));
    }
    // Lowest sector first; the stable sort keeps request order within a sector.
    reduced.sort_by(|left, right| order.sectors(&sector_of(&right.0), &sector_of(&left.0)));

    let mut replacements = BTreeMap::<Vec<i16>, (Combination, Vec<CoefficientPolynomial>)>::new();
    let mut pivot_conditions = Vec::new();
    for group in reduced.chunk_by(|left, right| sector_of::<N>(&left.0) == sector_of::<N>(&right.0))
    {
        let own = sector_of::<N>(&group[0].0);
        let mut conditions = Vec::new();
        let mut rows = Vec::with_capacity(group.len());
        for &(master, index) in group {
            let rule = &result.rules[index];
            extend_conditions(&mut conditions, &rule.nonzero_conditions);
            let one = unit(&rule.rhs[0].coefficient);
            let mut row = Combination::new();
            add(&mut row, master.to_vec(), one);
            for term in &rule.rhs {
                let key = values(&term.powers);
                match replacements.get(&key) {
                    Some((combination, used)) => {
                        extend_conditions(&mut conditions, used);
                        for (integral, coefficient) in combination {
                            add(
                                &mut row,
                                integral.clone(),
                                -(&term.coefficient * coefficient),
                            );
                        }
                    }
                    None => add(&mut row, key, -term.coefficient.clone()),
                }
            }
            rows.push((master.to_vec(), row));
        }

        // Gauss-Jordan over this sector's residuals that are not preferred.
        let mut pivots: Vec<Vec<i16>> = Vec::with_capacity(rows.len());
        for current in 0..rows.len() {
            for (earlier, pivot) in pivots.iter().enumerate() {
                if let Some(factor) = rows[current].1.get(pivot).cloned() {
                    let source = rows[earlier].1.clone();
                    subtract(&mut rows[current].1, &source, &factor);
                }
            }
            let pivot = rows[current]
                .1
                .keys()
                .filter(|key| sector_of(key) == own && !preferred_keys.contains(*key))
                .min_by(|left, right| order.integrals(left, right))
                .cloned();
            let Some(pivot) = pivot else {
                let master = &rows[current].0;
                let others: Vec<_> = rows[current]
                    .1
                    .keys()
                    .filter(|key| *key != master && sector_of(key) == own)
                    .collect();
                return Err(SolverError::InvalidInput(format!(
                    "preferred master {master:?} depends on preferred master(s) {others:?} modulo lower sectors"
                )));
            };
            let scale = rows[current].1[&pivot].clone();
            if !scale.numerator.is_constant() {
                for target in [&mut conditions, &mut pivot_conditions] {
                    if !target.contains(&scale.numerator) {
                        target.push(scale.numerator.clone());
                    }
                }
            }
            for coefficient in rows[current].1.values_mut() {
                *coefficient = &*coefficient / &scale;
            }
            for earlier in 0..current {
                if let Some(factor) = rows[earlier].1.get(&pivot).cloned() {
                    let source = rows[current].1.clone();
                    subtract(&mut rows[earlier].1, &source, &factor);
                }
            }
            pivots.push(pivot);
        }
        // Each normalized row reads pivot + sum_x a_x x = 0.
        for ((_, row), pivot) in rows.into_iter().zip(pivots) {
            let combination = row
                .into_iter()
                .filter(|(integral, _)| *integral != pivot)
                .map(|(integral, coefficient)| (integral, -coefficient))
                .collect();
            replacements.insert(pivot, (combination, conditions.clone()));
        }
    }

    let original_rules: BTreeMap<Vec<i16>, Combination> = result
        .rules
        .iter()
        .map(|rule| (values(&rule.target), combination(&rule.rhs)))
        .collect();
    let original: Vec<DynamicRule> = reduced
        .iter()
        .map(|&(_, index)| result.rules[index].clone())
        .collect();
    let mut rules = Vec::with_capacity(result.rules.len());
    for mut rule in std::mem::take(&mut result.rules) {
        if preferred_keys.contains(&values(&rule.target)) {
            continue;
        }
        if rule
            .rhs
            .iter()
            .any(|term| replacements.contains_key(&values(&term.powers)))
        {
            let mut rhs = Combination::new();
            for term in &rule.rhs {
                let key = values(&term.powers);
                match replacements.get(&key) {
                    Some((combination, used)) => {
                        extend_conditions(&mut rule.nonzero_conditions, used);
                        for (integral, coefficient) in combination {
                            add(&mut rhs, integral.clone(), &term.coefficient * coefficient);
                        }
                    }
                    None => add(&mut rhs, key, term.coefficient.clone()),
                }
            }
            rule.rhs = terms(rhs);
        }
        rules.push(rule);
    }
    for (pivot, (combination, conditions)) in &replacements {
        rules.push(DynamicRule {
            sector: sector_of::<N>(pivot).to_vec(),
            target: pivot
                .iter()
                .map(|&value| DynamicPower {
                    symbolic: false,
                    value,
                })
                .collect(),
            rhs: terms(combination.clone()),
            nonzero_conditions: conditions.clone(),
            exceptions: Vec::new(),
        });
    }
    result.rules = rules;
    let mut basis: BTreeSet<Vec<i16>> = residuals
        .into_iter()
        .filter(|residual| !replacements.contains_key(residual))
        .collect();
    basis.extend(preferred_keys);
    result.residuals = basis.into_iter().collect();

    let back: BTreeMap<Vec<i16>, Combination> = original
        .iter()
        .map(|rule| (values(&rule.target), combination(&rule.rhs)))
        .collect();
    for rule in &result.rules {
        let target = values(&rule.target);
        let mut mapped = Combination::new();
        for term in &rule.rhs {
            let key = values(&term.powers);
            match back.get(&key) {
                Some(combination) => {
                    for (integral, coefficient) in combination {
                        add(
                            &mut mapped,
                            integral.clone(),
                            &term.coefficient * coefficient,
                        );
                    }
                }
                None => add(&mut mapped, key, term.coefficient.clone()),
            }
        }
        let restored = match original_rules.get(&target) {
            Some(expected) => same(&mapped, expected),
            // A replaced residual must map back to itself.
            None => {
                mapped.len() == 1
                    && mapped
                        .get(&target)
                        .is_some_and(|coefficient| (coefficient - &unit(coefficient)).is_zero())
            }
        };
        if !restored {
            return Err(SolverError::ExactReplay(format!(
                "the preferred-master basis change does not map back to the search result for {target:?}"
            )));
        }
    }
    result.basis_change = Some(BasisChange {
        preferred: statuses,
        replaced: replacements.into_keys().collect(),
        original,
        conditions: pivot_conditions,
    });
    Ok(())
}

/// `target -= factor * source`.
fn subtract(target: &mut Combination, source: &Combination, factor: &Coefficient) {
    for (integral, coefficient) in source {
        add(target, integral.clone(), -(factor * coefficient));
    }
}

fn same(left: &Combination, right: &Combination) -> bool {
    left.len() == right.len()
        && left.iter().all(|(integral, coefficient)| {
            right
                .get(integral)
                .is_some_and(|other| (coefficient - other).is_zero())
        })
}

fn terms(combination: Combination) -> Vec<DynamicTerm> {
    combination
        .into_iter()
        .map(|(powers, coefficient)| DynamicTerm {
            powers: powers
                .into_iter()
                .map(|value| DynamicPower {
                    symbolic: false,
                    value,
                })
                .collect(),
            coefficient,
        })
        .collect()
}
