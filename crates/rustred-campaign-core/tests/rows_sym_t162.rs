//! Symmetry rows on T162 (interfaces/ibp-rows.md section 10): the group derived from the family text, verified, and
//! cross-checked against the independent checker's 12 maps (W/h3/symgroup.py output, copied to
//! tests/data/rows/sym/checker_symgroup_T162P.json; numerator images compared EXACTLY); the frozen span cases that
//! declare `sym_rows` (fa001, f1003, bp010, bp002) and the 12 near-corner covers of F4 now decided; the control
//! "without symmetry rows" fails; a planted wrong group element is refused.

mod rows_common;

use std::io::BufReader;
use std::sync::Arc;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    PointSpec, PointStatus, RowsOptions, RowsRule, SymUse, Verdict, identity_test,
};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, GroupCheck, Provenance, SlotImage, SymElement, SymGroup, VerifiedGroup, cross_check,
    derive_group, verify_group,
};
use serde_json::{Value, json};

pub fn t162_chart() -> Vec<bool> {
    (0..20).map(|i| i < 9).collect()
}

/// The checker's map file as a generic group: line_perm on the 9 lines, `forms` = D_j(Tk) on the D basis.
pub fn checker_group() -> SymGroup {
    let v: Value = serde_json::from_str(&data("sym/checker_symgroup_T162P.json")).unwrap();
    let mut els = Vec::new();
    for g in v.as_array().unwrap() {
        let mut action = Vec::new();
        for (j, f) in g["forms"].as_array().unwrap().iter().enumerate() {
            let terms: Vec<Value> = f[0]
                .as_object()
                .unwrap()
                .iter()
                .map(|(m, c)| json!([m.parse::<usize>().unwrap(), c]))
                .collect();
            if j < 9 {
                action.push(json!([g["line_perm"][j], false]));
            } else if terms.len() == 1 && f[1] == "0" && terms[0][1] == "1" && terms[0][0].as_u64().unwrap() >= 9 {
                action.push(json!([terms[0][0], false]));
            } else {
                action.push(json!({"terms": terms, "constant": f[1]}));
            }
        }
        els.push(json!({"matrix": null, "action": action}));
    }
    SymGroup::from_json(&json!({"arity": 20, "loops": 5, "chart": "11111111100000000000", "elements": els})).unwrap()
}

pub fn t162_group(fam: &Family) -> Arc<VerifiedGroup> {
    let (g, _) = derive_group(fam, &t162_chart(), &DeriveOptions::default()).unwrap();
    let ck = checker_group();
    assert!(matches!(cross_check(&g, &ck), GroupCheck::Equal { .. }));
    Arc::new(
        verify_group(fam, g, Provenance::CrossChecked { sources: vec!["checker h3/symgroup_T162P.json".into()] })
            .unwrap(),
    )
}

