//! The ACCEPTANCE CHAIN for one candidate rule in the context of a node (interface `interfaces/acceptance.md`;
//! plan 2.4.5, 2.4.7 D3/D6/D7, 2.5, 2.6; R8, R9, R13 A6, R29, R33, R38, R40 U-9/U-12).
//!
//! `accept_candidate` runs the ordered checks Record -> Served -> Coverage -> Regularity -> Descent -> Evidence ->
//! Exclusions -> Scope and returns ONE verdict with an evidence record the ledger can cite. A LIBRARY (R22): external
//! tools enter only through the back-end traits ([`facts::Regularity`], [`facts::CoefficientFacts`],
//! [`prover::EvidenceProver`]); a file-backed back-end without an answer makes the chain return `NeedsExternal` with
//! the request files to write. No master count, knock-out flag or old residual count enters anywhere (U-9).

pub mod descent;
pub mod facts;
pub mod ladder_adapters;
pub mod prover;
pub mod recipe;
pub mod record;
pub mod rule;

use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

use serde::Serialize;

use crate::closure::bodies::Sectors;
use crate::cover::{Assurance, Dispatch, Dispatcher, RuleRef, Table};
use crate::domain::{DomainOptions, Stage, TargetMap};
use crate::key::{Key, NodeFrame};
use crate::order::NodeComparator;
use crate::partition::{Containment, DomainList, DomainSource, shadowed};
use crate::poly::Poly;
use crate::record::{ReadOptions, RhsMode, read_table_file};

use descent::{
    DescentAt, DescentFailure, Instantiator, descent_at, symbolic_not_proved,
    symbolic_regularity_not_proved,
};
use facts::{CoefficientFacts, RegVerdict, Regularity};
pub use prover::RequiredEvidence;
use prover::{
    EvidenceArtefact, EvidenceProver, ExternalRequest, KeyFailure, KeyStatus, ProverAnswer,
};
use record::AcceptanceRecord;
use rule::{CreditWitness, RuleText, record_credit_witness, sha256_tag};

/// FULL (complete coverage of the sector, R33) or BOXED (user box, R35) node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTarget {
    Full,
    Boxed { box_id: String },
}

/// What the chain needs to know about the node.
pub struct NodeView<'a> {
    pub node: String,
    pub frame: &'a NodeFrame,
    pub cmp: &'a NodeComparator,
    pub target: NodeTarget,
    /// Per-node spec value (U-12): `Replay` (complete nodes) or `PerKey` (boxed nodes).
    pub required: RequiredEvidence,
    /// Dropped-line masks of zero sectors (hand-downs that vanish).
    pub zero_sectors: Option<BTreeSet<u64>>,
    /// Declared terminals (preferred + other, R41 P-4): never dispatched, never served.
    pub terminals: BTreeSet<Key>,
}

impl NodeView<'_> {
    pub fn arity(&self) -> usize {
        self.frame.arity()
    }
    pub fn lines(&self) -> Vec<bool> {
        self.frame.mask().to_vec()
    }
}

/// The attempt that produced the candidate (ledgerfix: an accept cites its producing attempt).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Producer {
    pub job: String,
    pub attempt: u32,
    pub table_digest: String,
}

/// Where the candidate's record lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleSource {
    pub table_id: String,
    pub table_path: String,
    pub table_digest: String,
    /// 0-based position among the table's rule records.
    pub ordinal: u64,
    pub byte_offset: Option<u64>,
    pub producer: Option<Producer>,
    /// The source table header's `guard_version` (int3 F2: a record without its own `guard_version` takes the
    /// table's, as the identity test does); `None` = the table has none (or no header).
    pub table_guard_version: Option<i64>,
}

/// Priority classes of the dispatch order (plan 2.4.7 D1), in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityClass {
    Input,
    FaceOrLocus,
    Point,
    SymmetryCredit,
    Terminal,
}

/// A stated position in the dispatch order: sorted by (class, input priority, fixer round, item id, rule index)
/// (D1, D6).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Position {
    pub class: PriorityClass,
    pub input_priority: u32,
    pub round: u32,
    pub item: String,
    pub index: u32,
}

/// The candidate.
pub struct Candidate<'a> {
    pub rule: &'a RuleText,
    pub source: RuleSource,
    pub position: Position,
}

/// Failure kinds a key can have (the exclusion policy chooses which of them become exclusions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// The candidate is the first selected rule but cannot be used (pole locus, recenter bound, elision).
    Blocked,
    Irregular,
    Hidden,
    NonMembership,
    NotDescending,
}

/// The policy of check 6.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AcceptPolicy {
    /// Key failures that become exclusions; any other failure rejects the rule (default: plan 2.4.7 D7's list plus
    /// `Blocked`; `NotDescending` rejects, plan 2.4.4).
    pub exclude_on: BTreeSet<FailureKind>,
    /// Upper bound on prover rounds (each round removes at least one failed key).
    pub max_prover_rounds: usize,
}

impl Default for AcceptPolicy {
    fn default() -> Self {
        AcceptPolicy {
            exclude_on: [
                FailureKind::Blocked,
                FailureKind::Irregular,
                FailureKind::Hidden,
                FailureKind::NonMembership,
            ]
            .into_iter()
            .collect(),
            max_prover_rounds: 64,
        }
    }
}

/// The checks of the chain, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Record,
    Served,
    Coverage,
    Regularity,
    Descent,
    Evidence,
    Exclusions,
    Scope,
}

/// Exclusion target: a key, or a locus (affine equations) for exclusions handed over from the ladder.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionTarget {
    Key(Key),
    Locus { equations: Vec<String> },
}

/// One exclusion of the effective domain (plan 2.4.7 D7 (ii)).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Exclusion {
    pub target: ExclusionTarget,
    pub reason: FailureKind,
    /// The stored disproof / reason text (pole term, non-membership certificate, blocking verdict).
    pub disproof: String,
}

/// The effective domain = the rule's case minus its exclusions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EffectiveDomain {
    pub rule_sha256: String,
    pub exclusions: Vec<Exclusion>,
}

/// What a rejection points at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Witness {
    Structural(String),
    AtKey { key: Key, detail: String },
    Rule(String),
}

/// What an undecided chain is missing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub key: Option<Key>,
    pub what: String,
}

