use symbolica::domains::SelfRing;

use super::*;
use crate::algebra::{CoefficientContext, ExactAlgebraLimits};
use crate::family::AffineDenominator;
use crate::sector::symmetry::{DenominatorAction, Jacobian};

/// Vacuum family from integer line momenta; `deformed` lines get `-1 - eta`.
fn family(name: &str, vectors: &[&[i64]], deformed: &[usize]) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d", "eta"]);
    let eta = context.parameter("eta").unwrap();
    let loops = vectors[0].len();
    let denominators = vectors
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut coefficients = Vec::new();
            for left in 0..loops {
                for right in left..loops {
                    coefficients.push(
                        context.integer(row[left] * row[right] * if left == right { 1 } else { 2 }),
                    );
                }
            }
            let mut constant = context.integer(-1);
            if deformed.contains(&slot) {
                constant = context
                    .try_sub(&constant, &eta, ExactAlgebraLimits::default())
                    .unwrap();
            }
            AffineDenominator::new(constant, coefficients)
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            name,
            (0..loops).map(|index| format!("k{index}")).collect(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); vectors.len()],
        )
        .unwrap(),
    )
}

fn family_of(name: &str, deformed: &[usize]) -> Arc<IntegralFamily> {
    family(name, SUNRISE, deformed)
}

/// Two-loop sunrise lines `k1`, `k2`, `k1 + k2` and nothing else.
const SUNRISE: &[&[i64]] = &[&[1, 0], &[0, 1], &[1, 1]];

