//! T178 order B: the exact partition against the checker's reference (h11b B1-B4) in BOTH elided references
//! (plan WP-I3, dry-run review F8): `ElidedPolicy::Ordinary` = 88 faces (87 generic + 1 locus-only), 5 loci,
//! 321 U + 284 F classes, n2 with states {1,2,3,>=4}, box 409 = 407 generic + 2 locus; `ElidedPolicy::FailClosed`
//! = the NEW reference (face list and the extra box keys printed and asserted for consistency).
//!
//! Run (release, ~1 min): `light_build.sh part test -p rustred-campaign-core --release --test partition_t178b --
//! --ignored --nocapture`.

mod partition_common;

use std::collections::BTreeSet;

use partition_common::*;
use rustred_campaign_core::key::Key;
use rustred_campaign_core::partition::{
    DispatcherSource, ElidedPolicy, FaceKind, LiteralTests, PartitionOptions, PartitionReport,
    gap_partition,
};

fn run(raw: bool, elided: ElidedPolicy, tests: LiteralTests) -> Option<PartitionReport> {
    let fx = fx()?;
    let (disp, _) = t178_dispatcher(&fx, raw);
    let src = DispatcherSource::new(&disp);
    let opts = PartitionOptions {
        elided,
        literal_tests: tests,
        ..Default::default()
    };
    let rep = gap_partition(&src, &t178_frame(), &opts, None).unwrap();
    println!(
        "T178B raw={raw} {elided:?} {tests:?}: inputs {:?}; nodes {} memo hits {} U {} F {} faces {} ({} generic, {} locus-only) loci {} ({} unresolved) in {} ms; unknown tests {}",
        rep.nf.stats,
        rep.stats.nodes,
        rep.stats.memo_hits,
        rep.n_u(),
        rep.n_f(),
        rep.faces.len(),
        rep.faces
            .iter()
            .filter(|f| f.kind == FaceKind::Generic)
            .count(),
        rep.faces
            .iter()
            .filter(|f| f.kind == FaceKind::LocusOnly)
            .count(),
        rep.resolution.as_ref().map_or(0, |r| r.loci.len()),
        rep.resolution.as_ref().map_or(0, |r| r.n_unresolved()),
        rep.millis,
        rep.stats.unknown_literal_tests,
    );
    Some(rep)
}

