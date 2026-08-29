use std::collections::BTreeSet;

use crate::algebra::CoefficientPolynomial;
use crate::family::CoefficientLocation;

/// Why one generic-domain condition is present.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConditionSource {
    Family(CoefficientLocation),
    PowerShiftSupport { denominator: usize },
}

/// One exact polynomial required to remain nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainCondition {
    polynomial: CoefficientPolynomial,
    sources: BTreeSet<ConditionSource>,
}

impl DomainCondition {
    pub fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub fn sources(&self) -> &BTreeSet<ConditionSource> {
        &self.sources
    }
}

/// Generic locus on which the family and effective power support are valid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Domain {
    conditions: Vec<DomainCondition>,
}

impl Domain {
    pub fn conditions(&self) -> &[DomainCondition] {
        &self.conditions
    }

    pub(super) fn insert(
        &mut self,
        polynomial: CoefficientPolynomial,
        sources: BTreeSet<ConditionSource>,
    ) {
        if let Some(condition) = self
            .conditions
            .iter_mut()
            .find(|condition| condition.polynomial == polynomial)
        {
            condition.sources.extend(sources);
        } else {
            self.conditions.push(DomainCondition {
                polynomial,
                sources,
            });
        }
    }
}
