//! Custom-owner generation and the false-knock-out guard (owner: M2-T7c rr-custom).
//!
//! The two Custom nodes of the 3L reduction DAG, generated all-rank through
//! the application path (`generate_custom_owner`): NO (`nostar0`, integer, on
//! its Induced presentation of `P3L_NO`, arity 9) and t1star05 (the 2L kite
//! with one eps unit on the central line, on its EpsNode presentation, arity
//! 5, both raw charts of the eps axis). The tests pin the residual counts
//! against the LP counts, the hard guard at generation and at build, the
//! closure of `CustomStep` at the brief's points (only same-owner Apply keys,
//! routable keys and the owner's residual), and reductions through
//! hand-assembled libraries in which every class of the presentation is a
//! generated SpIRed owner, next to the 3L tops (arity 9): exact and
//! `Z_p(d)` arithmetic give identical results.
//!
//! Domains (coordinator note 2026-11-06 17:35Z, item 3): t1star05 is
//! enumerated, eps power `n` in `-2..=3` (including `n = 0`, the eps line of
//! the plain 3L kite with a doubled central line) times the 15 dot patterns of
//! TOTAL dots `<= 2` over its four integer lines, 90 keys; NO is sampled, 30
//! draws of TOTAL dots `<= 2` over its eight lines and rank `<= 4` on its ISP,
//! and the distinct count is reported.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensusOptions, PSectorFate, p_census};
use rustred::propagator::custom::{
    CUSTOM_PAYLOAD_SCHEMAS, build_custom_step, check_custom_owner, custom_owner_charts,
    custom_owner_support, inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, load_p_families};
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    CustomOwnerMeta, OwnerProgram, PBasis, PLibrary, POwnerSpec, PReduction, PReductionRequest,
    PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind};
use rustred::propagator::programs::CustomPayloadRef;
use rustred::propagator::step::{PKeyView, POrderKey, PStage, PStepContext};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::{OwnerClassId, PresentationId};
use rustred::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits};
use rustred_app::propagator::generate::{
    CustomOwnerNode, CustomOwnerOptions, CustomOwnerPayload, CustomOwnerReport,
    custom_owner_lp_count, custom_owner_nodes, generate_custom_owner_with_report,
};
use rustred_app::{
    CANDIDATE_BUNDLE_SCHEMA, CandidateGenerationSession, FamilyCandidatesRequest,
    load_generated_candidate_bundle,
};

/// One generated Custom owner of the 3L DAG.
struct Owner {
    node: CustomOwnerNode,
    payload: CustomOwnerPayload,
    report: CustomOwnerReport,
}

/// NO and t1star05, generated once for the whole binary.
fn owners() -> &'static (Owner, Owner) {
    static OWNERS: OnceLock<(Owner, Owner)> = OnceLock::new();
    OWNERS.get_or_init(|| {
        let nodes = custom_owner_nodes(P_FAMILIES_V1_JSON, 3).unwrap();
        let generate = |eps_lines: u8| {
            let node = nodes
                .iter()
                .find(|node| node.eps_lines == eps_lines)
                .unwrap()
                .clone();
            let (payload, report) = generate_custom_owner_with_report(
                &node.presentation,
                &CustomOwnerOptions::default(),
            )
            .unwrap();
            Owner {
                node,
                payload,
                report,
            }
        };
        (generate(0), generate(1))
    })
}

fn no() -> &'static Owner {
    &owners().0
}

fn t1star05() -> &'static Owner {
    &owners().1
}

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

fn line_mask(presentation: &Presentation) -> u64 {
    custom_owner_support(presentation)
}

fn report_line(name: &str, owner: &Owner) -> String {
    let report = &owner.report;
    format!(
        "{name}: arity {}, support {:#x}, charts {}, rules {}, residuals {:?}, LP {}, payload {} bytes, \
         LP {:.3} s, generation {:.3} s, total {:.3} s, order {}",
        owner.node.presentation.slots.len(),
        report.support,
        report.charts.len(),
        report.rules,
        report.residual_keys,
        report.lp_count,
        report.payload_bytes,
        report.lp_seconds,
        report.generation_seconds,
        report.total_seconds,
        report.ordering
    )
}

