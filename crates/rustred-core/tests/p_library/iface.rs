//! Frozen p-library interfaces: tags, terminal codec, class ids, presentations,
//! contracts, builder, families (owner: M2-I0 rr-iface).
use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::propagator::census::{
    PCensus, PCensusOptions, PClassRecord, PSectorFate, SignedAffineLoopMap, p_census,
};
use rustred::propagator::class::{ClassLimits, ClassedSector, OwnerClassKey, classify, route};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile, load_p_families};
use rustred::propagator::lane::{
    BubbleRoles, CarpetRoles, LaneDecision, LaneRoles, OwnerLane, TriangleRoles,
};
use rustred::propagator::library::{
    OwnerProgram, PBasisMap, PLibrary, PLibraryBuilder, POwnerSpec, PReduction, PReductionRequest,
    PTerm, PTerminal,
};
use rustred::propagator::present::{edge_map, present};
use rustred::propagator::presentation::{
    AffineSlotForm, EdgeKind, EdgeMap, NodeRef, Presentation, PresentationKind, Slot, SlotIsp,
    SlotLine, check_carpet_contract, check_insertion_contract, check_triangle_contract,
    verify_edge_map,
};
use rustred::propagator::programs::lane_program;
use rustred::propagator::step::{PKeyView, PLaneStep, PStepContext, PStepOut};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::terminal::PTerminalCodec;
use rustred::propagator::topology::{
    ChildMode, Dag, DagEdge, NodeInfo, PTopo, canonical, reduction_graph,
};
use rustred::propagator::{
    NodeIndex, OwnerClassId, P_FAMILIES_SCHEMA, P_FAMILIES_VERSION, P_GAMMA_TAG_WIDTH, PTerminalId,
    PresentationId, p_versions,
};
use symbolica::prelude::Rational;

use crate::support::p_integrals::compile;

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn line(momentum: &[i64]) -> Slot {
    eps_line(momentum, 0)
}

fn eps_line(momentum: &[i64], eps_units: u8) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units,
    })
}

fn isp(left: &[i64], right: &[i64]) -> Slot {
    Slot::Isp(SlotIsp {
        left: left.to_vec(),
        right: right.to_vec(),
    })
}

fn hand_built(id: u32, loops: u8, slots: Vec<Slot>) -> Presentation {
    Presentation::from_slots(
        PresentationId(id),
        PresentationKind::EpsNode,
        &format!("hand_built_{id}"),
        loops,
        slots,
        &context(),
        None,
    )
    .unwrap()
}

fn bubble() -> Presentation {
    hand_built(0, 1, vec![line(&[1, 0]), line(&[1, -1])])
}

fn kite() -> Presentation {
    hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[0, 1, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, -1]),
            line(&[1, -1, 0]),
        ],
    )
}

fn random_tag(rng: &mut StdRng) -> GammaTag {
    let pairs: Vec<(i8, i16)> = (0..rng.random_range(0..6))
        .map(|_| (rng.random_range(-8..=8), rng.random_range(-30..=30)))
        .collect();
    GammaTag::from_pairs(pairs).unwrap()
}

fn assert_contract<T>(result: Result<T, PError>) {
    assert!(matches!(result, Err(PError::Contract(_))));
}

fn assert_malformed<T>(result: Result<T, PError>) {
    assert!(matches!(result, Err(PError::MalformedLibrary(_))));
}

#[test]
fn gamma_tags_add_neg_and_round_trip_dense() {
    let mut rng = StdRng::seed_from_u64(0x7a6);
    for _ in 0..2_000 {
        let (a, b) = (random_tag(&mut rng), random_tag(&mut rng));
        let exponents = a.exponents();
        assert!(exponents.iter().all(|&(m, e)| m != 0 && e != 0));
        assert!(exponents.windows(2).all(|pair| pair[0].0 < pair[1].0));

        let mut model = BTreeMap::new();
        for &(m, e) in a.exponents().iter().chain(b.exponents()) {
            *model.entry(m).or_insert(0) += e;
        }
        model.retain(|_, e| *e != 0);
        let sum = a.add(&b);
        assert_eq!(sum.exponents(), model.into_iter().collect::<Vec<_>>());
        assert_eq!(sum, b.add(&a));
        assert_eq!(sum.add(&b.neg()), a);
        assert_eq!(a.neg().neg(), a);
        assert!(a.add(&a.neg()).is_one());

        let mut slots = vec![7];
        a.encode_dense(&mut slots).unwrap();
        assert_eq!(slots.len(), 1 + P_GAMMA_TAG_WIDTH);
        assert_eq!(GammaTag::decode_dense(&slots[1..]).unwrap(), a);
    }
    assert!(GammaTag::one().is_one());
    assert_eq!(
        GammaTag::from_pairs([(0, 5), (2, 3), (2, -3)]),
        Ok(GammaTag::one())
    );
    let limits = GammaTag::from_pairs([(-8, -127), (8, 127)]).unwrap();
    let mut slots = Vec::new();
    limits.encode_dense(&mut slots).unwrap();
    assert_eq!((slots[0], slots[P_GAMMA_TAG_WIDTH - 1]), (-127, 127));
    assert_eq!(GammaTag::decode_dense(&slots).unwrap(), limits);
}

#[test]
fn gamma_tags_out_of_range_fail_closed() {
    for pairs in [
        vec![(9, 1)],
        vec![(-9, 1)],
        vec![(1, 128)],
        vec![(1, -128)],
        vec![(1, i16::MAX)],
        vec![(1, i16::MIN)],
        vec![(1, 100), (1, 100)],
    ] {
        assert_eq!(GammaTag::from_pairs(pairs), Err(PError::TagOutOfRange));
    }

    let large = GammaTag::from_pairs([(1, 100)]).unwrap();
    let doubled = large.add(&large);
    let mut slots = vec![1];
    assert_eq!(doubled.encode_dense(&mut slots), Err(PError::TagOutOfRange));
    assert_eq!(slots, [1], "a refused encoding leaves the key untouched");

    // Exponents saturate far outside the key range, so they stay refused.
    let mut saturated = doubled;
    for _ in 0..12 {
        saturated = saturated.add(&saturated);
    }
    assert_eq!(saturated.exponents(), [(1, i16::MAX)]);
    assert_eq!(
        saturated.encode_dense(&mut slots),
        Err(PError::TagOutOfRange)
    );
    assert_eq!(
        saturated.neg().encode_dense(&mut slots),
        Err(PError::TagOutOfRange)
    );

    let mut saturated = GammaTag::from_pairs([(1, -100)]).unwrap();
    for _ in 0..12 {
        saturated = saturated.add(&saturated);
    }
    assert_eq!(saturated.exponents(), [(1, i16::MIN)]);
    assert_eq!(
        saturated.encode_dense(&mut slots),
        Err(PError::TagOutOfRange)
    );
    assert_eq!(saturated.neg().exponents(), [(1, i16::MAX)]);
    assert_eq!(slots, [1]);

    assert_eq!(GammaTag::decode_dense(&[0; 15]), Err(PError::TagOutOfRange));
    let mut wide = [0; P_GAMMA_TAG_WIDTH];
    wide[3] = 128;
    assert_eq!(GammaTag::decode_dense(&wide), Err(PError::TagOutOfRange));
    wide[3] = i64::MAX;
    assert_eq!(GammaTag::decode_dense(&wide), Err(PError::TagOutOfRange));
    wide[3] = i64::from(i16::MIN);
    assert_eq!(GammaTag::decode_dense(&wide), Err(PError::TagOutOfRange));
}

