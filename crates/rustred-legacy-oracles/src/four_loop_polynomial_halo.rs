//! Exact degree-two affine transport for the selected four-loop next shell.
//!
//! [`FourLoopHaloMapper`](crate::FourLoopHaloMapper) proves the ten affine
//! source-basis images needed to transport a genuine corner.  This module
//! composes those already authenticated images into constant, linear, or
//! quadratic denominator polynomials.  It accepts the raw-term shapes emitted
//! from a `(D,N) <= (1,1)` seed and therefore covers the `(D,N) <= (2,2)`
//! dependency halo of the frozen 123-seed manifest.
//!
//! The result is deliberately only a transport certificate.  Same-mask
//! branches remain genuine four-loop columns and strictly lower-mask branches
//! still require scaleless, factorized tensor, or further genuine-sector
//! dispatch.  This module performs none of those reductions and makes no rank
//! or 910-key boundary-census claim.

use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt;

use crate::{
    FourLoopAffineDenominatorImage, FourLoopGenuineClassifier, FourLoopGenuineCornerType,
    FourLoopGenuineWitness, FourLoopHaloConfig, FourLoopHaloError, FourLoopHaloMapper,
    FourLoopNextRawRowId, FourLoopTopology, equal_mass_four_loop_vacuum,
};
use rustred::{
    Coefficient, IbpGenerationError, IbpGenerator, IbpIdentity, Integral, LinearCombination,
    VacuumFamily,
};

const BASIS: usize = 10;

/// Maximum numerator-factor count reached from a selected `(D,N) <= (1,1)`
/// seed by one native IBP shift.
pub const FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS: usize = 2;
/// One affine factor contains one constant and ten denominator coordinates.
pub const FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS: usize = BASIS + 1;
/// Conservative uncollected product count for two eleven-term factors.
pub const FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS: usize =
    FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS * FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS;
/// Number of degree-at-most-two monomials in ten commuting denominator
/// coordinates: `binomial(10 + 2, 2) = 66`.
pub const FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS: usize = 66;
/// Every collected monomial produces at most one reference integral.
pub const FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES: usize =
    FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS;
/// Conservative maximum collected width of one native manifest identity.
/// A seed has at most ten nonzero powers, each derivative contraction has at
/// most one constant plus ten basis terms, and a diagonal identity has one
/// divergence term: `1 + 10*11 = 111`.
pub const FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND: usize = 111;
/// Conservative aggregate convolution reservation for one numerator-bearing
/// manifest row: `111 * 121`.
pub const FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND: usize =
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND
        * FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS;
/// Conservative aggregate output reservation for one numerator-bearing
/// manifest row: `111 * 66`.
pub const FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND: usize =
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND
        * FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES;

/// Per-call limits for exact affine polynomial transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialHaloConfig {
    /// Limits used to authenticate the underlying ten affine images.
    pub affine: FourLoopHaloConfig,
    pub max_numerator_factors: usize,
    pub max_factor_terms: usize,
    pub max_convolution_products: usize,
    pub max_collected_monomials: usize,
    pub max_output_branches: usize,
    /// Maximum collected native terms admitted for one manifest origin.
    pub max_manifest_row_raw_collected_terms: usize,
    /// Aggregate per-term convolution reservation for one manifest origin.
    pub max_manifest_row_convolution_products: usize,
    /// Aggregate per-term output reservation for one manifest origin.
    pub max_manifest_row_output_branches: usize,
}

impl Default for FourLoopPolynomialHaloConfig {
    fn default() -> Self {
        Self {
            affine: FourLoopHaloConfig::default(),
            max_numerator_factors: FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS,
            max_factor_terms: FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            max_convolution_products: FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
            max_collected_monomials: FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS,
            max_output_branches: FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES,
            max_manifest_row_raw_collected_terms:
                FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND,
            max_manifest_row_convolution_products:
                FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND,
            max_manifest_row_output_branches:
                FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND,
        }
    }
}

/// A canonical commuting monomial in the ten frozen denominator coordinates.
///
/// The exponent sum is at most two.  Constants use the all-zero vector;
/// repeated factors are represented by an exponent two at one position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopPolynomialMonomial {
    denominator_powers: [u8; BASIS],
}

impl FourLoopPolynomialMonomial {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-polynomial-monomial-v1";

    const ONE: Self = Self {
        denominator_powers: [0; BASIS],
    };

    pub const fn denominator_powers(&self) -> &[u8; BASIS] {
        &self.denominator_powers
    }

    pub fn degree(self) -> u8 {
        self.denominator_powers.iter().copied().sum()
    }

    pub fn stable_key(self) -> String {
        let powers = self
            .denominator_powers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("{}:[{powers}]", Self::SCHEMA)
    }

    fn denominator(position: usize) -> Self {
        let mut powers = [0_u8; BASIS];
        powers[position] = 1;
        Self {
            denominator_powers: powers,
        }
    }

    fn checked_multiply(self, other: Self) -> Option<Self> {
        let mut powers = [0_u8; BASIS];
        for (target, (&left, &right)) in powers.iter_mut().zip(
            self.denominator_powers
                .iter()
                .zip(other.denominator_powers.iter()),
        ) {
            *target = left.checked_add(right)?;
        }
        let result = Self {
            denominator_powers: powers,
        };
        (result.degree() <= FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS as u8).then_some(result)
    }
}

/// Whether an expanded branch remains in the frozen genuine sector or must be
/// redispatched to a strictly lower physical mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopPolynomialBranchKind {
    SameGenuineMask { mask: u16 },
    StrictlyLowerPhysicalMask { parent_mask: u16, branch_mask: u16 },
}

/// One exact collected polynomial branch after applying its denominator
/// monomial to the transported positive powers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialBranch {
    monomial: FourLoopPolynomialMonomial,
    integral: Integral,
    coefficient: Coefficient,
    kind: FourLoopPolynomialBranchKind,
}

impl FourLoopPolynomialBranch {
    pub const fn monomial(&self) -> FourLoopPolynomialMonomial {
        self.monomial
    }

