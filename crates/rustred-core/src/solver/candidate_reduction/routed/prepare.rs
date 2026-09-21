use super::super::CandidateOwnerPrograms;
use super::model::*;
use std::collections::BTreeMap;
use std::sync::Arc;

impl<const N: usize> RoutedCandidateReducer<N> {
    pub fn try_new(
        programs: Arc<CandidateOwnerPrograms<N>>,
        routes: impl IntoIterator<Item = CandidateOwnerRoute>,
        limits: RoutedCandidateLimits,
    ) -> Result<Self, CandidateRoutedError> {
        if limits.expansion.exact_algebra != programs.context.limits.exact_algebra {
            return Err(CandidateRoutedError::InvalidInput(
                "transport and program exact-algebra policies must match".into(),
            ));
        }
        let fingerprint = programs.context.family.fingerprint();
        let mut selected = BTreeMap::new();
        for route in routes {
            if route.owner_sector.arity() != N
                || route.transport.source_root().arity() != N
                || route.transport.target_root().arity() != N
                || route.transport.source_family_fingerprint() != fingerprint
                || route.transport.target_family().fingerprint() != fingerprint
                || !route
                    .transport
                    .target_family()
                    .coefficient_context()
                    .has_same_variable_map(programs.context.family.coefficient_context())
            {
                return Err(CandidateRoutedError::InvalidInput(
                    "route does not bind the common family/arity".into(),
                ));
            }
            if route.transport.target_root() != &route.owner_sector {
                return Err(CandidateRoutedError::InvalidInput(
                    "route target differs from its declared owner".into(),
                ));
            }
            let owner = std::array::from_fn(|i| route.owner_sector.active_bits()[i]);
            let source = std::array::from_fn(|i| route.transport.source_root().active_bits()[i]);
            if !programs.owners.contains_key(&owner) {
                return Err(CandidateRoutedError::InvalidInput(
                    "route target owner is absent".into(),
                ));
            }
            if programs.owners.contains_key(&source) {
                return Err(CandidateRoutedError::InvalidInput(
                    "an installed literal owner cannot be redirected".into(),
                ));
            }
            if selected.insert(source, route).is_some() {
                return Err(CandidateRoutedError::InvalidInput(
                    "duplicate selected source-support route".into(),
                ));
            }
        }
        Ok(Self {
            programs,
            routes: selected,
            limits,
        })
    }
}
