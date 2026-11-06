//! The p library on the Ordered engine: closed forms, routes, eps supports, step
//! cache, typed errors, keys of mixed length (owner: M2-T6 rr-plib).
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensus, PClassRecord, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::OwnerClassKey;
use rustred::propagator::error::PError;
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    OwnerProgram, PBasis, PBasisMap, PLibrary, POwnerSpec, PReduction, PReductionRequest, PTerm,
    PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotIsp, SlotLine};
use rustred::propagator::step::{PChild, PKeyView, PLaneStep, PStage, PStepContext, PStepOut};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::terminal::PTerminalCodec;
use rustred::propagator::testing::{
    REFERENCE_FLIPPED, REFERENCE_SHIFTED, REFERENCE_SWAPPED, REFERENCE_TOP, bubble_tag,
    one_loop_reference_library,
};
use rustred::propagator::{OwnerClassId, P_GAMMA_TAG_WIDTH, PTerminalId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedKeyStage, OrderedSchedulerOptions, OrderedStepCache,
    OrderedStepCacheAdapter, OrderedStepCacheBinding, OrderedWeightArithmetic, PolynomialOptions,
    ReductionError, ReductionLimits, SampledOptions,
};
use rustred::solver::{CandidateOrderedPhaseKind, UnresolvedPolicy};

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn exact() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn request<'a>(inputs: Vec<(PresentationId, IntegralKey, Coefficient)>) -> PReductionRequest<'a> {
    PReductionRequest {
        inputs,
        arithmetic: OrderedWeightArithmetic::Exact,
        scheduler: OrderedSchedulerOptions::default(),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: false,
        limits: ReductionLimits::default(),
        inspector: None,
    }
}

fn modular_polynomial(threads: usize) -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularPolynomial(
        ModularCommon {
            threads,
            ..Default::default()
        },
        PolynomialOptions::default(),
    )
}

fn modular_sampled(threads: usize) -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularSampled(
        ModularCommon {
            threads,
            ..Default::default()
        },
        SampledOptions::default(),
    )
}

fn sixteen_threads() -> OrderedSchedulerOptions {
    OrderedSchedulerOptions {
        threads: 16,
        window_keys: 8,
        ..Default::default()
    }
}

/// Terms, conditions and the input-independent counters: what must be
/// bit-identical between runs.
fn same_result(left: &PReduction, right: &PReduction) {
    assert_eq!(left.terms, right.terms);
    assert_eq!(left.nonzero_conditions, right.nonzero_conditions);
    assert_eq!(left.dimension, right.dimension);
}

/// The sum of `coefficient / denominator`, an exact rational number.
fn fraction(context: &CoefficientContext, numerator: i64, denominator: i64) -> Coefficient {
    context
        .try_div(
            &context.integer(numerator),
            &context.integer(denominator),
            exact(),
        )
        .unwrap()
}

fn add(context: &CoefficientContext, a: &Coefficient, b: &Coefficient) -> Coefficient {
    context.try_add(a, b, exact()).unwrap()
}

fn mul(context: &CoefficientContext, a: &Coefficient, b: &Coefficient) -> Coefficient {
    context.try_mul(a, b, exact()).unwrap()
}

fn div(context: &CoefficientContext, a: &Coefficient, b: &Coefficient) -> Coefficient {
    context.try_div(a, b, exact()).unwrap()
}

/// `c + d`.
fn d_plus(context: &CoefficientContext, c: i64) -> Coefficient {
    add(
        context,
        &context.parameter("d").unwrap(),
        &context.integer(c),
    )
}

/// `(1 + m eps)_c`, `eps = (4 - d) / 2`, for an integer `c` of either sign:
/// `Gamma(1 + m eps + c) / Gamma(1 + m eps)`.
fn rising(context: &CoefficientContext, m: i64, c: i64) -> Coefficient {
    let eps = div(
        context,
        &context
            .try_sub(
                &context.integer(4),
                &context.parameter("d").unwrap(),
                exact(),
            )
            .unwrap(),
        &context.integer(2),
    );
    let base = add(
        context,
        &context.one(),
        &mul(context, &context.integer(m), &eps),
    );
    let mut product = context.one();
    for shift in 0..c {
        product = mul(
            context,
            &product,
            &add(context, &base, &context.integer(shift)),
        );
    }
    for shift in c..0 {
        product = div(
            context,
            &product,
            &add(context, &base, &context.integer(shift)),
        );
    }
    product
}

fn binomial(n: i64, k: i64) -> i64 {
    (1..=k).fold(1, |value, j| value * (n + 1 - j) / j)
}

/// `(k.q)_E^r / (k^2)^a ((k-q)^2)^b` integrated with `d^dk / pi^(d/2)` at
/// `q^2 = 1`, without the tag of `G`. From Feynman parameters, independently of
/// the `G` of the fake lane:
/// `1 / (Gamma(a) Gamma(b) Gamma(d-a-b+r)) Sum_m C(r,2m) (2m-1)!! 2^-m
///  Gamma(a+b-d/2-m) Gamma(d/2-a+r-m) Gamma(d/2-b+m)`.
fn euclidean_tensor_bubble(context: &CoefficientContext, a: i64, b: i64, r: i64) -> Coefficient {
    if a < 1 || b < 1 {
        return context.zero();
    }
    let factorial = |n: i64| (2..n).product::<i64>();
    let mut sum = context.zero();
    for m in 0..=r / 2 {
        let double_factorial = (1..=m).fold(1, |value, j| value * (2 * j - 1));
        let weight = fraction(
            context,
            binomial(r, 2 * m) * double_factorial,
            (1 << m) * factorial(a) * factorial(b),
        );
        let gammas = mul(
            context,
            &rising(context, 1, a + b - 3 - m),
            &mul(
                context,
                &rising(context, -1, 1 - a + r - m),
                &rising(context, -1, 1 - b + m),
            ),
        );
        sum = add(context, &sum, &mul(context, &weight, &gammas));
    }
    div(context, &sum, &rising(context, -2, 3 - a - b + r))
}

/// A routing of the bubble: the slot of `k^2`, the slot of `(k - s q)^2`, `s`.
const ROUTINGS: [(PresentationId, usize, usize, i64); 4] = [
    (REFERENCE_TOP, 0, 1, 1),
    (REFERENCE_SWAPPED, 1, 0, 1),
    (REFERENCE_FLIPPED, 0, 1, -1),
    (REFERENCE_SHIFTED, 1, 0, -1),
];