    pub const fn integral(&self) -> &Integral {
        &self.integral
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub const fn kind(&self) -> FourLoopPolynomialBranchKind {
        self.kind
    }

    #[doc(hidden)]
    pub fn with_integral_for_replay(&self, integral: Integral) -> Self {
        let mut branch = self.clone();
        branch.integral = integral;
        branch
    }
}

/// Exact work actually retained for one polynomial map.  The convolution
/// product field is the uncollected Cartesian-product size, while the last two
/// fields are postcollection counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialHaloStats {
    numerator_factors: usize,
    affine_factor_terms: usize,
    convolution_products: usize,
    collected_monomials: usize,
    output_branches: usize,
}

impl FourLoopPolynomialHaloStats {
    pub const fn numerator_factors(self) -> usize {
        self.numerator_factors
    }

    pub const fn affine_factor_terms(self) -> usize {
        self.affine_factor_terms
    }

    pub const fn convolution_products(self) -> usize {
        self.convolution_products
    }

    pub const fn collected_monomials(self) -> usize {
        self.collected_monomials
    }

    pub const fn output_branches(self) -> usize {
        self.output_branches
    }
}

/// Complete replayable evidence for one raw-term polynomial transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialMapWitness {
    source_family_fingerprint: String,
    reference_family_fingerprint: String,
    corner_type: FourLoopGenuineCornerType,
    manifest_raw_id: Option<FourLoopNextRawRowId>,
    source_seed: Integral,
    source_term: Integral,
    source_numerator_factors: Vec<usize>,
    factor_images: Vec<FourLoopAffineDenominatorImage>,
    collected_monomials: BTreeMap<FourLoopPolynomialMonomial, Coefficient>,
    branches: Vec<FourLoopPolynomialBranch>,
    stats: FourLoopPolynomialHaloStats,
}

impl FourLoopPolynomialMapWitness {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-polynomial-halo-map-v1";

    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_family_fingerprint
    }

    pub fn reference_family_fingerprint(&self) -> &str {
        &self.reference_family_fingerprint
    }

    pub const fn corner_type(&self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub const fn manifest_raw_id(&self) -> Option<FourLoopNextRawRowId> {
        self.manifest_raw_id
    }

    pub const fn source_seed(&self) -> &Integral {
        &self.source_seed
    }

    pub const fn source_term(&self) -> &Integral {
        &self.source_term
    }

    pub fn source_numerator_factors(&self) -> &[usize] {
        &self.source_numerator_factors
    }

    pub fn factor_images(&self) -> &[FourLoopAffineDenominatorImage] {
        &self.factor_images
    }

    pub const fn collected_monomials(&self) -> &BTreeMap<FourLoopPolynomialMonomial, Coefficient> {
        &self.collected_monomials
    }

    pub fn branches(&self) -> &[FourLoopPolynomialBranch] {
        &self.branches
    }

    pub const fn stats(&self) -> FourLoopPolynomialHaloStats {
        self.stats
    }

    #[doc(hidden)]
    pub fn with_source_family_fingerprint_for_replay(&self, fingerprint: String) -> Self {
        let mut map = self.clone();
        map.source_family_fingerprint = fingerprint;
        map
    }

    #[doc(hidden)]
    pub fn with_factor_image_for_replay(
        &self,
        occurrence: usize,
        image: FourLoopAffineDenominatorImage,
    ) -> Self {
        let mut map = self.clone();
        if occurrence < map.factor_images.len() {
            map.factor_images[occurrence] = image;
        }
        map
    }

    #[doc(hidden)]
    pub fn with_monomial_coefficient_for_replay(
        &self,
        monomial: FourLoopPolynomialMonomial,
        coefficient: Coefficient,
    ) -> Self {
        let mut map = self.clone();
        map.collected_monomials.insert(monomial, coefficient);
        map
    }

    #[doc(hidden)]
    pub fn with_branch_for_replay(
        &self,
        position: usize,
        branch: FourLoopPolynomialBranch,
    ) -> Self {
        let mut map = self.clone();
        if position < map.branches.len() {
            map.branches[position] = branch;
        }
        map
    }
}

/// One collected native coefficient paired with the exact transport of its
/// integral key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialRawTermMap {
    raw_coefficient: Coefficient,
    polynomial_map: FourLoopPolynomialMapWitness,
}

impl FourLoopPolynomialRawTermMap {
    pub const fn raw_coefficient(&self) -> &Coefficient {
        &self.raw_coefficient
    }

    pub const fn polynomial_map(&self) -> &FourLoopPolynomialMapWitness {
        &self.polynomial_map
    }

    /// Iterate over this raw term's own affine branches paired with their full
    /// equation coefficients
    /// `raw_coefficient * polynomial_branch.coefficient`.
    ///
    /// Keeping the multiplication on the owning term prevents a caller from
    /// accidentally weighting a branch with an unrelated raw coefficient.
    /// Row consumers must still recollect equal output integrals across all
    /// raw terms; [`FourLoopPolynomialRawRowMap::collected_linear_combination`]
    /// provides that transport-only operation.
    pub fn weighted_branches(
        &self,
    ) -> impl ExactSizeIterator<Item = (&FourLoopPolynomialBranch, Coefficient)> + '_ {
        self.polynomial_map
            .branches
            .iter()
            .map(|branch| (branch, &self.raw_coefficient * branch.coefficient()))
    }

    #[doc(hidden)]
    pub fn with_raw_coefficient_for_replay(&self, coefficient: Coefficient) -> Self {
        let mut term = self.clone();
        term.raw_coefficient = coefficient;
        term
    }
}

/// Exact retained width and conservative-by-degree aggregate work for one
/// generated manifest identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialRawRowStats {
    raw_collected_terms: usize,
    aggregate_convolution_products: usize,
    aggregate_output_branches: usize,
}

impl FourLoopPolynomialRawRowStats {
    pub const fn raw_collected_terms(self) -> usize {
        self.raw_collected_terms
    }

