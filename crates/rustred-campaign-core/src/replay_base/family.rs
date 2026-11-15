//! COPY (not a move) of rustred-core/src/node/replay/family.rs at engine commit b9f06de7 for the campaign's independent
//! A2 row check (plan 3.1 (e)); changes are listed in interfaces/ibp-rows.md section 5 and marked "campaign copy".
//!
//! Independent IBP generator of the replay (port of p5l-replay.v1 `checker/gibp.py`): reads only the family text,
//! builds the IBP operators exactly (small rationals), evaluates rows mod p, and decides scaleless sectors by Lee's
//! criterion on U + F computed from the propagators. No RustRed solver or Symbolica code is used.
//!
//! campaign copy, CHANGED (rows3, DEV-17 REVISED): DOMAIN LOGIC only. Every arithmetic step (rationals, the inverse of
//! the scalar-product matrix, the field operations of the rows, Lee's affine-hull test, the U / F determinants) goes
//! through `replay_base::algebra::CheckAlgebra` (`*_with(alg, ..)`; the plain methods run the certificate backend
//! `Independent`); the algebra itself lives in `crate::independent_algebra` (or the Symbolica backend).

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex; // campaign copy: Mutex instead of RefCell (Family: Sync)

use super::algebra::{AlgebraOrigin, CheckAlgebra, Q, Recorder, Stage, Unknown, ZPoly};
use super::certificate::CertAlg;
use crate::independent_algebra::Independent;

/// One IBP operator: `diag` (j == v) and per propagator `(t0, [(m, x)])`.
type OpTable = (bool, Vec<(Q, Vec<(usize, Q)>)>);

/// The family model. All fields are PRIVATE (re-audit A NEW-M3): a model can only come from [`Family::parse`] of a
/// text, and its digests / check result cannot be edited after the fact.
pub struct Family {
    n: usize,
    loops: usize,
    kshift: Vec<i64>,
    eps_axes: Vec<usize>,
    /// Momentum coefficients `c[i][v]`, `v` over loops then `q`.
    c: Vec<Vec<i64>>,
    ops: Vec<OpTable>,
    zero_cache: Mutex<HashMap<Vec<bool>, bool>>,
    /// campaign copy, ADDED (audit C2): sha256 of the family text bytes, the `name(..)` and the result of the
    /// header-free part of the engine's `family_check` plus the whitelist of [`family_text_check`].
    source_digest: String,
    /// sha256 of the text with all whitespace removed (the engine compares `family_source` up to whitespace)
    squashed_digest: String,
    name: Option<String>,
    check: Result<(), Unknown>,
    /// campaign copy, ADDED: the loop momentum names in `loops(..)` order (symmetry-group data refers to them)
    loop_names: Vec<String>,
    /// campaign copy, ADDED (rows3): the text the model was parsed from (a second backend re-parses it)
    source: String,
    /// rows4 (re-audit RA3-M1): which algebra built the operator table (certificate only through [`Family::parse`] /
    /// [`Family::parse_traced`]; [`Family::parse_with`] is always diagnostic)
    origin: AlgebraOrigin,
    /// rows4: sha256 of the operator table and the inverse scalar-product matrix (what a re-parse must reproduce)
    table_digest: String,
}

