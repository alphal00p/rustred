//! Orbit canonicalisation of Apply keys under the automorphisms of their owner
//! sector (M5 R0 A2, the p-path port of the vacuum ordered reducer's
//! `canonicalize`).
//!
//! An automorphism of an owner sector `(presentation, support)` is an integer
//! momentum map `k -> A k + b q`, `q -> s q` (`|det A| = 1`, `s = +-1`) that
//! permutes the support lines of the sector, eps units included. Candidates
//! are enumerated exactly (the images of `L + 1` independent support lines fix
//! the map) and are then authenticated one by one by
//! [`crate::sector::symmetry::verify()`], which derives the exact denominator
//! action; nothing here is taken on trust from the enumeration. A verified map
//! with a unit Jacobian acts on a key EXACTLY, as ONE key times `+-1`, when the
//! verified action sends every slot with a nonzero power of the key onto one
//! slot, `D_i = s_i D_j` with `s_i = +-1` and the same eps units:
//! `I(a) = prod_i s_i^{a_i} I(b)`, `b_j = a_i`. The Gamma tag of the key is a
//! factor of the key and is not touched.
//!
//! The set of maps acting on a key is closed under composition and inverse
//! (a composite of slot-onto-slot actions is one, and so is an inverse), so
//! "every exact single-key image" is the same set for every member of an
//! orbit, and the minimum of that set is a canonical representative. The
//! minimum is taken in the owner's own order (the order key of its lane, then
//! the powers): the representative is never above the raw key in the order,
//! so a same-owner child that lies strictly below its parent stays strictly
//! below it after canonicalisation and the Ordered engine's descent check
//! holds unchanged. Unlike [`crate::sector::symmetry::Canonicalizer`], which
//! needs a group of full, sign-free slot permutations, this action is partial
//! (an automorphism may send an ISP to a combination of slots; it then acts
//! only on keys without a power on that ISP) and carries signs.
use std::collections::BTreeSet;

use crate::sector::symmetry::{self, CoefficientMatrix, DenominatorAction, Jacobian, MomentumMap};

use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::presentation::{Presentation, Slot};

/// The image of one slot under an automorphism: the target slot and whether
/// the denominator changes sign.
pub(crate) type SlotImage = Option<(u16, bool)>;

/// The verified, non-trivial slot actions of the automorphisms of one owner
/// sector (deduplicated; the identity is implicit).
#[derive(Debug, Default)]
pub(crate) struct OwnerOrbit {
    pub(crate) actions: Vec<Box<[SlotImage]>>,
    /// For each action, the slot sent onto each slot (`u16::MAX`: none), so
    /// an image can be compared slot by slot without being built.
    pub(crate) inverses: Vec<Box<[u16]>>,
    /// Candidate maps found by the enumeration, the identity included.
    pub(crate) candidates: usize,
    /// Generators that `verify` authenticated (unit Jacobian, monomial rows
    /// equal to the integer action).
    pub(crate) verified: usize,
    /// Order of the group they generate (the identity and `-1` included).
    pub(crate) group_order: usize,
    /// The group: `n x n` integer matrices (`n = L + 1`, row-vector
    /// convention `v -> v T`), each a product of verified maps.
    pub(crate) group: Vec<Vec<i128>>,
}

impl OwnerOrbit {
    /// Add one slot action and its inverse.
    pub(crate) fn push(&mut self, action: Box<[SlotImage]>) {
        let mut inverse = vec![u16::MAX; action.len()];
        for (slot, image) in action.iter().enumerate() {
            if let Some((target, _)) = image
                && let Some(entry) = inverse.get_mut(usize::from(*target))
            {
                *entry = u16::try_from(slot).unwrap_or(u16::MAX);
            }
        }
        self.inverses.push(inverse.into_boxed_slice());
        self.actions.push(action);
    }

