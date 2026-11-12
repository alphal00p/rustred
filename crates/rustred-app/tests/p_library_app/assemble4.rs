//! The scope 1-4 library program (owner: M3-T8c rr-assemble4).
//!
//! STAGE 1 assembles the program of the built-in tops with 1-4 loops, the
//! Diamond nodes on the Diamond lane (M3-T5b; on the Custom lane before it,
//! plan M3 D-M3-2), from the Custom payloads
//! M3-T15 generated once and staged with their LP counts (D-M3-3): the owner
//! directory named by `RUSTRED_P_OWNERS_1_4` (`rustred p-generate` records,
//! read by `read_custom_payloads` as `CustomPayloads::Provided`). Checked: the
//! named counts and Custom owners of the golden `inventory_1-4.txt` (in
//! rustred-core's golden directory, beside M3-T15's `owners_1-4.tsv`), the
//! descent certificate, a short trace, per-rule firing on the Custom witness
//! boxes (I-5), the regeneration of the package from the staged payloads with
//! M3-I1's LP recount of the owners of at most 9 lines (every owner only with
//! `P_LP_RECOUNT_ALL=1`) and one load under `verify_descent` (I-8), and the
//! numbers of the D-SIZE decision (load times, first result, resident sizes).
//! STAGE 2 adds M3-T17's basis layer: the program under test is the one
//! `rustred p-package` writes (`assemble_p_package_recounted`, the layer onto
//! Forcer's 16 masters of `P_BASIS_MASTERS_1_TO_4` derived and attached), and
//! the inventory lists the layer's rows. The race of D-M3-2 ended in C-D
//! (M3-T5b not merged before M3-T17), so the M3 package kept the 4 Diamond
//! nodes d384, d394, d399 and no3 as Custom owners; since M3-T5b they are on
//! the Diamond lane (their staged payloads unused, 21 Custom owners; M5 combined:
//! no1, haha, bubu on the Scheme lane, 18 Custom owners).
//! The 4L tests are `#[ignore]` and run optimised once (notes/rr-assemble4.md).
//! The certificate's refusal of a Custom Route child on the owner support
//! (review S2 of M2-T8a, `RouteOnSupport`) runs in the gate at scope 1-2.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::family::IntegralKey;
use rustred::propagator::basis::{PBasisLayer, coefficient_text};
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
    OwnerProgram, PBasis, PLibrary, POwner, PReduction, PReductionRequest, PUncoveredPolicy,
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
use rustred_app::propagator::assemble::{
    LpRecount, RecountProvidedLp, ValidateOptions, assemble_library_with, validate_library,
};
use rustred_app::propagator::loader::read_custom_payloads;
use rustred_app::propagator::package::{
    assemble_p_package, assemble_p_package_recounted, p_library_differences, p_library_files,
    write_p_library_files,
};
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomOwnerOptions, CustomOwnerPayload, CustomPayloads,
    PLibraryOptions, PLibrarySource, load_p_library,
};

