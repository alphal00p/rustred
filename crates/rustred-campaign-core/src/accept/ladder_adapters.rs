//! The ladder's traits (`repair::Coverage`, `repair::FactSource`, `repair::KeyProver`; interfaces/repair-library.md
//! s.3) implemented on the real modules, so that the ladder (batch or live repair) judges with the same code as the
//! acceptance chain.

use std::collections::{BTreeSet, HashMap};

use super::descent::Instantiator;
use super::facts::RegVerdict;
use super::prover::{Evidence, EvidenceProver, KeyFailure, KeyStatus, ProverAnswer};
use super::rule::{RuleText, WitnessStatus};
use super::{KeyFacts, RuleSource};
use crate::closure::bodies::{Sectors, TermClass};
use crate::closure::facts::CoefFact;
use crate::cover::{Dispatch, Dispatcher};
use crate::key::Key;
use crate::repair::{Coverage, CoverageVerdict, FactSource, KeyProver};
use crate::worklist::items::EvidenceLevel;
use crate::worklist::ladder::{CandidateRule, KeyEvidence, PointFacts, ProofFailure, Tri};

/// A rule named by table id, record position and label (every credit names its rule, R8).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleName {
    pub table: String,
    pub record: usize,
    pub label: i64,
}

impl std::fmt::Display for RuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}@{}", self.table, self.label, self.record)
    }
}

/// TYPED coverage of the tables in force at a key (only `Covered` is coverage; `Blocked` is never credited).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyCoverage {
    /// A declared terminal: never dispatched.
    Terminal,
    Covered { rule: RuleName },
    Uncovered,
    Blocked { rule: RuleName, verdict: String },
    Undecided { rule: RuleName, why: String },
    OutsideChart { slot: usize },
    Error(String),
}

/// The tables in force (first match) plus the declared terminals.
pub struct TablesInForce<'a> {
    pub disp: &'a Dispatcher,
    pub terminals: BTreeSet<Key>,
}

impl TablesInForce<'_> {
    fn name(&self, r: crate::cover::RuleRef) -> RuleName {
        RuleName {
            table: self.disp.tables()[r.table].name.clone(),
            record: r.record,
            label: r.label,
        }
    }
    pub fn coverage_at(&self, key: &[i32]) -> KeyCoverage {
        if self.terminals.contains(key) {
            return KeyCoverage::Terminal;
        }
        match self.disp.first_match(key) {
            Ok(Dispatch::Covered(r)) => KeyCoverage::Covered { rule: self.name(r) },
            Ok(Dispatch::Uncovered) => KeyCoverage::Uncovered,
            Ok(Dispatch::Blocked { at, reason }) => KeyCoverage::Blocked {
                rule: self.name(at),
                verdict: format!("{reason:?}"),
            },
            Ok(Dispatch::Undecided { at, verdict }) => KeyCoverage::Undecided {
                rule: self.name(at),
                why: format!("{verdict:?}"),
            },
            Ok(Dispatch::OutsideChart { slot }) => KeyCoverage::OutsideChart { slot },
            Err(e) => KeyCoverage::Error(e.to_string()),
        }
    }
}

impl Coverage for TablesInForce<'_> {
    fn verdict(&self, key: &[i32]) -> CoverageVerdict {
        match self.coverage_at(key) {
            KeyCoverage::Covered { rule } => CoverageVerdict::Covered {
                rule: rule.to_string(),
            },
            KeyCoverage::Uncovered => CoverageVerdict::Uncovered,
            other => CoverageVerdict::Undecided {
                why: format!("{other:?}"),
            },
        }
    }
}

/// Per-key facts for the ladder from the candidate's record (the ladder's `CandidateRule.digest` must be the record
/// line's `sha256:` digest, as [`RuleText::line_sha256`]).
pub struct RecordFacts<F: KeyFacts> {
    pub store: HashMap<String, RuleText>,
    pub facts: F,
    pub sectors: Sectors,
}

impl<F: KeyFacts> RecordFacts<F> {
    pub fn new(facts: F, sectors: Sectors) -> Self {
        RecordFacts {
            store: HashMap::new(),
            facts,
            sectors,
        }
    }
    pub fn insert(&mut self, rule: RuleText) {
        self.store.insert(rule.line_sha256.clone(), rule);
    }
}

