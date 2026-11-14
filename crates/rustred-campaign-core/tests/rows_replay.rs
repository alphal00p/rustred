//! Evidence level `replay` (R40 U-12 / R41 P-3): the record of `replay_evidence` on real rules, and its planted
//! controls (a dropped exception branch makes the guarded witness locus a tested locus that hits a pole; a planted
//! denominator vanishing at a served key is found by the pole scan; an elided record is not established), plus a
//! locus rule with affine case equations from the engine's own test data (T225, engine node/testdata
//! locus_r38_gf1.table.jsonl, family text embedded in its header).

mod rows_common;

use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    LocusStatus, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, Verdict,
    identity_test, replay_evidence, rule_digest,
};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::Value;

fn f6_rule(id: &str) -> Option<(Value, Vec<Vec<i64>>)> {
    let o = oracle()?;
    let case = o
        .jsonl("span/F6_span_cases.jsonl")
        .into_iter()
        .find(|c| c["id"] == id)
        .unwrap();
    let line = f6_line(&o, &case)?;
    let v: Value = serde_json::from_str(line.trim_end()).unwrap();
    let r = if v.get("rules").is_some() {
        v["rules"][0].clone()
    } else {
        v
    };
    let mut keys = Vec::new();
    for k in ["C2", "C5"] {
        for p in case["clean"][0][k].as_array().into_iter().flatten() {
            keys.push(
                p["point"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_i64().unwrap())
                    .collect(),
            );
        }
    }
    Some((r, keys))
}

#[test]
fn rows_replay_record_fc000_and_fd006() {
    let fam = t162_family();
    let opts = RowsOptions::default();
    for id in ["f000", "fd006", "f111"] {
        let Some((r, keys)) = f6_rule(id) else { return };
        let rule = RowsRule::from_value(r, 20).unwrap();
        let rec = replay_evidence(&fam, &rule, &keys, &ReplayPolicy::default(), &opts);
        let loci: Vec<String> = rec.loci.iter().map(|l| format!("{:?}", l.status)).collect();
        println!(
            "{id}: level {:?} digest {} served {} applicable {} poles {} identity {} points_passed {} loci {:?}",
            rec.level,
            &rec.rule_digest[..16],
            rec.served_keys,
            rec.pole_scan.keys_applicable,
            rec.pole_scan.poles.len(),
            rec.identity.verdict.kind(),
            rec.identity.points_passed,
            loci
        );
        assert_eq!(rec.level, ReplayLevel::Established, "{id}");
        assert_eq!(rec.schema, "rustred.campaign.replay_evidence.v1");
        assert_eq!(rec.rule_digest.len(), 64);
        assert!(rec.pole_scan.poles.is_empty());
        // the record serialises (the ledger cites it)
        let js = serde_json::to_string(&rec).unwrap();
        assert!(js.contains("\"rule_digest\""));
        // every d-free witness locus of these rules is one of its exception branches
        assert!(
            rec.loci
                .iter()
                .all(|l| matches!(l.status, LocusStatus::Guarded | LocusStatus::Parameter))
        );
    }
}

#[test]
fn rows_replay_planted_controls() {
    let fam = t162_family();
    let opts = RowsOptions::default();
    let Some((r, keys)) = f6_rule("f111") else {
        return;
    };
    // (1) dropped exception branch [-1+n3]: the witness locus -1+n3 is no longer guarded; its point hits the
    //     denominator (1-n3-n2+n2*n3) = (1-n2)(1-n3): identity Pole there -> not established
    let mut dropped = r.clone();
    dropped["exceptions"] = serde_json::json!([["-1+n2"], ["n9"]]);
    let rule = RowsRule::from_value(dropped, 20).unwrap();
    let rec = replay_evidence(&fam, &rule, &keys, &ReplayPolicy::default(), &opts);
    println!(
        "dropped exception: {:?} loci {:?}",
        rec.level,
        rec.loci.iter().map(|l| &l.status).collect::<Vec<_>>()
    );
    assert!(matches!(rec.level, ReplayLevel::NotEstablished { .. }));
    assert!(
        rec.loci
            .iter()
            .any(|l| matches!(l.status, LocusStatus::Tested { .. }))
    );
    assert!(matches!(rec.identity.verdict, Verdict::Pole { .. }));
    // (2) the pole scan at served keys: a key ON the dropped branch is served -> a pole hit
    let on_locus: Vec<i64> = vec![1, 2, 3, 1, 1, 1, 1, 1, 1, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let rec = replay_evidence(
        &fam,
        &rule,
        std::slice::from_ref(&on_locus),
        &ReplayPolicy::default(),
        &opts,
    );
    assert_eq!(rec.pole_scan.poles.len(), 1, "{:?}", rec.pole_scan);
    assert_eq!(rec.pole_scan.poles[0].key, on_locus);
    // (3) elided witness loci: never established
    let mut elided = r.clone();
    elided["witness_loci"] = serde_json::json!(["elided"]);
    let rule = RowsRule::from_value(elided, 20).unwrap();
    let rec = replay_evidence(&fam, &rule, &keys, &ReplayPolicy::default(), &opts);
    assert!(matches!(rec.level, ReplayLevel::NotEstablished { .. }));
    // (4) a raw pivot that disagrees with the recenter refuses the rule
    let mut rp = r.clone();
    rp["recenter"][2] = serde_json::json!(0);
    let rule = RowsRule::from_value(rp, 20).unwrap();
    let v = identity_test(&fam, &rule, &[PointSpec::Generic], &opts);
    assert!(
        matches!(v.verdict, Verdict::Unsupported(_)),
        "{:?}",
        v.verdict
    );
    // the digest changes with any coefficient
    let a = rule_digest(&RowsRule::from_value(r.clone(), 20).unwrap());
    let mut r2 = r.clone();
    r2["rhs"][0][1] = Value::String(format!("{}+0", r2["rhs"][0][1].as_str().unwrap()));
    assert_ne!(a, rule_digest(&RowsRule::from_value(r2, 20).unwrap()));
}

#[test]
fn rows_replay_engine_locus_rule_with_affine_equations() {
    let text = data("locus_r38_gf1.table.jsonl");
    let mut lines = text.lines();
    let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    let fam = Family::parse(header["family_source"].as_str().unwrap()).unwrap();
    let mut n = 0;
    for l in lines {
        for rule in RowsRule::from_json_line(l, fam.n).unwrap() {
            assert!(!rule.equations.is_empty());
            let opts = RowsOptions::default();
            let v = identity_test(
                &fam,
                &rule,
                &[PointSpec::Generic, PointSpec::Generic],
                &opts,
            );
            println!("{}", summary("T225 locus r38", &v));
            assert_eq!(v.verdict, Verdict::Pass, "{:?}", v.verdict);
            let rec = replay_evidence(&fam, &rule, &[], &ReplayPolicy::default(), &opts);
            println!("T225 locus r38 replay: {:?}", rec.level);
            n += 1;
        }
    }
    assert_eq!(n, 1);
}