    /// The least exact image of `powers` strictly below it whose support
    /// passes `keep`, with its sign (the first action that reaches it decides
    /// the sign); `None` when the key is its own representative. Each action
    /// is compared slot by slot through its inverse and dropped at the first
    /// slot that decides; only an image below the best so far is built.
    pub(crate) fn least_image(
        &self,
        powers: &[i64],
        high: bool,
        keep: impl Fn(&[i64]) -> bool,
    ) -> Option<(Vec<i64>, bool)> {
        let n = powers.len();
        let mut scratch: smallvec::SmallVec<[i64; 32]> = smallvec::SmallVec::from_elem(0, n);
        let mut best: Option<(smallvec::SmallVec<[i64; 32]>, bool)> = None;
        for (action, inverse) in self.actions.iter().zip(&self.inverses) {
            if action.len() < n || inverse.len() < n {
                continue;
            }
            // Exact on this key: every slot with a power maps onto a slot.
            if powers
                .iter()
                .zip(action.iter())
                .any(|(&power, image)| power != 0 && image.is_none())
            {
                continue;
            }
            let bound = best.as_ref().map_or(powers, |(image, _)| image.as_slice());
            let mut below = false;
            for (target, &bound) in bound.iter().enumerate() {
                let source = inverse[target];
                let value = if source == u16::MAX {
                    0
                } else {
                    powers[usize::from(source)]
                };
                if value != bound {
                    // `high`: the greatest image instead (an experiment on
                    // which orientation the lanes step best).
                    below = if high { value > bound } else { value < bound };
                    break;
                }
            }
            if !below {
                continue;
            }
            let Some(negative) = Self::image_into(action, powers, &mut scratch) else {
                continue;
            };
            if keep(&scratch) {
                best = Some((scratch.clone(), negative));
            }
        }
        best.map(|(image, negative)| (image.into_vec(), negative))
    }

    /// [`Self::image`] written into `out` (resized to `powers.len()`), without
    /// allocating: the sign, or `None` when the action is not exact on the key.
    pub(crate) fn image_into(
        action: &[SlotImage],
        powers: &[i64],
        out: &mut [i64],
    ) -> Option<bool> {
        if action.len() < powers.len() || out.len() != powers.len() {
            return None;
        }
        out.fill(0);
        let mut negative = false;
        for (slot, &power) in powers.iter().enumerate() {
            if power == 0 {
                continue;
            }
            let (target, flip) = action[slot]?;
            out[usize::from(target)] = power;
            if flip && power % 2 != 0 {
                negative = !negative;
            }
        }
        Some(negative)
    }

    /// The image of `powers` under `action`, with its sign, when the action is
    /// exact on this key.
    pub(crate) fn image(action: &[SlotImage], powers: &[i64]) -> Option<(Vec<i64>, bool)> {
        if powers
            .iter()
            .zip(action)
            .any(|(&power, image)| power != 0 && image.is_none())
            || action.len() < powers.len()
        {
            return None;
        }
        let mut out = vec![0_i64; powers.len()];
        let mut negative = false;
        for (slot, &power) in powers.iter().enumerate() {
            if power == 0 {
                continue;
            }
            let (target, flip) = action[slot]?;
            out[usize::from(target)] = power;
            if flip && power % 2 != 0 {
                negative = !negative;
            }
        }
        Some((out, negative))
    }
}

