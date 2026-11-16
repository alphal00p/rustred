//! VALIDITY (no coefficient pole at a selected key, generic d), re-derived from the rule text. For every rule and every
//! divisor D(n, d) written in its coefficients (the engine evaluates the text as written; a divisor that vanishes is a
//! "coefficient pole"), the pole locus for generic d is L = {k : every d-coefficient of D(k, d) vanishes}. The
//! verifier computes the d-coefficients itself (Symbolica), restricts them to the rule's case (fixed slots, equations),
//! covers L by affine faces (linear members cut, nonlinear ones are factored, integer zeros of nonlinear factors are
//! enumerated in the bounded region), and on each face proves: an exception branch vanishes identically there (the
//! face holds no selected key), or the face is empty in the region, or by exact enumeration no lattice point of the
//! face is a pole selected by this rule at first match. A selected pole is reported with its key.
//! The records' own `witness_loci` are used only as CLAIMS to compare with.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use symbolica::poly::PolyVariable;
use symbolica::poly::factor::Factorize;

use crate::cover::{
    enum_simplex, lin_to_qpoly, qpoly_eval, qpoly_to_lin, qpoly_vars, total_degree,
};
use crate::dispatch;
use crate::expr::{Expr, QPoly, ZPoly, poly_vars, to_zpoly, zpoly_to_q};
use crate::face::{Face, Region};
use crate::table::{Rule, Table, for_each_line, parse_rule};
use crate::{Args, die};

pub struct Ctx<'a> {
    #[allow(dead_code)]
    pub t: &'a Table,
    pub region: Region,
    pub n: usize,
    pub vars: Arc<Vec<PolyVariable>>,
    pub point_cap: usize,
    pub zero_cap: u128,
}

pub fn restrict(p: &QPoly, f: &Face, n: usize) -> QPoly {
    let mut q = p.clone();
    for (v, e) in &f.rows {
        if q.degree(*v) > 0 {
            let ep = lin_to_qpoly(e, &q, n);
            q = q.replace_with_poly(*v, &ep);
        }
    }
    q
}

impl Ctx<'_> {
    /// Faces whose union contains the common integer zeros of `polys` on `f` in the region.
    pub fn locus_faces(
        &self,
        f: &Face,
        polys: &[QPoly],
        depth: usize,
    ) -> Result<Vec<Face>, String> {
        if depth > 64 {
            return Err("locus recursion too deep".into());
        }
        if !f.feasible(&self.region) {
            return Ok(vec![]);
        }
        let n = self.n;
        let mut rest: Vec<QPoly> = vec![];
        for p in polys {
            let q = restrict(p, f, n);
            if q.is_zero() {
                continue;
            }
            if q.is_constant() {
                return Ok(vec![]);
            }
            rest.push(q);
        }
        if rest.is_empty() {
            return Ok(vec![f.clone()]);
        }
        if let Some(i) = rest.iter().position(|q| total_degree(q) <= 1) {
            let l = qpoly_to_lin(&rest[i], n).ok_or("linear locus polynomial with d")?;
            let Some(g) = f.add_eq(&l) else {
                return Ok(vec![]);
            };
            let others: Vec<QPoly> = rest
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, q)| q.clone())
                .collect();
            return self.locus_faces(&g, &others, depth + 1);
        }
        let (pi, _) = rest
            .iter()
            .enumerate()
            .min_by_key(|(_, q)| (qpoly_vars(q).len(), total_degree(q)))
            .unwrap();
        let p = rest[pi].clone();
        let others: Vec<QPoly> = rest
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != pi)
            .map(|(_, q)| q.clone())
            .collect();
        let mut out = vec![];
        for (fac, _) in p.factor() {
            if fac.is_constant() {
                continue;
            }
            if total_degree(&fac) <= 1 {
                let l = qpoly_to_lin(&fac, n).ok_or("linear factor with d")?;
                if let Some(g) = f.add_eq(&l) {
                    out.extend(self.locus_faces(&g, &others, depth + 1)?);
                }
                continue;
            }
            let vs = qpoly_vars(&fac);
            if vs.iter().any(|&v| v >= n) {
                return Err("nonlinear locus factor uses d".into());
            }
            if vs.len() == 1 {
                // irreducible over Q, degree >= 2, one variable: no integer root
                continue;
            }
            let Some(r) = self.region.r else {
                return Err("nonlinear locus factor, unbounded region".into());
            };
            let budget = r - f.pinned_dr(&self.region);
            if budget < 0 {
                continue;
            }
            let m = vs.len() as u128;
            let mut size: u128 = 1;
            for i in 1..=m {
                size = size * (budget as u128 + i) / i;
            }
            if size > self.zero_cap {
                return Err(format!(
                    "nonlinear locus factor in {} variables over the enumeration cap",
                    vs.len()
                ));
            }
            let mut assign = vec![0i64; n + 1];
            let mut zeros: Vec<Vec<i64>> = vec![];
            enum_simplex(&vs, &self.region.mask, budget, &mut assign, 0, &mut |a| {
                if qpoly_eval(&fac, a).is_zero() {
                    zeros.push(vs.iter().map(|&v| a[v]).collect());
                }
            });
            for z in zeros {
                let mut g = Some(f.clone());
                for (&v, &x) in vs.iter().zip(&z) {
                    g = g.and_then(|g| g.pin(n, v, x));
                }
                if let Some(g) = g {
                    out.extend(self.locus_faces(&g, &others, depth + 1)?);
                }
            }
        }
        Ok(out)
    }

    /// Does some exception branch of the rule vanish identically on the face?
    pub fn branch_contains(&self, r: &Rule, f: &Face) -> Option<usize> {
        let n = self.n;
        'b: for (bi, br) in r.exceptions.iter().enumerate() {
            for m in br {
                if let Some(l) = &m.lin {
                    let s = f.subst(l);
                    if !(s.is_const() && s.c.is_zero()) {
                        continue 'b;
                    }
                } else {
                    let Ok(z) = to_zpoly(&m.expr.root, n, &self.vars) else {
                        continue 'b;
                    };
                    if !restrict(&zpoly_to_q(&z), f, n).is_zero() {
                        continue 'b;
                    }
                }
            }
            return Some(bi);
        }
        None
    }
}