/// An ACCEPTED candidate: the only proof of acceptance assembly and the tables in force take (audit-accept-A M1,
/// B M2). Fields are private and there is no public constructor and no `Deserialize`: it is built ONLY by
/// [`accept_candidate`] at the end of the chain. It binds the rule (line digest, source), the node it was judged
/// in (name, target, frame + comparator identity), the evidence artefact, the effective domain (exclusions), the
/// scope, the served set (digest) and the SET of records in force when it was judged (so assembly can check that the
/// shipped dispatch order is the one judged, audit-accept-B C7).
#[derive(Clone, Debug, PartialEq)]
pub struct Accepted {
    position: Position,
    effective: EffectiveDomain,
    record: AcceptanceRecord,
    artefact: EvidenceArtefact,
    source: RuleSource,
    node: String,
    target: NodeTarget,
    node_identity: String,
    scope: Scope,
    in_force_digest: String,
    in_force_count: u64,
    served_digest: String,
    served_count: u64,
}

impl Accepted {
    pub fn position(&self) -> &Position {
        &self.position
    }
    /// The effective domain: the rule's case minus its exclusions (from the chain, never caller data).
    pub fn effective(&self) -> &EffectiveDomain {
        &self.effective
    }
    pub fn exclusions(&self) -> &[Exclusion] {
        &self.effective.exclusions
    }
    pub fn record(&self) -> &AcceptanceRecord {
        &self.record
    }
    /// The evidence artefact (the caller writes `body` / cites the answer file).
    pub fn artefact(&self) -> &EvidenceArtefact {
        &self.artefact
    }
    pub fn source(&self) -> &RuleSource {
        &self.source
    }
    /// `sha256:` of the rule line.
    pub fn rule_sha256(&self) -> &str {
        &self.effective.rule_sha256
    }
    pub fn node(&self) -> &str {
        &self.node
    }
    pub fn target(&self) -> &NodeTarget {
        &self.target
    }
    /// [`node_identity`] of the frame and comparator the chain judged in.
    pub fn node_identity(&self) -> &str {
        &self.node_identity
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    /// [`record_set_digest`] of the records in force before the candidate when it was judged, and their count.
    pub fn in_force(&self) -> (&str, u64) {
        (&self.in_force_digest, self.in_force_count)
    }
    /// Digest (`keys_digest`) and size of the final served set.
    pub fn served(&self) -> (&str, u64) {
        (&self.served_digest, self.served_count)
    }
}

/// The verdict of the chain.
#[derive(Clone, Debug, PartialEq)]
pub enum AcceptVerdict {
    Accepted(Box<Accepted>),
    Rejected {
        check: Check,
        witness: Witness,
    },
    Undecided {
        check: Check,
        missing: Vec<Missing>,
    },
    NeedsExternal {
        check: Check,
        requests: Vec<ExternalRequest>,
    },
}

impl AcceptVerdict {
    pub fn is_accepted(&self) -> bool {
        matches!(self, AcceptVerdict::Accepted(_))
    }
    /// The acceptance token, if accepted.
    pub fn accepted(&self) -> Option<&Accepted> {
        match self {
            AcceptVerdict::Accepted(a) => Some(a),
            _ => None,
        }
    }
    /// The check that decided a non-acceptance.
    pub fn check(&self) -> Option<Check> {
        match self {
            AcceptVerdict::Accepted(_) => None,
            AcceptVerdict::Rejected { check, .. }
            | AcceptVerdict::Undecided { check, .. }
            | AcceptVerdict::NeedsExternal { check, .. } => Some(*check),
        }
    }
}

/// Caller mistakes (every mathematical outcome is a verdict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptError {
    Arity { expected: usize, got: usize },
    Cover(String),
    Position(String),
    /// Building the tables in force (unreadable table, a token for another rule, an exclusion that can not be honoured).
    InForce(String),
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptError::Arity { expected, got } => write!(f, "arity {got}, node arity {expected}"),
            AcceptError::Cover(e) => write!(f, "dispatch: {e}"),
            AcceptError::Position(e) => write!(f, "position: {e}"),
            AcceptError::InForce(e) => write!(f, "tables in force: {e}"),
        }
    }
}
impl std::error::Error for AcceptError {}

/// Back-ends of one chain run: per-key facts (regularity + coefficient zero tests) and the evidence prover.
pub trait KeyFacts: Regularity + CoefficientFacts {}
impl<T: Regularity + CoefficientFacts> KeyFacts for T {}

/// Two fact sources combined: regularity from `R`, coefficients from `C` (e.g. EC-2 regularity, exact coefficients).
pub struct FactPair<R, C> {
    pub regularity: R,
    pub coefficients: C,
}
impl<R: Regularity, C> Regularity for FactPair<R, C> {
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        self.regularity.regular(rule, key)
    }
    fn engine_fact_digest(&self) -> Option<String> {
        self.regularity.engine_fact_digest()
    }
    fn fact_digest_for(&self, rule: &RuleText) -> Option<String> {
        self.regularity.fact_digest_for(rule)
    }
}
impl<R, C: CoefficientFacts> CoefficientFacts for FactPair<R, C> {
    fn coefficient(
        &mut self,
        rule: &RuleText,
        key: &[i32],
        term: usize,
    ) -> crate::closure::facts::CoefFact {
        self.coefficients.coefficient(rule, key, term)
    }
}

/// One record of the tables in force (in dispatch order): the table it comes from and the sha256 of its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InForceRecord {
    pub table: String,
    pub line_sha256: String,
}

/// THE TABLES IN FORCE of a chain run, built only by [`InForceBuilder`] from table FILES and [`Accepted`] tokens, so
/// that the chain knows the identity (line digest) of every record it judged against (audit-accept-B C7) and the
/// tables follow the acceptance chain's own rules: a record whose witness loci are present but whose guard version
/// (record, else table header) is < 2 is NOT CARRIED, as in check 0 (int3 F2); an accepted rule's EXCLUSIONS ARE
/// HONOURED (the record is not selected at an excluded key: an exception branch vanishing exactly at the key's rule
/// point; a locus exclusion = an exception branch of its equations, canonical targets only), so a later rule (a point
/// rule that makes the exclusion safe) can be judged there.
pub struct InForce {
    disp: Option<Dispatcher>,
    records: Vec<InForceRecord>,
    set_digest: String,
}

