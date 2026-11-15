//! Indexed first match: the SAME dispatch as [`Dispatcher::first_match`] (tables in priority, records in order, the
//! first record whose stage is not `NotSelected` ends the search; common chart first), with a slot trie over the
//! fixed slots for the candidate list instead of one hash lookup per fixed-slot pattern (large tables have thousands
//! of patterns). Differentially tested against `Dispatcher::first_match` (randomized: tests/closure_fast_differential.rs,
//! default suite; full key sets of the large fixtures: ignored release tests).

use crate::cover::{Dispatch, Dispatcher, RuleRef};
use crate::domain::{Stage, Verdict};

const NONE: u32 = u32::MAX;

#[derive(Clone, Debug, Default)]
struct Node {
    vals: Vec<(i32, u32)>,
    wild: u32,
    leaf: Vec<u32>,
}

/// The dispatcher's records flattened in dispatch order, with a trie over their fixed slots.
pub struct FastDispatch<'a> {
    disp: &'a Dispatcher,
    /// position -> (table, record)
    entries: Vec<(u32, u32)>,
    /// table -> position of its record 0
    offsets: Vec<u32>,
    /// per position: the core's witness-status block of the record (`RecordMeta::witness_block`), applied when the
    /// record is selected (the core dispatcher's enforced verdict, re-audit RA-MAJOR-1)
    block: Vec<Option<Verdict>>,
    nodes: Vec<Node>,
}

impl<'a> FastDispatch<'a> {
    pub fn new(disp: &'a Dispatcher) -> FastDispatch<'a> {
        let tables = disp.tables();
        let mut entries = vec![];
        let mut offsets = vec![0u32; tables.len()];
        for &t in disp.priority() {
            offsets[t] = entries.len() as u32;
            for r in 0..tables[t].len() {
                entries.push((t as u32, r as u32));
            }
        }
        let n = disp.arity();
        let mut nodes = vec![Node {
            wild: NONE,
            ..Default::default()
        }];
        for (e, &(t, r)) in entries.iter().enumerate() {
            let d = &tables[t as usize].domains[r as usize];
            let mut cur = 0usize;
            for s in 0..n {
                let next = match d.fixed[s] {
                    Some(v) => match nodes[cur].vals.iter().find(|x| x.0 == v) {
                        Some(x) => x.1 as usize,
                        None => {
                            let id = nodes.len();
                            nodes.push(Node {
                                wild: NONE,
                                ..Default::default()
                            });
                            nodes[cur].vals.push((v, id as u32));
                            id
                        }
                    },
                    None => {
                        if nodes[cur].wild != NONE {
                            nodes[cur].wild as usize
                        } else {
                            let id = nodes.len();
                            nodes.push(Node {
                                wild: NONE,
                                ..Default::default()
                            });
                            nodes[cur].wild = id as u32;
                            id
                        }
                    }
                };
                cur = next;
            }
            nodes[cur].leaf.push(e as u32);
        }
        let block = entries
            .iter()
            .map(|&(t, r)| {
                tables[t as usize]
                    .meta
                    .get(r as usize)
                    .and_then(|m| m.witness_block())
            })
            .collect();
        FastDispatch {
            disp,
            entries,
            offsets,
            block,
            nodes,
        }
    }

    pub fn dispatcher(&self) -> &'a Dispatcher {
        self.disp
    }
    pub fn n_entries(&self) -> usize {
        self.entries.len()
    }
    /// Dispatch position of a rule.
    #[inline]
    pub fn position(&self, r: RuleRef) -> u32 {
        self.offsets[r.table] + r.record as u32
    }
    /// Rule at a dispatch position.
    #[inline]
    pub fn rule(&self, pos: u32) -> RuleRef {
        let (t, r) = self.entries[pos as usize];
        self.disp.rule_ref(t as usize, r as usize)
    }

