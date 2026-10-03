//! Seeded per-sector verdicts with memoized subsector Euler counts.

use std::collections::HashMap;
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::poly::PolyVariable;

use crate::algebra::Coefficient;
use crate::algebra::matrix::determinant_of_coefficient_matrix;
use crate::family::symanzik::{FeynmanPolynomialLimits, SymanzikPolynomials};
use crate::family::{IntegralFamily, symbolica_matrix_limits};
use crate::sector::Mask;
use crate::sector::zero::{Analyzer, Decision};

use super::error::MasterCountError;
use super::ideal::SectorPolynomial;
use super::model::{MasterCount, MasterCountOptions, NoVerdictReason};
use super::sample::{Kinematics, SampleStream};
use super::staircase::Staircase;

/// Largest active-denominator count of one sector; its Euler sum visits
/// `2^active - 1` faces.
const MAX_ACTIVE_DENOMINATORS: usize = 30;

/// Largest number of samples; each owns a distinct prime and point stream.
pub const MAX_SAMPLES: usize = 1024;

const SATURATION_VARIABLE: &str = "rustred::master_count_t";

/// Counts master integrals per top sector of one family, without symmetries.
///
/// The counter owns the exact `G = U + F`, the zero-sector analyzer, one
/// seeded point stream per sample, and memoized subsector Euler counts and
/// verdicts. Results are deterministic for a fixed seed and call sequence.
#[derive(Debug)]
pub struct MasterCounter {
    kinematics: Kinematics,
    scaleless: Scaleless,
    feynman_variables: Arc<Vec<PolyVariable>>,
    saturation: PolyVariable,
    gram_singular: bool,
    max_attempts: usize,
    streams: Vec<SampleStream>,
    verdicts: HashMap<Mask, MasterCount>,
}

/// Memoized exact scalelessness of sectors and Euler faces.
#[derive(Debug)]
struct Scaleless {
    analyzer: Analyzer,
    memo: HashMap<Mask, bool>,
}

/// Counts of one sector at one sample point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SampleOutcome {
    morse: Option<usize>,
    euler: i64,
}

impl SampleOutcome {
    /// Whether this sample alone exhibits `reason`.
    fn exhibits(&self, reason: NoVerdictReason) -> bool {
        match reason {
            NoVerdictReason::MorseNonIsolated => self.morse.is_none(),
            NoVerdictReason::NegativeEuler => self.euler < 0,
            NoVerdictReason::MorseEulerMismatch => {
                self.morse.and_then(|morse| i64::try_from(morse).ok()) != Some(self.euler)
            }
            NoVerdictReason::GramSingular | NoVerdictReason::SampleDisagreement => false,
        }
    }
}

impl MasterCounter {
    /// Build `G = U + F` and the zero-sector analyzer and decide whether the
    /// external Gram determinant vanishes identically. Nonzero power shifts
    /// and sample counts outside `2..=MAX_SAMPLES` are rejected. No sample
    /// point is drawn before a sector needs one.
    pub fn try_new(
        family: &IntegralFamily,
        options: MasterCountOptions,
    ) -> Result<Self, MasterCountError> {
        if !(2..=MAX_SAMPLES).contains(&options.samples) {
            return Err(MasterCountError::InvalidSampleCount {
                samples: options.samples,
            });
        }
        if let Some(denominator) = family
            .power_shifts()
            .iter()
            .position(|shift| !shift.is_zero())
        {
            return Err(MasterCountError::UnsupportedNonzeroPowerShift { denominator });
        }
        let symanzik = SymanzikPolynomials::try_from_family_with_limits(
            family,
            FeynmanPolynomialLimits::default(),
        )?;
        let analyzer = Analyzer::try_unrestricted(family)?;
        let (gram_singular, gram_determinant) = gram_determinant(family)?;
        let kinematics = Kinematics {
            parameters: family.coefficient_context().parameter_names().len(),
            denominators: family.denominator_count(),
            g: symanzik
                .g()
                .terms()
                .map(|(coefficient, exponents)| (coefficient.clone(), exponents.into()))
                .collect(),
            conditions: analyzer
                .domain()
                .conditions()
                .iter()
                .map(|condition| condition.polynomial().clone())
                .collect(),
            gram_determinant,
        };
        Ok(Self {
            kinematics,
            scaleless: Scaleless {
                analyzer,
                memo: HashMap::new(),
            },
            feynman_variables: symanzik.g().raw().variables().clone(),
            saturation: saturation_variable()?,
            gram_singular,
            max_attempts: options.max_attempts,
            streams: SampleStream::seeded(options.seed, options.samples),
            verdicts: HashMap::new(),
        })
    }

