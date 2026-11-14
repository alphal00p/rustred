//! EXACT coefficient facts from a rule record's OWN coefficient text, on Symbolica (lane closeloop3; interfaces/
//! closeloop.md section 16).
//!
//! The zero test of a coefficient at an integer key ("identically zero in d") is exact polynomial arithmetic on the
//! record's coefficient text: it does not need the record's witness loci. Regularity at the key is NOT read off a
//! record whose witness loci are elided or absent (verdict `hidden`); it is carried by the bound per-key proof
//! (first-class level `per-key`, closure.md 16). So the facts of records `rr_proof instantiate` refuses (elided witness
//! loci, fail closed) are produced here, and each line says how (`producer: exact_eval`, `backend`, the record line's
//! `record_sha256`), distinguishable from rr_proof's facts.
//!
//! SEMANTICS = rr_proof instantiate's (crates/rustred-proof/src/instantiate.rs at e13ebcb1, read 2026-11-14): every
//! coefficient text is split into its numerator and denominator TEXTS (`accept::facts::split_ratio`, the strict forms
//! of the core; a text outside them is `unknown`, never guessed); each side is EVALUATED at the rule point, never
//! reduced or factored: the denominator vanishes identically in d -> `unknown` (and the rule is `irregular` there);
//! else the numerator vanishes identically in d -> `zero` (NOT an edge, closure.md 16), else `nonzero`.
//!
//! SYMBOLICA (L11, verified in vendor/symbolica 9cd57678): each side is read by the atom parser and converted with
//! `AtomCore::try_to_rational_polynomial` over Z in n0..n{N-1}, d ([`SymbolicaAlgebra::parse_ratio`], the engine's own
//! coefficient path); the rule point is substituted EXACTLY with `MultivariatePolynomial::replace_multiple`
//! (src/poly/polynomial.rs:2792, integer values, the substituted variables erased) and the remainder, a polynomial in d
//! over Z, is tested with `is_zero` (polynomial.rs:1130). No algebra of our own.
//!
//! The key -> rule point map, the case test (fixed slots, target pattern, equations, exception branches) and the term
//! keys are the core's dispatch logic (`domain::CaseDomain::applies`, `closure::bodies::{rule_point,
//! parse_rhs_pattern}`), the same code the importer's dispatcher runs. Generic (L8): arity, frame, slots and terms are
//! data of the record and its table header.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use rustred_campaign_core::accept::facts::{CoefficientFacts, RegVerdict, Regularity, split_ratio};
use rustred_campaign_core::accept::rule::{CreditWitness, RuleText};
use rustred_campaign_core::closure::CoefFact;
use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
use rustred_campaign_core::closure::sha256::Sha256;
use rustred_campaign_core::domain::Verdict;
use rustred_campaign_core::import::evidence::FACT_VERDICT_SCHEMA;
use rustred_campaign_core::import::lines::{read_header, term_key_digest};
use rustred_campaign_core::key::NodeFrame;
use serde_json::{Value, json};
use symbolica::domains::integer::{Integer, IntegerRing};
use symbolica::poly::polynomial::MultivariatePolynomial;

use super::SymbolicaAlgebra;

/// `producer` of every line written here.
pub const PRODUCER: &str = "exact_eval";
/// `backend` of every line written here.
pub const BACKEND: &str = "symbolica";
/// The one-line description of the evaluation (`method` of every line).
pub const METHOD: &str = "numerator / denominator texts evaluated exactly at the rule point (Symbolica \
     try_to_rational_polynomial + replace_multiple), zero test in d; no reduction, no factoring";

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;

/// One side of a coefficient ratio (its own text read as a rational function: numerator, denominator; a polynomial
/// side has denominator 1).
#[derive(Clone, Debug)]
struct Side {
    num: ZPoly,
    den: ZPoly,
}

#[derive(Clone, Debug)]
enum Parsed {
    Ok { num: Side, den: Option<Side> },
    Bad(String),
}

/// A side's value at the rule point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SideAt {
    Zero,
    NonZero,
    /// A division inside the side's own text vanishes identically: the side has no value at the point.
    Undefined,
}

fn at_point(p: &ZPoly, vals: &[Integer]) -> ZPoly {
    // only the n-variables (indices 0..arity) are substituted; d (index arity) stays
    let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().collect();
    p.replace_multiple(&reps, true)
}

