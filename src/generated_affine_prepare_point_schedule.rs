//! Exact affine prepare-point shells for generated residual cases.
//!
//! The shell and cumulative schedule algorithms are shared verbatim with the
//! public V1 prepare-point implementation.  This module only supplies the
//! generated-case replay/ownership boundary: every retained layer owns the
//! exact [`GeneratedAffineParametricOrderingCertificate`] allocation and a
//! schedule authenticates that ordering once before compiling all depths.

use std::fmt;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use crate::affine_prepare_point_schedule::{
    AffinePreparePointScheduleCore, compile_affine_prepare_point_schedule_key_core,
};
use crate::affine_prepare_points::{
    AffinePreparePointLayerKeyCore, compile_affine_prepare_point_layer_key_core,
};
use crate::generated_affine_parametric_ordering::{
    GeneratedAffineParametricOrderingCertificate, GeneratedAffineParametricOrderingError,
};
use crate::solver::closure::case_inventory::GeneratedAffineResidualCaseAuthority;
use crate::{
    AffinePreparePointError, AffinePreparePointLimits, AffinePreparePointScheduleError,
    AffinePreparePointScheduleLimits, AffinePreparePointScheduleStats, AffinePreparePointStats,
    AffineStartIntegralComplexityKey, IndexShift, IntegralFamily, ParametricCoefficientContext,
};

pub(crate) const GENERATED_AFFINE_PREPARE_POINT_LAYER_V2_SCHEMA: &str =
    "rustred-generated-affine-prepare-point-layer-v2";
pub(crate) const GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_LAYER_V2_SCHEMA: &str =
    "rustred-generated-affine-prepare-point-schedule-layer-v2";
pub(crate) const GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_V2_SCHEMA: &str =
    "rustred-generated-affine-prepare-point-schedule-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointLayerLimits {
    pub(crate) prepare: AffinePreparePointLimits,
    pub(crate) max_ordering_replays: usize,
    pub(crate) max_authenticated_ordering_sessions: usize,
    pub(crate) max_retained_ordering_references: usize,
}

impl Default for GeneratedAffinePreparePointLayerLimits {
    fn default() -> Self {
        Self {
            prepare: AffinePreparePointLimits::default(),
            max_ordering_replays: 1,
            max_authenticated_ordering_sessions: 1,
            max_retained_ordering_references: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointLayerStats {
    ordering_replays: usize,
    authenticated_ordering_sessions: usize,
    retained_ordering_references: usize,
    prepare: AffinePreparePointStats,
}

impl GeneratedAffinePreparePointLayerStats {
    pub(crate) const fn ordering_replays(self) -> usize {
        self.ordering_replays
    }
    pub(crate) const fn authenticated_ordering_sessions(self) -> usize {
        self.authenticated_ordering_sessions
    }
    pub(crate) const fn retained_ordering_references(self) -> usize {
        self.retained_ordering_references
    }
    pub(crate) const fn prepare(self) -> AffinePreparePointStats {
        self.prepare
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedAffinePreparePointLayerCertificate {
    schema: &'static str,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    depth: usize,
    ordered_keys: Arc<Vec<AffineStartIntegralComplexityKey>>,
    limits: GeneratedAffinePreparePointLayerLimits,
    stats: GeneratedAffinePreparePointLayerStats,
}

impl fmt::Debug for GeneratedAffinePreparePointLayerCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffinePreparePointLayerCertificate")
            .field("schema", &self.schema)
            .field("depth", &self.depth)
            .field("point_count", &self.ordered_keys.len())
            .field("private_ordering", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffinePreparePointLayerCertificate {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self.schema == other.schema
                && Arc::ptr_eq(&self.ordering, &other.ordering)
                && self.depth == other.depth
                && self.ordered_keys == other.ordered_keys
                && self.limits == other.limits
                && self.stats == other.stats
    }
}

impl Eq for GeneratedAffinePreparePointLayerCertificate {}

impl GeneratedAffinePreparePointLayerCertificate {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        depth: usize,
        limits: GeneratedAffinePreparePointLayerLimits,
    ) -> Result<Self, GeneratedAffinePreparePointError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_unwind_boundary(family, context, ordering, authority, depth, limits)
        }))
        .map_err(|_| GeneratedAffinePreparePointError::SymbolicaPanic)?
    }

