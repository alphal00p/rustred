//! Proposal-only K6 requested-domain resolution.
//!
//! External anchors and autonomous uncovered-box leaders meet at this small
//! value boundary. Both become the same planner-owned
//! [`RequestedDomain`](crate::foundry::completion::source_discovery::leader_walk::RequestedDomain)
//! values; neither path can carry a source row, coefficient, solved rule, or
//! closure claim into the exact compiler.

use std::collections::BTreeSet;

#[cfg(test)]
use crate::foundry::completion::LatticePoint;
use crate::foundry::completion::source_discovery::leader_walk::LeaderWalkLimits;
#[cfg(test)]
use crate::foundry::completion::source_discovery::leader_walk::RequestedDomain;
use crate::foundry::completion::stratum::ImmutableOwnerSnapshot;
use crate::foundry::completion::{SectorChart, UncoveredPartition};
use crate::sector::{CoordinatePriority, Mask};

use super::{
    FoundryCampaignConfig, FoundryCampaignError, FoundryCampaignSetupStage, FoundrySearchProvenance,
};

/// Stable semantic scope shared by explicit K6 requested-domain plans and any
/// detached support sidecar intended for those plans.
pub(crate) const K6_REQUESTED_DOMAIN_SCOPE_KEY: &str = "rustred.k6.requested-domains.v1";

/// Canonical proposal geometry detached from any planner epoch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct K6RequestedDomainSpec {
    point: Box<[u64]>,
    symbolic_axes: Box<[usize]>,
}

impl K6RequestedDomainSpec {
    pub(super) fn new(point: Box<[u64]>, symbolic_axes: Box<[usize]>) -> Self {
        Self {
            point,
            symbolic_axes,
        }
    }

    #[cfg(test)]
    pub(crate) fn materialize(&self) -> Result<RequestedDomain, FoundryCampaignError> {
        let point = LatticePoint::try_new(self.point.iter().copied()).map_err(|error| {
            FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
        })?;
        Ok(RequestedDomain::new(
            point,
            self.symbolic_axes.iter().copied(),
        ))
    }

    pub(crate) fn retained_coordinate_cells(&self) -> Result<usize, FoundryCampaignError> {
        self.point
            .len()
            .checked_add(self.symbolic_axes.len())
            .ok_or(FoundryCampaignError::ResourceCountOverflow {
                stage: FoundryCampaignSetupStage::RequestedDomains,
                resource: "combined K6 requested-domain coordinate cells",
            })
    }

    pub(super) fn point(&self) -> &[u64] {
        &self.point
    }

    pub(super) fn symbolic_axes(&self) -> &[usize] {
        &self.symbolic_axes
    }
}