/// `(k.q)_M^r / D_k^a D_s^b` over the slots: with `q^2 = -1`,
/// `k.q = s (D_k - D_s - 1) / 2`.
fn lowered(
    context: &CoefficientContext,
    (presentation, k_slot, s_slot, sign): (PresentationId, usize, usize, i64),
    a: i64,
    b: i64,
    r: i64,
) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let mut inputs = Vec::new();
    for i in 0..=r {
        for j in 0..=r - i {
            let l = r - i - j;
            let multinomial = binomial(r, i) * binomial(r - i, j);
            let parity = if (j + l) % 2 == 0 { 1 } else { -1 };
            let orientation = if r % 2 == 0 { 1 } else { sign };
            let coefficient = fraction(context, orientation * parity * multinomial, 1 << r);
            let mut powers = [0; 2];
            powers[k_slot] = a - i;
            powers[s_slot] = b - j;
            inputs.push((presentation, key(&powers), coefficient));
        }
    }
    inputs
}

/// The value of [`lowered`] by conventions C1, C2 (without the tag of `G`):
/// `(-1)^(a + b + r) s^r` times the Euclidean integral.
fn expected_bubble(context: &CoefficientContext, sign: i64, a: i64, b: i64, r: i64) -> Coefficient {
    let phase = if (a + b + r).rem_euclid(2) == 0 {
        1
    } else {
        -1
    };
    let orientation = if r % 2 == 0 { 1 } else { sign };
    mul(
        context,
        &context.integer(phase * orientation),
        &euclidean_tensor_bubble(context, a, b, r),
    )
}

#[test]
fn the_bubble_is_one_over_eps() {
    let library = one_loop_reference_library().unwrap();
    let context = library.coefficient_context();
    let mut one_loop = request(vec![(REFERENCE_TOP, key(&[1, 1]), context.one())]);
    one_loop.onto_masters = true;
    let reduced = library.reduce(one_loop).unwrap();
    let eps = div(
        context,
        &context.integer(2),
        &context
            .try_sub(
                &context.integer(4),
                &context.parameter("d").unwrap(),
                exact(),
            )
            .unwrap(),
    );
    assert_eq!(
        reduced.terms,
        [PTerm {
            basis: PBasis::Unit,
            tag: GammaTag::one(),
            coefficient: eps
        }]
    );
    assert_eq!(reduced.dimension, 1);
    assert!(reduced.nonzero_conditions.is_empty());
}

#[test]
fn an_odd_rank_bubble_changes_sign_with_q() {
    let library = one_loop_reference_library().unwrap();
    let context = library.coefficient_context();
    let run = |routing| {
        library
            .reduce(request(lowered(context, routing, 1, 1, 1)))
            .unwrap()
            .terms
    };
    let (plus, minus) = (run(ROUTINGS[0]), run(ROUTINGS[2]));
    assert_eq!(plus.len(), 1);
    assert_eq!(minus.len(), 1);
    assert_eq!(
        plus[0].coefficient,
        context.try_neg(&minus[0].coefficient, exact()).unwrap()
    );
    // `k.q` integrates to `G(1,1) (q.q) / 2` with `q.q = -1` in the Minkowski
    // family, and `I_M(1,1) = G(1,1)`.
    assert_eq!(
        plus[0].coefficient,
        mul(
            context,
            &fraction(context, -1, 2),
            &euclidean_tensor_bubble(context, 1, 1, 0)
        )
    );
}

#[test]
fn random_bubbles_reduce_to_the_closed_form() {
    let library = one_loop_reference_library().unwrap();
    let context = library.coefficient_context();
    let tag = bubble_tag().unwrap();
    let mut rng = StdRng::seed_from_u64(0x706c6962);
    let mut nonzero = 0;
    for case in 0..200 {
        let routing = ROUTINGS[case % ROUTINGS.len()];
        let (a, b, r) = (
            rng.random_range(-2..6),
            rng.random_range(-2..6),
            rng.random_range(0..=6),
        );
        let inputs = lowered(context, routing, a, b, r);
        let expected = expected_bubble(context, routing.3, a, b, r);

        let reduced = library.reduce(request(inputs.clone())).unwrap();
        if expected.is_zero() {
            assert_eq!(reduced.terms, [], "({a}, {b}), rank {r}");
        } else {
            nonzero += 1;
            assert_eq!(
                reduced.terms,
                [PTerm {
                    basis: PBasis::Unit,
                    tag: tag.clone(),
                    coefficient: expected.clone(),
                }],
                "({a}, {b}), rank {r}, {:?}",
                routing.0
            );
        }

        let mut masters = request(inputs.clone());
        masters.onto_masters = true;
        let d_minus_3 = d_plus(context, -3);
        let onto = library.reduce(masters).unwrap();
        if expected.is_zero() {
            assert_eq!(onto.terms, []);
        } else {
            assert_eq!(
                onto.terms,
                [PTerm {
                    basis: PBasis::Unit,
                    tag: GammaTag::one(),
                    coefficient: mul(context, &expected, &d_minus_3),
                }]
            );
        }

        for (arithmetic, scheduler) in [
            (modular_polynomial(1), OrderedSchedulerOptions::default()),
            (modular_polynomial(16), sixteen_threads()),
            (OrderedWeightArithmetic::Exact, sixteen_threads()),
        ] {
            let mut run = request(inputs.clone());
            run.arithmetic = arithmetic;
            run.scheduler = scheduler;
            same_result(&library.reduce(run).unwrap(), &reduced);
        }
    }
    assert!(nonzero > 40, "{nonzero} nonzero cases");
}

