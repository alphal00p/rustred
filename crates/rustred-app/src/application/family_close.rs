//! Complete artifact generation from caller-supplied family data.
//!
//! This layer only orchestrates existing core services: exact zero analysis,
//! generic sector solving, original-source replay, coverage installation and
//! durable encoding. It supplies neither relations nor topology selectors.

use std::time::{Duration, Instant};

use rustred::family::IntegralFamily;
use rustred::foundry::artifact::{
    ClosedArtifact, SourcePortAudit, SourcePortAuditError, SourcePortLimits,
};
use rustred::solver::{SectorConfig, SectorExecutor, SectorSolveOptions};
use serde::Serialize;

use super::candidate_bundle::preparation;
use super::error::AppError;
use super::input::prepare_input;
use super::lowering::lower_project;
use super::resource_policy::ResourcePolicyOutput;
use super::{InputFormat, MAX_CLOSING_ARTIFACT_BYTES, MAX_INPUT_BYTES};

pub(super) mod progress;
mod scope;
pub use progress::{FamilyCloseGenerationStage, FamilyCloseProgress};
use progress::{Observer, emit, generation_stage, installation_event, sector_mask};

pub const FAMILY_CLOSE_SCHEMA: &str = "rustred.family-close-output.toml.v4";

/// Request a sector-complete closing artifact for an external family.
///
/// The existing core artifact admission currently requires an unshifted,
/// unit-mass vacuum with `d` as its sole scalar parameter. Every denominator
/// must have constant term `-1`. Coverage includes every sector in the
/// declared root domain. By default that domain is the unrestricted family;
/// `nonpositive_indices` can explicitly keep numerator coordinates inactive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyCloseRequest {
    pub source: String,
    pub input_format: InputFormat,
    pub n_cores: usize,
    /// A permutation of `0..arity` used coherently in every sector.
    pub permutation: Option<Vec<usize>>,
    /// Zero-based coordinates whose powers must remain nonpositive. All
    /// other powers remain unrestricted. Never inferred from target powers.
    pub nonpositive_indices: Vec<usize>,
    /// Resource policy for exact replay and publication, not proof evidence.
    pub publication_limits: SourcePortLimits,
}

impl FamilyCloseRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            input_format: InputFormat::Auto,
            n_cores: 1,
            permutation: None,
            nonpositive_indices: Vec::new(),
            publication_limits: SourcePortLimits::default(),
        }
    }
}

/// An exactly installed artifact, its durable bytes and a timing report.
///
/// The TOML report includes nondeterministic wall times; only the artifact
/// bytes are semantic output. Ordinary application uses the existing
/// `closing_artifact_inspect` and `closing_artifact_reduce` interfaces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyCloseResult {
    artifact: Vec<u8>,
    report_toml: String,
    pub arity: usize,
    pub solved_sectors: usize,
    pub zero_sectors: usize,
    /// All authenticated zero sectors retained for original-source replay,
    /// including those outside the declared root domain.
    pub global_zero_sectors: usize,
    pub generated_rules: usize,
    pub rule_cells: usize,
    pub terminals: usize,
    pub elapsed: Duration,
}

impl FamilyCloseResult {
    pub fn artifact(&self) -> &[u8] {
        &self.artifact
    }

    pub fn into_artifact(self) -> Vec<u8> {
        self.artifact
    }

    pub fn to_toml(&self) -> &str {
        &self.report_toml
    }
}

#[derive(Serialize)]
struct Report<'a> {
    schema: &'static str,
    status: &'static str,
    family_name: &'a str,
    family_fingerprint: String,
    artifact_schema: &'static str,
    algorithm_id: &'a str,
    arity: usize,
    solved_sectors: usize,
    zero_sectors: usize,
    global_zero_sectors: usize,
    root_sector: &'a [bool],
    generated_rules: usize,
    rule_cells: usize,
    terminals: usize,
    bytes: usize,
    workers: usize,
    permutation: Option<&'a [usize]>,
    preparation_us: u128,
    generation_us: u128,
    installation_us: u128,
    encoding_us: u128,
    total_us: u128,
    publication_resources: ResourcePolicyOutput,
}

struct Installed {
    artifact: ClosedArtifact,
    solved_sectors: usize,
    zero_sectors: usize,
    global_zero_sectors: usize,
    generated_rules: usize,
    prepared_at: Duration,
    generated_at: Duration,
    installed_at: Duration,
}

