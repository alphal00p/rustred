use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::prelude::{AtomView, PolyVariable, Symbol, UserData};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::ScalarProductCoordinate;

use super::budget::{coefficient_census, planned_coefficient_clone_census};
use super::error::SymbolicaAffineDenominatorError;
use super::limits::SymbolicaAffineDenominatorLimits;
use super::model::SymbolicaAffineDenominatorCompiler;
use super::{RUSTRED_NAMESPACE, SCALAR_PRODUCT_NAME};

impl SymbolicaAffineDenominatorCompiler {
    /// Authenticate one already-normalized ordered declaration.
    ///
    /// The base parameter list may have been explicit or inferred by a caller;
    /// this layer deliberately does not distinguish those provenance paths.
    pub fn try_new(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        catch_unwind(AssertUnwindSafe(|| {
            Self::try_new_inner(
                coefficients,
                loop_momenta,
                external_momenta,
                external_gram,
                limits,
            )
        }))
        .map_err(|_| SymbolicaAffineDenominatorError::SymbolicaPanic {
            stage: "compiler construction",
        })?
    }

    fn try_new_inner(
        coefficients: CoefficientContext,
        loop_momenta: Vec<String>,
        external_momenta: Vec<String>,
        external_gram: Vec<Vec<Coefficient>>,
        limits: SymbolicaAffineDenominatorLimits,
    ) -> Result<Self, SymbolicaAffineDenominatorError> {
        check_limit(
            "base parameters",
            coefficients.parameter_names().len(),
            limits.max_base_parameters,
        )?;
        // Authenticate the already-retained template without constructing an
        // additional zero coefficient before its storage policy is known.
        coefficients.validate_with_limits(coefficients.template(), limits.exact_algebra)?;
        if loop_momenta.is_empty() {
            return Err(SymbolicaAffineDenominatorError::NoLoopMomenta);
        }

        let momentum_count = loop_momenta
            .len()
            .checked_add(external_momenta.len())
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "declared momenta",
            })?;
        check_limit("declared momenta", momentum_count, limits.max_momenta)?;
        let coordinate_count =
            scalar_product_coordinate_count(loop_momenta.len(), external_momenta.len())?;
        check_limit(
            "scalar-product coordinates",
            coordinate_count,
            limits.max_scalar_product_coordinates,
        )?;

        let mut roles = BTreeMap::<String, &'static str>::new();
        let mut total_label_bytes = 0usize;
        for (role, labels) in [
            ("base parameter", coefficients.parameter_names()),
            ("loop momentum", loop_momenta.as_slice()),
            ("external momentum", external_momenta.as_slice()),
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

        validate_external_gram(&coefficients, &external_momenta, &external_gram, limits)?;

        let combined_count = coefficients
            .parameter_names()
            .len()
            .checked_add(momentum_count)
            .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "combined Symbolica variables",
            })?;
        check_limit(
            "combined Symbolica variables",
            combined_count,
            limits.max_combined_variables,
        )?;
        check_limit(
            "combined variable-map exponent width",
            combined_count,
            limits.max_combined_exponent_entries,
        )?;
        let mut combined_names = Vec::new();
        combined_names
            .try_reserve_exact(combined_count)
            .map_err(|_| SymbolicaAffineDenominatorError::AllocationFailure {
                resource: "combined Symbolica variable names",
                requested: combined_count,
            })?;
        combined_names.extend(coefficients.parameter_names().iter().cloned());
        combined_names.extend(loop_momenta.iter().cloned());
        combined_names.extend(external_momenta.iter().cloned());
        let combined = CoefficientContext::try_new(combined_names.clone())?;
        let combined_template_census = planned_coefficient_clone_census(
            combined.template(),
            combined.parameter_names().len(),
        )?;
        check_limit(
            "combined template retained bytes",
            combined_template_census.retained_bytes,
            limits.max_combined_retained_bytes,
        )?;

        for (position, label) in coefficients.parameter_names().iter().enumerate() {
            if combined.variables()[position] != coefficients.variables()[position] {
                return Err(
                    SymbolicaAffineDenominatorError::CombinedVariableMapMismatch {
                        position,
                        label: label.clone(),
                    },
                );
            }
        }

        let mut symbol_positions = BTreeMap::new();
        for (position, (variable, label)) in
            combined.variables().iter().zip(&combined_names).enumerate()
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
        if symbol_positions.len() != combined_count {
            return Err(
                SymbolicaAffineDenominatorError::InternalVerificationFailure {
                    detail: "combined symbol map lost a declared variable",
                },
            );
        }
        let scalar_product = plain_symbol(SCALAR_PRODUCT_NAME)?;
        authenticate_plain_symbol(scalar_product, "sp", SCALAR_PRODUCT_NAME)?;
        if symbol_positions.contains_key(&scalar_product) {
            return Err(SymbolicaAffineDenominatorError::ReservedLabel(
                "sp".to_owned(),
            ));
        }
        let coordinates = scalar_product_coordinates(
            loop_momenta.len(),
            external_momenta.len(),
            coordinate_count,
        )?;
        Ok(Self {
            coefficients,
            loop_momenta,
            external_momenta,
            external_gram,
            combined,
            symbol_positions,
            scalar_product,
            coordinates,
            limits,
        })
    }
}

