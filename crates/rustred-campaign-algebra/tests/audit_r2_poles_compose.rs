//! AUDIT r2-poles (independent auditor, question 2): an INDEPENDENT exact route for the composed pole profile of box
//! keys on a node's terminal set, to reproduce lane epsdepth's counts on a seeded sample of keys.
//!
//! Independence: nothing of lanes epsdepth / regbasis / poles / import is used. This file
//! - reads the closure statement (`statement.jsonl`, `terminals.json`) itself,
//! - locates every serving record by the sha256 of its table line (all `.jsonl` files named by the import spec, or under
//!   its `tables_dir`s), parses it with serde_json,
//! - matches the record's target pattern at the key to get the rule point (own affine parser) and evaluates the rhs key
//!   patterns there (own affine evaluator), keeps the TOP-SECTOR terms (record chart: line slots >= 1, others <= 0),
//! - reads each coefficient with Symbolica (`try_parse!` + `try_to_rational_polynomial`), puts the rule point in
//!   (`replace_multiple` over Z), rebuilds it as a polynomial in d alone and composes exactly in Q(d)
//!   (Symbolica `RationalPolynomial` over Z, gcd-normalised) bottom-up over the cone of the sampled keys,
//! - takes the order at d0 with its OWN synthetic division by (d - d0) over the integers (d0 integer), and the leading
//!   Laurent coefficient num_rest(d0) / den_rest(d0) by Horner.
//!
//! Keys served through a symmetry credit (statement server kind `credit`): I(k) = I(g k) with g an element of the node's
//! symmetry group (data: the tables' `symmetry_group` line; pure slot permutations without flips) whose image g k is a
//! statement key served by the SAME record (a one-term edge k -> g k). A credited key without such an image stays
//! undecided (reported).
//!
//! Run (ignored; Symbolica licence from `source WS/ops/env.sh`):
//!   AUDIT_IMPORT_SPEC=.. AUDIT_STATEMENT=<dir> AUDIT_N=2500 AUDIT_SEED=20261115 AUDIT_OUT=<dir>
//!   [AUDIT_ROWS=<epsdepth rows.jsonl to compare>] [AUDIT_D0=4]
//!   cargo test --release -p rustred-campaign-algebra --test audit_r2_poles_compose -- --ignored --nocapture

#![allow(clippy::needless_range_loop)] // matrix products written with explicit indices

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustred_campaign_core::replay_base::sha256::sha256_hex;
use serde_json::{Value, json};
use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::integer::{Integer, IntegerRing, Z};
use symbolica::domains::rational::Q;
use symbolica::domains::rational_polynomial::{FromNumeratorAndDenominator, RationalPolynomial};
use symbolica::poly::PolyVariable;
use symbolica::poly::polynomial::MultivariatePolynomial;
use symbolica::prelude::AtomCore;
use symbolica::try_parse;

type ZPoly = MultivariatePolynomial<IntegerRing, u16>;
type RF = RationalPolynomial<IntegerRing, u16>;
/// A record parsed once: rhs patterns and coefficient (numerator, denominator) over Z in n0.., d (None = zero).
type Parsed = Vec<(Vec<Affine>, Option<(ZPoly, ZPoly)>)>;
/// A regular-basis record read for the comparison: (path, M, M^-1, R).
type MapRec = (
    String,
    Vec<Vec<Option<RF>>>,
    Vec<Vec<Option<RF>>>,
    Vec<Vec<i64>>,
);

// ------------------------------------------------------------------------------------------------ own small pieces

/// splitmix64 (own PRNG for the seeded sample).
struct Mix(u64);
impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// An affine slot expression `c0 + sum c_a n_a` (own parser: integers, `n<a>`, `k*n<a>`, `+`, `-`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Affine {
    c0: i64,
    terms: Vec<(usize, i64)>,
}

fn parse_affine(s: &str) -> Result<Affine, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return Err("empty slot".into());
    }
    let mut out = Affine {
        c0: 0,
        terms: vec![],
    };
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let mut sign = 1i64;
        while i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            if b[i] == b'-' {
                sign = -sign;
            }
            i += 1;
        }
        let st = i;
        while i < b.len() && b[i] != b'+' && b[i] != b'-' {
            i += 1;
        }
        let tok = &s[st..i];
        if tok.is_empty() {
            return Err(format!("bad slot {s:?}"));
        }
        let (k, var) = match tok.split_once('*') {
            Some((k, v)) => (k.parse::<i64>().map_err(|_| format!("bad slot {s:?}"))?, v),
            None if tok.starts_with('n') => (1, tok),
            None => {
                out.c0 += sign * tok.parse::<i64>().map_err(|_| format!("bad slot {s:?}"))?;
                continue;
            }
        };
        let a: usize = var
            .strip_prefix('n')
            .and_then(|x| x.parse().ok())
            .ok_or_else(|| format!("bad slot {s:?}"))?;
        out.terms.push((a, sign * k));
    }
    Ok(out)
}

