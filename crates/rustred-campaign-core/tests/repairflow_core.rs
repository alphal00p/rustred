//! Pure tests of the repair flow library (interfaces/repairflow.md): policy, faces, judge, node budget, recipes.
//! Toy node: arity 5, lines 0..3, numerators 3..5 (test data only).

use rustred_campaign_core::repairflow::*;
use rustred_campaign_core::worklist::items::Face;
use serde_json::json;
use std::collections::BTreeMap;

fn lines() -> Vec<bool> {
    vec![true, true, true, false, false]
}

fn policy(over: serde_json::Value) -> RepairPolicy {
    let f = PolicyFile::parse(
        &json!({"schema": POLICY_SCHEMA, "repair_policy": over, "nodes": {}}).to_string(),
    )
    .unwrap();
    f.effective(None).unwrap()
}

fn m(bytes: u64, rhs: u64, mono: u64, rows: u64) -> Measures {
    Measures {
        bytes,
        rhs_terms: rhs,
        monomials: mono,
        trace_rows: rows,
        seconds: 0.0,
        free_dims: 0,
    }
}

#[test]
fn policy_defaults_merge_overrides_and_refusals() {
    let d = RepairPolicy::default();
    assert_eq!(d.keep.max_bytes, 2 << 20);
    assert_eq!(d.node_budget.max_bytes, 64 << 20);
    let f = PolicyFile::parse(
        &json!({"schema": POLICY_SCHEMA,
                "repair_policy": {"keep": {"max_bytes": 1000}, "symbolic": {"max_depth": 3}},
                "nodes": {"N": {"keep": {"min_keys_served": 7}, "symbolic": {"enabled": false}}}})
        .to_string(),
    )
    .unwrap();
    let c = f.effective(None).unwrap();
    assert_eq!(c.keep.max_bytes, 1000);
    assert_eq!(c.symbolic.max_depth, 3);
    assert!(c.symbolic.enabled);
    let n = f.effective(Some("N")).unwrap();
    assert_eq!(n.keep.max_bytes, 1000, "campaign section kept under the node override");
    assert_eq!(n.keep.min_keys_served, 7);
    assert!(!n.symbolic.enabled);
    assert_ne!(c.digest(), n.digest());
    // unknown fields, bad values, bad steps are refused (never a silent default)
    for bad in [
        json!({"keep": {"max_bites": 1}}),
        json!({"cost": {"cache_hit_fraction": 1.5}}),
        json!({"symbolic": {"centre": "middle"}}),
        json!({"recipes": {"escalation": ["x9"]}}),
        json!({"node_budget": {"order": "random"}}),
    ] {
        assert!(
            PolicyFile::parse(&json!({"schema": POLICY_SCHEMA, "repair_policy": bad}).to_string())
                .is_err(),
            "{bad} must be refused"
        );
    }
    assert!(PolicyFile::parse(&json!({"schema": "x", "repair_policy": {}}).to_string()).is_err());
}

#[test]
fn faces_group_order_and_requests() {
    let keys = vec![
        vec![2, 1, 1, 0, 0],
        vec![3, 1, 1, 0, 0],
        vec![1, 1, 1, -1, 0],
        vec![2, 1, 1, -1, 0],
        vec![1, 1, 1, 1, 0], // outside the chart orthant
    ];
    let (g, out) = group_open_keys(&keys, &lines(), "open_keys_desc");
    assert_eq!(out, vec![vec![1, 1, 1, 1, 0]]);
    assert_eq!(g.len(), 3);
    assert_eq!(g[0].label, "{0}x{}");
    assert_eq!(g[0].open_keys.len(), 2);
    let (ga, _) = group_open_keys(&keys, &lines(), "open_keys_asc");
    assert_eq!(ga.last().unwrap().label, "{0}x{}");
    let f = Face::new(vec![0, 2], vec![3]);
    assert_eq!(
        case_line(&f, &lines()),
        json!({"fixed": [null, 1, null, null, 0], "equations": []})
    );
    let p = RepairPolicy::default();
    let a = s_args(&f, &p, "id", &[2, 1, 0], &[4, 3]);
    let s = a.join(" ");
    // last line in the priority among the face lines = 0, floored
    assert!(s.contains("--seed-centre-offset 0:-1 --seed-face-floor 1"), "{s}");
    assert!(s.contains("--descend open"));
    assert!(s.contains("--screen 0,64,24,2500 --on-heavy refuse --open-on-refusal"));
    let pn = policy(json!({"symbolic": {"centre": "union_floored"}}));
    assert!(
        s_args(&f, &pn, "id", &[0, 1, 2], &[3, 4])
            .join(" ")
            .contains("0:-1;2:-1;3:1 --seed-face-floor 1")
    );
    let corner = Face::new(vec![], vec![]);
    assert!(!s_args(&corner, &p, "id", &[0, 1, 2], &[3, 4]).contains(&"--seed-centre-offset".to_string()));
}

