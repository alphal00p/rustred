//! Exact origin manifest for the first four-loop shell beyond scalar corners.
//!
//! This module freezes the deterministic 123-seed prefix selected by
//! `tools/four_loop_next_shell_rank.rs`: ten scalar corners, all 72 one-dot
//! seeds, all 28 one-numerator seeds, and the selected thirteen mixed seeds.
//! It authenticates the H/X reference topology of every corner and regenerates
//! all sixteen native `partial_(k_i).k_j` origins per seed.
//!
//! No affine halo normalization or sparse elimination is performed here.
//! In particular, the finite-field probe's higher-boundary column census is
//! deliberately not part of this exact manifest.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    FourLoopGenuineClassifier, FourLoopGenuineConfig, FourLoopGenuineCornerType,
    FourLoopGenuineError, FourLoopTopology, IbpGenerationError, IbpGenerator, Integral,
    four_loop_corner_seed,
};

const LOOPS: usize = 4;
const BASIS: usize = 10;

pub const FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS: usize = 10;
pub const FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS: usize = 72;
pub const FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS: usize = 28;
pub const FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS: usize = 13;
pub const FOUR_LOOP_NEXT_MANIFEST_SEEDS: usize = 123;
pub const FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS: usize = 1_968;
pub const FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES: usize = 927;
pub const FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND: usize = 163_644;
/// FNV-1a-64 over the ordered 123 seed stable keys, including one trailing
/// newline after every key.
pub const FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM: u64 = 0x0bff_80d5_dddb_4340;

const FNV1A64_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The four frozen layers of the selected prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FourLoopNextSeedPhase {
    Corner,
    Dot,
    Numerator,
    Mixed,
}

impl FourLoopNextSeedPhase {
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Corner => "corner",
            Self::Dot => "dot",
            Self::Numerator => "numerator",
            Self::Mixed => "mixed-prefix-13",
        }
    }

    pub const fn seed_count(self) -> usize {
        match self {
            Self::Corner => FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS,
            Self::Dot => FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
            Self::Numerator => FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS,
            Self::Mixed => FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS,
        }
    }
}

/// Stable phase/corner/power identifier for one selected seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopNextSeedId {
    phase: FourLoopNextSeedPhase,
    phase_index: u16,
    corner_type: FourLoopGenuineCornerType,
    powers: [i32; BASIS],
}

impl FourLoopNextSeedId {
    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-four-loop-next-seed-v1";

    const fn new(
        phase: FourLoopNextSeedPhase,
        phase_index: u16,
        corner_type: FourLoopGenuineCornerType,
        powers: [i32; BASIS],
    ) -> Self {
        Self {
            phase,
            phase_index,
            corner_type,
            powers,
        }
    }

    pub const fn phase(self) -> FourLoopNextSeedPhase {
        self.phase
    }

    pub const fn phase_index(self) -> u16 {
        self.phase_index
    }

    pub const fn corner_type(self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub const fn topology(self) -> FourLoopTopology {
        self.corner_type.reference_topology()
    }

    pub const fn powers(&self) -> &[i32; BASIS] {
        &self.powers
    }

    pub fn integral(self) -> Integral {
        Integral::from(self.powers)
    }

    pub fn stable_key(self) -> String {
        let powers = self
            .powers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{}:{}:{:03}:{}:[{}]",
            Self::SCHEMA,
            self.phase.stable_key(),
            self.phase_index,
            self.corner_type.stable_key(),
            powers
        )
    }
}

/// Stable provenance for one of the 123-by-16 native raw origins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopNextRawRowId {
    seed: FourLoopNextSeedId,
    differentiated_loop: u8,
    contraction_loop: u8,
}

impl FourLoopNextRawRowId {
    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-four-loop-next-raw-row-v1";

