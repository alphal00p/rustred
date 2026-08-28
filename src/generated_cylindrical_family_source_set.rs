//! Strict family-wide assembly of generated cylindrical persistent sources.
//!
//! V1 is deliberately a raw-sector composition layer.  It retains exactly
//! one empty-cylinder persistent V3 source for every sector in the
//! [`FamilySectorInventoryCertificate::unresolved_solve_order`] queue, in that
//! exact subsector-first order.  The inventory and the one optional generated
//! row span are shared by allocation across all roots.
//!
//! This module does not identify symmetry-unique sectors, map equivalent
//! sectors, manufacture dependent affine starts, run the exceptional
//! fixed-point search, infer masters, or contain loop-count/topology-specific
//! recurrences.  Those are separate certified stages.  A V1 source set is
//! complete for the raw unresolved queue or compilation fails with a typed
//! error; it is never a partial transcript.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use crate::generated_cylindrical_sector_root_start::effective_child_configuration;
use crate::{
    CylindricalPreparePointScheduleError, FamilySectorInventoryCertificate,
    FamilySectorInventoryCompiler, FamilySectorInventoryError, FamilySectorInventoryLimits,
    FamilySectorInventoryStatus, GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA,
    GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA,
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError,
    GeneratedCylindricalPersistentEliminationLimits, GeneratedCylindricalRowSystemCertificate,
    GeneratedCylindricalRowSystemError, GeneratedCylindricalRowSystemLimits,
    GeneratedCylindricalSectorRootStartCertificate, GeneratedCylindricalSectorRootStartError,
    GeneratedCylindricalSectorRootStartLimits, GeneratedSymbolicRowSpanCertificate,
    GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanConfig,
    GeneratedSymbolicRowSpanError, IntegralFamily, IntegralOrderingPolicy,
    ParametricCoefficientContext, ParametricEliminationError, ParametricIbpConfig,
    PowerShiftPolicy, SectorFoundationError, SectorMask, SectorRestrictions, ZeroSectorError,
    ZeroSectorResource,
};

pub const GENERATED_CYLINDRICAL_FAMILY_SOURCE_SET_V1_SCHEMA: &str =
    "rustred-generated-cylindrical-family-source-set-v1";

const SOURCES_RESOURCE: &str = "family cylindrical persistent sources";
const SOURCE_SCOPE_ENTRIES_RESOURCE: &str = "family cylindrical source-scope entries";
const SOURCE_INDEX_BYTES_RESOURCE: &str = "family cylindrical source-index bytes";
const TOTAL_PREPARE_POINTS_RESOURCE: &str = "family cylindrical total prepare points";
const TOTAL_EXPANDED_ROWS_RESOURCE: &str = "family cylindrical total expanded rows";
const TOTAL_RETAINED_ROWS_RESOURCE: &str = "family cylindrical total retained rows";
const TOTAL_PERSISTENT_EVENTS_RESOURCE: &str = "family cylindrical total persistent events";
const TOTAL_PERSISTENT_PIVOTS_RESOURCE: &str = "family cylindrical total persistent pivots";
const BINDING_BYTES_RESOURCE: &str = "family cylindrical binding bytes";
const REPLAY_COMPARISON_UNITS_RESOURCE: &str = "family cylindrical logical replay comparison units";

// One shallow family replay only. Deep inventory, row-span, root, row-system,
// and persistent-certificate replays retain independent proof budgets. A unit
// is one fixed field/branch or one retained item whose identity/order is
// inspected by `verify_shallow`; bytes are never comparison-unit surrogates.
const FIXED_FAMILY_REPLAY_FIELDS: usize = 24;
const PER_SOURCE_REPLAY_FIELDS: usize = 35;

/// Per-child proof budgets plus family-wide retained/work aggregation caps.
///
/// The generated row span carries its own limits inside
/// [`GeneratedSymbolicRowSpanConfig`].  `max_sources` and the source-index
/// limits are checked immediately after inventory construction and before any
/// generated row-span or cylindrical algebra is attempted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalFamilySourceSetLimits {
    pub inventory: FamilySectorInventoryLimits,
    pub sector_root: GeneratedCylindricalSectorRootStartLimits,
    pub row_system: GeneratedCylindricalRowSystemLimits,
    pub persistent: GeneratedCylindricalPersistentEliminationLimits,
    pub max_sources: usize,
    /// Aggregate logical scope surface: every bit in every retained raw
    /// solve-order sector.  Empty-root assignments contribute no entries.
    pub max_source_scope_entries: usize,
    /// Conservative retained source-index surface: every persistent-source
    /// `Arc` handle, every cloned `SectorMask` header, and every sector bit.
    pub max_source_index_bytes: usize,
    pub max_total_prepare_points: usize,
    pub max_total_expanded_rows: usize,
    pub max_total_retained_rows: usize,
    pub max_total_persistent_events: usize,
    pub max_total_persistent_pivots: usize,
    /// Deterministic logical family-binding surface: the full fixed
    /// certificate footprint plus defined identity, restriction, source-index,
    /// and source-budget payloads. This is not a peak allocator-byte claim.
    pub max_binding_bytes: usize,
    /// Logical shallow comparisons only.  Every child replay remains governed
    /// independently by its own exact proof budget.
    pub max_replay_comparison_units: usize,
}

impl Default for GeneratedCylindricalFamilySourceSetLimits {
    fn default() -> Self {
        Self {
            inventory: FamilySectorInventoryLimits::default(),
            sector_root: GeneratedCylindricalSectorRootStartLimits::default(),
            row_system: GeneratedCylindricalRowSystemLimits::default(),
            persistent: GeneratedCylindricalPersistentEliminationLimits::default(),
            max_sources: 1_048_576,
            max_source_scope_entries: 1_000_000_000,
            max_source_index_bytes: 1024 * 1024 * 1024,
            max_total_prepare_points: 100_000_000,
            max_total_expanded_rows: 100_000_000,
            max_total_retained_rows: 100_000_000,
            max_total_persistent_events: 100_000_000,
            max_total_persistent_pivots: 100_000_000,
            max_binding_bytes: 2 * 1024 * 1024 * 1024,
            max_replay_comparison_units: 1_000_000_000,
        }
    }
}

