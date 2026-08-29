//! Symbolica ring and field trait implementation over checked coefficients.

use std::fmt;

use rand::RngCore;
use symbolica::domains::SelfRing;
use symbolica::prelude::*;

use crate::algebra::Coefficient;

use super::state::CheckedCoefficientField;

impl Set for CheckedCoefficientField<'_> {
    type Element = Coefficient;

    fn size(&self) -> Option<Integer> {
        None
    }
}

impl RingOps<Coefficient> for CheckedCoefficientField<'_> {
    fn add(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.add_checked(&left, &right)
    }

    fn sub(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.sub_checked(&left, &right)
    }

    fn mul(&self, left: Coefficient, right: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.mul_checked(&left, &right)
    }

    fn neg(&self, value: Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.neg_checked(&value)
    }

    fn add_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.add_checked(left, &right);
    }

    fn sub_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.sub_checked(left, &right);
    }

    fn mul_assign(&self, left: &mut Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *left = self.mul_checked(left, &right);
    }

    fn add_mul_assign(&self, accumulator: &mut Coefficient, left: Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let product = self.mul_checked(&left, &right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(&self, accumulator: &mut Coefficient, left: Coefficient, right: Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let product = self.mul_checked(&left, &right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl RingOps<&Coefficient> for CheckedCoefficientField<'_> {
    fn add(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.add_checked(left, right)
    }

    fn sub(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.sub_checked(left, right)
    }

    fn mul(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.mul_checked(left, right)
    }

    fn neg(&self, value: &Coefficient) -> Coefficient {
        self.neg_checked(value)
    }

    fn add_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.add_checked(left, right);
    }

    fn sub_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.sub_checked(left, right);
    }

    fn mul_assign(&self, left: &mut Coefficient, right: &Coefficient) {
        *left = self.mul_checked(left, right);
    }

    fn add_mul_assign(
        &self,
        accumulator: &mut Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.add_checked(accumulator, &product);
    }

    fn sub_mul_assign(
        &self,
        accumulator: &mut Coefficient,
        left: &Coefficient,
        right: &Coefficient,
    ) {
        let product = self.mul_checked(left, right);
        *accumulator = self.sub_checked(accumulator, &product);
    }
}

impl Ring for CheckedCoefficientField<'_> {
    fn zero(&self) -> Coefficient {
        self.charge_counter(|stats| &mut stats.zero_constants);
        self.context.zero()
    }

    fn one(&self) -> Coefficient {
        self.charge_counter(|stats| &mut stats.one_constants);
        self.context.one()
    }

    fn nth(&self, value: Integer) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.contextual_integer(value)
    }

    fn pow(&self, base: &Coefficient, exponent: u64) -> Coefficient {
        self.charge_counter(|stats| &mut stats.power_calls);
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        let admission = self.preflight_power_admission(base, exponent);
        self.charge_power_operations(exponent, admission);
        self.finish_power_raw(self.inner.pow(base, exponent), admission)
    }

    fn is_zero(&self, value: &Coefficient) -> bool {
        self.charge_counter(|stats| &mut stats.zero_tests);
        value.is_zero()
    }

    fn is_one(&self, value: &Coefficient) -> bool {
        self.charge_counter(|stats| &mut stats.one_tests);
        value.is_one()
    }

    fn one_is_gcd_unit() -> bool {
        <RationalPolynomialField<IntegerRing, u16> as Ring>::one_is_gcd_unit()
    }

    fn characteristic(&self) -> Integer {
        self.inner.characteristic()
    }

    fn try_inv(&self, value: &Coefficient) -> Option<Coefficient> {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        if value.is_zero() {
            None
        } else {
            Some(self.div_checked(&self.context.one(), value))
        }
    }

    fn try_div(&self, numerator: &Coefficient, denominator: &Coefficient) -> Option<Coefficient> {
        if denominator.is_zero() {
            None
        } else {
            Some(self.div_checked(numerator, denominator))
        }
    }

    fn sample(&self, rng: &mut impl RngCore, range: (i64, i64)) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.contextual_integer(Z.sample(rng, range))
    }

    fn format<W: fmt::Write>(
        &self,
        element: &Coefficient,
        options: &PrintOptions,
        state: PrintState,
        formatter: &mut W,
    ) -> Result<bool, fmt::Error> {
        self.inner.format(element, options, state, formatter)
    }

    fn has_independent_elements(&self) -> bool {
        true
    }
}

impl EuclideanDomain for CheckedCoefficientField<'_> {
    fn rem(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.finish_raw(self.inner.rem(left, right))
    }

    fn quot_rem(
        &self,
        numerator: &Coefficient,
        denominator: &Coefficient,
    ) -> (Coefficient, Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        (
            self.div_checked(numerator, denominator),
            self.context.zero(),
        )
    }

    fn gcd(&self, left: &Coefficient, right: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.finish_raw(self.inner.gcd(left, right))
    }
}

impl Field for CheckedCoefficientField<'_> {
    fn div(&self, numerator: &Coefficient, denominator: &Coefficient) -> Coefficient {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        self.div_checked(numerator, denominator)
    }

    fn div_assign(&self, numerator: &mut Coefficient, denominator: &Coefficient) {
        self.charge_counter(|stats| &mut stats.non_matrix_trait_calls);
        *numerator = self.div_checked(numerator, denominator);
    }

    fn inv(&self, value: &Coefficient) -> Coefficient {
        self.div_checked(&self.context.one(), value)
    }
}
