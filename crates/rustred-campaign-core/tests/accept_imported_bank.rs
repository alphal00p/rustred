//! IMPORTED RULES (addendum 3): the T162 bank as per-origin engine tables (interfaces/coord-bank-tables.md) with the
//! converter's `rustred.campaign.accept.v1` provenance lines. Rules without witness loci (pre-guard-fix) may credit a
//! key only with per-key engine facts; here a TEST-ONLY fact source grants `Regular` for the requested pairs. Expected:
//! every used bank rule reaches "imported + regular fact" on the keys it wins (Undecided at Evidence: an imported label
//! never satisfies the required level), the gap keys stay open, nothing reaches `per-key` without proof certificates.
//! `#[ignore]` (large inputs): `light_build.sh accept test -p rustred-campaign-core --release --test accept_imported_bank
//! -- --ignored --nocapture`.

mod accept_common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rustred_campaign_core::accept::facts::{ExactRecordFacts, RegVerdict, Regularity};
use rustred_campaign_core::accept::prover::{ImportedEvidence, ImportedProver};
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::accept::{
    AcceptInput, AcceptPolicy, AcceptVerdict, Candidate, Check, FactPair, NodeTarget, NodeView,
    Position, PriorityClass, RequiredEvidence, RuleSource, accept_candidate,
};
use rustred_campaign_core::cover::{Dispatch, Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, RhsMode, read_table_file};
use serde_json::Value;

const BANK: &str = "/common/build-cache/vhirschi/p_integral_dev/bank-conv/20261114";
const OUT: &str =
    "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/bank-conv/out/20261114";

/// TEST ONLY: grants Regular for every requested (rule, key) and calls itself an engine fact source.
struct GrantRegular;
impl Regularity for GrantRegular {
    fn regular(&mut self, _: &RuleText, _: &[i32]) -> RegVerdict {
        RegVerdict::Regular
    }
    fn engine_fact_digest(&self) -> Option<String> {
        Some("sha256:test-only-grant".into())
    }
}

fn keys(path: PathBuf) -> Vec<Key> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            let k = if v.is_array() { v } else { v["key"].clone() };
            serde_json::from_value(k).unwrap()
        })
        .collect()
}

