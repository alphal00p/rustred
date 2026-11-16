//! THE IBP CORE OF A FACE (interfaces/coreseed.md; TRICKS.md items 5, 6, 7, 12, 13, 14): given a set of core keys (the
//! keys of a face that neither the dispatch nor a symmetry route serves: handed in by the caller), find for every key a
//! relation certified by the existing acceptance chain, through ONE generic recipe rule (centre on the key, the mixed
//! shells over all slots, no floor, comparator measure), seed minimisation in distance order with a DOMAIN TEST at
//! every key (a list minimised at one key overfits), an ordered escalation (more shells, shifted centres), a centre
//! scan before a large or singular relation is accepted, the up-centres that cure a spurious d = 4 pole (split recipe by
//! locus), and a joint minimisation over all core keys (small explicit rules).
//!
//! Library only (R22): no process, no file, no clock. Every solve is a request to a [`CoreSolver`] (the supervisor's
//! child solver or a test double); decisions are taken in a fixed order from the answers, so resume (a journal of
//! answers) reproduces the record and the number of CPUs never changes a result. Generic (L8): arity, chart, centres,
//! shells, orders and caps are input or policy data.

pub mod machine;

pub mod seeds;
pub mod spec;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use machine::{CoreReport, solve_core};
pub use seeds::Offset;
pub use spec::{CoreSeedPolicy, VariantSpec};

pub type Key = Vec<i32>;

/// Regularity of a certified relation at the key at the dimension d0 (lane poles owns the verdict).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum D0Verdict {
    Regular,
    Singular { order: Option<u32> },
    Undecided,
}

impl D0Verdict {
    pub fn is_regular(&self) -> bool {
        matches!(self, D0Verdict::Regular)
    }
    pub fn word(&self) -> &'static str {
        match self {
            D0Verdict::Regular => "regular",
            D0Verdict::Singular { .. } => "singular",
            D0Verdict::Undecided => "undecided",
        }
    }
}

/// The narrow call this lane needs from lane poles (`is_regular_at(relation, key, d0)`); `record` is the engine's
/// verified relation record (canonical JSON) at `key`.
pub trait RegularityJudge {
    fn is_regular_at(&self, record: &Value, key: &[i32], d0: &str) -> D0Verdict;
}

/// Until lane poles' function lands: every relation is `Undecided`.
pub struct StubJudge;

impl RegularityJudge for StubJudge {
    fn is_regular_at(&self, _record: &Value, _key: &[i32], _d0: &str) -> D0Verdict {
        D0Verdict::Undecided
    }
}

/// The in-process regularity test of the coreseed flow: THE core text judge ([`crate::poles::textjudge`], lane
/// polefix, audit r2-poles F5; one implementation shared with seedmin's `LocalValuationJudge`): `Regular` iff every
/// coefficient is in the ENGINE's printed form and its exact pole order at the integer d0 is 0 (sound); otherwise
/// `Undecided` (a non-engine form, a pole, a non-integer d0): the ORDER of a singular relation is the `pole-regular-at`
/// child job's (supervisor) or the algebra crate's `ExactJudge` where that crate is linked. Never answers `Singular`.
pub struct DenominatorJudge;

impl RegularityJudge for DenominatorJudge {
    fn is_regular_at(&self, record: &Value, _key: &[i32], d0: &str) -> D0Verdict {
        match crate::poles::textjudge::relation_pole_order(record, d0) {
            Some(0) => D0Verdict::Regular,
            _ => D0Verdict::Undecided,
        }
    }
}

/// One solve request: an explicit recipe (offsets from the key as centres) at a list of keys. Requests of one batch are
/// independent.
#[derive(Clone, Debug, Serialize)]
pub struct SolveReq {
    /// Deterministic label (group / phase / rule / list / trial).
    pub label: String,
    pub recipe: Value,
    /// Canonical text of the recipe + "\n" (the bytes of the recipe file).
    pub text: String,
    pub keys: Vec<Key>,
    /// The engine's relation rule written in the recipe.
    pub relation_rule: String,
}

impl SolveReq {
    /// sha256 of the recipe text and the key list: the journal identity of the request.
    pub fn digest(&self) -> String {
        let mut h = crate::closure::sha256::Sha256::new();
        h.update(self.text.as_bytes());
        for k in &self.keys {
            h.update(format!("{k:?}\n").as_bytes());
        }
        h.finish_hex()
    }
}

