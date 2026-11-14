//! The +1 stability check of `extend` (Kira's `(r, s, d)` enlargement):
//! once the masters have converged, the seeded regions are enlarged by
//! [`super::super::BoxOptions::stability`] and the master set must stay the
//! same.

use std::collections::BTreeMap;

use super::super::super::SolverError;
use super::super::seeds::{SeedRegion, region_shell_count, region_shell_seeds};
use super::super::{LimitKind, Seeding};
use super::{BoxLaportaSession, limit};

impl<const N: usize> BoxLaportaSession<N> {
    /// Whether the stability check reruns the seeding plan with enlarged
    /// explicit boxes (Kira 3 truncation stays in force, arXiv:2505.20197
    /// §3.1; tubes widen by the same delta) instead of enlarging every
    /// seeded region.
    fn truncated_stability(&self) -> bool {
        self.options.tubes.is_some()
            || self.truncate.is_some()
                && matches!(
                    self.options.seeding,
                    Seeding::KiraBox { .. } | Seeding::Staircase { .. }
                )
    }

    /// The stability re-check: enlarge every seeded region by `delta`.
    /// Under truncation (box and staircase seeding) the explicit boxes grow
    /// by `delta` and subsectors inherit them through the same Eq. 9 cap,
    /// as in a Kira rerun with larger `(r, s, d)` and the same `l`.
    pub(super) fn enlarge(&mut self, delta: (u8, u8, u8)) -> Result<(), SolverError> {
        if self.truncated_stability() {
            // The rows cap is checked before every batch is generated.
            return self.seed_round("stability", delta);
        }
        let mut by_level: BTreeMap<std::cmp::Reverse<u32>, Vec<(u64, SeedRegion)>> =
            BTreeMap::new();
        for (&bits, region) in &self.boxes {
            by_level
                .entry(std::cmp::Reverse(bits.count_ones()))
                .or_default()
                .push((bits, region.enlarged(delta)));
        }
        let estimate: usize = by_level
            .values()
            .flatten()
            .map(|(bits, new)| {
                region_shell_count(*bits as u32, self.arity, new, self.boxes.get(bits))
            })
            .sum::<usize>()
            * self.factory.source_count();
        limit(
            LimitKind::Rows,
            self.stats.rows + estimate,
            self.options.limits.max_rows,
        )?;
        let top_down = self.options.insertion == super::super::Insertion::TopDownLevels;
        let mut pending = Vec::new();
        for (level, regions) in by_level {
            let mut seeds = Vec::new();
            for (bits, new) in regions {
                let old = self.boxes.get(&bits).cloned();
                for seed in region_shell_seeds::<N>(bits as u32, self.arity, &new, old.as_ref()) {
                    let id = self.push_seed(seed, false)?;
                    seeds.push((id, seed));
                }
                self.boxes.insert(bits, new);
            }
            if top_down {
                if !seeds.is_empty() {
                    self.insert_level(&format!("stability/level{}", level.0), &seeds)?;
                }
            } else {
                pending.extend(seeds);
            }
        }
        if !pending.is_empty() {
            self.insert_level("stability/bottom-up", &pending)?;
        }
        self.check_seeded()
    }
}
