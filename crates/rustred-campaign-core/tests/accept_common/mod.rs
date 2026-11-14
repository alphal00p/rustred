//! Shared helpers of the acceptance / assembly tests on the coordinator's M2 slice (WS/fixtures/campaign-20261114,
//! interfaces/coord-fixtures.md). Fixture names and numbers appear here as TEST DATA only (L8).
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rustred_campaign_core::accept::facts::{ClosureFacts, ExactRecordFacts};
use rustred_campaign_core::accept::prover::{EvidenceProver, ReplayProver};
use rustred_campaign_core::accept::rule::{RuleText, sha256_tag};
use rustred_campaign_core::accept::{
    AcceptInput, AcceptPolicy, AcceptVerdict, Candidate, NodeTarget, NodeView, Position, PriorityClass,
    RequiredEvidence, RuleSource, accept_candidate,
};
use rustred_campaign_core::closure::{
    ClosureConfig, HandDowns, KeyCodec, KeySet, NoFacts, RuleBodies, Tier, closure,
};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::domain::DomainOptions;
use rustred_campaign_core::ibp_rows::RowsOptions;
use rustred_campaign_core::key::{Key, NodeFrame};
use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::record::{ReadOptions, RhsMode, TableFile, TableHeader, read_table_file};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::Value;

pub const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

pub fn fixture_root() -> PathBuf {
    std::env::var("RUSTRED_CAMPAIGN_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(WS).join("fixtures/campaign-20261114"))
}

pub fn slice(node: &str) -> PathBuf {
    fixture_root().join("small/m2-slice").join(node)
}

pub fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("accept").join(name);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub fn parse_key(s: &str) -> Key {
    let inner = s.trim().strip_prefix("I(").and_then(|x| x.strip_suffix(')')).unwrap();
    inner.split(',').map(|x| x.trim().parse().unwrap()).collect()
}

pub struct Node {
    pub name: String,
    pub dir: PathBuf,
    pub family_text: String,
    pub header: TableHeader,
    pub frame: NodeFrame,
    pub cmp: NodeComparator,
    pub zero_dropped: BTreeSet<u64>,
    pub box_keys: Vec<Key>,
    /// residual records of the closed table (terminals FOUND by generation; gate on these only, U-9)
    pub terminals: Vec<Key>,
    pub expected: Value,
}

pub fn available(node: &str) -> bool {
    slice(node).join("table.jsonl").is_file()
}

pub fn load_node(name: &str) -> Node {
    let dir = slice(name);
    let family_text = std::fs::read_to_string(dir.join("family.sym")).unwrap();
    let tf = read_table_file(dir.join("table.jsonl"), ReadOptions::default()).unwrap();
    let header = tf.header.clone().unwrap();
    let frame = NodeFrame::from_header(&header).unwrap();
    let cmp = NodeComparator::from_header(&header.raw).unwrap();
    let text = std::fs::read_to_string(dir.join("table.jsonl")).unwrap();
    let mut zero_dropped = BTreeSet::new();
    for line in text.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        if v["type"] == "zero_sectors" {
            for m in v["masks"].as_array().unwrap() {
                let m = m.as_str().unwrap();
                let mut dropped = 0u64;
                for (i, c) in m.chars().enumerate() {
                    if frame.is_line(i) && c == '0' {
                        dropped |= 1 << i;
                    }
                }
                zero_dropped.insert(dropped);
            }
        }
    }
    let mut box_keys = Vec::new();
    for line in std::fs::read_to_string(dir.join("base/winners.jsonl")).unwrap().lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        box_keys.push(v[0].as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect());
    }
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("EXPECTED.json")).unwrap()).unwrap();
    let terminals = expected["terminals_found_by_generation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| parse_key(k.as_str().unwrap()))
        .collect();
    Node {
        name: name.into(),
        dir,
        family_text,
        header,
        frame,
        cmp,
        zero_dropped,
        box_keys,
        terminals,
        expected,
    }
}

pub fn ko_entry<'a>(node: &'a Node, ko: &str) -> &'a Value {
    node.expected["ko"].as_array().unwrap().iter().find(|k| k["id"] == ko).unwrap()
}

