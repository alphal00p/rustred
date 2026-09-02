use std::collections::BTreeMap;
use std::sync::Arc;

use crate::foundry::completion::{LatticeBox, LatticePoint};
use crate::identity::IntegralShift;

use super::super::model::{LeaderWalkBoxKey, LeaderWalkGeometryEpochIdentity, LeaderWalkScopeKey};
use super::super::plan::chart_point_to_target_shift;
use super::super::{LeaderWalkLimits, LeaderWalkPlanError};
use super::{
    RequestedDomainPlan, RequestedDomainScopePartition, RequestedDomainTask, RequestedDomainTaskKey,
};

struct CanonicalScope<'a> {
    original_input_ordinal: usize,
    input: RequestedDomainScopePartition<'a>,
    canonical_boxes: Vec<&'a LatticeBox>,
}

struct PendingRequest<'a> {
    requested_ordinal: usize,
    leader: LatticePoint,
    symbolic_axes: &'a [usize],
    parent: &'a LatticeBox,
    requested_domain_lower: &'a [u64],
    requested_domain_upper: Vec<Option<u64>>,
    residual_domain_upper: Vec<Option<u64>>,
    target_shift: IntegralShift,
    fixed_indices: Vec<i64>,
}

struct PendingScope<'a> {
    key: LeaderWalkScopeKey,
    requests: Vec<PendingRequest<'a>>,
}