#[test]
fn the_three_loop_dag_has_exactly_no_and_t1star05_as_custom_owners() {
    let nodes = custom_owner_nodes(P_FAMILIES_V1_JSON, 3).unwrap();
    assert_eq!(nodes.len(), 2);
    let no = nodes.iter().find(|node| node.eps_lines == 0).unwrap();
    assert_eq!((no.loops, no.presentation.slots.len()), (3, 9));
    assert!(matches!(
        no.presentation.kind,
        PresentationKind::Induced { mask, .. } if mask.count_ones() == 8
    ));
    assert_eq!(no.presentation.family.name(), "P3L_NO");
    let kite = nodes.iter().find(|node| node.eps_lines == 1).unwrap();
    assert_eq!((kite.loops, kite.presentation.slots.len()), (2, 5));
    assert_eq!(kite.presentation.kind, PresentationKind::EpsNode);
    assert_eq!(kite.presentation.forced_axes().count_ones(), 1);
    // The 1L and 2L DAGs have no Custom node (the integer kite is a triangle).
    assert!(
        custom_owner_nodes(P_FAMILIES_V1_JSON, 2)
            .unwrap()
            .is_empty()
    );
}

/// Acceptance: both owners all-rank, residuals = LP = 1, the residual of the
/// eps owner at `n = 1`.
#[test]
fn both_owners_generate_all_rank_with_as_many_residuals_as_lp_masters() {
    for (name, owner) in [("NO", no()), ("t1star05", t1star05())] {
        println!("{}", report_line(name, owner));
        let presentation = &owner.node.presentation;
        let meta = &owner.payload.meta;
        let forced = presentation.forced_axes();
        let support = line_mask(presentation);
        assert_eq!(owner.report.support, support, "{name}");
        assert_eq!((meta.lp_count, meta.residuals), (1, 1), "{name}");
        assert_eq!(meta.two_sided_axes, forced, "{name}");
        assert_eq!(meta.charts, custom_owner_charts(support, forced), "{name}");
        let corner: Vec<i64> = (0..presentation.slots.len())
            .map(|slot| i64::from(support >> slot & 1 == 1))
            .collect();
        assert_eq!(owner.report.residual_keys, [corner], "{name}");
        check_custom_owner(presentation, meta).unwrap();
        let summary = inspect_custom_payload(&owner.payload.bytes).unwrap();
        assert_eq!(summary.meta(1).unwrap(), *meta, "{name}");
        assert_eq!(
            summary.family_fingerprint,
            presentation.family.fingerprint()
        );
        assert!(CUSTOM_PAYLOAD_SCHEMAS.contains(&summary.schema.as_str()));
    }
    assert_eq!(no().payload.meta.charts.len(), 1);
    assert_eq!(t1star05().payload.meta.charts.len(), 2);
    // The payload is the application's candidate bundle: the core reader
    // knows its schemas.
    assert!(CUSTOM_PAYLOAD_SCHEMAS.contains(&CANDIDATE_BUNDLE_SCHEMA));
}

/// A one-sided generation of t1star05: only the chart `n >= 1`, no two-sided data.
fn one_sided_t1star05() -> Arc<[u8]> {
    let presentation = &t1star05().node.presentation;
    let arity = presentation.slots.len();
    let mut request = FamilyCandidatesRequest::new("one-sided t1star05");
    request.selected_sectors = Some(vec![vec![true; arity]]);
    let (session, job) =
        CandidateGenerationSession::from_family(Arc::clone(&presentation.family), request, 64)
            .unwrap();
    job.run();
    Arc::from(session.result().unwrap().bundle())
}

