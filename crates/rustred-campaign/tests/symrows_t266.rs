//! Lane gencheck round 5 H0 GAP-1 (emission contract section 15): SYMMETRY RELATIONS are proven by the independent checker in
//! its symmetry-relation mode (`rr_proof --mode instance-span --symmetry-group G --symrows R --symrows-only --emit-certs C`),
//! the exact certificate bound to the record bytes, and table-check re-binds it. REAL checker, REAL descent test, T266 data:
//! - RED CONTROL (the old path): K2 without its element is checked on IBP rows and seeds -> the kernel refuses the seedless
//!   record (MISSING_SEEDS), as in lane t266's emitter attempt;
//! - K2 WITH its element: EXACT on its symmetry row alone, certificate bound; still REFUSED, for descent only (section 14);
//! - POSITIVE: a rank-1 symmetry relation of the form of lane t266's (b) X15 (element 1: I(1^14; -1,0^5) = I(1^13,0; 0^6)
//!   + I(1^14; 0^6)) is proven on its symmetry row, descends, is EMITTED, and table-check accepts it; edited after
//!   emission it is refused by table-check; with coefficient 2 it is refused by the emitter.
//!
//! Env (SKIPS without them): RUSTRED_T266_CAND_I (the frozen candidate directory; its proofs/symmetry_group.json is the
//! group), RUSTRED_SPAN_CHECKER (rr_proof with --symrows), RUSTRED_DESCENT_NODE (rr_node with `descent`). Symbolica-linked
//! children: run the test binary under ONE licence slot.
use rustred_campaign::emission::emitter::{Candidate, Checker, Emission, Emitter};
use rustred_campaign_core::emission::body::strip_derivation;
use rustred_campaign_core::emission::class::{Kind, Liveness, Origin, Policy};
use rustred_campaign_core::emission::side_dir;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

struct Env {
    cand: PathBuf,
    checker: PathBuf,
    node: PathBuf,
}

fn env() -> Option<Env> {
    let v = |k: &str| std::env::var(k).ok().map(PathBuf::from);
    match (v("RUSTRED_T266_CAND_I"), v("RUSTRED_SPAN_CHECKER"), v("RUSTRED_DESCENT_NODE")) {
        (Some(cand), Some(checker), Some(node)) => Some(Env { cand, checker, node }),
        _ => {
            eprintln!("SKIP: RUSTRED_T266_CAND_I / RUSTRED_SPAN_CHECKER / RUSTRED_DESCENT_NODE not set");
            None
        }
    }
}

