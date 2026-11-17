//! Economy of rules (plan 2.4.14 (a), (b); R31, R33, U-13; package WP-I15 measure part): the per-rule and per-node
//! measures with a total order, the selection score of the seed-search rung, the "supersedes" relation and
//! retirement records (a FULL node's retirement cannot be built without a containment proof, R33).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// lane shipecon: the SHIP ECONOMY of a closed node (interfaces/shipecon.md). REQUEST TO I-INT: keep as is (a child
/// module of `economy`, no lib.rs change).
pub mod ship;

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
    /// The rule's size under the node's economy weights ([`EconomyWeights::rule_size`], R46 (5)).
    pub weighted: i64,
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
/// that a symbolic case replaces on a reference node), a monomial 1/100 of a term, one key of closure spread 10
/// terms. WHERE THE LIBRARY READS THEM (v1.3, re-audit A M5 (b)): the ladder's selection keeps R31 (b) first (a
/// ONE-case cover always beats a multi-case cover, and an exact cover has the fewest members); among one-case covers
/// the rule size class and the order are taken on [`Self::rule_size`]; among exact covers of the same member count the
/// smallest summed [`Self::rule_size`] wins; optimise items need a strictly smaller `cases x case + size`
/// ([`Self::cases_cost`]); node-level comparisons use [`NodeMeasure::weighted_score`]. With unknown monomials (the
/// usual case) and the default weights the size is the RHS term count.
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

impl EconomyWeights {
    /// The weighted SIZE of one rule: `rhs_terms x rhs_term + monomials x monomial` (unknown monomials count 0),
    /// rounded, saturating (NaN weights count 0).
    pub fn rule_size(&self, rhs_terms: u64, monomials: Option<u64>) -> i64 {
        let v = rhs_terms as f64 * self.rhs_term + monomials.unwrap_or(0) as f64 * self.monomial;
        if v.is_nan() { 0 } else { v.round() as i64 }
    }
    /// The weighted cost of `cases` cases (`cases x case`), rounded, saturating.
    pub fn cases_cost(&self, cases: u64) -> i64 {
        let v = cases as f64 * self.case;
        if v.is_nan() { 0 } else { v.round() as i64 }
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

/// Proof that the covered domain did not shrink (R33): the exact partition of the NEW table set in DISPATCH mode
/// proves no gap. FIELDS ARE PRIVATE, there is no `Deserialize` and NO constructor that takes a verdict (re-audit A
/// M3, v1.3): the ONLY way to obtain one is [`prove_coverage_preserved`], which runs the partition itself and
/// computes both table-set digests from the domains it was given ([`table_set_digest`]). Its serialised form is
/// [`ContainmentProofRecord`], which is NOT a proof.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContainmentProof {
    old_tables_digest: String,
    new_tables_digest: String,
    /// "exact_partition"
    method: String,
    artefact_sha256: String,
    /// What the ENGINE must be asked before the proof may retire anything (lane partfix4, audit-part4
    /// recommendation 3): the probe keys and the core dispatcher's first match at each ([`EngineProbeRequest`]).
    #[serde(skip)]
    engine_probe: EngineProbeRequest,
    /// The checked engine cross-check ([`ContainmentProof::with_engine_check`]); `None` = not yet asked: such a proof
    /// retires nothing on a FULL node ([`Retirement::new`], assembly).
    engine_check: Option<EngineCrossCheck>,
}

/// THE digest of a table set (integration lane int4: the ONE definition; assembly's former digest over record line
/// digests is gone). It covers exactly what the DISPATCH-mode partition consumes, in dispatch order: the FRAME
/// (arity and line mask: audit-part3 m1, a proof minted on one mask must not check for another) and per record its
/// case domain (canonical text) and its witness block ([`crate::partition::DomainSource::witness_block`], which blocks
/// a selected record without serving). Two sets with one digest therefore have one dispatch coverage by construction;
/// a digest over record bytes alone would not bind a proof to the domains the partition actually ran on (a caller
/// could pair real line digests with fully-covering fake domains), and a digest over domains without the witness
/// blocks would let a proof run on unblocked domains stand for blocked ones. Record identities (which rule is
/// retired, which supersedes it) are checked by the caller against its own records. Version-4 text, not a fingerprint
/// of the files (version 1 had no witness blocks, version 2 no frame, version 3 was computed on domains whose reader
/// dropped `domain_bounds`: audit-part4 B1b; since lane partfix4 a record's `domain_bounds` are part of its domain as
/// the engine's exception branches, so two files that differ in them differ here, and no v3 digest equals a v4 one).
pub fn table_set_digest(
    src: &dyn crate::partition::DomainSource,
    frame: &crate::key::NodeFrame,
) -> String {
    let mut h = crate::replay_base::sha256::Sha256::new();
    h.update(
        format!(
            "containment-tables.v4;arity={};frame_arity={};mask={};n={}\n",
            src.arity(),
            frame.arity(),
            frame.mask_string(),
            src.len()
        )
        .as_bytes(),
    );
    for i in 0..src.len() {
        h.update(
            format!(
                "{:?};witness_block={:?}\n",
                src.domain(i),
                src.witness_block(i)
            )
            .as_bytes(),
        );
    }
    format!("sha256:{}", h.hex())
}

/// The ONLY caller knob of [`prove_coverage_preserved`] (audit-part3 C1): a work budget. The minting function builds
/// its own partition options (dispatch coverage, exact literal tests, no rule subset, no off-floor cap, every class
/// expanded); one [`crate::partition::WorkMeter`] bounds the partition (recursion, class listing, expansion cells,
/// locus resolution, Fourier-Motzkin and point-search work) AND the final cross-check (one unit per probe key).
/// Exhaustion is a refusal, never a proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofWork {
    pub max_units: u64,
    pub wall: Option<std::time::Duration>,
    /// Memo entries kept at most (memory bound only; exactness unaffected).
    pub max_memo: Option<usize>,
}

