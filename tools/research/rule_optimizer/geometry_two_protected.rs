//! Input-driven bridge to the native two-protected tangent source plan.
//! No source algebra, nullspace solver or proof authority is implemented here.
//! Native materialization retains the complete ordinary product and its guards;
//! the caller must compare that product with the common producer regeneration.

use super::{Result, array, checked, coefficient, integers, limit, policies, require};
use crate::geometry_tangent::Nomination;
use rustred::{
    algebra::IndexedCoefficient,
    family::{ContractionMomentum, IntegralFamily},
    identity::{
        CompletedIbpSourceRows, IndexShift, IntegralShift, ParametricIbpGenerator,
        TangentSourceLimits, TangentSourcePlan, TangentSourceSpec,
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) const SCHEMA: &str = "rustred.two-protected-tangent-chart.v1";

pub(super) struct MaterializedNomination {
    pub nomination: Nomination,
    pub full_product: BTreeMap<IndexShift, IndexedCoefficient>,
}

fn index(value: &Value) -> Result<usize> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| "two-protected index must be a nonnegative integer".into())
}

/// Shape checks do not assume a numerator axis or constrain arbitrary recenter.
/// Native construction subsequently checks actual loop/external availability.
pub(super) fn parse(r: &Value, arity: usize) -> Result<TangentSourceSpec> {
    require(r["schema"] == SCHEMA, "wrong two-protected schema")?;
    require(
        r.get("sources").is_none(),
        "two-protected nomination cannot also prescribe sources",
    )?;
    let n = r["nomination"]
        .as_object()
        .ok_or("nomination object required")?;
    require(
        n.keys().all(|key| {
            matches!(
                key.as_str(),
                "differentiated_loop" | "protected_denominators" | "contractions" | "recenter"
            )
        }),
        "unsupported two-protected nomination field (multipliers are not supported)",
    )?;
    let differentiated_loop = index(&r["nomination"]["differentiated_loop"])?;
    let protected = array(&r["nomination"], "protected_denominators")?;
    require(protected.len() == 2, "exactly two protected axes required")?;
    let protected_denominators = [index(&protected[0])?, index(&protected[1])?];
    require(
        differentiated_loop < arity
            && protected_denominators.iter().all(|&i| i < arity)
            && protected_denominators[0] != protected_denominators[1],
        "invalid loop or repeated/out-of-range protected axes",
    )?;
    let inputs = array(&r["nomination"], "contractions")?;
    require(
        inputs.len() == 3,
        "exactly three contraction directions required",
    )?;
    let mut contractions = [ContractionMomentum::Loop(0); 3];
    for (i, input) in inputs.iter().enumerate() {
        let object = input
            .as_object()
            .ok_or("typed contraction object required")?;
        require(
            object.len() == 2 && object.contains_key("kind") && object.contains_key("index"),
            "contraction requires only kind and index",
        )?;
        let ordinal = index(&input["index"])?;
        contractions[i] = match input["kind"].as_str() {
            Some("loop") => ContractionMomentum::Loop(ordinal),
            Some("external") => ContractionMomentum::External(ordinal),
            _ => return Err("unknown contraction kind".into()),
        };
        require(
            !contractions[..i].contains(&contractions[i]),
            "repeated contraction direction",
        )?;
    }
    Ok(TangentSourceSpec {
        differentiated_loop,
        protected_denominators,
        contractions,
        recenter: checked(IntegralShift::try_new(integers(
            &r["nomination"]["recenter"],
            arity,
        )?))?,
        multiplier: None,
    })
}

fn limits(r: &Value, arity: usize) -> Result<TangentSourceLimits> {
    Ok(TangentSourceLimits {
        exact_algebra: policies(r)?.cell.indexed_algebra.exact_algebra,
        max_denominators: arity,
        max_polynomial_terms: limit(r, "max_polynomial_terms")?,
        // Native verification multiplies degree-one derivatives by degree-two
        // minors. This is an intermediate allowance, NOT degree-three sources.
        max_polynomial_degree: 3,
        max_exponent_entries: limit(r, "max_coordinate_cells")?,
        max_term_operations: limit(r, "max_term_operations")?,
        max_selected_sources: limit(r, "max_source_rows")?,
        max_product_terms: limit(r, "max_terms")?,
        max_conditions: limit(r, "max_conditions")?,
        max_exact_operations: limit(r, "max_term_operations")?,
        max_retained_coordinate_cells: limit(r, "max_coordinate_cells")?,
    })
}

pub(super) fn nominate(
    family: &IntegralFamily,
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    r: &Value,
) -> Result<MaterializedNomination> {
    let spec = parse(r, family.denominator_count())?;
    let plan = checked(TangentSourcePlan::try_new(
        family,
        &spec,
        limits(r, family.denominator_count())?,
    ))?;
    let maximum_degree = plan
        .vector_coefficients()
        .iter()
        .flat_map(|p| {
            p.terms()
                .map(|(_, powers)| powers.iter().map(|&p| u64::from(p)).sum::<u64>())
        })
        .max()
        .unwrap_or(0);
    require(
        maximum_degree <= 2,
        "unmultiplied tangent emitted a degree above two",
    )?;
    let combination =
        checked(plan.materialize(generator, completed, policies(r)?.translated_sources))?;
    let sources = combination.sources().sources();
    require(
        sources.len() == combination.weights().len(),
        "native source/weight length mismatch",
    )?;
    let mut weights = BTreeMap::new();
    for (source, weight) in sources.iter().zip(combination.weights()) {
        let provenance = source.provenance();
        require(
            weights
                .insert(
                    (provenance.source_row().clone(), provenance.offset().clone()),
                    weight.clone(),
                )
                .is_none(),
            "native tangent repeated a named source/offset",
        )?;
    }
    require(
        !weights.is_empty(),
        "native tangent has no source contributions",
    )?;
    // Materialize retains every original guard, original coefficient pole and
    // weight pole BEFORE the product is coalesced. Family domain guards already
    // include denominator-basis/input poles; no nonexistent intermediate-pole
    // getter or unproved guard reconstruction is invented here.
    let conditions = combination
        .conditions()
        .iter()
        .map(|g| g.polynomial().clone())
        .collect();
    let source_report = weights.iter().map(|((row, offset), weight)| json!({
        "source_row":row.stable_string(),"offset":offset.values(),"weight":coefficient(weight)
    })).collect::<Vec<_>>();
    let report = json!({
        "method":"native-two-protected-tangent-plan-v1",
        "differentiated_loop":spec.differentiated_loop,
        "protected_denominators":spec.protected_denominators,
        "contractions":r["nomination"]["contractions"],
        "recenter":spec.recenter.values(),
        "multiplier":null,
        "vector_component_term_counts":plan.vector_coefficients().iter().map(|p|p.term_count()).collect::<Vec<_>>(),
        "maximum_emitted_polynomial_degree":maximum_degree,
        "maximum_intermediate_polynomial_degree":3,
        "sources":source_report,
        "materialized_full_product_terms":combination.product().len(),
        "materialized_pre_cancellation_conditions":combination.conditions().len(),
        "native_coefficients_kept_in_process":true,
        "full_product_equality_required":true,
        "mixed_tail_terms_suppressed":false,
        "nomination_is_source_proof":false,
        "target_or_descent_assumed":false,
    });
    Ok(MaterializedNomination {
        nomination: Nomination {
            weights,
            conditions,
            report,
        },
        full_product: combination.product().clone(),
    })
}

#[cfg(test)]
#[path = "geometry_two_protected_tests.rs"]
mod tests;
