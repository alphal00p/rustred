//! Native rational coefficient arithmetic for a strictly recognized subset,
//! then an exact grouped rebuild of the generic polynomial for leaf-separable
//! coefficients ([`super::grouped`]). Other coefficients retain the original
//! Symbolica AtomField conversion.
use super::denominator::Translated;
use super::{Error, native};
use crate::scalar_numerator::service::PreparedScalarProducts;
use symbolica::{
    atom::{Atom, AtomCore},
    domains::atom::AtomField,
    poly::polynomial::MultivariatePolynomial,
    prelude::Q,
};

pub(super) fn collect(
    prepared: &PreparedScalarProducts,
    translated: &Translated,
    mass: &Atom,
) -> Result<MultivariatePolynomial<AtomField, u32>, Error> {
    #[cfg(any(test, feature = "weighted-lowering-diagnostics"))]
    let force_generic = testing::FORCE_GENERIC.with(std::cell::Cell::get);
    #[cfg(not(any(test, feature = "weighted-lowering-diagnostics")))]
    let force_generic = false;
    if translated.real_rational_coefficients && !force_generic {
        #[cfg(any(test, feature = "weighted-lowering-diagnostics"))]
        testing::RATIONAL_CALLS.with(|calls| calls.set(calls.get() + 1));
        let polynomial: MultivariatePolynomial<_, u32> =
            native("rational denominator polynomial conversion", || {
                translated
                    .atom
                    .try_to_polynomial(&Q, prepared.scalar_product_variables.clone())
            })?
            .map_err(|error| Error::NonPolynomialScalarProducts {
                detail: format!(
                    "recognized rational denominator polynomial was not convertible: {error}"
                ),
            })?;
        // Symbolica may auto-promote unknown expressions to new variables.
        // Never let that feature turn an opaque spectator into an integral axis.
        if polynomial.variables().as_ref() != prepared.scalar_product_variables.as_ref() {
            return Err(Error::NonPolynomialScalarProducts {
                detail: "rational denominator conversion changed the admitted coordinate map"
                    .into(),
            });
        }
        return native("rational denominator coefficient conversion", || {
            polynomial.map_coeff(
                |coefficient| Atom::num(coefficient.clone()),
                AtomField::new(),
            )
        });
    }
    if !force_generic
        && let Some(polynomial) = native("grouped denominator polynomial conversion", || {
            super::grouped::collect(prepared, &translated.atom, mass)
        })?
    {
        #[cfg(any(test, feature = "weighted-lowering-diagnostics"))]
        testing::GROUPED_CALLS.with(|calls| calls.set(calls.get() + 1));
        return Ok(polynomial);
    }
    #[cfg(any(test, feature = "weighted-lowering-diagnostics"))]
    testing::GENERIC_CALLS.with(|calls| calls.set(calls.get() + 1));
    native("whole denominator polynomial conversion", || {
        translated
            .atom
            .to_polynomial_in_vars::<u32>(prepared.scalar_product_variables.clone())
    })
}

#[cfg(any(test, feature = "weighted-lowering-diagnostics"))]
pub(in crate::scalar_numerator) mod testing {
    use std::cell::Cell;
    thread_local! {
        pub(super) static FORCE_GENERIC:Cell<bool>=const {Cell::new(false)};
        pub(super) static RATIONAL_CALLS:Cell<usize>=const {Cell::new(0)};
        pub(super) static GENERIC_CALLS:Cell<usize>=const {Cell::new(0)};
        pub(super) static GROUPED_CALLS:Cell<usize>=const {Cell::new(0)};
        static ACTIVE:Cell<bool>=const {Cell::new(false)};
    }
    /// Calls of the (rational, generic, grouped) lanes in the active scope.
    pub fn current() -> Option<(usize, usize, usize)> {
        ACTIVE.with(Cell::get).then(|| {
            (
                RATIONAL_CALLS.with(Cell::get),
                GENERIC_CALLS.with(Cell::get),
                GROUPED_CALLS.with(Cell::get),
            )
        })
    }
    /// Run with the given policy (`force_generic` disables both exact lanes)
    /// and return the (rational, generic, grouped) lane calls.
    pub fn observed<T>(force_generic: bool, run: impl FnOnce() -> T) -> (T, usize, usize, usize) {
        struct Restore(bool, usize, usize, usize);
        impl Drop for Restore {
            fn drop(&mut self) {
                FORCE_GENERIC.with(|flag| flag.set(self.0));
                RATIONAL_CALLS.with(|calls| calls.set(self.1));
                GENERIC_CALLS.with(|calls| calls.set(self.2));
                GROUPED_CALLS.with(|calls| calls.set(self.3));
                ACTIVE.with(|active| active.set(false));
            }
        }
        ACTIVE.with(|active| {
            assert!(
                !active.replace(true),
                "nested lowering diagnostic scopes are unsupported"
            )
        });
        let old = Restore(
            FORCE_GENERIC.with(|flag| flag.replace(force_generic)),
            RATIONAL_CALLS.with(|calls| calls.replace(0)),
            GENERIC_CALLS.with(|calls| calls.replace(0)),
            GROUPED_CALLS.with(|calls| calls.replace(0)),
        );
        let output = run();
        let counts = (
            RATIONAL_CALLS.with(Cell::get),
            GENERIC_CALLS.with(Cell::get),
            GROUPED_CALLS.with(Cell::get),
        );
        drop(old);
        (output, counts.0, counts.1, counts.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use symbolica::{
        atom::{NamespacedSymbol, SymbolBuilder},
        poly::PolyVariable,
    };

    #[test]
    fn rational_polynomial_conversion_refuses_an_auto_added_coordinate() {
        let symbol = |name: &str| {
            SymbolBuilder::new(
                NamespacedSymbol::try_parse(format!("rational_weighted_map_guard::{name}"))
                    .unwrap(),
            )
            .build()
            .unwrap()
        };
        let x = symbol("x");
        let y = symbol("y");
        let prepared = PreparedScalarProducts {
            dot_head: symbol("dot"),
            loop_momenta: Vec::new(),
            scalar_products: vec![Atom::var(x)],
            scalar_product_variables: Arc::new(vec![PolyVariable::Symbol(x)]),
            limits: Default::default(),
        };
        // Deliberately corrupt the recognizer's bit to exercise the independent
        // post-conversion map guard. Normal syntax recognition declines y.
        let translated = Translated {
            atom: Atom::var(x) + Atom::var(y),
            real_rational_coefficients: true,
        };
        let error = collect(&prepared, &translated, &Atom::one()).unwrap_err();
        assert!(matches!(error, Error::NonPolynomialScalarProducts { .. }));
        assert!(error.to_string().contains("coordinate map"));
    }
}
