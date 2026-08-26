//! Fully parametric integration-by-parts and Lorentz-invariance identities.
//!
//! The implementation follows LiteRed's `GenerateIBP` convention.  It emits
//! reusable relations in symbolic integral indices, never concrete seed
//! equations, and applies no sector, symmetry, or zero-sector rewriting.

use std::fmt;
use std::sync::Arc;

use crate::generic_family::{
    ContractionMomentum, GenericFamilyError, IntegralFamily, ScalarProductCoordinate,
};
use crate::parallel_execution::ParallelExecution;
use crate::parametric_coefficient::{
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError,
};
use crate::parametric_relation::{
    IndexShift, IndexSpace, ParametricRelation, ParametricRelationError, ParametricRowId,
};

/// Resource policy for coefficient translations used while constructing LI
/// identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ParametricIbpConfig {
    pub arithmetic_limits: ParametricArithmeticLimits,
}

/// Typed failures from generic parametric IBP/LI generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricIbpError {
    BaseContextMismatch,
    WrongIndexArity { expected: usize, actual: usize },
    RowCountOverflow { loops: usize, externals: usize },
    Coefficient(ParametricCoefficientError),
    Relation(ParametricRelationError),
    Family(GenericFamilyError),
}

impl fmt::Display for ParametricIbpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseContextMismatch => formatter.write_str(
                "the parametric coefficient context does not extend the family's exact base map",
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "the parametric context has {actual} indices, expected {expected}"
            ),
            Self::RowCountOverflow { loops, externals } => write!(
                formatter,
                "the IBP/LI row count for {loops} loops and {externals} external momenta overflowed usize"
            ),
            Self::Coefficient(error) => error.fmt(formatter),
            Self::Relation(error) => error.fmt(formatter),
            Self::Family(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricIbpError {}

impl From<ParametricCoefficientError> for ParametricIbpError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

impl From<ParametricRelationError> for ParametricIbpError {
    fn from(value: ParametricRelationError) -> Self {
        Self::Relation(value)
    }
}

impl From<GenericFamilyError> for ParametricIbpError {
    fn from(value: GenericFamilyError) -> Self {
        Self::Family(value)
    }
}

/// Generated relations with their exact authenticated `K(n)` context.
#[derive(Clone, Debug)]
pub struct ParametricIbpRelations {
    family_fingerprint: Arc<str>,
    context: ParametricCoefficientContext,
    ordinary_ibp: Vec<ParametricRelation>,
    lorentz_invariance: Vec<ParametricRelation>,
}

impl ParametricIbpRelations {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context(&self) -> &ParametricCoefficientContext {
        &self.context
    }

    /// The `L*(L+E)` ordinary rows in contraction-major, loop-minor order.
    pub fn ordinary_ibp(&self) -> &[ParametricRelation] {
        &self.ordinary_ibp
    }

    /// The `E*(E-1)/2` LI rows in lexicographic external-pair order.
    pub fn lorentz_invariance(&self) -> &[ParametricRelation] {
        &self.lorentz_invariance
    }

    /// LiteRed's `IBPLI` order: all ordinary rows followed by all LI rows.
    pub fn ibp_li(&self) -> impl Iterator<Item = &ParametricRelation> {
        self.ordinary_ibp
            .iter()
            .chain(self.lorentz_invariance.iter())
    }

    pub fn into_parts(
        self,
    ) -> (
        ParametricCoefficientContext,
        Vec<ParametricRelation>,
        Vec<ParametricRelation>,
    ) {
        (self.context, self.ordinary_ibp, self.lorentz_invariance)
    }
}

/// A topology- and loop-count-independent generator for one complete family.
#[derive(Clone, Debug)]
pub struct ParametricIbpGenerator<'family> {
    family: &'family IntegralFamily,
    family_fingerprint: Arc<str>,
    context: ParametricCoefficientContext,
    index_space: IndexSpace,
    positive_units: Vec<IndexShift>,
    negative_units: Vec<IndexShift>,
    config: ParametricIbpConfig,
}

