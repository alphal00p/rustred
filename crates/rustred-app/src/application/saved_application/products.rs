//! Optional exact product presentation of the unchanged published terminals.
//! The existing audited core plan owns all momentum-block proofs. This layer
//! only qualifies keys, shares preparation budgets and retains its statistics.
use std::time::{Duration, Instant};

use super::*;
use rustred::reduction::terminal_normalization::{
    VacuumProductLimits, VacuumProductPlan, VacuumProductStatistics,
};

/// Exact equality in the stored unit-mass/common-formal-measure convention.
/// Factors are unevaluated lower-family integrals, not additional claimed
/// masters. Repeated factors remain repeated entries in the sorted product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedApplicationProduct {
    coefficient: Coefficient,
    factors: Vec<SavedApplicationIntegral>,
}
impl SavedApplicationProduct {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
    pub fn factors(&self) -> &[SavedApplicationIntegral] {
        &self.factors
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedApplicationProductStatistics {
    pub work: VacuumProductStatistics,
    pub lower_families: usize,
    /// Time spent in the first successful preparation, excluding loading and
    /// ordinary coefficient reduction. Repeated lookups do no preparation.
    pub preparation_time: Duration,
}

pub(super) struct TerminalProducts {
    pub(super) families: BTreeMap<String, Arc<IntegralFamily>>,
    products: BTreeMap<SavedApplicationIntegral, SavedApplicationProduct>,
    statistics: SavedApplicationProductStatistics,
}

impl<const N: usize> PreparedSavedApplication<N> {
    /// Prepare optional product presentations once. Connected, numerator and
    /// otherwise unsupported keys stay flat and are counted explicitly. This
    /// never changes terminal substitutions, entry scope or reduction caches.
    /// Failed preparation leaves an existing successful plan untouched.
    pub fn prepare_terminal_products(
        &mut self,
        limits: VacuumProductLimits,
    ) -> Result<SavedApplicationProductStatistics, AppError> {
        if let Some(prepared) = &self.terminal_products {
            check_statistics(&prepared.statistics, limits)?;
            return Ok(prepared.statistics.clone());
        }
        let prepared = TerminalProducts::prepare(&self.terminals, limits)?;
        let statistics = prepared.statistics.clone();
        self.terminal_products = Some(prepared);
        Ok(statistics)
    }

    /// Immutable product lookup. `None` means either preparation was not
    /// requested or the original terminal must retain its flat presentation.
    pub fn terminal_product(
        &self,
        terminal: &SavedApplicationIntegral,
    ) -> Option<&SavedApplicationProduct> {
        self.terminal_products.as_ref()?.products.get(terminal)
    }

    pub fn terminal_product_statistics(&self) -> Option<&SavedApplicationProductStatistics> {
        self.terminal_products
            .as_ref()
            .map(|prepared| &prepared.statistics)
    }
}

impl TerminalProducts {
    fn prepare(terminals: &TerminalMaps, limits: VacuumProductLimits) -> Result<Self, AppError> {
        let started = Instant::now();
        check_count(
            "product terminals",
            terminals.remaining.len(),
            limits.max_terminals,
        )?;
        let mut grouped = BTreeMap::<&str, BTreeSet<IntegralKey>>::new();
        for terminal in &terminals.remaining {
            grouped
                .entry(terminal.family_fingerprint())
                .or_default()
                .insert(terminal.integral.clone());
        }
        let mut prepared = Self {
            families: BTreeMap::new(),
            products: BTreeMap::new(),
            statistics: SavedApplicationProductStatistics {
                work: VacuumProductStatistics::default(),
                lower_families: 0,
                preparation_time: Duration::ZERO,
            },
        };
        for (fingerprint, keys) in grouped {
            let family = terminals
                .families
                .get(fingerprint)
                .ok_or_else(|| AppError::input("product terminal family is missing"))?;
            let plan = VacuumProductPlan::prepare(
                family,
                &keys,
                VacuumProductLimits {
                    max_basis_candidates: limits
                        .max_basis_candidates
                        .saturating_sub(prepared.statistics.work.basis_candidates),
                    max_canonical_candidates: limits
                        .max_canonical_candidates
                        .saturating_sub(prepared.statistics.work.canonical_candidates),
                    ..limits
                },
            )
            .map_err(failure)?;
            merge_work(&mut prepared.statistics.work, plan.statistics())?;
            for (fingerprint, lower) in plan.families() {
                if !family
                    .coefficient_context()
                    .has_same_variable_map(lower.coefficient_context())
                {
                    return Err(AppError::input(
                        "product lower-family coefficient context differs",
                    ));
                }
                if !prepared.families.contains_key(fingerprint) {
                    check_count(
                        "product lower families",
                        prepared.families.len().saturating_add(1),
                        limits.max_lower_families,
                    )?;
                    prepared.families.insert(fingerprint.clone(), lower.clone());
                }
            }
            for (key, product) in plan.products() {
                let mut factors = product
                    .factors()
                    .iter()
                    .map(|factor| SavedApplicationIntegral {
                        family_fingerprint: factor.family_fingerprint().to_owned(),
                        integral: factor.integral().clone(),
                    })
                    .collect::<Vec<_>>();
                factors.sort();
                prepared.products.insert(
                    SavedApplicationIntegral {
                        family_fingerprint: fingerprint.to_owned(),
                        integral: key.clone(),
                    },
                    SavedApplicationProduct {
                        coefficient: product.coefficient().clone(),
                        factors,
                    },
                );
            }
        }
        prepared.statistics.lower_families = prepared.families.len();
        prepared.statistics.preparation_time = started.elapsed();
        check_statistics(&prepared.statistics, limits)?;
        Ok(prepared)
    }
}

fn merge_work(
    total: &mut VacuumProductStatistics,
    next: &VacuumProductStatistics,
) -> Result<(), AppError> {
    for (total, next) in [
        (&mut total.requested_terminals, next.requested_terminals),
        (&mut total.factored_terminals, next.factored_terminals),
        (&mut total.retained_terminals, next.retained_terminals),
        (&mut total.numerator_terminals, next.numerator_terminals),
        (&mut total.analyzed_supports, next.analyzed_supports),
        (&mut total.basis_candidates, next.basis_candidates),
        (&mut total.canonical_candidates, next.canonical_candidates),
    ] {
        *total = total
            .checked_add(next)
            .ok_or_else(|| AppError::limit("product statistics overflow"))?;
    }
    Ok(())
}
fn check_count(name: &str, count: usize, limit: usize) -> Result<(), AppError> {
    if count > limit {
        Err(AppError::limit(format!(
            "{name} requires {count}; limit {limit}"
        )))
    } else {
        Ok(())
    }
}
fn check_statistics(
    stats: &SavedApplicationProductStatistics,
    limits: VacuumProductLimits,
) -> Result<(), AppError> {
    for (name, count, limit) in [
        (
            "product terminals",
            stats.work.requested_terminals,
            limits.max_terminals,
        ),
        (
            "product lower families",
            stats.lower_families,
            limits.max_lower_families,
        ),
        (
            "product basis candidates",
            stats.work.basis_candidates,
            limits.max_basis_candidates,
        ),
        (
            "product canonical candidates",
            stats.work.canonical_candidates,
            limits.max_canonical_candidates,
        ),
    ] {
        check_count(name, count, limit)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustred::algebra::CoefficientContext;
    use rustred::family::AffineDenominator;

    #[test]
    fn optional_products_retain_connected_and_numerator_terminals_and_flat_rows() {
        let c = CoefficientContext::try_new(["d"]).unwrap();
        let family = Arc::new(
            IntegralFamily::new(
                "app-products",
                vec!["q1".into(), "q2".into()],
                vec![],
                c.clone(),
                c.parameter("d").unwrap(),
                vec![
                    AffineDenominator::new(c.integer(-1), vec![c.one(), c.zero(), c.zero()]),
                    AffineDenominator::new(c.integer(-1), vec![c.zero(), c.zero(), c.one()]),
                    AffineDenominator::new(c.integer(-1), vec![c.one(), c.integer(-2), c.one()]),
                ],
                vec![],
                vec![c.zero(); 3],
            )
            .unwrap(),
        );
        let key = |powers| SavedApplicationIntegral {
            family_fingerprint: family.fingerprint().to_owned(),
            integral: IntegralKey::try_new(powers).unwrap(),
        };
        let product = key([2, 3, 0]);
        let connected = key([1, 1, 1]);
        let numerator = key([1, 1, -1]);
        let remaining = BTreeSet::from([product.clone(), connected.clone(), numerator.clone()]);
        let rows = remaining
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
            .collect();
        let maps = TerminalMaps {
            families: BTreeMap::from([(family.fingerprint().to_owned(), family)]),
            rows,
            remaining,
        };
        let original = maps.rows.clone();
        let prepared = TerminalProducts::prepare(&maps, Default::default()).unwrap();
        assert_eq!(prepared.statistics.work.requested_terminals, 3);
        assert_eq!(prepared.statistics.work.factored_terminals, 1);
        assert_eq!(prepared.statistics.work.retained_terminals, 2);
        assert_eq!(prepared.statistics.work.numerator_terminals, 1);
        assert_eq!(prepared.statistics.lower_families, 1);
        assert!(!prepared.products.contains_key(&connected));
        assert!(!prepared.products.contains_key(&numerator));
        let factors = prepared.products[&product].factors();
        assert_eq!(factors.len(), 2);
        assert!(factors[0] <= factors[1]);
        assert_eq!(
            factors
                .iter()
                .map(|key| key.integral.powers()[0])
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(
            factors
                .iter()
                .all(|key| prepared.families[key.family_fingerprint()].loop_count() == 1)
        );
        assert_eq!(maps.rows, original);
        assert!(
            TerminalProducts::prepare(
                &maps,
                VacuumProductLimits {
                    max_terminals: 2,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
