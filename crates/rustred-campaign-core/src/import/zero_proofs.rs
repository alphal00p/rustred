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
}

/// The proofs of one record: (term, locus, denominator check).
pub type RecordProofs = Vec<(u32, Locus, DenCheck)>;

/// The proofs of a set, by (table, record) and term (see the module documentation).
#[derive(Clone, Debug, Default)]
pub struct ZeroProofs {
    by_record: HashMap<(u32, u32), RecordProofs>,
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

    /// Bind one parsed line to the set (VERIFIED: the record's term count, the parameter range, the index echo).
    /// `Ok(false)`: the record is not in the set (skipped, counted foreign).
    pub fn insert(&mut self, set: &ImportedSet, l: &ProofLine) -> Result<bool, String> {
        self.stats.lines += 1;
        let Some((t, r)) = set.find_sha(&l.record_sha256) else {
            self.stats.foreign += 1;
            return Ok(false);
        };
        let n = set.bodies.n_terms(t, r);
        if !set.bodies.has(t, r) || l.term as usize >= n {
            return Err(format!(
                "record {}: term {} (the record has {n} terms)",
                l.record_sha256, l.term
            ));
        }
        if let Locus::Plane { param, .. } = l.locus
            && param as usize >= set.arity()
        {
            return Err(format!(
                "record {}: plane on parameter {param} (arity {})",
                l.record_sha256,
                set.arity()
            ));
        }
        let info = set.rule(t, r);
        let echo_ok = match (l.rule, info.index) {
            (Some(a), Some(b)) => a == b,
            (Some(a), None) => a == info.label,
            (None, _) => false,
        };
        if !echo_ok {
            return Err(format!(
                "record {}: rule echo {:?} (the record's index is {:?})",
                l.record_sha256, l.rule, info.index
            ));
        }
        match (l.verdict.as_str(), l.denominator) {
            (VERDICT_PROVED, Some(d)) => {
                self.stats.proved += 1;
                let check = match d {
                    DenStatus::NowhereZero => {
                        self.stats.proved_nowhere_zero += 1;
                        DenCheck::NowhereZero
                    }
                    DenStatus::WhereRegular => match l
                        .den
                        .as_deref()
                        .and_then(|s| Poly::parse_exact(s.trim(), set.arity(), true).ok())
                    {
                        Some(p) => {
                            self.stats.den_at_key += 1;
                            DenCheck::AtKey(PoleLocus::new(PoleKind::Denominator, &p))
                        }
                        None => {
                            self.stats.den_unchecked += 1;
                            DenCheck::Unchecked
                        }
                    },
                };
                let v = self.by_record.entry((t as u32, r as u32)).or_default();
                if v.is_empty() {
                    self.stats.records += 1;
                }
                if !v
                    .iter()
                    .any(|(j, lo, c)| *j == l.term && *lo == l.locus && *c == check)
                {
                    v.push((l.term, l.locus, check));
                }
            }
            ("not_proved", _) => self.stats.not_proved += 1,
            _ => self.stats.excluded += 1,
        }
        self.stats.tools.insert(l.tool.clone());
        Ok(true)
    }

    /// Read proof files into `self` (each line verified; any violation refuses with the file and line).
    pub fn load_files(
        &mut self,
        set: &ImportedSet,
        files: &[PathBuf],
        opts: &LoadOptions,
    ) -> Result<(), String> {
        for p in files {
            self.load_file(set, p, opts)?;
        }
        Ok(())
    }

    fn load_file(&mut self, set: &ImportedSet, p: &Path, opts: &LoadOptions) -> Result<(), String> {
        let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
        self.stats.files += 1;
        for (i, l) in std::io::BufReader::with_capacity(1 << 20, f)
            .lines()
            .enumerate()
        {
            let l = l.map_err(|e| format!("{}: {e}", p.display()))?;
            if l.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(&l)
                .map_err(|e| format!("{} line {}: {e}", p.display(), i + 1))?;
            if v["schema"] != ZERO_PROOF_SCHEMA {
                // other lines of a mixed answer file (per-key verdicts) are not proofs
                continue;
            }
            let pl = parse_line(&v).map_err(|e| format!("{} line {}: {e}", p.display(), i + 1))?;
            if let Some(allow) = &opts.tools
                && !allow.contains(&pl.tool)
            {
                return Err(format!(
                    "{} line {}: producer {} is not allowed",
                    p.display(),
                    i + 1,
                    pl.tool
                ));
            }
            self.insert(set, &pl)
                .map_err(|e| format!("{} line {}: {e}", p.display(), i + 1))?;
        }
        Ok(())
    }

