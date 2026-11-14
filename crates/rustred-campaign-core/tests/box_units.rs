//! Box data, membership, enumeration, counts, saturation, faces, digest (interface campaign-core-domain.md s.7).
//! No external data: runs by default.

mod box_common;

use std::collections::BTreeSet;

use box_common::*;
use proptest::prelude::*;
use rustred_campaign_core::boxdef::{
    BoxDef, BoxError, Face, SectorLayers, blake3_hex, capped_compositions, compositions_counts,
    group_by_face,
};
use rustred_campaign_core::key::{Key, NodeFrame};

/// `b3sum` 1.8.6 on the official test input (byte i = i mod 251).
const B3: &[(usize, &str)] = &[
    (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
    (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"),
    (63, "e9bc37a594daad83be9470df7f7b3798297c3d834ce80ba85d6e207627b7db7b"),
    (64, "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98"),
    (65, "de1e5fa0be70df6d2be8fffd0e99ceaa8eb6e8c93a63f2d8d1c30ecb6b263dee"),
    (1023, "10108970eeda3eb932baac1428c7a2163b0e924c9a9e25b35bba72b28f70bd11"),
    (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"),
    (1025, "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"),
    (2048, "e776b6028c7cd22a4d0ba182a8bf62205d2ef576467e838ed6f2529b85fba24a"),
    (2049, "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030"),
    (3072, "b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2"),
    (3073, "7124b49501012f81cc7f11ca069ec9226cecb8a2c850cfe644e327d22d3e1cd3"),
    (4096, "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969"),
    (4097, "9b4052b38f1c5fc8b1f9ff7ac7b27cd242487b3d890d15c96a1c25b8aa0fb995"),
    (5121, "628bd2cb2004694adaab7bbd778a25df25c47b9d4155a55f8fbd79f2fe154cff"),
    (7169, "a003fc7a51754a9b3c7fae0367ab3d782dccf28855a03d435f8cfe74605e7817"),
    (8192, "aae792484c8efe4f19e2ca7d371d8c467ffb10748d8a5a1ae579948f718a2a63"),
    (8193, "bab6c09cb8ce8cf459261398d2e7aef35700bf488116ceb94a36d0f5f1b7bc3b"),
    (31744, "62b6960e1a44bcc1eb1a611a8d6235b6b4b78f32e7abc4fb4c6cdcce94895c47"),
    (102400, "bc3e3d41a1146b069abffad3c0d44860cf664390afce4d9661f7902e7943e085"),
];

#[test]
fn blake3_matches_b3sum_vectors() {
    for &(n, h) in B3 {
        let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
        assert_eq!(blake3_hex(&data), h, "length {n}");
    }
    assert_eq!(
        blake3_hex(b"abc"),
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    );
}

#[test]
fn composition_counts_agree() {
    for parts in 0..7u64 {
        for total in 0..12u64 {
            for cap in [None, Some(0), Some(1), Some(2), Some(4)] {
                let caps = vec![cap.map(|c| c as u32); parts as usize];
                assert_eq!(
                    compositions_counts(total, &caps)[total as usize],
                    capped_compositions(total, parts, cap),
                    "total {total} parts {parts} cap {cap:?}"
                );
            }
        }
    }
}

/// Counts of the generated R* boxes (example generator + generic counter); the five-loop numbers are test data.
#[test]
fn band_box_counts_and_layers() {
    let t162 = rstar_union(&lines_first(20, 9));
    assert_eq!(t162.layers, vec![[0, 1], [1, 2], [2, 2], [3, 2], [4, 2]]);
    assert_eq!(t162.cap, Some(5));
    assert_eq!(t162.count(), 55_704);
    // per (D, r) as the frozen tree's README
    let want = [
        (0, 0, 1u128),
        (0, 1, 11),
        (1, 0, 9),
        (1, 1, 99),
        (1, 2, 594),
        (2, 0, 45),
        (2, 1, 495),
        (2, 2, 2970),
        (3, 0, 165),
        (3, 1, 1815),
        (3, 2, 10890),
        (4, 0, 495),
        (4, 1, 5445),
        (4, 2, 32670),
    ];
    let got: Vec<(u32, u32, u128)> = t162
        .layer_counts()
        .iter()
        .map(|c| (c.dots, c.rank, c.count))
        .collect();
    assert_eq!(got, want);
    assert_eq!(rstar_union(&lines_first(20, 10)).count(), 286_066);
    let t266 = rstar_union(&lines_first(20, 14));
    assert_eq!(t266.layers, vec![[0, 6], [1, 7], [2, 7], [3, 7], [4, 7]]);
    assert_eq!(t266.count(), 5_250_168);
    // the memo's "(2,0), (3,<=1), (4,<=2)" for 9 lines is the MAIN band
    let main = rstar_main(&lines_first(20, 9));
    assert_eq!(main.layers, vec![[2, 0], [3, 1], [4, 2]]);
    println!(
        "main band E=9: {} keys; E=10: {}; E=14: {}",
        main.count(),
        rstar_main(&lines_first(20, 10)).count(),
        rstar_main(&lines_first(20, 14)).count()
    );
    // a band never yields a negative layer: E=5 drops D=0 (r <= -3) and everything else (r <= -2)
    assert!(rstar_union(&lines_first(8, 5)).layers.is_empty());
}

#[test]
fn digest_is_stable_under_json_reordering_and_normalisation() {
    let b = rstar_union(&lines_first(20, 9));
    let d = b.digest();
    let text = r#"{"origin":"band(rstar-union-v1, E=9)","layers":[[4,2],[0,1],[2,1],[2,2],[3,2],[1,2]],
        "keys":[],"cap":5,"chart":"11111111100000000000","measure":"dots-rank-v1",
        "name":"R* box of renormalisable gauge theories in Feynman gauge, union band (box B)","id":"rstar-union-v1-E9",
        "schema":"rustred.campaign.box.v1","derived_from":null,"sectors":[]}"#;
    let c = BoxDef::from_json(text).unwrap();
    assert_eq!(c.digest(), d);
    assert_eq!(c, b);
    // stated digest checked
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    v["digest"] = serde_json::json!(d);
    assert!(BoxDef::from_value(&v).is_ok());
    v["digest"] = serde_json::json!("00");
    assert!(matches!(BoxDef::from_value(&v), Err(BoxError::Digest { .. })));
    // any content change changes it
    let mut e = b.clone();
    e.cap = Some(6);
    assert_ne!(e.digest(), d);
    // unknown fields refused
    let mut w: serde_json::Value = serde_json::from_str(text).unwrap();
    w["extra"] = serde_json::json!(1);
    assert!(matches!(BoxDef::from_value(&w), Err(BoxError::Schema(_))));
}

#[test]
fn per_line_cap_and_slot_caps() {
    // 3 lines, 2 numerators, cap 2: every line power <= 2, so D = d has C(3, d) line patterns
    let text = r#"{"schema":"rustred.campaign.box.v1","id":"capped","name":"capped","measure":"dots-rank-v1",
        "chart":"11100","layers":[[0,0],[1,0],[2,0],[3,0],[4,0]],"cap":2}"#;
    let b = BoxDef::from_json(text).unwrap();
    let ks: Vec<Key> = b.iter().collect();
    assert_eq!(ks.len(), 8);
    assert_eq!(b.count(), 8);
    assert!(ks.iter().all(|k| k[..3].iter().all(|&x| (1..=2).contains(&x))));
    assert!(!b.contains(&[3, 1, 1, 0, 0]));
    // slot caps: line 0 at most power 1, numerator 4 at most |1|
    let text = r#"{"schema":"rustred.campaign.box.v1","id":"slot","name":"slot","measure":"dots-rank-v1",
        "chart":"11100","layers":[[0,3],[1,3],[2,3]],"slot_caps":[1,null,null,null,1]}"#;
    let b = BoxDef::from_json(text).unwrap();
    let ks: Vec<Key> = b.iter().collect();
    assert_eq!(ks.len() as u128, b.count());
    assert!(ks.iter().all(|k| k[0] == 1 && k[4] >= -1));
    assert!(ks.iter().all(|k| b.contains(k)));
    assert!(!b.contains(&[2, 1, 1, 0, 0]));
    assert!(!b.contains(&[1, 1, 1, 0, -2]));
    // brute force: D in 0..=2 on lines 1, 2 only; r <= 3 with n4 in {0,-1}
    let mut n = 0;
    for a in 1..=3 {
        for c in 1..=3 {
            for x in -3..=0 {
                for y in -1..=0 {
                    let (d, r) = (a + c - 2, -x - y);
                    if d <= 2 && r <= 3 {
                        n += 1;
                    }
                }
            }
        }
    }
    assert_eq!(ks.len(), n);
    // a zero cap on a line slot is refused
    let bad = text.replace("[1,null,null,null,1]", "[0,null,null,null,1]");
    assert!(BoxDef::from_json(&bad).is_err());
}

