//! Growth between seeding rounds of [`BoxLaportaSession::extend`].
//!
//! With Kira boxes ([`Seeding::KiraBox`]) the seeded boxes are final: no
//! sector grows and `extend` stops after one round. Demand and staircase
//! seeding, whose boxes grow from the masters found (Kira 3's tuning recipe,
//! arXiv:2505.20197 §3.1.2), replace this step in a later step of the series.

use super::super::super::SolverError;
use super::super::{SectorReport, Seeding};
use super::BoxLaportaSession;

impl<const N: usize> BoxLaportaSession<N> {
    /// Sectors (Kira ids) to grow after a round with `masters` and the
    /// sector `reports`; empty when the seeding has converged.
    pub(super) fn grow_round(
        &mut self,
        _masters: &[[i16; N]],
        _reports: &[SectorReport],
    ) -> Result<Vec<u64>, SolverError> {
        match self.options.seeding {
            Seeding::KiraBox { .. } => Ok(Vec::new()),
        }
    }
}
