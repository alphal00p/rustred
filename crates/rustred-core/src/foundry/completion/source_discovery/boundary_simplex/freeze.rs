use super::super::simplex_support::try_finite_assignment_count;
use super::BoundarySimplexPlanError;
use super::canonical::CanonicalScope;
use super::combinatorics::try_unrank_axis_subset;
use super::model::{BoundarySimplexFaceKey, BoundarySimplexParentBoxKey, BoundarySimplexScopeKey};
use super::preflight::SCHEDULER_VISITS;
use super::resource::{checked_add, try_copy_string, try_reserve_exact};

const FINITE_ASSIGNMENTS: &str = "parent finite coordinate assignments";

pub(super) struct FrozenParent {
    key: BoundarySimplexParentBoxKey,
    free_axes: Vec<usize>,
    finite_assignment_count: usize,
}

pub(super) struct FrozenScope {
    key: BoundarySimplexScopeKey,
    parents: Vec<FrozenParent>,
}

pub(super) struct FrozenFace {
    pub(super) scope: BoundarySimplexScopeKey,
    pub(super) key: BoundarySimplexFaceKey,
    pub(super) finite_assignment_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FlatParentPosition {
    scope_ordinal: usize,
    parent_ordinal: usize,
}

impl FlatParentPosition {
    const UNINITIALIZED: Self = Self {
        scope_ordinal: usize::MAX,
        parent_ordinal: usize::MAX,
    };
}

pub(super) fn try_freeze_parents(
    canonical: &[CanonicalScope<'_>],
    parent_dimension: usize,
    selected_scope_count: usize,
    selected_parent_count: usize,
) -> Result<Vec<FrozenScope>, BoundarySimplexPlanError> {
    let mut frozen = Vec::new();
    try_reserve_exact(
        &mut frozen,
        selected_scope_count,
        "selected canonical scopes",
    )?;
    let mut retained_parents = 0usize;
    for scope in canonical {
        let count = scope
            .boxes
            .iter()
            .filter(|lattice_box| lattice_box.free_dimension() == parent_dimension)
            .count();
        if count == 0 {
            continue;
        }
        let scope_key = BoundarySimplexScopeKey::new(
            try_copy_string(scope.input.stable_scope_key, "stable scope key")?,
            scope.input.sector.clone(),
        );
        let mut parents = Vec::new();
        try_reserve_exact(&mut parents, count, "selected parent boxes")?;
        for lattice_box in scope
            .boxes
            .iter()
            .copied()
            .filter(|lattice_box| lattice_box.free_dimension() == parent_dimension)
        {
            let mut lower = Vec::new();
            try_reserve_exact(
                &mut lower,
                lattice_box.arity(),
                "selected parent lower endpoints",
            )?;
            lower.extend_from_slice(lattice_box.lower());
            let mut upper = Vec::new();
            try_reserve_exact(
                &mut upper,
                lattice_box.arity(),
                "selected parent upper endpoints",
            )?;
            upper.extend_from_slice(lattice_box.upper());
            let mut free_axes = Vec::new();
            try_reserve_exact(&mut free_axes, parent_dimension, "parent free axes")?;
            free_axes.extend(
                lattice_box
                    .upper()
                    .iter()
                    .enumerate()
                    .filter_map(|(position, upper)| upper.is_none().then_some(position)),
            );
            let finite_assignment_count = try_finite_assignment_count(
                lattice_box.lower(),
                lattice_box.upper(),
                FINITE_ASSIGNMENTS,
            )?;
            parents.push(FrozenParent {
                key: BoundarySimplexParentBoxKey::new(lower, upper),
                free_axes,
                finite_assignment_count,
            });
            retained_parents = checked_add("selected parent boxes", retained_parents, 1)?;
        }
        frozen.push(FrozenScope {
            key: scope_key,
            parents,
        });
    }
    if frozen.len() != selected_scope_count || retained_parents != selected_parent_count {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "frozen parent geometry differed from its exact preflight",
        });
    }
    Ok(frozen)
}

