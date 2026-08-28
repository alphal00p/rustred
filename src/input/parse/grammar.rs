//! Position-sensitive Token grammar and compact-clause classification.

use std::collections::BTreeMap;

use symbolica::parser::{Operator, Token};
use symbolica::prelude::Symbol;

use super::super::error::Error;
use super::super::limits::{Limits, check_limit, checked_add};
use super::super::symbols::{
    RESERVED_NAMES, plain_grammar_symbol, rustred_identifier, validate_identifier_text,
    validate_label_text,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpressionHeadPolicy {
    BaseCoefficient,
    Denominator,
    Tensor,
}

pub(in crate::input) fn validate_expression_token_tree(
    token: &Token,
    policy: ExpressionHeadPolicy,
    limits: Limits,
) -> Result<(), Error> {
    let mut pending = Vec::<&Token>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "position-sensitive Token validation",
            requested: 1,
        })?;
    pending.push(token);
    while let Some(current) = pending.pop() {
        let children: &[Token] = match current {
            Token::Fn(_, _, children) => {
                let Some(Token::ID(raw_head)) = children.first() else {
                    return Err(Error::UnsupportedToken {
                        detail: "expression function head is not an identifier".to_owned(),
                    });
                };
                let head = rustred_identifier(raw_head.as_str())?;
                let allowed = match policy {
                    ExpressionHeadPolicy::BaseCoefficient => false,
                    ExpressionHeadPolicy::Denominator => head == "sp",
                    ExpressionHeadPolicy::Tensor => {
                        matches!(head, "sp" | "vec" | "metric" | "J")
                    }
                };
                if !allowed {
                    return Err(Error::UnsupportedToken {
                        detail: format!(
                            "function head {head:?} is not allowed in a {policy:?} expression"
                        ),
                    });
                }
                let arity = children.len().saturating_sub(1);
                if matches!(head, "sp" | "vec" | "metric") && arity != 2 {
                    return Err(Error::UnsupportedToken {
                        detail: format!("expression head {head:?} needs exactly 2 arguments"),
                    });
                }
                &children[1..]
            }
            Token::Op(_, _, _, children) => children,
            Token::ID(_) | Token::Number(_, _) => continue,
            other => {
                return Err(Error::UnsupportedToken {
                    detail: other.to_string(),
                });
            }
        };
        for child in children {
            let requested = checked_add(
                "position-sensitive Token validation stack",
                pending.len(),
                1,
            )?;
            check_limit(
                "position-sensitive Token validation stack",
                requested,
                limits.max_atom_nodes,
            )?;
            pending
                .try_reserve(1)
                .map_err(|_| Error::AllocationFailure {
                    resource: "position-sensitive Token validation stack",
                    requested,
                })?;
            pending.push(child);
        }
    }
    Ok(())
}

