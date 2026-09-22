//! Bounded JSON planning of finite starting domains; never a reduction run.

use super::{EntryPowerBudget, FiniteEntryDomain};
use crate::AppError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use symbolica::domains::integer::Integer;

const MAX_SPEC_BYTES: usize = 1024 * 1024;
const MAX_SECTORS: usize = 10_000;
const MAX_ARITY: usize = 1024;
const MAX_PREVIEW_TARGETS: usize = 10_000;
const MAX_PREVIEW_COORDINATES: usize = 1_000_000;
const MAX_POSITIVE_LAYERS: usize = 1_000_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    schema: String,
    sectors: Vec<String>,
    budget: Option<EntryPowerBudget>,
    profile: Option<Profile>,
    #[serde(default)]
    max_preview_targets: usize,
    max_positive_layers_per_sector: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    name: String,
    loops: u32,
}

/// Plan a finite *starting* domain from `rustred.entry-domain.json.v1` JSON.
///
/// Exactly one explicit `budget` or named `profile` is required. Sector masks
/// are unique, same-arity 0/1 strings. The positive-layer allowance is required
/// and applies to each exact count, not to coverage. Any refusal fails the
/// entire plan rather than returning partial counts. The optional preview is
/// a global prefix (default zero), never a solver result or a closure claim.
///
/// Transport bounds: 1 MiB input, 10,000 sectors, 1,024 axes, 10,000 preview
/// targets / 1,000,000 preview coordinates, and 1,000,000 count layers per
/// sector. These are planning resource limits, not physical assumptions.
pub fn entry_domain_plan(source: &str) -> Result<Value, AppError> {
    if source.len() > MAX_SPEC_BYTES {
        return Err(AppError::limit("entry-domain specification exceeds 1 MiB"));
    }
    let spec: Spec = serde_json::from_str(source)
        .map_err(|e| AppError::schema(format!("entry-domain specification: {e}")))?;
    if spec.schema != "rustred.entry-domain.json.v1" {
        return Err(AppError::schema(
            "unsupported entry-domain specification schema",
        ));
    }
    if spec.sectors.is_empty() || spec.sectors.len() > MAX_SECTORS {
        return Err(AppError::input(
            "entry-domain sectors must contain 1..=10000 masks",
        ));
    }
    if spec.max_preview_targets > MAX_PREVIEW_TARGETS {
        return Err(AppError::limit(
            "entry-domain preview exceeds 10000 targets",
        ));
    }
    if !(1..=MAX_POSITIVE_LAYERS).contains(&spec.max_positive_layers_per_sector) {
        return Err(AppError::limit(
            "positive-layer allowance must be 1..=1000000 per sector",
        ));
    }
    let arity = spec.sectors[0].len();
    if !(1..=MAX_ARITY).contains(&arity) {
        return Err(AppError::input(
            "entry-domain masks must have 1..=1024 axes",
        ));
    }
    if spec
        .max_preview_targets
        .checked_mul(arity)
        .is_none_or(|n| n > MAX_PREVIEW_COORDINATES)
    {
        return Err(AppError::limit(
            "entry-domain preview exceeds 1000000 coordinates",
        ));
    }
    let mut seen = BTreeSet::new();
    for sector in &spec.sectors {
        if sector.len() != arity || !sector.bytes().all(|b| b == b'0' || b == b'1') {
            return Err(AppError::input(
                "entry-domain masks must be same-arity 0/1 strings",
            ));
        }
        if !seen.insert(sector.as_str()) {
            return Err(AppError::input(format!(
                "duplicate entry-domain sector {sector}"
            )));
        }
    }
    let (budget, profile) = match (spec.budget, spec.profile) {
        (Some(budget), None) => (
            budget,
            json!({
                "name": "explicit",
                "assumptions": "caller_supplied_bounds; no_automatic_physics_coverage_claim"
            }),
        ),
        (None, Some(profile)) if profile.name == "renormalizable_marginal_feynman" => {
            let budget = EntryPowerBudget::renormalizable_marginal_feynman(profile.loops)?;
            (
                budget,
                json!({
                    "name": profile.name,
                    "loops": profile.loops,
                    "assumptions": "caller_guaranteed; not_automatically_authenticated",
                    "requires": [
                        "Feynman gauge; dimension-four renormalizable polynomial vertices",
                        "ordinary three/four-valent bare skeleton; tree two-point insertions resummed",
                        "proper non-oversubtracted forests with vertex-disjoint maximal children and positive-loop quotients; no vacuum/one-point nodes",
                        "marginal coefficient; polynomial scalarization; no stripped inverse-mass factors",
                        "loops counts still-unintegrated loops, including factorized components, not total perturbative order"
                    ]
                }),
            )
        }
        (None, Some(_)) => return Err(AppError::input("unknown entry-domain profile")),
        _ => {
            return Err(AppError::input(
                "supply exactly one entry-domain budget or profile",
            ));
        }
    };
    let mut sectors = Vec::new();
    let mut preview = Vec::new();
    let mut total = Integer::from(0);
    for sector in &spec.sectors {
        let domain = FiniteEntryDomain::new(sector.bytes().map(|b| b == b'1').collect(), budget)?;
        let count = domain
            .target_count(spec.max_positive_layers_per_sector)
            .map_err(|e| AppError::new(e.kind(), format!("sector {sector}: {e}")))?;
        total += &count;
        sectors.push(json!({"sector": sector, "target_count": count.to_string()}));
        let remaining = spec.max_preview_targets - preview.len();
        for key in domain.targets().take(remaining) {
            let key = key?;
            preview.push(json!({"sector": sector, "powers": key.powers()}));
        }
    }
    let truncated = total > Integer::from(preview.len() as u64);
    Ok(json!({
        "schema": "rustred.entry-domain-plan.json.v1",
        "operation": "entry_domain_plan",
        "scope": "finite_starting_targets_only; not_descendant_bounds_or_solver_closure",
        "counts_exact": true,
        "arity": arity,
        "budget": budget,
        "profile": profile,
        "max_positive_layers_per_sector": spec.max_positive_layers_per_sector,
        "total_target_count": total.to_string(),
        "sectors": sectors,
        "preview": {
            "max_targets": spec.max_preview_targets,
            "emitted": preview.len(),
            "truncated": truncated,
            "order": "input_sector_order; native_A/R_shell_and_composition_order",
            "scope": "prefix_only; not_coverage_or_closure",
            "targets": preview
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppErrorKind;

    fn spec() -> Value {
        json!({"schema":"rustred.entry-domain.json.v1", "sectors":["10","01"],
            "budget":{"max_positive_power":3,"max_numerator_rank":2,"min_power_difference":1},
            "max_positive_layers_per_sector":3})
    }
    fn plan(spec: &Value) -> Result<Value, AppError> {
        entry_domain_plan(&spec.to_string())
    }

    #[test]
    fn entry_plan_exact_counts_and_bounded_global_preview() {
        let mut s = spec();
        let empty = plan(&s).unwrap();
        assert_eq!(empty["total_target_count"], "12");
        assert_eq!(empty["preview"]["emitted"], 0);
        assert_eq!(empty["preview"]["truncated"], true);
        s["max_preview_targets"] = json!(3);
        let p = plan(&s).unwrap();
        assert_eq!(p["sectors"][0]["target_count"], "6");
        assert_eq!(p["sectors"][1]["target_count"], "6");
        assert_eq!(
            p["preview"]["targets"],
            json!([
                {"sector":"10","powers":[1,0]},
                {"sector":"10","powers":[2,0]},
                {"sector":"10","powers":[2,-1]}
            ])
        );
        s["max_preview_targets"] = json!(12);
        let p = plan(&s).unwrap();
        assert_eq!(p["preview"]["emitted"], 12);
        assert_eq!(p["preview"]["truncated"], false);
    }

    #[test]
    fn entry_plan_profile_is_explicit_and_counts_are_decimal_strings() {
        let mut s = spec();
        s.as_object_mut().unwrap().remove("budget");
        s["profile"] = json!({"name":"renormalizable_marginal_feynman","loops":5});
        s["max_positive_layers_per_sector"] = json!(25);
        let p = plan(&s).unwrap();
        assert_eq!(p["budget"]["max_positive_power"], 24);
        assert_eq!(p["budget"]["max_numerator_rank"], 14);
        assert_eq!(p["budget"]["min_power_difference"], 10);
        assert_eq!(
            p["profile"]["assumptions"],
            "caller_guaranteed; not_automatically_authenticated"
        );
        s.as_object_mut().unwrap().remove("profile");
        s["budget"] = json!({"max_positive_power":80,"max_numerator_rank":0});
        s["sectors"] = json!(["1".repeat(40)]);
        s["max_positive_layers_per_sector"] = json!(41);
        assert_eq!(
            plan(&s).unwrap()["total_target_count"],
            "107507208733336176461620"
        );
    }

    #[test]
    fn entry_plan_rejects_duplicate_invalid_and_mixed_arity_masks() {
        for sectors in [
            json!([]),
            json!([""]),
            json!(["10", "10"]),
            json!(["10", "1"]),
            json!(["10", "1x"]),
        ] {
            let mut s = spec();
            s["sectors"] = sectors;
            assert_eq!(plan(&s).unwrap_err().kind(), AppErrorKind::Input);
        }
    }

    #[test]
    fn entry_plan_rejects_conflicting_missing_unknown_and_malformed_policy() {
        let mut s = spec();
        s["profile"] = json!({"name":"renormalizable_marginal_feynman","loops":5});
        assert!(plan(&s).is_err());
        s.as_object_mut().unwrap().remove("budget");
        s["profile"]["name"] = json!("invented");
        assert!(plan(&s).is_err());
        s.as_object_mut().unwrap().remove("profile");
        assert!(plan(&s).is_err());
        let mut s = spec();
        s["unknown"] = json!(true);
        assert_eq!(plan(&s).unwrap_err().kind(), AppErrorKind::Schema);
        let mut s = spec();
        s["schema"] = json!("other");
        assert_eq!(plan(&s).unwrap_err().kind(), AppErrorKind::Schema);
        let mut s = spec();
        s.as_object_mut()
            .unwrap()
            .remove("max_positive_layers_per_sector");
        assert_eq!(plan(&s).unwrap_err().kind(), AppErrorKind::Schema);
    }

    #[test]
    fn entry_plan_work_refusal_never_returns_partial_counts() {
        let mut s = spec();
        s["max_positive_layers_per_sector"] = json!(2);
        let e = plan(&s).unwrap_err();
        assert_eq!(e.kind(), AppErrorKind::Limit);
        assert!(e.message().contains("sector 10"));
        for (field, value) in [
            ("max_preview_targets", 10_001),
            ("max_positive_layers_per_sector", 0),
            ("max_positive_layers_per_sector", 1_000_001),
        ] {
            let mut s = spec();
            s[field] = json!(value);
            assert_eq!(plan(&s).unwrap_err().kind(), AppErrorKind::Limit);
        }
        let mut s = spec();
        s["budget"]["max_positive_power"] = json!(0);
        assert_eq!(plan(&s).unwrap()["total_target_count"], "0");
        assert_eq!(plan(&s).unwrap()["preview"]["truncated"], false);
    }
}