impl Family {
    /// number of slots (propagators and numerators)
    pub fn n(&self) -> usize {
        self.n
    }
    pub fn loops(&self) -> usize {
        self.loops
    }
    /// per slot: the integer k of the power shift k (4-d)/2 (0 = none)
    pub fn kshift(&self) -> &[i64] {
        &self.kshift
    }
    pub fn eps_axes(&self) -> &[usize] {
        &self.eps_axes
    }
    /// sha256 of the family text bytes
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
    /// sha256 of the text without whitespace (what a table header's `family_source` is compared with)
    pub fn squashed_digest(&self) -> &str {
        &self.squashed_digest
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    /// the header-free family check (text check + whitelist): `Err` = outside what the replay models
    pub fn check(&self) -> &Result<(), Unknown> {
        &self.check
    }
    pub fn loop_names(&self) -> &[String] {
        &self.loop_names
    }
    /// the family text this model was parsed from
    pub fn source(&self) -> &str {
        &self.source
    }
    /// which algebra built this model (a certificate run refuses a diagnostic origin)
    pub fn origin(&self) -> AlgebraOrigin {
        self.origin
    }
    /// sha256 of the operator table (+ inverse scalar-product matrix) of this model
    pub fn table_digest(&self) -> &str {
        &self.table_digest
    }
}

/// Top-level calls of a whitespace-free family text `I(c1,c2,..)` as (head, argument text).
fn top_level_calls(compact: &str) -> Result<Vec<(&str, &str)>, Unknown> {
    let inner = compact
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .ok_or("family text is not I(...)")?;
    let (mut depth, mut start) = (0i32, 0usize);
    let mut calls: Vec<&str> = Vec::new();
    for (k, &ch) in inner.as_bytes().iter().enumerate() {
        match ch {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                calls.push(&inner[start..k]);
                start = k + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return Err("unbalanced parentheses".into());
        }
    }
    if depth != 0 {
        return Err("unbalanced parentheses".into());
    }
    calls.push(&inner[start..]);
    calls
        .into_iter()
        .map(|call| {
            let open = call.find('(').filter(|_| call.ends_with(')'));
            match open {
                Some(o) => Ok((&call[..o], &call[o + 1..call.len() - 1])),
                None => Err(format!("family element is not a call: {call:.60}")),
            }
        })
        .collect()
}

/// campaign copy, ADDED (audit C2; re-audit A NEW-M2 2026-11-14): the engine replay's `family_check`
/// (replay/mod.rs:1979-2014 at b9f06de7) without the table-header part (that is [`family_header_check`]), checked on
/// the TOP-LEVEL calls (a substring test let `name(gram(q,q,1))` pass): exactly one `loops(..)`, `externals(q)`,
/// `dimension(d)`, `gram(q,q,1)`, at most one `name(..)`, exactly `n` `prop(..)` with flag 0 or 1, `power_shift` only;
/// any other construct of the engine's family grammar is refused BY NAME (never silently ignored).
pub fn family_text_check(text: &str, n: usize) -> Result<(), Unknown> {
    let c: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let calls = top_level_calls(&c)?;
    let count = |h: &str| calls.iter().filter(|(head, _)| *head == h).count();
    let only = |h: &str, arg: &str, what: &str| -> Result<(), Unknown> {
        if count(h) != 1 || !calls.iter().any(|(head, a)| *head == h && *a == arg) {
            return Err(what.to_string());
        }
        Ok(())
    };
    only(
        "externals",
        "q",
        "only massless one-scale q^2=1 families supported (exactly one externals(q))",
    )?;
    only(
        "gram",
        "q,q,1",
        "only massless one-scale q^2=1 families supported (exactly one gram(q,q,1))",
    )?;
    only(
        "dimension",
        "d",
        "dimension must be d (exactly one dimension(d))",
    )?;
    if count("loops") != 1 {
        return Err("exactly one loops(..) is required".into());
    }
    if count("name") > 1 {
        return Err("at most one name(..) is allowed".into());
    }
    if count("prop") != n {
        return Err("unsupported propagator grammar/mass".into());
    }
    for (head, arg) in &calls {
        match *head {
            "name" | "loops" | "externals" | "dimension" | "gram" | "power_shift" => {}
            "prop" => {
                if !(arg.ends_with(",0") || arg.ends_with(",1")) {
                    return Err(format!("propagator flag not 0/1: prop({arg:.60})"));
                }
            }
            other => {
                return Err(format!(
                    "family element `{other}` not modelled by the replay: {other}({arg:.60})"
                ));
            }
        }
    }
    Ok(())
}

/// campaign copy, ADDED (audit C2; re-audit A NEW-M1): the table-header part of the engine's `family_check`
/// (replay/mod.rs:1979-2014 at b9f06de7): `family_source` equal up to whitespace, `family` name, `denominators` /
/// `loops` equal, `parameters == ["d"]`, no `source_conditions`. Returns the header fields that were actually
/// COMPARED (absent fields are not compared); the family is BOUND only when `family_source` is among them (the caller
/// decides what an unbound family may obtain: never the `replay` level).
pub fn family_header_check(
    header: &serde_json::Value,
    f: &Family,
) -> Result<Vec<&'static str>, Unknown> {
    let mut bound: Vec<&'static str> = Vec::new();
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    match header.get("family_source") {
        None | Some(serde_json::Value::Null) => {}
        Some(serde_json::Value::String(embedded)) => {
            if super::sha256::sha256_hex(squash(embedded).as_bytes()) != f.squashed_digest {
                return Err("family differs from table physical-slot source".into());
            }
            bound.push("family_source");
        }
        Some(_) => return Err("table header family_source is not a string".into()),
    }
    if let Some(x) = header.get("family") {
        if x.as_str().is_none() || x.as_str() != f.name.as_deref() {
            return Err(format!(
                "table family {x} differs from the family's name {:?}",
                f.name
            ));
        }
        bound.push("family");
    }
    if let Some(x) = header.get("denominators") {
        if x.as_u64() != Some(f.n as u64) {
            return Err("family dimension metadata mismatch".into());
        }
        bound.push("denominators");
    }
    if let Some(x) = header.get("loops") {
        if x.as_u64() != Some(f.loops as u64) {
            return Err("family dimension metadata mismatch".into());
        }
        bound.push("loops");
    }
    if let Some(x) = header.get("parameters") {
        if *x != serde_json::json!(["d"]) {
            return Err("unsupported extra coefficient parameters".into());
        }
        bound.push("parameters");
    }
    if header
        .get("source_conditions")
        .and_then(|x| x.as_array())
        .is_some_and(|a| !a.is_empty())
    {
        return Err(
            "nonempty source conditions require an explicit independent implementation".into(),
        );
    }
    Ok(bound)
}

