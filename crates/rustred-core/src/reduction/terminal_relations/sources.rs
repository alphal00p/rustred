//! Existing exact ordinary-IBP generation, instantiated at finite integer seeds.
use super::*;
use crate::algebra::IndexedCoefficientContext;
use crate::identity::{ParametricIbpGenerator, ParametricRelation};

pub(super) struct Sources {
    context: IndexedCoefficientContext,
    rows: Vec<ParametricRelation>,
}
impl Sources {
    pub fn new(family: &IntegralFamily) -> Result<Self, TerminalRelationError> {
        let generator = ParametricIbpGenerator::try_new(family).map_err(algebra)?;
        let context = generator.context().clone();
        let batch = generator.prepare_ordinary_ibp().map_err(algebra)?;
        let generated = (0..batch.len()).map(|i| batch.generate(i)).collect();
        let rows = batch.complete(generated).map_err(algebra)?.into_relations();
        Ok(Self { context, rows })
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub(super) fn row_id(&self, ordinal: usize) -> Option<&crate::identity::RowId> {
        self.rows.get(ordinal).map(ParametricRelation::row_id)
    }
    pub fn row(
        &self,
        seed: &IntegralKey,
        ordinal: usize,
        limits: IndexedAlgebraLimits,
    ) -> Result<(TerminalRelationRow, Vec<Coefficient>), TerminalRelationError> {
        let source = self
            .rows
            .get(ordinal)
            .ok_or_else(|| invalid("source row is out of range"))?;
        let mut row = TerminalRelationRow::new();
        let mut conditions = Vec::new();
        for condition in source.nonzero_conditions() {
            let value = self
                .context
                .specialize_polynomial(condition.polynomial(), seed.powers(), limits)
                .map_err(algebra)?;
            if value.is_zero() {
                return Err(invalid("ordinary source condition vanishes at seed"));
            }
            if !value.is_constant() {
                conditions.push(value.into());
            }
        }
        for (shift, value) in source.terms() {
            let (coefficient, pole) = self
                .context
                .specialize(value, seed.powers(), limits)
                .map_err(algebra)?;
            if let Some(pole) = pole {
                conditions.push(pole.into());
            }
            if coefficient.is_zero() {
                continue;
            }
            let values = seed
                .powers()
                .iter()
                .zip(shift.values())
                .map(|(a, b)| {
                    a.checked_add(*b)
                        .ok_or_else(|| invalid("seed source integral power overflow"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let key = IntegralKey::try_new(values).map_err(invalid)?;
            let old = row
                .remove(&key)
                .unwrap_or_else(|| self.context.base().zero());
            let coefficient = self
                .context
                .base()
                .try_add(&old, &coefficient, limits.exact_algebra)
                .map_err(algebra)?;
            if !coefficient.is_zero() {
                row.insert(key, coefficient);
            }
        }
        Ok((row, conditions))
    }
}

/// Finite graph-distance expansion equals the signed-L1 balls around all
/// centres. It never clips descendants by numerator rank or sector support.
/// Breadth-first order, then lexical key order, is deterministic. Duplicated
/// seeds are shared between centres instead of regenerating their equations.
pub(super) fn seeds(
    centres: &BTreeSet<IntegralKey>,
    depth: u32,
    limit: usize,
) -> Result<Vec<IntegralKey>, TerminalRelationError> {
    let mut seen = centres.clone();
    check("seeds", seen.len(), limit)?;
    let mut result: Vec<_> = seen.iter().cloned().collect();
    let mut frontier = seen.clone();
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for seed in frontier {
            for slot in 0..seed.powers().len() {
                for shift in [-1, 1] {
                    let mut powers = seed.powers().to_vec();
                    powers[slot] = powers[slot]
                        .checked_add(shift)
                        .ok_or_else(|| invalid("seed power overflow"))?;
                    let key = IntegralKey::try_new(powers).map_err(invalid)?;
                    if !seen.contains(&key) {
                        next.insert(key);
                        check("seeds", seen.len().saturating_add(next.len()), limit)?;
                    }
                }
            }
        }
        result.extend(next.iter().cloned());
        seen.extend(next.iter().cloned());
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(result)
}

/// Direct support promotions, rather than a larger signed-power shell. Source
/// specialization remains the ordinary generator above, with all its guards.
pub(super) fn containing_sector_seeds(
    centres: &BTreeSet<IntegralKey>,
    max_promoted_axes: usize,
    limit: usize,
) -> Result<BTreeSet<IntegralKey>, TerminalRelationError> {
    fn visit(
        powers: &mut [i64],
        inactive: &[usize],
        start: usize,
        remaining: usize,
        output: &mut BTreeSet<IntegralKey>,
        limit: usize,
    ) -> Result<(), TerminalRelationError> {
        for position in start..inactive.len() {
            let axis = inactive[position];
            let previous = powers[axis];
            powers[axis] = 1;
            output.insert(IntegralKey::try_new(powers.to_vec()).map_err(invalid)?);
            check("seeds", output.len(), limit)?;
            if remaining > 1 {
                visit(powers, inactive, position + 1, remaining - 1, output, limit)?;
            }
            powers[axis] = previous;
        }
        Ok(())
    }

    let mut output = BTreeSet::new();
    if max_promoted_axes != 0 {
        for centre in centres {
            let inactive: Vec<_> = centre
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(axis, power)| (*power <= 0).then_some(axis))
                .collect();
            visit(
                &mut centre.powers().to_vec(),
                &inactive,
                0,
                max_promoted_axes,
                &mut output,
                limit,
            )?;
        }
    }
    Ok(output)
}
