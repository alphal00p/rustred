use super::super::RoutedCandidateReducer;
use super::model::*;
use super::power::{mapped_bounds, project_cover};
use super::support::{JointSourceSupport, NumeratorDegrees};
use crate::solver::candidate_reduction::power_domain::{
    DomainPowerBounds, DomainPowerError, project,
};
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicBool, Ordering};

fn cancelled(cancel: &AtomicBool) -> Result<(), CandidateDomainRouteFailure> {
    if cancel.load(Ordering::Acquire) {
        Err(CandidateDomainRouteFailure::Cancelled)
    } else {
        Ok(())
    }
}
fn admit(
    current: usize,
    addition: usize,
    limit: usize,
    resource: &'static str,
) -> Result<usize, CandidateDomainRouteFailure> {
    let requested = current
        .checked_add(addition)
        .ok_or(CandidateDomainRouteFailure::CountOverflow { resource })?;
    if requested > limit {
        Err(CandidateDomainRouteFailure::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(requested)
    }
}

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Initial and route-generated nonliteral requests need source admission before
    /// treating a route cover as completed work. Target validity does not imply
    /// source validity. RHS visitors already preserve this per-original-term
    /// obligation; this getter merely reports whether such conditions exist.
    pub fn domain_routing_requires_source_conditions(&self) -> bool {
        !self.programs.context.shared.source_conditions.is_empty()
    }
    /// Stream a conservative domain route cover from already admitted maps.
    /// No coefficient arithmetic, graph matching or numerator expansion occurs.
    /// The caller retains source validity and destination obligations, and must
    /// distinguish Route/Apply phases. Actual rank is never readmitted against
    /// saved entry scope. Errors/cancellation leave only an incomplete prefix.
    pub fn visit_domain_route_overcover(
        &self,
        source: [bool; N],
        actual_rank: Option<u32>,
        limits: CandidateDomainRouteLimits,
        cancellation: &AtomicBool,
        visit: impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    ) -> Result<CandidateDomainRouteStats, CandidateDomainRouteError> {
        self.visit_bounded_domain_route_overcover(
            source,
            &[0; N],
            &[None; N],
            actual_rank,
            limits,
            cancellation,
            visit,
        )
    }

    /// Stream a conservative image of a local-coordinate source box. Literal
    /// owners retain that exact box. Verified nonliteral maps preserve mapped
    /// positive upper bounds, but numerator cancellation can lower positive
    /// powers, and affine numerator substitution does not permute inactive
    /// bounds. The latter coordinates are conservatively rank-bounded only.
    ///
    /// Inverted or wrong-arity boxes fail explicitly. A box whose inactive
    /// lower bounds exceed its rank budget is empty and emits no events.
    /// Source validity is a separate obligation even for known-zero sectors.
    pub fn visit_bounded_domain_route_overcover(
        &self,
        source: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        actual_rank: Option<u32>,
        limits: CandidateDomainRouteLimits,
        cancellation: &AtomicBool,
        visit: impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    ) -> Result<CandidateDomainRouteStats, CandidateDomainRouteError> {
        self.visit_power_bounded_domain_route_overcover(
            source,
            lower,
            upper,
            actual_rank,
            DomainPowerBounds::default(),
            limits,
            cancellation,
            visit,
        )
    }

    /// Route a box intersected with retained total-positive-power and A-R bounds.
    /// Source and mapped covers are projected before enumerating further masks;
    /// verified inactive-row support bounds each active target's cancellable
    /// degree, retaining surviving positive lowers and rejecting impossible
    /// pinches without expanding numerator polynomials.
    /// omitted masks are proved empty, not missing-rule or terminal claims.
    /// The unconstrained bounds value retains the existing bounded visitor's
    /// exact output and accounting, including its unbounded-rank representation.
    pub fn visit_power_bounded_domain_route_overcover(
        &self,
        source: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        actual_rank: Option<u32>,
        power_bounds: DomainPowerBounds,
        limits: CandidateDomainRouteLimits,
        cancellation: &AtomicBool,
        visit: impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    ) -> Result<CandidateDomainRouteStats, CandidateDomainRouteError> {
        self.visit_power_bounded_domain_route_overcover_with_options(
            source,
            lower,
            upper,
            actual_rank,
            power_bounds,
            limits,
            CandidateDomainRouteOptions::default(),
            cancellation,
            visit,
        )
    }

    /// As the default visitor, with opt-in necessary joint mask exclusions.
    /// Passing the bound never asserts an attainable endpoint. Every examined
    /// mask, including a joint exclusion, still consumes the mask allowance.
    pub fn visit_power_bounded_domain_route_overcover_with_options(
        &self,
        source: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        actual_rank: Option<u32>,
        power_bounds: DomainPowerBounds,
        limits: CandidateDomainRouteLimits,
        options: CandidateDomainRouteOptions,
        cancellation: &AtomicBool,
        mut visit: impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    ) -> Result<CandidateDomainRouteStats, CandidateDomainRouteError> {
        let mut stats = CandidateDomainRouteStats::default();
        let result = self.route_cover(
            source,
            lower,
            upper,
            actual_rank,
            power_bounds,
            limits,
            options,
            cancellation,
            &mut visit,
            &mut stats,
        );
        result
            .map(|()| stats)
            .map_err(|failure| CandidateDomainRouteError { failure, stats })
    }

    fn route_cover(
        &self,
        source: [bool; N],
        lower: &[u64],
        upper: &[Option<u64>],
        actual_rank: Option<u32>,
        power_bounds: DomainPowerBounds,
        limits: CandidateDomainRouteLimits,
        options: CandidateDomainRouteOptions,
        cancellation: &AtomicBool,
        visit: &mut impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
        stats: &mut CandidateDomainRouteStats,
    ) -> Result<(), CandidateDomainRouteFailure> {
        cancelled(cancellation)?;
        let lower: [u64; N] = lower.try_into().map_err(|_| {
            CandidateDomainRouteFailure::InvalidDomain("source lower-bound arity differs")
        })?;
        let upper: [Option<u64>; N] = upper.try_into().map_err(|_| {
            CandidateDomainRouteFailure::InvalidDomain("source upper-bound arity differs")
        })?;
        if lower
            .iter()
            .zip(upper)
            .any(|(&lo, hi)| hi.is_some_and(|hi| lo > hi))
        {
            return Err(CandidateDomainRouteFailure::InvalidDomain(
                "source lower bound exceeds upper bound",
            ));
        }
        power_bounds
            .validate()
            .map_err(CandidateDomainRouteFailure::PowerDomain)?;
        if let Some(rank) = actual_rank {
            // Subtraction avoids an overflowing sum of otherwise valid u64
            // coordinate bounds. Failure here means an empty intersection.
            let mut remaining = u64::from(rank);
            for (axis, &on) in source.iter().enumerate() {
                if !on {
                    let Some(next) = remaining.checked_sub(lower[axis]) else {
                        return cancelled(cancellation);
                    };
                    remaining = next;
                }
            }
        }
        let constrained = !power_bounds.is_unconstrained();
        let projection = if constrained {
            let Some(projection) = project(&source, &lower, &upper, actual_rank, power_bounds)
                .map_err(CandidateDomainRouteFailure::PowerDomain)?
            else {
                return cancelled(cancellation);
            };
            Some(projection)
        } else {
            None
        };
        let lower = projection.as_ref().map_or(lower, |p| p.lower);
        let upper = projection.as_ref().map_or(upper, |p| p.upper);
        let actual_rank = projection
            .as_ref()
            .map_or(actual_rank, |p| p.effective_rank);
        // Source validity precedes zero in concrete evaluation. We report its
        // remaining obligation rather than treating a zero census as validity.
        if self.programs.context.shared.zero_sectors.contains(&source) {
            return emit(
                CandidateDomainRouteEvent::ZeroSector {
                    sector: source,
                    actual_rank,
                    power_bounds,
                    source_conditions_required: self.domain_routing_requires_source_conditions(),
                },
                limits,
                cancellation,
                visit,
                stats,
            );
        }
        if self.programs.owners.contains_key(&source) {
            return emit(
                CandidateDomainRouteEvent::Apply {
                    owner_sector: source,
                    cover: CandidateDomainRouteCover {
                        source_sector: source,
                        target_root: source,
                        lower,
                        upper,
                        actual_rank,
                        power_bounds,
                        conservative: true,
                    },
                },
                limits,
                cancellation,
                visit,
                stats,
            );
        }
        let Some(route) = self.routes.get(&source) else {
            return emit(
                CandidateDomainRouteEvent::MissingRoute {
                    source_sector: source,
                    actual_rank,
                    power_bounds,
                },
                limits,
                cancellation,
                visit,
                stats,
            );
        };
        if projection.as_ref().is_some_and(|p| {
            p.numerator_upper
                .is_some_and(|rank| rank > u128::from(u32::MAX))
        }) {
            // Literal boxes can retain this wider bound exactly. After affine
            // numerator mixing the rank interface cannot encode it; fail rather
            // than silently truncate it or turn a finite cap into infinity.
            return Err(CandidateDomainRouteFailure::PowerDomain(
                DomainPowerError::OutOfRange("routed numerator bound"),
            ));
        }
        let positive_upper = projection.as_ref().and_then(|p| p.positive_upper);
        let root: [bool; N] = std::array::from_fn(|axis| route.owner_sector.active_bits()[axis]);
        let count = root.iter().filter(|&&b| b).count();
        if count != source.iter().filter(|&&b| b).count() {
            return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                "active-root cardinality differs",
            ));
        }
        let active_target = route.transport.active_target_axes();
        if active_target.len() != N {
            return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                "active-map arity differs",
            ));
        }
        let mut target_upper = [None; N];
        let mut target_lower_cost = [0_u64; N];
        let mut seen = [false; N];
        for (axis, &target) in active_target.iter().enumerate() {
            match (source[axis], target) {
                (false, None) => {}
                (true, Some(target)) if target < N && root[target] && !seen[target] => {
                    seen[target] = true;
                    target_upper[target] = upper[axis];
                    // Keep the local lower bound without forming lower+1;
                    // finite-rank admission below checks subtraction first.
                    target_lower_cost[target] = lower[axis];
                }
                _ => {
                    return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                        "active map is not a source-to-root bijection",
                    ));
                }
            }
        }
        if seen != root {
            return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                "active map does not cover the target root",
            ));
        }
        let degrees = projection
            .as_ref()
            .map(|projected| {
                NumeratorDegrees::from_source(
                    &route.transport,
                    &source,
                    &root,
                    &projected.lower,
                    &projected.upper,
                    projected.numerator_upper,
                )
            })
            .transpose()?;
        let target_lower = std::array::from_fn(|axis| {
            degrees
                .as_ref()
                .filter(|_| root[axis])
                .map_or(0, |degrees| {
                    degrees.surviving_lower(axis, target_lower_cost[axis])
                })
        });
        let cover = CandidateDomainRouteCover {
            source_sector: source,
            target_root: root,
            lower: target_lower,
            upper: target_upper,
            actual_rank,
            power_bounds: if constrained {
                mapped_bounds(power_bounds, positive_upper, 0).expect("zero pinch cost")
            } else {
                power_bounds
            },
            conservative: true,
        };
        let root_cover = if constrained {
            project_cover(&root, cover)?
        } else {
            Some(cover)
        };
        if let Some(cover) = root_cover {
            emit(
                CandidateDomainRouteEvent::Apply {
                    owner_sector: root,
                    cover,
                },
                limits,
                cancellation,
                visit,
                stats,
            )?;
        } else {
            stats.masks_examined = admit(stats.masks_examined, 1, limits.max_masks, "route masks")?;
            stats.masks_pruned += 1;
        }

        // Prepared::compile admits a unit active-row bijection and only affine
        // inactive rows. Endpoints B-e have B>=0 and |e|<=D, so their numerator
        // rank is |e|-sum_j min(e_j,B_j). Removing k positive axes consumes
        // at least sum_j(lower_j+1) degree because each lost axis has
        // e_j>=B_j>=lower_j+1. Thus the strict-pinch endpoint rank is at most
        // D-sum_j(lower_j+1)<=R-sum_j(lower_j+1), even for unbounded positive
        // powers. Affine constants can only lower monomial degree; native
        // cancellation can only remove endpoints.
        // Enumerate only combinations of at most R removed axes.
        let max_removed = actual_rank.map_or(count, |r| {
            usize::try_from(r).unwrap_or(usize::MAX).min(count)
        });
        if max_removed == 0 {
            return cancelled(cancellation);
        }
        let mut joint_support = if options.joint_source_support_pruning && max_removed > 1 {
            Some(JointSourceSupport::new(
                &route.transport,
                &source,
                &lower,
                &upper,
                projection
                    .as_ref()
                    .map_or(actual_rank.map(u128::from), |p| p.numerator_upper),
            )?)
        } else {
            None
        };
        cancelled(cancellation)?;
        // Scratch holds O(N) coordinates, not all 2^k domains. The first emitted
        // domain already admitted 2*N logical cells before these allocations.
        let mut active = Vec::new();
        active.try_reserve_exact(count).map_err(|_| {
            CandidateDomainRouteFailure::AllocationFailure {
                resource: "active route axes",
            }
        })?;
        active.extend(
            root.iter()
                .enumerate()
                .filter_map(|(axis, &on)| on.then_some(axis)),
        );
        let mut positions = Vec::new();
        positions.try_reserve_exact(max_removed).map_err(|_| {
            CandidateDomainRouteFailure::AllocationFailure {
                resource: "removed route axes",
            }
        })?;
        for removed in 1..=max_removed {
            positions.clear();
            positions.extend(0..removed);
            loop {
                cancelled(cancellation)?;
                // Charge every examined complete subset, including impossible
                // support/weighted pinches. Preflight before scanning its coordinates.
                let masks = admit(stats.masks_examined, 1, limits.max_masks, "route masks")?;
                let mut sector = root;
                let mut pinched_rank = actual_rank;
                let mut possible = true;
                let mut pinch_cost = 0_u128;
                for &position in &positions {
                    let axis = active[position];
                    sector[axis] = false;
                    if degrees
                        .as_ref()
                        .is_some_and(|degrees| !degrees.can_pinch(axis, target_lower_cost[axis]))
                    {
                        possible = false;
                        break;
                    }
                    if constrained || joint_support.is_some() {
                        pinch_cost = pinch_cost
                            .checked_add(u128::from(target_lower_cost[axis]) + 1)
                            .ok_or(CandidateDomainRouteFailure::CountOverflow {
                                resource: "weighted pinch cost",
                            })?;
                    }
                    if let Some(remaining) = pinched_rank {
                        // lower>=remaining implies lower+1>remaining, even at
                        // u64::MAX. Never saturate an impossible pinch to rank 0.
                        if target_lower_cost[axis] >= u64::from(remaining) {
                            possible = false;
                            break;
                        }
                        pinched_rank = Some(remaining - target_lower_cost[axis] as u32 - 1);
                    }
                }
                // Each affine source row supplies at most its power in total
                // to the removed columns. Shared rows must be counted once.
                if possible && removed > 1 {
                    if let Some(joint) = &mut joint_support {
                        if !joint.can_pinch(
                            positions.iter().map(|&p| active[p]),
                            pinch_cost,
                            cancellation,
                        )? {
                            possible = false;
                            stats.joint_support_masks_pruned = admit(
                                stats.joint_support_masks_pruned,
                                1,
                                usize::MAX,
                                "joint support masks pruned",
                            )?;
                        }
                    }
                }
                // Even an installed/known-zero subsupport reenters Route. The
                // caller MUST admit source conditions before further routing;
                // this cover is not a validated original RHS child. Literal
                // Apply uses the ordinary matcher to establish its validity.
                if possible {
                    let mut pinched_upper = target_upper;
                    // Use source-derived lower bounds, never the additional
                    // projection of the all-positive root. A removed axis now
                    // measures excess numerator degree and must start at zero.
                    let mut pinched_lower = target_lower;
                    for (axis, &on) in sector.iter().enumerate() {
                        if !on {
                            // An inactive source bound cannot be carried
                            // through an affine map as if it were a permutation.
                            pinched_upper[axis] = None;
                            pinched_lower[axis] = 0;
                        }
                    }
                    let pinched = CandidateDomainRouteCover {
                        actual_rank: pinched_rank,
                        lower: pinched_lower,
                        upper: pinched_upper,
                        ..cover
                    };
                    let pinched = if constrained {
                        if let Some(bounds) =
                            mapped_bounds(power_bounds, positive_upper, pinch_cost)
                        {
                            project_cover(
                                &sector,
                                CandidateDomainRouteCover {
                                    power_bounds: bounds,
                                    ..pinched
                                },
                            )?
                        } else {
                            None
                        }
                    } else {
                        Some(pinched)
                    };
                    if let Some(cover) = pinched {
                        emit(
                            CandidateDomainRouteEvent::Route { sector, cover },
                            limits,
                            cancellation,
                            visit,
                            stats,
                        )?;
                    } else {
                        stats.masks_examined = masks;
                        stats.masks_pruned += 1;
                    }
                } else {
                    stats.masks_examined = masks;
                    stats.masks_pruned += 1;
                }
                let Some(index) = (0..removed)
                    .rev()
                    .find(|&i| positions[i] < count - removed + i)
                else {
                    break;
                };
                positions[index] += 1;
                for i in index + 1..removed {
                    positions[i] = positions[i - 1] + 1;
                }
            }
        }
        cancelled(cancellation)
    }
}

