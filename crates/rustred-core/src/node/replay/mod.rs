//! Independent sampled IBP-span replay of a rule table: semantics of Astra's `p5l-replay.v1`
//! (`/shared/vhirschi/t3code/astra_5L/tools/p5l_replay/replay.py`), as engine code.
//!
//! For every selected rule: an integer sample point inside the rule's case (provided, or a deterministic affine-RREF
//! search with the same sequence as the Python tool) that satisfies the chart signs, avoids every exception
//! conjunction and every coefficient pole at three values of d; the rule's recenter from an authority (a binding's
//! raw pivot / recenter, the table record's `raw_pivot_encoded` written by the rustred node driver, a fully numeric
//! case, or a unique one-source-row reconstruction for direct hits); a source support (the retained native trace
//! with the authenticated basis derivation, or ALL independently generated original IBPs at the retained/exported
//! seeds); then per dimension a fresh modular witness over P61 of `target - sum c_k I_k` in the span of those rows
//! with ALL physical columns, only independently proved scaleless sectors quotiented, and a +1 coefficient mutation
//! that the same witness must reject. Missing authority, unsupported input or no valid sample: UNKNOWN (never PASS).
//! PASS is sampled RS2/RS3 evidence only (no completeness, descent or master count).
//!
//! Independence: the IBP rows come from the family text through [`family::Family`] (own operator construction and
//! Lee criterion), all arithmetic is own u64/i128 code; no RustRed solver code and no Symbolica arithmetic is used.
//! Exact zero tests of exception conjunctions use two independent 61-bit primes instead of exact rationals.
//! Extension over p5l-replay.v1: a rule record carrying `raw_pivot_encoded` (rustred node tables) supplies the
//! recenter authority when no binding does; tables without that field replay exactly as with the Python tool.

pub mod arith;
pub mod expr;
pub mod family;
pub mod locus;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};

use arith::{P, Q};
use expr::{Expr, Unknown};
use family::Family;

pub const VERSION: &str = "p5l-replay.v1 (rustred node::replay)";
pub const DIMS: [&str; 3] = ["17/7", "19/7", "23/7"];

pub type Key = Vec<i64>;
pub type Row = BTreeMap<Key, u64>;

#[derive(Clone, Debug)]
pub struct ReplayArgs {
    pub family: Option<PathBuf>,
    pub table: PathBuf,
    pub cert: Option<PathBuf>,
    pub cand_dump_dir: Option<PathBuf>,
    pub bindings: Option<PathBuf>,
    /// `None` = all.
    pub rules: Option<BTreeSet<usize>>,
    pub chart: Option<String>,
    pub out: PathBuf,
    pub dimensions: Vec<String>,
    pub sample_attempts: usize,
    /// guard-fix hardening: replay this many ADDITIONAL distinct valid points per rule from the same deterministic
    /// search (0 = the single point of p5l-replay.v1). The rule passes only if every point passes.
    pub extra_points: usize,
    /// guard-fix hardening: JSONL of `{"chart":C,"index":I,"loci":["poly",...]}` (e.g. a rule's hidden source-weight
    /// poles from `rr_node witness`): per locus one more point ON the locus (inside the case, valid, off the rule's
    /// exceptions). A locus that is already one of the rule's exception branches is reported as `guarded`.
    /// Rule records carrying `witness_loci` (strings) are treated the same way.
    pub loci: Option<PathBuf>,
    /// guard-fix box replay: replay EVERY valid integer point of the rule's case whose free coordinates lie in the
    /// box |n| <= K on their chart side (n = 1..K on a positive axis, n = 0..-(K-1) on a non-positive one; pivot
    /// coordinates from the case equations, integral only), off the rule's exceptions and RHS poles; 0 = off.
    pub box_size: usize,
    /// Cap on box points per rule (deterministic lexicographic prefix; the record says when it was hit).
    pub box_max: usize,
    /// guard-fix --dump-weights: per cell also write `weights-<d>.json` (source rows, pivot rows, nonzero weights mod
    /// P, column keys, zero-sector keys, desired row) for the exact upgrade of a modular PASS (helper Y24).
    pub dump_weights: bool,
    /// guard-fix: draw d at random in F_p per cell instead of the fixed `dimensions` (rr_node replay default when
    /// no --dimensions is given; `--fixed-d` keeps the three fixed values 17/7 19/7 23/7 of p5l-replay.v1).
    pub random_d: bool,
    /// guard-fix: run each cell modulo P only (default: P and the second prime P2 = 2^61 - 31).
    pub single_prime: bool,
}

impl ReplayArgs {
    pub fn parse_rules(spec: &str) -> Result<Option<BTreeSet<usize>>, String> {
        if spec == "all" {
            return Ok(None);
        }
        let mut out = BTreeSet::new();
        for bit in spec.split(',') {
            match bit.split_once('-') {
                Some((a, z)) => {
                    let (a, z): (usize, usize) = (
                        a.parse().map_err(|_| "rule range")?,
                        z.parse().map_err(|_| "rule range")?,
                    );
                    if a > z {
                        return Err("invalid index range".into());
                    }
                    out.extend(a..=z);
                }
                None => {
                    out.insert(bit.parse().map_err(|_| format!("rule index {bit}"))?);
                }
            }
        }
        Ok(Some(out))
    }
}

/// One integral-key coordinate: (Some(axis), offset) symbolic, (None, value) numeric.
type Pat = Vec<(Option<usize>, i64)>;

fn parse_integral(text: &str, n: usize) -> Result<Pat, Unknown> {
    let inner = text
        .strip_prefix("I(")
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("unsupported integral key: {text}"))?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() != n {
        return Err("truncated physical key".into());
    }
    parts
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if let Some(rest) = x.strip_prefix('n') {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                let axis: usize = digits
                    .parse()
                    .map_err(|_| format!("unsupported integral key: {text}"))?;
                if axis != i {
                    return Err(format!(
                        "unsupported integral key: key axis permuted {text}"
                    ));
                }
                let off = &rest[digits.len()..];
                let off: i64 = if off.is_empty() {
                    0
                } else {
                    off.trim_start_matches('+')
                        .parse()
                        .map_err(|_| format!("unsupported integral key: {text}"))?
                };
                Ok((Some(i), off))
            } else {
                Ok((
                    None,
                    x.parse()
                        .map_err(|_| format!("unsupported integral key: {text}"))?,
                ))
            }
        })
        .collect()
}

fn key_at(pat: &Pat, point: &[i64]) -> Key {
    pat.iter()
        .map(|(ax, off)| ax.map_or(*off, |a| point[a] + off))
        .collect()
}

fn power_decode(b: i64) -> Result<(bool, i64), Unknown> {
    if !(0..256).contains(&b) {
        return Err("invalid Power byte".into());
    }
    let a = b & 127;
    Ok((b & 128 != 0, if a >= 64 { a - 128 } else { a }))
}

fn strip(r: &Row) -> Row {
    strip_p(P, r)
}

fn strip_p(p: u64, r: &Row) -> Row {
    r.iter()
        .filter(|(_, v)| **v % p != 0)
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}

fn addto(r: &mut Row, k: Key, x: u64) {
    addto_p(P, r, k, x)
}

fn addto_p(p: u64, r: &mut Row, k: Key, x: u64) {
    let e = r.entry(k).or_insert(0);
    *e = arith::add(p, *e, x);
}

fn entries(r: &Row) -> Value {
    Value::Array(strip(r).into_iter().map(|(k, v)| json!([k, v])).collect())
}

#[derive(Clone)]
struct Rule {
    raw: Value,
    chart: String,
    index: usize,
    fixed: Vec<Option<i64>>,
    equations: Vec<String>,
    exceptions: Vec<Vec<String>>,
    target: String,
    rhs: Vec<(String, String)>,
    seeds: Vec<String>,
    direct_hit: bool,
}

