use crate::{Integral, LinearCombination, VacuumFamily};

#[derive(Clone, Debug)]
pub struct IbpIdentity {
    pub seed: Integral,
    pub differentiated_loop: usize,
    pub contraction_loop: usize,
    pub equation: LinearCombination,
}

pub struct IbpGenerator<'family> {
    family: &'family VacuumFamily,
}

impl<'family> IbpGenerator<'family> {
    pub fn new(family: &'family VacuumFamily) -> Self {
        Self { family }
    }

    /// Generate all `L^2` identities `d/dk_i . k_j` for one seed integral.
    pub fn generate(&self, seed: &Integral) -> Vec<IbpIdentity> {
        self.try_generate(seed)
            .expect("integral exponents must remain representable while generating an IBP")
    }

    /// Checked variant of [`Self::generate`].
    pub fn try_generate(&self, seed: &Integral) -> Result<Vec<IbpIdentity>, IbpGenerationError> {
        self.generate_impl(seed, true)
    }

    fn generate_impl(
        &self,
        seed: &Integral,
        canonicalize: bool,
    ) -> Result<Vec<IbpIdentity>, IbpGenerationError> {
        if seed.powers().len() != self.family.denominator_count() {
            return Err(IbpGenerationError::WrongSeedLength {
                expected: self.family.denominator_count(),
                actual: seed.powers().len(),
            });
        }
        let seed = if canonicalize {
            self.family
                .canonicalize(seed)
                .unwrap_or_else(|| seed.clone())
        } else {
            seed.clone()
        };
        let mut identities = Vec::with_capacity(self.family.loops() * self.family.loops());
        for differentiated_loop in 0..self.family.loops() {
            for contraction_loop in 0..self.family.loops() {
                identities.push(self.generate_identity_impl(
                    &seed,
                    differentiated_loop,
                    contraction_loop,
                    canonicalize,
                )?);
            }
        }
        Ok(identities)
    }

    fn generate_identity_impl(
        &self,
        seed: &Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
        canonicalize: bool,
    ) -> Result<IbpIdentity, IbpGenerationError> {
        let mut equation = LinearCombination::new();
        if differentiated_loop == contraction_loop {
            self.add_term(
                &mut equation,
                seed.clone(),
                self.family.dimension().clone(),
                canonicalize,
            );
        }

        for (denominator, &power) in seed.powers().iter().enumerate() {
            if power == 0 {
                continue;
            }
            let contraction = self.family.derivative_contraction(
                denominator,
                differentiated_loop,
                contraction_loop,
            );
            // Lift the derivative multiplicity through i64 rather than
            // negating i32::MIN in the exponent type.
            let power_factor = self.family.coefficients().integer(-i64::from(power));

            if !contraction.constant.is_zero() {
                let coefficient = &contraction.constant * &power_factor;
                self.add_term(
                    &mut equation,
                    self.shift_seed(seed, &[(denominator, 1)])?,
                    coefficient,
                    canonicalize,
                );
            }

            for (cancelled_denominator, &kinematic_coefficient) in
                contraction.denominator_coefficients.iter().enumerate()
            {
                if kinematic_coefficient.is_zero() {
                    continue;
                }
                let rational = self.family.coefficients().rational(kinematic_coefficient);
                let coefficient = &rational * &power_factor;
                self.add_term(
                    &mut equation,
                    self.shift_seed(seed, &[(denominator, 1), (cancelled_denominator, -1)])?,
                    coefficient,
                    canonicalize,
                );
            }
        }

        Ok(IbpIdentity {
            seed: seed.clone(),
            differentiated_loop,
            contraction_loop,
            equation,
        })
    }

    /// Generate identities without applying sector symmetries or zero rules.
    ///
    /// This is primarily a generator-oracle/debugging surface: it exposes
    /// every derivative factor and index shift before canonicalization can
    /// combine terms. Production elimination should use [`Self::generate`].
    pub fn generate_raw(&self, seed: &Integral) -> Vec<IbpIdentity> {
        self.try_generate_raw(seed)
            .expect("integral exponents must remain representable while generating an IBP")
    }

