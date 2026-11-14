//! Symmetry groups and rows on the coordinator's M2 nodes of other arities (WS/fixtures/campaign-20261114/m2:
//! TOY2kite arity 5, L3a 9, L4a 14, T281 / T194 20; interfaces/coord-fixtures.md). Per node: the supplied group
//! (`SYMMETRY.json`, schema c-fix.symmetry-group.v1, elements from the frozen rr_node symmasters) is read, VERIFIED
//! exactly, and cross-checked against the group derived independently from the family text (numerator images compared
//! exactly); a planted wrong element is refused; one-term rules made of a symmetry relation pass with the group and fail
//! without; on a node with a multi-term numerator map (L3a), the expanded mode decides a rule the engine mode cannot.

mod rows_common;

use std::sync::Arc;

use rustred_campaign_core::ibp_rows::{
    PointSpec, RowsOptions, RowsRule, SymUse, Verdict, identity_test,
};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, GroupCheck, SlotImage, SymElement, SymGroup, SymRowMode, cross_check,
    derive_group, rows_at, verify_group,
};
use serde_json::{Value, json};

const M2: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/fixtures/campaign-20261114/m2";

fn node(name: &str) -> Option<(Family, Value)> {
    let fam = std::fs::read_to_string(format!("{M2}/{name}/family.sym")).ok()?;
    let sym = std::fs::read_to_string(format!("{M2}/{name}/SYMMETRY.json")).ok()?;
    Some((
        Family::parse(&fam).unwrap(),
        serde_json::from_str(&sym).unwrap(),
    ))
}

fn key_text(k: &[i64]) -> String {
    format!(
        "I({})",
        k.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    )
}

fn point_rule(chart: &[bool], target: &[i64], rhs: Vec<(Vec<i64>, String)>) -> Value {
    json!({"type": "rule", "index": 0, "chart": chart.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>(),
        "fixed": target, "equations": [], "exceptions": [], "target": key_text(target),
        "rhs": rhs.into_iter().map(|(k, c)| json!([key_text(&k), c])).collect::<Vec<_>>(),
        "seeds": [key_text(target)], "recenter": vec![0; target.len()], "witness_loci": [], "sym_rows": 1})
}

#[test]
fn rows_sym_m2_groups_supplied_verified_cross_checked() {
    let mut seen = 0;
    for name in ["TOY2kite", "L3a", "L4a", "T281", "T194"] {
        let Some((fam, sym)) = node(name) else {
            println!("SKIP {name}");
            continue;
        };
        let supplied = SymGroup::from_cfix_json(&sym, &fam).unwrap();
        let t0 = std::time::Instant::now();
        let (derived, st) = derive_group(&fam, &supplied.chart, &DeriveOptions::default()).unwrap();
        let dsecs = t0.elapsed().as_secs_f64();
        let c = cross_check(&derived, &supplied);
        let linear = supplied
            .elements
            .iter()
            .flat_map(|e| &e.action)
            .filter(|s| matches!(s, SlotImage::Linear { .. }))
            .count();
        println!(
            "{name}: arity {} |G| = {} (supplied, incl. identity {}), derived {} ({} candidates, {dsecs:.2}s), {linear} linear numerator images; cross-check {c:?}",
            fam.n(),
            supplied.elements.len() + 1,
            sym["order_full"],
            derived.elements.len() + 1,
            st.candidates
        );
        assert_eq!(
            supplied.elements.len() + 1,
            sym["order_full"].as_u64().unwrap() as usize
        );
        assert!(
            matches!(
                c,
                GroupCheck::Equal {
                    skipped_slots: 0,
                    ..
                }
            ),
            "{name}: {c:?}"
        );
        let v = verify_group(&fam, supplied.clone(), &format!("{name}/SYMMETRY.json")).unwrap();
        let vd = verify_group(&fam, derived, "derived").unwrap();
        assert_eq!(v.digest(), vd.digest());
        // planted: an element with two chart lines swapped on top of a true element's action
        let mut bad = supplied.clone();
        let lines: Vec<usize> = (0..fam.n()).filter(|&i| supplied.chart[i]).collect();
        let mut action: Vec<SlotImage> = (0..fam.n())
            .map(|i| SlotImage::Exact {
                target: i,
                flip: false,
            })
            .collect();
        action[lines[0]] = SlotImage::Exact {
            target: lines[1],
            flip: false,
        };
        action[lines[1]] = SlotImage::Exact {
            target: lines[0],
            flip: false,
        };
        let is_true = supplied.elements.iter().any(|e| {
            e.line_perm(&supplied.chart)
                == SymElement {
                    matrix: None,
                    action: action.clone(),
                }
                .line_perm(&supplied.chart)
        });
        if !is_true {
            bad.elements.push(SymElement {
                matrix: None,
                action,
            });
            assert!(verify_group(&fam, bad, "planted").is_err(), "{name}");
        }
        seen += 1;
    }
    if seen > 0 {
        assert_eq!(seen, 5);
    }
}

