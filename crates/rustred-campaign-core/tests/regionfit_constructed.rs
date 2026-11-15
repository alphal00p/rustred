//! regionfit on constructed faces (interfaces/regionfit.md): two choices split by n0 <= n2, three regions, no region
//! form (per key), an adversarial tempting condition, determinism under permutation, ranking, extends, the IBP use
//! (split recipe by locus), exact vs greedy, and a randomised soundness / partition property.

mod regionfit_common;

use proptest::prelude::*;
use regionfit_common::{check_record, face, fk, summary};
use rustred_campaign_core::regionfit::{FaceKey, FitMethod, RegionPolicy, fit_regions};

fn grid3(lo: i32, hi: i32, f: &dyn Fn(i32, i32, i32) -> Vec<&'static str>) -> Vec<FaceKey> {
    let mut v = vec![];
    for a in lo..=hi {
        for b in 1..=2 {
            for c in lo..=hi {
                v.push(fk(&[a, b, c, 0], &f(a, b, c)));
            }
        }
    }
    v
}

#[test]
fn two_elements_split_by_n0_le_n2() {
    let keys = grid3(1, 4, &|a, _, c| {
        if a <= c { vec!["g1"] } else { vec!["g2"] }
    });
    let f = face("split", keys);
    let fit = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    check_record(&f, &rec);
    assert_eq!(fit.method, FitMethod::Exact);
    assert_eq!(
        summary(&rec),
        vec![
            ("n0<=n2".to_string(), "g1".to_string()),
            ("n2<n0".to_string(), "g2".to_string())
        ]
    );
    assert!(fit.residual.is_empty());
    assert_eq!(rec["fit"]["pieces"], 2);
    assert_eq!(
        rec["regions"][0]["verified_on"],
        "verified on 32 keys of the box"
    );
}

#[test]
fn three_regions() {
    // n1 = 1 -> only c; else n0 <= n2 -> a; else b
    let keys = grid3(1, 4, &|a, b, c| {
        if b == 1 {
            vec!["c"]
        } else if a <= c {
            vec!["a"]
        } else {
            vec!["b"]
        }
    });
    let f = face("three", keys);
    let fit = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    check_record(&f, &rec);
    assert_eq!(fit.regions.len(), 3, "{:?}", summary(&rec));
    assert!(fit.residual.is_empty());
    let mut by: Vec<String> = fit.regions.iter().map(|r| r.choice.clone()).collect();
    by.sort();
    assert_eq!(by, vec!["a", "b", "c"]);
    // c is the single-atom region n1 = 1 (or its equivalent n1 <= 1)
    let c = fit.regions.iter().find(|r| r.choice == "c").unwrap();
    assert_eq!(c.atoms.len(), 1);
    assert_eq!(c.verified, 16);
}

#[test]
fn no_region_form_gives_per_key() {
    // one varying slot, choices alternate: with parity OFF no condition of the family isolates >= 2 keys of one choice
    let keys: Vec<FaceKey> = (1..=8)
        .map(|x| fk(&[x, 1], &[if x % 2 == 0 { "a" } else { "b" }]))
        .collect();
    let f = face("alt", keys);
    let mut p = RegionPolicy::default();
    p.atoms.parity = false;
    let fit = fit_regions(&f, None, &p).unwrap();
    check_record(&f, &fit.record());
    assert!(fit.regions.is_empty());
    assert_eq!(fit.residual.len(), 8);
    assert_eq!(fit.cost.pieces, 8);
    // the family is spec data: with parity ON the same face has two regions
    let fit2 = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    check_record(&f, &fit2.record());
    assert_eq!(
        summary(&fit2.record()),
        vec![
            ("n0 even".to_string(), "a".to_string()),
            ("n0 odd".to_string(), "b".to_string())
        ]
    );
}

#[test]
fn adversarial_tempting_condition_not_emitted() {
    // a holds on n0 <= n1 EXCEPT at (1,5); b holds on n0 > n1 and at (1,5)
    let mut keys = vec![];
    for x in 1..=5 {
        for y in 1..=5 {
            let acc: Vec<&str> = if (x, y) == (1, 5) {
                vec!["b"]
            } else if x <= y {
                vec!["a"]
            } else {
                vec!["b"]
            };
            keys.push(fk(&[x, y], &acc));
        }
    }
    let f = face("adv", keys);
    for max_atoms in 1..=3 {
        let p = RegionPolicy {
            max_atoms,
            ..RegionPolicy::default()
        };
        let fit = fit_regions(&f, None, &p).unwrap();
        let rec = fit.record();
        check_record(&f, &rec);
        assert!(
            !summary(&rec).contains(&("n0<=n1".to_string(), "a".to_string())),
            "tempting condition emitted"
        );
        // the exception key is served by b (a region or per key), never by a
        let k15 = vec![1, 5];
        let served_by_a = fit
            .regions
            .iter()
            .any(|r| r.choice == "a" && r.atoms.iter().all(|a| a.eval(&k15)));
        assert!(!served_by_a);
    }
}

