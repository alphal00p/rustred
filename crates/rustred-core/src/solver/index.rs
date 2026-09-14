use std::cmp::Ordering;
use std::fmt;
use std::ops::Index;

use super::SolverError;

/// Failure to represent or apply a compact propagator power.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerError {
    OutOfRange { value: i32 },
    NumericShift { index: usize },
}

impl fmt::Display for PowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange { value } => {
                write!(
                    f,
                    "propagator power {value} is outside the compact range -64..=63"
                )
            }
            Self::NumericShift { index } => {
                write!(f, "cannot symbolically shift numeric propagator {index}")
            }
        }
    }
}

impl std::error::Error for PowerError {}

/// SpIRed's active `pow8` representation: one symbolic bit and seven signed bits.
///
/// The derived ordering compares the encoded byte, as C++ `pow8` does. Integral
/// reduction ordering is provided separately by [`IntegralOrder`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Power(u8);

impl Power {
    pub const MIN: i16 = -64;
    pub const MAX: i16 = 63;

    pub const fn new(symbolic: bool, value: i16) -> Result<Self, PowerError> {
        Self::from_wide(symbolic, value as i32)
    }

    const fn from_wide(symbolic: bool, value: i32) -> Result<Self, PowerError> {
        if value < Self::MIN as i32 || value > Self::MAX as i32 {
            return Err(PowerError::OutOfRange { value });
        }
        Ok(Self((value as u8 & 0x7f) | if symbolic { 0x80 } else { 0 }))
    }

    pub const fn value(self) -> i16 {
        // Shift the seven-bit sign into the native sign position, then extend.
        ((self.0 << 1) as i8 >> 1) as i16
    }

    pub const fn is_symbolic(self) -> bool {
        self.0 & 0x80 != 0
    }

    /// Add a displacement without truncating or wrapping either signed value.
    pub const fn shifted(self, delta: i16) -> Result<Self, PowerError> {
        Self::from_wide(self.is_symbolic(), self.value() as i32 + delta as i32)
    }
}

/// An integral key with fixed inline storage and no allocation per key.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Integral<const N: usize>([Power; N]);

impl<const N: usize> Integral<N> {
    pub const fn new(powers: [Power; N]) -> Self {
        Self(powers)
    }

    pub const fn powers(&self) -> &[Power; N] {
        &self.0
    }

    pub fn symbolic(values: [i16; N]) -> Result<Self, PowerError> {
        Self::with_symbolic(true, values)
    }

    pub fn numeric(values: [i16; N]) -> Result<Self, PowerError> {
        Self::with_symbolic(false, values)
    }

    fn with_symbolic(symbolic: bool, values: [i16; N]) -> Result<Self, PowerError> {
        let mut powers = [Power::default(); N];
        for (power, value) in powers.iter_mut().zip(values) {
            *power = Power::new(symbolic, value)?;
        }
        Ok(Self(powers))
    }

    /// Apply the symbolic shifts accepted by C++ `integral::shift`.
    /// Numeric coordinates must have zero displacement.
    pub fn shifted(&self, shifts: [i16; N]) -> Result<Self, PowerError> {
        let mut result = *self;
        for (index, (power, shift)) in result.0.iter_mut().zip(shifts).enumerate() {
            if shift != 0 {
                if !power.is_symbolic() {
                    return Err(PowerError::NumericShift { index });
                }
                *power = power.shifted(shift)?;
            }
        }
        Ok(result)
    }
}

impl<const N: usize> Default for Integral<N> {
    fn default() -> Self {
        Self([Power::default(); N])
    }
}

impl<const N: usize> Index<usize> for Integral<N> {
    type Output = Power;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<const N: usize> fmt::Display for Integral<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("I(")?;
        for (index, power) in self.powers().iter().enumerate() {
            if index != 0 {
                f.write_str(",")?;
            }
            if power.is_symbolic() {
                write!(f, "n{index}")?;
                if power.value() != 0 {
                    write!(f, "{:+}", power.value())?;
                }
            } else {
                write!(f, "{}", power.value())?;
            }
        }
        f.write_str(")")
    }
}

/// The harder-first order of SpIRed's `intLessStatic` / `intLessDynamic`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntegralOrder<const N: usize> {
    sector: [bool; N],
    deltas: [bool; N],
    permutation: Option<[usize; N]>,
}

impl<const N: usize> IntegralOrder<N> {
    pub const fn new(sector: [bool; N], deltas: [bool; N]) -> Self {
        Self {
            sector,
            deltas,
            permutation: None,
        }
    }

    /// Set the coordinate order of the final denominator/numerator tie breaks.
    ///
    /// As in C++ `intLessDynamic`, this does not permute sector comparisons,
    /// cut priorities, powers, or family coordinates. The identity permutation
    /// is normalized to the static path, including when replacing an override.
    pub fn with_permutation(mut self, permutation: [usize; N]) -> Result<Self, SolverError> {
        let mut seen = [false; N];
        let mut identity = true;
        for (position, &index) in permutation.iter().enumerate() {
            if index >= N {
                return Err(SolverError::InvalidInput(format!(
                    "ordering permutation position {position} names coordinate {index}, outside 0..{N}"
                )));
            }
            if std::mem::replace(&mut seen[index], true) {
                return Err(SolverError::InvalidInput(format!(
                    "ordering permutation repeats coordinate {index} at position {position}"
                )));
            }
            identity &= position == index;
        }
        self.permutation = (!identity).then_some(permutation);
        Ok(self)
    }

