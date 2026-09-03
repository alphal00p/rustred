use std::collections::HashSet;
use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};

use crate::algebra::IndexedCoefficientContext;

use super::ModularGuideError;
use super::arena::ModularCoefficientDag;
use super::limits::ModularGuideLimits;
use super::model::{
    CoeffRef, DagOwner, ModularGuardQuery, ModularProbeCensus, ModularProbeIdentity,
    ModularQueryRole, ModularZeroEvidence,
};
use super::probe::{
    ModularEvaluationBatch, ModularEvaluationQuery, ModularProbe, RejectedProbeReport,
};

/// Result for one coefficient position in a complete consumed support batch.
/// A sampled zero is deliberately unresolved and has no exact authority.
#[derive(Debug)]
pub(super) enum NonzeroCertification {
    Certified(CertifiedNonzero),
    Unresolved(SampledZeroUnresolved),
}

/// Opaque one-sided proof that one exact coefficient root is not the zero
/// rational function.
///
/// Construction exists only through [`try_issue_support_certificates`], after
/// the complete tagged query batch has succeeded and every guard image has
/// been checked. Each proof shares one opaque validated batch seal and binds
/// its exact coefficient position within that seal. Root replay is therefore
/// constant-time; it never rescans the complete sparse-row query vector.
#[derive(Clone, Debug)]
pub(super) struct CertifiedNonzero {
    binding: BatchCertificateBinding,
    residue: u64,
}

/// A valid zero image at one query position. This records scheduling evidence
/// only; it cannot prove exact zero or be promoted to a certificate.
#[derive(Clone, Debug)]
pub(super) struct SampledZeroUnresolved {
    binding: BatchCertificateBinding,
}

#[derive(Clone, Debug)]
struct BatchCertificateBinding {
    seal: Arc<ValidatedBatchSeal>,
    query_position: usize,
}

/// Shared authority issued after exactly one complete-layout authentication.
///
/// The two boundary references are sufficient for O(1) liveness checks in
/// the append-only, suffix-rollback arena: if the greatest queried node and
/// greatest queried translation incarnation are live, every lower queried
/// slot authenticated at issuance is still live as well. A later compaction
/// generation must replace this seal rather than attempting to rebind it.
#[derive(Debug)]
struct ValidatedBatchSeal {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    queries: Arc<[ModularEvaluationQuery]>,
    guard_count: usize,
    node_boundary: Option<CoeffRef>,
    translation_boundary: Option<CoeffRef>,
    probe: Arc<ModularProbeIdentity>,
}

/// Complete ordered coefficient outcomes from one successfully consumed
/// guard-first modular batch.
///
/// The batch exposes no constructor from residues and no partial issuer. It
/// contains exactly one outcome for every coefficient root, in caller order.
#[derive(Debug)]
pub(super) struct CertifiedSupportBatch {
    seal: Arc<ValidatedBatchSeal>,
    outcomes: Box<[NonzeroCertification]>,
    census: ModularProbeCensus,
}

impl CertifiedNonzero {
    pub(super) fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: &CoeffRef,
    ) -> bool {
        self.binding.owns(dag, context, root)
    }

    /// Recheck the shared certificate boundary against the live append-only
    /// DAG without accepting a caller-supplied context identity. The DAG's
    /// authenticated context fingerprint is part of the issuance seal.
    pub(super) fn owns_live(&self, dag: &ModularCoefficientDag, root: &CoeffRef) -> bool {
        self.binding.owns_live(dag, root)
    }

    pub(super) fn probe(&self) -> &ModularProbeIdentity {
        &self.binding.seal.probe
    }

    pub(super) const fn residue(&self) -> u64 {
        self.residue
    }

    pub(super) const fn query_position(&self) -> usize {
        self.binding.query_position
    }
}

impl SampledZeroUnresolved {
    pub(super) fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: &CoeffRef,
    ) -> bool {
        self.binding.owns(dag, context, root)
    }

    pub(super) fn probe(&self) -> &ModularProbeIdentity {
        &self.binding.seal.probe
    }

    pub(super) const fn query_position(&self) -> usize {
        self.binding.query_position
    }
}

