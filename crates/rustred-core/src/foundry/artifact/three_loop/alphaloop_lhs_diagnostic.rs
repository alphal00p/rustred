//! Offline-only AlphaLoop LHS target itinerary for K=6 diagnostics.
//!
//! This fixture contains only the first-occurrence left-hand-side pattern
//! locations and domains from the read-only AlphaLoop oracle. It contains no
//! right-hand-side support, coefficient, pivot, source choice, or rule. Every
//! proposed owner must still be derived from RustRed's regenerated nine
//! ordinary IBPs and pass the normal exact replay/admission boundary.

use std::collections::BTreeMap;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralFamilyLimits, IntegralKey};
use crate::foundry::completion::stratum::{ImmutableOwnerSnapshot, StratumRegistryLimits};
use crate::foundry::completion::{
    BoxCover, CompletionGeometryLimits, LatticeBox, LatticePoint, SectorChart,
};
use crate::sector::symmetry::{
    CoefficientMatrix, DenominatorAction, Limits as SymmetryLimits, MomentumMap, RoutingWitness,
    verify,
};
use crate::sector::{Mask, OrderingPolicy};

use super::{
    canonical_family, canonical_s4, canonical_s4_with_ordering, derive_k6_terminal_authority,
};

/// One deduplicated first-occurrence AlphaLoop LHS pattern, already written
/// in the direct RustRed slot order certified by [`certify_alpha_to_rust_map`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AlphaLoopLhsAnchor {
    pub(crate) source_line: u16,
    pub(crate) sector_bits: &'static str,
    pub(crate) chart_digits: &'static str,
    pub(crate) symbolic_bits: &'static str,
}

/// One authenticated/canonicalized diagnostic target.
#[derive(Debug)]
pub(crate) struct MaterializedAlphaLoopLhsAnchor {
    pub(crate) source: AlphaLoopLhsAnchor,
    pub(crate) raw_integral: IntegralKey,
    pub(crate) canonical_integral: IntegralKey,
    pub(crate) canonical_sector: Mask,
    pub(crate) canonical_point: LatticePoint,
    pub(crate) canonical_symbolic_axes: Box<[usize]>,
    pub(crate) canonical_route: RoutingWitness,
}

/// Exact denominator-form and graph-symmetry evidence for the slot map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AlphaToRustMapCertificate {
    /// Executable matcher route `source_for_target[R] = A`:
    /// `R=[A1,A3,A2,A4,A6,A5]`.
    pub(crate) form_source_for_rust_target: [usize; 6],
    /// An equally valid direct identity-momentum edge labeling:
    /// `R=[A1,A2,A3,A6,A4,A5]`.
    pub(crate) direct_source_for_rust_target: [usize; 6],
    /// Authenticated Rust self-action carrying the FORM route to the direct
    /// identity-momentum route.
    pub(crate) direct_relative_to_form_s4: [usize; 6],
}

