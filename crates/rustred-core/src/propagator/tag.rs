//! Gamma tags `Prod Gamma(1 + m eps)^e` carried by p keys (owner: M2-I0 rr-iface).
//!
//! A tag is stored sparse, sorted by `m`, without `m = 0` (`Gamma(1) = 1`) and
//! without zero exponents, so equal tags are equal values. Inside a key it
//! takes [`P_GAMMA_TAG_WIDTH`] dense slots, one exponent per `m` in `-8..=8`
//! without `0`; the layout is part of [`P_GAMMA_TAG_VERSION`].
//!
//! [`P_GAMMA_TAG_VERSION`]: super::P_GAMMA_TAG_VERSION
use std::collections::BTreeMap;

use smallvec::SmallVec;

use super::error::PError;
use super::{P_GAMMA_TAG_MAX_M, P_GAMMA_TAG_WIDTH};

/// Largest `|e|` of a key: every slot of a recorded step identity fits `i8`.
const MAX_EXPONENT: i16 = 127;

fn in_range(m: i8, e: i16) -> bool {
    m != 0 && (-P_GAMMA_TAG_MAX_M..=P_GAMMA_TAG_MAX_M).contains(&m) && e.abs() <= MAX_EXPONENT
}

/// Dense slot of `m != 0`, `|m| <= 8`: `-8..=-1` then `1..=8`.
fn slot(m: i8) -> usize {
    let index = if m < 0 { m + 8 } else { m + 7 };
    index as usize
}

/// Sparse exponents `(m != 0, e != 0)` sorted by `m`: the value `Prod Gamma(1 + m eps)^e`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GammaTag(SmallVec<[(i8, i16); 4]>);

impl GammaTag {
    /// The empty product.
    pub fn one() -> Self {
        Self::default()
    }

    /// The tag of arbitrary `(m, e)` pairs: equal `m` add, `m = 0` and exponents
    /// that cancel disappear. A remaining pair outside `|m| <= 8`, `|e| <= 127`
    /// is [`PError::TagOutOfRange`].
    pub fn from_pairs(pairs: impl IntoIterator<Item = (i8, i16)>) -> Result<Self, PError> {
        let mut merged = BTreeMap::<i8, i64>::new();
        for (m, e) in pairs {
            if m != 0 {
                *merged.entry(m).or_default() += i64::from(e);
            }
        }
        let mut exponents = SmallVec::new();
        for (m, e) in merged {
            if e == 0 {
                continue;
            }
            let e = i16::try_from(e).map_err(|_| PError::TagOutOfRange)?;
            if !in_range(m, e) {
                return Err(PError::TagOutOfRange);
            }
            exponents.push((m, e));
        }
        Ok(Self(exponents))
    }

    /// The product of two tags. Exponents saturate at `i16`: a saturated tag is
    /// far outside the key range, so [`Self::encode_dense`] refuses it.
    pub fn add(&self, other: &Self) -> Self {
        let mut exponents = SmallVec::with_capacity(self.0.len() + other.0.len());
        let (mut left, mut right) = (self.0.iter().peekable(), other.0.iter().peekable());
        loop {
            let next = match (left.peek(), right.peek()) {
                (Some(&&(a, x)), Some(&&(b, y))) if a == b => {
                    left.next();
                    right.next();
                    (a, x.saturating_add(y))
                }
                (Some(&&(a, _)), Some(&&(b, _))) if b < a => *right.next().expect("peeked"),
                (Some(_), _) => *left.next().expect("peeked"),
                (None, Some(_)) => *right.next().expect("peeked"),
                (None, None) => break,
            };
            if next.1 != 0 {
                exponents.push(next);
            }
        }
        Self(exponents)
    }

    /// The inverse tag.
    pub fn neg(&self) -> Self {
        Self(
            self.0
                .iter()
                .map(|&(m, e)| (m, e.saturating_neg()))
                .collect(),
        )
    }

    pub fn is_one(&self) -> bool {
        self.0.is_empty()
    }

    pub fn exponents(&self) -> &[(i8, i16)] {
        &self.0
    }

    /// Append exactly [`P_GAMMA_TAG_WIDTH`] dense slots, each within `i8`;
    /// `out` is unchanged on [`PError::TagOutOfRange`].
    pub fn encode_dense(&self, out: &mut Vec<i64>) -> Result<(), PError> {
        let mut dense = [0; P_GAMMA_TAG_WIDTH];
        for &(m, e) in &self.0 {
            if !in_range(m, e) {
                return Err(PError::TagOutOfRange);
            }
            dense[slot(m)] = i64::from(e);
        }
        out.extend_from_slice(&dense);
        Ok(())
    }

    /// The tag of exactly [`P_GAMMA_TAG_WIDTH`] dense slots.
    pub fn decode_dense(slots: &[i64]) -> Result<Self, PError> {
        if slots.len() != P_GAMMA_TAG_WIDTH {
            return Err(PError::TagOutOfRange);
        }
        let mut exponents = SmallVec::new();
        for (m, &e) in (-P_GAMMA_TAG_MAX_M..=P_GAMMA_TAG_MAX_M)
            .filter(|&m| m != 0)
            .zip(slots)
        {
            let e = i16::try_from(e).map_err(|_| PError::TagOutOfRange)?;
            if e != 0 {
                if !in_range(m, e) {
                    return Err(PError::TagOutOfRange);
                }
                exponents.push((m, e));
            }
        }
        Ok(Self(exponents))
    }
}
