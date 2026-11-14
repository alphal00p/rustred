//! Model-based random testing (port of audit B experiment 1, updated to the NEW interface): seeded random
//! sequences of guided (legal-life) and adversarial records go through a real ledger. The oracle is the governing
//! principle of interfaces/ledger-and-progress.md:
//! * a refused record leaves the files byte for byte and the state unchanged (typed `Refused`);
//! * after every accepted record the INVARIANTS hold (forbidden states unreachable): a closed / certified node is
//!   bound to a defect-0 closure on its current rule set and dispatch order with no open item and (certified)
//!   every live rule at the spec's level; every live acceptance from a job comes from a current-generation,
//!   non-stale attempt whose contract row accepts records, with that attempt's sealed table; counters equal a
//!   recount; the incremental rule-set digest equals the from-scratch one; counted terminals within the bound; a
//!   boxed target only with the spec box and never after a FULL closure;
//! * reopen, `reduce(read_records)` and `verify_chain` reproduce the state;
//! * liveness: the guided legal life of every item is accepted when nothing illegal intervenes (a clean seed
//!   closes and certifies its nodes).
//!
//! Default run: `cargo test --release -p rustred-campaign --test ledger_model -- --ignored --nocapture`
//! (env RR_MODEL_SEEDS, RR_MODEL_STEPS); a short smoke version runs in debug.
mod ledger_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ledger_support::*;
use rustred_campaign::ledger::{
    self, AcceptSource, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind, Event, ItemEvent,
    JobEvent, Ledger, LedgerConfig, LedgerError, NodeSpecValues, RetirementProof,
};
use rustred_campaign::state::{
    AcceptedFrom, AttemptEndCause, CampaignEvent, CampaignState, ClosureTarget, ContractAccept,
    EvidenceLevel, FinishClass, ItemPhase, NodeEvent, NodePhase, Rung, accept_permits_records,
    contract_accepts, reduce, rule_set_digest_of, FINISH_CLASSES,
};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        (self.next() % 10_000) as f64 / 10_000.0 < p
    }
}

/// Generic node names (genericity: nothing five-loop specific).
const NODES: [&str; 2] = ["alpha", "beta"];

fn setup() -> Vec<Event> {
    let mut v = vec![
        input("spec", "spec", "spec"),
        input("bin", "binary", "bin v1"),
        input("alpha.family", "family", "f-a"),
        input("beta.family", "family", "f-b"),
        input("alpha.box", "box", "box-a"),
    ];
    for (n, b) in [("alpha", Some("alpha.box")), ("beta", None)] {
        v.push(Event::NodeRegistered {
            node: n.into(),
            inputs: vec![format!("{n}.family")],
            terminal_bound: Some(6),
            spec_box: b.map(String::from),
            spec: NodeSpecValues {
                terminal_rung: Some(Rung(1)),
                full_attempt_fixer_rounds: Some(2),
                ..Default::default()
            },
            reference: None,
        });
    }
    v.push(Event::Campaign {
        ev: CampaignEvent::Start,
        reason: "model".into(),
    });
    v.push(Event::Campaign {
        ev: CampaignEvent::Run,
        reason: "model".into(),
    });
    for n in NODES {
        v.push(Event::Node {
            node: n.into(),
            ev: NodeEvent::Start,
        });
    }
    v
}

fn iid(k: usize) -> String {
    format!("it{k:04}")
}
fn node_of(k: usize) -> &'static str {
    NODES[k % 2]
}
fn jid(k: usize, g: usize) -> String {
    format!("jb{k:04}-{g}")
}
fn levels() -> [EvidenceLevel; 3] {
    EvidenceLevel::ALL
}