impl InForce {
    /// The dispatcher (None when nothing is in force).
    pub fn dispatcher(&self) -> Option<&Dispatcher> {
        self.disp.as_ref()
    }
    pub fn records(&self) -> &[InForceRecord] {
        &self.records
    }
    /// [`record_set_digest`] of the records' line digests.
    pub fn set_digest(&self) -> &str {
        &self.set_digest
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Order-independent digest of a MULTISET of line digests (`sha256:<hex>` each): the sum of the 256-bit SHA-256 values
/// of the digest texts modulo 2^256, as `adsum256:<hex>`. Used to compare the set of records in force when a rule was
/// judged with the set placed before it in the assembled table (the served set depends on that SET only, not on the
/// order among the earlier records).
pub fn record_set_digest<'a>(ids: impl IntoIterator<Item = &'a str>) -> String {
    let mut acc = [0u64; 4];
    for id in ids {
        let h = sha256_tag(id.as_bytes());
        let hex = h.trim_start_matches("sha256:");
        let mut carry = 0u128;
        for (i, a) in acc.iter_mut().enumerate() {
            let chunk = &hex[(3 - i) * 16..(4 - i) * 16];
            let v = u64::from_str_radix(chunk, 16).unwrap_or(0);
            let s = *a as u128 + v as u128 + carry;
            *a = s as u64;
            carry = s >> 64;
        }
    }
    format!(
        "adsum256:{:016x}{:016x}{:016x}{:016x}",
        acc[3], acc[2], acc[1], acc[0]
    )
}

/// Identity of the frame and comparator a rule is judged in (and an assembled table dispatches in): mask, power
/// shifts, two-sided slots, the comparator's presentation (slot permutation) and its equivalent order descriptor.
pub fn node_identity(frame: &NodeFrame, cmp: &NodeComparator) -> String {
    sha256_tag(
        format!(
            "mask={};power_shifts={:?};two_sided={:?};presentation={:?};order={:?}",
            frame.mask_string(),
            frame.power_shifts(),
            frame.two_sided(),
            cmp.presentation(),
            cmp.equivalent_descriptor()
        )
        .as_bytes(),
    )
}

/// Builder of [`InForce`]: tables and accepted rules in DISPATCH ORDER (call order).
pub struct InForceBuilder<'a> {
    frame: &'a NodeFrame,
    opts: DomainOptions,
    tables: Vec<Table>,
    records: Vec<InForceRecord>,
    /// the last table is a campaign table that takes further accepted rules
    campaign_open: bool,
}

impl<'a> InForceBuilder<'a> {
    pub fn new(frame: &'a NodeFrame) -> Self {
        InForceBuilder {
            frame,
            opts: DomainOptions::default(),
            tables: Vec::new(),
            records: Vec::new(),
            campaign_open: false,
        }
    }
    /// Domain options of the table files (default: the core's).
    pub fn options(mut self, opts: DomainOptions) -> Self {
        self.opts = opts;
        self
    }

    /// An input table file (`table_dump.v1`): loaded with the core loader, the acceptance guard rule applied to its
    /// records (int3 F2), every dispatched record identified by the sha256 of its line.
    pub fn table_file(&mut self, id: &str, path: &Path) -> Result<&mut Self, AcceptError> {
        let err = |e: String| AcceptError::InForce(format!("{}: {e}", path.display()));
        let tf = read_table_file(path, ReadOptions { rhs: RhsMode::Skip })
            .map_err(|e| err(e.to_string()))?;
        let mut t = Table::from_table_file(id, path.display().to_string(), &tf, self.opts)
            .map_err(|e| err(e.to_string()))?;
        let header_guard = tf
            .header
            .as_ref()
            .and_then(|h| h.raw.get("guard_version"))
            .and_then(serde_json::Value::as_i64);
        // line digests and own guard versions of the dispatched records (by line number, one streaming pass)
        let want: BTreeMap<usize, usize> = t
            .meta
            .iter()
            .enumerate()
            .map(|(ri, m)| (m.line_no, ri))
            .collect();
        if want.len() != t.len() || want.contains_key(&0) {
            return Err(err("records without distinct line numbers".into()));
        }
        let mut digests = vec![String::new(); t.len()];
        let mut guards: Vec<Option<i64>> = vec![None; t.len()];
        let mut rd = std::io::BufReader::with_capacity(
            1 << 20,
            std::fs::File::open(path).map_err(|e| err(e.to_string()))?,
        );
        let mut buf = Vec::new();
        let mut line_no = 0usize;
        loop {
            buf.clear();
            if rd
                .read_until(b'\n', &mut buf)
                .map_err(|e| err(e.to_string()))?
                == 0
            {
                break;
            }
            line_no += 1;
            if let Some(&ri) = want.get(&line_no) {
                let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
                digests[ri] = sha256_tag(line);
                guards[ri] = rule::record_guard_version(line)
                    .map_err(|e| err(format!("line {line_no}: {e}")))?;
            }
        }
        if digests.iter().any(String::is_empty) {
            return Err(err("a record line was not found again".into()));
        }
        // int3 F2: the acceptance chain's witness rule (loci AND guard version >= 2) on the records in force
        for (ri, m) in t.meta.iter_mut().enumerate() {
            let r = tf
                .rules
                .get(m.source_index)
                .ok_or_else(|| err("record meta out of range".into()))?;
            if m.witness == crate::cover::WitnessStatus::Complete
                && m.witness_basis == Assurance::Record
                && record_credit_witness(r, guards[ri], header_guard) == CreditWitness::NotCarried
            {
                m.witness = crate::cover::WitnessStatus::NotCarried;
            }
        }
        for d in digests {
            self.records.push(InForceRecord {
                table: id.to_string(),
                line_sha256: d,
            });
        }
        self.tables.push(t);
        self.campaign_open = false;
        Ok(self)
    }

