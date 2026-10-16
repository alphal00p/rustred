//! Public transport/lifecycle for a separate exact finite-demand namespace.
//! The backend Session remains the sole enumerator, scheduler and commit path.
mod control;
mod control_io;
pub(crate) mod policy;
pub use policy::FiniteDemandPolicy;

use super::{
    backend::{self, Session},
    storage::{ScopeRole, ValidatedGeneration},
    windows,
};
use crate::application::routed_campaign::input::Selection;
use crate::{AppError, AppErrorKind, MasterReductionOptions, OwnerDomainWalkRequest};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FiniteDemandMode {
    Create,
    Resume,
    /// Explicitly repeat interrupted preparation only when no native store or
    /// execution marker exists; no missing-CURRENT store is reset or adopted.
    RepairPreparation,
}

#[derive(Clone, Debug)]
pub struct FiniteDemandRunRequest {
    /// Input/provenance carrier only: its CP6/legacy output paths are not run.
    pub source: OwnerDomainWalkRequest,
    pub policy: FiniteDemandPolicy,
    /// Fresh engine namespace; native Work is stored in its `store` child.
    pub directory: PathBuf,
    pub mode: FiniteDemandMode,
}
#[derive(Clone, Debug)]
pub struct FiniteDemandRunResult {
    pub complete: bool,
    pub document: Value,
}
fn check(cancel: &AtomicBool) -> Result<(), AppError> {
    if cancel.load(Ordering::Acquire) {
        Err(AppError::new(
            AppErrorKind::Cancelled,
            "finite lifecycle cancelled",
        ))
    } else {
        Ok(())
    }
}