/// A scratch directory of this test, emptied first.
fn scratch(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("rr-plib-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

/// BLAKE3 of the record files of a saved cache: names and bytes, sorted.
fn records_digest(directory: &Path) -> String {
    let mut files: Vec<_> = std::fs::read_dir(directory.join("shards"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    let mut hasher = blake3::Hasher::new();
    for file in &files {
        hasher.update(file.file_name().unwrap().as_encoded_bytes());
        hasher.update(&std::fs::read(file).unwrap());
    }
    hasher.finalize().to_hex().to_string()
}

fn no_tail() -> OrderedSchedulerOptions {
    OrderedSchedulerOptions {
        adjoint_tail: false,
        ..Default::default()
    }
}

#[test]
fn a_warm_run_replays_every_step_with_the_same_result() {
    let library = one_loop_reference_library().unwrap();
    let context = library.coefficient_context();
    let binding = library.step_cache_binding().unwrap();
    assert_eq!(binding.adapter, OrderedStepCacheAdapter::PropagatorLibrary);
    assert_eq!(binding.family_fingerprint, library.identity());
    assert_eq!(binding.context_variables, ["d"]);
    assert_eq!(binding.unresolved_policy, Some(UnresolvedPolicy::Reject));

    let inputs: Vec<_> = ROUTINGS
        .into_iter()
        .flat_map(|routing| lowered(context, routing, 2, 3, 3))
        .collect();
    for threads in [1, 16] {
        let directory = scratch(&format!("warm-{threads}"));
        let scheduler = OrderedSchedulerOptions {
            threads,
            ..no_tail()
        };
        let cold = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
        let mut first = request(inputs.clone());
        first.scheduler = scheduler;
        first.step_cache = Some(Arc::clone(&cold));
        let first = library.reduce(first).unwrap();
        let progress = first.statistics.progress;
        assert_eq!(progress.step_cache_hits, 0);
        assert_eq!(progress.step_cache_misses, progress.processed_keys);
        assert_eq!(cold.statistics().inserts as usize, progress.processed_keys);
        let saved = cold.save().unwrap();
        assert_eq!(saved.records, progress.processed_keys);

        let warm = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
        let mut second = request(inputs.clone());
        second.scheduler = scheduler;
        second.step_cache = Some(Arc::clone(&warm));
        let second = library.reduce(second).unwrap();
        same_result(&first, &second);
        let progress = second.statistics.progress;
        assert_eq!(progress.step_cache_misses, 0, "{threads} threads");
        assert_eq!(progress.step_cache_hits, progress.processed_keys);
        assert!(progress.step_cache_hits > 0);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[test]
fn one_cache_is_refused_by_another_program() {
    let library = one_loop_reference_library().unwrap();
    let foreign = OrderedStepCacheBinding::new(
        OrderedStepCacheAdapter::CandidateProgram,
        library.identity(),
        library.coefficient_context(),
        Some(UnresolvedPolicy::Reject),
    );
    let mut run = request(Vec::new());
    run.step_cache = Some(OrderedStepCache::in_memory(foreign, Default::default()));
    assert!(matches!(
        library.reduce(run),
        Err(PError::Engine(ReductionError::OrderedStepCache { .. }))
    ));
}

/// The record bytes and the identity of the reference program: a changed record
/// needs a changed `P_*` version, and with it a new identity here.
#[test]
fn the_identity_and_the_records_of_the_reference_program_are_pinned() {
    let library = one_loop_reference_library().unwrap();
    assert_eq!(
        library.identity(),
        "p-library:scope=1-1:\
         P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,\
         P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,\
         P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,\
         P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,\
         P_TRIANGLE_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,\
         P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,\
         P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1"
    );
    let context = library.coefficient_context();
    let directory = scratch("golden");
    let cache = OrderedStepCache::open(
        &directory,
        library.step_cache_binding().unwrap(),
        Default::default(),
    )
    .unwrap();
    let mut run = request(
        ROUTINGS
            .into_iter()
            .flat_map(|routing| lowered(context, routing, 2, 2, 2))
            .collect(),
    );
    run.scheduler = no_tail();
    run.step_cache = Some(Arc::clone(&cache));
    library.reduce(run).unwrap();
    cache.save().unwrap();
    assert_eq!(
        records_digest(&directory),
        "ad2eb1282d5b1d5be45ac91f4f39f7cf14182c337540d1f955a0ffd9ecfba55f",
        "the recorded p steps changed: bump the P_* version whose meaning changed"
    );
    std::fs::remove_dir_all(&directory).unwrap();
}

fn line(momentum: &[i64]) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units: 0,
    })
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

fn presentation(
    context: &CoefficientContext,
    id: u32,
    loops: u8,
    slots: Vec<Slot>,
) -> Presentation {
    Presentation::from_slots(
        PresentationId(id),
        PresentationKind::EpsNode,
        &format!("plib_{id}"),
        loops,
        slots,
        context,
        None,
    )
    .unwrap()
}

/// A class on `support` of presentation `id`, `t` lines, `eps_lines` forced axes.
fn class_record(
    name: &str,
    loops: u8,
    id: u32,
    support: u64,
    t: u8,
    eps_lines: u8,
) -> PClassRecord {
    PClassRecord {
        key: OwnerClassKey {
            loops,
            lines: t,
            eps_lines,
            canonical: Arc::from(name.as_bytes()),
        },
        owner: (PresentationId(id), support),
        node: None,
        nodes: Vec::new(),
        t,
        eps_lines,
    }
}

fn owner_spec(class: OwnerClassId, presentation: u32, lane: OwnerLane) -> POwnerSpec {
    POwnerSpec {
        class,
        lane,
        presentation: PresentationId(presentation),
        node: None,
        roles: LaneRoles::None,
        edge_maps: Vec::new(),
        payload: None,
        custom: None,
        terminals: Vec::new(),
    }
}

/// `k1 -> q - k1` on the kite: `D_1 <-> D_3`, `D_5 -> 1 + D_1 + D_2 + D_3 + D_4 - D_5`.
fn flip_first_loop() -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 2,
        loops_out: 2,
        loop_linear: Box::new([-1, 0, 0, 1]),
        loop_external: Box::new([1, 0]),
        external_sign: 1,
    }
}

fn kite_slots() -> Vec<Slot> {
    vec![
        line(&[1, 0, 0]),
        line(&[0, 1, 0]),
        line(&[1, 0, -1]),
        line(&[0, 1, -1]),
        line(&[1, -1, 0]),
    ]
}

/// A lane whose every key is its own residual terminal.
struct ResidualLane(BTreeMap<Vec<i64>, PTerminalId>);

impl PLaneStep for ResidualLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let id = self.0.get(key.powers).ok_or_else(|| PError::Uncovered {
            class: cx.owner.spec.class,
            key: IntegralKey::try_new(key.powers.iter().copied()).unwrap(),
        })?;
        Ok(PStepOut {
            terminals: vec![(*id, key.tag.clone(), cx.library.coefficient_context().one())],
            ..PStepOut::default()
        })
    }
}

/// Every key of `box_ranges`, as vectors.
fn keys_in(box_ranges: &[std::ops::RangeInclusive<i64>]) -> Vec<Vec<i64>> {
    box_ranges.iter().fold(vec![Vec::new()], |keys, range| {
        keys.into_iter()
            .flat_map(|key| {
                range.clone().map(move |power| {
                    let mut next = key.clone();
                    next.push(power);
                    next
                })
            })
            .collect()
    })
}

/// Source kite (0), target kite (1): sector 0b01111 of the source routes to the
/// owner on the target by [`flip_first_loop`], and every other sector is a zero
/// (a fake census: the point is the exact transport and the pinch).
fn routed_kite(context: &CoefficientContext) -> (PLibrary, BTreeMap<Vec<i64>, PTerminalId>) {
    let mut builder = PLibrary::builder(context.clone(), 2..=2);
    for id in 0..2 {
        builder
            .add_presentation(presentation(context, id, 2, kite_slots()))
            .unwrap();
    }
    const OWNER: u64 = 0b01111;
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("kite", 2, 1, OWNER, 4, 0))
        .unwrap();
    for id in 0..2 {
        for support in 0..1_u64 << 5 {
            census
                .sectors
                .insert((PresentationId(id), support), PSectorFate::Zero);
        }
    }
    census.sectors.remove(&(PresentationId(1), OWNER));
    census.sectors.insert(
        (PresentationId(0), OWNER),
        PSectorFate::Owner {
            class,
            map: flip_first_loop(),
        },
    );
    let mut terminals = BTreeMap::new();
    for powers in keys_in(&[1..=4, 1..=4, 1..=4, 1..=4, -3..=0]) {
        let id = builder
            .add_terminal(PTerminal::Residual {
                owner: class,
                key: key(&powers),
            })
            .unwrap();
        terminals.insert(powers, id);
    }
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(
            owner_spec(class, 1, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(ResidualLane(terminals.clone()))),
        )
        .unwrap();
    (builder.build().unwrap(), terminals)
}