/// Exact family-wide logical census. Aggregate row/event counters are sums of
/// already bounded child certificate statistics, never estimates. Binding
/// bytes follow the explicitly documented family-binding surface rather than
/// claiming all transitive or allocator-retained bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneratedCylindricalFamilySourceSetStats {
    inventory_entries: usize,
    unresolved_sources: usize,
    source_scope_entries: usize,
    source_index_bytes: usize,
    shared_row_spans: usize,
    generated_symbolic_rows: usize,
    sector_roots: usize,
    row_systems: usize,
    persistent_sources: usize,
    total_prepare_points: usize,
    total_expanded_rows: usize,
    total_retained_rows: usize,
    total_persistent_events: usize,
    total_persistent_pivots: usize,
    binding_bytes: usize,
    replay_comparison_units: usize,
}

/// Remaining family-wide allowances projected into one source's child
/// compilers.
///
/// Entries are positionally aligned with [`GeneratedCylindricalFamilySourceSetCertificate::solve_order`].
/// The canonical normalized family configuration remains available through
/// [`GeneratedCylindricalFamilySourceSetCertificate::limits`]; these five
/// values make every stricter operation-local child limit explicit and
/// replayable rather than silently replacing that configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCylindricalFamilySourceBudget {
    prepare_points_remaining: usize,
    expanded_rows_remaining: usize,
    retained_rows_remaining: usize,
    persistent_events_remaining: usize,
    persistent_pivots_remaining: usize,
}

impl GeneratedCylindricalFamilySourceBudget {
    pub const fn prepare_points_remaining(self) -> usize {
        self.prepare_points_remaining
    }

    pub const fn expanded_rows_remaining(self) -> usize {
        self.expanded_rows_remaining
    }

    pub const fn retained_rows_remaining(self) -> usize {
        self.retained_rows_remaining
    }

    pub const fn persistent_events_remaining(self) -> usize {
        self.persistent_events_remaining
    }

    pub const fn persistent_pivots_remaining(self) -> usize {
        self.persistent_pivots_remaining
    }
}

macro_rules! stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

impl GeneratedCylindricalFamilySourceSetStats {
    stats_getters!(
        inventory_entries,
        unresolved_sources,
        source_scope_entries,
        source_index_bytes,
        shared_row_spans,
        generated_symbolic_rows,
        sector_roots,
        row_systems,
        persistent_sources,
        total_prepare_points,
        total_expanded_rows,
        total_retained_rows,
        total_persistent_events,
        total_persistent_pivots,
        binding_bytes,
        replay_comparison_units,
    );
}

/// Complete source set aligned position-for-position with `solve_order`.
#[derive(Clone, Debug)]
pub struct GeneratedCylindricalFamilySourceSetCertificate {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    ibp_config: ParametricIbpConfig,
    row_span_config: GeneratedSymbolicRowSpanConfig,
    through_depth: usize,
    limits: GeneratedCylindricalFamilySourceSetLimits,
    inventory: Arc<FamilySectorInventoryCertificate>,
    row_span: Option<Arc<GeneratedSymbolicRowSpanCertificate>>,
    solve_order: Box<[SectorMask]>,
    source_budgets: Box<[GeneratedCylindricalFamilySourceBudget]>,
    persistent_sources: Box<[Arc<GeneratedCylindricalPersistentEliminationCertificate>]>,
    stats: GeneratedCylindricalFamilySourceSetStats,
}

impl GeneratedCylindricalFamilySourceSetCertificate {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn ibp_config(&self) -> ParametricIbpConfig {
        self.ibp_config
    }

    pub const fn row_span_config(&self) -> GeneratedSymbolicRowSpanConfig {
        self.row_span_config
    }

    pub const fn through_depth(&self) -> usize {
        self.through_depth
    }

    pub fn restrictions(&self) -> &SectorRestrictions {
        self.inventory.restrictions()
    }

    pub fn power_shift_policy(&self) -> PowerShiftPolicy {
        self.inventory.power_shift_policy()
    }

    pub fn ordering(&self) -> IntegralOrderingPolicy {
        self.inventory.ordering()
    }

    pub const fn inventory_arc(&self) -> &Arc<FamilySectorInventoryCertificate> {
        &self.inventory
    }

    pub const fn row_span_arc(&self) -> Option<&Arc<GeneratedSymbolicRowSpanCertificate>> {
        self.row_span.as_ref()
    }

    /// Raw unresolved sectors in the inventory's exact certified solve order.
    pub fn solve_order(&self) -> &[SectorMask] {
        &self.solve_order
    }

    /// Persistent V3 sources in exact positional correspondence with
    /// [`Self::solve_order`], ready for plural-provider consumption.
    pub fn persistent_sources(
        &self,
    ) -> &[Arc<GeneratedCylindricalPersistentEliminationCertificate>] {
        &self.persistent_sources
    }

    /// Per-source aggregate allowances in exact solve-order correspondence.
    pub fn source_budgets(&self) -> &[GeneratedCylindricalFamilySourceBudget] {
        &self.source_budgets
    }

    pub const fn limits(&self) -> GeneratedCylindricalFamilySourceSetLimits {
        self.limits
    }

    pub const fn stats(&self) -> GeneratedCylindricalFamilySourceSetStats {
        self.stats
    }
}

