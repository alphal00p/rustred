//! Replayable scalar `D=2` shell for the genuine three-loop `B4` sector.
//!
//! The shell contains every scalar B4 seed orbit through dot degree two:
//! the corner, the one-dot orbit, the triple-dot orbit, and the adjacent and
//! opposite double-dot orbits of the four-cycle stabilizer.  It generates all
//! nine native IBPs per
//! seed, jointly orients scalar and numerator powers with the proved
//! tetrahedron symmetry, closes every proper sector through
//! [`ThreeLoopBoundaryReducer`], and performs deterministic exact sparse
//! elimination with source-row weights.
//!
//! This is a finite certificate over generic `Q(d,m2)`.  It does not claim an
//! unrestricted B4 recurrence, master minimality, or validity on a recorded
//! pivot polynomial's exceptional locus.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use symbolica::prelude::AtomCore;

use crate::coefficient::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::three_loop::equal_mass_three_loop_tetrahedron;
use crate::{
    Coefficient, CoefficientContext, FamilyError, IbpGenerationError, IbpGenerator, Integral,
    LinearCombination, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, ThreeLoopBoundaryConfig,
    ThreeLoopBoundaryError, ThreeLoopBoundaryReducer, VacuumFamily,
};

const DENOMINATORS: usize = 6;
const LOOPS: usize = 3;
const B4_MASK: u8 = 43;
const B4_ACTIVE: [usize; 4] = [0, 1, 3, 5];
const B4_INACTIVE: [usize; 2] = [2, 4];

pub const THREE_LOOP_B4_D2_SEED_ORBITS: usize = 5;
pub const THREE_LOOP_B4_D2_RAW_ROWS: usize = THREE_LOOP_B4_D2_SEED_ORBITS * LOOPS * LOOPS;
/// One scalar seed has four nonzero powers.  A native row contains at most one
/// dimension term and, per active power, one constant plus six denominator
/// terms: `1 + 4*(1+6) = 29`.
pub const THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND: usize = THREE_LOOP_B4_D2_RAW_ROWS * 29;
pub const THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND: usize = THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND;
pub const THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND: usize =
    THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND;
pub const THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND: usize = THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND;
pub const THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND: usize =
    THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND * 24;
pub const THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND: usize =
    THREE_LOOP_B4_D2_RAW_ROWS * THREE_LOOP_B4_D2_RAW_ROWS;
