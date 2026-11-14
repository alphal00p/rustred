//! WP-0a / WP-0b (lane int): (1) the FIXTURE CAMPAIGN DIRECTORIES of `crates/rustred-campaign-core/schemas/fixtures/
//! cdir-{empty,mid,finished,stale}` are generated here with the AS-BUILT ledger writer (`Ledger`, a `ManualClock`, so the
//! hash chain and the timestamps are real and deterministic) and the AS-BUILT progress derivation (`progress::derive`);
//! regenerating gives byte-identical files (test `fixtures_are_deterministic_and_committed`; refresh deliberately with
//! `RR_UPDATE_FIXTURES=1`). Hand-written documents beside the ledger (accept records, terminals, control requests, job
//! records, registry) carry `"fixture_origin": "hand"`. (2) Every as-built struct of this crate round-trips the schema
//! examples and the fixtures byte for byte after canonicalisation: ledger records, manifest, snapshot, lock owner, site
//! policy. (3) `examples/ledger-record.jsonl` (one line per record kind and sub-event the as-built ledger accepts) is
//! regenerated here too.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign::clock::{Clock, ManualClock};
use rustred_campaign::cpu_policy::SiteConfig;
use rustred_campaign::ledger::{
    self, AcceptSource, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind, Event, ExportEvent,
    FileRef, Genesis, ItemEvent, JobEvent, Ledger, LedgerConfig, LevelEvidence, Manifest,
    NodeSpecValues, OutcomeBinding, Record, RetirementProof, RuleEconomy, Snapshot,
};
use rustred_campaign::lock::{self, CampaignLock, DeadReason, OwnerRecord, ProcessIdentity};
use rustred_campaign::progress::{self, Resources};
use rustred_campaign::state::{
    AttemptEndCause, Blockers, CampaignEvent, CertLevel, ContractAccept, EvidenceLevel,
    FinishClass, NodeEvent, Rung,
};
use rustred_campaign_core::schema::canon::canonical_json;
use rustred_campaign_core::schema::spec::CampaignSpec;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn schemas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rustred-campaign-core/schemas")
}

fn b3(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn h(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

fn pretty(v: &impl Serialize) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).unwrap();
    b.push(b'\n');
    b
}

/// The fixture being built: files in memory (relative path -> bytes), written at the end.
struct Fx {
    root: PathBuf,
    files: BTreeMap<String, Vec<u8>>,
}

impl Fx {
    fn put(&mut self, rel: &str, bytes: Vec<u8>) -> FileRef {
        let r = FileRef {
            path: rel.into(),
            bytes: bytes.len() as u64,
            blake3: b3(&bytes),
        };
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, &bytes).unwrap();
        self.files.insert(rel.into(), bytes);
        r
    }
}

fn large(path: &str) -> FileRef {
    FileRef {
        path: format!("large:{path}"),
        bytes: 1000 + path.len() as u64,
        blake3: b3(path.as_bytes()),
    }
}

/// The spec of a fixture: the schema example with this fixture's id and nodes.
/// `nodes` = (name, has a box, certification, arity): names and arities are DATA (P-5: one fixture uses an invented
/// non-five-loop family set).
fn spec_for(id: &str, nodes: &[(&str, bool, &str, usize)]) -> CampaignSpec {
    let mut v: Value =
        serde_json::from_slice(&fs::read(schemas().join("examples/campaign-spec.json")).unwrap())
            .unwrap();
    v["campaign"] = json!(id);
    v["dirs"]["campaign_dir"] = json!(format!("/abs/campaigns/{id}"));
    v["dirs"]["large_root"] = json!(format!("/common/build-cache/campaigns/{id}"));
    let template = v["nodes"][0].clone();
    let mut list = Vec::new();
    for (n, has_box, cert, arity) in nodes {
        let mut t = template.clone();
        let s = serde_json::to_string(&t).unwrap().replace("T162", n);
        t = serde_json::from_str(&s).unwrap();
        t["derived"] = json!({"arity": arity, "lines": arity / 2 + 1});
        t["preferred_terminals"] = json!([{"key": vec![1; *arity]}]);
        if !has_box {
            t["box"] = Value::Null;
        }
        t["certification"] = json!(cert);
        list.push(t);
    }
    v["nodes"] = Value::Array(list);
    v["resources"]["node_priority"] = json!(nodes.iter().map(|n| n.0).collect::<Vec<_>>());
    let s: CampaignSpec = serde_json::from_value(v).unwrap();
    s.check().unwrap();
    s
}

/// Ledger events registering the spec's inputs and nodes.
fn register(spec: &CampaignSpec, spec_ref: &FileRef) -> Vec<Event> {
    let mut v = vec![Event::InputRegistered {
        input: "spec".into(),
        role: "spec".into(),
        path: spec_ref.path.clone(),
        bytes: spec_ref.bytes,
        blake3: spec_ref.blake3.clone(),
    }];
    for (name, p) in &spec.programs {
        v.push(Event::InputRegistered {
            input: format!("program:{name}"),
            role: "binary".into(),
            path: p.path.clone(),
            bytes: 9_000_000,
            blake3: p.blake3.clone(),
        });
    }
    for n in &spec.nodes {
        let mut inputs = vec![n.family.id.clone()];
        v.push(Event::InputRegistered {
            input: n.family.id.clone(),
            role: "family".into(),
            path: n.family.path.clone(),
            bytes: 3000,
            blake3: n.family.blake3.clone(),
        });
        for t in &n.tables {
            inputs.push(t.id.clone());
            v.push(Event::InputRegistered {
                input: t.id.clone(),
                role: "table".into(),
                path: t.path.clone(),
                bytes: 77_000_000,
                blake3: t.blake3.clone(),
            });
        }
        if let Some(b) = &n.r#box {
            v.push(Event::InputRegistered {
                input: b.id.clone(),
                role: "box".into(),
                path: b
                    .path
                    .clone()
                    .unwrap_or_else(|| format!("spec.json#/nodes/{}/box", n.id)),
                bytes: 900,
                blake3: b.blake3.clone(),
            });
        }
        let cert = match serde_json::to_value(n.certification).unwrap().as_str() {
            Some("replay") => CertLevel::Replay,
            _ => CertLevel::PerKey,
        };
        v.push(Event::NodeRegistered {
            node: n.id.clone(),
            inputs,
            terminal_bound: n.terminals.bound,
            spec_box: n.r#box.as_ref().map(|b| b.id.clone()),
            spec: NodeSpecValues {
                certify_full: Some(cert),
                certify_boxed: Some(cert),
                full_attempt_fixer_rounds: Some(1),
                terminal_rung: Some(Rung(1)),
                ..Default::default()
            },
            reference: None,
        });
    }
    v
}

