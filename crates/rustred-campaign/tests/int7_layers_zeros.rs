//! int7 (integration round 2, wire-in B3): lane layers' one change in lane zeros2's file `closeloop/zeros.rs`
//! (`closure_membership`: a key of a LEAF set is a closure key) against zeros2's final scan (`scan_with`, which also
//! lists every served pair's zero terms for the all_zeros route). Pure (no process): closeloop4_zeros' toy node
//! (arity 3, chart 110, point rules + instantiate facts).
//!
//! Claim tested: closing the box LAYER BY LAYER gives the same decisive zeros and the same zero pairs as ONE walk of
//! the union, because an edge into an earlier layer's key is an edge into the (composed) closure, never a "reach"
//! zero. Without the leaf set the same zero IS decisive (reach): the leaf-key rule is what makes the two agree.
//! (The textual side of B3, "both merge orders give the same zeros.rs", is recorded in WS/notes/implementer.int7.md.)
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::closeloop::{layers, zeros};
use rustred_campaign_core::closure::{KeySet, Tier};
use rustred_campaign_core::import::check::{check_streamed, check_streamed_with_leaves};
use rustred_campaign_core::import::{
    CheckOptions, Evidence, ImportedSet, Level, TableSetSpec, load,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// (rule index, fixed key, [(term key, coefficient word)]): closeloop4_zeros' toy.
type Rule = (i64, [i32; 3], Vec<([i32; 3], &'static str)>);

fn rules() -> Vec<Rule> {
    vec![
        // a ZERO term to [3,2,0]: nothing of [5,1,0]'s own cone reaches it (decisive "reach" in a walk from [5,1,0])
        (
            0,
            [5, 1, 0],
            vec![
                ([4, 1, 0], "nonzero"),
                ([5, 0, 0], "nonzero"),
                ([3, 2, 0], "zero"),
            ],
        ),
        (
            1,
            [4, 1, 0],
            vec![([3, 1, 0], "nonzero"), ([2, 1, 0], "zero")],
        ),
        // a ZERO term to a key that is not lower (decisive: descent)
        (
            2,
            [3, 1, 0],
            vec![([2, 1, 0], "nonzero"), ([9, 1, 0], "zero")],
        ),
        (3, [2, 1, 0], vec![([2, 0, 0], "nonzero")]),
        // the earlier LAYER's key: its own zero term (decisive "reach" in every walk that contains [3,2,0])
        (
            4,
            [3, 2, 0],
            vec![([3, 0, 0], "nonzero"), ([2, 2, 0], "zero")],
        ),
    ]
}

fn key_text(k: &[i32; 3]) -> String {
    format!("I({},{},{})", k[0], k[1], k[2])
}

fn rule_line(r: &Rule) -> String {
    json!({"type":"rule","chart":"110","index":r.0,"fixed":r.1,"equations":[],"target":key_text(&r.1),
        "rhs": r.2.iter().map(|(k, _)| json!([key_text(k), "1"])).collect::<Vec<_>>(),
        "exceptions":[],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

fn material(dir: &Path) -> (ImportedSet, Evidence) {
    std::fs::create_dir_all(dir).unwrap();
    let rs = rules();
    let lines: Vec<String> = rs.iter().map(rule_line).collect();
    let mut t = format!("{HEADER}\n");
    t.push_str(r#"{"type":"chart","chart":"110","guards":["n>=1","n>=1","n<=0"],"zero":false}"#);
    t.push('\n');
    for l in &lines {
        t.push_str(l);
        t.push('\n');
    }
    t.push_str("{\"type\":\"end\",\"charts\":1}\n");
    let tp = dir.join("A.table.jsonl");
    std::fs::write(&tp, &t).unwrap();
    let mut facts = String::new();
    for (r, l) in rs.iter().zip(&lines) {
        let rhs: Vec<Value> =
            r.2.iter()
                .enumerate()
                .map(|(j, (k, w))| json!({"term": j, "key": k, "coef": w}))
                .collect();
        facts.push_str(
            &json!({"schema":"rustred.proof-kernel.instantiate.v1","record_sha256": sha(l.as_bytes()),
                "rule": r.0, "key": r.1, "in_case": true, "chart_ok": true, "branch": null, "regular": "regular",
                "irregular_terms": [], "hidden_loci": [], "rhs": rhs, "zero_test": "exact", "error": null})
            .to_string(),
        );
        facts.push('\n');
    }
    std::fs::write(dir.join("facts.jsonl"), facts).unwrap();
    std::fs::write(dir.join("start.jsonl"), "[5,1,0]\n").unwrap();
    let spec = json!({"schema":"rustred.campaign.import-spec.v1","node":"toy","frame_from":"A","bits":6,
        "start":{"path": dir.join("start.jsonl").display().to_string(),"format":"keys_jsonl"},
        "required_evidence":"imported","policies":{"tier":"C"},
        "sources":[{"id":"A","kind":"engine_table","class":"base","path": tp.display().to_string(),
                    "sha256": sha(t.as_bytes())}],
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string(),
                              "sha256": sha(&std::fs::read(dir.join("facts.jsonl")).unwrap()), "family": "TOY"}]}});
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

type Pairs = BTreeSet<(Vec<i32>, Vec<i32>)>;

/// (decisive (key, term key), zero pairs (applied key, terms) as (applied, [term..]) flattened to (applied, term key))
fn scanned(
    set: &ImportedSet,
    ev: &Evidence,
    res: &rustred_campaign_core::import::CheckResult,
) -> (Pairs, Pairs) {
    let sc = zeros::scan_with(set, ev, res, 2, true).unwrap();
    let decisive = sc
        .decisive
        .iter()
        .map(|z| (z.key.clone(), z.term_key.clone()))
        .collect();
    let zero_pairs = sc
        .zero_pairs
        .iter()
        .flat_map(|zp| {
            zp.terms
                .iter()
                .map(move |&j| (zp.applied.clone(), vec![zp.record as i32, j as i32]))
        })
        .collect();
    (decisive, zero_pairs)
}

#[test]
fn layered_decisive_zeros_and_zero_pairs_equal_one_walk_of_the_union_and_leaf_keys_are_closure_keys()
 {
    let root = std::env::temp_dir().join(format!("int7-layers-zeros-{}", std::process::id()));
    let (set, ev) = material(&root.join("m"));
    let ks = |keys: &[[i32; 3]]| {
        KeySet::from_keys(set.codec, keys.iter().map(|k| k.as_slice())).unwrap()
    };

    // ONE walk of the union {[5,1,0], [3,2,0]}
    let union = rustred_campaign_core::import::check::check_streamed_from(
        &set,
        &ev,
        &opts(),
        &root.join("union.rows.jsonl"),
        &ks(&[[5, 1, 0], [3, 2, 0]]),
    )
    .unwrap();
    let (d_union, z_union) = scanned(&set, &ev, &union);
    let want: Pairs = [
        (vec![3, 1, 0], vec![9, 1, 0]), // descent
        (vec![3, 2, 0], vec![2, 2, 0]), // reach
    ]
    .into_iter()
    .collect();
    assert_eq!(
        d_union, want,
        "decisive zeros of the single walk of the union"
    );

    // LAYER 0 = {[3,2,0]}, then LAYER 1 = {[5,1,0]} with layer 0's leaf set
    let rows0 = root.join("l0.rows.jsonl");
    let l0 =
        check_streamed_with_leaves(&set, &ev, &opts(), &rows0, &ks(&[[3, 2, 0]]), &[]).unwrap();
    let ls0 = layers::leafset_of(
        &set,
        &l0,
        &rows0,
        Level::Imported,
        true,
        "toy",
        "layer-0",
        "open",
        &ks(&[[3, 2, 0]]).digest(),
    )
    .unwrap();
    assert!(ls0.contains(&[3, 2, 0]));
    let l1 = check_streamed_with_leaves(
        &set,
        &ev,
        &opts(),
        &root.join("l1.rows.jsonl"),
        &ks(&[[5, 1, 0]]),
        &[Arc::new(ls0)],
    )
    .unwrap();
    // the membership of layer 1's closure includes the leaf key (lane layers' change in zeros.rs)
    let member = zeros::closure_membership(&l1);
    assert!(member(&[3, 2, 0]), "a leaf key is a closure key");
    assert!(member(&[5, 1, 0]) && !member(&[9, 1, 0]));
    let (d0, z0) = scanned(&set, &ev, &l0);
    let (d1, z1) = scanned(&set, &ev, &l1);
    assert!(
        !d1.contains(&(vec![5, 1, 0], vec![3, 2, 0])),
        "the zero edge into the earlier layer is not a reach zero: {d1:?}"
    );
    let layered: Pairs = d0.union(&d1).cloned().collect();
    assert_eq!(layered, d_union, "layered decisive zeros == single walk's");
    let layered_z: Pairs = z0.union(&z1).cloned().collect();
    assert_eq!(
        layered_z, z_union,
        "layered zero pairs == single walk's (all_zeros covers every layer)"
    );

    // WITHOUT the leaf set the same zero is decisive (reach): the leaf-key rule is what makes both agree
    let alone = check_streamed(&set, &ev, &opts(), &root.join("alone.rows.jsonl")).unwrap();
    let (d_alone, _) = scanned(&set, &ev, &alone);
    assert!(
        d_alone.contains(&(vec![5, 1, 0], vec![3, 2, 0])),
        "{d_alone:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}
