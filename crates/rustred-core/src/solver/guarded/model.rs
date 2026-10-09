use super::{IndexDomain, IndexRole};
use crate::algebra::CoefficientPolynomial;
use crate::solver::{IntegralOrder, PolynomialRow, RuleCandidate, SolverError, SourceSystem};

/// One identity, valid only on its declared integer domain and nonzero locus.
#[derive(Clone, Debug)]
pub struct GuardedSource<const N: usize> {
    pub id: String,
    pub row: PolynomialRow<N>,
    pub domain: IndexDomain<N>,
    pub nonzero_conditions: Vec<CoefficientPolynomial>,
}

impl<const N: usize> GuardedSource<N> {
    pub fn new(id: impl Into<String>, row: PolynomialRow<N>, domain: IndexDomain<N>) -> Self {
        Self {
            id: id.into(),
            row,
            domain,
            nonzero_conditions: Vec::new(),
        }
    }
    pub fn with_nonzero_conditions(mut self, conditions: Vec<CoefficientPolynomial>) -> Self {
        self.nonzero_conditions = conditions;
        self
    }
}

/// Stable original-source metadata; ordinals refer to the supplied corpus.
#[derive(Clone, Debug)]
pub struct GuardedSourceInfo<const N: usize> {
    pub id: String,
    pub domain: IndexDomain<N>,
    pub nonzero_conditions: Vec<CoefficientPolynomial>,
}

/// Immutable source context shared by discovery, exact replay and application.
///
/// `measure_id` identifies the caller's distribution conventions and support
/// functions. Saved programs additionally bind the complete original corpus.
/// Nontrivial symmetries can be supplied as guarded identities; no denominator
/// symmetry or ordinary scalelessness assumption is enabled implicitly.
#[derive(Debug)]
pub struct GuardedSourceSystem<const N: usize> {
    pub(in crate::solver) system: SourceSystem<N>,
    pub(in crate::solver) roles: [IndexRole; N],
    pub(in crate::solver) measure_id: String,
    pub(in crate::solver) sources: Vec<GuardedSourceInfo<N>>,
    pub(in crate::solver) zero_domains: Vec<IndexDomain<N>>,
}

