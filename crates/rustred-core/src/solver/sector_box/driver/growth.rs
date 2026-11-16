//! Growth between seeding rounds of [`BoxLaportaSession::extend`], and the
//! master check that drives it.
//!
//! With Kira boxes ([`Seeding::KiraBox`]) the seeded boxes are final: no
//! sector grows and `extend` stops after one round. Demand and staircase
//! seeding grow from the masters found (Kira 3's tuning recipe,
//! arXiv:2505.20197 §3.1.2); the master check is the analogue of Kira 3
//! `check_masters` (arXiv:2505.20197 §3.6).

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::SolverError;
use super::super::router::integral_bits;
use super::super::seeds::box_coordinates;
use super::super::{ColumnOrder, MasterCheckMode, MasterCheckReport, SectorReport, Seeding};
use super::BoxLaportaSession;

impl<const N: usize> BoxLaportaSession<N> {
    /// Sectors (Kira ids) to grow after a round with `masters` and the
    /// sector `reports`; empty when the seeding has converged.
    pub(super) fn grow_round(
        &mut self,
        masters: &[[i16; N]],
        reports: &[SectorReport],
    ) -> Result<Vec<u64>, SolverError> {
        match self.options.seeding {
            Seeding::KiraBox { .. } => Ok(Vec::new()),
            Seeding::Demand { max_growth, .. } => {
                let check = self.master_check(masters);
                self.grow(masters, reports, &check, max_growth)
            }
            Seeding::Staircase { max_growth, .. } => {
                let check = self.master_check(masters);
                self.grow_staircase(masters, reports, &check, max_growth)
            }
        }
    }

    /// Kira 3 `check_masters` analogue (arXiv:2505.20197 §3.6): masters
    /// against the preferred set and the expected per-class counts.
    pub(super) fn master_check(&self, masters: &[[i16; N]]) -> MasterCheckReport<N> {
        let master_set: BTreeSet<[i16; N]> = masters.iter().copied().collect();
        let preferred: BTreeSet<[i16; N]> = self.preferred.iter().copied().collect();
        let extra = if preferred.is_empty() {
            Vec::new()
        } else {
            masters
                .iter()
                .filter(|master| !preferred.contains(*master))
                .copied()
                .collect()
        };
        let mut unneeded: Vec<[i16; N]> = preferred
            .iter()
            .filter(|values| !master_set.contains(*values))
            .copied()
            .collect();
        unneeded.extend(self.preferred_zero.iter().copied());
        let mut found: BTreeMap<u64, usize> = BTreeMap::new();
        for master in masters {
            *found.entry(integral_bits(master)).or_default() += 1;
        }
        let expected: BTreeMap<u64, usize> = self
            .options
            .expected_counts
            .as_ref()
            .map(|counts| {
                counts
                    .iter()
                    .filter(|(bits, _)| self.visited.contains(bits) || found.contains_key(bits))
                    .map(|(&bits, &count)| (bits, count))
                    .collect()
            })
            .unwrap_or_default();
        let classes: BTreeSet<u64> = found.keys().chain(expected.keys()).copied().collect();
        let mut report = MasterCheckReport {
            extra,
            unneeded,
            ..Default::default()
        };
        for bits in classes {
            let count = found.get(&bits).copied().unwrap_or(0);
            let wanted = expected.get(&bits).copied();
            report.per_class.push((bits as u32, count, wanted));
            match wanted {
                Some(wanted) if count < wanted => report.missing.push((bits as u32, wanted, count)),
                Some(wanted) if count > wanted => report.excess.push((bits as u32, wanted, count)),
                _ => {}
            }
        }
        report
    }

    /// Demand seeding: grow sectors with boundary masters, more masters
    /// than their LP or expected count, or (master check `Grow`/`Strict`)
    /// masters outside the preferred set. Returns the grown sectors.
    fn grow(
        &mut self,
        masters: &[[i16; N]],
        reports: &[SectorReport],
        check: &MasterCheckReport<N>,
        max_growth: u8,
    ) -> Result<Vec<u64>, SolverError> {
        let mut wanted: BTreeMap<u64, (bool, bool)> = BTreeMap::new();
        for master in masters {
            let bits = integral_bits(master);
            let (lines, dots, numerators) = box_coordinates(master);
            let r = lines as i16 + dots;
            let Some(seed_box) = self.boxes.get(&bits).and_then(|region| region.hull()) else {
                wanted.entry(bits).or_default().0 = true;
                continue;
            };
            let on_r = r >= seed_box.r || dots >= seed_box.max_dots(lines);
            let on_s = numerators > 0 && numerators >= seed_box.s;
            if on_r || on_s {
                let entry = wanted.entry(bits).or_default();
                entry.0 |= on_r;
                entry.1 |= on_s;
            }
        }
        for report in reports {
            if let Some(lp) = report.lp
                && report.masters > lp
            {
                wanted.entry(report.id as u64).or_default().0 = true;
            }
        }
        for &(class, _, _) in &check.excess {
            wanted.entry(u64::from(class)).or_default().0 = true;
        }
        if self.options.master_check != MasterCheckMode::Report {
            for master in &check.extra {
                wanted.entry(integral_bits(master)).or_default().0 = true;
            }
        }
        let mut grown = Vec::new();
        for (bits, (r, s)) in wanted {
            let entry = self.growth.entry(bits).or_insert((0, 0));
            if entry.0.max(entry.1) >= max_growth {
                continue;
            }
            if r {
                entry.0 += 1;
            }
            if s {
                entry.1 += 1;
            }
            grown.push(bits);
        }
        Ok(grown)
    }

