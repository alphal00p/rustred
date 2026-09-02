mod coordinator;

use std::str::FromStr;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBool, PyBytes};
use rustred_app::{
    AppError, AppErrorKind, CampaignPlanRequest, CampaignPreflightRequest,
    ClosingArtifactGenerateRequest, ClosingArtifactInspectRequest, ClosingArtifactReduceRequest,
    ClosingFamilySelector, DeriveRequest, FoundryCampaignRunRequest, FoundryWaveCampaignRunRequest,
    InputFormat, RelationSelection, campaign_plan as app_campaign_plan,
    campaign_preflight as app_campaign_preflight,
    closing_artifact_generate as app_closing_artifact_generate,
    closing_artifact_inspect as app_closing_artifact_inspect,
    closing_artifact_reduce as app_closing_artifact_reduce, derive as app_derive,
    foundry_campaign_run as app_foundry_campaign_run,
    foundry_wave_campaign_run as app_foundry_wave_campaign_run,
};

use crate::coordinator::{CoordinatorError, process_coordinator};

create_exception!(rustred, RustRedError, PyException);
create_exception!(rustred, RustRedInputError, RustRedError);
create_exception!(rustred, RustRedSchemaError, RustRedInputError);
create_exception!(rustred, RustRedLimitError, RustRedInputError);
create_exception!(rustred, RustRedLoweringError, RustRedInputError);
create_exception!(rustred, RustRedDerivationError, RustRedError);
create_exception!(rustred, RustRedExecutionError, RustRedError);
create_exception!(rustred, RustRedLicenseError, RustRedExecutionError);
create_exception!(rustred, RustRedSerializationError, RustRedError);
create_exception!(rustred, RustRedOutputLimitError, RustRedSerializationError);
create_exception!(rustred, RustRedInternalError, RustRedError);
create_exception!(
    rustred,
    RustRedCoordinatorPoisonedError,
    RustRedInternalError
);

#[derive(Clone, Copy)]
struct PythonInteger(i128);

impl<'a, 'py> FromPyObject<'a, 'py> for PythonInteger {
    type Error = PyErr;

    fn extract(value: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            return Err(RustRedInputError::new_err(
                "integer arguments cannot be bool",
            ));
        }
        value.extract::<i128>().map(Self).map_err(|_| {
            RustRedInputError::new_err("integer argument must fit a signed 128-bit integer")
        })
    }
}

macro_rules! canonical_result {
    ($python_name:literal, $name:ident) => {
        #[pyclass(frozen, module = "rustred", name = $python_name)]
        #[derive(Debug)]
        pub struct $name {
            schema: &'static str,
            status: &'static str,
            canonical_toml: String,
        }

        impl $name {
            fn new(schema: &'static str, status: &'static str, canonical_toml: String) -> Self {
                Self {
                    schema,
                    status,
                    canonical_toml,
                }
            }
        }

        #[pymethods]
        impl $name {
            #[getter]
            fn schema(&self) -> &'static str {
                self.schema
            }

            #[getter]
            fn status(&self) -> &'static str {
                self.status
            }

            /// Return the exact newline-terminated TOML from `rustred-app`.
            fn to_toml(&self) -> &str {
                &self.canonical_toml
            }

            fn __repr__(&self) -> String {
                format!(
                    "{}(schema={:?}, status={:?})",
                    $python_name, self.schema, self.status
                )
            }
        }
    };
}

canonical_result!("DeriveResult", PyDeriveResult);
canonical_result!("CampaignPlanResult", PyCampaignPlanResult);
canonical_result!("CampaignPreflightResult", PyCampaignPreflightResult);
canonical_result!(
    "ClosingArtifactInspectionResult",
    PyClosingArtifactInspectionResult
);

