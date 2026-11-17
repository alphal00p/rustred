//! MODULAR-ZERO SAFETY of the engine's first match. The engine decides "literal = 0" by `Expr::is_zero_at`
//! (rustred-core node/replay/expr.rs: value mod P = 2^61-1 AND mod P2 = 2^61-31 both zero). For an integer-valued
//! literal v (every case literal here is an integer polynomial, d-free) a misread needs v != 0 with P*P2 | v, i.e.
//! |v| >= P*P2. For a key with D + r <= R and a canonical rule point (= the key on free slots, the fixed values
//! elsewhere) every line coordinate is in [1, R+1] and every numerator coordinate in [-R, 0], so
//!     |v| <= B(f, R) = sum_m |c_m| prod_i b_i^{e_mi},   b_i = |fixed_i| or R+1 (line) / R (numerator).
//! B(f, R) < P*P2 for every literal the first match can evaluate (every equation, every exception member, every
//! domain-bound branch, of every rule) makes the engine's test exact on the region. The largest such R is computed
//! per literal (exact Integer arithmetic, binary search) and the minimum over all literals is the verifier's radius.

use serde_json::{Value, json};
use symbolica::domains::integer::Integer;

use crate::expr::{poly_vars, to_zpoly};
use crate::table::{Member, Rule, Table};
use crate::{Args, die};

pub fn pp2() -> Integer {
    let p = Integer::from(2_305_843_009_213_693_951i64);
    let p2 = Integer::from(2_305_843_009_213_693_921i64);
    &p * &p2
}

/// B(f, R) for one literal of a rule.
fn bound(coeffs: &[(Integer, Vec<u16>)], r: &Rule, mask: &[bool], rad: i64) -> Integer {
    let n = mask.len();
    let b: Vec<Integer> = (0..n)
        .map(|i| match r.fixed[i] {
            Some(x) => Integer::from(x.abs()),
            None => Integer::from(if mask[i] { rad + 1 } else { rad }),
        })
        .collect();
    let mut s = Integer::zero();
    for (c, e) in coeffs {
        let mut t = c.abs();
        for i in 0..n {
            if e[i] > 0 {
                t = &t * &b[i].pow(e[i] as u64);
            }
        }
        s += &t;
    }
    s
}

fn literal_coeffs(m: &Member, n: usize) -> Result<Vec<(Integer, Vec<u16>)>, String> {
    if m.uses_d || m.expr.has_division() {
        return Err(format!("literal with d or division: {}", m.text));
    }
    let vars = poly_vars(n);
    let p = to_zpoly(&m.expr.root, n, &vars)?;
    Ok(p.coefficients
        .iter()
        .zip(p.exponents_iter())
        .map(|(c, e)| (c.clone(), e.to_vec()))
        .collect())
}

/// `radius --table T --radius R [--out FILE]`
pub fn cmd(mut a: Args) {
    let tp = a.need("--table");
    let rclaim: i64 = a
        .need("--radius")
        .parse()
        .unwrap_or_else(|_| die("--radius"));
    let out = a.take("--out");
    a.done();
    let t = Table::load(&tp).unwrap_or_else(|e| die(&e));
    let n = t.n;
    let lim = pp2();
    let p = Integer::from(2_305_843_009_213_693_951i64);
    let p2 = Integer::from(2_305_843_009_213_693_921i64);
    let mut literals = 0usize;
    let mut unsafe_at_claim: Vec<Value> = vec![];
    let mut per_rule: Vec<(i64, usize, String)> = vec![];
    let mut content_hits = vec![];
    let mut errors = vec![];
    let mut global_min = i64::MAX;
    let mut binding: Option<Value> = None;
    for r in &t.rules {
        let mut lits: Vec<(&Member, &str)> = vec![];
        for e in &r.equations {
            lits.push((e, "equation"));
        }
        for (bi, br) in r.exceptions.iter().enumerate() {
            for m in br {
                lits.push((
                    m,
                    if bi < r.n_record_branches {
                        "exception member"
                    } else {
                        "domain-bound branch"
                    },
                ));
            }
        }
        let mut rule_min = i64::MAX;
        for (m, kind) in lits {
            literals += 1;
            let cs = match literal_coeffs(m, n) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(json!({"rule": r.pos, "literal": m.text, "why": e}));
                    continue;
                }
            };
            // content divisible by a prime would make that prime's test vacuous
            let mut g = Integer::zero();
            for (c, _) in &cs {
                g = g.gcd(c);
            }
            if !g.is_zero() && ((&g % &p).is_zero() || (&g % &p2).is_zero()) {
                content_hits.push(json!({"rule": r.pos, "literal": m.text}));
            }
            if bound(&cs, r, &t.mask, rclaim) >= lim {
                unsafe_at_claim.push(json!({"rule": r.pos, "label": r.label, "source": r.origin["source"], "kind": kind,
                    "literal": crate::coef::short(&m.text)}));
            }
            // largest safe radius: binary search on [0, 2^40]
            let (mut lo, mut hi) = (0i64, 1i64 << 40);
            if bound(&cs, r, &t.mask, hi) < lim {
                lo = hi;
            } else if bound(&cs, r, &t.mask, 0) >= lim {
                hi = -1;
                lo = -1;
            } else {
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    if bound(&cs, r, &t.mask, mid) < lim {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
            }
            let _ = hi;
            if lo < rule_min {
                rule_min = lo;
            }
            if lo < global_min {
                global_min = lo;
                binding = Some(
                    json!({"rule": r.pos, "label": r.label, "source": r.origin["source"], "kind": kind,
                    "literal": crate::coef::short(&m.text), "safe_radius": lo}),
                );
            }
        }
        if rule_min < (1i64 << 40) {
            per_rule.push((
                rule_min,
                r.pos,
                r.origin["source"].as_str().unwrap_or("").to_string(),
            ));
        }
    }
    per_rule.sort();
    let res = json!({
        "schema": "rr_pverify.radius.v1", "table": tp, "claimed_radius": rclaim, "literals": literals,
        "P_times_P2": lim.to_string(),
        "literals_unsafe_at_claimed_radius": unsafe_at_claim,
        "verifier_safe_radius": global_min, "binding_literal": binding,
        "rules_with_finite_safe_radius": per_rule.len(),
        "smallest_safe_radii": per_rule.iter().take(30).map(|(x, p, s)| json!({"rule": p, "source": s, "safe_radius": x})).collect::<Vec<_>>(),
        "content_divisible_by_a_prime": content_hits, "errors": errors,
        "verdict": if unsafe_at_claim.is_empty() && errors.is_empty() && content_hits.is_empty() { "YES" } else { "NO" },
    });
    let s = serde_json::to_string_pretty(&res).unwrap();
    if let Some(p) = out {
        std::fs::write(&p, &s).unwrap();
    }
    println!("{s}");
}
