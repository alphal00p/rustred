//! Finite-field weight backends for the Ordered scheduler.
//!
//! Both modular arithmetics run the scheduler once with [`ModularRecorder`]
//! as its weight arithmetic. The recorder carries `PROBES` values of every
//! weight modulo one probe prime, which are used only for the exact-zero
//! erasure decision, and records a straight-line trace of the weight
//! propagation:
//!
//! - `Init(slot, c)`: a frontier slot starts at an input coefficient;
//! - `Pop(slot)`: the popped key's weight becomes the active weight;
//! - `New/Add(slot, c)`: `slot (=|+=) active * c` for a child;
//! - `Cancel(slot)`: the merged weight of `slot` was decided to be zero and
//!   the key was erased (every evaluation verifies that it is zero);
//! - `TermNew/TermAdd/TermCancel(t, c)`: the same for terminal outputs.
//!
//! Coefficients `c` are the exact step coefficients produced by the adapters,
//! interned by value and validated against the request's context and limits
//! once per distinct value, when first interned. The trace is then evaluated
//! modulo 64-bit primes, and the per-prime images are lifted over Q by the
//! driver shared by both arithmetics (`lift.rs`):
//!
//! - `ModularSampled` replays it at sample points and interpolates every
//!   terminal per prime (`sampled.rs`, `replay.rs`);
//! - `ModularPolynomial` replays it with weights in `Z_p(d)`, several primes
//!   per replay (`polynomial/`).
//!
//! All parallel work runs on the request's executor (`executor.rs`): inline
//! for one thread, else a private pool.
mod executor;
#[cfg(test)]
pub(crate) use executor::pools_built_on_this_thread;
mod field;
mod lift;
mod polynomial;
mod replay;
mod sampled;

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::time::Instant;

use hashbrown::HashTable;
use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::prelude::Integer;

use self::executor::Executor;
use self::field::{DenseRational, Fe, reduce};
use self::polynomial::{BasisBuilder, PolynomialSource};
use self::sampled::SampledSource;
use super::OrderedWeights;
use super::arithmetic::{
    ModularCommon, OrderedWeightArithmetic, PolynomialOptions, SampledOptions,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionLimits};

/// Probe values per weight used only for zero decisions in the recording pass.
pub(super) const PROBES: usize = 2;

pub(super) fn error(detail: impl Into<String>) -> ReductionError {
    ReductionError::ModularWeights {
        detail: detail.into(),
    }
}

/// The modular arithmetics interpolate in at most one variable.
pub(super) fn check_context(context: &CoefficientContext) -> Result<(), ReductionError> {
    if context.variables().len() > 1 {
        return Err(error(
            "modular Ordered weights require a coefficient context with at most one variable",
        ));
    }
    Ok(())
}

/// How the recorded trace is evaluated modulo each prime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lifter {
    /// Replays at sample points and per-prime interpolation.
    Sampled(SampledOptions),
    /// Replays over `Z_p(d)`.
    Polynomial(PolynomialOptions),
}

impl Lifter {
    /// The common options and evaluation of a modular arithmetic.
    pub(crate) fn of(arithmetic: OrderedWeightArithmetic) -> Option<(ModularCommon, Self)> {
        match arithmetic {
            OrderedWeightArithmetic::Exact => None,
            OrderedWeightArithmetic::ModularSampled(common, sampled) => {
                Some((common, Self::Sampled(sampled)))
            }
            OrderedWeightArithmetic::ModularPolynomial(common, polynomial) => {
                Some((common, Self::Polynomial(polynomial)))
            }
        }
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::Sampled(options) => options.validate(),
            Self::Polynomial(options) => options.validate(),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Sampled(_) => "modular_sampled",
            Self::Polynomial(_) => "modular_polynomial",
        }
    }
}

/// Interned coefficient reference; the top bit marks a constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CoefRef(u32);

const CONSTANT_FLAG: u32 = 1 << 31;

