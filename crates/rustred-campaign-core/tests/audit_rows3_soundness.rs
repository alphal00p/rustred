//! Independent audit `audit-rows3` (WS/notes/implementer.reaudit-rows3.md): soundness attacks on the `replay` evidence
//! level of the identity test (ibp_rows + replay_base + independent_algebra). Every test prints its numbers; the
//! assertions encode what the audit found (a test asserting a HOLE is named `..._hole` and documents the defect: it
//! fails once the defect is fixed, and must then be inverted).
//!
//! Family: LA3_handwritten.sym (3 loops, 9 slots, chart 111111100), the TRUE rule = one IBP row solved for its top term
//! (the audit-A construction of tests/rows_common/face.rs, rebuilt here for any `Family` object).

mod rows_common;

use std::cell::Cell;
use std::time::Instant;

use rows_common::face::{face, form_text, gcd, pat, served};
use rows_common::*;
use rustred_campaign_core::ibp_rows::{
    NonceSource, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
    Verdict, identity_test, replay_evidence, replay_evidence_with, verify_record,
};
use rustred_campaign_core::independent_algebra::{Independent, elim};
use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, Q, QResult, Span, Unknown, ZPoly,
};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::{Value, json};

// ------------------------------------------------------------------ helpers -----------------------------------------

/// The TRUE rule of IBP operator `op` of `fam` (solved for the term with the largest shift), chart 111111100.
fn row_rule(fam: &Family, op: usize) -> (Value, Vec<i64>) {
    let n = fam.n();
    let row = fam.exact_row(op).unwrap();
    let (tshift, tform) = row
        .iter()
        .filter(|(_, f)| f[1..=n].iter().any(|q| !q.is_zero()))
        .max_by_key(|(s, _)| s.iter().sum::<i64>())
        .map(|(s, f)| (s.clone(), f.clone()))
        .unwrap();
    let mut mm: i128 = 1;
    for f in row.values() {
        for q in f {
            mm = mm / gcd(mm, q.d) * q.d;
        }
    }
    let den = form_text(&tform, mm, n);
    let rhs: Vec<Value> = row
        .iter()
        .filter(|(sh, _)| **sh != tshift)
        .map(|(sh, f)| {
            let neg: Vec<Q> = f.iter().map(|q| Independent.q_neg(*q).unwrap()).collect();
            json!([pat(sh), format!("({})/({den})", form_text(&neg, mm, n))])
        })
        .collect();
    let chart: String = (0..n).map(|i| if i < 7 { '1' } else { '0' }).collect();
    (
        json!({
            "type": "rule", "index": 0, "chart": chart, "fixed": vec![Value::Null; n],
            "equations": [], "exceptions": [], "target": pat(&tshift), "rhs": rhs,
            "seeds": [pat(&vec![0; n])], "recenter": vec![0; n], "witness_loci": [], "guard_version": 2,
        }),
        tshift,
    )
}

/// Applicable keys of the chart-111111100 rule `rule` with target shift `tshift`: line values 1..=4, numerators -1/-2,
/// skipping keys where one of the rule's own denominators vanishes identically in d (its true poles).
fn keys_grid(rule: &Value, tshift: &[i64], count: usize) -> Vec<Vec<i64>> {
    let dens: Vec<String> = rule["rhs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t[1].as_str().unwrap().to_string())
        .collect();
    let p = rustred_campaign_core::replay_base::primes::verify_primes(1, &[])[0];
    let mut out = Vec::new();
    'all: for num in [-1i64, -2] {
        for m in 0..4u32.pow(7) {
            let mut key = vec![0i64; 9];
            let mut r = m;
            for k in key.iter_mut().take(7) {
                *k = 1 + (r % 4) as i64;
                r /= 4;
            }
            key[7] = num;
            key[8] = -1;
            // skip the rule's own poles (denominator zero at two d values)
            let pt: Vec<u64> = key
                .iter()
                .zip(tshift)
                .map(|(a, b)| Independent.from_i128(p, (a - b) as i128))
                .collect();
            let ctxs: Vec<Ctx> = [3u64, 5]
                .iter()
                .map(|&d| Ctx {
                    p,
                    vals: pt.iter().copied().chain([d]).collect(),
                })
                .collect();
            let pole = dens.iter().any(|t| {
                let (mut nn, mut dd) = ([0u64; 2], [0u64; 2]);
                Independent
                    .eval_ratio(t, 9, &ctxs, &mut nn, &mut dd)
                    .unwrap();
                dd == [0, 0]
            });
            if pole {
                continue;
            }
            out.push(key);
            if out.len() == count {
                break 'all;
            }
        }
    }
    out
}