impl BatchCertificateBinding {
    fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        root: &CoeffRef,
    ) -> bool {
        let Some(query) = self.seal.queries.get(self.query_position) else {
            return false;
        };
        self.query_position >= self.seal.guard_count
            && query.role() == ModularQueryRole::Coefficient
            && query.root() == root
            && self.seal.owns_environment(dag, context)
            && dag.raw(root).is_ok()
    }

    fn owns_live(&self, dag: &ModularCoefficientDag, root: &CoeffRef) -> bool {
        let Some(query) = self.seal.queries.get(self.query_position) else {
            return false;
        };
        self.query_position >= self.seal.guard_count
            && query.role() == ModularQueryRole::Coefficient
            && query.root() == root
            && self.seal.owns_live_dag(dag)
            && dag.raw(root).is_ok()
    }
}

impl ValidatedBatchSeal {
    fn owns_environment(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
    ) -> bool {
        self.owns_live_dag(dag) && context.owns_fingerprint(&self.context_fingerprint)
    }

    fn owns_live_dag(&self, dag: &ModularCoefficientDag) -> bool {
        self.dag_owner.belongs_to(dag.owner())
            && (Arc::ptr_eq(dag.context_fingerprint(), &self.context_fingerprint)
                || dag.context_fingerprint().as_str() == self.context_fingerprint.as_str())
            && self
                .node_boundary
                .as_ref()
                .is_none_or(|boundary| dag.raw(boundary).is_ok())
            && self
                .translation_boundary
                .as_ref()
                .is_none_or(|boundary| dag.raw(boundary).is_ok())
    }

    fn layout_matches_nonzero(&self, guards: &[CoeffRef], coefficients: &[CoeffRef]) -> bool {
        nonzero_query_layout_matches(&self.queries, self.guard_count, guards, coefficients)
    }

    fn layout_matches_typed(
        &self,
        guards: &[ModularGuardQuery],
        coefficients: &[CoeffRef],
    ) -> bool {
        typed_query_layout_matches(&self.queries, self.guard_count, guards, coefficients)
    }
}

impl CertifiedSupportBatch {
    pub(super) fn outcomes(&self) -> &[NonzeroCertification] {
        &self.outcomes
    }

    pub(super) fn into_outcomes(self) -> Box<[NonzeroCertification]> {
        self.outcomes
    }

    pub(super) fn probe(&self) -> &ModularProbeIdentity {
        &self.seal.probe
    }

    pub(super) const fn census(&self) -> ModularProbeCensus {
        self.census
    }

    pub(super) fn owns(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        guards: &[CoeffRef],
        coefficients: &[CoeffRef],
    ) -> bool {
        self.seal.owns_environment(dag, context)
            && self.seal.layout_matches_nonzero(guards, coefficients)
    }

    pub(super) fn owns_typed(
        &self,
        dag: &ModularCoefficientDag,
        context: &IndexedCoefficientContext,
        guards: &[ModularGuardQuery],
        coefficients: &[CoeffRef],
    ) -> bool {
        self.seal.owns_environment(dag, context)
            && self.seal.layout_matches_typed(guards, coefficients)
    }
}

/// Construct, evaluate, and consume one complete guard-first probe batch.
/// Constructor failures and evaluation failures both return census-only
/// rejection reports. No coefficient image or certificate is released on a
/// zero guard, singularity, stale/foreign reference, or resource stop.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_certify_batch(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    guards: &[CoeffRef],
    coefficients: &[CoeffRef],
    ordinal: usize,
    modulus: u64,
    full_integer_point: &[i64],
    limits: ModularGuideLimits,
) -> Result<CertifiedSupportBatch, RejectedProbeReport> {
    let mut typed_guards = Vec::new();
    typed_guards.try_reserve_exact(guards.len()).map_err(|_| {
        RejectedProbeReport::new(
            ModularGuideError::AllocationFailure {
                resource: "modular support certificate guard queries",
                requested: guards.len(),
            },
            ModularProbeCensus::default(),
        )
    })?;
    typed_guards.extend(guards.iter().cloned().map(ModularGuardQuery::Nonzero));
    try_certify_typed_batch(
        dag,
        context,
        &typed_guards,
        coefficients,
        ordinal,
        modulus,
        full_integer_point,
        limits,
    )
}