/// The automorphisms of sector `support` of `presentation`, authenticated.
pub(crate) fn owner_orbit(presentation: &Presentation, support: u64) -> Result<OwnerOrbit, PError> {
    let loops = usize::from(presentation.loops);
    let n = loops + 1;
    let lines: Vec<usize> = presentation
        .slots
        .iter()
        .enumerate()
        .filter(|&(slot, s)| matches!(s, Slot::Line(_)) && support >> slot & 1 == 1)
        .map(|(slot, _)| slot)
        .collect();
    let line = |slot: usize| match &presentation.slots[slot] {
        Slot::Line(line) => line,
        Slot::Isp(_) => unreachable!("support lines are lines"),
    };
    let vec_of =
        |slot: usize| -> Vec<i128> { line(slot).momentum.iter().map(|&c| i128::from(c)).collect() };
    let eps = |slot: usize| line(slot).eps_units;
    let mut orbit = OwnerOrbit::default();
    if loops == 0 || lines.is_empty() {
        return Ok(orbit);
    }
    // A basis of L + 1 independent support lines, greedily in slot order.
    let mut basis: Vec<usize> = Vec::new();
    let mut echelon: Vec<Vec<i128>> = Vec::new();
    for &slot in &lines {
        if basis.len() == n {
            break;
        }
        if let Some(row) = reduce(&echelon, vec_of(slot)) {
            echelon.push(row);
            basis.push(slot);
        }
    }
    if basis.len() < n {
        return Ok(orbit);
    }
    let b: Vec<Vec<i128>> = basis.iter().map(|&slot| vec_of(slot)).collect();
    let (det, adj) = determinant_and_adjugate(&b);
    if det == 0 {
        return Ok(orbit);
    }
    // v B^-1 = (v adj) / det; `last` = the last basis position a line depends on.
    let coef: Vec<Vec<i128>> = lines
        .iter()
        .map(|&slot| row_times(&vec_of(slot), &adj))
        .collect();
    let last: Vec<Option<usize>> = coef
        .iter()
        .map(|row| row.iter().rposition(|&c| c != 0))
        .collect();
    let by_vec: std::collections::BTreeMap<Vec<i128>, usize> = lines
        .iter()
        .flat_map(|&slot| {
            let v = vec_of(slot);
            let minus: Vec<i128> = v.iter().map(|c| -c).collect();
            [(v, slot), (minus, slot)]
        })
        .collect();
    let mut maps: BTreeSet<Vec<i128>> = BTreeSet::new();
    let mut images: Vec<Vec<i128>> = Vec::with_capacity(n);
    // An invariant every automorphism keeps: the number of pairs of other
    // support lines a line is a signed sum of (`v_i = +-v_j +-v_k`, one vertex
    // of momentum conservation). Basis images are only tried among lines with
    // the same invariant, which prunes the search without losing a map.
    let mut degree = vec![0_usize; presentation.slots.len()];
    for &i in &lines {
        let vi = vec_of(i);
        for (a, &j) in lines.iter().enumerate() {
            if j == i {
                continue;
            }
            let vj = vec_of(j);
            for &k in &lines[a + 1..] {
                if k == i {
                    continue;
                }
                let vk = vec_of(k);
                let hit = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
                    .iter()
                    .any(|&(sj, sk)| {
                        vi.iter()
                            .zip(&vj)
                            .zip(&vk)
                            .all(|((&x, &y), &z)| x == sj * y + sk * z)
                    });
                if hit {
                    degree[i] += 1;
                }
            }
        }
    }
    let search = Search {
        n,
        lines: &lines,
        basis: &basis,
        coef: &coef,
        last: &last,
        by_vec: &by_vec,
        det,
        adj: &adj,
        eps: &eps,
        degree: &degree,
        vec_of: &vec_of,
    };
    search.extend(0, &mut images, &mut maps);
    orbit.candidates = maps.len();
    // Authenticate a generating set: a candidate already in the group generated
    // by the verified maps so far is a product of verified isomorphisms and
    // needs no proof of its own; any other candidate is verified, and its
    // verified denominator action must equal the integer action computed here.
    let unit: Vec<i128> = (0..n * n)
        .map(|entry| i128::from(entry % (n + 1) == 0))
        .collect();
    let mut group: BTreeSet<Vec<i128>> = BTreeSet::from([unit]);
    for t in &maps {
        if group.contains(t) {
            continue;
        }
        let action = slot_action(presentation, t, n);
        if !authenticated(presentation, t, &action, loops)? {
            continue;
        }
        orbit.verified += 1;
        // Close the group under right multiplication by every verified generator.
        let mut frontier: Vec<Vec<i128>> = group.iter().cloned().collect();
        let mut generators: Vec<Vec<i128>> = Vec::with_capacity(group.len() + 1);
        generators.push(t.clone());
        generators.extend(orbit_generators(&group, n));
        while let Some(element) = frontier.pop() {
            for generator in &generators {
                let Some(product) = multiply(&element, generator, n) else {
                    continue;
                };
                if group.len() >= MAX_GROUP_ORDER {
                    break;
                }
                if group.insert(product.clone()) {
                    frontier.push(product);
                }
            }
        }
    }
    let (candidates, verified) = (orbit.candidates, orbit.verified);
    Ok(orbit_of_group(
        presentation,
        support,
        group.into_iter().collect(),
        candidates,
        verified,
    ))
}

