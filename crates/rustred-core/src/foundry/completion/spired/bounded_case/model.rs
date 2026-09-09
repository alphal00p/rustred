use crate::foundry::completion::stratum::{DecoratedStratum, DecoratedStratumId};
use crate::sector::InteriorBounds;

/// How one coordinate contributes to the finite-depth bulk partition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredBoundedCaseAxisDisposition {
    /// Active coordinates never create inactive-line activation faces.
    Active,
    /// A singleton inactive coordinate is searched with activation forbidden.
    FixedInactive,
    /// The complete parent interval already lies in the safe bulk.
    FullParentBulk,
    /// A nonempty bulk and one or more finite boundary faces were retained.
    SplitBoundaryFaces,
    /// No safe bulk point exists; discovery retains the parent interval and
    /// keeps activating columns forbidden on this axis.
    NoNonEmptyBulk,
}

/// Exact target-relative source envelope for one coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseAxisEnvelope {
    position: usize,
    parent_bounds: InteriorBounds,
    max_positive_relative_shift: i64,
    base_safe_upper: Option<i64>,
    retained_bulk_bounds: InteriorBounds,
    excluded_boundary_bounds: Option<InteriorBounds>,
    equality_face_count: usize,
    disposition: SpiredBoundedCaseAxisDisposition,
}

impl SpiredBoundedCaseAxisEnvelope {
    pub(super) const fn new(
        position: usize,
        parent_bounds: InteriorBounds,
        max_positive_relative_shift: i64,
        base_safe_upper: Option<i64>,
        retained_bulk_bounds: InteriorBounds,
        excluded_boundary_bounds: Option<InteriorBounds>,
        equality_face_count: usize,
        disposition: SpiredBoundedCaseAxisDisposition,
    ) -> Self {
        Self {
            position,
            parent_bounds,
            max_positive_relative_shift,
            base_safe_upper,
            retained_bulk_bounds,
            excluded_boundary_bounds,
            equality_face_count,
            disposition,
        }
    }

    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn parent_bounds(self) -> InteriorBounds {
        self.parent_bounds
    }

    /// `max_{row term q} max(0, q_i - p_i + d)`.
    pub(crate) const fn max_positive_relative_shift(self) -> i64 {
        self.max_positive_relative_shift
    }

    /// Largest safe base index on an inactive axis.  If `p_i` is the target
    /// shift and `M_i` the relative envelope, this is
    /// `min(0, -p_i-M_i)`. Active axes return `None`.
    pub(crate) const fn base_safe_upper(self) -> Option<i64> {
        self.base_safe_upper
    }

    pub(crate) const fn retained_bulk_bounds(self) -> InteriorBounds {
        self.retained_bulk_bounds
    }

    pub(crate) const fn excluded_boundary_bounds(self) -> Option<InteriorBounds> {
        self.excluded_boundary_bounds
    }

    pub(crate) const fn equality_face_count(self) -> usize {
        self.equality_face_count
    }

    pub(crate) const fn disposition(self) -> SpiredBoundedCaseAxisDisposition {
        self.disposition
    }
}

/// Complete coordinate-wise proof envelope for all source shells through one
/// signed-L1 depth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseShiftEnvelope {
    max_source_depth: usize,
    axes: Box<[SpiredBoundedCaseAxisEnvelope]>,
}

impl SpiredBoundedCaseShiftEnvelope {
    pub(super) fn from_parts(
        max_source_depth: usize,
        axes: Vec<SpiredBoundedCaseAxisEnvelope>,
    ) -> Self {
        Self {
            max_source_depth,
            axes: axes.into_boxed_slice(),
        }
    }

    pub(crate) const fn max_source_depth(&self) -> usize {
        self.max_source_depth
    }

    pub(crate) fn axes(&self) -> &[SpiredBoundedCaseAxisEnvelope] {
        &self.axes
    }
}

/// One execution-only rectangular case inside a logical equality obligation.
///
/// Unlike `SpiredCoordinateCaseObligation`, this value may contain proper
/// finite-depth or representability intervals. It is proposal geometry only
/// and cannot enter the equality worklist or acquire closure authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedWorkingCase {
    stratum: DecoratedStratum,
}

impl SpiredBoundedWorkingCase {
    pub(super) const fn new(stratum: DecoratedStratum) -> Self {
        Self { stratum }
    }