#[derive(Default)]
struct Tally {
    rules: usize,
    rules_empty_in_region: usize,
    divisors: usize,
    constant_divisor: usize,
    constant_d_coefficient: usize,
    locus_faces: usize,
    faces_in_branch: usize,
    faces_enumerated: usize,
    points_checked: usize,
    poles_on_domain_not_selected: usize,
}

/// `poles --table T --radius R [--out FILE]`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let rs = a.need("--radius");
    let r: Option<i128> = if rs == "none" {
        None
    } else {
        Some(rs.parse().unwrap_or_else(|_| die("--radius")))
    };
    let out = a.take("--out");
    let only: Option<Vec<usize>> = a
        .take("--rules")
        .map(|s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let extra = a.take("--extra-loci");
    a.done();
    let mut extra_loci: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    if let Some(p) = extra {
        crate::table::for_each_line(&p, |_, _, v| {
            let i = v["index"].as_u64().ok_or("extra loci: index")? as usize;
            let l: Vec<String> = v["loci"]
                .as_array()
                .ok_or("extra loci: loci")?
                .iter()
                .map(|x| x.as_str().unwrap_or("").to_string())
                .collect();
            extra_loci.insert(i, l);
            Ok(())
        })
        .unwrap_or_else(|e| die(&e));
    }
    let res = run_with(&tp, r, only, &extra_loci).unwrap_or_else(|e| die(&e));
    let s = serde_json::to_string_pretty(&res).unwrap();
    if let Some(p) = out {
        std::fs::write(&p, &s).unwrap();
    }
    println!("{s}");
}

/// The validity check of a table file on D + r <= r (see the module doc).
#[cfg_attr(not(test), allow(dead_code))]
pub fn run(tp: &str, r: Option<i128>, only: Option<Vec<usize>>) -> Result<Value, String> {
    run_with(tp, r, only, &BTreeMap::new())
}

