//! Replayable cumulative schedule of integer-affine prepare-point shells.
//!
//! Layers at depths `0..=through_depth` share one immutable affine ordering.
//! Every work counter is cumulative: the next layer receives only the exact
//! unspent allowance before it can enumerate, sort, or retain any point.
//! Boolean-branch guards stay uncomposed on the shared ordering; schedule
//! membership does not certify that translated indices remain in the branch.

use std::fmt;
use std::sync::Arc;

use crate::affine_prepare_points::{
    AffinePreparePointLayerCore, AffinePreparePointLayerKeyCore, AffinePreparePointOrdering,
    compile_affine_prepare_point_layer_core, compile_affine_prepare_point_layer_key_core,
};
use crate::{
    AffineParametricOrderingError, AffinePreparePointError, AffinePreparePointLayer,
    AffinePreparePointLimits, AffinePreparePointStats, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, ParametricCoefficientContext,
};

pub const AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA: &str =
    "rustred-affine-prepare-point-schedule-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AffinePreparePointScheduleLimits {
    pub max_depth: usize,
    pub max_enumeration_steps: usize,
    pub max_enumerated_offsets: usize,
    pub max_enumerated_components: usize,
    pub max_constant_sector_checks: usize,
    pub max_retained_points: usize,
    pub max_retained_components: usize,
    pub max_order_key_components: usize,
    pub max_order_key_integer_bits: usize,
    pub max_order_comparisons: usize,
    pub max_order_comparison_integer_bit_work: usize,
}

impl Default for AffinePreparePointScheduleLimits {
    fn default() -> Self {
        AffinePreparePointLimits::default().into()
    }
}

