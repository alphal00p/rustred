//! Shared helpers of the ledger_* / progress_* tests (package "ledger"). Not a test target itself.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign::clock::{Clock, ManualClock};
use rustred_campaign::ledger::{
    AcceptSource, Event, FileRef, ItemEvent, JobEvent, Ledger, LedgerConfig, RetirementProof,
    RuleEconomy,
};
use rustred_campaign::state::{CampaignEvent, EvidenceLevel, FinishClass, NodeEvent, Rung};

pub const T0: &str = "2026-11-13T23:00:00Z";

pub fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "rr-ledger-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub struct Cleanup(pub PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        if std::env::var_os("RR_KEEP_SCRATCH").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

pub fn clock() -> Arc<ManualClock> {
    Arc::new(ManualClock::at(T0).with_tick(250))
}

pub fn as_dyn(c: &Arc<ManualClock>) -> Arc<dyn Clock> {
    c.clone()
}

pub fn digest(s: &str) -> String {
    blake3::hash(s.as_bytes()).to_hex().to_string()
}

pub fn fref(path: &str) -> FileRef {
    FileRef {
        path: path.into(),
        bytes: path.len() as u64 * 100,
        blake3: digest(path),
    }
}

pub fn new_ledger(cdir: &Path, clock: Arc<dyn Clock>, cfg: LedgerConfig) -> Ledger {
    Ledger::create(cdir, clock, cfg, "test-campaign", &digest("spec"), 100).unwrap()
}

/// Inputs, one node, campaign started and running.
pub fn setup_events(node: &str) -> Vec<Event> {
    vec![
        Event::InputRegistered {
            input: "spec".into(),
            role: "spec".into(),
            path: "spec.json".into(),
            bytes: 1200,
            blake3: digest("spec"),
        },
        Event::InputRegistered {
            input: "rr_node".into(),
            role: "binary".into(),
            path: "bin/rr_node".into(),
            bytes: 9_000_000,
            blake3: digest("rr_node v1"),
        },
        Event::InputRegistered {
            input: format!("{node}.family"),
            role: "family".into(),
            path: format!("{node}.sym"),
            bytes: 3000,
            blake3: digest("family"),
        },
        Event::InputRegistered {
            input: format!("{node}.bank"),
            role: "table".into(),
            path: format!("large:{node}.bank.jsonl"),
            bytes: 77_000_000,
            blake3: digest("bank"),
        },
        Event::InputRegistered {
            input: format!("{node}.box"),
            role: "box".into(),
            path: format!("boxes/{node}.Rstar.json"),
            bytes: 900,
            blake3: digest("R* box"),
        },
        Event::NodeRegistered {
            node: node.into(),
            inputs: vec![format!("{node}.family"), format!("{node}.bank")],
            terminal_bound: None,
            spec_box: Some(format!("{node}.box")),
        },
        Event::Campaign {
            ev: CampaignEvent::Start,
            reason: "test".into(),
        },
        Event::Campaign {
            ev: CampaignEvent::Run,
            reason: "preflight ok".into(),
        },
        Event::Node {
            node: node.into(),
            ev: NodeEvent::Start,
        },
    ]
}

pub fn item_id(node: &str, k: usize) -> String {
    format!("{node}-item-{k:05}")
}
pub fn job_id(node: &str, k: usize, rung: u8) -> String {
    format!("{node}-job-{k:05}-L{rung}")
}

