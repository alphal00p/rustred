use std::time::{Duration, Instant};

use rustred::family::IntegralFamily;
use rustred::foundry::artifact::{SourcePortAudit, SourcePortAuditError};
use rustred::identity::ParametricIbpGenerator;
use serde::Serialize;

use crate::application::resource_policy::ResourcePolicyOutput;
use crate::application::{AppError, MAX_CLOSING_ARTIFACT_BYTES};

use super::{codec, model::*, preparation};

/// Replay and certify trusted generated candidate programs. Native binary
/// decoding requires trusted provenance independently of this mathematical
/// proof step: neither parsing nor the saved status grants proof authority.
pub fn certify_candidates(
    request: CandidateCertificationRequest,
) -> Result<CandidateCertificationResult, AppError> {
    if let Some(degree) = request.max_negative_index_degree {
        if degree > MAX_RANK_SCOPED_CERTIFICATION_DEGREE {
            return Err(AppError::input(format!(
                "max-negative-index-degree {degree} exceeds the supported rank-scoped limit {MAX_RANK_SCOPED_CERTIFICATION_DEGREE}"
            )));
        }
        return Err(AppError::execution(format!(
            "rank-scoped certification through max-negative-index-degree={degree} is not yet available: the durable artifact schema and runtime reducer do not persist or enforce a successor-closed entry scope; refusing an unbounded certification fallback"
        )));
    }
    let started = Instant::now();
    request
        .publication_limits
        .validate()
        .map_err(publication_error)?;
    let bundle = codec::read(&request.bundle, request.input_limits)?;
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
    macro_rules! dispatch {
        ($($n:literal),+) => { match family.denominator_count() {
            $($n => certify::<$n>(family, bundle, request, started, decoded_at),)+
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
    let artifact = audit
        .install_complete(
            prepared.family,
            solved
                .into_iter()
                .map(|(sector, solution)| (sector, permutation, solution)),
        )
        .map_err(publication_error)?;
    let certified_at = started.elapsed();
    let bytes = artifact
        .encode_durable()
        .map_err(|e| AppError::serialization(e.to_string()))?;
    if bytes.len() > MAX_CLOSING_ARTIFACT_BYTES {
        return Err(AppError::output_limit(
            "certified artifact exceeds application byte ceiling",
        ));
    }
    let elapsed = started.elapsed();
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