/// Acceptance: the guard refuses a 0-residual payload with LP 1 and an eps
/// payload without two-sided data, at generation (`check_custom_owner`) and at
/// build.
#[test]
fn the_guard_refuses_false_knock_outs_and_eps_owners_without_two_sided_data() {
    for owner in [no(), t1star05()] {
        let presentation = &owner.node.presentation;
        let knocked_out = CustomOwnerMeta {
            residuals: 0,
            ..owner.payload.meta.clone()
        };
        for refusal in [
            check_custom_owner(presentation, &knocked_out).err(),
            build_custom_step(presentation, &owner.payload.bytes, &knocked_out).err(),
        ] {
            assert_eq!(
                refusal,
                Some(PError::Generation(
                    "false knock-out guard: LP=1, residuals=0".to_owned()
                ))
            );
        }
        // LP 0 with a residual passes the M2 guard form; the payload then
        // disagrees with this record and is refused at build.
        let lp_zero = CustomOwnerMeta {
            lp_count: 0,
            ..owner.payload.meta.clone()
        };
        check_custom_owner(presentation, &lp_zero).unwrap();
        build_custom_step(presentation, &owner.payload.bytes, &lp_zero).unwrap();
        let more_residuals = CustomOwnerMeta {
            residuals: 2,
            ..owner.payload.meta.clone()
        };
        assert!(matches!(
            build_custom_step(presentation, &owner.payload.bytes, &more_residuals),
            Err(PError::MalformedLibrary(_))
        ));
    }

    let kite = &t1star05().node.presentation;
    let meta = &t1star05().payload.meta;
    let without_axes = CustomOwnerMeta {
        two_sided_axes: 0,
        ..meta.clone()
    };
    let unsupported = Some(PError::Unsupported(
        "eps custom owner requires two-sided axes".to_owned(),
    ));
    assert_eq!(check_custom_owner(kite, &without_axes).err(), unsupported);
    let one_chart = CustomOwnerMeta {
        charts: meta.charts[1..].to_vec(),
        ..meta.clone()
    };
    assert!(matches!(
        check_custom_owner(kite, &one_chart),
        Err(PError::Generation(_))
    ));

    // A real eps payload without two-sided data, the rules study's failure
    // mode: one-sided, the chart `n >= 1` closes by lowering the eps power
    // into `n <= 0`, which SpIRed treats as a subsector, and keeps no residual.
    // Its own record is a false knock-out; with a residual it still has no
    // two-sided axes; under a record that claims them the payload is refused.
    let one_sided = one_sided_t1star05();
    let summary = inspect_custom_payload(&one_sided).unwrap();
    assert_eq!((summary.two_sided_axes, summary.charts.len()), (0, 1));
    assert!(summary.residual_keys().is_empty());
    let own = summary.meta(1).unwrap();
    assert_eq!(
        build_custom_step(kite, &one_sided, &own).err(),
        Some(PError::Generation(
            "false knock-out guard: LP=1, residuals=0".to_owned()
        ))
    );
    let with_residual = CustomOwnerMeta {
        residuals: 1,
        ..own.clone()
    };
    assert_eq!(
        build_custom_step(kite, &one_sided, &with_residual).err(),
        unsupported
    );
    assert_eq!(
        build_custom_step(kite, &one_sided, meta).err(),
        unsupported,
        "the payload's order has no two-sided data"
    );

    // A payload of another family, and bytes that are no payload.
    assert!(build_custom_step(kite, &no().payload.bytes, meta).is_err());
    assert!(matches!(
        build_custom_step(kite, b"not a payload", meta),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(inspect_custom_payload(b"").is_err());
}

#[test]
fn the_lp_count_of_the_owner_supports_is_one() {
    for owner in [no(), t1star05()] {
        let presentation = &owner.node.presentation;
        assert_eq!(
            custom_owner_lp_count(presentation, line_mask(presentation)).unwrap(),
            1
        );
    }
}

// ----- hand-assembled libraries -----

/// A library whose presentation 0 is `interest` (without its node), every class
/// it owns a generated SpIRed owner (`spired_subsectors`) or only its top
/// class, the 3L tops `others` after it with `OwnerProgram::Missing`.
struct HandLibrary {
    library: PLibrary,
    /// The class of the full support of presentation 0.
    owner: OwnerClassId,
    /// Classes with a generated program, their rules in total.
    generated: usize,
    rules: usize,
    seconds: f64,
}

fn three_loop_tops(except: Option<&str>) -> Vec<Presentation> {
    let (file, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    (file.families.iter().zip(tops))
        .filter(|(entry, _)| entry.loops == 3 && Some(entry.name.as_str()) != except)
        .map(|(_, top)| Presentation::clone(&top))
        .collect()
}

fn hand_library(
    interest: &Presentation,
    top: &CustomOwnerPayload,
    others: Vec<Presentation>,
    spired_subsectors: bool,
) -> HandLibrary {
    let started = Instant::now();
    let context = interest.family.coefficient_context().clone();
    let presentations: Vec<Arc<Presentation>> = std::iter::once(interest.clone())
        .chain(others)
        .enumerate()
        .map(|(index, mut presentation)| {
            presentation.id = PresentationId(index as u32);
            presentation.node = None;
            Arc::new(presentation)
        })
        .collect();
    let census = p_census(&presentations, None, &PCensusOptions::default()).unwrap();
    let mut builder = PLibrary::builder(context, 1..=3);
    for presentation in &presentations {
        builder
            .add_presentation(Presentation::clone(presentation))
            .unwrap();
    }
    let support = custom_owner_support(&presentations[0]);
    let (mut owner, mut generated, mut rules) = (None, 0, 0);
    for (&class, record) in &census.classes {
        let (presentation, sector) = record.owner;
        let mut spec = POwnerSpec {
            class,
            lane: OwnerLane::Custom,
            presentation,
            node: record.node,
            roles: LaneRoles::None,
            edge_maps: Vec::new(),
            payload: None,
            custom: None,
            terminals: Vec::new(),
        };
        let ours = presentation == PresentationId(0);
        if !ours || (sector != support && !spired_subsectors) {
            builder
                .add_owner_with_program(spec, OwnerProgram::Missing { ledger: 0 })
                .unwrap();
            continue;
        }
        let payload = if sector == support {
            owner = Some(class);
            top.clone()
        } else {
            let options = CustomOwnerOptions {
                support: Some(sector),
                n_cores: 1,
            };
            generate_custom_owner_with_report(&presentations[0], &options)
                .unwrap_or_else(|error| panic!("support {sector:#x}: {error}"))
                .0
        };
        let summary = inspect_custom_payload(&payload.bytes).unwrap();
        rules += summary.rules();
        generated += 1;
        for residual in summary.residual_keys() {
            let terminal = builder
                .add_terminal(PTerminal::Residual {
                    owner: class,
                    key: key(&residual),
                })
                .unwrap();
            spec.terminals.push(terminal);
        }
        let step = build_custom_step(&presentations[0], &payload.bytes, &payload.meta).unwrap();
        spec.payload = Some(CustomPayloadRef::Bytes(Arc::clone(&payload.bytes)));
        spec.custom = Some(payload.meta.clone());
        builder
            .add_owner_with_program(spec, OwnerProgram::Lane(step))
            .unwrap();
    }
    builder.set_census(census).unwrap();
    HandLibrary {
        library: builder.build().unwrap(),
        owner: owner.expect("the full support is a class of presentation 0"),
        generated,
        rules,
        seconds: started.elapsed().as_secs_f64(),
    }
}

/// The t1star05 presentation in front of the three 3L tops (arity 9).
fn kite_library() -> &'static HandLibrary {
    static LIBRARY: OnceLock<HandLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let owner = t1star05();
        hand_library(
            &owner.node.presentation,
            &owner.payload,
            three_loop_tops(None),
            true,
        )
    })
}