impl<F: KeyFacts> FactSource for RecordFacts<F> {
    fn facts(&mut self, rule: &CandidateRule, key: &[i32]) -> PointFacts {
        let unknown = |note: &str| PointFacts {
            regular: Tri::Unknown,
            rhs_top: None,
            exceptions_complete: Tri::Unknown,
            note: Some(note.to_string()),
        };
        let Some(rt) = self.store.get(&rule.digest) else {
            return unknown("rule record not in the store");
        };
        let regular = match self.facts.regular(rt, key) {
            RegVerdict::Regular => Tri::Pass,
            RegVerdict::Pole { .. } => Tri::Fail,
            RegVerdict::Hidden { .. } | RegVerdict::Unknown(_) => Tri::Unknown,
        };
        let exceptions_complete = match (rt.witness_status(), rt.guard_version.unwrap_or(0) >= 2) {
            (WitnessStatus::Present { .. }, true) => Tri::Pass,
            (WitnessStatus::Elided, _) => Tri::Fail,
            _ => Tri::Unknown,
        };
        let rhs_top = Instantiator::new(rt).ok().and_then(|inst| {
            let terms = inst.terms(key).ok()?;
            let mut top = Vec::new();
            for (j, (t, cm)) in terms.into_iter().enumerate() {
                match self.sectors.classify_masked(&t, cm) {
                    TermClass::Top => {
                        if self.facts.coefficient(rt, key, j) != CoefFact::Zero {
                            top.push(t);
                        }
                    }
                    TermClass::PositiveNumerator { .. } => {
                        if self.facts.coefficient(rt, key, j) != CoefFact::Zero {
                            return None; // a positive numerator with a nonzero / unknown coefficient: unknown
                        }
                    }
                    TermClass::Subsector { .. } | TermClass::ZeroSector { .. } => {}
                }
            }
            Some(top)
        });
        PointFacts {
            regular,
            rhs_top,
            exceptions_complete,
            note: None,
        }
    }
}

/// The ladder's key prover over an [`EvidenceProver`] and a rule store (digest -> record, source).
pub struct ChainProver<P: EvidenceProver> {
    pub prover: P,
    pub store: HashMap<String, (RuleText, RuleSource)>,
}

impl<P: EvidenceProver> ChainProver<P> {
    pub fn new(prover: P) -> Self {
        ChainProver {
            prover,
            store: HashMap::new(),
        }
    }
    pub fn insert(&mut self, rule: RuleText, source: RuleSource) {
        self.store.insert(rule.line_sha256.clone(), (rule, source));
    }
}

fn level_of(e: Evidence) -> EvidenceLevel {
    match e {
        Evidence::Geometry => EvidenceLevel::A1,
        Evidence::Imported => EvidenceLevel::A0,
        Evidence::Replay => EvidenceLevel::A2,
        Evidence::PerKey => EvidenceLevel::A3,
    }
}

impl<P: EvidenceProver> KeyProver for ChainProver<P> {
    fn prove(&mut self, rule: &CandidateRule, keys: &[Key], wanted: EvidenceLevel) -> Vec<KeyEvidence> {
        let all = |f: ProofFailure| {
            keys.iter()
                .map(|k| KeyEvidence {
                    key: k.clone(),
                    level: None,
                    failure: Some(f.clone()),
                })
                .collect()
        };
        if level_of(self.prover.level()) < wanted {
            return all(ProofFailure::Unsupported(format!(
                "prover level {:?} below {wanted:?}",
                self.prover.level()
            )));
        }
        let Some((rt, src)) = self.store.get(&rule.digest) else {
            return all(ProofFailure::Unknown("rule record not in the store".into()));
        };
        match self.prover.prove(rt, src, keys) {
            ProverAnswer::Unavailable(w) => all(ProofFailure::Unsupported(w)),
            ProverAnswer::NeedsExternal(_) => {
                all(ProofFailure::Unknown("needs an external run (file-backed prover)".into()))
            }
            ProverAnswer::PerKey { keys: st, .. } => keys
                .iter()
                .map(|k| {
                    let s = st.iter().find(|(kk, _)| kk == k).map(|(_, s)| s.clone());
                    let (level, failure) = match s {
                        Some(KeyStatus::Reached(e)) => (Some(level_of(e)), None),
                        Some(KeyStatus::Failed(KeyFailure::NonMembership { .. })) => {
                            (None, Some(ProofFailure::NonMembership))
                        }
                        Some(KeyStatus::Failed(KeyFailure::Irregular { .. })) => (None, Some(ProofFailure::Irregular)),
                        Some(KeyStatus::Failed(KeyFailure::Hidden { .. })) => (None, Some(ProofFailure::Hidden)),
                        Some(KeyStatus::Unsupported(w)) => (None, Some(ProofFailure::Unsupported(w))),
                        Some(KeyStatus::NotCredited(w)) | Some(KeyStatus::Unknown(w)) => {
                            (None, Some(ProofFailure::Unknown(w)))
                        }
                        None => (None, Some(ProofFailure::Unknown("not answered".into()))),
                    };
                    KeyEvidence {
                        key: k.clone(),
                        level,
                        failure,
                    }
                })
                .collect(),
        }
    }
}
