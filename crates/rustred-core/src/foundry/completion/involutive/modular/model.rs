use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::algebra::IndexedCoefficient;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct CoeffNodeId(u64);

impl CoeffNodeId {
    pub(super) fn try_new(value: usize, incarnation: u32) -> Option<Self> {
        let ordinal = u32::try_from(value).ok()?;
        Some(Self((u64::from(incarnation) << 32) | u64::from(ordinal)))
    }

    pub(super) const fn ordinal(self) -> u32 {
        self.0 as u32
    }

    pub(super) const fn as_usize(self) -> usize {
        self.ordinal() as usize
    }

    pub(super) const fn incarnation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct PhysicalDeltaId(u64);

impl PhysicalDeltaId {
    pub(super) fn try_new(value: usize, incarnation: u32) -> Option<Self> {
        let ordinal = u32::try_from(value).ok()?;
        Some(Self((u64::from(incarnation) << 32) | u64::from(ordinal)))
    }

    pub(super) const fn as_usize(self) -> usize {
        self.ordinal() as usize
    }

    pub(super) const fn ordinal(self) -> u32 {
        self.0 as u32
    }

    pub(super) const fn incarnation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct AccumulatedDeltaId(u32);

impl AccumulatedDeltaId {
    pub(super) fn try_new(value: usize) -> Option<Self> {
        u32::try_from(value).ok().map(Self)
    }

    pub(super) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct ExactLeafId(u32);

impl ExactLeafId {
    pub(super) fn try_new(value: usize) -> Option<Self> {
        u32::try_from(value).ok().map(Self)
    }

    pub(super) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug)]
pub(super) struct DagOwner(Arc<()>);

impl DagOwner {
    pub(super) fn fresh() -> Self {
        Self(Arc::new(()))
    }

    pub(super) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for DagOwner {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for DagOwner {}

impl Hash for DagOwner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) struct RawCoeffRef {
    pub(super) node: CoeffNodeId,
    pub(super) translation: PhysicalDeltaId,
}

/// One field-independent coefficient-expression reference.
///
/// The opaque owner prevents equal numeric IDs from another arena from being
/// mistaken for this expression.  Its translation is a signed physical-index
/// delta; base parameters are never translated.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct CoeffRef {
    pub(super) owner: DagOwner,
    pub(super) raw: RawCoeffRef,
}

impl CoeffRef {
    pub(super) const fn node_ordinal(&self) -> u32 {
        self.raw.node.ordinal()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum CoeffNode {
    Zero,
    One,
    ExactLeaf(ExactLeafId),
    Neg(RawCoeffRef),
    Add(RawCoeffRef, RawCoeffRef),
    Mul(RawCoeffRef, RawCoeffRef),
    Inv(RawCoeffRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct EvaluationKey {
    pub(super) node: CoeffNodeId,
    pub(super) translation: AccumulatedDeltaId,
}

/// Whether a zero image was established structurally or merely sampled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModularZeroEvidence {
    KnownZero,
    SampledZero,
    Nonzero,
}

/// Semantic position of one query in a consumed modular batch.
///
/// ELC1 batches have one canonical layout: every guard precedes every
/// coefficient root.  Keeping the role beside the owned root prevents a raw
/// positional image from being reinterpreted under another layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModularQueryRole {
    /// A localization polynomial whose image must be nonzero.
    Guard,
    /// A rational coefficient which must be defined at the point, but whose
    /// numerator is allowed to vanish there.
    DefinedGuard,
    Coefficient,
}

/// Typed probe-point admissibility condition.
///
/// `Nonzero` is used for an exact polynomial localization condition.
/// `Defined` is used for `DenominatorOf(c)`: evaluating `c` already checks
/// every exact-leaf denominator and inverse on its path, while a zero value of
/// `c` itself is admissible and must not reject the point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ModularGuardQuery {
    Nonzero(CoeffRef),
    Defined(CoeffRef),
}

impl ModularGuardQuery {
    pub(super) fn root(&self) -> &CoeffRef {
        match self {
            Self::Nonzero(root) | Self::Defined(root) => root,
        }
    }

