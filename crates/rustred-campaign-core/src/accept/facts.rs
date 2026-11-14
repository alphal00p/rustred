//! Per-key facts of a candidate: regularity (d2) and "coefficient identically zero in d at the key" (d3 support).
//!
//! Two back-ends behind the same traits:
//! - [`ExactRecordFacts`]: from the record text, with the core's EXACT polynomial evaluation (`poly::Poly`,
//!   `domain::PoleLocus::vanishes_at`): no factoring (R38), no modular shortcut; an overflow, an unparsable text or a
//!   vanishing denominator is `Unknown`, never `Zero` / `Regular`.
//! - [`InstantiateFacts`]: the answers of `rr_proof instantiate` (EC-2, interfaces/coord-ec2-instantiate.md s.3) read
//!   from its OUT.jsonl; [`instantiate_request`] writes the request line (form (A)).

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use super::rule::RuleText;
use crate::closure::bodies::rule_point;
use crate::closure::facts::CoefFact;
use crate::domain::{PoleKind, PoleLocus};
use crate::poly::Poly;

/// d2 at one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegVerdict {
    Regular,
    /// RHS term `term` has a denominator that vanishes identically in d at the rule point.
    Pole {
        term: usize,
    },
    /// Witness locus `locus` vanishes at the rule point (a hidden weight pole: the key needs its own proof).
    Hidden {
        locus: usize,
    },
    Unknown(String),
}

/// Regularity of a rule at a key (the key, not the rule point: the back-end applies the rule's point map).
pub trait Regularity {
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict;
    /// `Some(digest of the fact file)` iff the verdicts are PER-KEY EXACT FACTS from the ENGINE (`rr_proof
    /// instantiate`): only such facts let a rule WITHOUT witness loci credit a key (coordinator ruling, addendum 3).
    /// Facts computed from the record (which cannot see hidden weight poles without the loci) give `None`.
    fn engine_fact_digest(&self) -> Option<String> {
        None
    }
    /// `Some(digest of the engine facts loaded FOR THIS RULE)` (audit-accept-A M4, B C2: facts are bound to the rule
    /// they certify, never to a record index alone): check 0 lets a rule without witness loci credit keys only when
    /// this is `Some` for the candidate itself. Default: `None` (no engine facts).
    fn fact_digest_for(&self, _rule: &RuleText) -> Option<String> {
        None
    }
}

/// Whether the coefficient of RHS term `term` vanishes identically in d at the key.
pub trait CoefficientFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact;
}

/// Split an engine coefficient text into numerator and denominator. Accepted forms ONLY (audit-accept-A M7: a text
/// such as `P + M/(Q)` has one top-level `/` but is not a ratio of the two sides): `P` (no top-level `/`), and
/// `[-](P)/(Q)`, `[-](P)/M`, `[-]M/(Q)`, `[-]M/M'` where `(P)` / `(Q)` is ONE parenthesised group spanning the whole
/// side and `M` is a monomial (no top-level `+`, no top-level `-` except a leading sign). Anything else is refused
/// (never guessed): the caller then answers `Unknown`.
pub fn split_ratio(text: &str) -> Result<(&str, Option<&str>), String> {
    let mut depth = 0i32;
    let mut cut = None;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err("unbalanced parentheses".into());
                }
            }
            '/' if depth == 0 => {
                if cut.is_some() {
                    return Err("more than one top-level '/'".into());
                }
                cut = Some(i);
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unbalanced parentheses".into());
    }
    let Some(i) = cut else {
        return Ok((text, None));
    };
    let (num, den) = (&text[..i], &text[i + 1..]);
    if !ratio_side(num, true) {
        return Err(format!(
            "numerator {num:?} is not one parenthesised group or a monomial"
        ));
    }
    if !ratio_side(den, false) {
        return Err(format!(
            "denominator {den:?} is not one parenthesised group or a monomial"
        ));
    }
    Ok((num, Some(den)))
}