#[test]
fn deterministic_under_permutation() {
    let keys = grid3(1, 4, &|a, b, c| {
        if b == 1 && a == c {
            vec!["x", "a"]
        } else if a <= c {
            vec!["a", "b"]
        } else {
            vec!["b"]
        }
    });
    let f1 = face("perm", keys.clone());
    let mut k2 = keys.clone();
    k2.reverse();
    k2.rotate_left(7);
    let f2 = face("perm", k2);
    let p = RegionPolicy::default();
    let r1 = fit_regions(&f1, None, &p).unwrap().record_text();
    let r2 = fit_regions(&f2, None, &p).unwrap().record_text();
    assert_eq!(r1, r2);
    let r3 = fit_regions(&f1, None, &p).unwrap().record_text();
    assert_eq!(r1, r3);
    // a duplicate key is refused
    let mut k3 = keys.clone();
    k3.push(keys[0].clone());
    assert!(fit_regions(&face("dup", k3), None, &p).is_err());
}

#[test]
fn ranking_breaks_ties() {
    let keys = grid3(1, 3, &|_, _, _| vec!["regular", "singular"]);
    let f = face("rank", keys);
    let a = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    assert_eq!(
        summary(&a.record()),
        vec![("true".into(), "regular".into())]
    );
    let p = RegionPolicy {
        ranking: vec!["singular".into()],
        ..RegionPolicy::default()
    };
    let b = fit_regions(&f, None, &p).unwrap();
    assert_eq!(
        summary(&b.record()),
        vec![("true".into(), "singular".into())]
    );
    assert_ne!(a.policy_digest, b.policy_digest);
}

#[test]
fn split_recipe_by_locus_ibp_use() {
    // TRICKS 12-13 shape: a generic recipe valid on n3 >= 3, an up-centre recipe valid on n3 = 2 (and on some n3 >= 3
    // keys too); choices are opaque recipe labels
    let mut keys = vec![];
    for n0 in 1..=3 {
        for n3 in 2..=5 {
            for n9 in -2..=0 {
                let acc: Vec<&str> = if n3 == 2 {
                    vec!["centre:key+e3;S0+S1"]
                } else if n0 == 1 {
                    vec!["centre:key;S0+S1", "centre:key+e3;S0+S1"]
                } else {
                    vec!["centre:key;S0+S1"]
                };
                keys.push(fk(&[n0, 1, 1, n3, n9], &acc));
            }
        }
    }
    let f = face("core", keys);
    let fit = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    check_record(&f, &rec);
    let s = summary(&rec);
    assert_eq!(s.len(), 2, "{s:?}");
    assert!(
        s.contains(&("n3>=3".to_string(), "centre:key;S0+S1".to_string())),
        "{s:?}"
    );
    assert!(
        s.iter()
            .any(|(t, c)| (t == "n3=2" || t == "n3<=2") && c == "centre:key+e3;S0+S1"),
        "{s:?}"
    );
}

