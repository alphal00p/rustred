//! Zero-allocation lexical admission before Symbolica builds a Token tree.

use super::super::error::Error;
use super::super::limits::{Limits, check_limit, checked_add};

pub(super) fn preflight_raw_source(source: &str, limits: Limits) -> Result<(), Error> {
    let mut units = 0usize;
    let mut depth = 0usize;
    let mut maximum_depth = 0usize;
    let mut prefix_operator_depth = 0usize;
    let mut maximum_prefix_operator_depth = 0usize;
    let mut expecting_operand = true;
    let mut has_add_layer = false;
    let mut has_mul_layer = false;
    let mut has_power_layer = false;
    let mut integer_digits = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let mut lexical_units = 0usize;
    let mut lexical_run = RawLexicalRun::None;
    for character in source.chars() {
        units = checked_add("raw parser units", units, 1)?;
        check_limit("raw parser units", units, limits.max_raw_parser_units)?;

        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            integer_digits = 0;
            continue;
        }
        if character == '"' {
            if lexical_run == RawLexicalRun::Numeric {
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
            } else if lexical_run == RawLexicalRun::None {
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
            }
            lexical_run = RawLexicalRun::Identifier;
            quoted = true;
            integer_digits = 0;
            continue;
        }

        // Symbolica removes these separators while it is scanning one numeric
        // literal. They must therefore preserve both the numeric run and its
        // cumulative digit count in the pre-Token census.
        if lexical_run == RawLexicalRun::Numeric && matches!(character, '_' | '\u{2009}') {
            continue;
        }

        // In Symbolica mode a backslash is parser whitespace even though Rust
        // does not classify it as Unicode whitespace. Splitting the lexical
        // run here charges the implicit multiplication that the next operand
        // may create.
        if character == '\\' || character.is_whitespace() {
            lexical_run = RawLexicalRun::None;
            integer_digits = 0;
            continue;
        }

        if character.is_ascii_digit() {
            if lexical_run == RawLexicalRun::None {
                if !expecting_operand {
                    has_mul_layer = true;
                }
                charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
                lexical_run = RawLexicalRun::Numeric;
            }
            if lexical_run == RawLexicalRun::Numeric {
                integer_digits = checked_add("raw integer digits", integer_digits, 1)?;
                check_limit(
                    "raw integer digits",
                    integer_digits,
                    limits.max_raw_integer_digits,
                )?;
            } else {
                integer_digits = 0;
            }
            expecting_operand = false;
            prefix_operator_depth = 0;
            continue;
        }

        integer_digits = 0;
        if matches!(
            character,
            '+' | '-' | '*' | '/' | '^' | '(' | ')' | '[' | ']' | ','
        ) {
            let run_before_operator = lexical_run;
            lexical_run = RawLexicalRun::None;
            charge_raw_lexical_units(&mut lexical_units, 1, limits)?;
            match character {
                '(' | '[' => {
                    if !expecting_operand && run_before_operator != RawLexicalRun::Identifier {
                        has_mul_layer = true;
                    }
                    depth = checked_add("raw parser nesting depth", depth, 1)?;
                    maximum_depth = maximum_depth.max(depth);
                    check_limit(
                        "raw parser nesting depth",
                        checked_add("raw parser nesting depth", depth, prefix_operator_depth)?,
                        limits.max_nesting_depth,
                    )?;
                    expecting_operand = true;
                }
                ')' | ']' => {
                    depth = depth.saturating_sub(1);
                    expecting_operand = false;
                    prefix_operator_depth = 0;
                }
                ',' => {
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                '-' | '/' => {
                    if !expecting_operand {
                        if character == '-' {
                            has_add_layer = true;
                        } else {
                            has_mul_layer = true;
                        }
                    }
                    prefix_operator_depth = if expecting_operand {
                        checked_add("raw parser nesting depth", prefix_operator_depth, 1)?
                    } else {
                        1
                    };
                    maximum_prefix_operator_depth =
                        maximum_prefix_operator_depth.max(prefix_operator_depth);
                    check_limit(
                        "raw parser nesting depth",
                        checked_add("raw parser nesting depth", depth, prefix_operator_depth)?,
                        limits.max_nesting_depth,
                    )?;
                    expecting_operand = true;
                }
                '+' => {
                    if !expecting_operand {
                        has_add_layer = true;
                        prefix_operator_depth = 0;
                    }
                    expecting_operand = true;
                }
                '*' => {
                    if !expecting_operand {
                        has_mul_layer = true;
                    }
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                '^' => {
                    if !expecting_operand {
                        has_power_layer = true;
                    }
                    expecting_operand = true;
                    prefix_operator_depth = 0;
                }
                _ => unreachable!(),
            }
            continue;
        }

        if !expecting_operand && lexical_run != RawLexicalRun::Identifier {
            has_mul_layer = true;
        }
        if lexical_run == RawLexicalRun::Numeric {
            // Symbolica may insert an implicit multiplication between a
            // numeric literal and a following identifier.
            charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
        } else if lexical_run == RawLexicalRun::None {
            charge_raw_lexical_units(&mut lexical_units, 2, limits)?;
        }
        lexical_run = RawLexicalRun::Identifier;
        if expecting_operand {
            // Any pending prefix chain has reached its operand. Its maximum
            // depth was charged while the chain was being constructed.
            expecting_operand = false;
            prefix_operator_depth = 0;
        } else {
            integer_digits = 0;
        }
    }
    let binary_layers = usize::from(has_add_layer)
        .checked_add(usize::from(has_mul_layer))
        .and_then(|layers| layers.checked_add(usize::from(has_power_layer)))
        .ok_or(Error::ResourceCountOverflow {
            resource: "raw parser nesting depth",
        })?;
    let conservative_depth = checked_add(
        "raw parser nesting depth",
        checked_add(
            "raw parser nesting depth",
            maximum_depth,
            maximum_prefix_operator_depth,
        )?,
        binary_layers,
    )?;
    check_limit(
        "raw parser nesting depth",
        conservative_depth,
        limits.max_nesting_depth,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawLexicalRun {
    None,
    Numeric,
    Identifier,
}

pub(super) fn charge_raw_lexical_units(
    total: &mut usize,
    amount: usize,
    limits: Limits,
) -> Result<(), Error> {
    *total = checked_add("raw lexical tokens", *total, amount)?;
    check_limit("raw lexical tokens", *total, limits.max_atom_nodes)
}
