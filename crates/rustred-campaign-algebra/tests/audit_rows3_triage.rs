//! Independent audit `audit-rows3` (WS/notes/implementer.reaudit-rows3.md): does the `Both` triage tell apart
//! "Symbolica wrong" / "our code wrong" / "rule bad"? One defect is planted per stage on EITHER side through a delegating
//! wrapper (independent of the crate's own `wrap::Planted`), and several kinds of bad rules are run clean. Every run
//! prints one table row: case | side | expected | triage class | stage | outlier. SKIPs without the Symbolica licence.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use rustred_campaign_algebra::modes::Traced;
use rustred_campaign_algebra::wrap::{Event, recording};
use rustred_campaign_algebra::{
    Outlier, SymbolicaAlgebra, Triage, replay_evidence_triage, triage_identity,
};
use rustred_campaign_core::ibp_rows::{
    NonceSource, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Eliminator, Q, QResult, Span, Stage, Unknown, ZPoly,
};
use rustred_campaign_core::replay_base::family::Family;
use serde_json::{Value, json};

// ------------------------------------------------------------------ the LA3 face rules (audit-A construction) --------

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn form_text(form: &[Q], m: i128, n: usize) -> String {
    let mut terms = Vec::new();
    for (j, q) in form.iter().enumerate() {
        if q.is_zero() {
            continue;
        }
        let c = q.n * (m / q.d);
        let var = match j {
            0 => String::new(),
            j if j == n + 1 => "d".into(),
            j => format!("n{}", j - 1),
        };
        terms.push(if var.is_empty() {
            format!("{c}")
        } else {
            format!("{c}*{var}")
        });
    }
    let s = terms.join("+").replace("+-", "-");
    if s.is_empty() { "0".into() } else { s }
}

fn pat(shift: &[i64]) -> String {
    let parts: Vec<String> = shift
        .iter()
        .enumerate()
        .map(|(i, &o)| match o {
            0 => format!("n{i}"),
            o if o > 0 => format!("n{i}+{o}"),
            o => format!("n{i}{o}"),
        })
        .collect();
    format!("I({})", parts.join(","))
}

struct Face {
    text: String,
    good: Value,
    bad: Value,
    j: usize,
    tshift: Vec<i64>,
    den: String,
}

fn face() -> Face {
    let text = core_data("LA3_handwritten.sym");
    let fam = Family::parse(&text).unwrap();
    let n = fam.n();
    let mut found = None;
    'o: for m in 1u32..(1 << 7) {
        let s: Vec<bool> = (0..n).map(|i| i < 7 && m >> i & 1 == 1).collect();
        if !fam.zero_mask(&s) {
            continue;
        }
        for j in [7usize, 8] {
            let mut t = s.clone();
            t[j] = true;
            if !fam.zero_mask(&t) {
                found = Some((s, j));
                break 'o;
            }
        }
    }
    let (s, j) = found.unwrap();
    let row = fam.exact_row(0).unwrap();
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
    let mut rhs: Vec<Value> = row
        .iter()
        .filter(|(sh, _)| **sh != tshift)
        .map(|(sh, f)| {
            let neg: Vec<Q> = f.iter().map(|q| Independent.q_neg(*q).unwrap()).collect();
            json!([pat(sh), format!("({})/({den})", form_text(&neg, mm, n))])
        })
        .collect();
    let chart: String = (0..n).map(|i| if i < 7 { '1' } else { '0' }).collect();
    let good = json!({
        "type": "rule", "index": 0, "chart": chart, "fixed": vec![Value::Null; n],
        "equations": [], "exceptions": [], "target": pat(&tshift), "rhs": rhs.clone(),
        "seeds": [pat(&vec![0; n])], "recenter": vec![0; n], "witness_loci": [], "guard_version": 2,
    });
    let zparts: Vec<String> = (0..n)
        .map(|i| {
            if i < 7 && !s[i] {
                "0".into()
            } else {
                let o = tshift[i] + i64::from(i == j);
                match o {
                    0 => format!("n{i}"),
                    o if o > 0 => format!("n{i}+{o}"),
                    o => format!("n{i}{o}"),
                }
            }
        })
        .collect();
    rhs.push(json!([format!("I({})", zparts.join(",")), "1"]));
    let mut bad = good.clone();
    bad["rhs"] = json!(rhs);
    Face {
        text,
        good,
        bad,
        j,
        tshift,
        den,
    }
}

