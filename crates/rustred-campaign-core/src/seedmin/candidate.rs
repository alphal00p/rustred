//! SMALL EXPLICIT RULE candidate (interfaces/seedmin.md 7; TRICKS 7): for a minimised list valid on a domain, the
//! per-key relation sizes on the UNSERVED sub-face only (generic face rules can have their exception locus exactly
//! there), and the hand-over to the repair flow's EXISTING size decision (`repairflow::judge::judge`): this module
//! measures, it does not decide.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::joint::Piece;
use super::{Key, list_digest};

/// Sizes of one key's relation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySize {
    pub key: Key,
    pub terms: u64,
    pub rhs_top: Option<u64>,
    pub rhs_sub: Option<u64>,
    pub bytes: Option<u64>,
    pub trace_rows: Option<u64>,
}

/// The candidate of one piece on the unserved sub-face.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCandidate {
    pub piece: String,
    pub list_sha256: String,
    pub seeds: usize,
    /// Keys of the unserved sub-face the piece serves (the sizes are judged there only).
    pub keys: Vec<KeySize>,
    /// Unserved sub-face keys the piece does not serve.
    pub not_in_piece: usize,
    /// Maxima over `keys`, in the names of the repair flow's `judge::Measures` (monomials: not known from point
    /// relations: null).
    pub measures: Value,
    pub totals: Value,
}

/// Measure `piece` on the unserved sub-face `unserved` (keys outside the piece are counted, not measured).
pub fn rule_candidate(piece: &Piece, unserved: &[Key]) -> RuleCandidate {
    let mut keys = vec![];
    let mut not_in = 0;
    for k in unserved {
        match piece.keys.iter().position(|x| x == k) {
            Some(i) => {
                let f = &piece.per_key[i];
                keys.push(KeySize {
                    key: k.clone(),
                    terms: f.rhs_count,
                    rhs_top: f.rhs_sub_count.map(|s| f.rhs_count.saturating_sub(s)),
                    rhs_sub: f.rhs_sub_count,
                    bytes: f.bytes,
                    trace_rows: f.trace_rows,
                });
            }
            None => not_in += 1,
        }
    }
    let max = |g: &dyn Fn(&KeySize) -> Option<u64>| keys.iter().filter_map(g).max();
    let sum = |g: &dyn Fn(&KeySize) -> Option<u64>| keys.iter().filter_map(g).sum::<u64>();
    RuleCandidate {
        piece: piece.label.clone(),
        list_sha256: list_digest(&piece.list),
        seeds: piece.list.len(),
        not_in_piece: not_in,
        measures: json!({"rhs_terms": max(&|s| Some(s.terms)), "bytes": max(&|s| s.bytes),
            "trace_rows": max(&|s| s.trace_rows), "monomials": null, "free_dims": null}),
        totals: json!({"keys": keys.len(), "rhs_terms": sum(&|s| Some(s.terms)), "bytes": sum(&|s| s.bytes)}),
        keys,
    }
}

/// The repair flow's size decision, implemented by the integrator with `repairflow::judge::judge` (branch
/// ac/repairflow): this lane writes no second decision.
pub trait SizeDecider {
    fn decide(&self, c: &RuleCandidate) -> Value;
}