    /// Staircase growth, Kira 3's tuning recipe (arXiv:2505.20197 §3.1.2),
    /// applied to the highest line level with any request:
    /// - a class with more masters than expected (expected count, else LP
    ///   count with the LP gate, else masters outside a preferred set under
    ///   [`MasterCheckMode::Grow`]) raises its `s`; when at least half of the
    ///   classes of that level (and at least two) need it and truncation is
    ///   on, `l` is lowered instead, far enough that the cap of that level
    ///   rises by one;
    /// - a master with numerators (numerator-free order `LinesRankDots`) or
    ///   on the dot boundary raises its class's dots.
    ///
    /// With `settle_on_expected`, nothing grows once every class has
    /// exactly its expected count. Returns the grown sectors (the whole
    /// level when `l` was lowered).
    fn grow_staircase(
        &mut self,
        masters: &[[i16; N]],
        reports: &[SectorReport],
        check: &MasterCheckReport<N>,
        max_growth: u8,
    ) -> Result<Vec<u64>, SolverError> {
        if self.options.settle_on_expected
            && let Some(expected) = &self.options.expected_counts
        {
            let settled = check.missing.is_empty()
                && check.excess.is_empty()
                && reports
                    .iter()
                    .all(|report| match expected.get(&u64::from(report.id)) {
                        Some(&count) => report.masters == count,
                        None => report.masters == 0,
                    });
            if settled {
                return Ok(Vec::new());
            }
        }
        let numerator_free = matches!(self.options.order, ColumnOrder::LinesRankDots);
        // (dots, s) requests per class.
        let mut wanted: BTreeMap<u64, (bool, bool)> = BTreeMap::new();
        for master in masters {
            let bits = integral_bits(master);
            let (lines, dots, numerators) = box_coordinates(master);
            let on_dots = match self.boxes.get(&bits) {
                Some(region) => !region.contains(lines, dots + 1, numerators),
                None => true,
            };
            if on_dots || (numerator_free && numerators > 0) {
                wanted.entry(bits).or_default().0 = true;
            }
        }
        let expected = self.options.expected_counts.clone();
        for report in reports {
            let bits = u64::from(report.id);
            let limit = expected
                .as_ref()
                .and_then(|counts| counts.get(&bits).copied())
                .or(report.lp);
            if limit.is_some_and(|limit| report.masters > limit) {
                wanted.entry(bits).or_default().1 = true;
            }
        }
        if self.options.master_check != MasterCheckMode::Report {
            for master in &check.extra {
                wanted.entry(integral_bits(master)).or_default().1 = true;
            }
        }
        // Drop classes that cannot grow any more, then keep the top level.
        wanted.retain(|bits, _| self.growth.get(bits).map_or(0, |&(d, s)| d.max(s)) < max_growth);
        let Some(level) = wanted.keys().map(|bits| bits.count_ones()).max() else {
            return Ok(Vec::new());
        };
        wanted.retain(|bits, _| bits.count_ones() == level);
        let raise_s = wanted.values().filter(|(_, s)| *s).count();
        let classes_at_level = self
            .visited
            .iter()
            .filter(|bits| bits.count_ones() == level)
            .count();
        let mut grown = BTreeSet::new();
        if let Some(l) = self.truncate
            && raise_s >= 2
            && 2 * raise_s >= classes_at_level
            && self.truncate_steps < max_growth
        {
            // Lower `l` far enough that the cap of this level rises by
            // one: below `max(1, t - l + 1) = 1` a step of one may not
            // change the level at all.
            let cap = 1.max(level as i16 - l + 1);
            self.truncate = Some((l - 1).min(level as i16 - cap));
            self.truncate_steps += 1;
            for entry in wanted.values_mut() {
                entry.1 = false;
            }
            grown.extend(
                self.visited
                    .iter()
                    .copied()
                    .filter(|bits| bits.count_ones() <= level),
            );
        }
        for (bits, (dots, s)) in wanted {
            if !dots && !s {
                continue;
            }
            let entry = self.growth.entry(bits).or_insert((0, 0));
            if dots {
                entry.0 += 1;
            }
            if s {
                entry.1 += 1;
            }
            grown.insert(bits);
        }
        Ok(grown.into_iter().collect())
    }
}
