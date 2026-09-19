//! Exact first-branch diagnostic payload, never a coverage certificate.

use std::fmt::{self, Write};

use crate::diagnostic::BoundedText;

use super::EntryDegreeBound;
use super::{Atom, CoefficientPolynomial, LatticeBox};

const MAX_DISPLAY_BYTES: usize = 16_384;

/// Constructed only when the existing coverage traversal fails. The exact
/// typed payload is bounded by that traversal's admitted predicate/geometry
/// limits; rendering has an additional small byte limit. Unknown atom truth
/// values remain `None`, never silently become false or concrete samples.
#[derive(Clone, PartialEq, Eq)]
pub(in crate::foundry::artifact) struct PredicateCoverWitness {
    sector: Box<[bool]>,
    lower: Box<[u64]>,
    upper: Box<[Option<u64>]>,
    atoms: Box<[WitnessAtom]>,
    required_degree: Option<EntryDegreeBound>,
}

#[derive(Clone, PartialEq, Eq)]
struct WitnessAtom {
    assignment: Option<bool>,
    indices: Box<[usize]>,
    equation: CoefficientPolynomial,
}

impl PredicateCoverWitness {
    pub(super) fn capture(
        sector: &[bool],
        first_box: &LatticeBox,
        atoms: &[Atom<'_>],
        assignments: &[Option<bool>],
    ) -> Self {
        debug_assert_eq!(atoms.len(), assignments.len());
        Self {
            sector: sector.into(),
            lower: first_box.lower().into(),
            upper: first_box.upper().into(),
            atoms: atoms
                .iter()
                .zip(assignments)
                .map(|(atom, &assignment)| WitnessAtom {
                    assignment,
                    indices: atom.indices.into(),
                    equation: atom.equation.clone(),
                })
                .collect(),
            required_degree: None,
        }
    }

    pub(super) fn with_required_degree(mut self, degree: Option<EntryDegreeBound>) -> Self {
        self.required_degree = degree;
        self
    }

    fn render(&self, output: &mut BoundedText) -> fmt::Result {
        output.write_str("abstract Boolean witness (not a concrete uncovered integral):\n")?;
        write!(
            output,
            "sector={:?}; local coordinates: x=n-1 if active, x=-n otherwise\nfirst_local_box: lower={:?}, upper={:?} (None means mathematical infinity)\n",
            self.sector, self.lower, self.upper
        )?;
        if let Some(degree) = self.required_degree {
            writeln!(
                output,
                "required_degree={degree:?}; only the intersection with this sum-bound is requested"
            )?;
        }
        for (ordinal, atom) in self.atoms.iter().enumerate() {
            let assignment = match atom.assignment {
                Some(true) => "equal_zero",
                Some(false) => "not_equal_zero",
                None => "unassigned",
            };
            writeln!(
                output,
                "atom {ordinal}: {assignment}; index_variables={:?}; equation={}",
                atom.indices, atom.equation
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for PredicateCoverWitness {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut text = BoundedText::new(MAX_DISPLAY_BYTES);
        let rendered = self.render(&mut text);
        if rendered.is_err() && !text.truncated {
            return Err(fmt::Error);
        }
        output.write_str(&text.text)?;
        if text.truncated {
            output.write_str("\n[predicate-cover diagnostic truncated at 16384 bytes; typed witness retains exact data]")?;
        }
        Ok(())
    }
}

// Debug is also bounded: source-port reports are commonly pretty-printed.
impl fmt::Debug for PredicateCoverWitness {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, output)
    }
}

#[cfg(test)]
mod tests;