    pub const fn aggregate_convolution_products(self) -> usize {
        self.aggregate_convolution_products
    }

    pub const fn aggregate_output_branches(self) -> usize {
        self.aggregate_output_branches
    }
}

/// Complete mapped contents of one of the 1,968 frozen native origins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopPolynomialRawRowMap {
    raw_id: FourLoopNextRawRowId,
    terms: Vec<FourLoopPolynomialRawTermMap>,
    stats: FourLoopPolynomialRawRowStats,
}

impl FourLoopPolynomialRawRowMap {
    pub const fn raw_id(&self) -> FourLoopNextRawRowId {
        self.raw_id
    }

    pub fn terms(&self) -> &[FourLoopPolynomialRawTermMap] {
        &self.terms
    }

    pub const fn stats(&self) -> FourLoopPolynomialRawRowStats {
        self.stats
    }

    /// Recollect the exact affine images of all raw terms in this native row,
    /// including each term's native sign and multiplicity.
    ///
    /// This is affine transport only.  It does not classify scaleless terms,
    /// recursively dispatch lower masks, close factorized boundaries, mass
    /// normalize, or eliminate the row.  Branch-kind dispatch provenance
    /// remains available in the nested term maps returned by [`Self::terms`].
    pub fn collected_linear_combination(&self) -> LinearCombination {
        let mut collected = LinearCombination::new();
        for term in &self.terms {
            for (branch, coefficient) in term.weighted_branches() {
                collected.add_term(branch.integral.clone(), coefficient);
            }
        }
        collected
    }

    #[doc(hidden)]
    pub fn with_term_for_replay(
        &self,
        position: usize,
        term: FourLoopPolynomialRawTermMap,
    ) -> Self {
        let mut row = self.clone();
        if position < row.terms.len() {
            row.terms[position] = term;
        }
        row
    }

    #[doc(hidden)]
    pub fn with_raw_id_for_replay(&self, raw_id: FourLoopNextRawRowId) -> Self {
        let mut row = self.clone();
        row.raw_id = raw_id;
        row
    }

    #[doc(hidden)]
    pub fn without_term_for_replay(&self, position: usize) -> Self {
        let mut row = self.clone();
        if position < row.terms.len() {
            row.terms.remove(position);
        }
        row
    }
}

/// Degree-two extension of an authenticated [`FourLoopHaloMapper`].
#[derive(Clone, Debug)]
pub struct FourLoopPolynomialHaloMapper {
    config: FourLoopPolynomialHaloConfig,
    affine: FourLoopHaloMapper,
    source_family: VacuumFamily,
    source_family_fingerprint: String,
    reference_family_fingerprint: String,
    manifest_expected_family_fingerprint: Option<String>,
    manifest_family_eligible: bool,
    active_line_map: [Option<usize>; BASIS],
}

impl FourLoopPolynomialHaloMapper {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-polynomial-halo-mapper-v1";

    /// Authenticate a genuine routing witness and retain its complete source
    /// family so manifest raw-row membership can later be regenerated.
    pub fn from_witness(
        classifier: &FourLoopGenuineClassifier,
        witness: &FourLoopGenuineWitness,
        config: FourLoopPolynomialHaloConfig,
    ) -> Result<Self, FourLoopPolynomialHaloError> {
        let affine = FourLoopHaloMapper::from_witness(classifier, witness, config.affine)?;
        let mut active_line_map = [None; BASIS];
        let mut reference_seen = [false; BASIS];
        for line in witness.signed_line_matches() {
            let source = line.source_physical_position();
            let reference = line.reference_physical_position();
            if source >= BASIS
                || reference >= BASIS
                || active_line_map[source].replace(reference).is_some()
                || std::mem::replace(&mut reference_seen[reference], true)
            {
                return Err(FourLoopPolynomialHaloError::ActiveLineMapMismatch);
            }
        }
        if active_line_map
            .iter()
            .enumerate()
            .any(|(position, reference)| {
                (affine.source_sector_mask() & (1_u16 << position) != 0) != reference.is_some()
            })
            || reference_seen.iter().enumerate().any(|(position, &seen)| {
                (affine.corner_type().reference_mask() & (1_u16 << position) != 0) != seen
            })
        {
            return Err(FourLoopPolynomialHaloError::ActiveLineMapMismatch);
        }

        let source_family_fingerprint = classifier.family().fingerprint();
        let manifest_expected_family_fingerprint = match classifier.topology() {
            FourLoopTopology::H | FourLoopTopology::X => {
                Some(equal_mass_four_loop_vacuum(classifier.topology())?.fingerprint())
            }
            FourLoopTopology::Bmw | FourLoopTopology::Fg => None,
        };
        let manifest_family_eligible = manifest_expected_family_fingerprint
            .as_deref()
            .is_some_and(|expected| expected == source_family_fingerprint.as_str());

        Ok(Self {
            config,
            source_family: classifier.family().clone(),
            source_family_fingerprint,
            reference_family_fingerprint: affine.reference_family().fingerprint(),
            manifest_expected_family_fingerprint,
            manifest_family_eligible,
            affine,
            active_line_map,
        })
    }

    pub const fn config(&self) -> FourLoopPolynomialHaloConfig {
        self.config
    }

    pub const fn affine_mapper(&self) -> &FourLoopHaloMapper {
        &self.affine
    }

    pub fn source_family(&self) -> &VacuumFamily {
        &self.source_family
    }

    pub fn source_family_fingerprint(&self) -> &str {
        &self.source_family_fingerprint
    }

    pub fn reference_family_fingerprint(&self) -> &str {
        &self.reference_family_fingerprint
    }