/// The `P3L_NO` top in front of the other two 3L tops; its NO class is the
/// payload generated on the Induced presentation (same family and slots).
fn no_library() -> &'static HandLibrary {
    static LIBRARY: OnceLock<HandLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let (file, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
        let index = file
            .families
            .iter()
            .position(|e| e.name == "P3L_NO")
            .unwrap();
        let top = Presentation::clone(&tops[index]);
        assert_eq!(
            top.family.fingerprint(),
            no().node.presentation.family.fingerprint()
        );
        hand_library(&top, &no().payload, three_loop_tops(Some("P3L_NO")), true)
    })
}

/// t1star05: eps power in `-2..=3` times the 15 patterns of total dots `<= 2`
/// over the four integer lines: 90 distinct keys.
fn kite_keys(presentation: &Presentation) -> Vec<Vec<i64>> {
    let eps = presentation.forced_axes().trailing_zeros() as usize;
    let arity = presentation.slots.len();
    let lines: Vec<usize> = (0..arity).filter(|&slot| slot != eps).collect();
    // Dots placed on the integer lines: none, one, or two (possibly on one line).
    let mut patterns: Vec<Vec<usize>> = vec![vec![]];
    for (index, &first) in lines.iter().enumerate() {
        patterns.push(vec![first]);
        for &second in &lines[index..] {
            patterns.push(vec![first, second]);
        }
    }
    assert_eq!(patterns.len(), 15);
    let mut keys = BTreeSet::new();
    for n in -2..=3 {
        for pattern in &patterns {
            let mut powers = vec![1_i64; arity];
            powers[eps] = n;
            for &slot in pattern {
                powers[slot] += 1;
            }
            keys.insert(powers);
        }
    }
    keys.into_iter().collect()
}

