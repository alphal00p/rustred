//! Symbol registration, label authentication, and scalar discovery.

use symbolica::atom::{Atom, AtomView, NamespacedSymbol, SymbolBuilder, UserData};
use symbolica::prelude::{Rational, Symbol};

use super::error::Error;
use super::limits::{Limits, Stats, check_limit, checked_add};
use super::request::AtomPropagator;

pub(super) const RUSTRED_NAMESPACE_PREFIX: &str = "rustred::";
pub(super) const RESERVED_NAMES: &[&str] = &[
    "I",
    "name",
    "loops",
    "externals",
    "parameters",
    "dimension",
    "prop",
    "power_shift",
    "gram",
    "numerator",
    "sp",
    "vec",
    "metric",
    "J",
];

pub(super) fn family_scalar_atoms<'a>(
    dimension: &'a Atom,
    props: &'a [AtomPropagator],
    gram: &'a [Atom],
) -> Result<Vec<&'a Atom>, Error> {
    let prop_slots = props
        .len()
        .checked_mul(2)
        .ok_or(Error::ResourceCountOverflow {
            resource: "family scalar expressions",
        })?;
    let requested = checked_add(
        "family scalar expressions",
        checked_add("family scalar expressions", 1, prop_slots)?,
        gram.len(),
    )?;
    let mut atoms = Vec::new();
    atoms
        .try_reserve_exact(requested)
        .map_err(|_| Error::AllocationFailure {
            resource: "family scalar expressions",
            requested,
        })?;
    atoms.push(dimension);
    for prop in props {
        atoms.push(&prop.expression);
        if let Some(shift) = &prop.power_shift {
            atoms.push(shift);
        }
    }
    atoms.extend(gram);
    Ok(atoms)
}

pub(super) fn discover_scalar_symbols(
    atoms: &[&Atom],
    momenta: &[&str],
    forbidden_identifiers: &[&str],
    stats: &mut Stats,
    limits: Limits,
) -> Result<Vec<String>, Error> {
    let mut output = Vec::<String>::new();
    let mut pending = Vec::<AtomView<'_>>::new();
    pending
        .try_reserve(atoms.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "scalar-symbol traversal",
            requested: atoms.len(),
        })?;
    pending.extend(atoms.iter().map(|atom| atom.as_view()));
    while let Some(atom) = pending.pop() {
        stats.symbol_inspections =
            checked_add("scalar symbol inspections", stats.symbol_inspections, 1)?;
        check_limit(
            "scalar symbol inspections",
            stats.symbol_inspections,
            limits.max_symbol_inspections,
        )?;
        match atom {
            AtomView::Var(variable) => {
                let label = symbol_label(variable.get_symbol(), "scalar parameter", limits)?;
                if RESERVED_NAMES.contains(&label.as_str()) {
                    return Err(Error::ReservedScalarSymbol { symbol: label });
                }
                if momenta.iter().any(|candidate| *candidate == label) {
                    continue;
                }
                if forbidden_identifiers
                    .iter()
                    .any(|candidate| *candidate == label)
                {
                    return Err(Error::IdentifierUsedAsScalar { symbol: label });
                }
                if !output.iter().any(|candidate| candidate == &label) {
                    let requested = checked_add("inferred parameters", output.len(), 1)?;
                    check_limit("inferred parameters", requested, limits.max_parameters)?;
                    output
                        .try_reserve(1)
                        .map_err(|_| Error::AllocationFailure {
                            resource: "inferred parameters",
                            requested,
                        })?;
                    output.push(label);
                }
            }
            AtomView::Fun(function) => append_pending_atoms(&mut pending, function.iter(), limits)?,
            AtomView::Pow(power) => append_pending_atoms(&mut pending, power.iter(), limits)?,
            AtomView::Mul(product) => append_pending_atoms(&mut pending, product.iter(), limits)?,
            AtomView::Add(sum) => append_pending_atoms(&mut pending, sum.iter(), limits)?,
            AtomView::Num(_) => {}
        }
    }
    output.sort_unstable();
    Ok(output)
}