    /// Map a term after proving that it occurs with nonzero coefficient in
    /// the exact native raw identity named by `raw_id`.
    pub fn map_manifest_raw_term(
        &self,
        raw_id: FourLoopNextRawRowId,
        term: &Integral,
    ) -> Result<FourLoopPolynomialMapWitness, FourLoopPolynomialHaloError> {
        let seed = self.authenticate_manifest_origin(raw_id)?;
        self.preflight_manifest_row_structural(raw_id, &seed)?;
        let identity = IbpGenerator::new(&self.source_family).try_generate_raw_identity(
            &seed,
            usize::from(raw_id.differentiated_loop()),
            usize::from(raw_id.contraction_loop()),
        )?;
        self.preflight_manifest_identity(raw_id, &seed, &identity)?;
        if identity.seed != seed || !identity.equation.terms().contains_key(term) {
            return Err(FourLoopPolynomialHaloError::TermAbsentFromManifestRawRow {
                raw_id,
                term: term.clone(),
            });
        }
        self.map_impl(&seed, term, Some(raw_id))
    }

    /// Regenerate and map one complete native identity.  This is the preferred
    /// shell-integration surface because origin membership is authenticated
    /// once rather than separately for every collected term.
    pub fn map_manifest_raw_identity(
        &self,
        raw_id: FourLoopNextRawRowId,
    ) -> Result<FourLoopPolynomialRawRowMap, FourLoopPolynomialHaloError> {
        let seed = self.authenticate_manifest_origin(raw_id)?;
        self.preflight_manifest_row_structural(raw_id, &seed)?;
        let identity = IbpGenerator::new(&self.source_family).try_generate_raw_identity(
            &seed,
            usize::from(raw_id.differentiated_loop()),
            usize::from(raw_id.contraction_loop()),
        )?;
        let stats = self.preflight_manifest_identity(raw_id, &seed, &identity)?;
        let terms = identity
            .equation
            .terms()
            .iter()
            .map(|(integral, coefficient)| {
                Ok(FourLoopPolynomialRawTermMap {
                    raw_coefficient: coefficient.clone(),
                    polynomial_map: self.map_impl(&seed, integral, Some(raw_id))?,
                })
            })
            .collect::<Result<Vec<_>, FourLoopPolynomialHaloError>>()?;
        Ok(FourLoopPolynomialRawRowMap {
            raw_id,
            terms,
            stats,
        })
    }

    /// Map a non-manifest term after authenticating only the bounded seed shape
    /// and algebraic one-shift raw-IBP adjacency.  Unlike the manifest APIs,
    /// this does not regenerate an identity or prove that the collected term
    /// has a nonzero coefficient in any particular native row.  It is useful
    /// for nonidentity BMW/FG routing tests and future versioned manifests.
    pub fn map_authenticated_raw_term(
        &self,
        seed: &Integral,
        term: &Integral,
    ) -> Result<FourLoopPolynomialMapWitness, FourLoopPolynomialHaloError> {
        self.map_impl(seed, term, None)
    }

    /// Replay the underlying affine map, raw origin or adjacency, complete
    /// polynomial convolution, and every typed output branch.
    pub fn replay_polynomial_map(
        &self,
        classifier: &FourLoopGenuineClassifier,
        witness: &FourLoopGenuineWitness,
        map: &FourLoopPolynomialMapWitness,
    ) -> Result<(), FourLoopPolynomialHaloError> {
        if classifier.family().fingerprint() != self.source_family_fingerprint
            || self.source_family.fingerprint() != self.source_family_fingerprint
            || self.affine.reference_family().fingerprint() != self.reference_family_fingerprint
        {
            return Err(FourLoopPolynomialHaloError::MapperFingerprintMismatch);
        }
        self.affine.replay_affine_images(classifier, witness)?;
        let rebuilt = if let Some(raw_id) = map.manifest_raw_id {
            self.map_manifest_raw_term(raw_id, &map.source_term)?
        } else {
            self.map_authenticated_raw_term(&map.source_seed, &map.source_term)?
        };
        if rebuilt == *map {
            Ok(())
        } else {
            Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
        }
    }

    /// Authenticate that a frozen origin is interpreted in its exact built-in
    /// H/X reference family, rather than merely a routing-compatible family or
    /// a different labelled sector of the same corner type.
    fn authenticate_manifest_origin(
        &self,
        raw_id: FourLoopNextRawRowId,
    ) -> Result<Integral, FourLoopPolynomialHaloError> {
        let seed_id = raw_id.seed();
        if seed_id.corner_type() != self.affine.corner_type()
            || seed_id.topology() != self.affine.source_topology()
        {
            return Err(FourLoopPolynomialHaloError::ManifestMapperMismatch { raw_id });
        }
        let expected_mask = seed_id.corner_type().reference_mask();
        let actual_mask = self.affine.source_sector_mask();
        if actual_mask != expected_mask {
            return Err(FourLoopPolynomialHaloError::ManifestSourceSectorMismatch {
                raw_id,
                expected_mask,
                actual_mask,
            });
        }
        if !self.manifest_family_eligible {
            return Err(
                FourLoopPolynomialHaloError::ManifestFamilyFingerprintMismatch {
                    raw_id,
                    expected: self
                        .manifest_expected_family_fingerprint
                        .clone()
                        .unwrap_or_else(|| "built-in H/X manifest family".to_owned()),
                    actual: self.source_family_fingerprint.clone(),
                },
            );
        }
        Ok(seed_id.integral())
    }