#[test]
fn explicit_key_list_box() {
    let text = r#"{"schema":"rustred.campaign.box.v1","id":"list","name":"list","measure":"dots-rank-v1",
        "chart":"1100","layers":[[0,0]],"keys":[[3,1,0,-2],[1,1,0,0],[2,2,-1,0],[3,1,0,-2]]}"#;
    let b = BoxDef::from_json(text).unwrap();
    let ks: Vec<Key> = b.iter().collect();
    // [1,1,0,0] is in the layered part; duplicates removed; extras sorted after the layered part
    assert_eq!(ks, vec![vec![1, 1, 0, 0], vec![2, 2, -1, 0], vec![3, 1, 0, -2]]);
    assert_eq!(b.count(), 3);
    assert!(b.contains(&[2, 2, -1, 0]));
    assert!(!b.contains(&[2, 2, 0, -1]));
    assert!(b.to_boxed_header_box().is_err());
}

#[test]
fn saturation_hull() {
    let b = rstar_union(&lines_first(20, 9));
    let mut far = vec![1i32; 9];
    far.extend(vec![0; 11]);
    far[0] = 6; // D = 5, line power 6
    far[9] = -4; // r = 4
    let mut pos = vec![1i32; 9];
    pos.extend(vec![0; 11]);
    pos[10] = 1; // positive numerator: not absorbable
    let mut sub = vec![1i32; 9];
    sub.extend(vec![0; 11]);
    sub[3] = 0; // unlisted support: a hand-down, not absorbed
    let inside = vec![1i32; 9].into_iter().chain(vec![0; 11]).collect::<Vec<_>>();
    let s = b.saturate([far.as_slice(), pos.as_slice(), sub.as_slice(), inside.as_slice()]);
    assert_eq!(s.absorbed, 1);
    assert_eq!(s.already_inside, 1);
    assert_eq!(s.not_absorbed, vec![pos.clone(), sub.clone()]);
    assert_eq!(s.raised_cap, Some((5, 6)));
    let bs = &s.boxdef;
    assert_eq!(bs.layers, vec![[0, 1], [1, 2], [2, 2], [3, 2], [4, 2], [5, 4]]);
    assert_eq!(bs.derived_from.as_deref(), Some(b.digest().as_str()));
    assert!(bs.id.ends_with("+sat"));
    assert!(bs.contains(&far));
    assert!(!s.is_fixed_point());
    // every key of the input box stays inside B*; nothing in a gap D (here D in 0..=5 all present)
    assert!(b.iter().all(|k| bs.contains(&k)));
    // saturating B* with the same keys is a fixed point modulo the non-absorbable ones
    let s2 = bs.saturate([far.as_slice(), inside.as_slice()]);
    assert!(s2.is_fixed_point());
    assert_eq!(s2.boxdef.digest(), bs.digest());
    // a D gap stays a gap (reading A): a key at D = 7 adds [7, r] only
    let mut d7 = inside.clone();
    d7[1] = 8;
    let s3 = b.saturate([d7.as_slice()]);
    assert_eq!(s3.boxdef.layers.last(), Some(&[7, 0]));
    assert!(!s3.boxdef.layers.iter().any(|l| l[0] == 5 || l[0] == 6));
    assert_eq!(s3.boxdef.cap, Some(8));
}

