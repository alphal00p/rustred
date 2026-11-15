//! Structural ZERO PROOFS of rule terms as a FACT SOURCE (lane zeros2; interfaces/zero-proofs.md).
//!
//! A proof line says: the NUMERATOR text N_j of term j of record r (sha256 of the raw record line) vanishes identically,
//! as a polynomial in the remaining indices and d, on a LOCUS of the record's rule points: its whole domain, or one
//! hyperplane `n_p = v` of the rule point (intersected with the domain; case equations may have been used). So at every
//! key whose rule point lies on the locus the coefficient c_j = N_j / D_j is identically zero in d wherever D_j does not
//! vanish identically there; `nowhere_zero` = the producer also proved that D_j never vanishes identically on the
//! locus, `where_regular` = not proved: such a line CARRIES the denominator text (`den`) and counts at a key ONLY where
//! that denominator does not vanish identically in d at the key's rule point (evaluated here, exactly, with the core's
//! `domain::PoleLocus`); at a 0/0 key it is not a proof (ONE semantics for every consumer). The proofs are made by the Symbolica-backed producer (`rr_coef_facts`, crate
//! rustred-campaign-algebra); this module only LOADS (verified: binding to the set's records, term index, parameter,
//! index echo, closed vocabulary; any violation refuses the file) and LOOKS UP (`is_zero_term`: rule point + hash lookup).
//!
//! lane zerosfix3 (audit-r2-zeros F1-F3, M5): the loader also checks every proved CLAIM against the RECORD'S OWN TEXT
//! (read at its offset, sha256-checked, streamed term by term; independent of the producer): the numerator of the term
//! must vanish identically after the record's constant slots and the plane are substituted (the core's exact
//! polynomial; a claim that does not is never a proof, counted `claims_false` and named in `false_claims`
//! (`NumeratorDoesNotVanish`); every claim is checked on its own); a `where_regular` line counts only
//! when its `den` equals the record's own denominator text, which is then the one evaluated at each key (any other
//! `den`: never a proof); a claim the core cannot re-check (case equations used, a text form the core parser refuses,
//! a term above the record budget) is never a proof. Two DIFFERENT lines for one (record, term, locus) refuse the file
//! (`ContradictingLines`). Refusals are typed ([`ZeroProofRefusal`]).
//!
//! Consumers: the closing loop's exact tier (the second route of every zero term), the ship gate, the repair flow's
//! shipped specs; [`ZeroProofFacts`] wraps any [`FactSource`] so that a proof turns an `Unknown` coefficient into
//! `Zero`. Generic (L8): records, terms, parameters and loci are data.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::load::ImportedSet;
use crate::closure::bodies::rule_point;
use crate::closure::facts::{CoefFact, FactSource, RegFact, RuleId};
use crate::domain::{PoleKind, PoleLocus};
use crate::poly::Poly;

/// Schema of a proof line.
pub const ZERO_PROOF_SCHEMA: &str = "rustred.campaign.zero-proof.v1";
/// Verdict word of a proved line.
pub const VERDICT_PROVED: &str = "zero_on_locus";

/// The locus a proof covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Locus {
    /// Every rule point of the record (its constants substituted).
    Domain,
    /// The hyperplane `n<param> = value` of the rule point.
    Plane { param: u32, value: i64 },
}

impl Locus {
    /// Whether the rule point `pt` lies on the locus.
    pub fn contains(&self, pt: &[i32]) -> bool {
        match *self {
            Locus::Domain => true,
            Locus::Plane { param, value } => {
                pt.get(param as usize).is_some_and(|&x| x as i64 == value)
            }
        }
    }

    /// The JSON form (`"domain"` | `{"plane": [p, v]}`).
    pub fn json(&self) -> Value {
        match *self {
            Locus::Domain => Value::from("domain"),
            Locus::Plane { param, value } => serde_json::json!({"plane": [param, value]}),
        }
    }

    /// Read the JSON form.
    pub fn from_json(v: &Value) -> Result<Locus, String> {
        if v.as_str() == Some("domain") {
            return Ok(Locus::Domain);
        }
        let a = v
            .get("plane")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("locus {v}: neither \"domain\" nor {{\"plane\": [p, v]}}"))?;
        match (
            a.first().and_then(Value::as_u64),
            a.get(1).and_then(Value::as_i64),
        ) {
            (Some(p), Some(x)) if a.len() == 2 && p <= u32::MAX as u64 => Ok(Locus::Plane {
                param: p as u32,
                value: x,
            }),
            _ => Err(format!("locus {v}: plane [param, value] expected")),
        }
    }
}

/// What a proof says about the denominator on its locus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DenStatus {
    /// D_j never vanishes identically in d on the locus: the coefficient is zero at every key of the locus.
    NowhereZero,
    /// Not proved: the coefficient is zero at a key of the locus where D_j does not vanish there (the record regular).
    WhereRegular,
}

impl DenStatus {
    pub fn word(self) -> &'static str {
        match self {
            DenStatus::NowhereZero => "nowhere_zero",
            DenStatus::WhereRegular => "where_regular",
        }
    }
    pub fn from_word(w: &str) -> Option<DenStatus> {
        match w {
            "nowhere_zero" => Some(DenStatus::NowhereZero),
            "where_regular" => Some(DenStatus::WhereRegular),
            _ => None,
        }
    }
}