fn key_off_face(f: &Face) -> Vec<i64> {
    let pt = [2i64, 3, 2, 4, 3, 2, 3, -1, -2];
    pt.iter().zip(&f.tshift).map(|(a, b)| a + b).collect()
}

fn key_on_face(f: &Face) -> Vec<i64> {
    let mut pt = vec![2i64, 3, 2, 4, 3, 2, 3, -1, -2];
    pt[f.j] = -f.tshift[f.j];
    pt.iter().zip(&f.tshift).map(|(a, b)| a + b).collect()
}

fn with_rhs(rule: &Value, f: impl FnOnce(&mut Vec<Value>)) -> Value {
    let mut r = rule.clone();
    f(r["rhs"].as_array_mut().unwrap());
    r
}

// ------------------------------------------------------------------ one planted defect per stage -------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Defect {
    None,
    /// the inverse of the scalar-product matrix (family operator table): entry [0][0] + 1
    FamilyInverse,
    /// primality answers false for primes p with bit 1 set (the prime selection moves)
    Primality,
    /// the numerator of the `nth` coefficient evaluation (first context) + 1
    Coefficient(usize),
    /// the `nth` field multiplication with both operands > 1 returns the product + 1
    Mul(usize),
    /// the `nth` Lee affine-hull test answers the opposite (a column wrongly kept / dropped)
    Lee(usize),
    /// the `nth` elimination drops its first non-empty row
    DropRow(usize),
    /// the +1 mutant span test (2nd solve of an elimination) answers "in the span"
    MutantInSpan,
    /// the rule-vector span test (1st solve of an elimination) answers "in the span"
    RuleInSpan,
    /// the `nth` case-literal zero test answers the opposite (applicability; not a traced stage)
    Literal(usize),
}

struct Bent<A: CheckAlgebra> {
    inner: A,
    defect: Defect,
    calls: AtomicUsize,
    /// how many times the defect actually CHANGED a value
    effects: Arc<AtomicUsize>,
    events: Mutex<Vec<Event>>,
}

fn bent<A: CheckAlgebra>(inner: A, defect: Defect) -> Bent<A> {
    Bent {
        inner,
        defect,
        calls: AtomicUsize::new(0),
        effects: Arc::new(AtomicUsize::new(0)),
        events: Mutex::new(Vec::new()),
    }
}

impl<A: CheckAlgebra> Bent<A> {
    fn hit(&self, nth: usize) -> bool {
        let h = self.calls.fetch_add(1, Ordering::SeqCst) == nth;
        if h {
            self.effects.fetch_add(1, Ordering::SeqCst);
        }
        h
    }
}

struct BentElim<E: Eliminator> {
    inner: E,
    defect: Defect,
    solves: AtomicUsize,
    effects: Arc<AtomicUsize>,
}

impl<E: Eliminator> Eliminator for BentElim<E> {
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
        let k = self.solves.fetch_add(1, Ordering::SeqCst);
        let mut s = self.inner.solve(desired)?;
        let lie = matches!(
            (self.defect, k),
            (Defect::RuleInSpan, 0) | (Defect::MutantInSpan, 1)
        );
        if lie {
            if !s.in_span {
                self.effects.fetch_add(1, Ordering::SeqCst);
            }
            s.in_span = true;
            s.remainder.clear();
        }
        Ok(s)
    }
}

