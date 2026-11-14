//! Wrappers around a [`CheckAlgebra`] backend used by the triage: [`Recording`] stores every [`Stage`] quantity the
//! domain logic reports (the trace the `Both` mode compares), [`Planted`] injects ONE deliberate defect (a wrong field
//! multiplication, a wrong parsed coefficient, a dropped row) to show that the triage localises it (the power test
//! of `tests/triage.rs`; never used by the entry points). Both delegate everything else unchanged.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustred_campaign_core::replay_base::algebra::{
    CheckAlgebra, CoeffError, CoeffStats, Ctx, Q, QResult, Stage, Unknown, ZPoly,
};

/// One trace entry (rows4: the core's type, shared with the certificate run's `Recorder`).
pub use rustred_campaign_core::replay_base::algebra::Event;

/// What a wrapper changes; the default methods delegate.
pub trait Hook<A: CheckAlgebra>: Sync {
    fn mul(&self, a: &A, p: u64, x: u64, y: u64) -> u64 {
        a.mul(p, x, y)
    }
    fn eval_ratio(
        &self,
        a: &A,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        a.eval_ratio(text, arity, ctxs, num, den)
    }
    /// the rows handed to the elimination (`None` = unchanged)
    fn rows<'r>(&self, rows: &'r [Vec<(usize, u64)>]) -> &'r [Vec<(usize, u64)>] {
        rows
    }
    fn observe(&self, _stage: Stage, _at: &dyn Fn() -> String, _value: &dyn Fn() -> String) {}
}

/// A backend `inner` seen through a [`Hook`].
pub struct Wrap<A: CheckAlgebra, H: Hook<A>> {
    pub inner: A,
    pub hook: H,
}

impl<A: CheckAlgebra, H: Hook<A> + 'static> CheckAlgebra for Wrap<A, H> {
    type Literal = A::Literal;
    type Elim<'r>
        = A::Elim<'r>
    where
        Self: 'r;

    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn is_prime(&self, n: u64) -> bool {
        self.inner.is_prime(n)
    }
    fn add(&self, p: u64, a: u64, b: u64) -> u64 {
        self.inner.add(p, a, b)
    }
    fn sub(&self, p: u64, a: u64, b: u64) -> u64 {
        self.inner.sub(p, a, b)
    }
    fn mul(&self, p: u64, a: u64, b: u64) -> u64 {
        self.hook.mul(&self.inner, p, a, b)
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
        self.inner.q_inverse(m)
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
        self.inner.q_unit_in_row_space(rows, col)
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
        self.hook
            .eval_ratio(&self.inner, text, arity, ctxs, num, den)
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
        self.inner.literal_is_zero_at(lit, point)
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
    ) -> Result<Self::Elim<'r>, Unknown> {
        self.inner
            .eliminate(self.hook.rows(rows), ncols, p, max_entries)
    }
    fn observe(&self, stage: Stage, at: &dyn Fn() -> String, value: &dyn Fn() -> String) {
        self.hook.observe(stage, at, value)
    }
}

/// Records every observed quantity.
#[derive(Default)]
pub struct RecordHook {
    pub events: Mutex<Vec<Event>>,
}

impl<A: CheckAlgebra> Hook<A> for RecordHook {
    fn observe(&self, stage: Stage, at: &dyn Fn() -> String, value: &dyn Fn() -> String) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Event {
                stage,
                at: at(),
                value: value(),
            });
    }
}

/// A backend whose trace is recorded.
pub type Recording<A> = Wrap<A, RecordHook>;

/// Record the trace of `inner`.
pub fn recording<A: CheckAlgebra>(inner: A) -> Recording<A> {
    Wrap {
        inner,
        hook: RecordHook::default(),
    }
}

impl<A: CheckAlgebra> Recording<A> {
    pub fn take_events(&self) -> Vec<Event> {
        std::mem::take(&mut *self.hook.events.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// The single defect a [`Planted`] backend injects (the triage's power test).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plant {
    /// the `nth` field multiplication with both operands > 1 returns the product + 1
    Multiplication { nth: usize },
    /// the numerator of the `nth` coefficient evaluation (first context) is off by one
    Coefficient { nth: usize },
    /// the `nth` elimination drops its first non-empty row
    DropRow { nth: usize },
}

/// Injects one [`Plant`] and records the trace.
pub struct PlantHook {
    pub plant: Plant,
    calls: AtomicUsize,
    pub record: RecordHook,
}

impl PlantHook {
    pub fn new(plant: Plant) -> PlantHook {
        PlantHook {
            plant,
            calls: AtomicUsize::new(0),
            record: RecordHook::default(),
        }
    }
    fn hit(&self, nth: usize) -> bool {
        self.calls.fetch_add(1, Ordering::SeqCst) == nth
    }
}

impl<A: CheckAlgebra> Hook<A> for PlantHook {
    fn mul(&self, a: &A, p: u64, x: u64, y: u64) -> u64 {
        let v = a.mul(p, x, y);
        match self.plant {
            Plant::Multiplication { nth } if x > 1 && y > 1 && self.hit(nth) => a.add(p, v, 1),
            _ => v,
        }
    }
    fn eval_ratio(
        &self,
        a: &A,
        text: &str,
        arity: usize,
        ctxs: &[Ctx],
        num: &mut [u64],
        den: &mut [u64],
    ) -> Result<CoeffStats, CoeffError> {
        let st = a.eval_ratio(text, arity, ctxs, num, den)?;
        if let Plant::Coefficient { nth } = self.plant
            && !ctxs.is_empty()
            && self.hit(nth)
        {
            num[0] = a.add(ctxs[0].p, num[0], 1);
        }
        Ok(st)
    }
    fn rows<'r>(&self, rows: &'r [Vec<(usize, u64)>]) -> &'r [Vec<(usize, u64)>] {
        match self.plant {
            Plant::DropRow { nth } if self.hit(nth) => {
                let mut copy: Vec<Vec<(usize, u64)>> = rows.to_vec();
                if let Some(i) = copy.iter().position(|r| !r.is_empty()) {
                    copy[i].clear();
                }
                // test-only: the elimination borrows its rows for its own lifetime
                Box::leak(copy.into_boxed_slice())
            }
            _ => rows,
        }
    }
    fn observe(&self, stage: Stage, at: &dyn Fn() -> String, value: &dyn Fn() -> String) {
        <RecordHook as Hook<A>>::observe(&self.record, stage, at, value)
    }
}

/// A backend with one planted defect (and a recorded trace).
pub type Planted<A> = Wrap<A, PlantHook>;

pub fn planted<A: CheckAlgebra>(inner: A, plant: Plant) -> Planted<A> {
    Wrap {
        inner,
        hook: PlantHook::new(plant),
    }
}

impl<A: CheckAlgebra> Planted<A> {
    pub fn take_events(&self) -> Vec<Event> {
        std::mem::take(
            &mut *self
                .hook
                .record
                .events
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        )
    }
}
