use std::sync::Arc;

use super::error::{Error, try_collect_vec, try_reserve_exact};
use super::mask::Mask;

/// Inclusive representable bounds for one coordinate of a sector interior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InteriorBounds {
    lower: i64,
    upper: i64,
}

impl InteriorBounds {
    pub const fn new(lower: i64, upper: i64) -> Self {
        Self { lower, upper }
    }

    pub const fn lower(self) -> i64 {
        self.lower
    }

    pub const fn upper(self) -> i64 {
        self.upper
    }

    pub const fn contains(self, value: i64) -> bool {
        self.lower <= value && value <= self.upper
    }
}

/// A fixed sector and a nonempty, closed i64 interior in every coordinate.
///
/// The intervals lie wholly inside the sector. They may additionally be
/// tightened so that translating every point by one of a finite collection of
/// shifts remains representable and in the same sector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectorInteriorDomain {
    sector: Mask,
    bounds: Arc<Vec<InteriorBounds>>,
}

impl SectorInteriorDomain {
    /// Construct an explicitly bounded interior, validating arity, nonemptiness,
    /// and membership of every interval in the fixed sector.
    pub fn try_new(
        sector: Mask,
        bounds: impl IntoIterator<Item = InteriorBounds>,
    ) -> Result<Self, Error> {
        let bounds = try_collect_vec(bounds, "sector interior bounds")?;
        if bounds.len() != sector.arity() {
            return Err(Error::WrongArity {
                expected: sector.arity(),
                actual: bounds.len(),
            });
        }
        for (position, (&active, bounds)) in
            sector.active_bits().iter().zip(bounds.iter()).enumerate()
        {
            if bounds.lower > bounds.upper {
                return Err(Error::InvalidInteriorBounds {
                    position,
                    lower: bounds.lower,
                    upper: bounds.upper,
                });
            }
            if (active && bounds.lower < 1) || (!active && bounds.upper > 0) {
                return Err(Error::InteriorOutsideSector {
                    position,
                    active,
                    lower: bounds.lower,
                    upper: bounds.upper,
                });
            }
        }
        Ok(Self {
            sector,
            bounds: Arc::new(bounds),
        })
    }

    /// Build the largest representable interior on which every supplied shift
    /// preserves the fixed sector.
    ///
    /// Each coordinate starts at `[1, i64::MAX]` when active and
    /// `[i64::MIN, 0]` when inactive. For a shift `s`, it is intersected with
    /// the translated preimage of that same interval. All endpoint arithmetic
    /// is performed in i128 before conversion back to a nonempty i64 interval.
    pub fn try_maximal_for_shifts<S>(sector: Mask, shifts: &[S]) -> Result<Self, Error>
    where
        S: AsRef<[i64]>,
    {
        for shift in shifts {
            let actual = shift.as_ref().len();
            if actual != sector.arity() {
                return Err(Error::WrongArity {
                    expected: sector.arity(),
                    actual,
                });
            }
        }

        let mut bounds = Vec::new();
        try_reserve_exact(&mut bounds, sector.arity(), "sector interior bounds")?;
        for (position, &active) in sector.active_bits().iter().enumerate() {
            let sector_lower = if active { 1_i64 } else { i64::MIN };
            let sector_upper = if active { i64::MAX } else { 0_i64 };
            let mut lower = i128::from(sector_lower);
            let mut upper = i128::from(sector_upper);
            for shift in shifts {
                let shift = i128::from(shift.as_ref()[position]);
                lower = lower.max(i128::from(sector_lower) - shift);
                upper = upper.min(i128::from(sector_upper) - shift);
            }
            lower = lower.max(i128::from(i64::MIN));
            upper = upper.min(i128::from(i64::MAX));
            if lower > upper {
                return Err(Error::EmptyShiftInterior { position });
            }
            let lower = i64::try_from(lower).map_err(|_| Error::EmptyShiftInterior { position })?;
            let upper = i64::try_from(upper).map_err(|_| Error::EmptyShiftInterior { position })?;
            bounds.push(InteriorBounds::new(lower, upper));
        }
        Self::try_new(sector, bounds)
    }

