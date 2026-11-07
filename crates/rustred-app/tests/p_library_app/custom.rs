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

use rustred::algebra::{Coefficient, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensusOptions, PSectorFate, p_census};
use rustred::propagator::custom::{
    CUSTOM_PAYLOAD_DUMP_VERSION, CUSTOM_PAYLOAD_SCHEMAS, CustomPower, build_custom_step,
    build_custom_step_prepared, check_custom_owner, custom_owner_charts, custom_owner_support,
    custom_payload_dump, custom_payload_rules, inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, load_p_families};
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    CustomOwnerMeta, OwnerProgram, PBasis, PLibrary, POwnerSpec, PReduction, PReductionRequest,
    PTerm, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot};
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
        // LP 0 with a residual passes the M2 guard form (`lp_count >= 1 =>
        // residuals >= 1`), at generation and at build: the payload does not
        // record the LP count. A residual count the payload does not hold is
        // refused at build.
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
/// residual terminals only (of the owner and of its subsector owners), with
/// tag one.
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
            assert!(!one.terms.is_empty(), "{name} {powers:?}");
            for term in one.terms {
                assert!(
                    matches!(term.basis, PBasis::Residual(_)) && term.tag.is_one(),
                    "{name} {powers:?}: {term:?}"
                );
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
            let walk = walk(hand, std::slice::from_ref(&corner));
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

// ----- independent checks of the values -----
//
// Neither oracle goes through the SpIRed generation: the triangle rule is an
// IBP identity of its own, and a line automorphism maps one key onto another
// whose SpIRed reduction takes another path (the order is not symmetric).

/// The line momenta by slot (`None` on an ISP) with their eps units.
fn lines_of(presentation: &Presentation) -> Vec<Option<(Vec<i64>, u8)>> {
    (presentation.slots.iter())
        .map(|slot| match slot {
            Slot::Line(line) => Some((line.momentum.clone(), line.eps_units)),
            Slot::Isp(_) => None,
        })
        .collect()
}

fn rank(rows: &[Vec<f64>]) -> usize {
    let mut rows = rows.to_vec();
    let columns = rows.first().map_or(0, Vec::len);
    let mut rank = 0;
    for column in 0..columns {
        let Some(pivot) = (rank..rows.len()).find(|&row| rows[row][column].abs() > 1e-9) else {
            continue;
        };
        rows.swap(rank, pivot);
        let pivot_row = rows[rank].clone();
        for (index, row) in rows.iter_mut().enumerate() {
            if index != rank {
                let factor = row[column] / pivot_row[column];
                for (entry, pivot_entry) in row.iter_mut().zip(&pivot_row) {
                    *entry -= factor * pivot_entry;
                }
            }
        }
        rank += 1;
    }
    rank
}

/// The linear map `M` with `M basis[j] = images[j]` (vectors of equal width,
/// `basis` invertible), by Gauss-Jordan; `None` if `basis` is singular.
fn solve(basis: &[Vec<f64>], images: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = basis.len();
    // V has the basis vectors as columns; M = W V^{-1}.
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|row| (0..n).map(|column| basis[column][row]).collect())
        .collect();
    let mut inverse: Vec<Vec<f64>> = (0..n)
        .map(|row| {
            (0..n)
                .map(|column| f64::from(u8::from(row == column)))
                .collect()
        })
        .collect();
    for column in 0..n {
        let pivot =
            (column..n).max_by(|&a, &b| v[a][column].abs().total_cmp(&v[b][column].abs()))?;
        if v[pivot][column].abs() < 1e-9 {
            return None;
        }
        v.swap(column, pivot);
        inverse.swap(column, pivot);
        let scale = v[column][column];
        for entry in 0..n {
            v[column][entry] /= scale;
            inverse[column][entry] /= scale;
        }
        for row in 0..n {
            if row != column {
                let factor = v[row][column];
                for entry in 0..n {
                    v[row][entry] -= factor * v[column][entry];
                    inverse[row][entry] -= factor * inverse[column][entry];
                }
            }
        }
    }
    Some(
        (0..n)
            .map(|row| {
                (0..n)
                    .map(|column| (0..n).map(|k| images[k][row] * inverse[k][column]).sum())
                    .collect()
            })
            .collect(),
    )
}

/// `m v` if it is an integer vector.
fn apply(m: &[Vec<f64>], v: &[i64]) -> Option<Vec<i64>> {
    (m.iter())
        .map(|row| {
            let x: f64 = row.iter().zip(v).map(|(a, &b)| a * b as f64).sum();
            ((x - x.round()).abs() < 1e-9).then_some(x.round() as i64)
        })
        .collect()
}