pub(super) fn try_build_faces(
    scopes: &[FrozenScope],
    selected_parent_count: usize,
    expected_round_count: usize,
    codimension: usize,
    faces_per_parent: usize,
    expected_face_count: usize,
) -> Result<(Vec<FrozenFace>, usize), BoundarySimplexPlanError> {
    let (flat_parents, mut visits) =
        try_flatten_parents(scopes, selected_parent_count, expected_round_count)?;
    let mut faces = Vec::new();
    try_reserve_exact(&mut faces, expected_face_count, "boundary faces")?;
    // Subset-rank rounds prevent one parent from monopolizing face creation.
    for subset_ordinal in 0..faces_per_parent {
        for position in &flat_parents {
            visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
            let scope =
                scopes
                    .get(position.scope_ordinal)
                    .ok_or(BoundarySimplexPlanError::Invariant {
                        detail: "a flattened parent referenced no selected scope",
                    })?;
            let parent = scope.parents.get(position.parent_ordinal).ok_or(
                BoundarySimplexPlanError::Invariant {
                    detail: "a flattened parent referenced no selected parent",
                },
            )?;
            let (pinned_axes, remaining_axes) =
                try_unrank_axis_subset(&parent.free_axes, codimension, subset_ordinal)?;
            faces.push(FrozenFace {
                scope: scope.key.clone(),
                key: BoundarySimplexFaceKey::new(parent.key.clone(), pinned_axes, remaining_axes),
                finite_assignment_count: parent.finite_assignment_count,
            });
        }
    }
    if faces.len() != expected_face_count {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "face construction differed from its exact binomial preflight",
        });
    }
    Ok((faces, visits))
}

fn try_flatten_parents(
    scopes: &[FrozenScope],
    selected_parent_count: usize,
    expected_round_count: usize,
) -> Result<(Vec<FlatParentPosition>, usize), BoundarySimplexPlanError> {
    let round_count = scopes.iter().map(|scope| scope.parents.len()).max().ok_or(
        BoundarySimplexPlanError::Invariant {
            detail: "selected boundary geometry has no scopes",
        },
    )?;
    if round_count == 0 || round_count != expected_round_count {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "parent round count differed from its exact preflight",
        });
    }
    let mut round_starts = Vec::new();
    try_reserve_exact(
        &mut round_starts,
        round_count,
        "parent scheduler round starts",
    )?;
    round_starts.resize(round_count, 0usize);
    let mut visits = 0usize;
    for scope in scopes {
        for parent_ordinal in 0..scope.parents.len() {
            visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
            round_starts[parent_ordinal] = checked_add(
                "parents in one scheduler round",
                round_starts[parent_ordinal],
                1,
            )?;
        }
    }
    let mut flattened_count = 0usize;
    for start in &mut round_starts {
        visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
        let count = *start;
        *start = flattened_count;
        flattened_count = checked_add("flattened parents", flattened_count, count)?;
    }
    if flattened_count != selected_parent_count {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "parent scheduler rounds differed from selected-parent preflight",
        });
    }
    let mut round_written = Vec::new();
    try_reserve_exact(
        &mut round_written,
        round_count,
        "parent scheduler round cursors",
    )?;
    round_written.resize(round_count, 0usize);
    let mut flattened = Vec::new();
    try_reserve_exact(&mut flattened, selected_parent_count, "flattened parents")?;
    flattened.resize(selected_parent_count, FlatParentPosition::UNINITIALIZED);
    for (scope_ordinal, scope) in scopes.iter().enumerate() {
        for parent_ordinal in 0..scope.parents.len() {
            visits = checked_add(SCHEDULER_VISITS, visits, 1)?;
            let position = checked_add(
                "flattened parent position",
                round_starts[parent_ordinal],
                round_written[parent_ordinal],
            )?;
            let slot = flattened
                .get_mut(position)
                .ok_or(BoundarySimplexPlanError::Invariant {
                    detail: "a parent scheduler round exceeded its allocation",
                })?;
            if *slot != FlatParentPosition::UNINITIALIZED {
                return Err(BoundarySimplexPlanError::Invariant {
                    detail: "two parents occupied one flattened position",
                });
            }
            *slot = FlatParentPosition {
                scope_ordinal,
                parent_ordinal,
            };
            round_written[parent_ordinal] = checked_add(
                "parent scheduler round cursor",
                round_written[parent_ordinal],
                1,
            )?;
        }
    }
    if flattened
        .iter()
        .any(|position| *position == FlatParentPosition::UNINITIALIZED)
    {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "the canonical parent scheduler left an uninitialized position",
        });
    }
    Ok((flattened, visits))
}
