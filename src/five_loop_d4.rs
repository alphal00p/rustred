//! Replayable scalar `D<=4` shell for the equal-mass five-loop banana.
//!
//! Seven scalar `S6` seed orbits through dot degree three generate 175
//! authenticated native IBP origins.  Their one-moment halo is closed by 50
//! exact diagonal/momentum identities and 11 certified five-line boundary
//! equations.  A mass grading projects the resulting fixed 236-by-82 system
//! from `Q(d,m2)` to `Q(d)` before modular skeleton discovery and exact sparse
//! elimination.
//!
//! The finite shell has exact rank 78.  Four of the five scalar `D=4` orbits
//! reduce, while `[2,2,2,2,1,1]` is retained as an explicitly named candidate
//! terminal.  This is not a master-minimality or unrestricted-reduction claim.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::{Integer, IntegerRing, MultivariatePolynomial, Z};

use crate::coefficient::{Coefficient, CoefficientContext, CoefficientProjectionError};
use crate::exact_sparse_elimination::{
    ExactSparseDerivationTrace, ExactSparseElimination, ExactSparseEliminationConfig,
    ExactSparseEliminationError, ExactSparseEliminationStats, ExactSparseRow,
};
use crate::five_loop::equal_mass_five_loop_banana;
use crate::five_loop_boundary::{
    FIVE_LOOP_BANANA_DENOMINATORS, FIVE_LOOP_BANANA_LOOP_MOMENTA,
    FIVE_LOOP_BANANA_PHYSICAL_LINES, FIVE_LOOP_BANANA_S6_ORDER, FiveLoopBananaBoundaryError,
    FiveLoopBananaBoundaryReducer, five_loop_banana_oriented_line_routing,
    five_loop_banana_physical_orbit_witness,
};
use crate::five_loop_d3::{
    FIVE_LOOP_BANANA_D3_RANK, FiveLoopBananaD3Config, FiveLoopBananaD3Error,
    FiveLoopBananaD3Shell,
};
use crate::{
    FamilyError, IbpGenerationError, IbpGenerator, Integral, LinearCombination, VacuumFamily,
};

const D4_SCHEMA: &str = "rustred-five-loop-banana-d4-shell-v1";
const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

pub const FIVE_LOOP_BANANA_D4_SEED_ORBITS: usize = 7;
pub const FIVE_LOOP_BANANA_D4_LABELLED_SEED_IMAGES: usize = 84;
pub const FIVE_LOOP_BANANA_D4_NATIVE_RAW_ORIGINS: usize = 175;
pub const FIVE_LOOP_BANANA_D4_NONZERO_NATIVE_ROWS: usize = 75;
pub const FIVE_LOOP_BANANA_D4_RAW_GRAPH_TERMS: usize = 385;
pub const FIVE_LOOP_BANANA_D4_NATIVE_EXPANSION_BOUND: usize =
    FIVE_LOOP_BANANA_D4_NATIVE_RAW_ORIGINS * (1 + 2 * (1 + FIVE_LOOP_BANANA_DENOMINATORS));
pub const FIVE_LOOP_BANANA_D4_MOMENT_POWER_CLASSES: usize = 11;
pub const FIVE_LOOP_BANANA_D4_ALGEBRAIC_CANDIDATES: usize =
    FIVE_LOOP_BANANA_D4_MOMENT_POWER_CLASSES * 12;
pub const FIVE_LOOP_BANANA_D4_ALGEBRAIC_ROWS: usize = 50;
pub const FIVE_LOOP_BANANA_D4_BOUNDARY_ROWS: usize = 11;
pub const FIVE_LOOP_BANANA_D4_SOURCE_ROWS: usize = FIVE_LOOP_BANANA_D4_NATIVE_RAW_ORIGINS
    + FIVE_LOOP_BANANA_D4_ALGEBRAIC_ROWS
    + FIVE_LOOP_BANANA_D4_BOUNDARY_ROWS;
