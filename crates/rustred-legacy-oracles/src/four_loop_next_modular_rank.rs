//! Deterministic finite-field discovery for the four-loop next-shell matrix.
//!
//! This module evaluates the exact, canonical parent rows at explicit images
//! of `Q(d)` and performs sparse modular elimination.  Its output is discovery
//! evidence only: neither a modular rank nor agreement across several images
//! is promoted to an exact rank statement.  Exact production claims still
//! require reconstruction and replay over `Q(d)`.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    FOUR_LOOP_NEXT_CLOSED_ROWS, FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES, FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
    FourLoopNextClosedRows,
};
use rustred::Coefficient;

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Explicit finite-field images used by the default discovery run.
///
/// Each prime is independently validated before use.  Every retained exact
/// denominator must also be nonzero at its image; a zero denominator rejects
/// the run instead of being silently skipped.
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES: [FourLoopNextModularImage; 3] = [
    FourLoopNextModularImage::new(1_000_003, 101),
    FourLoopNextModularImage::new(1_000_033, 211),
    FourLoopNextModularImage::new(1_000_037, 307),
];

/// Frozen finite-field regression evidence for the landed closed matrix.
///
/// These constants authenticate the discovery implementation and its default
/// images. They are not an exact `Q(d)` rank or master-count claim.
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_RANK: usize = 1_588;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FREE_COLUMNS: usize = 146;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_LIVE_NONZEROS: usize = 22_548;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_ROW_NONZEROS: usize = 173;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FILL_IN: usize = 35_667;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CANCELLATIONS: usize = 18_637;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CLEARED_PIVOTS: usize = 22_405;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FIELD_WORK_UNITS: usize = 187_454;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_DEPENDENT_ROWS: usize = 380;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_COLUMN_CATALOG_CHECKSUM: u64 = 0x8f90_45fe_64f3_72ab;
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_MATRIX_CHECKSUMS: [u64; 3] = [
    0xed2b_b65c_a209_b363,
    0x1785_02e2_cd90_1e9a,
    0xe7bf_f0c8_eb5e_034c,
];
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PIVOT_CHECKSUMS: [u64; 3] = [
    0x5436_69de_12d7_2458,
    0xb0bb_6da9_39e6_943c,
    0x915f_2974_4260_8f2b,
];
pub const FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CHECKSUM: u64 = 0x2cca_473b_7966_324a;

/// One specialization `d -> dimension (mod prime)` of the exact `Q(d)` rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopNextModularImage {
    prime: u64,
    dimension: u64,
}

impl FourLoopNextModularImage {
    pub const fn new(prime: u64, dimension: u64) -> Self {
        Self { prime, dimension }
    }

    pub const fn prime(self) -> u64 {
        self.prime
    }

    pub const fn dimension(self) -> u64 {
        self.dimension
    }
}

/// Independent resource limits for one modular discovery run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextModularRankConfig {
    pub max_images: usize,
    pub max_initial_nonzeros: usize,
    pub max_live_nonzeros: usize,
    pub max_cumulative_fill_in: usize,
    /// Maximum deterministic finite-field work units during elimination.
    ///
    /// One unit is charged for each pivot inversion, each pivot-row
    /// normalization multiplication, each cleared target pivot, and each
    /// non-pivot target coefficient update.
    pub max_elimination_updates: usize,
}

impl Default for FourLoopNextModularRankConfig {
    fn default() -> Self {
        Self {
            max_images: 16,
            max_initial_nonzeros: FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
            max_live_nonzeros: FOUR_LOOP_NEXT_CLOSED_ROWS
                * FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
            max_cumulative_fill_in: 100_000_000,
            max_elimination_updates: 500_000_000,
        }
    }
}

/// This status is intentionally incapable of expressing an exact rank claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextModularRankStatus {
    FiniteFieldDiscoveryEvidenceOnly,
}

/// One deterministic restricted-Markowitz pivot choice.
///
/// The hardest active column is fixed first.  Markowitz selection is then
/// restricted to rows incident on that column, with ties resolved by current
/// row width and finally by the immutable source-row index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextModularPivot {
    step: usize,
    source_row_index: usize,
    column_index: usize,
    row_nonzeros: usize,
    column_nonzeros: usize,
    markowitz_score: u64,
}

impl FourLoopNextModularPivot {
    pub const fn step(self) -> usize {
        self.step
    }

    pub const fn source_row_index(self) -> usize {
        self.source_row_index
    }

    pub const fn column_index(self) -> usize {
        self.column_index
    }

    pub const fn row_nonzeros(self) -> usize {
        self.row_nonzeros
    }

    pub const fn column_nonzeros(self) -> usize {
        self.column_nonzeros
    }

    pub const fn markowitz_score(self) -> u64 {
        self.markowitz_score
    }
}

/// Fill and arithmetic counters for one finite-field image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextModularFillStats {
    source_nonzeros: usize,
    initial_nonzeros: usize,
    evaluated_zero_coefficients: usize,
    peak_live_nonzeros: usize,
    peak_row_nonzeros: usize,
    cumulative_fill_in: usize,
    cancellations: usize,
    cleared_pivot_entries: usize,
    elimination_updates: usize,
    dependent_rows: usize,
}

impl FourLoopNextModularFillStats {
    pub const fn source_nonzeros(self) -> usize {
        self.source_nonzeros
    }