    pub const fn new(
        seed: FourLoopNextSeedId,
        differentiated_loop: u8,
        contraction_loop: u8,
    ) -> Result<Self, FourLoopNextRawRowIdError> {
        if differentiated_loop >= LOOPS as u8 {
            return Err(FourLoopNextRawRowIdError::DifferentiatedLoopOutOfRange {
                actual: differentiated_loop,
            });
        }
        if contraction_loop >= LOOPS as u8 {
            return Err(FourLoopNextRawRowIdError::ContractionLoopOutOfRange {
                actual: contraction_loop,
            });
        }
        Ok(Self::new_unchecked(
            seed,
            differentiated_loop,
            contraction_loop,
        ))
    }

    const fn new_unchecked(
        seed: FourLoopNextSeedId,
        differentiated_loop: u8,
        contraction_loop: u8,
    ) -> Self {
        Self {
            seed,
            differentiated_loop,
            contraction_loop,
        }
    }

    pub const fn seed(self) -> FourLoopNextSeedId {
        self.seed
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
            self.seed.stable_key(),
            self.differentiated_loop,
            self.contraction_loop
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextRawRowIdError {
    DifferentiatedLoopOutOfRange { actual: u8 },
    ContractionLoopOutOfRange { actual: u8 },
}

impl fmt::Display for FourLoopNextRawRowIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentiatedLoopOutOfRange { actual } => write!(
                formatter,
                "differentiated loop {actual} is outside the four-loop range 0..4"
            ),
            Self::ContractionLoopOutOfRange { actual } => write!(
                formatter,
                "contraction loop {actual} is outside the four-loop range 0..4"
            ),
        }
    }
}

impl Error for FourLoopNextRawRowIdError {}

/// Honest state of this staged production layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextManifestStatus {
    /// Seeds, topology witnesses, and raw origins are exact; affine/boundary
    /// normalization and elimination remain pending.
    ExactOriginsNormalizationPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextManifestConfig {
    pub genuine: FourLoopGenuineConfig,
    pub max_seeds: usize,
    pub max_raw_rows: usize,
    pub max_nonzero_seed_entries: usize,
    pub max_raw_term_incidences: usize,
}

impl Default for FourLoopNextManifestConfig {
    fn default() -> Self {
        Self {
            genuine: FourLoopGenuineConfig::default(),
            max_seeds: FOUR_LOOP_NEXT_MANIFEST_SEEDS,
            max_raw_rows: FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
            max_nonzero_seed_entries: FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
            max_raw_term_incidences: FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND,
        }
    }
}

/// Topology-authenticated exact manifest.  Raw equations are regenerated and
/// checked during construction, but are not retained as a false normalized
/// certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextManifest {
    config: FourLoopNextManifestConfig,
    seeds: Vec<FourLoopNextSeedId>,
    seed_checksum: u64,
    raw_row_ids: Vec<FourLoopNextRawRowId>,
    native_collected_terms: usize,
}

