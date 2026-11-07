//! Library assembly and golden inventories (owner: M2-T8a rr-assemble).
//!
//! The scope 1-2 and scope 1-3 library programs assembled from the built-in
//! families (scope 1-3 with the generated Custom owners NO and t1star05):
//! census, owners, lane programs and descent certificate; `p-validate`'s
//! trace on distinct random keys; the reductions of random inputs against P0
//! SpIRed reductions in their top family, in the tag basis (terminal and Gamma
//! tag); closed forms of series-parallel inputs; arithmetic, thread and
//! step-cache invariance; the Triangle fallback; the golden inventories pinned
//! to the library identity. Tests that need 3L reductions in bulk are
//! `#[ignore]` and run optimised (notes/rr-assemble.md).
//!
//! Bounds are TOTAL over the slots of a key: dots over the lines, rank over the
//! numerator slots (lines at negative powers and ISPs). Counts are of distinct
//! keys after deduplication; exactness inputs are distinct up to a scalar.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensus, PSectorFate};
use rustred::propagator::custom::{custom_payload_dump, inspect_custom_payload};
use rustred::propagator::descent::{self, DescentWitness, TraceFailureKind};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::kernel::gamma::FactoredRational;
use rustred::propagator::lane::{LaneRoles, OwnerLane, TriangleRoles};
use rustred::propagator::library::{
    CustomOwnerMeta, OwnerProgram, PBasis, PLibrary, POwnerSpec, PReduction, PReductionRequest,
    PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{EdgeMap, Presentation, PresentationKind, Slot};
use rustred::propagator::programs::{CustomPayloadRef, lane_program};
use rustred::propagator::step::{
    PChild, PKeyView, PLaneStep, POrderKey, PStage, PStepContext, PStepOut,
};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::Dag;
use rustred::propagator::triangle::{TemplateError, TemplateMismatch};
use rustred::propagator::{OwnerClassId, PTerminalId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedStepCache, OrderedWeightArithmetic,
    PolynomialOptions, ReductionLimits,
};
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use rustred_app::propagator::assemble::{
    SplitMix, ValidateOptions, assemble_library_with, unapplied_owners, validate_library,
};
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomOwnerPayload, CustomPayloads,
    assemble_library,
};
use rustred_app::{CandidateGenerationSession, FamilyCandidatesRequest};
use symbolica::domains::rational::{Q, Rational};

#[path = "../../../rustred-core/tests/support/p_integrals.rs"]
mod p_integrals;

use p_integrals::{GValue, PFamily, collect, rational};

// ---------------------------------------------------------------------------
// The assembled library
// ---------------------------------------------------------------------------

fn families() -> PFamiliesFile {
    serde_json::from_str(P_FAMILIES_V1_JSON).unwrap()
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

fn assemble(max_loops: u8) -> AssembledLibrary {
    assemble_library(
        &families(),
        max_loops,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions {
            threads: workers(),
            verify_descent: true,
        },
    )
    .unwrap()
}

/// The scope 1-2 library program, assembled once per test process.
fn scope_two() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| assemble(2))
}

fn lanes_of(assembled: &AssembledLibrary) -> BTreeMap<OwnerLane, usize> {
    let mut lanes = BTreeMap::new();
    for owner in assembled.library.owners() {
        *lanes.entry(owner.spec.lane).or_default() += 1;
    }
    lanes
}

/// The census, owners, edge maps, terminals and certificate of scope 1-2: 2 tops
/// and 6 node presentations (the trivial node has none), 6 classes (P1L's and the
/// five of the 2L DAG), owners SeriesParallel 5 + Triangle 1, no fallback.
#[test]
fn scope_one_to_two_assembles_with_a_strong_census_and_a_descent_certificate() {
    let assembled = scope_two();
    let library = &assembled.library;
    assert_eq!(library.scope(), 1..=2);
    assert_eq!(assembled.tops_of(1), [PresentationId(0)]);
    assert_eq!(assembled.tops_of(2), [PresentationId(1)]);
    assert_eq!(assembled.dag.nodes.len(), 7);
    assert_eq!(library.presentations().len(), 2 + 6);
    assert!(assembled.nodes.iter().all(|node| node.fallback.is_none()));
    assert!(assembled.custom.is_empty());
    assert_eq!(
        lanes_of(assembled),
        BTreeMap::from([(OwnerLane::SeriesParallel, 5), (OwnerLane::Triangle, 1)])
    );
    assert_eq!(library.census().classes.len(), 6);
    assert_eq!(library.terminals(), [PTerminal::Unit]);
    let descent = assembled.descent.as_ref().unwrap();
    assert_eq!(descent.owners, lanes_of(assembled));
    assert_eq!(
        (descent.edge_maps, descent.guarded_owners, descent.overlays),
        (3, 0, 0)
    );
    assert_eq!(
        descent.zero_sectors + descent.owner_sectors + descent.routes,
        library.census().sectors.len()
    );
    // The census and the topology are the ones assembly checked.
    let presentations = library.presentations();
    library
        .census()
        .check_complete(presentations, library.topology().map(Arc::as_ref))
        .unwrap();
    println!(
        "scope 1-2: {} presentations, {} sectors ({} zero, {} owner, {} routed), timings {:?}",
        presentations.len(),
        library.census().sectors.len(),
        descent.zero_sectors,
        descent.owner_sectors,
        descent.routes,
        assembled.timings
    );
}

/// Two assemblies give the same program: presentations, census, owners and edge
/// maps, in the same order.
#[test]
fn assembly_is_deterministic() {
    let again = assemble(2);
    assert_eq!(inventory(&again), inventory(scope_two()));
    assert_eq!(again.nodes, scope_two().nodes);
}

/// Scope 1 holds the 1L top, the bubble and the trivial node only.
#[test]
fn scope_one_is_the_bubble() {
    let assembled = assemble(1);
    assert_eq!(assembled.library.scope(), 1..=1);
    assert_eq!(assembled.library.presentations().len(), 2);
    assert_eq!(
        lanes_of(&assembled),
        BTreeMap::from([(OwnerLane::SeriesParallel, 1)])
    );
    assert!(matches!(
        assemble_library(
            &families(),
            0,
            CustomPayloads::Provided(BTreeMap::new()),
            &AssembleOptions::default()
        ),
        Err(PError::Unsupported(_))
    ));
}

// ---------------------------------------------------------------------------
// The lane-program factory
// ---------------------------------------------------------------------------

fn owner_of(library: &PLibrary, lane: OwnerLane) -> POwnerSpec {
    library
        .owners()
        .find(|owner| owner.spec.lane == lane)
        .unwrap()
        .spec
        .clone()
}

/// Every owner gets the step of its lane; a spec that contradicts its lane, a
/// payload on a rule lane, a lane without a program and a Custom owner without a
/// payload all fail closed.
#[test]
fn the_lane_program_factory_rebuilds_every_owner_and_fails_closed() {
    let library = &scope_two().library;
    for owner in library.owners() {
        let OwnerProgram::Lane(step) = lane_program(library, &owner.spec).unwrap() else {
            panic!("owner {:?} has no lane program", owner.spec.class);
        };
        assert_eq!(step.lane(), owner.spec.lane);
    }
    let sp = owner_of(library, OwnerLane::SeriesParallel);
    let triangle = owner_of(library, OwnerLane::Triangle);

    let mut wrong_roles = sp.clone();
    wrong_roles.roles = triangle.roles;
    assert!(matches!(
        lane_program(library, &wrong_roles),
        Err(PError::Contract(_))
    ));
    let mut wrong_lane = triangle.clone();
    wrong_lane.lane = OwnerLane::Carpet;
    assert!(matches!(
        lane_program(library, &wrong_lane),
        Err(PError::Contract(_))
    ));
    let mut with_payload = sp.clone();
    with_payload.payload = Some(CustomPayloadRef::Bytes(Arc::from(&b"payload"[..])));
    assert!(matches!(
        lane_program(library, &with_payload),
        Err(PError::MalformedLibrary(_))
    ));
    for lane in [OwnerLane::Zero, OwnerLane::Diamond, OwnerLane::Product] {
        let mut no_program = sp.clone();
        no_program.lane = lane;
        assert!(matches!(
            lane_program(library, &no_program),
            Err(PError::MalformedLibrary(_))
        ));
    }
    let mut custom = triangle.clone();
    custom.lane = OwnerLane::Custom;
    custom.roles = LaneRoles::None;
    assert!(matches!(
        lane_program(library, &custom),
        Err(PError::MalformedLibrary(_))
    ));
    // An edge map too many, or none above one loop.
    let mut extra = sp.clone();
    extra.edge_maps.push(0);
    extra.edge_maps.push(1);
    assert!(lane_program(library, &extra).is_err());
}

// ---------------------------------------------------------------------------
// The descent certificate on damaged libraries
// ---------------------------------------------------------------------------

/// Everything a library is built from.
#[derive(Clone)]
struct Parts {
    scope: std::ops::RangeInclusive<u8>,
    presentations: Vec<Presentation>,
    dag: Arc<Dag>,
    census: PCensus,
    edge_maps: Vec<EdgeMap>,
    owners: Vec<POwnerSpec>,
    terminals: Vec<PTerminal>,
}

impl Parts {
    fn of(library: &PLibrary) -> Self {
        Self {
            scope: library.scope(),
            presentations: library
                .presentations()
                .iter()
                .map(|p| Presentation::clone(p))
                .collect(),
            dag: Arc::clone(library.topology().unwrap()),
            census: library.census().clone(),
            edge_maps: library.edge_maps().to_vec(),
            owners: library.owners().map(|owner| owner.spec.clone()).collect(),
            terminals: library.terminals().to_vec(),
        }
    }

    fn build(self) -> Result<PLibrary, PError> {
        let context = self.presentations[0].family.coefficient_context().clone();
        let mut builder = PLibrary::builder(context, self.scope);
        for presentation in self.presentations {
            builder.add_presentation(presentation)?;
        }
        builder.set_topology(self.dag)?;
        builder.set_census(self.census)?;
        for map in self.edge_maps {
            builder.add_edge_map(map)?;
        }
        for terminal in self.terminals {
            builder.add_terminal(terminal)?;
        }
        for owner in self.owners {
            builder.add_owner(owner)?;
        }
        builder.build()
    }

    fn owner_mut(&mut self, lane: OwnerLane) -> &mut POwnerSpec {
        self.owners.iter_mut().find(|o| o.lane == lane).unwrap()
    }
}

fn refused(parts: Parts) -> PError {
    let library = parts.build().unwrap();
    descent::check(&library).unwrap_err()
}

/// The certificate accepts the library rebuilt from its parts and refuses each
/// damage: an edge map named twice or by nobody, a rule owner without its edge
/// (with and without the orphaned edge map),
/// a Triangle owner lowering one line twice or with broken roles, a route onto a
/// class of another `t`, a route that is not a symmetry, and a Custom owner
/// without payload (the guard itself: the next test).
#[test]
fn the_descent_certificate_refuses_damaged_libraries() {
    let library = &scope_two().library;
    let parts = Parts::of(library);
    let rebuilt = parts.clone().build().unwrap();
    assert_eq!(
        descent::check(&rebuilt).unwrap(),
        *scope_two().descent.as_ref().unwrap()
    );

    // An edge map nobody names.
    let mut damaged = parts.clone();
    let orphan = damaged.edge_maps[0].clone();
    damaged.edge_maps.push(orphan);
    assert!(matches!(refused(damaged), PError::MalformedLibrary(m) if m.contains("named by 0")));

    // An edge map named twice: by its owner twice (the builder refuses an
    // owner naming the edge map of another presentation).
    let mut damaged = parts.clone();
    let owner = damaged
        .owners
        .iter_mut()
        .find(|o| !o.edge_maps.is_empty())
        .unwrap();
    owner.edge_maps.push(owner.edge_maps[0]);
    assert!(refused(damaged).to_string().contains("named by 2"));
    let mut damaged = parts.clone();
    let named = damaged
        .owners
        .iter()
        .find(|o| !o.edge_maps.is_empty())
        .unwrap()
        .edge_maps[0];
    let other = damaged
        .owners
        .iter_mut()
        .find(|o| o.lane == OwnerLane::SeriesParallel && o.edge_maps.is_empty())
        .unwrap();
    other.edge_maps.push(named);
    assert!(matches!(damaged.build(), Err(PError::MalformedLibrary(_))));

    // A 2L SP owner that lost its edge.
    let mut damaged = parts.clone();
    let owner = damaged
        .owners
        .iter_mut()
        .find(|o| !o.edge_maps.is_empty())
        .unwrap();
    owner.edge_maps.clear();
    assert!(matches!(refused(damaged), PError::MalformedLibrary(m) if m.contains("named by 0")));
    // ... and one whose edge map is gone with it: the rule-owner check refuses.
    let mut damaged = parts.clone();
    let owner = damaged
        .owners
        .iter_mut()
        .find(|o| o.lane == OwnerLane::SeriesParallel && !o.edge_maps.is_empty())
        .unwrap();
    let edge = owner.edge_maps[0];
    owner.edge_maps.clear();
    damaged.edge_maps.remove(edge);
    for owner in &mut damaged.owners {
        for named in &mut owner.edge_maps {
            if *named > edge {
                *named -= 1;
            }
        }
    }
    // The certificate's own refusal (MalformedLibrary), not the lane program's
    // (the insertion step refuses the same owner with a Contract error).
    let error = refused(damaged);
    assert!(
        matches!(&error, PError::MalformedLibrary(m)
            if m.contains("SeriesParallel owner") && m.contains("0 edge maps at 2 loops")),
        "{error:?}"
    );

    // A Triangle owner that lowers c1 twice, and one whose b is an apex line
    // (the step refuses the roles: its row no longer has the shape).
    let mut damaged = parts.clone();
    let triangle = damaged.owner_mut(OwnerLane::Triangle);
    let LaneRoles::Triangle(mut roles) = triangle.roles else {
        unreachable!()
    };
    roles.b = roles.c[0];
    triangle.roles = LaneRoles::Triangle(roles);
    let error = refused(damaged);
    assert!(error.to_string().contains("b, c1, c2"), "{error}");
    let mut damaged = parts.clone();
    let triangle = damaged.owner_mut(OwnerLane::Triangle);
    let LaneRoles::Triangle(mut roles) = triangle.roles else {
        unreachable!()
    };
    roles.b = roles.a[0];
    triangle.roles = LaneRoles::Triangle(roles);
    assert!(matches!(refused(damaged), PError::Contract(_)));

    // A routed sector whose class has another t.
    let mut damaged = parts.clone();
    let (sector, class) = routed_sector(&damaged.census);
    let routed = &damaged.census.classes[&class];
    let other = damaged
        .census
        .classes
        .iter()
        .find(|(_, record)| record.key.loops == routed.key.loops && record.t != routed.t)
        .map(|(id, _)| *id)
        .unwrap();
    if let Some(PSectorFate::Owner { class, .. }) = damaged.census.sectors.get_mut(&sector) {
        *class = other;
    }
    assert!(matches!(refused(damaged), PError::MalformedLibrary(_)));

    // A route with a wrong external sign.
    let mut damaged = parts.clone();
    let (sector, _) = routed_sector(&damaged.census);
    if let Some(PSectorFate::Owner { map, .. }) = damaged.census.sectors.get_mut(&sector) {
        let mut wrong = map.loop_external.to_vec();
        wrong[0] += 1;
        map.loop_external = wrong.into_boxed_slice();
    }
    let error = refused(damaged);
    assert!(matches!(error, PError::MalformedLibrary(_)), "{error:?}");

    // A Custom owner without a payload.
    let mut damaged = parts.clone();
    let triangle = damaged.owner_mut(OwnerLane::Triangle);
    triangle.lane = OwnerLane::Custom;
    triangle.roles = LaneRoles::None;
    assert!(matches!(refused(damaged), PError::MalformedLibrary(m) if m.contains("no payload")));
}

