//! COMPOSITE DESCENT on Symbolica (lane repairflow; interfaces/repairflow.md 17; TRICKS 1 + 3 + 8): the exact
//! coefficient field of `rustred_campaign_core::repairflow::composite` = rational functions in d over Q, and the
//! first-match catalogue of an imported table set with each record's coefficients evaluated EXACTLY at the term.
//!
//! SYMBOLICA (L11, verified in vendor/symbolica): a coefficient text is read by the engine's path
//! [`SymbolicaAlgebra::parse_ratio`] (atom parser + `try_to_rational_polynomial` over Z in n0..n{N-1}, d); the rule
//! point is substituted exactly with `MultivariatePolynomial::replace_multiple` (src/poly/polynomial.rs:2792, the n
//! variables erased: the remainder is a polynomial in d alone); the field is `RationalPolynomialField<IntegerRing,
//! u16>` (src/domains/rational_polynomial.rs: `from_num_den` with GCD normalisation, `Ring::add / mul / neg`,
//! `Field::div`, `is_zero`). No algebra of our own: only the TEXT of a univariate result is formatted here (integer
//! coefficients and powers of d, the engine's coefficient grammar `(P)/(Q)`, `accept::facts::split_ratio` forms).
//!
//! The key -> rule point map, the dispatch and the term keys are the core's (`Dispatcher::first_match`,
//! `closure::bodies::{rule_point, parse_rhs_pattern}`), the same code the importer runs. Generic (L8): arity, frame,
//! tables and terms are data.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::{Arc, Mutex};

use rustred_campaign_algebra::SymbolicaAlgebra;
use rustred_campaign_core::accept::rule::RuleText;
use rustred_campaign_core::closure::bodies::{RhsSlot, parse_rhs_pattern, rule_point};
use rustred_campaign_core::cover::Dispatch;
use rustred_campaign_core::import::load::ImportedSet;
use rustred_campaign_core::repairflow::composite::{Catalogue, ComposeAlgebra, RuleUse, Service};
use rustred_campaign_core::replay_base::algebra::Q as RQ;
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational_polynomial::{
    FromNumeratorAndDenominator, RationalPolynomial, RationalPolynomialField,
};
use symbolica::domains::{Field, RingOps};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;

/// A rational function of d over Q (numerator and denominator over Z, normalised by Symbolica).
pub type Rd = RationalPolynomial<IntegerRing, u16>;
type ZPoly = MultivariatePolynomial<IntegerRing, u16>;

/// The exact field Q(d) on Symbolica.
pub struct SymQd {
    field: RationalPolynomialField<IntegerRing, u16>,
    vars: Arc<Vec<PolyVariable>>,
}

impl SymQd {
    /// Needs the Symbolica licence (the caller constructed a [`SymbolicaAlgebra`]).
    pub fn new() -> Result<SymQd, String> {
        let ns = NamespacedSymbol::try_parse("rustred::d").ok_or("symbol rustred::d")?;
        let d = SymbolBuilder::new(ns).build().map_err(|e| e.to_string())?;
        Ok(SymQd {
            field: RationalPolynomialField::new(Z),
            vars: Arc::new(vec![PolyVariable::Symbol(d)]),
        })
    }

    /// A rational function from a numerator and a denominator that are polynomials in d alone (nvars 1, the same
    /// variable map); `None` if the denominator is zero.
    fn ratio(&self, num: ZPoly, den: ZPoly) -> Option<Rd> {
        if den.is_zero() {
            return None;
        }
        let num = self.relabel(num);
        let den = self.relabel(den);
        Some(Rd::from_num_den(num, den, &Z, true))
    }

    /// A polynomial whose only variable is d (or a constant), re-expressed on this field's variable map.
    fn relabel(&self, p: ZPoly) -> ZPoly {
        let mut out = MultivariatePolynomial::new(&Z, None, self.vars.clone());
        let nv = p.nvars();
        for (c, e) in p.coefficients.iter().zip(p.exponents_iter()) {
            let k = if nv == 0 { 0 } else { e[nv - 1] };
            out.append_monomial(c.clone(), &[k]);
        }
        out
    }