    /// Reserve a conservative row envelope using only the frozen seed and raw
    /// derivative labels.  This runs before native generation and coefficient
    /// mapping.  Collection can only decrease the requested raw width.
    fn preflight_manifest_row_structural(
        &self,
        raw_id: FourLoopNextRawRowId,
        seed: &Integral,
    ) -> Result<FourLoopPolynomialRawRowStats, FourLoopPolynomialHaloError> {
        let nonzero_seed_entries = seed.powers().iter().filter(|&&power| power != 0).count();
        let derivative_terms = checked_resource_mul(
            "manifest row raw collected terms",
            nonzero_seed_entries,
            FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            self.config.max_manifest_row_raw_collected_terms,
        )?;
        let raw_collected_terms = checked_resource_add(
            "manifest row raw collected terms",
            derivative_terms,
            usize::from(raw_id.differentiated_loop() == raw_id.contraction_loop()),
            self.config.max_manifest_row_raw_collected_terms,
        )?;
        check_resource(
            "manifest row raw collected terms",
            raw_collected_terms,
            self.config.max_manifest_row_raw_collected_terms,
        )?;

        let seed_numerator_degree =
            seed.checked_numerator_degree()
                .ok_or(FourLoopPolynomialHaloError::ResourceLimit {
                    resource: "manifest row numerator degree",
                    requested: u128::MAX,
                    limit: FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS as u128,
                })?;
        let maximum_numerator_factors = seed_numerator_degree.checked_add(1).ok_or(
            FourLoopPolynomialHaloError::ResourceLimit {
                resource: "manifest row numerator degree",
                requested: u128::MAX,
                limit: FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS as u128,
            },
        )? as usize;
        let (per_term_convolution, per_term_output) =
            self.preflight_degree_reservation(maximum_numerator_factors)?;
        let aggregate_convolution_products = checked_resource_mul(
            "manifest row aggregate convolution products",
            raw_collected_terms,
            per_term_convolution,
            self.config.max_manifest_row_convolution_products,
        )?;
        let aggregate_output_branches = checked_resource_mul(
            "manifest row aggregate output branches",
            raw_collected_terms,
            per_term_output,
            self.config.max_manifest_row_output_branches,
        )?;
        check_resource(
            "manifest row aggregate convolution products",
            aggregate_convolution_products,
            self.config.max_manifest_row_convolution_products,
        )?;
        check_resource(
            "manifest row aggregate output branches",
            aggregate_output_branches,
            self.config.max_manifest_row_output_branches,
        )?;
        Ok(FourLoopPolynomialRawRowStats {
            raw_collected_terms,
            aggregate_convolution_products,
            aggregate_output_branches,
        })
    }

    /// Check exact collected width and exact degree-reserved aggregate work
    /// after generation but before constructing any polynomial maps.
    fn preflight_manifest_identity(
        &self,
        raw_id: FourLoopNextRawRowId,
        seed: &Integral,
        identity: &IbpIdentity,
    ) -> Result<FourLoopPolynomialRawRowStats, FourLoopPolynomialHaloError> {
        if identity.seed != *seed
            || identity.differentiated_loop != usize::from(raw_id.differentiated_loop())
            || identity.contraction_loop != usize::from(raw_id.contraction_loop())
        {
            return Err(FourLoopPolynomialHaloError::ManifestMapperMismatch { raw_id });
        }
        let raw_collected_terms = identity.equation.len();
        check_resource(
            "manifest row raw collected terms",
            raw_collected_terms,
            self.config.max_manifest_row_raw_collected_terms,
        )?;

        let seed_powers = powers(seed, "seed")?;
        let mut aggregate_convolution_products = 0_usize;
        let mut aggregate_output_branches = 0_usize;
        for term in identity.equation.terms().keys() {
            let term_powers = powers(term, "term")?;
            if !is_one_raw_shift(&seed_powers, &term_powers) {
                return Err(FourLoopPolynomialHaloError::NonAdjacentRawTerm {
                    seed: seed.clone(),
                    term: term.clone(),
                });
            }
            self.validate_term(seed, term, &term_powers)?;
            let numerator_factors = term_powers
                .iter()
                .filter(|&&power| power < 0)
                .try_fold(0_usize, |total, &power| {
                    total.checked_add(usize::try_from(power.unsigned_abs()).ok()?)
                })
                .ok_or(FourLoopPolynomialHaloError::ResourceLimit {
                    resource: "manifest row numerator degree",
                    requested: u128::MAX,
                    limit: FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS as u128,
                })?;
            let (convolution, output) = self.preflight_degree_reservation(numerator_factors)?;
            aggregate_convolution_products = checked_resource_add(
                "manifest row aggregate convolution products",
                aggregate_convolution_products,
                convolution,
                self.config.max_manifest_row_convolution_products,
            )?;
            aggregate_output_branches = checked_resource_add(
                "manifest row aggregate output branches",
                aggregate_output_branches,
                output,
                self.config.max_manifest_row_output_branches,
            )?;
        }
        check_resource(
            "manifest row aggregate convolution products",
            aggregate_convolution_products,
            self.config.max_manifest_row_convolution_products,
        )?;
        check_resource(
            "manifest row aggregate output branches",
            aggregate_output_branches,
            self.config.max_manifest_row_output_branches,
        )?;
        Ok(FourLoopPolynomialRawRowStats {
            raw_collected_terms,
            aggregate_convolution_products,
            aggregate_output_branches,
        })
    }

    fn preflight_degree_reservation(
        &self,
        numerator_factors: usize,
    ) -> Result<(usize, usize), FourLoopPolynomialHaloError> {
        check_resource(
            "numerator factors",
            numerator_factors,
            self.config.max_numerator_factors,
        )?;
        let (convolution, output) = match numerator_factors {
            0 => (0, 1),
            1 => (
                FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
                FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            ),
            2 => (
                FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
                FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS,
            ),
            _ => {
                return Err(FourLoopPolynomialHaloError::ResourceLimit {
                    resource: "numerator factors",
                    requested: numerator_factors as u128,
                    limit: FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS as u128,
                });
            }
        };
        if numerator_factors != 0 {
            check_resource(
                "terms per affine factor",
                FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
                self.config.max_factor_terms,
            )?;
        }
        check_resource(
            "affine convolution products",
            convolution,
            self.config.max_convolution_products,
        )?;
        check_resource(
            "collected polynomial monomials",
            output,
            self.config.max_collected_monomials,
        )?;
        check_resource(
            "polynomial output branches",
            output,
            self.config.max_output_branches,
        )?;
        Ok((convolution, output))
    }

