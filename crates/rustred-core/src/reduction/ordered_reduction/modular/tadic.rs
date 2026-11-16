//! `Tadic` (helper-vh2): replay of the recorded trace with weights that are
//! truncated Laurent series in `t = d - d0` over `F_p`, with a proved
//! absolute precision.
//!
//! Design notes: `notes/TADIC_DESIGN.md` of the helper workspace. In short:
//!
//! - The recorded trace (`Trace`, see `mod.rs`) is a straight-line program:
//!   `Init(s, c)`, `Pop(s)`, `New/Add(s, c)` (`s (=|+=) active * c`),
//!   `Cancel(s)` and the terminal counterparts. No operation divides by a
//!   weight: weights are only multiplied by exact step coefficients `c` and
//!   summed. Precision is therefore lost only through the poles of the step
//!   coefficients at `d0` (a product with `c` of valuation `v` has absolute
//!   precision `P + v`), never through a cancellation (a sum has the
//!   precision of its least precise term, whatever its leading terms do), and
//!   no operation ever needs the leading coefficient of a weight. A weight
//!   whose known coefficients all vanish is `O(t^P)`, never "zero".
//! - The valuation `v(c)` of every step coefficient at `d0` is computed
//!   EXACTLY over `Q` (root multiplicities by synthetic division over `Z`),
//!   so a backward pass over the trace (`plan`) gives, for every operation,
//!   the absolute precision its result needs for every output terminal to
//!   reach its target precision. The forward replay computes every value to
//!   exactly that precision (dead values are skipped). The output precision
//!   is then proved by construction; there is no "precision exhausted"
//!   retry loop. Exhaustion of the length budget (`max_length`) fails closed.
//! - Coefficients are expanded modulo `p` from their exact Taylor shift at
//!   `d0`. A prime is bad for a coefficient when it divides the leading
//!   Taylor coefficient of its denominator (or a constant denominator): the
//!   prime is then unusable and another one is drawn. On a usable prime every
//!   series coefficient computed is the reduction modulo `p` of the exact
//!   (p-integral) Laurent coefficient: there are no unlucky primes.
//! - Recorded erasures (`Cancel`, `TermCancel`) are decided by the recording
//!   pass at generic `d` (probes). They are certified here exactly as the
//!   sampled arithmetic certifies them: one numeric replay of the trace at
//!   `check_points` random points of every prime (`replay::replay`), which
//!   fails on any erased weight that is nonzero at a finite point. Optionally
//!   (`erasure_check_order`) the series of every erased weight is also
//!   computed to that absolute precision and required to vanish there.
//!
//! What is proved and what is probabilistic is stated in the design notes.
use std::time::Instant;

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
#[allow(unused_imports)] // the same trait set as `field.rs` (method resolution of `Zp64`)
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;
use symbolica::utils::resources;

use super::executor::Executor;
use super::field::{DenseRational, Fe, reduce};
use super::replay::{ReplayError, replay};
use super::{CoefRef, ModularStatistics, OpTag, Trace, error};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::ReductionError;
use crate::reduction::ordered_reduction::arithmetic::TadicOptions;

/// Prefix of every precision failure of the t-adic replay (a stable marker
/// inside `ReductionError::ModularWeights`, see [`is_precision_failure`]).
pub const TADIC_PRECISION_FAILURE: &str = "t-adic precision";

/// Need of a value that no output reads (planner, `i32` domain).
const DEAD: i32 = i32::MIN / 4;
/// Need of a value that no output reads (stored plan).
const DEAD16: i16 = i16::MIN;

/// Trace operations between two resource checkpoints of a series replay.
const CHECKPOINT_OPS: usize = 256;

/// Attempts at drawing erasure-check points of one prime before the prime
/// is declared unusable (every point a pole of some coefficient).
const CHECK_POINT_ATTEMPTS: usize = 4;

/// t-adic primes: `[2^62 + 2^61 + 2^60, 2^62 + 2^61 + 2^60 + 2^59)` plus the
/// primes consumed (disjoint from the probe, lift and verification ranges).
const TADIC_BASE: u64 = (1 << 62) + (1 << 61) + (1 << 60);
const TADIC_SALT: u64 = 0x7461_6469_63;
/// Seed salt of the erasure-check points.
const CHECK_POINTS_SALT: u64 = 0x7461_6368_6b;

fn precision_error(detail: impl std::fmt::Display) -> ReductionError {
    error(format!("{TADIC_PRECISION_FAILURE}: {detail}"))
}

/// Whether `error` is a precision failure of the t-adic replay (length budget
/// or an inconsistent plan): rerun with a larger `max_length`, or report.
pub fn is_precision_failure(error: &ReductionError) -> bool {
    matches!(error, ReductionError::ModularWeights { detail } if detail.starts_with(TADIC_PRECISION_FAILURE))
}

// ---------------------------------------------------------------------------
// Exact valuations over Q.
// ---------------------------------------------------------------------------

/// `(q, r)` with `a(x) = (x - d0) q(x) + r` (synthetic division; dense
/// ascending coefficients).
fn divide_by_root(a: &[Integer], d0: &Integer) -> (Vec<Integer>, Integer) {
    let n = a.len();
    if n <= 1 {
        return (vec![], a.first().cloned().unwrap_or_else(Integer::zero));
    }
    let mut q = vec![Integer::zero(); n - 1];
    q[n - 2] = a[n - 1].clone();
    for k in (1..n - 1).rev() {
        let next = &a[k] + &(d0 * &q[k]);
        q[k - 1] = next;
    }
    let r = &a[0] + &(d0 * &q[0]);
    (q, r)
}

/// Multiplicity of the root `d0` of an integer polynomial (dense ascending);
/// `None` for the zero polynomial.
fn root_multiplicity(polynomial: &[Integer], d0: &Integer) -> Option<u32> {
    let mut current: Vec<Integer> = polynomial.to_vec();
    while current.last().is_some_and(|c| c.is_zero()) {
        current.pop();
    }
    if current.is_empty() {
        return None;
    }
    let mut multiplicity = 0u32;
    loop {
        let (quotient, remainder) = divide_by_root(&current, d0);
        if !remainder.is_zero() {
            return Some(multiplicity);
        }
        multiplicity += 1;
        current = quotient;
    }
}

