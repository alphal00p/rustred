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
}

/// Whether the coefficient of RHS term `term` vanishes identically in d at the key.
pub trait CoefficientFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact;
}

/// Split an engine coefficient text at its ONE top-level `/` (forms `(P)/(Q)`, `-(P)/(Q)`, `M/(Q)`, `(P)/M`, `P`).
/// More than one top-level `/` is refused (never guessed).
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
    Ok(match cut {
        None => (text, None),
        Some(i) => (&text[..i], Some(&text[i + 1..])),
    })
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

#[derive(Clone, Debug)]
struct InstAnswer {
    regular: String,
    coef: BTreeMap<usize, String>,
    error: Option<String>,
}

/// Facts read from `rr_proof instantiate` OUT.jsonl (EC-2). Keyed by (record `index`, key). A missing answer is
/// `Unknown`; `hidden` is [`RegVerdict::Hidden`]; an `error` line makes every fact of that (rule, key) Unknown.
#[derive(Default, Debug)]
pub struct InstantiateFacts {
    answers: HashMap<(i64, Vec<i32>), InstAnswer>,
    digest: String,
}

impl InstantiateFacts {
    /// Read an OUT.jsonl text; lines of other schemas (closure fact answers) are skipped. Conflicting answers for
    /// one (rule, key) are an error (fail closed).
    pub fn read(text: &str) -> Result<InstantiateFacts, String> {
        let mut f = InstantiateFacts {
            digest: super::rule::sha256_tag(text.as_bytes()),
            ..Default::default()
        };
        for (no, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value =
                serde_json::from_str(line).map_err(|e| format!("line {}: {e}", no + 1))?;
            if v.get("schema").and_then(Value::as_str) == Some("rustred.campaign.fact.v1") {
                continue;
            }
            let Some(rule) = v.get("rule").and_then(Value::as_i64) else {
                continue;
            };
            let Some(key) = v.get("key").and_then(Value::as_array) else {
                continue;
            };
            let key: Vec<i32> = key
                .iter()
                .map(|x| x.as_i64().and_then(|y| i32::try_from(y).ok()))
                .collect::<Option<_>>()
                .ok_or_else(|| format!("line {}: key not integer", no + 1))?;
            let mut coef = BTreeMap::new();
            if let Some(a) = v.get("rhs").and_then(Value::as_array) {
                for t in a {
                    if let (Some(i), Some(c)) = (
                        t.get("term").and_then(Value::as_u64),
                        t.get("coef").and_then(Value::as_str),
                    ) {
                        coef.insert(i as usize, c.to_string());
                    }
                }
            }
            let ans = InstAnswer {
                regular: v
                    .get("regular")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                coef,
                error: v.get("error").and_then(Value::as_str).map(str::to_string),
            };
            if let Some(old) = f.answers.get(&(rule, key.clone()))
                && (old.regular != ans.regular || old.coef != ans.coef)
            {
                return Err(format!(
                    "line {}: conflicting answers for rule {rule}",
                    no + 1
                ));
            }
            f.answers.insert((rule, key), ans);
        }
        Ok(f)
    }
    pub fn len(&self) -> usize {
        self.answers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.answers.is_empty()
    }
    fn get(&self, rule: &RuleText, key: &[i32]) -> Option<&InstAnswer> {
        self.answers.get(&(rule.index()?, key.to_vec()))
    }
    /// True iff an answer exists for (rule, key).
    pub fn has(&self, rule: &RuleText, key: &[i32]) -> bool {
        self.get(rule, key).is_some()
    }
}

impl CoefficientFacts for InstantiateFacts {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> CoefFact {
        match self.get(rule, key) {
            Some(a) if a.error.is_none() => match a.coef.get(&term).map(String::as_str) {
                Some("zero") => CoefFact::Zero,
                Some("nonzero") => CoefFact::NonZero,
                _ => CoefFact::Unknown,
            },
            _ => CoefFact::Unknown,
        }
    }
}

impl Regularity for InstantiateFacts {
    fn engine_fact_digest(&self) -> Option<String> {
        Some(self.digest.clone())
    }
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        match self.get(rule, key) {
            None => RegVerdict::Unknown("no instantiate answer".into()),
            Some(a) if a.error.is_some() => RegVerdict::Unknown(format!(
                "instantiate error {}",
                a.error.clone().unwrap_or_default()
            )),
            Some(a) => match a.regular.as_str() {
                "regular" => RegVerdict::Regular,
                "irregular" => RegVerdict::Pole { term: usize::MAX },
                "hidden" => RegVerdict::Hidden { locus: usize::MAX },
                other => RegVerdict::Unknown(format!("regular = {other}")),
            },
        }
    }
}

/// EC-2 request line, form (A): `{"table": PATH, "offset": BYTES?, "rule": INDEX, "keys": [[..], ..]}`.
pub fn instantiate_request(
    table: &str,
    offset: Option<u64>,
    index: i64,
    keys: &[Vec<i32>],
) -> String {
    let mut v = json!({"table": table, "rule": index, "keys": keys});
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

impl crate::closure::facts::FactSource for ClosureFacts {
    fn coefficient(
        &self,
        rule: &crate::closure::facts::RuleId,
        key: &[i32],
        term: usize,
    ) -> CoefFact {
        match self.rules.get(&(rule.table.to_string(), rule.record)) {
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
    }
}