#[test]
fn rows_sym_t162_group_derived_verified_cross_checked() {
    let fam = t162_family();
    let t0 = std::time::Instant::now();
    let (g, st) = derive_group(&fam, &t162_chart(), &DeriveOptions::default()).unwrap();
    let secs = t0.elapsed().as_secs_f64();
    let linear = g.elements.iter().flat_map(|e| &e.action).filter(|s| matches!(s, SlotImage::Linear { .. })).count();
    println!(
        "T162 derived: {} non-identity elements, {} candidates, {} unimodular, {} automorphisms (T and -T), {linear} linear numerator images, {secs:.2}s",
        g.elements.len(),
        st.candidates,
        st.unimodular,
        st.automorphisms
    );
    assert_eq!(g.elements.len(), 11); // 12 maps incl. the identity (checker / z17 / symmetry_map.json)
    let ck = checker_group();
    assert_eq!(ck.elements.len(), 11);
    let c = cross_check(&g, &ck);
    println!("T162 cross-check derived vs checker: {c:?}");
    match c {
        GroupCheck::Equal { compared_slots, skipped_slots, .. } => {
            assert_eq!(skipped_slots, 0);
            assert_eq!(compared_slots, 11 * 20);
        }
        m => panic!("{m:?}"),
    }
    // the checker's maps verify on their own (T reconstructed from the line action)
    let vck = verify_group(&fam, ck, Provenance::Supplied { source: "checker".into() }).unwrap();
    assert!(vck.proofs.iter().all(|p| p.reconstructed));
    let v = verify_group(&fam, g.clone(), Provenance::Derived).unwrap();
    assert_eq!(v.digest, vck.digest);
    println!("T162 group digest {}", v.digest);
    // batch-02 symmetry_map.json line permutations (12, presented frame) = ours
    let sm = std::fs::read_to_string(format!("{HO}/t162-faces/batch-02/symmetry_map.json"));
    if let Ok(sm) = sm {
        let sm: Value = serde_json::from_str(&sm).unwrap();
        let mut theirs: Vec<Vec<usize>> = sm["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect())
            .filter(|p: &Vec<usize>| p.iter().enumerate().any(|(i, &t)| i != t))
            .collect();
        let mut ours: Vec<Vec<usize>> = g.elements.iter().map(|e| e.line_perm(&t162_chart()).unwrap()).collect();
        theirs.sort();
        ours.sort();
        assert_eq!(ours, theirs, "batch-02 symmetry_map.json");
        println!("T162 = batch-02 symmetry_map.json line permutations (11 non-identity)");
    }
}

#[test]
fn rows_sym_t162_planted_wrong_element_refused() {
    let fam = t162_family();
    let (g, _) = derive_group(&fam, &t162_chart(), &DeriveOptions::default()).unwrap();
    // (a) a line transposition that is not a symmetry (D1 <-> D2, everything else fixed)
    let mut bad = g.clone();
    let mut action: Vec<SlotImage> = (0..20).map(|i| SlotImage::Exact { target: i, flip: false }).collect();
    action[0] = SlotImage::Exact { target: 1, flip: false };
    action[1] = SlotImage::Exact { target: 0, flip: false };
    bad.elements.push(SymElement { matrix: None, action });
    let r = verify_group(&fam, bad, Provenance::Supplied { source: "planted".into() });
    println!("planted transposition: {:?}", r.as_ref().err());
    assert!(r.is_err());
    // (b) a true element with one numerator image changed (exact D10 -> D11 where the geometry says otherwise)
    let mut bad = g.clone();
    let e = bad.elements.iter_mut().find(|e| matches!(e.action[9], SlotImage::Linear { .. })).unwrap();
    e.action[9] = SlotImage::Exact { target: 10, flip: false };
    let r = verify_group(&fam, bad, Provenance::Supplied { source: "planted".into() });
    println!("planted numerator image: {:?}", r.as_ref().err());
    assert!(r.is_err());
    // (c) a true element with a wrong matrix
    let mut bad = g.clone();
    if let Some(m) = &mut bad.elements[0].matrix {
        m[0] += 1;
    }
    assert!(verify_group(&fam, bad, Provenance::Derived).is_err());
}

fn decide(fam: &Family, rule: &RowsRule, pts: &[PointSpec], g: &Arc<VerifiedGroup>) -> (Verdict, Verdict, String) {
    let with = RowsOptions { symmetry: Some(g.clone()), ..RowsOptions::default() };
    let without = RowsOptions { symmetry: Some(g.clone()), sym_use: SymUse::Without, ..RowsOptions::default() };
    let v = identity_test(fam, rule, pts, &with);
    let w = identity_test(fam, rule, pts, &without);
    let s = format!(
        "{} | sym rows {} support {:?} | without: {}",
        summary("", &v),
        v.symmetry.as_ref().map_or(0, |s| s.rows_total),
        v.symmetry.as_ref().map(|s| s.support.clone()),
        w.verdict.kind()
    );
    (v.verdict, w.verdict, s)
}

#[test]
fn rows_sym_t162_frozen_span_cases() {
    let Some(o) = oracle() else { return };
    let fam = t162_family();
    let g = t162_group(&fam);
    let mut n = 0;
    for case in o.jsonl("span/F6_span_cases.jsonl") {
        let id = case["id"].as_str().unwrap().to_string();
        let Some(line) = f6_line(&o, &case) else { continue };
        let rule = rule0(&line);
        if rule.sym_rows.is_none() {
            continue;
        }
        let key: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap()).collect();
        let t0 = std::time::Instant::now();
        let (v, w, s) = decide(&fam, &rule, &[PointSpec::Key(key)], &g);
        println!("{id} (sym_rows {:?}): {s} [{:.2}s]", rule.sym_rows, t0.elapsed().as_secs_f64());
        assert_eq!(v, Verdict::Pass, "{id}");
        assert!(matches!(w, Verdict::Fail { .. }), "{id}: without symmetry rows {w:?}");
        n += 1;
    }
    if n > 0 {
        assert_eq!(n, 4);
    }
}

