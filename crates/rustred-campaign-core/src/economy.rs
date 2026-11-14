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

/// What "small" means (R46 (5), spec value): weights of the economy SCORE of a node or a cover,
/// `score = cases * case + rhs_terms * rhs_term + monomials * monomial + closure_spread * spread` (smaller = better).
/// Default (seed-policy.md section 5): one case is worth 2,000 right-hand-side terms (the size of a typical point rule
/// that a symbolic case replaces on T162), a monomial 1/100 of a term, one key of closure spread 10 terms. The ladder's
/// selection keeps R31 (b) first (a ONE-case cover always beats a multi-case cover); the weights decide among covers
/// of the same case count and in node-level comparisons ([`NodeMeasure::weighted_score`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EconomyWeights {
    pub case: f64,
    pub rhs_term: f64,
    pub monomial: f64,
    pub spread: f64,
}

impl Default for EconomyWeights {
    fn default() -> Self {
        EconomyWeights {
            case: 2000.0,
            rhs_term: 1.0,
            monomial: 0.01,
            spread: 10.0,
        }
    }
}

impl NodeMeasure {
    /// The weighted economy score (R46 (5)); closure spread = keys reached outside the input range.
    pub fn weighted_score(&self, w: &EconomyWeights) -> f64 {
        self.cases as f64 * w.case
            + self.rhs_terms as f64 * w.rhs_term
            + self.monomials as f64 * w.monomial
            + self.closure_outside.unwrap_or(0) as f64 * w.spread
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
/// that the partition of the new table set reports no gap where the old one reported none. FIELDS ARE PRIVATE and
/// there is no `Deserialize` (audit A M5): outside this crate it cannot be built; inside, only the containment
/// checker (`crate::contain`, owned by the partition lane, still a placeholder) may call [`Self::checked`]. Its
/// serialised form is [`ContainmentProofRecord`], which is NOT a proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContainmentProof {
    old_tables_digest: String,
    new_tables_digest: String,
    /// "exact_partition"
    method: String,
    artefact_sha256: String,
}

impl ContainmentProof {
    /// For the containment checker only (crate-internal).
    #[allow(dead_code)]
    pub(crate) fn checked(old: &str, new: &str, method: &str, artefact_sha256: &str) -> Self {
        ContainmentProof {
            old_tables_digest: old.into(),
            new_tables_digest: new.into(),
            method: method.into(),
            artefact_sha256: artefact_sha256.into(),
        }
    }
    pub fn old_tables_digest(&self) -> &str {
        &self.old_tables_digest
    }
    pub fn new_tables_digest(&self) -> &str {
        &self.new_tables_digest
    }
    pub fn method(&self) -> &str {
        &self.method
    }
    pub fn artefact_sha256(&self) -> &str {
        &self.artefact_sha256
    }
    pub fn to_record(&self) -> ContainmentProofRecord {
        ContainmentProofRecord {
            old_tables_digest: self.old_tables_digest.clone(),
            new_tables_digest: self.new_tables_digest.clone(),
            method: self.method.clone(),
            artefact_sha256: self.artefact_sha256.clone(),
        }
    }
}

/// UNVERIFIED serialised form of a [`ContainmentProof`] (what a ledger reads back): never accepted as a proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainmentProofRecord {
    pub old_tables_digest: String,
    pub new_tables_digest: String,
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

/// A retirement record `rule_retired {node, rule, by}`. Fields are private and there is no `Deserialize`: a
/// retirement is built only by [`Retirement::new`] (a FULL node's needs a [`ContainmentProof`]) or by
/// [`RetirementRecord::verify`], which re-runs the same checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Retirement {
    node: String,
    retired: Vec<String>,
    by: String,
    reason: SupersedeReason,
    state: CoverageState,
    proof: Option<ContainmentProof>,
    old_measures: Vec<RuleMeasure>,
    new_measure: RuleMeasure,
}

