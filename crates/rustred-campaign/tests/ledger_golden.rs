//! Golden files of the ledger schemas (tests/data/ledger/): segment lines (byte for byte), manifest, snapshot,
//! lock owner record. A schema change shows up as a diff of these files. Regenerate deliberately with
//! `RR_UPDATE_GOLDEN=1 cargo test -p rustred-campaign --test ledger_golden`.
mod ledger_support;

use std::fs;
use std::path::PathBuf;

use ledger_support::*;
use rustred_campaign::clock::Clock;
use rustred_campaign::ledger::{
    self, CampaignMode, ControlEvent, Event, ExportEvent, Genesis,
    ItemEvent, Ledger, LedgerConfig, NodeSpecValues, RuleEconomy,
};
use rustred_campaign::lock;
use rustred_campaign::state::{CampaignEvent, CertLevel, EvidenceLevel, NodeEvent, Rung};

/// Records of kinds the scenario does not show (every kind has an example in record-examples.json).
fn extras() -> Vec<Event> {
    let n = "T162";
    vec![
        Event::Node {
            node: n.into(),
            ev: NodeEvent::WinnerDiffDone {
                round: 1,
                diff: fref("large:diff/T162.r1.json"),
                failures: 0,
            },
        },
        Event::Item {
            item: item_id(n, 0),
            ev: ItemEvent::ExcludeKeys {
                keys: fref("large:keysets/T162-0-excluded.keys"),
                disproof: fref("large:proofs/T162-0-exclusion.json"),
                reason: "keeps_terminal".into(),
            },
        },
        Event::Item {
            item: item_id(n, 3),
            ev: ItemEvent::Reinstate {
                reason: "replacement withdrawn".into(),
            },
        },
        Event::Control {
            id: "c-0002".into(),
            ev: ControlEvent::Refused {
                why: "item already reassigned".into(),
            },
        },
        Event::OrphanKilled {
            job: Some(job_id(n, 4, 0)),
            pid: 31337,
            start_ticks: 99,
        },
        Event::VerifyCheckpoint {
            seq: 3,
            clean: true,
            report: Some(fref("verify/report-1.json")),
        },
        Event::ArtefactLost {
            path: "accepted/T162/00001.json".into(),
            blake3: fref("accepted/T162/00001.json").blake3,
            items: vec![],
        },
        Event::Export {
            rev: "r1".into(),
            ev: ExportEvent::Candidate {
                manifest: fref("export/r1/MANIFEST.json"),
            },
        },
    ]
}

/// A campaign with invented names, another node shape and the optimise mode (genericity, R41 P-5): nothing in
/// the code knows the five-loop names.
fn invented() -> Vec<Event> {
    let n = "toy-sunset-3";
    let mut v = vec![
        input("spec", "spec", "toy spec"),
        input("rr_node", "binary", "rr_node toy"),
        input("toy.family", "family", "two-loop toy family with 3 lines"),
        Event::TablesImported {
            provenance: "toy-source".into(),
            producer: "campaign toy-0".into(),
            source_campaign: Some("toy-0".into()),
            manifest: fref("large:toy-0/MANIFEST.json"),
            files: vec![fref("large:toy-0/toy-sunset-3.table.jsonl")],
            level_claimed: EvidenceLevel::IdentityProved,
        },
        Event::NodeRegistered {
            node: n.into(),
            inputs: vec!["toy.family".into()],
            terminal_bound: Some(4),
            spec_box: None,
            spec: NodeSpecValues {
                certify_full: Some(CertLevel::PerKey),
                terminal_rung: Some(Rung(2)),
                ..Default::default()
            },
            reference: None,
        },
        Event::Campaign {
            ev: CampaignEvent::Start,
            reason: "toy".into(),
        },
        Event::Node {
            node: n.into(),
            ev: NodeEvent::Start,
        },
    ];
    for k in 0..3 {
        v.push(item_created(n, k, 0, None, "piece"));
        v.push(Event::Item {
            item: item_id(n, k),
            ev: ItemEvent::Import {
                rule: format!("{n}:R:{k}"),
                level: EvidenceLevel::IdentityProved,
                record: fref(&format!("accepted/{n}/{k}.json")),
                provenance: "toy-source".into(),
            },
        });
        v.push(Event::RuleEconomy {
            item: item_id(n, k),
            economy: RuleEconomy {
                cases: 1,
                rules: 1,
                rhs_terms: 40,
                monomials: 100,
                coefficient_degree: 3,
                bytes: 900,
            },
        });
    }
    v
}

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ledger")
}

