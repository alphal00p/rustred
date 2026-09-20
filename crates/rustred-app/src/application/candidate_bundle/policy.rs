//! Versioned generation metadata, never rule or closure authority.
//!
//! The original active policy fixes finite-case depth to two. Explicit depth
//! policies share its source definitions and every other search default. Their
//! canonical tags fit the existing native record without rewriting old output.

use crate::AppError;

use super::model::SOLVER_POLICY;

const PREFIX: &str = "ordinary-source-port-numerical-depth-";
const SUFFIX: &str = "-v1";
const DEFAULT_DEPTH: u32 = 2;

pub(super) fn encode(depth: u32) -> String {
    if depth == DEFAULT_DEPTH {
        SOLVER_POLICY.into()
    } else {
        format!("{PREFIX}{depth}{SUFFIX}")
    }
}

pub(super) fn numerical_depth(policy: &str) -> Result<u32, AppError> {
    if policy == SOLVER_POLICY {
        return Ok(DEFAULT_DEPTH);
    }
    let depth = policy
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|&depth| encode(depth) == policy)
        .ok_or_else(|| AppError::schema("unsupported candidate solver policy"))?;
    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_search_policies_are_canonical_and_preserve_current_defaults() {
        assert_eq!(
            rustred::solver::SectorSolveOptions::default().numerical_depth,
            DEFAULT_DEPTH
        );
        for depth in [0, 1, 2, 3, u32::MAX] {
            assert_eq!(numerical_depth(&encode(depth)).unwrap(), depth);
        }
        assert_eq!(encode(2), SOLVER_POLICY);
        for policy in [
            "",
            "ordinary-source-port-default-v2",
            "ordinary-source-port-numerical-depth-2-v1",
            "ordinary-source-port-numerical-depth-00-v1",
            "ordinary-source-port-numerical-depth-+1-v1",
            "ordinary-source-port-numerical-depth--1-v1",
            "ordinary-source-port-numerical-depth-4294967296-v1",
            "ordinary-source-port-numerical-depth-0-v1-trailing",
        ] {
            assert!(numerical_depth(policy).is_err(), "{policy}");
        }
    }
}
