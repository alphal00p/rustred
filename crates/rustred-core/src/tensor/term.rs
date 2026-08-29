//! Private bounded-grammar state for one projected monomial.

use symbolica::atom::Atom;

use crate::family::ScalarProductCoordinate;

#[derive(Clone, Debug)]
pub(super) enum InternalSlot {
    Free {
        loop_index: usize,
        index: Atom,
    },
    ExternalContracted {
        loop_index: usize,
        external_index: usize,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum MomentumRef {
    Loop(usize),
    External(usize),
}

pub(super) struct TermParts {
    pub(super) scalar_factors: Vec<Atom>,
    pub(super) outside_factors: Vec<Atom>,
    pub(super) scalar_products: Vec<ScalarProductCoordinate>,
    pub(super) internal_slots: Vec<InternalSlot>,
    pub(super) retained_tensor_indices: Vec<Atom>,
}

impl TermParts {
    pub(super) fn new() -> Self {
        Self {
            scalar_factors: Vec::new(),
            outside_factors: Vec::new(),
            scalar_products: Vec::new(),
            internal_slots: Vec::new(),
            retained_tensor_indices: Vec::new(),
        }
    }
}