/// campaign copy, CHANGED (re-audit A NEW-M2): a momentum name outside `declared` (the loop momenta and `q`) is an
/// ERROR (the engine copy dropped it silently, modelling another family than the text).
fn lin(mom: &str, declared: &[String]) -> Result<BTreeMap<String, i64>, Unknown> {
    // ([+-]?)(\d*)\*?([kq]\d*)
    let s: Vec<char> = mom.chars().filter(|c| !c.is_whitespace()).collect();
    let mut out: BTreeMap<String, i64> = BTreeMap::new();
    let mut i = 0;
    while i < s.len() {
        let mut sign = 1;
        if s[i] == '+' || s[i] == '-' {
            if s[i] == '-' {
                sign = -1;
            }
            i += 1;
        }
        let start = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let coef: i64 = if i > start {
            s[start..i]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| "momentum coefficient")?
        } else {
            1
        };
        if i < s.len() && s[i] == '*' {
            i += 1;
        }
        if i >= s.len() || !(s[i] == 'k' || s[i] == 'q') {
            return Err(format!("unsupported momentum {mom}"));
        }
        let vstart = i;
        i += 1;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        let v: String = s[vstart..i].iter().collect();
        if !declared.contains(&v) {
            return Err(format!("undeclared momentum `{v}` in ({mom})"));
        }
        *out.entry(v).or_insert(0) += sign * coef;
    }
    Ok(out)
}

/// Integer k with `value == k (4-d)/2` exactly (campaign copy, CHANGED (rows3): the affine form of `value` in d, no
/// n dependence, instead of three evaluations mod P61 at d = 0, 2, 4: the same answer for every polynomial value).
fn eps_multiple<A: CheckAlgebra>(alg: &A, value: &str, n: usize) -> Result<i64, Unknown> {
    let lin = alg
        .expr_linear(value, n)
        .map_err(|e| format!("power shift {value}: {e}"))?;
    let bad = || format!("power shift {value} is not an integer multiple of (4-d)/2");
    if lin[1..=n].iter().any(|q| !q.is_zero()) || lin[0].d != 1 {
        return Err(bad());
    }
    // value = c + b d = k (4 - d) / 2 = 2k - (k/2) d  <=>  c = 2k and b = -k/2
    let k = lin[0].n / 2;
    let half = alg.q_new(-k, 2)?;
    if lin[0].n % 2 != 0 || k == 0 || lin[n + 1] != half || !(-64..=64).contains(&k) {
        return Err(bad());
    }
    Ok(k as i64)
}

