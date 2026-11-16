//! Lane zerosfix3 (audit-r2-zeros M2, F3v): `rr_coef_facts prove` / `verify-zero-proofs` find a rule record whatever its
//! key order (`"type":"rule"` far beyond the first 256 bytes of the line), and the verifier refuses a file with two
//! lines for one (record, term, locus), even identical ones. Needs the Symbolica licence (`source WS/ops/env.sh`);
//! SKIPS without it. Toy table only (arity 3, mask 110).

use rustred_campaign_algebra::SymbolicaAlgebra;
use serde_json::Value;
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
    let d = std::env::temp_dir().join(format!("zerosfix3-ver-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The record with `rhs` FIRST (its text pushes `"type":"rule"` beyond byte 256): t1 "n2" is zero on n2 = 0.
fn table(dir: &Path) -> PathBuf {
    let pad = format!("(d-4)/(d-3+{})", "0*n0+".repeat(60) + "0");
    let rec = format!(
        r#"{{"rhs":[["I(n0-1,n1,n2)","{pad}"],["I(n0,n1,n2+1)","n2"],["I(n0,n1-1,n2+1)","(n2+n0-3)"]],"chart":"110","index":3,"fixed":[null,null,null],"equations":[],"target":"I(n0,n1,n2)","exceptions":[],"dispatch":"Partition","witness_loci":[],"type":"rule"}}"#
    );
    assert!(rec.find("\"type\"").unwrap() > 256);
    let p = dir.join("A.table.jsonl");
    std::fs::write(&p, format!("{HEADER}\n{rec}\n")).unwrap();
    p
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rr_coef_facts")
}

fn run(args: &[&str], extra: &[&Path]) -> i32 {
    let mut c = Command::new(bin());
    c.args(args);
    for e in extra {
        c.arg(e);
    }
    c.output().unwrap().status.code().unwrap_or(-1)
}

#[test]
fn a_late_rule_key_is_found_and_duplicate_lines_never_verify() {
    if !licensed() {
        return;
    }
    let d = tmp("late");
    let t = table(&d);
    let out = d.join("p.jsonl");
    let st = Command::new(bin())
        .args(["prove", "--tables"])
        .arg(&t)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "{}",
        String::from_utf8_lossy(&st.stdout)
    );
    let lines: Vec<Value> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let proved = lines
        .iter()
        .filter(|l| l["verdict"] == "zero_on_locus" && l["term"] == 1)
        .count();
    assert!(
        proved >= 1,
        "the record with a late \"rule\" key is proved: {lines:?}"
    );
    let sum = d.join("s.json");
    assert_eq!(
        run(
            &["verify-zero-proofs", "--tables"],
            &[
                &t,
                Path::new("--proofs"),
                &out,
                Path::new("--summary"),
                &sum
            ]
        ),
        0,
        "the honest file verifies"
    );
    // the same proved line twice: refused
    let body = std::fs::read_to_string(&out).unwrap();
    let dup_line = body
        .lines()
        .find(|l| l.contains("\"zero_on_locus\""))
        .unwrap()
        .to_string();
    let dup = d.join("dup.jsonl");
    std::fs::write(&dup, format!("{body}{dup_line}\n")).unwrap();
    assert_eq!(
        run(
            &["verify-zero-proofs", "--tables"],
            &[
                &t,
                Path::new("--proofs"),
                &dup,
                Path::new("--summary"),
                &sum
            ]
        ),
        3
    );
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&sum).unwrap()).unwrap();
    assert_eq!(s["duplicate_lines"], 1, "{s:#}");
    let _ = std::fs::remove_dir_all(&d);
}
