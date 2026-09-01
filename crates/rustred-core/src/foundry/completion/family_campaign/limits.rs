/// Resource envelope for one complete physical-contraction coverage plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FamilyCoverageLimits {
    /// Raw physical contraction masks visited before symmetry quotienting.
    pub(crate) max_physical_contractions: usize,
    /// Canonical sector representatives retained after quotienting.
    pub(crate) max_sector_orbits: usize,
}

impl Default for FamilyCoverageLimits {
    fn default() -> Self {
        Self {
            // This admits the anticipated fifteen physical propagators of a
            // six-loop cubic parent with substantial headroom. Auxiliary ISP
            // coordinates do not contribute to this exponential count.
            max_physical_contractions: 1_048_576,
            max_sector_orbits: 1_048_576,
        }
    }
}