fn side_at(s: &Side, vals: &[Integer]) -> SideAt {
    if at_point(&s.den, vals).is_zero() {
        SideAt::Undefined
    } else if at_point(&s.num, vals).is_zero() {
        SideAt::Zero
    } else {
        SideAt::NonZero
    }
}

fn coef_at(p: &Parsed, vals: &[Integer]) -> CoefFact {
    match p {
        Parsed::Bad(_) => CoefFact::Unknown,
        Parsed::Ok { num, den } => {
            if let Some(d) = den
                && side_at(d, vals) != SideAt::NonZero
            {
                return CoefFact::Unknown;
            }
            match side_at(num, vals) {
                SideAt::Zero => CoefFact::Zero,
                SideAt::NonZero => CoefFact::NonZero,
                SideAt::Undefined => CoefFact::Unknown,
            }
        }
    }
}

/// The facts of one (record, key), in the form the importer reads (interfaces/import.md 5.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyFacts {
    /// Fixed slots, target pattern and case equations hold at the key (the core's case test).
    pub in_case: bool,
    /// The first exception branch vanishing at the key.
    pub branch: Option<usize>,
    /// `regular` | `irregular` | `hidden` | `unusable:<why>`.
    pub verdict: String,
    /// Per term (record order): the term's key at the key and its coefficient fact.
    pub terms: Vec<(Vec<i32>, CoefFact)>,
}

impl KeyFacts {
    fn unusable(why: impl Into<String>) -> Self {
        KeyFacts {
            in_case: false,
            branch: None,
            verdict: format!("unusable:{}", why.into()),
            terms: vec![],
        }
    }
}

/// Exact facts on Symbolica behind the core's traits [`CoefficientFacts`] and [`Regularity`] (module documentation).
/// Parsed coefficients are cached per record line (sha256).
pub struct SymbolicaRecordFacts {
    alg: SymbolicaAlgebra,
    threads: usize,
    cache: HashMap<String, Arc<Vec<Parsed>>>,
}

impl SymbolicaRecordFacts {
    pub fn new(alg: SymbolicaAlgebra) -> Self {
        SymbolicaRecordFacts {
            alg,
            threads: 1,
            cache: HashMap::new(),
        }
    }

    /// Parse the coefficients of a record on `threads` threads (>= 1; results never depend on it).
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    /// Drop the cached coefficients of a record (callers that visit many records keep memory bounded).
    pub fn forget(&mut self, rule: &RuleText) {
        self.cache.remove(&rule.line_sha256);
    }

    fn parse_side(&self, text: &str, arity: usize) -> Result<Side, String> {
        let (num, den) = self.alg.parse_ratio(text.trim(), arity)?;
        if num.nvars() != arity + 1 || den.nvars() != arity + 1 {
            return Err("variable map is not n0..n{N-1}, d".into());
        }
        Ok(Side { num, den })
    }

    fn parse_one(&self, text: &str, arity: usize) -> Parsed {
        let (n, d) = match split_ratio(text.trim()) {
            Ok(x) => x,
            Err(e) => return Parsed::Bad(e),
        };
        let num = match self.parse_side(n, arity) {
            Ok(s) => s,
            Err(e) => return Parsed::Bad(format!("numerator: {e}")),
        };
        let den = match d {
            None => None,
            Some(d) => match self.parse_side(d, arity) {
                Ok(s) => Some(s),
                Err(e) => return Parsed::Bad(format!("denominator: {e}")),
            },
        };
        Parsed::Ok { num, den }
    }

