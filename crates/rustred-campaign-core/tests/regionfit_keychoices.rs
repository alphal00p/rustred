//! regionfit on the repair flow's per-key record of ALL acceptable choices (`KeyChoices`, repairflow2): the adapter on
//! constructed records, and the REAL T178 I1 record (371 unserved arrival keys vs the hand tables; rr_rf_symflow
//! f0b25bbb run t178-i1-fix1, rhs stripped in the fixture copy). Skips the real part without the fixture tree.

mod regionfit_common;

use regionfit_common::{check_record, summary};
use rustred_campaign_core::regionfit::keychoices::{
    ChoiceLabel, KeyChoicesOptions, faces_from_key_choices, ranking_from_preferred,
};
use rustred_campaign_core::regionfit::{RegionPolicy, fit_regions};
use rustred_campaign_core::repairflow::composite::{Choice, KEY_CHOICES_SCHEMA, KeyChoices};
use std::path::PathBuf;

fn choice(e: usize, kind: &str, form: &str, reg: Option<bool>) -> Choice {
    Choice {
        id: format!("sym:e{e}"),
        kind: kind.into(),
        element: Some(e),
        form: form.into(),
        rhs: vec![],
        records: vec![],
        key_solved: false,
        regular_at_4: reg,
    }
}

fn rec(key: &[i32], face: &str, choices: Vec<Choice>, preferred: Option<usize>) -> KeyChoices {
    KeyChoices {
        schema: KEY_CHOICES_SCHEMA.into(),
        key: key.to_vec(),
        face: face.into(),
        face_slots: vec![0, 2],
        choices,
        refused: vec![],
        preferred,
        status: "ok".into(),
    }
}

#[test]
fn adapter_constructed() {
    // face {0,2}: e1 (plain) on n0 <= n2, e2 (composite with a rule) elsewhere; one key with nothing
    let mut rs = vec![];
    for a in 2..=5 {
        for c in 2..=5 {
            let ch = if (a, c) == (5, 2) {
                vec![]
            } else if a <= c {
                vec![choice(1, "plain", "e1", None)]
            } else {
                vec![
                    choice(2, "composite", "e2+fi-c041#0", None),
                    choice(3, "composite", "e3+bank#944", Some(false)),
                ]
            };
            let p = (!ch.is_empty()).then_some(0);
            rs.push(rec(&[a, 1, c], "{0,2}x{}", ch, p));
        }
    }
    let o = KeyChoicesOptions::default();
    let faces = faces_from_key_choices(&rs, &o).unwrap();
    assert_eq!(faces.len(), 1);
    let rank = ranking_from_preferred(&rs, &o);
    assert_eq!(rank[0], "e1");
    let p = RegionPolicy {
        ranking: rank,
        ..RegionPolicy::default()
    };
    let fit = fit_regions(&faces[0], None, &p).unwrap();
    let r = fit.record();
    check_record(&faces[0], &r);
    assert_eq!(fit.unservable, vec![vec![5, 1, 2]]);
    assert!(summary(&r).contains(&("n0<=n2".to_string(), "e1".to_string())));
    // labels by element; singular-at-4 choices dropped
    let o2 = KeyChoicesOptions {
        label: ChoiceLabel::Element,
        exclude_singular_at_4: true,
        kinds: None,
    };
    let f2 = faces_from_key_choices(&rs, &o2).unwrap();
    assert!(f2[0].keys.iter().all(|k| !k.acceptable.contains("e3")));
    assert!(f2[0].keys.iter().any(|k| k.acceptable.contains("e2")));
    // only plain kinds
    let o3 = KeyChoicesOptions {
        kinds: Some(["plain".to_string()].into_iter().collect()),
        ..KeyChoicesOptions::default()
    };
    let f3 = faces_from_key_choices(&rs, &o3).unwrap();
    assert_eq!(
        f3[0]
            .keys
            .iter()
            .filter(|k| k.acceptable.is_empty())
            .count(),
        6
    );
    // refusals: another schema, a key twice
    let mut bad = rs.clone();
    bad[0].schema = "x".into();
    assert!(faces_from_key_choices(&bad, &o).is_err());
    let mut dup = rs.clone();
    dup.push(rs[0].clone());
    assert!(faces_from_key_choices(&dup, &o).is_err());
}

fn fixture() -> Option<PathBuf> {
    let base = std::env::var("RUSTRED_CAMPAIGN_FIXTURES").unwrap_or_else(|_| {
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/fixtures".into()
    });
    let r = PathBuf::from(base).join("regionfit");
    let p = r.join("T178keychoices/choices.rhs-stripped.jsonl");
    if !p.exists() {
        eprintln!("SKIP: no {}", p.display());
        return None;
    }
    let man = std::fs::read_to_string(r.join("MANIFEST.sha256")).unwrap();
    let line = man
        .lines()
        .find(|l| l.ends_with("T178keychoices/choices.rhs-stripped.jsonl"))
        .expect("manifest line");
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(
        rustred_campaign_core::closure::sha256::sha256_tag(&bytes),
        format!("sha256:{}", line.split_once("  ").unwrap().0)
    );
    Some(p)
}