/// One side of a ratio: `(..)` spanning the whole side, or a monomial; a leading `-` only on the numerator.
fn ratio_side(side: &str, allow_sign: bool) -> bool {
    let t = side.trim();
    let t = match t.strip_prefix('-') {
        Some(r) if allow_sign => r.trim_start(),
        Some(_) => return false,
        None => t,
    };
    if t.is_empty() {
        return false;
    }
    if t.starts_with('(') {
        // the group opened at 0 must close at the very end
        let mut depth = 0i32;
        for (i, c) in t.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return i + 1 == t.len();
                    }
                }
                _ => {}
            }
        }
        return false;
    }
    // monomial: no top-level + or - (a '-' directly after '^' or '*' is not allowed either: keep it strict)
    let mut depth = 0i32;
    for c in t.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' if depth == 0 => return false,
            _ => {}
        }
    }
    true
}

#[derive(Clone, Debug)]
enum Parsed {
    Ok {
        num: PoleLocus,
        den: Option<PoleLocus>,
    },
    Bad(String),
}

/// Exact facts from the record text (see the module documentation). Parsed coefficients are cached per rule.
#[derive(Default)]
pub struct ExactRecordFacts {
    cache: HashMap<String, Vec<Parsed>>,
}

impl ExactRecordFacts {
    pub fn new() -> Self {
        Self::default()
    }

    fn parsed(&mut self, rule: &RuleText) -> &Vec<Parsed> {
        let arity = rule.record.fixed.len();
        self.cache
            .entry(rule.line_sha256.clone())
            .or_insert_with(|| {
                rule.rhs
                    .iter()
                    .map(|(_, c)| {
                        let (n, d) = match split_ratio(c.trim()) {
                            Ok(x) => x,
                            Err(e) => return Parsed::Bad(e),
                        };
                        let num = match Poly::parse(n.trim(), arity, true) {
                            Ok(p) => PoleLocus::new(PoleKind::Denominator, &p),
                            Err(e) => return Parsed::Bad(format!("numerator: {e:?}")),
                        };
                        let den = match d {
                            None => None,
                            Some(d) => match Poly::parse(d.trim(), arity, true) {
                                Ok(p) => Some(PoleLocus::new(PoleKind::Denominator, &p)),
                                Err(e) => return Parsed::Bad(format!("denominator: {e:?}")),
                            },
                        };
                        Parsed::Ok { num, den }
                    })
                    .collect()
            })
    }
}

fn point_of(rule: &RuleText, key: &[i32]) -> Option<Vec<i32>> {
    let dom = rule.domain.as_ref()?;
    let mut pt = Vec::new();
    rule_point(dom, key, &mut pt).then_some(pt)
}

impl CoefficientFacts for ExactRecordFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact {
        let Some(pt) = point_of(rule, key) else {
            return CoefFact::Unknown;
        };
        match self.parsed(rule).get(term) {
            Some(Parsed::Ok { num, den }) => {
                if let Some(den) = den {
                    match den.vanishes_at(&pt) {
                        Ok(false) => {}
                        _ => return CoefFact::Unknown,
                    }
                }
                match num.vanishes_at(&pt) {
                    Ok(true) => CoefFact::Zero,
                    Ok(false) => CoefFact::NonZero,
                    Err(_) => CoefFact::Unknown,
                }
            }
            _ => CoefFact::Unknown,
        }
    }
}

impl Regularity for ExactRecordFacts {
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        let Some(pt) = point_of(rule, key) else {
            return RegVerdict::Unknown("no rule point at the key".into());
        };
        let parsed = self.parsed(rule).clone();
        for (term, p) in parsed.iter().enumerate() {
            match p {
                Parsed::Bad(e) => return RegVerdict::Unknown(format!("term {term}: {e}")),
                Parsed::Ok { den: Some(den), .. } => match den.vanishes_at(&pt) {
                    Ok(true) => return RegVerdict::Pole { term },
                    Ok(false) => {}
                    Err(e) => return RegVerdict::Unknown(format!("term {term}: {e:?}")),
                },
                Parsed::Ok { den: None, .. } => {}
            }
        }
        if let Some(dom) = &rule.domain {
            for (i, p) in dom.poles.iter().enumerate() {
                if p.kind != PoleKind::Witness {
                    continue;
                }
                match p.vanishes_at(&pt) {
                    Ok(true) => return RegVerdict::Hidden { locus: i },
                    Ok(false) => {}
                    Err(e) => return RegVerdict::Unknown(format!("witness locus {i}: {e:?}")),
                }
            }
        }
        RegVerdict::Regular
    }
}

