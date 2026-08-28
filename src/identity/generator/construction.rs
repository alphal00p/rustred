use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::family::IntegralFamily;

use super::super::relation::IndexSpace;
use super::config::ParametricIbpConfig;
use super::counts::checked_row_counts;
use super::error::ParametricIbpError;
use super::model::ParametricIbpGenerator;
use super::scope::IbpSourceScope;

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
        // constructor. Thus two distinct family definitions never alias an
        // index-variable identity merely because their display names agree.
        let scope = format!("ordinary-ibp|{family_fingerprint}");
        let context = IndexedCoefficientContext::try_new(
            family.coefficient_context(),
            &scope,
            family.denominator_count(),
        )?;
        let arity = family.denominator_count();
        checked_row_counts(family.loop_count(), family.external_count())?;
        let index_space = IndexSpace::try_new(arity)?;
        let positive_units = (0..arity)
            .map(|position| index_space.unit(position, 1))
            .collect::<Result<Vec<_>, _>>()?;
        let negative_units = (0..arity)
            .map(|position| index_space.unit(position, -1))
            .collect::<Result<Vec<_>, _>>()?;
        let zero_shift = index_space.try_zero()?;
        let source_scope = IbpSourceScope {
            family_fingerprint,
            context_fingerprint: context.fingerprint().into(),
        };
        Ok(Self {
            family,
            source_scope,
            context,
            zero_shift,
            positive_units,
            negative_units,
            config,
        })
    }

    pub fn context(&self) -> &IndexedCoefficientContext {
        &self.context
    }

    pub(super) fn prepare_ordinary_coefficients(
        &self,
    ) -> Result<(IndexedCoefficient, Vec<IndexedCoefficient>), ParametricIbpError> {
        let dimension = self.context.lift(self.family.dimension())?;
        let powers = self.prepare_ordinary_powers()?;
        Ok((dimension, powers))
    }

    pub(super) fn prepare_ordinary_powers(
        &self,
    ) -> Result<Vec<IndexedCoefficient>, ParametricIbpError> {
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
                        self.config.relation_limits.arithmetic.exact_algebra,
                    )
                    .map_err(ParametricIbpError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(powers)
    }
}
