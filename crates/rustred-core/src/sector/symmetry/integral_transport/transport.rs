use crate::family::IntegralKey;
use crate::family::numerator_expansion::{
    AdmittedSupport, ExpansionUsage, MultiAffineNumeratorFactor, preflight_coefficient_clones,
    try_expand_multi_affine_numerator_with_usage, try_expand_multi_affine_support_with_usage,
    try_expand_parametric_multi_affine_numerator_with_usage,
};

use super::compile::{admit, overflow, reserved};
use super::{Endpoint, Error, ExpansionError, ExpansionLimits, Prepared, TransportedIntegral};

impl Prepared {
    /// Expand the concrete numerator and route its monomials to target keys.
    ///
    /// Positive source powers may be arbitrarily large; only finite numerator
    /// expansion is budgeted. Target support is contained in the mapped
    /// positive source support; total numerator rank and dot excess cannot
    /// increase. Pinches and exact cancellations are retained.
    /// These facts do not establish a well-founded recursive owner schedule.
    ///
    /// A forced (eps) axis, power `n + j eps`, is a line at every integer
    /// power `n`: it maps through the active bijection onto the target axis
    /// with the same shift, also at `n <= 0`, never into a numerator factor.
    pub fn transport(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
    ) -> Result<TransportedIntegral, Error> {
        self.transport_with_usage(source, limits, |_| Ok(()))
    }

    /// Reserve shared work before native polynomial expansion and endpoint
    /// allocation. Earlier coefficient clones retain their per-call preflight.
    pub(crate) fn transport_with_usage(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
        reserve: impl FnOnce(ExpansionUsage) -> Result<(), ExpansionError>,
    ) -> Result<TransportedIntegral, Error> {
        let (base, factors) = self.expansion_input(source, limits)?;
        // Strictly compiled maps keep the native-Q lane unchanged; only an
        // explicit parametric opt-in reaches the substitution lane.
        let terms = if self.parametric_constants {
            try_expand_parametric_multi_affine_numerator_with_usage(
                &self.target,
                &base,
                &factors,
                limits,
                reserve,
            )?
        } else {
            try_expand_multi_affine_numerator_with_usage(
                &self.target,
                &base,
                &factors,
                limits,
                reserve,
            )?
        };
        Ok(TransportedIntegral {
            source: IntegralKey::try_new(source.powers().iter().copied())
                .map_err(ExpansionError::IntegralKey)?,
            source_fingerprint: self.source_fingerprint.clone(),
            target_fingerprint: self.target.fingerprint_owner(),
            terms,
        })
    }

    /// Trace-only support: native algebra and every virtual output limit are
    /// admitted before returning any keys. Real coefficient reduction must use
    /// `transport`; this iterator is not an integral combination. Maps with
    /// parameter-dependent affine constants are rejected by this native-Q
    /// support lane before any work; only `transport` substitutes them.
    pub(crate) fn transport_support_with_usage(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
        reserve: impl FnOnce(ExpansionUsage) -> Result<(), ExpansionError>,
    ) -> Result<AdmittedSupport, Error> {
        if self.parametric_constants {
            return Err(Error::ParametricConstantSupport);
        }
        let (base, factors) = self.expansion_input(source, limits)?;
        Ok(try_expand_multi_affine_support_with_usage(
            &self.target,
            &base,
            &factors,
            limits,
            reserve,
        )?)
    }

    /// Layered transport, first part: the positive powers of `source` on
    /// their target axes (zero elsewhere), after the admission of
    /// [`Prepared::transport`]. The full transport of `source` is this base
    /// times the product of the factors of its negative axes
    /// ([`Prepared::factor_expansion`]); the product may be expanded one
    /// factor at a time, in any order, with exactly the same coalesced result.
    /// A forced (eps) axis sits in the base at every power and has no factor:
    /// [`Prepared::factor_expansion`] refuses it.
    pub(crate) fn layered_base(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
    ) -> Result<IntegralKey, Error> {
        Ok(self.expansion_input(source, limits)?.0)
    }

