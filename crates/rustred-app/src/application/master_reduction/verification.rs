//! Publication assurance policy; inventory extraction and graph replay differ.
use super::*;

pub(super) const RECIPE: &str = "publication-verification-v1";

pub(super) fn for_phase(
    options: &MasterReductionOptions,
    resumed: Option<&Value>,
) -> Result<bool, AppError> {
    let Some(report) = resumed else {
        return Ok(options.deep_verification.unwrap_or(false));
    };
    // Publications written before this choice existed always ran deep replay.
    let recorded = match report.get("deep_verification") {
        None => true,
        Some(Value::Bool(value)) if report["publication_verification_recipe"] == RECIPE => *value,
        _ => return Err(AppError::input("invalid publication verification snapshot")),
    };
    if options
        .deep_verification
        .is_some_and(|requested| requested != recorded)
    {
        return Err(AppError::input(
            "cannot change deep verification on resume; start a new publication phase",
        ));
    }
    Ok(recorded)
}

pub(super) fn record(report: &mut Value, deep: bool) {
    report["deep_verification"] = json!(deep);
    report["publication_verification_recipe"] = json!(RECIPE);
    report["publication_verification_mode"] = json!(if deep {
        "independent_replay"
    } else {
        "inventory"
    });
}

pub(super) fn independently_verified(report: &Value) -> bool {
    report["inventory"]["independently_verified"]
        .as_bool()
        .unwrap_or_else(|| {
            report["inventory"]["complete"] == true
                && report["deep_verification"].as_bool().unwrap_or(true)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fast_and_resume_mode_is_frozen() {
        let mut options = MasterReductionOptions::new("unused", "unused");
        assert!(!for_phase(&options, None).unwrap());
        let mut report = json!({});
        record(&mut report, true);
        assert!(for_phase(&options, Some(&report)).unwrap());
        options.deep_verification = Some(false);
        assert!(for_phase(&options, Some(&report)).is_err());
        options.deep_verification = Some(true);
        assert!(for_phase(&options, Some(&report)).unwrap());
        report["publication_verification_recipe"] = json!("unknown");
        assert!(for_phase(&options, Some(&report)).is_err());
    }

    #[test]
    fn complete_fast_inventory_never_claims_independent_replay() {
        let fast = json!({"deep_verification":false,"inventory":{"complete":true,"independently_verified":false}});
        assert!(!independently_verified(&fast));
        let legacy = json!({"inventory":{"complete":true}});
        assert!(independently_verified(&legacy));
        assert!(
            for_phase(
                &MasterReductionOptions::new("unused", "unused"),
                Some(&legacy)
            )
            .unwrap()
        );
        assert!(!independently_verified(
            &json!({"deep_verification":true,"inventory":{"complete":false}})
        ));
    }
}
