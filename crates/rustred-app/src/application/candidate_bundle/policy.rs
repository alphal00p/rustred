//! Versioned generation metadata, never rule or closure authority.
//!
//! The original active policy fixes finite-case depth to two. Explicit depth
//! policies share its source definitions and every other search default. Their
//! canonical tags fit the existing native record without rewriting old output.

use crate::AppError;

use super::model::{FamilyCandidatesRequest, FiniteCaseLimits, FiniteCasePolicy, SOLVER_POLICY};

const PREFIX: &str = "ordinary-source-port-numerical-depth-";
const SUFFIX: &str = "-v1";
const DEFAULT_DEPTH: u32 = 2;
const RANK_SEPARATOR: &str = "-max-numerator-rank-";
const RETAIN_SEPARATOR: &str = "-retain-rank-finite-points-";

pub(super) fn validate_request_scope(request: &FamilyCandidatesRequest) -> Result<(), AppError> {
    if request.finite_case_policy == FiniteCasePolicy::RetainRankFinite {
        if request.max_numerator_rank.is_none() {
            return Err(AppError::input(
                "retain-rank-finite requires max_numerator_rank",
            ));
        }
        if request.finite_case_limits.max_visited_points == 0
            || request.finite_case_limits.max_retained_terminals == 0
        {
            return Err(AppError::input("finite retention limits must be positive"));
        }
    } else if request.finite_case_limits != FiniteCaseLimits::default() {
        return Err(AppError::input(
            "finite retention limits require retain-rank-finite",
        ));
    }
    Ok(())
}

/// Generation scope and finite-case policy, including its work limits.
/// Not a certificate. Transport/output and exact case-intersection work
/// budgets are deliberately excluded: a completed exact shard stays reusable
/// under different resource allowances. Intersection limits may change
/// conservative coverage scheduling, not the mathematical saved scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GenerationPolicy {
    pub numerical_depth: u32,
    pub max_numerator_rank: Option<u32>,
    pub finite_case_policy: FiniteCasePolicy,
    pub finite_case_limits: FiniteCaseLimits,
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

pub(super) fn encode_request(request: &FamilyCandidatesRequest) -> String {
    encode_configuration(GenerationPolicy {
        numerical_depth: request.numerical_depth,
        max_numerator_rank: request.max_numerator_rank,
        finite_case_policy: request.finite_case_policy,
        finite_case_limits: request.finite_case_limits,
    })
}

fn encode_configuration(policy: GenerationPolicy) -> String {
    let base = encode_scoped(policy.numerical_depth, policy.max_numerator_rank);
    match policy.finite_case_policy {
        FiniteCasePolicy::SearchFinite => base,
        FiniteCasePolicy::RetainRankFinite => format!(
            "{}{RETAIN_SEPARATOR}{}-terminals-{}{SUFFIX}",
            base.strip_suffix(SUFFIX).expect("canonical generation tag"),
            policy.finite_case_limits.max_visited_points,
            policy.finite_case_limits.max_retained_terminals,
        ),
    }
}

pub(super) fn parse(policy: &str) -> Result<GenerationPolicy, AppError> {
    if policy == SOLVER_POLICY {
        return Ok(GenerationPolicy {
            numerical_depth: DEFAULT_DEPTH,
            max_numerator_rank: None,
            finite_case_policy: FiniteCasePolicy::SearchFinite,
            finite_case_limits: FiniteCaseLimits::default(),
        });
    }
    let values = policy
        .strip_prefix(PREFIX)
        .and_then(|value| value.strip_suffix(SUFFIX))
        .ok_or_else(|| AppError::schema("unsupported candidate solver policy"))?;
    let (values, finite_case_policy, finite_case_limits) = match values.split_once(RETAIN_SEPARATOR)
    {
        Some((values, limits)) => {
            let (points, terminals) = limits
                .split_once("-terminals-")
                .ok_or_else(|| AppError::schema("invalid finite retention limits"))?;
            let limits = FiniteCaseLimits {
                max_visited_points: points
                    .parse::<usize>()
                    .map_err(|_| AppError::schema("invalid finite point limit"))?,
                max_retained_terminals: terminals
                    .parse::<usize>()
                    .map_err(|_| AppError::schema("invalid finite terminal limit"))?,
            };
            if limits.max_visited_points == 0 || limits.max_retained_terminals == 0 {
                return Err(AppError::schema("finite retention limits must be positive"));
            }
            (values, FiniteCasePolicy::RetainRankFinite, limits)
        }
        None => (
            values,
            FiniteCasePolicy::SearchFinite,
            FiniteCaseLimits::default(),
        ),
    };
    let parsed = match values.split_once(RANK_SEPARATOR) {
        Some((depth, rank)) => depth.parse::<u32>().ok().zip(rank.parse::<u32>().ok()).map(
            |(numerical_depth, rank)| GenerationPolicy {
                numerical_depth,
                max_numerator_rank: Some(rank),
                finite_case_policy,
                finite_case_limits,
            },
        ),
        None => values
            .parse::<u32>()
            .ok()
            .map(|numerical_depth| GenerationPolicy {
                numerical_depth,
                max_numerator_rank: None,
                finite_case_policy,
                finite_case_limits,
            }),
    }
    .filter(|parsed| {
        parsed.finite_case_policy == FiniteCasePolicy::SearchFinite
            || parsed.max_numerator_rank.is_some()
    })
    .filter(|parsed| encode_configuration(*parsed) == policy)
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
    fn finite_retention_tags_bind_scope_policy_and_work_limits() {
        for rank in [0, 10, 20] {
            let mut request = FamilyCandidatesRequest::new("not parsed");
            request.max_numerator_rank = Some(rank);
            request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
            request.finite_case_limits = FiniteCaseLimits {
                max_visited_points: 101,
                max_retained_terminals: 17,
            };
            let tag = encode_request(&request);
            let decoded = parse(&tag).unwrap();
            assert_eq!(decoded.max_numerator_rank, Some(rank));
            assert_eq!(decoded.finite_case_policy, request.finite_case_policy);
            assert_eq!(decoded.finite_case_limits, request.finite_case_limits);
            assert_eq!(encode_configuration(decoded), tag);
            for malformed in [
                tag.replace("points-101", "points-0101"),
                tag.replace("points-101", "points-0"),
                tag.replace("terminals-17", "terminals-0"),
                tag.replace("terminals-17", "terminals-+17"),
                tag.replace(&format!("-max-numerator-rank-{rank}"), ""),
                tag.replace("retain-rank-finite", "retain-everything"),
            ] {
                assert!(parse(&malformed).is_err(), "{malformed}");
            }
        }
    }

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
                        finite_case_policy: FiniteCasePolicy::SearchFinite,
                        finite_case_limits: FiniteCaseLimits::default(),
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