/// Root multiplicities at `d0` of one exact coefficient: `None` for the zero
/// coefficient, else `(numerator, denominator)` multiplicities.
fn multiplicities(
    rational: &DenseRational,
    d0: &Integer,
) -> Result<Option<(u32, u32)>, ReductionError> {
    let Some(numerator) = root_multiplicity(&rational.numerator, d0) else {
        return Ok(None);
    };
    let denominator = root_multiplicity(&rational.denominator, d0)
        .ok_or_else(|| error("a step coefficient has a zero denominator"))?;
    Ok(Some((numerator, denominator)))
}

/// Valuation at `d0` over Q of an exact coefficient (`None` for zero).
pub(crate) fn coefficient_valuation(
    coefficient: &Coefficient,
    d0: i64,
) -> Result<Option<i32>, ReductionError> {
    let dense = DenseRational::from_coefficient(coefficient);
    Ok(multiplicities(&dense, &Integer::from(d0))?
        .map(|(numerator, denominator)| numerator as i32 - denominator as i32))
}

/// Prime-independent orders of every interned step coefficient of a trace.
pub(super) struct Orders {
    /// Per constant: `false` for the zero constant.
    constants: Vec<bool>,
    /// Per polynomial coefficient: its multiplicities (`None`: zero).
    polynomials: Vec<Option<(u32, u32)>>,
}

impl Orders {
    pub fn new(trace: &Trace, d0: i64) -> Result<Self, ReductionError> {
        let d0 = Integer::from(d0);
        let constants = trace
            .constants
            .iter()
            .map(|(numerator, _)| !numerator.is_zero())
            .collect();
        let mut polynomials = Vec::with_capacity(trace.polynomials.len());
        for (index, rational) in trace.polynomials.iter().enumerate() {
            if index % 1024 == 0 {
                resources::checkpoint()?;
            }
            polynomials.push(multiplicities(rational, &d0)?);
        }
        Ok(Self {
            constants,
            polynomials,
        })
    }

    /// Valuation over Q of an interned coefficient; `None` for zero.
    fn valuation(&self, reference: CoefRef) -> Option<i32> {
        if reference.is_constant() {
            self.constants[reference.index()].then_some(0)
        } else {
            self.polynomials[reference.index()]
                .map(|(numerator, denominator)| numerator as i32 - denominator as i32)
        }
    }
}

// ---------------------------------------------------------------------------
// Series over F_p.
// ---------------------------------------------------------------------------

/// A truncated Laurent series modulo `p`: the coefficients of
/// `t^start .. t^(start + len - 1)` are known; the absolute precision is
/// `start + len` (the value is `sum + O(t^precision)`). An empty series is
/// `O(t^start)`: a value whose known coefficients all vanish, never an exact
/// zero. Leading zero coefficients are stripped (they are known zeros).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Series {
    start: i32,
    coefficients: Vec<Fe>,
}

impl Series {
    /// `O(t^precision)`.
    fn known_zero(precision: i32) -> Self {
        Self {
            start: precision,
            coefficients: vec![],
        }
    }

    fn precision(&self) -> i32 {
        self.start + self.coefficients.len() as i32
    }

    fn words(&self) -> usize {
        self.coefficients.len()
    }

    /// Forget every coefficient of order `>= precision`.
    fn truncate(&mut self, precision: i32) {
        if precision >= self.precision() {
            return;
        }
        if precision <= self.start {
            *self = Self::known_zero(precision);
        } else {
            self.coefficients
                .truncate((precision - self.start) as usize);
        }
    }

    fn normalize(&mut self, field: &Zp64) {
        let lead = self
            .coefficients
            .iter()
            .take_while(|c| field.is_zero(c))
            .count();
        if lead > 0 {
            self.coefficients.drain(..lead);
            self.start += lead as i32;
        }
    }

    /// `self += other`; the precision of the sum is the smaller one.
    fn add_assign(&mut self, field: &Zp64, other: &Series) {
        let precision = self.precision().min(other.precision());
        let start = self.start.min(other.start);
        if precision <= start {
            *self = Self::known_zero(precision);
            return;
        }
        let mut sum = vec![field.zero(); (precision - start) as usize];
        for series in [&*self, other] {
            for (k, c) in series.coefficients.iter().enumerate() {
                let order = series.start + k as i32;
                if order >= precision {
                    break;
                }
                let slot = &mut sum[(order - start) as usize];
                *slot = field.add(&*slot, c);
            }
        }
        *self = Self {
            start,
            coefficients: sum,
        };
        self.normalize(field);
    }

    fn any_nonzero(&self, field: &Zp64) -> bool {
        self.coefficients.iter().any(|c| !field.is_zero(c))
    }

    fn public(&self, field: &Zp64) -> TadicSeries {
        TadicSeries {
            start: self.start,
            coefficients: self
                .coefficients
                .iter()
                .map(|c| field.from_element(c))
                .collect(),
        }
    }

    fn from_public(field: &Zp64, series: &TadicSeries) -> Self {
        let mut series = Self {
            start: series.start,
            coefficients: series
                .coefficients
                .iter()
                .map(|c| field.to_element(*c % field.get_prime()))
                .collect(),
        };
        series.normalize(field);
        series
    }
}

/// A truncated Laurent series modulo one prime, in standard representatives
/// `0..p`: the coefficients of `t^start ..` are known up to (excluding) the
/// absolute precision `start + coefficients.len()`. Leading zeros are
/// stripped, so the first coefficient (if any) is nonzero modulo `p`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TadicSeries {
    pub start: i32,
    pub coefficients: Vec<u64>,
}

impl TadicSeries {
    /// Absolute precision: every order below it is known.
    pub fn precision(&self) -> i32 {
        self.start + self.coefficients.len() as i32
    }

    /// Order of the first coefficient that is nonzero modulo `p`; `None`
    /// when every known coefficient vanishes (the value is `O(t^precision)`).
    /// The exact Laurent coefficient at this order is nonzero over Q (a
    /// proof); the exact coefficients below it vanish modulo `p` (they vanish
    /// over Q unless `p` divides them).
    pub fn valuation(&self) -> Option<i32> {
        self.coefficients
            .iter()
            .position(|c| *c != 0)
            .map(|index| self.start + index as i32)
    }

    /// The coefficient of `t^order` when it is known.
    pub fn coefficient(&self, order: i32) -> Option<u64> {
        if order >= self.precision() {
            return None;
        }
        if order < self.start {
            return Some(0);
        }
        Some(self.coefficients[(order - self.start) as usize])
    }
}

