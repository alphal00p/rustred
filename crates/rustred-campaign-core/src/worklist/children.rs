//! Children of an item (plan 2.4.4 L2, standard children, general affine children; R13 A5, R32, R34, R39; Astra
//! A s.3): every left-over gap point is assigned to exactly ONE child, by priority
//! 1. the conjunction of the EXCEPTION BRANCHES (and pole loci) READ FROM THE RULES of the cover that except the
//!    point (then from the other accepted candidates): slot = value at any value, diagonals, sum planes, general affine
//!    equations with non-unit coefficients; nonlinear literals are not parametrised;
//! 2. a STANDARD plane: a free dotted line at 2 or 3, a free numerator at -1 or -2 (R39);
//! 3. a FIT through >= 2 left-over points (fallback, marked `Derived::Fit`);
//! 4. a POINT item.
//!
//! A child is created only when its free dimension is strictly below the parent's (no cycle; depth <= the root's
//! free slots, D-35).

use std::collections::BTreeMap;

use super::affine::{AffineEq, fit_loci};
use super::items::{CaseSpec, ChildOrigin, Derived, Item, ItemKind, Key, NodeParams};
use super::ladder::CandidateRule;
use crate::domain::Verdict;

/// The affine literals of the exception branch (or pole locus) of `rule` that excepts `key`, if every literal of
/// that branch is affine. `None`: the rule does not except the key, or the branch is nonlinear.
pub fn excepting_branch(rule: &CandidateRule, key: &[i32]) -> Option<(String, Vec<AffineEq>)> {
    let d = &rule.domain;
    match d.applies(key) {
        Verdict::OnException { branch } => {
            let lits = d.exceptions.get(branch)?;
            let eqs: Option<Vec<AffineEq>> = lits.iter().map(AffineEq::from_poly).collect();
            eqs.map(|e| (format!("{}#exc{}", rule.digest, branch), e))
        }
        Verdict::OnPole { index, .. } => {
            let pole = d.poles.get(index)?;
            let eqs: Option<Vec<AffineEq>> = pole
                .coeffs
                .iter()
                .filter(|c| !c.is_zero())
                .map(AffineEq::from_poly)
                .collect();
            eqs.map(|e| (format!("{}#pole{}", rule.digest, index), e))
        }
        _ => None,
    }
}

fn child_item(
    parent: &Item,
    case: CaseSpec,
    pts: Vec<Key>,
    origin: ChildOrigin,
    params: &NodeParams,
) -> Item {
    let kind = if case.is_point() {
        ItemKind::Point
    } else {
        ItemKind::Child
    };
    let mut it = Item::new(
        &params.node,
        kind,
        case,
        pts,
        Some(parent.id.clone()),
        parent.depth + 1,
        parent.root_dim,
        origin,
        parent.box_id.clone(),
    );
    it.range = parent.range;
    it
}

