//! Vectorised Montgomery arithmetic of the sampled replay across the points
//! of a batch (vk5l-physrun).
//!
//! The replay keeps every trace value as a row over the points of a batch
//! (structure of arrays); its four inner loops are `d = s·c` and `d += s·c`
//! with `c` a constant or a row. On x86-64 with AVX-512 F and DQ these run
//! eight points per instruction: the 64×64→128-bit product is built from four
//! 32×32→64 multiplies, Montgomery's `m = t·(−p⁻¹) mod 2⁶⁴` uses `vpmullq`,
//! and the result is reduced to `[0, p)`. Montgomery reduction has a unique
//! result in `[0, p)`, so every element equals the scalar `Zp64` operation bit
//! for bit (checked by the tests). Primes must lie below 2⁶³ (the replay's
//! primes lie near 2⁶²); larger primes and other targets take the scalar path.
//! `RUSTRED_REPLAY_SIMD=off` forces the scalar path.
use symbolica::domains::finite_field::{FiniteFieldCore, Zp64};
use symbolica::domains::RingOps;

use super::field::Fe;

/// The Montgomery constants of one prime for the vector kernels.
#[derive(Clone, Copy)]
pub(super) struct Kernel {
    p: u64,
    /// `−p⁻¹ mod 2⁶⁴`.
    neg_inverse: u64,
    vector: bool,
}

fn enabled() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        if matches!(std::env::var("RUSTRED_REPLAY_SIMD").as_deref(), Ok("off" | "0")) {
            return false;
        }
        #[cfg(target_arch = "x86_64")]
        {
            std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512dq")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    });
    *ON
}

impl Kernel {
    pub(super) fn new(field: &Zp64) -> Self {
        let p = field.get_prime();
        // p⁻¹ mod 2⁶⁴ by Newton iteration (p odd).
        let mut inverse = p;
        for _ in 0..6 {
            inverse = inverse.wrapping_mul(2u64.wrapping_sub(p.wrapping_mul(inverse)));
        }
        let layout = std::mem::size_of::<Fe>() == 8 && std::mem::align_of::<Fe>() == 8;
        Self {
            p,
            neg_inverse: inverse.wrapping_neg(),
            vector: enabled() && layout && p < (1 << 63),
        }
    }

    /// `destination[j] = source[j] · c[j]` (`c` a row) or `· c` (a constant),
    /// assigned or added.
    #[inline]
    pub(super) fn apply(
        &self,
        field: &Zp64,
        destination: &mut [Fe],
        source: &[Fe],
        coefficient: Coefficient<'_>,
        assign: bool,
    ) {
        #[cfg(target_arch = "x86_64")]
        if self.vector && destination.len() >= 8 {
            // SAFETY: Fe is a single u64 newtype (size and alignment checked
            // in `new`), the slices do not alias (distinct rows of the replay)
            // and the CPU features were detected at run time.
            unsafe {
                let d = std::slice::from_raw_parts_mut(destination.as_mut_ptr() as *mut u64, destination.len());
                let s = std::slice::from_raw_parts(source.as_ptr() as *const u64, source.len());
                match coefficient {
                    Coefficient::Constant(c) => {
                        let c = *(&c as *const Fe as *const u64);
                        x86::apply_constant(self.p, self.neg_inverse, d, s, c, assign);
                    }
                    Coefficient::Row(c) => {
                        let c = std::slice::from_raw_parts(c.as_ptr() as *const u64, c.len());
                        x86::apply_row(self.p, self.neg_inverse, d, s, c, assign);
                    }
                }
            }
            return;
        }
        scalar(field, destination, source, coefficient, assign);
    }
}

/// The coefficient of one replay operation.
#[derive(Clone, Copy)]
pub(super) enum Coefficient<'a> {
    Constant(Fe),
    Row(&'a [Fe]),
}

