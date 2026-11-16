//! STRICT DESCENT re-derived from the rule text. For a rule with canonical target (rule point = key) the domain is
//! over-approximated by a BOX (fixed slots; free lines [lo, inf), free numerators (-inf, hi], with lo / hi moved past
//! every chart-boundary value that a one-member exception branch removes; equations ignored = a superset, sound for
//! "for all" claims). Each right-hand-side slot is an affine function of ONE key coordinate (or a constant), so:
//! * ISP loci (a numerator slot of the term >= 1): finitely many hyperplanes n_a = v, or the whole domain;
//!   the coefficient must vanish there (the engine refuses a nonzero coefficient on such a term): exact zero proof;
//! * sector exits (a line slot of the term <= 0): the term is in a lower sector, no comparison needed;
//! * on the in-sector part (again a box) the order program is a sequence of affine forms (support equal: degree row 0
//!   = total excess, row 1 = numerator excess, then the coordinate ties of the header's priority), decided exactly on
//!   the box (ranges of affine forms; a one-variable form with a mixed sign splits the box at its root).
//!
//! Where a term is NOT lower, its coefficient must vanish (exact zero proof) or the rule fails descent.
//! Row 0 is D + r: for every in-sector term the verifier also records the change of D + r (claim (a): never raised).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::coef::{numerator_zero_on, parse_ratio, short};
use crate::expr::poly_vars;
use crate::table::{Pat, Rule, Table, for_each_line, parse_rule, target_is_canonical};
use crate::{Args, die};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Iv {
    pub lo: Option<i64>,
    pub hi: Option<i64>,
}

impl Iv {
    fn empty(&self) -> bool {
        matches!((self.lo, self.hi), (Some(a), Some(b)) if a > b)
    }
}

/// The rule's domain box (see module doc).
pub fn domain_box(r: &Rule, mask: &[bool]) -> Vec<Iv> {
    let n = mask.len();
    let mut b: Vec<Iv> = (0..n)
        .map(|i| match r.fixed[i] {
            Some(x) => Iv {
                lo: Some(x),
                hi: Some(x),
            },
            None => {
                if mask[i] {
                    Iv {
                        lo: Some(1),
                        hi: None,
                    }
                } else {
                    Iv {
                        lo: None,
                        hi: Some(0),
                    }
                }
            }
        })
        .collect();
    // one-member branches c + a n_v with an integer root remove the hyperplane n_v = root
    let mut removed: Vec<Vec<i64>> = vec![vec![]; n];
    for br in &r.exceptions {
        if br.len() == 1
            && let Some((c, v, a)) = br[0].fast
            && c % a == 0
        {
            removed[v].push((-c / a) as i64);
        }
    }
    for i in 0..n {
        if r.fixed[i].is_some() {
            if let Some(x) = r.fixed[i]
                && removed[i].contains(&x)
            {
                b[i] = Iv {
                    lo: Some(1),
                    hi: Some(0),
                }; // empty
            }
            continue;
        }
        if mask[i] {
            let mut lo = 1;
            while removed[i].contains(&lo) {
                lo += 1;
            }
            b[i].lo = Some(lo);
        } else {
            let mut hi = 0;
            while removed[i].contains(&hi) {
                hi -= 1;
            }
            b[i].hi = Some(hi);
        }
    }
    b
}

/// term slot j = coefficient * k_a + offset (a = None: constant offset)
fn slot_map(p: &Pat, j: usize) -> (Option<usize>, i64) {
    p[j]
}

/// Range of `c + sum a_i x_i` over a box: (min, max) with None = infinite.
fn range(c: i128, a: &[i128], b: &[Iv]) -> (Option<i128>, Option<i128>) {
    let (mut mn, mut mx) = (Some(c), Some(c));
    for (i, &ai) in a.iter().enumerate() {
        if ai == 0 {
            continue;
        }
        let (lo, hi) = (b[i].lo.map(|x| x as i128), b[i].hi.map(|x| x as i128));
        let (pmin, pmax) = if ai > 0 { (lo, hi) } else { (hi, lo) };
        mn = match (mn, pmin) {
            (Some(m), Some(p)) => Some(m + ai * p),
            _ => None,
        };
        mx = match (mx, pmax) {
            (Some(m), Some(p)) => Some(m + ai * p),
            _ => None,
        };
    }
    (mn, mx)
}

