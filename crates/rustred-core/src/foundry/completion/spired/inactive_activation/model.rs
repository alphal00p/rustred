use crate::algebra::IndexedCoefficient;
use crate::foundry::completion::stratum::DecoratedStratumId;
use crate::identity::IntegralShift;
use crate::sector::{InteriorBounds, SectorInteriorDomain, SectorMonotoneDomain};

/// One inactive coordinate on which a positive RHS shift can activate a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationAxis {
    position: usize,
    shift: i64,
    safe_bounds: Option<InteriorBounds>,
    activation_bounds: InteriorBounds,
    activation_value_count: usize,
}

impl SpiredInactiveActivationAxis {
    pub(super) const fn new(
        position: usize,
        shift: i64,
        safe_bounds: Option<InteriorBounds>,
        activation_bounds: InteriorBounds,
        activation_value_count: usize,
    ) -> Self {
        Self {
            position,
            shift,
            safe_bounds,
            activation_bounds,
            activation_value_count,
        }
    }

    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn shift(self) -> i64 {
        self.shift
    }

    /// Part of the parent interval on which this coordinate stays inactive.
    pub(crate) const fn safe_bounds(self) -> Option<InteriorBounds> {
        self.safe_bounds
    }

    /// Finite interval whose individual integer values activate this line.
    pub(crate) const fn activation_bounds(self) -> InteriorBounds {
        self.activation_bounds
    }

    pub(crate) const fn activation_value_count(self) -> usize {
        self.activation_value_count
    }
}

/// One exact cell in the deterministic first-activating-coordinate partition.
///
/// Earlier affected axes are restricted to their safe intervals, this axis is
/// fixed to `activation_value`, and later axes retain their parent intervals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationSlice {
    first_activation_axis_ordinal: usize,
    activation_position: usize,
    activation_value: i64,
    domain: SectorInteriorDomain,
}

impl SpiredInactiveActivationSlice {
    pub(super) const fn new(
        first_activation_axis_ordinal: usize,
        activation_position: usize,
        activation_value: i64,
        domain: SectorInteriorDomain,
    ) -> Self {
        Self {
            first_activation_axis_ordinal,
            activation_position,
            activation_value,
            domain,
        }
    }

    pub(crate) const fn first_activation_axis_ordinal(&self) -> usize {
        self.first_activation_axis_ordinal
    }

    pub(crate) const fn activation_position(&self) -> usize {
        self.activation_position
    }

    pub(crate) const fn activation_value(&self) -> i64 {
        self.activation_value
    }

    pub(crate) const fn domain(&self) -> &SectorInteriorDomain {
        &self.domain
    }
}

/// Exact structural geometry for a potentially conditionally allowed RHS term.
///
/// This descriptor deliberately has no admission, coefficient, rule, owner,
/// or publication capability. A later exact circuit join must specialize the
/// *combined* coefficient on every retained activation slice before any term
/// can be pruned or any owner can be minted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredConditionallyAllowedInactiveActivation {
    parent_domain: SectorMonotoneDomain,
    shift: IntegralShift,
    affected_axes: Box<[SpiredInactiveActivationAxis]>,
    safe_interior: Option<SectorInteriorDomain>,
    activation_slices: Box<[SpiredInactiveActivationSlice]>,
    retained_domain_bound_cells: usize,
}

impl SpiredConditionallyAllowedInactiveActivation {
    pub(super) fn from_parts(
        parent_domain: SectorMonotoneDomain,
        shift: IntegralShift,
        affected_axes: Vec<SpiredInactiveActivationAxis>,
        safe_interior: Option<SectorInteriorDomain>,
        activation_slices: Vec<SpiredInactiveActivationSlice>,
        retained_domain_bound_cells: usize,
    ) -> Self {
        Self {
            parent_domain,
            shift,
            affected_axes: affected_axes.into_boxed_slice(),
            safe_interior,
            activation_slices: activation_slices.into_boxed_slice(),
            retained_domain_bound_cells,
        }
    }

