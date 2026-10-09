//! Independent exact replay of guarded original-source traces.
//!
//! The augmented identity columns below are reduced by Symbolica's existing
//! sparse rational-polynomial reducer. They recover exact source weights;
//! multiplying those weights back into the original rows is a separate check.

use std::cmp::Ordering;

use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::prelude::{IntegerRing, Z};
use symbolica::tensors::sparse::{LuLMode, SparseRowReducer};

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::solver::instantiate::{canonicalize, instantiate_polynomial, translate};
use crate::solver::{ExactRow, IntegralOrder, RuleCandidate, Seed, SolverError, Term};

use super::{
    GuardedRule, GuardedSearchScope, GuardedSourceSystem, IndexBounds, IndexDomain, IndexRole,
};

struct Replayed<const N: usize> {
    domain: IndexDomain<N>,
    nonzero_conditions: Vec<CoefficientPolynomial>,
}

impl<const N: usize> GuardedSourceSystem<N> {
    /// Reconstruct the equation, applicability domain and coefficient guards
    /// from the original source corpus. Saved rule metadata is not authority.
    pub fn replay_rule(&self, rule: &GuardedRule<N>) -> Result<(), SolverError> {
        let replay = self.replay_candidate(&rule.candidate, &rule.order, &rule.discovery_domain)?;
        if !rule.domain.is_subset_of(&replay.domain) {
            return Err(certification(
                "saved rule domain exceeds the replayed source domain",
            ));
        }
        if rule.nonzero_conditions.len() != replay.nonzero_conditions.len()
            || replay
                .nonzero_conditions
                .iter()
                .any(|condition| !rule.nonzero_conditions.contains(condition))
        {
            return Err(certification(
                "saved rule nonzero conditions differ from exact replay",
            ));
        }
        Ok(())
    }

    pub(in crate::solver) fn seal_candidate(
        &self,
        candidate: RuleCandidate<N>,
        order: IntegralOrder<N>,
        discovery_domain: IndexDomain<N>,
    ) -> Result<GuardedRule<N>, SolverError> {
        let replay = self.replay_candidate(&candidate, &order, &discovery_domain)?;
        Ok(GuardedRule {
            candidate,
            domain: replay.domain,
            nonzero_conditions: replay.nonzero_conditions,
            order,
            discovery_domain,
        })
    }

