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
    let mut s = format!("{}\n", json!({"type":"header","schema":"table_dump.v1","family":"TOY"}));
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
        std::fs::write(d.join(format!("cert-{k}.json")), json!({"case": c, "fake_checker": {}}).to_string()).unwrap();
    }
    d
}

fn run(dir: &Path, table: &Path, certs_dir: &Path) -> (Value, bool, PathBuf) {
    let out = dir.join("out.table.jsonl");
    let work = dir.join("work");
    let em_for = |side: &Path| -> Result<Emitter, String> {
        let mut ck = Checker::new(Path::new(FAKE))?;
        ck.recover_rows = false;
        Emitter::new(ck, Policy::default(), side, &work)
    };
    let (s, ok) = emit_table(&em_for, table, &out, &CertIndex::from_dirs(&[certs_dir.to_path_buf()]), GateKind::Auto).unwrap();
    (s, ok, out)
}

#[test]
fn the_gate_emits_lifted_and_point_records_and_table_check_accepts() {
    let d = tmp("ok");
    let t = write_table(
        &d,
        &[
            rule(0, json!([null, null, null]), "Coordinate(A)", "I(n0,n1,n2)", None),
            rule(1, json!([1, null, null]), "Coordinate(B)", "I(1,n1,n2)", None),
            rule(2, json!([1, 1, 0]), "Coordinate(P)", "I(1,1,0)", None),
        ],
    );
    let c = certs(&d, &["Coordinate(A)", "Coordinate(B)"]);
    assert_eq!(cert_case(&c.join("cert-1.json")).as_deref(), Some("Coordinate(B)"));
    let (s, ok, out) = run(&d, &t, &c);
    assert!(ok, "{s:#}");
    assert_eq!(s["classes"]["whole domain"], 3, "{s:#}");
    let lines: Vec<String> = std::fs::read_to_string(&out).unwrap().lines().map(str::to_string).collect();
    assert_eq!(lines.len(), 5, "header, 3 rules, end");
    let der = |l: &str| -> Value { serde_json::from_str(&strip_derivation(l).unwrap().1.unwrap()).unwrap() };
    assert_eq!(der(&lines[1])["kind"], "lifted");
    assert!(der(&lines[1])["certificate"]["sha256"].is_string());
    let p = der(&lines[3]);
    assert_eq!((p["kind"].as_str(), p["check"]["mode"].as_str()), (Some("point"), Some("instance-span")), "{p:#}");
    let (tc, ok) = rustred_campaign::emission::tablecheck::table_check(&rustred_campaign::emission::tablecheck::Args {
        table: out.clone(),
        every: 1,
        ..Default::default()
    })
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
            rule(0, json!([null, null, null]), "Coordinate(A)", "I(n0,n1,n2)", None),
            rule(1, json!([1, null, null]), "Coordinate(B)", "I(1,n1,n2)", None),
        ],
    );
    // no certificate of case B: refused, NOTHING written
    let c = certs(&d, &["Coordinate(A)"]);
    let (s, ok, out) = run(&d, &t, &c);
    assert!(!ok);
    assert_eq!(s["refused"], 1, "{s:#}");
    assert!(s["refusals"][0]["reason"].as_str().unwrap().contains("no_certificate"), "{s:#}");
    assert!(!out.exists(), "nothing written");
    // a point record whose instance-span check is not exact: refused
    let d2 = tmp("refuse2");
    let t2 = write_table(&d2, &[rule(0, json!([1, 1, 0]), "Coordinate(P)", "I(1,1,0)", Some(("fake_span", "NOT_IN_SPAN")))]);
    let c2 = certs(&d2, &[]);
    let (s, ok, out) = run(&d2, &t2, &c2);
    assert!(!ok, "{s:#}");
    assert!(!out.exists());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}
