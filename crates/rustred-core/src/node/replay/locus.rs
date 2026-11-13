//! Decisive locus-rank test of a rule table (`rr_node locus-rank`): engine port of guard-theory's
//! `SCRATCHPAD/guard-theory/tools/locus_rank.py` (theory: notes/guard-theory.md T1-T3, Conjecture C).
//!
//! For every selected rule:
//! * S = seed-coordinate suspects: the hyperplanes y_i(n) = n_a + off_i + recenter_i = 0 of every exported seed y
//!   (slot i symbolic in axis a), written canonically on the rule's case; dropped when the value n_a = v lies
//!   outside the chart sign of a (empty), when an exception branch vanishes identically on case ∩ {y_i = 0} or an
//!   RHS coefficient has a pole on all of it (the rule never serves those points), or when no integer point exists
//!   (random search, then an exhaustive small box; reported in `suspects_no_point`). Optional extra loci
//!   (`--extra-loci`, d-free affine strings) join S. Batch 6 (helper Y18 final list, CLI default): the LOW layers of
//!   the positive axes join S, every intersection of the live ISP-zero layers is tested, and `--sym-census` adds the
//!   symmetry layers. The list stays incomplete in principle (Y1/Y18): verdicts are Monte-Carlo evidence only.
//! * flats = the case itself (control) + every live suspect; with `--full` every intersection (one hyperplane per
//!   parallel class), capped by `--max-flats`. Otherwise phase B tests the intersections of the suspects whose flat
//!   FAILed or showed a rank drop against the control (T1: no rank drop on a flat = no new failure there).
//! * per flat ONE generic integer point (random large coordinates inside the chart, on the flat, off every other
//!   suspect, off exceptions and RHS poles) and the replay of [`super`] at that point with ALL original IBP rows at
//!   the exported seeds (3 d, mod P, +1 mutation control).
//!
//! Verdict per rule: FAIL (some flat fails: at that point the rule is NOT DERIVED FROM THESE SEEDS; not evidence that
//! it is false), PASS_ON_TESTED_FLATS (every flat
//! passes; `_CAPPED` when the flat cap was hit), UNKNOWN (no control point, a replay refused, or a malformed
//! rule). PASS_ON_TESTED_FLATS is a complete validity statement for the case only under Conjecture C (every codim-1 rank
//! drop of the full seed system that is not an RHS pole lies on S); a FAIL needs no conjecture.
//!
//! Points come from the deterministic MT19937 of the replay seeded per rule, so a rule's result does not depend on
//! which other rules are selected (the Python tool shares one generator; its points differ, its verdicts agree).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};

use super::arith::{self, P, PyRandom, Q};
use super::expr::{Expr, Unknown};
use super::family::Family;
use super::{
    Ctx, ReplayArgs, Rule, box_points, family_check, parse_integral, point_valid, replay_point,
    rule_from,
};

pub const VERSION: &str =
    "locus-rank.v1 (rustred node::replay::locus; port of guard-theory locus_rank.py)";

#[derive(Clone, Debug)]
pub struct LocusArgs {
    pub family: Option<PathBuf>,
    pub table: PathBuf,
    pub out: PathBuf,
    /// `None` = all rules; entries `(Some(chart), index)` or `(None, index)` (any chart).
    pub rules: Option<BTreeSet<(Option<String>, usize)>>,
    pub chart: Option<String>,
    pub dimensions: Vec<String>,
    pub max_flats: usize,
    pub full: bool,
    pub extra_loci: Option<PathBuf>,
    pub keep_cells: bool,
    pub no_replay: bool,
    /// Random integer-point attempts per flat (the Python tool: 600; default here 4000: an attempt that is not
    /// integral or not in the chart costs only rational arithmetic).
    pub point_tries: usize,
    /// Batch 6 (helper Y18 final suspect list): LOW layers of the positive axes, n_a = 1 .. 1-m (m = lowest seed
    /// offset + recenter on a): own (y = 0), zero-sector seeds (y <= -1), lowered columns (y = 1); as the Python
    /// tool's `--low-layers`. CLI default on.
    pub low_layers: bool,
    /// Batch 6 (Y14/Y18): every intersection (>= 2 layers, one per parallel class) of the live "seed ISP exponent
    /// = 0" layers is tested as its own flat (at most 2^#ISP per offset class), capped by `max_flats`. CLI default on.
    pub isp_zero_intersections: bool,
    /// Batch 6 (helper X1 C1): symmetry layers from a census of the family's slot actions (`census_all.jsonl` of
    /// the symmetry investigation: `{"node": <table header family>, "elements": [{"action": [..]}]}`).
    pub sym_census: Option<PathBuf>,
}

impl LocusArgs {
    /// `all`, or a comma list of `I`, `A-Z` and `CHART:I`.
    pub fn parse_rules(spec: &str) -> Result<Option<BTreeSet<(Option<String>, usize)>>, String> {
        if spec == "all" {
            return Ok(None);
        }
        let mut out = BTreeSet::new();
        for bit in spec.split(',').filter(|b| !b.is_empty()) {
            if let Some((c, i)) = bit.split_once(':') {
                out.insert((
                    Some(c.to_string()),
                    i.parse().map_err(|_| format!("rule {bit}"))?,
                ));
            } else if let Some((a, z)) = bit.split_once('-') {
                let (a, z): (usize, usize) = (
                    a.parse().map_err(|_| "rule range")?,
                    z.parse().map_err(|_| "rule range")?,
                );
                if a > z {
                    return Err("invalid index range".into());
                }
                out.extend((a..=z).map(|i| (None, i)));
            } else {
                out.insert((None, bit.parse().map_err(|_| format!("rule index {bit}"))?));
            }
        }
        Ok(Some(out))
    }
}

/// Affine index form `[c0, c_0, .., c_{N-1}]`, value `c0 + sum c_a n_a`.
type Form = Vec<Q>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Canon {
    All,
    Empty,
    Form(Vec<i128>),
}

/// Canonical form string of the Python tool (`gt.form_str`): `-2+n2`, `-75+40*n3+42*n5-7*n10-7*n11`.
fn form_str(t: &[i128]) -> String {
    let mut parts = Vec::new();
    if t[0] != 0 {
        parts.push(t[0].to_string());
    }
    for (a, &c) in t[1..].iter().enumerate() {
        if c != 0 {
            let head = match c {
                1 => String::new(),
                -1 => "-".into(),
                c => format!("{c}*"),
            };
            parts.push(format!("{head}n{a}"));
        }
    }
    parts.join("+").replace("+-", "-")
}

