//! Unit tests of the pure emission layer (synthetic arity-2 / arity-3 records; no node knowledge).
use super::body::*;
use super::class::*;
use super::ladder::*;
use super::validate::*;
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

/// The stored toy rule of the independent checker's tests (crates/rustred-proof/src/wcert/tests.rs `stored`): the two
/// binding hashes must be the checker's (reference values from fc_tool.py's definitions).
const TOY: &str = r#"{"type": "rule", "chart": "10", "index": 0, "case": "toy", "fixed": [null, null], "equations": [], "target": "I(n0,n1)", "rhs": [["I(n0-1,n1)", "-(n0-2+n1)*(d-2*n0+2)/((n0-2)*(n0-1))"], ["I(n0-1,n1-1)", "-1/(n0-1)"], ["I(n0-2,n1+1)", "-(d-2*n0+2)/(n0-1)"]], "exceptions": [["-1+n0"], ["-2+n0"]], "recenter": [-1, 0]}"#;

#[test]
fn body_hashes_equal_the_checkers_binding_hashes() {
    let ids = body_ids(TOY).unwrap();
    assert_eq!(
        ids.relation_sha256,
        "1fa2f6ada10cc334dcc6a5ca65859c2bbe99c7434b1e1c492404e2176fc12c19"
    );
    assert_eq!(
        ids.domain_sha256,
        "de62e78251438d2384fdfd6b972bfb3c5c09f9d480477ec6dbfd6af0598adbf7"
    );
}

#[test]
fn body_hash_is_stable_under_renumbering_provenance_and_derivation() {
    let a = body_ids(TOY).unwrap();
    let renum = crate::economy::improve::set_index(TOY, 917).unwrap();
    assert_eq!(body_ids(&renum).unwrap(), a);
    let with_origin = TOY.replace("\"recenter\"", "\"origin\": {\"x\": 1}, \"recenter\"");
    assert_eq!(body_ids(&with_origin).unwrap(), a);
    let compact: String =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(TOY).unwrap()).unwrap();
    assert_eq!(
        body_ids(&compact).unwrap().body_sha256,
        a.body_sha256,
        "whitespace / member order"
    );
    let d = attach_derivation(TOY, &json!({"schema": SCHEMA})).unwrap();
    assert_eq!(body_ids(&d).unwrap(), a);
    // a coefficient text, a branch, the case: other bodies
    assert_ne!(
        body_ids(&TOY.replace("-1/(n0-1)", "-2/(n0-1)"))
            .unwrap()
            .relation_sha256,
        a.relation_sha256
    );
    let other_dom = TOY.replace("[\"-2+n0\"]", "[\"-3+n0\"]");
    assert_ne!(body_ids(&other_dom).unwrap().domain_sha256, a.domain_sha256);
    assert_ne!(
        body_ids(&TOY.replace("\"toy\"", "\"toy2\""))
            .unwrap()
            .body_sha256,
        a.body_sha256
    );
}

#[test]
fn derivation_member_round_trip() {
    let d = attach_derivation(TOY, &json!({"schema": SCHEMA, "x": [1, 2]})).unwrap();
    assert!(
        d.ends_with("\"derivation\":{\"schema\":\"rustred.campaign.emission.v1\",\"x\":[1,2]}}")
    );
    let (body, raw) = strip_derivation(&d).unwrap();
    assert_eq!(body, TOY);
    assert_eq!(
        raw.as_deref(),
        Some("{\"schema\":\"rustred.campaign.emission.v1\",\"x\":[1,2]}")
    );
    assert!(
        attach_derivation(&d, &json!({})).is_err(),
        "a body never carries two"
    );
    // a derivation as FIRST member (hand-made line) is stripped too
    let first = "{\"derivation\":{\"a\":1},\"type\":\"rule\",\"index\":0}";
    assert_eq!(
        strip_derivation(first).unwrap().0,
        "{\"type\":\"rule\",\"index\":0}"
    );
}

fn summary(class: &str, rows: &str, open: u64) -> CheckSummary {
    let mut loci = BTreeMap::new();
    loci.insert("EMPTY_CONSTANT".to_string(), 1);
    if open > 0 {
        loci.insert("OPEN".to_string(), open);
    }
    CheckSummary {
        rows: rows.into(),
        identity: "IDENTITY_EXACT".into(),
        binding: "BINDING_EQUAL".into(),
        factors: "PRODUCER_LIST_EQUAL".into(),
        loci,
        checker_class: class.into(),
        relation_sha256: String::new(),
        domain_sha256: String::new(),
        cert_sha256: String::new(),
        open: vec![],
    }
}