fn camp(ev: CampaignEvent, reason: &str) -> Event {
    Event::Campaign {
        ev,
        reason: reason.into(),
    }
}

fn node(n: &str, ev: NodeEvent) -> Event {
    Event::Node { node: n.into(), ev }
}

fn item(i: &str, ev: ItemEvent) -> Event {
    Event::Item { item: i.into(), ev }
}

fn job(j: &str, attempt: u32, ev: JobEvent) -> Event {
    Event::Job {
        job: j.into(),
        attempt,
        ev,
    }
}

fn created(n: &str, i: &str, kind: &str, rung: u8, parent: Option<&str>, label: &str) -> Event {
    Event::ItemCreated {
        item: i.into(),
        node: n.into(),
        kind: kind.into(),
        rung: Rung(rung),
        parent: parent.map(String::from),
        inputs: vec![format!("{n}.family")],
        label: label.into(),
        weight: 3,
    }
}

/// The contract's accept column for the fixture's classes (records only with a seal and the OUTCOME binding).
fn finished(
    j: &str,
    attempt: u32,
    class: FinishClass,
    exit: i32,
    sealed: Option<FileRef>,
    cpu_ms: u64,
    time_decided: bool,
) -> JobEvent {
    let accept = match (class, &sealed) {
        (FinishClass::RuleFound | FinishClass::Complete, Some(_)) => ContractAccept::Canonical,
        (FinishClass::NoRule | FinishClass::Spill, Some(_)) => ContractAccept::CommittedRules,
        _ => ContractAccept::Nothing,
    };
    let records = accept != ContractAccept::Nothing;
    JobEvent::Finished {
        class,
        accept,
        exit_code: Some(exit),
        signal: None,
        sealed_output: sealed,
        outcome: records.then(|| large(&format!("jobs/{j}/a{attempt}/OUTCOME.json"))),
        binding: records.then(|| OutcomeBinding {
            job: j.into(),
            attempt,
            spec_blake3: b3(j.as_bytes()),
        }),
        cpu_ms,
        wall_ms: cpu_ms + 50,
        peak_rss_bytes: 400 << 20,
        time_decided,
    }
}

/// Queue + start + launch + started of `j` for item `i` (a running job when nothing follows).
fn launch(l: &mut Ledger, i: &str, j: &str, rung: u8, cpu: u32) {
    l.append_all(vec![
        Event::JobQueued {
            job: j.into(),
            item: i.into(),
            rung: Rung(rung),
            kind: "generate".into(),
            spec_blake3: b3(j.as_bytes()),
            binary: "program:rr_node".into(),
            reserved_bytes: 2 << 30,
            inputs: vec![],
        },
        item(
            i,
            ItemEvent::Start {
                job: Some(j.into()),
            },
        ),
        job(
            j,
            1,
            JobEvent::Launch {
                run_dir: format!("large:jobs/{j}/a1"),
                cpus: vec![cpu],
                reserved_bytes: 2 << 30,
                spec_blake3: b3(j.as_bytes()),
                scope_unit: None,
            },
        ),
        job(
            j,
            1,
            JobEvent::Started {
                pid: 20_000 + cpu,
                start_ticks: 4_000_000 + cpu as u64,
                boot_id: "6b1f0c55-0000-4000-8000-000000000000".into(),
            },
        ),
    ])
    .unwrap();
}

/// The accept record file of an accepted rule (hand-made shape: no as-built writer of accept records yet).
fn accept_record(node: &str, rule: &str, table: &FileRef, level: &str, k: u64) -> Vec<u8> {
    let mut v: Value =
        serde_json::from_slice(&fs::read(schemas().join("examples/accept.json")).unwrap()).unwrap();
    v["rule"]["id"] = json!(rule);
    v["rule"]["node"] = json!(node);
    v["rule"]["table_blake3"] = json!(table.blake3);
    v["rule"]["rule_line_blake3"] = json!(b3(rule.as_bytes()));
    v["rule"]["ordinal"] = json!(k);
    v["level_after"] = json!(level);
    v["evidence_level"] = json!(rustred_campaign_core::schema::accept::evidence_word(level));
    v["seq_ref"] = Value::Null;
    v["credits"]["via_symmetry"] = json!([]);
    v["fixture_origin"] = json!("hand");
    pretty(&v)
}

fn level_word(l: EvidenceLevel) -> &'static str {
    match l {
        EvidenceLevel::GeometryAccepted => "A1",
        EvidenceLevel::IdentityTested => "A2",
        EvidenceLevel::IdentityProved => "A3",
    }
}

