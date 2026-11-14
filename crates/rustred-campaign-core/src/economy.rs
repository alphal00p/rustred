//! Economy of rules (plan 2.4.14 (a), (b); R31, R33, U-13; package WP-I15 measure part): the per-rule and per-node
//! measures with a total order, the selection score of the seed-search rung, the "supersedes" relation and
//! retirement records (a FULL node's retirement cannot be built without a containment proof, R33).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Selection score among candidate covers of one item (lexicographic, smaller = better): gap points failed, cases,
/// RHS SIZE CLASS (bit length of the RHS term count: a factor-2 band), then PREFERRED-terminal misses (P-4: more RHS
/// keys on preferred terminals is better; only among candidates of the same class, so a preference never buys a
/// rule twice as large, and never acceptance: only accepted candidates are scored), then exact RHS terms,
/// monomials, coefficient degree, enumeration order (determinism).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SelectionScore {
    pub failed: u64,
    pub cases: u64,
    pub size_class: u32,
    /// `u64::MAX - preferred hits`.
    pub preferred_misses: u64,
    pub rhs_terms: u64,
    pub monomials: u64,
    pub degree: u32,
    pub order: u64,
}

impl SelectionScore {
    /// Factor-2 size band of an RHS term count (0 for 0 terms).
    pub fn size_class_of(rhs_terms: u64) -> u32 {
        64 - rhs_terms.leading_zeros()
    }
}

/// Measure of one rule record (rr_node `table_dump.v1` rule line).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleMeasure {
    pub cases: u64,
    pub rhs_terms: u64,
    pub monomials: u64,
    /// The record had no `monomials` field: counted lexically from the coefficient strings (top-level `+`/`-`
    /// separators), never by parsing coefficients.
    pub monomials_estimated: bool,
    pub max_degree: Option<u32>,
    pub served_keys: Option<u64>,
    pub bytes: u64,
}

fn lexical_monomials(coef: &str) -> u64 {
    let mut depth = 0i32;
    let mut n = 1u64;
    let mut prev: Option<char> = None;
    for ch in coef.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-'
                if depth == 0
                    && !matches!(
                        prev,
                        None | Some('(')
                            | Some('*')
                            | Some('/')
                            | Some('^')
                            | Some('e')
                            | Some('E')
                    ) =>
            {
                n += 1;
            }
            _ => {}
        }
        if !ch.is_whitespace() {
            prev = Some(ch);
        }
    }
    n
}

impl RuleMeasure {
    /// Measure from one JSON rule line. Uses `monomials` when the record carries it (then coefficient text is not
    /// touched); `degree` / `max_degree` fields when present.
    pub fn from_record_json(line: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let rhs = v.get("rhs").and_then(|r| r.as_array());
        let rhs_terms = rhs.map_or(0, |r| r.len() as u64);
        let (monomials, est) = match v.get("monomials").and_then(|m| m.as_u64()) {
            Some(m) => (m, false),
            None => {
                let m = rhs.map_or(0, |r| {
                    r.iter()
                        .filter_map(|t| t.get(1).and_then(|c| c.as_str()))
                        .map(lexical_monomials)
                        .sum()
                });
                (m, true)
            }
        };
        let max_degree = v
            .get("max_degree")
            .or_else(|| v.get("degree"))
            .and_then(|d| d.as_u64())
            .map(|d| d as u32);
        Ok(RuleMeasure {
            cases: 1,
            rhs_terms,
            monomials,
            monomials_estimated: est,
            max_degree,
            served_keys: None,
            bytes: line.len() as u64,
        })
    }
}

/// Measure of one node's table (2.4.14 (a)).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeMeasure {
    pub cases: u64,
    pub rules: u64,
    pub rhs_terms: u64,
    pub monomials: u64,
    pub max_degree: Option<u32>,
    pub used_rules: u64,
    pub used_rhs_terms: u64,
    pub closure_inside: Option<u64>,
    pub closure_outside: Option<u64>,
    pub bytes: u64,
}

impl NodeMeasure {
    /// Streaming accumulation: add one rule (used = fires on at least one key of the required set).
    pub fn add(&mut self, r: &RuleMeasure, used: bool) {
        self.cases += r.cases;
        self.rules += 1;
        self.rhs_terms += r.rhs_terms;
        self.monomials += r.monomials;
        self.max_degree = match (self.max_degree, r.max_degree) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        if used {
            self.used_rules += 1;
            self.used_rhs_terms += r.rhs_terms;
        }
        self.bytes += r.bytes;
    }

    /// Used share by rules (used / rules), in [0, 1]; 0 for an empty table.
    pub fn used_share(&self) -> f64 {
        if self.rules == 0 {
            0.0
        } else {
            self.used_rules as f64 / self.rules as f64
        }
    }

    /// Used share by RHS volume.
    pub fn used_volume_share(&self) -> f64 {
        if self.rhs_terms == 0 {
            0.0
        } else {
            self.used_rhs_terms as f64 / self.rhs_terms as f64
        }
    }