/// Attach an explicit, bounded target sequence to one frozen exact cover.
///
/// This planner accepts pivot domains only. It cannot import a relation,
/// source selection, support, coefficient, owner, or closure claim. Every
/// requested rectangle is intersected with every exact uncovered box; a
/// request is skipped only when that full domain has no residual. Every
/// residual retains its complete parent box for the normal probe adapter's
/// stale-geometry check.
pub(crate) fn try_plan_requested_domains<'a>(
    epoch_ordinal: u64,
    scopes: impl IntoIterator<Item = RequestedDomainScopePartition<'a>>,
    limits: LeaderWalkLimits,
) -> Result<RequestedDomainPlan, LeaderWalkPlanError> {
    let mut canonical_scopes = Vec::new();
    let mut aggregate_scope_key_bytes = 0usize;
    let mut input_boxes = 0usize;
    let mut input_box_coordinate_cells = 0usize;
    let mut requested_domains = 0usize;

    for (input_ordinal, input) in scopes.into_iter().enumerate() {
        let scope_count = checked_add("input scopes", canonical_scopes.len(), 1)?;
        check_limit("input scopes", scope_count, limits.max_scopes)?;
        if input.stable_scope_key.is_empty() {
            return Err(LeaderWalkPlanError::EmptyStableScopeKey { input_ordinal });
        }
        aggregate_scope_key_bytes = checked_add(
            "aggregate stable-scope-key bytes",
            aggregate_scope_key_bytes,
            input.stable_scope_key.len(),
        )?;
        check_limit(
            "aggregate stable-scope-key bytes",
            aggregate_scope_key_bytes,
            limits.max_aggregate_scope_key_bytes,
        )?;
        check_limit("scope arity", input.sector.arity(), limits.max_arity)?;

        let mut canonical_boxes = Vec::new();
        try_reserve_exact(
            &mut canonical_boxes,
            input.uncovered.boxes().len(),
            "canonical input boxes",
        )?;
        for (box_ordinal, lattice_box) in input.uncovered.boxes().iter().enumerate() {
            if lattice_box.arity() != input.sector.arity() {
                return Err(LeaderWalkPlanError::WrongPartitionBoxArity {
                    input_scope_ordinal: input_ordinal,
                    box_ordinal,
                    expected: input.sector.arity(),
                    actual: lattice_box.arity(),
                });
            }
            input_boxes = checked_add("input uncovered boxes", input_boxes, 1)?;
            check_limit("input uncovered boxes", input_boxes, limits.max_input_boxes)?;
            input_box_coordinate_cells = checked_add(
                "input uncovered-box coordinate cells",
                input_box_coordinate_cells,
                checked_mul(
                    "input uncovered-box coordinate cells",
                    lattice_box.arity(),
                    2,
                )?,
            )?;
            check_limit(
                "input uncovered-box coordinate cells",
                input_box_coordinate_cells,
                limits.max_input_box_coordinate_cells,
            )?;
            canonical_boxes.push(lattice_box);
        }
        canonical_boxes.sort_unstable();

        requested_domains = checked_add(
            "requested pivot domains",
            requested_domains,
            input.requested.len(),
        )?;
        check_limit(
            "requested pivot domains",
            requested_domains,
            limits.max_tasks,
        )?;
        try_reserve_one(&mut canonical_scopes, "canonical requested scopes")?;
        canonical_scopes.push(CanonicalScope {
            original_input_ordinal: input_ordinal,
            input,
            canonical_boxes,
        });
    }

    if canonical_scopes.is_empty() {
        return Err(LeaderWalkPlanError::EmptyScopeSchedule);
    }
    if requested_domains == 0 {
        return Err(LeaderWalkPlanError::EmptyRequestedDomainSchedule);
    }

    canonical_scopes.sort_unstable_by(|left, right| {
        left.input
            .stable_scope_key
            .cmp(right.input.stable_scope_key)
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });
    for ordinal in 1..canonical_scopes.len() {
        if canonical_scopes[ordinal - 1].input.stable_scope_key
            == canonical_scopes[ordinal].input.stable_scope_key
        {
            return Err(LeaderWalkPlanError::DuplicateStableScopeKey {
                first_canonical_ordinal: ordinal - 1,
                duplicate_canonical_ordinal: ordinal,
            });
        }
    }
    canonical_scopes.sort_unstable_by(|left, right| {
        left.input
            .sector
            .cmp(right.input.sector)
            .then_with(|| left.canonical_boxes.cmp(&right.canonical_boxes))
            .then_with(|| {
                left.input
                    .stable_scope_key
                    .cmp(right.input.stable_scope_key)
            })
            .then_with(|| {
                left.original_input_ordinal
                    .cmp(&right.original_input_ordinal)
            })
    });

    let mut pending_scopes = Vec::new();
    try_reserve_exact(
        &mut pending_scopes,
        canonical_scopes.len(),
        "pending requested scopes",
    )?;
    let mut scheduled_residuals = 0usize;
    let mut fully_covered_domains = 0usize;
    let mut maximum_request_count = 0usize;

    for (canonical_scope_ordinal, canonical) in canonical_scopes.iter().enumerate() {
        let mut stable_key = String::new();
        stable_key
            .try_reserve_exact(canonical.input.stable_scope_key.len())
            .map_err(|_| LeaderWalkPlanError::AllocationFailure {
                resource: "stable scope key bytes",
                requested: canonical.input.stable_scope_key.len(),
            })?;
        stable_key.push_str(canonical.input.stable_scope_key);
        let key = LeaderWalkScopeKey::new(Arc::new(stable_key), canonical.input.sector.clone());
        let mut requests = Vec::new();
        try_reserve_exact(
            &mut requests,
            canonical.input.requested.len(),
            "pending requested domains",
        )?;
        let mut seen: BTreeMap<(&[u64], &[usize]), usize> = BTreeMap::new();
        for (request_ordinal, request) in canonical.input.requested.iter().enumerate() {
            let point = request.point();
            if point.arity() != canonical.input.sector.arity() {
                return Err(LeaderWalkPlanError::WrongRequestedDomainArity {
                    canonical_scope_ordinal,
                    request_ordinal,
                    expected: canonical.input.sector.arity(),
                    actual: point.arity(),
                });
            }
            let symbolic_axes = request.symbolic_axes();
            if symbolic_axes.windows(2).any(|pair| pair[0] >= pair[1])
                || symbolic_axes
                    .last()
                    .is_some_and(|&axis| axis >= point.arity())
            {
                return Err(LeaderWalkPlanError::InvalidRequestedDomainSymbolicAxes {
                    canonical_scope_ordinal,
                    request_ordinal,
                });
            }
            let semantic_identity = (point.coordinates(), symbolic_axes);
            if let Some(&first_request_ordinal) = seen.get(&semantic_identity) {
                return Err(LeaderWalkPlanError::DuplicateRequestedDomain {
                    canonical_scope_ordinal,
                    first_request_ordinal,
                    duplicate_request_ordinal: request_ordinal,
                });
            }
            seen.insert(semantic_identity, request_ordinal);
            let requested_domain_upper = (0..point.arity())
                .map(|position| {
                    symbolic_axes
                        .binary_search(&position)
                        .is_err()
                        .then_some(point.coordinates()[position])
                })
                .collect::<Vec<_>>();
            // A requested domain names one recurrence family. Replanning its
            // uncovered tail must retry that same family against fresh exact
            // geometry; translating the recurrence to the residual lower
            // endpoint silently changes the request and, at a finite machine
            // carrier fringe, can manufacture near-i64::MAX shifts for which
            // no representable base domain exists.
            let target_shift =
                chart_point_to_target_shift(&key, point.coordinates(), limits.max_arity)?;
            let mut residual_count = 0usize;
            for &parent in &canonical.canonical_boxes {
                let Some((leader, residual_domain_upper)) = intersect_requested_domain(
                    point.coordinates(),
                    &requested_domain_upper,
                    parent,
                )?
                else {
                    continue;
                };
                residual_count =
                    checked_add("requested-domain residual intersections", residual_count, 1)?;
                scheduled_residuals = checked_add(
                    "scheduled requested-domain residuals",
                    scheduled_residuals,
                    1,
                )?;
                check_limit(
                    "scheduled requested-domain residuals",
                    scheduled_residuals,
                    limits.max_tasks,
                )?;
                // SectorMonotoneDomain bounds are base indices. The residual
                // request is already represented by `target_shift`, so fixing
                // complementary axes at the requested pivot here would apply
                // that displacement twice. A fixed pivot literal c is instead
                // `corner + target_shift == c`.
                let fixed_indices = canonical.input.sector.corner_indices().collect::<Vec<_>>();
                requests.push(PendingRequest {
                    requested_ordinal: request_ordinal,
                    leader,
                    symbolic_axes,
                    parent,
                    requested_domain_lower: point.coordinates(),
                    requested_domain_upper: requested_domain_upper.clone(),
                    residual_domain_upper,
                    target_shift: target_shift.clone(),
                    fixed_indices,
                });
            }
            if residual_count == 0 {
                fully_covered_domains =
                    checked_add("fully covered requested domains", fully_covered_domains, 1)?;
            }
        }
        maximum_request_count = maximum_request_count.max(requests.len());
        pending_scopes.push(PendingScope { key, requests });
    }

    check_limit(
        "scheduled requested-domain residuals",
        scheduled_residuals,
        limits.max_tasks,
    )?;
    check_limit(
        "requested-domain residual parent boxes",
        scheduled_residuals,
        limits.max_selected_boxes,
    )?;
    let parent_cells = pending_scopes.iter().try_fold(0usize, |total, scope| {
        let scheduled = scope.requests.len();
        checked_add(
            "requested-domain residual parent-box coordinate cells",
            total,
            checked_mul(
                "requested-domain residual parent-box coordinate cells",
                checked_mul(
                    "requested-domain residual parent-box coordinate cells",
                    scheduled,
                    scope.key.sector().arity(),
                )?,
                2,
            )?,
        )
    })?;
    check_limit(
        "requested-domain residual parent-box coordinate cells",
        parent_cells,
        limits.max_selected_box_coordinate_cells,
    )?;
    let task_cells = pending_scopes.iter().try_fold(0usize, |total, scope| {
        scope.requests.iter().try_fold(total, |total, request| {
            let six_arity = checked_mul(
                "requested-domain residual task coordinate cells",
                scope.key.sector().arity(),
                6,
            )?;
            let per_task = checked_add(
                "requested-domain residual task coordinate cells",
                six_arity,
                request.symbolic_axes.len(),
            )?;
            checked_add(
                "requested-domain residual task coordinate cells",
                total,
                per_task,
            )
        })
    })?;
    check_limit(
        "requested-domain residual task coordinate cells",
        task_cells,
        limits.max_task_coordinate_cells,
    )?;

    let epoch_identity = LeaderWalkGeometryEpochIdentity::fresh();
    let mut tasks = Vec::new();
    try_reserve_exact(
        &mut tasks,
        scheduled_residuals,
        "requested-domain residual tasks",
    )?;
    for request_round in 0..maximum_request_count {
        for scope in &pending_scopes {
            let Some(pending) = scope.requests.get(request_round) else {
                continue;
            };
            let mut lower = Vec::new();
            try_reserve_exact(
                &mut lower,
                pending.parent.arity(),
                "requested parent lower endpoints",
            )?;
            lower.extend_from_slice(pending.parent.lower());
            let mut upper = Vec::new();
            try_reserve_exact(
                &mut upper,
                pending.parent.arity(),
                "requested parent upper endpoints",
            )?;
            upper.extend_from_slice(pending.parent.upper());
            let mut leader = Vec::new();
            try_reserve_exact(
                &mut leader,
                pending.leader.arity(),
                "requested-domain residual leader coordinates",
            )?;
            leader.extend_from_slice(pending.leader.coordinates());
            let mut symbolic_axes = Vec::new();
            try_reserve_exact(
                &mut symbolic_axes,
                pending.symbolic_axes.len(),
                "requested symbolic axes",
            )?;
            symbolic_axes.extend_from_slice(pending.symbolic_axes);
            let mut fixed_indices = Vec::new();
            try_reserve_exact(
                &mut fixed_indices,
                pending.fixed_indices.len(),
                "requested fixed indices",
            )?;
            fixed_indices.extend_from_slice(&pending.fixed_indices);
            let mut requested_domain_lower = Vec::new();
            try_reserve_exact(
                &mut requested_domain_lower,
                pending.requested_domain_lower.len(),
                "requested-domain lower endpoints",
            )?;
            requested_domain_lower.extend_from_slice(pending.requested_domain_lower);
            let mut requested_domain_upper = Vec::new();
            try_reserve_exact(
                &mut requested_domain_upper,
                pending.requested_domain_upper.len(),
                "requested-domain upper endpoints",
            )?;
            requested_domain_upper.extend_from_slice(&pending.requested_domain_upper);
            let mut residual_domain_upper = Vec::new();
            try_reserve_exact(
                &mut residual_domain_upper,
                pending.residual_domain_upper.len(),
                "requested-domain residual upper endpoints",
            )?;
            residual_domain_upper.extend_from_slice(&pending.residual_domain_upper);
            let canonical_ordinal = tasks.len();
            tasks.push(RequestedDomainTask::new(
                epoch_identity.clone(),
                epoch_ordinal,
                canonical_ordinal,
                RequestedDomainTaskKey::new(
                    scope.key.clone(),
                    LeaderWalkBoxKey::new(lower, upper),
                    pending.requested_ordinal,
                    Arc::new(leader),
                    Arc::new(symbolic_axes),
                    Arc::new(fixed_indices),
                    Arc::new(requested_domain_lower),
                    Arc::new(requested_domain_upper),
                    Arc::new(residual_domain_upper),
                ),
                pending.target_shift.clone(),
            ));
        }
    }
    if tasks.len() != scheduled_residuals || fully_covered_domains > requested_domains {
        return Err(LeaderWalkPlanError::Invariant {
            detail: "requested-domain attachment lost a residual task",
        });
    }

    let mut declared_scopes = Vec::new();
    try_reserve_exact(
        &mut declared_scopes,
        pending_scopes.len(),
        "retained requested semantic scopes",
    )?;
    declared_scopes.extend(pending_scopes.iter().map(|scope| scope.key.clone()));

    Ok(RequestedDomainPlan {
        epoch_identity,
        epoch_ordinal,
        declared_scopes: declared_scopes.into_boxed_slice(),
        input_scope_count: pending_scopes.len(),
        requested_domain_count: requested_domains,
        fully_covered_domain_count: fully_covered_domains,
        tasks,
    })
}

