//! Generic sectors, cuts, patterns, and deterministic integral ordering.
//!
//! This module is deliberately independent of loop count, topology, and
//! coefficient fields.  A sector is determined exclusively from the
//! **unshifted integer lattice indices**:
//!
//! ```text
//! active i:   n_i >= 1
//! inactive i: n_i <= 0
//! ```
//!
//! Family `PowerShifts` therefore do not appear in any API in this module.
//! Cuts and patterns classify sectors as excluded metadata; they are not zero
//! proofs.  Actual zero-sector certificates belong to a later analysis layer.
//!
//! Source correspondence:
//!
//! - LiteRed `jSector` supplies the raw sign convention;
//! - `jSubsectors` supplies active-bit contraction semantics;
//! - `CutDs` and `SectorsPattern` supply independent admissibility filters;
//! - `jComplexity`/`MakeOrderMatrix` motivate the named v1 complexity key.
//!
//! LiteRed permits caller-configurable (and even randomized) order matrices.
//! RustRed instead persists one deterministic policy identifier and exact key
//! schema.  Changing that identifier or schema invalidates discovered rules.

use std::cmp::Ordering;
use std::fmt;

/// Stable identifier of RustRed's first deterministic integral order.
pub const RUSTRED_UNSHIFTED_ORDER_V1_ID: &str = "rustred.unshifted-sector-order.v1";

/// Stable field order used by [`IntegralComplexityKey::to_stable_string`].
pub const RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA: &str =
    "arity,propagators,sector-bits,corner-distance,dots,numerators,index-excess";

/// A sector mask in denominator-index order.
///
/// Position zero is serialized first, matching the leftmost (most
/// significant) bit in LiteRed's `js[basis,...]`/`FromDigits[...,2]` usage.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SectorMask {
    active: Vec<bool>,
}

impl SectorMask {
    pub fn try_new(active: impl IntoIterator<Item = bool>) -> Result<Self, SectorFoundationError> {
        let active = try_collect_vec(active, "sector mask bits")?;
        Self::try_from_preallocated(active)
    }

    /// Retain an allocation which its caller already obtained through a
    /// fallible reservation boundary. No proportional copy or shrink occurs.
    pub(crate) fn try_from_preallocated(active: Vec<bool>) -> Result<Self, SectorFoundationError> {
        if active.is_empty() {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        Ok(Self { active })
    }

    /// Derive the sector from unshifted integral powers.
    pub fn try_from_indices(indices: &[i64]) -> Result<Self, SectorFoundationError> {
        if indices.is_empty() {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        Self::try_new(indices.iter().map(|&index| index >= 1))
    }

    /// Parse the stable index-major `0`/`1` representation.
    pub fn try_from_bit_string(bits: &str) -> Result<Self, SectorFoundationError> {
        if bits.is_empty() {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        let mut active = Vec::new();
        try_reserve_exact(&mut active, bits.len(), "sector mask bits")?;
        for (position, byte) in bits.bytes().enumerate() {
            match byte {
                b'0' => active.push(false),
                b'1' => active.push(true),
                _ => {
                    return Err(SectorFoundationError::InvalidSectorBit { position, byte });
                }
            }
        }
        Self::try_from_preallocated(active)
    }

    pub fn arity(&self) -> usize {
        self.active.len()
    }

    pub fn active_bits(&self) -> &[bool] {
        &self.active
    }

    pub fn is_active(&self, position: usize) -> Result<bool, SectorFoundationError> {
        self.active
            .get(position)
            .copied()
            .ok_or(SectorFoundationError::IndexOutOfRange {
                position,
                arity: self.arity(),
            })
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|&&active| active).count()
    }

    /// The LiteRed corner: `1` in active slots and `0` in inactive slots.
    pub fn corner_indices(&self) -> Vec<i64> {
        self.active
            .iter()
            .map(|&active| i64::from(active))
            .collect()
    }

    pub fn with_activity(
        &self,
        position: usize,
        active: bool,
    ) -> Result<Self, SectorFoundationError> {
        if position >= self.arity() {
            return Err(SectorFoundationError::IndexOutOfRange {
                position,
                arity: self.arity(),
            });
        }
        let mut result = Vec::new();
        try_reserve_exact(&mut result, self.arity(), "sector mask bits")?;
        result.extend_from_slice(&self.active);
        result[position] = active;
        Self::try_from_preallocated(result)
    }

    /// `self` is a subsector of `other` iff every active bit of `self` is
    /// also active in `other`.
    pub fn is_subsector_of(&self, other: &Self) -> Result<bool, SectorFoundationError> {
        self.check_other_arity(other)?;
        Ok(self
            .active
            .iter()
            .zip(&other.active)
            .all(|(&candidate, &container)| !candidate || container))
    }

    pub fn is_strict_subsector_of(&self, other: &Self) -> Result<bool, SectorFoundationError> {
        Ok(self != other && self.is_subsector_of(other)?)
    }

    pub fn to_bit_string(&self) -> String {
        self.active
            .iter()
            .map(|&active| if active { '1' } else { '0' })
            .collect()
    }

    fn check_arity(&self, actual: usize) -> Result<(), SectorFoundationError> {
        if actual == self.arity() {
            Ok(())
        } else {
            Err(SectorFoundationError::WrongArity {
                expected: self.arity(),
                actual,
            })
        }
    }

    fn check_other_arity(&self, other: &Self) -> Result<(), SectorFoundationError> {
        self.check_arity(other.arity())
    }
}

impl fmt::Display for SectorMask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_bit_string())
    }
}