/// A per-sector inventory interruption makes a strict V1 family source set
/// incomplete and therefore unpublishable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalFamilyInventoryInterruption {
    ResourceLimited(ZeroSectorResource),
    Failed(ZeroSectorError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GeneratedCylindricalFamilySourceSetError {
    SchemaMismatch,
    ReplayMismatch {
        detail: &'static str,
    },
    WrongFamily,
    WrongContext,
    WrongContextArity {
        expected: usize,
        actual: usize,
    },
    IncompleteInventory {
        sector: SectorMask,
        interruption: GeneratedCylindricalFamilyInventoryInterruption,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    Sector(SectorFoundationError),
    Inventory(FamilySectorInventoryError),
    RowSpan(GeneratedSymbolicRowSpanError),
    SectorRootConfiguration(GeneratedCylindricalSectorRootStartError),
    SectorRoot {
        solve_ordinal: usize,
        sector: SectorMask,
        error: GeneratedCylindricalSectorRootStartError,
    },
    RowSystem {
        solve_ordinal: usize,
        sector: SectorMask,
        error: GeneratedCylindricalRowSystemError,
    },
    Persistent {
        solve_ordinal: usize,
        sector: SectorMask,
        error: GeneratedCylindricalPersistentEliminationError,
    },
}

impl fmt::Display for GeneratedCylindricalFamilySourceSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("generated cylindrical family source-set schema mismatch")
            }
            Self::ReplayMismatch { detail } => write!(
                formatter,
                "generated cylindrical family source set does not replay: {detail}"
            ),
            Self::WrongFamily => formatter
                .write_str("generated cylindrical family source set belongs to another family"),
            Self::WrongContext => formatter.write_str(
                "generated cylindrical family source set belongs to another K(n) context",
            ),
            Self::WrongContextArity { expected, actual } => write!(
                formatter,
                "generated cylindrical family source-set K(n) context has arity {actual}, expected {expected}"
            ),
            Self::IncompleteInventory {
                sector,
                interruption,
            } => match interruption {
                GeneratedCylindricalFamilyInventoryInterruption::ResourceLimited(resource) => {
                    write!(
                        formatter,
                        "raw sector {sector} has resource-limited zero analysis at {}: requested {}, limit {}",
                        resource.resource(),
                        resource.requested(),
                        resource.limit()
                    )
                }
                GeneratedCylindricalFamilyInventoryInterruption::Failed(error) => {
                    write!(
                        formatter,
                        "raw sector {sector} zero analysis failed: {error}"
                    )
                }
            },
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "{resource} could not reserve {requested} elements"
            ),
            Self::Sector(error) => error.fmt(formatter),
            Self::Inventory(error) => error.fmt(formatter),
            Self::RowSpan(error) => error.fmt(formatter),
            Self::SectorRootConfiguration(error) => error.fmt(formatter),
            Self::SectorRoot {
                solve_ordinal,
                sector,
                error,
            } => write!(
                formatter,
                "sector-root compilation failed for solve entry {solve_ordinal} ({sector}): {error}"
            ),
            Self::RowSystem {
                solve_ordinal,
                sector,
                error,
            } => write!(
                formatter,
                "row-system compilation failed for solve entry {solve_ordinal} ({sector}): {error}"
            ),
            Self::Persistent {
                solve_ordinal,
                sector,
                error,
            } => write!(
                formatter,
                "persistent V3 compilation failed for solve entry {solve_ordinal} ({sector}): {error}"
            ),
        }
    }
}