/// Schema of the EC-2 per-(rule, key) answer lines.
pub const INSTANTIATE_SCHEMA: &str = "rustred.proof-kernel.instantiate.v1";
/// Schema of the closure-form (B) fact answers rr_proof writes beside (closure.md section 7: read by the closure's
/// `FileFacts`, not here).
pub const CLOSURE_FACT_SCHEMA: &str = "rustred.campaign.fact.v1";

/// How a tool's answer file is bound to the rule it answers for (audit-accept-A M4 M5, B C1 C2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerBinding {
    /// Every answer line must echo `rule_sha256` = the sha256 of the record line it answers for (the request carries
    /// it). The DEFAULT: evidence of one rule can never be loaded for another.
    #[default]
    DigestEcho,
    /// TRANSITION (until rr_proof echoes the digest; REQUEST to the proof-kernel owner): the record index must be
    /// echoed and equal, every structural echo present (record bytes, arity, chart, sizes, instantiated rhs keys)
    /// must equal the rule's; an echoed digest is still compared. Recorded as `binding=structural` in the artefact /
    /// fact digest so a ledger can tell it from a digest-bound answer.
    Structural,
}

impl AnswerBinding {
    pub fn tag(self) -> &'static str {
        match self {
            AnswerBinding::DigestEcho => "digest",
            AnswerBinding::Structural => "structural",
        }
    }
}

