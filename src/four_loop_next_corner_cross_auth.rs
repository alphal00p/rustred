//! Exact semantic cross-authentication between the native four-loop corner
//! shell and the larger fixed-seed exact elimination.
//!
//! This module deliberately does not perform elimination.  It borrows two
//! already constructed, independently replayable certificates, proves that the
//! 160 native corner rows embed exactly into the 1,968-row matrix over `Q(d)`,
//! and compares the old 64-column unresolved set with the exact pivot/free
//! partition of the larger shell.
//!
//! The resulting 48 columns are called `pivoted_nonterminals`, not terminally
//! reduced integrals: recursively expanding their rules may still reach other
//! free coordinates of the larger fixed shell.  This certificate also does not
//! call any free coordinate a master.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    Coefficient, CoefficientContext, CoefficientProjectionError,
    FOUR_LOOP_NEXT_ELIMINATION_COLUMNS, FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS,
    FOUR_LOOP_NEXT_ELIMINATION_RANK, FourLoopCornerColumnId, FourLoopCornerNormalizedRow,
    FourLoopCornerRawRowId, FourLoopCornerShellCertificate, FourLoopCornerShellError,
    FourLoopGenuineCornerType, FourLoopNextClosedRow, FourLoopNextElimination,
    FourLoopNextEliminationError, FourLoopNextSeedPhase, MassiveVacuumMaster, MasterProduct,
    MasterProductError,
};

pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS: usize = 160;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS: usize = 223;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ENTRIES: usize = 1_334;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_RANK: usize = 159;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_INHERITED_COLUMNS: usize = 64;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_NONTERMINALS: usize = 48;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS: usize = 16;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_SCALARS: usize = 10;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_PRODUCTS: usize = 6;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N0: usize = 22;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N1: usize = 26;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_EMBEDDED_ENTRIES: usize = 1_334;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_COEFFICIENT_PROJECTIONS: usize = 2_668;
pub const FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM: u64 = 0xa359_ccf8_3fd1_eb5c;

const MAX_BASE_ROW_WIDTH: usize = FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS;
const MAX_BASE_ENTRIES: usize =
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS * FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS;
const MAX_ROW_EMBEDDING_PROJECTIONS: usize = 2 * MAX_BASE_ENTRIES;

const CROSS_AUTH_SCHEMA: &str = "rustred-equal-mass-euclidean-four-loop-next-corner-cross-auth-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Scope of the composed semantic certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextCornerCrossAuthStatus {
    /// Exact row embedding and inherited-column disposition for the fixed seed
    /// shell only.
    CompleteInheritedCornerDispositionFixedSeedShell,
}

/// Which independently generated row failed exact `Q(d)` projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextCornerCrossAuthRowSide {
    Corner,
    NextShell,
}

impl fmt::Display for FourLoopNextCornerCrossAuthRowSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corner => formatter.write_str("corner"),
            Self::NextShell => formatter.write_str("next-shell"),
        }
    }
}

/// Exact census retained by the cross-authentication layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextCornerCrossAuthStats {
    base_rows: usize,
    base_columns: usize,
    base_entries: usize,
    base_rank: usize,
    inherited_columns: usize,
    embedded_rows: usize,
    embedded_entries: usize,
    coefficient_projections: usize,
    next_columns: usize,
    next_rank: usize,
    next_free_columns: usize,
    pivoted_nonterminals: usize,
    retained_terminals: usize,
    retained_scalars: usize,
    retained_products: usize,
    pivoted_d1_n0: usize,
    pivoted_d1_n1: usize,
}

macro_rules! stat_getters {
    ($($name:ident),* $(,)?) => {
        $(pub const fn $name(self) -> usize { self.$name })*
    };
}

impl FourLoopNextCornerCrossAuthStats {
    stat_getters!(
        base_rows,
        base_columns,
        base_entries,
        base_rank,
        inherited_columns,
        embedded_rows,
        embedded_entries,
        coefficient_projections,
        next_columns,
        next_rank,
        next_free_columns,
        pivoted_nonterminals,
        retained_terminals,
        retained_scalars,
        retained_products,
        pivoted_d1_n0,
        pivoted_d1_n1,
    );
}

/// Borrowed composition of the exact corner and next-shell certificates.
///
/// Construction performs only bounded semantic comparison.  It never mutates
/// either component and never builds another exact elimination.  [`Self::replay`]
/// composes both public component replays before recomputing this layer.
pub struct FourLoopNextCornerCrossAuth<'certificate, 'closed, 'sources, 'transport, 'inventory> {
    corner: &'certificate FourLoopCornerShellCertificate,
    elimination: &'certificate FourLoopNextElimination<'closed, 'sources, 'transport, 'inventory>,
    base_columns: Vec<FourLoopCornerColumnId>,
    inherited_columns: Vec<FourLoopCornerColumnId>,
    pivoted_nonterminals: Vec<FourLoopCornerColumnId>,
    retained_terminals: Vec<FourLoopCornerColumnId>,
    stats: FourLoopNextCornerCrossAuthStats,
    checksum: u64,
}