impl std::error::Error for GeneratedCylindricalFamilySourceSetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sector(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::RowSpan(error) => Some(error),
            Self::SectorRootConfiguration(error) | Self::SectorRoot { error, .. } => Some(error),
            Self::RowSystem { error, .. } => Some(error),
            Self::Persistent { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl From<FamilySectorInventoryError> for GeneratedCylindricalFamilySourceSetError {
    fn from(value: FamilySectorInventoryError) -> Self {
        Self::Inventory(value)
    }
}

impl From<SectorFoundationError> for GeneratedCylindricalFamilySourceSetError {
    fn from(value: SectorFoundationError) -> Self {
        Self::Sector(value)
    }
}

impl From<GeneratedSymbolicRowSpanError> for GeneratedCylindricalFamilySourceSetError {
    fn from(value: GeneratedSymbolicRowSpanError) -> Self {
        Self::RowSpan(value)
    }
}

pub struct GeneratedCylindricalFamilySourceSetCompiler;

impl GeneratedCylindricalFamilySourceSetCompiler {
    /// Compile a strict raw-sector family source set without topology dispatch.
    #[allow(clippy::too_many_arguments)]
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        restrictions: SectorRestrictions,
        power_shift_policy: PowerShiftPolicy,
        ordering: IntegralOrderingPolicy,
        ibp_config: ParametricIbpConfig,
        row_span_config: GeneratedSymbolicRowSpanConfig,
        through_depth: usize,
        mut limits: GeneratedCylindricalFamilySourceSetLimits,
    ) -> Result<
        GeneratedCylindricalFamilySourceSetCertificate,
        GeneratedCylindricalFamilySourceSetError,
    > {
        validate_context(family, context)?;

        // Normalize direct child caps before row-span generation.  The shared
        // root constructor will independently derive and require this same
        // effective configuration for every sector.
        let (row_span_config, sector_root_limits) =
            effective_child_configuration(row_span_config, through_depth, limits.sector_root)
                .map_err(GeneratedCylindricalFamilySourceSetError::SectorRootConfiguration)?;
        limits.sector_root = sector_root_limits;

        let inventory = Arc::new(FamilySectorInventoryCompiler::compile(
            family,
            restrictions,
            power_shift_policy,
            ordering,
            limits.inventory,
        )?);
        require_complete_inventory(&inventory)?;

        let source_count = inventory.unresolved_solve_order().len();
        // This gate intentionally precedes generated row-span construction.
        check_limit(SOURCES_RESOURCE, source_count, limits.max_sources)?;
        let (source_scope_entries, source_index_bytes) =
            source_index_census(source_count, family.denominator_count())?;
        check_limit(
            SOURCE_SCOPE_ENTRIES_RESOURCE,
            source_scope_entries,
            limits.max_source_scope_entries,
        )?;
        check_limit(
            SOURCE_INDEX_BYTES_RESOURCE,
            source_index_bytes,
            limits.max_source_index_bytes,
        )?;

        // The complete deterministic logical family-binding surface is
        // knowable from the inventory census. Keep this gate ahead of
        // solve-order cloning and, critically, generated row-span algebra.
        let binding_bytes = family_binding_bytes(
            family,
            context,
            &inventory,
            source_count,
            source_index_bytes,
        )?;
        check_limit(
            BINDING_BYTES_RESOURCE,
            binding_bytes,
            limits.max_binding_bytes,
        )?;

        let mut solve_order = Vec::new();
        try_reserve_exact(SOURCES_RESOURCE, &mut solve_order, source_count)?;
        for entry in inventory.unresolved_solve_order() {
            let mut bits = Vec::new();
            try_reserve_exact(
                SOURCE_INDEX_BYTES_RESOURCE,
                &mut bits,
                entry.sector().arity(),
            )?;
            bits.extend_from_slice(entry.sector().active_bits());
            solve_order.push(SectorMask::try_from_preallocated(bits)?);
        }
        let mut source_budgets = Vec::new();
        try_reserve_exact(SOURCES_RESOURCE, &mut source_budgets, source_count)?;
        let mut roots = Vec::new();
        try_reserve_exact(SOURCES_RESOURCE, &mut roots, source_count)?;
        let mut row_systems = Vec::new();
        try_reserve_exact(SOURCES_RESOURCE, &mut row_systems, source_count)?;
        let mut persistent_sources = Vec::new();
        try_reserve_exact(SOURCES_RESOURCE, &mut persistent_sources, source_count)?;

        let row_span = if source_count == 0 {
            None
        } else {
            Some(Arc::new(GeneratedSymbolicRowSpanCompiler::compile(
                family,
                context,
                ibp_config,
                row_span_config,
            )?))
        };

        let mut stats = shallow_stats(
            &inventory,
            row_span.as_deref(),
            source_count,
            source_scope_entries,
            source_index_bytes,
            binding_bytes,
            limits,
        )?;

        // Phase 1: construct every empty-cylinder root before any row-system
        // specialization.  Prepare points are constrained inside the child
        // schedule, while the exact expanded rectangle is charged here as
        // soon as each root has fixed its prepare-point count.
        for (solve_ordinal, sector) in solve_order.iter().cloned().enumerate() {
            let shared_row_span = Arc::clone(row_span.as_ref().ok_or(
                GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "nonempty solve order has no shared row span",
                },
            )?);
            let budget = initial_source_budget(stats, limits)?;
            let root_limits = projected_root_limits(limits.sector_root, budget);
            let root = Arc::new(
                GeneratedCylindricalSectorRootStartCertificate::
                    compile_with_replayed_inventory_and_row_span(
                        family,
                        context,
                        Arc::clone(&inventory),
                        sector.clone(),
                        shared_row_span,
                        through_depth,
                        root_limits,
                    )
                    .map_err(|error| {
                        map_root_error(
                            error,
                            solve_ordinal,
                            sector.clone(),
                            stats.total_prepare_points,
                            budget,
                            limits,
                            root_limits,
                        )
                    })?,
            );
            charge_root(&mut stats, &root, row_span.as_deref(), limits)?;
            source_budgets.push(budget);
            roots.push(root);
        }

        // Phase 2: specialize every precharged rectangle before beginning any
        // persistent elimination.  A retained row is also exactly one future
        // persistent event, so the stricter of those two family allowances is
        // projected into the row builder's per-row retention gate.
        for (solve_ordinal, ((sector, root), budget)) in solve_order
            .iter()
            .cloned()
            .zip(roots.into_iter())
            .zip(source_budgets.iter_mut())
            .enumerate()
        {
            budget.retained_rows_remaining = remaining(
                TOTAL_RETAINED_ROWS_RESOURCE,
                stats.total_retained_rows,
                limits.max_total_retained_rows,
            )?;
            budget.persistent_events_remaining = remaining(
                TOTAL_PERSISTENT_EVENTS_RESOURCE,
                stats.total_persistent_events,
                limits.max_total_persistent_events,
            )?;
            let row_limits = projected_row_limits(limits.row_system, *budget);
            let row_system = Arc::new(
                GeneratedCylindricalRowSystemCertificate::compile_from_sector_root(
                    family, context, root, row_limits,
                )
                .map_err(|error| {
                    map_row_error(
                        error,
                        solve_ordinal,
                        sector.clone(),
                        stats,
                        *budget,
                        limits,
                        row_limits,
                    )
                })?,
            );
            charge_row_system(&mut stats, &row_system, limits)?;
            row_systems.push(row_system);
        }

        // Phase 3: only rank-dependent pivot work remains.  Events were
        // precharged exactly in phase 2, while pivot-assumption closures have
        // their own (non-pivot) unit.  Install the family pivot allowance only
        // at the unit-correct elimination pivot-commit gate, so pivot N+1 is
        // never constructed once the family is full.
        for (solve_ordinal, ((sector, row_system), budget)) in solve_order
            .iter()
            .cloned()
            .zip(row_systems.into_iter())
            .zip(source_budgets.iter_mut())
            .enumerate()
        {
            budget.persistent_pivots_remaining = remaining(
                TOTAL_PERSISTENT_PIVOTS_RESOURCE,
                stats.total_persistent_pivots,
                limits.max_total_persistent_pivots,
            )?;
            let persistent_limits = projected_persistent_limits(limits.persistent, *budget);
            let persistent = Arc::new(
                GeneratedCylindricalPersistentEliminationCertificate::compile(
                    family,
                    context,
                    row_system,
                    persistent_limits,
                )
                .map_err(|error| {
                    map_persistent_error(
                        error,
                        solve_ordinal,
                        sector.clone(),
                        stats.total_persistent_pivots,
                        *budget,
                        limits,
                        persistent_limits,
                    )
                })?,
            );
            charge_persistent(&mut stats, &persistent, limits)?;
            persistent_sources.push(persistent);
        }

        let certificate = GeneratedCylindricalFamilySourceSetCertificate {
            schema: GENERATED_CYLINDRICAL_FAMILY_SOURCE_SET_V1_SCHEMA,
            family_fingerprint: family.fingerprint().into(),
            context_fingerprint: context.fingerprint().into(),
            ibp_config,
            row_span_config,
            through_depth,
            limits,
            inventory,
            row_span,
            solve_order: solve_order.into_boxed_slice(),
            source_budgets: source_budgets.into_boxed_slice(),
            persistent_sources: persistent_sources.into_boxed_slice(),
            stats,
        };
        certificate.verify_shallow(family, context)?;
        Ok(certificate)
    }
}

