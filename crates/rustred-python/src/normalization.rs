//! Explicit normalization in the same Symbolica-owning coordinator as generation.
use crate::streaming::{PyCandidateArtifact, html_preview, to_python};
use crate::{
    PythonInteger, RustRedInputError, execute, map_app_error, map_coordinator_error,
    nonnegative_usize,
};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use rustred_app::{
    CandidateTerminalNormalization, CandidateTerminalNormalizationLimits, NativeIntegralFamily,
};
use std::sync::Arc;

#[pyclass(frozen, module = "rustred", name = "TerminalNormalization")]
pub struct PyTerminalNormalization {
    inner: Arc<CandidateTerminalNormalization>,
}

/// Host seam: reuse the existing exact family allocation. No TOML serialization,
/// denominator rematching, external catalog, or second Symbolica extension.
pub fn normalize_from_native_family(
    py: Python<'_>,
    family: Arc<NativeIntegralFamily>,
    artifact: &PyCandidateArtifact,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyTerminalNormalization> {
    artifact.inner.check_process().map_err(map_app_error)?;
    let mut limits = CandidateTerminalNormalizationLimits::default();
    if let Some(options) = options {
        for (key, value) in options.iter() {
            let key: String = key.extract()?;
            let value = value.extract::<PythonInteger>()?;
            let value = nonnegative_usize(&key, value.0)?;
            if value == 0 {
                return Err(RustRedInputError::new_err(format!(
                    "{key} must be positive"
                )));
            }
            match key.as_str() {
                "max_terminals" => limits.max_terminals = value,
                "max_supports" => limits.max_supports = value,
                "max_matrix_cells" => limits.max_matrix_cells = value,
                "max_output_terms" => limits.max_output_terms = value,
                _ => {
                    return Err(RustRedInputError::new_err(format!(
                        "unknown normalization option {key}"
                    )));
                }
            }
        }
    }
    let artifact = artifact.inner.clone();
    let result = py
        .detach(move || execute(move || artifact.normalize_terminals(&family, limits)))
        .map_err(map_coordinator_error)?
        .map_err(map_app_error)?;
    Ok(PyTerminalNormalization {
        inner: Arc::new(result),
    })
}

#[pymethods]
impl PyTerminalNormalization {
    fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_python(py, &self.inner.metadata().map_err(map_app_error)?)
    }
    #[pyo3(signature=(start=PythonInteger(0),limit=PythonInteger(50)))]
    fn terminals(
        &self,
        py: Python<'_>,
        start: PythonInteger,
        limit: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .terminals(
                    nonnegative_usize("start", start.0)?,
                    nonnegative_usize("limit", limit.0)?,
                )
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(start=PythonInteger(0),limit=PythonInteger(50)))]
    fn relations(
        &self,
        py: Python<'_>,
        start: PythonInteger,
        limit: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .relations(
                    nonnegative_usize("start", start.0)?,
                    nonnegative_usize("limit", limit.0)?,
                )
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(ordinal,*,max_output_bytes=PythonInteger(1048576)))]
    fn relation(
        &self,
        py: Python<'_>,
        ordinal: PythonInteger,
        max_output_bytes: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .relation(
                    nonnegative_usize("ordinal", ordinal.0)?,
                    nonnegative_usize("max_output_bytes", max_output_bytes.0)?,
                )
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(id,*,max_output_bytes=PythonInteger(65536)))]
    fn coefficient(
        &self,
        py: Python<'_>,
        id: PythonInteger,
        max_output_bytes: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        self.inner.check_process().map_err(map_app_error)?;
        let id = nonnegative_usize("id", id.0)?;
        let limit = nonnegative_usize("max_output_bytes", max_output_bytes.0)?;
        let inner = self.inner.clone();
        let detail = py
            .detach(move || execute(move || inner.coefficient(id, limit)))
            .map_err(map_coordinator_error)?
            .map_err(map_app_error)?;
        to_python(py, &detail)
    }
    fn sidecar(&self, py: Python<'_>) -> PyResult<Py<PyBytes>> {
        self.inner.check_process().map_err(map_app_error)?;
        let inner = self.inner.clone();
        let bytes = py
            .detach(move || execute(move || inner.sidecar()))
            .map_err(map_coordinator_error)?
            .map_err(map_app_error)?;
        Ok(PyBytes::new(py, &bytes).unbind())
    }
    fn __repr__(&self) -> &'static str {
        "TerminalNormalization(exact_within_family=True, master_minimality=False)"
    }
    fn _repr_html_(&self) -> PyResult<String> {
        let m = self.inner.metadata().map_err(map_app_error)?;
        Ok(format!(
            "<div class=\"rustred-normalization\"><strong>Exact native terminal normalization</strong><p>{} unique raw terminals → {} after unit aliases → {} weighted outputs</p><small>{} · not a master-minimality or closure certificate</small></div>",
            m["unique_raw_terminals"],
            m["after_unit_aliases"],
            m["canonical_terminals"],
            html_preview(
                m["family_fingerprint"].as_str().unwrap_or("unknown family"),
                96
            )
        ))
    }
}