#[test]
fn t178_keychoices_regions_per_face() {
    let Some(p) = fixture() else { return };
    let rs: Vec<KeyChoices> = std::fs::read_to_string(&p)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let none = rs.iter().filter(|r| r.choices.is_empty()).count();
    let comp_only = rs
        .iter()
        .filter(|r| !r.choices.is_empty() && r.choices.iter().all(|c| c.kind == "composite"))
        .count();
    println!(
        "T178 I1 KeyChoices: {} records, {none} with no choice, {comp_only} composite only",
        rs.len()
    );
    for label in [ChoiceLabel::Form, ChoiceLabel::Element] {
        let o = KeyChoicesOptions {
            label,
            ..KeyChoicesOptions::default()
        };
        let rank = ranking_from_preferred(&rs, &o);
        let p = RegionPolicy {
            ranking: rank,
            ..RegionPolicy::default()
        };
        let faces = faces_from_key_choices(&rs, &o).unwrap();
        let (mut regions, mut residual, mut unserv) = (0, 0, 0);
        for f in &faces {
            let fit = fit_regions(f, None, &p).unwrap();
            let r = fit.record();
            check_record(f, &r);
            regions += fit.regions.len();
            residual += fit.residual.len();
            unserv += fit.unservable.len();
            if let Ok(d) = std::env::var("REGIONFIT_REPORT_DIR") {
                std::fs::create_dir_all(&d).unwrap();
                let name = format!(
                    "T178kc.{label:?}.{}",
                    f.signature.replace(['{', '}', ','], "_")
                );
                std::fs::write(
                    PathBuf::from(d).join(format!("{name}.json")),
                    fit.record_text(),
                )
                .unwrap();
            }
            println!(
                "KC {label:?} {}: keys {} | {} regions, {} per key, {} unservable | {} | {:?}",
                f.signature,
                fit.keys,
                fit.regions.len(),
                fit.residual.len(),
                fit.unservable.len(),
                fit.method.text(),
                summary(&r)
                    .iter()
                    .zip(&fit.regions)
                    .map(|((t, c), x)| format!("{t} -> {c} ({} keys)", x.keys))
                    .collect::<Vec<_>>()
            );
        }
        println!(
            "KC {label:?} TOTAL: {} faces, {regions} regions, {residual} per key, {unserv} unservable",
            faces.len()
        );
        assert_eq!(unserv, none);
    }
}

/// planfix (audit r2-planners R1): under the ELEMENT label one region `e3` may stand for different composed relations
/// (element + image rule) per key; the record names the image rules of every region (and of every residual key), and
/// says whether the label fixes one. Under the FORM label (default) each region's image rule is its choice.
#[test]
fn an_element_label_region_names_the_image_rules_it_stands_for() {
    let forms = [
        ([2, 1, -1], "e3+T#1"),
        ([3, 1, -1], "e3+T#2"),
        ([4, 1, -1], "e3+T#2"),
        ([2, 1, -2], "e3+T#1"),
    ];
    let rs: Vec<KeyChoices> = forms
        .iter()
        .map(|(k, f)| rec(k, "f", vec![choice(3, "composite", f, None)], Some(0)))
        .collect();
    let el = KeyChoicesOptions {
        label: ChoiceLabel::Element,
        ..Default::default()
    };
    let faces = faces_from_key_choices(&rs, &el).unwrap();
    assert_eq!(faces.len(), 1);
    assert_eq!(faces[0].keys[0].image_rules["e3"].len(), 1);
    let rec_el = fit_regions(&faces[0], None, &RegionPolicy::default())
        .unwrap()
        .record();
    let regions = rec_el["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1, "{rec_el}");
    assert_eq!(regions[0]["choice"], "e3");
    assert_eq!(
        regions[0]["image_rules"],
        serde_json::json!(["e3+T#1", "e3+T#2"]),
        "{rec_el}"
    );
    assert_eq!(regions[0]["image_rule_uniform"], false);
    // the form label: one image rule per region, the choice itself
    let faces = faces_from_key_choices(&rs, &KeyChoicesOptions::default()).unwrap();
    let rec_f = fit_regions(&faces[0], None, &RegionPolicy::default())
        .unwrap()
        .record();
    for r in rec_f["regions"].as_array().unwrap() {
        assert_eq!(
            r["image_rules"],
            serde_json::json!([r["choice"].clone()]),
            "{rec_f}"
        );
        assert_eq!(r["image_rule_uniform"], true);
    }
    for r in rec_f["residual"].as_array().unwrap() {
        assert_eq!(
            r["image_rules"],
            serde_json::json!([r["choice"].clone()]),
            "{rec_f}"
        );
    }
}