impl Default for ProofWork {
    fn default() -> Self {
        ProofWork {
            max_units: 20_000_000,
            wall: None,
            max_memo: Some(100_000),
        }
    }
}

/// The checked PRECONDITIONS of a coverage proof (audit-part3 A: every modelling assumption of the partition about
/// first-match dispatch is either modelled or excluded here; the list is interfaces/partition.md section 14). Built
/// only by [`ProofPreconditions::check`]; [`prove_coverage_preserved`] runs it on the new table set and refuses on
/// any failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProofPreconditions {
    pub records: usize,
    pub arity: usize,
    pub frame_mask: String,
    /// Records with a target pattern (their rule-point overflow is modelled as a stop piece).
    pub pattern_targets: usize,
    /// Records whose literals may meet the engine's modular zero test (modelled as an over-approximated stop piece).
    pub modular_zero_risk: usize,
    /// Records blocked by their witness status wherever selected (modelled).
    pub witness_blocked: usize,
}

impl ProofPreconditions {
    /// Check the preconditions on a table set. Refused (`Err(reason)`), each because the partition's model of first
    /// match would not be the dispatcher's / the engine's:
    /// - arity: the source, every domain and the frame agree (and the partition supports it: <= 64 slots);
    /// - every domain validates (`CaseDomain::validate`: the dispatcher refuses invalid domains);
    /// - chart: every domain's chart and the source's dispatch chart are absent or equal to the frame mask (the
    ///   region of the proof is the frame's chart orthant; a different dispatch chart sends keys of it to
    ///   `OutsideChart`);
    /// - credit basis (strict dispatch): no record's witness check is disabled (`Assurance::CheckDisabled` hides its
    ///   witness block) and no witness / target rests on an UNCHECKED attestation;
    /// - witness blocks are `Elided` or `WitnessNotCarried` (what the dispatcher produces);
    /// - every BOUND (selection `GE` literal) is ENGINE-READABLE ([`engine_readable_bound`]): a one-slot linear lower
    ///   bound on a line slot or upper bound on a numerator slot excluding at most
    ///   [`ENGINE_MAX_EXCLUDED_PER_SLOT`] chart values, which the engine reads as its `domain_bounds` (extra exception
    ///   branches: NotSelected, dispatch falls through, as here); any other bound has no engine counterpart.
    pub fn check(
        src: &dyn crate::partition::DomainSource,
        frame: &crate::key::NodeFrame,
    ) -> Result<ProofPreconditions, String> {
        use crate::cover::Assurance;
        use crate::domain::{TargetMap, Verdict};
        let n = frame.arity();
        if src.arity() != n {
            return Err(format!(
                "precondition arity: source {} vs frame {n}",
                src.arity()
            ));
        }
        if n > 64 {
            return Err(format!("precondition arity: {n} > 64 slots"));
        }
        let mask: Vec<bool> = (0..n).map(|i| frame.is_line(i)).collect();
        if let Some(c) = src.dispatch_chart()
            && c != mask
        {
            return Err(
                "precondition chart: the source's dispatch chart differs from the frame mask"
                    .into(),
            );
        }
        let mut pre = ProofPreconditions {
            records: src.len(),
            arity: n,
            frame_mask: frame.mask_string(),
            pattern_targets: 0,
            modular_zero_risk: 0,
            witness_blocked: 0,
        };
        for i in 0..src.len() {
            let d = src.domain(i);
            if d.arity != n {
                return Err(format!(
                    "precondition arity: record {i} has arity {}",
                    d.arity
                ));
            }
            d.validate()
                .map_err(|e| format!("precondition validate: record {i}: {e}"))?;
            if let Some(c) = &d.chart
                && *c != mask
            {
                return Err(format!(
                    "precondition chart: record {i}'s chart differs from the frame mask"
                ));
            }
            let b = src.rule_ref(i).basis;
            if b.witness == Assurance::CheckDisabled {
                return Err(format!(
                    "precondition strict dispatch: record {i}'s witness check is disabled"
                ));
            }
            if b.rests_on_unchecked_attestation() {
                return Err(format!(
                    "precondition strict dispatch: record {i} rests on an unchecked attestation"
                ));
            }
            match src.witness_block(i) {
                None => {}
                Some(Verdict::Elided) | Some(Verdict::WitnessNotCarried) => {
                    pre.witness_blocked = pre.witness_blocked.saturating_add(1);
                }
                Some(v) => {
                    return Err(format!(
                        "precondition witness block: record {i} has the block {v:?} (not a witness verdict)"
                    ));
                }
            }
            // audit-part4 B2 / B2b (lane partfix4): the engine has no inequality bounds; a selection GE literal is read
            // identically only as a `domain_bounds` entry under a CANONICAL target (rule point = key: on the chart the
            // engine's finitely many excluded values are exactly the literal's negative set); with a target pattern the
            // rule point can leave the chart side (the engine then excludes only those values) and on a fixed slot the
            // engine compares the key's fixed value: not modelled, refused
            if !d.bounds.is_empty() && d.target != TargetMap::Canonical {
                return Err(format!(
                    "precondition engine-readable: record {i}: a selection bound with a target pattern (the engine reads bounds as finitely many excluded rule-point values; audit-part4 B2)"
                ));
            }
            for g in &d.bounds {
                engine_readable_bound(g, frame).map_err(|w| {
                    format!("precondition engine-readable: record {i}: bound {g}: {w}")
                })?;
            }
            if matches!(d.target, TargetMap::Pattern(_)) {
                pre.pattern_targets = pre.pattern_targets.saturating_add(1);
            }
            if crate::partition::nf::modular_zero_risk(d).is_some() {
                pre.modular_zero_risk = pre.modular_zero_risk.saturating_add(1);
            }
        }
        Ok(pre)
    }
}