/// One item's life to an accepted rule at `level`: created, job, sealed rule_found, produce, accept record written
/// (intent / file / done), accept, economy.
#[allow(clippy::too_many_arguments)]
fn accepted_item(
    l: &mut Ledger,
    fx: &mut Fx,
    n: &str,
    k: u64,
    rung: u8,
    level: EvidenceLevel,
    cpu_ms: u64,
    rhs_terms: u64,
) -> String {
    let i = format!("{n}:I{k:04}");
    let j = format!("{n}-J{k:04}-L{rung}");
    l.append(created(n, &i, "face", rung, None, &format!("F{k:03}")))
        .unwrap();
    launch(l, &i, &j, rung, 20 + k as u32);
    let table = large(&format!("jobs/{j}/a1/out/F.table.jsonl"));
    let rule = format!("{n}:R:campaign:{k}");
    l.append_all(vec![
        job(
            &j,
            1,
            finished(
                &j,
                1,
                FinishClass::RuleFound,
                10,
                Some(table.clone()),
                cpu_ms,
                false,
            ),
        ),
        item(
            &i,
            ItemEvent::Produce {
                job: j.clone(),
                attempt: 1,
                table: table.clone(),
            },
        ),
    ])
    .unwrap();
    let op = format!("op-accept-{j}");
    let rel = format!("accepted/{n}/{k:04}.json");
    l.append(Event::Intent {
        op: op.clone(),
        action: "write_accepted_record".into(),
        target: rel.clone(),
    })
    .unwrap();
    let rec = fx.put(&rel, accept_record(n, &rule, &table, level_word(level), k));
    l.append_all(vec![
        Event::Done {
            op,
            result: Some(rec.clone()),
        },
        item(
            &i,
            ItemEvent::Accept {
                rule,
                level,
                record: rec,
                source: AcceptSource::Job {
                    job: j,
                    attempt: 1,
                    table_blake3: table.blake3,
                },
                elided: false,
                evidence: match level {
                    EvidenceLevel::GeometryAccepted => None,
                    EvidenceLevel::IdentityTested => Some(LevelEvidence::IdentityTested {
                        verdict: large(&format!("evidence/{n}-{k}.replay.json")),
                    }),
                    EvidenceLevel::IdentityProved => Some(LevelEvidence::IdentityProved {
                        proofs: large(&format!("evidence/{n}-{k}.proofs.jsonl")),
                        keys: large(&format!("evidence/{n}-{k}.keys")),
                    }),
                },
                round: Some(1),
            },
        ),
        Event::RuleEconomy {
            item: i.clone(),
            economy: RuleEconomy {
                cases: 1,
                rules: 1,
                rhs_terms,
                monomials: rhs_terms * 7,
                coefficient_degree: 12,
                bytes: rhs_terms * 120,
            },
        },
    ])
    .unwrap();
    i
}

/// A rejected item at L0 escalated to an L1 child that ends as a terminal candidate (proposed, optionally declared).
fn rejected_then_terminal(l: &mut Ledger, n: &str, k: u64, declare: bool) {
    let i = format!("{n}:I{k:04}");
    let j = format!("{n}-J{k:04}-L0");
    l.append(created(n, &i, "locus", 0, None, &format!("F{k:03} n9=-1")))
        .unwrap();
    launch(l, &i, &j, 0, 40 + k as u32);
    let t = large(&format!("jobs/{j}/a1/out/F.table.jsonl"));
    let c = format!("{n}:I{:04}", k + 1);
    l.append_all(vec![
        job(
            &j,
            1,
            finished(&j, 1, FinishClass::NoRule, 10, Some(t.clone()), 2500, true),
        ),
        item(
            &i,
            ItemEvent::Produce {
                job: j,
                attempt: 1,
                table: t,
            },
        ),
        item(
            &i,
            ItemEvent::Reject {
                reason: "rule_covers_none".into(),
                disproof: None,
            },
        ),
        item(
            &i,
            ItemEvent::Escalate {
                to: Rung(1),
                reason: "rule_covers_none".into(),
            },
        ),
        created(n, &c, "locus", 1, Some(&i), &format!("F{k:03} n9=-1")),
        item(
            &c,
            ItemEvent::ProposeTerminal {
                reason: "terminal_after_ladder".into(),
                preferred: false,
                exhausted: Some(large(&format!("ladder/{c}.exhausted.json"))),
            },
        ),
    ])
    .unwrap();
    if declare {
        l.append(item(
            &c,
            ItemEvent::DeclareTerminal {
                count: 2,
                keys: large(&format!("keysets/{n}-terminals.keys")),
                reason: "terminal_after_ladder".into(),
                preferred: false,
            },
        ))
        .unwrap();
    }
}

/// The table set assembled (dispatch order fixed) and a closure pass on the node's CURRENT rule set.
fn closure(l: &Ledger, n: &str, round: u32, defect: u64) -> Vec<Event> {
    let order = large(&format!("tables/{n}.order.json"));
    vec![
        node(
            n,
            NodeEvent::Assembled {
                round,
                manifest: large(&format!("tables/{n}.r{round}.manifest.json")),
                dispatch_order: order.clone(),
            },
        ),
        node(
            n,
            NodeEvent::ClosureResult {
                round,
                defect_keys: defect,
                closure_keys: 150_000 + defect,
                rules_used: Some((l.state().node_live_rules(n).len() as u64).min(5)),
                rules_total: Some(l.state().node_live_rules(n).len() as u64),
                keyset: Some(large(&format!("keysets/{n}-closure-{round}.keys"))),
                rule_set_digest: l.state().node_rule_set_digest(n).unwrap(),
                dispatch_order_digest: order.blake3,
                envelope: EnvelopeKind::Proved,
                envelope_tables: vec![],
            },
        ),
    ]
}

/// A node closed FULL (certified when every item is identity-proved).
fn full_node(l: &mut Ledger, fx: &mut Fx, n: &str, base: u64) {
    l.append(node(n, NodeEvent::Start)).unwrap();
    for k in 0..2 {
        accepted_item(
            l,
            fx,
            n,
            base + k,
            0,
            EvidenceLevel::IdentityProved,
            1000 * (k + 1),
            300,
        );
    }
    l.append_all(closure(l, n, 1, 0)).unwrap();
    l.append_all(vec![
        node(n, NodeEvent::DeclareClosed),
        node(n, NodeEvent::DeclareCertified),
    ])
    .unwrap();
}