#[test]
fn terminal_codec_round_trips_random_keys_within_i8() {
    let mut rng = StdRng::seed_from_u64(0xc0dec);
    for _ in 0..10_000 {
        let id = PTerminalId(rng.random_range(0..PTerminalCodec::ID_LIMIT));
        let tag = random_tag(&mut rng);
        let key = PTerminalCodec::encode(id, &tag).unwrap();
        assert_eq!(key.powers().len(), PTerminalCodec::KEY_SLOTS);
        assert!(key.powers().iter().all(|&slot| i8::try_from(slot).is_ok()));
        assert_eq!(PTerminalCodec::decode(&key).unwrap(), (id, tag));
    }

    let id = PTerminalId(2 * 128 * 128 + 3 * 128 + 5);
    let key = PTerminalCodec::encode(id, &GammaTag::one()).unwrap();
    assert_eq!(key.powers()[..3], [2, 3, 5]);
    let last = PTerminalId(PTerminalCodec::ID_LIMIT - 1);
    let key = PTerminalCodec::encode(last, &GammaTag::one()).unwrap();
    assert_eq!(key.powers()[..3], [127, 127, 127]);

    assert_malformed(PTerminalCodec::encode(
        PTerminalId(PTerminalCodec::ID_LIMIT),
        &GammaTag::one(),
    ));
    assert_malformed(PTerminalCodec::decode(
        &IntegralKey::try_new([1, 2, 3]).unwrap(),
    ));
    let mut digits = vec![0; PTerminalCodec::KEY_SLOTS];
    digits[1] = 128;
    assert_malformed(PTerminalCodec::decode(
        &IntegralKey::try_new(digits).unwrap(),
    ));
    let mut tag_slots = vec![0; PTerminalCodec::KEY_SLOTS];
    tag_slots[4] = i64::from(i16::MIN);
    assert_eq!(
        PTerminalCodec::decode(&IntegralKey::try_new(tag_slots).unwrap()),
        Err(PError::TagOutOfRange)
    );
}

fn class_key(bytes: &[u8]) -> OwnerClassKey {
    OwnerClassKey {
        loops: 3,
        lines: 8,
        eps_lines: 0,
        canonical: Arc::from(bytes),
    }
}

fn class_record(bytes: &[u8], owner: u32) -> PClassRecord {
    PClassRecord {
        key: class_key(bytes),
        owner: (PresentationId(owner), 0b11),
        node: None,
        nodes: Vec::new(),
        t: 2,
        eps_lines: 0,
    }
}

/// A 1L sector routed onto `class` by the identity.
fn route_to(class: OwnerClassId) -> PSectorFate {
    PSectorFate::Owner {
        class,
        map: unit_map(0, 1),
    }
}

fn owner_spec(class: OwnerClassId, presentation: u32) -> POwnerSpec {
    POwnerSpec {
        class,
        lane: OwnerLane::Insertion,
        presentation: PresentationId(presentation),
        node: None,
        roles: LaneRoles::None,
        edge_maps: Vec::new(),
        payload: None,
        custom: None,
        terminals: Vec::new(),
    }
}

#[test]
fn owner_class_id_and_label_are_pinned() {
    let key = class_key(b"rustred owner class golden");
    assert_eq!(key.id(), OwnerClassId(0x96fe48817f947061));
    assert_eq!(key.label(), "L3T08-96fe48817f947061");
}

#[test]
fn a_forced_class_id_collision_is_malformed() {
    let first = class_record(b"first canonical bytes", 0);
    let second = class_record(b"second canonical bytes", 0);
    let id = first.key.id();

    let mut census = PCensus::default();
    assert_eq!(census.insert_class(first.clone()), Ok(id));
    assert_eq!(census.insert_class(first.clone()), Ok(id));
    assert_malformed(census.insert_class_as(id, second.clone()));
    assert_eq!(census.classes[&id], first, "the first key stays");

    // The same canonical bytes with another record never overwrite it.
    let moved_owner = PClassRecord {
        owner: (PresentationId(7), 0b1),
        ..first.clone()
    };
    let other_nodes = PClassRecord {
        nodes: vec![NodeIndex(3)],
        ..first.clone()
    };
    let mut other_shape = first.clone();
    other_shape.key.loops = 2;
    for record in [moved_owner, other_nodes, other_shape] {
        assert_malformed(census.insert_class(record));
    }
    assert_eq!(census.classes[&id], first);

    let mut builder = PLibrary::builder(context(), 1..=1);
    builder.add_presentation(bubble()).unwrap();
    builder.add_owner(owner_spec(id, 0)).unwrap();
    census.classes.insert(id, second);
    builder.set_census(census).unwrap();
    assert_malformed(builder.build());
}

#[test]
fn every_census_owner_needs_an_owner_record() {
    let record = class_record(b"bubble", 0);
    let id = record.key.id();
    let mut census = PCensus::default();
    census.insert_class(record).unwrap();

    let with = |owners: Vec<POwnerSpec>, census: PCensus| -> Result<PLibrary, PError> {
        let mut builder = PLibrary::builder(context(), 1..=1);
        builder.add_presentation(bubble())?;
        for owner in owners {
            builder.add_owner(owner)?;
        }
        builder.set_census(census)?;
        builder.build()
    };
    assert_malformed(with(vec![], census.clone()));
    assert_malformed(with(
        vec![owner_spec(OwnerClassId(id.0 ^ 1), 0)],
        census.clone(),
    ));
    assert_malformed(with(
        vec![owner_spec(id, 0), owner_spec(id, 0)],
        census.clone(),
    ));
    let mut broken = owner_spec(id, 0);
    broken.edge_maps = vec![0];
    assert_malformed(with(vec![broken], census.clone()));
    let mut broken = owner_spec(id, 0);
    broken.terminals = vec![PTerminalId(1)];
    assert_malformed(with(vec![broken], census.clone()));

    let mut unknown = census.clone();
    let unknown_route = route_to(OwnerClassId(id.0 ^ 1));
    unknown
        .sectors
        .insert((PresentationId(0), 0b01), unknown_route);
    assert_malformed(with(vec![owner_spec(id, 0)], unknown));

    with(vec![owner_spec(id, 0)], census).unwrap();
}

#[test]
fn the_builder_fails_closed() {
    let wrong_id = hand_built(3, 1, vec![line(&[1, 0]), line(&[1, -1])]);
    assert_malformed(PLibrary::builder(context(), 1..=1).add_presentation(wrong_id));
    assert_malformed(PLibrary::builder(context(), 1..=1).add_presentation(kite()));
    assert_malformed(PLibrary::builder(context(), 2..=3).add_presentation(bubble()));

    let two_scales = CoefficientContext::try_new(["d", "x"]).unwrap();
    let mut foreign = PLibrary::builder(two_scales.clone(), 1..=1);
    assert_eq!(
        foreign.add_presentation(bubble()).unwrap_err(),
        PError::ContextMismatch
    );
    foreign.set_census(PCensus::default()).unwrap();
    assert_eq!(foreign.build().unwrap_err(), PError::ContextMismatch);

    assert_malformed(PLibrary::builder(context(), 1..=1).build());

    let mut twice = PLibrary::builder(context(), 1..=1);
    twice.set_census(PCensus::default()).unwrap();
    assert_malformed(twice.set_census(PCensus::default()));
    twice.set_topology(Arc::new(Dag::default())).unwrap();
    assert_malformed(twice.set_topology(Arc::new(Dag::default())));

    let mut duplicate = PLibrary::builder(context(), 1..=1);
    for id in 0..2 {
        let mut top = bubble();
        top.id = PresentationId(id);
        top.kind = PresentationKind::Top {
            name: "P1L".to_owned(),
        };
        duplicate.add_presentation(top).unwrap();
    }
    duplicate.set_census(PCensus::default()).unwrap();
    assert_malformed(duplicate.build());
}

