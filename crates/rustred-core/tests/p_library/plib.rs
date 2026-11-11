//! The p library on the Ordered engine: closed forms, routes, eps supports, step
//! cache, typed errors, keys of mixed length (owner: M2-T6 rr-plib).
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensus, PClassRecord, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::OwnerClassKey;
use rustred::propagator::error::PError;
use rustred::propagator::lane::{LaneRoles, OwnerLane, TriangleRoles};
use rustred::propagator::library::{
    OwnerProgram, PBasis, PBasisMap, PLibrary, PLibraryBuilder, POwnerSpec, PReduction,
    PReductionRequest, PTerm, PTerminal, PUncoveredPolicy,
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
    ModularCommon, OrderedKeyStage, OrderedProfile, OrderedSchedulerOptions, OrderedStepCache,
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
    assert_eq!(binding.family_fingerprint, library.step_identity());
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

/// A library regenerated without a version bump keeps the identity of the old
/// one, so it can open the old cache: a record that names a terminal the new
/// library does not have fails closed instead of reaching the output.
#[test]
fn a_stale_record_of_an_unknown_terminal_is_refused() {
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
    let residual = builder
        .add_terminal(PTerminal::Residual {
            owner: class,
            key: key(&[1, 1]),
        })
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
            OwnerProgram::Lane(Arc::new(ResidualLane(BTreeMap::from([(
                vec![1, 1],
                residual,
            )])))),
        )
        .unwrap();
    let old = builder.build().unwrap();
    let (new, _) = one_owner_library(Arc::new(EchoLane));
    // Same reduction settings (the fake lanes keep canonicalisation off), so
    // the two share one step cache binding.
    old.set_orbit_canonicalization(new.orbit_canonicalization());
    assert_eq!(new.identity(), old.identity());
    assert_eq!(new.step_identity(), old.step_identity());
    assert_eq!(new.terminals().len(), 1);

    for scheduler in [no_tail(), OrderedSchedulerOptions::default()] {
        let cache =
            OrderedStepCache::in_memory(old.step_cache_binding().unwrap(), Default::default());
        let run = |library: &PLibrary| {
            let mut run = request(bubble_input(library));
            run.scheduler = scheduler;
            run.step_cache = Some(Arc::clone(&cache));
            library.reduce(run)
        };
        assert_eq!(
            run(&old).unwrap().terms,
            [PTerm {
                basis: PBasis::Residual(residual),
                tag: GammaTag::one(),
                coefficient: context.one(),
            }]
        );
        assert!(matches!(run(&new), Err(PError::MalformedLibrary(_))));
    }
}

