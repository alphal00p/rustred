//! Box seeds: all integrals of one sector inside an `(r, s, dots)` box, or
//! inside a union of such boxes.
//!
//! For an integral `n` of a sector with `t` lines, `r = sum_{n_i>0} n_i`,
//! `s = sum_{n_i<0} |n_i|` and `dots = r - t`. A [`SeedBox`] contains the
//! integrals with `r <= r`, `s <= s` and `dots <= d` (Kira's `(r, s, d)`).
//! A [`SeedRegion`] is a union of boxes of one sector ("several blocks are
//! united", arXiv:1705.05610 §5): a staircase in the `(s, dots)` plane.
//! Growth only enumerates the shell between an old and a new region, so a
//! session never instantiates the same seed twice.

/// A Kira-style `(r, s, d)` box. `d` caps the dots; `r - t` caps them too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SeedBox {
    pub r: i16,
    pub s: i16,
    pub d: i16,
}

impl SeedBox {
    /// Effective dot cap in a sector with `lines` lines; negative when the
    /// box contains no integral of that sector.
    pub fn max_dots(&self, lines: usize) -> i16 {
        self.d.min(self.r - lines as i16)
    }

    pub fn contains(&self, lines: usize, dots: i16, numerators: i16) -> bool {
        dots >= 0 && dots <= self.max_dots(lines) && numerators >= 0 && numerators <= self.s
    }

    /// Component-wise maximum.
    pub fn hull(&self, other: &Self) -> Self {
        Self {
            r: self.r.max(other.r),
            s: self.s.max(other.s),
            d: self.d.max(other.d),
        }
    }

    pub fn enlarged(&self, (r, s, d): (u8, u8, u8)) -> Self {
        Self {
            r: self.r + i16::from(r),
            s: self.s + i16::from(s),
            d: self.d + i16::from(d),
        }
    }
}

/// A union of [`SeedBox`]es of one sector: a staircase (down-set) in the
/// `(numerators, dots)` plane.
///
/// Boxes are kept sorted and without componentwise-dominated members, so
/// the representation is canonical for a given set of boxes. A single box
/// region behaves exactly like the box.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SeedRegion {
    boxes: Vec<SeedBox>,
}

impl SeedRegion {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_box(seed_box: SeedBox) -> Self {
        Self {
            boxes: vec![seed_box],
        }
    }

    /// Union of the given boxes.
    pub fn from_boxes(boxes: impl IntoIterator<Item = SeedBox>) -> Self {
        let mut region = Self::new();
        for seed_box in boxes {
            region.insert(seed_box);
        }
        region
    }