impl<'certificate, 'closed, 'sources, 'transport, 'inventory>
    FourLoopNextCornerCrossAuth<'certificate, 'closed, 'sources, 'transport, 'inventory>
{
    pub const SCHEMA: &'static str = CROSS_AUTH_SCHEMA;

    /// Compose two already constructed immutable certificates.
    ///
    /// The component constructors establish their respective algebraic proofs;
    /// this method adds the exact row-embedding and typed disposition bridge.
    /// It intentionally does not rerun either expensive component replay.
    pub fn compose(
        corner: &'certificate FourLoopCornerShellCertificate,
        elimination: &'certificate FourLoopNextElimination<
            'closed,
            'sources,
            'transport,
            'inventory,
        >,
    ) -> Result<Self, FourLoopNextCornerCrossAuthError> {
        let derived = authenticate_cross_auth(corner, elimination)?;
        Ok(Self {
            corner,
            elimination,
            base_columns: derived.base_columns,
            inherited_columns: derived.inherited_columns,
            pivoted_nonterminals: derived.pivoted_nonterminals,
            retained_terminals: derived.retained_terminals,
            stats: derived.stats,
            checksum: derived.checksum,
        })
    }

    pub const fn status(&self) -> FourLoopNextCornerCrossAuthStatus {
        FourLoopNextCornerCrossAuthStatus::CompleteInheritedCornerDispositionFixedSeedShell
    }

    pub const fn corner(&self) -> &'certificate FourLoopCornerShellCertificate {
        self.corner
    }

    pub const fn elimination(
        &self,
    ) -> &'certificate FourLoopNextElimination<'closed, 'sources, 'transport, 'inventory> {
        self.elimination
    }

    /// All 223 typed columns occurring in the exact native corner rows.
    pub fn base_columns(&self) -> &[FourLoopCornerColumnId] {
        &self.base_columns
    }

    /// The exact 64-column free complement of the native corner certificate.
    pub fn inherited_columns(&self) -> &[FourLoopCornerColumnId] {
        &self.inherited_columns
    }

    /// The 48 inherited nonterminal coordinates pivoted by the larger shell.
    ///
    /// This name deliberately does not promise that recursively expanded rules
    /// have support only on the sixteen retained inherited terminals.
    pub fn pivoted_nonterminals(&self) -> &[FourLoopCornerColumnId] {
        &self.pivoted_nonterminals
    }

    /// The ten scalar corners and six canonical products which remain free in
    /// both finite shells.  They remain unresolved coordinates, not a claim of
    /// unrestricted master minimality.
    pub fn retained_terminals(&self) -> &[FourLoopCornerColumnId] {
        &self.retained_terminals
    }

    pub const fn stats(&self) -> FourLoopNextCornerCrossAuthStats {
        self.stats
    }

    /// Deterministic regression metadata.  Exact comparison and replay, not
    /// this non-cryptographic digest, establish the proof.
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }

    /// Replay both borrowed proof components and recompute every semantic
    /// comparison retained by this wrapper.
    pub fn replay(&self) -> Result<(), FourLoopNextCornerCrossAuthError> {
        self.corner.replay()?;
        self.elimination.replay()?;
        let replayed = authenticate_cross_auth(self.corner, self.elimination)?;
        if replayed.base_columns != self.base_columns
            || replayed.inherited_columns != self.inherited_columns
            || replayed.pivoted_nonterminals != self.pivoted_nonterminals
            || replayed.retained_terminals != self.retained_terminals
            || replayed.stats != self.stats
            || replayed.checksum != self.checksum
        {
            return Err(FourLoopNextCornerCrossAuthError::ReplayMismatch {
                component: "complete corner/next-shell composition",
            });
        }
        Ok(())
    }
}

impl fmt::Display for FourLoopNextCornerCrossAuth<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} status={:?} base_rows={} base_columns={} base_rank={} inherited={} pivoted_nonterminals={} retained_terminals={} next_rank={} next_free={} checksum=0x{:016x}; fixed-seed disposition only; pivoted rules may depend on other next-shell free coordinates",
            Self::SCHEMA,
            self.status(),
            self.stats.base_rows,
            self.stats.base_columns,
            self.stats.base_rank,
            self.stats.inherited_columns,
            self.stats.pivoted_nonterminals,
            self.stats.retained_terminals,
            self.stats.next_rank,
            self.stats.next_free_columns,
            self.checksum,
        )
    }
}