fn emit<const N: usize>(
    event: CandidateDomainRouteEvent<N>,
    limits: CandidateDomainRouteLimits,
    cancellation: &AtomicBool,
    visit: &mut impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    stats: &mut CandidateDomainRouteStats,
) -> Result<(), CandidateDomainRouteFailure> {
    cancelled(cancellation)?;
    let masks = admit(stats.masks_examined, 1, limits.max_masks, "route masks")?;
    let domain = matches!(
        event,
        CandidateDomainRouteEvent::Apply { .. } | CandidateDomainRouteEvent::Route { .. }
    );
    let entries = if domain {
        N.checked_mul(2)
            .ok_or(CandidateDomainRouteFailure::CountOverflow {
                resource: "route coordinate cells",
            })?
    } else {
        0
    };
    let cells = admit(
        stats.coordinate_cells,
        entries,
        limits.max_coordinate_cells,
        "route coordinate cells",
    )?;
    stats.masks_examined = masks;
    stats.coordinate_cells = cells;
    stats.events += 1;
    match event {
        CandidateDomainRouteEvent::Apply { .. } => stats.apply_domains += 1,
        CandidateDomainRouteEvent::Route { .. } => stats.route_domains += 1,
        CandidateDomainRouteEvent::MissingRoute { .. } => stats.missing_routes += 1,
        CandidateDomainRouteEvent::ZeroSector { .. } => stats.zero_sectors += 1,
    }
    if visit(event).is_break() {
        return Err(CandidateDomainRouteFailure::StoppedByConsumer);
    }
    cancelled(cancellation)
}