/// The engine's limit of chart values one `domain_bounds` entry may exclude (rustred-core `node::bounds`
/// `MAX_EXCLUDED_PER_SLOT`; a further bound is refused by the engine at load, so the record would never be served by it).
pub const ENGINE_MAX_EXCLUDED_PER_SLOT: i128 = 64;

/// Is a selection bound `g >= 0` (evaluated at the rule point) read IDENTICALLY by the engine? The engine knows
/// selection bounds only as `domain_bounds` (rustred-core `node::bounds`): inclusive `min` on a line slot / `max` on
/// a numerator slot of the rule point, turned into exception branches excluding the finitely many chart values beyond
/// (at most [`ENGINE_MAX_EXCLUDED_PER_SLOT`]). So `g` must be `a n_i + b` with `a > 0` on a line slot (n_i >=
/// ceil(-b / a)) or `a < 0` on a numerator slot (n_i <= floor(b / -a)); a constant `g >= 0` is trivially read.
pub fn engine_readable_bound(
    g: &crate::poly::Poly,
    frame: &crate::key::NodeFrame,
) -> Result<(), String> {
    let mut slot: Option<(usize, i128)> = None;
    let mut b: i128 = 0;
    let terms = g
        .small_terms()
        .ok_or("a coefficient beyond i128 (the engine's domain_bounds are i64)")?;
    for (m, c) in terms {
        match m.as_slice() {
            [] => b = *c,
            [(v, 1)] if *v != crate::poly::D_VAR && slot.is_none() => {
                slot = Some((usize::from(*v), *c));
            }
            _ => return Err("not a one-slot linear bound".into()),
        }
    }
    let Some((i, a)) = slot else {
        return if b >= 0 {
            Ok(())
        } else {
            Err("a constant bound that never holds".into())
        };
    };
    if i >= frame.arity() {
        return Err(format!("slot {i} beyond the arity"));
    }
    // the chart values the bound excludes: lines n >= 1 with a n + b < 0; numerators n <= 0 with a n + b < 0
    let excluded: i128 = if frame.is_line(i) {
        if a <= 0 {
            return Err(
                "an upper bound on a line slot (the engine refuses it: infinite exclusion)".into(),
            );
        }
        // smallest admitted n = ceil(-b / a); excluded = n in [1, ceil(-b/a) - 1]
        let lo = crate::partition::exact::ceil_neg_div(b, a).map_err(|e| format!("{e}"))?;
        lo.saturating_sub(1).max(0)
    } else {
        if a >= 0 {
            return Err(
                "a lower bound on a numerator slot (the engine refuses it: infinite exclusion)"
                    .into(),
            );
        }
        // largest admitted n = floor(-b / a) (a < 0); excluded = n in [floor(-b/a) + 1, 0]
        let hi = crate::partition::exact::floor_neg_div(b, a).map_err(|e| format!("{e}"))?;
        hi.saturating_neg().max(0)
    };
    if excluded > ENGINE_MAX_EXCLUDED_PER_SLOT {
        return Err(format!(
            "excludes {excluded} chart values (the engine's domain_bounds limit is {ENGINE_MAX_EXCLUDED_PER_SLOT})"
        ));
    }
    Ok(())
}

/// An owned SNAPSHOT of a domain source (audit-part3 m2): the partition, the digest and the cross-check all read the
/// same in-memory copy, so a source whose answers change between calls cannot pair a digest with domains the
/// partition never saw.
struct Snapshot {
    arity: usize,
    items: Vec<(
        crate::cover::RuleRef,
        crate::domain::CaseDomain,
        Option<crate::domain::Verdict>,
    )>,
    chart: Option<Vec<bool>>,
}

impl Snapshot {
    fn of(src: &dyn crate::partition::DomainSource) -> Snapshot {
        Snapshot {
            arity: src.arity(),
            items: (0..src.len())
                .map(|i| (src.rule_ref(i), src.domain(i).clone(), src.witness_block(i)))
                .collect(),
            chart: src.dispatch_chart(),
        }
    }

