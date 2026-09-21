//! Explicit per-call native expansion policy; aggregate campaign caps are separate.
use std::{fs::File, io::Read, path::Path};

use rustred::sector::symmetry::integral_transport::ExpansionLimits as MultiAffineNumeratorExpansionLimits;
use serde::{Deserialize, Serialize};

use crate::cli::{error::CliError, io::read_bounded};

const MAX_POLICY_BYTES: usize = 16 * 1024;

// A struct, rather than a JSON Value, rejects duplicate fields and non-integer
// values. Missing fields retain native defaults; explicit null is an error.
// Exact-algebra limits are deliberately excluded: they must match the common
// owner context, which this transport-only override does not change.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct ExpansionPolicy {
    max_factors: usize,
    max_relation_coefficient_entries: usize,
    max_total_power: u64,
    max_native_polynomial_terms: usize,
    max_native_polynomial_operations: usize,
    max_native_exponent_entries: usize,
    max_endpoints: usize,
    max_endpoint_power_entries: usize,
    max_retained_endpoint_key_bytes: usize,
    max_retained_coefficient_terms: usize,
    max_retained_coefficient_clone_owned_bytes: usize,
}

impl Default for ExpansionPolicy {
    fn default() -> Self {
        let limits = MultiAffineNumeratorExpansionLimits::default();
        Self {
            max_factors: limits.max_factors,
            max_relation_coefficient_entries: limits.max_relation_coefficient_entries,
            max_total_power: limits.max_total_power,
            max_native_polynomial_terms: limits.max_native_polynomial_terms,
            max_native_polynomial_operations: limits.max_native_polynomial_operations,
            max_native_exponent_entries: limits.max_native_exponent_entries,
            max_endpoints: limits.max_endpoints,
            max_endpoint_power_entries: limits.max_endpoint_power_entries,
            max_retained_endpoint_key_bytes: limits.max_retained_endpoint_key_bytes,
            max_retained_coefficient_terms: limits.max_retained_coefficient_terms,
            max_retained_coefficient_clone_owned_bytes: limits
                .max_retained_coefficient_clone_owned_bytes,
        }
    }
}

impl ExpansionPolicy {
    pub(super) fn read(path: &Path) -> Result<Self, CliError> {
        let file = File::open(path).map_err(|error| {
            CliError::InputIo(format!(
                "cannot open expansion limits {}: {error}",
                path.display()
            ))
        })?;
        if !file
            .metadata()
            .map_err(|error| CliError::InputIo(error.to_string()))?
            .is_file()
        {
            return Err(CliError::Input(
                "expansion limits must be a regular file".into(),
            ));
        }
        Self::from_reader(file)
    }

