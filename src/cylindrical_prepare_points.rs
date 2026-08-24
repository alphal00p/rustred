//! Replayable LiteRed-style `preparepoints` layers for an integer cylinder.
//!
//! Only coordinates fixed by [`crate::PartialIndexAssignment`] are checked
//! against the source sector after a displacement.  Free coordinates remain
//! formal and are never filtered through an invented corner.  Each retained
//! exact L1 shell is ordered by [`crate::CylindricalParametricEliminationOrdering`].

use std::fmt;
use std::sync::Arc;

use crate::{
    CylindricalIntegralComplexityKey, CylindricalOrderingError,
    CylindricalParametricEliminationOrdering, IndexShift, ParametricRelationError,
};

pub const CYLINDRICAL_PREPARE_POINT_LAYER_V1_SCHEMA: &str =
    "rustred-cylindrical-prepare-point-layer-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CylindricalPreparePointLimits {
    pub max_depth: usize,
    pub max_enumeration_steps: usize,
    pub max_enumerated_offsets: usize,
    pub max_enumerated_components: usize,
    pub max_fixed_sector_checks: usize,
    pub max_retained_points: usize,
    pub max_retained_components: usize,
    pub max_order_key_components: usize,
    pub max_order_comparisons: usize,
}

impl Default for CylindricalPreparePointLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_enumeration_steps: 100_000_000,
            max_enumerated_offsets: 16_000_000,
            max_enumerated_components: 256_000_000,
            max_fixed_sector_checks: 256_000_000,
            max_retained_points: 16_000_000,
            max_retained_components: 256_000_000,
            max_order_key_components: 256_000_000,
            max_order_comparisons: 256_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CylindricalPreparePointStats {
    enumeration_steps: usize,
    enumerated_offsets: usize,
    enumerated_components: usize,
    fixed_sector_checks: usize,
    rejected_fixed_sector_offsets: usize,
    retained_points: usize,
    retained_components: usize,
    order_key_components: usize,
    order_comparisons: usize,
}