#[test]
fn rows_sym_m2_one_term_rules_with_and_without() {
    for name in ["TOY2kite", "L3a", "L4a", "T281", "T194"] {
        let Some((fam, sym)) = node(name) else {
            continue;
        };
        let g = SymGroup::from_cfix_json(&sym, &fam).unwrap();
        let chart = g.chart.clone();
        let vg = Arc::new(verify_group(&fam, g, &name).unwrap());
        let (mut pass, mut fail_without, mut n) = (0, 0, 0);
        for (k, e) in vg.group().elements.iter().enumerate() {
            // a dotted corner whose dot moves: target = corner + e_i, image = corner + e_{g(i)}
            let Some(i) = (0..fam.n()).find(|&i| {
                chart[i]
                    && e.action[i]
                        != SlotImage::Exact {
                            target: i,
                            flip: false,
                        }
            }) else {
                continue;
            };
            let SlotImage::Exact { target: j, .. } = e.action[i] else {
                unreachable!()
            };
            let mut a: Vec<i64> = chart.iter().map(|&b| i64::from(b)).collect();
            let mut b = a.clone();
            a[i] += 1;
            b[j] += 1;
            let r = RowsRule::from_value(point_rule(&chart, &a, vec![(b, "1".into())]), fam.n())
                .unwrap();
            let with = RowsOptions {
                symmetry: Some(vg.clone()),
                ..RowsOptions::default()
            };
            let without = RowsOptions {
                symmetry: Some(vg.clone()),
                sym_use: SymUse::Without,
                ..RowsOptions::default()
            };
            let v = identity_test(&fam, &r, &[PointSpec::Key(a.clone())], &with);
            let w = identity_test(&fam, &r, &[PointSpec::Key(a)], &without);
            assert_eq!(v.verdict, Verdict::Pass, "{name} element {k}");
            assert!(
                v.symmetry.as_ref().unwrap().support.contains(&k)
                    || !v.symmetry.as_ref().unwrap().support.is_empty()
            );
            pass += 1;
            if matches!(w.verdict, Verdict::Fail { .. }) {
                fail_without += 1;
            }
            n += 1;
        }
        println!(
            "{name}: {n} one-term symmetry rules: {pass} pass with the group, {fail_without} fail without"
        );
        assert_eq!(
            fail_without, n,
            "{name}: IBP rows at one seed must not prove a symmetry relation"
        );
    }
}

/// L3a: an element maps the numerator D9 to D1+D2+D3-D8-D9+3 (multi-term). At the key with D9 = -1 the engine
/// (one-term) rows of that element do not exist; the expanded mode gives the multi-term relation exactly.
#[test]
fn rows_sym_m2_multi_term_numerator_map_l3a() {
    let Some((fam, sym)) = node("L3a") else {
        return;
    };
    let g = SymGroup::from_cfix_json(&sym, &fam).unwrap();
    let chart = g.chart.clone();
    let vg = Arc::new(verify_group(&fam, g, "L3a").unwrap());
    let (k, _) = vg
        .group()
        .elements
        .iter()
        .enumerate()
        .find(|(_, e)| matches!(e.action[8], SlotImage::Linear { .. }))
        .unwrap();
    let mut a: Vec<i64> = chart.iter().map(|&b| i64::from(b)).collect();
    a[0] = 2;
    a[8] = -1;
    let rows = rows_at(&vg, &(0, a.clone()), SymRowMode::Expanded, 1000).unwrap();
    let row = rows.iter().find(|r| r.element == k).unwrap();
    println!(
        "L3a expanded row of element {k} at {a:?}: {} terms",
        row.terms.len()
    );
    assert!(row.terms.len() > 2);
    assert!(
        rows_at(&vg, &(0, a.clone()), SymRowMode::Engine, 1000)
            .unwrap()
            .iter()
            .all(|r| r.element != k)
    );
    // the rule: I(a) = -sum_{others} c I(key)
    let rhs: Vec<(Vec<i64>, String)> = row
        .terms
        .iter()
        .filter(|(key, _)| key.1 != a)
        .map(|(key, c)| {
            let c = c.neg();
            (
                key.1.clone(),
                if c.d == 1 {
                    c.n.to_string()
                } else {
                    format!("({})/({})", c.n, c.d)
                },
            )
        })
        .collect();
    let r = RowsRule::from_value(point_rule(&chart, &a, rhs), fam.n()).unwrap();
    let pts = [PointSpec::Key(a)];
    let eng = identity_test(
        &fam,
        &r,
        &pts,
        &RowsOptions {
            symmetry: Some(vg.clone()),
            ..RowsOptions::default()
        },
    );
    let exp = identity_test(
        &fam,
        &r,
        &pts,
        &RowsOptions {
            symmetry: Some(vg.clone()),
            sym_mode: SymRowMode::Expanded,
            ..RowsOptions::default()
        },
    );
    println!(
        "L3a multi-term rule: engine mode {}, expanded mode {} support {:?}",
        eng.verdict.kind(),
        exp.verdict.kind(),
        exp.symmetry.as_ref().map(|s| &s.support)
    );
    assert_eq!(exp.verdict, Verdict::Pass);
    assert!(exp.symmetry.as_ref().unwrap().support.contains(&k));
}
