use super::error::{Error, try_collect_vec, try_reserve_exact};
use super::mask::Mask;

/// Required-active denominator positions (`CutDs` semantics).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CutConstraint {
    required_active: Mask,
}

impl CutConstraint {
    pub fn none(arity: usize) -> Result<Self, Error> {
        Ok(Self {
            required_active: Mask::try_new(std::iter::repeat_n(false, arity))?,
        })
    }

    pub fn try_new(required_active: impl IntoIterator<Item = bool>) -> Result<Self, Error> {
        Ok(Self {
            required_active: Mask::try_new(required_active)?,
        })
    }

    pub fn try_from_positions(
        arity: usize,
        positions: impl IntoIterator<Item = usize>,
    ) -> Result<Self, Error> {
        if arity == 0 {
            return Err(Error::EmptyIndexSpace);
        }
        let mut required = Vec::new();
        try_reserve_exact(&mut required, arity, "cut active mask")?;
        required.resize(arity, false);
        for position in positions {
            if position >= arity {
                return Err(Error::IndexOutOfRange { position, arity });
            }
            if required[position] {
                return Err(Error::DuplicateIndex { position });
            }
            required[position] = true;
        }
        Ok(Self {
            required_active: Mask::try_from_preallocated(required)?,
        })
    }

    pub fn arity(&self) -> usize {
        self.required_active.arity()
    }

    pub fn required_active(&self) -> &Mask {
        &self.required_active
    }

    pub fn missing_required_active(&self, sector: &Mask) -> Result<Vec<usize>, Error> {
        self.required_active.check_other_arity(sector)?;
        let missing_count = self
            .required_active
            .active
            .iter()
            .zip(&sector.active)
            .filter(|&(&required, &active)| required && !active)
            .count();
        let mut missing = Vec::new();
        try_reserve_exact(&mut missing, missing_count, "missing cut positions")?;
        for (position, (&required, &active)) in self
            .required_active
            .active
            .iter()
            .zip(&sector.active)
            .enumerate()
        {
            if required && !active {
                missing.push(position);
            }
        }
        debug_assert_eq!(missing.len(), missing_count);
        Ok(missing)
    }

    pub fn to_bit_string(&self) -> String {
        self.required_active.to_bit_string()
    }
}

/// One checked `SectorsPattern` slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PatternSlot {
    Any,
    Active,
    Inactive,
}

/// A fixed-arity sector admissibility pattern.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Pattern {
    // Keep the fallibly reserved allocation. Converting to a boxed slice may
    // perform a second proportional shrink allocation.
    slots: Vec<PatternSlot>,
}

impl Pattern {
    pub fn any(arity: usize) -> Result<Self, Error> {
        Self::try_new(std::iter::repeat_n(PatternSlot::Any, arity))
    }

    pub fn try_new(slots: impl IntoIterator<Item = PatternSlot>) -> Result<Self, Error> {
        let slots = try_collect_vec(slots, "sector pattern slots")?;
        Self::try_from_preallocated(slots)
    }

    fn try_from_preallocated(slots: Vec<PatternSlot>) -> Result<Self, Error> {
        if slots.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        Ok(Self { slots })
    }

    /// Parse stable pattern characters: `*` (any), `1` (active), `0`
    /// (inactive).
    pub fn try_from_string(pattern: &str) -> Result<Self, Error> {
        if pattern.is_empty() {
            return Err(Error::EmptyIndexSpace);
        }
        let mut slots = Vec::new();
        try_reserve_exact(&mut slots, pattern.len(), "sector pattern slots")?;
        for (position, byte) in pattern.bytes().enumerate() {
            slots.push(match byte {
                b'*' => PatternSlot::Any,
                b'1' => PatternSlot::Active,
                b'0' => PatternSlot::Inactive,
                _ => return Err(Error::InvalidPatternSlot { position, byte }),
            });
        }
        Self::try_from_preallocated(slots)
    }

    pub fn arity(&self) -> usize {
        self.slots.len()
    }

    pub fn slots(&self) -> &[PatternSlot] {
        &self.slots
    }

    pub fn mismatches(&self, sector: &Mask) -> Result<Vec<PatternMismatch>, Error> {
        if self.arity() != sector.arity() {
            return Err(Error::WrongArity {
                expected: self.arity(),
                actual: sector.arity(),
            });
        }
        let mismatch_count = self
            .slots
            .iter()
            .zip(&sector.active)
            .filter(|&(&required, &actual_active)| !slot_matches(required, actual_active))
            .count();
        let mut mismatches = Vec::new();
        try_reserve_exact(&mut mismatches, mismatch_count, "sector pattern mismatches")?;
        for (position, (&required, &actual_active)) in
            self.slots.iter().zip(&sector.active).enumerate()
        {
            if !slot_matches(required, actual_active) {
                mismatches.push(PatternMismatch {
                    position,
                    required,
                    actual_active,
                });
            }
        }
        debug_assert_eq!(mismatches.len(), mismatch_count);
        Ok(mismatches)
    }

    pub fn to_stable_string(&self) -> String {
        self.slots
            .iter()
            .map(|slot| match slot {
                PatternSlot::Any => '*',
                PatternSlot::Active => '1',
                PatternSlot::Inactive => '0',
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatternMismatch {
    position: usize,
    required: PatternSlot,
    actual_active: bool,
}

impl PatternMismatch {
    pub fn position(self) -> usize {
        self.position
    }

    pub fn required(self) -> PatternSlot {
        self.required
    }

    pub fn actual_active(self) -> bool {
        self.actual_active
    }
}

/// Structured evidence that user constraints excluded a sector.
///
/// This evidence is admissibility metadata, never an integral-zero proof.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Exclusion {
    // Retain the vectors whose capacity was acquired fallibly.
    missing_required_active: Vec<usize>,
    pattern_mismatches: Vec<PatternMismatch>,
}

impl Exclusion {
    pub fn missing_required_active(&self) -> &[usize] {
        &self.missing_required_active
    }

    pub fn pattern_mismatches(&self) -> &[PatternMismatch] {
        &self.pattern_mismatches
    }
}

/// Combined user cuts and sector pattern.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Restrictions {
    cuts: CutConstraint,
    pattern: Pattern,
}

impl Restrictions {
    pub fn unrestricted(arity: usize) -> Result<Self, Error> {
        Self::try_new(CutConstraint::none(arity)?, Pattern::any(arity)?)
    }

    pub fn try_new(cuts: CutConstraint, pattern: Pattern) -> Result<Self, Error> {
        if cuts.arity() != pattern.arity() {
            return Err(Error::WrongArity {
                expected: cuts.arity(),
                actual: pattern.arity(),
            });
        }
        Ok(Self { cuts, pattern })
    }

    pub fn arity(&self) -> usize {
        self.cuts.arity()
    }

    pub fn cuts(&self) -> &CutConstraint {
        &self.cuts
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub fn exclusion(&self, sector: &Mask) -> Result<Option<Exclusion>, Error> {
        let missing_required_active = self.cuts.missing_required_active(sector)?;
        let pattern_mismatches = self.pattern.mismatches(sector)?;
        if missing_required_active.is_empty() && pattern_mismatches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Exclusion {
                missing_required_active,
                pattern_mismatches,
            }))
        }
    }
}

fn slot_matches(required: PatternSlot, actual_active: bool) -> bool {
    match required {
        PatternSlot::Any => true,
        PatternSlot::Active => actual_active,
        PatternSlot::Inactive => !actual_active,
    }
}