/// Check the echoed rule identity of one answer line (`what` names the file kind for the error).
pub(crate) fn check_echo(
    v: &Value,
    rule: &RuleText,
    binding: AnswerBinding,
    no: usize,
) -> Result<(), String> {
    match (v.get("rule").and_then(Value::as_i64), rule.index()) {
        (Some(a), Some(b)) if a == b => {}
        (Some(a), Some(b)) => {
            return Err(format!("line {no}: answer for rule {a}, expected {b}"));
        }
        (None, _) => return Err(format!("line {no}: answer without a rule index")),
        (Some(_), None) => return Err(format!("line {no}: the candidate record has no index")),
    }
    match v.get("rule_sha256").and_then(Value::as_str) {
        Some(d) if d == rule.line_sha256 || format!("sha256:{d}") == rule.line_sha256 => {}
        Some(d) => {
            return Err(format!(
                "line {no}: answer for rule line {d}, expected {}",
                rule.line_sha256
            ));
        }
        None if binding == AnswerBinding::DigestEcho => {
            return Err(format!(
                "line {no}: no rule_sha256 echo (binding DigestEcho required; use AnswerBinding::Structural only \
                 for tools that do not echo it yet)"
            ));
        }
        None => {}
    }
    if let Some(b) = v.get("record_bytes").and_then(Value::as_u64)
        && b != rule.line.len() as u64
        && b != rule.line.len() as u64 + 1
    {
        return Err(format!(
            "line {no}: answer for a record of {b} bytes, the candidate has {}",
            rule.line.len()
        ));
    }
    if let Some(a) = v.get("arity").and_then(Value::as_u64)
        && a != rule.record.fixed.len() as u64
    {
        return Err(format!("line {no}: answer of arity {a}"));
    }
    if let (Some(c), Some(rc)) = (v.get("chart").and_then(Value::as_str), &rule.record.chart)
        && c != rc
    {
        return Err(format!("line {no}: answer for chart {c}, the record has {rc}"));
    }
    if let Some(sz) = v.get("rule_sizes") {
        let eq = |field: &str, n: usize| -> Result<(), String> {
            match sz.get(field).and_then(Value::as_u64) {
                Some(x) if x != n as u64 => Err(format!(
                    "line {no}: answer's rule_sizes.{field} = {x}, the candidate has {n}"
                )),
                _ => Ok(()),
            }
        };
        eq("rhs_terms", rule.rhs.len())?;
        eq("exception_branches", rule.record.exceptions.len())?;
        eq("case_equations", rule.record.equations.len())?;
        if let Some(seeds) = rule.value.get("seeds").and_then(Value::as_array) {
            eq("seeds", seeds.len())?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct InstAnswer {
    regular: String,
    coef: BTreeMap<usize, String>,
    error: Option<String>,
    /// `in_case && chart_ok && branch == null` (coord-ec2 s.3): only then are the facts about a key the rule applies at
    usable: Option<String>,
}

/// Facts read from `rr_proof instantiate` OUT.jsonl (EC-2), BOUND TO THE RULE they were loaded for: answers are kept by
/// (rule line sha256, key) (audit-accept-A M4, B C2: never by record index alone). One answer file per rule
/// ([`InstantiateFacts::load`]). A missing answer is `Unknown`; `hidden` is [`RegVerdict::Hidden`]; an `error` line,
/// or a line not `in_case` / not `chart_ok` / on an exception `branch`, makes every fact of that (rule, key) Unknown.
#[derive(Default, Debug)]
pub struct InstantiateFacts {
    answers: HashMap<(String, Vec<i32>), InstAnswer>,
    /// rule line sha256 -> digest of the answer text(s) loaded for it (with the binding tag)
    loaded: HashMap<String, String>,
    pub binding: AnswerBinding,
}

impl InstantiateFacts {
    pub fn new(binding: AnswerBinding) -> Self {
        InstantiateFacts {
            binding,
            ..Default::default()
        }
    }

    /// Load an OUT.jsonl text answering for `rule` (one request per rule). Every line must be classified (finding
    /// "no silent drops"): an instantiate answer for THIS rule (index echoed and equal; `rule_sha256` echoed and equal
    /// under [`AnswerBinding::DigestEcho`]; every instantiated rhs key equal to the rule's own instantiation at the
    /// key), or a closure-form (B) answer line (schema [`CLOSURE_FACT_SCHEMA`], read by the closure). Anything else,
    /// an unreadable key, or conflicting answers for one key is an error. Returns the number of keys answered.
    pub fn load(&mut self, rule: &RuleText, text: &str) -> Result<usize, String> {
        let mut staged: Vec<(Vec<i32>, InstAnswer)> = Vec::new();
        for (no, line) in text.lines().enumerate() {
            let no = no + 1;
            if line.trim().is_empty() {
                continue;
            }
            let v: Value = serde_json::from_str(line).map_err(|e| format!("line {no}: {e}"))?;
            if v.get("schema").and_then(Value::as_str) == Some(CLOSURE_FACT_SCHEMA) {
                continue; // classified: a closure-form answer (closure FileFacts)
            }
            match v.get("schema").and_then(Value::as_str) {
                None | Some(INSTANTIATE_SCHEMA) => {}
                Some(other) => return Err(format!("line {no}: unexpected schema {other}")),
            }
            if v.get("key").is_none() {
                return Err(format!("line {no}: neither an instantiate answer nor a closure fact"));
            }
            check_echo(&v, rule, self.binding, no)?;
            let key: Vec<i32> = v["key"]
                .as_array()
                .ok_or_else(|| format!("line {no}: key is not an array"))?
                .iter()
                .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
                .collect::<Option<_>>()
                .ok_or_else(|| format!("line {no}: key not integer"))?;
            if key.len() != rule.record.fixed.len() {
                return Err(format!("line {no}: key of length {}", key.len()));
            }
            let mut coef = BTreeMap::new();
            for t in v.get("rhs").and_then(Value::as_array).into_iter().flatten() {
                let i = t
                    .get("term")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| format!("line {no}: rhs entry without a term index"))?
                    as usize;
                if let Some(tk) = t.get("key").and_then(Value::as_array) {
                    let tk: Option<Vec<i32>> = tk
                        .iter()
                        .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
                        .collect();
                    let mine = term_key_of(rule, &key, i);
                    if mine.is_some() && tk != mine {
                        return Err(format!(
                            "line {no}: rhs term {i} at {key:?} instantiates to {tk:?}, the candidate's to {mine:?}: \
                             an answer for another rule body"
                        ));
                    }
                }
                if let Some(c) = t.get("coef").and_then(Value::as_str) {
                    coef.insert(i, c.to_string());
                }
            }
            let mut usable = None;
            if v.get("in_case").is_some_and(|x| x != &Value::Bool(true)) {
                usable = Some("not in case".to_string());
            } else if v.get("chart_ok").is_some_and(|x| x != &Value::Bool(true)) {
                usable = Some("not in chart".to_string());
            } else if v.get("branch").is_some_and(|x| !x.is_null()) {
                usable = Some(format!("on exception branch {}", v["branch"]));
            }
            staged.push((
                key,
                InstAnswer {
                    regular: v
                        .get("regular")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                    coef,
                    error: v.get("error").and_then(Value::as_str).map(str::to_string),
                    usable,
                },
            ));
        }
        let mut n = 0;
        for (key, ans) in staged {
            let id = (rule.line_sha256.clone(), key);
            if let Some(old) = self.answers.get(&id)
                && (old.regular != ans.regular || old.coef != ans.coef)
            {
                return Err(format!("conflicting answers at {:?}", id.1));
            }
            self.answers.insert(id, ans);
            n += 1;
        }
        let d = super::rule::sha256_tag(text.as_bytes());
        let e = self
            .loaded
            .entry(rule.line_sha256.clone())
            .or_insert_with(|| format!("binding={};", self.binding.tag()));
        e.push_str(&d);
        Ok(n)
    }
    pub fn len(&self) -> usize {
        self.answers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }
    fn get(&self, rule: &RuleText, key: &[i32]) -> Option<&InstAnswer> {
        self.answers.get(&(rule.line_sha256.clone(), key.to_vec()))
    }
    /// True iff an answer exists for (rule, key).
    pub fn has(&self, rule: &RuleText, key: &[i32]) -> bool {
        self.get(rule, key).is_some()
    }
}

impl CoefficientFacts for InstantiateFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact {
        match self.get(rule, key) {
            Some(a) if a.error.is_none() && a.usable.is_none() => {
                match a.coef.get(&term).map(String::as_str) {
                    Some("zero") => CoefFact::Zero,
                    Some("nonzero") => CoefFact::NonZero,
                    _ => CoefFact::Unknown,
                }
            }
            _ => CoefFact::Unknown,
        }
    }
}

impl Regularity for InstantiateFacts {
    fn engine_fact_digest(&self) -> Option<String> {
        if self.loaded.is_empty() {
            return None;
        }
        let mut all: Vec<&String> = self.loaded.values().collect();
        all.sort();
        let joined: String = all.into_iter().map(String::as_str).collect();
        Some(super::rule::sha256_tag(joined.as_bytes()))
    }
    fn fact_digest_for(&self, rule: &RuleText) -> Option<String> {
        self.loaded
            .get(&rule.line_sha256)
            .map(|t| super::rule::sha256_tag(t.as_bytes()))
    }
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        match self.get(rule, key) {
            None => RegVerdict::Unknown("no instantiate answer for this rule at this key".into()),
            Some(a) if a.error.is_some() => RegVerdict::Unknown(format!(
                "instantiate error {}",
                a.error.clone().unwrap_or_default()
            )),
            Some(InstAnswer {
                usable: Some(why), ..
            }) => RegVerdict::Unknown(format!("instantiate: {why}")),
            Some(a) => match a.regular.as_str() {
                "regular" => RegVerdict::Regular,
                "irregular" => RegVerdict::Pole { term: usize::MAX },
                "hidden" => RegVerdict::Hidden { locus: usize::MAX },
                other => RegVerdict::Unknown(format!("regular = {other}")),
            },
        }
    }
}

