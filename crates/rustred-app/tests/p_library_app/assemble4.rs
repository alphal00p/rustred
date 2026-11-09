//! The scope 1-4 library program (owner: M3-T8c rr-assemble4).
//!
//! STAGE 1 assembles the program of the built-in tops with 1-4 loops, the
//! Diamond nodes on the Custom lane (plan M3 D-M3-2), from the Custom payloads
//! M3-T15 generated once and staged with their LP counts (D-M3-3): the owner
//! directory named by `RUSTRED_P_OWNERS_1_4` (`rustred p-generate` records,
//! read by `read_custom_payloads` as `CustomPayloads::Provided`). Checked: the
//! named counts and Custom owners of the golden `inventory_1-4.txt` (in
//! rustred-core's golden directory, beside M3-T15's `owners_1-4.tsv`), the
//! descent certificate, a short trace, per-rule firing on the Custom witness
//! boxes (I-5), the regeneration of the package from the staged payloads with
//! the LP recount of the owners of at most 9 lines (every owner only with
//! `P_LP_RECOUNT_ALL=1`) and one load under `verify_descent` (I-8), and the
//! numbers of the D-SIZE decision (load times, first result, resident sizes).
//! The 4L tests are `#[ignore]` and run optimised once (notes/rr-assemble4.md).
//! The certificate's refusal of a Custom Route child on the owner support
//! (review S2 of M2-T8a, `RouteOnSupport`) runs in the gate at scope 1-2.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::family::IntegralKey;
use rustred::propagator::carpet::CarpetStep;
use rustred::propagator::census::PSectorFate;
use rustred::propagator::custom::{
    CustomPower, CustomRule, build_custom_step, build_custom_step_prepared, custom_payload_dump,
    custom_payload_rules, inspect_custom_payload,
};
use rustred::propagator::descent::{self, custom_witness_keys};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::insertion::InsertionStep;
use rustred::propagator::kernel::MemoCounters;
use rustred::propagator::lane::{OwnerLane, TriangleRoles};
use rustred::propagator::library::{
    OwnerProgram, PLibrary, POwner, PReduction, PReductionRequest, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot};
use rustred::propagator::series_parallel::SeriesParallelStep;
use rustred::propagator::step::{
    PChild, PKeyView, PLaneStep, POrderKey, PStage, PStepContext, PStepOut,
};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::ActionKind;
use rustred::propagator::triangle::{TemplateError, TemplateMismatch};
use rustred::propagator::{
    OwnerClassId, P_CUSTOM_OWNER_VERSION, P_OWNER_LIBRARY_SEMANTICS, PresentationId,
};
use rustred::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits};
use rustred_app::propagator::assemble::{ValidateOptions, assemble_library_with, validate_library};
use rustred_app::propagator::generate::custom_owner_lp_count;
use rustred_app::propagator::loader::read_custom_payloads;
use rustred_app::propagator::package::{
    assemble_p_package, p_library_differences, p_library_files, write_p_library_files,
};
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomOwnerPayload, CustomPayloads,
    PLibraryOptions, PLibrarySource, assemble_library, load_p_library,
};

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

/// The owners of at most this many lines are recounted by the regeneration
/// test; the 10- and 11-line owners only with `P_LP_RECOUNT_ALL=1` (each about
/// 24 min and 3.4 GB, D-M3-3: M3-T15's count is the one count).
const RECOUNT_MAX_LINES: u32 = 9;

const CORE_GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustred-core/tests/p_library/golden"
);
const FORCER_KEYS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../rustred-topology/tests/fixtures/forcer/graph_keys_p_topology_v1.tsv"
);

fn families() -> PFamiliesFile {
    serde_json::from_str(P_FAMILIES_V1_JSON).unwrap()
}

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

/// The staged owner directory of M3-T15.
fn owners_directory() -> PathBuf {
    let directory = std::env::var_os("RUSTRED_P_OWNERS_1_4").expect(
        "RUSTRED_P_OWNERS_1_4 names the staged Custom owners of scope 1-4 \
         (M3-T15: $WS/artefacts/m3/owners-1-4)",
    );
    PathBuf::from(directory)
}

/// The scope 1-4 program from the staged payloads, with its descent
/// certificate; assembled once per test process.
fn scope_four() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let payloads = read_custom_payloads(&PLibrarySource::Directory(owners_directory()))
            .expect("the staged owners read back");
        println!("{} staged Custom payloads", payloads.len());
        let start = Instant::now();
        let assembled = assemble_library(
            &families(),
            4,
            CustomPayloads::Provided(payloads),
            &AssembleOptions {
                threads: workers(),
                verify_descent: true,
            },
        )
        .expect("scope 1-4 assembles");
        println!(
            "scope 1-4 assembled in {:.1} s on {} census threads: {:?}",
            start.elapsed().as_secs_f64(),
            workers(),
            assembled.timings
        );
        assembled
    })
}

/// Forcer's topology names by GraphKey bytes (rustred-topology's fixture).
fn forcer_names() -> &'static BTreeMap<Vec<u8>, String> {
    static NAMES: OnceLock<BTreeMap<Vec<u8>, String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let text = std::fs::read_to_string(FORCER_KEYS).unwrap();
        (text.lines())
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let (name, hex) = line.split_once('\t').unwrap();
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
                    .collect();
                (bytes, name.to_owned())
            })
            .collect()
    })
}

/// The Forcer name of the DAG node of `presentation`, `-` without one.
fn name_of(presentation: &Presentation) -> String {
    (presentation.node.as_ref())
        .and_then(|node| forcer_names().get(node.graph_key.as_ref()))
        .cloned()
        .unwrap_or_else(|| "-".to_owned())
}