fn intersect_requested_domain(
    requested_lower: &[u64],
    requested_upper: &[Option<u64>],
    parent: &LatticeBox,
) -> Result<Option<(LatticePoint, Vec<Option<u64>>)>, LeaderWalkPlanError> {
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    try_reserve_exact(
        &mut lower,
        requested_lower.len(),
        "requested-domain residual lower endpoints",
    )?;
    try_reserve_exact(
        &mut upper,
        requested_upper.len(),
        "requested-domain residual upper endpoints",
    )?;
    for ((&requested_lower, &requested_upper), (&parent_lower, &parent_upper)) in requested_lower
        .iter()
        .zip(requested_upper)
        .zip(parent.lower().iter().zip(parent.upper()))
    {
        let intersection_lower = requested_lower.max(parent_lower);
        let intersection_upper = min_upper(requested_upper, parent_upper);
        if intersection_upper.is_some_and(|upper| upper < intersection_lower) {
            return Ok(None);
        }
        lower.push(intersection_lower);
        upper.push(intersection_upper);
    }
    Ok(Some((LatticePoint::try_new(lower)?, upper)))
}

fn min_upper(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(left), Some(right)) => Some(left.min(right)),
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, LeaderWalkPlanError> {
    left.checked_add(right)
        .ok_or(LeaderWalkPlanError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, LeaderWalkPlanError> {
    left.checked_mul(right)
        .ok_or(LeaderWalkPlanError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), LeaderWalkPlanError> {
    if requested > limit {
        Err(LeaderWalkPlanError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn try_reserve_one<T>(
    retained: &mut Vec<T>,
    resource: &'static str,
) -> Result<(), LeaderWalkPlanError> {
    let requested = checked_add(resource, retained.len(), 1)?;
    retained
        .try_reserve_exact(1)
        .map_err(|_| LeaderWalkPlanError::AllocationFailure {
            resource,
            requested,
        })
}

fn try_reserve_exact<T>(
    retained: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), LeaderWalkPlanError> {
    let requested = checked_add(resource, retained.len(), additional)?;
    retained
        .try_reserve_exact(additional)
        .map_err(|_| LeaderWalkPlanError::AllocationFailure {
            resource,
            requested,
        })
}
