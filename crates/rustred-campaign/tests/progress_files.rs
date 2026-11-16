//! Progress files: golden files of every progress schema, atomic writing with unchanged-file skipping,
//! re-derivation by a second reader, item detail, and `verify` (M5) incl. tamper and truncation detection.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
mod ledger_support;

use std::fs;
use std::path::PathBuf;

use ledger_support::*;
use rustred_campaign::clock::Clock;
use rustred_campaign::ledger::{self, Event, LedgerConfig};
use rustred_campaign::progress::{self, LiveJob, ProgressWriter, Resources, SupervisorInfo};
use rustred_campaign::state::{CampaignState, EvidenceLevel, reduce};

fn golden(name: &str, got: &serde_json::Value) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/ledger")
        .join(name);
    let mut bytes = serde_json::to_vec_pretty(got).unwrap();
    bytes.push(b'\n');
    if std::env::var_os("RR_UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, &bytes).unwrap();
        return;
    }
    let want = fs::read_to_string(&p)
        .unwrap_or_else(|_| panic!("golden {name} missing (RR_UPDATE_GOLDEN=1 creates it)"));
    let got = String::from_utf8(bytes).unwrap();
    for (i, (a, b)) in want.lines().zip(got.lines()).enumerate() {
        assert_eq!(a, b, "{name}: first difference at line {}", i + 1);
    }
    assert_eq!(want.len(), got.len(), "{name}: length differs");
}

pub fn resources() -> Resources {
    Resources {
        cpus_in_use: Some(21),
        cpus_allowed: Some(48),
        live_jobs: Some(3),
        reserved_bytes: Some(3 << 30),
        measured_rss_bytes: Some(1_234_567_890),
        mem_cap_bytes: Some(30 << 30),
        host_mem_available_bytes: Some(268 << 30),
        host_floor_bytes: Some(50 << 30),
        disk_free_bytes: Some(900 << 30),
        large_free_bytes: None,
        budget_cpu_hours: Some(200),
        sampled_utc: Some("2026-11-13T23:10:00.000Z".into()),
        supervisor: Some(SupervisorInfo {
            heartbeat_utc: Some("2026-11-13T23:10:00.500Z".into()),
            started_utc: Some("2026-11-13T23:00:00.000Z".into()),
            pid: Some(4242),
            host: Some("host-a".into()),
            grace_left_s: None,
            stopping: false,
        }),
        jobs: vec![LiveJob {
            job: "T162-job-00010-L0".into(),
            attempt: 1,
            cpu_set: vec![29, 30],
            rss_bytes: Some(431_000_000),
            budget_mem_bytes: Some(2 << 30),
            seconds: Some(18),
            budget_seconds: Some(300),
            heartbeat_utc: Some("2026-11-13T23:09:59.000Z".into()),
            pid: Some(5150),
            pgid: Some(5150),
            phase: Some("seeding".into()),
        }],
    }
}

#[test]
fn progress_schema_goldens() {
    let d = scratch("pgold");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    // some open work so the ETA has something to estimate
    for k in 10..13 {
        l.append_all(item_life("T162", k, 0, EvidenceLevel::IdentityTested, 1500)[..1].to_vec())
            .unwrap();
    }
    // one running attempt (shown in jobs.json with the supervisor's live sample)
    l.append_all(item_life("T162", 10, 0, EvidenceLevel::IdentityTested, 1500)[1..5].to_vec())
        .unwrap();
    let set = progress::derive(
        l.state(),
        l.head(),
        &resources(),
        "2026-11-13T23:10:01.000Z",
    );
    assert_eq!(
        set.keys().cloned().collect::<Vec<_>>(),
        vec![
            "attention.json",
            "campaign.json",
            "events.json",
            "heartbeat.json",
            "jobs.json",
            "nodes/T162.json",
            "terminals.json"
        ]
    );
    golden("progress-campaign.json", &set["campaign.json"]);
    golden("progress-node-T162.json", &set["nodes/T162.json"]);
    golden("progress-events.json", &set["events.json"]);
    golden("progress-terminals.json", &set["terminals.json"]);
    golden("progress-attention.json", &set["attention.json"]);
    golden("progress-jobs.json", &set["jobs.json"]);
    golden("progress-heartbeat.json", &set["heartbeat.json"]);
    golden(
        "progress-item-detail.json",
        &progress::item_detail(l.state(), &item_id("T162", 4)).unwrap(),
    );
    // spot checks of the required measures
    let c = &set["campaign.json"];
    assert_eq!(c["schema"], "rustred.campaign.progress.v1");
    assert_eq!(c["resources"]["cpus_in_use"], 21);
    assert_eq!(c["items"]["by_level"]["L0"]["open"], 2);
    assert_eq!(c["rules"]["accepted"], 5);
    assert_eq!(c["rules"]["imported"], 1);
    assert_eq!(c["rules"]["rejected"], 1);
    assert_eq!(c["terminals"]["declared"], 3);
    assert_eq!(c["terminals"]["preferred"]["used"], 1);
    assert_eq!(c["terminals"]["preferred"]["not_needed"], 2);
    assert_eq!(c["terminals"]["other"], 2);
    assert_eq!(c["closure"]["defect_keys"], 37);
    assert_eq!(c["certification"]["asked"]["replay"], 1);
    assert_eq!(c["cost"]["cpu_seconds"], 12.9);
    assert_eq!(c["campaign"]["mode"], "close");
    assert_eq!(c["attention"]["count"], 2);
    assert!(
        c["resources"]["large_free_bytes"].is_null(),
        "unknown = null (m10)"
    );
    assert!(c["eta"]["wall_seconds"].is_object(), "{}", c["eta"]);
    let n = &set["nodes/T162.json"];
    assert_eq!(n["certification"]["asked"], "replay");
    assert_eq!(n["provisional"], false);
    assert_eq!(n["reference"]["terminals"], 7);
    let j = &set["jobs.json"]["list"][0];
    assert_eq!(
        (j["cpus"].as_u64(), j["rss_bytes"].as_u64()),
        (Some(2), Some(431_000_000))
    );
}

