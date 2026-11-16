//! Descent of a rule term whose symbols sit at OTHER slots than the target's (slot-permuted patterns, e.g. the
//! one-term symmetry aliases `I(k) = I(g k)`), decided EXACTLY on the record's whole domain (T178-UNBOXED item B).
//!
//! The engine's index-free comparison (`compare_mixed`) needs every symbol at its own slot: it cancels the common
//! symbolic part slot by slot. Under a slot permutation nothing cancels slot by slot, but on a domain where BOTH patterns
//! stay in the chart (every line slot `>= 1`, every numerator slot `<= 0`) the order program is a fixed sequence of
//! LINEAR forms in the key: the support is the same (lines active, numerators inactive), every pre-support row, degree
//! row and coordinate excess is linear (`excess = v - 1` on a line, `-v` on a numerator; `absolute = v` / `-v`). The
//! comparison is the sign of the first form whose difference (term minus target, reversed for a `Descending`
//! coordinate group) is not zero. Each sign is decided on the domain by the exact emptiness test
//! ([`crate::parametric::descent::emptiness`]); where a sign is mixed the domain is split into the three sub-regions
//! `< 0`, `> 0`, `= 0` (the last one continues with the next form). Two-sided (eps) axes are not handled (undecided).
//! Mirrors `rustred_order::CompiledOrder::compare` on concrete keys (differential tests: `parametric_permuted`).

use serde::Serialize;

use crate::domain::CaseDomain;
use crate::key::NodeFrame;
use crate::loci::contain::ProofBudget;
use crate::order::{NodeComparator, PatternSlot, parse_pattern};
use crate::parametric::descent::{EmptyState, emptiness};
use crate::poly::Poly;
use rustred_order::{CoordinateGroups, Direction};

/// One slot of a general pattern: a constant, or a symbol `n<var>` (any slot's symbol) plus an offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenSlot {
    Fixed(i64),
    Symbol { var: usize, offset: i64 },
}

/// Parse `I(1,n2,1,n5-1,...)` allowing a symbol at any position.
pub fn parse_general(text: &str) -> Result<Vec<GenSlot>, String> {
    let inner = text
        .trim()
        .strip_prefix("I(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("not an integral key: {text}"))?;
    let mut out = Vec::new();
    for p in inner.split(',') {
        let p = p.trim();
        if let Some(rest) = p.strip_prefix('n') {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let var: usize = digits
                .parse()
                .map_err(|_| format!("key {text}: bad symbol {p}"))?;
            let off = &rest[digits.len()..];
            let offset: i64 = if off.is_empty() {
                0
            } else {
                off.trim_start_matches('+')
                    .parse()
                    .map_err(|_| format!("key {text}: bad offset {p}"))?
            };
            out.push(GenSlot::Symbol { var, offset });
        } else {
            out.push(GenSlot::Fixed(
                p.parse()
                    .map_err(|_| format!("key {text}: bad value {p}"))?,
            ));
        }
    }
    Ok(out)
}

/// The verdict of [`permuted_descent`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum PermutedVerdict {
    /// The term is LOWER than the target at every key of the domain; `decided_by` = the order components that decide
    /// (one per sub-region).
    Lower {
        decided_by: Vec<String>,
    },
    /// The term is NOT lower at `witness` (a key of the domain); `lower_elsewhere` = the domain also has a part where it
    /// is lower.
    NotLower {
        witness: Vec<i32>,
        component: String,
        lower_elsewhere: bool,
    },
    Undecided {
        reason: String,
    },
}

fn lin(p: Result<Poly, crate::poly::PolyError>) -> Result<Poly, String> {
    p.map_err(|e| format!("{e:?}"))
}

/// Exact comparison of `term` with `target` (pattern texts) on the domain `dom` under `cmp`'s order program.
pub fn permuted_descent(
    cmp: &NodeComparator,
    frame: &NodeFrame,
    dom: &CaseDomain,
    target: &str,
    term: &str,
    budget: &ProofBudget,
) -> PermutedVerdict {
    match compare_inner(cmp, frame, dom, target, term, budget) {
        Ok(v) => v,
        Err(reason) => PermutedVerdict::Undecided { reason },
    }
}

