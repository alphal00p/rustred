//! PART 3 (rows3): the whole checker in `Both` mode. Clean rules -> `AgreePass`; corrupted copies -> `AgreeFail`;
//! PLANTED backend defects (one wrong field multiplication, one wrong parsed coefficient, one dropped row, on either
//! side) -> `Differ` localised to the right stage and the right side (the triage's power). Costs per rule and mode
//! are printed. Every test SKIPs without the Symbolica licence (source WS/ops/env.sh).

mod common;

use std::sync::Arc;
use std::time::Instant;

use common::*;
use rustred_campaign_algebra::wrap::{Plant, planted, recording};
use rustred_campaign_algebra::{
    IdentityMode, Outlier, SymbolicaAlgebra, Triage, identity_test_mode, replay_evidence_mode,
    replay_evidence_triage, triage_identity,
};
use rustred_campaign_core::ibp_rows::{
    NonceSource, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
    os_nonce,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::Stage;
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, SymGroup, VerifiedGroup, verify_derived, verify_group,
};
use serde_json::Value;

fn t162_text() -> String {
    core_data("T162-presented-P3.sym")
}

/// Run one rule in `Both` mode; print the triage line; return the triage.
fn both(
    name: &str,
    family_text: &str,
    rule: &RowsRule,
    points: &[PointSpec],
    opts: &RowsOptions,
) -> Triage {
    let m = identity_test_mode(IdentityMode::Both, family_text, rule, points, opts).unwrap();
    let t = m.triage.unwrap();
    println!(
        "{name}: {:?} | independent {:.3} s, symbolica {:.3} s (x{:.1}), referee {:.3} s, trace {} entries",
        t.triage,
        t.seconds_independent,
        t.seconds_symbolica,
        t.seconds_symbolica / t.seconds_independent.max(1e-9),
        t.seconds_referee,
        t.trace_len_independent
    );
    t.triage
}

fn census_keys(case: &Value, max: usize) -> Vec<PointSpec> {
    let mut out = Vec::new();
    for k in ["C2", "C5"] {
        for p in case["clean"][0][k].as_array().into_iter().flatten() {
            if p["verdict"] == "covered_regular" && out.len() < max {
                out.push(PointSpec::Key(
                    p["point"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_i64().unwrap())
                        .collect(),
                ));
            }
        }
    }
    out
}

/// The frozen F6 span cases: clean rules AgreePass (generic points + census keys), corrupted copies AgreeFail; the
/// symmetry-row cases fa001 / f1003 / bp010 with the verified T162 group AgreePass.
#[test]
fn both_frozen_span_cases() {
    if !licensed_or_skip("both_frozen_span_cases") {
        return;
    }
    let Some(o) = oracle() else { return };
    let text = t162_text();
    let fam = Family::parse(&text).unwrap();
    let chart: Vec<bool> = (0..20).map(|i| i < 9).collect();
    let group: Arc<VerifiedGroup> =
        Arc::new(verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap());
    let (mut pass, mut fail) = (0, 0);
    for case in o.jsonl("span/F6_span_cases.jsonl") {
        let id = case["id"].as_str().unwrap().to_string();
        let Some(line) = f6_line(&o, &case) else {
            return;
        };
        let rule = rule0(&line, 20);
        if id == "bp002" {
            continue; // 9.6 s per independent run: in the ignored release test below
        }
        let mut points = vec![PointSpec::Generic, PointSpec::Generic];
        if rule.fixed.iter().all(Option::is_some) {
            points = vec![PointSpec::Key(record_key(&rule))];
        }
        points.extend(census_keys(&case, 3));
        let opts = RowsOptions {
            symmetry: rule.sym_rows.is_some().then(|| group.clone()),
            ..RowsOptions::default()
        };
        let t = both(&format!("F6 {id}"), &text, &rule, &points, &opts);
        assert_eq!(t, Triage::AgreePass, "{id}");
        pass += 1;
        // the corrupted copy: rhs[0]'s key shifted by +1 in its first slot (as rows_fc000's planted key)
        if rule.sym_rows.is_none() {
            let mut bad = rule.clone();
            let j = bad.rhs[0].0.iter().position(|x| x.0.is_some()).unwrap_or(0);
            bad.rhs[0].0[j].1 += 1;
            let t = both(&format!("F6 {id} corrupted"), &text, &bad, &points, &opts);
            assert!(
                matches!(t, Triage::AgreeFail { .. } | Triage::AgreeOther { .. }),
                "{id} corrupted: {t:?}"
            );
            fail += usize::from(matches!(t, Triage::AgreeFail { .. }));
        }
    }
    println!("F6: {pass} AgreePass, {fail} corrupted AgreeFail");
    assert!(pass >= 10 && fail >= 6, "{pass} {fail}");
}

