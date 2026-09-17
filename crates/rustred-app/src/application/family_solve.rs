//! Topology-neutral family solve diagnostic.
//!
//! This is deliberately not an artifact publisher.  It is the application
//! boundary for driving the generic core solver from an arbitrary family
//! project, which is useful while closure and affine-domain publication are
//! still evolving.  No family name is interpreted here.

use std::time::Duration;

use serde::Serialize;

use rustred::solver::{SectorConfig, SectorSolveOptions, solve_family};

use super::error::AppError;
use super::input::prepare_input;
use super::lowering::lower_project;
use super::{InputFormat, MAX_INPUT_BYTES};

pub const FAMILY_SOLVE_SCHEMA: &str = "rustred.family-solve-output.toml.v1";
const MAX_ENUMERATED_SECTORS: usize = 1 << 16;

/// Request a diagnostic solve for a family supplied through the normal input
/// grammar.  If `sectors` is `None`, all sectors are enumerated only when the
/// family arity is at most 16; larger families must provide an explicit
/// manifest to avoid accidental exponential allocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilySolveRequest {
    pub source: String,
    pub input_format: InputFormat,
    pub sectors: Option<Vec<Vec<bool>>>,
    pub n_cores: usize,
}

impl FamilySolveRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            input_format: InputFormat::Auto,
            sectors: None,
            n_cores: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct Output<'a> {
    schema: &'static str,
    status: &'static str,
    family_name: &'a str,
    arity: usize,
    sectors: usize,
    solved_sectors: usize,
    rules: usize,
    finite_residuals: usize,
    workers: usize,
    elapsed_us: u128,
}

/// Result of a topology-neutral family diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilySolveResult {
    canonical_toml: String,
    pub family_name: String,
    pub arity: usize,
    pub sectors: usize,
    pub solved_sectors: usize,
    pub rules: usize,
    pub finite_residuals: usize,
    pub workers: usize,
    pub elapsed: Duration,
}

impl FamilySolveResult {
    pub fn as_toml(&self) -> &str {
        &self.canonical_toml
    }
    pub fn into_toml(self) -> String {
        self.canonical_toml
    }
}

pub fn family_solve(request: FamilySolveRequest) -> Result<FamilySolveResult, AppError> {
    if request.source.len() > MAX_INPUT_BYTES {
        return Err(AppError::limit(format!(
            "family solve input has {} bytes, exceeding the application ceiling {}",
            request.source.len(),
            MAX_INPUT_BYTES
        )));
    }
    if request.n_cores == 0 {
        return Err(AppError::input("family solve n_cores must be positive"));
    }
    let prepared = prepare_input(&request.source, request.input_format)?;
    let lowered = lower_project(prepared)?;
    let (_form, _schema, _metadata, lowered) = lowered.into_parts();
    let family = lowered.into_family();
    let arity = family.denominator_count();
    let sectors = match request.sectors {
        Some(sectors) => sectors,
        None => enumerate_sectors(arity)?,
    };
    if sectors.iter().any(|sector| sector.len() != arity) {
        return Err(AppError::input(format!(
            "family solve sector manifest contains a sector with the wrong arity; expected {arity}"
        )));
    }
    let summary = dispatch_solve(arity, &family, &sectors, request.n_cores)?;
    let output = Output {
        schema: FAMILY_SOLVE_SCHEMA,
        status: "diagnostic",
        family_name: family.name(),
        arity,
        sectors: summary.0,
        solved_sectors: summary.1,
        rules: summary.2,
        finite_residuals: summary.3,
        workers: request.n_cores,
        elapsed_us: summary.4.as_micros(),
    };
    let canonical_toml = toml::to_string_pretty(&output).map_err(|error| {
        AppError::execution(format!("cannot serialize family solve output: {error}"))
    })?;
    Ok(FamilySolveResult {
        canonical_toml,
        family_name: family.name().to_owned(),
        arity,
        sectors: summary.0,
        solved_sectors: summary.1,
        rules: summary.2,
        finite_residuals: summary.3,
        workers: request.n_cores,
        elapsed: summary.4,
    })
}

fn enumerate_sectors(arity: usize) -> Result<Vec<Vec<bool>>, AppError> {
    if arity >= usize::BITS as usize || (1usize << arity) > MAX_ENUMERATED_SECTORS {
        return Err(AppError::input(format!(
            "family arity {arity} requires an explicit sector manifest (automatic enumeration is capped at {MAX_ENUMERATED_SECTORS} sectors)"
        )));
    }
    Ok((0..(1usize << arity))
        .map(|bits| (0..arity).map(|axis| bits & (1 << axis) != 0).collect())
        .collect())
}

fn dispatch_solve(
    arity: usize,
    family: &rustred::family::IntegralFamily,
    sectors: &[Vec<bool>],
    workers: usize,
) -> Result<(usize, usize, usize, usize, Duration), AppError> {
    macro_rules! arms {
        ($($n:literal),+ $(,)?) => {
            match arity {
                $(
                $n => {
                let typed: Vec<[bool; $n]> = sectors
                    .iter()
                    .map(|sector| <[bool; $n]>::try_from(sector.as_slice()).expect("validated sector arity"))
                    .collect();
                let summary = solve_family::<$n>(family, &typed, workers, SectorConfig::default(), SectorSolveOptions::default())
                    .map_err(|error| AppError::execution(error.to_string()))?;
                Ok((summary.sectors, summary.solved_sectors, summary.rules, summary.finite_residuals, summary.elapsed))
                }
                ),+
                _ => Err(AppError::input(format!("family arity {arity} exceeds the diagnostic dispatch limit 16"))),
            }
        };
    }
    arms!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_LOOP: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "user_supplied_tadpole"
loop_momenta = ["k1"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "D1"
expression = "k1^2-m"
[target]
powers = [1]
numerator = "1"
"#;

    #[test]
    fn arbitrary_project_toml_reaches_generic_family_solver() {
        let result = family_solve(FamilySolveRequest::new(ONE_LOOP)).unwrap();
        assert_eq!(result.family_name, "user_supplied_tadpole");
        assert_eq!(result.arity, 1);
        assert_eq!(result.sectors, 2);
        assert!(
            result
                .as_toml()
                .contains("rustred.family-solve-output.toml.v1")
        );
    }

    #[test]
    fn explicit_sector_manifest_must_match_family_arity() {
        let mut request = FamilySolveRequest::new(ONE_LOOP);
        request.sectors = Some(vec![vec![true, false]]);
        let error = family_solve(request).unwrap_err();
        assert!(error.to_string().contains("wrong arity"));
    }
}
