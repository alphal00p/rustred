//! Bounded exact symmetry transport for immutable owner lookup.
//!
//! Route records retain stable append-only ordinals. A separately rebuilt
//! sector index provides binary lookup without turning an identity string or a
//! detached permutation into authority.

use std::collections::HashSet;
use std::sync::Arc;

use crate::sector::symmetry::{Canonicalizer, RoutingWitness};
use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

use super::super::identity::BoundedIdentityBuilder;
use super::super::{
    StratumRegistryError, StratumRegistryLimits, check_limit, checked_add, checked_mul, try_reserve,
};
use super::ImmutableOwner;

/// Exact preimage region of one immutable owner under an authenticated route.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ImmutableOwnerRouteCoverage {
    Sector(Mask),
    Domain {
        sector: Mask,
        bounds: Arc<[(i64, i64)]>,
    },
    Point {
        sector: Mask,
        powers: Arc<[i64]>,
    },
}

impl ImmutableOwnerRouteCoverage {
    fn sector(&self) -> &Mask {
        match self {
            Self::Sector(sector) | Self::Domain { sector, .. } | Self::Point { sector, .. } => {
                sector
            }
        }
    }

    fn covers(&self, target: &SectorInteriorDomain) -> bool {
        if self.sector() != target.sector() || target.arity() != self.sector().arity() {
            return false;
        }
        match self {
            Self::Sector(_) => true,
            Self::Domain { bounds, .. } => {
                bounds.len() == target.bounds().len()
                    && bounds
                        .iter()
                        .zip(target.bounds())
                        .all(|(&(lower, upper), &target)| {
                            lower <= target.lower() && target.upper() <= upper
                        })
            }
            Self::Point { powers, .. } => {
                powers.len() == target.bounds().len()
                    && powers
                        .iter()
                        .zip(target.bounds())
                        .all(|(&power, &bounds)| bounds.lower() == power && bounds.upper() == power)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ImmutableOwnerTransport {
    /// The exact untransported coverage reconstructed from the retained owner
    /// authority. This is never accepted as a detached alias.
    Direct,
    Symmetry(RoutingWitness),
}

/// One append-only raw-region route to an existing owner ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImmutableOwnerRoute {
    owner_ordinal: usize,
    coverage: ImmutableOwnerRouteCoverage,
    transport: ImmutableOwnerTransport,
}

impl ImmutableOwnerRoute {
    pub(super) const fn owner_ordinal(&self) -> usize {
        self.owner_ordinal
    }

    pub(super) fn sector(&self) -> &Mask {
        self.coverage.sector()
    }

    pub(super) fn covers(
        &self,
        owner: &ImmutableOwner,
        ordering: OrderingPolicy,
        target: &SectorInteriorDomain,
    ) -> bool {
        (!matches!(
            owner,
            ImmutableOwner::SolvedRewriteSector {
                ordering: expected,
                ..
            } if *expected != ordering
        )) && self.coverage.covers(target)
    }

    pub(super) fn append_identity(
        &self,
        stable: &mut BoundedIdentityBuilder,
    ) -> Result<(), StratumRegistryError> {
        stable.push_usize(self.owner_ordinal)?;
        stable.push("@")?;
        append_coverage_identity(stable, &self.coverage)?;
        stable.push("->")?;
        match &self.transport {
            ImmutableOwnerTransport::Direct => stable.push("direct"),
            ImmutableOwnerTransport::Symmetry(route) => {
                stable.push("symmetry:")?;
                stable.push_usize(route.group_element())?;
                stable.push(":")?;
                for (target, &source) in route.source_for_target().iter().enumerate() {
                    if target != 0 {
                        stable.push(",")?;
                    }
                    stable.push_usize(source)?;
                }
                Ok(())
            }
        }
    }
}

/// One binary-searchable sector bucket into a sorted route-ordinal sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImmutableOwnerRouteBucket {
    sector: Mask,
    index_start: usize,
    index_end: usize,
}

impl ImmutableOwnerRouteBucket {
    pub(super) fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(super) fn route_ordinals<'a>(&self, index: &'a [usize]) -> Option<&'a [usize]> {
        index.get(self.index_start..self.index_end)
    }

    pub(super) fn candidate_count(&self) -> Option<usize> {
        self.index_end.checked_sub(self.index_start)
    }

    pub(super) fn append_identity(
        &self,
        stable: &mut BoundedIdentityBuilder,
    ) -> Result<(), StratumRegistryError> {
        append_mask(stable, &self.sector)?;
        stable.push(":")?;
        stable.push_usize(self.index_start)?;
        stable.push("..")?;
        stable.push_usize(self.index_end)
    }
}

