use std::sync::Arc;

use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};

use crate::algebra::IndexedCoefficientContext;

use super::ModularGuideError;
use super::arena::ModularCoefficientDag;
use super::limits::ModularGuideLimits;
use super::model::{
    CoeffRef, DagOwner, ModularEvaluationBatch, ModularEvaluationQuery, ModularProbeCensus,
    ModularProbeIdentity, ModularQueryRole, ModularZeroEvidence,
};
use super::probe::{ModularProbe, RejectedProbeReport};

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
/// been checked. Each proof shares the immutable ordered query layout and
/// binds its exact coefficient position within that layout.
#[derive(Debug)]
pub(super) struct CertifiedNonzero {
    binding: BatchCertificateBinding,
    residue: u64,
}

/// A valid zero image at one query position. This records scheduling evidence
/// only; it cannot prove exact zero or be promoted to a certificate.
#[derive(Debug)]
pub(super) struct SampledZeroUnresolved {
    binding: BatchCertificateBinding,
}

#[derive(Debug)]
struct BatchCertificateBinding {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    queries: Arc<[ModularEvaluationQuery]>,
    guard_count: usize,
    query_position: usize,
    probe: Arc<ModularProbeIdentity>,
}

/// Complete ordered coefficient outcomes from one successfully consumed
/// guard-first modular batch.
///
/// The batch exposes no constructor from residues and no partial issuer. It
/// contains exactly one outcome for every coefficient root, in caller order.
#[derive(Debug)]
pub(super) struct CertifiedSupportBatch {
    dag_owner: DagOwner,
    context_fingerprint: Arc<String>,
    queries: Arc<[ModularEvaluationQuery]>,
    guard_count: usize,
    outcomes: Box<[NonzeroCertification]>,
    probe: Arc<ModularProbeIdentity>,
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

    pub(super) fn probe(&self) -> &ModularProbeIdentity {
        &self.binding.probe
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
        &self.binding.probe
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
        let Some(query) = self.queries.get(self.query_position) else {
            return false;
        };
        self.query_position >= self.guard_count
            && query.role == ModularQueryRole::Coefficient
            && query.root == *root
            && self.dag_owner.belongs_to(dag.owner())
            && context.owns_fingerprint(&self.context_fingerprint)
            && self
                .queries
                .iter()
                .all(|query| dag.raw(&query.root).is_ok())
            && dag.raw(&query.root).is_ok()
            && dag.raw(root).is_ok()
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
        &self.probe
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
        self.dag_owner.belongs_to(dag.owner())
            && context.owns_fingerprint(&self.context_fingerprint)
            && query_layout_matches(&self.queries, self.guard_count, guards, coefficients)
            && self
                .queries
                .iter()
                .all(|query| dag.raw(&query.root).is_ok())
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
    let probe = ModularProbe::try_new(dag, context, ordinal, modulus, full_integer_point, limits)
        .map_err(|error| RejectedProbeReport::new(error, ModularProbeCensus::default()))?;
    let batch = probe.try_evaluate_guarded_batch(dag, guards, coefficients)?;
    let census = batch.census();
    try_issue_support_certificates(batch, dag, context, guards, coefficients)
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
    if !batch.dag_owner.belongs_to(dag.owner()) {
        return Err(ModularGuideError::WrongDagOwner);
    }
    for query in batch.queries() {
        dag.raw(&query.root)?;
    }
    if !batch.owns_context(context) {
        return Err(ModularGuideError::WrongIndexedContext);
    }
    if !query_layout_matches(
        batch.queries(),
        batch.guard_count(),
        expected_guards,
        expected_coefficients,
    ) {
        return Err(ModularGuideError::InconsistentBatchQueryLayout);
    }
    if batch.images.len() != batch.queries.len() {
        return Err(ModularGuideError::InconsistentBatchQueryLayout);
    }
    if batch.images[..batch.guard_count]
        .iter()
        .any(|image| image.zero_evidence() != ModularZeroEvidence::Nonzero)
    {
        return Err(ModularGuideError::SampledZeroLocalizationGuard);
    }

    let dag_owner = batch.dag_owner;
    let context_fingerprint = batch.context_fingerprint;
    let queries: Arc<[ModularEvaluationQuery]> = Arc::from(batch.queries);
    let probe = batch.identity;
    let guard_count = batch.guard_count;
    let census = batch.census;
    let coefficient_count = queries.len().saturating_sub(guard_count);
    let mut outcomes = Vec::new();
    outcomes.try_reserve_exact(coefficient_count).map_err(|_| {
        ModularGuideError::AllocationFailure {
            resource: "modular support certificate outcomes",
            requested: coefficient_count,
        }
    })?;
    let field = Zp64::new(probe.modulus());
    for (query_position, image) in batch
        .images
        .into_vec()
        .into_iter()
        .enumerate()
        .skip(guard_count)
    {
        let binding = BatchCertificateBinding {
            dag_owner: dag_owner.clone(),
            context_fingerprint: Arc::clone(&context_fingerprint),
            queries: Arc::clone(&queries),
            guard_count,
            query_position,
            probe: Arc::clone(&probe),
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
        dag_owner,
        context_fingerprint,
        queries,
        guard_count,
        outcomes: outcomes.into_boxed_slice(),
        probe,
        census,
    })
}

fn query_layout_matches(
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
        .all(|(query, expected)| query.role == ModularQueryRole::Guard && query.root == *expected)
        && actual[guard_count..]
            .iter()
            .zip(expected_coefficients)
            .all(|(query, expected)| {
                query.role == ModularQueryRole::Coefficient && query.root == *expected
            })
}
