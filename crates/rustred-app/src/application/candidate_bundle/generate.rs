use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::persistence::{CoefficientTableBuilder, NativeFamilyRecord};
use rustred::solver::{SectorConfig, SectorExecutionError, SectorExecutor, SectorSolveOptions};
use serde::Serialize;

use crate::application::AppError;
use crate::application::family_close::progress::{
    FamilyCloseProgress, Observer, emit, generation_failure, generation_stage, sector_mask,
};

use super::checkpoint::{CheckpointManifest, CheckpointStore};
use super::{codec, model::*, policy, preparation, save};

mod checkpoint;
mod portfolio;

pub fn family_candidates(
    request: FamilyCandidatesRequest,
) -> Result<CandidateBundleResult, AppError> {
    generate_request(request, None, None, None)
}

/// Observe generation without enabling source replay or closure certification.
/// Callbacks may overlap on sector workers and must synchronize their state.
/// Only preparation, generation and encoding events are emitted.
pub fn family_candidates_with_progress(
    request: FamilyCandidatesRequest,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<CandidateBundleResult, AppError> {
    generate_request(request, Some(&observe), None, None)
}

/// Controlled in-process generation. Cancellation is checked at sector and
/// assembly boundaries, never by interrupting an in-flight algebra operation.
pub fn family_candidates_controlled(
    request: FamilyCandidatesRequest,
    cancellation: Arc<AtomicBool>,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<CandidateBundleResult, AppError> {
    generate_request(request, Some(&observe), Some(cancellation), None)
}

/// Generate from an already prepared native family, without a text roundtrip.
/// The family supplies all denominator/parameter identities. `options.source`
/// and input format are not parsed; saved source text is a provenance label.
pub fn family_candidates_from_family_controlled(
    family: Arc<IntegralFamily>,
    mut options: FamilyCandidatesRequest,
    cancellation: Arc<AtomicBool>,
    observe: impl Fn(FamilyCloseProgress) + Send + Sync,
) -> Result<CandidateBundleResult, AppError> {
    options.source = format!("native-family:{}", family.fingerprint());
    generate_request(options, Some(&observe), Some(cancellation), Some(family))
}

fn check_cancel(token: Option<&Arc<AtomicBool>>) -> Result<(), AppError> {
    if token.is_some_and(|token| token.load(Ordering::Acquire)) {
        Err(AppError::new(
            crate::AppErrorKind::Cancelled,
            "candidate generation cancelled",
        ))
    } else {
        Ok(())
    }
}

fn generate_request(
    request: FamilyCandidatesRequest,
    observe: Observer<'_>,
    cancellation: Option<Arc<AtomicBool>>,
    native_family: Option<Arc<IntegralFamily>>,
) -> Result<CandidateBundleResult, AppError> {
    let started = Instant::now();
    check_cancel(cancellation.as_ref())?;
    request.exact_backend.solver_backend()?;
    if request.n_cores == 0 {
        return Err(AppError::input(
            "candidate generation n_cores must be positive",
        ));
    }
    if cfg!(target_arch = "wasm32") && request.n_cores != 1 {
        return Err(AppError::input(
            "WebAssembly candidate generation requires n_cores = 1",
        ));
    }
    policy::validate_request_scope(&request)?;
    let family = match native_family {
        Some(family) => family,
        None => Arc::new(preparation::family(&request.source, request.input_format)?),
    };
    let n = family.denominator_count();
    if !(1..=16).contains(&n) {
        return Err(AppError::input("candidate family arity must be 1..=16"));
    }
    let root = preparation::root(n, &request.nonpositive_indices)?;
    preparation::validate_permutation(n, request.permutation.as_deref())?;
    super::order::request_policy(&request, n)?;
    emit(observe, || FamilyCloseProgress::Preparing {
        arity: n,
        elapsed: started.elapsed(),
    });
    macro_rules! dispatch {
        ($($n:literal),*) => { match rustred::campaign_storage_arity(n ){
            $($n => generate::<$n>(family, request, &root, started, observe, cancellation),)*
            _ => Err(crate::AppError::input("campaign arity is not compiled")),
        } };
    }
    {
        crate::ensure_runtime_arity(n)?;
        rustred::with_app_runtime_arities!(dispatch)
    }
}

fn generate<const N: usize>(
    family: Arc<IntegralFamily>,
    request: FamilyCandidatesRequest,
    root: &[bool],
    started: Instant,
    observe: Observer<'_>,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<CandidateBundleResult, AppError> {
    check_cancel(cancellation.as_ref())?;
    let mut prepared =
        preparation::prepare_shared::<N>(family, root, request.permutation.as_deref())?;
    check_cancel(cancellation.as_ref())?;
    let selected_sectors = super::selection::apply(
        &mut prepared,
        request.selected_sectors.as_deref(),
        request.bundle_limits.max_collection_entries,
    )?;
    if let Some(strategy) = &request.discovery_strategy {
        strategy.validate(
            prepared.family.denominator_count(),
            &prepared
                .sectors
                .iter()
                .map(|s| s[..prepared.family.denominator_count()].to_vec())
                .collect::<Vec<_>>(),
            request.bundle_limits.max_collection_entries,
        )?;
    }
    // Preserve the existing default timing boundary (pool construction belongs
    // to preparation). Checkpoint-only resume must never construct a pool.
    let default_executor = request
        .checkpoint
        .is_none()
        .then(|| SectorExecutor::new(request.n_cores))
        .transpose()
        .map_err(|e| AppError::execution(e.to_string()))?;
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
    let store = request
        .checkpoint
        .as_ref()
        .map(|options| {
            let manifest = CheckpointManifest::for_request(
                &request,
                prepared.family.fingerprint(),
                root,
                prepared
                    .sectors
                    .iter()
                    .map(|s| s[..prepared.family.denominator_count()].to_vec())
                    .collect(),
            )?;
            CheckpointStore::open(options, manifest, request.bundle_limits)
        })
        .transpose()?;
    let pending: Vec<_> = if let Some(store) = &store {
        store
            .pending()?
            .into_iter()
            .map(|(ordinal, sector)| {
                rustred::storage_array(&sector, false)
                    .ok_or(())
                    .map(|sector| (ordinal, sector))
                    .map_err(|_| AppError::internal_invariant("checkpoint pending sector arity"))
            })
            .collect::<Result<_, _>>()?
    } else {
        prepared.sectors.iter().copied().enumerate().collect()
    };
    let reused_sectors = prepared.sectors.len() - pending.len();
    let jobs: Vec<_> = pending.iter().map(|(_, sector)| *sector).collect();
    let admitted_at = if store.is_some() {
        started.elapsed()
    } else {
        prepared_at
    };
    if store.is_some() {
        emit(observe, || FamilyCloseProgress::CheckpointPrepared {
            reused_sectors,
            pending_sectors: jobs.len(),
            elapsed: admitted_at,
        });
    }
    // A fully saved campaign only assembles; it must not initialize workers or
    // execute any sector search. Completion callbacks retain small receipts on
    // disk, not all expanded exact solutions in the executor's result vector.
    let solved = if jobs.is_empty() {
        Vec::new()
    } else {
        let mut executor = match default_executor {
            Some(executor) => executor,
            None => SectorExecutor::new(request.n_cores)
                .map_err(|e| AppError::execution(e.to_string()))?,
        };
        if let Some(token) = &cancellation {
            executor = executor.with_cancellation(token.clone());
        }
        if let Some(plan) = request
            .discovery_strategy
            .as_ref()
            .and_then(|strategy| strategy.pending_order(&pending))
        {
            executor = executor.with_sector_visit_order(plan);
        }
        let source_plans = request
            .discovery_strategy
            .as_ref()
            .map(|strategy| {
                jobs.iter()
                    .map(|s| strategy.source_for(&s[..prepared.family.denominator_count()]))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let rule_selection_plans = request
            .discovery_strategy
            .as_ref()
            .filter(|strategy| strategy.rule_selection.is_some())
            .map(|strategy| {
                jobs.iter()
                    .map(|sector| strategy.rule_selection_for(sector))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let symbolic_exact_backend = request.exact_backend.solver_backend()?;
        executor
            .map_configured_with_error_observer(
                &prepared.sources,
                &jobs,
                |ordinal, _| SectorConfig {
                    zero_sectors: prepared.zeros.clone(),
                    permutation: prepared.permutation,
                    integral_order: request.integral_order.clone(),
                    symbolic_exact_backend,
                    numerical_exact_backend: request.exact_backend.numerical_backend(),
                    source_discovery: source_plans
                        .as_ref()
                        .map(|plans| plans[ordinal].clone())
                        .unwrap_or_default(),
                    rule_selection: rule_selection_plans
                        .as_ref()
                        .map(|plans| plans[ordinal].clone())
                        .unwrap_or_default(),
                    ..Default::default()
                },
                SectorSolveOptions {
                    numerical_depth: request.numerical_depth,
                    max_numerator_rank: request.max_numerator_rank,
                    finite_case_policy: request.finite_case_policy,
                    finite_case_limits: request.finite_case_limits,
                    case_intersection_limits: request.case_intersection_limits,
                    ..Default::default()
                },
                |ordinal, sector, event| {
                    emit(observe, || FamilyCloseProgress::Generating {
                        ordinal: pending[ordinal].0,
                        sector: sector_mask(sector),
                        stage: generation_stage(event),
                        elapsed: started.elapsed(),
                    })
                },
                |error| {
                    if matches!(error, SectorExecutionError::Cancelled { .. }) {
                        return;
                    }
                    emit(observe, || {
                        generation_failure(error, pending[error.ordinal()].0, started.elapsed())
                    })
                },
                |done| {
                    // Keep the default result receipt small; only opt-in
                    // portfolios allocate a per-sector diagnostic summary.
                    let selection_stats = done.solution.stats.rule_selection.map(Box::new);
                    if done.solution.max_numerator_rank != request.max_numerator_rank {
                        return Err(AppError::internal_invariant(
                            "generated sector numerator-rank scope differs from its request",
                        ));
                    }
                    if done.solution.finite_case_policy != request.finite_case_policy {
                        return Err(AppError::internal_invariant(
                            "generated sector finite-case policy differs from its request",
                        ));
                    }
                    emit(observe, || FamilyCloseProgress::GeneratedSector {
                        ordinal: pending[done.ordinal].0,
                        sector: sector_mask(done.sector),
                        rules: done.solution.rules.len(),
                        finite_residuals: done.solution.finite_residuals.len(),
                        elapsed: started.elapsed(),
                    });
                    if let Some(store) = &store {
                        let bytes = save::encode_sector(
                            &request,
                            &prepared.family,
                            root,
                            done.sector,
                            &done.solution,
                        )?;
                        let receipt = store.publish(pending[done.ordinal].0, &bytes)?;
                        emit(observe, || FamilyCloseProgress::CheckpointedSector {
                            ordinal: receipt.ordinal,
                            sector: sector_mask(done.sector),
                            bytes: receipt.bytes,
                            elapsed: started.elapsed(),
                        });
                        Ok::<_, AppError>((None, selection_stats))
                    } else {
                        Ok((Some((done.sector, done.solution)), selection_stats))
                    }
                },
            )
            .map_err(|e| execution_error(e, &pending))?
    };
    check_cancel(cancellation.as_ref())?;
    let mut rule_selection = request
        .discovery_strategy
        .as_ref()
        .and_then(|strategy| strategy.rule_selection.as_ref())
        .map(|_| portfolio::Report::new());
    if let Some(report) = &mut rule_selection {
        for (_, stats) in &solved {
            report.newly_solved_sectors += 1;
            if let Some(stats) = stats {
                report.add(**stats);
            }
        }
    }
    let solved_at = started.elapsed();
    emit(observe, || FamilyCloseProgress::Encoding {
        elapsed: solved_at,
    });
    let mut coefficients = CoefficientTableBuilder::new(request.bundle_limits.binary_limits());
    let sectors = if let Some(store) = &store {
        checkpoint::assemble(store, &prepared, request.bundle_limits, &mut coefficients)?
    } else {
        solved
            // Release each expanded exact solution once its compact record and
            // native coefficients are interned. Encounter order is unchanged;
            // keeping all solutions alive until report writing doubles up
            // their storage with the completed native output unnecessarily.
            .into_iter()
            .map(|(solution, _)| solution)
            .flatten()
            .map(|(sector, solution)| {
                codec::physical_sector_record(
                    prepared.family.denominator_count(),
                    sector,
                    &solution,
                    &mut coefficients,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let assembled_at = started.elapsed();
    let solved_sectors = sectors.len();
    let generated_rules = sectors.iter().map(|s| s.rules.len()).sum();
    let finite_residuals = sectors.iter().map(|s| s.finite_residuals.len()).sum();
    let bundle = save::program_record(&request, &prepared.family, root, sectors)?;
    let family_record = NativeFamilyRecord::from_family(&prepared.family, &mut coefficients)
        .map_err(codec::binary_error)?;
    let coefficient_count = coefficients.len();
    let coefficients = coefficients.finish().map_err(codec::binary_error)?;
    let bytes = codec::write_records(
        &bundle,
        &family_record,
        &coefficients,
        request.bundle_limits,
    )?;
    let elapsed = started.elapsed();
    check_cancel(cancellation.as_ref())?;
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
        arity: usize,
        root_sector: &'a [bool],
        generation_scope: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        selected_sectors: Option<&'a [Vec<bool>]>,
        solved_sectors: usize,
        zero_sectors: usize,
        generated_rules: usize,
        finite_residuals: usize,
        workers: usize,
        exact_backend: &'static str,
        integral_order: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        discovery_strategy: Option<&'a super::CandidateDiscoveryStrategy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rule_selection: Option<portfolio::Report>,
        numerical_depth: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_numerator_rank: Option<u32>,
        finite_case_policy: &'static str,
        finite_max_visited_points: usize,
        finite_max_retained_terminals: usize,
        // Invocation resources, not historical allowances for reused shards.
        case_max_work_items: usize,
        case_max_terms_per_conjunction: usize,
        case_max_normalizations: usize,
        case_max_factorizations: usize,
        bytes: usize,
        unique_coefficients: usize,
        coefficient_table_bytes: usize,
        symbolica_state_bytes: usize,
        preparation_us: u128,
        solve_us: u128,
        bundle_encoding_us: u128,
        total_us: u128,
        #[serde(skip_serializing_if = "Option::is_none")]
        checkpoint: Option<checkpoint::Report>,
    }
    let report = Report {
        schema: FAMILY_CANDIDATES_SCHEMA,
        status: STATUS,
        workload: "prepare-solve-save; no source replay or closure certification",
        family_fingerprint: prepared.family.fingerprint(),
        arity: prepared.family.denominator_count(),
        root_sector: root,
        generation_scope: if selected_sectors.is_some() {
            "selected-sectors"
        } else {
            "root-downset"
        },
        selected_sectors: selected_sectors.as_deref(),
        solved_sectors,
        zero_sectors: prepared.zeros.len(),
        generated_rules,
        finite_residuals,
        workers: request.n_cores,
        exact_backend: request.exact_backend.as_str(),
        integral_order: super::order::request_policy(
            &request,
            prepared.family.denominator_count(),
        )?
        .stable_id()
        .to_string(),
        discovery_strategy: request.discovery_strategy.as_ref(),
        rule_selection,
        numerical_depth: request.numerical_depth,
        max_numerator_rank: request.max_numerator_rank,
        finite_case_policy: request.finite_case_policy.as_str(),
        finite_max_visited_points: request.finite_case_limits.max_visited_points,
        finite_max_retained_terminals: request.finite_case_limits.max_retained_terminals,
        case_max_work_items: request.case_intersection_limits.max_work_items,
        case_max_terms_per_conjunction: request.case_intersection_limits.max_terms_per_conjunction,
        case_max_normalizations: request.case_intersection_limits.max_normalizations,
        case_max_factorizations: request.case_intersection_limits.max_factorizations,
        bytes: bytes.len(),
        unique_coefficients: coefficient_count,
        coefficient_table_bytes: coefficients.atoms.len(),
        symbolica_state_bytes: coefficients.state.len(),
        preparation_us: prepared_at.as_micros(),
        solve_us: (solved_at - admitted_at).as_micros(),
        bundle_encoding_us: (elapsed - solved_at).as_micros(),
        total_us: elapsed.as_micros(),
        checkpoint: store
            .as_ref()
            .map(|store| {
                Ok::<_, AppError>(checkpoint::Report {
                    reused_sectors,
                    newly_solved_sectors: pending.len(),
                    disk_bytes: store.charged_bytes()?,
                    resume_validation_us: (admitted_at - prepared_at).as_micros(),
                    assembly_us: (assembled_at - solved_at).as_micros(),
                })
            })
            .transpose()?,
    };
    Ok(CandidateBundleResult {
        bundle: bytes,
        bundle_limits: request.bundle_limits,
        report_toml: toml::to_string_pretty(&report)
            .map_err(|e| AppError::serialization(e.to_string()))?,
    })
}

fn execution_error<const N: usize>(
    error: SectorExecutionError<N, AppError>,
    pending: &[(usize, [bool; N])],
) -> AppError {
    let ordinal = pending[error.ordinal()].0;
    let kind = match &error {
        SectorExecutionError::Cancelled { .. } => crate::AppErrorKind::Cancelled,
        SectorExecutionError::Consume { source, .. } => source.kind(),
        _ => crate::application::AppErrorKind::Execution,
    };
    let error = match error {
        SectorExecutionError::Cancelled { sector, .. } => {
            SectorExecutionError::Cancelled { ordinal, sector }
        }
        SectorExecutionError::Prepare { sector, source, .. } => SectorExecutionError::Prepare {
            ordinal,
            sector,
            source,
        },
        SectorExecutionError::Solve { sector, source, .. } => SectorExecutionError::Solve {
            ordinal,
            sector,
            source,
        },
        SectorExecutionError::Consume { sector, source, .. } => SectorExecutionError::Consume {
            ordinal,
            sector,
            source,
        },
    };
    AppError::new(kind, error.to_string())
}
