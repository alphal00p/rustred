//! Exact integral ordering on an integer residual cylinder.
//!
//! A cylindrical start fixes only the coordinates listed in a canonical
//! [`PartialIndexAssignment`].  Every other coordinate remains a formal index
//! in its source [`SectorMask`].  This module extends
//! [`IntegralOrderingPolicy::RustRedUnshiftedV1`] to that mixed start without
//! choosing a concrete corner (or any other representative point).
//!
//! This is deliberately only an ordering primitive.  In particular, it does
//! not decide whether a pivot solves one residual case or maps between cases;
//! that decision belongs to the later residual-case scheduler.
//!
//! [`PartialIndexAssignment`] represents the safe integer-cylinder subset of
//! LiteRed starts.  LiteRed can also obtain dependent affine starts (for
//! example, one index expressed through another); those require a later,
//! strictly more general start representation and are not claimed here.

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::sync::Arc;

use crate::{IndexShift, IntegralOrderingPolicy, PartialIndexAssignment, SectorMask};

/// Replay schema for one cylindrical ordering context.
pub const CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA: &str =
    "rustred-cylindrical-parametric-elimination-ordering-v1";

/// Replay schema for a key produced in one cylindrical ordering context.
pub const CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA: &str =
    "rustred-cylindrical-integral-complexity-key-v1";

/// Stable field order of the cylindrical extension of RustRed's V1 order.
///
/// The three complexity totals and the per-index excesses are signed offsets:
/// the common, still-symbolic contribution of every free coordinate has been
/// removed.  `lattice-shift` is the final deterministic, injective tie-break.
pub const RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA: &str = "arity,propagators,formal-sector-bits,signed-corner-distance-offset,\
signed-dots-offset,signed-numerators-offset,signed-index-excess,lattice-shift";

const KEY_FIXED_COMPONENTS: usize = 5;
const KEY_COMPONENT_VECTORS: usize = 3;

/// Explicit retained-payload bounds for a cylindrical ordering context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CylindricalOrderingLimits {
    /// Maximum number of integral indices in the ordering lattice.
    pub max_arity: usize,
    /// Maximum number of fixed coordinates in the residual cylinder.
    pub max_fixed_assignments: usize,
    /// Maximum scalar slots retained by one normalized key.
    ///
    /// V1 charges five scalar fields plus three arity-sized vectors: formal
    /// sector bits, signed excesses, and the injective lattice-shift tie-break.
    pub max_key_components: usize,
    /// Maximum bytes in the replay-stable ordering manifest.
    pub max_manifest_bytes: usize,
}

impl Default for CylindricalOrderingLimits {
    fn default() -> Self {
        Self {
            max_arity: 4096,
            max_fixed_assignments: 4096,
            max_key_components: 16_384,
            max_manifest_bytes: 16 * 1024 * 1024,
        }
    }
}

/// A topology-independent cylindrical extension of a persisted integral order.
///
/// Free positions are retained explicitly as the checked complement of the
/// canonical assignment.  There is intentionally no `anchor` accessor: no
/// concrete point represents this symbolic cylinder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CylindricalParametricEliminationOrdering {
    schema: &'static str,
    key_schema: &'static str,
    policy: IntegralOrderingPolicy,
    sector: SectorMask,
    assignment: PartialIndexAssignment,
    free_positions: Box<[usize]>,
    limits: CylindricalOrderingLimits,
    stable_manifest: Arc<str>,
}

impl CylindricalParametricEliminationOrdering {
    /// Authenticate one nonempty integer cylinder and construct its manifest.
    pub fn try_new(
        policy: IntegralOrderingPolicy,
        sector: SectorMask,
        assignment: PartialIndexAssignment,
        limits: CylindricalOrderingLimits,
    ) -> Result<Self, CylindricalOrderingError> {
        let ordering = Self::try_new_unreplayed(policy, sector, assignment, limits)?;
        ordering.replay()?;
        Ok(ordering)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn key_schema(&self) -> &'static str {
        self.key_schema
    }

    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub const fn sector(&self) -> &SectorMask {
        &self.sector
    }