impl From<AffinePreparePointLimits> for AffinePreparePointScheduleLimits {
    fn from(value: AffinePreparePointLimits) -> Self {
        Self {
            max_depth: value.max_depth,
            max_enumeration_steps: value.max_enumeration_steps,
            max_enumerated_offsets: value.max_enumerated_offsets,
            max_enumerated_components: value.max_enumerated_components,
            max_constant_sector_checks: value.max_constant_sector_checks,
            max_retained_points: value.max_retained_points,
            max_retained_components: value.max_retained_components,
            max_order_key_components: value.max_order_key_components,
            max_order_key_integer_bits: value.max_order_key_integer_bits,
            max_order_comparisons: value.max_order_comparisons,
            max_order_comparison_integer_bit_work: value.max_order_comparison_integer_bit_work,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffinePreparePointScheduleStats {
    layer_count: usize,
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

impl AffinePreparePointScheduleStats {
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
    pub const fn order_comparison_integer_bit_work(self) -> usize {
        self.order_comparison_integer_bit_work
    }

    fn checked_with_layer(
        self,
        depth: usize,
        layer: AffinePreparePointStats,
        limits: AffinePreparePointScheduleLimits,
    ) -> Result<Self, AffinePreparePointScheduleError> {
        Ok(Self {
            layer_count: checked_add("scheduled affine prepare-point layers", self.layer_count, 1)?,
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
            constant_sector_checks: cumulative_add(
                depth,
                "constant-row sector checks",
                self.constant_sector_checks,
                layer.constant_sector_checks(),
                limits.max_constant_sector_checks,
            )?,
            rejected_constant_sector_offsets: checked_add(
                "rejected constant-sector prepare-point offsets",
                self.rejected_constant_sector_offsets,
                layer.rejected_constant_sector_offsets(),
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
            order_key_integer_bits: cumulative_add(
                depth,
                "prepare-point order-key integer bits",
                self.order_key_integer_bits,
                layer.order_key_integer_bits(),
                limits.max_order_key_integer_bits,
            )?,
            order_comparisons: cumulative_add(
                depth,
                "prepare-point order comparisons",
                self.order_comparisons,
                layer.order_comparisons(),
                limits.max_order_comparisons,
            )?,
            order_comparison_integer_bit_work: cumulative_add(
                depth,
                "prepare-point order-comparison integer bit work",
                self.order_comparison_integer_bit_work,
                layer.order_comparison_integer_bit_work(),
                limits.max_order_comparison_integer_bit_work,
            )?,
        })
    }
}

pub(crate) struct AffinePreparePointScheduleCore<L> {
    layers: Vec<L>,
    stats: AffinePreparePointScheduleStats,
}

impl<L> AffinePreparePointScheduleCore<L> {
    pub(crate) fn into_parts(self) -> (Vec<L>, AffinePreparePointScheduleStats) {
        (self.layers, self.stats)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffinePreparePointScheduleCertificate {
    schema: &'static str,
    ordering: Arc<AffineStartParametricEliminationOrdering>,
    through_depth: usize,
    layers: Arc<Vec<AffinePreparePointLayer>>,
    limits: AffinePreparePointScheduleLimits,
    stats: AffinePreparePointScheduleStats,
}

impl AffinePreparePointScheduleCertificate {
    pub fn compile(
        context: &ParametricCoefficientContext,
        ordering: AffineStartParametricEliminationOrdering,
        through_depth: usize,
        limits: AffinePreparePointScheduleLimits,
    ) -> Result<Self, AffinePreparePointScheduleError> {
        Self::compile_with_authority(
            AffineStartReplayAuthority::ContextOnly(context),
            ordering,
            through_depth,
            limits,
        )
    }

    pub fn compile_with_authority(
        authority: AffineStartReplayAuthority<'_>,
        ordering: AffineStartParametricEliminationOrdering,
        through_depth: usize,
        limits: AffinePreparePointScheduleLimits,
    ) -> Result<Self, AffinePreparePointScheduleError> {
        ordering.replay_with_authority(authority)?;
        let result = compile_unreplayed(Arc::new(ordering), through_depth, limits)?;
        result.replay_with_replayed_ordering()?;
        Ok(result)
    }

    pub(crate) fn compile_with_replayed_shared_ordering(
        ordering: Arc<AffineStartParametricEliminationOrdering>,
        through_depth: usize,
        limits: AffinePreparePointScheduleLimits,
    ) -> Result<Self, AffinePreparePointScheduleError> {
        compile_unreplayed(ordering, through_depth, limits)
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn ordering(&self) -> &AffineStartParametricEliminationOrdering {
        self.ordering.as_ref()
    }
    pub const fn through_depth(&self) -> usize {
        self.through_depth
    }
    pub fn layers(&self) -> &[AffinePreparePointLayer] {
        self.layers.as_slice()
    }
    pub const fn limits(&self) -> AffinePreparePointScheduleLimits {
        self.limits
    }
    pub const fn stats(&self) -> AffinePreparePointScheduleStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), AffinePreparePointScheduleError> {
        self.replay_with_authority(AffineStartReplayAuthority::ContextOnly(context))
    }

    pub fn replay_with_authority(
        &self,
        authority: AffineStartReplayAuthority<'_>,
    ) -> Result<(), AffinePreparePointScheduleError> {
        if self.schema != AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA {
            return Err(AffinePreparePointScheduleError::SchemaMismatch);
        }
        self.ordering.replay_with_authority(authority)?;
        self.replay_with_replayed_ordering()
    }

    fn replay_with_replayed_ordering(&self) -> Result<(), AffinePreparePointScheduleError> {
        let replayed = compile_unreplayed(self.ordering.clone(), self.through_depth, self.limits)?;
        if replayed == *self {
            Ok(())
        } else {
            Err(AffinePreparePointScheduleError::ReplayMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AffinePreparePointScheduleError {
    DepthTooLarge {
        requested: usize,
        limit: usize,
    },
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
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    LayerFailure {
        depth: usize,
        source: AffinePreparePointError,
    },
    Ordering(AffineParametricOrderingError),
    SchemaMismatch,
    ReplayMismatch,
}

impl fmt::Display for AffinePreparePointScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DepthTooLarge { requested, limit } => write!(
                formatter,
                "affine prepare-point schedule depth {requested} exceeds cumulative ceiling {limit}"
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
                "affine {resource} at depth {depth} needs {requested_in_layer} after {consumed_before_layer} prior uses (cumulative {cumulative_requested}), exceeding cumulative limit {cumulative_limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "affine prepare-point schedule {resource} count overflowed usize"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "affine prepare-point schedule {resource} could not reserve {requested} entries"
            ),
            Self::LayerFailure { depth, source } => write!(
                formatter,
                "affine prepare-point layer {depth} failed: {source}"
            ),
            Self::Ordering(error) => error.fmt(formatter),
            Self::SchemaMismatch => {
                formatter.write_str("affine prepare-point schedule schema mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("affine prepare-point schedule does not replay")
            }
        }
    }
}

impl std::error::Error for AffinePreparePointScheduleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LayerFailure { source, .. } => Some(source),
            Self::Ordering(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AffineParametricOrderingError> for AffinePreparePointScheduleError {
    fn from(value: AffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}

fn compile_unreplayed(
    ordering: Arc<AffineStartParametricEliminationOrdering>,
    through_depth: usize,
    limits: AffinePreparePointScheduleLimits,
) -> Result<AffinePreparePointScheduleCertificate, AffinePreparePointScheduleError> {
    let core = compile_affine_prepare_point_schedule_core(
        ordering.as_ref(),
        through_depth,
        limits,
        |layer| {
            let expected_depth = layer.depth();
            let expected_limits = layer.limits();
            let layer = AffinePreparePointLayer::from_core(Arc::clone(&ordering), layer);
            if layer.depth() != expected_depth
                || layer.ordering() != ordering.as_ref()
                || layer.limits() != expected_limits
            {
                return Err(AffinePreparePointScheduleError::ReplayMismatch);
            }
            Ok(layer)
        },
    )?;
    let (layers, stats) = core.into_parts();
    Ok(AffinePreparePointScheduleCertificate {
        schema: AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA,
        ordering,
        through_depth,
        // Fixed-size shared ownership preserves the allocation obtained
        // through `try_reserve_exact` and keeps certificate clones shallow.
        layers: Arc::new(layers),
        limits,
        stats,
    })
}

pub(crate) fn compile_affine_prepare_point_schedule_core<O, L, F>(
    ordering: &O,
    through_depth: usize,
    limits: AffinePreparePointScheduleLimits,
    mut wrap_layer: F,
) -> Result<AffinePreparePointScheduleCore<L>, AffinePreparePointScheduleError>
where
    O: AffinePreparePointOrdering + ?Sized,
    F: FnMut(AffinePreparePointLayerCore) -> Result<L, AffinePreparePointScheduleError>,
{
    compile_affine_prepare_point_schedule_core_with(
        ordering,
        through_depth,
        limits,
        compile_affine_prepare_point_layer_core,
        wrap_layer,
    )
}

pub(crate) fn compile_affine_prepare_point_schedule_key_core<O, L, F>(
    ordering: &O,
    through_depth: usize,
    limits: AffinePreparePointScheduleLimits,
    wrap_layer: F,
) -> Result<AffinePreparePointScheduleCore<L>, AffinePreparePointScheduleError>
where
    O: AffinePreparePointOrdering + ?Sized,
    F: FnMut(AffinePreparePointLayerKeyCore) -> Result<L, AffinePreparePointScheduleError>,
{
    compile_affine_prepare_point_schedule_core_with(
        ordering,
        through_depth,
        limits,
        compile_affine_prepare_point_layer_key_core,
        wrap_layer,
    )
}

fn compile_affine_prepare_point_schedule_core_with<O, P, L, C, F>(
    ordering: &O,
    through_depth: usize,
    limits: AffinePreparePointScheduleLimits,
    mut compile_layer: C,
    mut wrap_layer: F,
) -> Result<AffinePreparePointScheduleCore<L>, AffinePreparePointScheduleError>
where
    O: AffinePreparePointOrdering + ?Sized,
    C: FnMut(
        &O,
        usize,
        AffinePreparePointLimits,
    ) -> Result<AffinePreparePointLayerCore<P>, AffinePreparePointError>,
    F: FnMut(AffinePreparePointLayerCore<P>) -> Result<L, AffinePreparePointScheduleError>,
{
    if through_depth > limits.max_depth {
        return Err(AffinePreparePointScheduleError::DepthTooLarge {
            requested: through_depth,
            limit: limits.max_depth,
        });
    }
    let layer_count = checked_add("scheduled affine prepare-point layers", through_depth, 1)?;
    let mut layers = Vec::new();
    layers.try_reserve_exact(layer_count).map_err(|_| {
        AffinePreparePointScheduleError::AllocationFailure {
            resource: "scheduled affine prepare-point layers",
            requested: layer_count,
        }
    })?;
    let mut stats = AffinePreparePointScheduleStats::default();
    for depth in 0..layer_count {
        let remaining = remaining_layer_limits(limits, stats)?;
        let layer = compile_layer(ordering, depth, remaining)
            .map_err(|source| map_layer_error(depth, stats, limits, source))?;
        if layer.depth() != depth || layer.limits() != remaining {
            return Err(AffinePreparePointScheduleError::ReplayMismatch);
        }
        stats = stats.checked_with_layer(depth, layer.stats(), limits)?;
        layers.push(wrap_layer(layer)?);
    }
    if stats.layer_count != layer_count {
        return Err(AffinePreparePointScheduleError::ReplayMismatch);
    }
    Ok(AffinePreparePointScheduleCore { layers, stats })
}

fn remaining_layer_limits(
    limits: AffinePreparePointScheduleLimits,
    stats: AffinePreparePointScheduleStats,
) -> Result<AffinePreparePointLimits, AffinePreparePointScheduleError> {
    Ok(AffinePreparePointLimits {
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
        max_constant_sector_checks: remaining(
            "constant-row sector checks",
            limits.max_constant_sector_checks,
            stats.constant_sector_checks,
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
        max_order_key_integer_bits: remaining(
            "prepare-point order-key integer bits",
            limits.max_order_key_integer_bits,
            stats.order_key_integer_bits,
        )?,
        max_order_comparisons: remaining(
            "prepare-point order comparisons",
            limits.max_order_comparisons,
            stats.order_comparisons,
        )?,
        max_order_comparison_integer_bit_work: remaining(
            "prepare-point order-comparison integer bit work",
            limits.max_order_comparison_integer_bit_work,
            stats.order_comparison_integer_bit_work,
        )?,
    })
}

fn map_layer_error(
    depth: usize,
    prior: AffinePreparePointScheduleStats,
    limits: AffinePreparePointScheduleLimits,
    source: AffinePreparePointError,
) -> AffinePreparePointScheduleError {
    if let AffinePreparePointError::ResourceLimit {
        resource,
        requested,
        limit,
    } = &source
    {
        let consumed = consumed_for(resource, prior);
        let cumulative_limit = configured_limit(resource, limits);
        if let (Some(consumed_before_layer), Some(cumulative_limit)) = (consumed, cumulative_limit)
        {
            let Some(expected_layer_limit) = cumulative_limit.checked_sub(consumed_before_layer)
            else {
                return AffinePreparePointScheduleError::ResourceCountOverflow { resource };
            };
            if *limit == expected_layer_limit {
                let Some(cumulative_requested) = consumed_before_layer.checked_add(*requested)
                else {
                    return AffinePreparePointScheduleError::ResourceCountOverflow { resource };
                };
                return AffinePreparePointScheduleError::CumulativeResourceLimit {
                    depth,
                    resource,
                    consumed_before_layer,
                    requested_in_layer: *requested,
                    cumulative_requested,
                    cumulative_limit,
                };
            }
        }
    }
    AffinePreparePointScheduleError::LayerFailure { depth, source }
}

fn consumed_for(resource: &str, stats: AffinePreparePointScheduleStats) -> Option<usize> {
    match resource {
        "prepare-point enumeration steps" => Some(stats.enumeration_steps),
        "enumerated prepare-point offsets" => Some(stats.enumerated_offsets),
        "enumerated prepare-point components" => Some(stats.enumerated_components),
        "constant-row sector checks" => Some(stats.constant_sector_checks),
        "retained prepare points" => Some(stats.retained_points),
        "retained prepare-point components" => Some(stats.retained_components),
        "prepare-point order-key components" => Some(stats.order_key_components),
        "prepare-point order-key integer bits" => Some(stats.order_key_integer_bits),
        "prepare-point order comparisons" => Some(stats.order_comparisons),
        "prepare-point order-comparison integer bit work" => {
            Some(stats.order_comparison_integer_bit_work)
        }
        _ => None,
    }
}

fn configured_limit(resource: &str, limits: AffinePreparePointScheduleLimits) -> Option<usize> {
    match resource {
        "prepare-point enumeration steps" => Some(limits.max_enumeration_steps),
        "enumerated prepare-point offsets" => Some(limits.max_enumerated_offsets),
        "enumerated prepare-point components" => Some(limits.max_enumerated_components),
        "constant-row sector checks" => Some(limits.max_constant_sector_checks),
        "retained prepare points" => Some(limits.max_retained_points),
        "retained prepare-point components" => Some(limits.max_retained_components),
        "prepare-point order-key components" => Some(limits.max_order_key_components),
        "prepare-point order-key integer bits" => Some(limits.max_order_key_integer_bits),
        "prepare-point order comparisons" => Some(limits.max_order_comparisons),
        "prepare-point order-comparison integer bit work" => {
            Some(limits.max_order_comparison_integer_bit_work)
        }
        _ => None,
    }
}

fn remaining(
    resource: &'static str,
    limit: usize,
    used: usize,
) -> Result<usize, AffinePreparePointScheduleError> {
    limit
        .checked_sub(used)
        .ok_or(AffinePreparePointScheduleError::CumulativeResourceLimit {
            depth: 0,
            resource,
            consumed_before_layer: used,
            requested_in_layer: 0,
            cumulative_requested: used,
            cumulative_limit: limit,
        })
}

fn cumulative_add(
    depth: usize,
    resource: &'static str,
    consumed_before_layer: usize,
    requested_in_layer: usize,
    cumulative_limit: usize,
) -> Result<usize, AffinePreparePointScheduleError> {
    let cumulative_requested = consumed_before_layer
        .checked_add(requested_in_layer)
        .ok_or(AffinePreparePointScheduleError::ResourceCountOverflow { resource })?;
    if cumulative_requested > cumulative_limit {
        Err(AffinePreparePointScheduleError::CumulativeResourceLimit {
            depth,
            resource,
            consumed_before_layer,
            requested_in_layer,
            cumulative_requested,
            cumulative_limit,
        })
    } else {
        Ok(cumulative_requested)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, AffinePreparePointScheduleError> {
    left.checked_add(right)
        .ok_or(AffinePreparePointScheduleError::ResourceCountOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_error_mapping_reports_comparison_work_overflow_without_saturation() {
        let prior = AffinePreparePointScheduleStats {
            order_comparison_integer_bit_work: usize::MAX,
            ..AffinePreparePointScheduleStats::default()
        };
        let limits = AffinePreparePointScheduleLimits {
            max_order_comparison_integer_bit_work: usize::MAX,
            ..AffinePreparePointScheduleLimits::default()
        };
        let source = AffinePreparePointError::ResourceLimit {
            resource: "prepare-point order-comparison integer bit work",
            requested: 1,
            limit: 0,
        };

        assert_eq!(
            map_layer_error(3, prior, limits, source),
            AffinePreparePointScheduleError::ResourceCountOverflow {
                resource: "prepare-point order-comparison integer bit work",
            }
        );
    }
}
