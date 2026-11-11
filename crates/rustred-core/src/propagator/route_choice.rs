//! Static-score route-map choice (M5 maps, step 2 of notes/m5-maps.md section 5).
//!
//! A routed census sector maps a source sector onto its owner through a stored
//! [`SignedAffineLoopMap`] `T`. For an automorphism `M` of the owner sector,
//! `T M` is another route of the same sector; the source's numerator slots
//! (ISPs and lines outside the support) expand into a different number of owner
//! terms. The traffic-free static score of a map is, over the source's
//! numerator slots `F_j` expressed in the owner's slot forms plus `q^2`:
//! `(Sum_j [n_j + n_j (n_j + 1) / 2], Sum_j n_j^2)`, `n_j` the number of
//! nonzero coordinates of `F_j(T k)`: the terms of `F_j` (rank-1 seeds) and of
//! `F_j^2` (rank-2 seeds, exactly `n (n + 1) / 2` distinct monomials for a
//! linear form with `n` nonzero integer terms), then Forcer's sum of squares.
//!
//! This is the Rust port of `tools/m5-maps/choose_maps.py static`: the same
//! automorphism enumeration in the same order, the same score and the same
//! tie rule (the first strict minimum over the group; the stored map is kept
//! unless strictly beaten), so it makes identical choices. Nothing here is
//! trusted: every chosen map is re-verified by `class::verify_route` before it
//! is written (example `p_choose_maps`).
//!
//! All arithmetic is exact (`i128` fractions, checked; an overflow is an error,
//! never a silent wrap).
use std::cmp::Ordering;

use super::census::SignedAffineLoopMap;
use super::error::PError;
use super::presentation::{Presentation, Slot};

/// An exact fraction over `i128`, always normalised (`den > 0`, coprime).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Frac {
    pub num: i128,
    pub den: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn overflow() -> PError {
    PError::Unsupported("route choice: exact fraction overflow (i128)".to_owned())
}

impl Frac {
    pub const ZERO: Self = Self { num: 0, den: 1 };
    pub const ONE: Self = Self { num: 1, den: 1 };

    pub fn int(value: i128) -> Self {
        Self { num: value, den: 1 }
    }

    fn new(num: i128, den: i128) -> Result<Self, PError> {
        if den == 0 {
            return Err(PError::Unsupported(
                "route choice: division by zero".to_owned(),
            ));
        }
        let g = gcd(num, den).max(1);
        let (mut num, mut den) = (num / g, den / g);
        if den < 0 {
            num = num.checked_neg().ok_or_else(overflow)?;
            den = den.checked_neg().ok_or_else(overflow)?;
        }
        Ok(Self { num, den })
    }

    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    pub fn is_integer(self) -> bool {
        self.den == 1
    }

    pub fn add(self, other: Self) -> Result<Self, PError> {
        let g = gcd(self.den, other.den);
        let l = self.den / g;
        let num = self
            .num
            .checked_mul(other.den / g)
            .and_then(|a| other.num.checked_mul(l).and_then(|b| a.checked_add(b)))
            .ok_or_else(overflow)?;
        let den = l.checked_mul(other.den).ok_or_else(overflow)?;
        Self::new(num, den)
    }

    pub fn neg(self) -> Self {
        Self {
            num: -self.num,
            den: self.den,
        }
    }

    pub fn sub(self, other: Self) -> Result<Self, PError> {
        self.add(other.neg())
    }

    pub fn mul(self, other: Self) -> Result<Self, PError> {
        if self.num == 0 || other.num == 0 {
            return Ok(Self::ZERO);
        }
        let g1 = gcd(self.num, other.den);
        let g2 = gcd(other.num, self.den);
        let num = (self.num / g1)
            .checked_mul(other.num / g2)
            .ok_or_else(overflow)?;
        let den = (self.den / g2)
            .checked_mul(other.den / g1)
            .ok_or_else(overflow)?;
        Self::new(num, den)
    }

    pub fn inv(self) -> Result<Self, PError> {
        Self::new(self.den, self.num)
    }
}