    pub const fn initial_nonzeros(self) -> usize {
        self.initial_nonzeros
    }

    pub const fn evaluated_zero_coefficients(self) -> usize {
        self.evaluated_zero_coefficients
    }

    pub const fn peak_live_nonzeros(self) -> usize {
        self.peak_live_nonzeros
    }

    pub const fn peak_row_nonzeros(self) -> usize {
        self.peak_row_nonzeros
    }

    pub const fn cumulative_fill_in(self) -> usize {
        self.cumulative_fill_in
    }

    pub const fn cancellations(self) -> usize {
        self.cancellations
    }

    pub const fn cleared_pivot_entries(self) -> usize {
        self.cleared_pivot_entries
    }

    /// Deterministic finite-field work units consumed by elimination.
    ///
    /// See [`FourLoopNextModularRankConfig::max_elimination_updates`] for the
    /// exact charging convention.
    pub const fn elimination_updates(self) -> usize {
        self.elimination_updates
    }

    pub const fn dependent_rows(self) -> usize {
        self.dependent_rows
    }
}

/// Discovery result for one accepted finite-field image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextModularImageReport {
    image: FourLoopNextModularImage,
    rank: usize,
    free_columns: usize,
    pivots: Vec<FourLoopNextModularPivot>,
    fill: FourLoopNextModularFillStats,
    matrix_checksum: u64,
    pivot_checksum: u64,
}

impl FourLoopNextModularImageReport {
    pub const fn image(&self) -> FourLoopNextModularImage {
        self.image
    }

    /// Modular rank at this image; this is not an exact `Q(d)` rank claim.
    pub const fn rank(&self) -> usize {
        self.rank
    }

    pub const fn free_columns(&self) -> usize {
        self.free_columns
    }

    pub fn pivots(&self) -> &[FourLoopNextModularPivot] {
        &self.pivots
    }

    pub const fn fill(&self) -> FourLoopNextModularFillStats {
        self.fill
    }

    pub const fn matrix_checksum(&self) -> u64 {
        self.matrix_checksum
    }

    pub const fn pivot_checksum(&self) -> u64 {
        self.pivot_checksum
    }
}

/// Multi-image discovery evidence for the frozen parent matrix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextModularRankReport {
    source_checksum: u64,
    column_catalog_checksum: u64,
    images: Vec<FourLoopNextModularImageReport>,
    ranks_agree: bool,
    pivot_columns_agree: bool,
    pivot_skeletons_agree: bool,
    checksum: u64,
}

impl FourLoopNextModularRankReport {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-next-modular-rank-discovery-v1";

    pub const fn status(&self) -> FourLoopNextModularRankStatus {
        FourLoopNextModularRankStatus::FiniteFieldDiscoveryEvidenceOnly
    }

    pub const fn source_checksum(&self) -> u64 {
        self.source_checksum
    }

    pub const fn column_catalog_checksum(&self) -> u64 {
        self.column_catalog_checksum
    }

    pub fn images(&self) -> &[FourLoopNextModularImageReport] {
        &self.images
    }

    pub const fn ranks_agree(&self) -> bool {
        self.ranks_agree
    }

    pub const fn pivot_columns_agree(&self) -> bool {
        self.pivot_columns_agree
    }

    pub const fn pivot_skeletons_agree(&self) -> bool {
        self.pivot_skeletons_agree
    }

    /// The common modular rank, if all requested images agree.
    ///
    /// Even when present, this remains finite-field discovery evidence only.
    pub fn common_modular_rank(&self) -> Option<usize> {
        self.ranks_agree
            .then(|| self.images.first().expect("a report has images").rank)
    }

    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
}

impl fmt::Display for FourLoopNextModularRankReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "{} status={:?}",
            Self::SCHEMA,
            FourLoopNextModularRankStatus::FiniteFieldDiscoveryEvidenceOnly
        )?;
        writeln!(
            formatter,
            "rows={} columns={} images={} source_checksum=0x{:016x} column_catalog_checksum=0x{:016x}",
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
            self.images.len(),
            self.source_checksum,
            self.column_catalog_checksum
        )?;
        for report in &self.images {
            writeln!(
                formatter,
                "image p={} d={} rank={} free_columns={} source_nnz={} initial_modular_nnz={} modular_zeros={} peak_live_nnz={} peak_row_nnz={} fill_in={} cancellations={} cleared_pivots={} field_work_units={} dependent_rows={} matrix_checksum=0x{:016x} pivot_checksum=0x{:016x}",
                report.image.prime,
                report.image.dimension,
                report.rank,
                report.free_columns,
                report.fill.source_nonzeros,
                report.fill.initial_nonzeros,
                report.fill.evaluated_zero_coefficients,
                report.fill.peak_live_nonzeros,
                report.fill.peak_row_nonzeros,
                report.fill.cumulative_fill_in,
                report.fill.cancellations,
                report.fill.cleared_pivot_entries,
                report.fill.elimination_updates,
                report.fill.dependent_rows,
                report.matrix_checksum,
                report.pivot_checksum,
            )?;
            let skeleton = report
                .pivots
                .iter()
                .map(|pivot| format!("{}@{}", pivot.column_index, pivot.source_row_index))
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                formatter,
                "pivot_skeleton[p={},d={}]=[{}]",
                report.image.prime, report.image.dimension, skeleton
            )?;
        }
        write!(
            formatter,
            "ranks_agree={} pivot_columns_agree={} pivot_skeletons_agree={} discovery_checksum=0x{:016x}; finite-field evidence only",
            self.ranks_agree, self.pivot_columns_agree, self.pivot_skeletons_agree, self.checksum,
        )
    }
}