    fn compile_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        depth: usize,
        limits: GeneratedAffinePreparePointLayerLimits,
    ) -> Result<Self, GeneratedAffinePreparePointError> {
        const ORDERING_REPLAYS: usize = 1;
        const AUTHENTICATED_ORDERING_SESSIONS: usize = 1;
        const RETAINED_ORDERING_REFERENCES: usize = 1;
        check_limit(
            "ordering replays",
            ORDERING_REPLAYS,
            limits.max_ordering_replays,
        )?;
        check_limit(
            "authenticated ordering sessions",
            AUTHENTICATED_ORDERING_SESSIONS,
            limits.max_authenticated_ordering_sessions,
        )?;
        check_limit(
            "retained ordering references",
            RETAINED_ORDERING_REFERENCES,
            limits.max_retained_ordering_references,
        )?;
        if depth > limits.prepare.max_depth {
            return Err(GeneratedAffinePreparePointError::Layer(
                AffinePreparePointError::DepthTooLarge {
                    requested: depth,
                    limit: limits.prepare.max_depth,
                },
            ));
        }

        ordering.replay(family, context, authority)?;
        let core = ordering.with_authenticated_algebra(context, |algebra| {
            compile_affine_prepare_point_layer_key_core(algebra, depth, limits.prepare)
                .map_err(GeneratedAffinePreparePointError::Layer)
        })?;
        let (depth, ordered_keys, prepare_limits, prepare_stats) = core.into_parts();
        if prepare_limits != limits.prepare {
            return Err(GeneratedAffinePreparePointError::ReplayMismatch);
        }
        Ok(Self {
            schema: GENERATED_AFFINE_PREPARE_POINT_LAYER_V2_SCHEMA,
            ordering,
            depth,
            ordered_keys: Arc::new(ordered_keys),
            limits,
            stats: GeneratedAffinePreparePointLayerStats {
                ordering_replays: ORDERING_REPLAYS,
                authenticated_ordering_sessions: AUTHENTICATED_ORDERING_SESSIONS,
                retained_ordering_references: RETAINED_ORDERING_REFERENCES,
                prepare: prepare_stats,
            },
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn ordering(&self) -> &GeneratedAffineParametricOrderingCertificate {
        self.ordering.as_ref()
    }
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }
    pub(crate) fn point_count(&self) -> usize {
        self.ordered_keys.len()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffinePreparePointLayerLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffinePreparePointLayerStats {
        self.stats
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffinePreparePointError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_PREPARE_POINT_LAYER_V2_SCHEMA {
                return Err(GeneratedAffinePreparePointError::SchemaMismatch);
            }
            if !Arc::ptr_eq(&self.ordering, ordering) {
                return Err(GeneratedAffinePreparePointError::OrderingAllocationMismatch);
            }
            let rebuilt = Self::compile_unwind_boundary(
                family,
                context,
                Arc::clone(ordering),
                authority,
                self.depth,
                self.limits,
            )?;
            if rebuilt == *self {
                Ok(())
            } else {
                Err(GeneratedAffinePreparePointError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffinePreparePointError::SymbolicaPanic)?
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointScheduleLimits {
    pub(crate) prepare: AffinePreparePointScheduleLimits,
    pub(crate) max_ordering_replays: usize,
    pub(crate) max_authenticated_ordering_sessions: usize,
    pub(crate) max_layer_certificates: usize,
    pub(crate) max_retained_ordering_references: usize,
}

impl Default for GeneratedAffinePreparePointScheduleLimits {
    fn default() -> Self {
        Self {
            prepare: AffinePreparePointScheduleLimits::default(),
            max_ordering_replays: 1,
            max_authenticated_ordering_sessions: 1,
            max_layer_certificates: 1_000_000,
            max_retained_ordering_references: 1_000_001,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointScheduleStats {
    ordering_replays: usize,
    authenticated_ordering_sessions: usize,
    layer_certificates: usize,
    retained_ordering_references: usize,
    prepare: AffinePreparePointScheduleStats,
}

impl GeneratedAffinePreparePointScheduleStats {
    pub(crate) const fn ordering_replays(self) -> usize {
        self.ordering_replays
    }
    pub(crate) const fn authenticated_ordering_sessions(self) -> usize {
        self.authenticated_ordering_sessions
    }
    pub(crate) const fn layer_certificates(self) -> usize {
        self.layer_certificates
    }
    pub(crate) const fn retained_ordering_references(self) -> usize {
        self.retained_ordering_references
    }
    pub(crate) const fn prepare(self) -> AffinePreparePointScheduleStats {
        self.prepare
    }
}

#[derive(Clone)]
pub(crate) struct GeneratedAffinePreparePointScheduleLayer {
    schema: &'static str,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    depth: usize,
    ordered_keys: Arc<Vec<AffineStartIntegralComplexityKey>>,
    limits: AffinePreparePointLimits,
    stats: AffinePreparePointStats,
}

impl GeneratedAffinePreparePointScheduleLayer {
    fn from_core(
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        core: AffinePreparePointLayerKeyCore,
    ) -> Self {
        let (depth, ordered_keys, limits, stats) = core.into_parts();
        Self {
            schema: GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_LAYER_V2_SCHEMA,
            ordering,
            depth,
            ordered_keys: Arc::new(ordered_keys),
            limits,
            stats,
        }
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn ordering(&self) -> &GeneratedAffineParametricOrderingCertificate {
        self.ordering.as_ref()
    }
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }
    pub(crate) fn point_count(&self) -> usize {
        self.ordered_keys.len()
    }
    pub(crate) const fn limits(&self) -> AffinePreparePointLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> AffinePreparePointStats {
        self.stats
    }
}

impl fmt::Debug for GeneratedAffinePreparePointScheduleLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffinePreparePointScheduleLayer")
            .field("schema", &self.schema)
            .field("depth", &self.depth)
            .field("point_count", &self.ordered_keys.len())
            .field("private_ordering", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffinePreparePointScheduleLayer {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self.schema == other.schema
                && Arc::ptr_eq(&self.ordering, &other.ordering)
                && self.depth == other.depth
                && self.ordered_keys == other.ordered_keys
                && self.limits == other.limits
                && self.stats == other.stats
    }
}

impl Eq for GeneratedAffinePreparePointScheduleLayer {}

/// Opaque point provenance minted only from one exact schedule allocation.
/// The key and its translation are borrowed, never copied or silently
/// reconstructed.
#[derive(Clone, Copy)]
pub(crate) struct GeneratedAffinePreparePointSchedulePointHandle<'schedule> {
    schedule: &'schedule Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ordering: &'schedule Arc<GeneratedAffineParametricOrderingCertificate>,
    depth: usize,
    point_ordinal: usize,
    key: &'schedule AffineStartIntegralComplexityKey,
    translation: &'schedule IndexShift,
}

impl fmt::Debug for GeneratedAffinePreparePointSchedulePointHandle<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffinePreparePointSchedulePointHandle")
            .field("depth", &self.depth)
            .field("point_ordinal", &self.point_ordinal)
            .field("private_schedule", &"<redacted>")
            .field("private_ordering", &"<redacted>")
            .field("private_point", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointAuthenticationLimits {
    pub(crate) max_schedule_replays: usize,
    pub(crate) max_pointer_checks: usize,
    pub(crate) max_index_checks: usize,
}

impl Default for GeneratedAffinePreparePointAuthenticationLimits {
    fn default() -> Self {
        Self {
            max_schedule_replays: 1,
            max_pointer_checks: POINT_AUTHENTICATION_POINTER_CHECKS,
            max_index_checks: POINT_AUTHENTICATION_INDEX_CHECKS,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedAffinePreparePointAuthenticationStats {
    schedule_replays: usize,
    pointer_checks: usize,
    index_checks: usize,
}

impl GeneratedAffinePreparePointAuthenticationStats {
    pub(crate) const fn schedule_replays(self) -> usize {
        self.schedule_replays
    }
    pub(crate) const fn pointer_checks(self) -> usize {
        self.pointer_checks
    }
    pub(crate) const fn index_checks(self) -> usize {
        self.index_checks
    }
}

pub(crate) struct GeneratedAffinePreparePointSchedulePoint<'schedule> {
    schedule: &'schedule Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ordering: &'schedule Arc<GeneratedAffineParametricOrderingCertificate>,
    depth: usize,
    point_ordinal: usize,
    key: &'schedule AffineStartIntegralComplexityKey,
    stats: GeneratedAffinePreparePointAuthenticationStats,
}

impl fmt::Debug for GeneratedAffinePreparePointSchedulePoint<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffinePreparePointSchedulePoint")
            .field("depth", &self.depth)
            .field("point_ordinal", &self.point_ordinal)
            .field("private_schedule", &"<redacted>")
            .field("private_ordering", &"<redacted>")
            .field("private_point", &"<redacted>")
            .finish()
    }
}

impl<'schedule> GeneratedAffinePreparePointSchedulePoint<'schedule> {
    pub(crate) const fn depth(&self) -> usize {
        self.depth
    }
    pub(crate) const fn point_ordinal(&self) -> usize {
        self.point_ordinal
    }
    pub(crate) fn translation(&self) -> &'schedule IndexShift {
        self.key.shift()
    }
    pub(crate) const fn key(&self) -> &'schedule AffineStartIntegralComplexityKey {
        self.key
    }
    pub(crate) const fn stats(&self) -> GeneratedAffinePreparePointAuthenticationStats {
        self.stats
    }
    pub(crate) fn same_schedule_allocation(
        &self,
        schedule: &Arc<GeneratedAffinePreparePointScheduleCertificate>,
    ) -> bool {
        Arc::ptr_eq(self.schedule, schedule)
    }
    pub(crate) fn same_ordering_allocation(
        &self,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    ) -> bool {
        Arc::ptr_eq(self.ordering, ordering)
    }
}

const POINT_AUTHENTICATION_SCHEDULE_REPLAYS: usize = 1;
const POINT_AUTHENTICATION_POINTER_CHECKS: usize = 5;
const POINT_AUTHENTICATION_INDEX_CHECKS: usize = 2;

#[derive(Clone)]
pub(crate) struct GeneratedAffinePreparePointScheduleCertificate {
    schema: &'static str,
    ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
    through_depth: usize,
    layers: Arc<Vec<GeneratedAffinePreparePointScheduleLayer>>,
    limits: GeneratedAffinePreparePointScheduleLimits,
    stats: GeneratedAffinePreparePointScheduleStats,
}

impl fmt::Debug for GeneratedAffinePreparePointScheduleCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAffinePreparePointScheduleCertificate")
            .field("schema", &self.schema)
            .field("through_depth", &self.through_depth)
            .field("layer_count", &self.layers.len())
            .field("private_ordering", &"<redacted>")
            .finish()
    }
}

impl PartialEq for GeneratedAffinePreparePointScheduleCertificate {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || self.schema == other.schema
                && Arc::ptr_eq(&self.ordering, &other.ordering)
                && self.through_depth == other.through_depth
                && self.layers == other.layers
                && self.limits == other.limits
                && self.stats == other.stats
    }
}

impl Eq for GeneratedAffinePreparePointScheduleCertificate {}

impl GeneratedAffinePreparePointScheduleCertificate {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        through_depth: usize,
        limits: GeneratedAffinePreparePointScheduleLimits,
    ) -> Result<Self, GeneratedAffinePreparePointError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::compile_unwind_boundary(
                family,
                context,
                ordering,
                authority,
                through_depth,
                limits,
            )
        }))
        .map_err(|_| GeneratedAffinePreparePointError::SymbolicaPanic)?
    }

    fn compile_unwind_boundary(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        through_depth: usize,
        limits: GeneratedAffinePreparePointScheduleLimits,
    ) -> Result<Self, GeneratedAffinePreparePointError> {
        const ORDERING_REPLAYS: usize = 1;
        const AUTHENTICATED_ORDERING_SESSIONS: usize = 1;
        check_limit(
            "ordering replays",
            ORDERING_REPLAYS,
            limits.max_ordering_replays,
        )?;
        check_limit(
            "authenticated ordering sessions",
            AUTHENTICATED_ORDERING_SESSIONS,
            limits.max_authenticated_ordering_sessions,
        )?;
        if through_depth > limits.prepare.max_depth {
            return Err(GeneratedAffinePreparePointError::Schedule(
                AffinePreparePointScheduleError::DepthTooLarge {
                    requested: through_depth,
                    limit: limits.prepare.max_depth,
                },
            ));
        }
        let layer_count = checked_add("layer certificates", through_depth, 1)?;
        check_limit(
            "layer certificates",
            layer_count,
            limits.max_layer_certificates,
        )?;
        let retained_ordering_references =
            checked_add("retained ordering references", layer_count, 1)?;
        check_limit(
            "retained ordering references",
            retained_ordering_references,
            limits.max_retained_ordering_references,
        )?;

        ordering.replay(family, context, authority)?;
        let core: AffinePreparePointScheduleCore<GeneratedAffinePreparePointScheduleLayer> =
            ordering.with_authenticated_algebra(context, |algebra| {
                compile_affine_prepare_point_schedule_key_core(
                    algebra,
                    through_depth,
                    limits.prepare,
                    |layer| {
                        Ok(GeneratedAffinePreparePointScheduleLayer::from_core(
                            Arc::clone(&ordering),
                            layer,
                        ))
                    },
                )
                .map_err(GeneratedAffinePreparePointError::Schedule)
            })?;
        let (layers, prepare_stats) = core.into_parts();
        if layers.len() != layer_count || prepare_stats.layer_count() != layer_count {
            return Err(GeneratedAffinePreparePointError::ReplayMismatch);
        }
        for (depth, layer) in layers.iter().enumerate() {
            if layer.schema != GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_LAYER_V2_SCHEMA
                || !Arc::ptr_eq(&layer.ordering, &ordering)
                || layer.depth != depth
            {
                return Err(GeneratedAffinePreparePointError::ReplayMismatch);
            }
        }
        Ok(Self {
            schema: GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_V2_SCHEMA,
            ordering,
            through_depth,
            layers: Arc::new(layers),
            limits,
            stats: GeneratedAffinePreparePointScheduleStats {
                ordering_replays: ORDERING_REPLAYS,
                authenticated_ordering_sessions: AUTHENTICATED_ORDERING_SESSIONS,
                layer_certificates: layer_count,
                retained_ordering_references,
                prepare: prepare_stats,
            },
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }
    pub(crate) fn ordering(&self) -> &GeneratedAffineParametricOrderingCertificate {
        self.ordering.as_ref()
    }
    pub(crate) const fn through_depth(&self) -> usize {
        self.through_depth
    }
    pub(crate) fn layers(&self) -> &[GeneratedAffinePreparePointScheduleLayer] {
        self.layers.as_slice()
    }
    pub(crate) const fn limits(&self) -> GeneratedAffinePreparePointScheduleLimits {
        self.limits
    }
    pub(crate) const fn stats(&self) -> GeneratedAffinePreparePointScheduleStats {
        self.stats
    }

    /// Complete schedule-local owner graph, excluding the generated ordering
    /// allocation. Layer `Arc` handles are inline in the layer vector, while
    /// each ordered-key allocation is charged here. A key's ordering manifest
    /// pointee is shared with the ordering and intentionally excluded by the
    /// key's own retained-byte API.
    pub(crate) fn owner_retained_bytes_excluding_ordering(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>().checked_add(arc_vec_owned_byte_bound(&self.layers)?)?;
        for layer in self.layers.iter() {
            bytes = bytes.checked_add(arc_vec_owned_byte_bound(&layer.ordered_keys)?)?;
            for key in layer.ordered_keys.iter() {
                let deep_key_bytes = key
                    .owned_retained_byte_bound()?
                    .checked_sub(size_of::<AffineStartIntegralComplexityKey>())?;
                bytes = bytes.checked_add(deep_key_bytes)?;
            }
        }
        Some(bytes)
    }

    pub(crate) fn same_ordering_allocation(
        &self,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.ordering, ordering)
    }

    pub(crate) fn point_handle<'schedule>(
        self: &'schedule Arc<Self>,
        depth: usize,
        point_ordinal: usize,
    ) -> Option<GeneratedAffinePreparePointSchedulePointHandle<'schedule>> {
        let layer = self.layers.get(depth)?;
        let key = layer.ordered_keys.get(point_ordinal)?;
        Some(GeneratedAffinePreparePointSchedulePointHandle {
            schedule: self,
            ordering: &self.ordering,
            depth,
            point_ordinal,
            key,
            translation: key.shift(),
        })
    }

    pub(crate) fn authenticate_point_handle<'schedule>(
        self: &'schedule Arc<Self>,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: &'schedule Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
        handle: GeneratedAffinePreparePointSchedulePointHandle<'schedule>,
        limits: GeneratedAffinePreparePointAuthenticationLimits,
    ) -> Result<GeneratedAffinePreparePointSchedulePoint<'schedule>, GeneratedAffinePreparePointError>
    {
        catch_unwind(AssertUnwindSafe(|| {
            check_limit(
                "point schedule replays",
                POINT_AUTHENTICATION_SCHEDULE_REPLAYS,
                limits.max_schedule_replays,
            )?;
            check_limit(
                "point pointer checks",
                POINT_AUTHENTICATION_POINTER_CHECKS,
                limits.max_pointer_checks,
            )?;
            check_limit(
                "point index checks",
                POINT_AUTHENTICATION_INDEX_CHECKS,
                limits.max_index_checks,
            )?;
            if !Arc::ptr_eq(self, handle.schedule)
                || !Arc::ptr_eq(&self.ordering, handle.ordering)
                || !Arc::ptr_eq(&self.ordering, ordering)
            {
                return Err(GeneratedAffinePreparePointError::PointBindingMismatch);
            }
            let retained_key = self
                .layers
                .get(handle.depth)
                .and_then(|layer| layer.ordered_keys.get(handle.point_ordinal))
                .ok_or(GeneratedAffinePreparePointError::PointBindingMismatch)?;
            if !std::ptr::eq(retained_key, handle.key)
                || !std::ptr::eq(retained_key.shift(), handle.translation)
            {
                return Err(GeneratedAffinePreparePointError::PointBindingMismatch);
            }
            self.replay(family, context, ordering, authority)?;
            Ok(GeneratedAffinePreparePointSchedulePoint {
                schedule: self,
                ordering: &self.ordering,
                depth: handle.depth,
                point_ordinal: handle.point_ordinal,
                key: retained_key,
                stats: GeneratedAffinePreparePointAuthenticationStats {
                    schedule_replays: POINT_AUTHENTICATION_SCHEDULE_REPLAYS,
                    pointer_checks: POINT_AUTHENTICATION_POINTER_CHECKS,
                    index_checks: POINT_AUTHENTICATION_INDEX_CHECKS,
                },
            })
        }))
        .map_err(|_| GeneratedAffinePreparePointError::SymbolicaPanic)?
    }

    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        ordering: &Arc<GeneratedAffineParametricOrderingCertificate>,
        authority: &Arc<GeneratedAffineResidualCaseAuthority>,
    ) -> Result<(), GeneratedAffinePreparePointError> {
        catch_unwind(AssertUnwindSafe(|| {
            if self.schema != GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_V2_SCHEMA {
                return Err(GeneratedAffinePreparePointError::SchemaMismatch);
            }
            if !Arc::ptr_eq(&self.ordering, ordering) {
                return Err(GeneratedAffinePreparePointError::OrderingAllocationMismatch);
            }
            check_limit("ordering replays", 1, self.limits.max_ordering_replays)?;
            check_limit(
                "authenticated ordering sessions",
                1,
                self.limits.max_authenticated_ordering_sessions,
            )?;
            if self.through_depth > self.limits.prepare.max_depth {
                return Err(GeneratedAffinePreparePointError::ReplayMismatch);
            }
            let expected_layer_count = checked_add("layer certificates", self.through_depth, 1)?;
            check_limit(
                "layer certificates",
                self.layers.len(),
                self.limits.max_layer_certificates,
            )?;
            let expected_retained_ordering_references =
                checked_add("retained ordering references", expected_layer_count, 1)?;
            check_limit(
                "retained ordering references",
                expected_retained_ordering_references,
                self.limits.max_retained_ordering_references,
            )?;
            if self.layers.len() != expected_layer_count
                || self.stats.ordering_replays != 1
                || self.stats.authenticated_ordering_sessions != 1
                || self.stats.layer_certificates != expected_layer_count
                || self.stats.retained_ordering_references != expected_retained_ordering_references
                || self.stats.prepare.layer_count() != expected_layer_count
            {
                return Err(GeneratedAffinePreparePointError::ReplayMismatch);
            }
            for (depth, layer) in self.layers.iter().enumerate() {
                if layer.schema != GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_LAYER_V2_SCHEMA
                    || !Arc::ptr_eq(&layer.ordering, ordering)
                    || layer.depth != depth
                {
                    return Err(GeneratedAffinePreparePointError::ReplayMismatch);
                }
            }
            let rebuilt = Self::compile_unwind_boundary(
                family,
                context,
                Arc::clone(ordering),
                authority,
                self.through_depth,
                self.limits,
            )?;
            if rebuilt == *self {
                Ok(())
            } else {
                Err(GeneratedAffinePreparePointError::ReplayMismatch)
            }
        }))
        .map_err(|_| GeneratedAffinePreparePointError::SymbolicaPanic)?
    }
}