fn record_key(rule: &RowsRule) -> Vec<i64> {
    let pt: Vec<i64> = rule.fixed.iter().map(|f| f.unwrap_or(0)).collect();
    rule.target
        .iter()
        .map(|&(ax, off)| ax.map_or(off, |a| pt[a] + off))
        .collect()
}

/// The repair witnesses of the campaign fixtures at arities 5 / 9 / 14 / 20 (frozen rr_node new rules).
#[test]
fn both_repair_witnesses() {
    if !licensed_or_skip("both_repair_witnesses") {
        return;
    }
    let fx = campaign_fixtures();
    let mut total = (0, 0);
    for (node, arity) in [("TOY2kite", 5), ("L3a", 9), ("L4a", 14), ("T281-b6", 20)] {
        let dir = format!("{fx}/small/m2-slice/{node}");
        let text = std::fs::read_to_string(format!("{dir}/family.sym")).unwrap();
        let fam = Family::parse(&text).unwrap();
        let group = std::fs::read_to_string(format!("{dir}/SYMMETRY.json"))
            .ok()
            .and_then(|g| SymGroup::from_cfix_json(&serde_json::from_str(&g).unwrap(), &fam).ok())
            .and_then(|g| verify_group(&fam, g, &format!("{node}/SYMMETRY.json")).ok())
            .map(Arc::new);
        for k in std::fs::read_dir(format!("{dir}/ko")).unwrap() {
            let path = k.unwrap().path().join("repair_witness/new_rules.jsonl.gz");
            if !path.exists() {
                continue;
            }
            let (_, rules) = table_rules(path.to_str().unwrap(), arity, 12);
            for (i, rule) in rules.iter().enumerate() {
                let points = if rule.fixed.iter().all(Option::is_some) {
                    vec![PointSpec::Key(record_key(rule))]
                } else {
                    vec![PointSpec::Generic, PointSpec::Generic]
                };
                let opts = RowsOptions {
                    symmetry: group.clone(),
                    ..RowsOptions::default()
                };
                let name = format!(
                    "{node} {} rule {i}",
                    path.parent().unwrap().parent().unwrap().display()
                );
                let t = both(&name, &text, rule, &points, &opts);
                assert!(!matches!(t, Triage::Differ(_)), "{name}: {t:?}");
                total.0 += 1;
                total.1 += usize::from(t == Triage::AgreePass);
            }
        }
    }
    println!(
        "repair witnesses: {} rules, {} AgreePass, no Differ",
        total.0, total.1
    );
    assert!(total.0 > 0);
}

/// A sample of the exported, checker-agreed T162 rules (batch-01 f faces, batch-02 fd faces).
#[test]
fn both_checker_agreed_sample() {
    if !licensed_or_skip("both_checker_agreed_sample") {
        return;
    }
    let text = t162_text();
    let mut n = 0;
    for (file, max) in [
        ("batch-02/fd.faces.jsonl", 7),
        ("batch-01/f.faces.jsonl", 12),
    ] {
        let path = format!("{HO}/t162-faces/{file}");
        if !std::path::Path::new(&path).exists() {
            println!("SKIP: {path}");
            return;
        }
        let (_, rules) = table_rules(&path, 20, max);
        for (i, rule) in rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.sym_rows.is_none())
        {
            let t = both(
                &format!("{file} rule {i}"),
                &text,
                rule,
                &[PointSpec::Generic, PointSpec::Generic],
                &RowsOptions::default(),
            );
            assert_eq!(t, Triage::AgreePass, "{file} rule {i}");
            n += 1;
        }
    }
    println!("checker-agreed sample: {n} rules AgreePass");
}

