//! Python presentation of the shared native session and lazy artifact APIs.

use crate::{
    PythonInteger, RustRedInputError, RustRedLimitError,
    candidates::{PyCandidateBundleResult, request_from_options},
    coordinator::process_coordinator,
    execute, map_app_error, map_coordinator_error, nonnegative_usize,
};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
use rustred_app::{
    CandidateArtifact, CandidateBundleLimits, CandidateGenerationSession, NativeIntegralFamily,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[pyclass(frozen, module = "rustred", name = "CandidateGenerationSession")]
pub struct PyCandidateGenerationSession {
    inner: CandidateGenerationSession,
}

#[pymethods]
impl PyCandidateGenerationSession {
    /// Native sessions run on the coordinator; WASM start() completes inline.
    #[getter]
    fn execution_mode(&self) -> &'static str {
        crate::execution_mode()
    }
    #[getter]
    fn done(&self) -> PyResult<bool> {
        self.inner.done().map_err(map_app_error)
    }
    fn cancel(&self) -> PyResult<()> {
        self.inner.cancel().map_err(map_app_error)
    }
    fn snapshot(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_python(py, &self.inner.snapshot().map_err(map_app_error)?)
    }
    #[pyo3(signature=(max_events=PythonInteger(128),timeout=0.0))]
    fn poll_events(
        &self,
        py: Python<'_>,
        max_events: PythonInteger,
        timeout: f64,
    ) -> PyResult<Py<PyAny>> {
        let count = nonnegative_usize("max_events", max_events.0)?;
        let timeout = seconds(timeout)?;
        let started = Instant::now();
        loop {
            let interval = timeout
                .saturating_sub(started.elapsed())
                .min(Duration::from_millis(100));
            let result = py
                .detach(|| self.inner.poll_events(count, interval))
                .map_err(map_app_error)?;
            py.check_signals()?;
            if !result.events.is_empty() || result.snapshot.done || started.elapsed() >= timeout {
                return to_python(py, &result);
            }
        }
    }
    #[pyo3(signature=(timeout=None))]
    fn wait(&self, py: Python<'_>, timeout: Option<f64>) -> PyResult<bool> {
        let timeout = timeout.map(seconds).transpose()?;
        let started = Instant::now();
        loop {
            let interval = timeout
                .map(|t| {
                    t.saturating_sub(started.elapsed())
                        .min(Duration::from_millis(100))
                })
                .unwrap_or(Duration::from_millis(100));
            let done = py
                .detach(|| self.inner.wait(Some(interval)))
                .map_err(map_app_error)?;
            py.check_signals()?;
            if done {
                return Ok(true);
            }
            if timeout.is_some_and(|t| started.elapsed() >= t) {
                return Ok(false);
            }
        }
    }
    fn result(&self, py: Python<'_>) -> PyResult<PyCandidateBundleResult> {
        let result = self.inner.result().map_err(map_app_error)?;
        Ok(PyCandidateBundleResult::from_native(py, &result))
    }
    fn __repr__(&self) -> PyResult<String> {
        let snapshot = self.inner.snapshot().map_err(map_app_error)?;
        Ok(format!(
            "CandidateGenerationSession(state={:?}, done={})",
            snapshot.state, snapshot.done
        ))
    }
    fn _repr_html_(&self) -> PyResult<String> {
        let s = self.inner.snapshot().map_err(map_app_error)?;
        Ok(format!(
            "<div class=\"rustred-session\"><strong>RustRed candidate generation: {}</strong><p>{:.2} s · {} sectors generated · {} reused · {} rules · {} finite residuals</p><small>Candidate generation only; completion is not a closure or master-minimality certificate.</small>{}</div>",
            html_escape(&format!("{:?}", s.state)),
            s.elapsed_seconds,
            s.counts["generated"],
            s.counts["reused"],
            s.counts["rules"],
            s.counts["finite_residuals"],
            s.last_error
                .as_ref()
                .map(|e| format!("<p>{}</p>", html_escape(e)))
                .unwrap_or_default()
        ))
    }
}

fn submit(
    inner: CandidateGenerationSession,
    job: rustred_app::CandidateGenerationJob,
) -> PyResult<PyCandidateGenerationSession> {
    process_coordinator()
        .map_err(crate::RustRedInternalError::new_err)?
        .submit(move || job.run())
        .map_err(map_coordinator_error)?;
    Ok(PyCandidateGenerationSession { inner })
}