pub(super) fn rustred_identifier(raw: &str) -> Result<&str, Error> {
    let logical = if let Some(label) = raw.strip_prefix("rustred::{}::") {
        label
    } else if let Some(label) = raw.strip_prefix(RUSTRED_NAMESPACE_PREFIX) {
        label
    } else if raw.contains("::") {
        return Err(Error::ForeignScalarSymbol {
            symbol: raw.to_owned(),
        });
    } else {
        raw
    };
    if logical.is_empty() || logical.contains("::") || logical.ends_with('_') {
        return Err(Error::InvalidLabelText {
            role: "Symbolica identifier",
            label: raw.to_owned(),
        });
    }
    Ok(logical)
}

pub(super) fn validate_identifier_text(identifier: &str, limits: Limits) -> Result<(), Error> {
    check_limit("identifier bytes", identifier.len(), limits.max_label_bytes)?;
    if identifier.is_empty() || identifier.contains("::") || identifier.ends_with('_') {
        return Err(Error::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        });
    }
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{identifier}");
    if NamespacedSymbol::try_parse(&qualified).is_none() {
        return Err(Error::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn authenticated_plain_symbol(
    identifier: &str,
    limits: Limits,
) -> Result<Symbol, Error> {
    validate_identifier_text(identifier, limits)?;
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{identifier}");
    let namespaced =
        NamespacedSymbol::try_parse(&qualified).ok_or_else(|| Error::InvalidLabelText {
            role: "Symbolica identifier",
            label: identifier.to_owned(),
        })?;
    let symbol = SymbolBuilder::new(namespaced)
        .build()
        .map_err(|detail| Error::GrammarSymbol {
            name: "input symbol",
            detail: detail.to_string(),
        })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

pub(super) fn authenticate_symbol_properties(
    symbol: Symbol,
    qualified: &str,
    wildcard_level: u8,
) -> Result<(), Error> {
    let unsafe_symbol = |reason| Error::UnsafeRegisteredSymbol {
        symbol: qualified.to_owned(),
        reason,
    };
    if symbol.get_name() != qualified {
        return Err(unsafe_symbol("canonical name mismatch"));
    }
    if symbol.get_wildcard_level() != wildcard_level {
        return Err(unsafe_symbol("unexpected wildcard level"));
    }
    if symbol.has_attributes() {
        return Err(unsafe_symbol("attributes or tags are present"));
    }
    if !symbol.is_exportable() {
        return Err(unsafe_symbol("a custom callback is registered"));
    }
    if !symbol.get_aliases().is_empty() {
        return Err(unsafe_symbol("aliases are registered"));
    }
    if !matches!(symbol.get_data(), UserData::None) {
        return Err(unsafe_symbol("user data is registered"));
    }
    Ok(())
}

pub(super) fn plain_grammar_symbol(name: &'static str) -> Result<Symbol, Error> {
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{name}");
    let namespaced =
        NamespacedSymbol::try_parse(&qualified).ok_or_else(|| Error::GrammarSymbol {
            name,
            detail: "invalid namespaced symbol".to_owned(),
        })?;
    let symbol = SymbolBuilder::new(namespaced)
        .build()
        .map_err(|error| Error::GrammarSymbol {
            name,
            detail: error.to_string(),
        })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

pub(super) fn label_symbol(
    label: &str,
    role: &'static str,
    limits: Limits,
) -> Result<Symbol, Error> {
    validate_label_text(label, role, limits)?;
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{label}");
    let namespaced =
        NamespacedSymbol::try_parse(&qualified).ok_or_else(|| Error::InvalidLabelText {
            role,
            label: label.to_owned(),
        })?;
    let symbol = SymbolBuilder::new(namespaced)
        .build()
        .map_err(|_| Error::InvalidLabelText {
            role,
            label: label.to_owned(),
        })?;
    authenticate_symbol_properties(symbol, &qualified, 0)?;
    Ok(symbol)
}

pub(super) fn collect_atom_views<'a>(
    arguments: impl Iterator<Item = AtomView<'a>>,
    count: usize,
) -> Result<Vec<AtomView<'a>>, Error> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailure {
            resource: "clause arguments",
            requested: count,
        })?;
    for argument in arguments {
        output.push(argument);
    }
    Ok(output)
}