impl CoefRef {
    #[inline(always)]
    pub fn is_constant(self) -> bool {
        self.0 & CONSTANT_FLAG != 0
    }
    #[inline(always)]
    pub fn index(self) -> usize {
        (self.0 & !CONSTANT_FLAG) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum OpTag {
    Init = 0,
    Pop = 1,
    New = 2,
    Add = 3,
    Cancel = 4,
    TermNew = 5,
    TermAdd = 6,
    TermCancel = 7,
}

const SLOT_BITS: u32 = 29;
const SLOT_MASK: u32 = (1 << SLOT_BITS) - 1;

#[derive(Clone, Copy, Debug)]
pub(super) struct Op {
    word: u32,
    pub coefficient: CoefRef,
}

impl Op {
    fn new(tag: OpTag, slot: u32, coefficient: CoefRef) -> Self {
        Self {
            word: ((tag as u32) << SLOT_BITS) | slot,
            coefficient,
        }
    }
    #[inline(always)]
    pub fn tag(self) -> OpTag {
        match self.word >> SLOT_BITS {
            0 => OpTag::Init,
            1 => OpTag::Pop,
            2 => OpTag::New,
            3 => OpTag::Add,
            4 => OpTag::Cancel,
            5 => OpTag::TermNew,
            6 => OpTag::TermAdd,
            _ => OpTag::TermCancel,
        }
    }
    #[inline(always)]
    pub fn slot(self) -> usize {
        (self.word & SLOT_MASK) as usize
    }
}

fn statistics_requested() -> bool {
    std::env::var_os("RUSTRED_ORDERED_MODULAR_STATS").is_some()
}

impl Trace {
    fn digest(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash};
        let mut hasher = DefaultHasher::new();
        for op in &self.ops {
            (op.word, op.coefficient.0).hash(&mut hasher);
        }
        self.constants.hash(&mut hasher);
        for polynomial in &self.polynomials {
            (&polynomial.numerator, &polynomial.denominator).hash(&mut hasher);
        }
        (self.slots, self.terminal_slots).hash(&mut hasher);
        hasher.finish()
    }
}

/// The weight-independent numeric program of one Ordered request.
pub(super) struct Trace {
    ops: Vec<Op>,
    /// Rational constants `(numerator, denominator)`.
    constants: Vec<(Integer, Integer)>,
    /// Non-constant univariate rational functions.
    polynomials: Vec<DenseRational>,
    /// Peak number of simultaneously live frontier slots.
    slots: usize,
    /// Terminal accumulators (including cancelled ones).
    terminal_slots: usize,
    /// `(phase, index of its first op)` at every phase start, in order
    /// (diagnostics only; not part of the digest).
    phase_starts: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ModularWeight {
    slot: u32,
    probe: [Fe; PROBES],
}

pub(crate) struct ModularContribution {
    coefficient: CoefRef,
    probe: [Fe; PROBES],
    zero: bool,
}

#[derive(Default, Clone, Copy)]
struct FxBuild;

struct FxHasher(u64);

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }
    #[inline]
    fn write_u64(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    #[inline]
    fn write_u32(&mut self, value: u32) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn write_i64(&mut self, value: i64) {
        self.write_u64(value as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

impl BuildHasher for FxBuild {
    type Hasher = FxHasher;
    fn build_hasher(&self) -> FxHasher {
        FxHasher(0)
    }
}

/// Statistics of one modular request (recording plus reconstruction).
///
/// The recording counters, the consumed primes and the degrees and sizes of
/// the result are independent of the number of threads; the timings, the
/// rounds, the work computed ahead of need (`discarded_images` and the replay
/// counters of the images behind them) and the thread counts are not.
#[derive(Clone, Debug, Default)]
pub(crate) struct ModularStatistics {
    pub arithmetic: &'static str,
    pub ops: usize,
    pub constants: usize,
    pub polynomials: usize,
    pub peak_slots: usize,
    pub terminal_slots: usize,
    pub output_terminals: usize,
    pub coefficient_lookups: usize,
    pub record_seconds: f64,
    pub prepare_seconds: f64,
    pub replay_seconds: f64,
    pub replay_cpu_seconds: f64,
    pub interpolation_seconds: f64,
    pub lift_seconds: f64,
    /// Requested worker threads, threads of the private pool (0 = inline),
    /// distinct threads that ran jobs, and whether all jobs ran on the caller.
    pub threads: usize,
    pub pool_threads: usize,
    pub job_threads: usize,
    pub jobs_on_caller_only: bool,
    /// Image rounds requested from the image source.
    pub rounds: usize,
    pub batches: usize,
    /// Sample points replayed (reconstruction and verification).
    pub points_replayed: usize,
    /// Usable reconstruction images consumed by the lift.
    pub primes: usize,
    /// Fresh verification primes consumed.
    pub verification_primes: usize,
    /// Primes the image source could not use.
    pub unusable_primes: usize,
    /// Images computed ahead of need and never consumed.
    pub discarded_images: usize,
    pub points_first_prime: usize,
    pub points_per_prime: usize,
    pub max_numerator_degree: usize,
    pub max_denominator_degree: usize,
    pub max_coefficient_bits: usize,
    /// Consumed images outside the winning signature group.
    pub unlucky_images: usize,
    pub bad_points: usize,
    pub verification_failures: usize,
    pub cancel_checks: usize,
    /// `ModularPolynomial`: factor basis of the step-coefficient denominators
    /// (all, linear) and the time to factor them.
    pub basis_factors: usize,
    pub basis_linear_factors: usize,
    pub factor_seconds: f64,
    /// `ModularPolynomial`: replay jobs, primes they replayed, and the
    /// largest resident weight storage of one job (field elements; counted
    /// only when statistics are reported, else 0).
    pub passes: usize,
    pub lanes_replayed: usize,
    pub peak_weight_words: usize,
    /// `ModularPolynomial`: replay CPU seconds per scheduler phase (summed
    /// over jobs; phase `None` = input admission before the first phase).
    pub replay_phase_cpu_seconds: Vec<(Option<usize>, f64)>,
    /// `ModularPolynomial`: per phase, the field elements (per lane) of the
    /// active weight summed over its products: the replay's op volume.
    pub replay_phase_volume: Vec<(Option<usize>, f64)>,
    /// Digest of the recorded trace (ops with their slots and coefficient
    /// references, and the coefficient tables in interning order), computed
    /// only when statistics are reported. Equal digests mean identical
    /// replays.
    pub trace_digest: Option<u64>,
}

impl ModularStatistics {
    pub(crate) fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\"arithmetic\":\"{}\",\"ops\":{},\"constants\":{},\"polynomials\":{},",
                "\"peak_slots\":{},\"terminal_slots\":{},\"output_terminals\":{},",
                "\"coefficient_lookups\":{},",
                "\"record_seconds\":{:.6},\"prepare_seconds\":{:.6},\"replay_seconds\":{:.6},",
                "\"replay_cpu_seconds\":{:.6},\"interpolation_seconds\":{:.6},\"lift_seconds\":{:.6},",
                "\"threads\":{},\"pool_threads\":{},\"job_threads\":{},\"jobs_on_caller_only\":{},",
                "\"rounds\":{},\"batches\":{},\"points_replayed\":{},\"primes\":{},",
                "\"verification_primes\":{},\"unusable_primes\":{},\"discarded_images\":{},",
                "\"points_first_prime\":{},\"points_per_prime\":{},",
                "\"max_numerator_degree\":{},\"max_denominator_degree\":{},",
                "\"max_coefficient_bits\":{},\"unlucky_images\":{},\"bad_points\":{},",
                "\"verification_failures\":{},\"cancel_checks\":{},",
                "\"basis_factors\":{},\"basis_linear_factors\":{},\"factor_seconds\":{:.6},",
                "\"passes\":{},\"lanes_replayed\":{},\"peak_weight_words\":{},\"trace_digest\":{},",
                "\"replay_phase_cpu_seconds\":[{}],\"replay_phase_volume\":[{}]}}"
            ),
            self.arithmetic,
            self.ops,
            self.constants,
            self.polynomials,
            self.peak_slots,
            self.terminal_slots,
            self.output_terminals,
            self.coefficient_lookups,
            self.record_seconds,
            self.prepare_seconds,
            self.replay_seconds,
            self.replay_cpu_seconds,
            self.interpolation_seconds,
            self.lift_seconds,
            self.threads,
            self.pool_threads,
            self.job_threads,
            self.jobs_on_caller_only,
            self.rounds,
            self.batches,
            self.points_replayed,
            self.primes,
            self.verification_primes,
            self.unusable_primes,
            self.discarded_images,
            self.points_first_prime,
            self.points_per_prime,
            self.max_numerator_degree,
            self.max_denominator_degree,
            self.max_coefficient_bits,
            self.unlucky_images,
            self.bad_points,
            self.verification_failures,
            self.cancel_checks,
            self.basis_factors,
            self.basis_linear_factors,
            self.factor_seconds,
            self.passes,
            self.lanes_replayed,
            self.peak_weight_words,
            self.trace_digest
                .map_or("null".into(), |digest| format!("\"{digest:016x}\"")),
            self.replay_phase_cpu_seconds
                .iter()
                .map(|(phase, seconds)| format!(
                    "[{},{seconds:.6}]",
                    phase.map_or("null".into(), |phase| phase.to_string())
                ))
                .collect::<Vec<_>>()
                .join(","),
            self.replay_phase_volume
                .iter()
                .map(|(phase, volume)| format!(
                    "[{},{volume:.0}]",
                    phase.map_or("null".into(), |phase| phase.to_string())
                ))
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

/// Recording weight backend.
pub(crate) struct ModularRecorder {
    common: ModularCommon,
    lifter: Lifter,
    context: CoefficientContext,
    limits: ExactAlgebraLimits,
    probe_field: Zp64,
    probe_points: [Fe; PROBES],
    table: HashTable<u32>,
    entries: Vec<Coefficient>,
    entry_refs: Vec<CoefRef>,
    entry_hashes: Vec<u64>,
    constant_probe: Vec<Fe>,
    polynomial_probe: Vec<[Fe; PROBES]>,
    constant_count: u32,
    polynomial_count: u32,
    ops: Vec<Op>,
    phase_starts: Vec<(usize, usize)>,
    free_slots: Vec<u32>,
    next_slot: u32,
    terminal_slots: u32,
    lookups: usize,
    started: Instant,
}

impl ModularRecorder {
    pub(crate) fn try_new(
        context: &CoefficientContext,
        limits: ReductionLimits,
        common: ModularCommon,
        lifter: Lifter,
    ) -> Result<Self, ReductionError> {
        common.validate().map_err(error)?;
        lifter.validate().map_err(error)?;
        check_context(context)?;
        // Probe primes come from a range disjoint from reconstruction primes.
        let mut rng = StdRng::seed_from_u64(common.seed ^ 0x7072_6f62_6573);
        let start = (1u64 << 61) + (rng.random::<u64>() >> 4);
        let prime = PrimeIteratorU64::new(start)
            .next()
            .ok_or_else(|| error("no probe prime"))?;
        let probe_field = Zp64::new(prime);
        let probe_points =
            std::array::from_fn(|_| probe_field.to_element(rng.random_range(2..prime)));
        Ok(Self {
            common,
            lifter,
            context: context.clone(),
            limits: limits.exact_algebra,
            probe_field,
            probe_points,
            table: HashTable::new(),
            entries: Vec::new(),
            entry_refs: Vec::new(),
            entry_hashes: Vec::new(),
            constant_probe: Vec::new(),
            polynomial_probe: Vec::new(),
            constant_count: 0,
            polynomial_count: 0,
            ops: Vec::new(),
            phase_starts: Vec::new(),
            free_slots: Vec::new(),
            next_slot: 0,
            terminal_slots: 0,
            lookups: 0,
            started: Instant::now(),
        })
    }

    fn intern(&mut self, coefficient: &Coefficient) -> Result<CoefRef, ReductionError> {
        self.lookups += 1;
        let hash = FxBuild.hash_one(coefficient);
        let entries = &self.entries;
        if let Some(&index) = self
            .table
            .find(hash, |&index| entries[index as usize] == *coefficient)
        {
            return Ok(self.entry_refs[index as usize]);
        }
        // Validation depends only on the value, so one check per distinct
        // coefficient covers every occurrence.
        self.context
            .validate_with_limits(coefficient, self.limits)
            .map_err(ReductionError::from)?;
        let dense = DenseRational::from_coefficient(coefficient);
        let values = self.evaluate_probe(&dense)?;
        let reference = if dense.is_constant() {
            self.constant_probe.push(values[0]);
            let reference = CoefRef(self.constant_count | CONSTANT_FLAG);
            self.constant_count += 1;
            if self.constant_count >= CONSTANT_FLAG {
                return Err(error("too many distinct constant coefficients"));
            }
            reference
        } else {
            self.polynomial_probe.push(values);
            let reference = CoefRef(self.polynomial_count);
            self.polynomial_count += 1;
            if self.polynomial_count >= CONSTANT_FLAG {
                return Err(error("too many distinct polynomial coefficients"));
            }
            reference
        };
        let index = u32::try_from(self.entries.len())
            .map_err(|_| error("too many distinct coefficients"))?;
        self.entries.push(coefficient.clone());
        self.entry_refs.push(reference);
        self.entry_hashes.push(hash);
        let hashes = &self.entry_hashes;
        self.table
            .insert_unique(hash, index, |&i| hashes[i as usize]);
        Ok(reference)
    }

    /// The probe values of an exact coefficient: its value at the probe
    /// points modulo the probe prime.
    fn evaluate_probe(&self, dense: &DenseRational) -> Result<[Fe; PROBES], ReductionError> {
        let field = &self.probe_field;
        if dense.is_constant() {
            let numerator = reduce(field, &dense.numerator[0]);
            let denominator = reduce(field, &dense.denominator[0]);
            if field.is_zero(&denominator) {
                return Err(error(
                    "probe prime divides a constant coefficient denominator",
                ));
            }
            return Ok([field.div(&numerator, &denominator); PROBES]);
        }
        let numerator: Vec<Fe> = dense.numerator.iter().map(|c| reduce(field, c)).collect();
        let denominator: Vec<Fe> = dense.denominator.iter().map(|c| reduce(field, c)).collect();
        let mut values = [field.zero(); PROBES];
        for (value, x) in values.iter_mut().zip(self.probe_points) {
            let den = field::horner(field, &denominator, x);
            if field.is_zero(&den) {
                return Err(error(
                    "a probe point is a pole of a step coefficient; change the seed",
                ));
            }
            *value = field.div(&field::horner(field, &numerator, x), &den);
        }
        Ok(values)
    }

    #[inline]
    fn probe(&self, reference: CoefRef) -> [Fe; PROBES] {
        if reference.is_constant() {
            [self.constant_probe[reference.index()]; PROBES]
        } else {
            self.polynomial_probe[reference.index()]
        }
    }

    fn allocate(&mut self) -> Result<u32, ReductionError> {
        if let Some(slot) = self.free_slots.pop() {
            return Ok(slot);
        }
        let slot = self.next_slot;
        if slot > SLOT_MASK {
            return Err(error("too many simultaneously live frontier slots"));
        }
        self.next_slot += 1;
        Ok(slot)
    }

    fn scaled(&self, weight: &ModularWeight, reference: CoefRef) -> [Fe; PROBES] {
        let values = self.probe(reference);
        std::array::from_fn(|i| self.probe_field.mul(&weight.probe[i], &values[i]))
    }

    fn add_probe(&self, existing: &mut ModularWeight, contribution: &ModularContribution) -> bool {
        for (value, added) in existing.probe.iter_mut().zip(contribution.probe) {
            *value = self.probe_field.add(&*value, &added);
        }
        existing
            .probe
            .iter()
            .all(|value| self.probe_field.is_zero(value))
    }

    /// Evaluate the trace modulo primes and lift the live terminal
    /// accumulators over Q.
    pub(crate) fn finish(
        self,
        terms: BTreeMap<IntegralKey, ModularWeight>,
    ) -> Result<(BTreeMap<IntegralKey, Coefficient>, ModularStatistics), ReductionError> {
        let record_seconds = self.started.elapsed().as_secs_f64();
        let prepare = Instant::now();
        let constant_total = self.constant_count as usize;
        let polynomial_total = self.polynomial_count as usize;
        let mut constants = vec![(Integer::zero(), Integer::one()); constant_total];
        let mut polynomials = vec![
            DenseRational {
                numerator: vec![],
                denominator: vec![],
            };
            polynomial_total
        ];
        // The polynomial arithmetic factors every polynomial coefficient's
        // denominator over one basis, in reference order.
        let mut factored: Vec<Option<&Coefficient>> = vec![];
        if matches!(self.lifter, Lifter::Polynomial(_)) {
            factored = vec![None; polynomial_total];
        }
        for (coefficient, reference) in self.entries.iter().zip(&self.entry_refs) {
            if reference.is_constant() {
                constants[reference.index()] = (
                    coefficient.numerator.get_constant(),
                    coefficient.denominator.get_constant(),
                );
            } else {
                polynomials[reference.index()] = DenseRational::from_coefficient(coefficient);
                if let Some(slot) = factored.get_mut(reference.index()) {
                    *slot = Some(coefficient);
                }
            }
        }
        let factoring = Instant::now();
        let mut builder = BasisBuilder::default();
        for coefficient in factored.into_iter().flatten() {
            builder.push(coefficient)?;
        }
        let basis = builder.finish();
        let factor_seconds = factoring.elapsed().as_secs_f64();
        let trace = Trace {
            ops: self.ops,
            constants,
            polynomials,
            slots: self.next_slot as usize,
            terminal_slots: self.terminal_slots as usize,
            phase_starts: self.phase_starts,
        };
        let mut statistics = ModularStatistics {
            arithmetic: self.lifter.name(),
            trace_digest: statistics_requested().then(|| trace.digest()),
            ops: trace.ops.len(),
            constants: constant_total,
            polynomials: polynomial_total,
            peak_slots: trace.slots,
            terminal_slots: trace.terminal_slots,
            output_terminals: terms.len(),
            coefficient_lookups: self.lookups,
            record_seconds,
            prepare_seconds: prepare.elapsed().as_secs_f64(),
            factor_seconds,
            basis_factors: basis.factor_count(),
            basis_linear_factors: basis.linear_count(),
            threads: self.common.threads,
            jobs_on_caller_only: true,
            ..Default::default()
        };
        if let Some(digest) = statistics.trace_digest {
            // The recorded trace before its (long) replay, so that runs stopped
            // during reconstruction can still be compared.
            report_line(&format!(
                "{{\"stage\":\"recorded\",\"ops\":{},\"peak_slots\":{},\"constants\":{},\
                 \"polynomials\":{},\"record_seconds\":{:.6},\"trace_digest\":\"{digest:016x}\"}}",
                statistics.ops,
                statistics.peak_slots,
                statistics.constants,
                statistics.polynomials,
                statistics.record_seconds,
            ));
        }
        let keys: Vec<IntegralKey> = terms.keys().cloned().collect();
        let slots: Vec<usize> = terms.values().map(|w| w.slot as usize).collect();
        let has_cancellations = trace
            .ops
            .iter()
            .any(|op| matches!(op.tag(), OpTag::Cancel | OpTag::TermCancel));
        if slots.is_empty() && !has_cancellations {
            // Nothing to reconstruct and no erasure to certify.
            return Ok((BTreeMap::new(), statistics));
        }
        let executor = Executor::new(self.common.threads)?;
        let lifted = match self.lifter {
            Lifter::Sampled(options) => {
                let mut source = SampledSource::new(&trace, &slots, options, self.common.seed);
                let lifted = lift::lift(
                    &mut source,
                    &self.context,
                    &self.common,
                    &executor,
                    &mut statistics,
                );
                // The structure is fixed by this recording.
                if let Ok(lifted) = &lifted {
                    debug_assert!(std::sync::Arc::ptr_eq(
                        &lifted.structure,
                        source.structure()
                    ));
                }
                lifted
            }
            Lifter::Polynomial(options) => {
                let mut source = PolynomialSource::new(&trace, &slots, &basis, options);
                let lifted = lift::lift(
                    &mut source,
                    &self.context,
                    &self.common,
                    &executor,
                    &mut statistics,
                );
                if let Ok(lifted) = &lifted {
                    debug_assert!(std::sync::Arc::ptr_eq(
                        &lifted.structure,
                        source.structure()
                    ));
                }
                lifted
            }
        };
        statistics.pool_threads = executor.pool_threads();
        (statistics.job_threads, statistics.jobs_on_caller_only) = executor.job_threads();
        let coefficients = lifted?.coefficients;
        let mut output = BTreeMap::new();
        for (key, coefficient) in keys.into_iter().zip(coefficients) {
            if !coefficient.is_zero() {
                output.insert(key, coefficient);
            }
        }
        Ok((output, statistics))
    }
}

impl OrderedWeights for ModularRecorder {
    type Weight = ModularWeight;
    type Contribution = ModularContribution;
    type Probe = [Fe; PROBES];

    fn admit(&mut self, coefficient: Coefficient) -> Result<ModularWeight, ReductionError> {
        let reference = self.intern(&coefficient)?;
        let slot = self.allocate()?;
        self.ops.push(Op::new(OpTag::Init, slot, reference));
        Ok(ModularWeight {
            slot,
            probe: self.probe(reference),
        })
    }

    fn begin_step(&mut self, weight: &ModularWeight) -> Result<(), ReductionError> {
        self.ops
            .push(Op::new(OpTag::Pop, weight.slot, CoefRef(CONSTANT_FLAG)));
        Ok(())
    }

    fn phase_started(&mut self, phase: usize) {
        self.phase_starts.push((phase, self.ops.len()));
    }

    fn end_step(&mut self, weight: ModularWeight) {
        self.free_slots.push(weight.slot);
    }

    fn scale(
        &mut self,
        weight: &ModularWeight,
        coefficient: &Coefficient,
    ) -> Result<ModularContribution, ReductionError> {
        let zero = coefficient.is_zero();
        let reference = self.intern(coefficient)?;
        Ok(ModularContribution {
            coefficient: reference,
            probe: self.scaled(weight, reference),
            zero,
        })
    }

    fn contribution_is_zero(&self, contribution: &ModularContribution) -> bool {
        // Frontier weights are nonzero; the product is zero exactly when the
        // exact step coefficient is.
        contribution.zero
    }

    fn insert(
        &mut self,
        contribution: ModularContribution,
    ) -> Result<ModularWeight, ReductionError> {
        let slot = self.allocate()?;
        self.ops
            .push(Op::new(OpTag::New, slot, contribution.coefficient));
        Ok(ModularWeight {
            slot,
            probe: contribution.probe,
        })
    }

    fn merge(
        &mut self,
        existing: &mut ModularWeight,
        contribution: ModularContribution,
    ) -> Result<bool, ReductionError> {
        self.ops
            .push(Op::new(OpTag::Add, existing.slot, contribution.coefficient));
        Ok(self.add_probe(existing, &contribution))
    }

    fn cancel(&mut self, removed: ModularWeight) -> Result<(), ReductionError> {
        self.ops
            .push(Op::new(OpTag::Cancel, removed.slot, CoefRef(CONSTANT_FLAG)));
        self.free_slots.push(removed.slot);
        Ok(())
    }

    fn insert_terminal(
        &mut self,
        contribution: ModularContribution,
    ) -> Result<ModularWeight, ReductionError> {
        let slot = self.terminal_slots;
        if slot > SLOT_MASK {
            return Err(error("too many terminal accumulators"));
        }
        self.terminal_slots += 1;
        self.ops
            .push(Op::new(OpTag::TermNew, slot, contribution.coefficient));
        Ok(ModularWeight {
            slot,
            probe: contribution.probe,
        })
    }

    fn merge_terminal(
        &mut self,
        existing: &mut ModularWeight,
        contribution: ModularContribution,
    ) -> Result<bool, ReductionError> {
        self.ops.push(Op::new(
            OpTag::TermAdd,
            existing.slot,
            contribution.coefficient,
        ));
        Ok(self.add_probe(existing, &contribution))
    }

    fn cancel_terminal(&mut self, removed: ModularWeight) -> Result<(), ReductionError> {
        self.ops.push(Op::new(
            OpTag::TermCancel,
            removed.slot,
            CoefRef(CONSTANT_FLAG),
        ));
        Ok(())
    }

    fn monomials(&self, _: &ModularWeight) -> Result<usize, ReductionError> {
        Ok(1)
    }

    /// Recorded weights are finite-field images, never exact values.
    fn exact(_: &ModularWeight) -> Option<&Coefficient> {
        None
    }

    fn tail_checkpoint(&self) -> Option<usize> {
        Some(self.ops.len())
    }

    fn tail_probe(&self, weight: &ModularWeight) -> Option<[Fe; PROBES]> {
        Some(weight.probe)
    }

    fn tail_probe_coefficient(
        &self,
        coefficient: &Coefficient,
    ) -> Result<[Fe; PROBES], ReductionError> {
        self.context
            .validate_with_limits(coefficient, self.limits)
            .map_err(ReductionError::from)?;
        self.evaluate_probe(&DenseRational::from_coefficient(coefficient))
    }

    fn tail_probe_add_product(
        &self,
        accumulator: &mut Option<[Fe; PROBES]>,
        a: &[Fe; PROBES],
        b: &[Fe; PROBES],
    ) {
        let field = &self.probe_field;
        let product: [Fe; PROBES] = std::array::from_fn(|i| field.mul(&a[i], &b[i]));
        match accumulator {
            None => *accumulator = Some(product),
            Some(sum) => {
                for (value, added) in sum.iter_mut().zip(product) {
                    *value = field.add(&*value, &added);
                }
            }
        }
    }

    fn tail_probe_is_zero(&self, probe: &[Fe; PROBES]) -> bool {
        probe.iter().all(|value| self.probe_field.is_zero(value))
    }

    fn tail_slot(weight: &ModularWeight) -> Option<u32> {
        Some(weight.slot)
    }

    /// Truncate the trace to `rewrite.ops` and append `T += sum_K w(K) u(K)`
    /// for the entries `(slot of w(K), u(K))`, grouped so that almost every
    /// recorded product has a constant coefficient: with `u(K) = N_K / D_K`
    /// and `N_K = sum_j n_{K,j} d^j`,
    ///
    /// `sum_K w(K) u(K) = sum_{D,j} (d^j / D) * S_{D,j}`,
    /// `S_{D,j} = sum_{K: D_K = D} n_{K,j} w(K)`,
    ///
    /// an exact identity. Each `S_{D,j}` accumulates in a fresh slot (never a
    /// slot of the truncated program), then one product per `(D, j)` enters
    /// the terminal's accumulator (the one live at the checkpoint, else a new
    /// one). `keep` is the forward decision on the terminal: kept, `terminal`
    /// points at the accumulator (a new weight carries `probe`); erased, the
    /// accumulator is recorded as erased, which every replay verifies.
    fn rewrite_tail(
        &mut self,
        rewrite: super::TailRewrite<'_>,
        terminal: &mut Option<ModularWeight>,
        probe: Option<[Fe; PROBES]>,
    ) -> Result<super::TailRewriteReport, ReductionError> {
        if rewrite.ops > self.ops.len() {
            return Err(error("tail checkpoint beyond the recorded trace"));
        }
        self.ops.truncate(rewrite.ops);
        let mut groups: std::collections::HashMap<crate::algebra::CoefficientPolynomial, usize> =
            std::collections::HashMap::new();
        let mut denominators = Vec::new();
        // (group, exponents) -> index into `basis`
        let mut index: std::collections::HashMap<(usize, Vec<u16>), usize> =
            std::collections::HashMap::new();
        let mut basis: Vec<(usize, Vec<u16>, u32)> = Vec::new();
        let mut pending = Vec::new();
        for &(slot, value) in rewrite.entries {
            if value.is_zero() {
                continue;
            }
            let next = groups.len();
            let group = *groups.entry(value.denominator.clone()).or_insert_with(|| {
                denominators.push(value.denominator.clone());
                next
            });
            pending.push((slot, value, group));
        }
        for (slot, value, group) in pending {
            self.ops
                .push(Op::new(OpTag::Pop, slot, CoefRef(CONSTANT_FLAG)));
            let numerator = &value.numerator;
            for (term, coefficient) in numerator.coefficients.iter().enumerate() {
                let exponents = numerator.exponents(term).to_vec();
                let constant = Coefficient {
                    numerator: numerator.constant(coefficient.clone()),
                    denominator: numerator.one(),
                };
                let reference = self.intern(&constant)?;
                match index.get(&(group, exponents.clone())) {
                    Some(&element) => {
                        self.ops
                            .push(Op::new(OpTag::Add, basis[element].2, reference));
                    }
                    None => {
                        let accumulator = self.next_slot;
                        if accumulator > SLOT_MASK {
                            return Err(error("too many simultaneously live frontier slots"));
                        }
                        self.next_slot += 1;
                        self.ops
                            .push(Op::new(OpTag::New, accumulator, reference));
                        index.insert((group, exponents.clone()), basis.len());
                        basis.push((group, exponents, accumulator));
                    }
                }
            }
        }
        let mut target = rewrite.live;
        for (group, exponents, accumulator) in &basis {
            let template = &denominators[*group];
            let monomial = Coefficient {
                numerator: template.monomial(
                    symbolica::domains::integer::Integer::one(),
                    exponents.clone(),
                ),
                denominator: template.one(),
            };
            let denominator = Coefficient {
                numerator: template.clone(),
                denominator: template.one(),
            };
            let function = self
                .context
                .try_div(&monomial, &denominator, self.limits)
                .map_err(ReductionError::from)?;
            let reference = self.intern(&function)?;
            self.ops
                .push(Op::new(OpTag::Pop, *accumulator, CoefRef(CONSTANT_FLAG)));
            match target {
                Some(slot) => self.ops.push(Op::new(OpTag::TermAdd, slot, reference)),
                None => {
                    let slot = self.terminal_slots;
                    if slot > SLOT_MASK {
                        return Err(error("too many terminal accumulators"));
                    }
                    self.terminal_slots += 1;
                    self.ops.push(Op::new(OpTag::TermNew, slot, reference));
                    target = Some(slot);
                }
            }
        }
        match (rewrite.keep, target) {
            (true, Some(slot)) => match terminal {
                Some(weight) => {
                    weight.slot = slot;
                    if let Some(probe) = probe {
                        weight.probe = probe;
                    }
                }
                None => {
                    *terminal = Some(ModularWeight {
                        slot,
                        probe: probe.ok_or_else(|| error("a new tail terminal has no probe"))?,
                    })
                }
            },
            (true, None) => {
                return Err(error(
                    "the forward pass kept a tail terminal that no entry reaches",
                ));
            }
            (false, target) => {
                // Erased (or never reached): every replay verifies that the
                // adjoint sum is zero.
                if let Some(slot) = target {
                    self.ops.push(Op::new(
                        OpTag::TermCancel,
                        slot,
                        CoefRef(CONSTANT_FLAG),
                    ));
                }
                *terminal = None;
            }
        }
        Ok(super::TailRewriteReport {
            denominators: denominators.len(),
            products: basis.len(),
        })
    }
}

/// Append one JSON line per modular request when requested by the caller's
/// environment (prototype instrumentation, no effect on results).
pub(crate) fn report_statistics(statistics: &ModularStatistics) {
    #[cfg(test)]
    REPORTED.with(|reported| reported.borrow_mut().push(statistics.clone()));
    report_line(&statistics.to_json());
}

pub(super) fn report_line(line: &str) {
    if let Some(path) = std::env::var_os("RUSTRED_ORDERED_MODULAR_STATS") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

#[cfg(test)]
thread_local! {
    static REPORTED: std::cell::RefCell<Vec<ModularStatistics>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Statistics reported on the current thread since the last call (tests only).
#[cfg(test)]
pub(crate) fn take_reported_statistics() -> Vec<ModularStatistics> {
    REPORTED.with(|reported| std::mem::take(&mut *reported.borrow_mut()))
}
