//! Authenticated, resource-bounded Symbolica text-to-Atom conversion.

mod census;
mod convert;
mod grammar;
mod lexical;
mod numeric;

use std::collections::BTreeMap;

use symbolica::atom::{Atom, Symbol};
use symbolica::parser::{ParseSettings, Token};
use symbolica::state::Workspace;

use super::error::Error;
use super::limits::{Limits, Stats, check_limit, checked_add};

use super::symbols::{authenticated_plain_symbol, rustred_identifier};
use convert::authenticated_token_to_atom;
use grammar::{
    ExpressionHeadPolicy, validate_and_authenticate_token_tree, validate_compact_token_grammar,
    validate_expression_token_tree,
};
use lexical::preflight_raw_source;
use numeric::validate_numeric_preconversion_envelope;

pub(super) use census::{
    AtomResourceCensus, authenticate_atom_tree, authenticate_project_parts, census_atom,
    census_atom_resources, census_project_parts,
};
pub(super) use grammar::{ClauseKind, IntegralSyntax, validate_clause_arity};

pub(super) enum RawSourceKind {
    CompactIntegral,
    BaseCoefficientExpression,
    DenominatorExpression,
    TensorExpression,
}

/// Bound raw parser work before Symbolica owns a recursive Token tree.  The
/// Token parser itself is iterative, but rejecting a deeply nested tree only
/// after construction would still recurse while that tree is dropped.
pub(super) struct AuthenticatedParsedSource {
    pub(super) atom: Atom,
    pub(super) preconversion_integer_bits: usize,
    pub(super) census: AtomResourceCensus,
}

pub(super) fn parse_authenticated_source(
    source: &str,
    kind: RawSourceKind,
    limits: Limits,
) -> Result<AuthenticatedParsedSource, Error> {
    check_limit(
        "Symbolica source bytes",
        source.len(),
        limits.max_input_bytes,
    )?;
    if source.contains('\u{1b}') {
        return Err(Error::UnsupportedToken {
            detail: "ANSI escape sequences are not accepted".to_owned(),
        });
    }
    preflight_raw_source(source, limits)?;
    let token = Token::parse(
        source,
        ParseSettings::symbolica().convert_mul_to_atom(false),
    )
    .map_err(Error::Parse)?;
    validate_and_authenticate_token_tree(&token, limits)?;
    match kind {
        RawSourceKind::CompactIntegral => validate_compact_token_grammar(&token, limits)?,
        RawSourceKind::BaseCoefficientExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::BaseCoefficient, limits)?;
        }
        RawSourceKind::DenominatorExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::Denominator, limits)?;
        }
        RawSourceKind::TensorExpression => {
            validate_expression_token_tree(&token, ExpressionHeadPolicy::Tensor, limits)?;
        }
    }
    let preconversion_integer_bits = validate_numeric_preconversion_envelope(&token, limits)?;

    let mut validated_names = BTreeMap::<String, String>::new();
    let mut pending = Vec::<&Token>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "raw identifier traversal",
            requested: 1,
        })?;
    pending.push(&token);
    while let Some(current) = pending.pop() {
        match current {
            Token::ID(raw) => {
                if validated_names.contains_key(raw.as_str()) {
                    continue;
                }
                let logical = rustred_identifier(raw.as_str())?;
                let requested = checked_add("raw Symbolica identifiers", validated_names.len(), 1)?;
                check_limit(
                    "unique raw Symbolica identifiers",
                    requested,
                    limits.max_unique_identifiers,
                )?;
                validated_names.insert(raw.to_string(), logical.to_owned());
            }
            Token::Op(_, _, _, children) | Token::Fn(_, _, children) => {
                for child in children {
                    let requested = checked_add("raw identifier traversal", pending.len(), 1)?;
                    check_limit("raw identifier traversal", requested, limits.max_atom_nodes)?;
                    pending
                        .try_reserve(1)
                        .map_err(|_| Error::AllocationFailure {
                            resource: "raw identifier traversal",
                            requested,
                        })?;
                    pending.push(child);
                }
            }
            Token::Number(_, _) => {}
            other => {
                return Err(Error::UnsupportedToken {
                    detail: other.to_string(),
                });
            }
        }
    }

    let mut symbols = BTreeMap::<String, Symbol>::new();
    for (raw, logical) in validated_names {
        symbols.insert(raw, authenticated_plain_symbol(&logical, limits)?);
    }
    let mut atom = Atom::new();
    Workspace::get_local()
        .with(|workspace| authenticated_token_to_atom(&token, workspace, &symbols, &mut atom))?;
    let census = census_atom_resources(
        atom.as_view(),
        limits.max_atom_nodes,
        limits.max_nesting_depth,
    )?;
    check_limit(
        "one parsed Atom integer bits",
        census.integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;
    check_limit(
        "one parsed Atom bytes",
        census.packed_bytes,
        limits.max_retained_atom_bytes,
    )?;
    Ok(AuthenticatedParsedSource {
        atom,
        preconversion_integer_bits,
        census,
    })
}

pub(super) fn parse_expression_accumulating(
    source: &str,
    kind: RawSourceKind,
    stats: &mut Stats,
    limits: Limits,
) -> Result<Atom, Error> {
    stats.input_bytes = checked_add(
        "explicit Symbolica expression bytes",
        stats.input_bytes,
        source.len(),
    )?;
    check_limit(
        "explicit Symbolica expression bytes",
        stats.input_bytes,
        limits.max_input_bytes,
    )?;
    let parsed = parse_authenticated_source(source, kind, limits)?;
    stats.preconversion_integer_bits = checked_add(
        "aggregate pre-conversion integer bits",
        stats.preconversion_integer_bits,
        parsed.preconversion_integer_bits,
    )?;
    check_limit(
        "aggregate pre-conversion integer bits",
        stats.preconversion_integer_bits,
        limits.max_preconversion_integer_bits,
    )?;
    stats.atom_nodes = checked_add(
        "explicit Symbolica expression nodes",
        stats.atom_nodes,
        parsed.census.nodes,
    )?;
    check_limit(
        "explicit Symbolica expression nodes",
        stats.atom_nodes,
        limits.max_atom_nodes,
    )?;
    stats.maximum_depth = stats.maximum_depth.max(parsed.census.maximum_depth);
    stats.retained_atom_integer_bits = checked_add(
        "aggregate explicit Atom integer bits",
        stats.retained_atom_integer_bits,
        parsed.census.integer_bits,
    )?;
    check_limit(
        "aggregate explicit Atom integer bits",
        stats.retained_atom_integer_bits,
        limits.max_retained_atom_integer_bits,
    )?;
    stats.retained_atom_bytes = checked_add(
        "aggregate explicit Atom bytes",
        stats.retained_atom_bytes,
        parsed.census.packed_bytes,
    )?;
    check_limit(
        "aggregate explicit Atom bytes",
        stats.retained_atom_bytes,
        limits.max_retained_atom_bytes,
    )?;
    Ok(parsed.atom)
}