/// The line permutations (slot to slot, ISPs fixed) induced by integer linear
/// maps of the loop momenta and `q` with `q -> +-q` that map every line onto
/// a line of the same eps units, up to sign: the graph automorphisms that fix
/// the external vertices up to their exchange.
fn line_automorphisms(presentation: &Presentation) -> Vec<Vec<usize>> {
    let lines = lines_of(presentation);
    let width = usize::from(presentation.loops) + 1;
    let line_slots: Vec<usize> = (0..lines.len()).filter(|&s| lines[s].is_some()).collect();
    let momentum = |slot: usize| lines[slot].as_ref().unwrap().0.clone();
    let units = |slot: usize| lines[slot].as_ref().unwrap().1;
    let row = |slot: usize| -> Vec<f64> { momentum(slot).iter().map(|&x| x as f64).collect() };
    // A basis among the lines.
    let mut basis: Vec<usize> = Vec::new();
    for &slot in &line_slots {
        let rows: Vec<Vec<f64>> = basis.iter().chain([&slot]).map(|&s| row(s)).collect();
        if basis.len() < width && rank(&rows) == rows.len() {
            basis.push(slot);
        }
    }
    assert_eq!(basis.len(), width, "the lines span the momenta");
    let basis_rows: Vec<Vec<f64>> = basis.iter().map(|&s| row(s)).collect();
    // Every ordered choice of distinct target lines for the basis, with signs.
    let mut targets: Vec<Vec<usize>> = vec![Vec::new()];
    for _ in 0..width {
        targets = (targets.into_iter())
            .flat_map(|target| {
                (line_slots.iter())
                    .filter(|slot| !target.contains(slot))
                    .map(|&slot| [target.as_slice(), &[slot]].concat())
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    let mut q = vec![0_i64; width];
    q[width - 1] = 1;
    let minus_q: Vec<i64> = q.iter().map(|x| -x).collect();
    let mut found = BTreeSet::new();
    for target in &targets {
        for signs in 0..1_u32 << width {
            let images: Vec<Vec<f64>> = (target.iter().enumerate())
                .map(|(j, &slot)| {
                    let sign = if signs >> j & 1 == 1 { -1.0 } else { 1.0 };
                    row(slot).iter().map(|x| sign * x).collect()
                })
                .collect();
            let Some(m) = solve(&basis_rows, &images) else {
                continue;
            };
            if m.iter().flatten().any(|x| (x - x.round()).abs() > 1e-9)
                || apply(&m, &q).is_none_or(|image| image != q && image != minus_q)
            {
                continue;
            }
            let mut sigma: Vec<usize> = (0..lines.len()).collect();
            let mut used = BTreeSet::new();
            let complete = line_slots.iter().all(|&slot| {
                let Some(image) = apply(&m, &momentum(slot)) else {
                    return false;
                };
                let negative: Vec<i64> = image.iter().map(|x| -x).collect();
                let hit = line_slots.iter().copied().find(|&other| {
                    (momentum(other) == image || momentum(other) == negative)
                        && units(other) == units(slot)
                });
                match hit {
                    Some(other) if used.insert(other) => {
                        sigma[slot] = other;
                        true
                    }
                    _ => false,
                }
            });
            if complete {
                found.insert(sigma);
            }
        }
    }
    found.into_iter().collect()
}

/// The exact reduction of `Sum c_i I(powers_i)` on presentation 0.
fn exact_terms(hand: &HandLibrary, inputs: Vec<(Vec<i64>, Coefficient)>) -> Vec<PTerm> {
    let inputs = (inputs.into_iter())
        .map(|(powers, coefficient)| (PresentationId(0), key(&powers), coefficient))
        .collect();
    hand.library
        .reduce(request(inputs, OrderedWeightArithmetic::Exact))
        .unwrap()
        .terms
}

/// Every key and each of its images under the line automorphisms of
/// presentation 0 reduce to identical terms; returns (automorphisms, pairs).
fn assert_symmetric_reductions(
    name: &str,
    hand: &HandLibrary,
    keys: &[Vec<i64>],
) -> (usize, usize) {
    let presentation = hand.library.presentation(PresentationId(0));
    let automorphisms = line_automorphisms(presentation);
    let one = hand.library.coefficient_context().one();
    let mut pairs = 0;
    for powers in keys {
        let reduced = exact_terms(hand, vec![(powers.clone(), one.clone())]);
        for sigma in &automorphisms {
            let mut image = powers.clone();
            for (slot, &target) in sigma.iter().enumerate() {
                image[target] = powers[slot];
            }
            if &image != powers {
                let other = exact_terms(hand, vec![(image.clone(), one.clone())]);
                assert_eq!(reduced, other, "{name}: {powers:?} and its image {image:?}");
                pairs += 1;
            }
        }
    }
    println!(
        "{name}: {} line automorphisms {automorphisms:?}, {pairs} key pairs agree exactly",
        automorphisms.len()
    );
    (automorphisms.len(), pairs)
}

/// t1star05: the kite's four automorphisms (left-right, up-down) give identical
/// exact reductions on the 90 keys of its domain and their images.
#[test]
fn t1star05_reductions_are_invariant_under_the_kite_automorphisms() {
    let keys = kite_keys(&t1star05().node.presentation);
    assert_eq!(
        assert_symmetric_reductions("t1star05", kite_library(), &keys),
        (4, 216)
    );
}

/// NO: its eight line automorphisms on the keys without ISP (the ISP is no
/// invariant of a line permutation): the corner, one dot and two dots.
#[test]
#[ignore = "slow: about 270 exact NO reductions, about 2 min in debug"]
fn no_reductions_are_invariant_under_its_line_automorphisms() {
    let presentation = &no().node.presentation;
    let support = line_mask(presentation);
    let lines: Vec<usize> = (0..presentation.slots.len())
        .filter(|&slot| support >> slot & 1 == 1)
        .collect();
    let corner: Vec<i64> = (0..presentation.slots.len())
        .map(|slot| i64::from(support >> slot & 1 == 1))
        .collect();
    let mut keys = BTreeSet::from([corner.clone()]);
    for (index, &first) in lines.iter().enumerate() {
        for &second in &lines[index..] {
            let mut one_dot = corner.clone();
            one_dot[first] += 1;
            keys.insert(one_dot.clone());
            one_dot[second] += 1;
            keys.insert(one_dot);
        }
    }
    let keys: Vec<_> = keys.into_iter().collect();
    assert_eq!(keys.len(), 45);
    assert_eq!(
        assert_symmetric_reductions("NO", no_library(), &keys),
        (8, 272)
    );
}

/// The triangles of t1star05 with the eps line `b` as the line of momentum
/// `k`: `(b, a1, c1, a2, c2)` with `a_i = +-(k + p_i)`, `c_i = +-p_i`, where
/// only `b`, `a1`, `a2` carry `k` (`c1`, `c2` and `q` span a complement of it).
fn eps_triangles(presentation: &Presentation) -> Vec<(usize, usize, usize, usize, usize)> {
    let lines = lines_of(presentation);
    let eps = presentation.forced_axes().trailing_zeros() as usize;
    let momentum = |slot: usize| lines[slot].as_ref().unwrap().0.clone();
    let row = |slot: usize| -> Vec<f64> { momentum(slot).iter().map(|&x| x as f64).collect() };
    let others: Vec<usize> = (0..lines.len())
        .filter(|&slot| slot != eps && lines[slot].is_some())
        .collect();
    let width = usize::from(presentation.loops) + 1;
    let mut q = vec![0.0; width];
    q[width - 1] = 1.0;
    let mut found = BTreeSet::new();
    for sign_b in [1, -1] {
        // (a, c): sign_a a - sign_b b = +-c
        let mut pairs = Vec::new();
        for &a in &others {
            for &c in &others {
                for sign_a in [1, -1] {
                    let p: Vec<i64> = (momentum(a).iter().zip(&momentum(eps)))
                        .map(|(x, y)| sign_a * x - sign_b * y)
                        .collect();
                    let minus_p: Vec<i64> = p.iter().map(|x| -x).collect();
                    if a != c && (momentum(c) == p || momentum(c) == minus_p) {
                        pairs.push((a, c));
                    }
                }
            }
        }
        for &(a1, c1) in &pairs {
            for &(a2, c2) in &pairs {
                let distinct = BTreeSet::from([a1, c1, a2, c2]).len() == 4;
                let complement = rank(&[row(c1), row(c2), q.clone()])
                    == usize::from(presentation.loops)
                    && rank(&[row(c1), row(c2), q.clone(), row(eps)]) == width;
                if a1 < a2 && distinct && complement {
                    found.insert((eps, a1, c1, a2, c2));
                }
            }
        }
    }
    found.into_iter().collect()
}

/// The triangle rule (Chetyrkin-Tkachov) with the eps line as apex, an IBP
/// identity independent of the SpIRed generation: with `n_b = n + eps` and
/// `d - 2 eps = 2 d - 4`,
/// `(2 d - 4 - 2 n - n_a1 - n_a2) I = n_a1 [I(a1+, b-) - I(a1+, c1-)] + n_a2 [I(a2+, b-) - I(a2+, c2-)]`.
/// Every term has the same integer power sum, so the C1 sign is common.
fn triangle_rule_residue(
    hand: &HandLibrary,
    (b, a1, c1, a2, c2): (usize, usize, usize, usize, usize),
    powers: &[i64],
    lhs: Coefficient,
) -> Vec<PTerm> {
    let context = hand.library.coefficient_context();
    let shifted = |changes: &[(usize, i64)]| {
        let mut shifted = powers.to_vec();
        for &(slot, delta) in changes {
            shifted[slot] += delta;
        }
        shifted
    };
    let (na1, na2) = (powers[a1], powers[a2]);
    exact_terms(
        hand,
        vec![
            (powers.to_vec(), lhs),
            (shifted(&[(a1, 1), (b, -1)]), context.integer(-na1)),
            (shifted(&[(a1, 1), (c1, -1)]), context.integer(na1)),
            (shifted(&[(a2, 1), (b, -1)]), context.integer(-na2)),
            (shifted(&[(a2, 1), (c2, -1)]), context.integer(na2)),
        ],
    )
}

/// Review round 1, MF1: the triangle rule holds exactly on t1star05's
/// reductions for both triangles at all 90 keys (`n` in `-2..=3`, the `n <= 0`
/// chart included).
#[test]
fn t1star05_obeys_the_triangle_rule_with_the_eps_line_as_apex() {
    let hand = kite_library();
    let presentation = hand.library.presentation(PresentationId(0));
    let context = hand.library.coefficient_context();
    let limits = ExactAlgebraLimits::default();
    let triangles = eps_triangles(presentation);
    println!("t1star05 triangles (b, a1, c1, a2, c2) with the eps apex: {triangles:?}");
    assert_eq!(triangles.len(), 2);
    let two_d = context
        .try_mul(
            &context.integer(2),
            &context.parameter("d").unwrap(),
            limits,
        )
        .unwrap();
    let mut points = 0;
    for &triangle in &triangles {
        let (b, a1, _, a2, _) = triangle;
        for powers in kite_keys(presentation) {
            let lhs = context
                .try_sub(
                    &two_d,
                    &context.integer(4 + 2 * powers[b] + powers[a1] + powers[a2]),
                    limits,
                )
                .unwrap();
            let residue = triangle_rule_residue(hand, triangle, &powers, lhs);
            assert!(
                residue.is_empty(),
                "{triangle:?} at {powers:?}: {residue:?}"
            );
            points += 1;
        }
    }
    assert_eq!(points, 180);
}

/// The triangle-rule check can fail: with the eps part of `n_b` dropped
/// (`d - 4` for `2 d - 6` at the corner) the residue is not zero.
#[test]
fn the_triangle_rule_check_fails_with_a_wrong_coefficient() {
    let hand = kite_library();
    let presentation = hand.library.presentation(PresentationId(0));
    let context = hand.library.coefficient_context();
    let lhs = context
        .try_sub(
            &context.parameter("d").unwrap(),
            &context.integer(4),
            ExactAlgebraLimits::default(),
        )
        .unwrap();
    let corner = vec![1_i64; presentation.slots.len()];
    let triangle = eps_triangles(presentation)[0];
    assert!(!triangle_rule_residue(hand, triangle, &corner, lhs).is_empty());
}

/// Review round 1, MF2: in the library t1star05 is reached below tagged
/// insertion steps. A key with a tag keeps it on every child and on the
/// terminal, and the residual's weight is one.
#[test]
fn tagged_keys_keep_their_tag_and_a_unit_residual_weight() {
    let tag = GammaTag::from_pairs([(1, 2), (-2, -1)]).unwrap();
    for (name, hand, keys) in [
        (
            "t1star05",
            kite_library(),
            kite_keys(&t1star05().node.presentation),
        ),
        ("NO", no_library(), no_keys(&no().node.presentation)),
    ] {
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
        let support = custom_owner_support(presentation);
        let corner: Vec<i64> = (0..presentation.slots.len())
            .map(|slot| i64::from(support >> slot & 1 == 1))
            .collect();
        let (mut children, mut terminals) = (0, 0);
        for powers in keys.iter().chain([&corner]) {
            let view = PKeyView {
                presentation: PresentationId(0),
                powers,
                tag: &tag,
            };
            let out = lane.step(&cx, &view).unwrap();
            for child in &out.children {
                assert_eq!(child.tag, tag, "{name} {powers:?}");
                children += 1;
            }
            for (_, terminal_tag, weight) in &out.terminals {
                assert_eq!(*terminal_tag, tag, "{name} {powers:?}");
                assert_eq!(*weight, library.coefficient_context().one(), "{name}");
                terminals += 1;
            }
        }
        println!("{name}: {children} tagged children, {terminals} tagged terminals");
        assert!(children > 0 && terminals >= 1, "{name}");
    }
}

// ----- the payload's rules as data -----

/// The rules of a payload as data, one per rule of its summary, and its
/// canonical dump; the eager build prepares every chart.
#[test]
fn the_payload_rules_and_dump_list_every_generated_rule() {
    for (name, owner) in [("NO", no()), ("t1star05", t1star05())] {
        let bytes = &owner.payload.bytes;
        let summary = inspect_custom_payload(bytes).unwrap();
        let rules = custom_payload_rules(bytes).unwrap();
        assert_eq!(rules.len(), summary.rules(), "{name}");
        for chart in &summary.charts {
            let ordinals: Vec<usize> = (rules.iter())
                .filter(|rule| rule.chart == chart.chart)
                .map(|rule| rule.ordinal)
                .collect();
            assert_eq!(ordinals, (0..chart.rules).collect::<Vec<_>>(), "{name}");
        }
        for rule in &rules {
            let arity = owner.node.presentation.slots.len();
            assert_eq!(rule.target.len(), arity, "{name}");
            assert!(
                rule.rhs.iter().all(|(powers, coefficient)| {
                    powers.len() == arity && !coefficient.is_empty()
                })
            );
            // The target's fixed slots are the face's.
            for (slot, power) in rule.target.iter().enumerate() {
                let fixed = rule.fixed.iter().find(|&&(axis, _)| axis == slot);
                match (power, fixed) {
                    (CustomPower::Fixed(value), Some(&(_, face))) => assert_eq!(*value, face),
                    (CustomPower::Symbolic(_), None) => {}
                    other => panic!("{name} rule {:#x}/{}: {other:?}", rule.chart, rule.ordinal),
                }
            }
        }
        let dump = custom_payload_dump(bytes).unwrap();
        assert_eq!(dump.lines().next(), Some(CUSTOM_PAYLOAD_DUMP_VERSION));
        assert_eq!(dump, custom_payload_dump(bytes).unwrap());
        assert_eq!(
            dump.lines()
                .filter(|line| line.starts_with("rule "))
                .count(),
            rules.len()
        );
        println!(
            "{name}: {} rules, {} right-hand terms, dump {} lines, first rule:\n{}",
            rules.len(),
            rules.iter().map(|rule| rule.rhs.len()).sum::<usize>(),
            dump.lines().count(),
            dump.lines()
                .skip_while(|line| !line.starts_with("rule "))
                .take(4)
                .collect::<Vec<_>>()
                .join("\n")
        );
        build_custom_step_prepared(&owner.node.presentation, bytes, &owner.payload.meta).unwrap();
    }
    assert!(custom_payload_rules(b"not a payload").is_err());
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
/// refuses an unknown owner and keeps existing output without `--force`; its
/// payload dumps as this process's does.
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
    // The bytes embed the process symbol table (a fresh process generated
    // t1star05 alone, this one after NO); the canonical dumps are equal.
    println!(
        "t1star05 payload: {} bytes from a fresh process, {} bytes here",
        payload.len(),
        t1star05().payload.bytes.len()
    );
    assert_eq!(
        custom_payload_dump(&payload).unwrap(),
        custom_payload_dump(&t1star05().payload.bytes).unwrap()
    );

    // Existing output is kept without --force, replaced with it.
    assert!(!run(&label, false).status.success());
    assert!(run(&label, true).status.success());
    // An owner that is no Custom owner of the scope.
    let unknown = run("L3T08-0000000000000000", true);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("no Custom owner"));
    std::fs::remove_dir_all(&directory).unwrap();
}
