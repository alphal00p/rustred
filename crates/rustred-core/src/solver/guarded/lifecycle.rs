//! Concrete application of source-replayed guarded rules.
//!
//! Explicit terminals are caller-selected stopping points, never a master count
//! or a statement of independence. No vacuum preparation runs on this path.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::solver::instantiate::instantiate_polynomial;
use crate::solver::{Integral, Seed, SolverError};

use super::{GuardedRule, GuardedSourceSystem, IndexRole};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardedApplicationFailure {
    InvalidOccupation { axis: usize },
    NoApplicableRule,
    ConditionVanished { rule: usize, condition: usize },
    CoefficientPole { rule: usize, term: usize },
    NonDescending { rule: usize },
    UnsupportedPower,
    WorkLimit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardedApplicationStatus {
    Applied { rule: usize },
    Terminal,
    Zero,
    Unresolved(GuardedApplicationFailure),
}

#[derive(Clone, Debug)]
pub struct GuardedApplication<const N: usize> {
    pub status: GuardedApplicationStatus,
    pub terms: BTreeMap<[i64; N], Coefficient>,
    /// Remaining conditions on physical parameters after index specialization.
    pub nonzero_conditions: Vec<CoefficientPolynomial>,
}

#[derive(Clone, Copy, Debug)]
pub struct GuardedReductionLimits {
    pub max_rule_applications: usize,
    /// Maximum distinct integrals queued for recursive application. Excess
    /// weighted RHS terms are returned directly as unresolved frontier terms.
    pub max_pending_integrals: usize,
}

impl Default for GuardedReductionLimits {
    fn default() -> Self {
        Self {
            max_rule_applications: 100_000,
            max_pending_integrals: 100_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GuardedUnresolvedTerm<const N: usize> {
    pub integral: [i64; N],
    pub coefficient: Coefficient,
    pub reason: GuardedApplicationFailure,
}

#[derive(Clone, Debug)]
pub struct GuardedReduction<const N: usize> {
    pub terms: BTreeMap<[i64; N], Coefficient>,
    pub unresolved: Vec<GuardedUnresolvedTerm<N>>,
    pub nonzero_conditions: Vec<CoefficientPolynomial>,
    pub rule_applications: usize,
}

/// Reusable rules bound to their exact supplied source corpus and measure.
#[derive(Debug)]
pub struct GuardedProgram<const N: usize> {
    pub(super) sources: Arc<GuardedSourceSystem<N>>,
    pub(super) rules: Vec<GuardedRule<N>>,
    pub(super) terminals: BTreeSet<[i64; N]>,
}

impl<const N: usize> GuardedProgram<N> {
    pub fn new(
        sources: Arc<GuardedSourceSystem<N>>,
        rules: Vec<GuardedRule<N>>,
        terminals: impl IntoIterator<Item = [i64; N]>,
    ) -> Result<Self, SolverError> {
        let permutation = rules.first().and_then(|rule| rule.order.permutation());
        for rule in &rules {
            if rule.order.program().is_some() || rule.order.permutation() != permutation {
                return Err(SolverError::InvalidInput(
                    "guarded program rules require one common coordinate order".into(),
                ));
            }
            sources.replay_rule(rule)?;
        }
        let terminals: BTreeSet<_> = terminals.into_iter().collect();
        for terminal in &terminals {
            if sources.roles.iter().zip(terminal).any(|(role, power)| {
                (*role == IndexRole::Occupation && *power < 0)
                    || (*role == IndexRole::RequiredCut && *power <= 0)
            }) || sources
                .zero_domains
                .iter()
                .any(|domain| domain.contains(terminal))
            {
                return Err(SolverError::InvalidInput(
                    "guarded terminal is undefined or declared zero".into(),
                ));
            }
        }
        Ok(Self {
            sources,
            rules,
            terminals,
        })
    }

    pub fn sources(&self) -> &Arc<GuardedSourceSystem<N>> {
        &self.sources
    }
    pub fn rules(&self) -> &[GuardedRule<N>] {
        &self.rules
    }
    pub fn terminals(&self) -> &BTreeSet<[i64; N]> {
        &self.terminals
    }

    fn one(&self) -> Coefficient {
        self.sources
            .system
            .rows()
            .iter()
            .flatten()
            .next()
            .expect("source construction rejects an empty corpus")
            .coefficient
            .one()
            .into()
    }

    fn is_zero(&self, target: &[i64; N]) -> bool {
        self.sources
            .roles
            .iter()
            .zip(target)
            .any(|(role, power)| *role == IndexRole::RequiredCut && *power <= 0)
            || self
                .sources
                .zero_domains
                .iter()
                .any(|domain| domain.contains(target))
    }

    /// Apply the first valid rule at a concrete integer point. Coefficient
    /// poles and missing coverage remain explicit; no residual is made zero.
    pub fn apply(&self, target: &[i64; N]) -> Result<GuardedApplication<N>, SolverError> {
        let mut unresolved = GuardedApplicationFailure::NoApplicableRule;
        let empty = |status| GuardedApplication {
            status,
            terms: BTreeMap::new(),
            nonzero_conditions: Vec::new(),
        };
        if let Some(axis) = self
            .sources
            .roles
            .iter()
            .zip(target)
            .position(|(role, value)| *role == IndexRole::Occupation && *value < 0)
        {
            return Ok(empty(GuardedApplicationStatus::Unresolved(
                GuardedApplicationFailure::InvalidOccupation { axis },
            )));
        }
        if self.is_zero(target) {
            return Ok(empty(GuardedApplicationStatus::Zero));
        }
        if self.terminals.contains(target) {
            return Ok(GuardedApplication {
                status: GuardedApplicationStatus::Terminal,
                terms: BTreeMap::from([(*target, self.one())]),
                nonzero_conditions: Vec::new(),
            });
        }
        'rules: for (ordinal, rule) in self.rules.iter().enumerate() {
            if !rule.domain.contains(target) {
                continue;
            }
            let mut values = [0_i16; N];
            for axis in 0..N {
                let power = rule.candidate.target[axis];
                if !power.is_symbolic() && target[axis] != i64::from(power.value()) {
                    continue 'rules;
                }
                let value = if power.is_symbolic() {
                    target[axis].checked_sub(i64::from(power.value()))
                } else {
                    Some(target[axis])
                };
                let Some(value) = value.and_then(|value| i16::try_from(value).ok()) else {
                    unresolved = GuardedApplicationFailure::UnsupportedPower;
                    continue 'rules;
                };
                values[axis] = value;
            }
            let Ok(integral) = Integral::numeric(values) else {
                unresolved = GuardedApplicationFailure::UnsupportedPower;
                continue;
            };
            let seed = Seed {
                integral,
                shifts: [0; N],
            };
            let specialize = |polynomial: &CoefficientPolynomial| {
                instantiate_polynomial(
                    polynomial,
                    &seed,
                    self.sources.system.index_variables(),
                    &[None; N],
                    None,
                )
            };
            let mut conditions = Vec::new();
            for (condition, polynomial) in rule.nonzero_conditions.iter().enumerate() {
                let value = specialize(polynomial)?;
                if value.is_zero() {
                    unresolved = GuardedApplicationFailure::ConditionVanished {
                        rule: ordinal,
                        condition,
                    };
                    continue 'rules;
                }
                retain_condition(&mut conditions, value.numerator);
            }
            let mut terms = BTreeMap::new();
            for (term_index, term) in rule.candidate.rhs.iter().enumerate() {
                let numerator = specialize(&term.coefficient.numerator)?;
                let denominator = specialize(&term.coefficient.denominator)?;
                if denominator.is_zero() {
                    unresolved = GuardedApplicationFailure::CoefficientPole {
                        rule: ordinal,
                        term: term_index,
                    };
                    continue 'rules;
                }
                retain_condition(&mut conditions, denominator.numerator.clone());
                let coefficient = &numerator / &denominator;
                if coefficient.is_zero() {
                    continue;
                }
                let child = std::array::from_fn(|axis| {
                    let power = term.integral[axis];
                    if power.is_symbolic() {
                        i64::from(values[axis]) + i64::from(power.value())
                    } else {
                        i64::from(power.value())
                    }
                });
                if self
                    .sources
                    .roles
                    .iter()
                    .zip(child)
                    .any(|(role, power)| *role == IndexRole::Occupation && power < 0)
                {
                    unresolved = GuardedApplicationFailure::NoApplicableRule;
                    continue 'rules;
                }
                if self.is_zero(&child) {
                    continue;
                }
                let concrete = |point: &[i64; N]| -> Option<Integral<N>> {
                    let mut powers = [0; N];
                    for (power, value) in powers.iter_mut().zip(point) {
                        *power = i16::try_from(*value).ok()?;
                    }
                    Integral::numeric(powers).ok()
                };
                let (Some(parent), Some(successor)) = (concrete(target), concrete(&child)) else {
                    unresolved = GuardedApplicationFailure::UnsupportedPower;
                    continue 'rules;
                };
                if rule.order.compare(&parent, &successor) != Ordering::Less {
                    unresolved = GuardedApplicationFailure::NonDescending { rule: ordinal };
                    continue 'rules;
                }
                accumulate(&mut terms, child, coefficient);
            }
            return Ok(GuardedApplication {
                status: GuardedApplicationStatus::Applied { rule: ordinal },
                terms,
                nonzero_conditions: conditions,
            });
        }
        Ok(empty(GuardedApplicationStatus::Unresolved(unresolved)))
    }

    /// Bounded recursive application. Any unfinished frontier is returned with
    /// its exact coefficient and reason, alongside declared terminal terms.
    pub fn reduce(
        &self,
        target: [i64; N],
        limits: GuardedReductionLimits,
    ) -> Result<GuardedReduction<N>, SolverError> {
        let mut result = GuardedReduction {
            terms: BTreeMap::new(),
            unresolved: Vec::new(),
            nonzero_conditions: Vec::new(),
            rule_applications: 0,
        };
        if limits.max_pending_integrals == 0 {
            result.unresolved.push(GuardedUnresolvedTerm {
                integral: target,
                coefficient: self.one(),
                reason: GuardedApplicationFailure::WorkLimit,
            });
            return Ok(result);
        }
        let mut pending = BTreeMap::from([(target, self.one())]);
        while let Some((integral, coefficient)) = pending.pop_first() {
            if result.rule_applications >= limits.max_rule_applications
                && !self.terminals.contains(&integral)
                && !self.is_zero(&integral)
            {
                result.unresolved.push(GuardedUnresolvedTerm {
                    integral,
                    coefficient,
                    reason: GuardedApplicationFailure::WorkLimit,
                });
                result
                    .unresolved
                    .extend(pending.into_iter().map(|(integral, coefficient)| {
                        GuardedUnresolvedTerm {
                            integral,
                            coefficient,
                            reason: GuardedApplicationFailure::WorkLimit,
                        }
                    }));
                break;
            }
            let applied = self.apply(&integral)?;
            match applied.status {
                GuardedApplicationStatus::Zero => {}
                GuardedApplicationStatus::Terminal => {
                    accumulate(&mut result.terms, integral, coefficient)
                }
                GuardedApplicationStatus::Unresolved(reason) => {
                    result.unresolved.push(GuardedUnresolvedTerm {
                        integral,
                        coefficient,
                        reason,
                    })
                }
                GuardedApplicationStatus::Applied { .. } => {
                    result.rule_applications += 1;
                    for condition in applied.nonzero_conditions {
                        retain_condition(&mut result.nonzero_conditions, condition);
                    }
                    for (child, value) in applied.terms {
                        let weighted = &coefficient * &value;
                        if weighted.is_zero() {
                            continue;
                        }
                        if pending.contains_key(&child)
                            || pending.len() < limits.max_pending_integrals
                        {
                            // Coalescing an existing key cannot grow the
                            // frontier and may free a slot by cancellation.
                            accumulate(&mut pending, child, weighted);
                        } else {
                            result.unresolved.push(GuardedUnresolvedTerm {
                                integral: child,
                                coefficient: weighted,
                                reason: GuardedApplicationFailure::WorkLimit,
                            });
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

fn retain_condition(conditions: &mut Vec<CoefficientPolynomial>, value: CoefficientPolynomial) {
    if !value.is_constant() && !conditions.contains(&value) {
        conditions.push(value);
    }
}

fn accumulate<const N: usize>(
    terms: &mut BTreeMap<[i64; N], Coefficient>,
    key: [i64; N],
    value: Coefficient,
) {
    if value.is_zero() {
        return;
    }
    match terms.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(value);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let sum = entry.get() + &value;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::{GuardedSource, IndexBounds, IndexDomain};
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{Case, IntegralOrder, RuleCandidate, SearchStats, SeedSource, Term};

    pub(in crate::solver::guarded) fn sample(measure: &str) -> GuardedProgram<1> {
        let context = CoefficientContext::new(["guarded_lifecycle_n", "guarded_lifecycle_x"]);
        let x = context.parameter("guarded_lifecycle_x").unwrap();
        let domain = IndexDomain::new([IndexBounds::new(Some(1), None).unwrap()]).unwrap();
        let target = Integral::symbolic([0]).unwrap();
        let successor = Integral::symbolic([-1]).unwrap();
        let source = GuardedSource::new(
            "surface-recurrence",
            vec![
                Term {
                    integral: target,
                    coefficient: context.one().numerator,
                },
                Term {
                    integral: successor,
                    coefficient: (-x.clone()).numerator,
                },
            ],
            domain.clone(),
        )
        .with_nonzero_conditions(vec![x.numerator.clone()]);
        let sources = Arc::new(
            GuardedSourceSystem::new(measure, [IndexRole::Occupation], [0], vec![source]).unwrap(),
        );
        let rule = GuardedRule {
            candidate: RuleCandidate {
                case: Case::generic(),
                target,
                rhs: vec![Term {
                    integral: successor,
                    coefficient: x.clone(),
                }],
                sources: vec![SeedSource {
                    basis_row: 0,
                    seed: Seed {
                        integral: target,
                        shifts: [0],
                    },
                }],
                stats: SearchStats::default(),
            },
            domain: domain.clone(),
            discovery_domain: domain,
            nonzero_conditions: vec![x.numerator],
            order: IntegralOrder::new([true], [false])
                .with_roles([IndexRole::Occupation])
                .unwrap(),
        };
        GuardedProgram::new(sources, vec![rule], [[0]]).unwrap()
    }

    #[test]
    fn occupation_zero_is_a_terminal_and_reduction_preserves_conditions() {
        let program = sample("theta-preserved");
        assert_eq!(
            program.apply(&[0]).unwrap().status,
            GuardedApplicationStatus::Terminal
        );
        let result = program.reduce([3], Default::default()).unwrap();
        let x = &program.rules[0].candidate.rhs[0].coefficient;
        assert_eq!(result.terms[&[0]], &(x * x) * x);
        assert!(result.unresolved.is_empty());
        assert_eq!(result.rule_applications, 3);
        assert_eq!(result.nonzero_conditions.len(), 1);
        assert!(matches!(
            program.apply(&[-1]).unwrap().status,
            GuardedApplicationStatus::Unresolved(GuardedApplicationFailure::InvalidOccupation {
                axis: 0
            })
        ));
    }

    #[test]
    fn bounded_reduction_retains_the_exact_unresolved_frontier() {
        let program = sample("bounded-frontier");
        let result = program
            .reduce(
                [3],
                GuardedReductionLimits {
                    max_rule_applications: 1,
                    max_pending_integrals: 10,
                },
            )
            .unwrap();
        assert!(result.terms.is_empty());
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.unresolved[0].integral, [2]);
        assert_eq!(
            result.unresolved[0].coefficient,
            program.rules[0].candidate.rhs[0].coefficient
        );
        assert_eq!(
            result.unresolved[0].reason,
            GuardedApplicationFailure::WorkLimit
        );
        let complete = program
            .reduce(
                [1],
                GuardedReductionLimits {
                    max_rule_applications: 1,
                    max_pending_integrals: 10,
                },
            )
            .unwrap();
        assert!(complete.unresolved.is_empty());
        assert_eq!(complete.terms.len(), 1);
    }

    #[test]
    fn unsupported_powers_and_missing_rules_are_explicit() {
        let program = sample("application-limits");
        assert_eq!(
            program.apply(&[64]).unwrap().status,
            GuardedApplicationStatus::Unresolved(GuardedApplicationFailure::UnsupportedPower)
        );
        let empty = GuardedProgram::new(program.sources, Vec::new(), []).unwrap();
        assert_eq!(
            empty.apply(&[0]).unwrap().status,
            GuardedApplicationStatus::Unresolved(GuardedApplicationFailure::NoApplicableRule)
        );
    }

    #[test]
    fn zero_rhs_still_checks_inherited_source_conditions() {
        let context = CoefficientContext::new(["guarded_zero_n"]);
        let condition = (context.parameter("guarded_zero_n").unwrap() - context.one()).numerator;
        let domain = IndexDomain::new([IndexBounds::new(Some(1), None).unwrap()]).unwrap();
        let target = Integral::symbolic([0]).unwrap();
        let source = GuardedSource::new(
            "conditional-zero",
            vec![Term {
                integral: target,
                coefficient: context.one().numerator,
            }],
            domain.clone(),
        )
        .with_nonzero_conditions(vec![condition.clone()]);
        let sources = Arc::new(
            GuardedSourceSystem::new(
                "conditional-zero-measure",
                [IndexRole::Occupation],
                [0],
                vec![source],
            )
            .unwrap(),
        );
        let rule = GuardedRule {
            candidate: RuleCandidate {
                case: Case::generic(),
                target,
                rhs: Vec::new(),
                sources: vec![SeedSource {
                    basis_row: 0,
                    seed: Seed {
                        integral: target,
                        shifts: [0],
                    },
                }],
                stats: SearchStats::default(),
            },
            domain: domain.clone(),
            discovery_domain: domain,
            nonzero_conditions: vec![condition],
            order: IntegralOrder::new([true], [false])
                .with_roles([IndexRole::Occupation])
                .unwrap(),
        };
        let program = GuardedProgram::new(sources, vec![rule], []).unwrap();
        assert!(matches!(
            program.apply(&[1]).unwrap().status,
            GuardedApplicationStatus::Unresolved(
                GuardedApplicationFailure::ConditionVanished { .. }
            )
        ));
        let result = program.apply(&[2]).unwrap();
        assert!(matches!(
            result.status,
            GuardedApplicationStatus::Applied { .. }
        ));
        assert!(result.terms.is_empty());
    }

    #[test]
    fn zero_rhs_retains_the_replayed_physical_pivot_condition() {
        let context = CoefficientContext::new(["guarded_pivot_n", "guarded_pivot_x"]);
        let pivot = context.parameter("guarded_pivot_x").unwrap().numerator;
        let domain = IndexDomain::new([IndexBounds::new(Some(1), None).unwrap()]).unwrap();
        let target = Integral::symbolic([0]).unwrap();
        let source = GuardedSource::new(
            "pivot-zero",
            vec![Term {
                integral: target,
                coefficient: pivot.clone(),
            }],
            domain.clone(),
        );
        let sources = Arc::new(
            GuardedSourceSystem::new(
                "pivot-zero-measure",
                [IndexRole::Occupation],
                [0],
                vec![source],
            )
            .unwrap(),
        );
        let rule = sources
            .seal_candidate(
                RuleCandidate {
                    case: Case::generic(),
                    target,
                    rhs: Vec::new(),
                    sources: vec![SeedSource {
                        basis_row: 0,
                        seed: Seed {
                            integral: target,
                            shifts: [0],
                        },
                    }],
                    stats: SearchStats::default(),
                },
                IntegralOrder::new([true], [false])
                    .with_roles([IndexRole::Occupation])
                    .unwrap(),
                domain,
            )
            .unwrap();
        let program = GuardedProgram::new(sources, vec![rule], []).unwrap();
        let applied = program.apply(&[2]).unwrap();
        assert!(applied.terms.is_empty());
        assert_eq!(applied.nonzero_conditions, vec![pivot]);
    }

    #[test]
    fn pending_cap_stages_rhs_without_losing_weighted_frontier_terms() {
        let context = CoefficientContext::new(["guarded_pending_n"]);
        let domain = IndexDomain::new([IndexBounds::new(Some(4), None).unwrap()]).unwrap();
        let target = Integral::symbolic([0]).unwrap();
        let mut row = vec![Term {
            integral: target,
            coefficient: context.one().numerator,
        }];
        let mut rhs = Vec::new();
        for shift in 1..=3_i16 {
            let integral = Integral::symbolic([-shift]).unwrap();
            row.push(Term {
                integral,
                coefficient: context.integer(-i64::from(shift)).numerator,
            });
            rhs.push(Term {
                integral,
                coefficient: context.integer(i64::from(shift)),
            });
        }
        let sources = Arc::new(
            GuardedSourceSystem::new(
                "pending-cap-measure",
                [IndexRole::Occupation],
                [0],
                vec![GuardedSource::new("three-successors", row, domain.clone())],
            )
            .unwrap(),
        );
        let rule = sources
            .seal_candidate(
                RuleCandidate {
                    case: Case::generic(),
                    target,
                    rhs,
                    sources: vec![SeedSource {
                        basis_row: 0,
                        seed: Seed {
                            integral: target,
                            shifts: [0],
                        },
                    }],
                    stats: SearchStats::default(),
                },
                IntegralOrder::new([true], [false])
                    .with_roles([IndexRole::Occupation])
                    .unwrap(),
                domain,
            )
            .unwrap();
        let program = GuardedProgram::new(sources, vec![rule], [[1], [2], [3]]).unwrap();
        let expected = program.apply(&[4]).unwrap().terms;
        let result = program
            .reduce(
                [4],
                GuardedReductionLimits {
                    max_rule_applications: 1,
                    max_pending_integrals: 1,
                },
            )
            .unwrap();
        // The first successor fits and is actually visited; later successors
        // are retained without ever enlarging the pending queue past one.
        assert_eq!(result.terms, BTreeMap::from([([1], context.integer(3))]));
        assert_eq!(result.unresolved.len(), 2);
        let mut recovered = result.terms;
        for term in result.unresolved {
            assert_eq!(term.reason, GuardedApplicationFailure::WorkLimit);
            accumulate(&mut recovered, term.integral, term.coefficient);
        }
        assert_eq!(recovered, expected);
        let zero_budget = program
            .reduce(
                [4],
                GuardedReductionLimits {
                    max_rule_applications: 1,
                    max_pending_integrals: 0,
                },
            )
            .unwrap();
        assert_eq!(zero_budget.rule_applications, 0);
        assert_eq!(zero_budget.unresolved.len(), 1);
        assert_eq!(zero_budget.unresolved[0].integral, [4]);
        assert_eq!(zero_budget.unresolved[0].coefficient, context.one());
    }
}