/// The answer of a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZeroStatus {
    /// A proof covers the term at the key: its numerator vanishes on the locus and the denominator is non-zero at the
    /// key (proved nowhere zero on the locus, or evaluated non-zero at this key).
    Proved,
    /// A proof covers the numerator but the denominator vanishes identically in d at this key (0/0): NOT a proof.
    DenominatorVanishes,
    NotProved,
}

/// How a proved line's denominator is checked at a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DenCheck {
    NowhereZero,
    /// lane zerosfix3 (audit M3): `nowhere_zero` by the producer's argument (Symbolica: factors, chart signs) that the
    /// core's constant-d-coefficient test does not confirm: the record's own denominator is ALSO evaluated by the core
    /// at every key the proof is used at (independent of the producer; a 0/0 key is never a proof).
    NowhereZeroAtKey(PoleLocus),
    /// `where_regular`: evaluated at the key (the line's `den` text).
    AtKey(PoleLocus),
    /// `where_regular` without a readable `den` text: never a proof at a key.
    Unchecked,
}

/// One parsed proof line (proved, not proved or excluded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofLine {
    pub record_sha256: String,
    pub rule: Option<i64>,
    pub term: u32,
    pub locus: Locus,
    pub verdict: String,
    pub denominator: Option<DenStatus>,
    /// The term's denominator text (`where_regular` lines; None: no denominator / not given).
    pub den: Option<String>,
    pub equations: Vec<u32>,
    /// `program@version` (the line's `tool` word, or `tool.program@tool.version`).
    pub tool: String,
}

/// Parse one proof line (schema and vocabulary only; the binding to a set is checked by [`ZeroProofs::insert`]).
pub fn parse_line(v: &Value) -> Result<ProofLine, String> {
    if v["schema"] != ZERO_PROOF_SCHEMA {
        return Err(format!(
            "schema {} (expected {ZERO_PROOF_SCHEMA})",
            v["schema"]
        ));
    }
    let sha = v["record_sha256"]
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("record_sha256: 64 hex digits expected")?;
    let term = v["term"]
        .as_u64()
        .filter(|&t| t <= u32::MAX as u64)
        .ok_or("term: an index expected")? as u32;
    let locus = Locus::from_json(&v["locus"])?;
    let verdict = v["verdict"].as_str().ok_or("verdict missing")?.to_string();
    let denominator = match verdict.as_str() {
        VERDICT_PROVED => Some(
            v["denominator"]
                .as_str()
                .and_then(DenStatus::from_word)
                .ok_or("a proved line needs denominator nowhere_zero | where_regular")?,
        ),
        "not_proved" | "excluded" => None,
        w => return Err(format!("verdict {w:?} unknown")),
    };
    let equations = match v.get("equations") {
        None | Some(Value::Null) => vec![],
        Some(e) => e
            .as_array()
            .ok_or("equations: an array expected")?
            .iter()
            .map(|x| {
                x.as_u64()
                    .filter(|&k| k <= u32::MAX as u64)
                    .map(|k| k as u32)
            })
            .collect::<Option<Vec<u32>>>()
            .ok_or("equations: indices expected")?,
    };
    let tool = match v["tool"].as_str() {
        Some(w) => w.to_string(),
        None => format!(
            "{}@{}",
            v["tool"]["program"].as_str().unwrap_or("?"),
            v["tool"]["version"].as_str().unwrap_or("?")
        ),
    };
    Ok(ProofLine {
        record_sha256: sha.to_string(),
        rule: v["rule"].as_i64(),
        term,
        locus,
        verdict,
        denominator,
        den: v["den"].as_str().map(str::to_string),
        equations,
        tool,
    })
}

/// Load options.
#[derive(Clone, Debug, Default)]
pub struct LoadOptions {
    /// Some: only proofs of these producers (`program@version`) are accepted; a line of another refuses the file.
    pub tools: Option<BTreeSet<String>>,
}

/// lane zerosfix3 (audit-r2-zeros M5): why the loader refuses a proof file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ZeroProofRefusal {
    Io(String),
    Json(String),
    /// Schema / vocabulary of the line ([`parse_line`]).
    Line(String),
    ProducerNotAllowed(String),
    TermOutOfRange {
        record: String,
        term: u32,
        n_terms: usize,
    },
    ParameterOutOfRange {
        record: String,
        param: u32,
        arity: usize,
    },
    RuleEcho {
        record: String,
        echo: Option<i64>,
        index: Option<i64>,
    },
    /// Two different lines for one (record, term, locus) (audit F3).
    ContradictingLines {
        record: String,
        term: u32,
        locus: Locus,
    },
    /// A proved claim whose numerator does NOT vanish on its locus by the record's own text (audit F1).
    NumeratorDoesNotVanish {
        record: String,
        term: u32,
        locus: Locus,
    },
    /// The record's text could not be read for the claim check (not the record the set holds, I/O).
    RecordUnreadable {
        record: String,
        msg: String,
    },
}