impl<A: CheckAlgebra> Traced for Bent<A> {
    fn take_events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl<A: CheckAlgebra> CheckAlgebra for Bent<A> {
    type Literal = A::Literal;
    type Elim<'r>
        = BentElim<A::Elim<'r>>
    where
        Self: 'r;
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn is_prime(&self, n: u64) -> bool {
        let v = self.inner.is_prime(n);
        if self.defect == Defect::Primality && v && (n >> 1) & 1 == 1 {
            self.effects.fetch_add(1, Ordering::SeqCst);
            false
        } else {
            v
        }
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        self.inner.add(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        self.inner.sub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        let v = self.inner.mul(p, a, b);
        match self.defect {
            Defect::Mul(nth) if a > 1 && b > 1 && self.hit(nth) => self.inner.add(p, v, 1),
            _ => v,
        }
    }
    fn neg(&self, p: u64, a: u64) -> u64 {
        self.inner.neg(p, a)
    }
    fn inv(&self, p: u64, a: u64) -> Option<u64> {
        self.inner.inv(p, a)
    }
    fn from_i128(&self, p: u64, x: i128) -> u64 {
        self.inner.from_i128(p, x)
    }
    fn q_new(&self, n: i128, d: i128) -> QResult {
        self.inner.q_new(n, d)
    }
    fn q_add(&self, a: Q, b: Q) -> QResult {
        self.inner.q_add(a, b)
    }
    fn q_sub(&self, a: Q, b: Q) -> QResult {
        self.inner.q_sub(a, b)
    }
    fn q_mul(&self, a: Q, b: Q) -> QResult {
        self.inner.q_mul(a, b)
    }
    fn q_div(&self, a: Q, b: Q) -> QResult {
        self.inner.q_div(a, b)
    }
    fn q_neg(&self, a: Q) -> QResult {
        self.inner.q_neg(a)
    }
    fn q_parse(&self, s: &str) -> QResult {
        self.inner.q_parse(s)
    }
    fn q_modp(&self, a: Q, p: u64) -> Option<u64> {
        self.inner.q_modp(a, p)
    }
    fn q_inverse(&self, m: &[Vec<Q>]) -> Result<Vec<Vec<Q>>, Unknown> {
        let mut inv = self.inner.q_inverse(m)?;
        if self.defect == Defect::FamilyInverse {
            self.effects.fetch_add(1, Ordering::SeqCst);
            inv[0][0] = self.inner.q_add(inv[0][0], Q::ONE)?;
        }
        Ok(inv)
    }
    fn q_rank(&self, rows: &[Vec<Q>]) -> Result<usize, Unknown> {
        self.inner.q_rank(rows)
    }
    fn det_i128(&self, m: &[Vec<i128>]) -> Option<i128> {
        self.inner.det_i128(m)
    }
    fn q_rref(&self, rows: &[Vec<Q>], ncols: usize) -> Result<(Vec<Vec<Q>>, Vec<usize>), Unknown> {
        self.inner.q_rref(rows, ncols)
    }
    fn q_unit_in_row_space(&self, rows: &[Vec<Q>], col: usize) -> Result<bool, Unknown> {
        let v = self.inner.q_unit_in_row_space(rows, col)?;
        Ok(match self.defect {
            Defect::Lee(nth) if self.hit(nth) => !v,
            _ => v,
        })
    }
    fn gcd_i128(&self, a: i128, b: i128) -> i128 {
        self.inner.gcd_i128(a, b)
    }
    fn zpoly_det(&self, m: &[Vec<ZPoly>], dim: usize, t: usize) -> ZPoly {
        self.inner.zpoly_det(m, dim, t)
    }
    fn eval_ratio(
        &self,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        let st = self.inner.eval_ratio(text, arity, ctxs, num, den)?;
        if let Defect::Coefficient(nth) = self.defect
            && !ctxs.is_empty()
            && self.hit(nth)
        {
            num[0] = self.inner.add(ctxs[0].p, num[0], 1);
        }
        Ok(st)
    }
    fn cross_check_coefficient(
        &self,
        text: &str,
        arity: usize,
        ctx: &Ctx,
        num: u64,
        den: u64,
    ) -> Result<(), Unknown> {
        self.inner
            .cross_check_coefficient(text, arity, ctx, num, den)
    }
    fn expr_linear(&self, text: &str, n: usize) -> Result<Vec<Q>, Unknown> {
        self.inner.expr_linear(text, n)
    }
    fn expr_has_d(&self, text: &str, n: usize) -> Result<bool, Unknown> {
        self.inner.expr_has_d(text, n)
    }
    fn literal_parse(&self, text: &str, n: usize) -> Result<Self::Literal, Unknown> {
        self.inner.literal_parse(text, n)
    }
    fn literal_is_zero_at(&self, lit: &Self::Literal, point: &[i32]) -> Result<bool, Unknown> {
        let v = self.inner.literal_is_zero_at(lit, point)?;
        Ok(match self.defect {
            Defect::Literal(nth) if self.hit(nth) => !v,
            _ => v,
        })
    }
    fn literal_same_up_to_constant(&self, a: &str, b: &str, n: usize) -> bool {
        self.inner.literal_same_up_to_constant(a, b, n)
    }
    fn eliminate<'r>(
        &self,
        rows: &'r [Vec<(usize, u64)>],
        ncols: usize,
        p: u64,
        max_entries: usize,
    ) -> Result<BentElim<A::Elim<'r>>, Unknown> {
        let rows: &'r [Vec<(usize, u64)>] = match self.defect {
            Defect::DropRow(nth) if self.hit(nth) => {
                let mut copy = rows.to_vec();
                if let Some(i) = copy.iter().position(|r| !r.is_empty()) {
                    copy[i].clear();
                }
                Box::leak(copy.into_boxed_slice())
            }
            _ => rows,
        };
        Ok(BentElim {
            inner: self.inner.eliminate(rows, ncols, p, max_entries)?,
            defect: self.defect,
            solves: AtomicUsize::new(0),
            effects: self.effects.clone(),
        })
    }
    fn observe(&self, stage: Stage, at: &dyn Fn() -> String, value: &dyn Fn() -> String) {
        self.events.lock().unwrap().push(Event {
            stage,
            at: at(),
            value: value(),
        });
    }
}