impl FourLoopNextManifest {
    pub fn build(config: FourLoopNextManifestConfig) -> Result<Self, FourLoopNextManifestError> {
        preflight(config)?;
        let seeds = frozen_seeds()?;
        let seed_checksum = authenticate_seed_shapes(&seeds)?;

        let h = FourLoopGenuineClassifier::build(FourLoopTopology::H, config.genuine)?;
        let x = FourLoopGenuineClassifier::build(FourLoopTopology::X, config.genuine)?;
        authenticate_reference_corners(&h, &x)?;

        let mut raw_row_ids = Vec::with_capacity(FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS);
        let mut native_collected_terms = 0_usize;
        for seed in &seeds {
            let classifier = match seed.topology() {
                FourLoopTopology::H => &h,
                FourLoopTopology::X => &x,
                FourLoopTopology::Bmw | FourLoopTopology::Fg => {
                    return Err(FourLoopNextManifestError::NonReferenceTopology {
                        topology: seed.topology(),
                    });
                }
            };
            let integral = seed.integral();
            let identities = IbpGenerator::new(classifier.family()).try_generate_raw(&integral)?;
            if identities.len() != LOOPS * LOOPS {
                return Err(FourLoopNextManifestError::RawRowCountMismatch {
                    expected: LOOPS * LOOPS,
                    actual: identities.len(),
                });
            }
            for identity in identities {
                let differentiated_loop = u8::try_from(identity.differentiated_loop)
                    .map_err(|_| FourLoopNextManifestError::RawRowLabelOutOfRange)?;
                let contraction_loop = u8::try_from(identity.contraction_loop)
                    .map_err(|_| FourLoopNextManifestError::RawRowLabelOutOfRange)?;
                if usize::from(differentiated_loop) >= LOOPS
                    || usize::from(contraction_loop) >= LOOPS
                {
                    return Err(FourLoopNextManifestError::RawRowLabelOutOfRange);
                }
                let raw_id = FourLoopNextRawRowId::new_unchecked(
                    *seed,
                    differentiated_loop,
                    contraction_loop,
                );
                if identity.seed != integral {
                    return Err(FourLoopNextManifestError::RawRowProvenanceMismatch { raw_id });
                }
                native_collected_terms = native_collected_terms
                    .checked_add(identity.equation.len())
                    .ok_or(FourLoopNextManifestError::CountOverflow)?;
                raw_row_ids.push(raw_id);
            }
        }
        if raw_row_ids.len() != FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS {
            return Err(FourLoopNextManifestError::RawRowCountMismatch {
                expected: FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
                actual: raw_row_ids.len(),
            });
        }
        if raw_row_ids.iter().copied().collect::<BTreeSet<_>>().len() != raw_row_ids.len() {
            return Err(FourLoopNextManifestError::DuplicateRawRowId);
        }
        Ok(Self {
            config,
            seeds,
            seed_checksum,
            raw_row_ids,
            native_collected_terms,
        })
    }

    pub const fn config(&self) -> FourLoopNextManifestConfig {
        self.config
    }

    pub const fn status(&self) -> FourLoopNextManifestStatus {
        FourLoopNextManifestStatus::ExactOriginsNormalizationPending
    }

    pub fn seeds(&self) -> &[FourLoopNextSeedId] {
        &self.seeds
    }

    /// Stable FNV-1a-64 digest of `seed.stable_key() + "\n"` for every seed
    /// in manifest order.
    pub const fn seed_checksum(&self) -> u64 {
        self.seed_checksum
    }

    pub fn raw_row_ids(&self) -> &[FourLoopNextRawRowId] {
        &self.raw_row_ids
    }

    /// Exact number of nonzero terms after native generator collection, prior
    /// to any affine mapping or boundary normalization.
    pub const fn native_collected_terms(&self) -> usize {
        self.native_collected_terms
    }

    pub fn replay(&self) -> Result<(), FourLoopNextManifestError> {
        let rebuilt = Self::build(self.config)?;
        if rebuilt == *self {
            Ok(())
        } else {
            Err(FourLoopNextManifestError::ReplayMismatch)
        }
    }
}

#[derive(Clone, Copy)]
struct AxisDescriptor {
    corner_type: FourLoopGenuineCornerType,
    position: usize,
}

#[derive(Clone, Copy)]
struct MixedDescriptor {
    corner_type: FourLoopGenuineCornerType,
    dot_position: usize,
    numerator_position: usize,
}

use FourLoopGenuineCornerType as C;