/// A small real rule for the planted-defect tests: L3a's first table rule (arity 9).
fn small_rule() -> (String, RowsRule) {
    let fx = campaign_fixtures();
    let dir = format!("{fx}/small/m2-slice/L3a");
    let text = std::fs::read_to_string(format!("{dir}/family.sym")).unwrap();
    let (_, rules) = table_rules(&format!("{dir}/table.jsonl"), 9, 40);
    let rule = rules
        .into_iter()
        .find(|r| r.sym_rows.is_none() && r.fixed.iter().any(Option::is_none) && r.rhs.len() > 4)
        .unwrap();
    (text, rule)
}

/// PLANTED DEFECTS: each yields `Differ`, at the planted stage, attributed to the planted side.
#[test]
fn planted_defects_are_localised() {
    if !licensed_or_skip("planted_defects_are_localised") {
        return;
    }
    let (text, rule) = small_rule();
    let pts = [PointSpec::Generic, PointSpec::Generic];
    let opts = RowsOptions::default();
    let sym = || SymbolicaAlgebra::new().unwrap();
    let clean = triage_identity(
        &recording(Independent),
        &recording(sym()),
        &text,
        &rule,
        &pts,
        &opts,
    )
    .unwrap()
    .2;
    assert_eq!(clean.triage, Triage::AgreePass);
    let check = |label: &str, t: &Triage, stages: &[Stage], side: Outlier| {
        println!("planted {label}: {t:?}");
        match t {
            Triage::Differ(d) => {
                assert!(stages.contains(&d.stage), "{label}: stage {:?}", d.stage);
                assert_eq!(d.outlier, side, "{label}: outlier");
            }
            other => panic!("{label}: expected Differ, got {other:?}"),
        }
    };
    let mut rows_hit = false;
    for nth in [0usize, 40, 400] {
        let p = Plant::Multiplication { nth };
        let t = triage_identity(
            &planted(Independent, p),
            &recording(sym()),
            &text,
            &rule,
            &pts,
            &opts,
        )
        .unwrap()
        .2
        .triage;
        check(
            &format!("mul #{nth} independent"),
            &t,
            &[Stage::Coefficient, Stage::Rows],
            Outlier::Independent,
        );
        rows_hit |= matches!(&t, Triage::Differ(d) if d.stage == Stage::Rows);
        let t = triage_identity(
            &recording(Independent),
            &planted(sym(), p),
            &text,
            &rule,
            &pts,
            &opts,
        )
        .unwrap()
        .2
        .triage;
        check(
            &format!("mul #{nth} symbolica"),
            &t,
            &[Stage::Coefficient, Stage::Rows],
            Outlier::Symbolica,
        );
    }
    assert!(
        rows_hit,
        "a planted multiplication must also be caught in the rows"
    );
    for nth in [0usize, 3] {
        let p = Plant::Coefficient { nth };
        let t = triage_identity(
            &planted(Independent, p),
            &recording(sym()),
            &text,
            &rule,
            &pts,
            &opts,
        )
        .unwrap()
        .2
        .triage;
        check(
            &format!("coefficient #{nth} independent"),
            &t,
            &[Stage::Coefficient],
            Outlier::Independent,
        );
        let t = triage_identity(
            &recording(Independent),
            &planted(sym(), p),
            &text,
            &rule,
            &pts,
            &opts,
        )
        .unwrap()
        .2
        .triage;
        check(
            &format!("coefficient #{nth} symbolica"),
            &t,
            &[Stage::Coefficient],
            Outlier::Symbolica,
        );
    }
    let p = Plant::DropRow { nth: 0 };
    let t = triage_identity(
        &planted(Independent, p),
        &recording(sym()),
        &text,
        &rule,
        &pts,
        &opts,
    )
    .unwrap()
    .2
    .triage;
    check(
        "drop row independent",
        &t,
        &[Stage::Elimination],
        Outlier::Independent,
    );
    let t = triage_identity(
        &recording(Independent),
        &planted(sym(), p),
        &text,
        &rule,
        &pts,
        &opts,
    )
    .unwrap()
    .2
    .triage;
    check(
        "drop row symbolica",
        &t,
        &[Stage::Elimination],
        Outlier::Symbolica,
    );
}