    fn from_reader(reader: impl Read) -> Result<Self, CliError> {
        let bytes = read_bounded(reader, "expansion limits", MAX_POLICY_BYTES)?;
        // Serde's derived struct visitor also accepts positional sequences;
        // this public policy is deliberately a named-field JSON object only.
        if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
            return Err(CliError::Input(
                "expansion limits must be a JSON object".into(),
            ));
        }
        let policy: Self = serde_json::from_slice(&bytes)
            .map_err(|error| CliError::Input(format!("invalid expansion limits: {error}")))?;
        if policy.max_total_power == 0
            || [
                policy.max_factors,
                policy.max_relation_coefficient_entries,
                policy.max_native_polynomial_terms,
                policy.max_native_polynomial_operations,
                policy.max_native_exponent_entries,
                policy.max_endpoints,
                policy.max_endpoint_power_entries,
                policy.max_retained_endpoint_key_bytes,
                policy.max_retained_coefficient_terms,
                policy.max_retained_coefficient_clone_owned_bytes,
            ]
            .contains(&0)
        {
            return Err(CliError::Input(
                "expansion limits must be positive integers".into(),
            ));
        }
        Ok(policy)
    }

    pub(super) fn limits(&self) -> MultiAffineNumeratorExpansionLimits {
        MultiAffineNumeratorExpansionLimits {
            max_factors: self.max_factors,
            max_relation_coefficient_entries: self.max_relation_coefficient_entries,
            max_total_power: self.max_total_power,
            max_native_polynomial_terms: self.max_native_polynomial_terms,
            max_native_polynomial_operations: self.max_native_polynomial_operations,
            max_native_exponent_entries: self.max_native_exponent_entries,
            max_endpoints: self.max_endpoints,
            max_endpoint_power_entries: self.max_endpoint_power_entries,
            max_retained_endpoint_key_bytes: self.max_retained_endpoint_key_bytes,
            max_retained_coefficient_terms: self.max_retained_coefficient_terms,
            max_retained_coefficient_clone_owned_bytes: self
                .max_retained_coefficient_clone_owned_bytes,
            ..MultiAffineNumeratorExpansionLimits::default()
        }
    }

    pub(super) fn json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("integer expansion policy is JSON representable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<ExpansionPolicy, CliError> {
        ExpansionPolicy::from_reader(text.as_bytes())
    }

    #[test]
    fn defaults_and_one_override_preserve_native_policy() {
        let defaults = MultiAffineNumeratorExpansionLimits::default();
        assert_eq!(parse("{}").unwrap().limits(), defaults);
        let parsed = parse(r#"{"max_native_polynomial_operations":500000000}"#).unwrap();
        assert_eq!(
            parsed.limits(),
            MultiAffineNumeratorExpansionLimits {
                max_native_polynomial_operations: 500_000_000,
                ..defaults
            }
        );
        assert_eq!(parsed.json()["max_factors"], defaults.max_factors);
        assert_eq!(
            parsed.json()["max_native_polynomial_operations"],
            500_000_000
        );
    }

    #[test]
    fn all_explicit_fields_are_forwarded_without_changing_exact_algebra() {
        let parsed = parse(
            r#"{
            "max_factors":1,"max_relation_coefficient_entries":2,"max_total_power":3,
            "max_native_polynomial_terms":4,"max_native_polynomial_operations":5,
            "max_native_exponent_entries":6,"max_endpoints":7,
            "max_endpoint_power_entries":8,"max_retained_endpoint_key_bytes":9,
            "max_retained_coefficient_terms":10,"max_retained_coefficient_clone_owned_bytes":11
        }"#,
        )
        .unwrap();
        assert_eq!(
            parsed.limits(),
            MultiAffineNumeratorExpansionLimits {
                max_factors: 1,
                max_relation_coefficient_entries: 2,
                max_total_power: 3,
                max_native_polynomial_terms: 4,
                max_native_polynomial_operations: 5,
                max_native_exponent_entries: 6,
                max_endpoints: 7,
                max_endpoint_power_entries: 8,
                max_retained_endpoint_key_bytes: 9,
                max_retained_coefficient_terms: 10,
                max_retained_coefficient_clone_owned_bytes: 11,
                ..Default::default()
            }
        );
        assert_eq!(parsed.json().as_object().unwrap().len(), 11);
    }

    #[test]
    fn malformed_duplicate_unknown_and_nonpositive_values_are_rejected() {
        for text in [
            r#"{"max_endpoints":1,"max_endpoints":2}"#,
            r#"{"max_endpoint":1}"#,
            r#"{"exact_algebra":{}}"#,
            r#"{"max_endpoints":null}"#,
            r#"{"max_endpoints":true}"#,
            r#"{"max_endpoints":1.5}"#,
            r#"{"max_endpoints":-1}"#,
            r#"{"max_endpoints":0}"#,
            r#"{"max_total_power":0}"#,
            r#"{"max_endpoints":18446744073709551616}"#,
            r#"{"max_total_power":18446744073709551616}"#,
            "[]",
            "[1,2,3,4,5,6,7,8,9,10,11]",
            "null",
            "{}{}",
        ] {
            assert!(parse(text).is_err(), "unexpected admission: {text}");
        }
    }

    #[test]
    fn policy_read_is_bounded_before_native_work() {
        assert!(parse(&" ".repeat(MAX_POLICY_BYTES + 1)).is_err());
        let mut exact = " ".repeat(MAX_POLICY_BYTES - 2);
        exact.push_str("{}");
        assert!(parse(&exact).is_ok());
    }
}