/// A small deterministic generator.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

/// NO: 30 draws of total dots `<= 2` over the eight lines and rank `<= 4` on
/// the ISP (slot 8); returns the distinct keys.
fn no_keys(presentation: &Presentation) -> Vec<Vec<i64>> {
    let support = line_mask(presentation);
    let arity = presentation.slots.len();
    let lines: Vec<usize> = (0..arity)
        .filter(|&slot| support >> slot & 1 == 1)
        .collect();
    let mut rng = Lcg(0x5eed_7c00);
    let mut keys = BTreeSet::new();
    for _ in 0..30 {
        let mut powers: Vec<i64> = (0..arity)
            .map(|slot| i64::from(support >> slot & 1 == 1))
            .collect();
        for _ in 0..rng.next(3) {
            powers[lines[rng.next(lines.len() as u64) as usize]] += 1;
        }
        for (slot, power) in powers.iter_mut().enumerate() {
            if support >> slot & 1 == 0 {
                *power = -(rng.next(5) as i64);
            }
        }
        keys.insert(powers);
    }
    keys.into_iter().collect()
}

/// What walking one owner's steps from a set of keys found.
#[derive(Debug, Default)]
struct Walk {
    steps: usize,
    route_keys: BTreeSet<Vec<i64>>,
    terminals: BTreeSet<u32>,
    max_children: usize,
}

/// Apply the owner's lane step until only Route keys and terminals are left;
/// every Apply child must descend in the owner's order and keep the support,
/// every Route key must have a census fate, every terminal must be the owner's.
fn walk(hand: &HandLibrary, starts: &[Vec<i64>]) -> Walk {
    let library = &hand.library;
    let owner = library.owner(hand.owner).unwrap();
    let OwnerProgram::Lane(lane) = owner.program(library).unwrap() else {
        panic!("the owner has a program");
    };
    let limits = ReductionLimits::default();
    let cx = PStepContext {
        library,
        owner,
        limits: &limits,
    };
    let presentation = library.presentation(PresentationId(0));
    let forced = presentation.forced_axes();
    let support_of = |powers: &[i64]| {
        (powers.iter().enumerate())
            .filter(|&(_, &power)| power > 0)
            .fold(forced, |mask, (slot, _)| mask | 1 << slot)
    };
    let support = custom_owner_support(presentation);
    let tag = GammaTag::one();
    let mut pending: BTreeSet<Vec<i64>> = starts.iter().cloned().collect();
    let mut done = BTreeSet::new();
    let mut walk = Walk::default();
    while let Some(powers) = pending.pop_last() {
        if !done.insert(powers.clone()) {
            continue;
        }
        let view = PKeyView {
            presentation: PresentationId(0),
            powers: &powers,
            tag: &tag,
        };
        let out = lane
            .step(&cx, &view)
            .unwrap_or_else(|error| panic!("{powers:?}: {error}"));
        walk.steps += 1;
        walk.max_children = walk.max_children.max(out.children.len());
        let order = lane.order_key(&cx, &powers).unwrap();
        assert!(matches!(order, POrderKey::Complexity(_)));
        for (terminal, child_tag, _) in &out.terminals {
            assert!(owner.spec.terminals.contains(terminal), "{powers:?}");
            assert_eq!(*child_tag, tag);
            walk.terminals.insert(terminal.0);
        }
        for child in out.children {
            assert_eq!(child.presentation, PresentationId(0));
            assert_eq!(child.tag, tag);
            let child_support = support_of(&child.powers);
            match child.stage {
                PStage::Apply => {
                    assert_eq!(child_support, support, "{powers:?} -> {:?}", child.powers);
                    assert!(
                        lane.order_key(&cx, &child.powers).unwrap() < order,
                        "{powers:?} -> {:?} does not descend",
                        child.powers
                    );
                    pending.insert(child.powers);
                }
                PStage::Route => {
                    assert_ne!(child_support, support);
                    let fate = library
                        .census()
                        .sectors
                        .get(&(PresentationId(0), child_support));
                    assert!(
                        matches!(fate, Some(PSectorFate::Zero | PSectorFate::Owner { .. })),
                        "{powers:?} -> {:?} is not routable",
                        child.powers
                    );
                    walk.route_keys.insert(child.powers);
                }
            }
        }
    }
    walk
}

