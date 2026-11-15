//! Independent audit of lane zeros2 (audit-r2-zeros, 2026-11-15): the standalone VERIFIER `rr_coef_facts
//! verify-zero-proofs` (no test of the lane runs it) against the honest `prove` output and against FORGED proof files.
//!
//! `ok_*` tests pass on int/7 d60f3839: the honest file verifies (exit 0) and every forgery listed is refused (exit 3 =
//! a mismatch, exit 2 = an unreadable file). They also detect three plausible verifier regressions (mutation check of
//! the audit, notes WS/notes/implementer.audit-r2-zeros.md): the den-text comparison dropped, the independent
//! re-check's exponent applied once, a missing proved line accepted.
//! `f3v_*` is `#[ignore = "AUDIT-R2-ZEROS ..."]`: the verifier keeps only the LAST line per (record, term, locus), so a
//! file carrying a forged proved line followed by the honest line VERIFIES (the core loader then accepts the forged line:
//! tests/audit_r2_zeros_forgery.rs f3 in crate rustred-campaign).
//! Needs the Symbolica licence (`source WS/ops/env.sh`); SKIPS without it. Toy table only (arity 3, mask 110).

use rustred_campaign_algebra::SymbolicaAlgebra;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

const HEADER: &str = r#"{"type":"header","schema":"table_dump.v1","family":"TOY","denominators":3,"mask":"110","guard_version":2}"#;

fn licensed() -> bool {
    match SymbolicaAlgebra::new() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("SKIP: {e}");
            false
        }
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("audit-r2-zeros-ver-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One parametric record (canonical target): t1 proved on n2 = 0 (nowhere zero), t2 NOT proved there, t3 proved
/// where_regular (denominator n0 - 3), t4 proved on n2 = -1 and n2 = 0 (the exponent of n2^2 matters on n2 = -1), t5
/// proved where_regular: its denominator n0 - 1 VANISHES on the chart bound n0 = 1 (never sign-definite).
fn table(dir: &Path) -> PathBuf {
    // written by hand with `type` FIRST: rr_coef_facts finds rule records by `"rule"` in a line's first 256 bytes (a
    // serde_json object would sort `type` after the long `rhs` and the record would be skipped)
    let rec = r#"{"type":"rule","chart":"110","index":3,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","rhs":[["I(n0-1,n1,n2)","(d-4)/(d-3)"],["I(n0,n1,n2+1)","(n2*(d-2*n0))/(d-1)"],["I(n0,n1-1,n2+1)","(n2+n0-3)"],["I(n0-1,n1,n2+1)","(n2*(n0-3))/(n0-3)"],["I(n0,n1,n2+2)","((n2^2+n2)*n1)/(d+n0)"],["I(n0-1,n1-1,n2+1)","(n2*(n0-1))/(n0-1)"]],"exceptions":[],"dispatch":"Partition","witness_loci":[]}"#;
    let p = dir.join("A.table.jsonl");
    std::fs::write(&p, format!("{HEADER}\n{rec}\n")).unwrap();
    p
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rr_coef_facts")
}

fn prove(dir: &Path, t: &Path) -> Vec<Value> {
    let out = dir.join("honest.zero-proofs.jsonl");
    let st = Command::new(bin())
        .args(["prove", "--tables"])
        .arg(t)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stdout)
    );
    std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// (exit code, summary) of verify-zero-proofs on `lines`.
fn verify(dir: &Path, t: &Path, name: &str, lines: &[Value]) -> (i32, Value) {
    let p = dir.join(name);
    let mut s = String::new();
    for l in lines {
        s.push_str(&l.to_string());
        s.push('\n');
    }
    std::fs::write(&p, s).unwrap();
    verify_file(dir, t, &p)
}

