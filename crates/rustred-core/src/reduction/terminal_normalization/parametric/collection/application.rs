//! Cheap coefficient application after exact collection preparation.

use std::collections::{BTreeMap, btree_map::Entry};

use super::{VacuumFamilyAliasError, VacuumFamilyAliasPlan, VacuumIntegralKey};
use crate::algebra::{Coefficient, ExactAlgebraLimits};
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_normalization::TerminalAliasError;

impl VacuumFamilyAliasPlan {
    /// Normalize a finite sum of declared, family-qualified terminals.
    ///
    /// The immutable plan is prepared once. Application performs only direct
    /// representative lookup, coefficient validation and Symbolica addition;
    /// it never reruns graph canonicalization or polynomial proof. Input
    /// coefficients may include the caller's external measure factors. The
    /// caller must retain the original reduction's conditions: this operation
    /// establishes no additional coverage or source-domain authority.
    ///
    /// Equalities preserve total propagator power and loop count, so common
    /// mass homogeneity is unchanged. Unknown keys/families are errors, even
    /// when their supplied coefficient is zero. Failure leaves this plan intact.
    /// The caller supplies a finite stream; algebra limits bound each operation,
    /// not the total number of input terms or the caller's iteration cost.
    pub fn apply<'a>(
        &self,
        terms: impl IntoIterator<Item = (&'a IntegralFamily, &'a IntegralKey, &'a Coefficient)>,
        limits: ExactAlgebraLimits,
    ) -> Result<BTreeMap<VacuumIntegralKey, Coefficient>, VacuumFamilyAliasError> {
        let algebra = |error: crate::algebra::ExactAlgebraError| {
            VacuumFamilyAliasError::Alias(TerminalAliasError::ExactAlgebra(error.to_string()))
        };
        let mut output = BTreeMap::new();
        for (family, key, coefficient) in terms {
            let representative = self.representative(family, key)?;
            let context = self.families[&representative.family].coefficient_context();
            context
                .validate_with_limits(coefficient, limits)
                .map_err(|error| match error {
                    crate::algebra::ExactAlgebraError::VariableMapMismatch { .. } => {
                        VacuumFamilyAliasError::Alias(TerminalAliasError::InvalidCoefficientContext)
                    }
                    other => algebra(other),
                })?;
            if coefficient.is_zero() {
                continue;
            }
            match output.entry(representative.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(coefficient.clone());
                }
                Entry::Occupied(mut entry) => {
                    let value = context
                        .try_add(entry.get(), coefficient, limits)
                        .map_err(algebra)?;
                    if value.is_zero() {
                        entry.remove();
                    } else {
                        entry.insert(value);
                    }
                }
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::family::AffineDenominator;
    use std::{collections::BTreeSet, sync::Arc};

    fn family(name: &str) -> Arc<IntegralFamily> {
        let context = CoefficientContext::new(["d"]);
        Arc::new(
            IntegralFamily::new(
                name,
                vec!["k".into()],
                vec![],
                context.clone(),
                context.parameter("d").unwrap(),
                vec![AffineDenominator::new(
                    context.integer(-1),
                    vec![context.one()],
                )],
                vec![],
                vec![context.zero()],
            )
            .unwrap(),
        )
    }

    #[test]
    fn application_coalesces_and_cancels_across_families_without_mutating_plan() {
        let a = family("collection-apply-a");
        let b = family("collection-apply-b");
        let key = IntegralKey::try_new([1]).unwrap();
        let plan = VacuumFamilyAliasPlan::prepare(
            &[
                (a.clone(), BTreeSet::from([key.clone()])),
                (b.clone(), BTreeSet::from([key.clone()])),
            ],
            Default::default(),
        )
        .unwrap();
        let context = a.coefficient_context();
        let d = context.parameter("d").unwrap();
        let minus_d = context
            .try_mul(&context.integer(-1), &d, Default::default())
            .unwrap();
        let before = plan.statistics().clone();
        let terms = [(a.as_ref(), &key, &d), (b.as_ref(), &key, &d)];
        let sum = plan.apply(terms, Default::default()).unwrap();
        assert_eq!(sum.len(), 1);
        assert_eq!(
            sum[plan.representative(&a, &key).unwrap()],
            context.try_add(&d, &d, Default::default()).unwrap()
        );
        let cancel = [(a.as_ref(), &key, &d), (b.as_ref(), &key, &minus_d)];
        assert!(plan.apply(cancel, Default::default()).unwrap().is_empty());
        assert_eq!(&before, plan.statistics());
        assert_eq!(plan.apply(terms, Default::default()).unwrap(), sum);
    }

    #[test]
    fn application_rejects_foreign_coefficients_and_unknown_zero_terms() {
        let a = family("collection-apply-errors");
        let key = IntegralKey::try_new([1]).unwrap();
        let plan = VacuumFamilyAliasPlan::prepare(
            &[(a.clone(), BTreeSet::from([key.clone()]))],
            Default::default(),
        )
        .unwrap();
        let foreign = CoefficientContext::new(["x"]).parameter("x").unwrap();
        assert!(matches!(
            plan.apply([(a.as_ref(), &key, &foreign)], Default::default()),
            Err(VacuumFamilyAliasError::Alias(
                TerminalAliasError::InvalidCoefficientContext
            ))
        ));
        let zero = a.coefficient_context().zero();
        let unknown = IntegralKey::try_new([2]).unwrap();
        assert!(matches!(
            plan.apply([(a.as_ref(), &unknown, &zero)], Default::default()),
            Err(VacuumFamilyAliasError::Alias(
                TerminalAliasError::UndeclaredTerminal
            ))
        ));
        assert!(
            plan.apply([(a.as_ref(), &key, &zero)], Default::default())
                .unwrap()
                .is_empty()
        );
    }
}