#[test]
fn judge_classes_and_cost_model() {
    let p = RepairPolicy::default();
    // small rule serving 5 keys: kept
    let j = judge(&m(10_000, 100, 1_000, 50), 5, 5, &p);
    assert_eq!(j.class, Class::Keep);
    // over a cap: the field and numbers are recorded
    let j = judge(&m(3 << 20, 100, 1_000, 50), 5, 5, &p);
    assert_eq!(
        j.class,
        Class::OverCap {
            field: "bytes".into(),
            value: 3 << 20,
            cap: 2 << 20
        }
    );
    // one key only: a point solve in disguise
    assert_eq!(
        judge(&m(10_000, 100, 1_000, 50), 1, 3, &p).class,
        Class::TooFewKeys {
            served: 1,
            needed: 2
        }
    );
    // fraction
    let pf = policy(json!({"keep": {"min_served_fraction": 0.5}}));
    assert_eq!(
        judge(&m(10_000, 100, 1_000, 50), 2, 10, &pf).class,
        Class::TooFewKeys {
            served: 2,
            needed: 5
        }
    );
    // cost model: with every use from the cache, a large rule serving few keys does not amortise its load
    let pc = policy(json!({"cost": {"cache_hit_fraction": 1.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}));
    let big = m(50 << 20, 5_000, 2_000_000, 4_000);
    assert_eq!(judge(&big, 2, 2, &pc).class, Class::CostlierThanRecipe);
    let c = cost(&big, 2, &pc);
    assert!(c.c_table_ns > c.c_recipe_ns);
    // uncached recipes (cold runs): the same rule is cheaper than solving at the key
    let pu = policy(json!({"cost": {"cache_hit_fraction": 0.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}));
    assert_eq!(judge(&big, 2, 2, &pu).class, Class::Keep);
    // cost off: caps only
    let po = policy(json!({"cost": {"enabled": false, "cache_hit_fraction": 1.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}));
    assert_eq!(judge(&big, 2, 2, &po).class, Class::Keep);
}

fn face(label_sd: &[usize], sn: &[usize], keys: Vec<Vec<i32>>) -> FaceGroup {
    let f = Face::new(label_sd.to_vec(), sn.to_vec());
    FaceGroup {
        label: f.label(),
        sd: f.sd.clone(),
        sn: f.sn.clone(),
        open_keys: keys,
    }
}

fn cand(sha: &str, bytes: u64, served: Vec<Vec<i32>>) -> CandidateAnswer {
    CandidateAnswer {
        record_sha256: sha.into(),
        line: format!("{{\"type\":\"rule\",\"x\":\"{sha}\"}}"),
        measures: m(bytes, 10, 100, 10),
        served,
    }
}

#[test]
fn node_budget_by_keys_per_byte_and_determinism() {
    let fa = face(&[0], &[], vec![vec![2, 1, 1, 0, 0], vec![3, 1, 1, 0, 0], vec![4, 1, 1, 0, 0]]);
    let fb = face(&[1], &[], vec![vec![1, 2, 1, 0, 0], vec![1, 3, 1, 0, 0]]);
    let fc = face(&[2], &[], vec![vec![1, 1, 2, 0, 0], vec![1, 1, 3, 0, 0]]);
    let faces = vec![fa.clone(), fb.clone(), fc.clone()];
    let shadow = vec![ShadowClass::Skipped; 3];
    let mut answers = BTreeMap::new();
    // A: 3 keys / 3000 B (1e-3), B: 2 keys / 1000 B (2e-3), C: 2 keys / 4000 B (5e-4)
    answers.insert(fa.label.clone(), FaceAnswer::Candidates { job: "ja".into(), candidates: vec![cand("a", 3000, fa.open_keys.clone())] });
    answers.insert(fb.label.clone(), FaceAnswer::Candidates { job: "jb".into(), candidates: vec![cand("b", 1000, fb.open_keys.clone())] });
    answers.insert(fc.label.clone(), FaceAnswer::Candidates { job: "jc".into(), candidates: vec![cand("c", 4000, fc.open_keys.clone())] });
    let p = policy(json!({"node_budget": {"max_bytes": 4500}}));
    let d = decide("N", &faces, &shadow, &answers, &[], &p);
    // B (best keys/byte) then A fit (4000 <= 4500); C does not
    assert_eq!(d.kept_rules, 2);
    assert_eq!(d.kept_bytes, 4000);
    assert!(d.faces[0].kept.is_some() && d.faces[1].kept.is_some());
    assert!(matches!(d.faces[2].symbolic.class, Class::OverNodeBudget { running_bytes: 4000, max_bytes: 4500, .. }));
    // every face gets the recipe escalation (kept ones too: their exception keys)
    assert!(d.faces.iter().all(|f| f.recipe_steps == p.recipes.escalation));
    // order "bytes_asc": B (1000), A (3000) -> same here; "keys": A first then B (5 keys / 4000 B) and C refused
    let pk = policy(json!({"node_budget": {"max_bytes": 4500, "order": "keys"}}));
    let dk = decide("N", &faces, &shadow, &answers, &[], &pk);
    assert_eq!(dk.kept_bytes, 4000);
    // determinism: the answers map in any insertion order gives byte-identical decisions
    let mut rev = BTreeMap::new();
    for (k, v) in answers.iter().rev() {
        rev.insert(k.clone(), v.clone());
    }
    assert_eq!(
        serde_json::to_string(&decide("N", &faces, &shadow, &rev, &[], &p)).unwrap(),
        serde_json::to_string(&d).unwrap()
    );
    // NEVER TABLES / NEVER RECIPES / face caps
    let nt = decide("N", &faces, &shadow, &answers, &[], &policy(json!({"symbolic": {"enabled": false}})));
    assert_eq!(nt.kept_rules, 0);
    assert!(nt.faces.iter().all(|f| f.symbolic.class == Class::SymbolicOff && !f.recipe_steps.is_empty()));
    let nr = decide("N", &faces, &shadow, &answers, &[], &policy(json!({"recipes": {"enabled": false}})));
    assert!(nr.faces.iter().all(|f| f.recipe_steps.is_empty()));
    let cap = s_plan(&faces, &shadow, &policy(json!({"symbolic": {"max_faces": 1}})));
    assert_eq!(cap, vec![None, Some(Class::FaceCap), Some(Class::FaceCap)]);
    let sh = vec![ShadowClass::Shadowed { certificate: "c".into() }, ShadowClass::Skipped, ShadowClass::Skipped];
    assert_eq!(s_plan(&faces, &sh, &p)[0], Some(Class::Shadowed));
}

#[test]
fn face_recipes_domain_and_seeds() {
    let ctx = RecipeContext { node: "N", node_order: "id", policy_digest: "p" };
    let f = Face::new(vec![0, 2], vec![3]);
    let r = face_recipe(&f, "d2_clast_fon_mM1C", &lines(), &[2, 1, 0], &[3, 4], &ctx).unwrap();
    assert_eq!(r.seeds.centres, vec![vec![-1, 0, 0, 0, 0]]);
    assert_eq!(r.seeds.min_offset.keys().copied().collect::<Vec<_>>(), vec![0, 2]);
    assert!(r.in_domain(&[2, 1, 3, -1, 0]));
    assert!(!r.in_domain(&[1, 1, 3, -1, 0]), "a key of a smaller face is outside the face domain");
    assert!(!r.in_domain(&[2, 1, 3, 0, 0]));
    assert!(!r.in_domain(&[2, 1, 3, -1, -1]), "another numerator off its floor: another face");
    let u = face_recipe(&f, "d3_cunion_foff_mM1C", &lines(), &[0, 1, 2], &[3, 4], &ctx).unwrap();
    assert_eq!(u.seeds.centres.len(), 3);
    assert!(u.seeds.min_offset.is_empty());
    let s = face_recipe(&f, "d0_cnone_foff_ssym_mM1C", &lines(), &[0, 1, 2], &[3, 4], &ctx).unwrap();
    assert!(s.symmetry_rows && s.seeds.centres.is_empty());
    let deep = face_recipe(&f, "d4_cfirst_foff_mM1C", &lines(), &[0, 1, 2], &[3, 4], &ctx).unwrap();
    assert!(matches!(deep.seeds.generator, rustred_campaign_core::worklist::recipe::RecipeGenerator::Ball(_)));
    assert!(face_recipe(&f, "d2_clast_fon", &lines(), &[0], &[3], &ctx).is_err(), "first_admissible not expressible");
    // the file the verb reads round-trips through the acceptance parser
    let text = serde_json::to_string(&r).unwrap();
    assert_eq!(rustred_campaign_core::worklist::recipe::Recipe::parse(&text).unwrap(), r);
}

#[test]
fn extension_sources_order() {
    let fa = face(&[0], &[], vec![vec![2, 1, 1, 0, 0], vec![3, 1, 1, 0, 0]]);
    let mut answers = BTreeMap::new();
    answers.insert(fa.label.clone(), FaceAnswer::Candidates { job: "j".into(), candidates: vec![cand("a", 10, fa.open_keys.clone())] });
    let p = policy(json!({"recipes": {"escalation": ["d2_clast_fon_mM1C", "d3_clast_fon_mM1C"]}}));
    let d = decide("N", std::slice::from_ref(&fa), &[ShadowClass::Skipped], &answers, &[], &p);
    let mut files = BTreeMap::new();
    for s in &p.recipes.escalation {
        files.insert((fa.label.clone(), s.clone()), format!("/r/{s}.json"));
    }
    let v = extension_sources(&d, Some("/t.jsonl"), &files);
    let ids: Vec<&str> = v.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["rf:symbolic", "rf:d0_n:d2_clast_fon_mM1C", "rf:d0_n:d3_clast_fon_mM1C"]);
}
