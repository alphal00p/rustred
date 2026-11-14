//! The ladder of one item as a pure state machine (plan 2.4.4 transition table, D-35, D-43; 2.4.13 grid; 2.4.14 (b)
//! selection): [`next_steps`] reads the item's state (its attempt history keyed by attempt key, the point verdicts
//! and the key evidence the driver recorded) and returns the next attempts, the next facts it needs, or the final
//! disposition. Acceptance of a candidate at a gap point is [`judge_point`] (d1-d4 through the core domain, the
//! driver's facts and the FULL node comparator).
//!
//! GOVERNING PRINCIPLE (audits ladder-A / ladder-B, repair-library.md v1.2 section 0): nothing is recorded as
//! accepted / covered / terminal beyond what the trait answers justify; every "unknown / unsupported / unavailable /
//! error" answer is NOT a justification; every request kind has a counter and the item has a global step bound
//! ([`Budgets::max_item_steps`]); an accepted cover member is a pair (rule, EFFECTIVE DOMAIN = written case minus its
//! [`Exclusion`]s), safe under first-match dispatch at every gap point of the item whatever the order of the members.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::affine::AffineEq;
use super::children::{derive_children, excepting_branch};
use super::items::{Budgets, CaseSpec, EvidenceLevel, Item, ItemId, ItemKind, Key, NodeParams};
use super::seed::{AttemptBudget, SeedSetting, attempt_budget, engine_seed_args, seed_space};
use crate::domain::{CaseDomain, Stage, Verdict};
use crate::economy::SelectionScore;
use crate::repair::{Descent, FactSource};

/// Rung of an attempt: the seed search (`L1s`, contains L0's target attempt) on a case, or its point form (L4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rung {
    SeedSearch,
    Point,
}

/// One engine call to make.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub item: ItemId,
    /// "<item id>|<setting canonical key>": never re-run once it has a VERDICT outcome (non-verdict outcomes are
    /// retried in a larger resource class up to `Budgets::nonverdict_retries`; interrupts as counted).
    pub key: String,
    pub case: super::items::CaseSpec,
    pub setting: SeedSetting,
    pub rung: Rung,
    /// Index of the wave of the item's seed space (built-in policy: 0 = depth 2, 1 = depth 3).
    pub wave: u8,
    pub budget: AttemptBudget,
    pub seed_args: Vec<String>,
    /// 1 + earlier answers of this key that were not verdicts (interrupts and non-verdict outcomes).
    pub attempt_no: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interruption {
    Operator,
    Infra,
}

/// A rule returned by the case solver (the driver builds `domain` from the rule record: case, exception branches,
/// pole and witness loci, elision).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateRule {
    pub domain: CaseDomain,
    pub digest: String,
    pub rhs_terms: u64,
    pub monomials: Option<u64>,
    pub max_degree: Option<u32>,
    pub bytes: Option<u64>,
    pub rows: Option<u64>,
    /// Informative only (the comparator decides descent).
    pub tie_break_only: Option<bool>,
    /// The rule was generated with symmetry rows: a per-key exact proof (instance-span) is `Unsupported` for it; a
    /// node requiring per-key evidence can then not credit it (route to the replay level only where the spec allows).
    pub symmetry_rows: bool,
    /// What the rule's SOURCE shows about its witness loci (core fix 2, campaign-core-domain.md section 8): only
    /// `Complete` can be credited; `Elided` / `NotCarried` make the rule unusable wherever it would be selected (an
    /// exclusion there). A candidate built from an export takes its status from `Table.meta`; new campaign jobs run
    /// with no elision (normally `Complete`).
    pub witness: crate::cover::WitnessStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveOutcome {
    Rules(Vec<CandidateRule>),
    /// No relation within the shells of the setting.
    NoRelation,
    BudgetStop {
        time_decided: bool,
    },
    RefusedHeavy {
        reason: String,
    },
    ProfileRefused {
        reason: String,
    },
    KilledMemory,
    Interrupted {
        by: Interruption,
    },
    Error(String),
}

/// Class of a solver outcome (C3 of audit ladder-A). Only a VERDICT exhausts a setting: the solver searched the
/// shells and returned rules (which acceptance then judges) or found no relation. Everything else says nothing about
/// the existence of a relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutcomeClass {
    /// `Rules` or `NoRelation`.
    Verdict,
    /// Crash (`Error`), memory kill, heavy / profile refusal, budget stop (by wall clock or work counter: the shells
    /// were not searched to the end): retried in a larger resource class, then ATTENTION; never a terminal.
    NonVerdict,
    /// Infrastructure interrupt (counted, capped by `infra_retries`; never a verdict).
    Infra,
    /// Operator interrupt: never counts against a retry cap (U-5); only a sanity bound
    /// (`max_operator_interrupts`) sends the item to ATTENTION.
    Operator,
}

impl SolveOutcome {
    pub fn tag(&self) -> &'static str {
        match self {
            SolveOutcome::Rules(_) => "rules",
            SolveOutcome::NoRelation => "no_relation",
            SolveOutcome::BudgetStop { .. } => "budget_stop",
            SolveOutcome::RefusedHeavy { .. } => "refused_heavy",
            SolveOutcome::ProfileRefused { .. } => "profile_refused",
            SolveOutcome::KilledMemory => "killed_memory",
            SolveOutcome::Interrupted { .. } => "interrupted",
            SolveOutcome::Error(_) => "error",
        }
    }

    pub fn class(&self) -> OutcomeClass {
        match self {
            SolveOutcome::Rules(_) | SolveOutcome::NoRelation => OutcomeClass::Verdict,
            SolveOutcome::Interrupted {
                by: Interruption::Operator,
            } => OutcomeClass::Operator,
            SolveOutcome::Interrupted {
                by: Interruption::Infra,
            } => OutcomeClass::Infra,
            _ => OutcomeClass::NonVerdict,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolveAnswer {
    pub outcome: SolveOutcome,
    pub cpu_ms: u64,
    pub rows: Option<u64>,
}

/// History entry of one attempt key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    pub setting: SeedSetting,
    pub wave: u8,
    /// The VERDICT answer (`Rules` / `NoRelation`), once there is one; never overwritten.
    pub answer: Option<SolveAnswer>,
    /// Non-verdict answers in arrival order (tags; the setting is retried while there are at most
    /// `Budgets::nonverdict_retries` of them).
    pub non_verdict: Vec<String>,
    pub operator_interrupts: u32,
    pub infra_interrupts: u32,
    pub cpu_ms_total: u64,
    /// The case the record's candidates were generated for, when it is not the item's own case (a reserve candidate
    /// inherited from the parent item, see [`InheritedCandidate`]).
    pub case: Option<CaseSpec>,
}

impl AttemptRecord {
    fn empty(setting: SeedSetting, wave: u8) -> Self {
        AttemptRecord {
            setting,
            wave,
            answer: None,
            non_verdict: Vec::new(),
            operator_interrupts: 0,
            infra_interrupts: 0,
            cpu_ms_total: 0,
            case: None,
        }
    }

    /// The setting has a VERDICT outcome.
    pub fn is_verdict(&self) -> bool {
        self.answer.is_some()
    }

    /// Nothing more will be asked for this setting: a verdict, or the non-verdict / infra caps are used up (the
    /// setting then has NO verdict: it can never count for an exhaustion token).
    pub fn settled(&self, b: &Budgets) -> bool {
        self.answer.is_some()
            || self.non_verdict.len() as u64 > b.nonverdict_retries as u64
            || self.infra_interrupts > b.infra_retries
    }

    /// Settled without a verdict.
    pub fn settled_without_verdict(&self, b: &Budgets) -> bool {
        self.answer.is_none() && self.settled(b)
    }

    pub fn candidates(&self) -> &[CandidateRule] {
        match &self.answer {
            Some(SolveAnswer {
                outcome: SolveOutcome::Rules(r),
                ..
            }) => r,
            _ => &[],
        }
    }
}

/// Per-key facts of a candidate (rr_proof instantiate as a library, EC-2; R38: regularity per key, exact, no
/// factoring of denominators).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointFacts {
    pub regular: Tri,
    /// Top-sector RHS keys whose coefficient is not identically zero at the key; `None` = unknown.
    pub rhs_top: Option<Vec<Key>>,
    pub exceptions_complete: Tri,
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tri {
    Pass,
    Fail,
    Unknown,
}

/// Verdict of a candidate at one gap point (only `Covered` credits).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointVerdict {
    Covered,
    /// Not selected by first match (fixed slot / equation / target constant differ): dispatch moves on.
    NotInCase,
    /// An exception branch vanishes: not selected, dispatch moves on.
    OnException {
        branch: usize,
    },
    /// A pole locus vanishes: the rule IS selected and blocks the key (needs an exclusion where another rule serves).
    OnPole,
    /// Witness loci elided: selected, unusable (fail closed; needs an exclusion).
    Elided,
    /// Selected but unusable for another reason (chart mismatch, recenter bound): needs an exclusion.
    Blocked {
        why: String,
    },
    /// d2 failed with no exception branch there: the rule's exceptions are incomplete (rejects the candidate).
    HiddenPole,
    NotDescending {
        rhs: Key,
    },
    Unknown {
        why: String,
    },
}