#[derive(Debug)]
pub enum FourLoopNextCornerCrossAuthError {
    Corner(FourLoopCornerShellError),
    Elimination(FourLoopNextEliminationError),
    Product(MasterProductError),
    CensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    AllocationFailed {
        resource: &'static str,
        requested: usize,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    InvalidPartition {
        component: &'static str,
    },
    MissingBaseColumn {
        column: FourLoopCornerColumnId,
    },
    InvalidCornerSeed {
        raw_id: crate::FourLoopNextRawRowId,
        reason: &'static str,
    },
    DuplicateCornerRow {
        raw_id: FourLoopCornerRawRowId,
        side: FourLoopNextCornerCrossAuthRowSide,
    },
    MissingCornerRow {
        raw_id: FourLoopCornerRawRowId,
        side: FourLoopNextCornerCrossAuthRowSide,
    },
    CornerRowMismatch {
        raw_id: FourLoopCornerRawRowId,
        component: &'static str,
    },
    CoefficientProjection {
        side: FourLoopNextCornerCrossAuthRowSide,
        raw_id: FourLoopCornerRawRowId,
        column: FourLoopCornerColumnId,
        source: CoefficientProjectionError,
    },
    ZeroProjectedCoefficient {
        side: FourLoopNextCornerCrossAuthRowSide,
        raw_id: FourLoopCornerRawRowId,
        column: FourLoopCornerColumnId,
    },
    ExpectedTerminalMissing {
        column: FourLoopCornerColumnId,
    },
    UnexpectedPivotedGrade {
        column: FourLoopCornerColumnId,
        dots: u32,
        numerators: u32,
    },
    DispositionMismatch {
        component: &'static str,
    },
    ChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    ReplayMismatch {
        component: &'static str,
    },
}

impl fmt::Display for FourLoopNextCornerCrossAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Corner(error) => write!(formatter, "corner certificate failed: {error}"),
            Self::Elimination(error) => {
                write!(formatter, "next-shell elimination failed: {error}")
            }
            Self::Product(error) => write!(formatter, "canonical terminal product failed: {error}"),
            Self::CensusMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "corner cross-authentication {resource} mismatch: expected {expected}, found {actual}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "corner cross-authentication {resource} requested {requested}, limit is {limit}"
            ),
            Self::AllocationFailed {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for corner cross-authentication {resource}"
            ),
            Self::ArithmeticOverflow { resource } => write!(
                formatter,
                "arithmetic overflow while counting corner cross-authentication {resource}"
            ),
            Self::InvalidPartition { component } => {
                write!(
                    formatter,
                    "invalid exact pivot/free partition in {component}"
                )
            }
            Self::MissingBaseColumn { column } => write!(
                formatter,
                "native corner column {} is absent from the next-shell catalog",
                column.stable_key()
            ),
            Self::InvalidCornerSeed { raw_id, reason } => write!(
                formatter,
                "next-shell corner origin {} is invalid: {reason}",
                raw_id.stable_key()
            ),
            Self::DuplicateCornerRow { raw_id, side } => write!(
                formatter,
                "duplicate {side} row for {}",
                raw_id.stable_key()
            ),
            Self::MissingCornerRow { raw_id, side } => {
                write!(formatter, "missing {side} row for {}", raw_id.stable_key())
            }
            Self::CornerRowMismatch { raw_id, component } => write!(
                formatter,
                "embedded row {} differs in {component}",
                raw_id.stable_key()
            ),
            Self::CoefficientProjection {
                side,
                raw_id,
                column,
                source,
            } => write!(
                formatter,
                "could not project {side} row {}, column {} into Q(d): {source}",
                raw_id.stable_key(),
                column.stable_key()
            ),
            Self::ZeroProjectedCoefficient {
                side,
                raw_id,
                column,
            } => write!(
                formatter,
                "{side} row {}, column {} projected to zero",
                raw_id.stable_key(),
                column.stable_key()
            ),
            Self::ExpectedTerminalMissing { column } => write!(
                formatter,
                "expected retained inherited terminal {} is absent from the native corner-free set",
                column.stable_key()
            ),
            Self::UnexpectedPivotedGrade {
                column,
                dots,
                numerators,
            } => write!(
                formatter,
                "inherited pivoted column {} has unexpected grading D{dots}/N{numerators}",
                column.stable_key()
            ),
            Self::DispositionMismatch { component } => write!(
                formatter,
                "exact inherited-column disposition mismatch in {component}"
            ),
            Self::ChecksumMismatch { expected, actual } => write!(
                formatter,
                "corner cross-authentication checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"
            ),
            Self::ReplayMismatch { component } => write!(
                formatter,
                "corner cross-authentication replay mismatch in {component}"
            ),
        }
    }
}

impl Error for FourLoopNextCornerCrossAuthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Corner(error) => Some(error),
            Self::Elimination(error) => Some(error),
            Self::Product(error) => Some(error),
            Self::CoefficientProjection { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<FourLoopCornerShellError> for FourLoopNextCornerCrossAuthError {
    fn from(error: FourLoopCornerShellError) -> Self {
        Self::Corner(error)
    }
}

impl From<FourLoopNextEliminationError> for FourLoopNextCornerCrossAuthError {
    fn from(error: FourLoopNextEliminationError) -> Self {
        Self::Elimination(error)
    }
}

impl From<MasterProductError> for FourLoopNextCornerCrossAuthError {
    fn from(error: MasterProductError) -> Self {
        Self::Product(error)
    }
}

struct DerivedCrossAuth {
    base_columns: Vec<FourLoopCornerColumnId>,
    inherited_columns: Vec<FourLoopCornerColumnId>,
    pivoted_nonterminals: Vec<FourLoopCornerColumnId>,
    retained_terminals: Vec<FourLoopCornerColumnId>,
    stats: FourLoopNextCornerCrossAuthStats,
    checksum: u64,
}

fn authenticate_cross_auth(
    corner: &FourLoopCornerShellCertificate,
    elimination: &FourLoopNextElimination<'_, '_, '_, '_>,
) -> Result<DerivedCrossAuth, FourLoopNextCornerCrossAuthError> {
    if !corner.is_complete() {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "incomplete native corner certificate",
        });
    }
    check_census(
        "base rows",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
        corner.normalized_rows().len(),
    )?;
    check_census(
        "base raw-row IDs",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
        corner.raw_row_ids().len(),
    )?;
    check_census(
        "base rank",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_RANK,
        corner.rank(),
    )?;

    let (base_column_set, base_entries) = base_catalog(corner)?;
    check_census(
        "base columns",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS,
        base_column_set.len(),
    )?;
    check_census(
        "base entries",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ENTRIES,
        base_entries,
    )?;

    let corner_pivots = corner
        .pivots()
        .iter()
        .map(|rule| rule.pivot().clone())
        .collect::<BTreeSet<_>>();
    if corner_pivots.len() != corner.rank() || !corner_pivots.is_subset(&base_column_set) {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "native corner pivot set",
        });
    }
    let inherited_set = corner
        .free_unresolved_columns()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    check_census(
        "inherited corner-free columns",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_INHERITED_COLUMNS,
        inherited_set.len(),
    )?;
    if corner
        .free_unresolved_columns()
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || !inherited_set.is_subset(&base_column_set)
        || !corner_pivots.is_disjoint(&inherited_set)
        || corner_pivots.len().checked_add(inherited_set.len()) != Some(base_column_set.len())
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "native corner pivot/free complement",
        });
    }

    check_resource(
        "next-shell typed columns",
        elimination.columns().len(),
        FOUR_LOOP_NEXT_ELIMINATION_COLUMNS,
    )?;
    check_resource(
        "next-shell exact pivots",
        elimination.pivots().len(),
        FOUR_LOOP_NEXT_ELIMINATION_RANK,
    )?;
    check_resource(
        "next-shell exact free columns",
        elimination.free_unresolved_columns().len(),
        FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS,
    )?;
    check_census(
        "next-shell typed columns",
        FOUR_LOOP_NEXT_ELIMINATION_COLUMNS,
        elimination.columns().len(),
    )?;
    check_census(
        "next-shell exact rank",
        FOUR_LOOP_NEXT_ELIMINATION_RANK,
        elimination.rank(),
    )?;
    check_census(
        "next-shell exact pivot rules",
        FOUR_LOOP_NEXT_ELIMINATION_RANK,
        elimination.pivots().len(),
    )?;
    check_census(
        "next-shell exact free columns",
        FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS,
        elimination.free_unresolved_columns().len(),
    )?;
    if !elimination
        .columns()
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "next-shell typed column order",
        });
    }
    if !elimination
        .pivots()
        .windows(2)
        .all(|pair| pair[0].pivot() > pair[1].pivot())
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "next-shell exact pivot order",
        });
    }
    if !elimination
        .free_unresolved_columns()
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "next-shell exact free-column order",
        });
    }
    let next_columns = elimination
        .columns()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if next_columns.len() != elimination.columns().len() {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "next-shell typed column catalog",
        });
    }
    for column in &base_column_set {
        if !next_columns.contains(column) {
            return Err(FourLoopNextCornerCrossAuthError::MissingBaseColumn {
                column: column.clone(),
            });
        }
    }

    let next_pivots = elimination
        .pivots()
        .iter()
        .map(|rule| rule.pivot().clone())
        .collect::<BTreeSet<_>>();
    let next_free = elimination
        .free_unresolved_columns()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if next_pivots.len() != elimination.rank()
        || next_free.len() != elimination.free_unresolved_columns().len()
        || !next_pivots.is_disjoint(&next_free)
        || !next_pivots.is_subset(&next_columns)
        || !next_free.is_subset(&next_columns)
        || next_pivots.len().checked_add(next_free.len()) != Some(next_columns.len())
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "next-shell exact pivot/free complement",
        });
    }

    let retained_set = expected_retained_terminals()?;
    check_census(
        "retained inherited terminals",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS,
        retained_set.len(),
    )?;
    for terminal in &retained_set {
        if !inherited_set.contains(terminal) {
            return Err(FourLoopNextCornerCrossAuthError::ExpectedTerminalMissing {
                column: terminal.clone(),
            });
        }
    }
    let retained_scalars = retained_set
        .iter()
        .filter(|column| matches!(column, FourLoopCornerColumnId::Genuine { .. }))
        .count();
    let retained_products = retained_set.len() - retained_scalars;
    check_census(
        "retained scalar corners",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_SCALARS,
        retained_scalars,
    )?;
    check_census(
        "retained canonical products",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_PRODUCTS,
        retained_products,
    )?;

    let pivoted_set = inherited_set
        .difference(&retained_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    check_census(
        "pivoted inherited nonterminals",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_NONTERMINALS,
        pivoted_set.len(),
    )?;
    let (pivoted_d1_n0, pivoted_d1_n1) = authenticate_pivoted_grading(&pivoted_set)?;
    check_census(
        "pivoted inherited D1/N0 columns",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N0,
        pivoted_d1_n0,
    )?;
    check_census(
        "pivoted inherited D1/N1 columns",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N1,
        pivoted_d1_n1,
    )?;

    let actual_retained = inherited_set
        .intersection(&next_free)
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_pivoted = inherited_set
        .intersection(&next_pivots)
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_retained != retained_set {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "sixteen retained inherited terminals",
        });
    }
    if actual_pivoted != pivoted_set {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "forty-eight pivoted inherited nonterminals",
        });
    }
    if pivoted_set
        .iter()
        .any(|column| elimination.pivot_rule(column).is_none())
        || retained_set
            .iter()
            .any(|column| elimination.pivot_rule(column).is_some())
    {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "typed exact pivot-rule lookup",
        });
    }

    if !corner_pivots.is_subset(&next_pivots) {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "native corner pivots retained as next-shell pivots",
        });
    }
    let expected_base_pivots = corner_pivots
        .union(&pivoted_set)
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_base_pivots = base_column_set
        .intersection(&next_pivots)
        .cloned()
        .collect::<BTreeSet<_>>();
    check_census(
        "next-shell pivots in the native base catalog",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_RANK
            + FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_NONTERMINALS,
        actual_base_pivots.len(),
    )?;
    if actual_base_pivots != expected_base_pivots {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "207 native-base next-shell pivots",
        });
    }
    let actual_base_free = base_column_set
        .intersection(&next_free)
        .cloned()
        .collect::<BTreeSet<_>>();
    check_census(
        "next-shell free columns in the native base catalog",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS,
        actual_base_free.len(),
    )?;
    if actual_base_free != retained_set {
        return Err(FourLoopNextCornerCrossAuthError::DispositionMismatch {
            component: "sixteen native-base next-shell free columns",
        });
    }

    let embedding = authenticate_row_embedding(corner, elimination, &base_column_set)?;
    check_census(
        "embedded entries",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_EMBEDDED_ENTRIES,
        embedding.entries,
    )?;
    check_census(
        "base/embedded entry equality",
        base_entries,
        embedding.entries,
    )?;
    let expected_projections = base_entries.checked_mul(2).ok_or(
        FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
            resource: "twice the base-entry census",
        },
    )?;
    check_census(
        "coefficient projections",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_COEFFICIENT_PROJECTIONS,
        embedding.projections,
    )?;
    check_census(
        "two projections per base entry",
        expected_projections,
        embedding.projections,
    )?;
    let base_columns = set_to_vec(&base_column_set, "base columns")?;
    let inherited_columns = set_to_vec(&inherited_set, "inherited columns")?;
    let pivoted_nonterminals = set_to_vec(&pivoted_set, "pivoted inherited nonterminals")?;
    let retained_terminals = set_to_vec(&retained_set, "retained inherited terminals")?;
    let stats = FourLoopNextCornerCrossAuthStats {
        base_rows: corner.normalized_rows().len(),
        base_columns: base_columns.len(),
        base_entries,
        base_rank: corner.rank(),
        inherited_columns: inherited_columns.len(),
        embedded_rows: embedding.rows,
        embedded_entries: embedding.entries,
        coefficient_projections: embedding.projections,
        next_columns: elimination.columns().len(),
        next_rank: elimination.rank(),
        next_free_columns: elimination.free_unresolved_columns().len(),
        pivoted_nonterminals: pivoted_nonterminals.len(),
        retained_terminals: retained_terminals.len(),
        retained_scalars,
        retained_products,
        pivoted_d1_n0,
        pivoted_d1_n1,
    };
    let checksum = cross_auth_checksum(
        corner,
        elimination,
        &base_columns,
        &inherited_columns,
        &pivoted_nonterminals,
        &retained_terminals,
        stats,
    );
    if checksum != FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM {
        return Err(FourLoopNextCornerCrossAuthError::ChecksumMismatch {
            expected: FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM,
            actual: checksum,
        });
    }

    Ok(DerivedCrossAuth {
        base_columns,
        inherited_columns,
        pivoted_nonterminals,
        retained_terminals,
        stats,
        checksum,
    })
}

