//! Verified component charts retain the momentum witness, including numerators.
mod prepare;
mod projection;

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use super::{VacuumProductLimits, VacuumProductStatistics};
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::symmetry::{self, integral_transport};

pub use projection::{
    VacuumProductProjection, VacuumTensorProjection, VacuumTensorProjectionInput,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumProductChartLimits {
    pub products: VacuumProductLimits,
    pub verification: symmetry::Limits,
    pub transport: integral_transport::ExpansionLimits,
}

/// One independent canonical momentum block. Both ranges refer to the chart's
/// combined family, not to the original family's denominator ordering.
#[derive(Clone, Debug)]
pub struct VacuumProductComponent {
    pub(super) family: Arc<IntegralFamily>,
    pub(super) base: IntegralKey,
    pub(super) loops: Range<usize>,
    pub(super) denominators: Range<usize>,
}
impl VacuumProductComponent {
    pub fn family(&self) -> &Arc<IntegralFamily> {
        &self.family
    }
    pub fn base_integral(&self) -> &IntegralKey {
        &self.base
    }
    pub fn loop_range(&self) -> Range<usize> {
        self.loops.clone()
    }
    pub fn denominator_range(&self) -> Range<usize> {
        self.denominators.clone()
    }
}

/// A unit-measure coordinate change, not yet a scalar product identity.
/// Negative source rows are retained in `transport().verified_map()`; no
/// numerator coupling is discarded by preparing this chart.
#[derive(Debug)]
pub struct VerifiedVacuumProductChart {
    pub(super) source: IntegralKey,
    pub(super) combined: Arc<IntegralFamily>,
    pub(super) base: IntegralKey,
    pub(super) components: Vec<VacuumProductComponent>,
    pub(super) transport: integral_transport::Prepared,
    pub(super) limits: VacuumProductChartLimits,
}
impl VerifiedVacuumProductChart {
    pub fn source(&self) -> &IntegralKey {
        &self.source
    }
    pub fn combined_family(&self) -> &Arc<IntegralFamily> {
        &self.combined
    }
    pub fn base_integral(&self) -> &IntegralKey {
        &self.base
    }
    pub fn components(&self) -> &[VacuumProductComponent] {
        &self.components
    }
    pub fn transport(&self) -> &integral_transport::Prepared {
        &self.transport
    }
}

/// Optional finite numerator-aware geometry. Support discovery and bounded
/// canonical searches are shared across all keys of the same positive support.
/// Connected, rank-deficient, or unsupported supports have no chart.
#[derive(Debug)]
pub struct VacuumProductChartPlan {
    pub(super) charts: BTreeMap<IntegralKey, VerifiedVacuumProductChart>,
    pub(super) statistics: VacuumProductStatistics,
}
impl VacuumProductChartPlan {
    pub fn charts(&self) -> &BTreeMap<IntegralKey, VerifiedVacuumProductChart> {
        &self.charts
    }
    pub fn statistics(&self) -> &VacuumProductStatistics {
        &self.statistics
    }
}