    /// Load proof files for `set` (module documentation).
    pub fn load(set: &ImportedSet, files: &[PathBuf], opts: &LoadOptions) -> Result<Self, String> {
        let mut z = ZeroProofs::new();
        z.load_files(set, files, opts)?;
        Ok(z)
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
                DenCheck::AtKey(pl) => match pl.vanishes_at(pt) {
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

/// The SECOND ROUTE of a first-route zero, by the first route's PRODUCER word (audit r2-planners G1; the same words and
/// table as lane zerosfix3's `closeloop/zeros.rs::ZeroRoute`, closeloop.md 23.3 + 18.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZeroRoute {
    /// The first route IS the exact evaluation (Symbolica): the zeros route runs on the same algebra and is not
    /// independent; only closeloop4's term-by-term comparison with rr_proof confirms (a confirmed basis word).
    Compare,
    /// The zeros route (structural proofs, `rr_coef_facts`, Symbolica): independent of rr_proof and of an unnamed
    /// compact producer.
    Zeros,
    /// No route knows the first route's word (`none`, `store` of a pre-v3 store, `unknown_producer`, anything else):
    /// the zero is never confirmed (`second_route_basis_unknown`; fail closed).
    Unknown,
}

/// The [`ZeroRoute`] of a first-route PRODUCER word (never a confirmed basis word).
pub fn zero_route_of(first: &str) -> ZeroRoute {
    match first {
        "exact_eval" => ZeroRoute::Compare,
        "rr_proof" | "rr_proof_elided" | "compact" => ZeroRoute::Zeros,
        _ => ZeroRoute::Unknown,
    }
}

/// Whether a second-route answer of producer `second` is INDEPENDENT of the first route's producer `first` (they share
/// no code below the coefficient text): rr_proof / rr_proof_elided / an unnamed compact producer against the exact
/// evaluation, and back (the same table as lane zerosfix3's `closeloop/zeros.rs::independent`).
pub fn independent(first: &str, second: &str) -> bool {
    match first {
        "rr_proof" | "compact" | "rr_proof_elided" => second == "exact_eval",
        "exact_eval" => second == "rr_proof" || second == "rr_proof_elided",
        _ => false,
    }
}

/// Whether a fact BASIS word is a CONFIRMED basis: `<first> + <second>` of two INDEPENDENT producers (the words
/// closeloop4's comparison writes through `Evidence::confirm_basis` when the two routes agree term by term).
pub fn is_confirmed_basis(word: &str) -> bool {
    match word.split_once(" + ") {
        Some((a, b)) => independent(a.trim(), b.trim()),
        None => false,
    }
}

/// Why a first-route zero is (not) confirmed (audit r2-planners G1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ZeroConfirmation {
    /// closeloop4's comparison: the fact's basis is a confirmed basis word ([`is_confirmed_basis`]).
    ConfirmedBasis,
    /// A structural zero proof of the term at the key, and the first route's producer is one the zeros route is
    /// independent of ([`ZeroRoute::Zeros`]).
    ZerosRoute,
    /// No independent second route confirmed it (`second_route_unconfirmed`).
    Unconfirmed,
    /// The first route's producer is no known producer (`second_route_basis_unknown`).
    BasisUnknown,
}

impl ZeroConfirmation {
    pub fn confirmed(self) -> bool {
        matches!(
            self,
            ZeroConfirmation::ConfirmedBasis | ZeroConfirmation::ZerosRoute
        )
    }
    pub fn word(self) -> &'static str {
        match self {
            ZeroConfirmation::ConfirmedBasis => "confirmed_basis",
            ZeroConfirmation::ZerosRoute => "zeros_route",
            ZeroConfirmation::Unconfirmed => "second_route_unconfirmed",
            ZeroConfirmation::BasisUnknown => "second_route_basis_unknown",
        }
    }
}

