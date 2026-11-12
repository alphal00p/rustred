//! coef-form probe: alternative representations of rule-table coefficients,
//! measured WITHOUT changing values.
//!
//! coef_form_probe <payload .rrbin> <scratch dir> [rule sample]
//!
//! For one Custom payload (4L package owner or imported 5L table) it reports,
//! as one JSON line:
//! - current: rrbin bytes, decode seconds, in-memory bytes of the expanded
//!   `RationalPolynomial` table;
//! - compact serialisations (same simple varint/sparse-exponent encoder) of
//!   (E) the expanded form and (F) a factored form: one per-table dictionary
//!   of irreducible factors (Symbolica `factor`), each coefficient =
//!   content * prod factor^e / (content * prod factor^e); polynomials with
//!   more than `COEF_FORM_FACTOR_CAP` terms (default 4000) are kept whole
//!   (as one dictionary entry); bytes, zstd -3 bytes (CLI), decode seconds,
//!   in-memory bytes;
//! - per rule application (a sample of rules, the same for every method, at
//!   random integer index points and random d, 3 primes):
//!   A  current exact path: index specialisation term by term + GCD
//!      normalisation over Z[d] (`from_num_den`), then Horner in d mod p;
//!   B  expanded num/den evaluated mod p at (n, d), per-term powers (the
//!      certification replay's method);
//!   C  factored: each distinct dictionary factor of the rule evaluated once
//!      mod p, coefficients as products;
//!   D  Symbolica evaluator (Horner + CSE, `evaluator_multiple` ->
//!      `map_to_ring(Zp64)`) per rule over all its num/den, build time apart;
//!   with value identity of B, C, D (and A where the index point is regular)
//!   checked at every point.
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientPolynomial};
use rustred::propagator::custom::custom_payload_coefficient_table;
use symbolica::atom::{Atom, AtomCore};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField, Zp64};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::poly::factor::Factorize;
use symbolica::prelude::{Integer, Z};

type Poly = CoefficientPolynomial;
type Fe = symbolica::domains::finite_field::FiniteFieldElement<u64>;

const PRIMES: [u64; 3] = [2305843009213693951, 4611686018427387847, 1152921504606846883];

fn rss_bytes() -> u64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: u64 = statm.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    pages * 4096
}