const DOT_ORDER: [AxisDescriptor; FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS] = [
    a(C::XNineLine, 5),
    a(C::XNineLine, 6),
    a(C::XNineLine, 7),
    a(C::XNineLine, 8),
    a(C::EightLineB, 6),
    a(C::EightLineB, 7),
    a(C::SevenLineC, 3),
    a(C::SevenLineC, 4),
    a(C::SevenLineC, 6),
    a(C::SevenLineC, 7),
    a(C::SevenLineB, 4),
    a(C::SixLineB, 1),
    a(C::SixLineB, 2),
    a(C::SixLineB, 3),
    a(C::SixLineB, 6),
    a(C::SixLineB, 7),
    a(C::SixLineA, 3),
    a(C::SixLineA, 6),
    a(C::FiveLine, 1),
    a(C::FiveLine, 3),
    a(C::FiveLine, 5),
    a(C::FiveLine, 6),
    a(C::XNineLine, 0),
    a(C::XNineLine, 1),
    a(C::XNineLine, 2),
    a(C::XNineLine, 3),
    a(C::XNineLine, 4),
    a(C::HNineLine, 0),
    a(C::HNineLine, 1),
    a(C::HNineLine, 2),
    a(C::HNineLine, 3),
    a(C::HNineLine, 4),
    a(C::HNineLine, 5),
    a(C::HNineLine, 6),
    a(C::HNineLine, 7),
    a(C::HNineLine, 8),
    a(C::EightLineB, 0),
    a(C::EightLineB, 1),
    a(C::EightLineB, 2),
    a(C::EightLineB, 3),
    a(C::EightLineB, 4),
    a(C::EightLineB, 5),
    a(C::EightLineA, 0),
    a(C::EightLineA, 1),
    a(C::EightLineA, 2),
    a(C::EightLineA, 3),
    a(C::EightLineA, 4),
    a(C::EightLineA, 5),
    a(C::EightLineA, 6),
    a(C::EightLineA, 8),
    a(C::SevenLineC, 0),
    a(C::SevenLineC, 1),
    a(C::SevenLineC, 2),
    a(C::SevenLineB, 0),
    a(C::SevenLineB, 1),
    a(C::SevenLineB, 2),
    a(C::SevenLineB, 3),
    a(C::SevenLineB, 5),
    a(C::SevenLineB, 6),
    a(C::SevenLineA, 0),
    a(C::SevenLineA, 1),
    a(C::SevenLineA, 2),
    a(C::SevenLineA, 3),
    a(C::SevenLineA, 4),
    a(C::SevenLineA, 5),
    a(C::SevenLineA, 8),
    a(C::SixLineB, 0),
    a(C::SixLineA, 0),
    a(C::SixLineA, 1),
    a(C::SixLineA, 2),
    a(C::SixLineA, 5),
    a(C::FiveLine, 0),
];

const NUMERATOR_ORDER: [AxisDescriptor; FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS] = [
    a(C::XNineLine, 9),
    a(C::HNineLine, 9),
    a(C::EightLineB, 9),
    a(C::EightLineB, 8),
    a(C::EightLineA, 9),
    a(C::EightLineA, 7),
    a(C::SevenLineC, 9),
    a(C::SevenLineC, 8),
    a(C::SevenLineC, 5),
    a(C::SevenLineB, 9),
    a(C::SevenLineB, 8),
    a(C::SevenLineB, 7),
    a(C::SevenLineA, 9),
    a(C::SevenLineA, 7),
    a(C::SevenLineA, 6),
    a(C::SixLineB, 9),
    a(C::SixLineB, 8),
    a(C::SixLineB, 5),
    a(C::SixLineB, 4),
    a(C::SixLineA, 9),
    a(C::SixLineA, 8),
    a(C::SixLineA, 7),
    a(C::SixLineA, 4),
    a(C::FiveLine, 9),
    a(C::FiveLine, 8),
    a(C::FiveLine, 7),
    a(C::FiveLine, 4),
    a(C::FiveLine, 2),
];

const MIXED_ORDER: [MixedDescriptor; FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS] = [
    m(C::EightLineA, 6, 9),
    m(C::EightLineA, 6, 7),
    m(C::EightLineA, 8, 7),
    m(C::SevenLineC, 4, 9),
    m(C::SevenLineC, 4, 5),
    m(C::SevenLineC, 6, 9),
    m(C::SevenLineB, 1, 7),
    m(C::SevenLineB, 5, 7),
    m(C::SevenLineB, 6, 9),
    m(C::SevenLineB, 6, 7),
    m(C::SevenLineA, 5, 7),
    m(C::SevenLineA, 8, 7),
    m(C::SevenLineA, 8, 6),
];