    fn parsed(&mut self, rule: &RuleText) -> Arc<Vec<Parsed>> {
        if let Some(p) = self.cache.get(&rule.line_sha256) {
            return p.clone();
        }
        let arity = rule.record.fixed.len();
        let texts: Vec<&str> = rule.rhs.iter().map(|(_, c)| c.as_str()).collect();
        let n = texts.len();
        let t = self.threads.min(n.max(1));
        let mut out: Vec<Option<Parsed>> = vec![None; n];
        if t <= 1 {
            for (j, c) in texts.iter().enumerate() {
                out[j] = Some(self.parse_one(c, arity));
            }
        } else {
            let chunk = n.div_ceil(t);
            let me = &*self;
            std::thread::scope(|s| {
                let hs: Vec<_> = out
                    .chunks_mut(chunk)
                    .zip(texts.chunks(chunk))
                    .map(|(o, tx)| {
                        s.spawn(move || {
                            for (slot, c) in o.iter_mut().zip(tx) {
                                *slot = Some(me.parse_one(c, arity));
                            }
                        })
                    })
                    .collect();
                for h in hs {
                    h.join().expect("parse thread");
                }
            });
        }
        let v = Arc::new(out.into_iter().map(|p| p.expect("parsed")).collect());
        self.cache.insert(rule.line_sha256.clone(), Arc::clone(&v));
        v
    }

    /// The witness loci of the record as Symbolica polynomials (only when listed and not elided).
    fn loci(&self, rule: &RuleText) -> Result<Vec<Side>, String> {
        let arity = rule.record.fixed.len();
        rule.record
            .witness_loci
            .iter()
            .flatten()
            .map(|t| self.parse_side(t, arity).map_err(|e| format!("witness locus {t:?}: {e}")))
            .collect()
    }

    /// The facts of `rule` at `key` (module documentation). `table_guard`: the table header's guard version (a
    /// record's witness loci count as complete only with guard version >= 2, the core's witness rule).
    pub fn key_facts(&mut self, rule: &RuleText, table_guard: Option<i64>, key: &[i32]) -> KeyFacts {
        let Some(dom) = rule.domain.as_ref() else {
            return KeyFacts::unusable(format!(
                "domain: {}",
                rule.domain_error.as_deref().unwrap_or("none")
            ));
        };
        let (in_case, branch) = match dom.applies(key) {
            Verdict::Applies | Verdict::Elided | Verdict::WitnessNotCarried => (true, None),
            Verdict::OnPole { .. } => (true, None),
            Verdict::OnException { branch } => (true, Some(branch)),
            v => return KeyFacts::unusable(format!("not_in_case: {v:?}")),
        };
        let mut pt = vec![];
        if !rule_point(dom, key, &mut pt) {
            return KeyFacts::unusable("no rule point at the key");
        }
        let vals: Vec<Integer> = pt.iter().map(|&x| Integer::from(x as i64)).collect();
        let parsed = self.parsed(rule);
        let mut terms = Vec::with_capacity(parsed.len());
        let mut irregular = false;
        for (j, ((pattern, _), p)) in rule.rhs.iter().zip(parsed.iter()).enumerate() {
            let Ok(slots) = parse_rhs_pattern(pattern, pt.len()) else {
                return KeyFacts::unusable(format!("term {j}: pattern {pattern:?}"));
            };
            let tk: Option<Vec<i32>> = slots
                .iter()
                .map(|s| match *s {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let Some(tk) = tk else {
                return KeyFacts::unusable(format!("term {j}: key out of range"));
            };
            let c = coef_at(p, &vals);
            if let Parsed::Ok { den: Some(d), .. } = p
                && side_at(d, &vals) == SideAt::Zero
            {
                irregular = true;
            }
            terms.push((tk, c));
        }
        let verdict = if branch.is_some() {
            "unusable:exception_branch".to_string()
        } else if irregular {
            "irregular".into()
        } else if rule.credit_witness(table_guard) != CreditWitness::Complete {
            // elided or absent witness loci: regularity is not read off the record (the bound per-key proof carries
            // it; a `hidden` line is usable for its terms, closure.md 16)
            "hidden".into()
        } else {
            match self.loci(rule) {
                Err(e) => format!("unusable:{e}"),
                Ok(loci) => {
                    if loci.iter().any(|l| side_at(l, &vals) != SideAt::NonZero) {
                        "hidden".into()
                    } else {
                        "regular".into()
                    }
                }
            }
        };
        KeyFacts {
            in_case,
            branch,
            verdict,
            terms,
        }
    }
}

fn point_vals(rule: &RuleText, key: &[i32]) -> Option<Vec<Integer>> {
    let dom = rule.domain.as_ref()?;
    let mut pt = vec![];
    rule_point(dom, key, &mut pt).then(|| pt.iter().map(|&x| Integer::from(x as i64)).collect())
}

impl CoefficientFacts for SymbolicaRecordFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact {
        let Some(vals) = point_vals(rule, key) else {
            return CoefFact::Unknown;
        };
        match self.parsed(rule).get(term) {
            Some(p) => coef_at(p, &vals),
            None => CoefFact::Unknown,
        }
    }
}

impl Regularity for SymbolicaRecordFacts {
    /// Denominators first (`Pole`), then the witness loci: elided or absent -> `Unknown` (never read off the record);
    /// listed -> `Hidden` when one vanishes identically in d at the rule point, else `Regular`. The record's own guard
    /// version is used (the table header's is not known here: use [`SymbolicaRecordFacts::key_facts`] with it).
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        let Some(vals) = point_vals(rule, key) else {
            return RegVerdict::Unknown("no rule point at the key".into());
        };
        let parsed = self.parsed(rule);
        for (term, p) in parsed.iter().enumerate() {
            match p {
                Parsed::Bad(e) => return RegVerdict::Unknown(format!("term {term}: {e}")),
                Parsed::Ok { den: Some(d), num } => match side_at(d, &vals) {
                    SideAt::Zero => return RegVerdict::Pole { term },
                    SideAt::Undefined => {
                        return RegVerdict::Unknown(format!(
                            "term {term}: a division inside the denominator vanishes"
                        ));
                    }
                    SideAt::NonZero if side_at(num, &vals) == SideAt::Undefined => {
                        return RegVerdict::Unknown(format!(
                            "term {term}: a division inside the numerator vanishes"
                        ));
                    }
                    SideAt::NonZero => {}
                },
                Parsed::Ok { den: None, num } => {
                    if side_at(num, &vals) == SideAt::Undefined {
                        return RegVerdict::Unknown(format!(
                            "term {term}: a division inside the numerator vanishes"
                        ));
                    }
                }
            }
        }
        if rule.credit_witness(None) != CreditWitness::Complete {
            return RegVerdict::Unknown(
                "witness loci elided or not carried: regularity is not read off the record".into(),
            );
        }
        match self.loci(rule) {
            Err(e) => RegVerdict::Unknown(e),
            Ok(loci) => match loci.iter().position(|l| side_at(l, &vals) != SideAt::NonZero) {
                Some(locus) => RegVerdict::Hidden { locus },
                None => RegVerdict::Regular,
            },
        }
    }
}

