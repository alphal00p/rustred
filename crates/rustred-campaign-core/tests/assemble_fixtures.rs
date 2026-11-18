//! TABLE ASSEMBLY on the M2 slice: KO table + accepted witnesses -> ONE table_dump.v1 file; the core dispatcher on
//! the assembled FILE closes the box with the generation terminals; `dispatch_equal` against the ENGINE's matcher
//! (frozen `rr_node domaincheck`, read-only test oracle, run inside light_build's 2 GB scope); exclusions made safe by
//! a hoisted point record / a terminal, an unsafe one blocks export; retirement and used-rule rules (R33).
//! Run: `light_build.sh accept test -p rustred-campaign-core --release --test assemble_fixtures -- --nocapture`.

mod accept_common;
mod inputs;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use accept_common::*;
use rustred_campaign_core::accept::facts::ExactRecordFacts;
use rustred_campaign_core::accept::prover::{EvidenceProver, FilePerKeyProver};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::accept::{
    AcceptInput, AcceptPolicy, Accepted, Candidate, FailureKind, NodeTarget, Position,
    PriorityClass, RequiredEvidence, RuleSource, accept_candidate,
};
use rustred_campaign_core::assemble::domaincheck::{compare, core_view, key_file};
use rustred_campaign_core::assemble::{
    AssemblyInput, FullRetirements, InputTable, assemble, verify_assembled,
};
use rustred_campaign_core::closure::{
    ClosureConfig, HandDowns, KeyCodec, KeySet, NoFacts, RuleBodies, TerminalSets, Tier, closure,
};
use rustred_campaign_core::economy::{CoverageState, Retirement, RuleMeasure, SupersedeReason};
use rustred_campaign_core::key::Key;
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};

const DOMAINCHECK: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/bin/frozen/20261114b/rr_node_domaincheck";

fn engine_out(table: &Path, keys: &[Key], dir: &Path) -> Option<String> {
    if !Path::new(DOMAINCHECK).is_file() {
        inputs::missing(format_args!("SKIP engine: {DOMAINCHECK} missing"));
        return None;
    }
    let kf = dir.join("keys.txt");
    std::fs::write(&kf, key_file(keys)).unwrap();
    let out = dir.join("domaincheck.out.jsonl");
    let st = Command::new(DOMAINCHECK)
        .args(["domaincheck", "--table"])
        .arg(table)
        .arg("--keys")
        .arg(&kf)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        st.status.success(),
        "domaincheck: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    Some(std::fs::read_to_string(out).unwrap())
}

/// The acceptance tokens of a KO run (assembly takes nothing else).
fn accepted_rules(run: &KoRun) -> Vec<Accepted> {
    run.accepted
        .iter()
        .map(|(_, _, _, v)| v.accepted().expect("accepted").clone())
        .collect()
}

fn input_of(
    node: &Node,
    run: &KoRun,
    accepted: Vec<Accepted>,
    target: NodeTarget,
) -> AssemblyInput {
    AssemblyInput {
        node: node.name.clone(),
        target,
        inputs: vec![InputTable {
            id: "ko".into(),
            path: run.ko_path.clone(),
            priority: 0,
            sha256: None,
        }],
        accepted,
        terminals: node.terminals.clone(),
        credits: vec![],
        retirements: vec![],
        removals: vec![],
        full_retirements: FullRetirements::Refuse,
        keep_skipped: false,
        used: None,
        renumber_index: true,
        recipes: 0,
    }
}

