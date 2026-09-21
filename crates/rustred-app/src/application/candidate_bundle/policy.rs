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
const RANK_SEPARATOR: &str = "-max-numerator-rank-";

/// Declared generation coverage, not a certificate or a resource budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GenerationPolicy {
    pub numerical_depth: u32,
    pub max_numerator_rank: Option<u32>,
}

pub(super) fn encode(depth: u32) -> String {
    if depth == DEFAULT_DEPTH {
        SOLVER_POLICY.into()
    } else {
        format!("{PREFIX}{depth}{SUFFIX}")
    }
}

pub(super) fn encode_scoped(depth: u32, max_numerator_rank: Option<u32>) -> String {
    match max_numerator_rank {
        Some(rank) => format!("{PREFIX}{depth}{RANK_SEPARATOR}{rank}{SUFFIX}"),
        None => encode(depth),
    }
}

pub(super) fn parse(policy: &str) -> Result<GenerationPolicy, AppError> {
    if policy == SOLVER_POLICY {
        return Ok(GenerationPolicy {
            numerical_depth: DEFAULT_DEPTH,
            max_numerator_rank: None,
        });
    }
    let values = policy
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
        .ok_or_else(|| AppError::schema("unsupported candidate solver policy"))?;
    let parsed = match values.split_once(RANK_SEPARATOR) {
        Some((depth, rank)) => depth.parse::<u32>().ok().zip(rank.parse::<u32>().ok()).map(
            |(numerical_depth, rank)| GenerationPolicy {
                numerical_depth,
                max_numerator_rank: Some(rank),
            },
        ),
        None => values
            .parse::<u32>()
            .ok()
            .map(|numerical_depth| GenerationPolicy {
                numerical_depth,
                max_numerator_rank: None,
            }),
    }
    .filter(|parsed| encode_scoped(parsed.numerical_depth, parsed.max_numerator_rank) == policy)
    .ok_or_else(|| AppError::schema("unsupported candidate solver policy"))?;
    Ok(parsed)
}

pub(super) fn numerical_depth(policy: &str) -> Result<u32, AppError> {
    Ok(parse(policy)?.numerical_depth)
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

    #[test]
    fn numerator_scopes_are_typed_canonical_and_distinct_from_search_depth() {
        for depth in [0, 2, u32::MAX] {
            for rank in [0, 10, 20, u32::MAX] {
                let tag = encode_scoped(depth, Some(rank));
                assert_eq!(
                    parse(&tag).unwrap(),
                    GenerationPolicy {
                        numerical_depth: depth,
                        max_numerator_rank: Some(rank),
                    }
                );
                assert_eq!(numerical_depth(&tag).unwrap(), depth);
            }
            assert_eq!(encode_scoped(depth, None), encode(depth));
            assert_eq!(parse(&encode(depth)).unwrap().max_numerator_rank, None);
        }
        for values in [
            "2-max-numerator-rank-",
            "2-max-numerator-rank-00",
            "02-max-numerator-rank-1",
            "2-max-numerator-rank-+1",
            "2-max-numerator-rank--1",
            "2-max-numerator-rank-4294967296",
            "2-max-numerator-rank-1-max-numerator-rank-2",
        ] {
            assert!(parse(&format!("{PREFIX}{values}{SUFFIX}")).is_err());
        }
    }
}