/// The Laurent expansion at `d0` modulo `p` of one exact nonzero coefficient
/// `N / D`, extended on demand: `N(d0 + t) = t^m N'(t)`, `D(d0 + t) = t^k
/// U(t)` with `U(0) != 0 mod p`, and the series of `N'/U` from order
/// `valuation = m - k`.
#[derive(Debug)]
struct Expansion {
    valuation: i32,
    numerator: Vec<Fe>,
    unit: Vec<Fe>,
    unit_inverse: Fe,
    series: Vec<Fe>,
}

impl Expansion {
    /// `None` when `p` divides the leading Taylor coefficient of the
    /// denominator (the prime is bad for this coefficient).
    fn new(
        field: &Zp64,
        d0: Fe,
        numerator: &[Integer],
        denominator: &[Integer],
        (m, k): (u32, u32),
    ) -> Option<Self> {
        let numerator = shifted(field, numerator, d0);
        let denominator = shifted(field, denominator, d0);
        let unit: Vec<Fe> = denominator.get(k as usize..)?.to_vec();
        let lead = *unit.first()?;
        if field.is_zero(&lead) {
            return None;
        }
        debug_assert!(
            numerator
                .iter()
                .take(m as usize)
                .all(|c| field.is_zero(c)),
            "an exact root of the numerator survives modulo p"
        );
        Some(Self {
            valuation: m as i32 - k as i32,
            numerator: numerator.get(m as usize..).unwrap_or_default().to_vec(),
            unit,
            unit_inverse: field.inv(&lead),
            series: vec![],
        })
    }

    fn constant(field: &Zp64, numerator: &Integer, denominator: &Integer) -> Option<Self> {
        let unit = reduce(field, denominator);
        if field.is_zero(&unit) {
            return None;
        }
        Some(Self {
            valuation: 0,
            numerator: vec![reduce(field, numerator)],
            unit: vec![unit],
            unit_inverse: field.inv(&unit),
            series: vec![],
        })
    }

    /// The first `len` coefficients of `N'/U` (from order `valuation`).
    fn ensure(&mut self, field: &Zp64, len: usize) {
        while self.series.len() < len {
            let i = self.series.len();
            let mut value = self.numerator.get(i).copied().unwrap_or(field.zero());
            for j in 1..=i.min(self.unit.len().saturating_sub(1)) {
                value = field.sub(&value, &field.mul(&self.unit[j], &self.series[i - j]));
            }
            self.series.push(field.mul(&value, &self.unit_inverse));
        }
    }

    /// The expansion itself to absolute precision `precision`.
    fn truncated(&mut self, field: &Zp64, precision: i32) -> Series {
        let len = precision - self.valuation;
        if len <= 0 {
            return Series::known_zero(precision);
        }
        self.ensure(field, len as usize);
        let mut series = Series {
            start: self.valuation,
            coefficients: self.series[..len as usize].to_vec(),
        };
        series.normalize(field);
        series
    }

    /// `active * self` to absolute precision `precision`, which the caller's
    /// plan guarantees to be at most `active.precision() + valuation`.
    fn product(
        &mut self,
        field: &Zp64,
        active: &Series,
        precision: i32,
    ) -> Result<Series, ReductionError> {
        let available = active.precision().saturating_add(self.valuation);
        if available < precision {
            return Err(precision_error(format!(
                "a product needs precision {precision} but its factor gives {available} \
                 (inconsistent plan)"
            )));
        }
        let start = active.start + self.valuation;
        let len = precision - start;
        if len <= 0 {
            return Ok(Series::known_zero(precision));
        }
        let len = len as usize;
        self.ensure(field, len);
        let mut out = vec![field.zero(); len];
        for (i, a) in active.coefficients.iter().enumerate().take(len) {
            if field.is_zero(a) {
                continue;
            }
            for (j, c) in self.series[..len - i].iter().enumerate() {
                out[i + j] = field.add(&out[i + j], &field.mul(a, c));
            }
        }
        let mut series = Series {
            start,
            coefficients: out,
        };
        series.normalize(field);
        Ok(series)
    }
}

/// Coefficients of `f(d0 + t)` modulo `p` (repeated synthetic division).
fn shifted(field: &Zp64, polynomial: &[Integer], d0: Fe) -> Vec<Fe> {
    let mut current: Vec<Fe> = polynomial.iter().map(|c| reduce(field, c)).collect();
    let mut out = Vec::with_capacity(current.len());
    while !current.is_empty() {
        let mut carry = field.zero();
        for k in (0..current.len()).rev() {
            carry = field.add(&current[k], &field.mul(&carry, &d0));
            current[k] = carry;
        }
        // current[0] is the remainder, current[1..] the quotient.
        out.push(current.remove(0));
    }
    out
}

/// Per-prime table of the coefficient expansions, built on first use.
struct Table<'t> {
    trace: &'t Trace,
    orders: &'t Orders,
    d0: Fe,
    constants: Vec<Option<Expansion>>,
    polynomials: Vec<Option<Expansion>>,
}

/// Outcome of a coefficient lookup.
enum Lookup<'e> {
    Zero,
    Expansion(&'e mut Expansion),
}

impl<'t> Table<'t> {
    fn new(field: &Zp64, trace: &'t Trace, orders: &'t Orders, d0: i64) -> Self {
        Self {
            trace,
            orders,
            d0: reduce(field, &Integer::from(d0)),
            constants: (0..trace.constants.len()).map(|_| None).collect(),
            polynomials: (0..trace.polynomials.len()).map(|_| None).collect(),
        }
    }

