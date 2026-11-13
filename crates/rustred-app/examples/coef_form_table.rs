//! coef-form: streaming size census and factored-text writer for a node rule
//! table (`F.table.jsonl`, `table_dump.v1`).
//!
//! coef_form_table <F.table.jsonl> <report.jsonl> [--label T] [--write OUT.jsonl] [--no-zstd]
//!
//! Reads the table one record at a time (memory: one rule record + a per-table
//! factor dictionary index), parses every RHS coefficient over (d, n0..n{k-1})
//! with Symbolica, factors numerator and denominator into irreducibles
//! (`Factorize::factor`; a polynomial above `COEF_FORM_FACTOR_CAP` terms,
//! default `FACTOR_TERM_CAP` = 4000, is kept whole; so is one whose
//! factorisation exceeds `COEF_FORM_FACTOR_TIMEOUT` s, default 60) and reports, as one JSON
//! line appended to `report.jsonl`:
//!
//! - bytes: the table as given, the same table with factored coefficient text
//!   (`--write` form), a compact binary of the distinct expanded coefficients
//!   and of the factored form (dictionary of distinct factors + per
//!   coefficient content and factor references); each also zstd -3;
//! - per rule, the SIZE MEASURES (monomial counts):
//!   * `audit`    = '+'/'-' count of the record's RHS text (size_audit_all.py);
//!   * `expanded` = sum over the RHS occurrences of numerator + denominator
//!                  terms (the size ruling's measure, exact);
//!   * `factored` = sum of the terms of the DISTINCT dictionary entries the
//!                  rule's RHS uses (irreducible factors, plus every polynomial
//!                  kept whole = the non-factored remainder, counted in full)
//!                  + one per factor reference + 2 per occurrence (contents);
//!   * `factored_whole` = the kept-whole part of `factored`;
//!   * `text`     = '+'/'-' count of the factored RHS text (the audit's
//!                  method applied to the `--write` form: factors repeated per
//!                  occurrence, no dictionary);
//!   and band counts (> 1M, > 10M) for each measure;
//! - times: parse, factor (incl. the exact multiply-back check of every
//!   factorisation = the load cost of the factored form), text check.
//!
//! `--write OUT.jsonl` writes the table with every RHS coefficient replaced by
//! `content*(f1)^e1*.../(content*(g1)^e1*...)` in the same grammar (integers,
//! `n<i>`, `d`, `+ - * / ^ ( )`), header extended by
//! `"coef_form":"factored-text.v1"` (TABLE_COEF_FORM); every other byte of
//! every record is unchanged. Each written coefficient is re-parsed and must
//! equal the original (numerator and denominator), else the run aborts.
#![recursion_limit = "512"]
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientPolynomial};
use rustred::persistence::FACTOR_TERM_CAP;
use symbolica::atom::AtomCore;
use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, PolyVariable, Q, Z};
use symbolica::{symbol, try_parse};

type Poly = CoefficientPolynomial;

/// Version tag of the factored coefficient text written by `--write`.
const TABLE_COEF_FORM: &str = "factored-text.v1";

// ---------- byte helpers (compact binary, as coef_form_probe) ----------

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}
fn put_integer(out: &mut Vec<u8>, c: &Integer) {
    match c {
        Integer::Single(i) if i.unsigned_abs() < (1u64 << 61) => {
            let z = ((*i << 1) ^ (*i >> 63)) as u64;
            varint(out, z << 1);
        }
        other => {
            let s = other.to_string();
            varint(out, ((s.len() as u64) << 1) | 1);
            out.extend_from_slice(s.as_bytes());
        }
    }
}
fn put_poly(out: &mut Vec<u8>, p: &Poly) {
    varint(out, p.nterms() as u64);
    for i in 0..p.nterms() {
        put_integer(out, &p.coefficients[i]);
        let e = p.exponents(i);
        varint(out, e.iter().filter(|&&x| x != 0).count() as u64);
        for (v, &x) in e.iter().enumerate() {
            if x != 0 {
                varint(out, v as u64);
                varint(out, u64::from(x));
            }
        }
    }
}

