//! Forcer notation -> `PTopo`, and the Forcer fixtures of `tests/fixtures/forcer/` (benruijl/forcer 68a30bf).
//!
//! Forcer notation: `a-b` is one line, every `*` one eps unit on it; the lines at vertex 0 and at the highest vertex
//! are the two external legs (their eps units are the eps power of `q`, not part of a topology).

#![allow(dead_code)]

use std::collections::BTreeMap;

use rustred_topology::{GraphKey, Normalised, PTopo, canonical, normalise};

pub const FORCER_DAG_TSV: &str = include_str!("../fixtures/forcer/forcer_dag_68a30bf.tsv");
pub const FORCER_TOPS_TSV: &str = include_str!("../fixtures/forcer/forcer_tops_68a30bf.tsv");
pub const FORCER_FIXED_TSV: &str = include_str!("../fixtures/forcer/forcer_fixed_68a30bf.tsv");
pub const GRAPH_KEYS_TSV_PATH: &str = "tests/fixtures/forcer/graph_keys_p_topology_v1.tsv";

/// A topology in Forcer notation: the `PTopo` of its internal lines (vertices renumbered compactly in order of first
/// appearance, the leg ends being `s` and `t`) and the eps units on `q` (the legs' stars).
#[derive(Clone, Debug)]
pub struct ForcerTopology {
    pub topo: PTopo,
    pub eps_on_q: u8,
}

pub fn parse_forcer_topology(text: &str) -> ForcerTopology {
    let lines: Vec<(u32, u32, u8)> = text
        .split_whitespace()
        .map(|tok| {
            let stars = tok.chars().filter(|&c| c == '*').count() as u8;
            let (a, b) = tok.trim_end_matches('*').split_once('-').expect("line a-b");
            (
                a.parse().expect("vertex"),
                b.parse().expect("vertex"),
                stars,
            )
        })
        .collect();
    let vmax = lines
        .iter()
        .map(|&(a, b, _)| a.max(b))
        .max()
        .expect("at least one line");
    let is_leg = |&(a, b, _): &(u32, u32, u8)| a == 0 || b == 0 || a == vmax || b == vmax;
    let eps_on_q = lines.iter().filter(|l| is_leg(l)).map(|l| l.2).sum();
    let internal: Vec<(u32, u32, u8)> = lines.iter().copied().filter(|l| !is_leg(l)).collect();
    if internal.is_empty() {
        return ForcerTopology {
            topo: PTopo::trivial(),
            eps_on_q,
        };
    }
    let leg_end = |outer: u32| {
        lines
            .iter()
            .find(|&&(a, b, _)| a == outer || b == outer)
            .map(|&(a, b, _)| if a == outer { b } else { a })
            .expect("leg present")
    };
    let (s, t) = (leg_end(0), leg_end(vmax));
    let mut ids: BTreeMap<u32, u8> = BTreeMap::new();
    let id = |v: u32, ids: &mut BTreeMap<u32, u8>| {
        let next = ids.len() as u8;
        *ids.entry(v).or_insert(next)
    };
    let s = id(s, &mut ids);
    let t = id(t, &mut ids);
    let mapped: Vec<(u8, u8, u8)> = internal
        .iter()
        .map(|&(a, b, e)| (id(a, &mut ids), id(b, &mut ids), e))
        .collect();
    ForcerTopology {
        topo: PTopo::new(ids.len() as u8, s, t, mapped),
        eps_on_q,
    }
}

/// One row of `forcer_dag_68a30bf.tsv`.
#[derive(Clone, Debug)]
pub struct ForcerNode {
    pub name: String,
    pub loops: u8,
    pub action: String,
    pub topology: ForcerTopology,
    /// change-topology targets (triangle / diamond / custom / trivial rows only)
    pub children: Vec<String>,
}

fn rows(tsv: &str) -> impl Iterator<Item = Vec<&str>> {
    tsv.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').collect())
}

