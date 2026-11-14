//! Demand and staircase seeding and numerator tubes: the wanted regions of
//! one line level (the non-Kira arms of `desired_regions`) and the tube
//! seeds of rank-heavy demand.
//!
//! - [`Seeding::Demand`]: a sector holding demand gets the `(r, s, dots)`
//!   hull of that demand plus the extra; subsectors inherit the `(s, dots)`
//!   hull of their parents' boxes (and, with top-down insertion, of what the
//!   U-closure still needs).
//! - [`Seeding::Staircase`]: the union of one box per demanded integral
//!   ("several blocks are united", arXiv:1705.05610 §5); subsectors inherit
//!   the union of their parents' boxes with the numerator rank capped by
//!   Kira 3 Eq. 9 (arXiv:2505.20197 §3.1).
//! - [`super::super::TubeOptions`]: tubes `u * y`, `u | N(target)`,
//!   `deg y <= halo`, for targets of high rank (staircase only).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::super::super::SolverError;
use super::super::Seeding;
use super::super::router::integral_bits;
use super::super::seeds::{SeedBox, SeedRegion, box_coordinates, tube_seeds};
use super::super::select::closure;
use super::{BoxLaportaSession, powers, subsets};

impl<const N: usize> BoxLaportaSession<N> {
    /// [`Seeding::Demand`] regions of one level. `boxes` are the regions
    /// seeded so far (higher levels of this round included); `use_closure`
    /// adds what the U-closure of the demand still needs (top-down
    /// insertion).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn demand_regions(
        &mut self,
        level: &[u64],
        use_closure: bool,
        boxes: &BTreeMap<u64, SeedRegion>,
        demand: &BTreeSet<[i16; N]>,
        truncate: Option<i16>,
        extra_r: u8,
        extra_s: u8,
    ) -> Result<Vec<(u64, SeedRegion)>, SolverError> {
        let parents = self.parents(level, boxes);
        let lines_here = level[0].count_ones();
        let wanted: BTreeSet<u64> = level.iter().copied().collect();
        // `(r, s, dots)` hulls: of the demand (gets the extra), and of
        // what supersector boxes and the U-closure pass down (does
        // not, so boxes do not grow level by level). A subsector
        // inherits its parents' dot cap, not their `r`: rows of a
        // parent box reach the subsector with the same dots.
        type Hull = BTreeMap<u64, (i16, i16, i16)>;
        let mut demanded: Hull = BTreeMap::new();
        let mut inherited: Hull = BTreeMap::new();
        let hull = |map: &mut Hull, bits: u64, r: i16, s: i16, dots: i16| {
            let entry = map.entry(bits).or_insert((r, s, dots));
            entry.0 = entry.0.max(r);
            entry.1 = entry.1.max(s);
            entry.2 = entry.2.max(dots);
        };
        for values in demand {
            let bits = integral_bits(values);
            if wanted.contains(&bits) {
                let (lines, dots, numerators) = box_coordinates(values);
                hull(&mut demanded, bits, lines as i16 + dots, numerators, dots);
            }
        }
        for (&bits, sector_parents) in &parents {
            for parent in sector_parents {
                let Some(parent_box) = boxes.get(parent).and_then(SeedRegion::hull) else {
                    continue;
                };
                let parent_dots = parent_box.max_dots(parent.count_ones() as usize);
                hull(
                    &mut inherited,
                    bits,
                    lines_here as i16 + parent_dots,
                    parent_box.s,
                    parent_dots,
                );
            }
        }
        if use_closure && self.modular.rank() > 0 {
            let demand = self.demand_columns()?;
            let discovery = &self.modular.discovery;
            let reached = closure(discovery, &demand);
            for &column in &reached.candidates {
                let values = powers(&discovery.columns()[column]);
                let bits = integral_bits(&values);
                if wanted.contains(&bits) {
                    let (lines, dots, numerators) = box_coordinates(&values);
                    hull(&mut inherited, bits, lines as i16 + dots, numerators, dots);
                }
            }
        }
        let mut regions = Vec::new();
        for &bits in level {
            let lines = bits.count_ones() as i16;
            let from_demand = demanded.get(&bits).map(|&(r, s, dots)| {
                let r = r.max(lines) + i16::from(extra_r);
                (r, s + i16::from(extra_s), dots.max(0) + i16::from(extra_r))
            });
            let explicit = from_demand.map_or(0, |(_, s, _)| s);
            let from_parents = inherited.get(&bits).map(|&(r, s, dots)| {
                let s = Self::truncation_cap(truncate, lines, explicit).map_or(s, |cap| s.min(cap));
                (r, s, dots)
            });
            let (r, s, dots) = match (from_demand, from_parents) {
                (Some(a), Some(b)) => (a.0.max(b.0), a.1.max(b.1), a.2.max(b.2)),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => continue,
            };
            let (grow_r, grow_s) = self.growth.get(&bits).copied().unwrap_or((0, 0));
            let dots = dots.max(0) + i16::from(grow_r);
            regions.push((
                bits,
                SeedRegion::from_box(SeedBox {
                    r: r.max(lines + dots),
                    s: s + i16::from(grow_s),
                    d: dots,
                }),
            ));
        }
        Ok(regions)
    }

    /// [`Seeding::Staircase`] regions of one level; `delta` enlarges the
    /// explicit (demand) boxes (the truncated stability check).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn staircase_regions(
        &self,
        level: &[u64],
        boxes: &BTreeMap<u64, SeedRegion>,
        demand: &BTreeSet<[i16; N]>,
        truncate: Option<i16>,
        delta: (u8, u8, u8),
        s_pad: u8,
        d_pad: u8,
    ) -> Result<Vec<(u64, SeedRegion)>, SolverError> {
        let (delta_s, delta_d) = (i16::from(delta.1), i16::from(delta.2));
        let parents = self.parents(level, boxes);
        let wanted: BTreeSet<u64> = level.iter().copied().collect();
        // Own boxes: one per demanded integral, `(R_i + s_pad,
        // dots_i + d_pad)`, united (not hulled).
        let mut own: BTreeMap<u64, SeedRegion> = BTreeMap::new();
        for values in demand {
            let bits = integral_bits(values);
            if !wanted.contains(&bits) {
                continue;
            }
            let (lines, dots, numerators) = box_coordinates(values);
            let dots = dots.max(0) + i16::from(d_pad) + delta_d;
            // A tube target keeps a low box of its own; the tube
            // covers its numerator direction.
            let numerators = match self.options.tubes {
                Some(tubes) => numerators.min(i16::from(tubes.min_rank) - 1),
                None => numerators,
            };
            own.entry(bits).or_default().insert(SeedBox {
                r: lines as i16 + dots,
                s: numerators + i16::from(s_pad) + delta_s,
                d: dots,
            });
        }
        let mut regions = Vec::new();
        for &bits in level {
            let lines = bits.count_ones() as i16;
            let mut region = own.remove(&bits).unwrap_or_default();
            let explicit = region.max_s().max(0);
            let cap = Self::truncation_cap(truncate, lines, explicit);
            // Inherited boxes keep the parent's dot cap; their
            // numerator rank is truncated (Kira 3 Eq. 9).
            for parent in parents.get(&bits).into_iter().flatten() {
                let parent_lines = parent.count_ones() as usize;
                let mut parent_boxes = boxes
                    .get(parent)
                    .map_or_else(Vec::new, |region| region.boxes().to_vec());
                parent_boxes.extend(self.tube_pass_down(*parent, demand, delta));
                for parent_box in &parent_boxes {
                    let dots = parent_box.max_dots(parent_lines);
                    if dots < 0 {
                        continue;
                    }
                    region.insert(SeedBox {
                        r: lines + dots,
                        s: cap.map_or(parent_box.s, |cap| parent_box.s.min(cap)),
                        d: dots,
                    });
                }
            }
            if region.is_empty() {
                continue;
            }
            let (grow_d, grow_s) = self.growth.get(&bits).copied().unwrap_or((0, 0));
            if grow_d > 0 || grow_s > 0 {
                region = SeedRegion::from_boxes(region.boxes().iter().map(|seed_box| SeedBox {
                    r: seed_box.r + i16::from(grow_d),
                    s: seed_box.s + i16::from(grow_s),
                    d: seed_box.d + i16::from(grow_d),
                }));
            }
            regions.push((bits, region));
        }
        Ok(regions)
    }

    /// For every sector of one level, the seeded sectors with more lines
    /// that pass their regions down to it: those containing it, and those
    /// containing a routed sector of its class with the same lines.
    fn parents(&self, level: &[u64], boxes: &BTreeMap<u64, SeedRegion>) -> BTreeMap<u64, Vec<u64>> {
        let Some(&first) = level.first() else {
            return BTreeMap::new();
        };
        let lines = first.count_ones();
        let wanted: BTreeSet<u64> = level.iter().copied().collect();
        let mut parents: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for &parent in boxes.keys() {
            if parent.count_ones() <= lines {
                continue;
            }
            let mut reached = BTreeSet::new();
            for bits in subsets(parent) {
                if bits.count_ones() != lines {
                    continue;
                }
                if let Some(representative) = self.factory.representative_bits(bits)
                    && wanted.contains(&representative)
                {
                    reached.insert(representative);
                }
            }
            for representative in reached {
                parents.entry(representative).or_default().push(parent);
            }
        }
        parents
    }

    /// Demanded integrals of sector `bits` that get a numerator tube.
    fn tube_targets<'a>(&self, bits: u64, demand: &'a BTreeSet<[i16; N]>) -> Vec<&'a [i16; N]> {
        let Some(tubes) = self.options.tubes else {
            return Vec::new();
        };
        demand
            .iter()
            .filter(|values| {
                integral_bits(values) == bits
                    && box_coordinates(*values).2 >= i16::from(tubes.min_rank)
            })
            .collect()
    }

    /// Boxes the tube targets of `parent` pass down to its subsectors, as
    /// if their full boxes `(R + s_pad, dots + d_pad)` (plus the parent's
    /// growth and `delta`) were seeded; Eq. 9 caps them on inheritance.
    pub(super) fn tube_pass_down(
        &self,
        parent: u64,
        demand: &BTreeSet<[i16; N]>,
        delta: (u8, u8, u8),
    ) -> Vec<SeedBox> {
        let Seeding::Staircase { s_pad, d_pad, .. } = self.options.seeding else {
            return Vec::new();
        };
        let (grow_d, grow_s) = self.growth.get(&parent).copied().unwrap_or((0, 0));
        self.tube_targets(parent, demand)
            .into_iter()
            .map(|values| {
                let (lines, dots, numerators) = box_coordinates(values);
                let dots = dots.max(0) + i16::from(d_pad) + i16::from(delta.2) + i16::from(grow_d);
                SeedBox {
                    r: lines as i16 + dots,
                    s: numerators + i16::from(s_pad) + i16::from(delta.1) + i16::from(grow_s),
                    d: dots,
                }
            })
            .collect()
    }

    /// Tube seeds wanted in sector `bits`: the union of the tubes of its
    /// tube targets, with the halo widened by the class's `s` growth and
    /// `delta.s`, and the dots by its dot growth and `delta.d`.
    fn desired_tubes(
        &self,
        bits: u64,
        demand: &BTreeSet<[i16; N]>,
        delta: (u8, u8, u8),
    ) -> BTreeSet<[i16; N]> {
        let (Some(tubes), Seeding::Staircase { d_pad, .. }) =
            (self.options.tubes, self.options.seeding)
        else {
            return BTreeSet::new();
        };
        let (grow_d, grow_s) = self.growth.get(&bits).copied().unwrap_or((0, 0));
        let halo = i16::from(tubes.halo) + i16::from(delta.1) + i16::from(grow_s);
        let mut wanted = BTreeSet::new();
        for values in self.tube_targets(bits, demand) {
            let dots = box_coordinates(values).1.max(0)
                + i16::from(d_pad)
                + i16::from(delta.2)
                + i16::from(grow_d);
            wanted.extend(tube_seeds::<N>(bits as u32, self.arity, values, halo, dots));
        }
        wanted
    }

    /// New tube seeds of sector `bits` against its seeded region and the
    /// tube seeds already generated (`known`).
    pub(super) fn fresh_tube_seeds(
        &self,
        bits: u64,
        region: Option<&SeedRegion>,
        known: Option<&BTreeSet<[i16; N]>>,
        demand: &BTreeSet<[i16; N]>,
        delta: (u8, u8, u8),
    ) -> Result<Vec<[i16; N]>, SolverError> {
        let mut wanted = self.desired_tubes(bits, demand, delta);
        wanted.extend(self.desired_subtubes(bits, demand, delta)?.iter().copied());
        Ok(wanted
            .into_iter()
            .filter(|seed| {
                let (lines, dots, numerators) = box_coordinates(seed);
                !region.is_some_and(|region| region.contains(lines, dots, numerators))
                    && !known.is_some_and(|known| known.contains(seed))
            })
            .collect())
    }

    /// Sub-tubes of sector `bits` ([`super::TubeOptions::subsector_halo`]):
    /// for every tube target `n` of a supersector `S`, each member sector
    /// `M` of `bits`'s class strictly below `S` gets the tube of `n` over
    /// its own ISPs (the pinched lines of `S` included), `u * y` with
    /// `u | N(n)` and `deg y <= subsector_halo`, with at most
    /// `dots(n) + d_pad` dots; the seeds are routed onto `bits` and the
    /// terms in `bits` itself kept. These are the subsector integrals the
    /// rows of the target's tube produce, so the inherited box can stay
    /// truncated (Eq. 9) while the numerator direction is still seeded.
    fn desired_subtubes(
        &self,
        bits: u64,
        demand: &BTreeSet<[i16; N]>,
        delta: (u8, u8, u8),
    ) -> Result<Arc<BTreeSet<[i16; N]>>, SolverError> {
        use rayon::prelude::*;
        let (Some(tubes), Seeding::Staircase { d_pad, .. }) =
            (self.options.tubes, self.options.seeding)
        else {
            return Ok(Arc::default());
        };
        let Some(subsector_halo) = tubes.subsector_halo else {
            return Ok(Arc::default());
        };
        let targets: Vec<&[i16; N]> = demand
            .iter()
            .filter(|values| {
                let top = integral_bits(values);
                top != bits
                    && box_coordinates(*values).2 >= i16::from(tubes.min_rank)
                    && top.count_ones() > bits.count_ones()
            })
            .collect();
        if targets.is_empty() {
            return Ok(Arc::default());
        }
        let (grow_d, grow_s) = self.growth.get(&bits).copied().unwrap_or((0, 0));
        let halo = i16::from(subsector_halo) + i16::from(delta.1) + i16::from(grow_s);
        let extra_dots = i16::from(d_pad) + i16::from(delta.2) + i16::from(grow_d);
        let mut hasher = blake3::Hasher::new();
        for target in &targets {
            for value in *target {
                hasher.update(&value.to_le_bytes());
            }
        }
        let key = (bits, halo, extra_dots, *hasher.finalize().as_bytes());
        if let Some(cached) = self.subtubes.lock().expect("sub-tube memo").get(&key) {
            return Ok(cached.clone());
        }
        let mut wanted = BTreeSet::new();
        let mut members: Vec<[i16; N]> = Vec::new();
        for target in targets {
            let top = integral_bits(target);
            let dots = box_coordinates(target).1.max(0) + extra_dots;
            for member in subsets(top) {
                if member == 0
                    || member == top
                    || self.factory.representative_bits(member) != Some(bits)
                {
                    continue;
                }
                let seeds = tube_seeds::<N>(member as u32, self.arity, target, halo, dots);
                if member == bits {
                    wanted.extend(seeds);
                } else {
                    members.extend(seeds);
                }
            }
        }
        members.sort_unstable();
        members.dedup();
        let factory = &self.factory;
        let routed: Vec<Result<Vec<[i16; N]>, SolverError>> = self.pool.install(|| {
            members
                .par_iter()
                .map(|seed| factory.route_keys(seed))
                .collect()
        });
        for keys in routed {
            wanted.extend(keys?.into_iter().filter(|key| integral_bits(key) == bits));
        }
        let wanted = Arc::new(wanted);
        self.subtubes
            .lock()
            .expect("sub-tube memo")
            .insert(key, wanted.clone());
        Ok(wanted)
    }
}