impl std::fmt::Display for ZeroProofRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZeroProofRefusal::Io(m) | ZeroProofRefusal::Json(m) | ZeroProofRefusal::Line(m) => {
                write!(f, "{m}")
            }
            ZeroProofRefusal::ProducerNotAllowed(t) => write!(f, "producer {t} is not allowed"),
            ZeroProofRefusal::TermOutOfRange {
                record,
                term,
                n_terms,
            } => write!(
                f,
                "record {record}: term {term} (the record has {n_terms} terms)"
            ),
            ZeroProofRefusal::ParameterOutOfRange {
                record,
                param,
                arity,
            } => write!(
                f,
                "record {record}: plane on parameter {param} (arity {arity})"
            ),
            ZeroProofRefusal::RuleEcho {
                record,
                echo,
                index,
            } => write!(
                f,
                "record {record}: rule echo {echo:?} (the record's index is {index:?})"
            ),
            ZeroProofRefusal::ContradictingLines {
                record,
                term,
                locus,
            } => write!(
                f,
                "record {record}: contradicting lines for term {term} on {}",
                locus.json()
            ),
            ZeroProofRefusal::NumeratorDoesNotVanish {
                record,
                term,
                locus,
            } => write!(
                f,
                "record {record}: the numerator of term {term} does not vanish on {} (the record's own text)",
                locus.json()
            ),
            ZeroProofRefusal::RecordUnreadable { record, msg } => {
                write!(
                    f,
                    "record {record}: its text cannot be read for the claim check: {msg}"
                )
            }
        }
    }
}

/// lane zerosfix3 (audit-r2-zeros M5): a refusal of the loader with its file and line (0 = the file as a whole).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZeroProofError {
    pub file: String,
    pub line: usize,
    pub reason: ZeroProofRefusal,
}

impl std::fmt::Display for ZeroProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.line > 0 {
            write!(f, "{} line {}: {}", self.file, self.line, self.reason)
        } else {
            write!(f, "{}: {}", self.file, self.reason)
        }
    }
}

impl std::error::Error for ZeroProofError {}

impl From<ZeroProofError> for String {
    fn from(e: ZeroProofError) -> String {
        e.to_string()
    }
}

/// lane zerosfix3: what one line says (F3: two lines of one (record, term, locus) must say the same).
#[derive(Clone, Debug, PartialEq, Eq)]
struct LineSig {
    verdict: String,
    denominator: Option<DenStatus>,
    den: Option<String>,
    equations: Vec<u32>,
}

/// lane zerosfix3: a proved line bound to the set whose claim waits for the check against the record text.
#[derive(Clone, Debug)]
struct PendingClaim {
    t: u32,
    r: u32,
    term: u32,
    locus: Locus,
    class: DenStatus,
    den: Option<String>,
    equations: bool,
    file: String,
    line: usize,
}

/// lane zerosfix3: the polynomial `text` (n0.. and d, the core's exact parser) after the integer substitutions `subs`
/// is identically zero. Err: the core parser refuses the text.
pub fn vanishes_after(text: &str, arity: usize, subs: &[(usize, i64)]) -> Result<bool, String> {
    use crate::poly::BigInt;
    let p = Poly::parse_exact(text.trim(), arity, true).map_err(|e| format!("{e:?}"))?;
    let val: HashMap<u16, i64> = subs.iter().map(|&(q, v)| (q as u16, v)).collect();
    let mut acc: BTreeMap<Vec<(u16, u32)>, BigInt> = BTreeMap::new();
    for (m, c) in p.exact_terms() {
        let mut c = c;
        let mut rest = Vec::with_capacity(m.len());
        for (var, e) in m {
            match val.get(&var) {
                Some(&v) => {
                    let b = BigInt::from_i128(v as i128);
                    for _ in 0..e {
                        c = c.mul(&b);
                    }
                }
                None => rest.push((var, e)),
            }
        }
        let e = acc.entry(rest).or_insert_with(BigInt::zero);
        *e = e.add(&c);
    }
    Ok(acc.values().all(BigInt::is_zero))
}

/// lane zerosfix3 (audit M3): after the integer substitutions `subs` some power of d has a non-zero CONSTANT
/// coefficient in `text` (then the polynomial never vanishes identically in d on the locus). Err: the core parser
/// refuses the text.
pub fn constant_d_coefficient_after(
    text: &str,
    arity: usize,
    subs: &[(usize, i64)],
) -> Result<bool, String> {
    use crate::poly::{BigInt, D_VAR};
    let p = Poly::parse_exact(text.trim(), arity, true).map_err(|e| format!("{e:?}"))?;
    let val: HashMap<u16, i64> = subs.iter().map(|&(q, v)| (q as u16, v)).collect();
    // d power -> (constant coefficient, whether a monomial with a free index survives)
    let mut by_d: BTreeMap<u32, (BigInt, bool)> = BTreeMap::new();
    let mut acc: BTreeMap<Vec<(u16, u32)>, BigInt> = BTreeMap::new();
    for (m, c) in p.exact_terms() {
        let mut c = c;
        let mut rest = Vec::with_capacity(m.len());
        for (var, e) in m {
            match val.get(&var) {
                Some(&v) => {
                    let b = BigInt::from_i128(v as i128);
                    for _ in 0..e {
                        c = c.mul(&b);
                    }
                }
                None => rest.push((var, e)),
            }
        }
        let e = acc.entry(rest).or_insert_with(BigInt::zero);
        *e = e.add(&c);
    }
    for (mono, c) in acc {
        if c.is_zero() {
            continue;
        }
        let dpow = mono.iter().find(|(v, _)| *v == D_VAR).map_or(0, |x| x.1);
        let free = mono.iter().any(|(v, _)| *v != D_VAR);
        let e = by_d.entry(dpow).or_insert_with(|| (BigInt::zero(), false));
        if free {
            e.1 = true;
        } else {
            e.0 = e.0.add(&c);
        }
    }
    Ok(by_d.values().any(|(k, free)| !k.is_zero() && !*free))
}

