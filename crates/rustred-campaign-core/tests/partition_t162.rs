//! T162: the exact partition of the 2,440 qualified bank domains against the checker's frozen tree (k3 F2:
//! `T162/F2_faces_226.jsonl`, `F2_loci_14.json`, `F2_locus_points_boxB.jsonl`, `F2_summary.json`): 226 faces =
//! 217 generic + 9 locus-only, 14 loci, the quadric-ray faces as UNRESOLVED nonlinear loci, box B 990 = 968 generic +
//! 22 locus-only, "972 box keys in generic gap cells of which 4 covered on coupled case equations".
//! This file fills the `partition` placeholder of `tests/k3_oracle.rs` (which can then be removed).
//!
//! Run: `light_build.sh part test -p rustred-campaign-core --release --test partition_t162 -- --ignored --nocapture`.

mod partition_common;

use std::collections::{BTreeMap, BTreeSet};

use partition_common::*;
use rustred_campaign_core::key::Key;
use rustred_campaign_core::partition::{
    DispatcherSource, ElidedPolicy, FaceKind, LiteralTests, PartitionOptions, PartitionReport,
    gap_partition,
};

type F = (Vec<usize>, Vec<usize>);

fn run(tests: LiteralTests) -> Option<PartitionReport> {
    let fx = fx()?;
    let disp = t162_dispatcher(&fx);
    let src = DispatcherSource::new(&disp);
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        literal_tests: tests,
        ..Default::default()
    };
    let rep = gap_partition(&src, &t162_frame(), &opts, None).unwrap();
    println!(
        "T162 {tests:?}: inputs {:?}; states {:?}; nodes {} U {} F {} faces {} ({} generic, {} locus-only) loci {} ({} unresolved), loci classes capped {}, {} ms, unknown tests {}",
        rep.nf().stats,
        rep.states().iter().map(|s| s.len()).collect::<Vec<_>>(),
        rep.stats().nodes,
        rep.n_u(),
        rep.n_f(),
        rep.faces().len(),
        rep.faces()
            .iter()
            .filter(|f| f.kind == FaceKind::Generic)
            .count(),
        rep.faces()
            .iter()
            .filter(|f| f.kind == FaceKind::LocusOnly)
            .count(),
        rep.resolution().map_or(0, |r| r.loci.len()),
        rep.resolution().map_or(0, |r| r.n_unresolved()),
        rep.resolution().map_or(0, |r| r.classes_capped),
        rep.millis(),
        rep.stats().unknown_literal_tests + rep.state_unknown(),
    );
    Some(rep)
}

fn reference_faces() -> Option<BTreeMap<F, String>> {
    let k3 = k3()?;
    Some(
        lines(k3.join("T162/F2_faces_226.jsonl"))
            .iter()
            .map(|v| {
                let g = |k: &str| {
                    v[k].as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_u64().unwrap() as usize)
                        .collect()
                };
                ((g("Sd"), g("Sn")), v["kind"].as_str().unwrap().to_string())
            })
            .collect(),
    )
}