/// EC-2 request line, form (A): `{"table": PATH, "offset": BYTES?, "rule": INDEX, "rule_sha256": DIGEST, "keys":
/// [[..], ..]}` (`rule_sha256`: the identity the answer must echo, [`AnswerBinding::DigestEcho`]).
pub fn instantiate_request(
    table: &str,
    offset: Option<u64>,
    rule: &RuleText,
    keys: &[Vec<i32>],
) -> String {
    let mut v = json!({"table": table, "rule": rule.index(), "rule_sha256": rule.line_sha256, "keys": keys});
    if let Some(o) = offset {
        v["offset"] = json!(o);
    }
    v.to_string()
}

/// The closure's fact source (Tier C, closure.md s.2) over the exact record facts: rules are looked up by (FACT
/// IDENTITY of the table, record position): the closure's `RuleId.table` is `ClosureConfig.fact_tables[name]`, else
/// the table's `provenance.source` (the path for loaded tables, the name for manual ones).
pub struct ClosureFacts {
    rules: HashMap<(String, usize), RuleText>,
    inner: std::sync::Mutex<ExactRecordFacts>,
}

impl ClosureFacts {
    pub fn new() -> Self {
        ClosureFacts {
            rules: HashMap::new(),
            inner: std::sync::Mutex::new(ExactRecordFacts::new()),
        }
    }
    /// Register the record at (table name, record position of the dispatcher's table).
    pub fn insert(&mut self, table: &str, record: usize, rule: RuleText) {
        self.rules.insert((table.to_string(), record), rule);
    }
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

impl Default for ClosureFacts {
    fn default() -> Self {
        Self::new()
    }
}

/// The instantiated key of term `term` of `rule` at `key` (its right-hand-side pattern at the rule point); `None`
/// when the key is not in the rule's case domain or the pattern does not parse.
fn term_key_of(rule: &RuleText, key: &[i32], term: usize) -> Option<Vec<i32>> {
    let pt = point_of(rule, key)?;
    let (pattern, _) = rule.rhs.get(term)?;
    let slots = crate::closure::bodies::parse_rhs_pattern(pattern, pt.len()).ok()?;
    slots
        .iter()
        .map(|s| match *s {
            crate::closure::bodies::RhsSlot::Param { param, offset } => {
                pt.get(param)?.checked_add(offset)
            }
            crate::closure::bodies::RhsSlot::Const(c) => Some(c),
        })
        .collect()
}

impl crate::closure::facts::FactSource for ClosureFacts {
    /// Since loopfix (closure.md 11.5) a coefficient fact is bound to the term's own instantiated key: the record
    /// registered at (table, record) answers only if ITS term `term` at `key` instantiates to `term_key` (a
    /// registration that does not match the closure's rule body answers `Unknown`).
    fn coefficient(
        &self,
        rule: &crate::closure::facts::RuleId,
        key: &[i32],
        term: usize,
        term_key: &[i32],
    ) -> CoefFact {
        match self.rules.get(&(rule.table.to_string(), rule.record)) {
            Some(r) if term_key_of(r, key, term).as_deref() != Some(term_key) => CoefFact::Unknown,
            Some(r) => self
                .inner
                .lock()
                .map(|mut f| f.coefficient(r, key, term))
                .unwrap_or(CoefFact::Unknown),
            None => CoefFact::Unknown,
        }
    }
    fn regularity(
        &self,
        rule: &crate::closure::facts::RuleId,
        key: &[i32],
    ) -> crate::closure::facts::RegFact {
        use crate::closure::facts::RegFact;
        let Some(r) = self.rules.get(&(rule.table.to_string(), rule.record)) else {
            return RegFact::Unknown;
        };
        match self.inner.lock().map(|mut f| f.regular(r, key)) {
            Ok(RegVerdict::Regular) => RegFact::Regular,
            Ok(RegVerdict::Pole { .. }) => RegFact::Pole,
            _ => RegFact::Unknown,
        }
    }
    fn label(&self) -> String {
        "accept:exact-record".into()
    }
}

#[cfg(test)]
mod tests {
    use super::split_ratio;

    #[test]
    fn split_forms() {
        assert_eq!(split_ratio("1").unwrap(), ("1", None));
        assert_eq!(split_ratio("(a)/(b)").unwrap(), ("(a)", Some("(b)")));
        assert_eq!(
            split_ratio("-n0/(-1+n4)").unwrap(),
            ("-n0", Some("(-1+n4)"))
        );
        assert!(split_ratio("a/b/c").is_err());
        assert!(split_ratio("(a/b").is_err());
        // audit-accept-A M7: a sum with a ratio inside is not a ratio of its two sides
        assert!(split_ratio("n2-4+1/(n1)").is_err());
        assert!(split_ratio("(a)+(b)/(c)").is_err());
        assert!(split_ratio("(a)/(b)+1").is_err());
        assert!(split_ratio("(a)/-(b)").is_err());
        assert_eq!(split_ratio("-(a)/(b)").unwrap(), ("-(a)", Some("(b)")));
        assert_eq!(split_ratio("(3+n9-d)/n15").unwrap(), ("(3+n9-d)", Some("n15")));
        assert_eq!(split_ratio("2*n1^2/(n3)").unwrap(), ("2*n1^2", Some("(n3)")));
    }
}