    /// Checked variant of [`Self::generate_raw`].
    pub fn try_generate_raw(
        &self,
        seed: &Integral,
    ) -> Result<Vec<IbpIdentity>, IbpGenerationError> {
        self.generate_impl(seed, false)
    }

    /// Generate exactly one uncanonicalized identity `d/dk_i . k_j`.
    ///
    /// Unlike filtering [`Self::try_generate_raw`], this constructs no
    /// unselected rows.  Bounded analytic recurrences should use this surface
    /// after preflighting precisely the selected shifts.
    pub fn try_generate_raw_identity(
        &self,
        seed: &Integral,
        differentiated_loop: usize,
        contraction_loop: usize,
    ) -> Result<IbpIdentity, IbpGenerationError> {
        if seed.powers().len() != self.family.denominator_count() {
            return Err(IbpGenerationError::WrongSeedLength {
                expected: self.family.denominator_count(),
                actual: seed.powers().len(),
            });
        }
        if differentiated_loop >= self.family.loops() {
            return Err(IbpGenerationError::DifferentiatedLoopOutOfRange {
                requested: differentiated_loop,
                loops: self.family.loops(),
            });
        }
        if contraction_loop >= self.family.loops() {
            return Err(IbpGenerationError::ContractionLoopOutOfRange {
                requested: contraction_loop,
                loops: self.family.loops(),
            });
        }
        self.generate_identity_impl(seed, differentiated_loop, contraction_loop, false)
    }

    pub fn generate_for_seeds(&self, seeds: &[Integral]) -> Vec<IbpIdentity> {
        self.try_generate_for_seeds(seeds)
            .expect("integral exponents must remain representable while generating IBPs")
    }

    pub fn try_generate_for_seeds(
        &self,
        seeds: &[Integral],
    ) -> Result<Vec<IbpIdentity>, IbpGenerationError> {
        let mut identities = Vec::new();
        for seed in seeds {
            identities.extend(self.try_generate(seed)?);
        }
        Ok(identities)
    }

    fn add_term(
        &self,
        equation: &mut LinearCombination,
        integral: Integral,
        coefficient: crate::Coefficient,
        canonicalize: bool,
    ) {
        if canonicalize {
            if let Some(integral) = self.family.canonicalize(&integral) {
                equation.add_term(integral, coefficient);
            }
        } else {
            equation.add_term(integral, coefficient);
        }
    }

    fn shift_seed(
        &self,
        seed: &Integral,
        shifts: &[(usize, i32)],
    ) -> Result<Integral, IbpGenerationError> {
        seed.checked_shifted(shifts)
            .ok_or_else(|| IbpGenerationError::ExponentOverflow {
                seed: seed.clone(),
                shifts: shifts.to_vec(),
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IbpGenerationError {
    WrongSeedLength {
        expected: usize,
        actual: usize,
    },
    DifferentiatedLoopOutOfRange {
        requested: usize,
        loops: usize,
    },
    ContractionLoopOutOfRange {
        requested: usize,
        loops: usize,
    },
    ExponentOverflow {
        seed: Integral,
        shifts: Vec<(usize, i32)>,
    },
}

impl std::fmt::Display for IbpGenerationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSeedLength { expected, actual } => write!(
                formatter,
                "IBP seed has {actual} exponents, expected {expected}"
            ),
            Self::DifferentiatedLoopOutOfRange { requested, loops } => write!(
                formatter,
                "differentiated loop {requested} is outside a {loops}-loop family"
            ),
            Self::ContractionLoopOutOfRange { requested, loops } => write!(
                formatter,
                "contraction loop {requested} is outside a {loops}-loop family"
            ),
            Self::ExponentOverflow { seed, shifts } => write!(
                formatter,
                "IBP index shift {shifts:?} is outside the i32 exponent range for {seed}"
            ),
        }
    }
}

impl std::error::Error for IbpGenerationError {}