#[test]
fn boxed_header_round_trip_and_frame() {
    let chart = lines_first(20, 9);
    let b = rstar_union(&chart);
    let h = b.to_boxed_header_box().unwrap();
    assert_eq!(h["measure"], "dots-rank-v1");
    assert_eq!(h["layers"], serde_json::json!([[0, 1], [1, 2], [2, 2], [3, 2], [4, 2]]));
    let c = BoxDef::from_boxed_header_box(&h, &chart).unwrap();
    assert_eq!(c.layers, b.layers);
    assert_eq!(c.cap, b.cap);
    assert_eq!(c.count(), 55_704);
    assert!(b.check_frame(&NodeFrame::without_header_lines_first(20, 9)).is_ok());
    assert!(b.check_frame(&NodeFrame::without_header_lines_first(20, 10)).is_err());
}

#[test]
fn faces() {
    let chart = lines_first(6, 3);
    assert_eq!(
        Face::of(&chart, &[2, 1, 3, 0, -1, 0]).unwrap().to_string(),
        "{0,2}x{4}"
    );
    assert_eq!(Face::of(&chart, &[2, 2, 1, 0, 0, 0]).unwrap().to_string(), "{0,1}");
    assert_eq!(Face::of(&chart, &[1, 1, 1, 0, 0, 0]).unwrap().to_string(), "{}");
    assert!(Face::of(&chart, &[0, 1, 1, 0, 0, 0]).is_none());
    assert!(Face::of(&chart, &[1, 1, 1, 1, 0, 0]).is_none());
    let g = group_by_face(
        &chart,
        vec![vec![2, 1, 1, 0, 0, -1], vec![1, 1, 1, 1, 0, 0], vec![3, 1, 1, 0, 0, -2]],
    );
    assert_eq!(g.faces.len(), 1);
    assert_eq!(g.faces.values().next().unwrap().len(), 2);
    assert_eq!(g.other.len(), 1);
}