    /// Candidate positions (records whose fixed slots all match), ascending.
    pub fn candidates(&self, key: &[i32], out: &mut Vec<u32>, stack: &mut Vec<(u32, u32)>) {
        out.clear();
        stack.clear();
        let n = key.len();
        stack.push((0, 0));
        while let Some((node, s)) = stack.pop() {
            let nd = &self.nodes[node as usize];
            if s as usize == n {
                out.extend_from_slice(&nd.leaf);
                continue;
            }
            let v = key[s as usize];
            if let Some(x) = nd.vals.iter().find(|x| x.0 == v) {
                stack.push((x.1, s + 1));
            }
            if nd.wild != NONE {
                stack.push((nd.wild, s + 1));
            }
        }
        out.sort_unstable();
    }

    /// First match (= `Dispatcher::first_match`).
    pub fn first_match_with(
        &self,
        key: &[i32],
        buf: &mut Vec<u32>,
        stack: &mut Vec<(u32, u32)>,
    ) -> Dispatch {
        self.first_match_ex(key, 0, None, &[], buf, stack).0
    }

    /// First match where every REUSABLE position below `old_stop` is known `NotSelected` (incremental closure): only
    /// candidates that are not reusable (`is_new(pos)`) or at/after `old_stop` are evaluated.
    pub fn first_match_cached(
        &self,
        key: &[i32],
        old_stop: u32,
        is_new: &dyn Fn(u32) -> bool,
        buf: &mut Vec<u32>,
        stack: &mut Vec<(u32, u32)>,
    ) -> (Dispatch, usize) {
        let reusable = |p: u32| !is_new(p);
        self.first_match_ex(key, old_stop, Some(&reusable), &[], buf, stack)
    }

    /// The general first match: positions in `excluded` (sorted; rule-level key exclusions at THIS key) are skipped
    /// as if absent (the next record serves); with `reusable`, a reusable position below `old_stop` is skipped as
    /// known `NotSelected` (incremental use; the caller guarantees it). Returns the dispatch and the number of
    /// records evaluated.
    pub fn first_match_ex(
        &self,
        key: &[i32],
        old_stop: u32,
        reusable: Option<&dyn Fn(u32) -> bool>,
        excluded: &[u32],
        buf: &mut Vec<u32>,
        stack: &mut Vec<(u32, u32)>,
    ) -> (Dispatch, usize) {
        if let Some(mask) = self.disp.chart()
            && let Some(slot) = key
                .iter()
                .enumerate()
                .position(|(i, &x)| (mask[i] && x < 1) || (!mask[i] && x > 0))
        {
            return (Dispatch::OutsideChart { slot }, 0);
        }
        self.candidates(key, buf, stack);
        let mut evaluated = 0usize;
        for &e in buf.iter() {
            if let Some(r) = reusable
                && e < old_stop
                && r(e)
            {
                continue;
            }
            if !excluded.is_empty() && excluded.binary_search(&e).is_ok() {
                continue;
            }
            evaluated += 1;
            let (t, r) = self.entries[e as usize];
            let mut v = self.disp.tables()[t as usize].domains[r as usize].applies(key);
            if v.stage() == Stage::Applies
                && let Some(b) = &self.block[e as usize]
            {
                v = b.clone();
            }
            match v.stage() {
                Stage::Applies => return (Dispatch::Covered(self.rule(e)), evaluated),
                Stage::NotSelected => {}
                Stage::Blocked => {
                    return (
                        Dispatch::Blocked {
                            at: self.rule(e),
                            reason: v,
                        },
                        evaluated,
                    );
                }
                Stage::Undecided => {
                    return (
                        Dispatch::Undecided {
                            at: self.rule(e),
                            verdict: v,
                        },
                        evaluated,
                    );
                }
            }
        }
        (Dispatch::Uncovered, evaluated)
    }

    pub fn memory_bytes(&self) -> usize {
        self.entries.capacity() * 8
            + self.nodes.capacity() * std::mem::size_of::<Node>()
            + self
                .nodes
                .iter()
                .map(|n| n.vals.capacity() * 8 + n.leaf.capacity() * 4)
                .sum::<usize>()
    }
}