fn face_set(rep: &PartitionReport, kind: Option<FaceKind>) -> BTreeSet<(Vec<usize>, Vec<usize>)> {
    rep.faces
        .iter()
        .filter(|f| kind.is_none_or(|k| f.kind == k))
        .map(|f| f.key())
        .collect()
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_ordinary_reference_compatible() {
    let Some(rep) = run(
        false,
        ElidedPolicy::Ordinary,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    // states: lines {1,2,>=3} except n2 {1,2,3,>=4}; numerators {0,-1,<=-2}
    for (s, st) in rep.states.iter().enumerate() {
        let want = if s == 2 { 4 } else { 3 };
        assert_eq!(st.len(), want, "slot {s}: {st:?}");
    }
    assert_eq!(rep.nf.stats.distinct, 3169);
    assert_eq!((rep.n_u(), rep.n_f()), (321, 284));
    let gens = face_set(&rep, Some(FaceKind::Generic));
    let loc = face_set(&rep, Some(FaceKind::LocusOnly));
    for f in &loc {
        println!("locus-only face {}", face_str(f));
    }
    let res = rep.resolution.as_ref().unwrap();
    for l in &res.loci {
        println!(
            "locus {} face {} eqs {:?} dim {} classes {} witness {:?} unresolved {}",
            l.id,
            face_str(&l.face),
            l.equations,
            l.dim,
            l.n_classes,
            l.witness.first(),
            l.unresolved
        );
    }
    assert_eq!(gens.len(), 87);
    assert_eq!(loc.len(), 1);
    assert!(loc.contains(&(vec![6, 7, 9], vec![])));
    assert_eq!(res.loci.len(), 5);
    let cen = rep.census(&t178_box()).unwrap();
    println!(
        "box: {} uncovered = {} generic + {} locus; covered keys in U classes {}; uncovered in C {}",
        cen.uncovered.len(),
        cen.n_generic(),
        cen.n_locus(),
        cen.covered_in_u.len(),
        cen.uncovered_in_c.len()
    );
    assert_eq!(cen.uncovered.len(), 409);
    assert_eq!((cen.n_generic(), cen.n_locus()), (407, 2));
    assert!(cen.uncovered_in_c.is_empty());
    assert_eq!(cen.covered_in_u.len(), 1);
    // the box gaps lie on listed faces
    let all = face_set(&rep, None);
    assert!(cen.faces().is_subset(&all));
    // reference 409 keys
    if let Some(k3) = k3() {
        let refu: BTreeSet<Key> = lines(k3.join("T178B/F5_box_uncovered_409.jsonl"))
            .iter()
            .map(as_key)
            .collect();
        let mine: BTreeSet<Key> = cen.uncovered.iter().map(|x| x.0.clone()).collect();
        assert_eq!(mine, refu);
    }
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_ordinary_exact_tests_same_faces() {
    let Some(a) = run(
        false,
        ElidedPolicy::Ordinary,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    let Some(b) = run(false, ElidedPolicy::Ordinary, LiteralTests::Exact) else {
        return;
    };
    // exact literal tests: same U classes, same generic and locus-only faces, same loci count; F may only shrink
    let ua: BTreeSet<_> = a
        .classes
        .iter()
        .filter(|c| c.label == rustred_campaign_core::partition::Label::Uncovered)
        .collect();
    let ub: BTreeSet<_> = b
        .classes
        .iter()
        .filter(|c| c.label == rustred_campaign_core::partition::Label::Uncovered)
        .collect();
    println!(
        "exact mode: U {} F {} (reference-compatible U {} F {})",
        b.n_u(),
        b.n_f(),
        a.n_u(),
        a.n_f()
    );
    assert_eq!(ua, ub);
    assert!(b.n_f() <= a.n_f());
    assert_eq!(face_set(&a, None), face_set(&b, None));
    assert_eq!(
        face_set(&a, Some(FaceKind::Generic)),
        face_set(&b, Some(FaceKind::Generic))
    );
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_oracle_equals_union_cover() {
    let Some(fx) = fx() else { return };
    let (disp, _) = t178_dispatcher(&fx, false);
    let src = DispatcherSource::new(&disp);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        resolve_loci: false,
        ..Default::default()
    };
    let rep = gap_partition(&src, &t178_frame(), &opts, None).unwrap();
    let keys = t178_box();
    let mut n = 0;
    for k in keys.iter().step_by(7) {
        let u = rep.uncovered(k).unwrap();
        let (cov, _) = disp.union_cover(k).unwrap();
        assert_eq!(u, cov.is_empty(), "{k:?}");
        n += 1;
    }
    println!("oracle == Dispatcher::union_cover on {n} box keys");
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_fail_closed_new_reference() {
    let Some(fx) = fx() else { return };
    let Some(ord) = run(
        true,
        ElidedPolicy::Ordinary,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    let Some(fc) = run(
        true,
        ElidedPolicy::FailClosed,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    assert_eq!(ord.nf.stats.dropped_elided, 0);
    assert_eq!(fc.nf.stats.dropped_elided, 1);
    // ordinary on the raw-substituted tables = ordinary on the case exports (the raw domains equal the exports)
    assert_eq!((ord.n_u(), ord.n_f()), (321, 284));
    let keys = t178_box();
    let co = ord.census(&keys).unwrap();
    let cf = fc.census(&keys).unwrap();
    assert_eq!(co.uncovered.len(), 409);
    let so: BTreeSet<Key> = co.uncovered.iter().map(|x| x.0.clone()).collect();
    let sf: BTreeSet<Key> = cf.uncovered.iter().map(|x| x.0.clone()).collect();
    assert!(so.is_subset(&sf));
    let extra: Vec<&Key> = sf.difference(&so).collect();
    println!(
        "FailClosed: U {} F {} faces {} ({} generic + {} locus-only) loci {}; box {} uncovered = {} generic + {} locus; extra keys:",
        fc.n_u(),
        fc.n_f(),
        fc.faces.len(),
        face_set(&fc, Some(FaceKind::Generic)).len(),
        face_set(&fc, Some(FaceKind::LocusOnly)).len(),
        fc.resolution.as_ref().map_or(0, |r| r.loci.len()),
        cf.uncovered.len(),
        cf.n_generic(),
        cf.n_locus()
    );
    for k in &extra {
        println!("  extra {}", rustred_campaign_core::key::key_text(k));
    }
    let new_faces: Vec<_> = face_set(&fc, None)
        .difference(&face_set(&ord, None))
        .cloned()
        .collect();
    println!(
        "FailClosed faces not in Ordinary: {}",
        new_faces.iter().map(face_str).collect::<Vec<_>>().join(" ")
    );
    for f in &fc.faces {
        println!(
            "FC face {} {:?} thresholds {:?}",
            face_str(&f.key()),
            f.kind,
            f.thresholds
        );
    }
    // fail closed = the core dispatcher's first match on the raw-substituted tables (elided record BLOCKS)
    let (disp, _) = t178_dispatcher(&fx, true);
    let rep = disp.batch(&keys, 4).unwrap();
    let open: BTreeSet<Key> = keys
        .iter()
        .zip(&rep.results)
        .filter(|(_, r)| r.is_open())
        .map(|(k, _)| k.clone())
        .collect();
    assert_eq!(
        open, sf,
        "fail-closed census = first-match open keys (409 uncovered + 21 blocked)"
    );
    assert!(cf.uncovered_in_c.is_empty());
    // Absent (skipped as if absent): the elided domain also occurs in another piece, so nothing changes
    let Some(ab) = run(
        true,
        ElidedPolicy::Absent,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    let ca = ab.census(&keys).unwrap();
    assert_eq!(ca.uncovered.len(), 409);
    assert_eq!(face_set(&ab, None), face_set(&ord, None));
    assert_eq!(extra.len(), 21, "the plan's 409 + 21");
}

/// R33: the 5 empty-domain T178B order-B case records of the frozen tree are PROVED empty.
#[test]
fn t178b_five_empty_domains_proved_empty() {
    use rustred_campaign_core::domain::{CaseDomain, DomainOptions};
    use rustred_campaign_core::partition::{Emptiness, is_empty};
    use rustred_campaign_core::record::RuleRecord;
    let Some(k3) = k3() else {
        println!("SKIP: no k3 tree");
        return;
    };
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(k3.join("T178B/F5_empty_domain_rules.json")).unwrap(),
    )
    .unwrap();
    let frame = t178_frame();
    let mut n = 0;
    for r in v.as_array().unwrap() {
        let c = &r["case_record"];
        let rec = RuleRecord {
            chart: c["chart"].as_str().map(str::to_string),
            fixed: c["fixed"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64())
                .collect(),
            equations: serde_json::from_value(c["equations"].clone()).unwrap(),
            exceptions: serde_json::from_value(c["exceptions"].clone()).unwrap(),
            dispatch: c["dispatch"].as_str().map(str::to_string),
            ..Default::default()
        };
        let d = CaseDomain::from_rule_record(
            &rec,
            &frame,
            DomainOptions {
                targets: EXPORT_TARGETS,
                ..Default::default()
            },
        )
        .unwrap();
        let e = is_empty(&d, &frame);
        println!("{} #{}: {e:?}", r["piece"], r["case_i"]);
        assert!(matches!(e, Emptiness::ProvedEmpty { .. }));
        // with the exceptions removed the domain is not empty (a verified witness)
        let mut d2 = d.clone();
        d2.exceptions.clear();
        assert!(matches!(is_empty(&d2, &frame), Emptiness::NonEmpty { .. }));
        n += 1;
    }
    assert_eq!(n, 5);
}

/// Memoised == slow reference mode (no memo), capped at 5 off-floor slots, class by class (h11b B2 (a): 589 =
/// 321 U + 268 F at cap 5).
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_memo_equals_reference_mode_cap5_cap6() {
    let Some(fx) = fx() else { return };
    let (disp, _) = t178_dispatcher(&fx, false);
    let src = DispatcherSource::new(&disp);
    let base = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        literal_tests: LiteralTests::ReferenceCompatible,
        resolve_loci: false,
        max_off_floor: Some(6),
        ..Default::default()
    };
    let a6 = gap_partition(&src, &t178_frame(), &base, None).unwrap();
    let b6 = gap_partition(
        &src,
        &t178_frame(),
        &PartitionOptions {
            memo: false,
            ..base.clone()
        },
        None,
    )
    .unwrap();
    println!(
        "cap 6: memo {} nodes, reference {} nodes {} ms; U {} F {}",
        a6.stats.nodes,
        b6.stats.nodes,
        b6.millis,
        a6.n_u(),
        a6.n_f()
    );
    let c6a: BTreeSet<_> = a6.classes.iter().cloned().collect();
    let c6b: BTreeSet<_> = b6.classes.iter().cloned().collect();
    assert_eq!(c6a, c6b);
    let base = PartitionOptions {
        max_off_floor: Some(5),
        ..base
    };
    let a = gap_partition(&src, &t178_frame(), &base, None).unwrap();
    let b = gap_partition(
        &src,
        &t178_frame(),
        &PartitionOptions {
            memo: false,
            ..base.clone()
        },
        None,
    )
    .unwrap();
    println!(
        "cap 5: memo {} nodes {} ms; reference {} nodes {} ms; U {} F {}",
        a.stats.nodes,
        a.millis,
        b.stats.nodes,
        b.millis,
        a.n_u(),
        a.n_f()
    );
    let ca: BTreeSet<_> = a.classes.iter().cloned().collect();
    let cb: BTreeSet<_> = b.classes.iter().cloned().collect();
    assert_eq!(ca, cb);
    assert_eq!((a.n_u(), a.n_f()), (321, 268));
}

/// R30 fallback: the faces of the 753 uncovered closure keys (k3 F5) are a lower bound and lie on gap faces of
/// the exact partition.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t178b_fallback_faces_from_closure_keys() {
    use rustred_campaign_core::partition::{KeyRange, faces_from_uncovered_keys};
    let Some(k3) = k3() else { return };
    let Some(rep) = run(
        false,
        ElidedPolicy::Ordinary,
        LiteralTests::ReferenceCompatible,
    ) else {
        return;
    };
    let keys: Vec<Key> = lines(k3.join("T178B/F5_closure_uncovered_753.jsonl"))
        .iter()
        .map(|v| as_key(if v.is_array() { v } else { &v["key"] }))
        .collect();
    assert_eq!(keys.len(), 753);
    let fb = faces_from_uncovered_keys(&t178_frame(), &keys, KeyRange::Measured);
    let exact = face_set(&rep, None);
    let lower: BTreeSet<_> = fb.iter().map(|f| f.key()).collect();
    println!(
        "fallback: {} faces from 753 closure gaps; exact partition {} faces; lower bound subset: {}",
        lower.len(),
        exact.len(),
        lower.is_subset(&exact)
    );
    assert!(lower.is_subset(&exact));
    for k in &keys {
        assert!(rep.uncovered(k).unwrap());
    }
}