fn rule_from(v: &Value) -> Result<Rule, Unknown> {
    let s = |x: &Value| {
        x.as_str()
            .map(str::to_string)
            .ok_or_else(|| "malformed rule record".to_string())
    };
    Ok(Rule {
        raw: v.clone(),
        chart: s(&v["chart"])?,
        index: v["index"].as_u64().ok_or("rule index")? as usize,
        fixed: v["fixed"]
            .as_array()
            .ok_or("rule fixed")?
            .iter()
            .map(|x| x.as_i64())
            .collect(),
        equations: v
            .get("equations")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        exceptions: v["exceptions"]
            .as_array()
            .ok_or("rule exceptions")?
            .iter()
            .map(|b| {
                b.as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter_map(|p| p.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect(),
        target: s(&v["target"])?,
        rhs: v["rhs"]
            .as_array()
            .ok_or("rule rhs")?
            .iter()
            .map(|t| Ok((s(&t[0])?, s(&t[1])?)))
            .collect::<Result<_, Unknown>>()?,
        seeds: v
            .get("seeds")
            .and_then(|e| e.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        direct_hit: v
            .get("direct_hit")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The authenticated certificate: original rows (exact, scaled own rows) and basis derivations.
struct Cert {
    /// m -> offset -> linear form [const, y.., d]
    rows: HashMap<usize, BTreeMap<Key, Vec<Q>>>,
    basis: HashMap<(String, usize), Vec<(usize, Expr)>>,
}

fn source_basis(path: &Path, f: &Family) -> Result<(Cert, Value), Unknown> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cert: {e}"))?;
    let mut head: Option<Value> = None;
    let mut src: Vec<Value> = Vec::new();
    let mut basis: HashMap<(String, usize), Vec<(usize, Expr)>> = HashMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).map_err(|e| format!("cert json: {e}"))?;
        match r["type"].as_str() {
            Some("cert_header") => head = Some(r),
            Some("source_row") => src.push(r),
            Some("basis_row") => {
                let k = (
                    r["chart"].as_str().unwrap_or_default().to_string(),
                    r["k"].as_u64().ok_or("basis k")? as usize,
                );
                if basis.contains_key(&k) {
                    return Err("duplicate native basis ordinal".into());
                }
                let entries = r["sources"]
                    .as_array()
                    .ok_or("basis sources")?
                    .iter()
                    .map(|e| {
                        Ok((
                            e[0].as_u64().ok_or("basis source m")? as usize,
                            Expr::parse(e[1].as_str().ok_or("basis coefficient")?, f.n)?,
                        ))
                    })
                    .collect::<Result<_, Unknown>>()?;
                basis.insert(k, entries);
            }
            _ => {}
        }
    }
    let head = head.ok_or("unsupported/missing certificate header")?;
    if head["conditions"].as_array().is_some_and(|c| !c.is_empty()) {
        return Err("unsupported/missing certificate header".into());
    }
    if head["loops"].as_u64() != Some(f.loops as u64) || head["externals"].as_u64() != Some(1) {
        return Err("certificate family metadata".into());
    }
    let l = f.loops;
    let mut rows = HashMap::new();
    let mut ops = BTreeSet::new();
    for r in &src {
        let dl = r["differentiated_loop"]
            .as_u64()
            .ok_or("original IBP operator out of range")? as usize;
        let cm = r["contraction_momentum"]
            .as_u64()
            .ok_or("original IBP operator out of range")? as usize;
        if dl >= l || cm > l {
            return Err("original IBP operator out of range".into());
        }
        let terms = r["terms"].as_array().ok_or("source row terms")?;
        let mut exported: BTreeMap<Key, Vec<Q>> = BTreeMap::new();
        for t in terms {
            let pat = parse_integral(t[0].as_str().ok_or("source key")?, f.n)?;
            if pat.iter().any(|(ax, _)| ax.is_none()) {
                return Err("native source rows fail independent authentication: source row with a numeric key".into());
            }
            let e = Expr::parse(t[1].as_str().ok_or("source coefficient")?, f.n)?;
            if e.degree()? > 1 {
                return Err("non-affine native original IBP row".into());
            }
            let form = e
                .linear()
                .map_err(|_| "non-affine native original IBP row".to_string())?;
            let key: Key = pat.iter().map(|(_, o)| *o).collect();
            if exported.insert(key, form).is_some() {
                return Err("duplicate original source-row key".into());
            }
        }
        let m = r["m"].as_u64().ok_or("source row m")? as usize;
        let op = dl * (l + 1) + cm.min(l);
        ops.insert((dl, cm));
        let own = f.exact_row(op)?;
        if own.keys().collect::<Vec<_>>() != exported.keys().collect::<Vec<_>>() {
            return Err(format!(
                "native source rows fail independent authentication: source row key mismatch {m}"
            ));
        }
        let (k0, form0) = own.iter().next().ok_or("empty own row")?;
        let kk = (0..form0.len())
            .rev()
            .find(|&i| !form0[i].is_zero())
            .ok_or("empty own form")?;
        let s = exported[k0][kk].div(form0[kk])?;
        for (k, form) in &own {
            for (a, b) in exported[k].iter().zip(form) {
                if *a != b.mul(s)? {
                    return Err(format!(
                        "native source rows fail independent authentication: source row not proportional to the own row {m}"
                    ));
                }
            }
        }
        if s.is_zero() {
            return Err("zero source-row scale".into());
        }
        let scaled = own
            .into_iter()
            .map(|(k, form)| {
                Ok((
                    k,
                    form.into_iter()
                        .map(|x| x.mul(s))
                        .collect::<Result<Vec<Q>, String>>()?,
                ))
            })
            .collect::<Result<_, Unknown>>()?;
        rows.insert(m, scaled);
    }
    if rows.len() != l * (l + 1) || src.len() != l * (l + 1) {
        return Err("incomplete original source basis".into());
    }
    if ops.len() != l * (l + 1) {
        return Err("duplicate/missing original IBP operator".into());
    }
    if basis
        .values()
        .any(|e| e.iter().any(|(m, _)| !rows.contains_key(m)))
    {
        return Err("basis refers to absent original source".into());
    }
    let auth = json!({"kind": "native original rows authenticated exactly against independent generator", "source_rows": rows.len(), "basis_rows": basis.len(), "native_weights_used": false});
    Ok((Cert { rows, basis }, auth))
}

fn eval_form_p(p: u64, form: &[Q], y: &[i64], d: u64) -> Result<u64, Unknown> {
    let n = y.len();
    let mut v = form[0].modp(p).ok_or("form pole")?;
    for i in 0..n {
        if !form[i + 1].is_zero() {
            v = arith::add(
                p,
                v,
                arith::mul(
                    p,
                    form[i + 1].modp(p).ok_or("form pole")?,
                    arith::from_i128(p, y[i] as i128),
                ),
            );
        }
    }
    Ok(arith::add(
        p,
        v,
        arith::mul(p, form[n + 1].modp(p).ok_or("form pole")?, d),
    ))
}

fn get_bindings(path: &Option<PathBuf>) -> Result<Vec<Value>, Unknown> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let x: Value =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| format!("bindings: {e}"))?)
            .map_err(|e| format!("bindings: {e}"))?;
    Ok(match x {
        Value::Array(a) => a,
        Value::Object(ref o) if o.contains_key("rules") => {
            x["rules"].as_array().cloned().unwrap_or_default()
        }
        other => vec![other],
    })
}

fn binding_for(bindings: &[Value], rule: &Rule) -> Result<Value, Unknown> {
    let hits: Vec<&Value> = bindings
        .iter()
        .filter(|x| {
            let idx = x
                .get("index")
                .or_else(|| x.get("native_rule_index"))
                .or_else(|| x.get("rule_index"))
                .and_then(Value::as_u64);
            idx == Some(rule.index as u64)
                && x.get("chart")
                    .and_then(Value::as_str)
                    .is_none_or(|c| c == rule.chart)
        })
        .collect();
    if hits.len() > 1 {
        return Err("ambiguous per-rule binding".into());
    }
    Ok(hits
        .first()
        .map(|x| (*x).clone())
        .unwrap_or_else(|| json!({})))
}

fn vector(x: &Value, n: usize) -> Result<Vec<i64>, Unknown> {
    match x {
        Value::Object(o) => {
            let mut z = vec![0; n];
            for (k, v) in o {
                let i: usize = k
                    .trim_start_matches('n')
                    .parse()
                    .map_err(|_| "bad recenter entry")?;
                if i >= n {
                    return Err("bad recenter entry".into());
                }
                z[i] = v.as_i64().ok_or("bad recenter entry")?;
            }
            Ok(z)
        }
        Value::Array(a) if a.len() == n => a
            .iter()
            .map(|v| v.as_i64().ok_or_else(|| "bad recenter vector".to_string()))
            .collect(),
        _ => Err("bad recenter vector".into()),
    }
}

fn recenter_for(rule: &Rule, b: &Value, n: usize) -> Result<(Vec<i64>, String), Unknown> {
    // Python `dict.get(key, default)` semantics: a present key wins even when its value is null (= None).
    let pyget = |o: &Value, key: &str, default: Option<Value>| -> Option<Value> {
        match o.get(key) {
            Some(v) => (!v.is_null()).then(|| v.clone()),
            None => default,
        }
    };
    let e = match b.get("recenter_evidence") {
        Some(v) if v.is_object() => v.clone(),
        _ => json!({}),
    };
    let explicit = pyget(
        b,
        "recenter",
        pyget(b, "canonicalization_shifts", pyget(&e, "recenter", None)),
    );
    let mut raw = pyget(b, "raw_pivot_encoded", pyget(&e, "raw_pivot_encoded", None));
    let mut authority_raw =
        "raw Power pivot independently decoded; target minus raw offset".to_string();
    if raw.is_none() && explicit.is_none() {
        if let Some(r) = rule.raw.get("raw_pivot_encoded").filter(|v| !v.is_null()) {
            raw = Some(r.clone());
            authority_raw = "table rule record raw pivot (rustred node driver ExactStarted/rule frame), independently decoded".into();
        }
    }
    if let Some(raw) = raw {
        let raw = raw.as_array().ok_or("raw pivot length")?;
        if raw.len() != n {
            return Err("raw pivot length".into());
        }
        let pat = parse_integral(&rule.target, n)?;
        let mut derived = Vec::with_capacity(n);
        for (a, (ax, toff)) in raw.iter().zip(&pat) {
            let (sym, off) = power_decode(a.as_i64().ok_or("invalid Power byte")?)?;
            if sym != ax.is_some() {
                return Err("raw pivot symbolic pattern disagrees with target".into());
            }
            if sym {
                derived.push(toff - off);
            } else {
                if off != *toff {
                    return Err("raw pivot numeric coordinate disagrees with target".into());
                }
                derived.push(0);
            }
        }
        if let Some(x) = &explicit {
            if vector(x, n)? != derived {
                return Err("raw pivot and recenter disagree".into());
            }
        }
        return Ok((derived, authority_raw));
    }
    if let Some(x) = explicit {
        let auth = pyget(
            b,
            "recenter_authority",
            pyget(&e, "status", pyget(&e, "event", None)),
        );
        let Some(auth) = auth.filter(|a| a != &json!("") && a != &json!(false)) else {
            return Err("explicit recenter has no authority".into());
        };
        return Ok((
            vector(&x, n)?,
            auth.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| auth.to_string()),
        ));
    }
    if rule.fixed.iter().all(Option::is_some) {
        return Ok((
            vec![0; n],
            "fully numeric case: no symbolic recenter".into(),
        ));
    }
    Err("missing symbolic raw pivot/recenter authority (including direct-hit cases)".into())
}