/// Failures from modular specialization or bounded sparse elimination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopNextModularRankError {
    EmptyImageSet,
    DuplicateImage(FourLoopNextModularImage),
    InvalidPrime(u64),
    PrimeTooLarge(u64),
    DimensionOutOfRange(FourLoopNextModularImage),
    SourceShapeMismatch {
        rows: usize,
        columns: usize,
    },
    SourceChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    CoefficientContextMismatch,
    ColumnCatalogMismatch {
        row_index: usize,
    },
    RetainedMassDependence {
        row_index: usize,
        column_index: usize,
        part: &'static str,
    },
    IntegerReductionFailed,
    ZeroDenominator {
        image_index: usize,
        image: FourLoopNextModularImage,
        row_index: usize,
        column_index: usize,
    },
    SingularPivot {
        step: usize,
        row_index: usize,
        column_index: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for FourLoopNextModularRankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyImageSet => {
                formatter.write_str("modular discovery needs at least one image")
            }
            Self::DuplicateImage(image) => write!(
                formatter,
                "duplicate modular image p={}, d={}",
                image.prime, image.dimension
            ),
            Self::InvalidPrime(prime) => write!(formatter, "{prime} is not an odd prime"),
            Self::PrimeTooLarge(prime) => write!(
                formatter,
                "prime {prime} exceeds the supported signed-integer reduction range"
            ),
            Self::DimensionOutOfRange(image) => write!(
                formatter,
                "modular dimension {} must be smaller than prime {}",
                image.dimension, image.prime
            ),
            Self::SourceShapeMismatch { rows, columns } => write!(
                formatter,
                "modular discovery requires the frozen {FOUR_LOOP_NEXT_CLOSED_ROWS}x{FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS} matrix, found {rows}x{columns}"
            ),
            Self::SourceChecksumMismatch { expected, actual } => write!(
                formatter,
                "parent-row checksum mismatch: expected 0x{expected:016x}, found 0x{actual:016x}"
            ),
            Self::CoefficientContextMismatch => formatter.write_str(
                "modular discovery requires the canonical ordered coefficient context [d,m2]",
            ),
            Self::ColumnCatalogMismatch { row_index } => write!(
                formatter,
                "row {row_index} contains a column outside the frozen catalog"
            ),
            Self::RetainedMassDependence {
                row_index,
                column_index,
                part,
            } => write!(
                formatter,
                "row {row_index}, column {column_index} retains m2 in its {part}"
            ),
            Self::IntegerReductionFailed => formatter.write_str(
                "an exact integer coefficient could not be reduced to the selected finite field",
            ),
            Self::ZeroDenominator {
                image_index,
                image,
                row_index,
                column_index,
            } => write!(
                formatter,
                "image {image_index} (p={}, d={}) is rejected: row {row_index}, column {column_index} has zero denominator",
                image.prime, image.dimension
            ),
            Self::SingularPivot {
                step,
                row_index,
                column_index,
            } => write!(
                formatter,
                "modular pivot {step} at row {row_index}, column {column_index} is singular"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "modular {resource} requested {requested}, limit is {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "arithmetic overflow while counting {resource}")
            }
        }
    }
}

impl Error for FourLoopNextModularRankError {}

/// Run the three frozen finite-field images with bounded discovery defaults.
pub fn discover_four_loop_next_modular_rank(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
) -> Result<FourLoopNextModularRankReport, FourLoopNextModularRankError> {
    discover_four_loop_next_modular_rank_at_images(
        closed,
        &FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES,
        FourLoopNextModularRankConfig::default(),
    )
}

/// Specialize and eliminate the frozen parent matrix at caller-selected images.
///
/// The exact rows are never mutated.  Images with a zero exact denominator are
/// rejected before modular division.  Agreement returned in the report is
/// evidence for reconstruction planning, not an exact theorem.
pub fn discover_four_loop_next_modular_rank_at_images(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    images: &[FourLoopNextModularImage],
    config: FourLoopNextModularRankConfig,
) -> Result<FourLoopNextModularRankReport, FourLoopNextModularRankError> {
    validate_source(closed, images, config)?;
    let column_catalog_checksum = column_catalog_checksum(closed);
    let mut reports = Vec::new();
    reports.try_reserve_exact(images.len()).map_err(|_| {
        FourLoopNextModularRankError::ResourceLimit {
            resource: "image reports",
            requested: images.len() as u128,
            limit: config.max_images as u128,
        }
    })?;
    for (image_index, &image) in images.iter().enumerate() {
        reports.push(specialize_and_eliminate(
            closed,
            image_index,
            image,
            config,
        )?);
    }

    let (ranks_agree, pivot_columns_agree, pivot_skeletons_agree) = agreement_flags(&reports);
    let checksum = report_checksum(
        closed.checksum(),
        column_catalog_checksum,
        &reports,
        ranks_agree,
        pivot_columns_agree,
        pivot_skeletons_agree,
    );
    Ok(FourLoopNextModularRankReport {
        source_checksum: closed.checksum(),
        column_catalog_checksum,
        images: reports,
        ranks_agree,
        pivot_columns_agree,
        pivot_skeletons_agree,
        checksum,
    })
}