fn base_catalog(
    corner: &FourLoopCornerShellCertificate,
) -> Result<(BTreeSet<FourLoopCornerColumnId>, usize), FourLoopNextCornerCrossAuthError> {
    let mut columns = BTreeSet::new();
    let mut entries = 0_usize;
    for row in corner.normalized_rows() {
        check_resource("base row width", row.entries().len(), MAX_BASE_ROW_WIDTH)?;
        entries = checked_add(entries, row.entries().len(), "base row entries")?;
        check_resource("base row entries", entries, MAX_BASE_ENTRIES)?;
        columns.extend(row.entries().keys().cloned());
    }
    Ok((columns, entries))
}

fn expected_retained_terminals()
-> Result<BTreeSet<FourLoopCornerColumnId>, FourLoopNextCornerCrossAuthError> {
    use MassiveVacuumMaster::{B4, F5, M6, S2, T1};

    let mut terminals = BTreeSet::new();
    for corner_type in FourLoopGenuineCornerType::ALL {
        terminals.insert(FourLoopCornerColumnId::Genuine {
            corner_type,
            powers: scalar_corner_powers(corner_type),
        });
    }
    let products = [
        MasterProduct::try_from_multiplicities([(T1, 4)])?,
        MasterProduct::try_from_multiplicities([(T1, 2), (S2, 1)])?,
        MasterProduct::try_from_multiplicities([(S2, 2)])?,
        MasterProduct::try_from_multiplicities([(T1, 1), (B4, 1)])?,
        MasterProduct::try_from_multiplicities([(T1, 1), (F5, 1)])?,
        MasterProduct::try_from_multiplicities([(T1, 1), (M6, 1)])?,
    ];
    terminals.extend(products.into_iter().map(FourLoopCornerColumnId::Product));
    Ok(terminals)
}

fn scalar_corner_powers(corner_type: FourLoopGenuineCornerType) -> [i32; 10] {
    std::array::from_fn(|position| {
        i32::from(corner_type.reference_mask() & (1_u16 << position) != 0)
    })
}

fn authenticate_pivoted_grading(
    pivoted: &BTreeSet<FourLoopCornerColumnId>,
) -> Result<(usize, usize), FourLoopNextCornerCrossAuthError> {
    let mut d1_n0 = 0_usize;
    let mut d1_n1 = 0_usize;
    for column in pivoted {
        let (dots, numerators) = column_grading(column)?;
        match (dots, numerators) {
            (1, 0) => d1_n0 = checked_add(d1_n0, 1, "D1/N0 inherited columns")?,
            (1, 1) => d1_n1 = checked_add(d1_n1, 1, "D1/N1 inherited columns")?,
            _ => {
                return Err(FourLoopNextCornerCrossAuthError::UnexpectedPivotedGrade {
                    column: column.clone(),
                    dots,
                    numerators,
                });
            }
        }
    }
    Ok((d1_n0, d1_n1))
}

fn column_grading(
    column: &FourLoopCornerColumnId,
) -> Result<(u32, u32), FourLoopNextCornerCrossAuthError> {
    let FourLoopCornerColumnId::Genuine {
        corner_type,
        powers,
    } = column
    else {
        return Err(FourLoopNextCornerCrossAuthError::UnexpectedPivotedGrade {
            column: column.clone(),
            dots: 0,
            numerators: 0,
        });
    };
    let mask = corner_type.reference_mask();
    let mut dots = 0_u32;
    let mut numerators = 0_u32;
    for (position, &power) in powers.iter().enumerate() {
        if mask & (1_u16 << position) != 0 {
            let degree = u32::try_from(power.saturating_sub(1).max(0)).map_err(|_| {
                FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
                    resource: "dot grading",
                }
            })?;
            dots = dots.checked_add(degree).ok_or(
                FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
                    resource: "dot grading",
                },
            )?;
        } else {
            let degree = u32::try_from(power.saturating_neg().max(0)).map_err(|_| {
                FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
                    resource: "numerator grading",
                }
            })?;
            numerators = numerators.checked_add(degree).ok_or(
                FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
                    resource: "numerator grading",
                },
            )?;
        }
    }
    Ok((dots, numerators))
}