#[test]
fn the_library_exposes_what_it_was_built_from() {
    fn thread_safe<T: Send + Sync>() {}
    thread_safe::<PLibrary>();
    thread_safe::<PError>();

    let (_, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    let scope = 1..=3;
    let top_context = tops[0].family.coefficient_context().clone();
    let mut builder = PLibrary::builder(top_context, scope.clone());
    for top in tops.iter().filter(|top| scope.contains(&top.loops)) {
        builder.add_presentation((**top).clone()).unwrap();
    }

    let record = class_record(b"kite", 1);
    let id = record.key.id();
    assert_eq!(builder.add_terminal(PTerminal::Unit), Ok(PTerminalId(0)));
    let key = IntegralKey::try_new([1, 1, 1, 1, 1]).unwrap();
    let residual = PTerminal::Residual { owner: id, key };
    assert_eq!(builder.add_terminal(residual.clone()), Ok(PTerminalId(1)));
    assert_eq!(builder.add_terminal(residual.clone()), Ok(PTerminalId(1)));

    let mut census = PCensus::default();
    census.insert_class(record).unwrap();
    builder.set_census(census.clone()).unwrap();
    let mut spec = owner_spec(id, 1);
    spec.terminals = vec![PTerminalId(1)];
    builder.add_owner(spec).unwrap();

    let library = builder.build().unwrap();
    assert_eq!(library.scope(), 1..=3);
    assert_eq!(library.coefficient_context().parameter_names(), ["d"]);
    assert_eq!(library.top_presentation("P3L_NO"), Some(PresentationId(4)));
    assert_eq!(library.top_presentation("P4L_NO1"), None);
    assert_eq!(library.presentation(PresentationId(4)).slots.len(), 9);
    assert_eq!(library.presentations().len(), 5);
    assert_eq!(library.owners().len(), 1);
    assert_eq!(
        library.owner(id).unwrap().spec.presentation,
        PresentationId(1)
    );
    assert_eq!(library.terminals(), [PTerminal::Unit, residual]);
    assert_eq!(*library.census(), census);
    assert!(library.topology().is_none() && library.basis_map().is_none());
    assert!(library.edge_maps().is_empty());

    let versions: Vec<String> = p_versions()
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    assert_eq!(
        library.identity(),
        format!("p-library:scope=1-3:{}", versions.join(","))
    );
    let names: Vec<_> = p_versions().into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        [
            "P_OWNER_LIBRARY_SEMANTICS",
            "P_CLASS_KEY_VERSION",
            "P_CENSUS_VERSION",
            "P_PRESENTATION_VERSION",
            "P_FAMILIES_VERSION",
            "P_GAMMA_TAG_VERSION",
            "P_GAMMA_TAG_MAX_M",
            "P_GAMMA_TAG_WIDTH",
            "P_INSERTION_KERNEL_VERSION",
            "P_CARPET_KERNEL_VERSION",
            "P_TRIANGLE_TEMPLATE_VERSION",
            "P_CUSTOM_OWNER_VERSION",
            "P_LIBRARY_SCHEMA",
            "P_FAMILIES_SCHEMA",
            "P_BASIS_LAYER_SCHEMA",
            "P_TOPOLOGY_VERSION",
        ]
    );
}

/// `a.b` for a symmetric matrix of the scalar products of `(k_1, .., k_L, q)`.
fn evaluate(gram: &[Vec<i64>], a: &[i64], b: &[i64]) -> i64 {
    let mut value = 0;
    for (i, &x) in a.iter().enumerate() {
        for (j, &y) in b.iter().enumerate() {
            value += x * y * gram[i][j];
        }
    }
    value
}

fn evaluate_form(
    form: &AffineSlotForm,
    presentation: &Presentation,
    gram: &[Vec<i64>],
) -> Rational {
    let mut value = form.constant.clone();
    for (slot, coefficient) in &form.terms {
        let denominator = match &presentation.slots[usize::from(*slot)] {
            Slot::Line(line) => evaluate(gram, &line.momentum, &line.momentum),
            Slot::Isp(isp) => evaluate(gram, &isp.left, &isp.right),
        };
        value += coefficient * &Rational::from(denominator);
    }
    value
}

/// The form of every pair of basis momenta and of random momenta holds at
/// random scalar products with `q.q = -1`.
fn assert_exact_scalar_products(presentation: &Presentation) {
    let mut rng = StdRng::seed_from_u64(0xd07);
    let width = usize::from(presentation.loops) + 1;
    let mut pairs = Vec::new();
    for i in 0..width {
        for j in i..width {
            let (mut a, mut b) = (vec![0; width], vec![0; width]);
            (a[i], b[j]) = (1, 1);
            pairs.push((a, b));
        }
    }
    for _ in 0..5 {
        let mut row = || -> Vec<i64> { (0..width).map(|_| rng.random_range(-3..=3)).collect() };
        pairs.push((row(), row()));
    }
    let forms: Vec<_> = pairs
        .iter()
        .map(|(a, b)| presentation.express_scalar_product(a, b).unwrap())
        .collect();
    for _ in 0..30 {
        let upper: Vec<Vec<i64>> = (0..width)
            .map(|_| (0..width).map(|_| rng.random_range(-20..=20)).collect())
            .collect();
        let mut gram: Vec<Vec<i64>> = (0..width)
            .map(|i| (0..width).map(|j| upper[i.min(j)][i.max(j)]).collect())
            .collect();
        gram[width - 1][width - 1] = -1;
        for ((a, b), form) in pairs.iter().zip(&forms) {
            assert_eq!(
                evaluate_form(form, presentation, &gram),
                Rational::from(evaluate(&gram, a, b))
            );
        }
    }
}