/// The 55 unique LHS targets in bottom-up, first-source-line chronology.
///
/// The entries were extracted offline from `integrateduv.frm` lines
/// 301..=1084. The reference file is never parsed at build or run time. The
/// 14 factorization entries at 1086..=1099 are predecessor/terminal authority,
/// not ordinary-replay targets.
pub(crate) const ALPHALOOP_LHS_ANCHORS: [AlphaLoopLhsAnchor; 55] = [
    anchor(301, "010011", "000110", "111111"),
    anchor(313, "010011", "100010", "111111"),
    anchor(325, "010011", "000101", "111111"),
    anchor(337, "010011", "001001", "111111"),
    anchor(349, "010011", "011000", "111111"),
    anchor(361, "010011", "110000", "111111"),
    anchor(373, "010011", "000010", "111111"),
    anchor(382, "010011", "000001", "111111"),
    anchor(391, "010011", "010000", "111111"),
    anchor(400, "010011", "000100", "101100"),
    anchor(416, "010011", "001000", "101000"),
    anchor(432, "010011", "100000", "100000"),
    anchor(441, "010111", "100010", "111111"),
    anchor(459, "010111", "001001", "111111"),
    anchor(474, "010111", "011000", "111111"),
    anchor(486, "010111", "110000", "111111"),
    anchor(498, "010111", "000010", "111111"),
    anchor(513, "010111", "000001", "111111"),
    anchor(525, "010111", "000100", "111111"),
    anchor(537, "010111", "010000", "111111"),
    anchor(564, "010111", "001000", "101000"),
    anchor(582, "010111", "100000", "100000"),
    anchor(591, "011110", "000011", "111111"),
    anchor(601, "011110", "100010", "111111"),
    anchor(611, "011110", "000101", "111111"),
    anchor(622, "011110", "100100", "111111"),
    anchor(631, "011110", "010001", "111111"),
    anchor(640, "011110", "110000", "111111"),
    anchor(651, "011110", "001001", "111111"),
    anchor(660, "011110", "101000", "111111"),
    anchor(669, "011110", "000010", "111111"),
    anchor(677, "011110", "000200", "011100"),
    anchor(688, "011110", "010100", "011000"),
    anchor(697, "011110", "020000", "011000"),
    anchor(702, "011110", "000001", "100001"),
    anchor(712, "011110", "000100", "001000"),
    anchor(721, "011110", "010000", "001000"),
    anchor(730, "011110", "001000", "001000"),
    anchor(743, "011110", "100000", "100000"),
    anchor(751, "011111", "100010", "111111"),
    anchor(770, "011111", "100100", "111111"),
    anchor(789, "011111", "110000", "111111"),
    anchor(808, "011111", "101000", "111111"),
    anchor(827, "011111", "000010", "111111"),
    anchor(839, "011111", "000001", "111111"),
    anchor(851, "011111", "000100", "111111"),
    anchor(863, "011111", "010000", "111111"),
    anchor(875, "011111", "001000", "111111"),
    anchor(887, "011111", "100000", "100000"),
    anchor(914, "111111", "000010", "111111"),
    anchor(933, "111111", "000001", "111111"),
    anchor(952, "111111", "000100", "111111"),
    anchor(971, "111111", "010000", "111111"),
    anchor(988, "111111", "001000", "111111"),
    anchor(1005, "111111", "100000", "111111"),
];

const fn anchor(
    source_line: u16,
    sector_bits: &'static str,
    chart_digits: &'static str,
    symbolic_bits: &'static str,
) -> AlphaLoopLhsAnchor {
    AlphaLoopLhsAnchor {
        source_line,
        sector_bits,
        chart_digits,
        symbolic_bits,
    }
}