    fn replay_candidate(
        &self,
        candidate: &RuleCandidate<N>,
        order: &IntegralOrder<N>,
        discovery_domain: &IndexDomain<N>,
    ) -> Result<Replayed<N>, SolverError> {
        if order.roles() != Some(&self.roles) || order.physical_arity() != N {
            return Err(certification(
                "guarded rule ordering differs from its coordinate roles",
            ));
        }
        let case = candidate
            .case
            .coordinate()
            .ok_or_else(|| certification("guarded replay currently requires a coordinate case"))?;
        if candidate.target != case.integral() {
            return Err(certification("guarded replay requires a canonical target"));
        }
        let case_domain = IndexDomain::from_sector_case(order.sector(), case)
            .and_then(|domain| domain.intersection(&IndexDomain::for_roles(&self.roles)))
            .ok_or_else(|| certification("rule case has no admissible index domain"))?;
        if !discovery_domain.is_subset_of(&case_domain) {
            return Err(certification(
                "discovery domain exceeds its declared coordinate case",
            ));
        }
        let template = self
            .system
            .rows()
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| certification("original sources have no coefficient map"))?
            .coefficient
            .clone();
        for term in &candidate.rhs {
            if term.coefficient.numerator.variables() != template.variables()
                || term.coefficient.denominator.variables() != template.variables()
                || (0..N).any(|axis| {
                    term.integral[axis].is_symbolic() != candidate.target[axis].is_symbolic()
                })
            {
                return Err(certification(
                    "rule RHS has a foreign coefficient map or coordinate pattern",
                ));
            }
        }
        if candidate.sources.is_empty() {
            return Err(certification(
                "guarded rule has no original-source provenance",
            ));
        }
        let scope = GuardedSearchScope::new(self, discovery_domain.clone());
        let mut rows = Vec::with_capacity(candidate.sources.len());
        for source in &candidate.sources {
            validate_seed(candidate, &source.seed)?;
            let original = self
                .system
                .rows()
                .get(source.basis_row)
                .ok_or_else(|| certification("original-source ordinal is out of range"))?;
            let row = scope
                .instantiate(source.basis_row, original, &source.seed, order)?
                .ok_or_else(|| {
                    certification("retained source is not valid throughout its discovery domain")
                })?;
            if row.windows(2).any(|terms| {
                order.compare(&terms[0].integral, &terms[1].integral) != Ordering::Less
            }) {
                return Err(certification(
                    "replayed original source row is not canonical",
                ));
            }
            rows.push(row);
        }
        let mut columns: Vec<_> = rows.iter().flatten().map(|term| term.integral).collect();
        columns.sort_unstable_by(|a, b| order.compare(a, b));
        columns.dedup();
        let native_columns = columns
            .len()
            .checked_add(rows.len())
            .and_then(|count| u32::try_from(count).ok())
            .ok_or_else(|| certification("guarded exact replay column count overflow"))?;
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
                        .expect("source column belongs to its physical union")
                        as u32
                })
                .collect();
            values.push(one.clone());
            indices.push((columns.len() + source_ordinal) as u32);
            let Some(pivot_column) = reducer.add_row(&values, &indices) else {
                continue;
            };
            let Some(raw_target) = columns.get(pivot_column as usize) else {
                continue;
            };
            if !candidate.case.matches(raw_target) {
                continue;
            }
            let u = reducer.u();
            let row_index = u.nrows() as usize - 1;
            let start = u.row_ptrs()[row_index];
            let end = u.row_ptrs()[row_index + 1];
            let mut physical: ExactRow<N> = Vec::new();
            let mut weights = Vec::new();
            for (&column, coefficient) in
                u.col_idcs()[start..end].iter().zip(&u.values()[start..end])
            {
                if let Some(integral) = columns.get(column as usize) {
                    physical.push(Term {
                        integral: *integral,
                        coefficient: coefficient.clone(),
                    });
                } else if !coefficient.is_zero() {
                    weights.push((column as usize - columns.len(), coefficient.clone()));
                }
            }
            let (target, rhs) = canonicalize(physical.clone(), self.system.index_variables())?;
            if target != candidate.target || rhs != candidate.rhs {
                continue;
            }
            let zero: Coefficient = template.zero().into();
            let mut check = vec![zero; columns.len()];
            for (ordinal, weight) in &weights {
                let source = rows
                    .get(*ordinal)
                    .ok_or_else(|| certification("replayed source-weight ordinal is invalid"))?;
                for term in source {
                    let column = columns
                        .binary_search_by(|key| order.compare(key, &term.integral))
                        .expect("source column belongs to replay union");
                    check[column] = &check[column] + &(weight * &term.coefficient);
                }
            }
            for term in &physical {
                let column = columns
                    .binary_search_by(|key| order.compare(key, &term.integral))
                    .expect("physical replay column belongs to source union");
                check[column] = &check[column] - &term.coefficient;
            }
            if check.iter().any(|coefficient| !coefficient.is_zero()) {
                return Err(certification(
                    "original-source weights fail exact multiply-back",
                ));
            }
            let recenter: [i16; N] = std::array::from_fn(|axis| {
                if raw_target[axis].is_symbolic() {
                    -raw_target[axis].value()
                } else {
                    0
                }
            });
            let shifts = recenter.map(i64::from);
            let mut domain = discovery_domain
                .intersection(&discovery_domain.pullback(&shifts)?)
                .ok_or_else(|| {
                    certification("recentered source proof misses the requested target domain")
                })?;
            // Keep each original source guard explicit after target recentering.
            // Numeric source coordinates are constants, not translated targets.
            for source in &candidate.sources {
                let guard = self
                    .sources
                    .get(source.basis_row)
                    .ok_or_else(|| certification("original source metadata is absent"))?;
                let source_preimage = seed_preimage(&guard.domain, &source.seed)?;
                domain = domain
                    .intersection(&source_preimage.pullback(&shifts)?)
                    .ok_or_else(|| {
                        certification("recentered original-source guards have empty intersection")
                    })?;
            }
            let pivot = translate(
                &physical[0].coefficient,
                self.system.index_variables(),
                &recenter,
            );
            let mut nonzero_conditions = Vec::new();
            append_condition(&mut nonzero_conditions, pivot.numerator.clone())?;
            append_condition(&mut nonzero_conditions, pivot.denominator.clone())?;
            for (ordinal, weight) in &weights {
                let normalized =
                    &translate(weight, self.system.index_variables(), &recenter) / &pivot;
                // In particular, a*I=0 needs a!=0 even though the RHS is empty.
                append_condition(&mut nonzero_conditions, normalized.denominator.clone())?;
                let source = &candidate.sources[*ordinal];
                for condition in &self.sources[source.basis_row].nonzero_conditions {
                    let value = instantiate_polynomial(
                        condition,
                        &source.seed,
                        self.system.index_variables(),
                        self.system.fixed(),
                        None,
                    )?;
                    let value = translate(&value, self.system.index_variables(), &recenter);
                    append_condition(&mut nonzero_conditions, value.numerator)?;
                    append_condition(&mut nonzero_conditions, value.denominator)?;
                }
            }
            for term in &candidate.rhs {
                append_condition(
                    &mut nonzero_conditions,
                    term.coefficient.denominator.clone(),
                )?;
            }
            prove_descent(candidate, order, &domain, &self.roles)?;
            return Ok(Replayed {
                domain,
                nonzero_conditions,
            });
        }
        Err(certification(
            "original-source replay does not recover the candidate equation",
        ))
    }
}

