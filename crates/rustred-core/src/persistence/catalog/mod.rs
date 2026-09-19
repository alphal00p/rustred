//! Native values attached to typed integral keys, independently of IBP authority.
//!
//! A declaration-complete catalog covers only its declared finite key set. It
//! neither discovers identities nor proves closure, minimality, or the supplied
//! values. Native loading requires trusted generated Symbolica data.

mod codec;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use symbolica::atom::{Atom, AtomCore, AtomView};
use symbolica::coefficient::CoefficientView;
use symbolica::prelude::PolyVariable;

use crate::family::IntegralKey;

use super::{BinaryIoError, BinaryIoLimits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCatalogCoverage {
    Partial,
    Complete,
}

/// Exact expressions supplied externally for a family-specific finite key set.
///
/// Values may contain functions and symbols; they are not restricted to rational
/// polynomials, one dimension variable, vacuum graphs or a fixed loop count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactTerminalCatalog {
    family_fingerprint: String,
    index_count: usize,
    coverage: TerminalCatalogCoverage,
    terms: BTreeMap<IntegralKey, Atom>,
}

impl ExactTerminalCatalog {
    pub fn try_new(
        family_fingerprint: impl Into<String>,
        index_count: usize,
        coverage: TerminalCatalogCoverage,
        terms: BTreeMap<IntegralKey, Atom>,
    ) -> Result<Self, BinaryIoError> {
        let family_fingerprint = family_fingerprint.into();
        validate_identity(&family_fingerprint, index_count)?;
        for (key, value) in &terms {
            if key.powers().len() != index_count {
                return Err(BinaryIoError::Invalid(
                    "terminal key arity differs from catalog",
                ));
            }
            validate_exact_value(value)?;
        }
        Ok(Self {
            family_fingerprint,
            index_count,
            coverage,
            terms,
        })
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn index_count(&self) -> usize {
        self.index_count
    }
    pub fn coverage(&self) -> TerminalCatalogCoverage {
        self.coverage
    }
    pub fn terms(&self) -> &BTreeMap<IntegralKey, Atom> {
        &self.terms
    }
    pub fn into_terms(self) -> BTreeMap<IntegralKey, Atom> {
        self.terms
    }

    /// Encode exact native expressions, sharing one state and value dictionary.
    /// IDs follow sorted integral-key order, not hash iteration order. Native
    /// bytes can depend on ambient Symbolica state; mathematical values do not.
    pub fn encode_native(&self, limits: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        codec::encode(self, limits)
    }

    /// Load a trusted generated catalog, with an independent expected family.
    ///
    /// Outer framing, counts and IDs are checked before native import. Symbolica
    /// owns expression/state decoding and may extend process-global state. Its
    /// native readers are not hardened against hostile inner allocation counts;
    /// these byte limits must not be advertised as a hostile-input memory bound.
    /// No source replay, rule admission or family-closure authority is granted.
    pub fn decode_generated(
        bytes: &[u8],
        expected_family_fingerprint: &str,
        expected_index_count: usize,
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        codec::decode(
            bytes,
            expected_family_fingerprint,
            expected_index_count,
            limits,
        )
    }
}

fn validate_identity(fingerprint: &str, index_count: usize) -> Result<(), BinaryIoError> {
    if fingerprint.is_empty() || index_count == 0 {
        return Err(BinaryIoError::Invalid(
            "terminal catalog requires a family and positive arity",
        ));
    }
    Ok(())
}

fn validate_exact_value(value: &Atom) -> Result<(), BinaryIoError> {
    let mut error = None;
    value.visitor(&mut |part| {
        if error.is_some() {
            return false;
        }
        if let AtomView::Num(number) = part {
            match number.get_coeff_view() {
                CoefficientView::Float(_, _) => {
                    error = Some(BinaryIoError::Invalid(
                        "terminal catalog value contains an approximate coefficient",
                    ));
                }
                CoefficientView::Indeterminate | CoefficientView::Infinity(_) => {
                    error = Some(BinaryIoError::Invalid(
                        "terminal catalog value contains a nonfinite coefficient",
                    ));
                }
                CoefficientView::RationalPolynomial(packed) => {
                    // Ordinary expression walking does not enter packed RP
                    // variable maps. Symbolica supplies the exact native map;
                    // its function/power atoms can themselves contain floats.
                    let polynomial = packed.deserialize();
                    for variable in polynomial.get_variables().iter() {
                        if let PolyVariable::Function(_, atom) | PolyVariable::Power(atom) =
                            variable
                        {
                            if let Err(nested) = validate_exact_value(atom) {
                                error = Some(nested);
                                break;
                            }
                        }
                    }
                }
                // Rational complex and finite-field values are exact. This
                // generic transport does not assume a characteristic-zero
                // physical master interpretation; that belongs to consumers.
                _ => {}
            }
        }
        error.is_none()
    });
    error.map_or(Ok(()), Err)
}