fn mask(bits: &[u8]) -> Mask {
    Mask::try_new(bits.iter().map(|&bit| bit == 1)).unwrap()
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn census(family: &Arc<IntegralFamily>, sectors: &[&[u8]]) -> SectorSymmetryCensus {
    SectorSymmetryCensus::discover(
        family.clone(),
        sectors.iter().map(|bits| mask(bits)),
        Default::default(),
        &Default::default(),
    )
    .unwrap()
}

const SUNRISE_SECTORS: &[&[u8]] = &[&[1, 1, 1], &[1, 1, 0], &[1, 0, 1], &[0, 1, 1]];

#[test]
fn unit_mass_sunrise_collapses_every_tadpole_product() {
    let family = family("unit_sunrise", SUNRISE, &[]);
    let census = census(&family, SUNRISE_SECTORS);
    assert_eq!(census.classes().len(), 2);
    let products = census.class_of(&mask(&[0, 1, 1])).unwrap();
    assert_eq!(products.representative(), &mask(&[1, 1, 0]));
    assert_eq!(
        products.members(),
        [mask(&[1, 1, 0]), mask(&[1, 0, 1]), mask(&[0, 1, 1])]
    );
    let sunrise = census.class_of(&mask(&[1, 1, 1])).unwrap();
    assert_eq!(sunrise.line_group_order(), Some(6));
    assert_eq!(products.line_group_order(), Some(2));
    let statistics = census.statistics();
    assert_eq!(statistics.declared_sectors, 4);
    assert_eq!(statistics.routed_sectors, 2);
    assert_eq!(statistics.nontrivial_line_automorphisms, Some(6));
}

#[test]
fn deformed_line_is_colored_apart_and_routes_stay_exact() {
    let family = family("deformed_sunrise", SUNRISE, &[0]);
    let census = census(&family, SUNRISE_SECTORS);
    assert_eq!(census.classes().len(), 3);
    assert_eq!(
        census.representative(&mask(&[1, 0, 1])),
        Some(&mask(&[1, 1, 0]))
    );
    assert_eq!(
        census.representative(&mask(&[0, 1, 1])),
        Some(&mask(&[0, 1, 1]))
    );
    // Only the two unit lines of the top sector may be interchanged.
    let top = census.class_of(&mask(&[1, 1, 1])).unwrap();
    assert_eq!(top.line_group_order(), Some(2));
    for images in top.line_permutations() {
        assert_eq!(images[0], 0);
    }
    for member in census.members() {
        let map = member.verified_map();
        assert!(matches!(map.jacobian(), Jacobian::Unit { .. }));
        for (slot, &active) in member.sector().active_bits().iter().enumerate() {
            if !active {
                continue;
            }
            let DenominatorAction::Monomial { target, scale } = &map.row_actions()[slot] else {
                panic!("active line has an affine image");
            };
            assert!(scale.is_one());
            assert!(member.representative().active_bits()[*target]);
            assert_eq!(
                family.denominators()[slot].constant(),
                family.denominators()[*target].constant()
            );
        }
    }
}

/// `2 + 2 eta` in the sunrise family context.
fn two_plus_two_eta(context: &CoefficientContext) -> Coefficient {
    let limits = ExactAlgebraLimits::default();
    context
        .try_mul(
            &context.integer(2),
            &context
                .try_add(&context.one(), &context.parameter("eta").unwrap(), limits)
                .unwrap(),
            limits,
        )
        .unwrap()
}

fn terms(pairs: &[(&[i64], Coefficient)]) -> Vec<(IntegralKey, Coefficient)> {
    let mut terms: Vec<_> = pairs
        .iter()
        .map(|(powers, coefficient)| (key(powers), coefficient.clone()))
        .collect();
    terms.sort_by(|left, right| left.0.cmp(&right.0));
    terms
}

#[test]
fn routed_numerator_transport_substitutes_the_deformed_constant() {
    let family = family("deformed_sunrise_numerator", SUNRISE, &[0]);
    let context = family.coefficient_context();
    let census = census(&family, SUNRISE_SECTORS);
    // Sector {0,2} -> {0,1} (D0 = k1^2 - 1 - eta, D1 = k2^2 - 1,
    // D2 = (k1 + k2)^2 - 1). The census deterministically picks
    // k1 = k1', k2 = k2' - k1', so D0 -> D0', D2 -> D1' and the inactive
    // unit line becomes (k2' - k1')^2 - 1 = 2 D0' + 2 D1' - D2' + 2 + 2 eta
    // (using 2 k1'.k2' = D2' - D0' - D1' - 1 - eta).
    let member = census.member(&mask(&[1, 0, 1])).unwrap();
    assert_eq!(member.representative(), &mask(&[1, 1, 0]));
    let map = member.verified_map();
    assert_eq!(
        map.momentum().loop_linear().entries(),
        [1, 0, -1, 1].map(|value| context.integer(value))
    );
    assert_eq!(
        map.denominators().constant(),
        [context.zero(), two_plus_two_eta(context), context.zero()]
    );
    assert_eq!(
        (0..3)
            .map(|column| map.denominators().linear().get(1, column).unwrap().clone())
            .collect::<Vec<_>>(),
        [2, 2, -1].map(|value| context.integer(value))
    );
    assert!(member.route().has_parametric_constants());
    // I(1,-1,2) = I'(1,2,0) times that numerator, expanded by hand.
    let source = key(&[1, -1, 2]);
    let transported = census.transport_to_representative(&source).unwrap();
    let expected = terms(&[
        (&[0, 2, 0], context.integer(2)),
        (&[1, 1, 0], context.integer(2)),
        (&[1, 2, -1], context.integer(-1)),
        (&[1, 2, 0], two_plus_two_eta(context)),
    ]);
    assert_eq!(
        transported
            .terms()
            .iter()
            .map(|term| (term.key().clone(), term.coefficient().clone()))
            .collect::<Vec<_>>(),
        expected
    );
    // I - T(I), sorted by key.
    let relation = census.route_relation(&source).unwrap().unwrap();
    assert_eq!(relation.source(), &source);
    assert_eq!(relation.automorphism(), None);
    let limits = ExactAlgebraLimits::default();
    let mut row: Vec<_> = expected
        .iter()
        .map(|(key, value)| (key.clone(), context.try_neg(value, limits).unwrap()))
        .chain(std::iter::once((source.clone(), context.one())))
        .collect();
    row.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(relation.terms(), row.as_slice());
    // Representatives have no route row.
    assert!(census.route_relation(&key(&[1, 2, -1])).unwrap().is_none());
    // The pinched endpoint I(0,2,0) lies in the undeclared tadpole {1}:
    // without a zero set the census cannot normalize it.
    assert!(matches!(
        census.normalize(&source),
        Err(SectorSymmetryCensusError::UnknownSector(sector)) if sector == mask(&[0, 1, 0])
    ));
}

#[test]
fn automorphism_rows_relate_interchanged_unit_lines() {
    let family = family("deformed_sunrise_rows", SUNRISE, &[0]);
    let census = census(&family, SUNRISE_SECTORS);
    let rows = census.automorphism_relations(&key(&[1, 2, 1])).unwrap();
    let context = family.coefficient_context();
    assert!(rows.iter().any(|row| {
        row.terms()
            == [
                (key(&[1, 1, 2]), context.integer(-1)),
                (key(&[1, 2, 1]), context.one()),
            ]
    }));
    // A symmetric numerator-free key is invariant: no nonzero row survives.
    assert!(
        census
            .automorphism_relations(&key(&[2, 1, 1]))
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        census.automorphism_relations(&key(&[1, 0, 1])),
        Err(SectorSymmetryCensusError::NotRepresentative(_))
    ));
}