fn validate_seed<const N: usize>(
    candidate: &RuleCandidate<N>,
    seed: &Seed<N>,
) -> Result<(), SolverError> {
    for axis in 0..N {
        let power = seed.integral[axis];
        if power.is_symbolic() != candidate.target[axis].is_symbolic()
            || (power.is_symbolic() && power.value() != seed.shifts[axis])
            || (!power.is_symbolic() && seed.shifts[axis] != 0)
        {
            return Err(certification(
                "source seed coefficient and integral translations disagree",
            ));
        }
    }
    Ok(())
}

fn seed_preimage<const N: usize>(
    domain: &IndexDomain<N>,
    seed: &Seed<N>,
) -> Result<IndexDomain<N>, SolverError> {
    let mut bounds = *domain.bounds();
    for (axis, bound) in bounds.iter_mut().enumerate() {
        let power = seed.integral[axis];
        if power.is_symbolic() {
            let shift = i64::from(power.value());
            bound.lower = bound
                .lower
                .map(|value| {
                    value
                        .checked_sub(shift)
                        .ok_or_else(|| certification("source guard endpoint overflow"))
                })
                .transpose()?;
            bound.upper = bound
                .upper
                .map(|value| {
                    value
                        .checked_sub(shift)
                        .ok_or_else(|| certification("source guard endpoint overflow"))
                })
                .transpose()?;
        } else if bound.contains(i64::from(power.value())) {
            *bound = IndexBounds::unbounded();
        } else {
            return Err(certification(
                "fixed source coordinate violates its identity guard",
            ));
        }
    }
    IndexDomain::new(bounds)
}

fn append_condition(
    conditions: &mut Vec<CoefficientPolynomial>,
    condition: CoefficientPolynomial,
) -> Result<(), SolverError> {
    if condition.is_zero() {
        return Err(certification(
            "source replay requires an identically zero factor to be nonzero",
        ));
    }
    if !condition.is_constant() && !conditions.contains(&condition) {
        conditions.push(condition);
    }
    Ok(())
}

/// Constant-sign symbolic differences make every ordinary comparison priority
/// independent of the free indices. Occupations use their separate graded order.
fn prove_descent<const N: usize>(
    candidate: &RuleCandidate<N>,
    order: &IntegralOrder<N>,
    domain: &IndexDomain<N>,
    roles: &[IndexRole; N],
) -> Result<(), SolverError> {
    for term in &candidate.rhs {
        if term.coefficient.is_zero() {
            continue;
        }
        for axis in 0..N {
            let power = term.integral[axis];
            let bound = domain.bounds()[axis];
            if roles[axis] == IndexRole::Occupation {
                let lower = if power.is_symbolic() {
                    bound
                        .lower()
                        .and_then(|value| value.checked_add(i64::from(power.value())))
                } else {
                    Some(i64::from(power.value()))
                };
                if !lower.is_some_and(|value| value >= 0) {
                    return Err(certification(
                        "rule RHS occupation validity is not uniform on its domain",
                    ));
                }
            } else if power.is_symbolic() {
                let displacement = i64::from(power.value());
                let stable = if order.sector()[axis] {
                    bound
                        .lower()
                        .and_then(|value| value.checked_add(displacement))
                        .is_some_and(|value| value > 0)
                } else {
                    bound
                        .upper()
                        .and_then(|value| value.checked_add(displacement))
                        .is_some_and(|value| value <= 0)
                };
                if !stable {
                    return Err(certification(
                        "symbolic RHS crosses an ordinary sector boundary; uniform descent is unresolved",
                    ));
                }
            }
        }
        if order.compare(&candidate.target, &term.integral) != Ordering::Less {
            return Err(certification(
                "rule RHS is not strictly lower in the role-aware integral order",
            ));
        }
    }
    Ok(())
}

