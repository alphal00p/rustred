use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use symbolica::prelude::PolyVariable;

use super::super::prepare::{SupportChart, discover};
use super::super::{VacuumProductError as Error, VacuumProductStatistics, algebra, charge};
use super::{
    VacuumProductChartLimits, VacuumProductChartPlan, VacuumProductComponent,
    VerifiedVacuumProductChart,
};
use crate::algebra::matrix::multiply_coefficient_matrices;
use crate::family::{
    AffineDenominator, IntegralFamily, IntegralKey, ScalarProductCoordinate,
    symbolica_matrix_limits,
};
use crate::reduction::terminal_normalization::products;
use crate::sector::{
    Mask,
    symmetry::{self, CoefficientMatrix, MomentumMap, integral_transport},
};

impl VacuumProductChartPlan {
    pub fn prepare(
        family: &IntegralFamily,
        terminals: &BTreeSet<IntegralKey>,
        limits: VacuumProductChartLimits,
    ) -> Result<Self, Error> {
        if limits.products.diagnostics_enabled && terminals.len() > limits.products.max_terminals {
            return Err(Error::ResourceLimit {
                resource: "chart terminals",
                requested: terminals.len(),
                limit: limits.products.max_terminals,
            });
        }
        for key in terminals {
            if key.powers().len() != family.denominator_count() {
                return Err(Error::InvalidInput(
                    "chart terminal has foreign arity".into(),
                ));
            }
        }
        let mut plan = Self {
            charts: BTreeMap::new(),
            statistics: VacuumProductStatistics {
                requested_terminals: terminals.len(),
                retained_terminals: terminals.len(),
                ..Default::default()
            },
        };
        if family.external_count() != 0
            || family.power_shifts().iter().any(|p| !p.is_zero())
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
        let mut supports = BTreeMap::<Vec<usize>, Option<SupportChart>>::new();
        let mut lower_families = BTreeSet::new();
        for key in terminals {
            if key.powers().iter().any(|&p| p < 0) {
                plan.statistics.numerator_terminals += 1;
            }
            let active = key
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| (p > 0).then_some(i))
                .collect::<Vec<_>>();
            if !supports.contains_key(&active) {
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
                    discover(
                        family,
                        &active,
                        &rows,
                        limits.products,
                        &mut plan.statistics,
                    )?
                };
                supports.insert(active.clone(), chart);
            }
            let Some(support) = &supports[&active] else {
                continue;
            };
            for block in &support.blocks {
                if !lower_families.contains(block.family.fingerprint()) {
                    let mut count = lower_families.len();
                    charge(
                        &mut count,
                        limits
                            .products
                            .diagnostic_limit(limits.products.max_lower_families),
                        "chart lower families",
                    )?;
                    lower_families.insert(block.family.fingerprint().to_owned());
                }
            }
            plan.charts
                .insert(key.clone(), make_chart(family, key, support, limits)?);
        }
        plan.statistics.factored_terminals = plan.charts.len();
        plan.statistics.retained_terminals -= plan.charts.len();
        Ok(plan)
    }
}

