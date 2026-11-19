//! Lane gencheck round 5 H0 GAP-4 (emission contract section 14): the DESCENT member. PERMANENT NEGATIVE TEST: lane t266's
//! candidate (i) K2 (an exact, d = 4 regular symmetry relation of T266 that ASCENDS in the node's order: rr_node descent
//! NotLower; the partner's rule emits it back, a cycle) must be REFUSED for "not lower" even when its exactness check passes
//! (here a stand-in instance-span checker says EXACT, so descent alone decides). POSITIVE: a one-dot terminal image of T266
//! (I(1,2,1^12; 0^6) -> I(2,1^13; 0^6), the form of the lane's (c) relations) descends and is accepted.
//!
//! Real data (read only): RUSTRED_T266_CAND_I = the frozen candidate directory
//! (implementer-scratch/t266study/candidates/T266-cand-i-symrel-20261119T1401Z), RUSTRED_DESCENT_NODE = an rr_node with the
//! `descent` verb (the engine's own test; Symbolica-linked: run the test binary under ONE licence slot). SKIPS without them.
use rustred_campaign::emission::emitter::{Candidate, Checker, Emitter};
use rustred_campaign_core::emission::class::{Kind, Liveness, Origin, Policy};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FAKE: &str = env!("CARGO_BIN_EXE_rr_fake_child");

fn setup() -> Option<(PathBuf, PathBuf)> {
    match (std::env::var("RUSTRED_T266_CAND_I"), std::env::var("RUSTRED_DESCENT_NODE")) {
        (Ok(d), Ok(n)) => Some((PathBuf::from(d), PathBuf::from(n))),
        _ => {
            eprintln!("SKIP: RUSTRED_T266_CAND_I / RUSTRED_DESCENT_NODE not set");
            None
        }
    }
}

fn emitter(node: &Path, tag: &str) -> (Emitter, PathBuf) {
    let base = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(base).join(format!("descent-t266-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // the stand-in exactness checker (EXACT at the key): only the descent test can refuse
    let mut ck = Checker::new(Path::new(FAKE)).unwrap();
    ck.recover_rows = false;
    ck.descent_node = Some((node.to_path_buf(), rustred_campaign::emission::child::file_sha256(node).unwrap()));
    let em = Emitter::new(ck, Policy::default(), &dir.join("t.emission"), &dir.join("work")).unwrap();
    (em, dir)
}

fn header(cand: &Path) -> String {
    std::fs::read_to_string(cand.join("proofs/K2.table.jsonl")).unwrap().lines().next().unwrap().to_string()
}

fn emit(em: &Emitter, header: String, line: String) -> rustred_campaign::emission::emitter::Emission {
    let c = Candidate {
        header,
        line,
        kind: Kind::Symmetry,
        origin: Origin::Lift,
        cert: None,
        provenance: None,
        lift_seconds: 0.0,
        ladder: None,
        cone: None,
        sample: None,
        tag: "t".into(),
    };
    em.emit(&c, Liveness::Unknown, false).unwrap()
}

#[test]
fn candidate_i_k2_exact_and_regular_but_not_lower_is_refused() {
    let Some((cand, node)) = setup() else { return };
    let (em, dir) = emitter(&node, "k2");
    let mut v: Value = serde_json::from_str(std::fs::read_to_string(cand.join("relations/K2.rule.json")).unwrap().trim()).unwrap();
    v["index"] = json!(0);
    let e = emit(&em, header(&cand), v.to_string());
    eprintln!("K2: refused {:?}; derivation descent {}", e.refused, e.derivation["descent"]);
    assert!(e.line.is_none(), "an exact relation that ascends must not be emitted");
    let why = e.refused.unwrap_or_default();
    assert!(why.contains("not_lower"), "refused for NOT LOWER, not for another reason: {why}");
    assert_eq!(e.derivation["descent"]["verdict"], "NOT_LOWER");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_one_dot_terminal_image_descends_and_is_accepted() {
    let Some((cand, node)) = setup() else { return };
    let (em, dir) = emitter(&node, "img");
    let mut key = vec![1i64; 14];
    key[1] = 2;
    key.extend([0i64; 6]);
    let mut img = vec![1i64; 14];
    img[0] = 2;
    img.extend([0i64; 6]);
    let t = |k: &[i64]| format!("I({})", k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
    let case = format!(
        "Coordinate(CoordinateCase {{ fixed: [{}] }})",
        key.iter().map(|x| format!("Some({x})")).collect::<Vec<_>>().join(", ")
    );
    let line = json!({"type": "rule", "chart": "11111111111111000000", "index": 0, "case": case, "fixed": key,
        "equations": [], "target": t(&key), "rhs": [[t(&img), "1"]], "exceptions": []});
    let e = emit(&em, header(&cand), line.to_string());
    eprintln!("image: refused {:?}; derivation descent {}", e.refused, e.derivation["descent"]);
    assert!(e.line.is_some(), "a descending point relation, exact at its key, is emitted: {:?}", e.refused);
    assert_eq!(e.derivation["descent"]["verdict"], "LOWER");
    let _ = std::fs::remove_dir_all(&dir);
}