/// As [`run`], with explicit witness loci replacing a record's listed ones (e.g. from an engine re-lift).
pub fn run_with(
    tp: &str,
    r: Option<i128>,
    only: Option<Vec<usize>>,
    extra_loci: &BTreeMap<usize, Vec<String>>,
) -> Result<Value, String> {
    let t = Table::load(tp)?;
    let n = t.n;
    let ctx = Ctx {
        t: &t,
        region: Region {
            mask: t.mask.clone(),
            r,
        },
        n,
        vars: poly_vars(n),
        point_cap: 400_000,
        zero_cap: 20_000_000,
    };
    let mut tally = Tally::default();
    let mut violations: Vec<Value> = vec![];
    let mut undecided: Vec<Value> = vec![];
    let mut claim_cmp: BTreeMap<String, u64> = BTreeMap::new();
    let mut claim_unmatched: Vec<Value> = vec![];
    let mut witness_tally: BTreeMap<String, u64> = BTreeMap::new();
    let mut elided_report: Vec<Value> = vec![];
    let mut pos = 0usize;
    let t0 = std::time::Instant::now();
    for_each_line(tp, |_, _, v| {
        if v["type"].as_str() != Some("rule") {
            return Ok(());
        }
        let mut rule = parse_rule(&v, n, pos).map_err(|e| e.to_string())?;
        if let Some(l) = extra_loci.get(&rule.pos) {
            rule.witness_loci = Some(l.clone());
        }
        pos += 1;
        if only.as_ref().is_some_and(|o| !o.contains(&rule.pos)) {
            return Ok(());
        }
        tally.rules += 1;
        // the rule's case as a face
        let mut f = Face::top();
        let mut ok = true;
        for (i, x) in rule.fixed.iter().enumerate() {
            if let Some(x) = x {
                match f.pin(n, i, *x) {
                    Some(g) => f = g,
                    None => ok = false,
                }
            }
        }
        for e in &rule.equations {
            if let Some(l) = &e.lin {
                match f.add_eq(l) {
                    Some(g) => f = g,
                    None => ok = false,
                }
            }
        }
        if !ok || !f.feasible(&ctx.region) {
            tally.rules_empty_in_region += 1;
            return Ok(());
        }
        // distinct divisors as written
        let mut dens: BTreeMap<String, ZPoly> = BTreeMap::new();
        for term in v["rhs"].as_array().unwrap() {
            let text = term[1].as_str().unwrap_or("");
            if !text.contains('/') && !text.contains("^-") && !text.contains("^(-") {
                continue;
            }
            let e = Expr::parse(text, n).map_err(|e| format!("rule {}: {e}", rule.pos))?;
            for dnode in e.divisors() {
                if crate::expr::node_has_division(&dnode) {
                    return Err(format!("rule {}: nested division in a divisor", rule.pos));
                }
                let z = to_zpoly(&dnode, n, &ctx.vars)?;
                dens.entry(format!("{z}")).or_insert(z);
            }
        }
        let mut my_loci = 0usize;
        let mut rule_viol = 0usize;
        let mut rule_und = 0usize;
        // items: every written divisor, then every LISTED witness locus polynomial of the record (the spec's validity:
        // its d-coefficient locus inside the exceptions on the selected domain)
        let mut items: Vec<(&'static str, String, ZPoly)> = dens
            .iter()
            .map(|(t, z)| ("divisor", t.clone(), z.clone()))
            .collect();
        if let Some(wl) = &rule.witness_loci {
            for w in wl {
                match Expr::parse(w, n)
                    .ok()
                    .and_then(|e| to_zpoly(&e.root, n, &ctx.vars).ok())
                {
                    Some(z) => items.push(("listed witness locus", w.clone(), z)),
                    None => {
                        *witness_tally
                            .entry("listed loci not parseable (elided)".into())
                            .or_default() += 1
                    }
                }
            }
        }
        for (kind, dtext, d) in &items {
            let kind: &str = kind;
            if kind == "divisor" {
                tally.divisors += 1;
            } else {
                *witness_tally
                    .entry("listed loci checked".into())
                    .or_default() += 1;
            }
            let dq = restrict(&zpoly_to_q(d), &f, n);
            if dq.is_zero() {
                violations.push(json!({"rule": rule.pos, "kind": kind, "divisor": crate::coef::short(dtext), "why": "identically zero on the case"}));
                rule_viol += 1;
                continue;
            }
            if dq.is_constant() {
                if kind == "divisor" {
                    tally.constant_divisor += 1;
                } else {
                    *witness_tally
                        .entry("listed loci: constant on the case (empty)".into())
                        .or_default() += 1;
                }
                continue;
            }
            let coeffs: Vec<QPoly> = dq
                .to_univariate_polynomial_list(n)
                .into_iter()
                .map(|(c, _)| c)
                .collect();
            if coeffs.iter().any(|c| c.is_constant() && !c.is_zero()) {
                if kind == "divisor" {
                    tally.constant_d_coefficient += 1;
                } else {
                    *witness_tally
                        .entry("listed loci: constant d-coefficient (empty)".into())
                        .or_default() += 1;
                }
                continue;
            }
            if kind == "divisor" {
                my_loci += 1;
            }
            let faces = match ctx.locus_faces(&f, &coeffs, 0) {
                Ok(x) => x,
                Err(e) => {
                    undecided.push(
                        json!({"rule": rule.pos, "kind": kind, "divisor": crate::coef::short(dtext), "why": e}),
                    );
                    rule_und += 1;
                    continue;
                }
            };
            for g in faces {
                if kind != "divisor" {
                    *witness_tally
                        .entry("listed loci: faces".into())
                        .or_default() += 1;
                    if ctx.branch_contains(&rule, &g).is_some() {
                        *witness_tally
                            .entry("listed loci: faces inside an exception branch".into())
                            .or_default() += 1;
                        continue;
                    }
                } else {
                    tally.locus_faces += 1;
                    if ctx.branch_contains(&rule, &g).is_some() {
                        tally.faces_in_branch += 1;
                        continue;
                    }
                }
                tally.faces_enumerated += 1;
                if ctx.region.r.is_none() {
                    undecided.push(json!({"rule": rule.pos, "kind": kind, "divisor": crate::coef::short(dtext),
                        "why": "locus face not inside an exception branch (unbounded region: not enumerated)", "face": g.describe()}));
                    rule_und += 1;
                    continue;
                }
                match g.lattice_points(&ctx.region, ctx.point_cap) {
                    Ok(pts) => {
                        for k in pts {
                            tally.points_checked += 1;
                            // exact pole test at the key: every d-coefficient of D vanishes
                            let pole = coeffs.iter().all(|c| {
                                let mut assign = k.clone();
                                assign.push(0);
                                qpoly_eval(c, &assign).is_zero()
                            });
                            if !pole || !dispatch::applies(&rule, &k) {
                                continue;
                            }
                            if dispatch::first_match(&t, &k) == Some(rule.pos) {
                                violations.push(json!({"rule": rule.pos, "kind": kind, "key": k, "divisor": crate::coef::short(dtext),
                                    "why": "pole locus point at a key the rule is selected at"}));
                                rule_viol += 1;
                            } else {
                                tally.poles_on_domain_not_selected += 1;
                            }
                        }
                    }
                    Err(c) => {
                        undecided.push(json!({"rule": rule.pos, "kind": kind, "divisor": crate::coef::short(dtext),
                            "why": format!("locus face with more than {c} lattice points and no containing branch")}));
                        rule_und += 1;
                    }
                }
            }
        }
        // claims: the record's witness loci (when listed)
        match &rule.witness_loci {
            Some(wl) if !wl.is_empty() => {
                let mut matched = 0;
                let mut why: Vec<String> = vec![];
                for w in wl {
                    let Ok(e) = Expr::parse(w, n) else {
                        why.push(format!("not a polynomial: {}", crate::coef::short(w)));
                        continue;
                    };
                    let Ok(wz) = to_zpoly(&e.root, n, &ctx.vars) else {
                        continue;
                    };
                    if dens.values().any(|d| d.try_div(&wz).is_some()) {
                        matched += 1;
                    } else {
                        why.push(format!("divides no divisor: {}", crate::coef::short(w)));
                    }
                }
                if matched < wl.len() {
                    claim_unmatched.push(json!({"rule": rule.pos, "source": rule.origin["source"], "listed": wl.len(), "why": why}));
                }
                *claim_cmp
                    .entry(if matched == wl.len() {
                        "every listed locus divides a divisor".into()
                    } else {
                        "some listed locus divides no divisor".into()
                    })
                    .or_default() += 1;
            }
            _ => {
                if rule.witness_poles.unwrap_or(0) > 0
                    || rule.origin["class"].as_str() == Some("appended")
                    || rule.witness_loci.as_ref().is_some_and(|w| w.is_empty()) && my_loci > 0
                {
                    elided_report.push(json!({"rule": rule.pos, "source": rule.origin["source"], "class": rule.origin["class"],
                        "witness_poles_field": rule.witness_poles, "divisors": dens.len(), "nonconstant_loci_computed": my_loci,
                        "violations": rule_viol, "undecided": rule_und}));
                }
                *claim_cmp.entry("no listed loci".into()).or_default() += 1;
            }
        }
        Ok(())
    })?;
    let res = json!({
        "schema": "rr_pverify.poles.v1", "table": tp, "radius": r.map(|x| x.to_string()),
        "rules": tally.rules, "rules_empty_in_region": tally.rules_empty_in_region,
        "divisors": tally.divisors, "constant_divisor": tally.constant_divisor,
        "constant_d_coefficient": tally.constant_d_coefficient, "locus_faces": tally.locus_faces,
        "faces_inside_an_exception_branch": tally.faces_in_branch, "faces_enumerated": tally.faces_enumerated,
        "points_checked": tally.points_checked, "poles_on_domain_where_an_earlier_rule_is_selected": tally.poles_on_domain_not_selected,
        "violations": violations, "undecided": undecided, "claims": claim_cmp, "claims_unmatched": claim_unmatched, "listed_witness_loci": witness_tally, "records_without_listed_loci": elided_report,
        "seconds": t0.elapsed().as_secs_f64(),
        "verdict": if violations.is_empty() && undecided.is_empty() { "YES" } else { "NO" },
    });
    Ok(res)
}