/// A random small box on a random chart (any arity), with optional cap, slot caps, one sector, explicit keys.
fn arb_box(arity: usize) -> impl Strategy<Value = BoxDef> {
    let chart = proptest::collection::vec(any::<bool>(), arity).prop_filter("a line", |c| {
        c.iter().filter(|&&l| l).count() >= 1
    });
    (
        chart,
        proptest::collection::vec((0u32..4, 0u32..3), 0..4),
        proptest::option::of(1u32..4),
        proptest::collection::vec(proptest::option::of(1u32..3), arity),
        any::<bool>(),
        proptest::collection::vec(proptest::collection::vec(-2i32..4, arity), 0..3),
        proptest::collection::vec((0u32..3, 0u32..3), 1..3),
    )
        .prop_map(move |(chart, layers, cap, scaps, use_sector, keys, slayers)| {
            let lines: Vec<usize> = (0..arity).filter(|&i| chart[i]).collect();
            let sectors = if use_sector && lines.len() >= 2 {
                vec![SectorLayers {
                    support: lines[1..].to_vec(),
                    layers: slayers.iter().map(|&(d, r)| [d, r]).collect(),
                }]
            } else {
                vec![]
            };
            let slot_caps = if scaps.iter().filter(|c| c.is_some()).count() % 2 == 0 {
                scaps
            } else {
                vec![]
            };
            BoxDef {
                id: "prop".into(),
                name: "prop".into(),
                chart,
                layers: layers.iter().map(|&(d, r)| [d, r]).collect(),
                sectors,
                cap,
                slot_caps,
                keys,
                origin: None,
                derived_from: None,
            }
        })
}