fn parse_pattern(p: &str) -> Result<Vec<Affine>, String> {
    let inner = p
        .trim()
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or_else(|| format!("not a key pattern: {p:?}"))?;
    inner.split(',').map(parse_affine).collect()
}

/// The rule point from the target pattern at `key` (`n_a + o` at slot j: pt[a] = key[j] - o; constants must match).
fn rule_point(target: &[Affine], key: &[i64]) -> Result<Vec<Option<i64>>, String> {
    if target.len() != key.len() {
        return Err("target arity".into());
    }
    let mut pt: Vec<Option<i64>> = vec![None; key.len()];
    for (j, a) in target.iter().enumerate() {
        match a.terms.as_slice() {
            [] => {
                if a.c0 != key[j] {
                    return Err(format!("constant slot {j} differs"));
                }
            }
            [(v, 1)] => {
                let val = key[j] - a.c0;
                match pt[*v] {
                    Some(x) if x != val => {
                        return Err(format!("inconsistent target variable n{v}"));
                    }
                    _ => pt[*v] = Some(val),
                }
            }
            _ => return Err(format!("unsupported target slot {a:?}")),
        }
    }
    Ok(pt)
}

fn eval_pattern(p: &[Affine], pt: &[Option<i64>]) -> Result<Vec<i64>, String> {
    p.iter()
        .map(|a| {
            let mut v = a.c0;
            for &(x, k) in &a.terms {
                v += k * pt[x].ok_or_else(|| format!("rhs uses n{x}, not bound by the target"))?;
            }
            Ok(v)
        })
        .collect()
}

/// Own exact order at an integer d0 and leading coefficient: dense integer coefficients (index = power of d).
fn dense(p: &ZPoly, deg_var: usize) -> Vec<Integer> {
    let mut v: Vec<Integer> = vec![];
    for t in p.into_iter() {
        let e = t.exponents[deg_var] as usize;
        if v.len() <= e {
            v.resize(e + 1, Integer::from(0i64));
        }
        v[e] = &v[e] + t.coefficient;
    }
    v
}

/// Multiplicity of the root d0 (synthetic division) and the quotient's value at d0. `None` for the zero polynomial.
fn mult_and_rest(mut a: Vec<Integer>, d0: i64) -> Option<(u32, Integer)> {
    while a.last().is_some_and(|x| x.is_zero()) {
        a.pop();
    }
    if a.is_empty() {
        return None;
    }
    let x = Integer::from(d0);
    let mut m = 0u32;
    loop {
        // Horner: value at d0, and the quotient by (d - d0)
        let n = a.len();
        let mut q: Vec<Integer> = vec![Integer::from(0i64); n.saturating_sub(1)];
        let mut acc = Integer::from(0i64);
        for i in (0..n).rev() {
            acc = &(&acc * &x) + &a[i];
            if i > 0 {
                q[i - 1] = acc.clone();
            }
        }
        if !acc.is_zero() {
            return Some((m, acc));
        }
        m += 1;
        a = q;
    }
}

/// (order, leading coefficient "n/d") of a non-zero rational function in d (variable index `dv`).
fn order_and_leading(r: &RF, dv: usize, d0: i64) -> Option<(i64, String)> {
    let (mn, vn) = mult_and_rest(dense(&r.numerator, dv), d0)?;
    let (md, vd) = mult_and_rest(dense(&r.denominator, dv), d0).expect("non-zero denominator");
    let lead = symbolica::domains::rational::Rational::from((vn, vd));
    Some((i64::from(mn) - i64::from(md), lead.to_string()))
}

// ------------------------------------------------------------------------------------------------ inputs

fn collect_paths(v: &Value, out: &mut Vec<PathBuf>) {
    match v {
        Value::String(s) => {
            let p = Path::new(s);
            if s.ends_with(".jsonl") && p.is_file() {
                out.push(p.to_path_buf());
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_paths(x, out)),
        Value::Object(m) => {
            for (k, x) in m {
                if k == "tables_dir"
                    && let Some(d) = x.as_str()
                {
                    walk(Path::new(d), out);
                } else if k != "evidence" {
                    collect_paths(x, out);
                }
            }
        }
        _ => {}
    }
}

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(d) {
        let mut es: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        es.sort();
        for p in es {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "jsonl") {
                out.push(p);
            }
        }
    }
}

fn key_text_parse(s: &str) -> Vec<i64> {
    s.trim()
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .expect("key text")
        .split(',')
        .map(|x| x.trim().parse::<i64>().expect("key entry"))
        .collect()
}