// ---------------- compact encoder ----------------

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}
fn read_varint(src: &[u8], at: &mut usize) -> u64 {
    let (mut v, mut shift) = (0u64, 0);
    loop {
        let b = src[*at];
        *at += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b < 0x80 {
            return v;
        }
        shift += 7;
    }
}
fn put_integer(out: &mut Vec<u8>, c: &Integer) {
    match c {
        // zigzag in 63 bits (the tag bit takes one): |i| < 2^61
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
fn get_integer(src: &[u8], at: &mut usize) -> Integer {
    let tag = read_varint(src, at);
    if tag & 1 == 0 {
        let z = tag >> 1;
        Integer::from(((z >> 1) as i64) ^ -((z & 1) as i64))
    } else {
        let n = (tag >> 1) as usize;
        let s = std::str::from_utf8(&src[*at..*at + n]).unwrap();
        *at += n;
        s.parse::<Integer>().ok().unwrap()
    }
}
/// nterms, then per term: coefficient, number of nonzero exponents, (var, exp).
fn put_poly(out: &mut Vec<u8>, p: &Poly) {
    varint(out, p.nterms() as u64);
    for i in 0..p.nterms() {
        put_integer(out, &p.coefficients[i]);
        let e = p.exponents(i);
        let nz = e.iter().filter(|&&x| x != 0).count();
        varint(out, nz as u64);
        for (v, &x) in e.iter().enumerate() {
            if x != 0 {
                out.push(v as u8);
                varint(out, u64::from(x));
            }
        }
    }
}
fn get_poly(src: &[u8], at: &mut usize, template: &Poly, exps: &mut Vec<u16>) -> Poly {
    let n = read_varint(src, at) as usize;
    let mut p = template.zero_with_capacity(n);
    for _ in 0..n {
        let c = get_integer(src, at);
        exps.iter_mut().for_each(|x| *x = 0);
        let nz = read_varint(src, at);
        for _ in 0..nz {
            let v = src[*at] as usize;
            *at += 1;
            exps[v] = read_varint(src, at) as u16;
        }
        p.append_monomial(c, exps);
    }
    p
}

fn integer_heap(c: &Integer) -> usize {
    match c {
        Integer::Large(_) => (c.to_string().len() * 425).div_ceil(1000) + 16,
        _ => 0,
    }
}
fn poly_mem(p: &Poly) -> usize {
    std::mem::size_of::<Poly>()
        + p.coefficients.capacity() * std::mem::size_of::<Integer>()
        + p.coefficients.iter().map(integer_heap).sum::<usize>()
        + p.exponents_iter().count() * p.nvars() * 2
}

// ---------------- factored form ----------------

#[derive(Clone, Debug)]
struct FPart {
    content: Integer,
    factors: Vec<(u32, u32)>,
}
#[derive(Clone, Debug)]
struct FCoef {
    num: FPart,
    den: FPart,
}

struct Dict {
    polys: Vec<Poly>,
    index: HashMap<Poly, u32>,
}
impl Dict {
    fn id(&mut self, p: Poly) -> u32 {
        if let Some(&i) = self.index.get(&p) {
            return i;
        }
        let i = self.polys.len() as u32;
        self.polys.push(p.clone());
        self.index.insert(p, i);
        i
    }
}

fn factor_part(p: &Poly, cap: usize, dict: &mut Dict, stats: &mut (usize, usize)) -> FPart {
    let mut part = FPart { content: Integer::one(), factors: vec![] };
    if p.is_constant() {
        part.content = p.lcoeff();
        if p.is_zero() {
            part.content = Integer::zero();
        }
        return part;
    }
    if p.nterms() > cap {
        stats.1 += 1;
        part.factors.push((dict.id(p.clone()), 1));
        return part;
    }
    stats.0 += 1;
    let mut check = p.one();
    for (f, e) in p.factor() {
        if f.is_constant() {
            let c = f.lcoeff();
            for _ in 0..e {
                part.content = &part.content * &c;
            }
            check = check * &f.pow(e);
            continue;
        }
        check = check * &f.pow(e);
        part.factors.push((dict.id(f), e as u32));
    }
    assert!(check == *p, "factorisation does not multiply back");
    part.factors.sort_unstable();
    part
}

fn put_part(out: &mut Vec<u8>, part: &FPart) {
    put_integer(out, &part.content);
    varint(out, part.factors.len() as u64);
    for &(f, e) in &part.factors {
        varint(out, u64::from(f));
        varint(out, u64::from(e));
    }
}
fn get_part(src: &[u8], at: &mut usize) -> FPart {
    let content = get_integer(src, at);
    let n = read_varint(src, at);
    let factors = (0..n)
        .map(|_| (read_varint(src, at) as u32, read_varint(src, at) as u32))
        .collect();
    FPart { content, factors }
}

// ---------------- modular evaluation ----------------

fn red(f: &Zp64, c: &Integer) -> Fe {
    c.to_finite_field(f)
}
/// Per-term powers (method B / factor evaluation).
fn eval_poly(f: &Zp64, p: &Poly, point: &[Fe]) -> Fe {
    let mut acc = f.zero();
    for i in 0..p.nterms() {
        let mut t = red(f, &p.coefficients[i]);
        for (v, &e) in p.exponents(i).iter().enumerate() {
            if e != 0 {
                t = f.mul(&t, &f.pow(&point[v], u64::from(e)));
            }
        }
        acc = f.add(&acc, &t);
    }
    acc
}
fn eval_part(f: &Zp64, part: &FPart, values: &[Option<Fe>]) -> Fe {
    let mut acc = red(f, &part.content);
    for &(id, e) in &part.factors {
        acc = f.mul(&acc, &f.pow(&values[id as usize].unwrap(), u64::from(e)));
    }
    acc
}

/// Method A: the package's exact specialisation (index variables to integers,
/// term by term) and GCD normalisation; returns the Q(d) coefficient.
fn specialize_exact(c: &Coefficient, base: usize, idx: &[i64]) -> Coefficient {
    let spec = |p: &Poly| -> Poly {
        let mut out = p.zero_with_capacity(p.nterms());
        let mut exps = vec![0u16; p.nvars()];
        for i in 0..p.nterms() {
            let e = p.exponents(i);
            let mut c = p.coefficients[i].clone();
            for (pos, &val) in idx.iter().enumerate() {
                let x = e[base + pos];
                if x != 0 {
                    c = c * Integer::from(val).pow(u64::from(x));
                }
            }
            exps[..base].copy_from_slice(&e[..base]);
            out.append_monomial(c, &exps);
        }
        out
    };
    let (num, den) = (spec(&c.numerator), spec(&c.denominator));
    <Coefficient as FromNumeratorAndDenominator<_, _, u16>>::from_num_den(num, den, &Z, true)
}
/// Index specialisation of one polynomial term by term (the package's
/// method, no normalisation).
fn specialize_poly(p: &Poly, base: usize, idx: &[i64]) -> Poly {
    let mut out = p.zero_with_capacity(p.nterms());
    let mut exps = vec![0u16; p.nvars()];
    for i in 0..p.nterms() {
        let e = p.exponents(i);
        let mut c = p.coefficients[i].clone();
        for (pos, &val) in idx.iter().enumerate() {
            let x = e[base + pos];
            if x != 0 {
                c = c * Integer::from(val).pow(u64::from(x));
            }
        }
        exps[..base].copy_from_slice(&e[..base]);
        out.append_monomial(c, &exps);
    }
    out
}
fn part_poly(part: &FPart, spec: &HashMap<u32, Poly>, one: &Poly) -> Poly {
    let mut acc = one.clone().mul_coeff(part.content.clone());
    for &(id, e) in &part.factors {
        acc = acc * &spec[&id].pow(e as usize);
    }
    acc
}
fn horner_d(f: &Zp64, p: &Poly, d: Fe) -> Fe {
    // dense in d (variable 0); other exponents are zero after specialisation
    let deg = (0..p.nterms()).map(|i| p.exponents(i)[0]).max().unwrap_or(0) as usize;
    let mut dense = vec![f.zero(); deg + 1];
    for i in 0..p.nterms() {
        let k = p.exponents(i)[0] as usize;
        dense[k] = f.add(&dense[k], &red(f, &p.coefficients[i]));
    }
    let mut v = f.zero();
    for c in dense.iter().rev() {
        v = f.add(&f.mul(&v, &d), c);
    }
    v
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn zstd_bytes(path: &Path, bytes: &[u8]) -> u64 {
    std::fs::write(path, bytes).unwrap();
    let out = path.with_extension("zst");
    let status = std::process::Command::new("zstd")
        .args(["-q", "-f", "-3", "-T1"])
        .arg(path)
        .arg("-o")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    let n = std::fs::metadata(&out).unwrap().len();
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(path);
    n
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = Path::new(&args[1]);
    let scratch = Path::new(&args[2]);
    let rule_sample: usize = args.get(3).map_or(400, |s| s.parse().unwrap());
    let cap: usize = std::env::var("COEF_FORM_FACTOR_CAP").ok().map_or(4000, |s| s.parse().unwrap());
    let eval_cap: usize = std::env::var("COEF_FORM_EVAL_CAP").ok().map_or(200_000, |s| s.parse().unwrap());
    let name = path.file_stem().unwrap().to_string_lossy().to_string();
    std::fs::create_dir_all(scratch).unwrap();
    let mut report = serde_json::Map::new();
    let mut put = |k: &str, v: serde_json::Value| {
        eprintln!("[{name}] {k} = {v}");
        report.insert(k.into(), v);
    };

    // ---- current form: read + decode ----
    let rss0 = rss_bytes();
    let t = Instant::now();
    let bytes = std::fs::read(path).unwrap();
    let read_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let table = custom_payload_coefficient_table(&bytes).unwrap();
    let decode_s = t.elapsed().as_secs_f64();
    let rss1 = rss_bytes();
    let rrbin_bytes = bytes.len() as u64;
    let rrbin_zstd = zstd_bytes(&scratch.join(format!("{name}.rrbin.tmp")), &bytes);
    drop(bytes);
    let coefs = &table.coefficients;
    let nvars = coefs.first().map_or(0, |c| c.numerator.nvars());
    let base = nvars - table.symbolic_slots;
    let vars: Vec<String> = coefs
        .first()
        .map(|c| c.numerator.variables().iter().map(|v| v.to_atom().to_string()).collect())
        .unwrap_or_default();
    put("table", name.clone().into());
    put("variables", serde_json::json!(vars));
    put("base_variables", base.into());
    put("rules", table.rules.len().into());
    put("rhs_terms", table.rules.iter().map(Vec::len).sum::<usize>().into());
    put("coefficients", coefs.len().into());
    let (nt, dt): (usize, usize) = coefs
        .iter()
        .fold((0, 0), |a, c| (a.0 + c.numerator.nterms(), a.1 + c.denominator.nterms()));
    put("num_terms", nt.into());
    put("den_terms", dt.into());
    let rule_terms: Vec<usize> = table
        .rules
        .iter()
        .map(|r| {
            r.iter()
                .map(|&i| coefs[i as usize].numerator.nterms() + coefs[i as usize].denominator.nterms())
                .sum()
        })
        .collect();
    put("max_rule_terms", rule_terms.iter().copied().max().unwrap_or(0).into());
    put("rrbin_bytes", rrbin_bytes.into());
    put("rrbin_zstd3_bytes", rrbin_zstd.into());
    put("rrbin_read_s", read_s.into());
    put("rrbin_decode_s", decode_s.into());
    put("rrbin_decode_rss_delta", (rss1.saturating_sub(rss0)).into());
    let mem_expanded: usize = coefs.iter().map(|c| poly_mem(&c.numerator) + poly_mem(&c.denominator)).sum();
    put("mem_expanded_bytes", mem_expanded.into());

    // ---- compact expanded ----
    let mut e_bytes = Vec::new();
    varint(&mut e_bytes, coefs.len() as u64);
    for c in coefs {
        put_poly(&mut e_bytes, &c.numerator);
        put_poly(&mut e_bytes, &c.denominator);
    }
    put("compact_expanded_bytes", e_bytes.len().into());
    put("compact_expanded_zstd3_bytes", zstd_bytes(&scratch.join(format!("{name}.ce.tmp")), &e_bytes).into());
    let template = coefs[0].numerator.zero();
    let t = Instant::now();
    {
        let mut at = 0;
        let mut exps = vec![0u16; nvars];
        let n = read_varint(&e_bytes, &mut at) as usize;
        let mut back = Vec::with_capacity(n);
        for _ in 0..n {
            let num = get_poly(&e_bytes, &mut at, &template, &mut exps);
            let den = get_poly(&e_bytes, &mut at, &template, &mut exps);
            back.push((num, den));
        }
        put("compact_expanded_decode_s", t.elapsed().as_secs_f64().into());
        let bad = back
            .iter()
            .zip(coefs)
            .filter(|(b, c)| b.0.to_string() != c.numerator.to_string() || b.1.to_string() != c.denominator.to_string())
            .count();
        put("compact_expanded_roundtrip_mismatches", bad.into());
        let maps: std::collections::HashSet<usize> = coefs.iter().map(|c| c.numerator.nvars()).collect();
        put("distinct_nvars", serde_json::json!(maps.into_iter().collect::<Vec<_>>()));
    }
    drop(e_bytes);

    // ---- factored ----
    let t = Instant::now();
    let mut dict = Dict { polys: vec![], index: HashMap::new() };
    let mut fstats = (0usize, 0usize);
    let fcoefs: Vec<FCoef> = coefs
        .iter()
        .map(|c| FCoef {
            num: factor_part(&c.numerator, cap, &mut dict, &mut fstats),
            den: factor_part(&c.denominator, cap, &mut dict, &mut fstats),
        })
        .collect();
    put("factor_s", t.elapsed().as_secs_f64().into());
    put("factored_polys", fstats.0.into());
    put("kept_whole_polys", fstats.1.into());
    put("dict_factors", dict.polys.len().into());
    put("dict_terms", dict.polys.iter().map(|p| p.nterms()).sum::<usize>().into());
    let refs: usize = fcoefs.iter().map(|c| c.num.factors.len() + c.den.factors.len()).sum();
    put("factor_refs", refs.into());
    let linear = dict.polys.iter().filter(|p| (0..p.nterms()).all(|i| p.exponents(i).iter().map(|&x| u32::from(x)).sum::<u32>() <= 1)).count();
    put("dict_linear_factors", linear.into());
    let mut f_bytes = Vec::new();
    varint(&mut f_bytes, dict.polys.len() as u64);
    for p in &dict.polys {
        put_poly(&mut f_bytes, p);
    }
    varint(&mut f_bytes, fcoefs.len() as u64);
    for c in &fcoefs {
        put_part(&mut f_bytes, &c.num);
        put_part(&mut f_bytes, &c.den);
    }
    put("compact_factored_bytes", f_bytes.len().into());
    put("compact_factored_zstd3_bytes", zstd_bytes(&scratch.join(format!("{name}.cf.tmp")), &f_bytes).into());
    let t = Instant::now();
    {
        let mut at = 0;
        let mut exps = vec![0u16; nvars];
        let n = read_varint(&f_bytes, &mut at) as usize;
        let polys: Vec<Poly> = (0..n).map(|_| get_poly(&f_bytes, &mut at, &template, &mut exps)).collect();
        let m = read_varint(&f_bytes, &mut at) as usize;
        let back: Vec<(FPart, FPart)> = (0..m).map(|_| (get_part(&f_bytes, &mut at), get_part(&f_bytes, &mut at))).collect();
        put("compact_factored_decode_s", t.elapsed().as_secs_f64().into());
        assert_eq!(polys.len(), dict.polys.len());
        assert_eq!(back.len(), fcoefs.len());
        // multiply back to today's expanded form (what a loader that keeps
        // the engine unchanged must do); identity checked on printed forms
        let one = template.one();
        let t = Instant::now();
        let expand = |part: &FPart| {
            let mut acc = one.clone().mul_coeff(part.content.clone());
            for &(id, e) in &part.factors {
                acc = acc * &polys[id as usize].pow(e as usize);
            }
            acc
        };
        let rebuilt: Vec<(Poly, Poly)> = back.iter().map(|(n, d)| (expand(n), expand(d))).collect();
        put("compact_factored_expand_s", t.elapsed().as_secs_f64().into());
        let t = Instant::now();
        let rebuilt_den: Vec<Poly> = back.iter().map(|(_, d)| expand(d)).collect();
        put("compact_denfactored_expand_den_s", t.elapsed().as_secs_f64().into());
        let bad = rebuilt
            .iter()
            .zip(&rebuilt_den)
            .zip(coefs)
            .filter(|((r, rd), c)| {
                r.0.to_string() != c.numerator.to_string()
                    || r.1.to_string() != c.denominator.to_string()
                    || rd.to_string() != c.denominator.to_string()
            })
            .count();
        put("compact_factored_rebuild_mismatches", bad.into());
    }
    let mem_factored: usize = dict.polys.iter().map(poly_mem).sum::<usize>()
        + fcoefs.iter().map(|c| 2 * std::mem::size_of::<FPart>() + (c.num.factors.len() + c.den.factors.len()) * 8).sum::<usize>();
    put("mem_factored_bytes", mem_factored.into());
    drop(f_bytes);

    // ---- variant 0: numerator expanded, denominator factored over a
    // per-table dictionary (what keeping the lift's FactorizedRationalPolynomial
    // denominator on write would give) ----
    let mut den_ids: Vec<u32> = fcoefs.iter().flat_map(|c| c.den.factors.iter().map(|x| x.0)).collect();
    den_ids.sort_unstable();
    den_ids.dedup();
    let remap: HashMap<u32, u32> = den_ids.iter().enumerate().map(|(k, &i)| (i, k as u32)).collect();
    let mut z_bytes = Vec::new();
    varint(&mut z_bytes, den_ids.len() as u64);
    for &i in &den_ids {
        put_poly(&mut z_bytes, &dict.polys[i as usize]);
    }
    varint(&mut z_bytes, fcoefs.len() as u64);
    for (c, f) in coefs.iter().zip(&fcoefs) {
        put_poly(&mut z_bytes, &c.numerator);
        let part = FPart {
            content: f.den.content.clone(),
            factors: f.den.factors.iter().map(|&(i, e)| (remap[&i], e)).collect(),
        };
        put_part(&mut z_bytes, &part);
    }
    put("den_dict_factors", den_ids.len().into());
    put("den_dict_terms", den_ids.iter().map(|&i| dict.polys[i as usize].nterms()).sum::<usize>().into());
    put("compact_denfactored_bytes", z_bytes.len().into());
    put("compact_denfactored_zstd3_bytes", zstd_bytes(&scratch.join(format!("{name}.cz.tmp")), &z_bytes).into());
    let mem_denfactored: usize = den_ids.iter().map(|&i| poly_mem(&dict.polys[i as usize])).sum::<usize>()
        + coefs.iter().zip(&fcoefs).map(|(c, f)| poly_mem(&c.numerator) + std::mem::size_of::<FPart>() + f.den.factors.len() * 8).sum::<usize>();
    put("mem_denfactored_bytes", mem_denfactored.into());
    drop(z_bytes);

    // ---- size ruling per rule (monomials): expanded = sum of num+den terms
    // of its RHS coefficients (as today); denfactored = numerator terms + terms
    // of the distinct denominator factors it uses + one per reference;
    // factored = terms of the distinct dictionary factors it uses + one per
    // reference ----
    {
        let mut over = serde_json::Map::new();
        let bands = [1_000_000usize, 10_000_000];
        let mut counts = [[0usize; 2]; 3];
        let mut listed = Vec::new();
        for (r, ids) in table.rules.iter().enumerate() {
            let mut all: Vec<u32> = Vec::new();
            let mut dens: Vec<u32> = Vec::new();
            let mut refs = 0usize;
            let mut num_terms = 0usize;
            for &i in ids {
                let f = &fcoefs[i as usize];
                all.extend(f.num.factors.iter().chain(&f.den.factors).map(|x| x.0));
                dens.extend(f.den.factors.iter().map(|x| x.0));
                refs += f.num.factors.len() + f.den.factors.len() + 2;
                num_terms += coefs[i as usize].numerator.nterms();
            }
            all.sort_unstable();
            all.dedup();
            dens.sort_unstable();
            dens.dedup();
            let fac: usize = all.iter().map(|&u| dict.polys[u as usize].nterms()).sum::<usize>() + refs;
            let den: usize = num_terms + dens.iter().map(|&u| dict.polys[u as usize].nterms()).sum::<usize>() + ids.len();
            let sizes = [rule_terms[r], den, fac];
            for (k, s) in sizes.iter().enumerate() {
                for (b, &band) in bands.iter().enumerate() {
                    if *s > band {
                        counts[k][b] += 1;
                    }
                }
            }
            if rule_terms[r] > 1_000_000 {
                listed.push(serde_json::json!([r, sizes[0], sizes[1], sizes[2]]));
            }
        }
        for (k, name) in ["expanded", "denfactored", "factored"].iter().enumerate() {
            over.insert(format!("{name}_over_1M"), counts[k][0].into());
            over.insert(format!("{name}_over_10M"), counts[k][1].into());
        }
        over.insert("rules_over_1M_expanded_den_factored".into(), listed.into());
        put("size_ruling", serde_json::Value::Object(over));
    }

    // ---- rule sample ----
    let mut rng = Rng(0x9e3779b97f4a7c15);
    let mut order: Vec<usize> = (0..table.rules.len()).collect();
    order.sort_by_key(|&r| std::cmp::Reverse(rule_terms[r]));
    let mut sample: Vec<usize> = order.iter().copied().filter(|&r| rule_terms[r] <= eval_cap).take(rule_sample / 10).collect();
    let rest: Vec<usize> = order.iter().copied().filter(|&r| rule_terms[r] <= eval_cap && !sample.contains(&r)).collect();
    let target = rule_sample.min(sample.len() + rest.len());
    while sample.len() < target {
        let r = rest[(rng.next() as usize) % rest.len()];
        if !sample.contains(&r) {
            sample.push(r);
        }
    }
    put("sample_rules", sample.len().into());
    put("sample_rule_terms", sample.iter().map(|&r| rule_terms[r]).sum::<usize>().into());
    put("skipped_rules_over_eval_cap", rule_terms.iter().filter(|&&t| t > eval_cap).count().into());

    // ---- D: Symbolica evaluators (Horner + CSE), built per sampled rule ----
    let params: Vec<Atom> = coefs[0].numerator.variables().iter().map(|v| v.to_atom()).collect();
    let t = Instant::now();
    let mut evaluators = Vec::new();
    for &r in &sample {
        let exprs: Vec<Atom> = table.rules[r]
            .iter()
            .flat_map(|&i| {
                let c = &coefs[i as usize];
                [c.numerator.to_expression(), c.denominator.to_expression()]
            })
            .collect();
        let ev = Atom::evaluator_multiple(&exprs, &params)
            .horner_iterations(1)
            .build()
            .unwrap();
        evaluators.push(ev);
    }
    put("D_build_s", t.elapsed().as_secs_f64().into());
    let fields: Vec<Zp64> = PRIMES.iter().map(|&p| Zp64::new(p)).collect();

    let (mut ta, mut tb, mut tc, mut td, mut tc0) = (0f64, 0f64, 0f64, 0f64, 0f64);
    let (mut a_spec, mut a_horner) = (0f64, 0f64);
    let (mut te, mut te0, mut e_checked) = (0f64, 0f64, 0usize);
    let mut checked = 0usize;
    let mut a_checked = 0usize;
    let mut a_skipped = 0usize;
    let mut mismatches = 0usize;
    let mut applications = 0usize;
    let mut a_applications = 0usize;
    for (k, &r) in sample.iter().enumerate() {
        let ids = &table.rules[r];
        for (pi, f) in fields.iter().enumerate() {
            let idx: Vec<i64> = (0..table.symbolic_slots).map(|_| (rng.next() % 9) as i64 - 3).collect();
            let dval = 1000 + rng.next() % 1_000_000;
            let mut point = vec![f.zero(); nvars];
            point[0] = f.to_element(dval);
            for (pos, &v) in idx.iter().enumerate() {
                point[base + pos] = red(f, &Integer::from(v));
            }
            applications += 1;
            // B
            let t = Instant::now();
            let vb: Vec<(Fe, Fe)> = ids
                .iter()
                .map(|&i| {
                    let c = &coefs[i as usize];
                    (eval_poly(f, &c.numerator, &point), eval_poly(f, &c.denominator, &point))
                })
                .collect();
            tb += t.elapsed().as_secs_f64();
            // C
            let t = Instant::now();
            let mut values: Vec<Option<Fe>> = vec![None; 0];
            let mut used: Vec<u32> = ids
                .iter()
                .flat_map(|&i| {
                    let c = &fcoefs[i as usize];
                    c.num.factors.iter().chain(&c.den.factors).map(|x| x.0)
                })
                .collect();
            used.sort_unstable();
            used.dedup();
            let mut local: HashMap<u32, Fe> = HashMap::with_capacity(used.len());
            for &u in &used {
                local.insert(u, eval_poly(f, &dict.polys[u as usize], &point));
            }
            let vc: Vec<(Fe, Fe)> = ids
                .iter()
                .map(|&i| {
                    let c = &fcoefs[i as usize];
                    let ev = |part: &FPart| {
                        let mut acc = red(f, &part.content);
                        for &(id, e) in &part.factors {
                            acc = f.mul(&acc, &f.pow(&local[&id], u64::from(e)));
                        }
                        acc
                    };
                    (ev(&c.num), ev(&c.den))
                })
                .collect();
            tc += t.elapsed().as_secs_f64();
            // C0: numerator expanded, denominator factors evaluated once per rule
            let t = Instant::now();
            let mut dused: Vec<u32> = ids.iter().flat_map(|&i| fcoefs[i as usize].den.factors.iter().map(|x| x.0)).collect();
            dused.sort_unstable();
            dused.dedup();
            let mut dlocal: HashMap<u32, Fe> = HashMap::with_capacity(dused.len());
            for &u in &dused {
                dlocal.insert(u, eval_poly(f, &dict.polys[u as usize], &point));
            }
            let v0: Vec<(Fe, Fe)> = ids
                .iter()
                .map(|&i| {
                    let part = &fcoefs[i as usize].den;
                    let mut acc = red(f, &part.content);
                    for &(id, e) in &part.factors {
                        acc = f.mul(&acc, &f.pow(&dlocal[&id], u64::from(e)));
                    }
                    (eval_poly(f, &coefs[i as usize].numerator, &point), acc)
                })
                .collect();
            tc0 += t.elapsed().as_secs_f64();
            for (b, z) in vb.iter().zip(&v0) {
                if !f.is_zero(&b.1) && (f.is_zero(&z.1) || f.div(&z.0, &z.1) != f.div(&b.0, &b.1)) {
                    mismatches += 1;
                }
            }
            values.clear();
            let _ = eval_part;
            // D
            let mut ev = evaluators[k].clone().map_to_ring(f).unwrap();
            let mut out = vec![f.zero(); 2 * ids.len()];
            let t = Instant::now();
            ev.evaluate_in_ring(&point, &mut out, f);
            td += t.elapsed().as_secs_f64();
            // compare B, C, D as num/den ratios where den != 0
            for (j, (b, c)) in vb.iter().zip(&vc).enumerate() {
                let d = (out[2 * j], out[2 * j + 1]);
                if f.is_zero(&b.1) {
                    continue;
                }
                checked += 1;
                let rb = f.div(&b.0, &b.1);
                let ok = !f.is_zero(&c.1)
                    && !f.is_zero(&d.1)
                    && f.div(&c.0, &c.1) == rb
                    && f.div(&d.0, &d.1) == rb;
                if !ok {
                    mismatches += 1;
                }
            }
            // A (exact): only where every denominator is regular at the index point
            let t = Instant::now();
            let mut regular = true;
            let mut va = Vec::with_capacity(ids.len());
            let mut spec_s = 0f64;
            for &i in ids {
                let c = &coefs[i as usize];
                let ts = Instant::now();
                // a vanishing specialised denominator is a pole: the package
                // refuses such an application; skip the rule at this point
                let den_zero = {
                    let p = &c.denominator;
                    let mut out = p.zero_with_capacity(p.nterms());
                    let mut exps = vec![0u16; p.nvars()];
                    for t in 0..p.nterms() {
                        let e = p.exponents(t);
                        let mut cc = p.coefficients[t].clone();
                        for (pos, &val) in idx.iter().enumerate() {
                            let x = e[base + pos];
                            if x != 0 {
                                cc = cc * Integer::from(val).pow(u64::from(x));
                            }
                        }
                        exps[..base].copy_from_slice(&e[..base]);
                        out.append_monomial(cc, &exps);
                    }
                    out.is_zero()
                };
                if den_zero {
                    regular = false;
                    break;
                }
                let s = specialize_exact(c, base, &idx);
                spec_s += ts.elapsed().as_secs_f64();
                va.push(s);
            }
            if !regular {
                a_skipped += 1;
                continue;
            }
            let th = Instant::now();
            let vals: Vec<(Fe, Fe)> = va.iter().map(|s| (horner_d(f, &s.numerator, point[0]), horner_d(f, &s.denominator, point[0]))).collect();
            a_horner += th.elapsed().as_secs_f64();
            ta += t.elapsed().as_secs_f64();
            a_spec += spec_s;
            a_applications += 1;
            for (j, a) in vals.iter().enumerate() {
                if f.is_zero(&a.1) || f.is_zero(&vb[j].1) {
                    continue;
                }
                a_checked += 1;
                if f.div(&a.0, &a.1) != f.div(&vb[j].0, &vb[j].1) {
                    mismatches += 1;
                }
            }
            // E: exact specialisation of the fully factored form (each used
            // dictionary factor specialised once per application, products,
            // NO GCD); E0: numerator expanded (term by term) + denominator
            // factors specialised once, NO GCD. Same Z[d] num/den up to a
            // common factor: values compared as ratios mod p with A.
            let one = coefs[0].numerator.one();
            let t = Instant::now();
            let spec: HashMap<u32, Poly> =
                used.iter().map(|&u| (u, specialize_poly(&dict.polys[u as usize], base, &idx))).collect();
            let ve: Vec<(Poly, Poly)> = ids
                .iter()
                .map(|&i| {
                    let c = &fcoefs[i as usize];
                    (part_poly(&c.num, &spec, &one), part_poly(&c.den, &spec, &one))
                })
                .collect();
            te += t.elapsed().as_secs_f64();
            let t = Instant::now();
            let dspec: HashMap<u32, Poly> =
                dused.iter().map(|&u| (u, specialize_poly(&dict.polys[u as usize], base, &idx))).collect();
            let ve0: Vec<(Poly, Poly)> = ids
                .iter()
                .map(|&i| {
                    let c = &fcoefs[i as usize];
                    (specialize_poly(&coefs[i as usize].numerator, base, &idx), part_poly(&c.den, &dspec, &one))
                })
                .collect();
            te0 += t.elapsed().as_secs_f64();
            for (j, a) in vals.iter().enumerate() {
                if f.is_zero(&a.1) {
                    continue;
                }
                let ra = f.div(&a.0, &a.1);
                for v in [&ve[j], &ve0[j]] {
                    let (n, d) = (horner_d(f, &v.0, point[0]), horner_d(f, &v.1, point[0]));
                    e_checked += 1;
                    if f.is_zero(&d) || f.div(&n, &d) != ra {
                        mismatches += 1;
                    }
                }
            }
            let _ = pi;
        }
    }
    let us = |s: f64, n: usize| if n == 0 { 0.0 } else { 1e6 * s / n as f64 };
    put("applications", applications.into());
    put("A_applications", a_applications.into());
    put("A_skipped_pole_points", a_skipped.into());
    put("A_exact_us_per_rule", us(ta, a_applications).into());
    put("A_specialise_gcd_us_per_rule", us(a_spec, a_applications).into());
    put("A_horner_us_per_rule", us(a_horner, a_applications).into());
    put("E_factored_exact_nogcd_us_per_rule", us(te, a_applications).into());
    put("E0_denfactored_exact_nogcd_us_per_rule", us(te0, a_applications).into());
    put("values_checked_E", e_checked.into());
    put("B_expanded_mod_us_per_rule", us(tb, applications).into());
    put("C_factored_mod_us_per_rule", us(tc, applications).into());
    put("C0_denfactored_mod_us_per_rule", us(tc0, applications).into());
    put("D_evaluator_mod_us_per_rule", us(td, applications).into());
    put("values_checked_BCD", checked.into());
    put("values_checked_A", a_checked.into());
    put("value_mismatches", mismatches.into());
    put("peak_rss_bytes", rss_bytes().into());
    let line = serde_json::Value::Object(report).to_string();
    let out = scratch.join("coef_form_probe.jsonl");
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(out).unwrap();
    writeln!(file, "{line}").unwrap();
    println!("{line}");
}