/// The orbit of sector `support` of `presentation` from its group (`n x n`
/// integer matrices, row-vector convention, the identity included), each
/// element a product of maps already authenticated: the slot action of every
/// element that permutes the support lines, deduplicated, the identity left
/// out. Shared by the search ([`owner_orbit`]) and the restore of a saved
/// group ([`super::saved_orbits`]); pure integer algebra.
pub(crate) fn orbit_of_group(
    presentation: &Presentation,
    support: u64,
    group: Vec<Vec<i128>>,
    candidates: usize,
    verified: usize,
) -> OwnerOrbit {
    let n = usize::from(presentation.loops) + 1;
    let lines: Vec<usize> = presentation
        .slots
        .iter()
        .enumerate()
        .filter(|&(slot, s)| matches!(s, Slot::Line(_)) && support >> slot & 1 == 1)
        .map(|(slot, _)| slot)
        .collect();
    let identity: Box<[SlotImage]> = (0..presentation.slots.len())
        .map(|slot| Some((u16::try_from(slot).unwrap_or(u16::MAX), false)))
        .collect();
    let mut orbit = OwnerOrbit {
        candidates,
        verified,
        ..OwnerOrbit::default()
    };
    let mut seen: BTreeSet<Box<[SlotImage]>> = BTreeSet::new();
    for t in &group {
        if t.len() != n * n {
            return OwnerOrbit::default();
        }
        let action = slot_action(presentation, t, n);
        let onto_support = lines
            .iter()
            .all(|&slot| action[slot].is_some_and(|(target, _)| support >> target & 1 == 1));
        if !onto_support || action == identity {
            continue;
        }
        if seen.insert(action.clone()) {
            orbit.push(action);
        }
    }
    orbit.group_order = group.len();
    orbit.group = group;
    orbit
}

/// The automorphisms of a routed sector `support` of `presentation`, carried
/// over from its owner: with the route `v -> v M` onto the owner (verified when
/// the route compiles) and an owner automorphism `T`, `M T M^-1` is an
/// automorphism of the routed sector, a product of verified maps. Its action on
/// the slots of `presentation` is computed exactly as for an owner, and only
/// maps that permute the support lines are kept.
pub(crate) fn routed_orbit(
    presentation: &Presentation,
    support: u64,
    map: &SignedAffineLoopMap,
    owner: &OwnerOrbit,
) -> OwnerOrbit {
    let loops = usize::from(presentation.loops);
    let n = loops + 1;
    let mut orbit = OwnerOrbit::default();
    if usize::from(map.loops_in) != loops
        || usize::from(map.loops_out) != loops
        || map.loop_linear.len() != loops * loops
        || map.loop_external.len() != loops
    {
        return orbit;
    }
    let mut m = vec![vec![0_i128; n]; n];
    for (row, (linear, &external)) in m
        .iter_mut()
        .zip(map.loop_linear.chunks(loops.max(1)).zip(&map.loop_external))
    {
        for (entry, &value) in row.iter_mut().zip(linear) {
            *entry = i128::from(value);
        }
        row[loops] = i128::from(external);
    }
    m[loops][loops] = i128::from(map.external_sign);
    let (det, adj) = determinant_and_adjugate(&m);
    if det.abs() != 1 {
        return orbit;
    }
    let flat = |rows: &[Vec<i128>]| -> Vec<i128> { rows.iter().flatten().copied().collect() };
    let m_flat = flat(&m);
    let inverse: Vec<i128> = flat(&adj).into_iter().map(|x| x * det).collect();
    let lines: Vec<usize> = presentation
        .slots
        .iter()
        .enumerate()
        .filter(|&(slot, s)| matches!(s, Slot::Line(_)) && support >> slot & 1 == 1)
        .map(|(slot, _)| slot)
        .collect();
    let arity = presentation.slots.len();
    let identity: Box<[SlotImage]> = (0..arity)
        .map(|slot| Some((u16::try_from(slot).unwrap_or(u16::MAX), false)))
        .collect();
    let mut seen: BTreeSet<Box<[SlotImage]>> = BTreeSet::new();
    for t in &owner.group {
        if t.len() != n * n {
            return OwnerOrbit::default();
        }
        let Some(conjugate) = multiply(&m_flat, t, n).and_then(|mt| multiply(&mt, &inverse, n))
        else {
            continue;
        };
        let action = slot_action(presentation, &conjugate, n);
        let onto_support = lines
            .iter()
            .all(|&slot| action[slot].is_some_and(|(target, _)| support >> target & 1 == 1));
        if !onto_support || action == identity {
            continue;
        }
        if seen.insert(action.clone()) {
            orbit.push(action);
        }
    }
    orbit.group_order = owner.group_order;
    orbit
}