/// The confirmation of a FIRST-ROUTE zero of term `term` of `rule` at `key`: `basis` = the evidence's fact basis word
/// of (rule, key) (`Evidence::fact_basis`: the producer, or a confirmed basis word), `facts` = the source that may hold
/// a structural zero proof ([`FactSource::zero_proof`]).
pub fn confirm_first_route_zero(
    basis: Option<&str>,
    facts: &dyn FactSource,
    rule: &RuleId,
    key: &[i32],
    term: usize,
) -> ZeroConfirmation {
    let basis = basis.unwrap_or("none");
    if is_confirmed_basis(basis) {
        return ZeroConfirmation::ConfirmedBasis;
    }
    match zero_route_of(basis) {
        ZeroRoute::Zeros if facts.zero_proof(rule, key, term) => ZeroConfirmation::ZerosRoute,
        ZeroRoute::Zeros | ZeroRoute::Compare => ZeroConfirmation::Unconfirmed,
        ZeroRoute::Unknown => ZeroConfirmation::BasisUnknown,
    }
}

/// A [`FactSource`] that answers like `inner`, except that an `Unknown` coefficient that a proof covers AT THE KEY
/// ([`ZeroStatus::Proved`]: the same semantics as [`ZeroProofs::is_zero_term`]) is `Zero`. Regularity and every other
/// question go to `inner`. audit r2-planners G1: such a proof-only `Zero` is ONE route; the ship gate does not drop a
/// term on it (nor on the inner source's first-route `Zero` alone): it asks [`FactSource::zero_proof`] as the second
/// route of the evidence's first-route zero ([`confirm_first_route_zero`]).
pub struct ZeroProofFacts<'a> {
    pub set: &'a ImportedSet,
    pub proofs: &'a ZeroProofs,
    pub inner: &'a (dyn FactSource + Sync),
    tables: BTreeMap<String, usize>,
    /// audit r2-zeros F5: the proofs come from files whose standalone verification is pinned
    /// ([`VerifiedZeroProofs`]); unverified proofs give NO fact (the inner source answers alone).
    verified: bool,
}

impl<'a> ZeroProofFacts<'a> {
    /// audit r2-zeros F5: proofs NOT bound to a pinned verification: this source answers like `inner` and holds no
    /// proof fact (no `Unknown -> Zero`, no [`FactSource::zero_proof`]). Use [`ZeroProofFacts::verified`].
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
            verified: false,
        }
    }
    /// The proofs of files whose standalone verification is pinned ([`VerifiedZeroProofs::load`]): their facts count.
    pub fn verified(
        set: &'a ImportedSet,
        proofs: &'a VerifiedZeroProofs,
        inner: &'a (dyn FactSource + Sync),
    ) -> Self {
        let mut f = ZeroProofFacts::new(set, &proofs.proofs, inner);
        f.verified = true;
        f
    }
    /// Whether the proofs' verification is pinned (else they give no fact).
    pub fn is_verified(&self) -> bool {
        self.verified
    }
}

/// Schema of the standalone verifier's summary (`rr_coef_facts verify-zero-proofs --summary`).
pub const VERIFY_SUMMARY_SCHEMA: &str = "rustred.campaign.zero-proof-verify.v1";

/// audit r2-zeros F5: proof files whose STANDALONE VERIFICATION is pinned: the verifier's summary (pinned by its own
/// sha256) says `verified`, no mismatch, no claim without its record, no header mismatch, and lists the sha256 of every
/// proof file it verified (`proofs_sha256`); each file loaded here hashes to one of them. Cheaper than re-checking the
/// numerator per lookup (the numerator split needs the Symbolica-backed algebra; the verifier re-checks every claim
/// once, independently, with the core's own polynomial). Refusals are typed `zero_proofs_unverified: ...`.
pub struct VerifiedZeroProofs {
    pub proofs: ZeroProofs,
    /// (path, sha256) of every proof file loaded.
    pub files: Vec<(String, String)>,
    /// sha256 of the verify summary (the pin).
    pub summary_sha256: String,
}