    /// rr-alloc: the powers of [`Self::layered_base`] of `source`, appended to
    /// `out`, without building the numerator factors: only the checks of
    /// [`Self::transport`] that read the positive powers (arity, root). A
    /// caller that reuses a factor expansion keyed by the non-positive powers
    /// of `source` (which carry every other check) gets exactly the endpoints
    /// of [`Self::transport`] as base plus relative endpoint.
    pub(crate) fn layered_base_powers(&self, source: &[i64], out: &mut Vec<i64>) -> Result<(), Error> {
        let arity = self.source_root.arity();
        if source.len() != arity {
            return Err(Error::WrongInputArity {
                expected: arity,
                actual: source.len(),
            });
        }
        let start = out.len();
        out.resize(start + self.target.denominator_count(), 0);
        for (axis, &power) in source.iter().enumerate() {
            if power > 0 && !self.source_root.active_bits()[axis] {
                return Err(Error::PositiveOutsideRoot { axis, power });
            }
            if power > 0 || self.is_forced(axis) {
                out[start + self.active_target[axis].expect("compiled active bijection")] = power;
            }
        }
        Ok(())
    }

    /// Layered transport, one factor: the exact expansion of the affine
    /// numerator factor of source axis `axis`, `(c_a + sum_l L_al D_l)^power`,
    /// as endpoints relative to the zero key (the monomial `prod_l D_l^e_l`
    /// has the key `-e`), sorted and exactly coalesced, through the same
    /// native expansion service and limits as [`Prepared::transport`].
    pub(crate) fn factor_expansion(
        &self,
        axis: usize,
        power: u64,
        limits: ExpansionLimits,
    ) -> Result<Box<[Endpoint]>, Error> {
        let arity = self.source_root.arity();
        if axis >= arity {
            return Err(Error::WrongInputArity {
                expected: arity,
                actual: axis.saturating_add(1),
            });
        }
        let target_arity = self.target.denominator_count();
        let mut base = reserved(target_arity, "transport base powers")?;
        base.resize(target_arity, 0);
        let base = IntegralKey::try_from_preallocated(base).map_err(ExpansionError::IntegralKey)?;
        // A forced (eps) axis is never a numerator factor; the layered base
        // already carries it at every power.
        if self.is_forced(axis) {
            return Err(Error::AnalyticPowerShift {
                side: "source",
                axis,
            });
        }
        let factor = self.factor(axis, power)?;
        Ok(try_expand_multi_affine_numerator_with_usage(
            &self.target,
            &base,
            std::slice::from_ref(&factor),
            limits,
            |_| Ok(()),
        )?)
    }

    /// Whether source axis `axis` is a forced (eps) axis: compile admits a
    /// shifted axis only inside the root and onto a target axis with the same
    /// shift, so the target shift of its image decides.
    fn is_forced(&self, axis: usize) -> bool {
        self.active_target
            .get(axis)
            .copied()
            .flatten()
            .is_some_and(|column| !self.target.power_shifts()[column].is_zero())
    }

    fn factor(&self, axis: usize, power: u64) -> Result<MultiAffineNumeratorFactor, Error> {
        let target_arity = self.target.denominator_count();
        Ok(MultiAffineNumeratorFactor::try_new(
            self.map.denominators().constant()[axis].clone(),
            (0..target_arity).map(|column| {
                self.map
                    .denominators()
                    .linear()
                    .get(axis, column)
                    .expect("verified matrix shape")
                    .clone()
            }),
            power,
        )?)
    }

