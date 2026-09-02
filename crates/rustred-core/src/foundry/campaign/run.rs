use crate::foundry::completion::frame::admission::{
    ExactOwnerCoverObstructionKind, ExactOwnerCoverStatus,
};
use crate::foundry::completion::source_discovery::leader_walk::{
    LeaderWalkLimits, RequestedDomainScopePartition, try_plan_requested_domains,
};
use crate::foundry::completion::source_discovery::scheduler::{
    ProbeLocalRejectionCategory, ProbeLocalRejectionSummary, ProbeLocalStage,
};
use crate::foundry::completion::source_discovery::{
    BoundaryProbeCoordinator, CanonicalExactOwnerLedger, ExactOwnerCoverSnapshot,
    ExactOwnerLedgerCoverStatus, ProbeCampaignAdapter, ProbeCampaignLimits, ProbeCoordinatorCensus,
    ProbeCoordinatorConfig, ProbeCoordinatorLimits, ProbeCoordinatorNeedsRefinementReason,
    ProbeCoordinatorOperationalReason, ProbeCoordinatorStop, ProbeCoordinatorTaskLocation,
    ProbeCoordinatorTaskLocationKind, RequestedProbeCoordinatorStop, TaskRelativeModularProbe,
};

use crate::foundry::artifact::FULL_RANK_ORBITS;

use super::k6_resource::K6CampaignResourceProfile;
use super::preset_k6::{
    k6_root_predecessor_for_ordering, shared_k6_algebra_inputs,
    try_new_k6_full_rank_ledger_with_profile_and_ordering,
};
use super::requested::{
    K6RequestedDomainSpec, try_resolve_autonomous_k6_shells, try_resolve_external_k6_domains,
};
use super::{
    FoundryCampaignCensus, FoundryCampaignConfig, FoundryCampaignCoverageObstruction,
    FoundryCampaignCoverageStatus, FoundryCampaignError, FoundryCampaignItinerary,
    FoundryCampaignNeedsRefinementReason, FoundryCampaignOperationalLimit, FoundryCampaignPreset,
    FoundryCampaignProbeStage, FoundryCampaignProgress, FoundryCampaignReport, FoundryCampaignRun,
    FoundryCampaignSchedulerRejection, FoundryCampaignSchedulerRejectionCategory,
    FoundryCampaignSetupStage, FoundryCampaignSnapshot, FoundryCampaignStop,
    FoundryCampaignTaskLocation, FoundryCampaignTaskLocationKind, FoundryCampaignUncoveredBox,
};

/// Run one bounded campaign from a fresh authenticated exact ledger.
///
/// Retained-owner changes are consumed internally by dropping the stale plan
/// and replanning from the new revision. Only a terminal typed stop is
/// returned. The result is detached diagnostics, never proof or artifact
/// publication authority.
pub fn run_foundry_campaign(
    config: &FoundryCampaignConfig,
) -> Result<FoundryCampaignRun, FoundryCampaignError> {
    run_foundry_campaign_with_progress(config, |_| {})
}

/// Run one bounded campaign and observe every committed exact owner mutation.
///
/// Progress values contain only allocation-free detached scalar telemetry and
/// are emitted in the same deterministic order as ledger revisions. The
/// callback cannot acquire ledger, owner, or publication authority.
pub fn run_foundry_campaign_with_progress(
    config: &FoundryCampaignConfig,
    observe: impl FnMut(FoundryCampaignProgress),
) -> Result<FoundryCampaignRun, FoundryCampaignError> {
    let resolved = config.try_resolve_search_program()?;
    let config = &resolved;
    if config.itinerary() != FoundryCampaignItinerary::SingleSectorFixedPoint {
        return Err(FoundryCampaignError::Invariant {
            detail: "single-sector campaign runner received a full-rank-wave itinerary",
        });
    }
    match config.preset() {
        FoundryCampaignPreset::ThreeLoopUnitMassVacuumK6Orbit0 => run_k6_orbit_0(config, observe),
    }
}