/// The next record of item `k`'s legal life given the current state (None: nothing to do).
fn guided(st: &CampaignState, r: &mut Rng, k: usize) -> Option<Event> {
    let id = iid(k);
    let n = node_of(k);
    // recover like a supervisor: resume a paused campaign / item, reopen a stale node, restart a finished campaign
    if st.paused || st.items.get(&id).is_some_and(|i| i.paused) {
        let cid = format!("resume-{}", st.last_seq);
        let target = if st.paused {
            ControlTarget::Campaign
        } else {
            ControlTarget::Item { item: id.clone() }
        };
        return Some(match st.controls.get(&cid) {
            None => Event::Control {
                id: cid,
                ev: ControlEvent::Requested {
                    verb: ControlVerb::Resume,
                    target,
                    by: "model".into(),
                    reason: "recover".into(),
                },
            },
            Some(_) => Event::Control {
                id: cid,
                ev: ControlEvent::Applied { effect: "resumed".into() },
            },
        });
    }
    if let Some((cid, _)) = st.controls.iter().find(|(c, x)| c.starts_with("resume-") && x.status == "requested") {
        return Some(Event::Control {
            id: cid.clone(),
            ev: ControlEvent::Applied { effect: "resumed".into() },
        });
    }
    if st.nodes[n].state.phase == NodePhase::Stale {
        return Some(Event::Node {
            node: n.into(),
            ev: NodeEvent::Reopen { reason: "recover".into() },
        });
    }
    let Some(it) = st.items.get(&id) else {
        return Some(Event::ItemCreated {
            item: id,
            node: n.into(),
            kind: "piece".into(),
            rung: Rung((r.below(3)) as u8),
            parent: None,
            inputs: vec![],
            label: format!("P{k}"),
            weight: 1,
        });
    };
    let job = it.jobs.last().cloned();
    let jr = job.as_ref().and_then(|j| st.jobs.get(j));
    let item = |ev| Some(Event::Item { item: id.clone(), ev });
    match &it.phase {
        ItemPhase::Open => {
            let fresh = jr.is_none_or(|j| j.generation != it.generation || j.stale.is_some());
            if fresh {
                let j = jid(k, it.jobs.len());
                return Some(Event::JobQueued {
                    job: j.clone(),
                    item: id,
                    rung: it.rung,
                    kind: "generate".into(),
                    spec_blake3: digest(&j),
                    binary: "bin".into(),
                    reserved_bytes: 1,
                    inputs: vec![],
                });
            }
            item(ItemEvent::Start { job })
        }
        ItemPhase::InProgress => {
            let j = jr?;
            let jn = job.clone().unwrap();
            let ev = |ev| {
                Some(Event::Job {
                    job: jn.clone(),
                    attempt: j.attempt,
                    ev,
                })
            };
            if j.generation != it.generation || j.stale.is_some() {
                return item(ItemEvent::Requeue {
                    reason: "stale job".into(),
                });
            }
            match j.state.tag() {
                "queued" => ev(JobEvent::Launch {
                    run_dir: format!("large:{jn}/a{}", j.attempt),
                    cpus: vec![1],
                    reserved_bytes: 1,
                }),
                "launching" => ev(JobEvent::Started {
                    pid: 100,
                    start_ticks: 1,
                    boot_id: "b".into(),
                }),
                "running" => {
                    if r.chance(0.08) {
                        return ev(JobEvent::Interrupt {
                            reason: "operator".into(),
                        });
                    }
                    let class = if r.chance(0.8) {
                        FinishClass::RuleFound
                    } else {
                        FINISH_CLASSES[r.below(FINISH_CLASSES.len())]
                    };
                    let acc = contract_accepts(class)[0];
                    let sealed = accept_permits_records(acc).then(|| table_ref(&jn, j.attempt));
                    ev(finished_with(&jn, j.attempt, class, acc, sealed, 1000))
                }
                "interrupted" | "lost" => Some(Event::Job {
                    job: jn,
                    attempt: j.attempt + 1,
                    ev: JobEvent::Requeue {
                        reason: "resume".into(),
                    },
                }),
                "failed" => Some(Event::Job {
                    job: jn,
                    attempt: j.attempt + 1,
                    ev: JobEvent::Retry {
                        reason: "retry".into(),
                    },
                }),
                _ => {
                    if j.accept.is_some_and(accept_permits_records) {
                        item(ItemEvent::Produce {
                            job: jn,
                            attempt: j.attempt,
                            table: j.sealed_table.clone().unwrap(),
                        })
                    } else if it.rung.0 < 3 {
                        item(ItemEvent::Escalate {
                            to: Rung(it.rung.0 + 1),
                            reason: "no rule".into(),
                        })
                    } else {
                        item(ItemEvent::ProposeTerminal {
                            reason: "ladder".into(),
                            preferred: false,
                            exhausted: Some(fref(&format!("large:ladder/{id}"))),
                        })
                    }
                }
            }
        }
        ItemPhase::Produced => {
            let p = it.produced.as_ref()?;
            let level = levels()[r.below(3)];
            item(ItemEvent::Accept {
                rule: format!("R-{id}"),
                level,
                record: fref(&format!("accepted/{id}-{}", it.generation)),
                source: AcceptSource::Job {
                    job: p.job.clone(),
                    attempt: p.attempt,
                    table_blake3: p.table.blake3.clone(),
                },
                elided: false,
                evidence: level_evidence(level, &id),
                round: None,
            })
        }
        ItemPhase::Accepted(l) if *l < EvidenceLevel::IdentityProved => item(ItemEvent::Upgrade {
            level: EvidenceLevel::IdentityProved,
            evidence: level_evidence(EvidenceLevel::IdentityProved, &id).unwrap(),
        }),
        ItemPhase::TerminalCandidate => item(ItemEvent::DeclareTerminal {
            count: 1,
            keys: fref(&format!("large:keys/{id}")),
            reason: "ladder".into(),
            preferred: it.terminal_preferred,
        }),
        ItemPhase::Stale | ItemPhase::Attention(_) => item(ItemEvent::Reopen {
            reason: "again".into(),
        }),
        ItemPhase::Rejected(_) => item(ItemEvent::Requeue {
            reason: "again".into(),
        }),
        ItemPhase::Skipped => None,
        _ => None,
    }
}