fn arc_payload_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<AtomicUsize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))?
        .checked_add(size_of::<T>())
}

fn arc_vec_owned_byte_bound<T>(value: &Arc<Vec<T>>) -> Option<usize> {
    arc_payload_control_and_padding_byte_bound::<Vec<T>>()?
        .checked_add(value.capacity().checked_mul(size_of::<T>())?)
}

pub(crate) enum GeneratedAffinePreparePointError {
    SchemaMismatch,
    OrderingAllocationMismatch,
    PointBindingMismatch,
    ReplayMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Ordering(GeneratedAffineParametricOrderingError),
    Layer(AffinePreparePointError),
    Schedule(AffinePreparePointScheduleError),
    SymbolicaPanic,
}

impl fmt::Debug for GeneratedAffinePreparePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for GeneratedAffinePreparePointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated affine prepare-point schema mismatch")
            }
            Self::OrderingAllocationMismatch => formatter
                .write_str("generated affine prepare-point requires its exact ordering allocation"),
            Self::PointBindingMismatch => {
                formatter.write_str("generated affine prepare-point handle binding mismatch")
            }
            Self::ReplayMismatch => {
                formatter.write_str("generated affine prepare-point certificate does not replay")
            }
            Self::ResourceLimit { resource, .. } => write!(
                formatter,
                "generated affine prepare-point {resource} resource limit exceeded"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "generated affine prepare-point {resource} count overflowed usize"
            ),
            Self::Ordering(_) => {
                formatter.write_str("generated affine prepare-point ordering replay failed")
            }
            Self::Layer(_) => {
                formatter.write_str("generated affine prepare-point layer compilation failed")
            }
            Self::Schedule(_) => {
                formatter.write_str("generated affine prepare-point schedule compilation failed")
            }
            Self::SymbolicaPanic => {
                formatter.write_str("Symbolica panicked during generated prepare-point operation")
            }
        }
    }
}

