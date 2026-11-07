//! Library assembly and golden inventories (owner: M2-T8a rr-assemble).
//!
//! The scope 1-2 library program assembled from the built-in families: its
//! census, owners, lane programs and descent certificate; `p-validate`'s trace
//! on distinct random keys; the reductions of random inputs against P0 SpIRed
//! reductions in their top family, in the tag basis; closed forms of
//! series-parallel inputs; arithmetic, thread and step-cache invariance; the
//! golden inventory pinned to the library identity.
//!
//! Bounds are TOTAL over the slots of a key: dots over the lines, rank over the
//! numerator slots (lines at negative powers and ISPs). Counts are of distinct
//! keys after deduplication.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensus, PSectorFate};
use rustred::propagator::descent::{self, TraceFailureKind};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::kernel::gamma::FactoredRational;
use rustred::propagator::lane::{LaneRoles, OwnerLane};
use rustred::propagator::library::{
    CustomOwnerMeta, OwnerProgram, PBasis, PLibrary, POwnerSpec, PReduction, PReductionRequest,
    PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{EdgeMap, Presentation, PresentationKind, Slot};
use rustred::propagator::programs::{CustomPayloadRef, lane_program};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::Dag;
use rustred::propagator::{OwnerClassId, PTerminalId, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedStepCache, OrderedWeightArithmetic,
    PolynomialOptions, ReductionLimits,
};
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use rustred_app::propagator::assemble::{
    SplitMix, ValidateOptions, unapplied_owners, validate_library,
};
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomOwnerPayload, CustomPayloads,
    assemble_library,
};
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
/// damage: an edge map named twice or by nobody, a rule owner without its edge,
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
    assert!(matches!(refused(damaged), PError::MalformedLibrary(_)));

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
    // The guard passes; the payload itself is M2-T7c's.
    let passing = custom(good.clone(), 1).build().unwrap();
    let spec = passing
        .owners()
        .find(|o| o.spec.lane == OwnerLane::Custom)
        .unwrap()
        .spec
        .clone();
    descent::check_custom_guard(&passing, &spec).unwrap();
    assert_eq!(
        lane_program(&passing, &spec).unwrap_err(),
        PError::NotImplemented("M2-T7c")
    );

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
    assert_eq!(rows[1].keys, 10_000);
    assert!(!rows[1].box_exhausted);
    assert_eq!(unapplied_owners(assembled, &rows), []);
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
const INVENTORY_PINS: &[(&str, &str, &str)] = &[(
    "1-2",
    "p-library:scope=1-2:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
    "c6f122ee6176383cb2acb1883e6ff89b915a4911771dcc400430fece3b294bf2",
)];

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

/// A reduction onto the terminals in the tag basis at `d`: every term is on the
/// trivial node at scope 1-2.
fn tag_values(reduction: &PReduction, d: &Rational) -> BTreeMap<GammaTag, Rational> {
    let mut values = BTreeMap::new();
    for term in &reduction.terms {
        assert_eq!(term.basis, PBasis::Unit, "a residual at scope 1-2");
        let entry = values
            .entry(term.tag.clone())
            .or_insert_with(Rational::zero);
        *entry = &*entry + &at(&term.coefficient, d);
    }
    values.retain(|_, value: &mut Rational| !value.is_zero());
    values
}