#[test]
#[ignore = "large inputs: the T162 bank tables"]
fn imported_bank_rules_reach_imported_plus_fact_only() {
    let prio: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{OUT}/DISPATCH-PRIORITY.json")).unwrap(),
    )
    .unwrap();
    let mut tables = Vec::new();
    let mut files = Vec::new();
    let mut frame = None;
    let mut header = None;
    for t in prio["qualified"].as_array().unwrap() {
        let p = PathBuf::from(BANK).join(t["file"].as_str().unwrap());
        let tf = read_table_file(
            &p,
            ReadOptions {
                rhs: RhsMode::KeysOnly,
            },
        )
        .unwrap();
        if frame.is_none() {
            frame = Some(NodeFrame::from_header(tf.header.as_ref().unwrap()).unwrap());
            header = Some(tf.header.as_ref().unwrap().raw.clone());
        }
        // the IN-FORCE dispatch: witness check off (records without loci would block every key); every credit
        // is decided below by the chain per rule with per-key facts (addendum 3)
        let opts = DomainOptions {
            witness: false,
            ..DomainOptions::default()
        };
        tables.push(
            Table::from_table_file(
                t["origin"].as_str().unwrap(),
                p.display().to_string(),
                &tf,
                opts,
            )
            .unwrap(),
        );
        files.push(p);
    }
    let frame = frame.unwrap();
    let cmp = NodeComparator::from_header(header.as_ref().unwrap()).unwrap();
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect()).unwrap();
    let k3 = accept_common::fixture_root().join("small/k3/T162");
    let covered = keys(k3.join("F1_covered_sample2000.jsonl"));
    let gaps = keys(k3.join("F1_uncovered_990.jsonl"));
    // winners of the sampled covered keys, grouped by rule
    let mut by_rule: BTreeMap<(usize, usize), Vec<Key>> = BTreeMap::new();
    for k in &covered {
        match disp.first_match(k).unwrap() {
            Dispatch::Covered(r) => by_rule
                .entry((r.table, r.record))
                .or_default()
                .push(k.clone()),
            other => panic!("sampled covered key {k:?} -> {other:?}"),
        }
    }
    for g in &gaps {
        assert!(
            disp.first_match(g).unwrap().is_open(),
            "gap key {g:?} covered"
        );
    }
    let imports = ImportedEvidence::read(
        &std::fs::read_to_string(format!("{OUT}/ACCEPT/T162.bank.accept.jsonl")).unwrap(),
    )
    .unwrap();
    assert_eq!(imports.len(), 2448);
    // rule lines per (table, record)
    let mut lines: BTreeMap<(usize, usize), (u64, Vec<u8>)> = BTreeMap::new();
    for (ti, p) in files.iter().enumerate() {
        let text = std::fs::read(p).unwrap();
        let (mut off, mut rec) = (0u64, 0usize);
        for l in text.split_inclusive(|&b| b == b'\n') {
            if l.starts_with(br#"{"type":"rule""#) || l.starts_with(br#"{"type": "rule""#) {
                if by_rule.contains_key(&(ti, rec)) {
                    lines.insert(
                        (ti, rec),
                        (off, l.strip_suffix(b"\n").unwrap_or(l).to_vec()),
                    );
                }
                rec += 1;
            }
            off += l.len() as u64;
        }
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut keys_imported = 0usize;
    for ((ti, rec), served) in &by_rule {
        let (off, line) = &lines[&(*ti, *rec)];
        let rule = RuleText::parse(line, &frame).unwrap();
        assert!(
            imports.get(&rule).is_some(),
            "no import record for table {ti} record {rec}"
        );
        let input = AcceptInput {
            node: NodeView {
                node: "T162".into(),
                frame: &frame,
                cmp: &cmp,
                target: NodeTarget::Boxed {
                    box_id: "R*".into(),
                },
                required: RequiredEvidence::PerKey,
                zero_sectors: None,
                terminals: Default::default(),
            },
            before: None,
            after: None,
            candidate: Candidate {
                rule: &rule,
                source: RuleSource {
                    table_id: format!("bank.{ti}"),
                    table_path: files[*ti].display().to_string(),
                    table_digest: String::new(),
                    ordinal: *rec as u64,
                    byte_offset: Some(*off),
                    producer: None,
                },
                position: Position {
                    class: PriorityClass::Input,
                    input_priority: *ti as u32,
                    round: 0,
                    item: format!("bank.{ti}"),
                    index: *rec as u32,
                },
            },
            gap_points: served,
            working: served,
            policy: AcceptPolicy::default(),
        };
        let mut facts = FactPair {
            regularity: GrantRegular,
            coefficients: ExactRecordFacts::new(),
        };
        let mut prover = ImportedProver {
            imports: imports.clone(),
        };
        let v = accept_candidate(&input, &mut facts, &mut prover).unwrap();
        let tag = match &v {
            AcceptVerdict::Accepted { .. } => "ACCEPTED".to_string(),
            AcceptVerdict::Undecided { check, missing } => format!(
                "undecided {check:?}: {}",
                missing[0].what.chars().take(60).collect::<String>()
            ),
            AcceptVerdict::Rejected { check, witness } => {
                format!("rejected {check:?}: {witness:?}")
            }
            AcceptVerdict::NeedsExternal { check, .. } => format!("needs external {check:?}"),
        };
        assert!(
            !v.is_accepted(),
            "an imported label must never accept at per-key: {tag}"
        );
        if matches!(
            v,
            AcceptVerdict::Undecided {
                check: Check::Evidence,
                ..
            }
        ) {
            keys_imported += served.len();
        }
        *counts.entry(tag).or_default() += 1;
    }
    println!(
        "bank: {} sampled covered keys, {} winning rules, {} gap keys open",
        covered.len(),
        by_rule.len(),
        gaps.len()
    );
    println!("keys at 'imported + regular fact' (Undecided at Evidence): {keys_imported}");
    for (t, c) in &counts {
        println!("  {c:5} {t}");
    }
}