    /// The expansion of `reference`; `Err(BadPrime)` when the prime is bad
    /// for it.
    fn lookup(&mut self, field: &Zp64, reference: CoefRef) -> Result<Lookup<'_>, SeriesError> {
        let index = reference.index();
        if reference.is_constant() {
            if !self.orders.constants[index] {
                return Ok(Lookup::Zero);
            }
            if self.constants[index].is_none() {
                let (numerator, denominator) = &self.trace.constants[index];
                self.constants[index] = Some(
                    Expansion::constant(field, numerator, denominator)
                        .ok_or(SeriesError::BadPrime)?,
                );
            }
            Ok(Lookup::Expansion(
                self.constants[index].as_mut().expect("built above"),
            ))
        } else {
            let Some(orders) = self.orders.polynomials[index] else {
                return Ok(Lookup::Zero);
            };
            if self.polynomials[index].is_none() {
                let rational = &self.trace.polynomials[index];
                self.polynomials[index] = Some(
                    Expansion::new(
                        field,
                        self.d0,
                        &rational.numerator,
                        &rational.denominator,
                        orders,
                    )
                    .ok_or(SeriesError::BadPrime)?,
                );
            }
            Ok(Lookup::Expansion(
                self.polynomials[index].as_mut().expect("built above"),
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Plan: the precision every operation needs (backward pass).
// ---------------------------------------------------------------------------

/// The absolute precision each trace operation must produce (`DEAD16`: its
/// result is never read by an output). For `Pop` it is the precision of the
/// active weight; for `Init`, `New`, `Add` and their terminal forms the
/// precision of the slot value after the operation; for the erasures the
/// precision to which the erased value is checked (`erasure_check_order`).
pub(super) struct Plan {
    need: Vec<i16>,
    /// Operations whose result no output reads (skipped by the replay).
    pub dead_ops: usize,
    /// Largest and smallest live need.
    pub max_need: i32,
    pub min_need: i32,
}

fn to_need(value: i32) -> Result<i16, ReductionError> {
    if value == DEAD {
        return Ok(DEAD16);
    }
    i16::try_from(value)
        .ok()
        .filter(|value| *value != DEAD16)
        .ok_or_else(|| precision_error(format!("a planned precision {value} is out of range")))
}

/// Backward pass: `outputs` are `(terminal slot, target absolute precision)`.
/// Mirrors the polynomial replay's semantics: `Pop` and the erasures TAKE the
/// slot value (the slot is empty afterwards), `New`/`Init` overwrite, `Add`
/// accumulates (an empty slot is an exact zero).
pub(super) fn plan(
    trace: &Trace,
    orders: &Orders,
    outputs: &[(usize, i32)],
    erasure_check_order: Option<i32>,
) -> Result<Plan, ReductionError> {
    let check = erasure_check_order.unwrap_or(DEAD);
    let mut slot_need = vec![DEAD; trace.slots];
    let mut term_need = vec![DEAD; trace.terminal_slots];
    for &(slot, target) in outputs {
        let entry = term_need
            .get_mut(slot)
            .ok_or_else(|| error("t-adic plan: an output names no terminal accumulator"))?;
        *entry = (*entry).max(target);
    }
    let mut need = vec![DEAD16; trace.ops.len()];
    let mut active = DEAD;
    let (mut dead_ops, mut max_need, mut min_need) = (0usize, i32::MIN, i32::MAX);
    for (index, op) in trace.ops.iter().enumerate().rev() {
        if index % (1 << 16) == 0 {
            resources::checkpoint()?;
        }
        let slot = op.slot();
        let value = match op.tag() {
            OpTag::Pop => {
                let value = active;
                // The popped value must be known to the active need; the slot
                // is empty after the pop.
                slot_need[slot] = active;
                active = DEAD;
                value
            }
            OpTag::Init => std::mem::replace(&mut slot_need[slot], DEAD),
            OpTag::New | OpTag::Add | OpTag::TermNew | OpTag::TermAdd => {
                let terminal = matches!(op.tag(), OpTag::TermNew | OpTag::TermAdd);
                let table = if terminal {
                    &mut term_need
                } else {
                    &mut slot_need
                };
                let value = table[slot];
                if value != DEAD
                    && let Some(valuation) = orders.valuation(op.coefficient)
                {
                    active = active.max(value - valuation);
                }
                if matches!(op.tag(), OpTag::New | OpTag::TermNew) {
                    table[slot] = DEAD;
                }
                value
            }
            OpTag::Cancel => {
                slot_need[slot] = check;
                check
            }
            OpTag::TermCancel => {
                term_need[slot] = check;
                check
            }
        };
        if value == DEAD {
            dead_ops += 1;
        } else {
            max_need = max_need.max(value);
            min_need = min_need.min(value);
        }
        need[index] = to_need(value)?;
    }
    if active != DEAD {
        return Err(error(
            "t-adic plan: the trace has products before its first popped weight",
        ));
    }
    Ok(Plan {
        need,
        dead_ops,
        max_need,
        min_need,
    })
}

// ---------------------------------------------------------------------------
// Forward replay modulo one prime.
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum SeriesError {
    /// The prime is bad for a coefficient (unusable prime).
    BadPrime,
    /// Any other failure (precision budget, invariants, cancellation).
    Failed(ReductionError),
}

impl From<ReductionError> for SeriesError {
    fn from(error: ReductionError) -> Self {
        Self::Failed(error)
    }
}

/// A frontier or terminal slot of the series replay.
#[derive(Debug, Default)]
enum Slot {
    /// Never written, taken or erased: an exact zero.
    #[default]
    Empty,
    /// Written by an operation the plan skipped: unknown, must not be read.
    Skipped,
    Value(Series),
}

fn skipped_read() -> SeriesError {
    SeriesError::Failed(precision_error(
        "an operation reads a value the plan skipped (inconsistent plan)",
    ))
}

struct SeriesReplay {
    /// The output terminals' series, in output order.
    outputs: Vec<Series>,
    peak_words: usize,
    max_length: usize,
    erasure_checks: usize,
}

/// Evaluate the series of every output terminal modulo `field`.
fn replay_series(
    trace: &Trace,
    plan: &Plan,
    table: &mut Table<'_>,
    field: &Zp64,
    outputs: &[(usize, i32)],
    max_length: usize,
) -> Result<SeriesReplay, SeriesError> {
    let mut weights: Vec<Slot> = (0..trace.slots).map(|_| Slot::Empty).collect();
    let mut terminals: Vec<Slot> = (0..trace.terminal_slots).map(|_| Slot::Empty).collect();
    let mut active: Option<Series> = None;
    let (mut words, mut peak_words, mut longest, mut erasure_checks) = (0usize, 0usize, 0usize, 0);
    let budget = |series: &Series, longest: &mut usize| -> Result<(), SeriesError> {
        *longest = (*longest).max(series.words());
        if series.words() > max_length {
            return Err(SeriesError::Failed(precision_error(format!(
                "a weight needs {} coefficients, above max_length = {max_length}",
                series.words()
            ))));
        }
        Ok(())
    };
    let slot_words = |slot: &Slot| match slot {
        Slot::Value(series) => series.words(),
        _ => 0,
    };
    for (index, op) in trace.ops.iter().enumerate() {
        if index % CHECKPOINT_OPS == 0 {
            resources::checkpoint().map_err(|e| SeriesError::Failed(e.into()))?;
        }
        let need = plan.need[index];
        let slot = op.slot();
        let tag = op.tag();
        let target = match tag {
            OpTag::Init | OpTag::Pop | OpTag::New | OpTag::Add | OpTag::Cancel => {
                &mut weights[slot]
            }
            OpTag::TermNew | OpTag::TermAdd | OpTag::TermCancel => &mut terminals[slot],
        };
        let before = slot_words(target);
        match tag {
            OpTag::Pop => {
                let taken = std::mem::take(target);
                active = if need == DEAD16 {
                    None
                } else {
                    let mut value = match taken {
                        Slot::Empty => Series::known_zero(i32::from(i16::MAX)),
                        Slot::Value(series) => series,
                        Slot::Skipped => return Err(skipped_read()),
                    };
                    value.truncate(i32::from(need));
                    Some(value)
                };
            }
            OpTag::Cancel | OpTag::TermCancel => {
                if need != DEAD16 {
                    // `erasure_check_order`: the erased value, known to `need`,
                    // must vanish there.
                    match &*target {
                        Slot::Value(series) => {
                            erasure_checks += 1;
                            if series.precision() < i32::from(need) {
                                return Err(SeriesError::Failed(precision_error(
                                    "an erased value is known below its check order",
                                )));
                            }
                            if series.any_nonzero(field) {
                                return Err(SeriesError::Failed(error(
                                    "a key erased as exactly zero in the recording pass has a \
                                     nonzero t-adic coefficient below the check order",
                                )));
                            }
                        }
                        Slot::Empty => erasure_checks += 1,
                        Slot::Skipped => return Err(skipped_read()),
                    }
                }
                *target = Slot::Empty;
            }
            OpTag::Init | OpTag::New | OpTag::TermNew | OpTag::Add | OpTag::TermAdd => {
                if need == DEAD16 {
                    *target = Slot::Skipped;
                } else {
                    let precision = i32::from(need);
                    let product = if tag == OpTag::Init {
                        match table.lookup(field, op.coefficient)? {
                            Lookup::Zero => Series::known_zero(precision),
                            Lookup::Expansion(expansion) => expansion.truncated(field, precision),
                        }
                    } else {
                        let factor = active.as_ref().ok_or_else(skipped_read)?;
                        match table.lookup(field, op.coefficient)? {
                            Lookup::Zero => Series::known_zero(precision),
                            Lookup::Expansion(expansion) => {
                                expansion.product(field, factor, precision)?
                            }
                        }
                    };
                    budget(&product, &mut longest)?;
                    let accumulate = matches!(tag, OpTag::Add | OpTag::TermAdd);
                    *target = match (accumulate, std::mem::take(target)) {
                        (false, _) | (true, Slot::Empty) => Slot::Value(product),
                        (true, Slot::Value(mut sum)) => {
                            sum.add_assign(field, &product);
                            sum.truncate(precision);
                            Slot::Value(sum)
                        }
                        (true, Slot::Skipped) => return Err(skipped_read()),
                    };
                }
            }
        }
        words = words + slot_words(target) - before;
        peak_words = peak_words.max(words + active.as_ref().map_or(0, Series::words));
    }
    let mut series = Vec::with_capacity(outputs.len());
    for &(slot, target) in outputs {
        let value = match std::mem::take(&mut terminals[slot]) {
            Slot::Empty => Series::known_zero(target),
            Slot::Value(value) => value,
            Slot::Skipped => return Err(skipped_read()),
        };
        if value.precision() < target {
            return Err(SeriesError::Failed(precision_error(format!(
                "an output reached precision {} below its target {target} (inconsistent plan)",
                value.precision()
            ))));
        }
        series.push(value);
    }
    Ok(SeriesReplay {
        outputs: series,
        peak_words,
        max_length: longest,
        erasure_checks,
    })
}

// ---------------------------------------------------------------------------
// Driver: primes, erasure certificate, images.
// ---------------------------------------------------------------------------

/// The series of the output terminals modulo one prime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TadicImage {
    pub prime: u64,
    /// One series per output terminal, in output order.
    pub terminals: Vec<TadicSeries>,
}

/// The result of a t-adic request: the live terminals (key order), the
/// absolute precision each was replayed to, and one image per usable prime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TadicTerminals {
    pub d0: i64,
    pub terminals: Vec<IntegralKey>,
    pub targets: Vec<i32>,
    pub images: Vec<TadicImage>,
}

/// Erasure-check points of `prime`, deterministic in `(seed, prime, attempt)`.
fn check_points(field: &Zp64, seed: u64, attempt: usize, count: usize) -> Vec<Fe> {
    let prime = field.get_prime();
    let mut rng = StdRng::seed_from_u64(
        seed ^ CHECK_POINTS_SALT ^ prime.rotate_left(31) ^ (attempt as u64).rotate_left(7),
    );
    let mut used = std::collections::HashSet::new();
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        let value = rng.random_range(2..prime);
        if used.insert(value) {
            points.push(field.to_element(value));
        }
    }
    points
}

/// Certify every recorded erasure at fresh points of `field` (as the sampled
/// arithmetic does at its sample points). `Ok(None)`: the prime is unusable.
/// `Ok(Some((checks, bad_points)))` otherwise.
fn certify_erasures(
    trace: &Trace,
    field: &Zp64,
    seed: u64,
    count: usize,
) -> Result<Option<(usize, usize)>, ReductionError> {
    let mut bad = 0;
    for attempt in 0..CHECK_POINT_ATTEMPTS {
        let points = check_points(field, seed, attempt, count);
        match replay(trace, field, &points) {
            Ok(batch) => {
                let good = batch.good.iter().filter(|good| **good).count();
                bad += points.len() - good;
                if good > 0 {
                    return Ok(Some((batch.cancel_checks * good, bad)));
                }
            }
            Err(ReplayError::BadPrime) => return Ok(None),
            Err(ReplayError::ZeroDecision) => {
                return Err(error(
                    "a key erased as exactly zero in the recording pass is nonzero at a check \
                     point of the t-adic replay",
                ));
            }
            Err(ReplayError::Stopped(stopped)) => return Err(stopped.into()),
        }
    }
    Ok(None)
}

/// Result of one prime.
struct PrimeOutcome {
    image: Option<TadicImage>,
    checks: usize,
    bad_points: usize,
    peak_words: usize,
    max_length: usize,
    seconds: f64,
}

#[allow(clippy::too_many_arguments)] // the inputs of one prime's replay
fn one_prime(
    trace: &Trace,
    orders: &Orders,
    plan: &Plan,
    outputs: &[(usize, i32)],
    options: &TadicOptions,
    seed: u64,
    prime: u64,
) -> Result<PrimeOutcome, ReductionError> {
    let begun = Instant::now();
    let field = Zp64::new(prime);
    let unusable = |seconds: f64, bad_points: usize| PrimeOutcome {
        image: None,
        checks: 0,
        bad_points,
        peak_words: 0,
        max_length: 0,
        seconds,
    };
    let Some((checks, bad_points)) =
        certify_erasures(trace, &field, seed, options.check_points)?
    else {
        return Ok(unusable(begun.elapsed().as_secs_f64(), 0));
    };
    let mut table = Table::new(&field, trace, orders, options.d0);
    match replay_series(trace, plan, &mut table, &field, outputs, options.max_length) {
        Ok(replayed) => Ok(PrimeOutcome {
            image: Some(TadicImage {
                prime,
                terminals: replayed
                    .outputs
                    .iter()
                    .map(|series| series.public(&field))
                    .collect(),
            }),
            checks: checks + replayed.erasure_checks,
            bad_points,
            peak_words: replayed.peak_words,
            max_length: replayed.max_length,
            seconds: begun.elapsed().as_secs_f64(),
        }),
        Err(SeriesError::BadPrime) => Ok(unusable(begun.elapsed().as_secs_f64(), bad_points)),
        Err(SeriesError::Failed(failure)) => Err(failure),
    }
}

/// Replay the trace for `options.primes` usable primes and return the series
/// of the output terminals (`outputs`: `(terminal slot, target precision)`).
pub(super) fn images(
    trace: &Trace,
    outputs: &[(usize, i32)],
    options: &TadicOptions,
    seed: u64,
    max_primes: usize,
    executor: &Executor,
    statistics: &mut ModularStatistics,
) -> Result<Vec<TadicImage>, ReductionError> {
    let prepared = Instant::now();
    let orders = Orders::new(trace, options.d0)?;
    let plan = plan(trace, &orders, outputs, options.erasure_check_order)?;
    statistics.prepare_seconds += prepared.elapsed().as_secs_f64();
    statistics.tadic_dead_ops = plan.dead_ops;
    statistics.tadic_max_need = if plan.max_need == i32::MIN { 0 } else { plan.max_need };
    statistics.tadic_min_need = if plan.min_need == i32::MAX { 0 } else { plan.min_need };
    let mut rng = StdRng::seed_from_u64(seed ^ TADIC_SALT);
    let mut stream = PrimeIteratorU64::new(TADIC_BASE + (rng.random::<u64>() >> 5));
    let mut images = Vec::with_capacity(options.primes);
    let mut consumed = 0usize;
    let started = Instant::now();
    while images.len() < options.primes {
        let wanted = options.primes - images.len();
        if consumed + wanted > max_primes {
            return Err(error(format!(
                "the t-adic replay found {} usable primes within max_primes = {max_primes}",
                images.len()
            )));
        }
        let mut primes = Vec::with_capacity(wanted);
        for _ in 0..wanted {
            primes.push(stream.next().ok_or_else(|| error("exhausted 64-bit primes"))?);
        }
        consumed += wanted;
        let (trace, orders, plan) = (trace, &orders, &plan);
        let outcomes = executor.map(primes, |prime| {
            one_prime(trace, orders, plan, outputs, options, seed, prime)
        })?;
        for outcome in outcomes {
            let outcome = outcome?;
            statistics.replay_cpu_seconds += outcome.seconds;
            statistics.cancel_checks += outcome.checks;
            statistics.bad_points += outcome.bad_points;
            statistics.peak_weight_words = statistics.peak_weight_words.max(outcome.peak_words);
            statistics.tadic_max_length = statistics.tadic_max_length.max(outcome.max_length);
            match outcome.image {
                Some(image) => {
                    statistics.primes += 1;
                    images.push(image);
                }
                None => statistics.unusable_primes += 1,
            }
        }
    }
    statistics.replay_seconds += started.elapsed().as_secs_f64();
    statistics.rounds += 1;
    Ok(images)
}

// ---------------------------------------------------------------------------
// Exact coefficients as series (cross-check, composition after the replay).
// ---------------------------------------------------------------------------

/// The Laurent expansion at `d0` modulo `prime` of an exact coefficient, to
/// absolute precision `precision`; `Ok(None)` when `prime` is bad for it.
/// The zero coefficient gives `O(t^precision)`.
pub fn expand_coefficient(
    coefficient: &Coefficient,
    d0: i64,
    prime: u64,
    precision: i32,
) -> Result<Option<TadicSeries>, ReductionError> {
    let field = Zp64::new(prime);
    let dense = DenseRational::from_coefficient(coefficient);
    let Some(orders) = multiplicities(&dense, &Integer::from(d0))? else {
        return Ok(Some(TadicSeries {
            start: precision,
            coefficients: vec![],
        }));
    };
    let d0 = reduce(&field, &Integer::from(d0));
    let Some(mut expansion) =
        Expansion::new(&field, d0, &dense.numerator, &dense.denominator, orders)
    else {
        return Ok(None);
    };
    Ok(Some(expansion.truncated(&field, precision).public(&field)))
}

/// `series * coefficient` modulo `prime` (exact coefficient, expanded at
/// `d0`): the precision moves by the coefficient's valuation over Q.
/// `Ok(None)` when `prime` is bad for the coefficient.
pub fn scale_series(
    series: &TadicSeries,
    coefficient: &Coefficient,
    d0: i64,
    prime: u64,
) -> Result<Option<TadicSeries>, ReductionError> {
    let field = Zp64::new(prime);
    let dense = DenseRational::from_coefficient(coefficient);
    let Some(orders) = multiplicities(&dense, &Integer::from(d0))? else {
        // An exact zero factor: the product is exactly zero; report it to the
        // factor's precision.
        return Ok(Some(TadicSeries {
            start: series.precision(),
            coefficients: vec![],
        }));
    };
    let d0 = reduce(&field, &Integer::from(d0));
    let Some(mut expansion) =
        Expansion::new(&field, d0, &dense.numerator, &dense.denominator, orders)
    else {
        return Ok(None);
    };
    let active = Series::from_public(&field, series);
    let precision = active.precision() + expansion.valuation;
    Ok(Some(expansion.product(&field, &active, precision)?.public(&field)))
}

/// `a + b` modulo `prime`; the precision is the smaller one.
pub fn add_series(a: &TadicSeries, b: &TadicSeries, prime: u64) -> TadicSeries {
    let field = Zp64::new(prime);
    let mut sum = Series::from_public(&field, a);
    sum.add_assign(&field, &Series::from_public(&field, b));
    sum.public(&field)
}

#[cfg(test)]
mod tests {
    use super::super::{CONSTANT_FLAG, Op};
    use super::*;
    use crate::algebra::CoefficientContext;

