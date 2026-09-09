use super::DirectShiftedSourceError;

const SHIFT_COORDINATES: &str = "shifted structural coordinates";
const RESIDUES: &str = "modular source residues";

/// One borrowed term from a complete modular source image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ShiftedModularTerm<'row> {
    structural_shift: &'row [i64],
    residue: u64,
}

impl<'row> ShiftedModularTerm<'row> {
    pub(crate) const fn structural_shift(self) -> &'row [i64] {
        self.structural_shift
    }

    pub(crate) const fn residue(self) -> u64 {
        self.residue
    }
}

/// Allocation-amortized owner of one complete shifted modular source row.
///
/// Coordinates are term-major. Every exact source term is retained, including
/// terms whose residue is zero at the current sample. The caller owns the only
/// full structural row allocation used by direct evaluation.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShiftedModularSourceBuffer {
    arity: usize,
    shifted_coordinates: Vec<i64>,
    residues: Vec<u64>,
}

impl ShiftedModularSourceBuffer {
    pub(crate) const fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) const fn len(&self) -> usize {
        self.residues.len()
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.residues.is_empty()
    }

    pub(crate) fn terms(&self) -> ShiftedModularTerms<'_> {
        ShiftedModularTerms {
            row: self,
            ordinal: 0,
        }
    }

    pub(super) fn clear(&mut self) {
        self.arity = 0;
        self.shifted_coordinates.clear();
        self.residues.clear();
    }

    pub(super) fn residues_mut(&mut self) -> &mut Vec<u64> {
        &mut self.residues
    }

    pub(super) fn try_prepare_residues(
        &mut self,
        term_count: usize,
    ) -> Result<(), DirectShiftedSourceError> {
        self.clear();
        try_reserve_total(&mut self.residues, term_count, RESIDUES)
    }

    pub(super) fn try_prepare_coordinates(
        &mut self,
        arity: usize,
        coordinate_count: usize,
    ) -> Result<(), DirectShiftedSourceError> {
        self.shifted_coordinates.clear();
        try_reserve_total(
            &mut self.shifted_coordinates,
            coordinate_count,
            SHIFT_COORDINATES,
        )?;
        self.arity = arity;
        Ok(())
    }

    pub(super) fn push_coordinate(&mut self, coordinate: i64) {
        self.shifted_coordinates.push(coordinate);
    }

    pub(super) fn coordinate_count(&self) -> usize {
        self.shifted_coordinates.len()
    }

    #[cfg(test)]
    pub(super) fn capacities_for_test(&self) -> (usize, usize) {
        (
            self.shifted_coordinates.capacity(),
            self.residues.capacity(),
        )
    }
}

/// Probe-local coefficient image for a structurally prepared source row.
///
/// Structural shifts and their exact roles live in the shared row plan. This
/// buffer therefore retains only one residue per exact source term, including
/// modular zeros, and can be reused by a probe for every prepared row.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShiftedModularResidueBuffer {
    residues: Vec<u64>,
}

impl ShiftedModularResidueBuffer {
    pub(crate) const fn len(&self) -> usize {
        self.residues.len()
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.residues.is_empty()
    }

    pub(crate) fn residues(&self) -> &[u64] {
        &self.residues
    }

    pub(super) fn clear(&mut self) {
        self.residues.clear();
    }

    pub(super) fn try_prepare(
        &mut self,
        term_count: usize,
    ) -> Result<(), DirectShiftedSourceError> {
        self.clear();
        try_reserve_total(&mut self.residues, term_count, RESIDUES)
    }

    pub(super) fn residues_mut(&mut self) -> &mut Vec<u64> {
        &mut self.residues
    }

    #[cfg(test)]
    pub(super) fn capacity_for_test(&self) -> usize {
        self.residues.capacity()
    }
}

/// Exact-size iterator over every structural term, including modular zeros.
#[derive(Clone, Debug)]
pub(crate) struct ShiftedModularTerms<'row> {
    row: &'row ShiftedModularSourceBuffer,
    ordinal: usize,
}

impl<'row> Iterator for ShiftedModularTerms<'row> {
    type Item = ShiftedModularTerm<'row>;

    fn next(&mut self) -> Option<Self::Item> {
        let residue = *self.row.residues.get(self.ordinal)?;
        let start = self.ordinal.checked_mul(self.row.arity)?;
        let end = start.checked_add(self.row.arity)?;
        let structural_shift = self.row.shifted_coordinates.get(start..end)?;
        self.ordinal += 1;
        Some(ShiftedModularTerm {
            structural_shift,
            residue,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.row.residues.len().saturating_sub(self.ordinal);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ShiftedModularTerms<'_> {}

fn try_reserve_total<T>(
    values: &mut Vec<T>,
    requested: usize,
    resource: &'static str,
) -> Result<(), DirectShiftedSourceError> {
    let additional = requested.saturating_sub(values.len());
    values
        .try_reserve_exact(additional)
        .map_err(|_| DirectShiftedSourceError::AllocationFailure {
            resource,
            requested,
        })
}