#[derive(Clone, Copy)]
struct EmbeddingStats {
    rows: usize,
    entries: usize,
    projections: usize,
}

fn authenticate_row_embedding(
    corner: &FourLoopCornerShellCertificate,
    elimination: &FourLoopNextElimination<'_, '_, '_, '_>,
    base_columns: &BTreeSet<FourLoopCornerColumnId>,
) -> Result<EmbeddingStats, FourLoopNextCornerCrossAuthError> {
    let mut corner_rows = BTreeMap::<FourLoopCornerRawRowId, &FourLoopCornerNormalizedRow>::new();
    for row in corner.normalized_rows() {
        if corner_rows.insert(row.raw_id(), row).is_some() {
            return Err(FourLoopNextCornerCrossAuthError::DuplicateCornerRow {
                raw_id: row.raw_id(),
                side: FourLoopNextCornerCrossAuthRowSide::Corner,
            });
        }
    }
    check_census(
        "unique base rows",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
        corner_rows.len(),
    )?;
    let raw_id_set = corner
        .raw_row_ids()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if raw_id_set.len() != FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS
        || raw_id_set != corner_rows.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "native corner raw-row IDs",
        });
    }

    let mut next_rows = BTreeMap::<FourLoopCornerRawRowId, &FourLoopNextClosedRow>::new();
    for row in elimination.closed_rows().rows() {
        let raw_id = row.raw_id();
        let seed = raw_id.seed();
        if seed.phase() != FourLoopNextSeedPhase::Corner {
            continue;
        }
        if seed.powers() != &scalar_corner_powers(seed.corner_type()) {
            return Err(FourLoopNextCornerCrossAuthError::InvalidCornerSeed {
                raw_id,
                reason: "powers are not the scalar reference corner",
            });
        }
        let expected_phase_index = FourLoopGenuineCornerType::ALL
            .iter()
            .position(|candidate| *candidate == seed.corner_type())
            .ok_or(FourLoopNextCornerCrossAuthError::InvalidCornerSeed {
                raw_id,
                reason: "corner type is outside the frozen ten-type list",
            })?;
        if usize::from(seed.phase_index()) != expected_phase_index {
            return Err(FourLoopNextCornerCrossAuthError::InvalidCornerSeed {
                raw_id,
                reason: "phase index does not match the frozen corner-type order",
            });
        }
        let corner_id = FourLoopCornerRawRowId::new(
            seed.corner_type(),
            raw_id.differentiated_loop(),
            raw_id.contraction_loop(),
        );
        if next_rows.insert(corner_id, row).is_some() {
            return Err(FourLoopNextCornerCrossAuthError::DuplicateCornerRow {
                raw_id: corner_id,
                side: FourLoopNextCornerCrossAuthRowSide::NextShell,
            });
        }
    }
    check_census(
        "embedded next-shell corner rows",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
        next_rows.len(),
    )?;

    let source_context = elimination.closed_rows().coefficient_context();
    let target_context = elimination.coefficient_context();
    if source_context.parameter_names() != ["d", "m2"] || target_context.parameter_names() != ["d"]
    {
        return Err(FourLoopNextCornerCrossAuthError::InvalidPartition {
            component: "Q(d,m2) to Q(d) coefficient contexts",
        });
    }

    let mut embedded_rows = 0_usize;
    let mut embedded_entries = 0_usize;
    let mut projections = 0_usize;
    for (raw_id, corner_row) in &corner_rows {
        let next_row = next_rows.get(raw_id).copied().ok_or(
            FourLoopNextCornerCrossAuthError::MissingCornerRow {
                raw_id: *raw_id,
                side: FourLoopNextCornerCrossAuthRowSide::NextShell,
            },
        )?;
        if corner_row.seed_mass_weight() != next_row.seed_mass_weight() {
            return Err(FourLoopNextCornerCrossAuthError::CornerRowMismatch {
                raw_id: *raw_id,
                component: "seed mass weight",
            });
        }
        if corner_row
            .entries()
            .keys()
            .any(|column| !base_columns.contains(column))
        {
            return Err(FourLoopNextCornerCrossAuthError::CornerRowMismatch {
                raw_id: *raw_id,
                component: "native base-column support",
            });
        }
        let projected_corner = project_row(
            corner_row.entries(),
            source_context,
            target_context,
            FourLoopNextCornerCrossAuthRowSide::Corner,
            *raw_id,
            &mut projections,
        )?;
        let projected_next = project_row(
            next_row.entries(),
            source_context,
            target_context,
            FourLoopNextCornerCrossAuthRowSide::NextShell,
            *raw_id,
            &mut projections,
        )?;
        if projected_corner != projected_next {
            return Err(FourLoopNextCornerCrossAuthError::CornerRowMismatch {
                raw_id: *raw_id,
                component: "canonical normalized Q(d) row",
            });
        }
        embedded_rows = checked_add(embedded_rows, 1, "embedded rows")?;
        embedded_entries = checked_add(
            embedded_entries,
            projected_corner.len(),
            "embedded row entries",
        )?;
        check_resource("embedded row entries", embedded_entries, MAX_BASE_ENTRIES)?;
    }
    for raw_id in next_rows.keys() {
        if !corner_rows.contains_key(raw_id) {
            return Err(FourLoopNextCornerCrossAuthError::MissingCornerRow {
                raw_id: *raw_id,
                side: FourLoopNextCornerCrossAuthRowSide::Corner,
            });
        }
    }
    check_census(
        "exactly embedded rows",
        FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
        embedded_rows,
    )?;

    Ok(EmbeddingStats {
        rows: embedded_rows,
        entries: embedded_entries,
        projections,
    })
}

