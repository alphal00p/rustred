use std::collections::BTreeMap;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::prelude::{PolyVariable, Symbol, UserData};

use crate::algebra::CoefficientContext;

use super::super::error::SymbolicaAffineDenominatorError;
use super::super::limits::SymbolicaAffineDenominatorLimits;
use super::super::{RUSTRED_NAMESPACE, SCALAR_PRODUCT_NAME};
use super::check_limit;

pub(in crate::input::affine) fn validate_declared_labels(
    coefficients: &CoefficientContext,
    loop_momenta: &[String],
    external_momenta: &[String],
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let mut roles = BTreeMap::<String, &'static str>::new();
    let mut total_label_bytes = 0usize;
    for (role, labels) in [
        ("base parameter", coefficients.parameter_names()),
        ("loop momentum", loop_momenta),
        ("external momentum", external_momenta),
    ] {
        for (position, label) in labels.iter().enumerate() {
            if label.is_empty() {
                return Err(SymbolicaAffineDenominatorError::EmptyLabel { role, position });
            }
            check_limit("label bytes", label.len(), limits.max_label_bytes)?;
            total_label_bytes = total_label_bytes.checked_add(label.len()).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "total label bytes",
                },
            )?;
            check_limit(
                "total label bytes",
                total_label_bytes,
                limits.max_total_label_bytes,
            )?;
            if label == "sp" || label == SCALAR_PRODUCT_NAME {
                return Err(SymbolicaAffineDenominatorError::ReservedLabel(
                    label.clone(),
                ));
            }
            if let Some(first_role) = roles.insert(label.clone(), role) {
                return Err(SymbolicaAffineDenominatorError::DuplicateLabel {
                    label: label.clone(),
                    first_role,
                    second_role: role,
                });
            }
        }
    }
    Ok(())
}

pub(in crate::input::affine) fn authenticate_combined_symbols(
    coefficients: &CoefficientContext,
    combined_names: &[String],
) -> Result<BTreeMap<Symbol, usize>, SymbolicaAffineDenominatorError> {
    let mut symbol_positions = BTreeMap::new();
    for (position, (variable, label)) in coefficients
        .variables()
        .iter()
        .zip(combined_names)
        .enumerate()
    {
        let PolyVariable::Symbol(symbol) = variable else {
            return Err(
                SymbolicaAffineDenominatorError::UnsupportedCombinedVariable {
                    position,
                    label: label.clone(),
                },
            );
        };
        let expected_name = format!("{RUSTRED_NAMESPACE}::{label}");
        authenticate_plain_symbol(*symbol, label, &expected_name)?;
        symbol_positions.insert(*symbol, position);
    }
    if symbol_positions.len() != combined_names.len() {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "combined symbol map lost a declared variable",
            },
        );
    }
    Ok(symbol_positions)
}

pub(in crate::input::affine) fn reserved_scalar_product(
    symbol_positions: &BTreeMap<Symbol, usize>,
) -> Result<Symbol, SymbolicaAffineDenominatorError> {
    let scalar_product = plain_symbol(SCALAR_PRODUCT_NAME)?;
    authenticate_plain_symbol(scalar_product, "sp", SCALAR_PRODUCT_NAME)?;
    if symbol_positions.contains_key(&scalar_product) {
        return Err(SymbolicaAffineDenominatorError::ReservedLabel(
            "sp".to_owned(),
        ));
    }
    Ok(scalar_product)
}

fn plain_symbol(name: &str) -> Result<Symbol, SymbolicaAffineDenominatorError> {
    let namespaced = NamespacedSymbol::try_parse(name).ok_or_else(|| {
        SymbolicaAffineDenominatorError::Parse(format!(
            "could not form reserved Symbolica symbol {name:?}"
        ))
    })?;
    SymbolBuilder::new(namespaced)
        .build()
        .map_err(|error| SymbolicaAffineDenominatorError::Parse(error.to_string()))
}

pub(in crate::input::affine) fn maximum_combined_symbol_bytes(
    coefficients: &CoefficientContext,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    coefficients
        .variables()
        .iter()
        .enumerate()
        .try_fold(1usize, |maximum, (position, variable)| {
            let PolyVariable::Symbol(symbol) = variable else {
                return Err(
                    SymbolicaAffineDenominatorError::UnsupportedCombinedVariable {
                        position,
                        label: coefficients.parameter_names()[position].clone(),
                    },
                );
            };
            Ok(maximum.max(symbol.get_name().len()))
        })
}

fn authenticate_plain_symbol(
    symbol: Symbol,
    label: &str,
    expected_name: &str,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let reject = |violation| SymbolicaAffineDenominatorError::ImpureDeclaredSymbol {
        label: label.to_owned(),
        violation,
    };
    if symbol.get_name() != expected_name {
        return Err(reject("canonical name differs from the declaration"));
    }
    if symbol.get_wildcard_level() != 0 {
        return Err(reject("wildcard level is not zero"));
    }
    if symbol.has_attributes() {
        return Err(reject("attributes or tags are present"));
    }
    if !symbol.is_exportable() {
        return Err(reject("a callback or custom function is registered"));
    }
    if !symbol.get_aliases().is_empty() {
        return Err(reject("aliases are registered"));
    }
    if !matches!(symbol.get_data(), UserData::None) {
        return Err(reject("custom user data is registered"));
    }
    Ok(())
}