impl PointVerdict {
    pub fn covered(&self) -> bool {
        matches!(self, PointVerdict::Covered)
    }
    /// First-match dispatch provably moves past the rule at this key (no exclusion needed there).
    pub fn not_selected(&self) -> bool {
        matches!(
            self,
            PointVerdict::NotInCase | PointVerdict::OnException { .. }
        )
    }
}

/// d1-d4 at one key (plan 2.4.5): d1 through the core domain (fixed, chart, equations, every exception branch, pole
/// loci, elision fail-closed), d2 and d4 from the facts, d3 with the FULL node comparator on every top RHS key.
pub fn judge_point(
    rule: &CandidateRule,
    key: &[i32],
    facts: &PointFacts,
    descent: &dyn Descent,
) -> PointVerdict {
    let v = rule.domain.applies(key);
    match v {
        Verdict::Applies => match rule.witness {
            crate::cover::WitnessStatus::Complete => {}
            crate::cover::WitnessStatus::Elided => return PointVerdict::Elided,
            crate::cover::WitnessStatus::NotCarried => {
                return PointVerdict::Blocked {
                    why: "witness loci not carried by the source".into(),
                };
            }
        },
        Verdict::OnException { branch } => return PointVerdict::OnException { branch },
        Verdict::OnPole { .. } => return PointVerdict::OnPole,
        Verdict::Elided => return PointVerdict::Elided,
        Verdict::Error(e) => return PointVerdict::Unknown { why: e.to_string() },
        other => {
            return match other.stage() {
                Stage::NotSelected => PointVerdict::NotInCase,
                Stage::Blocked => PointVerdict::Blocked {
                    why: format!("{other:?}"),
                },
                _ => PointVerdict::Unknown {
                    why: format!("{other:?}"),
                },
            };
        }
    }
    match facts.regular {
        Tri::Pass => {}
        Tri::Fail => return PointVerdict::HiddenPole,
        Tri::Unknown => {
            return PointVerdict::Unknown {
                why: "regularity unknown".into(),
            };
        }
    }
    let Some(rhs) = &facts.rhs_top else {
        return PointVerdict::Unknown {
            why: "rhs keys unknown".into(),
        };
    };
    for r in rhs {
        if !descent.lower(r, key) {
            return PointVerdict::NotDescending { rhs: r.clone() };
        }
    }
    match facts.exceptions_complete {
        Tri::Pass => PointVerdict::Covered,
        Tri::Fail => PointVerdict::HiddenPole,
        Tri::Unknown => PointVerdict::Unknown {
            why: "exception completeness unknown (d4)".into(),
        },
    }
}

const NO_FACTS: PointFacts = PointFacts {
    regular: Tri::Unknown,
    rhs_top: None,
    exceptions_complete: Tri::Unknown,
    note: None,
};

/// Judge one candidate at a list of keys through a fact source (driver helper).
pub fn judge_points(
    rule: &CandidateRule,
    keys: &[Key],
    facts: &mut dyn FactSource,
    descent: &dyn Descent,
) -> Vec<PointVerdict> {
    keys.iter()
        .map(|k| {
            // d1 first: facts are only fetched for keys inside the domain
            match rule.domain.applies(k) {
                Verdict::Applies => {
                    let f = facts.facts(rule, k);
                    judge_point(rule, k, &f, descent)
                }
                _ => judge_point(rule, k, &NO_FACTS, descent),
            }
        })
        .collect()
}

/// [`judge_points`], plus the number of (credited key, top RHS key) pairs whose RHS key is a PREFERRED terminal
/// (selection tie-break, P-4; computed from the same facts).
pub fn judge_points_with_hits(
    rule: &CandidateRule,
    keys: &[Key],
    facts: &mut dyn FactSource,
    descent: &dyn Descent,
    preferred: &super::items::PreferredTerminals,
    lines: &[bool],
) -> (Vec<PointVerdict>, u64) {
    let mut hits = 0u64;
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        match rule.domain.applies(k) {
            Verdict::Applies => {
                let f = facts.facts(rule, k);
                let v = judge_point(rule, k, &f, descent);
                if v.covered()
                    && let Some(rhs) = &f.rhs_top
                {
                    hits = hits.saturating_add(
                        rhs.iter().filter(|r| preferred.matches(r, lines)).count() as u64,
                    );
                }
                out.push(v);
            }
            _ => out.push(judge_point(rule, k, &NO_FACTS, descent)),
        }
    }
    (out, hits)
}

/// Proof failures of the key prover (A s.2: kept apart, never PASS).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ProofFailure {
    /// EXACT_NOT_IN_SPAN_AT_KEY (exact certificate) / FAIL_AT_KEY.
    NonMembership,
    Fail,
    /// HIDDEN_KEY: a witness-locus pole at the key.
    Hidden,
    /// IRREGULAR_KEY.
    Irregular,
    /// NOT_IN_SPAN_MODULAR: residual non-zero at two points, no exact certificate.
    NotInSpanModular,
    /// UNDECIDED (reason).
    Undecided(String),
    /// The tool cannot decide this rule / key at all (typed, never an assumption): e.g. "symmetry rows" (rules
    /// generated with symmetry rows cannot go to instance-span), "arity" (compiled arity), "affine case without a
    /// +-1 coefficient", "witness loci elided".
    Unsupported(String),
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvidence {
    pub key: Key,
    pub level: Option<EvidenceLevel>,
    pub failure: Option<ProofFailure>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerdictRequest {
    pub attempt_key: String,
    pub index: usize,
    pub keys: Vec<Key>,
}

/// Identity evidence for a candidate at these keys. Issued AT MOST ONCE per candidate: after one recorded answer,
/// a key the answer does not establish at `level` is NOT credited (no re-request; C2 of audit ladder-A).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRequest {
    pub attempt_key: String,
    pub index: usize,
    pub keys: Vec<Key>,
    pub level: EvidenceLevel,
}