impl GeneratedCylindricalFamilySourceSetCertificate {
    /// Replay every owning proof boundary after checking the family-wide
    /// shallow caps and exact shared-allocation/source-order invariants.
    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
        self.verify_shallow(family, context)?;
        self.inventory.replay(family)?;
        if let Some(row_span) = &self.row_span {
            row_span.replay(family, context)?;
        }
        for (solve_ordinal, (sector, source)) in self
            .solve_order
            .iter()
            .zip(self.persistent_sources.iter())
            .enumerate()
        {
            source.replay(family, context).map_err(|error| {
                GeneratedCylindricalFamilySourceSetError::Persistent {
                    solve_ordinal,
                    sector: sector.clone(),
                    error,
                }
            })?;
        }
        self.verify_shallow(family, context)
    }

    fn verify_shallow(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
        if self.schema != GENERATED_CYLINDRICAL_FAMILY_SOURCE_SET_V1_SCHEMA {
            return Err(GeneratedCylindricalFamilySourceSetError::SchemaMismatch);
        }
        if self.family_fingerprint.as_ref() != family.fingerprint() {
            return Err(GeneratedCylindricalFamilySourceSetError::WrongFamily);
        }
        if self.context_fingerprint.as_ref() != context.fingerprint() {
            return Err(GeneratedCylindricalFamilySourceSetError::WrongContext);
        }
        validate_context(family, context)?;
        if self.inventory.family_fingerprint() != family.fingerprint()
            || self.inventory.limits() != self.limits.inventory
        {
            return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "retained inventory scope or limits differ",
            });
        }
        require_complete_inventory(&self.inventory)?;

        let source_count = self.inventory.unresolved_solve_order().len();
        let (source_scope_entries, source_index_bytes) =
            source_index_census(source_count, family.denominator_count())?;
        let binding_bytes = family_binding_bytes(
            family,
            context,
            &self.inventory,
            source_count,
            source_index_bytes,
        )?;
        let mut recomputed = shallow_stats(
            &self.inventory,
            self.row_span.as_deref(),
            source_count,
            source_scope_entries,
            source_index_bytes,
            binding_bytes,
            self.limits,
        )?;

        if self.solve_order.len() != source_count
            || self.source_budgets.len() != source_count
            || self.persistent_sources.len() != source_count
            || !self
                .solve_order
                .iter()
                .zip(self.inventory.unresolved_solve_order())
                .all(|(sector, inventory_entry)| sector == inventory_entry.sector())
        {
            return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "persistent sources do not preserve the exact inventory solve order",
            });
        }
        match (&self.row_span, source_count) {
            (None, 0) => {}
            (Some(row_span), count) if count != 0 => {
                if row_span.family_fingerprint() != family.fingerprint()
                    || row_span.context_fingerprint() != context.fingerprint()
                    || row_span.ibp_config() != self.ibp_config
                    || row_span.config() != self.row_span_config
                {
                    return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                        detail: "shared generated row span has foreign configuration or scope",
                    });
                }
            }
            _ => {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "shared row-span presence does not match the unresolved source count",
                });
            }
        }

        // Replay the same three family phases as compilation.  Each budget
        // field is authenticated at the point where its consumed-before value
        // is available, then the exact projected child limits are checked.
        for (solve_ordinal, ((sector, budget), source)) in self
            .solve_order
            .iter()
            .zip(self.source_budgets.iter())
            .zip(self.persistent_sources.iter())
            .enumerate()
        {
            let expected_budget = initial_source_budget(recomputed, self.limits)?;
            if budget.prepare_points_remaining != expected_budget.prepare_points_remaining
                || budget.expanded_rows_remaining != expected_budget.expanded_rows_remaining
            {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "source prepare/expanded budget projection differs",
                });
            }
            let row_system = source.row_system();
            let root = row_system.start().sector_root_start().ok_or(
                GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "family source is not based on an empty sector root",
                },
            )?;
            let shared_row_span = self.row_span.as_ref().ok_or(
                GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "nonempty family source has no shared row span",
                },
            )?;
            let expected_root_limits = projected_root_limits(self.limits.sector_root, *budget);
            if root.sector() != sector
                || !root.assignment().is_empty()
                || root.ibp_config() != self.ibp_config
                || root.row_span_config() != self.row_span_config
                || root.ordering_policy() != self.inventory.ordering()
                || root.limits() != expected_root_limits
                || !Arc::ptr_eq(root.inventory_arc(), &self.inventory)
                || !Arc::ptr_eq(root.row_span_arc(), shared_row_span)
            {
                let _ = solve_ordinal;
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "source order, empty-root metadata, or exact shared Arcs differ",
                });
            }
            charge_root(&mut recomputed, root, self.row_span.as_deref(), self.limits)?;
        }

        for (budget, source) in self
            .source_budgets
            .iter()
            .zip(self.persistent_sources.iter())
        {
            let retained_rows_remaining = remaining(
                TOTAL_RETAINED_ROWS_RESOURCE,
                recomputed.total_retained_rows,
                self.limits.max_total_retained_rows,
            )?;
            let persistent_events_remaining = remaining(
                TOTAL_PERSISTENT_EVENTS_RESOURCE,
                recomputed.total_persistent_events,
                self.limits.max_total_persistent_events,
            )?;
            if budget.retained_rows_remaining != retained_rows_remaining
                || budget.persistent_events_remaining != persistent_events_remaining
            {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "source retained/event budget projection differs",
                });
            }
            let row_system = source.row_system();
            let expected_row_limits = projected_row_limits(self.limits.row_system, *budget);
            if row_system.schema() != GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA
                || row_system.family_fingerprint() != family.fingerprint()
                || row_system.context_fingerprint() != context.fingerprint()
                || row_system.limits() != expected_row_limits
            {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "persistent source does not own the projected sector-root V2 row system",
                });
            }
            charge_row_system(&mut recomputed, row_system, self.limits)?;
        }

        for (budget, source) in self
            .source_budgets
            .iter()
            .zip(self.persistent_sources.iter())
        {
            let persistent_pivots_remaining = remaining(
                TOTAL_PERSISTENT_PIVOTS_RESOURCE,
                recomputed.total_persistent_pivots,
                self.limits.max_total_persistent_pivots,
            )?;
            if budget.persistent_pivots_remaining != persistent_pivots_remaining {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "source pivot budget projection differs",
                });
            }
            let expected_persistent_limits =
                projected_persistent_limits(self.limits.persistent, *budget);
            if source.schema() != GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA
                || source.family_fingerprint() != family.fingerprint()
                || source.context_fingerprint() != context.fingerprint()
                || source.limits() != expected_persistent_limits
            {
                return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                    detail: "persistent source has foreign schema, scope, or projected limits",
                });
            }
            charge_persistent(&mut recomputed, source, self.limits)?;
        }

        if recomputed != self.stats {
            return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "family source-set statistics differ",
            });
        }
        Ok(())
    }
}

fn validate_context(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    if !family
        .coefficient_context()
        .has_same_variable_map(context.base())
    {
        return Err(GeneratedCylindricalFamilySourceSetError::WrongContext);
    }
    if family.denominator_count() != context.index_count() {
        return Err(
            GeneratedCylindricalFamilySourceSetError::WrongContextArity {
                expected: family.denominator_count(),
                actual: context.index_count(),
            },
        );
    }
    Ok(())
}