/// A routed 2L sector that is not an owner sector, and its class.
fn routed_sector(census: &PCensus) -> ((PresentationId, u64), OwnerClassId) {
    census
        .sectors
        .iter()
        .find_map(|(&sector, fate)| match fate {
            PSectorFate::Owner { class, .. }
                if census.classes[class].owner != sector
                    && census.classes[class].key.loops == 2 =>
            {
                Some((sector, *class))
            }
            _ => None,
        })
        .unwrap()
}

/// The hard guard on Custom owners: `lp_count >= 1 => residuals >= 1`, residual
/// terminals as recorded, two-sided axes equal to the forced axes, and every raw
/// chart. Checked on the Triangle owner of scope 1-2 turned into a Custom owner
/// with a payload (its sector has no forced axis: one chart).
#[test]
fn the_false_knock_out_guard_holds_at_the_certificate_and_the_factory() {
    let library = &scope_two().library;
    let parts = Parts::of(library);
    let triangle = owner_of(library, OwnerLane::Triangle);
    let support = library.census().classes[&triangle.class].owner.1;
    let custom = |meta: CustomOwnerMeta, residuals: usize| {
        let mut damaged = parts.clone();
        let key = IntegralKey::try_new(vec![1_i64; 5]).unwrap();
        for index in 0..residuals {
            let mut powers = vec![1_i64; 5];
            powers[0] += index as i64;
            damaged.terminals.push(PTerminal::Residual {
                owner: triangle.class,
                key: IntegralKey::try_new(powers).unwrap(),
            });
        }
        let _ = key;
        let owner = damaged.owner_mut(OwnerLane::Triangle);
        owner.lane = OwnerLane::Custom;
        owner.roles = LaneRoles::None;
        owner.payload = Some(CustomPayloadRef::Bytes(Arc::from(&b"payload"[..])));
        owner.custom = Some(meta);
        owner.terminals = (0..residuals).map(|i| PTerminalId(1 + i as u32)).collect();
        damaged
    };
    let good = CustomOwnerMeta {
        lp_count: 1,
        residuals: 1,
        two_sided_axes: 0,
        charts: vec![support],
    };
    // The guard passes; the bytes are no payload, which the Custom step
    // refuses after the guard (real payloads: the scope 1-3 tests).
    let passing = custom(good.clone(), 1).build().unwrap();
    let spec = passing
        .owners()
        .find(|o| o.spec.lane == OwnerLane::Custom)
        .unwrap()
        .spec
        .clone();
    descent::check_custom_guard(&passing, &spec).unwrap();
    assert!(matches!(
        lane_program(&passing, &spec),
        Err(PError::MalformedLibrary(_))
    ));

    let failing = [
        (
            CustomOwnerMeta {
                residuals: 0,
                ..good.clone()
            },
            0,
            "false knock-out",
        ),
        (good.clone(), 0, "residual terminals"),
        (
            CustomOwnerMeta {
                two_sided_axes: 1,
                ..good.clone()
            },
            1,
            "two-sided",
        ),
        (
            CustomOwnerMeta {
                charts: vec![],
                ..good.clone()
            },
            1,
            "charts",
        ),
        (
            CustomOwnerMeta {
                charts: vec![support, support & !1],
                ..good.clone()
            },
            1,
            "charts",
        ),
    ];
    for (meta, residuals, reason) in failing {
        let library = custom(meta.clone(), residuals).build().unwrap();
        let spec = library
            .owners()
            .find(|o| o.spec.lane == OwnerLane::Custom)
            .unwrap()
            .spec
            .clone();
        for error in [
            descent::check_custom_guard(&library, &spec).unwrap_err(),
            descent::check(&library).unwrap_err(),
            lane_program(&library, &spec).unwrap_err(),
        ] {
            assert!(
                error.to_string().contains(reason),
                "{meta:?}, {residuals} residuals: {error}"
            );
        }
    }
}

