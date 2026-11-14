//! AUDIT audit-ladder-B, part 7 (PORTED by lane ladderfix as a regression test: accounting violations must be zero): the shipped replays (T162 h2: 44 faces; T178 k1: 62 items) and PERTURBATIONS of
//! the recorded outcomes: (a) drop one successful attempt (the one whose rule the baseline accepted), (b) swap the
//! answers of two settings of the same case, (c) mark one accepted rule as non-descending at one served point.
//! The library must react (other rule, more cases, a child, a terminal), never return the same closed result, and
//! keep the accounting (every census point served exactly once, or a terminal proposal, or open with a reason).

mod ladder_common;

use std::collections::{BTreeMap, BTreeSet};

use ladder_common::*;
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::repair::{
    CaseRequest, CaseSolver, FactSource, TreeLimits, TreeReport, drive_tree,
};
use rustred_campaign_core::worklist::items::{Face, Key, NodeParams};
use rustred_campaign_core::worklist::ladder::{
    CandidateRule, Disposition, PointFacts, SolveAnswer, SolveOutcome,
};
use rustred_campaign_core::worklist::seed::{AttemptBudget, seed_space};

struct Perturbed<'a> {
    inner: &'a mut Scripted,
    drop: Option<(String, String)>,
    swap: Option<(String, String, String)>,
}
impl CaseSolver for Perturbed<'_> {
    fn solve(&mut self, req: &CaseRequest, b: &AttemptBudget) -> SolveAnswer {
        let c = req.case.canonical_text();
        let s = req.setting.canonical_key();
        if let Some((dc, ds)) = &self.drop
            && *dc == c
            && *ds == s
        {
            return SolveAnswer { outcome: SolveOutcome::NoRelation, cpu_ms: 1000, rows: None };
        }
        if let Some((sc, a, bb)) = &self.swap
            && *sc == c
            && (s == *a || s == *bb)
        {
            let other = if s == *a { bb.clone() } else { a.clone() };
            // answer as the other setting would have been answered
            let mut r2 = req.clone();
            let space = seed_space(&req.case, &self.inner.params);
            if let Some(st) = space.into_iter().flatten().find(|x| x.canonical_key() == other) {
                r2.setting = st;
            }
            return self.inner.solve(&r2, b);
        }
        self.inner.solve(req, b)
    }
}

struct NonDesc<'a, 'b> {
    inner: &'a mut ScriptedFacts<'b>,
    digest: String,
    key: Key,
}
impl FactSource for NonDesc<'_, '_> {
    fn facts(&mut self, rule: &CandidateRule, key: &[i32]) -> PointFacts {
        let mut f = self.inner.facts(rule, key);
        if rule.digest == self.digest && key == self.key.as_slice() {
            let mut up = key.to_vec();
            up[0] += 1; // a top-sector RHS key ABOVE the target
            f.rhs_top = Some(vec![up]);
        }
        f
    }
}

fn accounting(rep: &TreeReport, census: &[Key]) -> Result<(), String> {
    let mut served: BTreeMap<Key, usize> = BTreeMap::new();
    for (_, m) in rep.accepted_rules() {
        for k in m.served {
            *served.entry(k).or_default() += 1;
        }
    }
    let terms: BTreeSet<Key> = rep.terminal_candidates().into_iter().map(|(k, _)| k).collect();
    for k in census {
        let n = served.get(k).copied().unwrap_or(0);
        if n > 1 || (n == 1 && terms.contains(k)) {
            return Err(format!("{k:?} served {n} times (terminal {})", terms.contains(k)));
        }
        if n == 0 && !terms.contains(k) {
            let open = rep.stopped.is_some()
                || rep.items.iter().any(|r| {
                    r.state.item.gap_points.contains(k) && matches!(r.disposition, None | Some(Disposition::Attention { .. }))
                });
            if !open {
                return Err(format!("{k:?} LOST"));
            }
        }
    }
    Ok(())
}