/// Required-active denominator positions (`CutDs` semantics).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CutConstraint {
    required_active: SectorMask,
}

impl CutConstraint {
    pub fn none(arity: usize) -> Result<Self, SectorFoundationError> {
        Ok(Self {
            required_active: SectorMask::try_new(std::iter::repeat_n(false, arity))?,
        })
    }

    pub fn try_new(
        required_active: impl IntoIterator<Item = bool>,
    ) -> Result<Self, SectorFoundationError> {
        Ok(Self {
            required_active: SectorMask::try_new(required_active)?,
        })
    }

    pub fn try_from_positions(
        arity: usize,
        positions: impl IntoIterator<Item = usize>,
    ) -> Result<Self, SectorFoundationError> {
        if arity == 0 {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        let mut required = Vec::new();
        try_reserve_exact(&mut required, arity, "cut active mask")?;
        required.resize(arity, false);
        for position in positions {
            if position >= arity {
                return Err(SectorFoundationError::IndexOutOfRange { position, arity });
            }
            if required[position] {
                return Err(SectorFoundationError::DuplicateIndex { position });
            }
            required[position] = true;
        }
        Ok(Self {
            required_active: SectorMask::try_from_preallocated(required)?,
        })
    }

    pub fn arity(&self) -> usize {
        self.required_active.arity()
    }

    pub fn required_active(&self) -> &SectorMask {
        &self.required_active
    }

    pub fn missing_required_active(
        &self,
        sector: &SectorMask,
    ) -> Result<Vec<usize>, SectorFoundationError> {
        self.required_active.check_other_arity(sector)?;
        let missing_count = self
            .required_active
            .active
            .iter()
            .zip(&sector.active)
            .filter(|&(&required, &active)| required && !active)
            .count();
        let mut missing = Vec::new();
        try_reserve_exact(&mut missing, missing_count, "missing cut positions")?;
        for (position, (&required, &active)) in self
            .required_active
            .active
            .iter()
            .zip(&sector.active)
            .enumerate()
        {
            if required && !active {
                missing.push(position);
            }
        }
        debug_assert_eq!(missing.len(), missing_count);
        Ok(missing)
    }

    pub fn to_bit_string(&self) -> String {
        self.required_active.to_bit_string()
    }
}

/// One checked `SectorsPattern` slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SectorPatternSlot {
    Any,
    Active,
    Inactive,
}

/// A fixed-arity sector admissibility pattern.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorPattern {
    // Keep the fallibly reserved allocation. Converting to a boxed slice may
    // perform a second proportional shrink allocation.
    slots: Vec<SectorPatternSlot>,
}