    const P: u64 = 1_000_000_007;

    fn field() -> Zp64 {
        Zp64::new(P)
    }

    fn fe(value: i64) -> Fe {
        field().to_element(value.rem_euclid(P as i64) as u64)
    }

    fn u(value: i64) -> u64 {
        value.rem_euclid(P as i64) as u64
    }

    /// `numerator / denominator` modulo P as a standard representative.
    fn q(numerator: i64, denominator: i64) -> u64 {
        let f = field();
        f.from_element(&f.div(&fe(numerator), &fe(denominator)))
    }

    fn ints(values: &[i64]) -> Vec<Integer> {
        values.iter().map(|v| Integer::from(*v)).collect()
    }

    #[test]
    fn root_multiplicities_are_exact_over_the_integers() {
        let four = Integer::from(4);
        // (d-4)^2 (d+1) = d^3 - 7 d^2 + 8 d + 16
        assert_eq!(root_multiplicity(&ints(&[16, 8, -7, 1]), &four), Some(2));
        // d - 3
        assert_eq!(root_multiplicity(&ints(&[-3, 1]), &four), Some(0));
        assert_eq!(root_multiplicity(&ints(&[0, 0]), &four), None);
        assert_eq!(root_multiplicity(&ints(&[5]), &four), Some(0));
        // (2d - 8)^3 = 8 (d-4)^3
        assert_eq!(
            root_multiplicity(&ints(&[-512, 384, -96, 8]), &four),
            Some(3)
        );
    }