    /// An explicit nonidentity tie-break permutation; `None` means identity.
    pub const fn permutation(&self) -> Option<&[usize; N]> {
        self.permutation.as_ref()
    }

    pub const fn sector(&self) -> &[bool; N] {
        &self.sector
    }

    pub const fn deltas(&self) -> &[bool; N] {
        &self.deltas
    }

    /// Compare two keys within one symbolic/numeric coordinate pattern.
    ///
    /// Returns `Less` when `left` is harder. Like `intLessStatic`, this requires
    /// the two keys to agree on which coordinates are symbolic; symbolic delta
    /// coordinates must belong to the denominator sector.
    pub fn compare(&self, left: &Integral<N>, right: &Integral<N>) -> Ordering {
        let (mut denominators_left, mut denominators_right) = (0usize, 0usize);
        let mut sector_lex = Ordering::Equal;
        let mut delta_order = Ordering::Equal;
        let (mut absolute_left, mut absolute_right) = (0i64, 0i64);
        let (mut numerator_left, mut numerator_right) = (0i64, 0i64);

        for index in 0..N {
            let (left_power, right_power) = (left[index], right[index]);
            assert_eq!(
                left_power.is_symbolic(),
                right_power.is_symbolic(),
                "integral ordering requires the same symbolic coordinate pattern"
            );
            let (l, r) = (
                i64::from(left_power.value()),
                i64::from(right_power.value()),
            );
            if left_power.is_symbolic() {
                if self.sector[index] {
                    absolute_left += l;
                    absolute_right += r;
                    if self.deltas[index] && delta_order == Ordering::Equal {
                        delta_order = r.cmp(&l);
                    }
                } else {
                    assert!(
                        !self.deltas[index],
                        "a symbolic numerator cannot be a delta"
                    );
                    absolute_left -= l;
                    absolute_right -= r;
                    numerator_left -= l;
                    numerator_right -= r;
                }
            } else {
                denominators_left += usize::from(l > 0);
                denominators_right += usize::from(r > 0);
                if sector_lex == Ordering::Equal {
                    sector_lex = (l > 0).cmp(&(r > 0));
                }
                absolute_left += l.abs();
                absolute_right += r.abs();
                numerator_left += if l <= 0 { -l } else { 0 };
                numerator_right += if r <= 0 { -r } else { 0 };
                if self.deltas[index] && delta_order == Ordering::Equal {
                    delta_order = r.abs().cmp(&l.abs());
                }
            }
        }

        let aggregate = denominators_right
            .cmp(&denominators_left)
            .then(sector_lex.reverse())
            .then(delta_order)
            .then(absolute_right.cmp(&absolute_left))
            .then(numerator_right.cmp(&numerator_left));
        if aggregate != Ordering::Equal {
            return aggregate;
        }

        // Dispatch once, preserving the ordinary contiguous loop for the
        // identity lane instead of testing for an override per coordinate.
        match &self.permutation {
            Some(permutation) => {
                self.compare_coordinate_ties(left, right, permutation.iter().copied())
            }
            None => self.compare_coordinate_ties(left, right, 0..N),
        }
    }