/// A node with a spec box: the full attempt ends, falls back to the box, closes there with declared terminals; a
/// superseded rule (retire replaced_by) and a box-only pruning (box_closure_pass).
fn boxed_node(l: &mut Ledger, fx: &mut Fx, n: &str, base: u64) {
    l.append(node(n, NodeEvent::Start)).unwrap();
    let a = accepted_item(l, fx, n, base, 0, EvidenceLevel::IdentityProved, 3000, 2000);
    let b = accepted_item(
        l,
        fx,
        n,
        base + 1,
        1,
        EvidenceLevel::IdentityTested,
        4000,
        400,
    );
    let c = accepted_item(
        l,
        fx,
        n,
        base + 2,
        0,
        EvidenceLevel::GeometryAccepted,
        500,
        90,
    );
    rejected_then_terminal(l, n, base + 3, true);
    l.append_all(closure(l, n, 1, 37)).unwrap();
    l.append_all(vec![
        node(
            n,
            NodeEvent::FixerRoundBegin {
                round: 1,
                partition: Some(large(&format!("partition/{n}.r1.json"))),
            },
        ),
        node(
            n,
            NodeEvent::AttemptEnded {
                blockers: Blockers {
                    faces: 2,
                    loci: 1,
                    keys: 150,
                    list: Some(large(&format!("blockers/{n}-full.keys"))),
                },
                reason: "full attempt: the spec's fixer rounds without closure (U-6)".into(),
                cause: AttemptEndCause::FixerRounds,
            },
        ),
        node(
            n,
            NodeEvent::FallBackToBox {
                box_id: format!("{n}.box"),
            },
        ),
        item(
            &a,
            ItemEvent::Retire {
                rule: format!("{n}:R:campaign:{base}"),
                proof: RetirementProof::ReplacedBy {
                    proof: large(&format!("proofs/{n}-containment.json")),
                },
                superseded_by: Some(b.clone()),
            },
        ),
        item(
            &c,
            ItemEvent::Retire {
                rule: format!("{n}:R:campaign:{}", base + 2),
                proof: RetirementProof::BoxClosurePass {
                    closure: large(&format!("keysets/{n}-closure-2.keys")),
                },
                superseded_by: None,
            },
        ),
    ])
    .unwrap();
    l.append_all(closure(l, n, 2, 0)).unwrap();
    l.append(node(n, NodeEvent::DeclareClosed)).unwrap();
}

fn resources(sampled: &str, live: u32) -> Resources {
    Resources {
        cpus_in_use: Some(live * 2),
        cpus_allowed: Some(70),
        live_jobs: Some(live),
        reserved_bytes: Some(live as u64 * (2 << 30)),
        measured_rss_bytes: Some(live as u64 * 450_000_000),
        mem_cap_bytes: Some(200_000_000_000),
        host_mem_available_bytes: Some(287_762_808_832),
        host_floor_bytes: Some(50_000_000_000),
        sampled_utc: Some(sampled.into()),
        ..Default::default()
    }
}

/// Derive the progress files of the state with the as-built derivation and add them to the fixture.
fn write_progress(fx: &mut Fx, l: &Ledger, res: &Resources, written: &str) {
    let set = progress::derive(l.state(), l.head(), res, written);
    for (rel, v) in set {
        fx.put(&format!("progress/{rel}"), pretty(&v));
    }
}

fn read_ledger_files(fx: &mut Fx) {
    let ld = fx.root.join("ledger");
    let mut names: Vec<String> = fs::read_dir(&ld)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for n in names {
        let b = fs::read(ld.join(&n)).unwrap();
        fx.files.insert(format!("ledger/{n}"), b);
    }
}

fn owner(pid: u32) -> OwnerRecord {
    OwnerRecord {
        schema: "rustred.campaign.lock.v1".into(),
        identity: ProcessIdentity {
            pid,
            start_ticks: 123_456_789,
            boot_id: "6b1f0c55-0000-4000-8000-000000000000".into(),
            host: "itphlies".into(),
        },
        argv: vec![
            "rr_campaign".into(),
            "run".into(),
            "--spec".into(),
            "spec.json".into(),
        ],
        started_utc: "2026-11-14T09:00:00.000Z".into(),
        token: "0123456789abcdef0123456789abcdef".into(),
    }
}

fn hand_doc(name: &str, edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut v: Value =
        serde_json::from_slice(&fs::read(schemas().join("examples").join(name)).unwrap()).unwrap();
    edit(&mut v);
    v["fixture_origin"] = json!("hand");
    pretty(&v)
}

fn start_ledger(
    fx: &mut Fx,
    id: &str,
    nodes: &[(&str, bool, &str, usize)],
    t0: &str,
) -> (Ledger, Arc<ManualClock>, CampaignLock) {
    let spec = spec_for(id, nodes);
    let sref = fx.put("spec.json", pretty(&spec));
    let clk = Arc::new(ManualClock::at(t0).with_tick(250));
    let c: Arc<dyn Clock> = clk.clone();
    let (lk, _) = lock::acquire(&fx.root, clk.as_ref(), &["schema-fixture".into()]).unwrap();
    let mut l = Ledger::create(
        &fx.root,
        &lk,
        c,
        LedgerConfig::default(),
        Genesis::new(id, &sref.blake3, spec.terminal_bound),
    )
    .unwrap();
    l.append_all(register(&spec, &sref)).unwrap();
    (l, clk, lk)
}