/// Saturate reviewed external domains into one concrete K6 sector.
///
/// The authenticated canonicalizer is borrowed from the already installed
/// predecessor authority, so this cold ingress does not rebuild the K4 graph
/// action. Every authenticated route whose image stays in `sector` transports
/// the complete decorated rectangle (anchor and symbolic axes) before semantic
/// deduplication. Selecting only the canonical route would be unsound here:
/// distinct sector stabilizers can fix the undecorated anchor while rotating
/// its symbolic-axis decoration into inequivalent requested rectangles.
/// Search chronology is preserved first by input hint and then by the stable
/// authenticated group order.
pub(crate) fn try_resolve_external_k6_domains(
    config: &FoundryCampaignConfig,
    predecessor: &ImmutableOwnerSnapshot,
    sector: &Mask,
) -> Result<Vec<K6RequestedDomainSpec>, FoundryCampaignError> {
    if config.search_provenance() != FoundrySearchProvenance::ExternalHintsOnly {
        if config.domain_hints().is_empty() {
            return Ok(Vec::new());
        }
        return Err(FoundryCampaignError::Invariant {
            detail: "an autonomous campaign retained external requested-domain hints",
        });
    }
    let canonicalizer = predecessor
        .canonicalizer()
        .ok_or(FoundryCampaignError::Invariant {
            detail: "the K6 predecessor has no authenticated symmetry authority",
        })?;
    if canonicalizer.ordering() != config.ordering() || canonicalizer.arity() != sector.arity() {
        return Err(FoundryCampaignError::Invariant {
            detail: "requested-domain canonicalizer differs from the live K6 ledger scope",
        });
    }

    let mut seen = BTreeSet::new();
    let mut resolved = Vec::new();
    for hint in config.domain_hints() {
        for route in canonicalizer.routing_witnesses() {
            if !canonicalizer.authenticates_route(&route) {
                return Err(FoundryCampaignError::Invariant {
                    detail: "a requested-domain hint did not retain an authenticated K6 route",
                });
            }
            let mut routed_powers = vec![0_i64; canonicalizer.arity()];
            route
                .transport_into(hint.anchor().powers(), &mut routed_powers)
                .map_err(|error| {
                    FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
                })?;
            let routed_anchor =
                crate::family::IntegralKey::try_new(routed_powers).map_err(|error| {
                    FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
                })?;
            if !route.verify(hint.anchor(), &routed_anchor) {
                return Err(FoundryCampaignError::Invariant {
                    detail: "an authenticated K6 requested-domain route failed exact replay",
                });
            }
            let routed_sector =
                Mask::try_from_indices(routed_anchor.powers()).map_err(|error| {
                    FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
                })?;
            if &routed_sector != sector {
                continue;
            }
            let point = SectorChart::new(routed_sector)
                .to_lattice(&routed_anchor)
                .map_err(|error| {
                    FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
                })?;
            let symbolic_axes = route
                .source_for_target()
                .iter()
                .enumerate()
                .filter_map(|(target, &raw_source)| {
                    hint.symbolic_axes()
                        .binary_search(&raw_source)
                        .is_ok()
                        .then_some(target)
                })
                .collect::<Vec<_>>();
            let semantic_key = (point.coordinates().to_vec(), symbolic_axes);
            if seen.insert(semantic_key.clone()) {
                resolved.push(K6RequestedDomainSpec::new(
                    semantic_key.0.into_boxed_slice(),
                    semantic_key.1.into_boxed_slice(),
                ));
            }
        }
    }
    Ok(resolved)
}

