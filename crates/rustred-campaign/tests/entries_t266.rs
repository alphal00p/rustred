//! Lane gencheck round 5 H0 GAP-3, real data: the entry digest of the writer (`canonical_json` of the body, sha256) is the
//! ENGINE's: it reproduces the `entry_digest` of a shipped T266 entry (the r2 relation segment, written by the engine's
//! own canonical form), so an entry the writer makes is not refused as corrupt. Env RUSTRED_T266_ENTRY = a file holding
//! one shipped entry line (SKIPS without it).
use rustred_campaign::emission::entries::entry_digest;
use serde_json::Value;

#[test]
fn the_writer_digest_is_the_engines_on_a_shipped_t266_entry() {
    let Ok(p) = std::env::var("RUSTRED_T266_ENTRY") else {
        eprintln!("SKIP: RUSTRED_T266_ENTRY not set");
        return;
    };
    let e: Value = serde_json::from_str(std::fs::read_to_string(p).unwrap().trim()).unwrap();
    let d = entry_digest(&e["body"]).unwrap();
    eprintln!(
        "stated {} computed {d} ({} rhs terms)",
        e["entry_digest"],
        e["body"]["rhs"].as_array().map_or(0, Vec::len)
    );
    assert_eq!(e["entry_digest"].as_str(), Some(d.as_str()));
}