/// Builds one fixture under `root` and returns its files.
fn build(name: &str, root: &Path) -> BTreeMap<String, Vec<u8>> {
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root).unwrap();
    let mut fx = Fx {
        root: root.to_path_buf(),
        files: BTreeMap::new(),
    };
    match name {
        "cdir-empty" => {
            let (l, clk, _lk) = start_ledger(
                &mut fx,
                "fixture-empty",
                &[("T162", true, "per-key", 20)],
                "2026-11-14T08:00:00Z",
            );
            let t = clk.now_utc().to_iso();
            write_progress(&mut fx, &l, &Resources::default(), &t);
        }
        "cdir-mid" => {
            let nodes = [
                ("T164", false, "replay", 20),
                ("T162", true, "per-key", 20),
                ("T178", true, "per-key", 20),
            ];
            let (mut l, clk, _lk) =
                start_ledger(&mut fx, "fixture-mid", &nodes, "2026-11-14T11:58:00Z");
            l.append_all(vec![
                camp(CampaignEvent::Start, "rr_campaign run"),
                camp(CampaignEvent::Run, "preflight ok"),
            ])
            .unwrap();
            full_node(&mut l, &mut fx, "T164", 0);
            l.rotate().unwrap();
            boxed_node(&mut l, &mut fx, "T162", 0);
            // T178: OPEN, full attempt still running: a rejected item escalated to a terminal candidate (attention), one
            // accepted rule, two running jobs.
            l.append(node("T178", NodeEvent::Start)).unwrap();
            accepted_item(
                &mut l,
                &mut fx,
                "T178",
                0,
                0,
                EvidenceLevel::GeometryAccepted,
                800,
                150,
            );
            rejected_then_terminal(&mut l, "T178", 1, false);
            for k in [3u64, 4] {
                let i = format!("T178:I{k:04}");
                l.append(created("T178", &i, "face", 0, None, &format!("F{k:03}")))
                    .unwrap();
                launch(&mut l, &i, &format!("T178-J{k:04}-L0"), 0, 60 + k as u32);
            }
            let cl = closure(&l, "T178", 1, 409);
            l.append_all(cl).unwrap();
            fx.put("LOCK.json", pretty(&owner(4242)));
            fx.put(
                "control/20261114T115950Z-7f3a.json",
                hand_doc("control.json", |v| {
                    v["id"] = json!("c-20261114T115950Z-7f3a");
                    v["utc"] = json!("2026-11-14T11:59:50Z");
                    v["verb"] = json!("pause");
                    v["target"] = Value::Null;
                    v["args"] = json!({});
                    v["reason"] = json!("operator pause before the host maintenance");
                }),
            );
            fx.put(
                "terminals/T162.json",
                hand_doc("terminals.json", |v| {
                    v["campaign_id"] = json!("fixture-mid");
                    v["seq"] = json!(l.seq());
                }),
            );
            fx.put(
                "jobs/T164-J0000-L0.json",
                hand_doc("campaign-job.json", |v| {
                    v["job"] = json!("T164-J0000-L0");
                    v["item"] = json!("T164:I0000");
                    v["node"] = json!("T164");
                }),
            );
            let t = clk.now_utc().to_iso();
            write_progress(&mut fx, &l, &resources(&t, 2), &t);
        }
        "cdir-finished" => {
            // GENERICITY (P-5): an invented toy family set (arities 5 and 9), no five-loop name or number.
            let nodes = [
                ("tri-a", false, "replay", 5),
                ("kite.b9", true, "per-key", 9),
            ];
            let (mut l, clk, _lk) =
                start_ledger(&mut fx, "fixture-finished", &nodes, "2026-11-14T06:00:00Z");
            l.append_all(vec![
                camp(CampaignEvent::Start, "rr_campaign run"),
                camp(CampaignEvent::Run, "preflight ok"),
            ])
            .unwrap();
            full_node(&mut l, &mut fx, "tri-a", 0);
            boxed_node(&mut l, &mut fx, "kite.b9", 0);
            l.append(camp(CampaignEvent::Finish, "every node closed"))
                .unwrap();
            fx.put(
                "control/done/20261114T061000Z-0001.json",
                hand_doc("control.json", |v| {
                    v["id"] = json!("c-20261114T061000Z-0001");
                    v["utc"] = json!("2026-11-14T06:10:00Z");
                    v["verb"] = json!("note");
                    v["target"] = Value::Null;
                    v["args"] = json!({"text": "fixture"});
                    v["reason"] = json!("operator note");
                }),
            );
            fx.put(
                "terminals/kite.b9.json",
                hand_doc("terminals.json", |v| {
                    v["campaign_id"] = json!("fixture-finished");
                    v["seq"] = json!(l.seq());
                    let n = &mut v["nodes"][0];
                    n["id"] = json!("kite.b9");
                    n["box_id"] = json!("kite.b9.box");
                    n["graph"] = Value::Null;
                    n["keys_in_box"] = json!(812);
                    n["keys_covered"] = json!(811);
                    n["table"] = Value::Null;
                    n["reference"] = Value::Null;
                    let t = &mut n["terminals"][0];
                    t["id"] = json!("kite.b9:top");
                    t["key"] = json!([1, 1, 1, 1, 1, 0, 0, 0, 0]);
                    t["presented_key"] = Value::Null;
                    t["label"] = json!("top sector");
                    t["item"] = json!("kite.b9:I0004");
                    t["drawing"] = json!({"present_lines": [0, 1, 2, 3, 4], "pinched_lines": [], "dots": {}, "numerators": []});
                }),
            );
            let t = clk.now_utc().to_iso();
            write_progress(&mut fx, &l, &resources(&t, 0), &t);
        }
        "cdir-stale" => {
            let (mut l, clk, _lk) = start_ledger(
                &mut fx,
                "fixture-stale",
                &[("T162", true, "per-key", 20)],
                "2026-11-14T10:00:00Z",
            );
            l.append_all(vec![
                camp(CampaignEvent::Start, "rr_campaign run"),
                camp(CampaignEvent::Run, "preflight ok"),
            ])
            .unwrap();
            l.append(node("T162", NodeEvent::Start)).unwrap();
            accepted_item(
                &mut l,
                &mut fx,
                "T162",
                0,
                0,
                EvidenceLevel::IdentityTested,
                1500,
                300,
            );
            l.append(created("T162", "T162:I0001", "face", 0, None, "F001"))
                .unwrap();
            launch(&mut l, "T162:I0001", "T162-J0001-L0", 0, 7);
            // the supervisor died here (no drain / stop record): heartbeat and written_utc stay at 10:00
            fx.put("LOCK.json", pretty(&owner(31337)));
            let t = clk.now_utc().to_iso();
            write_progress(&mut fx, &l, &resources(&t, 1), &t);
        }
        other => panic!("unknown fixture {other}"),
    }
    read_ledger_files(&mut fx);
    fx.files
}

const FIXTURES: [&str; 4] = ["cdir-empty", "cdir-mid", "cdir-finished", "cdir-stale"];

/// `<one TMPDIR-rooted directory unique to this test process>/<tag>` (lane int5: never the pid alone); the same tag
/// gives the same path within the process (the tests remove and recreate their own scratch directories by tag).
fn scratch(tag: &str) -> PathBuf {
    static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    ROOT.get_or_init(|| unique_temp_dir("rr-schema")).join(tag)
}

