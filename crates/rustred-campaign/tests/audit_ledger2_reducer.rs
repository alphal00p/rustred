//! Re-audit of the ledger (lane audit-ledger2, independent): the reducer as a pure function of the committed
//! records. A guided random generator (own PRNG) produces record sequences with deliberate perturbations;
//! after every step a NAIVE model recomputes the derived quantities from the item / job maps; refusals must
//! leave the state unchanged; snapshot round trips, chunked replay and from-scratch replay must agree.
//! Deterministic attack tests: stale verdicts versus replays of older evidence.
//!
//! `RR_A2_SEEDS=200 RR_A2_STEPS=3000 cargo test --release --test audit_ledger2_reducer -- --nocapture`
mod ledger_support;

use std::collections::{BTreeMap, BTreeSet};

use ledger_support::*;
use rustred_campaign::ledger::{
    self, AcceptSource, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind, Event, ItemEvent,
    JobEvent, OutcomeBinding, Record, RetirementProof,
};
use rustred_campaign::state::{
    self, AttemptEndCause, AttemptState, CampaignState, ClosureTarget, ContractAccept,
    EvidenceLevel, FinishClass, ItemPhase, NodeEvent, NodePhase, ReduceError, Rung,
};

// ------------------------------------------------------------------------------------------------------------
// harness
// ------------------------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        if v.is_empty() {
            None
        } else {
            Some(&v[self.below(v.len())])
        }
    }
}

struct Harness {
    st: CampaignState,
    recs: Vec<Record>,
    refused: u64,
}