/// The `replay` level in the three modes: Independent grants it (algebra "independent"); Symbolica is Diagnostic;
/// Both with agreement keeps it (one OS nonce, NonceSource::Os); Both with a planted defect BLOCKS it and raises a
/// typed finding.
#[test]
fn replay_level_in_the_three_modes() {
    if !licensed_or_skip("replay_level_in_the_three_modes") {
        return;
    }
    let (text, rule) = small_rule();
    let fam = Family::parse(&text).unwrap();
    let opts = RowsOptions {
        table: Some(TableBinding::from_family_digest(fam.squashed_digest())),
        ..RowsOptions::default()
    };
    let served: Vec<Vec<i64>> = served_keys(&rule, 40)
        .into_iter()
        .filter(|k| {
            let v = rustred_campaign_core::ibp_rows::identity_test(
                &fam,
                &rule,
                &[PointSpec::Key(k.clone())],
                &RowsOptions::default(),
            );
            !matches!(
                v.points[0].status,
                rustred_campaign_core::ibp_rows::PointStatus::NotApplicable(_)
            )
        })
        .take(4)
        .collect();
    assert!(!served.is_empty());
    let pol = ReplayPolicy::default();
    let t0 = Instant::now();
    let ri = replay_evidence_mode(
        IdentityMode::Independent,
        &text,
        &rule,
        &served,
        &pol,
        &opts,
    )
    .unwrap();
    let ti = t0.elapsed().as_secs_f64();
    let ri = ri.independent.unwrap();
    println!(
        "replay Independent: {:?} algebra {} ({ti:.3} s)",
        ri.level, ri.algebra
    );
    assert_eq!(ri.algebra, "independent");
    assert_eq!(ri.level, ReplayLevel::Established);
    let t1 = Instant::now();
    let rs =
        replay_evidence_mode(IdentityMode::Symbolica, &text, &rule, &served, &pol, &opts).unwrap();
    let ts = t1.elapsed().as_secs_f64();
    let rs = rs.symbolica.unwrap();
    println!(
        "replay Symbolica: {:?} ({ts:.3} s, x{:.1})",
        rs.level,
        ts / ti.max(1e-9)
    );
    assert_eq!(rs.algebra, "symbolica");
    assert!(
        matches!(&rs.level, ReplayLevel::Diagnostic { level_if_certificate, .. }
        if **level_if_certificate == ReplayLevel::Established)
    );
    let rb = replay_evidence_mode(IdentityMode::Both, &text, &rule, &served, &pol, &opts).unwrap();
    let rec = rb.independent.unwrap();
    println!(
        "replay Both: {:?} triage {:?}",
        rec.level,
        rb.triage.as_ref().map(|t| &t.triage)
    );
    assert_eq!(rec.level, ReplayLevel::Established);
    assert_eq!(rec.nonce_source, NonceSource::Os);
    assert_eq!(rb.triage.unwrap().triage, Triage::AgreePass);
    assert!(rb.finding.is_none());
    // a planted Symbolica defect: Differ, the level is blocked, a typed finding is raised
    let drawn = os_nonce();
    let rp = replay_evidence_triage(
        &recording(Independent),
        // the pole scan evaluates every coefficient once per served key first: land in the identity cells
        &planted(
            SymbolicaAlgebra::new().unwrap(),
            Plant::Coefficient {
                nth: served.len() * rule.rhs.len() + 1,
            },
        ),
        &text,
        &rule,
        &served,
        &pol,
        &opts,
        drawn,
    )
    .unwrap();
    let rec = rp.independent.unwrap();
    println!(
        "replay Both with a planted Symbolica defect: {:?} finding {:?}",
        rec.level, rp.finding
    );
    assert!(matches!(rec.level, ReplayLevel::NotEstablished { .. }));
    let f = rp.finding.expect("typed finding");
    assert_eq!(f.difference.outlier, Outlier::Symbolica);
}