impl<'family> ParametricIbpGenerator<'family> {
    pub fn try_new(family: &'family IntegralFamily) -> Result<Self, ParametricIbpError> {
        Self::try_new_with_config(family, ParametricIbpConfig::default())
    }

    pub fn try_new_with_config(
        family: &'family IntegralFamily,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        let family_fingerprint: Arc<str> = family.fingerprint().into();
        // The full semantic fingerprint is encoded losslessly by the context
        // constructor.  Thus two distinct family definitions never alias an
        // index-variable identity merely because their display names agree.
        let scope = format!("ordinary-ibp|{family_fingerprint}");
        let context = ParametricCoefficientContext::try_new(
            family.coefficient_context(),
            &scope,
            family.denominator_count(),
        )?;
        Self::try_with_context_and_fingerprint(family, family_fingerprint, context, config)
    }

    /// Construct a generator with a caller-owned exact `K(n)` identity.
    ///
    /// This is useful when relations from several generation stages must use
    /// one shared index scope.  Both the base map and index arity are checked.
    pub fn try_with_context(
        family: &'family IntegralFamily,
        context: ParametricCoefficientContext,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        let family_fingerprint = family.fingerprint().into();
        Self::try_with_context_and_fingerprint(family, family_fingerprint, context, config)
    }