/// Largest group closed per owner sector; a larger one keeps what it has (each
/// element is still a product of verified maps).
const MAX_GROUP_ORDER: usize = 4096;

/// Every element of `group` is a word in the generators; the closure only needs
/// the group itself as generators of the next step.
fn orbit_generators(group: &BTreeSet<Vec<i128>>, n: usize) -> Vec<Vec<i128>> {
    let unit: Vec<i128> = (0..n * n)
        .map(|entry| i128::from(entry % (n + 1) == 0))
        .collect();
    group.iter().filter(|t| **t != unit).cloned().collect()
}

fn multiply(left: &[i128], right: &[i128], n: usize) -> Option<Vec<i128>> {
    let mut out = vec![0_i128; n * n];
    for row in 0..n {
        for column in 0..n {
            let mut sum = 0_i128;
            for k in 0..n {
                sum = sum.checked_add(left[row * n + k].checked_mul(right[k * n + column])?)?;
            }
            out[row * n + column] = sum;
        }
    }
    Some(out)
}

/// The exact action of `t` (row-vector convention, `v -> v t`) on every slot,
/// computed on the symmetric bilinear form of the slot: `l.r -> (l t).(r t)`.
/// A slot maps onto slot `j` when its image form is `+-` the form of `j`, with
/// the same eps units.
fn slot_action(presentation: &Presentation, t: &[i128], n: usize) -> Box<[SlotImage]> {
    let factors = |slot: &Slot| -> (Vec<i128>, Vec<i128>) {
        let (l, r) = match slot {
            Slot::Line(line) => (&line.momentum, &line.momentum),
            Slot::Isp(isp) => (&isp.left, &isp.right),
        };
        (
            l.iter().map(|&c| i128::from(c)).collect(),
            r.iter().map(|&c| i128::from(c)).collect(),
        )
    };
    let form = |l: &[i128], r: &[i128]| -> Vec<i128> {
        let mut m = vec![0_i128; n * n];
        for i in 0..n {
            for j in 0..n {
                m[i * n + j] = l[i] * r[j] + r[i] * l[j];
            }
        }
        m
    };
    let mapped = |v: &[i128]| -> Vec<i128> {
        (0..n)
            .map(|column| (0..n).map(|row| v[row] * t[row * n + column]).sum())
            .collect()
    };
    let eps = |slot: &Slot| match slot {
        Slot::Line(line) => line.eps_units,
        Slot::Isp(_) => 0,
    };
    let mut lookup: std::collections::BTreeMap<Vec<i128>, (u16, bool)> = Default::default();
    for (index, slot) in presentation.slots.iter().enumerate() {
        let (l, r) = factors(slot);
        let f = form(&l, &r);
        let minus: Vec<i128> = f.iter().map(|c| -c).collect();
        let index = u16::try_from(index).unwrap_or(u16::MAX);
        lookup.entry(f).or_insert((index, false));
        lookup.entry(minus).or_insert((index, true));
    }
    presentation
        .slots
        .iter()
        .map(|slot| {
            let (l, r) = factors(slot);
            let image = form(&mapped(&l), &mapped(&r));
            let (target, flip) = *lookup.get(&image)?;
            (eps(&presentation.slots[usize::from(target)]) == eps(slot)).then_some((target, flip))
        })
        .collect()
}