#[test]
fn equal_canonical_keys_with_non_unit_lattice_stay_separate() {
    // (k, l) and (l, 2k + l) share the two-line projector graph, but every
    // map between them has determinant two.
    let family = family("doubled_lattice", &[&[1, 0], &[0, 1], &[2, 1]], &[]);
    let census = census(&family, &[&[1, 1, 0], &[0, 1, 1]]);
    assert_eq!(census.classes().len(), 2);
    assert_eq!(census.statistics().shared_canonical_keys, 1);
    assert_eq!(
        census.member(&mask(&[1, 1, 0])).unwrap().canonical_key(),
        census.member(&mask(&[0, 1, 1])).unwrap().canonical_key()
    );
}

#[test]
fn representative_policy_and_input_order_do_not_change_the_partition() {
    let family = family("ordered_sunrise", SUNRISE, &[]);
    let forward = census(&family, SUNRISE_SECTORS);
    let mut reversed_sectors = SUNRISE_SECTORS.to_vec();
    reversed_sectors.reverse();
    let reversed = census(&family, &reversed_sectors);
    let highest = SectorSymmetryCensus::discover(
        family.clone(),
        SUNRISE_SECTORS.iter().map(|bits| mask(bits)),
        SectorSymmetryCensusOptions {
            representative: RepresentativePolicy::HighestSectorId,
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    for (left, right) in forward.members().zip(reversed.members()) {
        assert_eq!(left.sector(), right.sector());
        assert_eq!(left.representative(), right.representative());
        assert_eq!(
            left.verified_map().momentum(),
            right.verified_map().momentum()
        );
    }
    assert_eq!(
        highest.representative(&mask(&[1, 1, 0])),
        Some(&mask(&[0, 1, 1]))
    );
    assert_eq!(highest.classes().len(), forward.classes().len());
}

#[test]
fn subsector_census_skips_analyzer_zero_sectors() {
    let family = family("subsector_sunrise", SUNRISE, &[0]);
    let analyzer = Analyzer::try_unrestricted(&family).unwrap();
    let census = SectorSymmetryCensus::discover_subsectors(
        family.clone(),
        &mask(&[1, 1, 1]),
        &analyzer,
        Default::default(),
        &Default::default(),
    )
    .unwrap();
    // Single tadpoles are scaleless at two loops; four sectors remain.
    assert_eq!(census.statistics().enumerated_sectors, 7);
    assert_eq!(census.statistics().zero_sectors, 3);
    assert_eq!(census.statistics().declared_sectors, 4);
    assert_eq!(census.classes().len(), 3);
    let other = Analyzer::try_unrestricted(&family_of("other", &[])).unwrap();
    assert!(matches!(
        SectorSymmetryCensus::discover_subsectors(
            family,
            &mask(&[1, 1, 1]),
            &other,
            Default::default(),
            &Default::default(),
        ),
        Err(SectorSymmetryCensusError::ForeignAnalyzer)
    ));
}

#[test]
fn malformed_requests_are_typed_errors() {
    let family = family("malformed_sunrise", SUNRISE, &[]);
    assert!(matches!(
        SectorSymmetryCensus::discover(
            family.clone(),
            [mask(&[0, 0, 0])],
            Default::default(),
            &Default::default()
        ),
        Err(SectorSymmetryCensusError::EmptySector)
    ));
    assert!(matches!(
        SectorSymmetryCensus::discover(
            family.clone(),
            [mask(&[1, 1])],
            Default::default(),
            &Default::default()
        ),
        Err(SectorSymmetryCensusError::WrongArity { .. })
    ));
    let census = census(&family, &[&[1, 1, 1]]);
    assert!(matches!(
        census.transport_to_representative(&key(&[1, 1, 0])),
        Err(SectorSymmetryCensusError::UnknownSector(_))
    ));
    let cancelled = SectorSymmetryCensus::discover(
        family,
        [mask(&[1, 1, 1])],
        Default::default(),
        &SectorSymmetryCensusControl {
            cancelled: Some(&|| true),
            progress: None,
        },
    );
    assert!(matches!(
        cancelled,
        Err(SectorSymmetryCensusError::Discovery(
            DiscoveryError::Cancelled
        ))
    ));
}

#[test]
fn sector_ids_order_slot_zero_least_significant_and_census_is_shareable() {
    fn shareable<T: Send + Sync>() {}
    shareable::<SectorSymmetryCensus>();
    // ids 3 = {0,1}, 5 = {0,2}, 6 = {1,2}, 4 = {2}.
    assert_eq!(
        sector_id_cmp(&mask(&[1, 1, 0]), &mask(&[1, 0, 1])),
        Ordering::Less
    );
    assert_eq!(
        sector_id_cmp(&mask(&[0, 1, 1]), &mask(&[1, 0, 1])),
        Ordering::Greater
    );
    assert_eq!(
        sector_id_cmp(&mask(&[0, 0, 1]), &mask(&[1, 1, 0])),
        Ordering::Greater
    );
    assert_eq!(
        sector_id_cmp(&mask(&[0, 1, 1]), &mask(&[0, 1, 1])),
        Ordering::Equal
    );
}

fn subsector_census(
    family: &Arc<IntegralFamily>,
    options: SectorSymmetryCensusOptions,
) -> SectorSymmetryCensus {
    let analyzer = Analyzer::try_unrestricted(family).unwrap();
    let top = Mask::try_new(std::iter::repeat_n(true, family.denominator_count())).unwrap();
    SectorSymmetryCensus::discover_subsectors(
        family.clone(),
        &top,
        &analyzer,
        options,
        &Default::default(),
    )
    .unwrap()
}

#[test]
fn zero_set_is_retained_and_normal_forms_drop_it() {
    let family = family("zero_set_sunrise", SUNRISE, &[0]);
    let context = family.coefficient_context();
    let census = subsector_census(&family, Default::default());
    // Tadpoles and the empty sector are scaleless at two loops.
    let zeros: Vec<_> = census
        .zero_sectors()
        .map(|(mask, _)| mask.clone())
        .collect();
    assert_eq!(
        zeros,
        [
            mask(&[0, 0, 0]),
            mask(&[0, 0, 1]),
            mask(&[0, 1, 0]),
            mask(&[1, 0, 0])
        ]
    );
    assert!(
        census
            .zero_sectors()
            .all(|(_, kind)| kind == ZeroSectorKind::ProvedZero)
    );
    assert!(census.is_zero_sector(&mask(&[0, 1, 0])));
    assert!(!census.is_zero_sector(&mask(&[1, 0, 1])));
    assert_eq!(census.statistics().zero_sectors, 3);

    // Hand expansion of the {0,2} -> {0,1} route (see the test above); the
    // tadpole endpoint 2 I(0,2,0) vanishes.
    let source = key(&[1, -1, 2]);
    let normal = census.normalize(&source).unwrap();
    let expected = terms(&[
        (&[1, 1, 0], context.integer(2)),
        (&[1, 2, -1], context.integer(-1)),
        (&[1, 2, 0], two_plus_two_eta(context)),
    ]);
    assert_eq!(normal.source(), &source);
    assert_eq!(normal.terms(), expected.as_slice());
    assert_eq!((normal.transports(), normal.dropped_zero_terms()), (1, 1));
    // Representative integrals are their own normal form; zero ones vanish.
    let rep = census.normalize(&key(&[1, 2, -1])).unwrap();
    assert_eq!(
        rep.terms(),
        terms(&[(&[1, 2, -1], context.one())]).as_slice()
    );
    assert_eq!(rep.transports(), 0);
    let zero = census.normalize(&key(&[0, 2, -1])).unwrap();
    assert!(zero.terms().is_empty());
    assert_eq!(zero.dropped_zero_terms(), 1);
    // The batch API shares routes and returns the same normal forms.
    let batch = census
        .normalize_many([&source, &key(&[1, 2, -1]), &source])
        .unwrap();
    assert_eq!(batch, [normal.clone(), rep, normal]);

    // Relation endpoint treatments.
    let limits = ExactAlgebraLimits::default();
    let row = |pairs: Vec<(IntegralKey, Coefficient)>| {
        let mut row: Vec<_> = pairs
            .into_iter()
            .map(|(key, value)| (key, context.try_neg(&value, limits).unwrap()))
            .chain(std::iter::once((source.clone(), context.one())))
            .collect();
        row.sort_by(|left, right| left.0.cmp(&right.0));
        row
    };
    let transported = census.route_relation(&source).unwrap().unwrap();
    assert_eq!(transported.terms().len(), 5);
    assert!(
        transported
            .terms()
            .iter()
            .any(|(key, _)| key.powers() == [0, 2, 0])
    );
    for endpoints in [
        RelationEndpoints::DropZero,
        RelationEndpoints::Representatives,
    ] {
        let relation = census
            .route_relation_with(&source, endpoints)
            .unwrap()
            .unwrap();
        assert_eq!(relation.terms(), row(expected.clone()).as_slice());
    }
    // Zero-set integrals own no rows; undeclared nonzero sectors stay
    // distinct errors.
    assert!(matches!(
        census.route_relation(&key(&[0, 1, 0])),
        Err(SectorSymmetryCensusError::ZeroSector(_))
    ));
    assert!(matches!(
        census.automorphism_relations(&key(&[0, 0, 0])),
        Err(SectorSymmetryCensusError::ZeroSector(_))
    ));
}

/// Three-loop vacuum lines: the six lines of the Mercedes topology.
const MERCEDES: &[&[i64]] = &[
    &[1, 0, 0],
    &[0, 1, 0],
    &[0, 0, 1],
    &[1, -1, 0],
    &[0, 1, -1],
    &[1, 0, -1],
];

/// Independent reference: plain recursion without coalescing.
fn naive_normal_form(
    census: &SectorSymmetryCensus,
    source: &IntegralKey,
) -> BTreeMap<IntegralKey, Coefficient> {
    let context = census.family().coefficient_context();
    let limits = ExactAlgebraLimits::unbounded_resources();
    let sector = Mask::try_from_indices(source.powers()).unwrap();
    if census.is_zero_sector(&sector) {
        return BTreeMap::new();
    }
    if census.member(&sector).unwrap().is_representative() {
        return BTreeMap::from([(source.clone(), context.one())]);
    }
    let mut result = BTreeMap::new();
    for term in census.transport_to_representative(source).unwrap().terms() {
        for (key, value) in naive_normal_form(census, term.key()) {
            let value = context.try_mul(term.coefficient(), &value, limits).unwrap();
            let sum = match result.get(&key) {
                Some(old) => context.try_add(old, &value, limits).unwrap(),
                None => value,
            };
            if sum.is_zero() {
                result.remove(&key);
            } else {
                result.insert(key, sum);
            }
        }
    }
    result
}

#[test]
fn recursive_normal_forms_match_plain_recursion_and_land_on_representatives() {
    for deformed in [&[][..], &[0][..]] {
        let family = family("mercedes", MERCEDES, deformed);
        let census = subsector_census(&family, Default::default());
        assert!(census.skips().is_empty());
        let mut keys = Vec::new();
        for member in census.members() {
            let bits = member.sector().active_bits();
            let base: Vec<i64> = bits.iter().map(|&active| i64::from(active)).collect();
            for (slot, &active) in bits.iter().enumerate() {
                if active {
                    continue;
                }
                let mut numerator = base.clone();
                numerator[slot] = -1;
                keys.push(key(&numerator));
                numerator[slot] = -2;
                keys.push(key(&numerator));
                for (other, &active) in bits.iter().enumerate().skip(slot + 1) {
                    if !active {
                        let mut pair = base.clone();
                        pair[slot] = -1;
                        pair[other] = -1;
                        keys.push(key(&pair));
                    }
                }
            }
        }
        let batch = census.normalize_many(keys.iter()).unwrap();
        let mut deepest = 0;
        let mut dropped = 0;
        for (source, normal) in keys.iter().zip(&batch) {
            let reference = naive_normal_form(&census, source);
            assert_eq!(
                normal.terms(),
                reference.into_iter().collect::<Vec<_>>().as_slice(),
                "{source:?}"
            );
            for (key, _) in normal.terms() {
                let sector = Mask::try_from_indices(key.powers()).unwrap();
                assert!(census.member(&sector).unwrap().is_representative());
            }
            assert_eq!(normal, &census.normalize(source).unwrap());
            deepest = deepest.max(normal.transports());
            dropped += normal.dropped_zero_terms();
        }
        // Some route pinches onto a non-representative subsector, and some
        // endpoints vanish in the zero set.
        assert!(deepest >= 2, "deformed {deformed:?}: no recursive route");
        assert!(dropped > 0);
        // Relation rows routed onto representatives match the normal forms.
        for source in &keys {
            let sector = Mask::try_from_indices(source.powers()).unwrap();
            if census.member(&sector).unwrap().is_representative() {
                continue;
            }
            let row = census
                .route_relation_with(source, RelationEndpoints::Representatives)
                .unwrap();
            let normal = census.normalize(source).unwrap();
            let row = row.map(|row| row.terms().to_vec()).unwrap_or_default();
            let expected: Vec<_> = normal
                .terms()
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        family
                            .coefficient_context()
                            .try_neg(value, ExactAlgebraLimits::default())
                            .unwrap(),
                    )
                })
                .chain(std::iter::once((
                    source.clone(),
                    family.coefficient_context().one(),
                )))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect();
            assert_eq!(row, expected);
        }
    }
}