impl<const N: usize> GuardedSourceSystem<N> {
    pub fn new(
        measure_id: impl Into<String>,
        roles: [IndexRole; N],
        indices: [usize; N],
        sources: Vec<GuardedSource<N>>,
    ) -> Result<Self, SolverError> {
        let measure_id = measure_id.into();
        if N == 0 || measure_id.is_empty() {
            return Err(SolverError::InvalidInput(
                "guarded sources require coordinates and a measure identity".into(),
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut rows = Vec::with_capacity(sources.len());
        let mut metadata = Vec::with_capacity(sources.len());
        let valid = IndexDomain::for_roles(&roles);
        for source in sources {
            if source.id.is_empty() || !ids.insert(source.id.clone()) {
                return Err(SolverError::InvalidInput(
                    "source IDs must be nonempty and unique".into(),
                ));
            }
            if !source.domain.is_subset_of(&valid) {
                return Err(SolverError::InvalidInput(
                    "source domain includes a negative occupation index".into(),
                ));
            }
            rows.push(source.row);
            metadata.push(GuardedSourceInfo {
                id: source.id,
                domain: source.domain,
                nonzero_conditions: source.nonzero_conditions,
            });
        }
        let system = SourceSystem::new(rows, indices)?;
        for source in &metadata {
            for condition in &source.nonzero_conditions {
                if condition.variables().as_ref() != system.coefficient_variables()
                    || condition.is_zero()
                {
                    return Err(SolverError::InvalidInput(
                        "source condition has a different variable map or is identically zero"
                            .into(),
                    ));
                }
            }
        }
        Ok(Self {
            system,
            roles,
            measure_id,
            sources: metadata,
            zero_domains: Vec::new(),
        })
    }

    /// Caller-supplied measure-specific zero evidence. These domains are never
    /// inferred from an ordinary family and are bound into saved programs.
    pub fn with_zero_domains(mut self, domains: Vec<IndexDomain<N>>) -> Result<Self, SolverError> {
        let valid = IndexDomain::for_roles(&self.roles);
        if domains.iter().any(|domain| !domain.is_subset_of(&valid)) {
            return Err(SolverError::InvalidInput(
                "zero domain includes an invalid occupation index".into(),
            ));
        }
        self.zero_domains = domains;
        Ok(self)
    }

    /// Inspect the raw polynomial rows and coefficient-variable map.
    ///
    /// This view omits coordinate roles and identity guards. Use the guarded
    /// APIs for discovery and application so those restrictions remain enforced.
    pub fn native_sources(&self) -> &SourceSystem<N> {
        &self.system
    }
    pub fn roles(&self) -> &[IndexRole; N] {
        &self.roles
    }
    pub fn measure_id(&self) -> &str {
        &self.measure_id
    }
    pub fn sources(&self) -> &[GuardedSourceInfo<N>] {
        &self.sources
    }
    pub fn zero_domains(&self) -> &[IndexDomain<N>] {
        &self.zero_domains
    }
    pub fn valid_indices(&self, point: &[i64; N]) -> bool {
        IndexDomain::for_roles(&self.roles).contains(point)
    }
    pub fn is_zero(&self, point: &[i64; N]) -> bool {
        self.valid_indices(point)
            && (self
                .roles
                .iter()
                .zip(point)
                .any(|(role, n)| *role == IndexRole::RequiredCut && *n <= 0)
                || self
                    .zero_domains
                    .iter()
                    .any(|domain| domain.contains(point)))
    }
}

/// A replayed conditional rule; its source corpus remains owned by the context.
#[derive(Debug)]
pub struct GuardedRule<const N: usize> {
    pub(in crate::solver) candidate: RuleCandidate<N>,
    pub(in crate::solver) domain: IndexDomain<N>,
    pub(in crate::solver) nonzero_conditions: Vec<CoefficientPolynomial>,
    pub(in crate::solver) order: IntegralOrder<N>,
    pub(in crate::solver) discovery_domain: IndexDomain<N>,
}
impl<const N: usize> GuardedRule<N> {
    pub fn candidate(&self) -> &RuleCandidate<N> {
        &self.candidate
    }
    pub fn domain(&self) -> &IndexDomain<N> {
        &self.domain
    }
    pub fn nonzero_conditions(&self) -> &[CoefficientPolynomial] {
        &self.nonzero_conditions
    }
    pub fn ordering(&self) -> &IntegralOrder<N> {
        &self.order
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardedUnresolvedReason {
    SearchExhausted,
    ExceptionalCondition,
    DomainBudget,
    UnprovedDescent,
    UnsupportedPower,
}
#[derive(Clone, Debug)]
pub struct GuardedUnresolved<const N: usize> {
    pub domain: IndexDomain<N>,
    pub reason: GuardedUnresolvedReason,
    pub detail: String,
}
/// Partial coverage of explicitly requested domains. Residuals are not masters.
#[derive(Debug)]
pub struct GuardedSolution<const N: usize> {
    pub rules: Vec<GuardedRule<N>>,
    pub unresolved: Vec<GuardedUnresolved<N>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::CoefficientContext;
    use crate::solver::{Integral, Term};

    #[test]
    fn undefined_occupation_does_not_become_zero_when_a_required_cut_is_missing() {
        let context = CoefficientContext::new(["a", "b"]);
        let roles = [IndexRole::RequiredCut, IndexRole::Occupation];
        let source = GuardedSource::new(
            "test-source",
            vec![Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: context.one().numerator,
            }],
            IndexDomain::for_roles(&roles),
        );
        let system = GuardedSourceSystem::new("test-measure", roles, [0, 1], vec![source]).unwrap();
        assert!(!system.valid_indices(&[0, -1]));
        assert!(!system.is_zero(&[0, -1]));
        assert!(!system.is_zero(&[-1, -1]));
        assert!(system.is_zero(&[0, 0]));
        assert!(system.is_zero(&[-1, 1]));
        assert!(!system.is_zero(&[1, 0]));
    }
}