#[test]
fn class_of_derives_only_from_the_check_and_the_evidence() {
    let ev = Evidence::default();
    let w = class_of(
        Kind::Lifted,
        Origin::Lift,
        Some(&summary("WHOLE_DOMAIN", "ROWS_VERIFIED", 0)),
        &ev,
    );
    assert_eq!(w.class, ClassWord::WholeDomain);
    let w = class_of(
        Kind::Lifted,
        Origin::Lift,
        Some(&summary("WHOLE_DOMAIN", "ROWS_VERIFIED_BY_RECOVERY", 0)),
        &ev,
    );
    assert_eq!(w.class, ClassWord::WholeDomain);
    // v0 (rows not verified): none, typed
    let n = class_of(
        Kind::Lifted,
        Origin::Lift,
        Some(&summary(
            "NOT_WHOLE_DOMAIN",
            "ROWS_NOT_INDEPENDENTLY_VERIFIED",
            0,
        )),
        &ev,
    );
    assert_eq!(n.class, ClassWord::None);
    assert!(n.reason.unwrap().starts_with("rows_not_verified"));
    // open loci without per-key evidence: none; with complete proven cone evidence: the cone class
    let o = summary("OPEN_LOCI", "ROWS_VERIFIED", 1);
    assert_eq!(
        class_of(Kind::Lifted, Origin::Lift, Some(&o), &ev).class,
        ClassWord::None
    );
    let cone = ConeEvidence {
        cone: "simplex:39".into(),
        scope: "open-locus keys of the domain".into(),
        keys: 0,
        keys_sha256: sha256_hex(b""),
        complete: true,
        proven: 0,
        position: None,
        prefix_sha256: None,
    };
    let evc = Evidence {
        cone: Some(cone.clone()),
        ..Default::default()
    };
    assert_eq!(
        class_of(Kind::Lifted, Origin::Lift, Some(&o), &evc).class,
        ClassWord::EveryKeyInCone
    );
    let incomplete = Evidence {
        cone: Some(ConeEvidence {
            complete: false,
            ..cone
        }),
        ..Default::default()
    };
    assert_eq!(
        class_of(Kind::Lifted, Origin::Lift, Some(&o), &incomplete).class,
        ClassWord::None
    );
    // sampled: never for a lift, kept for a legacy import
    let evs = Evidence {
        sample: Some(SampleEvidence {
            keys: 300,
            proven: 300,
            keys_sha256: String::new(),
            how: "test".into(),
        }),
        ..Default::default()
    };
    assert_eq!(
        class_of(Kind::Lifted, Origin::Lift, None, &evs).class,
        ClassWord::None
    );
    assert_eq!(
        class_of(Kind::Legacy, Origin::Import, None, &evs).class,
        ClassWord::Sampled
    );
    assert_eq!(
        class_of(Kind::Legacy, Origin::Import, None, &Evidence::default())
            .reason
            .as_deref(),
        Some("legacy_unchecked")
    );
    // point / alias kinds
    let pt = Evidence {
        exact_at_key: Some(true),
        ..Default::default()
    };
    assert_eq!(
        class_of(Kind::Point, Origin::Lift, None, &pt).class,
        ClassWord::WholeDomain
    );
    let al = Evidence {
        alias_target: Some(ClassWord::Sampled),
        ..Default::default()
    };
    assert_eq!(
        class_of(Kind::Alias, Origin::Import, None, &al).class,
        ClassWord::Sampled
    );
}

#[test]
fn policy_defaults_are_the_implementers_ruling() {
    let p = Policy::default();
    let c = |class| Classed {
        class,
        reason: None,
        evidence: vec![],
    };
    assert!(
        p.admit(
            Origin::Lift,
            Liveness::Live,
            &c(ClassWord::WholeDomain),
            false,
            0
        )
        .is_ok()
    );
    assert!(
        p.admit(Origin::Lift, Liveness::Live, &c(ClassWord::None), false, 0)
            .is_err()
    );
    assert!(
        p.admit(
            Origin::Lift,
            Liveness::Unknown,
            &c(ClassWord::None),
            false,
            0
        )
        .is_err()
    );
    assert!(
        p.admit(
            Origin::Lift,
            Liveness::Live,
            &c(ClassWord::Sampled),
            false,
            999
        )
        .is_err()
    );
    assert!(
        p.admit(
            Origin::Lift,
            Liveness::Live,
            &c(ClassWord::EveryKeyInCone),
            false,
            0
        )
        .is_err()
    );
    assert!(
        p.admit(
            Origin::Lift,
            Liveness::Live,
            &c(ClassWord::EveryKeyInCone),
            true,
            0
        )
        .is_ok()
    );
    assert!(
        p.admit(
            Origin::Import,
            Liveness::Live,
            &c(ClassWord::Sampled),
            false,
            300
        )
        .is_ok()
    );
    assert!(
        p.admit(
            Origin::Import,
            Liveness::Live,
            &c(ClassWord::Sampled),
            false,
            10
        )
        .is_err()
    );
    assert!(
        p.admit(
            Origin::Import,
            Liveness::Live,
            &c(ClassWord::None),
            false,
            0
        )
        .is_err()
    );
    assert!(
        p.admit(Origin::Lift, Liveness::Dead, &c(ClassWord::None), false, 0)
            .is_ok()
    );
}