#[test]
fn scalar_products_are_exact_on_the_slots() {
    let bubble = bubble();
    assert_exact_scalar_products(&bubble);
    assert_exact_scalar_products(&kite());
    // Unit-row ISPs, left != right, as in every node presentation.
    assert_exact_scalar_products(&carpet());
    assert_exact_scalar_products(&nested_bubble());
    let (_, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    assert_exact_scalar_products(&tops[4]);

    // (k - q)^2 = k^2 - 2 k.q + q.q, with q.q = -1.
    let half = |n: i64| Rational::from((n, 2));
    let form = bubble.express_scalar_product(&[1, 0], &[0, 1]).unwrap();
    assert_eq!(
        form,
        AffineSlotForm {
            constant: half(-1),
            terms: vec![(0, half(1)), (1, half(-1))],
        }
    );
    let form = bubble.express_scalar_product(&[0, 1], &[0, 1]).unwrap();
    assert_eq!(form.constant, Rational::from(-1));
    assert!(form.terms.is_empty());

    assert_contract(bubble.express_scalar_product(&[1], &[1, 0]));
    assert_contract(bubble.express_scalar_product(&[1, 1 << 40], &[1, 0]));
}

#[test]
fn forced_axes_are_the_eps_lines_and_slots_match_the_family() {
    assert_eq!(kite().forced_axes(), 0);
    let eps_kite = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[0, 1, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, -1]),
            eps_line(&[1, -1, 0], 2),
        ],
    );
    assert_eq!(eps_kite.forced_axes(), 1 << 4);
    let context = eps_kite.family.coefficient_context();
    let four_minus_d = context
        .try_sub(
            &context.integer(4),
            &context.parameter("d").unwrap(),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    assert_eq!(eps_kite.family.power_shifts()[4], four_minus_d);
    eps_kite.validate().unwrap();

    let mut lied = eps_kite.clone();
    lied.slots[4] = line(&[1, -1, 0]);
    assert_contract(lied.validate());
    let mut lied = eps_kite.clone();
    lied.slots[0] = line(&[1, 1, 0]);
    assert_contract(lied.validate());
    let mut unordered = bubble();
    unordered.slots[0] = isp(&[1, 0], &[1, 0]);
    assert_contract(unordered.validate());

    // The right denominators in a fixed dimension are another family.
    let good = bubble();
    let family = IntegralFamily::new(
        "four_dimensional",
        vec!["k1".to_owned()],
        vec!["q".to_owned()],
        good.family.coefficient_context().clone(),
        good.family.coefficient_context().integer(4),
        good.family.denominators().to_vec(),
        good.family.external_gram().to_vec(),
        good.family.power_shifts().to_vec(),
    )
    .unwrap();
    let four_dimensional = Presentation {
        family: Arc::new(family),
        ..good
    };
    assert_contract(four_dimensional.validate());
    assert_contract(PLibrary::builder(context.clone(), 1..=1).add_presentation(four_dimensional));
}

#[test]
fn the_insertion_contract_accepts_a_bubble() {
    let roles = BubbleRoles {
        lines: [0, 1],
        loop_index: 0,
    };
    check_insertion_contract(&bubble(), &roles).unwrap();

    // The pure line comes first; the kite's k1 also runs through (k1 - k2).
    assert_contract(check_insertion_contract(
        &bubble(),
        &BubbleRoles {
            lines: [1, 0],
            ..roles
        },
    ));
    assert_contract(check_insertion_contract(
        &kite(),
        &BubbleRoles {
            lines: [0, 2],
            ..roles
        },
    ));

    // ISPs may be k_l.Y, not a square of a k_l-dependent momentum.
    let nested = |slot: Slot| {
        hand_built(
            0,
            2,
            vec![
                line(&[1, 0, 0]),
                line(&[1, -1, 0]),
                line(&[0, 1, 0]),
                line(&[0, 1, -1]),
                slot,
            ],
        )
    };
    check_insertion_contract(&nested(isp(&[1, 0, 0], &[0, 0, 1])), &roles).unwrap();
    assert_contract(check_insertion_contract(
        &nested(isp(&[1, 0, 1], &[1, 0, 1])),
        &roles,
    ));
}

/// Blob `l^2, (l-p)^2`, `r = (p-q)^2`, `third = p^2`, ISP `l.q`; loops `(l, p)`.
fn carpet() -> Presentation {
    hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            line(&[0, 1, 0]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    )
}

const CARPET_ROLES: CarpetRoles = CarpetRoles {
    r: 2,
    third: Some(3),
    outer_loop: 1,
};

#[test]
fn the_carpet_contract_routes_q_only_through_r() {
    check_carpet_contract(&carpet(), &CARPET_ROLES).unwrap();
    assert_contract(check_carpet_contract(
        &carpet(),
        &CarpetRoles {
            r: 3,
            third: Some(2),
            ..CARPET_ROLES
        },
    ));

    // Without third: the blob's external momentum is p; a q-free ISP may contain p.
    let without_third = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 1, 0]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    );
    let no_third = CarpetRoles {
        third: None,
        ..CARPET_ROLES
    };
    check_carpet_contract(&without_third, &no_third).unwrap();

    // Roles without third on a line +-p: a step would drop the power of that slot.
    assert_contract(check_carpet_contract(&carpet(), &no_third));
    let minus_p = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            line(&[0, -1, 0]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    );
    check_carpet_contract(&minus_p, &CARPET_ROLES).unwrap();
    assert_contract(check_carpet_contract(&minus_p, &no_third));

    // q in a blob line.
    let q_in_blob = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, -1]),
            line(&[0, 1, 0]),
            isp(&[1, 0, 0], &[0, 1, 0]),
        ],
    );
    assert_contract(check_carpet_contract(&q_in_blob, &CARPET_ROLES));

    // p.q as an ISP: it is expressed through r, third and p^2.
    let p_dot_q = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            isp(&[0, 1, 0], &[0, 0, 1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    );
    assert_contract(check_carpet_contract(&p_dot_q, &no_third));

    // (l + p).q has a p component.
    let mixed = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, -1]),
            line(&[0, 1, 0]),
            isp(&[1, 1, 0], &[0, 0, 1]),
        ],
    );
    assert_contract(check_carpet_contract(&mixed, &CARPET_ROLES));
}

/// The kite d43 in a triangle-adapted routing: `k` runs through `b, a1, a2`;
/// `eps` units on the slots `b, a1, a2, c1, c2`.
fn triangle_kite(eps: [u8; 5]) -> Presentation {
    hand_built(
        0,
        2,
        vec![
            eps_line(&[1, 0, 0], eps[0]),
            eps_line(&[1, 1, 0], eps[1]),
            eps_line(&[1, 1, -1], eps[2]),
            eps_line(&[0, 1, 0], eps[3]),
            eps_line(&[0, 1, -1], eps[4]),
        ],
    )
}

#[test]
fn the_triangle_contract_needs_k_in_three_lines() {
    let roles = TriangleRoles {
        b: 0,
        a: [1, 2],
        c: [3, 4],
        k_loop: 0,
    };
    check_triangle_contract(&triangle_kite([0; 5]), &roles).unwrap();

    // Only the apex lines a may carry eps.
    let eps_on = |slot: usize| {
        let mut eps = [0; 5];
        eps[slot] = 1;
        triangle_kite(eps)
    };
    for apex in [1, 2] {
        check_triangle_contract(&eps_on(apex), &roles).unwrap();
    }
    for integer_line in [0, 3, 4] {
        assert_contract(check_triangle_contract(&eps_on(integer_line), &roles));
    }

    assert_contract(check_triangle_contract(
        &triangle_kite([0; 5]),
        &TriangleRoles { c: [4, 3], ..roles },
    ));
    assert_contract(check_triangle_contract(
        &triangle_kite([0; 5]),
        &TriangleRoles { c: [3, 3], ..roles },
    ));
    // The plain kite routing: mom(b) = k1 - k2 is no loop momentum.
    assert_contract(check_triangle_contract(
        &kite(),
        &TriangleRoles {
            b: 4,
            a: [0, 2],
            c: [1, 3],
            k_loop: 0,
        },
    ));
}

fn unit_map(external: i64, sign: i8) -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 1,
        loops_out: 1,
        loop_linear: Box::new([1]),
        loop_external: Box::new([external]),
        external_sign: sign,
    }
}