    #[test]
    fn coefficients_expand_with_their_exact_valuation() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = |text: &str| context.coefficient_fixture(text);
        // 1/(d-4) = t^-1
        let s = expand_coefficient(&c("1/(d-4)"), 4, P, 3).unwrap().unwrap();
        assert_eq!(s.start, -1);
        assert_eq!(s.precision(), 3);
        assert_eq!(s.coefficients, vec![1, 0, 0, 0]);
        // (d-3)/(d-4)^2 = (1+t)/t^2
        let s = expand_coefficient(&c("(d-3)/(d-4)^2"), 4, P, 1).unwrap().unwrap();
        assert_eq!((s.start, s.coefficients.clone()), (-2, vec![1, 1, 0]));
        assert_eq!(s.valuation(), Some(-2));
        // 1/(d-2) = 1/(2+t) = 1/2 - t/4 + t^2/8
        let s = expand_coefficient(&c("1/(d-2)"), 4, P, 3).unwrap().unwrap();
        assert_eq!(s.coefficients, vec![q(1, 2), q(-1, 4), q(1, 8)]);
        // (d-4)^2/(d+1) = t^2/(5+t) = t^2/5 - t^3/25 + ...
        let s = expand_coefficient(&c("(d-4)^2/(d+1)"), 4, P, 4).unwrap().unwrap();
        assert_eq!((s.start, s.coefficients.clone()), (2, vec![q(1, 5), q(-1, 25)]));
        // Below the valuation nothing is known but zeros: O(t^1).
        let s = expand_coefficient(&c("(d-4)^2/(d+1)"), 4, P, 1).unwrap().unwrap();
        assert_eq!((s.start, s.coefficients.len()), (1, 0));
        assert_eq!(s.valuation(), None);
        // A prime dividing the leading Taylor coefficient of the denominator
        // is bad: (d - 4 - P) at d0 = 4 has U(0) = -P.
        let bad = c(&format!("1/(d-{})", 4 + P));
        assert_eq!(expand_coefficient(&bad, 4, P, 2).unwrap(), None);
        assert!(expand_coefficient(&bad, 4, 1_000_000_009, 2).unwrap().is_some());
    }