fn compare_inner(
    cmp: &NodeComparator,
    frame: &NodeFrame,
    dom: &CaseDomain,
    target: &str,
    term: &str,
    budget: &ProofBudget,
) -> Result<PermutedVerdict, String> {
    let n = frame.arity();
    let t = parse_pattern(target)?;
    let u = parse_general(term)?;
    if t.len() != n || u.len() != n {
        return Err("pattern length differs from the arity".into());
    }
    let desc = cmp.equivalent_descriptor();
    if desc.two_sided.iter().any(|&b| b) {
        return Err("two-sided (eps) axes are not handled".into());
    }
    // slot values as linear polynomials in the KEY (the target value is the key: n_s with the target offset is k_s)
    let tv: Vec<Poly> = (0..n)
        .map(|s| match t[s] {
            PatternSlot::Symbolic(_) => Poly::var(s),
            PatternSlot::Fixed(c) => Poly::constant(i128::from(c)),
        })
        .collect();
    let toff = |a: usize| -> i64 {
        match t.get(a) {
            Some(PatternSlot::Symbolic(o)) => i64::from(*o),
            _ => 0,
        }
    };
    let mut uv: Vec<Poly> = Vec::with_capacity(n);
    for slot in &u {
        uv.push(match *slot {
            GenSlot::Fixed(c) => Poly::constant(i128::from(c)),
            GenSlot::Symbol { var, offset } => {
                if var >= n {
                    return Err(format!("symbol n{var} outside the arity"));
                }
                // the rule point coordinate pt[var] = k_var - (target offset at var); the term slot is pt[var] + offset
                lin(Poly::var(var).add(&Poly::constant(i128::from(offset - toff(var)))))?
            }
        });
    }
    // both patterns in the chart on the whole domain
    for s in 0..n {
        for (side, p) in [("target", &tv[s]), ("term", &uv[s])] {
            // line: p >= 1 everywhere <=> {p <= 0} empty; numerator: p <= 0 everywhere <=> {p >= 1} empty
            let out = if frame.is_line(s) {
                lin(p.neg())?
            } else {
                lin(p.sub(&Poly::constant(1)))?
            };
            match emptiness(dom, frame, budget, &[], &[out]) {
                (EmptyState::Empty, _, _) => {}
                (EmptyState::NonEmpty, w, _) => {
                    return Err(format!(
                        "the {side} leaves the chart at slot {s} on part of the domain (e.g. {w:?})"
                    ));
                }
                (EmptyState::Unknown, _, r) => {
                    return Err(format!(
                        "chart of the {side} at slot {s}: undecided ({r:?})"
                    ));
                }
            }
        }
    }
    let line = |s: usize| frame.is_line(s);
    // absolute and excess as linear forms (in the chart: lines active, numerators inactive)
    let absolute = |p: &Poly, s: usize| -> Result<Poly, String> {
        if line(s) { Ok(p.clone()) } else { lin(p.neg()) }
    };
    let excess = |p: &Poly, s: usize| -> Result<Poly, String> {
        if line(s) {
            lin(p.sub(&Poly::constant(1)))
        } else {
            lin(p.neg())
        }
    };
    let mut forms: Vec<(String, Poly)> = Vec::new();
    for (k, row) in desc.pre_support_degree_rows.iter().enumerate() {
        let mut l = Poly::zero();
        for s in 0..n {
            let w = if line(s) {
                row.active[s]
            } else {
                row.inactive[s]
            };
            if w == 0 {
                continue;
            }
            let d = lin(absolute(&uv[s], s)?.sub(&absolute(&tv[s], s)?))?;
            l = lin(l.add(&lin(d.mul(&Poly::constant(i128::from(w))))?))?;
        }
        forms.push((format!("pre-support row {k}"), l));
    }
    // support: identical (both in the chart)
    for (k, row) in desc.degree_rows.iter().enumerate() {
        let mut l = Poly::zero();
        for s in 0..n {
            let w = if line(s) {
                row.active[s]
            } else {
                row.inactive[s]
            };
            if w == 0 {
                continue;
            }
            let d = lin(excess(&uv[s], s)?.sub(&excess(&tv[s], s)?))?;
            l = lin(l.add(&lin(d.mul(&Poly::constant(i128::from(w))))?))?;
        }
        forms.push((format!("degree row {k}"), l));
    }
    let passes: Vec<Option<bool>> = match desc.coordinate_groups {
        CoordinateGroups::Interleaved => vec![None],
        CoordinateGroups::ActiveFirst => vec![Some(true), Some(false)],
        _ => vec![Some(false), Some(true)],
    };
    for pass in passes {
        for &s in &desc.coordinate_priority {
            let active = line(s);
            if pass.is_some_and(|p| p != active) {
                continue;
            }
            let mut d = lin(excess(&uv[s], s)?.sub(&excess(&tv[s], s)?))?;
            let dir = if active {
                desc.active_direction
            } else {
                desc.inactive_direction
            };
            if dir == Direction::Descending {
                d = lin(d.neg())?;
            }
            forms.push((format!("coordinate n{s}"), d));
        }
    }
    let mut decided_by: Vec<String> = Vec::new();
    let mut not_lower: Option<(Vec<i32>, String)> = None;
    let mut any_lower = false;
    decide(
        dom,
        frame,
        budget,
        &forms,
        0,
        &mut Vec::new(),
        0,
        &mut decided_by,
        &mut not_lower,
        &mut any_lower,
    )?;
    Ok(match not_lower {
        Some((witness, component)) => PermutedVerdict::NotLower {
            witness,
            component,
            lower_elsewhere: any_lower,
        },
        None => {
            decided_by.sort();
            decided_by.dedup();
            PermutedVerdict::Lower { decided_by }
        }
    })
}