    /// Independently regenerate a complete native origin and all of its exact
    /// polynomial maps after replaying the shared affine routing witness once.
    pub fn replay_manifest_raw_identity(
        &self,
        classifier: &FourLoopGenuineClassifier,
        witness: &FourLoopGenuineWitness,
        row: &FourLoopPolynomialRawRowMap,
    ) -> Result<(), FourLoopPolynomialHaloError> {
        if classifier.family().fingerprint() != self.source_family_fingerprint
            || self.source_family.fingerprint() != self.source_family_fingerprint
            || self.affine.reference_family().fingerprint() != self.reference_family_fingerprint
        {
            return Err(FourLoopPolynomialHaloError::MapperFingerprintMismatch);
        }
        self.affine.replay_affine_images(classifier, witness)?;
        let rebuilt = self.map_manifest_raw_identity(row.raw_id)?;
        if rebuilt == *row {
            Ok(())
        } else {
            Err(FourLoopPolynomialHaloError::PolynomialReplayMismatch)
        }
    }

    fn map_impl(
        &self,
        seed: &Integral,
        term: &Integral,
        manifest_raw_id: Option<FourLoopNextRawRowId>,
    ) -> Result<FourLoopPolynomialMapWitness, FourLoopPolynomialHaloError> {
        let seed_powers = powers(seed, "seed")?;
        let term_powers = powers(term, "term")?;
        self.validate_seed(seed, &seed_powers)?;
        if !is_one_raw_shift(&seed_powers, &term_powers) {
            return Err(FourLoopPolynomialHaloError::NonAdjacentRawTerm {
                seed: seed.clone(),
                term: term.clone(),
            });
        }
        self.validate_term(seed, term, &term_powers)?;

        let source_numerator_factors = numerator_factors(&term_powers);
        let factor_count = source_numerator_factors.len();
        check_resource(
            "numerator factors",
            factor_count,
            self.config.max_numerator_factors,
        )?;
        if factor_count > FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS {
            return Err(FourLoopPolynomialHaloError::OutsideNextShellRawTerm {
                seed: seed.clone(),
                term: term.clone(),
            });
        }
        if factor_count != 0 {
            check_resource(
                "terms per affine factor",
                FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
                self.config.max_factor_terms,
            )?;
        }
        let convolution_reservation = match factor_count {
            0 => 0,
            1 => FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            2 => FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
            _ => unreachable!("the factor count was bounded above"),
        };
        let monomial_reservation = match factor_count {
            0 => 1,
            1 => FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
            2 => FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS,
            _ => unreachable!("the factor count was bounded above"),
        };
        check_resource(
            "affine convolution products",
            convolution_reservation,
            self.config.max_convolution_products,
        )?;
        check_resource(
            "collected polynomial monomials",
            monomial_reservation,
            self.config.max_collected_monomials,
        )?;
        check_resource(
            "polynomial output branches",
            monomial_reservation,
            self.config.max_output_branches,
        )?;

        let factor_images = source_numerator_factors
            .iter()
            .map(|&position| self.affine.image(position).cloned())
            .collect::<Result<Vec<_>, FourLoopHaloError>>()?;
        let factors = factor_images
            .iter()
            .map(|image| self.factor_polynomial(image))
            .collect::<Vec<_>>();
        let affine_factor_terms = factors.iter().map(BTreeMap::len).sum();
        let convolution_products = match factors.as_slice() {
            [] => 0,
            [factor] => factor.len(),
            [left, right] => left.len().checked_mul(right.len()).ok_or(
                FourLoopPolynomialHaloError::ResourceLimit {
                    resource: "affine convolution products",
                    requested: u128::MAX,
                    limit: self.config.max_convolution_products as u128,
                },
            )?,
            _ => unreachable!("the factor count was bounded above"),
        };
        let collected_monomials = match factors.as_slice() {
            [] => BTreeMap::from([(
                FourLoopPolynomialMonomial::ONE,
                self.affine.reference_family().coefficients().one(),
            )]),
            [factor] => factor.clone(),
            [left, right] => self.convolve(left, right)?,
            _ => unreachable!("the factor count was bounded above"),
        };
        check_resource(
            "collected polynomial monomials",
            collected_monomials.len(),
            self.config.max_collected_monomials,
        )?;

        let mut reference_powers = [0_i32; BASIS];
        for (source, &power) in term_powers.iter().enumerate() {
            if self.affine.source_sector_mask() & (1_u16 << source) == 0 {
                continue;
            }
            let reference = self.active_line_map[source]
                .ok_or(FourLoopPolynomialHaloError::ActiveLineMapMismatch)?;
            if reference_powers[reference] != 0 {
                return Err(FourLoopPolynomialHaloError::ActiveLineMapMismatch);
            }
            reference_powers[reference] = power;
        }

        let parent_mask = self.affine.corner_type().reference_mask();
        let mut branches = Vec::with_capacity(collected_monomials.len());
        for (&monomial, coefficient) in &collected_monomials {
            let mut branch_powers = reference_powers;
            for (power, &lowering) in branch_powers
                .iter_mut()
                .zip(monomial.denominator_powers.iter())
            {
                *power = power
                    .checked_sub(i32::from(lowering))
                    .ok_or(FourLoopPolynomialHaloError::ExponentOverflow { term: term.clone() })?;
            }
            let branch_mask = physical_mask(self.affine.reference_family(), &branch_powers)?;
            if branch_mask & !parent_mask != 0 {
                return Err(FourLoopPolynomialHaloError::NonDecreasingPhysicalMask {
                    parent_mask,
                    branch_mask,
                });
            }
            let kind = if branch_mask == parent_mask {
                FourLoopPolynomialBranchKind::SameGenuineMask { mask: parent_mask }
            } else {
                FourLoopPolynomialBranchKind::StrictlyLowerPhysicalMask {
                    parent_mask,
                    branch_mask,
                }
            };
            branches.push(FourLoopPolynomialBranch {
                monomial,
                integral: Integral::from(branch_powers),
                coefficient: coefficient.clone(),
                kind,
            });
        }
        check_resource(
            "polynomial output branches",
            branches.len(),
            self.config.max_output_branches,
        )?;

        let stats = FourLoopPolynomialHaloStats {
            numerator_factors: factor_count,
            affine_factor_terms,
            convolution_products,
            collected_monomials: collected_monomials.len(),
            output_branches: branches.len(),
        };
        Ok(FourLoopPolynomialMapWitness {
            source_family_fingerprint: self.source_family_fingerprint.clone(),
            reference_family_fingerprint: self.reference_family_fingerprint.clone(),
            corner_type: self.affine.corner_type(),
            manifest_raw_id,
            source_seed: seed.clone(),
            source_term: term.clone(),
            source_numerator_factors,
            factor_images,
            collected_monomials,
            branches,
            stats,
        })
    }

