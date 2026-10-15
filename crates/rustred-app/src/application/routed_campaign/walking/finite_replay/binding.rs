//! Exact authenticated input-row bindings. No containing-domain approximation.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(in crate::application::routed_campaign::walking) struct InitialBindings<const N: usize> {
    roots: BTreeMap<u32, Domain<N>>,
}
impl<const N: usize> InitialBindings<N> {
    /// Reconstructed only after ordinary input/admissible amendment validation.
    /// Original and appended query rows share this exact geometry check; p0 is
    /// deliberately not used as an eligibility boundary.
    pub fn build(
        reducer: &RoutedCandidateReducer<N>,
        request: &OwnerDomainWalkRequest,
        rows: &[Value],
        mut stored: impl FnMut(u32) -> Option<Domain<N>>,
    ) -> Result<Self, String> {
        request.validate_finite_replay().map_err(str::to_owned)?;
        if !request.finite_replay_initial_queries {
            return Err("initial-query bindings require explicit v3 mode".into());
        }
        let arity = reducer.programs().context().family().denominator_count();
        let original = matching::input::parse(
            &request.matching.queries_json,
            arity,
            request.matching.max_queries,
            request.matching.max_query_bytes,
        )
        .map_err(|error| error.to_string())?;
        let amendments = request
            .amendments
            .iter()
            .map(|value| super::super::rescue::parse(value, arity))
            .collect::<Result<Vec<_>, _>>()?;
        super::super::rescue::check_chain(
            &[],
            &amendments,
            &super::super::checkpoint::epoch_request_binding(request),
            &original,
        )?;
        if amendments
            .iter()
            .any(|amendment| !amendment.supersede.is_empty())
        {
            return Err("finite v3 cannot reinterpret rescue/quarantine amendments".into());
        }
        let expected: Vec<_> = original
            .iter()
            .map(|query| (query, None))
            .chain(amendments.iter().flat_map(|amendment| {
                amendment
                    .queries
                    .iter()
                    .map(move |query| (query, Some(amendment.sequence)))
            }))
            .collect();
        if rows.len() != expected.len() {
            return Err(
                "finite initial-query row inventory differs from authenticated scope".into(),
            );
        }
        let mut roots = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for (row, (query, amendment)) in rows.iter().zip(expected) {
            if !ids.insert(query.id.as_str())
                || row["id"].as_str() != Some(query.id.as_str())
                || row["role"].as_str()
                    != Some(if query.auxiliary {
                        "auxiliary"
                    } else {
                        "required"
                    })
                || row["role_declared"].as_bool() != Some(query.role_declared)
                || row["amendment"].as_u64() != amendment
            {
                return Err("finite initial query identity, role or amendment changed".into());
            }
            let domain = if amendment.is_some() {
                let installed = reducer.programs().owner_sectors().any(|owner| {
                    owner.as_slice()
                        == rustred::storage_array::<_, N>(&query.owner, false)
                            .as_ref()
                            .map(|owner| owner.as_slice())
                            .unwrap_or(&[])
                });
                Some(super::super::rescue::domain::<N>(
                    query,
                    installed,
                    request.route_domain_overcover,
                    reducer.domain_routing_requires_source_conditions(),
                )?)
            } else {
                query_domain(reducer, request, query)?
            };
            let Some(domain) = domain else {
                if !row["domain"].is_null() || row["source_validity_unresolved"] != true {
                    return Err(
                        "finite source-validity frontier was reinterpreted as an admitted root"
                            .into(),
                    );
                }
                continue;
            };
            if row["source_validity_unresolved"] == true {
                return Err("finite root source-validity flag differs".into());
            }
            let id = row["domain"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("finite initial query has no valid domain ID")?;
            let actual = stored(id).ok_or("finite query refers to an absent domain")?;
            if actual != domain {
                // Existing CP6 admission may alias a narrower requested query
                // to a containing root or descendant. Such a row remains
                // valid coverage but is not a whole-domain finite recipe.
                if !super::super::rescue::contains(&actual, &domain) {
                    return Err("finite query is not contained in its recorded domain".into());
                }
                continue;
            }
            if let Some(previous) = roots.insert(id, domain.clone())
                && previous != domain
            {
                return Err("different finite query geometries share one domain ID".into());
            }
        }
        Ok(Self { roots })
    }
    pub fn matches(&self, id: u32, domain: &Domain<N>) -> bool {
        self.roots.get(&id) == Some(domain)
    }
    pub fn len(&self) -> usize {
        self.roots.len()
    }
    pub fn contains(&self, id: u32) -> bool {
        self.roots.contains_key(&id)
    }
    #[cfg(test)]
    pub fn from_domains(roots: impl IntoIterator<Item = (u32, Domain<N>)>) -> Self {
        Self {
            roots: roots.into_iter().collect(),
        }
    }
}