/// A node step on the CURRENT state (legal when the node is ready; otherwise refused, which is checked too).
fn node_step(st: &CampaignState, r: &mut Rng, n: &str) -> Event {
    let nr = &st.nodes[n];
    let node = |ev| Event::Node {
        node: n.into(),
        ev,
    };
    match r.below(10) {
        0 | 1 => node(NodeEvent::Assembled {
            round: nr.state.closure_rounds + 1,
            manifest: fref(&format!("large:{n}/manifest")),
            dispatch_order: fref(&format!("large:{n}/order{}", r.below(2))),
        }),
        2..=4 => Event::Node {
            node: n.into(),
            ev: NodeEvent::ClosureResult {
                round: nr.state.closure_rounds + 1,
                defect_keys: if r.chance(0.6) { 0 } else { 3 },
                closure_keys: 100,
                rules_used: None,
                rules_total: None,
                keyset: None,
                rule_set_digest: nr.rule_set.digest(),
                dispatch_order_digest: nr.state.dispatch_order.clone().unwrap_or_default(),
                envelope: if r.chance(0.2) {
                    EnvelopeKind::Measured
                } else {
                    EnvelopeKind::Proved
                },
                envelope_tables: vec![],
            },
        },
        5 => node(NodeEvent::DeclareClosed),
        6 => node(NodeEvent::DeclareCertified),
        7 => node(NodeEvent::FixerRoundBegin {
            round: nr.state.last_fixer_round + 1,
            partition: None,
        }),
        8 => node(NodeEvent::AttemptEnded {
            blockers: blockers(),
            reason: "budget".into(),
            cause: AttemptEndCause::FixerRounds,
        }),
        _ => match r.below(4) {
            0 => node(NodeEvent::FallBackToBox {
                box_id: format!("{n}.box"),
            }),
            1 => node(NodeEvent::Reopen {
                reason: "again".into(),
            }),
            2 => node(NodeEvent::DeclareOpen {
                reason: "x".into(),
            }),
            _ => node(NodeEvent::EnvelopeRecheck {
                passed: r.chance(0.5),
                report: fref("large:recheck"),
                envelope_tables: vec![],
            }),
        },
    }
}

