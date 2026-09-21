//! Python steering of separate candidate generation and exact certification.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rustred_app::{
    CandidateCertificationRequest, CandidateCheckpointOptions, FamilyCandidatesRequest,
    FiniteCasePolicy,
};
use std::path::PathBuf;

use crate::{
    PyClosingArtifactGenerationResult, PythonInteger, RustRedInputError, RustRedLimitError,
    apply_resource_limits, bounded_owned_input, execute, map_app_error, map_coordinator_error,
    nonnegative_usize, parse_input_format, positive_core_count,
};

#[pyclass(frozen, module = "rustred", name = "CandidateBundleResult")]
#[derive(Debug)]
pub struct PyCandidateBundleResult {
    schema: &'static str,
    status: &'static str,
    report: String,
    bundle: Py<PyBytes>,
}

#[pymethods]
impl PyCandidateBundleResult {
    #[getter]
    fn schema(&self) -> &'static str {
        self.schema
    }
    #[getter]
    fn status(&self) -> &'static str {
        self.status
    }
    #[getter]
    fn bundle(&self, py: Python<'_>) -> Py<PyBytes> {
        self.bundle.clone_ref(py)
    }
    fn to_toml(&self) -> &str {
        &self.report
    }
    fn __repr__(&self) -> String {
        format!(
            "CandidateBundleResult(schema={:?}, status={:?})",
            self.schema, self.status
        )
    }
}