fn check_box(b: &BoxDef) -> Result<(), TestCaseError> {
    let b = b.normalised();
    prop_assert!(b.validate().is_ok());
    let ks: Vec<Key> = b.iter().collect();
    prop_assert_eq!(ks.len() as u128, b.count());
    let set: BTreeSet<&Key> = ks.iter().collect();
    prop_assert_eq!(set.len(), ks.len(), "duplicate key in enumeration");
    for k in &ks {
        prop_assert!(b.contains(k), "enumerated key not contained: {:?}", k);
    }
    // order: within the top section (D, r, line sub-vector, other sub-vector) ascending
    let lines: Vec<usize> = (0..b.arity()).filter(|&i| b.chart[i]).collect();
    let others: Vec<usize> = (0..b.arity()).filter(|&i| !b.chart[i]).collect();
    let n_top: u128 = b
        .layer_counts()
        .iter()
        .filter(|c| c.section == 0)
        .map(|c| c.count)
        .sum();
    let top: Vec<&Key> = ks.iter().take(n_top as usize).collect();
    prop_assert!(top.iter().all(|k| lines.iter().all(|&i| k[i] >= 1)));
    // the explicit extras come last, sorted
    let n_layered: u128 = b.layer_counts().iter().map(|c| c.count).sum();
    let extras = &ks[n_layered as usize..];
    prop_assert!(extras.windows(2).all(|w| w[0] < w[1]));
    let sort_key = |k: &Key| {
        let d: i32 = lines.iter().map(|&i| k[i] - 1).sum();
        let r: i32 = others.iter().map(|&i| -k[i]).sum();
        let l: Vec<i32> = lines.iter().map(|&i| k[i]).collect();
        let o: Vec<i32> = others.iter().map(|&i| k[i]).collect();
        (d, r, l, o)
    };
    for w in top.windows(2) {
        prop_assert!(sort_key(w[0]) < sort_key(w[1]), "order {:?} {:?}", w[0], w[1]);
    }
    // the visitor and next_into give the same stream
    let mut v = vec![];
    b.for_each_key(|k| v.push(k.to_vec()));
    prop_assert_eq!(&v, &ks);
    // digest stable under a JSON round trip with reversed object keys
    let val = b.to_value();
    let mut rev = serde_json::Map::new();
    let obj = val.as_object().unwrap();
    let mut names: Vec<&String> = obj.keys().collect();
    names.sort();
    names.reverse();
    for n in names {
        rev.insert(n.clone(), obj[n].clone());
    }
    let mut rev = serde_json::Value::Object(rev);
    rev.as_object_mut().unwrap().remove("digest");
    // reorder layers and keys too
    if let Some(a) = rev["layers"].as_array_mut() {
        a.reverse();
    }
    if let Some(a) = rev["keys"].as_array_mut() {
        a.reverse();
    }
    let back = BoxDef::from_value(&rev).map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert_eq!(back.digest(), b.digest());
    // saturating with the box's own keys is a fixed point
    let s = b.saturate(ks.iter().map(|k| k.as_slice()));
    prop_assert!(s.is_fixed_point());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn prop_arity5(b in arb_box(5)) { check_box(&b)?; brute_force(&b)?; }
    #[test]
    fn prop_arity9(b in arb_box(9)) { check_box(&b)?; }
    #[test]
    fn prop_arity14(b in arb_box(14)) { check_box(&b)?; }
    #[test]
    fn prop_arity20(b in arb_box(20)) { check_box(&b)?; }
}

/// Arity 5: membership over a window that contains the whole box equals the enumeration.
fn brute_force(b: &BoxDef) -> Result<(), TestCaseError> {
    let b = b.normalised();
    let ks: BTreeSet<Key> = b.iter().collect();
    let mut found = BTreeSet::new();
    let mut k = vec![-4i32; 5];
    loop {
        if b.contains(&k) {
            found.insert(k.clone());
        }
        let mut i = 0;
        loop {
            if i == 5 {
                // explicit keys may lie outside the window
                for e in &b.keys {
                    found.insert(e.clone());
                }
                prop_assert_eq!(found, ks);
                return Ok(());
            }
            if k[i] < 6 {
                k[i] += 1;
                break;
            }
            k[i] = -4;
            i += 1;
        }
    }
}