fn agreement_flags(reports: &[FourLoopNextModularImageReport]) -> (bool, bool, bool) {
    if reports.len() < 2 {
        return (false, false, false);
    }
    (
        reports.windows(2).all(|pair| pair[0].rank == pair[1].rank),
        reports
            .windows(2)
            .all(|pair| same_pivot_columns(pair[0].pivots.as_slice(), pair[1].pivots.as_slice())),
        reports
            .windows(2)
            .all(|pair| same_pivot_skeleton(pair[0].pivots.as_slice(), pair[1].pivots.as_slice())),
    )
}

fn same_pivot_columns(
    left: &[FourLoopNextModularPivot],
    right: &[FourLoopNextModularPivot],
) -> bool {
    left.iter()
        .map(|pivot| pivot.column_index)
        .eq(right.iter().map(|pivot| pivot.column_index))
}

fn same_pivot_skeleton(
    left: &[FourLoopNextModularPivot],
    right: &[FourLoopNextModularPivot],
) -> bool {
    left.iter()
        .map(|pivot| (pivot.column_index, pivot.source_row_index))
        .eq(right
            .iter()
            .map(|pivot| (pivot.column_index, pivot.source_row_index)))
}

fn validate_source(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    images: &[FourLoopNextModularImage],
    config: FourLoopNextModularRankConfig,
) -> Result<(), FourLoopNextModularRankError> {
    if images.is_empty() {
        return Err(FourLoopNextModularRankError::EmptyImageSet);
    }
    check_resource("images", images.len(), config.max_images)?;
    check_resource(
        "initial nonzeros",
        FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES,
        config.max_initial_nonzeros,
    )?;
    let mut unique = BTreeSet::new();
    for &image in images {
        if !unique.insert(image) {
            return Err(FourLoopNextModularRankError::DuplicateImage(image));
        }
        if image.prime > i64::MAX as u64 {
            return Err(FourLoopNextModularRankError::PrimeTooLarge(image.prime));
        }
        if image.prime == 2 || !is_prime(image.prime) {
            return Err(FourLoopNextModularRankError::InvalidPrime(image.prime));
        }
        if image.dimension >= image.prime {
            return Err(FourLoopNextModularRankError::DimensionOutOfRange(image));
        }
    }
    if closed.rows().len() != FOUR_LOOP_NEXT_CLOSED_ROWS
        || closed.columns().len() != FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS
    {
        return Err(FourLoopNextModularRankError::SourceShapeMismatch {
            rows: closed.rows().len(),
            columns: closed.columns().len(),
        });
    }
    if closed.checksum() != FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM {
        return Err(FourLoopNextModularRankError::SourceChecksumMismatch {
            expected: FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM,
            actual: closed.checksum(),
        });
    }
    if closed.coefficient_context().parameter_names() != ["d", "m2"] {
        return Err(FourLoopNextModularRankError::CoefficientContextMismatch);
    }
    if !closed.columns().windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(FourLoopNextModularRankError::CoefficientContextMismatch);
    }
    Ok(())
}

type ModularRow = BTreeMap<usize, u64>;