const fn a(corner_type: FourLoopGenuineCornerType, position: usize) -> AxisDescriptor {
    AxisDescriptor {
        corner_type,
        position,
    }
}

const fn m(
    corner_type: FourLoopGenuineCornerType,
    dot_position: usize,
    numerator_position: usize,
) -> MixedDescriptor {
    MixedDescriptor {
        corner_type,
        dot_position,
        numerator_position,
    }
}

fn frozen_seeds() -> Result<Vec<FourLoopNextSeedId>, FourLoopNextManifestError> {
    let mut seeds = Vec::with_capacity(FOUR_LOOP_NEXT_MANIFEST_SEEDS);
    for (index, corner_type) in FourLoopGenuineCornerType::ALL.into_iter().enumerate() {
        seeds.push(FourLoopNextSeedId::new(
            FourLoopNextSeedPhase::Corner,
            u16::try_from(index).map_err(|_| FourLoopNextManifestError::CountOverflow)?,
            corner_type,
            corner_powers(corner_type),
        ));
    }
    for (index, descriptor) in DOT_ORDER.into_iter().enumerate() {
        let mut powers = corner_powers(descriptor.corner_type);
        powers[descriptor.position] = 2;
        seeds.push(FourLoopNextSeedId::new(
            FourLoopNextSeedPhase::Dot,
            u16::try_from(index).map_err(|_| FourLoopNextManifestError::CountOverflow)?,
            descriptor.corner_type,
            powers,
        ));
    }
    for (index, descriptor) in NUMERATOR_ORDER.into_iter().enumerate() {
        let mut powers = corner_powers(descriptor.corner_type);
        powers[descriptor.position] = -1;
        seeds.push(FourLoopNextSeedId::new(
            FourLoopNextSeedPhase::Numerator,
            u16::try_from(index).map_err(|_| FourLoopNextManifestError::CountOverflow)?,
            descriptor.corner_type,
            powers,
        ));
    }
    for (index, descriptor) in MIXED_ORDER.into_iter().enumerate() {
        let mut powers = corner_powers(descriptor.corner_type);
        powers[descriptor.dot_position] = 2;
        powers[descriptor.numerator_position] = -1;
        seeds.push(FourLoopNextSeedId::new(
            FourLoopNextSeedPhase::Mixed,
            u16::try_from(index).map_err(|_| FourLoopNextManifestError::CountOverflow)?,
            descriptor.corner_type,
            powers,
        ));
    }
    if seeds.len() != FOUR_LOOP_NEXT_MANIFEST_SEEDS {
        return Err(FourLoopNextManifestError::SeedCountMismatch {
            expected: FOUR_LOOP_NEXT_MANIFEST_SEEDS,
            actual: seeds.len(),
        });
    }
    Ok(seeds)
}

const fn corner_powers(corner_type: FourLoopGenuineCornerType) -> [i32; BASIS] {
    let mask = corner_type.reference_mask();
    let mut powers = [0_i32; BASIS];
    let mut position = 0;
    while position < BASIS {
        powers[position] = if mask & (1_u16 << position) != 0 {
            1
        } else {
            0
        };
        position += 1;
    }
    powers
}