fn certification(message: &str) -> SolverError {
    SolverError::Certification(message.into())
}

#[cfg(test)]
mod tests {
    use super::super::GuardedSource;
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{CoordinateCase, Integral, SeedSource};

    fn domain(lower: i64) -> IndexDomain<2> {
        IndexDomain::new([
            IndexBounds::new(Some(lower), None).unwrap(),
            IndexBounds::fixed(0),
        ])
        .unwrap()
    }

    fn order() -> IntegralOrder<2> {
        IntegralOrder::new([true, false], [false; 2])
            .with_roles([IndexRole::Ordinary, IndexRole::Occupation])
            .unwrap()
    }

    fn candidate(shift: i16) -> RuleCandidate<2> {
        let case = CoordinateCase::new([None, Some(0)]).unwrap();
        let mut seed = case.integral();
        seed = seed.shifted([shift, 0]).unwrap();
        RuleCandidate {
            target: case.integral(),
            case: case.into(),
            rhs: Vec::new(),
            sources: vec![SeedSource {
                basis_row: 0,
                seed: Seed {
                    integral: seed,
                    shifts: [shift, 0],
                },
            }],
            stats: Default::default(),
        }
    }

    #[test]
    fn empty_rhs_keeps_source_weight_pole_and_rejects_tampering() {
        let context = CoefficientContext::new(["n", "b"]);
        let polynomial = context.coefficient_fixture("n").numerator;
        let source = GuardedSource::new(
            "conditional-zero",
            vec![Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: polynomial.clone(),
            }],
            domain(1),
        );
        let system = GuardedSourceSystem::new(
            "replay-test",
            [IndexRole::Ordinary, IndexRole::Occupation],
            [0, 1],
            vec![source],
        )
        .unwrap();
        let mut rule = system
            .seal_candidate(candidate(0), order(), domain(1))
            .unwrap();
        assert!(rule.candidate.rhs.is_empty());
        assert!(rule.nonzero_conditions.contains(&polynomial));
        system.replay_rule(&rule).unwrap();
        rule.nonzero_conditions.clear();
        assert!(system.replay_rule(&rule).is_err());
        let mut forged = candidate(0);
        forged.sources[0].seed.shifts[0] = 1;
        assert!(system.seal_candidate(forged, order(), domain(1)).is_err());
        let mut forged = candidate(0);
        forged.rhs.push(Term {
            integral: forged.target,
            coefficient: context.one(),
        });
        assert!(system.seal_candidate(forged, order(), domain(1)).is_err());
    }

    #[test]
    fn recentering_transports_original_source_domain_and_conditions() {
        let context = CoefficientContext::new(["n", "b"]);
        let condition = context.coefficient_fixture("n-1").numerator;
        let source = GuardedSource::new(
            "shifted-zero",
            vec![Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: context.one().numerator,
            }],
            domain(2),
        )
        .with_nonzero_conditions(vec![condition.clone()]);
        let system = GuardedSourceSystem::new(
            "recenter-test",
            [IndexRole::Ordinary, IndexRole::Occupation],
            [0, 1],
            vec![source],
        )
        .unwrap();
        // Discovery at n>=1 uses the original identity at n+1>=2. Its pivot
        // is I(n+1), so the final rule must restore the original n>=2 guard.
        let mut rule = system
            .seal_candidate(candidate(1), order(), domain(1))
            .unwrap();
        assert_eq!(rule.domain.bounds()[0].lower(), Some(2));
        assert!(!rule.domain.contains(&[1, 0]));
        assert!(rule.nonzero_conditions.contains(&condition));
        system.replay_rule(&rule).unwrap();
        rule.domain = domain(1);
        assert!(system.replay_rule(&rule).is_err());
    }
}
