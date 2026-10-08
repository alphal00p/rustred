//! Prepare-once scalar vacuum relations from native diagonal ordinary IBPs.
//!
//! Exact full-U aliases identify columns across families. All other columns
//! remain auxiliary obligations. This finite plan is neither a global Laporta
//! solver nor a minimality certificate, and does not alter single-family
//! terminal sessions. Symbolica owns elimination and source replay.

#[cfg(test)]
mod audit_tests;
mod elimination;
mod model;
mod prepare;
mod saved;
#[cfg(test)]
mod tests;

pub use model::{
    GuardedVacuumReduction, VacuumCollectionEquation, VacuumCollectionError,
    VacuumCollectionStatistics, VacuumDiagonalCollectionLimits, VacuumDiagonalCollectionPlan,
    VacuumDiagonalSource, VacuumOrdinarySource,
};
pub use saved::{TerminalCollectionLimits, TerminalCollectionPlan, TerminalCollectionStatistics};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::identity::RowId;
use crate::reduction::terminal_normalization::{VacuumFamilyAliasPlan, VacuumIntegralKey};

type Row = BTreeMap<VacuumIntegralKey, Coefficient>;
type Result<T> = std::result::Result<T, VacuumCollectionError>;

fn check(resource: &'static str, requested: usize, limit: usize) -> Result<()> {
    if requested > limit {
        Err(VacuumCollectionError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
fn algebra(error: impl std::fmt::Display) -> VacuumCollectionError {
    VacuumCollectionError::Algebra(error.to_string())
}
fn retain(conditions: &mut Vec<Coefficient>, value: Coefficient, limit: usize) -> Result<()> {
    if value.is_zero() {
        return Err(VacuumCollectionError::VanishingCondition);
    }
    if !value.is_constant() && !conditions.contains(&value) {
        check(
            "nonzero conditions",
            conditions.len().saturating_add(1),
            limit,
        )?;
        conditions.push(value);
    }
    Ok(())
}
fn accumulate<K: Ord>(
    row: &mut BTreeMap<K, Coefficient>,
    key: K,
    value: &Coefficient,
    context: &CoefficientContext,
    limits: VacuumDiagonalCollectionLimits,
) -> Result<()> {
    let old = row.remove(&key).unwrap_or_else(|| context.zero());
    let sum = context
        .try_add(&old, value, limits.algebra.exact_algebra)
        .map_err(algebra)?;
    if !sum.is_zero() {
        row.insert(key, sum);
    }
    Ok(())
}