    /// Total order, smaller = more economical: cases, rules, RHS terms, monomials, coefficient degree, used share
    /// (HIGHER is better, compared in parts per million), closure size, bytes.
    pub fn economy_cmp(&self, o: &NodeMeasure) -> Ordering {
        let share = |m: &NodeMeasure| (m.used_rules * 1_000_000).checked_div(m.rules).unwrap_or(0);
        let closure =
            |m: &NodeMeasure| m.closure_inside.unwrap_or(0) + m.closure_outside.unwrap_or(0);
        self.cases
            .cmp(&o.cases)
            .then(self.rules.cmp(&o.rules))
            .then(self.rhs_terms.cmp(&o.rhs_terms))
            .then(self.monomials.cmp(&o.monomials))
            .then(self.max_degree.unwrap_or(0).cmp(&o.max_degree.unwrap_or(0)))
            .then(share(o).cmp(&share(self)))
            .then(closure(self).cmp(&closure(o)))
            .then(self.bytes.cmp(&o.bytes))
    }
}

/// Coverage state of a node (2.4.15).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageState {
    Full,
    Boxed,
    Open,
}

/// Proof that the covered domain did not shrink (R33): produced by the exact partition (lane I-PART), which states
/// that the partition of the new table set reports no gap where the old one reported none. Only that lane should
/// construct it; this crate treats it as opaque evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainmentProof {
    pub old_tables_digest: String,
    pub new_tables_digest: String,
    /// "exact_partition"
    pub method: String,
    pub artefact_sha256: String,
}

/// Why a rule may supersede others.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupersedeReason {
    /// A face (or locus) rule now serves the keys of point rules (winner diff).
    FaceRetiresPoints,
    /// A cheaper rule with the same or a larger domain.
    CheaperSameOrLargerDomain,
}

/// A retirement record `rule_retired {node, rule, by}`. Fields are private: a FULL node's retirement can only be
/// built with a [`ContainmentProof`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retirement {
    node: String,
    retired: Vec<String>,
    by: String,
    reason: SupersedeReason,
    state: CoverageState,
    proof: Option<ContainmentProof>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetireError {
    /// R33: on a FULL node a retirement needs a containment proof.
    FullNodeNeedsProof,
    /// The new rule is not more economical than the retired ones.
    NotCheaper,
    /// Nothing to retire.
    Empty,
}

impl Retirement {
    /// Build a retirement: `new` must be strictly more economical than the `old` rules together (cases, then RHS
    /// terms, then monomials), and on a FULL node `proof` must be present.
    pub fn new(
        node: &str,
        state: CoverageState,
        old: &[(String, RuleMeasure)],
        new: (&str, &RuleMeasure),
        reason: SupersedeReason,
        proof: Option<ContainmentProof>,
    ) -> Result<Self, RetireError> {
        if old.is_empty() {
            return Err(RetireError::Empty);
        }
        if state == CoverageState::Full && proof.is_none() {
            return Err(RetireError::FullNodeNeedsProof);
        }
        if !supersedes(
            new.1,
            &old.iter().map(|(_, m)| m.clone()).collect::<Vec<_>>(),
        ) {
            return Err(RetireError::NotCheaper);
        }
        Ok(Retirement {
            node: node.to_string(),
            retired: old.iter().map(|(d, _)| d.clone()).collect(),
            by: new.0.to_string(),
            reason,
            state,
            proof,
        })
    }
    pub fn retired(&self) -> &[String] {
        &self.retired
    }
    pub fn by(&self) -> &str {
        &self.by
    }
    pub fn proof(&self) -> Option<&ContainmentProof> {
        self.proof.as_ref()
    }
    pub fn node(&self) -> &str {
        &self.node
    }
    pub fn reason(&self) -> &SupersedeReason {
        &self.reason
    }
    pub fn state(&self) -> CoverageState {
        self.state
    }
}

/// `new` supersedes the `old` rules (assuming a domain containing theirs, which the caller proves): strictly fewer
/// cases, or as many with fewer RHS terms, or as many terms with fewer monomials.
pub fn supersedes(new: &RuleMeasure, old: &[RuleMeasure]) -> bool {
    let cases: u64 = old.iter().map(|m| m.cases).sum();
    let terms: u64 = old.iter().map(|m| m.rhs_terms).sum();
    let mons: u64 = old.iter().map(|m| m.monomials).sum();
    (new.cases, new.rhs_terms, new.monomials) < (cases, terms, mons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_measure_uses_monomials_field() {
        let line =
            r#"{"type":"rule","rhs":[["I(n0)","1+n0"],["I(n1)","-2*d+(3-n1)"]],"monomials":100}"#;
        let m = RuleMeasure::from_record_json(line).unwrap();
        assert_eq!(
            (m.rhs_terms, m.monomials, m.monomials_estimated),
            (2, 100, false)
        );
        let line = r#"{"type":"rule","rhs":[["I(n0)","1+n0"],["I(n1)","-2*d+(3-n1)"]]}"#;
        let m = RuleMeasure::from_record_json(line).unwrap();
        assert_eq!((m.monomials, m.monomials_estimated), (4, true));
    }
}