/// A byte sink that counts and optionally pipes into `zstd -3`.
struct Sink {
    bytes: u64,
    child: Option<Child>,
    out: Option<std::thread::JoinHandle<u64>>,
}
impl Sink {
    fn new(zstd: bool) -> Sink {
        if !zstd {
            return Sink {
                bytes: 0,
                child: None,
                out: None,
            };
        }
        let mut child = Command::new("zstd")
            .args(["-q", "-3", "-T1", "-c"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("zstd");
        let mut stdout = child.stdout.take().unwrap();
        let out = std::thread::spawn(move || {
            let mut buf = vec![0u8; 1 << 20];
            let mut n = 0u64;
            loop {
                let k = stdout.read(&mut buf).unwrap();
                if k == 0 {
                    return n;
                }
                n += k as u64;
            }
        });
        Sink {
            bytes: 0,
            child: Some(child),
            out: Some(out),
        }
    }
    fn write(&mut self, b: &[u8]) {
        self.bytes += b.len() as u64;
        if let Some(c) = &mut self.child {
            c.stdin.as_mut().unwrap().write_all(b).unwrap();
        }
    }
    fn finish(mut self) -> (u64, Option<u64>) {
        let z = self.child.take().map(|mut c| {
            drop(c.stdin.take());
            let n = self.out.take().unwrap().join().unwrap();
            assert!(c.wait().unwrap().success());
            n
        });
        (self.bytes, z)
    }
}

// ---------- text ----------

fn print_poly(p: &Poly, names: &[String], out: &mut String) {
    if p.is_zero() {
        out.push('0');
        return;
    }
    let zero = Integer::zero();
    let one = Integer::one();
    for i in 0..p.nterms() {
        let c = &p.coefficients[i];
        let neg = *c < zero;
        let abs = if neg { -c.clone() } else { c.clone() };
        if neg {
            out.push('-');
        } else if i > 0 {
            out.push('+');
        }
        let e = p.exponents(i);
        let constant = e.iter().all(|&x| x == 0);
        let mut first = true;
        if abs != one || constant {
            write!(out, "{abs}").unwrap();
            first = false;
        }
        for (v, &x) in e.iter().enumerate() {
            if x != 0 {
                if !first {
                    out.push('*');
                }
                first = false;
                out.push_str(&names[v]);
                if x > 1 {
                    write!(out, "^{x}").unwrap();
                }
            }
        }
    }
}

fn hash128(s: &[u8]) -> u128 {
    let mut a = std::collections::hash_map::DefaultHasher::new();
    0x51u8.hash(&mut a);
    s.hash(&mut a);
    let mut b = std::collections::hash_map::DefaultHasher::new();
    0xa7u8.hash(&mut b);
    s.hash(&mut b);
    (u128::from(a.finish()) << 64) | u128::from(b.finish())
}

fn sign_count(s: &[u8]) -> u64 {
    s.iter().filter(|&&c| c == b'+' || c == b'-').count() as u64
}

// ---------- factored form ----------

#[derive(Clone)]
struct Part {
    content: Integer,
    factors: Vec<(u32, u32)>,
}
#[derive(Clone)]
struct CInfo {
    num: Part,
    den: Part,
    terms: u64,
    has_whole: bool,
    /// parsed by the big-coefficient path (whole parts = original text)
    big: bool,
}
struct Entry {
    terms: u64,
    whole: bool,
    /// printed text (factors only; whole entries are re-derived on use)
    text: Option<String>,
}

struct State {
    names: Vec<String>,
    vars: Arc<Vec<PolyVariable>>,
    cap: usize,
    index: HashMap<u128, u32>,
    dict: Vec<Entry>,
    cache: HashMap<u128, CInfo>,
    comp_exp: Sink,
    comp_fac: Sink,
    buf: Vec<u8>,
    parse_s: f64,
    factor_s: f64,
    factored_polys: u64,
    whole_polys: u64,
    max_factor_s: f64,
    distinct_coefs: u64,
    distinct_terms: u64,
    timeout: std::time::Duration,
    timeouts: u64,
    worker: Worker,
    part_cache: HashMap<u128, Part>,
    part_hits: u64,
    big_text: usize,
    big_coefs: u64,
    big_fallbacks: u64,
    unchecked: u64,
}

impl State {
    fn parse(&mut self, text: &str) -> Coefficient {
        let t = Instant::now();
        let atom = try_parse!(text).unwrap_or_else(|e| panic!("parse {text:.200}: {e}"));
        let c = atom
            .as_view()
            .try_to_rational_polynomial(&Q, &Z, Some(self.vars.clone()))
            .unwrap_or_else(|e| panic!("rational {text:.200}: {e}"));
        assert_eq!(
            c.numerator.nvars(),
            self.names.len(),
            "coefficient {text:.200} has symbols outside d, n0.."
        );
        self.parse_s += t.elapsed().as_secs_f64();
        c
    }

    fn entry(&mut self, p: &Poly, whole: bool) -> u32 {
        let mut s = String::new();
        print_poly(p, &self.names, &mut s);
        let h = hash128(s.as_bytes());
        if let Some(&i) = self.index.get(&h) {
            return i;
        }
        let i = self.dict.len() as u32;
        self.buf.clear();
        put_poly(&mut self.buf, p);
        let b = std::mem::take(&mut self.buf);
        self.comp_fac.write(&b);
        self.buf = b;
        self.dict.push(Entry {
            terms: p.nterms() as u64,
            whole,
            text: if whole { None } else { Some(s) },
        });
        self.index.insert(h, i);
        i
    }

    fn factor_part(&mut self, p: &Poly) -> Part {
        if p.is_constant() {
            return Part {
                content: if p.is_zero() {
                    Integer::zero()
                } else {
                    p.lcoeff()
                },
                factors: vec![],
            };
        }
        if p.nterms() > self.cap {
            self.whole_polys += 1;
            return Part {
                content: Integer::one(),
                factors: vec![(self.entry(p, true), 1)],
            };
        }
        let mut ptext = String::new();
        print_poly(p, &self.names, &mut ptext);
        let ph = hash128(ptext.as_bytes());
        if let Some(part) = self.part_cache.get(&ph) {
            self.part_hits += 1;
            return part.clone();
        }
        let t = Instant::now();
        // Symbolica's multivariate factorisation is pathologically slow on a
        // few polynomials (T064: > 25 min on one call): each call runs in a
        // worker PROCESS (this binary, --factor-worker) with a time budget;
        // past it the worker is killed (no leaked CPU or memory) and the
        // polynomial is kept whole (counted in full). Results are cached per
        // polynomial, so a slow polynomial costs its budget once per table.
        let factored = match self
            .worker
            .factor(&ptext, self.timeout, &self.vars, self.names.len())
        {
            Some(f) => f,
            None => {
                self.timeouts += 1;
                self.whole_polys += 1;
                self.factor_s += t.elapsed().as_secs_f64();
                eprintln!(
                    "factor timeout ({} terms) after {:.0} s: kept whole",
                    p.nterms(),
                    t.elapsed().as_secs_f64()
                );
                let part = Part {
                    content: Integer::one(),
                    factors: vec![(self.entry(p, true), 1)],
                };
                self.part_cache.insert(ph, part.clone());
                return part;
            }
        };
        let mut content = Integer::one();
        let mut fs = Vec::new();
        let mut check = p.one();
        for (f, e) in factored {
            check = check * &f.pow(e);
            if f.is_constant() {
                for _ in 0..e {
                    content = &content * &f.lcoeff();
                }
            } else {
                fs.push((f, e as u32));
            }
        }
        let dt = t.elapsed().as_secs_f64();
        self.factor_s += dt;
        self.max_factor_s = self.max_factor_s.max(dt);
        if check != *p {
            self.whole_polys += 1;
            let part = Part {
                content: Integer::one(),
                factors: vec![(self.entry(p, true), 1)],
            };
            self.part_cache.insert(ph, part.clone());
            return part;
        }
        self.factored_polys += 1;
        let mut factors: Vec<(u32, u32)> =
            fs.iter().map(|(f, e)| (self.entry(f, false), *e)).collect();
        factors.sort_unstable();
        let part = Part { content, factors };
        self.part_cache.insert(ph, part.clone());
        part
    }

    fn info(&mut self, text: &str) -> CInfo {
        let h = hash128(text.as_bytes());
        if let Some(c) = self.cache.get(&h) {
            return c.clone();
        }
        if text.len() > self.big_text {
            if let Some(info) = self.info_big(text) {
                self.cache.insert(h, info.clone());
                return info;
            }
            self.big_fallbacks += 1;
        }
        let c = self.parse(text);
        self.buf.clear();
        put_poly(&mut self.buf, &c.numerator);
        put_poly(&mut self.buf, &c.denominator);
        let b = std::mem::take(&mut self.buf);
        self.comp_exp.write(&b);
        self.buf = b;
        let num = self.factor_part(&c.numerator);
        let den = self.factor_part(&c.denominator);
        self.buf.clear();
        for part in [&num, &den] {
            put_integer(&mut self.buf, &part.content);
            varint(&mut self.buf, part.factors.len() as u64);
            for &(f, e) in &part.factors {
                varint(&mut self.buf, u64::from(f));
                varint(&mut self.buf, u64::from(e));
            }
        }
        let b = std::mem::take(&mut self.buf);
        self.comp_fac.write(&b);
        self.buf = b;
        let terms = (c.numerator.nterms() + c.denominator.nterms()) as u64;
        self.distinct_coefs += 1;
        self.distinct_terms += terms;
        let has_whole = num
            .factors
            .iter()
            .chain(&den.factors)
            .any(|&(id, _)| self.dict[id as usize].whole);
        let info = CInfo {
            num,
            den,
            terms,
            has_whole,
            big: false,
        };
        self.cache.insert(h, info.clone());
        info
    }

    fn write_parts(&mut self, num: &Part, den: &Part) {
        self.buf.clear();
        for part in [num, den] {
            put_integer(&mut self.buf, &part.content);
            varint(&mut self.buf, part.factors.len() as u64);
            for &(f, e) in &part.factors {
                varint(&mut self.buf, u64::from(f));
                varint(&mut self.buf, u64::from(e));
            }
        }
        let b = std::mem::take(&mut self.buf);
        self.comp_fac.write(&b);
        self.buf = b;
    }

    /// A coefficient whose text exceeds `big_text` bytes: split at the top
    /// level into numerator / denominator text; a part above `big_text` is
    /// never handed to Symbolica (memory): its terms are counted and
    /// compact-encoded from the text and it is one whole dictionary entry; a
    /// smaller part is parsed and factored as usual. `None`: text outside the
    /// expanded num/den shape (caller falls back to Symbolica).
    fn info_big(&mut self, text: &str) -> Option<CInfo> {
        let (nt, dt) = split_num_den(text)?;
        for part in [nt, dt] {
            if part.len() > self.big_text {
                scan_terms(part, &self.names, None)?;
            }
        }
        let (num, n_terms) = self.big_part(nt);
        let (den, d_terms) = self.big_part(dt);
        self.write_parts(&num, &den);
        let terms = n_terms + d_terms;
        self.distinct_coefs += 1;
        self.distinct_terms += terms;
        self.big_coefs += 1;
        let has_whole = num
            .factors
            .iter()
            .chain(&den.factors)
            .any(|&(id, _)| self.dict[id as usize].whole);
        Some(CInfo {
            num,
            den,
            terms,
            has_whole,
            big: true,
        })
    }

    fn big_part(&mut self, s: &str) -> (Part, u64) {
        if s.len() <= self.big_text {
            let c = self.parse(s);
            assert!(
                c.denominator.is_one(),
                "big-path part {s:.200} is not a polynomial"
            );
            self.buf.clear();
            put_poly(&mut self.buf, &c.numerator);
            let b = std::mem::take(&mut self.buf);
            self.comp_exp.write(&b);
            self.buf = b;
            let part = self.factor_part(&c.numerator);
            return (part, c.numerator.nterms() as u64);
        }
        let h = hash128(s.as_bytes());
        let known = self.index.get(&h).copied();
        let mut bytes = Vec::new();
        let terms = scan_terms(s, &self.names, Some(&mut bytes)).expect("checked");
        self.comp_exp.write(&bytes);
        let id = match known {
            Some(id) => id,
            None => {
                self.comp_fac.write(&bytes);
                let id = self.dict.len() as u32;
                self.dict.push(Entry {
                    terms,
                    whole: true,
                    text: None,
                });
                self.index.insert(h, id);
                id
            }
        };
        self.whole_polys += 1;
        (
            Part {
                content: Integer::one(),
                factors: vec![(id, 1)],
            },
            terms,
        )
    }

    /// Factored text of one coefficient (`original` for whole entries).
    fn text(&mut self, info: &CInfo, original: &str) -> String {
        let parsed = if info.has_whole && !info.big {
            Some(self.parse(original))
        } else {
            None
        };
        let split = if info.big {
            split_num_den(original)
        } else {
            None
        };
        let mut out = String::new();
        let part_text = |part: &Part, whole: Whole<'_>, st: &State, out: &mut String| {
            let one = Integer::one();
            let minus = -Integer::one();
            let mut first = true;
            if part.factors.is_empty() || (part.content != one && part.content != minus) {
                write!(out, "{}", part.content).unwrap();
                first = false;
            } else if part.content == minus {
                out.push('-');
            }
            for &(id, e) in &part.factors {
                if !first {
                    out.push('*');
                }
                first = false;
                out.push('(');
                match (&st.dict[id as usize].text, whole) {
                    (Some(t), _) => out.push_str(t),
                    (None, Whole::Poly(p)) => print_poly(p, &st.names, out),
                    (None, Whole::Text(t)) => out.push_str(t),
                    (None, Whole::None) => panic!("whole polynomial without its text"),
                }
                out.push(')');
                if e > 1 {
                    write!(out, "^{e}").unwrap();
                }
            }
        };
        let (whole_num, whole_den) = match (&parsed, &split) {
            (Some(c), _) => (Whole::Poly(&c.numerator), Whole::Poly(&c.denominator)),
            (None, Some((n, d))) => (Whole::Text(n), Whole::Text(d)),
            _ => (Whole::None, Whole::None),
        };
        part_text(&info.num, whole_num, &*self, &mut out);
        let den_one = info.den.factors.is_empty() && info.den.content == Integer::one();
        if !den_one {
            out.push_str("/(");
            part_text(&info.den, whole_den, &*self, &mut out);
            out.push(')');
        }
        out
    }
}

#[derive(Clone, Copy)]
enum Whole<'a> {
    None,
    Poly(&'a Poly),
    Text(&'a str),
}

/// Strip one pair of parentheses enclosing all of `s`.
fn strip_outer(s: &str) -> &str {
    let s = s.trim();
    let b = s.as_bytes();
    if b.first() != Some(&b'(') || b.last() != Some(&b')') {
        return s;
    }
    let mut depth = 0i64;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 && i + 1 != b.len() {
                    return s;
                }
            }
            _ => {}
        }
    }
    &s[1..s.len() - 1]
}

