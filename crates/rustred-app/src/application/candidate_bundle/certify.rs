use std::time::{Duration, Instant};

use rustred::family::IntegralFamily;
use rustred::foundry::artifact::{SourcePortAudit, SourcePortAuditError};
use rustred::identity::ParametricIbpGenerator;
use serde::Serialize;

use crate::application::family_close::progress::{
    FamilyCloseProgress, Observer, emit, installation_event,
};
use crate::application::resource_policy::ResourcePolicyOutput;
use crate::application::{AppError, MAX_CLOSING_ARTIFACT_BYTES};

use super::{codec, model::*, preparation};

/// Replay and certify trusted generated candidate programs. Native binary
/// decoding requires trusted provenance independently of this mathematical
/// proof step: neither parsing nor the saved status grants proof authority.
pub fn certify_candidates(
    request: CandidateCertificationRequest,
) -> Result<CandidateCertificationResult, AppError> {
    certify_request(request, None)
}

/// Observe saved-program preparation, replay, installation and encoding.
/// No generation is run or reported. The callback cannot alter proof policy.
pub fn certify_candidates_with_progress(
    request: CandidateCertificationRequest,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<CandidateCertificationResult, AppError> {
    certify_request(request, Some(&observe))
}

fn certify_request(
    request: CandidateCertificationRequest,
    observe: Observer<'_>,
) -> Result<CandidateCertificationResult, AppError> {
    if request.max_negative_index_degree.is_some() && request.max_total_excess_degree.is_some() {
        return Err(AppError::input(
            "numerator-only and total-excess certification bounds are mutually exclusive",
        ));
    }
    if let Some(degree) = request.max_negative_index_degree {
        if degree > MAX_RANK_SCOPED_CERTIFICATION_DEGREE {
            return Err(AppError::input(format!(
                "max-negative-index-degree {degree} exceeds the supported rank-scoped limit {MAX_RANK_SCOPED_CERTIFICATION_DEGREE}"
            )));
        }
        return Err(AppError::execution(format!(
            "rank-scoped certification through max-negative-index-degree={degree} is not yet available: a numerator-only successor-closed entry scope leaves dots unbounded and is not implemented; refusing an unbounded certification fallback"
        )));
    }
    let started = Instant::now();
    request
        .publication_limits
        .validate()
        .map_err(publication_error)?;
    let bundle = codec::read(&request.bundle, request.input_limits)?;
    if super::policy::parse(&bundle.solver_policy)?
        .max_numerator_rank
        .is_some()
    {
        return Err(AppError::execution(
            "numerator-rank-scoped candidates cannot be certified: a compatible successor-closed proof owner is not implemented; refusing unrestricted or total-excess fallback",
        ));
    }
    let decoded_at = started.elapsed();
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            request.input_limits.family_limits(),
            request.input_limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint
        || family.denominator_count() != bundle.root_sector.len()
    {
        return Err(AppError::input(
            "candidate family binding differs from reconstructed family",
        ));
    }
    emit(observe, || FamilyCloseProgress::Preparing {
        arity: family.denominator_count(),
        elapsed: started.elapsed(),
    });
    macro_rules! dispatch {
        ($($n:literal),+) => { match family.denominator_count() {
            $($n => certify::<$n>(family, bundle, request, started, decoded_at, observe),)+
            _ => unreachable!("candidate arity checked"),
        } };
    }
    dispatch!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn certify<const N: usize>(
    family: IntegralFamily,
    bundle: Bundle,
    request: CandidateCertificationRequest,
    started: Instant,
    decoded_at: Duration,
    observe: Observer<'_>,
) -> Result<CandidateCertificationResult, AppError> {
    SourcePortAudit::<N>::validate_install_family(&family)
        .map_err(|error| AppError::input(error.to_string()))?;
    let prepared =
        preparation::prepare::<N>(family, &bundle.root_sector, bundle.permutation.as_deref())?;
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|e| AppError::execution(e.to_string()))?
        .context()
        .clone();
    let prepared_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Prepared {
        sectors: prepared.sectors.len(),
        zero_sectors: prepared
            .zeros
            .iter()
            .filter(|sector| {
                sector
                    .iter()
                    .zip(prepared.root)
                    .all(|(&active, allowed)| !active || allowed)
            })
            .count(),
        global_zero_sectors: prepared.zeros.len(),
        elapsed: prepared_at,
    });
    let solved = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        request.input_limits,
    )?;
    let reconstructed_at = started.elapsed();
    drop(bundle);
    let permutation = prepared.permutation;
    let audit =
        SourcePortAudit::try_new_with_root_sector(&prepared.family, prepared.zeros, prepared.root)
            .map_err(publication_error)?
            .with_limits(request.publication_limits);
    let sectors = solved
        .into_iter()
        .map(|(sector, solution)| (sector, permutation, solution));
    let artifact = match request.max_total_excess_degree {
        Some(degree) => audit.install_complete_through_total_excess_with_observer(
            prepared.family,
            sectors,
            degree,
            |event| emit(observe, || installation_event(event, started.elapsed())),
        ),
        None => audit.install_complete_with_observer(prepared.family, sectors, |event| {
            emit(observe, || installation_event(event, started.elapsed()))
        }),
    }
    .map_err(publication_error)?;
    let certified_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoding {
        elapsed: certified_at,
    });
    let bytes = artifact
        .encode_durable()
        .map_err(|e| AppError::serialization(e.to_string()))?;
    if bytes.len() > MAX_CLOSING_ARTIFACT_BYTES {
        return Err(AppError::output_limit(
            "certified artifact exceeds application byte ceiling",
        ));
    }
    let elapsed = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoded {
        bytes: bytes.len(),
        elapsed,
    });
    #[derive(Serialize)]
    struct Report<'a> {
        schema: &'static str,
        status: &'static str,
        workload: &'static str,
        family_fingerprint: &'a str,
        artifact_schema: &'static str,
        rule_cells: usize,
        terminals: usize,
        bytes: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_total_excess_degree: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        successor_sector_count: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_successor_total_excess_degree: Option<u64>,
        bundle_decoding_us: u128,
        preparation_us: u128,
        native_reconstruction_us: u128,
        certification_us: u128,
        artifact_encoding_us: u128,
        total_us: u128,
        publication_resources: ResourcePolicyOutput,
    }
    let report = Report {
        schema: CANDIDATE_CERTIFICATION_SCHEMA,
        status: "generated-durable",
        workload: "reconstruct-replay-certify-encode; no solve and no cold reload",
        family_fingerprint: artifact.family_fingerprint(),
        artifact_schema: artifact.schema().stable_id(),
        rule_cells: artifact.rule_cells().len(),
        terminals: artifact.masters().len(),
        bytes: bytes.len(),
        max_total_excess_degree: artifact
            .total_excess_scope()
            .map(|scope| scope.max_entry_total_excess_degree()),
        successor_sector_count: artifact
            .total_excess_scope()
            .map(|scope| scope.successor_degrees().len()),
        max_successor_total_excess_degree: artifact
            .total_excess_scope()
            .and_then(|scope| scope.successor_degrees().values().copied().max()),
        bundle_decoding_us: decoded_at.as_micros(),
        preparation_us: (prepared_at - decoded_at).as_micros(),
        native_reconstruction_us: (reconstructed_at - prepared_at).as_micros(),
        certification_us: (certified_at - reconstructed_at).as_micros(),
        artifact_encoding_us: (elapsed - certified_at).as_micros(),
        total_us: elapsed.as_micros(),
        publication_resources: ResourcePolicyOutput::new(
            request
                .publication_limits
                .rule_derivation
                .max_domain_bound_endpoint_cells,
            request.publication_limits.max_predicate_consistency_work,
            request.publication_limits.max_predicate_atoms,
        ),
    };
    Ok(CandidateCertificationResult {
        artifact: bytes,
        report_toml: toml::to_string_pretty(&report)
            .map_err(|e| AppError::serialization(e.to_string()))?,
    })
}

fn publication_error(error: SourcePortAuditError) -> AppError {
    match error {
        SourcePortAuditError::ResourceBudgetExhausted { .. }
        | SourcePortAuditError::UnsupportedResourcePolicy { .. } => {
            AppError::limit(error.to_string())
        }
        _ => AppError::execution(error.to_string()),
    }
}