fn run_k6_orbit_0(
    config: &FoundryCampaignConfig,
    mut observe: impl FnMut(FoundryCampaignProgress),
) -> Result<FoundryCampaignRun, FoundryCampaignError> {
    let inputs = shared_k6_algebra_inputs()?;
    let predecessor = k6_root_predecessor_for_ordering(config.ordering())?;
    let representative = FULL_RANK_ORBITS
        .first()
        .ok_or(FoundryCampaignError::Invariant {
            detail: "K6 full-rank orbit manifest is empty",
        })?
        .representative;
    let resource_profile = K6CampaignResourceProfile::try_for_task_report_ceiling(
        config.max_task_reports(),
    )
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Ledger, error))?;
    let campaign_limits = resource_profile.probe_campaign_limits();
    let ledger = try_new_k6_full_rank_ledger_with_profile_and_ordering(
        inputs,
        representative,
        predecessor,
        config.ordering(),
        resource_profile,
        campaign_limits,
    )?;
    let adapter = ProbeCampaignAdapter::try_new(
        inputs.generator(),
        inputs.completed(),
        inputs.zero_sources(),
        campaign_limits,
    )
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Coordinator, error))?;
    let coordinator_config = try_build_coordinator_config(config, campaign_limits)?;
    if ledger.revision().get() != 0 || !ledger.owners().is_empty() {
        return Err(FoundryCampaignError::Invariant {
            detail: "built-in campaign did not start without discovered ordinary owners",
        });
    }
    let maximum_dimension = ledger.closure_carrier().arity();
    let retained = try_drive_live_ledger_until_terminal_with_progress(
        coordinator_config,
        config,
        adapter,
        ledger,
        |exact, census, location| {
            observe(FoundryCampaignProgress::new(
                detach_snapshot(exact),
                detach_census(census),
                location.map(detach_location),
                maximum_dimension,
                config.max_task_reports(),
            ));
        },
    )?;
    let final_census = retained.final_census();
    let (ledger, terminal_stop) = retained.into_parts();

    detach_run(config, ledger, terminal_stop, final_census)
}

pub(super) fn try_build_coordinator_config(
    config: &FoundryCampaignConfig,
    campaign_limits: ProbeCampaignLimits,
) -> Result<ProbeCoordinatorConfig, FoundryCampaignError> {
    let mut probes = Vec::new();
    probes
        .try_reserve_exact(config.probes().len())
        .map_err(|_| FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::ProbeProgram,
            message: format!(
                "could not reserve {} modular probe templates",
                config.probes().len()
            ),
        })?;
    for probe in config.probes() {
        probes.push(
            TaskRelativeModularProbe::try_new(
                probe.modulus(),
                probe.base_parameters().iter().copied(),
                probe.chart_offsets().iter().copied(),
                campaign_limits.replay.scheduler.campaign,
            )
            .map_err(|error| {
                FoundryCampaignError::setup(FoundryCampaignSetupStage::ProbeProgram, error)
            })?,
        );
    }
    let coordinator_limits = ProbeCoordinatorLimits {
        max_task_reports: config.max_task_reports(),
        ..ProbeCoordinatorLimits::default()
    };
    ProbeCoordinatorConfig::try_new(
        probes,
        config.interior_margin(),
        config.polynomial_degree_ceiling(),
        coordinator_limits,
    )
    .map(|coordinator| {
        coordinator
            .with_discovery_coordinate_priority(config.discovery_coordinate_priority().clone())
    })
    .map_err(|error| FoundryCampaignError::setup(FoundryCampaignSetupStage::Coordinator, error))
}

/// One terminal coordinator stop paired with the still-live exact ledger that
/// authorized it. This is the consuming seam used by same-rank wave
/// orchestration; detached public diagnostics deliberately drop the ledger.
#[derive(Debug)]
pub(crate) struct RetainedLedgerCampaignRun {
    ledger: CanonicalExactOwnerLedger,
    terminal_stop: ProbeCoordinatorStop,
    final_census: ProbeCoordinatorCensus,
}

impl RetainedLedgerCampaignRun {
    pub(crate) const fn ledger(&self) -> &CanonicalExactOwnerLedger {
        &self.ledger
    }

    pub(crate) const fn terminal_stop(&self) -> &ProbeCoordinatorStop {
        &self.terminal_stop
    }