    fn expansion_input(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
    ) -> Result<(IntegralKey, Vec<MultiAffineNumeratorFactor>), Error> {
        let arity = self.source_root.arity();
        if source.powers().len() != arity {
            return Err(Error::WrongInputArity {
                expected: arity,
                actual: source.powers().len(),
            });
        }
        let target_arity = self.target.denominator_count();
        admit(
            "transport source axes",
            arity,
            limits.max_endpoint_power_entries,
            limits.diagnostics_enabled,
        )?;
        admit(
            "transport target axes",
            target_arity,
            limits.max_endpoint_power_entries,
            limits.diagnostics_enabled,
        )?;
        let mut count = 0usize;
        let mut rank = 0u64;
        for (axis, &power) in source.powers().iter().enumerate() {
            if power > 0 && !self.source_root.active_bits()[axis] {
                return Err(Error::PositiveOutsideRoot { axis, power });
            }
            if power < 0 && !self.is_forced(axis) {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| overflow("transport factors"))?;
                rank = rank
                    .checked_add(power.unsigned_abs())
                    .ok_or_else(|| overflow("transport numerator rank"))?;
            }
        }
        admit(
            "transport factors",
            count,
            limits.max_factors,
            limits.diagnostics_enabled,
        )?;
        let entries = count
            .checked_mul(target_arity)
            .ok_or_else(|| overflow("transport relation entries"))?;
        admit(
            "transport relation entries",
            entries,
            limits.max_relation_coefficient_entries,
            limits.diagnostics_enabled,
        )?;
        if limits.diagnostics_enabled && rank > limits.max_total_power {
            return Err(ExpansionError::ResourceLimit {
                resource: "transport numerator rank",
                requested: usize::try_from(rank).unwrap_or(usize::MAX),
                limit: usize::try_from(limits.max_total_power).unwrap_or(usize::MAX),
            }
            .into());
        }
        preflight_coefficient_clones(
            source
                .powers()
                .iter()
                .enumerate()
                .filter(|(axis, power)| **power < 0 && !self.is_forced(*axis))
                .flat_map(|(axis, _)| {
                    std::iter::once(&self.map.denominators().constant()[axis]).chain(
                        (0..target_arity).map(move |column| {
                            self.map
                                .denominators()
                                .linear()
                                .get(axis, column)
                                .expect("verified matrix shape")
                        }),
                    )
                }),
            limits,
        )?;
        let mut base = reserved(target_arity, "transport base powers")?;
        base.resize(target_arity, 0);
        let mut factors = reserved(count, "transport factors")?;
        for (axis, &power) in source.powers().iter().enumerate() {
            if power > 0 || self.is_forced(axis) {
                base[self.active_target[axis].expect("compiled active bijection")] = power;
            } else if power < 0 {
                factors.push(self.factor(axis, power.unsigned_abs())?);
            }
        }
        let base = IntegralKey::try_from_preallocated(base).map_err(ExpansionError::IntegralKey)?;
        Ok((base, factors))
    }
}

#[cfg(test)]
mod forced_axis_tests {
    use std::sync::Arc;

    use crate::family::{IntegralFamily, IntegralKey};
    use crate::input::{Compiler, Limits, LoweringLimits};
    use crate::sector::Mask;
    use crate::sector::symmetry::{CoefficientMatrix, MomentumMap, verify};

    use super::super::{Error, compile};

    /// The 2L kite with eps lines D1 and D5 (rules-study T001c).
    fn t001c() -> Arc<IntegralFamily> {
        let source = "I(name(T001c),loops(k1,k2),externals(q),dimension(d),\
            prop(D1,(k1)^2,1),prop(D2,(-k1-k2)^2,1),prop(D3,(k2)^2,1),\
            prop(D4,(k1+k2-q)^2,1),prop(D5,(-k2+q)^2,1),gram(q,q,-1),\
            power_shift(D1,2-d/2),power_shift(D5,2-d/2))";
        Arc::new(
            Compiler::new(Limits::default())
                .unwrap()
                .compile_compact(source, None)
                .unwrap()
                .into_lowered(LoweringLimits::default())
                .unwrap()
                .into_family(),
        )
    }

    /// The layered lane keeps a forced axis in the base at every power and
    /// refuses to expand it as a numerator factor.
    #[test]
    fn layered_transport_keeps_forced_axes_in_the_base() {
        let family = t001c();
        let context = family.coefficient_context();
        let integers = |values: &[i64], rows, columns| {
            CoefficientMatrix::try_new(rows, columns, values.iter().map(|v| context.integer(*v)))
                .unwrap()
        };
        // q -> -q with k -> -k: every line onto itself.
        let map = MomentumMap::new(
            integers(&[-1, 0, 0, -1], 2, 2),
            integers(&[0, 0], 2, 1),
            integers(&[-1], 1, 1),
        );
        let verified = verify(&family, &family, map, Default::default()).unwrap();
        let root = Mask::try_new([true; 5]).unwrap();
        let prepared = compile(
            &family,
            Arc::clone(&family),
            Arc::new(verified),
            root.clone(),
            root,
            Default::default(),
        )
        .unwrap();
        let key = IntegralKey::try_new([-2, 1, 1, -1, 0]).unwrap();
        // D1 (forced) stays at -2 in the base, D4 (integer) becomes a factor.
        assert_eq!(
            prepared
                .layered_base(&key, Default::default())
                .unwrap()
                .powers(),
            [-2, 1, 1, 0, 0]
        );
        assert!(matches!(
            prepared.factor_expansion(0, 2, Default::default()),
            Err(Error::AnalyticPowerShift {
                side: "source",
                axis: 0
            })
        ));
        assert!(prepared.factor_expansion(3, 1, Default::default()).is_ok());
        let transported = prepared.transport(&key, Default::default()).unwrap();
        assert_eq!(transported.terms().len(), 1);
        assert_eq!(transported.terms()[0].key().powers(), key.powers());
    }
}