/// An adversarial record: plausible ids, wrong context (old jobs, foreign tables, illegal transitions, ...).
fn adversarial(st: &CampaignState, r: &mut Rng, n_items: usize) -> Event {
    let k = r.below(n_items);
    let id = iid(k);
    let any_job = st.jobs.keys().nth(r.below(st.jobs.len().max(1))).cloned().unwrap_or_default();
    let item = |ev| Event::Item { item: id.clone(), ev };
    match r.below(16) {
        0 => item(ItemEvent::Produce {
            job: any_job.clone(),
            attempt: 1 + r.below(2) as u32,
            table: table_ref(&any_job, 1),
        }),
        1 => {
            let level = levels()[r.below(3)];
            item(ItemEvent::Accept {
                rule: format!("R-{id}"),
                level,
                record: fref("accepted/x"),
                source: AcceptSource::Job {
                    job: any_job.clone(),
                    attempt: 1,
                    table_blake3: table_ref(&any_job, 1).blake3,
                },
                elided: r.chance(0.2),
                evidence: if r.chance(0.5) {
                    level_evidence(level, "x")
                } else {
                    None
                },
                round: None,
            })
        }
        2 => item(ItemEvent::Reject {
            reason: "adversary".into(),
            disproof: None,
        }),
        3 => item(ItemEvent::Retire {
            rule: format!("R-{id}"),
            proof: if r.chance(0.5) {
                RetirementProof::BoxClosurePass { closure: fref("c") }
            } else {
                RetirementProof::ReplacedBy { proof: fref("p") }
            },
            superseded_by: Some(iid(r.below(n_items))),
        }),
        4 => item(ItemEvent::ProposeTerminal {
            reason: "x".into(),
            preferred: r.chance(0.3),
            exhausted: if r.chance(0.5) { Some(fref("e")) } else { None },
        }),
        5 => item(ItemEvent::DeclareTerminal {
            count: r.below(4) as u32,
            keys: fref("k"),
            reason: "x".into(),
            preferred: r.chance(0.5),
        }),
        6 => input("bin", "binary", &format!("bin v{}", 1 + r.below(3))),
        7 => input(
            &format!("{}.family", NODES[r.below(2)]),
            "family",
            &format!("f{}", r.below(3)),
        ),
        8 => Event::Job {
            job: any_job.clone(),
            attempt: 1 + r.below(3) as u32,
            ev: match r.below(4) {
                0 => JobEvent::Requeue { reason: "x".into() },
                1 => JobEvent::Retry { reason: "x".into() },
                2 => JobEvent::Launch {
                    run_dir: "r".into(),
                    cpus: vec![],
                    reserved_bytes: 1,
                },
                _ => {
                    let class = FINISH_CLASSES[r.below(FINISH_CLASSES.len())];
                    let acc = [ContractAccept::Canonical, ContractAccept::CommittedRules, ContractAccept::Nothing][r.below(3)];
                    finished_with(&any_job, 1, class, acc, Some(table_ref(&any_job, 1)), 5)
                }
            },
        },
        9 => item(ItemEvent::Attention {
            code: "needs_person".into(),
            text: "t".into(),
        }),
        10 => {
            let cid = format!("c{}", r.below(8));
            let ev = match st.controls.get(&cid) {
                None => ControlEvent::Requested {
                    verb: [ControlVerb::Retry, ControlVerb::Skip, ControlVerb::Pause, ControlVerb::Resume][r.below(4)],
                    target: if r.chance(0.7) {
                        ControlTarget::Item { item: id.clone() }
                    } else {
                        ControlTarget::Campaign
                    },
                    by: "op".into(),
                    reason: "r".into(),
                },
                Some(_) if r.chance(0.8) => ControlEvent::Applied { effect: "e".into() },
                Some(_) => ControlEvent::Refused { why: "w".into() },
            };
            Event::Control { id: cid, ev }
        }
        11 => item(ItemEvent::Reinstate { reason: "x".into() }),
        12 => Event::Node {
            node: NODES[r.below(2)].into(),
            ev: NodeEvent::MarkStale { reason: "x".into() },
        },
        13 => item(ItemEvent::Escalate {
            to: Rung(r.below(4) as u8),
            reason: "x".into(),
        }),
        14 => item(ItemEvent::Replace {
            replaces: vec![iid(r.below(n_items))],
            containment: fref("c"),
        }),
        _ => Event::Campaign {
            ev: CampaignEvent::Finish,
            reason: "x".into(),
        },
    }
}

fn fingerprint(d: &Path) -> BTreeMap<String, Vec<u8>> {
    let ld = ledger::ledger_dir(d);
    let mut m = BTreeMap::new();
    for e in fs::read_dir(&ld).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            m.insert(
                p.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(&p).unwrap(),
            );
        }
    }
    m
}