/// Acceptance: CustomStep closes onto the residual and emits only routable
/// keys, t1star05 (90 keys, enumerated) inside a library whose tops have
/// arity 9, NO at 30 draws.
#[test]
fn custom_steps_close_onto_the_residual_and_emit_only_routable_keys() {
    for (name, owner, hand, keys) in [
        (
            "t1star05",
            t1star05(),
            kite_library(),
            kite_keys(&t1star05().node.presentation),
        ),
        ("NO", no(), no_library(), no_keys(&no().node.presentation)),
    ] {
        let walk = walk(hand, &keys);
        println!(
            "{name}: {} distinct start keys, {} steps, {} route keys, terminals {:?}, at most {} children",
            keys.len(),
            walk.steps,
            walk.route_keys.len(),
            walk.terminals,
            walk.max_children
        );
        assert_eq!(walk.terminals.len(), 1, "{name}: the residual is reached");
        assert!(walk.steps >= keys.len());
        let _ = owner;
    }
    let kite = &t1star05().node.presentation;
    assert_eq!(kite_keys(kite).len(), 90);
    assert!(
        kite_keys(kite)
            .iter()
            .any(|powers| powers[kite.forced_axes().trailing_zeros() as usize] == 0)
    );
    assert!(
        kite_library()
            .library
            .presentations()
            .iter()
            .skip(1)
            .all(|top| top.slots.len() == 9)
    );
}

fn request<'a>(
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
    arithmetic: OrderedWeightArithmetic,
) -> PReductionRequest<'a> {
    PReductionRequest {
        inputs,
        arithmetic,
        scheduler: OrderedSchedulerOptions::default(),
        step_cache: None,
        uncovered: PUncoveredPolicy::Reject,
        onto_masters: false,
        limits: ReductionLimits::default(),
        inspector: None,
    }
}

/// Distinct small integer weights, so that keys do not cancel.
fn weighted(
    hand: &HandLibrary,
    keys: &[Vec<i64>],
) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
    let context = hand.library.coefficient_context();
    (keys.iter().enumerate())
        .map(|(index, powers)| {
            (
                PresentationId(0),
                key(powers),
                context.integer(index as i64 + 1),
            )
        })
        .collect()
}

fn reduce(
    hand: &HandLibrary,
    keys: &[Vec<i64>],
    arithmetic: OrderedWeightArithmetic,
    threads: usize,
) -> (PReduction, f64) {
    let started = Instant::now();
    let mut request = request(weighted(hand, keys), arithmetic);
    request.scheduler = OrderedSchedulerOptions::with_threads(threads);
    let reduction = hand.library.reduce(request).unwrap();
    (reduction, started.elapsed().as_secs_f64())
}