fn lines_of(presentation: &Presentation, support: u64) -> u32 {
    let lines = (presentation.slots.iter().enumerate())
        .filter(|(_, slot)| matches!(slot, Slot::Line(_)))
        .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
    (support & lines).count_ones()
}

/// The Custom owners: label, owner, its owner support.
fn custom_owners(library: &PLibrary) -> Vec<(String, &POwner, u64)> {
    let mut owners: Vec<_> = (library.owners())
        .filter(|owner| owner.spec.lane == OwnerLane::Custom)
        .map(|owner| {
            let record = &library.census().classes[&owner.spec.class];
            (record.key.label(), owner, record.owner.1)
        })
        .collect();
    owners.sort_by(|a, b| a.0.cmp(&b.0));
    owners
}

// ---------------------------------------------------------------------------
// Named counts and the golden inventory
// ---------------------------------------------------------------------------

/// One named count of the inventory: its value and its definition.
struct Count {
    name: String,
    value: usize,
    definition: &'static str,
}

fn count(name: impl Into<String>, value: usize, definition: &'static str) -> Count {
    Count {
        name: name.into(),
        value,
        definition,
    }
}

/// The named counts of `assembled`, each with its definition.
fn named_counts(assembled: &AssembledLibrary) -> Vec<Count> {
    let library = &assembled.library;
    let dag = &assembled.dag;
    let census = library.census();
    let mut counts = vec![count(
        "forcer_tree_nodes",
        dag.nodes.len(),
        "nodes of the ForcerTree DAG of the tops with 1-4 loops, the trivial node included",
    )];
    let mut actions: BTreeMap<(u8, String), usize> = BTreeMap::new();
    let mut lanes: BTreeMap<(u8, String), usize> = BTreeMap::new();
    for (node, info) in assembled.nodes.iter().zip(&dag.nodes) {
        let action = format!("{:?}", info.action.kind());
        *actions.entry((info.loops, action)).or_default() += 1;
        let lane = format!("{:?}", node.decision.lane);
        *lanes.entry((info.loops, lane)).or_default() += 1;
    }
    for ((loops, action), n) in actions {
        counts.push(count(
            format!("forcer_action_L{loops}_{action}"),
            n,
            "DAG nodes of that loop count whose Forcer action (rustred-topology matcher) is that one",
        ));
    }
    for ((loops, lane), n) in lanes {
        counts.push(count(
            format!("node_lane_L{loops}_{lane}"),
            n,
            "DAG nodes of that loop count presented on that lane (Zero = the trivial node)",
        ));
    }
    let diamonds = (assembled.nodes.iter().zip(&dag.nodes))
        .filter(|(node, info)| {
            info.action.kind() == ActionKind::Diamond && node.decision.lane == OwnerLane::Custom
        })
        .count();
    counts.push(count(
        "diamond_nodes_on_custom",
        diamonds,
        "DAG nodes with the Forcer action Diamond presented on the Custom lane (D-M3-2, Diamond -> Custom)",
    ));
    counts.push(count(
        "triangle_fallbacks",
        assembled
            .nodes
            .iter()
            .filter(|n| n.fallback.is_some())
            .count(),
        "Triangle nodes presented as Custom because their IBP row is not Forcer's",
    ));
    counts.push(count(
        "presentations",
        library.presentations().len(),
        "presentations of the program: the tops, then one per DAG node with a presentation",
    ));
    counts.push(count(
        "census_nodes",
        assembled
            .nodes
            .iter()
            .filter(|n| n.presentation.is_some())
            .count(),
        "DAG nodes with a presentation in the census (every node but the trivial one)",
    ));
    counts.push(count(
        "census_classes",
        census.classes.len(),
        "owner classes of the census (one owner each)",
    ));
    for row in census.counts(library.presentations()).unwrap() {
        let l = row.loops;
        for (what, value, definition) in [
            (
                "presentations",
                row.presentations,
                "presentations of that loop count",
            ),
            (
                "sectors",
                row.sectors,
                "census sectors (presentation, support) of that loop count",
            ),
            ("zero_sectors", row.zero_sectors, "of them proved zero"),
            (
                "nonzero_sectors",
                row.nonzero_sectors,
                "of them sent to a class (owner sector or route)",
            ),
            (
                "classes",
                row.classes,
                "classes of that loop count (a class counts at its own loop order)",
            ),
        ] {
            counts.push(count(format!("census_L{l}_{what}"), value, definition));
        }
    }
    let tops: BTreeSet<PresentationId> = assembled.tops_of(4).into_iter().collect();
    let mut top_classes = BTreeSet::new();
    let mut top_sectors = 0;
    for (&(id, _), fate) in &census.sectors {
        if let (true, PSectorFate::Owner { class, .. }) = (tops.contains(&id), fate) {
            top_sectors += 1;
            top_classes.insert(*class);
        }
    }
    counts.push(count(
        "l4_tops",
        tops.len(),
        "top presentations with 4 loops (the families' 4L tops)",
    ));
    counts.push(count(
        "l4_top_nonzero_sectors",
        top_sectors,
        "sectors of the 4L top presentations the census sends to a class (M2 G3: nonzero labelled 4L sectors)",
    ));
    counts.push(count(
        "l4_top_classes",
        top_classes.len(),
        "distinct classes of those sectors (M2 G3: 4L classes)",
    ));
    let mut owners: BTreeMap<String, usize> = BTreeMap::new();
    for owner in library.owners() {
        *owners.entry(format!("{:?}", owner.spec.lane)).or_default() += 1;
    }
    for (lane, n) in owners {
        counts.push(count(
            format!("owners_{lane}"),
            n,
            "owners (one per class) on that lane",
        ));
    }
    counts.push(count(
        "edge_maps",
        library.edge_maps().len(),
        "verified edge maps of the rule owners' rule edges into a presented node",
    ));
    counts.push(count(
        "terminals",
        library.terminals().len(),
        "terminals: Unit, then one residual per residual key of a Custom owner",
    ));
    if let Some(report) = &assembled.descent {
        for (what, value, definition) in [
            (
                "zero_sectors",
                report.zero_sectors,
                "certificate: census sectors proved zero",
            ),
            (
                "owner_sectors",
                report.owner_sectors,
                "certificate: census sectors that are their class's owner sector",
            ),
            (
                "routes",
                report.routes,
                "certificate: census routes verified and compiled",
            ),
            (
                "edge_maps",
                report.edge_maps,
                "certificate: edge maps verified, each named by one owner",
            ),
            (
                "guarded_owners",
                report.guarded_owners,
                "certificate: Custom owners through the hard guard",
            ),
            (
                "custom_rules",
                report.custom_rules,
                "certificate: rules of the Custom payloads",
            ),
            (
                "witness_keys",
                report.witness_keys,
                "certificate: Custom witness-box keys stepped (dots <= 2, rank <= 2)",
            ),
        ] {
            counts.push(count(format!("certificate_{what}"), value, definition));
        }
        for (witness, n) in &report.witnesses {
            counts.push(count(
                format!("certificate_witnesses_{witness:?}"),
                *n,
                "certificate: children of the witness steps by witness",
            ));
        }
    }
    counts
}