fn authenticate_seed_shapes(
    seeds: &[FourLoopNextSeedId],
) -> Result<u64, FourLoopNextManifestError> {
    let unique = seeds.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != seeds.len() {
        return Err(FourLoopNextManifestError::DuplicateSeedId);
    }
    let mut phase_counts = [0_usize; 4];
    let mut nonzero_entries = 0_usize;
    for seed in seeds {
        let phase_slot = match seed.phase {
            FourLoopNextSeedPhase::Corner => 0,
            FourLoopNextSeedPhase::Dot => 1,
            FourLoopNextSeedPhase::Numerator => 2,
            FourLoopNextSeedPhase::Mixed => 3,
        };
        if usize::from(seed.phase_index) != phase_counts[phase_slot] {
            return Err(FourLoopNextManifestError::NonContiguousPhaseIndex { seed: *seed });
        }
        phase_counts[phase_slot] += 1;
        let corner = corner_powers(seed.corner_type);
        let dots = (0..BASIS)
            .filter(|&position| corner[position] == 1 && seed.powers[position] == 2)
            .count();
        let numerators = seed.powers.iter().filter(|&&power| power == -1).count();
        let valid_entries = (0..BASIS).all(|position| match corner[position] {
            1 => seed.powers[position] == 1 || seed.powers[position] == 2,
            0 => seed.powers[position] == 0 || seed.powers[position] == -1,
            _ => false,
        });
        let expected = match seed.phase {
            FourLoopNextSeedPhase::Corner => (0, 0),
            FourLoopNextSeedPhase::Dot => (1, 0),
            FourLoopNextSeedPhase::Numerator => (0, 1),
            FourLoopNextSeedPhase::Mixed => (1, 1),
        };
        if !valid_entries || (dots, numerators) != expected {
            return Err(FourLoopNextManifestError::SeedShapeMismatch { seed: *seed });
        }
        nonzero_entries = nonzero_entries
            .checked_add(seed.powers.iter().filter(|&&power| power != 0).count())
            .ok_or(FourLoopNextManifestError::CountOverflow)?;
    }
    if phase_counts
        != [
            FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS,
            FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS,
        ]
    {
        return Err(FourLoopNextManifestError::PhaseCountMismatch {
            actual: phase_counts,
        });
    }
    if nonzero_entries != FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES {
        return Err(FourLoopNextManifestError::NonzeroEntryCountMismatch {
            expected: FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
            actual: nonzero_entries,
        });
    }
    let checksum = ordered_seed_checksum(seeds);
    if checksum != FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM {
        return Err(FourLoopNextManifestError::SeedChecksumMismatch {
            expected: FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
            actual: checksum,
        });
    }
    Ok(checksum)
}

fn ordered_seed_checksum(seeds: &[FourLoopNextSeedId]) -> u64 {
    let mut checksum = FNV1A64_OFFSET;
    for seed in seeds {
        for byte in seed.stable_key().bytes().chain([b'\n']) {
            checksum ^= u64::from(byte);
            checksum = checksum.wrapping_mul(FNV1A64_PRIME);
        }
    }
    checksum
}

fn authenticate_reference_corners(
    h: &FourLoopGenuineClassifier,
    x: &FourLoopGenuineClassifier,
) -> Result<(), FourLoopNextManifestError> {
    for corner_type in FourLoopGenuineCornerType::ALL {
        let classifier = if corner_type.reference_topology() == FourLoopTopology::H {
            h
        } else {
            x
        };
        let class = classifier.classify_integral(&four_loop_corner_seed(corner_type))?;
        if class.corner_type() != corner_type
            || class.witness().reference_topology() != corner_type.reference_topology()
            || class.witness().reference_sector_mask() != corner_type.reference_mask()
        {
            return Err(FourLoopNextManifestError::TopologyWitnessMismatch { corner_type });
        }
    }
    Ok(())
}