#[test]
fn extends_reported_on_a_deeper_list() {
    let keys = grid3(1, 3, &|a, _, c| {
        if a <= c { vec!["g1"] } else { vec!["g2"] }
    });
    let f = face("ext", keys);
    // deeper layer: the form holds except at one key
    let mut deeper = vec![];
    for a in 1..=5 {
        for c in 4..=5 {
            let acc = if (a, c) == (5, 5) {
                vec!["g2"]
            } else if a <= c {
                vec!["g1"]
            } else {
                vec!["g2"]
            };
            deeper.push(fk(&[a, 1, c, 0], &acc));
        }
    }
    let fit = fit_regions(&f, Some(&deeper), &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    check_record(&f, &rec);
    let r1 = fit.regions.iter().find(|r| r.choice == "g1").unwrap();
    assert_eq!(r1.text, "n0<=n2");
    let e = r1.extends.unwrap();
    // deeper keys with n0 <= n2: (1..4, 4) and (1..5, 5) = 9; (5,5) fails
    assert_eq!((e.n, e.m), (8, 9));
    assert_eq!(rec["deeper"]["keys"], 10);
}

#[test]
fn deeper_list_picks_among_face_equivalent_conditions() {
    // on this face (n0 in {1,2,4}, n1 in {2,3}) the conditions n0<=n1, n0<=2 and n0+n1<=5 have the same truth set
    let mut keys = vec![];
    for a in [1, 2, 4] {
        for c in [2, 3] {
            keys.push(fk(&[a, c], &[if a <= 2 { "g1" } else { "g2" }]));
        }
    }
    let f = face("eqv", keys);
    let plain = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    check_record(&f, &plain.record());
    let r0 = plain.regions.iter().find(|r| r.choice == "g1").unwrap();
    // without a deeper list: the scale-free relation first
    assert_eq!(r0.text, "n0<=n1");
    assert!(r0.equivalent_conditions >= 3, "{r0:?}");
    assert!(r0.alternatives.contains(&"n0<=2".to_string()), "{r0:?}");
    // the deeper layer accepts g1 exactly on n0 <= 2: that form is shown
    let mut deeper = vec![];
    for a in 1..=6 {
        for c in 4..=5 {
            deeper.push(fk(&[a, c], &[if a <= 2 { "g1" } else { "g2" }]));
        }
    }
    let fit = fit_regions(&f, Some(&deeper), &RegionPolicy::default()).unwrap();
    check_record(&f, &fit.record());
    let r1 = fit.regions.iter().find(|r| r.choice == "g1").unwrap();
    assert_eq!(r1.text, "n0<=2");
    let e = r1.extends.unwrap();
    assert_eq!((e.n, e.m), (4, 4));
    // switched off: the deeper list is reported only
    let p = RegionPolicy {
        prefer_extending: false,
        ..RegionPolicy::default()
    };
    let off = fit_regions(&f, Some(&deeper), &p).unwrap();
    let r2 = off.regions.iter().find(|r| r.choice == "g1").unwrap();
    assert_eq!(r2.text, "n0<=n1");
    assert_eq!(r2.extends.map(|e| (e.n, e.m)), Some((4, 9)));
}

#[test]
fn exact_never_worse_than_greedy_and_method_reported() {
    let keys = grid3(1, 4, &|a, b, c| {
        let mut v = vec![];
        if a <= c {
            v.push("a");
        }
        if b == 2 || a == 4 {
            v.push("b");
        }
        if a + c <= 4 {
            v.push("c");
        }
        if v.is_empty() {
            v.push("d");
        }
        v
    });
    let f = face("mix", keys);
    let ex = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    let gr = fit_regions(
        &f,
        None,
        &RegionPolicy {
            exact_max_candidates: 0,
            ..RegionPolicy::default()
        },
    )
    .unwrap();
    check_record(&f, &ex.record());
    check_record(&f, &gr.record());
    assert_eq!(ex.method, FitMethod::Exact);
    assert_eq!(gr.method, FitMethod::Greedy);
    assert!(ex.cost <= gr.cost, "{:?} vs {:?}", ex.cost, gr.cost);
    assert_eq!(gr.record()["fit"]["method"], "greedy");
    let tiny = fit_regions(
        &f,
        None,
        &RegionPolicy {
            exact_node_budget: 0,
            ..RegionPolicy::default()
        },
    )
    .unwrap();
    check_record(&f, &tiny.record());
    assert_eq!(tiny.method, FitMethod::ExactBudgetExhausted);
    assert!(tiny.cost <= gr.cost);
}

#[test]
fn unservable_keys_block_regions_and_are_listed() {
    let keys = grid3(1, 3, &|a, b, c| {
        if (a, b, c) == (2, 1, 2) {
            vec![]
        } else {
            vec!["g1"]
        }
    });
    let f = face("hole", keys);
    let fit = fit_regions(&f, None, &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    check_record(&f, &rec);
    assert_eq!(fit.unservable, vec![vec![2, 1, 2, 0]]);
    assert!(fit.regions.iter().all(|r| r.text != "true"));
}

#[test]
fn empty_face_and_bad_input() {
    let fit = fit_regions(&face("empty", vec![]), None, &RegionPolicy::default()).unwrap();
    assert!(fit.regions.is_empty() && fit.residual.is_empty());
    assert!(
        fit.record()["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let bad = face("arity", vec![fk(&[1, 2], &["a"]), fk(&[1], &["a"])]);
    assert!(fit_regions(&bad, None, &RegionPolicy::default()).is_err());
    let p: RegionPolicy =
        serde_json::from_str(r#"{"max_atoms":1,"atoms":{"sum_le":false}}"#).unwrap();
    assert_eq!(p.max_atoms, 1);
    assert!(p.atoms.le_rel && !p.atoms.sum_le);
    assert!(serde_json::from_str::<RegionPolicy>(r#"{"bogus":1}"#).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]
    #[test]
    fn random_faces_sound_and_partitioned(
        cells in proptest::collection::vec(0u8..16, 27),
        max_atoms in 0usize..=3,
        exact in any::<bool>(),
    ) {
        // 27 keys on a 3x3x3 grid, each with a random subset of {a,b,c,d}
        let mut keys = vec![];
        let mut i = 0;
        for x in 0..3 {
            for y in 0..3 {
                for z in -1..2 {
                    let m = cells[i];
                    i += 1;
                    let acc: Vec<&str> = ["a", "b", "c", "d"]
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| m >> j & 1 == 1)
                        .map(|(_, s)| *s)
                        .collect();
                    keys.push(fk(&[x, y, z], &acc));
                }
            }
        }
        let f = face("rand", keys);
        let p = RegionPolicy {
            max_atoms,
            exact_max_candidates: if exact { 400 } else { 0 },
            ..RegionPolicy::default()
        };
        let fit = fit_regions(&f, None, &p).unwrap();
        check_record(&f, &fit.record());
        prop_assert_eq!(
            fit.cost.pieces,
            fit.regions.len() as u64 + fit.residual.len() as u64
        );
    }
}