fn seed_at(record: &Value, point: &[i64], recenter: &[i64], n: usize) -> Result<Key, Unknown> {
    let text = record["seed"]
        .as_str()
        .ok_or("unsupported native Seed encoding")?;
    let inner = text
        .strip_prefix("Seed { integral: Integral([")
        .and_then(|s| s.strip_suffix(" }"))
        .ok_or("unsupported native Seed encoding")?;
    let (powers, shifts) = inner
        .split_once("]), shifts: [")
        .ok_or("unsupported native Seed encoding")?;
    let shifts = shifts
        .strip_suffix(']')
        .ok_or("unsupported native Seed encoding")?;
    let raw: Vec<i64> = powers
        .split(", ")
        .map(|p| {
            p.strip_prefix("Power(")
                .and_then(|x| x.strip_suffix(')'))
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| "truncated Seed Power vector".to_string())
        })
        .collect::<Result<_, _>>()?;
    if raw.len() != n {
        return Err("truncated Seed Power vector".into());
    }
    let shifts: Vec<i64> = shifts
        .split(',')
        .map(|x| x.trim().parse().map_err(|_| "seed shifts".to_string()))
        .collect::<Result<_, _>>()?;
    if shifts.len() != n {
        return Err("seed shifts length".into());
    }
    let mut y = Vec::with_capacity(n);
    for (i, &x) in raw.iter().enumerate() {
        let (sym, off) = power_decode(x)?;
        if sym {
            if off != shifts[i] {
                return Err("Power vs Seed shift mismatch".into());
            }
            y.push(point[i] + off + recenter[i]);
        } else {
            if shifts[i] != 0 || recenter[i] != 0 {
                return Err("numeric seed coordinate cannot recenter".into());
            }
            y.push(off);
        }
    }
    Ok(y)
}

enum Support {
    /// (basis_row, seed record)
    Retained(Vec<Value>),
    /// full original IBPs at retained trace seeds
    RetainedSeeds(Vec<Value>),
    Exported(Vec<Pat>),
    /// (basis_row, seed pattern)
    Direct(Vec<(usize, Pat)>),
}

impl Support {
    fn kind(&self) -> &'static str {
        match self {
            Self::Retained(_) => "retained_native_basis_trace",
            Self::RetainedSeeds(_) => "full_original_IBPs_at_retained_trace_seeds",
            Self::Exported(_) => "full_original_IBPs_at_exported_seeds",
            Self::Direct(_) => "reconstructed_direct_basis",
        }
    }
}

fn support(
    rule: &Rule,
    b: &Value,
    dumpdir: &Option<PathBuf>,
    n: usize,
    cert: Option<&Cert>,
) -> Result<(Support, Value), Unknown> {
    let path: Option<PathBuf> = b
        .get("cand_dump")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            dumpdir.as_ref().map(|d| {
                d.join(format!(
                    "F.cand_dump.r{}.json",
                    b.get("source_rule_index")
                        .and_then(Value::as_u64)
                        .unwrap_or(rule.index as u64)
                ))
            })
        });
    if let Some(path) = path.filter(|p| p.exists()) {
        let dump: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if dump["schema"] != json!("xstuck.candidate_dump.v1") {
            return Err("unsupported candidate dump schema".into());
        }
        let fixed_json: Value = rule.raw["fixed"].clone();
        if dump["target"] != json!(rule.target) || dump["case_fixed"] != fixed_json {
            return Err("candidate/table target or fixed case differs".into());
        }
        let deq = dump
            .pointer("/case/equations")
            .cloned()
            .unwrap_or(json!([]));
        if deq != json!(rule.equations) {
            return Err("candidate/table affine equations differ".into());
        }
        if dump["rhs_count"].as_u64() != Some(rule.rhs.len() as u64) {
            return Err("candidate/table RHS count differs".into());
        }
        let piv = dump["pivots"]
            .as_array()
            .filter(|p| !p.is_empty())
            .ok_or("missing genuine retained trace descriptors")?
            .clone();
        let meta = json!({"path": path.display().to_string(), "descriptors": piv.len()});
        if let Some(c) = cert {
            if !piv.iter().all(|x| {
                x["basis_row"]
                    .as_u64()
                    .is_some_and(|k| c.basis.contains_key(&(rule.chart.clone(), k as usize)))
            }) {
                return Err("trace basis missing from authenticated certificate".into());
            }
            return Ok((Support::Retained(piv), meta));
        }
        return Ok((Support::RetainedSeeds(piv), meta));
    }
    if rule.seeds.is_empty() {
        return Err("no retained trace or native exported seed support".into());
    }
    let pats = rule
        .seeds
        .iter()
        .map(|s| parse_integral(s, n))
        .collect::<Result<Vec<_>, _>>()?;
    let meta = json!({"exported_seeds": pats.len(), "retained_native_trace": false});
    Ok((Support::Exported(pats), meta))
}

/// Exact RREF of the fixed coordinates and affine equations: (rows [coeffs.., rhs], pivot columns).
fn rref_equations(rule: &Rule, n: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
    let mut rows: Vec<Vec<Q>> = Vec::new();
    for (i, x) in rule.fixed.iter().enumerate() {
        if let Some(x) = x {
            rows.push(
                (0..n)
                    .map(|j| if j == i { Q::ONE } else { Q::ZERO })
                    .chain([Q::int(*x as i128)])
                    .collect(),
            );
        }
    }
    for text in &rule.equations {
        let e = Expr::parse(text, n)?;
        if e.degree()? > 1 || e.has_d() {
            return Err("case equation is not affine in indices".into());
        }
        let form = e.linear()?;
        rows.push(form[1..=n].iter().copied().chain([form[0].neg()]).collect());
    }
    let mut rank = 0;
    let mut piv = Vec::new();
    for col in 0..n {
        let Some(k) = (rank..rows.len()).find(|&i| !rows[i][col].is_zero()) else {
            continue;
        };
        rows.swap(rank, k);
        let v = rows[rank][col];
        rows[rank] = rows[rank]
            .iter()
            .map(|x| x.div(v))
            .collect::<Result<_, _>>()?;
        for i in 0..rows.len() {
            if i != rank && !rows[i][col].is_zero() {
                let v = rows[i][col];
                let pr = rows[rank].clone();
                rows[i] = rows[i]
                    .iter()
                    .zip(&pr)
                    .map(|(a, b)| a.sub(v.mul(*b)?))
                    .collect::<Result<_, _>>()?;
            }
        }
        piv.push(col);
        rank += 1;
    }
    if rows
        .iter()
        .any(|r| r[..n].iter().all(Q::is_zero) && !r[n].is_zero())
    {
        return Err("empty/inconsistent affine domain; no sample replay".into());
    }
    rows.truncate(rank);
    Ok((rows, piv))
}

fn point_valid(
    rule: &Rule,
    point: &[i64],
    dims: &[Q],
    chart: Option<&Value>,
    coefs: &[Expr],
) -> Result<(), Unknown> {
    let n = rule.fixed.len();
    if point.len() != n {
        return Err("sample must be complete integer physical vector".into());
    }
    if rule
        .fixed
        .iter()
        .enumerate()
        .any(|(i, f)| f.is_some_and(|f| point[i] != f))
    {
        return Err("sample outside fixed case".into());
    }
    for text in &rule.equations {
        if !Expr::parse(text, n)?.is_zero_at(point, Q::ZERO)? {
            return Err("sample outside affine case".into());
        }
    }
    let guards: Option<Vec<String>> = chart.and_then(|c| c["guards"].as_array()).map(|g| {
        g.iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    });
    let chart_bits: Vec<char> = rule.chart.chars().collect();
    for (i, &x) in point.iter().enumerate() {
        let g = match &guards {
            Some(g) => g.get(i).cloned().unwrap_or_default(),
            None => {
                if chart_bits.get(i) == Some(&'1') {
                    "n>=1".into()
                } else {
                    "n<=0".into()
                }
            }
        };
        // eps (two-sided) axes: the guard constrains the integer part of the exponent the same way
        let g = g
            .strip_prefix("two_sided:")
            .map(str::to_string)
            .unwrap_or(g);
        if !["n>=1", "n<=0", "n:any", "any", "n in Z"].contains(&g.as_str()) {
            return Err(format!("unsupported chart guard {g}"));
        }
        if (g == "n>=1" && x < 1) || (g == "n<=0" && x > 0) {
            return Err("sample outside chart signs".into());
        }
    }
    for branch in &rule.exceptions {
        if branch.is_empty() {
            return Err("empty/unsupported exception branch".into());
        }
        let exprs: Vec<Expr> = branch
            .iter()
            .map(|p| Expr::parse(p, n))
            .collect::<Result<_, _>>()?;
        for &ds in dims {
            let mut any_nonzero = false;
            for e in &exprs {
                if !e.is_zero_at(point, ds)? {
                    any_nonzero = true;
                    break;
                }
            }
            if !any_nonzero {
                return Err("sample inside exception conjunction".into());
            }
        }
    }
    for &ds in dims {
        for e in coefs {
            e.at_point(P, point, ds)?;
        }
    }
    Ok(())
}

/// Every valid integer point of the rule's case in the box of [`ReplayArgs::box_size`] (lexicographic over the
/// free coordinates), at most `cap`; the bool says whether the cap cut the enumeration.
fn box_points(
    rule: &Rule,
    dims: &[Q],
    chart: Option<&Value>,
    coefs: &[Expr],
    size: usize,
    cap: usize,
) -> Result<(Vec<Vec<i64>>, bool), Unknown> {
    let n = rule.fixed.len();
    let (rows, piv) = rref_equations(rule, n)?;
    let free: Vec<usize> = (0..n).filter(|i| !piv.contains(i)).collect();
    let bits: Vec<char> = rule.chart.chars().collect();
    let size = size.max(1) as i64;
    let value = |i: usize, k: i64| if bits.get(i) == Some(&'1') { 1 + k } else { -k };
    let mut counter = vec![0i64; free.len()];
    let mut found = Vec::new();
    loop {
        let mut p = vec![Q::ZERO; n];
        for (slot, &i) in free.iter().enumerate() {
            p[i] = Q::int(value(i, counter[slot]) as i128);
        }
        for (row, &i) in rows.iter().zip(&piv).rev() {
            let mut v = row[n];
            for &j in &free {
                v = v.sub(row[j].mul(p[j])?)?;
            }
            p[i] = v;
        }
        if p.iter().all(|x| x.d == 1) {
            let pt: Vec<i64> = p.iter().map(|x| x.n as i64).collect();
            if point_valid(rule, &pt, dims, chart, coefs).is_ok() {
                if found.len() >= cap {
                    return Ok((found, true));
                }
                found.push(pt);
            }
        }
        // odometer over the free coordinates
        let mut slot = 0;
        loop {
            if slot == counter.len() {
                return Ok((found, false));
            }
            counter[slot] += 1;
            if counter[slot] < size {
                break;
            }
            counter[slot] = 0;
            slot += 1;
        }
    }
}

