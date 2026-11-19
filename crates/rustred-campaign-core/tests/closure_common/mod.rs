//! Shared setup of the closure / symmetry tests: T178 order B (38 pieces in cfg order, bodies from the checker's
//! key files, order-B comparator from the real table header, the box).
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;

use rustred_campaign_core::closure::{KeyCodec, KeySet, KeySetBuilder, RuleBodies};
use rustred_campaign_core::cover::{Dispatcher, Table};
use rustred_campaign_core::order::NodeComparator;

#[path = "../k3_common/mod.rs"]
pub mod k3_common;

pub struct T178 {
    pub names: Vec<String>,
    pub tables: Vec<Table>,
    pub disp: Dispatcher,
    pub bodies: RuleBodies,
    pub cmp: NodeComparator,
    pub box_keys: KeySet,
}

pub fn headers() -> serde_json::Value {
    serde_json::from_str(include_str!("../data/order/headers.json")).unwrap()
}

pub fn keys_dir() -> PathBuf {
    PathBuf::from(k3_common::HO).join("t178-orderB/keys")
}

/// Key file of a piece (R0 has none: its case file is byte-identical to R0_try1's, the checker's identification).
pub fn key_file(name: &str) -> PathBuf {
    let n = if name == "R0" { "R0_try1" } else { name };
    keys_dir().join(format!("{n}.keys.jsonl"))
}

fn key_sums() -> HashMap<String, String> {
    include_str!("../data/closure/t178_keys.sha256")
        .lines()
        .filter_map(|l| {
            l.split_once("  ")
                .map(|(h, f)| (f.to_string(), h.to_string()))
        })
        .collect()
}

/// Load bodies for every table of `disp` named like a T178 piece.
pub fn t178_bodies(disp: &Dispatcher) -> RuleBodies {
    let sums = key_sums();
    let mut b = RuleBodies::new(disp);
    for (t, tab) in disp.tables().iter().enumerate() {
        let p = key_file(&tab.name);
        let bytes = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        let fname = p.file_name().unwrap().to_str().unwrap().to_string();
        assert_eq!(
            k3_common::sha256_hex(&bytes),
            sums[&fname],
            "key file {fname} changed"
        );
        b.load_keys_jsonl(disp, t, BufReader::new(&bytes[..]), 2)
            .unwrap_or_else(|e| panic!("{}: {e:?}", tab.name));
    }
    b
}

pub fn box_set(keys: &[Vec<i32>]) -> KeySet {
    let mut b = KeySetBuilder::new(KeyCodec::new(20, 6).unwrap());
    for k in keys {
        b.push(k).unwrap();
    }
    b.finish()
}

pub fn t178() -> Option<T178> {
    k3_common::oracle()?;
    let (names, tables, _frame) = k3_common::t178_tables()?;
    if !keys_dir().is_dir() {
        crate::inputs::missing(format_args!(
            "SKIP: no T178 key files at {}",
            keys_dir().display()
        ));
        return None;
    }
    let disp = Dispatcher::new(tables.clone(), (0..tables.len()).collect()).unwrap();
    let bodies = t178_bodies(&disp);
    let cmp = NodeComparator::from_header(&headers()["t178B_table"]).unwrap();
    let box_keys = box_set(&k3_common::t178_box());
    Some(T178 {
        names,
        tables,
        disp,
        bodies,
        cmp,
        box_keys,
    })
}

/// The 8 automorphisms of T178 as `GroupElement`s in the checker's element order (img(k,p)[i] = k[p[i]], i.e.
/// `lines` = the inverse permutation of p).
pub fn t178_group() -> Vec<rustred_campaign_core::symmetry::GroupElement> {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../data/closure/t178_group.json")).unwrap();
    v["perms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let p: Vec<usize> = p
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect();
            let mut inv = vec![0; p.len()];
            for (i, &j) in p.iter().enumerate() {
                inv[j] = i;
            }
            rustred_campaign_core::symmetry::GroupElement {
                lines: inv,
                numerators: None,
            }
        })
        .collect()
}

/// Peak resident memory of this process (VmHWM, bytes), if readable.
pub fn peak_rss() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let l = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: u64 = l.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// Answer fact requests as the EC-2 producer does (`rr_proof instantiate`: one instantiate line per (rule, key) with
/// the per-term keys, then one `rustred.campaign.fact.v1` line per answered request) and read them bound to the
/// requests. `spec` = (key, term (None = regularity), value); every spec entry must match a request.
pub fn answer_facts(
    reqs: &[rustred_campaign_core::closure::FactRequest],
    spec: &[(&[i32], Option<usize>, &str)],
    method: &str,
    accept_provisional: bool,
) -> rustred_campaign_core::closure::FileFacts {
    use std::collections::BTreeMap;
    let mut inst: BTreeMap<(i64, Vec<i32>), BTreeMap<usize, Vec<i32>>> = BTreeMap::new();
    for r in reqs {
        let e = inst.entry((r.label, r.key.clone())).or_default();
        if let (Some(t), Some(k)) = (r.term, &r.rhs) {
            e.insert(t, k.clone());
        }
    }
    let mut out = vec![];
    for ((label, key), terms) in &inst {
        let n = terms.keys().max().map(|m| m + 1).unwrap_or(0);
        let rhs: Vec<serde_json::Value> = (0..n)
            .map(|j| {
                serde_json::json!({"term": j, "key": terms.get(&j).cloned().unwrap_or(vec![i32::MIN; key.len()]),
                    "coef": "unknown"})
            })
            .collect();
        out.push(
            serde_json::json!({"schema":"rustred.proof-kernel.instantiate.v1","rule":label,"key":key,"rhs":rhs,
                "in_case":true,"chart_ok":true,"branch":null,"error":null})
                .to_string(),
        );
    }
    for (key, term, value) in spec {
        let r = reqs
            .iter()
            .find(|r| r.key == *key && r.term == *term)
            .unwrap_or_else(|| panic!("no request for {key:?} term {term:?}"));
        let mut v = serde_json::json!({"schema":"rustred.campaign.fact.v1","table":r.table,"record":r.record,
            "label":r.label,"key":key,"kind":r.kind,"value":value,"method":method});
        if let Some(t) = term {
            v["term"] = (*t).into();
        }
        out.push(v.to_string());
    }
    rustred_campaign_core::closure::FileFacts::read_bound(
        out.join("\n").as_bytes(),
        reqs,
        accept_provisional,
    )
    .unwrap()
}