fn with_term(rule: &Value, key: &str, coef: &str) -> Value {
    let mut r = rule.clone();
    r["rhs"].as_array_mut().unwrap().push(json!([key, coef]));
    r
}

fn level_kind(l: &ReplayLevel) -> String {
    match l {
        ReplayLevel::Established => "Established".into(),
        ReplayLevel::NotEstablished { reasons } => format!(
            "NotEstablished({})",
            reasons.first().cloned().unwrap_or_default()
        ),
        ReplayLevel::Overridden {
            level_if_default, ..
        } => format!("Overridden(if default: {})", level_kind(level_if_default)),
        ReplayLevel::Diagnostic {
            level_if_certificate,
            ..
        } => format!("Diagnostic({})", level_kind(level_if_certificate)),
        ReplayLevel::Unbound { .. } => "Unbound".into(),
        ReplayLevel::ReplaySampled { .. } => "ReplaySampled".into(),
    }
}

/// The corner of the chart (all lines 1, numerators 0): a nonzero integral never in the span of IBP rows of one seed.
const CORNER: &str = "I(1,1,1,1,1,1,1,0,0)";

// ------------------------------------------------------------------ a delegating backend that can lie --------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lie {
    /// delegate everything, but report the certificate backend's name (a diagnostic backend claiming "independent")
    NameOnly,
    /// the first span test of every elimination (the rule vector) answers "in the span"
    FirstSolveInSpan,
    /// the inverse of the scalar-product matrix (the family's operator table) is perturbed: entry [0][0] + 1
    FamilyInverse,
}

struct Liar(Lie);

struct LyingElim<'r> {
    inner: elim::Elim<'r>,
    lie: bool,
    calls: Cell<usize>,
}

impl Eliminator for LyingElim<'_> {
    fn rank(&self) -> usize {
        self.inner.rank()
    }
    fn entries(&self) -> usize {
        self.inner.entries()
    }
    fn pivot_columns(&self) -> Vec<usize> {
        self.inner.pivot_columns()
    }
    fn solve(&self, desired: &[(usize, u64)]) -> Result<Span, Unknown> {
        let k = self.calls.get();
        self.calls.set(k + 1);
        let mut s = self.inner.solve(desired)?;
        if self.lie && k == 0 {
            s.in_span = true;
            s.remainder.clear();
        }
        Ok(s)
    }
}