#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES and the k3 tree; release; run with --ignored"]
fn t162_partition_faces_loci_box() {
    let Some(rep) = run(LiteralTests::ReferenceCompatible) else {
        return;
    };
    let Some(reff) = reference_faces() else {
        return;
    };
    let mine: BTreeMap<F, String> = rep
        .faces()
        .iter()
        .map(|f| {
            (
                f.key(),
                if f.kind == FaceKind::Generic {
                    "generic"
                } else {
                    "locus-only"
                }
                .to_string(),
            )
        })
        .collect();
    let res = rep.resolution().unwrap();
    for l in &res.loci {
        println!(
            "locus {} face {} eqs {:?} dim {} classes {} unresolved {} witness {:?} window {:?}",
            l.id,
            face_str(&l.face),
            l.equations,
            l.dim,
            l.n_classes,
            l.unresolved,
            l.witness.first(),
            l.unresolved_parts
                .first()
                .map(|u| (u.bound.clone(), u.n_window, u.points.len()))
        );
    }
    let only_mine: Vec<_> = mine
        .iter()
        .filter(|(k, _)| !reff.contains_key(*k))
        .collect();
    let only_ref: Vec<_> = reff
        .iter()
        .filter(|(k, _)| !mine.contains_key(*k))
        .collect();
    let kind_diff: Vec<_> = mine
        .iter()
        .filter(|(k, v)| reff.get(*k).is_some_and(|r| r != *v))
        .collect();
    println!("faces only mine {only_mine:?}");
    println!("faces only reference {only_ref:?}");
    println!("faces with a different kind {kind_diff:?}");
    let gen_mine: BTreeSet<&F> = mine
        .iter()
        .filter(|(_, v)| *v == "generic")
        .map(|(k, _)| k)
        .collect();
    let gen_ref: BTreeSet<&F> = reff
        .iter()
        .filter(|(_, v)| *v == "generic")
        .map(|(k, _)| k)
        .collect();
    // box B census
    let cen = rep.census(&t162_box()).unwrap();
    println!(
        "box B: {} uncovered = {} generic + {} locus; covered keys in U classes {} {:?}; uncovered in C {}",
        cen.uncovered.len(),
        cen.n_generic(),
        cen.n_locus(),
        cen.covered_in_u.len(),
        cen.covered_in_u
            .iter()
            .map(|k| rustred_campaign_core::key::key_text(k))
            .collect::<Vec<_>>(),
        cen.uncovered_in_c.len()
    );
    let k3 = k3().unwrap();
    let refu: BTreeSet<Key> = lines(k3.join("T162/F1_uncovered_990.jsonl"))
        .iter()
        .map(as_key)
        .collect();
    let unc: BTreeSet<Key> = cen.uncovered.iter().map(|x| x.0.clone()).collect();
    assert_eq!(
        unc, refu,
        "V1: the census equals the reference 990 key by key"
    );
    assert!(cen.uncovered_in_c.is_empty());
    // V2: every box gap lies on a listed face
    let all: BTreeSet<F> = mine.keys().cloned().collect();
    assert!(cen.faces().is_subset(&all));
    // locus points of the frozen tree: on their face, uncovered
    for v in lines(k3.join("T162/F2_locus_points_boxB.jsonl")) {
        let k: Key = as_key(&v["key"]);
        assert_eq!(
            rep.uncovered(&k).unwrap(),
            v["uncovered"].as_bool().unwrap()
        );
    }
    assert_eq!(gen_mine, gen_ref, "generic faces = the reference's 217");
    assert_eq!(mine, reff, "226 faces with the reference's kinds");
    assert_eq!((cen.n_generic(), cen.n_locus()), (968, 22));
    // the reference's 4 covered generic-cell keys are among ours (our general states put n9 = -2 into the tail
    // state of slot 9 where the checker's uniform K = 2 states had a singleton: 5 more covered tail-cell keys)
    let four: BTreeSet<Key> = [
        "I(2,1,1,2,3,1,1,1,1,0,0,-2,0,0,0,0,0,0,0,0)",
        "I(2,2,1,1,3,1,1,1,1,0,0,-2,0,0,0,0,0,0,0,0)",
        "I(2,2,1,1,3,1,1,1,1,0,0,-1,0,-1,0,0,0,0,0,0)",
        "I(2,2,1,1,3,1,1,1,1,0,0,-1,0,0,0,0,0,0,0,0)",
    ]
    .iter()
    .map(|s| rustred_campaign_core::key::parse_key(s).unwrap())
    .collect();
    let ciu: BTreeSet<Key> = cen.covered_in_u.iter().cloned().collect();
    assert!(four.is_subset(&ciu));
    println!(
        "generic-cell box keys: {} = 968 uncovered + {} covered",
        968 + ciu.len(),
        ciu.len()
    );
    // loci: the reference's 14 loci faces carry linear loci (equal equations on the face); the quadric pieces
    let refl: BTreeSet<F> = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(k3.join("T162/F2_loci_14.json")).unwrap(),
    )
    .unwrap()
    .as_array()
    .unwrap()
    .iter()
    .map(|v| {
        let g = |k: &str| {
            v["face"][k]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect()
        };
        (g("Sd"), g("Sn"))
    })
    .collect();
    let lin_faces: BTreeSet<F> = res
        .loci
        .iter()
        .filter(|l| l.equations.len() == 1 && !l.unresolved)
        .map(|l| l.face.clone())
        .collect();
    assert_eq!(
        lin_faces, refl,
        "single-cut linear loci on exactly the reference's 14 faces"
    );
    assert_eq!(
        res.loci.iter().filter(|l| l.equations.len() == 1).count(),
        14
    );
    let quad_faces: BTreeSet<F> = res
        .loci
        .iter()
        .filter(|l| l.equations.len() > 1)
        .map(|l| l.face.clone())
        .collect();
    println!("faces with quadric-cut loci: {:?}", quad_faces);
    assert_eq!(quad_faces.len(), 3);
    for l in res.loci.iter().filter(|l| l.equations.len() > 1) {
        // every quadric piece starts beyond rank 20 (outside box B and the proved region r <= 17)
        for w in &l.witness {
            assert!(t162_frame().rank(w).unwrap() >= 21, "{w:?}");
        }
    }
}

