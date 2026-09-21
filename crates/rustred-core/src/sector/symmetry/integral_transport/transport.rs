use crate::family::IntegralKey;
use crate::family::numerator_expansion::{
    MultiAffineNumeratorFactor, preflight_coefficient_clones, try_expand_multi_affine_numerator,
};

use super::compile::{admit, overflow, reserved};
use super::{Error, ExpansionError, ExpansionLimits, Prepared, TransportedIntegral};

impl Prepared {
    /// Expand the concrete numerator and route its monomials to target keys.
    ///
    /// Positive source powers may be arbitrarily large; only finite numerator
    /// expansion is budgeted. Target support is contained in the mapped
    /// positive source support; total numerator rank and dot excess cannot
    /// increase. Pinches and exact cancellations are retained.
    /// These facts do not establish a well-founded recursive owner schedule.
    pub fn transport(
        &self,
        source: &IntegralKey,
        limits: ExpansionLimits,
    ) -> Result<TransportedIntegral, Error> {
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
        )?;
        admit(
            "transport target axes",
            target_arity,
            limits.max_endpoint_power_entries,
        )?;
        let mut count = 0usize;
        let mut rank = 0u64;
        for (axis, &power) in source.powers().iter().enumerate() {
            if power > 0 && !self.source_root.active_bits()[axis] {
                return Err(Error::PositiveOutsideRoot { axis, power });
            }
            if power < 0 {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| overflow("transport factors"))?;
                rank = rank
                    .checked_add(power.unsigned_abs())
                    .ok_or_else(|| overflow("transport numerator rank"))?;
            }
        }
        admit("transport factors", count, limits.max_factors)?;
        let entries = count
            .checked_mul(target_arity)
            .ok_or_else(|| overflow("transport relation entries"))?;
        admit(
            "transport relation entries",
            entries,
            limits.max_relation_coefficient_entries,
        )?;
        if rank > limits.max_total_power {
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
                .filter(|(_, power)| **power < 0)
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
            if power > 0 {
                base[self.active_target[axis].expect("compiled active bijection")] = power;
            } else if power < 0 {
                factors.push(MultiAffineNumeratorFactor::try_new(
                    self.map.denominators().constant()[axis].clone(),
                    (0..target_arity).map(|column| {
                        self.map
                            .denominators()
                            .linear()
                            .get(axis, column)
                            .expect("verified matrix shape")
                            .clone()
                    }),
                    power.unsigned_abs(),
                )?);
            }
        }
        let base = IntegralKey::try_from_preallocated(base).map_err(ExpansionError::IntegralKey)?;
        let terms = try_expand_multi_affine_numerator(&self.target, &base, &factors, limits)?;
        Ok(TransportedIntegral {
            source: IntegralKey::try_new(source.powers().iter().copied())
                .map_err(ExpansionError::IntegralKey)?,
            source_fingerprint: self.source_fingerprint.clone(),
            target_fingerprint: self.target.fingerprint_owner(),
            terms,
        })
    }
}
