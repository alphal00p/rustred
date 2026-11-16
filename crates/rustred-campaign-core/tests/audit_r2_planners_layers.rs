//! AUDIT (audit-r2-planners, notes/implementer.audit-r2-planners.md): lane layers' LEAF SETS (closure/leaves.rs,
//! import check with leaves) tried against "a frozen layer's `closed` is invariant under later layers and a layered
//! closure equals the single-pass closure".
//!
//! Tests ASSERT THE BEHAVIOUR FOUND. A test whose name ends in `_defect` documents a finding: the fixer inverts its
//! `DEFECT:` assertion with the fix. Pure (no process); toy tables (arity 3 / 4) and SEEDED random tables; nothing
//! five-loop specific (L8).

use std::collections::BTreeMap;
use std::sync::Arc;

use rustred_campaign_core::closure::{
    ClosureConfig, ClosureError, CoefFact, FactSource, HandDowns, KeyCodec, KeySet, LeafBinding,
    LeafProducer, LeafSet, LeafSetBuilder, RegFact, RuleBodies, RuleId, TerminalSets, Tier,
    Verdict, closure,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::CaseDomain;
use rustred_campaign_core::import::check::check_streamed_with_leaves;
use rustred_campaign_core::import::{
    CheckOptions, Evidence, ImportedSet, Level, TableSetSpec, load,
};
use rustred_campaign_core::order::NodeComparator;
use serde_json::{Value, json};
use std::path::Path;

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

fn key_text(k: &[i32; 3]) -> String {
    format!("I({},{},{})", k[0], k[1], k[2])
}

/// Chain [3,1,0] -> [2,1,0] -> [1,1,0] -> sub-sector keys only: closed from any start, every term non-zero.
fn rules() -> Vec<(i64, [i32; 3], Vec<[i32; 3]>)> {
    vec![
        (0, [3, 1, 0], vec![[2, 1, 0], [3, 0, 0]]),
        (1, [2, 1, 0], vec![[1, 1, 0], [2, 0, 0]]),
        (2, [1, 1, 0], vec![[1, 0, 0], [0, 1, 0]]),
    ]
}

/// One spec over the chain with `required_evidence` = `required` (everything else identical).
fn material(dir: &Path, required: &str) -> (ImportedSet, Evidence) {
    std::fs::create_dir_all(dir).unwrap();
    let mut t = format!("{HEADER}\n");
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    let mut facts = String::new();
    for (i, k, rhs) in rules() {
        let l = json!({"type":"rule","chart":"110","index":i,"fixed":k,"equations":[],"target":key_text(&k),
            "rhs": rhs.iter().map(|r| json!([key_text(r), "1"])).collect::<Vec<_>>(),
            "exceptions":[],"dispatch":"Partition","witness_loci":[]})
        .to_string();
        t.push_str(&l);
        t.push('\n');
        let r: Vec<Value> = rhs
            .iter()
            .enumerate()
            .map(|(j, x)| json!({"term": j, "key": x, "coef": "nonzero"}))
            .collect();
        facts.push_str(
            &json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(l.as_bytes()),
                "rule": i, "key": k, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
                "irregular_terms": [], "hidden_loci": [], "rhs": r, "zero_test": "exact", "error": null})
            .to_string(),
        );
        facts.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = dir.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    std::fs::write(dir.join("facts.jsonl"), &facts).unwrap();
    std::fs::write(dir.join("start.jsonl"), "[3,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence": required,"policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(facts.as_bytes()), "family": "TOY"}]}});
    let spec = TableSetSpec::from_json(&spec.to_string()).unwrap();
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    (set, ev)
}

fn opts() -> CheckOptions {
    CheckOptions {
        threads: 2,
        tier: Some(Tier::C),
        max_rounds: None,
    }
}

/// The certification rule of the closing loop's leaf-set writer (campaign `closeloop/layers.rs::row_json_certified`):
/// a served row is certified iff its evidence is at least the PRODUCER's required level (and no fact is unknown).
fn producer_certified(v: &Value, required: Level) -> bool {
    match v["status"].as_str() {
        Some("terminal") => true,
        Some("served") => {
            serde_json::from_value::<Level>(v["evidence"].clone())
                .ok()
                .is_some_and(|e| e >= required)
                && v["facts"]["regularity_unknown"] != true
                && v["facts"]["coefficient_unknown"] != true
        }
        _ => false,
    }
}

/// The leaf set of a checked layer, as `closeloop/layers.rs::leafset_of` builds it (same words, same binding).
fn leafset_like_layers(
    set: &ImportedSet,
    res: &rustred_campaign_core::import::CheckResult,
    rows: &Path,
    required: Level,
) -> LeafSet {
    let mut b = LeafSetBuilder::new(set.codec, vec![], vec![]);
    for l in std::fs::read_to_string(rows).unwrap().lines() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).unwrap();
        let key: Vec<i32> = serde_json::from_value(v["key"].clone()).unwrap();
        let level = match v["status"].as_str() {
            Some("served") => v["evidence"].as_str().unwrap_or("none").to_string(),
            Some("terminal") => "terminal".into(),
            _ => "open".into(),
        };
        b.push(&key, "table", &level, producer_certified(&v, required))
            .unwrap();
    }
    for im in &res.credit_images {
        b.aux(im).unwrap();
    }
    b.finish(LeafBinding {
        statement_digest: res
            .streamed
            .as_ref()
            .map(|s| s.sha256.clone())
            .unwrap_or_default(),
        source_verdict: "closed".into(),
        node: "toy".into(),
        clause: "layer-0".into(),
        start_digest: "toy".into(),
        walk: res.leaf_walk.clone(),
        relative_to: vec![],
        provenance: BTreeMap::new(),
        // planfix (L1, format 2): the producer's basis, as closeloop/layers.rs::leafset_of records it
        producer: Some(LeafProducer::new(
            required.word(),
            res.tier,
            !res.summary.diagnostic && res.tier == Tier::C,
        )),
    })
    .unwrap()
}