/// The nested bubble `l^2, (l-k)^2, k^2, (k-q)^2` with ISP `l.q`, loops `(l, k)`.
fn nested_bubble() -> Presentation {
    hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, -1, 0]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    )
}

fn one_loop_child(eps_units: u8) -> Presentation {
    hand_built(1, 1, vec![eps_line(&[1, 0], eps_units), line(&[1, -1])])
}

fn insertion_edge() -> EdgeMap {
    EdgeMap {
        parent: PresentationId(0),
        child: PresentationId(1),
        kind: EdgeKind::Insertion,
        integrated_loop: 0,
        outer_map: unit_map(0, 1),
        line_slots: vec![None, None, Some(0), Some(1), None],
        merged_slot: Some(0),
    }
}

#[test]
fn edge_maps_are_verified_exactly() {
    let parent = nested_bubble();
    let child = one_loop_child(1);
    let roles = BubbleRoles {
        lines: [0, 1],
        loop_index: 0,
    };
    check_insertion_contract(&parent, &roles).unwrap();
    let edge = insertion_edge();
    verify_edge_map(&parent, &child, &edge).unwrap();

    let wrong = [
        EdgeMap {
            outer_map: unit_map(0, -1),
            ..edge.clone()
        },
        EdgeMap {
            outer_map: unit_map(1, 1),
            ..edge.clone()
        },
        EdgeMap {
            outer_map: SignedAffineLoopMap {
                loop_linear: Box::new([2]),
                ..unit_map(0, 1)
            },
            ..edge.clone()
        },
        EdgeMap {
            line_slots: vec![None, None, Some(1), Some(0), None],
            ..edge.clone()
        },
        EdgeMap {
            line_slots: vec![None, None, None, Some(1), None],
            ..edge.clone()
        },
        EdgeMap {
            line_slots: vec![None, None, Some(0), Some(1), Some(0)],
            ..edge.clone()
        },
        EdgeMap {
            line_slots: vec![None, None, Some(0), Some(1)],
            ..edge.clone()
        },
        EdgeMap {
            merged_slot: None,
            ..edge.clone()
        },
        EdgeMap {
            merged_slot: Some(1),
            ..edge.clone()
        },
        EdgeMap {
            integrated_loop: 1,
            ..edge.clone()
        },
        EdgeMap {
            child: PresentationId(9),
            ..edge.clone()
        },
    ];
    for map in wrong {
        assert_contract(verify_edge_map(&parent, &child, &map));
    }

    // Eps units add, + 1 on the merged line.
    assert_contract(verify_edge_map(&parent, &one_loop_child(0), &edge));
    assert_contract(verify_edge_map(&parent, &one_loop_child(2), &edge));
    let eps_parent = hand_built(
        0,
        2,
        vec![
            eps_line(&[1, 0, 0], 1),
            line(&[1, -1, 0]),
            eps_line(&[0, 1, 0], 2),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 0, 1]),
        ],
    );
    verify_edge_map(&eps_parent, &one_loop_child(4), &edge).unwrap();
}

#[test]
fn absorbed_and_carpet_edges_are_verified() {
    // A bubble of momentum q: the merged line is absorbed.
    let parent = hand_built(
        0,
        2,
        vec![
            line(&[1, 0, 0]),
            line(&[1, 0, -1]),
            line(&[0, 1, 0]),
            line(&[0, 1, -1]),
            isp(&[1, 0, 0], &[0, 1, 0]),
        ],
    );
    let child = one_loop_child(0);
    let absorbed = EdgeMap {
        merged_slot: None,
        ..insertion_edge()
    };
    verify_edge_map(&parent, &child, &absorbed).unwrap();
    assert_contract(verify_edge_map(&nested_bubble(), &child, &absorbed));

    // The carpet's blob, with p -> q.
    let edge = EdgeMap {
        kind: EdgeKind::Carpet,
        integrated_loop: 1,
        line_slots: vec![Some(0), Some(1), None, None, None],
        merged_slot: None,
        ..insertion_edge()
    };
    check_carpet_contract(&carpet(), &CARPET_ROLES).unwrap();
    verify_edge_map(&carpet(), &child, &edge).unwrap();
    assert_contract(verify_edge_map(
        &carpet(),
        &child,
        &EdgeMap {
            merged_slot: Some(0),
            ..edge.clone()
        },
    ));
    assert_contract(verify_edge_map(
        &carpet(),
        &child,
        &EdgeMap {
            line_slots: vec![Some(0), None, None, None, None],
            ..edge.clone()
        },
    ));
    assert_contract(verify_edge_map(
        &carpet(),
        &child,
        &EdgeMap {
            line_slots: vec![Some(0), Some(1), Some(1), None, None],
            ..edge
        },
    ));
}

#[test]
fn tampered_presentations_are_typed_errors_not_panics() {
    let tampered = |presentation: &Presentation, slot: usize, row: Vec<i64>| {
        let mut broken = presentation.clone();
        broken.slots[slot] = line(&row);
        broken
    };
    let bubble_roles = BubbleRoles {
        lines: [0, 1],
        loop_index: 0,
    };
    let triangle_roles = TriangleRoles {
        b: 0,
        a: [1, 2],
        c: [3, 4],
        k_loop: 0,
    };
    for row in [vec![1], vec![i64::MIN, 0, 0], vec![1 << 40, 0, 0]] {
        let bubble = tampered(&bubble(), 1, row.clone());
        assert_contract(check_insertion_contract(&bubble, &bubble_roles));
        let carpet = tampered(&carpet(), 1, row.clone());
        assert_contract(check_carpet_contract(&carpet, &CARPET_ROLES));
        let triangle = tampered(&triangle_kite([0; 5]), 1, row.clone());
        assert_contract(check_triangle_contract(&triangle, &triangle_roles));
        let parent = tampered(&nested_bubble(), 1, row.clone());
        assert_contract(verify_edge_map(
            &parent,
            &one_loop_child(1),
            &insertion_edge(),
        ));
        let child = tampered(&one_loop_child(1), 1, row);
        assert_contract(verify_edge_map(&nested_bubble(), &child, &insertion_edge()));
    }
}

#[test]
fn loop_maps_act_on_momentum_rows() {
    let map = SignedAffineLoopMap {
        loops_in: 2,
        loops_out: 2,
        loop_linear: Box::new([0, 1, 1, -1]),
        loop_external: Box::new([1, 0]),
        external_sign: -1,
    };
    assert!(map.is_unimodular());
    // k1 = k2' + q', k2 = k1' - k2', q = -q'.
    assert_eq!(map.map_momentum(&[1, 1, 1]), Ok(vec![1, 0, 0]));
    assert_contract(map.map_momentum(&[1, 1]));
    let map = SignedAffineLoopMap {
        loop_linear: Box::new([1, 1, 1, 1]),
        ..map
    };
    assert!(!map.is_unimodular());
    assert!(!unit_map(0, 2).is_unimodular());
    assert!(!unit_map(0, i8::MIN).is_unimodular());
    assert_contract(
        unit_map(0, 1)
            .map_momentum(&[i64::MAX, 0])
            .and_then(|row| unit_map(i64::MAX, 1).map_momentum(&row)),
    );
}