/// Up to `want` distinct valid points of the deterministic search (provided samples: the first valid one only),
/// optionally restricted to the zero set of `locus` (an affine locus joins the RREF system; a nonlinear one filters),
/// skipping `avoid`. Err when none is found (never an emptiness proof).
#[allow(clippy::too_many_arguments)]
fn sample_points(
    rule: &Rule,
    b: &Value,
    dims: &[Q],
    chart: Option<&Value>,
    coefs: &[Expr],
    attempts: usize,
    want: usize,
    locus: Option<(&str, &Expr)>,
    avoid: &[Vec<i64>],
) -> Result<(Vec<Vec<i64>>, String), Unknown> {
    let n = rule.fixed.len();
    let ps = match b.get("points") {
        Some(v) => Some(v),
        None => match b.get("point") {
            Some(v) => Some(v),
            None => b.get("point_original_slots"),
        },
    }
    .filter(|v| !v.is_null());
    if let Some(ps) = ps {
        let list: Vec<Value> = match ps.as_array() {
            Some(a) if a.first().is_some_and(Value::is_i64) => vec![ps.clone()],
            Some(a) => a.clone(),
            None => return Err("provided samples invalid".into()),
        };
        let mut errors = Vec::new();
        for p in list {
            let pt: Option<Vec<i64>> = p
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_i64).collect());
            match pt {
                Some(pt) => match point_valid(rule, &pt, dims, chart, coefs) {
                    Ok(()) if locus.is_none() && !avoid.contains(&pt) => {
                        return Ok((vec![pt], "provided exact integer sample".into()));
                    }
                    Ok(()) => errors.push("provided sample already used or off the locus".into()),
                    Err(e) => errors.push(e),
                },
                None => errors.push("sample must be complete integer physical vector".into()),
            }
        }
        if locus.is_none() && avoid.is_empty() {
            return Err(format!(
                "provided samples invalid/poles: {}",
                errors.join("; ")
            ));
        }
    }
    let mut extended = rule.clone();
    let affine_locus = match locus {
        Some((text, e)) if e.degree()? <= 1 && !e.has_d() => {
            extended.equations.push(text.to_string());
            true
        }
        _ => false,
    };
    let (rows, piv) = rref_equations(&extended, n)?;
    let free: Vec<usize> = (0..n).filter(|i| !piv.contains(i)).collect();
    let mut rng = arith::PyRandom::new((0x525332 + rule.index) as u32);
    let mut found: Vec<Vec<i64>> = Vec::new();
    let bits: Vec<char> = rule.chart.chars().collect();
    let mut last = String::new();
    for trial in 0..attempts {
        let mut p = vec![Q::ZERO; n];
        for &i in &free {
            let z = if trial < 10 {
                1 + ((trial + i) % 5) as i64
            } else {
                rng.randint(1, 31.min(5 + (trial / 5) as i64))
            };
            p[i] = Q::int(if bits.get(i) == Some(&'1') { z } else { -z } as i128);
        }
        for (row, &i) in rows.iter().zip(&piv).rev() {
            let mut v = row[n];
            for &j in &free {
                v = v.sub(row[j].mul(p[j])?)?;
            }
            p[i] = v;
        }
        if p.iter().any(|x| x.d != 1) {
            continue;
        }
        let pt: Vec<i64> = p.iter().map(|x| x.n as i64).collect();
        if found.contains(&pt) || avoid.contains(&pt) {
            continue;
        }
        if let Some((_, e)) = locus.filter(|_| !affine_locus) {
            if !e.is_zero_at(&pt, Q::ZERO)? {
                continue;
            }
        }
        match point_valid(rule, &pt, dims, chart, coefs) {
            Ok(()) => {
                found.push(pt);
                if found.len() >= want {
                    break;
                }
            }
            Err(e) => last = e,
        }
    }
    if found.is_empty() {
        return Err(format!(
            "no valid non-pole integer sample in bounded search; not an emptiness proof; last=['{last}']"
        ));
    }
    Ok((
        found,
        if locus.is_some() {
            "deterministic affine-RREF integer search ON a witness locus".into()
        } else {
            "deterministic affine-RREF integer search".into()
        },
    ))
}

struct Built {
    rows: Vec<Row>,
    labels: Vec<Value>,
    universe: BTreeSet<Key>,
}

#[allow(clippy::too_many_arguments)]
fn rows_for(
    support: &Support,
    point: &[i64],
    recenter: &[i64],
    d: u64,
    f: &Family,
    cert: Option<&Cert>,
    chart: &str,
) -> Result<Built, Unknown> {
    rows_for_p(P, support, point, recenter, d, f, cert, chart)
}

/// [`rows_for`] modulo the prime `p` (guard-fix replay hardening: second prime).
#[allow(clippy::too_many_arguments)]
fn rows_for_p(
    p: u64,
    support: &Support,
    point: &[i64],
    recenter: &[i64],
    d: u64,
    f: &Family,
    cert: Option<&Cert>,
    chart: &str,
) -> Result<Built, Unknown> {
    let n = f.n;
    let mut out = Built {
        rows: Vec::new(),
        labels: Vec::new(),
        universe: BTreeSet::new(),
    };
    let mut seen_basis: BTreeSet<(usize, Key)> = BTreeSet::new();
    let mut seen_y: BTreeSet<Key> = BTreeSet::new();
    let at_pat = |pat: &Pat| -> Key {
        pat.iter()
            .enumerate()
            .map(|(i, (ax, off))| ax.map_or(*off, |a| point[a] + off + recenter[i]))
            .collect()
    };
    // (basis row or None, y)
    let items: Vec<(Option<usize>, Key)> = match support {
        Support::Exported(pats) => pats.iter().map(|p| (None, at_pat(p))).collect(),
        Support::Direct(descs) => descs
            .iter()
            .map(|(bid, p)| (Some(*bid), at_pat(p)))
            .collect(),
        Support::Retained(descs) | Support::RetainedSeeds(descs) => descs
            .iter()
            .map(|desc| {
                Ok((
                    Some(desc["basis_row"].as_u64().ok_or("descriptor basis_row")? as usize),
                    seed_at(desc, point, recenter, n)?,
                ))
            })
            .collect::<Result<_, Unknown>>()?,
    };
    let basis_path = matches!(support, Support::Retained(_) | Support::Direct(_));
    for (bid, y) in items {
        if basis_path {
            let bid = bid.expect("basis descriptor");
            if !seen_basis.insert((bid, y.clone())) {
                return Err("duplicate retained source descriptor".into());
            }
            let cert = cert.ok_or("basis support without certificate")?;
            let terms = cert
                .basis
                .get(&(chart.to_string(), bid))
                .ok_or("trace basis missing from authenticated certificate")?;
            let yp: Vec<u64> = y.iter().map(|&v| arith::from_i128(p, v as i128)).collect();
            let mut row = Row::new();
            for (m, coef) in terms {
                let cv = coef.at_mod(p, &yp, d)?;
                for (delta, form) in &cert.rows[m] {
                    let key: Key = y.iter().zip(delta).map(|(a, b)| a + b).collect();
                    out.universe.insert(key.clone());
                    addto_p(p, &mut row, key, arith::mul(p, cv, eval_form_p(p, form, &y, d)?));
                }
            }
            out.rows.push(strip_p(p, &row));
            out.labels.push(json!({"basis_row": bid, "at": y}));
        } else {
            if !seen_y.insert(y.clone()) {
                continue;
            }
            for (op, r) in f.rows_at(&y, d, p)?.into_iter().enumerate() {
                let mut rr = Row::new();
                for (delta, v) in r {
                    let key: Key = y.iter().zip(&delta).map(|(a, b)| a + b).collect();
                    out.universe.insert(key.clone());
                    addto_p(p, &mut rr, key, v);
                }
                out.rows.push(strip_p(p, &rr));
                out.labels
                    .push(json!({"independent_original_operator": op, "at": y}));
            }
        }
    }
    Ok(out)
}