    pub(super) const fn role(&self) -> ModularQueryRole {
        match self {
            Self::Nonzero(_) => ModularQueryRole::Guard,
            Self::Defined(_) => ModularQueryRole::DefinedGuard,
        }
    }

    pub(super) const fn requires_nonzero(&self) -> bool {
        matches!(self, Self::Nonzero(_))
    }
}

/// Immutable identity of one independent `(prime, point)` lane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModularProbeIdentity {
    ordinal: usize,
    modulus: u64,
    point: Arc<Vec<i64>>,
    residues: Arc<Vec<u64>>,
}

impl ModularProbeIdentity {
    pub(super) fn new(
        ordinal: usize,
        modulus: u64,
        point: Arc<Vec<i64>>,
        residues: Arc<Vec<u64>>,
    ) -> Self {
        Self {
            ordinal,
            modulus,
            point,
            residues,
        }
    }

    pub(super) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(super) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(super) fn point(&self) -> &[i64] {
        self.point.as_slice()
    }

    pub(super) fn residues(&self) -> &[u64] {
        self.residues.as_slice()
    }

    /// Compare canonical field points, independently of scheduling ordinal
    /// and of the integer representatives supplied by callers.
    pub(super) fn residue_equivalent(&self, other: &Self) -> bool {
        self.modulus == other.modulus && self.residues == other.residues
    }
}

/// Cumulative work performed by one probe before success or rejection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ModularProbeCensus {
    pub(super) queries: usize,
    pub(super) delta_compositions: usize,
    pub(super) delta_coordinate_operations: usize,
    pub(super) evaluation_steps: usize,
    pub(super) evaluation_frame_pushes: usize,
    pub(super) peak_live_evaluation_frames: usize,
    pub(super) peak_live_evaluation_values: usize,
    pub(super) cache_hits: usize,
    pub(super) exact_leaf_evaluations: usize,
    pub(super) exact_leaf_terms_evaluated: usize,
    pub(super) exact_leaf_exponent_cells_evaluated: usize,
}

impl ModularProbeCensus {
    pub(super) const fn queries(self) -> usize {
        self.queries
    }

    pub(super) const fn delta_compositions(self) -> usize {
        self.delta_compositions
    }

    pub(super) const fn evaluation_steps(self) -> usize {
        self.evaluation_steps
    }

    pub(super) const fn evaluation_frame_pushes(self) -> usize {
        self.evaluation_frame_pushes
    }

    pub(super) const fn peak_live_evaluation_frames(self) -> usize {
        self.peak_live_evaluation_frames
    }

    pub(super) const fn peak_live_evaluation_values(self) -> usize {
        self.peak_live_evaluation_values
    }

    pub(super) const fn delta_coordinate_operations(self) -> usize {
        self.delta_coordinate_operations
    }

    pub(super) const fn cache_hits(self) -> usize {
        self.cache_hits
    }

    pub(super) const fn exact_leaf_evaluations(self) -> usize {
        self.exact_leaf_evaluations
    }

    pub(super) const fn exact_leaf_terms_evaluated(self) -> usize {
        self.exact_leaf_terms_evaluated
    }

    pub(super) const fn exact_leaf_exponent_cells_evaluated(self) -> usize {
        self.exact_leaf_exponent_cells_evaluated
    }
}

pub(super) type ExactLeaf = Arc<IndexedCoefficient>;

/// Structural lookup key for one immutable exact leaf.  The arena's indexed
/// context is fixed, so hashing the canonical Symbolica rational polynomial
/// and checking full authenticated-value equality gives stable hash-consing
/// without depending on incidental `Arc` allocation identity.
#[derive(Clone, Debug)]
pub(super) struct ExactLeafKey(pub(super) ExactLeaf);

impl PartialEq for ExactLeafKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ExactLeafKey {}

impl Hash for ExactLeafKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.raw().hash(state);
    }
}