pub(super) fn collect_labels(
    args: &[AtomView<'_>],
    role: &'static str,
    limits: Limits,
) -> Result<Vec<String>, Error> {
    let mut labels = Vec::new();
    labels
        .try_reserve_exact(args.len())
        .map_err(|_| Error::AllocationFailure {
            resource: "input labels",
            requested: args.len(),
        })?;
    for &arg in args {
        labels.push(atom_label(arg, role, limits)?);
    }
    Ok(labels)
}

pub(super) fn atom_label(
    atom: AtomView<'_>,
    role: &'static str,
    limits: Limits,
) -> Result<String, Error> {
    let AtomView::Var(variable) = atom else {
        return Err(Error::InvalidLabel {
            role,
            expression: atom.to_owned(),
        });
    };
    let label = symbol_label(variable.get_symbol(), role, limits)?;
    validate_label_text(&label, role, limits)?;
    Ok(label)
}

pub(super) fn symbol_label(
    symbol: Symbol,
    _role: &'static str,
    limits: Limits,
) -> Result<String, Error> {
    let qualified = symbol.get_name();
    let Some(label) = qualified.strip_prefix(RUSTRED_NAMESPACE_PREFIX) else {
        return Err(Error::ForeignScalarSymbol {
            symbol: qualified.to_owned(),
        });
    };
    if label.contains("::") || label.ends_with('_') {
        return Err(Error::ForeignScalarSymbol {
            symbol: qualified.to_owned(),
        });
    }
    check_limit("label bytes", label.len(), limits.max_label_bytes)?;
    Ok(label.to_owned())
}

pub(super) fn validate_label_text(
    label: &str,
    role: &'static str,
    limits: Limits,
) -> Result<(), Error> {
    check_limit("label bytes", label.len(), limits.max_label_bytes)?;
    if label.is_empty() || label.contains("::") || label.ends_with('_') {
        return Err(Error::InvalidLabelText {
            role,
            label: label.to_owned(),
        });
    }
    if RESERVED_NAMES.contains(&label) {
        return Err(Error::ReservedLabel {
            role,
            label: label.to_owned(),
        });
    }
    let qualified = format!("{RUSTRED_NAMESPACE_PREFIX}{label}");
    if NamespacedSymbol::try_parse(&qualified).is_none() {
        return Err(Error::InvalidLabelText {
            role,
            label: label.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn validate_ordered_labels(
    labels: &[String],
    role: &'static str,
    maximum: usize,
    limits: Limits,
) -> Result<(), Error> {
    check_limit(role, labels.len(), maximum)?;
    for (ordinal, label) in labels.iter().enumerate() {
        validate_label_text(label, role, limits)?;
        if labels[..ordinal].iter().any(|candidate| candidate == label) {
            return Err(Error::DuplicateLabel {
                role,
                label: label.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn atom_i64(atom: AtomView<'_>) -> Option<i64> {
    let value = Rational::try_from(atom).ok()?;
    if !value.is_integer() {
        return None;
    }
    value.numerator().to_i64()
}

pub(super) fn append_pending_atoms<'a>(
    pending: &mut Vec<AtomView<'a>>,
    children: impl Iterator<Item = AtomView<'a>>,
    limits: Limits,
) -> Result<(), Error> {
    for child in children {
        let requested = checked_add("scalar-symbol traversal stack", pending.len(), 1)?;
        check_limit(
            "scalar-symbol traversal stack",
            requested,
            limits.max_atom_nodes,
        )?;
        pending
            .try_reserve(1)
            .map_err(|_| Error::AllocationFailure {
                resource: "scalar-symbol traversal stack",
                requested,
            })?;
        pending.push(child);
    }
    Ok(())
}
