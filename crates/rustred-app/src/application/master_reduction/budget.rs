//! Operational budgets are not part of a mathematical search identity.
use super::*;

pub(super) fn configure(
    options: &MasterReductionOptions,
    session: &mut TerminalRelationSession,
    report: &mut Value,
) -> Result<(), AppError> {
    let stored = match report.get("max_relation_columns") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| AppError::input("invalid finite relation column budget"))?,
        ),
    };
    let maximum = options.max_relation_columns.or(stored).filter(|&n| n != 0);
    session.set_max_columns(maximum).map_err(io)?;
    report["max_relation_columns"] = json!(maximum);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_default_has_no_preset_column_count_ceiling() {
        for profile in [
            MasterNormalizationProfile::ConservativeV1,
            MasterNormalizationProfile::StandardV1,
        ] {
            let limits = profile.relation_limits();
            assert_eq!(limits.max_columns, usize::MAX);
            // Trusted generated native state follows the process RAM guard;
            // only explicit caller grants add operational resource stops.
            assert_eq!(limits.max_nonzeros, usize::MAX);
            assert_eq!(
                limits.algebra,
                rustred::algebra::IndexedAlgebraLimits::unbounded_resources()
            );
            assert_eq!(
                profile::native_io_limits(None).max_program_bytes,
                usize::MAX
            );
        }
    }
}
