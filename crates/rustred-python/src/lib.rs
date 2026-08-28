mod coordinator;

use std::str::FromStr;

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBool};
use rustred_app::{
    AppError, AppErrorKind, CampaignPlanRequest, CampaignPreflightRequest, DeriveRequest,
    InputFormat, RelationSelection, campaign_plan as app_campaign_plan,
    campaign_preflight as app_campaign_preflight, derive as app_derive,
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
    module.add_function(wrap_pyfunction!(derive, module)?)?;
    module.add_function(wrap_pyfunction!(campaign_plan, module)?)?;
    module.add_function(wrap_pyfunction!(campaign_preflight, module)?)?;
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