/// Deterministic diagnostic state from one bounded foundry experiment.
///
/// Measurements are deliberately separate from the semantic report so that
/// callers can compare report bytes across runs and worker configurations.
#[pyclass(frozen, module = "rustred", name = "FoundryCampaignRunResult")]
#[derive(Debug)]
pub struct PyFoundryCampaignRunResult {
    schema: &'static str,
    measurements_schema: &'static str,
    report_toml: String,
    measurements_toml: String,
}

/// Deterministic detached state from one full-rank atomic-wave experiment.
#[pyclass(frozen, module = "rustred", name = "FoundryWaveCampaignRunResult")]
#[derive(Debug)]
pub struct PyFoundryWaveCampaignRunResult {
    schema: &'static str,
    measurements_schema: &'static str,
    report_toml: String,
    measurements_toml: String,
    artifact: Option<Vec<u8>>,
}

#[pymethods]
impl PyFoundryWaveCampaignRunResult {
    #[getter]
    fn schema(&self) -> &'static str {
        self.schema
    }

    #[getter]
    fn measurements_schema(&self) -> &'static str {
        self.measurements_schema
    }

    fn to_toml(&self) -> &str {
        &self.report_toml
    }

    fn measurements_to_toml(&self) -> &str {
        &self.measurements_toml
    }

    #[getter]
    fn artifact_bytes<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyBytes>> {
        self.artifact
            .as_deref()
            .map(|bytes| PyBytes::new(py, bytes))
    }

    fn __repr__(&self) -> String {
        format!(
            "FoundryWaveCampaignRunResult(schema={:?}, measurements_schema={:?})",
            self.schema, self.measurements_schema
        )
    }
}

#[pymethods]
impl PyFoundryCampaignRunResult {
    #[getter]
    fn schema(&self) -> &'static str {
        self.schema
    }

    #[getter]
    fn measurements_schema(&self) -> &'static str {
        self.measurements_schema
    }

    /// Return the deterministic, newline-terminated campaign report.
    fn to_toml(&self) -> &str {
        &self.report_toml
    }

    /// Return the nonsemantic wall-clock measurement sidecar.
    fn measurements_to_toml(&self) -> &str {
        &self.measurements_toml
    }

    fn __repr__(&self) -> String {
        format!(
            "FoundryCampaignRunResult(schema={:?}, measurements_schema={:?})",
            self.schema, self.measurements_schema
        )
    }
}

#[pyclass(frozen, module = "rustred", name = "ClosingArtifactGenerationResult")]
#[derive(Debug)]
pub struct PyClosingArtifactGenerationResult {
    schema: &'static str,
    status: &'static str,
    canonical_toml: String,
    artifact: Py<PyBytes>,
}

#[pymethods]
impl PyClosingArtifactGenerationResult {
    #[getter]
    fn schema(&self) -> &'static str {
        self.schema
    }

    #[getter]
    fn status(&self) -> &'static str {
        self.status
    }

    #[getter]
    fn artifact(&self, py: Python<'_>) -> Py<PyBytes> {
        self.artifact.clone_ref(py)
    }

    fn to_toml(&self) -> &str {
        &self.canonical_toml
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "ClosingArtifactGenerationResult(schema={:?}, status={:?}, artifact_bytes={})",
            self.schema,
            self.status,
            self.artifact.bind(py).as_bytes().len()
        )
    }
}

#[pyclass(
    frozen,
    module = "rustred",
    name = "ExactMasterCoefficient",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyExactMasterCoefficient {
    master_powers: Vec<i64>,
    unit_mass_coefficient: String,
    common_mass_squared_power: i128,
}

#[pymethods]
impl PyExactMasterCoefficient {
    #[getter]
    fn master_powers(&self) -> Vec<i64> {
        self.master_powers.clone()
    }

    #[getter]
    fn unit_mass_coefficient(&self) -> &str {
        &self.unit_mass_coefficient
    }

    #[getter]
    fn common_mass_squared_power(&self) -> i128 {
        self.common_mass_squared_power
    }