fn reconstruct_direct(
    rule: &Rule,
    point: &[i64],
    dims: &[(String, u64)],
    f: &Family,
    cert: Option<&Cert>,
    coefs: &[Expr],
) -> Result<(Vec<i64>, Support, Value), Unknown> {
    if !rule.direct_hit {
        return Err("missing raw pivot for a non-direct rule".into());
    }
    let cert = cert
        .filter(|c| !c.basis.is_empty())
        .ok_or("direct-hit reconstruction requires authenticated basis derivations")?;
    if rule.seeds.len() != 1 {
        return Err("direct-hit reconstruction requires one retained exported seed".into());
    }
    let n = f.n;
    let pat = parse_integral(&rule.seeds[0], n)?;
    let tpat = parse_integral(&rule.target, n)?;
    if pat
        .iter()
        .zip(&tpat)
        .any(|(a, b)| a.0.is_none() != b.0.is_none())
    {
        return Err("direct seed/target symbolic patterns differ".into());
    }
    let target = key_at(&tpat, point);
    let rhskeys: Vec<Key> = rule
        .rhs
        .iter()
        .map(|(k, _)| parse_integral(k, n).map(|p| key_at(&p, point)))
        .collect::<Result<_, _>>()?;
    let mut desireds = Vec::new();
    for (_, d) in dims {
        let mut row = Row::new();
        row.insert(target.clone(), 1);
        for (k, c) in rhskeys.iter().zip(coefs) {
            let v = c.at_mod(
                P,
                &point
                    .iter()
                    .map(|&x| arith::from_i128(P, x as i128))
                    .collect::<Vec<_>>(),
                *d,
            )?;
            addto(&mut row, k.clone(), arith::neg(P, v));
        }
        desireds.push(strip(&row));
    }
    let nonzero = |k: &Key| !f.zero_key(k);
    let mut matches: BTreeMap<Vec<i64>, Vec<usize>> = BTreeMap::new();
    let mut tested = 0usize;
    let mut keys: Vec<&(String, usize)> = cert.basis.keys().collect();
    keys.sort();
    for key in keys {
        if key.0 != rule.chart {
            continue;
        }
        let bid = key.1;
        let terms = &cert.basis[key];
        let shifts: BTreeSet<&Key> = terms
            .iter()
            .flat_map(|(m, _)| cert.rows[m].keys())
            .collect();
        let mut candidates: BTreeSet<Vec<i64>> = BTreeSet::new();
        for delta in shifts {
            let mut rc = Vec::with_capacity(n);
            let mut ok = true;
            for (i, ((ax, off), (_, toff))) in pat.iter().zip(&tpat).enumerate() {
                if ax.is_none() {
                    if off + delta[i] != *toff {
                        ok = false;
                        break;
                    }
                    rc.push(0);
                } else {
                    rc.push(toff - off - delta[i]);
                }
            }
            if ok {
                candidates.insert(rc);
            }
        }
        for rc in candidates {
            tested += 1;
            let mut accepted = true;
            let sup = Support::Direct(vec![(bid, pat.clone())]);
            for ((_, d), wanted) in dims.iter().zip(&desireds) {
                let built = rows_for(&sup, point, &rc, *d, f, Some(cert), &rule.chart)?;
                let r = &built.rows[0];
                let pivot = r.get(&target).copied().unwrap_or(0);
                if pivot == 0 {
                    accepted = false;
                    break;
                }
                let inv = arith::inv(P, pivot).unwrap();
                let all: BTreeSet<&Key> = r.keys().chain(wanted.keys()).collect();
                if all.into_iter().any(|k| {
                    arith::sub(
                        P,
                        arith::mul(P, r.get(k).copied().unwrap_or(0), inv),
                        wanted.get(k).copied().unwrap_or(0),
                    ) != 0
                        && nonzero(k)
                }) {
                    accepted = false;
                    break;
                }
            }
            if accepted {
                matches.entry(rc).or_default().push(bid);
            }
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "direct-hit full-row reconstruction found {} distinct recenters; tested {tested}",
            matches.len()
        ));
    }
    let (rc, bids) = matches.into_iter().next().unwrap();
    let bid = *bids.iter().min().unwrap();
    let meta = json!({"kind": "independent full normalized one-basis-row agreement", "retained_exported_seed": rule.seeds[0], "basis_row": bid, "matching_basis_rows": bids, "candidate_recenters_tested": tested, "unique_recenter": true, "native_raw_pivot_claimed": false});
    Ok((rc, Support::Direct(vec![(bid, pat)]), meta))
}

/// Sparse elimination mod P (the independent witness solver): weights `w` with `desired - sum w_i rows_i` reduced to
/// its remainder; status IN_SPAN iff the remainder is empty.
pub struct Span {
    pub in_span: bool,
    pub rank: usize,
    pub weights: BTreeMap<usize, u64>,
    pub remainder: Vec<(usize, u64)>,
    /// source row index of every pivot, in elimination order (guard-fix --dump-weights)
    pub pivot_rows: Vec<usize>,
}

fn sparse_subtract(p: u64, a: &[(usize, u64)], b: &[(usize, u64)], f: u64) -> Vec<(usize, u64)> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if j == b.len() || (i < a.len() && a[i].0 < b[j].0) {
            out.push(a[i]);
            i += 1;
        } else if i == a.len() || b[j].0 < a[i].0 {
            let v = arith::mul(p, f, b[j].1);
            if v != 0 {
                out.push((b[j].0, p - v));
            }
            j += 1;
        } else {
            let v = arith::sub(p, a[i].1, arith::mul(p, f, b[j].1));
            if v != 0 {
                out.push((a[i].0, v));
            }
            i += 1;
            j += 1;
        }
    }
    out
}

/// The row echelon form of the replay rows (insertion order), reusable for several desired vectors
/// (the rule row and its re-solved mutants).
pub struct Elim<'r> {
    rows: &'r [Vec<(usize, u64)>],
    prime: u64,
    ps: Vec<ElimPivot>,
    bycol: Vec<Option<usize>>,
}

struct ElimPivot {
    row: Vec<(usize, u64)>,
    source: usize,
    inverse: u64,
    deps: Vec<(usize, u64)>,
}

impl<'r> Elim<'r> {
    pub fn new(rows: &'r [Vec<(usize, u64)>], ncols: usize, prime: u64) -> Result<Self, Unknown> {
        let mut ps: Vec<ElimPivot> = Vec::new();
        let mut bycol: Vec<Option<usize>> = vec![None; ncols];
        for (rid, original) in rows.iter().enumerate() {
            let mut r = original.clone();
            let mut deps = Vec::new();
            while let Some(&(c, f)) = r.first() {
                let Some(k) = bycol[c] else { break };
                deps.push((k, f));
                r = sparse_subtract(prime, &r, &ps[k].row, f);
            }
            if let Some(&(c, lead)) = r.first() {
                let iv = arith::inv(prime, lead).ok_or("zero inversion")?;
                for t in r.iter_mut() {
                    t.1 = arith::mul(prime, t.1, iv);
                }
                bycol[c] = Some(ps.len());
                ps.push(ElimPivot {
                    row: r,
                    source: rid,
                    inverse: iv,
                    deps,
                });
            }
        }
        Ok(Elim {
            rows,
            prime,
            ps,
            bycol,
        })
    }

    pub fn rank(&self) -> usize {
        self.ps.len()
    }

    pub fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let p = self.prime;
        let mut rem = desired.to_vec();
        let mut alpha = vec![0u64; self.ps.len()];
        while let Some(&(c, f)) = rem.first() {
            let Some(k) = self.bycol.get(c).copied().flatten() else {
                break;
            };
            alpha[k] = arith::add(p, alpha[k], f);
            rem = sparse_subtract(p, &rem, &self.ps[k].row, f);
        }
        let mut weights = vec![0u64; self.rows.len()];
        for i in (0..self.ps.len()).rev() {
            let w = arith::mul(p, alpha[i], self.ps[i].inverse);
            weights[self.ps[i].source] = arith::add(p, weights[self.ps[i].source], w);
            for &(j, f) in &self.ps[i].deps {
                if j >= i {
                    return Err("non-DAG dependency".into());
                }
                alpha[j] = arith::sub(p, alpha[j], arith::mul(p, w, f));
            }
        }
        let mut replay = desired.to_vec();
        for (j, w) in weights.iter().enumerate() {
            if *w != 0 {
                replay = sparse_subtract(p, &replay, &self.rows[j], *w);
            }
        }
        if replay != rem {
            return Err("independent original-row product differs from elimination remainder".into());
        }
        Ok(Span {
            in_span: rem.is_empty(),
            rank: self.ps.len(),
            weights: weights
                .into_iter()
                .enumerate()
                .filter(|(_, w)| *w != 0)
                .collect(),
            remainder: rem,
            pivot_rows: self.ps.iter().map(|q| q.source).collect(),
        })
    }
}

pub fn span(
    rows: &[Vec<(usize, u64)>],
    desired: &[(usize, u64)],
    ncols: usize,
) -> Result<Span, Unknown> {
    Elim::new(rows, ncols, P)?.solve(desired)
}

struct Ctx<'a> {
    args: &'a ReplayArgs,
    f: &'a Family,
    cert: Option<&'a Cert>,
    dims: Vec<(String, u64, Q)>,
}

fn replay_rule(
    rule: &Rule,
    b: &Value,
    ctx: &Ctx,
    chart: Option<&Value>,
    out: &Path,
    loci: &[String],
) -> Result<Value, Unknown> {
    let n = ctx.f.n;
    let coefs: Vec<Expr> = rule
        .rhs
        .iter()
        .map(|(_, c)| Expr::parse(c, n))
        .collect::<Result<_, _>>()?;
    let dq: Vec<Q> = ctx.dims.iter().map(|x| x.2).collect();
    let attempts = ctx.args.sample_attempts;
    let (mut points, kind) = sample_points(
        rule,
        b,
        &dq,
        chart,
        &coefs,
        attempts,
        1 + ctx.args.extra_points,
        None,
        &[],
    )?;
    let first = points.remove(0);
    let mut main = replay_point(rule, b, ctx, out, &first, kind.clone())?;
    let (boxed, box_capped) = if ctx.args.box_size > 0 {
        box_points(rule, &dq, chart, &coefs, ctx.args.box_size, ctx.args.box_max)?
    } else {
        (Vec::new(), false)
    };
    if points.is_empty() && loci.is_empty() && ctx.args.box_size == 0 {
        return Ok(main);
    }
    // guard-fix hardening: more generic points, then one point ON each witness locus
    let mut extra = Vec::new();
    let mut used = vec![first];
    let mut todo: Vec<(Vec<i64>, String, Option<String>)> = points
        .into_iter()
        .map(|p| (p, kind.clone(), None))
        .collect();
    let mut loci_report = Vec::new();
    for text in loci {
        let e = match Expr::parse(text, n) {
            Ok(e) => e,
            Err(err) => {
                loci_report.push(json!({"locus": text, "status": "UNKNOWN", "reason": err}));
                continue;
            }
        };
        // a locus with d: the index guard is where it vanishes for EVERY d (its d-coefficients); a constant
        // nonzero d-coefficient makes that empty. Not sampled here (the rule's exceptions carry the index part).
        if e.has_d() {
            loci_report.push(json!({"locus": text, "status": "parameter"}));
            continue;
        }
        // a locus that is one of the rule's own exception branches is guarded by construction
        if rule.exceptions.iter().any(|branch| branch.len() == 1 && same_poly(&branch[0], text, n)) {
            loci_report.push(json!({"locus": text, "status": "guarded"}));
            continue;
        }
        let avoid: Vec<Vec<i64>> = used.iter().chain(todo.iter().map(|t| &t.0)).cloned().collect();
        match sample_points(rule, b, &dq, chart, &coefs, attempts, 1, Some((text, &e)), &avoid) {
            Ok((mut ps, k)) => todo.push((ps.remove(0), k, Some(text.clone()))),
            Err(err) => loci_report.push(json!({"locus": text, "status": "no_point", "reason": err})),
        }
    }
    let box_total = boxed.len();
    for point in boxed {
        if !used.contains(&point) && !todo.iter().any(|t| t.0 == point) {
            todo.push((point, "box enumeration".into(), None));
        }
    }
    for (k, (point, kind, locus)) in todo.into_iter().enumerate() {
        let dir = out.join(format!("p{}", k + 1));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut v = match replay_point(rule, b, ctx, &dir, &point, kind) {
            Ok(v) => v,
            Err(e) => json!({"status": "UNKNOWN", "reason": e, "point": point}),
        };
        if let Some(l) = &locus {
            v["locus"] = json!(l);
            loci_report.push(json!({"locus": l, "status": v["status"], "point": point}));
        }
        used.push(point);
        extra.push(v);
    }
    let statuses: Vec<String> = std::iter::once(&main)
        .chain(extra.iter())
        .map(|v| v["status"].as_str().unwrap_or("UNKNOWN").to_string())
        .collect();
    let overall = if statuses.iter().any(|s| s.starts_with("FAIL")) {
        "FAIL_RS2_RS3_SAMPLED".to_string()
    } else if statuses.iter().all(|s| s == "PASS_RS2_RS3_SAMPLED") {
        "PASS_RS2_RS3_SAMPLED".to_string()
    } else {
        statuses[0].clone()
    };
    main["first_point_status"] = main["status"].clone();
    main["status"] = json!(overall);
    main["extra_points"] = Value::Array(extra);
    main["loci"] = Value::Array(loci_report);
    if ctx.args.box_size > 0 {
        main["box"] = json!({"size": ctx.args.box_size, "points": box_total, "capped": box_capped,
            "max": ctx.args.box_max});
    }
    Ok(main)
}