impl std::error::Error for GeneratedAffinePreparePointError {}

impl From<GeneratedAffineParametricOrderingError> for GeneratedAffinePreparePointError {
    fn from(value: GeneratedAffineParametricOrderingError) -> Self {
        Self::Ordering(value)
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedAffinePreparePointError> {
    left.checked_add(right)
        .ok_or(GeneratedAffinePreparePointError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedAffinePreparePointError> {
    if requested > limit {
        Err(GeneratedAffinePreparePointError::ResourceLimit {
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
    use std::sync::{Arc, Weak};

    use super::*;
    use crate::affine_parametric_ordering::AffineConstantRowSectorTransition;
    use crate::generated_affine_parametric_ordering::GeneratedAffineParametricOrderingLimits;
    use crate::generated_affine_residual_boolean_cover::{
        GeneratedAffineResidualBooleanCoverCompiler, GeneratedAffineResidualBooleanCoverLimits,
    };
    use crate::generated_affine_residual_source_authority::GeneratedAffineResidualSourceAuthority;
    use crate::solver::closure::case_inventory::{
        GeneratedAffineResidualCaseAuthorityLimits,
        GeneratedAffineResidualCaseInventoryCertificate,
        GeneratedAffineResidualCaseInventoryCompiler, GeneratedAffineResidualCaseInventoryLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricIbpGenerator,
        SectorMask,
    };

    fn equal_mass_two_loop_family(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap()
    }

    fn fixture(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<GeneratedAffineResidualCaseInventoryCertificate>,
        Arc<GeneratedAffineResidualCaseAuthority>,
        Arc<GeneratedAffineParametricOrderingCertificate>,
    ) {
        let family = equal_mass_two_loop_family(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("011").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let source = GeneratedAffineResidualSourceAuthority::initial_global(queue);
        let boolean = Arc::new(
            GeneratedAffineResidualBooleanCoverCompiler::compile(
                &family,
                &context,
                source,
                GeneratedAffineResidualBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let inventory = Arc::new(
            GeneratedAffineResidualCaseInventoryCompiler::compile(
                &family,
                &context,
                boolean,
                GeneratedAffineResidualCaseInventoryLimits::default(),
            )
            .unwrap(),
        );
        let case_ordinal = (0..inventory.case_count())
            .max_by_key(|&ordinal| {
                inventory
                    .authenticated_case_view(&context, ordinal)
                    .unwrap()
                    .source()
                    .affine_map()
                    .free_positions()
                    .len()
            })
            .unwrap();
        let authority = Arc::new(
            GeneratedAffineResidualCaseAuthority::try_new(
                &family,
                &context,
                Arc::clone(&inventory),
                case_ordinal,
                GeneratedAffineResidualCaseAuthorityLimits::default(),
            )
            .unwrap(),
        );
        let ordering = Arc::new(
            GeneratedAffineParametricOrderingCertificate::try_new(
                &family,
                &context,
                Arc::clone(&authority),
                GeneratedAffineParametricOrderingLimits::default(),
            )
            .unwrap(),
        );
        (family, context, inventory, authority, ordering)
    }

    fn enumerate_l1(
        position: usize,
        remaining: i64,
        values: &mut [i64],
        output: &mut Vec<IndexShift>,
    ) {
        if position + 1 == values.len() {
            if remaining == 0 {
                values[position] = 0;
                output.push(IndexShift::try_new(values.iter().copied(), values.len()).unwrap());
            } else {
                values[position] = -remaining;
                output.push(IndexShift::try_new(values.iter().copied(), values.len()).unwrap());
                values[position] = remaining;
                output.push(IndexShift::try_new(values.iter().copied(), values.len()).unwrap());
            }
            return;
        }
        for value in -remaining..=remaining {
            values[position] = value;
            enumerate_l1(
                position + 1,
                remaining - value.unsigned_abs() as i64,
                values,
                output,
            );
        }
    }

    fn independent_expected_keys(
        context: &ParametricCoefficientContext,
        ordering: &GeneratedAffineParametricOrderingCertificate,
        depth: usize,
    ) -> (usize, Vec<AffineStartIntegralComplexityKey>) {
        let mut shifts = Vec::new();
        enumerate_l1(
            0,
            i64::try_from(depth).unwrap(),
            &mut vec![0; ordering.arity()],
            &mut shifts,
        );
        let enumerated = shifts.len();
        shifts.retain(|shift| {
            ordering.constant_positions().iter().all(|&position| {
                ordering
                    .classify_constant_row_shift(context, position, shift.values()[position])
                    .is_ok_and(|classification| {
                        classification.transition()
                            == AffineConstantRowSectorTransition::StaysInSourceSector
                    })
            })
        });
        let mut keys = shifts
            .iter()
            .map(|shift| ordering.key_for_shift(context, shift).unwrap())
            .collect::<Vec<_>>();
        keys.sort();
        (enumerated, keys)
    }

    #[test]
    fn generated_shells_and_retained_keys_match_independent_oracle() {
        let (family, context, _inventory, authority, ordering) =
            fixture("generated-affine-prepare-v2-oracle-private");
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                2,
                GeneratedAffinePreparePointScheduleLimits::default(),
            )
            .unwrap(),
        );
        assert_eq!(
            schedule.schema(),
            GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_V2_SCHEMA
        );
        assert_eq!(schedule.through_depth(), 2);
        assert_eq!(schedule.layers().len(), 3);
        assert_eq!(schedule.stats().ordering_replays(), 1);
        assert_eq!(schedule.stats().authenticated_ordering_sessions(), 1);
        assert_eq!(schedule.stats().layer_certificates(), 3);
        assert_eq!(schedule.stats().retained_ordering_references(), 4);
        assert!(schedule.same_ordering_allocation(&ordering));

        let mut expected_retained = 0usize;
        let mut expected_enumerated = 0usize;
        for (depth, layer) in schedule.layers().iter().enumerate() {
            let (enumerated, expected) = independent_expected_keys(&context, &ordering, depth);
            assert_eq!(
                layer.schema(),
                GENERATED_AFFINE_PREPARE_POINT_SCHEDULE_LAYER_V2_SCHEMA
            );
            assert_eq!(layer.depth(), depth);
            assert_eq!(layer.point_count(), expected.len());
            assert_eq!(layer.ordered_keys.as_slice(), expected.as_slice());
            assert!(Arc::ptr_eq(&layer.ordering, &ordering));
            assert_eq!(layer.stats().enumerated_offsets(), enumerated);
            for key in layer.ordered_keys.iter() {
                assert_eq!(
                    key.shift()
                        .values()
                        .iter()
                        .map(|value| value.unsigned_abs() as usize)
                        .sum::<usize>(),
                    depth
                );
                ordering.replay_key(&context, key).unwrap();
            }
            expected_retained += expected.len();
            expected_enumerated += enumerated;
        }
        assert_eq!(
            schedule.stats().prepare().retained_points(),
            expected_retained
        );
        assert_eq!(
            schedule.stats().prepare().enumerated_offsets(),
            expected_enumerated
        );

        let standalone = GeneratedAffinePreparePointLayerCertificate::compile(
            &family,
            &context,
            Arc::clone(&ordering),
            &authority,
            2,
            GeneratedAffinePreparePointLayerLimits::default(),
        )
        .unwrap();
        assert_eq!(
            standalone.schema(),
            GENERATED_AFFINE_PREPARE_POINT_LAYER_V2_SCHEMA
        );
        assert_eq!(standalone.depth(), 2);
        assert_eq!(standalone.point_count(), schedule.layers()[2].point_count());
        assert_eq!(standalone.ordered_keys, schedule.layers()[2].ordered_keys);
        assert_eq!(standalone.stats().prepare(), schedule.layers()[2].stats());
        standalone
            .replay(&family, &context, &ordering, &authority)
            .unwrap();
        schedule
            .replay(&family, &context, &ordering, &authority)
            .unwrap();
    }

    #[test]
    fn exact_outer_limits_pointer_handles_lifetime_and_redaction() {
        let (family, context, inventory, authority, ordering) =
            fixture("generated-affine-prepare-v2-binding-private");
        let baseline = GeneratedAffinePreparePointScheduleCertificate::compile(
            &family,
            &context,
            Arc::clone(&ordering),
            &authority,
            1,
            GeneratedAffinePreparePointScheduleLimits::default(),
        )
        .unwrap();
        let stats = baseline.stats();
        let mut exact = GeneratedAffinePreparePointScheduleLimits::default();
        exact.max_ordering_replays = stats.ordering_replays();
        exact.max_authenticated_ordering_sessions = stats.authenticated_ordering_sessions();
        exact.max_layer_certificates = stats.layer_certificates();
        exact.max_retained_ordering_references = stats.retained_ordering_references();
        let schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                1,
                exact,
            )
            .unwrap(),
        );

        macro_rules! schedule_outer_one_below {
            ($field:ident, $value:expr) => {
                if $value > 0 {
                    let mut limits = exact;
                    limits.$field = $value - 1;
                    assert!(matches!(
                        GeneratedAffinePreparePointScheduleCertificate::compile(
                            &family,
                            &context,
                            Arc::clone(&ordering),
                            &authority,
                            1,
                            limits,
                        ),
                        Err(GeneratedAffinePreparePointError::ResourceLimit { .. })
                    ));
                }
            };
        }
        schedule_outer_one_below!(max_ordering_replays, stats.ordering_replays());
        schedule_outer_one_below!(
            max_authenticated_ordering_sessions,
            stats.authenticated_ordering_sessions()
        );
        schedule_outer_one_below!(max_layer_certificates, stats.layer_certificates());
        schedule_outer_one_below!(
            max_retained_ordering_references,
            stats.retained_ordering_references()
        );

        let handle = schedule.point_handle(1, 0).unwrap();
        let point = schedule
            .authenticate_point_handle(
                &family,
                &context,
                &ordering,
                &authority,
                handle,
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            )
            .unwrap();
        assert_eq!(point.depth(), 1);
        assert_eq!(point.point_ordinal(), 0);
        assert!(std::ptr::eq(point.translation(), point.key().shift()));
        assert!(point.same_schedule_allocation(&schedule));
        assert!(point.same_ordering_allocation(&ordering));
        assert_eq!(point.stats().schedule_replays(), 1);
        assert_eq!(
            point.stats().pointer_checks(),
            POINT_AUTHENTICATION_POINTER_CHECKS
        );
        assert_eq!(
            point.stats().index_checks(),
            POINT_AUTHENTICATION_INDEX_CHECKS
        );

        for field in 0..3 {
            let mut limits = GeneratedAffinePreparePointAuthenticationLimits::default();
            match field {
                0 => limits.max_schedule_replays = 0,
                1 => limits.max_pointer_checks = POINT_AUTHENTICATION_POINTER_CHECKS - 1,
                _ => limits.max_index_checks = POINT_AUTHENTICATION_INDEX_CHECKS - 1,
            }
            assert!(matches!(
                schedule.authenticate_point_handle(
                    &family, &context, &ordering, &authority, handle, limits,
                ),
                Err(GeneratedAffinePreparePointError::ResourceLimit { .. })
            ));
        }

        let independent_ordering = Arc::new((*ordering).clone());
        assert!(!Arc::ptr_eq(&ordering, &independent_ordering));
        assert!(matches!(
            schedule.replay(&family, &context, &independent_ordering, &authority),
            Err(GeneratedAffinePreparePointError::OrderingAllocationMismatch)
        ));
        assert!(matches!(
            schedule.authenticate_point_handle(
                &family,
                &context,
                &independent_ordering,
                &authority,
                handle,
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            ),
            Err(GeneratedAffinePreparePointError::PointBindingMismatch)
        ));
        let other_schedule = Arc::new(
            GeneratedAffinePreparePointScheduleCertificate::compile(
                &family,
                &context,
                Arc::clone(&ordering),
                &authority,
                1,
                exact,
            )
            .unwrap(),
        );
        let foreign_handle = other_schedule.point_handle(1, 0).unwrap();
        assert!(matches!(
            schedule.authenticate_point_handle(
                &family,
                &context,
                &ordering,
                &authority,
                foreign_handle,
                GeneratedAffinePreparePointAuthenticationLimits::default(),
            ),
            Err(GeneratedAffinePreparePointError::PointBindingMismatch)
        ));

        let independent_authority = Arc::new((*authority).clone());
        assert!(
            schedule
                .replay(&family, &context, &ordering, &independent_authority)
                .is_err()
        );
        let debug = format!("{schedule:?} {handle:?} {point:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("m2"));
        assert!(!debug.contains("binding-private"));

        let weak_ordering: Weak<GeneratedAffineParametricOrderingCertificate> =
            Arc::downgrade(&ordering);
        drop(point);
        drop(other_schedule);
        drop(baseline);
        drop(independent_ordering);
        drop(independent_authority);
        drop(ordering);
        drop(authority);
        drop(inventory);
        assert!(weak_ordering.upgrade().is_some());
        drop(schedule);
        assert!(weak_ordering.upgrade().is_none());
    }

    #[test]
    fn generated_session_enforces_every_positive_cumulative_core_budget_one_below() {
        let (family, context, _inventory, authority, ordering) =
            fixture("generated-affine-prepare-v2-core-limits-private");
        ordering.replay(&family, &context, &authority).unwrap();
        let through_depth = 2usize;
        let baseline_limits = AffinePreparePointScheduleLimits::default();
        let baseline_stats = ordering
            .with_authenticated_algebra::<_, GeneratedAffinePreparePointError>(
                &context,
                |algebra| {
                    compile_affine_prepare_point_schedule_key_core(
                        algebra,
                        through_depth,
                        baseline_limits,
                        |_| Ok(()),
                    )
                    .map(AffinePreparePointScheduleCore::into_parts)
                    .map(|(_, stats)| stats)
                    .map_err(GeneratedAffinePreparePointError::Schedule)
                },
            )
            .unwrap();

        let mut depth_one_below = baseline_limits;
        depth_one_below.max_depth = through_depth - 1;
        assert!(
            ordering
                .with_authenticated_algebra::<_, GeneratedAffinePreparePointError>(
                    &context,
                    |algebra| compile_affine_prepare_point_schedule_key_core(
                        algebra,
                        through_depth,
                        depth_one_below,
                        |_| Ok(()),
                    )
                    .map(|_| ())
                    .map_err(GeneratedAffinePreparePointError::Schedule),
                )
                .is_err()
        );

        macro_rules! cumulative_one_below {
            ($field:ident, $getter:ident) => {
                if baseline_stats.$getter() > 0 {
                    let mut limits = baseline_limits;
                    limits.$field = baseline_stats.$getter() - 1;
                    assert!(
                        ordering
                            .with_authenticated_algebra::<_, GeneratedAffinePreparePointError>(
                                &context,
                                |algebra| compile_affine_prepare_point_schedule_key_core(
                                    algebra,
                                    through_depth,
                                    limits,
                                    |_| Ok(()),
                                )
                                .map(|_| ())
                                .map_err(GeneratedAffinePreparePointError::Schedule),
                            )
                            .is_err()
                    );
                }
            };
        }
        cumulative_one_below!(max_enumeration_steps, enumeration_steps);
        cumulative_one_below!(max_enumerated_offsets, enumerated_offsets);
        cumulative_one_below!(max_enumerated_components, enumerated_components);
        cumulative_one_below!(max_constant_sector_checks, constant_sector_checks);
        cumulative_one_below!(max_retained_points, retained_points);
        cumulative_one_below!(max_retained_components, retained_components);
        cumulative_one_below!(max_order_key_components, order_key_components);
        cumulative_one_below!(max_order_key_integer_bits, order_key_integer_bits);
        cumulative_one_below!(max_order_comparisons, order_comparisons);
        cumulative_one_below!(
            max_order_comparison_integer_bit_work,
            order_comparison_integer_bit_work
        );
    }

    #[test]
    fn replay_rejects_nested_schema_pointer_and_count_tampering_before_rebuild() {
        let (family, context, _inventory, authority, ordering) =
            fixture("generated-affine-prepare-v2-tamper-private");
        let schedule = GeneratedAffinePreparePointScheduleCertificate::compile(
            &family,
            &context,
            Arc::clone(&ordering),
            &authority,
            1,
            GeneratedAffinePreparePointScheduleLimits::default(),
        )
        .unwrap();

        let mut wrong_schema = schedule.clone();
        wrong_schema.schema = "wrong-generated-affine-prepare-schedule";
        assert!(matches!(
            wrong_schema.replay(&family, &context, &ordering, &authority),
            Err(GeneratedAffinePreparePointError::SchemaMismatch)
        ));
        let mut wrong_count = schedule.clone();
        wrong_count.stats.layer_certificates += 1;
        assert!(matches!(
            wrong_count.replay(&family, &context, &ordering, &authority),
            Err(GeneratedAffinePreparePointError::ReplayMismatch)
        ));
        let mut wrong_layer_schema = schedule.clone();
        Arc::make_mut(&mut wrong_layer_schema.layers)[0].schema = "wrong-schedule-layer";
        assert!(matches!(
            wrong_layer_schema.replay(&family, &context, &ordering, &authority),
            Err(GeneratedAffinePreparePointError::ReplayMismatch)
        ));
        let independent_ordering = Arc::new((*ordering).clone());
        let mut wrong_layer_ordering = schedule.clone();
        Arc::make_mut(&mut wrong_layer_ordering.layers)[0].ordering = independent_ordering;
        assert!(matches!(
            wrong_layer_ordering.replay(&family, &context, &ordering, &authority),
            Err(GeneratedAffinePreparePointError::ReplayMismatch)
        ));
    }
}