#[derive(Debug, Clone)]
pub enum Verdict {
    Lower,
    /// sub-boxes where the term is not lower (with the reason)
    NotLower(Vec<(Vec<Iv>, String)>),
    Undecided(String),
}

/// Lexicographic decision of "term simpler" over a box: forms h_1..h_m (term simpler iff the first nonzero h < 0).
fn decide(forms: &[(i128, Vec<i128>, String)], b: &[Iv]) -> Verdict {
    if b.iter().any(Iv::empty) {
        return Verdict::Lower;
    }
    let Some(((c, a, name), rest)) = forms.split_first() else {
        return Verdict::NotLower(vec![(b.to_vec(), "term equals target".into())]);
    };
    let (mn, mx) = range(*c, a, b);
    if mx.is_some_and(|x| x < 0) {
        return Verdict::Lower;
    }
    if mn.is_some_and(|x| x > 0) {
        return Verdict::NotLower(vec![(b.to_vec(), format!("{name} > 0"))]);
    }
    if mn == Some(0) && mx == Some(0) {
        return decide(rest, b);
    }
    // mixed: split along a single variable
    let vs: Vec<usize> = (0..a.len())
        .filter(|&i| a[i] != 0 && b[i].lo != b[i].hi)
        .collect();
    if vs.len() != 1 {
        return Verdict::Undecided(format!("{name}: mixed sign in {} variables", vs.len()));
    }
    let v = vs[0];
    // c' + a_v x = 0 with c' = c + sum over the degenerate variables
    let mut c2 = *c;
    for (i, &ai) in a.iter().enumerate() {
        if i != v && ai != 0 {
            c2 += ai * b[i].lo.unwrap() as i128;
        }
    }
    let av = a[v];
    // x < root, x = root, x > root
    let mut pieces: Vec<Vec<Iv>> = vec![];
    // root x0 = -c2 / av, exact integer floor division (no floating point)
    let (num, den) = if av > 0 { (-c2, av) } else { (c2, -av) };
    let x_floor = num.div_euclid(den) as i64;
    let exact = num.rem_euclid(den) == 0;
    let (below, exact, above): (i64, Option<i64>, i64) = if exact {
        (x_floor - 1, Some(x_floor), x_floor + 1)
    } else {
        (x_floor, None, x_floor + 1)
    };
    let mut lo_box = b.to_vec();
    lo_box[v].hi = Some(lo_box[v].hi.map_or(below, |h| h.min(below)));
    pieces.push(lo_box);
    if let Some(x) = exact {
        let mut eb = b.to_vec();
        eb[v].lo = Some(eb[v].lo.map_or(x, |l| l.max(x)));
        eb[v].hi = Some(eb[v].hi.map_or(x, |h| h.min(x)));
        pieces.push(eb);
    }
    let mut hi_box = b.to_vec();
    hi_box[v].lo = Some(hi_box[v].lo.map_or(above, |l| l.max(above)));
    pieces.push(hi_box);
    let mut fails = vec![];
    for p in pieces {
        match decide(forms, &p) {
            Verdict::Lower => {}
            Verdict::NotLower(mut f) => fails.append(&mut f),
            Verdict::Undecided(s) => return Verdict::Undecided(s),
        }
    }
    if fails.is_empty() {
        Verdict::Lower
    } else {
        Verdict::NotLower(fails)
    }
}

pub struct TermAnalysis {
    pub isp_loci: Vec<Vec<Option<i64>>>,
    pub isp_whole: bool,
    pub verdict: Verdict,
    /// max change of D + r over the in-sector part (None = unbounded above)
    pub d_dr_max: Option<i128>,
    pub in_sector_nonempty: bool,
}

