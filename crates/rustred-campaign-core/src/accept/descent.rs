//! d3 descent of a candidate at a key, with the CLOSURE's instantiation code (rule point, offsets, term classes:
//! `closure::bodies`) and the node comparator (full comparator with tie-breaks, R29 (b)); positive-numerator and
//! non-lower terms are followed only if their coefficient is verified ZERO at the key (R9 (d), R13 A2).
//! Also the symbolic pattern check of a FULL node's scope (`NodeComparator::cmp_pattern`, the engine's
//! `compare_mixed`).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use super::facts::CoefficientFacts;
use super::rule::RuleText;
use crate::closure::bodies::{RuleBodies, Sectors, TermClass};
use crate::closure::facts::CoefFact;
use crate::cover::{Dispatcher, Table};
use crate::order::{NodeComparator, PatternSlot, parse_pattern};

/// The candidate alone in a one-record dispatcher, with its body: the closure's `RuleBodies::instantiate` applies
/// the rule's point map and the pattern offsets exactly as the closure does.
pub struct Instantiator {
    disp: Dispatcher,
    bodies: RuleBodies,
    arity: usize,
}

impl Instantiator {
    pub fn new(rule: &RuleText) -> Result<Instantiator, String> {
        let dom = rule.domain.clone().ok_or("candidate without a domain")?;
        let arity = dom.arity;
        let mut t = Table::new("candidate", arity);
        t.push(rule.index().unwrap_or(0), dom)
            .map_err(|e| e.to_string())?;
        let disp = Dispatcher::new(vec![t], vec![0]).map_err(|e| e.to_string())?;
        let mut bodies = RuleBodies::new(&disp);
        let pats: Vec<&str> = rule.rhs.iter().map(|(k, _)| k.as_str()).collect();
        bodies
            .set(&disp, 0, 0, &pats)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Instantiator {
            disp,
            bodies,
            arity,
        })
    }

    /// The RHS keys at `key` with their constant-slot masks.
    pub fn terms(&self, key: &[i32]) -> Result<Vec<(Vec<i32>, u64)>, String> {
        let mut out = Vec::new();
        let mut cm = Vec::new();
        let r = self.disp.rule_ref(0, 0);
        let n = self
            .bodies
            .instantiate(&self.disp, r, key, &mut out, &mut cm)
            .map_err(|e| format!("{e:?}"))?
            .ok_or("no body")?;
        Ok((0..n)
            .map(|j| (out[j * self.arity..(j + 1) * self.arity].to_vec(), cm[j]))
            .collect())
    }
}

/// Why a key fails descent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescentFailure {
    /// A top-sector term not lower than the key whose coefficient is NOT zero.
    NotLower { term: usize, rhs: Vec<i32> },
    /// A positive-numerator term whose coefficient is NOT zero.
    PositiveNumeratorNonZero { term: usize, rhs: Vec<i32> },
}

/// d3 at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescentAt {
    Pass {
        /// terms dropped because their coefficient is verified zero
        dropped_zero: usize,
        /// hand-downs (subsector / zero sector)
        hand_downs: usize,
    },
    Fail(DescentFailure),
    Unknown {
        term: usize,
        rhs: Vec<i32>,
        why: String,
    },
}

/// d3 of `rule` at `key`.
pub fn descent_at(
    inst: &Instantiator,
    sectors: &Sectors,
    cmp: &NodeComparator,
    facts: &mut dyn CoefficientFacts,
    rule: &RuleText,
    key: &[i32],
) -> DescentAt {
    let terms = match inst.terms(key) {
        Ok(t) => t,
        Err(e) => {
            return DescentAt::Unknown {
                term: usize::MAX,
                rhs: vec![],
                why: e,
            };
        }
    };
    let mut dropped_zero = 0;
    let mut hand_downs = 0;
    for (j, (t, cm)) in terms.into_iter().enumerate() {
        match sectors.classify_masked(&t, cm) {
            TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => hand_downs += 1,
            TermClass::Top => match cmp.try_cmp(&t, key) {
                Ok(Ordering::Less) => {}
                Ok(_) => match facts.coefficient(rule, key, j) {
                    CoefFact::Zero => dropped_zero += 1,
                    CoefFact::NonZero => {
                        return DescentAt::Fail(DescentFailure::NotLower { term: j, rhs: t });
                    }
                    CoefFact::Unknown => {
                        return DescentAt::Unknown {
                            term: j,
                            rhs: t,
                            why: "not lower, coefficient unknown".into(),
                        };
                    }
                },
                Err(e) => {
                    return DescentAt::Unknown {
                        term: j,
                        rhs: t,
                        why: format!("comparator: {e:?}"),
                    };
                }
            },
            TermClass::PositiveNumerator { .. } => match facts.coefficient(rule, key, j) {
                CoefFact::Zero => dropped_zero += 1,
                CoefFact::NonZero => {
                    return DescentAt::Fail(DescentFailure::PositiveNumeratorNonZero {
                        term: j,
                        rhs: t,
                    });
                }
                CoefFact::Unknown => {
                    return DescentAt::Unknown {
                        term: j,
                        rhs: t,
                        why: "positive numerator, coefficient unknown".into(),
                    };
                }
            },
        }
    }
    DescentAt::Pass {
        dropped_zero,
        hand_downs,
    }
}

/// Symbolic descent of every RHS pattern against the target pattern (`cmp_pattern` on the node mask): the terms
/// that are NOT proved lower (an incomparable pattern, a mismatch of symbolic slots, or not `Less`). Terms whose
/// pattern lies in a subsector for every value (a line slot fixed at <= 0) are hand-downs and not compared.
pub fn symbolic_not_proved(
    rule: &RuleText,
    cmp: &NodeComparator,
    lines: &[bool],
) -> BTreeSet<usize> {
    let mut bad = BTreeSet::new();
    let target = match rule.record.target.as_deref().map(parse_pattern) {
        Some(Ok(t)) => t,
        _ => return (0..rule.rhs.len()).collect(),
    };
    for (j, (k, _)) in rule.rhs.iter().enumerate() {
        let p = match parse_pattern(k) {
            Ok(p) => p,
            Err(_) => {
                bad.insert(j);
                continue;
            }
        };
        let hand_down = p
            .iter()
            .zip(lines)
            .any(|(s, &line)| line && matches!(s, PatternSlot::Fixed(v) if *v <= 0));
        if hand_down {
            continue;
        }
        if cmp.cmp_pattern(lines, &p, &target) != Ok(Ordering::Less) {
            bad.insert(j);
        }
    }
    bad
}
