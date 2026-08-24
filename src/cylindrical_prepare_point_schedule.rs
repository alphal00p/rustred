//! Replayable cumulative scheduling of integer-cylinder prepare-point shells.
//!
//! This certificate binds one
//! [`crate::CylindricalParametricEliminationOrdering`] and compiles every exact
//! shell at depths `0..=through_depth`.  All work and retained-payload limits
//! (except the depth ceiling itself) are cumulative across those layers.  A
//! layer receives only the exact unspent allowance, before that layer can
//! enumerate, construct ordering keys, sort, or retain points.
//!
//! Scope is intentionally narrow.  This represents one canonical integer
//! cylinder (`PartialIndexAssignment`), not LiteRed's union around a group of
//! fully numeric starts and not a dependent affine/rational symbolic start.
//! It schedules points only: it generates no identities, performs no
//! elimination, proves no residual case closed, and infers no master.

use std::fmt;
use std::sync::Arc;

use crate::{
    CylindricalOrderingError, CylindricalParametricEliminationOrdering,
    CylindricalPreparePointError, CylindricalPreparePointLayer, CylindricalPreparePointLimits,
    CylindricalPreparePointStats,
};

/// Stable replay schema for one cumulative single-cylinder schedule.
pub const CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA: &str =
    "rustred-cylindrical-prepare-point-schedule-v1";

/// Aggregate bounds across all retained layers at depths
/// `0..=through_depth`.
///
/// `max_depth` is a ceiling, while every other field is a cumulative budget,
/// not a fresh allowance for each shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CylindricalPreparePointScheduleLimits {
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

impl Default for CylindricalPreparePointScheduleLimits {
    fn default() -> Self {
        CylindricalPreparePointLimits::default().into()
    }
}

impl From<CylindricalPreparePointLimits> for CylindricalPreparePointScheduleLimits {
    fn from(value: CylindricalPreparePointLimits) -> Self {
        Self {
            max_depth: value.max_depth,
            max_enumeration_steps: value.max_enumeration_steps,
            max_enumerated_offsets: value.max_enumerated_offsets,
            max_enumerated_components: value.max_enumerated_components,
            max_fixed_sector_checks: value.max_fixed_sector_checks,
            max_retained_points: value.max_retained_points,
            max_retained_components: value.max_retained_components,
            max_order_key_components: value.max_order_key_components,
            max_order_comparisons: value.max_order_comparisons,
        }
    }
}

/// Exact cumulative census across every scheduled shell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CylindricalPreparePointScheduleStats {
    layer_count: usize,
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

impl CylindricalPreparePointScheduleStats {
    pub const fn layer_count(self) -> usize {
        self.layer_count
    }
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

    fn checked_with_layer(
        self,
        depth: usize,
        layer: CylindricalPreparePointStats,
        limits: CylindricalPreparePointScheduleLimits,
    ) -> Result<Self, CylindricalPreparePointScheduleError> {
        Ok(Self {
            layer_count: checked_add("scheduled prepare-point layers", self.layer_count, 1)?,
            enumeration_steps: cumulative_add(
                depth,
                "prepare-point enumeration steps",
                self.enumeration_steps,
                layer.enumeration_steps(),
                limits.max_enumeration_steps,
            )?,
            enumerated_offsets: cumulative_add(
                depth,
                "enumerated prepare-point offsets",
                self.enumerated_offsets,
                layer.enumerated_offsets(),
                limits.max_enumerated_offsets,
            )?,
            enumerated_components: cumulative_add(
                depth,
                "enumerated prepare-point components",
                self.enumerated_components,
                layer.enumerated_components(),
                limits.max_enumerated_components,
            )?,
            fixed_sector_checks: cumulative_add(
                depth,
                "fixed-coordinate sector checks",
                self.fixed_sector_checks,
                layer.fixed_sector_checks(),
                limits.max_fixed_sector_checks,
            )?,
            rejected_fixed_sector_offsets: checked_add(
                "rejected fixed-sector prepare-point offsets",
                self.rejected_fixed_sector_offsets,
                layer.rejected_fixed_sector_offsets(),
            )?,
            retained_points: cumulative_add(
                depth,
                "retained prepare points",
                self.retained_points,
                layer.retained_points(),
                limits.max_retained_points,
            )?,
            retained_components: cumulative_add(
                depth,
                "retained prepare-point components",
                self.retained_components,
                layer.retained_components(),
                limits.max_retained_components,
            )?,
            order_key_components: cumulative_add(
                depth,
                "prepare-point order-key components",
                self.order_key_components,
                layer.order_key_components(),
                limits.max_order_key_components,
            )?,
            order_comparisons: cumulative_add(
                depth,
                "prepare-point order comparisons",
                self.order_comparisons,
                layer.order_comparisons(),
                limits.max_order_comparisons,
            )?,
        })
    }
}