/// The records of one item's full life at rung `rung`: created, job queued, launch intent, started, finished
/// (rule found, sealed), produced, accepted record intent/done, accepted at `level`.
pub fn item_life(node: &str, k: usize, rung: u8, level: EvidenceLevel, cpu_ms: u64) -> Vec<Event> {
    let item = item_id(node, k);
    let job = job_id(node, k, rung);
    let table = fref(&format!("large:jobs/{job}.a1/out/F.table.jsonl"));
    let record = fref(&format!("accepted/{node}/{k:05}.json"));
    let op = format!("op-accept-{job}");
    vec![
        Event::ItemCreated {
            item: item.clone(),
            node: node.into(),
            kind: "face".into(),
            rung: Rung(rung),
            parent: None,
            inputs: vec![],
            label: format!("F{k:03}"),
            weight: 3,
        },
        Event::JobQueued {
            job: job.clone(),
            item: item.clone(),
            rung: Rung(rung),
            kind: "generate".into(),
            spec_blake3: digest(&job),
            binary: "rr_node".into(),
            reserved_bytes: 2 << 30,
        },
        Event::Item {
            item: item.clone(),
            ev: ItemEvent::Start {
                job: Some(job.clone()),
            },
        },
        Event::Job {
            job: job.clone(),
            attempt: 1,
            ev: JobEvent::Launch {
                run_dir: format!("large:jobs/{job}.a1"),
                cpus: vec![(k % 32) as u32],
                reserved_bytes: 2 << 30,
            },
        },
        Event::Job {
            job: job.clone(),
            attempt: 1,
            ev: JobEvent::Started {
                pid: 10_000 + k as u32,
                start_ticks: 777,
                boot_id: "boot".into(),
            },
        },
        Event::Job {
            job: job.clone(),
            attempt: 1,
            ev: finished(FinishClass::RuleFound, Some(table.clone()), cpu_ms),
        },
        Event::Item {
            item: item.clone(),
            ev: ItemEvent::Produce {
                job: job.clone(),
                attempt: 1,
                table: table.clone(),
            },
        },
        Event::Intent {
            op: op.clone(),
            action: "write_accepted_record".into(),
            target: record.path.clone(),
        },
        Event::Done {
            op,
            result: Some(record.clone()),
        },
        Event::Item {
            item,
            ev: ItemEvent::Accept {
                rule: format!("{node}:R:campaign:{k}"),
                level,
                record,
                source: AcceptSource::Job {
                    job,
                    attempt: 1,
                    table_blake3: table.blake3.clone(),
                },
                elided: false,
            },
        },
    ]
}

pub fn finished(class: FinishClass, sealed: Option<FileRef>, cpu_ms: u64) -> JobEvent {
    JobEvent::Finished {
        class,
        exit_code: Some(0),
        signal: None,
        sealed_output: sealed,
        outcome: None,
        cpu_ms,
        wall_ms: cpu_ms + 50,
        peak_rss_bytes: 400 << 20,
        time_decided: false,
    }
}