fn project_row(
    entries: &BTreeMap<FourLoopCornerColumnId, Coefficient>,
    source_context: &CoefficientContext,
    target_context: &CoefficientContext,
    side: FourLoopNextCornerCrossAuthRowSide,
    raw_id: FourLoopCornerRawRowId,
    projections: &mut usize,
) -> Result<BTreeMap<FourLoopCornerColumnId, Coefficient>, FourLoopNextCornerCrossAuthError> {
    check_resource("embedded row width", entries.len(), MAX_BASE_ROW_WIDTH)?;
    let mut projected = BTreeMap::new();
    for (column, coefficient) in entries {
        *projections = checked_add(*projections, 1, "row-embedding coefficient projections")?;
        check_resource(
            "row-embedding coefficient projections",
            *projections,
            MAX_ROW_EMBEDDING_PROJECTIONS,
        )?;
        let coefficient = source_context
            .project_parameter_free(coefficient, "m2", target_context)
            .map_err(
                |source| FourLoopNextCornerCrossAuthError::CoefficientProjection {
                    side,
                    raw_id,
                    column: column.clone(),
                    source,
                },
            )?;
        if coefficient.is_zero() {
            return Err(FourLoopNextCornerCrossAuthError::ZeroProjectedCoefficient {
                side,
                raw_id,
                column: column.clone(),
            });
        }
        if projected.insert(column.clone(), coefficient).is_some() {
            return Err(FourLoopNextCornerCrossAuthError::CornerRowMismatch {
                raw_id,
                component: "duplicate projected typed column",
            });
        }
    }
    Ok(projected)
}

fn set_to_vec(
    set: &BTreeSet<FourLoopCornerColumnId>,
    resource: &'static str,
) -> Result<Vec<FourLoopCornerColumnId>, FourLoopNextCornerCrossAuthError> {
    let mut output = Vec::new();
    output.try_reserve_exact(set.len()).map_err(|_| {
        FourLoopNextCornerCrossAuthError::AllocationFailed {
            resource,
            requested: set.len(),
        }
    })?;
    output.extend(set.iter().cloned());
    Ok(output)
}

fn cross_auth_checksum(
    corner: &FourLoopCornerShellCertificate,
    elimination: &FourLoopNextElimination<'_, '_, '_, '_>,
    base_columns: &[FourLoopCornerColumnId],
    inherited_columns: &[FourLoopCornerColumnId],
    pivoted_nonterminals: &[FourLoopCornerColumnId],
    retained_terminals: &[FourLoopCornerColumnId],
    stats: FourLoopNextCornerCrossAuthStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_tag(&mut hash, CROSS_AUTH_SCHEMA.as_bytes());
    hash_tag(&mut hash, FourLoopCornerShellCertificate::SCHEMA.as_bytes());
    hash_tag(&mut hash, FourLoopNextElimination::SCHEMA.as_bytes());
    hash_u64(&mut hash, elimination.checksum());
    hash_stats(&mut hash, stats);
    hash_columns(&mut hash, base_columns);
    hash_columns(&mut hash, inherited_columns);
    hash_columns(&mut hash, pivoted_nonterminals);
    hash_columns(&mut hash, retained_terminals);
    hash_usize(&mut hash, corner.raw_row_ids().len());
    for raw_id in corner.raw_row_ids() {
        hash_tag(&mut hash, raw_id.stable_key().as_bytes());
    }
    hash
}

fn hash_stats(hash: &mut u64, stats: FourLoopNextCornerCrossAuthStats) {
    for value in [
        stats.base_rows,
        stats.base_columns,
        stats.base_entries,
        stats.base_rank,
        stats.inherited_columns,
        stats.embedded_rows,
        stats.embedded_entries,
        stats.coefficient_projections,
        stats.next_columns,
        stats.next_rank,
        stats.next_free_columns,
        stats.pivoted_nonterminals,
        stats.retained_terminals,
        stats.retained_scalars,
        stats.retained_products,
        stats.pivoted_d1_n0,
        stats.pivoted_d1_n1,
    ] {
        hash_usize(hash, value);
    }
}

fn hash_columns(hash: &mut u64, columns: &[FourLoopCornerColumnId]) {
    hash_usize(hash, columns.len());
    for column in columns {
        hash_tag(hash, column.stable_key().as_bytes());
    }
}

fn hash_tag(hash: &mut u64, bytes: &[u8]) {
    hash_usize(hash, bytes.len());
    hash_bytes(hash, bytes);
}