/// Region = the domain with the extra equations `eqs`. Decide the forms from `i` on.
#[allow(clippy::too_many_arguments)]
fn decide(
    dom: &CaseDomain,
    frame: &NodeFrame,
    budget: &ProofBudget,
    forms: &[(String, Poly)],
    i: usize,
    eqs: &mut Vec<Poly>,
    depth: usize,
    decided_by: &mut Vec<String>,
    not_lower: &mut Option<(Vec<i32>, String)>,
    any_lower: &mut bool,
) -> Result<(), String> {
    // an empty region needs nothing
    match emptiness(dom, frame, budget, eqs, &[]) {
        (EmptyState::Empty, _, _) => return Ok(()),
        (EmptyState::Unknown, _, r) if depth > 0 => {
            return Err(format!("emptiness of a sub-region undecided: {r:?}"));
        }
        _ => {}
    }
    let Some((name, l)) = forms.get(i) else {
        // every form is zero on the region: the patterns are EQUAL there (not lower)
        if let (EmptyState::NonEmpty, Some(w), _) = emptiness(dom, frame, budget, eqs, &[]) {
            *not_lower = Some((w, "equal".into()));
            return Ok(());
        }
        return Err("all forms vanish on a region whose points could not be produced".into());
    };
    if l.is_zero() {
        return decide(
            dom,
            frame,
            budget,
            forms,
            i + 1,
            eqs,
            depth,
            decided_by,
            not_lower,
            any_lower,
        );
    }
    if depth > 8 {
        return Err("split depth exceeded".into());
    }
    // l < 0 everywhere on the region <=> {l >= 0} empty: term lower here
    let neg_everywhere = matches!(
        emptiness(dom, frame, budget, eqs, std::slice::from_ref(l)).0,
        EmptyState::Empty
    );
    if neg_everywhere {
        decided_by.push(name.clone());
        *any_lower = true;
        return Ok(());
    }
    // a key with l >= 1: term HIGHER there (the components before l vanish on the region)
    let l1 = lin(l.sub(&Poly::constant(1)))?;
    match emptiness(dom, frame, budget, eqs, &[l1]) {
        (EmptyState::NonEmpty, Some(w), _) => {
            if not_lower.is_none() {
                *not_lower = Some((w, name.clone()));
            }
        }
        (EmptyState::Empty, _, _) => {}
        (_, _, r) => return Err(format!("sign of {name} undecided: {r:?}")),
    }
    // the part with l <= -1: lower there
    let lm = lin(lin(l.neg())?.sub(&Poly::constant(1)))?;
    if !matches!(
        emptiness(dom, frame, budget, eqs, &[lm]).0,
        EmptyState::Empty
    ) {
        decided_by.push(name.clone());
        *any_lower = true;
    }
    // the part with l = 0: the next form decides
    eqs.push(l.clone());
    let r = decide(
        dom,
        frame,
        budget,
        forms,
        i + 1,
        eqs,
        depth + 1,
        decided_by,
        not_lower,
        any_lower,
    );
    eqs.pop();
    r
}