#[test]
fn a_route_transports_numerators_exactly_and_pinches_to_the_census() {
    let context = context();
    let (library, terminals) = routed_kite(&context);
    for (a, n) in [([1, 1, 1, 1], 0), ([2, 1, 3, 1], 1), ([2, 3, 2, 2], 3)] {
        let mut source = a.to_vec();
        source.push(-n);
        let reduced = library
            .reduce(request(vec![(
                PresentationId(0),
                key(&source),
                context.one(),
            )]))
            .unwrap();

        // (1 + D1 + D2 + D3 + D4 - D5)^n over the image of the denominators;
        // a line that reaches power 0 pinches onto a zero of the census.
        let mut expected: BTreeMap<PTerminalId, i64> = BTreeMap::new();
        for exponents in keys_in(&[0..=n, 0..=n, 0..=n, 0..=n, 0..=n, 0..=n]) {
            if exponents.iter().sum::<i64>() != n {
                continue;
            }
            let multinomial = exponents.iter().fold((1, n), |(value, rest), &e| {
                (value * binomial(rest, e), rest - e)
            });
            let sign = if exponents[5] % 2 == 0 { 1 } else { -1 };
            let image = [
                a[2] - exponents[1],
                a[1] - exponents[2],
                a[0] - exponents[3],
                a[3] - exponents[4],
                -exponents[5],
            ];
            if image[..4].iter().all(|&power| power > 0) {
                *expected.entry(terminals[image.as_slice()]).or_default() += sign * multinomial.0;
            }
        }
        expected.retain(|_, value| *value != 0);
        let expected: Vec<PTerm> = expected
            .into_iter()
            .map(|(id, value)| PTerm {
                basis: PBasis::Residual(id),
                tag: GammaTag::one(),
                coefficient: context.integer(value),
            })
            .collect();
        assert_eq!(reduced.terms, expected, "{source:?}");
        assert!(!reduced.terms.is_empty());
    }
}

#[test]
fn phases_descend_through_route_and_apply_with_their_stages() {
    let context = context();
    let (library, _) = routed_kite(&context);
    let events = RefCell::new(Vec::new());
    let mut inspect = |event: &rustred::reduction::OrderedPhaseEvent<'_>| {
        for entry in event.frontier() {
            events.borrow_mut().push((
                event.phase,
                event.descriptor.kind,
                event.descriptor.support,
                match entry.stage {
                    OrderedKeyStage::ClassRoute { class } => format!("route {class}"),
                    OrderedKeyStage::ClassApply { .. } => "apply".to_owned(),
                    other => format!("{other:?}"),
                },
                entry.integral.powers().len(),
            ));
        }
        Ok(())
    };
    let mut run = request(vec![(
        PresentationId(0),
        key(&[2, 1, 3, 1, -1]),
        context.one(),
    )]);
    run.inspector = Some(&mut inspect);
    library.reduce(run).unwrap();
    let events = events.into_inner();
    let slots = 5 + P_GAMMA_TAG_WIDTH;
    assert!(events.contains(&(
        2,
        CandidateOrderedPhaseKind::Routing,
        Some(4),
        "route 0".to_owned(),
        slots
    )));
    assert!(events.contains(&(
        3,
        CandidateOrderedPhaseKind::Reduction,
        Some(4),
        "apply".to_owned(),
        slots
    )));
}

/// One integer-line owner of `lane` on a bubble, on every positive key.
fn one_owner_library(lane: Arc<dyn PLaneStep>) -> (PLibrary, OwnerClassId) {
    library_with_program(OwnerProgram::Lane(lane))
}

fn library_with_program(program: OwnerProgram) -> (PLibrary, OwnerClassId) {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])],
        ))
        .unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("bubble", 1, 0, 0b11, 2, 0))
        .unwrap();
    for support in 0b00..0b11 {
        census
            .sectors
            .insert((PresentationId(0), support), PSectorFate::Zero);
    }
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(owner_spec(class, 0, OwnerLane::Custom), program)
        .unwrap();
    (builder.build().unwrap(), class)
}

fn bubble_input(library: &PLibrary) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    vec![(
        PresentationId(0),
        key(&[1, 1]),
        library.coefficient_context().one(),
    )]
}

#[test]
fn a_missing_owner_is_a_typed_error() {
    let (library, class) = library_with_program(OwnerProgram::Missing { ledger: 7 });
    assert_eq!(
        library.reduce(request(bubble_input(&library))),
        Err(PError::MissingOwner { class, ledger: 7 })
    );
    // Cancelled before its step: nothing to reduce, nothing missing.
    let context = library.coefficient_context();
    let mut inputs = bubble_input(&library);
    inputs.push((PresentationId(0), key(&[1, 1]), context.integer(-1)));
    assert_eq!(library.reduce(request(inputs)).unwrap().terms, []);
}

#[test]
fn an_uncovered_key_is_a_typed_error() {
    let (library, class) = one_owner_library(Arc::new(ResidualLane(BTreeMap::new())));
    assert_eq!(
        library.reduce(request(bubble_input(&library))),
        Err(PError::Uncovered {
            class,
            key: key(&[1, 1])
        })
    );
}

#[test]
fn a_request_without_inputs_reduces_to_nothing() {
    let library = one_loop_reference_library().unwrap();
    for onto_masters in [false, true] {
        let mut run = request(Vec::new());
        run.onto_masters = onto_masters;
        let reduced = library.reduce(run).unwrap();
        assert_eq!(reduced.dimension, 0);
        assert_eq!(reduced.terms, []);
        assert!(reduced.nonzero_conditions.is_empty());
    }
}

#[test]
fn one_request_has_one_loop_count() {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 1..=2);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])],
        ))
        .unwrap();
    builder
        .add_presentation(presentation(&context, 1, 2, kite_slots()))
        .unwrap();
    builder.set_census(PCensus::default()).unwrap();
    let library = builder.build().unwrap();
    assert_eq!(
        library.reduce(request(vec![
            (PresentationId(0), key(&[1, 1]), context.one()),
            (PresentationId(1), key(&[1, 1, 1, 1, 1]), context.one()),
        ])),
        Err(PError::MixedDimension)
    );
}

/// A lane returning a fixed output.
struct FixedLane(PStepOut);

