//! PORT of audit loop-B's model-based differential harness (WS/notes/implementer.audit-loop-B.md; generators, naive
//! reference closure and comparison from implementer-scratch/audit-loop-B/.../examples/audit_b, bin_v16), adapted to the
//! semantics fixed by lane loopfix (closure.md section 11; see tests/closure_model/reference.rs). Variants per random
//! node: Tier K, Tier K + credits, Tier K + terminals, Tier C with truthful facts, Tier C with adversarial in-memory
//! facts (25 % missing coefficients / 25 % unknown regularity / Zero->NonZero flips / all unknown), Tier C + credits;
//! every comparison of the audit (keys, rounds, edges, non-descending edges, defect with blocking rule, used set with
//! counts, exits, terminals, credits incl. validity / first element / no chaining, cycles, chain, unrepresentable,
//! verdict, term counts, undecided counts, requests cover every needed fact) plus: Tier C `closed` only if the
//! reference with TRUTHFUL facts is closed. Default suite: a small seeded run; the audit's 100,000-node run:
//! `CLOSURE_MODEL_NODES=100000 light_build.sh <lane> test -p rustred-campaign-core --release --test closure_model_reference -- --ignored`.

#[path = "closure_model/facts.rs"]
mod facts;
#[path = "closure_model/reference.rs"]
mod reference;
#[path = "closure_model/spec.rs"]
mod spec;

use facts::{Adv, Truth};
use reference::{RefCfg, RefOut};
use rustred_campaign_core::closure::{
    Closure, ClosureConfig, HandDowns, NoFacts, TerminalSets, Tier, closure, write_fact_requests,
};
use rustred_campaign_core::symmetry::CreditPolicy;
use spec::{GenOpts, NodeSpec, Rng, build, gen_node, group_elems, keyset};
use std::collections::{BTreeMap, BTreeSet};

fn ccfg(sp: &NodeSpec, tier: Tier, credits: bool, terminals: bool, threads: usize) -> ClosureConfig {
    ClosureConfig {
        tier,
        threads,
        hand_downs: HandDowns::Keys,
        witness_cap: 1 << 30,
        longest_chain: true,
        // the generator's numerator maps are pure permutations by construction: certified
        credits: credits.then(|| {
            let mut p = CreditPolicy::new(group_elems(sp));
            p.pure_numerator_maps = (0..sp.group.len()).collect();
            p
        }),
        terminals: terminals.then(|| TerminalSets {
            preferred: Some(keyset(sp.n, &sp.term_pref)),
            other: Some(keyset(sp.n, &sp.term_other)),
        }),
        fact_tables: sp.tables.iter().map(|t| (t.name.clone(), t.name.clone())).collect(),
        table_digests: sp.tables.iter().map(|t| (t.name.clone(), format!("test:{}", t.name))).collect(),
        ..Default::default()
    }
}

fn vstr<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).unwrap().as_str().map(|s| s.to_string()).unwrap_or_else(|| format!("{:?}", serde_json::to_value(v).unwrap()))
}