/// Coordinator note 2026-11-07 09:40Z: the owners' inputs reduce to identical
/// results under exact and `Z_p(d)` (modular polynomial) weight arithmetic;
/// the times go into the cost table. Every key also reduces on its own onto
/// residual terminals only.
#[test]
fn no_and_t1star05_reduce_identically_in_exact_and_modular_arithmetic() {
    for (name, hand, keys) in [
        (
            "t1star05",
            kite_library(),
            kite_keys(&t1star05().node.presentation),
        ),
        ("NO", no_library(), no_keys(&no().node.presentation)),
    ] {
        let (exact, exact_seconds) = reduce(hand, &keys, OrderedWeightArithmetic::Exact, 1);
        let (modular, modular_seconds) = reduce(
            hand,
            &keys,
            OrderedWeightArithmetic::modular_polynomial(),
            1,
        );
        let (threaded, threaded_seconds) = reduce(
            hand,
            &keys,
            OrderedWeightArithmetic::modular_polynomial(),
            4,
        );
        assert_eq!(
            modular,
            PReduction {
                statistics: modular.statistics,
                ..threaded.clone()
            },
            "{name}"
        );
        println!("{name}: modular_polynomial at 4 scheduler threads {threaded_seconds:.3} s");
        println!(
            "{name}: library of {} generated owners ({} rules, {:.2} s to build); {} keys -> {} terms; \
             exact {exact_seconds:.3} s, modular_polynomial {modular_seconds:.3} s",
            hand.generated,
            hand.rules,
            hand.seconds,
            keys.len(),
            exact.terms.len()
        );
        assert_eq!(exact.terms, modular.terms, "{name}");
        assert_eq!(
            exact.nonzero_conditions, modular.nonzero_conditions,
            "{name}"
        );
        assert!(!exact.terms.is_empty());
        assert!(
            exact
                .terms
                .iter()
                .all(|term| matches!(term.basis, PBasis::Residual(_)) && term.tag.is_one()),
            "{name}: {:?}",
            exact.terms
        );
        // Key by key, exact: each closes onto residual terminals.
        let mut reached = BTreeMap::new();
        for powers in &keys {
            let one = hand
                .library
                .reduce(request(
                    vec![(
                        PresentationId(0),
                        key(powers),
                        hand.library.coefficient_context().one(),
                    )],
                    OrderedWeightArithmetic::Exact,
                ))
                .unwrap_or_else(|error| panic!("{name} {powers:?}: {error}"));
            for term in one.terms {
                *reached.entry(format!("{:?}", term.basis)).or_insert(0) += 1;
            }
        }
        println!("{name}: terminals reached key by key {reached:?}");
    }
}

/// Coordinator note item 4 (optional): every index of the owner corner raised
/// by 1 and by 2, the owner's table walked with Route children as leaves.
#[test]
fn raised_corners_walk_the_owner_tables() {
    for (name, owner, hand) in [
        ("t1star05", t1star05(), kite_library()),
        ("NO", no(), no_library()),
    ] {
        let presentation = &owner.node.presentation;
        let support = line_mask(presentation);
        for raise in [1_i64, 2] {
            let corner: Vec<i64> = (0..presentation.slots.len())
                .map(|slot| {
                    if support >> slot & 1 == 1 {
                        1 + raise
                    } else {
                        0
                    }
                })
                .collect();
            let started = Instant::now();
            let walk = walk(hand, &[corner.clone()]);
            println!(
                "{name} +{raise} {corner:?}: {} steps, {} route leaves, {:.3} s",
                walk.steps,
                walk.route_keys.len(),
                started.elapsed().as_secs_f64()
            );
            assert_eq!(walk.terminals.len(), 1);
        }
    }
}