impl PLaneStep for FixedLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, _: &PStepContext<'_>, _: &PKeyView<'_>) -> Result<PStepOut, PError> {
        Ok(self.0.clone())
    }
}

#[test]
fn a_tag_outside_the_key_range_is_a_typed_error() {
    let context = context();
    let wide = GammaTag::from_pairs([(1, 100)]).unwrap();
    let tag = wide.add(&wide);
    for output in [
        PStepOut {
            terminals: vec![(PTerminalId(0), tag.clone(), context.one())],
            ..PStepOut::default()
        },
        PStepOut {
            children: vec![PChild {
                stage: PStage::Route,
                presentation: PresentationId(0),
                powers: vec![0, 1],
                tag: tag.clone(),
                coefficient: context.one(),
            }],
            ..PStepOut::default()
        },
    ] {
        let (library, _) = one_owner_library(Arc::new(FixedLane(output)));
        assert_eq!(
            library.reduce(request(bubble_input(&library))),
            Err(PError::TagOutOfRange)
        );
    }
}

#[test]
fn one_coefficient_context_is_required_everywhere() {
    let foreign = CoefficientContext::try_new(["d", "x"]).unwrap();

    // A presentation of another context.
    let mut builder = PLibrary::builder(foreign.clone(), 1..=1);
    assert_eq!(
        builder.add_presentation(presentation(
            &context(),
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])]
        )),
        Err(PError::ContextMismatch)
    );
    // A library in another context.
    let mut builder = PLibrary::builder(foreign.clone(), 1..=1);
    builder.set_census(PCensus::default()).unwrap();
    assert!(matches!(builder.build(), Err(PError::ContextMismatch)));

    let (library, _) = one_owner_library(Arc::new(FixedLane(PStepOut::default())));
    // A foreign input coefficient, even with a weight that would cancel.
    assert_eq!(
        library.reduce(request(vec![(
            PresentationId(0),
            key(&[1, 1]),
            foreign.parameter("x").unwrap()
        )])),
        Err(PError::ContextMismatch)
    );

    // A foreign step coefficient: child, terminal or condition.
    let x = foreign.parameter("x").unwrap();
    for output in [
        PStepOut {
            terminals: vec![(PTerminalId(0), GammaTag::one(), x.clone())],
            ..PStepOut::default()
        },
        PStepOut {
            conditions: vec![x.clone()],
            ..PStepOut::default()
        },
    ] {
        let (library, _) = one_owner_library(Arc::new(FixedLane(output)));
        assert_eq!(
            library.reduce(request(bubble_input(&library))),
            Err(PError::ContextMismatch)
        );
    }
}

#[test]
fn gaps_of_the_inputs_and_of_the_census_fail_closed() {
    let context = context();
    let (library, _) = one_owner_library(Arc::new(FixedLane(PStepOut::default())));
    // Foreign presentation, wrong arity.
    assert!(matches!(
        library.reduce(request(vec![(
            PresentationId(1),
            key(&[1, 1]),
            context.one()
        )])),
        Err(PError::Unsupported(_))
    ));
    assert!(matches!(
        library.reduce(request(vec![(
            PresentationId(0),
            key(&[1, 1, 1]),
            context.one()
        )])),
        Err(PError::Unsupported(_))
    ));

    // No fate for a sector: never a silent zero, even with zero weight.
    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])],
        ))
        .unwrap();
    builder.set_census(PCensus::default()).unwrap();
    let library = builder.build().unwrap();
    assert!(matches!(
        library.reduce(request(vec![(
            PresentationId(0),
            key(&[1, 1]),
            context.zero()
        )])),
        Err(PError::MalformedLibrary(_))
    ));

    // An ISP in the denominator.
    let mut builder = PLibrary::builder(context.clone(), 1..=2);
    builder
        .add_presentation(presentation(
            &context,
            0,
            2,
            vec![
                line(&[1, 0, 0]),
                line(&[0, 1, 0]),
                line(&[1, 0, -1]),
                line(&[0, 1, -1]),
                isp(&[1, -1, 0], &[1, -1, 0]),
            ],
        ))
        .unwrap();
    builder.set_census(PCensus::default()).unwrap();
    let library = builder.build().unwrap();
    assert!(matches!(
        library.reduce(request(vec![(
            PresentationId(0),
            key(&[1, 1, 1, 1, 1]),
            context.one()
        )])),
        Err(PError::Unsupported(_))
    ));

    // Two classes own one sector.
    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])],
        ))
        .unwrap();
    let mut census = PCensus::default();
    let classes = ["one", "two"].map(|name| {
        census
            .insert_class(class_record(name, 1, 0, 0b11, 2, 0))
            .unwrap()
    });
    builder.set_census(census).unwrap();
    for class in classes {
        builder
            .add_owner_with_program(
                owner_spec(class, 0, OwnerLane::Custom),
                OwnerProgram::Lane(Arc::new(EchoLane)),
            )
            .unwrap();
    }
    let library = builder.build().unwrap();
    assert!(matches!(
        library.reduce(request(bubble_input(&library))),
        Err(PError::MalformedLibrary(_))
    ));

    // Lane output that names nothing of the library.
    for output in [
        PStepOut {
            terminals: vec![(PTerminalId(9), GammaTag::one(), context.one())],
            ..PStepOut::default()
        },
        PStepOut {
            children: vec![PChild {
                stage: PStage::Apply,
                presentation: PresentationId(0),
                powers: vec![1, 0],
                tag: GammaTag::one(),
                coefficient: context.one(),
            }],
            ..PStepOut::default()
        },
        PStepOut {
            children: vec![PChild {
                stage: PStage::Route,
                presentation: PresentationId(0),
                powers: vec![1, 0, 0],
                tag: GammaTag::one(),
                coefficient: context.one(),
            }],
            ..PStepOut::default()
        },
    ] {
        let (library, _) = one_owner_library(Arc::new(FixedLane(output)));
        assert!(matches!(
            library.reduce(request(bubble_input(&library))),
            Err(PError::MalformedLibrary(_))
        ));
    }
}

/// `I(n) -> I(n)` and a Route child at the same `t` break the descent: the
/// engine refuses both.
#[test]
fn a_step_without_descent_is_refused() {
    let context = context();
    for stage in [PStage::Apply, PStage::Route] {
        let output = PStepOut {
            children: vec![PChild {
                stage,
                presentation: PresentationId(0),
                powers: vec![1, 1],
                tag: GammaTag::one(),
                coefficient: context.one(),
            }],
            ..PStepOut::default()
        };
        let (library, _) = one_owner_library(Arc::new(FixedLane(output)));
        let refused = library.reduce(request(bubble_input(&library)));
        assert!(
            matches!(
                refused,
                Err(PError::Engine(ReductionError::ReducerInvariant { .. }))
            ),
            "{refused:?}"
        );
    }
}