pub fn family_close(request: FamilyCloseRequest) -> Result<FamilyCloseResult, AppError> {
    family_close_impl(request, None)
}

/// Generate the same artifact while observing lightweight progress.
///
/// Generation callbacks run on sector workers and may overlap. Observers
/// must be short and must synchronize their own mutable state. A panic aborts
/// normally; observations cannot authorize rules or change solver policy.
pub fn family_close_with_progress(
    request: FamilyCloseRequest,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<FamilyCloseResult, AppError> {
    family_close_impl(request, Some(&observe))
}

fn family_close_impl(
    request: FamilyCloseRequest,
    observe: Observer<'_>,
) -> Result<FamilyCloseResult, AppError> {
    let start = Instant::now();
    request
        .publication_limits
        .validate()
        .map_err(publication_error)?;
    if request.source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(format!(
            "family close input exceeds the application ceiling {MAX_INPUT_BYTES} bytes"
        )));
    }
    if request.n_cores == 0 {
        return Err(AppError::input("family close n_cores must be positive"));
    }
    let lowered = lower_project(prepare_input(&request.source, request.input_format)?)?;
    let (_, _, _, lowered) = lowered.into_parts();
    let family = lowered.into_family();
    let arity = family.denominator_count();
    if !(1..=16).contains(&arity) {
        return Err(AppError::input(format!(
            "family close supports denominator arities 1 through 16; got {arity}"
        )));
    }
    if let Some(permutation) = &request.permutation {
        let mut coordinates = permutation.clone();
        coordinates.sort_unstable();
        if coordinates != (0..arity).collect::<Vec<_>>() {
            return Err(AppError::input(format!(
                "family close permutation must contain every coordinate in 0..{arity} exactly once"
            )));
        }
    }
    let family_name = family.name().to_owned();
    let root_sector = scope::root_sector(arity, &request.nonpositive_indices)?;
    emit(observe, || FamilyCloseProgress::Preparing {
        arity,
        elapsed: start.elapsed(),
    });
    let installed = dispatch_close(family, &request, &root_sector, start, observe)?;
    emit(observe, || FamilyCloseProgress::Encoding {
        elapsed: start.elapsed(),
    });
    let bytes = installed
        .artifact
        .encode_durable()
        .map_err(|error| AppError::serialization(error.to_string()))?;
    if bytes.len() > MAX_CLOSING_ARTIFACT_BYTES {
        return Err(AppError::output_limit(format!(
            "generated artifact exceeds the application ceiling {MAX_CLOSING_ARTIFACT_BYTES} bytes"
        )));
    }
    let elapsed = start.elapsed();
    let report = Report {
        schema: FAMILY_CLOSE_SCHEMA,
        status: "generated-durable",
        family_name: &family_name,
        family_fingerprint: installed.artifact.family_fingerprint().to_string(),
        artifact_schema: installed.artifact.schema().stable_id(),
        algorithm_id: installed.artifact.algorithm_id(),
        arity,
        solved_sectors: installed.solved_sectors,
        zero_sectors: installed.zero_sectors,
        global_zero_sectors: installed.global_zero_sectors,
        root_sector: &root_sector,
        generated_rules: installed.generated_rules,
        rule_cells: installed.artifact.rule_cells().len(),
        terminals: installed.artifact.masters().len(),
        bytes: bytes.len(),
        workers: request.n_cores,
        permutation: request.permutation.as_deref(),
        preparation_us: installed.prepared_at.as_micros(),
        generation_us: (installed.generated_at - installed.prepared_at).as_micros(),
        installation_us: (installed.installed_at - installed.generated_at).as_micros(),
        encoding_us: (elapsed - installed.installed_at).as_micros(),
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
    let report_toml = toml::to_string_pretty(&report)
        .map_err(|error| AppError::serialization(error.to_string()))?;
    emit(observe, || FamilyCloseProgress::Encoded {
        bytes: bytes.len(),
        elapsed: start.elapsed(),
    });
    Ok(FamilyCloseResult {
        artifact: bytes,
        report_toml,
        arity,
        solved_sectors: report.solved_sectors,
        zero_sectors: report.zero_sectors,
        global_zero_sectors: report.global_zero_sectors,
        generated_rules: report.generated_rules,
        rule_cells: report.rule_cells,
        terminals: report.terminals,
        elapsed,
    })
}