impl Harness {
    fn new() -> Harness {
        let mut h = Harness {
            st: CampaignState::default(),
            recs: Vec::new(),
            refused: 0,
        };
        h.ok(genesis_event());
        h
    }
    fn record(&self, e: Event) -> Record {
        Record {
            schema: ledger::LEDGER_SCHEMA.into(),
            seq: self.recs.len() as u64 + 1,
            utc: format!(
                "2026-11-14T00:{:02}:{:02}.000Z",
                (self.recs.len() / 60) % 60,
                self.recs.len() % 60
            ),
            prev: ledger::GENESIS.into(),
            event: e,
        }
    }
    /// Applies; on refusal asserts the state is unchanged (byte for byte).
    fn try_(&mut self, e: Event) -> Result<(), ReduceError> {
        let r = self.record(e);
        let before = self.st.clone();
        match self.st.apply_record(&r) {
            Ok(()) => {
                self.recs.push(r);
                Ok(())
            }
            Err(err) => {
                assert!(
                    self.st == before,
                    "REFUSED record {} ({}) changed the state: {err}",
                    r.seq,
                    r.event.kind()
                );
                self.refused += 1;
                Err(err)
            }
        }
    }
    fn ok(&mut self, e: Event) {
        let k = e.kind();
        if let Err(err) = self.try_(e) {
            panic!("expected acceptance of {k}: {err}");
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// the naive model: everything derived, recomputed from scratch
// ------------------------------------------------------------------------------------------------------------

fn naive_check(st: &CampaignState, ctx: &str) {
    for (nid, n) in &st.nodes {
        let mut by_phase = [0u64; state::N_ITEM_PHASES];
        let mut by_level = [0u64; 3];
        let mut terminals = 0u32;
        let mut preferred = 0u32;
        let mut open = 0u64;
        for it in st.items.values().filter(|i| &i.node == nid) {
            by_phase[it.phase.index()] += 1;
            if let ItemPhase::Accepted(l) = it.phase {
                by_level[l as usize] += 1;
            }
            if it.phase == ItemPhase::Terminal {
                if it.terminal_counted {
                    terminals += it.terminal_keys;
                }
                if it.terminal_preferred {
                    preferred += it.terminal_keys;
                }
            }
            if it.phase.is_open() {
                open += 1;
            }
        }
        assert_eq!(
            n.items_by_phase, by_phase,
            "{ctx}: node {nid} items_by_phase"
        );
        assert_eq!(
            n.accepted_by_level, by_level,
            "{ctx}: node {nid} accepted_by_level"
        );
        assert_eq!(
            n.state.terminals, terminals,
            "{ctx}: node {nid} counted terminals"
        );
        assert_eq!(
            n.state.preferred_terminals, preferred,
            "{ctx}: node {nid} preferred terminals"
        );
        let live = st.node_live_rules(nid);
        let scratch = state::rule_set_digest_of(
            live.iter()
                .map(|(i, r, b, x)| (i.as_str(), r.as_str(), b.as_str(), x.as_slice())),
        );
        assert_eq!(
            n.rule_set.digest(),
            scratch,
            "{ctx}: node {nid} rule-set digest incremental != scratch"
        );
        assert_eq!(
            n.rule_set.count,
            live.len() as u64,
            "{ctx}: node {nid} rule count"
        );
        // verdict binding (5.3)
        if matches!(n.state.phase, NodePhase::Closed | NodePhase::Certified) {
            let c = n
                .state
                .closure
                .as_ref()
                .unwrap_or_else(|| panic!("{ctx}: {nid} closed without closure"));
            assert_eq!(c.defect_keys, 0, "{ctx}: {nid} closed with defect");
            assert_eq!(
                c.rule_set_digest, scratch,
                "{ctx}: {nid} closed on another rule set"
            );
            assert_eq!(
                n.state.dispatch_order.as_deref(),
                Some(c.dispatch_order_digest.as_str()),
                "{ctx}: {nid} dispatch"
            );
            assert_eq!(open, 0, "{ctx}: {nid} closed with open items");
        }
        if n.state.phase == NodePhase::Certified {
            let need = n.state.required_cert().required_level();
            for it in st.items.values().filter(|i| &i.node == nid) {
                if let ItemPhase::Accepted(l) = it.phase {
                    assert!(
                        l >= need,
                        "{ctx}: {nid} certified with a rule below the level"
                    );
                }
            }
        }
        if n.state.phase == NodePhase::Stale {
            for (iid, it) in st.items.iter().filter(|(_, i)| &i.node == nid) {
                assert!(
                    matches!(
                        it.phase,
                        ItemPhase::Stale
                            | ItemPhase::Escalated(_)
                            | ItemPhase::Rejected(_)
                            | ItemPhase::Skipped
                    ),
                    "{ctx}: item {iid} is {} on the stale node {nid}",
                    it.phase.tag()
                );
            }
        }
    }
    let total: u64 = st
        .items
        .values()
        .filter(|i| i.phase == ItemPhase::Terminal)
        .map(|i| i.terminal_keys as u64)
        .sum();
    assert_eq!(st.terminals_total, total, "{ctx}: terminals_total");
    for (iid, it) in &st.items {
        if it.phase == ItemPhase::Produced {
            let p = it
                .produced
                .as_ref()
                .unwrap_or_else(|| panic!("{ctx}: {iid} produced without result"));
            let j = &st.jobs[&p.job];
            assert_eq!(j.item, *iid, "{ctx}: produced from another item's job");
            assert_eq!(
                j.generation, it.generation,
                "{ctx}: {iid} produced from an old generation"
            );
            assert!(j.stale.is_none(), "{ctx}: {iid} produced from a stale job");
        }
        if let (ItemPhase::Accepted(_), Some(a)) = (&it.phase, &it.accepted)
            && let state::AcceptedFrom::Job { job, .. } = &a.source
        {
            {
                assert!(
                    st.jobs[job].stale.is_none(),
                    "{ctx}: {iid} accepted from the stale job {job}"
                );
            }
        }
        if it.phase == ItemPhase::Stale {
            assert!(it.stale_from.is_some(), "{ctx}: {iid} stale without cause");
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// guided random generator
// ------------------------------------------------------------------------------------------------------------

const NODES: [&str; 2] = ["N0", "N1"];
const ITEM_INPUTS: [&str; 2] = ["X0", "X1"];

fn setup(h: &mut Harness) {
    h.ok(input("rr_node", "binary", "rr_node v1"));
    for x in ITEM_INPUTS {
        h.ok(input(x, "seed_policy", &format!("{x} v1")));
    }
    for n in NODES {
        h.ok(input(&format!("{n}.family"), "family", "f v1"));
        h.ok(input(&format!("{n}.box"), "box", "box v1"));
        h.ok(Event::NodeRegistered {
            node: n.into(),
            inputs: vec![format!("{n}.family")],
            terminal_bound: Some(5),
            spec_box: Some(format!("{n}.box")),
            spec: ledger::NodeSpecValues {
                terminal_rung: Some(Rung(1)),
                full_attempt_fixer_rounds: Some(1),
                ..Default::default()
            },
            reference: None,
        });
    }
    h.ok(Event::TablesImported {
        provenance: "P0".into(),
        producer: "earlier campaign".into(),
        source_campaign: Some("c0".into()),
        manifest: fref("large:import/P0.manifest.json"),
        files: vec![fref("large:import/P0.tables")],
        level_claimed: EvidenceLevel::IdentityTested,
    });
    h.ok(Event::EvidenceImported {
        evidence: "E0".into(),
        producer: "hand".into(),
        description: "hand evidence".into(),
        files: vec![fref("large:hand/E0")],
        level_claimed: EvidenceLevel::IdentityProved,
        producer_blake3: Some(digest("producer")),
        provenance_class: None,
    });
    h.ok(Event::Campaign {
        ev: state::CampaignEvent::Start,
        reason: "t".into(),
    });
    h.ok(Event::Campaign {
        ev: state::CampaignEvent::Run,
        reason: "t".into(),
    });
}

fn finished_ev(rng: &mut Rng, job: &str, attempt: u32, launched: Option<&str>) -> JobEvent {
    let class = *rng.pick(&state::FINISH_CLASSES).unwrap();
    let allowed = state::contract_accepts(class);
    let accept = if rng.chance(90) {
        *rng.pick(allowed).unwrap()
    } else {
        ContractAccept::Canonical
    };
    let permits = state::accept_permits_records(accept);
    let table = if permits || rng.chance(20) {
        Some(table_ref(job, attempt))
    } else {
        None
    };
    let binding = if permits && rng.chance(92) {
        Some(OutcomeBinding {
            job: job.into(),
            attempt,
            spec_blake3: launched.unwrap_or("x").into(),
        })
    } else {
        None
    };
    JobEvent::Finished {
        class,
        accept,
        exit_code: Some(0),
        signal: None,
        sealed_output: table,
        outcome: Some(fref(&format!("large:jobs/{job}.a{attempt}/OUTCOME.json"))),
        binding,
        cpu_ms: rng.below(5000) as u64,
        wall_ms: 1,
        peak_rss_bytes: 1,
        time_decided: rng.chance(10),
    }
}

fn level(rng: &mut Rng) -> EvidenceLevel {
    *rng.pick(&[
        EvidenceLevel::GeometryAccepted,
        EvidenceLevel::IdentityTested,
        EvidenceLevel::IdentityProved,
    ])
    .unwrap()
}

fn random_event(h: &Harness, rng: &mut Rng, counter: &mut u64) -> Event {
    *counter += 1;
    let st = &h.st;
    let items: Vec<String> = st.items.keys().cloned().collect();
    let jobs: Vec<String> = st.jobs.keys().cloned().collect();
    match rng.below(100) {
        // inputs change (sometimes back to an earlier digest)
        0 => {
            let all = [
                "rr_node",
                "X0",
                "X1",
                "N0.family",
                "N1.family",
                "N0.box",
                "N1.box",
            ];
            let i = *rng.pick(&all).unwrap();
            let role = match i {
                "rr_node" => "binary",
                "X0" | "X1" => "seed_policy",
                x if x.ends_with(".box") => "box",
                _ => "family",
            };
            let content = format!(
                "{} v{}",
                if role == "binary" { "rr_node" } else { i },
                1 + rng.below(3)
            );
            let content = if i.ends_with(".family") || i.ends_with(".box") {
                format!(
                    "{} v{}",
                    if i.ends_with(".box") { "box" } else { "f" },
                    1 + rng.below(3)
                )
            } else {
                content
            };
            input(i, role, &content)
        }
        1..=7 => {
            let k = if rng.chance(85) {
                items.len()
            } else {
                rng.below(items.len() + 1)
            };
            let node = *rng.pick(&NODES).unwrap();
            let parent = if rng.chance(30) {
                rng.pick(&items).cloned()
            } else {
                None
            };
            let inputs = if rng.chance(40) {
                vec![rng.pick(&ITEM_INPUTS).unwrap().to_string()]
            } else {
                vec![]
            };
            Event::ItemCreated {
                item: format!("I{k:03}"),
                node: node.into(),
                kind: "face".into(),
                rung: Rung(rng.below(3) as u8),
                parent,
                inputs,
                label: format!("F{k}"),
                weight: 1,
            }
        }
        8..=15 if !items.is_empty() => {
            let it = rng.pick(&items).unwrap().clone();
            let job = format!("J{:04}", *counter);
            let inputs = if rng.chance(30) {
                vec![rng.pick(&ITEM_INPUTS).unwrap().to_string()]
            } else {
                vec![]
            };
            Event::JobQueued {
                job: job.clone(),
                item: it,
                rung: Rung(rng.below(3) as u8),
                kind: "generate".into(),
                spec_blake3: digest(&job),
                binary: "rr_node".into(),
                reserved_bytes: 1,
                inputs,
            }
        }
        16..=45 if !jobs.is_empty() => {
            let job = rng.pick(&jobs).unwrap().clone();
            let j = &st.jobs[&job];
            let mut attempt = j.attempt;
            if rng.chance(5) {
                attempt = attempt.saturating_sub(1).max(1);
            }
            let ev = match j.state {
                AttemptState::Queued => JobEvent::Launch {
                    run_dir: format!("large:jobs/{job}.a{attempt}"),
                    cpus: vec![1],
                    reserved_bytes: 1,
                    spec_blake3: digest(&format!("{job}.a{attempt}")),
                    scope_unit: None,
                },
                AttemptState::Launching => match rng.below(4) {
                    0 => JobEvent::Interrupt {
                        reason: "stop".into(),
                    },
                    1 => JobEvent::Lose {
                        reason: "gone".into(),
                    },
                    _ => JobEvent::Started {
                        pid: 7,
                        start_ticks: 9,
                        boot_id: "b".into(),
                    },
                },
                AttemptState::Running => match rng.below(6) {
                    0 => JobEvent::Interrupt {
                        reason: "stop".into(),
                    },
                    1 => JobEvent::Lose {
                        reason: "gone".into(),
                    },
                    _ => finished_ev(rng, &job, attempt, j.attempt_spec.as_deref()),
                },
                AttemptState::Interrupted | AttemptState::Lost => {
                    attempt = j.attempt + 1;
                    JobEvent::Requeue {
                        reason: "resume".into(),
                    }
                }
                AttemptState::Failed(_) => {
                    attempt = j.attempt + 1;
                    JobEvent::Retry {
                        reason: "retry".into(),
                    }
                }
                _ => {
                    if rng.chance(50) {
                        JobEvent::Cancel { reason: "x".into() }
                    } else {
                        finished_ev(rng, &job, attempt, j.attempt_spec.as_deref())
                    }
                }
            };
            Event::Job { job, attempt, ev }
        }
        46..=80 if !items.is_empty() => {
            let id = rng.pick(&items).unwrap().clone();
            let it = &st.items[&id];
            let cur_jobs: Vec<&String> = it
                .jobs
                .iter()
                .filter(|j| rng.chance(10) || st.jobs[*j].generation == it.generation)
                .collect();
            let ev = match &it.phase {
                ItemPhase::Open => match rng.below(5) {
                    0 => ItemEvent::ProposeTerminal {
                        reason: "r".into(),
                        preferred: rng.chance(50),
                        exhausted: rng.chance(50).then(|| fref("large:ladder")),
                    },
                    1 => ItemEvent::Import {
                        rule: format!("R-{id}-imp"),
                        level: level(rng),
                        record: fref(&format!("acc/{id}.imp")),
                        provenance: if rng.chance(85) {
                            "P0".into()
                        } else {
                            "E0".into()
                        },
                    },
                    _ => ItemEvent::Start {
                        job: cur_jobs.first().map(|s| s.to_string()),
                    },
                },
                ItemPhase::InProgress => match rng.below(8) {
                    0 => ItemEvent::Escalate {
                        to: Rung(it.rung.0 + 1 + rng.below(2) as u8),
                        reason: "x".into(),
                    },
                    1 => ItemEvent::Requeue { reason: "x".into() },
                    _ => {
                        let Some(j) = rng.pick(&cur_jobs) else {
                            return Event::Note {
                                text: "no job".into(),
                            };
                        };
                        let jr = &st.jobs[*j];
                        let mut table = jr
                            .sealed_table
                            .clone()
                            .unwrap_or_else(|| table_ref(j, jr.attempt));
                        if rng.chance(5) {
                            table = fref("large:other.table");
                        }
                        ItemEvent::Produce {
                            job: j.to_string(),
                            attempt: jr.attempt,
                            table,
                        }
                    }
                },
                ItemPhase::Produced => {
                    if rng.chance(20) {
                        ItemEvent::Reject {
                            reason: "nope".into(),
                            disproof: None,
                        }
                    } else {
                        let p = it.produced.clone().unwrap();
                        let l = level(rng);
                        let src = if rng.chance(85) {
                            AcceptSource::Job {
                                job: p.job.clone(),
                                attempt: p.attempt,
                                table_blake3: p.table.blake3.clone(),
                            }
                        } else {
                            AcceptSource::Evidence {
                                evidence: "E0".into(),
                            }
                        };
                        ItemEvent::Accept {
                            rule: format!("R-{id}-g{}", it.generation),
                            level: l,
                            record: fref(&format!("acc/{id}.g{}", it.generation)),
                            source: src,
                            elided: rng.chance(3),
                            evidence: if rng.chance(90) {
                                level_evidence(l, &id)
                            } else {
                                None
                            },
                            round: Some(1),
                        }
                    }
                }
                ItemPhase::Accepted(l) => match rng.below(7) {
                    0 => ItemEvent::Upgrade {
                        level: EvidenceLevel::IdentityProved,
                        evidence: level_evidence(EvidenceLevel::IdentityProved, &id).unwrap(),
                    },
                    1 => ItemEvent::Reject {
                        reason: "disproved".into(),
                        disproof: Some(fref("large:disproof")),
                    },
                    2 => ItemEvent::Retire {
                        rule: it
                            .accepted
                            .as_ref()
                            .map(|a| a.rule.clone())
                            .unwrap_or_default(),
                        proof: if rng.chance(50) {
                            RetirementProof::EmptyDomain {
                                proof: fref("large:p"),
                            }
                        } else {
                            RetirementProof::BoxClosurePass {
                                closure: fref("large:c"),
                            }
                        },
                        superseded_by: None,
                    },
                    3 => ItemEvent::ExcludeKeys {
                        keys: fref(&format!("large:ex/{}", rng.below(3))),
                        disproof: fref("large:d"),
                        reason: "keeps_terminal".into(),
                    },
                    4 => {
                        return Event::RuleEconomy {
                            item: id.clone(),
                            economy: ledger::RuleEconomy {
                                cases: 1,
                                rules: 1,
                                rhs_terms: 3,
                                monomials: 5,
                                coefficient_degree: 2,
                                bytes: 100,
                            },
                        };
                    }
                    _ => ItemEvent::Upgrade {
                        level: *l,
                        evidence: level_evidence(EvidenceLevel::IdentityTested, &id).unwrap(),
                    },
                },
                ItemPhase::Retired => ItemEvent::Reinstate { reason: "x".into() },
                ItemPhase::TerminalCandidate => {
                    if rng.chance(25) {
                        ItemEvent::Reopen { reason: "x".into() }
                    } else {
                        ItemEvent::DeclareTerminal {
                            count: 1 + rng.below(3) as u32,
                            keys: fref(&format!("large:term/{id}")),
                            reason: "t".into(),
                            preferred: it.terminal_preferred,
                        }
                    }
                }
                ItemPhase::Rejected(_) => match rng.below(3) {
                    0 => ItemEvent::Escalate {
                        to: Rung(it.rung.0 + 1),
                        reason: "x".into(),
                    },
                    1 => ItemEvent::Requeue { reason: "x".into() },
                    _ => ItemEvent::ProposeTerminal {
                        reason: "r".into(),
                        preferred: true,
                        exhausted: None,
                    },
                },
                ItemPhase::Attention(_) | ItemPhase::Stale | ItemPhase::Skipped => {
                    if rng.chance(50) {
                        let cid = format!("C{:04}", *counter);
                        return Event::Control {
                            id: cid,
                            ev: ControlEvent::Requested {
                                verb: if rng.chance(80) {
                                    ControlVerb::Retry
                                } else {
                                    ControlVerb::Skip
                                },
                                target: ControlTarget::Item { item: id.clone() },
                                by: "op".into(),
                                reason: "x".into(),
                            },
                        };
                    }
                    ItemEvent::Reopen { reason: "x".into() }
                }
                _ => ItemEvent::MarkStale { input: "x".into() },
            };
            let ev = if rng.chance(1) {
                ItemEvent::MarkStale {
                    input: "manual".into(),
                }
            } else if rng.chance(1) {
                ItemEvent::Attention {
                    code: "a".into(),
                    text: "t".into(),
                }
            } else {
                ev
            };
            Event::Item { item: id, ev }
        }
        81..=94 => {
            let node = rng.pick(&NODES).unwrap().to_string();
            let n = &st.nodes[&node];
            let round = n.state.last_fixer_round + 1;
            let ev = match rng.below(13) {
                0 => NodeEvent::Start,
                1 => NodeEvent::FixerRoundBegin {
                    round,
                    partition: None,
                },
                2 | 3 => NodeEvent::Assembled {
                    round,
                    manifest: fref("large:m"),
                    dispatch_order: fref(&format!("large:order{}", rng.below(2))),
                },
                4..=6 => NodeEvent::ClosureResult {
                    round: n.state.closure.as_ref().map(|c| c.round + 1).unwrap_or(1),
                    defect_keys: if rng.chance(60) { 0 } else { 5 },
                    closure_keys: 100,
                    rules_used: Some(0),
                    rules_total: Some(st.node_live_rules(&node).len() as u64),
                    keyset: None,
                    rule_set_digest: if rng.chance(90) {
                        n.rule_set.digest()
                    } else {
                        digest("other")
                    },
                    dispatch_order_digest: n.state.dispatch_order.clone().unwrap_or_default(),
                    envelope: if rng.chance(70) {
                        EnvelopeKind::Proved
                    } else {
                        EnvelopeKind::Measured
                    },
                    envelope_tables: vec![],
                },
                7 => NodeEvent::DeclareClosed,
                8 => NodeEvent::DeclareCertified,
                9 => NodeEvent::AttemptEnded {
                    blockers: blockers(),
                    reason: "x".into(),
                    cause: AttemptEndCause::FixerRounds,
                },
                10 => match &n.state.target {
                    ClosureTarget::Full => NodeEvent::FallBackToBox {
                        box_id: format!("{node}.box"),
                    },
                    _ => NodeEvent::DeclareOpen { reason: "x".into() },
                },
                11 => NodeEvent::Reopen { reason: "x".into() },
                _ => {
                    if rng.chance(5) {
                        NodeEvent::MarkStale {
                            reason: "dep".into(),
                        }
                    } else {
                        NodeEvent::EnvelopeRecheck {
                            passed: rng.chance(70),
                            report: fref("large:r"),
                            envelope_tables: vec![],
                        }
                    }
                }
            };
            Event::Node { node, ev }
        }
        95..=97 => {
            // apply / refuse an open control
            let open: Vec<String> = st
                .controls
                .iter()
                .filter(|(_, c)| c.status == "requested")
                .map(|(k, _)| k.clone())
                .collect();
            match rng.pick(&open) {
                Some(id) => Event::Control {
                    id: id.clone(),
                    ev: if rng.chance(85) {
                        ControlEvent::Applied {
                            effect: "done".into(),
                        }
                    } else {
                        ControlEvent::Refused { why: "no".into() }
                    },
                },
                None => Event::Note { text: "n".into() },
            }
        }
        _ => {
            let accepted: Vec<(&String, &state::ItemRec)> = st
                .items
                .iter()
                .filter(|(_, i)| i.accepted.is_some())
                .collect();
            match rng.pick(&accepted) {
                Some((iid, it)) if rng.chance(50) => Event::ArtefactLost {
                    path: it.accepted.as_ref().unwrap().record.path.clone(),
                    blake3: it.accepted.as_ref().unwrap().record.blake3.clone(),
                    items: vec![iid.to_string()],
                },
                _ => Event::Note { text: "n".into() },
            }
        }
    }
}

fn run_seed(seed: u64, steps: usize) -> (Harness, u64) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut h = Harness::new();
    setup(&mut h);
    let mut counter = 0;
    let mut stale_jobs: BTreeSet<String> = BTreeSet::new();
    let mut gens: BTreeMap<String, u32> = BTreeMap::new();
    let mut accepted = 0u64;
    for step in 0..steps {
        let e = random_event(&h, &mut rng, &mut counter);
        if h.try_(e).is_ok() {
            accepted += 1;
            let ctx = format!("seed {seed} step {step} seq {}", h.recs.len());
            naive_check(&h.st, &ctx);
            // stale is sticky, generations never decrease
            for (id, j) in &h.st.jobs {
                if j.stale.is_some() {
                    stale_jobs.insert(id.clone());
                } else {
                    assert!(
                        !stale_jobs.contains(id),
                        "{ctx}: job {id} lost its stale mark"
                    );
                }
            }
            for (id, it) in &h.st.items {
                let g = gens.entry(id.clone()).or_insert(it.generation);
                assert!(it.generation >= *g, "{ctx}: generation of {id} decreased");
                *g = it.generation;
            }
            if step % 97 == 0 {
                let back: CampaignState =
                    serde_json::from_slice(&serde_json::to_vec(&h.st).unwrap()).unwrap();
                assert!(back == h.st, "{ctx}: snapshot round trip differs");
            }
        }
    }
    // drive every non-stale node to closure, certification, then perturb a closed node (verdict binding)
    for node in NODES {
        if h.st.nodes[node].state.phase == NodePhase::Stale {
            let _ = h.try_(Event::Node {
                node: node.into(),
                ev: NodeEvent::Reopen {
                    reason: "drive".into(),
                },
            });
        }
        let ids: Vec<String> =
            h.st.items
                .iter()
                .filter(|(_, i)| i.node == node)
                .map(|(k, _)| k.clone())
                .collect();
        for id in &ids {
            for _ in 0..4 {
                let ph = h.st.items[id].phase.clone();
                let ev = match ph {
                    ItemPhase::Stale | ItemPhase::TerminalCandidate => Some(ItemEvent::Reopen {
                        reason: "drive".into(),
                    }),
                    ItemPhase::Open => Some(ItemEvent::Import {
                        rule: format!("R-{id}-drive"),
                        level: EvidenceLevel::IdentityTested,
                        record: fref(&format!("acc/{id}.drive")),
                        provenance: "P0".into(),
                    }),
                    ItemPhase::InProgress => Some(ItemEvent::Requeue {
                        reason: "drive".into(),
                    }),
                    ItemPhase::Produced => Some(ItemEvent::Reject {
                        reason: "drive".into(),
                        disproof: None,
                    }),
                    ItemPhase::Rejected(_) => Some(ItemEvent::Requeue {
                        reason: "drive".into(),
                    }),
                    ItemPhase::Attention(_) | ItemPhase::Skipped => {
                        let cid = format!("drive-{id}-{}", h.recs.len());
                        let _ = h.try_(Event::Control {
                            id: cid.clone(),
                            ev: ControlEvent::Requested {
                                verb: ControlVerb::Retry,
                                target: ControlTarget::Item { item: id.clone() },
                                by: "drive".into(),
                                reason: "x".into(),
                            },
                        });
                        let _ = h.try_(Event::Control {
                            id: cid,
                            ev: ControlEvent::Applied {
                                effect: "retry".into(),
                            },
                        });
                        None
                    }
                    _ => None,
                };
                if let Some(ev) = ev
                    && h.try_(Event::Item {
                        item: id.clone(),
                        ev,
                    })
                    .is_ok()
                {
                    naive_check(&h.st, &format!("seed {seed} drive {id}"));
                }
            }
        }
        let n = &h.st.nodes[node];
        let round = n.state.last_fixer_round + 1;
        let _ = h.try_(Event::Node {
            node: node.into(),
            ev: NodeEvent::Start,
        });
        let _ = h.try_(Event::Node {
            node: node.into(),
            ev: NodeEvent::FixerRoundBegin {
                round,
                partition: None,
            },
        });
        let _ = h.try_(Event::Node {
            node: node.into(),
            ev: NodeEvent::Assembled {
                round,
                manifest: fref("large:m"),
                dispatch_order: fref("large:order-drive"),
            },
        });
        let n = &h.st.nodes[node];
        let cr = NodeEvent::ClosureResult {
            round: n
                .state
                .closure
                .as_ref()
                .map(|c| c.round + 1)
                .unwrap_or(1)
                .max(round),
            defect_keys: 0,
            closure_keys: 100,
            rules_used: Some(0),
            rules_total: Some(h.st.node_live_rules(node).len() as u64),
            keyset: None,
            rule_set_digest: n.rule_set.digest(),
            dispatch_order_digest: n.state.dispatch_order.clone().unwrap_or_default(),
            envelope: EnvelopeKind::Proved,
            envelope_tables: vec![],
        };
        let r1 = h.try_(Event::Node {
            node: node.into(),
            ev: cr,
        });
        let r2 = h.try_(Event::Node {
            node: node.into(),
            ev: NodeEvent::DeclareClosed,
        });
        naive_check(&h.st, &format!("seed {seed} closed {node}"));
        if r1.is_ok() && r2.is_ok() {
            CLOSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let _ = h.try_(Event::Node {
                node: node.into(),
                ev: NodeEvent::DeclareCertified,
            });
            if h.st.nodes[node].state.phase == NodePhase::Certified {
                CERTIFIED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            naive_check(&h.st, &format!("seed {seed} certified {node}"));
            // perturb: reject one live rule -> the verdict must be withdrawn
            if let Some((id, ..)) = h.st.node_live_rules(node).first().cloned() {
                h.ok(Event::Item {
                    item: id,
                    ev: ItemEvent::Reject {
                        reason: "late disproof".into(),
                        disproof: Some(fref("large:dis")),
                    },
                });
                naive_check(&h.st, &format!("seed {seed} perturbed {node}"));
                assert!(
                    !matches!(
                        h.st.nodes[node].state.phase,
                        NodePhase::Closed | NodePhase::Certified
                    ),
                    "verdict survived a rejected rule"
                );
            }
        }
    }
    let ctx = format!("seed {seed} end");
    naive_check(&h.st, &ctx);
    (h, accepted)
}

static CLOSED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CERTIFIED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[test]
fn a2_random_records_vs_naive_model() {
    let seeds: u64 = std::env::var("RR_A2_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let steps: usize = std::env::var("RR_A2_STEPS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);
    let (mut acc, mut refd) = (0u64, 0u64);
    let mut phases: BTreeMap<String, u64> = BTreeMap::new();
    for seed in 1..=seeds {
        let (h, a) = run_seed(seed, steps);
        acc += a;
        refd += h.refused;
        // from-scratch replay = incremental; chunked replay through a serialised snapshot = incremental
        let scratch = state::reduce(h.recs.iter()).unwrap();
        assert!(scratch == h.st, "seed {seed}: from-scratch replay differs");
        let k = h.recs.len() / 2;
        let mut mid = state::reduce(h.recs[..k].iter()).unwrap();
        mid = serde_json::from_slice(&serde_json::to_vec(&mid).unwrap()).unwrap();
        for r in &h.recs[k..] {
            mid.apply_record(r).unwrap();
        }
        assert!(
            mid == h.st,
            "seed {seed}: replay through a snapshot differs"
        );
        assert_eq!(mid.digest(), h.st.digest());
        for it in h.st.items.values() {
            *phases.entry(it.phase.tag().to_string()).or_default() += 1;
        }
        for n in h.st.nodes.values() {
            *phases
                .entry(format!("node:{}", n.state.phase.tag()))
                .or_default() += 1;
        }
    }
    eprintln!(
        "REDUCER PROPERTY: {seeds} seeds x {steps} steps: {acc} accepted, {refd} refused (state unchanged on every refusal); nodes driven closed {} / certified {}; final phases {phases:?}",
        CLOSED.load(std::sync::atomic::Ordering::Relaxed),
        CERTIFIED.load(std::sync::atomic::Ordering::Relaxed)
    );
}

// ------------------------------------------------------------------------------------------------------------
// deterministic attacks: stale verdicts versus older evidence
// ------------------------------------------------------------------------------------------------------------

fn item_with_input(k: &str, node: &str, inp: &[&str]) -> Event {
    Event::ItemCreated {
        item: k.into(),
        node: node.into(),
        kind: "face".into(),
        rung: Rung(0),
        parent: None,
        inputs: inp.iter().map(|s| s.to_string()).collect(),
        label: k.into(),
        weight: 1,
    }
}

fn import(item: &str, prov: &str) -> Event {
    Event::Item {
        item: item.into(),
        ev: ItemEvent::Import {
            rule: format!("R-{item}"),
            level: EvidenceLevel::GeometryAccepted,
            record: fref(&format!("acc/{item}.json")),
            provenance: prov.into(),
        },
    }
}

fn reopen_item(item: &str) -> Event {
    Event::Item {
        item: item.into(),
        ev: ItemEvent::Reopen {
            reason: "fresh inputs".into(),
        },
    }
}

/// An item made STALE by a changed input can be re-accepted by importing again from a provenance recorded
/// BEFORE the change (evidence produced under the old input): the stale verdict is undone by older evidence.
#[test]
fn a2_stale_item_reaccepted_from_pre_change_provenance() {
    let mut h = Harness::new();
    setup(&mut h);
    h.ok(item_with_input("I1", "N0", &["X0"]));
    h.ok(import("I1", "P0"));
    assert!(matches!(h.st.items["I1"].phase, ItemPhase::Accepted(_)));
    let p0_seq = h.st.provenance["P0"].seq;
    h.ok(input("X0", "seed_policy", "X0 v2")); // the input the imported rule was made under changes
    assert_eq!(h.st.items["I1"].phase, ItemPhase::Stale);
    h.ok(reopen_item("I1"));
    let r = h.try_(import("I1", "P0"));
    eprintln!(
        "re-import from P0 (recorded at seq {p0_seq}, before X0 changed at seq {}): {:?} -> item {}",
        h.st.inputs["X0"].registered_seq,
        r.as_ref().map_err(|e| e.to_string()),
        h.st.items["I1"].phase.tag()
    );
    assert!(
        r.is_err(),
        "a stale item was re-accepted from a provenance older than the input change"
    );
}

/// Same through the NODE input: node family changes -> node and items stale; node reopened; the item reopened
/// and re-imported from the old provenance.
#[test]
fn a2_stale_node_reaccepted_from_pre_change_provenance() {
    let mut h = Harness::new();
    setup(&mut h);
    h.ok(item_with_input("I1", "N0", &[]));
    h.ok(import("I1", "P0"));
    h.ok(input("N0.family", "family", "f v2"));
    assert_eq!(h.st.nodes["N0"].state.phase, NodePhase::Stale);
    h.ok(Event::Node {
        node: "N0".into(),
        ev: NodeEvent::Reopen {
            reason: "new family".into(),
        },
    });
    h.ok(reopen_item("I1"));
    let r = h.try_(import("I1", "P0"));
    eprintln!(
        "node path: re-import after the family changed: {:?}",
        r.as_ref().map_err(|e| e.to_string())
    );
    assert!(
        r.is_err(),
        "a stale node's item was re-accepted from a provenance older than the family change"
    );
}

/// Changing an input BACK to its old digest must not revive anything: jobs stay stale, old generations stay
/// unusable.
#[test]
fn a2_input_flip_back_revives_nothing() {
    let mut h = Harness::new();
    setup(&mut h);
    h.ok(item_with_input("I1", "N0", &["X0"]));
    let job = "J1";
    h.ok(Event::JobQueued {
        job: job.into(),
        item: "I1".into(),
        rung: Rung(0),
        kind: "generate".into(),
        spec_blake3: digest(job),
        binary: "rr_node".into(),
        reserved_bytes: 1,
        inputs: vec![],
    });
    h.ok(Event::Item {
        item: "I1".into(),
        ev: ItemEvent::Start {
            job: Some(job.into()),
        },
    });
    h.ok(Event::Job {
        job: job.into(),
        attempt: 1,
        ev: JobEvent::Launch {
            run_dir: "large:j".into(),
            cpus: vec![1],
            reserved_bytes: 1,
            spec_blake3: digest("a1"),
            scope_unit: None,
        },
    });
    h.ok(Event::Job {
        job: job.into(),
        attempt: 1,
        ev: JobEvent::Started {
            pid: 1,
            start_ticks: 1,
            boot_id: "b".into(),
        },
    });
    h.ok(input("X0", "seed_policy", "X0 v2"));
    h.ok(input("X0", "seed_policy", "X0 v1")); // back
    assert!(h.st.jobs[job].stale.is_some());
    // the stale job may settle (its process exists) ...
    h.ok(Event::Job {
        job: job.into(),
        attempt: 1,
        ev: JobEvent::Finished {
            class: FinishClass::RuleFound,
            accept: ContractAccept::Canonical,
            exit_code: Some(0),
            signal: None,
            sealed_output: Some(table_ref(job, 1)),
            outcome: Some(fref("large:o")),
            binding: Some(OutcomeBinding {
                job: job.into(),
                attempt: 1,
                spec_blake3: digest("a1"),
            }),
            cpu_ms: 1,
            wall_ms: 1,
            peak_rss_bytes: 1,
            time_decided: false,
        },
    });
    // ... but never feeds a result, neither before nor after a reopen
    h.ok(reopen_item("I1"));
    h.ok(Event::Item {
        item: "I1".into(),
        ev: ItemEvent::Start { job: None },
    });
    let r = h.try_(Event::Item {
        item: "I1".into(),
        ev: ItemEvent::Produce {
            job: job.into(),
            attempt: 1,
            table: table_ref(job, 1),
        },
    });
    assert!(
        r.is_err(),
        "a stale job fed a result after the input flipped back"
    );
}

/// An operator retry revives a stale PARENT whose escalated child carries the work (item reopen refuses this,
/// audit A m11; control retry does not check it): duplicate open work.
#[test]
fn a2_control_retry_revives_parent_with_live_child() {
    let mut h = Harness::new();
    setup(&mut h);
    h.ok(item_with_input("P", "N0", &[]));
    h.ok(Event::ItemCreated {
        item: "C".into(),
        node: "N0".into(),
        kind: "face".into(),
        rung: Rung(1),
        parent: Some("P".into()),
        inputs: vec![],
        label: "C".into(),
        weight: 1,
    });
    h.ok(Event::Item {
        item: "P".into(),
        ev: ItemEvent::MarkStale {
            input: "manual".into(),
        },
    });
    // child is stale too (transitive)? only if marked through stale_items; an item.mark_stale is not transitive
    let child_phase = h.st.items["C"].phase.tag().to_string();
    let reopen = h.try_(reopen_item("P"));
    h.ok(Event::Control {
        id: "c1".into(),
        ev: ControlEvent::Requested {
            verb: ControlVerb::Retry,
            target: ControlTarget::Item { item: "P".into() },
            by: "op".into(),
            reason: "x".into(),
        },
    });
    let applied = h.try_(Event::Control {
        id: "c1".into(),
        ev: ControlEvent::Applied {
            effect: "retry".into(),
        },
    });
    eprintln!(
        "child C is {child_phase}; item reopen of P: {:?}; control retry of P: {:?} -> P {}, C {}",
        reopen.as_ref().map_err(|e| e.to_string()),
        applied.as_ref().map_err(|e| e.to_string()),
        h.st.items["P"].phase.tag(),
        h.st.items["C"].phase.tag()
    );
    assert!(
        !(reopen.is_err()
            && applied.is_ok()
            && h.st.items["P"].phase == ItemPhase::Open
            && h.st.items["C"].phase.is_open()),
        "control retry revived a parent whose child carries the work (item reopen refuses the same)"
    );
}

/// The rule-set digest must not depend on the ORDER in which the same keys were excluded (same live rule set).
#[test]
fn a2_rule_set_digest_depends_on_exclusion_order() {
    let mk = |order: [&str; 2]| {
        let mut h = Harness::new();
        setup(&mut h);
        h.ok(item_with_input("I1", "N0", &[]));
        h.ok(import("I1", "P0"));
        for k in order {
            h.ok(Event::Item {
                item: "I1".into(),
                ev: ItemEvent::ExcludeKeys {
                    keys: fref(k),
                    disproof: fref("large:d"),
                    reason: "x".into(),
                },
            });
        }
        h.st.nodes["N0"].rule_set.digest()
    };
    let a = mk(["large:ex/A", "large:ex/B"]);
    let b = mk(["large:ex/B", "large:ex/A"]);
    eprintln!("exclusions A,B -> {}..; B,A -> {}..", &a[..12], &b[..12]);
    assert_eq!(
        a, b,
        "same live rule set (same exclusions), different rule-set digest"
    );
}

/// Order-insensitivity where the spec says so: accepting the same rules in another order gives the same digest.
#[test]
fn a2_rule_set_digest_is_acceptance_order_free() {
    let mk = |order: [&str; 3]| {
        let mut h = Harness::new();
        setup(&mut h);
        for k in ["I1", "I2", "I3"] {
            h.ok(item_with_input(k, "N0", &[]));
        }
        for k in order {
            h.ok(import(k, "P0"));
        }
        h.st.nodes["N0"].rule_set.digest()
    };
    assert_eq!(mk(["I1", "I2", "I3"]), mk(["I3", "I1", "I2"]));
}

/// A job of an OLD generation (item reopened since) or of a stale item can still be LAUNCHED (economy, R31: it
/// can never feed a result, but it burns CPU).
#[test]
fn a2_launch_of_job_that_can_never_feed() {
    let mut h = Harness::new();
    setup(&mut h);
    h.ok(item_with_input("I1", "N0", &[]));
    h.ok(Event::JobQueued {
        job: "J1".into(),
        item: "I1".into(),
        rung: Rung(0),
        kind: "g".into(),
        spec_blake3: digest("J1"),
        binary: "rr_node".into(),
        reserved_bytes: 1,
        inputs: vec![],
    });
    h.ok(Event::Item {
        item: "I1".into(),
        ev: ItemEvent::MarkStale {
            input: "manual".into(),
        },
    });
    let r = h.try_(Event::Job {
        job: "J1".into(),
        attempt: 1,
        ev: JobEvent::Launch {
            run_dir: "large:j".into(),
            cpus: vec![1],
            reserved_bytes: 1,
            spec_blake3: digest("a"),
            scope_unit: None,
        },
    });
    eprintln!(
        "launch of a queued job of a STALE item: {:?}",
        r.as_ref().map_err(|e| e.to_string())
    );
    assert!(
        r.is_err(),
        "a job whose item is stale (it can never feed a result) was launched"
    );
}

/// Progress files: two distinct node names that sanitise to the same file name (`a/b`, `a_b`) share ONE node
/// file: one node's progress silently overwrites the other's and `progress::verify` cannot notice.
#[test]
fn a2_progress_node_file_collision() {
    let mut h = Harness::new();
    h.ok(input("rr_node", "binary", "rr_node v1"));
    for n in ["fam/x", "fam_x"] {
        h.ok(input(&format!("{n}.f"), "family", n));
        h.ok(Event::NodeRegistered {
            node: n.into(),
            inputs: vec![format!("{n}.f")],
            terminal_bound: None,
            spec_box: None,
            spec: ledger::NodeSpecValues::default(),
            reference: None,
        });
    }
    let set = rustred_campaign::progress::derive(
        &h.st,
        "head",
        &rustred_campaign::progress::Resources::default(),
        "2026-11-14T00:00:00.000Z",
    );
    let node_files: Vec<&String> = set.keys().filter(|k| k.starts_with("nodes/")).collect();
    eprintln!("2 nodes registered -> node files {node_files:?}");
    assert_eq!(node_files.len(), 2, "two nodes, one progress file");
}
