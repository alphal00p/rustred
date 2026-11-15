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
    assert_eq!(
        n.keep.max_bytes, 1000,
        "campaign section kept under the node override"
    );
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
        json!({"frame": "PRESENTED", "fixed": [null, 1, null, null, 0], "equations": []})
    );
    let p = policy(json!({"symbolic": {"centre": "last_line_floored"}}));
    let a = s_args(&f, &p, "id", &[2, 1, 0], &[4, 3]);
    let s = a.join(" ");
    // last line in the priority among the face lines = 0, floored
    assert!(
        s.contains("--seed-centre-offset 0:-1 --seed-face-floor 1"),
        "{s}"
    );
    assert!(s.contains("--descend open"));
    assert!(s.contains("--screen 0,64,24,2500 --on-heavy refuse --open-on-refusal"));
    // the DEFAULT seed setting: every free slot one step to its floor, floored (helper w13: reproduces the engine's
    // minimal face rule term for term)
    let pn = RepairPolicy::default();
    assert!(
        s_args(&f, &pn, "id", &[0, 1, 2], &[3, 4])
            .join(" ")
            .contains("0:-1;2:-1;3:1 --seed-face-floor 1")
    );
    let corner = Face::new(vec![], vec![]);
    assert!(
        !s_args(&corner, &p, "id", &[0, 1, 2], &[3, 4])
            .contains(&"--seed-centre-offset".to_string())
    );
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
    let pc = policy(
        json!({"cost": {"cache_hit_fraction": 1.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}),
    );
    let big = m(50 << 20, 5_000, 2_000_000, 4_000);
    assert_eq!(judge(&big, 2, 2, &pc).class, Class::CostlierThanRecipe);
    let c = cost(&big, 2, &pc);
    assert!(c.c_table_ns > c.c_recipe_ns);
    // uncached recipes (cold runs): the same rule is cheaper than solving at the key
    let pu = policy(
        json!({"cost": {"cache_hit_fraction": 0.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}),
    );
    assert_eq!(judge(&big, 2, 2, &pu).class, Class::Keep);
    // cost off: caps only
    let po = policy(
        json!({"cost": {"enabled": false, "cache_hit_fraction": 1.0}, "keep": {"max_bytes": 1u64 << 40,
        "max_monomials": 1u64 << 40, "max_rhs_terms": 1u64 << 40, "max_trace_rows": 1u64 << 40}}),
    );
    assert_eq!(judge(&big, 2, 2, &po).class, Class::Keep);
}

fn face(label_sd: &[usize], sn: &[usize], keys: Vec<Vec<i32>>) -> FaceGroup {
    let f = Face::new(label_sd.to_vec(), sn.to_vec());
    FaceGroup {
        label: f.label(),
        sd: f.sd.clone(),
        sn: f.sn.clone(),
        open_keys: keys.clone(),
        exception: None,
        unattributed: keys.len() as u64,
    }
}

fn cand(sha: &str, bytes: u64, served: Vec<Vec<i32>>) -> CandidateAnswer {
    CandidateAnswer {
        record_sha256: sha.into(),
        line: format!("{{\"type\":\"rule\",\"x\":\"{sha}\"}}"),
        measures: m(bytes, 10, 100, 10),
        served,
        boundary: BTreeMap::new(),
    }
}

#[test]
fn node_budget_by_keys_per_byte_and_determinism() {
    let fa = face(
        &[0],
        &[],
        vec![
            vec![2, 1, 1, 0, 0],
            vec![3, 1, 1, 0, 0],
            vec![4, 1, 1, 0, 0],
        ],
    );
    let fb = face(&[1], &[], vec![vec![1, 2, 1, 0, 0], vec![1, 3, 1, 0, 0]]);
    let fc = face(&[2], &[], vec![vec![1, 1, 2, 0, 0], vec![1, 1, 3, 0, 0]]);
    let faces = vec![fa.clone(), fb.clone(), fc.clone()];
    let shadow = vec![ShadowClass::Skipped; 3];
    let mut answers = BTreeMap::new();
    // A: 3 keys / 3000 B (1e-3), B: 2 keys / 1000 B (2e-3), C: 2 keys / 4000 B (5e-4)
    answers.insert(
        fa.label.clone(),
        FaceAnswer::Candidates {
            job: "ja".into(),
            candidates: vec![cand("a", 3000, fa.open_keys.clone())],
        },
    );
    answers.insert(
        fb.label.clone(),
        FaceAnswer::Candidates {
            job: "jb".into(),
            candidates: vec![cand("b", 1000, fb.open_keys.clone())],
        },
    );
    answers.insert(
        fc.label.clone(),
        FaceAnswer::Candidates {
            job: "jc".into(),
            candidates: vec![cand("c", 4000, fc.open_keys.clone())],
        },
    );
    let p = policy(json!({"node_budget": {"max_bytes": 4500}}));
    let d = decide("N", &faces, &shadow, &answers, &[], &p);
    // B (best keys/byte) then A fit (4000 <= 4500); C does not
    assert_eq!(d.kept_rules, 2);
    assert_eq!(d.kept_bytes, 4000);
    assert!(d.faces[0].kept.is_some() && d.faces[1].kept.is_some());
    assert!(matches!(
        d.faces[2].symbolic.class,
        Class::OverNodeBudget {
            running_bytes: 4000,
            max_bytes: 4500,
            ..
        }
    ));
    // every face gets the recipe escalation (kept ones too: their exception keys)
    assert!(
        d.faces
            .iter()
            .all(|f| f.recipe_steps == p.recipes.escalation)
    );
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
    let nt = decide(
        "N",
        &faces,
        &shadow,
        &answers,
        &[],
        &policy(json!({"symbolic": {"enabled": false}})),
    );
    assert_eq!(nt.kept_rules, 0);
    assert!(
        nt.faces
            .iter()
            .all(|f| f.symbolic.class == Class::SymbolicOff && !f.recipe_steps.is_empty())
    );
    let nr = decide(
        "N",
        &faces,
        &shadow,
        &answers,
        &[],
        &policy(json!({"recipes": {"enabled": false}})),
    );
    assert!(nr.faces.iter().all(|f| f.recipe_steps.is_empty()));
    let cap = s_plan(
        &faces,
        &shadow,
        &policy(json!({"symbolic": {"max_faces": 1}})),
    );
    assert_eq!(cap, vec![None, Some(Class::FaceCap), Some(Class::FaceCap)]);
    let sh = vec![
        ShadowClass::Shadowed {
            certificate: "c".into(),
        },
        ShadowClass::Skipped,
        ShadowClass::Skipped,
    ];
    assert_eq!(s_plan(&faces, &sh, &p)[0], Some(Class::Shadowed));
}

#[test]
fn face_recipes_domain_and_seeds() {
    let ctx = RecipeContext {
        node: "N",
        node_order: "id",
        policy_digest: "p",
        mixed_shells: RepairPolicy::default().recipes.mixed_shells,
    };
    let f = Face::new(vec![0, 2], vec![3]);
    // the mixed-shell face recipe: explicit offsets (one centre per seed), shell S0, no floor, the proper face domain
    let m = face_recipe(&f, "mixed_shell", &lines(), &[2, 1, 0], &[3, 4], &ctx).unwrap();
    assert!(m.seeds.centres.len() > 3 && m.seeds.min_offset.is_empty());
    assert!(
        m.seeds.centres.contains(&vec![0, 0, 0, 0, 0])
            && m.seeds.centres.contains(&vec![-1, 0, 0, 0, 0])
    );
    assert!(m.in_domain(&[2, 1, 3, -1, 0]) && !m.in_domain(&[1, 1, 3, -1, 0]));
    let r = face_recipe(&f, "d2_clast_fon_mM1C", &lines(), &[2, 1, 0], &[3, 4], &ctx).unwrap();
    assert_eq!(r.seeds.centres, vec![vec![-1, 0, 0, 0, 0]]);
    assert_eq!(
        r.seeds.min_offset.keys().copied().collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert!(r.in_domain(&[2, 1, 3, -1, 0]));
    assert!(
        !r.in_domain(&[1, 1, 3, -1, 0]),
        "a key of a smaller face is outside the face domain"
    );
    assert!(!r.in_domain(&[2, 1, 3, 0, 0]));
    assert!(
        !r.in_domain(&[2, 1, 3, -1, -1]),
        "another numerator off its floor: another face"
    );
    let u = face_recipe(
        &f,
        "d3_cunion_foff_mM1C",
        &lines(),
        &[0, 1, 2],
        &[3, 4],
        &ctx,
    )
    .unwrap();
    assert_eq!(u.seeds.centres.len(), 3);
    assert!(u.seeds.min_offset.is_empty());
    let s = face_recipe(
        &f,
        "d0_cnone_foff_ssym_mM1C",
        &lines(),
        &[0, 1, 2],
        &[3, 4],
        &ctx,
    )
    .unwrap();
    assert!(s.symmetry_rows && s.seeds.centres.is_empty());
    let deep = face_recipe(
        &f,
        "d4_cfirst_foff_mM1C",
        &lines(),
        &[0, 1, 2],
        &[3, 4],
        &ctx,
    )
    .unwrap();
    assert!(matches!(
        deep.seeds.generator,
        rustred_campaign_core::worklist::recipe::RecipeGenerator::Ball(_)
    ));
    assert!(
        face_recipe(&f, "d2_clast_fon", &lines(), &[0], &[3], &ctx).is_err(),
        "first_admissible not expressible"
    );
    // the file the verb reads round-trips through the acceptance parser
    let text = serde_json::to_string(&r).unwrap();
    assert_eq!(
        rustred_campaign_core::worklist::recipe::Recipe::parse(&text).unwrap(),
        r
    );
}

#[test]
fn extension_sources_order() {
    let fa = face(&[0], &[], vec![vec![2, 1, 1, 0, 0], vec![3, 1, 1, 0, 0]]);
    let mut answers = BTreeMap::new();
    answers.insert(
        fa.label.clone(),
        FaceAnswer::Candidates {
            job: "j".into(),
            candidates: vec![cand("a", 10, fa.open_keys.clone())],
        },
    );
    let p = policy(json!({"recipes": {"escalation": ["d2_clast_fon_mM1C", "d3_clast_fon_mM1C"]}}));
    let d = decide(
        "N",
        std::slice::from_ref(&fa),
        &[ShadowClass::Skipped],
        &answers,
        &[],
        &p,
    );
    let mut files = BTreeMap::new();
    for s in &p.recipes.escalation {
        files.insert((fa.label.clone(), s.clone()), format!("/r/{s}.json"));
    }
    let v = extension_sources(&d, Some("/t.jsonl"), &files);
    let ids: Vec<&str> = v.iter().map(|x| x["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec![
            "rf:symbolic",
            "rf:d0_n:d2_clast_fon_mM1C",
            "rf:d0_n:d3_clast_fon_mM1C"
        ]
    );
}

#[test]
fn exception_faces_from_the_domain_code() {
    use rustred_campaign_core::domain::CaseDomain;
    use rustred_campaign_core::partition::DomainList;
    use rustred_campaign_core::poly::Poly;
    let n = 5;
    let p = |t: &str| Poly::parse_d_free(t, n).unwrap();
    // rule 0: case n2 = 1, n3 = 0, n4 = 0 (n0, n1 free); exception branches: n1 = 2 (affine) | n0^2 - 5 = 0 (nonlinear)
    let mut d0 = CaseDomain::free(n, Some(lines()));
    d0.fixed = vec![None, None, Some(1), Some(0), Some(0)];
    d0.exceptions = vec![vec![p("n1-2")], vec![p("n0^2-5")]];
    // rule 1: the corner face {}x{} only, no exception
    let mut d1 = CaseDomain::free(n, Some(lines()));
    d1.fixed = vec![Some(1), Some(1), Some(1), Some(0), Some(0)];
    let mut list = DomainList::new(n);
    list.push(rustred_campaign_core::cover::RuleRef::bare(0, 0, 10), d0);
    list.push(rustred_campaign_core::cover::RuleRef::bare(0, 1, 11), d1);
    let keys = vec![
        vec![3, 2, 1, 0, 0],
        vec![4, 2, 1, 0, 0],
        vec![2, 3, 1, 0, 0],
        vec![1, 1, 2, 0, 0],
    ];
    let prov = |e: usize| (format!("T{e}"), e as i64, format!("sha{e}"));
    let (faces, attributed, st) = exception_faces(&keys, &list, &lines(), 1_000, &prov);
    // [3,2,..] and [4,2,..] are cut out by branch 0 of rule 0: ONE parametric face, n1 fixed at 2, n0 free (1 direction)
    assert_eq!(faces.len(), 1, "{faces:#?}");
    let x = faces[0].exception.as_ref().unwrap();
    assert_eq!((x.entry, x.branch), (0, 0));
    assert_eq!(
        x.case,
        json!({"frame": "PRESENTED", "fixed": [null, 2, 1, 0, 0], "equations": []})
    );
    assert_eq!(x.free_dims, 1);
    assert_eq!(faces[0].label, "x:{0,1}x{}:T0#0@b0");
    assert!(x.constrained, "the branch fixes a face slot");
    let lay = rustred_campaign_core::repairflow::faces::layers(&faces[0], &lines());
    assert_eq!(lay.len(), 2, "{lay:?}");
    assert_eq!(attributed.len(), 2);
    assert_eq!(st.attributed, 2);
    assert!(!st.truncated);
    // the evaluation budget bounds the attribution
    let (_, _, st2) = exception_faces(&keys, &list, &lines(), 1, &prov);
    assert!(st2.truncated);
    // affine literal text and a non-linear branch refused as a case
    assert_eq!(
        rustred_campaign_core::repairflow::faces::affine_text(&p("2*n3+n1-1"), n).unwrap(),
        "n1+2*n3-1"
    );
    let mut d2 = CaseDomain::free(n, Some(lines()));
    d2.exceptions = vec![vec![p("n0^2-5")]];
    assert!(rustred_campaign_core::repairflow::faces::exception_case(&d2, 0).is_err());
    // a plain face whose keys are all explained by exception faces gets no symbolic attempt of its own
    let mut fp = face(&[0], &[], vec![vec![3, 2, 1, 0, 0]]);
    fp.unattributed = 0;
    let plan = s_plan(
        &[faces[0].clone(), fp],
        &[ShadowClass::Skipped, ShadowClass::Skipped],
        &RepairPolicy::default(),
    );
    assert_eq!(plan, vec![None, Some(Class::PlainFaceExplained)]);
}

/// MEASUREMENT (ignored; reads real material, no child process, one thread): exception-face attribution of the open
/// keys of a node's import spec (`RF_SPEC` = import spec path, `RF_BASE` = its base directory).
#[test]
#[ignore]
fn measure_exception_faces_of_a_real_node() {
    use rustred_campaign_core::import::check::{CheckOptions, RowStatus};
    let spec = std::env::var("RF_SPEC").expect("RF_SPEC");
    let base = std::env::var("RF_BASE").unwrap_or_else(|_| "/".into());
    let text = std::fs::read_to_string(&spec).unwrap();
    let ts = rustred_campaign_core::import::TableSetSpec::from_json(&text).unwrap();
    let t0 = std::time::Instant::now();
    let set = rustred_campaign_core::import::load(&ts, std::path::Path::new(&base)).unwrap();
    let ev = rustred_campaign_core::import::Evidence::from_spec(&set, std::path::Path::new(&base))
        .unwrap();
    let res = rustred_campaign_core::import::check::check(
        &set,
        &ev,
        &CheckOptions {
            threads: 1,
            tier: None,
            max_rounds: None,
        },
    )
    .unwrap();
    let open: Vec<Vec<i32>> = res
        .rows
        .iter()
        .filter(|r| r.status == RowStatus::Open && r.open_status.as_deref() == Some("uncovered"))
        .map(|r| r.key.clone())
        .collect();
    let t1 = t0.elapsed().as_secs_f64();
    let src = rustred_campaign_core::partition::DispatcherSource::new(&set.disp);
    let prov = |e: usize| {
        use rustred_campaign_core::partition::DomainSource;
        let r = src.rule_ref(e);
        let i = &set.rules[r.table][r.record];
        (i.source.clone(), i.label, i.record_sha256.clone())
    };
    let t2 = std::time::Instant::now();
    let (faces, attributed, st) =
        exception_faces(&open, &src, set.frame.mask(), 2_000_000_000, &prov);
    let (plain, _) = group_open_keys(&open, set.frame.mask(), "open_keys_desc");
    let unexplained_plain = plain
        .iter()
        .filter(|f| f.open_keys.iter().any(|k| !attributed.contains(k)))
        .count();
    let sizes: Vec<usize> = faces.iter().map(|f| f.open_keys.len()).collect();
    let dims: Vec<u64> = faces
        .iter()
        .map(|f| f.exception.as_ref().unwrap().free_dims)
        .collect();
    println!(
        "{}",
        json!({"closure_keys": res.summary.keys_total, "open_uncovered": open.len(), "load_check_s": t1,
            "attribution_s": t2.elapsed().as_secs_f64(), "stats": st, "exception_faces": faces.len(),
            "keys_per_exception_face": sizes, "free_dims": dims, "plain_faces": plain.len(),
            "plain_faces_with_unattributed_keys": unexplained_plain,
            "largest": faces.iter().take(5).map(|f| json!({"face": f.label, "keys": f.open_keys.len(), "case": f.exception.as_ref().unwrap().case})).collect::<Vec<_>>()})
    );
}

/// MEASUREMENT (ignored; real material, no child process, one thread): first match of a binary arrival key file
/// (`RF_KEYS`: one signed byte per slot, arity of the spec's frame) against an import spec's TABLES (`RF_SPEC`);
/// writes the unserved keys (`RF_OUT`, JSONL) and prints their support signatures (the faces of the repair flow).
#[test]
#[ignore]
fn measure_unserved_arrivals() {
    let spec = std::env::var("RF_SPEC").expect("RF_SPEC");
    let keys = std::env::var("RF_KEYS").expect("RF_KEYS");
    let out = std::env::var("RF_OUT").expect("RF_OUT");
    let ts = rustred_campaign_core::import::TableSetSpec::from_json(
        &std::fs::read_to_string(&spec).unwrap(),
    )
    .unwrap();
    let t0 = std::time::Instant::now();
    let set = rustred_campaign_core::import::load(&ts, std::path::Path::new("/")).unwrap();
    let n = set.frame.arity();
    let lines = set.frame.mask().to_vec();
    let raw = std::fs::read(&keys).unwrap();
    assert_eq!(raw.len() % n, 0);
    let mut unserved: Vec<Vec<i32>> = vec![];
    let mut outside = 0u64;
    for c in raw.chunks_exact(n) {
        let k: Vec<i32> = c.iter().map(|&b| b as i8 as i32).collect();
        if !k
            .iter()
            .zip(&lines)
            .all(|(&v, &l)| if l { v >= 1 } else { v <= 0 })
        {
            outside += 1;
            continue;
        }
        if !set
            .disp
            .first_match(&k)
            .map(|d| d.is_covered())
            .unwrap_or(false)
        {
            unserved.push(k);
        }
    }
    let (faces, _) = group_open_keys(&unserved, &lines, "open_keys_desc");
    std::fs::write(
        &out,
        unserved
            .iter()
            .map(|k| format!("{}\n", json!(k)))
            .collect::<String>(),
    )
    .unwrap();
    println!(
        "{}",
        json!({"arrival_keys": raw.len() / n, "outside_chart": outside, "unserved": unserved.len(),
            "seconds": t0.elapsed().as_secs_f64(), "signatures": faces.len(),
            "faces": faces.iter().map(|f| json!({"face": f.label, "keys": f.open_keys.len(),
                "layers": rustred_campaign_core::repairflow::faces::layers(f, &lines)})).collect::<Vec<_>>()})
    );
}

#[test]
fn job_family_is_derived_from_the_frame_header() {
    use rustred_campaign_core::repairflow::faces::job_family_from_header;
    let h = json!({"family_source": "P", "original_family_source": "O", "node_order": {"slot_permutation": [1, 0]}});
    assert_eq!(
        job_family_from_header(&h).unwrap(),
        ("O".to_string(), "original_family_source")
    );
    let h = json!({"family_source": "P", "node_order": {"slot_permutation": null}});
    assert_eq!(
        job_family_from_header(&h).unwrap(),
        ("P".to_string(), "family_source")
    );
    assert!(
        job_family_from_header(
            &json!({"family_source": "P", "node_order": {"slot_permutation": [1, 0]}})
        )
        .is_err()
    );
    assert!(job_family_from_header(&json!({})).is_err());
    // real headers (read-only material; ignored when absent): RF_HEADERS = paths of table files, ':'-separated
    if let Ok(paths) = std::env::var("RF_HEADERS") {
        for p in paths.split(':') {
            let l = std::fs::read_to_string(p).unwrap();
            let h: serde_json::Value = serde_json::from_str(l.lines().next().unwrap()).unwrap();
            let (t, f) = job_family_from_header(&h).unwrap();
            println!("{p}: {f}, {} bytes", t.len());
        }
    }
}

/// MEASUREMENT (ignored; real material, in-process, no child): ROUTE 0b = the EXACT multi-term symmetry relation at the
/// key (verified group derived from the frame header's family, `termmin::symrel::symmetry_relation_at_key`) at given
/// keys (`RF_KEYS_JSONL`), restricted to one signature (`RF_FACE`, e.g. "{9}x{11,13,14,15}"); right-hand side checked
/// against the tables of `RF_SPEC` (first match) and the keys of the same set.
#[test]
#[ignore]
fn measure_route0b_symmetry_relations() {
    use rustred_campaign_core::independent_algebra::Independent;
    use rustred_campaign_core::termmin::symrel::{SymRelation, symmetry_relation_at_key};
    let spec = std::env::var("RF_SPEC").expect("RF_SPEC");
    let keys = std::env::var("RF_KEYS_JSONL").expect("RF_KEYS_JSONL");
    let want = std::env::var("RF_FACE").ok();
    let ts = rustred_campaign_core::import::TableSetSpec::from_json(
        &std::fs::read_to_string(&spec).unwrap(),
    )
    .unwrap();
    let base = std::path::Path::new("/");
    let set = rustred_campaign_core::import::load(&ts, base).unwrap();
    let lines = set.frame.mask().to_vec();
    let fsrc = ts.sources.iter().find(|s| s.id == ts.frame_from).unwrap();
    let h: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(fsrc.path.as_ref().unwrap())
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let fam = h["family_source"].as_str().map(str::to_string);
    let gs: rustred_campaign_core::import::symmetry::GroupSpec =
        serde_json::from_value(json!({})).unwrap();
    let (g, rep) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        base,
        &lines,
        set.cmp.presentation().new_to_old(),
        fam.as_deref(),
    )
    .unwrap();
    let all: Vec<Vec<i32>> = std::fs::read_to_string(&keys)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let keyset: std::collections::BTreeSet<Vec<i32>> = all.iter().cloned().collect();
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let mut out = vec![];
    let (mut served, mut tested, mut rhs_ok) = (0, 0, 0);
    let mut elems = std::collections::BTreeMap::new();
    for k in &all {
        let f = rustred_campaign_core::worklist::items::Face::of_key(k, &lines).map(|f| f.label());
        if want.is_some() && f != want {
            continue;
        }
        tested += 1;
        match symmetry_relation_at_key(&Independent, &g, k, &cmp, 65_536) {
            SymRelation::Relation {
                rhs,
                rows,
                elements,
                ..
            } => {
                served += 1;
                *elems.entry(format!("{elements:?}")).or_insert(0u64) += 1;
                let ok = rhs.iter().all(|(r, _)| {
                    r.iter().zip(&lines).any(|(&v, &l)| l && v <= 0)
                        || set
                            .disp
                            .first_match(r)
                            .map(|d| d.is_covered())
                            .unwrap_or(false)
                        || !keyset.contains(r) && false
                });
                if ok {
                    rhs_ok += 1;
                }
                out.push(json!({"key": k, "rhs_terms": rhs.len(), "rows": rows, "elements": elements, "rhs_served_by_tables": ok}));
            }
            SymRelation::NoRelation { rows, rank, .. } => {
                out.push(json!({"key": k, "no_relation": {"rows": rows, "rank": rank}}))
            }
            SymRelation::Unknown(w) => out.push(json!({"key": k, "unknown": w})),
        }
    }
    println!(
        "{}",
        json!({"group": format!("{rep:?}").chars().take(300).collect::<String>(), "face": want, "tested": tested,
            "served": served, "rhs_all_served_by_tables": rhs_ok, "elements": elems,
            "rhs_terms": out.iter().filter_map(|o| o["rhs_terms"].as_u64()).collect::<Vec<_>>(),
            "first": out.iter().take(3).collect::<Vec<_>>()})
    );
}

/// MEASUREMENT (ignored): route 0b with the SMALLEST element per key (`repairflow::symrel`), checked against a size
/// law 1 + (b+1)(c+1) + b + c right-hand-side terms with b, c = |key| at the slots `RF_LAW_SLOTS` (e.g. "13,14").
#[test]
#[ignore]
fn measure_route0b_smallest_element() {
    use rustred_campaign_core::independent_algebra::Independent;
    use rustred_campaign_core::repairflow::symrel::smallest_symmetry_relation;
    let spec = std::env::var("RF_SPEC").expect("RF_SPEC");
    let keys = std::env::var("RF_KEYS_JSONL").expect("RF_KEYS_JSONL");
    let want = std::env::var("RF_FACE").ok();
    let law: Vec<usize> = std::env::var("RF_LAW_SLOTS")
        .map(|s| s.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_default();
    let ts = rustred_campaign_core::import::TableSetSpec::from_json(
        &std::fs::read_to_string(&spec).unwrap(),
    )
    .unwrap();
    let base = std::path::Path::new("/");
    let set = rustred_campaign_core::import::load(&ts, base).unwrap();
    let lines = set.frame.mask().to_vec();
    let fsrc = ts.sources.iter().find(|s| s.id == ts.frame_from).unwrap();
    let h: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(fsrc.path.as_ref().unwrap())
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let fam = h["family_source"].as_str().map(str::to_string);
    let gs: rustred_campaign_core::import::symmetry::GroupSpec =
        serde_json::from_value(json!({})).unwrap();
    let (g, _) = rustred_campaign_core::import::symmetry::load_verified_group(
        &gs,
        base,
        &lines,
        set.cmp.presentation().new_to_old(),
        fam.as_deref(),
    )
    .unwrap();
    let cmp = |a: &[i32], b: &[i32]| set.cmp.cmp(a, b);
    let (mut tested, mut served, mut lawok, mut rhs_ok) = (0, 0, 0, 0);
    let mut best_el = std::collections::BTreeMap::new();
    let mut sizes_all = vec![];
    for l in std::fs::read_to_string(&keys).unwrap().lines() {
        let k: Vec<i32> = serde_json::from_str(l).unwrap();
        let f = rustred_campaign_core::worklist::items::Face::of_key(&k, &lines).map(|f| f.label());
        if want.is_some() && f != want {
            continue;
        }
        tested += 1;
        let r = smallest_symmetry_relation(&Independent, &g, &k, &cmp, 65_536).unwrap();
        if let Some(b) = &r.best {
            served += 1;
            *best_el.entry(b.element).or_insert(0u64) += 1;
            if law.len() == 2 {
                let (x, y) = ((-k[law[0]]).max(0) as usize, (-k[law[1]]).max(0) as usize);
                if b.rhs.len() == 1 + (x + 1) * (y + 1) + x + y {
                    lawok += 1;
                }
            }
            if b.rhs.iter().all(|(r, _)| {
                r.iter().zip(&lines).any(|(&v, &l)| l && v <= 0)
                    || set
                        .disp
                        .first_match(r)
                        .map(|d| d.is_covered())
                        .unwrap_or(false)
            }) {
                rhs_ok += 1;
            }
        }
        sizes_all.push(json!({"key": k, "best": r.best.as_ref().map(|b| (b.element, b.rhs.len())), "sizes": r.sizes,
            "rhs_keys": r.best.as_ref().map(|b| b.rhs.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>())}));
    }
    let actions: Vec<serde_json::Value> = g
        .group()
        .elements
        .iter()
        .enumerate()
        .map(|(i, e)| json!({"element": i, "lines": e.line_perm(&lines)}))
        .collect();
    println!(
        "{}",
        json!({"tested": tested, "served": served, "law_match": lawok, "rhs_all_served": rhs_ok,
            "best_element_counts": best_el, "elements": actions, "first": sizes_all.iter().collect::<Vec<_>>()})
    );
}

/// WRITER (ignored; reads a finished run's decisions): `face_recipe.v1` records of a node (`RF_DECISIONS` ->
/// `RF_OUT`).
#[test]
#[ignore]
fn write_face_recipe_records() {
    use rustred_campaign_core::repairflow::face_format::{Binding, face_recipe_records};
    let d: NodeDecisions = serde_json::from_str(
        &std::fs::read_to_string(std::env::var("RF_DECISIONS").unwrap()).unwrap(),
    )
    .unwrap();
    let recs = face_recipe_records(
        &d,
        &Binding {
            policy_digest: d.policy_digest.clone(),
            ..Default::default()
        },
    );
    std::fs::write(
        std::env::var("RF_OUT").unwrap(),
        recs.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    println!("{} records", recs.len());
}

#[test]
fn face_recipe_records_round_trip() {
    use rustred_campaign_core::repairflow::face_format::{
        Binding, face_recipe_records, read_face_record,
    };
    let fa = face(&[0], &[], vec![vec![2, 1, 1, 0, 0], vec![3, 1, 1, 0, 0]]);
    let fb = face(&[1], &[], vec![vec![1, 2, 1, 0, 0], vec![1, 3, 1, 0, 0]]);
    let mut answers = BTreeMap::new();
    answers.insert(
        fa.label.clone(),
        FaceAnswer::Candidates {
            job: "j".into(),
            candidates: vec![cand("a", 10, fa.open_keys.clone())],
        },
    );
    let p = RepairPolicy::default();
    let d = decide(
        "N",
        &[fa.clone(), fb.clone()],
        &[ShadowClass::Skipped, ShadowClass::Skipped],
        &answers,
        &[],
        &p,
    );
    let b = Binding {
        node_identity: Some("id".into()),
        policy_digest: d.policy_digest.clone(),
        ..Default::default()
    };
    let recs = face_recipe_records(&d, &b);
    assert_eq!(recs.len(), 2);
    for (r, f) in recs.iter().zip(&d.faces) {
        let back = read_face_record(&r.to_string()).unwrap();
        assert_eq!(back.face_id, f.face);
        assert_eq!(back.dotted, f.sd);
        assert_eq!(back.numerators, f.sn);
        assert_eq!(back.bound_to["node_identity"], "id");
        assert_eq!(back.raw, *r, "the whole record survives");
    }
    assert_eq!(
        read_face_record(&recs[0].to_string())
            .unwrap()
            .route_decision
            .as_deref(),
        Some("symbolic_rule")
    );
    // a helper-style record without our additive fields reads with nulls
    let h = read_face_record(r#"{"schema":"face_recipe.v1","id":"F1","signature":{"dotted":[9],"numerators":[11,13,14,15]}}"#).unwrap();
    assert_eq!((h.face_id.as_str(), h.route_decision), ("F1", None));
    assert!(
        read_face_record(r#"{"schema":"other.v1","signature":{"dotted":[],"numerators":[]}}"#)
            .is_err()
    );
}

/// MEASUREMENT (ignored): closure size, open keys (classified by status and face) and served-by-class counts of
/// nested import-spec variants (`RF_SPECS`, ':'-separated): the attribution of a closure's growth per object class.
#[test]
#[ignore]
fn measure_closure_variants() {
    use rustred_campaign_core::import::check::{CheckOptions, RowStatus, Server};
    for spec in std::env::var("RF_SPECS").expect("RF_SPECS").split(':') {
        let ts = rustred_campaign_core::import::TableSetSpec::from_json(
            &std::fs::read_to_string(spec).unwrap(),
        )
        .unwrap();
        let base = std::path::Path::new("/");
        let set = rustred_campaign_core::import::load(&ts, base).unwrap();
        let ev = rustred_campaign_core::import::Evidence::from_spec(&set, base).unwrap();
        let res = rustred_campaign_core::import::check::check(
            &set,
            &ev,
            &CheckOptions {
                threads: 4,
                tier: None,
                max_rounds: None,
            },
        )
        .unwrap();
        let lines = set.frame.mask().to_vec();
        let mut by: std::collections::BTreeMap<String, u64> = Default::default();
        let mut open: std::collections::BTreeMap<String, u64> = Default::default();
        for r in &res.rows {
            match r.status {
                RowStatus::Served => {
                    let c = match &r.server {
                        Some(Server::Rule { table, record }) => {
                            set.rules[*table][*record].class.clone()
                        }
                        Some(Server::Credit { .. }) => "credit".into(),
                        Some(Server::Relation { set: s, .. }) => {
                            set.classes.get(s).cloned().unwrap_or_else(|| s.clone())
                        }
                        None => "?".into(),
                    };
                    *by.entry(c).or_default() += 1;
                }
                RowStatus::Open => {
                    let f = rustred_campaign_core::worklist::items::Face::of_key(&r.key, &lines)
                        .map(|f| f.label())
                        .unwrap_or_default();
                    let st = r.open_status.clone().unwrap_or_default();
                    let cand = if r.open_reason.as_deref().unwrap_or("").contains("recipe") {
                        "recipe candidates left"
                    } else {
                        "no candidate"
                    };
                    *open.entry(format!("{st} | {cand} | {f}")).or_default() += 1;
                }
                RowStatus::Terminal => *by.entry("terminal".into()).or_default() += 1,
            }
        }
        println!(
            "{}",
            json!({"spec": spec, "closure_keys": res.summary.keys_total, "open": res.summary.open_total,
            "served_by_class": by, "open_classified": open, "solve_requests": res.solve_requests.len()})
        );
    }
}

#[test]
fn minimised_objects_are_accepted_only_when_tested_at_every_key_and_never_lose_keys() {
    use rustred_campaign_core::repairflow::minimise::judge_minimised;
    use std::collections::BTreeSet;
    let region: Vec<Vec<i32>> = (2..8).map(|d| vec![d, 1, 1, 0, 0]).collect();
    let original: BTreeSet<Vec<i32>> = region.iter().cloned().collect();
    // tested at ONE key only (the minimisation key): refused, whatever it gave there
    let one: BTreeMap<Vec<i32>, bool> = [(region[0].clone(), true)].into_iter().collect();
    let v = judge_minimised(&region, &original, &one);
    assert!(!v.accept, "{v:?}");
    assert_eq!(v.tested, "1 of 6 keys of the region");
    // tested everywhere, but it DROPS a sub-locus (d = 2, 3): accepted only with the lost keys as a guard sub-locus
    let all: BTreeMap<Vec<i32>, bool> = region.iter().map(|k| (k.clone(), k[0] >= 4)).collect();
    let v = judge_minimised(&region, &original, &all);
    assert!(v.accept);
    assert_eq!(v.lost_keys, vec![vec![2, 1, 1, 0, 0], vec![3, 1, 1, 0, 0]]);
    // no key disappears: minimised-served + guard (served by the original) = what the original served
    let mut served: BTreeSet<Vec<i32>> = v.accepted_keys.iter().cloned().collect();
    served.extend(v.lost_keys.iter().cloned());
    assert_eq!(served, original);
    // serving nothing: refused
    let none: BTreeMap<Vec<i32>, bool> = region.iter().map(|k| (k.clone(), false)).collect();
    assert!(!judge_minimised(&region, &original, &none).accept);
}
