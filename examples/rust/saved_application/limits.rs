//! Explicit benchmark resource overrides, not a second application schema.
use rustred_app::SavedApplicationOptions;
use std::path::Path;

pub(super) fn override_from_json(
    options: &mut SavedApplicationOptions,
    path: &Path,
) -> super::Result<()> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    override_from_value(options, &value)
}

fn override_from_value(
    options: &mut SavedApplicationOptions,
    value: &serde_json::Value,
) -> super::Result<()> {
    let object = value.as_object().ok_or("limits must be a JSON object")?;
    // Validate a complete copy before altering the caller's configuration.
    let mut candidate = *options;
    for (name, value) in object {
        if name == "cache_policy" {
            candidate.cache_policy = match value.as_str() {
                Some("evict-lru") => {
                    rustred::solver::CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
                }
                Some("evict-aged-lfu") => {
                    rustred::solver::CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed
                }
                Some("fail-on-limit") => rustred::solver::CandidateRoutedCachePolicy::FailOnLimit,
                _ => {
                    return Err(
                        "cache_policy must be evict-lru, evict-aged-lfu or fail-on-limit".into(),
                    );
                }
            };
            continue;
        }
        let value = value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("{name} must be a nonnegative native-size integer"))?;
        let destination = match name.as_str() {
            "preparation_workers" if value != 0 => &mut candidate.preparation_workers,
            "preparation_workers" => return Err("preparation_workers must be positive".into()),
            "max_input_targets" => &mut candidate.trace_limits.max_input_targets,
            "max_unique_nodes" => &mut candidate.trace_limits.max_unique_nodes,
            "max_transport_calls" => &mut candidate.trace_limits.max_transport_calls,
            "max_transport_operations" => &mut candidate.trace_limits.max_transport_operations,
            "max_transport_endpoints" => &mut candidate.trace_limits.max_transport_endpoints,
            "max_rule_applications" => &mut candidate.reduction_limits.max_rule_applications,
            "max_cached_integrals" => &mut candidate.reduction_limits.max_cached_integrals,
            "max_cached_coefficient_terms" => {
                &mut candidate.reduction_limits.max_cached_coefficient_terms
            }
            "max_cached_coefficient_bytes" => {
                &mut candidate.reduction_limits.max_cached_coefficient_bytes
            }
            "max_pending_frames" => &mut candidate.reduction_limits.max_pending_frames,
            "max_coalescing_additions" => &mut candidate.reduction_limits.max_coalescing_additions,
            "max_factorization_terms" => &mut candidate.reduction_limits.max_factorization_terms,
            _ => return Err(format!("unknown operational limit {name}").into()),
        };
        *destination = value;
    }
    *options = candidate;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn work_overrides_do_not_change_unmentioned_policy() {
        let original = SavedApplicationOptions::default();
        let mut options = original;
        override_from_value(
            &mut options,
            &json!({"max_transport_endpoints": 40_000_000, "max_rule_applications": 0}),
        )
        .unwrap();
        assert_eq!(options.trace_limits.max_transport_endpoints, 40_000_000);
        assert_eq!(options.reduction_limits.max_rule_applications, 0);
        options.trace_limits.max_transport_endpoints =
            original.trace_limits.max_transport_endpoints;
        options.reduction_limits.max_rule_applications =
            original.reduction_limits.max_rule_applications;
        assert_eq!(options, original);
    }

    #[test]
    fn invalid_or_unknown_limits_are_transactional_errors() {
        for value in [
            json!({"cache_policy": "unknown"}),
            json!({"cache_policy": 1}),
            json!({"preparation_workers": 0}),
            json!({"max_rule_applications": -1}),
            json!({"max_rule_applications": 1.5}),
            json!({"max_rule_applications": "unlimited"}),
            json!({"max_rule_applications": 0, "rank": 12}),
            json!([]),
        ] {
            let mut options = SavedApplicationOptions::default();
            let original = options;
            assert!(override_from_value(&mut options, &value).is_err());
            assert_eq!(options, original);
        }
    }

    #[test]
    fn preparation_workers_has_no_campaign_worker_ceiling() {
        let mut options = SavedApplicationOptions::default();
        override_from_value(&mut options, &json!({"preparation_workers": 64})).unwrap();
        assert_eq!(options.preparation_workers, 64);
    }

    #[test]
    fn cache_policy_is_explicit_and_transactional() {
        let mut options = SavedApplicationOptions::default();
        assert_eq!(
            options.cache_policy,
            rustred::solver::CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
        );
        override_from_value(&mut options, &json!({"cache_policy": "fail-on-limit"})).unwrap();
        assert_eq!(
            options.cache_policy,
            rustred::solver::CandidateRoutedCachePolicy::FailOnLimit
        );
        override_from_value(&mut options, &json!({"cache_policy": "evict-aged-lfu"})).unwrap();
        assert_eq!(
            options.cache_policy,
            rustred::solver::CandidateRoutedCachePolicy::EvictAgedLeastFrequentlyUsed
        );
        let before = options;
        assert!(
            override_from_value(
                &mut options,
                &json!({"cache_policy": "evict-lru", "unknown": 0})
            )
            .is_err()
        );
        assert_eq!(options, before);
    }
}