/// Equal polynomials up to a nonzero constant factor (two-prime check at a few points).
fn same_poly(a: &str, b: &str, n: usize) -> bool {
    let (Ok(x), Ok(y)) = (Expr::parse(a, n), Expr::parse(b, n)) else {
        return false;
    };
    // compare x/y at a few deterministic points: constant ratio and same zero pattern
    let mut ratio: Option<(u64, u64)> = None;
    for t in 1..=6i64 {
        let point: Vec<i64> = (0..n as i64).map(|i| (t * 7 + i * 3) % 11 - 5).collect();
        let (Ok(u), Ok(v)) = (x.at_point(P, &point, Q::int(t as i128 + 2)), y.at_point(P, &point, Q::int(t as i128 + 2))) else {
            return false;
        };
        match ratio {
            None => ratio = Some((u, v)),
            Some((u0, v0)) => {
                if arith::mul(P, u, v0) != arith::mul(P, u0, v) {
                    return false;
                }
            }
        }
    }
    ratio.is_some_and(|(u, v)| (u == 0) == (v == 0))
}

fn replay_point(
    rule: &Rule,
    b: &Value,
    ctx: &Ctx,
    out: &Path,
    point: &[i64],
    sample_kind: String,
) -> Result<Value, Unknown> {
    let started = Instant::now();
    let point = point.to_vec();
    let n = ctx.f.n;
    if let Some(x) = b.get("fixed") {
        if x != &rule.raw["fixed"] {
            return Err("binding fixed differs from actual rule".into());
        }
    }
    if let Some(x) = b.get("equations") {
        if x != &json!(rule.equations) {
            return Err("binding equations differs from actual rule".into());
        }
    }
    let coefs: Vec<Expr> = rule
        .rhs
        .iter()
        .map(|(_, c)| Expr::parse(c, n))
        .collect::<Result<_, _>>()?;
    let target = key_at(&parse_integral(&rule.target, n)?, &point);
    let rhskeys: Vec<Key> = rule
        .rhs
        .iter()
        .map(|(k, _)| parse_integral(k, n).map(|p| key_at(&p, &point)))
        .collect::<Result<_, _>>()?;
    if ctx.f.zero_key(&target) {
        return Err("target is independently scaleless; not a nonzero-rule test".into());
    }
    let (candidates, authority, sup, supp_meta) = match recenter_for(rule, b, n).and_then(
        |(rc, a)| {
            support(rule, b, &ctx.args.cand_dump_dir, n, ctx.cert).map(|(s, m)| (vec![rc], a, s, m))
        },
    ) {
        Ok(x) => x,
        Err(e)
            if e.starts_with("missing symbolic raw pivot/recenter")
                && rule.direct_hit
                && !(recenter_search() && ctx.cert.is_none()) =>
        {
            let dm: Vec<(String, u64)> = ctx.dims.iter().map(|x| (x.0.clone(), x.1)).collect();
            let (rc, s, m) = reconstruct_direct(rule, &point, &dm, ctx.f, ctx.cert, &coefs)?;
            (vec![rc], "independent sampled one-source-row reconstruction; not an authenticated native raw pivot".to_string(), s, m)
        }
        Err(e) if e.starts_with("missing symbolic raw pivot/recenter") && recenter_search() => {
            // p5l-close: no recenter authority (table_dump tables). Span membership at a point does not depend
            // on how the seeds were placed, so the recenter may be SEARCHED: candidates are the translations that
            // put the target and every non-scaleless RHS key into the key universe of the exported seeds' original
            // IBPs; each candidate is replayed in full (3 d, mutation control) and the first PASS is the witness.
            let (s, m) = support(rule, b, &ctx.args.cand_dump_dir, n, ctx.cert)?;
            let rhsvals: Vec<u64> = coefs
                .iter()
                .map(|c| c.at_point(P, &point, ctx.dims[0].2))
                .collect::<Result<_, _>>()?;
            let cands = recenter_candidates(&s, &point, ctx, &target, &rhskeys, &rhsvals, rule)?;
            if cands.is_empty() {
                return Err(
                    "recenter search: no translation puts target and RHS into the seed universe"
                        .into(),
                );
            }
            let k = cands.len();
            (
                cands,
                format!(
                    "span-verified recenter search ({k} candidates; RR_REPLAY_RECENTER_SEARCH)"
                ),
                s,
                m,
            )
        }
        Err(e) => return Err(e),
    };
    let mut tried = Vec::new();
    let mut last = None;
    for rc in candidates.iter().take(recenter_search_limit()) {
        let cells = replay_cells(rule, ctx, &point, rc, &sup, &coefs, &target, &rhskeys, out)?;
        let pass = cells.iter().all(|c| c["result"] == "PASS");
        tried.push(json!({"recenter": rc, "pass": pass}));
        last = Some((rc.clone(), cells));
        if pass {
            break;
        }
    }
    let (rc, cells) = last.ok_or("no recenter candidate replayed")?;
    let pass = cells.iter().all(|c| c["result"] == "PASS");
    let fail = cells.iter().any(|c| c["result"] == "FAIL");
    // FAIL = the rule row is not in the span of these seeds' rows at this point (not derived from these seeds; NOT a
    // statement that the rule is false). CONTROL_FAILED = in the span, but so is the re-solved +1 mutant: no content.
    Ok(json!({
        "status": if pass { "PASS_RS2_RS3_SAMPLED" } else if fail { "FAIL_RS2_RS3_SAMPLED" } else { "UNKNOWN_CONTROL_FAILED" },
        "fail_meaning": "FAIL = not derived from these seeds at this point (mod-P span test); not evidence that the rule is false",
        "point": point, "sample_authority": sample_kind, "recenter": rc, "recenter_authority": authority,
        "recenter_tried": tried,
        "support_kind": sup.kind(), "support": supp_meta, "cells": cells, "elapsed_seconds": started.elapsed().as_secs_f64(),
    }))
}

fn recenter_search() -> bool {
    std::env::var("RR_REPLAY_RECENTER_SEARCH").is_ok_and(|v| v == "1")
}