/// Typed guarded counterpart used by ELC1 classification. `Defined` guards
/// establish point admissibility only; their own zero images are accepted.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_certify_typed_batch(
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    guards: &[ModularGuardQuery],
    coefficients: &[CoeffRef],
    ordinal: usize,
    modulus: u64,
    full_integer_point: &[i64],
    limits: ModularGuideLimits,
) -> Result<CertifiedSupportBatch, RejectedProbeReport> {
    let probe = ModularProbe::try_new(dag, context, ordinal, modulus, full_integer_point, limits)
        .map_err(|error| RejectedProbeReport::new(error, ModularProbeCensus::default()))?;
    let batch = probe.try_evaluate_typed_guarded_batch(dag, guards, coefficients)?;
    let census = batch.census();
    try_issue_typed_support_certificates(batch, dag, context, guards, coefficients)
        .map_err(|error| RejectedProbeReport::new(error, census))
}

/// Consume one already complete tagged batch and issue root-bound outcomes.
///
/// The expected layout is authenticated in full before the first certificate
/// is built. There is no scalar or prefix variant of this function.
pub(super) fn try_issue_support_certificates(
    batch: ModularEvaluationBatch,
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    expected_guards: &[CoeffRef],
    expected_coefficients: &[CoeffRef],
) -> Result<CertifiedSupportBatch, ModularGuideError> {
    let mut typed_guards = Vec::new();
    typed_guards
        .try_reserve_exact(expected_guards.len())
        .map_err(|_| ModularGuideError::AllocationFailure {
            resource: "modular support certificate guard queries",
            requested: expected_guards.len(),
        })?;
    typed_guards.extend(
        expected_guards
            .iter()
            .cloned()
            .map(ModularGuardQuery::Nonzero),
    );
    try_issue_typed_support_certificates(batch, dag, context, &typed_guards, expected_coefficients)
}