fn specialize_and_eliminate(
    closed: &FourLoopNextClosedRows<'_, '_, '_>,
    image_index: usize,
    image: FourLoopNextModularImage,
    config: FourLoopNextModularRankConfig,
) -> Result<FourLoopNextModularImageReport, FourLoopNextModularRankError> {
    let prime = image.prime;
    let dimension = image.dimension % prime;
    let mut matrix_hash = FNV1A64_OFFSET;
    hash_u64(&mut matrix_hash, closed.checksum());
    hash_u64(&mut matrix_hash, prime);
    hash_u64(&mut matrix_hash, dimension);

    let mut exact_nonzeros = 0_usize;
    let mut evaluated_zero_coefficients = 0_usize;
    let mut rows = Vec::new();
    rows.try_reserve_exact(closed.rows().len()).map_err(|_| {
        FourLoopNextModularRankError::ResourceLimit {
            resource: "specialized rows",
            requested: closed.rows().len() as u128,
            limit: FOUR_LOOP_NEXT_CLOSED_ROWS as u128,
        }
    })?;
    for (row_index, source_row) in closed.rows().iter().enumerate() {
        hash_usize(&mut matrix_hash, row_index);
        let mut row = ModularRow::new();
        for (column, coefficient) in source_row.entries() {
            exact_nonzeros = checked_add(exact_nonzeros, 1, "initial nonzeros")?;
            let column_index = closed
                .columns()
                .binary_search(column)
                .map_err(|_| FourLoopNextModularRankError::ColumnCatalogMismatch { row_index })?;
            let value = evaluate_coefficient(
                coefficient,
                dimension,
                prime,
                image_index,
                image,
                row_index,
                column_index,
            )?;
            if value == 0 {
                evaluated_zero_coefficients = checked_add(
                    evaluated_zero_coefficients,
                    1,
                    "evaluated zero coefficients",
                )?;
                continue;
            }
            hash_usize(&mut matrix_hash, column_index);
            hash_u64(&mut matrix_hash, value);
            row.insert(column_index, value);
        }
        hash_u64(&mut matrix_hash, u64::MAX);
        rows.push(row);
    }
    check_resource(
        "initial nonzeros",
        exact_nonzeros,
        config.max_initial_nonzeros,
    )?;
    let (pivots, mut fill) =
        eliminate_modular_rows(rows, closed.columns().len(), Field { prime }, config)?;
    fill.source_nonzeros = exact_nonzeros;
    fill.evaluated_zero_coefficients = evaluated_zero_coefficients;
    let rank = pivots.len();
    let free_columns = closed.columns().len().checked_sub(rank).ok_or(
        FourLoopNextModularRankError::ArithmeticOverflow {
            resource: "free columns",
        },
    )?;
    let pivot_checksum = pivot_checksum(image, &pivots, fill);
    Ok(FourLoopNextModularImageReport {
        image,
        rank,
        free_columns,
        pivots,
        fill,
        matrix_checksum: matrix_hash,
        pivot_checksum,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_coefficient(
    coefficient: &Coefficient,
    dimension: u64,
    prime: u64,
    image_index: usize,
    image: FourLoopNextModularImage,
    row_index: usize,
    column_index: usize,
) -> Result<u64, FourLoopNextModularRankError> {
    if coefficient.numerator.variables.len() != 2
        || coefficient.denominator.variables.as_ref() != coefficient.numerator.variables.as_ref()
    {
        return Err(FourLoopNextModularRankError::CoefficientContextMismatch);
    }
    ensure_mass_free(
        coefficient,
        CoefficientPolynomialPart::Numerator,
        row_index,
        column_index,
    )?;
    ensure_mass_free(
        coefficient,
        CoefficientPolynomialPart::Denominator,
        row_index,
        column_index,
    )?;
    let numerator = evaluate_qd_polynomial(
        coefficient,
        CoefficientPolynomialPart::Numerator,
        dimension,
        prime,
    )?;
    let denominator = evaluate_qd_polynomial(
        coefficient,
        CoefficientPolynomialPart::Denominator,
        dimension,
        prime,
    )?;
    if denominator == 0 {
        return Err(FourLoopNextModularRankError::ZeroDenominator {
            image_index,
            image,
            row_index,
            column_index,
        });
    }
    Ok(mod_mul(numerator, mod_inverse(denominator, prime), prime))
}

#[derive(Clone, Copy)]
enum CoefficientPolynomialPart {
    Numerator,
    Denominator,
}

impl CoefficientPolynomialPart {
    const fn label(self) -> &'static str {
        match self {
            Self::Numerator => "numerator",
            Self::Denominator => "denominator",
        }
    }
}

fn ensure_mass_free(
    coefficient: &Coefficient,
    part: CoefficientPolynomialPart,
    row_index: usize,
    column_index: usize,
) -> Result<(), FourLoopNextModularRankError> {
    let polynomial = match part {
        CoefficientPolynomialPart::Numerator => &coefficient.numerator,
        CoefficientPolynomialPart::Denominator => &coefficient.denominator,
    };
    if polynomial
        .exponents
        .chunks_exact(2)
        .any(|exponents| exponents[1] != 0)
    {
        return Err(FourLoopNextModularRankError::RetainedMassDependence {
            row_index,
            column_index,
            part: part.label(),
        });
    }
    Ok(())
}

fn evaluate_qd_polynomial(
    coefficient: &Coefficient,
    part: CoefficientPolynomialPart,
    dimension: u64,
    prime: u64,
) -> Result<u64, FourLoopNextModularRankError> {
    let polynomial = match part {
        CoefficientPolynomialPart::Numerator => &coefficient.numerator,
        CoefficientPolynomialPart::Denominator => &coefficient.denominator,
    };
    let signed_prime =
        i64::try_from(prime).map_err(|_| FourLoopNextModularRankError::PrimeTooLarge(prime))?;
    let mut result = 0_u64;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        let exponent = u64::from(polynomial.exponents[term * 2]);
        let remainder = coefficient % signed_prime;
        let remainder = remainder
            .to_i64()
            .ok_or(FourLoopNextModularRankError::IntegerReductionFailed)?;
        let coefficient = u64::try_from(remainder)
            .map_err(|_| FourLoopNextModularRankError::IntegerReductionFailed)?;
        let monomial = mod_mul(coefficient, mod_power(dimension, exponent, prime), prime);
        result = mod_add(result, monomial, prime);
    }
    Ok(result)
}

#[derive(Clone, Copy)]
struct Field {
    prime: u64,
}

impl Field {
    fn sub(self, left: u64, right: u64) -> u64 {
        if left >= right {
            left - right
        } else {
            self.prime - (right - left)
        }
    }

    fn mul(self, left: u64, right: u64) -> u64 {
        mod_mul(left, right, self.prime)
    }

    fn inverse(self, value: u64) -> u64 {
        mod_inverse(value, self.prime)
    }
}

fn eliminate_modular_rows(
    source_rows: Vec<ModularRow>,
    column_count: usize,
    field: Field,
    config: FourLoopNextModularRankConfig,
) -> Result<
    (Vec<FourLoopNextModularPivot>, FourLoopNextModularFillStats),
    FourLoopNextModularRankError,
> {
    let source_row_count = source_rows.len();
    let mut rows = source_rows.into_iter().map(Some).collect::<Vec<_>>();
    let mut incidence = vec![BTreeSet::<usize>::new(); column_count];
    let mut live_nonzeros = 0_usize;
    let mut peak_row_nonzeros = 0_usize;
    for (row_index, row) in rows.iter().enumerate() {
        let row = row.as_ref().expect("source rows start active");
        live_nonzeros = checked_add(live_nonzeros, row.len(), "live nonzeros")?;
        peak_row_nonzeros = peak_row_nonzeros.max(row.len());
        for &column in row.keys() {
            let column_rows = incidence
                .get_mut(column)
                .ok_or(FourLoopNextModularRankError::ColumnCatalogMismatch { row_index })?;
            column_rows.insert(row_index);
        }
    }
    check_resource("live nonzeros", live_nonzeros, config.max_live_nonzeros)?;
    let mut fill = FourLoopNextModularFillStats {
        initial_nonzeros: live_nonzeros,
        peak_live_nonzeros: live_nonzeros,
        peak_row_nonzeros,
        ..FourLoopNextModularFillStats::default()
    };
    let mut pivots = Vec::new();
    pivots
        .try_reserve_exact(source_row_count.min(column_count))
        .map_err(|_| FourLoopNextModularRankError::ResourceLimit {
            resource: "pivot skeleton",
            requested: source_row_count.min(column_count) as u128,
            limit: column_count as u128,
        })?;

    while let Some(column_index) = incidence.iter().rposition(|rows| !rows.is_empty()) {
        let column_nonzeros = incidence[column_index].len();
        let markowitz_column_factor = column_nonzeros.saturating_sub(1);
        let (source_row_index, row_nonzeros, markowitz_score) = incidence[column_index]
            .iter()
            .copied()
            .map(|row_index| {
                let row_nonzeros = rows[row_index]
                    .as_ref()
                    .expect("incidence only names active rows")
                    .len();
                let score = markowitz_score(row_nonzeros, markowitz_column_factor);
                (row_index, row_nonzeros, score)
            })
            .min_by_key(|&(row_index, row_nonzeros, score)| (score, row_nonzeros, row_index))
            .expect("an active column has an incident row");
        let step = pivots.len();
        let mut pivot_row = rows[source_row_index]
            .take()
            .expect("selected pivot row is active");
        let pivot_coefficient =
            *pivot_row
                .get(&column_index)
                .ok_or(FourLoopNextModularRankError::SingularPivot {
                    step,
                    row_index: source_row_index,
                    column_index,
                })?;
        if pivot_coefficient == 0 {
            return Err(FourLoopNextModularRankError::SingularPivot {
                step,
                row_index: source_row_index,
                column_index,
            });
        }
        charge_update(&mut fill, config)?;
        let inverse = field.inverse(pivot_coefficient);
        for coefficient in pivot_row.values_mut() {
            charge_update(&mut fill, config)?;
            *coefficient = field.mul(*coefficient, inverse);
        }
        if pivot_row.get(&column_index) != Some(&1) {
            return Err(FourLoopNextModularRankError::SingularPivot {
                step,
                row_index: source_row_index,
                column_index,
            });
        }

        let target_rows = incidence[column_index]
            .iter()
            .copied()
            .filter(|&row_index| row_index != source_row_index)
            .collect::<Vec<_>>();
        for &column in pivot_row.keys() {
            incidence[column].remove(&source_row_index);
        }
        live_nonzeros = live_nonzeros.checked_sub(pivot_row.len()).ok_or(
            FourLoopNextModularRankError::ArithmeticOverflow {
                resource: "live nonzeros",
            },
        )?;

        for target_row_index in target_rows {
            let target = rows[target_row_index]
                .as_mut()
                .expect("pivot incidence only names active target rows");
            let factor = target.remove(&column_index).ok_or(
                FourLoopNextModularRankError::SingularPivot {
                    step,
                    row_index: target_row_index,
                    column_index,
                },
            )?;
            incidence[column_index].remove(&target_row_index);
            live_nonzeros = live_nonzeros.checked_sub(1).ok_or(
                FourLoopNextModularRankError::ArithmeticOverflow {
                    resource: "live nonzeros",
                },
            )?;
            fill.cleared_pivot_entries =
                checked_add(fill.cleared_pivot_entries, 1, "cleared pivot entries")?;
            charge_update(&mut fill, config)?;

            for (&column, &pivot_value) in &pivot_row {
                if column == column_index {
                    continue;
                }
                charge_update(&mut fill, config)?;
                let delta = field.mul(factor, pivot_value);
                if let Some(current) = target.get(&column).copied() {
                    let updated = field.sub(current, delta);
                    if updated == 0 {
                        target.remove(&column);
                        incidence[column].remove(&target_row_index);
                        live_nonzeros = live_nonzeros.checked_sub(1).ok_or(
                            FourLoopNextModularRankError::ArithmeticOverflow {
                                resource: "live nonzeros",
                            },
                        )?;
                        fill.cancellations = checked_add(fill.cancellations, 1, "cancellations")?;
                    } else {
                        target.insert(column, updated);
                    }
                } else if delta != 0 {
                    target.insert(column, field.sub(0, delta));
                    incidence[column].insert(target_row_index);
                    live_nonzeros = checked_add(live_nonzeros, 1, "live nonzeros")?;
                    fill.cumulative_fill_in =
                        checked_add(fill.cumulative_fill_in, 1, "cumulative fill-in")?;
                    check_resource(
                        "cumulative fill-in",
                        fill.cumulative_fill_in,
                        config.max_cumulative_fill_in,
                    )?;
                    check_resource("live nonzeros", live_nonzeros, config.max_live_nonzeros)?;
                    fill.peak_live_nonzeros = fill.peak_live_nonzeros.max(live_nonzeros);
                    fill.peak_row_nonzeros = fill.peak_row_nonzeros.max(target.len());
                }
            }
            fill.peak_row_nonzeros = fill.peak_row_nonzeros.max(target.len());
            if target.is_empty() {
                rows[target_row_index] = None;
            }
        }

        pivots.push(FourLoopNextModularPivot {
            step,
            source_row_index,
            column_index,
            row_nonzeros,
            column_nonzeros,
            markowitz_score,
        });
    }

    fill.dependent_rows = source_row_count.checked_sub(pivots.len()).ok_or(
        FourLoopNextModularRankError::ArithmeticOverflow {
            resource: "dependent rows",
        },
    )?;
    Ok((pivots, fill))
}

fn markowitz_score(row_nonzeros: usize, column_factor: usize) -> u64 {
    u64::try_from(row_nonzeros.saturating_sub(1))
        .unwrap_or(u64::MAX)
        .saturating_mul(u64::try_from(column_factor).unwrap_or(u64::MAX))
}

fn charge_update(
    fill: &mut FourLoopNextModularFillStats,
    config: FourLoopNextModularRankConfig,
) -> Result<(), FourLoopNextModularRankError> {
    fill.elimination_updates = checked_add(fill.elimination_updates, 1, "finite-field work units")?;
    check_resource(
        "finite-field work units",
        fill.elimination_updates,
        config.max_elimination_updates,
    )
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FourLoopNextModularRankError> {
    if requested > limit {
        Err(FourLoopNextModularRankError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopNextModularRankError> {
    left.checked_add(right)
        .ok_or(FourLoopNextModularRankError::ArithmeticOverflow { resource })
}

fn mod_add(left: u64, right: u64, prime: u64) -> u64 {
    ((u128::from(left) + u128::from(right)) % u128::from(prime)) as u64
}

fn mod_mul(left: u64, right: u64, prime: u64) -> u64 {
    (u128::from(left) * u128::from(right) % u128::from(prime)) as u64
}

fn mod_power(mut base: u64, mut exponent: u64, prime: u64) -> u64 {
    base %= prime;
    let mut value = 1_u64;
    while exponent > 0 {
        if exponent & 1 == 1 {
            value = mod_mul(value, base, prime);
        }
        base = mod_mul(base, base, prime);
        exponent >>= 1;
    }
    value
}

fn mod_inverse(value: u64, prime: u64) -> u64 {
    debug_assert_ne!(value, 0);
    mod_power(value, prime - 2, prime)
}

fn is_prime(value: u64) -> bool {
    if value < 2 {
        return false;
    }
    for prime in [2_u64, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if value == prime {
            return true;
        }
        if value % prime == 0 {
            return false;
        }
    }
    let trailing = (value - 1).trailing_zeros();
    let odd_part = (value - 1) >> trailing;
    for base in [2_u64, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022] {
        let base = base % value;
        if base == 0 {
            continue;
        }
        let mut witness = mod_power(base, odd_part, value);
        if witness == 1 || witness == value - 1 {
            continue;
        }
        let mut composite = true;
        for _ in 1..trailing {
            witness = mod_mul(witness, witness, value);
            if witness == value - 1 {
                composite = false;
                break;
            }
        }
        if composite {
            return false;
        }
    }
    true
}

fn column_catalog_checksum(closed: &FourLoopNextClosedRows<'_, '_, '_>) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, FourLoopNextModularRankReport::SCHEMA.as_bytes());
    for (column_index, column) in closed.columns().iter().enumerate() {
        hash_usize(&mut hash, column_index);
        hash_bytes(&mut hash, column.stable_key().as_bytes());
    }
    hash
}

fn pivot_checksum(
    image: FourLoopNextModularImage,
    pivots: &[FourLoopNextModularPivot],
    fill: FourLoopNextModularFillStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_u64(&mut hash, image.prime);
    hash_u64(&mut hash, image.dimension);
    for pivot in pivots {
        hash_usize(&mut hash, pivot.step);
        hash_usize(&mut hash, pivot.source_row_index);
        hash_usize(&mut hash, pivot.column_index);
        hash_usize(&mut hash, pivot.row_nonzeros);
        hash_usize(&mut hash, pivot.column_nonzeros);
        hash_u64(&mut hash, pivot.markowitz_score);
    }
    for value in [
        fill.source_nonzeros,
        fill.initial_nonzeros,
        fill.evaluated_zero_coefficients,
        fill.peak_live_nonzeros,
        fill.peak_row_nonzeros,
        fill.cumulative_fill_in,
        fill.cancellations,
        fill.cleared_pivot_entries,
        fill.elimination_updates,
        fill.dependent_rows,
    ] {
        hash_usize(&mut hash, value);
    }
    hash
}

fn report_checksum(
    source_checksum: u64,
    column_catalog_checksum: u64,
    reports: &[FourLoopNextModularImageReport],
    ranks_agree: bool,
    pivot_columns_agree: bool,
    pivot_skeletons_agree: bool,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, FourLoopNextModularRankReport::SCHEMA.as_bytes());
    hash_u64(&mut hash, source_checksum);
    hash_u64(&mut hash, column_catalog_checksum);
    for report in reports {
        hash_u64(&mut hash, report.image.prime);
        hash_u64(&mut hash, report.image.dimension);
        hash_usize(&mut hash, report.rank);
        hash_usize(&mut hash, report.free_columns);
        hash_u64(&mut hash, report.matrix_checksum);
        hash_u64(&mut hash, report.pivot_checksum);
    }
    hash_u64(&mut hash, if ranks_agree { 1 } else { 0 });
    hash_u64(&mut hash, if pivot_columns_agree { 1 } else { 0 });
    hash_u64(&mut hash, if pivot_skeletons_agree { 1 } else { 0 });
    hash
}

fn hash_usize(hash: &mut u64, value: usize) {
    hash_u64(hash, value as u64);
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entries: &[(usize, u64)]) -> ModularRow {
        entries.iter().copied().collect()
    }

    fn pivot(
        step: usize,
        source_row_index: usize,
        column_index: usize,
        row_nonzeros: usize,
        column_nonzeros: usize,
        markowitz_score: u64,
    ) -> FourLoopNextModularPivot {
        FourLoopNextModularPivot {
            step,
            source_row_index,
            column_index,
            row_nonzeros,
            column_nonzeros,
            markowitz_score,
        }
    }

    fn image_report(pivots: Vec<FourLoopNextModularPivot>) -> FourLoopNextModularImageReport {
        FourLoopNextModularImageReport {
            image: FourLoopNextModularImage::new(1_000_003, 101),
            rank: pivots.len(),
            free_columns: 0,
            pivots,
            fill: FourLoopNextModularFillStats::default(),
            matrix_checksum: 0,
            pivot_checksum: 0,
        }
    }

    #[test]
    fn frozen_images_are_distinct_odd_primes() {
        let distinct = FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            distinct.len(),
            FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES.len()
        );
        for image in FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES {
            assert!(is_prime(image.prime));
            assert_ne!(image.prime, 2);
            assert!(image.dimension < image.prime);
        }
    }

    #[test]
    fn restricted_markowitz_is_hardest_column_first_and_row_deterministic() {
        let rows = vec![
            row(&[(0, 1), (2, 1), (3, 1)]),
            row(&[(1, 1), (3, 1)]),
            row(&[(0, 1), (2, 1)]),
        ];
        let (pivots, fill) = eliminate_modular_rows(
            rows,
            4,
            Field { prime: 1_000_003 },
            FourLoopNextModularRankConfig::default(),
        )
        .unwrap();
        assert_eq!(
            pivots
                .iter()
                .map(|pivot| (pivot.column_index, pivot.source_row_index))
                .collect::<Vec<_>>(),
            [(3, 1), (2, 2), (1, 0)]
        );
        assert!(
            pivots
                .windows(2)
                .all(|pair| pair[0].column_index > pair[1].column_index)
        );
        assert_eq!(fill.dependent_rows, 0);
    }

    #[test]
    fn transient_row_fill_is_included_in_peak_and_normalization_is_charged() {
        // Columns 0 and 1 fill before column 2 cancels because BTreeMap walks
        // the normalized pivot row in ascending order.  The target therefore
        // reaches width five transiently even though both source rows and the
        // final target have width four.
        let rows = vec![
            row(&[(0, 1), (1, 1), (2, 1), (6, 1)]),
            row(&[(2, 1), (3, 1), (4, 1), (6, 1)]),
        ];
        let (_, fill) = eliminate_modular_rows(
            rows,
            7,
            Field { prime: 1_000_003 },
            FourLoopNextModularRankConfig::default(),
        )
        .unwrap();
        assert_eq!(fill.peak_row_nonzeros, 5);
        assert_eq!(fill.cumulative_fill_in, 2);
        assert_eq!(fill.cancellations, 1);
        // Two inversions + eight normalization multiplications + one cleared
        // pivot + three target coefficient updates.
        assert_eq!(fill.elimination_updates, 14);
    }

    #[test]
    fn one_image_is_not_cross_image_agreement() {
        let reports = [image_report(vec![pivot(0, 7, 11, 3, 2, 2)])];
        assert_eq!(agreement_flags(&reports), (false, false, false));
    }

    #[test]
    fn displayed_pivot_skeleton_ignores_fill_metadata() {
        let left = [pivot(0, 7, 11, 3, 2, 2), pivot(1, 4, 9, 5, 3, 8)];
        let right = [pivot(0, 7, 11, 30, 20, 200), pivot(1, 4, 9, 50, 30, 800)];
        assert_ne!(left, right);
        assert!(same_pivot_columns(&left, &right));
        assert!(same_pivot_skeleton(&left, &right));
        let reports = [image_report(left.to_vec()), image_report(right.to_vec())];
        assert_eq!(agreement_flags(&reports), (true, true, true));

        let different_row = [pivot(0, 8, 11, 30, 20, 200), right[1]];
        assert!(same_pivot_columns(&left, &different_row));
        assert!(!same_pivot_skeleton(&left, &different_row));
    }

    #[test]
    fn deterministic_u64_primality_rejects_basic_composites() {
        for value in [0, 1, 4, 9, 25, 1_000_005, 3_215_031_751] {
            assert!(!is_prime(value));
        }
        for value in [2, 3, 37, 1_000_003, 1_000_033, 1_000_037] {
            assert!(is_prime(value));
        }
    }
}