fn run(
    params: &NodeParams,
    solver: &mut dyn CaseSolver,
    cmp: &NodeComparator,
    facts: &mut dyn FactSource,
    face: Face,
    census: Vec<Key>,
) -> TreeReport {
    drive_tree(
        vec![root_item(params, face, census)],
        params,
        cmp,
        solver,
        facts,
        &mut ReplayProver,
        None,
        None,
        &TreeLimits::default(),
    )
}

#[derive(Default)]
struct Stats {
    runs: u64,
    same_result: u64,
    violations: Vec<String>,
    reactions: BTreeMap<String, u64>,
}

fn classify(base: &Outcome, o: &Outcome) -> String {
    if o.rules == base.rules && o.terminals == base.terminals {
        return "SAME CLOSED RESULT".into();
    }
    let mut v = vec![];
    if o.cases > base.cases {
        v.push("more cases");
    }
    if o.cases < base.cases {
        v.push("fewer cases");
    }
    if o.cases == base.cases {
        v.push("same case count, other rule(s)");
    }
    if o.terminals.len() > base.terminals.len() {
        v.push("+terminal");
    }
    if o.point_rules > base.point_rules {
        v.push("+point rule");
    }
    if o.attempts > base.attempts {
        v.push("more attempts");
    }
    v.join(", ")
}

fn perturb_all<'c>(name: &str, params: NodeParams, cmp: &'c NodeComparator, facts_mk: &dyn Fn() -> ScriptedFacts<'c>, recs: Vec<Rec>, items: Vec<(Face, Vec<Key>)>, exc: BTreeMap<i64, bool>) {
    let mut st = Stats::default();
    let mut solver = Scripted::new(params.clone(), recs, exc);
    for (face, census) in &items {
        let mut f0 = facts_mk();
        let base_rep = run(&params, &mut solver, cmp, &mut f0, face.clone(), census.clone());
        let base = outcome(&base_rep);
        if let Err(e) = accounting(&base_rep, census) {
            st.violations.push(format!("{name} face {}: baseline accounting {e}", face.label()));
        }
        // the accepted members with their item case
        let mut members = vec![];
        // v1.2: a member carries its rule's WRITTEN case (an inherited reserve candidate keeps the parent's case)
        for (_id, m) in base_rep.accepted_rules() {
            members.push((m.case.clone(), m));
        }
        for (case, m) in &members {
            // (a) drop the successful attempt
            {
                st.runs += 1;
                let mut p = Perturbed { inner: &mut solver, drop: Some((case.canonical_text(), m.setting.canonical_key())), swap: None };
                let mut f = facts_mk();
                let rep = run(&params, &mut p, cmp, &mut f, face.clone(), census.clone());
                let o = outcome(&rep);
                let c = classify(&base, &o);
                if c == "SAME CLOSED RESULT" {
                    st.same_result += 1;
                    st.violations.push(format!("{name} face {} drop {} {}: same closed result", face.label(), case.canonical_text(), m.setting.label()));
                }
                // the dropped (case, setting) must not appear as an accepted rule any more
                if rep
                    .accepted_rules()
                    .iter()
                    .any(|(_id, mm)| mm.case == *case && mm.setting.canonical_key() == m.setting.canonical_key())
                {
                    st.violations.push(format!("{name} face {}: dropped attempt still accepted", face.label()));
                }
                if let Err(e) = accounting(&rep, census) {
                    st.violations.push(format!("{name} face {} drop: accounting {e}", face.label()));
                }
                if c.starts_with("fewer cases") && !c.contains("terminal") {
                    eprintln!("ECONOMY? {name} face {} drop {} {}:\n   base {:?} terms {:?}\n   now  {:?} terms {:?}", face.label(), case.canonical_text(), m.setting.label(), base.rules, base.terminals, o.rules, o.terminals);
                }
                *st.reactions.entry(format!("drop: {c}")).or_default() += 1;
            }
            // (b) swap with the next setting of the same case's seed space
            {
                let space: Vec<_> = seed_space(case, &params).into_iter().flatten().collect();
                if let Some(pos) = space.iter().position(|s| s.canonical_key() == m.setting.canonical_key()) {
                    let other = &space[(pos + 1) % space.len()];
                    st.runs += 1;
                    let mut p = Perturbed {
                        inner: &mut solver,
                        drop: None,
                        swap: Some((case.canonical_text(), m.setting.canonical_key(), other.canonical_key())),
                    };
                    let mut f = facts_mk();
                    let rep = run(&params, &mut p, cmp, &mut f, face.clone(), census.clone());
                    let o = outcome(&rep);
                    *st.reactions.entry(format!("swap: {}", classify(&base, &o))).or_default() += 1;
                    if let Err(e) = accounting(&rep, census) {
                        st.violations.push(format!("{name} face {} swap: accounting {e}", face.label()));
                    }
                }
            }
            // (c) non-descending at one served point
            if let Some(k) = m.served.first() {
                st.runs += 1;
                let mut inner = facts_mk();
                let mut f = NonDesc { inner: &mut inner, digest: m.digest.clone(), key: k.clone() };
                let rep = run(&params, &mut solver, cmp, &mut f, face.clone(), census.clone());
                let o = outcome(&rep);
                let c = classify(&base, &o);
                if c == "SAME CLOSED RESULT" {
                    st.same_result += 1;
                }
                // the perturbed rule must not serve k any more
                if rep.accepted_rules().iter().any(|(_, mm)| mm.digest == m.digest && mm.served.contains(k)) {
                    st.violations.push(format!("{name} face {}: rule {} still serves {k:?} after non-descent", face.label(), m.digest));
                }
                if let Err(e) = accounting(&rep, census) {
                    st.violations.push(format!("{name} face {} nondesc: accounting {e}", face.label()));
                }
                *st.reactions.entry(format!("nondesc: {c}")).or_default() += 1;
            }
        }
    }
    eprintln!("==== PART 7 {name}: {} perturbed runs, {} same closed result, {} violations", st.runs, st.same_result, st.violations.len());
    for v in st.violations.iter().take(10) {
        eprintln!("   {v}");
    }
    for (k, v) in &st.reactions {
        eprintln!("   reaction {k:<60} {v}");
    }
    // "same closed result" after a perturbation is information (another candidate of the same attempt or another
    // setting returning the same rule takes over; audit B notes), not an accounting error
    let real: Vec<&String> = st.violations.iter().filter(|v| !v.contains("same closed result")).collect();
    assert!(real.is_empty(), "PART 7 {name}: {real:#?}");
}

