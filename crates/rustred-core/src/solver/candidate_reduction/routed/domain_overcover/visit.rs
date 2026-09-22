use super::super::RoutedCandidateReducer;
use super::model::*;
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
        mut visit: impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
    ) -> Result<CandidateDomainRouteStats, CandidateDomainRouteError> {
        let mut stats = CandidateDomainRouteStats::default();
        let result = self.route_cover(
            source,
            actual_rank,
            limits,
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
        actual_rank: Option<u32>,
        limits: CandidateDomainRouteLimits,
        cancellation: &AtomicBool,
        visit: &mut impl FnMut(CandidateDomainRouteEvent<N>) -> ControlFlow<()>,
        stats: &mut CandidateDomainRouteStats,
    ) -> Result<(), CandidateDomainRouteFailure> {
        cancelled(cancellation)?;
        // Source validity precedes zero in concrete evaluation. We report its
        // remaining obligation rather than treating a zero census as validity.
        if self.programs.context.shared.zero_sectors.contains(&source) {
            return emit(
                CandidateDomainRouteEvent::ZeroSector {
                    sector: source,
                    actual_rank,
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
                        actual_rank,
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
                },
                limits,
                cancellation,
                visit,
                stats,
            );
        };
        let root: [bool; N] = std::array::from_fn(|axis| route.owner_sector.active_bits()[axis]);
        let count = root.iter().filter(|&&b| b).count();
        if count != source.iter().filter(|&&b| b).count() {
            return Err(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                "active-root cardinality differs",
            ));
        }
        let cover = CandidateDomainRouteCover {
            source_sector: source,
            target_root: root,
            actual_rank,
            conservative: true,
        };
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

        // Prepared::compile admits a unit active-row bijection and only affine
        // inactive rows. Endpoints B-e have B>=0 and |e|<=D, so their numerator
        // rank is |e|-sum_j min(e_j,B_j). Removing k positive axes consumes
        // at least k degree because each lost axis has e_j>=B_j>=1. Thus the
        // strict-pinch endpoint rank is <=D-k<=R-k, even for unbounded positive
        // powers. Affine constants can only lower monomial degree; native
        // cancellation can only remove endpoints.
        // Enumerate only combinations of at most R removed axes.
        let max_removed = actual_rank.map_or(count, |r| {
            usize::try_from(r).unwrap_or(usize::MAX).min(count)
        });
        if max_removed == 0 {
            return cancelled(cancellation);
        }
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
            let pinched_rank = match actual_rank {
                Some(rank) => Some(
                    u32::try_from(removed)
                        .ok()
                        .and_then(|lost| rank.checked_sub(lost))
                        .ok_or(CandidateDomainRouteFailure::InvalidAdmittedRoute(
                            "removed support exceeds incoming numerator rank",
                        ))?,
                ),
                None => None,
            };
            let pinched_cover = CandidateDomainRouteCover {
                actual_rank: pinched_rank,
                ..cover
            };
            positions.clear();
            positions.extend(0..removed);
            loop {
                cancelled(cancellation)?;
                let mut sector = root;
                for &position in &positions {
                    sector[active[position]] = false;
                }
                // Even an installed/known-zero subsupport reenters Route. The
                // caller MUST admit source conditions before further routing;
                // this cover is not a validated original RHS child. Literal
                // Apply uses the ordinary matcher to establish its validity.
                emit(
                    CandidateDomainRouteEvent::Route {
                        sector,
                        cover: pinched_cover,
                    },
                    limits,
                    cancellation,
                    visit,
                    stats,
                )?;
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
    stats.events = masks;
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