/// Emits one Unit terminal whose coefficient records the key.
struct EchoLane;

impl PLaneStep for EchoLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let context = cx.library.coefficient_context();
        let value = key.powers.iter().fold(0, |value, power| 10 * value + power);
        Ok(PStepOut {
            terminals: vec![(PTerminalId(0), key.tag.clone(), context.integer(value))],
            ..PStepOut::default()
        })
    }
}

/// A bubble with an eps line on slot 0 owns its full sector, whose axes are
/// `{0, 1}`; without the integer line the sector is scaleless.
fn eps_bubble() -> PLibrary {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![eps_line(&[1, 0], 1), line(&[1, -1])],
        ))
        .unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("eps bubble", 1, 0, 0b11, 2, 1))
        .unwrap();
    census
        .sectors
        .insert((PresentationId(0), 0b01), PSectorFate::Zero);
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(
            owner_spec(class, 0, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(EchoLane)),
        )
        .unwrap();
    builder.build().unwrap()
}

#[test]
fn eps_powers_at_or_below_zero_keep_the_full_support() {
    let library = eps_bubble();
    let context = library.coefficient_context();
    for (powers, value) in [
        ([2, 1], 21),
        ([1, 1], 11),
        ([0, 1], 1),
        ([-1, 1], -9),
        ([-2, 3], -17),
    ] {
        let reduced = library
            .reduce(request(vec![(
                PresentationId(0),
                key(&powers),
                context.one(),
            )]))
            .unwrap();
        assert_eq!(
            reduced.terms,
            [PTerm {
                basis: PBasis::Unit,
                tag: GammaTag::one(),
                coefficient: context.integer(value),
            }],
            "{powers:?}"
        );
        // Full support at any eps power: Route then Apply, never a subsector.
        assert_eq!(reduced.statistics.progress.processed_keys, 2, "{powers:?}");
    }
    // The integer line gone: the forced axis alone, whatever the eps power.
    for powers in [[1, 0], [0, 0], [-1, -2], [3, -1]] {
        let reduced = library
            .reduce(request(vec![(
                PresentationId(0),
                key(&powers),
                context.one(),
            )]))
            .unwrap();
        assert_eq!(reduced.terms, [], "{powers:?}");
    }
}

/// `k1 <-> k2` on a kite with an eps line on `k1 - k2`.
fn swap_loops() -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 2,
        loops_out: 2,
        loop_linear: Box::new([0, 1, 1, 0]),
        loop_external: Box::new([0, 0]),
        external_sign: 1,
    }
}

fn eps_kite_slots() -> Vec<Slot> {
    vec![
        line(&[1, 0, 0]),
        line(&[0, 1, 0]),
        line(&[1, 0, -1]),
        line(&[0, 1, -1]),
        eps_line(&[1, -1, 0], 1),
    ]
}

/// Sector 0b11101 of an eps kite routes onto 0b11110 of its image. Every other
/// support with the forced axis is a zero.
fn routed_eps_kite(context: &CoefficientContext) -> (PLibrary, BTreeMap<Vec<i64>, PTerminalId>) {
    let mut builder = PLibrary::builder(context.clone(), 2..=2);
    for id in 0..2 {
        builder
            .add_presentation(presentation(context, id, 2, eps_kite_slots()))
            .unwrap();
    }
    const SOURCE: u64 = 0b11101;
    const OWNER: u64 = 0b11110;
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("eps kite", 2, 1, OWNER, 4, 1))
        .unwrap();
    for id in 0..2 {
        for support in (0..1_u64 << 5).filter(|support| support & 0b10000 != 0) {
            census
                .sectors
                .insert((PresentationId(id), support), PSectorFate::Zero);
        }
    }
    census.sectors.remove(&(PresentationId(1), OWNER));
    census.sectors.insert(
        (PresentationId(0), SOURCE),
        PSectorFate::Owner {
            class,
            map: swap_loops(),
        },
    );
    let mut terminals = BTreeMap::new();
    for powers in keys_in(&[-3..=0, 1..=3, 1..=3, 1..=3, -3..=3]) {
        let id = builder
            .add_terminal(PTerminal::Residual {
                owner: class,
                key: key(&powers),
            })
            .unwrap();
        terminals.insert(powers, id);
    }
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(
            owner_spec(class, 1, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(ResidualLane(terminals.clone()))),
        )
        .unwrap();
    (builder.build().unwrap(), terminals)
}

#[test]
fn a_route_carries_eps_axes_at_every_power() {
    let context = context();
    let (library, terminals) = routed_eps_kite(&context);
    // Slot 0 -> 1, 1 -> 0, 2 -> 3, 3 -> 2, the eps axis onto itself at any power.
    for (source, image) in [
        ([1, 0, 1, 1, 1], [0, 1, 1, 1, 1]),
        ([2, -1, 1, 3, 0], [-1, 2, 3, 1, 0]),
        ([1, -2, 2, 1, -1], [-2, 1, 1, 2, -1]),
        ([3, -3, 1, 2, -3], [-3, 3, 2, 1, -3]),
        ([1, 0, 2, 1, 3], [0, 1, 1, 2, 3]),
    ] {
        let reduced = library
            .reduce(request(vec![(
                PresentationId(0),
                key(&source),
                context.one(),
            )]))
            .unwrap();
        assert_eq!(
            reduced.terms,
            [PTerm {
                basis: PBasis::Residual(terminals[image.as_slice()]),
                tag: GammaTag::one(),
                coefficient: context.one(),
            }],
            "{source:?}"
        );
        // Route, Apply, nothing in between: never a subsector.
        assert_eq!(reduced.statistics.progress.processed_keys, 2, "{source:?}");
    }
    // A pinched integer line is a zero of the census, whatever the eps power.
    let reduced = library
        .reduce(request(vec![(
            PresentationId(0),
            key(&[0, -1, 1, 1, -2]),
            context.one(),
        )]))
        .unwrap();
    assert_eq!(reduced.terms, []);
}

/// All `L(L + 3) / 2` lines `k_i^2`, `(k_i - q)^2`, `(k_i - k_j)^2`.
fn all_lines(loops: usize) -> Vec<Slot> {
    let unit = |index: usize| -> Vec<i64> {
        let mut row = vec![0; loops + 1];
        row[index] = 1;
        row
    };
    let minus = |left: usize, right: usize| {
        let mut row = unit(left);
        row[right] -= 1;
        row
    };
    let mut slots: Vec<_> = (0..loops).map(|i| line(&unit(i))).collect();
    slots.extend((0..loops).map(|i| line(&minus(i, loops))));
    for i in 0..loops {
        slots.extend((i + 1..loops).map(|j| line(&minus(i, j))));
    }
    slots
}

/// One level of the fake descent: a ladder inside the owner, then the next
/// presentation (or the Unit terminal).
struct ChainLane {
    next: Option<PresentationId>,
    next_arity: usize,
}

