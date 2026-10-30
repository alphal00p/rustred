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
        let selected = validate_routes(
            &programs.context,
            |sector| programs.owners.contains_key(sector),
            routes,
            limits,
        )?;
        Ok(Self {
            programs,
            routes: selected,
            limits,
        })
    }
}

pub(super) fn validate_routes<const N: usize>(
    context: &super::super::CandidateOwnerContext<N>,
    contains_owner: impl Fn(&[bool; N]) -> bool,
    routes: impl IntoIterator<Item = CandidateOwnerRoute>,
    limits: RoutedCandidateLimits,
) -> Result<BTreeMap<[bool; N], CandidateOwnerRoute>, CandidateRoutedError> {
    if limits.expansion.exact_algebra != context.limits.exact_algebra {
        return Err(CandidateRoutedError::InvalidInput(
            "transport and program exact-algebra policies must match".into(),
        ));
    }
    let fingerprint = context.family.fingerprint();
    let mut selected = BTreeMap::new();
    for route in routes {
        // Routed traversal expands numerator support in the native-Q
        // trace-support lane, which cannot carry parameter-dependent affine
        // constants (an `integral_transport::AffineConstants::Parametric`
        // opt-in). Refuse such a route first, rather than mid-traversal.
        if route.transport.has_parametric_constants() {
            return Err(CandidateRoutedError::Transport(
                crate::sector::symmetry::integral_transport::Error::ParametricConstantSupport,
            ));
        }
        if route.owner_sector.arity() != context.family.denominator_count()
            || route.transport.source_root().arity() != context.family.denominator_count()
            || route.transport.target_root().arity() != context.family.denominator_count()
            || route.transport.source_family_fingerprint() != fingerprint
            || route.transport.target_family().fingerprint() != fingerprint
            || !route
                .transport
                .target_family()
                .coefficient_context()
                .has_same_variable_map(context.family.coefficient_context())
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

        let owner = std::array::from_fn(|i| {
            route
                .owner_sector
                .active_bits()
                .get(i)
                .copied()
                .unwrap_or(false)
        });
        let source = std::array::from_fn(|i| {
            route
                .transport
                .source_root()
                .active_bits()
                .get(i)
                .copied()
                .unwrap_or(false)
        });
        if !contains_owner(&owner) {
            return Err(CandidateRoutedError::InvalidInput(
                "route target owner is absent".into(),
            ));
        }
        if contains_owner(&source) {
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
    Ok(selected)
}