/// Whether `s` has a top-level '+'/'-' after its first character.
fn top_level_sum(s: &str) -> bool {
    let mut depth = 0i64;
    for (i, c) in s.bytes().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'+' | b'-' if depth == 0 && i > 0 => return true,
            _ => {}
        }
    }
    false
}

/// `num/den` of an expanded coefficient text, each a polynomial text (outer
/// parentheses removed); `den` = "1" without a division. `None` otherwise.
fn split_num_den(text: &str) -> Option<(&str, &str)> {
    let t = text.trim();
    let mut depth = 0i64;
    let mut slash = None;
    for (i, c) in t.bytes().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'/' if depth == 0 => {
                if slash.is_some() {
                    return None;
                }
                slash = Some(i);
            }
            _ => {}
        }
    }
    let (n, d) = match slash {
        Some(i) => (&t[..i], &t[i + 1..]),
        None => (t, "1"),
    };
    fn check(x: &str, division: bool) -> Option<&str> {
        let inner = strip_outer(x);
        if inner.len() == x.trim().len() && top_level_sum(inner) && division {
            return None; // `a+b/(c)`: not a num/den split
        }
        if inner.contains('(') {
            return None;
        }
        Some(inner)
    }
    Some((check(n, slash.is_some())?, check(d, slash.is_some())?))
}