impl CheckAlgebra for Liar {
    type Literal = <Independent as CheckAlgebra>::Literal;
    type Elim<'r> = LyingElim<'r>;
    fn name(&self) -> &'static str {
        "independent"
    }
    fn is_prime(&self, n: u64) -> bool {
        Independent.is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        Independent.add(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        Independent.sub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        Independent.mul(p, a, b)
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        Independent.neg(p, a)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        Independent.inv(p, a)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        Independent.from_i128(p, x)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        Independent.q_new(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        Independent.q_add(a, b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        Independent.q_sub(a, b)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        Independent.q_mul(a, b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        Independent.q_div(a, b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        Independent.q_neg(a)
    }
    fn q_parse(&self, s: &str) -> QResult {
        Independent.q_parse(s)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        Independent.q_modp(a, p)
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        let mut inv = Independent.q_inverse(m)?;
        if self.0 == Lie::FamilyInverse {
            inv[0][0] = Independent.q_add(inv[0][0], Q::ONE)?;
        }
        Ok(inv)
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        Independent.q_rank(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        Independent.det_i128(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        Independent.q_rref(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        Independent.q_unit_in_row_space(rows, col)
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        Independent.gcd_i128(a, b)
    }
    fn int_mat_mul(
        &self,
        a: &[i128],
        b: &[i128],
        r: usize,
        k: usize,
        c: usize,
    ) -> Option<Vec<i128>> {
        Independent.int_mat_mul(a, b, r, k, c)
    }
    fn int_div_exact(&self, a: i128, b: i128) -> Option<i128> {
        Independent.int_div_exact(a, b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        Independent.zpoly_det(m, dim, t)
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        Independent.eval_ratio(text, arity, ctxs, num, den)
    }
    fn cross_check_coefficient(
        &self,
        text: &str,
        arity: usize,
        ctx: &Ctx,
        num: u64,
        den: u64,
    ) -> Result<(), Unknown> {
        Independent.cross_check_coefficient(text, arity, ctx, num, den)
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        Independent.expr_linear(text, n)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        Independent.expr_has_d(text, n)
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown> {
        Independent.literal_parse(text, n)
    }
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown> {
        Independent.literal_is_zero_at(lit, point)
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        Independent.literal_same_up_to_constant(a, b, n)
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<LyingElim<'r>, Unknown> {
        Ok(LyingElim {
            inner: Independent.eliminate(rows, ncols, p, max_entries)?,
            lie: self.0 == Lie::FirstSolveInSpan,
            calls: Cell::new(0),
        })
    }
}

// ------------------------------------------------------------------ attacks ---------------------------------------

/// A rule wrong at exactly ONE served key (a boundary face n_j = 0 hit by one key of 401; generic points pass):
/// never Established over 20 master seeds (fresh OS nonce each run), and the record names that key.
#[test]
fn audit_rows3_single_bad_key_never_established() {
    let f = face();
    let n = f.fam.n();
    let bad = RowsRule::from_value(f.bad.clone(), n).unwrap();
    let all = served(&f);
    let face_keys: Vec<&Vec<i64>> = all.iter().filter(|k| k[f.j] == 0).collect();
    let mut keys: Vec<Vec<i64>> = all.iter().filter(|k| k[f.j] != 0).cloned().collect();
    keys.push(face_keys[0].clone());
    let g = identity_test(
        &f.fam,
        &bad,
        &[PointSpec::Generic, PointSpec::Generic],
        &RowsOptions::default(),
    );
    assert_eq!(
        g.verdict,
        Verdict::Pass,
        "generic points cannot see the face"
    );
    let mut est = 0;
    for seed in 0..20u64 {
        let r = replay_evidence(
            &f.fam,
            &bad,
            &keys,
            &ReplayPolicy::default(),
            // rows4: a fresh campaign beacon per trial (a non-default master seed is an override)
            &RowsOptions {
                beacon: Some(test_beacon(seed)),
                ..bound(&f.fam)
            },
        );
        if r.level == ReplayLevel::Established {
            est += 1;
        }
        let failed: Vec<&PointSpec> = r
            .identity
            .points
            .iter()
            .filter(|p| p.status != rustred_campaign_core::ibp_rows::PointStatus::Pass)
            .map(|p| &p.spec)
            .collect();
        assert_eq!(failed, vec![&PointSpec::Key(face_keys[0].clone())]);
    }
    println!(
        "single bad key: {} served ({} on the face), Established in {est}/20",
        keys.len(),
        1
    );
    assert_eq!(est, 0);
}

/// Pole scan coverage on a 10,000-key served domain: a TRUE rule plus a term `(0)/(sum (n_i - v_i)^2)` whose
/// denominator vanishes at exactly ONE served key; the scan must find exactly that key; the clean rule is Established.
#[test]
fn audit_rows3_pole_scan_10k_keys() {
    let fam = rows_common::face::face().fam;
    let (good, tshift) = row_rule(&fam, 0);
    let keys = keys_grid(&good, &tshift, 10_000);
    let n = fam.n();
    let good_r = RowsRule::from_value(good.clone(), n).unwrap();
    let t0 = Instant::now();
    let rg = replay_evidence(&fam, &good_r, &keys, &ReplayPolicy::default(), &bound(&fam));
    let t_good = t0.elapsed().as_secs_f64();
    println!(
        "10k clean: {} applicable {} poles {} replayed {} in {t_good:.1}s",
        level_kind(&rg.level),
        rg.pole_scan.keys_applicable,
        rg.pole_scan.poles.len(),
        rg.served_keys_replayed
    );
    assert_eq!(rg.level, ReplayLevel::Established);
    assert_eq!(rg.pole_scan.keys_applicable, 10_000);
    // the pole key: the 7,777th key; its rule point v = key - tshift
    let pk = keys[7_777].clone();
    let v: Vec<i64> = pk.iter().zip(&tshift).map(|(a, b)| a - b).collect();
    let mut den: Vec<String> = Vec::new();
    let mut c0 = 0i64;
    for (i, &vi) in v.iter().enumerate().take(7) {
        den.push(format!("1*n{i}^2"));
        if vi != 0 {
            den.push(format!("{}*n{i}", -2 * vi));
        }
        c0 += vi * vi;
    }
    den.push(c0.to_string());
    let text = format!("(0)/({})", den.join("+").replace("+-", "-"));
    let mut zero_key = tshift.clone();
    zero_key[0] -= 2;
    let polar = with_term(&good, &pat(&zero_key), &text);
    let pr = RowsRule::from_value(polar, n).unwrap();
    let t1 = Instant::now();
    let rp = replay_evidence(&fam, &pr, &keys, &ReplayPolicy::default(), &bound(&fam));
    let t_pole = t1.elapsed().as_secs_f64();
    let hits: Vec<&Vec<i64>> = rp.pole_scan.poles.iter().map(|h| &h.key).collect();
    println!(
        "10k one pole: {} poles {:?} (expected {:?}) undecided {} in {t_pole:.1}s",
        level_kind(&rp.level),
        hits,
        pk,
        rp.pole_scan.undecided.len()
    );
    assert_eq!(hits, vec![&pk]);
    assert!(matches!(rp.level, ReplayLevel::NotEstablished { .. }));
}

/// The family binding: no binding / a header without the family text = never Established (Unbound); a binding to
/// another text = Unsupported(Family); every override combined with Unbound stays non-Established.
#[test]
fn audit_rows3_unbound_never_certifies() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let keys = served(&f);
    let pol = ReplayPolicy::default();
    let none = replay_evidence(&f.fam, &good, &keys, &pol, &RowsOptions::default());
    let hdr = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            table: TableBinding::from_header(
                &json!({"type": "header", "loops": 3, "denominators": 9}),
            ),
            ..RowsOptions::default()
        },
    );
    let other = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            table: Some(TableBinding::from_family_digest(&"0".repeat(64))),
            ..RowsOptions::default()
        },
    );
    let both = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            nonce: Some(7),
            ..RowsOptions::default()
        },
    );
    for (what, r) in [
        ("none", &none),
        ("header", &hdr),
        ("other", &other),
        ("nonce+none", &both),
    ] {
        println!("unbound {what}: {}", level_kind(&r.level));
        assert_ne!(r.level, ReplayLevel::Established, "{what}");
    }
    assert!(matches!(none.level, ReplayLevel::Unbound { .. }));
    assert!(matches!(hdr.level, ReplayLevel::Unbound { .. }));
    assert!(matches!(other.identity.verdict, Verdict::Unsupported(_)));
}

/// Errors only at special dimension values / only in a high-degree term / only at the engine's sample dimensions
/// d = 37/7 and d = -11/5: each false copy (true rule + c(d) * corner) is refused at every served key; the
/// Schwartz-Zippel degree recorded covers the d-degree of the perturbation.
#[test]
fn audit_rows3_dimension_and_degree_perturbations() {
    let f = face();
    let n = f.fam.n();
    let keys: Vec<Vec<i64>> = served(&f)
        .into_iter()
        .filter(|k| k[f.j] != 0)
        .take(60)
        .collect();
    let cases = [
        (
            "vanishes at d=37/7 and d=-11/5 (the engine's sample dimensions)",
            "35*d^2-108*d-407",
        ),
        ("vanishes at d=4", "1*d-4"),
        ("degree 20000 in d", "1*d^10000*d^10000"),
        ("d^10000 - d^9999", "1*d^10000-1*d^9999"),
        (
            "huge integer coefficient times (d-4)",
            "123456789012345678901234567890123456789*d-493827156049382715604938271560493827156",
        ),
    ];
    for (what, c) in cases {
        let bad = RowsRule::from_value(with_term(&f.good, CORNER, c), n).unwrap();
        let r = replay_evidence(
            &f.fam,
            &bad,
            &keys,
            &ReplayPolicy::default(),
            &bound(&f.fam),
        );
        let failed_keys = r
            .identity
            .points
            .iter()
            .filter(|p| matches!(p.spec, PointSpec::Key(_)))
            .filter(|p| p.status == rustred_campaign_core::ibp_rows::PointStatus::Fail)
            .count();
        let szmax = r
            .identity
            .points
            .iter()
            .flat_map(|p| &p.cells)
            .map(|c| c.sz_degree)
            .max()
            .unwrap_or(0);
        println!(
            "{what}: {} ; identity {} ; failed keys {failed_keys}/{} ; max sz degree {szmax}",
            level_kind(&r.level),
            r.identity.verdict.kind(),
            keys.len()
        );
        assert!(
            matches!(r.level, ReplayLevel::NotEstablished { .. }),
            "{what}"
        );
        assert_eq!(
            failed_keys,
            keys.len(),
            "{what}: every served key must fail"
        );
    }
    // the exponent limit: 10001 is refused, never truncated
    let over = RowsRule::from_value(with_term(&f.good, CORNER, "1*d^10001"), n).unwrap();
    let r = identity_test(
        &f.fam,
        &over,
        &[PointSpec::Generic],
        &RowsOptions::default(),
    );
    println!("exponent 10001: {:?}", r.verdict);
    assert!(matches!(r.verdict, Verdict::Unsupported(_)));
}

/// CLOSED by rows4 (was the HOLE `audit_rows3_family_parsed_by_foreign_backend_hole`, major RA3-M1): a `Family` parsed
/// with ANY other `CheckAlgebra` (here one whose matrix inverse is perturbed) has the same text digests as the honest
/// parse but a DIAGNOSTIC origin and another operator-table digest: the certificate run refuses it
/// (`Unsupported(Family)`, never Established) and `verify_record` given that family object refuses the record.
#[test]
fn audit_rows3_family_parsed_by_foreign_backend_closed() {
    let src = data("LA3_handwritten.sym");
    let honest = Family::parse(&src).unwrap();
    let forged = Family::parse_with(&Liar(Lie::FamilyInverse), &src).unwrap();
    assert_eq!(honest.source_digest(), forged.source_digest());
    assert_eq!(honest.squashed_digest(), forged.squashed_digest());
    assert!(honest.origin().is_certificate());
    assert!(!forged.origin().is_certificate());
    assert_ne!(honest.table_digest(), forged.table_digest());
    let n = honest.n();
    let (rule_v, tshift) = row_rule(&forged, 0);
    let rule = RowsRule::from_value(rule_v.clone(), n).unwrap();
    let keys: Vec<Vec<i64>> = keys_grid(&rule_v, &tshift, 300);
    let on_forged = replay_evidence(
        &forged,
        &rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&forged),
    );
    let on_honest = replay_evidence(
        &honest,
        &rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&honest),
    );
    let v_forged = verify_record(&on_forged, &rule, &forged, &keys);
    let v_honest = verify_record(&on_forged, &rule, &honest, &keys);
    println!(
        "foreign-backend family: certificate run on it: {} / identity {:?}; on the honest parse: {} / identity {}; \
         verify_record(forged family) ok={}; verify_record(honest family) ok={}",
        level_kind(&on_forged.level),
        on_forged.identity.verdict,
        level_kind(&on_honest.level),
        on_honest.identity.verdict.kind(),
        v_forged.is_ok(),
        v_honest.is_ok()
    );
    assert!(matches!(
        on_forged.level,
        ReplayLevel::NotEstablished { .. }
    ));
    assert!(matches!(
        on_forged.identity.verdict,
        Verdict::Unsupported(rustred_campaign_core::ibp_rows::Unsupported::Family(_))
    ));
    assert!(!on_forged.certificate);
    assert!(matches!(on_honest.identity.verdict, Verdict::Fail { .. }));
    assert!(v_forged.is_err());
    assert!(v_honest.is_err());
    // a record made honestly verifies only with the certificate parse: the forged object is refused by its table
    let good = replay_evidence(
        &honest,
        &rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&honest),
    );
    assert!(verify_record(&good, &rule, &honest, &keys).is_ok());
    assert!(verify_record(&good, &rule, &forged, &keys).is_err());
}

