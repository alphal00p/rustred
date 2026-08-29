use std::collections::BTreeSet;

use crate::algebra::CoefficientPolynomial;
use crate::family::{CoefficientLocation, FamilyDomain};

use super::limits::{check_limit, checked_add};
use super::{CoefficientMatrix, Error, Limits};

/// One stable reason why a polynomial must remain nonzero for an affine map.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConditionSource {
    SourceFamily(CoefficientLocation),
    TargetFamily(CoefficientLocation),
    MomentumMapDenominator {
        matrix: &'static str,
        row: usize,
        column: usize,
    },
    LoopMapDeterminantNumerator,
    ExternalMapDeterminantNumerator,
    DenominatorScaleNumerator {
        source_denominator: usize,
        target_denominator: usize,
    },
}

/// One merged, exact nonzero condition retained by authoritative replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonZeroCondition {
    polynomial: CoefficientPolynomial,
    sources: BTreeSet<ConditionSource>,
}

impl NonZeroCondition {
    pub const fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub const fn sources(&self) -> &BTreeSet<ConditionSource> {
        &self.sources
    }
}

pub(super) struct Collector {
    limits: Limits,
    conditions: Vec<NonZeroCondition>,
    source_count: usize,
}

impl Collector {
    pub(super) fn new(limits: Limits) -> Self {
        Self {
            limits,
            conditions: Vec::new(),
            source_count: 0,
        }
    }

    pub(super) fn add_family_domain(
        &mut self,
        domain: &FamilyDomain,
        source: bool,
    ) -> Result<(), Error> {
        for condition in domain.conditions() {
            for family_source in condition.sources() {
                let source = if source {
                    ConditionSource::SourceFamily(family_source.clone())
                } else {
                    ConditionSource::TargetFamily(family_source.clone())
                };
                self.add(condition.polynomial().clone(), source)?;
            }
        }
        Ok(())
    }

    pub(super) fn add(
        &mut self,
        polynomial: CoefficientPolynomial,
        source: ConditionSource,
    ) -> Result<(), Error> {
        if let Some(existing) = self
            .conditions
            .iter_mut()
            .find(|condition| condition.polynomial == polynomial)
        {
            if existing.sources.contains(&source) {
                return Ok(());
            }
            let requested = checked_add(self.source_count, 1, "symmetry condition sources")?;
            check_limit(
                "condition sources",
                requested,
                self.limits.max_condition_sources,
            )?;
            existing.sources.insert(source);
            self.source_count = requested;
            return Ok(());
        }

        let condition_count = checked_add(self.conditions.len(), 1, "symmetry nonzero conditions")?;
        check_limit(
            "nonzero conditions",
            condition_count,
            self.limits.max_nonzero_conditions,
        )?;
        let source_count = checked_add(self.source_count, 1, "symmetry condition sources")?;
        check_limit(
            "condition sources",
            source_count,
            self.limits.max_condition_sources,
        )?;
        self.conditions.push(NonZeroCondition {
            polynomial,
            sources: BTreeSet::from([source]),
        });
        self.source_count = source_count;
        Ok(())
    }

    pub(super) fn condition_count(&self) -> usize {
        self.conditions.len()
    }

    pub(super) fn source_count(&self) -> usize {
        self.source_count
    }

    pub(super) fn finish(self) -> Box<[NonZeroCondition]> {
        self.conditions.into_boxed_slice()
    }
}

pub(super) fn add_candidate_denominators<'a>(
    matrices: impl IntoIterator<Item = (&'static str, &'a CoefficientMatrix)>,
    conditions: &mut Collector,
) -> Result<(), Error> {
    for (name, matrix) in matrices {
        for row in 0..matrix.rows {
            for column in 0..matrix.columns {
                let coefficient = matrix.at(row, column);
                if coefficient.denominator.is_one() {
                    continue;
                }
                conditions.add(
                    coefficient.denominator.clone(),
                    ConditionSource::MomentumMapDenominator {
                        matrix: name,
                        row,
                        column,
                    },
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::algebra::CoefficientContext;

    use super::*;

    #[test]
    fn candidate_denominators_merge_directly_with_complete_provenance() {
        let context = CoefficientContext::new(["x"]);
        let reciprocal = context.coefficient_fixture("1/x");
        let matrix = CoefficientMatrix::try_new(1, 2, [reciprocal.clone(), reciprocal]).unwrap();
        let expected = matrix.entries()[0].denominator.clone();
        let mut conditions = Collector::new(Limits::default());

        add_candidate_denominators([("A", &matrix)], &mut conditions).unwrap();

        assert_eq!(conditions.condition_count(), 1);
        assert_eq!(conditions.source_count(), 2);
        let conditions = conditions.finish();
        assert_eq!(conditions[0].polynomial(), &expected);
        assert_eq!(
            conditions[0].sources(),
            &BTreeSet::from([
                ConditionSource::MomentumMapDenominator {
                    matrix: "A",
                    row: 0,
                    column: 0,
                },
                ConditionSource::MomentumMapDenominator {
                    matrix: "A",
                    row: 0,
                    column: 1,
                },
            ])
        );
    }
}