fn require_complete_inventory(
    inventory: &FamilySectorInventoryCertificate,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    for entry in inventory.entries() {
        let interruption = match entry.status() {
            FamilySectorInventoryStatus::ResourceLimited(resource) => Some(
                GeneratedCylindricalFamilyInventoryInterruption::ResourceLimited(resource.clone()),
            ),
            FamilySectorInventoryStatus::Failed(error) => Some(
                GeneratedCylindricalFamilyInventoryInterruption::Failed(error.clone()),
            ),
            FamilySectorInventoryStatus::Excluded(_)
            | FamilySectorInventoryStatus::ProvedZero(_)
            | FamilySectorInventoryStatus::UnresolvedNoZeroCertificate(_) => None,
        };
        if let Some(interruption) = interruption {
            return Err(
                GeneratedCylindricalFamilySourceSetError::IncompleteInventory {
                    sector: entry.sector().clone(),
                    interruption,
                },
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn shallow_stats(
    inventory: &FamilySectorInventoryCertificate,
    row_span: Option<&GeneratedSymbolicRowSpanCertificate>,
    source_count: usize,
    source_scope_entries: usize,
    source_index_bytes: usize,
    binding_bytes: usize,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<GeneratedCylindricalFamilySourceSetStats, GeneratedCylindricalFamilySourceSetError> {
    check_limit(SOURCES_RESOURCE, source_count, limits.max_sources)?;
    check_limit(
        SOURCE_SCOPE_ENTRIES_RESOURCE,
        source_scope_entries,
        limits.max_source_scope_entries,
    )?;
    check_limit(
        SOURCE_INDEX_BYTES_RESOURCE,
        source_index_bytes,
        limits.max_source_index_bytes,
    )?;

    let shared_row_spans = usize::from(row_span.is_some());
    let generated_symbolic_rows = row_span.map_or(0, |span| span.rows().len());
    check_limit(
        BINDING_BYTES_RESOURCE,
        binding_bytes,
        limits.max_binding_bytes,
    )?;

    let per_source_units = checked_mul(
        REPLAY_COMPARISON_UNITS_RESOURCE,
        source_count,
        PER_SOURCE_REPLAY_FIELDS,
    )?;
    // Each source-sector bit is inspected once against the inventory order and
    // once again against the retained root sector.
    let solve_order_sector_units =
        checked_mul(REPLAY_COMPARISON_UNITS_RESOURCE, source_scope_entries, 2)?;
    let replay_comparison_units = checked_sum(
        REPLAY_COMPARISON_UNITS_RESOURCE,
        [
            FIXED_FAMILY_REPLAY_FIELDS,
            inventory.entries().len(),
            generated_symbolic_rows,
            shared_row_spans,
            solve_order_sector_units,
            per_source_units,
        ],
    )?;
    check_limit(
        REPLAY_COMPARISON_UNITS_RESOURCE,
        replay_comparison_units,
        limits.max_replay_comparison_units,
    )?;

    Ok(GeneratedCylindricalFamilySourceSetStats {
        inventory_entries: inventory.entries().len(),
        unresolved_sources: source_count,
        source_scope_entries,
        source_index_bytes,
        shared_row_spans,
        generated_symbolic_rows,
        binding_bytes,
        replay_comparison_units,
        ..GeneratedCylindricalFamilySourceSetStats::default()
    })
}

fn family_binding_bytes(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    inventory: &FamilySectorInventoryCertificate,
    source_count: usize,
    source_index_bytes: usize,
) -> Result<usize, GeneratedCylindricalFamilySourceSetError> {
    // Charge the complete fixed object once.  This covers every field header,
    // enum/Option footprint, stats, and compiler-inserted padding without a
    // brittle hand-maintained list.  Add only dynamic/logical binding payloads
    // below, so no fixed Box/Arc/configuration header is double counted.
    let fixed_bytes = size_of::<GeneratedCylindricalFamilySourceSetCertificate>();
    let logical_identity_bytes = checked_sum(
        BINDING_BYTES_RESOURCE,
        [
            GENERATED_CYLINDRICAL_FAMILY_SOURCE_SET_V1_SCHEMA.len(),
            family.fingerprint().len(),
            context.fingerprint().len(),
            inventory.ordering().stable_id().len(),
            inventory.ordering().key_schema().len(),
        ],
    )?;
    let restriction_bytes =
        checked_mul(BINDING_BYTES_RESOURCE, inventory.restrictions().arity(), 2)?;
    let source_budget_bytes = checked_mul(
        BINDING_BYTES_RESOURCE,
        source_count,
        size_of::<GeneratedCylindricalFamilySourceBudget>(),
    )?;
    checked_sum(
        BINDING_BYTES_RESOURCE,
        [
            fixed_bytes,
            logical_identity_bytes,
            restriction_bytes,
            source_index_bytes,
            source_budget_bytes,
        ],
    )
}

fn charge_root(
    stats: &mut GeneratedCylindricalFamilySourceSetStats,
    root: &GeneratedCylindricalSectorRootStartCertificate,
    row_span: Option<&GeneratedSymbolicRowSpanCertificate>,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    let row_span = row_span.ok_or(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
        detail: "aggregate root census has no shared row span",
    })?;
    stats.sector_roots = bounded_add(SOURCES_RESOURCE, stats.sector_roots, 1, limits.max_sources)?;
    stats.total_prepare_points = bounded_add(
        TOTAL_PREPARE_POINTS_RESOURCE,
        stats.total_prepare_points,
        root.stats().prepare_points(),
        limits.max_total_prepare_points,
    )?;
    let expanded_rows = checked_mul(
        TOTAL_EXPANDED_ROWS_RESOURCE,
        root.stats().prepare_points(),
        row_span.rows().len(),
    )?;
    stats.total_expanded_rows = bounded_add(
        TOTAL_EXPANDED_ROWS_RESOURCE,
        stats.total_expanded_rows,
        expanded_rows,
        limits.max_total_expanded_rows,
    )?;
    Ok(())
}

fn charge_row_system(
    stats: &mut GeneratedCylindricalFamilySourceSetStats,
    row_system: &GeneratedCylindricalRowSystemCertificate,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    let root = row_system.start().sector_root_start().ok_or(
        GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
            detail: "aggregate row census encountered a non-sector-root source",
        },
    )?;
    let expected_expanded = checked_mul(
        TOTAL_EXPANDED_ROWS_RESOURCE,
        root.stats().prepare_points(),
        root.row_span_arc().rows().len(),
    )?;
    if row_system.stats().expanded_rows() != expected_expanded {
        return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
            detail: "row-system expanded census differs from the precharged rectangle",
        });
    }
    stats.row_systems = bounded_add(SOURCES_RESOURCE, stats.row_systems, 1, limits.max_sources)?;
    stats.total_retained_rows = bounded_add(
        TOTAL_RETAINED_ROWS_RESOURCE,
        stats.total_retained_rows,
        row_system.stats().retained_rows(),
        limits.max_total_retained_rows,
    )?;
    stats.total_persistent_events = bounded_add(
        TOTAL_PERSISTENT_EVENTS_RESOURCE,
        stats.total_persistent_events,
        row_system.stats().retained_rows(),
        limits.max_total_persistent_events,
    )?;
    Ok(())
}

fn charge_persistent(
    stats: &mut GeneratedCylindricalFamilySourceSetStats,
    source: &GeneratedCylindricalPersistentEliminationCertificate,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    if source.events().len() != source.row_system().stats().retained_rows() {
        return Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
            detail: "persistent event census differs from precharged retained rows",
        });
    }
    stats.persistent_sources = bounded_add(
        SOURCES_RESOURCE,
        stats.persistent_sources,
        1,
        limits.max_sources,
    )?;
    stats.total_persistent_pivots = bounded_add(
        TOTAL_PERSISTENT_PIVOTS_RESOURCE,
        stats.total_persistent_pivots,
        source.stats().pivot_rows(),
        limits.max_total_persistent_pivots,
    )?;
    Ok(())
}