    pub fn sector(&self) -> &Mask {
        &self.sector
    }

    pub fn arity(&self) -> usize {
        self.sector.arity()
    }

    pub fn bounds(&self) -> &[InteriorBounds] {
        self.bounds.as_slice()
    }

    pub fn bound(&self, position: usize) -> Result<InteriorBounds, Error> {
        self.bounds
            .get(position)
            .copied()
            .ok_or(Error::IndexOutOfRange {
                position,
                arity: self.arity(),
            })
    }

    /// Whether an i64 lattice point is an anchor inside this interior.
    pub fn contains(&self, indices: &[i64]) -> Result<bool, Error> {
        self.check_arity(indices.len())?;
        Ok(self
            .bounds
            .iter()
            .zip(indices)
            .all(|(&bounds, &index)| bounds.contains(index)))
    }

    /// Whether every point in this domain remains representable and in the
    /// fixed sector after the supplied shift.
    pub fn covers_shift(&self, shift: &[i64]) -> Result<bool, Error> {
        self.check_arity(shift.len())?;
        Ok(self
            .bounds
            .iter()
            .zip(shift)
            .zip(self.sector.active_bits())
            .all(|((&bounds, &shift), &active)| {
                let translated_lower = i128::from(bounds.lower) + i128::from(shift);
                let translated_upper = i128::from(bounds.upper) + i128::from(shift);
                let sector_lower = if active { 1_i64 } else { i64::MIN };
                let sector_upper = if active { i64::MAX } else { 0_i64 };
                i128::from(sector_lower) <= translated_lower
                    && translated_upper <= i128::from(sector_upper)
            }))
    }

    /// Translate one contained anchor, returning `None` if the anchor is
    /// outside this domain or the translated i64 point does not exist.
    pub fn checked_translate(
        &self,
        anchor: &[i64],
        shift: &[i64],
    ) -> Result<Option<Vec<i64>>, Error> {
        self.check_arity(anchor.len())?;
        self.check_arity(shift.len())?;
        if !self.contains(anchor)? {
            return Ok(None);
        }
        let mut translated = Vec::new();
        try_reserve_exact(
            &mut translated,
            self.arity(),
            "translated sector interior point",
        )?;
        for (&index, &shift) in anchor.iter().zip(shift) {
            let Some(index) = index.checked_add(shift) else {
                return Ok(None);
            };
            translated.push(index);
        }
        if self
            .sector
            .active_bits()
            .iter()
            .zip(&translated)
            .all(|(&active, &index)| active == (index >= 1))
        {
            Ok(Some(translated))
        } else {
            Ok(None)
        }
    }

    pub(super) fn require_shift(&self, shift: &[i64]) -> Result<(), Error> {
        self.check_arity(shift.len())?;
        for (position, ((&bounds, &shift), &active)) in self
            .bounds
            .iter()
            .zip(shift)
            .zip(self.sector.active_bits())
            .enumerate()
        {
            let translated_lower = i128::from(bounds.lower) + i128::from(shift);
            let translated_upper = i128::from(bounds.upper) + i128::from(shift);
            let sector_lower = if active { 1_i64 } else { i64::MIN };
            let sector_upper = if active { i64::MAX } else { 0_i64 };
            if translated_lower < i128::from(sector_lower)
                || translated_upper > i128::from(sector_upper)
            {
                return Err(Error::ShiftNotCovered { position, shift });
            }
        }
        Ok(())
    }

    fn check_arity(&self, actual: usize) -> Result<(), Error> {
        if actual == self.arity() {
            Ok(())
        } else {
            Err(Error::WrongArity {
                expected: self.arity(),
                actual,
            })
        }
    }
}