fn hash_usize(hash: &mut u64, value: usize) {
    hash_u64(hash, value as u64);
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn check_census(
    resource: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), FourLoopNextCornerCrossAuthError> {
    if actual == expected {
        Ok(())
    } else {
        Err(FourLoopNextCornerCrossAuthError::CensusMismatch {
            resource,
            expected,
            actual,
        })
    }
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FourLoopNextCornerCrossAuthError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(FourLoopNextCornerCrossAuthError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopNextCornerCrossAuthError> {
    left.checked_add(right)
        .ok_or(FourLoopNextCornerCrossAuthError::ArithmeticOverflow { resource })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_column(corner_type: FourLoopGenuineCornerType) -> FourLoopCornerColumnId {
        FourLoopCornerColumnId::Genuine {
            corner_type,
            powers: scalar_corner_powers(corner_type),
        }
    }

    #[test]
    fn expected_terminals_are_exactly_ten_scalars_and_six_products() {
        use MassiveVacuumMaster::{B4, F5, M6, S2, T1};

        let terminals = expected_retained_terminals().unwrap();
        assert_eq!(
            terminals.len(),
            FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS
        );
        for corner_type in FourLoopGenuineCornerType::ALL {
            assert!(terminals.contains(&scalar_column(corner_type)));
        }
        let expected_products = [
            MasterProduct::try_from_multiplicities([(T1, 4)]).unwrap(),
            MasterProduct::try_from_multiplicities([(T1, 2), (S2, 1)]).unwrap(),
            MasterProduct::try_from_multiplicities([(S2, 2)]).unwrap(),
            MasterProduct::try_from_multiplicities([(T1, 1), (B4, 1)]).unwrap(),
            MasterProduct::try_from_multiplicities([(T1, 1), (F5, 1)]).unwrap(),
            MasterProduct::try_from_multiplicities([(T1, 1), (M6, 1)]).unwrap(),
        ];
        for product in expected_products {
            assert!(terminals.contains(&FourLoopCornerColumnId::Product(product)));
        }
        assert_eq!(
            terminals
                .iter()
                .filter(|column| matches!(column, FourLoopCornerColumnId::Genuine { .. }))
                .count(),
            FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_SCALARS
        );
        assert_eq!(
            terminals
                .iter()
                .filter(|column| matches!(column, FourLoopCornerColumnId::Product(_)))
                .count(),
            FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_PRODUCTS
        );
    }

    #[test]
    fn scalar_powers_and_nonterminal_grades_are_structural() {
        for corner_type in FourLoopGenuineCornerType::ALL {
            let powers = scalar_corner_powers(corner_type);
            assert!(powers.iter().enumerate().all(|(position, &power)| {
                power == i32::from(corner_type.reference_mask() & (1_u16 << position) != 0)
            }));
            assert_eq!(column_grading(&scalar_column(corner_type)).unwrap(), (0, 0));
        }

        let corner_type = FourLoopGenuineCornerType::FiveLine;
        let mask = corner_type.reference_mask();
        let active = (0..10)
            .find(|position| mask & (1_u16 << position) != 0)
            .unwrap();
        let inactive = (0..10)
            .find(|position| mask & (1_u16 << position) == 0)
            .unwrap();
        let mut dotted = scalar_corner_powers(corner_type);
        dotted[active] = 2;
        let dotted_column = FourLoopCornerColumnId::Genuine {
            corner_type,
            powers: dotted,
        };
        assert_eq!(column_grading(&dotted_column).unwrap(), (1, 0));

        let mut mixed = dotted;
        mixed[inactive] = -1;
        let mixed_column = FourLoopCornerColumnId::Genuine {
            corner_type,
            powers: mixed,
        };
        assert_eq!(column_grading(&mixed_column).unwrap(), (1, 1));
        assert_eq!(
            authenticate_pivoted_grading(&BTreeSet::from([dotted_column, mixed_column,])).unwrap(),
            (1, 1)
        );
    }

    #[test]
    fn product_and_unexpected_genuine_grades_are_rejected() {
        let product = FourLoopCornerColumnId::Product(MasterProduct::identity());
        assert!(matches!(
            column_grading(&product),
            Err(FourLoopNextCornerCrossAuthError::UnexpectedPivotedGrade {
                column,
                dots: 0,
                numerators: 0,
            }) if column == product
        ));

        let corner_type = FourLoopGenuineCornerType::FiveLine;
        let mut powers = scalar_corner_powers(corner_type);
        let active = (0..10)
            .find(|position| corner_type.reference_mask() & (1_u16 << position) != 0)
            .unwrap();
        powers[active] = 3;
        let d2 = FourLoopCornerColumnId::Genuine {
            corner_type,
            powers,
        };
        assert!(matches!(
            authenticate_pivoted_grading(&BTreeSet::from([d2.clone()])),
            Err(FourLoopNextCornerCrossAuthError::UnexpectedPivotedGrade {
                column,
                dots: 2,
                numerators: 0,
            }) if column == d2
        ));
    }

    #[test]
    fn retained_set_projection_preserves_typed_order() {
        let set = BTreeSet::from([
            scalar_column(FourLoopGenuineCornerType::XNineLine),
            scalar_column(FourLoopGenuineCornerType::FiveLine),
            FourLoopCornerColumnId::Product(MasterProduct::identity()),
        ]);
        let ordered = set_to_vec(&set, "test ordered set").unwrap();
        assert_eq!(ordered, set.iter().cloned().collect::<Vec<_>>());
        assert!(ordered.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn census_resource_and_overflow_helpers_fail_typed() {
        assert!(check_census("test census", 3, 3).is_ok());
        assert!(matches!(
            check_census("test census", 3, 2),
            Err(FourLoopNextCornerCrossAuthError::CensusMismatch {
                resource: "test census",
                expected: 3,
                actual: 2,
            })
        ));
        assert!(check_resource("test resource", 3, 3).is_ok());
        assert!(matches!(
            check_resource("test resource", 4, 3),
            Err(FourLoopNextCornerCrossAuthError::ResourceLimit {
                resource: "test resource",
                requested: 4,
                limit: 3,
            })
        ));
        assert!(matches!(
            checked_add(usize::MAX, 1, "test overflow"),
            Err(FourLoopNextCornerCrossAuthError::ArithmeticOverflow {
                resource: "test overflow",
            })
        ));
    }
}