/// A relation CERTIFIED by the acceptance chain (domain, exact proof, regular lift, descent under the full node
/// comparator, bound to binary / recipe digest / key).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationInfo {
    pub relation_digest: String,
    pub rhs: Vec<Key>,
    pub rhs_count: u64,
    pub seeds_total: Option<u64>,
    /// Offsets from the key of the seeds with a nonzero proof weight (the support), when the engine reports them.
    pub seeds_used: Option<Vec<Offset>>,
    pub d0: D0Verdict,
    pub recipe_digest: String,
    pub determinism_id: String,
    pub relation_rule_taken: Option<String>,
    /// The engine minimiser's `minimise` member (backend `engine`), verbatim.
    pub minimise: Option<Value>,
    /// sha256 of the verified canonical record line.
    pub record_sha256: String,
}

/// The per-key answer of a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer")]
pub enum KeyAnswer {
    Relation(Box<RelationInfo>),
    /// NO_RELATION_IN_SEEDS: a completed search without the key as pivot (decided).
    NoRelation,
    NotInDomain,
    /// A work or wall cap: UNDECIDED (never "no relation").
    Budget {
        what: String,
    },
    /// ERROR / IRREGULAR / refused / proof failed: undecided, attention.
    Failed {
        class: String,
        message: String,
    },
}

impl KeyAnswer {
    pub fn word(&self) -> &'static str {
        match self {
            KeyAnswer::Relation(_) => "relation",
            KeyAnswer::NoRelation => "no_relation",
            KeyAnswer::NotInDomain => "not_in_domain",
            KeyAnswer::Budget { .. } => "budget",
            KeyAnswer::Failed { .. } => "failed",
        }
    }
    pub fn relation(&self) -> Option<&RelationInfo> {
        match self {
            KeyAnswer::Relation(r) => Some(r),
            _ => None,
        }
    }
}

/// The solver: runs a batch of independent requests, returns per request the answers in key order.
pub trait CoreSolver {
    fn solve(&mut self, batch: &[SolveReq]) -> Result<Vec<Vec<KeyAnswer>>, String>;
    /// Progress hook (partial report after each phase); default: nothing.
    fn progress(&mut self, _phase: &str, _partial: &Value) {}
    /// The CLOSE-FIRST report (stage `close_first`: after A + B, every key served or decided), handed over BEFORE any
    /// request of the variant phase or the lists (POLES_OBJECTIVE.md 1: closing first; audit r2-poles F2). A caller
    /// persists it at once: the later phases only add alternatives and pieces, and their failure, budget or a request
    /// that never returns cannot hold it back. Default: nothing (the final report is returned as before).
    fn publish(&mut self, _report: &CoreReport) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_temporary_judge_reads_normalised_denominators() {
        let j = DenominatorJudge;
        let reg = json!({"rhs": [["I(1)", "(8-3*d)/(-14+6*d)", "top"], ["I(2)", "-1/(-14+6*d)", "top"], ["I(3)", "2", "sub"]]});
        assert_eq!(j.is_regular_at(&reg, &[], "4"), D0Verdict::Regular);
        // a vanishing denominator: never "singular" from this test (it may cancel), only undecided
        let sing = json!({"rhs": [["I(1)", "(8-3*d)/(-8+2*d)", "top"]]});
        assert_eq!(j.is_regular_at(&sing, &[], "4"), D0Verdict::Undecided);
        let big = json!({"rhs": [["I(1)", "(123456789012345678901234567890*d)/(-4+d)", "top"]]});
        assert_eq!(j.is_regular_at(&big, &[], "4"), D0Verdict::Undecided);
        let big_reg =
            json!({"rhs": [["I(1)", "(123456789012345678901234567890*d)/(-5+d)", "top"]]});
        assert_eq!(j.is_regular_at(&big_reg, &[], "4"), D0Verdict::Regular);
        let junk = json!({"rhs": [["I(1)", "x/y/z", "top"]]});
        assert_eq!(j.is_regular_at(&junk, &[], "4"), D0Verdict::Undecided);
        assert_eq!(
            StubJudge.is_regular_at(&reg, &[], "4"),
            D0Verdict::Undecided
        );
    }
}