/// Children for the left-over points (see the module documentation) and the ALTERNATIVE standard-plane children
/// (every other standard plane holding left-over points, run only for points the primaries leave). Deterministic:
/// groups in canonical case order.
pub(crate) fn derive_children(
    item: &Item,
    cover: &[&CandidateRule],
    reserve: &[&CandidateRule],
    others: &[&CandidateRule],
    leftover: &[Key],
    params: &NodeParams,
) -> (Vec<Item>, Vec<Item>) {
    let dim = item.case.dim();
    let mut groups: BTreeMap<String, (CaseSpec, Vec<Key>, ChildOrigin)> = BTreeMap::new();
    let add = |case: CaseSpec,
               p: Key,
               origin: ChildOrigin,
               groups: &mut BTreeMap<String, (CaseSpec, Vec<Key>, ChildOrigin)>| {
        let e = groups
            .entry(case.canonical_text())
            .or_insert((case, Vec::new(), origin));
        e.1.push(p);
    };
    let mut rest: Vec<Key> = Vec::new();
    for p in leftover {
        // 1. branches read from the rules (cover first, then the other accepted candidates)
        let mut placed = false;
        for pool in [cover, others] {
            let mut names = Vec::new();
            let mut eqs = Vec::new();
            for a in pool {
                if let Some((name, e)) = excepting_branch(a, p) {
                    names.push(name);
                    eqs.extend(e);
                }
            }
            if eqs.is_empty() {
                continue;
            }
            if let Ok(c) = item.case.constrained(&[], &eqs)
                && c.dim() < dim
                && c.contains(p, &params.lines)
            {
                add(
                    c,
                    p.clone(),
                    ChildOrigin::ExceptionBranch {
                        derived: Derived::FromRule,
                        branches: names,
                    },
                    &mut groups,
                );
                placed = true;
                break;
            }
        }
        if placed {
            continue;
        }
        // 2. standard planes (line at 2 or 3, numerator at -1 or -2)
        for s in item.case.free() {
            let vals = params.standard_values.for_slot(params.lines[s]);
            if vals.contains(&p[s])
                && let Ok(c) = item.case.constrained(&[(s, p[s])], &[])
                && c.dim() < dim
            {
                add(
                    c,
                    p.clone(),
                    ChildOrigin::Standard {
                        slot: s,
                        value: p[s],
                    },
                    &mut groups,
                );
                placed = true;
                break;
            }
        }
        if !placed {
            rest.push(p.clone());
        }
    }
    // 3. fit through >= 2 remaining points
    let free = item.case.free();
    let mut fitted = vec![false; rest.len()];
    for (eq, held) in fit_loci(&rest, &free) {
        if let Ok(c) = item.case.constrained(&[], std::slice::from_ref(&eq))
            && c.dim() < dim
        {
            for &i in &held {
                fitted[i] = true;
                add(
                    c.clone(),
                    rest[i].clone(),
                    ChildOrigin::ExceptionBranch {
                        derived: Derived::Fit,
                        branches: vec![format!("fit:{}", eq.to_engine())],
                    },
                    &mut groups,
                );
            }
        }
    }
    // 4. points
    for (i, p) in rest.iter().enumerate() {
        if fitted[i] {
            continue;
        }
        let fx: Vec<(usize, i32)> = free.iter().map(|&s| (s, p[s])).collect();
        if let Ok(c) = item.case.constrained(&fx, &[])
            && c.dim() < dim
        {
            add(c, p.clone(), ChildOrigin::PointOf, &mut groups);
        }
    }
    let mut out: Vec<Item> = groups
        .into_values()
        .map(|(c, pts, origin)| child_item(item, c, pts, origin, params))
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    // alternatives: (a) the conjunction of the branches of the cover AND reserve rules that except the point (the
    // intersection of the loci the pruned candidates leave, e.g. a diagonal where two centres' planes meet), then (b)
    // every standard plane (default values from the node's spec) holding left-over points; never the point's own
    // primary case
    let mut alt: BTreeMap<String, (CaseSpec, Vec<Key>, ChildOrigin)> = BTreeMap::new();
    if !reserve.is_empty() {
        for p in leftover {
            let mut names = Vec::new();
            let mut eqs = Vec::new();
            for a in cover.iter().chain(reserve) {
                if let Some((name, e)) = excepting_branch(a, p) {
                    names.push(name);
                    eqs.extend(e);
                }
            }
            if eqs.is_empty() {
                continue;
            }
            if let Ok(c) = item.case.constrained(&[], &eqs)
                && c.dim() < dim
                && !out.iter().any(|o| o.case == c && o.gap_points.contains(p))
            {
                alt.entry(c.canonical_text())
                    .or_insert((
                        c,
                        Vec::new(),
                        ChildOrigin::ExceptionBranch {
                            derived: Derived::FromRule,
                            branches: names,
                        },
                    ))
                    .1
                    .push(p.clone());
            }
        }
    }
    for p in leftover {
        for s in item.case.free() {
            let vals = params.standard_values.for_slot(params.lines[s]);
            if !vals.contains(&p[s]) {
                continue;
            }
            let Ok(c) = item.case.constrained(&[(s, p[s])], &[]) else {
                continue;
            };
            if c.dim() >= dim || out.iter().any(|o| o.case == c && o.gap_points.contains(p)) {
                continue;
            }
            alt.entry(c.canonical_text())
                .or_insert((
                    c,
                    Vec::new(),
                    ChildOrigin::Standard {
                        slot: s,
                        value: p[s],
                    },
                ))
                .1
                .push(p.clone());
        }
    }
    let alternatives: Vec<Item> = alt
        .into_values()
        .map(|(c, pts, origin)| child_item(item, c, pts, origin, params))
        .collect();
    (out, alternatives)
}

/// Gate of a standard child created with its parent in state `blocked: parent_pending` (2.4.4): after the parent's
/// disposition, it is released iff one of the parent's left-over points was assigned to a child with the same case
/// (same id), else closed `covered_by_parent`; a child released early (parallel run) whose plane the parent ends up
/// serving is `Superseded` (its rule is not kept: economy, R31 (b)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChildGate {
    Blocked,
    Release,
    CoveredByParent,
    Superseded,
}

pub fn standard_child_gate(
    child: &Item,
    parent_children: Option<&[Item]>,
    released_early: bool,
) -> ChildGate {
    match parent_children {
        None => ChildGate::Blocked,
        Some(ch) if ch.iter().any(|c| c.id == child.id) => ChildGate::Release,
        Some(_) if released_early => ChildGate::Superseded,
        Some(_) => ChildGate::CoveredByParent,
    }
}
