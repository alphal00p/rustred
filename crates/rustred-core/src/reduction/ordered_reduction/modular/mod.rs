//! Finite-field weight backend for the Ordered scheduler.
//!
//! The scheduler is run once with this recorder as its weight arithmetic. The
//! recorder carries `PROBES` values of every weight modulo one probe prime,
//! which are used only for the exact-zero erasure decision, and records a
//! straight-line trace of the weight propagation:
//!
//! - `Init(slot, c)`: a frontier slot starts at an input coefficient;
//! - `Pop(slot)`: the popped key's weight becomes the active weight;
//! - `New/Add(slot, c)`: `slot (=|+=) active * c` for a child;
//! - `Cancel(slot)`: the merged weight of `slot` was decided to be zero and
//!   the key was erased (replays verify that it is zero at every point);
//! - `TermNew/TermAdd/TermCancel(t, c)`: the same for terminal outputs.
//!
//! Coefficients `c` are the exact step coefficients produced by the adapters,
//! interned by value. The trace is then replayed over many 64-bit primes and
//! sample points in parallel, and each terminal coefficient is reconstructed
//! over Q (`reconstruct.rs`).
mod field;
mod reconstruct;
mod replay;

use std::collections::BTreeMap;
use std::hash::{BuildHasher, Hasher};
use std::time::Instant;

use hashbrown::HashTable;
use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::{Field, Ring, RingOps};
use symbolica::domains::finite_field::{FiniteFieldCore, PrimeIteratorU64, Zp64};
use symbolica::prelude::Integer;

use self::field::{DenseRational, Fe, reduce};
use super::OrderedWeights;
use super::arithmetic::ModularWeightOptions;
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::ReductionError;

/// Probe values per weight used only for zero decisions in the recording pass.
pub(super) const PROBES: usize = 2;

pub(super) fn error(detail: impl Into<String>) -> ReductionError {
    ReductionError::ModularWeights {
        detail: detail.into(),
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
#[derive(Clone, Debug, Default)]
pub(crate) struct ModularStatistics {
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
    pub batches: usize,
    pub primes: usize,
    pub points_first_prime: usize,
    pub points_per_prime: usize,
    pub max_numerator_degree: usize,
    pub max_denominator_degree: usize,
    pub max_coefficient_bits: usize,
    pub unlucky_images: usize,
    pub bad_points: usize,
    pub verification_failures: usize,
    pub cancel_checks: usize,
}

impl ModularStatistics {
    pub(crate) fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\"ops\":{},\"constants\":{},\"polynomials\":{},\"peak_slots\":{},",
                "\"terminal_slots\":{},\"output_terminals\":{},\"coefficient_lookups\":{},",
                "\"record_seconds\":{:.6},\"prepare_seconds\":{:.6},\"replay_seconds\":{:.6},",
                "\"replay_cpu_seconds\":{:.6},\"interpolation_seconds\":{:.6},\"lift_seconds\":{:.6},",
                "\"batches\":{},\"primes\":{},\"points_first_prime\":{},\"points_per_prime\":{},",
                "\"max_numerator_degree\":{},\"max_denominator_degree\":{},",
                "\"max_coefficient_bits\":{},\"unlucky_images\":{},\"bad_points\":{},",
                "\"verification_failures\":{},\"cancel_checks\":{}}}"
            ),
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
            self.batches,
            self.primes,
            self.points_first_prime,
            self.points_per_prime,
            self.max_numerator_degree,
            self.max_denominator_degree,
            self.max_coefficient_bits,
            self.unlucky_images,
            self.bad_points,
            self.verification_failures,
            self.cancel_checks,
        )
    }
}

/// Recording weight backend.
pub(crate) struct ModularRecorder {
    options: ModularWeightOptions,
    context: CoefficientContext,
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
    free_slots: Vec<u32>,
    next_slot: u32,
    terminal_slots: u32,
    lookups: usize,
    started: Instant,
}

impl ModularRecorder {
    pub(crate) fn try_new(
        context: &CoefficientContext,
        options: ModularWeightOptions,
    ) -> Result<Self, ReductionError> {
        options.validate().map_err(error)?;
        if context.variables().len() > 1 {
            return Err(error(
                "modular Ordered weights require a coefficient context with at most one variable",
            ));
        }
        // Probe primes come from a range disjoint from reconstruction primes.
        let mut rng = StdRng::seed_from_u64(options.seed ^ 0x7072_6f62_6573);
        let start = (1u64 << 61) + (rng.random::<u64>() >> 4);
        let prime = PrimeIteratorU64::new(start)
            .next()
            .ok_or_else(|| error("no probe prime"))?;
        let probe_field = Zp64::new(prime);
        let probe_points =
            std::array::from_fn(|_| probe_field.to_element(rng.random_range(2..prime)));
        Ok(Self {
            options,
            context: context.clone(),
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
        let dense = DenseRational::from_coefficient(coefficient);
        let field = &self.probe_field;
        let reference = if dense.is_constant() {
            let numerator = reduce(field, &dense.numerator[0]);
            let denominator = reduce(field, &dense.denominator[0]);
            if field.is_zero(&denominator) {
                return Err(error("probe prime divides a constant coefficient denominator"));
            }
            self.constant_probe
                .push(field.div(&numerator, &denominator));
            let reference = CoefRef(self.constant_count | CONSTANT_FLAG);
            self.constant_count += 1;
            if self.constant_count >= CONSTANT_FLAG {
                return Err(error("too many distinct constant coefficients"));
            }
            reference
        } else {
            let numerator: Vec<Fe> = dense.numerator.iter().map(|c| reduce(field, c)).collect();
            let denominator: Vec<Fe> =
                dense.denominator.iter().map(|c| reduce(field, c)).collect();
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

    /// Replay and reconstruct the live terminal accumulators.
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
        for (coefficient, reference) in self.entries.into_iter().zip(self.entry_refs) {
            if reference.is_constant() {
                constants[reference.index()] = (
                    coefficient.numerator.get_constant(),
                    coefficient.denominator.get_constant(),
                );
            } else {
                polynomials[reference.index()] = DenseRational::from_coefficient(&coefficient);
            }
        }
        let trace = Trace {
            ops: self.ops,
            constants,
            polynomials,
            slots: self.next_slot as usize,
            terminal_slots: self.terminal_slots as usize,
        };
        let mut statistics = ModularStatistics {
            ops: trace.ops.len(),
            constants: constant_total,
            polynomials: polynomial_total,
            peak_slots: trace.slots,
            terminal_slots: trace.terminal_slots,
            output_terminals: terms.len(),
            coefficient_lookups: self.lookups,
            record_seconds,
            prepare_seconds: prepare.elapsed().as_secs_f64(),
            ..Default::default()
        };
        let keys: Vec<IntegralKey> = terms.keys().cloned().collect();
        let slots: Vec<usize> = terms.values().map(|w| w.slot as usize).collect();
        let results = reconstruct::reconstruct(
            &trace,
            &slots,
            &self.context,
            &self.options,
            &mut statistics,
        )?;
        let mut output = BTreeMap::new();
        for (key, coefficient) in keys.into_iter().zip(results) {
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

    fn insert(&mut self, contribution: ModularContribution) -> Result<ModularWeight, ReductionError> {
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
}

/// Append one JSON line per modular request when requested by the caller's
/// environment (prototype instrumentation, no effect on results).
pub(crate) fn report_statistics(statistics: &ModularStatistics) {
    if let Some(path) = std::env::var_os("RUSTRED_ORDERED_MODULAR_STATS") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{}", statistics.to_json());
        }
    }
}