/// Append routes for a new owner suffix without changing any retained route
/// ordinal. Aggregate worst-case counts are rejected before allocation.
pub(super) fn try_append_owner_routes(
    routes: &mut Vec<ImmutableOwnerRoute>,
    owners: &[ImmutableOwner],
    first_owner_ordinal: usize,
    canonicalizer: Option<&Canonicalizer>,
    family_fingerprint: &str,
    arity: usize,
    limits: StratumRegistryLimits,
) -> Result<(), StratumRegistryError> {
    let additional_upper = check_owner_route_capacity(
        routes.len(),
        owners.len(),
        canonicalizer,
        family_fingerprint,
        arity,
        limits,
    )?;
    try_reserve(routes, additional_upper, "immutable owner symmetry routes")?;

    for (offset, owner) in owners.iter().enumerate() {
        let owner_ordinal =
            checked_add("immutable owner route ordinal", first_owner_ordinal, offset)?;
        if owner.arity() != arity {
            return Err(StratumRegistryError::WrongOwnerArity {
                owner: owner_ordinal,
                expected: arity,
                actual: owner.arity(),
            });
        }
        let direct = try_direct_coverage(owner)?;
        let seen_capacity = canonicalizer.map_or(Ok(1), |owner| {
            checked_add(
                "immutable owner symmetry-route deduplication",
                owner.group_order(),
                1,
            )
        })?;
        let mut seen = HashSet::new();
        seen.try_reserve(seen_capacity)
            .map_err(|_| StratumRegistryError::AllocationFailure {
                resource: "immutable owner symmetry-route deduplication",
                requested: seen_capacity,
            })?;
        seen.insert(direct.clone());
        routes.push(ImmutableOwnerRoute {
            owner_ordinal,
            coverage: direct,
            transport: ImmutableOwnerTransport::Direct,
        });
        if let Some(canonicalizer) = canonicalizer {
            for route in canonicalizer.routing_witnesses() {
                let coverage = try_preimage_coverage(owner, route.source_for_target(), arity)?;
                if seen.insert(coverage.clone()) {
                    routes.push(ImmutableOwnerRoute {
                        owner_ordinal,
                        coverage,
                        transport: ImmutableOwnerTransport::Symmetry(route),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Reject aggregate route and coordinate upper bounds before a caller copies
/// any owner or layer payload for a prospective extension.
pub(super) fn check_owner_route_capacity(
    current_route_count: usize,
    additional_owner_count: usize,
    canonicalizer: Option<&Canonicalizer>,
    family_fingerprint: &str,
    arity: usize,
    limits: StratumRegistryLimits,
) -> Result<usize, StratumRegistryError> {
    if let Some(canonicalizer) = canonicalizer {
        if canonicalizer.family_fingerprint() != family_fingerprint
            || canonicalizer.arity() != arity
        {
            return Err(StratumRegistryError::WrongOwnerRouteCanonicalizer);
        }
    }
    let routes_per_owner = match canonicalizer {
        Some(owner) => checked_add("immutable owner symmetry routes", 1, owner.group_order())?,
        None => 1,
    };
    let additional_upper = checked_mul(
        "immutable owner symmetry routes",
        additional_owner_count,
        routes_per_owner,
    )?;
    let requested_upper = checked_add(
        "immutable owner symmetry routes",
        current_route_count,
        additional_upper,
    )?;
    check_retained_route_capacity(requested_upper, arity, limits)?;
    Ok(additional_upper)
}

/// Bound an already retained route table before inspecting any route payload.
/// This is shared by construction preflight and cold verification under a
/// potentially tighter caller policy.
fn check_retained_route_capacity(
    route_count: usize,
    arity: usize,
    limits: StratumRegistryLimits,
) -> Result<(), StratumRegistryError> {
    check_limit(
        "immutable owner symmetry routes",
        route_count,
        limits.max_owner_routes,
    )?;
    let coordinate_upper = checked_mul(
        "immutable owner symmetry-route coordinate cells",
        checked_mul(
            "immutable owner symmetry-route coordinate cells",
            route_count,
            arity,
        )?,
        3,
    )?;
    check_limit(
        "immutable owner symmetry-route coordinate cells",
        coordinate_upper,
        limits.max_owner_route_coordinate_cells,
    )?;
    Ok(())
}

/// Rebuild the sorted lookup sidecars without reordering authoritative route
/// records. Candidate order is owner ordinal, then stable route ordinal.
pub(super) fn build_owner_route_index(
    routes: &[ImmutableOwnerRoute],
    limits: StratumRegistryLimits,
) -> Result<(Vec<usize>, Vec<ImmutableOwnerRouteBucket>), StratumRegistryError> {
    check_limit(
        "immutable owner symmetry routes",
        routes.len(),
        limits.max_owner_routes,
    )?;
    let mut index = Vec::new();
    try_reserve(
        &mut index,
        routes.len(),
        "immutable owner route-ordinal index",
    )?;
    index.extend(0..routes.len());
    index.sort_unstable_by(|&left, &right| {
        routes[left]
            .sector()
            .cmp(routes[right].sector())
            .then_with(|| routes[left].owner_ordinal.cmp(&routes[right].owner_ordinal))
            .then_with(|| left.cmp(&right))
    });

    let mut buckets = Vec::new();
    try_reserve(
        &mut buckets,
        routes.len(),
        "immutable owner route-sector buckets",
    )?;
    let mut start = 0usize;
    while start < index.len() {
        let sector = routes[index[start]].sector().clone();
        let mut end = start + 1;
        while end < index.len() && routes[index[end]].sector() == &sector {
            end += 1;
        }
        buckets.push(ImmutableOwnerRouteBucket {
            sector,
            index_start: start,
            index_end: end,
        });
        start = end;
    }
    Ok((index, buckets))
}

/// Cold reconstruction of every route, coverage record, sorted ordinal, and
/// bucket. Symmetry routes rejoin through the exact retained group authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn verify_owner_routes(
    owners: &[ImmutableOwner],
    routes: &[ImmutableOwnerRoute],
    route_index: &[usize],
    route_buckets: &[ImmutableOwnerRouteBucket],
    canonicalizer: Option<&Canonicalizer>,
    family_fingerprint: &str,
    arity: usize,
    limits: StratumRegistryLimits,
) -> Result<bool, StratumRegistryError> {
    check_retained_route_capacity(routes.len(), arity, limits)?;
    check_limit(
        "immutable owner route-ordinal index",
        route_index.len(),
        limits.max_owner_routes,
    )?;
    check_limit(
        "immutable owner route-sector buckets",
        route_buckets.len(),
        limits.max_owner_routes,
    )?;
    for route in routes {
        match &route.transport {
            ImmutableOwnerTransport::Direct => {}
            ImmutableOwnerTransport::Symmetry(witness) => {
                if !canonicalizer.is_some_and(|owner| owner.authenticates_route(witness)) {
                    return Ok(false);
                }
            }
        }
    }
    let mut expected = Vec::new();
    try_append_owner_routes(
        &mut expected,
        owners,
        0,
        canonicalizer,
        family_fingerprint,
        arity,
        limits,
    )?;
    if expected != routes {
        return Ok(false);
    }
    let (expected_index, expected_buckets) = build_owner_route_index(&expected, limits)?;
    Ok(expected_index == route_index && expected_buckets == route_buckets)
}

#[cfg(test)]
pub(super) fn corrupt_first_symmetry_authority_for_test(
    routes: &mut [ImmutableOwnerRoute],
    canonicalizer: &Canonicalizer,
) -> bool {
    for route in routes {
        if matches!(route.transport, ImmutableOwnerTransport::Symmetry(_)) {
            route.transport =
                ImmutableOwnerTransport::Symmetry(canonicalizer.unauthenticated_route_for_test());
            return true;
        }
    }
    false
}

fn try_direct_coverage(
    owner: &ImmutableOwner,
) -> Result<ImmutableOwnerRouteCoverage, StratumRegistryError> {
    match owner {
        ImmutableOwner::ZeroSector { sector, .. } => {
            Ok(ImmutableOwnerRouteCoverage::Sector(sector.clone()))
        }
        ImmutableOwner::Factorization { domain, .. }
        | ImmutableOwner::SolvedRewriteSector { domain, .. } => {
            let mut bounds = Vec::new();
            try_reserve(
                &mut bounds,
                domain.bounds().len(),
                "owner-route factorization bounds",
            )?;
            for bound in domain.bounds() {
                bounds.push((bound.lower(), bound.upper()));
            }
            Ok(ImmutableOwnerRouteCoverage::Domain {
                sector: domain.sector().clone(),
                bounds: bounds.into(),
            })
        }
        ImmutableOwner::Master { key } => Ok(ImmutableOwnerRouteCoverage::Point {
            sector: Mask::try_from_indices(key.powers())?,
            powers: Arc::from(key.powers()),
        }),
    }
}

fn try_preimage_coverage(
    owner: &ImmutableOwner,
    source_for_target: &[usize],
    arity: usize,
) -> Result<ImmutableOwnerRouteCoverage, StratumRegistryError> {
    if source_for_target.len() != arity {
        return Err(StratumRegistryError::Invariant {
            detail: "authenticated owner route has the wrong arity",
        });
    }
    match owner {
        ImmutableOwner::ZeroSector { sector, .. } => Ok(ImmutableOwnerRouteCoverage::Sector(
            try_preimage_mask(sector, source_for_target)?,
        )),
        ImmutableOwner::Factorization { domain, .. }
        | ImmutableOwner::SolvedRewriteSector { domain, .. } => {
            let raw_sector = try_preimage_mask(domain.sector(), source_for_target)?;
            let mut raw_bounds = try_empty_slots(arity, "owner-route factorization bounds")?;
            for (target, &source) in source_for_target.iter().enumerate() {
                let bounds = domain.bounds()[target];
                install_slot(
                    &mut raw_bounds,
                    source,
                    (bounds.lower(), bounds.upper()),
                    "owner-route factorization bounds",
                )?;
            }
            let raw_bounds = collect_slots(raw_bounds, "owner-route factorization bounds")?;
            // Recheck that exact transport preserved a nonempty rectangle
            // wholly inside the transported sector. Cold replay performs the
            // same validation before comparing stored coverage.
            SectorInteriorDomain::try_new(
                raw_sector.clone(),
                raw_bounds
                    .iter()
                    .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
            )?;
            Ok(ImmutableOwnerRouteCoverage::Domain {
                sector: raw_sector,
                bounds: raw_bounds.into(),
            })
        }
        ImmutableOwner::Master { key } => {
            let mut raw_powers = try_empty_slots(arity, "owner-route master powers")?;
            for (target, &source) in source_for_target.iter().enumerate() {
                install_slot(
                    &mut raw_powers,
                    source,
                    key.powers()[target],
                    "owner-route master powers",
                )?;
            }
            let raw_powers = collect_slots(raw_powers, "owner-route master powers")?;
            Ok(ImmutableOwnerRouteCoverage::Point {
                sector: Mask::try_from_indices(&raw_powers)?,
                powers: raw_powers.into(),
            })
        }
    }
}

fn try_preimage_mask(
    owner_sector: &Mask,
    source_for_target: &[usize],
) -> Result<Mask, StratumRegistryError> {
    let mut raw = try_empty_slots(owner_sector.arity(), "owner-route sector bits")?;
    for (target, &source) in source_for_target.iter().enumerate() {
        install_slot(
            &mut raw,
            source,
            owner_sector.active_bits()[target],
            "owner-route sector bits",
        )?;
    }
    Ok(Mask::try_new(collect_slots(
        raw,
        "owner-route sector bits",
    )?)?)
}

fn try_empty_slots<T>(
    arity: usize,
    resource: &'static str,
) -> Result<Vec<Option<T>>, StratumRegistryError> {
    let mut slots = Vec::new();
    try_reserve(&mut slots, arity, resource)?;
    slots.resize_with(arity, || None);
    Ok(slots)
}

fn install_slot<T>(
    slots: &mut [Option<T>],
    source: usize,
    value: T,
    _resource: &'static str,
) -> Result<(), StratumRegistryError> {
    let Some(slot) = slots.get_mut(source) else {
        return Err(StratumRegistryError::Invariant {
            detail: "authenticated owner route points outside its arity",
        });
    };
    if slot.replace(value).is_some() {
        return Err(StratumRegistryError::Invariant {
            detail: "authenticated owner route is not injective",
        });
    }
    Ok(())
}

fn collect_slots<T>(
    slots: Vec<Option<T>>,
    resource: &'static str,
) -> Result<Vec<T>, StratumRegistryError> {
    if slots.iter().any(Option::is_none) {
        return Err(StratumRegistryError::Invariant {
            detail: "authenticated owner route is not surjective",
        });
    }
    let mut values = Vec::new();
    try_reserve(&mut values, slots.len(), resource)?;
    for value in slots {
        let Some(value) = value else {
            return Err(StratumRegistryError::Invariant {
                detail: "authenticated owner route is not surjective",
            });
        };
        values.push(value);
    }
    Ok(values)
}

fn append_coverage_identity(
    stable: &mut BoundedIdentityBuilder,
    coverage: &ImmutableOwnerRouteCoverage,
) -> Result<(), StratumRegistryError> {
    match coverage {
        ImmutableOwnerRouteCoverage::Sector(sector) => {
            stable.push("sector:")?;
            append_mask(stable, sector)
        }
        ImmutableOwnerRouteCoverage::Domain { sector, bounds } => {
            stable.push("domain:")?;
            append_mask(stable, sector)?;
            stable.push(":")?;
            for (position, &(lower, upper)) in bounds.iter().enumerate() {
                if position != 0 {
                    stable.push(",")?;
                }
                stable.push_i64(lower)?;
                stable.push("..")?;
                stable.push_i64(upper)?;
            }
            Ok(())
        }
        ImmutableOwnerRouteCoverage::Point { sector, powers } => {
            stable.push("point:")?;
            append_mask(stable, sector)?;
            stable.push(":")?;
            for (position, &power) in powers.iter().enumerate() {
                if position != 0 {
                    stable.push(",")?;
                }
                stable.push_i64(power)?;
            }
            Ok(())
        }
    }
}

fn append_mask(
    stable: &mut BoundedIdentityBuilder,
    sector: &Mask,
) -> Result<(), StratumRegistryError> {
    for &active in sector.active_bits() {
        stable.push(if active { "1" } else { "0" })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::foundry::artifact::derive_two_loop_unit_mass_sunset;
    use crate::sector::{InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain};

    use super::super::ImmutableOwner;
    use super::{
        ImmutableOwnerRouteCoverage, ImmutableOwnerTransport, StratumRegistryLimits,
        build_owner_route_index, try_append_owner_routes, verify_owner_routes,
    };

    #[test]
    fn asymmetric_s3_domain_keeps_all_preimages_and_noninvolutive_direction() {
        let artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let canonicalizer = artifact.canonicalizer().unwrap();
        assert_eq!(canonicalizer.group_order(), 6);
        let owners = [ImmutableOwner::Factorization {
            source_ordinal: 0,
            domain: SectorInteriorDomain::try_new(
                Mask::try_new([true; 3]).unwrap(),
                [
                    InteriorBounds::new(1, 2),
                    InteriorBounds::new(1, 3),
                    InteriorBounds::new(1, 4),
                ],
            )
            .unwrap(),
        }];
        let mut routes = Vec::new();
        try_append_owner_routes(
            &mut routes,
            &owners,
            0,
            Some(canonicalizer),
            artifact.family_fingerprint(),
            artifact.arity(),
            StratumRegistryLimits::default(),
        )
        .unwrap();
        assert_eq!(
            routes.len(),
            6,
            "equal masks must not collapse unequal boxes"
        );

        let cycle = routes
            .iter()
            .find(|route| {
                matches!(
                    &route.transport,
                    ImmutableOwnerTransport::Symmetry(witness)
                        if witness.source_for_target() == [1, 2, 0]
                )
            })
            .unwrap();
        let ImmutableOwnerTransport::Symmetry(cycle_witness) = &cycle.transport else {
            panic!("the nonidentity cycle must retain authenticated transport")
        };
        let ImmutableOwnerRouteCoverage::Domain { bounds, .. } = &cycle.coverage else {
            panic!("the synthetic owner must retain a routed rectangle")
        };
        assert_eq!(bounds.as_ref(), &[(1, 4), (1, 2), (1, 3)]);
        let raw_point = [4, 2, 3];
        let mut owner_point = [0; 3];
        cycle_witness
            .transport_into(&raw_point, &mut owner_point)
            .unwrap();
        assert_eq!(owner_point, [2, 3, 4]);
        let ImmutableOwner::Factorization { domain, .. } = &owners[0] else {
            unreachable!()
        };
        assert!(domain.contains(&owner_point).unwrap());
    }

    #[test]
    fn routed_solved_owner_respects_ordering_and_root_precedence() {
        let artifact = derive_two_loop_unit_mass_sunset().unwrap();
        let canonicalizer = artifact.canonicalizer().unwrap();
        let canonical_sector = Mask::try_new([true, true, false]).unwrap();
        let owners = [
            ImmutableOwner::Factorization {
                source_ordinal: 0,
                domain: SectorInteriorDomain::try_new(
                    canonical_sector.clone(),
                    [
                        InteriorBounds::new(1, 2),
                        InteriorBounds::new(1, 3),
                        InteriorBounds::new(0, 0),
                    ],
                )
                .unwrap(),
            },
            ImmutableOwner::SolvedRewriteSector {
                domain: SectorInteriorDomain::try_new(
                    canonical_sector,
                    [
                        InteriorBounds::new(1, 6),
                        InteriorBounds::new(1, 7),
                        InteriorBounds::new(-2, 0),
                    ],
                )
                .unwrap(),
                ordering: OrderingPolicy::default(),
                layer_ordinal: 0,
            },
        ];
        let limits = StratumRegistryLimits::default();
        let mut routes = Vec::new();
        try_append_owner_routes(
            &mut routes,
            &owners,
            0,
            Some(canonicalizer),
            artifact.family_fingerprint(),
            artifact.arity(),
            limits,
        )
        .unwrap();

        let solved_domain = match &owners[1] {
            ImmutableOwner::SolvedRewriteSector { domain, .. } => domain,
            _ => unreachable!(),
        };
        let direct_solved = routes
            .iter()
            .find(|route| route.owner_ordinal() == 1 && route.sector() == solved_domain.sector())
            .unwrap();
        assert!(direct_solved.covers(&owners[1], OrderingPolicy::default(), solved_domain,));
        let direct_outside = SectorInteriorDomain::try_new(
            solved_domain.sector().clone(),
            [
                InteriorBounds::new(1, 6),
                InteriorBounds::new(1, 7),
                InteriorBounds::new(-3, 0),
            ],
        )
        .unwrap();
        assert!(!direct_solved.covers(&owners[1], OrderingPolicy::default(), &direct_outside,));
        let (index, buckets) = build_owner_route_index(&routes, limits).unwrap();
        assert!(
            verify_owner_routes(
                &owners,
                &routes,
                &index,
                &buckets,
                Some(canonicalizer),
                artifact.family_fingerprint(),
                artifact.arity(),
                limits,
            )
            .unwrap()
        );

        let raw_sector = Mask::try_new([false, true, true]).unwrap();
        let factor_route = routes
            .iter()
            .find(|route| route.owner_ordinal() == 0 && route.sector() == &raw_sector)
            .unwrap();
        let ImmutableOwnerRouteCoverage::Domain { bounds, .. } = &factor_route.coverage else {
            unreachable!()
        };
        let factor_target = SectorInteriorDomain::try_new(
            raw_sector.clone(),
            bounds
                .iter()
                .map(|&(lower, upper)| InteriorBounds::new(lower, upper)),
        )
        .unwrap();
        let bucket = &buckets[buckets
            .binary_search_by(|bucket| bucket.sector().cmp(&raw_sector))
            .unwrap()];
        let ordinals = bucket.route_ordinals(&index).unwrap();
        let selected = ordinals
            .iter()
            .copied()
            .find_map(|route_ordinal| {
                let route = &routes[route_ordinal];
                route
                    .covers(
                        &owners[route.owner_ordinal()],
                        OrderingPolicy::default(),
                        &factor_target,
                    )
                    .then_some(route.owner_ordinal())
            })
            .unwrap();
        assert_eq!(selected, 0, "root factorization must precede solved alias");

        let widened = SectorInteriorDomain::try_new(
            raw_sector,
            factor_target
                .bounds()
                .iter()
                .enumerate()
                .map(|(slot, &bounds)| {
                    if slot == 0 {
                        InteriorBounds::new(-1, 0)
                    } else {
                        bounds
                    }
                }),
        )
        .unwrap();
        let select = |ordering| {
            ordinals.iter().copied().find_map(|route_ordinal| {
                let route = &routes[route_ordinal];
                route
                    .covers(&owners[route.owner_ordinal()], ordering, &widened)
                    .then_some(route.owner_ordinal())
            })
        };
        assert_eq!(select(OrderingPolicy::default()), Some(1));
        assert_eq!(select(OrderingPolicy::TestOnlyDistinct), None);

        let symmetry_outside = SectorInteriorDomain::try_new(
            widened.sector().clone(),
            widened.bounds().iter().enumerate().map(|(slot, &bounds)| {
                if slot == 0 {
                    InteriorBounds::new(-3, 0)
                } else {
                    bounds
                }
            }),
        )
        .unwrap();
        let select_outside = |ordering| {
            ordinals.iter().copied().find_map(|route_ordinal| {
                let route = &routes[route_ordinal];
                route
                    .covers(&owners[route.owner_ordinal()], ordering, &symmetry_outside)
                    .then_some(route.owner_ordinal())
            })
        };
        assert_eq!(select_outside(OrderingPolicy::default()), None);
    }
}