fn coef_word(c: CoefFact) -> &'static str {
    match c {
        CoefFact::Zero => "zero",
        CoefFact::NonZero => "nonzero",
        CoefFact::Unknown => "unknown",
    }
}

/// The fact line of (record, key) in the importer's compact form (`rustred.campaign.fact-verdict.v1`, interfaces/
/// import.md 5.5: the term-key digests of the NON-ZERO terms only; `unknown` = denominator vanishes identically), with
/// its provenance (`producer`, `backend`, `method`, `record_sha256` = sha256 of the raw record line). `detail` adds
/// `coef` = the word of every term in record order (`zero` / `nonzero` / `unknown`) and `term_keys` (cross-checks).
pub fn fact_line(
    rule_index: i64,
    record_sha256: &str,
    key: &[i32],
    f: &KeyFacts,
    detail: bool,
    seconds: f64,
) -> Value {
    let (mut nonzero, mut unknown, mut zero) = (vec![], vec![], 0usize);
    for (tk, c) in &f.terms {
        let d = format!("{:016x}", term_key_digest(tk));
        match c {
            CoefFact::Zero => zero += 1,
            CoefFact::NonZero => nonzero.push(d),
            CoefFact::Unknown => unknown.push(d),
        }
    }
    nonzero.sort();
    nonzero.dedup();
    unknown.sort();
    unknown.dedup();
    let mut v = json!({"schema": FACT_VERDICT_SCHEMA, "producer": PRODUCER, "backend": BACKEND, "method": METHOD,
        "record_sha256": record_sha256, "rule": rule_index, "index": rule_index, "key": key,
        "in_case": f.in_case, "branch": f.branch, "verdict": f.verdict, "n_terms": f.terms.len(),
        "nonzero": nonzero, "unknown": unknown, "zero_terms": zero,
        "seconds": (seconds * 1e3).round() / 1e3});
    if detail {
        v["coef"] = json!(f.terms.iter().map(|(_, c)| coef_word(*c)).collect::<Vec<_>>());
        v["term_keys"] = json!(f.terms.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>());
    }
    v
}

