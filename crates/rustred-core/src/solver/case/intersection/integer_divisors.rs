//! Exhaustive signed-divisor orchestration over native exact integer factors.
//! No factoring/primality kernel lives here. The u64 boundary is where native
//! primality is deterministic; larger constants receive no completeness claim.

use std::time::Instant;

use symbolica::prelude::Integer;

use super::{
    CaseIntersectionBudget, CaseIntersectionFailure, CaseIntersectionLimits, CaseIntersectionStats,
    engine::spend,
};

/// Return false for zero or unsupported factor size, never for budget failure.
/// The callback receives every signed divisor exactly once, under the shared
/// work budget. An error invalidates the entire caller's OR result.
pub(super) fn visit(
    value: &Integer,
    limits: CaseIntersectionLimits,
    stats: &mut CaseIntersectionStats,
    pending: usize,
    mut callback: impl FnMut(&Integer) -> Result<(), CaseIntersectionFailure>,
) -> Result<bool, CaseIntersectionFailure> {
    let magnitude = value.abs();
    if magnitude.is_zero() || u64::try_from(magnitude.clone()).is_err() {
        return Ok(false);
    }
    let factors = if magnitude.is_one() {
        Vec::new()
    } else {
        spend(
            &mut stats.factorizations,
            limits.max_factorizations,
            CaseIntersectionBudget::Factorizations,
        )?;
        stats.integer_factorizations += 1;
        let start = Instant::now();
        let factors = magnitude.factor();
        stats.factorization_time += start.elapsed();
        factors
    };
    let budget_error = || CaseIntersectionFailure::Budget {
        kind: CaseIntersectionBudget::WorkItems,
        limit: limits.max_work_items,
    };
    let mut divisor_count = 1usize;
    let mut product = Integer::one();
    let mut exponents = Vec::with_capacity(factors.len());
    for (prime, exponent) in &factors {
        // Product equality alone cannot certify an opaque composite factor.
        if u64::try_from(prime.clone()).is_err() || !prime.is_prime(0) {
            return Err(CaseIntersectionFailure::NativeAlgebra);
        }
        let exponent = u32::try_from(exponent.clone())
            .ok()
            .filter(|&e| e > 0 && e <= 64)
            .ok_or(CaseIntersectionFailure::NativeAlgebra)?;
        product *= prime.pow(exponent as u64);
        divisor_count = divisor_count
            .checked_mul(exponent as usize + 1)
            .ok_or_else(budget_error)?;
        exponents.push(exponent);
    }
    if product != magnitude || factors.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(CaseIntersectionFailure::NativeAlgebra);
    }
    let visits = divisor_count.checked_mul(2).ok_or_else(budget_error)?;
    if stats
        .work_items
        .checked_add(pending)
        .and_then(|n| n.checked_add(visits))
        .is_none_or(|n| n > limits.max_work_items)
    {
        return Err(budget_error());
    }
    let mut divisors = vec![Integer::one()];
    for ((prime, _), exponent) in factors.iter().zip(exponents) {
        let previous = divisors.len();
        let mut power = Integer::one();
        for _ in 0..exponent {
            power *= prime;
            for position in 0..previous {
                divisors.push(&divisors[position] * &power);
            }
        }
    }
    for positive in divisors {
        for divisor in [positive.clone(), -positive] {
            spend(
                &mut stats.work_items,
                limits.max_work_items,
                CaseIntersectionBudget::WorkItems,
            )?;
            stats.integer_divisors += 1;
            callback(&divisor)?;
        }
    }
    Ok(true)
}