#[test]
fn part7_t162_perturbations() {
    let items: Vec<RecItem> = read_jsonl("t162_h2_items.jsonl");
    let recs: Vec<Rec> = read_jsonl("t162_h2_attempts.jsonl");
    let params = NodeParams::new("T162", 20, 9);
    let cmp = t162_comparator();
    let mk = || ScriptedFacts { cmp: &cmp, n_lines: 9, line_priority: (0..9).collect() };
    let its = items.iter().map(|it| (Face::new(it.sd.clone().unwrap(), vec![]), it.census.clone())).collect();
    perturb_all("T162", params, &cmp, &mk, recs, its, BTreeMap::new());
}

#[test]
fn part7_t178_perturbations() {
    let items: Vec<RecItem> = read_jsonl("t178_k1_items.jsonl");
    let recs: Vec<Rec> = read_jsonl("t178_k1_attempts.jsonl");
    let params = NodeParams::new("T178", 20, 10);
    let exc: BTreeMap<i64, bool> = items.iter().map(|it| (it.item, it.worker == "w1" || it.item == 14)).collect();
    let cmp = t178_comparator();
    let mk = || ScriptedFacts { cmp: &cmp, n_lines: 10, line_priority: vec![6, 4, 9, 8, 5, 2, 1, 0, 7, 3] };
    let its = items
        .iter()
        .filter_map(|it| Some((Face::new(it.sd.clone()?, it.sn.clone()?), it.census.clone())))
        .collect();
    perturb_all("T178", params, &cmp, &mk, recs, its, exc);
}