/// One request (rr_proof instantiate form (A)): `{"table", "offset", "rule", "keys"}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub table: PathBuf,
    pub offset: u64,
    pub rule: i64,
    pub keys: Vec<Vec<i32>>,
}

/// Parse form (A) request lines (blank lines skipped; `offset` is required: the record is read there).
pub fn parse_requests(text: &str) -> Result<Vec<Request>, String> {
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| format!("request {}: {e}", i + 1))?;
        let (Some(table), Some(offset), Some(rule)) = (
            v["table"].as_str(),
            v["offset"].as_u64(),
            v["rule"].as_i64(),
        ) else {
            return Err(format!("request {}: table / offset / rule", i + 1));
        };
        let keys: Vec<Vec<i32>> = serde_json::from_value(v["keys"].clone())
            .map_err(|e| format!("request {}: keys: {e}", i + 1))?;
        out.push(Request {
            table: PathBuf::from(table),
            offset,
            rule,
            keys,
        });
    }
    Ok(out)
}

/// The raw line at `offset` of `path` (without its newline) and its sha256 (hex).
pub fn line_at(path: &Path, offset: u64) -> Result<(Vec<u8>, String), String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    f.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut line = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        if let Some(p) = buf[..n].iter().position(|&c| c == b'\n') {
            line.extend_from_slice(&buf[..p]);
            break;
        }
        line.extend_from_slice(&buf[..n]);
    }
    let mut h = Sha256::new();
    h.update(&line);
    Ok((line, h.finish_hex()))
}

/// A table's frame and guard version (from its header line only).
fn table_frame(path: &Path) -> Result<(NodeFrame, Option<i64>), String> {
    let h = read_header("coefficient-facts", path).map_err(|e| e.to_string())?;
    let g = h.guard_version;
    Ok((NodeFrame::from_header(&h).map_err(|e| format!("{e:?}"))?, g))
}

/// Visit every requested (record, key) in request order with its facts. A record whose line at the requested offset is
/// not rule `rule` is an error (the whole job: nothing is guessed).
pub fn for_each_pair(
    requests: &[Request],
    facts: &mut SymbolicaRecordFacts,
    mut visit: impl FnMut(i64, &str, &[i32], &KeyFacts, f64),
) -> Result<(), String> {
    let mut frames: BTreeMap<PathBuf, (NodeFrame, Option<i64>)> = BTreeMap::new();
    for r in requests {
        if !frames.contains_key(&r.table) {
            frames.insert(r.table.clone(), table_frame(&r.table)?);
        }
        let (frame, guard) = frames[&r.table].clone();
        let (line, sha) = line_at(&r.table, r.offset)?;
        let rt = RuleText::parse(&line, &frame)
            .map_err(|e| format!("{}@{}: {e:?}", r.table.display(), r.offset))?;
        if rt.index() != Some(r.rule) {
            return Err(format!(
                "the record at offset {} of {} is rule {:?}, not {}",
                r.offset,
                r.table.display(),
                rt.index(),
                r.rule
            ));
        }
        for k in &r.keys {
            let t0 = Instant::now();
            let f = facts.key_facts(&rt, guard, k);
            visit(r.rule, &sha, k, &f, t0.elapsed().as_secs_f64());
        }
        // a record's parsed coefficients are not needed after its keys (bounded memory over many records)
        facts.forget(&rt);
    }
    Ok(())
}

/// Answer every request: one fact line per (record, key), in request order.
pub fn answer(
    requests: &[Request],
    facts: &mut SymbolicaRecordFacts,
    detail: bool,
) -> Result<Vec<Value>, String> {
    let mut out = vec![];
    for_each_pair(requests, facts, |rule, sha, k, f, secs| {
        out.push(fact_line(rule, sha, k, f, detail, secs));
    })?;
    Ok(out)
}

/// rr_proof's instantiate answer of one (record, key), compact: usability, regularity word, per term (term-key digest,
/// coefficient fact).
#[derive(Clone, Debug)]
pub struct TheirAnswer {
    pub usable: bool,
    pub regular: Option<String>,
    pub terms: Vec<(u64, CoefFact)>,
    /// in_case / chart_ok / branch / error (diagnostics of a usability difference)
    pub head: Value,
}