/// The derived KO table, rebuilt from table.jsonl by deleting the removed rule lines (the fixture's definition,
/// README "Files per node"), checked against the fixture's sha256; written to scratch. Returns the path.
pub fn ko_table(node: &Node, ko: &str) -> PathBuf {
    let e = ko_entry(node, ko);
    let removed: BTreeSet<i64> = e["removed_record_indices"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
    let text = std::fs::read(node.dir.join("table.jsonl")).unwrap();
    let mut out = Vec::new();
    for line in text.split_inclusive(|&b| b == b'\n') {
        let v: Value = serde_json::from_slice(line).unwrap();
        if v["type"] == "rule" && removed.contains(&v["index"].as_i64().unwrap()) {
            continue;
        }
        out.extend_from_slice(line);
    }
    let want = e["derived_table_sha256_uncompressed"].as_str().unwrap();
    assert_eq!(sha256_tag(&out), format!("sha256:{want}"), "{} {ko}: derived table digest", node.name);
    let p = scratch(&node.name).join(format!("{ko}.table.jsonl"));
    std::fs::write(&p, &out).unwrap();
    p
}

pub fn witness_path(node: &str, ko: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/accept/witnesses")
        .join(node)
        .join(ko)
        .join("new_rules.jsonl")
}

/// Witness lines with their byte offsets.
pub fn witness_lines(node: &str, ko: &str) -> Vec<(u64, Vec<u8>)> {
    let p = witness_path(node, ko);
    if !p.is_file() {
        return vec![];
    }
    let text = std::fs::read(p).unwrap();
    let mut off = 0u64;
    let mut out = Vec::new();
    for line in text.split_inclusive(|&b| b == b'\n') {
        let l = line.strip_suffix(b"\n").unwrap_or(line);
        out.push((off, l.to_vec()));
        off += line.len() as u64;
    }
    out
}

pub fn gap_keys(node: &Node, ko: &str) -> Vec<Key> {
    ko_entry(node, ko)["gap"]["gap_keys"].as_array().unwrap().iter().map(|k| parse_key(k.as_str().unwrap())).collect()
}

pub fn load_table(path: &Path, name: &str) -> (TableFile, Table) {
    let tf = read_table_file(path, ReadOptions { rhs: RhsMode::KeysOnly }).unwrap();
    let t = Table::from_table_file(name, path.display().to_string(), &tf, DomainOptions::default()).unwrap();
    (tf, t)
}

pub fn view<'a>(node: &'a Node, target: NodeTarget, required: RequiredEvidence) -> NodeView<'a> {
    NodeView {
        node: node.name.clone(),
        frame: &node.frame,
        cmp: &node.cmp,
        target,
        required,
        zero_sectors: Some(node.zero_dropped.clone()),
        terminals: node.terminals.iter().cloned().collect(),
    }
}

pub fn replay_prover(node: &Node) -> ReplayProver {
    ReplayProver::new(Family::parse(&node.family_text).unwrap(), RowsOptions::default())
}

pub fn position(rule: &RuleText, ko: &str, i: usize) -> Position {
    Position {
        class: if rule.is_point() { PriorityClass::Point } else { PriorityClass::FaceOrLocus },
        input_priority: 0,
        round: 1,
        item: ko.into(),
        index: i as u32,
    }
}

pub fn source(node: &str, ko: &str, off: u64, i: usize) -> RuleSource {
    RuleSource {
        table_id: format!("{node}/{ko}/witness"),
        table_path: witness_path(node, ko).display().to_string(),
        table_digest: sha256_tag(&std::fs::read(witness_path(node, ko)).unwrap()),
        ordinal: i as u64,
        byte_offset: Some(off),
        producer: None,
    }
}

/// Tier K closure key set of the box under the given tables (box + envelope), terminals declared.
pub fn working_range(node: &Node, disp: &Dispatcher, bodies: &RuleBodies) -> Vec<Key> {
    let codec = KeyCodec::new(node.frame.arity(), 8).unwrap();
    let start = KeySet::from_keys(codec, node.box_keys.iter().map(|k| k.as_slice())).unwrap();
    let cfg = ClosureConfig {
        tier: Tier::K,
        threads: 2,
        hand_downs: HandDowns::Off,
        longest_chain: false,
        zero_sectors: Some(node.zero_dropped.clone()),
        ..Default::default()
    };
    let c = closure(disp, bodies, &node.cmp, &start, &cfg, &NoFacts).unwrap();
    c.keys().iter().collect()
}