/// Analyse one term pattern of a rule on the rule's domain box.
pub fn analyse_term(r: &Rule, p: &Pat, mask: &[bool], prio: &[usize], b: &[Iv]) -> TermAnalysis {
    let n = mask.len();
    let mut isp_loci = vec![];
    let mut isp_whole = false;
    // in-sector box of the KEY: start from the domain box, tighten by term-slot sector conditions
    let mut ib = b.to_vec();
    #[allow(clippy::needless_range_loop)]
    for j in 0..n {
        let (a, o) = slot_map(p, j);
        match a {
            None => {
                let in_sec = if mask[j] { o >= 1 } else { o <= 0 };
                if !mask[j] && o >= 1 {
                    isp_whole = true;
                }
                if !in_sec {
                    ib[0] = Iv {
                        lo: Some(1),
                        hi: Some(0),
                    }; // empty: never in sector
                }
            }
            Some(a) => {
                if mask[j] {
                    // k_a + o >= 1  <=>  k_a >= 1 - o
                    ib[a].lo = Some(ib[a].lo.map_or(1 - o, |l| l.max(1 - o)));
                } else {
                    // ISP where k_a + o >= 1; in sector where k_a <= -o
                    let lo = b[a].lo;
                    let hi = b[a].hi;
                    let from = lo.map_or(1 - o, |l| l.max(1 - o));
                    match hi {
                        Some(h) => {
                            if from <= h {
                                if h - from > 64 {
                                    isp_whole = true;
                                } else {
                                    for v in from..=h {
                                        let mut pins = r.fixed.clone();
                                        pins[a] = Some(v);
                                        isp_loci.push(pins);
                                    }
                                }
                            }
                        }
                        None => isp_whole = true,
                    }
                    ib[a].hi = Some(ib[a].hi.map_or(-o, |h| h.min(-o)));
                }
            }
        }
    }
    let in_sector_nonempty = !ib.iter().any(Iv::empty);
    // affine forms of the comparison (term simpler iff first nonzero h < 0)
    // term slot j: t_j = k_a + o or o; key slot j: k_j.
    let lin_of_slot = |j: usize| -> (i128, Vec<i128>) {
        let mut a = vec![0i128; n];
        let (ax, o) = slot_map(p, j);
        let c = match ax {
            Some(x) => {
                a[x] += 1;
                o as i128
            }
            None => o as i128,
        };
        a[j] -= 1; // minus k_j
        (c, a)
    };
    let mut forms: Vec<(i128, Vec<i128>, String)> = vec![];
    // row 0: sum lines (t-k) - sum numerators (t-k)
    let mut c0 = 0i128;
    let mut a0 = vec![0i128; n];
    let mut c1 = 0i128;
    let mut a1 = vec![0i128; n];
    for (j, &mj) in mask.iter().enumerate() {
        let (c, a) = lin_of_slot(j);
        let s = if mj { 1 } else { -1 };
        c0 += s * c;
        for i in 0..n {
            a0[i] += s * a[i];
        }
        if !mj {
            c1 -= c;
            for i in 0..n {
                a1[i] -= a[i];
            }
        }
    }
    let (_, d_dr_max) = range(c0, &a0, &ib);
    forms.push((c0, a0, "degree row 0 (D + r)".into()));
    forms.push((c1, a1, "degree row 1 (numerator degree)".into()));
    for pass in 0..2 {
        for &ax in prio {
            if mask[ax] != (pass == 0) {
                continue;
            }
            let (c, a) = lin_of_slot(ax);
            // lines: simpler iff (t-k) > 0  -> h = -(t-k); numerators: excess -t: simpler iff -(t-k) > 0 -> h = (t-k)
            if mask[ax] {
                forms.push((
                    -c,
                    a.iter().map(|x| -x).collect(),
                    format!("coordinate n{ax}"),
                ));
            } else {
                forms.push((c, a, format!("coordinate n{ax}")));
            }
        }
    }
    let verdict = if in_sector_nonempty {
        decide(&forms, &ib)
    } else {
        Verdict::Lower
    };
    TermAnalysis {
        isp_loci,
        isp_whole,
        verdict,
        d_dr_max: if in_sector_nonempty {
            d_dr_max
        } else {
            Some(i128::MIN)
        },
        in_sector_nonempty,
    }
}