/// The compact form of a `rustred.proof-kernel.instantiate.v1` line.
pub fn their_answer(v: &Value) -> TheirAnswer {
    let usable = v["in_case"] == true
        && v["chart_ok"] == true
        && v.get("branch").is_none_or(Value::is_null)
        && v.get("error").is_none_or(Value::is_null);
    let terms = v["rhs"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| {
                    let k: Vec<i32> = serde_json::from_value(t["key"].clone()).unwrap_or_default();
                    let c = match t["coef"].as_str() {
                        Some("zero") => CoefFact::Zero,
                        Some("nonzero") => CoefFact::NonZero,
                        _ => CoefFact::Unknown,
                    };
                    (term_key_digest(&k), c)
                })
                .collect()
        })
        .unwrap_or_default();
    TheirAnswer {
        usable,
        regular: v["regular"].as_str().map(str::to_string),
        terms,
        head: json!({"in_case": v["in_case"], "chart_ok": v["chart_ok"], "branch": v["branch"], "error": v["error"]}),
    }
}

/// The cross-check of the exact evaluation against rr_proof instantiate answers: per (record, key) both usable, every
/// term's key equal and its fact (`zero` / `nonzero` / `unknown`) equal.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CrossCheck {
    pub pairs: u64,
    pub pairs_without_answer: u64,
    pub pairs_unusable: u64,
    pub triples: u64,
    pub agree: u64,
    pub disagree: u64,
    pub by_word: BTreeMap<String, u64>,
    pub regular_compared: u64,
    pub regular_disagree: u64,
    pub records: u64,
    pub first_disagreements: Vec<Value>,
}

/// Evaluate every requested (record, key) and compare it at once with rr_proof's answer keyed by (record_sha256, key)
/// (memory: one record's parsed coefficients and the compact answers).
pub fn cross_check(
    requests: &[Request],
    facts: &mut SymbolicaRecordFacts,
    theirs: &HashMap<(String, Vec<i32>), TheirAnswer>,
) -> Result<CrossCheck, String> {
    let mut c = CrossCheck {
        records: requests.len() as u64,
        ..CrossCheck::default()
    };
    for_each_pair(requests, facts, |_, sha, key, o, _| {
        c.pairs += 1;
        let Some(t) = theirs.get(&(sha.to_string(), key.to_vec())) else {
            c.pairs_without_answer += 1;
            return;
        };
        let usable_o = o.in_case && !o.verdict.starts_with("unusable");
        if !(t.usable && usable_o) {
            c.pairs_unusable += 1;
            if t.usable != usable_o {
                c.disagree += 1;
                if c.first_disagreements.len() < 20 {
                    c.first_disagreements.push(json!({"record_sha256": sha, "key": key,
                        "what": "usability", "ours": o.verdict, "rr_proof": t.head}));
                }
            }
            return;
        }
        if let Some(b) = &t.regular
            && matches!(o.verdict.as_str(), "regular" | "irregular")
        {
            c.regular_compared += 1;
            if o.verdict != *b {
                c.regular_disagree += 1;
            }
        }
        if t.terms.len() != o.terms.len() {
            c.disagree += 1;
            if c.first_disagreements.len() < 20 {
                c.first_disagreements.push(json!({"record_sha256": sha, "key": key, "what": "term count",
                    "ours": o.terms.len(), "rr_proof": t.terms.len()}));
            }
            return;
        }
        for (j, ((tk, tw), (ok, ow))) in t.terms.iter().zip(&o.terms).enumerate() {
            c.triples += 1;
            let key_eq = *tk == term_key_digest(ok);
            if key_eq && tw == ow {
                c.agree += 1;
                *c.by_word.entry(coef_word(*ow).to_string()).or_default() += 1;
            } else {
                c.disagree += 1;
                if c.first_disagreements.len() < 20 {
                    c.first_disagreements.push(json!({"record_sha256": sha, "key": key, "term": j,
                        "ours": coef_word(*ow), "rr_proof": coef_word(*tw), "term_key_equal": key_eq}));
                }
            }
        }
    })?;
    Ok(c)
}