fn preflight(config: FourLoopNextManifestConfig) -> Result<(), FourLoopNextManifestError> {
    for (resource, requested, limit) in [
        (
            "selected seeds",
            FOUR_LOOP_NEXT_MANIFEST_SEEDS,
            config.max_seeds,
        ),
        (
            "native raw rows",
            FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
            config.max_raw_rows,
        ),
        (
            "nonzero seed entries",
            FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
            config.max_nonzero_seed_entries,
        ),
        (
            "raw term incidences",
            FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND,
            config.max_raw_term_incidences,
        ),
    ] {
        if requested > limit {
            return Err(FourLoopNextManifestError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum FourLoopNextManifestError {
    Genuine(FourLoopGenuineError),
    Ibp(IbpGenerationError),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    NonReferenceTopology {
        topology: FourLoopTopology,
    },
    SeedCountMismatch {
        expected: usize,
        actual: usize,
    },
    PhaseCountMismatch {
        actual: [usize; 4],
    },
    NonzeroEntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    SeedChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    RawRowCountMismatch {
        expected: usize,
        actual: usize,
    },
    DuplicateSeedId,
    DuplicateRawRowId,
    NonContiguousPhaseIndex {
        seed: FourLoopNextSeedId,
    },
    SeedShapeMismatch {
        seed: FourLoopNextSeedId,
    },
    TopologyWitnessMismatch {
        corner_type: FourLoopGenuineCornerType,
    },
    RawRowLabelOutOfRange,
    RawRowProvenanceMismatch {
        raw_id: FourLoopNextRawRowId,
    },
    CountOverflow,
    ReplayMismatch,
}

impl From<FourLoopGenuineError> for FourLoopNextManifestError {
    fn from(error: FourLoopGenuineError) -> Self {
        Self::Genuine(error)
    }
}

impl From<IbpGenerationError> for FourLoopNextManifestError {
    fn from(error: IbpGenerationError) -> Self {
        Self::Ibp(error)
    }
}

impl fmt::Display for FourLoopNextManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Genuine(error) => write!(formatter, "four-loop next-shell atlas: {error}"),
            Self::Ibp(error) => write!(formatter, "four-loop next-shell IBP: {error}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop next-shell {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::NonReferenceTopology { topology } => write!(
                formatter,
                "non-reference topology {topology:?} in H/X next-shell manifest"
            ),
            Self::SeedCountMismatch { expected, actual } => write!(
                formatter,
                "next-shell manifest has {actual} seeds; expected {expected}"
            ),
            Self::PhaseCountMismatch { actual } => {
                write!(formatter, "next-shell phase census mismatch: {actual:?}")
            }
            Self::NonzeroEntryCountMismatch { expected, actual } => write!(
                formatter,
                "next-shell seeds have {actual} nonzero powers; expected {expected}"
            ),
            Self::SeedChecksumMismatch { expected, actual } => write!(
                formatter,
                "next-shell seed checksum is fnv1a64:{actual:016x}; expected fnv1a64:{expected:016x}"
            ),
            Self::RawRowCountMismatch { expected, actual } => write!(
                formatter,
                "next-shell generated {actual} raw rows; expected {expected}"
            ),
            Self::DuplicateSeedId => {
                formatter.write_str("next-shell manifest contains a duplicate seed ID")
            }
            Self::DuplicateRawRowId => {
                formatter.write_str("next-shell manifest contains a duplicate raw-row ID")
            }
            Self::NonContiguousPhaseIndex { seed } => write!(
                formatter,
                "non-contiguous phase index in {}",
                seed.stable_key()
            ),
            Self::SeedShapeMismatch { seed } => {
                write!(formatter, "phase/power mismatch in {}", seed.stable_key())
            }
            Self::TopologyWitnessMismatch { corner_type } => write!(
                formatter,
                "reference topology witness mismatch for {corner_type}"
            ),
            Self::RawRowLabelOutOfRange => {
                formatter.write_str("next-shell raw-row derivative label does not fit u8")
            }
            Self::RawRowProvenanceMismatch { raw_id } => write!(
                formatter,
                "native generator did not replay {}",
                raw_id.stable_key()
            ),
            Self::CountOverflow => formatter.write_str("next-shell structural counter overflowed"),
            Self::ReplayMismatch => {
                formatter.write_str("rebuilt next-shell manifest differs from stored exact origins")
            }
        }
    }
}

impl Error for FourLoopNextManifestError {}