/// Derive a no-hint proposal itinerary from the exact live complement.
///
/// Every box contributes all fair simplex-shell anchors through `shell_depth`
/// on exactly its unbounded axes. This is a counterexample-guided search
/// schedule, not a completeness test: a stable pass must still be followed by
/// the generic boundary complement, and only the exact compiler may declare
/// closure.
pub(crate) fn try_resolve_autonomous_k6_shells(
    partition: &UncoveredPartition,
    shell_depth: usize,
    priority: &CoordinatePriority,
    limits: LeaderWalkLimits,
) -> Result<Vec<K6RequestedDomainSpec>, FoundryCampaignError> {
    if partition
        .boxes()
        .iter()
        .any(|lattice_box| lattice_box.arity() != priority.arity())
    {
        return Err(FoundryCampaignError::Invariant {
            detail: "autonomous K6 shell priority differs from uncovered geometry arity",
        });
    }
    let admitted_domain_count =
        try_admit_autonomous_k6_shells(partition, shell_depth, priority.arity(), limits)?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(partition.boxes().len())
        .map_err(|_| FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            message: format!(
                "could not reserve {} autonomous K6 uncovered-box references",
                partition.boxes().len()
            ),
        })?;
    boxes.extend(partition.boxes());
    boxes.sort_unstable_by(|left, right| {
        right
            .free_dimension()
            .cmp(&left.free_dimension())
            .then_with(|| left.lower().cmp(right.lower()))
            .then_with(|| left.upper().cmp(right.upper()))
    });

    let mut resolved = Vec::new();
    resolved
        .try_reserve_exact(admitted_domain_count)
        .map_err(|_| FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            message: format!(
                "could not reserve {admitted_domain_count} autonomous K6 shell domains"
            ),
        })?;
    for free_dimension in (0..=priority.arity()).rev() {
        for degree in 0..=shell_depth {
            for lattice_box in boxes
                .iter()
                .copied()
                .filter(|cell| cell.free_dimension() == free_dimension)
            {
                let mut symbolic_axes = Vec::new();
                symbolic_axes
                    .try_reserve_exact(lattice_box.free_dimension())
                    .map_err(|_| FoundryCampaignError::Setup {
                        stage: FoundryCampaignSetupStage::RequestedDomains,
                        message: format!(
                            "could not reserve {} autonomous K6 symbolic axes",
                            lattice_box.free_dimension()
                        ),
                    })?;
                symbolic_axes.extend(
                    lattice_box
                        .upper()
                        .iter()
                        .enumerate()
                        .filter_map(|(axis, upper)| upper.is_none().then_some(axis)),
                );
                let mut priority_axes = Vec::new();
                priority_axes
                    .try_reserve_exact(symbolic_axes.len())
                    .map_err(|_| FoundryCampaignError::Setup {
                        stage: FoundryCampaignSetupStage::RequestedDomains,
                        message: format!(
                            "could not reserve {} autonomous K6 priority axes",
                            symbolic_axes.len()
                        ),
                    })?;
                priority_axes.extend_from_slice(&symbolic_axes);
                priority_axes.sort_unstable_by_key(|&axis| priority.rank_by_slot()[axis]);
                let mut delta = Vec::new();
                delta.try_reserve_exact(priority.arity()).map_err(|_| {
                    FoundryCampaignError::Setup {
                        stage: FoundryCampaignSetupStage::RequestedDomains,
                        message: format!(
                            "could not reserve {} autonomous K6 shell coordinates",
                            priority.arity()
                        ),
                    }
                })?;
                delta.resize(priority.arity(), 0_u64);
                try_enumerate_shell(&priority_axes, 0, degree, &mut delta, &mut |delta| {
                    let mut point = Vec::new();
                    point.try_reserve_exact(priority.arity()).map_err(|_| {
                        FoundryCampaignError::Setup {
                            stage: FoundryCampaignSetupStage::RequestedDomains,
                            message: format!(
                                "could not reserve {} autonomous K6 anchor coordinates",
                                priority.arity()
                            ),
                        }
                    })?;
                    point.extend_from_slice(lattice_box.lower());
                    for &axis in &priority_axes {
                        point[axis] = point[axis].checked_add(delta[axis]).ok_or(
                            FoundryCampaignError::Invariant {
                                detail: "autonomous K6 shell anchor overflowed u64",
                            },
                        )?;
                    }
                    let mut retained_axes = Vec::new();
                    retained_axes
                        .try_reserve_exact(symbolic_axes.len())
                        .map_err(|_| FoundryCampaignError::Setup {
                            stage: FoundryCampaignSetupStage::RequestedDomains,
                            message: format!(
                                "could not reserve {} autonomous K6 retained symbolic axes",
                                symbolic_axes.len()
                            ),
                        })?;
                    retained_axes.extend_from_slice(&symbolic_axes);
                    // The exact uncovered partition is disjoint, and every
                    // emitted anchor remains inside its originating box.
                    // Weak compositions are unique within a box, so no
                    // allocation-heavy post-generation deduplication is
                    // needed for this autonomous lane.
                    resolved.push(K6RequestedDomainSpec::new(
                        point.into_boxed_slice(),
                        retained_axes.into_boxed_slice(),
                    ));
                    Ok(())
                })?;
            }
        }
    }
    if resolved.len() != admitted_domain_count {
        return Err(FoundryCampaignError::Invariant {
            detail: "autonomous K6 shell enumeration disagreed with checked admission",
        });
    }
    Ok(resolved)
}