    /// Final cumulative coordinator census. The retained terminal reason may
    /// deliberately be the first local obstruction, while later probing can
    /// still increase cumulative work without mutating the owner ledger.
    pub(crate) const fn final_census(&self) -> ProbeCoordinatorCensus {
        self.final_census
    }

    pub(crate) fn into_parts(self) -> (CanonicalExactOwnerLedger, ProbeCoordinatorStop) {
        (self.ledger, self.terminal_stop)
    }
}

pub(crate) fn try_drive_live_ledger_until_terminal_with_progress<'inputs, 'sources, 'family>(
    coordinator_config: ProbeCoordinatorConfig,
    campaign_config: &FoundryCampaignConfig,
    adapter: ProbeCampaignAdapter<'inputs, 'sources, 'family>,
    mut ledger: CanonicalExactOwnerLedger,
    mut observe: impl FnMut(
        ExactOwnerCoverSnapshot,
        ProbeCoordinatorCensus,
        Option<ProbeCoordinatorTaskLocation>,
    ),
) -> Result<RetainedLedgerCampaignRun, FoundryCampaignError> {
    let mut coordinator = BoundaryProbeCoordinator::try_new(coordinator_config, adapter, &ledger)
        .map_err(|error| {
        FoundryCampaignError::setup(FoundryCampaignSetupStage::Coordinator, error)
    })?;
    let external_domains = try_resolve_external_k6_domains(
        campaign_config,
        ledger.predecessor_snapshot(),
        ledger.sector(),
    )?;
    let mut autonomous_shell_depth = 0usize;
    let requested_domain_limits = LeaderWalkLimits::default();
    let mut first_local_obstruction = None;

    loop {
        let partition = ledger.try_clone_uncovered_partition().map_err(|error| {
            FoundryCampaignError::Execution {
                message: error.to_string(),
            }
        })?;
        let blind_domains = match try_resolve_autonomous_k6_shells(
            &partition,
            autonomous_shell_depth,
            campaign_config.discovery_coordinate_priority(),
            requested_domain_limits,
        ) {
            Ok(domains) => domains,
            Err(
                error @ (FoundryCampaignError::ResourceCountOverflow { .. }
                | FoundryCampaignError::ResourceLimit { .. }),
            ) => {
                if let Some(terminal_stop) = first_local_obstruction.take() {
                    return Ok(RetainedLedgerCampaignRun {
                        ledger,
                        terminal_stop,
                        final_census: coordinator.census(),
                    });
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        let requested_domains = match merge_requested_domains(
            &external_domains,
            blind_domains,
            requested_domain_limits,
        ) {
            Ok(domains) => domains,
            Err(
                error @ (FoundryCampaignError::ResourceCountOverflow { .. }
                | FoundryCampaignError::ResourceLimit { .. }),
            ) => {
                if let Some(terminal_stop) = first_local_obstruction.take() {
                    return Ok(RetainedLedgerCampaignRun {
                        ledger,
                        terminal_stop,
                        final_census: coordinator.census(),
                    });
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        if !requested_domains.is_empty() {
            let requested_stop = try_run_requested_phase(
                &mut coordinator,
                &mut ledger,
                &requested_domains,
                requested_domain_limits,
            )?;
            match requested_stop {
                RequestedProbeCoordinatorStop::OwnerSetChanged(changed) => {
                    validate_and_observe_owner_change(&ledger, &changed, &mut observe)?;
                    first_local_obstruction = None;
                    autonomous_shell_depth = 0;
                    continue;
                }
                RequestedProbeCoordinatorStop::CompilerClosed {
                    census,
                    ledger_snapshot,
                    exact,
                } => {
                    // Requested and boundary closure have the same exact
                    // compiler authority. If this phase ran a task, that task
                    // committed the closing owner and must produce the same
                    // detached mutation callback as boundary closure.
                    if census.task_reports() != 0 {
                        observe(ledger.snapshot(), census, None);
                    }
                    return Ok(RetainedLedgerCampaignRun {
                        ledger,
                        terminal_stop: ProbeCoordinatorStop::CompilerClosed {
                            census,
                            ledger_snapshot,
                            exact,
                        },
                        final_census: coordinator.census(),
                    });
                }
                RequestedProbeCoordinatorStop::NeedsRefinement(stop) => {
                    if first_local_obstruction.is_none() {
                        first_local_obstruction = Some(ProbeCoordinatorStop::NeedsRefinement(stop));
                    }
                }
                RequestedProbeCoordinatorStop::OperationallyBounded(stop) => {
                    if matches!(
                        stop.reason(),
                        ProbeCoordinatorOperationalReason::IncompleteProbeExecution { .. }
                    ) {
                        if first_local_obstruction.is_none() {
                            first_local_obstruction =
                                Some(ProbeCoordinatorStop::OperationallyBounded(stop));
                        }
                    } else {
                        return Ok(RetainedLedgerCampaignRun {
                            ledger,
                            terminal_stop: ProbeCoordinatorStop::OperationallyBounded(stop),
                            final_census: coordinator.census(),
                        });
                    }
                }
                RequestedProbeCoordinatorStop::Failed(failure) => {
                    return Err(FoundryCampaignError::Execution {
                        message: failure.failure().to_string(),
                    });
                }
                RequestedProbeCoordinatorStop::PhaseCompleted { .. } => {
                    // Local stability is not exhaustion. Service the complete
                    // generic boundary schedule on this same live revision.
                }
            }
        }

        match coordinator.try_run_boundary_epoch(&mut ledger) {
            ProbeCoordinatorStop::OwnerSetChanged(changed) => {
                validate_and_observe_owner_change(&ledger, &changed, &mut observe)?;
                first_local_obstruction = None;
                autonomous_shell_depth = 0;
            }
            ProbeCoordinatorStop::Failed(failure) => {
                return Err(FoundryCampaignError::Execution {
                    message: failure.failure().to_string(),
                });
            }
            stop @ ProbeCoordinatorStop::CompilerClosed { .. } => {
                let census = stop.census();
                // A coordinator constructed over an already closed ledger has
                // no new mutation to report. A nonzero task census means the
                // terminal task committed the closing owner in this drive.
                if census.task_reports() != 0 {
                    observe(ledger.snapshot(), census, None);
                }
                return Ok(RetainedLedgerCampaignRun {
                    ledger,
                    terminal_stop: stop,
                    final_census: coordinator.census(),
                });
            }
            ProbeCoordinatorStop::NeedsRefinement(stop) => {
                if first_local_obstruction.is_none() {
                    first_local_obstruction = Some(ProbeCoordinatorStop::NeedsRefinement(stop));
                }
                autonomous_shell_depth = autonomous_shell_depth.checked_add(1).ok_or(
                    FoundryCampaignError::Invariant {
                        detail: "autonomous K6 shell depth overflowed usize",
                    },
                )?;
            }
            ProbeCoordinatorStop::OperationallyBounded(stop) => {
                if matches!(
                    stop.reason(),
                    ProbeCoordinatorOperationalReason::IncompleteProbeExecution { .. }
                ) {
                    if first_local_obstruction.is_none() {
                        first_local_obstruction =
                            Some(ProbeCoordinatorStop::OperationallyBounded(stop));
                    }
                    autonomous_shell_depth = autonomous_shell_depth.checked_add(1).ok_or(
                        FoundryCampaignError::Invariant {
                            detail: "autonomous K6 shell depth overflowed usize",
                        },
                    )?;
                } else {
                    return Ok(RetainedLedgerCampaignRun {
                        ledger,
                        terminal_stop: ProbeCoordinatorStop::OperationallyBounded(stop),
                        final_census: coordinator.census(),
                    });
                }
            }
            ProbeCoordinatorStop::ExhaustedAtConfig { .. } => {
                autonomous_shell_depth = autonomous_shell_depth.checked_add(1).ok_or(
                    FoundryCampaignError::Invariant {
                        detail: "autonomous K6 shell depth overflowed usize",
                    },
                )?;
            }
        }
    }
}

fn merge_requested_domains(
    external: &[K6RequestedDomainSpec],
    blind: Vec<K6RequestedDomainSpec>,
    limits: LeaderWalkLimits,
) -> Result<Vec<K6RequestedDomainSpec>, FoundryCampaignError> {
    let requested =
        external
            .len()
            .checked_add(blind.len())
            .ok_or(FoundryCampaignError::Invariant {
                detail: "combined K6 requested-domain count overflowed usize",
            })?;
    if requested > limits.max_tasks {
        return Err(FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "combined K6 requested domains",
            requested,
            limit: limits.max_tasks,
        });
    }
    let requested_coordinate_cells =
        external
            .iter()
            .chain(&blind)
            .try_fold(0usize, |retained, domain| {
                let domain_cells = domain.retained_coordinate_cells()?;
                retained.checked_add(domain_cells).ok_or(
                    FoundryCampaignError::ResourceCountOverflow {
                        stage: FoundryCampaignSetupStage::RequestedDomains,
                        resource: "combined K6 requested-domain coordinate cells",
                    },
                )
            })?;
    if requested_coordinate_cells > limits.max_task_coordinate_cells {
        return Err(FoundryCampaignError::ResourceLimit {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            resource: "combined K6 requested-domain coordinate cells",
            requested: requested_coordinate_cells,
            limit: limits.max_task_coordinate_cells,
        });
    }
    let mut merged = Vec::new();
    merged
        .try_reserve_exact(requested)
        .map_err(|_| FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            message: format!("could not reserve {requested} combined K6 requested domains"),
        })?;
    let mut seen = std::collections::BTreeSet::new();
    for domain in external.iter().cloned().chain(blind) {
        if seen.insert(domain.clone()) {
            merged.push(domain);
        }
    }
    Ok(merged)
}

fn try_run_requested_phase<'inputs, 'sources, 'family>(
    coordinator: &mut BoundaryProbeCoordinator<'inputs, 'sources, 'family>,
    ledger: &mut CanonicalExactOwnerLedger,
    specs: &[K6RequestedDomainSpec],
    limits: LeaderWalkLimits,
) -> Result<RequestedProbeCoordinatorStop, FoundryCampaignError> {
    let partition = ledger.try_clone_uncovered_partition().map_err(|error| {
        FoundryCampaignError::Execution {
            message: error.to_string(),
        }
    })?;
    let mut requests = Vec::new();
    requests
        .try_reserve_exact(specs.len())
        .map_err(|_| FoundryCampaignError::Setup {
            stage: FoundryCampaignSetupStage::RequestedDomains,
            message: format!(
                "could not reserve {} canonical requested domains",
                specs.len()
            ),
        })?;
    for spec in specs {
        requests.push(spec.materialize()?);
    }
    let plan = try_plan_requested_domains(
        ledger.revision().get(),
        [RequestedDomainScopePartition::new(
            "rustred.k6.requested-domains.v1",
            ledger.sector(),
            &partition,
            &requests,
        )],
        limits,
    )
    .map_err(|error| {
        FoundryCampaignError::setup(FoundryCampaignSetupStage::RequestedDomains, error)
    })?;
    Ok(coordinator.try_run_requested_plan(&plan, ledger))
}

fn validate_and_observe_owner_change(
    ledger: &CanonicalExactOwnerLedger,
    changed: &crate::foundry::completion::source_discovery::ProbeCoordinatorOwnerSetChanged,
    observe: &mut impl FnMut(
        ExactOwnerCoverSnapshot,
        ProbeCoordinatorCensus,
        Option<ProbeCoordinatorTaskLocation>,
    ),
) -> Result<(), FoundryCampaignError> {
    let expected_revision =
        changed
            .before_revision()
            .checked_add(1)
            .ok_or(FoundryCampaignError::Invariant {
                detail: "owner mutation overflowed the exact ledger revision",
            })?;
    if changed.after_revision() != expected_revision
        || ledger.revision().get() != changed.after_revision()
    {
        return Err(FoundryCampaignError::Invariant {
            detail: "owner mutation did not advance the fresh ledger by one revision",
        });
    }
    observe(
        ledger.snapshot(),
        changed.census(),
        Some(changed.location()),
    );
    Ok(())
}

fn detach_run(
    config: &FoundryCampaignConfig,
    ledger: CanonicalExactOwnerLedger,
    terminal_stop: ProbeCoordinatorStop,
    final_census: ProbeCoordinatorCensus,
) -> Result<FoundryCampaignRun, FoundryCampaignError> {
    let report = detach_report(config, &ledger, &terminal_stop, final_census)?;
    Ok(FoundryCampaignRun::new(report))
}

/// Detach complete, bounded diagnostics while retaining the live ledger.
///
/// Full-wave orchestration needs this borrowing form so an incomplete sibling
/// can expose its exact residual geometry without surrendering or cloning any
/// executable-owner authority retained by the unpublished wave.
pub(super) fn detach_report(
    config: &FoundryCampaignConfig,
    ledger: &CanonicalExactOwnerLedger,
    terminal_stop: &ProbeCoordinatorStop,
    final_census: ProbeCoordinatorCensus,
) -> Result<FoundryCampaignReport, FoundryCampaignError> {
    let stop = detach_stop(terminal_stop)?;
    let census = detach_census(final_census);
    let exact = ledger.snapshot();
    let snapshot = detach_snapshot(exact);
    let partition = ledger.try_clone_uncovered_partition().map_err(|error| {
        FoundryCampaignError::Execution {
            message: error.to_string(),
        }
    })?;
    if partition.boxes().len() != exact.uncovered_box_count() {
        return Err(FoundryCampaignError::Invariant {
            detail: "detached uncovered partition disagrees with exact snapshot census",
        });
    }
    let uncovered_boxes = partition
        .boxes()
        .iter()
        .take(config.max_reported_uncovered_boxes())
        .map(|lattice_box| {
            FoundryCampaignUncoveredBox::new(
                lattice_box.lower().to_vec().into_boxed_slice(),
                lattice_box.upper().to_vec().into_boxed_slice(),
                lattice_box.free_dimension(),
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let predecessor = ledger.predecessor_snapshot();
    let report = FoundryCampaignReport::new(
        config.preset(),
        config.ordering(),
        predecessor.family_fingerprint().to_owned(),
        predecessor.context_fingerprint().to_owned(),
        ledger.sector().active_bits().to_vec().into_boxed_slice(),
        stop,
        census,
        snapshot,
        uncovered_boxes,
    );
    Ok(report)
}

fn detach_stop(stop: &ProbeCoordinatorStop) -> Result<FoundryCampaignStop, FoundryCampaignError> {
    Ok(match stop {
        ProbeCoordinatorStop::CompilerClosed { .. } => FoundryCampaignStop::CompilerClosed,
        ProbeCoordinatorStop::NeedsRefinement(stop) => FoundryCampaignStop::NeedsRefinement {
            location: stop.location().map(detach_location),
            reason: detach_refinement(stop.reason()),
        },
        ProbeCoordinatorStop::OperationallyBounded(stop) => {
            FoundryCampaignStop::OperationallyBounded {
                location: stop.location().map(detach_location),
                limit: detach_operational(stop.reason()),
            }
        }
        ProbeCoordinatorStop::ExhaustedAtConfig {
            ledger_revision,
            completed_classes,
            completed_tasks,
            ..
        } => FoundryCampaignStop::ExhaustedAtConfig {
            ledger_revision: *ledger_revision,
            completed_classes: *completed_classes,
            completed_tasks: *completed_tasks,
        },
        ProbeCoordinatorStop::OwnerSetChanged(_) => {
            return Err(FoundryCampaignError::Invariant {
                detail: "owner-change stop escaped the campaign replan loop",
            });
        }
        ProbeCoordinatorStop::Failed(failure) => {
            return Err(FoundryCampaignError::Execution {
                message: failure.failure().to_string(),
            });
        }
    })
}

fn detach_location(location: ProbeCoordinatorTaskLocation) -> FoundryCampaignTaskLocation {
    let kind = match location.kind() {
        ProbeCoordinatorTaskLocationKind::BoundarySimplex => {
            FoundryCampaignTaskLocationKind::BoundarySimplex
        }
        ProbeCoordinatorTaskLocationKind::RequestedDomain { requested_ordinal } => {
            FoundryCampaignTaskLocationKind::RequestedDomain { requested_ordinal }
        }
    };
    FoundryCampaignTaskLocation::new(
        kind,
        location.ledger_revision(),
        location.class_ordinal(),
        location.effective_dimension(),
        location.parent_free_dimension(),
        location.boundary_codimension(),
        location.task_ordinal(),
    )
}

fn detach_operational(
    reason: ProbeCoordinatorOperationalReason,
) -> FoundryCampaignOperationalLimit {
    match reason {
        ProbeCoordinatorOperationalReason::EpochLimit { requested, limit } => {
            FoundryCampaignOperationalLimit::Epoch { requested, limit }
        }
        ProbeCoordinatorOperationalReason::PlanLimit { requested, limit } => {
            FoundryCampaignOperationalLimit::Plan { requested, limit }
        }
        ProbeCoordinatorOperationalReason::TaskReportLimit { requested, limit } => {
            FoundryCampaignOperationalLimit::TaskReport { requested, limit }
        }
        ProbeCoordinatorOperationalReason::IncompleteProbeExecution {
            scheduler_budget_stops,
            scheduler_rejections,
            scheduler_exact_lift_errors,
            terminal_scheduler_rejection,
        } => FoundryCampaignOperationalLimit::IncompleteProbeExecution {
            scheduler_budget_stops,
            scheduler_rejections,
            scheduler_exact_lift_errors,
            terminal_scheduler_rejection: terminal_scheduler_rejection
                .map(detach_scheduler_rejection),
        },
    }
}

fn detach_refinement(
    reason: ProbeCoordinatorNeedsRefinementReason,
) -> FoundryCampaignNeedsRefinementReason {
    match reason {
        ProbeCoordinatorNeedsRefinementReason::IncompleteProposal { exact_obstructions } => {
            FoundryCampaignNeedsRefinementReason::IncompleteProposal { exact_obstructions }
        }
        ProbeCoordinatorNeedsRefinementReason::ProbeStalled { scheduler_stalls } => {
            FoundryCampaignNeedsRefinementReason::ProbeStalled { scheduler_stalls }
        }
        ProbeCoordinatorNeedsRefinementReason::CanonicalQueryRejected {
            canonical_query_rejections,
        } => FoundryCampaignNeedsRefinementReason::CanonicalQueryRejected {
            canonical_query_rejections,
        },
        ProbeCoordinatorNeedsRefinementReason::DiagnosticExactObstructions { count } => {
            FoundryCampaignNeedsRefinementReason::DiagnosticExactObstructions { count }
        }
        ProbeCoordinatorNeedsRefinementReason::ExactCompilerState {
            status,
            uncovered_is_finite,
            missing_terminal_count,
            guard_incomplete_owner_count,
        } => FoundryCampaignNeedsRefinementReason::ExactCompilerState {
            coverage: detach_coverage(status),
            uncovered_is_finite,
            missing_terminal_count,
            guard_incomplete_owner_count,
        },
    }
}

fn detach_snapshot(exact: ExactOwnerCoverSnapshot) -> FoundryCampaignSnapshot {
    FoundryCampaignSnapshot::new(
        exact.revision().get(),
        detach_coverage(exact.status()),
        exact.owner_count(),
        exact.terminal_count(),
        exact.uncovered_box_count(),
        exact.uncovered_is_finite(),
        exact.missing_terminal_count(),
        exact.guard_incomplete_owner_count(),
    )
}

fn detach_coverage(status: ExactOwnerLedgerCoverStatus) -> FoundryCampaignCoverageStatus {
    match status {
        ExactOwnerLedgerCoverStatus::OwnerFree => FoundryCampaignCoverageStatus::OwnerFree,
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Closed) => {
            FoundryCampaignCoverageStatus::Closed
        }
        ExactOwnerLedgerCoverStatus::Compiled(ExactOwnerCoverStatus::Incomplete(obstruction)) => {
            FoundryCampaignCoverageStatus::Incomplete(match obstruction {
                ExactOwnerCoverObstructionKind::NonFinite => {
                    FoundryCampaignCoverageObstruction::NonFinite
                }
                ExactOwnerCoverObstructionKind::GuardIncomplete => {
                    FoundryCampaignCoverageObstruction::GuardIncomplete
                }
                ExactOwnerCoverObstructionKind::FiniteTerminalOwnership => {
                    FoundryCampaignCoverageObstruction::FiniteTerminalOwnership
                }
            })
        }
    }
}

fn detach_census(census: ProbeCoordinatorCensus) -> FoundryCampaignCensus {
    FoundryCampaignCensus {
        epochs_started: census.epochs_started(),
        plans_built: census.plans_built(),
        classes_completed: census.classes_completed(),
        task_reports: census.task_reports(),
        no_proposal: census.no_proposal(),
        duplicate: census.duplicate(),
        incomplete_proposal: census.incomplete_proposal(),
        changed_without_geometric_shrink: census.changed_without_geometric_shrink(),
        strict_geometric_shrink: census.strict_geometric_shrink(),
        compiler_closed: census.compiler_closed(),
        invalidated_tickets: census.invalidated_tickets(),
        scheduler_budget_stops: census.scheduler_budget_stops(),
        scheduler_rejections: census.scheduler_rejections(),
        first_scheduler_rejection: census
            .first_scheduler_rejection()
            .map(detach_scheduler_rejection),
        scheduler_stalls: census.scheduler_stalls(),
        scheduler_exact_lift_errors: census.scheduler_exact_lift_errors(),
        canonical_replayed: census.canonical_replayed(),
        canonical_no_modular_hit: census.canonical_no_modular_hit(),
        canonical_query_rejections: census.canonical_query_rejections(),
        canonical_support_did_not_lift: census.canonical_support_did_not_lift(),
        exact_obstructions: census.exact_obstructions(),
        declared_probes: census.declared_probes(),
        scheduler_replayed: census.scheduler_replayed(),
        scheduler_support_did_not_lift: census.scheduler_support_did_not_lift(),
        scheduler_sampled_dual: census.scheduler_sampled_dual(),
    }
}

pub(super) const fn detach_scheduler_rejection(
    rejection: ProbeLocalRejectionSummary,
) -> FoundryCampaignSchedulerRejection {
    let category = match rejection.category() {
        ProbeLocalRejectionCategory::Campaign => {
            FoundryCampaignSchedulerRejectionCategory::Campaign
        }
        ProbeLocalRejectionCategory::SourceDiscovery => {
            FoundryCampaignSchedulerRejectionCategory::SourceDiscovery
        }
        ProbeLocalRejectionCategory::SampledDual => {
            FoundryCampaignSchedulerRejectionCategory::SampledDual
        }
    };
    let stage = match rejection.stage() {
        ProbeLocalStage::UnexecutedAggregateSuffix => {
            FoundryCampaignProbeStage::UnexecutedAggregateSuffix
        }
        ProbeLocalStage::BootstrapNomination => FoundryCampaignProbeStage::BootstrapNomination,
        ProbeLocalStage::BootstrapAccumulation => FoundryCampaignProbeStage::BootstrapAccumulation,
        ProbeLocalStage::EpochAdmission => FoundryCampaignProbeStage::EpochAdmission,
        ProbeLocalStage::EpochBuild => FoundryCampaignProbeStage::EpochBuild,
        ProbeLocalStage::ModularQuery => FoundryCampaignProbeStage::ModularQuery,
        ProbeLocalStage::ObstructionNomination => FoundryCampaignProbeStage::ObstructionNomination,
        ProbeLocalStage::ResidualEvaluation => FoundryCampaignProbeStage::ResidualEvaluation,
        ProbeLocalStage::ObstructionBlockNomination => {
            FoundryCampaignProbeStage::ObstructionBlockNomination
        }
        ProbeLocalStage::ObstructionBlockEvaluation => {
            FoundryCampaignProbeStage::ObstructionBlockEvaluation
        }
        ProbeLocalStage::ObstructionBlockSelection => {
            FoundryCampaignProbeStage::ObstructionBlockSelection
        }
        ProbeLocalStage::RequestMerge => FoundryCampaignProbeStage::RequestMerge,
        ProbeLocalStage::SampledDualAdmission => FoundryCampaignProbeStage::SampledDualAdmission,
        ProbeLocalStage::ExactLift => FoundryCampaignProbeStage::ExactLift,
    };
    FoundryCampaignSchedulerRejection::new(category, stage, rejection.subkind())
}