    /// The core dispatcher of the snapshot (one table, dispatch order, the witness blocks as record status, chart =
    /// the frame mask): THE reference of the cross-check.
    fn dispatcher(&self, mask: &[bool]) -> Result<crate::cover::Dispatcher, String> {
        use crate::cover::{Assurance, Dispatcher, RecordMeta, Table, WitnessStatus};
        use crate::domain::Verdict;
        let mut t = Table::new("proof-snapshot", self.arity);
        for (i, (_, d, wb)) in self.items.iter().enumerate() {
            let witness = match wb {
                None => WitnessStatus::Complete,
                Some(Verdict::Elided) => WitnessStatus::Elided,
                Some(Verdict::WitnessNotCarried) => WitnessStatus::NotCarried,
                Some(v) => return Err(format!("record {i}: witness block {v:?}")),
            };
            let meta = RecordMeta {
                source_index: i,
                line_no: 0,
                witness,
                witness_basis: Assurance::Record,
                target_basis: Assurance::Record,
            };
            let label = i64::try_from(i).map_err(|e| format!("{e}"))?;
            t.push_with_meta(label, d.clone(), meta)
                .map_err(|e| format!("record {i}: {e}"))?;
        }
        Dispatcher::new(vec![t], vec![0])
            .and_then(|d| d.with_chart(mask.to_vec()))
            .map_err(|e| format!("{e}"))
    }
}

impl crate::partition::DomainSource for Snapshot {
    fn arity(&self) -> usize {
        self.arity
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn domain(&self, i: usize) -> &crate::domain::CaseDomain {
        &self.items[i].1
    }
    fn rule_ref(&self, i: usize) -> crate::cover::RuleRef {
        self.items[i].0
    }
    fn witness_block(&self, i: usize) -> Option<crate::domain::Verdict> {
        self.items[i].2.clone()
    }
    fn dispatch_chart(&self) -> Option<Vec<bool>> {
        self.chart.clone()
    }
}

/// THE CHECKING FUNCTION of R33 (re-audit A M3; audit-part3 A): the only constructor of a [`ContainmentProof`].
/// What a proof rests on:
/// 1. a SNAPSHOT of both table sets, taken once (m2);
/// 2. [`ProofPreconditions::check`] on the new set: every assumption the partition makes about first-match dispatch
///    that it does not model is excluded there (list: interfaces/partition.md 14);
/// 3. the exact partition of the new set in DISPATCH mode with options built HERE (no caller option except the work
///    budget [`ProofWork`]: no rule subset, no off-floor cap, exact literal tests; C1 / C3), whose report must carry
///    the `CompleteRun` token and `proves_no_gap(DispatchCoverage)`; the normal form models rule-point overflow and
///    the engine's modular zero test as stop pieces (M1 / C2);
/// 4. a CROSS-CHECK against the REAL core dispatcher (`cover::Dispatcher::first_match`, rebuilt from the snapshot
///    with the frame as chart): every probe key the partition's own structure defines
///    ([`crate::partition::PartitionReport::dispatch_probes`]: state boundaries per slot, diagonals, coupled slot
///    pairs, points on and next to every linear literal's zero set, fragile cells, far probes) must be `Covered`;
///    any other verdict refuses the proof and names the key. Cost: one work unit per probe (at most a few times the
///    sum of the state counts, the coupled pairs, the literals and the fragile cells, plus 256 far probes), bounded
///    by the same budget; an exhausted budget refuses;
/// 5. both digests over the snapshots and the frame ([`table_set_digest`] v4, m1), the dispatch order statement (an
///    empty new-order digest is refused) and the certificate (incl. the preconditions and the probe count);
/// 6. (lane partfix4, audit-part4 recommendation 3) the keys the ENGINE must answer identically
///    ([`ContainmentProof::engine_probe`]): such a proof retires nothing on a FULL node until
///    [`ContainmentProof::with_engine_check`] has bound the frozen engine's first match at every one of them.
pub fn prove_coverage_preserved(
    old_tables: &dyn crate::partition::DomainSource,
    new_tables: &dyn crate::partition::DomainSource,
    frame: &crate::key::NodeFrame,
    work: &ProofWork,
    order: &crate::partition::DispatchOrderStatement,
) -> Result<ContainmentProof, String> {
    prove_coverage_preserved_with_box(old_tables, new_tables, frame, work, order, &[])
}

/// [`prove_coverage_preserved`] with the node's BOX keys (lane partfix4, audit-part4 recommendation 3): every box key
/// must also be `Covered` by the core dispatcher (one work unit each), and the box keys join the partition's probe keys
/// in the proof's [`EngineProbeRequest`] (the keys the frozen engine must answer identically before the proof may
/// retire anything: [`ContainmentProof::with_engine_check`]). The box is the caller's data (the spec's predicate,
/// enumerated); keys outside the frame's chart orthant are refused.
pub fn prove_coverage_preserved_with_box(
    old_tables: &dyn crate::partition::DomainSource,
    new_tables: &dyn crate::partition::DomainSource,
    frame: &crate::key::NodeFrame,
    work: &ProofWork,
    order: &crate::partition::DispatchOrderStatement,
    box_keys: &[Vec<i32>],
) -> Result<ContainmentProof, String> {
    use crate::partition::{
        CoverageNotion, DispatchOrderStatement, WorkMeter, dispatch_proof_method,
    };
    if let DispatchOrderStatement::NewOrder(d) = order
        && d.trim().is_empty()
    {
        return Err("a new dispatch order needs its digest".into());
    }
    let old = Snapshot::of(old_tables);
    let new = Snapshot::of(new_tables);
    if old.arity != frame.arity() {
        return Err(format!(
            "the old table set has arity {}, the frame {}",
            old.arity,
            frame.arity()
        ));
    }
    let pre = ProofPreconditions::check(&new, frame)?;
    let t0 = std::time::Instant::now();
    let meter = std::rc::Rc::new(WorkMeter::new(
        Some(work.max_units),
        work.wall.and_then(|w| t0.checked_add(w)),
    ));
    let (rep, certificate) =
        crate::partition::prove_dispatch_no_gap(&new, frame, &meter, work.max_memo)?;
    debug_assert_eq!(rep.coverage(), CoverageNotion::DispatchCoverage);
    // the cross-check against the real dispatcher
    let mask: Vec<bool> = (0..frame.arity()).map(|i| frame.is_line(i)).collect();
    let disp = new.dispatcher(&mask)?;
    let probes = rep.dispatch_probes(&meter);
    if meter.exhausted() {
        return Err(format!(
            "cross-check: the work budget was exhausted while building the probe keys ({})",
            meter.reason().unwrap_or_default()
        ));
    }
    let n_probes = probes.len();
    let mut keys = probes;
    for k in box_keys {
        if k.len() != frame.arity()
            || k.iter()
                .enumerate()
                .any(|(i, &x)| if frame.is_line(i) { x < 1 } else { x > 0 })
        {
            return Err(format!(
                "box key {k:?} is not a key of the frame's chart orthant"
            ));
        }
        keys.push(k.clone());
    }
    keys.sort();
    keys.dedup();
    let expected = cross_check(&disp, &keys, &meter)?;
    let new_tables_digest = table_set_digest(&new, frame);
    let engine_probe = EngineProbeRequest::new(keys, expected, &new_tables_digest);
    let full_certificate = format!(
        "{certificate}; preconditions {pre:?}; cross-check: {n_probes} probe keys + {} box keys ({} distinct) served by Dispatcher::first_match; engine probe keys sha256 {}; work {} units",
        box_keys.len(),
        engine_probe.keys.len(),
        engine_probe.keys_sha256,
        meter.used()
    );
    Ok(ContainmentProof {
        old_tables_digest: table_set_digest(&old, frame),
        new_tables_digest,
        method: dispatch_proof_method(order),
        artefact_sha256: crate::replay_base::sha256::sha256_hex(full_certificate.as_bytes()),
        engine_probe,
        engine_check: None,
    })
}

/// THE core cross-check (audit-part3 A, factored out by lane partfix4 so that it is tested on its own): every key must
/// be `Covered` by the real dispatcher (one work unit per key; exhaustion refuses); returns the serving record's
/// dispatch position per key (the reference the engine is compared with).
pub(crate) fn cross_check(
    disp: &crate::cover::Dispatcher,
    probes: &[Vec<i32>],
    meter: &crate::partition::WorkMeter,
) -> Result<Vec<usize>, String> {
    let mut served = Vec::with_capacity(probes.len());
    for (done, k) in probes.iter().enumerate() {
        if !meter.tick(1) {
            return Err(format!(
                "cross-check: the work budget was exhausted after {done} of {} probe keys ({})",
                probes.len(),
                meter.reason().unwrap_or_default()
            ));
        }
        match disp.first_match(k) {
            Ok(crate::cover::Dispatch::Covered(r)) => served.push(r.record),
            other => {
                return Err(format!(
                    "cross-check: key {k:?} is {other:?} for Dispatcher::first_match although the partition proves no gap: no proof (the partition's model of dispatch differs from the dispatcher here)"
                ));
            }
        }
    }
    Ok(served)
}

/// The keys the frozen ENGINE must be asked for a proof (lane partfix4, audit-part4 recommendation 3; minted inside
/// the proof, never by a caller): the partition's probe keys and the node's box keys (sorted, distinct) and the core
/// dispatcher's first match at each, as the serving record's POSITION in the new table set (dispatch order, 0-based).
/// The engine reads the new set as written ([`engine_probe_table`]: the records' raw lines in dispatch order, so a
/// field the core misreads is read by the engine as it is), and `rr_node first-match --table T --keys K` must give
/// the same position at every key ([`ContainmentProof::with_engine_check`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct EngineProbeRequest {
    pub keys: Vec<Vec<i32>>,
    /// Per key: the core dispatcher's serving record (position in the new set).
    pub expected: Vec<usize>,
    /// sha256 hex of [`EngineProbeRequest::keys_file`].
    pub keys_sha256: String,
    /// The proof's new table-set digest.
    pub new_tables_digest: String,
}