/// Compare a crate closure with the reference. Returns the list of disagreements ("category: detail").
fn compare(sp: &NodeSpec, c: &Closure, r: &RefOut, credits: bool) -> Vec<String> {
    let mut d = vec![];
    let s = c.summary();
    let ck: BTreeSet<Vec<i32>> = c.keys().iter().collect();
    if ck != r.keys {
        let miss: Vec<_> = r.keys.difference(&ck).take(3).collect();
        let extra: Vec<_> = ck.difference(&r.keys).take(3).collect();
        d.push(format!("keys: crate {} ref {} missing {:?} extra {:?}", ck.len(), r.keys.len(), miss, extra));
    }
    if s.rounds != r.rounds {
        d.push(format!("rounds: crate {} ref {}", s.rounds, r.rounds));
    }
    if s.edges != r.edges {
        d.push(format!("edges: crate {} ref {}", s.edges, r.edges));
    }
    let cw: BTreeSet<(Vec<i32>, String, usize, usize, Vec<i32>)> = c
        .descent_witnesses()
        .into_iter()
        .map(|w| (w.key, w.rule.table, w.rule.record, w.term, w.rhs))
        .collect();
    if cw != r.nondesc {
        let miss: Vec<_> = r.nondesc.difference(&cw).take(2).collect();
        let extra: Vec<_> = cw.difference(&r.nondesc).take(2).collect();
        d.push(format!("nondesc: crate {} ref {} missing {:?} extra {:?}", cw.len(), r.nondesc.len(), miss, extra));
    }
    if s.descent_violations != r.nondesc.len() as u64 {
        d.push(format!("violations_count: crate {} ref {}", s.descent_violations, r.nondesc.len()));
    }
    let co: BTreeMap<Vec<i32>, (String, Option<(String, usize)>)> = c
        .open_keys()
        .into_iter()
        .map(|o| (o.key, (vstr(&o.status), o.rule.map(|x| (x.table, x.record)))))
        .collect();
    if co != r.open {
        let a: Vec<_> = r.open.iter().filter(|(k, v)| co.get(*k) != Some(v)).take(3).collect();
        let b: Vec<_> = co.iter().filter(|(k, v)| r.open.get(*k) != Some(v)).take(3).collect();
        d.push(format!("open: crate {} ref {} ref-only {:?} crate-only {:?}", co.len(), r.open.len(), a, b));
    }
    let cu: BTreeMap<(String, usize), (u64, u64)> =
        c.used_rules().into_iter().map(|u| ((u.rule.table, u.rule.record), (u.fires, u.fires_in_start))).collect();
    if cu != r.used {
        let a: Vec<_> = r.used.iter().filter(|(k, v)| cu.get(*k) != Some(v)).take(3).collect();
        let b: Vec<_> = cu.iter().filter(|(k, v)| r.used.get(*k) != Some(v)).take(3).collect();
        d.push(format!("used: ref-only {:?} crate-only {:?}", a, b));
    }
    let ch: BTreeMap<u64, BTreeSet<Vec<i32>>> =
        c.hand_down_sets().iter().map(|(m, ks)| (*m, ks.iter().collect())).filter(|(_, s): &(u64, BTreeSet<_>)| !s.is_empty()).collect();
    if ch != r.hand {
        d.push(format!(
            "exits: crate masks {:?} ref masks {:?}",
            ch.iter().map(|(m, s)| (*m, s.len())).collect::<Vec<_>>(),
            r.hand.iter().map(|(m, s)| (*m, s.len())).collect::<Vec<_>>()
        ));
    }
    let ct: BTreeMap<Vec<i32>, String> = c.terminal_records().into_iter().map(|t| (t.key, vstr(&t.kind))).collect();
    if ct != r.terminals {
        d.push(format!("terminals: crate {:?} ref {:?}", ct, r.terminals));
    }
    if credits {
        let cc = c.credits();
        let ckeys: BTreeSet<Vec<i32>> = cc.iter().map(|x| x.key.clone()).collect();
        let rkeys: BTreeSet<Vec<i32>> = r.credits.keys().cloned().collect();
        if ckeys != rkeys {
            d.push(format!(
                "credit_set: crate-only {:?} ref-only {:?}",
                ckeys.difference(&rkeys).take(3).collect::<Vec<_>>(),
                rkeys.difference(&ckeys).take(3).collect::<Vec<_>>()
            ));
        }
        for x in &cc {
            match r.credit_valid.get(&x.key) {
                Some(v) if v.contains(&x.element) => {
                    let g = &sp.group[x.element];
                    if reference::apply_g(sp, g, &x.key).as_ref() != Some(&x.image) {
                        d.push(format!("credit_image: {:?}", x));
                    }
                    match reference::dispatch(sp, &x.image) {
                        reference::RD::Covered { t, r: rr, .. } => {
                            if (sp.tables[t].name.clone(), rr) != (x.rule.table.clone(), x.rule.record) {
                                d.push(format!("credit_rule: {:?}", x));
                            }
                        }
                        _ => d.push(format!("credit_rule_uncovered_image: {:?}", x)),
                    }
                    if r.credits[&x.key].0 != x.element {
                        d.push(format!("credit_element_not_first: {:?} ref first {}", x.key, r.credits[&x.key].0));
                    }
                }
                Some(_) => d.push(format!("credit_invalid_element: {:?}", x)),
                None => {}
            }
        }
    }
    let ccy = s.cycles.map(|x| x > 0);
    if ccy.is_some() && ccy != Some(r.has_cycle) {
        d.push(format!("cycles: crate {:?} ref {}", s.cycles, r.has_cycle));
    }
    if r.has_cycle && s.verdict != rustred_campaign_core::closure::Verdict::Failed {
        d.push(format!("cycle_not_failed: verdict {:?}", s.verdict));
    }
    if let (Some(ch), Some(l)) = (&s.longest_chain, r.longest)
        && !r.has_cycle
        && ch.edges != l
    {
        d.push(format!("longest: crate {} ref {}", ch.edges, l));
    }
    if s.unrepresentable != r.unrep {
        d.push(format!("unrepresentable: crate {} ref {}", s.unrepresentable, r.unrep));
    }
    let v = vstr(&s.verdict);
    if v != r.verdict {
        d.push(format!("verdict: crate {} ref {}", v, r.verdict));
    }
    let t = &s.terms;
    let mine = (r.top_terms, r.pn_top, r.pn_sub, r.sub_terms, r.requires_zero, r.dropped_zero, r.pos_nonzero);
    let theirs = (t.top, t.positive_numerator_top, t.positive_numerator_sub, t.subsector, s.requires_coefficient_zero, t.dropped_zero, s.positive_numerator_nonzero);
    if mine != theirs {
        d.push(format!("terms: (top,pn_top,pn_sub,sub,req0,dropped,pos_nz) crate {:?} ref {:?}", theirs, mine));
    }
    let u = &s.undecided;
    if (u.coef_unknown_positive_numerator, u.coef_unknown_not_lower, u.regularity_unknown) != (r.und_pn, r.und_nl, r.und_reg) {
        d.push(format!(
            "undecided: crate {:?} ref {:?}",
            (u.coef_unknown_positive_numerator, u.coef_unknown_not_lower, u.regularity_unknown),
            (r.und_pn, r.und_nl, r.und_reg)
        ));
    }
    d
}