#[test]
fn normalization_limits_are_opt_in_typed_diagnostics() {
    let family = family("limited_mercedes", MERCEDES, &[]);
    let census = subsector_census(
        &family,
        SectorSymmetryCensusOptions {
            normalization: NormalizationLimits {
                max_terms: usize::MAX,
                max_transports: 0,
                diagnostics_enabled: false,
            },
            ..Default::default()
        },
    );
    let routed = census
        .members()
        .find(|member| !member.is_representative())
        .unwrap();
    let powers: Vec<i64> = routed
        .sector()
        .active_bits()
        .iter()
        .map(|&active| i64::from(active))
        .collect();
    // Count thresholds are opt-in diagnostics: ordinary execution ignores
    // them and relies on the inherited Symbolica RSS/time budget.
    let ordinary = census.normalize(&key(&powers)).unwrap();
    assert!(ordinary.transports() > 0);
    let diagnostic = SectorSymmetryCensus::discover_subsectors(
        family.clone(),
        &Mask::try_new([true; 6]).unwrap(),
        &Analyzer::try_unrestricted(&family).unwrap(),
        SectorSymmetryCensusOptions {
            normalization: census.options().normalization.with_diagnostics(),
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    assert!(matches!(
        diagnostic.normalize(&key(&powers)),
        Err(SectorSymmetryCensusError::ResourceLimit {
            resource: "normal-form transports",
            ..
        })
    ));
}

#[test]
fn strict_affine_constants_skip_parametric_routes_without_aborting() {
    let family = family("strict_sunrise", SUNRISE, &[0]);
    let strict = SectorSymmetryCensus::discover(
        family.clone(),
        SUNRISE_SECTORS.iter().map(|bits| mask(bits)),
        SectorSymmetryCensusOptions {
            affine_constants: AffineConstants::Rational,
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    // The {0,2} -> {0,1} route needs the constant 2 + 2 eta: the strict
    // compiler refuses it, so {0,2} keeps its own class and the census
    // reports why instead of failing.
    assert_eq!(strict.classes().len(), 4);
    assert_eq!(
        strict.representative(&mask(&[1, 0, 1])),
        Some(&mask(&[1, 0, 1]))
    );
    // Self-maps whose inactive rows need eta (for example k2 -> -k2 on
    // {0,1}, where D2 -> 2 D0 + 2 D1 - D2 + 2 + 2 eta) are dropped too; their
    // classes then report no group order rather than a too-small one.
    for skip in strict.skips() {
        eprintln!("{} {:?}: {}", skip.sector(), skip.stage(), skip.reason());
    }
    assert_eq!(strict.statistics().skips, strict.skips().len());
    let routes: Vec<_> = strict
        .skips()
        .iter()
        .filter(|skip| matches!(skip.stage(), SectorSymmetrySkipStage::Route { .. }))
        .collect();
    let [route] = routes.as_slice() else {
        panic!("one route skip expected")
    };
    assert_eq!(route.sector(), &mask(&[1, 0, 1]));
    assert_eq!(
        route.stage(),
        &SectorSymmetrySkipStage::Route {
            representative: mask(&[1, 1, 0])
        }
    );
    for skip in strict.skips() {
        assert!(matches!(
            skip.reason(),
            SectorSymmetrySkipReason::Transport(
                integral_transport::Error::NonconstantCoefficient { column: None, .. }
            )
        ));
        if let SectorSymmetrySkipStage::AutomorphismGenerator { .. } = skip.stage() {
            assert_eq!(
                strict.class_of(skip.sector()).unwrap().line_group_order(),
                None
            );
        } else {
            assert!(std::ptr::eq(skip, *route));
        }
    }
    assert_eq!(strict.statistics().nontrivial_line_automorphisms, None);
    assert!(
        strict
            .members()
            .all(|member| !member.route().has_parametric_constants())
    );
    // Rational-constant families lose nothing under the strict policy.
    let unit = family_of("strict_unit_sunrise", &[]);
    let strict_unit = SectorSymmetryCensus::discover(
        unit.clone(),
        SUNRISE_SECTORS.iter().map(|bits| mask(bits)),
        SectorSymmetryCensusOptions {
            affine_constants: AffineConstants::Rational,
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    assert!(strict_unit.skips().is_empty());
    assert_eq!(
        strict_unit.classes().len(),
        census(&unit, SUNRISE_SECTORS).classes().len()
    );
}

#[test]
fn preferred_representatives_follow_the_caller_list() {
    let family = family("preferred_sunrise", SUNRISE, &[]);
    let preferred = SectorSymmetryCensus::discover(
        family.clone(),
        SUNRISE_SECTORS.iter().map(|bits| mask(bits)),
        SectorSymmetryCensusOptions {
            representative: RepresentativePolicy::Preferred(
                vec![mask(&[1, 0, 1]), mask(&[0, 1, 1]), mask(&[1, 0, 1])].into(),
            ),
            ..Default::default()
        },
        &Default::default(),
    )
    .unwrap();
    let products = preferred.class_of(&mask(&[1, 1, 0])).unwrap();
    assert_eq!(products.representative(), &mask(&[1, 0, 1]));
    // Listed members first, in list order; the rest by lowest sector id.
    assert_eq!(
        products.members(),
        [mask(&[1, 0, 1]), mask(&[0, 1, 1]), mask(&[1, 1, 0])]
    );
    let lowest = census(&family, SUNRISE_SECTORS);
    assert_eq!(preferred.classes().len(), lowest.classes().len());
    assert!(matches!(
        SectorSymmetryCensus::discover(
            family,
            [mask(&[1, 1, 1])],
            SectorSymmetryCensusOptions {
                representative: RepresentativePolicy::Preferred(vec![mask(&[1, 1])].into()),
                ..Default::default()
            },
            &Default::default(),
        ),
        Err(SectorSymmetryCensusError::WrongArity { .. })
    ));
}
