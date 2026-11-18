//! (e) Locus decisions of one denominator factor on the DOMAIN of the stored rule (read from the stored line only).
//!
//! A key k of the domain is a hidden validity locus of F(m, d) = sum_i c_i(m) d^i only if F(k, d) == 0 identically in d,
//! i.e. every c_i(k) = 0 (rules hold in Q(d)). Decisions, in this order:
//!   D_ONLY          F has no index variable (a pole in d only, never a validity locus);
//!   (c1) EMPTY_CONSTANT  some c_i is a nonzero constant;
//!   (c2) EMPTY_SIGN      on the box B containing the domain, some c_i has strict sign (alg::sign_on_box);
//!   (c3) EMPTY_IDEAL     the Groebner basis of <c_i> over Q is {1};
//!        IN_EXCEPTION    every d-coefficient of every polynomial of one exception branch lies in <c_i>: every point of
//!                        {c_i = 0} is on that branch, hence outside the domain;
//!   (c4) OPEN            none of the above (locus description in the argument, for key enumeration elsewhere).
use std::collections::BTreeMap;

use super::Rule;
use super::alg::{self, Bound, Poly, Vars};
use crate::core::jstr;
use crate::stream::solve_case;

/// Affine case: dependent slot := polynomial in the other slots (canonical frame).
pub fn dependent_forms(vars: &Vars, rule: &Rule) -> Result<Vec<(usize, Poly)>, String> {
    if rule.equations.is_empty() {
        return Ok(vec![]);
    }
    let mut fixed = [None; crate::core::MAXN];
    for (i, f) in rule.fixed.iter().enumerate().take(crate::core::MAXN) {
        fixed[i] = *f;
    }
    let solved = solve_case(&rule.equations, &fixed)?;
    Ok(solved
        .into_iter()
        .map(|(v, f)| {
            let lin: Vec<(usize, i64)> = f
                .lin
                .iter()
                .map(|&(x, c)| (Vars::slot(x as usize), c))
                .collect();
            (Vars::slot(v), vars.affine(f.c0, &lin))
        })
        .collect())
}

/// The domain as read from the stored rule.
pub struct Domain {
    /// var -> bounds (symbolic, independent slots only)
    pub bounds: BTreeMap<usize, Bound>,
    pub free: Vec<usize>,
    pub dependent: Vec<usize>,
    /// exception branches: per branch, the d-coefficients of all its polynomials (canonical frame, case substituted)
    pub branches: Vec<Vec<Poly>>,
    pub branch_texts: Vec<Vec<String>>,
}

impl Domain {
    pub fn from_rule(vars: &Vars, rule: &Rule) -> Result<Domain, String> {
        let forms = dependent_forms(vars, rule)?;
        let dependent: Vec<usize> = forms.iter().map(|x| x.0).collect();
        let fixedv: Vec<(usize, i64)> = rule
            .fixed
            .iter()
            .enumerate()
            .filter_map(|(i, x)| x.map(|v| (Vars::slot(i), v)))
            .collect();
        let mut bounds = BTreeMap::new();
        let mut free = vec![];
        for i in 0..rule.arity {
            if rule.fixed[i].is_some() || dependent.contains(&Vars::slot(i)) {
                continue;
            }
            free.push(Vars::slot(i));
            let mut b = Bound {
                lo: rule.bounds_min[i],
                hi: rule.bounds_max[i],
            };
            // the chart condition on the target value at this slot: '1' -> >= 1, '0' -> <= 0
            let (sym, off) = rule.target[i];
            if sym {
                match rule.chart.as_bytes().get(i) {
                    Some(b'1') => b.lo = Some(b.lo.map_or(1 - off, |x| x.max(1 - off))),
                    Some(b'0') => b.hi = Some(b.hi.map_or(-off, |x| x.min(-off))),
                    _ => {}
                }
            }
            bounds.insert(Vars::slot(i), b);
        }
        let mut branches = vec![];
        for br in &rule.exceptions {
            let mut cs = vec![];
            for t in br {
                let p = vars
                    .poly(t)
                    .map_err(|e| format!("exception polynomial {t}: {e}"))?;
                let q = alg::subst_poly_forms(&alg::subst_poly(&p, &fixedv), &forms);
                for (_, c) in alg::d_coefficients(&q) {
                    cs.push(c);
                }
                if q.is_zero() {
                    cs.push(q);
                }
            }
            branches.push(cs);
        }
        Ok(Domain {
            bounds,
            free,
            dependent,
            branches,
            branch_texts: rule.exceptions.clone(),
        })
    }