/// Load counts.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct LoadStats {
    pub files: u64,
    pub lines: u64,
    pub proved: u64,
    pub proved_nowhere_zero: u64,
    /// `where_regular` lines with a readable denominator (checked at each key) / without one (never a proof).
    pub den_at_key: u64,
    pub den_unchecked: u64,
    pub not_proved: u64,
    pub excluded: u64,
    /// Lines of records that are not in the set (skipped).
    pub foreign: u64,
    /// Records of the set with at least one proof.
    pub records: u64,
    pub tools: BTreeSet<String>,
    /// lane zerosfix3: proved claims re-checked against the record text and held as proofs.
    pub claims_confirmed: u64,
    /// lane zerosfix3: proved claims the core cannot re-check (case equations, a text form the core parser refuses, a
    /// term above the record budget): never a proof.
    pub claims_unchecked: u64,
    /// lane zerosfix3: `where_regular` lines whose `den` is not the record's own denominator text (never a proof).
    pub den_text_mismatch: u64,
    /// lane zerosfix3 (audit F1): proved claims whose numerator does NOT vanish on the locus by the record's own text
    /// (never a proof; named in [`ZeroProofs::false_claims`]).
    pub claims_false: u64,
    /// lane zerosfix3 (audit M3): `nowhere_zero` claims confirmed by the core for the whole locus (no denominator, or a
    /// non-zero constant d-coefficient) / re-checked by the core at each key (record denominator evaluated) / resting
    /// on the producer alone (the core parser refuses the denominator text).
    pub den_independent: u64,
    pub den_checked_at_key: u64,
    pub den_producer_only: u64,
}

/// The proofs of one record: (term, locus, denominator check).
pub type RecordProofs = Vec<(u32, Locus, DenCheck)>;

/// The proofs of a set, by (table, record) and term (see the module documentation).
#[derive(Clone, Debug, Default)]
pub struct ZeroProofs {
    by_record: HashMap<(u32, u32), RecordProofs>,
    /// lane zerosfix3 (F3): every line held so far by (table, record, term, locus).
    lines_seen: HashMap<(u32, u32, u32, Locus), LineSig>,
    /// lane zerosfix3 (F1 / F2): proved lines bound to the set, claims not yet checked against the record text (held
    /// as proofs only by [`ZeroProofs::load_files`] after the check).
    pending: Vec<PendingClaim>,
    /// lane zerosfix3 (audit F1): the first false claims met (file, line, record, term, locus).
    pub false_claims: Vec<ZeroProofError>,
    pub stats: LoadStats,
}