pub(crate) fn start_request(
    request: rustred_app::FamilyCandidatesRequest,
    event_capacity: usize,
) -> PyResult<PyCandidateGenerationSession> {
    let (session, job) =
        CandidateGenerationSession::prepare(request, event_capacity).map_err(map_app_error)?;
    submit(session, job)
}

/// Embedded-host entry. The Arc is the existing family with its original
/// denominator/parameter identity; options only steer the common generator.
/// The host must link this rlib into its existing Symbolica-owning extension.
/// On WASM this runs synchronously and returns an already completed session;
/// buffered events remain available, but there is no background progress.
pub fn start_from_native_family(
    py: Python<'_>,
    family: Arc<NativeIntegralFamily>,
    options: Option<&Bound<'_, PyDict>>,
    event_capacity: usize,
) -> PyResult<PyCandidateGenerationSession> {
    let request = request_from_options(py, "native-family", options)?;
    let (session, job) = CandidateGenerationSession::from_family(family, request, event_capacity)
        .map_err(map_app_error)?;
    submit(session, job)
}

/// Start generation on the native background coordinator, or synchronously in
/// WebAssembly. Inspect execution_capabilities() before offering live progress
/// or cancellation controls in a browser UI.
#[pyfunction]
#[pyo3(signature=(source,*,event_capacity=PythonInteger(256),**options))]
fn start_family_candidates(
    py: Python<'_>,
    source: &str,
    event_capacity: PythonInteger,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<PyCandidateGenerationSession> {
    let request = request_from_options(py, source, options)?;
    start_request(
        request,
        nonnegative_usize("event_capacity", event_capacity.0)?,
    )
}

#[pyclass(frozen, module = "rustred", name = "CandidateArtifact")]
pub struct PyCandidateArtifact {
    pub(crate) inner: Arc<CandidateArtifact>,
}

impl PyCandidateArtifact {
    pub(crate) fn from_bytes(
        py: Python<'_>,
        bytes: &[u8],
        limits: CandidateBundleLimits,
    ) -> PyResult<Self> {
        let cap = limits
            .max_bundle_bytes
            .min(rustred_app::MAX_CANDIDATE_BUNDLE_BYTES);
        if bytes.len() > cap {
            return Err(RustRedLimitError::new_err(format!(
                "artifact exceeds native byte limit: {} > {cap}",
                bytes.len(),
            )));
        }
        let bytes = bytes.to_vec();
        // Framing/structural parsing imports no Symbolica state and decodes no
        // coefficient polynomial, so it need not queue behind generation. The
        // existing codec's once-only format-byte probe encodes the integer 1.
        let inner = py
            .detach(move || CandidateArtifact::open(&bytes, limits))
            .map_err(map_app_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
}

#[pymethods]
impl PyCandidateArtifact {
    #[staticmethod]
    #[pyo3(signature=(bundle, *, bundle_max_bytes=None, bundle_max_entries=None, bundle_max_coefficient_bytes=None, bundle_max_total_coefficient_bytes=None))]
    fn open(
        py: Python<'_>,
        bundle: &Bound<'_, PyBytes>,
        bundle_max_bytes: Option<PythonInteger>,
        bundle_max_entries: Option<PythonInteger>,
        bundle_max_coefficient_bytes: Option<PythonInteger>,
        bundle_max_total_coefficient_bytes: Option<PythonInteger>,
    ) -> PyResult<Self> {
        Self::from_bytes(
            py,
            bundle.as_bytes(),
            read_limits([
                bundle_max_bytes,
                bundle_max_entries,
                bundle_max_coefficient_bytes,
                bundle_max_total_coefficient_bytes,
            ])?,
        )
    }
    #[staticmethod]
    #[pyo3(signature=(path, *, bundle_max_bytes=None, bundle_max_entries=None, bundle_max_coefficient_bytes=None, bundle_max_total_coefficient_bytes=None))]
    fn open_file(
        py: Python<'_>,
        path: PathBuf,
        bundle_max_bytes: Option<PythonInteger>,
        bundle_max_entries: Option<PythonInteger>,
        bundle_max_coefficient_bytes: Option<PythonInteger>,
        bundle_max_total_coefficient_bytes: Option<PythonInteger>,
    ) -> PyResult<Self> {
        let limits = read_limits([
            bundle_max_bytes,
            bundle_max_entries,
            bundle_max_coefficient_bytes,
            bundle_max_total_coefficient_bytes,
        ])?;
        let inner = py
            .detach(move || CandidateArtifact::open_file(path, limits))
            .map_err(map_app_error)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
    fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        to_python(py, &self.inner.metadata().map_err(map_app_error)?)
    }
    #[pyo3(signature=(start=PythonInteger(0),limit=PythonInteger(50)))]
    fn sectors(
        &self,
        py: Python<'_>,
        start: PythonInteger,
        limit: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .sectors(index("start", start)?, index("limit", limit)?)
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(sector,start=PythonInteger(0),limit=PythonInteger(50)))]
    fn rules(
        &self,
        py: Python<'_>,
        sector: PythonInteger,
        start: PythonInteger,
        limit: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .rules(
                    index("sector", sector)?,
                    index("start", start)?,
                    index("limit", limit)?,
                )
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(sector,start=PythonInteger(0),limit=PythonInteger(50)))]
    fn terminals(
        &self,
        py: Python<'_>,
        sector: PythonInteger,
        start: PythonInteger,
        limit: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .terminals(
                    index("sector", sector)?,
                    index("start", start)?,
                    index("limit", limit)?,
                )
                .map_err(map_app_error)?,
        )
    }
    #[pyo3(signature=(sector,ordinal,*,max_output_bytes=PythonInteger(1048576)))]
    fn rule(
        &self,
        py: Python<'_>,
        sector: PythonInteger,
        ordinal: PythonInteger,
        max_output_bytes: PythonInteger,
    ) -> PyResult<Py<PyAny>> {
        to_python(
            py,
            &self
                .inner
                .rule(
                    index("sector", sector)?,
                    index("ordinal", ordinal)?,
                    index("max_output_bytes", max_output_bytes)?,
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
        let id = index("id", id)?;
        let budget = index("max_output_bytes", max_output_bytes)?;
        let inner = self.inner.clone();
        let detail = py
            .detach(move || execute(move || inner.coefficient(id, budget)))
            .map_err(map_coordinator_error)?
            .map_err(map_app_error)?;
        to_python(py, &detail)
    }
    fn __repr__(&self) -> &'static str {
        "CandidateArtifact(lazy=True, authority='uncertified candidates')"
    }
    fn _repr_html_(&self) -> PyResult<String> {
        let meta = self.inner.metadata().map_err(map_app_error)?;
        Ok(format!(
            "<div class=\"rustred-artifact\"><strong>RustRed candidate artifact</strong><p>{} axes · {} sectors · {} rules · {} finite residuals</p><p>{}/{} coefficients decoded</p><small>{} · structural view, not a closure certificate</small></div>",
            meta["arity"],
            meta["total_sectors"],
            meta["total_rules"],
            meta["total_terminals"],
            meta["decoded_coefficients"],
            meta["total_coefficients"],
            html_preview(
                meta["family_fingerprint"]
                    .as_str()
                    .unwrap_or("unknown family"),
                96,
            )
        ))
    }
}

pub(crate) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn html_preview(value: &str, max_chars: usize) -> String {
    let prefix: String = value.chars().take(max_chars).collect();
    if prefix.len() < value.len() {
        format!(
            "{}… ({} bytes; full identity in metadata)",
            html_escape(&prefix),
            value.len()
        )
    } else {
        html_escape(value)
    }
}

fn read_limits(values: [Option<PythonInteger>; 4]) -> PyResult<CandidateBundleLimits> {
    let mut limits = CandidateBundleLimits::default();
    for ((name, slot), value) in [
        ("bundle_max_bytes", &mut limits.max_bundle_bytes),
        ("bundle_max_entries", &mut limits.max_collection_entries),
        (
            "bundle_max_coefficient_bytes",
            &mut limits.max_coefficient_bytes,
        ),
        (
            "bundle_max_total_coefficient_bytes",
            &mut limits.max_total_coefficient_bytes,
        ),
    ]
    .into_iter()
    .zip(values)
    {
        if let Some(value) = value {
            let value = nonnegative_usize(name, value.0)?;
            if value == 0 {
                return Err(RustRedInputError::new_err(format!(
                    "{name} must be positive"
                )));
            }
            *slot = value;
        }
    }
    Ok(limits)
}

fn index(name: &str, value: PythonInteger) -> PyResult<usize> {
    nonnegative_usize(name, value.0)
}
fn seconds(value: f64) -> PyResult<Duration> {
    if !value.is_finite() || value < 0.0 || value > 86_400.0 {
        return Err(RustRedInputError::new_err(
            "timeout must be finite and between 0 and 86400 seconds",
        ));
    }
    Ok(Duration::from_secs_f64(value))
}
pub(crate) fn to_python(py: Python<'_>, value: &impl Serialize) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(value)
        .map_err(|e| crate::RustRedInternalError::new_err(e.to_string()))?;
    value_to_python(py, value)
}
fn value_to_python(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(v) => v.into_pyobject(py)?.to_owned().into_any().unbind(),
        Value::String(v) => v.into_pyobject(py)?.into_any().unbind(),
        Value::Number(v) => {
            if let Some(v) = v.as_u64() {
                v.into_pyobject(py)?.into_any().unbind()
            } else if let Some(v) = v.as_i64() {
                v.into_pyobject(py)?.into_any().unbind()
            } else {
                v.as_f64()
                    .expect("JSON finite float")
                    .into_pyobject(py)?
                    .into_any()
                    .unbind()
            }
        }
        Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_python(py, item)?)?;
            }
            list.into_any().unbind()
        }
        Value::Object(items) => {
            let dict = PyDict::new(py);
            for (key, value) in items {
                dict.set_item(key, value_to_python(py, value)?)?;
            }
            dict.into_any().unbind()
        }
    })
}
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCandidateGenerationSession>()?;
    module.add_class::<PyCandidateArtifact>()?;
    module.add_class::<crate::normalization::PyTerminalNormalization>()?;
    module.add_function(wrap_pyfunction!(start_family_candidates, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    #[test]
    fn explicit_read_limits_preserve_defaults_and_reject_invalid_values() {
        assert_eq!(
            read_limits([None; 4]).unwrap(),
            CandidateBundleLimits::default()
        );
        assert_eq!(
            read_limits([None, Some(PythonInteger(10_000_000)), None, None])
                .unwrap()
                .max_collection_entries,
            10_000_000
        );
        for value in [0, -1, i128::MAX] {
            assert!(read_limits([Some(PythonInteger(value)), None, None, None]).is_err());
        }
    }

    #[test]
    fn rich_identity_preview_is_short_unicode_safe_and_escaped() {
        let text = "λ<&>".repeat(1000);
        let preview = html_preview(&text, 96);
        assert!(preview.len() < 600);
        assert!(preview.contains("&lt;"));
        assert!(!preview.contains('<'));
        assert!(preview.contains("full identity in metadata"));
        assert_eq!(html_preview("<short>", 96), "&lt;short&gt;");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn wait_and_poll_release_python_for_a_required_heartbeat() {
        Python::initialize();
        for use_poll in [false, true] {
            let heartbeat = Arc::new(AtomicBool::new(false));
            let (worker, completed) = Python::attach(|py| {
                let (inner, job) = CandidateGenerationSession::prepare(
                    rustred_app::FamilyCandidatesRequest::new("unused test job"),
                    4,
                )
                .unwrap();
                let session = PyCandidateGenerationSession { inner };
                // A zero-time poll is observational and cannot finish this
                // queued producer. No wall-time performance assertion needed.
                session.poll_events(py, PythonInteger(1), 0.0).unwrap();
                assert!(!session.done().unwrap());
                let (started, ready) = mpsc::sync_channel(1);
                let heartbeat = heartbeat.clone();
                let worker = std::thread::spawn(move || {
                    started.send(()).unwrap();
                    // The job cannot finish until this OTHER thread acquires
                    // Python. Holding the GIL throughout wait/poll would make
                    // the bounded call time out, rather than deadlock the test.
                    Python::attach(|_| heartbeat.store(true, Ordering::Release));
                    drop(job);
                });
                ready.recv().unwrap();
                let completed = if use_poll {
                    session.poll_events(py, PythonInteger(1), 5.0).unwrap();
                    session.done().unwrap()
                } else {
                    session.wait(py, Some(5.0)).unwrap()
                };
                (worker, completed)
            });
            worker.join().unwrap();
            assert!(heartbeat.load(Ordering::Acquire));
            assert!(completed, "Python heartbeat could not run during wait/poll");
        }
    }
}