fn add_scaled(
    sum: &mut BTreeMap<GammaTag, Rational>,
    values: &BTreeMap<GammaTag, Rational>,
    factor: &Rational,
) {
    for (tag, value) in values {
        let entry = sum.entry(tag.clone()).or_insert_with(Rational::zero);
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
    /// The sign of `q` in its scalar products, `0` for a plain key.
    q_sign: i64,
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

/// Denominators of a random input: a random subsector of the lines with `dots`
/// extra units; the other slots are numerator slots.
fn denominators(p: &Presentation, dots: u32, rng: &mut SplitMix) -> Vec<i64> {
    let lines: Vec<usize> = (0..p.slots.len())
        .filter(|&s| matches!(p.slots[s], Slot::Line(_)))
        .collect();
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
        q_sign: 0,
    }
}

/// Denominators times `rank` random scalar products `k_i.k_j` and
/// `k_i.(s q)`, `s = q_sign`, lowered onto the slots by
/// `express_scalar_product` and expanded.
fn numerator_input(p: &Presentation, rank: u32, q_sign: i64, rng: &mut SplitMix) -> Input {
    let dots = rng.below(3) as u32;
    let base = denominators(p, dots, rng);
    let width = usize::from(p.loops) + 1;
    let unit = |column: usize, sign: i64| {
        let mut row = vec![0_i64; width];
        row[column] = sign;
        row
    };
    // Polynomial in the slots: exponents -> coefficient.
    let mut polynomial: BTreeMap<Vec<i64>, Rational> =
        BTreeMap::from([(vec![0_i64; p.slots.len()], small_rational(rng))]);
    for _ in 0..rank {
        let i = rng.below(u64::from(p.loops)) as usize;
        let other = rng.below(u64::from(p.loops) + 1) as usize;
        let right = if other == usize::from(p.loops) {
            unit(other, q_sign)
        } else {
            unit(other, 1)
        };
        let form = p.express_scalar_product(&unit(i, 1), &right).unwrap();
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
    let terms = polynomial
        .into_iter()
        .map(|(exponents, weight)| {
            let powers = base.iter().zip(&exponents).map(|(b, e)| b - e).collect();
            (powers, weight)
        })
        .collect();
    Input {
        top: p.id,
        terms,
        rank,
        q_sign,
    }
}

/// `count` random inputs over the tops of `loops` loops: plain keys (dots <= 2,
/// rank <= 6 as negative powers -3..-1) and numerator inputs (dots <= 2, rank
/// 0..=6 in scalar products, both signs of `q`), alternately.
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
                let sign = if rng.below(2) == 0 { 1 } else { -1 };
                numerator_input(top, rank, sign, &mut rng)
            }
        })
        .collect()
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

/// Distinct inputs per loop count of the exactness test.
const EXACT_INPUTS: usize = 320;

