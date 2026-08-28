//! Replayable scalar `D<=3` shell for the equal-mass five-loop banana.
//!
//! Four scalar `S6` seed orbits (`M`, `A`, `A2`, and `B2`) authenticate all
//! one hundred native `d/dk_i.k_j` origins.  Their oriented-line form is kept
//! in a disjoint scalar/one-moment column universe and closed by explicit
//! `l_i.l_i=D_i-m2`, `sum_j l_i.l_j=0`, and certified five-line boundary
//! rows.  Deterministic sparse elimination is exact over Symbolica-backed
//! `Q(d,m2)` and retains source-row weights for complete replay.
//!
//! The finite shell proves the displayed `D=3` reductions inside this seed
//! box.  `B2` is deliberately named a candidate terminal: this module does
//! not claim unrestricted master minimality or coverage above dot degree 3.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::five_loop::equal_mass_five_loop_banana;
use crate::five_loop_boundary::{
    FIVE_LOOP_BANANA_DENOMINATORS, FIVE_LOOP_BANANA_LOOP_MOMENTA, FIVE_LOOP_BANANA_PHYSICAL_LINES,
    FIVE_LOOP_BANANA_S6_ORDER, FiveLoopBananaBoundaryConfig, FiveLoopBananaBoundaryError,
    FiveLoopBananaBoundaryReducer, five_loop_banana_oriented_line_routing,
};
use crate::{
    FamilyError, IbpGenerationError, IbpGenerator, Integral, LinearCombination, VacuumFamily,
};
use rustred::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use rustred::{
    Coefficient, CoefficientContext, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, canonical_symbolica_atom,
};

pub const FIVE_LOOP_BANANA_D3_SEED_ORBITS: usize = 4;
pub const FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS: usize =
    FIVE_LOOP_BANANA_D3_SEED_ORBITS * FIVE_LOOP_BANANA_LOOP_MOMENTA * FIVE_LOOP_BANANA_LOOP_MOMENTA;
pub const FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS: usize = 36;
pub const FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES: usize = 6;
pub const FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS: usize = 26;
pub const FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS: usize = 6;
pub const FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS: usize = 43;
pub const FIVE_LOOP_BANANA_D3_RANK: usize = 40;

/// The oriented native formula has at most a divergence term and two
/// denominator-derivative moment terms per origin.
pub const FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND: usize =
    FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS * 3;
/// Each moment expands into one constant and at most all fifteen basis
/// denominators when checked against the independently generated native row.
pub const FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND: usize =
    FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS * (1 + 2 * (1 + FIVE_LOOP_BANANA_DENOMINATORS));
/// The fixed raw halo has exactly six distinct marked-power classes.
/// Each emits six diagonal and six momentum candidates.
pub const FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND: usize =
    FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES * 12;
/// Raw collection uses at most 220 additions; the algebraic candidates use at
/// most `6*6*(3+6)`.  Every joint orientation examines all 720 permutations.
pub const FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND: usize =
    (220 + FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES * 6 * 9) * FIVE_LOOP_BANANA_S6_ORDER;