fn try_admit_autonomous_k6_shells(
    partition: &UncoveredPartition,
    shell_depth: usize,
    arity: usize,
    limits: LeaderWalkLimits,
) -> Result<usize, FoundryCampaignError> {
    check_limit("autonomous K6 shell arity", arity, limits.max_arity)?;
    check_limit(
        "autonomous K6 uncovered boxes",
        partition.boxes().len(),
        limits.max_input_boxes,
    )?;
    let box_coordinate_cells = checked_product(
        "autonomous K6 uncovered-box coordinate cells",
        partition.boxes().len(),
        checked_product("autonomous K6 uncovered-box coordinate cells", arity, 2)?,
    )?;
    check_limit(
        "autonomous K6 uncovered-box coordinate cells",
        box_coordinate_cells,
        limits.max_input_box_coordinate_cells,
    )?;

    let mut domains = 0usize;
    let mut coordinate_cells = 0usize;
    let mut symbolic_axis_cells = 0usize;
    for lattice_box in partition.boxes() {
        let free_dimension = lattice_box.free_dimension();
        let remaining = limits.max_tasks.saturating_sub(domains);
        let box_domains = try_count_cumulative_simplex_shell(
            shell_depth,
            free_dimension,
            remaining,
            limits.max_tasks,
        )?;
        domains = checked_sum("autonomous K6 shell domains", domains, box_domains)?;
        check_limit("autonomous K6 shell domains", domains, limits.max_tasks)?;

        let cells_per_domain = checked_sum(
            "autonomous K6 shell coordinate cells",
            arity,
            free_dimension,
        )?;
        coordinate_cells = checked_sum(
            "autonomous K6 shell coordinate cells",
            coordinate_cells,
            checked_product(
                "autonomous K6 shell coordinate cells",
                box_domains,
                cells_per_domain,
            )?,
        )?;
        check_limit(
            "autonomous K6 shell coordinate cells",
            coordinate_cells,
            limits.max_task_coordinate_cells,
        )?;
        symbolic_axis_cells = checked_sum(
            "autonomous K6 shell symbolic-axis cells",
            symbolic_axis_cells,
            checked_product(
                "autonomous K6 shell symbolic-axis cells",
                box_domains,
                free_dimension,
            )?,
        )?;
        check_limit(
            "autonomous K6 shell symbolic-axis cells",
            symbolic_axis_cells,
            limits.max_selected_free_axis_cells,
        )?;
    }
    Ok(domains)
}

/// Count all weak-composition shells of `free_dimension` axes through one
/// depth. For positive dimension this is `C(shell_depth + d, d)`; a zero-axis
/// box contributes only its degree-zero corner.
fn try_count_cumulative_simplex_shell(
    shell_depth: usize,
    free_dimension: usize,
    remaining_limit: usize,
    aggregate_limit: usize,
) -> Result<usize, FoundryCampaignError> {
    if free_dimension == 0 {
        if remaining_limit == 0 {
            return Err(resource_limit_after(
                aggregate_limit,
                "autonomous K6 shell domains",
            )?);
        }
        return Ok(1);
    }
    let n = shell_depth.checked_add(free_dimension).ok_or(
        FoundryCampaignError::ResourceCountOverflow {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "autonomous K6 shell domains",
        },
    )?;
    let k = shell_depth.min(free_dimension);
    let mut count = 1_u128;
    if count > remaining_limit as u128 {
        return Err(resource_limit_after(
            aggregate_limit,
            "autonomous K6 shell domains",
        )?);
    }
    for step in 1..=k {
        let numerator = n - k + step;
        count = count.checked_mul(numerator as u128).ok_or(
            FoundryCampaignError::ResourceCountOverflow {
                stage: FoundryCampaignSetupStage::RequestedDomains,
                resource: "autonomous K6 shell domains",
            },
        )? / step as u128;
        if count > remaining_limit as u128 {
            return Err(resource_limit_after(
                aggregate_limit,
                "autonomous K6 shell domains",
            )?);
        }
    }
    usize::try_from(count).map_err(|_| FoundryCampaignError::ResourceCountOverflow {
        stage: FoundryCampaignSetupStage::RequestedDomains,
        resource: "autonomous K6 shell domains",
    })
}

