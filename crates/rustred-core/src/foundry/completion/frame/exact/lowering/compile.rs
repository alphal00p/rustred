use crate::algebra::IndexedCoefficientContext;
use crate::foundry::cell::SourceViewBatch;
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleTerm, SectorMonotoneDependency, SectorMonotoneTargetAdmission,
    verify_concrete_specialization_replay,
};

use super::super::ExactTargetCircuit;
use super::guards::{compile_guards, preflight_guards};
use super::join::{build_proof_domain, validate_plan_and_circuit};
use super::preflight::{preflight_circuit_payload, preflight_source_view_and_rule};
use super::replay::independently_replay_full_span;
use super::resource::try_vec;
use super::source::{
    canonical_source_shift, clone_selected_sources, collect_selected_rows, compile_pivot_guards,
    compile_source_combination, compile_source_shift_columns,
};
use super::{
    ExactCircuitLoweringError, ExactCircuitLoweringLimits, ExactCircuitLoweringSeal,
    LoweredExactCircuit,
};
use crate::foundry::completion::frame::PhysicalFramePlan;

/// Losslessly lower one live-plan-bound exact circuit into the existing
/// source-view/parametric-rule representation.
///
/// `anchor` is explicit because modular discovery retains only a modular
/// fingerprint, whereas `ParametricRule` deliberately requires an exact
/// concrete specialization replay. This function creates no completion
/// authority and makes no closure claim.
pub(crate) fn try_lower_exact_circuit(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
    anchor: &[i64],
    limits: ExactCircuitLoweringLimits,
) -> Result<LoweredExactCircuit, ExactCircuitLoweringError> {
    validate_plan_and_circuit(context, plan, circuit)?;
    preflight_circuit_payload(context, circuit, limits)?;
    preflight_guards(circuit, limits)?;
    let seal = ExactCircuitLoweringSeal::new();

    let selected_rows = collect_selected_rows(plan, circuit, limits)?;
    let (relations, provenance) =
        clone_selected_sources(&seal, context, plan, &selected_rows, limits)?;
    let source_shift_columns = compile_source_shift_columns(&relations, limits)?;
    preflight_source_view_and_rule(
        context,
        circuit,
        &selected_rows,
        &relations,
        &source_shift_columns,
        limits,
    )?;
    let sources = SourceViewBatch::try_from_exact_lowered_parts(
        &seal,
        plan.family_fingerprint_owner(),
        plan.context_fingerprint_owner(),
        relations,
        provenance,
    )?;

    let pivot = canonical_source_shift(
        plan,
        &source_shift_columns,
        circuit.target_column(),
        "target shift is absent from compact source-view columns",
    )?;
    let ordering = circuit.residual_terms()[0].descent().policy();
    let proof_domain = build_proof_domain(plan, circuit)?;
    let mut right_hand_side = try_vec("right-hand-side terms", circuit.residual_terms().len())?;
    let mut dependencies = try_vec(
        "sector-monotone dependencies",
        circuit.residual_terms().len(),
    )?;
    for (ordinal, term) in circuit.residual_terms().iter().enumerate() {
        let shift = canonical_source_shift(
            plan,
            &source_shift_columns,
            term.physical_column(),
            "residual shift is absent from compact source-view columns",
        )?;
        let coefficient = context.neg_with_limits(
            term.coefficient(),
            limits.parametric.indexed_algebra.exact_algebra,
        )?;
        let descent =
            ordering.prove_shift_strict_descent(&proof_domain, pivot.values(), shift.values())?;
        right_hand_side.push(ParametricRuleTerm::from_exact_lowering(
            &seal,
            shift.clone(),
            coefficient,
            descent,
        ));
        dependencies.push(SectorMonotoneDependency::from_exact_lowering(
            &seal,
            ordinal,
            pivot.clone(),
            shift,
            term.descent().clone(),
        ));
    }

    let monotone_domain = circuit.residual_terms()[0].descent().domain().clone();
    let sector_monotone_admission = SectorMonotoneTargetAdmission::from_exact_lowering(
        &seal,
        monotone_domain,
        pivot.clone(),
        dependencies,
    );
    if !sector_monotone_admission.verify() {
        return Err(ExactCircuitLoweringError::Invariant(
            "retained sector-monotone admission did not verify",
        ));
    }

    let source_combination =
        compile_source_combination(&seal, plan, circuit, &selected_rows, sources.relations())?;
    let pivot_guards = compile_pivot_guards(
        &seal,
        plan,
        circuit,
        &selected_rows,
        sources.relations(),
        &source_shift_columns,
    )?;
    let nonzero_guards = compile_guards(
        &seal,
        plan,
        circuit,
        &selected_rows,
        sources.relations(),
        &source_shift_columns,
    )?;
    let replay = independently_replay_full_span(
        &seal,
        context,
        plan,
        circuit,
        &selected_rows,
        &source_combination,
        sources.relations(),
        limits,
    )?;

    if anchor.len() != context.index_count() {
        return Err(ExactCircuitLoweringError::WrongAnchorArity {
            expected: context.index_count(),
            actual: anchor.len(),
        });
    }
    if !sector_monotone_admission.domain().contains(anchor)? {
        return Err(ExactCircuitLoweringError::AnchorOutsideMonotoneAdmission);
    }
    let concrete_replay = verify_concrete_specialization_replay(
        context,
        sources.relations(),
        anchor,
        &pivot,
        &right_hand_side,
        &nonzero_guards,
        &source_combination,
        limits.parametric,
    )?;

    let rule = ParametricRule::from_replayed_exact_parts(
        &seal,
        plan.family_fingerprint_owner(),
        plan.context_fingerprint_owner(),
        proof_domain,
        ordering,
        pivot,
        right_hand_side,
        pivot_guards,
        nonzero_guards,
        source_combination,
        replay,
        concrete_replay,
        sector_monotone_admission,
    );
    Ok(LoweredExactCircuit::new(rule, sources))
}
