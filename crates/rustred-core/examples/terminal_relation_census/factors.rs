//! Diagnostic common-parameter factors of exact, support-restricted U.
//! This proposes sectors for momentum/block replay; it supplies no equations.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::Write;
use std::sync::Arc;

use rustred::family::symanzik::SymanzikPolynomials;
use rustred::family::{IntegralFamily, IntegralKey};
use symbolica::domains::rational_polynomial::RationalPolynomialField;
use symbolica::poly::factor::Factorize;
use symbolica::prelude::{IntegerRing, MultivariatePolynomial, PolyVariable, Q, Rational, Z};

type Polynomial = MultivariatePolynomial<RationalPolynomialField<IntegerRing, u16>, u16>;

fn merges_components(positive: &[Vec<usize>], nonzero: &[Vec<usize>]) -> bool {
    nonzero.iter().any(|block| {
        positive
            .iter()
            .filter(|component| component.iter().any(|slot| block.contains(slot)))
            .take(2)
            .count()
            > 1
    })
}

fn components(
    full: &Polynomial,
    support: &[usize],
    family: &IntegralFamily,
    cache: &mut BTreeMap<Vec<usize>, Option<Vec<Vec<usize>>>>,
) -> Option<Vec<Vec<usize>>> {
    cache
        .entry(support.to_vec())
        .or_insert_with(|| {
            let mut restricted = full.clone();
            for slot in 0..family.denominator_count() {
                if support.binary_search(&slot).is_err() {
                    restricted = restricted.replace(slot, &family.coefficient_context().zero());
                }
            }
            if restricted.is_zero()
                || restricted.coefficients.iter().any(|c| {
                    !c.numerator.is_constant()
                        || !c.denominator.is_constant()
                        || c.denominator.is_zero()
                })
            {
                return None;
            }
            // Only this diagnostic specializes the coefficient representation,
            // and only after proving every coefficient is an exact rational.
            let rational = restricted.map_coeff(
                |c| Rational::from((c.numerator.get_constant(), c.denominator.get_constant())),
                Q,
            );
            let mut blocks = Vec::new();
            for (factor, _) in rational.factor() {
                if factor.is_constant() {
                    continue;
                }
                blocks.push(
                    support
                        .iter()
                        .copied()
                        .filter(|&slot| factor.exponents_iter().any(|powers| powers[slot] != 0))
                        .collect::<Vec<_>>(),
                );
            }
            blocks.sort();
            Some(blocks)
        })
        .clone()
}

fn factors(
    full: &Polynomial,
    support: &[usize],
    family: &IntegralFamily,
    cache: &mut BTreeMap<Vec<usize>, Vec<usize>>,
) -> Vec<usize> {
    cache
        .entry(support.to_vec())
        .or_insert_with(|| {
            let mut restricted = full.clone();
            // This is the same native x_inactive=0 operation used by the library's
            // verified parametric geometry lane; no elimination is implemented here.
            for slot in 0..family.denominator_count() {
                if support.binary_search(&slot).is_err() {
                    restricted = restricted.replace(slot, &family.coefficient_context().zero());
                }
            }
            if restricted.is_zero() {
                return Vec::new();
            }
            support
                .iter()
                .copied()
                .filter(|&slot| restricted.exponents_iter().all(|powers| powers[slot] == 1))
                .collect()
        })
        .clone()
}