/// `verify` proves `t` a family isomorphism with a unit Jacobian and its
/// monomial rows equal `action` (every slot `action` maps onto a slot is a
/// verified monomial row with scale `+-1`, and no other).
fn authenticated(
    presentation: &Presentation,
    t: &[i128],
    action: &[SlotImage],
    loops: usize,
) -> Result<bool, PError> {
    let n = loops + 1;
    let family = &presentation.family;
    let context = family.coefficient_context();
    let integer = |value: i128| i64::try_from(value).ok().map(|v| context.integer(v));
    let entry = |row: usize, column: usize| t[row * n + column];
    let a: Option<Vec<_>> = (0..loops)
        .flat_map(|row| (0..loops).map(move |column| (row, column)))
        .map(|(row, column)| integer(entry(row, column)))
        .collect();
    let bq: Option<Vec<_>> = (0..loops).map(|row| integer(entry(row, loops))).collect();
    let (Some(a), Some(bq), Some(s)) = (a, bq, integer(entry(loops, loops))) else {
        return Ok(false);
    };
    let (Ok(a), Ok(bq), Ok(s)) = (
        CoefficientMatrix::try_new(loops, loops, a),
        CoefficientMatrix::try_new(loops, 1, bq),
        CoefficientMatrix::try_new(1, 1, [s]),
    ) else {
        return Ok(false);
    };
    let Ok(verified) = symmetry::verify(
        family,
        family,
        MomentumMap::new(a, bq, s),
        symmetry::Limits::default(),
    ) else {
        return Ok(false);
    };
    if !matches!(verified.jacobian(), Jacobian::Unit { .. }) {
        return Ok(false);
    }
    let (one, minus_one) = (context.integer(1), context.integer(-1));
    let rows = verified.row_actions();
    if rows.len() != action.len() {
        return Ok(false);
    }
    for (slot, (row, image)) in rows.iter().zip(action).enumerate() {
        let proved = match row {
            DenominatorAction::Monomial { target, scale }
                if same_eps(presentation, slot, *target)
                    && verified
                        .denominators()
                        .constant()
                        .get(slot)
                        .is_none_or(|c| c.is_zero()) =>
            {
                let target = u16::try_from(*target).ok();
                if *scale == one {
                    target.map(|t| (t, false))
                } else if *scale == minus_one {
                    target.map(|t| (t, true))
                } else {
                    None
                }
            }
            _ => None,
        };
        if proved != *image {
            return Err(PError::Contract(format!(
                "orbit canonicalisation: the integer action of an automorphism of presentation {} \
                 disagrees with its verified action on slot {slot}",
                presentation.id.0
            )));
        }
    }
    Ok(true)
}

fn same_eps(presentation: &Presentation, source: usize, target: usize) -> bool {
    let eps = |slot: usize| match presentation.slots.get(slot) {
        Some(Slot::Line(line)) => Some(line.eps_units),
        Some(Slot::Isp(_)) => Some(0),
        None => None,
    };
    matches!((eps(source), eps(target)), (Some(a), Some(b)) if a == b)
}

struct Search<'a, E: Fn(usize) -> u8, V: Fn(usize) -> Vec<i128>> {
    n: usize,
    lines: &'a [usize],
    basis: &'a [usize],
    coef: &'a [Vec<i128>],
    last: &'a [Option<usize>],
    by_vec: &'a std::collections::BTreeMap<Vec<i128>, usize>,
    det: i128,
    adj: &'a [Vec<i128>],
    eps: &'a E,
    degree: &'a [usize],
    vec_of: &'a V,
}