    /// An ACCEPTED campaign rule (its token and its record text, which must be the token's rule), with the token's
    /// exclusions honoured.
    pub fn accepted(&mut self, acc: &Accepted, rule: &RuleText) -> Result<&mut Self, AcceptError> {
        if acc.rule_sha256() != rule.line_sha256 {
            return Err(AcceptError::InForce(format!(
                "token of rule {} given with the record {}",
                acc.rule_sha256(),
                rule.line_sha256
            )));
        }
        let mut dom = rule
            .domain
            .clone()
            .ok_or_else(|| AcceptError::InForce("accepted rule without a domain".into()))?;
        honour_exclusions(&mut dom, acc.exclusions())?;
        if !self.campaign_open {
            self.tables
                .push(Table::new(format!("campaign:{}", self.tables.len()), dom.arity));
            self.campaign_open = true;
        }
        let t = self.tables.last_mut().expect("just pushed");
        let label = t.len() as i64;
        t.push(label, dom)
            .map_err(|e| AcceptError::InForce(e.to_string()))?;
        self.records.push(InForceRecord {
            table: t.name.clone(),
            line_sha256: rule.line_sha256.clone(),
        });
        Ok(self)
    }

    pub fn build(self) -> Result<InForce, AcceptError> {
        let _ = self.frame;
        let set_digest = record_set_digest(self.records.iter().map(|r| r.line_sha256.as_str()));
        let disp = if self.tables.is_empty() {
            None
        } else {
            let n = self.tables.len();
            Some(
                Dispatcher::new(self.tables, (0..n).collect())
                    .map_err(|e| AcceptError::InForce(e.to_string()))?,
            )
        };
        Ok(InForce {
            disp,
            records: self.records,
            set_digest,
        })
    }
}

/// Honour exclusions in a domain (see [`InForce`]).
fn honour_exclusions(
    dom: &mut crate::domain::CaseDomain,
    exclusions: &[Exclusion],
) -> Result<(), AcceptError> {
    let n = dom.arity;
    for e in exclusions {
        match &e.target {
            ExclusionTarget::Key(k) => {
                if k.len() != n {
                    return Err(AcceptError::Arity {
                        expected: n,
                        got: k.len(),
                    });
                }
                let mut pt = Vec::new();
                if crate::closure::bodies::rule_point(dom, k, &mut pt) {
                    dom.exceptions.push(
                        (0..n)
                            .map(|i| Poly::slot_minus(i, i128::from(pt[i])))
                            .collect(),
                    );
                }
            }
            ExclusionTarget::Locus { equations } => {
                if dom.target != TargetMap::Canonical {
                    return Err(AcceptError::InForce(
                        "a locus exclusion on a rule with a non-canonical target can not be honoured".into(),
                    ));
                }
                let branch = equations
                    .iter()
                    .map(|q| Poly::parse_d_free(q, n))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| AcceptError::InForce(format!("locus exclusion: {e:?}")))?;
                if branch.is_empty() {
                    return Err(AcceptError::InForce("empty locus exclusion".into()));
                }
                dom.exceptions.push(branch);
            }
        }
    }
    Ok(())
}

/// Inputs of one chain run.
pub struct AcceptInput<'a> {
    pub node: NodeView<'a>,
    /// Tables in force placed BEFORE the candidate (None = nothing before it).
    pub before: Option<&'a InForce>,
    /// Rules placed AFTER the candidate (winner diff of a non-final position); None = the candidate is last.
    pub after: Option<&'a InForce>,
    pub candidate: Candidate<'a>,
    pub gap_points: &'a [Key],
    /// The working key range (box + envelope for boxed nodes; the checked range for FULL nodes).
    pub working: &'a [Key],
    pub policy: AcceptPolicy,
}

/// Where the composed dispatch (before, candidate, after) stops at a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Declared terminal: not dispatched.
    Terminal,
    /// A rule before the candidate decides (any verdict, incl. open ones).
    Before(Dispatch),
    /// The candidate applies: served.
    Served,
    /// The candidate is selected but blocked / undecided.
    Stopped { undecided: bool, verdict: String },
    /// Neither: a rule after it (or nothing).
    After(Option<Dispatch>),
}

/// First match over the concatenation (before, candidate, after), exactly as `Dispatcher::first_match` over the
/// concatenated table list (sequential composition; nothing is copied).
pub fn composed_stop(
    node: &NodeView,
    before: Option<&Dispatcher>,
    rule: &RuleText,
    after: Option<&Dispatcher>,
    key: &[i32],
) -> Result<Stop, AcceptError> {
    composed_stop_domain(node, before, rule.domain.as_ref(), after, key)
}

/// [`composed_stop`] for a bare domain (recipe rules carry a domain and no record).
pub fn composed_stop_domain(
    node: &NodeView,
    before: Option<&Dispatcher>,
    domain: Option<&crate::domain::CaseDomain>,
    after: Option<&Dispatcher>,
    key: &[i32],
) -> Result<Stop, AcceptError> {
    if node.terminals.contains(key) {
        return Ok(Stop::Terminal);
    }
    if let Some(b) = before {
        let d = b
            .first_match(key)
            .map_err(|e| AcceptError::Cover(e.to_string()))?;
        if d != Dispatch::Uncovered {
            return Ok(Stop::Before(d));
        }
    }
    let Some(dom) = domain else {
        return Ok(Stop::After(None));
    };
    let v = dom.applies(key);
    match v.stage() {
        Stage::Applies => Ok(Stop::Served),
        Stage::Blocked => Ok(Stop::Stopped {
            undecided: false,
            verdict: format!("{v:?}"),
        }),
        Stage::Undecided => Ok(Stop::Stopped {
            undecided: true,
            verdict: format!("{v:?}"),
        }),
        Stage::NotSelected => match after {
            None => Ok(Stop::After(None)),
            Some(a) => Ok(Stop::After(Some(
                a.first_match(key)
                    .map_err(|e| AcceptError::Cover(e.to_string()))?,
            ))),
        },
    }
}

/// The served / stopped / displaced sets of check 1 (also used by the winner diff after an assembly change).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServedSets {
    pub served: Vec<Key>,
    /// Keys where the candidate stops dispatch without serving (verdict text; true = undecided).
    pub stopped: Vec<(Key, bool, String)>,
    /// Keys a rule placed after the candidate served before (now the candidate's).
    pub displaced: Vec<Key>,
    pub terminals_in_range: usize,
    pub working: usize,
}

