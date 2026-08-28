use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::family::IntegralFamily;

use super::super::relation::{IndexShift, ParametricRelation};
use super::config::ParametricIbpConfig;
use super::scope::{IbpSourceLayout, IbpSourceScope};

/// A topology- and loop-count-independent generator for one complete family.
#[derive(Debug)]
pub struct ParametricIbpGenerator<'family> {
    pub(super) family: &'family IntegralFamily,
    pub(super) source_scope: IbpSourceScope,
    pub(super) context: IndexedCoefficientContext,
    pub(super) zero_shift: IndexShift,
    pub(super) positive_units: Vec<IndexShift>,
    pub(super) negative_units: Vec<IndexShift>,
    pub(super) config: ParametricIbpConfig,
}

#[derive(Debug)]
pub(super) enum PreparedIbpSource {
    CompleteOrdinary { dimension: IndexedCoefficient },
    ExternalOnly,
}

impl PreparedIbpSource {
    pub(super) const fn layout(&self) -> IbpSourceLayout {
        match self {
            Self::CompleteOrdinary { .. } => IbpSourceLayout::CompleteOrdinary,
            Self::ExternalOnly => IbpSourceLayout::ExternalOnly,
        }
    }
}

/// Immutable ordinary or external-only IBP source work prepared once for
/// deterministic ordinal execution by an application-owned executor.
#[derive(Debug)]
pub struct PreparedIbpSourceBatch<'generator, 'family> {
    pub(super) generator: &'generator ParametricIbpGenerator<'family>,
    pub(super) scope: IbpSourceScope,
    pub(super) source: PreparedIbpSource,
    pub(super) powers: Vec<IndexedCoefficient>,
    pub(super) rows: usize,
}

/// One sealed source row. Only a prepared batch can construct it; completion
/// validates its semantic scope, layout, and stable ordinal.
#[derive(Debug)]
pub struct IbpSourceRow {
    pub(super) scope: IbpSourceScope,
    pub(super) layout: IbpSourceLayout,
    pub(super) ordinal: usize,
    pub(super) relation: ParametricRelation,
}

/// A single validated ordered IBP source barrier accepted by LI preparation.
#[derive(Debug)]
pub struct CompletedIbpSourceRows {
    pub(super) scope: IbpSourceScope,
    pub(super) layout: IbpSourceLayout,
    pub(super) relations: Vec<ParametricRelation>,
}

/// Immutable LI work prepared from one completed IBP source barrier.
#[derive(Debug)]
pub struct PreparedLorentzInvarianceBatch<'generator, 'family, 'ordinary> {
    pub(super) generator: &'generator ParametricIbpGenerator<'family>,
    pub(super) ordinary: &'ordinary [ParametricRelation],
    pub(super) source_offset: usize,
    pub(super) external_pairs: Vec<(usize, usize)>,
}