/// Parse without permitting Symbolica to convert partial products into Atoms.
/// Every identifier is then mapped explicitly to an authenticated plain
/// `rustred` symbol before the sole controlled Token-to-Atom conversion.
pub(in crate::input) fn validate_and_authenticate_token_tree(
    token: &Token,
    limits: Limits,
) -> Result<(), Error> {
    let mut pending = Vec::<(&Token, usize)>::new();
    pending
        .try_reserve_exact(1)
        .map_err(|_| Error::AllocationFailure {
            resource: "raw Token census",
            requested: 1,
        })?;
    pending.push((token, 0));
    let mut nodes = 0usize;
    while let Some((current, depth)) = pending.pop() {
        nodes = checked_add("raw Token nodes", nodes, 1)?;
        check_limit("raw Token nodes", nodes, limits.max_atom_nodes)?;
        check_limit("raw Token nesting depth", depth, limits.max_nesting_depth)?;
        let children: &[Token] = match current {
            Token::Number(number, imaginary) => {
                let Some(digits) = raw_integer_digits(number) else {
                    return Err(Error::UnsupportedToken {
                        detail: format!("non-integral numeric literal {number}"),
                    });
                };
                if *imaginary {
                    return Err(Error::UnsupportedToken {
                        detail: format!("non-integral numeric literal {number}"),
                    });
                }
                check_limit(
                    "raw integer digits",
                    digits.len(),
                    limits.max_raw_integer_digits,
                )?;
                continue;
            }
            Token::ID(raw) => {
                let logical = rustred_identifier(raw.as_str())?;
                validate_identifier_text(logical, limits)?;
                continue;
            }
            Token::Op(more_left, more_right, operator, children) => {
                if *more_left || *more_right {
                    return Err(Error::UnsupportedToken {
                        detail: "incomplete operator token".to_owned(),
                    });
                }
                let valid_arity = match operator {
                    Operator::Mul | Operator::Add => !children.is_empty(),
                    Operator::Pow => children.len() == 2,
                    Operator::Neg | Operator::Inv => children.len() == 1,
                    Operator::Argument => false,
                };
                if !valid_arity {
                    return Err(Error::UnsupportedToken {
                        detail: format!("invalid {operator} token arity {}", children.len()),
                    });
                }
                if *operator == Operator::Pow {
                    let exponent =
                        raw_i64(&children[1]).ok_or_else(|| Error::UnsupportedToken {
                            detail: format!(
                                "power exponent must be a syntactic exact signed integer, found {}",
                                children[1]
                            ),
                        })?;
                    let magnitude = exponent.unsigned_abs();
                    let requested =
                        usize::try_from(magnitude).map_err(|_| Error::ResourceCountOverflow {
                            resource: "raw absolute power",
                        })?;
                    check_limit(
                        "raw absolute power",
                        requested,
                        limits.max_abs_power as usize,
                    )?;
                }
                children
            }
            Token::Fn(more_right, bracket, children) => {
                if *more_right || *bracket || children.is_empty() {
                    return Err(Error::UnsupportedToken {
                        detail: "incomplete, bracketed, or headless function token".to_owned(),
                    });
                }
                let Token::ID(raw_head) = &children[0] else {
                    return Err(Error::UnsupportedToken {
                        detail: "function head is not an identifier".to_owned(),
                    });
                };
                let head = rustred_identifier(raw_head.as_str())?;
                if !RESERVED_NAMES.contains(&head) {
                    return Err(Error::UnsupportedToken {
                        detail: format!("function head {head:?} is outside the v1 grammar"),
                    });
                }
                children
            }
            Token::SpecialNumber(character) => {
                return Err(Error::UnsupportedToken {
                    detail: format!("special number {character}"),
                });
            }
            Token::RationalPolynomial(_)
            | Token::ParsedMul(_)
            | Token::Start
            | Token::OpenParenthesis
            | Token::CloseParenthesis
            | Token::CloseBracket
            | Token::EOF => {
                return Err(Error::UnsupportedToken {
                    detail: current.to_string(),
                });
            }
        };
        let child_depth = depth.checked_add(1).ok_or(Error::ResourceCountOverflow {
            resource: "raw Token nesting depth",
        })?;
        check_limit(
            "raw Token nesting depth",
            child_depth,
            limits.max_nesting_depth,
        )?;
        for child in children {
            let requested = checked_add("raw Token census stack", pending.len(), 1)?;
            check_limit("raw Token census stack", requested, limits.max_atom_nodes)?;
            pending
                .try_reserve(1)
                .map_err(|_| Error::AllocationFailure {
                    resource: "raw Token census stack",
                    requested,
                })?;
            pending.push((child, child_depth));
        }
    }
    Ok(())
}

