//! The final modular selection of a session: the input of the materialising
//! lanes (finite-field and exact), which re-instantiate the selected rows
//! over the family parameters from their provenance (later steps of the
//! series).

use super::super::Integral;

/// The final modular selection of a session, kept for materialisation.
#[derive(Clone, Debug)]
// Read by the materialising lanes (a later step of the series).
#[allow(dead_code)]
pub(crate) struct SelectionState<const N: usize> {
    /// `(seed powers, source)` of the selected rows, in pivot-column order.
    pub rows: Vec<([i16; N], u16)>,
    /// Modular pivot of each selected row.
    pub pivots: Vec<Integral<N>>,
    /// Masters in column order (hardest first).
    pub masters: Vec<Integral<N>>,
    /// Demanded representative-sector integrals.
    pub demand: Vec<Integral<N>>,
    pub selected_hash: String,
    pub master_hash: String,
}