impl ZeroProofs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.by_record.is_empty()
    }

    /// Proofs held (proved lines bound to the set).
    pub fn len(&self) -> usize {
        self.by_record.values().map(Vec::len).sum()
    }

    /// Bind one parsed line to the set (VERIFIED: the record's term count, the parameter range, the index echo; lane
    /// zerosfix3: no other line of the same (record, term, locus) says something else). A proved line is held as
    /// PENDING: it becomes a proof only through [`ZeroProofs::load_files`], after its claim was checked against the
    /// record text. `Ok(false)`: the record is not in the set (skipped, counted foreign).
    pub fn insert(&mut self, set: &ImportedSet, l: &ProofLine) -> Result<bool, ZeroProofRefusal> {
        self.insert_at(set, l, "", 0)
    }

    fn insert_at(
        &mut self,
        set: &ImportedSet,
        l: &ProofLine,
        file: &str,
        line: usize,
    ) -> Result<bool, ZeroProofRefusal> {
        self.stats.lines += 1;
        let Some((t, r)) = set.find_sha(&l.record_sha256) else {
            self.stats.foreign += 1;
            return Ok(false);
        };
        let n = set.bodies.n_terms(t, r);
        if !set.bodies.has(t, r) || l.term as usize >= n {
            return Err(ZeroProofRefusal::TermOutOfRange {
                record: l.record_sha256.clone(),
                term: l.term,
                n_terms: n,
            });
        }
        if let Locus::Plane { param, .. } = l.locus
            && param as usize >= set.arity()
        {
            return Err(ZeroProofRefusal::ParameterOutOfRange {
                record: l.record_sha256.clone(),
                param,
                arity: set.arity(),
            });
        }
        let info = set.rule(t, r);
        let echo_ok = match (l.rule, info.index) {
            (Some(a), Some(b)) => a == b,
            (Some(a), None) => a == info.label,
            (None, _) => false,
        };
        if !echo_ok {
            return Err(ZeroProofRefusal::RuleEcho {
                record: l.record_sha256.clone(),
                echo: l.rule,
                index: info.index,
            });
        }
        let sig = LineSig {
            verdict: l.verdict.clone(),
            denominator: l.denominator,
            den: l.den.as_deref().map(|x| x.trim().to_string()),
            equations: l.equations.clone(),
        };
        match self.lines_seen.get(&(t as u32, r as u32, l.term, l.locus)) {
            // a PROVED claim beside any other statement of the same locus refuses; two statements that both prove
            // nothing (not_proved / excluded) are harmless (the first is kept)
            Some(old)
                if *old != sig
                    && (old.verdict == VERDICT_PROVED || sig.verdict == VERDICT_PROVED) =>
            {
                return Err(ZeroProofRefusal::ContradictingLines {
                    record: l.record_sha256.clone(),
                    term: l.term,
                    locus: l.locus,
                });
            }
            // the same line again (two answer files about one record): held once
            Some(_) => {
                self.stats.tools.insert(l.tool.clone());
                return Ok(true);
            }
            None => {
                self.lines_seen
                    .insert((t as u32, r as u32, l.term, l.locus), sig);
            }
        }
        match (l.verdict.as_str(), l.denominator) {
            (VERDICT_PROVED, Some(d)) => {
                self.stats.proved += 1;
                if d == DenStatus::NowhereZero {
                    self.stats.proved_nowhere_zero += 1;
                }
                self.pending.push(PendingClaim {
                    t: t as u32,
                    r: r as u32,
                    term: l.term,
                    locus: l.locus,
                    class: d,
                    den: l.den.clone(),
                    equations: !l.equations.is_empty(),
                    file: file.to_string(),
                    line,
                });
            }
            ("not_proved", _) => self.stats.not_proved += 1,
            _ => self.stats.excluded += 1,
        }
        self.stats.tools.insert(l.tool.clone());
        Ok(true)
    }

    /// lane zerosfix3 (F1 / F2): check every pending claim against its record's own text and hold the confirmed ones
    /// as proofs (module documentation). Records are read once each (streamed, sha256-checked), up to four at a time.
    fn check_pending(&mut self, set: &ImportedSet) -> Result<(), ZeroProofError> {
        let mut pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            return Ok(());
        }
        pending.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        let mut need: BTreeMap<(u32, u32), BTreeSet<u32>> = BTreeMap::new();
        for c in &pending {
            need.entry((c.t, c.r)).or_default().insert(c.term);
        }
        let recs: Vec<((u32, u32), BTreeSet<u32>)> = need.into_iter().collect();
        type Texts = Result<Option<HashMap<u32, String>>, String>;
        let texts: std::sync::Mutex<HashMap<(u32, u32), Texts>> =
            std::sync::Mutex::new(HashMap::new());
        let next = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|sc| {
            for _ in 0..recs.len().clamp(1, 4) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(((t, r), terms)) = recs.get(i) else {
                            break;
                        };
                        let x = record_term_texts(set, *t as usize, *r as usize, terms);
                        texts
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert((*t, *r), x);
                    }
                });
            }
        });
        let texts = texts.into_inner().unwrap_or_else(|e| e.into_inner());
        let arity = set.arity();
        for c in pending {
            let sha = set.rule(c.t as usize, c.r as usize).record_sha256.clone();
            let refuse = |reason: ZeroProofRefusal| ZeroProofError {
                file: c.file.clone(),
                line: c.line,
                reason,
            };
            let text = match texts.get(&(c.t, c.r)) {
                Some(Ok(Some(m))) => m.get(&c.term).cloned(),
                // a term above the record budget: the claim cannot be re-checked
                Some(Ok(None)) => None,
                Some(Err(msg)) => {
                    return Err(refuse(ZeroProofRefusal::RecordUnreadable {
                        record: sha,
                        msg: msg.clone(),
                    }));
                }
                None => None,
            };
            let sides = text.as_deref().and_then(|t| {
                crate::accept::facts::split_ratio(t.trim())
                    .ok()
                    .map(|(n, d)| (n.to_string(), d.map(|x| x.trim().to_string())))
            });
            let (Some((num, den)), false) = (sides, c.equations) else {
                self.stats.claims_unchecked += 1;
                continue;
            };
            // the record's constant slots and the plane (a plane on a constant slot adds nothing)
            let fixed = &set.disp.tables()[c.t as usize].domains[c.r as usize].fixed;
            let mut subs: Vec<(usize, i64)> = fixed
                .iter()
                .enumerate()
                .filter_map(|(q, v)| v.map(|x| (q, x as i64)))
                .collect();
            if let Locus::Plane { param, value } = c.locus
                && !subs.iter().any(|(q, _)| *q == param as usize)
            {
                subs.push((param as usize, value));
            }
            match vanishes_after(&num, arity, &subs) {
                Ok(true) => {}
                // a FALSE claim (each claim is checked on its own: it never becomes a proof; the first few are named)
                Ok(false) => {
                    self.stats.claims_false += 1;
                    if self.false_claims.len() < 100 {
                        self.false_claims
                            .push(refuse(ZeroProofRefusal::NumeratorDoesNotVanish {
                                record: sha,
                                term: c.term,
                                locus: c.locus,
                            }));
                    }
                    continue;
                }
                Err(_) => {
                    self.stats.claims_unchecked += 1;
                    continue;
                }
            }
            let check = match c.class {
                // lane zerosfix3 (M3): independent where the core can: no denominator, or a non-zero constant
                // d-coefficient on the locus; else the core evaluates the record's denominator at each key
                DenStatus::NowhereZero => match den.as_deref() {
                    None => {
                        self.stats.den_independent += 1;
                        DenCheck::NowhereZero
                    }
                    Some(dt) => match constant_d_coefficient_after(dt, arity, &subs) {
                        Ok(true) => {
                            self.stats.den_independent += 1;
                            DenCheck::NowhereZero
                        }
                        _ => match Poly::parse_exact(dt, arity, true) {
                            Ok(p) => {
                                self.stats.den_checked_at_key += 1;
                                DenCheck::NowhereZeroAtKey(PoleLocus::new(
                                    PoleKind::Denominator,
                                    &p,
                                ))
                            }
                            // the core cannot read the denominator: the producer's claim alone
                            Err(_) => {
                                self.stats.den_producer_only += 1;
                                DenCheck::NowhereZero
                            }
                        },
                    },
                },
                DenStatus::WhereRegular => {
                    let same = den.is_some() && c.den.as_deref().map(str::trim) == den.as_deref();
                    match den
                        .as_deref()
                        .filter(|_| same)
                        .and_then(|s| Poly::parse_exact(s, arity, true).ok())
                    {
                        Some(p) => {
                            self.stats.den_at_key += 1;
                            DenCheck::AtKey(PoleLocus::new(PoleKind::Denominator, &p))
                        }
                        None => {
                            if !same {
                                self.stats.den_text_mismatch += 1;
                            }
                            self.stats.den_unchecked += 1;
                            DenCheck::Unchecked
                        }
                    }
                }
            };
            self.stats.claims_confirmed += 1;
            let v = self.by_record.entry((c.t, c.r)).or_default();
            if v.is_empty() {
                self.stats.records += 1;
            }
            if !v
                .iter()
                .any(|(j, lo, k)| *j == c.term && *lo == c.locus && *k == check)
            {
                v.push((c.term, c.locus, check));
            }
        }
        Ok(())
    }

    /// Read proof files into `self` (each line verified; any violation refuses with the file and line); lane
    /// zerosfix3: then every proved claim is checked against its record's text ([`ZeroProofRefusal`] on a false one).
    pub fn load_files(
        &mut self,
        set: &ImportedSet,
        files: &[PathBuf],
        opts: &LoadOptions,
    ) -> Result<(), ZeroProofError> {
        for p in files {
            self.load_file(set, p, opts)?;
        }
        self.check_pending(set)
    }

    fn load_file(
        &mut self,
        set: &ImportedSet,
        p: &Path,
        opts: &LoadOptions,
    ) -> Result<(), ZeroProofError> {
        let file = p.display().to_string();
        let err = |line: usize, reason: ZeroProofRefusal| ZeroProofError {
            file: file.clone(),
            line,
            reason,
        };
        let f = std::fs::File::open(p).map_err(|e| err(0, ZeroProofRefusal::Io(e.to_string())))?;
        self.stats.files += 1;
        for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
            .lines()
            .enumerate()
        {
            let l = l.map_err(|e| err(i + 1, ZeroProofRefusal::Io(e.to_string())))?;
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&l)
                .map_err(|e| err(i + 1, ZeroProofRefusal::Json(e.to_string())))?;
            if v["schema"] != ZERO_PROOF_SCHEMA {
                // other lines of a mixed answer file (per-key verdicts) are not proofs
                continue;
            }
            let pl = parse_line(&v).map_err(|e| err(i + 1, ZeroProofRefusal::Line(e)))?;
            if let Some(allow) = &opts.tools
                && !allow.contains(&pl.tool)
            {
                return Err(err(i + 1, ZeroProofRefusal::ProducerNotAllowed(pl.tool)));
            }
            self.insert_at(set, &pl, &file, i + 1)
                .map_err(|e| err(i + 1, e))?;
        }
        Ok(())
    }

    /// Load proof files for `set` (module documentation).
    pub fn load(
        set: &ImportedSet,
        files: &[PathBuf],
        opts: &LoadOptions,
    ) -> Result<Self, ZeroProofError> {
        let mut z = ZeroProofs::new();
        z.load_files(set, files, opts)?;
        Ok(z)
    }

    /// lane zerosfix3 (audit F4): the status of ONE held proof line (term, locus) of (table, record) at the RULE POINT
    /// `pt` with the ONE semantics: `Proved` only when the locus holds the point and the line's denominator check
    /// passes there (nowhere zero, or the record's denominator evaluated non-zero at the point).
    pub fn line_status_at_point(
        &self,
        table: usize,
        record: usize,
        term: usize,
        locus: Locus,
        pt: &[i32],
    ) -> ZeroStatus {
        if !locus.contains(pt) {
            return ZeroStatus::NotProved;
        }
        let mut best = ZeroStatus::NotProved;
        for (j, lo, den) in self.of_record(table, record) {
            if *j as usize != term || *lo != locus {
                continue;
            }
            match den {
                DenCheck::NowhereZero => return ZeroStatus::Proved,
                DenCheck::AtKey(pl) | DenCheck::NowhereZeroAtKey(pl) => match pl.vanishes_at(pt) {
                    Ok(false) => return ZeroStatus::Proved,
                    Ok(true) => best = ZeroStatus::DenominatorVanishes,
                    Err(_) => {}
                },
                DenCheck::Unchecked => {}
            }
        }
        best
    }

    /// The best status of term `term` of (table, record) at the RULE POINT `pt`.
    pub fn status_at_point(
        &self,
        table: usize,
        record: usize,
        term: usize,
        pt: &[i32],
    ) -> ZeroStatus {
        let Some(v) = self.by_record.get(&(table as u32, record as u32)) else {
            return ZeroStatus::NotProved;
        };
        let mut best = ZeroStatus::NotProved;
        for (j, locus, den) in v {
            if *j as usize != term || !locus.contains(pt) {
                continue;
            }
            match den {
                DenCheck::NowhereZero => return ZeroStatus::Proved,
                DenCheck::AtKey(pl) | DenCheck::NowhereZeroAtKey(pl) => match pl.vanishes_at(pt) {
                    Ok(false) => return ZeroStatus::Proved,
                    Ok(true) => best = ZeroStatus::DenominatorVanishes,
                    Err(_) => {}
                },
                DenCheck::Unchecked => {}
            }
        }
        best
    }

    /// The status of term `term` of (table, record) applied at `key` (its rule point by the record's domain; a key
    /// without a rule point is not proved).
    pub fn is_zero_term(
        &self,
        set: &ImportedSet,
        table: usize,
        record: usize,
        term: usize,
        key: &[i32],
    ) -> ZeroStatus {
        if !self.by_record.contains_key(&(table as u32, record as u32)) {
            return ZeroStatus::NotProved;
        }
        let dom = &set.disp.tables()[table].domains[record];
        let mut pt = Vec::with_capacity(key.len());
        if !rule_point(dom, key, &mut pt) {
            return ZeroStatus::NotProved;
        }
        self.status_at_point(table, record, term, &pt)
    }

    /// Whether any proof is held for (table, record).
    pub fn has_record(&self, table: usize, record: usize) -> bool {
        self.by_record.contains_key(&(table as u32, record as u32))
    }

    /// The proofs of (table, record): (term, locus, denominator status), sorted.
    pub fn of_record(&self, table: usize, record: usize) -> &[(u32, Locus, DenCheck)] {
        self.by_record
            .get(&(table as u32, record as u32))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// lane zerosfix3: the coefficient texts of the terms `terms` of (table, record), streamed from the record's own line
/// (checked against its record_sha256). Ok(None): a term above the record budget (cannot be held).
fn record_term_texts(
    set: &ImportedSet,
    table: usize,
    record: usize,
    terms: &BTreeSet<u32>,
) -> Result<Option<HashMap<u32, String>>, String> {
    let info = set.rule(table, record);
    let budget = set.spec.read_limits().record_budget;
    let mut out: HashMap<u32, String> = HashMap::new();
    let mut bad: Option<String> = None;
    let mut visit = |j: usize, raw: &[u8]| -> Result<(), crate::record::RecordError> {
        if bad.is_some() || !terms.contains(&(j as u32)) {
            return Ok(());
        }
        match serde_json::from_slice::<Value>(raw) {
            Ok(t) => match t.get(1).and_then(Value::as_str) {
                Some(c) => {
                    out.insert(j as u32, c.to_string());
                }
                None => bad = Some(format!("rhs term {j}: [key, coefficient] expected")),
            },
            Err(e) => bad = Some(format!("rhs term {j}: {e}")),
        }
        Ok(())
    };
    match super::exact_facts::stream_record(info, budget, Some(&mut visit)) {
        Ok(_) => {}
        Err(super::error::ImportError::Refused { reason, .. })
            if matches!(*reason, super::error::TableRefusal::RecordTooLarge { .. }) =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e.to_string()),
    }
    if let Some(b) = bad {
        return Err(b);
    }
    Ok(Some(out))
}