/// Text inside the first balanced `name(` ... `)` occurrence.
fn call_args(src: &str, name: &str) -> Option<String> {
    let start = src.find(&format!("{name}("))? + name.len() + 1;
    let mut depth = 1;
    for (k, ch) in src[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(src[start..start + k].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

impl Family {
    #[allow(
        clippy::needless_range_loop,
        reason = "index arithmetic over the contraction matrix"
    )]
    /// THE CERTIFICATE PARSE: the model of `src` built by the certificate backend (origin certificate).
    pub fn parse(src: &str) -> Result<Self, Unknown> {
        Self::parse_core(&CertAlg::new(None), src, CertAlg::ORIGIN)
    }

    /// [`Family::parse`] with its [`Stage::Family`] quantities recorded (the `Both` mode's certificate run).
    pub fn parse_traced(src: &str, rec: &Recorder) -> Result<Self, Unknown> {
        Self::parse_core(&CertAlg::new(Some(rec)), src, CertAlg::ORIGIN)
    }

    /// [`Family::parse`] with the algebra of `alg` (operator table, inverse of the scalar-product matrix): a model of
    /// DIAGNOSTIC origin, whatever `alg` is (a certificate run refuses it; rows4, re-audit RA3-M1).
    pub fn parse_with<A: CheckAlgebra>(alg: &A, src: &str) -> Result<Self, Unknown> {
        Self::parse_core(alg, src, AlgebraOrigin::diagnostic(alg.name()))
    }

    #[allow(
        clippy::needless_range_loop,
        reason = "index arithmetic over the contraction matrix"
    )]
    fn parse_core<A: CheckAlgebra>(
        alg: &A,
        src: &str,
        origin: AlgebraOrigin,
    ) -> Result<Self, Unknown> {
        let compact: String = src.chars().filter(|c| !c.is_whitespace()).collect();
        // campaign copy, CHANGED (re-audit A NEW-M2): the propagators, power shifts and loop momenta are read from the
        // TOP-LEVEL calls; labels must be the canonical D1..Dn in order (power shifts address them by label), every
        // momentum name must be declared, a malformed prop or power_shift is an error (the engine copy skipped it)
        let calls = top_level_calls(&compact)?;
        let loops: Vec<String> = call_args(&compact, "loops")
            .ok_or("family without loops(...)")?
            .split(',')
            .map(str::to_string)
            .collect();
        for (i, l) in loops.iter().enumerate() {
            let ok =
                l.len() > 1 && l.starts_with('k') && l[1..].bytes().all(|b| b.is_ascii_digit());
            if !ok || loops[..i].contains(l) {
                return Err(format!(
                    "loop momentum name `{l}` not modelled (k<digits>, distinct)"
                ));
            }
        }
        let mut props: Vec<(String, String)> = Vec::new();
        for (head, arg) in calls.iter().filter(|(h, _)| *h == "prop") {
            let parsed = (|| {
                let s = arg.strip_prefix('D')?;
                let digits = s.chars().take_while(char::is_ascii_digit).count();
                let label: usize = s[..digits].parse().ok()?;
                let s3 = s[digits..].strip_prefix(",(")?;
                let close = s3.find(')')?;
                let mom = &s3[..close];
                let flag = s3[close..].strip_prefix(")^2,")?;
                (flag.len() == 1 && flag.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| (label, mom.to_string(), flag.to_string()))
            })();
            match parsed {
                Some((label, mom, flag)) if label == props.len() + 1 => props.push((mom, flag)),
                Some((label, ..)) => {
                    return Err(format!(
                        "propagator label D{label} at position {}: labels must be D1..Dn in order",
                        props.len() + 1
                    ));
                }
                None => return Err(format!("malformed {head}({arg:.60})")),
            }
        }
        let n = props.len();
        let mut kshift = vec![0i64; n];
        let mut shifted = vec![false; n];
        for (_, arg) in calls.iter().filter(|(h, _)| *h == "power_shift") {
            let (label, value) = arg
                .split_once(',')
                .ok_or_else(|| format!("malformed power_shift({arg:.60})"))?;
            let j: usize = label
                .strip_prefix('D')
                .and_then(|x| x.parse().ok())
                .ok_or_else(|| format!("power_shift label `{label}`"))?;
            if j == 0 || j > n || shifted[j - 1] {
                return Err(format!(
                    "power_shift label D{j}: out of range D1..D{n} or repeated"
                ));
            }
            shifted[j - 1] = true;
            // p5l-close: two-sided (eps) families. A power shift must be an integer multiple k of
            // eps = (4-d)/2; the line's exponent is then a_i + k eps (checked at d = 0, 2, 4).
            kshift[j - 1] = if value == "0" || value.is_empty() {
                0
            } else {
                eps_multiple(alg, value, n)?
            };
        }
        let lcount = loops.len();
        let vs: Vec<String> = loops.iter().cloned().chain(["q".to_string()]).collect();
        let c: Vec<Vec<i64>> = props
            .iter()
            .map(|(m, _)| {
                lin(m, &vs).map(|l| vs.iter().map(|v| l.get(v).copied().unwrap_or(0)).collect())
            })
            .collect::<Result<_, _>>()?;
        let pairs: Vec<(usize, usize)> = (0..=lcount)
            .flat_map(|a| (a..=lcount).map(move |b| (a, b)))
            .filter(|&(a, b)| !(a == lcount && b == lcount))
            .collect();
        if pairs.len() != n {
            return Err(format!(
                "{} scalar products for {n} propagators: incomplete basis",
                pairs.len()
            ));
        }
        let idx = |a: usize, b: usize| pairs.iter().position(|&p| p == (a.min(b), a.max(b)));
        let mut m = vec![vec![Q::ZERO; n]; n];
        let mut c0 = vec![Q::ZERO; n];
        for i in 0..n {
            for a in 0..=lcount {
                for b in a..=lcount {
                    let co = c[i][a] * c[i][b] * if a == b { 1 } else { 2 };
                    if co == 0 {
                        continue;
                    }
                    if a == lcount && b == lcount {
                        c0[i] = alg.q_add(c0[i], Q::int(co as i128))?;
                    } else {
                        let k = idx(a, b).unwrap();
                        m[i][k] = alg.q_add(m[i][k], Q::int(co as i128))?;
                    }
                }
            }
        }
        let mi = alg
            .q_inverse(&m)
            .map_err(|e| format!("scalar-product matrix: {e}"))?;
        let sp = |a: usize, b: usize| {
            if a == lcount && b == lcount {
                None
            } else {
                idx(a, b)
            }
        };
        let mut ops = Vec::new();
        for j in 0..lcount {
            for v in 0..=lcount {
                let mut tab = Vec::with_capacity(n);
                for i in 0..n {
                    let mut t0 = Q::ZERO;
                    let mut tm = vec![Q::ZERO; n];
                    let cij = c[i][j];
                    if cij != 0 {
                        for a in 0..=lcount {
                            let cia = c[i][a];
                            if cia == 0 {
                                continue;
                            }
                            let f = Q::int(2 * cij as i128 * cia as i128);
                            match sp(a, v) {
                                None => t0 = alg.q_add(t0, f)?,
                                Some(s) => {
                                    for mm in 0..n {
                                        let x = mi[s][mm];
                                        if !x.is_zero() {
                                            let fx = alg.q_mul(f, x)?;
                                            tm[mm] = alg.q_add(tm[mm], fx)?;
                                            t0 = alg.q_sub(t0, alg.q_mul(fx, c0[mm])?)?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    tab.push((
                        t0,
                        tm.into_iter()
                            .enumerate()
                            .filter(|(_, x)| !x.is_zero())
                            .collect(),
                    ));
                }
                ops.push((j == v, tab));
            }
        }
        let table_digest = super::sha256::sha256_hex(format!("{mi:?}{ops:?}").as_bytes());
        alg.observe(Stage::Family, &|| "operator table".into(), &|| {
            table_digest.clone()
        });
        let eps_axes = (0..n).filter(|&i| kshift[i] != 0).collect();
        let check = family_text_check(src, n);
        let name = call_args(&compact, "name");
        Ok(Self {
            source_digest: super::sha256::sha256_hex(src.as_bytes()),
            squashed_digest: super::sha256::sha256_hex(compact.as_bytes()),
            name,
            check,
            loop_names: loops.clone(),
            source: src.to_string(),
            origin,
            table_digest,
            n,
            loops: lcount,
            kshift,
            eps_axes,
            c,
            ops,
            zero_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Momentum coefficients of every propagator: `momenta()[i][v]`, `v` over the loop momenta then `q`.
    pub fn momenta(&self) -> &[Vec<i64>] {
        &self.c
    }

    /// The L(L+1) IBP rows at base exponents `a` (integers), shift 0, dimension `d` (field element), mod p.
    /// Each row maps an offset (relative to `a`) to its coefficient. Certificate backend; see [`Family::rows_at_with`].
    pub fn rows_at(
        &self,
        a: &[i64],
        d: u64,
        p: u64,
    ) -> Result<Vec<BTreeMap<Vec<i64>, u64>>, Unknown> {
        self.rows_at_with(&Independent, a, d, p)
    }

    /// [`Family::rows_at`] with the field arithmetic of `alg`.
    pub fn rows_at_with<A: CheckAlgebra>(
        &self,
        alg: &A,
        a: &[i64],
        d: u64,
        p: u64,
    ) -> Result<Vec<BTreeMap<Vec<i64>, u64>>, Unknown> {
        let base: Vec<u64> = a.iter().map(|&x| alg.from_i128(p, x as i128)).collect();
        self.rows_at_mod_with(alg, &base, d, p)
    }

    /// campaign copy, ADDED: number of IBP operators (L(L+1)).
    pub fn n_ops(&self) -> usize {
        self.ops.len()
    }

    /// campaign copy, ADDED: [`Family::rows_at`] with the base exponents given as residues mod `p` (a generic point:
    /// symbolic slots carry random field values). `rows_at` reduces its integers mod p and calls this (rows3: one
    /// body instead of two identical copies).
    pub fn rows_at_mod(
        &self,
        a: &[u64],
        d: u64,
        p: u64,
    ) -> Result<Vec<BTreeMap<Vec<i64>, u64>>, Unknown> {
        self.rows_at_mod_with(&Independent, a, d, p)
    }

    /// [`Family::rows_at_mod`] with the field arithmetic of `alg`.
    pub fn rows_at_mod_with<A: CheckAlgebra>(
        &self,
        alg: &A,
        a: &[u64],
        d: u64,
        p: u64,
    ) -> Result<Vec<BTreeMap<Vec<i64>, u64>>, Unknown> {
        let n = self.n;
        if a.len() != n {
            return Err(format!("rows_at_mod: {} exponents for {n} slots", a.len()));
        }
        // eps = (4-d)/2 mod p; exponent of line i = a_i + kshift_i eps
        let eps = alg.mul(p, alg.sub(p, 4 % p, d % p), alg.inv(p, 2).ok_or("p = 2")?);
        let ai: Vec<u64> = (0..n)
            .map(|i| {
                alg.add(
                    p,
                    a[i] % p,
                    alg.mul(p, alg.from_i128(p, self.kshift[i] as i128), eps),
                )
            })
            .collect();
        let mut out = Vec::with_capacity(self.ops.len());
        for (diag, tab) in &self.ops {
            let mut r: BTreeMap<Vec<i64>, u64> = BTreeMap::new();
            if *diag {
                r.insert(vec![0; n], d % p);
            }
            for i in 0..n {
                let (t0, tm) = &tab[i];
                if t0.is_zero() && tm.is_empty() {
                    continue;
                }
                let x = ai[i];
                if x == 0 {
                    continue;
                }
                let mut up = vec![0i64; n];
                up[i] += 1;
                if !t0.is_zero() {
                    let v = alg.mul(p, x, alg.q_modp(*t0, p).ok_or("operator table pole")?);
                    let e = r.entry(up.clone()).or_insert(0);
                    *e = alg.sub(p, *e, v);
                }
                for (m, y) in tm {
                    up[*m] -= 1;
                    let v = alg.mul(p, x, alg.q_modp(*y, p).ok_or("operator table pole")?);
                    let e = r.entry(up.clone()).or_insert(0);
                    *e = alg.sub(p, *e, v);
                    up[*m] += 1;
                }
            }
            r.retain(|_, v| *v != 0);
            out.push(r);
        }
        Ok(out)
    }

    /// Exact operator row `op`: offset -> linear form `[const, y_0.., y_{n-1}, d]` (a_i = y_i).
    pub fn exact_row(&self, op: usize) -> Result<BTreeMap<Vec<i64>, Vec<Q>>, Unknown> {
        self.exact_row_with(&Independent, op)
    }

    /// [`Family::exact_row`] with the rational arithmetic of `alg`.
    pub fn exact_row_with<A: CheckAlgebra>(
        &self,
        alg: &A,
        op: usize,
    ) -> Result<BTreeMap<Vec<i64>, Vec<Q>>, Unknown> {
        let n = self.n;
        let (diag, tab) = &self.ops[op];
        let mut r: BTreeMap<Vec<i64>, Vec<Q>> = BTreeMap::new();
        let mut bump = |k: Vec<i64>, var: usize, s: Q| -> Result<(), Unknown> {
            let e = r.entry(k.clone()).or_insert_with(|| vec![Q::ZERO; n + 2]);
            e[var] = alg.q_add(e[var], s)?;
            if e.iter().all(Q::is_zero) {
                r.remove(&k);
            }
            Ok(())
        };
        if *diag {
            bump(vec![0; n], n + 1, Q::ONE)?;
        }
        for i in 0..n {
            let (t0, tm) = &tab[i];
            if t0.is_zero() && tm.is_empty() {
                continue;
            }
            let mut up = vec![0i64; n];
            up[i] = 1;
            // exponent a_i + k eps = y_i + 2k - (k/2) d
            let k = self.kshift[i];
            let half = alg.q_new(-(k as i128), 2)?;
            let twok = Q::int(2 * k as i128);
            let mut terms: Vec<(Vec<i64>, Q)> = Vec::new();
            if !t0.is_zero() {
                terms.push((up.clone(), *t0));
            }
            for (m, x) in tm {
                let mut u = up.clone();
                u[*m] -= 1;
                terms.push((u, *x));
            }
            for (u, c) in terms {
                let nc = alg.q_neg(c)?;
                bump(u.clone(), i + 1, nc)?;
                if k != 0 {
                    bump(u.clone(), 0, alg.q_mul(nc, twok)?)?;
                    bump(u, n + 1, alg.q_mul(nc, half)?)?;
                }
            }
        }
        Ok(r)
    }

    /// Scaleless sector (Lee's criterion; eps axes always on). Certificate backend; see [`Family::zero_mask_with`].
    pub fn zero_mask(&self, mask: &[bool]) -> bool {
        self.zero_mask_with(&Independent, mask)
    }

    /// [`Family::zero_mask`] with the algebra of `alg`. The answer is cached per family: use one backend per parsed
    /// family (the triage parses the family once per backend).
    pub fn zero_mask_with<A: CheckAlgebra>(&self, alg: &A, mask: &[bool]) -> bool {
        let mut m = mask.to_vec();
        for &i in &self.eps_axes {
            m[i] = true;
        }
        // campaign copy: Mutex (a poisoned lock is recovered: the cache holds only computed values)
        if let Some(z) = self
            .zero_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&m)
        {
            return *z;
        }
        let z = self.zero(alg, &m);
        self.zero_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(m, z);
        z
    }

    pub fn zero_key(&self, key: &[i64]) -> bool {
        self.zero_mask(&key.iter().map(|&x| x > 0).collect::<Vec<_>>())
    }

    /// campaign copy, CHANGED (rows3): the affine-hull test through `CheckAlgebra::q_unit_in_row_space`; an overflow
    /// (the copy panicked) answers "not scaleless" (keeping a column is the safe side: it can only make a span test
    /// fail, never pass).
    fn zero<A: CheckAlgebra>(&self, alg: &A, m: &[bool]) -> bool {
        let axes: Vec<usize> = (0..self.n).filter(|&i| m[i]).collect();
        let t = axes.len();
        if t == 0 {
            return true;
        }
        let (u, f) = self.graph_polys(alg, &axes);
        let mut vecs: Vec<Vec<u8>> = u.keys().chain(f.keys()).cloned().collect();
        vecs.sort();
        vecs.dedup();
        if vecs.is_empty() {
            return true;
        }
        let rows: Vec<Vec<Q>> = vecs
            .iter()
            .map(|k| {
                k.iter()
                    .map(|&e| Q::int(e as i128))
                    .chain([Q::ONE])
                    .collect()
            })
            .collect();
        !alg.q_unit_in_row_space(&rows, t).unwrap_or(true)
    }

    /// U and F (bordered determinant, q^2 = 1) of the lines `axes`, as exponent vectors -> coefficients.
    fn graph_polys<A: CheckAlgebra>(&self, alg: &A, axes: &[usize]) -> (ZPoly, ZPoly) {
        let l = self.loops;
        let t = axes.len();
        let mut mm: Vec<Vec<ZPoly>> = vec![vec![ZPoly::new(); l + 1]; l + 1];
        for (pos, &ax) in axes.iter().enumerate() {
            let c = &self.c[ax];
            let mut key = vec![0u8; t];
            key[pos] = 1;
            for a in 0..=l {
                for b in 0..=l {
                    let co = c[a] * c[b];
                    if co != 0 {
                        let e = mm[a][b].entry(key.clone()).or_insert(0);
                        *e += co as i128;
                        if *e == 0 {
                            mm[a][b].remove(&key);
                        }
                    }
                }
            }
        }
        (alg.zpoly_det(&mm, l, t), alg.zpoly_det(&mm, l + 1, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::independent_algebra::arith;

    const SUNSET: &str = "I(\n  name(S),\n  loops(k1,k2),\n  externals(q),\n  dimension(d),\n  prop(D1,(k1)^2,1),\n  prop(D2,(k2)^2,1),\n  prop(D3,(k1+k2-q)^2,1),\n  prop(D4,(k1+q)^2,0),\n  prop(D5,(k2+q)^2,0),\n  gram(q,q,1)\n)\n";

    /// p5l-close (M5 batch 5 tests): a power shift must be a nonzero integer multiple k of
    /// eps = (4-d)/2 (checked at d = 0, 2, 4); anything else is refused.
    #[test]
    fn eps_power_shifts_are_integer_multiples_of_eps() {
        assert_eq!(eps_multiple(&Independent, "(4-d)/2", 5).unwrap(), 1);
        assert_eq!(eps_multiple(&Independent, "2-d/2", 5).unwrap(), 1);
        assert_eq!(eps_multiple(&Independent, "4-d", 5).unwrap(), 2);
        assert_eq!(eps_multiple(&Independent, "(d-4)/2", 5).unwrap(), -1);
        assert_eq!(eps_multiple(&Independent, "3*(4-d)/2", 5).unwrap(), 3);
        for bad in ["d", "1", "(4-d)/3", "(4-d)/2+1", "0*d", "(6-d)/2"] {
            assert!(eps_multiple(&Independent, bad, 5).is_err(), "{bad}");
        }
    }

    /// An eps family (one line with exponent a_1 + k eps): the exact IBP forms (with the k eps
    /// terms on the constant and d offsets) agree with the modular rows at y = a, d = 7, for
    /// k = 1 and k = -2; the rows differ from the plain family's and from k = 2 (a doubled
    /// shift is not silently accepted as the single one).
    #[test]
    fn eps_family_rows_agree_with_their_exact_forms() {
        let shifted = |shift: &str| {
            SUNSET.replace(
                "  gram(q,q,1)\n",
                &format!("  power_shift(D1,{shift}),\n  gram(q,q,1)\n"),
            )
        };
        let plain = Family::parse(SUNSET).unwrap();
        let a = [1i128, 1, 1, 0, 0];
        let d = 7u64;
        let mut seen = Vec::new();
        for (shift, k) in [("(4-d)/2", 1), ("d-4", -2), ("4-d", 2)] {
            let f = Family::parse(&shifted(shift)).unwrap();
            assert_eq!(f.kshift(), vec![k, 0, 0, 0, 0], "{shift}");
            let rows = f.rows_at(&[1, 1, 1, 0, 0], d, arith::P).unwrap();
            assert_eq!(rows.len(), 6);
            for (op, md) in rows.iter().enumerate() {
                let ex = f.exact_row(op).unwrap();
                for (key, form) in &ex {
                    let mut v = Independent.q_modp(form[0], arith::P).unwrap();
                    for i in 0..5 {
                        let term = arith::mul(
                            arith::P,
                            Independent.q_modp(form[i + 1], arith::P).unwrap(),
                            arith::from_i128(arith::P, a[i]),
                        );
                        v = arith::add(arith::P, v, term);
                    }
                    let dterm =
                        arith::mul(arith::P, Independent.q_modp(form[6], arith::P).unwrap(), d);
                    v = arith::add(arith::P, v, dterm);
                    assert_eq!(
                        md.get(key).copied().unwrap_or(0),
                        v,
                        "{shift} op {op} {key:?}"
                    );
                }
            }
            seen.push(rows);
        }
        let base = plain.rows_at(&[1, 1, 1, 0, 0], d, arith::P).unwrap();
        assert_ne!(seen[0], base, "k = 1 must change the rows");
        assert_ne!(seen[0], seen[2], "k = 2 (doubled) must differ from k = 1");
    }

    #[allow(clippy::needless_range_loop)]
    #[test]
    fn sunset_family_ibp_and_zero_sectors() {
        let f = Family::parse(SUNSET).unwrap();
        assert_eq!((f.n(), f.loops()), (5, 2));
        // vacuum-like supports are scaleless; the sunset (D1 D2 D3) is not; D1 D3 leaves a k2 tadpole
        assert!(f.zero_mask(&[true, true, false, false, false]));
        assert!(!f.zero_mask(&[true, true, true, false, false]));
        assert!(f.zero_mask(&[true, false, true, false, false]));
        assert!(f.zero_mask(&[false, false, false, false, false]));
        // L(L+1) operators; the diagonal ones contain d at offset 0
        let rows = f.rows_at(&[1, 1, 1, 0, 0], 7, arith::P).unwrap();
        assert_eq!(rows.len(), 6);
        // exact rows agree with the modular rows at y = a, d = 7
        for op in 0..6 {
            let ex = f.exact_row(op).unwrap();
            let md = &rows[op];
            for (k, form) in &ex {
                let a = [1i128, 1, 1, 0, 0];
                let mut v = Independent.q_modp(form[0], arith::P).unwrap();
                for i in 0..5 {
                    v = arith::add(
                        arith::P,
                        v,
                        arith::mul(
                            arith::P,
                            Independent.q_modp(form[i + 1], arith::P).unwrap(),
                            arith::from_i128(arith::P, a[i]),
                        ),
                    );
                }
                v = arith::add(
                    arith::P,
                    v,
                    arith::mul(arith::P, Independent.q_modp(form[6], arith::P).unwrap(), 7),
                );
                assert_eq!(md.get(k).copied().unwrap_or(0), v, "op {op} key {k:?}");
            }
        }
    }
}
