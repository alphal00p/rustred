//! WP-0b (lane int): the contract crate's structs round-trip the WP-0a examples of their formats byte for byte after
//! canonicalisation (schemas in `crates/rustred-campaign-core/schemas/`), and the contract's RECORD canonical form
//! (`canon::canonical_line`) is compared with the shared vectors `schemas/canon-vectors.json`: since lane contractfix
//! it IS the plan's 3.3.0 rule (plus removal of volatile keys), so `record_rule` must equal `document_rule` on every
//! vector.

use rustred_campaign_contract::events::EventLine;
use rustred_campaign_contract::fake::FakeScript;
use rustred_campaign_contract::outcome::Outcome;
use rustred_campaign_contract::resume::ResumeState;
use rustred_campaign_contract::spec::JobSpec;
use rustred_campaign_contract::{VersionInfo, canonical_line};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::PathBuf;

fn schemas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rustred-campaign-core/schemas")
}

/// Canonical text of a serde_json value: compact, keys sorted bytewise (serde_json's map is ordered by key; the
/// record canonicaliser of this crate gives the same bytes on serde-written input).
fn canon(v: &Value) -> String {
    String::from_utf8(canonical_line(&serde_json::to_vec(v).unwrap()).unwrap()).unwrap()
}

fn roundtrip<T: DeserializeOwned + Serialize>(text: &[u8], what: &str) {
    let original: Value = serde_json::from_slice(text).unwrap();
    let typed: T = serde_json::from_slice(text).unwrap_or_else(|e| panic!("{what}: {e}"));
    let back = serde_json::to_value(&typed).unwrap();
    assert_eq!(canon(&original), canon(&back), "{what} does not round-trip");
}

#[test]
fn contract_examples_roundtrip() {
    let e = schemas().join("examples");
    let r = |n: &str| std::fs::read(e.join(n)).unwrap();
    roundtrip::<JobSpec>(&r("child-job-spec.json"), "child-job-spec.json");
    roundtrip::<FakeScript>(&r("fake-script.json"), "fake-script.json");
    roundtrip::<Outcome>(&r("outcome.json"), "outcome.json");
    roundtrip::<ResumeState>(&r("resume.json"), "resume.json");
    roundtrip::<VersionInfo>(&r("version.json"), "version.json");
    let mut n = 0;
    for l in r("events-line.jsonl")
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        roundtrip::<EventLine>(l, "events-line.jsonl");
        n += 1;
    }
    JobSpec::from_json(&r("child-job-spec.json"))
        .unwrap()
        .validate()
        .unwrap();
    println!("contract examples round-tripped: 5 documents + {n} EVENTS lines");
}

#[test]
fn record_canonical_form_on_shared_vectors() {
    let v: Value =
        serde_json::from_slice(&std::fs::read(schemas().join("canon-vectors.json")).unwrap())
            .unwrap();
    let mut n = 0;
    for x in v["vectors"].as_array().unwrap() {
        let input = x["input"].as_str().unwrap();
        let got = String::from_utf8(canonical_line(input.as_bytes()).unwrap()).unwrap();
        assert_eq!(
            got,
            x["record_rule"].as_str().unwrap(),
            "vector {}",
            x["name"]
        );
        assert_eq!(
            got,
            x["document_rule"].as_str().unwrap(),
            "vector {}",
            x["name"]
        );
        assert!(x["agree"].as_bool().unwrap(), "vector {}", x["name"]);
        n += 1;
    }
    println!("record rule == document rule (3.3.0) on all {n} vectors");
}
