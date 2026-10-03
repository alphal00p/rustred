//! Optional native source-weight reconstruction, followed by the same exact
//! source composition and publication gates as symbolic elimination.
use super::*;
use project::{Guard, Limits, Projection, Proposal, Row};
use rustred::algebra::IndexedCoefficientContext;
use rustred::solver::{ProjectedSourceWeightLimits, reconstruct_projected_source_weights};
use symbolica::domains::SelfRing;

pub(super) fn enabled(r: &Value) -> bool {
    r["projection_backend"].as_str() == Some("source-weights")
}

pub(super) fn validate(r: &Value) -> Result<()> {
    if !enabled(r) {
        return Ok(());
    }
    let options = &r["source_weight_reconstruction"];
    for name in [
        "max_degree",
        "max_probes",
        "max_attempts",
        "max_primes",
        "max_cached_images",
        "max_cached_values",
        "max_weight_slots",
    ] {
        require(
            number(options, name)? > 0,
            "positive reconstruction allowance required",
        )?;
    }
    checked(u16::try_from(number(options, "max_degree")?))?;
    require(
        number(options, "max_primes")? >= 2,
        "at least two reconstruction primes required",
    )
}

pub(super) fn project(
    r: &Value,
    c: &IndexedCoefficientContext,
    rows: &[Row],
    target: &IndexShift,
    forbidden: &BTreeSet<IndexShift>,
    input_guards: &[Guard],
    limits: Limits,
) -> project::Result<Projection> {
    let invalid = |e: String| project::Error::Invalid(e);
    validate(r).map_err(invalid)?;
    let option = |name| number(&r["source_weight_reconstruction"], name).map_err(invalid);
    let policy = ProjectedSourceWeightLimits {
        arithmetic: limits.arithmetic,
        max_rows: limits.rows,
        max_columns: limits.columns,
        max_input_nonzeros: limits.nonzeros,
        max_coefficient_terms: limits.coefficient_terms,
        max_degree: u16::try_from(option("max_degree")?)
            .map_err(|_| project::Error::Budget("reconstruction degree"))?,
        max_probes: option("max_probes")?,
        max_attempts: option("max_attempts")?,
        max_primes: option("max_primes")?,
        max_cached_images: option("max_cached_images")?,
        max_cached_values: option("max_cached_values")?,
        max_weight_slots: option("max_weight_slots")?,
    };
    let mut guards = Vec::new();
    for guard in input_guards {
        project::retain(
            c,
            &mut guards,
            guard.polynomial.clone(),
            guard.origin.clone(),
            limits,
        )?;
    }
    // Retain every original image denominator, not merely the sampled prefix.
    // Reconstructed weights are a NEW exact certificate; no numerical pivot is
    // imported as an everywhere-nonzero assumption or as publication authority.
    for value in rows.iter().flat_map(|row| row.values()) {
        project::denominator(c, &mut guards, value, "reconstruction input", limits)?;
    }
    let proposal = project::native(|| {
        reconstruct_projected_source_weights(c, rows, target, forbidden, policy, |event| {
            progress::event(
                "source_weight_reconstruction",
                || json!({"detail":format!("{event:?}")}),
            )
        })
    })?
    .map_err(|error| project::Error::Invalid(format!("source-weight reconstruction: {error}")))?;
    // Native reconstruction has already checked every column of W*A. Replay
    // again through the common bounded indexed product, retaining weight poles
    // BEFORE any cancellation and restoring the original coefficient context.
    let image = project::replay(c, rows, &proposal.weights, &mut guards, limits)?;
    project::require(
        image == proposal.image,
        "reconstructed full source product differs",
    )?;
    project::require(
        image.get(target).is_some_and(|v| v.raw().is_one())
            && forbidden.iter().all(|shift| !image.contains_key(shift)),
        "reconstructed target/F identity differs",
    )?;
    progress::event("projection_target", || {
        json!({
            "backend":"source-weights", "rows_visited":proposal.prefix_rows,
            "f_size":forbidden.len(), "weight_terms":proposal.weights.len(),
            "full_image_terms":image.len(),
        })
    });
    Ok(Projection::Target(Proposal {
        weights: proposal.weights,
        image,
        guards,
        prefix_rows: proposal.prefix_rows,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Value {
        json!({"max_degree":16,"max_probes":4096,"max_attempts":4,
            "max_primes":8,"max_cached_images":512,"max_cached_values":100000,
            "max_weight_slots":1000})
    }

    #[test]
    fn source_weight_options_are_explicit_and_checked() {
        let mut r = json!({"projection_backend":"source-weights", "source_weight_reconstruction":options()});
        assert!(validate(&r).is_ok());
        for name in [
            "max_degree",
            "max_probes",
            "max_attempts",
            "max_primes",
            "max_cached_images",
            "max_cached_values",
            "max_weight_slots",
        ] {
            let mut bad = r.clone();
            bad["source_weight_reconstruction"][name] = json!(0);
            assert!(validate(&bad).is_err());
        }
        r["source_weight_reconstruction"]["max_degree"] = json!(65536);
        assert!(validate(&r).is_err());
        assert!(validate(&json!({})).is_ok());
    }

    #[test]
    fn reconstructed_tadpole_exports_same_checked_rule_as_exact_selected_frame() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        r["exact_source_ordinals"] = json!([1]);
        let (exact, expected) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(exact["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        r["projection_backend"] = json!("source-weights");
        r["source_weight_reconstruction"] = options();
        let (reconstructed, actual) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert_eq!(reconstructed["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(actual, expected);
        assert_eq!(reconstructed["finite_bank_rows"], exact["finite_bank_rows"]);
        assert_eq!(
            reconstructed["exact_projection_rows"],
            exact["exact_projection_rows"]
        );
    }

    #[test]
    fn genuine_reconstructed_weight_pole_cannot_publish_on_its_zero_boundary() {
        let (bytes, mut r) = crate::tests::tadpole();
        r["forbid_cofinally_higher_columns"] = json!(true);
        r["exact_source_ordinals"] = json!([1]);
        r["projection_backend"] = json!("source-weights");
        r["source_weight_reconstruction"] = options();
        // The backward recurrence needs 1/(n-1). Local lower zero includes
        // physical n=1: rational identity alone cannot authorize this chart.
        r["chart"]["lower"] = json!([0]);
        let (report, artifact) = crate::run::<1>(&bytes, &r, true).unwrap();
        assert!(artifact.is_none());
        assert_ne!(report["status"], "CHECKED_PRIORITY_OWNER_EXPORTED");
        assert_eq!(report["attempts"].as_array().unwrap().len(), 1);
        assert_eq!(
            report["attempts"][0]["full_original_product_replayed"],
            true
        );
        assert!(report["attempts"][0].get("proof_error").is_some());
    }
}