/// Consume one complete typed guarded batch and issue support outcomes only
/// after authenticating the canonical unique layout in full.
pub(super) fn try_issue_typed_support_certificates(
    batch: ModularEvaluationBatch,
    dag: &ModularCoefficientDag,
    context: &IndexedCoefficientContext,
    expected_guards: &[ModularGuardQuery],
    expected_coefficients: &[CoeffRef],
) -> Result<CertifiedSupportBatch, ModularGuideError> {
    if !batch.belongs_to_dag_owner(dag) {
        return Err(ModularGuideError::WrongDagOwner);
    }
    if !batch.owns_context(context) {
        return Err(ModularGuideError::WrongIndexedContext);
    }
    if !typed_query_layout_matches(
        batch.queries(),
        batch.guard_count(),
        expected_guards,
        expected_coefficients,
    ) {
        return Err(ModularGuideError::InconsistentBatchQueryLayout);
    }
    if batch.images().len() != batch.queries().len() {
        return Err(ModularGuideError::InconsistentBatchQueryLayout);
    }
    let mut unique_roots = HashSet::new();
    unique_roots
        .try_reserve(batch.queries().len())
        .map_err(|_| ModularGuideError::AllocationFailure {
            resource: "modular support certificate unique query roots",
            requested: batch.queries().len(),
        })?;
    let mut node_boundary: Option<CoeffRef> = None;
    let mut translation_boundary: Option<CoeffRef> = None;
    for query in batch.queries() {
        dag.raw(query.root())?;
        if !unique_roots.insert(query.root().clone()) {
            return Err(ModularGuideError::InconsistentBatchQueryLayout);
        }
        if node_boundary
            .as_ref()
            .is_none_or(|current| query.root().raw.node > current.raw.node)
        {
            node_boundary = Some(query.root().clone());
        }
        if translation_boundary
            .as_ref()
            .is_none_or(|current| query.root().raw.translation > current.raw.translation)
        {
            translation_boundary = Some(query.root().clone());
        }
    }
    if batch.images()[..batch.guard_count()]
        .iter()
        .zip(expected_guards)
        .any(|(image, guard)| {
            guard.requires_nonzero() && image.zero_evidence() != ModularZeroEvidence::Nonzero
        })
    {
        return Err(ModularGuideError::SampledZeroLocalizationGuard);
    }

    let parts = batch.into_parts();
    let queries: Arc<[ModularEvaluationQuery]> = Arc::from(parts.queries);
    let guard_count = parts.guard_count;
    let census = parts.census;
    let seal = Arc::new(ValidatedBatchSeal {
        dag_owner: parts.dag_owner,
        context_fingerprint: parts.context_fingerprint,
        queries,
        guard_count,
        node_boundary,
        translation_boundary,
        probe: parts.identity,
    });
    let coefficient_count = seal.queries.len().saturating_sub(guard_count);
    let mut outcomes = Vec::new();
    outcomes.try_reserve_exact(coefficient_count).map_err(|_| {
        ModularGuideError::AllocationFailure {
            resource: "modular support certificate outcomes",
            requested: coefficient_count,
        }
    })?;
    let field = Zp64::new(seal.probe.modulus());
    for (query_position, image) in parts
        .images
        .into_vec()
        .into_iter()
        .enumerate()
        .skip(guard_count)
    {
        let binding = BatchCertificateBinding {
            seal: Arc::clone(&seal),
            query_position,
        };
        match image.zero_evidence() {
            ModularZeroEvidence::KnownZero => {
                return Err(ModularGuideError::KnownZeroCannotBeCertified);
            }
            ModularZeroEvidence::SampledZero => {
                outcomes.push(NonzeroCertification::Unresolved(SampledZeroUnresolved {
                    binding,
                }))
            }
            ModularZeroEvidence::Nonzero => {
                outcomes.push(NonzeroCertification::Certified(CertifiedNonzero {
                    binding,
                    residue: field.from_element(image.value()),
                }));
            }
        }
    }

    Ok(CertifiedSupportBatch {
        seal,
        outcomes: outcomes.into_boxed_slice(),
        census,
    })
}

fn nonzero_query_layout_matches(
    actual: &[ModularEvaluationQuery],
    guard_count: usize,
    expected_guards: &[CoeffRef],
    expected_coefficients: &[CoeffRef],
) -> bool {
    if guard_count != expected_guards.len()
        || actual.len()
            != expected_guards
                .len()
                .saturating_add(expected_coefficients.len())
    {
        return false;
    }
    actual[..guard_count]
        .iter()
        .zip(expected_guards)
        .all(|(query, expected)| {
            query.role() == ModularQueryRole::Guard && query.root() == expected
        })
        && actual[guard_count..]
            .iter()
            .zip(expected_coefficients)
            .all(|(query, expected)| {
                query.role() == ModularQueryRole::Coefficient && query.root() == expected
            })
}

fn typed_query_layout_matches(
    actual: &[ModularEvaluationQuery],
    guard_count: usize,
    expected_guards: &[ModularGuardQuery],
    expected_coefficients: &[CoeffRef],
) -> bool {
    if guard_count != expected_guards.len()
        || actual.len()
            != expected_guards
                .len()
                .saturating_add(expected_coefficients.len())
    {
        return false;
    }
    actual[..guard_count]
        .iter()
        .zip(expected_guards)
        .all(|(query, expected)| query.role() == expected.role() && query.root() == expected.root())
        && actual[guard_count..]
            .iter()
            .zip(expected_coefficients)
            .all(|(query, expected)| {
                query.role() == ModularQueryRole::Coefficient && query.root() == expected
            })
}