/// Complete replayable shell schedule for one integer cylinder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CylindricalPreparePointScheduleCertificate {
    schema: &'static str,
    // This is the one owned ordering payload for the whole schedule.  Every
    // child layer keeps an `Arc` clone of this exact allocation, so cumulative
    // depth does not multiply retained assignment/manifest storage.
    ordering: Arc<CylindricalParametricEliminationOrdering>,
    through_depth: usize,
    layers: Box<[CylindricalPreparePointLayer]>,
    limits: CylindricalPreparePointScheduleLimits,
    stats: CylindricalPreparePointScheduleStats,
}

impl CylindricalPreparePointScheduleCertificate {
    pub fn compile(
        ordering: CylindricalParametricEliminationOrdering,
        through_depth: usize,
        limits: CylindricalPreparePointScheduleLimits,
    ) -> Result<Self, CylindricalPreparePointScheduleError> {
        ordering.replay()?;
        let result = compile_unreplayed(Arc::new(ordering), through_depth, limits)?;
        result.replay_with_replayed_ordering()?;
        Ok(result)
    }

    /// Construct a schedule from the exact ordering allocation already
    /// replayed by a parent certificate.
    ///
    /// Child layers are deliberately constructed through their unreplayed
    /// crate-private path.  The parent schedule's replay reconstructs the
    /// complete payload once, without recursively compiling and replaying each
    /// layer again.
    pub(crate) fn compile_with_replayed_shared_ordering(
        ordering: Arc<CylindricalParametricEliminationOrdering>,
        through_depth: usize,
        limits: CylindricalPreparePointScheduleLimits,
    ) -> Result<Self, CylindricalPreparePointScheduleError> {
        compile_unreplayed(ordering, through_depth, limits)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// The one formal integer-cylinder ordering shared by all layers.
    pub fn ordering(&self) -> &CylindricalParametricEliminationOrdering {
        self.ordering.as_ref()
    }

    pub const fn through_depth(&self) -> usize {
        self.through_depth
    }

    /// Layers in exact increasing depth order, including depth zero.
    pub fn layers(&self) -> &[CylindricalPreparePointLayer] {
        &self.layers
    }

    pub const fn limits(&self) -> CylindricalPreparePointScheduleLimits {
        self.limits
    }

    pub const fn stats(&self) -> CylindricalPreparePointScheduleStats {
        self.stats
    }

    /// Recompile every layer with the exact remaining allowance and compare
    /// all typed payloads and cumulative statistics.
    pub fn replay(&self) -> Result<(), CylindricalPreparePointScheduleError> {
        if self.schema != CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA {
            return Err(CylindricalPreparePointScheduleError::SchemaMismatch);
        }
        self.ordering.replay()?;
        self.replay_with_replayed_ordering()
    }

    fn replay_with_replayed_ordering(&self) -> Result<(), CylindricalPreparePointScheduleError> {
        let replayed = compile_unreplayed(self.ordering.clone(), self.through_depth, self.limits)?;
        if replayed == *self {
            Ok(())
        } else {
            Err(CylindricalPreparePointScheduleError::ReplayMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CylindricalPreparePointScheduleError {
    DepthTooLarge {
        requested: usize,
        limit: usize,
    },
    /// A layer's next operation would exceed the remaining cumulative budget.
    CumulativeResourceLimit {
        depth: usize,
        resource: &'static str,
        consumed_before_layer: usize,
        requested_in_layer: usize,
        cumulative_requested: usize,
        cumulative_limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    LayerFailure {
        depth: usize,
        source: CylindricalPreparePointError,
    },
    Ordering(CylindricalOrderingError),
    SchemaMismatch,
    ReplayMismatch,
}

impl fmt::Display for CylindricalPreparePointScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthTooLarge { requested, limit } => write!(
                formatter,
                "cylindrical prepare-point schedule depth {requested} exceeds cumulative ceiling {limit}"
            ),
            Self::CumulativeResourceLimit {
                depth,
                resource,
                consumed_before_layer,
                requested_in_layer,
                cumulative_requested,
                cumulative_limit,
            } => write!(
                formatter,
                "cylindrical {resource} at depth {depth} needs {requested_in_layer} after {consumed_before_layer} prior uses (cumulative {cumulative_requested}), exceeding cumulative limit {cumulative_limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "cylindrical schedule {resource} count overflowed usize"
                )
            }
            Self::LayerFailure { depth, source } => {
                write!(
                    formatter,
                    "cylindrical prepare-point layer {depth} failed: {source}"
                )
            }
            Self::Ordering(error) => error.fmt(formatter),
            Self::SchemaMismatch => {
                formatter.write_str("cylindrical prepare-point schedule schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("cylindrical prepare-point schedule does not replay")
            }
        }
    }
}

impl std::error::Error for CylindricalPreparePointScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LayerFailure { source, .. } => Some(source),
            Self::Ordering(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CylindricalOrderingError> for CylindricalPreparePointScheduleError {
    fn from(value: CylindricalOrderingError) -> Self {
        Self::Ordering(value)
    }
}

fn compile_unreplayed(
    ordering: Arc<CylindricalParametricEliminationOrdering>,
    through_depth: usize,
    limits: CylindricalPreparePointScheduleLimits,
) -> Result<CylindricalPreparePointScheduleCertificate, CylindricalPreparePointScheduleError> {
    if through_depth > limits.max_depth {
        return Err(CylindricalPreparePointScheduleError::DepthTooLarge {
            requested: through_depth,
            limit: limits.max_depth,
        });
    }
    let layer_count = through_depth.checked_add(1).ok_or(
        CylindricalPreparePointScheduleError::ResourceCountOverflow {
            resource: "scheduled prepare-point layers",
        },
    )?;

    // `max_depth` is the explicit aggregate retained-layer bound.  Reserve
    // only after both that ceiling and `through_depth + 1` have been checked.
    let mut layers = Vec::with_capacity(layer_count);
    let mut stats = CylindricalPreparePointScheduleStats::default();
    for depth in 0..layer_count {
        // Compute every unspent allowance before cloning the ordering or
        // allowing the layer compiler to allocate its traversal/point state.
        let remaining = remaining_layer_limits(limits, stats)?;
        let layer = CylindricalPreparePointLayer::compile_with_replayed_shared_ordering(
            ordering.clone(),
            depth,
            remaining,
        )
        .map_err(|source| map_layer_error(depth, stats, limits, source))?;
        if layer.depth() != depth
            || layer.ordering() != ordering.as_ref()
            || layer.limits() != remaining
        {
            return Err(CylindricalPreparePointScheduleError::ReplayMismatch);
        }
        let next_stats = stats.checked_with_layer(depth, layer.stats(), limits)?;
        layers.push(layer);
        stats = next_stats;
    }
    if stats.layer_count != layer_count {
        return Err(CylindricalPreparePointScheduleError::ReplayMismatch);
    }

    Ok(CylindricalPreparePointScheduleCertificate {
        schema: CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA,
        ordering,
        through_depth,
        layers: layers.into_boxed_slice(),
        limits,
        stats,
    })
}

fn remaining_layer_limits(
    limits: CylindricalPreparePointScheduleLimits,
    stats: CylindricalPreparePointScheduleStats,
) -> Result<CylindricalPreparePointLimits, CylindricalPreparePointScheduleError> {
    Ok(CylindricalPreparePointLimits {
        max_depth: limits.max_depth,
        max_enumeration_steps: remaining(
            "prepare-point enumeration steps",
            limits.max_enumeration_steps,
            stats.enumeration_steps,
        )?,
        max_enumerated_offsets: remaining(
            "enumerated prepare-point offsets",
            limits.max_enumerated_offsets,
            stats.enumerated_offsets,
        )?,
        max_enumerated_components: remaining(
            "enumerated prepare-point components",
            limits.max_enumerated_components,
            stats.enumerated_components,
        )?,
        max_fixed_sector_checks: remaining(
            "fixed-coordinate sector checks",
            limits.max_fixed_sector_checks,
            stats.fixed_sector_checks,
        )?,
        max_retained_points: remaining(
            "retained prepare points",
            limits.max_retained_points,
            stats.retained_points,
        )?,
        max_retained_components: remaining(
            "retained prepare-point components",
            limits.max_retained_components,
            stats.retained_components,
        )?,
        max_order_key_components: remaining(
            "prepare-point order-key components",
            limits.max_order_key_components,
            stats.order_key_components,
        )?,
        max_order_comparisons: remaining(
            "prepare-point order comparisons",
            limits.max_order_comparisons,
            stats.order_comparisons,
        )?,
    })
}

fn map_layer_error(
    depth: usize,
    stats: CylindricalPreparePointScheduleStats,
    limits: CylindricalPreparePointScheduleLimits,
    source: CylindricalPreparePointError,
) -> CylindricalPreparePointScheduleError {
    match source {
        CylindricalPreparePointError::ResourceLimit {
            resource,
            requested,
            limit: layer_limit,
        } => {
            if let Some((consumed, limit)) = resource_account(resource, stats, limits) {
                match consumed.checked_add(requested) {
                    Some(cumulative_requested) => {
                        CylindricalPreparePointScheduleError::CumulativeResourceLimit {
                            depth,
                            resource,
                            consumed_before_layer: consumed,
                            requested_in_layer: requested,
                            cumulative_requested,
                            cumulative_limit: limit,
                        }
                    }
                    None => {
                        CylindricalPreparePointScheduleError::ResourceCountOverflow { resource }
                    }
                }
            } else {
                CylindricalPreparePointScheduleError::LayerFailure {
                    depth,
                    source: CylindricalPreparePointError::ResourceLimit {
                        resource,
                        requested,
                        limit: layer_limit,
                    },
                }
            }
        }
        source => CylindricalPreparePointScheduleError::LayerFailure { depth, source },
    }
}

fn resource_account(
    resource: &'static str,
    stats: CylindricalPreparePointScheduleStats,
    limits: CylindricalPreparePointScheduleLimits,
) -> Option<(usize, usize)> {
    match resource {
        "prepare-point enumeration steps" => {
            Some((stats.enumeration_steps, limits.max_enumeration_steps))
        }
        "enumerated prepare-point offsets" => {
            Some((stats.enumerated_offsets, limits.max_enumerated_offsets))
        }
        "enumerated prepare-point components" => Some((
            stats.enumerated_components,
            limits.max_enumerated_components,
        )),
        "fixed-coordinate sector checks" => {
            Some((stats.fixed_sector_checks, limits.max_fixed_sector_checks))
        }
        "retained prepare points" => Some((stats.retained_points, limits.max_retained_points)),
        "retained prepare-point components" => {
            Some((stats.retained_components, limits.max_retained_components))
        }
        "prepare-point order-key components" => {
            Some((stats.order_key_components, limits.max_order_key_components))
        }
        "prepare-point order comparisons" => {
            Some((stats.order_comparisons, limits.max_order_comparisons))
        }
        _ => None,
    }
}

fn remaining(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, CylindricalPreparePointScheduleError> {
    limit
        .checked_sub(consumed)
        .ok_or(CylindricalPreparePointScheduleError::ResourceCountOverflow { resource })
}

fn cumulative_add(
    depth: usize,
    resource: &'static str,
    consumed: usize,
    requested_in_layer: usize,
    limit: usize,
) -> Result<usize, CylindricalPreparePointScheduleError> {
    let requested = checked_add(resource, consumed, requested_in_layer)?;
    if requested > limit {
        Err(
            CylindricalPreparePointScheduleError::CumulativeResourceLimit {
                depth,
                resource,
                consumed_before_layer: consumed,
                requested_in_layer,
                cumulative_requested: requested,
                cumulative_limit: limit,
            },
        )
    } else {
        Ok(requested)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CylindricalPreparePointScheduleError> {
    left.checked_add(right)
        .ok_or(CylindricalPreparePointScheduleError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use crate::{
        CylindricalOrderingLimits, IntegralOrderingPolicy, PartialIndexAssignment, SectorMask,
    };

    use super::*;

    fn ordering(bits: &str) -> CylindricalParametricEliminationOrdering {
        let sector = SectorMask::try_from_bit_string(bits).unwrap();
        CylindricalParametricEliminationOrdering::try_new(
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            sector.clone(),
            PartialIndexAssignment::try_new([], sector.arity(), 0).unwrap(),
            CylindricalOrderingLimits::default(),
        )
        .unwrap()
    }

    #[test]
    fn replay_rejects_private_schema_stats_and_layer_order_tamper() {
        let original = CylindricalPreparePointScheduleCertificate::compile(
            ordering("11"),
            2,
            CylindricalPreparePointScheduleLimits::default(),
        )
        .unwrap();
        original.replay().unwrap();

        let mut tampered = original.clone();
        tampered.schema = "rustred-cylindrical-prepare-point-schedule-v999";
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointScheduleError::SchemaMismatch)
        );

        let mut tampered = original.clone();
        tampered.stats.retained_points += 1;
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointScheduleError::ReplayMismatch)
        );

        let mut tampered = original.clone();
        tampered.layers.swap(0, 1);
        assert_eq!(
            tampered.replay(),
            Err(CylindricalPreparePointScheduleError::ReplayMismatch)
        );
    }

    #[test]
    fn every_layer_and_schedule_clone_share_one_ordering_allocation() {
        let original = CylindricalPreparePointScheduleCertificate::compile(
            ordering("11"),
            3,
            CylindricalPreparePointScheduleLimits::default(),
        )
        .unwrap();
        assert!(
            original
                .layers
                .iter()
                .all(|layer| Arc::ptr_eq(&original.ordering, layer.ordering_arc()))
        );

        let cloned = original.clone();
        assert!(Arc::ptr_eq(&original.ordering, &cloned.ordering));
        assert!(
            cloned
                .layers
                .iter()
                .all(|layer| Arc::ptr_eq(&original.ordering, layer.ordering_arc()))
        );
        cloned.replay().unwrap();
    }

    #[test]
    fn maximum_depth_layer_count_overflow_fails_before_iteration() {
        let limits = CylindricalPreparePointScheduleLimits {
            max_depth: usize::MAX,
            ..CylindricalPreparePointScheduleLimits::default()
        };
        assert_eq!(
            CylindricalPreparePointScheduleCertificate::compile(ordering("1"), usize::MAX, limits,),
            Err(
                CylindricalPreparePointScheduleError::ResourceCountOverflow {
                    resource: "scheduled prepare-point layers",
                }
            )
        );
    }
}
