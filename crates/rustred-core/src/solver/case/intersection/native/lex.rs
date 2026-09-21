//! Admission for the cold native elimination-order fallback.
//!
//! These topology-independent input caps limit exposure to expensive Lex F4;
//! they are not a hard bound on time or memory inside a native invocation.

use crate::algebra::CoefficientPolynomial;

pub(super) fn eligible(equations: &[CoefficientPolynomial]) -> bool {
    if !(2..=8).contains(&equations.len()) {
        return false;
    }
    let mut terms = 0_usize;
    let mut variables = Vec::with_capacity(3);
    for equation in equations {
        let Some(next_terms) = terms.checked_add(equation.nterms()) else {
            return false;
        };
        if next_terms > 128
            || equation
                .coefficients
                .iter()
                .any(|value| value.significant_bits() > 128)
        {
            return false;
        }
        terms = next_terms;
        for powers in equation.exponents_iter() {
            let mut degree = 0_u32;
            for (axis, &power) in powers.iter().enumerate() {
                if power == 0 {
                    continue;
                }
                // Each exponent is u16; checked addition also documents the
                // admission boundary independently of the coefficient map.
                let Some(next_degree) = degree.checked_add(u32::from(power)) else {
                    return false;
                };
                if next_degree > 4 {
                    return false;
                }
                degree = next_degree;
                if !variables.contains(&axis) {
                    if variables.len() == 3 {
                        return false;
                    }
                    variables.push(axis);
                }
            }
        }
    }
    true
}

#[cfg(test)]
std::thread_local! {
    static CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn record_call() {
    CALLS.with(|calls| calls.set(calls.get() + 1));
}

#[cfg(test)]
pub(super) fn take_calls() -> usize {
    CALLS.with(|calls| calls.replace(0))
}