    fn validate_seed(
        &self,
        seed: &Integral,
        powers: &[i32; BASIS],
    ) -> Result<(), FourLoopPolynomialHaloError> {
        let source_mask = self.affine.source_sector_mask();
        let mut dots = 0_u32;
        let mut numerators = 0_u32;
        for (position, &power) in powers.iter().enumerate() {
            if source_mask & (1_u16 << position) != 0 {
                if !(1..=2).contains(&power) {
                    return Err(FourLoopPolynomialHaloError::OutsideNextShellSeed {
                        seed: seed.clone(),
                    });
                }
                dots += u32::try_from(power - 1).expect("a positive bounded power fits u32");
            } else {
                if !(-1..=0).contains(&power) {
                    return Err(FourLoopPolynomialHaloError::OutsideNextShellSeed {
                        seed: seed.clone(),
                    });
                }
                numerators += power.unsigned_abs();
            }
        }
        if dots > 1 || numerators > 1 {
            return Err(FourLoopPolynomialHaloError::OutsideNextShellSeed { seed: seed.clone() });
        }
        Ok(())
    }

    fn validate_term(
        &self,
        seed: &Integral,
        term: &Integral,
        powers: &[i32; BASIS],
    ) -> Result<(), FourLoopPolynomialHaloError> {
        let source_mask = self.affine.source_sector_mask();
        let mut dots = 0_u32;
        let mut numerators = 0_u32;
        for (position, &power) in powers.iter().enumerate() {
            if source_mask & (1_u16 << position) != 0 {
                if !(0..=3).contains(&power) {
                    return Err(FourLoopPolynomialHaloError::OutsideNextShellRawTerm {
                        seed: seed.clone(),
                        term: term.clone(),
                    });
                }
                dots = dots.saturating_add(power.saturating_sub(1).max(0) as u32);
            } else {
                if !(-2..=0).contains(&power) {
                    return Err(FourLoopPolynomialHaloError::OutsideNextShellRawTerm {
                        seed: seed.clone(),
                        term: term.clone(),
                    });
                }
                numerators = numerators.saturating_add(power.unsigned_abs());
            }
        }
        if dots > 2 || numerators > 2 {
            return Err(FourLoopPolynomialHaloError::OutsideNextShellRawTerm {
                seed: seed.clone(),
                term: term.clone(),
            });
        }
        Ok(())
    }

    fn factor_polynomial(
        &self,
        image: &FourLoopAffineDenominatorImage,
    ) -> BTreeMap<FourLoopPolynomialMonomial, Coefficient> {
        let mut factor = BTreeMap::new();
        add_polynomial_term(
            &mut factor,
            FourLoopPolynomialMonomial::ONE,
            image.constant().clone(),
        );
        for (position, coefficient) in image.denominator_coefficients().iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            add_polynomial_term(
                &mut factor,
                FourLoopPolynomialMonomial::denominator(position),
                self.affine
                    .reference_family()
                    .coefficients()
                    .rational(coefficient),
            );
        }
        factor
    }

    fn convolve(
        &self,
        left: &BTreeMap<FourLoopPolynomialMonomial, Coefficient>,
        right: &BTreeMap<FourLoopPolynomialMonomial, Coefficient>,
    ) -> Result<BTreeMap<FourLoopPolynomialMonomial, Coefficient>, FourLoopPolynomialHaloError>
    {
        let mut output = BTreeMap::new();
        for (&left_monomial, left_coefficient) in left {
            for (&right_monomial, right_coefficient) in right {
                let monomial = left_monomial
                    .checked_multiply(right_monomial)
                    .ok_or(FourLoopPolynomialHaloError::PolynomialDegreeOverflow)?;
                add_polynomial_term(&mut output, monomial, left_coefficient * right_coefficient);
            }
        }
        Ok(output)
    }
}

fn powers(
    integral: &Integral,
    role: &'static str,
) -> Result<[i32; BASIS], FourLoopPolynomialHaloError> {
    integral
        .powers()
        .try_into()
        .map_err(|_| FourLoopPolynomialHaloError::WrongIntegralArity {
            role,
            expected: BASIS,
            actual: integral.powers().len(),
        })
}

fn is_one_raw_shift(seed: &[i32; BASIS], term: &[i32; BASIS]) -> bool {
    if seed == term {
        return true;
    }
    for raised in 0..BASIS {
        if seed[raised] == 0 {
            continue;
        }
        let mut candidate = *seed;
        let Some(value) = candidate[raised].checked_add(1) else {
            continue;
        };
        candidate[raised] = value;
        if &candidate == term {
            return true;
        }
        for lowered in 0..BASIS {
            let mut candidate = candidate;
            let Some(value) = candidate[lowered].checked_sub(1) else {
                continue;
            };
            candidate[lowered] = value;
            if &candidate == term {
                return true;
            }
        }
    }
    false
}

fn numerator_factors(powers: &[i32; BASIS]) -> Vec<usize> {
    powers
        .iter()
        .enumerate()
        .flat_map(|(position, &power)| {
            let multiplicity = if power < 0 {
                usize::try_from(power.unsigned_abs())
                    .expect("validated numerator degree fits usize")
            } else {
                0
            };
            std::iter::repeat_n(position, multiplicity)
        })
        .collect()
}

fn physical_mask(
    family: &VacuumFamily,
    powers: &[i32; BASIS],
) -> Result<u16, FourLoopPolynomialHaloError> {
    let mut mask = 0_u16;
    for (position, (&power, denominator)) in powers.iter().zip(family.denominators()).enumerate() {
        if power <= 0 {
            continue;
        }
        if !denominator.is_propagator() {
            return Err(FourLoopPolynomialHaloError::PositiveAuxiliaryPower { position });
        }
        mask |= 1_u16 << position;
    }
    Ok(mask)
}