/// Exact RREF of affine forms over Q: (rows with unit pivot coefficient, pivot axes); None = inconsistent.
fn rref(forms: &[Form], n: usize) -> Result<Option<(Vec<Form>, Vec<usize>)>, Unknown> {
    let mut rows: Vec<Form> = forms.to_vec();
    let mut piv = Vec::new();
    let mut rank = 0;
    for col in 0..n {
        let Some(k) = (rank..rows.len()).find(|&i| !rows[i][col + 1].is_zero()) else {
            continue;
        };
        rows.swap(rank, k);
        let v = rows[rank][col + 1];
        rows[rank] = rows[rank]
            .iter()
            .map(|x| x.div(v))
            .collect::<Result<_, _>>()?;
        for i in 0..rows.len() {
            if i != rank && !rows[i][col + 1].is_zero() {
                let f = rows[i][col + 1];
                let pr = rows[rank].clone();
                rows[i] = rows[i]
                    .iter()
                    .zip(&pr)
                    .map(|(a, b)| a.sub(f.mul(*b)?))
                    .collect::<Result<_, _>>()?;
            }
        }
        piv.push(col);
        rank += 1;
    }
    if rows[rank..].iter().any(|r| !r[0].is_zero()) {
        return Ok(None);
    }
    rows.truncate(rank);
    Ok(Some((rows, piv)))
}

/// The rule's case: affine part in RREF (fixed coordinates + affine equations); nonlinear equations are only
/// checked on points (by [`point_valid`]).
struct Case {
    n: usize,
    rows: Vec<Form>,
    piv: Vec<usize>,
    /// The rule with only its affine equations (for the small-box fallback).
    affine_rule: Rule,
}

fn unit_form(n: usize, axis: usize, c0: i128) -> Form {
    let mut f = vec![Q::ZERO; n + 1];
    f[0] = Q::int(c0);
    f[axis + 1] = Q::ONE;
    f
}

impl Case {
    fn new(rule: &Rule) -> Result<Self, Unknown> {
        let n = rule.fixed.len();
        let mut forms = Vec::new();
        for (a, v) in rule.fixed.iter().enumerate() {
            if let Some(v) = v {
                forms.push(unit_form(n, a, -(*v as i128)));
            }
        }
        let mut affine_rule = rule.clone();
        affine_rule.equations.clear();
        for text in &rule.equations {
            let e = Expr::parse(text, n)?;
            if e.has_d() {
                return Err("case equation depends on d".into());
            }
            if e.degree()? <= 1 {
                let l = e.linear()?;
                forms.push(l[..=n].to_vec());
                affine_rule.equations.push(text.clone());
            }
        }
        let (rows, piv) = rref(&forms, n)?.ok_or("inconsistent case")?;
        Ok(Case {
            n,
            rows,
            piv,
            affine_rule,
        })
    }

    fn reduce(&self, form: &Form) -> Result<Form, Unknown> {
        let mut f = form.clone();
        for (row, &c) in self.rows.iter().zip(&self.piv) {
            let k = f[c + 1];
            if !k.is_zero() {
                f = f
                    .iter()
                    .zip(row)
                    .map(|(x, y)| x.sub(k.mul(*y)?))
                    .collect::<Result<_, _>>()?;
            }
        }
        Ok(f)
    }

    /// Canonical primitive integer form on the case (leading index coefficient positive).
    fn canon(&self, form: &Form) -> Result<Canon, Unknown> {
        let f = self.reduce(form)?;
        if f[1..].iter().all(Q::is_zero) {
            return Ok(if f[0].is_zero() {
                Canon::All
            } else {
                Canon::Empty
            });
        }
        let g = |mut a: i128, mut b: i128| {
            a = a.abs();
            b = b.abs();
            while b != 0 {
                (a, b) = (b, a % b);
            }
            a
        };
        let mut den: i128 = 1;
        for x in &f {
            den = den / g(den, x.d) * x.d;
        }
        let ints: Vec<i128> = f.iter().map(|x| x.n * (den / x.d)).collect();
        let gg = ints.iter().fold(0, |a, &b| g(a, b)).max(1);
        let mut ints: Vec<i128> = ints.into_iter().map(|x| x / gg).collect();
        let lead = *ints[1..].iter().find(|&&x| x != 0).unwrap();
        if lead < 0 {
            ints.iter_mut().for_each(|x| *x = -*x);
        }
        Ok(Canon::Form(ints))
    }

    /// Random point mod P of case ∩ {extra = 0} (index residues) or None when that is empty over Q.
    fn point_mod(&self, rng: &mut PyRandom, extra: &[Form]) -> Result<Option<Vec<u64>>, Unknown> {
        let mut forms = self.rows.clone();
        forms.extend(extra.iter().cloned());
        let Some((rows, piv)) = rref(&forms, self.n)? else {
            return Ok(None);
        };
        let mut v = vec![0u64; self.n];
        let free: Vec<usize> = (0..self.n).filter(|a| !piv.contains(a)).collect();
        for &a in &free {
            v[a] = rand_mod(rng);
        }
        for (row, &c) in rows.iter().zip(&piv) {
            let mut x = row[0].modp(P).ok_or("form pole")?;
            for &j in &free {
                x = arith::add(
                    P,
                    x,
                    arith::mul(P, row[j + 1].modp(P).ok_or("form pole")?, v[j]),
                );
            }
            v[c] = arith::neg(P, x);
        }
        Ok(Some(v))
    }
}

fn rand_mod(rng: &mut PyRandom) -> u64 {
    ((rng.genrand_u32() as u64) << 29 ^ rng.genrand_u32() as u64) % P
}

fn form_of(c: &[i128]) -> Form {
    c.iter().map(|&x| Q::int(x)).collect()
}

fn sign_ok(g: &str, v: i64) -> bool {
    match g {
        "n>=1" => v >= 1,
        "n<=0" => v <= 0,
        _ => true,
    }
}

fn guards_of(rule: &Rule, chart: Option<&Value>) -> Vec<String> {
    let from_chart: Option<Vec<String>> = chart.and_then(|c| c["guards"].as_array()).map(|g| {
        g.iter()
            .map(|x| x.as_str().unwrap_or_default().to_string())
            .collect()
    });
    let g = from_chart.unwrap_or_else(|| {
        rule.chart
            .chars()
            .map(|b| {
                if b == '1' {
                    "n>=1".into()
                } else {
                    "n<=0".into()
                }
            })
            .collect()
    });
    g.into_iter()
        .map(|x| {
            if x.starts_with("two_sided") {
                "any".into()
            } else {
                x
            }
        })
        .collect()
}

/// Per rule the precomputed context of the flats.
struct RuleCtx {
    /// Batch 6: the live suspects that are "seed ISP exponent = 0" layers.
    isp_zero: Vec<Vec<i128>>,
    case: Case,
    guards: Vec<String>,
    coefs: Vec<Expr>,
    exceptions: Vec<Vec<Expr>>,
    live: Vec<Vec<i128>>,
    smallpt: HashMap<Vec<i128>, Vec<i64>>,
    binding: Value,
}