    fn compare_coordinate_ties(
        &self,
        left: &Integral<N>,
        right: &Integral<N>,
        coordinates: impl Iterator<Item = usize> + Clone,
    ) -> Ordering {
        // Aggregate equality implies that numeric sectors agree coordinate-wise.
        // Denominator ties use increasing power, numerator ties decreasing power.
        for index in coordinates.clone() {
            let is_denominator = if left[index].is_symbolic() {
                self.sector[index]
            } else {
                left[index].value() > 0
            };
            if is_denominator {
                let order = left[index].value().cmp(&right[index].value());
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
        for index in coordinates {
            let is_numerator = if left[index].is_symbolic() {
                !self.sector[index]
            } else {
                left[index].value() <= 0
            };
            if is_numerator {
                let order = right[index].value().cmp(&left[index].value());
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
        Ordering::Equal
    }
}

#[cfg(test)]
#[path = "index/permutation_tests.rs"]
mod permutation_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powers_round_trip_all_encodings_and_are_one_byte() {
        assert_eq!(std::mem::size_of::<Power>(), 1);
        assert_eq!(std::mem::size_of::<Integral<17>>(), 17);
        assert_eq!(std::mem::align_of::<Integral<17>>(), 1);
        for symbolic in [false, true] {
            for value in Power::MIN..=Power::MAX {
                let power = Power::new(symbolic, value).unwrap();
                assert_eq!(power.value(), value);
                assert_eq!(power.is_symbolic(), symbolic);
            }
        }
    }

    #[test]
    fn constructors_and_shifts_reject_overflow() {
        assert!(Power::new(false, -65).is_err());
        assert!(Power::new(true, 64).is_err());
        assert!(Power::new(true, 63).unwrap().shifted(1).is_err());
        assert!(Power::new(false, -64).unwrap().shifted(-1).is_err());
        assert!(Power::new(false, 63).unwrap().shifted(i16::MAX).is_err());
        assert!(Power::new(true, -64).unwrap().shifted(i16::MIN).is_err());
        assert!(Integral::symbolic([0, 64]).is_err());
        assert!(Integral::numeric([-65, 0]).is_err());
        let initial = Integral::symbolic([63, -64]).unwrap();
        assert!(initial.shifted([1, 0]).is_err());
        assert_eq!(initial, Integral::symbolic([63, -64]).unwrap());
        assert_eq!(
            Integral::numeric([1]).unwrap().shifted([1]),
            Err(PowerError::NumericShift { index: 0 })
        );
    }

    #[test]
    fn integral_order_matches_each_cpp_comparison_priority() {
        let numeric = Integral::<3>::numeric;
        let ordinary = IntegralOrder::new([true, true, false], [false; 3]);
        for (hard, easy) in [
            ([1, 1, 0], [8, 0, -8]),    // denominator count before all powers
            ([1, 0, 0], [0, 9, -9]),    // inverse sector lexicographic order
            ([2, 1, -2], [1, 1, -2]),   // total absolute power
            ([1, 1, -2], [2, 1, -1]),   // numerator total before ties
            ([1, 2, -1], [2, 1, -1]),   // denominator tie is increasing
            ([1, -1, -2], [1, -2, -1]), // numerator tie is decreasing
        ] {
            let (hard, easy) = (numeric(hard).unwrap(), numeric(easy).unwrap());
            assert_eq!(ordinary.compare(&hard, &easy), Ordering::Less);
            assert_eq!(ordinary.compare(&easy, &hard), Ordering::Greater);
            assert_eq!(ordinary.compare(&hard, &hard), Ordering::Equal);
        }
        let cut = IntegralOrder::new([true, true, false], [true, false, false]);
        assert_eq!(
            cut.compare(&numeric([2, 1, 0]).unwrap(), &numeric([1, 9, -9]).unwrap()),
            Ordering::Less
        );
        // Numeric cut ordering uses absolute power, including nonpositive cuts.
        assert_eq!(
            cut.compare(
                &numeric([-2, 1, 0]).unwrap(),
                &numeric([-1, 9, -9]).unwrap()
            ),
            Ordering::Less
        );
    }

    #[test]
    fn symbolic_totals_use_sector_sign_without_absolute_values() {
        let order = IntegralOrder::new([true, false], [false; 2]);
        let symbolic = Integral::<2>::symbolic;
        assert_eq!(
            order.compare(&symbolic([1, 0]).unwrap(), &symbolic([-2, 0]).unwrap()),
            Ordering::Less
        );
        assert_eq!(
            order.compare(&symbolic([0, -1]).unwrap(), &symbolic([0, 2]).unwrap()),
            Ordering::Less
        );
        let cut = IntegralOrder::new([true, false], [true, false]);
        assert_eq!(
            cut.compare(&symbolic([-1, 9]).unwrap(), &symbolic([-2, -9]).unwrap()),
            Ordering::Less
        );
    }

    #[test]
    fn mixed_order_matches_compiled_spired_fixture() {
        // Generated with the vendored integral.hpp / intLessStatic<3>::compare
        // compiled by GCC 14.4.0. Sample all encoded powers, mixed coordinate
        // patterns, sectors, and admissible cuts with this deterministic stream.
        let mut random = 0x5245_5350_4952_4544u64;
        let mut next = || {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            random
        };
        let mut hash = 14_695_981_039_346_656_037u64;
        for _ in 0..20_000 {
            let flags = next();
            let mut sector = [false; 3];
            let mut deltas = [false; 3];
            let mut left = [Power::default(); 3];
            let mut right = [Power::default(); 3];
            for index in 0..3 {
                let symbolic = (flags >> index) & 1 != 0;
                sector[index] = (flags >> (index + 3)) & 1 != 0;
                deltas[index] = (flags >> (index + 6)) & 1 != 0 && (!symbolic || sector[index]);
                left[index] = Power::new(symbolic, ((next() >> 32) & 127) as i16 - 64).unwrap();
                right[index] = Power::new(symbolic, ((next() >> 32) & 127) as i16 - 64).unwrap();
            }
            let order = IntegralOrder::new(sector, deltas)
                .compare(&Integral::new(left), &Integral::new(right));
            let byte = match order {
                Ordering::Less => 0,
                Ordering::Equal => 1,
                Ordering::Greater => 2,
            };
            hash = (hash ^ byte).wrapping_mul(1_099_511_628_211);
        }
        assert_eq!(hash, 9_141_143_126_569_818_431);
    }
}