    pub const fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }

    /// Checked, increasing complement of [`Self::assignment`].
    pub fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    pub const fn limits(&self) -> CylindricalOrderingLimits {
        self.limits
    }

    pub fn stable_manifest(&self) -> &str {
        &self.stable_manifest
    }

    /// Share the exact replay identity with crate-internal certificate layers
    /// without allocating a second manifest payload.
    pub(crate) const fn stable_manifest_arc(&self) -> &Arc<str> {
        &self.stable_manifest
    }

    pub fn arity(&self) -> usize {
        self.sector.arity()
    }

    pub(crate) fn exact_key_component_count(&self) -> Result<usize, CylindricalOrderingError> {
        key_component_count(self.arity())
    }

    /// Build the exact signed normalized key of one lattice shift.
    pub fn key_for_shift(
        &self,
        shift: &IndexShift,
    ) -> Result<CylindricalIntegralComplexityKey, CylindricalOrderingError> {
        let key = self.key_for_shift_unreplayed(shift)?;
        self.replay_key(&key)?;
        Ok(key)
    }

    /// Construct one key after an owning certificate has replayed this
    /// ordering. Unlike [`Self::key_for_shift`], this does not construct a
    /// second key solely to replay the first.
    pub(crate) fn key_for_shift_with_replayed_ordering(
        &self,
        shift: &IndexShift,
    ) -> Result<CylindricalIntegralComplexityKey, CylindricalOrderingError> {
        self.key_for_shift_unreplayed(shift)
    }

    /// Compare two shifts. `Less` means that `left` is simpler.
    pub fn compare_shifts(
        &self,
        left: &IndexShift,
        right: &IndexShift,
    ) -> Result<Ordering, CylindricalOrderingError> {
        Ok(self.key_for_shift(left)?.cmp(&self.key_for_shift(right)?))
    }

    /// Rebuild this ordering from its authenticated inputs and compare every
    /// persisted field, including the stable schema and free-position order.
    pub fn replay(&self) -> Result<(), CylindricalOrderingError> {
        if self.schema != CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA
            || self.key_schema != RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA
        {
            return Err(CylindricalOrderingError::SchemaMismatch);
        }
        let replayed = Self::try_new_unreplayed(
            self.policy,
            self.sector.clone(),
            self.assignment.clone(),
            self.limits,
        )?;
        if replayed == *self {
            Ok(())
        } else {
            Err(CylindricalOrderingError::ReplayMismatch)
        }
    }

    /// Recompute one key and bind it to this exact cylindrical context.
    pub fn replay_key(
        &self,
        key: &CylindricalIntegralComplexityKey,
    ) -> Result<(), CylindricalOrderingError> {
        if key.schema != CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA
            || key.key_schema != RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA
        {
            return Err(CylindricalOrderingError::SchemaMismatch);
        }
        if key.ordering_manifest.as_ref() != self.stable_manifest.as_ref() {
            return Err(CylindricalOrderingError::KeyOrderingMismatch);
        }
        let replayed = self.key_for_shift_unreplayed(&key.shift)?;
        if replayed == *key {
            Ok(())
        } else {
            Err(CylindricalOrderingError::ReplayMismatch)
        }
    }

    fn try_new_unreplayed(
        policy: IntegralOrderingPolicy,
        sector: SectorMask,
        assignment: PartialIndexAssignment,
        limits: CylindricalOrderingLimits,
    ) -> Result<Self, CylindricalOrderingError> {
        // `try_new` validates before its final replay.  Reproduce that exact
        // construction here without recursively replaying.
        match policy {
            // Keep this match exhaustive: adding a policy must not silently
            // inherit the signed V1 formula.
            IntegralOrderingPolicy::RustRedUnshiftedV1 => {}
        }
        let arity = sector.arity();
        check_limit("cylindrical ordering arity", arity, limits.max_arity)?;
        if assignment.arity() != arity {
            return Err(CylindricalOrderingError::WrongAssignmentArity {
                expected: arity,
                actual: assignment.arity(),
            });
        }
        check_limit(
            "cylindrical fixed assignments",
            assignment.entries().len(),
            limits.max_fixed_assignments,
        )?;
        check_limit(
            "cylindrical order-key components",
            key_component_count(arity)?,
            limits.max_key_components,
        )?;
        for &(position, value) in assignment.entries() {
            let source_active = sector.active_bits()[position];
            if source_active != (value >= 1) {
                return Err(
                    CylindricalOrderingError::FixedAssignmentOutsideSourceSector {
                        position,
                        value,
                        source_active,
                    },
                );
            }
        }
        let free_count = arity.checked_sub(assignment.entries().len()).ok_or(
            CylindricalOrderingError::ResourceCountOverflow {
                resource: "cylindrical free positions",
            },
        )?;
        let mut free_positions = Vec::with_capacity(free_count);
        let mut next_fixed = assignment.entries().iter().peekable();
        for position in 0..arity {
            if next_fixed
                .peek()
                .is_some_and(|&&(fixed_position, _)| fixed_position == position)
            {
                next_fixed.next();
            } else {
                free_positions.push(position);
            }
        }
        if next_fixed.next().is_some() {
            return Err(CylindricalOrderingError::InternalInvariant {
                detail: "canonical assignment was not exhausted by its authenticated arity",
            });
        }
        let stable_manifest = ordering_manifest(
            policy,
            &sector,
            &assignment,
            &free_positions,
            limits.max_manifest_bytes,
        )?;
        Ok(Self {
            schema: CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA,
            key_schema: RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
            policy,
            sector,
            assignment,
            free_positions: free_positions.into_boxed_slice(),
            limits,
            stable_manifest: stable_manifest.into(),
        })
    }

    fn key_for_shift_unreplayed(
        &self,
        shift: &IndexShift,
    ) -> Result<CylindricalIntegralComplexityKey, CylindricalOrderingError> {
        // Avoid recursion from `replay_key`: construct through the public path's
        // arithmetic with a temporary key and no final replay.
        if shift.arity() != self.arity() {
            return Err(CylindricalOrderingError::WrongShiftArity {
                expected: self.arity(),
                actual: shift.arity(),
            });
        }
        check_limit(
            "cylindrical order-key components",
            key_component_count(self.arity())?,
            self.limits.max_key_components,
        )?;
        let mut bits = Vec::new();
        try_reserve_key_entries("cylindrical order-key sector bits", &mut bits, self.arity())?;
        let mut excesses = Vec::new();
        try_reserve_key_entries(
            "cylindrical order-key signed excesses",
            &mut excesses,
            self.arity(),
        )?;
        let mut propagators = 0usize;
        let mut corner = 0i128;
        let mut dots = 0i128;
        let mut numerators = 0i128;
        let mut next_fixed = self.assignment.entries().iter().peekable();
        for (position, (&source_active, &displacement)) in self
            .sector
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let fixed = match next_fixed.peek() {
                Some(&&(fixed_position, value)) if fixed_position == position => {
                    next_fixed.next();
                    Some(value)
                }
                _ => None,
            };
            let (active, excess) = if let Some(value) = fixed {
                let shifted = value.checked_add(displacement).ok_or(
                    CylindricalOrderingError::FixedIndexOverflow {
                        position,
                        value,
                        displacement,
                    },
                )?;
                if shifted >= 1 {
                    (true, i128::from(shifted) - 1)
                } else {
                    (false, -i128::from(shifted))
                }
            } else if source_active {
                (true, i128::from(displacement))
            } else {
                (false, -i128::from(displacement))
            };
            bits.push(active);
            excesses.push(excess);
            corner = checked_signed_add(corner, excess, "signed corner-distance offset")?;
            if active {
                propagators = propagators.checked_add(1).ok_or(
                    CylindricalOrderingError::ResourceCountOverflow {
                        resource: "cylindrical propagator count",
                    },
                )?;
                dots = checked_signed_add(dots, excess, "signed dot offset")?;
            } else {
                numerators = checked_signed_add(numerators, excess, "signed numerator offset")?;
            }
        }
        if next_fixed.next().is_some() {
            return Err(CylindricalOrderingError::InternalInvariant {
                detail: "canonical assignment was not exhausted while constructing a key",
            });
        }
        let formal_sector = SectorMask::try_from_preallocated(bits).map_err(|_| {
            CylindricalOrderingError::InternalInvariant {
                detail: "a nonempty authenticated sector produced an empty formal sector",
            }
        })?;
        let mut retained_shift = Vec::new();
        try_reserve_key_entries(
            "cylindrical order-key shift components",
            &mut retained_shift,
            self.arity(),
        )?;
        retained_shift.extend_from_slice(shift.values());
        let retained_shift = IndexShift::try_from_preallocated(retained_shift, self.arity())
            .map_err(|_| CylindricalOrderingError::InternalInvariant {
                detail: "an authenticated shift changed arity while constructing its key",
            })?;
        Ok(CylindricalIntegralComplexityKey {
            schema: CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
            key_schema: self.key_schema,
            policy: self.policy,
            arity: self.arity(),
            propagators,
            formal_sector,
            corner_distance_offset: corner,
            dots_offset: dots,
            numerators_offset: numerators,
            signed_index_excess: excesses,
            shift: retained_shift,
            ordering_manifest: self.stable_manifest.clone(),
        })
    }
}

