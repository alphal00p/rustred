use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedCoefficientContext};
use crate::family::IntegralFamily;

use super::super::relation::IndexSpace;
use super::config::ParametricIbpConfig;
use super::counts::checked_row_counts;
use super::error::{ParametricIbpError, try_preallocate_vec};
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
        let mut positive_units = try_preallocate_vec("positive unit index shifts", arity)?;
        for position in 0..arity {
            positive_units.push(index_space.unit(position, 1)?);
        }
        let mut negative_units = try_preallocate_vec("negative unit index shifts", arity)?;
        for position in 0..arity {
            negative_units.push(index_space.unit(position, -1)?);
        }
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
        let arity = self.family.denominator_count();
        let mut powers = try_preallocate_vec("ordinary power coefficients", arity)?;
        for denominator in 0..arity {
            let index = self.context.index(denominator)?;
            let power_shift = self
                .context
                .lift(&self.family.power_shifts()[denominator])?;
            powers.push(self.context.add_with_limits(
                &index,
                &power_shift,
                self.config.relation_limits.arithmetic.exact_algebra,
            )?);
        }
        Ok(powers)
    }
}