impl SectorPattern {
    pub fn any(arity: usize) -> Result<Self, SectorFoundationError> {
        Self::try_new(std::iter::repeat_n(SectorPatternSlot::Any, arity))
    }

    pub fn try_new(
        slots: impl IntoIterator<Item = SectorPatternSlot>,
    ) -> Result<Self, SectorFoundationError> {
        let slots = try_collect_vec(slots, "sector pattern slots")?;
        Self::try_from_preallocated(slots)
    }

    fn try_from_preallocated(slots: Vec<SectorPatternSlot>) -> Result<Self, SectorFoundationError> {
        if slots.is_empty() {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        Ok(Self { slots })
    }

    /// Parse stable pattern characters: `*` (any), `1` (active), `0`
    /// (inactive).
    pub fn try_from_string(pattern: &str) -> Result<Self, SectorFoundationError> {
        if pattern.is_empty() {
            return Err(SectorFoundationError::EmptyIndexSpace);
        }
        let mut slots = Vec::new();
        try_reserve_exact(&mut slots, pattern.len(), "sector pattern slots")?;
        for (position, byte) in pattern.bytes().enumerate() {
            slots.push(match byte {
                b'*' => SectorPatternSlot::Any,
                b'1' => SectorPatternSlot::Active,
                b'0' => SectorPatternSlot::Inactive,
                _ => {
                    return Err(SectorFoundationError::InvalidPatternSlot { position, byte });
                }
            });
        }
        Self::try_from_preallocated(slots)
    }

    pub fn arity(&self) -> usize {
        self.slots.len()
    }

    pub fn slots(&self) -> &[SectorPatternSlot] {
        &self.slots
    }

    pub fn mismatches(
        &self,
        sector: &SectorMask,
    ) -> Result<Vec<SectorPatternMismatch>, SectorFoundationError> {
        if self.arity() != sector.arity() {
            return Err(SectorFoundationError::WrongArity {
                expected: self.arity(),
                actual: sector.arity(),
            });
        }
        let mismatch_count = self
            .slots
            .iter()
            .zip(&sector.active)
            .filter(|&(&required, &actual_active)| !pattern_slot_matches(required, actual_active))
            .count();
        let mut mismatches = Vec::new();
        try_reserve_exact(&mut mismatches, mismatch_count, "sector pattern mismatches")?;
        for (position, (&required, &actual_active)) in
            self.slots.iter().zip(&sector.active).enumerate()
        {
            if !pattern_slot_matches(required, actual_active) {
                mismatches.push(SectorPatternMismatch {
                    position,
                    required,
                    actual_active,
                });
            }
        }
        debug_assert_eq!(mismatches.len(), mismatch_count);
        Ok(mismatches)
    }

    pub fn to_stable_string(&self) -> String {
        self.slots
            .iter()
            .map(|slot| match slot {
                SectorPatternSlot::Any => '*',
                SectorPatternSlot::Active => '1',
                SectorPatternSlot::Inactive => '0',
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SectorPatternMismatch {
    position: usize,
    required: SectorPatternSlot,
    actual_active: bool,
}

impl SectorPatternMismatch {
    pub fn position(self) -> usize {
        self.position
    }

    pub fn required(self) -> SectorPatternSlot {
        self.required
    }

    pub fn actual_active(self) -> bool {
        self.actual_active
    }
}

/// Structured evidence that user constraints excluded a sector.
///
/// This evidence is admissibility metadata, never an integral-zero proof.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorExclusion {
    // Retain the vectors whose capacity was acquired fallibly.
    missing_required_active: Vec<usize>,
    pattern_mismatches: Vec<SectorPatternMismatch>,
}

impl SectorExclusion {
    pub fn missing_required_active(&self) -> &[usize] {
        &self.missing_required_active
    }

    pub fn pattern_mismatches(&self) -> &[SectorPatternMismatch] {
        &self.pattern_mismatches
    }
}

/// Combined user cuts and sector pattern.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorRestrictions {
    cuts: CutConstraint,
    pattern: SectorPattern,
}

impl SectorRestrictions {
    pub fn unrestricted(arity: usize) -> Result<Self, SectorFoundationError> {
        Self::try_new(CutConstraint::none(arity)?, SectorPattern::any(arity)?)
    }

    pub fn try_new(
        cuts: CutConstraint,
        pattern: SectorPattern,
    ) -> Result<Self, SectorFoundationError> {
        if cuts.arity() != pattern.arity() {
            return Err(SectorFoundationError::WrongArity {
                expected: cuts.arity(),
                actual: pattern.arity(),
            });
        }
        Ok(Self { cuts, pattern })
    }

    pub fn arity(&self) -> usize {
        self.cuts.arity()
    }

    pub fn cuts(&self) -> &CutConstraint {
        &self.cuts
    }

    pub fn pattern(&self) -> &SectorPattern {
        &self.pattern
    }

    pub fn exclusion(
        &self,
        sector: &SectorMask,
    ) -> Result<Option<SectorExclusion>, SectorFoundationError> {
        let missing_required_active = self.cuts.missing_required_active(sector)?;
        let pattern_mismatches = self.pattern.mismatches(sector)?;
        if missing_required_active.is_empty() && pattern_mismatches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SectorExclusion {
                missing_required_active,
                pattern_mismatches,
            }))
        }
    }
}

