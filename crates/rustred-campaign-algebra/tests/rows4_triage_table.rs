//! Lane rows4: the PERMANENT TRIAGE TABLE (re-audit RA3-m3 / RA3-M2 / RA3-M3, rebuilt from the auditor's
//! `audit_rows3_triage.rs`). One defect is planted per stage on EITHER side (independent / Symbolica) through a
//! delegating wrapper, on a TRUE and on a FALSE rule; clean bad rules and degenerate texts are run without defects; the
//! shared-logic oracle is exercised on verdicts a shared-logic defect would produce. Requirements (binding, asserted):
//! every planted defect that changed a value is a `Differ` blamed on the RIGHT side, or an explicit `Unresolved` class
//! with its reason; no wrong side; a TRUE rule with a wrong recorded seed reads `RecordInconsistent`; a removable 0/0 is
//! `TextPole`, a grammar-refused text `GrammarRefusal` (no side blamed). Prints the table (case x verdict) and the
//! summary line. SKIPs without the Symbolica licence.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use common::*;
use rustred_campaign_algebra::modes::{Traced, UnresolvedClass, oracle};
use rustred_campaign_algebra::wrap::{Event, recording};
use rustred_campaign_algebra::{
    Outlier, SymbolicaAlgebra, Triage, replay_evidence_triage, triage_identity,
};
use rustred_campaign_core::ibp_rows::{
    CellStatus, PointSpec, PointStatus, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule,
    TableBinding, identity_test,
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
    /// the `nth` case-literal zero test answers the opposite (applicability: rows4 traces it at stage Points)
    Literal(usize),
    /// rows4 (the audit's source mutation R1, now planted): every RREF row's last (constant) column negated
    Rref,
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
        let (mut r, pv) = self.inner.q_rref(rows, ncols)?;
        if self.defect == Defect::Rref {
            for row in r.iter_mut() {
                if let Some(x) = row.last_mut()
                    && !x.is_zero()
                {
                    self.effects.fetch_add(1, Ordering::SeqCst);
                    x.n = -x.n;
                }
            }
        }
        Ok((r, pv))
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
    fn int_mat_mul(
        &self,
        a: &[i128],
        b: &[i128],
        r: usize,
        k: usize,
        c: usize,
    ) -> Option<Vec<i128>> {
        self.inner.int_mat_mul(a, b, r, k, c)
    }
    fn int_div_exact(&self, a: i128, b: i128) -> Option<i128> {
        self.inner.int_div_exact(a, b)
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

struct Row {
    case: String,
    side: &'static str,
    expected: &'static str,
    class: String,
    stage: String,
    outlier: String,
    unresolved: String,
    ind: String,
    sym: String,
    fired: usize,
}

fn class_of(t: &Triage) -> (String, String, String, String) {
    let dash = || "-".to_string();
    match t {
        Triage::AgreePass => ("AgreePass".into(), dash(), dash(), dash()),
        Triage::AgreeFail { .. } => ("AgreeFail".into(), dash(), dash(), dash()),
        Triage::RecordInconsistent { .. } => ("RecordInconsistent".into(), dash(), dash(), dash()),
        Triage::SharedLogicDiffers(d) => {
            ("SharedLogicDiffers".into(), d.what.clone(), dash(), dash())
        }
        Triage::AgreeOther { verdict } => (
            format!(
                "AgreeOther({})",
                verdict.split([' ', '(', '{']).next().unwrap_or("")
            ),
            dash(),
            dash(),
            dash(),
        ),
        Triage::Differ(d) => (
            "Differ".into(),
            format!("{:?}", d.stage),
            format!("{:?}", d.outlier),
            d.unresolved
                .as_ref()
                .map_or_else(dash, |u| format!("{u:?}")),
        ),
        Triage::SymbolicaUnsupported { reason } => (
            format!(
                "SymbolicaUnsupported({})",
                reason.chars().take(40).collect::<String>()
            ),
            dash(),
            dash(),
            dash(),
        ),
    }
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
    let (class, stage, outlier, unresolved) = class_of(&rep.triage);
    if let Triage::Differ(d) = &rep.triage
        && d.outlier == Outlier::Unresolved
    {
        println!("  [{case} / {side}] Unresolved reason: {}", d.note);
    }
    Row {
        case: case.into(),
        side,
        expected,
        class,
        stage,
        outlier,
        unresolved,
        ind: vi.verdict.kind().into(),
        sym: vs.verdict.kind().into(),
        fired,
    }
}

/// The table (case x verdict), rebuilt from the auditor's and extended (Rref, Points); the binding assertions.
#[test]
fn rows4_triage_table() {
    if !licensed_or_skip("rows4_triage_table") {
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
    let removable = with_rhs(&f.good, |r| {
        r.push(json!([
            pat(&zk),
            format!("(0)/(1*n0-{})", off[0] - f.tshift[0])
        ]));
    });
    let grammar = with_rhs(&f.good, |r| r.push(json!([pat(&zk), "1/2*n1-1/2*n1"])));
    let clean: Vec<(&str, &'static str, Value)> = vec![
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
            "RecordInconsistent",
            wrong_seed,
        ),
        (
            "true + 0/(n0-v0): removable 0/0 at a key",
            "Differ TextPole",
            removable,
        ),
        (
            "true + `1/2*n1-1/2*n1` (grammar-refused)",
            "Differ GrammarRefusal",
            grammar,
        ),
    ];
    for (name, exp, rule) in &clean {
        rows.push(run(name, "-", exp, Defect::None, &f.text, rule, &pts));
    }
    // (B) one defect per stage, each side, on the TRUE and the FALSE (+corner) rule
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
        ("Applicability: literal #0", Defect::Literal(0)),
    ];
    let mut with_exc = f.good.clone();
    with_exc["exceptions"] = json!([["n0-100"]]);
    let mut with_eq = f.good.clone();
    with_eq["equations"] = json!(["n0-n1-1"]);
    let gen2 = vec![PointSpec::Generic, PointSpec::Generic];
    for side in ["independent", "symbolica"] {
        let exp = if side == "independent" {
            "Differ, outlier Independent"
        } else {
            "Differ, outlier Symbolica"
        };
        rows.push(run(
            "true rule + exception n0-100 | Applicability: literal #2",
            side,
            exp,
            Defect::Literal(2),
            &f.text,
            &with_exc,
            &pts,
        ));
        rows.push(run(
            "true rule + equation n0-n1-1 | RREF constant column negated (R1)",
            side,
            exp,
            Defect::Rref,
            &f.text,
            &with_eq,
            &gen2,
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
        "\n| case | defect side | expected | triage | stage | outlier | unresolved class | ind verdict | sym verdict | defect effects |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.case,
            r.side,
            r.expected,
            r.class,
            r.stage,
            r.outlier,
            r.unresolved,
            r.ind,
            r.sym,
            r.fired
        );
    }
    let planted: Vec<&Row> = rows
        .iter()
        .filter(|r| r.side != "-" && r.fired > 0)
        .collect();
    let right = |r: &Row| {
        r.class == "Differ"
            && ((r.side == "independent" && r.outlier == "Independent")
                || (r.side == "symbolica" && r.outlier == "Symbolica"))
    };
    let n_right = planted.iter().filter(|r| right(r)).count();
    let unresolved: Vec<&&Row> = planted
        .iter()
        .filter(|r| r.class == "Differ" && r.outlier == "Unresolved")
        .collect();
    let wrong = planted
        .iter()
        .filter(|r| {
            r.class == "Differ"
                && ((r.side == "independent" && r.outlier == "Symbolica")
                    || (r.side == "symbolica" && r.outlier == "Independent"))
        })
        .count();
    let invisible: Vec<&&Row> = planted.iter().filter(|r| r.class != "Differ").collect();
    println!(
        "planted runs {} (defect fired in {}): right outlier {n_right}, Unresolved {} {:?}, WRONG outlier {wrong}, not seen as Differ {}",
        rows.iter().filter(|r| r.side != "-").count(),
        planted.len(),
        unresolved.len(),
        unresolved
            .iter()
            .map(|r| format!("{} [{}]: {}", r.case, r.side, r.unresolved))
            .collect::<Vec<_>>(),
        invisible.len()
    );
    // class x verdict summary
    let mut summary: std::collections::BTreeMap<(String, String), usize> = Default::default();
    for r in &planted {
        let class = r
            .case
            .split(" | ")
            .nth(1)
            .unwrap_or(&r.case)
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let verdict = if right(r) {
            "right side".to_string()
        } else {
            format!("{} {}", r.class, r.unresolved)
        };
        *summary.entry((class, verdict)).or_insert(0) += 1;
    }
    println!("defect class x verdict: {summary:?}");
    assert_eq!(wrong, 0, "a planted defect blamed on the wrong side");
    assert!(invisible.is_empty(), "a fired defect not seen as Differ");
    for r in &unresolved {
        assert!(
            r.unresolved != "-",
            "Unresolved without a class: {}",
            r.case
        );
    }
    assert_eq!(n_right + unresolved.len(), planted.len());
    // the clean cases
    let get = |name: &str| rows.iter().find(|r| r.case == name).unwrap();
    assert_eq!(get("true rule").class, "AgreePass");
    for name in [
        "false: + 1*corner",
        "false on the face key only",
        "false: coefficient 0 doubled",
    ] {
        assert_eq!(get(name).class, "AgreeFail", "{name}");
    }
    assert_eq!(
        get("TRUE rule, wrong recorded seed").class,
        "RecordInconsistent"
    );
    let tp = get("true + 0/(n0-v0): removable 0/0 at a key");
    assert_eq!(
        (tp.class.as_str(), tp.unresolved.as_str()),
        ("Differ", "TextPole")
    );
    let gr = get("true + `1/2*n1-1/2*n1` (grammar-refused)");
    assert_eq!(
        (gr.class.as_str(), gr.unresolved.as_str()),
        ("Differ", "GrammarRefusal")
    );
    let _ = (UnresolvedClass::TextPole, Stage::Points);
}

/// RA3-M3: the oracle re-decides the shared logic at the integer keys: clean verdicts agree; verdicts a shared-logic
/// defect would produce ("never fail" = audit mutant M02, RHS sign flip = M05, an applicability defect, a control
/// defect = M03, a combination defect = M01) are reported as differences.
#[test]
fn rows4_shared_logic_oracle() {
    if !licensed_or_skip("rows4_shared_logic_oracle") {
        return;
    }
    let f = face();
    let fam = Family::parse(&f.text).unwrap();
    let off = key_off_face(&f);
    let on = key_on_face(&f);
    let pts = vec![PointSpec::Key(off.clone()), PointSpec::Key(on.clone())];
    let good = RowsRule::from_value(f.good.clone(), 9).unwrap();
    let corner = "I(1,1,1,1,1,1,1,0,0)";
    let falsy =
        RowsRule::from_value(with_rhs(&f.good, |r| r.push(json!([corner, "1"]))), 9).unwrap();
    let vg = identity_test(&fam, &good, &pts, &RowsOptions::default());
    let vf = identity_test(&fam, &falsy, &pts, &RowsOptions::default());
    let cg = oracle::check(&f.text, &good, &vg);
    let cf = oracle::check(&f.text, &falsy, &vf);
    println!(
        "clean: true rule {} keys / {} cells, diffs {:?}, skipped {:?}",
        cg.keys_checked, cg.cells_checked, cg.diffs, cg.skipped
    );
    println!(
        "clean: false rule {} keys / {} cells, diffs {:?}",
        cf.keys_checked, cf.cells_checked, cf.diffs
    );
    assert!(cg.diffs.is_empty() && cf.diffs.is_empty());
    assert_eq!(cg.cells_checked, 4);
    // M02 "never fail": the false rule's key cells reported in the span and passing
    let mut m02 = vf.clone();
    for p in &mut m02.points {
        for c in &mut p.cells {
            c.in_span = true;
            c.status = CellStatus::Pass;
            c.mutation_rejected = Some(true);
            c.corners_in_span.clear();
        }
        p.status = PointStatus::Pass;
    }
    // M05 sign flip: the true rule's key cells reported failing
    let mut m05 = vg.clone();
    for p in &mut m05.points {
        for c in &mut p.cells {
            c.in_span = false;
            c.status = CellStatus::Fail;
        }
        p.status = PointStatus::Fail;
    }
    // applicability: an applicable key reported not applicable
    let mut app = vg.clone();
    app.points[0].status = PointStatus::NotApplicable(
        rustred_campaign_core::ibp_rows::NotApplicableReason::Equation(0),
    );
    // M03 control: the +1 mutant reported in the span (a passing cell would be undecided)
    let mut m03 = vg.clone();
    m03.points[0].cells[0].mutation_rejected = Some(false);
    // M01 combination: one cell failing, the point reported passing
    let mut m01 = vg.clone();
    m01.points[0].cells[1].in_span = false;
    m01.points[0].cells[1].status = CellStatus::Fail;
    for (name, rule, v) in [
        ("M02 never fail", &falsy, &m02),
        ("M05 sign flip", &good, &m05),
        ("applicability", &good, &app),
        ("M03 control", &good, &m03),
        ("M01 combination", &good, &m01),
    ] {
        let c = oracle::check(&f.text, rule, v);
        println!(
            "{name}: {} diff(s), first {:?}",
            c.diffs.len(),
            c.diffs.first()
        );
        assert!(!c.diffs.is_empty(), "{name}: the oracle saw nothing");
    }
}

/// `Both` replay: an independent-side false pass is blocked (Differ at Membership, outlier Independent); a TRUE rule
/// with a wrong recorded seed is `RecordInconsistent` and its level is blocked; a clean true rule keeps Established.
#[test]
fn rows4_both_replay_blocks() {
    if !licensed_or_skip("rows4_both_replay_blocks") {
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
        beacon: Some(
            rustred_campaign_core::ibp_rows::Beacon::from_hex("feedfacecafebeef0123456789abcdef")
                .unwrap(),
        ),
        ..RowsOptions::default()
    };
    let pol = ReplayPolicy::default();
    let m = replay_evidence_triage(
        &bent(Independent, Defect::RuleInSpan),
        &recording(SymbolicaAlgebra::new().unwrap()),
        &f.text,
        &false_rule,
        &keys,
        &pol,
        &opts,
    )
    .unwrap();
    let t = m.triage.unwrap();
    println!(
        "independent side lies: {:?}; level {:?}",
        class_of(&t.triage),
        m.independent.as_ref().map(|r| &r.level)
    );
    assert!(matches!(
        m.independent.unwrap().level,
        ReplayLevel::NotEstablished { .. }
    ));
    assert!(matches!(t.triage, Triage::Differ(ref d) if d.outlier == Outlier::Independent));
    // the certificate Both mode
    let mut ws = f.good.clone();
    let mut s = vec![0i64; 9];
    s[0] = 3;
    ws["seeds"] = json!([pat(&s)]);
    let wrong_seed = RowsRule::from_value(ws, 9).unwrap();
    let good = RowsRule::from_value(f.good.clone(), 9).unwrap();
    for (name, rule, want) in [
        ("true rule", &good, "AgreePass"),
        ("true rule, wrong seed", &wrong_seed, "RecordInconsistent"),
    ] {
        let m = rustred_campaign_algebra::replay_evidence_mode(
            rustred_campaign_algebra::IdentityMode::Both,
            &f.text,
            rule,
            &keys,
            &pol,
            &opts,
        )
        .unwrap();
        let rec = m.independent.unwrap();
        let t = m.triage.unwrap();
        println!(
            "Both replay {name}: {:?}, level {:?}, oracle {:?}",
            class_of(&t.triage),
            rec.level,
            t.oracle.as_ref().map(|o| (o.keys_checked, o.cells_checked))
        );
        assert_eq!(class_of(&t.triage).0, want, "{name}");
        assert!(rec.certificate);
        if want == "AgreePass" {
            assert_eq!(rec.level, ReplayLevel::Established);
        } else {
            assert!(matches!(rec.level, ReplayLevel::NotEstablished { .. }));
        }
    }
}

/// The referee agrees with BOTH sides on their shared contract (a referee that is itself wrong would blame the wrong
/// side): primality incl. strong pseudoprimes, field, rationals, linear algebra, coefficient texts of the face rule.
#[test]
fn rows4_referee_agrees_with_both_sides() {
    if !licensed_or_skip("rows4_referee_agrees_with_both_sides") {
        return;
    }
    use rustred_campaign_algebra::referee::Referee;
    let sym = SymbolicaAlgebra::new().unwrap();
    let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut nums: Vec<u64> = vec![
        2047,
        1_373_653,
        25_326_001,
        3_215_031_751,
        2_152_302_898_747,
        3_474_749_660_383,
        341_550_071_728_321,
        3_825_123_056_546_413_051,
        561,
        1105,
        1729,
        2_305_843_009_213_693_951,
        (1u64 << 63) - 25,
        u64::MAX - 58,
    ];
    for _ in 0..20_000 {
        let base = 7u64 << 60;
        nums.push(base + next() % (1u64 << 59));
        nums.push(next() % 1_000_000);
    }
    let mut primes = 0;
    for &n in &nums {
        let r = Referee.is_prime(n);
        assert_eq!(r, Independent.is_prime(n), "{n}");
        assert_eq!(r, sym.is_prime(n), "{n}");
        primes += usize::from(r);
    }
    let p = (7u64 << 60) + 1_234_567;
    let p = (p..).find(|&q| Independent.is_prime(q)).unwrap();
    for _ in 0..20_000 {
        let (a, b) = (next() % p, next() % p);
        assert_eq!(Referee.mul(p, a, b), Independent.mul(p, a, b));
        assert_eq!(Referee.sub(p, a, b), sym.sub(p, a, b));
        assert_eq!(Referee.inv(p, a), Independent.inv(p, a));
        let (x, y) = (
            Q {
                n: (next() % 2001) as i128 - 1000,
                d: (next() % 97 + 1) as i128,
            },
            Q {
                n: (next() % 2001) as i128 - 1000,
                d: (next() % 89 + 1) as i128,
            },
        );
        let (x, y) = (
            Independent.q_new(x.n, x.d).unwrap(),
            Independent.q_new(y.n, y.d).unwrap(),
        );
        assert_eq!(Referee.q_add(x, y), Independent.q_add(x, y));
        assert_eq!(Referee.q_mul(x, y), sym.q_mul(x, y));
        assert_eq!(Referee.q_div(x, y).ok(), Independent.q_div(x, y).ok());
    }
    for k in 0..300 {
        let n = 2 + k % 5;
        let m: Vec<Vec<Q>> = (0..n)
            .map(|_| {
                (0..n + 1)
                    .map(|_| Q::int((next() % 7) as i128 - 3))
                    .collect()
            })
            .collect();
        assert_eq!(
            Referee.q_rref(&m, n).unwrap(),
            Independent.q_rref(&m, n).unwrap()
        );
        assert_eq!(Referee.q_rank(&m).unwrap(), sym.q_rank(&m).unwrap());
        let sq: Vec<Vec<Q>> = m.iter().map(|r| r[..n].to_vec()).collect();
        assert_eq!(Referee.q_inverse(&sq).ok(), Independent.q_inverse(&sq).ok());
        let iz: Vec<Vec<i128>> = sq.iter().map(|r| r.iter().map(|q| q.n).collect()).collect();
        assert_eq!(Referee.det_i128(&iz), sym.det_i128(&iz));
        assert_eq!(
            Referee.q_unit_in_row_space(&m, k % (n + 1)).unwrap(),
            Independent.q_unit_in_row_space(&m, k % (n + 1)).unwrap()
        );
    }
    let r = RowsRule::from_value(face().good, 9).unwrap();
    let mut texts = 0;
    for (_, t) in &r.rhs {
        for _ in 0..50 {
            let ctx = Ctx {
                p,
                vals: (0..10).map(|_| next() % p).collect(),
            };
            let (mut a, mut b, mut c, mut d) = ([0u64], [0u64], [0u64], [0u64]);
            Referee
                .eval_ratio(t, 9, std::slice::from_ref(&ctx), &mut a, &mut b)
                .unwrap();
            Independent
                .eval_ratio(t, 9, std::slice::from_ref(&ctx), &mut c, &mut d)
                .unwrap();
            let v = |n: u64, dd: u64| Independent.inv(p, dd).map(|i| Independent.mul(p, n, i));
            assert_eq!(v(a[0], b[0]), v(c[0], d[0]), "{t}");
            texts += 1;
        }
    }
    println!(
        "referee: {} numbers ({primes} prime), 20,000 field / rational ops, 300 matrices, {texts} coefficient values: equal to both sides",
        nums.len()
    );
}

/// The referee and the oracle stay SMALL (re-audit RA3-M2 asks for a tiny third implementation): their non-test line
/// counts, printed and bounded.
#[test]
fn referee_is_small() {
    let count = |rel: &str| {
        let t = std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        t.split("#[cfg(test)]").next().unwrap_or("").lines().count()
    };
    let (r, o) = (count("src/referee.rs"), count("src/modes/oracle.rs"));
    println!("referee {r} non-test lines, oracle {o} non-test lines");
    assert!(r <= 1_000 && o <= 800, "referee {r}, oracle {o}");
}