/// Check 1 over a working range.
pub fn served_sets(
    node: &NodeView,
    before: Option<&Dispatcher>,
    rule: &RuleText,
    after: Option<&Dispatcher>,
    working: &[Key],
) -> Result<ServedSets, AcceptError> {
    let mut seen = BTreeSet::new();
    let mut out = ServedSets::default();
    for k in working {
        if k.len() != node.arity() {
            return Err(AcceptError::Arity {
                expected: node.arity(),
                got: k.len(),
            });
        }
        if !seen.insert(k.clone()) {
            continue;
        }
        out.working += 1;
        let stop = composed_stop(node, before, rule, after, k)?;
        let took_from_after = |out: &mut ServedSets| {
            if let Some(a) = after
                && let Ok(Dispatch::Covered(_)) = a.first_match(k)
            {
                out.displaced.push(k.clone());
            }
        };
        match stop {
            Stop::Terminal => out.terminals_in_range += 1,
            Stop::Served => {
                took_from_after(&mut out);
                out.served.push(k.clone());
            }
            Stop::Stopped { undecided, verdict } => {
                took_from_after(&mut out);
                out.stopped.push((k.clone(), undecided, verdict));
            }
            Stop::Before(_) | Stop::After(_) => {}
        }
    }
    Ok(out)
}

fn describe_stop(s: &Stop) -> String {
    match s {
        Stop::Terminal => "declared terminal".into(),
        Stop::Before(d) => format!("decided before the candidate: {d:?}"),
        Stop::Served => "served".into(),
        Stop::Stopped { verdict, .. } => format!("candidate selected but not usable: {verdict}"),
        Stop::After(None) => "not in the candidate's domain".into(),
        Stop::After(Some(d)) => format!("not in the candidate's domain; after it: {d:?}"),
    }
}

/// Per-key bookkeeping of the chain.
#[derive(Default)]
struct KeyLedger {
    failed: BTreeMap<Key, (FailureKind, Check, String)>,
    unknown: BTreeMap<Key, (Check, String)>,
}

impl KeyLedger {
    fn fail(&mut self, k: &Key, kind: FailureKind, check: Check, detail: String) {
        self.failed
            .entry(k.clone())
            .or_insert((kind, check, detail));
        self.unknown.remove(k);
    }
    fn unknown(&mut self, k: &Key, check: Check, why: String) {
        if !self.failed.contains_key(k) {
            self.unknown.entry(k.clone()).or_insert((check, why));
        }
    }
    fn open(&self, k: &Key) -> bool {
        !self.failed.contains_key(k) && !self.unknown.contains_key(k)
    }
}

/// Outcome summaries of the checks (for the record).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CheckOutcome {
    pub check: Option<Check>,
    pub outcome: String,
    pub counts: BTreeMap<String, u64>,
}

fn outcome(check: Check, text: &str, counts: &[(&str, u64)]) -> CheckOutcome {
    CheckOutcome {
        check: Some(check),
        outcome: text.to_string(),
        counts: counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    }
}

/// The scope a record certifies (check 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// FULL node: the symbolic conditions hold on the whole effective domain.
    FullDomain,
    /// Only the working range (digest) is certified; `missing` = why not the whole domain (FULL nodes).
    KeyRange {
        digest: String,
        missing: Vec<String>,
    },
}