/// Requests written by the crate (through write_fact_requests) must contain every fact the reference needs.
fn check_requests(c: &Closure, r: &RefOut) -> Vec<String> {
    let mut buf = vec![];
    write_fact_requests(&c.fact_requests(), &mut buf).unwrap();
    let mut got = BTreeSet::new();
    for l in String::from_utf8(buf).unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(l).unwrap();
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        got.insert((
            v["table"].as_str().unwrap().to_string(),
            v["record"].as_u64().unwrap() as usize,
            v["label"].as_i64().unwrap(),
            key,
            v["kind"].as_str().unwrap().to_string(),
            v["term"].as_u64().map(|x| x as usize),
        ));
    }
    let missing: Vec<_> = r.needed.difference(&got).take(3).collect();
    if missing.is_empty() { vec![] } else { vec![format!("requests_missing: {} e.g. {:?}", r.needed.difference(&got).count(), missing)] }
}

/// One variant run: crate closure + reference; returns the disagreements.
fn variant(sp: &NodeSpec, var: &str) -> Result<Vec<String>, String> {
    let b = build(sp)?;
    let credits = var.contains("cred");
    let terminals = var.contains("term");
    let tier_c = var.starts_with('C');
    let tier = if tier_c { Tier::C } else { Tier::K };
    let cfg = ccfg(sp, tier, credits, terminals, 1);
    let mode: u32 = var.split(':').nth(1).map(|x| x.parse().unwrap()).unwrap_or(0);
    let adv = Adv { truth: Truth::new(sp), mode };
    let c = match closure(&b.disp, &b.bodies, &b.cmp, &b.start, &cfg, if tier_c { &adv } else { &NoFacts }) {
        Ok(c) => c,
        Err(e) => return Ok(vec![format!("crate_error: {e:?}")]),
    };
    let r = reference::run(sp, &b.cmp, &RefCfg { tier_c, credits, terminals, require_regularity: true, facts: Some(&adv) });
    let mut d = compare(sp, &c, &r, credits);
    if tier_c {
        d.extend(check_requests(&c, &r));
        if c.summary().verdict == rustred_campaign_core::closure::Verdict::Closed {
            let t = Adv { truth: Truth::new(sp), mode: 0 };
            let rt = reference::run(sp, &b.cmp, &RefCfg { tier_c, credits, terminals, require_regularity: true, facts: Some(&t) });
            if rt.verdict != "closed" {
                d.push(format!("closed_but_truth_says: {} (variant {var})", rt.verdict));
            }
        }
    }
    Ok(d)
}