fn recenter_search_limit() -> usize {
    std::env::var("RR_REPLAY_RECENTER_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6)
}

/// Translations r (zero on numeric axes of the target) with target - r and every non-scaleless RHS key - r in the
/// key universe of the support placed with recenter 0; sorted by |r|_1.
fn recenter_candidates(
    sup: &Support,
    point: &[i64],
    ctx: &Ctx,
    target: &Key,
    rhskeys: &[Key],
    rhsvals: &[u64],
    rule: &Rule,
) -> Result<Vec<Vec<i64>>, Unknown> {
    let n = ctx.f.n;
    let zero = vec![0i64; n];
    let d = ctx.dims.first().ok_or("no dimension")?.1;
    let built = rows_for(sup, point, &zero, d, ctx.f, ctx.cert, &rule.chart)?;
    let pat = parse_integral(&rule.target, n)?;
    let numeric: Vec<bool> = pat.iter().map(|(ax, _)| ax.is_none()).collect();
    let mut cands: Vec<Vec<i64>> = built
        .universe
        .iter()
        .map(|u| {
            target
                .iter()
                .zip(u)
                .map(|(t, u)| t - u)
                .collect::<Vec<i64>>()
        })
        .filter(|r: &Vec<i64>| r.iter().zip(&numeric).all(|(x, num)| !num || *x == 0))
        .collect();
    cands.sort_by_key(|r| (r.iter().map(|x| x.abs()).sum::<i64>(), r.clone()));
    cands.dedup();
    // a RHS key with a nonzero coefficient at this point must be touched by the rows (a vanishing coefficient
    // drops its term); ranking only: if no candidate meets every RHS key, the target-only candidates are tried
    // in |r|_1 order (each is a full span check, so a wrong translation can only fail, never pass)
    let score = |r: &Vec<i64>| -> usize {
        rhskeys
            .iter()
            .zip(rhsvals)
            .filter(|(k, v)| {
                **v != 0 && !ctx.f.zero_key(k) && {
                    let u: Key = k.iter().zip(r).map(|(a, b)| a - b).collect();
                    !built.universe.contains(&u)
                }
            })
            .count()
    };
    let mut scored: Vec<(usize, Vec<i64>)> = cands.into_iter().map(|r| (score(&r), r)).collect();
    scored.sort_by_key(|(m, r)| (*m, r.iter().map(|x| x.abs()).sum::<i64>()));
    Ok(scored.into_iter().map(|(_, r)| r).collect())
}

#[allow(clippy::too_many_arguments)]
fn replay_cells(
    rule: &Rule,
    ctx: &Ctx,
    point: &[i64],
    rc: &[i64],
    sup: &Support,
    coefs: &[Expr],
    target: &Key,
    rhskeys: &[Key],
    out: &Path,
) -> Result<Vec<Value>, Unknown> {
    let (point, rc, target) = (point.to_vec(), rc.to_vec(), target.clone());
    let mut cells = Vec::new();
    // guard-fix replay hardening (helper Y24): each cell runs at the replay prime P AND the second prime P2
    // (unless --single-prime); d is either one of the fixed dimensions (--dimensions / --fixed-d) or drawn at random
    // in F_p per cell (the CLI default), deterministically from (rule, point, cell, prime) so a run is reproducible.
    let primes: Vec<u64> = if ctx.args.single_prime {
        vec![P]
    } else {
        vec![P, arith::P2]
    };
    let cell_dims: Vec<(String, Option<Q>)> = if ctx.args.random_d {
        (0..ctx.dims.len().max(1))
            .map(|k| (format!("random{k}"), None))
            .collect()
    } else {
        ctx.dims.iter().map(|(ds, _, q)| (ds.clone(), Some(*q))).collect()
    };
    let point_hash = {
        let mut h: u32 = 0x811c_9dc5;
        for byte in format!("{}|{}|{:?}", rule.chart, rule.index, point).bytes() {
            h ^= byte as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
        h
    };
    let point_mod = |p: u64| -> Vec<u64> {
        point.iter().map(|&x| arith::from_i128(p, x as i128)).collect()
    };
    for (ck, (ds, dq)) in cell_dims.iter().enumerate() {
      for &p in &primes {
        let d: u64 = match dq {
            Some(q) => q.modp(p).ok_or("dimension pole")?,
            None => {
                let mut rng = arith::PyRandom::new(
                    point_hash ^ (ck as u32).wrapping_mul(0x9e37_79b9) ^ if p == P { 0 } else { 0x5bd1_e995 },
                );
                let hi = rng.getrandbits(32) as u64;
                let lo = rng.getrandbits(32) as u64;
                2 + ((hi << 32) | lo) % (p - 3)
            }
        };
        let mut desired = Row::new();
        desired.insert(target.clone(), 1);
        let pm = point_mod(p);
        let values: Vec<u64> = coefs
            .iter()
            .map(|c| c.at_mod(p, &pm, d))
            .collect::<Result<_, _>>()?;
        for (k, v) in rhskeys.iter().zip(&values) {
            addto_p(p, &mut desired, k.clone(), arith::neg(p, *v));
        }
        let built = rows_for_p(p, sup, &point, &rc, d, ctx.f, ctx.cert, &rule.chart)?;
        let mut universe = built.universe.clone();
        universe.insert(target.clone());
        universe.extend(rhskeys.iter().cloned());
        let columns: Vec<Key> = universe.into_iter().collect();
        let ids: HashMap<&Key, usize> = columns.iter().enumerate().map(|(i, k)| (k, i)).collect();
        let zeros: BTreeSet<Key> = columns
            .iter()
            .filter(|k| ctx.f.zero_key(k))
            .cloned()
            .collect();
        let enc = |row: &Row| -> Vec<(usize, u64)> {
            let mut es: Vec<(usize, u64)> = strip_p(p, row)
                .into_iter()
                .filter(|(k, _)| !zeros.contains(k))
                .map(|(k, v)| (ids[&k], v))
                .collect();
            es.sort_unstable();
            es
        };
        let enc_rows: Vec<Vec<(usize, u64)>> = built.rows.iter().map(enc).collect();
        let elim = Elim::new(&enc_rows, columns.len(), p)?;
        let enc_desired = enc(&desired);
        let sp = elim.solve(&enc_desired)?;
        let mut raw = desired.clone();
        for (i, w) in &sp.weights {
            for (k, v) in &built.rows[*i] {
                addto_p(p, &mut raw, k.clone(), arith::neg(p, arith::mul(p, *w, *v)));
            }
        }
        let raw = strip_p(p, &raw);
        let bad: Row = raw
            .iter()
            .filter(|(k, _)| !zeros.contains(*k))
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        let projected: Row = sp
            .remainder
            .iter()
            .map(|(c, v)| (columns[*c].clone(), *v))
            .collect();
        if bad != projected {
            return Err("full-column replay differs from elimination".into());
        }
        let mutidx = rhskeys
            .iter()
            .zip(&values)
            .position(|(k, v)| !zeros.contains(k) && *v != 0)
            .ok_or("no nonzero non-scaleless RHS coefficient for required +1 mutation")?;
        // guard-fix (Y24): the +1 mutant is RE-SOLVED against the same echelon form (rule + delta e_k in the span?).
        // A PASS cell whose mutant is also in the span has no content (the RHS integral k itself lies in the span).
        let mut mutant_desired = desired.clone();
        addto_p(p, &mut mutant_desired, rhskeys[mutidx].clone(), p - 1);
        let mutant_span = elim.solve(&enc(&mutant_desired))?;
        let mutant_rejected = !mutant_span.in_span;
        // guard-fix (Y24 control 2): no known nonzero integral lies in the span. Per non-zero sector touched by the
        // columns, its lowest column WITHOUT numerators (every line of the sector at power >= 1, every other slot at
        // exactly 0): a Euclidean integral with a positive integrand, nonzero for generic d. Such a column in the span
        // would mean the rows (or the zero list) prove a nonzero integral = 0.
        let mut sectors_touched: BTreeSet<Vec<bool>> = BTreeSet::new();
        let mut probe_of: BTreeMap<Vec<bool>, (i64, usize)> = BTreeMap::new();
        for (c, k) in columns.iter().enumerate() {
            if zeros.contains(k) {
                continue;
            }
            let sec: Vec<bool> = k.iter().map(|&x| x >= 1).collect();
            sectors_touched.insert(sec.clone());
            if k.iter().all(|&x| x >= 1 || x == 0) {
                let weight: i64 = k.iter().sum();
                let e = probe_of.entry(sec).or_insert((weight, c));
                if weight < e.0 {
                    *e = (weight, c);
                }
            }
        }
        let (mut corners_tested, mut corners_in_span) = (0usize, Vec::new());
        for (_, c) in probe_of.values() {
            corners_tested += 1;
            if elim.solve(&[(*c, 1)])?.in_span {
                corners_in_span.push(columns[*c].clone());
            }
        }
        let nonzero_control_ok = corners_in_span.is_empty();
        let nonzero_sector_columns = columns.len() - zeros.len();
        let content = sp.rank < nonzero_sector_columns;
        let derived = sp.in_span && bad.is_empty();
        let ok = derived && mutant_rejected && nonzero_control_ok;
        let plabel = if p == P { "P" } else { "P2" };
        let label = format!("{}-{plabel}", ds.replace('/', "_"));
        let cell = json!({
            "dimension": ds, "dimension_mod_p": d.to_string(), "prime": p, "prime_label": plabel, "point": point,
            "target": target, "recenter": rc, "source_rows": built.rows.len(),
            "rhs_terms": rhskeys.len(), "full_column_universe": columns.len(), "zero_sector_columns": zeros.len(),
            "nonzero_sector_columns": nonzero_sector_columns,
            "rank_below_nonzero_sector_columns": content,
            "raw_residual": entries(&raw), "nonzero_sector_residual": entries(&bad),
            "mutation": {"rhs_index": mutidx, "coefficient_delta": 1, "same_recomputed_witness": false, "re_solved": true, "rejected": mutant_rejected},
            "nonzero_integral_control": {"sectors_touched": sectors_touched.len(), "corners_tested": corners_tested,
                "corners_in_span": corners_in_span, "ok": nonzero_control_ok},
            "elimination": {"status": if sp.in_span { "IN_SPAN" } else { "NOT_IN_SPAN" }, "rank": sp.rank, "weights": sp.weights.len()},
        });
        let cp = out.join(format!("cell-{label}.json"));
        std::fs::write(&cp, serde_json::to_string_pretty(&cell).unwrap() + "\n")
            .map_err(|e| e.to_string())?;
        if ctx.args.dump_weights {
            // guard-fix --dump-weights (Y24 exact upgrade): every source row (index, absolute seed, operator / basis
            // row), the pivot rows, the nonzero weights mod p, the column keys, the zero-sector keys and the desired
            // rule row mod p (target +1, RHS -c_i), all in the column numbering of this cell.
            let row_entries: Vec<Value> = built
                .labels
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let mut v = l.clone();
                    v["row"] = json!(i);
                    v
                })
                .collect();
            let weights: Vec<Value> = sp.weights.iter().map(|(i, w)| json!([i, w.to_string()])).collect();
            let desired_cols: Vec<Value> = enc_desired.iter().map(|(c, v)| json!([c, v.to_string()])).collect();
            let zero_keys: Vec<&Key> = zeros.iter().collect();
            let dump = json!({
                "schema": "guard-fix.replay-weights.v1", "prime": p.to_string(), "dimension": ds, "dimension_mod_p": d.to_string(),
                "point": point, "target": target, "recenter": rc, "chart": rule.chart, "rule": rule.index,
                "rows": row_entries, "pivot_rows": sp.pivot_rows, "rank": sp.rank,
                "weights": weights, "weights_note": "weights[i] = [row, w mod p as decimal string]; desired - sum w_i row_i = 0 on every nonzero-sector column",
                "columns": columns, "zero_sector_keys": zero_keys, "desired": desired_cols,
                "in_span": sp.in_span,
            });
            let wp = out.join(format!("weights-{label}.json"));
            std::fs::write(&wp, serde_json::to_string(&dump).unwrap() + "\n").map_err(|e| e.to_string())?;
        }
        let result = if ok {
            "PASS"
        } else if !derived {
            "FAIL"
        } else {
            "CONTROL_FAILED"
        };
        let summary = json!({"dimension": ds, "prime": plabel, "result": result, "path": cp.display().to_string(), "source_rows": built.rows.len(), "rank": sp.rank, "full_columns": columns.len(), "nonzero_sector_columns": nonzero_sector_columns, "rank_below_nonzero_sector_columns": content, "raw_residual_count": raw.len(), "nonzero_sector_residual_count": bad.len(), "mutation_rejected": mutant_rejected, "mutation_re_solved": true, "nonzero_control_ok": nonzero_control_ok, "corners_tested": corners_tested});
        eprintln!(
            "{}",
            json!({"stage": "cell_complete", "chart": rule.chart, "rule": rule.index, "cell": summary})
        );
        cells.push(summary);
      }
    }
    Ok(cells)
}