/// A [`FactSource`] that answers like `inner`, except that an `Unknown` coefficient that a proof covers AT THE KEY
/// ([`ZeroStatus::Proved`]: the same semantics as [`ZeroProofs::is_zero_term`]) is `Zero`. Regularity and every other
/// question go to `inner`.
pub struct ZeroProofFacts<'a> {
    pub set: &'a ImportedSet,
    pub proofs: &'a ZeroProofs,
    pub inner: &'a (dyn FactSource + Sync),
    tables: BTreeMap<String, usize>,
}

impl<'a> ZeroProofFacts<'a> {
    pub fn new(
        set: &'a ImportedSet,
        proofs: &'a ZeroProofs,
        inner: &'a (dyn FactSource + Sync),
    ) -> Self {
        let tables = set
            .disp
            .tables()
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect();
        ZeroProofFacts {
            set,
            proofs,
            inner,
            tables,
        }
    }
}

impl FactSource for ZeroProofFacts<'_> {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact {
        let c = self.inner.coefficient(rule, key, term, term_key);
        if c != CoefFact::Unknown {
            return c;
        }
        let Some(&t) = self.tables.get(rule.table) else {
            return c;
        };
        match self
            .proofs
            .is_zero_term(self.set, t, rule.record, term, key)
        {
            ZeroStatus::Proved => CoefFact::Zero,
            _ => c,
        }
    }
    fn regularity(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        self.inner.regularity(rule, key)
    }
    fn exact(&self) -> bool {
        self.inner.exact()
    }
    fn label(&self) -> String {
        format!(
            "{} + zero-proofs({})",
            self.inner.label(),
            self.proofs.len()
        )
    }
    fn requested_by(&self) -> Option<String> {
        self.inner.requested_by()
    }
    fn witness_lift(&self, rule: &RuleId, key: &[i32]) -> RegFact {
        self.inner.witness_lift(rule, key)
    }
    fn per_key_proof(&self, rule: &RuleId, key: &[i32]) -> bool {
        self.inner.per_key_proof(rule, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(locus: Value, verdict: &str, den: Option<&str>) -> Value {
        let mut v = json!({"schema": ZERO_PROOF_SCHEMA, "record_sha256": "a".repeat(64), "rule": 7, "term": 2,
            "locus": locus, "verdict": verdict, "tool": {"program": "p", "version": "1"}});
        if let Some(d) = den {
            v["denominator"] = json!(d);
        }
        v
    }

    #[test]
    fn lines_parse_and_refuse() {
        let p = parse_line(&line(
            json!({"plane": [3, -1]}),
            VERDICT_PROVED,
            Some("nowhere_zero"),
        ))
        .unwrap();
        assert_eq!(
            p.locus,
            Locus::Plane {
                param: 3,
                value: -1
            }
        );
        assert_eq!(p.denominator, Some(DenStatus::NowhereZero));
        assert_eq!(p.tool, "p@1");
        let p = parse_line(&line(
            json!("domain"),
            VERDICT_PROVED,
            Some("where_regular"),
        ))
        .unwrap();
        assert_eq!(p.locus, Locus::Domain);
        assert!(parse_line(&line(json!("domain"), VERDICT_PROVED, None)).is_err());
        assert!(
            parse_line(&line(
                json!("everything"),
                VERDICT_PROVED,
                Some("nowhere_zero")
            ))
            .is_err()
        );
        assert!(
            parse_line(&line(
                json!({"plane": [3]}),
                VERDICT_PROVED,
                Some("nowhere_zero")
            ))
            .is_err()
        );
        assert!(parse_line(&line(json!("domain"), "maybe", None)).is_err());
        assert!(parse_line(&line(json!("domain"), "not_proved", None)).is_ok());
        let mut bad = line(json!("domain"), "excluded", None);
        bad["record_sha256"] = json!("xyz");
        assert!(parse_line(&bad).is_err());
    }

    #[test]
    fn locus_membership() {
        let l = Locus::Plane { param: 1, value: 0 };
        assert!(l.contains(&[5, 0, -3]));
        assert!(!l.contains(&[5, -1, -3]));
        assert!(!Locus::Plane { param: 9, value: 0 }.contains(&[0, 0]));
        assert!(Locus::Domain.contains(&[]));
        assert_eq!(Locus::from_json(&l.json()).unwrap(), l);
        assert_eq!(
            Locus::from_json(&Locus::Domain.json()).unwrap(),
            Locus::Domain
        );
    }

    #[test]
    fn a_where_regular_proof_counts_only_where_its_denominator_is_non_zero_at_the_key() {
        let den = |s: &str| {
            DenCheck::AtKey(PoleLocus::new(
                PoleKind::Denominator,
                &Poly::parse_exact(s, 3, true).unwrap(),
            ))
        };
        let mut z = ZeroProofs::new();
        z.by_record.insert(
            (0, 0),
            vec![
                (1, Locus::Plane { param: 2, value: 0 }, den("(n0-3)")),
                (
                    2,
                    Locus::Plane { param: 2, value: 0 },
                    DenCheck::NowhereZero,
                ),
                (3, Locus::Plane { param: 2, value: 0 }, DenCheck::Unchecked),
                // identically zero in d only where n0 = 3 AND n1 = 1 (both d-coefficients vanish)
                (4, Locus::Domain, den("(n0-3)*d+(n1-1)")),
            ],
        );
        assert_eq!(z.status_at_point(0, 0, 1, &[4, 1, 0]), ZeroStatus::Proved);
        assert_eq!(
            z.status_at_point(0, 0, 1, &[3, 1, 0]),
            ZeroStatus::DenominatorVanishes
        );
        assert_eq!(
            z.status_at_point(0, 0, 1, &[4, 1, -1]),
            ZeroStatus::NotProved
        );
        assert_eq!(z.status_at_point(0, 0, 2, &[3, 1, 0]), ZeroStatus::Proved);
        assert_eq!(
            z.status_at_point(0, 0, 3, &[4, 1, 0]),
            ZeroStatus::NotProved
        );
        assert_eq!(z.status_at_point(0, 0, 4, &[3, 2, -5]), ZeroStatus::Proved);
        assert_eq!(
            z.status_at_point(0, 0, 4, &[3, 1, -5]),
            ZeroStatus::DenominatorVanishes
        );
        assert_eq!(
            z.status_at_point(0, 0, 9, &[3, 1, 0]),
            ZeroStatus::NotProved
        );
        assert_eq!(
            z.status_at_point(0, 1, 1, &[4, 1, 0]),
            ZeroStatus::NotProved
        );
    }

    #[test]
    fn a_where_regular_line_carries_its_denominator() {
        let mut v = line(
            json!({"plane": [2, 0]}),
            VERDICT_PROVED,
            Some("where_regular"),
        );
        v["den"] = json!("(n0-3)");
        assert_eq!(parse_line(&v).unwrap().den.as_deref(), Some("(n0-3)"));
    }
}