impl VerifiedZeroProofs {
    pub fn load(
        set: &ImportedSet,
        files: &[PathBuf],
        summary: &Path,
        summary_sha256: &str,
        opts: &LoadOptions,
    ) -> Result<VerifiedZeroProofs, String> {
        let un = |m: String| format!("zero_proofs_unverified: {m}");
        let bytes =
            std::fs::read(summary).map_err(|e| un(format!("{}: {e}", summary.display())))?;
        let sha = |b: &[u8]| {
            let mut h = crate::closure::sha256::Sha256::new();
            h.update(b);
            h.finish_hex()
        };
        let pin = summary_sha256
            .trim_start_matches("sha256:")
            .to_ascii_lowercase();
        let got = sha(&bytes);
        if got != pin {
            return Err(un(format!(
                "the verify summary {} has sha256 {got}, pinned {pin}",
                summary.display()
            )));
        }
        let text = String::from_utf8_lossy(&bytes);
        let v: Value = text
            .lines()
            .find(|l| !l.trim().is_empty())
            .and_then(|l| serde_json::from_str(l).ok())
            .ok_or_else(|| un(format!("{}: no JSON summary line", summary.display())))?;
        if v["schema"] != VERIFY_SUMMARY_SCHEMA {
            return Err(un(format!("summary schema {}", v["schema"])));
        }
        let clean = v["verified"] == true
            && v["counts"]["mismatches"].as_u64() == Some(0)
            && v["counts"]["claims_without_record"].as_u64() == Some(0)
            && v["header_mismatch"]
                .as_array()
                .is_some_and(|a| a.is_empty());
        if !clean {
            return Err(un("the summary does not state a clean verification".into()));
        }
        let listed: BTreeSet<String> = v["proofs_sha256"]
            .as_array()
            .ok_or_else(|| {
                un("the summary binds no proof file by its sha256 (proofs_sha256 absent)".into())
            })?
            .iter()
            .filter_map(|x| {
                x.as_str()
                    .map(|s| s.trim_start_matches("sha256:").to_ascii_lowercase())
            })
            .collect();
        let mut out = vec![];
        for f in files {
            let b = std::fs::read(f).map_err(|e| un(format!("{}: {e}", f.display())))?;
            let h = sha(&b);
            if !listed.contains(&h) {
                return Err(un(format!(
                    "{} (sha256 {h}) is not a file the pinned summary verified",
                    f.display()
                )));
            }
            out.push((f.display().to_string(), h));
        }
        let proofs = ZeroProofs::load(set, files, opts)?;
        Ok(VerifiedZeroProofs {
            proofs,
            files: out,
            summary_sha256: pin,
        })
    }
}

impl FactSource for ZeroProofFacts<'_> {
    fn coefficient(&self, rule: &RuleId, key: &[i32], term: usize, term_key: &[i32]) -> CoefFact {
        let c = self.inner.coefficient(rule, key, term, term_key);
        if c != CoefFact::Unknown || !self.verified {
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
    /// audit r2-planners G1: the structural proof itself (the SECOND route of a first-route zero; the ship gate drops a
    /// term only on a first-route zero confirmed by it, [`confirm_first_route_zero`]).
    fn zero_proof(&self, rule: &RuleId, key: &[i32], term: usize) -> bool {
        if !self.verified {
            // audit r2-zeros F5: an unverified proof file gives no fact
            return self.inner.zero_proof(rule, key, term);
        }
        let Some(&t) = self.tables.get(rule.table) else {
            return self.inner.zero_proof(rule, key, term);
        };
        self.proofs
            .is_zero_term(self.set, t, rule.record, term, key)
            == ZeroStatus::Proved
            || self.inner.zero_proof(rule, key, term)
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
