//! AUDIT (audit-r2-planners): lane regions' `fit_regions` is a GENERALISATION from the keys handed in. Brief 3: find a
//! region whose fit is perfect on the sample and wrong outside it, and follow it through the accept path.
//!
//! Found [read + this test]: the fit claims nothing outside its list (record: "verified on N keys of the box"); the
//! deeper list only fills `extends {n, m}`; NO production code consumes a region record (grep: no caller of
//! `fit_regions` outside src/regionfit and tests), so no key outside the sample can be served by a region today. The
//! label `ChoiceLabel::Element` hides the image rule: one region "e3" stands for different composed relations per key.
//! Pure; toy keys (L8).

use std::collections::BTreeSet;

use rustred_campaign_core::regionfit::{FaceInput, FaceKey, RegionPolicy, fit_regions};

fn fk(key: &[i32], acc: &[&str]) -> FaceKey {
    FaceKey {
        key: key.to_vec(),
        acceptable: acc.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>(),
        image_rules: Default::default(),
    }
}

#[test]
fn a_region_perfect_on_the_sample_and_wrong_one_layer_deeper_is_emitted_with_extends_0_of_m_and_claims_only_the_sample()
 {
    // sample: n0 in 2..=4 (the box), every key accepts g1 only; one layer deeper (n0 = 5, 6) only g2 serves
    let mut keys = vec![];
    for a in 2..=4 {
        for c in -2..=-1 {
            keys.push(fk(&[a, 1, c], &["g1"]));
        }
    }
    let deeper: Vec<FaceKey> = (5..=6)
        .flat_map(|a| (-2..=-1).map(move |c| fk(&[a, 1, c], &["g2"])))
        .collect();
    let face = FaceInput {
        signature: "audit-face".into(),
        keys,
    };
    let fit = fit_regions(&face, Some(&deeper), &RegionPolicy::default()).unwrap();
    let rec = fit.record();
    let regions = rec["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1, "{rec}");
    assert_eq!(regions[0]["choice"], "g1");
    assert_eq!(regions[0]["verified_on"], "verified on 6 keys of the box");
    // the generalisation fails at every deeper key, and the record says so
    assert_eq!(regions[0]["extends"]["n"], 0, "{rec}");
    assert_eq!(regions[0]["extends"]["m"], 4, "{rec}");
}

#[test]
fn an_element_label_region_spans_keys_whose_composed_relations_differ() {
    // the composed form (element + image rule) differs by key; labelled by ELEMENT the face is one region "e3"
    let keys_form = vec![
        fk(&[2, 1, -1], &["e3+T#1"]),
        fk(&[3, 1, -1], &["e3+T#2"]),
        fk(&[4, 1, -1], &["e3+T#2"]),
        fk(&[2, 1, -2], &["e3+T#1"]),
    ];
    let keys_el: Vec<FaceKey> = keys_form.iter().map(|k| fk(&k.key, &["e3"])).collect();
    let p = RegionPolicy::default();
    let by_el = fit_regions(
        &FaceInput {
            signature: "f".into(),
            keys: keys_el,
        },
        None,
        &p,
    )
    .unwrap()
    .record();
    let by_form = fit_regions(
        &FaceInput {
            signature: "f".into(),
            keys: keys_form,
        },
        None,
        &p,
    )
    .unwrap()
    .record();
    assert_eq!(by_el["regions"].as_array().unwrap().len(), 1, "{by_el}");
    assert!(
        by_form["regions"].as_array().unwrap().len() >= 2,
        "the forms need a split: {by_form}"
    );
}