/// Prove the executable FORM-matcher slot map from exact denominator forms and
/// prove that the alternative direct graph labeling differs only by an
/// authenticated element of the K4 edge action.
pub(crate) fn certify_alpha_to_rust_map() -> AlphaToRustMapCertificate {
    let rust = canonical_family().unwrap();
    let alpha = alpha_mercedes_family();
    let context = alpha.coefficient_context();
    let one = context.one();
    let zero = context.zero();
    let identity = MomentumMap::new(
        CoefficientMatrix::try_new(
            3,
            3,
            [
                one.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
                one.clone(),
                zero.clone(),
                zero.clone(),
                zero.clone(),
                one.clone(),
            ],
        )
        .unwrap(),
        CoefficientMatrix::try_new(3, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    );
    let verified = verify(&alpha, &rust, identity, SymmetryLimits::default()).unwrap();
    let mut form_source_for_rust_target = [usize::MAX; 6];
    for (alpha_source, action) in verified.row_actions().iter().enumerate() {
        let DenominatorAction::Monomial { target, scale } = action else {
            panic!("an AlphaLoop Mercedes denominator did not map monomially")
        };
        assert_eq!(scale, &one, "the direct slot map must have unit scales");
        assert_eq!(form_source_for_rust_target[*target], usize::MAX);
        form_source_for_rust_target[*target] = alpha_source;
    }
    assert_eq!(form_source_for_rust_target, [0, 2, 1, 3, 5, 4]);

    // An exact FORM5 sentinel and Vakint prose both give the map above. A
    // direct identity-momentum graph labeling used during the forensic audit
    // is also valid, but differs by this authenticated K4 edge action.
    let direct_source_for_rust_target = [0, 1, 2, 5, 3, 4];
    let direct_relative_to_form_s4 = [0, 2, 1, 4, 3, 5];
    let canonicalizer = canonical_s4(&rust).unwrap();
    assert!(
        canonicalizer
            .group_elements()
            .any(|element| element == direct_relative_to_form_s4),
        "the FORM and direct graph routes must be related by authenticated K4 symmetry"
    );

    AlphaToRustMapCertificate {
        form_source_for_rust_target,
        direct_source_for_rust_target,
        direct_relative_to_form_s4,
    }
}

/// Canonicalize all LHS domains through the authenticated K4 action while
/// preserving their bottom-up oracle chronology.
pub(crate) fn materialize_alpha_loop_lhs_anchors() -> Vec<MaterializedAlphaLoopLhsAnchor> {
    materialize_alpha_loop_lhs_anchors_with_ordering(OrderingPolicy::default())
}

/// Materialize the frozen LHS domains under the exact ordering that will own
/// and reduce the resulting rules. The authenticated graph action is
/// unchanged, but a non-`S4`-invariant coordinate priority can select a
/// different representative from each orbit.
pub(crate) fn materialize_alpha_loop_lhs_anchors_with_ordering(
    ordering: OrderingPolicy,
) -> Vec<MaterializedAlphaLoopLhsAnchor> {
    let _certificate = certify_alpha_to_rust_map();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4_with_ordering(&family, ordering).unwrap();
    ALPHALOOP_LHS_ANCHORS
        .iter()
        .copied()
        .map(|source| {
            let raw_sector = parse_mask(source.sector_bits);
            let raw_point = parse_point(source.chart_digits);
            let raw_symbolic = parse_bits(source.symbolic_bits);
            let raw_integral = SectorChart::new(raw_sector)
                .to_integral(&raw_point)
                .unwrap();
            let canonicalization = canonicalizer.canonicalize(&raw_integral).unwrap();
            assert!(canonicalization.verify());
            assert!(canonicalizer.authenticates_route(canonicalization.route()));
            let canonical_integral = canonicalization.canonical().clone();
            let canonical_sector = Mask::try_from_indices(canonical_integral.powers()).unwrap();
            let canonical_point = SectorChart::new(canonical_sector.clone())
                .to_lattice(&canonical_integral)
                .unwrap();
            let canonical_symbolic_axes = canonicalization
                .route()
                .source_for_target()
                .iter()
                .enumerate()
                .filter_map(|(target, &raw_source)| raw_symbolic[raw_source].then_some(target))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            MaterializedAlphaLoopLhsAnchor {
                source,
                raw_integral,
                canonical_integral,
                canonical_sector,
                canonical_point,
                canonical_symbolic_axes,
                canonical_route: canonicalization.route().clone(),
            }
        })
        .collect()
}

fn parse_mask(bits: &str) -> Mask {
    Mask::try_new(parse_bits(bits)).unwrap()
}

fn parse_point(digits: &str) -> LatticePoint {
    LatticePoint::try_new(digits.bytes().map(|digit| u64::from(digit - b'0'))).unwrap()
}

fn parse_bits(bits: &str) -> Vec<bool> {
    assert_eq!(bits.len(), 6);
    bits.bytes()
        .map(|bit| match bit {
            b'0' => false,
            b'1' => true,
            _ => panic!("a frozen AlphaLoop bit string is not binary"),
        })
        .collect()
}

fn alpha_mercedes_family() -> IntegralFamily {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let dimension = context.parameter("d").unwrap();
    let zero = context.zero();
    let minus_one = context.integer(-1);
    let denominator = |coefficients: [i64; 6]| {
        AffineDenominator::new(
            minus_one.clone(),
            coefficients
                .into_iter()
                .map(|entry| context.integer(entry))
                .collect(),
        )
    };
    let denominators = vec![
        denominator([1, 0, 0, 0, 0, 0]),  // A1 = k1^2 - 1
        denominator([0, 0, 0, 0, 0, 1]),  // A2 = k3^2 - 1
        denominator([0, 0, 0, 1, 0, 0]),  // A3 = k2^2 - 1
        denominator([1, 0, -2, 0, 0, 1]), // A4 = (k3-k1)^2 - 1
        denominator([0, 0, 0, 1, -2, 1]), // A5 = (k2-k3)^2 - 1
        denominator([1, -2, 0, 1, 0, 0]), // A6 = (k1-k2)^2 - 1
    ];
    IntegralFamily::new_with_limits(
        "offline-alphaloop-three-loop-unit-mass-mercedes",
        vec!["k1".to_owned(), "k2".to_owned(), "k3".to_owned()],
        Vec::new(),
        context,
        dimension,
        denominators,
        Vec::new(),
        vec![
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero.clone(),
            zero,
        ],
        IntegralFamilyLimits::default(),
    )
    .unwrap()
}

#[test]
fn direct_alpha_slot_map_is_denominator_verified_and_s4_equivalent_to_prose_route() {
    assert_eq!(
        certify_alpha_to_rust_map(),
        AlphaToRustMapCertificate {
            form_source_for_rust_target: [0, 2, 1, 3, 5, 4],
            direct_source_for_rust_target: [0, 1, 2, 5, 3, 4],
            direct_relative_to_form_s4: [0, 2, 1, 4, 3, 5],
        }
    );
}

#[test]
fn all_fifty_five_lhs_domains_canonicalize_with_authenticated_routes() {
    let materialized = materialize_alpha_loop_lhs_anchors();
    assert_eq!(materialized.len(), 55);
    assert!(
        materialized
            .windows(2)
            .all(|pair| pair[0].source.source_line < pair[1].source.source_line)
    );
    assert!(materialized.iter().all(|entry| {
        entry
            .canonical_route
            .verify(&entry.raw_integral, &entry.canonical_integral)
    }));
}

#[test]
fn custom_priority_selects_and_certifies_its_own_s4_representatives() {
    const WINNER: &str = "rustred.unshifted-sector-order.v1;priority=rustred.coordinate-priority.v1;k=6;rank-by-slot=5,3,4,2,0,1";
    let custom = OrderingPolicy::try_from_stable_id(WINNER).unwrap();
    let family = canonical_family().unwrap();
    let natural_canonicalizer = canonical_s4(&family).unwrap();
    let custom_canonicalizer = canonical_s4_with_ordering(&family, custom).unwrap();
    assert_eq!(natural_canonicalizer.ordering(), OrderingPolicy::default());
    assert_eq!(custom_canonicalizer.ordering(), custom);

    // Pairwise-distinct powers expose the non-S4-invariant final coordinate
    // tie-break without relying on an AlphaLoop rule or RHS witness.
    let witness = IntegralKey::try_new([1, 2, 3, 4, 5, 6]).unwrap();
    let natural = natural_canonicalizer.canonicalize(&witness).unwrap();
    let selected = custom_canonicalizer.canonicalize(&witness).unwrap();
    assert_ne!(natural.canonical(), selected.canonical());
    assert_eq!(selected.no_harder().policy(), custom);
    assert!(selected.verify());

    let materialized = materialize_alpha_loop_lhs_anchors_with_ordering(custom);
    assert_eq!(materialized.len(), 55);
    for entry in &materialized {
        let replayed = custom_canonicalizer
            .canonicalize(&entry.raw_integral)
            .unwrap();
        assert_eq!(replayed.canonical(), &entry.canonical_integral);
        assert_eq!(replayed.no_harder().policy(), custom);
        assert!(replayed.verify());
    }

    // The sealed terminal authority indexes every authenticated route, so a
    // custom-policy representative rejoins the same exact terminal owner
    // instead of silently creating a second master convention.
    let authority = derive_k6_terminal_authority().unwrap();
    let snapshot = ImmutableOwnerSnapshot::try_from_terminal_authority(
        authority.clone(),
        StratumRegistryLimits::default(),
    )
    .unwrap();
    for terminal in authority.master_terminals() {
        let custom_terminal = custom_canonicalizer.canonicalize(terminal).unwrap();
        assert!(
            snapshot
                .authenticates_explicit_terminal(custom_terminal.canonical())
                .unwrap()
        );
    }
}

#[test]
fn form_and_direct_graph_routes_canonicalize_all_semantic_domains_identically() {
    let certificate = certify_alpha_to_rust_map();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let form = materialize_alpha_loop_lhs_anchors();
    for entry in &form {
        let form_symbolic = parse_bits(entry.source.symbolic_bits);
        let relative = certificate.direct_relative_to_form_s4;
        let direct_integral = IntegralKey::try_new(
            relative
                .iter()
                .map(|&source| entry.raw_integral.powers()[source]),
        )
        .unwrap();
        let direct_symbolic = relative
            .iter()
            .map(|&source| form_symbolic[source])
            .collect::<Vec<_>>();
        let direct = canonicalizer.canonicalize(&direct_integral).unwrap();
        assert!(direct.verify());
        assert!(canonicalizer.authenticates_route(direct.route()));
        assert_eq!(direct.canonical(), &entry.canonical_integral);
        let direct_axes = direct
            .route()
            .source_for_target()
            .iter()
            .enumerate()
            .filter_map(|(target, &source)| direct_symbolic[source].then_some(target))
            .collect::<Vec<_>>();
        assert_eq!(direct_axes, entry.canonical_symbolic_axes.as_ref());
        let direct_sector = Mask::try_from_indices(direct.canonical().powers()).unwrap();
        let direct_point = SectorChart::new(direct_sector)
            .to_lattice(direct.canonical())
            .unwrap();
        assert_eq!(direct_point, entry.canonical_point);
    }
}

#[test]
fn lhs_domain_union_leaves_only_authenticated_finite_terminals() {
    let materialized = materialize_alpha_loop_lhs_anchors();
    let family = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&family).unwrap();
    let group = canonicalizer.group_elements().collect::<Vec<_>>();
    let mut per_sector: BTreeMap<Mask, Vec<LatticeBox>> = BTreeMap::new();
    for entry in &materialized {
        for symmetry in &group {
            let routed_sector = Mask::try_new(
                symmetry
                    .iter()
                    .map(|&source| entry.canonical_sector.active_bits()[source]),
            )
            .unwrap();
            if routed_sector != entry.canonical_sector {
                continue;
            }
            let lower = symmetry
                .iter()
                .map(|&source| entry.canonical_point.coordinates()[source])
                .collect::<Vec<_>>();
            let upper = symmetry.iter().map(|&source| {
                entry
                    .canonical_symbolic_axes
                    .binary_search(&source)
                    .is_err()
                    .then_some(entry.canonical_point.coordinates()[source])
            });
            per_sector
                .entry(entry.canonical_sector.clone())
                .or_default()
                .push(LatticeBox::try_new(lower, upper).unwrap());
        }
    }
    assert_eq!(per_sector.len(), 5);

    let authority = derive_k6_terminal_authority().unwrap();
    let terminal_canonicalizer = authority.canonicalizer().unwrap();
    for (sector, boxes) in per_sector {
        // Entries reach this union in FORM first-match chronology. BoxCover's
        // deterministic sort is semantically harmless because finite box
        // union is order-independent.
        let residual =
            BoxCover::try_new(sector.arity(), boxes, CompletionGeometryLimits::default())
                .unwrap()
                .uncovered_partition()
                .unwrap();
        assert!(
            residual
                .boxes()
                .iter()
                .all(|cell| cell.varying_dimension() == 0),
            "the LHS itinerary left a positive-dimensional complement in {sector:?}: {residual:?}"
        );
        for cell in residual.boxes() {
            assert_eq!(
                cell.lower().iter().copied().map(Some).collect::<Vec<_>>(),
                cell.upper()
            );
            let point = LatticePoint::try_new(cell.lower().iter().copied()).unwrap();
            let terminal = SectorChart::new(sector.clone())
                .to_integral(&point)
                .unwrap();
            assert!(!authority.is_zero_terminal(&terminal));
            let canonical_terminal = terminal_canonicalizer.canonicalize(&terminal).unwrap();
            assert!(authority.master_terminals().any(|candidate| {
                terminal_canonicalizer
                    .canonicalize(candidate)
                    .is_ok_and(|canonical| canonical.canonical() == canonical_terminal.canonical())
            }));
        }
    }
}