/// A small but complete scenario used by golden and verify tests (deterministic under the ManualClock).
pub fn scenario() -> Vec<Event> {
    let mut v = setup_events("T162");
    v.push(Event::EvidenceImported {
        evidence: "astra-bank-20261113".into(),
        producer: "Astra (Codex session), hand closure of T162".into(),
        description: "T162 bank, 2,448 rules; imported as hand-made evidence (M4)".into(),
        files: vec![
            fref("large:handoff/t162-bank/MANIFEST.json"),
            fref("large:handoff/t162-bank/NATIVE-RULE-BODIES.jsonl"),
        ],
    });
    for k in 0..4 {
        v.extend(item_life(
            "T162",
            k,
            0,
            if k % 2 == 0 {
                EvidenceLevel::IdentityProved
            } else {
                EvidenceLevel::GeometryAccepted
            },
            1000 * (k as u64 + 1),
        ));
    }
    // item 4: rejected at L0, escalated to L1 (child item 5), which goes to terminal.
    let i4 = item_id("T162", 4);
    let j4 = job_id("T162", 4, 0);
    let t4 = fref("large:jobs/j4/F.table.jsonl");
    v.push(Event::ItemCreated {
        item: i4.clone(),
        node: "T162".into(),
        kind: "locus".into(),
        rung: Rung(0),
        parent: None,
        inputs: vec![],
        label: "F004 n9=-1".into(),
        weight: 5,
    });
    v.push(Event::JobQueued {
        job: j4.clone(),
        item: i4.clone(),
        rung: Rung(0),
        kind: "generate".into(),
        spec_blake3: digest(&j4),
        binary: "rr_node".into(),
        reserved_bytes: 1 << 30,
    });
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Start {
            job: Some(j4.clone()),
        },
    });
    v.push(Event::Job {
        job: j4.clone(),
        attempt: 1,
        ev: JobEvent::Launch {
            run_dir: "large:jobs/j4.a1".into(),
            cpus: vec![7],
            reserved_bytes: 1 << 30,
        },
    });
    v.push(Event::Job {
        job: j4.clone(),
        attempt: 1,
        ev: JobEvent::Interrupt {
            reason: "campaign stop".into(),
        },
    });
    v.push(Event::Job {
        job: j4.clone(),
        attempt: 2,
        ev: JobEvent::Requeue {
            reason: "resume".into(),
        },
    });
    v.push(Event::Job {
        job: j4.clone(),
        attempt: 2,
        ev: JobEvent::Launch {
            run_dir: "large:jobs/j4.a2".into(),
            cpus: vec![7],
            reserved_bytes: 1 << 30,
        },
    });
    v.push(Event::Job {
        job: j4.clone(),
        attempt: 2,
        ev: finished(FinishClass::NoRule, Some(t4.clone()), 2500),
    });
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Produce {
            job: j4.clone(),
            attempt: 2,
            table: t4,
        },
    });
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Reject {
            reason: "rule_covers_none".into(),
        },
    });
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Escalate {
            to: Rung(1),
            reason: "rule_covers_none".into(),
        },
    });
    let i5 = item_id("T162", 5);
    v.push(Event::ItemCreated {
        item: i5.clone(),
        node: "T162".into(),
        kind: "locus".into(),
        rung: Rung(1),
        parent: Some(i4),
        inputs: vec![],
        label: "F004 n9=-1".into(),
        weight: 5,
    });
    v.push(Event::Item {
        item: i5.clone(),
        ev: ItemEvent::ProposeTerminal {
            reason: "terminal_after_ladder".into(),
        },
    });
    v.push(Event::Item {
        item: i5,
        ev: ItemEvent::DeclareTerminal {
            count: 2,
            keys: Some(fref("large:keysets/T162-terminals-1.keys")),
            reason: "terminal_after_ladder".into(),
        },
    });
    // an imported rule (evidence) and an upgrade
    let i6 = item_id("T162", 6);
    v.push(Event::ItemCreated {
        item: i6.clone(),
        node: "T162".into(),
        kind: "face".into(),
        rung: Rung(0),
        parent: None,
        inputs: vec![],
        label: "F006".into(),
        weight: 1,
    });
    v.push(Event::Item {
        item: i6.clone(),
        ev: ItemEvent::Start { job: None },
    });
    // evidence-backed acceptance: produce needs a job, so imported rules go open -> in_progress -> produced via a
    // checker job? No: imported rules are accepted from evidence after a geometry check job; modelled here with a
    // completed checker attempt.
    let j6 = job_id("T162", 6, 0);
    let t6 = fref("large:jobs/j6/verdict.json");
    v.push(Event::JobQueued {
        job: j6.clone(),
        item: i6.clone(),
        rung: Rung(0),
        kind: "descent".into(),
        spec_blake3: digest(&j6),
        binary: "rr_node".into(),
        reserved_bytes: 1 << 29,
    });
    v.push(Event::Job {
        job: j6.clone(),
        attempt: 1,
        ev: JobEvent::Launch {
            run_dir: "large:jobs/j6.a1".into(),
            cpus: vec![3],
            reserved_bytes: 1 << 29,
        },
    });
    v.push(Event::Job {
        job: j6.clone(),
        attempt: 1,
        ev: finished(FinishClass::Complete, Some(t6.clone()), 400),
    });
    v.push(Event::Item {
        item: i6.clone(),
        ev: ItemEvent::Produce {
            job: j6,
            attempt: 1,
            table: t6,
        },
    });
    v.push(Event::Item {
        item: i6.clone(),
        ev: ItemEvent::Accept {
            rule: "T162:R:bank:1043".into(),
            level: EvidenceLevel::GeometryAccepted,
            record: fref("accepted/T162/bank-1043.json"),
            source: AcceptSource::Evidence {
                evidence: "astra-bank-20261113".into(),
            },
            elided: false,
        },
    });
    v.push(Event::Item {
        item: i6,
        ev: ItemEvent::Upgrade {
            level: EvidenceLevel::IdentityTested,
            evidence: fref("accepted/T162/bank-1043.span.json"),
        },
    });
    v.push(Event::Node {
        node: "T162".into(),
        ev: NodeEvent::ClosureResult {
            round: 1,
            defect_keys: 37,
            closure_keys: 154_783,
            rules_used: Some(5),
            keyset: Some(fref("large:keysets/T162-closure-1.keys")),
        },
    });
    // economy measures (R31) and one supersession on this FULL-target node (coverage-preserving proof)
    for k in [0usize, 1, 2, 3] {
        v.push(Event::RuleEconomy {
            item: item_id("T162", k),
            economy: RuleEconomy {
                cases: 1,
                rules: 1,
                rhs_terms: 700 + 10 * k as u64,
                monomials: 5000 * (k as u64 + 1),
                coefficient_degree: 12 + k as u64,
                bytes: 90_000 + k as u64,
            },
        });
    }
    v.push(Event::Item {
        item: item_id("T162", 3),
        ev: ItemEvent::Retire {
            rule: "T162:R:campaign:3".into(),
            proof: RetirementProof::ReplacedBy {
                proof: fref("large:proofs/T162-containment-3-in-2.json"),
            },
            superseded_by: Some(item_id("T162", 2)),
        },
    });
    let mut counters = BTreeMap::new();
    counters.insert("rows".to_string(), 412u64);
    v.push(Event::JobProgress {
        job: job_id("T162", 0, 0),
        attempt: 1,
        counters,
    });
    v.push(Event::Note {
        text: "scenario end".into(),
    });
    v
}