fn tmp(tag: &str) -> PathBuf {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let d = PathBuf::from(base).join(format!("symrows-t266-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn emitter(e: &Env, dir: &Path) -> Emitter {
    emitter_with(e, dir, &e.node)
}

fn emitter_with(e: &Env, dir: &Path, descent_node: &Path) -> Emitter {
    let mut ck = Checker::new(&e.checker).unwrap();
    ck.recover_rows = false;
    ck.floor_s = 600.0;
    ck.symmetry_group = Some(e.cand.join("proofs/symmetry_group.json"));
    ck.descent_node = Some((descent_node.to_path_buf(), rustred_campaign::emission::child::file_sha256(descent_node).unwrap()));
    Emitter::new(ck, Policy::default(), &side_dir(&dir.join("t.table.jsonl")), &dir.join("work")).unwrap()
}

fn header(e: &Env) -> String {
    std::fs::read_to_string(e.cand.join("proofs/K2.table.jsonl")).unwrap().lines().next().unwrap().to_string()
}

fn emit(em: &Emitter, header: &str, line: &str, tag: &str) -> Emission {
    let c = Candidate {
        header: header.to_string(),
        line: line.to_string(),
        kind: Kind::Symmetry,
        origin: Origin::Lift,
        cert: None,
        provenance: None,
        lift_seconds: 0.0,
        ladder: None,
        cone: None,
        sample: None,
        tag: tag.into(),
    };
    em.emit(&c, Liveness::Unknown, false).unwrap()
}

fn k2(e: &Env) -> Value {
    serde_json::from_str(std::fs::read_to_string(e.cand.join("relations/K2.rule.json")).unwrap().trim()).unwrap()
}

#[test]
fn k2_old_path_red_then_proven_on_its_symmetry_row_and_refused_only_for_descent() {
    let Some(e) = env() else { return };
    let d = tmp("k2");
    let em = emitter(&e, &d);
    let h = header(&e);
    // RED CONTROL: the record without its element goes the old way (IBP rows + seeds): the kernel refuses it
    let mut old = k2(&e);
    old.as_object_mut().unwrap().remove("origin");
    let r = emit(&em, &h, &old.to_string(), "old");
    eprintln!("K2 old path: refused {:?}; check {}", r.refused, r.derivation["check"]);
    assert!(r.line.is_none());
    let old_out = std::fs::read_to_string(d.join("work/old/span.out")).unwrap_or_default();
    assert!(old_out.contains("MISSING_SEEDS"), "the old path fails on the seedless record: {old_out}");
    // GREEN: with its element, EXACT on the symmetry row alone; refused for descent only
    let r = emit(&em, &h, &k2(&e).to_string(), "k2");
    eprintln!("K2 symrows: refused {:?}; check {}; certificate {}", r.refused, r.derivation["check"], r.derivation["certificate"]);
    assert_eq!(r.derivation["check"]["verdict"], "EXACT_ALL_KEYS_IN_SPAN");
    assert_eq!(r.derivation["check"]["symrows"]["only"], true);
    assert_eq!(r.derivation["certificate"]["form"], "span-cert");
    assert!(r.line.is_none(), "K2 ascends");
    assert!(r.refused.unwrap_or_default().contains("not_lower"));
    // the stored certificate equals the lane's frozen one (same kernel, same record bytes)
    let lane = std::fs::read_to_string(e.cand.join("proofs/K2.certificate.json")).unwrap();
    let kv = k2(&e)["fixed"].as_array().unwrap().iter().map(|x| x.to_string()).collect::<Vec<_>>().join("_");
    let mine = std::fs::read_to_string(d.join(format!("work/k2/span-certs/cert_{kv}.json"))).unwrap();
    let norm = |s: &str| serde_json::from_str::<Value>(s.trim()).unwrap();
    assert_eq!(norm(&mine)["weights"], norm(&lane)["weights"]);
    assert_eq!(norm(&mine)["key"], norm(&lane)["key"]);
    // A WRONG DESCENT BINARY (the fake says LOWER for everything): K2 is emitted with a forged LOWER verdict of the right
    // shape (1 rule, 6 terms); the record-level table-check cannot tell (a passing output names no record), the RE-RUN of
    // the engine's descent test on the record's bytes refuses it
    let d2 = tmp("k2forged");
    let fake = Path::new(env!("CARGO_BIN_EXE_rr_fake_child"));
    let em2 = emitter_with(&e, &d2, fake);
    let r = emit(&em2, &h, &k2(&e).to_string(), "k2f");
    let forged = r.line.expect("the fake descent binary lets K2 through");
    let table = d2.join("t.table.jsonl");
    std::fs::write(&table, format!("{h}\n{forged}\n")).unwrap();
    let tc = |node: Option<PathBuf>| {
        rustred_campaign::emission::tablecheck::table_check(&rustred_campaign::emission::tablecheck::Args {
            table: table.clone(),
            every: 1,
            work: Some(d2.join("tc")),
            descent_recheck: node,
            ..Default::default()
        })
        .unwrap()
    };
    let (s, ok) = tc(None);
    assert!(ok, "record level: {s:#}");
    let (s, ok) = tc(Some(e.node.clone()));
    eprintln!("K2 forged LOWER, descent re-run: {s}");
    assert!(!ok && s.to_string().contains("descent re-run"), "{s:#}");
    assert_eq!(s["descent_rechecked"], 1);
    let _ = std::fs::remove_dir_all(&d2);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_rank1_symmetry_relation_is_proven_on_its_symmetry_row_descends_is_emitted_and_table_check_accepts() {
    let Some(e) = env() else { return };
    let d = tmp("rank1");
    let em = emitter(&e, &d);
    let h = header(&e);
    // the form of lane t266's (b) relation X15 (key slot 14, element 1, corner + one scalar pinch), built here:
    // I(1^14; -1,0^5) = I(1^13,0; 0^6) + I(1^14; 0^6)
    let t = |k: &[i64]| format!("I({})", k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
    let mut target = vec![1i64; 14];
    target.extend([-1, 0, 0, 0, 0, 0]);
    let mut pinch = vec![1i64; 13];
    pinch.extend([0i64; 7]);
    let mut corner = vec![1i64; 14];
    corner.extend([0i64; 6]);
    let line = |c: &str| {
        let case = format!(
            "Coordinate(CoordinateCase {{ fixed: [{}] }})",
            target.iter().map(|x| format!("Some({x})")).collect::<Vec<_>>().join(", ")
        );
        json!({"type": "rule", "chart": "11111111111111000000", "case": case, "fixed": target, "equations": [],
            "target": t(&target), "rhs": [[t(&pinch), c], [t(&corner), "1"]], "exceptions": [], "dispatch": "Partition",
            "index": 0, "origin": {"kind": "symmetry_relation", "lane": "gencheck GAP-1 test (X15 form)", "element": 1}})
        .to_string()
    };
    let r = emit(&em, &h, &line("1"), "x15");
    eprintln!("X15 form: refused {:?}; check {}; descent {}", r.refused, r.derivation["check"], r.derivation["descent"]);
    let out = r.line.clone().expect("a descending symmetry relation, exact on its row, is emitted");
    assert_eq!(r.derivation["check"]["verdict"], "EXACT_ALL_KEYS_IN_SPAN");
    assert_eq!(r.derivation["check"]["symrows"]["only"], true);
    assert_eq!(r.derivation["check"]["symrows"]["element"], 1);
    assert_eq!(r.derivation["certificate"]["form"], "span-cert");
    assert_eq!(r.derivation["descent"]["verdict"], "LOWER");
    // table-check: the emitted table (header + the record) is accepted; the certificate re-bound to the record bytes
    let table = d.join("t.table.jsonl");
    std::fs::write(&table, format!("{h}\n{out}\n")).unwrap();
    let check = |p: &Path| {
        rustred_campaign::emission::tablecheck::table_check(&rustred_campaign::emission::tablecheck::Args {
            table: p.to_path_buf(),
            every: 1,
            ..Default::default()
        })
        .unwrap()
    };
    let (tc, ok) = check(&table);
    eprintln!("table-check: {tc}");
    assert!(ok, "{tc:#}");
    // and with the descent re-run (the engine again on the record's bytes): accepted
    let (tc, ok) = rustred_campaign::emission::tablecheck::table_check(&rustred_campaign::emission::tablecheck::Args {
        table: table.clone(),
        every: 1,
        work: Some(d.join("tc")),
        descent_recheck: Some(e.node.clone()),
        ..Default::default()
    })
    .unwrap();
    assert!(ok && tc["descent_rechecked"] == 1, "{tc:#}");
    let (body, der) = strip_derivation(&out).unwrap();
    let der: Value = serde_json::from_str(&der.unwrap()).unwrap();
    let cert = std::fs::read(side_dir(&table).join(format!("cert/{}.json", der["certificate"]["sha256"].as_str().unwrap()))).unwrap();
    let c: Value = serde_json::from_slice(&cert).unwrap();
    assert_eq!(c["record_sha256"].as_str().unwrap(), rustred_campaign_core::emission::body::sha256_hex(body.as_bytes()));
    // the record's bytes changed after emission (coefficient 1 -> 2, derivation kept): table-check refuses
    let edited = out.replacen(&format!("[\"{}\",\"1\"]", t(&pinch)), &format!("[\"{}\",\"2\"]", t(&pinch)), 1);
    assert_ne!(edited, out);
    std::fs::write(&table, format!("{h}\n{edited}\n")).unwrap();
    let (tc, ok) = check(&table);
    assert!(!ok, "{tc:#}");
    // coefficient 2 emitted afresh: not in the span of the symmetry row -> refused
    let r = emit(&em, &h, &line("2"), "x15x2");
    eprintln!("X15 form x2: refused {:?}; check {}", r.refused, r.derivation["check"]);
    assert!(r.line.is_none());
    assert_ne!(r.derivation["check"]["verdict"], "EXACT_ALL_KEYS_IN_SPAN");
    let _ = std::fs::remove_dir_all(&d);
}
