//! Shared helpers of the ledger_* / progress_* tests (package "ledger"). Not a test target itself.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use rustred_campaign::clock::{Clock, ManualClock, SystemClock};
use rustred_campaign::ledger::{
    self, AcceptSource, CampaignMode, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind, Event,
    FileOps, FileRef, Genesis, ItemEvent, JobEvent, Ledger, LedgerConfig, LedgerError,
    LevelEvidence, NodeSpecValues, OpenOptions, OpenReport, OutcomeBinding, RetirementProof,
    RuleEconomy,
};
use rustred_campaign::lock::{self, CampaignLock};
use rustred_campaign::state::{
    self, AttemptEndCause, Blockers, CampaignEvent, CampaignState, ContractAccept, EvidenceLevel,
    FinishClass, NodeEvent, Rung,
};

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

/// The campaign lock of `cdir`, acquired once per process and kept for the test's lifetime (a ledger write
/// handle needs it, audit A M6).
pub fn lock_of(cdir: &Path) -> &'static CampaignLock {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, &'static CampaignLock>>> = OnceLock::new();
    let m = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut g = m.lock().unwrap();
    let key = std::fs::canonicalize(cdir).unwrap_or_else(|_| cdir.to_path_buf());
    if let Some(l) = g.get(&key) {
        return l;
    }
    let (l, _) = lock::acquire(cdir, &SystemClock::new(), &["test".into()]).unwrap();
    let l: &'static CampaignLock = Box::leak(Box::new(l));
    g.insert(key, l);
    l
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

pub fn genesis() -> Genesis {
    Genesis::new("test-campaign", &digest("spec"), 100)
}

pub fn new_ledger(cdir: &Path, clock: Arc<dyn Clock>, cfg: LedgerConfig) -> Ledger {
    Ledger::create(cdir, lock_of(cdir), clock, cfg, genesis()).unwrap()
}

pub fn new_ledger_with(
    cdir: &Path,
    clock: Arc<dyn Clock>,
    cfg: LedgerConfig,
    ops: Arc<dyn FileOps>,
) -> Result<Ledger, LedgerError> {
    Ledger::create_with(cdir, lock_of(cdir), clock, cfg, ops, genesis())
}

pub fn open_ledger(
    cdir: &Path,
    clock: Arc<dyn Clock>,
    cfg: LedgerConfig,
) -> Result<(Ledger, OpenReport), LedgerError> {
    Ledger::open(cdir, lock_of(cdir), clock, cfg)
}

pub fn open_ledger_with(
    cdir: &Path,
    clock: Arc<dyn Clock>,
    cfg: LedgerConfig,
    ops: Arc<dyn FileOps>,
    opts: OpenOptions,
) -> Result<(Ledger, OpenReport), LedgerError> {
    Ledger::open_with(cdir, lock_of(cdir), clock, cfg, ops, opts)
}

pub fn input(id: &str, role: &str, content: &str) -> Event {
    Event::InputRegistered {
        input: id.into(),
        role: role.into(),
        path: format!("inputs/{id}"),
        bytes: 1000,
        blake3: digest(content),
    }
}

