use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::PhysicalFramePlan;
use crate::sector::SectorInteriorDomain;

use super::super::ExactTargetCircuit;
use super::ExactCircuitLoweringError;
use super::resource::try_vec;

pub(super) fn validate_plan_and_circuit(
    context: &IndexedCoefficientContext,
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
) -> Result<(), ExactCircuitLoweringError> {
    if !circuit.is_bound_to(plan) {
        return Err(ExactCircuitLoweringError::WrongPhysicalPlan);
    }
    if context.fingerprint_owner().as_str() != plan.context_fingerprint() {
        return Err(ExactCircuitLoweringError::WrongContext);
    }
    if plan.columns().is_empty() || plan.sector().arity() != context.index_count() {
        return Err(ExactCircuitLoweringError::TargetJoin(
            "plan arity or columns are invalid",
        ));
    }
    let target = plan.columns().get(circuit.target_column()).ok_or(
        ExactCircuitLoweringError::TargetJoin("target column is outside the plan"),
    )?;
    if target != circuit.target_shift() {
        return Err(ExactCircuitLoweringError::TargetJoin(
            "target shift differs from the physical column",
        ));
    }
    if circuit.residual_terms().is_empty() {
        return Err(ExactCircuitLoweringError::EmptyRightHandSide);
    }

    let mut previous = None;
    let first = circuit.residual_terms()[0].descent();
    if !first.verify() || first.domain().sector() != plan.sector() {
        return Err(ExactCircuitLoweringError::ResidualJoin {
            term: 0,
            detail: "sector-monotone witness is invalid or belongs to another sector",
        });
    }
    for (ordinal, term) in circuit.residual_terms().iter().enumerate() {
        if previous.is_some_and(|column| column >= term.physical_column()) {
            return Err(ExactCircuitLoweringError::ResidualJoin {
                term: ordinal,
                detail: "physical residual columns are not strictly ordered",
            });
        }
        previous = Some(term.physical_column());
        let shift = plan.columns().get(term.physical_column()).ok_or(
            ExactCircuitLoweringError::ResidualJoin {
                term: ordinal,
                detail: "physical residual column is outside the plan",
            },
        )?;
        if shift != term.shift() || term.coefficient().is_zero() {
            return Err(ExactCircuitLoweringError::ResidualJoin {
                term: ordinal,
                detail: "shift join failed or coefficient is zero",
            });
        }
        let descent = term.descent();
        if !descent.verify()
            || descent.policy() != first.policy()
            || descent.domain() != first.domain()
            || !key_matches(descent.pivot(), circuit.target_shift().values())
            || !key_matches(descent.target(), term.shift().values())
        {
            return Err(ExactCircuitLoweringError::ResidualJoin {
                term: ordinal,
                detail: "strict-descent evidence does not join the exact shifts",
            });
        }
    }
    Ok(())
}

pub(super) fn build_proof_domain(
    plan: &PhysicalFramePlan,
    circuit: &ExactTargetCircuit,
) -> Result<SectorInteriorDomain, ExactCircuitLoweringError> {
    if !circuit.fixed_indices().is_empty() {
        let stratum = circuit.residual_terms()[0].descent().domain();
        return Ok(SectorInteriorDomain::try_new(
            stratum.sector().clone(),
            stratum.bounds().iter().copied(),
        )?);
    }
    let count = circuit.residual_terms().len().checked_add(1).ok_or(
        ExactCircuitLoweringError::ResourceCountOverflow {
            resource: "proof-domain shifts",
        },
    )?;
    let mut shifts = try_vec("proof-domain shifts", count)?;
    shifts.push(circuit.target_shift().values());
    shifts.extend(
        circuit
            .residual_terms()
            .iter()
            .map(|term| term.shift().values()),
    );
    match SectorInteriorDomain::try_maximal_for_shifts(plan.sector().clone(), &shifts) {
        Ok(domain) => Ok(domain),
        Err(crate::sector::Error::EmptyShiftInterior { .. }) => {
            Err(ExactCircuitLoweringError::NoCommonSectorInterior)
        }
        Err(error) => Err(ExactCircuitLoweringError::Sector(error)),
    }
}

fn key_matches(key: &crate::sector::ShiftComplexityKey, shift: &[i64]) -> bool {
    key.arity() == shift.len()
        && shift
            .iter()
            .enumerate()
            .all(|(position, &value)| key.shift_at(position) == Ok(value))
}