fn dispatch_close(
    family: IntegralFamily,
    request: &FamilyCloseRequest,
    root_sector: &[bool],
    start: Instant,
    observe: Observer<'_>,
) -> Result<Installed, AppError> {
    macro_rules! arms {
        ($($n:literal),+ $(,)?) => {
            match family.denominator_count() {
                $($n => close::<$n>(family, request, root_sector, start, observe),)+
                _ => unreachable!("family-close arity checked before dispatch"),
            }
        };
    }
    arms!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

fn close<const N: usize>(
    family: IntegralFamily,
    request: &FamilyCloseRequest,
    root_sector: &[bool],
    start: Instant,
    observe: Observer<'_>,
) -> Result<Installed, AppError> {
    SourcePortAudit::<N>::validate_install_family(&family).map_err(|error| {
        AppError::input(format!(
            "family close requires an unshifted unit-mass vacuum, denominator constant terms -1, \
             and d as its sole scalar parameter: {error}"
        ))
    })?;
    let preparation::Prepared {
        family,
        root: root_sector,
        permutation,
        zeros,
        sectors,
        sources,
    } = preparation::prepare::<N>(family, root_sector, request.permutation.as_deref())?;
    let admits = |sector: &[bool; N]| {
        sector
            .iter()
            .zip(root_sector)
            .all(|(&active, allowed)| !active || allowed)
    };
    let scoped_zero_sectors = zeros.iter().filter(|sector| admits(sector)).count();
    let audit = SourcePortAudit::try_new_with_root_sector(&family, zeros.clone(), root_sector)
        .map_err(|error| AppError::execution(error.to_string()))?
        .with_limits(request.publication_limits);
    let executor = SectorExecutor::new(request.n_cores)
        .map_err(|error| AppError::execution(error.to_string()))?;
    let prepared_at = start.elapsed();
    emit(observe, || FamilyCloseProgress::Prepared {
        sectors: sectors.len(),
        zero_sectors: scoped_zero_sectors,
        global_zero_sectors: zeros.len(),
        elapsed: prepared_at,
    });
    let solved = executor
        .map_with_observer(
            &sources,
            &sectors,
            &SectorConfig {
                zero_sectors: zeros.clone(),
                permutation,
                ..Default::default()
            },
            SectorSolveOptions::default(),
            |ordinal, sector, event| {
                emit(observe, || FamilyCloseProgress::Generating {
                    ordinal,
                    sector: sector_mask(sector),
                    stage: generation_stage(event),
                    elapsed: start.elapsed(),
                })
            },
            |done| {
                emit(observe, || FamilyCloseProgress::GeneratedSector {
                    ordinal: done.ordinal,
                    sector: sector_mask(done.sector),
                    rules: done.solution.rules.len(),
                    finite_residuals: done.solution.finite_residuals.len(),
                    elapsed: start.elapsed(),
                });
                Ok::<_, std::io::Error>((done.sector, permutation, done.solution))
            },
        )
        .map_err(|error| AppError::execution(error.to_string()))?;
    let generated_at = start.elapsed();
    let generated_rules = solved
        .iter()
        .map(|(_, _, solution)| solution.rules.len())
        .sum();
    let artifact = audit
        .install_complete_with_observer(family, solved, |event| {
            emit(observe, || installation_event(event, start.elapsed()));
        })
        .map_err(publication_error)?;
    Ok(Installed {
        artifact,
        solved_sectors: sectors.len(),
        zero_sectors: scoped_zero_sectors,
        global_zero_sectors: zeros.len(),
        generated_rules,
        prepared_at,
        generated_at,
        installed_at: start.elapsed(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClosingArtifactReduceRequest, closing_artifact_reduce};

    const K1: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "arbitrary_user_label"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
numerator = "1"
"#;

    const K3: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "another_user_label"
loop_momenta = ["p", "q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "A"
expression = "p^2-1"
[[family.denominators]]
id = "B"
expression = "q^2-1"
[[family.denominators]]
id = "C"
expression = "(p-q)^2-1"
[target]
powers = [1,1,1]
numerator = "1"
"#;

    #[test]
    fn supplied_k1_closes_and_cold_loaded_artifact_reduces() {
        let result = family_close(FamilyCloseRequest::new(K1)).unwrap();
        assert_eq!(result.arity, 1);
        assert_eq!(result.solved_sectors + result.zero_sectors, 2);
        assert_eq!(result.terminals, 1);
        assert!(result.to_toml().contains("generated-durable"));
        let reduction = closing_artifact_reduce(ClosingArtifactReduceRequest::new(
            result.into_artifact(),
            vec![3],
        ))
        .unwrap();
        assert_eq!(reduction.terms().len(), 1);
        assert_eq!(reduction.terms()[0].master_powers(), &[1]);
    }

    #[test]
    fn publication_policy_is_reported_without_changing_artifact_authority() {
        let baseline = family_close(FamilyCloseRequest::new(K1)).unwrap();
        let mut request = FamilyCloseRequest::new(K1);
        assert_eq!(request.publication_limits, SourcePortLimits::default());
        request
            .publication_limits
            .rule_derivation
            .max_domain_bound_endpoint_cells = usize::MAX;
        request.publication_limits.max_predicate_consistency_work = usize::MAX;
        let chosen = family_close(request).unwrap();
        assert_eq!(chosen.artifact(), baseline.artifact());
        let report: toml::Value = toml::from_str(chosen.to_toml()).unwrap();
        assert_eq!(report["schema"].as_str(), Some(FAMILY_CLOSE_SCHEMA));
        for name in [
            "max_domain_bound_endpoint_cells",
            "max_predicate_consistency_work",
        ] {
            assert_eq!(
                report["publication_resources"][name].as_str(),
                Some(usize::MAX.to_string().as_str())
            );
        }
        let defaults: toml::Value = toml::from_str(baseline.to_toml()).unwrap();
        assert_eq!(
            defaults["publication_resources"]["max_predicate_consistency_work"].as_str(),
            Some(
                SourcePortLimits::default()
                    .max_predicate_consistency_work
                    .to_string()
                    .as_str()
            )
        );
        let error = publication_error(SourcePortAuditError::ResourceBudgetExhausted {
            resource: "native affine literal consistency",
        });
        assert_eq!(error.kind(), crate::AppErrorKind::Limit);
        assert!(
            error
                .message()
                .contains("native affine literal consistency")
        );
        assert!(!error.message().contains("requested"));
        let mut restricted = FamilyCloseRequest::new(K1);
        restricted
            .publication_limits
            .rule_derivation
            .max_domain_bound_endpoint_cells = 0;
        assert!(family_close(restricted).is_err());
    }

    #[test]
    fn supplied_k3_closes_full_census_and_reduces_dotted_target() {
        let result = family_close(FamilyCloseRequest::new(K3)).unwrap();
        assert_eq!(result.arity, 3);
        assert_eq!(result.solved_sectors + result.zero_sectors, 8);
        let mut parallel = FamilyCloseRequest::new(K3);
        parallel.n_cores = 2;
        let generated = std::sync::atomic::AtomicUsize::new(0);
        let parallel = family_close_with_progress(parallel, |event| {
            if matches!(event, FamilyCloseProgress::GeneratedSector { .. }) {
                generated.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .unwrap();
        assert_eq!(
            generated.load(std::sync::atomic::Ordering::Relaxed),
            parallel.solved_sectors
        );
        assert_eq!(result.artifact(), parallel.artifact());
        let reduction = closing_artifact_reduce(ClosingArtifactReduceRequest::new(
            result.into_artifact(),
            vec![2, 2, 1],
        ))
        .unwrap();
        assert!(!reduction.terms().is_empty());
    }

    #[test]
    fn explicit_scope_is_cold_visible_and_preserves_in_scope_reductions() {
        let full = family_close(FamilyCloseRequest::new(K3)).unwrap();
        let mut request = FamilyCloseRequest::new(K3);
        request.nonpositive_indices = vec![2];
        let scoped = family_close(request.clone()).unwrap();
        assert_eq!(scoped.solved_sectors, 1);
        assert_eq!(scoped.zero_sectors, 3);
        assert_eq!(scoped.global_zero_sectors, 4);
        let report: toml::Value = toml::from_str(scoped.to_toml()).unwrap();
        assert_eq!(
            report["root_sector"]
                .as_array()
                .unwrap()
                .iter()
                .map(|bit| bit.as_bool().unwrap())
                .collect::<Vec<_>>(),
            [true, true, false]
        );
        request.n_cores = 2;
        assert_eq!(scoped.artifact(), family_close(request).unwrap().artifact());

        let inspected = crate::closing_artifact_inspect(crate::ClosingArtifactInspectRequest {
            load_limits: Default::default(),
            artifact: scoped.artifact().to_vec(),
        })
        .unwrap();
        let inspected: toml::Value = toml::from_str(inspected.to_toml()).unwrap();
        assert_eq!(
            inspected["artifact"]["root_power_upper"][2].as_integer(),
            Some(0)
        );
        assert_eq!(
            inspected["artifact"]["in_scope_zero_sectors"].as_integer(),
            Some(3)
        );
        assert_eq!(
            inspected["artifact"]["zero_terminals"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        for target in [vec![2, 2, 0], vec![2, 2, -1], vec![2, 2, -2]] {
            let full_result = closing_artifact_reduce(ClosingArtifactReduceRequest::new(
                full.artifact().to_vec(),
                target.clone(),
            ))
            .unwrap();
            let scoped_result = closing_artifact_reduce(ClosingArtifactReduceRequest::new(
                scoped.artifact().to_vec(),
                target,
            ))
            .unwrap();
            assert_eq!(full_result.terms(), scoped_result.terms());
        }
        let failure = closing_artifact_reduce(ClosingArtifactReduceRequest::new(
            scoped.into_artifact(),
            vec![1, 1, 1],
        ))
        .unwrap_err();
        assert_eq!(failure.kind(), crate::AppErrorKind::Input);
    }

    #[test]
    fn owned_progress_preserves_bytes_and_distinguishes_install_from_encoding() {
        let baseline = family_close(FamilyCloseRequest::new(K1)).unwrap();
        let events = std::sync::Mutex::new(Vec::new());
        let observed = family_close_with_progress(FamilyCloseRequest::new(K1), |event| {
            events.lock().unwrap().push(event);
        })
        .unwrap();
        assert_eq!(baseline.artifact(), observed.artifact());
        let events = events.into_inner().unwrap();
        assert!(matches!(
            events.first(),
            Some(FamilyCloseProgress::Preparing { arity: 1, .. })
        ));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, FamilyCloseProgress::Generating { .. }))
        );
        let checked = events
            .iter()
            .position(|event| matches!(event, FamilyCloseProgress::CheckedSector { .. }))
            .unwrap();
        let lowered = events
            .iter()
            .position(|event| matches!(event, FamilyCloseProgress::LoweredSector { .. }))
            .unwrap();
        let installed = events
            .iter()
            .position(|event| matches!(event, FamilyCloseProgress::Installed { .. }))
            .unwrap();
        let encoding = events
            .iter()
            .position(|event| matches!(event, FamilyCloseProgress::Encoding { .. }))
            .unwrap();
        assert!(checked < lowered && lowered < installed && installed < encoding);
        assert!(
            matches!(events.last(), Some(FamilyCloseProgress::Encoded { bytes, .. }) if *bytes == observed.artifact().len())
        );
    }

    #[test]
    fn rejected_family_never_announces_success() {
        let events = std::sync::Mutex::new(Vec::new());
        assert!(
            family_close_with_progress(
                FamilyCloseRequest::new(K1.replace("q^2-1", "q^2-2")),
                |event| {
                    events.lock().unwrap().push(event);
                }
            )
            .is_err()
        );
        assert!(events.into_inner().unwrap().iter().all(|event| !matches!(
            event,
            FamilyCloseProgress::Installed { .. } | FamilyCloseProgress::Encoded { .. }
        )));
    }

    #[test]
    fn permutation_validation_is_topology_independent() {
        let mut request = FamilyCloseRequest::new(K3);
        request.permutation = Some(vec![0, 0, 2]);
        let error = family_close(request).unwrap_err();
        assert_eq!(error.kind(), super::super::AppErrorKind::Input);
        assert!(error.to_string().contains("exactly once"));
    }

    #[test]
    fn nonunit_mass_cannot_be_published() {
        for denominator in ["q^2-2", "q^2-m"] {
            let error = family_close(FamilyCloseRequest::new(K1.replace("q^2-1", denominator)))
                .unwrap_err();
            assert_eq!(error.kind(), super::super::AppErrorKind::Input);
            assert!(error.to_string().contains("unit-mass vacuum"));
        }
    }
}