struct Item {
    flat: Vec<Vec<i128>>,
    point: Option<Vec<i64>>,
    why: Option<String>,
}

/// Exception branch vanishing identically on case ∩ {forms = 0}, or an RHS coefficient with a pole on all of it
/// (three random points mod P with random d). None when the flat is empty over Q.
fn never_served(rc: &RuleCtx, forms: &[Form], rng: &mut PyRandom) -> Result<Option<bool>, Unknown> {
    let mut pts = Vec::new();
    for _ in 0..3 {
        let Some(p) = rc.case.point_mod(rng, forms)? else {
            return Ok(None);
        };
        pts.push((p, rand_mod(rng)));
    }
    for br in &rc.exceptions {
        if !br.is_empty()
            && pts
                .iter()
                .all(|(p, d)| br.iter().all(|e| e.at_mod(P, p, *d).is_ok_and(|v| v == 0)))
        {
            return Ok(Some(true));
        }
    }
    for c in &rc.coefs {
        if pts.iter().all(|(p, d)| c.at_mod(P, p, *d).is_err()) {
            return Ok(Some(true));
        }
    }
    Ok(Some(false))
}

/// Generic integer point of case ∩ {forms = 0} inside the chart, off `avoid`, valid for the replay
/// ([`point_valid`]: exceptions, coefficient poles, nonlinear equations). Err("EMPTY_RATIONAL" | "NO_POINT").
#[allow(clippy::too_many_arguments)]
fn int_point(
    rule: &Rule,
    rc: &RuleCtx,
    forms: &[Form],
    avoid: &[Form],
    dims: &[Q],
    chart: Option<&Value>,
    rng: &mut PyRandom,
    tries: usize,
) -> Result<Result<Vec<i64>, &'static str>, Unknown> {
    let n = rc.case.n;
    let mut all = rc.case.rows.clone();
    all.extend(forms.iter().cloned());
    let Some((rows, piv)) = rref(&all, n)? else {
        return Ok(Err("EMPTY_RATIONAL"));
    };
    let free: Vec<usize> = (0..n).filter(|a| !piv.contains(a)).collect();
    'tries: for t in 0..tries {
        let r = 12 + 6 * (t / 50) as i64;
        let mut vals = vec![Q::ZERO; n];
        for &a in &free {
            let x = match rc.guards[a].as_str() {
                "n>=1" => rng.randint(3, 3 + r),
                "n<=0" => -rng.randint(2, 2 + r),
                _ => {
                    let x = rng.randint(3, 3 + r);
                    if rng.randint(0, 1) == 0 { -x } else { x }
                }
            };
            vals[a] = Q::int(x as i128);
        }
        for (row, &c) in rows.iter().zip(&piv) {
            let mut v = row[0];
            for &j in &free {
                v = v.add(row[j + 1].mul(vals[j])?)?;
            }
            if v.d != 1 {
                continue 'tries;
            }
            vals[c] = v.neg();
        }
        let p: Vec<i64> = vals.iter().map(|x| x.n as i64).collect();
        if (0..n).any(|a| !sign_ok(&rc.guards[a], p[a])) {
            continue;
        }
        for af in avoid {
            let mut s = af[0];
            for a in 0..n {
                if !af[a + 1].is_zero() {
                    s = s.add(af[a + 1].mul(Q::int(p[a] as i128))?)?;
                }
            }
            if s.is_zero() {
                continue 'tries;
            }
        }
        if point_valid(rule, &p, dims, chart, &rc.coefs).is_ok() {
            return Ok(Ok(p));
        }
    }
    Ok(Err("NO_POINT"))
}

/// Every flat of `live` (one hyperplane per parallel class), the empty flat first, in itertools order, capped.
fn all_flats(live: &[Vec<i128>], cap: usize) -> Vec<Vec<Vec<i128>>> {
    fn combos(k: usize, n: usize) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        let mut idx: Vec<usize> = (0..k).collect();
        if k > n {
            return out;
        }
        loop {
            out.push(idx.clone());
            let Some(i) = (0..k).rev().find(|&i| idx[i] != i + n - k) else {
                return out;
            };
            idx[i] += 1;
            for j in i + 1..k {
                idx[j] = idx[j - 1] + 1;
            }
        }
    }
    let mut flats = vec![Vec::new()];
    for k in 1..=live.len() {
        for comb in combos(k, live.len()) {
            let lin: BTreeSet<&[i128]> = comb.iter().map(|&i| &live[i][1..]).collect();
            if lin.len() < k {
                continue;
            }
            flats.push(comb.iter().map(|&i| live[i].clone()).collect());
            if flats.len() >= cap {
                return flats;
            }
        }
    }
    flats
}

fn make_items(
    rule: &Rule,
    rc: &RuleCtx,
    flats: &[Vec<Vec<i128>>],
    dims: &[Q],
    chart: Option<&Value>,
    rng: &mut PyRandom,
    tries: usize,
) -> Result<Vec<Item>, Unknown> {
    let mut items = Vec::new();
    for fl in flats {
        let forms: Vec<Form> = fl.iter().map(|c| form_of(c)).collect();
        let avoid: Vec<Form> = rc
            .live
            .iter()
            .filter(|c| !fl.contains(c))
            .map(|c| form_of(c))
            .collect();
        let mut why: Option<String> = None;
        let mut p = int_point(rule, rc, &forms, &avoid, dims, chart, rng, tries)?;
        if p == Err("NO_POINT") {
            p = int_point(rule, rc, &forms, &[], dims, chart, rng, tries)?;
            why = Some(if p.is_ok() {
                "NONGENERIC_POINT".into()
            } else {
                "NO_POINT".into()
            });
        }
        if p.is_err() && fl.len() == 1 {
            if let Some(sp) = rc.smallpt.get(&fl[0]) {
                p = Ok(sp.clone());
                why = Some("SMALL_POINT".into());
            }
        }
        match p {
            Err("EMPTY_RATIONAL") => continue,
            Err(_) if fl.len() > 1 => continue,
            Err(w) => items.push(Item {
                flat: fl.clone(),
                point: None,
                why: Some(w.into()),
            }),
            Ok(pt) => items.push(Item {
                flat: fl.clone(),
                point: Some(pt),
                why,
            }),
        }
    }
    Ok(items)
}

/// Result of one flat replay.
#[derive(Clone)]
struct Res {
    status: String,
    rank: Vec<Option<u64>>,
    rows: Vec<Option<u64>>,
    reason: Option<String>,
}