    fn try_with_context_and_fingerprint(
        family: &'family IntegralFamily,
        family_fingerprint: Arc<str>,
        context: ParametricCoefficientContext,
        config: ParametricIbpConfig,
    ) -> Result<Self, ParametricIbpError> {
        if !family
            .coefficient_context()
            .has_same_variable_map(context.base())
        {
            return Err(ParametricIbpError::BaseContextMismatch);
        }
        let arity = family.denominator_count();
        if context.index_count() != arity {
            return Err(ParametricIbpError::WrongIndexArity {
                expected: arity,
                actual: context.index_count(),
            });
        }
        checked_generated_row_counts(family.loop_count(), family.external_count())?;
        let index_space = IndexSpace::try_new(arity)?;
        let positive_units = (0..arity)
            .map(|position| index_space.unit(position, 1))
            .collect::<Result<Vec<_>, _>>()?;
        let negative_units = (0..arity)
            .map(|position| index_space.unit(position, -1))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            family,
            family_fingerprint,
            context,
            index_space,
            positive_units,
            negative_units,
            config,
        })
    }

    pub fn family(&self) -> &IntegralFamily {
        self.family
    }

    pub fn context(&self) -> &ParametricCoefficientContext {
        &self.context
    }

    pub fn config(&self) -> ParametricIbpConfig {
        self.config
    }

    /// Generate the `L*(L+E)` raw ordinary IBPs.
    pub fn generate_ordinary_ibp(&self) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        self.generate_ordinary_ibp_impl(None)
    }

    /// Generate ordinary IBPs under one explicitly bounded execution budget.
    ///
    /// Rows are independent and use their fixed contraction-major ordinal as
    /// the work key.  Results, including failures, are consumed in that order,
    /// so worker completion order cannot change the returned transcript.
    pub fn generate_ordinary_ibp_with_execution(
        &self,
        execution: &ParallelExecution,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        self.generate_ordinary_ibp_impl(Some(execution))
    }

    fn generate_ordinary_ibp_impl(
        &self,
        execution: Option<&ParallelExecution>,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let (ordinary_count, _) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        let dimension = self.context.lift(self.family.dimension())?;
        let powers = (0..self.family.denominator_count())
            .map(|denominator| {
                let index = self.context.index(denominator)?;
                let power_shift = self
                    .context
                    .lift(&self.family.power_shifts()[denominator])?;
                self.context
                    .add_with_limits(
                        &index,
                        &power_shift,
                        self.config.arithmetic_limits.exact_algebra,
                    )
                    .map_err(ParametricIbpError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // LiteRed constructs `Outer[..., qms, lms]` and then flattens it:
        // contraction momentum is the major index, differentiated loop minor.
        let generate = |ordinal| self.generate_ordinary_row(ordinal, &dimension, &powers);
        let staged = match execution {
            Some(execution) => execution.map_ordered(ordinary_count, generate),
            None => (0..ordinary_count).map(generate).collect(),
        };
        let rows = staged.into_iter().collect::<Result<Vec<_>, _>>()?;
        debug_assert_eq!(rows.len(), ordinary_count);
        Ok(rows)
    }

    fn generate_ordinary_row(
        &self,
        ordinal: usize,
        dimension: &ParametricCoefficient,
        powers: &[ParametricCoefficient],
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let loops = self.family.loop_count();
        debug_assert!(loops > 0);
        let contraction_index = ordinal / loops;
        let differentiated_loop = ordinal % loops;
        let contraction = self.family.contraction_momenta()[contraction_index];
        let row_id = ParametricRowId::OrdinaryIbp {
            contraction_momentum: contraction_index,
            differentiated_loop,
        };
        let mut row = self.empty_relation(row_id)?;
        if contraction == ContractionMomentum::Loop(differentiated_loop) {
            row.add_term_with_limits(
                &self.context,
                self.index_space.zero(),
                dimension.clone(),
                self.config.arithmetic_limits,
            )?;
        }

        for (denominator, power) in powers.iter().enumerate() {
            let derivative = self.family.derivative_contraction(
                denominator,
                differentiated_loop,
                contraction,
            )?;
            self.add_negative_derivative_term(
                &mut row,
                self.positive_units[denominator].clone(),
                power,
                derivative.constant(),
            )?;
            for (target, coefficient) in derivative.denominator_coefficients().iter().enumerate() {
                let shift =
                    self.positive_units[denominator].checked_add(&self.negative_units[target])?;
                self.add_negative_derivative_term(&mut row, shift, power, coefficient)?;
            }
        }
        Ok(row)
    }

    /// Generate ordinary and LI rows with their shared authenticated context.
    pub fn generate(&self) -> Result<ParametricIbpRelations, ParametricIbpError> {
        let ordinary_ibp = self.generate_ordinary_ibp()?;
        let lorentz_invariance = self.generate_li_from_ordinary(&ordinary_ibp)?;
        Ok(self.relations(ordinary_ibp, lorentz_invariance))
    }

    /// Generate ordinary and LI rows using one owned execution context.
    ///
    /// The complete ordinary phase is a barrier before LI work begins because
    /// LI rows are exact linear combinations of external-contraction IBPs.
    pub fn generate_with_execution(
        &self,
        execution: &ParallelExecution,
    ) -> Result<ParametricIbpRelations, ParametricIbpError> {
        let ordinary_ibp = self.generate_ordinary_ibp_with_execution(execution)?;
        let lorentz_invariance =
            self.generate_li_from_ordinary_impl(&ordinary_ibp, Some(execution))?;
        Ok(self.relations(ordinary_ibp, lorentz_invariance))
    }

    fn relations(
        &self,
        ordinary_ibp: Vec<ParametricRelation>,
        lorentz_invariance: Vec<ParametricRelation>,
    ) -> ParametricIbpRelations {
        ParametricIbpRelations {
            family_fingerprint: self.family_fingerprint.clone(),
            context: self.context.clone(),
            ordinary_ibp,
            lorentz_invariance,
        }
    }

    /// Generate only the LI rows.  Ordinary external-contraction rows are
    /// derived first, exactly as in LiteRed, and are not returned.
    pub fn generate_lorentz_invariance(
        &self,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let (_, li_count) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        if li_count == 0 {
            return Ok(Vec::new());
        }
        let ordinary = self.generate_ordinary_ibp()?;
        self.generate_li_from_ordinary(&ordinary)
    }

    /// Generate only LI rows under one explicitly bounded execution budget.
    pub fn generate_lorentz_invariance_with_execution(
        &self,
        execution: &ParallelExecution,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let (_, li_count) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        if li_count == 0 {
            return Ok(Vec::new());
        }
        let ordinary = self.generate_ordinary_ibp_with_execution(execution)?;
        self.generate_li_from_ordinary_impl(&ordinary, Some(execution))
    }

    fn generate_li_from_ordinary(
        &self,
        ordinary: &[ParametricRelation],
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        self.generate_li_from_ordinary_impl(ordinary, None)
    }

    fn generate_li_from_ordinary_impl(
        &self,
        ordinary: &[ParametricRelation],
        execution: Option<&ParallelExecution>,
    ) -> Result<Vec<ParametricRelation>, ParametricIbpError> {
        let (_, li_count) =
            checked_generated_row_counts(self.family.loop_count(), self.family.external_count())?;
        let mut pairs = Vec::with_capacity(li_count);
        for first_external in 0..self.family.external_count() {
            for second_external in first_external + 1..self.family.external_count() {
                pairs.push((first_external, second_external));
            }
        }
        let generate = |ordinal| {
            let (first_external, second_external) = pairs[ordinal];
            self.generate_li_row(ordinary, first_external, second_external)
        };
        let staged = match execution {
            Some(execution) => execution.map_ordered(li_count, generate),
            None => (0..li_count).map(generate).collect(),
        };
        let rows = staged.into_iter().collect::<Result<Vec<_>, _>>()?;
        debug_assert_eq!(rows.len(), li_count);
        Ok(rows)
    }

    fn generate_li_row(
        &self,
        ordinary: &[ParametricRelation],
        first_external: usize,
        second_external: usize,
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let row_id = ParametricRowId::LorentzInvariance {
            first_external,
            second_external,
        };
        let mut row = self.empty_relation(row_id.clone())?;
        for differentiated_loop in 0..self.family.loop_count() {
            // M_ba: X_{i b} B_{a i}
            let source_a =
                self.external_ordinary_row(ordinary, first_external, differentiated_loop)?;
            let coordinate_b =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: second_external,
                    })?;
            let multiplier_b = self.family.scalar_product_expansion(coordinate_b)?;
            self.add_weighted_translation(
                &mut row,
                source_a,
                multiplier_b.constant(),
                multiplier_b.denominator_coefficients(),
                false,
                &row_id,
            )?;

            // -M_ab: -X_{i a} B_{b i}
            let source_b =
                self.external_ordinary_row(ordinary, second_external, differentiated_loop)?;
            let coordinate_a =
                self.family
                    .coordinate_index(ScalarProductCoordinate::LoopExternal {
                        loop_index: differentiated_loop,
                        external_index: first_external,
                    })?;
            let multiplier_a = self.family.scalar_product_expansion(coordinate_a)?;
            self.add_weighted_translation(
                &mut row,
                source_b,
                multiplier_a.constant(),
                multiplier_a.denominator_coefficients(),
                true,
                &row_id,
            )?;
        }
        Ok(row)
    }

    fn external_ordinary_row<'rows>(
        &self,
        ordinary: &'rows [ParametricRelation],
        external: usize,
        differentiated_loop: usize,
    ) -> Result<&'rows ParametricRelation, ParametricIbpError> {
        let contraction_index = self.family.loop_count().checked_add(external).ok_or(
            ParametricIbpError::RowCountOverflow {
                loops: self.family.loop_count(),
                externals: self.family.external_count(),
            },
        )?;
        let row = contraction_index
            .checked_mul(self.family.loop_count())
            .and_then(|offset| offset.checked_add(differentiated_loop))
            .and_then(|position| ordinary.get(position))
            .ok_or(ParametricIbpError::RowCountOverflow {
                loops: self.family.loop_count(),
                externals: self.family.external_count(),
            })?;
        Ok(row)
    }

    fn add_negative_derivative_term(
        &self,
        row: &mut ParametricRelation,
        shift: IndexShift,
        power: &ParametricCoefficient,
        derivative_coefficient: &crate::Coefficient,
    ) -> Result<(), ParametricIbpError> {
        if derivative_coefficient.is_zero() {
            return Ok(());
        }
        let derivative = self.context.lift(derivative_coefficient)?;
        let product = self.context.mul_with_limits(
            power,
            &derivative,
            self.config.arithmetic_limits.exact_algebra,
        )?;
        let coefficient = self
            .context
            .neg_with_limits(&product, self.config.arithmetic_limits.exact_algebra)?;
        row.add_term_with_limits(
            &self.context,
            shift,
            coefficient,
            self.config.arithmetic_limits,
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn add_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        constant: &crate::Coefficient,
        denominator_coefficients: &[crate::Coefficient],
        negate: bool,
        row_id: &ParametricRowId,
    ) -> Result<(), ParametricIbpError> {
        self.add_one_weighted_translation(
            target,
            source,
            self.index_space.zero(),
            constant,
            negate,
            row_id,
        )?;
        for (denominator, coefficient) in denominator_coefficients.iter().enumerate() {
            self.add_one_weighted_translation(
                target,
                source,
                self.negative_units[denominator].clone(),
                coefficient,
                negate,
                row_id,
            )?;
        }
        Ok(())
    }

    fn add_one_weighted_translation(
        &self,
        target: &mut ParametricRelation,
        source: &ParametricRelation,
        translation: IndexShift,
        base_factor: &crate::Coefficient,
        negate: bool,
        row_id: &ParametricRowId,
    ) -> Result<(), ParametricIbpError> {
        if base_factor.is_zero() {
            return Ok(());
        }
        let translated = source.translated(
            &self.context,
            &translation,
            row_id.clone(),
            self.config.arithmetic_limits,
        )?;
        let mut factor = self.context.lift(base_factor)?;
        if negate {
            factor = self
                .context
                .neg_with_limits(&factor, self.config.arithmetic_limits.exact_algebra)?;
        }
        target.add_scaled_with_limits(
            &self.context,
            &translated,
            &factor,
            self.config.arithmetic_limits,
        )?;
        Ok(())
    }

    fn empty_relation(
        &self,
        row_id: ParametricRowId,
    ) -> Result<ParametricRelation, ParametricIbpError> {
        let mut relation =
            ParametricRelation::new(self.family_fingerprint.clone(), row_id, &self.context);
        // Preserve the complete family domain before any fraction-field
        // cancellation.  Tautological nonzero constants are intentionally
        // omitted by ParametricRelation.
        for condition in self.family.domain().conditions() {
            let lifted = self.context.lift_base_polynomial(condition.polynomial())?;
            let lifted = self.context.nonzero_condition_with_origins_and_limits(
                lifted,
                condition.origins().iter().cloned(),
                self.config.arithmetic_limits.exact_algebra,
            )?;
            relation.add_guarded_nonzero_condition_with_limits(
                &self.context,
                lifted,
                self.config.arithmetic_limits,
            )?;
        }
        Ok(relation)
    }
}