impl EngineProbeRequest {
    fn new(keys: Vec<Vec<i32>>, expected: Vec<usize>, new_tables_digest: &str) -> Self {
        let mut r = EngineProbeRequest {
            keys,
            expected,
            keys_sha256: String::new(),
            new_tables_digest: new_tables_digest.to_string(),
        };
        r.keys_sha256 = crate::replay_base::sha256::sha256_hex(&r.keys_file());
        r
    }
    /// The `--keys` file of `rr_node first-match`: one JSON array per line.
    pub fn keys_file(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for k in &self.keys {
            out.push(b'[');
            for (i, x) in k.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(x.to_string().as_bytes());
            }
            out.extend_from_slice(b"]\n");
        }
        out
    }
}

/// The table file the engine reads for the cross-check: the header line, the chart lines and the NEW set's DISPATCHED
/// rule lines VERBATIM in dispatch order (no other record: `rr_node first-match` answers the position among the
/// file's rule records, which is then the core's position), each terminated by one newline.
pub fn engine_probe_table(header: &[u8], charts: &[Vec<u8>], records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for l in std::iter::once(header)
        .chain(charts.iter().map(Vec::as_slice))
        .chain(records.iter().map(Vec::as_slice))
    {
        out.extend_from_slice(l.strip_suffix(b"\n").unwrap_or(l));
        out.push(b'\n');
    }
    out
}