#[test]
fn branch_plans() {
    let free = vec![None, None, None];
    assert_eq!(
        plan_branch(&["-1+n1".into()], &free).unwrap(),
        BranchPlan::Fix(vec![(1, 1)])
    );
    assert_eq!(
        plan_branch(&["n2".into()], &free).unwrap(),
        BranchPlan::Fix(vec![(2, 0)])
    );
    assert_eq!(
        plan_branch(&["2*n2-3".into()], &free).unwrap(),
        BranchPlan::Empty
    );
    assert_eq!(
        plan_branch(&["-1+n1".into()], &[None, Some(2), None]).unwrap(),
        BranchPlan::Empty
    );
    assert_eq!(
        plan_branch(&["n1-n2".into()], &free).unwrap(),
        BranchPlan::Equation("n1-n2".into())
    );
    assert!(matches!(
        plan_branch(&["n1^2+n2^2-25".into()], &free).unwrap(),
        BranchPlan::Curve(_)
    ));
    assert_eq!(
        plan_branch(&["-1+n1".into(), "n2+3".into()], &free).unwrap(),
        BranchPlan::Fix(vec![(1, 1), (2, -3)])
    );
    assert_eq!(slot_minus_text(8, 1), "-1+n8");
    assert_eq!(slot_minus_text(13, -1), "1+n13");
    // an open locus: the first generator that fixes a slot
    assert_eq!(
        open_locus_branch(&["n0*n1-4".into(), "-1+n1".into()], &free).unwrap(),
        BranchPlan::Fix(vec![(1, 1)])
    );
}

#[test]
fn child_record_fixes_the_slot_in_target_and_case() {
    let parent = json!({"type": "rule", "chart": "110", "index": 5, "case": "Coordinate(CoordinateCase { fixed: [Some(1), None, None] })",
        "fixed": [1, null, null], "equations": [], "target": "I(1,n1,n2)", "rhs": [["I(1,n1,n2-1)", "1"]], "exceptions": [["n2"]]});
    let c = child_record(
        &parent,
        &BranchPlan::Fix(vec![(1, 1)]),
        &[vec!["n2".into()]],
    )
    .unwrap();
    assert_eq!(c["fixed"], json!([1, 1, null]));
    assert_eq!(c["target"], json!("I(1,1,n2)"));
    assert_eq!(
        c["case"],
        json!("Coordinate(CoordinateCase { fixed: [Some(1), Some(1), None] })")
    );
    assert_eq!(c["rhs"], parent["rhs"]);
    assert_eq!(c["exceptions"], json!([["n2"]]));
}

const HEADER: &str = r#"{"type":"header","schema":"table_dump.v1","mask":"110","denominators":3,"power_shifts":["0","0","0"],"charts":["110"]}"#;

fn rec(fixed: serde_json::Value, target: &str, exc: serde_json::Value) -> serde_json::Value {
    json!({"type": "rule", "chart": "110", "index": 0, "case": "c", "fixed": fixed, "equations": [], "target": target,
           "rhs": [], "exceptions": exc})
}