/// Persisted choice of integral-ordering semantics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntegralOrderingPolicy {
    #[default]
    RustRedUnshiftedV1,
}

impl IntegralOrderingPolicy {
    pub fn try_from_stable_id(id: &str) -> Result<Self, SectorFoundationError> {
        match id {
            RUSTRED_UNSHIFTED_ORDER_V1_ID => Ok(Self::RustRedUnshiftedV1),
            _ => Err(SectorFoundationError::UnknownOrderingPolicy {
                id: try_copy_string(id, "ordering policy identifier")?,
            }),
        }
    }

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::RustRedUnshiftedV1 => RUSTRED_UNSHIFTED_ORDER_V1_ID,
        }
    }

    pub const fn key_schema(self) -> &'static str {
        match self {
            Self::RustRedUnshiftedV1 => RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA,
        }
    }

    /// Build an exact, injective complexity key from unshifted indices.
    pub fn complexity_key(
        self,
        indices: &[i64],
    ) -> Result<IntegralComplexityKey, SectorFoundationError> {
        let sector = SectorMask::try_from_indices(indices)?;
        let mut dots = 0_u128;
        let mut numerators = 0_u128;
        let mut index_excess = Vec::new();
        try_reserve_exact(
            &mut index_excess,
            indices.len(),
            "integral complexity index excess",
        )?;
        for (&active, &index) in sector.active.iter().zip(indices) {
            let excess = if active {
                debug_assert!(index >= 1);
                u128::from((index - 1) as u64)
            } else {
                u128::from(index.unsigned_abs())
            };
            index_excess.push(excess);
            let target = if active { &mut dots } else { &mut numerators };
            *target =
                target
                    .checked_add(excess)
                    .ok_or(SectorFoundationError::ComplexityOverflow {
                        measure: if active { "dots" } else { "numerators" },
                    })?;
        }
        let corner_distance =
            dots.checked_add(numerators)
                .ok_or(SectorFoundationError::ComplexityOverflow {
                    measure: "corner distance",
                })?;
        Ok(IntegralComplexityKey {
            policy: self,
            arity: indices.len(),
            propagators: sector.active_count(),
            sector,
            corner_distance,
            dots,
            numerators,
            index_excess,
        })
    }

    /// Compare integrals by the persisted exact key.  `Less` means simpler.
    pub fn compare(self, left: &[i64], right: &[i64]) -> Result<Ordering, SectorFoundationError> {
        if left.len() != right.len() {
            return Err(SectorFoundationError::WrongArity {
                expected: left.len(),
                actual: right.len(),
            });
        }
        Ok(self.complexity_key(left)?.cmp(&self.complexity_key(right)?))
    }

    /// Prove that `target` is strictly simpler than `source` under this exact
    /// serialized policy.
    pub fn prove_strict_descent(
        self,
        source: &[i64],
        target: &[i64],
    ) -> Result<StrictDescentWitness, SectorFoundationError> {
        if source.len() != target.len() {
            return Err(SectorFoundationError::WrongArity {
                expected: source.len(),
                actual: target.len(),
            });
        }
        let source_key = self.complexity_key(source)?;
        let target_key = self.complexity_key(target)?;
        if target_key >= source_key {
            return Err(SectorFoundationError::NotStrictDescent);
        }
        let decisive_component = first_differing_component(&source_key, &target_key)
            .expect("strictly different keys have a first differing component");
        Ok(StrictDescentWitness {
            policy: self,
            source: source_key,
            target: target_key,
            decisive_component,
        })
    }
}