/// `1 / (d + slot + 5)` per dot on `slot`.
fn ladder_weight(context: &CoefficientContext, slot: usize, dots: i64) -> Coefficient {
    div(
        context,
        &context.one(),
        &d_plus(context, slot as i64 + dots + 5),
    )
}

fn chain_tag() -> GammaTag {
    GammaTag::from_pairs([(1, 1), (-2, -1)]).unwrap()
}

/// `(d + 1) / (d + 2)` per level.
fn level_weight(context: &CoefficientContext) -> Coefficient {
    div(context, &d_plus(context, 1), &d_plus(context, 2))
}

impl PLaneStep for ChainLane {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let context = cx.library.coefficient_context();
        let dotted: Vec<usize> = (0..key.powers.len())
            .filter(|&slot| key.powers[slot] > 1)
            .collect();
        let lowered = |slot: usize| {
            let mut powers = key.powers.to_vec();
            powers[slot] -= 1;
            powers
        };
        if !dotted.is_empty() {
            // Two parents reach the same key: frontier merges and cancellations.
            let children = dotted
                .iter()
                .take(2)
                .enumerate()
                .map(|(index, &slot)| PChild {
                    stage: PStage::Apply,
                    presentation: key.presentation,
                    powers: lowered(slot),
                    tag: key.tag.clone(),
                    coefficient: {
                        let weight = ladder_weight(context, slot, key.powers[slot]);
                        if index == 0 {
                            weight
                        } else {
                            context.try_neg(&weight, exact()).unwrap()
                        }
                    },
                })
                .collect();
            return Ok(PStepOut {
                children,
                ..PStepOut::default()
            });
        }
        let tag = key.tag.add(&chain_tag());
        let weight = level_weight(context);
        let Some(next) = self.next else {
            return Ok(PStepOut {
                terminals: vec![(PTerminalId(0), tag, weight)],
                ..PStepOut::default()
            });
        };
        let mut powers = vec![1; self.next_arity];
        powers[0] = 2;
        let mut pinched = powers.clone();
        pinched[self.next_arity - 1] = 0;
        Ok(PStepOut {
            children: vec![
                PChild {
                    stage: PStage::Apply,
                    presentation: next,
                    powers,
                    tag: tag.clone(),
                    coefficient: weight,
                },
                PChild {
                    stage: PStage::Route,
                    presentation: next,
                    powers: pinched,
                    tag,
                    coefficient: context.one(),
                },
            ],
            ..PStepOut::default()
        })
    }
}

/// The value of a key of the chain, by the rule of [`ChainLane`], on one tag.
fn chain_value(
    context: &CoefficientContext,
    levels: &[usize],
    powers: Vec<i64>,
    memo: &mut BTreeMap<(usize, Vec<i64>), Coefficient>,
) -> Coefficient {
    let id = (levels.len(), powers.clone());
    if let Some(value) = memo.get(&id) {
        return value.clone();
    }
    let dotted: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] > 1).collect();
    let value = if dotted.is_empty() {
        let weight = level_weight(context);
        match levels.split_first() {
            None => weight,
            Some((&arity, rest)) => {
                let mut next = vec![1; arity];
                next[0] = 2;
                mul(context, &weight, &chain_value(context, rest, next, memo))
            }
        }
    } else {
        let mut sum = context.zero();
        for (index, &slot) in dotted.iter().take(2).enumerate() {
            let mut child = powers.clone();
            child[slot] -= 1;
            let weight = ladder_weight(context, slot, powers[slot]);
            let weight = if index == 0 {
                weight
            } else {
                context.try_neg(&weight, exact()).unwrap()
            };
            let value = chain_value(context, levels, child, memo);
            sum = add(context, &sum, &mul(context, &weight, &value));
        }
        sum
    };
    memo.insert(id, value.clone());
    value
}

const CHAIN_LOOPS: [usize; 4] = [5, 3, 2, 1];
const CHAIN_ARITIES: [usize; 4] = [20, 9, 5, 2];

/// Presentations of arity 20, 9, 5, 2, in descending order of descent, one
/// all-line owner each; a line-less sector of a presentation is a zero.
fn chain_library() -> PLibrary {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 1..=5);
    let mut census = PCensus::default();
    let mut classes = Vec::new();
    for (index, (&loops, &arity)) in CHAIN_LOOPS.iter().zip(&CHAIN_ARITIES).enumerate() {
        let id = index as u32;
        builder
            .add_presentation(presentation(&context, id, loops as u8, all_lines(loops)))
            .unwrap();
        let support = (1_u64 << arity) - 1;
        classes.push(
            census
                .insert_class(class_record(
                    &format!("chain {arity}"),
                    loops as u8,
                    id,
                    support,
                    arity as u8,
                    0,
                ))
                .unwrap(),
        );
        let pinched = support & !(1 << (arity - 1));
        census
            .sectors
            .insert((PresentationId(id), pinched), PSectorFate::Zero);
    }
    builder.set_census(census).unwrap();
    for (index, &class) in classes.iter().enumerate() {
        let next = (index + 1 < classes.len()).then(|| PresentationId(index as u32 + 1));
        let next_arity = CHAIN_ARITIES.get(index + 1).copied().unwrap_or(0);
        builder
            .add_owner_with_program(
                owner_spec(class, index as u32, OwnerLane::Custom),
                OwnerProgram::Lane(Arc::new(ChainLane { next, next_arity })),
            )
            .unwrap();
    }
    builder.build().unwrap()
}

fn chain_input(context: &CoefficientContext) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let mut powers = vec![1; 20];
    powers[0] = 3;
    powers[7] = 2;
    powers[19] = 2;
    vec![(PresentationId(0), key(&powers), context.one())]
}

