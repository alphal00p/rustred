use std::mem::size_of;
use std::sync::Arc;

use crate::identity::TranslatedSourceRequest;

use super::super::DirectShiftedSourceCorpusCensus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredPreparedTermRole {
    Target,
    Allowed,
    Forbidden(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredPreparedRowPlan {
    ordinal: usize,
    request: TranslatedSourceRequest,
    term_roles: Box<[SpiredPreparedTermRole]>,
    forbidden_frontier: usize,
}

impl SpiredPreparedRowPlan {
    pub(super) fn new(
        ordinal: usize,
        request: TranslatedSourceRequest,
        term_roles: Box<[SpiredPreparedTermRole]>,
        forbidden_frontier: usize,
    ) -> Self {
        Self {
            ordinal,
            request,
            term_roles,
            forbidden_frontier,
        }
    }

    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn request(&self) -> &TranslatedSourceRequest {
        &self.request
    }

    pub(crate) fn term_roles(&self) -> &[SpiredPreparedTermRole] {
        &self.term_roles
    }

    /// Number of compact forbidden IDs known after this row's structural
    /// scan. Every forbidden role in this row must name an ID below this
    /// frontier. Probe-local reducers still register only columns occurring in
    /// admitted rows, so exclusion branches never acquire unused columns.
    pub(crate) const fn forbidden_frontier(&self) -> usize {
        self.forbidden_frontier
    }

    pub(crate) fn view(&self) -> SpiredPreparedRowView<'_> {
        SpiredPreparedRowView::new(
            self.ordinal,
            &self.request,
            &self.term_roles,
            self.forbidden_frontier,
        )
    }
}

/// One prepared-row header whose roles live in a separate flat arena.
///
/// Target workspaces append these compact headers and all term roles to two
/// amortized vectors. The request's integral shift is already `Arc`-backed, so
/// cloning it into the retained header does not allocate a coordinate buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredPreparedRowSpan {
    ordinal: usize,
    request: TranslatedSourceRequest,
    first_term_role: usize,
    term_role_count: usize,
    forbidden_frontier: usize,
}

impl SpiredPreparedRowSpan {
    pub(super) const fn new(
        ordinal: usize,
        request: TranslatedSourceRequest,
        first_term_role: usize,
        term_role_count: usize,
        forbidden_frontier: usize,
    ) -> Self {
        Self {
            ordinal,
            request,
            first_term_role,
            term_role_count,
            forbidden_frontier,
        }
    }

    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn request(&self) -> &TranslatedSourceRequest {
        &self.request
    }

    pub(crate) const fn term_role_count(&self) -> usize {
        self.term_role_count
    }

    pub(crate) fn try_view<'row>(
        &'row self,
        term_roles: &'row [SpiredPreparedTermRole],
    ) -> Option<SpiredPreparedRowView<'row>> {
        let end = self.first_term_role.checked_add(self.term_role_count)?;
        Some(SpiredPreparedRowView::new(
            self.ordinal,
            &self.request,
            term_roles.get(self.first_term_role..end)?,
            self.forbidden_frontier,
        ))
    }
}

/// Allocation-free borrowed view shared by chunk-backed and arena-backed rows.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpiredPreparedRowView<'row> {
    ordinal: usize,
    request: &'row TranslatedSourceRequest,
    term_roles: &'row [SpiredPreparedTermRole],
    forbidden_frontier: usize,
}

impl<'row> SpiredPreparedRowView<'row> {
    const fn new(
        ordinal: usize,
        request: &'row TranslatedSourceRequest,
        term_roles: &'row [SpiredPreparedTermRole],
        forbidden_frontier: usize,
    ) -> Self {
        Self {
            ordinal,
            request,
            term_roles,
            forbidden_frontier,
        }
    }

    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn request(self) -> &'row TranslatedSourceRequest {
        self.request
    }

    pub(crate) const fn term_roles(self) -> &'row [SpiredPreparedTermRole] {
        self.term_roles
    }

    pub(crate) const fn forbidden_frontier(self) -> usize {
        self.forbidden_frontier
    }
}

/// Non-forgeable live identity joining plans and probes to one exact prepared
/// case/source/owner snapshot. Equality is pointer identity, never a hash.
#[derive(Debug)]
pub(crate) struct SpiredStructuralScopeIdentity {
    _private: (),
}