/// CLOSED by rows4 (was the HOLE `audit_rows3_backend_name_gate_hole`, minor RA3-m1): the level is no longer gated on
/// the STRING `CheckAlgebra::name()`: every generic `replay_evidence_with` run is DIAGNOSTIC (`certificate: false`,
/// `ReplayLevel::Diagnostic`), here for a backend that answers "independent" and lies on the span test.
#[test]
fn audit_rows3_backend_name_gate_closed() {
    let f = face();
    let n = f.fam.n();
    let keys = served(&f);
    let false_rule = RowsRule::from_value(with_term(&f.good, CORNER, "1"), n).unwrap();
    let honest = replay_evidence(
        &f.fam,
        &false_rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    let lying = replay_evidence_with(
        &Liar(Lie::FirstSolveInSpan),
        &f.fam,
        &false_rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    let named = replay_evidence_with(
        &Liar(Lie::NameOnly),
        &f.fam,
        &false_rule,
        &keys,
        &ReplayPolicy::default(),
        &bound(&f.fam),
    );
    let ver = verify_record(&lying, &false_rule, &f.fam, &keys);
    println!(
        "name gate: honest {} ; lying backend {} (algebra {:?}, certificate {}) ; delegating backend {} ; verify_record(lying) ok={}",
        level_kind(&honest.level),
        level_kind(&lying.level),
        lying.algebra,
        lying.certificate,
        level_kind(&named.level),
        ver.is_ok()
    );
    assert!(matches!(honest.level, ReplayLevel::NotEstablished { .. }));
    assert!(matches!(lying.level, ReplayLevel::Diagnostic { .. }));
    assert!(matches!(named.level, ReplayLevel::Diagnostic { .. }));
    assert!(!lying.certificate && !named.certificate);
    assert!(ver.is_err());
}

/// Forged records against `verify_record`: (1) a NotEstablished record relabelled Established, (2) a record made on a
/// subset of the served keys presented for the full set, (3) a caller-nonce (Overridden) record relabelled as an OS
/// nonce, (4) the same record relabelled as a DERIVED nonce with a beacon. All four are rejected (rows4, RA3-m2: the
/// certificate nonce is derived from the rule digest and the recorded beacon, so a relabelled caller nonce no longer
/// verifies; before rows4 (3) and (4) verified).
#[test]
fn audit_rows3_forged_records() {
    let f = face();
    let n = f.fam.n();
    let keys = served(&f);
    let bad = RowsRule::from_value(f.bad.clone(), n).unwrap();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let pol = ReplayPolicy::default();
    // (1)
    let mut r1 = replay_evidence(&f.fam, &bad, &keys, &pol, &bound(&f.fam));
    assert!(matches!(r1.level, ReplayLevel::NotEstablished { .. }));
    r1.level = ReplayLevel::Established;
    let v1 = verify_record(&r1, &bad, &f.fam, &keys);
    // (2) the bad rule on the keys off the face: legitimately Established there; presented for all keys
    let off: Vec<Vec<i64>> = keys.iter().filter(|k| k[f.j] != 0).cloned().collect();
    let r2 = replay_evidence(&f.fam, &bad, &off, &pol, &bound(&f.fam));
    assert_eq!(
        r2.level,
        ReplayLevel::Established,
        "the bad rule is right off the face"
    );
    let v2 = verify_record(&r2, &bad, &f.fam, &keys);
    // (3)
    let r3 = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            nonce: Some(42),
            ..bound(&f.fam)
        },
    );
    assert!(matches!(r3.level, ReplayLevel::Overridden { .. }));
    let mut r3os = r3.clone();
    r3os.nonce_source = NonceSource::Os;
    r3os.overrides.clear();
    r3os.level = ReplayLevel::Established;
    let v3 = verify_record(&r3os, &good, &f.fam, &keys);
    // (4)
    let mut r4 = r3.clone();
    r4.nonce_source = NonceSource::Derived;
    r4.overrides.clear();
    r4.level = ReplayLevel::Established;
    let v4 = verify_record(&r4, &good, &f.fam, &keys);
    // the honest record verifies
    let r5 = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    let v5 = verify_record(&r5, &good, &f.fam, &keys);
    println!(
        "forged records: (1) relabelled level ok={} (2) subset record for the full set ok={} (3) caller nonce relabelled Os ok={} (4) caller nonce relabelled Derived ok={}; honest {} ok={} (nonce {:?})",
        v1.is_ok(),
        v2.is_ok(),
        v3.is_ok(),
        v4.is_ok(),
        level_kind(&r5.level),
        v5.is_ok(),
        r5.nonce_source
    );
    assert!(v1.is_err());
    assert!(v2.is_err());
    assert!(v3.is_err());
    assert!(v4.is_err());
    assert_eq!(r5.level, ReplayLevel::Established);
    assert_eq!(r5.nonce_source, NonceSource::Derived);
    assert!(v5.is_ok(), "{v5:?}");
}