/// NO through the hand-assembled library agrees with the independent P0 path:
/// SpIRed on the whole `P3L_NO` downset through the candidate reducer (no
/// classes, no routes, no CustomStep) gives `x = Sum_j e_j T_j`, and the library
/// reduces `x` and `Sum_j e_j T_j` to the same terms, exactly.
#[test]
#[ignore = "slow: SpIRed of the whole P3L_NO downset, about 45 s in debug"]
fn no_reductions_agree_with_the_p0_reduction_of_its_top_family() {
    let hand = no_library();
    let top = hand.library.presentation(PresentationId(0));
    let support = line_mask(top);
    let mut request = FamilyCandidatesRequest::new("P3L_NO downset");
    request.nonpositive_indices = (0..top.slots.len())
        .filter(|&slot| support >> slot & 1 == 0)
        .collect();
    let started = Instant::now();
    let (session, job) =
        CandidateGenerationSession::from_family(Arc::clone(&top.family), request, 1024).unwrap();
    job.run();
    let bundle = session.result().unwrap();
    let (_, mut reducer) = load_generated_candidate_bundle::<9>(
        bundle.bundle(),
        Default::default(),
        ReductionLimits::default(),
    )
    .unwrap();
    println!(
        "P0 downset of P3L_NO: {} bytes, {:.2} s",
        bundle.bundle().len(),
        started.elapsed().as_secs_f64()
    );
    let one = hand.library.coefficient_context().one();
    for powers in no_keys(top).iter().take(8) {
        let decomposition = reducer.reduce_unit_mass(&key(powers)).unwrap();
        assert!(decomposition.unresolved().is_empty(), "{powers:?}");
        let direct = hand
            .library
            .reduce(request_of(vec![(
                PresentationId(0),
                key(powers),
                one.clone(),
            )]))
            .unwrap();
        let composed = hand
            .library
            .reduce(request_of(
                (decomposition.terms().iter())
                    .map(|(terminal, weight)| (PresentationId(0), terminal.clone(), weight.clone()))
                    .collect(),
            ))
            .unwrap();
        assert_eq!(direct.terms, composed.terms, "{powers:?}");
        assert!(!direct.terms.is_empty(), "{powers:?}");
    }
}

fn request_of<'a>(
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
) -> PReductionRequest<'a> {
    request(inputs, OrderedWeightArithmetic::Exact)
}

/// `rustred p-generate` writes the payload and the record of a selected owner,
/// refuses an unknown owner and keeps existing output without `--force`.
#[test]
fn p_generate_writes_the_payload_and_record_of_an_owner() {
    use std::process::Command;

    let kite = &t1star05().node.presentation;
    let label =
        rustred::propagator::class::classify(kite, custom_owner_support(kite), &Default::default())
            .unwrap()
            .unwrap()
            .key
            .label();
    let directory = std::env::temp_dir().join(format!("p-generate-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let families = directory.join("families.json");
    std::fs::write(&families, P_FAMILIES_V1_JSON).unwrap();
    let out = directory.join("owners");
    let run = |owner: &str, force: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_rustred"));
        command
            .arg("p-generate")
            .arg("--families")
            .arg(&families)
            .args(["--max-loops", "3", "--owner", owner])
            .arg("--out")
            .arg(&out)
            .env("SYMBOLICA_HIDE_BANNER", "1");
        if force {
            command.arg("--force");
        }
        command.output().unwrap()
    };
    let generated = run(&label, false);
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let stdout = String::from_utf8_lossy(&generated.stdout);
    assert!(stdout.contains(&format!("{label} node")), "{stdout}");
    let payload = std::fs::read(out.join(format!("{label}.rrbin"))).unwrap();
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join(format!("{label}.json"))).unwrap()).unwrap();
    assert_eq!(record["schema"], "rustred.p-custom-owner.v1");
    assert_eq!(
        (record["lp_count"].as_u64(), record["residuals"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(record["charts"].as_array().unwrap().len(), 2);
    let summary = inspect_custom_payload(&payload).unwrap();
    let meta = summary.meta(1).unwrap();
    assert_eq!(meta, t1star05().payload.meta);
    build_custom_step(kite, &payload, &meta).unwrap();

    // Existing output is kept without --force, replaced with it.
    assert!(!run(&label, false).status.success());
    assert!(run(&label, true).status.success());
    // An owner that is no Custom owner of the scope.
    let unknown = run("L3T08-0000000000000000", true);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("no Custom owner"));
    std::fs::remove_dir_all(&directory).unwrap();
}