impl SpiredStructuralScopeIdentity {
    pub(super) const fn new() -> Self {
        Self { _private: () }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SpiredPreparedRequestChunk {
    scope: Arc<SpiredStructuralScopeIdentity>,
    first_row_ordinal: usize,
    rows: Arc<[SpiredPreparedRowPlan]>,
}

impl SpiredPreparedRequestChunk {
    pub(super) fn new(
        scope: Arc<SpiredStructuralScopeIdentity>,
        first_row_ordinal: usize,
        rows: Box<[SpiredPreparedRowPlan]>,
    ) -> Self {
        Self {
            scope,
            first_row_ordinal,
            rows: Arc::from(rows),
        }
    }

    pub(crate) const fn first_row_ordinal(&self) -> usize {
        self.first_row_ordinal
    }

    pub(crate) fn rows(&self) -> &[SpiredPreparedRowPlan] {
        &self.rows
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub(in crate::foundry::completion::spired) const fn scope(
        &self,
    ) -> &Arc<SpiredStructuralScopeIdentity> {
        &self.scope
    }

    pub(crate) fn same_structural_plan_as(&self, other: &Self) -> bool {
        self.first_row_ordinal == other.first_row_ordinal && self.rows == other.rows
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredStructuralPreparationCensus {
    exact_sources: DirectShiftedSourceCorpusCensus,
    prepared_rows: usize,
    request_inspections: usize,
    source_term_inspections: usize,
    unique_shifts: usize,
    unique_shift_coordinate_cells: usize,
    forbidden_columns: usize,
    target_terms: usize,
    allowed_terms: usize,
    forbidden_terms: usize,
    target_sector_cells: usize,
    owner_probes: usize,
    retained_owner_witnesses: usize,
    emitted_request_offset_cells: usize,
    emitted_term_roles: usize,
}

impl SpiredStructuralPreparationCensus {
    pub(super) const fn with_exact_sources(exact_sources: DirectShiftedSourceCorpusCensus) -> Self {
        Self {
            exact_sources,
            prepared_rows: 0,
            request_inspections: 0,
            source_term_inspections: 0,
            unique_shifts: 0,
            unique_shift_coordinate_cells: 0,
            forbidden_columns: 0,
            target_terms: 0,
            allowed_terms: 0,
            forbidden_terms: 0,
            target_sector_cells: 0,
            owner_probes: 0,
            retained_owner_witnesses: 0,
            emitted_request_offset_cells: 0,
            emitted_term_roles: 0,
        }
    }

    pub(crate) const fn exact_sources(self) -> DirectShiftedSourceCorpusCensus {
        self.exact_sources
    }
    pub(crate) const fn prepared_rows(self) -> usize {
        self.prepared_rows
    }
    pub(crate) const fn request_inspections(self) -> usize {
        self.request_inspections
    }
    pub(crate) const fn source_term_inspections(self) -> usize {
        self.source_term_inspections
    }
    pub(crate) const fn unique_shifts(self) -> usize {
        self.unique_shifts
    }
    pub(crate) const fn unique_shift_coordinate_cells(self) -> usize {
        self.unique_shift_coordinate_cells
    }
    pub(crate) const fn forbidden_columns(self) -> usize {
        self.forbidden_columns
    }
    pub(crate) const fn target_terms(self) -> usize {
        self.target_terms
    }
    pub(crate) const fn allowed_terms(self) -> usize {
        self.allowed_terms
    }
    pub(crate) const fn forbidden_terms(self) -> usize {
        self.forbidden_terms
    }
    pub(crate) const fn target_sector_cells(self) -> usize {
        self.target_sector_cells
    }
    pub(crate) const fn owner_probes(self) -> usize {
        self.owner_probes
    }
    pub(crate) const fn retained_owner_witnesses(self) -> usize {
        self.retained_owner_witnesses
    }
    pub(crate) const fn emitted_request_offset_cells(self) -> usize {
        self.emitted_request_offset_cells
    }
    pub(crate) const fn emitted_term_roles(self) -> usize {
        self.emitted_term_roles
    }

    /// Deterministic lower bound for immutable row-plan payload emitted by the
    /// planner. Allocator metadata and `Arc` control blocks are excluded.
    pub(crate) const fn emitted_plan_payload_bytes_lower_bound(self) -> usize {
        self.prepared_rows
            .saturating_mul(size_of::<SpiredPreparedRowPlan>())
            .saturating_add(
                self.emitted_term_roles
                    .saturating_mul(size_of::<SpiredPreparedTermRole>()),
            )
            .saturating_add(
                self.emitted_request_offset_cells
                    .saturating_mul(size_of::<i64>()),
            )
    }

    /// Deterministic lower bound for the serial role registry's owned payload.
    /// Hash-table bucket and allocator overhead are deliberately excluded.
    pub(crate) const fn registry_payload_bytes_lower_bound(self) -> usize {
        self.unique_shift_coordinate_cells
            .saturating_mul(size_of::<i64>())
            .saturating_add(self.unique_shifts.saturating_mul(
                size_of::<Vec<i64>>().saturating_add(size_of::<SpiredPreparedTermRole>()),
            ))
    }

    pub(super) fn update_registry(
        &mut self,
        unique_shifts: usize,
        coordinate_cells: usize,
        forbidden_columns: usize,
        target_sector_cells: usize,
        owner_probes: usize,
        retained_owner_witnesses: usize,
    ) {
        self.unique_shifts = unique_shifts;
        self.unique_shift_coordinate_cells = coordinate_cells;
        self.forbidden_columns = forbidden_columns;
        self.target_sector_cells = target_sector_cells;
        self.owner_probes = owner_probes;
        self.retained_owner_witnesses = retained_owner_witnesses;
    }

    pub(super) fn update_emitted(
        &mut self,
        rows: usize,
        request_inspections: usize,
        source_terms: usize,
        target_terms: usize,
        allowed_terms: usize,
        forbidden_terms: usize,
        offset_cells: usize,
        term_roles: usize,
    ) {
        self.prepared_rows = rows;
        self.request_inspections = request_inspections;
        self.source_term_inspections = source_terms;
        self.target_terms = target_terms;
        self.allowed_terms = allowed_terms;
        self.forbidden_terms = forbidden_terms;
        self.emitted_request_offset_cells = offset_cells;
        self.emitted_term_roles = term_roles;
    }
}