pub const FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS: usize = 82;
pub const FIVE_LOOP_BANANA_D4_COLLECTED_ENTRIES: usize = 357;
pub const FIVE_LOOP_BANANA_D4_RANK: usize = 78;
pub const FIVE_LOOP_BANANA_D4_FREE_COLUMNS: usize = 4;
pub const FIVE_LOOP_BANANA_D4_TARGET_ORBITS: usize = 5;
pub const FIVE_LOOP_BANANA_D4_LABELLED_TARGETS: usize = 126;
pub const FIVE_LOOP_BANANA_D4_SCALAR_BOX_TARGETS: usize = 2_972;
pub const FIVE_LOOP_BANANA_D4_SCALAR_BOX_SCALELESS: usize = 2_006;
pub const FIVE_LOOP_BANANA_D4_SCALAR_BOX_BOUNDARY: usize = 756;
pub const FIVE_LOOP_BANANA_D4_SCALAR_BOX_TOP: usize = 210;
pub const FIVE_LOOP_BANANA_D4_SYMMETRY_IMAGE_BOUND: usize =
    (FIVE_LOOP_BANANA_D4_RAW_GRAPH_TERMS
        + FIVE_LOOP_BANANA_D4_MOMENT_POWER_CLASSES * 6 * 9)
        * FIVE_LOOP_BANANA_S6_ORDER;
pub const FIVE_LOOP_BANANA_D4_MASS_POWER_STEP_BOUND: usize = 128;