    fn __repr__(&self) -> String {
        format!(
            "ExactMasterCoefficient(master_powers={:?}, unit_mass_coefficient={:?}, common_mass_squared_power={})",
            self.master_powers, self.unit_mass_coefficient, self.common_mass_squared_power
        )
    }
}

#[pyclass(frozen, module = "rustred", name = "ClosingArtifactReductionResult")]
#[derive(Debug)]
pub struct PyClosingArtifactReductionResult {
    schema: &'static str,
    status: &'static str,
    canonical_toml: String,
    family_fingerprint: String,
    target_powers: Vec<i64>,
    terms: Vec<PyExactMasterCoefficient>,
}

#[pymethods]
impl PyClosingArtifactReductionResult {
    #[getter]
    fn schema(&self) -> &'static str {
        self.schema
    }

    #[getter]
    fn status(&self) -> &'static str {
        self.status
    }

    #[getter]
    fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    #[getter]
    fn target_powers(&self) -> Vec<i64> {
        self.target_powers.clone()
    }

    #[getter]
    fn terms(&self) -> Vec<PyExactMasterCoefficient> {
        self.terms.clone()
    }

    fn to_toml(&self) -> &str {
        &self.canonical_toml
    }

    fn __repr__(&self) -> String {
        format!(
            "ClosingArtifactReductionResult(schema={:?}, status={:?}, target_powers={:?}, terms={})",
            self.schema,
            self.status,
            self.target_powers,
            self.terms.len()
        )
    }
}

#[pyfunction]
#[pyo3(
    signature = (source, *, input_format = "auto", relations = "all", n_cores = PythonInteger(1)),
    text_signature = "(source, *, input_format='auto', relations='all', n_cores=1)"
)]
fn derive(
    py: Python<'_>,
    source: &str,
    input_format: &str,
    relations: &str,
    n_cores: PythonInteger,
) -> PyResult<PyDeriveResult> {
    let request = DeriveRequest {
        source: bounded_owned_input("derive input", source)?,
        input_format: parse_input_format(input_format)?,
        relations: parse_relation_selection(relations)?,
        n_cores: positive_core_count("derive n_cores", n_cores.0)?,
    };
    let result = py
        .detach(move || execute(move || app_derive(request)))
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyDeriveResult::new(
        result.schema(),
        result.status(),
        result.into_toml(),
    ))
}

#[pyfunction]
#[pyo3(
    signature = (source, *, input_format = "auto", root_id = None),
    text_signature = "(source, *, input_format='auto', root_id=None)"
)]
fn campaign_plan(
    py: Python<'_>,
    source: &str,
    input_format: &str,
    root_id: Option<&str>,
) -> PyResult<PyCampaignPlanResult> {
    let request = CampaignPlanRequest {
        source: bounded_owned_input("campaign input", source)?,
        input_format: parse_input_format(input_format)?,
        // Root identifiers have their own application/core validation and
        // error category. The adapter must not apply the source-payload limit
        // or invent a competing frontend rule.
        root_id: root_id.map(str::to_owned),
    };
    let result = py
        .detach(move || execute(move || app_campaign_plan(request)))
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyCampaignPlanResult::new(
        result.schema(),
        result.status(),
        result.into_toml(),
    ))
}

#[pyfunction]
#[pyo3(
    signature = (profile, *, n_cores = PythonInteger(1), max_memory_bytes),
    text_signature = "(profile, *, n_cores=1, max_memory_bytes)"
)]
fn campaign_preflight(
    py: Python<'_>,
    profile: &str,
    n_cores: PythonInteger,
    max_memory_bytes: PythonInteger,
) -> PyResult<PyCampaignPreflightResult> {
    let request = CampaignPreflightRequest {
        profile: bounded_owned_input("campaign execution resource profile", profile)?,
        n_cores: positive_core_count("campaign preflight n_cores", n_cores.0)?,
        max_memory_bytes: positive_memory_bytes(max_memory_bytes.0)?,
    };
    let result = py
        .detach(move || execute(move || app_campaign_preflight(request)))
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyCampaignPreflightResult::new(
        result.schema(),
        result.status(),
        result.into_toml(),
    ))
}