/// What the engine answered (the child job's result; UNVERIFIED until [`ContainmentProof::with_engine_check`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineFirstMatchRun {
    /// sha256 hex of the engine binary that answered (a frozen rr_node).
    pub engine_sha256: String,
    /// sha256 hex of the table file it read ([`engine_probe_table`]).
    pub table_sha256: String,
    /// sha256 hex of the keys file it read ([`EngineProbeRequest::keys_file`]).
    pub keys_sha256: String,
    /// Per key, in the keys file's order: `position` of `rr_node first-match` (None = no rule applies).
    pub positions: Vec<Option<usize>>,
}

impl EngineFirstMatchRun {
    /// Read `rr_node first-match` output (one JSON line `{key, position, label}` per key, in the keys file's order);
    /// every line's key must be the request's key at that place.
    pub fn from_rr_node_output(
        text: &str,
        request: &EngineProbeRequest,
        engine_sha256: &str,
        table_sha256: &str,
    ) -> Result<EngineFirstMatchRun, String> {
        let mut positions = Vec::with_capacity(request.keys.len());
        for (i, l) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let v: serde_json::Value =
                serde_json::from_str(l).map_err(|e| format!("engine line {}: {e}", i + 1))?;
            let key: Vec<i64> = serde_json::from_value(v["key"].clone())
                .map_err(|e| format!("engine line {}: key: {e}", i + 1))?;
            let want = request
                .keys
                .get(i)
                .ok_or_else(|| format!("engine line {}: more answers than keys", i + 1))?;
            if key.len() != want.len() || key.iter().zip(want).any(|(a, &b)| *a != i64::from(b)) {
                return Err(format!(
                    "engine line {}: key {key:?}, expected {want:?}",
                    i + 1
                ));
            }
            positions.push(match &v["position"] {
                serde_json::Value::Null => None,
                p => Some(
                    p.as_u64()
                        .and_then(|x| usize::try_from(x).ok())
                        .ok_or_else(|| format!("engine line {}: position {p}", i + 1))?,
                ),
            });
        }
        Ok(EngineFirstMatchRun {
            engine_sha256: engine_sha256.to_string(),
            table_sha256: table_sha256.to_string(),
            keys_sha256: request.keys_sha256.clone(),
            positions,
        })
    }
}

/// A CHECKED engine cross-check, bound into the proof (and its record): which engine read which table at which keys,
/// and that it served every key by the record the core dispatcher names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCrossCheck {
    pub engine_sha256: String,
    pub table_sha256: String,
    pub keys_sha256: String,
    pub keys: usize,
    pub new_tables_digest: String,
}