/// A change of the file needs a bump of `P_FAMILIES_VERSION` and a new pin here.
#[test]
fn the_families_file_is_pinned_to_its_version() {
    assert_eq!(
        (
            P_FAMILIES_VERSION,
            blake3::hash(P_FAMILIES_V1_JSON.as_bytes())
                .to_hex()
                .as_str()
        ),
        (
            1,
            "654b53417c3a0b1dc2a5731e52105ec7d2daa27591b897ee1a2628a50a9d4250"
        )
    );
}

#[test]
fn the_families_are_vakints() {
    let on_disk = concat!(env!("CARGO_MANIFEST_DIR"), "/data/p/p-families.v1.json");
    assert_eq!(
        std::fs::read_to_string(on_disk).unwrap(),
        P_FAMILIES_V1_JSON
    );

    let (file, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    assert_eq!(file.schema, P_FAMILIES_SCHEMA);
    let printed = serde_json::to_string(&file).unwrap();
    assert_eq!(
        serde_json::from_str::<PFamiliesFile>(&printed).unwrap(),
        file
    );
    let (_, again) = load_p_families(&printed).unwrap();
    assert_eq!(again.len(), tops.len());

    let names = [
        ("P1L", 1, 0),
        ("P2L", 2, 0),
        ("P3L_LA", 3, 1),
        ("P3L_BE", 3, 1),
        ("P3L_NO", 3, 1),
        ("P4L_LA4", 4, 3),
        ("P4L_J1", 4, 3),
        ("P4L_J2", 4, 3),
        ("P4L_J4", 4, 3),
        ("P4L_J5", 4, 3),
        ("P4L_J6", 4, 3),
        ("P4L_J11", 4, 3),
        ("P4L_HAHA", 4, 3),
        ("P4L_NO1", 4, 3),
        ("P4L_NO2", 4, 3),
        ("P4L_NO3", 4, 3),
    ];
    assert_eq!(tops.len(), names.len());
    for (index, (top, (name, loops, isps))) in tops.iter().zip(names).enumerate() {
        assert_eq!(top.id, PresentationId(index as u32));
        assert_eq!(top.loops, loops);
        assert_eq!(top.family.name(), name);
        assert_eq!(
            top.kind,
            PresentationKind::Top {
                name: name.to_owned()
            }
        );
        let lines = 3 * usize::from(loops) - 1;
        assert_eq!(top.slots.len(), lines + isps);
        assert_eq!(top.family.denominator_count(), lines + isps);
        assert!(
            top.slots[..lines]
                .iter()
                .all(|s| matches!(s, Slot::Line(_)))
        );
        assert!(top.slots[lines..].iter().all(|s| matches!(s, Slot::Isp(_))));
        assert_eq!(top.forced_axes(), 0);
        assert!(top.node.is_none());
        let same_context = top.family.coefficient_context();
        assert!(same_context.has_same_variable_map(tops[0].family.coefficient_context()));
    }

    // Slot order at spot checks of vakint's registry.
    let Slot::Line(second) = &tops[1].slots[3] else {
        panic!("slot 3 of P2L is a line");
    };
    assert_eq!(second.momentum, [1, 0, 1]);
    assert_eq!(tops[4].slots[8], isp(&[1, -1, 0, 0], &[1, -1, 0, 0]));
    assert_eq!(
        tops[15].slots[13],
        isp(&[0, 0, 1, 0, -1], &[0, 0, 1, 0, -1])
    );
}

/// vakint's compact form of a family: the lines, then the squares of the ISPs.
fn compact_source(top: &Presentation) -> String {
    let loops = usize::from(top.loops);
    let names: Vec<String> = (1..=loops).map(|l| format!("k{l}")).collect();
    let mut props = String::new();
    for (slot, content) in top.slots.iter().enumerate() {
        let row = match content {
            Slot::Line(line) => &line.momentum,
            Slot::Isp(isp) => &isp.left,
        };
        let momentum: String = row
            .iter()
            .enumerate()
            .filter(|(_, c)| **c != 0)
            .map(|(column, c)| {
                let name = names.get(column).map_or("q", String::as_str);
                format!("{c:+}*{name}")
            })
            .collect();
        props += &format!("  prop(D{},({momentum})^2,1),\n", slot + 1);
    }
    format!(
        "I(\n  name({}),\n  loops({}),\n  externals(q),\n  dimension(d),\n{props}  gram(q,q,-1)\n)",
        top.family.name(),
        names.join(",")
    )
}

#[test]
fn top_families_equal_their_compact_input() {
    let (_, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    for top in tops {
        let compiled = compile(&compact_source(&top));
        assert_eq!(compiled.fingerprint(), top.family.fingerprint());
    }
}

#[test]
fn malformed_family_files_fail_closed() {
    let edited = |from: &str, to: &str| {
        assert!(P_FAMILIES_V1_JSON.contains(from));
        P_FAMILIES_V1_JSON.replacen(from, to, 1)
    };
    assert!(matches!(
        load_p_families(&edited("rustred.p-families.v1", "rustred.p-families.v2")),
        Err(PError::VersionMismatch { .. })
    ));
    assert_malformed(load_p_families("{"));
    assert_malformed(load_p_families(&edited("\"gram\"", "\"Gram\"")));
    // Momentum conservation, kinematics, external vertices, names.
    assert_malformed(load_p_families(&edited(
        "\"endpoints\": [2, 1], \"momentum\": [1, -1]",
        "\"endpoints\": [2, 1], \"momentum\": [1, 1]",
    )));
    assert_malformed(load_p_families(&edited(
        "\"gram\": [[-1]]",
        "\"gram\": [[1]]",
    )));
    assert_malformed(load_p_families(&edited(
        "\"external\": {\"in\": 1, \"out\": 2}",
        "\"external\": {\"in\": 1, \"out\": 1}",
    )));
    assert_malformed(load_p_families(&edited(
        "\"name\": \"P2L\"",
        "\"name\": \"P1L\"",
    )));
}

struct FakeLane;

impl PLaneStep for FakeLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Insertion
    }

    fn step(&self, _cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        Err(PError::Uncovered {
            class: OwnerClassId(u64::from(key.presentation.0)),
            key: IntegralKey::try_new(key.powers.to_vec()).unwrap(),
        })
    }
}

struct FakeBasis;