#[pyfunction]
#[pyo3(signature = (config), text_signature = "(config)")]
fn run_foundry_campaign(py: Python<'_>, config: &str) -> PyResult<PyFoundryCampaignRunResult> {
    let request = FoundryCampaignRunRequest {
        config: bounded_owned_input("foundry campaign configuration", config)?,
    };
    let result = py
        .detach(move || execute(move || app_foundry_campaign_run(request)))
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyFoundryCampaignRunResult {
        schema: result.schema(),
        measurements_schema: result.measurements_schema(),
        report_toml: result.to_toml().to_owned(),
        measurements_toml: result.measurements_to_toml().to_owned(),
    })
}

#[pyfunction]
#[pyo3(
    signature = (config, *, n_cores = PythonInteger(1)),
    text_signature = "(config, *, n_cores=1)"
)]
fn run_foundry_wave_campaign(
    py: Python<'_>,
    config: &str,
    n_cores: PythonInteger,
) -> PyResult<PyFoundryWaveCampaignRunResult> {
    let request = FoundryWaveCampaignRunRequest {
        config: bounded_owned_input("foundry wave campaign configuration", config)?,
        sibling_worker_count: positive_core_count("foundry wave campaign n_cores", n_cores.0)?,
    };
    let result = py
        .detach(move || execute(move || app_foundry_wave_campaign_run(request)))
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyFoundryWaveCampaignRunResult {
        schema: result.schema(),
        measurements_schema: result.measurements_schema(),
        report_toml: result.to_toml().to_owned(),
        measurements_toml: result.measurements_to_toml().to_owned(),
        artifact: result.artifact_bytes().map(<[u8]>::to_vec),
    })
}

#[pyfunction]
#[pyo3(
    signature = (*, family = "unit-mass-vacuum-k1"),
    text_signature = "(*, family='unit-mass-vacuum-k1')"
)]
fn generate_closing_artifact(
    py: Python<'_>,
    family: &str,
) -> PyResult<PyClosingArtifactGenerationResult> {
    let family = family
        .parse::<ClosingFamilySelector>()
        .map_err(|error| RustRedInputError::new_err(error.to_string()))?;
    let result = py
        .detach(move || {
            execute(move || {
                app_closing_artifact_generate(ClosingArtifactGenerateRequest { family })
            })
        })
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyClosingArtifactGenerationResult {
        schema: result.schema(),
        status: result.status(),
        canonical_toml: result.to_toml().to_owned(),
        artifact: PyBytes::new(py, result.artifact()).unbind(),
    })
}

#[pyfunction]
#[pyo3(signature = (artifact), text_signature = "(artifact)")]
fn inspect_closing_artifact(
    py: Python<'_>,
    artifact: &Bound<'_, PyBytes>,
) -> PyResult<PyClosingArtifactInspectionResult> {
    let artifact = bounded_artifact_bytes(artifact)?;
    let result = py
        .detach(move || {
            execute(move || {
                app_closing_artifact_inspect(ClosingArtifactInspectRequest { artifact })
            })
        })
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    Ok(PyClosingArtifactInspectionResult::new(
        result.schema(),
        result.status(),
        result.into_toml(),
    ))
}