    #[test]
    fn series_arithmetic_tracks_the_absolute_precision() {
        let f = field();
        let a = Series {
            start: 0,
            coefficients: vec![fe(1), fe(2), fe(3)],
        };
        assert_eq!(a.precision(), 3);
        // a / t^2: precision 3 - 2 = 1, starting at t^-2.
        let mut pole = Expansion::constant(&f, &Integer::from(1), &Integer::from(1)).unwrap();
        pole.valuation = -2;
        let product = pole.product(&f, &a, 1).unwrap();
        assert_eq!(product.start, -2);
        assert_eq!(product.precision(), 1);
        // Asking for more than the factor carries is a plan error, never a
        // silent truncation.
        let error = pole.product(&f, &a, 2).unwrap_err();
        assert!(is_precision_failure(&error), "{error}");
        // Sums take the smaller precision; a cancellation of the leading
        // terms raises the valuation, not the precision.
        let mut s = Series {
            start: 0,
            coefficients: vec![fe(1), fe(1), fe(5)],
        };
        s.add_assign(
            &f,
            &Series {
                start: 0,
                coefficients: vec![fe(-1), fe(-1)],
            },
        );
        assert_eq!((s.start, s.precision()), (2, 2));
        assert!(s.coefficients.is_empty(), "O(t^2), not zero");
        // O(t^2) + (t^-1 + O(t)) = t^-1 + O(t)
        s.add_assign(
            &f,
            &Series {
                start: -1,
                coefficients: vec![fe(1), fe(0)],
            },
        );
        assert_eq!((s.start, s.precision()), (-1, 1));
        assert_eq!(s.public(&f).valuation(), Some(-1));
    }

    fn constant(index: u32) -> CoefRef {
        CoefRef(index | CONSTANT_FLAG)
    }

    fn polynomial(index: u32) -> CoefRef {
        CoefRef(index)
    }