// ---------------------------------------------------------------------------
// The program
// ---------------------------------------------------------------------------

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
/// certificate, M3-I1's LP recount of the payloads (the owners of at most 9
/// lines; every owner only with `P_LP_RECOUNT_ALL=1`, D-M3-3) and its basis
/// layer onto the 4L masters (`assemble_p_package_recounted`: the program
/// `rustred p-package --recount-lp` writes); assembled once per test process.
fn scope_four() -> &'static (AssembledLibrary, Vec<LpRecount>) {
    static LIBRARY: OnceLock<(AssembledLibrary, Vec<LpRecount>)> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let payloads = read_custom_payloads(&PLibrarySource::Directory(owners_directory()))
            .expect("the staged owners read back");
        println!("{} staged Custom payloads", payloads.len());
        let start = Instant::now();
        let recount = RecountProvidedLp::from_env();
        let (assembled, recounts) = assemble_p_package_recounted(
            &families(),
            4,
            CustomPayloads::Provided(payloads),
            &AssembleOptions {
                threads: workers(),
                verify_descent: true,
            },
            &recount,
        )
        .expect("scope 1-4 assembles");
        println!(
            "scope 1-4 assembled in {:.1} s on {} census threads (LP recount up to {:?} lines): {:?}",
            start.elapsed().as_secs_f64(),
            workers(),
            recount.max_lines(),
            assembled.timings
        );
        (assembled, recounts)
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
        "DAG nodes with the Forcer action Diamond presented on the Custom lane (D-M3-2; 0 since the Diamond lane of M3-T5b)",
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
    if let Some(layer) = PBasisLayer::from_library(library).unwrap() {
        counts.push(count(
            "layer_rows",
            layer.rows().len(),
            "rows of the basis layer, one per terminal (M3-T17: Unit, then each residual onto Forcer's masters)",
        ));
        counts.push(count(
            "layer_masters",
            layer.masters().len(),
            "distinct masters named by the layer's rows (of P_BASIS_MASTERS_1_TO_4 at scope 1-4)",
        ));
        counts.push(count(
            "layer_rows_one_master_term",
            (layer.rows().iter())
                .filter(|row| {
                    matches!(row.terms.as_slice(), [term] if matches!(term.basis, PBasis::Master(_)))
                })
                .count(),
            "residual rows that are a single master term (a square system whose matrix is a permutation)",
        ));
    }
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
/// <definition>`), the Custom owners (checked against M3-T15's
/// `owners_1-4.tsv` by [`check_owners_tsv`]), the basis layer's rows.
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
    for row in custom_rows(assembled) {
        writeln!(out, "{row}").unwrap();
    }
    for row in layer_rows(&assembled.library) {
        writeln!(out, "{row}").unwrap();
    }
    out
}

/// One line per row of the program's basis layer (none without a layer):
/// the terminal (id, class, node, key), then its terms (coefficient in
/// `Q(d)`, Gamma tag exponents, `1` or the master).
fn layer_rows(library: &PLibrary) -> Vec<String> {
    let Some(layer) = PBasisLayer::from_library(library).unwrap() else {
        return Vec::new();
    };
    (layer.rows().iter())
        .map(|row| {
            let terms: Vec<String> = (row.terms.iter())
                .map(|term| {
                    let basis = match &term.basis {
                        PBasis::Master(name) => name.clone(),
                        other => format!("{other:?}"),
                    };
                    format!(
                        "({}) tag {:?} {basis}",
                        coefficient_text(&term.coefficient),
                        term.tag.exponents()
                    )
                })
                .collect();
            format!(
                "layer {} {} node {} key {:?} = {}",
                row.terminal.id.0,
                row.terminal.class.as_deref().unwrap_or("unit"),
                row.terminal.node.map_or(-1, |n| i64::from(n.0)),
                row.terminal.key,
                terms.join(" + ")
            )
        })
        .collect()
}

