//! Acceptance of a change of seed list (a drop, a support restriction, a union): the spec-given LEXICOGRAPHIC
//! objective with regularity INSIDE the test (TRICKS 15 iii; pole-diagnostics.md addenda 7 / 8), the guard
//! `monotone_rhs`, and lane tadic's system judge when available. Pure.

use std::cmp::Ordering;

use super::policy::SeedMinPolicy;
use super::{Key, Offset, PoleOrder, RelationFacts, SystemPoles, add};

/// Comparison of the pole order at one key: `Some(ordering of new vs old)` or `None` = undecided.
fn pole_cmp(new: PoleOrder, old: PoleOrder) -> Option<Ordering> {
    match (new, old) {
        (PoleOrder::Order { order: a }, PoleOrder::Order { order: b }) => Some(a.cmp(&b)),
        // regular can never be worse than anything
        (PoleOrder::Order { order: 0 }, PoleOrder::Singular | PoleOrder::Undecided) => {
            Some(Ordering::Less)
        }
        // a singular relation (order unknown) is worse than a regular one
        (PoleOrder::Singular, PoleOrder::Order { order: 0 }) => Some(Ordering::Greater),
        _ => None,
    }
}

/// The verdict on a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub keep: bool,
    pub reason: String,
}

/// One state of a key: its relation under a list.
pub struct State<'a> {
    pub key: &'a Key,
    pub facts: &'a RelationFacts,
}