/// The invariants of the governing principle (forbidden states unreachable).
fn check_invariants(st: &CampaignState, ctx: &str) {
    for (nid, n) in &st.nodes {
        let c = n.context();
        // counters equal a recount
        let mut by_phase = [0u64; rustred_campaign::state::N_ITEM_PHASES];
        let mut by_level = [0u64; 3];
        let mut counted = 0u32;
        for it in st.items.values().filter(|i| &i.node == nid) {
            by_phase[it.phase.index()] += 1;
            if let ItemPhase::Accepted(l) = it.phase {
                by_level[l as usize] += 1;
            }
            if it.phase == ItemPhase::Terminal && it.terminal_counted {
                counted += it.terminal_keys;
            }
        }
        assert_eq!(by_phase, n.items_by_phase, "{ctx}: {nid} phase counters");
        assert_eq!(by_level, n.accepted_by_level, "{ctx}: {nid} level counters");
        assert_eq!(counted, n.state.terminals, "{ctx}: {nid} counted terminals");
        assert!(n.state.terminals <= n.state.terminal_bound, "{ctx}: {nid} bound");
        // incremental rule-set digest == from scratch
        let live = st.node_live_rules(nid);
        let scratch = rule_set_digest_of(
            live.iter()
                .map(|(i, r, b, x)| (i.as_str(), r.as_str(), b.as_str(), x.as_slice())),
        );
        assert_eq!(scratch, c.rule_set_digest, "{ctx}: {nid} rule-set digest");
        // derived verdicts
        if matches!(n.state.phase, NodePhase::Closed | NodePhase::Certified) {
            let b = n.state.closure.as_ref().unwrap_or_else(|| panic!("{ctx}: {nid} closed without closure"));
            assert_eq!(b.defect_keys, 0, "{ctx}: {nid}");
            assert_eq!(b.rule_set_digest, c.rule_set_digest, "{ctx}: {nid} closed on an older rule set");
            assert_eq!(n.state.dispatch_order.as_deref(), Some(b.dispatch_order_digest.as_str()), "{ctx}: {nid}");
            assert_eq!(c.items_open, 0, "{ctx}: {nid} closed with open items");
        }
        if n.state.phase == NodePhase::Certified {
            let need = n.state.required_cert().required_level();
            assert!(
                st.items.values().filter(|i| &i.node == nid).all(|i| match i.phase {
                    ItemPhase::Accepted(l) => l >= need,
                    _ => true,
                }),
                "{ctx}: {nid} certified below its level"
            );
        }
        if let ClosureTarget::Boxed { box_id } = &n.state.target {
            assert_eq!(n.state.spec_box.as_ref(), Some(box_id), "{ctx}: {nid} boxed with another box");
            assert!(!n.state.full_closed_ever, "{ctx}: {nid} boxed after a FULL closure");
        }
    }
    for (id, it) in &st.items {
        if let (ItemPhase::Accepted(_), Some(a)) = (&it.phase, &it.accepted)
            && let AcceptedFrom::Job { job, attempt, table_blake3 } = &a.source
        {
            let j = &st.jobs[job];
            assert_eq!(&j.item, id, "{ctx}: {id} accepted from a foreign job");
            assert_eq!(j.attempt, *attempt, "{ctx}: {id}");
            assert_eq!(j.generation, it.generation, "{ctx}: {id} accepted from an old generation");
            assert!(j.stale.is_none(), "{ctx}: {id} accepted from a stale job");
            assert!(j.accept.is_some_and(accept_permits_records), "{ctx}: {id} from a job accepting nothing");
            assert_eq!(j.sealed_table.as_ref().map(|t| &t.blake3), Some(table_blake3), "{ctx}: {id} table");
        }
    }
}