#[inline]
pub(super) fn scalar(field: &Zp64, destination: &mut [Fe], source: &[Fe], coefficient: Coefficient<'_>, assign: bool) {
    match (coefficient, assign) {
        (Coefficient::Constant(c), true) => {
            for (d, s) in destination.iter_mut().zip(source) {
                *d = field.mul(s, &c);
            }
        }
        (Coefficient::Constant(c), false) => {
            for (d, s) in destination.iter_mut().zip(source) {
                *d = field.add(&*d, &field.mul(s, &c));
            }
        }
        (Coefficient::Row(c), true) => {
            for ((d, s), c) in destination.iter_mut().zip(source).zip(c) {
                *d = field.mul(s, c);
            }
        }
        (Coefficient::Row(c), false) => {
            for ((d, s), c) in destination.iter_mut().zip(source).zip(c) {
                *d = field.add(&*d, &field.mul(s, c));
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    use std::arch::x86_64::*;

    /// Montgomery product of eight lanes, reduced to `[0, p)`.
    #[inline]
    #[target_feature(enable = "avx512f,avx512dq")]
    fn mont_mul(a: __m512i, b: __m512i, p: __m512i, p_hi: __m512i, neg_inverse: __m512i) -> __m512i {
        let mask = _mm512_set1_epi64(0xffff_ffff);
        let a_hi = _mm512_srli_epi64::<32>(a);
        let b_hi = _mm512_srli_epi64::<32>(b);
        let p00 = _mm512_mul_epu32(a, b);
        let p01 = _mm512_mul_epu32(a, b_hi);
        let p10 = _mm512_mul_epu32(a_hi, b);
        let p11 = _mm512_mul_epu32(a_hi, b_hi);
        let mid = _mm512_add_epi64(
            _mm512_add_epi64(_mm512_srli_epi64::<32>(p00), _mm512_and_si512(p01, mask)),
            _mm512_and_si512(p10, mask),
        );
        let t_lo = _mm512_or_si512(_mm512_and_si512(p00, mask), _mm512_slli_epi64::<32>(mid));
        let t_hi = _mm512_add_epi64(
            _mm512_add_epi64(p11, _mm512_srli_epi64::<32>(p01)),
            _mm512_add_epi64(_mm512_srli_epi64::<32>(p10), _mm512_srli_epi64::<32>(mid)),
        );
        let m = _mm512_mullo_epi64(t_lo, neg_inverse);
        let m_hi = _mm512_srli_epi64::<32>(m);
        let q00 = _mm512_mul_epu32(m, p);
        let q01 = _mm512_mul_epu32(m, p_hi);
        let q10 = _mm512_mul_epu32(m_hi, p);
        let q11 = _mm512_mul_epu32(m_hi, p_hi);
        let qmid = _mm512_add_epi64(
            _mm512_add_epi64(_mm512_srli_epi64::<32>(q00), _mm512_and_si512(q01, mask)),
            _mm512_and_si512(q10, mask),
        );
        let mp_hi = _mm512_add_epi64(
            _mm512_add_epi64(q11, _mm512_srli_epi64::<32>(q01)),
            _mm512_add_epi64(_mm512_srli_epi64::<32>(q10), _mm512_srli_epi64::<32>(qmid)),
        );
        // t + m·p ≡ 0 mod 2⁶⁴: the low halves carry exactly when t_lo ≠ 0.
        let mut u = _mm512_add_epi64(t_hi, mp_hi);
        let carry = _mm512_test_epi64_mask(t_lo, t_lo);
        u = _mm512_mask_add_epi64(u, carry, u, _mm512_set1_epi64(1));
        let reduce = _mm512_cmpge_epu64_mask(u, p);
        _mm512_mask_sub_epi64(u, reduce, u, p)
    }

    #[inline]
    #[target_feature(enable = "avx512f")]
    fn add_mod(a: __m512i, b: __m512i, p: __m512i) -> __m512i {
        let r = _mm512_add_epi64(a, b);
        let reduce = _mm512_cmpge_epu64_mask(r, p);
        _mm512_mask_sub_epi64(r, reduce, r, p)
    }

    fn mont_mul_scalar(a: u64, b: u64, p: u64, neg_inverse: u64) -> u64 {
        let t = a as u128 * b as u128;
        let m = (t as u64).wrapping_mul(neg_inverse);
        let u = ((t + m as u128 * p as u128) >> 64) as u64;
        if u >= p { u - p } else { u }
    }

    fn add_scalar(a: u64, b: u64, p: u64) -> u64 {
        let r = a + b;
        if r >= p { r - p } else { r }
    }

    #[target_feature(enable = "avx512f,avx512dq")]
    pub(super) unsafe fn apply_constant(p: u64, neg_inverse: u64, d: &mut [u64], s: &[u64], c: u64, assign: bool) {
        let n = d.len().min(s.len());
        let pv = _mm512_set1_epi64(p as i64);
        let p_hi = _mm512_set1_epi64((p >> 32) as i64);
        let ni = _mm512_set1_epi64(neg_inverse as i64);
        let cv = _mm512_set1_epi64(c as i64);
        let mut j = 0;
        while j + 8 <= n {
            unsafe {
                let sv = _mm512_loadu_si512(s.as_ptr().add(j) as *const __m512i);
                let product = mont_mul(sv, cv, pv, p_hi, ni);
                let out = if assign {
                    product
                } else {
                    add_mod(_mm512_loadu_si512(d.as_ptr().add(j) as *const __m512i), product, pv)
                };
                _mm512_storeu_si512(d.as_mut_ptr().add(j) as *mut __m512i, out);
            }
            j += 8;
        }
        while j < n {
            let product = mont_mul_scalar(s[j], c, p, neg_inverse);
            d[j] = if assign { product } else { add_scalar(d[j], product, p) };
            j += 1;
        }
    }

    #[target_feature(enable = "avx512f,avx512dq")]
    pub(super) unsafe fn apply_row(p: u64, neg_inverse: u64, d: &mut [u64], s: &[u64], c: &[u64], assign: bool) {
        let n = d.len().min(s.len()).min(c.len());
        let pv = _mm512_set1_epi64(p as i64);
        let p_hi = _mm512_set1_epi64((p >> 32) as i64);
        let ni = _mm512_set1_epi64(neg_inverse as i64);
        let mut j = 0;
        while j + 8 <= n {
            unsafe {
                let sv = _mm512_loadu_si512(s.as_ptr().add(j) as *const __m512i);
                let cv = _mm512_loadu_si512(c.as_ptr().add(j) as *const __m512i);
                let product = mont_mul(sv, cv, pv, p_hi, ni);
                let out = if assign {
                    product
                } else {
                    add_mod(_mm512_loadu_si512(d.as_ptr().add(j) as *const __m512i), product, pv)
                };
                _mm512_storeu_si512(d.as_mut_ptr().add(j) as *mut __m512i, out);
            }
            j += 8;
        }
        while j < n {
            let product = mont_mul_scalar(s[j], c[j], p, neg_inverse);
            d[j] = if assign { product } else { add_scalar(d[j], product, p) };
            j += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use symbolica::domains::finite_field::FiniteFieldCore;

    use super::*;

    /// Every vector kernel equals the scalar Zp64 arithmetic bit for bit, for
    /// replay-size primes near 2⁶² and 2⁶³ - 25 and widths with tails.
    #[test]
    fn the_vector_kernels_equal_the_scalar_field_arithmetic() {
        let mut rng = StdRng::seed_from_u64(7);
        for prime in [4611686018427388039u64, 6917529027641081903, 9223372036854775783] {
            let field = Zp64::new(prime);
            let kernel = Kernel::new(&field);
            for width in [1usize, 7, 8, 9, 31, 64, 67, 128] {
                let element = |rng: &mut StdRng| field.to_element(rng.random_range(0..prime));
                let source: Vec<Fe> = (0..width).map(|_| element(&mut rng)).collect();
                let row: Vec<Fe> = (0..width).map(|_| element(&mut rng)).collect();
                let start: Vec<Fe> = (0..width).map(|_| element(&mut rng)).collect();
                let constant = element(&mut rng);
                for assign in [true, false] {
                    for coefficient in [Coefficient::Constant(constant), Coefficient::Row(&row)] {
                        let mut expected = start.clone();
                        scalar(&field, &mut expected, &source, coefficient, assign);
                        let mut got = start.clone();
                        kernel.apply(&field, &mut got, &source, coefficient, assign);
                        assert_eq!(got, expected, "prime {prime} width {width} assign {assign}");
                    }
                }
            }
        }
    }
}
