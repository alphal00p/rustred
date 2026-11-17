//! int8 (round 3 stage 2): the independent verifier `rr_pverify` must STAY independent of the producer's code. It re-derives
//! the engine's semantics with its own parser, first match, face recursion and arithmetic (lane pverify; only Symbolica is
//! shared, for polynomials), so no file of `src/bin/rr_pverify/` may reach the campaign crates (the producer's
//! `parametric`, `partition`, `loci`, ... live in `rustred_campaign_core`) or pull a source file from outside its own
//! directory with `#[path]` / `include!`. Pure source scan: no build of the verifier, no licence.

use std::path::{Path, PathBuf};

/// Forms that would make the verifier share code with the producer.
const FORBIDDEN: &[&str] = &[
    "rustred_campaign_core",
    "rustred_campaign_algebra",
    "rustred_campaign_contract",
    "rustred_campaign::",
    "#[path",
    "include!(",
];

fn verifier_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/bin/rr_pverify")
}

/// The forbidden forms on the code part of each line (text after `//` is a comment and ignored).
fn hits_in(rel: &str, text: &str) -> Vec<String> {
    let mut out = vec![];
    for (i, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        for pat in FORBIDDEN {
            if code.contains(pat) {
                out.push(format!("{rel}:{}: [{pat}] {}", i + 1, line.trim()));
            }
        }
    }
    out
}

fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

#[test]
fn rr_pverify_shares_no_code_with_the_producer() {
    let dir = verifier_dir();
    let files = rs_files(&dir);
    assert!(
        files.iter().any(|f| f.ends_with("main.rs")),
        "verifier sources not found under {}",
        dir.display()
    );
    let mut hits = vec![];
    for f in &files {
        let rel = f.strip_prefix(&dir).unwrap().display().to_string();
        hits.extend(hits_in(&rel, &std::fs::read_to_string(f).unwrap()));
    }
    assert!(
        hits.is_empty(),
        "rr_pverify must stay independent of the producer (no campaign crate, no #[path] / include! from outside): {hits:#?}"
    );
}

#[test]
fn the_tripwire_catches_planted_imports() {
    for planted in [
        "use rustred_campaign_core::parametric::descent;",
        "let p = rustred_campaign_core::partition::partition(&x);",
        "use rustred_campaign_core::loci::contain;",
        "use rustred_campaign_algebra::SymbolicaAlgebra;",
        "#[path = \"../../parametric/cone.rs\"] mod cone;",
        "include!(\"../../../rustred-campaign-core/src/loci/contain.rs\");",
    ] {
        assert!(!hits_in("planted.rs", planted).is_empty(), "{planted}");
    }
    // comments and the verifier's own modules are not forms
    assert!(hits_in("ok.rs", "use crate::cover; // unlike rustred_campaign_core::partition").is_empty());
}