    pub fn json(&self, vars: &Vars, rule: &Rule) -> String {
        let bx: Vec<String> = self
            .bounds
            .iter()
            .map(|(v, b)| {
                format!(
                    "{}:[{},{}]",
                    jstr(vars.name(*v)),
                    b.lo.map(|x| x.to_string()).unwrap_or("null".into()),
                    b.hi.map(|x| x.to_string()).unwrap_or("null".into())
                )
            })
            .collect();
        format!(
            "{{\"source\":\"stored rule line (fixed, equations, domain_bounds, chart of the target, exceptions)\",\"free\":[{}],\"dependent\":[{}],\"box\":{{{}}},\"has_domain_bounds\":{},\"equations\":{},\"exception_branches\":{}}}",
            self.free
                .iter()
                .map(|v| jstr(vars.name(*v)))
                .collect::<Vec<_>>()
                .join(","),
            self.dependent
                .iter()
                .map(|v| jstr(vars.name(*v)))
                .collect::<Vec<_>>()
                .join(","),
            bx.join(","),
            rule.has_bounds,
            rule.equation_texts.len(),
            self.branch_texts.len()
        )
    }
}

pub struct Decision {
    pub verdict: &'static str,
    pub argument: String,
}

fn ptxt(p: &Poly) -> String {
    jstr(&alg::short(p.to_string(), 300))
}

/// Decide one canonical-frame factor (case substituted).
pub fn decide(vars: &Vars, dom: &Domain, f: &Poly) -> Decision {
    if f.is_zero() {
        return Decision {
            verdict: "OPEN",
            argument: "{\"reason\":\"the factor vanishes identically on the case: every key of the domain is on the locus\"}".into(),
        };
    }
    if !alg::has_index(f) {
        return Decision {
            verdict: "D_ONLY",
            argument: "{\"reason\":\"no index variable: a pole in d only\"}".into(),
        };
    }
    let cs = alg::d_coefficients(f);
    // (c1)
    for (e, c) in &cs {
        if c.is_constant() && !c.is_zero() {
            return Decision {
                verdict: "EMPTY_CONSTANT",
                argument: format!("{{\"d_power\":{},\"coefficient\":{}}}", e, ptxt(c)),
            };
        }
    }
    // (c2)
    for (e, c) in &cs {
        for prefer_upper in [false, true] {
            if let Some((c0, how)) = alg::sign_on_box(c, &dom.bounds, prefer_upper) {
                let how: Vec<String> = how
                    .split(',')
                    .filter(|x| !x.is_empty())
                    .map(|x| {
                        let (v, rest) = x.split_once('=').unwrap_or((x, ""));
                        let v: usize = v.parse().unwrap_or(0);
                        format!(
                            "{}={}",
                            vars.name(v),
                            rest.replace("u", &format!("u_{}", vars.name(v)))
                        )
                    })
                    .collect();
                return Decision {
                    verdict: "EMPTY_SIGN",
                    argument: format!(
                        "{{\"d_power\":{},\"coefficient\":{},\"substitution\":{},\"constant\":{},\"statement\":\"after the substitution every coefficient has the sign of the nonzero constant term, all u >= 0 on the box\"}}",
                        e,
                        ptxt(c),
                        jstr(&how.join(", ")),
                        jstr(&c0.to_string())
                    ),
                };
            }
        }
    }
    // (c3)
    let gens: Vec<Poly> = cs.iter().map(|x| x.1.clone()).collect();
    let g = alg::groebner(&gens);
    if alg::is_unit_ideal(&g) {
        return Decision {
            verdict: "EMPTY_IDEAL",
            argument: format!(
                "{{\"generators\":[{}],\"groebner\":\"{{1}}\"}}",
                gens.iter().map(ptxt).collect::<Vec<_>>().join(",")
            ),
        };
    }
    for (bi, br) in dom.branches.iter().enumerate() {
        if !br.is_empty() && br.iter().all(|p| alg::in_ideal(p, &g)) {
            return Decision {
                verdict: "IN_EXCEPTION",
                argument: format!(
                    "{{\"branch\":{},\"branch_polynomials\":[{}],\"generators\":[{}],\"statement\":\"every d-coefficient of every branch polynomial has normal form 0 modulo the Groebner basis of the generators (grevlex, Q)\"}}",
                    bi,
                    dom.branch_texts[bi]
                        .iter()
                        .map(|t| jstr(t))
                        .collect::<Vec<_>>()
                        .join(","),
                    gens.iter().map(ptxt).collect::<Vec<_>>().join(",")
                ),
            };
        }
    }
    let used: Vec<usize> = (1..f.nvars()).filter(|&v| f.degree(v) > 0).collect();
    Decision {
        verdict: "OPEN",
        argument: format!(
            "{{\"variables\":[{}],\"generators\":[{}],\"groebner\":[{}],\"box\":{{{}}},\"note\":\"locus {{all generators = 0}} on the domain box minus the exception branches: enumerate its keys in the first-match cone for per-key proofs\"}}",
            used.iter()
                .map(|v| jstr(vars.name(*v)))
                .collect::<Vec<_>>()
                .join(","),
            gens.iter().map(ptxt).collect::<Vec<_>>().join(","),
            g.iter()
                .map(|p| jstr(&alg::short(p.to_string(), 300)))
                .collect::<Vec<_>>()
                .join(","),
            used.iter()
                .filter_map(|v| dom.bounds.get(v).map(|b| format!(
                    "{}:[{},{}]",
                    jstr(vars.name(*v)),
                    b.lo.map(|x| x.to_string()).unwrap_or("null".into()),
                    b.hi.map(|x| x.to_string()).unwrap_or("null".into())
                )))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