fn run_seed(seed: u64, steps: usize, adversarial_share: f64, rotate: u64) -> (u64, u64) {
    let d = scratch(&format!("model-{seed}"));
    let _c = Cleanup(d.clone());
    let clk = clock();
    let cfg = LedgerConfig {
        rotate_bytes: rotate,
        ..LedgerConfig::default()
    };
    let mut l = new_ledger(&d, as_dyn(&clk), cfg.clone());
    l.append_all(setup()).unwrap();
    let mut r = Rng(seed);
    let n_items = 24;
    let (mut ok, mut refused) = (0u64, 0u64);
    for step in 0..steps {
        let st = l.state().clone();
        let ev = if r.chance(adversarial_share) {
            adversarial(&st, &mut r, n_items)
        } else if r.chance(0.2) {
            let n = NODES[r.below(2)];
            node_step(&st, &mut r, n)
        } else {
            let k = r.below(n_items);
            match guided(&st, &mut r, k) {
                Some(e) => e,
                None => node_step(&st, &mut r, node_of(k)),
            }
        };
        let fp = fingerprint(&d);
        let digest_before = st.digest();
        let ctx = format!("seed {seed} step {step} ({})", ev.kind());
        match l.append(ev.clone()) {
            Ok(_) => {
                ok += 1;
                check_invariants(l.state(), &ctx);
            }
            Err(LedgerError::Refused(_)) => {
                refused += 1;
                assert_eq!(fingerprint(&d), fp, "{ctx}: a refusal changed the files");
                assert_eq!(l.state().digest(), digest_before, "{ctx}: a refusal changed the state");
            }
            Err(e) => panic!("{ctx}: {e} for {ev:?}"),
        }
        if step % 97 == 96 {
            let want = l.state().digest();
            drop(l);
            let (l2, _) = open_ledger(&d, as_dyn(&clk), cfg.clone()).unwrap();
            assert_eq!(l2.state().digest(), want, "seed {seed} step {step}: reopen");
            let recs = ledger::read_records(&d).unwrap();
            assert_eq!(reduce(&recs).unwrap().digest(), want, "seed {seed}: replay");
            assert_eq!(ledger::verify_chain(&d).unwrap().state.digest(), want, "seed {seed}: verify_chain");
            l = l2;
        }
    }
    (ok, refused)
}

/// Liveness: guided lives only (no adversary): every node gets its rules accepted and can close and certify.
fn clean_run(seed: u64) {
    let d = scratch(&format!("model-clean-{seed}"));
    let _c = Cleanup(d.clone());
    let clk = clock();
    let mut l: Ledger = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup()).unwrap();
    let mut r = Rng(seed);
    for k in 0..8 {
        while let Some(ev) = guided(l.state(), &mut r, k) {
            if l.append(ev.clone()).is_err() {
                panic!("seed {seed}: guided legal record refused: {ev:?}");
            }
            if matches!(l.state().items[&iid(k)].phase, ItemPhase::Accepted(EvidenceLevel::IdentityProved) | ItemPhase::Terminal) {
                break;
            }
        }
    }
    for n in NODES {
        let st = l.state().clone();
        l.append(assembled(n, 1)).unwrap();
        l.append(closure_event(&st, n, 1, 0, EnvelopeKind::Proved)).unwrap();
        l.append(Event::Node { node: n.into(), ev: NodeEvent::DeclareClosed }).unwrap();
        l.append(Event::Node { node: n.into(), ev: NodeEvent::DeclareCertified }).unwrap();
        assert_eq!(l.state().nodes[n].state.phase, NodePhase::Certified, "seed {seed} {n}");
    }
}

#[test]
fn model_smoke() {
    for seed in 0..3 {
        clean_run(seed);
        let (ok, refused) = run_seed(seed, 400, 0.3, 4096);
        eprintln!("model smoke seed {seed}: {ok} accepted, {refused} refused");
        assert!(ok > 150 && refused > 20, "seed {seed}: {ok} ok, {refused} refused");
    }
}

#[test]
#[ignore = "long: release model-based run (audit B experiment 1 port)"]
fn model_random_release() {
    let seeds: u64 = std::env::var("RR_MODEL_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let steps: usize = std::env::var("RR_MODEL_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(3000);
    let (mut ok, mut refused) = (0, 0);
    for seed in 0..seeds {
        clean_run(seed);
        let share = [0.15, 0.35, 0.6][(seed % 3) as usize];
        let rotate = [1500, 4096, 20_000, 64 << 20][(seed % 4) as usize];
        let (a, b) = run_seed(seed, steps, share, rotate);
        ok += a;
        refused += b;
    }
    eprintln!("model: {seeds} seeds x {steps} steps: {ok} accepted, {refused} refused; invariants held after every step");
}