pub const FIVE_LOOP_BANANA_D4_MODULAR_IMAGES: [FiveLoopBananaD4ModularImage; 3] = [
    FiveLoopBananaD4ModularImage::new(1_000_003, 17),
    FiveLoopBananaD4ModularImage::new(1_000_033, 23),
    FiveLoopBananaD4ModularImage::new(1_000_037, 31),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4ModularImage {
    prime: u64,
    dimension: u64,
}
impl FiveLoopBananaD4ModularImage {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4Config {
    pub d3: FiveLoopBananaD3Config,
    pub exact: ExactSparseEliminationConfig,
    pub max_seed_orbits: usize,
    pub max_labelled_seed_images: usize,
    pub max_native_raw_origins: usize,
    pub max_raw_graph_terms: usize,
    pub max_native_expansion_incidences: usize,
    pub max_moment_power_classes: usize,
    pub max_algebraic_candidates: usize,
    pub max_algebraic_rows: usize,
    pub max_boundary_rows: usize,
    pub max_source_rows: usize,
    pub max_symmetry_images: usize,
    pub max_global_columns: usize,
    pub max_collected_entries: usize,
    pub max_mass_power_steps: usize,
    pub max_modular_updates: usize,
    pub max_typed_rhs_entries: usize,
    pub max_typed_trace_reductions: usize,
}

impl Default for FiveLoopBananaD4Config {
    fn default() -> Self {
        let exact = ExactSparseEliminationConfig {
            max_rows: FIVE_LOOP_BANANA_D4_SOURCE_ROWS,
            max_columns: FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS,
            max_input_entries: FIVE_LOOP_BANANA_D4_COLLECTED_ENTRIES,
            max_input_coefficient_bytes: 16 * 1024 * 1024,
            max_reductions: 4_000,
            max_updates: 1_000_000,
            max_retained_entries: FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS
                * FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS,
            max_retained_coefficient_terms: 10_000_000,
            max_retained_coefficient_bytes: 128 * 1024 * 1024,
            max_coefficient_degree: 1_024,
            max_coefficient_operation_terms: 1_000_000,
            max_coefficient_dense_terms: 10_000_000,
            max_integer_bits: 100_000,
            max_coefficient_pair_products: 100_000_000,
            max_canonicalization_work: 1_000_000_000,
            max_replay_reductions: 25_000,
            max_replay_updates: 5_000_000,
        };
        Self {
            d3: FiveLoopBananaD3Config::default(),
            exact,
            max_seed_orbits: FIVE_LOOP_BANANA_D4_SEED_ORBITS,
            max_labelled_seed_images: FIVE_LOOP_BANANA_D4_LABELLED_SEED_IMAGES,
            max_native_raw_origins: FIVE_LOOP_BANANA_D4_NATIVE_RAW_ORIGINS,
            max_raw_graph_terms: FIVE_LOOP_BANANA_D4_RAW_GRAPH_TERMS,
            max_native_expansion_incidences: FIVE_LOOP_BANANA_D4_NATIVE_EXPANSION_BOUND,
            max_moment_power_classes: FIVE_LOOP_BANANA_D4_MOMENT_POWER_CLASSES,
            max_algebraic_candidates: FIVE_LOOP_BANANA_D4_ALGEBRAIC_CANDIDATES,
            max_algebraic_rows: FIVE_LOOP_BANANA_D4_ALGEBRAIC_ROWS,
            max_boundary_rows: FIVE_LOOP_BANANA_D4_BOUNDARY_ROWS,
            max_source_rows: FIVE_LOOP_BANANA_D4_SOURCE_ROWS,
            max_symmetry_images: FIVE_LOOP_BANANA_D4_SYMMETRY_IMAGE_BOUND,
            max_global_columns: FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS,
            max_collected_entries: FIVE_LOOP_BANANA_D4_COLLECTED_ENTRIES,
            max_mass_power_steps: FIVE_LOOP_BANANA_D4_MASS_POWER_STEP_BOUND,
            max_modular_updates: 2_000_000,
            max_typed_rhs_entries: FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS
                * (FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS - 1),
            max_typed_trace_reductions: FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS
                * (FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS - 1)
                / 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD4SeedOrbit {
    P0,
    P1,
    P2,
    P11,
    P3,
    P21,
    P111,
}

impl FiveLoopBananaD4SeedOrbit {
    pub const ALL: [Self; FIVE_LOOP_BANANA_D4_SEED_ORBITS] = [
        Self::P0,
        Self::P1,
        Self::P2,
        Self::P11,
        Self::P3,
        Self::P21,
        Self::P111,
    ];

    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::P0 => "rustred-five-loop-banana-d4-seed-v1:P0",
            Self::P1 => "rustred-five-loop-banana-d4-seed-v1:P1",
            Self::P2 => "rustred-five-loop-banana-d4-seed-v1:P2",
            Self::P11 => "rustred-five-loop-banana-d4-seed-v1:P11-candidate",
            Self::P3 => "rustred-five-loop-banana-d4-seed-v1:P3",
            Self::P21 => "rustred-five-loop-banana-d4-seed-v1:P21",
            Self::P111 => "rustred-five-loop-banana-d4-seed-v1:P111",
        }
    }

    pub const fn powers(self) -> [i32; 6] {
        match self {
            Self::P0 => [1, 1, 1, 1, 1, 1],
            Self::P1 => [2, 1, 1, 1, 1, 1],
            Self::P2 => [3, 1, 1, 1, 1, 1],
            Self::P11 => [2, 2, 1, 1, 1, 1],
            Self::P3 => [4, 1, 1, 1, 1, 1],
            Self::P21 => [3, 2, 1, 1, 1, 1],
            Self::P111 => [2, 2, 2, 1, 1, 1],
        }
    }

    pub const fn labelled_orbit_size(self) -> usize {
        match self {
            Self::P0 => 1,
            Self::P1 | Self::P2 | Self::P3 => 6,
            Self::P11 => 15,
            Self::P21 => 30,
            Self::P111 => 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4Seed {
    orbit: FiveLoopBananaD4SeedOrbit,
    integral: Integral,
    labelled_orbit_size: usize,
}

impl FiveLoopBananaD4Seed {
    pub const fn orbit(&self) -> FiveLoopBananaD4SeedOrbit {
        self.orbit
    }
    pub const fn integral(&self) -> &Integral {
        &self.integral
    }
    pub const fn labelled_orbit_size(&self) -> usize {
        self.labelled_orbit_size
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiveLoopBananaD4NativeRowId {
    seed: FiveLoopBananaD4SeedOrbit,
    differentiated_loop: u8,
    contraction_loop: u8,
}

impl FiveLoopBananaD4NativeRowId {
    pub const fn new(seed: FiveLoopBananaD4SeedOrbit, differentiated_loop: u8, contraction_loop: u8) -> Self {
        Self { seed, differentiated_loop, contraction_loop }
    }
    pub const fn seed(self) -> FiveLoopBananaD4SeedOrbit { self.seed }
    pub const fn differentiated_loop(self) -> u8 { self.differentiated_loop }
    pub const fn contraction_loop(self) -> u8 { self.contraction_loop }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD4AlgebraicKind {
    Diagonal,
    Momentum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiveLoopBananaD4AlgebraicRowId {
    index: u16,
    kind: FiveLoopBananaD4AlgebraicKind,
    powers: [i32; 6],
    marked_line: u8,
}

impl FiveLoopBananaD4AlgebraicRowId {
    pub const fn index(self) -> u16 { self.index }
    pub const fn kind(self) -> FiveLoopBananaD4AlgebraicKind { self.kind }
    pub const fn powers(self) -> [i32; 6] { self.powers }
    pub const fn marked_line(self) -> u8 { self.marked_line }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiveLoopBananaD4BoundaryRowId {
    index: u8,
    powers: [i32; 6],
}

impl FiveLoopBananaD4BoundaryRowId {
    pub const fn index(self) -> u8 { self.index }
    pub const fn powers(self) -> [i32; 6] { self.powers }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD4RowId {
    Native(FiveLoopBananaD4NativeRowId),
    Algebraic(FiveLoopBananaD4AlgebraicRowId),
    Boundary(FiveLoopBananaD4BoundaryRowId),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FiveLoopBananaD4Column {
    BoundaryTerminal,
    ProperBoundary { powers: [i32; 6] },
    Scalar { powers: [i32; 6] },
    OneMoment { powers: [i32; 6], edge: [u8; 2] },
}

impl FiveLoopBananaD4Column {
    pub fn active_lines(&self) -> usize {
        match self {
            Self::BoundaryTerminal => 5,
            Self::ProperBoundary { powers } | Self::Scalar { powers } | Self::OneMoment { powers, .. } => {
                powers.iter().filter(|power| **power > 0).count()
            }
        }
    }

    pub fn dot_degree(&self) -> u64 {
        match self {
            Self::BoundaryTerminal => 0,
            Self::ProperBoundary { powers } | Self::Scalar { powers } | Self::OneMoment { powers, .. } => powers
                .iter()
                .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
                .sum(),
        }
    }

    pub const fn numerator_degree(&self) -> u64 {
        match self { Self::OneMoment { .. } => 1, _ => 0 }
    }

    pub fn mass_weight(&self) -> i32 {
        match self {
            Self::BoundaryTerminal => 5,
            Self::ProperBoundary { powers } | Self::Scalar { powers } => powers.iter().sum(),
            Self::OneMoment { powers, .. } => powers.iter().sum::<i32>() - 1,
        }
    }

    fn order_key(&self) -> (usize, u64, u64, u8, [i32; 6], [u8; 2]) {
        match self {
            Self::BoundaryTerminal => (5, 0, 0, 0, [0; 6], [0; 2]),
            Self::ProperBoundary { powers } => (5, self.dot_degree(), self.dot_degree(), 1, *powers, [0; 2]),
            Self::Scalar { powers } => (6, self.dot_degree(), self.dot_degree(), 0, *powers, [0; 2]),
            Self::OneMoment { powers, edge } => (6, self.dot_degree() + 1, self.dot_degree(), 1, *powers, *edge),
        }
    }
}

impl Ord for FiveLoopBananaD4Column {
    fn cmp(&self, other: &Self) -> Ordering { self.order_key().cmp(&other.order_key()) }
}
impl PartialOrd for FiveLoopBananaD4Column {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4SourceRow {
    row_id: FiveLoopBananaD4RowId,
    mass_weight: i32,
    entries: BTreeMap<FiveLoopBananaD4Column, Coefficient>,
}

impl FiveLoopBananaD4SourceRow {
    pub const fn row_id(&self) -> FiveLoopBananaD4RowId { self.row_id }
    pub const fn mass_weight(&self) -> i32 { self.mass_weight }
    /// Entries in the structurally projected `Q(d)` coefficient context.
    pub const fn entries(&self) -> &BTreeMap<FiveLoopBananaD4Column, Coefficient> { &self.entries }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4BoundaryClosure {
    row_id: FiveLoopBananaD4BoundaryRowId,
    input: Integral,
    reduction: LinearCombination,
}

impl FiveLoopBananaD4BoundaryClosure {
    pub const fn row_id(&self) -> FiveLoopBananaD4BoundaryRowId { self.row_id }
    pub const fn input(&self) -> &Integral { &self.input }
    pub const fn reduction(&self) -> &LinearCombination { &self.reduction }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4TraceReduction {
    prior_pivot_ordinal: usize,
    prior_pivot: FiveLoopBananaD4Column,
    factor: Coefficient,
}

impl FiveLoopBananaD4TraceReduction {
    pub const fn prior_pivot_ordinal(&self) -> usize { self.prior_pivot_ordinal }
    pub const fn prior_pivot(&self) -> &FiveLoopBananaD4Column { &self.prior_pivot }
    pub const fn factor(&self) -> &Coefficient { &self.factor }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4Trace {
    base_source_row_index: usize,
    base_source_row_id: FiveLoopBananaD4RowId,
    reductions: Vec<FiveLoopBananaD4TraceReduction>,
    divisor: Coefficient,
}

impl FiveLoopBananaD4Trace {
    pub const fn base_source_row_index(&self) -> usize { self.base_source_row_index }
    pub const fn base_source_row_id(&self) -> FiveLoopBananaD4RowId { self.base_source_row_id }
    pub fn reductions(&self) -> &[FiveLoopBananaD4TraceReduction] { &self.reductions }
    pub const fn divisor(&self) -> &Coefficient { &self.divisor }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD4PivotRule {
    pivot: FiveLoopBananaD4Column,
    rhs: BTreeMap<FiveLoopBananaD4Column, Coefficient>,
    trace: FiveLoopBananaD4Trace,
}

impl FiveLoopBananaD4PivotRule {
    pub const fn pivot(&self) -> &FiveLoopBananaD4Column { &self.pivot }
    pub const fn rhs(&self) -> &BTreeMap<FiveLoopBananaD4Column, Coefficient> { &self.rhs }
    pub const fn trace(&self) -> &FiveLoopBananaD4Trace { &self.trace }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FiveLoopBananaD4Stats {
    pub seed_orbits: usize,
    pub labelled_seed_images: usize,
    pub native_raw_origins: usize,
    pub nonzero_native_rows: usize,
    pub raw_graph_terms: usize,
    pub native_expansion_incidences: usize,
    pub moment_power_classes: usize,
    pub algebraic_candidates: usize,
    pub algebraic_rows: usize,
    pub boundary_rows: usize,
    pub source_rows: usize,
    pub symmetry_images: usize,
    pub global_columns: usize,
    pub collected_entries: usize,
    pub mass_power_steps: usize,
    pub modular_updates: usize,
    pub typed_rhs_entries: usize,
    pub typed_trace_reductions: usize,
}

#[derive(Clone, Debug)]
pub struct FiveLoopBananaD4Shell {
    config: FiveLoopBananaD4Config,
    d3: FiveLoopBananaD3Shell,
    qd_context: CoefficientContext,
    seeds: Vec<FiveLoopBananaD4Seed>,
    rows: Vec<FiveLoopBananaD4SourceRow>,
    boundary_closures: Vec<FiveLoopBananaD4BoundaryClosure>,
    columns: Vec<FiveLoopBananaD4Column>,
    pivots: Vec<FiveLoopBananaD4PivotRule>,
    free_columns: Vec<FiveLoopBananaD4Column>,
    targets: [FiveLoopBananaD4Column; FIVE_LOOP_BANANA_D4_TARGET_ORBITS],
    d4_candidate_terminal: Integral,
    exact: ExactSparseElimination,
    stats: FiveLoopBananaD4Stats,
    checksum: u64,
}

impl FiveLoopBananaD4Shell {
    pub const SCHEMA: &'static str = D4_SCHEMA;

    pub fn build(config: FiveLoopBananaD4Config) -> Result<Self, FiveLoopBananaD4Error> {
        Self::new(equal_mass_five_loop_banana()?, config)
    }

    pub fn new(
        family: VacuumFamily,
        config: FiveLoopBananaD4Config,
    ) -> Result<Self, FiveLoopBananaD4Error> {
        preflight_config(config)?;
        let shell = build_once(family, config)?;
        shell.replay()?;
        Ok(shell)
    }

    pub const fn config(&self) -> FiveLoopBananaD4Config { self.config }
    pub fn family(&self) -> &VacuumFamily { self.d3.family() }
    pub fn d3_shell(&self) -> &FiveLoopBananaD3Shell { &self.d3 }
    pub fn boundary(&self) -> &FiveLoopBananaBoundaryReducer { self.d3.boundary() }
    pub fn qd_context(&self) -> &CoefficientContext { &self.qd_context }
    pub fn seeds(&self) -> &[FiveLoopBananaD4Seed] { &self.seeds }
    pub fn source_rows(&self) -> &[FiveLoopBananaD4SourceRow] { &self.rows }
    pub fn boundary_closures(&self) -> &[FiveLoopBananaD4BoundaryClosure] { &self.boundary_closures }
    pub fn columns(&self) -> &[FiveLoopBananaD4Column] { &self.columns }
    pub fn pivots(&self) -> &[FiveLoopBananaD4PivotRule] { &self.pivots }
    pub fn free_columns(&self) -> &[FiveLoopBananaD4Column] { &self.free_columns }
    pub const fn targets(&self) -> &[FiveLoopBananaD4Column; FIVE_LOOP_BANANA_D4_TARGET_ORBITS] { &self.targets }
    pub fn rank(&self) -> usize { self.pivots.len() }
    pub const fn stats(&self) -> FiveLoopBananaD4Stats { self.stats }
    pub const fn exact_stats(&self) -> ExactSparseEliminationStats { self.exact.stats() }
    pub const fn source_checksum(&self) -> u64 { self.exact.source_checksum() }
    pub const fn exact_checksum(&self) -> u64 { self.exact.checksum() }
    pub const fn checksum(&self) -> u64 { self.checksum }

    /// Stable `[2,2,2,2,1,1]` terminal of this fixed shell.  Its free status
    /// does not assert unrestricted irreducibility.
    pub fn d4_candidate_terminal(&self) -> &Integral { &self.d4_candidate_terminal }

    /// Normal form in the mass-normalized `Q(d)` column basis.
    pub fn reduce_normalized_column(
        &self,
        column: &FiveLoopBananaD4Column,
    ) -> Result<BTreeMap<FiveLoopBananaD4Column, Coefficient>, FiveLoopBananaD4Error> {
        if self.columns.binary_search(column).is_err() {
            return Err(FiveLoopBananaD4Error::ColumnOutsideCertifiedShell {
                column: column.clone(),
            });
        }
        let rules = self
            .pivots
            .iter()
            .map(|rule| (rule.pivot.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        reduce_qd_by_rules(
            BTreeMap::from([(column.clone(), self.qd_context.one())]),
            &rules,
        )
    }

    /// Public scalar reduction through total physical dot degree four.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FiveLoopBananaD4Error> {
        let physical = validate_scalar_input(integral)?;
        let dot_degree = exact_dot_degree(&physical);
        if dot_degree > 4 {
            return Err(FiveLoopBananaD4Error::OutOfCoverage {
                integral: integral.clone(),
                dot_degree,
                maximum: 4,
            });
        }
        let active = physical.iter().filter(|power| **power > 0).count();
        if active < FIVE_LOOP_BANANA_PHYSICAL_LINES {
            return Ok(self.boundary().reduce_integral(integral)?);
        }
        if dot_degree <= 3 {
            return Ok(self.d3.reduce_integral(integral)?);
        }

        let canonical = *five_loop_banana_physical_orbit_witness(physical).canonical();
        let target = FiveLoopBananaD4Column::Scalar { powers: canonical };
        let target_weight = target.mass_weight();
        let normalized = self.reduce_normalized_column(&target)?;
        let mass = self
            .family()
            .coefficients()
            .parameter("m2")
            .ok_or(FiveLoopBananaD4Error::MissingParameter { name: "m2" })?;
        let mut output = LinearCombination::new();
        for (column, qd_coefficient) in normalized {
            let terminal = match &column {
                FiveLoopBananaD4Column::BoundaryTerminal => self.boundary().product_master().clone(),
                FiveLoopBananaD4Column::Scalar { powers } => scalar_integral(*powers),
                other => return Err(FiveLoopBananaD4Error::UnexpectedPublicTerminal { column: other.clone() }),
            };
            let coefficient = lift_qd_coefficient(
                &qd_coefficient,
                &self.qd_context,
                self.family().coefficients(),
            )?;
            let coefficient = apply_mass_power(
                coefficient,
                column.mass_weight() - target_weight,
                &mass,
                None,
            )?;
            output.add_term(terminal, coefficient);
        }
        Ok(output)
    }

    /// Rebuild all native/algebraic/boundary rows, repeat modular discovery,
    /// and replay the exact certificate against the regenerated projected rows.
    pub fn replay(&self) -> Result<(), FiveLoopBananaD4Error> {
        let replay = build_once(self.family().clone(), self.config)?;
        if replay.seeds != self.seeds
            || replay.rows != self.rows
            || replay.boundary_closures != self.boundary_closures
            || replay.columns != self.columns
            || replay.pivots != self.pivots
            || replay.free_columns != self.free_columns
            || replay.targets != self.targets
            || replay.d4_candidate_terminal != self.d4_candidate_terminal
            || replay.stats != self.stats
            || replay.checksum != self.checksum
            || replay.exact.source_checksum() != self.exact.source_checksum()
            || replay.exact.checksum() != self.exact.checksum()
        {
            return Err(FiveLoopBananaD4Error::CertificateReplayMismatch);
        }
        let indexed = index_source_rows(&replay.rows, &replay.columns)?;
        self.exact.replay(&replay.qd_context, &indexed)?;
        Ok(())
    }
}

fn build_once(
    family: VacuumFamily,
    config: FiveLoopBananaD4Config,
) -> Result<FiveLoopBananaD4Shell, FiveLoopBananaD4Error> {
    preflight_config(config)?;
    let d3 = FiveLoopBananaD3Shell::new(family.clone(), config.d3)?;
    if d3.rank() != FIVE_LOOP_BANANA_D3_RANK || d3.free_columns().len() != 3 {
        return Err(FiveLoopBananaD4Error::D3CrossAuthentication {
            rank: d3.rank(),
            free_columns: d3.free_columns().len(),
        });
    }
    ShellBuilder::new(family, config, d3)?.build()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphColumn {
    powers: [i32; 6],
    edge: Option<[u8; 2]>,
}

impl GraphColumn {
    fn active_lines(&self) -> usize { self.powers.iter().filter(|power| **power > 0).count() }
    fn dot_degree(&self) -> u64 {
        self.powers.iter().map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap()).sum()
    }
    fn order_key(&self) -> (usize, u64, u64, [i32; 6], Option<[u8; 2]>) {
        let numerator = u64::from(self.edge.is_some());
        (self.active_lines(), self.dot_degree() + numerator, self.dot_degree(), self.powers, self.edge)
    }
}
impl Ord for GraphColumn {
    fn cmp(&self, other: &Self) -> Ordering { self.order_key().cmp(&other.order_key()) }
}
impl PartialOrd for GraphColumn {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
type GraphRow = BTreeMap<GraphColumn, Coefficient>;

#[derive(Clone)]
struct Permutations {
    values: Vec<([usize; 6], [usize; 6])>,
}

impl Permutations {
    fn new() -> Self {
        let mut source = [0, 1, 2, 3, 4, 5];
        let mut values = Vec::with_capacity(FIVE_LOOP_BANANA_S6_ORDER);
        loop {
            let mut inverse = [0; 6];
            for (target, value) in source.into_iter().enumerate() { inverse[value] = target; }
            values.push((source, inverse));
            if !next_permutation(&mut source) { break; }
        }
        Self { values }
    }

    fn canonical(&self, column: &GraphColumn) -> GraphColumn {
        self.values
            .iter()
            .map(|(source, inverse)| GraphColumn {
                powers: std::array::from_fn(|target| column.powers[source[target]]),
                edge: column.edge.map(|edge| normalized_edge(inverse[usize::from(edge[0])], inverse[usize::from(edge[1])])),
            })
            .max()
            .expect("S6 contains the identity")
    }

    fn labelled_scalar_orbit_size(&self, powers: [i32; 6]) -> usize {
        self.values
            .iter()
            .map(|(source, _)| std::array::from_fn::<_, 6, _>(|target| powers[source[target]]))
            .collect::<BTreeSet<_>>()
            .len()
    }
}

#[derive(Clone)]
struct UnprojectedRow {
    row_id: FiveLoopBananaD4RowId,
    mass_weight: i32,
    entries: BTreeMap<FiveLoopBananaD4Column, Coefficient>,
}

struct CollectedRows {
    seeds: Vec<FiveLoopBananaD4Seed>,
    rows: Vec<UnprojectedRow>,
    boundary_closures: Vec<FiveLoopBananaD4BoundaryClosure>,
    stats: FiveLoopBananaD4Stats,
}

struct ShellBuilder {
    config: FiveLoopBananaD4Config,
    boundary: FiveLoopBananaBoundaryReducer,
    qd_context: CoefficientContext,
    permutations: Permutations,
    d3: FiveLoopBananaD3Shell,
}

impl ShellBuilder {
    fn new(
        family: VacuumFamily,
        config: FiveLoopBananaD4Config,
        d3: FiveLoopBananaD3Shell,
    ) -> Result<Self, FiveLoopBananaD4Error> {
        Ok(Self {
            boundary: FiveLoopBananaBoundaryReducer::new(family, config.d3.boundary)?,
            qd_context: CoefficientContext::new(["d"]),
            permutations: Permutations::new(),
            config,
            d3,
        })
    }

    fn build(self) -> Result<FiveLoopBananaD4Shell, FiveLoopBananaD4Error> {
        let mut collected = self.collect_rows()?;
        let columns = collected
            .rows
            .iter()
            .flat_map(|row| row.entries.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        collected.stats.global_columns = columns.len();
        check_resource("global columns", columns.len(), self.config.max_global_columns)?;
        require_count("global columns", FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS, columns.len())?;

        let (rows, mass_power_steps) = self.project_rows(collected.rows, &columns)?;
        collected.stats.mass_power_steps = mass_power_steps;
        collected.stats.source_rows = rows.len();
        collected.stats.collected_entries = rows.iter().map(|row| row.entries.len()).sum();
        check_resource("source rows", rows.len(), self.config.max_source_rows)?;
        check_resource("collected entries", collected.stats.collected_entries, self.config.max_collected_entries)?;
        check_resource("mass-power steps", mass_power_steps, self.config.max_mass_power_steps)?;
        require_count("source rows", FIVE_LOOP_BANANA_D4_SOURCE_ROWS, rows.len())?;
        require_count("collected entries", FIVE_LOOP_BANANA_D4_COLLECTED_ENTRIES, collected.stats.collected_entries)?;

        let indexed = index_source_rows(&rows, &columns)?;
        let (skeleton, modular_updates) = discover_common_modular_skeleton(
            &indexed,
            FIVE_LOOP_BANANA_D4_GLOBAL_COLUMNS,
            self.config.max_modular_updates,
        )?;
        collected.stats.modular_updates = modular_updates;
        if skeleton.len() != FIVE_LOOP_BANANA_D4_RANK {
            return Err(FiveLoopBananaD4Error::RankMismatch {
                stage: "modular proposal",
                expected: FIVE_LOOP_BANANA_D4_RANK,
                actual: skeleton.len(),
            });
        }
        let exact = ExactSparseElimination::build(
            &self.qd_context,
            &indexed,
            columns.len(),
            &skeleton,
            self.config.exact,
        )?;
        if exact.rank() != FIVE_LOOP_BANANA_D4_RANK {
            return Err(FiveLoopBananaD4Error::RankMismatch {
                stage: "exact proof",
                expected: FIVE_LOOP_BANANA_D4_RANK,
                actual: exact.rank(),
            });
        }

        let (pivots, typed_rhs_entries, typed_trace_reductions) = project_pivots(
            &exact,
            &rows,
            &columns,
            self.config,
        )?;
        collected.stats.typed_rhs_entries = typed_rhs_entries;
        collected.stats.typed_trace_reductions = typed_trace_reductions;
        let free_columns = exact.free_columns().iter().map(|index| columns[*index].clone()).collect::<Vec<_>>();
        let expected_free = vec![
            FiveLoopBananaD4Column::BoundaryTerminal,
            FiveLoopBananaD4Column::Scalar { powers: [1, 1, 1, 1, 1, 1] },
            FiveLoopBananaD4Column::Scalar { powers: [2, 2, 1, 1, 1, 1] },
            FiveLoopBananaD4Column::Scalar { powers: [2, 2, 2, 2, 1, 1] },
        ];
        if free_columns != expected_free {
            return Err(FiveLoopBananaD4Error::FreeColumnMismatch {
                expected: expected_free,
                actual: free_columns,
            });
        }
        let targets = [
            [5, 1, 1, 1, 1, 1],
            [4, 2, 1, 1, 1, 1],
            [3, 3, 1, 1, 1, 1],
            [3, 2, 2, 1, 1, 1],
            [2, 2, 2, 2, 1, 1],
        ].map(|powers| FiveLoopBananaD4Column::Scalar { powers });
        let pivot_set = pivots.iter().map(|rule| rule.pivot.clone()).collect::<BTreeSet<_>>();
        for target in &targets[..4] {
            if !pivot_set.contains(target) {
                return Err(FiveLoopBananaD4Error::MissingTargetPivot { target: target.clone() });
            }
        }
        if pivot_set.contains(&targets[4]) {
            return Err(FiveLoopBananaD4Error::CandidateUnexpectedlyPivoted);
        }

        let checksum = shell_checksum(
            &columns,
            &rows,
            &pivots,
            &free_columns,
            exact.source_checksum(),
            exact.checksum(),
        );
        Ok(FiveLoopBananaD4Shell {
            config: self.config,
            d3: self.d3,
            qd_context: self.qd_context,
            seeds: collected.seeds,
            rows,
            boundary_closures: collected.boundary_closures,
            columns,
            pivots,
            free_columns,
            targets,
            d4_candidate_terminal: scalar_integral([2, 2, 2, 2, 1, 1]),
            exact,
            stats: collected.stats,
            checksum,
        })
    }
}