// ------------------------------------------------------------------------------------------------------------------
// Exclusions (C1)

/// Where a rule must not be dispatched.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExclusionTarget {
    Key(Key),
    /// Every key where all equations hold (an AND-list, presented frame).
    Locus {
        equations: Vec<AffineEq>,
    },
}

impl ExclusionTarget {
    pub fn holds(&self, key: &[i32]) -> bool {
        match self {
            ExclusionTarget::Key(k) => k.as_slice() == key,
            ExclusionTarget::Locus { equations } => equations.iter().all(|e| e.holds(key)),
        }
    }
}

/// Why a key (or locus) is excluded from a rule's effective domain.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExclusionReason {
    /// d3 failed: a top RHS key is not lower under the full node comparator.
    NotDescending { rhs: Key },
    /// A pole locus vanishes at the key (the rule would be selected and block).
    OnPole,
    /// Witness loci elided (selected, unusable).
    Elided,
    /// Selected but unusable (chart mismatch, recenter bound).
    Blocked { why: String },
    /// Coverage (d1 evaluation, d2, rhs keys or d4) unknown.
    CoverageUnknown { why: String },
    /// The prover reported a failure (if a pass was ALSO recorded for the key, `conflicting_pass` is set: failure wins).
    IdentityFailed {
        failure: ProofFailure,
        conflicting_pass: bool,
    },
    /// The prover answered without a level and without a failure.
    IdentityUnknown,
    /// The prover's answer did not mention the key.
    IdentityNoAnswer,
    /// Evidence below the level the node's spec requires.
    EvidenceBelowLevel {
        got: EvidenceLevel,
        required: EvidenceLevel,
    },
    /// A gap point of the same tree inside the rule's written domain that THIS rule was not credited at (served by
    /// another rule, or by an item judged elsewhere): the rule is not dispatched there.
    NotCreditedHere,
    /// A preferred terminal of the spec inside the rule's domain (P-4: keeps the terminal).
    PreferredTerminal,
    /// The locus of a child item read from this rule's POLE locus: the rule is excluded there so that the child is
    /// reached (a pole blocks first-match dispatch).
    PoleLocusChild { child: ItemId },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Exclusion {
    pub target: ExclusionTarget,
    pub reason: ExclusionReason,
}

pub(crate) fn add_exclusion(list: &mut Vec<Exclusion>, e: Exclusion) {
    if !list.iter().any(|x| x.target == e.target) {
        list.push(e);
    }
}