fn committed(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = fs::read_dir(d) else { return };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn fixtures_are_deterministic_and_committed() {
    let update = std::env::var_os("RR_UPDATE_FIXTURES").is_some();
    for name in FIXTURES {
        let a = build(name, &scratch(&format!("{name}-a")));
        let b = build(name, &scratch(&format!("{name}-b")));
        assert_eq!(a, b, "{name}: two generations differ (not deterministic)");
        let _ = fs::remove_dir_all(scratch(&format!("{name}-b")));
        let dest = schemas().join("fixtures").join(name);
        if update {
            let _ = fs::remove_dir_all(&dest);
            for (rel, bytes) in &a {
                let p = dest.join(rel);
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(&p, bytes).unwrap();
            }
        }
        let c = committed(&dest);
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            c.keys().collect::<Vec<_>>(),
            "{name}: file list differs from the committed fixture (RR_UPDATE_FIXTURES=1 regenerates)"
        );
        for (rel, bytes) in &a {
            assert!(
                c[rel] == *bytes,
                "{name}/{rel} differs from the committed fixture (RR_UPDATE_FIXTURES=1)"
            );
        }
        // the as-built verifier agrees with the progress files and the chain
        let rep = progress::verify(&dest).unwrap();
        assert!(
            rep.clean(),
            "{name}: progress::verify differences {:?}",
            rep.differences
        );
        let _ = fs::remove_dir_all(scratch(&format!("{name}-a")));
    }
    let reg = hand_doc("registry.json", |v| {
        v["campaigns"] = Value::Array(
            FIXTURES
                .iter()
                .map(|n| {
                    json!({"id": format!("fixture-{}", &n[5..]), "dir": n, "kind": "test", "mode": "close",
                           "started_utc": "2026-11-14T06:00:00Z", "owner": "fixture", "optimised_from": null})
                })
                .collect(),
        );
    });
    let p = schemas().join("fixtures/INDEX.json");
    if update {
        fs::write(&p, &reg).unwrap();
    }
    assert_eq!(
        fs::read(&p).unwrap(),
        reg,
        "fixtures/INDEX.json differs (RR_UPDATE_FIXTURES=1)"
    );
}