    pub(crate) const fn stratum(&self) -> &DecoratedStratum {
        &self.stratum
    }
}

/// One broad working equality face excluded from a safe bulk coordinate.
///
/// Other coordinates retain their logical-parent bounds deliberately. This is
/// still only a search partition: an exact replay/activation proof must
/// separately authorize the corresponding logical equality child before it
/// can enter the case worklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseEqualityFace {
    position: usize,
    value: i64,
    working_case: SpiredBoundedWorkingCase,
}

impl SpiredBoundedCaseEqualityFace {
    pub(super) const fn new(
        position: usize,
        value: i64,
        working_case: SpiredBoundedWorkingCase,
    ) -> Self {
        Self {
            position,
            value,
            working_case,
        }
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) const fn value(&self) -> i64 {
        self.value
    }

    pub(crate) const fn working_case(&self) -> &SpiredBoundedWorkingCase {
        &self.working_case
    }
}

/// Work and retained-geometry census for one bounded partition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseEnvelopeCensus {
    pub(super) source_rows: usize,
    pub(super) source_terms: usize,
    pub(super) source_coordinate_cells: usize,
    pub(super) active_axes: usize,
    pub(super) fixed_inactive_axes: usize,
    pub(super) full_parent_bulk_axes: usize,
    pub(super) split_axes: usize,
    pub(super) no_nonempty_bulk_axes: usize,
    pub(super) equality_faces: usize,
    pub(super) retained_domains: usize,
    pub(super) retained_domain_bound_cells: usize,
}

impl SpiredBoundedCaseEnvelopeCensus {
    pub(crate) const fn source_rows(self) -> usize {
        self.source_rows
    }

    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn source_coordinate_cells(self) -> usize {
        self.source_coordinate_cells
    }

    pub(crate) const fn active_axes(self) -> usize {
        self.active_axes
    }

    pub(crate) const fn fixed_inactive_axes(self) -> usize {
        self.fixed_inactive_axes
    }

    pub(crate) const fn full_parent_bulk_axes(self) -> usize {
        self.full_parent_bulk_axes
    }

    pub(crate) const fn split_axes(self) -> usize {
        self.split_axes
    }

    pub(crate) const fn no_nonempty_bulk_axes(self) -> usize {
        self.no_nonempty_bulk_axes
    }

    pub(crate) const fn equality_faces(self) -> usize {
        self.equality_faces
    }

    pub(crate) const fn retained_domains(self) -> usize {
        self.retained_domains
    }

    pub(crate) const fn retained_domain_bound_cells(self) -> usize {
        self.retained_domain_bound_cells
    }
}

/// Exact proposal geometry for one finite-depth translated-source search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredBoundedCaseEnvelope {
    declared_carrier: DecoratedStratumId,
    parent: DecoratedStratumId,
    bulk: SpiredBoundedWorkingCase,
    equality_faces: Box<[SpiredBoundedCaseEqualityFace]>,
    shift_envelope: SpiredBoundedCaseShiftEnvelope,
    census: SpiredBoundedCaseEnvelopeCensus,
}

impl SpiredBoundedCaseEnvelope {
    pub(super) fn from_parts(
        declared_carrier: DecoratedStratumId,
        parent: DecoratedStratumId,
        bulk: SpiredBoundedWorkingCase,
        equality_faces: Vec<SpiredBoundedCaseEqualityFace>,
        shift_envelope: SpiredBoundedCaseShiftEnvelope,
        census: SpiredBoundedCaseEnvelopeCensus,
    ) -> Self {
        Self {
            declared_carrier,
            parent,
            bulk,
            equality_faces: equality_faces.into_boxed_slice(),
            shift_envelope,
            census,
        }
    }

    pub(crate) const fn declared_carrier(&self) -> &DecoratedStratumId {
        &self.declared_carrier
    }

    pub(crate) const fn parent(&self) -> &DecoratedStratumId {
        &self.parent
    }

    pub(crate) const fn bulk(&self) -> &SpiredBoundedWorkingCase {
        &self.bulk
    }

    pub(crate) fn equality_faces(&self) -> &[SpiredBoundedCaseEqualityFace] {
        &self.equality_faces
    }

    pub(crate) const fn shift_envelope(&self) -> &SpiredBoundedCaseShiftEnvelope {
        &self.shift_envelope
    }

    pub(crate) const fn census(&self) -> SpiredBoundedCaseEnvelopeCensus {
        self.census
    }
}