/// M3-T15's committed owner table `owners_1-4.tsv` describes the Custom
/// owners of the program, row by row: one row per Custom owner, with its
/// Forcer name, lane (`Custom(diamond)` exactly on the Diamond nodes), node,
/// loops, lines, eps lines, family, support, two-sided axes, charts, rules,
/// residuals, LP and payload bytes. (The payload sha256 column is checked
/// against the staged files by `sha256sum -c`, notes/rr-assemble4.md.) The
/// `Custom(diamond)` rows are on the Diamond lane now (M3-T5b): no payload.
fn check_owners_tsv(assembled: &AssembledLibrary) {
    let library = &assembled.library;
    let path = Path::new(CORE_GOLDEN).join("owners_1-4.tsv");
    let text = std::fs::read_to_string(&path).expect("M3-T15's owners_1-4.tsv");
    let mut rows = text.lines().filter(|line| !line.starts_with('#'));
    let header: Vec<&str> = rows.next().unwrap().split('\t').collect();
    let column = |name: &str| header.iter().position(|c| *c == name).unwrap();
    let owners: BTreeMap<String, (&POwner, u64)> = (custom_owners(library).into_iter())
        .map(|(label, owner, support)| (label, (owner, support)))
        .collect();
    let mut seen = BTreeSet::new();
    let mut diamonds = 0;
    let mut schemes = 0;
    for row in rows {
        let cells: Vec<&str> = row.split('\t').collect();
        let cell = |name: &str| cells[column(name)];
        let label = cell("class");
        if cell("lane") == "Custom(diamond)" {
            let census = library.census();
            let owner = (library.owners())
                .find(|o| census.classes[&o.spec.class].key.label() == label)
                .unwrap_or_else(|| panic!("{label}: no owner"));
            assert_eq!(owner.spec.lane, OwnerLane::Diamond, "{label}");
            assert!(owner.spec.payload.is_none(), "{label}");
            assert!(seen.insert(label.to_owned()), "{label} twice");
            diamonds += 1;
            continue;
        }
        // M5 combined: no1, haha and bubu are Scheme owners by default (their
        // staged Custom payloads are not used); M5 batch 2: no2 and no6 too
        // (no6's payload is its Scheme owner's fallback).
        if !owners.contains_key(label) {
            let census = library.census();
            let owner = (library.owners())
                .find(|o| census.classes[&o.spec.class].key.label() == label)
                .unwrap_or_else(|| panic!("{label}: no owner"));
            assert_eq!(owner.spec.lane, OwnerLane::Scheme, "{label}");
            assert!(
                ["no1", "haha", "bubu", "no2", "no6"].contains(&cell("forcer")),
                "{label}: {} on the Scheme lane",
                cell("forcer")
            );
            assert!(seen.insert(label.to_owned()), "{label} twice");
            schemes += 1;
            continue;
        }
        let (owner, support) = owners[label];
        let spec = &owner.spec;
        let presentation = library.presentation(spec.presentation);
        let node = spec.node.unwrap();
        let diamond = assembled.dag.nodes[node.0 as usize].action.kind() == ActionKind::Diamond;
        let meta = spec.custom.as_ref().unwrap();
        let bytes = spec.payload.as_ref().unwrap().bytes().unwrap();
        let summary = inspect_custom_payload(&bytes).unwrap();
        let arity = presentation.slots.len();
        let held = [
            name_of(presentation),
            (if diamond { "Custom(diamond)" } else { "Custom" }).to_owned(),
            node.0.to_string(),
            presentation.loops.to_string(),
            lines_of(presentation, support).to_string(),
            presentation.forced_axes().count_ones().to_string(),
            presentation.family.name().to_owned(),
            mask_text(support, arity),
            mask_text(meta.two_sided_axes, arity),
            meta.charts.len().to_string(),
            summary.rules().to_string(),
            meta.residuals.to_string(),
            meta.lp_count.to_string(),
            bytes.len().to_string(),
        ];
        let recorded = [
            "forcer",
            "lane",
            "node",
            "loops",
            "lines",
            "eps_lines",
            "family",
            "support",
            "two_sided_axes",
            "charts",
            "rules",
            "residuals",
            "lp",
            "payload_bytes",
        ]
        .map(|name| cell(name).to_owned());
        assert_eq!(
            held,
            recorded,
            "{label}: the program against {}",
            path.display()
        );
        assert!(seen.insert(label.to_owned()), "{label} twice");
    }
    assert_eq!(diamonds, 4, "owners_1-4.tsv: the Diamond nodes");
    assert_eq!(schemes, 5, "owners_1-4.tsv: the Scheme nodes");
    assert_eq!(
        seen.len(),
        owners.len() + diamonds + schemes,
        "owners_1-4.tsv rows against Custom owners"
    );
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
/// 1-3 pins of M2-T8a: a new inventory of the program needs a new identity,
/// and two pins never share an identity. The STAGE 1 pin (never pushed) was
/// replaced in STAGE 2, same identity: the inventory gained the basis layer's
/// rows, which act after the engine and are not part of the identity (they are
/// re-derived at load under `verify_descent`; D-T8c4-7). The second pin is
/// M3-T5b's Diamond lane (P_DIAMOND_TEMPLATE_VERSION in the identity: the 4
/// Diamond nodes on the Diamond lane, 21 Custom owners).
const INVENTORY_PINS: &[(&str, &str, &str)] = &[
    (
        "1-4",
        "p-library:scope=1-4:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "b006e4a0e7193a65d1ae683c6657dd42c27cbd7228cd412203908c92b077ac54",
    ),
    (
        "1-4",
        "p-library:scope=1-4:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=1,P_PRESENTATION_VERSION=1,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_DIAMOND_TEMPLATE_VERSION=1,P_SCHEME_TEMPLATE_VERSION=1,P_CUSTOM_OWNER_VERSION=1,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "e22393316fe87ca20ca9b7e94b342a55f27f4555f3f194e290186a31ec22d94f",
    ),
    // M5 combined: Forcer's schemes for no1, haha, bubu on by default
    // (P_SCHEME_TEMPLATE_VERSION 2: Scheme 3, Custom 18, the M3-T17 layer of 17
    // rows), route maps (a) and the ISP choice (P_CENSUS_VERSION 2,
    // P_PRESENTATION_VERSION 2), Custom owner version 2 (M5 B2).
    (
        "1-4",
        "p-library:scope=1-4:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=2,P_PRESENTATION_VERSION=2,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_DIAMOND_TEMPLATE_VERSION=1,P_SCHEME_TEMPLATE_VERSION=2,P_CUSTOM_OWNER_VERSION=2,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "532a3a6a40708455ea9fa22d2b2faf215f8f46310a3df017c35012d4a45dfb9e",
    ),
    // M5 batch 2: Scheme 5 (no2, no6 with its Custom fallback), Custom 16 (bebe and
    // cross from Astra's combo3), orbits/route maps in the package
    // (P_SCHEME_TEMPLATE_VERSION 4).
    (
        "1-4",
        "p-library:scope=1-4:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=2,P_PRESENTATION_VERSION=2,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_DIAMOND_TEMPLATE_VERSION=1,P_SCHEME_TEMPLATE_VERSION=4,P_CUSTOM_OWNER_VERSION=2,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "7025792b23b3d6aea9c2ed61be1d89f33dc87e78c9aae790128dfee9611e6b02",
    ),
    // M5 batch 4: route-basis ISP preferences (P_PRESENTATION_VERSION 3); same
    // owners and counts, new identity.
    (
        "1-4",
        "p-library:scope=1-4:P_OWNER_LIBRARY_SEMANTICS=rustred-p-owner-library-2026-11-05,P_CLASS_KEY_VERSION=1,P_CENSUS_VERSION=2,P_PRESENTATION_VERSION=3,P_FAMILIES_VERSION=1,P_GAMMA_TAG_VERSION=1,P_GAMMA_TAG_MAX_M=8,P_GAMMA_TAG_WIDTH=16,P_INSERTION_KERNEL_VERSION=1,P_CARPET_KERNEL_VERSION=1,P_TRIANGLE_TEMPLATE_VERSION=1,P_DIAMOND_TEMPLATE_VERSION=1,P_SCHEME_TEMPLATE_VERSION=4,P_CUSTOM_OWNER_VERSION=2,P_LIBRARY_SCHEMA=rustred.p-owner-library.v1,P_FAMILIES_SCHEMA=rustred.p-families.v1,P_BASIS_LAYER_SCHEMA=rustred.p-basis-layer.v1,P_TOPOLOGY_VERSION=1",
        "9ab626505bf812796907fbf01b0c1a84a56f274421a2fa0c25da7292430c3201",
    ),
];

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

/// The program of scope 1-4 from M3-T15's staged payloads (G1, the Diamond
/// lane active since M3-T5b; G4's certificate): ForcerTree 438 nodes with
/// Forcer's action table, the Diamond nodes on the Diamond lane (their staged
/// payloads unused), the census strongly complete over 437 node presentations
/// and 310 classes (4,519 nonzero sectors of the 4L tops on 183 classes), one
/// owner per class with its verified edge maps or its staged payload, the
/// descent certificate, M3-T17's basis layer (17 rows onto the 16 masters); a
/// short trace refuses nothing (0 MissingOwner); the golden inventory pins all
/// of it to the identity.
#[test]
#[ignore = "slow: scope 1-4 assembly with census, completeness check and certificate; needs RUSTRED_P_OWNERS_1_4"]
fn scope_one_to_four_assembles_from_the_staged_owners() {
    let (assembled, _) = scope_four();
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
        ("diamond_nodes_on_custom", 0),
        ("triangle_fallbacks", 0),
        // M3-T17's layer: Unit + 16 residuals, each one master term. (M5 B2's
        // rank-gated no1 added a second no1 residual; M5 combined puts no1 on
        // the Scheme lane by default, so the layer is M3-T17's again.)
        ("terminals", 17),
        ("layer_rows", 17),
        ("layer_masters", 16),
        ("layer_rows_one_master_term", 16),
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
        "Diamond",
        "Scheme",
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
            ("Diamond", 4),
            // Forcer's schemes no1, haha, bubu (M5 combined: on by default), no2
            // and no6 (M5 batch 2, P_SCHEME_TEMPLATE_VERSION 4).
            ("Scheme", 5),
            ("Custom", 16),
            ("Zero", 1),
        ])
    );
    // Every Custom node's class has a staged payload, owned at its node.
    let library = &assembled.library;
    for (label, owner, _) in custom_owners(library) {
        assert!(assembled.custom.contains_key(&label), "{label}");
        assert!(owner.spec.payload.is_some() && owner.spec.custom.is_some());
    }
    // The Custom owners' payloads, plus no6's: its Scheme owner carries the
    // shipped Custom no6 rules as its fallback (M5 batch 2).
    assert_eq!(assembled.custom.len(), at("owners_Custom") + 1);

    // A short trace of every loop count: nothing refused, no missing owner.
    let start = Instant::now();
    let rows = validate_library(
        assembled,
        &ValidateOptions {
            points: 10,
            max_dots: 1,
            max_rank: 2,
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
    check_owners_tsv(assembled);
    // The staged package (`rustred p-package --max-loops 4 --recount-lp`;
    // STAGE 2: the final package with its layer), when named, is this program.
    if std::env::var_os("RUSTRED_P_LIBRARY_1_4").is_some() {
        assert_eq!(
            p_library_differences(library, &staged_package()).unwrap(),
            Vec::<String>::new()
        );
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
/// A face is met when some key of the box matches the rule's target and fixed
/// slots (not its case equations). Keys whose step has no child are counted
/// apart (terminal keys: residuals only; zero keys: nothing), since every
/// rule meeting them would "match".
#[derive(Debug, Default)]
struct Firing {
    rules: usize,
    keys: usize,
    terminal_keys: usize,
    zero_keys: usize,
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
        let chart = (powers.iter().enumerate())
            .filter(|&(_, &p)| p > 0)
            .fold(0_u64, |mask, (slot, _)| mask | 1 << slot);
        let mut faced = Vec::new();
        for (index, rule) in rules.iter().enumerate() {
            if rule.chart != chart {
                continue;
            }
            if let Some(rhs) = instantiated(rule, &powers) {
                out.faces_met.insert(index);
                faced.push((index, rhs));
            }
        }
        if step_out.children.is_empty() {
            if step_out.terminals.is_empty() {
                out.zero_keys += 1;
            } else {
                out.terminal_keys += 1;
            }
            continue;
        }
        let children: BTreeSet<&Vec<i64>> = step_out.children.iter().map(|c| &c.powers).collect();
        let matching: Vec<usize> = (faced.iter())
            .filter(|(_, rhs)| children.iter().all(|child| rhs.contains(child)))
            .map(|(index, _)| *index)
            .collect();
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
/// their children. Asserted: the box meets every rule's face and every step
/// with children is attributed to a rule; recorded: fired (unique) and
/// possibly fired. The certificate itself (every child descends) is
/// `scope_one_to_four_assembles_...`'s.
#[test]
#[ignore = "slow: every Custom witness box of scope 1-4; needs RUSTRED_P_OWNERS_1_4"]
fn the_four_loop_witness_boxes_fire_the_custom_rules() {
    let (assembled, _) = scope_four();
    let library = &assembled.library;
    let (mut rules, mut fired, mut possibly, mut faces, mut keys) = (0, 0, 0, 0, 0);
    for (label, owner, support) in custom_owners(library) {
        let start = Instant::now();
        let f = firing(library, owner, support);
        println!(
            "firing {label} {}: {} rules, faces met {}, fired (unique) {}, possibly fired {}, \
             keys {} (terminal {}, zero {}, ambiguous {}, unattributed {}), {:.2} s",
            name_of(library.presentation(owner.spec.presentation)),
            f.rules,
            f.faces_met.len(),
            f.fired.len(),
            f.possibly_fired.len(),
            f.keys,
            f.terminal_keys,
            f.zero_keys,
            f.ambiguous_keys,
            f.unattributed_keys,
            start.elapsed().as_secs_f64()
        );
        assert!(f.keys > 0, "{label}: an empty witness box");
        assert_eq!(f.faces_met.len(), f.rules, "{label}: a rule's face unmet");
        assert_eq!(f.unattributed_keys, 0, "{label}: a step no rule explains");
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

/// glibc's `struct mallinfo2` (only two fields are read).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[allow(dead_code)]
#[repr(C)]
struct MallInfo2 {
    arena: usize,
    ordblks: usize,
    smblks: usize,
    hblks: usize,
    hblkhd: usize,
    usmblks: usize,
    fsmblks: usize,
    uordblks: usize,
    fordblks: usize,
    keepcost: usize,
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallinfo2() -> MallInfo2;
}

/// Heap bytes in use (glibc `mallinfo2`: allocated chunks plus mmapped
/// blocks), a measure that freed-but-resident memory does not blur, unlike
/// the RSS. glibc reports the main arena only: run with `MALLOC_ARENA_MAX=1`
/// (tools/release_tests.sh) so that every thread allocates there.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn heap_bytes() -> usize {
    // SAFETY: mallinfo2 takes no argument and returns a plain struct.
    let info = unsafe { mallinfo2() };
    info.uordblks + info.hblkhd
}

/// Without glibc there is no `mallinfo2`: the measures read 0.
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn heap_bytes() -> usize {
    0
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1 << 20) as f64
}

/// Keys of a kernel owner of `support` on `arity` slots: the support's
/// slots at `1 + dots` with at most `dots` in total, the other slots (ISPs)
/// at `-rank` with at most `rank` in total.
fn kernel_box(arity: usize, support: u64, dots: u32, rank: u32) -> Vec<Vec<i64>> {
    fn spread(slots: &[usize], total: u32, sign: i64, base: &[i64], out: &mut Vec<Vec<i64>>) {
        let Some((&slot, rest)) = slots.split_first() else {
            out.push(base.to_vec());
            return;
        };
        for amount in 0..=total {
            let mut powers = base.to_vec();
            powers[slot] += sign * i64::from(amount);
            spread(rest, total - amount, sign, &powers, out);
        }
    }
    let (lines, other): (Vec<usize>, Vec<usize>) =
        (0..arity).partition(|&slot| support >> slot & 1 == 1);
    let corner: Vec<i64> = (0..arity)
        .map(|slot| i64::from(support >> slot & 1 == 1))
        .collect();
    let mut dotted = Vec::new();
    spread(&lines, dots, 1, &corner, &mut dotted);
    let mut keys = Vec::new();
    for base in dotted {
        spread(&other, rank, -1, &base, &mut keys);
    }
    keys
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

/// The regeneration test of scope 1-4 (I-8; D-M3-3): the staged payloads with
/// their stored LP (recounted for the owners of at most 9 lines), the package
/// written and loaded back once under `verify_descent` (certificate, strong
/// completeness), the same program. Recorded for D-SIZE (D-M3-6), not
/// criteria: bytes per part, load time with and without the certificate, the
/// time to the first result of a 1L input against the 1-4 and the 1-3
/// package (resident sizes: `the_scope_one_to_four_package_resident_sizes`).
#[test]
#[ignore = "slow: scope 1-4 package written and certified; needs RUSTRED_P_OWNERS_1_4"]
fn the_scope_one_to_four_package_regenerates_from_the_staged_owners() {
    let (assembled, recounts) = scope_four();
    let library = &assembled.library;
    // M3-I1's recount refuses a count other than the record's; here: which
    // owners it recounted (every Custom owner of at most 9 lines).
    for r in recounts {
        println!(
            "LP {} ({} lines): stored {}, recounted {:?}",
            r.label, r.lines, r.stored, r.recounted
        );
        assert_eq!(
            r.recounted.is_some(),
            RecountProvidedLp::from_env().recounts(r.lines)
        );
        assert!(r.recounted.is_none_or(|lp| lp == r.stored));
    }
    assert_eq!(recounts.len(), assembled.custom.len());
    let recounted = recounts.iter().filter(|r| r.recounted.is_some()).count();
    // The 16 Custom payloads of at most 9 lines (owners_1-4.tsv without the
    // Diamond nodes, M3-T5b, and without the Scheme nodes no1, haha, bubu,
    // M5 combined, no2, M5 batch 2; no6's 10-line payload is its Scheme
    // owner's fallback), all 17 with P_LP_RECOUNT_ALL=1: a line filter
    // regressing to none fails here.
    let expected = match RecountProvidedLp::from_env().max_lines() {
        None => 17,
        Some(_) => 16,
    };
    assert_eq!(recounted, expected, "Custom owners LP-recounted");
    println!(
        "LP recounted for {recounted} owners, {} owners of more than {} lines not (stored LP trusted)",
        recounts.len() - recounted,
        RecountProvidedLp::DEFAULT_MAX_LINES
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

    let _ = std::fs::remove_dir_all(&directory);
    let _ = std::fs::remove_dir_all(&three);
}

/// The package of scope 1-4 named by `RUSTRED_P_LIBRARY_1_4` (the staged
/// package: $WS/artefacts/m3/p-library-1-4, with its layer, since STAGE 2),
/// loaded without the certificate.
fn staged_package() -> Arc<PLibrary> {
    let directory = std::env::var_os("RUSTRED_P_LIBRARY_1_4").expect(
        "RUSTRED_P_LIBRARY_1_4 names the scope 1-4 package ($WS/artefacts/m3/p-library-1-4)",
    );
    let before = heap_bytes();
    let (library, seconds) = load(Path::new(&directory), false);
    println!(
        "package loaded without the certificate in {seconds:.2} s, heap of the program {:.1} MiB",
        mib(heap_bytes().saturating_sub(before))
    );
    library
}

/// I-19 and D-SIZE (recorded, not criteria): the heap of each Custom
/// owner's step, lazy (charts prepared on first use, the run-time default)
/// and eager (every chart prepared), and the kernel memos of the 4L kernel
/// owners, fresh steps over a box of their keys, measured by `mallinfo2` in a
/// process that only loaded the package. A measurement, not a check: it runs
/// only when `RUSTRED_P_MEASURE_RESIDENT=1` (a run of every ignored test skips
/// it; its kernel sweep can hold many GB within one owner, review S1 should-fix
/// 9).
#[test]
#[ignore = "measurement: needs RUSTRED_P_MEASURE_RESIDENT=1, RUSTRED_P_LIBRARY_1_4 and MALLOC_ARENA_MAX=1"]
fn the_scope_one_to_four_package_resident_sizes() {
    if std::env::var_os("RUSTRED_P_MEASURE_RESIDENT").is_none_or(|v| v != "1") {
        println!("SKIPPED: a measurement, run only with RUSTRED_P_MEASURE_RESIDENT=1");
        return;
    }
    let unverified = staged_package();
    // CustomStep heap per owner: lazy (charts prepared on first use), then
    // eager (every chart prepared); both held, so each delta is that step's.
    let arenas = std::env::var("MALLOC_ARENA_MAX").unwrap_or_default();
    println!("heap measured by mallinfo2, MALLOC_ARENA_MAX={arenas:?} (1: every arena counted)");
    let mut steps: Vec<Arc<dyn PLaneStep>> = Vec::new();
    let (mut lazy_total, mut eager_total) = (0, 0);
    for (label, owner, _) in custom_owners(&unverified) {
        let spec = &owner.spec;
        let presentation = unverified.presentation(spec.presentation);
        let bytes = spec.payload.as_ref().unwrap().bytes().unwrap();
        let meta = spec.custom.as_ref().unwrap();
        let before = heap_bytes();
        steps.push(build_custom_step(presentation, &bytes, meta).unwrap());
        let lazy = heap_bytes().saturating_sub(before);
        let start = Instant::now();
        let before = heap_bytes();
        steps.push(build_custom_step_prepared(presentation, &bytes, meta).unwrap());
        let eager = heap_bytes().saturating_sub(before);
        (lazy_total, eager_total) = (lazy_total + lazy, eager_total + eager);
        println!(
            "CustomStep {label} {}: payload {} bytes, heap lazy {:.2} MiB, eager {:.2} MiB, prepared in {:.2} s",
            name_of(presentation),
            bytes.len(),
            mib(lazy),
            mib(eager),
            start.elapsed().as_secs_f64()
        );
    }
    println!(
        "CustomStep heap, all {} owners: lazy {:.1} MiB, eager {:.1} MiB",
        steps.len() / 2,
        mib(lazy_total),
        mib(eager_total)
    );
    drop(steps);

    // The kernel memos of the 4L kernel owners, fresh steps over a box of
    // their keys: at most one dot, ISP rank up to 8 (the benchmark cells'
    // rank); owner by owner, all held, until a wall or heap budget is spent.
    const SECONDS: f64 = 1200.0;
    const HEAP: usize = 16 << 30;
    let limits = ReductionLimits::default();
    let tag = GammaTag::one();
    let (mut owners, mut keys) = (0, 0);
    let (mut weights, mut tables) = (MemoCounters::default(), MemoCounters::default());
    let before = heap_bytes();
    let start = Instant::now();
    let mut held: Vec<Arc<dyn PLaneStep>> = Vec::new();
    let mut kernel_owners = 0;
    for owner in unverified.owners() {
        let spec = &owner.spec;
        let presentation = unverified.presentation(spec.presentation);
        if presentation.loops != 4 {
            continue;
        }
        if matches!(
            spec.lane,
            OwnerLane::SeriesParallel | OwnerLane::Insertion | OwnerLane::Carpet
        ) {
            kernel_owners += 1;
        }
        if start.elapsed().as_secs_f64() > SECONDS || heap_bytes().saturating_sub(before) > HEAP {
            continue;
        }
        let owner_start = (Instant::now(), heap_bytes(), keys);
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
        for powers in kernel_box(presentation.slots.len(), support, 1, 8) {
            let view = PKeyView {
                presentation: spec.presentation,
                powers: &powers,
                tag: &tag,
            };
            step.step(&cx, &view).unwrap();
            keys += 1;
        }
        let (w, t) = counters();
        println!(
            "kernel {:?} {} ({} lines): {} keys, weight entries {}, table entries {}, heap {:.1} MiB, {:.1} s",
            spec.lane,
            name_of(presentation),
            lines_of(presentation, support),
            keys - owner_start.2,
            w.misses,
            t.misses,
            mib(heap_bytes().saturating_sub(owner_start.1)),
            owner_start.0.elapsed().as_secs_f64()
        );
        weights = weights + w;
        tables = tables + t;
        owners += 1;
        held.push(step);
    }
    println!(
        "kernel memos at 4L: {owners} of {kernel_owners} kernel owners (wall budget {SECONDS} s, heap budget \
         {} MiB), {keys} keys (dots <= 1, ISP rank <= 8) stepped in {:.1} s; weights {weights:?}, numerator \
         tables {tables:?} (entries = misses, at most 2^16 per memo); heap of the steps and their memos {:.1} MiB",
        HEAP >> 20,
        start.elapsed().as_secs_f64(),
        mib(heap_bytes().saturating_sub(before))
    );
    drop(held);
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
    // A package that lost its library.json stays a package (review S1
    // should-fix 7): refused for the missing file, not read as records.
    std::fs::write(directory.join("census.json"), "{}").unwrap();
    let error = read_custom_payloads(&PLibrarySource::Directory(directory.clone())).unwrap_err();
    assert!(
        error.to_string().contains("missing file library.json"),
        "{error}"
    );
    std::fs::remove_dir_all(&directory).unwrap();
}