    fn constant(&self, n: Integer, d: Integer) -> Result<Rd, String> {
        let mut num = MultivariatePolynomial::new(&Z, None, self.vars.clone());
        num.append_monomial(n, &[0]);
        let mut den = MultivariatePolynomial::new(&Z, None, self.vars.clone());
        den.append_monomial(d, &[0]);
        if den.is_zero() {
            return Err("division by zero".into());
        }
        Ok(Rd::from_num_den(num, den, &Z, true))
    }
}

fn poly_text(p: &ZPoly) -> String {
    // highest power of d first; integer coefficients; `d`, `d^k`
    let mut terms: Vec<(u16, Integer)> = p
        .coefficients
        .iter()
        .zip(p.exponents_iter())
        .map(|(c, e)| (e.first().copied().unwrap_or(0), c.clone()))
        .collect();
    if terms.is_empty() {
        return "0".into();
    }
    terms.sort_by(|a, b| b.0.cmp(&a.0));
    let mut s = String::new();
    for (i, (k, c)) in terms.iter().enumerate() {
        let neg = *c < Integer::from(0);
        let a = if neg { -c.clone() } else { c.clone() };
        if neg {
            s.push('-');
        } else if i > 0 {
            s.push('+');
        }
        let one = a == Integer::from(1);
        match k {
            0 => s.push_str(&a.to_string()),
            _ => {
                if !one {
                    s.push_str(&a.to_string());
                    s.push('*');
                }
                s.push('d');
                if *k > 1 {
                    s.push_str(&format!("^{k}"));
                }
            }
        }
    }
    s
}

impl ComposeAlgebra for SymQd {
    type E = Rd;
    fn from_q(&self, q: RQ) -> Result<Rd, String> {
        self.constant(Integer::from(q.n), Integer::from(q.d))
    }
    fn add(&self, a: &Rd, b: &Rd) -> Result<Rd, String> {
        Ok(self.field.add(a, b))
    }
    fn mul(&self, a: &Rd, b: &Rd) -> Result<Rd, String> {
        Ok(self.field.mul(a, b))
    }
    fn neg(&self, a: &Rd) -> Result<Rd, String> {
        Ok(self.field.neg(a))
    }
    fn div(&self, a: &Rd, b: &Rd) -> Result<Rd, String> {
        if b.is_zero() {
            return Err("division by an exact zero".into());
        }
        Ok(self.field.div(a, b))
    }
    fn is_zero(&self, a: &Rd) -> Result<bool, String> {
        Ok(a.is_zero())
    }
    fn text(&self, a: &Rd) -> String {
        let n = poly_text(&a.numerator);
        if a.denominator.is_one() {
            return n;
        }
        format!("({n})/({})", poly_text(&a.denominator))
    }
}

/// A record's coefficients read once (numerator, denominator over Z in n0..n{N-1}, d) with its term patterns.
struct ParsedRecord {
    rule: RuleText,
    coefs: Vec<(ZPoly, ZPoly)>,
    slots: Vec<Vec<RhsSlot>>,
}

/// The first-match catalogue of an imported table set, coefficients evaluated exactly at the term (Symbolica).
pub struct SetCatalogue<'a> {
    pub set: &'a ImportedSet,
    alg: SymbolicaAlgebra,
    cache: Mutex<HashMap<(usize, usize), Arc<Result<ParsedRecord, String>>>>,
    /// Records larger than this are not parsed (the element is refused at that term, never guessed).
    pub max_record_bytes: u64,
}

impl<'a> SetCatalogue<'a> {
    pub fn new(set: &'a ImportedSet, alg: SymbolicaAlgebra, max_record_bytes: u64) -> Self {
        SetCatalogue {
            set,
            alg,
            cache: Mutex::new(HashMap::new()),
            max_record_bytes,
        }
    }

    fn parsed(&self, table: usize, record: usize) -> Arc<Result<ParsedRecord, String>> {
        if let Some(p) = self.cache.lock().expect("cache").get(&(table, record)) {
            return p.clone();
        }
        let p = Arc::new(self.parse(table, record));
        self.cache
            .lock()
            .expect("cache")
            .insert((table, record), p.clone());
        p
    }