fn verify_file(dir: &Path, t: &Path, p: &Path) -> (i32, Value) {
    let sum = dir.join(format!(
        "{}.summary.json",
        p.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_file(&sum);
    let st = Command::new(bin())
        .args(["verify-zero-proofs", "--tables"])
        .arg(t)
        .arg("--proofs")
        .arg(p)
        .arg("--summary")
        .arg(&sum)
        .output()
        .unwrap();
    let v = std::fs::read_to_string(&sum)
        .ok()
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null);
    (st.status.code().unwrap_or(-1), v)
}

fn is_line(l: &Value, term: u64, locus: Value) -> bool {
    l["term"] == term && l["locus"] == locus
}

fn plane(p: u32, v: i64) -> Value {
    json!({"plane": [p, v]})
}

#[test]
fn ok_the_honest_prove_output_verifies_with_independent_rechecks() {
    if !licensed() {
        return;
    }
    let d = tmp("honest");
    let t = table(&d);
    let lines = prove(&d, &t);
    assert!(lines[0]["schema"] == "rustred.campaign.zero-proof-header.v1");
    let find = |term: u64, locus: Value| {
        lines
            .iter()
            .find(|l| is_line(l, term, locus.clone()))
            .cloned()
    };
    assert_eq!(find(1, plane(2, 0)).unwrap()["denominator"], "nowhere_zero");
    assert_eq!(find(2, plane(2, 0)).unwrap()["verdict"], "not_proved");
    let wr = find(3, plane(2, 0)).unwrap();
    assert_eq!(
        (wr["denominator"].as_str(), wr["den"].as_str()),
        (Some("where_regular"), Some("(n0-3)"))
    );
    assert_eq!(find(4, plane(2, -1)).unwrap()["verdict"], "zero_on_locus");
    // a d-free linear denominator that vanishes ON the chart bound (n0 = 1) is not sign-definite: where_regular
    let t5 = find(5, plane(2, 0)).unwrap();
    assert_eq!(
        (t5["denominator"].as_str(), t5["den"].as_str()),
        (Some("where_regular"), Some("(n0-1)")),
        "{t5}"
    );
    let (code, s) = verify(&d, &t, "honest.jsonl", &lines);
    let _ = std::fs::remove_dir_all(&d);
    assert_eq!(code, 0, "{s:#}");
    assert_eq!(s["counts"]["mismatches"], 0);
    assert!(
        s["counts"]["independent_numerator"].as_u64().unwrap() >= 4,
        "{s:#}"
    );
}

#[test]
fn ok_every_listed_forgery_is_refused_by_the_verifier() {
    if !licensed() {
        return;
    }
    let d = tmp("forge");
    let t = table(&d);
    let honest = prove(&d, &t);
    let edit = |f: &dyn Fn(&mut Vec<Value>)| -> Vec<Value> {
        let mut v = honest.clone();
        f(&mut v);
        v
    };
    let pos = |v: &Vec<Value>, term: u64, locus: Value| {
        v.iter()
            .position(|l| is_line(l, term, locus.clone()))
            .unwrap()
    };
    let mut refused: Vec<(&str, i32)> = vec![];
    // (a) a not-proved locus claimed proved
    let a = edit(&|v| {
        let i = pos(v, 2, plane(2, 0));
        v[i]["verdict"] = json!("zero_on_locus");
        v[i]["denominator"] = json!("nowhere_zero");
    });
    refused.push(("not proved -> proved", verify(&d, &t, "a.jsonl", &a).0));
    // (b) a where_regular line with another den text (mutation V1: the den-text comparison dropped)
    let b = edit(&|v| {
        let i = pos(v, 3, plane(2, 0));
        v[i]["den"] = json!("1");
    });
    refused.push(("den text forged", verify(&d, &t, "b.jsonl", &b).0));
    // (c) a proved line removed (mutation V3: a missing proved line accepted)
    let c = edit(&|v| {
        let i = pos(v, 1, plane(2, 0));
        v.remove(i);
    });
    refused.push(("proved line removed", verify(&d, &t, "c.jsonl", &c).0));
    // (d) term index swapped: t1's proof presented for t2 (replacing t2's not-proved line)
    let dd = edit(&|v| {
        let i = pos(v, 1, plane(2, 0));
        let j = pos(v, 2, plane(2, 0));
        let mut l = v[i].clone();
        l["term"] = json!(2);
        v[j] = l;
    });
    refused.push((
        "proof of term 1 presented for term 2",
        verify(&d, &t, "d.jsonl", &dd).0,
    ));
    // (e) a claim re-digested for a record the tables do not hold
    let e = edit(&|v| {
        let i = pos(v, 1, plane(2, 0));
        v[i]["record_sha256"] = json!("ab".repeat(32));
    });
    refused.push(("foreign record sha", verify(&d, &t, "e.jsonl", &e).0));
    // (f) a denominator class changed
    let f = edit(&|v| {
        let i = pos(v, 1, plane(2, 0));
        v[i]["denominator"] = json!("where_regular");
        v[i]["den"] = json!("(d-1)");
    });
    refused.push(("denominator class changed", verify(&d, &t, "f.jsonl", &f).0));
    // (g) a header whose table sha256 is not the table's
    let g = edit(&|v| {
        v[0]["tables"][0]["sha256"] = json!("00".repeat(32));
    });
    refused.push(("header sha256 forged", verify(&d, &t, "g.jsonl", &g).0));
    // (h) a file truncated inside a line
    let ph = d.join("h.jsonl");
    let s: String = honest.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&ph, &s[..s.len() - 10]).unwrap();
    refused.push(("truncated inside a line", verify_file(&d, &t, &ph).0));
    // (i) a plane claim outside the enumeration that does not hold (t4 on n2 = -2: n2^2 + n2 = 2)
    let i = edit(&|v| {
        let k = pos(v, 4, plane(2, -1));
        let mut l = v[k].clone();
        l["locus"] = plane(2, -2);
        v.push(l);
    });
    refused.push((
        "claim on a plane where the numerator is 2",
        verify(&d, &t, "i.jsonl", &i).0,
    ));
    // (j) the honest file against an EDITED table (one coefficient changed: the record's sha256 changes)
    let t2 = d.join("A2.table.jsonl");
    std::fs::write(
        &t2,
        std::fs::read_to_string(&t)
            .unwrap()
            .replace("(n2+n0-3)", "(n2+n0-4)"),
    )
    .unwrap();
    let ph2 = d.join("j.jsonl");
    std::fs::write(&ph2, &s).unwrap();
    refused.push(("table edited after prove", verify_file(&d, &t2, &ph2).0));
    let _ = std::fs::remove_dir_all(&d);
    let accepted: Vec<&(&str, i32)> = refused.iter().filter(|(_, c)| *c == 0).collect();
    assert!(
        accepted.is_empty(),
        "forgeries VERIFIED (exit 0): {accepted:?}; all: {refused:?}"
    );
    assert!(
        refused.iter().all(|(_, c)| *c == 3 || *c == 2),
        "{refused:?}"
    );
}

/// F3v: a forged proved line for t2 (truly non-zero on n2 = 0) placed BEFORE the honest not-proved line of the same
/// locus: the verifier's claim map keeps the last line, re-derives `not_proved`, and VERIFIES the file.
#[test]
#[ignore = "AUDIT-R2-ZEROS F3: verify-zero-proofs keeps the last line per locus: a forged earlier line verifies"]
fn f3v_verifier_refuses_two_lines_for_one_locus() {
    if !licensed() {
        return;
    }
    let d = tmp("dup");
    let t = table(&d);
    let mut v = prove(&d, &t);
    let j = v.iter().position(|l| is_line(l, 2, plane(2, 0))).unwrap();
    let mut forged = v[j].clone();
    forged["verdict"] = json!("zero_on_locus");
    forged["denominator"] = json!("nowhere_zero");
    forged.as_object_mut().unwrap().remove("reason");
    v.insert(j, forged);
    let (code, s) = verify(&d, &t, "dup.jsonl", &v);
    let _ = std::fs::remove_dir_all(&d);
    assert_ne!(
        code, 0,
        "a file with a forged proved line VERIFIES: {:#}",
        s["counts"]
    );
}