/// Exact strict total-order key.  Field declaration order is the policy.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IntegralComplexityKey {
    policy: IntegralOrderingPolicy,
    arity: usize,
    propagators: usize,
    sector: SectorMask,
    corner_distance: u128,
    dots: u128,
    numerators: u128,
    // Retain the fallibly reserved backing allocation.
    index_excess: Vec<u128>,
}

impl IntegralComplexityKey {
    pub fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn propagators(&self) -> usize {
        self.propagators
    }

    pub fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub fn corner_distance(&self) -> u128 {
        self.corner_distance
    }

    pub fn dots(&self) -> u128 {
        self.dots
    }

    pub fn numerators(&self) -> u128 {
        self.numerators
    }

    pub fn index_excess(&self) -> &[u128] {
        &self.index_excess
    }

    /// Stable text encoding for persistence diagnostics and golden tests.
    pub fn to_stable_string(&self) -> String {
        let excess = self
            .index_excess
            .iter()
            .map(u128::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{}|arity={}|propagators={}|sector={}|corner={}|dots={}|numerators={}|excess=[{}]",
            self.policy.stable_id(),
            self.arity,
            self.propagators,
            self.sector,
            self.corner_distance,
            self.dots,
            self.numerators,
            excess
        )
    }
}

/// First field that proves strict descent in the named lexicographic key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntegralComplexityComponent {
    Arity,
    PropagatorCount,
    SectorBit { position: usize },
    CornerDistance,
    DotPower,
    NumeratorPower,
    IndexExcess { position: usize },
}

/// Exact witness that a target key is strictly below a source key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StrictDescentWitness {
    policy: IntegralOrderingPolicy,
    source: IntegralComplexityKey,
    target: IntegralComplexityKey,
    decisive_component: IntegralComplexityComponent,
}

impl StrictDescentWitness {
    pub fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub fn source(&self) -> &IntegralComplexityKey {
        &self.source
    }

    pub fn target(&self) -> &IntegralComplexityKey {
        &self.target
    }

    pub fn decisive_component(&self) -> IntegralComplexityComponent {
        self.decisive_component
    }

    pub fn verify(&self) -> bool {
        self.source.policy == self.policy
            && self.target.policy == self.policy
            && self.target < self.source
            && first_differing_component(&self.source, &self.target)
                == Some(self.decisive_component)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SectorFoundationError {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        position: usize,
        arity: usize,
    },
    DuplicateIndex {
        position: usize,
    },
    InvalidSectorBit {
        position: usize,
        byte: u8,
    },
    InvalidPatternSlot {
        position: usize,
        byte: u8,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    ComplexityOverflow {
        measure: &'static str,
    },
    UnknownOrderingPolicy {
        id: String,
    },
    NotStrictDescent,
}

impl fmt::Display for SectorFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("a sector needs at least one index"),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "sector arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { position, arity } => write!(
                formatter,
                "sector index {position} is outside an index space of arity {arity}"
            ),
            Self::DuplicateIndex { position } => {
                write!(formatter, "sector index {position} is repeated")
            }
            Self::InvalidSectorBit { position, byte } => write!(
                formatter,
                "invalid sector bit byte {byte} at position {position}; expected 0 or 1"
            ),
            Self::InvalidPatternSlot { position, byte } => write!(
                formatter,
                "invalid sector-pattern byte {byte} at position {position}; expected *, 0, or 1"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} bounded entries for {resource}"
            ),
            Self::ComplexityOverflow { measure } => {
                write!(formatter, "integral {measure} complexity overflowed u128")
            }
            Self::UnknownOrderingPolicy { id } => {
                write!(formatter, "unknown integral-ordering policy {id:?}")
            }
            Self::NotStrictDescent => formatter.write_str(
                "the proposed target is not strictly simpler under the named ordering policy",
            ),
        }
    }
}