/// Result of the acceptance pipeline on one KO set.
pub struct KoRun {
    pub ko_path: PathBuf,
    pub accepted: Vec<(usize, u64, RuleText, AcceptVerdict)>,
    pub rejected: Vec<(usize, AcceptVerdict)>,
    pub working: Vec<Key>,
    pub gaps: Vec<Key>,
}

/// Build a manual table of accepted rule domains.
pub fn accepted_table(arity: usize, rules: &[&RuleText]) -> Table {
    let mut t = Table::new("campaign", arity);
    for (i, r) in rules.iter().enumerate() {
        t.push(i as i64, r.domain.clone().unwrap()).unwrap();
    }
    t
}

/// The acceptance pipeline of a KO set: the witnesses are offered in file order, each judged by the chain against
/// the KO table plus the witnesses accepted before it.
pub fn run_ko(node: &Node, ko: &str, prover: &mut dyn EvidenceProver, required: RequiredEvidence) -> KoRun {
    let ko_path = ko_table(node, ko);
    let (tf, ko_t) = load_table(&ko_path, "ko");
    let lines = witness_lines(&node.name, ko);
    let rules: Vec<RuleText> = lines.iter().map(|(_, l)| RuleText::parse(l, &node.frame).unwrap()).collect();
    // working range: closure of the box under KO + all witnesses
    let all = Dispatcher::new(
        vec![ko_t.clone(), accepted_table(node.frame.arity(), &rules.iter().collect::<Vec<_>>())],
        vec![0, 1],
    )
    .unwrap();
    let mut bodies = RuleBodies::new(&all);
    bodies.load_table_records(&all, 0, &tf).unwrap();
    for (i, r) in rules.iter().enumerate() {
        let pats: Vec<&str> = r.rhs.iter().map(|(k, _)| k.as_str()).collect();
        bodies.set(&all, 1, i, &pats).unwrap();
    }
    let working = working_range(node, &all, &bodies);
    let gaps = gap_keys(node, ko);
    let mut facts = ExactRecordFacts::new();
    let mut accepted: Vec<(usize, u64, RuleText, AcceptVerdict)> = Vec::new();
    let mut rejected = Vec::new();
    for (i, ((off, _), r)) in lines.iter().zip(&rules).enumerate() {
        let acc: Vec<&RuleText> = accepted.iter().map(|(_, _, r, _)| r).collect();
        let before = Dispatcher::new(vec![ko_t.clone(), accepted_table(node.frame.arity(), &acc)], vec![0, 1]).unwrap();
        let input = AcceptInput {
            node: view(node, NodeTarget::Boxed { box_id: "fixture-box".into() }, required),
            before: Some(&before),
            after: None,
            candidate: Candidate {
                rule: r,
                source: source(&node.name, ko, *off, i),
                position: position(r, ko, i),
            },
            gap_points: &gaps,
            working: &working,
            policy: AcceptPolicy::default(),
        };
        let v = accept_candidate(&input, &mut facts, prover).unwrap();
        if v.is_accepted() {
            accepted.push((i, *off, r.clone(), v));
        } else {
            rejected.push((i, v));
        }
    }
    KoRun {
        ko_path,
        accepted,
        rejected,
        working,
        gaps,
    }
}

/// Closure facts for every record of the given tables of a dispatcher (table name, record) -> rule text.
pub fn closure_facts(frame: &NodeFrame, tables: &[(&str, &Path, Option<&[RuleText]>)]) -> ClosureFacts {
    let mut cf = ClosureFacts::new();
    for (name, path, given) in tables {
        match given {
            Some(rs) => {
                for (i, r) in rs.iter().enumerate() {
                    cf.insert(name, i, r.clone());
                }
            }
            None => {
                let text = std::fs::read(path).unwrap();
                let mut i = 0;
                for line in text.split(|&b| b == b'\n') {
                    if line.starts_with(br#"{"type":"rule""#) {
                        cf.insert(name, i, RuleText::parse(line, frame).unwrap());
                        i += 1;
                    }
                }
            }
        }
    }
    cf
}