/// k3 F4: the 12 near-corner covers that declare symmetry rows, at their key, with the derived group.
#[test]
fn rows_sym_t162_f4_nearcorner_covers() {
    let Some(o) = oracle() else { return };
    let fam = t162_family();
    let g = t162_group(&fam);
    let mut by_file: std::collections::BTreeMap<String, Vec<Value>> = std::collections::BTreeMap::new();
    for e in o.jsonl("T162/F4_nearcorner.jsonl") {
        if e["verified_by_us"] == true && e["sym_rows"] == true {
            by_file.entry(e["rule_file"].as_str().unwrap().to_string()).or_default().push(e);
        }
    }
    let (mut pass, mut fail_without) = (0, 0);
    for (file, mut es) in by_file {
        es.sort_by_key(|e| e["rule_line0"].as_u64().unwrap());
        let path = format!("{HO}/t162-faces/{file}");
        let Ok(f) = std::fs::File::open(&path) else {
            println!("SKIP: {path}");
            return;
        };
        let mut rd = BufReader::with_capacity(1 << 20, f);
        let mut buf: Vec<u8> = Vec::new();
        let mut at = 0usize;
        for e in es {
            let line0 = e["rule_line0"].as_u64().unwrap() as usize;
            loop {
                buf.clear();
                assert!(std::io::BufRead::read_until(&mut rd, b'\n', &mut buf).unwrap() > 0);
                at += 1;
                if at - 1 == line0 {
                    break;
                }
            }
            let claimed = e["claimed"].as_str().unwrap();
            let k: i64 = claimed.split('#').nth(1).unwrap().parse().unwrap();
            let rules = RowsRule::from_json_line(std::str::from_utf8(&buf).unwrap().trim_end(), 20).unwrap();
            let rule = if rules.len() == 1 { &rules[0] } else { rules.iter().find(|r| r.index == Some(k)).unwrap() };
            let key: Vec<i64> = e["key"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
            let mut pts = vec![PointSpec::Key(key)];
            if rule.fixed.iter().any(Option::is_none) {
                pts.push(PointSpec::Generic);
            }
            let (v, w, s) = decide(&fam, rule, &pts, &g);
            println!("{claimed} (sym_rows {:?}): {s}", rule.sym_rows);
            assert_eq!(v, Verdict::Pass, "{claimed}: a FINDING if this fails");
            pass += 1;
            if matches!(w, Verdict::Fail { .. }) {
                fail_without += 1;
            }
        }
    }
    println!("F4 near-corner with symmetry rows: {pass} pass, {fail_without} fail without symmetry rows");
    if pass > 0 {
        assert_eq!(pass, 12);
    }
}

/// determinism: two runs give byte-identical verdict JSON (except the clock fields)
#[test]
fn rows_sym_t162_determinism() {
    let Some(o) = oracle() else { return };
    let fam = t162_family();
    let g = t162_group(&fam);
    let case = o.jsonl("span/F6_span_cases.jsonl").into_iter().find(|c| c["id"] == "bp010").unwrap();
    let Some(line) = f6_line(&o, &case) else { return };
    let rule = rule0(&line);
    let key: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap()).collect();
    let opts = RowsOptions { symmetry: Some(g), ..RowsOptions::default() };
    let strip = |v: &rustred_campaign_core::ibp_rows::RowsVerdict| {
        let mut j = serde_json::to_value(v).unwrap();
        j["cost"]["seconds"] = json!(0);
        for p in j["points"].as_array_mut().unwrap() {
            for c in p["cells"].as_array_mut().unwrap() {
                c["seconds"] = json!(0);
            }
        }
        j.to_string()
    };
    let a = identity_test(&fam, &rule, &[PointSpec::Key(key.clone())], &opts);
    let b = identity_test(&fam, &rule, &[PointSpec::Key(key)], &opts);
    assert_eq!(strip(&a), strip(&b));
    assert_eq!(a.points[0].status, PointStatus::Pass);
}