/// Terms of an expanded polynomial text (`[+-] [int] (* (int | var[^int]))*`
/// summed); with `out`, the compact encoding (as `put_poly`) is appended.
/// `None`: anything else (parentheses, rationals, unknown symbols).
fn scan_terms(s: &str, names: &[String], mut out: Option<&mut Vec<u8>>) -> Option<u64> {
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut terms = 0u64;
    let mut body = Vec::new();
    let mut exps: Vec<(usize, u64)> = Vec::new();
    let skip = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    let digits = |i: &mut usize| -> Option<(usize, usize)> {
        let st = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        (*i > st).then_some((st, *i))
    };
    skip(&mut i);
    while i < b.len() {
        let mut neg = false;
        if b[i] == b'+' || b[i] == b'-' {
            neg = b[i] == b'-';
            i += 1;
            skip(&mut i);
        }
        let mut coef: Option<(usize, usize)> = None;
        exps.clear();
        loop {
            skip(&mut i);
            if i < b.len() && b[i].is_ascii_digit() {
                if coef.is_some() {
                    return None;
                }
                coef = Some(digits(&mut i)?);
            } else if i < b.len() && (b[i] == b'd' || b[i] == b'n') {
                let v = if b[i] == b'd' {
                    i += 1;
                    0
                } else {
                    i += 1;
                    let (x, y) = digits(&mut i)?;
                    s[x..y].parse::<usize>().ok()? + 1
                };
                if v >= names.len()
                    || (i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_'))
                {
                    return None;
                }
                skip(&mut i);
                let mut e = 1u64;
                if i < b.len() && b[i] == b'^' {
                    i += 1;
                    skip(&mut i);
                    let (x, y) = digits(&mut i)?;
                    e = s[x..y].parse().ok()?;
                }
                match exps.iter_mut().find(|(w, _)| *w == v) {
                    Some(slot) => slot.1 += e,
                    None => exps.push((v, e)),
                }
            } else {
                return None;
            }
            skip(&mut i);
            if i < b.len() && b[i] == b'*' {
                i += 1;
                continue;
            }
            break;
        }
        if i < b.len() && b[i] != b'+' && b[i] != b'-' {
            return None;
        }
        terms += 1;
        if let Some(out) = out.as_deref_mut() {
            let text = match coef {
                Some((x, y)) => &s[x..y],
                None => "1",
            };
            let value: Integer = if neg {
                format!("-{text}").parse().ok()?
            } else {
                text.parse().ok()?
            };
            body.clear();
            put_integer(&mut body, &value);
            exps.sort_unstable();
            varint(&mut body, exps.len() as u64);
            for &(v, e) in &exps {
                varint(&mut body, v as u64);
                varint(&mut body, e);
            }
            out.extend_from_slice(&body);
        }
    }
    if let Some(out) = out {
        // put_poly order: nterms first
        let mut head = Vec::new();
        varint(&mut head, terms);
        out.splice(0..0, head);
    }
    Some(terms)
}

/// (key, coefficient) of every RHS term, as slices of `body` (no escapes);
/// `None` if the array is not of that plain shape.
fn rhs_items(body: &[u8], a: usize, b: usize) -> Option<Vec<(&str, &str)>> {
    let s = std::str::from_utf8(&body[a..b]).ok()?;
    if s.contains('\\') {
        return None;
    }
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let ws = |i: &mut usize| {
        while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    let expect = |i: &mut usize, c: u8| -> Option<()> {
        ws(i);
        (bytes.get(*i) == Some(&c)).then(|| *i += 1)
    };
    let string = |i: &mut usize| -> Option<&str> {
        expect(i, b'"')?;
        let st = *i;
        let end = st + bytes[st..].iter().position(|&c| c == b'"')?;
        *i = end + 1;
        Some(&s[st..end])
    };
    let mut out = Vec::new();
    expect(&mut i, b'[')?;
    ws(&mut i);
    if bytes.get(i) == Some(&b']') {
        return Some(out);
    }
    loop {
        expect(&mut i, b'[')?;
        let key = string(&mut i)?;
        expect(&mut i, b',')?;
        let coef = string(&mut i)?;
        expect(&mut i, b']')?;
        out.push((key, coef));
        ws(&mut i);
        match bytes.get(i) {
            Some(b',') => i += 1,
            Some(b']') => return Some(out),
            _ => return None,
        }
    }
}

/// Byte span of the `"rhs":[...]` array value in a rule record.
fn rhs_span(line: &[u8]) -> Option<(usize, usize)> {
    let key = b"\"rhs\"";
    let mut start = line.windows(key.len()).position(|w| w == key)? + key.len();
    while start < line.len() && (line[start].is_ascii_whitespace() || line[start] == b':') {
        start += 1;
    }
    let mut depth = 0i64;
    let mut in_str = false;
    let mut i = start;
    while i < line.len() {
        let c = line[i];
        if in_str {
            if c == b'\\' {
                i += 1;
            } else if c == b'"' {
                in_str = false;
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == b'[' {
            depth += 1;
        } else if c == b']' {
            depth -= 1;
            if depth == 0 {
                return Some((start, i + 1));
            }
        }
        i += 1;
    }
    None
}

/// The `type` of a record whose first key is `"type"` (compact or spaced JSON).
fn record_type(body: &[u8]) -> &[u8] {
    let skip = |b: &[u8], mut i: usize| {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        i
    };
    let mut i = skip(body, 0);
    if body.get(i) != Some(&b'{') {
        return b"";
    }
    i = skip(body, i + 1);
    if !body[i..].starts_with(b"\"type\"") {
        return b"";
    }
    i = skip(body, i + 6);
    if body.get(i) != Some(&b':') {
        return b"";
    }
    i = skip(body, i + 1);
    if body.get(i) != Some(&b'"') {
        return b"";
    }
    let end = body[i + 1..]
        .iter()
        .position(|&c| c == b'"')
        .map_or(i + 1, |k| i + 1 + k);
    &body[i + 1..end]
}

// ---------- factorisation worker process ----------

/// A child process of this binary (`--factor-worker NVARS`): one polynomial
/// text per line on stdin, answers `F <exp> <factor text>` lines then `END`.
#[derive(Default)]
struct Worker {
    child: Option<(Child, std::process::ChildStdin, Receiver<Option<String>>)>,
    spawned: u64,
    killed: u64,
}
impl Worker {
    fn factor(
        &mut self,
        text: &str,
        timeout: std::time::Duration,
        vars: &Arc<Vec<PolyVariable>>,
        nvars: usize,
    ) -> Option<Vec<(Poly, usize)>> {
        if self.child.is_none() {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args(["--factor-worker", &(nvars - 1).to_string()])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("factor worker");
            let stdin = child.stdin.take().unwrap();
            let stdout = child.stdout.take().unwrap();
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                for l in BufReader::new(stdout).lines() {
                    if tx.send(l.ok()).is_err() {
                        break;
                    }
                }
                let _ = tx.send(None);
            });
            self.child = Some((child, stdin, rx));
            self.spawned += 1;
        }
        let deadline = Instant::now() + timeout;
        let (_, stdin, rx) = self.child.as_mut().unwrap();
        let mut ok = stdin.write_all(text.as_bytes()).is_ok()
            && stdin.write_all(b"\n").is_ok()
            && stdin.flush().is_ok();
        let mut out = Vec::new();
        while ok {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(Some(l)) if l == "END" => return Some(out),
                Ok(Some(l)) => {
                    let mut it = l.splitn(3, ' ');
                    let (_, e, f) = (it.next(), it.next().unwrap(), it.next().unwrap());
                    let c = try_parse!(f)
                        .unwrap()
                        .as_view()
                        .try_to_rational_polynomial(&Q, &Z, Some(vars.clone()))
                        .unwrap();
                    assert!(c.denominator.is_one());
                    out.push((c.numerator, e.parse().unwrap()));
                }
                _ => ok = false,
            }
        }
        let (mut child, _, _) = self.child.take().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.killed += 1;
        None
    }
}

fn factor_worker(arity: usize) {
    let mut names = vec!["d".to_string()];
    names.extend((0..arity).map(|k| format!("n{k}")));
    let vars: Arc<Vec<PolyVariable>> = Arc::new(
        names
            .iter()
            .map(|s| PolyVariable::Symbol(symbol!(s.as_str())))
            .collect(),
    );
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut line = String::new();
    let mut lock = stdin.lock();
    loop {
        line.clear();
        if lock.read_line(&mut line).unwrap() == 0 {
            return;
        }
        let c = try_parse!(line.trim())
            .unwrap()
            .as_view()
            .try_to_rational_polynomial(&Q, &Z, Some(vars.clone()))
            .unwrap();
        let mut s = String::new();
        for (f, e) in c.numerator.factor() {
            s.clear();
            print_poly(&f, &names, &mut s);
            writeln!(out, "F {e} {s}").unwrap();
        }
        writeln!(out, "END").unwrap();
        out.flush().unwrap();
    }
}

fn vmhwm() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
        * 1024
}