/// Exact inverse of a square matrix (Gauss-Jordan, first nonzero pivot, as
/// `mapslib.inverse`); `None` when singular.
pub fn inverse(matrix: &[Vec<Frac>]) -> Result<Option<Vec<Vec<Frac>>>, PError> {
    let n = matrix.len();
    let mut a: Vec<Vec<Frac>> = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut r = row.clone();
            r.extend((0..n).map(|j| if i == j { Frac::ONE } else { Frac::ZERO }));
            r
        })
        .collect();
    for c in 0..n {
        let Some(p) = (c..n).find(|&r| !a[r][c].is_zero()) else {
            return Ok(None);
        };
        a.swap(c, p);
        let inv = a[c][c].inv()?;
        for x in &mut a[c] {
            *x = x.mul(inv)?;
        }
        let pivot = a[c].clone();
        for (r, row) in a.iter_mut().enumerate() {
            if r == c || row[c].is_zero() {
                continue;
            }
            let f = row[c];
            for (x, y) in row.iter_mut().zip(&pivot) {
                *x = x.sub(f.mul(*y)?)?;
            }
        }
    }
    Ok(Some(a.into_iter().map(|row| row[n..].to_vec()).collect()))
}

/// Exact determinant (as `mapslib.det`).
pub fn determinant(matrix: &[Vec<Frac>]) -> Result<Frac, PError> {
    let n = matrix.len();
    let mut a = matrix.to_vec();
    let mut d = Frac::ONE;
    for c in 0..n {
        let Some(p) = (c..n).find(|&r| !a[r][c].is_zero()) else {
            return Ok(Frac::ZERO);
        };
        if p != c {
            a.swap(c, p);
            d = d.neg();
        }
        d = d.mul(a[c][c])?;
        let pivot = a[c].clone();
        for row in &mut a[c + 1..] {
            if row[c].is_zero() {
                continue;
            }
            let f = row[c].mul(pivot[c].inv()?)?;
            for (x, y) in row.iter_mut().zip(&pivot) {
                *x = x.sub(f.mul(*y)?)?;
            }
        }
    }
    Ok(d)
}

/// Index of the pair `(i, j)`, `i <= j`, of `width` variables, row major
/// (`mapslib.pair_index`).
pub fn pair_index(width: usize, i: usize, j: usize) -> usize {
    let (i, j) = if i <= j { (i, j) } else { (j, i) };
    // pairs before row i: Sum_{r < i} (width - r)
    i * width - i * i.saturating_sub(1) / 2 + (j - i)
}

/// Number of pairs `(i <= j)` of `width` variables.
pub fn pair_count(width: usize) -> usize {
    width * (width + 1) / 2
}

/// The quadratic form `l . r` as a pair vector over `(k_1.., q)` (`mapslib.form`).
pub fn bilinear_form(left: &[i64], right: &[i64]) -> Vec<i128> {
    let width = left.len();
    let mut v = vec![0_i128; pair_count(width)];
    for (i, &a) in left.iter().enumerate() {
        if a == 0 {
            continue;
        }
        for (j, &b) in right.iter().enumerate() {
            if b != 0 {
                v[pair_index(width, i, j)] += i128::from(a) * i128::from(b);
            }
        }
    }
    v
}

/// The two momentum rows of a slot (a line: its momentum twice).
pub fn slot_rows(slot: &Slot) -> (&[i64], &[i64]) {
    match slot {
        Slot::Line(line) => (&line.momentum, &line.momentum),
        Slot::Isp(isp) => (&isp.left, &isp.right),
    }
}

/// The coordinates of a presentation: its slot forms then `q^2`, with the
/// exact inverse scaled to integers (`coords * scale = v . inverse`).
#[derive(Clone, Debug)]
pub struct FormBasis {
    width: usize,
    size: usize,
    /// Row-major `size x size`, `inverse * scale`.
    inverse: Vec<i128>,
}

impl FormBasis {
    /// `None` when the forms are not a basis of the quadratic forms.
    pub fn new(forms: &[Vec<i128>], width: usize) -> Result<Option<Self>, PError> {
        let size = pair_count(width);
        if forms.len() + 1 != size {
            return Ok(None);
        }
        let mut rows: Vec<Vec<Frac>> = forms
            .iter()
            .map(|f| f.iter().map(|&x| Frac::int(x)).collect())
            .collect();
        let mut q2 = vec![Frac::ZERO; size];
        q2[pair_index(width, width - 1, width - 1)] = Frac::ONE;
        rows.push(q2);
        let Some(inv) = inverse(&rows)? else {
            return Ok(None);
        };
        let mut scale = 1_i128;
        for x in inv.iter().flatten() {
            let g = gcd(scale, x.den);
            scale = (scale / g).checked_mul(x.den).ok_or_else(overflow)?;
        }
        let inverse = inv
            .iter()
            .flatten()
            .map(|x| x.num.checked_mul(scale / x.den).ok_or_else(overflow))
            .collect::<Result<_, _>>()?;
        Ok(Some(Self {
            width,
            size,
            inverse,
        }))
    }

