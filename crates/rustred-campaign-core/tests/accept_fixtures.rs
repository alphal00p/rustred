//! The acceptance chain on the coordinator's M2 slice (small nodes): for each KO set the knocked-out table plus the
//! repair witnesses as candidates; identity at the `replay` level through the in-crate identity test, descent,
//! coverage, regularity; then a Tier C closure of the box over (KO table + accepted rules) with exact facts: closed
//! with exactly the terminals found by generation (U-9: no master count anywhere).
//! Run: `light_build.sh accept test -p rustred-campaign-core --release --test accept_fixtures -- --nocapture`.

mod accept_common;

use accept_common::*;
use rustred_campaign_core::accept::{AcceptVerdict, Check, RequiredEvidence};
use rustred_campaign_core::closure::TerminalSets;
use rustred_campaign_core::closure::{
    ClosureConfig, HandDowns, KeyCodec, KeySet, RuleBodies, Tier, Verdict, closure,
};
use rustred_campaign_core::cover::{Dispatch, Dispatcher};
use rustred_campaign_core::glue::{ClosureRule, closure_exclusions_from_accept};

fn kos(node: &str) -> Vec<String> {
    let d = slice(node).join("ko");
    let mut v: Vec<String> = std::fs::read_dir(d)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("repair_witness").is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    v.sort();
    v
}