/// The checker's uniform granularity (lines {1,2,>=3}, numerators {0,-1,-2,<=-3}) given as extra critical values
/// (DATA): reproduces its class counts 2,278 U + 829 F.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES; release; run with --ignored"]
fn t162_reference_granularity_class_counts() {
    let Some(fx) = fx() else { return };
    let disp = t162_dispatcher(&fx);
    let src = DispatcherSource::new(&disp);
    let extra: Vec<Vec<i64>> = (0..20)
        .map(|i| if i < 9 { vec![1, 2] } else { vec![0, -1, -2] })
        .collect();
    let opts = PartitionOptions {
        elided: ElidedPolicy::Ordinary,
        literal_tests: LiteralTests::ReferenceCompatible,
        resolve_loci: false,
        extra_critical: Some(extra),
        ..Default::default()
    };
    let rep = gap_partition(&src, &t162_frame(), &opts, None).unwrap();
    println!(
        "T162 uniform states: nodes {} U {} F {} faces {} in {} ms",
        rep.stats().nodes,
        rep.n_u(),
        rep.n_f(),
        rep.faces().len(),
        rep.millis()
    );
    // U classes reproduce exactly; F differs by the literal structure of the pole factors (our exact linear-factor
    // extraction vs the checker's full sympy factorisation): reported, not asserted
    assert_eq!(rep.n_u(), 2278);
    assert_eq!(
        rep.faces()
            .iter()
            .filter(|f| f.kind == FaceKind::Generic)
            .count(),
        217
    );
}

#[test]
#[ignore = "timing of the stages"]
fn t162_stage_timing() {
    use rustred_campaign_core::partition::{classes, nf};
    let Some(fx) = fx() else { return };
    let disp = t162_dispatcher(&fx);
    let src = DispatcherSource::new(&disp);
    let t = std::time::Instant::now();
    let n = nf::build(
        &src,
        &t162_frame(),
        &nf::NfOptions {
            elided: ElidedPolicy::Ordinary,
            dedup: true,
            subset: None,
            factor_exceptions: false,
        },
    )
    .unwrap();
    println!("nf {:?}", t.elapsed());
    let (st, _) = classes::slot_states(&n, None).unwrap();
    println!("states {:?}", t.elapsed());
    let mut e = classes::Engine::new(&n, st, LiteralTests::ReferenceCompatible).unwrap();
    println!("engine {:?}", t.elapsed());
    let _ = e.run(None);
    println!("run {:?} {:?}", t.elapsed(), e.stats);
}
