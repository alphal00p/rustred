//! Lane import5 (interfaces/import.md 18.5; coord-pole-engine.md 9c): SLICE RECORDS of `rr_node symface` are ordinary
//! rule records: they load; `fixed` is the slice domain (a key beyond the slices falls through to a later record); the
//! provenance member `symface` is part of the raw line, hence of `record_sha256`; the export can group the slices by
//! face (`ImportedSet::symface_groups`); the header member `symface_tables` is provenance (not identity).

mod import5_common;
use import5_common::*;

use import::{RowStatus, Server};
use rustred_campaign_core::import;
use serde_json::{Value, json};

const LOW: [i32; 5] = [1, 1, 1, 0, 0];

fn slice_rule(index: i64, m: i32) -> String {
    let key = [1, 1, 1, m, 0];
    let mut v: Value = serde_json::from_str(&point_rule(index, &key, &[&LOW])).unwrap();
    v["guard_version"] = json!(2);
    v["seeds"] = json!([]);
    v["symface"] = json!({"schema": "rustred.symface.v1",
        "element": {"index": 1, "line_permutation": [1, 0, 2], "matrix": SWAP_MATRIX},
        "face": {"dotted": [], "numerators": [3]}, "substituted": [3], "slice": {"3": m}, "slice_max": 2,
        "proof": {"automorphism": "verified", "form_check": "exact", "expansion_check": "exact",
                  "descent": "affine_forms"}});
    v.to_string()
}

/// A general record AFTER the slices: serves every key of the numerator line the slices do not fix.
fn general_rule(index: i64) -> String {
    json!({"type":"rule","chart":MASK,"index":index,"fixed":[1,1,1,null,0],"equations":[],"target":"I(1,1,1,n3,0)",
        "rhs":[[kt(&LOW),"1"]],"exceptions":[["n3"]],"dispatch":"Partition","witness_loci":[]})
    .to_string()
}

#[test]
fn slice_records_load_bound_by_fixed_with_provenance() {
    let d = scratch("symface");
    let lines = vec![slice_rule(0, -1), slice_rule(1, -2), general_rule(2)];
    let mut hdr = header_value();
    hdr["symface_tables"] = json!({"slice_max": 2, "faces": [{"face": {"dotted": [], "numerators": [3]},
        "element": 1, "slices": 2, "rules": 2, "rhs_terms": 2}]});
    let tail =
        vec![json!({"type":"end","rules":3,"symface":{"faces":1,"slice_max":2}}).to_string()];
    let a = write_table_with(&d, "a.jsonl", &hdr, &lines, Some(&tail));
    // a second table WITHOUT symface_tables: header identity still agrees (provenance member)
    let b = write_table(&d, "b.jsonl", &[point_rule(0, &[1, 1, 1, -5, 0], &[&LOW])]);
    let k1 = [1, 1, 1, -1, 0];
    let k2 = [1, 1, 1, -2, 0];
    let k3 = [1, 1, 1, -3, 0];
    let sp = spec(
        &d,
        &a,
        &[&k1, &k2, &k3],
        json!({}),
        &[
            json!({"id":"B","kind":"engine_table","path":b.display().to_string()}),
            json!({"id":"T","kind":"terminals","keys":[LOW]}),
        ],
    );
    let (set, _ev, r) = run(&sp, &d);
    for (k, rec) in [(k1, 0usize), (k2, 1)] {
        let x = row(&r, &k);
        assert_eq!(x.status, RowStatus::Served);
        assert_eq!(
            x.server,
            Some(Server::Rule {
                table: 0,
                record: rec
            }),
            "{k:?}"
        );
    }
    // beyond the slices: falls through to the general record
    assert_eq!(
        row(&r, &k3).server,
        Some(Server::Rule {
            table: 0,
            record: 2
        })
    );
    // provenance in the record's identity
    let text = std::fs::read_to_string(&a).unwrap();
    let line = text.lines().nth(2).unwrap();
    assert!(line.contains("\"symface\""));
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(line.as_bytes());
    let info = set.rule(0, 0);
    assert_eq!(info.record_sha256, h.finish_hex());
    let sf = info.symface.as_ref().expect("symface provenance");
    assert_eq!(sf.element, Some(1));
    assert_eq!(sf.face, json!({"dotted": [], "numerators": [3]}));
    assert!(set.rule(0, 2).symface.is_none());
    // export grouping: one face, one element, both slices
    let g = set.symface_groups();
    assert_eq!(g.len(), 1);
    assert_eq!(g.values().next().unwrap(), &vec![(0, 0), (0, 1)]);
}