/// `PLib(x) == Sum_j e_j(d) PLib(T_j)` in the tag basis, where `x = Sum_j e_j T_j`
/// is the P0 SpIRed reduction of `x` in its top family, at two random `d` each:
/// 320 distinct inputs per loop count (L = 1, 2), and every owner applied by
/// them (counted by the trace of their keys).
#[test]
fn reductions_equal_p0_sector_reductions_in_the_tag_basis() {
    let assembled = scope_two();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let mut rng = SplitMix::new(0x5ec7_0f2a);
    let points = dimensions(&mut rng, 2);
    let mut all_keys = BTreeSet::new();
    for loops in 1..=2_u8 {
        let mut inputs = random_inputs(
            assembled,
            loops,
            4 * EXACT_INPUTS,
            0x1000 + u64::from(loops),
        );
        // Distinct inputs only.
        let mut seen = BTreeSet::new();
        inputs.retain(|input| seen.insert(format!("{:?} {:?}", input.top, input.terms)));
        inputs.truncate(EXACT_INPUTS);
        assert_eq!(
            inputs.len(),
            EXACT_INPUTS,
            "L{loops}: too few distinct inputs"
        );
        let (mut odd, mut negative_q, mut max_rank) = (0, 0, 0);
        for input in &inputs {
            odd += usize::from(input.rank % 2 == 1);
            negative_q += usize::from(input.q_sign < 0);
            max_rank = max_rank.max(input.rank);
        }
        let start = Instant::now();
        for &top in &assembled.tops_of(loops) {
            let mut p0 = P0::new(library.presentation(top));
            let ours: Vec<&Input> = inputs.iter().filter(|input| input.top == top).collect();
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
            for d in &points {
                let rows = p0.rows(&keys, d);
                let master_values: Vec<_> = masters.iter().map(|m| tag_values(m, d)).collect();
                for input in &ours {
                    let ours_value = tag_values(&reduce(library, input.request_inputs(context)), d);
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
                }
            }
        }
        println!(
            "exactness L{loops}: {} distinct inputs ({odd} odd rank, {negative_q} with -q, rank <= {max_rank}), \
             2 d each, {:.1} s",
            inputs.len(),
            start.elapsed().as_secs_f64()
        );
    }
    let keys: Vec<_> = all_keys.into_iter().collect();
    let trace = descent::trace(library, &keys, ReductionLimits::default()).unwrap();
    assert!(trace.is_clean());
    println!(
        "coverage: {} distinct keys, {} apply sites over {} owners: {:?}",
        keys.len(),
        trace.apply_sites,
        trace.applied.len(),
        trace
            .applied
            .iter()
            .map(|(class, sites)| (library.census().classes[class].key.label(), *sites))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        trace.applied.len(),
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

/// The test family of a top presentation: its line momenta.
fn closed_form_family(p: &Presentation) -> PFamily {
    let lines: Vec<&'static [i64]> = p
        .slots
        .iter()
        .map(|slot| -> &'static [i64] {
            match slot {
                Slot::Line(line) => Box::leak(line.momentum.clone().into_boxed_slice()),
                Slot::Isp(_) => unreachable!("the 1L and 2L tops have no ISP"),
            }
        })
        .collect();
    PFamily {
        name: "top",
        loops: usize::from(p.loops),
        lines: Box::leak(lines.into_boxed_slice()),
        root: Box::leak(vec![true; p.slots.len()].into_boxed_slice()),
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
        while sampled.len() < budget.min(keys.len()) {
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
/// scheduler threads the same bits, on 60 random inputs per loop count; the wall
/// time of both arithmetics is printed.
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
        _ => &[(1, 0), (2, 0), (1, 2), (1, 0), (2, 0), (1, 2)],
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

/// The 1L and 2L benchmark cells: distinct tags per Apply site (plan risk 12);
/// cold and warm wall times under Exact (step cache on disk, warm = reopened)
/// and ModularPolynomial weights (in memory); the bytes of the saved records and
/// the share of identities in them (a shard file stores each identity once: the
/// tagged trace counts the distinct identities per shard).
/// Measurements, printed; run optimised.
#[test]
#[ignore = "slow: benchmark cells at rank 8, measured optimised"]
fn benchmark_cells_tags_and_timings() {
    let assembled = scope_two();
    let library = &assembled.library;
    let context = library.coefficient_context();
    let binding = library.step_cache_binding().unwrap();
    let tagged = descent::TraceOptions {
        limits: ReductionLimits::default(),
        tags: true,
    };
    for loops in 1..=2_u8 {
        let mut all_keys = Vec::new();
        let (mut record_bytes, mut identities) = (0_u64, 0_usize);
        for dots in 0..=2 {
            for rank in [0, 2, 4, 6, 8] {
                let mut inputs = Vec::new();
                for top in assembled.tops_of(loops) {
                    let terms = benchmark_input(library.presentation(top), dots, rank);
                    all_keys.extend(terms.iter().map(|(powers, _)| (top, powers.clone())));
                    inputs.extend(
                        Input {
                            top,
                            terms,
                            rank,
                            q_sign: 1,
                        }
                        .request_inputs(context),
                    );
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
                    line += &format!(", exact {state} {:.4} s", start.elapsed().as_secs_f64());
                    if state == "cold" {
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
                    line += &format!(", modular {state} {:.4} s", start.elapsed().as_secs_f64());
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
             identities about {:.0}% of them ({identities} identities of {} bytes)",
            tags.apply_keys,
            tags.apply_sites,
            tags.apply_keys as f64 / tags.apply_sites.max(1) as f64,
            100.0 * identity_share,
            identity_bytes(library)
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