fn terms(states: &[State<'_>], agg: &str) -> u64 {
    let it = states.iter().map(|s| s.facts.rhs_count);
    if agg == "sum" {
        it.sum()
    } else {
        it.max().unwrap_or(0)
    }
}

/// Judge the change from `old` (list `before`) to `new` (list `after`) on the same keys (in the same order). Every
/// key of `new` must carry a relation (the caller refuses a change that loses a key before calling).
#[allow(clippy::too_many_arguments)]
pub fn judge_change(
    old: &[State<'_>],
    new: &[State<'_>],
    before: &[Offset],
    after: &[Offset],
    centre: &[i32],
    policy: &SeedMinPolicy,
    system: &mut dyn SystemPoles,
) -> Verdict {
    let refuse = |r: String| Verdict {
        keep: false,
        reason: r,
    };
    let undecided_keep = policy.on_undecided_regularity == "drop";
    // POLES_OBJECTIVE.md: the pole part is a PREFERENCE (default on); `off` = plain size minimisation
    let poles_on = policy.pole_objective != "off";
    let mut decided: Option<(bool, String)> = None;
    for m in &policy.objective {
        let ord = match m.as_str() {
            "pole_order" if !poles_on => Ordering::Equal,
            "pole_order" => {
                // per-key Pareto: worse at one key = worse; better at one, worse at none = better
                let mut any_better = false;
                let mut undecided = 0usize;
                let mut worse: Option<String> = None;
                for (o, n) in old.iter().zip(new) {
                    match pole_cmp(n.facts.pole, o.facts.pole) {
                        Some(Ordering::Greater) => {
                            worse = Some(format!(
                                "pole order worse at {:?}: {:?} -> {:?}",
                                n.key,
                                o.facts.pole.order(),
                                n.facts.pole.order()
                            ));
                            break;
                        }
                        Some(Ordering::Less) => any_better = true,
                        Some(Ordering::Equal) => {}
                        None => undecided += 1,
                    }
                }
                if let Some(w) = worse {
                    return refuse(w);
                }
                if undecided > 0 && !undecided_keep {
                    return refuse(format!(
                        "pole order undecided at {undecided} key(s) (on_undecided_regularity keep_seed)"
                    ));
                }
                if any_better && undecided == 0 {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            }
            "seeds" => after.len().cmp(&before.len()),
            "terms" => {
                let a = terms(new, &policy.terms_aggregate);
                let b = terms(old, &policy.terms_aggregate);
                a.cmp(&b)
            }
            _ => Ordering::Equal,
        };
        match ord {
            Ordering::Less => {
                decided = Some((true, format!("better in {m}")));
                break;
            }
            Ordering::Greater => {
                decided = Some((false, format!("worse in {m}")));
                break;
            }
            Ordering::Equal => {}
        }
    }
    let (keep, reason) = decided.unwrap_or((true, "equal in every measure".into()));
    if !keep {
        return refuse(reason);
    }
    if policy.monotone_rhs {
        let a = terms(new, &policy.terms_aggregate);
        let b = terms(old, &policy.terms_aggregate);
        if a > b {
            return refuse(format!("monotone_rhs: terms {b} -> {a}"));
        }
    }
    if poles_on && policy.system_regularity == "on" && system.available() {
        for n in new {
            let b: Vec<Offset> = before.iter().map(|o| add(centre, o)).collect();
            let a: Vec<Offset> = after.iter().map(|o| add(centre, o)).collect();
            match system.keeps_regularity(&b, &a, n.key, &policy.d0) {
                Some(true) => {}
                Some(false) => {
                    return refuse(format!(
                        "system pole order p* worse at {:?} ({})",
                        n.key,
                        system.name()
                    ));
                }
                None if !undecided_keep => {
                    return refuse(format!(
                        "system regularity undecided at {:?} (on_undecided_regularity keep_seed)",
                        n.key
                    ));
                }
                None => {}
            }
        }
    }
    Verdict { keep, reason }
}

#[cfg(test)]
mod tests {
    use super::super::StubSystemPoles;
    use super::*;

    fn facts(rhs: u64, pole: PoleOrder) -> RelationFacts {
        RelationFacts {
            relation_digest: "0".repeat(64),
            rhs_count: rhs,
            rhs_sub_count: None,
            bytes: None,
            trace_rows: None,
            seeds_used: None,
            pole,
            minimise: None,
        }
    }

    #[test]
    fn pole_order_comes_first_and_undecided_follows_the_policy() {
        let k: Key = vec![1, 2];
        let p = SeedMinPolicy::default();
        let mut s = StubSystemPoles;
        let reg = facts(10, PoleOrder::regular());
        let sing = facts(5, PoleOrder::Order { order: 1 });
        let und = facts(5, PoleOrder::Undecided);
        let (b, a) = (vec![vec![0, 0], vec![1, 0]], vec![vec![0, 0]]);
        let st = |f| [State { key: &k, facts: f }];
        // USER RULING 2026-11-19 (rule 21): by DEFAULT the terms come first, the pole order only breaks ties, and an
        // undecided regularity does not stop a drop: regular -> singular with fewer terms is KEPT
        let v = judge_change(&st(&reg), &st(&sing), &b, &a, &[0, 0], &p, &mut s);
        assert!(v.keep, "{}", v.reason);
        assert!(judge_change(&st(&reg), &st(&und), &b, &a, &[0, 0], &p, &mut s).keep);
        // N1 of the gate R-C read: equal terms and seeds: the pole order breaks the tie (regular -> singular refused)
        let sing_same = facts(10, PoleOrder::Order { order: 1 });
        let v = judge_change(&st(&reg), &st(&sing_same), &b, &b, &[0, 0], &p, &mut s);
        assert!(!v.keep, "{}", v.reason);
        // the EXPLICIT withdrawn order (pole order first, keep_seed): the rest of this test
        let mut o = p.clone();
        o.objective = vec!["pole_order".into(), "seeds".into(), "terms".into()];
        o.on_undecided_regularity = "keep_seed".into();
        // regular -> singular (smaller): refused (pole order first)
        let v = judge_change(&st(&reg), &st(&sing), &b, &a, &[0, 0], &o, &mut s);
        assert!(!v.keep, "{}", v.reason);
        // singular -> regular (larger): kept by pole order, but the monotone_rhs guard refuses more terms
        let v = judge_change(&st(&sing), &st(&reg), &b, &a, &[0, 0], &o, &mut s);
        assert!(!v.keep && v.reason.contains("monotone_rhs"), "{}", v.reason);
        let mut q = o.clone();
        q.monotone_rhs = false;
        assert!(judge_change(&st(&sing), &st(&reg), &b, &a, &[0, 0], &q, &mut s).keep);
        // undecided: keep_seed refuses, drop keeps
        assert!(!judge_change(&st(&reg), &st(&und), &b, &a, &[0, 0], &o, &mut s).keep);
        q.on_undecided_regularity = "drop".into();
        assert!(judge_change(&st(&reg), &st(&und), &b, &a, &[0, 0], &q, &mut s).keep);
        // equal pole order: fewer seeds wins
        let reg2 = facts(9, PoleOrder::regular());
        assert!(judge_change(&st(&reg), &st(&reg2), &b, &a, &[0, 0], &o, &mut s).keep);
        // objective terms first: more terms refused even with fewer seeds
        let mut t = p.clone();
        t.objective = vec!["terms".into(), "seeds".into()];
        t.monotone_rhs = false;
        let big = facts(11, PoleOrder::regular());
        assert!(!judge_change(&st(&reg), &st(&big), &b, &a, &[0, 0], &t, &mut s).keep);
    }
}
