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
use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
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
/// Abandoned (timed-out) factorisation workers after which no further
/// factorisation is attempted in this run.
const MAX_ABANDONED: usize = 16;

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
    abandoned: usize,
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
        let t = Instant::now();
        // Symbolica's multivariate factorisation is pathologically slow on a
        // few polynomials (T064: > 25 min on one call): each call runs in a
        // worker thread with a time budget; past it the polynomial is kept
        // whole (counted in full) and the worker is abandoned. After
        // `MAX_ABANDONED` abandoned workers no further call is attempted.
        if self.abandoned >= MAX_ABANDONED {
            self.whole_polys += 1;
            self.timeouts += 1;
            return Part {
                content: Integer::one(),
                factors: vec![(self.entry(p, true), 1)],
            };
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let (tid_tx, tid_rx) = std::sync::mpsc::channel();
        let q = p.clone();
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let tid = std::fs::read_link("/proc/thread-self")
                    .ok()
                    .and_then(|l| l.file_name().map(|n| n.to_string_lossy().to_string()));
                let _ = tid_tx.send(tid);
                let _ = tx.send(q.factor());
            })
            .expect("factor worker");
        let factored = match rx.recv_timeout(self.timeout) {
            Ok(f) => f,
            Err(_) => {
                // the abandoned worker keeps running until the process ends:
                // lowest priority, so it no longer competes with this run
                if let Ok(Some(tid)) = tid_rx.recv() {
                    let _ = Command::new("renice")
                        .args(["-n", "19", "-p", &tid])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
                self.abandoned += 1;
                self.timeouts += 1;
                self.whole_polys += 1;
                self.factor_s += t.elapsed().as_secs_f64();
                eprintln!(
                    "factor timeout ({} terms) after {:.0} s: kept whole",
                    p.nterms(),
                    t.elapsed().as_secs_f64()
                );
                return Part {
                    content: Integer::one(),
                    factors: vec![(self.entry(p, true), 1)],
                };
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
            return Part {
                content: Integer::one(),
                factors: vec![(self.entry(p, true), 1)],
            };
        }
        self.factored_polys += 1;
        let mut factors: Vec<(u32, u32)> = fs.iter().map(|(f, e)| (self.entry(f, false), *e)).collect();
        factors.sort_unstable();
        Part { content, factors }
    }

    fn info(&mut self, text: &str) -> CInfo {
        let h = hash128(text.as_bytes());
        if let Some(c) = self.cache.get(&h) {
            return c.clone();
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
        };
        self.cache.insert(h, info.clone());
        info
    }

    /// Factored text of one coefficient (`original` for whole entries).
    fn text(&mut self, info: &CInfo, original: &str) -> String {
        let parsed = if info.has_whole {
            Some(self.parse(original))
        } else {
            None
        };
        let mut out = String::new();
        let part_text = |part: &Part, whole: Option<&Poly>, st: &State, out: &mut String| {
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
                match &st.dict[id as usize].text {
                    Some(t) => out.push_str(t),
                    None => print_poly(whole.expect("whole polynomial"), &st.names, out),
                }
                out.push(')');
                if e > 1 {
                    write!(out, "^{e}").unwrap();
                }
            }
        };
        part_text(
            &info.num,
            parsed.as_ref().map(|c| &c.numerator),
            self,
            &mut out,
        );
        let den_one = info.den.factors.is_empty() && info.den.content == Integer::one();
        if !den_one {
            out.push_str("/(");
            part_text(
                &info.den,
                parsed.as_ref().map(|c| &c.denominator),
                self,
                &mut out,
            );
            out.push(')');
        }
        out
    }
}

