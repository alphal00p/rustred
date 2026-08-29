//! Checked Symbolica coefficient field boundary.

mod power;
mod state;
mod traits;
mod unwind;

pub(super) use state::{CheckedCoefficientField, CheckedFieldState};
pub(super) use unwind::{call_native, call_native_result};

#[cfg(test)]
pub(super) use unwind::abort_checked_field;
