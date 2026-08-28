//! Replayable exact `preparepoints` shell for one integer-affine residual start.
//!
//! Ambient displacements are enumerated on the exact L1 shell.  A displacement
//! is rejected only when a zero matrix row of the affine start crosses its
//! authenticated source-sector half-line.  Nonconstant rows are never sampled
//! or sign-filtered. Boolean-branch nonzero guards remain attached to the
//! ordering and are not composed through translations; a retained offset is
//! therefore not a certificate that `F(t)+q` remains in the branch.

use std::fmt;

use crate::affine_parametric_ordering::AffineParametricOrderingAlgebra;
use crate::{
    AffineParametricOrderingError, AffineStartIntegralComplexityKey, IndexShift,
    ParametricRelationError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffinePreparePointLimits {
    pub max_depth: usize,
    pub max_enumeration_steps: usize,
    pub max_enumerated_offsets: usize,
    pub max_enumerated_components: usize,
    pub max_constant_sector_checks: usize,
    pub max_retained_points: usize,
    pub max_retained_components: usize,
    pub max_order_key_components: usize,
    /// Exact cumulative magnitude bits materialized in all temporary
    /// arbitrary-precision ordering keys for this layer.
    pub max_order_key_integer_bits: usize,
    pub max_order_comparisons: usize,
    /// Cumulative conservative GMP comparison work.  Before every key
    /// comparison the layer charges the sum of the two keys' complete
    /// retained integer-payload bit censuses, even when an earlier,
    /// non-integer key component will decide the comparison.
    pub max_order_comparison_integer_bit_work: usize,
}

impl Default for AffinePreparePointLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_enumeration_steps: 100_000_000,
            max_enumerated_offsets: 16_000_000,
            max_enumerated_components: 256_000_000,
            max_constant_sector_checks: 256_000_000,
            max_retained_points: 16_000_000,
            max_retained_components: 256_000_000,
            max_order_key_components: 256_000_000,
            max_order_key_integer_bits: 512 * 1024 * 1024,
            max_order_comparisons: 256_000_000,
            max_order_comparison_integer_bit_work: 8_000_000_000_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffinePreparePointStats {
    enumeration_steps: usize,
    enumerated_offsets: usize,
    enumerated_components: usize,
    constant_sector_checks: usize,
    rejected_constant_sector_offsets: usize,
    retained_points: usize,
    retained_components: usize,
    order_key_components: usize,
    order_key_integer_bits: usize,
    order_comparisons: usize,
    order_comparison_integer_bit_work: usize,
}

impl AffinePreparePointStats {
    pub const fn enumeration_steps(self) -> usize {
        self.enumeration_steps
    }
    pub const fn enumerated_offsets(self) -> usize {
        self.enumerated_offsets
    }
    pub const fn enumerated_components(self) -> usize {
        self.enumerated_components
    }
    pub const fn constant_sector_checks(self) -> usize {
        self.constant_sector_checks
    }
    pub const fn rejected_constant_sector_offsets(self) -> usize {
        self.rejected_constant_sector_offsets
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
    pub const fn order_key_integer_bits(self) -> usize {
        self.order_key_integer_bits
    }
    pub const fn order_comparisons(self) -> usize {
        self.order_comparisons
    }
    /// Exact sum of the conservative charge made before every attempted key
    /// comparison in this layer.
    pub const fn order_comparison_integer_bit_work(self) -> usize {
        self.order_comparison_integer_bit_work
    }
}

/// Sealed source-neutral ordering operations required by exact L1-shell
/// preparation. Implementations are authenticated before entering the core;
/// the core therefore neither knows nor fabricates a V1 source locator or a
/// generated-inventory case handle.
pub(crate) trait AffinePreparePointOrdering {
    fn prepare_point_arity(&self) -> usize;
    fn prepare_point_constant_positions(&self) -> &[usize];
    fn prepare_point_max_key_total_integer_bits(&self) -> usize;
    fn prepare_point_constant_row_stays_in_sector(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<bool, AffineParametricOrderingError>;
    fn prepare_point_key_for_owned_shift(
        &self,
        shift: IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError>;
}

impl AffinePreparePointOrdering for AffineParametricOrderingAlgebra<'_, '_> {
    fn prepare_point_arity(&self) -> usize {
        self.arity()
    }

    fn prepare_point_constant_positions(&self) -> &[usize] {
        self.constant_positions()
    }

    fn prepare_point_max_key_total_integer_bits(&self) -> usize {
        self.max_key_total_integer_bits()
    }

    fn prepare_point_constant_row_stays_in_sector(
        &self,
        position: usize,
        displacement: i64,
    ) -> Result<bool, AffineParametricOrderingError> {
        self.constant_row_shift_stays_in_source_sector(position, displacement)
    }

    fn prepare_point_key_for_owned_shift(
        &self,
        shift: IndexShift,
        max_retained_total_integer_bits: usize,
    ) -> Result<AffineStartIntegralComplexityKey, AffineParametricOrderingError> {
        self.key_for_owned_shift(shift, max_retained_total_integer_bits)
    }
}

pub(crate) struct AffinePreparePointLayerCore<P = IndexShift> {
    depth: usize,
    ordered_points: Vec<P>,
    limits: AffinePreparePointLimits,
    stats: AffinePreparePointStats,
}

pub(crate) type AffinePreparePointLayerKeyCore =
    AffinePreparePointLayerCore<AffineStartIntegralComplexityKey>;

impl<P> AffinePreparePointLayerCore<P> {
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }

    pub(crate) const fn limits(&self) -> AffinePreparePointLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> AffinePreparePointStats {
        self.stats
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        usize,
        Vec<P>,
        AffinePreparePointLimits,
        AffinePreparePointStats,
    ) {
        (self.depth, self.ordered_points, self.limits, self.stats)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffinePreparePointError {
    DepthTooLarge {
        requested: usize,
        limit: usize,
    },
    ResourceLimit {
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
    ReplayMismatch,
    Ordering(AffineParametricOrderingError),
    Relation(ParametricRelationError),
}

impl fmt::Display for AffinePreparePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthTooLarge { requested, limit } => write!(
                formatter,
                "affine prepare-point depth {requested} exceeds configured limit {limit}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "affine prepare-point {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "affine prepare-point {resource} count overflowed usize"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "affine prepare-point {resource} could not reserve {requested} entries"
            ),
            Self::SchemaMismatch => {
                formatter.write_str("affine prepare-point layer schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("affine prepare-point layer does not replay")
            }
            Self::Ordering(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AffinePreparePointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ordering(error) => Some(error),
            Self::Relation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AffineParametricOrderingError> for AffinePreparePointError {
    fn from(value: AffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}

impl From<ParametricRelationError> for AffinePreparePointError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

#[derive(Clone, Copy)]
struct EnumerationFrame {
    position: usize,
    remaining: i64,
    next_magnitude: i64,
    positive_pending: bool,
}

pub(crate) fn compile_affine_prepare_point_layer_core<O>(
    ordering: &O,
    depth: usize,
    limits: AffinePreparePointLimits,
) -> Result<AffinePreparePointLayerCore, AffinePreparePointError>
where
    O: AffinePreparePointOrdering + ?Sized,
{
    let core = compile_affine_prepare_point_layer_key_core(ordering, depth, limits)?;
    let (depth, decorated, limits, stats) = core.into_parts();
    let mut ordered = Vec::new();
    try_reserve_exact(
        "ordered affine prepare points",
        &mut ordered,
        decorated.len(),
    )?;
    for key in decorated {
        ordered.push(key.into_shift()?);
    }
    Ok(AffinePreparePointLayerCore {
        depth,
        ordered_points: ordered,
        limits,
        stats,
    })
}

pub(crate) fn compile_affine_prepare_point_layer_key_core<O>(
    ordering: &O,
    depth: usize,
    limits: AffinePreparePointLimits,
) -> Result<AffinePreparePointLayerKeyCore, AffinePreparePointError>
where
    O: AffinePreparePointOrdering + ?Sized,
{
    if depth > limits.max_depth {
        return Err(AffinePreparePointError::DepthTooLarge {
            requested: depth,
            limit: limits.max_depth,
        });
    }
    let arity = ordering.prepare_point_arity();
    check_limit(
        "enumerated prepare-point components",
        arity,
        limits.max_enumerated_components,
    )?;
    let depth_i64 =
        i64::try_from(depth).map_err(|_| AffinePreparePointError::ResourceCountOverflow {
            resource: "prepare-point depth magnitude",
        })?;

    let mut current = Vec::new();
    try_reserve_exact("prepare-point current offset", &mut current, arity)?;
    current.resize(arity, 0i64);
    let mut retained = Vec::new();
    let mut stack = Vec::new();
    let stack_capacity = checked_add("prepare-point traversal stack", arity, 1)?;
    try_reserve_exact("prepare-point traversal stack", &mut stack, stack_capacity)?;
    stack.push(EnumerationFrame {
        position: 0,
        remaining: depth_i64,
        next_magnitude: 0,
        positive_pending: false,
    });
    let mut stats = AffinePreparePointStats::default();

    while let Some(frame) = stack.last().copied() {
        stats.enumeration_steps = bounded_add(
            "prepare-point enumeration steps",
            stats.enumeration_steps,
            1,
            limits.max_enumeration_steps,
        )?;
        if frame.position + 1 == arity {
            stack.pop();
            if frame.remaining == 0 {
                current[frame.position] = 0;
                consider_offset(ordering, &current, &mut retained, &mut stats, limits)?;
            } else {
                current[frame.position] = -frame.remaining;
                consider_offset(ordering, &current, &mut retained, &mut stats, limits)?;
                current[frame.position] = frame.remaining;
                consider_offset(ordering, &current, &mut retained, &mut stats, limits)?;
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
            .ok_or(AffinePreparePointError::ReplayMismatch)?;
        let value = if magnitude == 0 {
            parent.next_magnitude = 1;
            0
        } else if !frame.positive_pending {
            parent.positive_pending = true;
            -magnitude
        } else {
            parent.positive_pending = false;
            parent.next_magnitude =
                magnitude
                    .checked_add(1)
                    .ok_or(AffinePreparePointError::ResourceCountOverflow {
                        resource: "prepare-point magnitude enumeration",
                    })?;
            magnitude
        };
        current[frame.position] = value;
        stack.push(EnumerationFrame {
            position: frame.position + 1,
            remaining: frame.remaining.checked_sub(magnitude).ok_or(
                AffinePreparePointError::ResourceCountOverflow {
                    resource: "prepare-point remaining magnitude",
                },
            )?,
            next_magnitude: 0,
            positive_pending: false,
        });
    }

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
    let mut decorated = Vec::new();
    try_reserve_exact(
        "decorated affine prepare points",
        &mut decorated,
        retained.len(),
    )?;
    for shift in retained {
        let remaining_integer_bits = limits
            .max_order_key_integer_bits
            .checked_sub(stats.order_key_integer_bits)
            .ok_or(AffinePreparePointError::ReplayMismatch)?;
        let key = match ordering.prepare_point_key_for_owned_shift(shift, remaining_integer_bits) {
            Err(AffineParametricOrderingError::ResourceLimit {
                resource: "affine key total integer bits",
                requested,
                limit,
            }) if remaining_integer_bits < ordering.prepare_point_max_key_total_integer_bits()
                && limit == remaining_integer_bits =>
            {
                let requested = checked_add(
                    "prepare-point order-key integer bits",
                    stats.order_key_integer_bits,
                    requested,
                )?;
                return Err(AffinePreparePointError::ResourceLimit {
                    resource: "prepare-point order-key integer bits",
                    requested,
                    limit: limits.max_order_key_integer_bits,
                });
            }
            Err(error) => return Err(error.into()),
            Ok(key) => key,
        };
        stats.order_key_integer_bits = bounded_add(
            "prepare-point order-key integer bits",
            stats.order_key_integer_bits,
            key.retained_integer_bits(),
            limits.max_order_key_integer_bits,
        )?;
        decorated.push(key);
    }
    stable_insertion_sort(&mut decorated, &mut stats, limits)?;

    Ok(AffinePreparePointLayerCore {
        depth,
        ordered_points: decorated,
        limits,
        stats,
    })
}

fn consider_offset<O>(
    ordering: &O,
    current: &[i64],
    retained: &mut Vec<IndexShift>,
    stats: &mut AffinePreparePointStats,
    limits: AffinePreparePointLimits,
) -> Result<(), AffinePreparePointError>
where
    O: AffinePreparePointOrdering + ?Sized,
{
    let arity = ordering.prepare_point_arity();
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
    if constant_rows_stay_in_sector(ordering, current, stats, limits)? {
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
        try_reserve_exact("retained affine prepare points", retained, 1)?;
        let mut values = Vec::new();
        try_reserve_exact(
            "retained affine prepare-point shift components",
            &mut values,
            arity,
        )?;
        values.extend_from_slice(current);
        retained.push(IndexShift::try_from_preallocated(values, arity)?);
    } else {
        stats.rejected_constant_sector_offsets = checked_add(
            "rejected constant-sector prepare-point offsets",
            stats.rejected_constant_sector_offsets,
            1,
        )?;
    }
    Ok(())
}

fn constant_rows_stay_in_sector<O>(
    ordering: &O,
    offset: &[i64],
    stats: &mut AffinePreparePointStats,
    limits: AffinePreparePointLimits,
) -> Result<bool, AffinePreparePointError>
where
    O: AffinePreparePointOrdering + ?Sized,
{
    for &position in ordering.prepare_point_constant_positions() {
        stats.constant_sector_checks = bounded_add(
            "constant-row sector checks",
            stats.constant_sector_checks,
            1,
            limits.max_constant_sector_checks,
        )?;
        if !ordering.prepare_point_constant_row_stays_in_sector(position, offset[position])? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn stable_insertion_sort(
    values: &mut [AffineStartIntegralComplexityKey],
    stats: &mut AffinePreparePointStats,
    limits: AffinePreparePointLimits,
) -> Result<(), AffinePreparePointError> {
    for insertion in 1..values.len() {
        let mut position = insertion;
        while position != 0 {
            stats.order_comparisons = bounded_add(
                "prepare-point order comparisons",
                stats.order_comparisons,
                1,
                limits.max_order_comparisons,
            )?;
            // Debit the full possible GMP comparison footprint before `Ord`
            // can inspect either arbitrary-precision payload.  Charging both
            // complete retained payloads is deliberately conservative: it is
            // independent of where lexicographic comparison happens to stop,
            // and therefore replay-stable.
            let comparison_integer_bit_work = checked_add(
                "prepare-point order-comparison integer bit work",
                values[position - 1].retained_integer_bits(),
                values[position].retained_integer_bits(),
            )?;
            stats.order_comparison_integer_bit_work = bounded_add(
                "prepare-point order-comparison integer bit work",
                stats.order_comparison_integer_bit_work,
                comparison_integer_bit_work,
                limits.max_order_comparison_integer_bit_work,
            )?;
            if values[position - 1].cmp(&values[position]).is_le() {
                break;
            }
            values.swap(position - 1, position);
            position -= 1;
        }
    }
    Ok(())
}

fn key_component_count(arity: usize) -> Result<usize, AffinePreparePointError> {
    checked_add(
        "prepare-point order-key components",
        checked_mul("prepare-point order-key components", arity, 3)?,
        5,
    )
}

fn try_reserve_exact<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), AffinePreparePointError> {
    let requested = checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| AffinePreparePointError::AllocationFailure {
            resource,
            requested,
        })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffinePreparePointError> {
    left.checked_add(right)
        .ok_or(AffinePreparePointError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffinePreparePointError> {
    left.checked_mul(right)
        .ok_or(AffinePreparePointError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, AffinePreparePointError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), AffinePreparePointError> {
    if requested > limit {
        Err(AffinePreparePointError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