// ------------------------------------------------------------------ the table ------------------------------------

fn class(t: &Triage) -> (String, String, String) {
    match t {
        Triage::AgreePass => ("AgreePass".into(), "-".into(), "-".into()),
        Triage::AgreeFail { .. } => ("AgreeFail".into(), "-".into(), "-".into()),
        Triage::AgreeOther { verdict } => (
            format!(
                "AgreeOther({})",
                verdict.split([' ', '(', '{']).next().unwrap_or("")
            ),
            "-".into(),
            "-".into(),
        ),
        Triage::Differ(d) => (
            "Differ".into(),
            format!("{:?}", d.stage),
            format!("{:?}", d.outlier),
        ),
        Triage::SymbolicaUnsupported { reason } => (
            format!(
                "SymbolicaUnsupported({})",
                reason.chars().take(40).collect::<String>()
            ),
            "-".into(),
            "-".into(),
        ),
    }
}

struct Row {
    case: String,
    side: &'static str,
    expected: &'static str,
    got: (String, String, String),
    ind: String,
    sym: String,
    fired: usize,
}

fn run(
    case: &str,
    side: &'static str,
    expected: &'static str,
    defect: Defect,
    text: &str,
    rule: &Value,
    points: &[PointSpec],
) -> Row {
    let r = RowsRule::from_value(rule.clone(), 9).unwrap();
    let opts = RowsOptions::default();
    let sym = SymbolicaAlgebra::new().unwrap();
    let (ind_d, sym_d) = match side {
        "independent" => (defect, Defect::None),
        "symbolica" => (Defect::None, defect),
        _ => (Defect::None, Defect::None),
    };
    let (bi, bs) = (bent(Independent, ind_d), bent(sym, sym_d));
    let (vi, vs, rep) = triage_identity(&bi, &bs, text, &r, points, &opts).unwrap();
    let fired = bi.effects.load(Ordering::SeqCst) + bs.effects.load(Ordering::SeqCst);
    Row {
        case: case.into(),
        side,
        expected,
        got: class(&rep.triage),
        ind: vi.verdict.kind().into(),
        sym: vs.verdict.kind().into(),
        fired,
    }
}