fn family_check(header: &Value, text: &str, f: &Family) -> Result<(), Unknown> {
    let embedded = header["family_source"]
        .as_str()
        .ok_or("table header lacks full family_source")?;
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    if squash(text) != squash(embedded) {
        return Err("family differs from table physical-slot source".into());
    }
    let c = squash(text);
    if !c.contains("externals(q)") || !c.contains("gram(q,q,1)") {
        return Err("only massless one-scale q^2=1 families supported".into());
    }
    if !c.contains("dimension(d)") {
        return Err("dimension must be d".into());
    }
    if c.matches("prop(").count() != f.n {
        return Err("unsupported propagator grammar/mass".into());
    }
    if header["denominators"].as_u64() != Some(f.n as u64)
        || header["loops"].as_u64() != Some(f.loops as u64)
    {
        return Err("family dimension metadata mismatch".into());
    }
    if header["parameters"] != json!(["d"]) {
        return Err("unsupported extra coefficient parameters".into());
    }
    if header["source_conditions"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        return Err(
            "nonempty source conditions require an explicit independent implementation".into(),
        );
    }
    Ok(())
}

/// Run the replay; writes `OUT/RESULT.json` and per rule `OUT/<chart>-r<index>/RESULT.json`.
/// Returns the overall status (`PASS_RS2_RS3_SAMPLED`, `FAIL`, `UNKNOWN`) and the result document.
pub fn run(args: &ReplayArgs) -> Result<(String, Value), String> {
    if args.out.exists() {
        return Err("output directory already exists".into());
    }
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let mut result = json!({"schema": "p5l.replay-result.v1", "tool": VERSION, "prime": P, "dimensions": args.dimensions,
        "scope": "Finite exact modular RS2/RS3 only. Completeness, LP/master and strict descent are separate gates.", "verified": false, "rules": []});
    let inner = (|| -> Result<Vec<Value>, Unknown> {
        let text = std::fs::read_to_string(&args.table).map_err(|e| format!("table: {e}"))?;
        let mut header: Option<Value> = None;
        let mut charts: HashMap<String, Value> = HashMap::new();
        let mut rules: Vec<Rule> = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let r: Value = serde_json::from_str(line).map_err(|e| format!("table json: {e}"))?;
            match r["type"].as_str() {
                Some("header") => {
                    if header.is_some() {
                        return Err("duplicate table header".into());
                    }
                    header = Some(r);
                }
                Some("chart") => {
                    charts.insert(r["chart"].as_str().unwrap_or_default().to_string(), r);
                }
                Some("rule") => {
                    let idx = r["index"].as_u64().unwrap_or(u64::MAX) as usize;
                    let chart_ok = args
                        .chart
                        .as_ref()
                        .is_none_or(|c| r["chart"].as_str() == Some(c));
                    if chart_ok && args.rules.as_ref().is_none_or(|w| w.contains(&idx)) {
                        rules.push(rule_from(&r)?);
                    }
                }
                _ => {}
            }
        }
        let header = header
            .filter(|_| !rules.is_empty())
            .ok_or("no header/selected rules")?;
        let family_text = match &args.family {
            Some(p) => std::fs::read_to_string(p).map_err(|e| format!("family: {e}"))?,
            None => header["family_source"]
                .as_str()
                .ok_or("table header lacks full family_source")?
                .to_string(),
        };
        let f = Family::parse(&family_text)?;
        family_check(&header, &family_text, &f)?;
        let (cert, auth) = match &args.cert {
            Some(p) => {
                let (c, a) = source_basis(p, &f)?;
                (Some(c), a)
            }
            None => (
                None,
                json!({"kind": "independent full original IBPs; no native basis supplied"}),
            ),
        };
        let bindings = get_bindings(&args.bindings)?;
        let mut loci_of: HashMap<(String, usize), Vec<String>> = HashMap::new();
        if let Some(path) = &args.loci {
            let text = std::fs::read_to_string(path).map_err(|e| format!("loci: {e}"))?;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let v: Value = serde_json::from_str(line).map_err(|e| format!("loci json: {e}"))?;
                let key = (
                    v["chart"].as_str().unwrap_or_default().to_string(),
                    v["index"].as_u64().ok_or("loci record without index")? as usize,
                );
                let list = v["loci"].as_array().ok_or("loci record without loci")?;
                loci_of
                    .entry(key)
                    .or_default()
                    .extend(list.iter().filter_map(|x| x.as_str().map(str::to_string)));
            }
        }
        let dims: Vec<(String, u64, Q)> = args
            .dimensions
            .iter()
            .map(|ds| {
                let q = Q::parse(ds)?;
                Ok((ds.clone(), q.modp(P).ok_or("dimension pole")?, q))
            })
            .collect::<Result<_, Unknown>>()?;
        let ctx = Ctx {
            args,
            f: &f,
            cert: cert.as_ref(),
            dims,
        };
        let mut recs = Vec::new();
        for rule in &rules {
            let rd = args.out.join(format!("{}-r{}", rule.chart, rule.index));
            std::fs::create_dir_all(&rd).map_err(|e| e.to_string())?;
            let canonical = serde_json::to_string(&rule.raw).unwrap();
            let mut rec = json!({"chart": rule.chart, "index": rule.index, "rule_record_blake3": blake3::hash(canonical.as_bytes()).to_hex().to_string(),
                "dimensions": args.dimensions, "rhs_terms": rule.rhs.len(), "fixed": rule.raw["fixed"], "equations": rule.equations});
            let outcome = binding_for(&bindings, rule)
                .and_then(|b| {
                    let mut loci: Vec<String> = loci_of
                        .get(&(rule.chart.clone(), rule.index))
                        .cloned()
                        .unwrap_or_default();
                    if let Some(own) = rule.raw["witness_loci"].as_array() {
                        loci.extend(own.iter().filter_map(|x| x.as_str().map(str::to_string)));
                    }
                    loci.sort();
                    loci.dedup();
                    replay_rule(rule, &b, &ctx, charts.get(&rule.chart), &rd, &loci)
                });
            match outcome {
                Ok(v) => {
                    for (k, x) in v.as_object().unwrap() {
                        rec[k] = x.clone();
                    }
                }
                Err(e) => {
                    rec["status"] = json!("UNKNOWN");
                    rec["reason"] = json!(e);
                }
            }
            std::fs::write(
                rd.join("RESULT.json"),
                serde_json::to_string_pretty(&rec).unwrap() + "\n",
            )
            .map_err(|e| e.to_string())?;
            eprintln!(
                "{}",
                json!({"stage": "rule_complete", "chart": rule.chart, "index": rule.index, "status": rec["status"], "reason": rec.get("reason")})
            );
            recs.push(rec);
        }
        Ok(vec![auth, Value::Array(recs)])
    })();
    let status = match inner {
        Ok(v) => {
            result["source_authentication"] = v[0].clone();
            let recs = v[1].as_array().cloned().unwrap_or_default();
            let ss: Vec<String> = recs
                .iter()
                .map(|r| r["status"].as_str().unwrap_or("UNKNOWN").to_string())
                .collect();
            result["rules"] = Value::Array(recs);
            if ss.iter().any(|s| s.starts_with("FAIL")) {
                "FAIL".to_string()
            } else if ss.iter().all(|s| s == "PASS_RS2_RS3_SAMPLED") {
                "PASS_RS2_RS3_SAMPLED".to_string()
            } else {
                "UNKNOWN".to_string()
            }
        }
        Err(e) => {
            result["reason"] = json!(e);
            "UNKNOWN".to_string()
        }
    };
    result["status"] = json!(status);
    let counts = |s: &str| {
        result["rules"]
            .as_array()
            .map_or(0, |r| r.iter().filter(|x| x["status"] == s).count())
    };
    result["counts"] = json!({"PASS": counts("PASS_RS2_RS3_SAMPLED"), "FAIL": counts("FAIL_RS2_RS3_SAMPLED"), "UNKNOWN": counts("UNKNOWN")});
    std::fs::write(
        args.out.join("RESULT.json"),
        serde_json::to_string_pretty(&result).unwrap() + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok((status, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_solver_weights_and_remainder() {
        // rows: e0 + e1, e1 + e2 ; desired e0 - e2 = row0 - row1 ; desired2 = e0 (not in span)
        let rows = vec![vec![(0, 1), (1, 1)], vec![(1, 1), (2, 1)]];
        let s = span(&rows, &[(0, 1), (2, P - 1)], 3).unwrap();
        assert!(s.in_span && s.rank == 2);
        assert_eq!(s.weights.get(&0), Some(&1));
        assert_eq!(s.weights.get(&1), Some(&(P - 1)));
        let s2 = span(&rows, &[(0, 1)], 3).unwrap();
        assert!(!s2.in_span);
    }

    #[test]
    fn keys_powers_and_rules_spec() {
        assert_eq!(
            parse_integral("I(1,n1-2,0)", 3).unwrap(),
            vec![(None, 1), (Some(1), -2), (None, 0)]
        );
        assert!(parse_integral("I(1,n2,0)", 3).is_err());
        assert_eq!(power_decode(255).unwrap(), (true, -1));
        assert_eq!(power_decode(127).unwrap(), (false, -1));
        assert_eq!(power_decode(128).unwrap(), (true, 0));
        let r = ReplayArgs::parse_rules("0-2,7").unwrap().unwrap();
        assert_eq!(r.into_iter().collect::<Vec<_>>(), vec![0, 1, 2, 7]);
        assert!(ReplayArgs::parse_rules("all").unwrap().is_none());
    }
}
