//! Native generation sessions in the existing HEPKit/Symbolica host.

use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyBool, PyDict},
};
use symbolica::api::python::PythonExpression;

use super::{PyIbpFamily, record_usage};

struct EventCapacity(usize);

impl<'a, 'py> FromPyObject<'a, 'py> for EventCapacity {
    type Error = PyErr;

    fn extract(value: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            return Err(PyValueError::new_err(
                "event_capacity must be an integer, not bool",
            ));
        }
        value.extract::<usize>().map(Self)
    }
}

#[pymethods]
impl PyIbpFamily {
    /// Explicit exact finite normalization of this family's candidate terminals.
    ///
    /// Reuses RustRed's verified vacuum U/permutation and quadratic-numerator
    /// transformations. It reads structural terminal keys, not rule coefficients;
    /// unsupported shapes remain declared outputs. No FMFT values, new IBPs,
    /// generation, closure or master-minimality claim is involved. The returned
    /// native object has paged keys/relations and an explicit native sidecar.
    #[pyo3(signature=(artifact, **options))]
    fn normalize_candidate_terminals(
        &self,
        py: Python<'_>,
        artifact: &rustred_python::PyCandidateArtifact,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<rustred_python::PyTerminalNormalization> {
        if self.cuts.required_active().active_count() != 0 {
            return Err(PyValueError::new_err(
                "vacuum terminal normalization does not support cut families",
            ));
        }
        record_usage();
        rustred_python::normalize_from_native_family(py, self.family.clone(), artifact, options)
    }
    /// Native internal parameters paired with their original HEPKit expressions.
    ///
    /// The existing bridge represents opaque scalar invariants as independent
    /// rational-polynomial variables. Artifact IDs and variable names retain
    /// that representation; this exact native mapping provides a display legend
    /// without changing the serialized algebra or replacing printed strings.
    #[getter]
    fn parameter_bindings(&self) -> Vec<(PythonExpression, PythonExpression)> {
        self.parameters
            .iter()
            .map(|(internal, original)| (internal.clone().into(), original.clone().into()))
            .collect()
    }

    /// Start native parametric-IBP candidate generation without blocking Python.
    ///
    /// The family's routed denominators and internal parameter basis are passed
    /// directly to RustRed. See ``parameter_bindings`` for their original HEPKit
    /// expressions. No graph matching, source serialization or second
    /// Symbolica extension is used. Set ``nonpositive_indices`` for auxiliary
    /// ISP coordinates, and ``n_cores`` to control native worker count.
    ///
    /// Poll ``session.poll_events()`` for structured events and an aggregate
    /// snapshot. ``session.cancel()`` requests cooperative cancellation at
    /// safe sector boundaries; it does not interrupt an in-flight CAS call.
    /// Once completed, ``session.result().artifact()`` opens a lazy rule and
    /// terminal explorer. A generated candidate is not a closure certificate.
    ///
    /// Cut families are currently supported by the finite/parametric bridge,
    /// not by this all-sector artifact front end; they are rejected explicitly.
    ///
    /// Examples
    /// --------
    /// >>> session = ibp.start_generation(n_cores=1, nonpositive_indices=[9])
    /// >>> batch = session.poll_events(max_events=64, timeout=0.0)
    /// >>> batch["snapshot"]["state"]
    #[pyo3(signature = (*, event_capacity=EventCapacity(256), **options))]
    fn start_generation(
        &self,
        py: Python<'_>,
        event_capacity: EventCapacity,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<rustred_python::PyCandidateGenerationSession> {
        if self.cuts.required_active().active_count() != 0 {
            return Err(PyValueError::new_err(
                "all-sector candidate generation does not yet support cut families; \
                 use solve_parametric or reduce_laporta with the cut-aware bridge",
            ));
        }
        record_usage();
        rustred_python::start_from_native_family(py, self.family.clone(), options, event_capacity.0)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let child = PyModule::new(module.py(), "rustred")?;
    rustred_python::register_rustred_module(&child)?;
    module.add_submodule(&child)
}