    pub(crate) const fn parent_domain(&self) -> &SectorMonotoneDomain {
        &self.parent_domain
    }

    pub(crate) const fn shift(&self) -> &IntegralShift {
        &self.shift
    }

    pub(crate) fn affected_axes(&self) -> &[SpiredInactiveActivationAxis] {
        &self.affected_axes
    }

    /// Cell on which every affected coordinate remains inactive after shift.
    pub(crate) const fn safe_interior(&self) -> Option<&SectorInteriorDomain> {
        self.safe_interior.as_ref()
    }

    /// Pairwise-disjoint cells covering the complement of `safe_interior`.
    pub(crate) fn activation_slices(&self) -> &[SpiredInactiveActivationSlice] {
        &self.activation_slices
    }

    pub(crate) fn retained_domain_count(&self) -> usize {
        self.activation_slices.len() + usize::from(self.safe_interior.is_some())
    }

    pub(crate) const fn retained_domain_bound_cells(&self) -> usize {
        self.retained_domain_bound_cells
    }
}

/// One combined physical term from an exactly replayed candidate relation.
///
/// Exact replay must combine equal physical columns before constructing this
/// view.  The activation bridge authenticates the coefficient against its
/// supplied indexed context, but does not itself prove that the term belongs
/// to an ordinary-source consequence.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpiredReplayedInactiveActivationTerm<'candidate> {
    physical_column: usize,
    shift: &'candidate IntegralShift,
    coefficient: &'candidate IndexedCoefficient,
}

impl<'candidate> SpiredReplayedInactiveActivationTerm<'candidate> {
    pub(crate) const fn new(
        physical_column: usize,
        shift: &'candidate IntegralShift,
        coefficient: &'candidate IndexedCoefficient,
    ) -> Self {
        Self {
            physical_column,
            shift,
            coefficient,
        }
    }

    pub(crate) const fn physical_column(self) -> usize {
        self.physical_column
    }

    pub(crate) const fn shift(self) -> &'candidate IntegralShift {
        self.shift
    }

    pub(crate) const fn coefficient(self) -> &'candidate IndexedCoefficient {
        self.coefficient
    }
}

/// One canonical equality face on which at least one inactive-line term
/// retains a nonzero exact numerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSurvivingInactiveActivationFace {
    position: usize,
    value: i64,
    source_physical_columns: Box<[usize]>,
}

impl SpiredSurvivingInactiveActivationFace {
    pub(super) fn from_parts(
        position: usize,
        value: i64,
        source_physical_columns: Vec<usize>,
    ) -> Self {
        Self {
            position,
            value,
            source_physical_columns: source_physical_columns.into_boxed_slice(),
        }
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) const fn value(&self) -> i64 {
        self.value
    }

    pub(crate) fn source_physical_columns(&self) -> &[usize] {
        &self.source_physical_columns
    }
}

/// One exact application-cell proposal for the replayed relation.
///
/// The cell is part of a pairwise-disjoint partition of the parent box after
/// every unresolved activation face has been removed.  A physical column is
/// listed in `pruned_physical_columns` only when its combined exact
/// coefficient vanishes identically after all singleton coordinates of this
/// cell are substituted.  This remains proposal geometry: a later bridge must
/// map physical columns back to the authenticated rule, rebuild the specialized
/// `RuleCell`, prove descent, intersect guards, and compile ownership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationApplicationCell {
    domain: SectorInteriorDomain,
    pruned_physical_columns: Box<[usize]>,
}

impl SpiredInactiveActivationApplicationCell {
    pub(super) fn from_parts(
        domain: SectorInteriorDomain,
        pruned_physical_columns: Vec<usize>,
    ) -> Self {
        Self {
            domain,
            pruned_physical_columns: pruned_physical_columns.into_boxed_slice(),
        }
    }

    pub(crate) const fn domain(&self) -> &SectorInteriorDomain {
        &self.domain
    }

    pub(crate) fn pruned_physical_columns(&self) -> &[usize] {
        &self.pruned_physical_columns
    }
}

