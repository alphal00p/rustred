//! Optional numerator-aware component charts for the published flat terminals.
//! Exact geometry and projection remain core-owned; no formula is generated.
use std::time::{Duration, Instant};

use super::products::{check_count, merge_work};
use super::*;
use rustred::reduction::terminal_normalization::{
    VacuumProductChartLimits, VacuumProductChartPlan, VacuumProductStatistics,
    VerifiedVacuumProductChart,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedApplicationProductChartStatistics {
    /// Requested terminals are only the negative-power published terminals.
    /// `factored_terminals` counts verified charts, not completed projections.
    pub work: VacuumProductStatistics,
    pub lower_families: usize,
    pub preparation_time: Duration,
}

pub(super) struct TerminalProductCharts {
    pub(super) families: BTreeMap<String, Arc<IntegralFamily>>,
    plans: BTreeMap<String, VacuumProductChartPlan>,
    statistics: SavedApplicationProductChartStatistics,
    limits: VacuumProductChartLimits,
}

impl<const N: usize> PreparedSavedApplication<N> {
    /// Prepare numerator-aware geometry grouped by source family and positive
    /// support. Positive terminals use `prepare_terminal_products` instead.
    /// A chart retains all numerator transport; obtaining it does not discard
    /// cross-component couplings or establish their scalar tensor projection.
    /// Identical settings reuse the plan. Changed settings prepare a fresh plan
    /// transactionally, preserving the old one if preparation fails.
    pub fn prepare_terminal_product_charts(
        &mut self,
        limits: VacuumProductChartLimits,
    ) -> Result<SavedApplicationProductChartStatistics, AppError> {
        if let Some(prepared) = &self.terminal_product_charts {
            if prepared.limits == limits {
                return Ok(prepared.statistics.clone());
            }
        }
        let prepared = TerminalProductCharts::prepare(&self.terminals, limits)?;
        let statistics = prepared.statistics.clone();
        self.terminal_product_charts = Some(prepared);
        Ok(statistics)
    }

    pub fn terminal_product_chart(
        &self,
        terminal: &SavedApplicationIntegral,
    ) -> Option<&VerifiedVacuumProductChart> {
        self.terminal_product_charts
            .as_ref()?
            .plans
            .get(terminal.family_fingerprint())?
            .charts()
            .get(terminal.integral())
    }

    pub fn terminal_product_chart_statistics(
        &self,
    ) -> Option<&SavedApplicationProductChartStatistics> {
        self.terminal_product_charts
            .as_ref()
            .map(|prepared| &prepared.statistics)
    }
}

impl TerminalProductCharts {
    fn prepare(
        terminals: &TerminalMaps,
        limits: VacuumProductChartLimits,
    ) -> Result<Self, AppError> {
        let started = Instant::now();
        let mut grouped = BTreeMap::<&str, BTreeSet<IntegralKey>>::new();
        let mut requested = 0usize;
        for terminal in &terminals.remaining {
            if !terminal.integral.powers().iter().any(|&power| power < 0) {
                continue;
            }
            requested = requested
                .checked_add(1)
                .ok_or_else(|| AppError::limit("product chart terminal count overflow"))?;
            check_count(
                "product chart terminals",
                requested,
                limits.products.max_terminals,
            )?;
            grouped
                .entry(terminal.family_fingerprint())
                .or_default()
                .insert(terminal.integral.clone());
        }
        let mut prepared = Self {
            families: BTreeMap::new(),
            plans: BTreeMap::new(),
            statistics: SavedApplicationProductChartStatistics {
                work: VacuumProductStatistics::default(),
                lower_families: 0,
                preparation_time: Duration::ZERO,
            },
            limits,
        };
        for (fingerprint, keys) in grouped {
            let family = terminals
                .families
                .get(fingerprint)
                .ok_or_else(|| AppError::input("product chart terminal family is missing"))?;
            let plan = VacuumProductChartPlan::prepare(
                family,
                &keys,
                VacuumProductChartLimits {
                    products: rustred::reduction::terminal_normalization::VacuumProductLimits {
                        max_basis_candidates: limits
                            .products
                            .max_basis_candidates
                            .saturating_sub(prepared.statistics.work.basis_candidates),
                        max_canonical_candidates: limits
                            .products
                            .max_canonical_candidates
                            .saturating_sub(prepared.statistics.work.canonical_candidates),
                        ..limits.products
                    },
                    ..limits
                },
            )
            .map_err(failure)?;
            merge_work(&mut prepared.statistics.work, plan.statistics())?;
            for chart in plan.charts().values() {
                for component in chart.components() {
                    let lower = component.family();
                    if !family
                        .coefficient_context()
                        .has_same_variable_map(lower.coefficient_context())
                    {
                        return Err(AppError::input(
                            "product chart lower-family coefficient context differs",
                        ));
                    }
                    if !prepared.families.contains_key(lower.fingerprint()) {
                        check_count(
                            "product chart lower families",
                            prepared.families.len().saturating_add(1),
                            limits.products.max_lower_families,
                        )?;
                        prepared
                            .families
                            .insert(lower.fingerprint().to_owned(), lower.clone());
                    }
                }
            }
            prepared.plans.insert(fingerprint.to_owned(), plan);
        }
        prepared.statistics.lower_families = prepared.families.len();
        prepared.statistics.preparation_time = started.elapsed();
        Ok(prepared)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::algebra::CoefficientContext;
    use rustred::family::AffineDenominator;

    fn maps(swapped: bool) -> TerminalMaps {
        let c = CoefficientContext::try_new(["d"]).unwrap();
        let mut denominators = vec![
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero(), c.zero()]),
            AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
            AffineDenominator::new(c.integer(-1), vec![c.one(), c.integer(-2), c.one()]),
        ];
        if swapped {
            denominators.swap(0, 1);
        }
        let family = Arc::new(
            IntegralFamily::new(
                "app-charts",
                vec!["q1".into(), "q2".into()],
                vec![],
                c.clone(),
                c.parameter("d").unwrap(),
                denominators,
                vec![],
                vec![c.zero(); 3],
            )
            .unwrap(),
        );
        let remaining: BTreeSet<_> = [[2, 3, 0], [1, 1, -1], [2, 3, -2], [-1, 1, 0]]
            .map(|powers| SavedApplicationIntegral {
                family_fingerprint: family.fingerprint().to_owned(),
                integral: IntegralKey::try_new(powers).unwrap(),
            })
            .into_iter()
            .collect();
        TerminalMaps {
            rows: remaining
                .iter()
                .map(|key| {
                    (
                        key.clone(),
                        TerminalRow {
                            terms: BTreeMap::from([(key.clone(), c.one())]),
                            conditions: Arc::new(vec![]),
                        },
                    )
                })
                .collect(),
            families: BTreeMap::from([(family.fingerprint().to_owned(), family)]),
            remaining,
        }
    }

    #[test]
    fn charts_only_prepare_negative_terminals_and_keep_flat_rows() {
        let maps = maps(false);
        let original = maps.rows.clone();
        let prepared = TerminalProductCharts::prepare(&maps, Default::default()).unwrap();
        assert_eq!(prepared.statistics.work.requested_terminals, 3);
        assert_eq!(prepared.statistics.work.factored_terminals, 2);
        assert_eq!(prepared.statistics.work.retained_terminals, 1);
        assert_eq!(prepared.statistics.work.numerator_terminals, 3);
        assert_eq!(prepared.statistics.lower_families, 1);
        let plan = prepared.plans.values().next().unwrap();
        assert!(plan.charts().keys().all(|key| key.powers()[2] < 0));
        for chart in plan.charts().values() {
            assert_eq!(chart.components().len(), 2);
            assert!(chart.components().iter().all(|component| {
                prepared
                    .families
                    .contains_key(component.family().fingerprint())
            }));
        }
        assert_eq!(maps.rows, original);
    }

    #[test]
    fn chart_search_allowances_are_shared_across_source_families() {
        let mut first = maps(false);
        let second = maps(true);
        assert_ne!(first.families.keys().next(), second.families.keys().next());
        let one = TerminalProductCharts::prepare(&first, Default::default()).unwrap();
        first.families.extend(second.families);
        first.remaining.extend(second.remaining);
        first.rows.extend(second.rows);
        let all = TerminalProductCharts::prepare(&first, Default::default()).unwrap();
        assert!(
            all.statistics.work.canonical_candidates > one.statistics.work.canonical_candidates
        );
        let mut limits = VacuumProductChartLimits::default();
        limits.products.max_canonical_candidates = one.statistics.work.canonical_candidates;
        assert!(TerminalProductCharts::prepare(&first, limits).is_err());
        limits = VacuumProductChartLimits::default();
        limits.products.max_terminals = 5;
        assert!(TerminalProductCharts::prepare(&first, limits).is_err());
    }
}
