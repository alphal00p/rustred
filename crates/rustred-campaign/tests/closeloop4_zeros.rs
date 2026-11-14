//! Lane closeloop4 (interfaces/closeloop.md 18.1): the DECISIVE-ZERO set is exactly the set of zero-coefficient
//! facts whose edge changes the exact closure. Pure (no process): a toy node (arity 3, chart 110) with point rules and
//! instantiate facts; every zero edge is put back one at a time (its fact flipped to non-zero) and the Tier C check is
//! rerun: a NON-decisive zero leaves the key set and the descent verdict unchanged, a decisive one changes them.

use rustred_campaign::closeloop::zeros;
use rustred_campaign_core::closure::Tier;
use rustred_campaign_core::import::check::check_streamed;
use rustred_campaign_core::import::{CheckOptions, Evidence, TableSetSpec, load};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const HEADER: &str =
    r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110"}"#;

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// (rule index, fixed key, [(term key, coefficient word)]).
type Rule = (i64, [i32; 3], Vec<([i32; 3], &'static str)>);

fn rules() -> Vec<Rule> {
    vec![
        // the start key: a lower term, a hand-down, and a ZERO term to a key nothing else reaches (decisive: reach)
        (
            0,
            [5, 1, 0],
            vec![
                ([4, 1, 0], "nonzero"),
                ([5, 0, 0], "nonzero"),
                ([3, 2, 0], "zero"),
            ],
        ),
        // a ZERO term to a key the closure reaches anyway (NOT decisive)
        (
            1,
            [4, 1, 0],
            vec![([3, 1, 0], "nonzero"), ([2, 1, 0], "zero")],
        ),
        // a ZERO term to a key that is not lower (decisive: an edge would be a descent violation; also outside C)
        (
            2,
            [3, 1, 0],
            vec![([2, 1, 0], "nonzero"), ([9, 1, 0], "zero")],
        ),
        (3, [2, 1, 0], vec![([2, 0, 0], "nonzero")]),
        // a key OUTSIDE the closure: its zero term is never expanded (NOT decisive)
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

/// Write the material; `flip` = (rule index, term) whose zero fact is written non-zero.
fn material(dir: &Path, flip: Option<(i64, usize)>) -> (TableSetSpec, Vec<String>) {
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
                .map(|(j, (k, w))| {
                    let w = if flip == Some((r.0, j)) {
                        "nonzero"
                    } else {
                        *w
                    };
                    json!({"term": j, "key": k, "coef": w})
                })
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
        "evidence":{"facts":[{"path": dir.join("facts.jsonl").display().to_string()}]}});
    (TableSetSpec::from_json(&spec.to_string()).unwrap(), lines)
}

struct Checked {
    keys: BTreeSet<Vec<i32>>,
    descent: u64,
    decisive: BTreeSet<(Vec<i32>, Vec<i32>)>,
}

fn checked(dir: &Path, flip: Option<(i64, usize)>) -> Checked {
    let (spec, _) = material(dir, flip);
    let set = load(&spec, Path::new("/")).unwrap();
    let ev = Evidence::from_spec(&set, Path::new("/")).unwrap();
    let res = check_streamed(
        &set,
        &ev,
        &CheckOptions {
            threads: 2,
            tier: Some(Tier::C),
            max_rounds: None,
        },
        &dir.join("rows.jsonl"),
    )
    .unwrap();
    let keys = std::fs::read_to_string(dir.join("rows.jsonl"))
        .unwrap()
        .lines()
        .map(|l| {
            serde_json::from_value(serde_json::from_str::<Value>(l).unwrap()["key"].clone())
                .unwrap()
        })
        .collect();
    let scan = zeros::scan(&set, &ev, &res, 2).unwrap();
    Checked {
        keys,
        descent: res.summary.descent_violations,
        decisive: scan
            .decisive
            .iter()
            .map(|z| (z.key.clone(), z.term_key.clone()))
            .collect(),
    }
}

#[test]
fn adding_back_non_decisive_zero_edges_leaves_the_closure_and_decisive_ones_change_it() {
    let root = std::env::temp_dir().join(format!("cl4zeros-{}", std::process::id()));
    let base = checked(&root.join("base"), None);
    let want: BTreeSet<(Vec<i32>, Vec<i32>)> = [
        (vec![5, 1, 0], vec![3, 2, 0]),
        (vec![3, 1, 0], vec![9, 1, 0]),
    ]
    .into_iter()
    .collect();
    assert_eq!(base.decisive, want, "the decisive set");
    assert!(!base.keys.contains(&vec![3, 2, 0]) && base.keys.contains(&vec![2, 1, 0]));
    assert_eq!(base.descent, 0);
    // every zero edge of the material, put back one at a time
    let mut n = 0;
    for r in rules() {
        for (j, (tk, w)) in r.2.iter().enumerate() {
            if *w != "zero" {
                continue;
            }
            n += 1;
            let c = checked(&root.join(format!("flip{}-{j}", r.0)), Some((r.0, j)));
            let decisive = base.decisive.contains(&(r.1.to_vec(), tk.to_vec()));
            let same = c.keys == base.keys && c.descent == base.descent;
            assert_eq!(
                same,
                !decisive,
                "rule {} term {j} -> {tk:?}: decisive {decisive}, closure unchanged {same} ({} vs {} keys, descent {})",
                r.0,
                c.keys.len(),
                base.keys.len(),
                c.descent
            );
        }
    }
    assert_eq!(n, 4);
    let _ = std::fs::remove_dir_all(&root);
}