/// The guard's charts on an eps owner sector: the eps bubble class of scope 1-2
/// (support `0x3`, forced axis `0x2`: two raw charts) made a Custom owner. Both
/// charts pass in either order; one chart only, a chart twice, an extra chart
/// and one-sided axes are refused by the guard, the certificate and the factory.
#[test]
fn the_guard_needs_both_raw_charts_of_an_eps_owner() {
    let library = &scope_two().library;
    let parts = Parts::of(library);
    let (&class, record) = (library.census().classes.iter())
        .find(|(_, record)| record.eps_lines == 1)
        .unwrap();
    let (presentation, support) = record.owner;
    let forced = library.presentation(presentation).forced_axes();
    assert_eq!((support, forced), (0x3, 0x2), "{}", record.key.label());
    let custom = |two_sided_axes: u64, charts: Vec<u64>| {
        let mut damaged = parts.clone();
        let arity = library.presentation(presentation).slots.len();
        damaged.terminals.push(PTerminal::Residual {
            owner: class,
            key: IntegralKey::try_new(vec![1_i64; arity]).unwrap(),
        });
        let residual = PTerminalId(damaged.terminals.len() as u32 - 1);
        let owner = damaged
            .owners
            .iter_mut()
            .find(|o| o.class == class)
            .unwrap();
        owner.lane = OwnerLane::Custom;
        owner.roles = LaneRoles::None;
        owner.edge_maps.clear();
        owner.payload = Some(CustomPayloadRef::Bytes(Arc::from(&b"payload"[..])));
        owner.custom = Some(CustomOwnerMeta {
            lp_count: 1,
            residuals: 1,
            two_sided_axes,
            charts,
        });
        owner.terminals = vec![residual];
        let built = damaged.build().unwrap();
        let spec = built.owner(class).unwrap().spec.clone();
        (built, spec)
    };
    let (minus, plus) = (support & !forced, support);
    for charts in [vec![minus, plus], vec![plus, minus]] {
        let (built, spec) = custom(forced, charts);
        descent::check_custom_guard(&built, &spec).unwrap();
        // Past the library's guard, the step's own reading: the record lists
        // the charts ascending, and the bytes are no payload.
        assert!(lane_program(&built, &spec).is_err());
    }
    for (two_sided, charts, reason) in [
        (forced, vec![plus], "charts"),
        (forced, vec![minus], "charts"),
        (forced, vec![minus, minus], "charts"),
        (forced, vec![minus, plus, plus | 1 << 7], "charts"),
        (0, vec![minus, plus], "two-sided"),
    ] {
        let (built, spec) = custom(two_sided, charts.clone());
        for error in [
            descent::check_custom_guard(&built, &spec).unwrap_err(),
            descent::check(&built).unwrap_err(),
            lane_program(&built, &spec).unwrap_err(),
        ] {
            assert!(
                error.to_string().contains(reason),
                "{two_sided:#x} {charts:#x?}: {error}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// p-validate: the support-only trace
// ---------------------------------------------------------------------------

/// 10^4 distinct random keys per loop count (the whole box at one loop: it holds
/// fewer), dots <= 3 and rank <= 8 in total: no uncovered key, no missing owner,
/// no step that fails to descend, and every owner applied.
#[test]
fn p_validate_traces_ten_thousand_distinct_keys_per_loop_count() {
    let assembled = scope_two();
    let rows = validate_library(assembled, &ValidateOptions::default()).unwrap();
    for row in &rows {
        let trace = &row.report;
        println!(
            "p-validate L{}: {} distinct keys{}, {} route + {} apply keys ({} zero), {} owners applied, {:.2} s",
            row.loops,
            row.keys,
            if row.box_exhausted {
                " (whole box)"
            } else {
                ""
            },
            trace.route_keys,
            trace.apply_keys,
            trace.zero_keys,
            trace.applied.len(),
            row.seconds
        );
        assert!(trace.is_clean(), "L{}: {:?}", row.loops, trace.samples);
        for kind in [
            TraceFailureKind::Uncovered,
            TraceFailureKind::MissingOwner,
            TraceFailureKind::Descent,
        ] {
            assert_eq!(trace.failures_of(kind), 0);
        }
    }
    assert!(rows[0].box_exhausted);
    assert!(!rows[1].box_exhausted);
    assert_eq!(unapplied_owners(assembled, &rows), []);
    // What the trace reaches is deterministic: (loops, distinct inputs, Route
    // keys, Apply keys, Route keys on zero sectors). A trace that skips keys
    // (e.g. a route's endpoints off the owner support) changes it.
    let reached: Vec<_> = rows
        .iter()
        .map(|row| {
            let trace = &row.report;
            (
                row.loops,
                row.keys,
                trace.route_keys,
                trace.apply_keys,
                trace.zero_keys,
            )
        })
        .collect();
    assert_eq!(
        reached,
        [(1, 127, 127, 10, 117), (2, 10_000, 22_803, 1_686, 19_364)]
    );
}

/// The trace counts what a reduction would refuse: a library whose Triangle
/// owner lost its program reports a missing owner for the kite, which reaches
/// it, and none for a bubble chain, which does not. Descent failures of a lane:
/// `rustred-core`'s `p_library` test `assemble::`.
#[test]
fn the_trace_counts_missing_owners() {
    let library = &scope_two().library;
    let mut parts = Parts::of(library);
    let triangle = parts.owner_mut(OwnerLane::Triangle).clone();
    parts.owners.retain(|o| o.class != triangle.class);
    let context = parts.presentations[0].family.coefficient_context().clone();
    let mut builder = PLibrary::builder(context, parts.scope.clone());
    for presentation in parts.presentations.clone() {
        builder.add_presentation(presentation).unwrap();
    }
    builder.set_topology(Arc::clone(&parts.dag)).unwrap();
    builder.set_census(parts.census.clone()).unwrap();
    for map in parts.edge_maps.clone() {
        builder.add_edge_map(map).unwrap();
    }
    for owner in parts.owners.clone() {
        builder.add_owner(owner).unwrap();
    }
    builder
        .add_owner_with_program(triangle, OwnerProgram::Missing { ledger: 7 })
        .unwrap();
    let missing = builder.build().unwrap();
    assert!(matches!(
        descent::check(&missing),
        Err(PError::MissingOwner { ledger: 7, .. })
    ));
    let kite = (PresentationId(1), vec![1, 1, 1, 1, 1]);
    let bubble = (PresentationId(1), vec![1, 1, 0, 1, 1]);
    let report = descent::trace(
        &missing,
        std::slice::from_ref(&bubble),
        ReductionLimits::default(),
    )
    .unwrap();
    assert!(report.is_clean());
    let report = descent::trace(&missing, &[kite, bubble], ReductionLimits::default()).unwrap();
    assert_eq!(report.failures_of(TraceFailureKind::MissingOwner), 1);
    assert_eq!(report.samples[0].input, 0);
}

/// A scope needs a family at every loop count: scope 1-3 of a file of 1L-2L
/// families is refused at assembly, a loop count without keys at validation,
/// and `rustred p-validate` on such a file exits 5 (derivation), never 0.
#[test]
fn a_scope_needs_a_family_at_every_loop_count() {
    let mut file = families();
    file.families.retain(|entry| entry.loops <= 2);
    let options = AssembleOptions {
        threads: 1,
        verify_descent: true,
    };
    let assemble = |loops| {
        assemble_library(
            &file,
            loops,
            CustomPayloads::Provided(BTreeMap::new()),
            &options,
        )
    };
    let error = assemble(3).unwrap_err();
    assert!(
        matches!(&error, PError::Unsupported(m) if m.contains("no family with 3 loops")),
        "{error}"
    );
    let mut assembled = assemble(2).unwrap();
    assembled.max_loops = 3;
    let error = validate_library(
        &assembled,
        &ValidateOptions {
            points: 50,
            ..ValidateOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        matches!(&error, PError::Unsupported(m) if m.contains("no key of 3 loops")),
        "{error}"
    );

    let mut value: serde_json::Value = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    value["families"]
        .as_array_mut()
        .unwrap()
        .retain(|entry| entry["loops"].as_u64().unwrap() <= 2);
    let directory = std::env::temp_dir().join(format!("rr-assemble-scope-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("families.json");
    std::fs::write(&path, serde_json::to_string(&value).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .arg("p-validate")
        .arg("--families")
        .arg(&path)
        .args(["--max-loops", "3", "--points", "50"])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(5), "{stderr}");
    assert!(stderr.contains("no family with 3 loops"), "{stderr}");
}

/// `rustred p-validate` runs the same trace and prints one line per loop count.
#[test]
fn the_p_validate_command_validates_scope_one_to_two() {
    let output = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(["p-validate", "--max-loops", "2", "--points", "300"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with(&scope_two().library.identity()));
    assert!(stdout.contains("L2: 300 distinct keys"), "{stdout}");
    assert!(stdout.contains("uncovered 0, missing owner 0, descent 0, other 0"));
    let usage = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .args(["p-validate", "--points", "3"])
        .output()
        .unwrap();
    assert!(!usage.status.success());
}

// ---------------------------------------------------------------------------
// The golden inventory
// ---------------------------------------------------------------------------

const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/p_library_app/golden");

fn dump_kind(kind: &PresentationKind) -> String {
    match kind {
        PresentationKind::Top { name } => format!("Top({name})"),
        PresentationKind::Induced { top, mask } => format!("Induced({}, {mask:#x})", top.0),
        other => format!("{other:?}"),
    }
}

fn row(momentum: &[i64]) -> String {
    let entries: Vec<String> = momentum.iter().map(|c| format!("{c:2}")).collect();
    format!("[{}]", entries.join(" "))
}

/// Identity, presentations, classes, owners, edge maps, census counts and
/// terminals of an assembled library: what its program means, without wire ids.
fn inventory(assembled: &AssembledLibrary) -> String {
    use std::fmt::Write as _;
    let library = &assembled.library;
    let mut out = String::new();
    writeln!(out, "identity {}", library.identity()).unwrap();
    for p in library.presentations() {
        let node = p.node.as_ref().map_or("-".to_owned(), |node| {
            node.graph_key.iter().map(|b| format!("{b:02x}")).collect()
        });
        writeln!(
            out,
            "presentation {} L={} {} node {node}",
            p.id.0,
            p.loops,
            dump_kind(&p.kind)
        )
        .unwrap();
        for (slot, s) in p.slots.iter().enumerate() {
            match s {
                Slot::Line(line) => {
                    writeln!(
                        out,
                        "  {slot:2} line {} eps {}",
                        row(&line.momentum),
                        line.eps_units
                    )
                }
                Slot::Isp(isp) => {
                    writeln!(
                        out,
                        "  {slot:2} isp  {} . {}",
                        row(&isp.left),
                        row(&isp.right)
                    )
                }
            }
            .unwrap();
        }
    }
    for node in &assembled.nodes {
        writeln!(
            out,
            "node {} {:?} {:?} presentation {:?} fallback {:?}",
            node.index.0,
            node.decision.lane,
            node.decision.roles,
            node.presentation.map(|p| p.0),
            node.fallback
        )
        .unwrap();
    }
    let census = library.census();
    let (mut zero, mut routed) = (0, 0);
    for fate in census.sectors.values() {
        match fate {
            PSectorFate::Zero => zero += 1,
            PSectorFate::Owner { .. } => routed += 1,
        }
    }
    writeln!(
        out,
        "census {} sectors, {zero} zero, {routed} on classes, {} classes",
        census.sectors.len(),
        census.classes.len()
    )
    .unwrap();
    for (class, record) in &census.classes {
        let owner = library.owner(*class).unwrap();
        writeln!(
            out,
            "class {} owner ({}, {:#x}) node {:?} t {} eps {} lane {:?} roles {:?} edges {:?} terminals {:?}",
            record.key.label(),
            record.owner.0.0,
            record.owner.1,
            record.node.map(|n| n.0),
            record.t,
            record.eps_lines,
            owner.spec.lane,
            owner.spec.roles,
            owner.spec.edge_maps,
            owner.spec.terminals.iter().map(|t| t.0).collect::<Vec<_>>()
        )
        .unwrap();
        if let Some(meta) = &owner.spec.custom {
            writeln!(
                out,
                "  custom lp {} residuals {} two-sided {:#x} charts {:x?}",
                meta.lp_count, meta.residuals, meta.two_sided_axes, meta.charts
            )
            .unwrap();
            // The payload by its content: the canonical dump does not depend
            // on the process symbol table the bytes embed (M2-T7c).
            let bytes = owner.spec.payload.as_ref().unwrap().bytes().unwrap();
            let summary = inspect_custom_payload(&bytes).unwrap();
            let dump = custom_payload_dump(&bytes).unwrap();
            writeln!(
                out,
                "  payload {} rules, residual keys {:?}, support {:#x}, order {}, dump {} lines blake3 {}",
                summary.rules(),
                summary.residual_keys(),
                summary.support,
                summary.ordering,
                dump.lines().count(),
                blake3::hash(dump.as_bytes()).to_hex()
            )
            .unwrap();
        }
    }
    for (index, map) in library.edge_maps().iter().enumerate() {
        writeln!(
            out,
            "edge {index} {} -> {} {:?} loop {} outer {:?} {:?} {} slots {:?} merged {:?}",
            map.parent.0,
            map.child.0,
            map.kind,
            map.integrated_loop,
            map.outer_map.loop_linear,
            map.outer_map.loop_external,
            map.outer_map.external_sign,
            map.line_slots,
            map.merged_slot
        )
        .unwrap();
    }
    for (index, terminal) in library.terminals().iter().enumerate() {
        writeln!(out, "terminal {index} {terminal:?}").unwrap();
    }
    out
}

/// `(identity, blake3 of the inventory)` per scope, append-only: a new inventory
/// needs a new identity, i.e. a bumped version constant. Two pins never share an
/// identity.
const INVENTORY_PINS: &[(&str, &str, &str)] = &[
    (
        "1-2",
        "p-library:scope=1-2:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "c6f122ee6176383cb2acb1883e6ff89b915a4911771dcc400430fece3b294bf2",
    ),
    (
        "1-3",
        "p-library:scope=1-3:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "9dbf8df84155465c41d7a0855703c46690c36f2c2a8f36d6a1276d8e697fc23e",
    ),
];

fn check_inventory(scope: &str, assembled: &AssembledLibrary) {
    let text = inventory(assembled);
    let path = Path::new(GOLDEN_DIR).join(format!("inventory_{scope}.txt"));
    let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
    if std::env::var_os("RUSTRED_P_BLESS").is_some() {
        std::fs::create_dir_all(GOLDEN_DIR).unwrap();
        std::fs::write(&path, &text).unwrap();
        println!("scope {scope} inventory pin {hash}");
        return;
    }
    let identities: BTreeSet<&str> = INVENTORY_PINS.iter().map(|(_, id, _)| *id).collect();
    assert_eq!(
        identities.len(),
        INVENTORY_PINS.len(),
        "two pins share an identity"
    );
    let identity = assembled.library.identity();
    let pinned = INVENTORY_PINS
        .iter()
        .rev()
        .find(|(pin_scope, ..)| *pin_scope == scope)
        .unwrap();
    let golden = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        golden == text,
        "the scope {scope} inventory differs from {}: bump the P_* constant whose meaning changed, \
         re-bless with RUSTRED_P_BLESS=1 and append a pin",
        path.display()
    );
    assert_eq!(
        (pinned.1, pinned.2),
        (identity.as_str(), hash.as_str()),
        "the scope {scope} inventory changed while PLibrary::identity() did not: bump a version"
    );
}

#[test]
fn the_scope_one_to_two_inventory_is_pinned_to_the_identity() {
    check_inventory("1-2", scope_two());
}

// ---------------------------------------------------------------------------
// Reductions: helpers
// ---------------------------------------------------------------------------

fn exact() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
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

fn key(powers: &[i64]) -> IntegralKey {
    IntegralKey::try_new(powers.iter().copied()).unwrap()
}

/// `value` at a rational `d`.
fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

fn coefficient_of(context: &CoefficientContext, value: &Rational) -> Coefficient {
    FactoredRational::from_rational(value.clone())
        .to_coefficient(context, exact())
        .unwrap()
}

/// `count` distinct rational `d` over the prime 101: no integer, no half-integer.
fn dimensions(rng: &mut SplitMix, count: usize) -> Vec<Rational> {
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        let d = rational(
            101 * (1 + rng.below(19) as i64) + 1 + rng.below(100) as i64,
            101,
        );
        if !points.contains(&d) {
            points.push(d);
        }
    }
    points
}

/// A basis element of a reduction onto the terminals: the terminal (`Unit`, or
/// a Custom owner's residual) and the Gamma tag.
type TagBasis = (String, GammaTag);

/// A reduction onto the terminals in the tag basis at `d`: per terminal and
/// tag (the trivial node `Unit`; a residual of NO or t1star05 at scope 1-3).
fn tag_values(reduction: &PReduction, d: &Rational) -> BTreeMap<TagBasis, Rational> {
    let mut values = BTreeMap::new();
    for term in &reduction.terms {
        let entry = values
            .entry((format!("{:?}", term.basis), term.tag.clone()))
            .or_insert_with(Rational::zero);
        *entry = &*entry + &at(&term.coefficient, d);
    }
    values.retain(|_, value: &mut Rational| !value.is_zero());
    values
}

fn add_scaled(
    sum: &mut BTreeMap<TagBasis, Rational>,
    values: &BTreeMap<TagBasis, Rational>,
    factor: &Rational,
) {
    for (basis, value) in values {
        let entry = sum.entry(basis.clone()).or_insert_with(Rational::zero);
        *entry = &*entry + &(value * factor);
    }
    sum.retain(|_, value| !value.is_zero());
}

/// One input: a weighted sum of keys of one top.
#[derive(Clone, Debug)]
struct Input {
    top: PresentationId,
    terms: Vec<(Vec<i64>, Rational)>,
    rank: u32,
}

impl Input {
    fn request_inputs(
        &self,
        context: &CoefficientContext,
    ) -> Vec<(PresentationId, IntegralKey, Coefficient)> {
        self.terms
            .iter()
            .map(|(powers, weight)| (self.top, key(powers), coefficient_of(context, weight)))
            .collect()
    }

    /// The input up to a scalar: its terms in key order, divided by the weight
    /// of the first. Inputs with one normal form have proportional reductions.
    fn normal_form(&self) -> String {
        let mut terms = self.terms.clone();
        terms.sort_by(|a, b| a.0.cmp(&b.0));
        let first = terms[0].1.clone();
        let terms: Vec<_> = terms
            .iter()
            .map(|(powers, weight)| (powers, weight / &first))
            .collect();
        format!("{} {terms:?}", self.top.0)
    }
}

fn small_rational(rng: &mut SplitMix) -> Rational {
    let numerator = 1 + rng.below(9) as i64;
    let numerator = if rng.below(2) == 0 {
        numerator
    } else {
        -numerator
    };
    rational(numerator, 1 + rng.below(4) as i64)
}

fn line_slots(p: &Presentation) -> Vec<usize> {
    (0..p.slots.len())
        .filter(|&s| matches!(p.slots[s], Slot::Line(_)))
        .collect()
}

/// Denominators of a random input: a random subsector of the lines with `dots`
/// extra units; the other slots are numerator slots.
fn denominators(p: &Presentation, dots: u32, rng: &mut SplitMix) -> Vec<i64> {
    let lines = line_slots(p);
    let mut powers = vec![0_i64; p.slots.len()];
    let mut kept = Vec::new();
    for &slot in &lines {
        if rng.below(5) != 0 {
            powers[slot] = 1;
            kept.push(slot);
        }
    }
    if kept.is_empty() {
        let slot = lines[rng.below(lines.len() as u64) as usize];
        powers[slot] = 1;
        kept.push(slot);
    }
    for _ in 0..dots {
        powers[kept[rng.below(kept.len() as u64) as usize]] += 1;
    }
    powers
}

/// A plain key: denominators, and negative powers `-3..-1` on random other
/// slots up to `rank` in total.
fn plain_input(p: &Presentation, rng: &mut SplitMix) -> Input {
    let dots = rng.below(3) as u32;
    let rank = rng.below(7) as u32;
    let mut powers = denominators(p, dots, rng);
    let numerators: Vec<usize> = (0..powers.len()).filter(|&s| powers[s] == 0).collect();
    let mut left = rank;
    while left > 0 && !numerators.is_empty() {
        let slot = numerators[rng.below(numerators.len() as u64) as usize];
        if powers[slot] > -3 {
            powers[slot] -= 1;
            left -= 1;
        } else if numerators.iter().all(|&s| powers[s] == -3) {
            break;
        }
    }
    let rank = numerators.iter().map(|&s| -powers[s]).sum::<i64>() as u32;
    Input {
        top: p.id,
        terms: vec![(powers, small_rational(rng))],
        rank,
    }
}

/// The scalar products of a numerator input: `k_i.k_j` (`i <= j < L`) and
/// `k_i.q` (`j = L`). `k_i.(-q)` is minus `k_i.q`: an input with it is minus
/// one without, up to a scalar the same input, so `q` keeps its sign.
fn scalar_products(p: &Presentation) -> Vec<(usize, usize)> {
    let loops = usize::from(p.loops);
    (0..loops)
        .flat_map(|i| (i..=loops).map(move |j| (i, j)))
        .collect()
}

/// `weight` times the denominators `base` times the scalar products `products`,
/// lowered onto the slots by `express_scalar_product` and expanded.
fn lowered(
    p: &Presentation,
    base: &[i64],
    products: &[(usize, usize)],
    weight: Rational,
) -> Vec<(Vec<i64>, Rational)> {
    let width = usize::from(p.loops) + 1;
    let unit = |column: usize| {
        let mut row = vec![0_i64; width];
        row[column] = 1;
        row
    };
    // Polynomial in the slots: exponents -> coefficient.
    let mut polynomial: BTreeMap<Vec<i64>, Rational> =
        BTreeMap::from([(vec![0_i64; p.slots.len()], weight)]);
    for &(i, j) in products {
        let form = p.express_scalar_product(&unit(i), &unit(j)).unwrap();
        let mut next: BTreeMap<Vec<i64>, Rational> = BTreeMap::new();
        for (exponents, weight) in &polynomial {
            let mut add = |exponents: Vec<i64>, value: Rational| {
                let entry = next.entry(exponents).or_insert_with(Rational::zero);
                *entry = &*entry + &value;
            };
            if !form.constant.is_zero() {
                add(exponents.clone(), weight * &form.constant);
            }
            for (slot, factor) in &form.terms {
                let mut raised = exponents.clone();
                raised[usize::from(*slot)] += 1;
                add(raised, weight * factor);
            }
        }
        next.retain(|_, value| !value.is_zero());
        polynomial = next;
    }
    polynomial
        .into_iter()
        .map(|(exponents, weight)| {
            let powers = base.iter().zip(&exponents).map(|(b, e)| b - e).collect();
            (powers, weight)
        })
        .collect()
}

/// Denominators times `rank` random scalar products.
fn numerator_input(p: &Presentation, rank: u32, rng: &mut SplitMix) -> Input {
    let dots = rng.below(3) as u32;
    let base = denominators(p, dots, rng);
    let types = scalar_products(p);
    let weight = small_rational(rng);
    let products: Vec<(usize, usize)> = (0..rank)
        .map(|_| types[rng.below(types.len() as u64) as usize])
        .collect();
    Input {
        top: p.id,
        terms: lowered(p, &base, &products, weight),
        rank,
    }
}

/// `count` random inputs over the tops of `loops` loops: plain keys (dots <= 2,
/// rank <= 6 as negative powers -3..-1) and numerator inputs (dots <= 2, rank
/// 0..=6 in scalar products), alternately.
fn random_inputs(assembled: &AssembledLibrary, loops: u8, count: usize, seed: u64) -> Vec<Input> {
    let mut rng = SplitMix::new(seed);
    let tops = assembled.tops_of(loops);
    (0..count)
        .map(|index| {
            let top = assembled
                .library
                .presentation(tops[rng.below(tops.len() as u64) as usize]);
            if index % 2 == 0 {
                plain_input(top, &mut rng)
            } else {
                let rank = rng.below(7) as u32;
                numerator_input(top, rank, &mut rng)
            }
        })
        .collect()
}

/// Every nondecreasing sequence of `size` indices below `n`: the multisets.
fn multisets(n: usize, size: usize) -> Vec<Vec<usize>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for shorter in multisets(n, size - 1) {
        for index in shorter.last().copied().unwrap_or(0)..n {
            let mut longer = shorter.clone();
            longer.push(index);
            out.push(longer);
        }
    }
    out
}

/// The bounded input space of `p`: every input [`random_inputs`] can draw on
/// it, one per normal form (weights random), if there are at most `limit`
/// candidates before deduplication, else `None` (it is enumerated at one loop).
fn input_space(p: &Presentation, limit: usize, rng: &mut SplitMix) -> Option<Vec<Input>> {
    let lines = line_slots(p);
    let types = scalar_products(p);
    let numerator_products: Vec<Vec<usize>> = (0..=6)
        .flat_map(|rank| multisets(types.len(), rank))
        .collect();
    let mut bases = Vec::new();
    for mask in 1..1_u64 << lines.len() {
        let kept: Vec<usize> = (0..lines.len())
            .filter(|&i| mask >> i & 1 == 1)
            .map(|i| lines[i])
            .collect();
        for dots in (0..=2).flat_map(|dots| multisets(kept.len(), dots)) {
            let mut base = vec![0_i64; p.slots.len()];
            for &slot in &kept {
                base[slot] = 1;
            }
            for index in dots {
                base[kept[index]] += 1;
            }
            bases.push(base);
        }
    }
    let plain = |base: &[i64]| -> Vec<Vec<i64>> {
        let free: Vec<usize> = (0..base.len()).filter(|&s| base[s] == 0).collect();
        (0..4_u64.pow(free.len() as u32))
            .map(|code| {
                let mut powers = base.to_vec();
                for (position, &slot) in free.iter().enumerate() {
                    powers[slot] = -(((code >> (2 * position)) & 3) as i64);
                }
                powers
            })
            .filter(|powers| free.iter().map(|&s| -powers[s]).sum::<i64>() <= 6)
            .collect()
    };
    let candidates: usize = bases
        .iter()
        .map(|base| plain(base).len() + numerator_products.len())
        .sum();
    if candidates > limit {
        return None;
    }
    let mut space = BTreeMap::new();
    for base in &bases {
        for powers in plain(base) {
            let rank = powers.iter().filter(|&&x| x < 0).map(|x| -x).sum::<i64>() as u32;
            let input = Input {
                top: p.id,
                terms: vec![(powers, small_rational(rng))],
                rank,
            };
            space.entry(input.normal_form()).or_insert(input);
        }
        for product in &numerator_products {
            let products: Vec<(usize, usize)> = product.iter().map(|&t| types[t]).collect();
            let input = Input {
                top: p.id,
                terms: lowered(p, base, &products, small_rational(rng)),
                rank: products.len() as u32,
            };
            space.entry(input.normal_form()).or_insert(input);
        }
    }
    Some(space.into_values().collect())
}

/// A top family under the P0 lane: all-rank SpIRed rules on its lines.
struct P0 {
    lane: DynamicLiteRedSession,
    masters: Vec<IntegralKey>,
}

impl P0 {
    fn new(p: &Presentation) -> Self {
        let root: Vec<bool> = p.slots.iter().map(|s| matches!(s, Slot::Line(_))).collect();
        let options = LiteRedOptions {
            workers: workers(),
            ..LiteRedOptions::default()
        };
        let (mut lane, _) = DynamicLiteRedSession::generate(
            p.family.clone(),
            &root,
            workers(),
            true,
            options,
            None,
        )
        .unwrap();
        lane.compress(&[]).unwrap();
        let masters = lane.masters();
        Self { lane, masters }
    }

    /// Rows of `keys` on the masters at `d`.
    fn rows(&mut self, keys: &[Vec<i64>], d: &Rational) -> Vec<Vec<Rational>> {
        let keys: Vec<IntegralKey> = keys.iter().map(|powers| key(powers)).collect();
        let ids = self.lane.add_targets(&keys).unwrap();
        assert_eq!(self.lane.statistics(&ids).uncovered, 0);
        self.lane
            .reduce_exact_at_point(std::slice::from_ref(d), &ids)
            .unwrap()
            .coefficients
    }
}

fn reduce(
    library: &PLibrary,
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
) -> PReduction {
    library.reduce(request(inputs)).unwrap()
}

// ---------------------------------------------------------------------------
// Exactness against P0 SpIRed reductions in the top family
// ---------------------------------------------------------------------------

/// Random inputs per loop count of the exactness test, distinct up to a scalar:
/// about half of them reduce to zero (zero sectors), so 640 for at least 300
/// nonzero ones.
const EXACT_INPUTS: usize = 640;

/// Candidates up to which the bounded input space of a loop count is
/// enumerated instead of sampled.
const SPACE_LIMIT: usize = 4_096;

/// The exactness inputs of `loops` loops, distinct up to a scalar: the whole
/// bounded input space when it holds at most [`EXACT_INPUTS`] (one loop: the
/// second return value is `true`), else [`EXACT_INPUTS`] random ones.
fn exactness_inputs(assembled: &AssembledLibrary, loops: u8, seed: u64) -> (Vec<Input>, bool) {
    exactness_inputs_of(assembled, loops, seed, EXACT_INPUTS)
}

/// [`exactness_inputs`] with `count` random inputs when the space is sampled.
fn exactness_inputs_of(
    assembled: &AssembledLibrary,
    loops: u8,
    seed: u64,
    count: usize,
) -> (Vec<Input>, bool) {
    let mut rng = SplitMix::new(seed);
    let library = &assembled.library;
    let mut space = Vec::new();
    for &top in &assembled.tops_of(loops) {
        match input_space(library.presentation(top), SPACE_LIMIT, &mut rng) {
            Some(inputs) => space.extend(inputs),
            None => {
                space.clear();
                break;
            }
        }
    }
    if !space.is_empty() && space.len() <= count {
        // The random generator draws inside the enumerated space.
        let forms: BTreeSet<String> = space.iter().map(Input::normal_form).collect();
        for input in random_inputs(assembled, loops, 4 * count, seed) {
            assert!(forms.contains(&input.normal_form()), "{input:?}");
        }
        return (space, true);
    }
    let mut seen = BTreeSet::new();
    let mut inputs = Vec::new();
    for round in 0..64 {
        let draws = random_inputs(assembled, loops, count, seed ^ (round << 32));
        for input in draws {
            if inputs.len() < count && seen.insert(input.normal_form()) {
                inputs.push(input);
            }
        }
        if inputs.len() == count {
            break;
        }
    }
    assert_eq!(
        inputs.len(),
        count,
        "L{loops}: too few inputs distinct up to a scalar"
    );
    (inputs, false)
}

/// `PLib(x) == Sum_j e_j(d) PLib(T_j)` in the tag basis, where `x = Sum_j e_j T_j`
/// is the P0 SpIRed reduction of `x` in its top family, at two random `d` each,
/// on inputs distinct up to a scalar: the whole bounded input space at one loop
/// (177 inputs, 42 of them nonzero), 640 random inputs at two (at least 300 of
/// them nonzero). Per loop count the distinct keys, nonzero comparisons and
/// odd-rank nonzero inputs are counted and bounded below; every owner is applied
/// (counted by the trace of the keys).
/// What one batch of exactness inputs reached.
struct ExactRow {
    /// (loop count, exhausted space, inputs, keys, nonzero comparisons,
    /// nonzero inputs, odd-rank nonzero inputs, max nonzero rank).
    summary: (u8, bool, usize, usize, usize, usize, usize, u32),
    trace: descent::TraceReport,
}

/// `PLib(x) == Sum_j e_j(d) PLib(T_j)` in the tag basis for every input of
/// `inputs` (keys of tops of `loops` loops), `x = Sum_j e_j T_j` the P0
/// reduction in the input's top family, at each of `points`; the keys traced.
fn compare_with_p0(
    assembled: &AssembledLibrary,
    loops: u8,
    inputs: &[Input],
    exhausted: bool,
    points: &[Rational],
) -> ExactRow {
    let library = &assembled.library;
    let context = library.coefficient_context();
    let (mut nonzero, mut nonzero_inputs, mut odd, mut max_rank) = (0, 0, 0, 0);
    let mut all_keys = BTreeSet::new();
    let start = Instant::now();
    for &top in &assembled.tops_of(loops) {
        let ours: Vec<&Input> = inputs.iter().filter(|input| input.top == top).collect();
        if ours.is_empty() {
            continue;
        }
        let mut p0 = P0::new(library.presentation(top));
        let keys: Vec<Vec<i64>> = ours
            .iter()
            .flat_map(|input| input.terms.iter().map(|(powers, _)| powers.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        all_keys.extend(keys.iter().map(|powers| (top, powers.clone())));
        let index: BTreeMap<&Vec<i64>, usize> =
            keys.iter().enumerate().map(|(i, k)| (k, i)).collect();
        let masters: Vec<PReduction> = p0
            .masters
            .clone()
            .into_iter()
            .map(|master| reduce(library, vec![(top, master, context.one())]))
            .collect();
        let reductions: Vec<PReduction> = ours
            .iter()
            .map(|input| reduce(library, input.request_inputs(context)))
            .collect();
        let mut nonzero_of = vec![false; ours.len()];
        for d in points {
            let rows = p0.rows(&keys, d);
            let master_values: Vec<_> = masters.iter().map(|m| tag_values(m, d)).collect();
            for (position, input) in ours.iter().enumerate() {
                let ours_value = tag_values(&reductions[position], d);
                let mut expected = BTreeMap::new();
                for (powers, weight) in &input.terms {
                    for (master, coefficient) in rows[index[powers]].iter().enumerate() {
                        add_scaled(
                            &mut expected,
                            &master_values[master],
                            &(coefficient * weight),
                        );
                    }
                }
                assert_eq!(ours_value, expected, "L{loops} {input:?} at d = {d}");
                if !ours_value.is_empty() {
                    nonzero += 1;
                    nonzero_of[position] = true;
                }
            }
        }
        for (input, _) in ours.iter().zip(&nonzero_of).filter(|(_, n)| **n) {
            nonzero_inputs += 1;
            odd += usize::from(input.rank % 2 == 1);
            max_rank = max_rank.max(input.rank);
        }
    }
    let keys: Vec<_> = all_keys.into_iter().collect();
    let trace = descent::trace(library, &keys, ReductionLimits::default()).unwrap();
    assert!(trace.is_clean(), "{:?}", trace.samples);
    println!(
        "exactness L{loops}: {} inputs distinct up to a scalar{}, {} distinct keys, \
         {nonzero}/{} nonzero comparisons ({} d each), {nonzero_inputs} nonzero inputs \
         ({odd} of odd rank, rank <= {max_rank}); {} apply sites over {} owners: {:?}; \
         {} route keys through q -> -q routes; {:.1} s",
        inputs.len(),
        if exhausted {
            " (the whole bounded input space)"
        } else {
            ""
        },
        keys.len(),
        points.len() * inputs.len(),
        points.len(),
        trace.apply_sites,
        trace.applied.len(),
        trace
            .applied
            .iter()
            .map(|(class, sites)| (library.census().classes[class].key.label(), *sites))
            .collect::<Vec<_>>(),
        trace.flipped_route_keys,
        start.elapsed().as_secs_f64()
    );
    ExactRow {
        summary: (
            loops,
            exhausted,
            inputs.len(),
            keys.len(),
            nonzero,
            nonzero_inputs,
            odd,
            max_rank,
        ),
        trace,
    }
}

/// Routes of `library` that flip `q` (`external_sign = -1`).
fn flipped_routes(library: &PLibrary) -> usize {
    let census = library.census();
    (census.sectors.iter())
        .filter(|(sector, fate)| {
            matches!(fate, PSectorFate::Owner { class, map }
                if map.external_sign < 0 && census.classes[class].owner != **sector)
        })
        .count()
}

#[test]
fn reductions_equal_p0_sector_reductions_in_the_tag_basis() {
    let assembled = scope_two();
    let library = &assembled.library;
    let mut rng = SplitMix::new(0x5ec7_0f2a);
    let points = dimensions(&mut rng, 2);
    // No route of scope 1-2 flips q: `q -> -q` is not traversed here.
    println!("routes with external_sign -1: {}", flipped_routes(library));
    let mut applied = BTreeSet::new();
    let mut summary = Vec::new();
    for loops in 1..=2_u8 {
        let (inputs, exhausted) = exactness_inputs(assembled, loops, 0x1000 + u64::from(loops));
        let row = compare_with_p0(assembled, loops, &inputs, exhausted, &points);
        applied.extend(row.trace.applied.keys().copied());
        summary.push(row.summary);
    }
    println!("exactness rows: {summary:?}");
    let [one, two] = summary.as_slice() else {
        unreachable!()
    };
    // One loop: the bounded input space, exhausted (deterministic up to the
    // weights): (loops, exhausted, inputs, keys, nonzero comparisons, nonzero
    // inputs, odd-rank nonzero inputs, max nonzero rank).
    assert_eq!(*one, (1, true, 177, 69, 84, 42, 18, 6));
    // Two loops: random inputs; lower bounds on what they reach.
    let &(_, exhausted, inputs, keys, nonzero, nonzero_inputs, odd, max_rank) = two;
    assert!(!exhausted && inputs == EXACT_INPUTS);
    assert!(keys >= 2_000, "L2: {keys} keys");
    assert!(nonzero >= 600, "L2: {nonzero} nonzero comparisons");
    assert!(nonzero_inputs >= 300, "L2: {nonzero_inputs} nonzero inputs");
    assert!(
        odd >= 100 && max_rank == 6,
        "L2: {odd} odd, rank {max_rank}"
    );
    assert_eq!(
        applied.len(),
        library.owners().len(),
        "an owner never applied"
    );
}

// ---------------------------------------------------------------------------
// Series-parallel inputs and their closed forms
// ---------------------------------------------------------------------------

/// `Gamma(c + m d/2)` for `m != 0` as `Pochhammer * Gamma(m d/2)`.
fn gamma_value(c: i64, m: i64, d: &Rational) -> GValue {
    let x = &(d * &rational(m, 2));
    let mut coefficient = Rational::one();
    for j in 0..c {
        coefficient = &coefficient * &(x + &rational(j, 1));
    }
    for j in 1..=-c {
        coefficient = &coefficient / &(x - &rational(j, 1));
    }
    GValue {
        coefficient,
        signature: BTreeMap::from([(m, 1)]),
    }
}

/// The value of a tag at `d`, with `1 + m eps = 1 + 2 m - m d/2`.
fn tag_value(tag: &GammaTag, d: &Rational) -> GValue {
    let mut value = GValue::rational(Rational::one());
    for &(m, e) in tag.exponents() {
        let (m, e) = (i64::from(m), i64::from(e));
        let base = gamma_value(1 + 2 * m, -m, d);
        let factor = if e > 0 {
            base
        } else {
            GValue {
                coefficient: base.coefficient.inv(),
                signature: base.signature.iter().map(|(m, e)| (*m, -e)).collect(),
            }
        };
        for _ in 0..e.abs() {
            value = value.mul(&factor);
        }
    }
    value
}

/// The test family of a top presentation: its line momenta (keys of it have
/// their ISPs at power 0 and are given by their line powers).
fn closed_form_family(p: &Presentation) -> PFamily {
    let lines: Vec<&'static [i64]> = (p.slots.iter())
        .filter_map(|slot| match slot {
            Slot::Line(line) => Some(&*Box::leak(line.momentum.clone().into_boxed_slice())),
            Slot::Isp(_) => None,
        })
        .collect();
    let count = lines.len();
    PFamily {
        name: "top",
        loops: usize::from(p.loops),
        lines: Box::leak(lines.into_boxed_slice()),
        root: Box::leak(vec![true; count].into_boxed_slice()),
    }
}

/// Every key of the 1L and 2L tops with powers in -2..=3 that is a chain of
/// one-loop insertions (its value is a product of G-functions) equals that
/// value: `I_M = (-1)^{Sum n} I_E`, at two random `d`.
#[test]
fn series_parallel_inputs_equal_their_closed_forms() {
    let assembled = scope_two();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let mut rng = SplitMix::new(0x00c1_05ed);
    let points = dimensions(&mut rng, 2);
    for (top, budget) in [(PresentationId(0), usize::MAX), (PresentationId(1), 400)] {
        let p = library.presentation(top);
        let family = closed_form_family(p);
        let mut keys = Vec::new();
        let width = p.slots.len() as u32;
        for code in 0..6_u64.pow(width) {
            let powers: Vec<i64> = (0..width)
                .map(|slot| (code / 6_u64.pow(slot) % 6) as i64 - 2)
                .collect();
            let sum: i64 = powers.iter().map(|power| power.abs()).sum();
            if powers.iter().all(|&power| power > 0) && p.loops == 2 || sum > 9 {
                continue; // the kite itself is no chain
            }
            if family.value(&key(&powers), &points[0]).is_some() {
                keys.push(powers);
            }
        }
        let mut sampled = Vec::new();
        let target = budget.min(keys.len());
        while sampled.len() < target {
            let index = rng.below(keys.len() as u64) as usize;
            sampled.push(keys.swap_remove(index));
        }
        let mut nonzero = 0;
        for powers in &sampled {
            let reduction = reduce(library, vec![(top, key(powers), context.one())]);
            let sign = if powers.iter().sum::<i64>() % 2 == 0 {
                1
            } else {
                -1
            };
            for d in &points {
                let ours: Vec<GValue> = reduction
                    .terms
                    .iter()
                    .map(|term| {
                        tag_value(&term.tag, d)
                            .scale(&(&at(&term.coefficient, d) * &rational(sign, 1)))
                    })
                    .collect();
                let expected = family.value(&key(powers), d).unwrap();
                let difference = ours
                    .iter()
                    .cloned()
                    .chain([expected.scale(&rational(-1, 1))]);
                assert!(
                    collect(difference).is_empty(),
                    "{top:?} {powers:?} at d = {d}"
                );
                nonzero += usize::from(!expected.is_zero());
            }
        }
        println!(
            "closed forms on presentation {}: {} distinct chain keys, {nonzero} nonzero values",
            top.0,
            sampled.len()
        );
        assert!(nonzero > 0);
    }
}

// ---------------------------------------------------------------------------
// Arithmetic, threads, step cache
// ---------------------------------------------------------------------------

/// Exact and ModularPolynomial weights give the same terms, and 1 and 16
/// scheduler threads the same bits, on 60 random inputs per loop count, also
/// modular with 4 threads under 16 scheduler threads (vakint's default shape);
/// the wall time of both arithmetics is printed.
#[test]
fn exact_equals_modular_polynomial_and_threads_are_bit_identical() {
    let assembled = scope_two();
    let library = &assembled.library;
    let context = library.coefficient_context();
    for loops in 1..=2_u8 {
        let inputs = random_inputs(assembled, loops, 60, 0x2000 + u64::from(loops));
        let mut seconds = [0.0_f64; 2];
        for input in &inputs {
            let start = Instant::now();
            let exact = reduce(library, input.request_inputs(context));
            seconds[0] += start.elapsed().as_secs_f64();
            let start = Instant::now();
            let mut modular = request(input.request_inputs(context));
            modular.arithmetic = modular_polynomial(1);
            let modular = library.reduce(modular).unwrap();
            seconds[1] += start.elapsed().as_secs_f64();
            assert_eq!(exact.terms, modular.terms, "{input:?}");
            assert_eq!(exact.nonzero_conditions, modular.nonzero_conditions);
            let mut threaded = request(input.request_inputs(context));
            threaded.scheduler = OrderedSchedulerOptions {
                threads: 16,
                window_keys: 8,
                ..Default::default()
            };
            let threaded = library.reduce(threaded).unwrap();
            assert_eq!(exact.terms, threaded.terms);
            assert_eq!(exact.nonzero_conditions, threaded.nonzero_conditions);
            // vakint's shape: modular with several threads under a threaded
            // scheduler.
            let mut both = request(input.request_inputs(context));
            both.arithmetic = modular_polynomial(4);
            both.scheduler = OrderedSchedulerOptions {
                threads: 16,
                window_keys: 8,
                ..Default::default()
            };
            let both = library.reduce(both).unwrap();
            assert_eq!(exact.terms, both.terms, "{input:?}");
            assert_eq!(exact.nonzero_conditions, both.nonzero_conditions);
        }
        println!(
            "L{loops}: {} inputs, Exact {:.3} s, ModularPolynomial {:.3} s",
            inputs.len(),
            seconds[0],
            seconds[1]
        );
    }
}

/// One cache serves the program: a second 2L input that meets the first one's
/// subkeys at the same absolute tag replays them, with the result of an uncached
/// run. A 1L input after them meets the bubble only under the tag of the 2L
/// steps: no replay (plan amendment 2026-11-07 09:10Z), but the same result.
#[test]
fn the_step_cache_replays_records_where_the_absolute_tag_matches() {
    let library = &scope_two().library;
    let context = library.coefficient_context();
    let cache =
        OrderedStepCache::in_memory(library.step_cache_binding().unwrap(), Default::default());
    let cached = |powers: &[i64], top: PresentationId| {
        let mut run = request(vec![(top, key(powers), context.one())]);
        run.step_cache = Some(Arc::clone(&cache));
        library.reduce(run).unwrap()
    };
    let first = cached(&[1, 1, 1, 1, 1], PresentationId(1));
    assert_eq!(first.statistics.progress.step_cache_hits, 0);
    let second = cached(&[2, 1, 1, 1, 1], PresentationId(1));
    let uncached = reduce(
        library,
        vec![(PresentationId(1), key(&[2, 1, 1, 1, 1]), context.one())],
    );
    assert_eq!(second.terms, uncached.terms);
    let progress = second.statistics.progress;
    assert!(progress.step_cache_hits > 0, "{progress:?}");
    let bubble = cached(&[1, 1], PresentationId(0));
    let uncached = reduce(
        library,
        vec![(PresentationId(0), key(&[1, 1]), context.one())],
    );
    assert_eq!(bubble.terms, uncached.terms);
    println!(
        "second 2L input: {} hits, {} misses; 1L input after them: {} hits, {} misses",
        progress.step_cache_hits,
        progress.step_cache_misses,
        bubble.statistics.progress.step_cache_hits,
        bubble.statistics.progress.step_cache_misses
    );
}

// ---------------------------------------------------------------------------
// The benchmark cells (vakint `benchmark_p_integrals.json`, 1L and 2L)
// ---------------------------------------------------------------------------

/// The numerator factors of the benchmark cells of `loops` loops, in order:
/// `(i, j)` is `k_i.k_j`, `(i, 0)` is `k_i.q`; rank `r` takes the shortest prefix
/// of rank `r`.
fn benchmark_factors(loops: u8) -> &'static [(usize, usize)] {
    match loops {
        1 => &[(1, 0); 8],
        2 => &[(1, 0), (2, 0), (1, 2), (1, 0), (2, 0), (1, 2)],
        _ => &[(1, 0), (2, 0), (1, 3), (2, 0), (3, 0), (1, 2)],
    }
}

/// The top powers of a cell: `dots` 0, 1, 2 on the first and last line.
fn benchmark_powers(p: &Presentation, dots: u32) -> Vec<i64> {
    let lines = p
        .slots
        .iter()
        .filter(|s| matches!(s, Slot::Line(_)))
        .count();
    let mut powers: Vec<i64> = (0..p.slots.len()).map(|s| i64::from(s < lines)).collect();
    if dots >= 1 {
        powers[0] += 1;
    }
    if dots >= 2 {
        powers[lines - 1] += 1;
    }
    powers
}

/// The input of one benchmark cell: the top with its dots times the numerator
/// of rank `rank`, lowered onto the slots.
fn benchmark_input(p: &Presentation, dots: u32, rank: u32) -> Vec<(Vec<i64>, Rational)> {
    let base = benchmark_powers(p, dots);
    let width = usize::from(p.loops) + 1;
    let column = |index: usize| {
        let mut row = vec![0_i64; width];
        row[if index == 0 { width - 1 } else { index - 1 }] = 1;
        row
    };
    let mut polynomial: BTreeMap<Vec<i64>, Rational> =
        BTreeMap::from([(vec![0_i64; p.slots.len()], Rational::one())]);
    let mut left = rank;
    for &(i, j) in benchmark_factors(p.loops) {
        let weight = if j == 0 { 1 } else { 2 };
        if left < weight {
            break;
        }
        left -= weight;
        let form = p.express_scalar_product(&column(i), &column(j)).unwrap();
        let mut next: BTreeMap<Vec<i64>, Rational> = BTreeMap::new();
        for (exponents, value) in &polynomial {
            let mut add = |exponents: Vec<i64>, term: Rational| {
                let entry = next.entry(exponents).or_insert_with(Rational::zero);
                *entry = &*entry + &term;
            };
            if !form.constant.is_zero() {
                add(exponents.clone(), value * &form.constant);
            }
            for (slot, factor) in &form.terms {
                let mut raised = exponents.clone();
                raised[usize::from(*slot)] += 1;
                add(raised, value * factor);
            }
        }
        next.retain(|_, value| !value.is_zero());
        polynomial = next;
    }
    polynomial
        .into_iter()
        .map(|(exponents, value)| {
            (
                base.iter().zip(&exponents).map(|(b, e)| b - e).collect(),
                value,
            )
        })
        .collect()
}

/// Slots of one recorded identity: stage byte, class (8 bytes), then the padded
/// key of the library (D-T6-3).
fn identity_bytes(library: &PLibrary) -> usize {
    let arity = library
        .presentations()
        .iter()
        .map(|p| p.slots.len())
        .max()
        .unwrap();
    9 + (arity + 16).max(19)
}

/// The 1L-3L benchmark cells in the scope 1-3 library (the program vakint
/// ships): distinct tags per Apply site (plan risk 12); cold and warm wall
/// times under Exact (step cache on disk, warm = reopened) and
/// ModularPolynomial weights (in memory); the bytes of the saved records and
/// the share of identities in them (a shard file stores each identity once:
/// the tagged trace counts the distinct identities per shard), the rest being
/// coefficients. Numerators over each family's own `k` columns.
/// Measurements, printed; run optimised.
#[test]
#[ignore = "slow: benchmark cells at rank 8, measured optimised"]
fn benchmark_cells_tags_and_timings() {
    let assembled = scope_three();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let binding = library.step_cache_binding().unwrap();
    let tagged = descent::TraceOptions {
        limits: ReductionLimits::default(),
        tags: true,
    };
    for loops in 1..=3_u8 {
        let mut all_keys = Vec::new();
        let (mut record_bytes, mut identities) = (0_u64, 0_usize);
        // Cold wall seconds of the cells: Exact, ModularPolynomial.
        let mut cold = [0.0_f64; 2];
        for dots in 0..=2 {
            for rank in [0, 2, 4, 6, 8] {
                let mut inputs = Vec::new();
                for top in assembled.tops_of(loops) {
                    let terms = benchmark_input(library.presentation(top), dots, rank);
                    all_keys.extend(terms.iter().map(|(powers, _)| (top, powers.clone())));
                    inputs.extend(Input { top, terms, rank }.request_inputs(context));
                }
                let keys: Vec<_> = inputs
                    .iter()
                    .map(|(top, key, _)| (*top, key.powers().to_vec()))
                    .collect();
                let tags = descent::trace_with(library, &keys, tagged).unwrap();
                let cell_identities = tags.shard_identities;
                let mut line = format!(
                    "L{loops}_d{dots}_r{rank}: {} terms, {} apply keys / {} sites = {:.3} tags per site",
                    inputs.len(),
                    tags.apply_keys,
                    tags.apply_sites,
                    tags.apply_keys as f64 / tags.apply_sites.max(1) as f64
                );
                let directory = std::env::temp_dir().join(format!(
                    "rr-assemble-bench-{}-L{loops}-d{dots}-r{rank}",
                    std::process::id()
                ));
                let mut results = Vec::new();
                for state in ["cold", "warm"] {
                    let cache =
                        OrderedStepCache::open(&directory, binding.clone(), Default::default())
                            .unwrap();
                    let mut run = request(inputs.clone());
                    run.step_cache = Some(Arc::clone(&cache));
                    let start = Instant::now();
                    let reduction = library.reduce(run).unwrap();
                    let seconds = start.elapsed().as_secs_f64();
                    line += &format!(", exact {state} {seconds:.4} s");
                    if state == "cold" {
                        cold[0] += seconds;
                        let saved = cache.save().unwrap();
                        line += &format!(" ({} records, {} bytes)", saved.records, saved.bytes);
                        record_bytes += saved.bytes;
                        identities += cell_identities;
                    } else {
                        assert_eq!(reduction.statistics.progress.step_cache_misses, 0);
                    }
                    results.push(reduction.terms);
                }
                std::fs::remove_dir_all(&directory).unwrap();
                let cache = OrderedStepCache::in_memory(binding.clone(), Default::default());
                for state in ["cold", "warm"] {
                    let mut run = request(inputs.clone());
                    run.arithmetic = modular_polynomial(1);
                    run.step_cache = Some(Arc::clone(&cache));
                    let start = Instant::now();
                    let reduction = library.reduce(run).unwrap();
                    let seconds = start.elapsed().as_secs_f64();
                    line += &format!(", modular {state} {seconds:.4} s");
                    if state == "cold" {
                        cold[1] += seconds;
                    }
                    results.push(reduction.terms);
                }
                assert!(results.windows(2).all(|pair| pair[0] == pair[1]));
                println!("{line}");
            }
        }
        let tags = descent::trace_with(library, &all_keys, tagged).unwrap();
        let identity_share =
            (identities * identity_bytes(library)) as f64 / record_bytes.max(1) as f64;
        println!(
            "L{loops} all cells: {} apply keys / {} sites = {:.3} tags per site; records {record_bytes} bytes, \
             identities about {:.0}% of them ({identities} identities of {} bytes), coefficients about {:.0}%; \
             cold Exact {:.3} s, cold ModularPolynomial {:.3} s",
            tags.apply_keys,
            tags.apply_sites,
            tags.apply_keys as f64 / tags.apply_sites.max(1) as f64,
            100.0 * identity_share,
            identity_bytes(library),
            100.0 * (1.0 - identity_share),
            cold[0],
            cold[1]
        );
    }
}

/// A Custom payload can be provided verbatim by class label; at scope 1-2 there
/// is no Custom owner, so nothing is asked for.
#[test]
fn provided_payloads_are_not_needed_at_scope_one_to_two() {
    let provided: BTreeMap<String, CustomOwnerPayload> = BTreeMap::new();
    let assembled = assemble_library(
        &families(),
        2,
        CustomPayloads::Provided(provided),
        &AssembleOptions {
            threads: 1,
            verify_descent: false,
        },
    )
    .unwrap();
    assert!(assembled.descent.is_none());
    assert_eq!(inventory(&assembled), inventory(scope_two()));
}

/// `rustred p-census` keeps a copy of the assembly loop (notes/rr-census.md,
/// SF5): its census of scope 1-2 is the census of the assembled library, byte
/// for byte in the JSON form.
#[test]
fn the_p_census_command_writes_the_census_of_the_assembly() {
    let assembled = scope_two();
    let directory = std::env::temp_dir().join(format!("rr-assemble-census-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let (input, out) = (
        directory.join("families.json"),
        directory.join("census.json"),
    );
    std::fs::write(&input, P_FAMILIES_V1_JSON).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_rustred"))
        .arg("p-census")
        .arg("--families")
        .arg(&input)
        .args(["--max-loops", "2", "--force", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let library = &assembled.library;
    let expected = library.census().to_json(library.presentations()).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), expected);
    std::fs::remove_dir_all(&directory).unwrap();
}

// ---------------------------------------------------------------------------
// Scope 1-3: the Custom owners NO and t1star05 (stage 2)
// ---------------------------------------------------------------------------

/// The scope 1-3 library program (the built-in release scope) with its two
/// Custom owners generated, assembled once per test process.
fn scope_three() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| assemble(3))
}

/// Class labels of the Custom owners (M2-T7c; `P_CLASS_KEY_VERSION` 1): NO
/// (`nostar0`) and the eps kite t1star05.
const NO_LABEL: &str = "L3T08-61cd69124cf81eb3";
const T1STAR05_LABEL: &str = "L2T05-810c5f0ff11c4c40";

fn label_of(library: &PLibrary, class: OwnerClassId) -> String {
    library.census().classes[&class].key.label()
}

fn owner_labelled(library: &PLibrary, label: &str) -> POwnerSpec {
    library
        .owners()
        .find(|owner| label_of(library, owner.spec.class) == label)
        .unwrap()
        .spec
        .clone()
}

/// The lanes of the DAG nodes (the trivial node as `Trivial`).
fn node_lanes(assembled: &AssembledLibrary) -> BTreeMap<String, usize> {
    let mut lanes = BTreeMap::new();
    for node in &assembled.nodes {
        let lane = if node.decision.trivial {
            "Trivial".to_owned()
        } else {
            format!("{:?}", node.decision.lane)
        };
        *lanes.entry(lane).or_default() += 1;
    }
    lanes
}

/// Gate G2 lanes over the 43 nodes of the 3L DAG, the two Custom owners on
/// their presentations (D7: NO on the Induced presentation of `P3L_NO`, the
/// eps kite on its EpsNode presentation), one residual terminal each at the
/// owner's corner, a strong census and a descent certificate whose Custom
/// part (items 1, 2, 7) reads both payloads and steps both witness boxes.
#[test]
fn scope_one_to_three_assembles_with_custom_owners_and_a_descent_certificate() {
    let assembled = scope_three();
    let library = &assembled.library;
    assert_eq!(library.scope(), 1..=3);
    let names: Vec<String> = (0..5)
        .map(|id| match &library.presentation(PresentationId(id)).kind {
            PresentationKind::Top { name } => name.clone(),
            other => panic!("presentation {id} is {other:?}"),
        })
        .collect();
    assert_eq!(names, ["P1L", "P2L", "P3L_LA", "P3L_BE", "P3L_NO"]);
    assert_eq!(
        assembled.tops_of(3),
        [PresentationId(2), PresentationId(3), PresentationId(4)]
    );
    assert_eq!(assembled.dag.nodes.len(), 43);
    assert!(assembled.nodes.iter().all(|node| node.fallback.is_none()));
    assert_eq!(
        node_lanes(assembled),
        BTreeMap::from(
            [
                ("SeriesParallel", 26),
                ("Insertion", 6),
                ("Carpet", 2),
                ("Triangle", 6),
                ("Custom", 2),
                ("Trivial", 1)
            ]
            .map(|(lane, count)| (lane.to_owned(), count))
        )
    );

    // The Custom owners and their payloads.
    let labels: Vec<&str> = assembled.custom.keys().map(String::as_str).collect();
    assert_eq!(labels, [T1STAR05_LABEL, NO_LABEL]);
    for payload in assembled.custom.values() {
        assert_eq!((payload.meta.lp_count, payload.meta.residuals), (1, 1));
    }
    let no = owner_labelled(library, NO_LABEL);
    let kite = owner_labelled(library, T1STAR05_LABEL);
    assert_eq!(
        library.presentation(no.presentation).kind,
        PresentationKind::Induced {
            top: PresentationId(4),
            mask: 0xff
        }
    );
    assert_eq!(
        library.presentation(kite.presentation).kind,
        PresentationKind::EpsNode
    );
    // Every owner is on the presentation of its node (review SF9b): the
    // lane-adapted presentation, the Induced one for an integer Custom node.
    for owner in library.owners() {
        let node = &assembled.nodes[owner.spec.node.unwrap().0 as usize];
        assert_eq!(node.presentation, Some(owner.spec.presentation));
    }
    // One residual each, at the corner of the owner (n = 1 on the eps axis).
    assert_eq!(library.terminals().len(), 3);
    for (spec, corner) in [
        (&no, vec![1, 1, 1, 1, 1, 1, 1, 1, 0]),
        (&kite, vec![1, 1, 1, 1, 1]),
    ] {
        let residuals: Vec<&PTerminal> = (spec.terminals.iter())
            .map(|terminal| &library.terminals()[terminal.0 as usize])
            .collect();
        assert_eq!(
            residuals,
            [&PTerminal::Residual {
                owner: spec.class,
                key: key(&corner)
            }]
        );
    }

    // The certificate.
    let descent = assembled.descent.as_ref().unwrap();
    assert_eq!(descent.owners, lanes_of(assembled));
    assert_eq!(descent.owners[&OwnerLane::Custom], 2);
    assert_eq!((descent.guarded_owners, descent.custom_rules), (2, 25 + 18));
    let boxes: Vec<usize> = [&no, &kite]
        .iter()
        .map(|spec| {
            let p = library.presentation(spec.presentation);
            let support = library.census().classes[&spec.class].owner.1;
            descent::custom_witness_keys(p.slots.len(), support, p.forced_axes()).len()
        })
        .collect();
    assert_eq!(boxes, [135, 66]);
    assert_eq!(descent.witness_keys, 135 + 66);
    for witness in [DescentWitness::SameSectorStrict, DescentWitness::Subsector] {
        assert!(descent.witnesses[&witness] > 0, "{:?}", descent.witnesses);
    }
    assert_eq!(
        descent.zero_sectors + descent.owner_sectors + descent.routes,
        library.census().sectors.len()
    );
    let presentations = library.presentations();
    library
        .census()
        .check_complete(presentations, library.topology().map(Arc::as_ref))
        .unwrap();
    println!(
        "scope 1-3: {} presentations, {} classes, {} sectors ({} zero, {} owner, {} routed, {} flip q), \
         {} edge maps, owners {:?}, custom rules {}, witness keys {}, witnesses {:?}, timings {:?}",
        presentations.len(),
        library.census().classes.len(),
        library.census().sectors.len(),
        descent.zero_sectors,
        descent.owner_sectors,
        descent.routes,
        flipped_routes(library),
        descent.edge_maps,
        descent.owners,
        descent.custom_rules,
        descent.witness_keys,
        descent.witnesses,
        assembled.timings
    );
}

/// Provided payloads are used verbatim by class label: the same program and
/// payloads as a generating assembly; a missing label is refused, and so are
/// payloads under each other's label (the record does not fit the sector).
#[test]
fn provided_payloads_are_reused_verbatim_by_class_label() {
    let assembled = scope_three();
    let assemble_provided = |payloads: BTreeMap<String, CustomOwnerPayload>| {
        assemble_library(
            &families(),
            3,
            CustomPayloads::Provided(payloads),
            &AssembleOptions {
                threads: workers(),
                verify_descent: false,
            },
        )
    };
    let provided = assemble_provided(assembled.custom.clone()).unwrap();
    assert_eq!(provided.custom, assembled.custom);
    assert_eq!(inventory(&provided), inventory(assembled));

    let mut missing = assembled.custom.clone();
    missing.remove(NO_LABEL);
    let error = assemble_provided(missing).unwrap_err();
    assert!(
        matches!(&error, PError::MalformedLibrary(m) if m.contains(NO_LABEL)),
        "{error}"
    );
    let mut swapped = assembled.custom.clone();
    let no = swapped.remove(NO_LABEL).unwrap();
    let kite = swapped.remove(T1STAR05_LABEL).unwrap();
    swapped.insert(NO_LABEL.to_owned(), kite);
    swapped.insert(T1STAR05_LABEL.to_owned(), no);
    assert!(assemble_provided(swapped).is_err());
}

/// A one-sided generation of the eps kite: the chart `n >= 1` only, no
/// two-sided data (the rules study's false knock-out mode).
fn one_sided_payload(presentation: &Presentation) -> Arc<[u8]> {
    let mut request = FamilyCandidatesRequest::new("one-sided t1star05");
    request.selected_sectors = Some(vec![vec![true; presentation.slots.len()]]);
    let (session, job) =
        CandidateGenerationSession::from_family(Arc::clone(&presentation.family), request, 64)
            .unwrap();
    job.run();
    Arc::from(session.result().unwrap().bundle())
}

/// How [`Damaged`] breaks the step of a Custom owner.
#[derive(Clone, Copy, Debug)]
enum Damage {
    /// An extra Apply child one dot above the key.
    Climbs,
    /// An extra Route child outside the owner support (its ISP raised).
    Escapes,
    /// An extra terminal that is no residual of the owner (`Unit`).
    UnitTerminal,
}

/// A real Custom step with one damage on every key.
struct Damaged {
    inner: Arc<dyn PLaneStep>,
    damage: Damage,
}

impl PLaneStep for Damaged {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let mut out = self.inner.step(cx, key)?;
        let one = cx.library.coefficient_context().one();
        let mut powers = key.powers.to_vec();
        match self.damage {
            Damage::Climbs => {
                powers[0] += 1;
                out.children.push(PChild {
                    stage: PStage::Apply,
                    presentation: key.presentation,
                    powers,
                    tag: key.tag.clone(),
                    coefficient: one,
                });
            }
            Damage::Escapes => {
                *powers.last_mut().unwrap() = 1;
                out.children.push(PChild {
                    stage: PStage::Route,
                    presentation: key.presentation,
                    powers,
                    tag: key.tag.clone(),
                    coefficient: one,
                });
            }
            Damage::UnitTerminal => out.terminals.push((PTerminalId(0), key.tag.clone(), one)),
        }
        Ok(out)
    }

    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        self.inner.order_key(cx, powers)
    }
}

impl Parts {
    /// The library with `program` for the owner of `class`.
    fn build_with(self, class: OwnerClassId, program: OwnerProgram) -> Result<PLibrary, PError> {
        let context = self.presentations[0].family.coefficient_context().clone();
        let mut builder = PLibrary::builder(context, self.scope);
        for presentation in self.presentations {
            builder.add_presentation(presentation)?;
        }
        builder.set_topology(self.dag)?;
        builder.set_census(self.census)?;
        for map in self.edge_maps {
            builder.add_edge_map(map)?;
        }
        for terminal in self.terminals {
            builder.add_terminal(terminal)?;
        }
        let mut program = Some(program);
        for owner in self.owners {
            if owner.class == class {
                builder.add_owner_with_program(owner, program.take().unwrap())?;
            } else {
                builder.add_owner(owner)?;
            }
        }
        builder.build()
    }
}

/// Items 1 and 2 of the certificate on Custom owners (amendment 2026-11-06
/// 17:35Z): t1star05 passes (the assembled certificate), the eps kite with a
/// one-sided order under its own record fails; a step whose Apply child does
/// not descend in the order key, whose Route child leaves the owner support,
/// or that emits a terminal that is no residual of the owner fails.
#[test]
fn the_descent_certificate_checks_custom_orders_and_witnesses() {
    let library = &scope_three().library;
    let parts = Parts::of(library);

    let kite = owner_labelled(library, T1STAR05_LABEL);
    let mut damaged = parts.clone();
    let owner = (damaged.owners.iter_mut())
        .find(|owner| owner.class == kite.class)
        .unwrap();
    owner.payload = Some(CustomPayloadRef::Bytes(one_sided_payload(
        library.presentation(kite.presentation),
    )));
    let built = damaged.build().unwrap();
    let error = descent::check(&built).unwrap_err();
    assert!(
        matches!(&error, PError::MalformedLibrary(m)
            if m.contains("two-sided on 0x0 (program 0x0), the forced axes are 0x1")),
        "{error}"
    );
    let spec = built.owner(kite.class).unwrap().spec.clone();
    assert_eq!(
        lane_program(&built, &spec).unwrap_err(),
        PError::Unsupported("eps custom owner requires two-sided axes".to_owned())
    );

    let no = owner_labelled(library, NO_LABEL);
    let OwnerProgram::Lane(inner) = lane_program(library, &no).unwrap() else {
        unreachable!()
    };
    for (damage, reason) in [
        (Damage::Climbs, "Apply child"),
        (Damage::Escapes, "Route child"),
        (Damage::UnitTerminal, "the terminal 0"),
    ] {
        let program = OwnerProgram::Lane(Arc::new(Damaged {
            inner: Arc::clone(&inner),
            damage,
        }));
        let built = parts.clone().build_with(no.class, program).unwrap();
        let error = descent::check(&built).unwrap_err();
        assert!(
            matches!(&error, PError::MalformedLibrary(m) if m.contains(reason)),
            "{damage:?}: {error}"
        );
    }
}

/// `points` distinct random keys per loop count at scope 1-3 (the whole box at
/// one loop), dots <= 3 and rank <= 8 in total: nothing refused, no shortfall,
/// every owner applied, every DAG node reached (its class applied, or keys
/// routed on its presentation). Returns (loops, keys, Route keys, Apply keys,
/// zero Route keys) per row.
fn validate_scope_three(points: usize) -> Vec<(u8, usize, usize, usize, usize)> {
    let assembled = scope_three();
    let library = &assembled.library;
    let options = ValidateOptions {
        points,
        ..ValidateOptions::default()
    };
    let rows = validate_library(assembled, &options).unwrap();
    for row in &rows {
        let trace = &row.report;
        println!(
            "p-validate scope 1-3 L{}: {} distinct keys{}, {} route + {} apply keys ({} zero, {} through \
             q -> -q routes), {} owners applied, {:.2} s",
            row.loops,
            row.keys,
            if row.box_exhausted {
                " (whole box)"
            } else {
                ""
            },
            trace.route_keys,
            trace.apply_keys,
            trace.zero_keys,
            trace.flipped_route_keys,
            trace.applied.len(),
            row.seconds
        );
        assert!(trace.is_clean(), "L{}: {:?}", row.loops, trace.samples);
        assert_eq!(row.shortfall(options.points), 0);
    }
    assert_eq!(unapplied_owners(assembled, &rows), []);
    let applied: BTreeSet<OwnerClassId> = (rows.iter())
        .flat_map(|row| row.report.applied.keys().copied())
        .collect();
    let routed: BTreeSet<PresentationId> = (rows.iter())
        .flat_map(|row| row.report.routed.keys().copied())
        .collect();
    let census = library.census();
    for node in assembled.nodes.iter().filter(|node| !node.decision.trivial) {
        let class_applied = (census.classes.iter())
            .any(|(class, record)| record.nodes.contains(&node.index) && applied.contains(class));
        let presentation = node.presentation.unwrap();
        assert!(
            class_applied || routed.contains(&presentation),
            "node {} (presentation {}) never reached",
            node.index.0,
            presentation.0
        );
    }
    let reached: Vec<_> = rows
        .iter()
        .map(|row| {
            let trace = &row.report;
            (
                row.loops,
                row.keys,
                trace.route_keys,
                trace.apply_keys,
                trace.zero_keys,
            )
        })
        .collect();
    println!("p-validate scope 1-3 rows: {reached:?}");
    reached
}

/// p-validate at scope 1-3 with 100 keys per loop count, unoptimised in the
/// branch gate (the full 10^4: the slow test below, about 11 min optimised).
#[test]
fn p_validate_traces_a_hundred_keys_per_loop_count_at_scope_one_to_three() {
    let reached = validate_scope_three(100);
    assert_eq!(
        reached,
        [
            (1, 100, 100, 10, 90),
            (2, 100, 1_812, 406, 1_490),
            (3, 100, 409_663, 58_440, 335_988)
        ]
    );
}

/// The acceptance run: 10^4 distinct keys per loop count at scope 1-3; the 1L
/// and 2L rows are those of scope 1-2.
#[test]
#[ignore = "slow: 10^4 3L keys at rank <= 8 through the kernels; run optimised"]
fn p_validate_traces_ten_thousand_keys_per_loop_count_at_scope_one_to_three() {
    let reached = validate_scope_three(10_000);
    assert_eq!(
        reached,
        [
            (1, 127, 127, 10, 117),
            (2, 10_000, 22_803, 1_686, 19_364),
            (3, 10_000, 4_344_905, 374_703, 3_677_774)
        ]
    );
}

/// One cache serves the 3L program: a dotted LA top after the plain one meets
/// its subkeys at the same absolute tag and replays them, with the result of
/// an uncached run.
#[test]
fn the_step_cache_replays_three_loop_records_where_the_absolute_tag_matches() {
    let library = &scope_three().library;
    let context = library.coefficient_context();
    let cache =
        OrderedStepCache::in_memory(library.step_cache_binding().unwrap(), Default::default());
    let la = PresentationId(2);
    let cached = |powers: &[i64]| {
        let mut run = request(vec![(la, key(powers), context.one())]);
        run.step_cache = Some(Arc::clone(&cache));
        library.reduce(run).unwrap()
    };
    let first = cached(&[1, 1, 1, 1, 1, 1, 1, 1, 0]);
    assert_eq!(first.statistics.progress.step_cache_hits, 0);
    let dotted = [2, 1, 1, 1, 1, 1, 1, 1, 0];
    let second = cached(&dotted);
    let uncached = reduce(library, vec![(la, key(&dotted), context.one())]);
    assert_eq!(second.terms, uncached.terms);
    let progress = second.statistics.progress;
    assert!(progress.step_cache_hits > 0, "{progress:?}");
    println!(
        "dotted LA after LA: {} hits, {} misses",
        progress.step_cache_hits, progress.step_cache_misses
    );
}

/// Review SF9a: a Triangle node whose IBP row does not have Forcer's shape is
/// presented as Custom (on its Induced presentation, D7) with the reason kept,
/// its owner generated, and the library certified; the kite then reduces to
/// the values of the Triangle library. A failure of the run stops assembly
/// with its error and changes no lane.
#[test]
fn a_triangle_mismatch_falls_back_to_custom_and_a_failure_stops() {
    let options = AssembleOptions {
        threads: workers(),
        verify_descent: true,
    };
    let mismatch = |p: &Presentation, _: &TriangleRoles| {
        Err(TemplateError::Mismatch(TemplateMismatch {
            presentation: p.id,
            reason: "test: not Forcer's row".to_owned(),
        }))
    };
    let fallback = assemble_library_with(
        &families(),
        2,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &options,
        &mismatch,
    )
    .unwrap();
    let node = fallback
        .nodes
        .iter()
        .find(|node| node.fallback.is_some())
        .unwrap();
    assert_eq!(node.decision.lane, OwnerLane::Custom);
    assert!(
        node.fallback
            .as_ref()
            .unwrap()
            .contains("test: not Forcer's row")
    );
    let library = &fallback.library;
    assert_eq!(
        library.presentation(node.presentation.unwrap()).kind,
        PresentationKind::Induced {
            top: PresentationId(1),
            mask: 0x1f
        }
    );
    assert_eq!(
        lanes_of(&fallback),
        BTreeMap::from([(OwnerLane::SeriesParallel, 5), (OwnerLane::Custom, 1)])
    );
    assert_eq!(fallback.custom.len(), 1);
    let descent = fallback.descent.as_ref().unwrap();
    assert_eq!(descent.guarded_owners, 1);
    assert!(descent.witness_keys > 0);

    let reference = &scope_two().library;
    let context = library.coefficient_context();
    let mut rng = SplitMix::new(0x0fa1_1bac);
    let points = dimensions(&mut rng, 1);
    for powers in [
        [1, 1, 1, 1, 1],
        [2, 1, 1, 1, 1],
        [1, 1, 2, 1, 1],
        [1, 1, 1, 1, -1],
        [2, 1, 1, 0, 1],
    ] {
        let input = vec![(PresentationId(1), key(&powers), context.one())];
        assert_eq!(
            tag_values(&reduce(library, input.clone()), &points[0]),
            tag_values(&reduce(reference, input), &points[0]),
            "{powers:?}"
        );
    }

    let failure = |_: &Presentation, _: &TriangleRoles| {
        Err(TemplateError::Failure(PError::Generation(
            "test: the row failed".to_owned(),
        )))
    };
    assert_eq!(
        assemble_library_with(
            &families(),
            2,
            CustomPayloads::Provided(BTreeMap::new()),
            &options,
            &failure,
        )
        .unwrap_err(),
        PError::Generation("test: the row failed".to_owned())
    );
}

/// Keys of the three 3L tops with line powers in -1..=2 (ISP at 0, at most 8
/// in absolute sum) that are chains of one-loop insertions equal their
/// G-function values, `I_M = (-1)^{Sum n} I_E`, at two random `d`: per top 32
/// sampled chain keys of nonzero value and 8 scaleless ones.
#[test]
fn three_loop_series_parallel_inputs_equal_their_closed_forms() {
    let assembled = scope_three();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let mut rng = SplitMix::new(0x3c10_5ed0);
    let points = dimensions(&mut rng, 2);
    for top in assembled.tops_of(3) {
        let p = library.presentation(top);
        let family = closed_form_family(p);
        let lines = family.lines.len() as u32;
        // Chain keys with a nonzero value, and scaleless ones.
        let (mut nonzero_keys, mut zero_keys) = (Vec::new(), Vec::new());
        for code in 0..4_u64.pow(lines) {
            let line_powers: Vec<i64> = (0..lines)
                .map(|slot| (code / 4_u64.pow(slot) % 4) as i64 - 1)
                .collect();
            if line_powers.iter().map(|power| power.abs()).sum::<i64>() > 8 {
                continue;
            }
            match family.value(&key(&line_powers), &points[0]) {
                Some(value) if value.is_zero() => zero_keys.push(line_powers),
                Some(_) => nonzero_keys.push(line_powers),
                None => {}
            }
        }
        let total = (nonzero_keys.len(), zero_keys.len());
        let mut sampled = Vec::new();
        for (keys, budget) in [(&mut nonzero_keys, 32), (&mut zero_keys, 8)] {
            let target = sampled.len() + budget.min(keys.len());
            while sampled.len() < target {
                let index = rng.below(keys.len() as u64) as usize;
                sampled.push(keys.swap_remove(index));
            }
        }
        let mut nonzero = 0;
        for line_powers in &sampled {
            let mut powers = line_powers.clone();
            powers.resize(p.slots.len(), 0);
            let reduction = reduce(library, vec![(top, key(&powers), context.one())]);
            let sign = if powers.iter().sum::<i64>() % 2 == 0 {
                1
            } else {
                -1
            };
            for d in &points {
                let ours: Vec<GValue> = reduction
                    .terms
                    .iter()
                    .map(|term| {
                        assert_eq!(term.basis, PBasis::Unit);
                        tag_value(&term.tag, d)
                            .scale(&(&at(&term.coefficient, d) * &rational(sign, 1)))
                    })
                    .collect();
                let expected = family.value(&key(line_powers), d).unwrap();
                let difference = ours
                    .iter()
                    .cloned()
                    .chain([expected.scale(&rational(-1, 1))]);
                assert!(
                    collect(difference).is_empty(),
                    "{top:?} {powers:?} at d = {d}"
                );
                nonzero += usize::from(!expected.is_zero());
            }
        }
        println!(
            "closed forms on presentation {}: {} of {} nonzero and {} scaleless chain keys, \
             {nonzero} nonzero values",
            top.0,
            sampled.len(),
            total.0,
            total.1
        );
        assert!(nonzero >= 2 * 32.min(total.0));
    }
}

/// Exactness at scope 1-3 against P0 SpIRed reductions in the top families, in
/// the tag basis at two random `d`, on inputs distinct up to a scalar: L1 the
/// bounded space, L2 640 and L3 960 random inputs over all tops (dots <= 2,
/// negative powers -3..-1, random subsector masks, rank 0-6 incl. odd), plus
/// 64 rank 7-8 numerator inputs at L3 through the kernel owners. Every owner
/// and every DAG node is reached; per-L counts are bounded below.
#[test]
#[ignore = "slow: P0 SpIRed on the three 8-line 3L tops and about 2,000 reductions; run optimised"]
fn scope_one_to_three_reductions_equal_p0_sector_reductions() {
    let assembled = scope_three();
    let library = &assembled.library;
    let mut rng = SplitMix::new(0x5ec7_0f3b);
    let points = dimensions(&mut rng, 2);
    println!(
        "scope 1-3 routes with external_sign -1: {}",
        flipped_routes(library)
    );
    let mut rows = Vec::new();
    for loops in 1..=3_u8 {
        let count = if loops == 3 { 960 } else { EXACT_INPUTS };
        let (inputs, exhausted) =
            exactness_inputs_of(assembled, loops, 0x1000 + u64::from(loops), count);
        rows.push(compare_with_p0(
            assembled, loops, &inputs, exhausted, &points,
        ));
    }
    // Rank 7 and 8 through the kernel owners (rr-kernel: order sigma = 4 of
    // the numerator rewrite needs rank 8).
    let mut rng = SplitMix::new(0x8a4c_0003);
    let tops = assembled.tops_of(3);
    let mut seen = BTreeSet::new();
    let mut high = Vec::new();
    while high.len() < 64 {
        let top = library.presentation(tops[rng.below(tops.len() as u64) as usize]);
        let input = numerator_input(top, 7 + rng.below(2) as u32, &mut rng);
        if seen.insert(input.normal_form()) {
            high.push(input);
        }
    }
    let high = compare_with_p0(assembled, 3, &high, false, &points);
    let summaries: Vec<_> = rows.iter().map(|row| row.summary).collect();
    println!(
        "exactness rows: {summaries:?}; rank 7-8: {:?}",
        high.summary
    );

    let (_, _, inputs, keys, nonzero, nonzero_inputs, odd, max_rank) = summaries[2];
    assert_eq!(inputs, 960);
    assert!(keys >= 5_000, "L3: {keys} keys");
    assert!(nonzero >= 600, "L3: {nonzero} nonzero comparisons");
    assert!(nonzero_inputs >= 300, "L3: {nonzero_inputs} nonzero inputs");
    assert!(
        odd >= 100 && max_rank == 6,
        "L3: {odd} odd, rank {max_rank}"
    );
    let (_, _, _, _, _, high_nonzero, _, high_rank) = high.summary;
    assert!(high_nonzero >= 16 && high_rank == 8, "{:?}", high.summary);
    let kernel_sites: usize = (high.trace.applied.iter())
        .filter(|(class, _)| {
            matches!(
                library.owner(**class).unwrap().spec.lane,
                OwnerLane::SeriesParallel | OwnerLane::Insertion | OwnerLane::Carpet
            )
        })
        .map(|(_, sites)| sites)
        .sum();
    assert!(kernel_sites > 0);

    let applied: BTreeSet<OwnerClassId> = (rows.iter().chain([&high]))
        .flat_map(|row| row.trace.applied.keys().copied())
        .collect();
    assert_eq!(
        applied.len(),
        library.owners().len(),
        "an owner never applied"
    );
    let routed: BTreeSet<PresentationId> = (rows.iter().chain([&high]))
        .flat_map(|row| row.trace.routed.keys().copied())
        .collect();
    let census = library.census();
    for node in assembled.nodes.iter().filter(|node| !node.decision.trivial) {
        let class_applied = (census.classes.iter())
            .any(|(class, record)| record.nodes.contains(&node.index) && applied.contains(class));
        assert!(
            class_applied || routed.contains(&node.presentation.unwrap()),
            "node {} never reached",
            node.index.0
        );
    }
}

/// Exact and ModularPolynomial weights, 1 and 16 scheduler threads, and
/// modular with 4 threads under 16 scheduler threads give the same terms on 60
/// random inputs per loop count of the scope 1-3 program; wall times of both
/// arithmetics printed per loop count.
#[test]
#[ignore = "slow: 3L reductions in four configurations; run optimised"]
fn scope_one_to_three_exact_equals_modular_polynomial_and_threads() {
    let assembled = scope_three();
    let library = &assembled.library;
    let context = library.coefficient_context();
    for loops in 1..=3_u8 {
        let inputs = random_inputs(assembled, loops, 60, 0x2100 + u64::from(loops));
        let mut seconds = [0.0_f64; 3];
        for input in &inputs {
            let start = Instant::now();
            let exact = reduce(library, input.request_inputs(context));
            seconds[0] += start.elapsed().as_secs_f64();
            let start = Instant::now();
            let mut modular = request(input.request_inputs(context));
            modular.arithmetic = modular_polynomial(1);
            let modular = library.reduce(modular).unwrap();
            seconds[1] += start.elapsed().as_secs_f64();
            assert_eq!(exact.terms, modular.terms, "{input:?}");
            assert_eq!(exact.nonzero_conditions, modular.nonzero_conditions);
            let mut threaded = request(input.request_inputs(context));
            threaded.scheduler = OrderedSchedulerOptions {
                threads: 16,
                window_keys: 8,
                ..Default::default()
            };
            let threaded = library.reduce(threaded).unwrap();
            assert_eq!(exact.terms, threaded.terms);
            assert_eq!(exact.nonzero_conditions, threaded.nonzero_conditions);
            let start = Instant::now();
            let mut both = request(input.request_inputs(context));
            both.arithmetic = modular_polynomial(4);
            both.scheduler = OrderedSchedulerOptions {
                threads: 16,
                window_keys: 8,
                ..Default::default()
            };
            let both = library.reduce(both).unwrap();
            seconds[2] += start.elapsed().as_secs_f64();
            assert_eq!(exact.terms, both.terms, "{input:?}");
            assert_eq!(exact.nonzero_conditions, both.nonzero_conditions);
        }
        println!(
            "scope 1-3 L{loops}: {} inputs, Exact {:.3} s, ModularPolynomial {:.3} s, \
             ModularPolynomial(4) x 16 scheduler threads {:.3} s",
            inputs.len(),
            seconds[0],
            seconds[1],
            seconds[2]
        );
    }
}

/// The scope 1-3 golden inventory with the Custom payload descriptors.
#[test]
#[ignore = "slow: generates NO and t1star05 and dumps both payloads"]
fn the_scope_one_to_three_inventory_is_pinned_to_the_identity() {
    check_inventory("1-3", scope_three());
}