fn add_polynomial_term(
    polynomial: &mut BTreeMap<FourLoopPolynomialMonomial, Coefficient>,
    monomial: FourLoopPolynomialMonomial,
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    match polynomial.entry(monomial) {
        Entry::Vacant(entry) => {
            entry.insert(coefficient);
        }
        Entry::Occupied(mut entry) => {
            let sum = entry.get() + &coefficient;
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

fn check_resource(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FourLoopPolynomialHaloError> {
    if requested > limit {
        return Err(FourLoopPolynomialHaloError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        });
    }
    Ok(())
}

fn checked_resource_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, FourLoopPolynomialHaloError> {
    left.checked_add(right)
        .ok_or(FourLoopPolynomialHaloError::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        })
}

fn checked_resource_mul(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, FourLoopPolynomialHaloError> {
    left.checked_mul(right)
        .ok_or(FourLoopPolynomialHaloError::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        })
}

#[derive(Debug)]
pub enum FourLoopPolynomialHaloError {
    Halo(FourLoopHaloError),
    Ibp(IbpGenerationError),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    WrongIntegralArity {
        role: &'static str,
        expected: usize,
        actual: usize,
    },
    OutsideNextShellSeed {
        seed: Integral,
    },
    NonAdjacentRawTerm {
        seed: Integral,
        term: Integral,
    },
    OutsideNextShellRawTerm {
        seed: Integral,
        term: Integral,
    },
    ManifestMapperMismatch {
        raw_id: FourLoopNextRawRowId,
    },
    ManifestSourceSectorMismatch {
        raw_id: FourLoopNextRawRowId,
        expected_mask: u16,
        actual_mask: u16,
    },
    ManifestFamilyFingerprintMismatch {
        raw_id: FourLoopNextRawRowId,
        expected: String,
        actual: String,
    },
    TermAbsentFromManifestRawRow {
        raw_id: FourLoopNextRawRowId,
        term: Integral,
    },
    ActiveLineMapMismatch,
    MapperFingerprintMismatch,
    PolynomialDegreeOverflow,
    ExponentOverflow {
        term: Integral,
    },
    PositiveAuxiliaryPower {
        position: usize,
    },
    NonDecreasingPhysicalMask {
        parent_mask: u16,
        branch_mask: u16,
    },
    PolynomialReplayMismatch,
}

impl From<FourLoopHaloError> for FourLoopPolynomialHaloError {
    fn from(error: FourLoopHaloError) -> Self {
        Self::Halo(error)
    }
}

impl From<IbpGenerationError> for FourLoopPolynomialHaloError {
    fn from(error: IbpGenerationError) -> Self {
        Self::Ibp(error)
    }
}

impl From<rustred::FamilyError> for FourLoopPolynomialHaloError {
    fn from(error: rustred::FamilyError) -> Self {
        Self::Halo(FourLoopHaloError::Family(error))
    }
}

impl fmt::Display for FourLoopPolynomialHaloError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Halo(error) => write!(formatter, "four-loop affine polynomial map: {error}"),
            Self::Ibp(error) => write!(formatter, "four-loop affine polynomial IBP: {error}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop affine polynomial {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::WrongIntegralArity {
                role,
                expected,
                actual,
            } => write!(
                formatter,
                "four-loop affine polynomial {role} has {actual} powers, expected {expected}"
            ),
            Self::OutsideNextShellSeed { seed } => {
                write!(
                    formatter,
                    "{seed} is outside the selected (D,N) <= (1,1) seed box"
                )
            }
            Self::NonAdjacentRawTerm { seed, term } => write!(
                formatter,
                "{term} is not a native one-shift raw-IBP neighbor of seed {seed}"
            ),
            Self::OutsideNextShellRawTerm { seed, term } => write!(
                formatter,
                "raw term {term} from seed {seed} is outside the (D,N) <= (2,2) dependency halo"
            ),
            Self::ManifestMapperMismatch { raw_id } => write!(
                formatter,
                "manifest origin {} does not belong to this genuine mapper",
                raw_id.stable_key()
            ),
            Self::ManifestSourceSectorMismatch {
                raw_id,
                expected_mask,
                actual_mask,
            } => write!(
                formatter,
                "manifest origin {} requires reference mask {expected_mask:#05x}, but the mapper source mask is {actual_mask:#05x}",
                raw_id.stable_key()
            ),
            Self::ManifestFamilyFingerprintMismatch {
                raw_id,
                expected,
                actual,
            } => write!(
                formatter,
                "manifest origin {} requires exact family fingerprint {expected:?}, not {actual:?}",
                raw_id.stable_key()
            ),
            Self::TermAbsentFromManifestRawRow { raw_id, term } => write!(
                formatter,
                "term {term} is absent from exact manifest origin {}",
                raw_id.stable_key()
            ),
            Self::ActiveLineMapMismatch => {
                formatter.write_str("genuine witness does not define a bijective active-line map")
            }
            Self::MapperFingerprintMismatch => formatter.write_str(
                "source/reference family fingerprint does not replay for this polynomial mapper",
            ),
            Self::PolynomialDegreeOverflow => formatter
                .write_str("affine convolution exceeded the certified polynomial degree two"),
            Self::ExponentOverflow { term } => {
                write!(
                    formatter,
                    "mapped denominator exponent overflowed for {term}"
                )
            }
            Self::PositiveAuxiliaryPower { position } => write!(
                formatter,
                "mapped branch created a positive auxiliary power at position {position}"
            ),
            Self::NonDecreasingPhysicalMask {
                parent_mask,
                branch_mask,
            } => write!(
                formatter,
                "mapped physical mask {branch_mask:#05x} is not a subset of parent {parent_mask:#05x}"
            ),
            Self::PolynomialReplayMismatch => formatter
                .write_str("rebuilt exact affine polynomial map differs from the stored witness"),
        }
    }
}

impl Error for FourLoopPolynomialHaloError {}