fn make_chart(
    parent: &IntegralFamily,
    key: &IntegralKey,
    support: &SupportChart,
    limits: VacuumProductChartLimits,
) -> Result<VerifiedVacuumProductChart, Error> {
    let context = parent.coefficient_context();
    let loops = parent.loop_count();
    let mut components = Vec::new();
    let mut denominators = Vec::new();
    let mut all_powers = Vec::new();
    let mut block_map = vec![vec![context.zero(); loops]; loops];
    let mut loop_start = 0;
    for (block, group) in support.blocks.iter().zip(&support.groups) {
        // Same power-minimizing alignment as the positive-only plan, with the
        // matching momentum map rather than an independent power permutation.
        let (alignment, transform) = block
            .alignments
            .iter()
            .min_by_key(|(slots, _)| {
                slots
                    .iter()
                    .map(|&slot| key.powers()[slot])
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| Error::InvalidInput("component has no canonical alignment".into()))?;
        let block_loops = block.family.loop_count();
        for (row, &old_axis) in group.iter().enumerate() {
            for column in 0..block_loops {
                block_map[old_axis][loop_start + column] = transform[row][column].clone();
            }
        }
        let mut powers = alignment
            .iter()
            .map(|&slot| key.powers()[slot])
            .collect::<Vec<_>>();
        powers.resize(block.family.denominator_count(), 0);
        let denominator_start = denominators.len();
        for denominator in block.family.denominators() {
            let mut coefficients = vec![context.zero(); parent.coordinates().len()];
            for (coordinate, coefficient) in block
                .family
                .coordinates()
                .iter()
                .zip(denominator.coefficients())
            {
                let ScalarProductCoordinate::LoopLoop { left, right } = *coordinate else {
                    unreachable!("vacuum lower block");
                };
                let target = coordinate_index(loops, loop_start + left, loop_start + right);
                coefficients[target] = coefficient.clone();
            }
            denominators.push(AffineDenominator::new(
                denominator.constant().clone(),
                coefficients,
            ));
        }
        all_powers.extend(&powers);
        components.push(VacuumProductComponent {
            family: block.family.clone(),
            base: IntegralKey::try_new(powers).map_err(algebra)?,
            loops: loop_start..loop_start + block_loops,
            denominators: denominator_start..denominators.len(),
        });
        loop_start += block_loops;
    }
    if loop_start != loops {
        return Err(Error::InvalidInput(
            "components omit loop coordinates".into(),
        ));
    }
    // Complete the common-mass family with cross-block ISP coordinates. These
    // have zero base powers and MUST vanish from the projected output keys.
    for (i, left) in components.iter().enumerate() {
        for right in &components[i + 1..] {
            for a in left.loops.clone() {
                for b in right.loops.clone() {
                    let mut row = vec![context.zero(); parent.coordinates().len()];
                    row[coordinate_index(loops, a, a)] = context.one();
                    row[coordinate_index(loops, b, b)] = context.one();
                    row[coordinate_index(loops, a, b)] = context.integer(-2);
                    denominators.push(AffineDenominator::new(context.integer(-1), row));
                    all_powers.push(0);
                }
            }
        }
    }
    let combined = Arc::new(
        IntegralFamily::new_with_limits(
            "unit_mass_component_chart",
            (0..loops).map(|i| format!("q{i}")).collect(),
            vec![],
            context.clone(),
            parent.dimension().clone(),
            denominators,
            vec![],
            vec![context.zero(); all_powers.len()],
            parent.construction_limits(),
        )
        .map_err(algebra)?,
    );
    let momentum = multiply_coefficient_matrices(
        context,
        &support.inverse,
        &block_map,
        symbolica_matrix_limits(parent.construction_limits()),
    )
    .map_err(algebra)?
    .0;
    let map = Arc::new(
        symmetry::verify(
            parent,
            &combined,
            MomentumMap::new(
                if limits.verification.diagnostics_enabled {
                    CoefficientMatrix::try_new_with_max_entries(
                        loops,
                        loops,
                        momentum.into_iter().flatten(),
                        limits.verification.max_matrix_entries,
                    )
                } else {
                    CoefficientMatrix::try_new(loops, loops, momentum.into_iter().flatten())
                }
                .map_err(algebra)?,
                CoefficientMatrix::try_new(loops, 0, []).map_err(algebra)?,
                CoefficientMatrix::try_new(0, 0, []).map_err(algebra)?,
            ),
            limits.verification,
        )
        .map_err(algebra)?,
    );
    let base = IntegralKey::try_new(all_powers).map_err(algebra)?;
    let transport = integral_transport::compile(
        parent,
        combined.clone(),
        map,
        Mask::try_from_indices(key.powers()).map_err(algebra)?,
        Mask::try_from_indices(base.powers()).map_err(algebra)?,
        limits.transport,
    )
    .map_err(algebra)?;
    Ok(VerifiedVacuumProductChart {
        source: key.clone(),
        combined,
        base,
        components,
        transport,
        limits,
    })
}

fn coordinate_index(loops: usize, left: usize, right: usize) -> usize {
    debug_assert!(left <= right && right < loops);
    left * loops - left * left.saturating_sub(1) / 2 + right - left
}