/// Inputs, one node, campaign started and running. The node's spec: terminal rung L1, fixer-round budget 2.
pub fn setup_events(node: &str) -> Vec<Event> {
    vec![
        input("spec", "spec", "spec"),
        input("rr_node", "binary", "rr_node v1"),
        input(&format!("{node}.family"), "family", "family"),
        input(&format!("{node}.bank"), "table", "bank"),
        input(&format!("{node}.box"), "box", "user box"),
        Event::NodeRegistered {
            node: node.into(),
            inputs: vec![format!("{node}.family"), format!("{node}.bank")],
            terminal_bound: None,
            spec_box: Some(format!("{node}.box")),
            spec: NodeSpecValues {
                terminal_rung: Some(Rung(1)),
                full_attempt_fixer_rounds: Some(2),
                preferred_terminals: Some(fref(&format!("spec/{node}.preferred.json"))),
                preferred_listed: Some(3),
                ..Default::default()
            },
            reference: Some(BTreeMap::from([
                ("name".to_string(), "old ledger".to_string()),
                ("terminals".to_string(), "7".to_string()),
            ])),
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
pub fn rule_id(node: &str, k: usize) -> String {
    format!("{node}:R:campaign:{k}")
}
pub fn record_ref(node: &str, k: usize) -> FileRef {
    fref(&format!("accepted/{node}/{k:05}.json"))
}
pub fn table_ref(job: &str, attempt: u32) -> FileRef {
    fref(&format!("large:jobs/{job}.a{attempt}/out/F.table.jsonl"))
}

/// Evidence artefacts for a level (M3).
pub fn level_evidence(level: EvidenceLevel, tag: &str) -> Option<LevelEvidence> {
    match level {
        EvidenceLevel::GeometryAccepted => None,
        EvidenceLevel::IdentityTested => Some(LevelEvidence::IdentityTested {
            verdict: fref(&format!("evidence/{tag}.replay.json")),
        }),
        EvidenceLevel::IdentityProved => Some(LevelEvidence::IdentityProved {
            proofs: fref(&format!("large:evidence/{tag}.proofs.jsonl")),
            keys: fref(&format!("large:evidence/{tag}.keys")),
        }),
    }
}

pub fn finished_with(
    job: &str,
    attempt: u32,
    class: FinishClass,
    accept: ContractAccept,
    sealed: Option<FileRef>,
    cpu_ms: u64,
) -> JobEvent {
    let records = matches!(
        accept,
        ContractAccept::Canonical | ContractAccept::CommittedRules
    );
    JobEvent::Finished {
        class,
        accept,
        exit_code: Some(0),
        signal: None,
        sealed_output: sealed,
        outcome: records.then(|| fref(&format!("large:jobs/{job}.a{attempt}/OUTCOME.json"))),
        binding: records.then(|| OutcomeBinding {
            job: job.into(),
            attempt,
            spec_blake3: digest(job),
        }),
        cpu_ms,
        wall_ms: cpu_ms + 50,
        peak_rss_bytes: 400 << 20,
        time_decided: false,
    }
}

pub fn job_queued(job: &str, item: &str, rung: u8, kind: &str) -> Event {
    Event::JobQueued {
        job: job.into(),
        item: item.into(),
        rung: Rung(rung),
        kind: kind.into(),
        spec_blake3: digest(job),
        binary: "rr_node".into(),
        reserved_bytes: 2 << 30,
        inputs: vec![],
    }
}

pub fn item_created(node: &str, k: usize, rung: u8, parent: Option<String>, kind: &str) -> Event {
    Event::ItemCreated {
        item: item_id(node, k),
        node: node.into(),
        kind: kind.into(),
        rung: Rung(rung),
        parent,
        inputs: vec![],
        label: format!("F{k:03}"),
        weight: 3,
    }
}

/// The records up to a sealed, produced result of item `k` (job attempt 1, class rule_found, canonical).
pub fn item_until_produced(node: &str, k: usize, rung: u8, cpu_ms: u64) -> Vec<Event> {
    let item = item_id(node, k);
    let job = job_id(node, k, rung);
    let table = table_ref(&job, 1);
    vec![
        item_created(node, k, rung, None, "face"),
        job_queued(&job, &item, rung, "generate"),
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
            ev: finished_with(
                &job,
                1,
                FinishClass::RuleFound,
                ContractAccept::Canonical,
                Some(table.clone()),
                cpu_ms,
            ),
        },
        Event::Item {
            item,
            ev: ItemEvent::Produce {
                job,
                attempt: 1,
                table,
            },
        },
    ]
}

pub fn accept_event(node: &str, k: usize, rung: u8, level: EvidenceLevel) -> Event {
    let job = job_id(node, k, rung);
    Event::Item {
        item: item_id(node, k),
        ev: ItemEvent::Accept {
            rule: rule_id(node, k),
            level,
            record: record_ref(node, k),
            source: AcceptSource::Job {
                table_blake3: table_ref(&job, 1).blake3,
                job,
                attempt: 1,
            },
            elided: false,
            evidence: level_evidence(level, &format!("{node}-{k}")),
            round: Some(1),
        },
    }
}

/// The records of one item's full life at rung `rung`: created, job queued, launch, started, finished (rule
/// found, sealed, canonical), produced, accepted-record intent/done, accepted at `level`.
pub fn item_life(node: &str, k: usize, rung: u8, level: EvidenceLevel, cpu_ms: u64) -> Vec<Event> {
    let mut v = item_until_produced(node, k, rung, cpu_ms);
    let op = format!("op-accept-{}", job_id(node, k, rung));
    v.push(Event::Intent {
        op: op.clone(),
        action: "write_accepted_record".into(),
        target: record_ref(node, k).path,
    });
    v.push(Event::Done {
        op,
        result: Some(record_ref(node, k)),
    });
    v.push(accept_event(node, k, rung, level));
    v
}

pub fn finished(class: FinishClass, sealed: Option<FileRef>, cpu_ms: u64) -> JobEvent {
    let accept = state::contract_accepts(class)[0];
    let accept = if sealed.is_none() {
        ContractAccept::Nothing
    } else {
        accept
    };
    finished_with("job", 1, class, accept, sealed, cpu_ms)
}

/// Assembled + closure result of `node` on its CURRENT rule set (computed from `st`).
pub fn closure_events(st: &CampaignState, node: &str, round: u32, defect: u64) -> Vec<Event> {
    vec![
        assembled(node, round),
        closure_event(st, node, round, defect, EnvelopeKind::Proved),
    ]
}

pub fn assembled(node: &str, round: u32) -> Event {
    Event::Node {
        node: node.into(),
        ev: NodeEvent::Assembled {
            round,
            manifest: fref(&format!("large:tables/{node}.r{round}.manifest.json")),
            dispatch_order: fref(&format!("large:tables/{node}.order.json")),
        },
    }
}

pub fn closure_event(
    st: &CampaignState,
    node: &str,
    round: u32,
    defect: u64,
    envelope: EnvelopeKind,
) -> Event {
    Event::Node {
        node: node.into(),
        ev: NodeEvent::ClosureResult {
            round,
            defect_keys: defect,
            closure_keys: 154_783,
            rules_used: Some(5),
            rules_total: Some(st.node_live_rules(node).len() as u64),
            keyset: Some(fref(&format!("large:keysets/{node}-closure-{round}.keys"))),
            rule_set_digest: st.node_rule_set_digest(node).unwrap(),
            dispatch_order_digest: fref(&format!("large:tables/{node}.order.json")).blake3,
            envelope,
            envelope_tables: vec![fref(&format!("large:handdown/{node}.ranges.json"))],
        },
    }
}

/// Reduces events (the test's model of what a ledger holds after appending them).
pub fn reduce_events(events: &[Event]) -> CampaignState {
    let mut st = CampaignState::default();
    for (i, e) in events.iter().enumerate() {
        let r = ledger::Record {
            schema: ledger::LEDGER_SCHEMA.into(),
            seq: i as u64 + 1,
            utc: T0.into(),
            prev: ledger::GENESIS.into(),
            event: e.clone(),
        };
        st.apply_record(&r)
            .unwrap_or_else(|err| panic!("event {i} ({}): {err}", e.kind()));
    }
    st
}

pub fn genesis_event() -> Event {
    Event::CampaignCreated {
        campaign_id: "test-campaign".into(),
        spec_blake3: digest("spec"),
        terminal_bound: 100,
        mode: CampaignMode::Close,
    }
}

/// A small but complete scenario used by golden and verify tests (deterministic under the ManualClock).
pub fn scenario() -> Vec<Event> {
    let n = "T162";
    let mut v = setup_events(n);
    v.push(Event::EvidenceImported {
        evidence: "astra-bank-20261113".into(),
        producer: "hand closure (reference)".into(),
        description: "bank of hand-made rules; imported as hand-made evidence (M4)".into(),
        files: vec![
            fref("large:handoff/bank/MANIFEST.json"),
            fref("large:handoff/bank/NATIVE-RULE-BODIES.jsonl"),
        ],
        level_claimed: EvidenceLevel::IdentityTested,
    });
    v.push(Event::TablesImported {
        provenance: "earlier-campaign".into(),
        producer: "campaign c-20261101".into(),
        source_campaign: Some("c-20261101".into()),
        manifest: fref("large:imports/c-20261101/MANIFEST.json"),
        files: vec![fref("large:imports/c-20261101/T162.table.jsonl")],
        level_claimed: EvidenceLevel::IdentityTested,
    });
    for k in 0..4 {
        v.extend(item_life(
            n,
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
    // item 4: interrupted, requeued, no rule (committed rules row 4), rejected, escalated to L1 (child item 5),
    // which becomes a terminal after its ladder is exhausted.
    let i4 = item_id(n, 4);
    let j4 = job_id(n, 4, 0);
    let t4 = table_ref(&j4, 2);
    v.push(item_created(n, 4, 0, None, "locus"));
    v.push(job_queued(&j4, &i4, 0, "generate"));
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Start {
            job: Some(j4.clone()),
        },
    });
    for (attempt, ev) in [
        (
            1,
            JobEvent::Launch {
                run_dir: "large:jobs/j4.a1".into(),
                cpus: vec![7],
                reserved_bytes: 1 << 30,
            },
        ),
        (
            1,
            JobEvent::Interrupt {
                reason: "campaign stop".into(),
            },
        ),
        (
            2,
            JobEvent::Requeue {
                reason: "resume".into(),
            },
        ),
        (
            2,
            JobEvent::Launch {
                run_dir: "large:jobs/j4.a2".into(),
                cpus: vec![7],
                reserved_bytes: 1 << 30,
            },
        ),
        (
            2,
            finished_with(
                &j4,
                2,
                FinishClass::NoRule,
                ContractAccept::CommittedRules,
                Some(t4.clone()),
                2500,
            ),
        ),
    ] {
        v.push(Event::Job {
            job: j4.clone(),
            attempt,
            ev,
        });
    }
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
            disproof: None,
        },
    });
    v.push(Event::Item {
        item: i4.clone(),
        ev: ItemEvent::Escalate {
            to: Rung(1),
            reason: "rule_covers_none".into(),
        },
    });
    let i5 = item_id(n, 5);
    v.push(item_created(n, 5, 1, Some(i4), "locus"));
    v.push(Event::Item {
        item: i5.clone(),
        ev: ItemEvent::ProposeTerminal {
            reason: "terminal_after_ladder".into(),
            preferred: false,
            exhausted: Some(fref("large:ladder/T162-item-5.exhausted.json")),
        },
    });
    v.push(Event::Item {
        item: i5,
        ev: ItemEvent::DeclareTerminal {
            count: 2,
            keys: fref("large:keysets/T162-terminals-1.keys"),
            reason: "terminal_after_ladder".into(),
            preferred: false,
        },
    });
    // item 6: a hand-made rule checked by a checker job, accepted from evidence, upgraded with its replay verdict
    let i6 = item_id(n, 6);
    let j6 = job_id(n, 6, 0);
    let t6 = fref("large:jobs/j6/verdict.json");
    v.push(item_created(n, 6, 0, None, "face"));
    v.push(Event::Item {
        item: i6.clone(),
        ev: ItemEvent::Start { job: None },
    });
    v.push(job_queued(&j6, &i6, 0, "descent"));
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
        ev: finished_with(
            &j6,
            1,
            FinishClass::Complete,
            ContractAccept::Canonical,
            Some(t6.clone()),
            400,
        ),
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
            evidence: None,
            round: None,
        },
    });
    v.push(Event::Item {
        item: i6,
        ev: ItemEvent::Upgrade {
            level: EvidenceLevel::IdentityTested,
            evidence: level_evidence(EvidenceLevel::IdentityTested, "bank-1043").unwrap(),
        },
    });
    // item 7: a preferred terminal (allowed from the start, no ladder citation, not counted against the bound)
    let i7 = item_id(n, 7);
    v.push(item_created(n, 7, 0, None, "point_set"));
    v.push(Event::Item {
        item: i7.clone(),
        ev: ItemEvent::ProposeTerminal {
            reason: "preferred terminal of the spec".into(),
            preferred: true,
            exhausted: None,
        },
    });
    v.push(Event::Item {
        item: i7,
        ev: ItemEvent::DeclareTerminal {
            count: 1,
            keys: fref("large:keysets/T162-preferred-1.keys"),
            reason: "preferred".into(),
            preferred: true,
        },
    });
    // item 8: an imported rule (tables_imported provenance)
    let i8 = item_id(n, 8);
    v.push(item_created(n, 8, 0, None, "face"));
    v.push(Event::Item {
        item: i8,
        ev: ItemEvent::Import {
            rule: "T162:R:imported:8".into(),
            level: EvidenceLevel::IdentityTested,
            record: fref("accepted/T162/imported-8.json"),
            provenance: "earlier-campaign".into(),
        },
    });
    // closure result on the current rule set
    let st = reduce_events(&with_genesis(&v));
    v.push(Event::Node {
        node: n.into(),
        ev: NodeEvent::FixerRoundBegin {
            round: 1,
            partition: Some(fref("large:partition/T162.r1.json")),
        },
    });
    v.extend(closure_events(&st, n, 1, 37));
    // economy measures (R31) and one supersession on this FULL-target node (coverage-preserving proof)
    for k in [0usize, 1, 2, 3] {
        v.push(Event::RuleEconomy {
            item: item_id(n, k),
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
        item: item_id(n, 3),
        ev: ItemEvent::Retire {
            rule: rule_id(n, 3),
            proof: RetirementProof::ReplacedBy {
                proof: fref("large:proofs/T162-containment-3-in-2.json"),
            },
            superseded_by: Some(item_id(n, 2)),
        },
    });
    // item 9: needs a person; an operator retry is requested (pending: shows in attention.json)
    let i9 = item_id(n, 9);
    v.push(item_created(n, 9, 0, None, "face"));
    v.push(Event::Item {
        item: i9.clone(),
        ev: ItemEvent::Attention {
            code: "needs_bigmem".into(),
            text: "the L2 run needs more memory than the policy allows".into(),
        },
    });
    v.push(Event::Control {
        id: "c-0001".into(),
        ev: ControlEvent::Requested {
            verb: ControlVerb::Pause,
            target: ControlTarget::Campaign,
            by: "operator (tty)".into(),
            reason: "maintenance".into(),
        },
    });
    v.push(Event::Control {
        id: "c-0001".into(),
        ev: ControlEvent::Applied {
            effect: "launches paused".into(),
        },
    });
    v.push(Event::Control {
        id: "c-0003".into(),
        ev: ControlEvent::Requested {
            verb: ControlVerb::Resume,
            target: ControlTarget::Campaign,
            by: "operator (tty)".into(),
            reason: "maintenance done".into(),
        },
    });
    v.push(Event::Control {
        id: "c-0003".into(),
        ev: ControlEvent::Applied {
            effect: "launches resumed".into(),
        },
    });
    v.push(Event::Control {
        id: "c-0002".into(),
        ev: ControlEvent::Requested {
            verb: ControlVerb::Retry,
            target: ControlTarget::Item { item: i9 },
            by: "operator (tty)".into(),
            reason: "BIGMEM authorised".into(),
        },
    });
    v.push(Event::Note {
        text: "scenario end".into(),
    });
    v
}

