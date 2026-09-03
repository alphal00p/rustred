use std::hash::{Hash, Hasher};
use std::sync::Arc;

use symbolica::domains::finite_field::FiniteFieldElement;

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
    Guard,
    Coefficient,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ModularEvaluationQuery {
    pub(super) role: ModularQueryRole,
    pub(super) root: CoeffRef,
}

impl ModularEvaluationQuery {
    pub(super) fn root(&self) -> &CoeffRef {
        &self.root
    }

    pub(super) const fn role(&self) -> ModularQueryRole {
        self.role
    }
}

/// One scalar image from a single independent modular probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ModularImage {
    value: FiniteFieldElement<u64>,
    zero_evidence: ModularZeroEvidence,
}

impl ModularImage {
    pub(super) fn new(value: FiniteFieldElement<u64>, zero_evidence: ModularZeroEvidence) -> Self {
        Self {
            value,
            zero_evidence,
        }
    }

    pub(super) const fn value(&self) -> &FiniteFieldElement<u64> {
        &self.value
    }

    pub(super) const fn zero_evidence(&self) -> ModularZeroEvidence {
        self.zero_evidence
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

/// A complete, successfully evaluated coefficient batch from one consumed
/// probe. No scalar image crosses the module boundary before every requested
/// coefficient has succeeded; on any singularity or resource stop the partial
/// buffer is dropped with the rejected probe.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModularEvaluationBatch {
    pub(super) identity: Arc<ModularProbeIdentity>,
    pub(super) dag_owner: DagOwner,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) queries: Box<[ModularEvaluationQuery]>,
    pub(super) guard_count: usize,
    pub(super) images: Box<[ModularImage]>,
    pub(super) census: ModularProbeCensus,
}

impl ModularEvaluationBatch {
    pub(super) fn identity(&self) -> &ModularProbeIdentity {
        &self.identity
    }

    pub(super) fn images(&self) -> &[ModularImage] {
        &self.images
    }

    /// The exact ordered coefficient references corresponding one-to-one to
    /// [`Self::images`].  Retaining these references prevents positional
    /// images from being accidentally reinterpreted as another query batch.
    pub(super) fn queries(&self) -> &[ModularEvaluationQuery] {
        &self.queries
    }

    pub(super) const fn guard_count(&self) -> usize {
        self.guard_count
    }

    pub(super) fn owns_context(&self, context: &crate::algebra::IndexedCoefficientContext) -> bool {
        context.owns_fingerprint(&self.context_fingerprint)
    }

    pub(super) fn owns_dag(&self, dag: &super::arena::ModularCoefficientDag) -> bool {
        self.dag_owner.belongs_to(dag.owner())
            && self
                .queries
                .iter()
                .all(|query| dag.raw(&query.root).is_ok())
    }

    pub(super) const fn census(&self) -> ModularProbeCensus {
        self.census
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