fn validate_external_gram(
    coefficients: &CoefficientContext,
    external_momenta: &[String],
    gram: &[Vec<Coefficient>],
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let expected = external_momenta.len();
    let expected_entries = expected.checked_mul(expected).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "external Gram entries",
        },
    )?;
    check_limit(
        "external Gram entries",
        expected_entries,
        limits.max_external_gram_entries,
    )?;
    if gram.len() != expected {
        return Err(SymbolicaAffineDenominatorError::WrongExternalGramRowCount {
            expected,
            actual: gram.len(),
        });
    }
    for (row, entries) in gram.iter().enumerate() {
        if entries.len() != expected {
            return Err(
                SymbolicaAffineDenominatorError::WrongExternalGramColumnCount {
                    row,
                    expected,
                    actual: entries.len(),
                },
            );
        }
    }
    let mut polynomial_terms = 0usize;
    let mut exponent_entries = 0usize;
    let mut integer_bits = 0usize;
    for (row, entries) in gram.iter().enumerate() {
        for (column, coefficient) in entries.iter().enumerate() {
            coefficients
                .validate_with_limits(coefficient, limits.exact_algebra)
                .map_err(|error| {
                    SymbolicaAffineDenominatorError::InvalidExternalGramCoefficient {
                        row,
                        column,
                        error,
                    }
                })?;
            let coefficient_terms = coefficient
                .numerator
                .nterms()
                .checked_add(coefficient.denominator.nterms())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                })?;
            polynomial_terms = polynomial_terms.checked_add(coefficient_terms).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram polynomial terms",
                },
            )?;
            check_limit(
                "external Gram polynomial terms",
                polynomial_terms,
                limits.max_external_gram_polynomial_terms,
            )?;
            let coefficient_exponents = coefficient_terms
                .checked_mul(coefficients.parameter_names().len())
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                })?;
            exponent_entries = exponent_entries.checked_add(coefficient_exponents).ok_or(
                SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram exponent entries",
                },
            )?;
            check_limit(
                "external Gram exponent entries",
                exponent_entries,
                limits.max_external_gram_exponent_entries,
            )?;
            integer_bits = integer_bits
                .checked_add(coefficient_census(coefficient)?.integer_bits)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "external Gram integer bits",
                })?;
            check_limit(
                "external Gram integer bits",
                integer_bits,
                limits.max_external_gram_integer_bits,
            )?;
            if gram[column][row] != *coefficient {
                return Err(SymbolicaAffineDenominatorError::AsymmetricExternalGram {
                    row,
                    column,
                });
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn schedule_atom_views_with_depth<'a>(
    pending: &mut Vec<(AtomView<'a>, usize)>,
    children: impl Iterator<Item = AtomView<'a>>,
    child_count: usize,
    depth: usize,
    inspected: usize,
    node_limit: usize,
    allocation_resource: &'static str,
) -> Result<(), SymbolicaAffineDenominatorError> {
    let scheduled = inspected
        .checked_add(pending.len())
        .and_then(|value| value.checked_add(child_count))
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "input Atom nodes",
        })?;
    // `scheduled` is a census of every inspected or pending Atom, so this is
    // the public node-limit gate.  Keep the traversal-stack label solely for
    // an allocator failure after that logical admission check.
    check_limit("input Atom nodes", scheduled, node_limit)?;
    pending.try_reserve(child_count).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: allocation_resource,
            requested: child_count,
        }
    })?;
    let before = pending.len();
    pending.extend(children.map(|child| (child, depth)));
    if pending.len() != before + child_count {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "Atom child iterator disagrees with its authenticated arity",
            },
        );
    }
    Ok(())
}

