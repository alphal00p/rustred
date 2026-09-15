//! Example-only steering for the opt-in native symbolic exact-lifting pilot.

use rustred::solver::SymbolicExactBackend;

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