impl<E: Fn(usize) -> u8, V: Fn(usize) -> Vec<i128>> Search<'_, E, V> {
    /// Choose the image of basis line `k`; prune on every line whose
    /// coordinates end at `k`. `T` and `-T` act alike on keys, so the first
    /// image keeps its sign.
    fn extend(&self, k: usize, images: &mut Vec<Vec<i128>>, maps: &mut BTreeSet<Vec<i128>>) {
        if k == self.n {
            if let Some(t) = self.finish(images) {
                maps.insert(t);
            }
            return;
        }
        let source = self.basis[k];
        for &target in self.lines {
            if (self.eps)(target) != (self.eps)(source)
                || self.degree[target] != self.degree[source]
            {
                continue;
            }
            let signs: &[i128] = if k == 0 { &[1] } else { &[1, -1] };
            for &sign in signs {
                let image: Vec<i128> = (self.vec_of)(target).iter().map(|c| sign * c).collect();
                images.push(image);
                if self.consistent(k, images) {
                    self.extend(k + 1, images, maps);
                }
                images.pop();
            }
        }
    }

    fn consistent(&self, k: usize, images: &[Vec<i128>]) -> bool {
        for (index, &slot) in self.lines.iter().enumerate() {
            if self.last[index] != Some(k) {
                continue;
            }
            let Some(w) = combine(&self.coef[index], images, self.det) else {
                return false;
            };
            match self.by_vec.get(&w) {
                Some(&m) if (self.eps)(m) == (self.eps)(slot) => {}
                _ => return false,
            }
        }
        true
    }

    /// `T = B^-1 B'` in row-vector convention (`v -> v T`), when it is an
    /// integer map fixing `q` up to sign with `|det A| = 1` that permutes the
    /// support lines.
    fn finish(&self, images: &[Vec<i128>]) -> Option<Vec<i128>> {
        let n = self.n;
        let mut t = vec![0_i128; n * n];
        for row in 0..n {
            for column in 0..n {
                let mut sum = 0_i128;
                for (q, image) in images.iter().enumerate() {
                    sum = sum.checked_add(self.adj[row][q].checked_mul(image[column])?)?;
                }
                if sum % self.det != 0 {
                    return None;
                }
                t[row * n + column] = sum / self.det;
            }
        }
        let loops = n - 1;
        if (0..loops).any(|column| t[loops * n + column] != 0) || t[loops * n + loops].abs() != 1 {
            return None;
        }
        let a: Vec<Vec<i128>> = (0..loops)
            .map(|row| (0..loops).map(|column| t[row * n + column]).collect())
            .collect();
        if determinant_and_adjugate(&a).0.abs() != 1 {
            return None;
        }
        let mut hit = BTreeSet::new();
        for &slot in self.lines {
            let v = (self.vec_of)(slot);
            let mapped: Vec<i128> = (0..n)
                .map(|column| (0..n).map(|row| v[row] * t[row * n + column]).sum())
                .collect();
            match self.by_vec.get(&mapped) {
                Some(&m) if (self.eps)(m) == (self.eps)(slot) => {
                    hit.insert(m);
                }
                _ => return None,
            }
        }
        (hit.len() == self.lines.len()).then_some(t)
    }
}

/// `sum_q coef[q] images[q] / det`, when integral.
fn combine(coef: &[i128], images: &[Vec<i128>], det: i128) -> Option<Vec<i128>> {
    let width = images.first()?.len();
    let mut out = vec![0_i128; width];
    for (q, image) in images.iter().enumerate() {
        for (column, value) in image.iter().enumerate() {
            out[column] = out[column].checked_add(coef[q].checked_mul(*value)?)?;
        }
    }
    for value in &mut out {
        if *value % det != 0 {
            return None;
        }
        *value /= det;
    }
    Some(out)
}

fn row_times(v: &[i128], m: &[Vec<i128>]) -> Vec<i128> {
    (0..m.len())
        .map(|column| {
            v.iter()
                .enumerate()
                .map(|(row, &x)| x * m[row][column])
                .sum()
        })
        .collect()
}