#[test]
fn partition_proof_accepts_a_ladder_and_refuses_an_overlap_or_a_gap() {
    let tmp = std::env::temp_dir().join(format!("gencheck-part-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // original: I(n0, n1, n2) minus {n2 = 0}; ladder: A = original minus {n1 = 1}, B = {n1 = 1} minus {n0 = 1}, C = {n1 = 1, n0 = 1}
    let o = rec(json!([null, null, null]), "I(n0,n1,n2)", json!([["n2"]]));
    let a = rec(
        json!([null, null, null]),
        "I(n0,n1,n2)",
        json!([["n2"], ["-1+n1"]]),
    );
    let b = rec(
        json!([null, 1, null]),
        "I(n0,1,n2)",
        json!([["n2"], ["-1+n0"]]),
    );
    let c = rec(json!([1, 1, null]), "I(1,1,n2)", json!([["n2"]]));
    let (ok, checks) = prove_partition(
        HEADER,
        &[],
        &o,
        &[a.clone(), b.clone(), c.clone()],
        &tmp,
        &[false],
    )
    .unwrap();
    assert!(ok, "{checks:?}");
    // one piece with the original's domain: a partition by identity
    let same = rec(json!([null, null, null]), "I(n0,n1,n2)", json!([["n2"]]));
    let (ok, checks) = prove_partition(HEADER, &[], &o, &[same], &tmp, &[false]).unwrap();
    assert!(ok && checks[0].check.starts_with("identity"), "{checks:?}");
    // a gap (C missing): the original stays live after the pieces
    let (ok, _) =
        prove_partition(HEADER, &[], &o, &[a.clone(), b.clone()], &tmp, &[false]).unwrap();
    assert!(!ok);
    // an overlap (B without its branch n0 = 1 meets C)
    let b2 = rec(json!([null, 1, null]), "I(n0,1,n2)", json!([["n2"]]));
    let (ok, checks) = prove_partition(HEADER, &[], &o, &[a, b2, c], &tmp, &[false]).unwrap();
    assert!(!ok, "{checks:#?}");
    std::fs::remove_dir_all(&tmp).ok();
}

fn side_map<'a>(files: &'a [(&'a str, Vec<u8>)]) -> impl Fn(&str, &str) -> Option<Vec<u8>> + 'a {
    move |area: &str, sha: &str| {
        files
            .iter()
            .find(|(a, b)| *a == area && sha256_hex(b) == sha)
            .map(|(_, b)| b.clone())
    }
}

#[test]
fn validator_refuses_a_hand_edited_class_and_a_changed_body() {
    let ids = body_ids(TOY).unwrap();
    let check_line = format!(
        r#"{{"schema":"rustred.proof-kernel.weight-cert.v1","mode":"weight-cert","cert":{{"sha256":"{c}"}},"rule":{{"target_rhs_sha256":"{r}","domain_sha256":"{d}"}},"rows":{{"verdict":"ROWS_VERIFIED_BY_RECOVERY"}},"identity":{{"verdict":"IDENTITY_EXACT"}},"binding":{{"verdict":"BINDING_EQUAL"}},"factors":{{"producer":"PRODUCER_LIST_ABSENT"}},"loci":[],"loci_counts":{{"EMPTY_CONSTANT":1,"IN_EXCEPTION":2}},"class":"WHOLE_DOMAIN"}}"#,
        c = sha256_hex(b"cert"),
        r = ids.relation_sha256,
        d = ids.domain_sha256
    );
    let s = CheckSummary::parse(&check_line).unwrap();
    let policy = Policy::default();
    let policy_text = serde_json::to_vec(&policy).unwrap();
    let classed = class_of(Kind::Lifted, Origin::Lift, Some(&s), &Evidence::default());
    assert_eq!(classed.class, ClassWord::WholeDomain);
    let parts = Parts {
        kind: Kind::Lifted,
        origin: Origin::Lift,
        body_sha256: ids.body_sha256.clone(),
        producer: None,
        certificate: Some((sha256_hex(b"cert"), 4, "v0+rows".into())),
        check: Ok((sha256_hex(b"checker"), sha256_hex(check_line.as_bytes()), s)),
        classed,
        cone: None,
        sample: None,
        ladder: None,
        policy: (policy.name.clone(), sha256_hex(&policy_text)),
    };
    let d = derivation(&parts);
    assert!(
        d.to_string().len() < 1024,
        "derivation member {} bytes",
        d.to_string().len()
    );
    let line = attach_derivation(TOY, &d).unwrap();
    let files = [
        (AREA_CERT, b"cert".to_vec()),
        (AREA_CHECK, check_line.clone().into_bytes()),
        (AREA_POLICY, policy_text),
    ];
    let side = side_map(&files);
    let none_alias = |_: i64| None;
    let v = check_record(&line, 0, &side, &none_alias);
    assert!(v.ok(), "{:?}", v.problems);
    assert_eq!(v.class, Some(ClassWord::WholeDomain));
    // index != position
    assert!(!check_record(&line, 3, &side, &none_alias).ok());
    // a hand-edited class (stronger than the evidence): refused
    let weak = line.replace("\"word\":\"whole domain\"", "\"word\":\"sampled\"");
    let v = check_record(&weak, 0, &side, &none_alias);
    assert!(
        v.problems.iter().any(|p| p.contains("stated class")),
        "{:?}",
        v.problems
    );
    // stated stages edited
    let edited = line.replace(
        "\"rows\":\"ROWS_VERIFIED_BY_RECOVERY\"",
        "\"rows\":\"ROWS_VERIFIED\"",
    );
    assert!(!check_record(&edited, 0, &side, &none_alias).ok());
    // a coefficient changed after emission: the body is another
    let changed = line.replace("-1/(n0-1)", "-2/(n0-1)");
    let v = check_record(&changed, 0, &side, &none_alias);
    assert!(
        v.problems.iter().any(|p| p.contains("body_sha256")),
        "{:?}",
        v.problems
    );
    // a missing check output
    let v = check_record(&line, 0, &side_map(&files[..1]), &none_alias);
    assert!(
        v.problems.iter().any(|p| p.contains("missing")),
        "{:?}",
        v.problems
    );
    // no derivation at all
    assert!(!check_record(TOY, 0, &side, &none_alias).ok());
}