/// Run the acceptance chain (interface section 2).
pub fn accept_candidate(
    input: &AcceptInput,
    facts: &mut dyn KeyFacts,
    prover: &mut dyn EvidenceProver,
) -> Result<AcceptVerdict, AcceptError> {
    let node = &input.node;
    let rule = input.candidate.rule;
    let n = node.arity();
    let mut checks: Vec<CheckOutcome> = Vec::new();

    // ---- 0 Record (A0)
    if rule.record.fixed.len() != n {
        return Err(AcceptError::Arity {
            expected: n,
            got: rule.record.fixed.len(),
        });
    }
    let pos = &input.candidate.position;
    match pos.class {
        PriorityClass::Point if !rule.is_point() => {
            return Err(AcceptError::Position(
                "class Point for a rule with free slots".into(),
            ));
        }
        PriorityClass::FaceOrLocus if rule.is_point() => {
            return Err(AcceptError::Position(
                "class FaceOrLocus for a point rule".into(),
            ));
        }
        PriorityClass::SymmetryCredit | PriorityClass::Terminal => {
            return Err(AcceptError::Position(
                "a rule is never placed in the credit / terminal class".into(),
            ));
        }
        _ => {}
    }
    let rejected = |check, w| Ok(AcceptVerdict::Rejected { check, witness: w });
    if let Some(e) = &rule.domain_error {
        return rejected(Check::Record, Witness::Structural(format!("domain: {e}")));
    }
    if rule.empty_exception_branch {
        return rejected(
            Check::Record,
            Witness::Structural("empty exception AND-list".into()),
        );
    }
    // witness loci (R24 (6)) and guard version: a record without its loci (or pre-guard-fix) may credit a key ONLY
    // with per-key exact engine facts (coordinator ruling, addendum 3); elided loci are never liftable
    // the facts must be bound to THIS rule (audit-accept-A M4, B C2): `fact_digest_for(rule)`, never a rule-free digest
    let table_guard = input.candidate.source.table_guard_version;
    let fact_digest = facts.fact_digest_for(rule);
    let credit_basis = match rule.credit_witness(table_guard) {
        CreditWitness::Elided => {
            return rejected(
                Check::Record,
                Witness::Structural("witness loci elided (R24 (6))".into()),
            );
        }
        CreditWitness::Complete => "witness_loci".to_string(),
        CreditWitness::NotCarried => match &fact_digest {
            Some(d) => format!("regularity_fact:{d}"),
            None => {
                return Ok(AcceptVerdict::Undecided {
                    check: Check::Record,
                    missing: vec![Missing {
                        key: None,
                        what: format!(
                            "witness {:?}, guard version {:?}: credit needs per-key exact engine facts (rr_proof \
                             instantiate) loaded for this rule",
                            rule.witness_status(),
                            rule.effective_guard(table_guard)
                        ),
                    }],
                });
            }
        },
    };
    checks.push(outcome(Check::Record, "pass", &[]));

    // ---- 1 Served (winner diff over the working range)
    let before = input.before.and_then(InForce::dispatcher);
    let after = input.after.and_then(InForce::dispatcher);
    let sets = served_sets(node, before, rule, after, input.working)?;
    let mut shadow = None;
    if node.target == NodeTarget::Full
        && let (Some(b), Some(dom)) = (before, &rule.domain)
    {
        // audit-part-A M3 / m5 (partfix): "shadowed by the tables in force" is a FIRST-MATCH statement: an earlier
        // record counts where it is SELECTED (served or blocked), not only where it applies (`contained_in` is
        // union coverage since partfix, and its `NotContained` witness may be a key where an earlier record blocks,
        // so it would not show that the candidate wins). `partition::shadowed` with the candidate placed after the
        // tables in force, as acceptance.md check 1 states (int3 integration).
        let mut src = DomainList::new(dom.arity);
        for &t in b.priority() {
            let tab = &b.tables()[t];
            for (r, d) in tab.domains.iter().enumerate() {
                src.push(RuleRef::bare(t, r, tab.labels[r]), d.clone());
            }
        }
        src.push(RuleRef::bare(b.tables().len(), 0, 0), dom.clone());
        let c = shadowed(&src, src.len() - 1, node.frame);
        if let Containment::Proved { certificate } = &c {
            return rejected(
                Check::Served,
                Witness::Rule(format!(
                    "shadowed on the whole domain by the tables in force: {certificate}"
                )),
            );
        }
        shadow = Some(c);
    }
    if sets.served.is_empty() {
        return rejected(
            Check::Served,
            Witness::Structural(format!(
                "wins no key of the working range ({} keys)",
                sets.working
            )),
        );
    }
    checks.push(outcome(
        Check::Served,
        "pass",
        &[
            ("working", sets.working as u64),
            ("served", sets.served.len() as u64),
            ("stopped", sets.stopped.len() as u64),
            ("displaced", sets.displaced.len() as u64),
            ("terminals_in_range", sets.terminals_in_range as u64),
        ],
    ));
    let mut led = KeyLedger::default();
    for (k, undecided, v) in &sets.stopped {
        if *undecided {
            led.unknown(k, Check::Served, v.clone());
        } else {
            led.fail(k, FailureKind::Blocked, Check::Served, v.clone());
        }
    }

    // ---- 2 Coverage at the gap points (credit names the rule, R8)
    let served_set: BTreeSet<&Key> = sets.served.iter().collect();
    let mut not_credited: Vec<(Key, String)> = Vec::new();
    let mut credited: Vec<Key> = Vec::new();
    for g in input.gap_points {
        if served_set.contains(g) {
            credited.push(g.clone());
        } else {
            let s = composed_stop(node, before, rule, after, g)?;
            not_credited.push((g.clone(), describe_stop(&s)));
        }
    }
    if credited.is_empty() {
        return rejected(
            Check::Coverage,
            Witness::Structural(format!(
                "serves none of the {} gap points",
                input.gap_points.len()
            )),
        );
    }
    checks.push(outcome(
        Check::Coverage,
        "pass",
        &[
            ("credited", credited.len() as u64),
            ("not_credited", not_credited.len() as u64),
        ],
    ));

    // ---- 3 Regularity at every served key (exact, per key, R38)
    for k in &sets.served {
        match facts.regular(rule, k) {
            RegVerdict::Regular => {}
            RegVerdict::Pole { term } => led.fail(
                k,
                FailureKind::Irregular,
                Check::Regularity,
                format!("pole of term {term}"),
            ),
            RegVerdict::Hidden { locus } => led.fail(
                k,
                FailureKind::Hidden,
                Check::Regularity,
                format!("witness locus {locus} vanishes"),
            ),
            RegVerdict::Unknown(w) => led.unknown(k, Check::Regularity, w),
        }
    }
    checks.push(outcome(
        Check::Regularity,
        "done",
        &[
            ("failed", led.failed.len() as u64),
            ("unknown", led.unknown.len() as u64),
        ],
    ));

    // ---- 4 Descent at every served key (comparator with tie-breaks; coefficient zero verified)
    let inst = match Instantiator::new(rule) {
        Ok(i) => i,
        Err(e) => return rejected(Check::Descent, Witness::Structural(format!("rhs: {e}"))),
    };
    let sectors = Sectors {
        lines: node.lines(),
        zero_sectors: node.zero_sectors.clone(),
    };
    let mut dropped_zero = 0u64;
    for k in &sets.served {
        if !led.open(k) {
            continue;
        }
        match descent_at(&inst, &sectors, node.cmp, facts, rule, k) {
            DescentAt::Pass {
                dropped_zero: z, ..
            } => dropped_zero += z as u64,
            DescentAt::Fail(f) => {
                let d = match f {
                    DescentFailure::NotLower { term, rhs } => {
                        format!("term {term} rhs {rhs:?} not lower, coefficient nonzero")
                    }
                    DescentFailure::PositiveNumeratorNonZero { term, rhs } => {
                        format!("term {term} rhs {rhs:?} positive numerator, coefficient nonzero")
                    }
                };
                led.fail(k, FailureKind::NotDescending, Check::Descent, d)
            }
            DescentAt::Unknown { term, rhs, why } => {
                led.unknown(k, Check::Descent, format!("term {term} rhs {rhs:?}: {why}"))
            }
        }
    }
    checks.push(outcome(
        Check::Descent,
        "done",
        &[
            ("failed", led.failed.len() as u64),
            ("unknown", led.unknown.len() as u64),
            ("dropped_zero", dropped_zero),
        ],
    ));

    // Non-excludable failures reject; unknowns make the chain undecided BEFORE any evidence is spent.
    if let Some(v) = reject_or_undecide(&led, &input.policy) {
        return Ok(v);
    }

    // ---- 5 Evidence at the required level, for EXACTLY the served set
    if !node.required.satisfied_by(prover.level()) {
        return Ok(AcceptVerdict::Undecided {
            check: Check::Evidence,
            missing: vec![Missing {
                key: None,
                what: format!(
                    "the prover gives {:?}, the node requires {:?}",
                    prover.level(),
                    node.required
                ),
            }],
        });
    }
    let mut artefact: Option<EvidenceArtefact> = None;
    let mut rule_failure: Option<prover::RuleLevel> = None;
    let mut rounds = 0;
    loop {
        let keys: Vec<Key> = sets
            .served
            .iter()
            .filter(|k| led.open(k))
            .cloned()
            .collect();
        if keys.is_empty() {
            break;
        }
        rounds += 1;
        if rounds > input.policy.max_prover_rounds {
            return Ok(AcceptVerdict::Undecided {
                check: Check::Evidence,
                missing: vec![Missing {
                    key: None,
                    what: "prover rounds exhausted".into(),
                }],
            });
        }
        match prover.prove(rule, &input.candidate.source, &keys) {
            ProverAnswer::NeedsExternal(requests) => {
                return Ok(AcceptVerdict::NeedsExternal {
                    check: Check::Evidence,
                    requests,
                });
            }
            ProverAnswer::Unavailable(why) => {
                return Ok(AcceptVerdict::Undecided {
                    check: Check::Evidence,
                    missing: vec![Missing {
                        key: None,
                        what: format!("prover unavailable: {why}"),
                    }],
                });
            }
            ProverAnswer::PerKey {
                keys: statuses,
                artefact: art,
                rule_level,
            } => {
                let answered: BTreeMap<&Key, &KeyStatus> =
                    statuses.iter().map(|(k, s)| (k, s)).collect();
                let mut new_failures = false;
                for k in &keys {
                    match answered.get(k) {
                        Some(KeyStatus::Reached(l)) if node.required.satisfied_by(*l) => {}
                        Some(KeyStatus::Reached(l)) => {
                            led.unknown(k, Check::Evidence, format!("reached {l:?} only"))
                        }
                        Some(KeyStatus::Failed(f)) => {
                            new_failures = true;
                            let (kind, d) = match f {
                                KeyFailure::NonMembership { disproof } => {
                                    (FailureKind::NonMembership, disproof.clone())
                                }
                                KeyFailure::Irregular { detail } => {
                                    (FailureKind::Irregular, detail.clone())
                                }
                                KeyFailure::Hidden { detail } => {
                                    (FailureKind::Hidden, detail.clone())
                                }
                            };
                            led.fail(k, kind, Check::Evidence, d);
                        }
                        Some(KeyStatus::NotCredited(w)) => {
                            led.unknown(k, Check::Evidence, format!("prover: {w}, d1 says applies"))
                        }
                        Some(KeyStatus::Unsupported(w)) => {
                            led.unknown(k, Check::Evidence, format!("unsupported: {w}"))
                        }
                        Some(KeyStatus::Unknown(w)) => led.unknown(k, Check::Evidence, w.clone()),
                        None => led.unknown(
                            k,
                            Check::Evidence,
                            "the prover did not answer this key".into(),
                        ),
                    }
                }
                if let Some(r) = &rule_level {
                    if r.disproof {
                        return rejected(Check::Evidence, Witness::Rule(r.text.clone()));
                    }
                    // a rule-level failure: no key reaches the level (statuses say Unknown / Failed per key)
                    rule_failure = Some(r.clone());
                }
                artefact = art;
                if !new_failures {
                    break;
                }
                // keys failed: the artefact must cover EXACTLY the remaining served set: ask again
            }
        }
    }
    let n_reached = sets.served.iter().filter(|k| led.open(k)).count() as u64;
    if let Some(r) = &rule_failure
        && n_reached > 0
    {
        // a rule-level failure can not coexist with reached keys: fail closed
        return Ok(AcceptVerdict::Undecided {
            check: Check::Evidence,
            missing: vec![Missing {
                key: None,
                what: format!("rule-level evidence failure: {}", r.text),
            }],
        });
    }
    checks.push(outcome(
        Check::Evidence,
        "done",
        &[
            ("reached", n_reached),
            ("failed", led.failed.len() as u64),
            ("unknown", led.unknown.len() as u64),
            ("rounds", rounds as u64),
        ],
    ));

    // ---- 6 Exclusions
    if let Some(v) = reject_or_undecide(&led, &input.policy) {
        return Ok(v);
    }
    let final_served: Vec<Key> = sets
        .served
        .iter()
        .filter(|k| led.open(k))
        .cloned()
        .collect();
    let credited_final: Vec<Key> = credited.iter().filter(|k| led.open(k)).cloned().collect();
    for k in credited.iter().filter(|k| !led.open(k)) {
        let (_, _, d) = &led.failed[k];
        not_credited.push((k.clone(), format!("excluded: {d}")));
    }
    if credited_final.is_empty() {
        let (k, (kind, check, d)) = led
            .failed
            .iter()
            .find(|(k, _)| credited.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap_or((
                credited[0].clone(),
                (FailureKind::Blocked, Check::Exclusions, String::new()),
            ));
        return rejected(
            check,
            Witness::AtKey {
                key: k,
                detail: format!("{kind:?}: {d}; no gap point left"),
            },
        );
    }
    let art = match artefact {
        Some(a) if a.keys_digest == prover::keys_digest(n, &final_served) => a,
        _ => {
            return Ok(AcceptVerdict::Undecided {
                check: Check::Evidence,
                missing: vec![Missing {
                    key: None,
                    what: "no evidence artefact for exactly the served set".into(),
                }],
            });
        }
    };
    // the artefact must be evidence for THIS rule (audit-accept-B M1); a replay record is checked by content
    if let Err(why) = artefact_binds(&art, rule, &final_served) {
        return Ok(AcceptVerdict::Undecided {
            check: Check::Evidence,
            missing: vec![Missing {
                key: None,
                what: format!("evidence artefact not bound to the candidate: {why}"),
            }],
        });
    }
    let exclusions: Vec<Exclusion> = led
        .failed
        .iter()
        .map(|(k, (kind, _, d))| Exclusion {
            target: ExclusionTarget::Key(k.clone()),
            reason: *kind,
            disproof: d.clone(),
        })
        .collect();
    checks.push(outcome(
        Check::Exclusions,
        "pass",
        &[("exclusions", exclusions.len() as u64)],
    ));

    // ---- 7 Scope
    let working_digest = prover::keys_digest(n, input.working);
    let not_proved = symbolic_not_proved(rule, node.cmp, &node.lines());
    let regularity_not_proved = symbolic_regularity_not_proved(rule);
    let scope = match node.target {
        NodeTarget::Boxed { .. } => Scope::KeyRange {
            digest: working_digest.clone(),
            missing: vec![],
        },
        NodeTarget::Full => {
            let mut missing = Vec::new();
            if !exclusions.is_empty() {
                missing.push(format!("{} exclusions", exclusions.len()));
            }
            if art.schema != crate::ibp_rows::REPLAY_SCHEMA {
                missing
                    .push("identity not tested at generic points (per-key artefact only)".into());
            }
            if !not_proved.is_empty() {
                missing.push(format!(
                    "symbolic descent not proved for terms {not_proved:?}"
                ));
            }
            // audit-accept-B C3: regularity on the whole effective domain (denominators, witness loci)
            missing.extend(
                regularity_not_proved
                    .iter()
                    .map(|w| format!("symbolic regularity: {w}")),
            );
            if credit_basis != "witness_loci" {
                missing.push("witness loci not carried (credited by per-key facts only)".into());
            }
            match &shadow {
                Some(Containment::NotContained { .. }) => {}
                Some(Containment::Unknown { reason }) => {
                    missing.push(format!("shadowing undecided: {reason}"))
                }
                _ => missing.push("shadowing not decided".into()),
            }
            if missing.is_empty() {
                Scope::FullDomain
            } else {
                Scope::KeyRange {
                    digest: working_digest.clone(),
                    missing,
                }
            }
        }
    };
    checks.push(outcome(
        Check::Scope,
        if scope == Scope::FullDomain {
            "full_domain"
        } else {
            "key_range"
        },
        &[],
    ));

    let effective = EffectiveDomain {
        rule_sha256: rule.line_sha256.clone(),
        exclusions: exclusions.clone(),
    };
    let identity = node_identity(node.frame, node.cmp);
    let (in_force_digest, in_force_count) = match input.before {
        Some(b) => (b.set_digest().to_string(), b.len() as u64),
        None => (record_set_digest(std::iter::empty()), 0),
    };
    let record = AcceptanceRecord::build(record::RecordParts {
        node,
        candidate: &input.candidate,
        policy: &input.policy,
        working_digest,
        working_count: sets.working as u64,
        served: &final_served,
        stopped: sets.stopped.len() as u64,
        displaced: sets.displaced.len() as u64,
        gap_count: input.gap_points.len() as u64,
        credited: &credited_final,
        not_credited: &not_credited,
        checks: &checks,
        exclusions: &exclusions,
        evidence_level: node.required.as_evidence(),
        artefact: &art,
        scope: &scope,
        shadowed: shadow.as_ref().map(|c| format!("{c:?}")),
        pattern_not_proved: not_proved.iter().copied().collect(),
        regularity_not_proved: &regularity_not_proved,
        credit_basis,
        node_identity: &identity,
        in_force: (&in_force_digest, in_force_count),
    });
    Ok(AcceptVerdict::Accepted(Box::new(Accepted {
        position: input.candidate.position.clone(),
        effective,
        record,
        artefact: art,
        source: input.candidate.source.clone(),
        node: node.node.clone(),
        target: node.target.clone(),
        node_identity: identity,
        scope,
        in_force_digest,
        in_force_count,
        served_digest: prover::keys_digest(n, &final_served),
        served_count: final_served.len() as u64,
    })))
}

/// The artefact names the candidate's rule; a replay record (in-process body) is checked BY CONTENT: its rule digest
/// is the candidate's (`ibp_rows::rule_digest`), its served-key digest is the final served set's, its level is
/// `Established` and its family was bound (rows3 NEW-M1).
fn artefact_binds(art: &EvidenceArtefact, rule: &RuleText, served: &[Key]) -> Result<(), String> {
    if art.rule_sha256 != rule.line_sha256 {
        return Err(format!(
            "artefact names rule {}, the candidate is {}",
            art.rule_sha256, rule.line_sha256
        ));
    }
    if art.schema != crate::ibp_rows::REPLAY_SCHEMA {
        return Ok(());
    }
    let body = art
        .body
        .as_ref()
        .ok_or("replay artefact without its record")?;
    let rr = crate::ibp_rows::RowsRule::from_value(rule.value.clone(), rule.record.fixed.len())
        .map_err(|e| format!("identity test refuses the record: {e}"))?;
    if body.get("rule_digest").and_then(serde_json::Value::as_str)
        != Some(crate::ibp_rows::rule_digest(&rr).as_str())
    {
        return Err("replay record of another rule (rule_digest)".into());
    }
    let mut keys: Vec<Vec<i64>> = served
        .iter()
        .map(|k| k.iter().map(|&x| i64::from(x)).collect())
        .collect();
    keys.sort();
    keys.dedup();
    if body
        .get("served_keys_digest")
        .and_then(serde_json::Value::as_str)
        != Some(crate::ibp_rows::key_set_digest(&keys).as_str())
    {
        return Err("replay record of another served set (served_keys_digest)".into());
    }
    let established = serde_json::to_value(crate::ibp_rows::ReplayLevel::Established)
        .map_err(|e| e.to_string())?;
    if body.get("level") != Some(&established) {
        return Err(format!("replay record level {:?}", body.get("level")));
    }
    if body.get("family_bound_to_table") != Some(&serde_json::Value::Bool(true)) {
        return Err("replay record without a family binding".into());
    }
    Ok(())
}

fn reject_or_undecide(led: &KeyLedger, policy: &AcceptPolicy) -> Option<AcceptVerdict> {
    let mut worst: Option<(&Key, &(FailureKind, Check, String))> = None;
    for (k, v) in &led.failed {
        if !policy.exclude_on.contains(&v.0) && worst.is_none_or(|(_, w)| v.1 < w.1) {
            worst = Some((k, v));
        }
    }
    if let Some((k, (kind, check, d))) = worst {
        return Some(AcceptVerdict::Rejected {
            check: *check,
            witness: Witness::AtKey {
                key: k.clone(),
                detail: format!("{kind:?}: {d}"),
            },
        });
    }
    if !led.unknown.is_empty() {
        let check = led
            .unknown
            .values()
            .map(|(c, _)| *c)
            .min()
            .unwrap_or(Check::Evidence);
        return Some(AcceptVerdict::Undecided {
            check,
            missing: led
                .unknown
                .iter()
                .map(|(k, (c, w))| Missing {
                    key: Some(k.clone()),
                    what: format!("{c:?}: {w}"),
                })
                .collect(),
        });
    }
    None
}