fn check_node(name: &str) {
    if !available(name) {
        println!("SKIP {name}: fixture missing");
        return;
    }
    let node = load_node(name);
    for ko in kos(name) {
        let mut prover = replay_prover(&node);
        let run = run_ko(&node, &ko, &mut prover, RequiredEvidence::Replay);
        let rej: Vec<(usize, Option<Check>)> =
            run.rejected.iter().map(|(i, v)| (*i, v.check())).collect();
        // every non-accepted witness is a rule the KO table (or an earlier witness) already serves: Served
        for (i, v) in &run.rejected {
            assert!(
                matches!(
                    v,
                    AcceptVerdict::Rejected {
                        check: Check::Served,
                        ..
                    }
                ),
                "{name} {ko}: witness {i} not accepted for another reason: {v:?}"
            );
        }
        let (tf, ko_t) = load_table(&run.ko_path, "ko");
        let acc: Vec<&_> = run.accepted.iter().map(|(_, _, r, _)| r).collect();
        let disp = Dispatcher::new(
            vec![ko_t, accepted_table(node.frame.arity(), &acc)],
            vec![0, 1],
        )
        .unwrap();
        let terminals: std::collections::BTreeSet<_> = node.terminals.iter().cloned().collect();
        // coverage at every gap key: covered by an accepted rule, or a terminal found by generation
        for g in &run.gaps {
            let d = disp.first_match(g).unwrap();
            assert!(
                terminals.contains(g) || matches!(d, Dispatch::Covered(r) if r.table == 1),
                "{name} {ko}: gap key {g:?} -> {d:?}"
            );
        }
        // Tier C closure from the box with exact record facts
        let mut bodies = RuleBodies::new(&disp);
        bodies.load_table_records(&disp, 0, &tf).unwrap();
        let rules: Vec<_> = run.accepted.iter().map(|(_, _, r, _)| r.clone()).collect();
        for (i, r) in rules.iter().enumerate() {
            let pats: Vec<&str> = r.rhs.iter().map(|(k, _)| k.as_str()).collect();
            bodies.set(&disp, 1, i, &pats).unwrap();
        }
        let ko_id = run.ko_path.display().to_string();
        let facts = closure_facts(
            &node.frame,
            &[
                (&ko_id, &run.ko_path, None),
                ("campaign", &run.ko_path, Some(&rules)),
            ],
        );
        let codec = KeyCodec::new(node.frame.arity(), 8).unwrap();
        let start = KeySet::from_keys(codec, node.box_keys.iter().map(|k| k.as_slice())).unwrap();
        let tset = KeySet::from_keys(codec, node.terminals.iter().map(|k| k.as_slice())).unwrap();
        let cfg = ClosureConfig {
            tier: Tier::C,
            threads: 2,
            hand_downs: HandDowns::Counts,
            longest_chain: false,
            zero_sectors: Some(node.zero_dropped.clone()),
            zero_sector_source: Some(node.zero_source.clone()),
            terminals: Some(TerminalSets {
                preferred: None,
                other: Some(tset),
            }),
            // closure.md 11.5 (loopfix): every firing table needs its coefficient-file digest, else the Tier C
            // verdict is at best `closed_provisional`: the KO file, and the accepted rule lines of the campaign table
            table_digests: [
                (
                    "ko".to_string(),
                    rustred_campaign_core::accept::rule::sha256_tag(
                        &std::fs::read(&run.ko_path).unwrap(),
                    ),
                ),
                (
                    "campaign".to_string(),
                    rustred_campaign_core::accept::rule::sha256_tag(
                        &rules
                            .iter()
                            .flat_map(|r| r.line.iter().copied().chain(*b"\n"))
                            .collect::<Vec<u8>>(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            // acceptance.md (exclusions to the closure once it accepts them; int3 glue): every accepted rule's
            // effective-domain exclusions, at its record of the campaign table
            exclusions: run
                .accepted
                .iter()
                .enumerate()
                .flat_map(|(i, (_, _, _, v))| {
                    let AcceptVerdict::Accepted { effective, .. } = v else {
                        unreachable!()
                    };
                    closure_exclusions_from_accept(
                        ClosureRule {
                            table: "campaign",
                            record: i,
                            label: i as i64,
                        },
                        &effective.exclusions,
                    )
                    .expect("acceptance exclusions are per key")
                })
                .collect(),
            ..Default::default()
        };
        let n_excl = cfg.exclusions.len();
        let c = closure(&disp, &bodies, &node.cmp, &start, &cfg, &facts).unwrap();
        let s = c.summary();
        println!(
            "{name} {ko}: witnesses {} accepted {} rejected {:?}; gap keys {}; working range {}; served {}; exclusions {n_excl}; closure {:?} keys {} open {} terminals {} (expected {})",
            run.accepted.len() + run.rejected.len(),
            run.accepted.len(),
            rej,
            run.gaps.len(),
            run.working.len(),
            run.accepted
                .iter()
                .map(|(_, _, _, v)| match v {
                    AcceptVerdict::Accepted { record, .. } =>
                        record.value["served"]["count"].as_u64().unwrap(),
                    _ => 0,
                })
                .sum::<u64>(),
            s.verdict,
            s.keys_total,
            s.open_total,
            s.terminals_in_start,
            node.terminals.len()
        );
        assert_eq!(
            s.open_total,
            0,
            "{name} {ko}: open keys {:?}",
            c.open_keys()
        );
        assert_eq!(
            s.verdict,
            Verdict::Closed,
            "{name} {ko}: closure verdict; blocking {:?}; undecided {:?}; requests {:?}",
            s.blocking,
            s.undecided,
            &c.fact_requests()[..c.fact_requests().len().min(3)]
        );
        assert_eq!(
            s.terminals_in_start as usize,
            node.terminals.len(),
            "{name} {ko}: terminals"
        );
        // every accepted record names its rule and its evidence at the required level
        for (_, _, r, v) in &run.accepted {
            let AcceptVerdict::Accepted {
                record, effective, ..
            } = v
            else {
                unreachable!()
            };
            assert_eq!(record.value["rule"]["line_sha256"], r.line_sha256);
            assert_eq!(record.value["evidence"]["ledger_word"], "identity_tested");
            assert!(
                effective.exclusions.is_empty(),
                "{name} {ko}: unexpected exclusions {:?}",
                effective.exclusions
            );
            assert!(record.id.starts_with("sha256:"));
        }
    }
}

#[test]
fn toy2kite_witnesses_accepted_and_closed() {
    check_node("TOY2kite");
}

#[test]
fn l3a_witnesses_accepted_and_closed() {
    check_node("L3a");
}

#[test]
fn l4a_witnesses_accepted_and_closed() {
    check_node("L4a");
}