/// The reason a NON-crediting verdict needs an exclusion, `None` where dispatch provably moves past the rule.
pub fn exclusion_reason(v: &PointVerdict) -> Option<ExclusionReason> {
    match v {
        PointVerdict::Covered | PointVerdict::NotInCase | PointVerdict::OnException { .. } => None,
        PointVerdict::OnPole => Some(ExclusionReason::OnPole),
        PointVerdict::Elided => Some(ExclusionReason::Elided),
        PointVerdict::Blocked { why } => Some(ExclusionReason::Blocked { why: why.clone() }),
        PointVerdict::HiddenPole => Some(ExclusionReason::CoverageUnknown {
            why: "hidden pole".into(),
        }),
        PointVerdict::NotDescending { rhs } => {
            Some(ExclusionReason::NotDescending { rhs: rhs.clone() })
        }
        PointVerdict::Unknown { why } => {
            Some(ExclusionReason::CoverageUnknown { why: why.clone() })
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// The exhaustion token (C3, (e))

/// The token that a POINT's ladder failed completely: EVERY setting of the policy's seed space of the item has a
/// VERDICT outcome (no crash, kill, refusal, budget stop or interrupt counts) and no candidate was accepted at the
/// key. Fields are private and it has no `Deserialize`: the only constructors are the ladder (`next_steps`) and
/// [`LadderExhaustedRecord::verify`], which re-checks the item state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LadderExhausted {
    item: ItemId,
    key: Key,
    /// Number of verdict outcomes (= the size of the seed space).
    attempts: u32,
    settings: u32,
    depth3_tried: bool,
    depth3_unfloored_tried: bool,
    /// The space contains a depth >= 3 unfloored setting (with the built-in policy always; a spec policy may
    /// replace the space).
    space_has_depth3_unfloored: bool,
    parent: Option<ItemId>,
}

impl LadderExhausted {
    /// Mint the token from the item state, or `None` if a setting lacks a verdict.
    pub(crate) fn mint(state: &ItemState, params: &NodeParams, key: &[i32]) -> Option<Self> {
        let item = &state.item;
        if !item.gap_points.iter().any(|k| k.as_slice() == key) {
            return None;
        }
        let space: Vec<SeedSetting> = seed_space(&item.case, params)
            .into_iter()
            .flatten()
            .collect();
        if space.is_empty() {
            return None;
        }
        let mut recs = Vec::new();
        for s in &space {
            let r = state.records.get(&attempt_key(&item.id, s))?;
            if !r.is_verdict() {
                return None;
            }
            recs.push(r);
        }
        Some(LadderExhausted {
            item: item.id.clone(),
            key: key.to_vec(),
            attempts: recs.len() as u32,
            settings: space.len() as u32,
            depth3_tried: recs.iter().any(|r| r.setting.depth >= 3),
            depth3_unfloored_tried: recs
                .iter()
                .any(|r| r.setting.depth >= 3 && !r.setting.floor),
            space_has_depth3_unfloored: space.iter().any(|s| s.depth >= 3 && !s.floor),
            parent: item.parent.clone(),
        })
    }

    pub fn item(&self) -> &ItemId {
        &self.item
    }
    pub fn key(&self) -> &[i32] {
        &self.key
    }
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
    pub fn settings(&self) -> u32 {
        self.settings
    }
    pub fn depth3_tried(&self) -> bool {
        self.depth3_tried
    }
    pub fn depth3_unfloored_tried(&self) -> bool {
        self.depth3_unfloored_tried
    }
    pub fn parent(&self) -> Option<&ItemId> {
        self.parent.as_ref()
    }
    /// The deep condition of plan 2.4.4 (7): if the space has a depth >= 3 unfloored setting, it was tried (always
    /// true for a minted token; kept as an explicit check for the terminal set).
    pub fn deep_condition_met(&self) -> bool {
        !self.space_has_depth3_unfloored || self.depth3_unfloored_tried
    }
    /// The unverified, serialisable form.
    pub fn to_record(&self) -> LadderExhaustedRecord {
        LadderExhaustedRecord {
            item: self.item.clone(),
            key: self.key.clone(),
            attempts: self.attempts,
            settings: self.settings,
            depth3_tried: self.depth3_tried,
            depth3_unfloored_tried: self.depth3_unfloored_tried,
            space_has_depth3_unfloored: self.space_has_depth3_unfloored,
            parent: self.parent.clone(),
        }
    }
    #[cfg(test)]
    pub(crate) fn for_tests(key: Key, depth3_unfloored_tried: bool) -> Self {
        LadderExhausted {
            item: ItemId("it1-test".into()),
            key,
            attempts: 0,
            settings: 0,
            depth3_tried: true,
            depth3_unfloored_tried,
            space_has_depth3_unfloored: true,
            parent: None,
        }
    }
}

/// UNVERIFIED (deserialisable) form of a [`LadderExhausted`] token: becomes a token only through [`Self::verify`]
/// against the item's recorded state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LadderExhaustedRecord {
    pub item: ItemId,
    pub key: Key,
    pub attempts: u32,
    pub settings: u32,
    pub depth3_tried: bool,
    pub depth3_unfloored_tried: bool,
    #[serde(default)]
    pub space_has_depth3_unfloored: bool,
    pub parent: Option<ItemId>,
}

impl LadderExhaustedRecord {
    /// Re-verify against the item state: the item must be the record's, every setting of its seed space must have a
    /// verdict, and the state machine must propose exactly this key as a terminal.
    pub fn verify(
        &self,
        state: &ItemState,
        params: &NodeParams,
    ) -> Result<LadderExhausted, String> {
        if state.item.id != self.item {
            return Err("token names another item".into());
        }
        match next_steps(state, params) {
            Next::Done(Disposition::TerminalCandidates { keys, exhausted })
                if keys.contains(&self.key) =>
            {
                let mut t = exhausted;
                t.key = self.key.clone();
                if t.to_record() != *self {
                    return Err("token fields differ from the recorded state".into());
                }
                Ok(t)
            }
            other => Err(format!(
                "the item state does not propose this key as a terminal: {other:?}"
            )),
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Covers

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverMember {
    pub attempt_key: String,
    pub index: usize,
    pub setting: SeedSetting,
    pub digest: String,
    pub rhs_terms: u64,
    /// The case the rule was generated for (its WRITTEN case; for an inherited reserve candidate the parent's).
    pub case: CaseSpec,
    /// Gap points this member serves (first member in cover order that credits the key).
    pub served: Vec<Key>,
    /// Every gap point the member is CREDITED at (verdict covered and evidence at the required level); served ⊆
    /// credited.
    pub credited: Vec<Key>,
    /// The member's EFFECTIVE DOMAIN is its written domain minus these: every gap point (of the item, and after
    /// [`crate::repair::seal_tree`] of the whole tree) inside the written domain where the member is not credited.
    pub exclusions: Vec<Exclusion>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cover {
    pub members: Vec<CoverMember>,
    /// The set cover was computed greedily (more than [`EXACT_COVER_MAX`] candidates).
    pub greedy: bool,
}

impl Cover {
    pub fn cases(&self) -> usize {
        self.members.len()
    }
}

/// Why an item needs a human (typed; never silent, never a terminal).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionReason {
    /// The item's global step bound (`Budgets::max_item_steps`) was reached (C2).
    StepBound { steps: u64, bound: u64 },
    /// Operator interrupts of one attempt above the sanity bound (U-5: never a retry cap; a human decides).
    OperatorInterrupts {
        attempt: String,
        count: u32,
        bound: u32,
    },
    /// A POINT item has no accepted rule but some settings of its space have NO verdict outcome (crash, kill,
    /// refusal, budget stop, infra cap): never a terminal proposal (C3).
    NonVerdict { settings: Vec<String> },
    /// A child's dimension above the root's (malformed input).
    DimensionAboveRoot,
    /// Left-over gap points no child could be built for (they would be lost).
    UnplacedPoints { keys: Vec<Key> },
    /// The seed space of the case is empty (a policy that offers nothing for this case).
    EmptySeedSpace,
    /// A point case with several gap points left (malformed case; audit B m1).
    MalformedPointCase { keys: Vec<Key> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Disposition {
    /// Every gap point is served by the cover at the node's evidence level.
    Covered { cover: Cover },
    /// Part (or none) of the gap points served; the rest assigned to children (sub-cases, then points).
    /// `alternatives`: other standard planes holding left-over points; a driver releases one only for the points its
    /// primary children leave unserved (after their whole subtree), so no point gets two rules.
    /// `reserve`: accepted candidates kept out of the cover by the economy pruning; they are handed to the children
    /// whose gap points they credit as INHERITED candidates (judged and selected there with the child's own ones,
    /// before the child splits further), so a pruned symbolic rule never loses to point rules (audit A M1).
    Split {
        cover: Cover,
        children: Vec<Item>,
        alternatives: Vec<Item>,
        reserve: Vec<CoverMember>,
    },
    /// A point whose ladder failed with a VERDICT on every setting: terminal PROPOSAL (L5; declared only at S9).
    TerminalCandidates {
        keys: Vec<Key>,
        exhausted: LadderExhausted,
    },
    /// A counter was exceeded or the input is unusable; never silent.
    Attention {
        reason: AttentionReason,
        why: String,
    },
    /// Optimise item: no acceptable replacement; the old rules stay (U-13).
    OptimiseKept { why: String },
}

impl Disposition {
    fn attention(reason: AttentionReason) -> Self {
        let why = format!("{reason:?}");
        Disposition::Attention { reason, why }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Next {
    Run(Vec<Attempt>),
    NeedVerdicts(Vec<VerdictRequest>),
    NeedEvidence(Vec<EvidenceRequest>),
    /// Optimise items only: containment of the old rules' domains in the candidate's.
    NeedContainment(Vec<ContainmentRequest>),
    /// WAVE 0 (R43): ask the candidate source(s) for offers on this item; record them with
    /// [`ItemState::record_offers`] (an empty list is a valid answer).
    NeedOffers,
    Done(Disposition),
}

/// A reserve candidate of the parent handed to a child (economy pruning, decision D-L4 as revised): judged and
/// selected at the child's gap points like the child's own candidates (wave 0, no solver CPU).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InheritedCandidate {
    pub rule: CandidateRule,
    /// The rule's written case (the parent's case).
    pub case: CaseSpec,
    pub setting: SeedSetting,
    /// "<parent attempt key>#<index>".
    pub from: String,
}

/// State of one item: the item and everything the driver recorded for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub item: Item,
    pub records: BTreeMap<String, AttemptRecord>,
    pub verdicts: BTreeMap<(String, usize), Vec<PointVerdict>>,
    pub evidence: BTreeMap<(String, usize), Vec<KeyEvidence>>,
    pub containment: BTreeMap<(String, usize), Tri>,
    /// Preferred-terminal RHS hits per candidate (P-4 selection tie-break); absent = 0.
    pub preferred_hits: BTreeMap<(String, usize), u64>,
    /// Wave 0 answered (offers recorded, possibly none).
    pub offers_recorded: bool,
    /// Number of driver answers recorded (every `record_*` call): the item's global step counter (C2).
    pub steps: u64,
}

impl ItemState {
    pub fn new(item: Item) -> Self {
        let mut st = ItemState {
            records: BTreeMap::new(),
            verdicts: BTreeMap::new(),
            evidence: BTreeMap::new(),
            containment: BTreeMap::new(),
            preferred_hits: BTreeMap::new(),
            offers_recorded: false,
            steps: 0,
            item,
        };
        for (n, ic) in st.item.inherited.clone().into_iter().enumerate() {
            let mut r = AttemptRecord::empty(ic.setting.clone(), 0);
            r.answer = Some(SolveAnswer {
                outcome: SolveOutcome::Rules(vec![ic.rule]),
                cpu_ms: 0,
                rows: None,
            });
            r.case = Some(ic.case);
            st.records
                .insert(offer_key(&st.item.id, &format!("inherited{n}")), r);
        }
        st
    }

    /// Record the wave-0 offers (R43) as one pseudo-attempt "<item>|offer|<source>" with zero solver CPU.
    pub fn record_offers(&mut self, source: &str, offers: Vec<CandidateRule>) {
        self.steps += 1;
        self.offers_recorded = true;
        if offers.is_empty() {
            return;
        }
        let key = offer_key(&self.item.id, source);
        if self.records.contains_key(&key) {
            return; // never overwritten
        }
        let mut r = AttemptRecord::empty(
            SeedSetting {
                centre: super::seed::CentreKind::Explicit,
                offsets: vec![],
                floor: false,
                depth: 0,
                tight: false,
                layer_cap: None,
                symmetry_rows: false,
            },
            0,
        );
        r.answer = Some(SolveAnswer {
            outcome: SolveOutcome::Rules(offers),
            cpu_ms: 0,
            rows: None,
        });
        self.records.insert(key, r);
    }

    pub fn record_preferred_hits(&mut self, attempt_key: &str, index: usize, hits: u64) {
        if hits > 0 {
            self.preferred_hits
                .insert((attempt_key.to_string(), index), hits);
        }
    }

    pub fn record_containment(&mut self, attempt_key: &str, index: usize, t: Tri) {
        self.steps += 1;
        self.containment
            .entry((attempt_key.to_string(), index))
            .or_insert(t);
    }

    /// Record the solver's answer to an attempt. A verdict is final and never overwritten; a non-verdict outcome
    /// or an interrupt keeps the key open (counted per class, C3 / U-5).
    pub fn record_answer(&mut self, attempt: &Attempt, answer: SolveAnswer) {
        self.steps += 1;
        let r = self
            .records
            .entry(attempt.key.clone())
            .or_insert_with(|| AttemptRecord::empty(attempt.setting.clone(), attempt.wave));
        if r.answer.is_some() {
            return; // never re-run, never overwritten
        }
        // interrupt CPU is not attempt CPU: it never feeds the economy extra budget (audit B M2, D-35)
        if !matches!(
            answer.outcome.class(),
            OutcomeClass::Operator | OutcomeClass::Infra
        ) {
            r.cpu_ms_total = r.cpu_ms_total.saturating_add(answer.cpu_ms);
        }
        match answer.outcome.class() {
            OutcomeClass::Operator => {
                r.operator_interrupts = r.operator_interrupts.saturating_add(1)
            }
            OutcomeClass::Infra => r.infra_interrupts = r.infra_interrupts.saturating_add(1),
            OutcomeClass::NonVerdict => r.non_verdict.push(answer.outcome.tag().to_string()),
            OutcomeClass::Verdict => r.answer = Some(answer),
        }
    }

    /// Verdicts of a candidate (recorded once; a later answer for the same candidate is ignored).
    pub fn record_verdicts(
        &mut self,
        attempt_key: &str,
        index: usize,
        verdicts: Vec<PointVerdict>,
    ) {
        self.steps += 1;
        self.verdicts
            .entry((attempt_key.to_string(), index))
            .or_insert(verdicts);
    }

    /// Evidence of a candidate. The FIRST call for a candidate closes its evidence round: keys it does not
    /// establish (no entry, no level, a failure, a lower level) are not credited; conflicting entries for one key:
    /// failure wins.
    pub fn record_evidence(&mut self, attempt_key: &str, index: usize, ev: Vec<KeyEvidence>) {
        self.steps += 1;
        self.evidence
            .entry((attempt_key.to_string(), index))
            .or_default()
            .extend(ev);
    }

    /// Attempts with a VERDICT outcome.
    pub fn attempts_made(&self) -> usize {
        self.records.values().filter(|r| r.is_verdict()).count()
    }
}

/// Attempt key of the wave-0 offers of a source.
pub fn offer_key(item: &ItemId, source: &str) -> String {
    format!("{}|offer|{source}", item.0)
}

/// Attempt key of an item and setting.
pub fn attempt_key(item: &ItemId, s: &SeedSetting) -> String {
    format!("{}|{}", item.0, s.canonical_key())
}

/// Candidates with more than this many members get a greedy set cover (flagged in [`Cover::greedy`]).
pub const EXACT_COVER_MAX: usize = 24;

/// One accepted candidate: its place, its credited gap points (after verdicts and evidence), its score parts.
#[derive(Clone, Debug)]
pub(crate) struct Accepted {
    pub key: String,
    pub index: usize,
    pub order: usize,
    pub setting: SeedSetting,
    pub case: CaseSpec,
    pub rule: CandidateRule,
    /// Gap point indices credited by the verdicts (minus keys whose evidence failed).
    pub credited: BTreeSet<usize>,
    /// Gap point indices still lacking evidence at the required level (never asked yet).
    pub lacking: BTreeSet<usize>,
    /// Gap point indices inside the written domain that are NOT credited, with the exclusion reason.
    pub uncredited: BTreeMap<usize, ExclusionReason>,
    pub preferred_hits: u64,
}

/// The total seed-search space of an item (the sub-counter, D-43).
pub fn sub_counter(state: &ItemState, params: &NodeParams) -> usize {
    seed_space(&state.item.case, params)
        .iter()
        .map(|w| w.len())
        .sum()
}

/// Evidence judgement of one key from every recorded entry (fail closed; failure wins over a pass).
enum EvJudge {
    Pass,
    Lacking,
    Fail(ExclusionReason),
}

fn judge_evidence(evs: Option<&Vec<KeyEvidence>>, key: &[i32], min: EvidenceLevel) -> EvJudge {
    let Some(evs) = evs else {
        return EvJudge::Lacking;
    };
    let mine: Vec<&KeyEvidence> = evs.iter().filter(|e| e.key.as_slice() == key).collect();
    if mine.is_empty() {
        return EvJudge::Fail(ExclusionReason::IdentityNoAnswer);
    }
    let passes = mine
        .iter()
        .any(|e| e.failure.is_none() && e.level.is_some_and(|l| l >= min));
    if let Some(f) = mine.iter().find_map(|e| e.failure.clone()) {
        return EvJudge::Fail(ExclusionReason::IdentityFailed {
            failure: f,
            conflicting_pass: passes,
        });
    }
    if mine.iter().any(|e| e.level.is_none()) {
        return EvJudge::Fail(ExclusionReason::IdentityUnknown);
    }
    if let Some(low) = mine.iter().filter_map(|e| e.level).find(|&l| l < min) {
        return EvJudge::Fail(ExclusionReason::EvidenceBelowLevel {
            got: low,
            required: min,
        });
    }
    EvJudge::Pass
}

/// The accepted candidates of the finished attempts in `keys` (in enumeration order), with credited gap points.
/// Returns `Err(requests)` if verdicts are missing.
pub(crate) fn accepted_candidates(
    state: &ItemState,
    params: &NodeParams,
    settings: &[(usize, SeedSetting, String)],
) -> Result<Vec<Accepted>, Vec<VerdictRequest>> {
    let gp = &state.item.gap_points;
    let mut need = Vec::new();
    let mut out = Vec::new();
    for (order, setting, key) in settings {
        let Some(rec) = state.records.get(key) else {
            continue;
        };
        for (idx, cand) in rec.candidates().iter().enumerate() {
            let Some(v) = state.verdicts.get(&(key.clone(), idx)) else {
                need.push(VerdictRequest {
                    attempt_key: key.clone(),
                    index: idx,
                    keys: gp.clone(),
                });
                continue;
            };
            // a rule found WITH symmetry rows whose class cannot reach the node's required level is never accepted:
            // "found, awaiting a certificate route" (kept in the records, shown by explain; addendum 3 of R46)
            if (setting.symmetry_rows || cand.symmetry_rows)
                && !params.capabilities.can(true, params.min_evidence)
            {
                continue;
            }
            if v.len() != gp.len() || v.iter().any(|x| matches!(x, PointVerdict::HiddenPole)) {
                continue; // incomplete exceptions (or malformed answer): rejected for this item
            }
            let mut credited: BTreeSet<usize> = (0..gp.len()).filter(|&i| v[i].covered()).collect();
            if credited.is_empty() {
                continue; // covers none of its gap points: rejected (the checker found 5 such rules)
            }
            let mut uncredited: BTreeMap<usize, ExclusionReason> = (0..gp.len())
                .filter_map(|i| exclusion_reason(&v[i]).map(|r| (i, r)))
                .collect();
            let mut lacking = BTreeSet::new();
            if params.min_evidence > EvidenceLevel::A1 {
                let ev = state.evidence.get(&(key.clone(), idx));
                for &i in &credited.clone() {
                    match judge_evidence(ev, &gp[i], params.min_evidence) {
                        EvJudge::Pass => {}
                        EvJudge::Lacking => {
                            lacking.insert(i);
                        }
                        EvJudge::Fail(r) => {
                            credited.remove(&i);
                            uncredited.insert(i, r);
                        }
                    }
                }
                if credited.is_empty() {
                    continue;
                }
            }
            out.push(Accepted {
                key: key.clone(),
                index: idx,
                order: *order,
                setting: setting.clone(),
                case: rec.case.clone().unwrap_or_else(|| state.item.case.clone()),
                rule: cand.clone(),
                credited,
                lacking,
                uncredited,
                preferred_hits: state
                    .preferred_hits
                    .get(&(key.clone(), idx))
                    .copied()
                    .unwrap_or(0),
            });
        }
    }
    if need.is_empty() { Ok(out) } else { Err(need) }
}

fn score(a: &Accepted, n_points: usize) -> SelectionScore {
    SelectionScore {
        failed: (n_points - a.credited.len()) as u64,
        cases: 1,
        size_class: SelectionScore::size_class_of(a.rule.rhs_terms),
        preferred_misses: u64::MAX - a.preferred_hits,
        rhs_terms: a.rule.rhs_terms,
        monomials: a.rule.monomials.unwrap_or(u64::MAX),
        degree: a.rule.max_degree.unwrap_or(u32::MAX),
        order: a.order as u64,
    }
}

/// Best ONE-CASE cover among accepted candidates (credits every gap point), by the economy selection score.
pub(crate) fn best_one_case(acc: &[Accepted], n_points: usize) -> Option<usize> {
    (0..acc.len())
        .filter(|&i| acc[i].credited.len() == n_points)
        .min_by_key(|&i| score(&acc[i], n_points))
}

/// Deterministic tie key of a candidate (no collision: tuples, not `order * 1000 + index`).
fn tie(a: &Accepted) -> (usize, usize) {
    (a.order, a.index)
}

/// (volume, tie keys, members) of the best exact cover found so far.
type BestCover = (u64, Vec<(usize, usize)>, Vec<usize>);

/// Minimum set cover over the accepted candidates of the coverable points: exact (fewest members, then smallest
/// RHS volume, then enumeration order) up to [`EXACT_COVER_MAX`] candidates and 6 members, else greedy.
pub(crate) fn min_cover(acc: &[Accepted]) -> (Vec<usize>, bool) {
    let coverable: BTreeSet<usize> = acc
        .iter()
        .flat_map(|a| a.credited.iter().copied())
        .collect();
    if coverable.is_empty() {
        return (vec![], false);
    }
    let n = acc.len();
    if n <= EXACT_COVER_MAX {
        for k in 1..=n.min(6) {
            let mut best: Option<BestCover> = None;
            let mut comb: Vec<usize> = (0..k).collect();
            loop {
                let u: BTreeSet<usize> = comb
                    .iter()
                    .flat_map(|&i| acc[i].credited.iter().copied())
                    .collect();
                if u == coverable {
                    let vol: u64 = comb
                        .iter()
                        .map(|&i| acc[i].rule.rhs_terms)
                        .fold(0u64, |a, b| a.saturating_add(b));
                    let key: Vec<(usize, usize)> = comb.iter().map(|&i| tie(&acc[i])).collect();
                    let better = match &best {
                        None => true,
                        Some((bv, bk, _)) => vol < *bv || (vol == *bv && key < *bk),
                    };
                    if better {
                        best = Some((vol, key, comb.clone()));
                    }
                }
                // next combination
                let mut i = k;
                while i > 0 && comb[i - 1] == n - k + i - 1 {
                    i -= 1;
                }
                if i == 0 {
                    break;
                }
                comb[i - 1] += 1;
                for j in i..k {
                    comb[j] = comb[j - 1] + 1;
                }
            }
            if let Some((_, _, c)) = best {
                return (c, false);
            }
        }
    }
    // greedy: most new points, then smaller RHS, then order
    let mut left = coverable;
    let mut chosen = Vec::new();
    while !left.is_empty() {
        let Some(pick) = (0..n).filter(|i| !chosen.contains(i)).max_by(|&a, &b| {
            let ga = acc[a].credited.intersection(&left).count();
            let gb = acc[b].credited.intersection(&left).count();
            ga.cmp(&gb)
                .then(acc[b].rule.rhs_terms.cmp(&acc[a].rule.rhs_terms))
                .then(tie(&acc[b]).cmp(&tie(&acc[a])))
        }) else {
            break;
        };
        if acc[pick].credited.intersection(&left).count() == 0 {
            break;
        }
        for p in &acc[pick].credited {
            left.remove(p);
        }
        chosen.push(pick);
    }
    (chosen, true)
}

fn member_of(state: &ItemState, a: &Accepted, served: Vec<Key>) -> CoverMember {
    let gp = &state.item.gap_points;
    CoverMember {
        attempt_key: a.key.clone(),
        index: a.index,
        setting: a.setting.clone(),
        digest: a.rule.digest.clone(),
        rhs_terms: a.rule.rhs_terms,
        case: a.case.clone(),
        served,
        credited: a.credited.iter().map(|&p| gp[p].clone()).collect(),
        exclusions: a
            .uncredited
            .iter()
            .map(|(&p, r)| Exclusion {
                target: ExclusionTarget::Key(gp[p].clone()),
                reason: r.clone(),
            })
            .collect(),
    }
}

pub(crate) fn make_cover(
    state: &ItemState,
    acc: &[Accepted],
    members: &[usize],
    greedy: bool,
) -> Cover {
    let gp = &state.item.gap_points;
    let mut taken = BTreeSet::new();
    let mut out = Vec::new();
    let mut ms: Vec<usize> = members.to_vec();
    ms.sort_by_key(|&i| tie(&acc[i]));
    for i in ms {
        let a = &acc[i];
        let served: Vec<Key> = a
            .credited
            .iter()
            .filter(|p| taken.insert(**p))
            .map(|&p| gp[p].clone())
            .collect();
        out.push(member_of(state, a, served));
    }
    Cover {
        members: out,
        greedy,
    }
}

/// Request: does candidate (attempt key, index) have a domain CONTAINING the old rules' domains (optimise items,
/// U-13; on FULL nodes a proof by the exact partition, R33)?
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainmentRequest {
    pub attempt_key: String,
    pub index: usize,
    pub old: Vec<String>,
}

/// THE STATE MACHINE (repair-library.md section 4). Pure: same state and params -> same result.
pub fn next_steps(state: &ItemState, params: &NodeParams) -> Next {
    let item = &state.item;
    let b = &params.budgets;
    let gp = &item.gap_points;
    let n_points = gp.len();
    if state.steps >= b.max_item_steps {
        return Next::Done(Disposition::attention(AttentionReason::StepBound {
            steps: state.steps,
            bound: b.max_item_steps,
        }));
    }
    if n_points == 0 && item.kind != ItemKind::Optimise {
        return Next::Done(Disposition::Covered {
            cover: Cover::default(),
        });
    }
    if item.case.dim() > item.root_dim {
        return Next::Done(Disposition::attention(AttentionReason::DimensionAboveRoot));
    }
    if !state.offers_recorded {
        return Next::NeedOffers;
    }
    for (k, r) in &state.records {
        if !r.is_verdict() && r.operator_interrupts > b.max_operator_interrupts {
            return Next::Done(Disposition::attention(
                AttentionReason::OperatorInterrupts {
                    attempt: k.clone(),
                    count: r.operator_interrupts,
                    bound: b.max_operator_interrupts,
                },
            ));
        }
    }
    let point = item.kind == ItemKind::Point || item.case.is_point();
    let optimise = item.kind == ItemKind::Optimise;
    let waves = seed_space(&item.case, params);
    // wave 0: offered and inherited candidates (order 0), judged and selected like searched ones
    let prefix = format!("{}|offer|", item.id.0);
    let mut done_settings: Vec<(usize, SeedSetting, String)> = state
        .records
        .iter()
        .filter(|(k, r)| k.starts_with(&prefix) && r.is_verdict())
        .map(|(k, r)| (0usize, r.setting.clone(), k.clone()))
        .collect();
    let mut order = 0usize;
    for (w, wave) in waves.iter().enumerate() {
        let entries: Vec<(usize, SeedSetting, String)> = wave
            .iter()
            .map(|s| {
                order += 1;
                (order, s.clone(), attempt_key(&item.id, s))
            })
            .collect();
        let settled = |k: &String| state.records.get(k).is_some_and(|r| r.settled(b));
        let finished: Vec<(usize, SeedSetting, String)> =
            entries.iter().filter(|e| settled(&e.2)).cloned().collect();
        let pending: Vec<&(usize, SeedSetting, String)> =
            entries.iter().filter(|e| !settled(&e.2)).collect();
        let mut all_done = done_settings.clone();
        all_done.extend(finished.iter().cloned());
        let mut acc = match accepted_candidates(state, params, &all_done) {
            Ok(a) => a,
            Err(need) => return Next::NeedVerdicts(need),
        };
        if optimise && let Err(req) = optimise_filter(state, &mut acc) {
            return Next::NeedContainment(req);
        }
        let one = match one_case_with_evidence(&acc, gp, params) {
            Ok(o) => o,
            Err(req) => return Next::NeedEvidence(req),
        };
        // WAVE SEMANTICS (audit B M3, addendum 4): a wave is the set of its settings fixed when it is issued; it is
        // ALWAYS run to the end, so the choice is a function of the SET of verdict results of the wave and a
        // sequential driver and a parallel batch driver reach the same disposition. The next wave runs only if this
        // one gave no one-case cover. Only a wave-0 candidate (offered / inherited, no solver CPU) that covers every
        // gap point before the first wave starts ends the search.
        let offer = match one {
            None => !pending.is_empty(),
            Some(bi) => !pending.is_empty() && !(acc[bi].order == 0 && w == 0 && finished.is_empty()),
        };
        if offer {
            let heavy = state
                .records
                .values()
                .any(|r| r.non_verdict.iter().any(|t| t == "refused_heavy"));
            let attempts: Vec<Attempt> = pending
                .iter()
                .map(|(_, s, k)| {
                    let r = state.records.get(k);
                    // a retry after a non-verdict outcome runs in the larger (oversized) resource class (C3)
                    let retry = r.is_some_and(|r| !r.non_verdict.is_empty());
                    Attempt {
                        item: item.id.clone(),
                        key: k.clone(),
                        case: item.case.clone(),
                        setting: s.clone(),
                        rung: if point { Rung::Point } else { Rung::SeedSearch },
                        wave: w as u8,
                        budget: attempt_budget(s, &item.case, params, point, heavy || retry),
                        seed_args: engine_seed_args(s),
                        attempt_no: 1 + r.map_or(0, |r| {
                            r.operator_interrupts
                                .saturating_add(r.infra_interrupts)
                                .saturating_add(r.non_verdict.len() as u32)
                        }),
                    }
                })
                .collect();
            return Next::Run(attempts);
        }
        if let Some(bi) = one {
            let cover = make_cover(state, &acc, &[bi], false);
            return Next::Done(Disposition::Covered { cover });
        }
        done_settings.extend(finished);
    }
    if optimise {
        return Next::Done(Disposition::OptimiseKept {
            why: "no acceptable replacement with a containing domain and a strictly better economy measure".into(),
        });
    }
    // every wave answered without a one-case cover: minimum set cover, then children / terminal proposal
    let acc = match accepted_candidates(state, params, &done_settings) {
        Ok(a) => a,
        Err(need) => return Next::NeedVerdicts(need),
    };
    let (members, greedy) = min_cover(&acc);
    let mut req = Vec::new();
    for &m in &members {
        let a = &acc[m];
        if !a.lacking.is_empty() {
            req.push(EvidenceRequest {
                attempt_key: a.key.clone(),
                index: a.index,
                keys: a.lacking.iter().map(|&i| gp[i].clone()).collect(),
                level: params.min_evidence,
            });
        }
    }
    if !req.is_empty() {
        return Next::NeedEvidence(req);
    }
    // ECONOMY PRUNING (decision D-L4, revised after audit A M1; repair-library.md 4.2): a member whose own points
    // all lie on exception branches READ FROM an earlier member's rule, where that branch also holds a point the
    // full cover leaves, adds a case the branch child makes anyway: it goes to the RESERVE and is handed to the
    // children whose gap points it credits as an INHERITED candidate, where it competes in the child's own
    // selection BEFORE the child splits further (so it can never lose to point rules).
    let mut members = members;
    let mut reserve_idx: Vec<usize> = Vec::new();
    loop {
        let full = make_cover(state, &acc, &members, greedy);
        let served_full: BTreeSet<Key> = full
            .members
            .iter()
            .flat_map(|m| m.served.iter().cloned())
            .collect();
        let left_full: Vec<Key> = gp
            .iter()
            .filter(|k| !served_full.contains(*k))
            .cloned()
            .collect();
        if left_full.is_empty() || members.len() < 2 {
            break;
        }
        let mut dropped = None;
        for (pos, m) in full.members.iter().enumerate().rev() {
            if pos == 0 || m.served.is_empty() {
                continue;
            }
            let earlier: Vec<&CandidateRule> = full.members[..pos]
                .iter()
                .filter_map(|e| {
                    acc.iter()
                        .find(|a| a.key == e.attempt_key && a.index == e.index)
                        .map(|a| &a.rule)
                })
                .collect();
            let all_on_live_branch = m.served.iter().all(|p| {
                earlier.iter().any(|r| match excepting_branch(r, p) {
                    Some((name, _)) => left_full
                        .iter()
                        .any(|q| excepting_branch(r, q).is_some_and(|(n2, _)| n2 == name)),
                    None => false,
                })
            });
            if all_on_live_branch {
                dropped = acc
                    .iter()
                    .position(|a| a.key == m.attempt_key && a.index == m.index);
                break;
            }
        }
        match dropped {
            Some(i) => {
                members.retain(|&x| x != i);
                reserve_idx.push(i);
            }
            None => break,
        }
    }
    let cover = make_cover(state, &acc, &members, greedy);
    let reserve: Vec<CoverMember> = reserve_idx
        .iter()
        .map(|&i| member_of(state, &acc[i], vec![]))
        .collect();
    let served: BTreeSet<Key> = cover
        .members
        .iter()
        .flat_map(|m| m.served.iter().cloned())
        .collect();
    let leftover: Vec<Key> = gp
        .iter()
        .filter(|k| !served.contains(*k))
        .cloned()
        .collect();
    if leftover.is_empty() {
        return Next::Done(Disposition::Covered { cover });
    }
    if point && leftover.len() > 1 {
        // a point case holds ONE key; several left-over keys mean a malformed case (audit B m1): never one token
        // for several keys
        return Next::Done(Disposition::attention(
            AttentionReason::MalformedPointCase { keys: leftover },
        ));
    }
    if point {
        // C3: a terminal proposal only when EVERY setting of the space has a verdict outcome
        let no_verdict: Vec<String> = waves
            .iter()
            .flatten()
            .filter(|s| {
                !state
                    .records
                    .get(&attempt_key(&item.id, s))
                    .is_some_and(|r| r.is_verdict())
            })
            .map(|s| s.label())
            .collect();
        if waves.iter().all(|w| w.is_empty()) {
            return Next::Done(Disposition::attention(AttentionReason::EmptySeedSpace));
        }
        if !no_verdict.is_empty() {
            return Next::Done(Disposition::attention(AttentionReason::NonVerdict {
                settings: no_verdict,
            }));
        }
        return match LadderExhausted::mint(state, params, &leftover[0]) {
            Some(exhausted) => Next::Done(Disposition::TerminalCandidates {
                keys: leftover,
                exhausted,
            }),
            None => Next::Done(Disposition::attention(AttentionReason::NonVerdict {
                settings: vec![],
            })),
        };
    }
    let cover_rules: Vec<&CandidateRule> = members.iter().map(|&m| &acc[m].rule).collect();
    // every other judged candidate informs the children, also one that credits no gap point (all of them on its
    // exception branches: plan 2.4.4 "one L2 item per branch whose zero set meets an uncovered target point");
    // candidates with a hidden pole are not trusted for their exception set
    let mut informants: Vec<&CandidateRule> = Vec::new();
    for (_, _, key) in &done_settings {
        let Some(rec) = state.records.get(key) else {
            continue;
        };
        for (idx, c) in rec.candidates().iter().enumerate() {
            let hidden = state
                .verdicts
                .get(&(key.clone(), idx))
                .is_none_or(|v| v.iter().any(|x| matches!(x, PointVerdict::HiddenPole)));
            if !hidden && !cover_rules.iter().any(|r| r.digest == c.digest) {
                informants.push(c);
            }
        }
    }
    let reserve_rules: Vec<&CandidateRule> = reserve_idx.iter().map(|&i| &acc[i].rule).collect();
    let (mut children, alternatives) = derive_children(
        item,
        &cover_rules,
        &reserve_rules,
        &informants,
        &leftover,
        params,
    );
    let placed: BTreeSet<&Key> = children.iter().flat_map(|c| c.gap_points.iter()).collect();
    let unplaced: Vec<Key> = leftover
        .iter()
        .filter(|k| !placed.contains(k))
        .cloned()
        .collect();
    if !unplaced.is_empty() {
        return Next::Done(Disposition::attention(AttentionReason::UnplacedPoints {
            keys: unplaced,
        }));
    }
    // hand the reserve to the children whose gap points it credits
    for c in &mut children {
        for &i in &reserve_idx {
            let a = &acc[i];
            if a.credited.iter().any(|&p| c.gap_points.contains(&gp[p])) {
                c.inherited.push(InheritedCandidate {
                    rule: a.rule.clone(),
                    case: a.case.clone(),
                    setting: a.setting.clone(),
                    from: format!("{}#{}", a.key, a.index),
                });
            }
        }
    }
    Next::Done(Disposition::Split {
        cover,
        children,
        alternatives,
        reserve,
    })
}

/// The best one-case candidate whose credited keys all carry evidence at the required level; `Err(request)` when the
/// best one still lacks evidence (a failure there removes keys, and the next candidate becomes the best).
fn one_case_with_evidence(
    acc: &[Accepted],
    gp: &[Key],
    params: &NodeParams,
) -> Result<Option<usize>, Vec<EvidenceRequest>> {
    let Some(bi) = best_one_case(acc, gp.len()) else {
        return Ok(None);
    };
    let a = &acc[bi];
    if a.lacking.is_empty() {
        return Ok(Some(bi));
    }
    Err(vec![EvidenceRequest {
        attempt_key: a.key.clone(),
        index: a.index,
        keys: a.lacking.iter().map(|&i| gp[i].clone()).collect(),
        level: params.min_evidence,
    }])
}

/// Optimise items (U-13): keep only candidates whose domain CONTAINS the old rules' (driver verdict, R33 proof on
/// FULL nodes) and whose measure is STRICTLY better than the old rules' together; ask for missing containment.
fn optimise_filter(
    state: &ItemState,
    acc: &mut Vec<Accepted>,
) -> Result<(), Vec<ContainmentRequest>> {
    let old = &state.item.replaces;
    let old_rhs: u64 = old.iter().map(|o| o.rhs_terms).sum();
    let old_score = SelectionScore {
        failed: 0,
        cases: old.len() as u64,
        size_class: SelectionScore::size_class_of(old_rhs),
        preferred_misses: u64::MAX,
        rhs_terms: old_rhs,
        monomials: old
            .iter()
            .map(|o| o.monomials.unwrap_or(u64::MAX / 4))
            .fold(0u64, |a, b| a.saturating_add(b)),
        degree: old
            .iter()
            .map(|o| o.max_degree.unwrap_or(u32::MAX))
            .max()
            .unwrap_or(u32::MAX),
        order: 0,
    };
    let n = state.item.gap_points.len();
    let mut need = Vec::new();
    acc.retain(|a| {
        let s = score(a, n);
        let better = (s.failed, s.cases, s.rhs_terms, s.monomials, s.degree)
            < (
                old_score.failed,
                old_score.cases,
                old_score.rhs_terms,
                old_score.monomials,
                old_score.degree,
            );
        if !better || a.credited.len() != n {
            return false;
        }
        match state.containment.get(&(a.key.clone(), a.index)) {
            Some(Tri::Pass) => true,
            Some(_) => false,
            None => {
                need.push(ContainmentRequest {
                    attempt_key: a.key.clone(),
                    index: a.index,
                    old: old.iter().map(|o| o.digest.clone()).collect(),
                });
                false
            }
        }
    });
    if need.is_empty() { Ok(()) } else { Err(need) }
}
