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

use symbolica::domains::{integer::Z, rational_polynomial::RationalPolynomialField};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::CoefficientPolynomial;
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

        // Keep eligible residuals first, hardest first. The remaining columns
        // are protected preferred masters and lower sectors, not legal pivots.
        // Symbolica owns both elimination and back substitution.
        let all: BTreeSet<Vec<i16>> = rows
            .iter()
            .flat_map(|(_, row)| row.keys().cloned())
            .collect();
        let eligible = |key: &Vec<i16>| sector_of(key) == own && !preferred_keys.contains(key);
        let mut columns: Vec<_> = all.iter().filter(|key| eligible(key)).cloned().collect();
        columns.sort_by(|left, right| order.integrals(left, right));
        let eligible_count = columns.len();
        columns.extend(all.iter().filter(|key| !eligible(key)).cloned());
        let width = u32::try_from(columns.len())
            .map_err(|_| SolverError::InvalidInput("too many preferred-basis columns".into()))?;
        let index: BTreeMap<_, _> = columns
            .iter()
            .enumerate()
            .map(|(i, key)| (key, i as u32))
            .collect();
        let mut reducer =
            SparseRowReducer::new(width, RationalPolynomialField::new(Z), LuLMode::Full);
        for (master, row) in &rows {
            let mut entries: Vec<_> = row
                .iter()
                .map(|(key, coefficient)| (index[key], coefficient.clone()))
                .collect();
            entries.sort_by_key(|(column, _)| *column);
            let (indices, values): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
            let pivot = reducer.add_row(&values, &indices);
            if !pivot.is_some_and(|column| (column as usize) < eligible_count) {
                let others: Vec<_> = preferred_keys
                    .iter()
                    .filter(|key| *key != master && sector_of(key) == own)
                    .collect();
                return Err(SolverError::InvalidInput(format!(
                    "preferred master {master:?} depends on preferred master(s) {others:?} modulo lower sectors"
                )));
            }
            // Native L's last diagonal is the unnormalized pivot. Retain its
            // pole before back_substitute clears L, even if the final RHS cancels it.
            let scale = reducer
                .l()
                .last_row()
                .and_then(|(_, _, values)| values.last())
                .ok_or_else(|| {
                    SolverError::ExactReplay("native preferred-basis pivot scale missing".into())
                })?;
            if !scale.numerator.is_constant() {
                for target in [&mut conditions, &mut pivot_conditions] {
                    if !target.contains(&scale.numerator) {
                        target.push(scale.numerator.clone());
                    }
                }
            }
        }
        reducer.back_substitute();
        // Each normalized row reads pivot + sum_x a_x x = 0.
        // Native back substitution may reorder rows: bind through its pivot map.
        for (pivot, row) in reducer
            .pivots()
            .iter()
            .enumerate()
            .filter_map(|(p, r)| r.map(|r| (p, r as usize)))
        {
            let matrix = reducer.u();
            let range = matrix.row_ptrs()[row]..matrix.row_ptrs()[row + 1];
            let combination = matrix.col_idcs()[range.clone()]
                .iter()
                .zip(&matrix.values()[range])
                .filter(|(column, _)| **column as usize != pivot)
                .map(|(column, coefficient)| {
                    (columns[*column as usize].clone(), -coefficient.clone())
                })
                .collect();
            replacements.insert(columns[pivot].clone(), (combination, conditions.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::{Coefficient, CoefficientContext};

    #[test]
    fn native_basis_reducer_preserves_coupled_rows_lower_tails_and_pivot_guards() {
        let c = CoefficientContext::try_new(["d"]).unwrap();
        let d = c.parameter("d").unwrap();
        let rule = |target: [i16; 2], rhs: Combination| DynamicRule {
            sector: vec![true, true],
            target: target
                .into_iter()
                .map(|value| DynamicPower {
                    symbolic: false,
                    value,
                })
                .collect(),
            rhs: terms(rhs),
            nonzero_conditions: Vec::new(),
            exceptions: Vec::new(),
        };
        let mut solution = DynamicSolution {
            rules: vec![
                rule(
                    [3, 1],
                    BTreeMap::from([
                        (vec![1, 1], d.clone()),
                        (vec![2, 1], c.integer(1)),
                        (vec![1, 0], c.integer(1)),
                    ]),
                ),
                rule(
                    [4, 1],
                    BTreeMap::from([(vec![1, 1], c.integer(1)), (vec![2, 1], d.clone())]),
                ),
            ],
            residuals: vec![vec![1, 1], vec![2, 1], vec![1, 0]],
            ..Default::default()
        };
        let original = solution.clone();
        prefer::<2>(&mut solution, &[[3, 1], [4, 1]]).unwrap();
        let determinant = &(&d * &d) - &c.integer(1);
        let expected = BTreeMap::from([
            (vec![3, 1], &d / &determinant),
            (vec![4, 1], -(&c.integer(1) / &determinant)),
            (vec![1, 0], -(&d / &determinant)),
        ]);
        let actual = solution
            .rules
            .iter()
            .find(|rule| values(&rule.target) == [1, 1])
            .unwrap();
        assert!(same(&combination(&actual.rhs), &expected));
        assert!(
            solution
                .basis_change
                .as_ref()
                .unwrap()
                .conditions
                .iter()
                .any(|condition| {
                    let value = Coefficient::from(condition.clone());
                    (&value - &determinant).is_zero() || (&value + &determinant).is_zero()
                })
        );
        // The same native path must refuse a dependent preferred direction,
        // rather than pivot on a protected preferred or lower-sector column.
        let mut dependent = original;
        dependent.rules[1].rhs = dependent.rules[0].rhs.clone();
        assert!(matches!(
            prefer::<2>(&mut dependent, &[[3, 1], [4, 1]]),
            Err(SolverError::InvalidInput(_))
        ));
    }
}
