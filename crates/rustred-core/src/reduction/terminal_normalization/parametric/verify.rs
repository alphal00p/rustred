use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::family::{IntegralFamily, IntegralKey};

use super::super::TerminalAliasError as Error;
use super::model::{Support, VerifiedVacuumParameterMap};

pub(super) fn prove(
    family: &IntegralFamily,
    source_key: &IntegralKey,
    representative_key: &IntegralKey,
    source: Arc<Support>,
    representative: Arc<Support>,
    permutation: Vec<usize>,
) -> Result<VerifiedVacuumParameterMap, Error> {
    let count = source.slots.len();
    if count != representative.slots.len() || permutation.len() != count {
        return Err(Error::InvalidParametricWitness);
    }
    let mut seen = vec![false; count];
    for (from, &to) in permutation.iter().enumerate() {
        if to >= count
            || std::mem::replace(&mut seen[to], true)
            || source_key.powers()[source.slots[from]]
                != representative_key.powers()[representative.slots[to]]
        {
            return Err(Error::InvalidParametricWitness);
        }
    }
    // Public native rename_variable alters only the map. Two phases prevent
    // cycles such as x1→x2→x1 from identifying distinct parameters. Native
    // rearrange_with_growth then puts the exact monomials into the same map.
    let source_variables = source.u.variables();
    let target_variables = representative.u.variables();
    let temporary: Vec<_> = (0..count).map(PolyVariable::Temporary).collect();
    if temporary
        .iter()
        .any(|v| source_variables.contains(v) || target_variables.contains(v))
    {
        return Err(Error::InvalidParametricWitness);
    }
    let mut renamed = source.u.clone();
    for (from, temp) in source_variables.iter().zip(&temporary) {
        renamed.rename_variable(from, temp);
    }
    for (from, &to) in permutation.iter().enumerate() {
        renamed.rename_variable(&temporary[from], &target_variables[to]);
    }
    let renamed = renamed
        .rearrange_with_growth(target_variables)
        .map_err(Error::ExactAlgebra)?;
    // Symbolica polynomial equality intentionally ignores constant maps;
    // bind both outer variables and every coefficient context explicitly.
    if renamed.variables() != representative.u.variables()
        || renamed
            .coefficients
            .iter()
            .chain(&representative.u.coefficients)
            .any(|c| !family.coefficient_context().contains(c))
        || renamed != representative.u
    {
        return Err(Error::InvalidParametricWitness);
    }
    Ok(VerifiedVacuumParameterMap {
        loops: family.loop_count(),
        source,
        representative,
        permutation,
    })
}