/// Exact normalized complexity key for one shift on a cylindrical start.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CylindricalIntegralComplexityKey {
    schema: &'static str,
    key_schema: &'static str,
    policy: IntegralOrderingPolicy,
    arity: usize,
    propagators: usize,
    formal_sector: SectorMask,
    corner_distance_offset: i128,
    dots_offset: i128,
    numerators_offset: i128,
    signed_index_excess: Vec<i128>,
    shift: IndexShift,
    ordering_manifest: Arc<str>,
}

impl CylindricalIntegralComplexityKey {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn key_schema(&self) -> &'static str {
        self.key_schema
    }

    pub const fn policy(&self) -> IntegralOrderingPolicy {
        self.policy
    }

    pub const fn arity(&self) -> usize {
        self.arity
    }

    pub const fn propagators(&self) -> usize {
        self.propagators
    }

    /// Formal source bits in free coordinates, exact shifted bits in fixed
    /// coordinates.
    pub const fn formal_sector(&self) -> &SectorMask {
        &self.formal_sector
    }

    pub const fn corner_distance_offset(&self) -> i128 {
        self.corner_distance_offset
    }

    pub const fn dots_offset(&self) -> i128 {
        self.dots_offset
    }

    pub const fn numerators_offset(&self) -> i128 {
        self.numerators_offset
    }

    pub fn signed_index_excess(&self) -> &[i128] {
        &self.signed_index_excess
    }

    pub const fn shift(&self) -> &IndexShift {
        &self.shift
    }

    pub fn ordering_manifest(&self) -> &str {
        &self.ordering_manifest
    }

    /// Complete bytes owned by this key. The ordering manifest allocation is
    /// shared; its `Arc` handle is already included in `size_of::<Self>()`.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(self.formal_sector.owned_retained_byte_bound()?)?
            .checked_add(
                self.signed_index_excess
                    .capacity()
                    .checked_mul(size_of::<i128>())?,
            )?
            .checked_add(self.shift.owned_retained_byte_bound()?)
    }

    pub(crate) fn into_shift(self) -> IndexShift {
        self.shift
    }

    /// Stable diagnostic encoding.  Certificates persist the typed key and
    /// ordering; this string makes schema drift visible in golden tests.
    pub fn to_stable_string(&self) -> String {
        let excess = signed_values_string(&self.signed_index_excess);
        let shift = signed_values_string(self.shift.values());
        format!(
            "{}|ordering-bytes={}|ordering={}|arity={}|propagators={}|sector={}|corner-offset={}|dots-offset={}|numerators-offset={}|excess=[{}]|shift=[{}]",
            self.schema,
            self.ordering_manifest.len(),
            self.ordering_manifest,
            self.arity,
            self.propagators,
            self.formal_sector,
            self.corner_distance_offset,
            self.dots_offset,
            self.numerators_offset,
            excess,
            shift,
        )
    }
}

impl Ord for CylindricalIntegralComplexityKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.policy
            .cmp(&other.policy)
            .then_with(|| self.arity.cmp(&other.arity))
            .then_with(|| self.propagators.cmp(&other.propagators))
            .then_with(|| self.formal_sector.cmp(&other.formal_sector))
            .then_with(|| {
                self.corner_distance_offset
                    .cmp(&other.corner_distance_offset)
            })
            .then_with(|| self.dots_offset.cmp(&other.dots_offset))
            .then_with(|| self.numerators_offset.cmp(&other.numerators_offset))
            .then_with(|| self.signed_index_excess.cmp(&other.signed_index_excess))
            .then_with(|| self.shift.cmp(&other.shift))
            // Cross-context comparison has no reduction semantics, but these
            // final replay fields make `Ord` a strict total order globally.
            .then_with(|| self.ordering_manifest.cmp(&other.ordering_manifest))
            .then_with(|| self.schema.cmp(other.schema))
            .then_with(|| self.key_schema.cmp(other.key_schema))
    }
}

impl PartialOrd for CylindricalIntegralComplexityKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CylindricalOrderingError {
    WrongAssignmentArity {
        expected: usize,
        actual: usize,
    },
    WrongShiftArity {
        expected: usize,
        actual: usize,
    },
    FixedAssignmentOutsideSourceSector {
        position: usize,
        value: i64,
        source_active: bool,
    },
    FixedIndexOverflow {
        position: usize,
        value: i64,
        displacement: i64,
    },
    SignedComplexityOverflow {
        measure: &'static str,
    },
    ResourceLimitExceeded {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SchemaMismatch,
    KeyOrderingMismatch,
    ReplayMismatch,
    InternalInvariant {
        detail: &'static str,
    },
}

impl fmt::Display for CylindricalOrderingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongAssignmentArity { expected, actual } => write!(
                formatter,
                "cylindrical assignment arity is {actual}, expected {expected}"
            ),
            Self::WrongShiftArity { expected, actual } => write!(
                formatter,
                "cylindrical shift arity is {actual}, expected {expected}"
            ),
            Self::FixedAssignmentOutsideSourceSector {
                position,
                value,
                source_active,
            } => write!(
                formatter,
                "fixed index {position}={value} is outside the source {} half-line",
                if *source_active { "active" } else { "inactive" }
            ),
            Self::FixedIndexOverflow {
                position,
                value,
                displacement,
            } => write!(
                formatter,
                "fixed index {position} overflowed while adding {value} and shift {displacement}"
            ),
            Self::SignedComplexityOverflow { measure } => {
                write!(formatter, "cylindrical {measure} overflowed i128")
            }
            Self::ResourceLimitExceeded {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested}, exceeding configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} could not reserve {requested} elements"
            ),
            Self::SchemaMismatch => formatter.write_str("cylindrical ordering schema mismatch"),
            Self::KeyOrderingMismatch => {
                formatter.write_str("cylindrical key belongs to another ordering context")
            }
            Self::ReplayMismatch => formatter.write_str("cylindrical ordering replay mismatch"),
            Self::InternalInvariant { detail } => {
                write!(formatter, "cylindrical ordering invariant failed: {detail}")
            }
        }
    }
}