impl CylindricalPreparePointStats {
    pub const fn enumeration_steps(self) -> usize {
        self.enumeration_steps
    }
    pub const fn enumerated_offsets(self) -> usize {
        self.enumerated_offsets
    }
    pub const fn enumerated_components(self) -> usize {
        self.enumerated_components
    }
    pub const fn fixed_sector_checks(self) -> usize {
        self.fixed_sector_checks
    }
    pub const fn rejected_fixed_sector_offsets(self) -> usize {
        self.rejected_fixed_sector_offsets
    }
    pub const fn retained_points(self) -> usize {
        self.retained_points
    }
    pub const fn retained_components(self) -> usize {
        self.retained_components
    }
    pub const fn order_key_components(self) -> usize {
        self.order_key_components
    }
    pub const fn order_comparisons(self) -> usize {
        self.order_comparisons
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CylindricalPreparePointLayer {
    schema: &'static str,
    // Every layer in a cumulative schedule refers to the same potentially
    // large, authenticated ordering payload.  Keep that ownership shallow:
    // cloning a layer must not clone the assignment, free-position table, or
    // stable manifest again.
    ordering: Arc<CylindricalParametricEliminationOrdering>,
    depth: usize,
    ordered_translations: Box<[IndexShift]>,
    limits: CylindricalPreparePointLimits,
    stats: CylindricalPreparePointStats,
}

impl CylindricalPreparePointLayer {
    pub fn compile(
        ordering: CylindricalParametricEliminationOrdering,
        depth: usize,
        limits: CylindricalPreparePointLimits,
    ) -> Result<Self, CylindricalPreparePointError> {
        ordering.replay()?;
        let result = compile_unreplayed(Arc::new(ordering), depth, limits)?;
        result.replay_with_replayed_ordering()?;
        Ok(result)
    }

    /// Construct one layer from an ordering already replayed by its owner.
    ///
    /// This is crate-private specifically so a schedule can authenticate its
    /// shared ordering once and avoid recursively invoking every child
    /// layer's public compile-and-replay path.
    pub(crate) fn compile_with_replayed_shared_ordering(
        ordering: Arc<CylindricalParametricEliminationOrdering>,
        depth: usize,
        limits: CylindricalPreparePointLimits,
    ) -> Result<Self, CylindricalPreparePointError> {
        compile_unreplayed(ordering, depth, limits)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn ordering(&self) -> &CylindricalParametricEliminationOrdering {
        self.ordering.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn ordering_arc(&self) -> &Arc<CylindricalParametricEliminationOrdering> {
        &self.ordering
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
    pub fn ordered_translations(&self) -> &[IndexShift] {
        &self.ordered_translations
    }
    pub const fn limits(&self) -> CylindricalPreparePointLimits {
        self.limits
    }
    pub const fn stats(&self) -> CylindricalPreparePointStats {
        self.stats
    }

    pub fn replay(&self) -> Result<(), CylindricalPreparePointError> {
        if self.schema != CYLINDRICAL_PREPARE_POINT_LAYER_V1_SCHEMA {
            return Err(CylindricalPreparePointError::SchemaMismatch);
        }
        self.ordering.replay()?;
        self.replay_with_replayed_ordering()
    }

    fn replay_with_replayed_ordering(&self) -> Result<(), CylindricalPreparePointError> {
        let replayed = compile_unreplayed(self.ordering.clone(), self.depth, self.limits)?;
        if replayed == *self {
            Ok(())
        } else {
            Err(CylindricalPreparePointError::ReplayMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CylindricalPreparePointError {
    DepthTooLarge {
        requested: usize,
        limit: usize,
    },
    FixedIndexOverflow {
        position: usize,
        value: i64,
        displacement: i64,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    SchemaMismatch,
    ReplayMismatch,
    Ordering(CylindricalOrderingError),
    Relation(ParametricRelationError),
}

impl fmt::Display for CylindricalPreparePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthTooLarge { requested, limit } => write!(
                formatter,
                "cylindrical prepare-point depth {requested} exceeds configured limit {limit}"
            ),
            Self::FixedIndexOverflow {
                position,
                value,
                displacement,
            } => write!(
                formatter,
                "fixed index {position} overflowed while adding {value} and displacement {displacement}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "cylindrical {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "cylindrical {resource} count overflowed usize")
            }
            Self::SchemaMismatch => {
                formatter.write_str("cylindrical prepare-point schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("cylindrical prepare-point layer does not replay")
            }
            Self::Ordering(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CylindricalPreparePointError {}

impl From<CylindricalOrderingError> for CylindricalPreparePointError {
    fn from(value: CylindricalOrderingError) -> Self {
        Self::Ordering(value)
    }
}

impl From<ParametricRelationError> for CylindricalPreparePointError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

fn compile_unreplayed(
    ordering: Arc<CylindricalParametricEliminationOrdering>,
    depth: usize,
    limits: CylindricalPreparePointLimits,
) -> Result<CylindricalPreparePointLayer, CylindricalPreparePointError> {
    if depth > limits.max_depth {
        return Err(CylindricalPreparePointError::DepthTooLarge {
            requested: depth,
            limit: limits.max_depth,
        });
    }
    let arity = ordering.arity();
    check_limit(
        "enumerated prepare-point components",
        arity,
        limits.max_enumerated_components,
    )?;
    let depth_i64 =
        i64::try_from(depth).map_err(|_| CylindricalPreparePointError::ResourceCountOverflow {
            resource: "prepare-point depth magnitude",
        })?;

    #[derive(Clone, Copy)]
    struct Frame {
        position: usize,
        remaining: i64,
        next_magnitude: i64,
        /// For a nonzero magnitude, emit the negative branch first and retain
        /// this flag until the positive branch has been visited.
        positive_pending: bool,
    }

    let mut stats = CylindricalPreparePointStats::default();
    let mut current = vec![0i64; arity];
    let mut retained = Vec::new();
    let mut stack = vec![Frame {
        position: 0,
        remaining: depth_i64,
        next_magnitude: 0,
        positive_pending: false,
    }];
    while let Some(frame) = stack.last().copied() {
        stats.enumeration_steps = bounded_add(
            "prepare-point enumeration steps",
            stats.enumeration_steps,
            1,
            limits.max_enumeration_steps,
        )?;
        // The last magnitude is forced to equal `remaining`; only its one or
        // two sign variants exist.  Earlier positions enumerate weak
        // compositions of the depth and sign only nonzero parts.  This is the
        // direct heap-resident counterpart of LiteRed's `diamond`, without
        // walking dead points in the enclosing L1 ball.
        if frame.position + 1 == arity {
            stack.pop();
            if frame.remaining == 0 {
                current[frame.position] = 0;
                consider_offset(&ordering, &current, &mut retained, &mut stats, limits)?;
            } else {
                current[frame.position] = -frame.remaining;
                consider_offset(&ordering, &current, &mut retained, &mut stats, limits)?;
                current[frame.position] = frame.remaining;
                consider_offset(&ordering, &current, &mut retained, &mut stats, limits)?;
            }
            continue;
        }
        if frame.next_magnitude > frame.remaining {
            stack.pop();
            continue;
        }
        let magnitude = frame.next_magnitude;
        let parent = stack
            .last_mut()
            .expect("copied prepare-point frame remains present");
        let value = if magnitude == 0 {
            parent.next_magnitude = 1;
            0
        } else if !frame.positive_pending {
            parent.positive_pending = true;
            -magnitude
        } else {
            parent.positive_pending = false;
            parent.next_magnitude = magnitude.checked_add(1).ok_or(
                CylindricalPreparePointError::ResourceCountOverflow {
                    resource: "prepare-point magnitude enumeration",
                },
            )?;
            magnitude
        };
        let remaining = frame.remaining.checked_sub(magnitude).ok_or(
            CylindricalPreparePointError::ResourceCountOverflow {
                resource: "prepare-point remaining magnitude",
            },
        )?;
        current[frame.position] = value;
        stack.push(Frame {
            position: frame.position + 1,
            remaining,
            next_magnitude: 0,
            positive_pending: false,
        });
    }

    // Compute each exact key once, then use an explicit stable insertion sort.
    // Its comparison schedule is part of this V1 replay protocol and cannot
    // drift when the standard library changes sorting implementations.
    let components_per_key = key_component_count(arity)?;
    stats.order_key_components = checked_mul(
        "prepare-point order-key components",
        retained.len(),
        components_per_key,
    )?;
    check_limit(
        "prepare-point order-key components",
        stats.order_key_components,
        limits.max_order_key_components,
    )?;
    let mut decorated = Vec::with_capacity(retained.len());
    for shift in retained {
        decorated.push((ordering.key_for_shift(&shift)?, shift));
    }
    stable_insertion_sort(&mut decorated, &mut stats, limits)?;
    let retained = decorated
        .into_iter()
        .map(|(_, shift)| shift)
        .collect::<Vec<_>>();

    Ok(CylindricalPreparePointLayer {
        schema: CYLINDRICAL_PREPARE_POINT_LAYER_V1_SCHEMA,
        ordering,
        depth,
        ordered_translations: retained.into_boxed_slice(),
        limits,
        stats,
    })
}

fn consider_offset(
    ordering: &CylindricalParametricEliminationOrdering,
    current: &[i64],
    retained: &mut Vec<IndexShift>,
    stats: &mut CylindricalPreparePointStats,
    limits: CylindricalPreparePointLimits,
) -> Result<(), CylindricalPreparePointError> {
    let arity = ordering.arity();
    stats.enumerated_offsets = bounded_add(
        "enumerated prepare-point offsets",
        stats.enumerated_offsets,
        1,
        limits.max_enumerated_offsets,
    )?;
    stats.enumerated_components = bounded_add(
        "enumerated prepare-point components",
        stats.enumerated_components,
        arity,
        limits.max_enumerated_components,
    )?;
    if fixed_coordinates_stay_in_sector(ordering, current, stats, limits)? {
        stats.retained_points = bounded_add(
            "retained prepare points",
            stats.retained_points,
            1,
            limits.max_retained_points,
        )?;
        stats.retained_components = bounded_add(
            "retained prepare-point components",
            stats.retained_components,
            arity,
            limits.max_retained_components,
        )?;
        retained.push(IndexShift::try_new(current.iter().copied(), arity)?);
    } else {
        stats.rejected_fixed_sector_offsets = checked_add(
            "rejected fixed-sector prepare-point offsets",
            stats.rejected_fixed_sector_offsets,
            1,
        )?;
    }
    Ok(())
}

fn stable_insertion_sort(
    values: &mut [(CylindricalIntegralComplexityKey, IndexShift)],
    stats: &mut CylindricalPreparePointStats,
    limits: CylindricalPreparePointLimits,
) -> Result<(), CylindricalPreparePointError> {
    for insertion in 1..values.len() {
        let mut position = insertion;
        while position != 0 {
            stats.order_comparisons = bounded_add(
                "prepare-point order comparisons",
                stats.order_comparisons,
                1,
                limits.max_order_comparisons,
            )?;
            if values[position - 1].0 <= values[position].0 {
                break;
            }
            values.swap(position - 1, position);
            position -= 1;
        }
    }
    Ok(())
}

fn key_component_count(arity: usize) -> Result<usize, CylindricalPreparePointError> {
    checked_add(
        "prepare-point order-key components",
        checked_mul("prepare-point order-key components", arity, 3)?,
        5,
    )
}

fn fixed_coordinates_stay_in_sector(
    ordering: &CylindricalParametricEliminationOrdering,
    offset: &[i64],
    stats: &mut CylindricalPreparePointStats,
    limits: CylindricalPreparePointLimits,
) -> Result<bool, CylindricalPreparePointError> {
    for &(position, value) in ordering.assignment().entries() {
        stats.fixed_sector_checks = bounded_add(
            "fixed-coordinate sector checks",
            stats.fixed_sector_checks,
            1,
            limits.max_fixed_sector_checks,
        )?;
        let displacement = offset[position];
        let shifted = value.checked_add(displacement).ok_or(
            CylindricalPreparePointError::FixedIndexOverflow {
                position,
                value,
                displacement,
            },
        )?;
        if (shifted >= 1) != ordering.sector().active_bits()[position] {
            return Ok(false);
        }
    }
    Ok(true)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CylindricalPreparePointError> {
    left.checked_add(right)
        .ok_or(CylindricalPreparePointError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CylindricalPreparePointError> {
    left.checked_mul(right)
        .ok_or(CylindricalPreparePointError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, CylindricalPreparePointError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CylindricalPreparePointError> {
    if requested > limit {
        Err(CylindricalPreparePointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CylindricalOrderingLimits, IntegralOrderingPolicy, PartialIndexAssignment, SectorMask,
    };

    fn fixture() -> CylindricalPreparePointLayer {
        let ordering = CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            SectorMask::try_from_bit_string("10").unwrap(),
            PartialIndexAssignment::try_new([(0, 1)], 2, 1).unwrap(),
            CylindricalOrderingLimits::default(),
        )
        .unwrap();
        CylindricalPreparePointLayer::compile(ordering, 2, CylindricalPreparePointLimits::default())
            .unwrap()
    }

    #[test]
    fn replay_rejects_private_schema_order_and_stats_tampering() {
        let original = fixture();
        original.replay().unwrap();

        let mut tampered = original.clone();
        tampered.schema = "rustred-cylindrical-prepare-point-layer-v999";
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointError::SchemaMismatch)
        );

        let mut tampered = original.clone();
        tampered.ordered_translations.reverse();
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointError::ReplayMismatch)
        );

        let mut tampered = original;
        tampered.stats.order_comparisons += 1;
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointError::ReplayMismatch)
        );
    }
}