fn check_bytes(name: &str, got: &[u8]) {
    let p = golden_dir().join(name);
    if std::env::var_os("RR_UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(golden_dir()).unwrap();
        fs::write(&p, got).unwrap();
        return;
    }
    let want = fs::read(&p)
        .unwrap_or_else(|_| panic!("golden {name} missing (RR_UPDATE_GOLDEN=1 creates it)"));
    if want != got {
        let w = String::from_utf8_lossy(&want);
        let g = String::from_utf8_lossy(got);
        for (i, (a, b)) in w.lines().zip(g.lines()).enumerate() {
            assert_eq!(a, b, "{name}: first difference at line {}", i + 1);
        }
        panic!(
            "{name}: lengths differ ({} vs {} bytes)",
            want.len(),
            got.len()
        );
    }
}

/// Pretty JSON golden (readable diffs) compared as values.
fn check_json(name: &str, got: &serde_json::Value) {
    let mut bytes = serde_json::to_vec_pretty(got).unwrap();
    bytes.push(b'\n');
    check_bytes(name, &bytes);
}

#[test]
fn ledger_schema_goldens() {
    let d = scratch("golden");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(scenario()).unwrap();
    l.append_all(extras()).unwrap();
    l.rotate().unwrap();
    l.append(Event::Campaign {
        ev: CampaignEvent::Drain,
        reason: "SIGINT (first Ctrl-C)".into(),
    })
    .unwrap();
    l.append(Event::Campaign {
        ev: CampaignEvent::Stop,
        reason: "drained".into(),
    })
    .unwrap();
    let ld = ledger::ledger_dir(&d);
    let m = l.manifest().clone();
    check_bytes(
        "segment-000001.jsonl",
        &fs::read(ld.join(&m.sealed[0].file)).unwrap(),
    );
    check_bytes(
        "segment-000002.jsonl",
        &fs::read(ld.join(&m.active.file)).unwrap(),
    );
    check_json("manifest.json", &serde_json::to_value(&m).unwrap());
    let snap: serde_json::Value =
        serde_json::from_slice(&fs::read(ld.join(&m.snapshot.as_ref().unwrap().file)).unwrap())
            .unwrap();
    check_json("snapshot.json", &snap);
    // one example record per kind, pretty, for the interface document
    let mut examples = serde_json::Map::new();
    for r in ledger::read_records(&d).unwrap() {
        let kind = r.event.kind().to_string();
        let key = match &r.event {
            Event::Item { ev, .. } => format!(
                "{kind}.{}",
                serde_json::to_value(ev).unwrap()["ev"].as_str().unwrap()
            ),
            Event::Job { ev, .. } => format!(
                "{kind}.{}",
                serde_json::to_value(ev).unwrap()["ev"].as_str().unwrap()
            ),
            Event::Node { ev, .. } => format!(
                "{kind}.{}",
                serde_json::to_value(ev).unwrap()["ev"].as_str().unwrap()
            ),
            _ => kind,
        };
        examples
            .entry(key)
            .or_insert(serde_json::to_value(&r).unwrap());
    }
    check_json("record-examples.json", &serde_json::Value::Object(examples));
}

#[test]
fn invented_campaign_golden() {
    let d = scratch("golden-invented");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut g = Genesis::new("toy-optimise", &digest("toy spec"), 4);
    g.mode = CampaignMode::Optimise;
    let mut l = Ledger::create(&d, lock_of(&d), as_dyn(&clk), LedgerConfig::default(), g).unwrap();
    l.append_all(invented()).unwrap();
    let ld = ledger::ledger_dir(&d);
    check_bytes(
        "invented-segment-000001.jsonl",
        &fs::read(ld.join(&l.manifest().active.file)).unwrap(),
    );
    let set = rustred_campaign::progress::derive(l.state(), l.head(), &Default::default(), T0);
    check_json("invented-progress-node.json", &set["nodes/toy-sunset-3.json"]);
    assert_eq!(set["campaign.json"]["campaign"]["mode"], "optimise");
}

#[test]
fn lock_owner_record_golden() {
    let d = scratch("goldlock");
    let _c = Cleanup(d.clone());
    let clk = clock();
    let c: &dyn Clock = clk.as_ref();
    let (lk, _) = lock::acquire(
        &d,
        c,
        &[
            "rr_campaign".into(),
            "run".into(),
            "--spec".into(),
            "spec.json".into(),
        ],
    )
    .unwrap();
    let mut v: serde_json::Value =
        serde_json::from_slice(&fs::read(d.join(lock::OWNER_FILE)).unwrap()).unwrap();
    // host-dependent fields normalised
    for (k, val) in [
        ("pid", serde_json::json!(4242)),
        ("start_ticks", serde_json::json!(123456789)),
        (
            "boot_id",
            serde_json::json!("6b1f0c55-0000-4000-8000-000000000000"),
        ),
        ("host", serde_json::json!("itphlies")),
        (
            "token",
            serde_json::json!("0123456789abcdef0123456789abcdef"),
        ),
    ] {
        assert!(v.get(k).is_some(), "owner record has {k}");
        v[k] = val;
    }
    check_json("lock-owner.json", &v);
    drop(lk);
}