/// Byte span of the `"rhs":[...]` array value in a rule record.
fn rhs_span(line: &[u8]) -> Option<(usize, usize)> {
    let key = b"\"rhs\":";
    let start = line.windows(key.len()).position(|w| w == key)? + key.len();
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

fn vmhwm() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
        * 1024
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = &args[1];
    let report = &args[2];
    let mut label = std::path::Path::new(input)
        .parent()
        .and_then(|p| p.file_name())
        .map_or("table".into(), |s| s.to_string_lossy().to_string());
    let mut write: Option<String> = None;
    let mut zstd = true;
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
        in_text.write(&line);
        let body: &[u8] = line.strip_suffix(b"\n").unwrap_or(&line);
        if body.starts_with(b"{\"type\":\"header\"") {
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
                abandoned: 0,
            });
            let tag = b"{\"type\":\"header\",";
            let mut out = Vec::with_capacity(line.len() + 40);
            out.extend_from_slice(tag);
            write!(out, "\"coef_form\":\"{TABLE_COEF_FORM}\",").unwrap();
            out.extend_from_slice(&line[tag.len()..]);
            out_text.write(&out);
            if let Some(w) = &mut writer {
                w.write_all(&out).unwrap();
            }
            continue;
        }
        if !body.starts_with(b"{\"type\":\"rule\"") {
            out_text.write(&line);
            if let Some(w) = &mut writer {
                w.write_all(&line).unwrap();
            }
            continue;
        }
        let s = st.as_mut().expect("rule before header");
        rules += 1;
        let (a, b) = rhs_span(body).expect("rhs");
        let rhs: Vec<(String, String)> = serde_json::from_slice(&body[a..b]).unwrap();
        let audit = sign_count(&body[a..]);
        let mut expanded = 0u64;
        let mut refs = 0u64;
        let mut ids: Vec<u32> = Vec::new();
        let mut new_rhs: Vec<(String, String)> = Vec::with_capacity(rhs.len());
        for (key, coef) in &rhs {
            occurrences += 1;
            let info = s.info(coef);
            expanded += info.terms;
            refs += 2 + (info.num.factors.len() + info.den.factors.len()) as u64;
            ids.extend(info.num.factors.iter().chain(&info.den.factors).map(|x| x.0));
            let t = Instant::now();
            let text = s.text(&info, coef);
            write_s += t.elapsed().as_secs_f64();
            if writer.is_some() {
                // exact identity of the written coefficient
                let t = Instant::now();
                let a = s.parse(coef);
                let b = s.parse(&text);
                assert!(
                    a.numerator == b.numerator && a.denominator == b.denominator,
                    "factored text differs: {coef:.200} vs {text:.200}"
                );
                check_s += t.elapsed().as_secs_f64();
            }
            new_rhs.push((key.clone(), text));
        }
        ids.sort_unstable();
        ids.dedup();
        let whole: u64 = ids
            .iter()
            .filter(|&&i| s.dict[i as usize].whole)
            .map(|&i| s.dict[i as usize].terms)
            .sum();
        let factored: u64 = ids.iter().map(|&i| s.dict[i as usize].terms).sum::<u64>() + refs;
        let rhs_text = serde_json::to_vec(&new_rhs).unwrap();
        let mut out = Vec::with_capacity(a + rhs_text.len() + (body.len() - b) + 1);
        out.extend_from_slice(&body[..a]);
        out.extend_from_slice(&rhs_text);
        out.extend_from_slice(&body[b..]);
        let text_est = sign_count(&out[a..]);
        out.push(b'\n');
        out_text.write(&out);
        if let Some(w) = &mut writer {
            w.write_all(&out).unwrap();
        }
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
            let field = |key: &[u8]| -> String {
                body.windows(key.len())
                    .position(|w| w == key)
                    .map(|p| {
                        let rest = &body[p + key.len()..];
                        let end = rest
                            .iter()
                            .position(|&c| c == b',' || c == b'}')
                            .unwrap_or(rest.len());
                        String::from_utf8_lossy(&rest[..end]).trim_matches('"').to_string()
                    })
                    .unwrap_or_default()
            };
            largest.push(serde_json::json!({
                "chart": field(b"\"chart\":"), "index": field(b"\"index\":"), "audit": audit, "expanded": expanded,
                "factored": factored, "factored_whole": whole, "text": text_est,
                "record_bytes": body.len(), "factored_record_bytes": out.len() - 1,
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
        "factor_timeouts": s.timeouts, "factor_timeout_s": s.timeout.as_secs_f64(),
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