    /// Masters whose top sector is `sector`.
    pub fn count(&mut self, sector: &Mask) -> Result<MasterCount, MasterCountError> {
        if sector.arity() != self.kinematics.denominators {
            return Err(MasterCountError::WrongArity {
                expected: self.kinematics.denominators,
                actual: sector.arity(),
            });
        }
        if let Some(verdict) = self.verdicts.get(sector) {
            return Ok(verdict.clone());
        }
        let verdict = self.count_uncached(sector)?;
        self.verdicts.insert(sector.clone(), verdict.clone());
        Ok(verdict)
    }

    fn count_uncached(&mut self, sector: &Mask) -> Result<MasterCount, MasterCountError> {
        if self.scaleless.check(&self.kinematics, sector)? {
            return Ok(MasterCount::Zero);
        }
        let active = active_positions(sector);
        if active.len() > MAX_ACTIVE_DENOMINATORS {
            return Err(MasterCountError::SectorTooLarge {
                active: active.len(),
                limit: MAX_ACTIVE_DENOMINATORS,
            });
        }
        // A singular Gram matrix decides the verdict; sampling would only cost.
        if self.gram_singular {
            return Ok(MasterCount::NoVerdict {
                morse: None,
                euler: None,
                reason: NoVerdictReason::GramSingular,
            });
        }
        let mut outcomes = Vec::with_capacity(self.streams.len());
        for stream in 0..self.streams.len() {
            self.streams[stream].reset_attempts();
            outcomes.push(self.sample(stream, &active)?);
        }
        Ok(verdict(&outcomes))
    }

    /// Morse and Moebius-inverted Euler counts of one sector at one sample,
    /// redrawing the sample point whenever an Euler face is unlucky.
    fn sample(
        &mut self,
        stream: usize,
        active: &[usize],
    ) -> Result<SampleOutcome, MasterCountError> {
        loop {
            if let Some(outcome) = self.try_sample(stream, active)? {
                return Ok(outcome);
            }
            self.streams[stream].discard();
        }
    }

    /// `None` when a torus Euler ideal is positive-dimensional, which for
    /// generic exponents `u` cannot happen (Huh): the point is unlucky.
    fn try_sample(
        &mut self,
        stream: usize,
        active: &[usize],
    ) -> Result<Option<SampleOutcome>, MasterCountError> {
        let Self {
            kinematics,
            scaleless,
            feynman_variables,
            saturation,
            max_attempts,
            streams,
            ..
        } = self;
        let (field, point) = streams[stream].point(kinematics, *max_attempts)?;
        let restrict = |face: &[usize]| {
            SectorPolynomial::restrict(field, &point.g, face, feynman_variables, saturation)
        };
        let morse = match restrict(active).morse()? {
            Staircase::Finite(count) => Some(count),
            Staircase::Infinite => None,
        };

        let mut euler = 0_i64;
        // Nonempty faces T of S as bit subsets of the active positions;
        // MAX_ACTIVE_DENOMINATORS keeps the shift in range.
        for subset in 1_u64..(1_u64 << active.len()) {
            let face: Vec<usize> = active
                .iter()
                .enumerate()
                .filter(|(bit, _)| subset >> bit & 1 == 1)
                .map(|(_, &position)| position)
                .collect();
            let mask = Mask::try_new(
                (0..kinematics.denominators).map(|position| face.contains(&position)),
            )?;
            let count = match point.euler.get(&mask) {
                Some(&count) => count,
                None => {
                    let count = if scaleless.check(kinematics, &mask)? {
                        0
                    } else {
                        let exponents: Vec<_> = face
                            .iter()
                            .map(|&position| point.exponents[position])
                            .collect();
                        match restrict(&face).euler(&exponents)? {
                            Staircase::Finite(count) => count,
                            Staircase::Infinite => return Ok(None),
                        }
                    };
                    point.euler.insert(mask, count);
                    count
                }
            };
            // Counts are below MAX_QUOTIENT_DIMENSION and there are fewer than
            // 2^MAX_ACTIVE_DENOMINATORS faces, so the sum cannot overflow.
            let count = count as i64;
            if (active.len() - face.len()).is_multiple_of(2) {
                euler += count;
            } else {
                euler -= count;
            }
        }
        Ok(Some(SampleOutcome { morse, euler }))
    }
}

impl Scaleless {
    /// `G_T` vanishes identically or the zero-sector rank test proves the
    /// sector zero.
    fn check(&mut self, kinematics: &Kinematics, sector: &Mask) -> Result<bool, MasterCountError> {
        if let Some(&scaleless) = self.memo.get(sector) {
            return Ok(scaleless);
        }
        let supported = kinematics.g.iter().any(|(_, exponents)| {
            exponents
                .iter()
                .zip(sector.active_bits())
                .all(|(&exponent, &active)| exponent == 0 || active)
        });
        let scaleless =
            !supported || matches!(self.analyzer.analyze(sector)?, Decision::ProvedZero(_));
        self.memo.insert(sector.clone(), scaleless);
        Ok(scaleless)
    }
}