    /// The basis of `p`'s slots.
    pub fn of(p: &Presentation) -> Result<Option<Self>, PError> {
        let forms: Vec<_> = p
            .slots
            .iter()
            .map(|slot| {
                let (l, r) = slot_rows(slot);
                bilinear_form(l, r)
            })
            .collect();
        Self::new(&forms, usize::from(p.loops) + 1)
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Coordinate `j` of the form `v`, times the basis scale (zero exactly
    /// when the coordinate is; the last one is `q^2`).
    pub fn coordinate(&self, v: &[i128], j: usize) -> Result<i128, PError> {
        let mut acc = 0_i128;
        for (i, &x) in v.iter().enumerate() {
            if x != 0 {
                let t = x
                    .checked_mul(self.inverse[i * self.size + j])
                    .ok_or_else(overflow)?;
                acc = acc.checked_add(t).ok_or_else(overflow)?;
            }
        }
        Ok(acc)
    }

    /// Nonzero coordinates of the form `v` (the `q^2` one included): the
    /// number of terms of the linear factor (`len(linear_factor(...))`).
    pub fn terms(&self, v: &[i128]) -> Result<usize, PError> {
        let mut count = 0;
        for j in 0..self.size {
            let mut acc = 0_i128;
            for (i, &x) in v.iter().enumerate() {
                if x != 0 {
                    let t = x
                        .checked_mul(self.inverse[i * self.size + j])
                        .ok_or_else(overflow)?;
                    acc = acc.checked_add(t).ok_or_else(overflow)?;
                }
            }
            count += usize::from(acc != 0);
        }
        Ok(count)
    }
}

/// `r T` of a momentum row (`mapslib.map_row`, `SignedAffineLoopMap::map_momentum`).
pub fn map_row(row: &[i64], t: &SignedAffineLoopMap) -> Result<Vec<i64>, PError> {
    t.map_momentum(row)
}

/// `r -> (r T) M` (`mapslib.compose`).
pub fn compose(t: &SignedAffineLoopMap, m: &SignedAffineLoopMap) -> SignedAffineLoopMap {
    let l = usize::from(t.loops_out);
    let li = usize::from(t.loops_in);
    let mut a = vec![0_i64; li * l];
    let mut b = vec![0_i64; li];
    for i in 0..li {
        for j in 0..l {
            a[i * l + j] = (0..l)
                .map(|k| t.loop_linear[i * l + k] * m.loop_linear[k * l + j])
                .sum();
        }
        b[i] = (0..l)
            .map(|k| t.loop_linear[i * l + k] * m.loop_external[k])
            .sum::<i64>()
            + t.loop_external[i] * i64::from(m.external_sign);
    }
    SignedAffineLoopMap {
        loops_in: t.loops_in,
        loops_out: m.loops_out,
        loop_linear: a.into(),
        loop_external: b.into(),
        external_sign: t.external_sign * m.external_sign,
    }
}

pub fn identity_map(loops: u8) -> SignedAffineLoopMap {
    let l = usize::from(loops);
    SignedAffineLoopMap {
        loops_in: loops,
        loops_out: loops,
        loop_linear: (0..l * l).map(|x| i64::from(x / l == x % l)).collect(),
        loop_external: vec![0; l].into(),
        external_sign: 1,
    }
}

/// Every signed affine loop map `M` of `p` (row action) permuting the support
/// lines up to sign with equal eps units, `M` and `-M` kept once; the order of
/// `mapslib.automorphisms` exactly (it decides ties). With fewer than `L`
/// independent support lines: the identity alone.
pub fn sector_automorphisms(
    p: &Presentation,
    support: u64,
) -> Result<Vec<SignedAffineLoopMap>, PError> {
    let l = usize::from(p.loops);
    let lines: Vec<usize> = p
        .slots
        .iter()
        .enumerate()
        .filter(|(i, s)| matches!(s, Slot::Line(_)) && support >> i & 1 == 1)
        .map(|(i, _)| i)
        .collect();
    let row = |i: usize| match &p.slots[i] {
        Slot::Line(line) => &line.momentum,
        Slot::Isp(_) => unreachable!(),
    };
    let eps = |i: usize| match &p.slots[i] {
        Slot::Line(line) => line.eps_units,
        Slot::Isp(_) => unreachable!(),
    };
    // greedy independent k-parts (the Gram determinant test of mapslib)
    let mut basis: Vec<usize> = Vec::new();
    for &i in &lines {
        let mut cand = basis.clone();
        cand.push(i);
        let gram: Vec<Vec<Frac>> = cand
            .iter()
            .map(|&a| {
                cand.iter()
                    .map(|&b| Frac::int((0..l).map(|k| i128::from(row(a)[k] * row(b)[k])).sum()))
                    .collect()
            })
            .collect();
        if !determinant(&gram)?.is_zero() {
            basis = cand;
        }
        if basis.len() == l {
            break;
        }
    }
    if basis.len() < l {
        return Ok(vec![identity_map(p.loops)]);
    }
    let rk: Vec<Vec<Frac>> = basis
        .iter()
        .map(|&j| (0..l).map(|k| Frac::int(i128::from(row(j)[k]))).collect())
        .collect();
    let rinv = inverse(&rk)?.ok_or_else(|| PError::Contract("singular line basis".to_owned()))?;
    // r_i = Sum_m c_m r_basis_m + c_q e_q
    let mut coef: Vec<(Vec<Frac>, Frac)> = Vec::with_capacity(lines.len());
    let mut lastdep: Vec<isize> = Vec::with_capacity(lines.len());
    for &i in &lines {
        let mut c = vec![Frac::ZERO; l];
        for (m, cm) in c.iter_mut().enumerate() {
            for (k, rinv_k) in rinv.iter().enumerate() {
                *cm = cm.add(Frac::int(i128::from(row(i)[k])).mul(rinv_k[m])?)?;
            }
        }
        let mut cq = Frac::int(i128::from(row(i)[l]));
        for (m, cm) in c.iter().enumerate() {
            cq = cq.sub(cm.mul(Frac::int(i128::from(row(basis[m])[l])))?)?;
        }
        lastdep.push(
            (0..l)
                .rev()
                .find(|&m| !c[m].is_zero())
                .map_or(-1, |m| m as isize),
        );
        coef.push((c, cq));
    }
    // by_vec: later lines overwrite earlier ones, as the Python dict does
    let mut by_vec: std::collections::HashMap<Vec<i64>, usize> = Default::default();
    for (pos, &i) in lines.iter().enumerate() {
        by_vec.insert(row(i).clone(), pos);
        by_vec.insert(row(i).iter().map(|x| -x).collect(), pos);
    }
    struct Ctx<'a> {
        l: usize,
        lines: &'a [usize],
        coef: &'a [(Vec<Frac>, Frac)],
        lastdep: &'a [isize],
        by_vec: &'a std::collections::HashMap<Vec<i64>, usize>,
        eps: Vec<u8>,
        rows: Vec<Vec<i64>>,
        basis_pos: Vec<usize>,
        rinv: &'a [Vec<Frac>],
        s: i64,
        out: Vec<SignedAffineLoopMap>,
    }
    fn image(ctx: &Ctx, pos: usize, imgs: &[Vec<i64>]) -> Result<Option<Vec<i64>>, PError> {
        let (c, cq) = &ctx.coef[pos];
        let mut v = Vec::with_capacity(ctx.l + 1);
        for k in 0..=ctx.l {
            let mut x = Frac::ZERO;
            for (m, img) in imgs.iter().enumerate() {
                if !c[m].is_zero() {
                    x = x.add(c[m].mul(Frac::int(i128::from(img[k])))?)?;
                }
            }
            if k == ctx.l {
                x = x.add(cq.mul(Frac::int(i128::from(ctx.s)))?)?;
            }
            v.push(x);
        }
        if v.iter().any(|x| !x.is_integer()) {
            return Ok(None);
        }
        Ok(Some(
            v.into_iter()
                .map(|x| i64::try_from(x.num).map_err(|_| overflow()))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn dfs(
        ctx: &mut Ctx,
        m: usize,
        imgs: &mut Vec<Vec<i64>>,
        used: &mut Vec<bool>,
    ) -> Result<(), PError> {
        let l = ctx.l;
        if m == l {
            let mut perm: Vec<Option<usize>> = vec![None; ctx.lines.len()];
            let mut taken = vec![false; ctx.lines.len()];
            for pos in 0..ctx.lines.len() {
                let Some(v) = image(ctx, pos, imgs)? else {
                    return Ok(());
                };
                let Some(&j) = ctx.by_vec.get(&v) else {
                    return Ok(());
                };
                if ctx.eps[j] != ctx.eps[pos] || taken[j] {
                    return Ok(());
                }
                taken[j] = true;
                perm[pos] = Some(j);
            }
            // A = Rinv Img_k ; b = Rinv (img_q - r_q s)
            let mut a = vec![0_i64; l * l];
            let mut b = vec![0_i64; l];
            let mut afrac = vec![vec![Frac::ZERO; l]; l];
            for i in 0..l {
                for j in 0..l {
                    let mut x = Frac::ZERO;
                    for kk in 0..l {
                        x = x.add(ctx.rinv[i][kk].mul(Frac::int(i128::from(imgs[kk][j])))?)?;
                    }
                    if !x.is_integer() {
                        return Ok(());
                    }
                    afrac[i][j] = x;
                    a[i * l + j] = i64::try_from(x.num).map_err(|_| overflow())?;
                }
            }
            for i in 0..l {
                let mut x = Frac::ZERO;
                for kk in 0..l {
                    let iq = i128::from(imgs[kk][l])
                        - i128::from(ctx.rows[ctx.basis_pos[kk]][l]) * i128::from(ctx.s);
                    x = x.add(ctx.rinv[i][kk].mul(Frac::int(iq))?)?;
                }
                if !x.is_integer() {
                    return Ok(());
                }
                b[i] = i64::try_from(x.num).map_err(|_| overflow())?;
            }
            let det = determinant(&afrac)?;
            if det != Frac::ONE && det != Frac::int(-1) {
                return Ok(());
            }
            ctx.out.push(SignedAffineLoopMap {
                loops_in: l as u8,
                loops_out: l as u8,
                loop_linear: a.into(),
                loop_external: b.into(),
                external_sign: ctx.s as i8,
            });
            return Ok(());
        }
        let src = ctx.basis_pos[m];
        for j in 0..ctx.lines.len() {
            if used[j] || ctx.eps[j] != ctx.eps[src] {
                continue;
            }
            for sg in [1_i64, -1] {
                let img: Vec<i64> = ctx.rows[j].iter().map(|x| sg * x).collect();
                imgs.push(img);
                let mut ok = true;
                for pos in 0..ctx.lines.len() {
                    if ctx.lastdep[pos] == m as isize {
                        let v = image(ctx, pos, imgs)?;
                        let hit = v.and_then(|v| ctx.by_vec.get(&v).copied());
                        if hit.is_none_or(|h| ctx.eps[h] != ctx.eps[pos]) {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    used[j] = true;
                    dfs(ctx, m + 1, imgs, used)?;
                    used[j] = false;
                }
                imgs.pop();
            }
        }
        Ok(())
    }
    let basis_pos: Vec<usize> = basis
        .iter()
        .map(|b| lines.iter().position(|x| x == b).expect("basis line"))
        .collect();
    let mut ctx = Ctx {
        l,
        lines: &lines,
        coef: &coef,
        lastdep: &lastdep,
        by_vec: &by_vec,
        eps: lines.iter().map(|&i| eps(i)).collect(),
        rows: lines.iter().map(|&i| row(i).clone()).collect(),
        basis_pos,
        rinv: &rinv,
        s: 1,
        out: Vec::new(),
    };
    for s in [1_i64, -1] {
        ctx.s = s;
        let mut imgs = Vec::new();
        let mut used = vec![false; lines.len()];
        dfs(&mut ctx, 0, &mut imgs, &mut used)?;
    }
    // dedupe +-M, keeping the first of each pair
    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for m in ctx.out {
        let key: Vec<i64> = m
            .loop_linear
            .iter()
            .chain(m.loop_external.iter())
            .copied()
            .chain([i64::from(m.external_sign)])
            .collect();
        let neg: Vec<i64> = key.iter().map(|x| -x).collect();
        if seen.contains(&key) || seen.contains(&neg) {
            continue;
        }
        seen.insert(key);
        unique.push(m);
    }
    Ok(unique)
}

/// The static score of routing `source`'s sector `support` through `t` into
/// the owner `basis` (module docs).
pub fn static_score(
    source: &Presentation,
    support: u64,
    t: &SignedAffineLoopMap,
    owner: &FormBasis,
) -> Result<(u64, u64), PError> {
    let (mut s1, mut s2, mut proxy) = (0_u64, 0_u64, 0_u64);
    for (j, slot) in source.slots.iter().enumerate() {
        if matches!(slot, Slot::Line(_)) && support >> j & 1 == 1 {
            continue;
        }
        let (l, r) = slot_rows(slot);
        let form = bilinear_form(&t.map_momentum(l)?, &t.map_momentum(r)?);
        let n = owner.terms(&form)? as u64;
        s1 += n;
        s2 += n * (n + 1) / 2;
        proxy += n * n;
    }
    Ok((s1 + s2, proxy))
}

/// One sector's choice.
#[derive(Clone, Debug)]
pub struct RouteChoice {
    pub map: SignedAffineLoopMap,
    pub score_before: (u64, u64),
    pub score_after: (u64, u64),
}

/// What [`choose_static`] decided for one routed sector.
#[derive(Clone, Debug)]
pub enum RouteDecision {
    /// The owner sector group is trivial (or has one element).
    TrivialGroup,
    /// The stored map is (one of) the best.
    Kept,
    Changed(RouteChoice),
}

/// The static choice for one routed sector, `group` the owner sector's
/// [`sector_automorphisms`] and `owner` its [`FormBasis`].
pub fn choose_static(
    source: &Presentation,
    support: u64,
    stored: &SignedAffineLoopMap,
    group: &[SignedAffineLoopMap],
    owner: &FormBasis,
) -> Result<RouteDecision, PError> {
    if group.len() <= 1 {
        return Ok(RouteDecision::TrivialGroup);
    }
    let mut best: Option<(SignedAffineLoopMap, (u64, u64))> = None;
    let mut current = None;
    for m in group {
        let t2 = compose(stored, m);
        let score = static_score(source, support, &t2, owner)?;
        if t2 == *stored {
            current = Some(score);
        }
        if best
            .as_ref()
            .is_none_or(|(_, b)| score.cmp(b) == Ordering::Less)
        {
            best = Some((t2, score));
        }
    }
    let (map, after) = best.expect("a nonempty group");
    match current {
        Some(before) if after < before => Ok(RouteDecision::Changed(RouteChoice {
            map,
            score_before: before,
            score_after: after,
        })),
        _ => Ok(RouteDecision::Kept),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_index_is_row_major_upper_triangle() {
        let width = 4;
        let mut expected = 0;
        for i in 0..width {
            for j in i..width {
                assert_eq!(pair_index(width, i, j), expected);
                assert_eq!(pair_index(width, j, i), expected);
                expected += 1;
            }
        }
        assert_eq!(expected, pair_count(width));
    }

    #[test]
    fn fractions_are_exact() {
        let a = Frac::new(6, -4).unwrap();
        assert_eq!(a, Frac { num: -3, den: 2 });
        assert_eq!(a.add(Frac::new(3, 2).unwrap()).unwrap(), Frac::ZERO);
        assert_eq!(a.mul(a.inv().unwrap()).unwrap(), Frac::ONE);
    }

    #[test]
    fn unit_basis_counts_terms() {
        // L = 1: forms k^2, k.q, then q^2: (k + q)^2 = k^2 + 2 k.q + q^2 has 3 terms.
        let width = 2;
        let forms = vec![
            bilinear_form(&[1, 0], &[1, 0]),
            bilinear_form(&[1, 0], &[0, 1]),
        ];
        let basis = FormBasis::new(&forms, width).unwrap().unwrap();
        assert_eq!(basis.terms(&bilinear_form(&[1, 1], &[1, 1])).unwrap(), 3);
        assert_eq!(basis.terms(&bilinear_form(&[1, 0], &[1, 0])).unwrap(), 1);
    }

    #[test]
    fn compose_with_identity_is_identity() {
        let t = SignedAffineLoopMap {
            loops_in: 2,
            loops_out: 2,
            loop_linear: vec![-1, -1, 0, 1].into(),
            loop_external: vec![1, 0].into(),
            external_sign: -1,
        };
        assert_eq!(compose(&t, &identity_map(2)), t);
    }
}