/// examples/ledger-record.jsonl: one line per record kind and sub-event that the as-built ledger accepts, written by the
/// as-built writer (the fixtures' records plus a catalogue ledger for the kinds the fixtures do not use).
#[test]
fn ledger_record_examples_cover_every_kind() {
    let d = scratch("catalogue");
    let _ = fs::remove_dir_all(&d);
    let mut recs: Vec<Record> = Vec::new();
    for name in FIXTURES {
        recs.extend(ledger::read_records(&schemas().join("fixtures").join(name)).unwrap());
    }
    // catalogue: the kinds no fixture contains
    let clk = Arc::new(ManualClock::at("2026-11-14T05:00:00Z").with_tick(250));
    let c: Arc<dyn Clock> = clk.clone();
    fs::create_dir_all(&d).unwrap();
    let (lk, _) = lock::acquire(&d, clk.as_ref(), &["catalogue".into()]).unwrap();
    let mut l = Ledger::create(
        &d,
        &lk,
        c,
        LedgerConfig::default(),
        Genesis::new("catalogue", &h('1'), 100),
    )
    .unwrap();
    let spec = spec_for("catalogue", &[("T162", true, "per-key", 20)]);
    let sref = FileRef {
        path: "spec.json".into(),
        bytes: 1,
        blake3: h('1'),
    };
    l.append_all(register(&spec, &sref)).unwrap();
    let id = owner(4242).identity;
    l.append_all(vec![
        Event::SupervisorStarted {
            identity: id,
            argv: vec!["rr_campaign".into(), "run".into()],
            version: "0.1.0".into(),
            lock_token: "0123456789abcdef0123456789abcdef".into(),
        },
        Event::LockTakeover {
            previous: owner(31337),
            reason: DeadReason::PidAbsent,
            orphans: vec![31338],
        },
        Event::Recovery {
            segment: 1,
            last_good_seq: 9,
            truncated_bytes: 37,
            saved_as: Some("ledger/torn-000000000010-1794632400000.bin".into()),
            detail: "torn tail".into(),
            forced: false,
        },
        Event::EvidenceImported {
            evidence: "astra-bank".into(),
            producer: "Astra".into(),
            description: "T162 bank".into(),
            files: vec![large("handoff/t162-bank/MANIFEST.json")],
            level_claimed: EvidenceLevel::IdentityTested,
            producer_blake3: Some(h('7')),
            provenance_class: None,
        },
        Event::TablesImported {
            provenance: "earlier".into(),
            producer: "campaign c0".into(),
            source_campaign: Some("c0".into()),
            manifest: large("c0/MANIFEST.json"),
            files: vec![],
            level_claimed: EvidenceLevel::IdentityTested,
        },
        camp(CampaignEvent::Start, "run"),
        camp(CampaignEvent::Run, "ok"),
        node("T162", NodeEvent::Start),
        created("T162", "T162:I0000", "face", 0, None, "F000"),
        item(
            "T162:I0000",
            ItemEvent::Requeue {
                reason: "operator retry".into(),
            },
        ),
        Event::JobQueued {
            job: "j0".into(),
            item: "T162:I0000".into(),
            rung: Rung(0),
            kind: "generate".into(),
            spec_blake3: h('2'),
            binary: "program:rr_node".into(),
            reserved_bytes: 1 << 30,
            inputs: vec![],
        },
        job(
            "j0",
            1,
            JobEvent::Cancel {
                reason: "skip".into(),
            },
        ),
        Event::JobQueued {
            job: "j1".into(),
            item: "T162:I0000".into(),
            rung: Rung(0),
            kind: "generate".into(),
            spec_blake3: h('3'),
            binary: "program:rr_node".into(),
            reserved_bytes: 1 << 30,
            inputs: vec![],
        },
        job(
            "j1",
            1,
            JobEvent::Launch {
                run_dir: "large:jobs/j1/a1".into(),
                cpus: vec![3],
                reserved_bytes: 1 << 30,
                spec_blake3: h('3'),
                scope_unit: None,
            },
        ),
        job(
            "j1",
            1,
            JobEvent::Lose {
                reason: "host reboot".into(),
            },
        ),
        job(
            "j1",
            2,
            JobEvent::Requeue {
                reason: "resume".into(),
            },
        ),
        job(
            "j1",
            2,
            JobEvent::Launch {
                run_dir: "large:jobs/j1/a2".into(),
                cpus: vec![3],
                reserved_bytes: 1 << 30,
                spec_blake3: h('3'),
                scope_unit: None,
            },
        ),
        job(
            "j1",
            2,
            finished("j1", 2, FinishClass::Crashed, 70, None, 10, false),
        ),
        job(
            "j1",
            3,
            JobEvent::Retry {
                reason: "crashed once".into(),
            },
        ),
        Event::Note {
            text: "catalogue".into(),
        },
    ])
    .unwrap();
    let blk = Blockers {
        faces: 1,
        loci: 0,
        keys: 3,
        list: None,
    };
    let cl = closure(&l, "T162", 1, 3);
    l.append_all(cl).unwrap();
    l.append_all(vec![
        node(
            "T162",
            NodeEvent::FixerRoundBegin {
                round: 1,
                partition: None,
            },
        ),
        node(
            "T162",
            NodeEvent::WinnerDiffDone {
                round: 1,
                diff: large("diff/T162.r1.json"),
                failures: 0,
            },
        ),
        node(
            "T162",
            NodeEvent::AttemptEnded {
                blockers: blk.clone(),
                reason: "budget".into(),
                cause: AttemptEndCause::FixerRounds,
            },
        ),
        node(
            "T162",
            NodeEvent::FallBackToBox {
                box_id: "T162.box".into(),
            },
        ),
    ])
    .unwrap();
    let cl = closure(&l, "T162", 2, 2);
    l.append_all(cl).unwrap();
    l.append_all(vec![
        node(
            "T162",
            NodeEvent::FixerRoundBegin {
                round: 2,
                partition: None,
            },
        ),
        node(
            "T162",
            NodeEvent::AttemptEnded {
                blockers: blk,
                reason: "box budget".into(),
                cause: AttemptEndCause::FixerRounds,
            },
        ),
        node(
            "T162",
            NodeEvent::DeclareOpen {
                reason: "box attempt ended".into(),
            },
        ),
        node(
            "T162",
            NodeEvent::Reopen {
                reason: "new seeds".into(),
            },
        ),
        node(
            "T162",
            NodeEvent::MarkStale {
                reason: "family changed".into(),
            },
        ),
        item(
            "T162:I0000",
            ItemEvent::MarkStale {
                input: "T162.family".into(),
            },
        ),
        Event::Control {
            id: "c-1".into(),
            ev: ControlEvent::Requested {
                verb: ControlVerb::Pause,
                target: ControlTarget::Campaign,
                by: "operator".into(),
                reason: "maintenance".into(),
            },
        },
        Event::Control {
            id: "c-1".into(),
            ev: ControlEvent::Applied {
                effect: "launches paused".into(),
            },
        },
        Event::Control {
            id: "c-2".into(),
            ev: ControlEvent::Requested {
                verb: ControlVerb::Resume,
                target: ControlTarget::Campaign,
                by: "operator".into(),
                reason: "done".into(),
            },
        },
        Event::Control {
            id: "c-2".into(),
            ev: ControlEvent::Refused {
                why: "supervisor stopping".into(),
            },
        },
        Event::OrphanKilled {
            job: Some("j1".into()),
            pid: 31338,
            start_ticks: 99,
        },
        Event::VerifyCheckpoint {
            seq: 3,
            clean: true,
            report: None,
        },
        Event::ArtefactLost {
            path: "accepted/T162/0000.json".into(),
            blake3: h('4'),
            items: vec![],
        },
        Event::Export {
            rev: "r1".into(),
            ev: ExportEvent::Revoked {
                cause: "superseded".into(),
            },
        },
    ])
    .unwrap();
    drop(l);
    drop(lk);
    recs.extend(ledger::read_records(&d).unwrap());
    let _ = fs::remove_dir_all(&d);
    // lane int5: the unconfirmed-launch records (a launch journalled in session 1, never confirmed; session 2 marks it
    // `unconfirmed`, then the proven `not_spawned` and the requeue), in a second small ledger.
    let d2 = scratch("catalogue-unconfirmed");
    let _ = fs::remove_dir_all(&d2);
    fs::create_dir_all(&d2).unwrap();
    let clk2 = Arc::new(ManualClock::at("2026-11-14T06:00:00Z").with_tick(250));
    let c2: Arc<dyn Clock> = clk2.clone();
    let (lk2, _) = lock::acquire(&d2, clk2.as_ref(), &["catalogue".into()]).unwrap();
    let mut l2 = Ledger::create(
        &d2,
        &lk2,
        c2,
        LedgerConfig::default(),
        Genesis::new("catalogue", &h('1'), 100),
    )
    .unwrap();
    l2.append_all(register(&spec, &sref)).unwrap();
    l2.append_all(vec![
        camp(CampaignEvent::Start, "supervisor session"),
        camp(CampaignEvent::Run, "scheduling"),
        node("T162", NodeEvent::Start),
        created("T162", "T162:I0000", "face", 0, None, "F000"),
        Event::JobQueued {
            job: "j1".into(),
            item: "T162:I0000".into(),
            rung: Rung(0),
            kind: "generate".into(),
            spec_blake3: h('3'),
            binary: "program:rr_node".into(),
            reserved_bytes: 1 << 30,
            inputs: vec![],
        },
        item(
            "T162:I0000",
            ItemEvent::Start {
                job: Some("j1".into()),
            },
        ),
        job(
            "j1",
            1,
            JobEvent::Launch {
                run_dir: "large:jobs/j1/a1".into(),
                cpus: vec![3],
                reserved_bytes: 1 << 30,
                spec_blake3: h('4'),
                scope_unit: None,
            },
        ),
        camp(CampaignEvent::Start, "supervisor session"),
        job(
            "j1",
            1,
            JobEvent::Unconfirmed {
                reason: "launch journalled in session 1, never confirmed by job.started before that supervisor ended"
                    .into(),
            },
        ),
        job(
            "j1",
            1,
            JobEvent::NotSpawned {
                reason: "authenticated run directory large:jobs/j1/a1 has no RUN/sup/SPAWN; no marked process of j1:a1"
                    .into(),
            },
        ),
        job(
            "j1",
            2,
            JobEvent::Requeue {
                reason: "spawn provably never happened".into(),
            },
        ),
    ])
    .unwrap();
    drop(l2);
    drop(lk2);
    recs.extend(ledger::read_records(&d2).unwrap());
    let _ = fs::remove_dir_all(&d2);
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for r in &recs {
        let v = serde_json::to_value(r).unwrap();
        let mut key = r.event.kind().to_string();
        if let Some(ev) = v["event"]["body"]["ev"]
            .get("ev")
            .or(v["event"]["body"].get("ev"))
            && matches!(key.as_str(), "item" | "job" | "node" | "campaign")
        {
            {
                key = format!("{key}.{}", ev.as_str().unwrap_or(""));
            }
        }
        seen.entry(key)
            .or_insert_with(|| String::from_utf8(ledger::encode_record(r).0).unwrap());
    }
    let text: String = seen.values().map(|l| format!("{l}\n")).collect();
    let p = schemas().join("examples/ledger-record.jsonl");
    if std::env::var_os("RR_UPDATE_FIXTURES").is_some() {
        fs::write(&p, &text).unwrap();
    }
    assert_eq!(
        fs::read_to_string(&p).unwrap(),
        text,
        "examples/ledger-record.jsonl differs (RR_UPDATE_FIXTURES=1)"
    );
    println!(
        "ledger record kinds / sub-events exampled: {} ({:?})",
        seen.len(),
        seen.keys().collect::<Vec<_>>()
    );
}