/// `descent --table T [--out FILE] [--claims descent.jsonl]`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let out = a.take("--out");
    a.done();
    let res = run(&tp).unwrap_or_else(|e| die(&e));
    let s = serde_json::to_string_pretty(&res).unwrap();
    if let Some(p) = out {
        std::fs::write(&p, &s).unwrap();
    }
    println!("{s}");
}

/// The descent check of a table file (see the module doc).
pub fn run(tp: &str) -> Result<Value, String> {
    let tp = tp.to_string();
    let mut header = Value::Null;
    let mut n = 0usize;
    let mut mask: Vec<bool> = vec![];
    let mut prio: Vec<usize> = vec![];
    let mut sprio: Vec<usize> = vec![];
    let mut pos = 0usize;
    let mut stats: BTreeMap<String, u64> = BTreeMap::new();
    let mut failures: Vec<Value> = vec![];
    let mut undecided: Vec<Value> = vec![];
    let mut dr_raise: Vec<Value> = vec![];
    let mut zero_proofs: BTreeMap<String, u64> = BTreeMap::new();
    let mut per_rule_ok = 0usize;
    let mut permuted_rules = vec![];
    let mut max_ddr: i128 = i128::MIN;
    let t0 = std::time::Instant::now();
    let mut vars = poly_vars(1);
    for_each_line(&tp, |_, _, v| {
        match v["type"].as_str().unwrap_or("") {
            "header" => {
                header = v.clone();
                n = v["denominators"].as_u64().unwrap() as usize;
                mask = v["mask"]
                    .as_str()
                    .unwrap()
                    .chars()
                    .map(|c| c == '1')
                    .collect();
                let mut t = Table {
                    n,
                    mask: mask.clone(),
                    zero: Default::default(),
                    rules: vec![],
                    masters: vec![],
                    header: v.clone(),
                    has_end: false,
                    record_lines: 0,
                };
                t.header = v.clone();
                let od = t.order_data().map_err(|e| e.to_string())?;
                sprio = od.0;
                prio = od.1;
                vars = poly_vars(n);
            }
            "rule" => {
                let r = parse_rule(&v, n, pos).map_err(|e| format!("rule {pos}: {e}"))?;
                pos += 1;
                if r.skip {
                    *stats.entry("skipped (dispatch)".into()).or_default() += 1;
                    return Ok(());
                }
                if !target_is_canonical(&r) {
                    undecided.push(json!({"rule": r.pos, "why": "non-canonical target"}));
                    return Ok(());
                }
                let b = domain_box(&r, &mask);
                let rhs = v["rhs"].as_array().unwrap();
                let permuted = r.rhs_pats.iter().any(|p| {
                    p.iter()
                        .enumerate()
                        .any(|(j, (ax, _))| ax.is_some_and(|x| x != j))
                });
                if permuted {
                    permuted_rules.push(r.pos);
                }
                let mut rule_ok = true;
                // concrete sample keys of the domain box (low corner, and the corner moved by 3 along each free slot)
                let corner: Vec<i64> = b.iter().map(|iv| iv.lo.or(iv.hi).unwrap()).collect();
                let mut samples: Vec<Vec<i64>> = vec![];
                if !b.iter().any(Iv::empty) {
                    samples.push(corner.clone());
                    for i in 0..n {
                        if b[i].lo != b[i].hi {
                            let mut k = corner.clone();
                            k[i] += if mask[i] { 3 } else { -3 };
                            samples.push(k);
                        }
                    }
                }
                let concrete = crate::order::Order {
                    support_priority: sprio.clone(),
                    coordinate_priority: prio.clone(),
                };
                for (ti, p) in r.rhs_pats.iter().enumerate() {
                    *stats.entry("terms".into()).or_default() += 1;
                    let an = analyse_term(&r, p, &mask, &prio, &b);
                    // cross-check: where the symbolic analysis says Lower, the concrete comparator must agree at every
                    // sample key whose term is in the sector (no ISP slot, no line <= 0)
                    if matches!(an.verdict, Verdict::Lower) {
                        for k in &samples {
                            if !crate::dispatch::applies(&r, k) {
                                continue;
                            }
                            let tk = crate::table::at(p, k);
                            if !crate::dispatch::in_chart(&mask, &tk) {
                                continue;
                            }
                            *stats
                                .entry("concrete order cross-checks".into())
                                .or_default() += 1;
                            if concrete.cmp_simpler(&tk, k) != std::cmp::Ordering::Less {
                                failures.push(json!({"rule": r.pos, "term": ti, "key": k, "why": "concrete comparator disagrees with the symbolic verdict"}));
                                rule_ok = false;
                            }
                        }
                    }
                    if let Some(x) = an.d_dr_max
                        && an.in_sector_nonempty
                    {
                        max_ddr = max_ddr.max(x);
                    }
                    if an.in_sector_nonempty && an.d_dr_max.is_none_or(|x| x > 0) {
                        // only a problem where the term is also not lower (checked below); record anyway
                        dr_raise.push(json!({"rule": r.pos, "term": ti, "max_change": an.d_dr_max.map(|x| x.to_string())}));
                    }
                    let mut obligations: Vec<(Vec<Option<i64>>, String)> = vec![];
                    if an.isp_whole {
                        obligations.push((
                            r.fixed.clone(),
                            "ISP on an unbounded part / whole domain".into(),
                        ));
                    }
                    for l in &an.isp_loci {
                        obligations.push((l.clone(), "ISP hyperplane".into()));
                    }
                    match &an.verdict {
                        Verdict::Lower => {}
                        Verdict::NotLower(boxes) => {
                            for (bx, why) in boxes {
                                // zero on the box: pins = degenerate dims of the box
                                let pins: Vec<Option<i64>> = bx
                                    .iter()
                                    .map(|iv| {
                                        if iv.lo.is_some() && iv.lo == iv.hi {
                                            iv.lo
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                obligations.push((pins, format!("not lower: {why}")));
                            }
                        }
                        Verdict::Undecided(s) => {
                            undecided.push(json!({"rule": r.pos, "term": ti, "why": s}));
                            rule_ok = false;
                        }
                    }
                    if obligations.is_empty() {
                        continue;
                    }
                    *stats
                        .entry("terms with zero obligations".into())
                        .or_default() += 1;
                    let text = rhs[ti][1].as_str().unwrap_or("");
                    let ratio = match parse_ratio(text, n, &vars) {
                        Ok(x) => x,
                        Err(e) => {
                            failures.push(json!({"rule": r.pos, "term": ti, "why": format!("coefficient not parsed: {e}")}));
                            rule_ok = false;
                            continue;
                        }
                    };
                    for (pins, why) in obligations {
                        *stats.entry("zero obligations".into()).or_default() += 1;
                        match numerator_zero_on(&ratio.num, &r, &pins, n, &vars) {
                            Ok(how) => {
                                *zero_proofs.entry(how.into()).or_default() += 1;
                            }
                            Err(e) => {
                                rule_ok = false;
                                failures.push(json!({"rule": r.pos, "term": ti, "pattern": rhs[ti][0], "obligation": why,
                                    "pins": pins, "why": e, "coefficient": short(text)}));
                            }
                        }
                    }
                }
                if rule_ok {
                    per_rule_ok += 1;
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    let _ = header;
    let res = json!({
        "schema": "rr_pverify.descent.v1", "table": tp, "rules": pos, "rules_descending_on_whole_domain": per_rule_ok,
        "stats": stats, "zero_proofs": zero_proofs, "failures": failures, "undecided": undecided,
        "permuted_rules": permuted_rules,
        "max_change_of_D_plus_r_over_in_sector_terms": max_ddr.to_string(),
        "terms_that_may_raise_D_plus_r": dr_raise.len(), "raise_examples": dr_raise.iter().take(20).collect::<Vec<_>>(),
        "seconds": t0.elapsed().as_secs_f64(),
        "verdict": if failures.is_empty() && undecided.is_empty() { "YES" } else { "NO" },
    });
    Ok(res)
}