impl std::error::Error for SectorFoundationError {}

fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), SectorFoundationError> {
    let requested = values.len().checked_add(additional).unwrap_or(usize::MAX);
    values
        .try_reserve_exact(additional)
        .map_err(|_| SectorFoundationError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_collect_vec<T>(
    values: impl IntoIterator<Item = T>,
    resource: &'static str,
) -> Result<Vec<T>, SectorFoundationError> {
    let iterator = values.into_iter();
    // A non-exact upper hint can be arbitrarily loose. Reserve only the
    // iterator-guaranteed lower bound, then grow through checked seams.
    let (initial, _) = iterator.size_hint();
    let mut retained = Vec::new();
    try_reserve_exact(&mut retained, initial, resource)?;
    for value in iterator {
        if retained.len() == retained.capacity() {
            try_reserve_exact(&mut retained, 1, resource)?;
        }
        retained.push(value);
    }
    Ok(retained)
}

fn try_copy_string(source: &str, resource: &'static str) -> Result<String, SectorFoundationError> {
    let mut retained = String::new();
    retained.try_reserve_exact(source.len()).map_err(|_| {
        SectorFoundationError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    retained.push_str(source);
    Ok(retained)
}

fn pattern_slot_matches(required: SectorPatternSlot, actual_active: bool) -> bool {
    match required {
        SectorPatternSlot::Any => true,
        SectorPatternSlot::Active => actual_active,
        SectorPatternSlot::Inactive => !actual_active,
    }
}

fn first_differing_component(
    source: &IntegralComplexityKey,
    target: &IntegralComplexityKey,
) -> Option<IntegralComplexityComponent> {
    if source.arity != target.arity {
        return Some(IntegralComplexityComponent::Arity);
    }
    if source.propagators != target.propagators {
        return Some(IntegralComplexityComponent::PropagatorCount);
    }
    if source.sector != target.sector {
        let position = source
            .sector
            .active
            .iter()
            .zip(&target.sector.active)
            .position(|(left, right)| left != right)
            .expect("different equal-arity sectors have a differing bit");
        return Some(IntegralComplexityComponent::SectorBit { position });
    }
    if source.corner_distance != target.corner_distance {
        return Some(IntegralComplexityComponent::CornerDistance);
    }
    if source.dots != target.dots {
        return Some(IntegralComplexityComponent::DotPower);
    }
    if source.numerators != target.numerators {
        return Some(IntegralComplexityComponent::NumeratorPower);
    }
    source
        .index_excess
        .iter()
        .zip(&target.index_excess)
        .position(|(left, right)| left != right)
        .map(|position| IntegralComplexityComponent::IndexExcess { position })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;

    struct ImpossibleExactSizeHint<T> {
        value: Option<T>,
    }

    impl<T> ImpossibleExactSizeHint<T> {
        fn one(value: T) -> Self {
            Self { value: Some(value) }
        }
    }

    impl<T> Iterator for ImpossibleExactSizeHint<T> {
        type Item = T;

        fn next(&mut self) -> Option<Self::Item> {
            self.value.take()
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            (usize::MAX, Some(usize::MAX))
        }
    }

    fn all_indices(arity: usize, minimum: i64, maximum: i64) -> Vec<Vec<i64>> {
        fn recurse(
            arity: usize,
            minimum: i64,
            maximum: i64,
            current: &mut Vec<i64>,
            output: &mut Vec<Vec<i64>>,
        ) {
            if current.len() == arity {
                output.push(current.clone());
                return;
            }
            for value in minimum..=maximum {
                current.push(value);
                recurse(arity, minimum, maximum, current, output);
                current.pop();
            }
        }
        let mut output = Vec::new();
        recurse(arity, minimum, maximum, &mut Vec::new(), &mut output);
        output
    }

    #[test]
    fn raw_membership_is_exhaustive_and_power_shift_independent() {
        for arity in 1..=4 {
            for indices in all_indices(arity, -2, 2) {
                let sector = SectorMask::try_from_indices(&indices).unwrap();
                assert_eq!(
                    sector.active_bits(),
                    indices.iter().map(|&index| index >= 1).collect::<Vec<_>>()
                );
                // Deliberately vary arbitrary would-be PowerShifts.  The API
                // has no slot for them: membership and ordering remain a
                // function of raw n alone.
                for ignored_power_shifts in
                    [vec![0_i64; arity], vec![1_i64; arity], vec![-7_i64; arity]]
                {
                    let _ = ignored_power_shifts;
                    assert_eq!(SectorMask::try_from_indices(&indices).unwrap(), sector);
                    assert_eq!(
                        IntegralOrderingPolicy::default()
                            .complexity_key(&indices)
                            .unwrap(),
                        IntegralOrderingPolicy::default()
                            .complexity_key(&indices)
                            .unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn bit_orientation_and_stable_round_trip_match_litered() {
        let sector = SectorMask::try_from_bit_string("101001").unwrap();
        assert_eq!(
            sector.active_bits(),
            &[true, false, true, false, false, true]
        );
        assert_eq!(sector.to_bit_string(), "101001");
        assert_eq!(sector.corner_indices(), vec![1, 0, 1, 0, 0, 1]);
        assert_eq!(
            sector.with_activity(1, true).unwrap().to_bit_string(),
            "111001"
        );
        assert_eq!(sector.to_string(), "101001");
        assert!(matches!(
            SectorMask::try_from_bit_string("10x"),
            Err(SectorFoundationError::InvalidSectorBit {
                position: 2,
                byte: b'x'
            })
        ));
    }

    #[test]
    fn fallible_foundation_allocations_are_typed_and_report_requested_entries() {
        assert!(matches!(
            SectorMask::try_new(ImpossibleExactSizeHint::one(true)),
            Err(SectorFoundationError::AllocationFailure {
                resource: "sector mask bits",
                requested: usize::MAX,
            })
        ));
        assert!(matches!(
            SectorPattern::try_new(ImpossibleExactSizeHint::one(SectorPatternSlot::Any)),
            Err(SectorFoundationError::AllocationFailure {
                resource: "sector pattern slots",
                requested: usize::MAX,
            })
        ));
        assert!(matches!(
            CutConstraint::try_from_positions(usize::MAX, std::iter::empty()),
            Err(SectorFoundationError::AllocationFailure {
                resource: "cut active mask",
                requested: usize::MAX,
            })
        ));

        let allocation_error = SectorFoundationError::AllocationFailure {
            resource: "test payload",
            requested: 17,
        };
        assert_eq!(
            allocation_error.to_string(),
            "could not reserve 17 bounded entries for test payload"
        );
    }

    #[test]
    fn subset_relations_form_the_boolean_lattice_exhaustively() {
        let sectors = (0_u8..16)
            .map(|bits| {
                SectorMask::try_new((0..4).map(|position| bits & (1 << (3 - position)) != 0))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        for (left_bits, left) in sectors.iter().enumerate() {
            for (right_bits, right) in sectors.iter().enumerate() {
                let expected = (left_bits & !right_bits) == 0;
                assert_eq!(left.is_subsector_of(right).unwrap(), expected);
                assert_eq!(
                    left.is_strict_subsector_of(right).unwrap(),
                    expected && left_bits != right_bits
                );
            }
        }
    }

    #[test]
    fn cuts_and_patterns_are_exclusions_never_zero_proofs() {
        let restrictions = SectorRestrictions::try_new(
            CutConstraint::try_from_positions(4, [0, 2]).unwrap(),
            SectorPattern::try_new([
                SectorPatternSlot::Any,
                SectorPatternSlot::Inactive,
                SectorPatternSlot::Any,
                SectorPatternSlot::Active,
            ])
            .unwrap(),
        )
        .unwrap();

        for bits in 0_u8..16 {
            let sector =
                SectorMask::try_new((0..4).map(|position| bits & (1 << (3 - position)) != 0))
                    .unwrap();
            let expected_admissible = sector.active_bits()[0]
                && sector.active_bits()[2]
                && !sector.active_bits()[1]
                && sector.active_bits()[3];
            let exclusion = restrictions.exclusion(&sector).unwrap();
            if expected_admissible {
                assert_eq!(exclusion, None);
            } else {
                let exclusion = exclusion.expect("inadmissible sectors carry exclusion evidence");
                assert!(
                    !exclusion.missing_required_active().is_empty()
                        || !exclusion.pattern_mismatches().is_empty()
                );
            }
        }

        assert_eq!(restrictions.cuts().to_bit_string(), "1010");
        assert_eq!(restrictions.pattern().to_stable_string(), "*0*1");
        assert!(matches!(
            CutConstraint::try_from_positions(4, [1, 1]),
            Err(SectorFoundationError::DuplicateIndex { position: 1 })
        ));
    }

    #[test]
    fn complexity_key_is_injective_strict_and_has_stable_manifest() {
        let policy = IntegralOrderingPolicy::RustRedUnshiftedV1;
        assert_eq!(policy.stable_id(), RUSTRED_UNSHIFTED_ORDER_V1_ID);
        assert_eq!(policy.key_schema(), RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA);
        assert_eq!(
            IntegralOrderingPolicy::try_from_stable_id(policy.stable_id()).unwrap(),
            policy
        );
        assert!(matches!(
            IntegralOrderingPolicy::try_from_stable_id("rustred.unknown-order.v9"),
            Err(SectorFoundationError::UnknownOrderingPolicy { .. })
        ));

        let points = all_indices(3, -3, 3);
        let keys = points
            .iter()
            .map(|point| policy.complexity_key(point).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(keys.iter().collect::<HashSet<_>>().len(), points.len());
        assert_eq!(
            keys.iter().cloned().collect::<BTreeSet<_>>().len(),
            points.len()
        );

        for (left_position, left) in points.iter().enumerate() {
            for (right_position, right) in points.iter().enumerate() {
                let comparison = policy.compare(left, right).unwrap();
                assert_eq!(comparison, keys[left_position].cmp(&keys[right_position]));
                assert_eq!(comparison == Ordering::Equal, left == right);
                assert_eq!(comparison, policy.compare(right, left).unwrap().reverse());
            }
        }

        let key = policy.complexity_key(&[2, 0, -3]).unwrap();
        assert_eq!(key.propagators(), 1);
        assert_eq!(key.sector().to_bit_string(), "100");
        assert_eq!(key.corner_distance(), 4);
        assert_eq!(key.dots(), 1);
        assert_eq!(key.numerators(), 3);
        assert_eq!(key.index_excess(), &[1, 0, 3]);
        assert_eq!(
            key.to_stable_string(),
            "rustred.unshifted-sector-order.v1|arity=3|propagators=1|sector=100|corner=4|dots=1|numerators=3|excess=[1,0,3]"
        );
    }

    #[test]
    fn descent_witness_identifies_the_first_strict_component() {
        let policy = IntegralOrderingPolicy::default();

        let dot_descent = policy.prove_strict_descent(&[3, 1], &[2, 1]).unwrap();
        assert_eq!(
            dot_descent.decisive_component(),
            IntegralComplexityComponent::CornerDistance
        );
        assert!(dot_descent.verify());

        let sector_descent = policy.prove_strict_descent(&[1, 1], &[1, 0]).unwrap();
        assert_eq!(
            sector_descent.decisive_component(),
            IntegralComplexityComponent::PropagatorCount
        );
        assert!(sector_descent.verify());

        let coordinate_descent = policy.prove_strict_descent(&[3, 2], &[2, 3]).unwrap();
        assert_eq!(
            coordinate_descent.decisive_component(),
            IntegralComplexityComponent::IndexExcess { position: 0 }
        );
        assert!(coordinate_descent.verify());

        assert_eq!(
            policy.prove_strict_descent(&[1, 1], &[1, 1]),
            Err(SectorFoundationError::NotStrictDescent)
        );
        assert_eq!(
            policy.prove_strict_descent(&[1, 1], &[2, 1]),
            Err(SectorFoundationError::NotStrictDescent)
        );
    }
}