/// Value of a top-level scalar field `"key": value` of a record (spaced or compact JSON).
fn field(body: &[u8], key: &str) -> String {
    let k = format!("\"{key}\"");
    let k = k.as_bytes();
    let Some(p) = body.windows(k.len()).position(|w| w == k) else {
        return String::new();
    };
    let mut i = p + k.len();
    while i < body.len() && (body[i].is_ascii_whitespace() || body[i] == b':') {
        i += 1;
    }
    let rest = &body[i..];
    let end = rest
        .iter()
        .position(|&c| c == b',' || c == b'}')
        .unwrap_or(rest.len());
    String::from_utf8_lossy(&rest[..end])
        .trim()
        .trim_matches('"')
        .to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--factor-worker") {
        factor_worker(args[2].parse().unwrap());
        return;
    }
    let input = &args[1];
    let report = &args[2];
    let mut label = std::path::Path::new(input)
        .parent()
        .and_then(|p| p.file_name())
        .map_or("table".into(), |s| s.to_string_lossy().to_string());
    let mut write: Option<String> = None;
    let mut zstd = true;
    // --only FILE: lines `chart:index`; only these rules are measured
    // (others are passed through, counted as bytes only)
    let mut only: Option<std::collections::HashSet<String>> = None;
    // --budget-s S: stop after the first rule finishing past S seconds
    // (report `complete: false` and the input bytes read)
    let mut budget: Option<f64> = None;
    let mut bytes_read = 0u64;
    let mut complete = true;
    let mut skipped_rules = 0u64;
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--label" => {
                label = args[i + 1].clone();
                i += 1;
            }
            "--write" => {
                write = Some(args[i + 1].clone());
                i += 1;
            }
            "--no-zstd" => zstd = false,
            "--only" => {
                only = Some(
                    std::fs::read_to_string(&args[i + 1])
                        .unwrap()
                        .lines()
                        .filter(|l| !l.trim().is_empty())
                        .map(|l| l.trim().to_string())
                        .collect(),
                );
                i += 1;
            }
            "--budget-s" => {
                budget = Some(args[i + 1].parse().unwrap());
                i += 1;
            }
            o => panic!("unknown option {o}"),
        }
        i += 1;
    }
    let cap: usize = std::env::var("COEF_FORM_FACTOR_CAP")
        .ok()
        .map_or(FACTOR_TERM_CAP, |s| s.parse().unwrap());
    let started = Instant::now();
    let file = std::fs::File::open(input).unwrap();
    let in_bytes_total = file.metadata().unwrap().len();
    let mut reader = BufReader::with_capacity(1 << 24, file);
    let mut writer = write
        .as_ref()
        .map(|w| BufWriter::with_capacity(1 << 24, std::fs::File::create(w).unwrap()));
    let mut in_text = Sink::new(zstd);
    let mut out_text = Sink::new(zstd);
    let mut st: Option<State> = None;
    let mut line = Vec::new();
    let (mut rules, mut occurrences) = (0u64, 0u64);
    let bands = [1_000_000u64, 10_000_000];
    // measure index: audit, expanded, factored, text
    let mut over = [[0u64; 2]; 4];
    let mut totals = [0u64; 4];
    let mut largest: Vec<serde_json::Value> = Vec::new();
    let mut max_rule = [0u64; 4];
    let mut check_s = 0f64;
    let mut write_s = 0f64;
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line).unwrap();
        if n == 0 {
            break;
        }
        if budget.is_some_and(|b| started.elapsed().as_secs_f64() > b) {
            complete = false;
            break;
        }
        bytes_read += n as u64;
        in_text.write(&line);
        let body: &[u8] = line.strip_suffix(b"\n").unwrap_or(&line);
        let kind = record_type(body);
        if kind == b"header" {
            let h: serde_json::Value = serde_json::from_slice(body).unwrap();
            let arity = h["denominators"]
                .as_u64()
                .or_else(|| h["arity"].as_u64())
                .expect("header denominators") as usize;
            let mut names = vec!["d".to_string()];
            names.extend((0..arity).map(|k| format!("n{k}")));
            let vars: Arc<Vec<PolyVariable>> = Arc::new(
                names
                    .iter()
                    .map(|s| PolyVariable::Symbol(symbol!(s.as_str())))
                    .collect(),
            );
            st = Some(State {
                names,
                vars,
                cap,
                index: HashMap::new(),
                dict: Vec::new(),
                cache: HashMap::new(),
                comp_exp: Sink::new(zstd),
                comp_fac: Sink::new(zstd),
                buf: Vec::new(),
                parse_s: 0.0,
                factor_s: 0.0,
                factored_polys: 0,
                whole_polys: 0,
                max_factor_s: 0.0,
                distinct_coefs: 0,
                distinct_terms: 0,
                timeout: std::time::Duration::from_secs_f64(
                    std::env::var("COEF_FORM_FACTOR_TIMEOUT")
                        .ok()
                        .map_or(60.0, |s| s.parse().unwrap()),
                ),
                timeouts: 0,
                worker: Worker::default(),
                part_cache: HashMap::new(),
                part_hits: 0,
                big_text: std::env::var("COEF_FORM_BIG_TEXT")
                    .ok()
                    .map_or(1 << 20, |s| s.parse().unwrap()),
                big_coefs: 0,
                big_fallbacks: 0,
                unchecked: 0,
            });
            // `"coef_form"` right after the type value (type stays the first key)
            let at = line
                .windows(8)
                .position(|w| w == b"\"header\"")
                .expect("header type")
                + 8;
            let mut out = Vec::with_capacity(line.len() + 40);
            out.extend_from_slice(&line[..at]);
            write!(out, ",\"coef_form\":\"{TABLE_COEF_FORM}\"").unwrap();
            out.extend_from_slice(&line[at..]);
            out_text.write(&out);
            if let Some(w) = &mut writer {
                w.write_all(&out).unwrap();
            }
            continue;
        }
        if kind != b"rule" {
            out_text.write(&line);
            if let Some(w) = &mut writer {
                w.write_all(&line).unwrap();
            }
            continue;
        }
        let s = st.as_mut().expect("rule before header");
        if let Some(o) = &only {
            if !o.contains(&format!(
                "{}:{}",
                field(body, "chart"),
                field(body, "index")
            )) {
                skipped_rules += 1;
                out_text.write(&line);
                if let Some(w) = &mut writer {
                    w.write_all(&line).unwrap();
                }
                continue;
            }
        }
        rules += 1;
        let (a, b) = rhs_span(body).expect("rhs");
        // (key, coefficient) byte ranges; serde only for escaped strings
        let items: Vec<(String, String)>;
        let spans: Vec<(&str, &str)> = match rhs_items(body, a, b) {
            Some(v) => v,
            None => {
                items = serde_json::from_slice(&body[a..b]).unwrap();
                items
                    .iter()
                    .map(|(k, c)| (k.as_str(), c.as_str()))
                    .collect()
            }
        };
        let audit = sign_count(&body[a..]);
        let mut expanded = 0u64;
        let mut refs = 0u64;
        let mut ids: Vec<u32> = Vec::new();
        // the factored record, streamed piece by piece (no copy of the record)
        let mut out_len = 0usize;
        let mut text_est = sign_count(&body[b..]);
        let mut emit = |piece: &[u8], sink: &mut Sink, w: &mut Option<BufWriter<std::fs::File>>| {
            out_len += piece.len();
            sink.write(piece);
            if let Some(w) = w {
                w.write_all(piece).unwrap();
            }
        };
        emit(&body[..a], &mut out_text, &mut writer);
        emit(b"[", &mut out_text, &mut writer);
        for (k, &(key, coef)) in spans.iter().enumerate() {
            occurrences += 1;
            let info = s.info(coef);
            expanded += info.terms;
            refs += 2 + (info.num.factors.len() + info.den.factors.len()) as u64;
            ids.extend(
                info.num
                    .factors
                    .iter()
                    .chain(&info.den.factors)
                    .map(|x| x.0),
            );
            let t = Instant::now();
            let text = s.text(&info, coef);
            write_s += t.elapsed().as_secs_f64();
            if writer.is_some() {
                if info.big {
                    s.unchecked += 1;
                } else {
                    // exact identity of the written coefficient
                    let t = Instant::now();
                    let x = s.parse(coef);
                    let y = s.parse(&text);
                    assert!(
                        x.numerator == y.numerator && x.denominator == y.denominator,
                        "factored text differs: {coef:.200} vs {text:.200}"
                    );
                    check_s += t.elapsed().as_secs_f64();
                }
            }
            text_est += sign_count(key.as_bytes()) + sign_count(text.as_bytes());
            let piece = format!(
                "{}[{},{}]",
                if k == 0 { "" } else { "," },
                serde_json::to_string(key).unwrap(),
                serde_json::to_string(&text).unwrap()
            );
            emit(piece.as_bytes(), &mut out_text, &mut writer);
        }
        emit(b"]", &mut out_text, &mut writer);
        emit(&body[b..], &mut out_text, &mut writer);
        emit(b"\n", &mut out_text, &mut writer);
        let out_len = out_len - 1;
        ids.sort_unstable();
        ids.dedup();
        let whole: u64 = ids
            .iter()
            .filter(|&&i| s.dict[i as usize].whole)
            .map(|&i| s.dict[i as usize].terms)
            .sum();
        let factored: u64 = ids.iter().map(|&i| s.dict[i as usize].terms).sum::<u64>() + refs;
        let m = [audit, expanded, factored, text_est];
        for k in 0..4 {
            totals[k] += m[k];
            max_rule[k] = max_rule[k].max(m[k]);
            for (j, &band) in bands.iter().enumerate() {
                if m[k] > band {
                    over[k][j] += 1;
                }
            }
        }
        if audit > 1_000_000 || expanded > 1_000_000 {
            largest.push(serde_json::json!({
                "chart": field(body, "chart"), "index": field(body, "index"), "audit": audit, "expanded": expanded,
                "factored": factored, "factored_whole": whole, "text": text_est,
                "record_bytes": body.len(), "factored_record_bytes": out_len,
            }));
        }
    }
    if let Some(mut w) = writer {
        w.flush().unwrap();
    }
    let s = st.expect("no header");
    let (in_b, in_z) = in_text.finish();
    let (out_b, out_z) = out_text.finish();
    let (ce_b, ce_z) = s.comp_exp.finish();
    let (cf_b, cf_z) = s.comp_fac.finish();
    let names = ["audit", "expanded", "factored", "text"];
    let mut sizes = serde_json::Map::new();
    for k in 0..4 {
        sizes.insert(
            names[k].into(),
            serde_json::json!({"total": totals[k], "max_rule": max_rule[k], "over_1M": over[k][0], "over_10M": over[k][1]}),
        );
    }
    let rec = serde_json::json!({
        "label": label, "table": input, "cap": cap, "coef_form": TABLE_COEF_FORM,
        "bytes": in_bytes_total, "text_bytes": in_b, "text_zstd3": in_z,
        "factored_text_bytes": out_b, "factored_text_zstd3": out_z,
        "compact_expanded_bytes": ce_b, "compact_expanded_zstd3": ce_z,
        "compact_factored_bytes": cf_b, "compact_factored_zstd3": cf_z,
        "rules": rules, "rhs_occurrences": occurrences,
        "distinct_coefficients": s.distinct_coefs, "distinct_coefficient_terms": s.distinct_terms,
        "dict_entries": s.dict.len(), "dict_terms": s.dict.iter().map(|e| e.terms).sum::<u64>(),
        "whole_entries": s.dict.iter().filter(|e| e.whole).count(),
        "whole_terms": s.dict.iter().filter(|e| e.whole).map(|e| e.terms).sum::<u64>(),
        "factored_polys": s.factored_polys, "whole_polys": s.whole_polys,
        "complete": complete, "bytes_read": bytes_read, "skipped_rules": skipped_rules,
        "only": only.as_ref().map(|o| o.len()), "budget_s": budget,
        "factor_cache_hits": s.part_hits, "factor_workers_spawned": s.worker.spawned, "factor_workers_killed": s.worker.killed,
        "factor_timeouts": s.timeouts, "factor_timeout_s": s.timeout.as_secs_f64(),
        "big_text_bytes": s.big_text, "big_coefficients": s.big_coefs, "big_fallbacks": s.big_fallbacks,
        "unchecked_written_big_coefficients": s.unchecked,
        "sizes": sizes, "rules_over_1M": largest,
        "parse_s": s.parse_s, "factor_and_multiply_back_s": s.factor_s, "max_poly_factor_s": s.max_factor_s,
        "factored_text_s": write_s, "text_check_s": check_s, "written": write,
        "seconds": started.elapsed().as_secs_f64(), "vmhwm_bytes": vmhwm(),
    });
    let line = rec.to_string();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(report)
        .unwrap();
    writeln!(f, "{line}").unwrap();
    println!("{line}");
}
