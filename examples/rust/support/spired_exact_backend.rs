//! Example-only steering for the opt-in native symbolic exact-lifting pilot.

use rustred::solver::{CoefficientVariableOrder, SymbolicExactBackend};

pub fn from_environment() -> Result<SymbolicExactBackend, String> {
    let mode = std::env::var("RUSTRED_SPIRED_SYMBOLIC_EXACT_BACKEND")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            _ => Err("symbolic exact backend must be valid UTF-8".to_owned()),
        })?;
    let limit = std::env::var("RUSTRED_SPIRED_FRACTION_FREE_MAX_ENTRIES")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            _ => Err("fraction-free entry limit must be valid UTF-8".to_owned()),
        })?;
    parse(mode.as_deref(), limit.as_deref())
}

pub fn coefficient_order_from_environment() -> Result<CoefficientVariableOrder, String> {
    let order = std::env::var("RUSTRED_SPIRED_COEFFICIENT_VARIABLE_ORDER")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            _ => Err("coefficient variable order must be valid UTF-8".to_owned()),
        })?;
    parse_coefficient_order(order.as_deref())
}

fn parse_coefficient_order(order: Option<&str>) -> Result<CoefficientVariableOrder, String> {
    match order.unwrap_or("original") {
        "original" => Ok(CoefficientVariableOrder::Original),
        "reverse" => Ok(CoefficientVariableOrder::Reverse),
        "indices-first" => Ok(CoefficientVariableOrder::IndicesFirst),
        _ => Err("coefficient variable order must be original, reverse or indices-first".into()),
    }
}

fn parse(mode: Option<&str>, limit: Option<&str>) -> Result<SymbolicExactBackend, String> {
    match mode.unwrap_or("sparse") {
        "sparse" if limit.is_none() => Ok(SymbolicExactBackend::Sparse),
        "sparse" => Err("fraction-free entry limit requires dense-fraction-free mode".into()),
        "dense-fraction-free" => {
            let max_matrix_entries = limit
                .unwrap_or("1000000")
                .parse::<usize>()
                .ok()
                .filter(|limit| *limit > 0)
                .ok_or("fraction-free entry limit must be a positive integer")?;
            Ok(SymbolicExactBackend::DenseFractionFree { max_matrix_entries })
        }
        _ => Err("symbolic exact backend must be sparse or dense-fraction-free".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_order_is_explicit_and_preserves_the_default() {
        assert_eq!(
            parse_coefficient_order(None).unwrap(),
            CoefficientVariableOrder::Original
        );
        assert_eq!(
            parse_coefficient_order(Some("original")).unwrap(),
            CoefficientVariableOrder::Original
        );
        assert_eq!(
            parse_coefficient_order(Some("reverse")).unwrap(),
            CoefficientVariableOrder::Reverse
        );
        assert_eq!(
            parse_coefficient_order(Some("indices-first")).unwrap(),
            CoefficientVariableOrder::IndicesFirst
        );
        assert!(parse_coefficient_order(Some("automatic")).is_err());
        assert!(parse_coefficient_order(Some("")).is_err());
    }

    #[test]
    fn explicit_dense_policy_never_changes_the_default() {
        assert_eq!(parse(None, None).unwrap(), SymbolicExactBackend::Sparse);
        assert_eq!(
            parse(Some("sparse"), None).unwrap(),
            SymbolicExactBackend::Sparse
        );
        assert_eq!(
            parse(Some("dense-fraction-free"), Some("143636")).unwrap(),
            SymbolicExactBackend::DenseFractionFree {
                max_matrix_entries: 143636
            }
        );
    }

    #[test]
    fn unknown_modes_and_silent_or_invalid_limits_are_rejected() {
        assert!(parse(Some("automatic"), None).is_err());
        assert!(parse(None, Some("100")).is_err());
        assert!(parse(Some("dense-fraction-free"), Some("0")).is_err());
        assert!(parse(Some("dense-fraction-free"), Some("-1")).is_err());
        assert!(parse(Some("dense-fraction-free"), Some("unbounded")).is_err());
    }
}