fn roundtrip<T: DeserializeOwned + Serialize>(text: &[u8], what: &str) {
    let original: Value = serde_json::from_slice(text).unwrap();
    let typed: T = serde_json::from_slice(text).unwrap_or_else(|e| panic!("{what}: {e}"));
    let back = serde_json::to_value(&typed).unwrap();
    assert_eq!(
        canonical_json(&original, true).unwrap(),
        canonical_json(&back, true).unwrap(),
        "{what} does not round-trip"
    );
}

/// Every as-built struct of this crate round-trips the examples and the fixtures; ledger lines also byte for byte
/// WITHOUT canonicalisation (the writer's field order is the canonical bytes of the hash chain).
#[test]
fn as_built_structs_roundtrip_examples_and_fixtures() {
    let e = schemas().join("examples");
    let mut lines = 0;
    let mut texts: Vec<(String, Vec<u8>)> = Vec::new();
    for l in fs::read(e.join("ledger-record.jsonl"))
        .unwrap()
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
    {
        texts.push(("examples/ledger-record.jsonl".into(), l.to_vec()));
    }
    for name in FIXTURES {
        let dir = schemas().join("fixtures").join(name).join("ledger");
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        for p in names {
            let fname = p.file_name().unwrap().to_string_lossy().into_owned();
            let b = fs::read(&p).unwrap();
            if fname.starts_with("segment-") {
                for l in b.split(|c| *c == b'\n').filter(|l| !l.is_empty()) {
                    texts.push((format!("{name}/{fname}"), l.to_vec()));
                }
            } else if fname == "MANIFEST.json" {
                roundtrip::<Manifest>(&b, &format!("{name}/{fname}"));
            } else if fname.starts_with("snapshot-") {
                roundtrip::<Snapshot>(&b, &format!("{name}/{fname}"));
            }
        }
        let lock = schemas().join("fixtures").join(name).join("LOCK.json");
        if lock.exists() {
            roundtrip::<OwnerRecord>(&fs::read(&lock).unwrap(), &format!("{name}/LOCK.json"));
        }
    }
    for (what, l) in &texts {
        let (r, _) = ledger::decode_record(l).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(
            ledger::encode_record(&r).0,
            *l,
            "{what}: ledger line does not re-serialise byte for byte"
        );
        let _: Record = r;
        lines += 1;
    }
    roundtrip::<Manifest>(
        &fs::read(e.join("ledger-manifest.json")).unwrap(),
        "examples/ledger-manifest.json",
    );
    roundtrip::<Snapshot>(
        &fs::read(e.join("ledger-snapshot.json")).unwrap(),
        "examples/ledger-snapshot.json",
    );
    roundtrip::<OwnerRecord>(
        &fs::read(e.join("lock.json")).unwrap(),
        "examples/lock.json",
    );
    roundtrip::<SiteConfig>(
        &fs::read(e.join("site-policy.json")).unwrap(),
        "examples/site-policy.json",
    );
    SiteConfig::read(&e.join("site-policy.json")).unwrap();
    println!("ledger lines round-tripped byte for byte: {lines}");
}

/// The coordinator's converted bank accept lines (coord-bank-tables.md), validated by REFERENCE (2,448 lines are not
/// copied into the repository; 3 representative lines are in `examples/accept-bank-import.jsonl`). Ignored: it reads
/// a workspace file; skips with a reason when the path is absent. `RR_BANK_ACCEPT` overrides the path.
#[test]
#[ignore = "reads the coordinator's bank conversion output (WS/SCRATCHPAD/bank-conv); run with --ignored"]
fn coordinator_bank_accept_lines_validate() {
    let path = std::env::var("RR_BANK_ACCEPT").unwrap_or_else(|_| {
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/bank-conv/out/20261114/ACCEPT/T162.bank.accept.jsonl"
            .into()
    });
    if !Path::new(&path).exists() {
        println!(
            "SKIPPED: {path} absent (the coordinator's bank conversion output is not on this host)"
        );
        return;
    }
    let o = std::process::Command::new("python3")
        .arg("-I")
        .arg("validate.py")
        .arg("--schema")
        .arg("accept")
        .arg(&path)
        .current_dir(schemas())
        .output()
        .expect("python3");
    let text =
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    let errors: Vec<&str> = text.lines().filter(|l| l.starts_with("error:")).collect();
    let only_class = !errors.is_empty()
        && errors
            .iter()
            .all(|l| l.ends_with("/rule/producer: missing required field 'provenance'"));
    assert!(
        o.status.success(),
        "{} of the coordinator's lines invalid{}:\n{}",
        errors.len(),
        if only_class {
            " ONLY because rule.producer.provenance is missing (null bin_blake3 needs a provenance class, e.g. \
             \"unknown: historical import\"): the converter must write it"
        } else {
            ""
        },
        errors
            .iter()
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A NEW directory `<TMPDIR>/<prefix>-<token>` unique on the host (lane int5, procfix3 item 0): never derived from the
/// pid alone (inside the private pid namespaces of rule L10 every test binary is pid 1, and two lanes may share a temp
/// root), and created with `create_dir` (a directory it did not create is never reused or removed).
#[allow(dead_code)]
pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let base = std::env::temp_dir();
    std::fs::create_dir_all(&base).unwrap();
    let ns = std::fs::read_link("/proc/self/ns/pid")
        .map(|l| l.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    loop {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let token = blake3::hash(format!("{ns}|{}|{stat}|{t}|{n}", std::process::id()).as_bytes())
            .to_hex()[..12]
            .to_string();
        let p = base.join(format!("{prefix}-{token}"));
        match std::fs::create_dir(&p) {
            Ok(()) => return p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("{}: {e}", p.display()),
        }
    }
}