    fn parse(&self, table: usize, record: usize) -> Result<ParsedRecord, String> {
        let info = &self.set.rules[table][record];
        if info.bytes > self.max_record_bytes {
            return Err(format!(
                "record of {} bytes above max_record_bytes {}",
                info.bytes, self.max_record_bytes
            ));
        }
        let mut f = std::fs::File::open(&info.path).map_err(|e| format!("{}: {e}", info.path))?;
        f.seek(SeekFrom::Start(info.offset))
            .map_err(|e| e.to_string())?;
        let mut line = Vec::with_capacity(info.bytes as usize + 1);
        BufReader::new(f)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        while line.last() == Some(&b'\n') || line.last() == Some(&b'\r') {
            line.pop();
        }
        let rule = RuleText::parse(&line, &self.set.frame).map_err(|e| format!("{e:?}"))?;
        if rule.line_sha256 != info.record_sha256
            && format!(
                "sha256:{}",
                info.record_sha256.trim_start_matches("sha256:")
            ) != rule.line_sha256
        {
            return Err(format!(
                "record at {}+{} has {} (import: {})",
                info.path, info.offset, rule.line_sha256, info.record_sha256
            ));
        }
        let arity = self.set.frame.arity();
        let mut coefs = Vec::with_capacity(rule.rhs.len());
        let mut slots = Vec::with_capacity(rule.rhs.len());
        for (pat, text) in &rule.rhs {
            slots.push(parse_rhs_pattern(pat, arity)?);
            coefs.push(self.alg.parse_ratio(text, arity)?);
        }
        Ok(ParsedRecord { rule, coefs, slots })
    }
}

impl Catalogue<SymQd> for SetCatalogue<'_> {
    fn serve(&self, alg: &SymQd, term: &[i32]) -> Result<Service<Rd>, String> {
        let rr = match self.set.disp.first_match(term) {
            Ok(Dispatch::Covered(rr)) => rr,
            Ok(d) => return Ok(Service::Unserved(format!("{d:?}"))),
            Err(e) => return Ok(Service::Unserved(format!("{e:?}"))),
        };
        let p = self.parsed(rr.table, rr.record);
        let p = match p.as_ref() {
            Ok(p) => p,
            Err(e) => return Ok(Service::Unserved(format!("record unreadable: {e}"))),
        };
        let Some(dom) = p.rule.domain.as_ref() else {
            return Ok(Service::Unserved("record without a domain".into()));
        };
        let mut pt = vec![];
        if !rule_point(dom, term, &mut pt) {
            return Ok(Service::Unserved("no rule point at the term".into()));
        }
        let vals: Vec<Integer> = pt.iter().map(|&x| Integer::from(x as i64)).collect();
        let reps: Vec<(usize, &Integer)> = vals.iter().enumerate().collect();
        let mut rhs = Vec::with_capacity(p.coefs.len());
        for ((num, den), slots) in p.coefs.iter().zip(&p.slots) {
            let k: Option<Vec<i32>> = slots
                .iter()
                .map(|s| match *s {
                    RhsSlot::Param { param, offset } => pt.get(param)?.checked_add(offset),
                    RhsSlot::Const(c) => Some(c),
                })
                .collect();
            let Some(k) = k else {
                return Ok(Service::Unserved("term key out of range".into()));
            };
            let n = num.replace_multiple(&reps, true);
            let d = den.replace_multiple(&reps, true);
            let Some(c) = alg.ratio(n, d) else {
                return Ok(Service::Unserved(format!(
                    "coefficient of {k:?} has a vanishing denominator at the term (pole)"
                )));
            };
            rhs.push((k, c));
        }
        let info = &self.set.rules[rr.table][rr.record];
        Ok(Service::Rule {
            rule: RuleUse {
                table: rr.table,
                record: rr.record,
                source: info.source.clone(),
                label: info.label,
                record_sha256: info.record_sha256.clone(),
            },
            rhs,
        })
    }
}