fn resource_limit_after(
    limit: usize,
    resource: &'static str,
) -> Result<FoundryCampaignError, FoundryCampaignError> {
    let requested = limit
        .checked_add(1)
        .ok_or(FoundryCampaignError::ResourceCountOverflow {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource,
        })?;
    Ok(FoundryCampaignError::ResourceLimit {
        stage: FoundryCampaignSetupStage::RequestedDomains,
        resource,
        requested,
        limit,
    })
}

fn checked_sum(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, FoundryCampaignError> {
    left.checked_add(right)
        .ok_or(FoundryCampaignError::ResourceCountOverflow {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource,
        })
}

fn checked_product(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, FoundryCampaignError> {
    left.checked_mul(right)
        .ok_or(FoundryCampaignError::ResourceCountOverflow {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource,
        })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FoundryCampaignError> {
    if requested > limit {
        Err(FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_enumerate_shell(
    priority_axes: &[usize],
    axis_ordinal: usize,
    remaining: usize,
    delta: &mut [u64],
    emit: &mut impl FnMut(&[u64]) -> Result<(), FoundryCampaignError>,
) -> Result<(), FoundryCampaignError> {
    let Some(&axis) = priority_axes.get(axis_ordinal) else {
        return (remaining == 0)
            .then(|| emit(delta))
            .transpose()
            .map(|_| ());
    };
    if axis_ordinal + 1 == priority_axes.len() {
        delta[axis] = u64::try_from(remaining).map_err(|_| FoundryCampaignError::Invariant {
            detail: "autonomous K6 shell degree did not fit u64",
        })?;
        emit(delta)?;
        delta[axis] = 0;
        return Ok(());
    }
    // Rank zero is considered first. Assign its largest possible coordinate
    // first so the generated weak compositions follow that semantic priority
    // instead of accidentally privileging the last/lowest-priority axis.
    for coordinate in (0..=remaining).rev() {
        delta[axis] = u64::try_from(coordinate).map_err(|_| FoundryCampaignError::Invariant {
            detail: "autonomous K6 shell coordinate did not fit u64",
        })?;
        try_enumerate_shell(
            priority_axes,
            axis_ordinal + 1,
            remaining - coordinate,
            delta,
            emit,
        )?;
    }
    delta[axis] = 0;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::family::IntegralKey;
    use crate::foundry::artifact::{
        FULL_RANK_ORBITS, materialize_alpha_loop_lhs_anchors_with_ordering,
    };
    use crate::foundry::completion::{BoxCover, CompletionGeometryLimits, LatticeBox};
    use crate::sector::{CoordinatePriority, OrderingPolicy};

    use super::*;
    use crate::foundry::campaign::preset_k6::k6_root_predecessor_for_ordering;
    use crate::foundry::campaign::{
        FoundryCampaignDomainHint, FoundryCampaignExternalHints, FoundryCampaignItinerary,
        FoundryCampaignPreset, FoundryCampaignProbe,
    };

    fn winner_ordering() -> OrderingPolicy {
        let priority = CoordinatePriority::try_new(6, &[5, 3, 4, 2, 0, 1], Default::default())
            .expect("winner coordinate priority");
        OrderingPolicy::try_with_coordinate_priority(&priority).expect("winner ordering")
    }

    fn external_config(
        ordering: OrderingPolicy,
        domains: impl IntoIterator<Item = FoundryCampaignDomainHint>,
    ) -> FoundryCampaignConfig {
        let hints = FoundryCampaignExternalHints::try_new_with_domains(
            FoundryCampaignItinerary::SingleSectorFixedPoint,
            [FoundryCampaignProbe::try_new(1_000_000_007, [37], [0, 0, 0, 0, 0, 0]).unwrap()],
            2,
            0,
            ordering,
            None,
            domains,
        )
        .unwrap();
        FoundryCampaignConfig::try_external_hints(
            FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0,
            hints,
            16,
            16,
        )
        .unwrap()
    }

    #[test]
    fn external_domains_use_the_installed_s4_authority_and_semantic_deduplication() {
        let anchor = IntegralKey::try_new([0, 0, 1, 0, 1, 1]).unwrap();
        let domain = || FoundryCampaignDomainHint::try_new(anchor.clone(), [2, 4, 5]).unwrap();
        let config = external_config(OrderingPolicy::default(), [domain(), domain()]);
        let predecessor = k6_root_predecessor_for_ordering(OrderingPolicy::default()).unwrap();
        let sector = Mask::try_from_indices(anchor.powers()).unwrap();

        let resolved = try_resolve_external_k6_domains(&config, &predecessor, &sector).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].point(), [0, 0, 0, 0, 0, 0]);
        assert_eq!(resolved[0].symbolic_axes().len(), 3);
        assert!(
            resolved[0]
                .symbolic_axes()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );
        let materialized = resolved[0].materialize().unwrap();
        assert_eq!(materialized.point().coordinates(), resolved[0].point());
        assert_eq!(materialized.symbolic_axes(), resolved[0].symbolic_axes());
    }

    #[test]
    fn full_rank_stabilizer_saturates_the_axis_decoration_under_every_ordering() {
        let anchor = IntegralKey::try_new([1, 1, 1, 1, 1, 1]).unwrap();
        let sector = Mask::try_from_indices(anchor.powers()).unwrap();
        for ordering in [OrderingPolicy::default(), winner_ordering()] {
            let domain =
                FoundryCampaignDomainHint::try_new(anchor.clone(), [0]).expect("valid domain");
            let config = external_config(ordering, [domain.clone(), domain]);
            let predecessor = k6_root_predecessor_for_ordering(ordering).unwrap();
            let resolved = try_resolve_external_k6_domains(&config, &predecessor, &sector).unwrap();

            assert_eq!(resolved.len(), 6);
            assert!(resolved.iter().all(|domain| domain.point() == [0; 6]));
            assert_eq!(
                resolved
                    .iter()
                    .map(|domain| domain.symbolic_axes().to_vec())
                    .collect::<BTreeSet<_>>(),
                (0..6).map(|axis| vec![axis]).collect::<BTreeSet<_>>()
            );
        }
    }

    #[test]
    fn fifty_five_lhs_rectangles_reproduce_the_finite_complement_certificate() {
        let natural = saturated_lhs_rectangles(OrderingPolicy::default());
        let winner = saturated_lhs_rectangles(winner_ordering());
        assert_eq!(winner, natural);
        assert_eq!(natural.len(), 5);

        for (sector, domains) in natural {
            let boxes = domains
                .iter()
                .map(|domain| {
                    let upper = domain.point().iter().enumerate().map(|(axis, &lower)| {
                        domain
                            .symbolic_axes()
                            .binary_search(&axis)
                            .is_err()
                            .then_some(lower)
                    });
                    LatticeBox::try_new(domain.point().iter().copied(), upper).unwrap()
                })
                .collect::<Vec<_>>();
            let residual =
                BoxCover::try_new(sector.arity(), boxes, CompletionGeometryLimits::default())
                    .unwrap()
                    .uncovered_partition()
                    .unwrap();
            assert!(
                residual
                    .boxes()
                    .iter()
                    .all(|cell| cell.varying_dimension() == 0),
                "saturated LHS rectangles left a positive-dimensional complement in {sector:?}: {residual:?}"
            );
        }
    }

    fn saturated_lhs_rectangles(
        ordering: OrderingPolicy,
    ) -> BTreeMap<Mask, Vec<K6RequestedDomainSpec>> {
        let domains = materialize_alpha_loop_lhs_anchors_with_ordering(ordering)
            .into_iter()
            .map(|entry| {
                let symbolic_axes = entry
                    .source
                    .symbolic_bits
                    .bytes()
                    .enumerate()
                    .filter_map(|(axis, bit)| (bit == b'1').then_some(axis));
                FoundryCampaignDomainHint::try_new(entry.raw_integral, symbolic_axes).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(domains.len(), 55);
        let config = external_config(ordering, domains);
        let predecessor = k6_root_predecessor_for_ordering(ordering).unwrap();
        let mut per_sector = BTreeMap::new();
        for orbit in FULL_RANK_ORBITS {
            let sector = Mask::try_from_indices(&orbit.representative).unwrap();
            let resolved = try_resolve_external_k6_domains(&config, &predecessor, &sector).unwrap();
            if !resolved.is_empty() {
                per_sector.insert(sector, resolved);
            }
        }
        per_sector
    }

    #[test]
    fn autonomous_leaders_follow_only_the_exact_live_box_geometry() {
        let partition = BoxCover::try_new(2, [], CompletionGeometryLimits::default())
            .unwrap()
            .uncovered_partition()
            .unwrap();
        let priority = CoordinatePriority::try_natural(2, Default::default()).unwrap();
        let resolved =
            try_resolve_autonomous_k6_shells(&partition, 1, &priority, LeaderWalkLimits::default())
                .unwrap();
        assert_eq!(resolved.len(), 3);
        assert_eq!(resolved[0].point(), [0, 0]);
        assert_eq!(resolved[0].symbolic_axes(), [0, 1]);
        assert_eq!(resolved[1].point(), [1, 0]);
        assert_eq!(resolved[2].point(), [0, 1]);
        assert!(
            resolved[1..]
                .iter()
                .all(|domain| domain.symbolic_axes() == [0, 1])
        );
    }

    #[test]
    fn autonomous_leaders_follow_a_nonnatural_coordinate_priority() {
        let partition = BoxCover::try_new(2, [], CompletionGeometryLimits::default())
            .unwrap()
            .uncovered_partition()
            .unwrap();
        let priority = CoordinatePriority::try_new(2, &[1, 0], Default::default()).unwrap();
        let resolved =
            try_resolve_autonomous_k6_shells(&partition, 1, &priority, LeaderWalkLimits::default())
                .unwrap();
        assert_eq!(resolved[0].point(), [0, 0]);
        assert_eq!(resolved[1].point(), [0, 1]);
        assert_eq!(resolved[2].point(), [1, 0]);
    }

    #[test]
    fn autonomous_shells_are_admitted_before_any_combinatorial_enumeration() {
        let partition = BoxCover::try_new(2, [], CompletionGeometryLimits::default())
            .unwrap()
            .uncovered_partition()
            .unwrap();
        let priority = CoordinatePriority::try_natural(2, Default::default()).unwrap();
        let limits = LeaderWalkLimits {
            max_tasks: 5,
            ..LeaderWalkLimits::default()
        };
        assert_eq!(
            try_resolve_autonomous_k6_shells(&partition, 2, &priority, limits).unwrap_err(),
            FoundryCampaignError::ResourceLimit {
                stage: FoundryCampaignSetupStage::RequestedDomains,
                resource: "autonomous K6 shell domains",
                requested: 6,
                limit: 5,
            }
        );

        let coordinate_limits = LeaderWalkLimits {
            max_task_coordinate_cells: 3,
            ..LeaderWalkLimits::default()
        };
        assert_eq!(
            try_resolve_autonomous_k6_shells(&partition, 0, &priority, coordinate_limits)
                .unwrap_err(),
            FoundryCampaignError::ResourceLimit {
                stage: FoundryCampaignSetupStage::RequestedDomains,
                resource: "autonomous K6 shell coordinate cells",
                requested: 4,
                limit: 3,
            }
        );
    }
}
