//! Exact linear combinations of numeric integrals, shared by the basis change
//! and the certificate.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::algebra::Coefficient;
use crate::solver::{Integral, IntegralOrder};

use super::{DynamicPower, DynamicTerm};

/// A linear combination of integrals, keyed by numeric powers.
pub(super) type Combination = BTreeMap<Vec<i16>, Coefficient>;

/// The numeric powers of an integral.
pub(super) fn values(powers: &[DynamicPower]) -> Vec<i16> {
    powers.iter().map(|power| power.value).collect()
}

pub(super) fn sector_of<const N: usize>(powers: &[i16]) -> [bool; N] {
    std::array::from_fn(|axis| powers[axis] > 0)
}

/// One, over the same variable map as `template`.
pub(super) fn unit(template: &Coefficient) -> Coefficient {
    Coefficient::from(template.numerator.one())
}

pub(super) fn combination(terms: &[DynamicTerm]) -> Combination {
    let mut combination = Combination::new();
    for term in terms {
        add(
            &mut combination,
            values(&term.powers),
            term.coefficient.clone(),
        );
    }
    combination
}

/// Add `coefficient * integral`, dropping a cancelled term.
pub(super) fn add(combination: &mut Combination, integral: Vec<i16>, coefficient: Coefficient) {
    let sum = match combination.remove(&integral) {
        Some(existing) => &existing + &coefficient,
        None => coefficient,
    };
    if !sum.is_zero() {
        combination.insert(integral, sum);
    }
}

/// The search's integral order on numeric keys: `Less` means harder.
pub(super) struct NumericOrder<const N: usize>(IntegralOrder<N>);

impl<const N: usize> NumericOrder<N> {
    pub(super) fn new() -> Self {
        // Numeric keys compare by denominators and sector first, independently
        // of the order's own sector, so one order serves every sector.
        Self(IntegralOrder::new([false; N], [false; N]))
    }

    pub(super) fn integrals(&self, left: &[i16], right: &[i16]) -> Ordering {
        self.0.compare(&integral(left), &integral(right))
    }

    /// Compare sectors through their corner integrals: `Less` means harder.
    pub(super) fn sectors(&self, left: &[bool; N], right: &[bool; N]) -> Ordering {
        let corner = |sector: &[bool; N]| sector.map(i16::from);
        self.integrals(&corner(left), &corner(right))
    }
}

fn integral<const N: usize>(powers: &[i16]) -> Integral<N> {
    let powers: [i16; N] = powers
        .try_into()
        .expect("integral keys have the family's arity");
    Integral::numeric(powers).expect("integral keys lie in the compact power range")
}