fn vars(arity: usize) -> Arc<Vec<PolyVariable>> {
    let mut v = vec![];
    for name in (0..arity)
        .map(|i| format!("n{i}"))
        .chain(std::iter::once("d".to_string()))
    {
        let ns = NamespacedSymbol::try_parse(format!("rustred::{name}")).expect("symbol");
        v.push(PolyVariable::Symbol(
            SymbolBuilder::new(ns).build().expect("symbol"),
        ));
    }
    Arc::new(v)
}

enum Served {
    Terminal(usize),
    Record(String),
    /// Served through a symmetry credit by the record with this sha256 at an image g k.
    Credit(String),
    Other(String),
}

/// Image of `k` under a group element's `action` (slot i -> (slot j, flip)); `inverse` reads the map the other way.
/// `None` when a non-zero slot has no pure image or a flip is involved (no one-term identity).
fn apply_action(action: &[Value], k: &[i64], inverse: bool) -> Option<Vec<i64>> {
    let mut out: Vec<Option<i64>> = vec![None; k.len()];
    for (i, a) in action.iter().enumerate() {
        let Some(a) = a.as_array() else {
            if k[i] != 0 && !inverse {
                return None;
            }
            continue;
        };
        let j = a[0].as_u64()? as usize;
        if a[1].as_bool() != Some(false) {
            return None;
        }
        if inverse {
            out[i] = Some(k[j]);
        } else {
            out[j] = Some(k[i]);
        }
    }
    let mut v = vec![];
    for (i, x) in out.into_iter().enumerate() {
        match x {
            Some(x) => v.push(x),
            None if inverse => {
                // slot i receives nothing: only allowed when the slot that would map to it is zero (checked by the
                // forward direction of the inverse element; here require zero by construction)
                v.push(0);
            }
            None => {
                if k[i] != 0 {
                    return None;
                }
                v.push(0)
            }
        }
    }
    if inverse {
        // every non-zero slot of k must be read by some image slot
        let read: HashSet<usize> = action
            .iter()
            .filter_map(|a| a.as_array().and_then(|a| a[0].as_u64()).map(|j| j as usize))
            .collect();
        if k.iter()
            .enumerate()
            .any(|(j, &x)| x != 0 && !read.contains(&j))
        {
            return None;
        }
    }
    Some(v)
}

struct Edge {
    child: Vec<i64>,
    coef: RF,
}

fn instantiate(
    rec: &Value,
    parsed: &Parsed,
    key: &[i64],
    dvars: &Arc<Vec<PolyVariable>>,
    arity: usize,
) -> Result<Vec<Edge>, String> {
    let chart: Vec<bool> = rec["chart"]
        .as_str()
        .ok_or("record without chart")?
        .bytes()
        .map(|c| c == b'1')
        .collect();
    let pt: Vec<Option<i64>> = match rec["target"].as_str() {
        Some(t) => rule_point(&parse_pattern(t)?, key)?,
        None => return Err("record without target".into()),
    };
    let reps_vals: Vec<(usize, Integer)> = pt
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|x| (i, Integer::from(x))))
        .collect();
    let reps: Vec<(usize, &Integer)> = reps_vals.iter().map(|(i, v)| (*i, v)).collect();
    let mut out = vec![];
    for (pat, nd) in parsed {
        let child = eval_pattern(pat, &pt)?;
        let lines_ok = child.iter().zip(&chart).all(|(&k, &l)| !l || k >= 1);
        if !lines_ok {
            continue; // sub-sector term: dropped (modulo subsectors)
        }
        let Some((num, den)) = nd else {
            continue; // coefficient exactly zero as a text
        };
        let n = num.replace_multiple(&reps, false);
        let d = den.replace_multiple(&reps, false);
        // rebuild in d alone; any index left is an error
        let one = |p: &ZPoly| -> Result<ZPoly, String> {
            let mut q = ZPoly::new(&Z, Some(p.nterms()), dvars.clone());
            for t in p.into_iter() {
                if t.exponents[..arity].iter().any(|&e| e > 0) {
                    return Err("index variable left after the rule point was put in".into());
                }
                q.append_monomial(t.coefficient.clone(), &[t.exponents[arity]]);
            }
            Ok(q)
        };
        let (n1, d1) = (one(&n)?, one(&d)?);
        if d1.is_zero() {
            return Err("denominator vanishes at the rule point".into());
        }
        if n1.is_zero() {
            continue;
        }
        let numerators_ok = child.iter().zip(&chart).all(|(&k, &l)| l || k <= 0);
        if !numerators_ok {
            return Err(format!(
                "term {child:?} with a positive numerator index has a non-zero coefficient"
            ));
        }
        let c = <RF as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
            n1, d1, &Z, true,
        );
        out.push(Edge { child, coef: c });
    }
    Ok(out)
}

