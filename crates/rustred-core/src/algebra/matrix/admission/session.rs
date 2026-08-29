//! Session-level operation counters.

use std::cell::RefCell;
use std::rc::Rc;

use crate::algebra::matrix::field::CheckedFieldState;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixStats};

pub(in crate::algebra::matrix) fn increment_session_counter(
    state: &Rc<RefCell<CheckedFieldState>>,
    resource: &'static str,
    select: impl FnOnce(&mut SymbolicaCoefficientMatrixStats) -> &mut usize,
) -> Result<(), SymbolicaCoefficientMatrixError> {
    let mut state = state.borrow_mut();
    let counter = select(&mut state.stats);
    *counter = counter
        .checked_add(1)
        .ok_or(SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource })?;
    Ok(())
}