/// Tier K closure of the box on the assembled FILE (core view), terminals declared.
fn closure_on_file(node: &Node, table: &Path) -> (u64, u64, BTreeSet<String>) {
    let cv = core_view(table).unwrap();
    let tf = read_table_file(
        table,
        ReadOptions {
            rhs: RhsMode::KeysOnly,
        },
    )
    .unwrap();
    let mut bodies = RuleBodies::new(&cv.disp);
    bodies.load_table_records(&cv.disp, 0, &tf).unwrap();
    let codec = KeyCodec::new(node.frame.arity(), 8).unwrap();
    let start = KeySet::from_keys(codec, node.box_keys.iter().map(|k| k.as_slice())).unwrap();
    let tset = KeySet::from_keys(codec, cv.terminals.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 2,
        hand_downs: HandDowns::Off,
        longest_chain: false,
        zero_sectors: Some(node.zero_dropped.clone()),
        zero_sector_source: Some(node.zero_source.clone()),
        terminals: Some(TerminalSets {
            preferred: None,
            other: Some(tset),
        }),
        ..Default::default()
    };
    let c = closure(&cv.disp, &bodies, &node.cmp, &start, &cfg, &NoFacts).unwrap();
    let s = c.summary();
    // used rules by SOURCE line digest (the manifest maps assembled position -> source digest)
    let man = std::fs::read_to_string(
        table.with_file_name(format!("{}.assembly.manifest.jsonl", node.name)),
    )
    .unwrap();
    let src: Vec<String> = man
        .lines()
        .skip(1)
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["line_sha256"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let used = c
        .used_rules()
        .iter()
        .filter(|u| u.fires > 0)
        .map(|u| src[u.rule.record].clone())
        .collect();
    (s.open_total, s.terminals_in_start, used)
}

fn assemble_node(name: &str) {
    if !available(name) {
        inputs::missing(format_args!("SKIP {name}"));
        return;
    }
    let node = load_node(name);
    let kos: Vec<String> = node.expected["ko"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["id"].as_str().unwrap().to_string())
        .filter(|k| witness_path(name, k).is_file())
        .collect();
    for ko in kos {
        let mut prover = replay_prover(&node);
        let run = run_ko(&node, &ko, &mut prover, RequiredEvidence::Replay);
        let dir = scratch(&format!("{name}-{ko}-asm"));
        let inp = input_of(
            &node,
            &run,
            accepted_rules(&run),
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
        );
        let out = assemble(&inp, &dir).unwrap();
        // the shipped file is the manifest's records in order, byte for byte (index renumbering undone)
        assert_eq!(
            verify_assembled(&out.table, &out.manifest).unwrap() as u64,
            out.report.records
        );
        assert_eq!(out.report.order_checked, out.report.campaign_records);
        // determinism: assembling again gives the same bytes
        let out2 = assemble(&inp, &scratch(&format!("{name}-{ko}-asm2"))).unwrap();
        assert_eq!(out.table_sha256, out2.table_sha256);
        assert_eq!(out.order_digest, out2.order_digest);
        let (open, terms, used) = closure_on_file(&node, &out.table);
        assert_eq!(open, 0, "{name} {ko}: open keys on the assembled file");
        assert_eq!(terms as usize, node.terminals.len());
        // dispatch_equal on the working range (box + envelope) against the engine's matcher
        let eq = engine_out(&out.table, &run.working, &dir)
            .map(|o| compare(&core_view(&out.table).unwrap(), &run.working, &o).unwrap());
        if let Some(eq) = &eq {
            assert!(
                eq.equal(false),
                "{name} {ko}: dispatch_equal {:?}",
                &eq.mismatches[..eq.mismatches.len().min(5)]
            );
        }
        // the used-rule report: BOXED ships the used rules only (R20), FULL keeps everything (R33)
        let mut boxed_used = inp;
        boxed_used.used = Some(used.clone());
        let ob = assemble(&boxed_used, &scratch(&format!("{name}-{ko}-asm-used"))).unwrap();
        let (open_b, _, _) = closure_on_file(&node, &ob.table);
        assert_eq!(
            open_b, 0,
            "{name} {ko}: shipping only used rules keeps the box closed"
        );
        // tokens judged for the BOXED node are not assemblable into the FULL node (binding: target)
        let mut full_used = boxed_used;
        full_used.target = NodeTarget::Full;
        if out.report.campaign_records > 0 {
            assert!(assemble(&full_used, &scratch(&format!("{name}-{ko}-asm-full"))).is_err());
        }
        // R33: FULL keeps everything (the used report is attached, nothing dropped): input tables alone
        full_used.accepted.clear();
        let of = assemble(&full_used, &scratch(&format!("{name}-{ko}-asm-full"))).unwrap();
        assert!(of.report.dropped_unused.is_empty());
        assert_eq!(of.report.records, out.report.input_records);
        println!(
            "{name} {ko}: assembled {} records ({} input + {} campaign), sha {}, order {}; closure open {open}, terminals {terms}; dispatch_equal {}; boxed used-only {} records (dropped {})",
            out.report.records,
            out.report.input_records,
            out.report.campaign_records,
            &out.table_sha256[..23],
            &out.order_digest[..23],
            eq.map(|e| format!(
                "{} keys: {} rule, {} terminal, {} none, mismatches {}",
                e.keys,
                e.rule,
                e.terminal,
                e.none,
                e.mismatches.len()
            ))
            .unwrap_or_else(|| "skipped".into()),
            ob.report.records,
            ob.report.dropped_unused.len()
        );
    }
}

#[test]
fn toy2kite_assembled_closes_and_matches_engine() {
    assemble_node("TOY2kite");
}

#[test]
fn l3a_assembled_closes_and_matches_engine() {
    assemble_node("L3a");
}

#[test]
fn l4a_assembled_closes_and_matches_engine() {
    assemble_node("L4a");
}

#[allow(clippy::too_many_arguments)]
/// The chain on the BOXED fixture node with the per-key prover fed SYNTHETIC statuses (plumbing only; every answer
/// echoes the rule digest): `fail` maps served keys to a status word, every other served key is in span.
fn judge_per_key(
    node: &Node,
    before: &rustred_campaign_core::accept::InForce,
    line: &[u8],
    src: RuleSource,
    position: Position,
    gaps: &[Key],
    working: &[Key],
    fail: &[(Key, &str)],
) -> rustred_campaign_core::accept::AcceptVerdict {
    let rule = RuleText::parse(line, &node.frame).unwrap();
    let mut prover = FilePerKeyProver::new(vec![node.frame.arity()], None);
    let mut facts = ExactRecordFacts::new();
    let input = AcceptInput {
        node: view(
            node,
            NodeTarget::Boxed {
                box_id: "fixture-box".into(),
            },
            RequiredEvidence::PerKey,
        ),
        before: Some(before),
        after: None,
        candidate: Candidate {
            rule: &rule,
            source: src,
            position,
        },
        gap_points: gaps,
        working,
        policy: AcceptPolicy::default(),
    };
    let rustred_campaign_core::accept::AcceptVerdict::NeedsExternal { requests, .. } =
        accept_candidate(&input, &mut facts, &mut prover).unwrap()
    else {
        panic!("expected a request first")
    };
    let req: serde_json::Value = serde_json::from_str(requests[0].text.trim()).unwrap();
    let keys: Vec<Key> = serde_json::from_value(req["keys"].clone()).unwrap();
    let ks: Vec<serde_json::Value> = keys
        .iter()
        .map(|k| {
            let st = fail
                .iter()
                .find(|(f, _)| f == k)
                .map(|(_, s)| s.to_string())
                .unwrap_or_else(|| "EXACT_IN_SPAN_AT_KEY".into());
            serde_json::json!({"key": k, "status": st})
        })
        .collect();
    let ans = serde_json::json!({"schema": "rustred.proof-kernel.span.v1", "rule": rule.index(),
        "rule_sha256": rule.line_sha256, "keys": ks});
    prover.load_answer(&rule, &ans.to_string()).unwrap();
    let _ = prover.level();
    accept_candidate(&input, &mut facts, &mut prover).unwrap()
}

/// Exclusions (from the CHAIN, never caller data): safe by a hoisted point record that the chain accepted against the
/// face rule with its exclusion honoured (engine agrees at the key), safe by a terminal, UNSAFE otherwise.
#[test]
fn exclusions_hoist_terminal_and_unsafe() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let ko = "K2-single";
    let ko_path = ko_table(&node, ko);
    let gaps = gap_keys(&node, ko);
    let (off, line) = witness_lines("TOY2kite", ko)[0].clone();
    let r0 = RuleText::parse(&line, &node.frame).unwrap();
    let served: Vec<Key> = gaps
        .iter()
        .filter(|g| r0.domain.as_ref().unwrap().applies(g).applies())
        .cloned()
        .collect();
    let (k_hoist, k_unsafe, k_term) = (served[0].clone(), served[1].clone(), served[2].clone());
    let before = in_force(&node, Some(&ko_path), &[]);
    let face = judge_per_key(
        &node,
        &before,
        &line,
        source("TOY2kite", ko, off, 0),
        position(&r0, ko, 0),
        &gaps,
        &node.box_keys,
        &[
            (k_hoist.clone(), "EXACT_NOT_IN_SPAN_AT_KEY (synthetic)"),
            (k_unsafe.clone(), "IRREGULAR_KEY (synthetic)"),
            (k_term.clone(), "HIDDEN_KEY (synthetic)"),
        ],
    );
    let face_acc = face
        .accepted()
        .unwrap_or_else(|| panic!("{face:?}"))
        .clone();
    assert_eq!(face_acc.exclusions().len(), 3);
    // a POINT record at k_hoist (a copy of the face rule with every slot fixed), judged by the chain against the KO
    // table and the face rule WITH its exclusions honoured: it wins k_hoist
    let dir = scratch("exclusions");
    let mut v: serde_json::Value = serde_json::from_slice(&r0.line).unwrap();
    v["fixed"] = serde_json::json!(k_hoist);
    v["target"] = serde_json::json!(format!(
        "I({})",
        k_hoist
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ));
    v["exceptions"] = serde_json::json!([]);
    v["witness_loci"] = serde_json::json!([]);
    let point_line = serde_json::to_vec(&v).unwrap();
    let pf: PathBuf = dir.join("point.jsonl");
    std::fs::write(&pf, [point_line.as_slice(), b"\n"].concat()).unwrap();
    let before_p = in_force(&node, Some(&ko_path), &[(&face, &r0)]);
    let point_src = RuleSource {
        table_id: "point".into(),
        table_path: pf.display().to_string(),
        table_digest: rustred_campaign_core::accept::rule::sha256_tag(&std::fs::read(&pf).unwrap()),
        ordinal: 0,
        byte_offset: Some(0),
        producer: None,
        table_guard_version: None,
    };
    let point_pos = Position {
        class: PriorityClass::Point,
        input_priority: 0,
        round: 1,
        item: "pt".into(),
        index: 0,
    };
    let point = judge_per_key(
        &node,
        &before_p,
        &point_line,
        point_src,
        point_pos,
        std::slice::from_ref(&k_hoist),
        &node.box_keys,
        &[],
    );
    let point_acc = point
        .accepted()
        .unwrap_or_else(|| panic!("{point:?}"))
        .clone();
    assert_eq!(point_acc.served().1, 1);
    // without the exclusion honoured, the face rule would have won k_hoist and the point rule served nothing
    let mut inp = input_of(
        &node,
        &KoRun {
            ko_path: ko_path.clone(),
            accepted: vec![],
            rejected: vec![],
            working: vec![],
            gaps: vec![],
        },
        vec![face_acc, point_acc],
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    inp.terminals.push(k_term.clone());
    let out = assemble(&inp, &dir).unwrap();
    println!("exclusions: {:?}", out.report);
    assert_eq!(out.report.hoisted.len(), 1);
    assert_eq!(out.report.exclusions_safe, 2, "hoisted point + terminal");
    assert_eq!(out.report.blocking.len(), 1, "the key without a point rule");
    assert!(!out.report.export_ready);
    // the hoisted point record now wins at k_hoist in the core AND in the engine
    let cv = core_view(&out.table).unwrap();
    let hoisted_n = out.report.hoisted[0].0 as usize;
    let d = cv.disp.first_match(&k_hoist).unwrap();
    assert_eq!(d.winner().unwrap().record, hoisted_n);
    let keys = vec![k_hoist.clone(), k_unsafe.clone(), k_term.clone()];
    if let Some(o) = engine_out(&out.table, &keys, &dir) {
        let eq = compare(&cv, &keys, &o).unwrap();
        println!("exclusions dispatch_equal: {eq:?}");
        assert!(eq.equal(false));
        assert_eq!(eq.terminal, 1);
    }
    let extras = std::fs::read_to_string(&out.extras).unwrap();
    assert_eq!(extras.lines().count(), 3);
    assert!(extras.contains("hoisted_point_rule"));
    assert_eq!(
        verify_assembled(&out.table, &out.manifest).unwrap() as u64,
        out.report.records
    );
    let _ = FailureKind::Hidden;
}

/// R33: on a FULL node every retirement is refused while the partition proofs are not certified (and needs a bound
/// proof after); a FULL removal is refused; on a BOXED node a retirement applies.
#[test]
fn retirement_rules() {
    if !available("TOY2kite") {
        return;
    }
    let node = load_node("TOY2kite");
    let ko = "K2-single";
    let mut prover = replay_prover(&node);
    let run = run_ko(&node, ko, &mut prover, RequiredEvidence::Replay);
    let acc = accepted_rules(&run);
    let victim = acc[0].rule_sha256().to_string();
    let m = |terms: u64| RuleMeasure {
        cases: 1,
        rhs_terms: terms,
        monomials: terms,
        monomials_estimated: false,
        max_degree: None,
        served_keys: None,
        bytes: 0,
    };
    let boxed = Retirement::new(
        &node.name,
        CoverageState::Boxed,
        &[(victim.clone(), m(10))],
        ("by", &m(2)),
        SupersedeReason::CheaperSameOrLargerDomain,
        None,
    )
    .unwrap();
    assert!(
        Retirement::new(
            &node.name,
            CoverageState::Full,
            &[(victim.clone(), m(10))],
            ("by", &m(2)),
            SupersedeReason::CheaperSameOrLargerDomain,
            None
        )
        .is_err()
    );
    // FULL: an input rule retired with a BOXED retirement / any retirement while proofs are not certified: refused
    let first_input = {
        let text = std::fs::read(&run.ko_path).unwrap();
        let l = text
            .split(|&b| b == b'\n')
            .find(|l| l.starts_with(br#"{"type":"rule""#))
            .unwrap()
            .to_vec();
        rustred_campaign_core::accept::rule::sha256_tag(&l)
    };
    let in_ret = Retirement::new(
        &node.name,
        CoverageState::Boxed,
        &[(first_input.clone(), m(10))],
        ("by", &m(2)),
        SupersedeReason::CheaperSameOrLargerDomain,
        None,
    )
    .unwrap();
    let mut inp = input_of(&node, &run, vec![], NodeTarget::Full);
    inp.retirements = vec![in_ret];
    assert!(
        assemble(&inp, &scratch("retire-full")).is_err(),
        "a retirement on a FULL node is refused"
    );
    inp.full_retirements = FullRetirements::BoundProof;
    assert!(
        assemble(&inp, &scratch("retire-full")).is_err(),
        "a FULL node needs a FULL retirement with a bound proof"
    );
    let mut inp = input_of(&node, &run, vec![], NodeTarget::Full);
    inp.removals = vec![rustred_campaign_core::assemble::Removal {
        rule_sha256: first_input,
        reason: "test".into(),
        disproof: "free text".into(),
    }];
    assert!(
        assemble(&inp, &scratch("remove-full")).is_err(),
        "FULL removal refused"
    );
    let mut inp = input_of(
        &node,
        &run,
        acc,
        NodeTarget::Boxed {
            box_id: "fixture-box".into(),
        },
    );
    inp.retirements = vec![boxed];
    let out = assemble(&inp, &scratch("retire-boxed")).unwrap();
    assert_eq!(out.report.retired, vec![victim]);
    assert_eq!(out.report.campaign_records, 0);
}