/// Deterministic work retained by one exact activation analysis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationAnalysisCensus {
    pub(super) candidate_terms: usize,
    pub(super) activating_terms: usize,
    pub(super) face_specializations: usize,
    pub(super) vanishing_activation_slices: usize,
    pub(super) surviving_activation_slices: usize,
    pub(super) unique_surviving_faces: usize,
    pub(super) duplicate_surviving_face_sources: usize,
    pub(super) partition_face_values: usize,
    pub(super) application_cell_product_states: usize,
    pub(super) application_cells: usize,
    pub(super) application_cell_bound_cells: usize,
    pub(super) pruned_physical_column_references: usize,
}

impl SpiredInactiveActivationAnalysisCensus {
    pub(crate) const fn candidate_terms(self) -> usize {
        self.candidate_terms
    }

    pub(crate) const fn activating_terms(self) -> usize {
        self.activating_terms
    }

    pub(crate) const fn face_specializations(self) -> usize {
        self.face_specializations
    }

    pub(crate) const fn vanishing_activation_slices(self) -> usize {
        self.vanishing_activation_slices
    }

    pub(crate) const fn surviving_activation_slices(self) -> usize {
        self.surviving_activation_slices
    }

    pub(crate) const fn unique_surviving_faces(self) -> usize {
        self.unique_surviving_faces
    }

    pub(crate) const fn duplicate_surviving_face_sources(self) -> usize {
        self.duplicate_surviving_face_sources
    }

    pub(crate) const fn partition_face_values(self) -> usize {
        self.partition_face_values
    }

    pub(crate) const fn application_cell_product_states(self) -> usize {
        self.application_cell_product_states
    }

    pub(crate) const fn application_cells(self) -> usize {
        self.application_cells
    }

    pub(crate) const fn application_cell_bound_cells(self) -> usize {
        self.application_cell_bound_cells
    }

    pub(crate) const fn pruned_physical_column_references(self) -> usize {
        self.pruned_physical_column_references
    }
}

/// Checked, proposal-only join between exact replay coefficients and boundary
/// geometry.
///
/// `application_cells` are pairwise-disjoint exact cells on which every
/// inspected inactive-line shift either stays in-sector or names a combined
/// coefficient that can be pruned after exact singleton specialization. A
/// future owner compiler must still materialize those prunings and intersect
/// them with guard and descent authority. `surviving_faces` are deliberately
/// broad equality *specifications*: they may overlap, but are not themselves
/// worklist obligations. The driver must apply them to the canonical logical
/// parent case rather than to a temporary finite-depth working envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredInactiveActivationAnalysis {
    parent_stratum_id: DecoratedStratumId,
    application_cells: Box<[SpiredInactiveActivationApplicationCell]>,
    surviving_faces: Box<[SpiredSurvivingInactiveActivationFace]>,
    census: SpiredInactiveActivationAnalysisCensus,
}

impl SpiredInactiveActivationAnalysis {
    pub(super) fn from_parts(
        parent_stratum_id: DecoratedStratumId,
        application_cells: Vec<SpiredInactiveActivationApplicationCell>,
        surviving_faces: Vec<SpiredSurvivingInactiveActivationFace>,
        census: SpiredInactiveActivationAnalysisCensus,
    ) -> Self {
        Self {
            parent_stratum_id,
            application_cells: application_cells.into_boxed_slice(),
            surviving_faces: surviving_faces.into_boxed_slice(),
            census,
        }
    }

    pub(crate) const fn parent_stratum_id(&self) -> &DecoratedStratumId {
        &self.parent_stratum_id
    }

    pub(crate) fn application_cells(&self) -> &[SpiredInactiveActivationApplicationCell] {
        &self.application_cells
    }

    pub(crate) fn surviving_faces(&self) -> &[SpiredSurvivingInactiveActivationFace] {
        &self.surviving_faces
    }

    pub(crate) const fn census(&self) -> SpiredInactiveActivationAnalysisCensus {
        self.census
    }
}