/// The record bytes and the identity of the reference program: a changed record
/// needs a changed `P_*` version, and with it a new identity here. Orbit
/// canonicalisation (M5 R0 A2, opt-in) changes the records and adds
/// `P_ORBIT_CANON_VERSION` to the step identity (never to the library identity
/// that `library.json` records); switched off, both identities and the records
/// are the ones from before it.
#[test]
fn the_identity_and_the_records_of_the_reference_program_are_pinned() {
    const BASE: &str = "p-library:scope=1-1:\
         P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,\
         P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=2,P_PRESENTATION_VERSION=2,\
         P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,\
         P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,\
         P_TRIANGLE_TEMPLATE_VERSION=1,P_DIAMOND_TEMPLATE_VERSION=1,P_SCHEME_TEMPLATE_VERSION=3,P_CUSTOM_OWNER_VERSION=2,\
         P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,\
         P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1";
    for (on, identity, digest) in [
        (
            false,
            BASE.to_owned(),
            "e06d50fd1bc0e8e681c69ba2eb2dc406c0da56ebc6b2acb40ceab845345c8e64",
        ),
        (
            true,
            format!("{BASE},P_ORBIT_CANON_VERSION=3"),
            ORBIT_CANON_RECORDS_DIGEST,
        ),
    ] {
        let library = one_loop_reference_library().unwrap();
        library.set_orbit_canonicalization(on);
        assert_eq!(library.identity(), BASE);
        assert_eq!(library.step_identity(), identity);
        let context = library.coefficient_context();
        let directory = scratch(if on { "golden-orbit" } else { "golden" });
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
            digest,
            "the recorded p steps changed: bump the P_* version whose meaning changed"
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

/// The records of the reference program with orbit canonicalisation on.
const ORBIT_CANON_RECORDS_DIGEST: &str =
    "f8c04985502236c51ac223573077ca72a1bf4106d05e237cddd0be17aad273a0";

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
    // Fake lanes: their values are not symmetric under the automorphisms of
    // the presentation, so orbit canonicalisation (M5 R0 A2) stays off here.
    let library = builder.build().unwrap();
    library.set_orbit_canonicalization(false);
    (library, terminals)
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

/// A top on presentation 0 whose lane emits a tagged Route child on presentation
/// 1, where sector 0b01111 routes by [`flip_first_loop`] onto its owner on
/// presentation 2. Sector 0b00111 of presentation 2 has an owner of its own, so a
/// numerator of the route pinches from one nonzero owner onto the other.
fn route_tag_library(context: &CoefficientContext) -> (PLibrary, BTreeMap<Vec<i64>, PTerminalId>) {
    let mut builder = PLibrary::builder(context.clone(), 2..=2);
    for id in 0..3 {
        builder
            .add_presentation(presentation(context, id, 2, kite_slots()))
            .unwrap();
    }
    let mut census = PCensus::default();
    let top = census
        .insert_class(class_record("top", 2, 0, 0b11111, 5, 0))
        .unwrap();
    let upper = census
        .insert_class(class_record("upper", 2, 2, 0b01111, 4, 0))
        .unwrap();
    let lower = census
        .insert_class(class_record("lower", 2, 2, 0b00111, 3, 0))
        .unwrap();
    for id in 0..3 {
        for support in 0..1_u64 << 5 {
            census
                .sectors
                .insert((PresentationId(id), support), PSectorFate::Zero);
        }
    }
    census.sectors.remove(&(PresentationId(0), 0b11111));
    census.sectors.remove(&(PresentationId(2), 0b01111));
    census.sectors.remove(&(PresentationId(2), 0b00111));
    census.sectors.insert(
        (PresentationId(1), 0b01111),
        PSectorFate::Owner {
            class: upper,
            map: flip_first_loop(),
        },
    );
    let mut terminals = BTreeMap::new();
    let mut residuals =
        |builder: &mut PLibraryBuilder, owner, ranges: &[std::ops::RangeInclusive<i64>]| {
            let mut own = BTreeMap::new();
            for powers in keys_in(ranges) {
                let id = builder
                    .add_terminal(PTerminal::Residual {
                        owner,
                        key: key(&powers),
                    })
                    .unwrap();
                own.insert(powers.clone(), id);
                terminals.insert(powers, id);
            }
            own
        };
    let upper_terminals = residuals(&mut builder, upper, &[1..=4, 1..=4, 1..=4, 1..=4, -3..=0]);
    let lower_terminals = residuals(&mut builder, lower, &[1..=4, 1..=4, 1..=4, -3..=0, -3..=0]);
    builder.set_census(census).unwrap();
    for (class, own) in [(upper, upper_terminals), (lower, lower_terminals)] {
        builder
            .add_owner_with_program(
                owner_spec(class, 2, OwnerLane::Custom),
                OwnerProgram::Lane(Arc::new(ResidualLane(own))),
            )
            .unwrap();
    }
    builder
        .add_owner_with_program(
            owner_spec(top, 0, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(FnLane(
                |cx: &PStepContext<'_>, key: &PKeyView<'_>| {
                    Ok(PStepOut {
                        children: vec![PChild {
                            stage: PStage::Route,
                            presentation: PresentationId(1),
                            powers: vec![2, 1, 3, 1, -1],
                            tag: key.tag.add(&chain_tag()),
                            coefficient: cx.library.coefficient_context().integer(5),
                        }],
                        ..PStepOut::default()
                    })
                },
            ))),
        )
        .unwrap();
    // Fake lanes: their values are not symmetric under the automorphisms of
    // the presentation, so orbit canonicalisation (M5 R0 A2) stays off here.
    let library = builder.build().unwrap();
    library.set_orbit_canonicalization(false);
    (library, terminals)
}

#[test]
fn a_route_keeps_its_tag_and_pinches_onto_another_owner() {
    let context = context();
    let (library, terminals) = route_tag_library(&context);
    let inputs = vec![(PresentationId(0), key(&[1, 1, 1, 1, 1]), context.one())];
    // (1 + D1 + D2 + D3 + D4 - D5) over the image [3, 1, 2, 1, 0] of the
    // denominators: the D2 term lands on a zero, the D4 term on the second owner.
    let mut expected: BTreeMap<PTerminalId, i64> = BTreeMap::new();
    for (powers, value) in [
        ([3, 1, 2, 1, 0], 5),
        ([2, 1, 2, 1, 0], 5),
        ([3, 1, 1, 1, 0], 5),
        ([3, 1, 2, 0, 0], 5),
        ([3, 1, 2, 1, -1], -5),
    ] {
        expected.insert(terminals[powers.as_slice()], value);
    }
    let expected: Vec<PTerm> = expected
        .into_iter()
        .map(|(id, value)| PTerm {
            basis: PBasis::Residual(id),
            tag: chain_tag(),
            coefficient: context.integer(value),
        })
        .collect();
    assert_eq!(
        library.reduce(request(inputs.clone())).unwrap().terms,
        expected
    );
    for (arithmetic, threads) in [
        (modular_polynomial(1), 1),
        (modular_polynomial(16), 16),
        (OrderedWeightArithmetic::Exact, 16),
    ] {
        let mut run = request(inputs.clone());
        run.arithmetic = arithmetic;
        run.scheduler = OrderedSchedulerOptions {
            threads,
            window_keys: 8,
            adjoint_tail: true,
            // Generic: the tail runs as `adjoint_tail` says (the p profile turns it off).
            profile: OrderedProfile::Generic,
        };
        assert_eq!(library.reduce(run).unwrap().terms, expected);
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

/// The opt-in path census (M5 R0 WP0) through an inspector.
struct CensusProbe(rustred::propagator::path_census::PPathCensus);

impl rustred::propagator::library::PInspector for CensusProbe {
    fn inspect(&mut self, _: &rustred::reduction::OrderedPhaseEvent<'_>) -> Result<(), PError> {
        Ok(())
    }

    fn path_census(&mut self) -> Option<&mut rustred::propagator::path_census::PPathCensus> {
        Some(&mut self.0)
    }
}

/// T-BFS: the census changes no value, sees every popped key once, in
/// non-decreasing phases, each owner in one run per Apply phase, and counts
/// the driver's keys and edges; its key cap stops the request.
#[test]
fn the_path_census_changes_no_value_and_certifies_the_layers() {
    use rustred::propagator::path_census::{PPathCensus, PPathCensusOptions};
    let context = context();
    let (library, _) = routed_kite(&context);
    let inputs = vec![
        (PresentationId(0), key(&[2, 1, 3, 1, -1]), context.one()),
        (
            PresentationId(0),
            key(&[1, 2, 1, 1, -2]),
            context.integer(3),
        ),
    ];
    let plain = library.reduce(request(inputs.clone())).unwrap();
    let options = PPathCensusOptions {
        keep_apply_keys: true,
        max_keys: None,
    };
    for (arithmetic, scheduler) in [
        (
            OrderedWeightArithmetic::Exact,
            OrderedSchedulerOptions::default(),
        ),
        (modular_polynomial(1), OrderedSchedulerOptions::default()),
        (OrderedWeightArithmetic::Exact, sixteen_threads()),
        (modular_polynomial(16), sixteen_threads()),
    ] {
        let mut probe = CensusProbe(PPathCensus::new(options));
        let mut run = request(inputs.clone());
        run.arithmetic = arithmetic;
        run.scheduler = scheduler;
        run.inspector = Some(&mut probe);
        let reduced = library.reduce(run).unwrap();
        same_result(&reduced, &plain);
        let census = probe.0;
        let certificate = census.certificate();
        assert!(certificate.holds(), "{certificate:?}");
        assert!(certificate.popped > 0 && certificate.apply_phases > 0);
        let progress = reduced.statistics.progress;
        assert_eq!(certificate.popped, progress.processed_keys as u64);
        let steps: u64 = census.steps.values().map(|agg| agg.steps).sum();
        assert_eq!(steps, census.popped);
        let edges: u64 = census.steps.values().map(|agg| agg.edges()).sum();
        assert_eq!(edges, progress.emitted_terms as u64);
        let applies: u64 = census
            .steps
            .iter()
            .filter(|(key, _)| key.1 == 1)
            .map(|(_, agg)| agg.steps)
            .sum();
        assert_eq!(census.apply_keys.len() as u64, applies);
        assert!(census.inflow.values().any(|inflow| !inflow.is_empty()));
        assert!(!census.owners.is_empty());
    }
    let mut capped = CensusProbe(PPathCensus::new(PPathCensusOptions {
        keep_apply_keys: false,
        max_keys: Some(1),
    }));
    let mut run = request(inputs);
    run.inspector = Some(&mut capped);
    assert!(matches!(library.reduce(run), Err(PError::Unsupported(_))));
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
    // Fake lanes: their values are not symmetric under the automorphisms of
    // the presentation, so orbit canonicalisation (M5 R0 A2) stays off here.
    let library = builder.build().unwrap();
    library.set_orbit_canonicalization(false);
    (library, class)
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

/// A lane given by a closure.
struct FnLane<F>(F);

impl<F> PLaneStep for FnLane<F>
where
    F: Fn(&PStepContext<'_>, &PKeyView<'_>) -> Result<PStepOut, PError> + Send + Sync,
{
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        (self.0)(cx, key)
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
            children: vec![PChild {
                stage: PStage::Route,
                presentation: PresentationId(0),
                powers: vec![0, 1],
                tag: GammaTag::one(),
                coefficient: x.clone(),
            }],
            ..PStepOut::default()
        },
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
    // An extra slot with an ISP sign would read as a tag exponent, a short key
    // as a sector of the census: neither reaches the engine.
    for powers in [&[1, 1, 1][..], &[1, 1, -1], &[1]] {
        assert!(
            matches!(
                library.reduce(request(vec![(
                    PresentationId(0),
                    key(powers),
                    context.one()
                )])),
                Err(PError::Unsupported(_))
            ),
            "{powers:?}"
        );
    }

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

    // Lane output that names nothing of the library, even if it cancels.
    for output in [
        PStepOut {
            terminals: vec![(PTerminalId(9), GammaTag::one(), context.one())],
            ..PStepOut::default()
        },
        PStepOut {
            terminals: vec![
                (PTerminalId(9), GammaTag::one(), context.one()),
                (PTerminalId(9), GammaTag::one(), context.integer(-1)),
            ],
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

/// The Apply child `(a - 1, b)` down to `a = 1`, then a Unit terminal; each step
/// emits the condition `d - a - 10`.
fn conditional_lane(cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
    let context = cx.library.coefficient_context();
    let &[a, b] = key.powers else {
        panic!("a key of the bubble")
    };
    let conditions = vec![d_plus(context, -(a + 10))];
    if a == 1 {
        return Ok(PStepOut {
            terminals: vec![(PTerminalId(0), key.tag.clone(), context.one())],
            conditions,
            ..PStepOut::default()
        });
    }
    Ok(PStepOut {
        children: vec![PChild {
            stage: PStage::Apply,
            presentation: key.presentation,
            powers: vec![a - 1, b],
            tag: key.tag.clone(),
            coefficient: div(context, &context.one(), &d_plus(context, -a)),
        }],
        conditions,
        ..PStepOut::default()
    })
}

#[test]
fn conditions_reach_the_output_on_every_path() {
    let (library, _) = one_owner_library(Arc::new(FnLane(conditional_lane)));
    let context = library.coefficient_context();
    let inputs = vec![(PresentationId(0), key(&[4, 1]), context.one())];
    let base = library.reduce(request(inputs.clone())).unwrap();
    let expected: Vec<_> = [-14, -13, -12, -11]
        .into_iter()
        .map(|constant| d_plus(context, constant))
        .collect();
    assert_eq!(base.nonzero_conditions, expected);
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
                    // Generic: the tail runs as `adjoint_tail` says (the p profile turns it off).
                    profile: OrderedProfile::Generic,
                };
                same_result(&library.reduce(run).unwrap(), &base);
            }
        }
    }
    // Cold, then warm from disk.
    let binding = library.step_cache_binding().unwrap();
    for (index, arithmetic) in [OrderedWeightArithmetic::Exact, modular_polynomial(4)]
        .into_iter()
        .enumerate()
    {
        for adjoint_tail in [true, false] {
            let directory = scratch(&format!("conditions-{index}-{adjoint_tail}"));
            let scheduler = OrderedSchedulerOptions {
                threads: 4,
                adjoint_tail,
                ..Default::default()
            };
            let cached = |cache: &Arc<OrderedStepCache>| {
                let mut run = request(inputs.clone());
                run.arithmetic = arithmetic;
                run.scheduler = scheduler;
                run.step_cache = Some(Arc::clone(cache));
                library.reduce(run).unwrap()
            };
            let cold =
                OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
            same_result(&cached(&cold), &base);
            cold.save().unwrap();
            let warm =
                OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
            let replayed = cached(&warm);
            same_result(&replayed, &base);
            assert_eq!(replayed.statistics.progress.step_cache_misses, 0);
            std::fs::remove_dir_all(&directory).unwrap();
        }
    }
}

/// A key below the tail entry of an earlier run needs the guards of its own
/// descendants, and only those.
#[test]
fn a_warm_tail_keeps_the_guards_of_a_new_input() {
    let (library, _) = one_owner_library(Arc::new(FnLane(conditional_lane)));
    let context = library.coefficient_context();
    let binding = library.step_cache_binding().unwrap();
    let directory = scratch("tail-guards");
    let cached = |cache: &Arc<OrderedStepCache>, powers: &[i64]| {
        let mut run = request(vec![(PresentationId(0), key(powers), context.one())]);
        run.arithmetic = modular_polynomial(4);
        run.scheduler = OrderedSchedulerOptions::with_threads(4);
        run.step_cache = Some(Arc::clone(cache));
        library.reduce(run).unwrap()
    };
    let cold = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
    cached(&cold, &[5, 1]);
    cold.save().unwrap();

    let lower = [3, 1];
    let plain = library
        .reduce(request(vec![(
            PresentationId(0),
            key(&lower),
            context.one(),
        )]))
        .unwrap();
    assert_eq!(
        plain.nonzero_conditions,
        [-13, -12, -11].map(|constant| d_plus(context, constant))
    );
    let warm = OrderedStepCache::open(&directory, binding, Default::default()).unwrap();
    same_result(&cached(&warm, &lower), &plain);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_condition_that_is_identically_zero_is_refused() {
    let context = context();
    let (library, _) = one_owner_library(Arc::new(FixedLane(PStepOut {
        terminals: vec![(PTerminalId(0), GammaTag::one(), context.one())],
        conditions: vec![context.zero()],
        ..PStepOut::default()
    })));
    assert!(matches!(
        library.reduce(request(bubble_input(&library))),
        Err(PError::Contract(_))
    ));
}

/// Without nodes the rank follows the class id, the smaller id reduced first: an
/// Apply child on another owner of the same `t` needs a lower rank.
#[test]
fn the_rank_orders_owners_of_one_t() {
    let context = context();
    let corner =
        |support: u64| -> Vec<i64> { (0..5).map(|slot| (support >> slot & 1) as i64).collect() };
    let build = |parent_is_first: bool| {
        let mut builder = PLibrary::builder(context.clone(), 2..=2);
        builder
            .add_presentation(presentation(&context, 0, 2, kite_slots()))
            .unwrap();
        let mut census = PCensus::default();
        let one = census
            .insert_class(class_record("one", 2, 0, 0b01111, 4, 0))
            .unwrap();
        let two = census
            .insert_class(class_record("two", 2, 0, 0b10111, 4, 0))
            .unwrap();
        let (mut first, mut second) = ((one, 0b01111_u64), (two, 0b10111_u64));
        if second.0 < first.0 {
            std::mem::swap(&mut first, &mut second);
        }
        let (parent, child) = if parent_is_first {
            (first, second)
        } else {
            (second, first)
        };
        let powers = corner(child.1);
        builder.set_census(census).unwrap();
        builder
            .add_owner_with_program(
                owner_spec(parent.0, 0, OwnerLane::Custom),
                OwnerProgram::Lane(Arc::new(FnLane(
                    move |cx: &PStepContext<'_>, key: &PKeyView<'_>| {
                        Ok(PStepOut {
                            children: vec![PChild {
                                stage: PStage::Apply,
                                presentation: key.presentation,
                                powers: powers.clone(),
                                tag: key.tag.clone(),
                                coefficient: cx.library.coefficient_context().integer(3),
                            }],
                            ..PStepOut::default()
                        })
                    },
                ))),
            )
            .unwrap();
        builder
            .add_owner_with_program(
                owner_spec(child.0, 0, OwnerLane::Custom),
                OwnerProgram::Lane(Arc::new(EchoLane)),
            )
            .unwrap();
        (builder.build().unwrap(), corner(parent.1))
    };
    let (library, input) = build(true);
    let reduced = library
        .reduce(request(vec![(
            PresentationId(0),
            key(&input),
            context.one(),
        )]))
        .unwrap();
    assert_eq!(reduced.terms.len(), 1);
    let (library, input) = build(false);
    assert!(matches!(
        library.reduce(request(vec![(
            PresentationId(0),
            key(&input),
            context.one()
        )])),
        Err(PError::Engine(ReductionError::ReducerInvariant { .. }))
    ));
}

/// A Triangle step can keep the corner distance and lower only
/// `n_b + n_c1 + n_c2`: the default order key of a Triangle owner reads it first.
#[test]
fn the_default_order_key_lets_a_triangle_step_descend() {
    let context = context();
    let mut builder = PLibrary::builder(context.clone(), 2..=2);
    builder
        .add_presentation(presentation(&context, 0, 2, kite_slots()))
        .unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record("triangle", 2, 0, 0b11111, 5, 0))
        .unwrap();
    builder.set_census(census).unwrap();
    let mut spec = owner_spec(class, 0, OwnerLane::Triangle);
    spec.roles = LaneRoles::Triangle(TriangleRoles {
        b: 0,
        a: [1, 2],
        c: [3, 4],
        k_loop: 0,
    });
    builder
        .add_owner_with_program(
            spec,
            OwnerProgram::Lane(Arc::new(FnLane(
                |cx: &PStepContext<'_>, key: &PKeyView<'_>| {
                    let context = cx.library.coefficient_context();
                    if key.powers[0] == 1 {
                        return Ok(PStepOut {
                            terminals: vec![(PTerminalId(0), key.tag.clone(), context.one())],
                            ..PStepOut::default()
                        });
                    }
                    // A dot moves from `b` to `a`: same distance from the corner.
                    let mut powers = key.powers.to_vec();
                    powers[0] -= 1;
                    powers[1] += 1;
                    Ok(PStepOut {
                        children: vec![PChild {
                            stage: PStage::Apply,
                            presentation: key.presentation,
                            powers,
                            tag: key.tag.clone(),
                            coefficient: context.integer(2),
                        }],
                        ..PStepOut::default()
                    })
                },
            ))),
        )
        .unwrap();
    // Fake lanes: their values are not symmetric under the automorphisms of
    // the presentation, so orbit canonicalisation (M5 R0 A2) stays off here.
    let library = builder.build().unwrap();
    library.set_orbit_canonicalization(false);
    let reduced = library
        .reduce(request(vec![(
            PresentationId(0),
            key(&[3, 1, 1, 1, 1]),
            context.one(),
        )]))
        .unwrap();
    assert_eq!(reduced.terms[0].coefficient, context.integer(4));
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
    // Fake lanes: their values are not symmetric under the automorphisms of
    // the presentation, so orbit canonicalisation (M5 R0 A2) stays off here.
    let library = builder.build().unwrap();
    library.set_orbit_canonicalization(false);
    library
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
                    // Generic: the tail runs as `adjoint_tail` says (the p profile turns it off).
                    profile: OrderedProfile::Generic,
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
    // Generic: the p profile turns the adjoint tail off, and with it the tail
    // table this chain tests (M5 batch 2).
    run.scheduler = OrderedSchedulerOptions {
        profile: OrderedProfile::Generic,
        ..OrderedSchedulerOptions::with_threads(4)
    };
    run.step_cache = Some(Arc::clone(&cold));
    let first = library.reduce(run).unwrap();
    assert_eq!(first.terms, expected);
    let saved = cold.save().unwrap();
    assert!(saved.records > 0);
    assert!(cold.tail_statistics().is_some_and(|tail| tail.entries > 0));
    assert_eq!(cold.statistics().skipped_inserts, 0, "every identity fits");
    assert_eq!(
        identity_widths(&directory),
        BTreeSet::from([IDENTITY_HEADER + 20 + P_GAMMA_TAG_WIDTH]),
        "one width across presentations and terminals"
    );

    for adjoint_tail in [true, false] {
        let warm = OrderedStepCache::open(&directory, binding.clone(), Default::default()).unwrap();
        let mut run = request(inputs.clone());
        run.arithmetic = modular_polynomial(4);
        run.scheduler = OrderedSchedulerOptions {
            threads: 4,
            adjoint_tail,
            profile: OrderedProfile::Generic,
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

/// A bubble whose one owner is [`EchoLane`], with `map` as its basis map.
fn mapped_bubble(map: Arc<dyn PBasisMap>) -> PLibrary {
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
    builder.set_basis_map(map).unwrap();
    builder.build().unwrap()
}

fn onto_masters(library: &PLibrary) -> Result<PReduction, PError> {
    let mut run = request(bubble_input(library));
    run.onto_masters = true;
    library.reduce(run)
}

#[test]
fn onto_masters_goes_through_the_basis_map_and_sorts_its_terms() {
    let context = context();
    let library = mapped_bubble(Arc::new(DoublingMap));
    // 11 * (d - 3) * 2, tag of (eps G(1,1))^-1; the two Unit rows cancel.
    assert_eq!(
        onto_masters(&library).unwrap().terms,
        [PTerm {
            basis: PBasis::Master("no".to_owned()),
            tag: GammaTag::from_pairs([(1, -1), (-1, -2), (-2, 1)]).unwrap(),
            coefficient: mul(&context, &context.integer(22), &d_plus(&context, -3)),
        }]
    );
}

/// A map whose tag leaves the key range: `|e| <= 127` holds at every step of the
/// engine, and at its exit too.
struct SaturatingMap;

impl PBasisMap for SaturatingMap {
    fn map(&self, _: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        let wide = GammaTag::from_pairs([(1, 127)]).unwrap();
        Ok(vec![PTerm {
            basis: PBasis::Unit,
            tag: tag.add(&wide).add(&wide).add(&wide),
            coefficient: context().one(),
        }])
    }
}

struct EmptyMap;

impl PBasisMap for EmptyMap {
    fn map(&self, _: PTerminalId, _: &GammaTag) -> Result<Vec<PTerm>, PError> {
        Ok(Vec::new())
    }
}

/// A terminal that really vanishes says so with a zero coefficient.
struct ZeroMap;

impl PBasisMap for ZeroMap {
    fn map(&self, _: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        Ok(vec![PTerm {
            basis: PBasis::Unit,
            tag: tag.clone(),
            coefficient: context().zero(),
        }])
    }
}

#[test]
fn a_basis_map_answers_with_a_row_in_the_key_range() {
    assert_eq!(
        onto_masters(&mapped_bubble(Arc::new(SaturatingMap))).map(|reduced| reduced.terms),
        Err(PError::TagOutOfRange)
    );
    assert!(matches!(
        onto_masters(&mapped_bubble(Arc::new(EmptyMap))),
        Err(PError::MalformedLibrary(_))
    ));
    assert_eq!(
        onto_masters(&mapped_bubble(Arc::new(ZeroMap)))
            .unwrap()
            .terms,
        []
    );
}

/// `|e| = 127` holds on the key; the scheme of `onto_masters` crosses it.
#[test]
fn the_scheme_of_onto_masters_can_leave_the_key_range() {
    let context = context();
    let (library, _) = one_owner_library(Arc::new(FixedLane(PStepOut {
        terminals: vec![(
            PTerminalId(0),
            GammaTag::from_pairs([(-2, 127)]).unwrap(),
            context.one(),
        )],
        ..PStepOut::default()
    })));
    let mut run = request(bubble_input(&library));
    run.onto_masters = true;
    assert_eq!(
        library.reduce(run).map(|reduced| reduced.terms),
        Err(PError::TagOutOfRange)
    );
}

/// A row of the basis map with a coefficient of another context.
struct ForeignMap;

impl PBasisMap for ForeignMap {
    fn map(&self, _: PTerminalId, tag: &GammaTag) -> Result<Vec<PTerm>, PError> {
        let foreign = CoefficientContext::try_new(["d", "x"]).unwrap();
        Ok(vec![PTerm {
            basis: PBasis::Unit,
            tag: tag.clone(),
            coefficient: foreign.parameter("x").unwrap(),
        }])
    }
}

#[test]
fn a_foreign_coefficient_in_a_row_is_refused() {
    assert_eq!(
        onto_masters(&mapped_bubble(Arc::new(ForeignMap))).map(|reduced| reduced.terms),
        Err(PError::ContextMismatch)
    );
}

/// One Unit terminal on the full sector of `L (L + 3) / 2` lines.
fn unit_owner_library(context: &CoefficientContext, loops: u8) -> PLibrary {
    let slots = all_lines(usize::from(loops));
    let arity = slots.len();
    let mut builder = PLibrary::builder(context.clone(), loops..=loops);
    builder
        .add_presentation(presentation(context, 0, loops, slots))
        .unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(class_record(
            "unit",
            loops,
            0,
            (1_u64 << arity) - 1,
            arity as u8,
            0,
        ))
        .unwrap();
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(
            owner_spec(class, 0, OwnerLane::Custom),
            OwnerProgram::Lane(Arc::new(FnLane(
                |cx: &PStepContext<'_>, key: &PKeyView<'_>| {
                    Ok(PStepOut {
                        terminals: vec![(
                            PTerminalId(0),
                            key.tag.clone(),
                            cx.library.coefficient_context().one(),
                        )],
                        ..PStepOut::default()
                    })
                },
            ))),
        )
        .unwrap();
    builder.build().unwrap()
}

/// Convention C3: `(eps G(1,1))^-L`, in the tag and in `(d - 3)^L`.
#[test]
fn onto_masters_scales_with_the_loop_count() {
    let context = context();
    for loops in [2_u8, 3] {
        let library = unit_owner_library(&context, loops);
        let arity = library.presentation(PresentationId(0)).slots.len();
        let mut run = request(vec![(
            PresentationId(0),
            key(&vec![1; arity]),
            context.one(),
        )]);
        run.onto_masters = true;
        let reduced = library.reduce(run).unwrap();
        let l = i16::from(loops);
        let factor = (0..loops).fold(context.one(), |factor, _| {
            mul(&context, &factor, &d_plus(&context, -3))
        });
        assert_eq!(
            reduced.terms,
            [PTerm {
                basis: PBasis::Unit,
                tag: GammaTag::from_pairs([(1, -l), (-1, -2 * l), (-2, l)]).unwrap(),
                coefficient: factor,
            }]
        );
        assert_eq!(reduced.dimension, i64::from(loops));
    }
}

/// Bytes before the slots of a recorded identity (`NODE_HEADER`, `step_cache/codec.rs`).
const IDENTITY_HEADER: usize = 9;

/// The identity widths the shards of a saved cache declare: the first varint of
/// each payload, after the 99 header bytes of the shard file (`SHARD_HEADER`,
/// `step_cache/mod.rs`). Every width here fits one byte of that varint.
fn identity_widths(directory: &Path) -> BTreeSet<usize> {
    std::fs::read_dir(directory.join("shards"))
        .unwrap()
        .map(|entry| usize::from(std::fs::read(entry.unwrap().path()).unwrap()[99]))
        .collect()
}

/// Bytes of the recorded identities of a run (plan sec.3.5, critique-eng S4):
/// one width per program, `header + max(max arity + tag slots, terminal slots)`.
/// The width of this program is read from its files; the printed widths of the
/// other arities are that formula, a projection.
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
    let identity =
        |arity: usize| IDENTITY_HEADER + (arity + P_GAMMA_TAG_WIDTH).max(PTerminalCodec::KEY_SLOTS);
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
    assert_eq!(identity_widths(&directory), BTreeSet::from([28]));
    assert!(saved.records > 0);
    std::fs::remove_dir_all(&directory).unwrap();
}

/// M5 R0 A2: orbit canonicalisation merges the symmetric copies of an Apply
/// key and changes no value. On the reference bubble `G(a, b) = G(b, a)` (the
/// swap `k -> q - k` is the owner's automorphism), so `G(2, 1)` and `G(1, 2)`
/// become one Apply key; the swapped presentation routes onto the same
/// representative. Exact and modular, one and sixteen threads.
#[test]
fn orbit_canonicalisation_merges_symmetric_keys_and_changes_no_value() {
    let library = one_loop_reference_library().unwrap();
    assert_eq!(
        library.orbit_canonicalization(),
        rustred::propagator::P_ORBIT_CANON_DEFAULT
    );
    let context = library.coefficient_context().clone();
    let inputs = vec![
        (REFERENCE_TOP, key(&[2, 1]), context.one()),
        (REFERENCE_TOP, key(&[1, 2]), context.integer(3)),
        (REFERENCE_TOP, key(&[3, 1]), context.integer(5)),
        (REFERENCE_TOP, key(&[1, 3]), context.integer(-7)),
        (REFERENCE_SWAPPED, key(&[2, 3]), context.integer(2)),
        (REFERENCE_TOP, key(&[3, 2]), context.one()),
    ];
    let reduce = |on: bool, arithmetic, scheduler| {
        library.set_orbit_canonicalization(on);
        let mut run = request(inputs.clone());
        run.arithmetic = arithmetic;
        run.scheduler = scheduler;
        library.reduce(run).unwrap()
    };
    let off = reduce(
        false,
        OrderedWeightArithmetic::Exact,
        OrderedSchedulerOptions::default(),
    );
    assert!(!off.terms.is_empty());
    for (arithmetic, scheduler) in [
        (
            OrderedWeightArithmetic::Exact,
            OrderedSchedulerOptions::default(),
        ),
        (modular_polynomial(1), OrderedSchedulerOptions::default()),
        (OrderedWeightArithmetic::Exact, sixteen_threads()),
        (modular_polynomial(16), sixteen_threads()),
    ] {
        let on = reduce(true, arithmetic, scheduler);
        same_result(&on, &off);
        assert!(
            on.statistics.progress.processed_keys < off.statistics.progress.processed_keys,
            "{} keys on, {} off",
            on.statistics.progress.processed_keys,
            off.statistics.progress.processed_keys
        );
        assert!(on.statistics.work.rule_applications() < off.statistics.work.rule_applications());
    }
    library.set_orbit_canonicalization(rustred::propagator::P_ORBIT_CANON_DEFAULT);
}