/// Generate unsealed formulas in a native Symbolica binary bundle.
/// This does not certify family closure. The separate to_toml() report contains
/// observational metadata, not the program's coefficient payload.
/// exact_backend selects "sparse" (default), "sparse-factorized" (native
/// factorized denominators during exact lifting), "sparse-target-factorized"
/// (factorized symbolic target-block lifting, shared full numerical lifting),
/// or "semi-numerical". All modes retain ordinary coefficient output.
/// numerical_depth bounds only fully fixed case searches. Zero still searches
/// their initial seeds; finite residuals need not be independent masters.
/// max_numerator_rank bounds sum(max(-n_i,0)) at entry, with unbounded positive
/// powers. It is an experimental candidate scope, not certified closure, tensor
/// rank, a per-axis bound, or numerical_depth. None preserves the previous scope.
/// finite_case_policy="retain-rank-finite" deliberately keeps every finite
/// in-scope leaf once all positive axes are fixed, without minimizing it.
/// It requires max_numerator_rank; finite_* limits bound work, not the domain.
/// checkpoint_dir enables trusted-local native sector checkpoints. Use resume
/// only with the same source/root/order/backend/depth/rank. Keep final outputs outside
/// the dedicated directory; checkpoint_max_bytes is a positive payload budget,
/// not a RAM limit. Checkpoints do not certify rules or family closure.
#[pyfunction]
#[pyo3(
    signature=(source, *, input_format="auto", n_cores=PythonInteger(1), permutation=None, nonpositive_indices=None, exact_backend="sparse", numerical_depth=PythonInteger(2), max_numerator_rank=None, finite_case_policy="search", finite_max_visited_points=None, finite_max_retained_terminals=None, checkpoint_dir=None, resume=false, checkpoint_max_bytes=None),
    text_signature="(source, *, input_format='auto', n_cores=1, permutation=None, nonpositive_indices=None, exact_backend='sparse', numerical_depth=2, max_numerator_rank=None, finite_case_policy='search', finite_max_visited_points=None, finite_max_retained_terminals=None, checkpoint_dir=None, resume=False, checkpoint_max_bytes=None)"
)]
fn family_candidates(
    py: Python<'_>,
    source: &str,
    input_format: &str,
    n_cores: PythonInteger,
    permutation: Option<Vec<PythonInteger>>,
    nonpositive_indices: Option<Vec<PythonInteger>>,
    exact_backend: &str,
    numerical_depth: PythonInteger,
    max_numerator_rank: Option<PythonInteger>,
    finite_case_policy: &str,
    finite_max_visited_points: Option<PythonInteger>,
    finite_max_retained_terminals: Option<PythonInteger>,
    checkpoint_dir: Option<PathBuf>,
    resume: bool,
    checkpoint_max_bytes: Option<PythonInteger>,
) -> PyResult<PyCandidateBundleResult> {
    let finite_case_policy: FiniteCasePolicy = finite_case_policy
        .parse()
        .map_err(RustRedInputError::new_err)?;
    if finite_case_policy == FiniteCasePolicy::RetainRankFinite && max_numerator_rank.is_none() {
        return Err(RustRedInputError::new_err(
            "retain-rank-finite requires max_numerator_rank",
        ));
    }
    if finite_case_policy != FiniteCasePolicy::RetainRankFinite
        && (finite_max_visited_points.is_some() || finite_max_retained_terminals.is_some())
    {
        return Err(RustRedInputError::new_err(
            "finite retention limits require retain-rank-finite",
        ));
    }
    if checkpoint_dir.is_none() && (resume || checkpoint_max_bytes.is_some()) {
        return Err(RustRedInputError::new_err(
            "resume and checkpoint_max_bytes require checkpoint_dir",
        ));
    }
    let checkpoint = checkpoint_dir
        .map(|directory| {
            if directory.as_os_str().is_empty() {
                return Err(RustRedInputError::new_err(
                    "checkpoint_dir must not be empty",
                ));
            }
            let mut options = CandidateCheckpointOptions::new(directory);
            options.resume = resume;
            if let Some(value) = checkpoint_max_bytes {
                let bytes = nonnegative_usize("checkpoint_max_bytes", value.0)?;
                if bytes == 0 {
                    return Err(RustRedInputError::new_err(
                        "checkpoint_max_bytes must be positive",
                    ));
                }
                options.max_total_bytes = bytes;
            }
            Ok(options)
        })
        .transpose()?;
    let mut request =
        FamilyCandidatesRequest::new(bounded_owned_input("candidate family input", source)?);
    request.input_format = parse_input_format(input_format)?;
    request.exact_backend = exact_backend.parse().map_err(map_app_error)?;
    request.finite_case_policy = finite_case_policy;
    for (name, value, slot) in [
        (
            "finite_max_visited_points",
            finite_max_visited_points,
            &mut request.finite_case_limits.max_visited_points,
        ),
        (
            "finite_max_retained_terminals",
            finite_max_retained_terminals,
            &mut request.finite_case_limits.max_retained_terminals,
        ),
    ] {
        if let Some(value) = value {
            let limit = nonnegative_usize(name, value.0)?;
            if limit == 0 {
                return Err(RustRedInputError::new_err(format!(
                    "{name} must be positive"
                )));
            }
            *slot = limit;
        }
    }
    request.checkpoint = checkpoint;
    request.numerical_depth = u32::try_from(numerical_depth.0).map_err(|_| {
        RustRedInputError::new_err("numerical_depth must be an integer from 0 to 4294967295")
    })?;
    request.max_numerator_rank = max_numerator_rank
        .map(|value| {
            u32::try_from(value.0).map_err(|_| {
                RustRedInputError::new_err(
                    "max_numerator_rank must be an integer from 0 to 4294967295",
                )
            })
        })
        .transpose()?;
    request.n_cores = positive_core_count("family candidates n_cores", n_cores.0)?;
    request.permutation = permutation
        .map(|values| indices("permutation", values))
        .transpose()?;
    request.nonpositive_indices = indices(
        "nonpositive_indices",
        nonpositive_indices.unwrap_or_default(),
    )?;
    let result = py
        .detach(move || execute(move || rustred_app::family_candidates(request)))
        .map_err(map_coordinator_error)?
        .map_err(map_app_error)?;
    Ok(PyCandidateBundleResult {
        schema: result.schema(),
        status: result.status(),
        report: result.to_toml().to_owned(),
        bundle: PyBytes::new(py, result.bundle()).unbind(),
    })
}