/// One line per Custom owner: label, Forcer name, node, sizes, meta and the
/// payload by its content (the canonical dump does not depend on the process
/// symbol table the bytes embed).
fn custom_rows(assembled: &AssembledLibrary) -> Vec<String> {
    let library = &assembled.library;
    custom_owners(library)
        .into_iter()
        .map(|(label, owner, support)| {
            let spec = &owner.spec;
            let presentation = library.presentation(spec.presentation);
            let meta = spec.custom.as_ref().unwrap();
            let bytes = spec.payload.as_ref().unwrap().bytes().unwrap();
            let summary = inspect_custom_payload(&bytes).unwrap();
            let dump = custom_payload_dump(&bytes).unwrap();
            format!(
                "custom {label} {} node {} L{} lines {} eps {} arity {} lp {} residuals {} two-sided {:#x} \
                 charts {:x?} rules {} residual keys {:?} order {} dump {} lines blake3 {}",
                name_of(presentation),
                spec.node.map_or(-1, |n| i64::from(n.0)),
                presentation.loops,
                lines_of(presentation, support),
                presentation.forced_axes().count_ones(),
                presentation.slots.len(),
                meta.lp_count,
                meta.residuals,
                meta.two_sided_axes,
                meta.charts,
                summary.rules(),
                summary.residual_keys(),
                summary.ordering.split(';').next().unwrap(),
                dump.lines().count(),
                blake3::hash(dump.as_bytes()).to_hex()
            )
        })
        .collect()
}

/// The inventory text: identity, the named counts (`count <name> <value>  #
/// <definition>`), M3-T15's `owners_1-4.tsv` by hash, the Custom owners.
fn inventory(assembled: &AssembledLibrary) -> String {
    use std::fmt::Write as _;
    let mut out = String::from(
        "# The scope 1-4 p library program (M3-T8c). Counts are compared by name; \
         RUSTRED_P_BLESS=1 rewrites this file.\n",
    );
    writeln!(out, "identity {}", assembled.library.identity()).unwrap();
    for c in named_counts(assembled) {
        writeln!(out, "count {} {}  # {}", c.name, c.value, c.definition).unwrap();
    }
    let owners_tsv = std::fs::read(Path::new(CORE_GOLDEN).join("owners_1-4.tsv")).map_or_else(
        |_| "absent".to_owned(),
        |b| blake3::hash(&b).to_hex().to_string(),
    );
    writeln!(out, "owners_1-4.tsv blake3 {owners_tsv}").unwrap();
    for row in custom_rows(assembled) {
        writeln!(out, "{row}").unwrap();
    }
    out
}

/// `count` lines of an inventory by name.
fn counts_by_name(text: &str) -> BTreeMap<String, String> {
    (text.lines())
        .filter_map(|line| line.strip_prefix("count "))
        .map(|line| {
            let (name, rest) = line.split_once(' ').unwrap();
            (
                name.to_owned(),
                rest.split("  #").next().unwrap().to_owned(),
            )
        })
        .collect()
}

/// `(scope, identity, blake3 of the inventory)`, append-only as the 1-2 and
/// 1-3 pins of M2-T8a: a new inventory needs a new identity.
const INVENTORY_PINS: &[(&str, &str, &str)] = &[];