fn diff(seed: u64, count: u64) -> BTreeMap<String, (u64, u64)> {
    let threads = 4u64;
    let stats = std::sync::Mutex::new(BTreeMap::<String, (u64, u64)>::new());
    let fails = std::sync::Mutex::new(Vec::<String>::new());
    std::thread::scope(|s| {
        for tid in 0..threads {
            let (stats, fails) = (&stats, &fails);
            s.spawn(move || {
                let mut i = tid;
                while i < count {
                    let mut rng = Rng(seed.wrapping_mul(1_000_003).wrapping_add(i));
                    let asc = [0u64, 0, 2, 6][rng.below(4) as usize];
                    let o = GenOpts { sparse: rng.chance(35), asc_rate: asc, group: rng.chance(40), terminals: rng.chance(15) };
                    let sp = gen_node(&mut rng, &o);
                    let mut vars: Vec<String> = vec!["K".into()];
                    if !sp.group.is_empty() {
                        vars.push("Kcred".into());
                    }
                    if !sp.term_pref.is_empty() || !sp.term_other.is_empty() {
                        vars.push("Kterm".into());
                    }
                    vars.push("C:0".into());
                    vars.push(format!("C:{}", [1, 2, 3, 5][rng.below(4) as usize]));
                    if !sp.group.is_empty() {
                        vars.push("Ccred:0".into());
                        vars.push(format!("Ccred:{}", [1, 2, 3][rng.below(3) as usize]));
                    }
                    for v in vars {
                        let res = variant(&sp, &v);
                        let mut st = stats.lock().unwrap();
                        let e = st.entry(v.clone()).or_default();
                        e.0 += 1;
                        match res {
                            Err(err) => {
                                st.entry(format!("build_error {err}")).or_default().0 += 1;
                            }
                            Ok(d) if !d.is_empty() => {
                                e.1 += 1;
                                let mut f = fails.lock().unwrap();
                                if f.len() < 5 {
                                    f.push(format!("node {i} variant {v}: {:?}
  spec {}", d, serde_json::to_string(&sp).unwrap()));
                                }
                            }
                            Ok(_) => {}
                        }
                    }
                    i += threads;
                }
            });
        }
    });
    let f = fails.into_inner().unwrap();
    assert!(f.is_empty(), "disagreements with the reference:
{}", f.join("
"));
    stats.into_inner().unwrap()
}

#[test]
fn model_reference_small() {
    let st = diff(20261114, 400);
    println!("closure vs naive reference (400 nodes): {st:?}");
}

#[test]
#[ignore = "audit loop-B scale (100,000 nodes by default): run in release with --ignored"]
fn model_reference_full() {
    let n: u64 = std::env::var("CLOSURE_MODEL_NODES").ok().and_then(|x| x.parse().ok()).unwrap_or(100_000);
    let st = diff(20261114, n);
    println!("closure vs naive reference ({n} nodes): {st:?}");
}