    fn dense(numerator: &[i64], denominator: &[i64]) -> DenseRational {
        DenseRational {
            numerator: ints(numerator),
            denominator: ints(denominator),
        }
    }

    /// Two paths with opposite spurious poles: w1 = 1/(d-4), w2 = 1 -
    /// 1/(d-4); terminal T = (d-2) (w1 + w2) = d - 2 = 2 + t. Each path has
    /// a pole, the composed coefficient has none; the target precision is
    /// reached exactly through the plan.
    fn cancelling_trace() -> Trace {
        let ops = vec![
            Op::new(OpTag::Init, 0, constant(0)), // w0 = 1
            Op::new(OpTag::Pop, 0, constant(0)),
            Op::new(OpTag::New, 1, polynomial(0)), // w1 = 1/(d-4)
            Op::new(OpTag::New, 2, polynomial(1)), // w2 = (d-5)/(d-4)
            Op::new(OpTag::Pop, 1, constant(0)),
            Op::new(OpTag::TermNew, 0, polynomial(2)), // T = (d-2) w1
            Op::new(OpTag::Pop, 2, constant(0)),
            Op::new(OpTag::TermAdd, 0, polynomial(2)), // T += (d-2) w2
        ];
        Trace {
            ops,
            constants: vec![(Integer::from(1), Integer::from(1))],
            polynomials: vec![
                dense(&[1], &[-4, 1]),
                dense(&[-5, 1], &[-4, 1]),
                dense(&[-2, 1], &[1]),
            ],
            slots: 3,
            terminal_slots: 1,
            phase_starts: vec![],
        }
    }

    #[test]
    fn spurious_poles_cancel_with_a_proved_precision() {
        let trace = cancelling_trace();
        let orders = Orders::new(&trace, 4).unwrap();
        let outputs = [(0usize, 3i32)];
        let plan = plan(&trace, &orders, &outputs, None).unwrap();
        // The terminal products need 3; the popped w1, w2 need 3 - 0 = 3;
        // the products that make them need 3 and 3; the input needs 3 + 1 = 4.
        assert_eq!(plan.need[7], 3);
        assert_eq!(plan.need[5], 3);
        assert_eq!(plan.need[1], 4);
        assert_eq!(plan.need[0], 4);
        let f = field();
        let mut table = Table::new(&f, &trace, &orders, 4);
        let replayed = replay_series(&trace, &plan, &mut table, &f, &outputs, 64).unwrap();
        let t = replayed.outputs[0].public(&f);
        assert_eq!(t.precision(), 3);
        assert_eq!(t.valuation(), Some(0));
        assert_eq!(t.coefficients, vec![2, 1, 0]);
    }

    #[test]
    fn dead_values_are_skipped_and_never_read() {
        let mut trace = cancelling_trace();
        // A second terminal nobody asks for: its products are dead.
        trace.ops.push(Op::new(OpTag::Pop, 0, constant(0)));
        trace.terminal_slots = 2;
        let orders = Orders::new(&trace, 4).unwrap();
        let plan = plan(&trace, &orders, &[(0, 1)], None).unwrap();
        assert_eq!(plan.need[8], DEAD16);
        assert!(plan.dead_ops >= 1);
    }

    #[test]
    fn the_images_driver_returns_one_series_per_prime() {
        let trace = cancelling_trace();
        let executor = Executor::new(1).unwrap();
        let mut statistics = ModularStatistics::default();
        let options = TadicOptions {
            primes: 2,
            ..TadicOptions::default()
        };
        let images = images(&trace, &[(0, 2)], &options, 7, 16, &executor, &mut statistics)
            .unwrap();
        assert_eq!(images.len(), 2);
        assert_ne!(images[0].prime, images[1].prime);
        for image in &images {
            assert_eq!(image.terminals[0].coefficients, vec![2, 1]);
            assert_eq!(image.terminals[0].precision(), 2);
        }
        assert_eq!(statistics.primes, 2);
    }

    /// Seeded corruption control: a recorded erasure of a nonzero weight is
    /// caught by the erasure certificate (check points), and a corrupted
    /// coefficient changes the series.
    #[test]
    fn corrupted_traces_are_caught_or_change_the_result() {
        let mut trace = cancelling_trace();
        // Erase w2 although it is nonzero.
        trace.ops.insert(4, Op::new(OpTag::Cancel, 2, constant(0)));
        let executor = Executor::new(1).unwrap();
        let mut statistics = ModularStatistics::default();
        let error = images(
            &trace,
            &[(0, 2)],
            &TadicOptions::default(),
            7,
            16,
            &executor,
            &mut statistics,
        )
        .unwrap_err();
        assert!(error.to_string().contains("erased"), "{error}");
        // Corrupt one coefficient: (d-5)/(d-4) -> (d-6)/(d-4).
        let mut trace = cancelling_trace();
        trace.polynomials[1] = dense(&[-6, 1], &[-4, 1]);
        let images = images(
            &trace,
            &[(0, 1)],
            &TadicOptions::default(),
            7,
            16,
            &executor,
            &mut statistics,
        )
        .unwrap();
        // T = (d-2)(1 + (1 - 2)/(d-4))... now has a pole: -2/t - 1 + ...
        assert_eq!(images[0].terminals[0].valuation(), Some(-1));
        assert_eq!(images[0].terminals[0].coefficients[0], u(-2));
    }

    #[test]
    fn scaling_and_adding_public_series() {
        let context = CoefficientContext::try_new(["d"]).unwrap();
        let c = |text: &str| context.coefficient_fixture(text);
        let s = TadicSeries {
            start: 0,
            coefficients: vec![1, 1, 1],
        };
        // (1 + t + t^2 + O(t^3)) / (d-4) = t^-1 + 1 + t + O(t^2)
        let scaled = scale_series(&s, &c("1/(d-4)"), 4, P).unwrap().unwrap();
        assert_eq!((scaled.start, scaled.precision()), (-1, 2));
        assert_eq!(scaled.coefficients, vec![1, 1, 1]);
        let sum = add_series(
            &scaled,
            &TadicSeries {
                start: -1,
                coefficients: vec![u(-1)],
            },
            P,
        );
        // the pole cancels; precision min(2, 0) = 0
        assert_eq!((sum.start, sum.precision()), (0, 0));
        assert_eq!(sum.valuation(), None);
    }
}