#[pyfunction]
#[pyo3(
    signature = (artifact, target_powers, *, max_rule_applications = PythonInteger(1_000_000)),
    text_signature = "(artifact, target_powers, *, max_rule_applications=1000000)"
)]
fn reduce_with_closing_artifact(
    py: Python<'_>,
    artifact: &Bound<'_, PyBytes>,
    target_powers: Vec<PythonInteger>,
    max_rule_applications: PythonInteger,
) -> PyResult<PyClosingArtifactReductionResult> {
    let artifact = bounded_artifact_bytes(artifact)?;
    let target_powers = target_powers
        .into_iter()
        .enumerate()
        .map(|(position, power)| {
            i64::try_from(power.0).map_err(|_| {
                RustRedInputError::new_err(format!(
                    "target_powers[{position}] must fit a signed 64-bit integer"
                ))
            })
        })
        .collect::<PyResult<Vec<_>>>()?;
    let max_rule_applications = nonnegative_usize(
        "closing-artifact max_rule_applications",
        max_rule_applications.0,
    )?;
    let result = py
        .detach(move || {
            execute(move || {
                app_closing_artifact_reduce(ClosingArtifactReduceRequest {
                    artifact,
                    target_powers,
                    max_rule_applications,
                })
            })
        })
        .map_err(map_coordinator_error)?;
    let result = result.map_err(map_app_error)?;
    let terms = result
        .terms()
        .iter()
        .map(|term| PyExactMasterCoefficient {
            master_powers: term.master_powers().to_vec(),
            unit_mass_coefficient: term.unit_mass_coefficient().to_owned(),
            common_mass_squared_power: term.common_mass_squared_power(),
        })
        .collect();
    Ok(PyClosingArtifactReductionResult {
        schema: result.schema(),
        status: result.status(),
        canonical_toml: result.to_toml().to_owned(),
        family_fingerprint: result.family_fingerprint().to_owned(),
        target_powers: result.target_powers().to_vec(),
        terms,
    })
}

fn execute<T, F>(operation: F) -> Result<T, CoordinatorError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    // Module initialization establishes the process coordinator before any
    // binding can reach here. If initialization ever failed, the module did
    // not load and no Python call is possible.
    process_coordinator()
        .map_err(CoordinatorError::Unavailable)?
        .execute(operation)
}

fn parse_input_format(value: &str) -> PyResult<InputFormat> {
    InputFormat::from_str(value).map_err(|error| RustRedInputError::new_err(error.to_string()))
}

fn parse_relation_selection(value: &str) -> PyResult<RelationSelection> {
    RelationSelection::from_str(value)
        .map_err(|error| RustRedInputError::new_err(error.to_string()))
}

fn positive_core_count(label: &str, value: i128) -> PyResult<usize> {
    if value <= 0 {
        return Err(RustRedInputError::new_err(format!(
            "{label} must be a positive integer"
        )));
    }
    usize::try_from(value).map_err(|_| {
        RustRedInputError::new_err(format!(
            "{label} must be a positive integer fitting this platform"
        ))
    })
}

fn nonnegative_usize(label: &str, value: i128) -> PyResult<usize> {
    if value < 0 {
        return Err(RustRedInputError::new_err(format!(
            "{label} must be a nonnegative integer"
        )));
    }
    usize::try_from(value).map_err(|_| {
        RustRedInputError::new_err(format!(
            "{label} must be a nonnegative integer fitting this platform"
        ))
    })
}

fn positive_memory_bytes(value: i128) -> PyResult<u64> {
    const LABEL: &str = "campaign preflight max_memory_bytes";
    if value <= 0 {
        return Err(RustRedInputError::new_err(format!(
            "{LABEL} must be positive"
        )));
    }
    u64::try_from(value).map_err(|_| {
        RustRedInputError::new_err(format!("{LABEL} must fit an unsigned 64-bit integer"))
    })
}

fn bounded_owned_input(label: &str, source: &str) -> PyResult<String> {
    if source.len() > rustred_app::MAX_INPUT_BYTES {
        return Err(RustRedLimitError::new_err(format!(
            "{label} has {} bytes, exceeding the {}-byte application limit",
            source.len(),
            rustred_app::MAX_INPUT_BYTES
        )));
    }
    Ok(source.to_owned())
}