impl PBasisMap for FakeBasis {
    fn map(&self, _terminal: PTerminalId, _tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        Ok(Vec::new())
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn the_frozen_signatures_hold() {
    let _: fn(&Presentation, u64, &ClassLimits) -> Result<Option<ClassedSector>, PError> = classify;
    let _: fn(
        &ClassedSector,
        &ClassedSector,
        &ClassLimits,
    ) -> Result<Option<SignedAffineLoopMap>, PError> = route;
    let _: fn(&[Arc<Presentation>], Option<&Dag>, &PCensusOptions) -> Result<PCensus, PError> =
        p_census;
    let _: fn(&PCensus, &[Arc<Presentation>], Option<&Dag>) -> Result<(), PError> =
        PCensus::check_complete;
    let _: fn(
        &NodeInfo,
        &LaneDecision,
        &[Arc<Presentation>],
        PresentationId,
        &CoefficientContext,
    ) -> Result<Presentation, PError> = present;
    let _: fn(&Presentation, &Presentation, &DagEdge) -> Result<EdgeMap, PError> = edge_map;
    let _: fn(&PLibrary, &POwnerSpec) -> Result<OwnerProgram, PError> = lane_program;
    let _: fn(&PLibrary, PReductionRequest<'_>) -> Result<PReduction, PError> = PLibrary::reduce;
    let _: fn(&mut PLibraryBuilder, Arc<Dag>) -> Result<(), PError> = PLibraryBuilder::set_topology;
    let _: fn(&mut PLibraryBuilder, POwnerSpec, OwnerProgram) -> Result<(), PError> =
        PLibraryBuilder::add_owner_with_program;

    let program = OwnerProgram::Lane(Arc::new(FakeLane));
    assert_eq!(format!("{program:?}"), "Lane(Insertion)");
    let mut builder = PLibrary::builder(context(), 1..=1);
    builder.set_basis_map(Arc::new(FakeBasis)).unwrap();
    assert_malformed(builder.set_basis_map(Arc::new(FakeBasis)));
}

#[test]
fn seeded_programs_never_reach_the_factory() {
    let lane_record = class_record(b"seeded lane", 0);
    let missing_record = class_record(b"seeded missing", 0);
    let (lane, missing) = (lane_record.key.id(), missing_record.key.id());
    let mut census = PCensus::default();
    census.insert_class(lane_record).unwrap();
    census.insert_class(missing_record).unwrap();

    let step: Arc<dyn PLaneStep> = Arc::new(FakeLane);
    let mut builder = PLibrary::builder(context(), 1..=1);
    builder.add_presentation(bubble()).unwrap();
    builder
        .add_owner_with_program(owner_spec(lane, 0), OwnerProgram::Lane(step.clone()))
        .unwrap();
    builder
        .add_owner_with_program(owner_spec(missing, 0), OwnerProgram::Missing { ledger: 7 })
        .unwrap();
    assert_malformed(builder.add_owner(owner_spec(lane, 0)));
    assert_malformed(
        builder.add_owner_with_program(owner_spec(missing, 0), OwnerProgram::Missing { ledger: 8 }),
    );
    builder.set_census(census).unwrap();
    let library = builder.build().unwrap();

    let owner = library.owner(lane).unwrap();
    let Ok(OwnerProgram::Lane(seeded)) = owner.program(&library) else {
        panic!("the seeded lane is gone");
    };
    assert!(Arc::ptr_eq(seeded, &step));
    assert!(matches!(
        library.owner(missing).unwrap().program(&library),
        Ok(OwnerProgram::Missing { ledger: 7 })
    ));
}

/// A one-class library on `presentation`, `edit`ed before it is built.
fn built_with(
    presentation: Presentation,
    edit: impl FnOnce(&mut PLibraryBuilder, &mut PCensus, &mut POwnerSpec) -> Result<(), PError>,
) -> Result<PLibrary, PError> {
    let mut census = PCensus::default();
    let id = census.insert_class(class_record(b"bubble", 0))?;
    let mut owner = owner_spec(id, 0);
    let mut builder = PLibrary::builder(context(), 1..=2);
    builder.add_presentation(presentation)?;
    edit(&mut builder, &mut census, &mut owner)?;
    builder.add_owner(owner)?;
    builder.set_census(census)?;
    builder.build()
}

fn bubble_topology() -> PTopo {
    PTopo::new(2, 0, 1, [(0, 1, 0), (0, 1, 0)])
}

fn bubble_dag() -> Arc<Dag> {
    Arc::new(reduction_graph(&[bubble_topology()], ChildMode::ForcerTree))
}

#[test]
fn the_scope_needs_loops() {
    for scope in [RangeInclusive::new(3, 1), 0..=1] {
        let mut builder = PLibrary::builder(context(), scope);
        builder.set_census(PCensus::default()).unwrap();
        assert_malformed(builder.build());
    }
}

#[test]
fn every_node_reference_needs_the_topology() {
    built_with(bubble(), |_, _, _| Ok(())).unwrap();

    // Owners and class records name existing nodes of a set topology.
    let in_topology = |node: NodeIndex| {
        move |builder: &mut PLibraryBuilder, census: &mut PCensus, owner: &mut POwnerSpec| {
            builder.set_topology(bubble_dag())?;
            owner.node = Some(node);
            census.classes.get_mut(&owner.class).unwrap().node = owner.node;
            Ok(())
        }
    };
    built_with(bubble(), in_topology(NodeIndex(0))).unwrap();
    assert_malformed(built_with(bubble(), in_topology(NodeIndex(99))));
    assert_malformed(built_with(bubble(), |_, census, owner| {
        owner.node = Some(NodeIndex(0));
        census.classes.get_mut(&owner.class).unwrap().node = owner.node;
        Ok(())
    }));
    for nodes in [(Some(NodeIndex(99)), vec![]), (None, vec![NodeIndex(99)])] {
        assert_malformed(built_with(bubble(), |builder, census, owner| {
            builder.set_topology(bubble_dag())?;
            let class = census.classes.get_mut(&owner.class).unwrap();
            (class.node, class.nodes) = nodes;
            Ok(())
        }));
    }

    // A presentation names its node with the node's graph key.
    let at_node = |index: u32, graph_key: Arc<[u8]>| {
        let mut presentation = bubble();
        presentation.node = Some(NodeRef {
            index: NodeIndex(index),
            graph_key,
        });
        presentation
    };
    let key = canonical(&bubble_topology()).0.0;
    let with_dag = |builder: &mut PLibraryBuilder, _: &mut PCensus, _: &mut POwnerSpec| {
        builder.set_topology(bubble_dag())
    };
    assert_eq!(bubble_dag().nodes[0].key.0, key);
    built_with(at_node(0, key.clone()), with_dag).unwrap();
    assert_malformed(built_with(at_node(1 << 20, key.clone()), with_dag));
    assert_malformed(built_with(at_node(0, Arc::from(&[1_u8][..])), with_dag));
    assert_malformed(built_with(at_node(0, key), |_, _, _| Ok(())));
}

#[test]
fn every_induced_presentation_needs_its_top() {
    let induced = |top: u32, mask: u64| {
        let mut presentation = hand_built(1, 1, vec![line(&[1, 0]), line(&[1, -1])]);
        presentation.kind = PresentationKind::Induced {
            top: PresentationId(top),
            mask,
        };
        presentation
    };
    let library = |top_kind: PresentationKind, induced: Presentation| {
        let mut builder = PLibrary::builder(context(), 1..=1);
        let mut top = bubble();
        top.kind = top_kind;
        builder.add_presentation(top)?;
        builder.add_presentation(induced)?;
        builder.set_census(PCensus::default())?;
        builder.build()
    };
    let top = || PresentationKind::Top {
        name: "P1L".to_owned(),
    };
    library(top(), induced(0, 0b11)).unwrap();
    assert_malformed(library(top(), induced(99, 0b11)));
    assert_malformed(library(top(), induced(0, 0b100)));
    assert_malformed(library(PresentationKind::EpsNode, induced(0, 0b11)));
}

#[test]
fn supports_contain_the_forced_axes_and_lie_on_the_slots() {
    let eps_bubble = || hand_built(0, 1, vec![line(&[1, 0]), eps_line(&[1, -1], 1)]);
    let with_sector = |support: u64| {
        built_with(eps_bubble(), move |_, census, owner| {
            census.classes.get_mut(&owner.class).unwrap().eps_lines = 1;
            let fate = route_to(owner.class);
            census.sectors.insert((PresentationId(0), support), fate);
            Ok(())
        })
    };
    with_sector(0b11).unwrap();
    with_sector(0b10).unwrap();
    assert_malformed(with_sector(0b01));
    assert_malformed(with_sector(0b110));

    assert_malformed(built_with(eps_bubble(), |_, census, owner| {
        census.classes.get_mut(&owner.class).unwrap().owner = (PresentationId(0), 0b01);
        Ok(())
    }));
    assert_malformed(built_with(bubble(), |_, census, owner| {
        census.classes.get_mut(&owner.class).unwrap().owner.1 = 0b100;
        Ok(())
    }));
    assert_malformed(built_with(bubble(), |_, census, _| {
        let unknown = (PresentationId(9), 0b11);
        census.sectors.insert(unknown, PSectorFate::Zero);
        Ok(())
    }));
}

#[test]
fn routes_keep_the_loop_counts_of_their_presentations() {
    let route = |loops_in: u8, loops_out: u8| {
        let columns = usize::from(loops_in) * usize::from(loops_out);
        SignedAffineLoopMap {
            loops_in,
            loops_out,
            loop_linear: vec![1; columns].into_boxed_slice(),
            loop_external: vec![0; usize::from(loops_in)].into_boxed_slice(),
            external_sign: 1,
        }
    };
    let with_route = |map: SignedAffineLoopMap| {
        built_with(bubble(), move |_, census, owner| {
            let fate = PSectorFate::Owner {
                class: owner.class,
                map,
            };
            census.sectors.insert((PresentationId(0), 0b11), fate);
            Ok(())
        })
    };
    with_route(route(1, 1)).unwrap();
    assert_malformed(with_route(route(2, 1)));
    assert_malformed(with_route(route(1, 2)));

    // A route is a change of basis: det +-1 and q -> +-q, on complete rows.
    let square = || route(1, 1);
    with_route(SignedAffineLoopMap {
        loop_linear: Box::new([-1]),
        external_sign: -1,
        ..square()
    })
    .unwrap();
    for map in [
        SignedAffineLoopMap {
            loop_linear: Box::new([2]),
            ..square()
        },
        SignedAffineLoopMap {
            loop_linear: Box::new([0]),
            ..square()
        },
        SignedAffineLoopMap {
            external_sign: 5,
            ..square()
        },
        SignedAffineLoopMap {
            external_sign: 0,
            ..square()
        },
        SignedAffineLoopMap {
            loop_linear: Box::new([]),
            ..square()
        },
        SignedAffineLoopMap {
            loop_external: Box::new([]),
            ..square()
        },
    ] {
        assert_malformed(with_route(map));
    }
}

#[test]
fn class_records_agree_with_their_owner() {
    // t and eps_lines count the owner sector: lines, eps lines among them.
    let counted = |support: u64, t: u8, eps_lines: u8| {
        let eps_bubble = hand_built(0, 1, vec![line(&[1, 0]), eps_line(&[1, -1], 1)]);
        built_with(eps_bubble, move |_, census, owner| {
            let class = census.classes.get_mut(&owner.class).unwrap();
            class.owner.1 = support;
            (class.t, class.eps_lines) = (t, eps_lines);
            Ok(())
        })
    };
    counted(0b11, 2, 1).unwrap();
    counted(0b10, 1, 1).unwrap();
    assert_malformed(counted(0b11, 1, 1));
    assert_malformed(counted(0b11, 3, 1));
    assert_malformed(counted(0b11, 2, 0));
    assert_malformed(counted(0b11, 2, 2));

    // The class and its owner record name one node.
    let at_nodes = |class_node: Option<u32>, owner_node: Option<u32>| {
        built_with(bubble(), move |builder, census, owner| {
            builder.set_topology(bubble_dag())?;
            census.classes.get_mut(&owner.class).unwrap().node = class_node.map(NodeIndex);
            owner.node = owner_node.map(NodeIndex);
            Ok(())
        })
    };
    assert_eq!(bubble_dag().nodes.len(), 2);
    at_nodes(None, None).unwrap();
    at_nodes(Some(1), Some(1)).unwrap();
    assert_malformed(at_nodes(Some(0), None));
    assert_malformed(at_nodes(None, Some(0)));
    assert_malformed(at_nodes(Some(0), Some(1)));

    // A second known class, on the first line alone.
    let second_class = || PClassRecord {
        owner: (PresentationId(0), 0b01),
        t: 1,
        ..class_record(b"second", 0)
    };

    // The owner sector stays alive and in its class; other sectors may vanish.
    let with_fate = |support: u64, fate: fn(OwnerClassId) -> PSectorFate| {
        built_with(bubble(), move |_, census, owner| {
            let fate = fate(owner.class);
            census.sectors.insert((PresentationId(0), support), fate);
            Ok(())
        })
    };
    with_fate(0b11, route_to).unwrap();
    with_fate(0b01, |_| PSectorFate::Zero).unwrap();
    assert_malformed(with_fate(0b11, |_| PSectorFate::Zero));
    assert_malformed(built_with(bubble(), |builder, census, _| {
        let second = census.insert_class(second_class())?;
        builder.add_owner(owner_spec(second, 0))?;
        census
            .sectors
            .insert((PresentationId(0), 0b11), route_to(second));
        Ok(())
    }));

    // An owner lists the unit terminal and its own residuals, no other class's.
    let listing_residual_of = |other: bool| {
        built_with(bubble(), move |builder, census, owner| {
            let second = census.insert_class(second_class())?;
            builder.add_owner(owner_spec(second, 0))?;
            let residual = builder.add_terminal(PTerminal::Residual {
                owner: if other { second } else { owner.class },
                key: IntegralKey::try_new([1, 1]).unwrap(),
            })?;
            owner.terminals = vec![PTerminalId(0), residual];
            Ok(())
        })
    };
    listing_residual_of(false).unwrap();
    assert_malformed(listing_residual_of(true));
}

#[test]
fn owner_edge_maps_leave_the_owner_presentation() {
    let library = |owner_presentation: u32| {
        let mut census = PCensus::default();
        let id = census.insert_class(PClassRecord {
            eps_lines: u8::from(owner_presentation == 1),
            ..class_record(b"node", owner_presentation)
        })?;
        let mut builder = PLibrary::builder(context(), 1..=2);
        builder.add_presentation(nested_bubble())?;
        builder.add_presentation(one_loop_child(1))?;
        let mut owner = owner_spec(id, owner_presentation);
        owner.edge_maps = vec![builder.add_edge_map(insertion_edge())?];
        builder.add_owner(owner)?;
        builder.set_census(census)?;
        builder.build()
    };
    library(0).unwrap();
    assert_malformed(library(1));
}

#[test]
fn residual_terminals_name_a_class_and_its_arity() {
    let with_residual = |unknown: bool, slots: usize| {
        built_with(bubble(), move |builder, _, owner| {
            let class = OwnerClassId(owner.class.0 ^ u64::from(unknown));
            builder.add_terminal(PTerminal::Residual {
                owner: class,
                key: IntegralKey::try_new(vec![1; slots]).unwrap(),
            })?;
            Ok(())
        })
    };
    with_residual(false, 2).unwrap();
    assert_malformed(with_residual(true, 2));
    assert_malformed(with_residual(false, 3));
}