/// Thin native loop over the existing exact-window backend. An error or stop
/// never adopts the unfinished window; prior immutable commits remain valid.
/// No historical CP6 import or coefficient back-substitution is performed.
pub fn run_finite_demand_with_progress(
    request: FiniteDemandRunRequest,
    cancel: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<FiniteDemandRunResult, AppError> {
    check(cancel)?;
    let (_, arity, _) = Selection::parse_with_byte_limit(
        &request.source.matching.selection_json,
        request.policy.max_source_bytes,
    )?;
    crate::ensure_runtime_arity(arity)?;
    let mut controls = control::Control::prepare(
        &request.directory,
        request.mode,
        &request.source,
        &request.policy,
        cancel,
    )?;
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity) {
        $($n => run::<$n>(&request, &mut controls, cancel, &observer),)*
        _ => Err(AppError::input("finite campaign arity is not compiled")),
    } } }
    rustred::with_app_runtime_arities!(dispatch)
}
fn session<const N: usize>(
    request: &FiniteDemandRunRequest,
    controls: &mut control::Control,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Session<N>, AppError> {
    let store = controls.store();
    let result = match request.mode {
        FiniteDemandMode::Create | FiniteDemandMode::RepairPreparation => Session::create(
            &request.source,
            request.policy.native(),
            &store,
            cancel,
            observer,
        ),
        FiniteDemandMode::Resume => Session::resume(
            &request.source,
            request.policy.native(),
            &store,
            cancel,
            observer,
        ),
    }?;
    controls.adopt_source(&result.generation()?)?;
    check(cancel)?;
    Ok(result)
}
fn run<const N: usize>(
    request: &FiniteDemandRunRequest,
    controls: &mut control::Control,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<FiniteDemandRunResult, AppError> {
    let forward = |mut event: Value| {
        event["operation"] = json!("finite_demand");
        if let Some(native) = event.get("native").cloned() {
            event["snapshot"] = native;
        }
        observer(event);
    };
    let mut session = session::<N>(request, controls, cancel, &forward)?;
    // PrefixWindows already cached each request's exact count during admission.
    // Sum it once; overlapping requests remain separate input occurrences.
    let input_occurrences_total = session.total_input_occurrences().to_string();
    let first = session.generation()?;
    observer(
        json!({"event":if request.mode != FiniteDemandMode::Resume {"finite_checkpoint_saved"} else {"finite_checkpoint_restored"},
        "operation":"finite_demand","checkpoint":controls.store(),"generation":control::hex(first.digest()),
        "generation_number":first.generation(),"committed":committed(&first)?,
        "input_occurrences_total":input_occurrences_total}),
    );
    loop {
        check(cancel)?;
        let step = session.step(cancel, &forward)?;
        let Some(window) = step.window else {
            break;
        };
        let generation = session.generation()?;
        observer(
            json!({"event":"finite_checkpoint_saved","operation":"finite_demand",
            "checkpoint":controls.store(),"generation":control::hex(generation.digest()),
            "generation_number":generation.generation(),"committed":committed(&generation)?,
            "window":{"row":window.row,"inputs":window.inputs,"new_work":window.new_work,
                "baseline_hits":window.baseline_hits,"cleanup_warning":window.cleanup_warning}}),
        );
    }
    check(cancel)?;
    let generation = session.generation()?;
    let scope = committed(&generation)?;
    let complete = scope["all_rows_exhausted"] == true && scope["required_complete"] == true;
    if !complete {
        return Err(AppError::internal_invariant(
            "finite loop ended with pending scope",
        ));
    }
    Ok(FiniteDemandRunResult {
        complete,
        document: json!({
        "schema":"rustred.finite-demand-run.v1","engine":control::ENGINE,"status":"complete","complete":true,
        "generation":{"number":generation.generation(),"digest":control::hex(generation.digest())},
        "checkpoint":controls.store(),"committed":scope,"policy":request.policy,
        "input_occurrences_total":input_occurrences_total,
        "cancelled":false,"error_kind":null,"error":null,
        "raw_census_streamed":false,"publication_claim":false,"family_closure_claim":false}),
    })
}

/// Bounded native metadata inspection, not independent replay, source loading,
/// a prefix cursor constructor or a publication certificate.
pub fn inspect_finite_demand(directory: &Path) -> Result<Value, AppError> {
    inspect_finite_demand_with_policy(directory, None)
}

/// Inspect with caller-selected resource grants. Without an override, metadata
/// counts are uncapped, directories/control files default to 256 MiB and native
/// frames remain bounded by the format. Saved policies never elevate grants.
pub fn inspect_finite_demand_with_policy(
    directory: &Path,
    policy: Option<&FiniteDemandPolicy>,
) -> Result<Value, AppError> {
    let snapshot = control::inspect(directory, policy)?;
    let Some(generation) = &snapshot.generation else {
        return Ok(
            json!({"schema":"rustred.finite-demand-status.v1","engine":control::ENGINE,
            "status":"preparing","generation":null,"committed":null,"source":{"status":"unadmitted"},
            "requested":snapshot.requested,"repair_preparation_possible":true,
            "authority":"no native generation exists; explicit repair recomputes preparation without Work reuse",
            "publication_claim":false,"family_closure_claim":false}),
        );
    };
    let mut source = snapshot
        .source
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| AppError::serialization(e.to_string()))?
        .unwrap_or_else(|| json!({}));
    source["status"] = json!(snapshot.source_status);
    Ok(
        json!({"schema":"rustred.finite-demand-status.v1","engine":control::ENGINE,
        "generation":{"number":generation.generation(),"digest":control::hex(generation.digest())},
        "committed":committed(generation)?,"source":source,"requested":snapshot.requested,"repair_preparation_possible":false,
        "authority":"trusted-generated committed metadata; no full raw-census or independent closure verification",
        "publication_claim":false,"family_closure_claim":false}),
    )
}
fn committed(generation: &ValidatedGeneration) -> Result<Value, AppError> {
    let state = generation.state();
    let (mut required, mut required_done, mut auxiliary, mut auxiliary_done) =
        (0usize, 0usize, 0usize, 0usize);
    let mut current = None;
    let mut current_window = None;
    let mut occurrences = 0u64;
    let mut committed_windows = 0u64;
    for (index, row) in state.scope.rows.iter().enumerate() {
        let token = generation.cursor(index).map_err(backend::storage_error)?;
        let cursor = windows::inspect_committed_cursor(
            &token,
            generation.execution().physical_arity as usize,
        )
        .map_err(backend::window_error)?;
        occurrences = occurrences
            .checked_add(cursor.input_occurrences)
            .ok_or_else(|| AppError::limit("committed input count overflow"))?;
        committed_windows = committed_windows
            .checked_add(cursor.completed_windows)
            .ok_or_else(|| AppError::limit("committed window count overflow"))?;
        match row.role {
            ScopeRole::Required => {
                required += 1;
                required_done += usize::from(cursor.exhausted);
            }
            ScopeRole::Auxiliary => {
                auxiliary += 1;
                auxiliary_done += usize::from(cursor.exhausted);
            }
        }
        if !cursor.exhausted && current.is_none() {
            current = Some(index);
            current_window = Some(cursor.completed_windows);
        }
    }
    if occurrences != state.input_occurrences {
        return Err(AppError::serialization(
            "committed cursor occurrence total differs",
        ));
    }
    Ok(
        json!({"rows":state.scope.rows.len(),"required_rows":required,"exhausted_required_rows":required_done,
        "auxiliary_rows":auxiliary,"exhausted_auxiliary_rows":auxiliary_done,"required_complete":required==required_done,
        "all_rows_exhausted":current.is_none(),"current_row":current,"current_window":current_window,
        "completed_work":state.completed_work,"input_occurrences":occurrences,
        "committed_windows":committed_windows}),
    )
}

/// Explicit publication from a completed current source. The existing master
/// tail owns normalization, inherited refinement, output locks and artifacts.
pub fn publish_finite_demand_with_progress(
    request: FiniteDemandRunRequest,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: impl Fn(Value),
) -> Result<Value, AppError> {
    if request.mode != FiniteDemandMode::Resume
        || options.operation != crate::MasterReductionOperation::Publish
    {
        return Err(AppError::input(
            "finite publication needs an existing namespace; refine the resulting artifact separately",
        ));
    }
    if options.deep_verification == Some(true) {
        return Err(AppError::input(
            "finite-demand publication has no independent graph replay; trusted generated source verification is retained",
        ));
    }
    check(cancel)?;
    let (_, arity, _) = Selection::parse_with_byte_limit(
        &request.source.matching.selection_json,
        request.policy.max_source_bytes,
    )?;
    crate::ensure_runtime_arity(arity)?;
    let mut controls = control::Control::prepare(
        &request.directory,
        FiniteDemandMode::Resume,
        &request.source,
        &request.policy,
        cancel,
    )?;
    controls.require_committed_source()?;
    let output = if options.directory.exists() {
        options.directory.canonicalize()
    } else {
        options
            .directory
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .canonicalize()
            .map(|p| p.join(options.directory.file_name().unwrap_or_default()))
    }
    .map_err(|e| AppError::execution(e.to_string()))?;
    if output.starts_with(controls.directory()) || controls.directory().starts_with(&output) {
        return Err(AppError::input(
            "publication output overlaps the finite namespace",
        ));
    }
    macro_rules! dispatch { ($($n:literal),*) => { match rustred::campaign_storage_arity(arity) {
        $($n => publish::<$n>(&request, &mut controls, options, cancel, &observer),)*
        _ => Err(AppError::input("finite publication arity is not compiled")),
    } } }
    rustred::with_app_runtime_arities!(dispatch)
}
fn publish<const N: usize>(
    request: &FiniteDemandRunRequest,
    controls: &mut control::Control,
    options: &MasterReductionOptions,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> Result<Value, AppError> {
    let session = session::<N>(request, controls, cancel, observer)?;
    let captured = session.capture_completed_source(
        backend::publication::CaptureLimits {
            max_terminal_keys: request.policy.export.max_records,
            max_terminal_payload_bytes: request.policy.export.max_live_payload_bytes,
        },
        cancel,
    )?;
    check(cancel)?;
    let mut options = options.clone();
    options.finite_policy = Some(request.policy.clone());
    crate::application::master_reduction::master_reduce_finite_demand(
        &captured, &options, cancel, observer,
    )
}