/// The 438 topologies of Forcer's 4L reduction tree.
pub fn forcer_dag() -> Vec<ForcerNode> {
    rows(FORCER_DAG_TSV)
        .map(|c| ForcerNode {
            name: c[0].to_string(),
            loops: c[1].parse().expect("L"),
            action: c[2].to_string(),
            topology: parse_forcer_topology(c[3]),
            children: c
                .get(4)
                .filter(|x| !x.is_empty())
                .map(|x| x.split(',').map(str::to_string).collect())
                .unwrap_or_default(),
        })
        .collect()
}

/// Forcer's top-level diagrams with `loops` loops (2L 1, 3L 3, 4L 11, 5L 64): (name, topology).
pub fn forcer_tops(loops: u8) -> Vec<(String, PTopo)> {
    rows(FORCER_TOPS_TSV)
        .filter(|c| c[0].parse::<u8>().expect("L") == loops)
        .map(|c| (c[1].to_string(), parse_forcer_topology(c[2]).topo))
        .collect()
}

/// Forcer's 18 fixed topologies (createFixedTopologies(4)): (name, L, topology).
pub fn forcer_fixed() -> Vec<(String, u8, PTopo)> {
    rows(FORCER_FIXED_TSV)
        .map(|c| {
            (
                c[1].to_string(),
                c[0].parse().expect("L"),
                parse_forcer_topology(c[2]).topo,
            )
        })
        .collect()
}

/// Key of a topology that is already in Forcer's normal form (asserts that normalising changes nothing).
pub fn key_of_normal_form(t: &PTopo) -> GraphKey {
    match normalise(t) {
        Normalised::Trivial => canonical(&PTopo::trivial()).0,
        Normalised::Topo(p, fates) => {
            assert!(
                fates
                    .iter()
                    .enumerate()
                    .all(|(i, f)| *f == rustred_topology::Fate::Kept(i as u8)),
                "Forcer topology {t:?} is not in normal form: {fates:?}"
            );
            canonical(&p).0
        }
        Normalised::Zero => panic!("Forcer topology {t:?} is zero"),
    }
}

/// name -> key for the 438 Forcer topologies, and the reverse map (asserting the keys are distinct).
pub fn forcer_names_by_key(nodes: &[ForcerNode]) -> BTreeMap<GraphKey, String> {
    let mut map = BTreeMap::new();
    for n in nodes {
        let key = key_of_normal_form(&n.topology.topo);
        if let Some(other) = map.insert(key, n.name.clone()) {
            panic!("Forcer topologies {other} and {} have the same key", n.name);
        }
    }
    map
}

/// Every nonzero integer sector class of `tops`: the closure under contracting single lines (eager Demand roots,
/// the census `eall` mode).
pub fn sector_classes(tops: &[PTopo]) -> Vec<PTopo> {
    let mut seen: BTreeMap<GraphKey, PTopo> = BTreeMap::new();
    let mut work: Vec<PTopo> = Vec::new();
    let push =
        |t: &PTopo, seen: &mut BTreeMap<GraphKey, PTopo>, work: &mut Vec<PTopo>| match normalise(t)
        {
            Normalised::Topo(p, _) => {
                let (key, canon) = canonical(&p);
                if let std::collections::btree_map::Entry::Vacant(slot) = seen.entry(key) {
                    let c = canon.apply(&p);
                    slot.insert(c.clone());
                    work.push(c);
                }
            }
            Normalised::Trivial => {
                let (key, _) = canonical(&PTopo::trivial());
                seen.entry(key).or_insert_with(PTopo::trivial);
            }
            Normalised::Zero => {}
        };
    for t in tops {
        push(t, &mut seen, &mut work);
    }
    while let Some(t) = work.pop() {
        for i in 0..t.lines.len() {
            let (a, b, _) = t.lines[i];
            let lines: Vec<(u8, u8, u8)> = t
                .lines
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, &(u, v, e))| (if u == b { a } else { u }, if v == b { a } else { v }, e))
                .collect();
            let s = if t.s == b { a } else { t.s };
            let tt = if t.t == b { a } else { t.t };
            push(&PTopo::new(t.n, s, tt, lines), &mut seen, &mut work);
        }
    }
    seen.into_values().collect()
}