/// The whole table (printed; the assertions are the audit's verdicts, see the report).
#[test]
fn audit_rows3_triage_table() {
    if !licensed_or_skip("audit_rows3_triage_table") {
        return;
    }
    let f = face();
    let off = key_off_face(&f);
    let on = key_on_face(&f);
    let pts = vec![
        PointSpec::Generic,
        PointSpec::Generic,
        PointSpec::Key(off.clone()),
        PointSpec::Key(on.clone()),
    ];
    let corner = "I(1,1,1,1,1,1,1,0,0)";
    let mut rows: Vec<Row> = Vec::new();
    // (A) rules, no defect
    // coefficient 0 doubled: the same (key, coefficient) term listed twice (the rule vector accumulates per key)
    let doubled = with_rhs(&f.good, |r| {
        let t = r[0].clone();
        r.push(t);
    });
    let wrong_seed = {
        let mut r = f.good.clone();
        let mut s = vec![0i64; 9];
        s[0] = 3;
        r["seeds"] = json!([pat(&s)]);
        r
    };
    let mut zk = f.tshift.clone();
    zk[0] -= 2;
    // a removable 0/0 at every point: numerator and denominator both carry the factor (n0 - n0 + 1) = 1 ... use den*den
    let removable = with_rhs(&f.good, |r| {
        let (k, c) = (r[0][0].clone(), r[0][1].as_str().unwrap().to_string());
        // c = (N)/(D)  ->  (N*(n1-n1))/(D*(n1-n1)) is not expanded; use the off-face key value instead:
        // (N)/(D) with an extra additive term 0/(n0 - v0) whose denominator vanishes at the off-face key only
        r[0] = json!([k, c]);
        r.push(json!([
            pat(&zk),
            format!("(0)/(1*n0-{})", off[0] - f.tshift[0])
        ]));
    });
    let grammar = with_rhs(&f.good, |r| r.push(json!([pat(&zk), "1/2*n1-1/2*n1"])));
    let cases: Vec<(&str, &'static str, Value)> = vec![
        ("true rule", "AgreePass", f.good.clone()),
        (
            "false: + 1*corner",
            "AgreeFail",
            with_rhs(&f.good, |r| r.push(json!([corner, "1"]))),
        ),
        ("false on the face key only", "AgreeFail", f.bad.clone()),
        ("false: coefficient 0 doubled", "AgreeFail", doubled),
        (
            "TRUE rule, wrong recorded seed",
            "AgreeFail (rule is true!)",
            wrong_seed,
        ),
        (
            "true + 0/(n0-v0): removable 0/0 at a key",
            "pole one side",
            removable,
        ),
        ("true + `1/2*n1-1/2*n1` (grammar-refused)", "?", grammar),
    ];
    for (name, exp, rule) in &cases {
        rows.push(run(name, "-", exp, Defect::None, &f.text, rule, &pts));
    }
    // (B) one defect per stage, each side, on the TRUE rule and on the FALSE (+corner) rule
    let false_rule = with_rhs(&f.good, |r| r.push(json!([corner, "1"])));
    let defects: Vec<(&str, Defect)> = vec![
        ("Family: operator-table inverse", Defect::FamilyInverse),
        ("Primes: primality", Defect::Primality),
        ("Coefficient #0", Defect::Coefficient(0)),
        ("Coefficient #5", Defect::Coefficient(5)),
        ("Rows: mul #200", Defect::Mul(200)),
        ("Rows: mul #5000", Defect::Mul(5000)),
        ("Rows: mul #20000", Defect::Mul(20000)),
        ("Columns: Lee test #0", Defect::Lee(0)),
        ("Columns: Lee test #3", Defect::Lee(3)),
        ("Columns: Lee test #1", Defect::Lee(1)),
        ("Elimination: drop row #0", Defect::DropRow(0)),
        ("Membership: +1 mutant in span", Defect::MutantInSpan),
        ("Membership: rule vector in span", Defect::RuleInSpan),
        ("Applicability: literal #0 (no stage)", Defect::Literal(0)),
    ];
    // a rule with one never-holding exception branch (so case literals are evaluated) for the applicability defect
    let mut with_exc = f.good.clone();
    with_exc["exceptions"] = json!([["n0-100"]]);
    for side in ["independent", "symbolica"] {
        rows.push(run(
            "true rule + exception n0-100 | Applicability: literal #2",
            side,
            "Differ",
            Defect::Literal(2),
            &f.text,
            &with_exc,
            &pts,
        ));
    }
    for (rule_name, rule) in [("true", &f.good), ("false", &false_rule)] {
        for (dname, d) in &defects {
            for side in ["independent", "symbolica"] {
                let exp = if side == "independent" {
                    "Differ, outlier Independent"
                } else {
                    "Differ, outlier Symbolica"
                };
                rows.push(run(
                    &format!("{rule_name} rule | {dname}"),
                    side,
                    exp,
                    *d,
                    &f.text,
                    rule,
                    &pts,
                ));
            }
        }
    }
    println!(
        "\n| case | defect side | expected | triage | stage | outlier | ind verdict | sym verdict | defect effects |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.case, r.side, r.expected, r.got.0, r.got.1, r.got.2, r.ind, r.sym, r.fired
        );
    }
    let all_planted = rows.iter().filter(|r| r.side != "-").count();
    let planted: Vec<&Row> = rows
        .iter()
        .filter(|r| r.side != "-" && r.fired > 0)
        .collect();
    println!(
        "planted runs {all_planted}, of which the defect changed a value in {}",
        planted.len()
    );
    let right = planted
        .iter()
        .filter(|r| {
            r.got.0 == "Differ"
                && ((r.side == "independent" && r.got.2 == "Independent")
                    || (r.side == "symbolica" && r.got.2 == "Symbolica"))
        })
        .count();
    let unresolved = planted
        .iter()
        .filter(|r| r.got.0 == "Differ" && r.got.2 == "Unresolved")
        .count();
    let wrong = planted
        .iter()
        .filter(|r| {
            r.got.0 == "Differ"
                && ((r.side == "independent" && r.got.2 == "Symbolica")
                    || (r.side == "symbolica" && r.got.2 == "Independent"))
        })
        .count();
    let invisible = planted.iter().filter(|r| r.got.0 != "Differ").count();
    println!(
        "planted runs {}: right outlier {right}, Unresolved {unresolved}, WRONG outlier {wrong}, not seen as Differ {invisible}",
        planted.len()
    );
    // no planted defect may be reported as an agreement on a rule verdict that differs from the clean run
    for r in &planted {
        if r.got.0 != "Differ" {
            println!(
                "  not seen: {} [{}] -> {} (ind {}, sym {})",
                r.case, r.side, r.got.0, r.ind, r.sym
            );
        }
    }
    assert_eq!(wrong, 0, "a planted defect blamed on the wrong side");
}

