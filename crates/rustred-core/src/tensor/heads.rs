//! Authentication of caller-supplied Symbolica tensor heads.

use symbolica::atom::{Symbol, SymbolAttribute, UserData};

use super::error::{TensorHeadError, TensorHeadKind, TensorHeadViolation};

/// Four caller-owned Symbolica heads used by the bounded tensor grammar.
///
/// Plain heads are accepted for every role.  `metric` may instead carry
/// exactly Symbolica's `Symmetric` attribute. `dot` may carry `Symmetric` or
/// the Vakint-compatible `Symmetric + Linear` set; RustRed supplies symmetric
/// semantics even when the caller chose a plain head.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TensorHeads {
    loop_vector: Symbol,
    external_vector: Symbol,
    metric: Symbol,
    dot: Symbol,
}

impl TensorHeads {
    pub fn try_new(
        loop_vector: Symbol,
        external_vector: Symbol,
        metric: Symbol,
        dot: Symbol,
    ) -> Result<Self, TensorHeadError> {
        let entries = [
            (TensorHeadKind::LoopVector, loop_vector),
            (TensorHeadKind::ExternalVector, external_vector),
            (TensorHeadKind::Metric, metric),
            (TensorHeadKind::Dot, dot),
        ];
        for (kind, symbol) in entries {
            validate_symbol(kind, symbol)?;
        }
        for right in 1..entries.len() {
            for left in 0..right {
                if entries[left].1 == entries[right].1 {
                    return Err(TensorHeadError::Duplicate {
                        first: entries[left].0,
                        second: entries[right].0,
                    });
                }
            }
        }
        Ok(Self {
            loop_vector,
            external_vector,
            metric,
            dot,
        })
    }

    pub const fn loop_vector(&self) -> Symbol {
        self.loop_vector
    }

    pub const fn external_vector(&self) -> Symbol {
        self.external_vector
    }

    pub const fn metric(&self) -> Symbol {
        self.metric
    }

    pub const fn dot(&self) -> Symbol {
        self.dot
    }

    pub(crate) fn kind_for_symbol(&self, symbol: Symbol) -> Option<TensorHeadKind> {
        if symbol == self.loop_vector {
            Some(TensorHeadKind::LoopVector)
        } else if symbol == self.external_vector {
            Some(TensorHeadKind::ExternalVector)
        } else if symbol == self.metric {
            Some(TensorHeadKind::Metric)
        } else if symbol == self.dot {
            Some(TensorHeadKind::Dot)
        } else {
            None
        }
    }
}

fn validate_symbol(kind: TensorHeadKind, symbol: Symbol) -> Result<(), TensorHeadError> {
    let violation = if symbol.get_wildcard_level() != 0 {
        Some(TensorHeadViolation::Wildcard)
    } else if symbol.is_builtin() {
        Some(TensorHeadViolation::BuiltIn)
    } else if !symbol.is_exportable() {
        Some(TensorHeadViolation::CustomBehavior)
    } else if !symbol.get_aliases().is_empty() {
        Some(TensorHeadViolation::Aliases)
    } else if !symbol.get_tags().is_empty() {
        Some(TensorHeadViolation::Tags)
    } else if !matches!(symbol.get_data(), UserData::None) {
        Some(TensorHeadViolation::UserData)
    } else {
        let attributes = symbol.get_attributes();
        let permitted_symmetric = matches!(kind, TensorHeadKind::Metric | TensorHeadKind::Dot)
            && attributes.as_slice() == [SymbolAttribute::Symmetric];
        let permitted_linear_dot = kind == TensorHeadKind::Dot
            && attributes.as_slice() == [SymbolAttribute::Symmetric, SymbolAttribute::Linear];
        if attributes.is_empty() || permitted_symmetric || permitted_linear_dot {
            None
        } else {
            Some(TensorHeadViolation::Attributes)
        }
    };
    match violation {
        Some(violation) => Err(TensorHeadError::Invalid { kind, violation }),
        None => Ok(()),
    }
}
