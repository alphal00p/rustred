//! Lane ledgerfix2: reducer tests of the re-audit fixes and of the UNCONFIRMED LAUNCH state (execution unknown).
//! Pure state machines (no files, no processes).
mod ledger_support;

use ledger_support::*;
use rustred_campaign::ledger::{
    self, ControlEvent, ControlTarget, ControlVerb, Event, ItemEvent, JobEvent, OutcomeBinding,
    Record,
};
use rustred_campaign::state::{
    AttemptState, CampaignEvent, CampaignState, ContractAccept, EvidenceLevel, FinishClass,
    ItemPhase, NodeEvent, ReduceError, Rung,
};

struct H {
    st: CampaignState,
    seq: u64,
}

impl H {
    fn new() -> H {
        let mut h = H {
            st: CampaignState::default(),
            seq: 0,
        };
        h.ok(genesis_event());
        for e in setup_events("N") {
            h.ok(e);
        }
        h
    }
    fn try_(&mut self, e: Event) -> Result<(), ReduceError> {
        let r = Record {
            schema: ledger::LEDGER_SCHEMA.into(),
            seq: self.seq + 1,
            utc: T0.into(),
            prev: ledger::GENESIS.into(),
            event: e,
        };
        let before = self.st.clone();
        match self.st.apply_record(&r) {
            Ok(()) => {
                self.seq += 1;
                Ok(())
            }
            Err(err) => {
                assert!(self.st == before, "a refusal changed the state: {err}");
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
    fn refused(&mut self, e: Event, what: &str) -> String {
        match self.try_(e) {
            Ok(()) => panic!("{what}: accepted"),
            Err(err) => err.to_string(),
        }
    }
}

fn job(job: &str, attempt: u32, ev: JobEvent) -> Event {
    Event::Job {
        job: job.into(),
        attempt,
        ev,
    }
}

fn launch(spec: &str) -> JobEvent {
    JobEvent::Launch {
        run_dir: "large:jobs/J.a1".into(),
        cpus: vec![1],
        reserved_bytes: 1,
        spec_blake3: digest(spec),
        scope_unit: None,
    }
}

fn finished(job: &str, attempt: u32, binding: Option<&str>, sealed: bool) -> JobEvent {
    JobEvent::Finished {
        class: if sealed {
            FinishClass::RuleFound
        } else {
            FinishClass::Lost
        },
        accept: if sealed {
            ContractAccept::Canonical
        } else {
            ContractAccept::Nothing
        },
        exit_code: None,
        signal: None,
        sealed_output: sealed.then(|| table_ref(job, attempt)),
        outcome: binding.map(|_| fref("large:jobs/J.a1/OUTCOME.json")),
        binding: binding.map(|s| OutcomeBinding {
            job: job.into(),
            attempt,
            spec_blake3: digest(s),
        }),
        cpu_ms: 5,
        wall_ms: 5,
        peak_rss_bytes: 1,
        time_decided: false,
    }
}

fn unconfirmed() -> JobEvent {
    JobEvent::Unconfirmed {
        reason: "launch of an earlier session never confirmed".into(),
    }
}

fn session(h: &mut H) {
    h.ok(Event::Campaign {
        ev: CampaignEvent::Start,
        reason: "resume".into(),
    });
    h.ok(Event::Campaign {
        ev: CampaignEvent::Run,
        reason: "resume".into(),
    });
}

/// Item `I` with job `J` launched (intent journalled) but never started; then a new session.
fn unconfirmed_setup() -> H {
    let mut h = H::new();
    h.ok(item_created("N", 0, 0, None, "face"));
    let item = item_id("N", 0);
    h.ok(job_queued("J", &item, 0, "generate"));
    h.ok(Event::Item {
        item: item.clone(),
        ev: ItemEvent::Start {
            job: Some("J".into()),
        },
    });
    h.ok(job("J", 1, launch("a1")));
    assert!(
        h.st.unconfirmed_launches().is_empty(),
        "same session: not unconfirmed yet"
    );
    session(&mut h);
    let u: Vec<String> =
        h.st.unconfirmed_launches()
            .into_iter()
            .map(|(j, _)| j.clone())
            .collect();
    assert_eq!(u, vec!["J".to_string()], "the recovery hook's worklist");
    h
}

#[test]
fn unconfirmed_launch_is_never_did_not_run_and_never_retried_automatically() {
    let mut h = unconfirmed_setup();
    let item = item_id("N", 0);
    h.ok(job("J", 1, unconfirmed()));
    assert_eq!(h.st.jobs["J"].state, AttemptState::Unconfirmed);
    assert!(
        !h.st.jobs["J"].state.is_settled(),
        "not settled: the campaign cannot finish"
    );
    assert_eq!(h.st.execution_unknown_jobs().len(), 1);
    assert!(h.st.unconfirmed_launches().is_empty());
    // never reduced to "did not run"
    for ev in [
        JobEvent::Lose { reason: "x".into() },
        JobEvent::Interrupt { reason: "x".into() },
    ] {
        h.refused(job("J", 1, ev), "lose / interrupt of an unconfirmed launch");
    }
    h.refused(
        job("J", 2, JobEvent::Requeue { reason: "x".into() }),
        "requeue",
    );
    h.refused(job("J", 2, JobEvent::Retry { reason: "x".into() }), "retry");
    h.refused(
        job("J", 1, finished("J", 1, None, false)),
        "unauthenticated finish (no OUTCOME binding)",
    );
    h.refused(
        job("J", 1, finished("J", 1, Some("another launch"), true)),
        "finish bound to another launch",
    );
    h.refused(
        job("J", 1, JobEvent::Cancel { reason: "x".into() }),
        "cancel of the current work",
    );
    // never retried automatically through the item either
    for ev in [
        ItemEvent::Requeue { reason: "x".into() },
        ItemEvent::Escalate {
            to: Rung(1),
            reason: "x".into(),
        },
        ItemEvent::ProposeTerminal {
            reason: "x".into(),
            preferred: true,
            exhausted: None,
        },
    ] {
        h.refused(
            Event::Item {
                item: item.clone(),
                ev,
            },
            "item requeue / escalate / terminal over an unconfirmed launch",
        );
    }
    h.refused(
        job_queued("J2", &item, 0, "generate"),
        "a new job for the same work",
    );
    h.refused(
        Event::Campaign {
            ev: CampaignEvent::Finish,
            reason: "x".into(),
        },
        "finish over an execution-unknown attempt",
    );
    // authenticated outcome of exactly this launch: consumed without re-solving
    h.ok(job("J", 1, finished("J", 1, Some("a1"), true)));
    assert!(matches!(
        h.st.jobs["J"].state,
        AttemptState::SucceededSealed(_)
    ));
    h.ok(Event::Item {
        item: item.clone(),
        ev: ItemEvent::Produce {
            job: "J".into(),
            attempt: 1,
            table: table_ref("J", 1),
        },
    });
    assert_eq!(h.st.items[&item].phase, ItemPhase::Produced);
}

#[test]
fn unconfirmed_launch_late_confirmation_and_operator_retry() {
    // late confirmation: the recorded child is found and adopted
    let mut h = unconfirmed_setup();
    h.ok(job("J", 1, unconfirmed()));
    h.ok(job(
        "J",
        1,
        JobEvent::Started {
            pid: 7,
            start_ticks: 9,
            boot_id: "b".into(),
        },
    ));
    assert_eq!(h.st.jobs["J"].state, AttemptState::Running);
    // no valid result: the item needs a person; only an operator retry starts the work again
    let mut h = unconfirmed_setup();
    let item = item_id("N", 0);
    h.ok(job("J", 1, unconfirmed()));
    h.ok(Event::Item {
        item: item.clone(),
        ev: ItemEvent::Attention {
            code: "execution_unknown".into(),
            text: "run directory holds no authenticated outcome".into(),
        },
    });
    h.refused(
        Event::Item {
            item: item.clone(),
            ev: ItemEvent::Reopen { reason: "x".into() },
        },
        "an automatic reopen from attention",
    );
    h.ok(Event::Control {
        id: "c1".into(),
        ev: ControlEvent::Requested {
            verb: ControlVerb::Retry,
            target: ControlTarget::Item { item: item.clone() },
            by: "operator".into(),
            reason: "checked the run directory".into(),
        },
    });
    h.ok(Event::Control {
        id: "c1".into(),
        ev: ControlEvent::Applied {
            effect: "retry".into(),
        },
    });
    assert_eq!(h.st.items[&item].phase, ItemPhase::Open);
    // the old attempt can no longer feed: it may now be cleared (cancel), and new work queued
    h.ok(job(
        "J",
        1,
        JobEvent::Cancel {
            reason: "superseded by the operator retry".into(),
        },
    ));
    h.ok(job_queued("J2", &item, 0, "generate"));
}

#[test]
fn strict_mode_refuses_the_legacy_recovery_of_an_unconfirmed_launch() {
    // lane int5: strict mode is ON by default (ENFORCE_UNCONFIRMED_ACROSS_SESSIONS, the supervisor adopted the hook):
    // the legacy lose of an earlier session's launch is refused even without the per-state switch
    const STRICT: bool = rustred_campaign::state::ENFORCE_UNCONFIRMED_ACROSS_SESSIONS;
    const _: () = assert!(STRICT, "strict unconfirmed mode is on");
    let mut h = unconfirmed_setup();
    h.refused(
        job(
            "J",
            1,
            JobEvent::Lose {
                reason: "legacy".into(),
            },
        ),
        "legacy lose (strict by default)",
    );
    // strict: lose / interrupt / an unauthenticated finish of an earlier session's launch are refused
    let mut h = unconfirmed_setup();
    h.st.strict_unconfirmed = true;
    h.refused(
        job("J", 1, JobEvent::Lose { reason: "x".into() }),
        "strict lose",
    );
    h.refused(
        job("J", 1, JobEvent::Interrupt { reason: "x".into() }),
        "strict interrupt",
    );
    h.refused(
        job("J", 1, finished("J", 1, None, false)),
        "strict unauthenticated finish",
    );
    h.ok(job("J", 1, unconfirmed()));
    // within ONE session the supervisor knows the truth (e.g. spawn failed): not affected
    let mut h = H::new();
    h.st.strict_unconfirmed = true;
    h.ok(item_created("N", 0, 0, None, "face"));
    h.ok(job_queued("J", &item_id("N", 0), 0, "generate"));
    h.ok(job("J", 1, launch("a1")));
    h.ok(job("J", 1, finished("J", 1, None, false)));
}

/// M2 property: random input changes, imports, reopenings: a live item accepted from a provenance (import or
/// evidence) never rests on a provenance recorded before the last change of one of its inputs (item, node, box).
#[test]
fn m2_property_no_acceptance_from_provenance_older_than_an_input_change() {
    let mut accepted_total = 0u64;
    let mut refused_m2 = 0u64;
    for seed in 1..=40u64 {
        let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut h = H::new();
        h.ok(input("X0", "seed_policy", "X0 v0"));
        let mut provs = 0u32;
        let mut versions = [0u32; 3];
        let ids = ["X0", "N.family", "N.box"];
        for k in 0..4 {
            h.ok(Event::ItemCreated {
                item: format!("I{k}"),
                node: "N".into(),
                kind: "face".into(),
                rung: Rung(0),
                parent: None,
                inputs: if k % 2 == 0 {
                    vec!["X0".into()]
                } else {
                    vec![]
                },
                label: format!("I{k}"),
                weight: 1,
            });
        }
        for _ in 0..300 {
            let item = format!("I{}", next() % 4);
            let ev = match next() % 7 {
                0 => {
                    provs += 1;
                    Event::TablesImported {
                        provenance: format!("P{provs}"),
                        producer: "earlier".into(),
                        source_campaign: None,
                        manifest: fref(&format!("large:p{provs}.manifest")),
                        files: vec![],
                        level_claimed: EvidenceLevel::IdentityTested,
                    }
                }
                1 => {
                    let i = (next() % 3) as usize;
                    versions[i] += 1;
                    input(
                        ids[i],
                        if i == 2 { "box" } else { "family" },
                        &format!("{} v{}", ids[i], versions[i]),
                    )
                }
                2 => Event::Node {
                    node: "N".into(),
                    ev: NodeEvent::Reopen {
                        reason: "fresh inputs".into(),
                    },
                },
                3 => Event::Item {
                    item,
                    ev: ItemEvent::Reopen {
                        reason: "fresh inputs".into(),
                    },
                },
                _ if provs > 0 => Event::Item {
                    item: item.clone(),
                    ev: ItemEvent::Import {
                        rule: format!("R-{item}-{}", next() % 1000),
                        level: EvidenceLevel::GeometryAccepted,
                        record: fref(&format!("acc/{item}")),
                        provenance: format!("P{}", 1 + next() % provs as u64),
                    },
                },
                _ => continue,
            };
            match h.try_(ev) {
                Ok(()) => {}
                Err(e) if e.to_string().contains("M2") => refused_m2 += 1,
                Err(_) => {}
            }
            // the invariant, after every step
            for (iid, it) in h.st.items.iter() {
                let (ItemPhase::Accepted(_), Some(a)) = (&it.phase, &it.accepted) else {
                    continue;
                };
                let rustred_campaign::state::AcceptedFrom::Import { provenance } = &a.source else {
                    continue;
                };
                let p = &h.st.provenance[provenance];
                assert!(
                    h.st.input_changed_after(p.seq, it).is_none(),
                    "seed {seed}: {iid} accepted from {provenance} (seq {}) older than an input change",
                    p.seq
                );
                accepted_total += 1;
            }
        }
    }
    eprintln!("M2 property: {accepted_total} accepted-item checks, {refused_m2} M2 refusals");
    assert!(
        refused_m2 > 0 && accepted_total > 0,
        "the generator exercises both sides"
    );
}

/// M4: the children index carries staleness through a deep escalation chain, and reopen / retry see the child.
#[test]
fn m4_children_index_carries_staleness_through_chains() {
    let mut h = H::new();
    h.ok(input("X0", "seed_policy", "X0 v0"));
    let mut parent: Option<String> = None;
    for r in 0..6u8 {
        let id = format!("C{r}");
        h.ok(Event::ItemCreated {
            item: id.clone(),
            node: "N".into(),
            kind: "face".into(),
            rung: Rung(r),
            parent: parent.clone(),
            inputs: if r == 0 { vec!["X0".into()] } else { vec![] },
            label: id.clone(),
            weight: 1,
        });
        if let Some(p) = &parent {
            assert_eq!(h.st.items[p].children, vec![id.clone()]);
        }
        parent = Some(id);
    }
    h.ok(input("X0", "seed_policy", "X0 v1"));
    for r in 0..6 {
        assert_eq!(h.st.items[&format!("C{r}")].phase, ItemPhase::Stale, "C{r}");
    }
    // the leaf may be reopened, the parents not while a non-stale child carries their work
    h.ok(Event::Item {
        item: "C5".into(),
        ev: ItemEvent::Reopen { reason: "x".into() },
    });
    let why = h.refused(
        Event::Item {
            item: "C4".into(),
            ev: ItemEvent::Reopen { reason: "x".into() },
        },
        "reopen of a parent with a live child",
    );
    assert!(why.contains("carries the work"), "{why}");
}

/// Lane int5: `not_spawned` (the spawn of an unconfirmed launch PROVABLY never happened: authenticated run directory
/// without the spawn sentinel) is the only way from `unconfirmed` to "did not run": unconfirmed -> lost, then requeue.
/// Not from `launching` (the hook marks first), not from a confirmed (running) attempt; lose / requeue from
/// `unconfirmed` stay refused.
#[test]
fn not_spawned_is_the_only_proven_did_not_run_of_an_unconfirmed_launch() {
    let ns = || JobEvent::NotSpawned {
        reason: "authenticated run directory without RUN/sup/SPAWN".into(),
    };
    let rq = || JobEvent::Requeue {
        reason: "spawn provably never happened".into(),
    };
    // from launching (not marked): refused
    let mut h = unconfirmed_setup();
    h.refused(job("J", 1, ns()), "not_spawned from launching");
    h.ok(job("J", 1, unconfirmed()));
    h.refused(
        job("J", 1, JobEvent::Lose { reason: "x".into() }),
        "lose from unconfirmed",
    );
    h.refused(job("J", 2, rq()), "requeue from unconfirmed");
    h.ok(job("J", 1, ns()));
    assert_eq!(h.st.jobs["J"].state, AttemptState::Lost);
    assert!(h.st.execution_unknown_jobs().is_empty());
    h.ok(job("J", 2, rq()));
    assert_eq!(h.st.jobs["J"].attempt, 2);
    // after a late confirmation (running) it is refused
    let mut h = unconfirmed_setup();
    h.ok(job("J", 1, unconfirmed()));
    h.ok(job(
        "J",
        1,
        JobEvent::Started {
            pid: 4242,
            start_ticks: 7,
            boot_id: "b".into(),
        },
    ));
    h.refused(job("J", 1, ns()), "not_spawned of a running attempt");
}