/// In `Both` replay mode a defect that makes the INDEPENDENT side pass a false rule blocks the level (Differ at
/// Membership, outlier Independent); the same defect on the Symbolica side leaves the (correct) NotEstablished.
#[test]
fn audit_rows3_both_blocks_independent_false_pass() {
    if !licensed_or_skip("audit_rows3_both_blocks_independent_false_pass") {
        return;
    }
    let f = face();
    let fam = Family::parse(&f.text).unwrap();
    let corner = "I(1,1,1,1,1,1,1,0,0)";
    let false_rule =
        RowsRule::from_value(with_rhs(&f.good, |r| r.push(json!([corner, "1"]))), 9).unwrap();
    let keys = vec![key_off_face(&f)];
    let opts = RowsOptions {
        table: Some(TableBinding::from_family_digest(fam.squashed_digest())),
        ..RowsOptions::default()
    };
    let sym = SymbolicaAlgebra::new().unwrap();
    let m = replay_evidence_triage(
        &bent(Independent, Defect::RuleInSpan),
        &recording(sym),
        &f.text,
        &false_rule,
        &keys,
        &ReplayPolicy::default(),
        &opts,
        (12345, NonceSource::Os),
    )
    .unwrap();
    let ri = m.independent.unwrap();
    let t = m.triage.unwrap();
    println!(
        "Both replay, independent side lies: triage {:?}; independent level {:?}",
        class(&t.triage),
        ri.level
    );
    assert!(matches!(ri.level, ReplayLevel::NotEstablished { .. }));
    assert!(matches!(t.triage, Triage::Differ(ref d) if d.outlier == Outlier::Independent));
    let _ = f.den;
}

/// The referee delegates rationals, RREF, affine forms and case literals to the INDEPENDENT backend
/// (src/referee.rs). A rule with an affine case equation: its generic point is solved by `q_rref` / `expr_linear`, which
/// are not traced; a difference there first shows at `Coefficient`, a referee-covered stage, where the referee inherits
/// the independent side's solution. Clean source: AgreePass. Under the audit's source mutation R1 (independent
/// `q_rref` with a negated constant column, applied temporarily, never committed) this printed the outlier the triage
/// names; see the report.
#[test]
fn audit_rows3_referee_shares_independent_linalg() {
    if !licensed_or_skip("audit_rows3_referee_shares_independent_linalg") {
        return;
    }
    let f = face();
    let mut rule = f.good.clone();
    rule["equations"] = json!(["n0-n1-1"]);
    let r = RowsRule::from_value(rule, 9).unwrap();
    let sym = SymbolicaAlgebra::new().unwrap();
    let (vi, vs, rep) = triage_identity(
        &recording(Independent),
        &recording(sym),
        &f.text,
        &r,
        &[PointSpec::Generic, PointSpec::Generic],
        &RowsOptions::default(),
    )
    .unwrap();
    println!(
        "referee-sharing probe: triage {:?} | independent {} | symbolica {}",
        class(&rep.triage),
        vi.verdict.kind(),
        vs.verdict.kind()
    );
}