impl ContainmentProof {
    /// Unit tests of this module only (never in a build of the library).
    #[cfg(test)]
    pub(crate) fn for_tests(old: &str, new: &str, method: &str, artefact_sha256: &str) -> Self {
        ContainmentProof {
            old_tables_digest: old.into(),
            new_tables_digest: new.into(),
            method: method.into(),
            artefact_sha256: artefact_sha256.into(),
            engine_probe: EngineProbeRequest::default(),
            engine_check: Some(EngineCrossCheck {
                engine_sha256: "test".into(),
                table_sha256: "test".into(),
                keys_sha256: "test".into(),
                keys: 1,
                new_tables_digest: new.into(),
            }),
        }
    }
    /// The keys the engine must answer ([`EngineProbeRequest`]).
    pub fn engine_probe(&self) -> &EngineProbeRequest {
        &self.engine_probe
    }
    /// The checked engine cross-check, if done.
    pub fn engine_check(&self) -> Option<&EngineCrossCheck> {
        self.engine_check.as_ref()
    }
    /// Bind the ENGINE's answer (lane partfix4, audit-part4 recommendation 3): `run` must be for this proof's keys
    /// file, for the table file whose sha256 the CALLER computed from the bytes it gave the engine
    /// (`table_sha256`, [`engine_probe_table`]), name the engine binary, and give at EVERY key the core dispatcher's
    /// serving position. Any difference refuses and names the first key with both answers. The returned proof carries
    /// the check (and a new artefact digest that includes it); only such a proof retires anything on a FULL node.
    pub fn with_engine_check(
        mut self,
        run: &EngineFirstMatchRun,
        table_sha256: &str,
    ) -> Result<ContainmentProof, String> {
        let req = &self.engine_probe;
        if req.keys.is_empty() || req.keys_sha256.is_empty() {
            return Err("engine cross-check: the proof carries no probe keys".into());
        }
        if self.engine_check.is_some() {
            return Err("engine cross-check: already bound".into());
        }
        if run.engine_sha256.trim().is_empty() || table_sha256.trim().is_empty() {
            return Err(
                "engine cross-check: the engine binary and the table file must be named".into(),
            );
        }
        if run.keys_sha256 != req.keys_sha256 {
            return Err(format!(
                "engine cross-check: the engine answered keys file {}, the proof asks {}",
                run.keys_sha256, req.keys_sha256
            ));
        }
        if run.table_sha256 != table_sha256 {
            return Err(format!(
                "engine cross-check: the engine read table {}, the caller wrote {table_sha256}",
                run.table_sha256
            ));
        }
        if run.positions.len() != req.keys.len() {
            return Err(format!(
                "engine cross-check: {} answers for {} keys",
                run.positions.len(),
                req.keys.len()
            ));
        }
        for ((k, want), got) in req.keys.iter().zip(&req.expected).zip(&run.positions) {
            if *got != Some(*want) {
                return Err(format!(
                    "engine cross-check: at key {k:?} the engine's first match is {got:?}, the core dispatcher's is record {want}: no retirement (the engine reads this table set differently)"
                ));
            }
        }
        let check = EngineCrossCheck {
            engine_sha256: run.engine_sha256.clone(),
            table_sha256: table_sha256.to_string(),
            keys_sha256: req.keys_sha256.clone(),
            keys: req.keys.len(),
            new_tables_digest: req.new_tables_digest.clone(),
        };
        self.artefact_sha256 = crate::replay_base::sha256::sha256_hex(
            format!("{};engine-check={check:?}", self.artefact_sha256).as_bytes(),
        );
        self.engine_check = Some(check);
        Ok(self)
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
            engine_check: self.engine_check.clone(),
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
    /// The engine cross-check bound into the proof (absent in records written before lane partfix4).
    #[serde(default)]
    pub engine_check: Option<EngineCrossCheck>,
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
    /// A FULL node's proof carries no checked ENGINE cross-check (lane partfix4, audit-part4 recommendation 3:
    /// [`ContainmentProof::with_engine_check`]), or one for another table set.
    EngineCheckMissing,
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
        // R33 / audit-part-A M3 (c): on a FULL node the coverage-preservation proof must come from a DISPATCH-mode
        // partition with a stated dispatch order (a union-coverage partition does not see records that are
        // selected but blocked); anything else, including an unknown method, is refused.
        if state == CoverageState::Full
            && !proof
                .as_ref()
                .is_some_and(|p| crate::partition::parse_dispatch_proof_method(&p.method).is_some())
        {
            return Err(RetireError::FullNodeNeedsProof);
        }
        if state == CoverageState::Full
            && !proof.as_ref().is_some_and(|p| {
                p.engine_check
                    .as_ref()
                    .is_some_and(|c| c.new_tables_digest == p.new_tables_digest && c.keys > 0)
            })
        {
            return Err(RetireError::EngineCheckMissing);
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
    /// The (old, new) table-set digests a FULL node's retirement is bound to (those of its proof).
    pub fn tables(&self) -> Option<(&str, &str)> {
        self.proof
            .as_ref()
            .map(|p| (p.old_tables_digest.as_str(), p.new_tables_digest.as_str()))
    }
    /// Check that the retirement is applied to the table sets its proof checked (re-audit A M3: "any checked proof
    /// fits any retirement" no more): on a FULL node both digests must equal [`table_set_digest`] of the given sets
    /// IN THE GIVEN FRAME (audit-part3 m1: the frame is part of the digest).
    pub fn check_tables(
        &self,
        old_tables: &dyn crate::partition::DomainSource,
        new_tables: &dyn crate::partition::DomainSource,
        frame: &crate::key::NodeFrame,
    ) -> Result<(), RetireError> {
        match self.tables() {
            None if self.state == CoverageState::Full => Err(RetireError::FullNodeNeedsProof),
            None => Ok(()),
            Some((o, n))
                if o == table_set_digest(old_tables, frame)
                    && n == table_set_digest(new_tables, frame) =>
            {
                Ok(())
            }
            Some(_) => Err(RetireError::ProofMismatch),
        }
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

    /// lane partfix4 (audit-part4 mutants 106 / 107: the core cross-check disabled or weakened): `cross_check`
    /// refuses a key the dispatcher leaves Uncovered and a key it Blocks, returns the serving positions otherwise,
    /// and charges the budget.
    #[test]
    fn pf4_cross_check_refuses_uncovered_and_blocked_keys() {
        use crate::cover::{Dispatcher, Table};
        use crate::domain::CaseDomain;
        use crate::partition::WorkMeter;
        let mut p = CaseDomain::free(1, Some(vec![true]));
        p.fixed = vec![Some(1)];
        let mut blocked = CaseDomain::free(1, Some(vec![true]));
        blocked.fixed = vec![Some(2)];
        blocked.witness_elided = true;
        let mut t = Table::new("t", 1);
        t.push(0, p).unwrap();
        t.push(1, blocked).unwrap();
        let d = Dispatcher::new(vec![t], vec![0])
            .unwrap()
            .with_chart(vec![true])
            .unwrap();
        let m = WorkMeter::unlimited();
        assert_eq!(cross_check(&d, &[vec![1]], &m).unwrap(), vec![0]);
        let e = cross_check(&d, &[vec![1], vec![3]], &m).unwrap_err();
        assert!(e.contains("[3]") && e.contains("Uncovered"), "{e}");
        let e = cross_check(&d, &[vec![2]], &m).unwrap_err();
        assert!(e.contains("[2]") && e.contains("Blocked"), "{e}");
        let small = WorkMeter::new(Some(1), None);
        assert!(
            cross_check(&d, &[vec![1], vec![1]], &small).is_err(),
            "one unit per key"
        );
    }

    /// lane partfix4 (audit-part4 mutant 118: the snapshot's dispatcher losing the witness blocks): the reference
    /// dispatcher of a snapshot BLOCKS a record whose witness block is Elided or NotCarried.
    #[test]
    fn pf4_snapshot_dispatcher_keeps_witness_blocks() {
        use crate::cover::{Dispatch, Dispatcher, Table};
        use crate::domain::CaseDomain;
        use crate::record::{RecordFormat, RuleRecord};
        // a record WITHOUT witness_loci: the loader marks it NotCarried (blocked wherever selected)
        let rec = RuleRecord {
            format: RecordFormat::TableDump,
            index: Some(0),
            chart: Some("1".into()),
            fixed: vec![None],
            target: Some("I(n0)".into()),
            ..Default::default()
        };
        let frame = crate::key::NodeFrame::without_header_lines_first(1, 1);
        let mut t = Table::new("t", 1);
        let d0 =
            CaseDomain::from_rule_record(&rec, &frame, crate::domain::DomainOptions::default())
                .unwrap();
        t.push_with_meta(
            0,
            d0,
            crate::cover::RecordMeta {
                source_index: 0,
                line_no: 0,
                witness: crate::cover::WitnessStatus::NotCarried,
                witness_basis: crate::cover::Assurance::Record,
                target_basis: crate::cover::Assurance::Record,
            },
        )
        .unwrap();
        let disp = Dispatcher::new(vec![t], vec![0])
            .unwrap()
            .with_chart(vec![true])
            .unwrap();
        let src = crate::partition::DispatcherSource::new(&disp);
        let snap = Snapshot::of(&src);
        assert!(
            snap.items[0].2.is_some(),
            "the source reports the witness block"
        );
        let sd = snap.dispatcher(&[true]).unwrap();
        assert!(matches!(
            sd.first_match(&[3]).unwrap(),
            Dispatch::Blocked { .. }
        ));
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
        // audit-part-A M3 (partfix): the proof method must be a dispatch-partition method with an order statement
        // (was "exact_partition", a union-coverage method, now refused: see m3_economy_requires_a_dispatch_proof).
        let same = crate::partition::dispatch_proof_method(
            &crate::partition::DispatchOrderStatement::SameOrder,
        );
        let proof = ContainmentProof::for_tests("o", "n", &same, "x");
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
        assert_eq!(
            rec.verify(None).unwrap_err(),
            RetireError::FullNodeNeedsProof
        );
        assert_eq!(rec.verify(Some(proof)).unwrap(), r);
        let other = ContainmentProof::for_tests("o", "n2", &same, "x");
        assert_eq!(
            rec.verify(Some(other)).unwrap_err(),
            RetireError::ProofMismatch
        );
        let forged: RetirementRecord = serde_json::from_str(
            r#"{"node":"N","retired":["a","b"],"by":"c","reason":"CheaperSameOrLargerDomain","state":"Full","proof":null,"old_measures":[],"new_measure":{"cases":1,"rhs_terms":1,"monomials":1,"monomials_estimated":false,"max_degree":null,"served_keys":null,"bytes":1}}"#,
        )
        .unwrap();
        assert!(forged.verify(None).is_err());
    }

    /// audit-part-A M3 (c) (lane partfix; moved here from tests/partition_audit_regressions.rs by the int3
    /// integration because ContainmentProof's fields are private since ladder audit A M5): on a FULL node only a
    /// dispatch-partition method with an order statement is accepted.
    #[test]
    fn m3_economy_requires_a_dispatch_proof() {
        use crate::partition::{DispatchOrderStatement, dispatch_proof_method};
        let old = vec![("a".to_string(), m(100))];
        let proof = |method: &str| ContainmentProof::for_tests("o", "n", method, "x");
        let retire = |p: ContainmentProof| {
            Retirement::new(
                "N",
                CoverageState::Full,
                &old,
                ("c", &m(50)),
                SupersedeReason::FaceRetiresPoints,
                Some(p),
            )
        };
        // a union-coverage proof is refused on a FULL node
        assert_eq!(
            retire(proof("exact_partition")).unwrap_err(),
            RetireError::FullNodeNeedsProof
        );
        assert_eq!(
            retire(proof("dispatch_partition.v1")).unwrap_err(),
            RetireError::FullNodeNeedsProof,
            "no order statement"
        );
        assert!(
            retire(proof(&dispatch_proof_method(
                &DispatchOrderStatement::SameOrder
            )))
            .is_ok()
        );
        let m2 = dispatch_proof_method(&DispatchOrderStatement::NewOrder("abc123".into()));
        assert!(retire(proof(&m2)).is_ok());
        // the record route re-runs the same check
        let r = retire(proof(&m2)).unwrap();
        let mut rec = r.to_record();
        assert_eq!(rec.verify(Some(proof(&m2))).unwrap(), r);
        rec.proof.as_mut().unwrap().method = "exact_partition".into();
        assert!(rec.verify(Some(proof("exact_partition"))).is_err());
    }
}

/// Lane sizegen (interfaces/genchoice.md): the generation-choice portfolio of a whole node. Declared here until the
/// integration lane adds `pub mod genchoice;` to lib.rs (REQUEST TO I-INT in notes/implementer.sizegen.md).
#[path = "genchoice.rs"]
pub mod genchoice;

/// Lane sizegen (interfaces/genchoice.md 5): the improvement record and improve mode (same I-INT request as genchoice).
#[path = "improve.rs"]
pub mod improve;

/// Lane sizegen ITEM 2 (interfaces/genchoice.md 6): pole score and pole objective of the generation choice and improve.
#[path = "polescore.rs"]
pub mod polescore;

/// Lane sizegen (TRICKS 31 / 35): the effective derivation of every generated / replaced rule.
#[path = "derivation.rs"]
pub mod derivation;