/// Return the exact ordinary-IBP and LI row census without constructing any
/// symbolic row.  Resource-bounded callers use this preflight before entering
/// the generator's allocation and exact-algebra work.
pub(crate) fn checked_generated_row_counts(
    loops: usize,
    externals: usize,
) -> Result<(usize, usize), ParametricIbpError> {
    let contractions = loops
        .checked_add(externals)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let ordinary = loops
        .checked_mul(contractions)
        .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?;
    let li = if externals < 2 {
        0
    } else {
        let predecessor = externals - 1;
        let (left, right) = if externals % 2 == 0 {
            (externals / 2, predecessor)
        } else {
            (externals, predecessor / 2)
        };
        left.checked_mul(right)
            .ok_or(ParametricIbpError::RowCountOverflow { loops, externals })?
    };
    Ok((ordinary, li))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AffineDenominator, Coefficient, CoefficientContext};

    fn coefficient_for<'a>(
        relation: &'a crate::ConcreteRelation,
        powers: &[i64],
    ) -> Option<&'a Coefficient> {
        relation
            .terms()
            .iter()
            .find_map(|(key, coefficient)| (key.powers() == powers).then_some(coefficient))
    }

    fn assert_coefficient_eq(left: &Coefficient, right: &Coefficient) {
        assert!((left - right).is_zero(), "left={left}, right={right}");
    }

    fn identity_denominators(
        context: &CoefficientContext,
        constants: Vec<Coefficient>,
    ) -> Vec<AffineDenominator> {
        let size = constants.len();
        constants
            .into_iter()
            .enumerate()
            .map(|(row, constant)| {
                AffineDenominator::new(
                    constant,
                    (0..size)
                        .map(|column| {
                            if row == column {
                                context.one()
                            } else {
                                context.zero()
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn one_loop_tadpole_is_a_fully_parametric_recurrence() {
        let base = CoefficientContext::new(["d", "m2", "nu"]);
        let d = base.parameter("d").unwrap();
        let m2 = base.parameter("m2").unwrap();
        let nu = base.parameter("nu").unwrap();
        let family = IntegralFamily::new(
            "one-loop-tadpole-parametric",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            d.clone(),
            vec![AffineDenominator::new(m2.clone(), vec![base.one()])],
            Vec::new(),
            vec![nu.clone()],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let generated = generator.generate().unwrap();

        assert_eq!(generated.ordinary_ibp().len(), 1);
        assert!(generated.lorentz_invariance().is_empty());
        assert_eq!(
            generated.ordinary_ibp()[0].row_id(),
            &ParametricRowId::OrdinaryIbp {
                contraction_momentum: 0,
                differentiated_loop: 0,
            }
        );
        let concrete = generated.ordinary_ibp()[0]
            .specialize(
                generated.context(),
                &[3],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(concrete.terms().len(), 2);
        let shifted_power = &base.integer(3) + &nu;
        let expected_same = &d - &(&base.integer(2) * &shifted_power);
        let expected_raised = &(&base.integer(2) * &m2) * &shifted_power;
        assert_coefficient_eq(coefficient_for(&concrete, &[3]).unwrap(), &expected_same);
        assert_coefficient_eq(coefficient_for(&concrete, &[4]).unwrap(), &expected_raised);

        // Sector signs are determined by the raw index, but a power shift is
        // still present in the coefficient at n=0.  Raw generation must not
        // use the concrete zero-index shortcut of the legacy vacuum code.
        let at_zero = generated.ordinary_ibp()[0]
            .specialize(
                generated.context(),
                &[0],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(at_zero.terms().len(), 2);
        assert_coefficient_eq(
            coefficient_for(&at_zero, &[0]).unwrap(),
            &(&d - &(&base.integer(2) * &nu)),
        );
        assert_coefficient_eq(
            coefficient_for(&at_zero, &[1]).unwrap(),
            &(&(&base.integer(2) * &m2) * &nu),
        );
    }

    #[test]
    fn one_loop_li_has_litered_sign_and_weighted_denominator_shifts() {
        let base = CoefficientContext::new(["d", "s00", "s11", "c1", "c2", "nu0", "nu1", "nu2"]);
        let s00 = base.parameter("s00").unwrap();
        let s11 = base.parameter("s11").unwrap();
        let c1 = base.parameter("c1").unwrap();
        let c2 = base.parameter("c2").unwrap();
        let nu1 = base.parameter("nu1").unwrap();
        let nu2 = base.parameter("nu2").unwrap();
        let family = IntegralFamily::new(
            "one-loop-two-leg-li",
            vec!["k".into()],
            vec!["p0".into(), "p1".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            identity_denominators(&base, vec![base.zero(), c1.clone(), c2.clone()]),
            vec![
                vec![s00.clone(), base.zero()],
                vec![base.zero(), s11.clone()],
            ],
            vec![base.parameter("nu0").unwrap(), nu1.clone(), nu2.clone()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();

        assert_eq!(generated.ordinary_ibp().len(), 3);
        assert_eq!(generated.lorentz_invariance().len(), 1);
        assert_eq!(
            generated.lorentz_invariance()[0].row_id(),
            &ParametricRowId::LorentzInvariance {
                first_external: 0,
                second_external: 1,
            }
        );
        let concrete = generated.lorentz_invariance()[0]
            .specialize(
                generated.context(),
                &[2, 3, 4],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(concrete.terms().len(), 4);
        let n1 = &base.integer(3) + &nu1;
        let n2 = &base.integer(4) + &nu2;
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 4, 4]).unwrap(),
            &(&(&c2 * &s00) * &n1),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 4, 3]).unwrap(),
            &(-(&s00 * &n1)),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 3, 5]).unwrap(),
            &(-(&(&c1 * &s11) * &n2)),
        );
        assert_coefficient_eq(
            coefficient_for(&concrete, &[2, 2, 5]).unwrap(),
            &(&s11 * &n2),
        );
    }

    #[test]
    fn two_loop_rows_are_q_major_and_li_pairs_are_lexicographic() {
        let base = CoefficientContext::new(["d", "s00", "s01", "s02", "s11", "s12", "s22", "nu"]);
        let family = IntegralFamily::new(
            "two-loop-three-leg-structure",
            vec!["k0".into(), "k1".into()],
            vec!["p0".into(), "p1".into(), "p2".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            identity_denominators(&base, vec![base.zero(); 9]),
            vec![
                vec![
                    base.parameter("s00").unwrap(),
                    base.parameter("s01").unwrap(),
                    base.parameter("s02").unwrap(),
                ],
                vec![
                    base.parameter("s01").unwrap(),
                    base.parameter("s11").unwrap(),
                    base.parameter("s12").unwrap(),
                ],
                vec![
                    base.parameter("s02").unwrap(),
                    base.parameter("s12").unwrap(),
                    base.parameter("s22").unwrap(),
                ],
            ],
            vec![
                base.parameter("nu").unwrap(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
                base.zero(),
            ],
        )
        .unwrap();
        let generator = ParametricIbpGenerator::try_new(&family).unwrap();
        let generated = generator.generate().unwrap();

        let ids = generated
            .ordinary_ibp()
            .iter()
            .map(|row| row.row_id().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 0,
                    differentiated_loop: 0,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 0,
                    differentiated_loop: 1,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 1,
                    differentiated_loop: 0,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 1,
                    differentiated_loop: 1,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 0,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 2,
                    differentiated_loop: 1,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 0,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 3,
                    differentiated_loop: 1,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 0,
                },
                ParametricRowId::OrdinaryIbp {
                    contraction_momentum: 4,
                    differentiated_loop: 1,
                },
            ]
        );
        assert_eq!(
            generated
                .lorentz_invariance()
                .iter()
                .map(|row| row.row_id().clone())
                .collect::<Vec<_>>(),
            vec![
                ParametricRowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 1,
                },
                ParametricRowId::LorentzInvariance {
                    first_external: 0,
                    second_external: 2,
                },
                ParametricRowId::LorentzInvariance {
                    first_external: 1,
                    second_external: 2,
                },
            ]
        );
        assert_eq!(generated.ibp_li().count(), 13);
        assert!(
            generated
                .ordinary_ibp()
                .iter()
                .chain(generated.lorentz_invariance())
                .all(|row| row.arity() == 9 && row.family_fingerprint() == family.fingerprint())
        );

        // This family has ten independent ordinary source rows and three LI
        // combinations, so it exercises both deterministic parallel phases.
        let available = std::thread::available_parallelism().unwrap().get();
        for n_cores in [1, 2, 4].into_iter().filter(|width| *width <= available) {
            let execution = ParallelExecution::try_new(n_cores).unwrap();
            let candidate = generator.generate_with_execution(&execution).unwrap();
            assert_eq!(candidate.ordinary_ibp(), generated.ordinary_ibp());
            assert_eq!(
                candidate.lorentz_invariance(),
                generated.lorentz_invariance()
            );
        }
    }

    #[test]
    fn every_row_inherits_input_and_determinant_domain_guards() {
        let base = CoefficientContext::new(["d", "a", "b", "s", "g"]);
        let family = IntegralFamily::new(
            "guarded-one-loop-one-leg",
            vec!["k".into()],
            vec!["p".into()],
            base.clone(),
            base.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(base.zero(), vec![base.parse("a/s").unwrap(), base.one()]),
                AffineDenominator::new(
                    base.zero(),
                    vec![base.parameter("b").unwrap(), base.integer(2)],
                ),
            ],
            vec![vec![base.parameter("g").unwrap()]],
            vec![base.zero(), base.zero()],
        )
        .unwrap();
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let determinant = generated
            .context()
            .lift_base_polynomial(family.domain().determinant_nonzero().polynomial())
            .unwrap();
        let input_denominator = generated
            .context()
            .lift_base_polynomial(
                family
                    .domain()
                    .input_denominators()
                    .iter()
                    .find(|condition| !condition.polynomial().is_constant())
                    .unwrap()
                    .polynomial(),
            )
            .unwrap();
        assert_eq!(generated.ordinary_ibp().len(), 2);
        assert!(generated.ordinary_ibp().iter().all(|row| {
            row.nonzero_conditions().contains(&determinant)
                && row.nonzero_conditions().contains(&input_denominator)
        }));
        assert!(generated.ordinary_ibp().iter().all(|row| {
            let determinant_guard = row
                .guarded_nonzero_conditions()
                .iter()
                .find(|condition| condition.polynomial() == &determinant)
                .unwrap();
            let input_guard = row
                .guarded_nonzero_conditions()
                .iter()
                .find(|condition| condition.polynomial() == &input_denominator)
                .unwrap();
            determinant_guard
                .origins()
                .contains(&crate::GuardOrigin::FamilyBasisDeterminantNumerator)
                && input_guard.origins().contains(
                    &crate::GuardOrigin::FamilyInputCoefficientDenominator {
                        location: crate::CoefficientLocation::DenominatorCoefficient {
                            denominator: 0,
                            coordinate: 0,
                        },
                    },
                )
        }));
    }

    #[test]
    fn custom_context_must_match_family_base_and_arity() {
        let base = CoefficientContext::new(["d"]);
        let family = IntegralFamily::new(
            "context-check",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            base.one(),
            identity_denominators(&base, vec![base.zero()]),
            Vec::new(),
            vec![base.zero()],
        )
        .unwrap();
        let wrong_base = CoefficientContext::new(["x"]);
        let wrong_context =
            ParametricCoefficientContext::try_new(&wrong_base, "wrong-base", 1).unwrap();
        assert!(matches!(
            ParametricIbpGenerator::try_with_context(
                &family,
                wrong_context,
                ParametricIbpConfig::default()
            ),
            Err(ParametricIbpError::BaseContextMismatch)
        ));

        let wrong_arity = ParametricCoefficientContext::try_new(&base, "wrong-arity", 2).unwrap();
        assert!(matches!(
            ParametricIbpGenerator::try_with_context(
                &family,
                wrong_arity,
                ParametricIbpConfig::default()
            ),
            Err(ParametricIbpError::WrongIndexArity {
                expected: 1,
                actual: 2
            })
        ));
    }

    #[test]
    fn maximal_power_shift_times_parameter_is_a_typed_error_not_a_symbolica_panic() {
        let base = CoefficientContext::new(["x"]);
        let x = base.parameter("x").unwrap();
        let maximal_power = base.parse("x^65535").unwrap();
        let family = IntegralFamily::new(
            "maximal-power-shift",
            vec!["k".into()],
            Vec::new(),
            base.clone(),
            base.integer(4),
            vec![AffineDenominator::new(x, vec![base.one()])],
            Vec::new(),
            vec![maximal_power],
        )
        .unwrap();

        let error = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate_ordinary_ibp()
            .unwrap_err();
        assert!(matches!(
            error,
            ParametricIbpError::Coefficient(ParametricCoefficientError::ExactAlgebra(
                crate::ExactAlgebraError::ExponentLimit {
                    operation: crate::ExactAlgebraOperation::Multiply,
                    requested: 65_536,
                    limit: crate::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                    ..
                }
            ))
        ));
    }
}