/// Independently replay saved candidates and prove coverage before publication.
/// Only supply trusted generated bundles: Symbolica's native state/Atom import
/// is not a hardened hostile-input decoder. Mathematical certification remains
/// independent of that native deserialization boundary.
/// max_total_excess_degree bounds sum(max(n_i-1,0) + max(-n_i,0)) at entry:
/// dots and negative powers both count. Proved descendant bounds may be larger.
/// None preserves unrestricted certification. This semantic scope is separate
/// from resource limits and mutually exclusive with max_negative_index_degree.
#[pyfunction]
#[pyo3(
    signature=(bundle, *, max_domain_bound_endpoint_cells=None, max_predicate_consistency_work=None, max_predicate_atoms=None, max_negative_index_degree=None, max_total_excess_degree=None),
    text_signature="(bundle, *, max_domain_bound_endpoint_cells=None, max_predicate_consistency_work=None, max_predicate_atoms=None, max_negative_index_degree=None, max_total_excess_degree=None)"
)]
fn certify_candidates(
    py: Python<'_>,
    bundle: &Bound<'_, PyBytes>,
    max_domain_bound_endpoint_cells: Option<PythonInteger>,
    max_predicate_consistency_work: Option<PythonInteger>,
    max_predicate_atoms: Option<PythonInteger>,
    max_negative_index_degree: Option<PythonInteger>,
    max_total_excess_degree: Option<PythonInteger>,
) -> PyResult<PyClosingArtifactGenerationResult> {
    let mut limits = rustred_app::SourcePortLimits::default();
    apply_resource_limits(
        max_domain_bound_endpoint_cells,
        max_predicate_consistency_work,
        max_predicate_atoms,
        &mut limits.rule_derivation.max_domain_bound_endpoint_cells,
        &mut limits.max_predicate_consistency_work,
        &mut limits.max_predicate_atoms,
    )?;
    let bytes = bundle.as_bytes();
    if bytes.len() > rustred_app::MAX_CLOSING_ARTIFACT_BYTES {
        return Err(RustRedLimitError::new_err(format!(
            "candidate bundle has {} bytes, exceeding the {}-byte application limit",
            bytes.len(),
            rustred_app::MAX_CLOSING_ARTIFACT_BYTES,
        )));
    }
    let mut request = CandidateCertificationRequest::new(bytes.to_vec());
    request.publication_limits = limits;
    request.max_negative_index_degree = max_negative_index_degree
        .map(|value| nonnegative_usize("max_negative_index_degree", value.0))
        .transpose()?;
    request.max_total_excess_degree = max_total_excess_degree
        .map(|value| {
            u64::try_from(value.0).map_err(|_| {
                RustRedInputError::new_err(
                    "max_total_excess_degree must be an integer from 0 to 18446744073709551615",
                )
            })
        })
        .transpose()?;
    let result = py
        .detach(move || execute(move || rustred_app::certify_candidates(request)))
        .map_err(map_coordinator_error)?
        .map_err(map_app_error)?;
    Ok(PyClosingArtifactGenerationResult {
        schema: result.schema(),
        status: result.status(),
        canonical_toml: result.to_toml().to_owned(),
        artifact: PyBytes::new(py, result.artifact()).unbind(),
    })
}

fn indices(label: &str, values: Vec<PythonInteger>) -> PyResult<Vec<usize>> {
    values
        .into_iter()
        .enumerate()
        .map(|(position, value)| nonnegative_usize(&format!("{label}[{position}]"), value.0))
        .collect()
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCandidateBundleResult>()?;
    module.add_function(wrap_pyfunction!(family_candidates, module)?)?;
    module.add_function(wrap_pyfunction!(certify_candidates, module)?)?;
    Ok(())
}