    /// The maximal boxes, sorted.
    pub fn boxes(&self) -> &[SeedBox] {
        &self.boxes
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Add one box; boxes it dominates componentwise are dropped.
    pub fn insert(&mut self, seed_box: SeedBox) {
        if self.boxes.iter().any(|old| dominates(old, &seed_box)) {
            return;
        }
        self.boxes.retain(|old| !dominates(&seed_box, old));
        self.boxes.push(seed_box);
        self.boxes.sort_unstable();
    }

    pub fn union(&self, other: &Self) -> Self {
        let mut region = self.clone();
        for &seed_box in &other.boxes {
            region.insert(seed_box);
        }
        region
    }

    pub fn contains(&self, lines: usize, dots: i16, numerators: i16) -> bool {
        self.boxes
            .iter()
            .any(|seed_box| seed_box.contains(lines, dots, numerators))
    }

    /// Largest dot count of the region in a sector with `lines` lines
    /// (negative when empty).
    pub fn max_dots(&self, lines: usize) -> i16 {
        self.boxes
            .iter()
            .map(|seed_box| seed_box.max_dots(lines))
            .max()
            .unwrap_or(-1)
    }

    /// Largest numerator rank of the region (negative when empty).
    pub fn max_s(&self) -> i16 {
        self.boxes
            .iter()
            .map(|seed_box| seed_box.s)
            .max()
            .unwrap_or(-1)
    }

    /// The componentwise hull (one box containing the region).
    pub fn hull(&self) -> Option<SeedBox> {
        let mut boxes = self.boxes.iter();
        let first = *boxes.next()?;
        Some(boxes.fold(first, |hull, seed_box| hull.hull(seed_box)))
    }

    /// Every box enlarged by `delta`.
    pub fn enlarged(&self, delta: (u8, u8, u8)) -> Self {
        Self::from_boxes(self.boxes.iter().map(|seed_box| seed_box.enlarged(delta)))
    }

    /// Every box with its numerator rank capped at `cap`.
    pub fn capped(&self, cap: i16) -> Self {
        Self::from_boxes(self.boxes.iter().map(|seed_box| SeedBox {
            s: seed_box.s.min(cap),
            ..*seed_box
        }))
    }
}

fn dominates(outer: &SeedBox, inner: &SeedBox) -> bool {
    outer.r >= inner.r && outer.s >= inner.s && outer.d >= inner.d
}

/// `(lines, dots, numerators)` of a numeric integral.
pub(crate) fn box_coordinates(powers: &[i16]) -> (usize, i16, i16) {
    let mut lines = 0;
    let mut r = 0i16;
    let mut s = 0i16;
    for &value in powers {
        if value > 0 {
            lines += 1;
            r += value;
        } else {
            s -= value;
        }
    }
    (lines, r - lines as i16, s)
}

fn binomial(n: usize, k: usize) -> usize {
    let k = k.min(n.saturating_sub(k));
    let mut result = 1usize;
    for i in 0..k {
        result = result.saturating_mul(n - i) / (i + 1);
    }
    result
}

/// Number of weak compositions of `total` into `parts` parts.
fn compositions(total: usize, parts: usize) -> usize {
    if parts == 0 {
        return usize::from(total == 0);
    }
    binomial(total + parts - 1, parts - 1)
}

/// Seeds of `sector` (bit `i` = slot `i` active) in `new` but not in `old`,
/// over `arity` physical slots. No seed is produced for an empty sector.
#[cfg(test)]
pub(crate) fn shell_count(sector: u32, arity: usize, new: SeedBox, old: Option<SeedBox>) -> usize {
    region_shell_count(
        sector,
        arity,
        &SeedRegion::from_box(new),
        old.map(SeedRegion::from_box).as_ref(),
    )
}

/// [`shell_count`] for regions.
pub(crate) fn region_shell_count(
    sector: u32,
    arity: usize,
    new: &SeedRegion,
    old: Option<&SeedRegion>,
) -> usize {
    let lines = sector.count_ones() as usize;
    if lines == 0 {
        return 0;
    }
    let inactive = arity - lines;
    let mut count = 0usize;
    for dots in 0..=new.max_dots(lines).max(-1) {
        for numerators in 0..=new.max_s().max(-1) {
            if !new.contains(lines, dots, numerators)
                || old.is_some_and(|old| old.contains(lines, dots, numerators))
            {
                continue;
            }
            count = count.saturating_add(
                compositions(dots as usize, lines)
                    .saturating_mul(compositions(numerators as usize, inactive)),
            );
        }
    }
    count
}

/// Enumerate the shell deterministically: by `(dots, numerators)`, then by
/// the lexicographic order of the dot and numerator distributions.
pub(crate) fn shell_seeds<const N: usize>(
    sector: u32,
    arity: usize,
    new: SeedBox,
    old: Option<SeedBox>,
) -> Vec<[i16; N]> {
    region_shell_seeds(
        sector,
        arity,
        &SeedRegion::from_box(new),
        old.map(SeedRegion::from_box).as_ref(),
    )
}

/// [`shell_seeds`] for regions (same enumeration order).
pub(crate) fn region_shell_seeds<const N: usize>(
    sector: u32,
    arity: usize,
    new: &SeedRegion,
    old: Option<&SeedRegion>,
) -> Vec<[i16; N]> {
    let lines = sector.count_ones() as usize;
    let mut seeds = Vec::with_capacity(region_shell_count(sector, arity, new, old));
    if lines == 0 {
        return seeds;
    }
    let active: Vec<usize> = (0..arity).filter(|&slot| sector >> slot & 1 == 1).collect();
    let inactive: Vec<usize> = (0..arity).filter(|&slot| sector >> slot & 1 == 0).collect();
    let mut dot_parts = vec![0i16; active.len()];
    let mut numerator_parts = vec![0i16; inactive.len()];
    for dots in 0..=new.max_dots(lines).max(-1) {
        for numerators in 0..=new.max_s().max(-1) {
            if !new.contains(lines, dots, numerators)
                || old.is_some_and(|old| old.contains(lines, dots, numerators))
            {
                continue;
            }
            for_each_composition(dots, &mut dot_parts, 0, &mut |dot_parts| {
                for_each_composition(
                    numerators,
                    &mut numerator_parts,
                    0,
                    &mut |numerator_parts| {
                        let mut seed = [0i16; N];
                        for (&slot, &extra) in active.iter().zip(dot_parts.iter()) {
                            seed[slot] = 1 + extra;
                        }
                        for (&slot, &power) in inactive.iter().zip(numerator_parts.iter()) {
                            seed[slot] = -power;
                        }
                        seeds.push(seed);
                    },
                );
            });
        }
    }
    seeds
}

/// Numerator tube of a demanded integral of `sector` (bit `i` = slot `i`
/// active): every integral of the sector with at most `max_dots` dots (any
/// distribution over the lines) whose numerator monomial is `u * y`, where
/// `u` divides the numerator of `target` and `y` is a monomial of degree at
/// most `halo` in the sector's inactive slots. Sorted by
/// `(dots, numerators)` and then lexicographically, without duplicates.
pub fn tube_seeds<const N: usize>(
    sector: u32,
    arity: usize,
    target: &[i16; N],
    halo: i16,
    max_dots: i16,
) -> Vec<[i16; N]> {
    use std::collections::BTreeSet;
    let lines = sector.count_ones() as usize;
    if lines == 0 || halo < 0 || max_dots < 0 {
        return Vec::new();
    }
    let active: Vec<usize> = (0..arity).filter(|&slot| sector >> slot & 1 == 1).collect();
    let inactive: Vec<usize> = (0..arity).filter(|&slot| sector >> slot & 1 == 0).collect();
    let exponents: Vec<i16> = inactive
        .iter()
        .map(|&slot| (-target[slot]).max(0))
        .collect();
    // Divisors u of N(target), as exponent vectors over the inactive slots.
    let mut divisors = vec![vec![0i16; inactive.len()]];
    for (position, &exponent) in exponents.iter().enumerate() {
        let mut next = Vec::with_capacity(divisors.len() * (exponent as usize + 1));
        for divisor in &divisors {
            for power in 0..=exponent {
                let mut extended = divisor.clone();
                extended[position] = power;
                next.push(extended);
            }
        }
        divisors = next;
    }
    // Halo monomials y of degree <= halo.
    let mut halos = Vec::new();
    let mut parts = vec![0i16; inactive.len()];
    for degree in 0..=halo {
        for_each_composition(degree, &mut parts, 0, &mut |parts| {
            halos.push(parts.to_vec())
        });
    }
    let mut numerators: BTreeSet<Vec<i16>> = BTreeSet::new();
    for divisor in &divisors {
        for y in &halos {
            numerators.insert(divisor.iter().zip(y).map(|(a, b)| a + b).collect());
        }
    }
    let mut seeds = Vec::new();
    let mut dot_parts = vec![0i16; active.len()];
    for dots in 0..=max_dots {
        let mut level: Vec<[i16; N]> = Vec::new();
        for_each_composition(dots, &mut dot_parts, 0, &mut |dot_parts| {
            for numerator in &numerators {
                let mut seed = [0i16; N];
                for (&slot, &extra) in active.iter().zip(dot_parts.iter()) {
                    seed[slot] = 1 + extra;
                }
                for (&slot, &power) in inactive.iter().zip(numerator.iter()) {
                    seed[slot] = -power;
                }
                level.push(seed);
            }
        });
        level.sort_unstable_by_key(|seed| (box_coordinates(seed).2, *seed));
        seeds.extend(level);
    }
    seeds
}

/// Visit every weak composition of `total` into `parts.len()` parts, with
/// the first part largest first (lexicographically decreasing).
fn for_each_composition(
    total: i16,
    parts: &mut Vec<i16>,
    position: usize,
    visit: &mut dyn FnMut(&[i16]),
) {
    if parts.is_empty() {
        if total == 0 {
            visit(parts);
        }
        return;
    }
    if position + 1 == parts.len() {
        parts[position] = total;
        visit(parts);
        return;
    }
    for value in (0..=total).rev() {
        parts[position] = value;
        for_each_composition(total - value, parts, position + 1, visit);
    }
    parts[position] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn shells_partition_the_box_and_match_the_count() {
        let sector = 0b0111u32;
        let arity = 5;
        let small = SeedBox { r: 4, s: 1, d: 1 };
        let large = SeedBox { r: 6, s: 2, d: 3 };
        let inner = shell_seeds::<5>(sector, arity, small, None);
        let outer = shell_seeds::<5>(sector, arity, large, Some(small));
        let all = shell_seeds::<5>(sector, arity, large, None);
        assert_eq!(inner.len(), shell_count(sector, arity, small, None));
        assert_eq!(outer.len(), shell_count(sector, arity, large, Some(small)));
        assert_eq!(all.len(), inner.len() + outer.len());
        let union: BTreeSet<_> = inner.iter().chain(&outer).copied().collect();
        assert_eq!(union, all.iter().copied().collect());
        for seed in &all {
            let (lines, dots, numerators) = box_coordinates(seed);
            assert_eq!(lines, 3);
            assert!(large.contains(lines, dots, numerators));
            assert!(seed[..3].iter().all(|&v| v > 0) && seed[3..].iter().all(|&v| v <= 0));
        }
        // 3 lines, dots <= min(3, 6-3) = 3, s <= 2 over 2 inactive slots:
        // C(dots+2,2) for dots 0..=3 -> 1+3+6+10 = 20; numerators 1+2+3 = 6.
        assert_eq!(all.len(), 20 * 6);
    }

    #[test]
    fn regions_are_unions_not_hulls() {
        let sector = 0b0111u32;
        let arity = 6;
        // Two targets: rank 2 without dots, and rank 0 with two dots.
        let numerators = SeedBox { r: 3, s: 2, d: 0 };
        let dots = SeedBox { r: 5, s: 0, d: 2 };
        let region = SeedRegion::from_boxes([numerators, dots]);
        assert_eq!(region.boxes().len(), 2);
        assert!(region.contains(3, 0, 2));
        assert!(region.contains(3, 2, 0));
        assert!(!region.contains(3, 1, 1));
        let hull = region.hull().unwrap();
        assert_eq!(hull, SeedBox { r: 5, s: 2, d: 2 });
        let union = region_shell_seeds::<6>(sector, arity, &region, None);
        let a = shell_seeds::<6>(sector, arity, numerators, None);
        let b = shell_seeds::<6>(sector, arity, dots, None);
        let expected: BTreeSet<_> = a.iter().chain(&b).copied().collect();
        assert_eq!(union.iter().copied().collect::<BTreeSet<_>>(), expected);
        assert_eq!(
            union.len(),
            region_shell_count(sector, arity, &region, None)
        );
        assert!(union.len() < shell_count(sector, arity, hull, None));
        // A dominated box adds nothing; a single-box region is the box.
        let mut grown = region.clone();
        grown.insert(SeedBox { r: 3, s: 1, d: 0 });
        assert_eq!(grown, region);
        assert_eq!(
            region_shell_seeds::<6>(sector, arity, &SeedRegion::from_box(hull), None),
            shell_seeds::<6>(sector, arity, hull, None)
        );
        // Shells of regions partition the larger region.
        let larger = region.enlarged((0, 1, 0));
        let outer = region_shell_seeds::<6>(sector, arity, &larger, Some(&region));
        let all = region_shell_seeds::<6>(sector, arity, &larger, None);
        assert_eq!(all.len(), union.len() + outer.len());
        assert_eq!(region.capped(1).max_s(), 1);
    }

    #[test]
    fn tubes_are_divisor_times_halo_monomials() {
        // Sector {0, 1} of arity 5; target numerator x2^2 x3 (rank 3).
        let target = [1, 1, -2, -1, 0];
        let tube = tube_seeds::<5>(0b11, 5, &target, 0, 0);
        // halo 0: the 3 * 2 divisors of x2^2 x3, no dots.
        assert_eq!(tube.len(), 6);
        assert!(tube.contains(&target) && tube.contains(&[1, 1, 0, 0, 0]));
        // halo 1 adds x_j * u for every ISP j; dots <= 1 doubles over the
        // two-line dot distributions (1 + 2).
        let wide = tube_seeds::<5>(0b11, 5, &target, 1, 1);
        let numerators: BTreeSet<[i16; 3]> = wide
            .iter()
            .map(|seed| [seed[2], seed[3], seed[4]])
            .collect();
        // u * y over u | x2^2 x3, deg y <= 1: exponents (a, b, c) with
        // a <= 3, b <= 2, c <= 1, at most one exponent above its bound.
        let expected: BTreeSet<[i16; 3]> = (0..=3)
            .flat_map(|a| (0..=2).flat_map(move |b| (0..=1).map(move |c| [a, b, c])))
            .filter(|&[a, b, c]| usize::from(a > 2) + usize::from(b > 1) + usize::from(c > 0) <= 1)
            .map(|[a, b, c]| [-a, -b, -c])
            .collect();
        assert_eq!(numerators, expected);
        assert_eq!(wide.len(), expected.len() * 3);
        let (lines, dots, _) = box_coordinates(&wide[0]);
        assert_eq!((lines, dots), (2, 0));
        assert!(wide.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn coordinates_follow_kira_conventions() {
        assert_eq!(box_coordinates(&[2, 1, 1, 0, -2]), (3, 1, 2));
        assert_eq!(SeedBox { r: 10, s: 1, d: 3 }.max_dots(8), 2);
        assert_eq!(SeedBox { r: 10, s: 1, d: 3 }.max_dots(5), 3);
    }
}
