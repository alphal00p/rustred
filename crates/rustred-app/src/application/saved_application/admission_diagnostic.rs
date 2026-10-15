//! Input-controlled admission/verified transport only. No IBP application.
use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    powers: Vec<i64>,
    admitted: bool,
    route_is_zero: Option<bool>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    preparation_workers: usize,
    cases: Vec<Case>,
}
impl Request {
    fn validate_arity(&self, arity: usize) -> Result<(), AppError> {
        if arity == 0 || self.cases.iter().any(|c| c.powers.len() != arity) {
            return Err(AppError::input("entry diagnostic physical arity differs"));
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), AppError> {
        if self.preparation_workers == 0 || self.cases.is_empty() || self.cases.len() > 64 {
            return Err(AppError::input(
                "entry diagnostic needs positive workers and1..64 cases",
            ));
        }
        let mut ids = BTreeSet::new();
        for case in &self.cases {
            if case.id.is_empty()
                || case.id.len() > 128
                || !ids.insert(&case.id)
                || (!case.admitted && case.route_is_zero.is_some())
            {
                return Err(AppError::input(
                    "entry diagnostic IDs/route expectation malformed",
                ));
            }
        }
        Ok(())
    }
}
fn probe<const N: usize>(package: &Path, request: &Request) -> Result<Value, AppError> {
    let started = Instant::now();
    let app = load_saved_application::<N>(
        package,
        SavedApplicationOptions {
            preparation_workers: request.preparation_workers,
            ..Default::default()
        },
    )?;
    let load_seconds = started.elapsed().as_secs_f64();
    println!("saved_entry_package_load_seconds={load_seconds:.6}");
    let before = app.cache_statistics().rule_applications();
    let mut cases = Vec::new();
    for case in &request.cases {
        let started = Instant::now();
        if case.powers.len() != app.family().denominator_count() {
            return Err(AppError::input("entry diagnostic physical arity differs"));
        }
        let key = IntegralKey::try_new(case.powers.iter().copied()).map_err(failure)?;
        let admitted = app.routed_entry_is_admitted(&key)?;
        if admitted != case.admitted {
            return Err(AppError::execution(format!(
                "entry admission expectation differs for {}",
                case.id
            )));
        }
        let route = if admitted {
            let routed = app.route_entry_to_owner(&key)?;
            let zero = routed.terms().is_empty();
            if case.route_is_zero.is_some_and(|expected| expected != zero) {
                return Err(AppError::execution(format!(
                    "exact entry route zero expectation differs for {}",
                    case.id
                )));
            }
            Some(
                json!({"is_zero":zero,"terms":routed.terms().iter().map(|(key,coefficient)|
                json!({"powers":key.powers(),"coefficient":coefficient.to_expression().to_string()})).collect::<Vec<_>>(),
                "nonzero_conditions":routed.nonzero_conditions().iter().map(|c|c.to_expression().to_string()).collect::<Vec<_>>()}),
            )
        } else {
            None
        };
        let result = json!({"id":case.id,"powers":case.powers,"admitted":admitted,
            "route":route,"seconds":started.elapsed().as_secs_f64()});
        println!("saved_entry_case={result}");
        cases.push(result);
    }
    let after = app.cache_statistics().rule_applications();
    if before != after {
        return Err(AppError::internal_invariant(
            "entry-only probe applied a saved IBP rule",
        ));
    }
    Ok(
        json!({"schema":"rustred.saved-entry-diagnostic.v1","complete":true,"cases":cases,
        "package_load_seconds":load_seconds,"storage_capacity":N,
        "rule_applications_before":before,"rule_applications_after":after,
        "new_ibps":false,"coefficient_reduction":false,
        "qualification":"Exact saved entry/zero/verified-transport checks only; rejected entries are not proofs of nonzero integrals or all-routing failure."}),
    )
}

#[test]
#[ignore = "manual input-controlled saved-package entry probe; external resource guard required"]
fn input_controlled_saved_entry_admission_without_ibps() -> Result<(), AppError> {
    if std::env::var("RUSTRED_RUN_SAVED_ENTRY_DIAGNOSTIC").as_deref() != Ok("1") {
        return Err(AppError::input(
            "explicit saved-entry diagnostic opt-in required",
        ));
    }
    let path = |name| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .ok_or_else(|| AppError::input(format!("explicit {name} required")))
    };
    let package = path("RUSTRED_SAVED_ENTRY_PACKAGE")?;
    let input = path("RUSTRED_SAVED_ENTRY_REQUEST")?;
    let output = path("RUSTRED_SAVED_ENTRY_OUTPUT")?;
    if output.exists() {
        return Err(AppError::input("entry diagnostic output must be fresh"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&input)
        .map_err(failure)?
        .take(1048577)
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() > 1048576 {
        return Err(AppError::limit("entry steering exceeds1MiB"));
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(failure)?;
    request.validate()?;
    let arity = saved_application_arity(&package)?;
    request.validate_arity(arity)?;
    crate::ensure_runtime_arity(arity)?;
    macro_rules! dispatch {($($n:literal),*)=>{match rustred::campaign_storage_arity(arity) {
        $($n=>probe::<$n>(&package,&request),)* _=>Err(AppError::input("entry diagnostic arity not compiled")),
    }}}
    let mut result = rustred::with_app_runtime_arities!(dispatch)?;
    result["request_blake3"] = json!(blake3::hash(&bytes).to_hex().to_string());
    let data = serde_json::to_vec(&result).map_err(failure)?;
    if data.len().checked_add(1).is_none_or(|n| n > 8 * 1048576) {
        return Err(AppError::limit("entry diagnostic output exceeds8MiB"));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)
        .map_err(failure)?;
    file.write_all(&data).map_err(failure)?;
    file.write_all(b"\n").map_err(failure)?;
    file.flush().map_err(failure)?;
    Ok(())
}

#[test]
fn saved_entry_diagnostic_strict_request_and_distinct_case_labels() {
    let valid = json!({"preparation_workers":1,"cases":[{"id":"negative","powers":[1],"admitted":false,"route_is_zero":null}]});
    let decoded = serde_json::from_value::<Request>(valid.clone()).unwrap();
    decoded.validate().unwrap();
    decoded.validate_arity(1).unwrap();
    assert!(decoded.validate_arity(2).is_err());
    let mut unknown = valid.clone();
    unknown["cases"][0]["assume_zero"] = json!(true);
    assert!(serde_json::from_value::<Request>(unknown).is_err());
    let mut contradiction = valid.clone();
    contradiction["cases"][0]["route_is_zero"] = json!(true);
    assert!(
        serde_json::from_value::<Request>(contradiction)
            .unwrap()
            .validate()
            .is_err()
    );
    let mut repeated = valid.clone();
    repeated["cases"]
        .as_array_mut()
        .unwrap()
        .push(valid["cases"][0].clone());
    assert!(
        serde_json::from_value::<Request>(repeated)
            .unwrap()
            .validate()
            .is_err()
    );
}
