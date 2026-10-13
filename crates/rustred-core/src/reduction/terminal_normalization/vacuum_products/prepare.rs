use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use symbolica::prelude::PolyVariable;

use super::canonical::{CanonicalFactor, canonical_factor, combinations, unimodular_transform};
use super::{
    VacuumProductError as Error, VacuumProductLimits, VacuumProductPlan, VacuumProductStatistics,
    VerifiedVacuumProduct, algebra, charge,
};
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_normalization::{VacuumIntegralKey, products};

impl VacuumProductPlan {
    /// Discover exact independent momentum blocks from active denominator
    /// squares. Only nonnegative unshifted unit-mass vacuum keys are eligible.
    /// Each lower factor is an unevaluated integral, never a newly inferred master.
    pub fn prepare(
        family: &IntegralFamily,
        terminals: &BTreeSet<IntegralKey>,
        limits: VacuumProductLimits,
    ) -> Result<Self, Error> {
        if terminals.len() > limits.max_terminals {
            return Err(Error::ResourceLimit {
                resource: "terminals",
                requested: terminals.len(),
                limit: limits.max_terminals,
            });
        }
        for key in terminals {
            if key.powers().len() != family.denominator_count() {
                return Err(Error::InvalidInput("terminal has foreign arity".into()));
            }
        }
        let mut plan = Self {
            family_fingerprint: family.fingerprint_owner(),
            products: BTreeMap::new(),
            families: BTreeMap::new(),
            statistics: VacuumProductStatistics {
                requested_terminals: terminals.len(),
                retained_terminals: terminals.len(),
                ..Default::default()
            },
        };
        if family.external_count() != 0
            || family.power_shifts().iter().any(|shift| !shift.is_zero())
            || family.loop_count() < 2
        {
            return Ok(plan);
        }
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta = BTreeMap::new();
        let mut charts: BTreeMap<Vec<usize>, Option<Vec<CanonicalFactor>>> = BTreeMap::new();
        for key in terminals {
            if key.powers().iter().any(|&power| power < 0) {
                plan.statistics.numerator_terminals += 1;
                continue;
            }
            let active: Vec<usize> = key
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(i, &power)| (power > 0).then_some(i))
                .collect();
            if !charts.contains_key(&active) {
                plan.statistics.analyzed_supports += 1;
                let mut rows = Vec::new();
                for &slot in &active {
                    if !momenta.contains_key(&slot) {
                        momenta.insert(
                            slot,
                            products::momentum(family, slot, &variables)
                                .map_err(algebra)?
                                .ok(),
                        );
                    }
                    let Some(row) = &momenta[&slot] else {
                        break;
                    };
                    rows.push(row.clone());
                }
                let chart = if rows.len() != active.len() {
                    None
                } else {
                    discover(family, &active, &rows, limits, &mut plan.statistics)?
                };
                charts.insert(active.clone(), chart);
            }
            let Some(chart) = &charts[&active] else {
                continue;
            };
            let mut factors = Vec::new();
            for block in chart {
                if !plan.families.contains_key(block.family.fingerprint()) {
                    let mut count = plan.families.len();
                    charge(&mut count, limits.max_lower_families, "lower families")?;
                    plan.families
                        .insert(block.family.fingerprint().to_owned(), block.family.clone());
                }
                let mut powers = block
                    .alignments
                    .iter()
                    .map(|alignment| {
                        alignment
                            .iter()
                            .map(|&slot| key.powers()[slot])
                            .collect::<Vec<_>>()
                    })
                    .min()
                    .ok_or_else(|| {
                        Error::InvalidInput("canonical factor lacks a verified alignment".into())
                    })?;
                powers.resize(block.family.denominator_count(), 0);
                factors.push(VacuumIntegralKey::from_family(
                    &block.family,
                    IntegralKey::try_new(powers).map_err(algebra)?,
                ));
            }
            factors.sort();
            plan.products.insert(
                key.clone(),
                VerifiedVacuumProduct {
                    coefficient: family.coefficient_context().one(),
                    factors,
                },
            );
        }
        plan.statistics.factored_terminals = plan.products.len();
        plan.statistics.retained_terminals = terminals.len() - plan.products.len();
        Ok(plan)
    }
}

fn discover(
    family: &IntegralFamily,
    active: &[usize],
    rows: &[Vec<Coefficient>],
    limits: VacuumProductLimits,
    stats: &mut VacuumProductStatistics,
) -> Result<Option<Vec<CanonicalFactor>>, Error> {
    let loops = family.loop_count();
    if active.len() < loops {
        return Ok(None);
    }
    let mut transformed = None;
    combinations(rows.len(), loops, &mut |selection| {
        charge(
            &mut stats.basis_candidates,
            limits.max_basis_candidates,
            "basis candidates",
        )?;
        if let Some(value) = unimodular_transform(family, rows, selection)? {
            transformed = Some(value);
            Ok(false)
        } else {
            Ok(true)
        }
    })?;
    let Some(rows) = transformed else {
        return Ok(None);
    };
    // Fundamental momentum supports give the direct-sum components. A full
    // basis was verified above, so every loop belongs to at least one row.
    let mut groups = (0..loops).map(|i| BTreeSet::from([i])).collect::<Vec<_>>();
    for row in &rows {
        let support: BTreeSet<_> = row
            .iter()
            .enumerate()
            .filter_map(|(i, value)| (!value.is_zero()).then_some(i))
            .collect();
        let mut combined = support.clone();
        groups.retain(|group| {
            if group.is_disjoint(&support) {
                true
            } else {
                combined.extend(group);
                false
            }
        });
        groups.push(combined);
    }
    if groups.len() < 2 {
        return Ok(None);
    }
    groups.sort();
    let mut chart = Vec::new();
    let mut covered = BTreeSet::new();
    for group in groups {
        let mut block_rows = Vec::new();
        let mut slots = Vec::new();
        for (row, &slot) in rows.iter().zip(active) {
            if group.iter().any(|&axis| !row[axis].is_zero()) {
                if row
                    .iter()
                    .enumerate()
                    .any(|(axis, value)| !group.contains(&axis) && !value.is_zero())
                    || !covered.insert(slot)
                {
                    return Err(Error::InvalidInput(
                        "proposed momentum blocks overlap".into(),
                    ));
                }
                slots.push(slot);
                block_rows.push(group.iter().map(|&axis| row[axis].clone()).collect());
            }
        }
        chart.push(canonical_factor(
            family,
            &slots,
            &block_rows,
            limits,
            stats,
        )?);
    }
    if covered.len() != active.len() {
        return Err(Error::InvalidInput(
            "momentum blocks omit an active denominator".into(),
        ));
    }
    Ok(Some(chart))
}