/// Combine the samples into a verdict, reporting the most specific reason
/// and the diagnostics of the first sample exhibiting it.
fn verdict(outcomes: &[SampleOutcome]) -> MasterCount {
    let no_verdict = |outcome: &SampleOutcome, reason| MasterCount::NoVerdict {
        morse: outcome.morse,
        euler: Some(outcome.euler),
        reason,
    };
    let Some(first) = outcomes.first() else {
        // Unreachable: the counter requires at least two samples.
        return MasterCount::NoVerdict {
            morse: None,
            euler: None,
            reason: NoVerdictReason::SampleDisagreement,
        };
    };
    for reason in [
        NoVerdictReason::MorseNonIsolated,
        NoVerdictReason::NegativeEuler,
        NoVerdictReason::MorseEulerMismatch,
    ] {
        if let Some(outcome) = outcomes.iter().find(|outcome| outcome.exhibits(reason)) {
            return no_verdict(outcome, reason);
        }
    }
    match first.morse {
        Some(count) if outcomes.iter().all(|outcome| outcome == first) => {
            MasterCount::Counted(count)
        }
        _ => no_verdict(first, NoVerdictReason::SampleDisagreement),
    }
}

fn active_positions(sector: &Mask) -> Vec<usize> {
    sector
        .active_bits()
        .iter()
        .enumerate()
        .filter_map(|(position, &active)| active.then_some(position))
        .collect()
}

/// Exact external Gram determinant: whether it vanishes identically, and the
/// determinant a sample must keep nonzero otherwise. A vacuum family has no
/// external constraint.
fn gram_determinant(
    family: &IntegralFamily,
) -> Result<(bool, Option<Coefficient>), MasterCountError> {
    if family.external_count() == 0 {
        return Ok((false, None));
    }
    let (determinant, _stats) = determinant_of_coefficient_matrix(
        family.coefficient_context(),
        family.external_gram(),
        symbolica_matrix_limits(family.construction_limits()),
    )
    .map_err(|error| MasterCountError::GramDeterminant {
        detail: error.to_string(),
    })?;
    Ok(if determinant.is_zero() {
        (true, None)
    } else {
        (false, Some(determinant))
    })
}

/// The Rabinowitsch variable `t` of `1 - t G`. It only ever shares a ring
/// with Feynman parameters, whose names differ.
fn saturation_variable() -> Result<PolyVariable, MasterCountError> {
    let namespaced = NamespacedSymbol::try_parse(SATURATION_VARIABLE).ok_or_else(|| {
        MasterCountError::SaturationSymbol {
            detail: "invalid namespaced symbol".to_owned(),
        }
    })?;
    let symbol = SymbolBuilder::new(namespaced).build().map_err(|error| {
        MasterCountError::SaturationSymbol {
            detail: error.to_string(),
        }
    })?;
    Ok(PolyVariable::Symbol(symbol))
}

#[cfg(test)]
mod tests {
    use super::{MasterCount, NoVerdictReason, SampleOutcome, verdict};

    const fn outcome(morse: Option<usize>, euler: i64) -> SampleOutcome {
        SampleOutcome { morse, euler }
    }

    fn reason(outcomes: &[SampleOutcome]) -> Option<NoVerdictReason> {
        match verdict(outcomes) {
            MasterCount::NoVerdict { reason, .. } => Some(reason),
            _ => None,
        }
    }

    #[test]
    fn agreeing_samples_count() {
        assert_eq!(
            verdict(&[outcome(Some(4), 4), outcome(Some(4), 4)]),
            MasterCount::Counted(4)
        );
        assert_eq!(
            verdict(&[outcome(Some(0), 0), outcome(Some(0), 0)]),
            MasterCount::Counted(0)
        );
    }

    #[test]
    fn reasons_follow_specificity() {
        let good = outcome(Some(2), 2);
        assert_eq!(
            reason(&[good, outcome(None, -1)]),
            Some(NoVerdictReason::MorseNonIsolated)
        );
        assert_eq!(
            reason(&[outcome(Some(1), 3), outcome(Some(1), -1)]),
            Some(NoVerdictReason::NegativeEuler)
        );
        assert_eq!(
            reason(&[good, outcome(Some(1), 3)]),
            Some(NoVerdictReason::MorseEulerMismatch)
        );
        assert_eq!(
            reason(&[good, outcome(Some(3), 3)]),
            Some(NoVerdictReason::SampleDisagreement)
        );
    }

    #[test]
    fn diagnostics_come_from_the_offending_sample() {
        assert_eq!(
            verdict(&[outcome(Some(2), 2), outcome(None, 5)]),
            MasterCount::NoVerdict {
                morse: None,
                euler: Some(5),
                reason: NoVerdictReason::MorseNonIsolated,
            }
        );
    }
}
