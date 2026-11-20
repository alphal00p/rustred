//! Lane gencheck (emission contract, round 2 item 1): THE EMISSION GATE of a whole-table writer (`rr_campaign
//! emit-table`, used for node assembly and the closeloop relation / symmetry tables), with `rr_fake_child` standing in
//! for the independent checker in both modes (weight-cert for lifted records, instance-span for point records). The
//! fake binds its answers to the record bytes and certificate like `rr_proof`. FAIL CLOSED: one refused record and
//! nothing is written. Spawns processes (the checker children): run inside a private pid namespace (`LIGHT_PIDNS=1`).
use rustred_campaign::emission::emitter::{Checker, Emitter};
use rustred_campaign::emission::gate::{CertIndex, GateKind, cert_case, emit_table};
use rustred_campaign_core::emission::body::strip_derivation;
use rustred_campaign_core::emission::class::Policy;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");

fn tmp(tag: &str) -> PathBuf {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let d = PathBuf::from(base).join(format!("emission-gate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rule(i: usize, fixed: Value, case: &str, target: &str, extra: Option<(&str, &str)>) -> Value {
    let mut r = json!({"type":"rule","chart":"110","index":i,"case":case,"fixed":fixed,"equations":[],"target":target,
        "rhs":[["I(n0-1,n1,n2)","1"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]});
    if let Some((k, v)) = extra {
        r[k] = json!(v);
    }
    r
}

fn write_table(dir: &Path, rules: &[Value]) -> PathBuf {
    let t = dir.join("in.table.jsonl");
    let mut s = format!(
        "{}\n",
        json!({"type":"header","schema":"table_dump.v1","family":"TOY"})
    );
    for r in rules {
        s.push_str(&format!("{r}\n"));
    }
    s.push_str(&format!("{}\n", json!({"type":"end","rules":rules.len()})));
    std::fs::write(&t, s).unwrap();
    t
}

fn certs(dir: &Path, cases: &[&str]) -> PathBuf {
    let d = dir.join("certs");
    std::fs::create_dir_all(&d).unwrap();
    for (k, c) in cases.iter().enumerate() {
        // an engine certificate starts with its case (members sorted)
        std::fs::write(
            d.join(format!("cert-{k}.json")),
            json!({"case": c, "fake_checker": {}}).to_string(),
        )
        .unwrap();
    }
    d
}

fn run(dir: &Path, table: &Path, certs_dir: &Path) -> (Value, bool, PathBuf) {
    let out = dir.join("out.table.jsonl");
    let work = dir.join("work");
    let em_for = |side: &Path| -> Result<Emitter, String> {
        let mut ck = Checker::new(Path::new(FAKE))?;
        ck.recover_rows = false;
        // the fake engine descent test (contract section 14): every rule LOWER unless it says "fake_descent": "NotLower"
        ck.descent_node = Some((
            PathBuf::from(FAKE),
            rustred_campaign::emission::child::file_sha256(Path::new(FAKE))?,
        ));
        Emitter::new(ck, Policy::default(), side, &work)
    };
    let (s, ok) = emit_table(
        &em_for,
        table,
        &out,
        &CertIndex::from_dirs(&[certs_dir.to_path_buf()]),
        GateKind::Auto,
    )
    .unwrap();
    (s, ok, out)
}

#[test]
fn the_gate_emits_lifted_and_point_records_and_table_check_accepts() {
    let d = tmp("ok");
    let t = write_table(
        &d,
        &[
            rule(
                0,
                json!([null, null, null]),
                "Coordinate(A)",
                "I(n0,n1,n2)",
                None,
            ),
            rule(
                1,
                json!([1, null, null]),
                "Coordinate(B)",
                "I(1,n1,n2)",
                None,
            ),
            rule(2, json!([1, 1, 0]), "Coordinate(P)", "I(1,1,0)", None),
        ],
    );
    let c = certs(&d, &["Coordinate(A)", "Coordinate(B)"]);
    assert_eq!(
        cert_case(&c.join("cert-1.json")).as_deref(),
        Some("Coordinate(B)")
    );
    let (s, ok, out) = run(&d, &t, &c);
    assert!(ok, "{s:#}");
    assert_eq!(s["classes"]["whole domain"], 3, "{s:#}");
    let lines: Vec<String> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 5, "header, 3 rules, end");
    let der = |l: &str| -> Value {
        serde_json::from_str(&strip_derivation(l).unwrap().1.unwrap()).unwrap()
    };
    assert_eq!(der(&lines[1])["kind"], "lifted");
    assert!(der(&lines[1])["certificate"]["sha256"].is_string());
    let p = der(&lines[3]);
    assert_eq!(
        (p["kind"].as_str(), p["check"]["mode"].as_str()),
        (Some("point"), Some("instance-span")),
        "{p:#}"
    );
    let (tc, ok) = rustred_campaign::emission::tablecheck::table_check(
        &rustred_campaign::emission::tablecheck::Args {
            table: out.clone(),
            every: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(ok, "{tc:#}");
    assert_eq!(tc["classes"]["whole domain"], 3, "{tc:#}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn the_gate_fails_closed_without_a_certificate_or_with_a_point_not_in_span() {
    let d = tmp("refuse");
    let t = write_table(
        &d,
        &[
            rule(
                0,
                json!([null, null, null]),
                "Coordinate(A)",
                "I(n0,n1,n2)",
                None,
            ),
            rule(
                1,
                json!([1, null, null]),
                "Coordinate(B)",
                "I(1,n1,n2)",
                None,
            ),
        ],
    );
    // no certificate of case B: refused, NOTHING written
    let c = certs(&d, &["Coordinate(A)"]);
    let (s, ok, out) = run(&d, &t, &c);
    assert!(!ok);
    assert_eq!(s["refused"], 1, "{s:#}");
    assert!(
        s["refusals"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("no_certificate"),
        "{s:#}"
    );
    assert!(!out.exists(), "nothing written");
    // a point record whose instance-span check is not exact: refused
    let d2 = tmp("refuse2");
    let t2 = write_table(
        &d2,
        &[rule(
            0,
            json!([1, 1, 0]),
            "Coordinate(P)",
            "I(1,1,0)",
            Some(("fake_span", "NOT_IN_SPAN")),
        )],
    );
    let c2 = certs(&d2, &[]);
    let (s, ok, out) = run(&d2, &t2, &c2);
    assert!(!ok, "{s:#}");
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

/// THE IN-FLOW GATE (closeloop relation / symmetry tables): the emitted copy is written ONCE beside the flow's table; a
/// later pass reuses it without running the checker; a verifying restart (no write) refuses a missing or different copy;
/// a refusal writes nothing.
#[test]
fn the_in_flow_gate_writes_once_reuses_and_refuses_on_a_verifying_restart() {
    use rustred_campaign::emission::gate::gate_beside;
    let d = tmp("beside");
    let line = |i: usize, k: Value, tg: &str, extra: Option<(&str, &str)>| {
        rule(i, k, "point", tg, extra).to_string()
    };
    let plain = format!(
        "{}\n{}\n{}\n",
        json!({"type":"header","schema":"table_dump.v1","family":"TOY"}),
        line(0, json!([1, 1, 0]), "I(1,1,0)", None),
        line(1, json!([2, 1, 0]), "I(2,1,0)", None)
    );
    let es: rustred_campaign_core::emission::Settings = serde_json::from_value(
        json!({"checker": FAKE, "recover_rows": false, "descent_node": FAKE}),
    )
    .unwrap();
    let em = d.join("sym.emitted.table.jsonl");
    let s = gate_beside(&es, &plain, &em, GateKind::Symmetry, &[], true).unwrap();
    assert_eq!(s["classes"]["whole domain"], 2, "{s:#}");
    let first = std::fs::read(&em).unwrap();
    // a second pass: reused, not re-gated (bytes unchanged even though the fake checker's output would be the same)
    let s = gate_beside(&es, &plain, &em, GateKind::Symmetry, &[], true).unwrap();
    assert_eq!(s["reused"], true, "{s:#}");
    assert_eq!(std::fs::read(&em).unwrap(), first);
    // a verifying restart with ANOTHER table: refused, nothing rewritten
    let other = plain
        .replace("I(2,1,0)", "I(3,1,0)")
        .replace("[2,1,0]", "[3,1,0]");
    assert!(gate_beside(&es, &other, &em, GateKind::Symmetry, &[], false).is_err());
    assert_eq!(std::fs::read(&em).unwrap(), first);
    // a record not in span: refused, nothing written
    let bad = format!(
        "{}\n{}\n",
        json!({"type":"header","schema":"table_dump.v1","family":"TOY"}),
        line(
            0,
            json!([1, 1, 0]),
            "I(1,1,0)",
            Some(("fake_span", "NOT_IN_SPAN"))
        )
    );
    let em2 = d.join("bad.emitted.table.jsonl");
    assert!(gate_beside(&es, &bad, &em2, GateKind::Point, &[], true).is_err());
    assert!(!em2.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// DESCENT (contract section 14): an exact record whose right-hand side is NOT LOWER than its target (the engine's descent
/// test; here the fake's `fake_descent` member) is REFUSED by the gate, and nothing is written; without an engine descent
/// binary every record is refused; table-check refuses an emitted record whose descent member was removed or whose stated
/// verdict differs from the stored output.
#[test]
fn descent_the_gate_refuses_an_ascending_record_and_table_check_needs_the_descent_member() {
    let d = tmp("descent");
    let c = certs(&d, &["Coordinate(A)"]);
    let t = write_table(
        &d,
        &[rule(
            0,
            json!([null, null, null]),
            "Coordinate(A)",
            "I(n0,n1,n2)",
            Some(("fake_descent", "NotLower")),
        )],
    );
    let (s, ok, out) = run(&d, &t, &c);
    assert!(!ok, "{s:#}");
    assert!(
        s["refusals"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("not_lower"),
        "{s:#}"
    );
    assert!(!out.exists(), "nothing written");
    // no engine descent binary: refused (descent_not_checked)
    let d0 = tmp("descent0");
    let t0 = write_table(
        &d0,
        &[rule(
            0,
            json!([null, null, null]),
            "Coordinate(A)",
            "I(n0,n1,n2)",
            None,
        )],
    );
    let c0 = certs(&d0, &["Coordinate(A)"]);
    let work = d0.join("work");
    let em_for = |side: &Path| -> Result<Emitter, String> {
        let mut ck = Checker::new(Path::new(FAKE))?;
        ck.recover_rows = false;
        Emitter::new(ck, Policy::default(), side, &work)
    };
    let (s, ok) = emit_table(
        &em_for,
        &t0,
        &d0.join("out.table.jsonl"),
        &CertIndex::from_dirs(&[c0]),
        GateKind::Auto,
    )
    .unwrap();
    assert!(!ok, "{s:#}");
    assert!(
        s["refusals"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("descent_not_checked"),
        "{s:#}"
    );
    // a descending record is emitted with its member; then tamper with the member and table-check refuses
    let d2 = tmp("descent2");
    let t2 = write_table(
        &d2,
        &[rule(
            0,
            json!([null, null, null]),
            "Coordinate(A)",
            "I(n0,n1,n2)",
            None,
        )],
    );
    let c2 = certs(&d2, &["Coordinate(A)"]);
    let (s, ok, out2) = run(&d2, &t2, &c2);
    assert!(ok, "{s:#}");
    let text = std::fs::read_to_string(&out2).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let der: Value = serde_json::from_str(&strip_derivation(lines[1]).unwrap().1.unwrap()).unwrap();
    assert_eq!(der["descent"]["verdict"], "LOWER", "{der:#}");
    let tc = |text: &str| {
        std::fs::write(&out2, text).unwrap();
        rustred_campaign::emission::tablecheck::table_check(
            &rustred_campaign::emission::tablecheck::Args {
                table: out2.clone(),
                every: 1,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let (r, ok) = tc(&text);
    assert!(ok, "{r:#}");
    for (what, from, to) in [
        ("removed", der["descent"].to_string(), "null".to_string()),
        (
            "restated",
            "\"verdict\":\"LOWER\"".to_string(),
            "\"verdict\":\"NOT_LOWER\"".to_string(),
        ),
    ] {
        assert!(text.contains(&from), "{what}: {from}");
        let (r, ok) = tc(&text.replacen(&from, &to, 1));
        assert!(!ok, "{what}: {r:#}");
        assert!(r.to_string().contains("descent"), "{what}: {r:#}");
    }
    for x in [d, d0, d2] {
        let _ = std::fs::remove_dir_all(&x);
    }
}

/// SYMMETRY RELATIONS (contract section 15, round 5 H0 GAP-1): a record naming its symmetry element is checked on the expanded
/// symmetry row alone (`--symrows --symrows-only --emit-certs`), its certificate bound to the record bytes and stored beside
/// the table; table-check re-binds output, certificate and request. Refused: no symmetry group given; a certificate on an
/// IBP row; a record whose symrows member was removed.
#[test]
fn symmetry_relations_are_proven_on_their_symmetry_row_and_bound_to_the_record() {
    use rustred_campaign::emission::gate::gate_beside;
    let d = tmp("symrel");
    let group = d.join("group.json");
    std::fs::write(
        &group,
        json!({"elements": [{"element": 1, "matrix": [1]}], "fake": true}).to_string(),
    )
    .unwrap();
    let rel = |extra: Option<(&str, &str)>| {
        let mut r = rule(0, json!([1, 2, 0]), "point", "I(1,2,0)", extra);
        r["rhs"] = json!([["I(2,1,0)", "1"]]);
        r["origin"] = json!({"kind": "symmetry_relation", "element": 1});
        format!(
            "{}\n{r}\n",
            json!({"type":"header","schema":"table_dump.v1","family":"TOY"})
        )
    };
    let settings = |g: Option<&Path>| -> rustred_campaign_core::emission::Settings {
        let mut s = json!({"checker": FAKE, "recover_rows": false, "descent_node": FAKE});
        if let Some(g) = g {
            s["symmetry_group"] = json!(g);
        }
        serde_json::from_value(s).unwrap()
    };
    let em = d.join("sym.emitted.table.jsonl");
    let s = gate_beside(
        &settings(Some(&group)),
        &rel(None),
        &em,
        GateKind::Symmetry,
        &[],
        true,
    )
    .unwrap();
    assert_eq!(s["classes"]["whole domain"], 1, "{s:#}");
    let text = std::fs::read_to_string(&em).unwrap();
    let line = text.lines().nth(1).unwrap();
    let der: Value = serde_json::from_str(&strip_derivation(line).unwrap().1.unwrap()).unwrap();
    assert_eq!(der["check"]["symrows"]["only"], true, "{der:#}");
    assert_eq!(der["check"]["symrows"]["element"], 1, "{der:#}");
    assert_eq!(der["certificate"]["form"], "span-cert", "{der:#}");
    // table-check: accepted as written; refused with the symrows member removed
    let tc = |t: &str| {
        std::fs::write(&em, t).unwrap();
        rustred_campaign::emission::tablecheck::table_check(
            &rustred_campaign::emission::tablecheck::Args {
                table: em.clone(),
                every: 1,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let (r, ok) = tc(&text);
    assert!(ok, "{r:#}");
    let cut = format!(",\"symrows\":{}", der["check"]["symrows"]);
    assert!(text.contains(&cut), "{cut}");
    let (r, ok) = tc(&text.replacen(&cut, "", 1));
    assert!(!ok && r.to_string().contains("symrows"), "{r:#}");
    // no symmetry group: refused, nothing written
    let em2 = d.join("nogroup.emitted.table.jsonl");
    let e = gate_beside(
        &settings(None),
        &rel(None),
        &em2,
        GateKind::Symmetry,
        &[],
        true,
    )
    .unwrap_err();
    assert!(e.contains("symmetry group"), "{e}");
    assert!(!em2.exists());
    // a certificate on an IBP row: refused (symrows_binding)
    let em3 = d.join("ibp.emitted.table.jsonl");
    let e = gate_beside(
        &settings(Some(&group)),
        &rel(Some(("fake_symcert", "ibp"))),
        &em3,
        GateKind::Symmetry,
        &[],
        true,
    )
    .unwrap_err();
    assert!(e.contains("symrows_binding"), "{e}");
    assert!(!em3.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// descent-stamp (contract section 14): a record emitted BEFORE section 14 (its descent member removed here) is refused by
/// table-check; stamped by the engine's test (the fake) it is accepted again with the body unchanged; a record that ascends
/// is refused by the stamp and nothing is written.
#[test]
fn descent_stamp_adds_the_member_to_records_emitted_before_section_14() {
    let d = tmp("stamp");
    let c = certs(&d, &["Coordinate(A)"]);
    let t = write_table(
        &d,
        &[rule(
            0,
            json!([null, null, null]),
            "Coordinate(A)",
            "I(n0,n1,n2)",
            None,
        )],
    );
    let (s, ok, out) = run(&d, &t, &c);
    assert!(ok, "{s:#}");
    let text = std::fs::read_to_string(&out).unwrap();
    let line = text.lines().nth(1).unwrap();
    let der: Value = serde_json::from_str(&strip_derivation(line).unwrap().1.unwrap()).unwrap();
    let old = text.replacen(
        &format!(",\"descent\":{}", der["descent"]),
        ",\"descent\":null",
        1,
    );
    assert_ne!(old, text);
    let pre = d.join("pre14.table.jsonl");
    std::fs::write(&pre, &old).unwrap();
    rustred_campaign::emission::carry_side(
        &rustred_campaign_core::emission::side_dir(&out),
        &rustred_campaign_core::emission::side_dir(&pre),
    )
    .unwrap();
    let tc = |p: &Path| {
        rustred_campaign::emission::tablecheck::table_check(
            &rustred_campaign::emission::tablecheck::Args {
                table: p.to_path_buf(),
                every: 1,
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert!(!tc(&pre).1, "a pre-section-14 record is refused");
    let stamped = d.join("stamped.table.jsonl");
    let (s, ok) = rustred_campaign::emission::stamp::descent_stamp(
        &pre,
        &stamped,
        Path::new(FAKE),
        &d.join("sw"),
    )
    .unwrap();
    assert!(ok, "{s:#}");
    assert_eq!(s["stamped"], 1);
    assert!(tc(&stamped).1);
    let a = strip_derivation(
        std::fs::read_to_string(&stamped)
            .unwrap()
            .lines()
            .nth(1)
            .unwrap(),
    )
    .unwrap()
    .0;
    assert_eq!(
        a,
        strip_derivation(line).unwrap().0,
        "the body is unchanged"
    );
    // an ascending record: refused, nothing written
    let up = old.replacen(
        "\"dispatch\"",
        "\"fake_descent\":\"NotLower\",\"dispatch\"",
        1,
    );
    let pre2 = d.join("pre14up.table.jsonl");
    std::fs::write(&pre2, &up).unwrap();
    let out2 = d.join("stamped-up.table.jsonl");
    let (s, ok) = rustred_campaign::emission::stamp::descent_stamp(
        &pre2,
        &out2,
        Path::new(FAKE),
        &d.join("sw2"),
    )
    .unwrap();
    assert!(
        !ok && s["refusals"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("not_lower"),
        "{s:#}"
    );
    assert!(!out2.exists());
    let _ = std::fs::remove_dir_all(&d);
}