pub(super) fn write(
    out: &mut impl Write,
    family: &IntegralFamily,
    keys: &BTreeSet<IntegralKey>,
) -> Result<(), Box<dyn Error>> {
    let polynomials = SymanzikPolynomials::try_from_family_with_limits(family, Default::default())?;
    let variables = Arc::new(
        (0..family.denominator_count())
            .map(PolyVariable::Temporary)
            .collect(),
    );
    let mut full = Polynomial::new(&RationalPolynomialField::new(Z), None, variables);
    for (coefficient, powers) in polynomials.u().terms() {
        full.append_monomial(coefficient.clone(), powers);
    }
    let mut cache = BTreeMap::new();
    let mut component_cache = BTreeMap::new();
    let mut row_count = 0usize;
    let mut scalar_count = 0usize;
    let mut dotted_scalar_count = 0usize;
    let mut uncoupled_dotted_count = 0usize;
    let mut existing_lowered_count = 0usize;
    write!(
        out,
        "{{\"verified_momentum_factorization\":false,\"full_u_terms\":{},\"rows\":[",
        full.nterms()
    )?;
    for key in keys {
        let positive: Vec<_> = key
            .powers()
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| (p > 0).then_some(i))
            .collect();
        let positive_factors = factors(&full, &positive, family, &mut cache);
        if positive_factors.is_empty() {
            continue;
        }
        let nonzero: Vec<_> = key
            .powers()
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| (p != 0).then_some(i))
            .collect();
        let nonzero_factors = factors(&full, &nonzero, family, &mut cache);
        let scalar = key.powers().iter().all(|&p| p >= 0);
        let dotted: Vec<_> = positive_factors
            .iter()
            .copied()
            .filter(|&i| key.powers()[i] > 1)
            .collect();
        let uncoupled_dotted: Vec<_> = dotted
            .iter()
            .copied()
            .filter(|i| nonzero_factors.contains(i))
            .collect();
        let mut existing_lowered = Vec::new();
        for &slot in &uncoupled_dotted {
            let mut powers = key.powers().to_vec();
            powers[slot] -= 1;
            if keys.contains(&IntegralKey::try_new(powers.clone())?) {
                existing_lowered.push(powers);
            }
        }
        if row_count != 0 {
            write!(out, ",")?;
        }
        write!(
            out,
            "{{\"key\":{:?},\"scalar\":{scalar},\"positive_support_factors\":{positive_factors:?},\"nonzero_support_factors\":{nonzero_factors:?},\"dotted_factor_slots\":{dotted:?},\"uncoupled_dotted_slots\":{uncoupled_dotted:?},\"existing_lowered_keys\":{existing_lowered:?}}}",
            key.powers()
        )?;
        row_count += 1;
        scalar_count += usize::from(scalar);
        dotted_scalar_count += usize::from(scalar && !dotted.is_empty());
        uncoupled_dotted_count += usize::from(!uncoupled_dotted.is_empty());
        existing_lowered_count += existing_lowered.len();
    }
    write!(
        out,
        "],\"keys_with_positive_support_factor\":{row_count},\"scalar_keys_with_factor\":{scalar_count},\"dotted_scalar_factor_keys\":{dotted_scalar_count},\"uncoupled_dotted_factor_keys\":{uncoupled_dotted_count},\"existing_lowered_key_pairs\":{existing_lowered_count},\"distinct_restricted_supports\":{},\"component_candidates\":[",
        cache.len()
    )?;
    let mut component_rows = 0usize;
    let mut coupled_rows = 0usize;
    let mut coupled_rank_one = 0usize;
    for key in keys {
        let support = |nonzero: bool| {
            key.powers()
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| (p > 0 || nonzero && p < 0).then_some(i))
                .collect::<Vec<_>>()
        };
        let Some(positive) = components(&full, &support(false), family, &mut component_cache)
        else {
            continue;
        };
        if positive.len() < 2 {
            continue;
        }
        let nonzero = components(&full, &support(true), family, &mut component_cache);
        let coupled = nonzero
            .as_ref()
            .is_some_and(|blocks| merges_components(&positive, blocks));
        let rank = key
            .powers()
            .iter()
            .filter(|&&p| p < 0)
            .map(|&p| i128::from(p).abs())
            .sum::<i128>();
        if component_rows != 0 {
            write!(out, ",")?;
        }
        write!(
            out,
            "{{\"key\":{:?},\"numerator_rank\":{rank},\"positive_u_components\":{positive:?},\"numerator_merges_components\":{coupled}}}",
            key.powers()
        )?;
        component_rows += 1;
        coupled_rows += usize::from(coupled);
        coupled_rank_one += usize::from(coupled && rank == 1);
    }
    write!(
        out,
        "],\"keys_with_disconnected_u\":{component_rows},\"keys_with_numerator_component_coupling\":{coupled_rows},\"rank_one_keys_with_numerator_component_coupling\":{coupled_rank_one},\"component_restricted_supports\":{}}}",
        component_cache.len()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::merges_components;

    #[test]
    fn adding_a_numerator_within_one_factor_does_not_couple_components() {
        let positive = vec![vec![0], vec![1, 2], vec![3, 4]];
        assert!(!merges_components(
            &positive,
            &[vec![0], vec![1, 2, 5], vec![3, 4]]
        ));
        assert!(merges_components(
            &positive,
            &[vec![0], vec![1, 2, 3, 4, 5]]
        ));
        assert!(merges_components(
            &positive,
            &[vec![0, 1, 2, 5], vec![3, 4]]
        ));
    }
}
