//! WP-0b: every example of the formats whose structs live in `rustred_campaign_core::schema` (and every progress file of
//! the fixture campaign directories) round-trips through its struct byte for byte after canonicalisation (plan 3.3.0).
//! Also runs the ONE validator, `schemas/validate.py`, on every example and fixture and its mutation tests
//! (`schemas/test_validate.py`): skipped with a printed reason only when no `python3` is on PATH.

mod inputs;
use rustred_campaign_core::schema::canon::{canonical_json, has_duplicate_keys};
use rustred_campaign_core::schema::{
    accept, control, item, job, progress, registry, spec, terminals,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn schemas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas")
}

/// Parse `path` into `T`, write it back, compare the canonical forms of both (floats allowed: progress files).
fn roundtrip<T: DeserializeOwned + Serialize>(path: &Path) {
    let text = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert!(
        !has_duplicate_keys(&text).unwrap(),
        "{}: duplicate keys",
        path.display()
    );
    let original: Value = serde_json::from_slice(&text).unwrap();
    let typed: T =
        serde_json::from_slice(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let back = serde_json::to_value(&typed).unwrap();
    let a = canonical_json(&original, true).unwrap();
    let b = canonical_json(&back, true).unwrap();
    assert_eq!(a, b, "{} does not round-trip", path.display());
}

#[test]
fn examples_roundtrip_through_core_structs() {
    let e = schemas().join("examples");
    roundtrip::<spec::CampaignSpec>(&e.join("campaign-spec.json"));
    roundtrip::<item::ItemDoc>(&e.join("item.json"));
    roundtrip::<job::CampaignJob>(&e.join("campaign-job.json"));
    roundtrip::<accept::AcceptRecord>(&e.join("accept.json"));
    roundtrip::<terminals::TerminalsDoc>(&e.join("terminals.json"));
    roundtrip::<control::ControlRequest>(&e.join("control.json"));
    roundtrip::<registry::Registry>(&e.join("registry.json"));
    roundtrip::<progress::CampaignProgress>(&e.join("progress-campaign.json"));
    roundtrip::<progress::NodeProgress>(&e.join("progress-node.json"));
    roundtrip::<progress::EventsProgress>(&e.join("progress-events.json"));
    roundtrip::<progress::ItemDetail>(&e.join("progress-item-detail.json"));
    let s: spec::CampaignSpec =
        serde_json::from_slice(&std::fs::read(e.join("campaign-spec.json")).unwrap()).unwrap();
    s.check().unwrap();
    let n: progress::NodeProgress =
        serde_json::from_slice(&std::fs::read(e.join("progress-node.json")).unwrap()).unwrap();
    assert!(n.closure_kind_consistent());
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    v.sort();
    for p in v {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn fixture_documents_roundtrip() {
    let mut files = Vec::new();
    walk(&schemas().join("fixtures"), &mut files);
    let mut n = 0;
    for p in files
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
    {
        let v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        let schema = v.get("schema").and_then(|s| s.as_str()).unwrap_or("");
        let file = v.get("file").and_then(|s| s.as_str()).unwrap_or("");
        match (schema, file) {
            ("rustred.campaign.progress.v1", "campaign") => {
                roundtrip::<progress::CampaignProgress>(p)
            }
            ("rustred.campaign.progress.v1", "node") => roundtrip::<progress::NodeProgress>(p),
            ("rustred.campaign.progress.v1", "events") => roundtrip::<progress::EventsProgress>(p),
            ("rustred.campaign.spec.v1", _) => roundtrip::<spec::CampaignSpec>(p),
            ("rustred.campaign.accept.v1", _) => roundtrip::<accept::AcceptRecord>(p),
            ("rustred.campaign.terminals.v1", _) => roundtrip::<terminals::TerminalsDoc>(p),
            ("rustred.campaign.control.v1", _) => roundtrip::<control::ControlRequest>(p),
            ("rustred.campaign.registry.v1", _) => roundtrip::<registry::Registry>(p),
            ("rustred.campaign.job.v1", _) => roundtrip::<job::CampaignJob>(p),
            _ => continue,
        }
        n += 1;
    }
    println!("fixture documents round-tripped through core structs: {n}");
    assert!(n > 0 || !schemas().join("fixtures/cdir-mid").exists());
}

#[test]
fn major_version_rule() {
    use rustred_campaign_core::schema::check_major;
    assert!(check_major("rustred.campaign.spec.v1", "rustred.campaign.spec.v1").is_ok());
    let e = check_major("rustred.campaign.spec.v2", "rustred.campaign.spec.v1").unwrap_err();
    assert!(e.contains("major version"), "{e}");
}

fn python() -> Option<&'static str> {
    match Command::new("python3").arg("--version").output() {
        Ok(o) if o.status.success() => Some("python3"),
        _ => {
            inputs::optional(format_args!(
                "SKIPPED: no python3 on PATH, the validator tests cannot run"
            ));
            None
        }
    }
}

fn run_py(args: &[&str]) -> (i32, String) {
    let o = Command::new("python3")
        .arg("-I")
        .args(args)
        .current_dir(schemas())
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    (o.status.code().unwrap_or(-1), text)
}

#[test]
fn validator_accepts_every_example_and_fixture() {
    if python().is_none() {
        return;
    }
    let mut ex: Vec<String> = std::fs::read_dir(schemas().join("examples"))
        .unwrap()
        .map(|e| format!("examples/{}", e.unwrap().file_name().to_string_lossy()))
        .collect();
    ex.sort();
    let mut args = vec!["validate.py".to_string()];
    args.extend(ex);
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (code, out) = run_py(&a);
    assert_eq!(code, 0, "validate.py on the examples:\n{out}");
    println!("{}", out.lines().last().unwrap_or(""));
    for d in ["cdir-empty", "cdir-mid", "cdir-finished", "cdir-stale"] {
        let p = format!("fixtures/{d}");
        if !schemas().join(&p).exists() {
            panic!("fixture {p} missing (generated by rustred-campaign tests/schema_roundtrip.rs)");
        }
        let (code, out) = run_py(&["validate.py", "--cdir", &p, "--now", "2026-11-14T11:59:00Z"]);
        assert_eq!(code, 0, "validate.py --cdir {p}:\n{out}");
        if d == "cdir-stale" {
            assert!(
                out.contains("STALE"),
                "stale fixture not reported STALE:\n{out}"
            );
        } else {
            assert!(!out.contains("STALE"), "{d} reported STALE:\n{out}");
        }
    }
}

#[test]
fn validator_rejects_mutations() {
    if python().is_none() {
        return;
    }
    let (code, out) = run_py(&["test_validate.py"]);
    println!("{out}");
    assert_eq!(code, 0, "mutation tests of validate.py failed:\n{out}");
}

/// The document rule of this crate equals validate.py's on the shared vectors (rows serde_json cannot hold exactly,
/// integers above 2^64 and `-0`, are skipped: they are refused or normalised by serde_json itself).
#[test]
fn document_canonical_form_on_shared_vectors() {
    let v: Value =
        serde_json::from_slice(&std::fs::read(schemas().join("canon-vectors.json")).unwrap())
            .unwrap();
    let mut n = 0;
    for x in v["vectors"].as_array().unwrap() {
        if matches!(
            x["name"].as_str(),
            Some("integer_above_2_64" | "minus_zero")
        ) {
            continue;
        }
        let got = rustred_campaign_core::schema::canon::canonical_bytes(
            x["input"].as_str().unwrap().as_bytes(),
            x["floats"].as_bool().unwrap(),
        )
        .unwrap();
        assert_eq!(
            got,
            x["document_rule"].as_str().unwrap(),
            "vector {}",
            x["name"]
        );
        n += 1;
    }
    println!("document-rule vectors equal in Rust and Python: {n}");
}