fn initial_source_budget(
    stats: GeneratedCylindricalFamilySourceSetStats,
    limits: GeneratedCylindricalFamilySourceSetLimits,
) -> Result<GeneratedCylindricalFamilySourceBudget, GeneratedCylindricalFamilySourceSetError> {
    Ok(GeneratedCylindricalFamilySourceBudget {
        prepare_points_remaining: remaining(
            TOTAL_PREPARE_POINTS_RESOURCE,
            stats.total_prepare_points,
            limits.max_total_prepare_points,
        )?,
        expanded_rows_remaining: remaining(
            TOTAL_EXPANDED_ROWS_RESOURCE,
            stats.total_expanded_rows,
            limits.max_total_expanded_rows,
        )?,
        retained_rows_remaining: remaining(
            TOTAL_RETAINED_ROWS_RESOURCE,
            stats.total_retained_rows,
            limits.max_total_retained_rows,
        )?,
        persistent_events_remaining: remaining(
            TOTAL_PERSISTENT_EVENTS_RESOURCE,
            stats.total_persistent_events,
            limits.max_total_persistent_events,
        )?,
        persistent_pivots_remaining: remaining(
            TOTAL_PERSISTENT_PIVOTS_RESOURCE,
            stats.total_persistent_pivots,
            limits.max_total_persistent_pivots,
        )?,
    })
}

fn projected_root_limits(
    mut limits: GeneratedCylindricalSectorRootStartLimits,
    budget: GeneratedCylindricalFamilySourceBudget,
) -> GeneratedCylindricalSectorRootStartLimits {
    limits.max_prepare_points = limits
        .max_prepare_points
        .min(budget.prepare_points_remaining);
    limits.schedule.max_retained_points = limits
        .schedule
        .max_retained_points
        .min(budget.prepare_points_remaining);
    limits
}

fn projected_row_limits(
    mut limits: GeneratedCylindricalRowSystemLimits,
    budget: GeneratedCylindricalFamilySourceBudget,
) -> GeneratedCylindricalRowSystemLimits {
    limits.max_expanded_rows = limits.max_expanded_rows.min(budget.expanded_rows_remaining);
    limits.max_retained_rows = limits
        .max_retained_rows
        .min(budget.retained_rows_remaining)
        .min(budget.persistent_events_remaining);
    limits
}

fn projected_persistent_limits(
    mut limits: GeneratedCylindricalPersistentEliminationLimits,
    budget: GeneratedCylindricalFamilySourceBudget,
) -> GeneratedCylindricalPersistentEliminationLimits {
    limits.elimination.max_pivots = limits
        .elimination
        .max_pivots
        .min(budget.persistent_pivots_remaining);
    limits
}

#[allow(clippy::too_many_arguments)]
fn map_root_error(
    error: GeneratedCylindricalSectorRootStartError,
    solve_ordinal: usize,
    sector: SectorMask,
    consumed_prepare_points: usize,
    budget: GeneratedCylindricalFamilySourceBudget,
    limits: GeneratedCylindricalFamilySourceSetLimits,
    effective: GeneratedCylindricalSectorRootStartLimits,
) -> GeneratedCylindricalFamilySourceSetError {
    let projected_request = match &error {
        GeneratedCylindricalSectorRootStartError::Schedule(
            CylindricalPreparePointScheduleError::CumulativeResourceLimit {
                resource: "retained prepare points",
                cumulative_requested,
                cumulative_limit,
                ..
            },
        ) if effective.schedule.max_retained_points
            < limits.sector_root.schedule.max_retained_points
            && *cumulative_limit == effective.schedule.max_retained_points
            && effective.schedule.max_retained_points == budget.prepare_points_remaining =>
        {
            Some(*cumulative_requested)
        }
        GeneratedCylindricalSectorRootStartError::ResourceLimit {
            resource: "sector-root prepare points",
            requested,
            limit,
        } if effective.max_prepare_points < limits.sector_root.max_prepare_points
            && *limit == effective.max_prepare_points
            && effective.max_prepare_points == budget.prepare_points_remaining =>
        {
            Some(*requested)
        }
        _ => None,
    };
    if let Some(requested) = projected_request {
        return cumulative_resource_error(
            TOTAL_PREPARE_POINTS_RESOURCE,
            consumed_prepare_points,
            requested,
            limits.max_total_prepare_points,
        );
    }
    GeneratedCylindricalFamilySourceSetError::SectorRoot {
        solve_ordinal,
        sector,
        error,
    }
}

#[allow(clippy::too_many_arguments)]
fn map_row_error(
    error: GeneratedCylindricalRowSystemError,
    solve_ordinal: usize,
    sector: SectorMask,
    stats: GeneratedCylindricalFamilySourceSetStats,
    budget: GeneratedCylindricalFamilySourceBudget,
    limits: GeneratedCylindricalFamilySourceSetLimits,
    effective: GeneratedCylindricalRowSystemLimits,
) -> GeneratedCylindricalFamilySourceSetError {
    if let GeneratedCylindricalRowSystemError::ResourceLimit {
        resource: "retained rows",
        requested,
        limit,
    } = &error
    {
        let family_allowance = budget
            .retained_rows_remaining
            .min(budget.persistent_events_remaining);
        if effective.max_retained_rows < limits.row_system.max_retained_rows
            && *limit == effective.max_retained_rows
            && effective.max_retained_rows == family_allowance
        {
            let (resource, used, outer_limit) =
                if budget.retained_rows_remaining <= budget.persistent_events_remaining {
                    (
                        TOTAL_RETAINED_ROWS_RESOURCE,
                        stats.total_retained_rows,
                        limits.max_total_retained_rows,
                    )
                } else {
                    (
                        TOTAL_PERSISTENT_EVENTS_RESOURCE,
                        stats.total_persistent_events,
                        limits.max_total_persistent_events,
                    )
                };
            return cumulative_resource_error(resource, used, *requested, outer_limit);
        }
    }
    GeneratedCylindricalFamilySourceSetError::RowSystem {
        solve_ordinal,
        sector,
        error,
    }
}