/// The golden inventory: equal by name for the counts, then as a whole, and
/// pinned to the identity.
fn check_inventory(assembled: &AssembledLibrary) {
    let text = inventory(assembled);
    let path = Path::new(CORE_GOLDEN).join("inventory_1-4.txt");
    let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
    let identity = assembled.library.identity();
    if std::env::var_os("RUSTRED_P_BLESS").is_some() {
        std::fs::write(&path, &text).unwrap();
        println!("scope 1-4 inventory pin (\"1-4\", \"{identity}\", \"{hash}\")");
        return;
    }
    let golden = std::fs::read_to_string(&path).unwrap_or_default();
    let (held, found) = (counts_by_name(&golden), counts_by_name(&text));
    let differing: Vec<String> = (held.keys().chain(found.keys()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|name| held.get(*name) != found.get(*name))
        .map(|name| {
            format!(
                "{name}: golden {:?}, now {:?}",
                held.get(name),
                found.get(name)
            )
        })
        .collect();
    assert!(
        differing.is_empty(),
        "named counts differ from {}: {differing:#?}",
        path.display()
    );
    assert!(
        golden == text,
        "the scope 1-4 inventory differs from {}: bump the P_* constant whose meaning changed, \
         re-bless with RUSTRED_P_BLESS=1 and append a pin",
        path.display()
    );
    let pinned = INVENTORY_PINS
        .iter()
        .rev()
        .find(|(scope, ..)| *scope == "1-4")
        .expect("a pin for the scope 1-4 inventory");
    assert_eq!(
        (pinned.1, pinned.2),
        (identity.as_str(), hash.as_str()),
        "the scope 1-4 inventory changed while PLibrary::identity() did not: bump a version"
    );
}

/// The program of scope 1-4 from M3-T15's staged payloads (G1 except Diamond
/// active; G4's certificate): ForcerTree 438 nodes with Forcer's action table,
/// the Diamond nodes on the Custom lane, the census strongly complete over 437
/// node presentations and 310 classes (4,519 nonzero sectors of the 4L tops on
/// 183 classes), one owner per class with its verified edge maps or its
/// staged payload, the descent certificate; a short trace refuses nothing (0
/// MissingOwner); the golden inventory pins all of it to the identity.
#[test]
#[ignore = "slow: scope 1-4 assembly with census, completeness check and certificate; needs RUSTRED_P_OWNERS_1_4"]
fn scope_one_to_four_assembles_from_the_staged_owners() {
    let assembled = scope_four();
    let counts: BTreeMap<String, usize> = (named_counts(assembled).into_iter())
        .map(|c| (c.name, c.value))
        .collect();
    let at = |name: &str| counts.get(name).copied().unwrap_or(0);
    // G1 (M2 G1, G3): by name, before the golden.
    for (name, value) in [
        ("forcer_tree_nodes", 438),
        ("census_nodes", 437),
        ("census_classes", 310),
        ("l4_tops", 11),
        ("l4_top_nonzero_sectors", 4_519),
        ("l4_top_classes", 183),
        ("diamond_nodes_on_custom", 4),
        ("triangle_fallbacks", 0),
        ("forcer_action_L0_Trivial", 1),
        ("forcer_action_L1_Insertion", 6),
        ("forcer_action_L2_Insertion", 20),
        ("forcer_action_L2_Triangle", 4),
        ("forcer_action_L2_Custom", 5),
        ("forcer_action_L3_Insertion", 89),
        ("forcer_action_L3_Carpet", 10),
        ("forcer_action_L3_Triangle", 16),
        ("forcer_action_L3_Custom", 7),
        ("forcer_action_L4_Insertion", 220),
        ("forcer_action_L4_Carpet", 14),
        ("forcer_action_L4_Triangle", 33),
        ("forcer_action_L4_Diamond", 4),
        ("forcer_action_L4_Custom", 9),
    ] {
        assert_eq!(at(name), value, "{name}");
    }
    let lanes: BTreeMap<&str, usize> = [
        "SeriesParallel",
        "Insertion",
        "Carpet",
        "Triangle",
        "Custom",
        "Zero",
    ]
    .into_iter()
    .map(|lane| {
        let total = (0..=4).map(|l| at(&format!("node_lane_L{l}_{lane}"))).sum();
        (lane, total)
    })
    .collect();
    assert_eq!(
        lanes,
        BTreeMap::from([
            ("SeriesParallel", 161),
            ("Insertion", 174),
            ("Carpet", 24),
            ("Triangle", 53),
            ("Custom", 25),
            ("Zero", 1),
        ])
    );
    // Every Custom node's class has a staged payload, owned at its node.
    let library = &assembled.library;
    for (label, owner, _) in custom_owners(library) {
        assert!(assembled.custom.contains_key(&label), "{label}");
        assert!(owner.spec.payload.is_some() && owner.spec.custom.is_some());
    }
    assert_eq!(assembled.custom.len(), at("owners_Custom"));

    // A short trace of every loop count: nothing refused, no missing owner.
    let start = Instant::now();
    let rows = validate_library(
        assembled,
        &ValidateOptions {
            points: 50,
            ..ValidateOptions::default()
        },
    )
    .unwrap();
    for row in &rows {
        println!(
            "trace L{}: {} keys, {} Route + {} Apply keys ({} zero), {} owners applied, clean {}",
            row.loops,
            row.keys,
            row.report.route_keys,
            row.report.apply_keys,
            row.report.zero_keys,
            row.report.applied.len(),
            row.report.is_clean()
        );
        assert!(row.report.is_clean(), "L{}: {:?}", row.loops, row.report);
    }
    println!("trace {:.1} s", start.elapsed().as_secs_f64());

    for c in named_counts(assembled) {
        println!("count {} {}", c.name, c.value);
    }
    for row in custom_rows(assembled) {
        println!("{row}");
    }
    check_inventory(assembled);
}

// ---------------------------------------------------------------------------
// Per-rule firing on the witness boxes (I-5)
// ---------------------------------------------------------------------------

/// The RHS keys of `rule` at the key `powers`, if its target matches the key
/// (fixed slots equal, the face met): each free index `n<slot>` is the key's
/// power minus the target's offset.
fn instantiated(rule: &CustomRule, powers: &[i64]) -> Option<Vec<Vec<i64>>> {
    let face = rule
        .fixed
        .iter()
        .all(|&(axis, value)| powers[axis] == value);
    let mut free = vec![None; powers.len()];
    for (slot, power) in rule.target.iter().enumerate() {
        match *power {
            CustomPower::Fixed(value) if powers[slot] != value => return None,
            CustomPower::Fixed(_) => {}
            CustomPower::Symbolic(offset) => free[slot] = Some(powers[slot] - offset),
        }
    }
    if !face {
        return None;
    }
    // A term over a slot without a free index cannot be instantiated: left out.
    let terms = (rule.rhs.iter()).filter_map(|(rhs, _)| {
        (rhs.iter().enumerate())
            .map(|(slot, power)| match *power {
                CustomPower::Fixed(value) => Some(value),
                CustomPower::Symbolic(offset) => free[slot].map(|n| n + offset),
            })
            .collect::<Option<Vec<i64>>>()
    });
    Some(terms.collect())
}

/// Which rules fire on the witness box of one Custom owner, attributed from
/// the outside: a key's step applies one rule of its chart whose target and
/// face match the key and whose instantiated RHS holds every child of the
/// step. Unique: one such rule; ambiguous: several; unattributed: none (the
/// step's own dispatch is not visible, so this is a lower bound of firing).
#[derive(Debug, Default)]
struct Firing {
    rules: usize,
    keys: usize,
    terminal_keys: usize,
    faces_met: BTreeSet<usize>,
    fired: BTreeSet<usize>,
    possibly_fired: BTreeSet<usize>,
    ambiguous_keys: usize,
    unattributed_keys: usize,
}

fn firing(library: &PLibrary, owner: &POwner, support: u64) -> Firing {
    let spec = &owner.spec;
    let presentation = library.presentation(spec.presentation);
    let bytes = spec.payload.as_ref().unwrap().bytes().unwrap();
    let rules = custom_payload_rules(&bytes).unwrap();
    let OwnerProgram::Lane(step) = owner.program(library).unwrap() else {
        panic!("a Custom owner without program")
    };
    let limits = ReductionLimits::default();
    let cx = PStepContext {
        library,
        owner,
        limits: &limits,
    };
    let tag = GammaTag::one();
    let mut out = Firing {
        rules: rules.len(),
        ..Firing::default()
    };
    for powers in custom_witness_keys(
        presentation.slots.len(),
        support,
        presentation.forced_axes(),
    ) {
        out.keys += 1;
        let view = PKeyView {
            presentation: spec.presentation,
            powers: &powers,
            tag: &tag,
        };
        let step_out = step.step(&cx, &view).unwrap();
        if step_out.children.is_empty() && !step_out.terminals.is_empty() {
            out.terminal_keys += 1;
            continue;
        }
        let children: BTreeSet<&Vec<i64>> = step_out.children.iter().map(|c| &c.powers).collect();
        let chart = (powers.iter().enumerate())
            .filter(|&(_, &p)| p > 0)
            .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
        let mut matching = Vec::new();
        for (index, rule) in rules.iter().enumerate() {
            if rule.chart != chart {
                continue;
            }
            let Some(rhs) = instantiated(rule, &powers) else {
                continue;
            };
            out.faces_met.insert(index);
            if children.iter().all(|child| rhs.contains(child)) {
                matching.push(index);
            }
        }
        match matching.as_slice() {
            [] => out.unattributed_keys += 1,
            [one] => {
                out.fired.insert(*one);
                out.possibly_fired.insert(*one);
            }
            several => {
                out.ambiguous_keys += 1;
                out.possibly_fired.extend(several.iter().copied());
            }
        }
    }
    out
}

/// I-5 (D-T8a-15 follow-up): per Custom owner of scope 1-4, the rules that
/// fire on its certificate witness box (dots <= 2, rank <= 2), attributed by
/// their children. Recorded, with the faces the box meets; the certificate
/// itself (every child descends) is `scope_one_to_four_assembles_...`'s.
#[test]
#[ignore = "slow: every Custom witness box of scope 1-4; needs RUSTRED_P_OWNERS_1_4"]
fn the_four_loop_witness_boxes_fire_the_custom_rules() {
    let assembled = scope_four();
    let library = &assembled.library;
    let (mut rules, mut fired, mut possibly, mut faces, mut keys) = (0, 0, 0, 0, 0);
    for (label, owner, support) in custom_owners(library) {
        let start = Instant::now();
        let f = firing(library, owner, support);
        println!(
            "firing {label} {}: {} rules, faces met {}, fired (unique) {}, possibly fired {}, \
             keys {} (terminal {}, ambiguous {}, unattributed {}), {:.2} s",
            name_of(library.presentation(owner.spec.presentation)),
            f.rules,
            f.faces_met.len(),
            f.fired.len(),
            f.possibly_fired.len(),
            f.keys,
            f.terminal_keys,
            f.ambiguous_keys,
            f.unattributed_keys,
            start.elapsed().as_secs_f64()
        );
        assert!(f.keys > 0 && f.faces_met.len() <= f.rules);
        rules += f.rules;
        fired += f.fired.len();
        possibly += f.possibly_fired.len();
        faces += f.faces_met.len();
        keys += f.keys;
    }
    println!(
        "firing total: {rules} rules, faces met {faces}, fired (unique) {fired}, possibly fired {possibly}, {keys} keys"
    );
}

// ---------------------------------------------------------------------------
// Regeneration from the staged payloads (I-8, I-2) and the D-SIZE numbers
// ---------------------------------------------------------------------------

/// The memo counters of a kernel step: weights, then numerator tables.
type MemoReader = Box<dyn Fn() -> (MemoCounters, MemoCounters)>;

/// Resident set size of this process, KiB (`/proc/self/status`).
fn rss_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    (status.lines())
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kib| kib.parse().ok())
        .unwrap()
}

