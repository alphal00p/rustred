use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::algebra::Coefficient;
use crate::family::IntegralFamily;

use super::super::{
    ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasStatistics, products,
};
use super::model::{Support, VacuumPolynomial};

/// Check the physical assumptions before a nonzero determinant is interpreted
/// as full rank. Unsupported inactive denominators are deliberately irrelevant.
pub(super) fn prepare(
    family: &IntegralFamily,
    slots: &[usize],
    full_u: &VacuumPolynomial,
    variables: &Arc<Vec<PolyVariable>>,
    momenta: &mut [Option<Result<Vec<Coefficient>, Skip>>],
    statistics: &mut TerminalAliasStatistics,
) -> Result<Result<Arc<Support>, Skip>, Error> {
    for &slot in slots {
        if momenta[slot].is_none() {
            statistics.analyzed_denominators += 1;
            momenta[slot] = Some(products::momentum(family, slot, variables)?);
        }
        if let Err(reason) = momenta[slot].as_ref().expect("cached momentum") {
            return Ok(Err(*reason));
        }
    }
    let mut u = full_u.clone();
    let zero = family.coefficient_context().zero();
    for slot in 0..family.denominator_count() {
        if slots.binary_search(&slot).is_err() {
            u = u.replace(slot, &zero);
        }
    }
    if u.is_zero() {
        return Ok(Err(Skip::SingularMomentumBasis));
    }
    // Native substitution retains the complete variable map. Condense only
    // now, keeping explicit positional ownership of every active parameter.
    let active_variables: Vec<_> = slots
        .iter()
        .map(|&slot| full_u.variables()[slot].clone())
        .collect();
    u = u
        .rearrange_with_growth(&active_variables)
        .map_err(Error::ExactAlgebra)?;
    for term in 0..u.nterms() {
        let Some(coefficient) =
            products::integer(&u.coefficients[term], family.coefficient_context())?
        else {
            return Err(Error::InvalidParametricWitness);
        };
        // Cauchy–Binet for integer real q: each nonzero coefficient is a
        // positive squared minor; rank-L terms are squarefree of degree L.
        if coefficient <= 0_i32
            || u.exponents(term).iter().any(|&power| power > 1)
            || u.exponents(term)
                .iter()
                .map(|&power| usize::from(power))
                .sum::<usize>()
                != family.loop_count()
        {
            return Err(Error::InvalidParametricWitness);
        }
    }
    Ok(Ok(Arc::new(Support {
        slots: slots.to_vec(),
        u,
    })))
}