/// UNVERIFIED serialised form of a [`Retirement`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetirementRecord {
    pub node: String,
    pub retired: Vec<String>,
    pub by: String,
    pub reason: SupersedeReason,
    pub state: CoverageState,
    pub proof: Option<ContainmentProofRecord>,
    pub old_measures: Vec<RuleMeasure>,
    pub new_measure: RuleMeasure,
}

impl RetirementRecord {
    /// Re-verify: the same checks as [`Retirement::new`]; a FULL node's record needs the CHECKED proof (`proof`),
    /// which must match the record's.
    pub fn verify(&self, proof: Option<ContainmentProof>) -> Result<Retirement, RetireError> {
        if let (Some(p), Some(r)) = (&proof, &self.proof)
            && p.to_record() != *r
        {
            return Err(RetireError::ProofMismatch);
        }
        if self.old_measures.len() != self.retired.len() {
            return Err(RetireError::Empty);
        }
        let old: Vec<(String, RuleMeasure)> = self
            .retired
            .iter()
            .cloned()
            .zip(self.old_measures.iter().cloned())
            .collect();
        Retirement::new(
            &self.node,
            self.state,
            &old,
            (&self.by, &self.new_measure),
            self.reason.clone(),
            proof,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetireError {
    /// R33: on a FULL node a retirement needs a containment proof.
    FullNodeNeedsProof,
    /// The new rule is not more economical than the retired ones.
    NotCheaper,
    /// Nothing to retire (or a malformed record).
    Empty,
    /// The checked proof is not the one the record names.
    ProofMismatch,
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
            old_measures: old.iter().map(|(_, m)| m.clone()).collect(),
            new_measure: new.1.clone(),
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
    pub fn to_record(&self) -> RetirementRecord {
        RetirementRecord {
            node: self.node.clone(),
            retired: self.retired.clone(),
            by: self.by.clone(),
            reason: self.reason.clone(),
            state: self.state,
            proof: self.proof.as_ref().map(|p| p.to_record()),
            old_measures: self.old_measures.clone(),
            new_measure: self.new_measure.clone(),
        }
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

    fn m(t: u64) -> RuleMeasure {
        RuleMeasure {
            cases: 1,
            rhs_terms: t,
            monomials: t,
            monomials_estimated: false,
            max_degree: None,
            served_keys: None,
            bytes: 1,
        }
    }

    /// R33 / audit A M5: a FULL node's retirement needs a CHECKED proof; the serialised form is not a proof.
    #[test]
    fn full_node_retirement_needs_a_checked_proof_and_records_reverify() {
        let old = vec![("a".to_string(), m(100)), ("b".to_string(), m(100))];
        let e = Retirement::new(
            "N",
            CoverageState::Full,
            &old,
            ("c", &m(50)),
            SupersedeReason::FaceRetiresPoints,
            None,
        );
        assert_eq!(e.unwrap_err(), RetireError::FullNodeNeedsProof);
        let proof = ContainmentProof::checked("o", "n", "exact_partition", "x");
        let r = Retirement::new(
            "N",
            CoverageState::Full,
            &old,
            ("c", &m(50)),
            SupersedeReason::FaceRetiresPoints,
            Some(proof.clone()),
        )
        .unwrap();
        let text = serde_json::to_string(&r).unwrap();
        let rec: RetirementRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(rec.verify(None).unwrap_err(), RetireError::FullNodeNeedsProof);
        assert_eq!(rec.verify(Some(proof)).unwrap(), r);
        let other = ContainmentProof::checked("o", "n2", "exact_partition", "x");
        assert_eq!(rec.verify(Some(other)).unwrap_err(), RetireError::ProofMismatch);
        let forged: RetirementRecord = serde_json::from_str(
            r#"{"node":"N","retired":["a","b"],"by":"c","reason":"CheaperSameOrLargerDomain","state":"Full","proof":null,"old_measures":[],"new_measure":{"cases":1,"rhs_terms":1,"monomials":1,"monomials_estimated":false,"max_degree":null,"served_keys":null,"bytes":1}}"#,
        )
        .unwrap();
        assert!(forged.verify(None).is_err());
    }
}