#[test]
fn keys_of_mixed_length_pass_every_path() {
    let library = chain_library();
    let context = library.coefficient_context();
    let inputs = chain_input(context);
    let mut powers = vec![1; 20];
    powers[0] = 3;
    powers[7] = 2;
    powers[19] = 2;
    let value = chain_value(context, &CHAIN_ARITIES[1..], powers, &mut BTreeMap::new());
    let mut tag = GammaTag::one();
    for _ in CHAIN_ARITIES {
        tag = tag.add(&chain_tag());
    }
    let expected = [PTerm {
        basis: PBasis::Unit,
        tag,
        coefficient: value,
    }];

    let exact_run = library.reduce(request(inputs.clone())).unwrap();
    assert_eq!(exact_run.terms, expected);

    // Every arithmetic, 1 and 16 threads, tails on and off.
    for arithmetic in [
        OrderedWeightArithmetic::Exact,
        modular_polynomial(1),
        modular_polynomial(16),
        modular_sampled(4),
    ] {
        for threads in [1, 16] {
            for adjoint_tail in [true, false] {
                let mut run = request(inputs.clone());
                run.arithmetic = arithmetic;
                run.scheduler = OrderedSchedulerOptions {
                    threads,
                    window_keys: 8,
                    adjoint_tail,
                };
                let reduced = library.reduce(run).unwrap();
                assert_eq!(reduced.terms, expected, "{arithmetic:?} {threads}");
                let (actual, base) = (reduced.statistics.progress, exact_run.statistics.progress);
                assert_eq!(
                    (
                        actual.processed_keys,
                        actual.emitted_terms,
                        actual.merged_terms
                    ),
                    (base.processed_keys, base.emitted_terms, base.merged_terms),
                    "{arithmetic:?} {threads} {adjoint_tail}"
                );
            }
        }
    }

    // Step cache and tail table, on disk: cold, then warm in a fresh cache.
    let binding = library.step_cache_binding().unwrap();
    let directory = scratch("chain");
    let cold = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
    let mut run = request(inputs.clone());
    run.arithmetic = modular_polynomial(4);
    run.scheduler = OrderedSchedulerOptions::with_threads(4);
    run.step_cache = Some(Arc::clone(&cold));
    let first = library.reduce(run).unwrap();
    assert_eq!(first.terms, expected);
    let saved = cold.save().unwrap();
    assert!(saved.records > 0);
    assert!(cold.tail_statistics().is_some_and(|tail| tail.entries > 0));
    assert_eq!(cold.statistics().skipped_inserts, 0, "every identity fits");

    for adjoint_tail in [true, false] {
        let warm = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
        let mut run = request(inputs.clone());
        run.arithmetic = modular_polynomial(4);
        run.scheduler = OrderedSchedulerOptions {
            threads: 4,
            adjoint_tail,
            ..Default::default()
        };
        run.step_cache = Some(Arc::clone(&warm));
        let second = library.reduce(run).unwrap();
        assert_eq!(second.terms, expected);
        let progress = second.statistics.progress;
        assert_eq!(progress.step_cache_misses, 0);
        if adjoint_tail {
            // The persistent tail closes the run: the input's Route step, no more.
            assert_eq!(progress.processed_keys, 1);
        } else {
            assert_eq!(progress.step_cache_hits, progress.processed_keys);
            assert!(progress.processed_keys > 0);
        }
    }
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn two_inputs_record_every_identity_of_the_chain() {
    let library = chain_library();
    let context = library.coefficient_context();
    // Two inputs on the widest presentation: descendants on every presentation
    // merge per tag, and every identity fits the compact record.
    let mut second = vec![1; 20];
    second[3] = 2;
    let mut inputs = chain_input(context);
    inputs.push((PresentationId(0), key(&second), fraction(context, 3, 7)));
    let cache =
        OrderedStepCache::in_memory(library.step_cache_binding().unwrap(), Default::default());
    let mut run = request(inputs);
    run.step_cache = Some(Arc::clone(&cache));
    library.reduce(run).unwrap();
    let statistics = cache.statistics();
    assert!(statistics.records > 0);
    assert_eq!(statistics.skipped_inserts, 0);
}

/// A basis map: Unit stays, with its tag doubled through the map.
struct DoublingMap;

impl PBasisMap for DoublingMap {
    fn map(&self, terminal: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        assert_eq!(terminal, PTerminalId(0));
        let context = context();
        Ok(vec![
            PTerm {
                basis: PBasis::Master("no".to_owned()),
                tag: tag.clone(),
                coefficient: context.integer(2),
            },
            PTerm {
                basis: PBasis::Unit,
                tag: tag.clone(),
                coefficient: context.integer(1),
            },
            PTerm {
                basis: PBasis::Unit,
                tag: tag.clone(),
                coefficient: context.integer(-1),
            },
        ])
    }
}

#[test]
fn onto_masters_goes_through_the_basis_map_and_sorts_its_terms() {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 1..=1);
    builder
        .add_presentation(presentation(
            &context,
            0,
            1,
            vec![line(&[1, 0]), line(&[1, -1])],
        ))
        .unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("mapped", 1, 0, 0b11, 2, 0))
        .unwrap();
    for support in 0b00..0b11 {
        census
            .sectors
            .insert((PresentationId(0), support), PSectorFate::Zero);
    }
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(
            owner_spec(class, 0, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(EchoLane)),
        )
        .unwrap();
    builder.set_basis_map(Arc::new(DoublingMap)).unwrap();
    let library = builder.build().unwrap();

    let mut run = request(vec![(PresentationId(0), key(&[1, 1]), context.one())]);
    run.onto_masters = true;
    let reduced = library.reduce(run).unwrap();
    // 11 * (d - 3) * 2, tag of (eps G(1,1))^-1; the two Unit rows cancel.
    assert_eq!(
        reduced.terms,
        [PTerm {
            basis: PBasis::Master("no".to_owned()),
            tag: GammaTag::from_pairs([(1, -1), (-1, -2), (-2, 1)]).unwrap(),
            coefficient: mul(&context, &context.integer(22), &d_plus(&context, -3)),
        }]
    );
}

/// Bytes of the recorded identities of a run (plan sec.3.5, critique-eng S4):
/// one width per program, `header + max(max arity + tag slots, terminal slots)`.
#[test]
fn key_bytes_per_recorded_identity() {
    let library = one_loop_reference_library().unwrap();
    let context = library.coefficient_context();
    let directory = scratch("bytes");
    let cache = OrderedStepCache::open(
        &directory,
        library.step_cache_binding().unwrap(),
        Default::default(),
    )
    .unwrap();
    let mut run = request(
        ROUTINGS
            .into_iter()
            .flat_map(|routing| lowered(context, routing, 3, 3, 4))
            .collect(),
    );
    run.step_cache = Some(Arc::clone(&cache));
    run.scheduler = no_tail();
    library.reduce(run).unwrap();
    let saved = cache.save().unwrap();
    const HEADER: usize = 9;
    let identity =
        |arity: usize| HEADER + (arity + P_GAMMA_TAG_WIDTH).max(PTerminalCodec::KEY_SLOTS);
    let shard = std::fs::read_dir(directory.join("shards"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .max_by_key(|path| std::fs::metadata(path).unwrap().len())
        .unwrap();
    eprintln!(
        "reference library: {} records, {} file bytes, {:.1} per record, largest shard {}; \
         identity bytes by max arity: 2 {}, 5 {}, 9 {}, 14 {}, 20 {}",
        saved.records,
        saved.bytes,
        saved.bytes as f64 / saved.records as f64,
        std::fs::metadata(&shard).unwrap().len(),
        identity(2),
        identity(5),
        identity(9),
        identity(14),
        identity(20),
    );
    assert_eq!(identity(2), 28);
    assert_eq!(identity(9), 34);
    assert_eq!(identity(14), 39);
    assert!(saved.records > 0);
    std::fs::remove_dir_all(&directory).unwrap();
}