pub(super) fn checked_atom_shape(
    atom: AtomView<'_>,
    limits: SymbolicaAffineDenominatorLimits,
) -> Result<(usize, usize), SymbolicaAffineDenominatorError> {
    let mut count = 0usize;
    let mut maximum_depth = 0usize;
    let mut pending = vec![(atom, 0usize)];
    while let Some((current, depth)) = pending.pop() {
        count =
            count
                .checked_add(1)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "input Atom nodes",
                })?;
        check_limit("input Atom nodes", count, limits.max_input_nodes)?;
        if depth > limits.max_nesting_depth {
            return Err(SymbolicaAffineDenominatorError::ResourceLimit {
                resource: "input Atom nesting depth",
                requested: depth as u128,
                limit: limits.max_nesting_depth as u128,
            });
        }
        maximum_depth = maximum_depth.max(depth);
        let next_depth =
            depth
                .checked_add(1)
                .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
                    resource: "input Atom nesting depth",
                })?;
        match current {
            AtomView::Fun(function) => schedule_atom_views_with_depth(
                &mut pending,
                function.iter(),
                function.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Pow(power) => schedule_atom_views_with_depth(
                &mut pending,
                power.iter(),
                2,
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Mul(product) => schedule_atom_views_with_depth(
                &mut pending,
                product.iter(),
                product.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Add(sum) => schedule_atom_views_with_depth(
                &mut pending,
                sum.iter(),
                sum.get_nargs(),
                next_depth,
                count,
                limits.max_input_nodes,
                "input Atom traversal stack",
            )?,
            AtomView::Num(_) | AtomView::Var(_) => {}
        }
    }
    Ok((count, maximum_depth))
}

fn scalar_product_coordinate_count(
    loops: usize,
    externals: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    upper_triangular_count(loops)?
        .checked_add(loops.checked_mul(externals).ok_or(
            SymbolicaAffineDenominatorError::ResourceCountOverflow {
                resource: "loop-external scalar products",
            },
        )?)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "scalar-product coordinates",
        })
}

pub(super) fn upper_triangular_count(
    size: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    size.checked_add(1)
        .and_then(|next| size.checked_mul(next))
        .map(|product| product / 2)
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular scalar products",
        })
}

pub(super) fn upper_triangular_index(
    left: usize,
    right: usize,
    size: usize,
) -> Result<usize, SymbolicaAffineDenominatorError> {
    if left > right || right >= size {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "invalid upper-triangular scalar-product coordinate",
            },
        );
    }
    let preceding = left
        .checked_mul(size)
        .and_then(|value| {
            left.checked_mul(left.saturating_sub(1))
                .map(|triangle| value - triangle / 2)
        })
        .ok_or(SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        })?;
    preceding.checked_add(right - left).ok_or(
        SymbolicaAffineDenominatorError::ResourceCountOverflow {
            resource: "upper-triangular coordinate index",
        },
    )
}

fn scalar_product_coordinates(
    loops: usize,
    externals: usize,
    capacity: usize,
) -> Result<Vec<ScalarProductCoordinate>, SymbolicaAffineDenominatorError> {
    let mut coordinates = Vec::new();
    coordinates.try_reserve_exact(capacity).map_err(|_| {
        SymbolicaAffineDenominatorError::AllocationFailure {
            resource: "scalar-product coordinates",
            requested: capacity,
        }
    })?;
    for left in 0..loops {
        for right in left..loops {
            coordinates.push(ScalarProductCoordinate::LoopLoop { left, right });
        }
    }
    for loop_index in 0..loops {
        for external_index in 0..externals {
            coordinates.push(ScalarProductCoordinate::LoopExternal {
                loop_index,
                external_index,
            });
        }
    }
    if coordinates.len() != capacity {
        return Err(
            SymbolicaAffineDenominatorError::InternalVerificationFailure {
                detail: "scalar-product coordinate census disagrees with construction",
            },
        );
    }
    Ok(coordinates)
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

pub(super) fn maximum_combined_symbol_bytes(
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

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SymbolicaAffineDenominatorError> {
    if requested > limit {
        Err(SymbolicaAffineDenominatorError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}