impl std::error::Error for CylindricalOrderingError {}

fn key_component_count(arity: usize) -> Result<usize, CylindricalOrderingError> {
    arity
        .checked_mul(KEY_COMPONENT_VECTORS)
        .and_then(|vectors| vectors.checked_add(KEY_FIXED_COMPONENTS))
        .ok_or(CylindricalOrderingError::ResourceCountOverflow {
            resource: "cylindrical order-key components",
        })
}

fn try_reserve_key_entries<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), CylindricalOrderingError> {
    let requested = values
        .len()
        .checked_add(additional)
        .ok_or(CylindricalOrderingError::ResourceCountOverflow { resource })?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| CylindricalOrderingError::AllocationFailure {
            resource,
            requested,
        })
}

fn checked_signed_add(
    left: i128,
    right: i128,
    measure: &'static str,
) -> Result<i128, CylindricalOrderingError> {
    left.checked_add(right)
        .ok_or(CylindricalOrderingError::SignedComplexityOverflow { measure })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CylindricalOrderingError> {
    if requested > limit {
        Err(CylindricalOrderingError::ResourceLimitExceeded {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn ordering_manifest(
    policy: IntegralOrderingPolicy,
    sector: &SectorMask,
    assignment: &PartialIndexAssignment,
    free_positions: &[usize],
    max_bytes: usize,
) -> Result<String, CylindricalOrderingError> {
    let mut output = String::new();
    push_manifest(
        &mut output,
        CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA,
        max_bytes,
    )?;
    push_manifest(&mut output, "|policy=", max_bytes)?;
    push_manifest(&mut output, policy.stable_id(), max_bytes)?;
    push_manifest(&mut output, "|key-schema=", max_bytes)?;
    push_manifest(
        &mut output,
        RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
        max_bytes,
    )?;
    push_manifest(&mut output, "|sector=", max_bytes)?;
    push_manifest(&mut output, &sector.to_bit_string(), max_bytes)?;
    push_manifest(&mut output, "|assignment=[", max_bytes)?;
    for (ordinal, &(position, value)) in assignment.entries().iter().enumerate() {
        if ordinal != 0 {
            push_manifest(&mut output, ",", max_bytes)?;
        }
        let entry = format!("{position}:{value}");
        push_manifest(&mut output, &entry, max_bytes)?;
    }
    push_manifest(&mut output, "]|free=[", max_bytes)?;
    for (ordinal, position) in free_positions.iter().enumerate() {
        if ordinal != 0 {
            push_manifest(&mut output, ",", max_bytes)?;
        }
        let position = position.to_string();
        push_manifest(&mut output, &position, max_bytes)?;
    }
    push_manifest(&mut output, "]", max_bytes)?;
    Ok(output)
}

fn push_manifest(
    output: &mut String,
    fragment: &str,
    limit: usize,
) -> Result<(), CylindricalOrderingError> {
    let requested = output.len().checked_add(fragment.len()).ok_or(
        CylindricalOrderingError::ResourceCountOverflow {
            resource: "cylindrical ordering manifest bytes",
        },
    )?;
    check_limit("cylindrical ordering manifest bytes", requested, limit)?;
    output.push_str(fragment);
    Ok(())
}

fn signed_values_string<T: fmt::Display>(values: &[T]) -> String {
    let mut output = String::new();
    for (ordinal, value) in values.iter().enumerate() {
        if ordinal != 0 {
            output.push(',');
        }
        write!(&mut output, "{value}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordering() -> CylindricalParametricEliminationOrdering {
        CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            SectorMask::try_from_bit_string("101").unwrap(),
            PartialIndexAssignment::try_new([(1, -2)], 3, 1).unwrap(),
            CylindricalOrderingLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn replay_rejects_every_private_schema_or_geometry_tamper() {
        let original = ordering();
        original.replay().unwrap();

        let mut tampered = original.clone();
        tampered.schema = "rustred-cylindrical-parametric-elimination-ordering-v999";
        assert_eq!(
            tampered.replay(),
            Err(CylindricalOrderingError::SchemaMismatch)
        );

        let mut tampered = original.clone();
        tampered.key_schema = "different-key-fields";
        assert_eq!(
            tampered.replay(),
            Err(CylindricalOrderingError::SchemaMismatch)
        );

        let mut tampered = original.clone();
        tampered.free_positions = vec![2, 0].into_boxed_slice();
        assert_eq!(
            tampered.replay(),
            Err(CylindricalOrderingError::ReplayMismatch)
        );

        let mut tampered = original.clone();
        tampered.stable_manifest = Arc::from("detached-manifest");
        assert_eq!(
            tampered.replay(),
            Err(CylindricalOrderingError::ReplayMismatch)
        );
    }

    #[test]
    fn replay_rejects_key_payload_and_context_tamper() {
        let original = ordering();
        let shift = IndexShift::try_new([-2, 3, 1], 3).unwrap();
        let key = original.key_for_shift(&shift).unwrap();
        original.replay_key(&key).unwrap();

        let mut tampered = key.clone();
        tampered.corner_distance_offset += 1;
        assert_eq!(
            original.replay_key(&tampered),
            Err(CylindricalOrderingError::ReplayMismatch)
        );

        let other = CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            SectorMask::try_from_bit_string("101").unwrap(),
            PartialIndexAssignment::try_new([(1, -3)], 3, 1).unwrap(),
            CylindricalOrderingLimits::default(),
        )
        .unwrap();
        assert_eq!(
            other.replay_key(&key),
            Err(CylindricalOrderingError::KeyOrderingMismatch)
        );
    }

    #[test]
    fn scalar_and_component_count_overflows_fail_closed() {
        assert_eq!(
            checked_signed_add(i128::MAX, 1, "adversarial signed sum"),
            Err(CylindricalOrderingError::SignedComplexityOverflow {
                measure: "adversarial signed sum",
            })
        );
        assert_eq!(
            key_component_count(usize::MAX),
            Err(CylindricalOrderingError::ResourceCountOverflow {
                resource: "cylindrical order-key components",
            })
        );
    }
}
