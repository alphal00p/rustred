//! Conservative integer-growth admission before Token-to-Atom conversion.

use symbolica::parser::{Operator, Token};

use super::super::error::Error;
use super::super::limits::{Limits, check_limit};
use super::grammar::{raw_i64, raw_integer_digits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NumericBitEnvelope {
    numerator: usize,
    denominator: usize,
}

pub(super) fn validate_numeric_preconversion_envelope(
    token: &Token,
    limits: Limits,
) -> Result<usize, Error> {
    let mut aggregate = 0usize;
    analyze_numeric_token(token, &mut aggregate, limits)?;
    Ok(aggregate)
}

fn analyze_numeric_token(
    token: &Token,
    aggregate: &mut usize,
    limits: Limits,
) -> Result<Option<NumericBitEnvelope>, Error> {
    let envelope = match token {
        Token::Number(number, false) => {
            let digits = raw_integer_digits(number).ok_or_else(|| Error::UnsupportedToken {
                detail: format!("non-integral numeric literal {number}"),
            })?;
            let significant = digits.trim_start_matches('0').len().max(1);
            let numerator = significant
                .checked_mul(4)
                .ok_or(Error::ResourceCountOverflow {
                    resource: "pre-conversion integer bits",
                })?;
            Some(NumericBitEnvelope {
                numerator,
                denominator: 1,
            })
        }
        Token::ID(_) => None,
        Token::Fn(_, _, children) => {
            for argument in children.iter().skip(1) {
                analyze_numeric_token(argument, aggregate, limits)?;
            }
            None
        }
        Token::Op(_, _, operator, arguments) => match operator {
            Operator::Add | Operator::Mul => {
                let mut combined = None::<NumericBitEnvelope>;
                let mut all_numeric = true;
                for argument in arguments {
                    let child = analyze_numeric_token(argument, aggregate, limits)?;
                    let Some(child) = child else {
                        all_numeric = false;
                        continue;
                    };
                    if all_numeric {
                        combined = Some(match combined {
                            None => child,
                            Some(current) if *operator == Operator::Add => {
                                add_numeric_envelopes(current, child)?
                            }
                            Some(current) => multiply_numeric_envelopes(current, child)?,
                        });
                    }
                }
                if all_numeric { combined } else { None }
            }
            Operator::Neg => analyze_numeric_token(&arguments[0], aggregate, limits)?,
            Operator::Inv => {
                analyze_numeric_token(&arguments[0], aggregate, limits)?.map(|value| {
                    NumericBitEnvelope {
                        numerator: value.denominator,
                        denominator: value.numerator,
                    }
                })
            }
            Operator::Pow => {
                let base = analyze_numeric_token(&arguments[0], aggregate, limits)?;
                analyze_numeric_token(&arguments[1], aggregate, limits)?;
                match base {
                    Some(base) => {
                        let exponent =
                            raw_i64(&arguments[1]).ok_or_else(|| Error::UnsupportedToken {
                                detail: "authenticated power lost its exact integer exponent"
                                    .to_owned(),
                            })?;
                        let magnitude = usize::try_from(exponent.unsigned_abs()).map_err(|_| {
                            Error::ResourceCountOverflow {
                                resource: "pre-conversion power magnitude",
                            }
                        })?;
                        if magnitude == 0 {
                            Some(NumericBitEnvelope {
                                numerator: 1,
                                denominator: 1,
                            })
                        } else {
                            let numerator = base.numerator.checked_mul(magnitude).ok_or(
                                Error::ResourceCountOverflow {
                                    resource: "pre-conversion power numerator bits",
                                },
                            )?;
                            let denominator = base.denominator.checked_mul(magnitude).ok_or(
                                Error::ResourceCountOverflow {
                                    resource: "pre-conversion power denominator bits",
                                },
                            )?;
                            if exponent < 0 {
                                Some(NumericBitEnvelope {
                                    numerator: denominator,
                                    denominator: numerator,
                                })
                            } else {
                                Some(NumericBitEnvelope {
                                    numerator,
                                    denominator,
                                })
                            }
                        }
                    }
                    None => None,
                }
            }
            Operator::Argument => None,
        },
        other => {
            return Err(Error::UnsupportedToken {
                detail: other.to_string(),
            });
        }
    };
    if let Some(envelope) = envelope {
        let retained = envelope.numerator.checked_add(envelope.denominator).ok_or(
            Error::ResourceCountOverflow {
                resource: "pre-conversion integer bits",
            },
        )?;
        check_limit(
            "pre-conversion integer bits",
            retained,
            limits.max_preconversion_integer_bits,
        )?;
        *aggregate = aggregate
            .checked_add(retained)
            .ok_or(Error::ResourceCountOverflow {
                resource: "aggregate pre-conversion integer bits",
            })?;
        check_limit(
            "aggregate pre-conversion integer bits",
            *aggregate,
            limits.max_preconversion_integer_bits,
        )?;
    }
    Ok(envelope)
}

fn multiply_numeric_envelopes(
    left: NumericBitEnvelope,
    right: NumericBitEnvelope,
) -> Result<NumericBitEnvelope, Error> {
    Ok(NumericBitEnvelope {
        numerator: left.numerator.checked_add(right.numerator).ok_or(
            Error::ResourceCountOverflow {
                resource: "pre-conversion product numerator bits",
            },
        )?,
        denominator: left.denominator.checked_add(right.denominator).ok_or(
            Error::ResourceCountOverflow {
                resource: "pre-conversion product denominator bits",
            },
        )?,
    })
}

fn add_numeric_envelopes(
    left: NumericBitEnvelope,
    right: NumericBitEnvelope,
) -> Result<NumericBitEnvelope, Error> {
    let left_cross =
        left.numerator
            .checked_add(right.denominator)
            .ok_or(Error::ResourceCountOverflow {
                resource: "pre-conversion sum numerator bits",
            })?;
    let right_cross =
        right
            .numerator
            .checked_add(left.denominator)
            .ok_or(Error::ResourceCountOverflow {
                resource: "pre-conversion sum numerator bits",
            })?;
    Ok(NumericBitEnvelope {
        numerator: left_cross.max(right_cross).checked_add(1).ok_or(
            Error::ResourceCountOverflow {
                resource: "pre-conversion sum numerator bits",
            },
        )?,
        denominator: left.denominator.checked_add(right.denominator).ok_or(
            Error::ResourceCountOverflow {
                resource: "pre-conversion sum denominator bits",
            },
        )?,
    })
}
