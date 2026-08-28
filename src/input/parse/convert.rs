//! Materialization of a fully authenticated Token tree into Symbolica atoms.

use std::collections::BTreeMap;

use symbolica::atom::{Atom, FunctionBuilder, Symbol};
use symbolica::domains::integer::Integer;
use symbolica::parser::{Operator, Token};
use symbolica::state::Workspace;

use super::super::error::Error;

pub(super) fn authenticated_token_to_atom(
    token: &Token,
    workspace: &Workspace,
    symbols: &BTreeMap<String, Symbol>,
    out: &mut Atom,
) -> Result<(), Error> {
    match token {
        Token::Number(number, false) => {
            let integer = number.parse::<Integer>().map_err(|error| {
                Error::Parse(format!(
                    "could not parse authenticated integer {number:?}: {error}"
                ))
            })?;
            out.to_num(integer);
        }
        Token::ID(raw) => {
            let symbol = symbols.get(raw.as_str()).ok_or_else(|| {
                Error::Parse(format!("authenticated symbol map lost identifier {raw:?}"))
            })?;
            out.to_var(*symbol);
        }
        Token::Op(_, _, operator, arguments) => match operator {
            Operator::Mul => {
                let factors = authenticated_token_arguments_to_atoms(
                    arguments,
                    workspace,
                    symbols,
                    "authenticated product factors",
                )?;
                Atom::mul_many(factors).as_view().clone_into(out);
            }
            Operator::Add => {
                let terms = authenticated_token_arguments_to_atoms(
                    arguments,
                    workspace,
                    symbols,
                    "authenticated sum terms",
                )?;
                Atom::add_many(terms).as_view().clone_into(out);
            }
            Operator::Pow => {
                let mut base = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut base)?;
                let mut exponent = workspace.new_atom();
                authenticated_token_to_atom(&arguments[1], workspace, symbols, &mut exponent)?;
                let mut power = workspace.new_atom();
                power.to_pow(base.as_view(), exponent.as_view());
                power.as_view().normalize(workspace, out);
            }
            Operator::Neg => {
                let mut value = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut value)?;
                value.as_view().neg_with_ws_into(workspace, out);
            }
            Operator::Inv => {
                let mut value = workspace.new_atom();
                authenticated_token_to_atom(&arguments[0], workspace, symbols, &mut value)?;
                let minus_one = workspace.new_num(-1);
                let mut power = workspace.new_atom();
                power.to_pow(value.as_view(), minus_one.as_view());
                power.as_view().normalize(workspace, out);
            }
            Operator::Argument => {
                return Err(Error::UnsupportedToken {
                    detail: "argument operator reached authenticated conversion".to_owned(),
                });
            }
        },
        Token::Fn(_, _, children) => {
            let Some(Token::ID(raw_head)) = children.first() else {
                return Err(Error::UnsupportedToken {
                    detail: "function without an authenticated identifier head".to_owned(),
                });
            };
            let head = symbols.get(raw_head.as_str()).ok_or_else(|| {
                Error::Parse(format!(
                    "authenticated symbol map lost function head {raw_head:?}"
                ))
            })?;
            let arguments = authenticated_token_arguments_to_atoms(
                &children[1..],
                workspace,
                symbols,
                "authenticated function arguments",
            )?;
            FunctionBuilder::new(*head)
                .add_args(arguments)
                .finish()
                .as_view()
                .clone_into(out);
        }
        other => {
            return Err(Error::UnsupportedToken {
                detail: other.to_string(),
            });
        }
    }
    Ok(())
}

/// Materialize one already-authenticated argument slice for Symbolica's public
/// n-ary builders. `validate_and_authenticate_token_tree` has bounded the
/// entire Token tree before this conversion, and the exact reserve keeps this
/// remaining allocation failure typed.
pub(super) fn authenticated_token_arguments_to_atoms(
    arguments: &[Token],
    workspace: &Workspace,
    symbols: &BTreeMap<String, Symbol>,
    resource: &'static str,
) -> Result<Vec<Atom>, Error> {
    let mut converted = Vec::new();
    converted
        .try_reserve_exact(arguments.len())
        .map_err(|_| Error::AllocationFailure {
            resource,
            requested: arguments.len(),
        })?;
    for argument in arguments {
        let mut child = workspace.new_atom();
        authenticated_token_to_atom(argument, workspace, symbols, &mut child)?;
        converted.push(child.into_inner());
    }
    Ok(converted)
}