impl Res {
    fn json(&self) -> Value {
        json!({"status": self.status, "rank": self.rank, "rows": self.rows, "reason": self.reason})
    }
    /// The Python tool's effective status: a replay that refuses the point as an exception/pole point is GUARDED.
    fn eff(&self) -> String {
        let r = self.reason.as_deref().unwrap_or_default();
        if self.status == "UNKNOWN"
            && (r.contains("exception conjunction") || r.contains("pole"))
            && !r.contains("outside chart")
        {
            "GUARDED".into()
        } else {
            self.status.clone()
        }
    }
    fn min_rank(&self) -> Option<u64> {
        if self.rank.is_empty() || self.rank.iter().any(Option::is_none) {
            return None;
        }
        self.rank.iter().flatten().min().copied()
    }
}

fn run_item(
    rule: &Rule,
    rc: &RuleCtx,
    ctx: &Ctx,
    dir: &std::path::Path,
    point: &[i64],
    keep: bool,
) -> Res {
    let out = (|| -> Result<Value, Unknown> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        replay_point(
            rule,
            &rc.binding,
            ctx,
            dir,
            point,
            "locus-rank generic flat point".into(),
        )
    })();
    if !keep {
        let _ = std::fs::remove_dir_all(dir);
    }
    match out {
        Ok(v) => {
            let cells = v["cells"].as_array().cloned().unwrap_or_default();
            Res {
                status: v["status"].as_str().unwrap_or("UNKNOWN").to_string(),
                rank: cells.iter().map(|c| c["rank"].as_u64()).collect(),
                rows: cells.iter().map(|c| c["source_rows"].as_u64()).collect(),
                reason: v["reason"].as_str().map(str::to_string),
            }
        }
        Err(e) => Res {
            status: "UNKNOWN".into(),
            rank: Vec::new(),
            rows: Vec::new(),
            reason: Some(e),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    rule: &Rule,
    chart: Option<&Value>,
    dims: &[Q],
    extra: Option<&Vec<String>>,
    extra_rc: Option<&Value>,
    rng: &mut PyRandom,
    tries: usize,
    rec: &mut Value,
    low_layers: bool,
    sym: &[Vec<Option<usize>>],
) -> Result<RuleCtx, Unknown> {
    let n = rule.fixed.len();
    let case = Case::new(rule)?;
    let guards = guards_of(rule, chart);
    if guards.len() != n {
        return Err("chart guard length".into());
    }
    let coefs: Vec<Expr> = rule
        .rhs
        .iter()
        .map(|(_, c)| Expr::parse(c, n))
        .collect::<Result<_, _>>()?;
    let exceptions: Vec<Vec<Expr>> = rule
        .exceptions
        .iter()
        .map(|b| {
            b.iter()
                .map(|p| Expr::parse(p, n))
                .collect::<Result<_, _>>()
        })
        .collect::<Result<_, _>>()?;
    // recenter of the suspects = the replay's recenter authority (table recenter / raw pivot / --extra-loci)
    let has_raw = rule
        .raw
        .get("raw_pivot_encoded")
        .is_some_and(|v| !v.is_null());
    let mut binding = json!({});
    let table_rc = rule.raw.get("recenter").filter(|v| !v.is_null()).cloned();
    let rc_val = match (&table_rc, extra_rc) {
        (Some(v), _) => Some(v.clone()),
        (None, Some(v)) if !has_raw => Some(v.clone()),
        _ => None,
    };
    if !has_raw {
        if let Some(v) = &rc_val {
            binding = json!({"recenter": v, "recenter_authority": "table/extra-loci recenter (locus-rank)"});
        }
    }
    rec["binding_recenter"] = if has_raw {
        Value::Null
    } else {
        rc_val.clone().unwrap_or(Value::Null)
    };
    let recenter: Vec<i64> = match &rc_val {
        Some(v) => super::vector(v, n)?,
        None if has_raw => super::recenter_for(rule, &json!({}), n)?.0,
        None => vec![0; n],
    };
    // S_C: canon -> [(seed, slot, axis, value)]
    let mut suspects: BTreeMap<Vec<i128>, Vec<(usize, usize, usize, i64)>> = BTreeMap::new();
    for (k, s) in rule.seeds.iter().enumerate() {
        for (i, (ax, off)) in parse_integral(s, n)?.into_iter().enumerate() {
            let Some(ax) = ax else { continue };
            let c0 = off + recenter[i];
            if let Canon::Form(c) = case.canon(&unit_form(n, ax, c0 as i128))? {
                suspects.entry(c).or_default().push((k, i, ax, -c0));
            }
        }
    }
    // seed ISP exponent = 0 layers (S_C entries on an ISP axis): their intersections are flats of their own
    let isp_zero: BTreeSet<Vec<i128>> = suspects
        .iter()
        .filter(|(_, reps)| reps.iter().any(|&(_, _, ax, _)| guards[ax] == "n<=0"))
        .map(|(c, _)| c.clone())
        .collect();
    if low_layers {
        let mut low = Vec::new();
        for (k, s) in rule.seeds.iter().enumerate() {
            for (i, (ax, off)) in parse_integral(s, n)?.into_iter().enumerate() {
                let Some(ax) = ax else { continue };
                if guards[ax] != "n>=1" {
                    continue;
                }
                for v in 1..(2 - (off + recenter[i])) {
                    if let Canon::Form(c) = case.canon(&unit_form(n, ax, -(v as i128)))? {
                        if let std::collections::btree_map::Entry::Vacant(slot) = suspects.entry(c) {
                            low.push(form_str(slot.key()));
                            slot.insert(vec![(k, i, ax, v)]);
                        }
                    }
                }
            }
        }
        rec["low_layers"] = json!(low);
    }
    let mut cands: BTreeSet<Vec<i128>> = suspects.keys().cloned().collect();
    if !sym.is_empty() {
        // symmetry layers (port of locus_rank.py --sym-census): element g (slot action a, None = ISP sent to a
        // combination) maps seed k onto seed k' on {y^k'_{a(i)} = y^k_i (defined slots), y^k_i = 0 (None slots),
        // y^k'_j = 0 (slots not hit)}; kept when that set is one hyperplane of the case.
        let yforms: Vec<Vec<Form>> = rule
            .seeds
            .iter()
            .map(|s| {
                Ok(parse_integral(s, n)?
                    .into_iter()
                    .enumerate()
                    .map(|(i, (ax, off))| {
                        let mut f = vec![Q::ZERO; n + 1];
                        match ax {
                            None => f[0] = Q::int(off as i128),
                            Some(ax) => {
                                f[0] = Q::int((off + recenter[i]) as i128);
                                f[ax + 1] = Q::ONE;
                            }
                        }
                        f
                    })
                    .collect())
            })
            .collect::<Result<_, Unknown>>()?;
        let mut sym_new: Vec<Vec<i128>> = Vec::new();
        for a in sym {
            if a.len() != n {
                continue;
            }
            let hit: BTreeSet<usize> = a.iter().flatten().copied().collect();
            for y1 in &yforms {
                for y2 in &yforms {
                    let mut forms: BTreeSet<Vec<i128>> = BTreeSet::new();
                    let mut bad = false;
                    for i in 0..n {
                        let f: Form = match a[i] {
                            Some(j) => y2[j]
                                .iter()
                                .zip(&y1[i])
                                .map(|(u, v)| u.sub(*v))
                                .collect::<Result<_, _>>()?,
                            None => y1[i].clone(),
                        };
                        match case.canon(&f)? {
                            Canon::Empty => bad = true,
                            Canon::All => {}
                            Canon::Form(c) => {
                                forms.insert(c);
                            }
                        }
                        if bad || forms.len() > 1 {
                            bad = true;
                            break;
                        }
                    }
                    if !bad {
                        for j in (0..n).filter(|j| !hit.contains(j)) {
                            match case.canon(&y2[j])? {
                                Canon::Empty => {
                                    bad = true;
                                    break;
                                }
                                Canon::All => {}
                                Canon::Form(c) => {
                                    forms.insert(c);
                                }
                            }
                        }
                    }
                    if bad || forms.len() != 1 {
                        continue;
                    }
                    let c = forms.into_iter().next().unwrap();
                    if !cands.contains(&c) && !sym_new.contains(&c) {
                        sym_new.push(c);
                    }
                }
            }
        }
        rec["sym_layers"] = json!(sym_new.iter().map(|c| form_str(c)).collect::<Vec<_>>());
        cands.extend(sym_new);
    }
    for text in extra.into_iter().flatten() {
        let e = Expr::parse(text, n)?;
        if e.has_d() || e.degree()? > 1 {
            continue;
        }
        if let Canon::Form(c) = case.canon(&e.linear()?[..=n].to_vec())? {
            cands.insert(c);
        }
    }
    let mut rcx = RuleCtx {
        isp_zero: Vec::new(),
        case,
        guards,
        coefs,
        exceptions,
        live: Vec::new(),
        smallpt: HashMap::new(),
        binding,
    };
    let mut ordered: Vec<Vec<i128>> = cands.into_iter().collect();
    ordered.sort_by_key(|c| format!("{c:?}"));
    let mut dropped = Vec::new();
    for c in ordered {
        let reps = suspects.get(&c).cloned().unwrap_or_default();
        if reps
            .iter()
            .any(|&(_, _, ax, v)| !sign_ok(&rcx.guards[ax], v))
        {
            continue; // n_a = v outside the chart sign: empty (exact)
        }
        let fl = vec![form_of(&c)];
        if never_served(&rcx, &fl, rng)? == Some(true) {
            continue;
        }
        match int_point(rule, &rcx, &fl, &[], dims, chart, rng, tries)? {
            Ok(_) => rcx.live.push(c),
            Err(_) => {
                // exhaustive small box (n = 1..5 / 0..-4 on the free coordinates) before giving up
                let mut ext = rcx.case.affine_rule.clone();
                ext.equations.push(form_str(&c));
                // bounded enumeration: size^free <= ~20k points, n up to 64 on a low-dimensional flat (cases whose
                // equations have a large modulus need it: r38's rule 1 has integral points only every 62 steps)
                let free = n.saturating_sub(rcx.case.piv.len() + 1).max(1) as f64;
                let size = (20_000f64.powf(1.0 / free).floor() as usize).clamp(2, 64);
                match box_points(&ext, dims, chart, &rcx.coefs, size, 1) {
                    Ok((pts, _))
                        if !pts.is_empty()
                            && point_valid(rule, &pts[0], dims, chart, &rcx.coefs).is_ok() =>
                    {
                        rcx.smallpt.insert(c.clone(), pts[0].clone());
                        rcx.live.push(c);
                    }
                    Ok(_) => {
                        dropped.push(json!([form_str(&c), format!("NO_POINT_SMALLBOX{size}")]))
                    }
                    Err(e) => dropped.push(json!([form_str(&c), format!("NO_POINT ({e})")])),
                }
            }
        }
    }
    rcx.live.sort_by_key(|c| form_str(c));
    rcx.isp_zero = rcx
        .live
        .iter()
        .filter(|c| isp_zero.contains(*c))
        .cloned()
        .collect();
    rec["suspects"] = json!(rcx.live.iter().map(|c| form_str(c)).collect::<Vec<_>>());
    rec["isp_zero_layers"] = json!(rcx.isp_zero.iter().map(|c| form_str(c)).collect::<Vec<_>>());
    rec["suspects_no_point"] = Value::Array(dropped);
    Ok(rcx)
}

/// Run the locus-rank test; writes `OUT/RESULTS.jsonl` (one line per rule) and `OUT/SUMMARY.json`.
/// Returns the overall status: FAIL (some rule fails), PASS_ON_TESTED_FLATS (every rule), UNKNOWN otherwise.
pub fn run(args: &LocusArgs) -> Result<(String, Value), String> {
    let started = Instant::now();
    std::fs::create_dir_all(&args.out).map_err(|e| e.to_string())?;
    let results_path = args.out.join("RESULTS.jsonl");
    if results_path.exists() {
        return Err("RESULTS.jsonl already exists in the output directory".into());
    }
    let text = std::fs::read_to_string(&args.table).map_err(|e| format!("table: {e}"))?;
    let mut header: Option<Value> = None;
    let mut charts: HashMap<String, Value> = HashMap::new();
    let mut rules: Vec<Rule> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let r: Value = serde_json::from_str(line).map_err(|e| format!("table json: {e}"))?;
        match r["type"].as_str() {
            Some("header") => header = Some(r),
            Some("chart") => {
                charts.insert(r["chart"].as_str().unwrap_or_default().to_string(), r);
            }
            Some("rule") => {
                let idx = r["index"].as_u64().unwrap_or(u64::MAX) as usize;
                let ch = r["chart"].as_str().unwrap_or_default().to_string();
                let chart_ok = args.chart.as_ref().is_none_or(|c| *c == ch);
                let sel = args.rules.as_ref().is_none_or(|w| {
                    w.contains(&(None, idx)) || w.contains(&(Some(ch.clone()), idx))
                });
                if chart_ok && sel {
                    rules.push(rule_from(&r)?);
                }
            }
            _ => {}
        }
    }
    let header = header.ok_or("table without header")?;
    if rules.is_empty() {
        return Err("no selected rules".into());
    }
    let family_text = match &args.family {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("family: {e}"))?,
        None => header["family_source"]
            .as_str()
            .ok_or("table header lacks full family_source")?
            .to_string(),
    };
    let f = Family::parse(&family_text)?;
    family_check(&header, &family_text, &f)?;
    let mut extra: HashMap<(String, usize), Vec<String>> = HashMap::new();
    let mut extra_rc: HashMap<(String, usize), Value> = HashMap::new();
    if let Some(path) = &args.extra_loci {
        let t = std::fs::read_to_string(path).map_err(|e| format!("extra loci: {e}"))?;
        for line in t.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value =
                serde_json::from_str(line).map_err(|e| format!("extra loci json: {e}"))?;
            let key = (
                v["chart"].as_str().unwrap_or_default().to_string(),
                v.get("rule")
                    .or_else(|| v.get("index"))
                    .and_then(Value::as_u64)
                    .ok_or("extra loci record without rule")? as usize,
            );
            extra.entry(key.clone()).or_default().extend(
                v["loci"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string)),
            );
            if let Some(r) = v.get("recenter").filter(|r| !r.is_null()) {
                extra_rc.insert(key, r.clone());
            }
        }
    }
    // batch 6: symmetry layers from a census of the family's slot actions
    let mut sym: Vec<Vec<Option<usize>>> = Vec::new();
    if let Some(path) = &args.sym_census {
        let family_name = header["family"].as_str().unwrap_or_default().to_string();
        let t = std::fs::read_to_string(path).map_err(|e| format!("sym census: {e}"))?;
        for line in t.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value = serde_json::from_str(line).map_err(|e| format!("sym census json: {e}"))?;
            if v["node"].as_str() != Some(family_name.as_str()) {
                continue;
            }
            for e in v["elements"].as_array().into_iter().flatten() {
                let action: Vec<Option<usize>> = e["action"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|x| x.as_u64().map(|x| x as usize))
                    .collect();
                if action.iter().enumerate().any(|(i, a)| *a != Some(i)) {
                    sym.push(action);
                }
            }
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
    let dq: Vec<Q> = dims.iter().map(|x| x.2).collect();
    let rargs = ReplayArgs {
        family: args.family.clone(),
        table: args.table.clone(),
        cert: None,
        cand_dump_dir: None,
        bindings: None,
        rules: None,
        chart: None,
        out: args.out.clone(),
        dimensions: args.dimensions.clone(),
        sample_attempts: 128,
        extra_points: 0,
        loci: None,
        box_size: 0,
        box_max: 0,
        dump_weights: false,
        random_d: false,
        single_prime: true,
    };
    let ctx = Ctx {
        args: &rargs,
        f: &f,
        cert: None,
        dims,
    };
    let mut lines = String::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for rule in &rules {
        let t0 = Instant::now();
        let chart = charts.get(&rule.chart);
        let key = (rule.chart.clone(), rule.index);
        let mut rng = PyRandom::new((1112 + rule.index) as u32);
        let mut rec = json!({"chart": rule.chart, "rule": rule.index,
            "nsources": rule.raw.get("nsources"), "nseeds": rule.seeds.len()});
        let cells = args
            .out
            .join("cells")
            .join(format!("{}-r{}", rule.chart, rule.index));
        let prepared = prepare(
            rule,
            chart,
            &dq,
            extra.get(&key),
            extra_rc.get(&key),
            &mut rng,
            args.point_tries,
            &mut rec,
            args.low_layers,
            &sym,
        );
        let verdict = match prepared {
            Err(e) => {
                rec["error"] = json!(e);
                rec["flat_results"] = json!([]);
                "UNKNOWN".to_string()
            }
            Ok(rc) => {
                let outcome = (|| -> Result<String, Unknown> {
                    let flats: Vec<Vec<Vec<i128>>> = if args.full {
                        all_flats(&rc.live, args.max_flats)
                    } else {
                        std::iter::once(Vec::new())
                            .chain(rc.live.iter().map(|c| vec![c.clone()]))
                            .collect()
                    };
                    let mut done: Vec<(Item, Option<Res>)> = Vec::new();
                    let items =
                        make_items(rule, &rc, &flats, &dq, chart, &mut rng, args.point_tries)?;
                    for (k, it) in items.into_iter().enumerate() {
                        let r = match (&it.point, args.no_replay) {
                            (Some(p), false) => Some(run_item(
                                rule,
                                &rc,
                                &ctx,
                                &cells.join(format!("a{k}")),
                                p,
                                args.keep_cells,
                            )),
                            _ => None,
                        };
                        done.push((it, r));
                    }
                    // phase B: intersections of the hot (FAIL or rank-dropping) codim-1 suspects
                    if !args.full && !args.no_replay {
                        let ctrl = done
                            .first()
                            .filter(|(it, _)| it.flat.is_empty())
                            .and_then(|(_, r)| r.clone());
                        let cr = ctrl.as_ref().and_then(Res::min_rank);
                        let hot: Vec<Vec<i128>> = done
                            .iter()
                            .filter(|(it, _)| it.flat.len() == 1)
                            .filter_map(|(it, r)| {
                                let r = r.as_ref()?;
                                let drop =
                                    matches!((cr, r.min_rank()), (Some(c), Some(x)) if x < c);
                                (r.status.starts_with("FAIL") || drop).then(|| it.flat[0].clone())
                            })
                            .collect();
                        rec["rank_drop_suspects"] =
                            json!(hot.iter().map(|c| form_str(c)).collect::<Vec<_>>());
                        if hot.len() >= 2 {
                            let fl2: Vec<Vec<Vec<i128>>> = all_flats(&hot, args.max_flats)
                                .into_iter()
                                .filter(|f| f.len() >= 2)
                                .collect();
                            rec["flats_capped"] = json!(fl2.len() + 1 >= args.max_flats);
                            let items = make_items(
                                rule,
                                &rc,
                                &fl2,
                                &dq,
                                chart,
                                &mut rng,
                                args.point_tries,
                            )?;
                            for (k, it) in items.into_iter().enumerate() {
                                let r = it.point.as_ref().map(|p| {
                                    run_item(
                                        rule,
                                        &rc,
                                        &ctx,
                                        &cells.join(format!("b{k}")),
                                        p,
                                        args.keep_cells,
                                    )
                                });
                                done.push((it, r));
                            }
                        }
                    }
                    // phase C (batch 6, Y14/Y18): every intersection of the live ISP-zero layers
                    if args.isp_zero_intersections && !args.full && rc.isp_zero.len() >= 2 {
                        let tested: BTreeSet<Vec<Vec<i128>>> =
                            done.iter().map(|(it, _)| it.flat.clone()).collect();
                        let fl3: Vec<Vec<Vec<i128>>> = all_flats(&rc.isp_zero, args.max_flats)
                            .into_iter()
                            .filter(|f| f.len() >= 2 && !tested.contains(f))
                            .collect();
                        let capped = fl3.len() + 1 >= args.max_flats;
                        rec["isp_zero_flats"] = json!(fl3.len());
                        if capped {
                            rec["flats_capped"] = json!(true);
                        }
                        let items =
                            make_items(rule, &rc, &fl3, &dq, chart, &mut rng, args.point_tries)?;
                        for (k, it) in items.into_iter().enumerate() {
                            let r = match (&it.point, args.no_replay) {
                                (Some(p), false) => Some(run_item(
                                    rule,
                                    &rc,
                                    &ctx,
                                    &cells.join(format!("c{k}")),
                                    p,
                                    args.keep_cells,
                                )),
                                _ => None,
                            };
                            done.push((it, r));
                        }
                    }
                    let mut fr = Vec::new();
                    let mut sts = Vec::new();
                    for (it, r) in &done {
                        let mut v = json!({"flat": it.flat.iter().map(|c| form_str(c)).collect::<Vec<_>>(),
                        "point": it.point, "why": it.why});
                        if let Some(r) = r {
                            for (k, x) in r.json().as_object().unwrap() {
                                v[k] = x.clone();
                            }
                        }
                        if it.point.is_some() {
                            let e = r.as_ref().map(Res::eff);
                            if e.as_deref() != Some("GUARDED") {
                                sts.push(e);
                            }
                        }
                        fr.push(v);
                    }
                    rec["flat_results"] = Value::Array(fr);
                    // guard-fix (coordinator 18:49Z, helper Y11): a PASS states the strata tested; it never certifies
                    rec["tested_strata"] = json!(format!(
                        "{}; suspects = own seed-coordinate layers{}{}{}",
                        if args.full {
                            "every intersection of live suspect layers (up to the case dimension)"
                        } else if args.isp_zero_intersections {
                            "the case + single live suspect layers + intersections of layers that showed a rank drop + every intersection of the live ISP-zero layers"
                        } else {
                            "the case + single live suspect layers + intersections of layers that showed a rank drop"
                        },
                        if args.low_layers { " + low layers" } else { "" },
                        if sym.is_empty() { "" } else { " + symmetry layers" },
                        if extra.contains_key(&key) { " + extra loci" } else { "" }
                    ));
                    rec["verdict_meaning"] = json!(
                        "PASS_ON_TESTED_FLATS = in the span at one generic point of each tested flat (Monte-Carlo evidence; not a certificate, suspect list incomplete); FAIL = not derived from these seeds at a tested point"
                    );
                    Ok(
                        if sts
                            .iter()
                            .any(|s| s.as_deref().is_some_and(|s| s.starts_with("FAIL")))
                        {
                            "FAIL".into()
                        } else if args.no_replay {
                            "PLANNED".into()
                        } else if !sts.is_empty()
                            && sts
                                .iter()
                                .all(|s| s.as_deref() == Some("PASS_RS2_RS3_SAMPLED"))
                        {
                            if rec["flats_capped"].as_bool() == Some(true) {
                                "PASS_ON_TESTED_FLATS_CAPPED".into()
                            } else {
                                "PASS_ON_TESTED_FLATS".into()
                            }
                        } else {
                            "UNKNOWN".into()
                        },
                    )
                })();
                match outcome {
                    Ok(v) => v,
                    Err(e) => {
                        rec["error"] = json!(e);
                        "UNKNOWN".to_string()
                    }
                }
            }
        };
        if !args.keep_cells {
            let _ = std::fs::remove_dir_all(&cells);
        }
        rec["verdict"] = json!(verdict);
        rec["seconds"] = json!((t0.elapsed().as_secs_f64() * 1000.0).round() / 1000.0);
        eprintln!(
            "{}",
            json!({"stage": "locus_rule", "chart": rule.chart, "rule": rule.index, "verdict": verdict})
        );
        *counts.entry(verdict).or_default() += 1;
        lines.push_str(&serde_json::to_string(&rec).unwrap());
        lines.push('\n');
    }
    if !args.keep_cells {
        let _ = std::fs::remove_dir(args.out.join("cells"));
    }
    std::fs::write(&results_path, lines).map_err(|e| e.to_string())?;
    let status = if counts.contains_key("FAIL") {
        "FAIL"
    } else if counts.keys().all(|k| k.starts_with("PASS_ON_TESTED_FLATS")) {
        "PASS_ON_TESTED_FLATS"
    } else {
        "UNKNOWN"
    }
    .to_string();
    let summary = json!({"schema": "rustred.locus-rank.v1", "tool": VERSION, "table": args.table, "status": status,
        "rules": rules.len(), "verdicts": counts, "dimensions": args.dimensions, "prime": P,
        "seconds": started.elapsed().as_secs_f64(),
        "scope": "FAIL = a point where the rule is not derived from these seeds (mod-P span test); not evidence that the rule is false. PASS_ON_TESTED_FLATS = PASS on generic points of the tested flats only (Monte-Carlo evidence, not a certificate); the suspect list is not complete (helper Y1/Y11)."});
    std::fs::write(
        args.out.join("SUMMARY.json"),
        serde_json::to_string_pretty(&summary).unwrap() + "\n",
    )
    .map_err(|e| e.to_string())?;
    Ok((status, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T157: &str = include_str!("../testdata/locus_T157_a4.table.jsonl");
    const R38: &str = include_str!("../testdata/locus_r38_gf1.table.jsonl");

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rr-locus-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn args(table: PathBuf, out: PathBuf, rules: &str) -> LocusArgs {
        LocusArgs {
            family: None,
            table,
            out,
            rules: LocusArgs::parse_rules(rules).unwrap(),
            chart: None,
            dimensions: super::super::DIMS.iter().map(|s| s.to_string()).collect(),
            max_flats: 64,
            full: false,
            extra_loci: None,
            keep_cells: false,
            no_replay: false,
            point_tries: 4000,
            low_layers: false,
            isp_zero_intersections: false,
            sym_census: None,
        }
    }

    fn results(out: &std::path::Path) -> BTreeMap<usize, Value> {
        std::fs::read_to_string(out.join("RESULTS.jsonl"))
            .unwrap()
            .lines()
            .map(|l| {
                let v: Value = serde_json::from_str(l).unwrap();
                (v["rule"].as_u64().unwrap() as usize, v)
            })
            .collect()
    }

    fn strs(v: &Value) -> Vec<String> {
        let mut s: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect();
        s.sort();
        s
    }

    #[test]
    fn form_strings_and_flats() {
        assert_eq!(form_str(&[-2, 0, 0, 1]), "-2+n2");
        assert_eq!(form_str(&[-75, 0, 0, 0, 40, 0, 42]), "-75+40*n3+42*n5");
        assert_eq!(form_str(&[0, 0, -1]), "-n1");
        // parallel hyperplanes never meet: {n0=1, n0=2, n1=1} -> (), 3 singles, 2 pairs
        let live = vec![vec![-1, 1, 0], vec![-2, 1, 0], vec![-1, 0, 1]];
        let f = all_flats(&live, 64);
        assert_eq!(f.len(), 1 + 3 + 2);
        assert_eq!(all_flats(&live, 3).len(), 3);
        let r = LocusArgs::parse_rules("1-2,5,1111:7").unwrap().unwrap();
        assert!(
            r.contains(&(None, 2))
                && r.contains(&(Some("1111".into()), 7))
                && !r.contains(&(None, 7))
        );
    }

    /// T157-a4 (pre-guard-fix seed-rung table): rules 1668/1669 FAIL exactly on the layer n2 = 2, the passing
    /// sample PASSes; suspects and verdicts equal guard-theory's locus_rank.py
    /// (SCRATCHPAD/guard-theory/res/000511-T157-01440/RESULTS.jsonl).
    #[test]
    fn t157_known_failures_and_passing_sample() {
        let d = tmp("t157");
        let table = d.join("t.jsonl");
        std::fs::write(&table, T157).unwrap();
        let out = d.join("out");
        let (status, summary) = run(&args(table, out.clone(), "all")).unwrap();
        assert_eq!(status, "FAIL", "{summary}");
        let res = results(&out);
        let python: &[(usize, &str, &[&str], &[&str])] = &[
            (
                1440,
                "PASS_ON_TESTED_FLATS",
                &["n19", "n12", "n11", "n10", "1+n18", "1+n11"],
                &[],
            ),
            (
                1585,
                "PASS_ON_TESTED_FLATS",
                &["-1+n9", "-1+n1", "1+n15"],
                &[],
            ),
            (
                1599,
                "PASS_ON_TESTED_FLATS",
                &["-1+n1", "n10", "1+n10"],
                &["n10"],
            ),
            (
                1609,
                "PASS_ON_TESTED_FLATS",
                &[
                    "n19", "n11", "n10", "1+n19", "1+n12", "1+n11", "1+n10", "2+n12",
                ],
                &[],
            ),
            (1668, "FAIL", &["-2+n2"], &["-2+n2"]),
            (1669, "FAIL", &["-2+n2"], &["-2+n2"]),
            (
                1689,
                "PASS_ON_TESTED_FLATS",
                &["n18", "n11", "n10"],
                &["n18", "n11", "n10"],
            ),
            (
                1699,
                "PASS_ON_TESTED_FLATS",
                &["n11", "n10", "1+n18", "1+n11"],
                &["1+n18"],
            ),
            (
                1732,
                "PASS_ON_TESTED_FLATS",
                &["n12", "n10", "1+n10"],
                &["n12", "n10"],
            ),
            (1773, "PASS_ON_TESTED_FLATS", &[], &[]),
            (1780, "PASS_ON_TESTED_FLATS", &["-1+n1"], &[]),
            (1816, "PASS_ON_TESTED_FLATS", &["-1+n0"], &[]),
            (1819, "PASS_ON_TESTED_FLATS", &[], &[]),
        ];
        for (rule, verdict, suspects, hot) in python {
            let r = &res[rule];
            assert_eq!(r["verdict"], json!(verdict), "rule {rule}: {r}");
            let mut s: Vec<String> = suspects.iter().map(|x| x.to_string()).collect();
            s.sort();
            assert_eq!(strs(&r["suspects"]), s, "rule {rule} suspects");
            let mut h: Vec<String> = hot.iter().map(|x| x.to_string()).collect();
            h.sort();
            assert_eq!(strs(&r["rank_drop_suspects"]), h, "rule {rule} rank drops");
        }
        for rule in [1668, 1669] {
            let fr = res[&rule]["flat_results"].as_array().unwrap();
            assert_eq!(fr[0]["flat"], json!([]));
            assert_eq!(fr[0]["status"], "PASS_RS2_RS3_SAMPLED");
            let layer = fr.iter().find(|x| x["flat"] == json!(["-2+n2"])).unwrap();
            assert_eq!(layer["status"], "FAIL_RS2_RS3_SAMPLED");
            assert_eq!(layer["point"][2], 2);
            // rank drop on the layer (python: 1070 generic -> 1068 on n2 = 2 for rule 1668)
            assert!(layer["rank"][0].as_u64() < fr[0]["rank"][0].as_u64());
        }
        assert_eq!(
            res[&1668]["flat_results"][0]["rank"],
            json!([1070, 1070, 1070])
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Planted defect: r38 regenerated with witness guards (rr_node_gf1) carries the hidden pole n3 - 1 as an
    /// exception and passes; removing that one branch reproduces Astra's failing r38 layer and must FAIL there.
    #[test]
    fn planted_defect_r38_cleared_witness_branch_fails() {
        let d = tmp("r38");
        let good = d.join("good.jsonl");
        std::fs::write(&good, R38).unwrap();
        let (status, summary) = run(&args(good, d.join("good"), "0")).unwrap();
        assert_eq!(status, "PASS_ON_TESTED_FLATS", "{summary}");
        let r = &results(&d.join("good"))[&0];
        assert_eq!(
            strs(&r["suspects"]),
            Vec::<String>::new(),
            "every hidden pole is an exception: {r}"
        );

        let planted: String = R38
            .lines()
            .map(|l| {
                let mut v: Value = serde_json::from_str(l).unwrap();
                if v["type"] == "rule" {
                    let ex: Vec<Value> = v["exceptions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|b| **b != json!(["-1+n3"]))
                        .cloned()
                        .collect();
                    assert_eq!(ex.len() + 1, v["exceptions"].as_array().unwrap().len());
                    v["exceptions"] = Value::Array(ex);
                }
                serde_json::to_string(&v).unwrap() + "\n"
            })
            .collect();
        let bad = d.join("bad.jsonl");
        std::fs::write(&bad, planted).unwrap();
        let (status, _) = run(&args(bad, d.join("bad"), "0")).unwrap();
        assert_eq!(status, "FAIL");
        let r = &results(&d.join("bad"))[&0];
        assert_eq!(strs(&r["suspects"]), vec!["-1+n3".to_string()]);
        let fr = r["flat_results"].as_array().unwrap();
        assert_eq!(fr[0]["status"], "PASS_RS2_RS3_SAMPLED");
        let layer = fr.iter().find(|x| x["flat"] == json!(["-1+n3"])).unwrap();
        assert_eq!(layer["status"], "FAIL_RS2_RS3_SAMPLED");
        assert_eq!(layer["point"][3], 1);
        let _ = std::fs::remove_dir_all(&d);
    }
}