pub const FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND: usize = 512;
pub const FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND: usize = 4_000_000;
pub const FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND: usize = FIVE_LOOP_BANANA_D3_RANK
    * (FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS
        + FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS
        + FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3Config {
    pub boundary: FiveLoopBananaBoundaryConfig,
    pub max_seed_orbits: usize,
    pub max_native_raw_origins: usize,
    pub max_raw_graph_terms: usize,
    pub max_native_expansion_incidences: usize,
    pub max_algebraic_candidates: usize,
    pub max_algebraic_rows: usize,
    pub max_boundary_rows: usize,
    pub max_symmetry_images: usize,
    pub max_global_columns: usize,
    pub max_collected_nonzeros: usize,
    pub max_elimination_updates: usize,
    pub max_source_row_weights: usize,
    pub max_coefficient_degree: usize,
}

impl Default for FiveLoopBananaD3Config {
    fn default() -> Self {
        Self {
            boundary: FiveLoopBananaBoundaryConfig::default(),
            max_seed_orbits: FIVE_LOOP_BANANA_D3_SEED_ORBITS,
            max_native_raw_origins: FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS,
            max_raw_graph_terms: FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND,
            max_native_expansion_incidences: FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND,
            max_algebraic_candidates: FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND,
            max_algebraic_rows: FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS,
            max_boundary_rows: FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS,
            max_symmetry_images: FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND,
            max_global_columns: FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS,
            max_collected_nonzeros: FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND,
            max_elimination_updates: FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND,
            max_source_row_weights: FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND,
            max_coefficient_degree: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD3SeedOrbit {
    M,
    A,
    A2,
    B2,
}

impl FiveLoopBananaD3SeedOrbit {
    pub const ALL: [Self; FIVE_LOOP_BANANA_D3_SEED_ORBITS] = [Self::M, Self::A, Self::A2, Self::B2];

    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::M => "rustred-five-loop-banana-d3-seed-v1:M",
            Self::A => "rustred-five-loop-banana-d3-seed-v1:A",
            Self::A2 => "rustred-five-loop-banana-d3-seed-v1:A2",
            Self::B2 => "rustred-five-loop-banana-d3-seed-v1:B2-candidate",
        }
    }

    pub const fn labelled_orbit_size(self) -> usize {
        match self {
            Self::M => 1,
            Self::A | Self::A2 => 6,
            Self::B2 => 15,
        }
    }

    const fn powers(self) -> [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        match self {
            Self::M => [1, 1, 1, 1, 1, 1],
            Self::A => [2, 1, 1, 1, 1, 1],
            Self::A2 => [3, 1, 1, 1, 1, 1],
            Self::B2 => [2, 2, 1, 1, 1, 1],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3Seed {
    orbit: FiveLoopBananaD3SeedOrbit,
    integral: Integral,
    labelled_orbit_size: usize,
}

impl FiveLoopBananaD3Seed {
    pub const fn orbit(&self) -> FiveLoopBananaD3SeedOrbit {
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
pub struct FiveLoopBananaD3NativeRowId {
    seed_orbit: FiveLoopBananaD3SeedOrbit,
    differentiated_loop: u8,
    contraction_loop: u8,
}

impl FiveLoopBananaD3NativeRowId {
    pub const SCHEMA: &'static str = "rustred-five-loop-banana-d3-native-row-v1";

    pub const fn new(
        seed_orbit: FiveLoopBananaD3SeedOrbit,
        differentiated_loop: u8,
        contraction_loop: u8,
    ) -> Self {
        Self {
            seed_orbit,
            differentiated_loop,
            contraction_loop,
        }
    }

    pub const fn seed_orbit(self) -> FiveLoopBananaD3SeedOrbit {
        self.seed_orbit
    }

    pub const fn differentiated_loop(self) -> u8 {
        self.differentiated_loop
    }

    pub const fn contraction_loop(self) -> u8 {
        self.contraction_loop
    }

    pub fn stable_key(self) -> String {
        format!(
            "{}:{}:d{}:k{}",
            Self::SCHEMA,
            self.seed_orbit.stable_key(),
            self.differentiated_loop,
            self.contraction_loop
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD3AlgebraicKind {
    Diagonal,
    Momentum,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiveLoopBananaD3AlgebraicRowId {
    index: u16,
    kind: FiveLoopBananaD3AlgebraicKind,
    powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    marked_line: u8,
}

impl FiveLoopBananaD3AlgebraicRowId {
    pub const SCHEMA: &'static str = "rustred-five-loop-banana-d3-algebra-row-v1";

    pub const fn index(self) -> u16 {
        self.index
    }

    pub const fn kind(self) -> FiveLoopBananaD3AlgebraicKind {
        self.kind
    }

    pub const fn powers(self) -> [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        self.powers
    }

    pub const fn marked_line(self) -> u8 {
        self.marked_line
    }

    pub fn stable_key(self) -> String {
        format!(
            "{}:{}:{:?}:[{},{},{},{},{},{}]:l{}",
            Self::SCHEMA,
            self.index,
            self.kind,
            self.powers[0],
            self.powers[1],
            self.powers[2],
            self.powers[3],
            self.powers[4],
            self.powers[5],
            self.marked_line
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FiveLoopBananaD3BoundaryRowId {
    index: u8,
    powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
}

impl FiveLoopBananaD3BoundaryRowId {
    pub const SCHEMA: &'static str = "rustred-five-loop-banana-d3-boundary-row-v1";

    pub const fn index(self) -> u8 {
        self.index
    }

    pub const fn powers(self) -> [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        self.powers
    }

    pub fn stable_key(self) -> String {
        format!(
            "{}:{}:[{},{},{},{},{},{}]",
            Self::SCHEMA,
            self.index,
            self.powers[0],
            self.powers[1],
            self.powers[2],
            self.powers[3],
            self.powers[4],
            self.powers[5]
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FiveLoopBananaD3RowId {
    Native(FiveLoopBananaD3NativeRowId),
    Algebraic(FiveLoopBananaD3AlgebraicRowId),
    Boundary(FiveLoopBananaD3BoundaryRowId),
}

impl FiveLoopBananaD3RowId {
    pub fn stable_key(self) -> String {
        match self {
            Self::Native(id) => id.stable_key(),
            Self::Algebraic(id) => id.stable_key(),
            Self::Boundary(id) => id.stable_key(),
        }
    }
}

/// Stable, disjoint finite-shell column IDs.  A marked edge `[u,v]` denotes
/// the oriented scalar product `l_u.l_v`; loops `[u,u]` are retained until an
/// explicit diagonal algebra row removes them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FiveLoopBananaD3Column {
    BoundaryTerminal,
    ProperBoundary {
        powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    },
    Scalar {
        powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    },
    OneMoment {
        powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
        edge: [u8; 2],
    },
}

impl FiveLoopBananaD3Column {
    pub const SCHEMA: &'static str = "rustred-five-loop-banana-d3-column-v1";

    pub fn stable_key(&self) -> String {
        let powers = match self {
            Self::BoundaryTerminal => {
                return format!("{}:boundary:T1^5", Self::SCHEMA);
            }
            Self::ProperBoundary { powers }
            | Self::Scalar { powers }
            | Self::OneMoment { powers, .. } => powers,
        };
        let prefix = match self {
            Self::ProperBoundary { .. } => "proper-boundary",
            Self::Scalar { .. } => "scalar",
            Self::OneMoment { .. } => "one-moment",
            Self::BoundaryTerminal => unreachable!(),
        };
        let suffix = match self {
            Self::OneMoment { edge, .. } => format!(":edge[{},{}]", edge[0], edge[1]),
            _ => String::new(),
        };
        format!(
            "{}:{}:[{},{},{},{},{},{}]{}",
            Self::SCHEMA,
            prefix,
            powers[0],
            powers[1],
            powers[2],
            powers[3],
            powers[4],
            powers[5],
            suffix
        )
    }

    pub fn active_lines(&self) -> usize {
        match self {
            Self::BoundaryTerminal => 5,
            Self::ProperBoundary { powers }
            | Self::Scalar { powers }
            | Self::OneMoment { powers, .. } => powers.iter().filter(|power| **power > 0).count(),
        }
    }

    pub fn dot_degree(&self) -> u64 {
        match self {
            Self::BoundaryTerminal => 0,
            Self::ProperBoundary { powers }
            | Self::Scalar { powers }
            | Self::OneMoment { powers, .. } => powers
                .iter()
                .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
                .sum(),
        }
    }

    pub const fn numerator_degree(&self) -> u64 {
        match self {
            Self::OneMoment { .. } => 1,
            _ => 0,
        }
    }

    fn order_key(&self) -> (usize, u64, u64, u8, [i32; 6], [u8; 2]) {
        match self {
            Self::BoundaryTerminal => (5, 0, 0, 0, [0; 6], [0; 2]),
            Self::ProperBoundary { powers } => {
                (5, self.dot_degree(), self.dot_degree(), 1, *powers, [0; 2])
            }
            Self::Scalar { powers } => {
                (6, self.dot_degree(), self.dot_degree(), 0, *powers, [0; 2])
            }
            Self::OneMoment { powers, edge } => (
                6,
                self.dot_degree() + 1,
                self.dot_degree(),
                1,
                *powers,
                *edge,
            ),
        }
    }
}

impl Ord for FiveLoopBananaD3Column {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}

impl PartialOrd for FiveLoopBananaD3Column {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3BoundaryClosure {
    row_id: FiveLoopBananaD3BoundaryRowId,
    input: Integral,
    reduction: LinearCombination,
}

impl FiveLoopBananaD3BoundaryClosure {
    pub const fn row_id(&self) -> FiveLoopBananaD3BoundaryRowId {
        self.row_id
    }

    pub const fn input(&self) -> &Integral {
        &self.input
    }

    pub const fn reduction(&self) -> &LinearCombination {
        &self.reduction
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3NormalizedRow {
    row_id: FiveLoopBananaD3RowId,
    row_scale: Coefficient,
    entries: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
}

impl FiveLoopBananaD3NormalizedRow {
    pub const fn row_id(&self) -> FiveLoopBananaD3RowId {
        self.row_id
    }

    pub const fn row_scale(&self) -> &Coefficient {
        &self.row_scale
    }

    pub const fn entries(&self) -> &BTreeMap<FiveLoopBananaD3Column, Coefficient> {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3PivotRule {
    pivot: FiveLoopBananaD3Column,
    rhs: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
    source_row_weights: BTreeMap<FiveLoopBananaD3RowId, Coefficient>,
}

impl FiveLoopBananaD3PivotRule {
    pub const fn pivot(&self) -> &FiveLoopBananaD3Column {
        &self.pivot
    }

    pub const fn rhs(&self) -> &BTreeMap<FiveLoopBananaD3Column, Coefficient> {
        &self.rhs
    }

    pub const fn source_row_weights(&self) -> &BTreeMap<FiveLoopBananaD3RowId, Coefficient> {
        &self.source_row_weights
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiveLoopBananaD3ConditionSource {
    GenericMassDomain,
    Row(FiveLoopBananaD3RowId),
    Pivot(FiveLoopBananaD3Column),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD3NonzeroCondition {
    source: FiveLoopBananaD3ConditionSource,
    polynomial: String,
}

impl FiveLoopBananaD3NonzeroCondition {
    pub const fn source(&self) -> &FiveLoopBananaD3ConditionSource {
        &self.source
    }

    pub fn polynomial(&self) -> &str {
        &self.polynomial
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FiveLoopBananaD3Stats {
    pub seed_orbits: usize,
    pub native_raw_origins: usize,
    pub nonzero_native_rows: usize,
    pub raw_graph_terms: usize,
    pub native_expansion_incidences: usize,
    pub moment_power_classes: usize,
    pub algebraic_candidates: usize,
    pub algebraic_rows: usize,
    pub boundary_rows: usize,
    pub symmetry_images: usize,
    pub global_columns: usize,
    pub collected_nonzeros: usize,
    pub elimination_updates: usize,
    pub source_row_weights: usize,
}

#[derive(Clone, Debug)]
pub struct FiveLoopBananaD3Shell {
    config: FiveLoopBananaD3Config,
    boundary: FiveLoopBananaBoundaryReducer,
    seeds: Vec<FiveLoopBananaD3Seed>,
    rows: Vec<FiveLoopBananaD3NormalizedRow>,
    boundary_closures: Vec<FiveLoopBananaD3BoundaryClosure>,
    pivots: Vec<FiveLoopBananaD3PivotRule>,
    free_columns: Vec<FiveLoopBananaD3Column>,
    nonzero_conditions: Vec<FiveLoopBananaD3NonzeroCondition>,
    targets: [FiveLoopBananaD3Column; 3],
    d2_candidate_terminal: Integral,
    stats: FiveLoopBananaD3Stats,
}

impl FiveLoopBananaD3Shell {
    pub const SCHEMA: &'static str = "rustred-five-loop-banana-d3-shell-v1";

    pub fn build(config: FiveLoopBananaD3Config) -> Result<Self, FiveLoopBananaD3Error> {
        preflight_config(config)?;
        Self::new(equal_mass_five_loop_banana()?, config)
    }

    pub fn new(
        family: VacuumFamily,
        config: FiveLoopBananaD3Config,
    ) -> Result<Self, FiveLoopBananaD3Error> {
        preflight_config(config)?;
        let shell = ShellBuilder::new(family, config)?.build()?;
        shell.replay()?;
        Ok(shell)
    }

    pub const fn config(&self) -> FiveLoopBananaD3Config {
        self.config
    }

    pub fn family(&self) -> &VacuumFamily {
        self.boundary.family()
    }

    pub fn boundary(&self) -> &FiveLoopBananaBoundaryReducer {
        &self.boundary
    }

    pub fn seeds(&self) -> &[FiveLoopBananaD3Seed] {
        &self.seeds
    }

    pub fn normalized_rows(&self) -> &[FiveLoopBananaD3NormalizedRow] {
        &self.rows
    }

    pub fn boundary_closures(&self) -> &[FiveLoopBananaD3BoundaryClosure] {
        &self.boundary_closures
    }

    pub fn pivots(&self) -> &[FiveLoopBananaD3PivotRule] {
        &self.pivots
    }

    pub fn rank(&self) -> usize {
        self.pivots.len()
    }

    pub fn free_columns(&self) -> &[FiveLoopBananaD3Column] {
        &self.free_columns
    }

    pub fn nonzero_conditions(&self) -> &[FiveLoopBananaD3NonzeroCondition] {
        &self.nonzero_conditions
    }

    pub const fn target_columns(&self) -> &[FiveLoopBananaD3Column; 3] {
        &self.targets
    }

    pub const fn stats(&self) -> FiveLoopBananaD3Stats {
        self.stats
    }

    /// Stable `B2=[2,2,1,1,1,1]` candidate terminal.  Being free in this
    /// finite shell is not an unrestricted non-reducibility theorem.
    pub fn d2_candidate_terminal(&self) -> &Integral {
        &self.d2_candidate_terminal
    }

    pub fn reduce_target(
        &self,
        integral: &Integral,
    ) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
        let column = classify_top_scalar(integral)?;
        if column.dot_degree() > 3 {
            return Err(FiveLoopBananaD3Error::OutOfCoverage {
                integral: integral.clone(),
                dot_degree: column.dot_degree(),
                maximum: 3,
            });
        }
        let column = canonical_public_scalar(&column, &Permutations::new());
        self.reduce_column(&column)
    }

    /// Reduce one typed column only when it belongs to the exact reconstructed
    /// 43-column census.  An absent column is outside this finite certificate,
    /// not an additional free terminal.
    pub fn reduce_column(
        &self,
        column: &FiveLoopBananaD3Column,
    ) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
        if !self.rows.iter().any(|row| row.entries.contains_key(column)) {
            return Err(FiveLoopBananaD3Error::ColumnOutsideCertifiedShell {
                column: column.clone(),
            });
        }
        self.reduce_column_unchecked(column)
    }

    /// Reduce a column already proved to belong to this shell.  Keeping this
    /// path private prevents an arbitrary absent column from being returned as
    /// an accidental free terminal by the triangular normal-form engine.
    fn reduce_column_unchecked(
        &self,
        column: &FiveLoopBananaD3Column,
    ) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
        let rules = self
            .pivots
            .iter()
            .map(|rule| (rule.pivot.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        reduce_by_rules(
            &Arithmetic::new(self.family().coefficients(), self.config)?,
            BTreeMap::from([(column.clone(), self.family().coefficients().one())]),
            &rules,
        )
    }

    /// Public scalar `D<=3` reduction.  Proper sectors are delegated to the
    /// exact five-line boundary reducer; top outputs contain only `M` and the
    /// explicitly named `B2` candidate terminal.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FiveLoopBananaD3Error> {
        validate_scalar_input(integral)?;
        let active = integral.powers()[..FIVE_LOOP_BANANA_PHYSICAL_LINES]
            .iter()
            .filter(|power| **power > 0)
            .count();
        if active < FIVE_LOOP_BANANA_PHYSICAL_LINES {
            return Ok(self.boundary.reduce_integral(integral)?);
        }
        let reduced = self.reduce_target(integral)?;
        let mut output = LinearCombination::new();
        for (column, coefficient) in reduced {
            let terminal = match column {
                FiveLoopBananaD3Column::BoundaryTerminal => self.boundary.product_master().clone(),
                FiveLoopBananaD3Column::Scalar { powers } => scalar_integral(powers),
                other => {
                    return Err(FiveLoopBananaD3Error::UnexpectedPublicTerminal { column: other });
                }
            };
            output.add_term(terminal, coefficient);
        }
        Ok(output)
    }

    /// Regenerate every native and algebraic identity, independently replay
    /// all boundary witnesses, and reconstruct each pivot from its exact
    /// source-row weights.
    pub fn replay(&self) -> Result<(), FiveLoopBananaD3Error> {
        // Rebuild the complete deterministic certificate, not only the input
        // rows.  The free set, exceptional locus, and pivot provenance are
        // semantic outputs: stale metadata there would misstate this finite
        // shell even when every stored row happened to reduce to zero.
        let replay = ShellBuilder::new(self.family().clone(), self.config)?.build()?;
        if replay.seeds != self.seeds
            || replay.rows != self.rows
            || replay.boundary_closures != self.boundary_closures
            || replay.pivots != self.pivots
            || replay.free_columns != self.free_columns
            || replay.nonzero_conditions != self.nonzero_conditions
            || replay.targets != self.targets
            || replay.d2_candidate_terminal != self.d2_candidate_terminal
            || replay.stats != self.stats
        {
            return Err(FiveLoopBananaD3Error::CertificateReplayMismatch);
        }
        let arithmetic = Arithmetic::new(self.family().coefficients(), self.config)?;
        let rules = self
            .pivots
            .iter()
            .map(|rule| (rule.pivot.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        for row in &self.rows {
            let remainder = reduce_by_rules(&arithmetic, row.entries.clone(), &rules)?;
            if !remainder.is_empty() {
                return Err(FiveLoopBananaD3Error::NormalizedRowRemainder {
                    row_id: row.row_id,
                    remainder,
                });
            }
        }
        let rows = self
            .rows
            .iter()
            .map(|row| (row.row_id, row))
            .collect::<BTreeMap<_, _>>();
        for rule in &self.pivots {
            let mut actual = BTreeMap::new();
            for (row_id, weight) in &rule.source_row_weights {
                let row = rows
                    .get(row_id)
                    .ok_or(FiveLoopBananaD3Error::UnknownSourceRow { row_id: *row_id })?;
                for (column, coefficient) in &row.entries {
                    let scaled = arithmetic.checked_mul(coefficient, weight)?;
                    arithmetic.add_sparse(&mut actual, column.clone(), scaled)?;
                }
            }
            let mut expected =
                BTreeMap::from([(rule.pivot.clone(), self.family().coefficients().one())]);
            for (column, coefficient) in &rule.rhs {
                arithmetic.add_sparse(&mut expected, column.clone(), -coefficient.clone())?;
            }
            if actual != expected {
                return Err(FiveLoopBananaD3Error::PivotProvenanceMismatch {
                    pivot: rule.pivot.clone(),
                });
            }
            if rule.rhs.keys().any(|column| column >= &rule.pivot) {
                return Err(FiveLoopBananaD3Error::NonTriangularPivot {
                    pivot: rule.pivot.clone(),
                });
            }
        }
        self.verify_candidate_formulas()
    }

    fn verify_candidate_formulas(&self) -> Result<(), FiveLoopBananaD3Error> {
        for (position, target) in self.targets.iter().enumerate() {
            let actual = self.reduce_column_unchecked(target)?;
            let expected = expected_d3_formula(self.family().coefficients(), position)?;
            if actual != expected {
                return Err(FiveLoopBananaD3Error::CandidateFormulaMismatch {
                    target: target.clone(),
                    actual,
                    expected,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphColumn {
    powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    edge: Option<[u8; 2]>,
}

impl GraphColumn {
    fn active_lines(&self) -> usize {
        self.powers.iter().filter(|power| **power > 0).count()
    }

    fn dot_degree(&self) -> u64 {
        self.powers
            .iter()
            .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
            .sum()
    }

    fn order_key(&self) -> (usize, u64, u64, [i32; 6], Option<[u8; 2]>) {
        let numerator = u64::from(self.edge.is_some());
        (
            self.active_lines(),
            self.dot_degree() + numerator,
            self.dot_degree(),
            self.powers,
            self.edge,
        )
    }
}

impl Ord for GraphColumn {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}

impl PartialOrd for GraphColumn {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
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
            for (target, value) in source.into_iter().enumerate() {
                inverse[value] = target;
            }
            values.push((source, inverse));
            if !next_permutation(&mut source) {
                break;
            }
        }
        debug_assert_eq!(values.len(), FIVE_LOOP_BANANA_S6_ORDER);
        Self { values }
    }

    fn canonical(&self, column: &GraphColumn) -> GraphColumn {
        let mut best = None;
        for (source, inverse) in &self.values {
            let powers = std::array::from_fn(|target| column.powers[source[target]]);
            let edge = column.edge.map(|edge| {
                normalized_edge(inverse[usize::from(edge[0])], inverse[usize::from(edge[1])])
            });
            let candidate = GraphColumn { powers, edge };
            if best
                .as_ref()
                .is_none_or(|current: &GraphColumn| candidate.cmp(current) == Ordering::Greater)
            {
                best = Some(candidate);
            }
        }
        best.expect("S6 contains the identity")
    }

    fn labelled_scalar_orbit_size(&self, powers: [i32; 6]) -> usize {
        self.values
            .iter()
            .map(|(source, _)| {
                std::array::from_fn::<_, FIVE_LOOP_BANANA_PHYSICAL_LINES, _>(|target| {
                    powers[source[target]]
                })
            })
            .collect::<BTreeSet<_>>()
            .len()
    }
}

fn next_permutation(values: &mut [usize; 6]) -> bool {
    let Some(left) = (0..5).rfind(|position| values[*position] < values[*position + 1]) else {
        return false;
    };
    let right = (left + 1..6)
        .rfind(|position| values[left] < values[*position])
        .expect("a successor exists");
    values.swap(left, right);
    values[left + 1..].reverse();
    true
}

fn normalized_edge(left: usize, right: usize) -> [u8; 2] {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    [u8::try_from(left).unwrap(), u8::try_from(right).unwrap()]
}

struct CollectedRows {
    seeds: Vec<FiveLoopBananaD3Seed>,
    rows: Vec<FiveLoopBananaD3NormalizedRow>,
    boundary_closures: Vec<FiveLoopBananaD3BoundaryClosure>,
    conditions: Vec<FiveLoopBananaD3NonzeroCondition>,
    stats: FiveLoopBananaD3Stats,
}

struct ShellBuilder {
    config: FiveLoopBananaD3Config,
    boundary: FiveLoopBananaBoundaryReducer,
    arithmetic: Arithmetic,
    permutations: Permutations,
}

impl ShellBuilder {
    fn new(
        family: VacuumFamily,
        config: FiveLoopBananaD3Config,
    ) -> Result<Self, FiveLoopBananaD3Error> {
        let boundary = FiveLoopBananaBoundaryReducer::new(family, config.boundary)?;
        let arithmetic = Arithmetic::new(boundary.family().coefficients(), config)?;
        Ok(Self {
            config,
            boundary,
            arithmetic,
            permutations: Permutations::new(),
        })
    }

    fn build(self) -> Result<FiveLoopBananaD3Shell, FiveLoopBananaD3Error> {
        let collected = self.collect_rows()?;
        let targets = [
            self.scalar_column([4, 1, 1, 1, 1, 1])?,
            self.scalar_column([3, 2, 1, 1, 1, 1])?,
            self.scalar_column([2, 2, 2, 1, 1, 1])?,
        ];
        if targets.iter().collect::<BTreeSet<_>>().len() != 3 {
            return Err(FiveLoopBananaD3Error::TargetOrbitCollapse);
        }
        let (pivots, free_columns, conditions, mut stats) = eliminate(
            &self.arithmetic,
            self.config,
            &collected.rows,
            collected.conditions,
            collected.stats,
        )?;
        if pivots.len() != FIVE_LOOP_BANANA_D3_RANK {
            return Err(FiveLoopBananaD3Error::RankMismatch {
                expected: FIVE_LOOP_BANANA_D3_RANK,
                actual: pivots.len(),
            });
        }
        let pivot_columns = pivots
            .iter()
            .map(|rule| rule.pivot.clone())
            .collect::<BTreeSet<_>>();
        for target in &targets {
            if !pivot_columns.contains(target) {
                return Err(FiveLoopBananaD3Error::MissingTargetPivot {
                    target: target.clone(),
                });
            }
        }
        stats.source_row_weights = pivots
            .iter()
            .map(|rule| rule.source_row_weights.len())
            .sum();
        check_resource(
            "source-row provenance weights",
            stats.source_row_weights,
            self.config.max_source_row_weights,
        )?;
        let shell = FiveLoopBananaD3Shell {
            config: self.config,
            boundary: self.boundary,
            seeds: collected.seeds,
            rows: collected.rows,
            boundary_closures: collected.boundary_closures,
            pivots,
            free_columns,
            nonzero_conditions: conditions,
            targets,
            d2_candidate_terminal: scalar_integral([2, 2, 1, 1, 1, 1]),
            stats,
        };
        shell.verify_candidate_formulas()?;
        Ok(shell)
    }

    fn collect_rows(&self) -> Result<CollectedRows, FiveLoopBananaD3Error> {
        let mut stats = FiveLoopBananaD3Stats::default();
        let mut conditions = vec![FiveLoopBananaD3NonzeroCondition {
            source: FiveLoopBananaD3ConditionSource::GenericMassDomain,
            polynomial: canonical_symbolica_atom(&self.arithmetic.mass.numerator.to_expression()),
        }];
        let mut seeds = Vec::with_capacity(FIVE_LOOP_BANANA_D3_SEED_ORBITS);
        for orbit in FiveLoopBananaD3SeedOrbit::ALL {
            let powers = orbit.powers();
            let labelled_orbit_size = self.permutations.labelled_scalar_orbit_size(powers);
            if labelled_orbit_size != orbit.labelled_orbit_size() {
                return Err(FiveLoopBananaD3Error::SeedOrbitSizeMismatch {
                    orbit,
                    expected: orbit.labelled_orbit_size(),
                    actual: labelled_orbit_size,
                });
            }
            seeds.push(FiveLoopBananaD3Seed {
                orbit,
                integral: scalar_integral(powers),
                labelled_orbit_size,
            });
        }
        stats.seed_orbits = seeds.len();

        let mut rows = Vec::new();
        let mut canonical_raw_graph_rows = Vec::new();
        for seed in &seeds {
            for differentiated in 0..FIVE_LOOP_BANANA_LOOP_MOMENTA {
                for contracted in 0..FIVE_LOOP_BANANA_LOOP_MOMENTA {
                    stats.native_raw_origins = checked_add_resource(
                        "native raw origins",
                        stats.native_raw_origins,
                        1,
                        self.config.max_native_raw_origins,
                    )?;
                    let native_id = FiveLoopBananaD3NativeRowId::new(
                        seed.orbit,
                        u8::try_from(differentiated).unwrap(),
                        u8::try_from(contracted).unwrap(),
                    );
                    let graph = self.raw_graph_row(
                        seed.orbit.powers(),
                        differentiated,
                        contracted,
                        &mut stats,
                    )?;
                    self.authenticate_native_row(native_id, &seed.integral, &graph, &mut stats)?;
                    let graph = self.canonicalize_graph_row(&graph, &mut stats)?;
                    if !graph.is_empty() {
                        stats.nonzero_native_rows += 1;
                    }
                    canonical_raw_graph_rows.push(graph.clone());
                    let entries = self.graph_to_typed(&graph)?;
                    rows.push(self.normalized_row(
                        FiveLoopBananaD3RowId::Native(native_id),
                        entries,
                        &mut conditions,
                        &mut stats,
                    )?);
                }
            }
        }
        if stats.native_raw_origins != FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS {
            return Err(FiveLoopBananaD3Error::NativeOriginCount {
                expected: FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS,
                actual: stats.native_raw_origins,
            });
        }
        if stats.nonzero_native_rows != FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS {
            return Err(FiveLoopBananaD3Error::NonzeroNativeRowCount {
                expected: FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS,
                actual: stats.nonzero_native_rows,
            });
        }

        let algebraic = self.algebraic_rows(&canonical_raw_graph_rows, &mut stats)?;
        for (row_id, graph) in algebraic {
            let entries = self.graph_to_typed(&graph)?;
            rows.push(self.normalized_row(
                FiveLoopBananaD3RowId::Algebraic(row_id),
                entries,
                &mut conditions,
                &mut stats,
            )?);
        }

        let graph_columns = rows
            .iter()
            .flat_map(|row| row.entries.keys())
            .filter(|column| !matches!(column, FiveLoopBananaD3Column::BoundaryTerminal))
            .cloned()
            .collect::<BTreeSet<_>>();
        if graph_columns.len() != FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS - 1 {
            return Err(FiveLoopBananaD3Error::GraphColumnCount {
                expected: FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS - 1,
                actual: graph_columns.len(),
            });
        }
        let proper = graph_columns
            .iter()
            .filter_map(|column| match column {
                FiveLoopBananaD3Column::ProperBoundary { powers } => Some(*powers),
                _ => None,
            })
            .collect::<Vec<_>>();
        if proper.len() != FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS {
            return Err(FiveLoopBananaD3Error::BoundaryRowCount {
                expected: FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS,
                actual: proper.len(),
            });
        }
        let mut boundary_closures = Vec::new();
        for (index, powers) in proper.into_iter().enumerate() {
            let row_id = FiveLoopBananaD3BoundaryRowId {
                index: u8::try_from(index).unwrap(),
                powers,
            };
            let input = scalar_integral(powers);
            let reduction = self.boundary.reduce_integral(&input)?;
            if reduction.len() != 1
                || reduction
                    .coefficient(self.boundary.product_master())
                    .is_none()
            {
                return Err(FiveLoopBananaD3Error::UnexpectedBoundaryReduction {
                    input,
                    reduction,
                });
            }
            let coefficient = reduction
                .coefficient(self.boundary.product_master())
                .expect("one checked terminal")
                .clone();
            let entries = BTreeMap::from([
                (
                    FiveLoopBananaD3Column::ProperBoundary { powers },
                    self.arithmetic.context.one(),
                ),
                (FiveLoopBananaD3Column::BoundaryTerminal, -coefficient),
            ]);
            rows.push(self.normalized_row(
                FiveLoopBananaD3RowId::Boundary(row_id),
                entries,
                &mut conditions,
                &mut stats,
            )?);
            boundary_closures.push(FiveLoopBananaD3BoundaryClosure {
                row_id,
                input,
                reduction,
            });
            stats.boundary_rows += 1;
        }
        check_resource(
            "proper-boundary rows",
            stats.boundary_rows,
            self.config.max_boundary_rows,
        )?;

        let global_columns = rows
            .iter()
            .flat_map(|row| row.entries.keys().cloned())
            .collect::<BTreeSet<_>>();
        stats.global_columns = global_columns.len();
        check_resource(
            "global columns",
            stats.global_columns,
            self.config.max_global_columns,
        )?;
        if stats.global_columns != FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS {
            return Err(FiveLoopBananaD3Error::GlobalColumnCount {
                expected: FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS,
                actual: stats.global_columns,
            });
        }
        Ok(CollectedRows {
            seeds,
            rows,
            boundary_closures,
            conditions,
            stats,
        })
    }

    fn raw_graph_row(
        &self,
        powers: [i32; 6],
        differentiated: usize,
        contracted: usize,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<GraphRow, FiveLoopBananaD3Error> {
        let mut row = GraphRow::new();
        if differentiated == contracted {
            self.add_graph_unoriented(
                &mut row,
                GraphColumn { powers, edge: None },
                self.arithmetic.dimension.clone(),
                stats,
            )?;
        }
        let mut dotted = powers;
        dotted[differentiated] = dotted[differentiated]
            .checked_add(1)
            .ok_or(FiveLoopBananaD3Error::ExponentOverflow)?;
        self.add_graph_unoriented(
            &mut row,
            GraphColumn {
                powers: dotted,
                edge: Some(normalized_edge(differentiated, contracted)),
            },
            self.arithmetic
                .context
                .integer(-2 * i64::from(powers[differentiated])),
            stats,
        )?;
        let mut dotted_sixth = powers;
        dotted_sixth[5] = dotted_sixth[5]
            .checked_add(1)
            .ok_or(FiveLoopBananaD3Error::ExponentOverflow)?;
        self.add_graph_unoriented(
            &mut row,
            GraphColumn {
                powers: dotted_sixth,
                edge: Some(normalized_edge(5, contracted)),
            },
            self.arithmetic.context.integer(2 * i64::from(powers[5])),
            stats,
        )?;
        Ok(row)
    }

    fn add_graph_unoriented(
        &self,
        row: &mut GraphRow,
        column: GraphColumn,
        coefficient: Coefficient,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<(), FiveLoopBananaD3Error> {
        stats.raw_graph_terms = checked_add_resource(
            "raw graph terms",
            stats.raw_graph_terms,
            1,
            self.config.max_raw_graph_terms,
        )?;
        self.arithmetic.add_sparse(row, column, coefficient)
    }

    fn canonicalize_graph_row(
        &self,
        input: &GraphRow,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<GraphRow, FiveLoopBananaD3Error> {
        let mut output = GraphRow::new();
        for (column, coefficient) in input {
            stats.symmetry_images = checked_add_resource(
                "joint S6 symmetry images",
                stats.symmetry_images,
                FIVE_LOOP_BANANA_S6_ORDER,
                self.config.max_symmetry_images,
            )?;
            let column = self.permutations.canonical(column);
            if column.active_lines() <= 4 {
                continue;
            }
            self.arithmetic
                .add_sparse(&mut output, column, coefficient.clone())?;
        }
        Ok(output)
    }

    fn authenticate_native_row(
        &self,
        row_id: FiveLoopBananaD3NativeRowId,
        seed: &Integral,
        graph: &GraphRow,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<(), FiveLoopBananaD3Error> {
        let native = IbpGenerator::new(self.boundary.family()).try_generate_raw_identity(
            seed,
            usize::from(row_id.differentiated_loop),
            usize::from(row_id.contraction_loop),
        )?;
        if native.seed != *seed {
            return Err(FiveLoopBananaD3Error::NativeRowLabelMismatch { row_id });
        }
        let expanded = self.expand_graph_row(graph, stats)?;
        if expanded != native.equation {
            return Err(FiveLoopBananaD3Error::NativeExpansionMismatch {
                row_id,
                expected: native.equation,
                actual: expanded,
            });
        }
        Ok(())
    }

    fn expand_graph_row(
        &self,
        graph: &GraphRow,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<LinearCombination, FiveLoopBananaD3Error> {
        let mut output = LinearCombination::new();
        for (column, coefficient) in graph {
            let base = scalar_integral(column.powers);
            let Some(edge) = column.edge else {
                stats.native_expansion_incidences = checked_add_resource(
                    "native expansion incidences",
                    stats.native_expansion_incidences,
                    1,
                    self.config.max_native_expansion_incidences,
                )?;
                output.add_term(base, coefficient.clone());
                continue;
            };
            let expansion =
                self.oriented_dot_expansion(usize::from(edge[0]), usize::from(edge[1]))?;
            stats.native_expansion_incidences = checked_add_resource(
                "native expansion incidences",
                stats.native_expansion_incidences,
                1 + expansion.1.len(),
                self.config.max_native_expansion_incidences,
            )?;
            output.add_term(
                base.clone(),
                self.arithmetic.checked_mul(coefficient, &expansion.0)?,
            );
            for (position, dot_coefficient) in expansion.1 {
                if dot_coefficient.is_zero() {
                    continue;
                }
                let shifted = base
                    .checked_shifted(&[(position, -1)])
                    .ok_or(FiveLoopBananaD3Error::ExponentOverflow)?;
                output.add_term(
                    shifted,
                    self.arithmetic.checked_mul(coefficient, &dot_coefficient)?,
                );
            }
        }
        Ok(output)
    }

    fn oriented_dot_expansion(
        &self,
        left_line: usize,
        right_line: usize,
    ) -> Result<(Coefficient, Vec<(usize, Coefficient)>), FiveLoopBananaD3Error> {
        let left = five_loop_banana_oriented_line_routing(left_line)
            .ok_or(FiveLoopBananaD3Error::OrientedLineOutOfRange { line: left_line })?;
        let right = five_loop_banana_oriented_line_routing(right_line)
            .ok_or(FiveLoopBananaD3Error::OrientedLineOutOfRange { line: right_line })?;
        let context = self.boundary.family().coefficients();
        let mut constant = context.zero();
        let mut denominators = vec![context.zero(); FIVE_LOOP_BANANA_DENOMINATORS];
        for first in 0..FIVE_LOOP_BANANA_LOOP_MOMENTA {
            for second in first..FIVE_LOOP_BANANA_LOOP_MOMENTA {
                let integer = if first == second {
                    i64::from(left[first]) * i64::from(right[first])
                } else {
                    i64::from(left[first]) * i64::from(right[second])
                        + i64::from(left[second]) * i64::from(right[first])
                };
                if integer == 0 {
                    continue;
                }
                let scalar = self
                    .boundary
                    .family()
                    .scalar_product_expansion(first, second)?;
                constant = self.arithmetic.checked_add(
                    &constant,
                    &self
                        .arithmetic
                        .checked_mul(&context.integer(integer), scalar.constant())?,
                )?;
                for (position, rational) in scalar.denominator_coefficients().iter().enumerate() {
                    if rational.is_zero() {
                        continue;
                    }
                    denominators[position] = self.arithmetic.checked_add(
                        &denominators[position],
                        &context.scale_rational(&context.integer(integer), rational),
                    )?;
                }
            }
        }
        Ok((
            constant,
            denominators
                .into_iter()
                .enumerate()
                .filter(|(_, coefficient)| !coefficient.is_zero())
                .collect(),
        ))
    }

    fn algebraic_rows(
        &self,
        raw: &[GraphRow],
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<Vec<(FiveLoopBananaD3AlgebraicRowId, GraphRow)>, FiveLoopBananaD3Error> {
        let powers = raw
            .iter()
            .flat_map(|row| row.keys())
            .filter(|column| column.edge.is_some())
            .map(|column| column.powers)
            .collect::<BTreeSet<_>>();
        stats.moment_power_classes = powers.len();
        if stats.moment_power_classes != FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES {
            return Err(FiveLoopBananaD3Error::MomentPowerClassCount {
                expected: FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES,
                actual: stats.moment_power_classes,
            });
        }
        let mut unique =
            BTreeMap::<String, (FiveLoopBananaD3AlgebraicKind, [i32; 6], u8, GraphRow)>::new();
        for powers in powers {
            for line in 0..FIVE_LOOP_BANANA_PHYSICAL_LINES {
                for kind in [
                    FiveLoopBananaD3AlgebraicKind::Diagonal,
                    FiveLoopBananaD3AlgebraicKind::Momentum,
                ] {
                    stats.algebraic_candidates = checked_add_resource(
                        "algebraic row candidates",
                        stats.algebraic_candidates,
                        1,
                        self.config.max_algebraic_candidates,
                    )?;
                    let row = self.algebraic_candidate(powers, line, kind, stats)?;
                    if row.is_empty() {
                        continue;
                    }
                    let key = graph_row_key(&row);
                    unique
                        .entry(key)
                        .or_insert((kind, powers, u8::try_from(line).unwrap(), row));
                }
            }
        }
        if stats.algebraic_candidates != FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND {
            return Err(FiveLoopBananaD3Error::AlgebraicCandidateCount {
                expected: FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND,
                actual: stats.algebraic_candidates,
            });
        }
        stats.algebraic_rows = unique.len();
        check_resource(
            "algebraic rows",
            stats.algebraic_rows,
            self.config.max_algebraic_rows,
        )?;
        if stats.algebraic_rows != FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS {
            return Err(FiveLoopBananaD3Error::AlgebraicRowCount {
                expected: FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS,
                actual: stats.algebraic_rows,
            });
        }
        Ok(unique
            .into_values()
            .enumerate()
            .map(|(index, (kind, powers, marked_line, row))| {
                (
                    FiveLoopBananaD3AlgebraicRowId {
                        index: u16::try_from(index).unwrap(),
                        kind,
                        powers,
                        marked_line,
                    },
                    row,
                )
            })
            .collect())
    }

    fn algebraic_candidate(
        &self,
        powers: [i32; 6],
        line: usize,
        kind: FiveLoopBananaD3AlgebraicKind,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<GraphRow, FiveLoopBananaD3Error> {
        let mut raw = GraphRow::new();
        match kind {
            FiveLoopBananaD3AlgebraicKind::Diagonal => {
                self.add_algebra_term(
                    &mut raw,
                    GraphColumn {
                        powers,
                        edge: Some(normalized_edge(line, line)),
                    },
                    self.arithmetic.context.one(),
                    stats,
                )?;
                let mut lowered = powers;
                lowered[line] = lowered[line]
                    .checked_sub(1)
                    .ok_or(FiveLoopBananaD3Error::ExponentOverflow)?;
                self.add_algebra_term(
                    &mut raw,
                    GraphColumn {
                        powers: lowered,
                        edge: None,
                    },
                    self.arithmetic.context.integer(-1),
                    stats,
                )?;
                self.add_algebra_term(
                    &mut raw,
                    GraphColumn { powers, edge: None },
                    self.arithmetic.mass.clone(),
                    stats,
                )?;
            }
            FiveLoopBananaD3AlgebraicKind::Momentum => {
                for other in 0..FIVE_LOOP_BANANA_PHYSICAL_LINES {
                    self.add_algebra_term(
                        &mut raw,
                        GraphColumn {
                            powers,
                            edge: Some(normalized_edge(line, other)),
                        },
                        self.arithmetic.context.one(),
                        stats,
                    )?;
                }
            }
        }
        self.canonicalize_graph_row(&raw, stats)
    }

    fn add_algebra_term(
        &self,
        row: &mut GraphRow,
        column: GraphColumn,
        coefficient: Coefficient,
        _stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<(), FiveLoopBananaD3Error> {
        self.arithmetic.add_sparse(row, column, coefficient)
    }

    fn graph_to_typed(
        &self,
        graph: &GraphRow,
    ) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
        let mut entries = BTreeMap::new();
        for (column, coefficient) in graph {
            let active = column.active_lines();
            if active <= 4 {
                continue;
            }
            let typed = match (active, column.edge) {
                (5, None) => FiveLoopBananaD3Column::ProperBoundary {
                    powers: column.powers,
                },
                (6, None) => FiveLoopBananaD3Column::Scalar {
                    powers: column.powers,
                },
                (6, Some(edge)) => FiveLoopBananaD3Column::OneMoment {
                    powers: column.powers,
                    edge,
                },
                (5, Some(edge)) => {
                    return Err(FiveLoopBananaD3Error::UnsupportedProperMoment {
                        powers: column.powers,
                        edge,
                    });
                }
                _ => unreachable!(),
            };
            self.arithmetic
                .add_sparse(&mut entries, typed, coefficient.clone())?;
        }
        Ok(entries)
    }

    fn normalized_row(
        &self,
        row_id: FiveLoopBananaD3RowId,
        mut entries: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
        conditions: &mut Vec<FiveLoopBananaD3NonzeroCondition>,
        stats: &mut FiveLoopBananaD3Stats,
    ) -> Result<FiveLoopBananaD3NormalizedRow, FiveLoopBananaD3Error> {
        let row_scale = if let Some(scale) = entries
            .last_key_value()
            .map(|(_, coefficient)| coefficient.clone())
        {
            record_condition(
                &self.arithmetic,
                &scale,
                FiveLoopBananaD3ConditionSource::Row(row_id),
                conditions,
            )?;
            for coefficient in entries.values_mut() {
                *coefficient = self.arithmetic.checked_div(coefficient, &scale)?;
            }
            scale
        } else {
            self.arithmetic.context.one()
        };
        stats.collected_nonzeros = checked_add_resource(
            "collected normalized nonzeros",
            stats.collected_nonzeros,
            entries.len(),
            self.config.max_collected_nonzeros,
        )?;
        Ok(FiveLoopBananaD3NormalizedRow {
            row_id,
            row_scale,
            entries,
        })
    }

    fn scalar_column(
        &self,
        powers: [i32; 6],
    ) -> Result<FiveLoopBananaD3Column, FiveLoopBananaD3Error> {
        let canonical = self
            .permutations
            .canonical(&GraphColumn { powers, edge: None });
        if canonical.active_lines() != 6 {
            return Err(FiveLoopBananaD3Error::MalformedTopScalar { powers });
        }
        Ok(FiveLoopBananaD3Column::Scalar {
            powers: canonical.powers,
        })
    }
}

fn graph_row_key(row: &GraphRow) -> String {
    row.iter()
        .map(|(column, coefficient)| {
            format!(
                "{:?}={}",
                column,
                canonical_symbolica_atom(&coefficient.to_expression())
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn canonical_public_scalar(
    column: &FiveLoopBananaD3Column,
    permutations: &Permutations,
) -> FiveLoopBananaD3Column {
    let FiveLoopBananaD3Column::Scalar { powers } = column else {
        return column.clone();
    };
    let graph = permutations.canonical(&GraphColumn {
        powers: *powers,
        edge: None,
    });
    FiveLoopBananaD3Column::Scalar {
        powers: graph.powers,
    }
}

fn scalar_integral(powers: [i32; 6]) -> Integral {
    let mut all = vec![0; FIVE_LOOP_BANANA_DENOMINATORS];
    all[..FIVE_LOOP_BANANA_PHYSICAL_LINES].copy_from_slice(&powers);
    Integral::new(all)
}

fn validate_scalar_input(integral: &Integral) -> Result<(), FiveLoopBananaD3Error> {
    if integral.powers().len() != FIVE_LOOP_BANANA_DENOMINATORS {
        return Err(FiveLoopBananaD3Error::WrongIntegralArity {
            expected: FIVE_LOOP_BANANA_DENOMINATORS,
            actual: integral.powers().len(),
        });
    }
    if let Some((position, power)) = integral
        .powers()
        .iter()
        .enumerate()
        .find(|(position, power)| **power < 0 || (*position >= 6 && **power != 0))
    {
        return Err(FiveLoopBananaD3Error::NonScalarInput {
            position,
            power: *power,
        });
    }
    Ok(())
}

fn classify_top_scalar(
    integral: &Integral,
) -> Result<FiveLoopBananaD3Column, FiveLoopBananaD3Error> {
    validate_scalar_input(integral)?;
    let powers: [i32; 6] = integral.powers()[..6].try_into().unwrap();
    if powers.iter().any(|power| *power <= 0) {
        return Err(FiveLoopBananaD3Error::NotTopSector {
            integral: integral.clone(),
        });
    }
    Ok(FiveLoopBananaD3Column::Scalar { powers })
}

fn expected_d3_formula(
    context: &CoefficientContext,
    target: usize,
) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
    let (b2, master) = match target {
        0 => (
            "5*(11*d-50)/(72*m2)",
            "(-125*d^3+1225*d^2-3830*d+3864)/(864*m2^3)",
        ),
        1 => (
            "(19*d-46)/(24*m2)",
            "(-50*d^3+385*d^2-986*d+840)/(288*m2^3)",
        ),
        2 => ("(47-17*d)/(12*m2)", "(50*d^3-385*d^2+986*d-840)/(288*m2^3)"),
        _ => unreachable!(),
    };
    Ok(BTreeMap::from([
        (
            FiveLoopBananaD3Column::Scalar {
                powers: [1, 1, 1, 1, 1, 1],
            },
            context
                .parse(master)
                .map_err(FiveLoopBananaD3Error::CoefficientParse)?,
        ),
        (
            FiveLoopBananaD3Column::Scalar {
                powers: [2, 2, 1, 1, 1, 1],
            },
            context
                .parse(b2)
                .map_err(FiveLoopBananaD3Error::CoefficientParse)?,
        ),
    ]))
}

#[derive(Clone)]
struct WorkRow {
    entries: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
    source_weights: BTreeMap<FiveLoopBananaD3RowId, Coefficient>,
}

fn eliminate(
    arithmetic: &Arithmetic,
    config: FiveLoopBananaD3Config,
    rows: &[FiveLoopBananaD3NormalizedRow],
    mut conditions: Vec<FiveLoopBananaD3NonzeroCondition>,
    mut stats: FiveLoopBananaD3Stats,
) -> Result<
    (
        Vec<FiveLoopBananaD3PivotRule>,
        Vec<FiveLoopBananaD3Column>,
        Vec<FiveLoopBananaD3NonzeroCondition>,
        FiveLoopBananaD3Stats,
    ),
    FiveLoopBananaD3Error,
> {
    let all_columns = rows
        .iter()
        .flat_map(|row| row.entries.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut pivots = BTreeMap::<FiveLoopBananaD3Column, WorkRow>::new();
    for row in rows {
        let mut work = WorkRow {
            entries: row.entries.clone(),
            source_weights: BTreeMap::from([(row.row_id, arithmetic.context.one())]),
        };
        loop {
            let Some(hardest) = work
                .entries
                .last_key_value()
                .map(|(column, _)| column.clone())
            else {
                break;
            };
            let Some(pivot) = pivots.get(&hardest) else {
                break;
            };
            let factor = work.entries.get(&hardest).unwrap().clone();
            add_scaled_work_row(arithmetic, config, &mut work, pivot, &(-factor), &mut stats)?;
        }
        if work.entries.is_empty() {
            continue;
        }
        let pivot = work.entries.last_key_value().unwrap().0.clone();
        let divisor = work.entries.get(&pivot).unwrap().clone();
        record_condition(
            arithmetic,
            &divisor,
            FiveLoopBananaD3ConditionSource::Pivot(pivot.clone()),
            &mut conditions,
        )?;
        divide_work_row(arithmetic, config, &mut work, &divisor, &mut stats)?;
        pivots.insert(pivot, work);
    }
    let pivot_columns = pivots.keys().cloned().collect::<BTreeSet<_>>();
    let free_columns = all_columns.difference(&pivot_columns).cloned().collect();
    let rules = pivots
        .into_iter()
        .rev()
        .map(|(pivot, work)| {
            let mut equation = work.entries;
            equation.remove(&pivot).expect("pivot row contains pivot");
            FiveLoopBananaD3PivotRule {
                pivot,
                rhs: equation
                    .into_iter()
                    .map(|(column, coefficient)| (column, -coefficient))
                    .collect(),
                source_row_weights: work.source_weights,
            }
        })
        .collect();
    Ok((rules, free_columns, conditions, stats))
}

fn add_scaled_work_row(
    arithmetic: &Arithmetic,
    config: FiveLoopBananaD3Config,
    target: &mut WorkRow,
    source: &WorkRow,
    factor: &Coefficient,
    stats: &mut FiveLoopBananaD3Stats,
) -> Result<(), FiveLoopBananaD3Error> {
    for (column, coefficient) in &source.entries {
        charge_update(config, stats)?;
        let scaled = arithmetic.checked_mul(coefficient, factor)?;
        arithmetic.add_sparse(&mut target.entries, column.clone(), scaled)?;
    }
    for (row_id, coefficient) in &source.source_weights {
        charge_update(config, stats)?;
        let scaled = arithmetic.checked_mul(coefficient, factor)?;
        arithmetic.add_sparse(&mut target.source_weights, *row_id, scaled)?;
    }
    Ok(())
}

fn divide_work_row(
    arithmetic: &Arithmetic,
    config: FiveLoopBananaD3Config,
    row: &mut WorkRow,
    divisor: &Coefficient,
    stats: &mut FiveLoopBananaD3Stats,
) -> Result<(), FiveLoopBananaD3Error> {
    for coefficient in row.entries.values_mut() {
        charge_update(config, stats)?;
        *coefficient = arithmetic.checked_div(coefficient, divisor)?;
    }
    for coefficient in row.source_weights.values_mut() {
        charge_update(config, stats)?;
        *coefficient = arithmetic.checked_div(coefficient, divisor)?;
    }
    Ok(())
}

fn charge_update(
    config: FiveLoopBananaD3Config,
    stats: &mut FiveLoopBananaD3Stats,
) -> Result<(), FiveLoopBananaD3Error> {
    stats.elimination_updates = checked_add_resource(
        "elimination coefficient updates",
        stats.elimination_updates,
        1,
        config.max_elimination_updates,
    )?;
    Ok(())
}

fn reduce_by_rules(
    arithmetic: &Arithmetic,
    mut entries: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
    rules: &BTreeMap<FiveLoopBananaD3Column, &FiveLoopBananaD3PivotRule>,
) -> Result<BTreeMap<FiveLoopBananaD3Column, Coefficient>, FiveLoopBananaD3Error> {
    loop {
        let Some((pivot, rule)) = entries
            .keys()
            .rev()
            .find_map(|column| rules.get(column).map(|rule| (column.clone(), *rule)))
        else {
            break;
        };
        let factor = entries.remove(&pivot).expect("selected pivot exists");
        for (column, coefficient) in &rule.rhs {
            let scaled = arithmetic.checked_mul(coefficient, &factor)?;
            arithmetic.add_sparse(&mut entries, column.clone(), scaled)?;
        }
    }
    Ok(entries)
}

fn record_condition(
    arithmetic: &Arithmetic,
    divisor: &Coefficient,
    source: FiveLoopBananaD3ConditionSource,
    conditions: &mut Vec<FiveLoopBananaD3NonzeroCondition>,
) -> Result<(), FiveLoopBananaD3Error> {
    if divisor.is_zero() {
        return Err(FiveLoopBananaD3Error::ZeroPivot);
    }
    if coefficient_variable_degrees(divisor)
        .iter()
        .all(|(numerator, _)| *numerator == 0)
    {
        return Ok(());
    }
    let condition = FiveLoopBananaD3NonzeroCondition {
        source,
        polynomial: canonical_symbolica_atom(&divisor.numerator.to_expression()),
    };
    if !conditions.contains(&condition) {
        conditions.push(condition);
    }
    let _ = arithmetic;
    Ok(())
}

struct Arithmetic {
    context: CoefficientContext,
    dimension: Coefficient,
    mass: Coefficient,
    max_degree: usize,
}

impl Arithmetic {
    fn new(
        context: &CoefficientContext,
        config: FiveLoopBananaD3Config,
    ) -> Result<Self, FiveLoopBananaD3Error> {
        let dimension = context
            .parameter("d")
            .ok_or(FiveLoopBananaD3Error::MissingParameter { name: "d" })?;
        let mass = context
            .parameter("m2")
            .ok_or(FiveLoopBananaD3Error::MissingParameter { name: "m2" })?;
        Ok(Self {
            context: context.clone(),
            dimension,
            mass,
            max_degree: config.max_coefficient_degree,
        })
    }

    fn check_degree(&self, requested: u128) -> Result<(), FiveLoopBananaD3Error> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(FiveLoopBananaD3Error::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        if requested > self.max_degree as u128 {
            return Err(FiveLoopBananaD3Error::ResourceLimit {
                resource: "configured coefficient exponent degree",
                requested,
                limit: self.max_degree as u128,
            });
        }
        Ok(())
    }

    fn checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FiveLoopBananaD3Error> {
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        Ok(left * right)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FiveLoopBananaD3Error> {
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        Ok(left + right)
    }

    fn checked_div(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FiveLoopBananaD3Error> {
        if right.is_zero() {
            return Err(FiveLoopBananaD3Error::ZeroPivot);
        }
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        Ok(left / right)
    }

    fn add_sparse<K: Ord>(
        &self,
        entries: &mut BTreeMap<K, Coefficient>,
        key: K,
        coefficient: Coefficient,
    ) -> Result<(), FiveLoopBananaD3Error> {
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = entries.get_mut(&key) {
            let sum = self.checked_add(current, &coefficient)?;
            if sum.is_zero() {
                entries.remove(&key);
            } else {
                *current = sum;
            }
        } else {
            entries.insert(key, coefficient);
        }
        Ok(())
    }
}

fn coefficient_quotient_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() {
        return u128::MAX;
    }
    coefficient_variable_degrees(left)
        .into_iter()
        .zip(coefficient_variable_degrees(right))
        .map(
            |((left_numerator, left_denominator), (right_numerator, right_denominator))| {
                left_numerator
                    .saturating_add(right_denominator)
                    .max(left_denominator.saturating_add(right_numerator))
            },
        )
        .max()
        .unwrap_or(0)
}

fn preflight_config(config: FiveLoopBananaD3Config) -> Result<(), FiveLoopBananaD3Error> {
    for (resource, requested, limit) in [
        (
            "scalar seed orbits",
            FIVE_LOOP_BANANA_D3_SEED_ORBITS,
            config.max_seed_orbits,
        ),
        (
            "native raw origins",
            FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS,
            config.max_native_raw_origins,
        ),
        (
            "raw graph terms",
            FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND,
            config.max_raw_graph_terms,
        ),
        (
            "native expansion incidences",
            FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND,
            config.max_native_expansion_incidences,
        ),
        (
            "algebraic row candidates",
            FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND,
            config.max_algebraic_candidates,
        ),
        (
            "algebraic rows",
            FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS,
            config.max_algebraic_rows,
        ),
        (
            "proper-boundary rows",
            FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS,
            config.max_boundary_rows,
        ),
        (
            "joint S6 symmetry images",
            FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND,
            config.max_symmetry_images,
        ),
        (
            "global columns",
            FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS,
            config.max_global_columns,
        ),
        (
            "collected normalized nonzeros",
            FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND,
            config.max_collected_nonzeros,
        ),
        (
            "elimination coefficient updates",
            FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND,
            config.max_elimination_updates,
        ),
        (
            "source-row provenance weights",
            FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND,
            config.max_source_row_weights,
        ),
    ] {
        check_resource(resource, requested, limit)?;
    }
    if config.max_coefficient_degree == 0 {
        return Err(FiveLoopBananaD3Error::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: 1,
            limit: 0,
        });
    }
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FiveLoopBananaD3Error::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: config.max_coefficient_degree as u128,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    Ok(())
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FiveLoopBananaD3Error> {
    if requested > limit {
        Err(FiveLoopBananaD3Error::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        })
    } else {
        Ok(())
    }
}

fn checked_add_resource(
    resource: &'static str,
    current: usize,
    increment: usize,
    limit: usize,
) -> Result<usize, FiveLoopBananaD3Error> {
    let requested = current
        .checked_add(increment)
        .ok_or(FiveLoopBananaD3Error::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        })?;
    check_resource(resource, requested, limit)?;
    Ok(requested)
}

#[derive(Debug)]
pub enum FiveLoopBananaD3Error {
    Family(FamilyError),
    Boundary(FiveLoopBananaBoundaryError),
    Ibp(IbpGenerationError),
    MissingParameter {
        name: &'static str,
    },
    CoefficientParse(String),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    NonScalarInput {
        position: usize,
        power: i32,
    },
    NotTopSector {
        integral: Integral,
    },
    OutOfCoverage {
        integral: Integral,
        dot_degree: u64,
        maximum: u64,
    },
    UnexpectedPublicTerminal {
        column: FiveLoopBananaD3Column,
    },
    ColumnOutsideCertifiedShell {
        column: FiveLoopBananaD3Column,
    },
    SeedOrbitSizeMismatch {
        orbit: FiveLoopBananaD3SeedOrbit,
        expected: usize,
        actual: usize,
    },
    NativeOriginCount {
        expected: usize,
        actual: usize,
    },
    NonzeroNativeRowCount {
        expected: usize,
        actual: usize,
    },
    NativeRowLabelMismatch {
        row_id: FiveLoopBananaD3NativeRowId,
    },
    NativeExpansionMismatch {
        row_id: FiveLoopBananaD3NativeRowId,
        expected: LinearCombination,
        actual: LinearCombination,
    },
    AlgebraicRowCount {
        expected: usize,
        actual: usize,
    },
    MomentPowerClassCount {
        expected: usize,
        actual: usize,
    },
    AlgebraicCandidateCount {
        expected: usize,
        actual: usize,
    },
    BoundaryRowCount {
        expected: usize,
        actual: usize,
    },
    GraphColumnCount {
        expected: usize,
        actual: usize,
    },
    GlobalColumnCount {
        expected: usize,
        actual: usize,
    },
    RankMismatch {
        expected: usize,
        actual: usize,
    },
    TargetOrbitCollapse,
    MissingTargetPivot {
        target: FiveLoopBananaD3Column,
    },
    UnexpectedBoundaryReduction {
        input: Integral,
        reduction: LinearCombination,
    },
    UnsupportedProperMoment {
        powers: [i32; 6],
        edge: [u8; 2],
    },
    MalformedTopScalar {
        powers: [i32; 6],
    },
    OrientedLineOutOfRange {
        line: usize,
    },
    ExponentOverflow,
    ZeroPivot,
    CertificateReplayMismatch,
    NormalizedRowRemainder {
        row_id: FiveLoopBananaD3RowId,
        remainder: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
    },
    UnknownSourceRow {
        row_id: FiveLoopBananaD3RowId,
    },
    PivotProvenanceMismatch {
        pivot: FiveLoopBananaD3Column,
    },
    NonTriangularPivot {
        pivot: FiveLoopBananaD3Column,
    },
    CandidateFormulaMismatch {
        target: FiveLoopBananaD3Column,
        actual: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
        expected: BTreeMap<FiveLoopBananaD3Column, Coefficient>,
    },
}

impl fmt::Display for FiveLoopBananaD3Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "five-loop D3 family error: {error}"),
            Self::Boundary(error) => write!(formatter, "five-loop D3 boundary error: {error}"),
            Self::Ibp(error) => write!(formatter, "five-loop D3 IBP error: {error}"),
            Self::MissingParameter { name } => {
                write!(formatter, "five-loop D3 family misses {name}")
            }
            Self::CoefficientParse(error) => {
                write!(formatter, "five-loop D3 coefficient parse failed: {error}")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "five-loop D3 {resource} requires {requested}, exceeding {limit}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "five-loop D3 input has {actual} powers, expected {expected}"
            ),
            Self::NonScalarInput { position, power } => write!(
                formatter,
                "five-loop D3 scalar input has unsupported power {power} at {position}"
            ),
            Self::NotTopSector { integral } => {
                write!(formatter, "{integral} is not a six-line top-sector target")
            }
            Self::OutOfCoverage {
                dot_degree,
                maximum,
                ..
            } => write!(
                formatter,
                "five-loop banana dot degree {dot_degree} exceeds D={maximum} coverage"
            ),
            Self::UnexpectedPublicTerminal { column } => write!(
                formatter,
                "unexpected public D3 terminal {}",
                column.stable_key()
            ),
            Self::ColumnOutsideCertifiedShell { column } => write!(
                formatter,
                "column {} is outside the certified five-loop D3 shell",
                column.stable_key()
            ),
            Self::SeedOrbitSizeMismatch {
                orbit,
                expected,
                actual,
            } => write!(
                formatter,
                "seed {orbit:?} has labelled orbit size {actual}, expected {expected}"
            ),
            Self::NativeOriginCount { expected, actual } => write!(
                formatter,
                "authenticated {actual} native origins, expected {expected}"
            ),
            Self::NonzeroNativeRowCount { expected, actual } => write!(
                formatter,
                "collected {actual} nonzero native rows, expected {expected}"
            ),
            Self::NativeRowLabelMismatch { row_id } => write!(
                formatter,
                "native row label mismatch at {}",
                row_id.stable_key()
            ),
            Self::NativeExpansionMismatch { row_id, .. } => write!(
                formatter,
                "oriented row does not expand to native row {}",
                row_id.stable_key()
            ),
            Self::AlgebraicRowCount { expected, actual } => write!(
                formatter,
                "collected {actual} algebraic rows, expected {expected}"
            ),
            Self::MomentPowerClassCount { expected, actual } => write!(
                formatter,
                "moment halo has {actual} power classes, expected {expected}"
            ),
            Self::AlgebraicCandidateCount { expected, actual } => write!(
                formatter,
                "moment halo emitted {actual} algebraic candidates, expected {expected}"
            ),
            Self::BoundaryRowCount { expected, actual } => write!(
                formatter,
                "collected {actual} boundary rows, expected {expected}"
            ),
            Self::GraphColumnCount { expected, actual } => write!(
                formatter,
                "graph shell has {actual} columns, expected {expected}"
            ),
            Self::GlobalColumnCount { expected, actual } => write!(
                formatter,
                "closed shell has {actual} columns, expected {expected}"
            ),
            Self::RankMismatch { expected, actual } => write!(
                formatter,
                "exact shell rank is {actual}, expected {expected}"
            ),
            Self::TargetOrbitCollapse => {
                formatter.write_str("three D3 scalar target orbits collapsed")
            }
            Self::MissingTargetPivot { target } => write!(
                formatter,
                "D3 target {} is not a pivot",
                target.stable_key()
            ),
            Self::UnexpectedBoundaryReduction { input, .. } => write!(
                formatter,
                "five-line boundary {input} did not reduce only to T1^5"
            ),
            Self::UnsupportedProperMoment { powers, edge } => write!(
                formatter,
                "unsupported five-line moment {powers:?} edge {edge:?}"
            ),
            Self::MalformedTopScalar { powers } => {
                write!(formatter, "malformed top scalar powers {powers:?}")
            }
            Self::OrientedLineOutOfRange { line } => {
                write!(formatter, "oriented line {line} is outside 0..6")
            }
            Self::ExponentOverflow => formatter.write_str("five-loop D3 exponent shift overflow"),
            Self::ZeroPivot => formatter.write_str("five-loop D3 attempted division by zero"),
            Self::CertificateReplayMismatch => {
                formatter.write_str("five-loop D3 regenerated certificate differs from stored data")
            }
            Self::NormalizedRowRemainder { row_id, .. } => write!(
                formatter,
                "normalized row {} has nonzero remainder",
                row_id.stable_key()
            ),
            Self::UnknownSourceRow { row_id } => write!(
                formatter,
                "unknown pivot source row {}",
                row_id.stable_key()
            ),
            Self::PivotProvenanceMismatch { pivot } => write!(
                formatter,
                "pivot {} has invalid source weights",
                pivot.stable_key()
            ),
            Self::NonTriangularPivot { pivot } => {
                write!(formatter, "pivot {} is not triangular", pivot.stable_key())
            }
            Self::CandidateFormulaMismatch { target, .. } => write!(
                formatter,
                "exact normal form for {} differs from reconstructed formula",
                target.stable_key()
            ),
        }
    }
}

impl std::error::Error for FiveLoopBananaD3Error {}

impl From<FamilyError> for FiveLoopBananaD3Error {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<FiveLoopBananaBoundaryError> for FiveLoopBananaD3Error {
    fn from(value: FiveLoopBananaBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<IbpGenerationError> for FiveLoopBananaD3Error {
    fn from(value: IbpGenerationError) -> Self {
        Self::Ibp(value)
    }
}