pub const THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND: usize = THREE_LOOP_B4_D2_RAW_ROWS
    * THREE_LOOP_B4_D2_RAW_ROWS
    * (THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND + THREE_LOOP_B4_D2_RAW_ROWS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopB4D2Config {
    pub boundary: ThreeLoopBoundaryConfig,
    pub max_seed_orbits: usize,
    pub max_raw_rows: usize,
    pub max_raw_term_incidences: usize,
    pub max_boundary_calls: usize,
    pub max_symmetry_images: usize,
    pub max_global_columns: usize,
    pub max_collected_nonzeros: usize,
    pub max_elimination_updates: usize,
    pub max_source_row_weights: usize,
    pub max_coefficient_degree: usize,
}

impl Default for ThreeLoopB4D2Config {
    fn default() -> Self {
        Self {
            boundary: ThreeLoopBoundaryConfig {
                max_numerator_degree: 1,
                max_two_loop_dots: 3,
                max_two_loop_seed_candidates: 500,
                ..ThreeLoopBoundaryConfig::default()
            },
            max_seed_orbits: THREE_LOOP_B4_D2_SEED_ORBITS,
            max_raw_rows: THREE_LOOP_B4_D2_RAW_ROWS,
            max_raw_term_incidences: THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND,
            max_boundary_calls: THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND,
            max_symmetry_images: THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND,
            max_global_columns: THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND,
            max_collected_nonzeros: THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND,
            max_elimination_updates: THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND,
            max_source_row_weights: THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND,
            max_coefficient_degree: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThreeLoopB4D2SeedOrbit {
    Corner,
    OneDot,
    TripleDot,
    AdjacentDoubleDot,
    OppositeDoubleDot,
}

impl ThreeLoopB4D2SeedOrbit {
    pub const ALL: [Self; THREE_LOOP_B4_D2_SEED_ORBITS] = [
        Self::Corner,
        Self::OneDot,
        Self::TripleDot,
        Self::AdjacentDoubleDot,
        Self::OppositeDoubleDot,
    ];

    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Corner => "rustred-three-loop-b4-d2-seed-v1:D0",
            Self::OneDot => "rustred-three-loop-b4-d2-seed-v1:D1",
            Self::TripleDot => "rustred-three-loop-b4-d2-seed-v1:D2:A",
            Self::AdjacentDoubleDot => "rustred-three-loop-b4-d2-seed-v1:D2:C-adjacent",
            Self::OppositeDoubleDot => "rustred-three-loop-b4-d2-seed-v1:D2:C-opposite",
        }
    }

    fn representative(self) -> Integral {
        match self {
            Self::Corner => Integral::from([1, 1, 0, 1, 0, 1]),
            Self::OneDot => Integral::from([2, 1, 0, 1, 0, 1]),
            Self::TripleDot => Integral::from([3, 1, 0, 1, 0, 1]),
            Self::AdjacentDoubleDot => Integral::from([2, 2, 0, 1, 0, 1]),
            Self::OppositeDoubleDot => Integral::from([2, 1, 0, 1, 0, 2]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopB4D2Seed {
    orbit: ThreeLoopB4D2SeedOrbit,
    integral: Integral,
}

impl ThreeLoopB4D2Seed {
    pub const fn orbit(&self) -> ThreeLoopB4D2SeedOrbit {
        self.orbit
    }

    pub const fn integral(&self) -> &Integral {
        &self.integral
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThreeLoopB4D2RawRowId {
    seed_orbit: ThreeLoopB4D2SeedOrbit,
    differentiated_loop: u8,
    contraction_loop: u8,
}

impl ThreeLoopB4D2RawRowId {
    pub const SCHEMA: &'static str = "rustred-three-loop-b4-d2-raw-row-v1";

    pub const fn new(
        seed_orbit: ThreeLoopB4D2SeedOrbit,
        differentiated_loop: u8,
        contraction_loop: u8,
    ) -> Self {
        Self {
            seed_orbit,
            differentiated_loop,
            contraction_loop,
        }
    }

    pub const fn seed_orbit(self) -> ThreeLoopB4D2SeedOrbit {
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
pub enum ThreeLoopB4BoundaryColumn {
    TadpoleCubed,
    TadpoleSunset,
}

impl ThreeLoopB4BoundaryColumn {
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::TadpoleCubed => "rustred-three-loop-b4-d2-boundary-v1:T1^3",
            Self::TadpoleSunset => "rustred-three-loop-b4-d2-boundary-v1:T1*S2",
        }
    }

    const fn mass_weight(self) -> i64 {
        match self {
            Self::TadpoleCubed => 3,
            Self::TadpoleSunset => 4,
        }
    }
}

/// Disjoint finite-shell columns.  Scalar powers are stored in compact B4
/// order `(0,1,3,5)`; numerator columns retain all six jointly oriented powers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopB4D2Column {
    Boundary(ThreeLoopB4BoundaryColumn),
    Scalar { powers: [i32; 4] },
    Numerator { powers: [i32; 6] },
}

impl ThreeLoopB4D2Column {
    pub const SCHEMA: &'static str = "rustred-three-loop-b4-d2-column-v1";

    pub fn stable_key(&self) -> String {
        match self {
            Self::Boundary(boundary) => boundary.stable_key().to_string(),
            Self::Scalar { powers } => format!(
                "{}:scalar:[{},{},{},{}]",
                Self::SCHEMA,
                powers[0],
                powers[1],
                powers[2],
                powers[3]
            ),
            Self::Numerator { powers } => format!(
                "{}:numerator:[{}]",
                Self::SCHEMA,
                powers
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    pub fn mass_weight(&self) -> i64 {
        match self {
            Self::Boundary(boundary) => boundary.mass_weight(),
            Self::Scalar { powers } => powers.iter().map(|power| i64::from(*power)).sum(),
            Self::Numerator { powers } => powers.iter().map(|power| i64::from(*power)).sum(),
        }
    }

    pub fn dot_degree(&self) -> u64 {
        match self {
            Self::Boundary(_) => 0,
            Self::Scalar { powers } => powers
                .iter()
                .map(|power| u64::from(power.saturating_sub(1).max(0) as u32))
                .sum(),
            Self::Numerator { powers } => B4_ACTIVE
                .iter()
                .map(|position| u64::from(powers[*position].saturating_sub(1).max(0) as u32))
                .sum(),
        }
    }

    pub fn numerator_degree(&self) -> u64 {
        match self {
            Self::Boundary(_) | Self::Scalar { .. } => 0,
            Self::Numerator { powers } => powers
                .iter()
                .map(|power| u64::from(power.saturating_neg().max(0) as u32))
                .sum(),
        }
    }

    fn order_key(&self) -> (u8, u64, u64, u8, [i32; 6]) {
        match self {
            Self::Boundary(boundary) => (0, 0, 0, *boundary as u8, [0, 0, 0, 0, 0, 0]),
            Self::Scalar { powers } => (
                4,
                self.dot_degree(),
                self.dot_degree(),
                0,
                [powers[0], powers[1], 0, powers[2], 0, powers[3]],
            ),
            Self::Numerator { powers } => (
                4,
                self.dot_degree().saturating_add(self.numerator_degree()),
                self.dot_degree(),
                1,
                *powers,
            ),
        }
    }
}

impl Ord for ThreeLoopB4D2Column {
    fn cmp(&self, other: &Self) -> Ordering {
        self.order_key().cmp(&other.order_key())
    }
}

impl PartialOrd for ThreeLoopB4D2Column {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopB4D2NormalizedRow {
    raw_id: ThreeLoopB4D2RawRowId,
    seed_mass_weight: i64,
    row_scale: Coefficient,
    entries: BTreeMap<ThreeLoopB4D2Column, Coefficient>,
}

impl ThreeLoopB4D2NormalizedRow {
    pub const fn raw_id(&self) -> ThreeLoopB4D2RawRowId {
        self.raw_id
    }

    pub const fn seed_mass_weight(&self) -> i64 {
        self.seed_mass_weight
    }

    pub const fn row_scale(&self) -> &Coefficient {
        &self.row_scale
    }

    pub const fn entries(&self) -> &BTreeMap<ThreeLoopB4D2Column, Coefficient> {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopB4D2PivotRule {
    pivot: ThreeLoopB4D2Column,
    rhs: BTreeMap<ThreeLoopB4D2Column, Coefficient>,
    source_row_weights: BTreeMap<ThreeLoopB4D2RawRowId, Coefficient>,
}

impl ThreeLoopB4D2PivotRule {
    pub const fn pivot(&self) -> &ThreeLoopB4D2Column {
        &self.pivot
    }

    pub const fn rhs(&self) -> &BTreeMap<ThreeLoopB4D2Column, Coefficient> {
        &self.rhs
    }

    pub const fn source_row_weights(&self) -> &BTreeMap<ThreeLoopB4D2RawRowId, Coefficient> {
        &self.source_row_weights
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopB4D2ConditionSource {
    RawRow(ThreeLoopB4D2RawRowId),
    Pivot(ThreeLoopB4D2Column),
}

/// Polynomial in `d` which must be nonzero for one recorded division.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopB4D2NonzeroCondition {
    source: ThreeLoopB4D2ConditionSource,
    polynomial: String,
}

impl ThreeLoopB4D2NonzeroCondition {
    pub const fn source(&self) -> &ThreeLoopB4D2ConditionSource {
        &self.source
    }

    pub fn polynomial(&self) -> &str {
        &self.polynomial
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThreeLoopB4D2Stats {
    pub raw_rows: usize,
    pub raw_term_incidences: usize,
    pub boundary_calls: usize,
    pub symmetry_images: usize,
    pub collected_nonzeros: usize,
    pub elimination_updates: usize,
    pub source_row_weights: usize,
}

#[derive(Clone, Debug)]
pub struct ThreeLoopB4D2Shell {
    config: ThreeLoopB4D2Config,
    family: VacuumFamily,
    seeds: Vec<ThreeLoopB4D2Seed>,
    rows: Vec<ThreeLoopB4D2NormalizedRow>,
    pivots: Vec<ThreeLoopB4D2PivotRule>,
    free_columns: Vec<ThreeLoopB4D2Column>,
    nonzero_conditions: Vec<ThreeLoopB4D2NonzeroCondition>,
    targets: [ThreeLoopB4D2Column; 3],
    stats: ThreeLoopB4D2Stats,
}

impl ThreeLoopB4D2Shell {
    pub const SCHEMA: &'static str = "rustred-three-loop-b4-d2-shell-v1";

    pub fn build(config: ThreeLoopB4D2Config) -> Result<Self, ThreeLoopB4D2Error> {
        preflight_config(config)?;
        Self::new(equal_mass_three_loop_tetrahedron()?, config)
    }

    pub fn new(
        family: VacuumFamily,
        config: ThreeLoopB4D2Config,
    ) -> Result<Self, ThreeLoopB4D2Error> {
        preflight_config(config)?;
        let shell = ShellBuilder::new(family, config)?.build()?;
        shell.replay()?;
        Ok(shell)
    }

    pub const fn config(&self) -> ThreeLoopB4D2Config {
        self.config
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub fn seeds(&self) -> &[ThreeLoopB4D2Seed] {
        &self.seeds
    }

    pub fn normalized_rows(&self) -> &[ThreeLoopB4D2NormalizedRow] {
        &self.rows
    }

    pub fn pivots(&self) -> &[ThreeLoopB4D2PivotRule] {
        &self.pivots
    }

    pub fn rank(&self) -> usize {
        self.pivots.len()
    }

    pub fn free_columns(&self) -> &[ThreeLoopB4D2Column] {
        &self.free_columns
    }

    pub fn nonzero_conditions(&self) -> &[ThreeLoopB4D2NonzeroCondition] {
        &self.nonzero_conditions
    }

    pub const fn target_columns(&self) -> &[ThreeLoopB4D2Column; 3] {
        &self.targets
    }

    pub const fn stats(&self) -> ThreeLoopB4D2Stats {
        self.stats
    }

    /// Reduce any labelled scalar `D=2` B4 orbit in the homogeneous shell
    /// basis. Every symmetry image is jointly oriented before rule lookup.
    pub fn reduce_target(
        &self,
        integral: &Integral,
    ) -> Result<BTreeMap<ThreeLoopB4D2Column, Coefficient>, ThreeLoopB4D2Error> {
        let column = classify_public_target(&self.family, integral)?;
        if !self.targets.contains(&column) {
            return Err(ThreeLoopB4D2Error::UnexpectedD2Orbit {
                integral: integral.clone(),
            });
        }
        self.reduce_column(&column)
    }

    /// Reduce any retained typed shell column through the replayable triangular
    /// rules.  This is useful for checking native row and transfer identities;
    /// it does not advertise the column as a public target.
    pub fn reduce_column(
        &self,
        column: &ThreeLoopB4D2Column,
    ) -> Result<BTreeMap<ThreeLoopB4D2Column, Coefficient>, ThreeLoopB4D2Error> {
        let rules = self
            .pivots
            .iter()
            .map(|rule| (rule.pivot.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        reduce_by_rules(
            &Arithmetic::new(self.family.coefficients(), self.config)?,
            BTreeMap::from([(column.clone(), self.family.coefficients().one())]),
            &rules,
        )
    }

    /// Regenerate and normalize every authenticated native row, then replay
    /// all stored rows and source-row combinations through the pivot rules.
    pub fn replay(&self) -> Result<(), ThreeLoopB4D2Error> {
        // Rebuild the complete deterministic shell rather than only its raw
        // rows.  Free-column and exceptional-locus metadata are part of the
        // certificate: accepting stale values there would make an otherwise
        // valid row replay advertise the wrong terminal set or generic
        // domain.  Exact work statistics are deterministic for this fixed
        // elimination order and are replayed for the same reason.
        let replay = ShellBuilder::new(self.family.clone(), self.config)?.build()?;
        if replay.seeds != self.seeds
            || replay.rows != self.rows
            || replay.pivots != self.pivots
            || replay.free_columns != self.free_columns
            || replay.nonzero_conditions != self.nonzero_conditions
            || replay.targets != self.targets
            || replay.stats != self.stats
        {
            return Err(ThreeLoopB4D2Error::RawRowReplayMismatch);
        }
        let arithmetic = Arithmetic::new(self.family.coefficients(), self.config)?;
        let rules = self
            .pivots
            .iter()
            .map(|rule| (rule.pivot.clone(), rule))
            .collect::<BTreeMap<_, _>>();
        for row in &self.rows {
            let remainder = reduce_by_rules(&arithmetic, row.entries.clone(), &rules)?;
            if !remainder.is_empty() {
                return Err(ThreeLoopB4D2Error::NormalizedRowRemainder {
                    raw_id: row.raw_id,
                    remainder,
                });
            }
        }
        let rows = self
            .rows
            .iter()
            .map(|row| (row.raw_id, row))
            .collect::<BTreeMap<_, _>>();
        for rule in &self.pivots {
            let mut actual = BTreeMap::new();
            for (raw_id, weight) in &rule.source_row_weights {
                let row = rows
                    .get(raw_id)
                    .ok_or(ThreeLoopB4D2Error::UnknownSourceRow { raw_id: *raw_id })?;
                for (column, coefficient) in &row.entries {
                    let scaled = arithmetic.checked_mul(coefficient, weight)?;
                    arithmetic.add_sparse(&mut actual, column.clone(), scaled)?;
                }
            }
            let mut expected =
                BTreeMap::from([(rule.pivot.clone(), self.family.coefficients().one())]);
            for (column, coefficient) in &rule.rhs {
                arithmetic.add_sparse(&mut expected, column.clone(), -coefficient.clone())?;
            }
            if actual != expected {
                return Err(ThreeLoopB4D2Error::PivotProvenanceMismatch {
                    pivot: rule.pivot.clone(),
                });
            }
            if rule.rhs.keys().any(|column| column >= &rule.pivot) {
                return Err(ThreeLoopB4D2Error::NonTriangularPivot {
                    pivot: rule.pivot.clone(),
                });
            }
        }
        Ok(())
    }
}

struct CollectedRows {
    seeds: Vec<ThreeLoopB4D2Seed>,
    rows: Vec<ThreeLoopB4D2NormalizedRow>,
    conditions: Vec<ThreeLoopB4D2NonzeroCondition>,
    stats: ThreeLoopB4D2Stats,
}

struct ShellBuilder {
    config: ThreeLoopB4D2Config,
    boundary: ThreeLoopBoundaryReducer,
    arithmetic: Arithmetic,
}

impl ShellBuilder {
    fn new(family: VacuumFamily, config: ThreeLoopB4D2Config) -> Result<Self, ThreeLoopB4D2Error> {
        let boundary = ThreeLoopBoundaryReducer::new(family, config.boundary)?;
        let arithmetic = Arithmetic::new(boundary.family().coefficients(), config)?;
        Ok(Self {
            config,
            boundary,
            arithmetic,
        })
    }

    fn build(self) -> Result<ThreeLoopB4D2Shell, ThreeLoopB4D2Error> {
        let family = self.boundary.family().clone();
        let collected = self.collect_rows()?;
        let target_a = scalar_column(&orient_b4(
            &family,
            &ThreeLoopB4D2SeedOrbit::TripleDot.representative(),
        )?)?;
        let target_adjacent = scalar_column(&orient_b4(
            &family,
            &ThreeLoopB4D2SeedOrbit::AdjacentDoubleDot.representative(),
        )?)?;
        let target_opposite = scalar_column(&orient_b4(
            &family,
            &ThreeLoopB4D2SeedOrbit::OppositeDoubleDot.representative(),
        )?)?;
        if [&target_a, &target_adjacent, &target_opposite]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 3
        {
            return Err(ThreeLoopB4D2Error::TargetOrbitCollapse);
        }
        let (pivots, free_columns, conditions, mut stats) = eliminate(
            &self.arithmetic,
            self.config,
            &collected.rows,
            collected.conditions,
            collected.stats,
        )?;
        let pivot_columns = pivots
            .iter()
            .map(|rule| rule.pivot.clone())
            .collect::<BTreeSet<_>>();
        for target in [&target_a, &target_adjacent, &target_opposite] {
            if !pivot_columns.contains(target) {
                return Err(ThreeLoopB4D2Error::MissingTargetPivot {
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
        Ok(ThreeLoopB4D2Shell {
            config: self.config,
            family,
            seeds: collected.seeds,
            rows: collected.rows,
            pivots,
            free_columns,
            nonzero_conditions: conditions,
            targets: [target_a, target_adjacent, target_opposite],
            stats,
        })
    }

    fn collect_rows(&self) -> Result<CollectedRows, ThreeLoopB4D2Error> {
        let family = self.boundary.family();
        let mut seeds = Vec::with_capacity(THREE_LOOP_B4_D2_SEED_ORBITS);
        for orbit in ThreeLoopB4D2SeedOrbit::ALL {
            let integral = orient_b4(family, &orbit.representative())?;
            seeds.push(ThreeLoopB4D2Seed { orbit, integral });
        }
        if seeds
            .iter()
            .map(|seed| seed.integral.clone())
            .collect::<BTreeSet<_>>()
            .len()
            != THREE_LOOP_B4_D2_SEED_ORBITS
        {
            return Err(ThreeLoopB4D2Error::SeedOrbitCollapse);
        }

        let mut rows = Vec::with_capacity(THREE_LOOP_B4_D2_RAW_ROWS);
        let mut conditions = Vec::new();
        let mut stats = ThreeLoopB4D2Stats::default();
        for seed in &seeds {
            let identities = IbpGenerator::new(family).try_generate_raw(&seed.integral)?;
            if identities.len() != LOOPS * LOOPS {
                return Err(ThreeLoopB4D2Error::RawRowCount {
                    expected: LOOPS * LOOPS,
                    actual: identities.len(),
                });
            }
            for identity in identities {
                let differentiated_loop = u8::try_from(identity.differentiated_loop)
                    .map_err(|_| ThreeLoopB4D2Error::RawRowLabelOutOfRange)?;
                let contraction_loop = u8::try_from(identity.contraction_loop)
                    .map_err(|_| ThreeLoopB4D2Error::RawRowLabelOutOfRange)?;
                let raw_id =
                    ThreeLoopB4D2RawRowId::new(seed.orbit, differentiated_loop, contraction_loop);
                if identity.seed != seed.integral
                    || usize::from(differentiated_loop) >= LOOPS
                    || usize::from(contraction_loop) >= LOOPS
                {
                    return Err(ThreeLoopB4D2Error::RawRowProvenance { raw_id });
                }
                stats.raw_rows =
                    checked_add_resource("raw rows", stats.raw_rows, 1, self.config.max_raw_rows)?;
                stats.raw_term_incidences = checked_add_resource(
                    "raw term incidences",
                    stats.raw_term_incidences,
                    identity.equation.len(),
                    self.config.max_raw_term_incidences,
                )?;
                let (mut entries, boundary_calls, symmetry_images) = self.normalize_equation(
                    &identity.equation,
                    seed.integral
                        .powers()
                        .iter()
                        .map(|power| i64::from(*power))
                        .sum(),
                )?;
                stats.boundary_calls = checked_add_resource(
                    "boundary normalization calls",
                    stats.boundary_calls,
                    boundary_calls,
                    self.config.max_boundary_calls,
                )?;
                stats.symmetry_images = checked_add_resource(
                    "B4 symmetry images",
                    stats.symmetry_images,
                    symmetry_images,
                    self.config.max_symmetry_images,
                )?;
                let row_scale = canonicalize_row(
                    &self.arithmetic,
                    &mut entries,
                    Some((&mut conditions, raw_id)),
                )?;
                stats.collected_nonzeros = checked_add_resource(
                    "collected normalized nonzeros",
                    stats.collected_nonzeros,
                    entries.len(),
                    self.config.max_collected_nonzeros,
                )?;
                rows.push(ThreeLoopB4D2NormalizedRow {
                    raw_id,
                    seed_mass_weight: seed
                        .integral
                        .powers()
                        .iter()
                        .map(|power| i64::from(*power))
                        .sum(),
                    row_scale,
                    entries,
                });
            }
        }
        if rows.len() != THREE_LOOP_B4_D2_RAW_ROWS {
            return Err(ThreeLoopB4D2Error::RawRowCount {
                expected: THREE_LOOP_B4_D2_RAW_ROWS,
                actual: rows.len(),
            });
        }
        let global_columns = rows
            .iter()
            .flat_map(|row| row.entries.keys().cloned())
            .collect::<BTreeSet<_>>();
        check_resource(
            "global columns",
            global_columns.len(),
            self.config.max_global_columns,
        )?;
        Ok(CollectedRows {
            seeds,
            rows,
            conditions,
            stats,
        })
    }

    fn normalize_equation(
        &self,
        equation: &LinearCombination,
        seed_weight: i64,
    ) -> Result<(BTreeMap<ThreeLoopB4D2Column, Coefficient>, usize, usize), ThreeLoopB4D2Error>
    {
        let mut entries = BTreeMap::new();
        let mut boundary_calls = 0_usize;
        let mut symmetry_images = 0_usize;
        for (integral, coefficient) in equation.terms() {
            if let Some(boundary_reduction) = self.boundary.try_reduce_integral(integral)? {
                boundary_calls += 1;
                for (terminal, boundary_coefficient) in boundary_reduction.terms() {
                    let column = if terminal == self.boundary.product_master() {
                        ThreeLoopB4D2Column::Boundary(ThreeLoopB4BoundaryColumn::TadpoleCubed)
                    } else if terminal == self.boundary.sunset_times_tadpole_master() {
                        ThreeLoopB4D2Column::Boundary(ThreeLoopB4BoundaryColumn::TadpoleSunset)
                    } else {
                        return Err(ThreeLoopB4D2Error::UnexpectedBoundaryTerminal {
                            integral: terminal.clone(),
                        });
                    };
                    let combined = self
                        .arithmetic
                        .checked_mul(coefficient, boundary_coefficient)?;
                    let normalized = self.arithmetic.mass_normalize(
                        &combined,
                        seed_weight - column.mass_weight(),
                        &column,
                    )?;
                    self.arithmetic
                        .add_sparse(&mut entries, column, normalized)?;
                }
                continue;
            }
            let Some(_) = self.boundary.family().try_canonicalize(integral)? else {
                continue;
            };
            if canonical_sector_mask(self.boundary.family(), integral)? != Some(B4_MASK) {
                return Err(ThreeLoopB4D2Error::UnexpectedGenuineSector {
                    integral: integral.clone(),
                    canonical_mask: canonical_sector_mask(self.boundary.family(), integral)?,
                });
            }
            symmetry_images = symmetry_images
                .checked_add(self.boundary.family().symmetries().len())
                .ok_or(ThreeLoopB4D2Error::ResourceLimit {
                    resource: "B4 symmetry images",
                    requested: u128::MAX,
                    limit: self.config.max_symmetry_images as u128,
                })?;
            let oriented = orient_b4(self.boundary.family(), integral)?;
            let column = b4_column(&oriented)?;
            let normalized = self.arithmetic.mass_normalize(
                coefficient,
                seed_weight - column.mass_weight(),
                &column,
            )?;
            self.arithmetic
                .add_sparse(&mut entries, column, normalized)?;
        }
        Ok((entries, boundary_calls, symmetry_images))
    }
}

#[derive(Clone)]
struct WorkRow {
    entries: BTreeMap<ThreeLoopB4D2Column, Coefficient>,
    source_weights: BTreeMap<ThreeLoopB4D2RawRowId, Coefficient>,
}

fn eliminate(
    arithmetic: &Arithmetic,
    config: ThreeLoopB4D2Config,
    rows: &[ThreeLoopB4D2NormalizedRow],
    mut conditions: Vec<ThreeLoopB4D2NonzeroCondition>,
    mut stats: ThreeLoopB4D2Stats,
) -> Result<
    (
        Vec<ThreeLoopB4D2PivotRule>,
        Vec<ThreeLoopB4D2Column>,
        Vec<ThreeLoopB4D2NonzeroCondition>,
        ThreeLoopB4D2Stats,
    ),
    ThreeLoopB4D2Error,
> {
    let all_columns = rows
        .iter()
        .flat_map(|row| row.entries.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut pivots = BTreeMap::<ThreeLoopB4D2Column, WorkRow>::new();
    for row in rows {
        let mut work = WorkRow {
            entries: row.entries.clone(),
            source_weights: BTreeMap::from([(row.raw_id, arithmetic.context.one())]),
        };
        loop {
            let Some(hardest) = work.entries.last_key_value().map(|(key, _)| key.clone()) else {
                break;
            };
            let Some(pivot) = pivots.get(&hardest) else {
                break;
            };
            let factor = work
                .entries
                .get(&hardest)
                .expect("hardest entry exists")
                .clone();
            add_scaled_work_row(arithmetic, config, &mut work, pivot, &(-factor), &mut stats)?;
        }
        if work.entries.is_empty() {
            continue;
        }
        let pivot = work
            .entries
            .last_key_value()
            .map(|(column, _)| column.clone())
            .expect("nonzero row has a pivot");
        let divisor = work.entries.get(&pivot).unwrap().clone();
        record_condition(
            arithmetic,
            &divisor,
            ThreeLoopB4D2ConditionSource::Pivot(pivot.clone()),
            &mut conditions,
        )?;
        divide_work_row(arithmetic, config, &mut work, &divisor, &mut stats)?;
        pivots.insert(pivot, work);
    }
    let pivot_columns = pivots.keys().cloned().collect::<BTreeSet<_>>();
    let free_columns = all_columns
        .difference(&pivot_columns)
        .cloned()
        .collect::<Vec<_>>();
    let rules = pivots
        .into_iter()
        .rev()
        .map(|(pivot, work)| {
            let mut equation = work.entries;
            equation
                .remove(&pivot)
                .expect("pivot row contains its pivot");
            ThreeLoopB4D2PivotRule {
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
    config: ThreeLoopB4D2Config,
    target: &mut WorkRow,
    source: &WorkRow,
    factor: &Coefficient,
    stats: &mut ThreeLoopB4D2Stats,
) -> Result<(), ThreeLoopB4D2Error> {
    for (column, coefficient) in &source.entries {
        charge_update(config, stats)?;
        let scaled = arithmetic.checked_mul(coefficient, factor)?;
        arithmetic.add_sparse(&mut target.entries, column.clone(), scaled)?;
    }
    for (raw_id, coefficient) in &source.source_weights {
        charge_update(config, stats)?;
        let scaled = arithmetic.checked_mul(coefficient, factor)?;
        arithmetic.add_sparse(&mut target.source_weights, *raw_id, scaled)?;
    }
    Ok(())
}

fn divide_work_row(
    arithmetic: &Arithmetic,
    config: ThreeLoopB4D2Config,
    row: &mut WorkRow,
    divisor: &Coefficient,
    stats: &mut ThreeLoopB4D2Stats,
) -> Result<(), ThreeLoopB4D2Error> {
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
    config: ThreeLoopB4D2Config,
    stats: &mut ThreeLoopB4D2Stats,
) -> Result<(), ThreeLoopB4D2Error> {
    stats.elimination_updates = checked_add_resource(
        "elimination coefficient updates",
        stats.elimination_updates,
        1,
        config.max_elimination_updates,
    )?;
    Ok(())
}

fn canonicalize_row(
    arithmetic: &Arithmetic,
    entries: &mut BTreeMap<ThreeLoopB4D2Column, Coefficient>,
    condition: Option<(
        &mut Vec<ThreeLoopB4D2NonzeroCondition>,
        ThreeLoopB4D2RawRowId,
    )>,
) -> Result<Coefficient, ThreeLoopB4D2Error> {
    let Some(scale) = entries.last_key_value().map(|(_, value)| value.clone()) else {
        return Ok(arithmetic.context.one());
    };
    if let Some((conditions, raw_id)) = condition {
        record_condition(
            arithmetic,
            &scale,
            ThreeLoopB4D2ConditionSource::RawRow(raw_id),
            conditions,
        )?;
    }
    for coefficient in entries.values_mut() {
        *coefficient = arithmetic.checked_div(coefficient, &scale)?;
    }
    Ok(scale)
}

fn record_condition(
    arithmetic: &Arithmetic,
    divisor: &Coefficient,
    source: ThreeLoopB4D2ConditionSource,
    conditions: &mut Vec<ThreeLoopB4D2NonzeroCondition>,
) -> Result<(), ThreeLoopB4D2Error> {
    if divisor.is_zero() {
        return Err(ThreeLoopB4D2Error::ZeroPivot);
    }
    arithmetic.require_mass_independent(divisor, None)?;
    if divisor.numerator.degree(arithmetic.dimension_position) == 0 {
        return Ok(());
    }
    let polynomial = divisor.numerator.to_expression().to_canonical_string();
    let condition = ThreeLoopB4D2NonzeroCondition { source, polynomial };
    if !conditions.contains(&condition) {
        conditions.push(condition);
    }
    Ok(())
}

struct Arithmetic {
    context: CoefficientContext,
    mass: Coefficient,
    mass_position: usize,
    dimension_position: usize,
    max_degree: usize,
}

impl Arithmetic {
    fn new(
        context: &CoefficientContext,
        config: ThreeLoopB4D2Config,
    ) -> Result<Self, ThreeLoopB4D2Error> {
        let mass_position = context
            .parameter_names()
            .iter()
            .position(|name| name == "m2")
            .ok_or(ThreeLoopB4D2Error::MissingParameter { name: "m2" })?;
        let dimension_position = context
            .parameter_names()
            .iter()
            .position(|name| name == "d")
            .ok_or(ThreeLoopB4D2Error::MissingParameter { name: "d" })?;
        let mass = context
            .parameter("m2")
            .ok_or(ThreeLoopB4D2Error::MissingParameter { name: "m2" })?;
        Ok(Self {
            context: context.clone(),
            mass,
            mass_position,
            dimension_position,
            max_degree: config.max_coefficient_degree,
        })
    }

    fn mass_normalize(
        &self,
        coefficient: &Coefficient,
        exponent: i64,
        column: &ThreeLoopB4D2Column,
    ) -> Result<Coefficient, ThreeLoopB4D2Error> {
        let mut output = coefficient.clone();
        if exponent >= 0 {
            for _ in 0..u64::try_from(exponent).expect("nonnegative i64 fits u64") {
                output = self.checked_mul(&output, &self.mass)?;
            }
        } else {
            for _ in 0..exponent.unsigned_abs() {
                output = self.checked_div(&output, &self.mass)?;
            }
        }
        self.require_mass_independent(&output, Some(column.clone()))?;
        Ok(output)
    }

    fn require_mass_independent(
        &self,
        coefficient: &Coefficient,
        column: Option<ThreeLoopB4D2Column>,
    ) -> Result<(), ThreeLoopB4D2Error> {
        let degrees = coefficient_variable_degrees(coefficient);
        let (numerator_degree, denominator_degree) = degrees
            .get(self.mass_position)
            .copied()
            .ok_or(ThreeLoopB4D2Error::MissingParameter { name: "m2" })?;
        if numerator_degree != 0 || denominator_degree != 0 {
            return Err(ThreeLoopB4D2Error::ResidualMassDependence {
                column,
                numerator_degree,
                denominator_degree,
            });
        }
        Ok(())
    }

    fn check_degree(&self, requested: u128) -> Result<(), ThreeLoopB4D2Error> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(ThreeLoopB4D2Error::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        if requested > self.max_degree as u128 {
            return Err(ThreeLoopB4D2Error::ResourceLimit {
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
    ) -> Result<Coefficient, ThreeLoopB4D2Error> {
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        Ok(left * right)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, ThreeLoopB4D2Error> {
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        Ok(left + right)
    }

    fn checked_div(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, ThreeLoopB4D2Error> {
        if right.is_zero() {
            return Err(ThreeLoopB4D2Error::ZeroPivot);
        }
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        Ok(left / right)
    }

    fn add_sparse<K: Ord>(
        &self,
        entries: &mut BTreeMap<K, Coefficient>,
        key: K,
        coefficient: Coefficient,
    ) -> Result<(), ThreeLoopB4D2Error> {
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

fn reduce_by_rules(
    arithmetic: &Arithmetic,
    mut entries: BTreeMap<ThreeLoopB4D2Column, Coefficient>,
    rules: &BTreeMap<ThreeLoopB4D2Column, &ThreeLoopB4D2PivotRule>,
) -> Result<BTreeMap<ThreeLoopB4D2Column, Coefficient>, ThreeLoopB4D2Error> {
    loop {
        let Some((pivot, rule)) = entries
            .keys()
            .rev()
            .find_map(|column| rules.get(column).map(|rule| (column.clone(), *rule)))
        else {
            break;
        };
        let factor = entries.remove(&pivot).expect("selected pivot is present");
        for (column, coefficient) in &rule.rhs {
            let scaled = arithmetic.checked_mul(coefficient, &factor)?;
            arithmetic.add_sparse(&mut entries, column.clone(), scaled)?;
        }
    }
    Ok(entries)
}

fn classify_public_target(
    family: &VacuumFamily,
    integral: &Integral,
) -> Result<ThreeLoopB4D2Column, ThreeLoopB4D2Error> {
    if integral.powers().len() != DENOMINATORS {
        return Err(ThreeLoopB4D2Error::WrongIntegralArity {
            expected: DENOMINATORS,
            actual: integral.powers().len(),
        });
    }
    if integral.powers().iter().any(|power| *power < 0) {
        return Err(ThreeLoopB4D2Error::NumeratorInput {
            integral: integral.clone(),
        });
    }
    if canonical_sector_mask(family, integral)? != Some(B4_MASK) {
        return Err(ThreeLoopB4D2Error::OutsideB4 {
            integral: integral.clone(),
        });
    }
    let oriented = orient_b4(family, integral)?;
    let column = scalar_column(&oriented)?;
    if column.dot_degree() != 2 {
        return Err(ThreeLoopB4D2Error::OutOfCoverage {
            integral: integral.clone(),
            dot_degree: column.dot_degree(),
            expected: 2,
        });
    }
    Ok(column)
}

fn b4_column(integral: &Integral) -> Result<ThreeLoopB4D2Column, ThreeLoopB4D2Error> {
    let powers: [i32; DENOMINATORS] =
        integral
            .powers()
            .try_into()
            .map_err(|_| ThreeLoopB4D2Error::WrongIntegralArity {
                expected: DENOMINATORS,
                actual: integral.powers().len(),
            })?;
    if B4_ACTIVE.iter().any(|position| powers[*position] <= 0)
        || B4_INACTIVE.iter().any(|position| powers[*position] > 0)
    {
        return Err(ThreeLoopB4D2Error::MalformedB4Column {
            integral: integral.clone(),
        });
    }
    if B4_INACTIVE.iter().all(|position| powers[*position] == 0) {
        scalar_column(integral)
    } else {
        let numerator_degree = B4_INACTIVE
            .iter()
            .map(|position| powers[*position].saturating_neg().max(0) as u32)
            .sum::<u32>();
        if numerator_degree != 1 {
            return Err(ThreeLoopB4D2Error::UnexpectedNumeratorHalo {
                integral: integral.clone(),
                numerator_degree,
            });
        }
        Ok(ThreeLoopB4D2Column::Numerator { powers })
    }
}

fn scalar_column(integral: &Integral) -> Result<ThreeLoopB4D2Column, ThreeLoopB4D2Error> {
    let powers = integral.powers();
    if powers.len() != DENOMINATORS
        || B4_ACTIVE.iter().any(|position| powers[*position] <= 0)
        || B4_INACTIVE.iter().any(|position| powers[*position] != 0)
    {
        return Err(ThreeLoopB4D2Error::MalformedB4Column {
            integral: integral.clone(),
        });
    }
    Ok(ThreeLoopB4D2Column::Scalar {
        powers: B4_ACTIVE.map(|position| powers[position]),
    })
}

/// Jointly orient all six powers while fixing the labelled B4 mask.  This is
/// essential for numerator columns: scalar powers and numerator positions may
/// not be canonicalized independently.
fn orient_b4(family: &VacuumFamily, integral: &Integral) -> Result<Integral, ThreeLoopB4D2Error> {
    if integral.powers().len() != DENOMINATORS {
        return Err(ThreeLoopB4D2Error::WrongIntegralArity {
            expected: DENOMINATORS,
            actual: integral.powers().len(),
        });
    }
    family
        .symmetries()
        .iter()
        .map(|permutation| {
            Integral::new(
                permutation
                    .iter()
                    .map(|source| integral.powers()[*source])
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|candidate| sector_mask(candidate) == B4_MASK)
        .max()
        .ok_or_else(|| ThreeLoopB4D2Error::OutsideB4 {
            integral: integral.clone(),
        })
}

fn canonical_sector_mask(
    family: &VacuumFamily,
    integral: &Integral,
) -> Result<Option<u8>, ThreeLoopB4D2Error> {
    let boolean = Integral::new(
        integral
            .powers()
            .iter()
            .map(|power| i32::from(*power > 0))
            .collect::<Vec<_>>(),
    );
    Ok(family
        .try_canonicalize(&boolean)?
        .map(|canonical| sector_mask(&canonical)))
}

fn sector_mask(integral: &Integral) -> u8 {
    integral
        .powers()
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (position, power)| {
            mask | (u8::from(*power > 0) << position)
        })
}

fn preflight_config(config: ThreeLoopB4D2Config) -> Result<(), ThreeLoopB4D2Error> {
    for (resource, requested, limit) in [
        (
            "scalar seed orbits",
            THREE_LOOP_B4_D2_SEED_ORBITS,
            config.max_seed_orbits,
        ),
        ("raw rows", THREE_LOOP_B4_D2_RAW_ROWS, config.max_raw_rows),
        (
            "raw term incidences",
            THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND,
            config.max_raw_term_incidences,
        ),
        (
            "boundary normalization calls",
            THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND,
            config.max_boundary_calls,
        ),
        (
            "B4 symmetry images",
            THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND,
            config.max_symmetry_images,
        ),
        (
            "global columns",
            THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND,
            config.max_global_columns,
        ),
        (
            "collected normalized nonzeros",
            THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND,
            config.max_collected_nonzeros,
        ),
        (
            "elimination coefficient updates",
            THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND,
            config.max_elimination_updates,
        ),
        (
            "source-row provenance weights",
            THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND,
            config.max_source_row_weights,
        ),
    ] {
        check_resource(resource, requested, limit)?;
    }
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(ThreeLoopB4D2Error::ResourceLimit {
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
) -> Result<(), ThreeLoopB4D2Error> {
    if requested > limit {
        Err(ThreeLoopB4D2Error::ResourceLimit {
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
) -> Result<usize, ThreeLoopB4D2Error> {
    let requested = current
        .checked_add(increment)
        .ok_or(ThreeLoopB4D2Error::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        })?;
    check_resource(resource, requested, limit)?;
    Ok(requested)
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

#[derive(Debug)]
pub enum ThreeLoopB4D2Error {
    Family(FamilyError),
    Boundary(ThreeLoopBoundaryError),
    Ibp(IbpGenerationError),
    MissingParameter {
        name: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    NumeratorInput {
        integral: Integral,
    },
    OutsideB4 {
        integral: Integral,
    },
    OutOfCoverage {
        integral: Integral,
        dot_degree: u64,
        expected: u64,
    },
    UnexpectedD2Orbit {
        integral: Integral,
    },
    MalformedB4Column {
        integral: Integral,
    },
    UnexpectedNumeratorHalo {
        integral: Integral,
        numerator_degree: u32,
    },
    UnexpectedGenuineSector {
        integral: Integral,
        canonical_mask: Option<u8>,
    },
    UnexpectedBoundaryTerminal {
        integral: Integral,
    },
    RawRowCount {
        expected: usize,
        actual: usize,
    },
    RawRowLabelOutOfRange,
    RawRowProvenance {
        raw_id: ThreeLoopB4D2RawRowId,
    },
    SeedOrbitCollapse,
    TargetOrbitCollapse,
    MissingTargetPivot {
        target: ThreeLoopB4D2Column,
    },
    ZeroPivot,
    ResidualMassDependence {
        column: Option<ThreeLoopB4D2Column>,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    RawRowReplayMismatch,
    NormalizedRowRemainder {
        raw_id: ThreeLoopB4D2RawRowId,
        remainder: BTreeMap<ThreeLoopB4D2Column, Coefficient>,
    },
    UnknownSourceRow {
        raw_id: ThreeLoopB4D2RawRowId,
    },
    PivotProvenanceMismatch {
        pivot: ThreeLoopB4D2Column,
    },
    NonTriangularPivot {
        pivot: ThreeLoopB4D2Column,
    },
}

impl From<FamilyError> for ThreeLoopB4D2Error {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<ThreeLoopBoundaryError> for ThreeLoopB4D2Error {
    fn from(error: ThreeLoopBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

impl From<IbpGenerationError> for ThreeLoopB4D2Error {
    fn from(error: IbpGenerationError) -> Self {
        Self::Ibp(error)
    }
}

impl fmt::Display for ThreeLoopB4D2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "B4 D2 family: {error}"),
            Self::Boundary(error) => write!(formatter, "B4 D2 boundary: {error}"),
            Self::Ibp(error) => write!(formatter, "B4 D2 native IBP: {error}"),
            Self::MissingParameter { name } => write!(formatter, "B4 D2 context is missing {name}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "B4 D2 {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "B4 D2 integral has {actual} powers; expected {expected}"
            ),
            Self::NumeratorInput { integral } => write!(
                formatter,
                "B4 D2 public scalar input {integral} contains a numerator"
            ),
            Self::OutsideB4 { integral } => {
                write!(formatter, "{integral} is outside the B4 sector orbit")
            }
            Self::OutOfCoverage {
                integral,
                dot_degree,
                expected,
            } => write!(
                formatter,
                "B4 target {integral} has dot degree {dot_degree}; this shell accepts exactly {expected}"
            ),
            Self::UnexpectedD2Orbit { integral } => {
                write!(formatter, "unrecognized scalar B4 D2 orbit {integral}")
            }
            Self::MalformedB4Column { integral } => {
                write!(formatter, "malformed oriented B4 column {integral}")
            }
            Self::UnexpectedNumeratorHalo {
                integral,
                numerator_degree,
            } => write!(
                formatter,
                "B4 row produced numerator degree {numerator_degree} outside its one-step halo: {integral}"
            ),
            Self::UnexpectedGenuineSector {
                integral,
                canonical_mask,
            } => write!(
                formatter,
                "B4 row reached unsupported genuine sector {canonical_mask:?}: {integral}"
            ),
            Self::UnexpectedBoundaryTerminal { integral } => write!(
                formatter,
                "B4 boundary returned unknown terminal {integral}"
            ),
            Self::RawRowCount { expected, actual } => write!(
                formatter,
                "B4 D2 generated {actual} raw rows; expected {expected}"
            ),
            Self::RawRowLabelOutOfRange => {
                formatter.write_str("B4 D2 raw row label does not fit u8")
            }
            Self::RawRowProvenance { raw_id } => write!(
                formatter,
                "B4 D2 row {} does not replay its origin",
                raw_id.stable_key()
            ),
            Self::SeedOrbitCollapse => {
                formatter.write_str("B4 scalar seed orbits collapsed under the proved stabilizer")
            }
            Self::TargetOrbitCollapse => {
                formatter.write_str("distinct B4 D2 scalar target orbits collapsed")
            }
            Self::MissingTargetPivot { target } => write!(
                formatter,
                "B4 D2 shell did not pivot target {}",
                target.stable_key()
            ),
            Self::ZeroPivot => {
                formatter.write_str("B4 D2 exact elimination attempted division by zero")
            }
            Self::ResidualMassDependence {
                column,
                numerator_degree,
                denominator_degree,
            } => write!(
                formatter,
                "B4 D2 normalized coefficient for {column:?} retains m2 degrees ({numerator_degree},{denominator_degree})"
            ),
            Self::RawRowReplayMismatch => formatter
                .write_str("B4 D2 deterministic certificate artifacts did not regenerate exactly"),
            Self::NormalizedRowRemainder { raw_id, .. } => write!(
                formatter,
                "B4 D2 row {} has a nonzero replay remainder",
                raw_id.stable_key()
            ),
            Self::UnknownSourceRow { raw_id } => write!(
                formatter,
                "B4 D2 pivot references unknown row {}",
                raw_id.stable_key()
            ),
            Self::PivotProvenanceMismatch { pivot } => write!(
                formatter,
                "B4 D2 source weights do not reconstruct pivot {}",
                pivot.stable_key()
            ),
            Self::NonTriangularPivot { pivot } => write!(
                formatter,
                "B4 D2 pivot {} is not triangular",
                pivot.stable_key()
            ),
        }
    }
}

impl Error for ThreeLoopB4D2Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Family(error) => Some(error),
            Self::Boundary(error) => Some(error),
            Self::Ibp(error) => Some(error),
            _ => None,
        }
    }
}