#[test]
fn audit_own_pieces() {
    let a = parse_affine("n17+1").unwrap();
    assert_eq!(
        a,
        Affine {
            c0: 1,
            terms: vec![(17, 1)]
        }
    );
    assert_eq!(parse_affine("-2").unwrap().c0, -2);
    assert_eq!(
        parse_affine("n3-n4+2").unwrap().terms,
        vec![(3, 1), (4, -1)]
    );
    let t = parse_pattern("I(n0+1,1,n2)").unwrap();
    let pt = rule_point(&t, &[3, 1, -2]).unwrap();
    assert_eq!(pt, vec![Some(2), None, Some(-2)]);
    // (d-4)^2 (d-1) -> multiplicity 2 at 4, rest 3
    let m = mult_and_rest(
        vec![
            Integer::from(-16i64),
            Integer::from(24i64),
            Integer::from(-9i64),
            Integer::from(1i64),
        ],
        4,
    );
    assert_eq!(m, Some((2, Integer::from(3i64))));
    assert_eq!(mult_and_rest(vec![Integer::from(0i64)], 4), None);
}

#[test]
#[ignore = "real node; env AUDIT_*"]
fn audit_independent_composition() {
    let env = |k: &str| std::env::var(k).ok();
    let spec_p = env("AUDIT_IMPORT_SPEC").expect("AUDIT_IMPORT_SPEC");
    let stmt = PathBuf::from(env("AUDIT_STATEMENT").expect("AUDIT_STATEMENT"));
    let out_dir = PathBuf::from(env("AUDIT_OUT").expect("AUDIT_OUT"));
    let n_sample: usize = env("AUDIT_N").map_or(2500, |x| x.parse().unwrap());
    let seed: u64 = env("AUDIT_SEED").map_or(20261115, |x| x.parse().unwrap());
    let d0: i64 = env("AUDIT_D0").map_or(4, |x| x.parse().unwrap());
    std::fs::create_dir_all(&out_dir).unwrap();
    let t0 = std::time::Instant::now();
    let say = |m: String| eprintln!("[{:7.1} s] {m}", t0.elapsed().as_secs_f64());

    // statement
    let terms_j: Value =
        serde_json::from_slice(&std::fs::read(stmt.join("terminals.json")).unwrap()).unwrap();
    let terminals: Vec<Vec<i64>> = terms_j["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| key_text_parse(x.as_str().unwrap()))
        .collect();
    let nterm = terminals.len();
    let mut served: HashMap<Vec<i64>, Served> = HashMap::new();
    let mut box_keys: Vec<Vec<i64>> = vec![];
    let mut want: HashSet<String> = HashSet::new();
    let f = std::fs::File::open(stmt.join("statement.jsonl")).unwrap();
    for line in BufReader::new(f).lines() {
        let r: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let key: Vec<i64> = serde_json::from_value(r["key"].clone()).unwrap();
        if r["in_start"].as_bool() == Some(true) {
            box_keys.push(key.clone());
        }
        let s = match r["status"].as_str() {
            Some("terminal") => Served::Terminal(
                terminals
                    .iter()
                    .position(|t| *t == key)
                    .expect("terminal row not in terminals.json"),
            ),
            Some("served") => match r["server"]["kind"].as_str() {
                Some("credit") => Served::Credit(
                    r["server"]["record_sha256"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                ),
                Some(_) => {
                    let h = r["server"]["record_sha256"].as_str().unwrap().to_string();
                    want.insert(h.clone());
                    Served::Record(h)
                }
                None => Served::Other("server without kind".into()),
            },
            o => Served::Other(format!("status {o:?}")),
        };
        served.insert(key, s);
    }
    let arity = terminals[0].len();
    say(format!(
        "statement: {} keys, {} box (in_start), {} records wanted, {} terminals",
        served.len(),
        box_keys.len(),
        want.len(),
        nterm
    ));

    // locate records by sha256 of their line
    let spec: Value = serde_json::from_slice(&std::fs::read(&spec_p).unwrap()).unwrap();
    let mut files = vec![];
    collect_paths(&spec, &mut files);
    files.sort();
    files.dedup();
    let mut loc: HashMap<String, (PathBuf, u64, usize)> = HashMap::new();
    for p in &files {
        let mut rd = BufReader::with_capacity(1 << 22, std::fs::File::open(p).unwrap());
        let mut off = 0u64;
        let mut buf = vec![];
        loop {
            buf.clear();
            let n = rd.read_until(b'\n', &mut buf).unwrap();
            if n == 0 {
                break;
            }
            let body = buf.strip_suffix(b"\n").unwrap_or(&buf);
            if body.starts_with(b"{\"type\":\"rule\"") || body.len() > 64 {
                let h = sha256_hex(body);
                if want.contains(&h) {
                    loc.entry(h).or_insert((p.clone(), off, body.len()));
                }
            }
            off += n as u64;
        }
    }
    say(format!(
        "{} table files scanned, {} of {} records located",
        files.len(),
        loc.len(),
        want.len()
    ));

    // the node's symmetry group (data: the first `symmetry_group` line of the table files), for credited keys
    let mut group: Vec<Vec<Value>> = vec![];
    'files: for p in &files {
        let rd = BufReader::with_capacity(1 << 22, std::fs::File::open(p).unwrap());
        for line in rd.split(b'\n') {
            let line = line.unwrap();
            if line.starts_with(b"{\"type\":\"symmetry_group\"") {
                let v: Value = serde_json::from_slice(&line).unwrap();
                group = v["group"]["elements"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e["action"].as_array().unwrap().clone())
                    .collect();
                break 'files;
            }
        }
    }
    say(format!("symmetry group: {} elements", group.len()));

    // sample
    box_keys.sort();
    let mut rng = Mix(seed);
    let mut idx: Vec<usize> = (0..box_keys.len()).collect();
    for i in 0..idx.len().min(n_sample) {
        let j = i + (rng.next() % (idx.len() - i) as u64) as usize;
        idx.swap(i, j);
    }
    let mut sample: Vec<Vec<i64>> = idx[..idx.len().min(n_sample)]
        .iter()
        .map(|&i| box_keys[i].clone())
        .collect();
    // AUDIT_EXTRA_KEYS = "k0,k1,..;k0,.." : keys added to the sample (e.g. the contested key K0)
    if let Some(x) = env("AUDIT_EXTRA_KEYS") {
        for k in x.split(';').filter(|s| !s.trim().is_empty()) {
            let k: Vec<i64> = k.split(',').map(|v| v.trim().parse().unwrap()).collect();
            if !sample.contains(&k) {
                sample.push(k);
            }
        }
    }

    // cone discovery: rhs keys need only the patterns (records parsed lazily, grouped)
    let allv = vars(arity);
    let dvars = Arc::new(vec![allv[arity].clone()]);
    let mut rec_cache: HashMap<String, (Value, Parsed)> = HashMap::new();
    let read_rec = |h: &str| -> Result<Value, String> {
        let (p, off, len) = loc.get(h).ok_or("record not located")?;
        let mut fh = std::fs::File::open(p).map_err(|e| e.to_string())?;
        fh.seek(SeekFrom::Start(*off)).map_err(|e| e.to_string())?;
        let mut b = vec![0u8; *len];
        fh.read_exact(&mut b).map_err(|e| e.to_string())?;
        if sha256_hex(&b) != h {
            return Err("sha256 mismatch on re-read".into());
        }
        serde_json::from_slice(&b).map_err(|e| e.to_string())
    };
    // group: record -> keys of the cone it serves (discovered breadth-first)
    let mut edges: HashMap<Vec<i64>, Result<Vec<Edge>, String>> = HashMap::new();
    let mut frontier: Vec<Vec<i64>> = sample.clone();
    let mut seen: HashSet<Vec<i64>> = sample.iter().cloned().collect();
    let mut rounds = 0;
    let mut credit_images = 0usize;
    while !frontier.is_empty() {
        rounds += 1;
        let mut by_rec: BTreeMap<String, Vec<Vec<i64>>> = BTreeMap::new();
        let mut next_frontier: Vec<Vec<i64>> = vec![];
        let cur: Vec<Vec<i64>> = std::mem::take(&mut frontier);
        for k in cur {
            match served.get(&k) {
                Some(Served::Record(h)) => by_rec.entry(h.clone()).or_default().push(k),
                Some(Served::Terminal(_)) => {}
                Some(Served::Credit(h)) => {
                    // I(k) = I(g k) for a group element g whose image is a statement key served by the SAME record
                    let img = group
                        .iter()
                        .flat_map(|act| [false, true].map(|inv| apply_action(act, &k, inv)))
                        .flatten()
                        .find(|g| matches!(served.get(g), Some(Served::Record(h2)) if h2 == h));
                    match img {
                        Some(g) => {
                            let one = ZPoly::new(&Z, Some(1), dvars.clone()).one();
                            credit_images += 1;
                            if seen.insert(g.clone()) {
                                next_frontier.push(g.clone());
                            }
                            edges.insert(
                                k,
                                Ok(vec![Edge {
                                    child: g,
                                    coef: RF::from(one),
                                }]),
                            );
                        }
                        None => {
                            edges.insert(k, Err("credit without a served image".into()));
                        }
                    }
                }
                Some(Served::Other(w)) => {
                    edges.insert(k, Err(w.clone()));
                }
                None => {
                    edges.insert(k, Err("not in the statement".into()));
                }
            }
        }
        for (h, ks) in by_rec {
            if !rec_cache.contains_key(&h) {
                let entry = match read_rec(&h) {
                    Ok(rec) => {
                        let mut parsed = vec![];
                        let mut err = None;
                        for t in rec["rhs"].as_array().into_iter().flatten() {
                            let pat = match parse_pattern(t[0].as_str().unwrap_or("")) {
                                Ok(p) => p,
                                Err(e) => {
                                    err = Some(e);
                                    break;
                                }
                            };
                            let text = t[1].as_str().unwrap_or("").trim();
                            let atom = match try_parse!(text, default_namespace = "rustred") {
                                Ok(a) => a,
                                Err(e) => {
                                    err = Some(format!("parse: {e}"));
                                    break;
                                }
                            };
                            let rp = match atom.as_view().try_to_rational_polynomial::<_, _, u16>(
                                &Q,
                                &Z,
                                Some(allv.clone()),
                            ) {
                                Ok(r) => r,
                                Err(e) => {
                                    err = Some(format!("not rational: {e}"));
                                    break;
                                }
                            };
                            if rp.numerator.get_vars_ref().len() != allv.len() {
                                err = Some("extra symbols".into());
                                break;
                            }
                            let nd = if rp.numerator.is_zero() {
                                None
                            } else {
                                Some((rp.numerator, rp.denominator))
                            };
                            parsed.push((pat, nd));
                        }
                        match err {
                            None => (rec, parsed),
                            Some(e) => (json!({"error": e}), vec![]),
                        }
                    }
                    Err(e) => (json!({"error": e}), vec![]),
                };
                rec_cache.insert(h.clone(), entry);
            }
            let (rec, parsed) = &rec_cache[&h];
            for k in ks {
                let r = if let Some(e) = rec.get("error") {
                    Err(format!("record: {e}"))
                } else {
                    instantiate(rec, parsed, &k, &dvars, arity)
                };
                if let Ok(es) = &r {
                    for e in es {
                        if seen.insert(e.child.clone()) {
                            frontier.push(e.child.clone());
                        }
                    }
                }
                edges.insert(k, r);
            }
            // big records are freed once used in this round: the coefficients are kept per edge
            if loc.get(&h).is_some_and(|l| l.2 > (64 << 20)) {
                rec_cache.remove(&h);
            }
        }
        frontier.extend(next_frontier);
    }
    let nedges: usize = edges.values().map(|r| r.as_ref().map_or(0, Vec::len)).sum();
    let nerr = edges.values().filter(|r| r.is_err()).count();
    say(format!(
        "cone {} keys ({} rounds), {} top edges, {} keys without edges (error / credit)",
        seen.len(),
        rounds,
        nedges,
        nerr
    ));

    // bottom-up exact composition (iterative post-order)
    let mut vecs: HashMap<Vec<i64>, Result<Vec<Option<RF>>, String>> = HashMap::new();
    let one_rf = || -> RF {
        let one = ZPoly::new(&Z, Some(1), dvars.clone()).one();
        RF::from(one)
    };
    for s in &sample {
        let mut stack: Vec<(Vec<i64>, bool)> = vec![(s.clone(), false)];
        let mut on_path: HashSet<Vec<i64>> = HashSet::new();
        while let Some((k, done)) = stack.pop() {
            if vecs.contains_key(&k) {
                continue;
            }
            if let Some(Served::Terminal(j)) = served.get(&k) {
                let mut v: Vec<Option<RF>> = (0..nterm).map(|_| None).collect();
                v[*j] = Some(one_rf());
                vecs.insert(k, Ok(v));
                continue;
            }
            let es = match edges.get(&k) {
                Some(Ok(es)) => es,
                Some(Err(e)) => {
                    vecs.insert(k, Err(e.clone()));
                    continue;
                }
                None => {
                    vecs.insert(k, Err("no edges".into()));
                    continue;
                }
            };
            if !done {
                if !on_path.insert(k.clone()) {
                    vecs.insert(k, Err("cycle".into()));
                    continue;
                }
                stack.push((k.clone(), true));
                for e in es {
                    if !vecs.contains_key(&e.child) {
                        stack.push((e.child.clone(), false));
                    }
                }
                continue;
            }
            on_path.remove(&k);
            let mut acc: Vec<Option<RF>> = (0..nterm).map(|_| None).collect();
            let mut bad: Option<String> = None;
            for e in es {
                match vecs.get(&e.child) {
                    Some(Ok(cv)) => {
                        for (aj, x) in acc.iter_mut().zip(cv) {
                            if let Some(x) = x {
                                let p = &e.coef * x;
                                *aj = Some(match aj.take() {
                                    None => p,
                                    Some(a) => &a + &p,
                                });
                            }
                        }
                    }
                    Some(Err(w)) => {
                        bad = Some(format!("child: {w}"));
                        break;
                    }
                    None => {
                        bad = Some("child not composed".into());
                        break;
                    }
                }
            }
            let acc: Vec<Option<RF>> = acc
                .into_iter()
                .map(|x| x.filter(|r| !r.numerator.is_zero()))
                .collect();
            vecs.insert(k, bad.map_or(Ok(acc), Err));
        }
    }
    say(format!("composed {} keys", vecs.len()));

    // per sampled key: orders and leading coefficients
    let theirs: HashMap<Vec<i64>, (Vec<Value>, Vec<Value>)> = match env("AUDIT_ROWS") {
        Some(p) => {
            let mut m = HashMap::new();
            for line in BufReader::new(std::fs::File::open(p).unwrap()).lines() {
                let r: Value = serde_json::from_str(&line.unwrap()).unwrap();
                if let Some(k) = r["key"].as_array() {
                    let k: Vec<i64> = k.iter().map(|x| x.as_i64().unwrap()).collect();
                    m.insert(
                        k,
                        (
                            r["orders"].as_array().cloned().unwrap_or_default(),
                            r["leading"].as_array().cloned().unwrap_or_default(),
                        ),
                    );
                }
            }
            m
        }
        None => HashMap::new(),
    };
    // AUDIT_REGBASIS = comma-separated regular-basis records (T_i = sum_j M[i][j] R_j): c_R(K) = c_T(K) M
    let maps: Vec<MapRec> = env("AUDIT_REGBASIS")
        .map(|x| {
            x.split(',')
                .filter(|s| !s.is_empty())
                .map(|p| {
                    let r: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
                    let t: Vec<Vec<i64>> = serde_json::from_value(r["terminals"].clone()).unwrap();
                    assert_eq!(
                        t, terminals,
                        "{p}: terminal order differs from terminals.json"
                    );
                    let m: Vec<Vec<Option<RF>>> = r["M"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|row| {
                            row.as_array()
                                .unwrap()
                                .iter()
                                .map(|x| {
                                    let a = try_parse!(
                                        x.as_str().unwrap(),
                                        default_namespace = "rustred"
                                    )
                                    .unwrap();
                                    let rp = a
                                        .as_view()
                                        .try_to_rational_polynomial::<_, _, u16>(
                                            &Q,
                                            &Z,
                                            Some(dvars.clone()),
                                        )
                                        .unwrap();
                                    assert_eq!(
                                        rp.numerator.get_vars_ref().len(),
                                        1,
                                        "M entry not in d alone"
                                    );
                                    (!rp.numerator.is_zero()).then_some(rp)
                                })
                                .collect()
                        })
                        .collect();
                    let parse_mat = |name: &str| -> Vec<Vec<Option<RF>>> {
                        r[name]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|row| {
                                row.as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|x| {
                                        let a = try_parse!(
                                            x.as_str().unwrap(),
                                            default_namespace = "rustred"
                                        )
                                        .unwrap();
                                        let rp = a
                                            .as_view()
                                            .try_to_rational_polynomial::<_, _, u16>(
                                                &Q,
                                                &Z,
                                                Some(dvars.clone()),
                                            )
                                            .unwrap();
                                        (!rp.numerator.is_zero()).then_some(rp)
                                    })
                                    .collect()
                            })
                            .collect()
                    };
                    let minv = parse_mat("M_inv");
                    let rset: Vec<Vec<i64>> =
                        serde_json::from_value(r["regular_set"].clone()).unwrap();
                    (p.to_string(), m, minv, rset)
                })
                .collect()
        })
        .unwrap_or_default();
    // R = M^-1 T: the independently composed vector of every R_j must equal row j of M^-1 EXACTLY
    let mut minv_check: Vec<Value> = vec![];
    for (p, _, minv, rset) in &maps {
        for (j, rk) in rset.iter().enumerate() {
            let verdict = match vecs.get(rk) {
                Some(Ok(v)) => {
                    let eq = (0..nterm).all(|i| match (&v[i], &minv[j][i]) {
                        (None, None) => true,
                        (Some(a), Some(b)) => (a - b).numerator.is_zero(),
                        _ => false,
                    });
                    json!(if eq { "equal" } else { "DIFFERENT" })
                }
                Some(Err(e)) => json!(format!("undecided: {e}")),
                None => json!("not composed (add it to AUDIT_EXTRA_KEYS)"),
            };
            minv_check.push(json!({"record": p, "R_j": rk, "row_of_Minv": verdict}));
        }
    }
    let mut on_r_pole: Vec<usize> = vec![0; maps.len()];
    let mut on_r_hist: Vec<BTreeMap<String, usize>> = vec![BTreeMap::new(); maps.len()];
    let mut rows = std::fs::File::create(out_dir.join("audit_rows.jsonl")).unwrap();
    let (mut decided, mut undecided, mut with_pole) = (0usize, 0usize, 0usize);
    let mut hist: BTreeMap<String, usize> = BTreeMap::new();
    let (mut cmp_eq, mut cmp_ne, mut cmp_missing, mut lead_eq, mut lead_ne) = (0, 0, 0, 0, 0);
    let mut mismatches: Vec<Value> = vec![];
    for s in &sample {
        match &vecs[s] {
            Ok(v) => {
                decided += 1;
                let ol: Vec<Option<(i64, String)>> = v
                    .iter()
                    .map(|c| c.as_ref().and_then(|r| order_and_leading(r, 0, d0)))
                    .collect();
                let poles: Vec<Option<i64>> = ol
                    .iter()
                    .map(|x| x.as_ref().map(|(o, _)| (-o).max(0)))
                    .collect();
                if poles.iter().any(|p| p.unwrap_or(0) > 0) {
                    with_pole += 1;
                }
                *hist.entry(format!("{poles:?}")).or_default() += 1;
                let lead: Vec<Option<String>> = ol
                    .iter()
                    .map(|x| x.as_ref().map(|(_, l)| l.clone()))
                    .collect();
                if let Some((to, tl)) = theirs.get(s) {
                    let mine_o: Vec<Value> = poles.iter().map(|p| json!(p)).collect();
                    if *to == mine_o {
                        cmp_eq += 1;
                    } else {
                        cmp_ne += 1;
                        if mismatches.len() < 50 {
                            mismatches.push(json!({"key": s, "mine": mine_o, "theirs": to}));
                        }
                    }
                    let mine_l: Vec<Value> = lead.iter().map(|l| json!(l)).collect();
                    if *tl == mine_l {
                        lead_eq += 1;
                    } else {
                        lead_ne += 1;
                        if mismatches.len() < 50 {
                            mismatches
                                .push(json!({"key": s, "lead_mine": mine_l, "lead_theirs": tl}));
                        }
                    }
                } else if !theirs.is_empty() {
                    cmp_missing += 1;
                }
                let mut on_r = vec![];
                for (mi, (_, m, _, _)) in maps.iter().enumerate() {
                    let mut cr: Vec<Option<(i64, String)>> = vec![];
                    for j in 0..nterm {
                        let mut acc: Option<RF> = None;
                        for i in 0..nterm {
                            if let (Some(c), Some(e)) = (&v[i], &m[i][j]) {
                                let p = c * e;
                                acc = Some(match acc {
                                    None => p,
                                    Some(a) => &a + &p,
                                });
                            }
                        }
                        cr.push(acc.as_ref().and_then(|r| order_and_leading(r, 0, d0)));
                    }
                    let pr: Vec<Option<i64>> = cr
                        .iter()
                        .map(|x| x.as_ref().map(|(o, _)| (-o).max(0)))
                        .collect();
                    if pr.iter().any(|p| p.unwrap_or(0) > 0) {
                        on_r_pole[mi] += 1;
                    }
                    *on_r_hist[mi].entry(format!("{pr:?}")).or_default() += 1;
                    on_r.push(json!({"valuations": cr.iter().map(|x| x.as_ref().map(|(o, _)| *o)).collect::<Vec<_>>(),
                                     "leading": cr.iter().map(|x| x.as_ref().map(|(_, l)| l.clone())).collect::<Vec<_>>()}));
                }
                writeln!(
                    rows,
                    "{}",
                    json!({"key": s, "orders": poles, "valuations": ol.iter().map(|x| x.as_ref().map(|(o, _)| *o)).collect::<Vec<_>>(), "leading": lead, "on_R": on_r})
                )
                .unwrap();
            }
            Err(e) => {
                undecided += 1;
                writeln!(rows, "{}", json!({"key": s, "undecided": e})).unwrap();
            }
        }
    }
    let summary = json!({"schema": "audit-r2-poles.independent-composition.v1",
        "inputs": {"import_spec": spec_p, "statement": stmt, "seed": seed, "n": sample.len(), "d0": d0,
                   "box_keys": box_keys.len()},
        "cone_keys": seen.len(), "top_edges": nedges, "keys_without_edges": nerr, "credit_images": credit_images,
        "decided": decided, "undecided": undecided, "with_pole_on_T": with_pole,
        "fraction_with_pole": (with_pole as f64) / (decided.max(1) as f64),
        "on_R": maps.iter().enumerate().map(|(i, (p, _, _, _))| json!({"record": p, "keys_with_pole": on_r_pole[i], "patterns": on_r_hist[i]})).collect::<Vec<_>>(),
        "R_equals_Minv_T": minv_check,
        "pole_patterns": hist,
        "compare": {"orders_equal": cmp_eq, "orders_differ": cmp_ne, "not_in_their_rows": cmp_missing,
                    "leading_equal": lead_eq, "leading_differ": lead_ne, "mismatches": mismatches},
        "seconds": t0.elapsed().as_secs_f64()});
    std::fs::write(
        out_dir.join("audit_summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
    say(format!("{summary}"));
}