#[allow(clippy::too_many_arguments)]
fn map_persistent_error(
    error: GeneratedCylindricalPersistentEliminationError,
    solve_ordinal: usize,
    sector: SectorMask,
    consumed_pivots: usize,
    budget: GeneratedCylindricalFamilySourceBudget,
    limits: GeneratedCylindricalFamilySourceSetLimits,
    effective: GeneratedCylindricalPersistentEliminationLimits,
) -> GeneratedCylindricalFamilySourceSetError {
    if let GeneratedCylindricalPersistentEliminationError::Elimination(
        ParametricEliminationError::ResourceLimit {
            resource: "parametric pivots",
            requested,
            limit,
        },
    ) = &error
    {
        if effective.elimination.max_pivots < limits.persistent.elimination.max_pivots
            && *limit == effective.elimination.max_pivots
            && effective.elimination.max_pivots == budget.persistent_pivots_remaining
        {
            return cumulative_resource_error(
                TOTAL_PERSISTENT_PIVOTS_RESOURCE,
                consumed_pivots,
                *requested,
                limits.max_total_persistent_pivots,
            );
        }
    }
    GeneratedCylindricalFamilySourceSetError::Persistent {
        solve_ordinal,
        sector,
        error,
    }
}

fn cumulative_resource_error(
    resource: &'static str,
    consumed: usize,
    requested_in_source: usize,
    limit: usize,
) -> GeneratedCylindricalFamilySourceSetError {
    match consumed.checked_add(requested_in_source) {
        Some(requested) => GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        None => GeneratedCylindricalFamilySourceSetError::ResourceCountOverflow { resource },
    }
}

fn remaining(
    resource: &'static str,
    used: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalFamilySourceSetError> {
    limit
        .checked_sub(used)
        .ok_or(GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource,
            requested: used,
            limit,
        })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, GeneratedCylindricalFamilySourceSetError> {
    values.into_iter().try_fold(0usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(GeneratedCylindricalFamilySourceSetError::ResourceCountOverflow { resource })
    })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedCylindricalFamilySourceSetError> {
    left.checked_mul(right)
        .ok_or(GeneratedCylindricalFamilySourceSetError::ResourceCountOverflow { resource })
}

fn source_index_census(
    source_count: usize,
    sector_arity: usize,
) -> Result<(usize, usize), GeneratedCylindricalFamilySourceSetError> {
    let scope_entries = checked_mul(SOURCE_SCOPE_ENTRIES_RESOURCE, source_count, sector_arity)?;
    let bit_payload_bytes = checked_mul(
        SOURCE_INDEX_BYTES_RESOURCE,
        scope_entries,
        size_of::<bool>(),
    )?;
    let sector_headers = checked_mul(
        SOURCE_INDEX_BYTES_RESOURCE,
        source_count,
        size_of::<SectorMask>(),
    )?;
    let source_arc_bytes = checked_mul(
        SOURCE_INDEX_BYTES_RESOURCE,
        source_count,
        size_of::<Arc<GeneratedCylindricalPersistentEliminationCertificate>>(),
    )?;
    let index_bytes = checked_sum(
        SOURCE_INDEX_BYTES_RESOURCE,
        [bit_payload_bytes, sector_headers, source_arc_bytes],
    )?;
    Ok((scope_entries, index_bytes))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    if requested > limit {
        Err(GeneratedCylindricalFamilySourceSetError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    added: usize,
    limit: usize,
) -> Result<usize, GeneratedCylindricalFamilySourceSetError> {
    let requested = current
        .checked_add(added)
        .ok_or(GeneratedCylindricalFamilySourceSetError::ResourceCountOverflow { resource })?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    resource: &'static str,
    target: &mut Vec<T>,
    requested: usize,
) -> Result<(), GeneratedCylindricalFamilySourceSetError> {
    target.try_reserve_exact(requested).map_err(|_| {
        GeneratedCylindricalFamilySourceSetError::AllocationFailure {
            resource,
            requested,
        }
    })
}

#[cfg(test)]
mod replay_tamper_tests {
    use super::*;
    use crate::{AffineDenominator, ParametricIbpGenerator, algebra::CoefficientContext};

    fn certificate() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        GeneratedCylindricalFamilySourceSetCertificate,
    ) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let family = IntegralFamily::new(
            "family-source-set-private-tamper",
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let certificate = GeneratedCylindricalFamilySourceSetCompiler::compile(
            &family,
            &context,
            SectorRestrictions::unrestricted(1).unwrap(),
            PowerShiftPolicy::FormalGeneric,
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            ParametricIbpConfig::default(),
            GeneratedSymbolicRowSpanConfig::default(),
            1,
            GeneratedCylindricalFamilySourceSetLimits::default(),
        )
        .unwrap();
        (family, context, certificate)
    }

    #[test]
    fn replay_rejects_equivalent_but_distinct_shared_arcs_and_order_tampering() {
        let (family, context, certificate) = certificate();

        let mut distinct_inventory = certificate.clone();
        distinct_inventory.inventory = Arc::new((*distinct_inventory.inventory).clone());
        assert!(matches!(
            distinct_inventory.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "source order, empty-root metadata, or exact shared Arcs differ"
            })
        ));

        let mut distinct_row_span = certificate.clone();
        distinct_row_span.row_span = Some(Arc::new(
            (**distinct_row_span.row_span.as_ref().unwrap()).clone(),
        ));
        assert!(matches!(
            distinct_row_span.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "source order, empty-root metadata, or exact shared Arcs differ"
            })
        ));

        let mut wrong_order = certificate.clone();
        wrong_order.solve_order[0] = SectorMask::try_from_bit_string("0").unwrap();
        assert!(matches!(
            wrong_order.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "persistent sources do not preserve the exact inventory solve order"
            })
        ));

        let mut wrong_budget = certificate.clone();
        wrong_budget.source_budgets[0].prepare_points_remaining -= 1;
        assert!(matches!(
            wrong_budget.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "source prepare/expanded budget projection differs"
            })
        ));

        let mut omitted_budget = certificate.clone();
        omitted_budget.source_budgets = Box::new([]);
        assert!(matches!(
            omitted_budget.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "persistent sources do not preserve the exact inventory solve order"
            })
        ));

        let mut omitted_source = certificate;
        omitted_source.persistent_sources = Box::new([]);
        assert!(matches!(
            omitted_source.replay(&family, &context),
            Err(GeneratedCylindricalFamilySourceSetError::ReplayMismatch {
                detail: "persistent sources do not preserve the exact inventory solve order"
            })
        ));
    }
}