pub(in crate::input) fn validate_compact_token_grammar(
    token: &Token,
    limits: Limits,
) -> Result<(), Error> {
    let (root, clauses) = raw_function_parts(token)?;
    if root != "I" {
        return Err(Error::WrongRoot);
    }
    if clauses.is_empty() {
        return Err(Error::WrongRoot);
    }
    check_limit("I clauses", clauses.len(), limits.max_clauses)?;
    for (ordinal, clause) in clauses.iter().enumerate() {
        let (head, arguments) = raw_function_parts(clause)?;
        let kind = ClauseKind::from_head(head).ok_or_else(|| Error::UnsupportedToken {
            detail: format!("unknown I clause {ordinal} head {head:?}"),
        })?;
        validate_clause_arity(kind, arguments.len(), ordinal)?;
        match kind {
            ClauseKind::Name => {
                validate_raw_label(&arguments[0], "family name", limits)?;
            }
            ClauseKind::Loops => {
                for argument in arguments {
                    validate_raw_label(argument, "loop momentum", limits)?;
                }
            }
            ClauseKind::Externals => {
                for argument in arguments {
                    validate_raw_label(argument, "external momentum", limits)?;
                }
            }
            ClauseKind::Parameters => {
                for argument in arguments {
                    validate_raw_label(argument, "parameter", limits)?;
                }
            }
            ClauseKind::Dimension => {
                validate_expression_token_tree(
                    &arguments[0],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
            ClauseKind::Numerator => {
                validate_expression_token_tree(
                    &arguments[0],
                    ExpressionHeadPolicy::Tensor,
                    limits,
                )?;
            }
            ClauseKind::Prop => {
                let id = validate_raw_label(&arguments[0], "propagator", limits)?;
                validate_expression_token_tree(
                    &arguments[1],
                    ExpressionHeadPolicy::Denominator,
                    limits,
                )?;
                if raw_i64(&arguments[2]).is_none() {
                    return Err(Error::UnsupportedToken {
                        detail: format!("target power for {id} is not an exact i64 integer"),
                    });
                }
            }
            ClauseKind::PowerShift => {
                validate_raw_label(&arguments[0], "power-shift propagator", limits)?;
                validate_expression_token_tree(
                    &arguments[1],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
            ClauseKind::Gram => {
                validate_raw_label(&arguments[0], "Gram momentum", limits)?;
                validate_raw_label(&arguments[1], "Gram momentum", limits)?;
                validate_expression_token_tree(
                    &arguments[2],
                    ExpressionHeadPolicy::BaseCoefficient,
                    limits,
                )?;
            }
        }
    }
    Ok(())
}

pub(in crate::input) fn raw_function_parts(token: &Token) -> Result<(&str, &[Token]), Error> {
    let Token::Fn(false, false, children) = token else {
        return Err(Error::WrongRoot);
    };
    let Some(Token::ID(raw_head)) = children.first() else {
        return Err(Error::WrongRoot);
    };
    Ok((rustred_identifier(raw_head.as_str())?, &children[1..]))
}

pub(in crate::input) fn validate_raw_label<'a>(
    token: &'a Token,
    role: &'static str,
    limits: Limits,
) -> Result<&'a str, Error> {
    let Token::ID(raw) = token else {
        return Err(Error::UnsupportedToken {
            detail: format!("{role} is not an identifier"),
        });
    };
    let label = rustred_identifier(raw.as_str())?;
    validate_label_text(label, role, limits)?;
    Ok(label)
}

pub(in crate::input) fn raw_i64(token: &Token) -> Option<i64> {
    match token {
        Token::Number(number, false) => number.parse::<i64>().ok(),
        Token::Op(false, false, Operator::Neg, arguments) if arguments.len() == 1 => {
            let Token::Number(number, false) = &arguments[0] else {
                return None;
            };
            let magnitude = number.parse::<u64>().ok()?;
            if magnitude == (i64::MAX as u64) + 1 {
                Some(i64::MIN)
            } else {
                i64::try_from(magnitude).ok()?.checked_neg()
            }
        }
        _ => None,
    }
}

pub(in crate::input) fn raw_integer_digits(number: &str) -> Option<&str> {
    let digits = number.strip_prefix('-').unwrap_or(number);
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        None
    } else {
        Some(digits)
    }
}

pub(in crate::input) fn validate_clause_arity(
    kind: ClauseKind,
    actual: usize,
    clause: usize,
) -> Result<(), Error> {
    let valid = match kind {
        ClauseKind::Name | ClauseKind::Dimension | ClauseKind::Numerator => actual == 1,
        ClauseKind::Loops => actual >= 1,
        ClauseKind::Externals | ClauseKind::Parameters => true,
        ClauseKind::Prop | ClauseKind::Gram => actual == 3,
        ClauseKind::PowerShift => actual == 2,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::WrongClauseArity {
            clause,
            kind: kind.head(),
            expected: kind.expected_arity(),
            actual,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::input) enum ClauseKind {
    Name,
    Loops,
    Externals,
    Parameters,
    Dimension,
    Prop,
    PowerShift,
    Gram,
    Numerator,
}

impl ClauseKind {
    pub(in crate::input) const ALL: [Self; 9] = [
        Self::Name,
        Self::Loops,
        Self::Externals,
        Self::Parameters,
        Self::Dimension,
        Self::Prop,
        Self::PowerShift,
        Self::Gram,
        Self::Numerator,
    ];

    pub(in crate::input) const fn head(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Loops => "loops",
            Self::Externals => "externals",
            Self::Parameters => "parameters",
            Self::Dimension => "dimension",
            Self::Prop => "prop",
            Self::PowerShift => "power_shift",
            Self::Gram => "gram",
            Self::Numerator => "numerator",
        }
    }

    pub(in crate::input) const fn expected_arity(self) -> &'static str {
        match self {
            Self::Name | Self::Dimension | Self::Numerator => "exactly 1",
            Self::Loops => "at least 1",
            Self::Externals | Self::Parameters => "zero or more",
            Self::Prop | Self::Gram => "exactly 3",
            Self::PowerShift => "exactly 2",
        }
    }

    pub(in crate::input) fn from_head(head: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.head() == head)
    }
}

pub(in crate::input) struct IntegralSyntax {
    pub(in crate::input) root: Symbol,
    heads: BTreeMap<&'static str, Symbol>,
}

impl IntegralSyntax {
    pub(in crate::input) fn try_new() -> Result<Self, Error> {
        let mut heads = BTreeMap::new();
        for &name in RESERVED_NAMES {
            let symbol = plain_grammar_symbol(name)?;
            heads.insert(name, symbol);
        }
        Ok(Self {
            root: heads["I"],
            heads,
        })
    }

    pub(in crate::input) fn head(&self, kind: ClauseKind) -> Symbol {
        self.heads[kind.head()]
    }

    pub(in crate::input) fn classify(&self, symbol: Symbol) -> Option<ClauseKind> {
        ClauseKind::ALL
            .into_iter()
            .find(|kind| self.head(*kind) == symbol)
    }
}