/// `v` reduced against an echelon of integer rows; `None` when dependent.
fn reduce(echelon: &[Vec<i128>], mut v: Vec<i128>) -> Option<Vec<i128>> {
    for row in echelon {
        let pivot = row.iter().position(|&c| c != 0)?;
        if v[pivot] != 0 {
            let (p, c) = (row[pivot], v[pivot]);
            for (x, &r) in v.iter_mut().zip(row) {
                *x = *x * p - r * c;
            }
            let g = v.iter().fold(0_i128, |g, &x| gcd(g, x.abs()));
            if g > 1 {
                v.iter_mut().for_each(|x| *x /= g);
            }
        }
    }
    v.iter().any(|&c| c != 0).then_some(v)
}

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Determinant and adjugate of a small square integer matrix.
fn determinant_and_adjugate(m: &[Vec<i128>]) -> (i128, Vec<Vec<i128>>) {
    let n = m.len();
    if n == 0 {
        return (1, Vec::new());
    }
    let det = determinant(m);
    let mut adj = vec![vec![0_i128; n]; n];
    if n == 1 {
        adj[0][0] = 1;
        return (det, adj);
    }
    for (i, adj_row) in adj.iter_mut().enumerate() {
        for (j, entry) in adj_row.iter_mut().enumerate() {
            // adj[i][j] = (-1)^(i+j) det(minor without row j, column i)
            let minor: Vec<Vec<i128>> = m
                .iter()
                .enumerate()
                .filter(|&(row, _)| row != j)
                .map(|(_, r)| {
                    r.iter()
                        .enumerate()
                        .filter(|&(column, _)| column != i)
                        .map(|(_, &x)| x)
                        .collect()
                })
                .collect();
            let d = determinant(&minor);
            *entry = if (i + j) % 2 == 0 { d } else { -d };
        }
    }
    (det, adj)
}

/// The determinant of a small square integer matrix (for the structural check
/// of a restored group, [`super::saved_orbits`]).
pub(crate) fn determinant_of(m: &[Vec<i128>]) -> i128 {
    determinant(m)
}

/// Bareiss fraction-free elimination.
fn determinant(m: &[Vec<i128>]) -> i128 {
    let n = m.len();
    if n == 0 {
        return 1;
    }
    let mut a: Vec<Vec<i128>> = m.to_vec();
    let mut sign = 1_i128;
    let mut previous = 1_i128;
    for k in 0..n - 1 {
        if a[k][k] == 0 {
            let Some(swap) = (k + 1..n).find(|&row| a[row][k] != 0) else {
                return 0;
            };
            a.swap(k, swap);
            sign = -sign;
        }
        for i in k + 1..n {
            for j in k + 1..n {
                a[i][j] = (a[i][j] * a[k][k] - a[i][k] * a[k][j]) / previous;
            }
        }
        previous = a[k][k];
    }
    sign * a[n - 1][n - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinants_and_adjugates_are_exact() {
        let m = vec![vec![2, 0, 1], vec![1, 3, 2], vec![1, 1, 1]];
        let (det, adj) = determinant_and_adjugate(&m);
        assert_eq!(det, 1);
        // m * adj = det * I
        for i in 0..3 {
            for j in 0..3 {
                let s: i128 = (0..3).map(|k| m[i][k] * adj[k][j]).sum();
                assert_eq!(s, if i == j { det } else { 0 });
            }
        }
        assert_eq!(determinant(&[vec![0, 1], vec![1, 0]]), -1);
    }

    #[test]
    fn an_image_carries_the_sign_of_odd_flipped_powers() {
        let action: Vec<SlotImage> =
            vec![Some((1, false)), Some((0, false)), Some((2, true)), None];
        assert_eq!(
            OwnerOrbit::image(&action, &[1, 2, -1, 0]),
            Some((vec![2, 1, -1, 0], true))
        );
        assert_eq!(
            OwnerOrbit::image(&action, &[1, 2, -2, 0]),
            Some((vec![2, 1, -2, 0], false))
        );
        assert_eq!(OwnerOrbit::image(&action, &[1, 2, 0, -1]), None);
    }
}