/// FINDING L1 (MAJOR; brief 1, "a credit level imported from a leaf set that is higher than what was certified
/// there"): a leaf set records each key's evidence LEVEL word but its binding carries neither the producer's required
/// level nor its tier (`closure/leaves.rs:27-46`: "tier ... never enter"), and the consumer checks only the
/// `certified` flag (`closure/mod.rs:3262-3281`, `import/check.rs` leaf rows). A run that requires `per-key` closes
/// "relative to" a leaf set whose keys are certified only at `imported` (another run with required_evidence
/// imported). The single walk of the same start under `per-key` is NOT closed (below_required_evidence). The closing
/// loop would label it `closed, <target>, per-key, relative to leaf set(s) ...` (`closeloop/pass.rs:2417-2422`).
#[test]
fn a_per_key_run_closes_relative_to_a_leaf_set_certified_only_at_imported_defect_fixed() {
    let root = std::env::temp_dir().join(format!("audit-r2-layers-l1-{}", std::process::id()));
    let (set_i, ev_i) = material(&root.join("imported"), "imported");
    let (set_p, ev_p) = material(&root.join("per-key"), "per-key");
    let ks = |s: &ImportedSet, keys: &[[i32; 3]]| {
        KeySet::from_keys(s.codec, keys.iter().map(|k| k.as_slice())).unwrap()
    };
    // producer: layer 0 = {[2,1,0]} under required `imported`
    let rows0 = root.join("l0.rows.jsonl");
    let l0 = check_streamed_with_leaves(
        &set_i,
        &ev_i,
        &opts(),
        &rows0,
        &ks(&set_i, &[[2, 1, 0]]),
        &[],
    )
    .unwrap();
    assert!(
        l0.blocking().is_empty(),
        "layer 0 closed at its required level: {:?}",
        l0.blocking()
    );
    let ls = leafset_like_layers(&set_i, &l0, &rows0, Level::Imported);
    assert!(ls.is_closed());
    let levels: Vec<String> = (0..ls.len()).map(|i| ls.level_of(i).to_string()).collect();
    assert!(
        levels.iter().all(|l| l == "imported"),
        "the leaves are certified at `imported` only: {levels:?}"
    );

    // reference: ONE walk of the same start under `per-key`: not closed
    let single = check_streamed_with_leaves(
        &set_p,
        &ev_p,
        &opts(),
        &root.join("single.rows.jsonl"),
        &ks(&set_p, &[[2, 1, 0]]),
        &[],
    )
    .unwrap();
    assert!(
        single
            .blocking()
            .iter()
            .any(|b| b.starts_with("below_required_evidence:")),
        "{:?}",
        single.blocking()
    );

    // consumer: same start under `per-key`, the producer's leaf set as a prior clause
    let rel = check_streamed_with_leaves(
        &set_p,
        &ev_p,
        &opts(),
        &root.join("rel.rows.jsonl"),
        &ks(&set_p, &[[2, 1, 0]]),
        &[Arc::new(ls.clone())],
    )
    .expect("the binding accepts a leaf set of another required level");
    assert_eq!(rel.required, Level::PerKey);
    let hits = rel.leaves.as_ref().expect("leaves met");
    assert_eq!(hits.summary.reached, 1);
    assert_eq!(hits.summary.open_reached, 0);
    // FIXED (planfix L1; was DEFECT): the leaf certified only at `imported` BLOCKS the per-key run (typed word), the
    // walk is never `closed` relative to it
    assert!(
        rel.blocking()
            .iter()
            .any(|b| b == "leaf_below_required_evidence:1"),
        "{:?}",
        rel.blocking()
    );
    assert_ne!(rel.summary.verdict, Verdict::Closed);
    assert_eq!(hits.summary.relative_to, vec![ls.digest().to_string()]);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------------------------------------------
// core closure: the binding against a TERMINAL declared later at a frozen key, and a seeded differential test of
// layered vs single-pass closure on random tables.

struct AllRegular;
impl FactSource for AllRegular {
    fn coefficient(&self, _: &RuleId, _: &[i32], _: usize, _: &[i32]) -> CoefFact {
        CoefFact::NonZero
    }
    fn regularity(&self, _: &RuleId, _: &[i32]) -> RegFact {
        RegFact::Regular
    }
    fn label(&self) -> String {
        "audit:all-regular".into()
    }
}

const CHART: [bool; 4] = [true, true, false, false];

fn codec4() -> KeyCodec {
    KeyCodec::new(4, 6).unwrap()
}
fn cmp4() -> NodeComparator {
    NodeComparator::legacy(4, None).unwrap()
}
fn ks4(keys: &[Vec<i32>]) -> KeySet {
    KeySet::from_keys(codec4(), keys.iter().map(|k| k.as_slice())).unwrap()
}
fn cfg4() -> ClosureConfig {
    ClosureConfig {
        tier: Tier::C,
        threads: 2,
        hand_downs: HandDowns::Keys,
        table_digests: [("A".to_string(), "sha256:A".to_string())].into(),
        ..Default::default()
    }
}

/// The leaf set of a finished core walk (own keys; certified = served or terminal; level word fixed).
fn leaves_of(c: &rustred_campaign_core::closure::Closure, verdict: &str) -> LeafSet {
    let mut b = LeafSetBuilder::new(codec4(), vec![], vec![]);
    for k in c.own_keys().iter() {
        let i = c.index_of(&k).unwrap();
        let cert = c.winner(i).is_some() || c.is_terminal(i);
        b.push(&k, "table", if cert { "per-key" } else { "open" }, cert)
            .unwrap();
    }
    for im in c.credit_images().iter() {
        b.aux(&im).unwrap();
    }
    b.finish(LeafBinding {
        statement_digest: "audit".into(),
        source_verdict: verdict.into(),
        node: "audit".into(),
        clause: "l".into(),
        start_digest: c.summary().start_set_digest,
        walk: c.leaf_walk().clone(),
        relative_to: vec![],
        provenance: BTreeMap::new(),
        // planfix (L1, format 2): certified at per-key on the exact tier
        producer: Some(LeafProducer::new("per-key", Tier::C, true)),
    })
    .unwrap()
}

/// Free rule on the chart lowering line 1 (`I(n0, n1-1)`), so every top key is served.
fn chain_node() -> (Dispatcher, RuleBodies) {
    let mut t = Table::new("A", 4);
    t.push(0, CaseDomain::free(4, Some(CHART.to_vec())))
        .unwrap();
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    b.set(&d, 0, 0, &["I(n0,n1-1,n2,n3)"]).unwrap();
    (d, b)
}

/// Brief 1: a terminal declared LATER at a key of a frozen layer refuses the later run (binding `terminals@keys`);
/// a terminal declared at a key outside the leaf set does not touch the binding.
#[test]
fn a_terminal_declared_later_at_a_frozen_key_refuses_the_later_walk() {
    let (d, b) = chain_node();
    let base = closure(
        &d,
        &b,
        &cmp4(),
        &ks4(&[vec![3, 3, 0, 0]]),
        &cfg4(),
        &AllRegular,
    )
    .unwrap();
    assert_eq!(base.summary().verdict, Verdict::Closed);
    let ls = Arc::new(leaves_of(&base, "closed"));
    let term_at = |k: Vec<i32>| ClosureConfig {
        leaves: vec![ls.clone()],
        terminals: Some(TerminalSets {
            preferred: Some(ks4(&[k])),
            other: None,
        }),
        ..cfg4()
    };
    match closure(
        &d,
        &b,
        &cmp4(),
        &ks4(&[vec![3, 5, 0, 0]]),
        &term_at(vec![3, 2, 0, 0]),
        &AllRegular,
    ) {
        Err(ClosureError::Leaves(m)) => assert!(m.contains("terminals@keys"), "{m}"),
        r => panic!("not refused: {:?}", r.map(|c| c.summary().verdict)),
    }
    let ok = closure(
        &d,
        &b,
        &cmp4(),
        &ks4(&[vec![3, 5, 0, 0]]),
        &term_at(vec![7, 7, 0, 0]),
        &AllRegular,
    );
    assert!(ok.is_ok(), "{:?}", ok.err());
}

/// xorshift64* (seeded, no dependency)
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random point-rule node on the chart (lines 0, 1 >= 1; numerators 2, 3 <= 0) over the box n0, n1 in 1..=3,
/// n2, n3 in -1..=0. Three modes by seed: (0) every box key has a rule, every term strictly lower (or a sub-sector
/// hand-down): closes; (1) every key has a rule, about one term in eight RAISES a slot (non-descending, cycles);
/// (2) as (1) and a quarter of the keys have no rule (open).
fn random_node(seed: u64) -> (Dispatcher, RuleBodies, Vec<Vec<i32>>) {
    let mode = seed % 3;
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut boxk = vec![];
    for a in 1..=3 {
        for b in 1..=3 {
            for c in -1..=0 {
                for e in -1..=0 {
                    boxk.push(vec![a, b, c, e]);
                }
            }
        }
    }
    let mut t = Table::new("A", 4);
    let mut bodies: Vec<(usize, Vec<String>)> = vec![];
    let mut n = 0usize;
    for k in &boxk {
        if mode == 2 && r.below(4) == 0 {
            continue;
        }
        let mut dom = CaseDomain::free(4, Some(CHART.to_vec()));
        dom.fixed = k.iter().map(|&x| Some(x)).collect();
        t.push(n as i64, dom).unwrap();
        let nt = 1 + r.below(3) as usize;
        let mut terms = vec![];
        for _ in 0..nt {
            let slot = r.below(4) as usize;
            let up = mode != 0 && r.below(8) == 0;
            let mut x = k.clone();
            if slot < 2 {
                x[slot] += if up { 1 } else { -1 };
            } else {
                x[slot] += if up { -1 } else { 1 };
            }
            let in_box = x[0] >= 1
                && x[0] <= 3
                && x[1] >= 1
                && x[1] <= 3
                && x[2] >= -1
                && x[2] <= 0
                && x[3] >= -1
                && x[3] <= 0;
            let tk: Vec<i32> = if r.below(5) == 0 || !in_box {
                vec![k[0], 0, k[2], k[3]] // sub-sector (line 1 dropped): a hand-down
            } else {
                x
            };
            terms.push(format!("I({},{},{},{})", tk[0], tk[1], tk[2], tk[3]));
        }
        bodies.push((n, terms));
        n += 1;
    }
    let d = Dispatcher::new(vec![t], vec![0]).unwrap();
    let mut b = RuleBodies::new(&d);
    for (i, terms) in &bodies {
        let refs: Vec<&str> = terms.iter().map(|s| s.as_str()).collect();
        b.set(&d, 0, *i, &refs).unwrap();
    }
    (d, b, boxk)
}

fn coarse(k: &[i32]) -> i32 {
    (k[0] - 1) + (k[1] - 1) - k[2] - k[3]
}

/// Brief 1 (differential, synthetic): on 96 SEEDED random tables, close the box layer by layer (D + r ascending, each
/// layer's walk with every earlier layer's leaf set) and compare with ONE walk of the whole box: the union of the
/// layers' own keys equals the single walk's keys, the winners agree key for key, and "every layer closed (no
/// blocker)" equals "single walk closed". Non-descending rules and cycles are generated on purpose.
#[test]
fn layered_closure_equals_the_single_walk_on_seeded_random_tables() {
    let mut closed_both = 0;
    let mut open_both = 0;
    for seed in 1..=96u64 {
        let (d, b, boxk) = random_node(seed);
        let single = closure(&d, &b, &cmp4(), &ks4(&boxk), &cfg4(), &AllRegular).unwrap();
        let s_closed = single.summary().verdict == Verdict::Closed;
        let mut by: BTreeMap<i32, Vec<Vec<i32>>> = BTreeMap::new();
        for k in &boxk {
            by.entry(coarse(k)).or_default().push(k.clone());
        }
        let mut leaves: Vec<Arc<LeafSet>> = vec![];
        let mut all_closed = true;
        let mut own: BTreeMap<Vec<i32>, String> = BTreeMap::new();
        for keys in by.values() {
            let cf = ClosureConfig {
                leaves: leaves.clone(),
                ..cfg4()
            };
            let c = closure(&d, &b, &cmp4(), &ks4(keys), &cf, &AllRegular).unwrap();
            let v = c.summary().verdict;
            if v != Verdict::Closed {
                all_closed = false;
            }
            for i in 0..c.len() {
                if !c.is_leaf(i) {
                    let w = c
                        .winner(i)
                        .map(|w| format!("{}#{}", w.table, w.record))
                        .unwrap_or_else(|| "open".into());
                    assert!(
                        own.insert(c.key(i), w).is_none(),
                        "seed {seed}: key walked twice"
                    );
                }
            }
            leaves.push(Arc::new(leaves_of(
                &c,
                if v == Verdict::Closed {
                    "closed"
                } else {
                    "open"
                },
            )));
        }
        let single_w: BTreeMap<Vec<i32>, String> = (0..single.len())
            .map(|i| {
                (
                    single.key(i),
                    single
                        .winner(i)
                        .map(|w| format!("{}#{}", w.table, w.record))
                        .unwrap_or_else(|| "open".into()),
                )
            })
            .collect();
        assert_eq!(own, single_w, "seed {seed}: keys / winners differ");
        assert_eq!(
            all_closed,
            s_closed,
            "seed {seed}: verdicts differ (single {:?})",
            single.summary().blocking
        );
        if s_closed {
            closed_both += 1;
        } else {
            open_both += 1;
        }
    }
    eprintln!("random tables: closed in both {closed_both}, open in both {open_both}");
    assert!(
        closed_both >= 10 && open_both >= 10,
        "both outcomes exercised: {closed_both} / {open_both}"
    );
}