/// Integer keys inside the rule's case and chart (free slots at small excess), applicable by construction.
fn served_keys(rule: &RowsRule, max: usize) -> Vec<Vec<i64>> {
    let fam_n = rule.arity;
    let mut out = Vec::new();
    for e in 0..40i64 {
        let pt: Vec<i64> = (0..fam_n)
            .map(|i| match rule.fixed[i] {
                Some(v) => v,
                None if rule.chart[i] => 1 + (e + i as i64) % 3,
                None => -((e + 2 * i as i64) % 3),
            })
            .collect();
        let key: Vec<i64> = rule
            .target
            .iter()
            .map(|&(ax, off)| ax.map_or(off, |a| pt[a] + off))
            .collect();
        if rule
            .chart
            .iter()
            .zip(&key)
            .all(|(&l, &k)| if l { k >= 1 } else { k <= 0 })
            && !out.contains(&key)
        {
            out.push(key);
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Not a check of correctness: the cost of the two modes per rule on bp002 (rank 10,837) (ignored: slow).
#[test]
#[ignore]
fn cost_bp002_both() {
    if !licensed_or_skip("cost_bp002_both") {
        return;
    }
    let Some(o) = oracle() else { return };
    let text = t162_text();
    let fam = Family::parse(&text).unwrap();
    let chart: Vec<bool> = (0..20).map(|i| i < 9).collect();
    let group = Arc::new(verify_derived(&fam, &chart, &DeriveOptions::default()).unwrap());
    let case = o
        .jsonl("span/F6_span_cases.jsonl")
        .into_iter()
        .find(|c| c["id"] == "bp002")
        .unwrap();
    let Some(line) = f6_line(&o, &case) else {
        return;
    };
    let rule = rule0(&line, 20);
    let opts = RowsOptions {
        symmetry: Some(group),
        ..RowsOptions::default()
    };
    let t = both(
        "F6 bp002",
        &text,
        &rule,
        &[PointSpec::Key(record_key(&rule))],
        &opts,
    );
    assert_eq!(t, Triage::AgreePass);
}

/// Keys per second of integer-key replay cells on the largest frozen F6 rules (f000: rank 1,410; fp008: rank 4,041),
/// both backends, with the one-pass key batch (re-audit B m-2). Ignored: a measurement.
#[test]
#[ignore]
fn cost_keys_per_second() {
    if !licensed_or_skip("cost_keys_per_second") {
        return;
    }
    let Some(o) = oracle() else { return };
    let text = t162_text();
    let s = SymbolicaAlgebra::new().unwrap();
    for id in ["f000", "fp008", "fs023"] {
        let case = o
            .jsonl("span/F6_span_cases.jsonl")
            .into_iter()
            .find(|c| c["id"] == id)
            .unwrap();
        let Some(line) = f6_line(&o, &case) else {
            return;
        };
        let rule = rule0(&line, 20);
        let keys = census_keys(&case, 50);
        if keys.is_empty() {
            continue;
        }
        let pts: Vec<PointSpec> = (0..4).flat_map(|_| keys.clone()).collect();
        let fi = Family::parse(&text).unwrap();
        let t0 = Instant::now();
        let vi = rustred_campaign_core::ibp_rows::identity_test(
            &fi,
            &rule,
            &pts,
            &RowsOptions::default(),
        );
        let ti = t0.elapsed().as_secs_f64();
        let fs = Family::parse_with(&s, &text).unwrap();
        let t1 = Instant::now();
        let vs = rustred_campaign_core::ibp_rows::identity_test_with(
            &s,
            &fs,
            &rule,
            &pts,
            &RowsOptions::default(),
        );
        let ts = t1.elapsed().as_secs_f64();
        println!(
            "{id}: {} key points ({} coefficient bytes): independent {:.2} keys/s ({:?}), symbolica {:.2} keys/s ({:?})",
            pts.len(),
            rule.coefficient_bytes(),
            pts.len() as f64 / ti,
            vi.verdict.kind(),
            pts.len() as f64 / ts,
            vs.verdict.kind()
        );
    }
}