/// `events` preceded by record 1 (for [`reduce_events`]).
pub fn with_genesis(events: &[Event]) -> Vec<Event> {
    let mut v = vec![genesis_event()];
    v.extend_from_slice(events);
    v
}

/// A node closed FULL from scratch (items 0..n_items accepted at `level`), events only.
pub fn closed_node(node: &str, n_items: usize, level: EvidenceLevel) -> Vec<Event> {
    let mut v = setup_events(node);
    for k in 0..n_items {
        v.extend(item_life(node, k, 0, level, 1000));
    }
    let st = reduce_events(&with_genesis(&v));
    v.extend(closure_events(&st, node, 1, 0));
    v.push(Event::Node {
        node: node.into(),
        ev: NodeEvent::DeclareClosed,
    });
    v
}

pub fn blockers() -> Blockers {
    Blockers {
        faces: 3,
        loci: 1,
        keys: 87,
        list: Some(fref("large:keysets/blockers.keys")),
    }
}

pub fn attempt_ended(node: &str) -> Event {
    Event::Node {
        node: node.into(),
        ev: NodeEvent::AttemptEnded {
            blockers: blockers(),
            reason: "fixer rounds exhausted".into(),
            cause: AttemptEndCause::FixerRounds,
        },
    }
}

pub fn fixer_round(node: &str, round: u32) -> Event {
    Event::Node {
        node: node.into(),
        ev: NodeEvent::FixerRoundBegin {
            round,
            partition: None,
        },
    }
}