fn bounded_artifact_bytes(artifact: &Bound<'_, PyBytes>) -> PyResult<Vec<u8>> {
    let bytes = artifact.as_bytes();
    if bytes.len() > rustred_app::MAX_CLOSING_ARTIFACT_BYTES {
        return Err(RustRedLimitError::new_err(format!(
            "closing artifact has {} bytes, exceeding the {}-byte application limit",
            bytes.len(),
            rustred_app::MAX_CLOSING_ARTIFACT_BYTES
        )));
    }
    Ok(bytes.to_vec())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PythonExceptionKind {
    Input,
    Schema,
    Limit,
    Lowering,
    Derivation,
    Execution,
    License,
    Serialization,
    OutputLimit,
    Internal,
}

fn python_exception_kind(kind: AppErrorKind) -> PythonExceptionKind {
    match kind {
        AppErrorKind::Input => PythonExceptionKind::Input,
        AppErrorKind::Schema => PythonExceptionKind::Schema,
        AppErrorKind::Limit => PythonExceptionKind::Limit,
        AppErrorKind::Lowering => PythonExceptionKind::Lowering,
        AppErrorKind::Derivation => PythonExceptionKind::Derivation,
        AppErrorKind::Execution => PythonExceptionKind::Execution,
        AppErrorKind::License => PythonExceptionKind::License,
        AppErrorKind::Serialization => PythonExceptionKind::Serialization,
        AppErrorKind::OutputLimit => PythonExceptionKind::OutputLimit,
        AppErrorKind::InternalInvariant => PythonExceptionKind::Internal,
        // AppErrorKind is non-exhaustive across the crate boundary. Unknown
        // future kinds fail closed as internal errors until this adapter adds
        // an explicit public mapping and regression case.
        _ => PythonExceptionKind::Internal,
    }
}

fn map_app_error(error: AppError) -> PyErr {
    let kind = python_exception_kind(error.kind());
    let message = error.into_message();
    match kind {
        PythonExceptionKind::Input => RustRedInputError::new_err(message),
        PythonExceptionKind::Schema => RustRedSchemaError::new_err(message),
        PythonExceptionKind::Limit => RustRedLimitError::new_err(message),
        PythonExceptionKind::Lowering => RustRedLoweringError::new_err(message),
        PythonExceptionKind::Derivation => RustRedDerivationError::new_err(message),
        PythonExceptionKind::Execution => RustRedExecutionError::new_err(message),
        PythonExceptionKind::License => RustRedLicenseError::new_err(message),
        PythonExceptionKind::Serialization => RustRedSerializationError::new_err(message),
        PythonExceptionKind::OutputLimit => RustRedOutputLimitError::new_err(message),
        PythonExceptionKind::Internal => RustRedInternalError::new_err(message),
    }
}

fn map_coordinator_error(error: CoordinatorError) -> PyErr {
    match error {
        CoordinatorError::Poisoned => RustRedCoordinatorPoisonedError::new_err(
            "the RustRed Python coordinator is permanently poisoned after an internal panic",
        ),
        CoordinatorError::Forked {
            creator_pid,
            current_pid,
        } => RustRedCoordinatorPoisonedError::new_err(format!(
            "the RustRed Python coordinator was created in process {creator_pid} and cannot be reused after fork in process {current_pid}"
        )),
        CoordinatorError::Panicked(message) => RustRedInternalError::new_err(format!(
            "RustRed caught an internal panic and permanently poisoned the Python coordinator: {message}"
        )),
        CoordinatorError::Unavailable(message) => RustRedCoordinatorPoisonedError::new_err(message),
    }
}

#[pymodule(gil_used = true)]
fn _rustred(module: &Bound<'_, PyModule>) -> PyResult<()> {
    // Starting this thread during module initialization establishes it before
    // any binding can call the core or initialize Symbolica.
    process_coordinator().map_err(RustRedInternalError::new_err)?;

    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add("RustRedError", module.py().get_type::<RustRedError>())?;
    module.add(
        "RustRedInputError",
        module.py().get_type::<RustRedInputError>(),
    )?;
    module.add(
        "RustRedSchemaError",
        module.py().get_type::<RustRedSchemaError>(),
    )?;
    module.add(
        "RustRedLimitError",
        module.py().get_type::<RustRedLimitError>(),
    )?;
    module.add(
        "RustRedLoweringError",
        module.py().get_type::<RustRedLoweringError>(),
    )?;
    module.add(
        "RustRedDerivationError",
        module.py().get_type::<RustRedDerivationError>(),
    )?;
    module.add(
        "RustRedExecutionError",
        module.py().get_type::<RustRedExecutionError>(),
    )?;
    module.add(
        "RustRedLicenseError",
        module.py().get_type::<RustRedLicenseError>(),
    )?;
    module.add(
        "RustRedSerializationError",
        module.py().get_type::<RustRedSerializationError>(),
    )?;
    module.add(
        "RustRedOutputLimitError",
        module.py().get_type::<RustRedOutputLimitError>(),
    )?;
    module.add(
        "RustRedInternalError",
        module.py().get_type::<RustRedInternalError>(),
    )?;
    module.add(
        "RustRedCoordinatorPoisonedError",
        module.py().get_type::<RustRedCoordinatorPoisonedError>(),
    )?;
    module.add_class::<PyDeriveResult>()?;
    module.add_class::<PyCampaignPlanResult>()?;
    module.add_class::<PyCampaignPreflightResult>()?;
    module.add_class::<PyFoundryCampaignRunResult>()?;
    module.add_class::<PyFoundryWaveCampaignRunResult>()?;
    module.add_class::<PyClosingArtifactGenerationResult>()?;
    module.add_class::<PyClosingArtifactInspectionResult>()?;
    module.add_class::<PyExactMasterCoefficient>()?;
    module.add_class::<PyClosingArtifactReductionResult>()?;
    module.add_function(wrap_pyfunction!(derive, module)?)?;
    module.add_function(wrap_pyfunction!(campaign_plan, module)?)?;
    module.add_function(wrap_pyfunction!(campaign_preflight, module)?)?;
    module.add_function(wrap_pyfunction!(run_foundry_campaign, module)?)?;
    module.add_function(wrap_pyfunction!(run_foundry_wave_campaign, module)?)?;
    module.add_function(wrap_pyfunction!(generate_closing_artifact, module)?)?;
    module.add_function(wrap_pyfunction!(inspect_closing_artifact, module)?)?;
    module.add_function(wrap_pyfunction!(reduce_with_closing_artifact, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_values_are_validated_before_coordinator_work() {
        assert_eq!(
            parse_input_format("symbolica").expect("input format"),
            InputFormat::Symbolica
        );
        assert_eq!(
            parse_relation_selection("li").expect("relation selection"),
            RelationSelection::LorentzInvariance
        );
        assert!(parse_input_format("json").is_err());
        assert!(parse_relation_selection("laporta").is_err());
        assert!(positive_core_count("n_cores", 0).is_err());
        assert!(positive_core_count("n_cores", -1).is_err());
        assert_eq!(
            positive_core_count("n_cores", 4).expect("positive count"),
            4
        );
    }

    #[test]
    fn every_current_application_error_kind_has_an_explicit_python_mapping() {
        let cases = [
            (AppErrorKind::Input, PythonExceptionKind::Input),
            (AppErrorKind::Schema, PythonExceptionKind::Schema),
            (AppErrorKind::Limit, PythonExceptionKind::Limit),
            (AppErrorKind::Lowering, PythonExceptionKind::Lowering),
            (AppErrorKind::Derivation, PythonExceptionKind::Derivation),
            (AppErrorKind::Execution, PythonExceptionKind::Execution),
            (AppErrorKind::License, PythonExceptionKind::License),
            (
                AppErrorKind::Serialization,
                PythonExceptionKind::Serialization,
            ),
            (AppErrorKind::OutputLimit, PythonExceptionKind::OutputLimit),
            (
                AppErrorKind::InternalInvariant,
                PythonExceptionKind::Internal,
            ),
        ];
        for (application, python) in cases {
            assert_eq!(python_exception_kind(application), python);
        }
    }
}
