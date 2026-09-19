//! Python steering of separate candidate generation and exact certification.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use rustred_app::{CandidateCertificationRequest, FamilyCandidatesRequest};

use crate::{
    PyClosingArtifactGenerationResult, PythonInteger, RustRedLimitError, apply_resource_limits,
    bounded_owned_input, execute, map_app_error, map_coordinator_error, nonnegative_usize,
    parse_input_format, positive_core_count,
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

/// Generate unsealed formulas only; this does not certify family closure.
#[pyfunction]
#[pyo3(
    signature=(source, *, input_format="auto", n_cores=PythonInteger(1), permutation=None, nonpositive_indices=None, exact_backend="sparse"),
    text_signature="(source, *, input_format='auto', n_cores=1, permutation=None, nonpositive_indices=None, exact_backend='sparse')"
)]
fn family_candidates(
    py: Python<'_>,
    source: &str,
    input_format: &str,
    n_cores: PythonInteger,
    permutation: Option<Vec<PythonInteger>>,
    nonpositive_indices: Option<Vec<PythonInteger>>,
    exact_backend: &str,
) -> PyResult<PyCandidateBundleResult> {
    let mut request =
        FamilyCandidatesRequest::new(bounded_owned_input("candidate family input", source)?);
    request.input_format = parse_input_format(input_format)?;
    request.exact_backend = exact_backend.parse().map_err(map_app_error)?;
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
#[pyfunction]
#[pyo3(
    signature=(bundle, *, max_domain_bound_endpoint_cells=None, max_predicate_consistency_work=None, max_predicate_atoms=None, max_negative_index_degree=None),
    text_signature="(bundle, *, max_domain_bound_endpoint_cells=None, max_predicate_consistency_work=None, max_predicate_atoms=None, max_negative_index_degree=None)"
)]
fn certify_candidates(
    py: Python<'_>,
    bundle: &Bound<'_, PyBytes>,
    max_domain_bound_endpoint_cells: Option<PythonInteger>,
    max_predicate_consistency_work: Option<PythonInteger>,
    max_predicate_atoms: Option<PythonInteger>,
    max_negative_index_degree: Option<PythonInteger>,
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