#[test]
fn writer_is_atomic_and_skips_unchanged_files_and_a_second_reader_rederives() {
    let d = scratch("pwrite");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    let mut w = ProgressWriter::new();
    let c: &dyn Clock = clk.as_ref();
    let r1 = w.write(&d, l.state(), l.head(), &resources(), c).unwrap();
    assert_eq!(r1.written.len(), 7);
    // nothing changed: only the heartbeat is rewritten (an idle supervisor never looks stale)
    let r2 = w.write(&d, l.state(), l.head(), &resources(), c).unwrap();
    assert_eq!(r2.written, vec!["heartbeat.json".to_string()], "{r2:?}");
    assert_eq!(r2.unchanged, 6);
    l.append(Event::Note {
        text: "only a note".into(),
    })
    .unwrap();
    // only the ledger position moved: campaign.json (which carries it) and the heartbeat (audit A m9)
    let r3 = w.write(&d, l.state(), l.head(), &resources(), c).unwrap();
    assert_eq!(
        r3.written,
        vec!["campaign.json".to_string(), "heartbeat.json".to_string()],
        "{r3:?}"
    );
    // no temporaries left in progress/
    for e in fs::read_dir(d.join("progress")).unwrap() {
        assert!(!e.unwrap().file_name().to_string_lossy().starts_with('.'));
    }
    // a second process derives the same files from the ledger alone
    let recs = ledger::read_records(&d).unwrap();
    let st: CampaignState = reduce(&recs).unwrap();
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join("progress/campaign.json")).unwrap()).unwrap();
    let again = progress::derive(
        &st,
        l.head(),
        &resources(),
        on_disk["written_utc"].as_str().unwrap(),
    );
    assert_eq!(again["campaign.json"], on_disk);
}

#[test]
fn verify_recomputes_every_number_and_reports_differences() {
    let d = scratch("pverify");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let cfg = LedgerConfig {
        rotate_bytes: 6 * 1024,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger(&d, as_dyn(&clk), cfg.clone());
    l.append_all(scenario()).unwrap();
    for k in 20..40 {
        l.append_all(item_life("T162", k, 1, EvidenceLevel::IdentityProved, 700))
            .unwrap();
    }
    assert!(!l.manifest().sealed.is_empty(), "rotation happened");
    let mut w = ProgressWriter::new();
    let c: &dyn Clock = clk.as_ref();
    w.write(&d, l.state(), l.head(), &resources(), c).unwrap();
    let written_at = l.seq();
    let rep = progress::verify(&d).unwrap();
    assert!(
        rep.clean(),
        "{:?} {:?} {:?}",
        rep.differences,
        rep.missing,
        rep.stale
    );
    assert_eq!(rep.files_checked, 7);
    assert_eq!(rep.snapshot_matches, Some(true));
    // the ledger moves on: verified against its END the files are older (M7) and verify says which; verified
    // against the seq they were written at they are clean
    l.append_all(item_life("T162", 50, 2, EvidenceLevel::IdentityTested, 900))
        .unwrap();
    let rep = progress::verify(&d).unwrap();
    assert!(!rep.clean());
    assert_eq!(rep.stale, vec![("campaign.json".to_string(), written_at)]);
    let rep = progress::verify_against(&d, Some(written_at)).unwrap();
    assert!(rep.clean(), "{:?}", rep.differences);
    w.write(&d, l.state(), l.head(), &resources(), c).unwrap();
    // tamper with a number
    let p = d.join("progress/campaign.json");
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    v["rules"]["accepted"] = serde_json::json!(999);
    fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let rep = progress::verify(&d).unwrap();
    assert_eq!(rep.differences.len(), 1, "{:?}", rep.differences);
    assert!(rep.missing.is_empty() && rep.stale.is_empty());
    assert_eq!(rep.differences[0].path, "/rules/accepted");
    assert_eq!(rep.differences[0].found, "999");
    // a progress file ahead of the ledger means records were lost (ledger truncated)
    v["rules"]["accepted"] = serde_json::json!(l.state().rules.accepted);
    v["ledger"]["seq"] = serde_json::json!(l.seq() + 10);
    fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let rep = progress::verify(&d).unwrap();
    assert!(
        rep.differences
            .iter()
            .any(|x| x.path == "/ledger/seq" && x.found.contains("ahead")),
        "{:?}",
        rep.differences
    );
    // absent files are not clean (M7)
    fs::remove_file(d.join("progress/terminals.json")).unwrap();
    let rep = progress::verify(&d).unwrap();
    assert_eq!(rep.missing, vec!["terminals.json".to_string()]);
    fs::remove_dir_all(d.join("progress")).unwrap();
    let rep = progress::verify(&d).unwrap();
    assert!(!rep.clean() && rep.missing == vec!["campaign.json".to_string()]);
    drop(l);
    // reopen is unaffected (progress files are derived, never read back)
    assert!(open_ledger(&d, as_dyn(&clk), cfg).is_ok());
}