fn temporary(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("rr-assemble4-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// One 1L input (the bubble at unit powers) reduced onto the terminals.
fn first_result(library: &PLibrary) -> PReduction {
    let one = library.coefficient_context().one();
    library
        .reduce(PReductionRequest {
            inputs: vec![(
                PresentationId(0),
                IntegralKey::try_new([1, 1]).unwrap(),
                one,
            )],
            arithmetic: OrderedWeightArithmetic::Exact,
            scheduler: OrderedSchedulerOptions::default(),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap()
}

fn load(directory: &Path, verify_descent: bool) -> (Arc<PLibrary>, f64) {
    let start = Instant::now();
    let library = load_p_library(
        PLibrarySource::Directory(directory.to_path_buf()),
        &PLibraryOptions { verify_descent },
    )
    .unwrap();
    (library, start.elapsed().as_secs_f64())
}

/// The stored LP counts of the staged payloads against RustRed's counter:
/// the owners of at most [`RECOUNT_MAX_LINES`] lines, all with
/// `P_LP_RECOUNT_ALL=1` (I-2, D-M3-3). Returns `(recounted, skipped)`.
fn recount_lp(library: &PLibrary) -> (usize, usize) {
    let all = std::env::var("P_LP_RECOUNT_ALL").is_ok_and(|value| value == "1");
    let (mut recounted, mut skipped) = (0, 0);
    for (label, owner, support) in custom_owners(library) {
        let presentation = library.presentation(owner.spec.presentation);
        let lines = lines_of(presentation, support);
        if lines > RECOUNT_MAX_LINES && !all {
            skipped += 1;
            continue;
        }
        let start = Instant::now();
        let lp = custom_owner_lp_count(presentation, support).unwrap();
        let stored = owner.spec.custom.as_ref().unwrap().lp_count;
        println!(
            "LP recount {label} {} ({lines} lines): {lp}, stored {stored}, {:.2} s",
            name_of(presentation),
            start.elapsed().as_secs_f64()
        );
        assert_eq!(
            lp, stored,
            "{label}: RustRed's count differs from the stored LP"
        );
        recounted += 1;
    }
    (recounted, skipped)
}

/// The regeneration test of scope 1-4 (I-8; D-M3-3): the staged payloads with
/// their stored LP (recounted for the owners of at most 9 lines), the package
/// written and loaded back once under `verify_descent` (certificate, strong
/// completeness), the same program. Recorded for D-SIZE (D-M3-6), not
/// criteria: bytes per part, load time with and without the certificate, the
/// time to the first result of a 1L input against the 1-4 and the 1-3
/// package, CustomStep resident size per owner (lazy, eager) and the kernel
/// memos of the 4L kernel owners over their witness boxes (I-19).
#[test]
#[ignore = "slow: scope 1-4 package written and certified; needs RUSTRED_P_OWNERS_1_4"]
fn the_scope_one_to_four_package_regenerates_from_the_staged_owners() {
    let assembled = scope_four();
    let library = &assembled.library;
    let (recounted, skipped) = recount_lp(library);
    println!(
        "LP recounted for {recounted} owners, {skipped} owners of more than {RECOUNT_MAX_LINES} lines skipped"
    );

    // The package: written, then loaded once with the certificate.
    let files = p_library_files(assembled).unwrap();
    let mut parts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (name, bytes) in &files {
        let part = name.split('/').next().unwrap();
        let entry = parts.entry(part).or_default();
        entry.0 += 1;
        entry.1 += bytes.len();
    }
    let total: usize = files.iter().map(|(_, b)| b.len()).sum();
    println!(
        "package parts (files, bytes): {parts:?}, total {total} bytes in {} files",
        files.len()
    );
    let directory = temporary("package");
    write_p_library_files(&files, &directory).unwrap();
    let (unverified, without) = load(&directory, false);
    let (verified, with) = load(&directory, true);
    println!("load release: without the certificate {without:.2} s, with {with:.2} s");
    assert_eq!(
        p_library_differences(library, &verified).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(
        p_library_differences(library, &unverified).unwrap(),
        Vec::<String>::new()
    );
    drop(verified);

    // The first result of one 1L input: the 1-4 package against a 1-3 package.
    let three = temporary("package-1-3");
    let scope_three = assemble_p_package(
        &families(),
        3,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions {
            threads: workers(),
            verify_descent: false,
        },
    )
    .unwrap();
    write_p_library_files(&p_library_files(&scope_three).unwrap(), &three).unwrap();
    let mut results = Vec::new();
    for (scope, package) in [("1-3", &three), ("1-4", &directory)] {
        let start = Instant::now();
        let (loaded, _) = load(package, false);
        let reduction = first_result(&loaded);
        println!(
            "first 1L result against the {scope} package: {:.3} s (load + reduce)",
            start.elapsed().as_secs_f64()
        );
        results.push(reduction.terms);
    }
    assert_eq!(
        results[0], results[1],
        "the 1L bubble differs between the programs"
    );

    // CustomStep resident size per owner: lazy, then every chart prepared.
    let mut steps: Vec<Arc<dyn PLaneStep>> = Vec::new();
    for (label, owner, _) in custom_owners(&unverified) {
        let spec = &owner.spec;
        let presentation = unverified.presentation(spec.presentation);
        let bytes = spec.payload.as_ref().unwrap().bytes().unwrap();
        let meta = spec.custom.as_ref().unwrap();
        let before = rss_kib();
        steps.push(build_custom_step(presentation, &bytes, meta).unwrap());
        let lazy = rss_kib();
        let start = Instant::now();
        steps.push(build_custom_step_prepared(presentation, &bytes, meta).unwrap());
        println!(
            "CustomStep {label} {}: payload {} bytes, resident lazy {} KiB, eager {} KiB, prepared in {:.2} s",
            name_of(presentation),
            bytes.len(),
            lazy.saturating_sub(before),
            rss_kib().saturating_sub(lazy),
            start.elapsed().as_secs_f64()
        );
    }
    drop(steps);

    // The kernel memos of the 4L kernel owners over their witness boxes.
    let limits = ReductionLimits::default();
    let tag = GammaTag::one();
    let (mut owners, mut keys) = (0, 0);
    let (mut weights, mut tables) = (MemoCounters::default(), MemoCounters::default());
    let before = rss_kib();
    let start = Instant::now();
    let mut held: Vec<Arc<dyn PLaneStep>> = Vec::new();
    for owner in unverified.owners() {
        let spec = &owner.spec;
        let presentation = unverified.presentation(spec.presentation);
        if presentation.loops != 4 {
            continue;
        }
        let (step, counters): (Arc<dyn PLaneStep>, MemoReader) = match spec.lane {
            OwnerLane::SeriesParallel => {
                let step = Arc::new(SeriesParallelStep::from_owner(&unverified, spec).unwrap());
                let held = Arc::clone(&step);
                (step, Box::new(move || held.memo_counters()))
            }
            OwnerLane::Insertion => {
                let step = Arc::new(InsertionStep::from_owner(&unverified, spec).unwrap());
                let held = Arc::clone(&step);
                (step, Box::new(move || held.memo_counters()))
            }
            OwnerLane::Carpet => {
                let step = Arc::new(CarpetStep::from_owner(&unverified, spec).unwrap());
                let held = Arc::clone(&step);
                (step, Box::new(move || held.memo_counters()))
            }
            _ => continue,
        };
        let support = unverified.census().classes[&spec.class].owner.1;
        let cx = PStepContext {
            library: &unverified,
            owner,
            limits: &limits,
        };
        for powers in custom_witness_keys(presentation.slots.len(), support, 0) {
            let view = PKeyView {
                presentation: spec.presentation,
                powers: &powers,
                tag: &tag,
            };
            step.step(&cx, &view).unwrap();
            keys += 1;
        }
        let (w, t) = counters();
        weights = weights + w;
        tables = tables + t;
        owners += 1;
        held.push(step);
    }
    println!(
        "kernel memos at 4L: {owners} owners, {keys} witness keys stepped in {:.1} s; weights {weights:?}, \
         numerator tables {tables:?} (entries = misses below 2^16 per memo); resident growth {} KiB",
        start.elapsed().as_secs_f64(),
        rss_kib().saturating_sub(before)
    );
    drop(held);
    let _ = std::fs::remove_dir_all(&directory);
    let _ = std::fs::remove_dir_all(&three);
}

// ---------------------------------------------------------------------------
// Review S2 of M2-T8a: a Custom Route child on the owner support
// ---------------------------------------------------------------------------

/// A real Custom step that adds a Route child with the key's own powers: on
/// the owner support, so not on a strict subsector.
struct RouteOnSupport {
    inner: Arc<dyn PLaneStep>,
}

impl PLaneStep for RouteOnSupport {
    fn lane(&self) -> OwnerLane {
        OwnerLane::Custom
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let mut out = self.inner.step(cx, key)?;
        out.children.push(PChild {
            stage: PStage::Route,
            presentation: key.presentation,
            powers: key.powers.to_vec(),
            tag: key.tag.clone(),
            coefficient: cx.library.coefficient_context().one(),
        });
        Ok(out)
    }

    fn order_key(&self, cx: &PStepContext<'_>, powers: &[i64]) -> Result<POrderKey, PError> {
        self.inner.order_key(cx, powers)
    }
}

/// `library` rebuilt with `program` for the owner of `class`.
fn rebuilt_with(library: &PLibrary, class: OwnerClassId, program: OwnerProgram) -> PLibrary {
    let mut builder = PLibrary::builder(library.coefficient_context().clone(), library.scope());
    for presentation in library.presentations() {
        builder
            .add_presentation(Presentation::clone(presentation))
            .unwrap();
    }
    builder
        .set_topology(Arc::clone(library.topology().unwrap()))
        .unwrap();
    builder.set_census(library.census().clone()).unwrap();
    for map in library.edge_maps() {
        builder.add_edge_map(map.clone()).unwrap();
    }
    for terminal in library.terminals() {
        builder.add_terminal(terminal.clone()).unwrap();
    }
    let mut program = Some(program);
    for owner in library.owners() {
        if owner.spec.class == class {
            builder
                .add_owner_with_program(owner.spec.clone(), program.take().unwrap())
                .unwrap();
        } else {
            builder.add_owner(owner.spec.clone()).unwrap();
        }
    }
    builder.build().unwrap()
}

/// Scope 1-2 with the 2L kite presented as Custom (a Triangle mismatch, its
/// owner generated in seconds): a small library with a Custom owner.
fn kite() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let mismatch = |p: &Presentation, _: &TriangleRoles| {
            Err(TemplateError::Mismatch(TemplateMismatch {
                presentation: p.id,
                reason: "test: the kite as a Custom owner".to_owned(),
            }))
        };
        assemble_library_with(
            &families(),
            2,
            CustomPayloads::Generate(CustomOwnerOptions::default()),
            &AssembleOptions {
                threads: 1,
                verify_descent: true,
            },
            &mismatch,
        )
        .unwrap()
    })
}

/// Item 2 of the certificate (review S2, mutant S2_WIT_SUBSET_EQ of M2-T8a): a
/// Custom step's Route child must lie on a STRICT subsector of the owner
/// support. The kite's Custom owner is certified as it is and refused with a
/// Route child that keeps the key's own support.
#[test]
fn a_custom_route_child_on_the_owner_support_does_not_descend() {
    let library = &kite().library;
    let owner = (library.owners())
        .find(|owner| owner.spec.lane == OwnerLane::Custom)
        .unwrap();
    assert!(matches!(
        library.presentation(owner.spec.presentation).kind,
        PresentationKind::Induced { .. }
    ));
    let OwnerProgram::Lane(real) = owner.program(library).unwrap() else {
        unreachable!()
    };
    let damaged = rebuilt_with(
        library,
        owner.spec.class,
        OwnerProgram::Lane(Arc::new(RouteOnSupport {
            inner: Arc::clone(real),
        })),
    );
    let error = descent::check(&damaged).unwrap_err();
    assert!(
        matches!(&error, PError::MalformedLibrary(m)
            if m.contains("the Route child") && m.contains("does not descend")),
        "{error}"
    );
    // The same parts with the real program pass.
    descent::check(&rebuilt_with(
        library,
        owner.spec.class,
        OwnerProgram::Lane(Arc::clone(real)),
    ))
    .unwrap();
}

// ---------------------------------------------------------------------------
// Provided payloads from an owner directory of `rustred p-generate`
// ---------------------------------------------------------------------------

/// A slot mask as `rustred p-generate` writes it: one `0`/`1` per slot.
fn mask_text(mask: u64, arity: usize) -> String {
    (0..arity)
        .map(|slot| if mask >> slot & 1 == 1 { '1' } else { '0' })
        .collect()
}

/// The owner record `rustred p-generate` writes for `payload` (the fields the
/// reader checks, cli/p/generate.rs).
fn owner_record(label: &str, payload: &CustomOwnerPayload) -> serde_json::Value {
    let summary = inspect_custom_payload(&payload.bytes).unwrap();
    let arity = summary.arity;
    serde_json::json!({
        "schema": "rustred.p-custom-owner.v1",
        "label": label,
        "arity": arity,
        "support": mask_text(summary.support, arity),
        "lp_count": payload.meta.lp_count,
        "residuals": payload.meta.residuals,
        "two_sided_axes": mask_text(payload.meta.two_sided_axes, arity),
        "charts": summary.charts.iter().map(|chart| serde_json::json!({
            "chart": mask_text(chart.chart, arity),
            "rules": chart.rules,
        })).collect::<Vec<_>>(),
        "versions": {
            "P_CUSTOM_OWNER_VERSION": P_CUSTOM_OWNER_VERSION,
            "P_OWNER_LIBRARY_SEMANTICS": P_OWNER_LIBRARY_SEMANTICS,
        },
    })
}

/// D-T8c4-1: `read_custom_payloads` of a directory without `library.json`
/// reads `rustred p-generate` owner records (M3-T15's staging of the scope 1-4
/// owners): the kite's record gives its payload and meta verbatim, with the
/// stored LP; other files are not read. A record of another schema or version,
/// another label, a record that does not describe its payload, the false
/// knock-out guard and a directory without a record are refused.
#[test]
fn owner_records_of_p_generate_are_provided_payloads() {
    let (label, payload) = kite().custom.iter().next().unwrap();
    let directory = temporary("owners");
    let write = |record: &serde_json::Value| {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{label}.rrbin")), &payload.bytes).unwrap();
        std::fs::write(
            directory.join(format!("{label}.json")),
            serde_json::to_vec_pretty(record).unwrap(),
        )
        .unwrap();
        std::fs::write(directory.join("README.md"), "not a record").unwrap();
        std::fs::write(directory.join("index.json"), "[]").unwrap();
        read_custom_payloads(&PLibrarySource::Directory(directory.clone()))
    };
    let record = owner_record(label, payload);
    assert_eq!(
        write(&record).unwrap(),
        BTreeMap::from([(label.clone(), payload.clone())])
    );
    let edited = |pointer: &str, value: serde_json::Value| {
        let mut record = record.clone();
        *record.pointer_mut(pointer).unwrap() = value;
        record
    };
    for (record, needle) in [
        (edited("/schema", "other".into()), "schema"),
        (
            edited("/versions/P_CUSTOM_OWNER_VERSION", 99.into()),
            "P_CUSTOM_OWNER_VERSION",
        ),
        (edited("/label", "L2T05-0".into()), "another label"),
        (edited("/residuals", 1.into()), "records"),
        (edited("/support", "00000".into()), "records"),
        (edited("/lp_count", 1.into()), "false knock-out guard"),
    ] {
        let error = write(&record).unwrap_err();
        assert!(error.to_string().contains(needle), "{needle}: {error}");
    }
    std::fs::remove_dir_all(&directory).unwrap();
    std::fs::create_dir_all(&directory).unwrap();
    let error = read_custom_payloads(&PLibrarySource::Directory(directory.clone())).unwrap_err();
    assert!(error.to_string().contains("neither"), "{error}");
    std::fs::remove_dir_all(&directory).unwrap();
}
