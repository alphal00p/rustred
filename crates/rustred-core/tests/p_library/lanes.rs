//! Lane decisions, lane-adapted node presentations, verified edge maps and the
//! production DAG against Forcer's (owner: M2-T3 rr-lanes).
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rustred::algebra::CoefficientContext;
use rustred::propagator::P_PRESENTATION_VERSION;
use rustred::propagator::decide::{ADAPTED_LOOP, decide};
use rustred::propagator::diamond::check_diamond_contract;
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile, load_p_families};
use rustred::propagator::lane::{
    BubbleRoles, CarpetRoles, LaneDecision, LaneRoles, OwnerLane, TriangleRoles,
};
use rustred::propagator::present::{
    IspPreference, edge_map, present, present_node, present_with_isp_preference, top_topology,
};
use rustred::propagator::presentation::{
    EdgeKind, EdgeMap, Presentation, PresentationKind, Slot, SlotIsp, check_carpet_contract,
    check_insertion_contract, check_triangle_contract, verify_edge_map,
};
use rustred::propagator::topology::{
    Action, ActionKind, ChildMode, Dag, DagEdge, DagEdgeKind, Fate, GraphKey, Ins, NodeInfo,
    Normalised, PTopo, canonical, normalise, reduction_graph,
};
use rustred::propagator::{NodeIndex, PresentationId};
use symbolica::utils::resources::{ResourceBudget, ResourceLimits};

#[path = "../../../rustred-topology/tests/support/forcer.rs"]
mod forcer;

const CENSUS_L4_TSV: &str = include_str!("golden/census_ftops_L4.tsv");
const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/p_library/golden");

/// A change of a dump needs a bump of `P_PRESENTATION_VERSION`, a re-bless and
/// new pins here: `RUSTRED_P_BLESS=1 cargo test -p rustred --test p_library lanes`.
// P_PRESENTATION_VERSION 2 (M5 combined: the ISP choice of three scope 1-4
// Insertion owners): the 2L/3L dumps change only in their header line.
// P_PRESENTATION_VERSION 3 (M5 batch 4, route bases): edge-pair ISPs of the
// generic presentations of the table; the 2L dump changes in its header only.
const DUMP_PINS: (u32, [(u8, &str); 2]) = (
    3,
    [
        (
            2,
            "9c1c5bb98184eb80386448f94a112d4ab3c11c28ea0bea2d4792aadc4705ac8b",
        ),
        (
            3,
            "6170032ceba9a50f86e99eaa25f3e0b4def9994880d9b3d3a5ddb43ee0dae719",
        ),
    ],
);

const DAG_2L: [&str; 7] = ["d22", "d26", "d9", "d43", "d3", "d4", "d1"];

const DAG_3L: [&str; 43] = [
    "d121", "d60", "nostar0", "d106", "d107", "d148", "d149", "d190", "d191", "d28", "d32", "d33",
    "d37", "d50", "d54", "d55", "d77", "d78", "d79", "d80", "d81", "d87", "d92", "d118", "d137",
    "d192", "d197", "t1star05", "d11", "d13", "d14", "d15", "d17", "d22", "d26", "d9", "d30",
    "d43", "d3", "d4", "d6", "d7", "d1",
];

fn context() -> &'static CoefficientContext {
    tops().1[0].family.coefficient_context()
}

/// The vakint families and their top presentations.
fn tops() -> &'static (PFamiliesFile, Vec<Arc<Presentation>>) {
    static TOPS: OnceLock<(PFamiliesFile, Vec<Arc<Presentation>>)> = OnceLock::new();
    TOPS.get_or_init(|| load_p_families(P_FAMILIES_V1_JSON).unwrap())
}

/// The DAG of vakint's `loops`-loop tops: the production input.
fn production_dag(loops: u8) -> Dag {
    let (file, _) = tops();
    let roots: Vec<PTopo> = file
        .families
        .iter()
        .filter(|entry| entry.loops == loops)
        .map(top_topology)
        .collect();
    reduction_graph(&roots, ChildMode::ForcerTree)
}

fn forcer_names() -> &'static BTreeMap<GraphKey, String> {
    static NAMES: OnceLock<BTreeMap<GraphKey, String>> = OnceLock::new();
    NAMES.get_or_init(|| forcer::forcer_names_by_key(&forcer::forcer_dag()))
}

fn forcer_name(key: &GraphKey) -> &'static str {
    forcer_names()
        .get(key)
        .unwrap_or_else(|| panic!("{key:?} is not a Forcer topology"))
}

fn sorted_names(names: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    names.sort();
    names
}

/// One DAG with its decisions and presentations; ids start after the tops.
struct Lanes {
    dag: Dag,
    decisions: Vec<LaneDecision>,
    presentations: Vec<Option<Arc<Presentation>>>,
}

impl Lanes {
    fn build(loops: u8) -> Self {
        let dag = production_dag(loops);
        let (_, top_presentations) = tops();
        let first_id = u32::try_from(top_presentations.len()).unwrap();
        let mut decisions = Vec::with_capacity(dag.nodes.len());
        let mut presentations = Vec::with_capacity(dag.nodes.len());
        for node in &dag.nodes {
            let id = PresentationId(first_id + node.index);
            let built = present_node(node, top_presentations, id, context()).unwrap();
            assert_eq!(built.fallback, None, "{}", forcer_name(&node.key));
            assert_eq!(built.decision, decide(node).unwrap());
            decisions.push(built.decision);
            presentations.push(built.presentation.map(Arc::new));
        }
        Self {
            dag,
            decisions,
            presentations,
        }
    }

    fn presentation(&self, index: u32) -> Option<&Arc<Presentation>> {
        self.presentations[index as usize].as_ref()
    }

    fn by_name(&self, name: &str) -> (&NodeInfo, &LaneDecision, &Presentation) {
        let node = self
            .dag
            .nodes
            .iter()
            .find(|node| forcer_name(&node.key) == name)
            .unwrap_or_else(|| panic!("no node {name}"));
        let presentation = self.presentation(node.index).unwrap();
        (node, &self.decisions[node.index as usize], presentation)
    }

    /// `lane name -> nodes`; the trivial node is its own entry.
    fn table(&self) -> BTreeMap<&'static str, usize> {
        let mut table = BTreeMap::new();
        for decision in &self.decisions {
            *table.entry(lane_name(decision)).or_default() += 1;
        }
        table
    }

    /// The Forcer names of a lane, sorted.
    fn names(&self, name: &str) -> Vec<String> {
        let names: Vec<&str> = self
            .dag
            .nodes
            .iter()
            .zip(&self.decisions)
            .filter(|(_, decision)| lane_name(decision) == name)
            .map(|(node, _)| forcer_name(&node.key))
            .collect();
        sorted_names(&names)
    }

    fn rule_edges(&self) -> impl Iterator<Item = &DagEdge> {
        self.dag
            .edges
            .iter()
            .filter(|edge| edge.kind == DagEdgeKind::Rule)
    }

    /// The map of `edge`; `None` into the trivial node.
    fn map_of(&self, edge: &DagEdge) -> Option<EdgeMap> {
        let parent = self.presentation(edge.parent).unwrap();
        let child = self.presentation(edge.child)?;
        Some(edge_map(parent, child, edge).unwrap())
    }
}

fn lane_name(decision: &LaneDecision) -> &'static str {
    if decision.trivial {
        return "Trivial";
    }
    match decision.lane {
        OwnerLane::Zero => "Zero",
        OwnerLane::SeriesParallel => "SeriesParallel",
        OwnerLane::Insertion => "Insertion",
        OwnerLane::Carpet => "Carpet",
        OwnerLane::Triangle => "Triangle",
        OwnerLane::Diamond => "Diamond",
        OwnerLane::Custom => "Custom",
        OwnerLane::Product => "Product",
        OwnerLane::Scheme => "Scheme",
    }
}

fn lanes(loops: u8) -> &'static Lanes {
    static LANES: [OnceLock<Lanes>; 4] = [const { OnceLock::new() }; 4];
    LANES[usize::from(loops) - 1].get_or_init(|| Lanes::build(loops))
}

fn table(entries: &[(&'static str, usize)]) -> BTreeMap<&'static str, usize> {
    entries.iter().copied().collect()
}

#[test]
fn lane_tables_are_the_plans() {
    assert_eq!(
        lanes(1).table(),
        table(&[("SeriesParallel", 1), ("Trivial", 1)])
    );
    assert_eq!(
        lanes(2).table(),
        table(&[("SeriesParallel", 5), ("Triangle", 1), ("Trivial", 1)])
    );
    let three = lanes(3);
    assert_eq!(
        three.table(),
        table(&[
            ("SeriesParallel", 26),
            ("Insertion", 6),
            ("Carpet", 2),
            ("Triangle", 6),
            ("Custom", 2),
            ("Trivial", 1),
        ])
    );
    assert_eq!(three.names("Carpet"), sorted_names(&["d121", "d60"]));
    assert_eq!(
        three.names("Custom"),
        sorted_names(&["nostar0", "t1star05"])
    );
    assert_eq!(
        three.names("Triangle"),
        sorted_names(&["d43", "d30", "d118", "d137", "d192", "d197"])
    );
    // 4L ForcerTree, pinned: Forcer's actions with the SP split.
    let four = lanes(4);
    assert_eq!(
        four.table(),
        table(&[
            ("SeriesParallel", 161),
            ("Insertion", 174),
            ("Carpet", 24),
            ("Triangle", 53),
            ("Diamond", 4),
            ("Scheme", 5),
            ("Custom", 16),
            ("Trivial", 1),
        ])
    );
    assert_eq!(
        four.names("Diamond"),
        sorted_names(&["d384", "d394", "d399", "no3"])
    );
    // Forcer's schemes, on by default (P_SCHEME_TEMPLATE_VERSION 2, M5 combined;
    // no2 since P_SCHEME_TEMPLATE_VERSION 3, no6 since 4, M5 batch 2).
    assert_eq!(
        four.names("Scheme"),
        sorted_names(&["bubu", "haha", "no1", "no2", "no6"])
    );
}

/// `(L, action)` and the SP split of the 4L DAG agree with the independent
/// Python census node by node.
#[test]
fn the_4l_table_equals_the_python_census() {
    let dag = &lanes(4).dag;
    let mut seen = BTreeSet::new();
    let rows = CENSUS_L4_TSV.lines().filter(|l| !l.starts_with('#'));
    for row in rows {
        let columns: Vec<&str> = row.split('\t').collect();
        let [loops, action, series_parallel, n, s, t, lines] = columns.as_slice() else {
            panic!("census row {row}");
        };
        let lines = lines.split_whitespace().map(|line| {
            let mut parts = line.split('-').map(|x| x.parse::<u8>().unwrap());
            let mut next = || parts.next().unwrap();
            (next(), next(), next())
        });
        let topo = PTopo::new(
            n.parse().unwrap(),
            s.parse().unwrap(),
            t.parse().unwrap(),
            lines,
        );
        let key = match normalise(&topo) {
            Normalised::Topo(normal, _) => canonical(&normal).0,
            Normalised::Trivial => canonical(&PTopo::trivial()).0,
            Normalised::Zero => panic!("zero census row {row}"),
        };
        let node = dag
            .find(&key)
            .unwrap_or_else(|| panic!("no node for {row}"));
        assert!(
            seen.insert(node.index),
            "two census rows on one node: {row}"
        );
        assert_eq!(node.loops.to_string(), *loops, "{row}");
        assert_eq!(node.action.kind().as_str(), *action, "{row}");
        assert_eq!(node.series_parallel, *series_parallel == "1", "{row}");

        let expected = match node.action.kind() {
            ActionKind::Trivial => OwnerLane::Zero,
            ActionKind::Insertion if node.series_parallel => OwnerLane::SeriesParallel,
            ActionKind::Insertion => OwnerLane::Insertion,
            ActionKind::Carpet => OwnerLane::Carpet,
            ActionKind::Triangle => OwnerLane::Triangle,
            ActionKind::Diamond => OwnerLane::Diamond,
            // Forcer's schemes (no1, haha, bubu) are on by default (M5 combined).
            ActionKind::Custom if rustred::propagator::scheme::scheme_for(node).is_some() => {
                OwnerLane::Scheme
            }
            ActionKind::Custom => OwnerLane::Custom,
        };
        assert_eq!(
            lanes(4).decisions[node.index as usize].lane,
            expected,
            "{row}"
        );
    }
    assert_eq!(seen.len(), dag.nodes.len());
}

/// The lane's contract, by the checker of `presentation.rs`.
fn check_contract(presentation: &Presentation, decision: &LaneDecision) {
    match decision.roles {
        LaneRoles::None => {}
        LaneRoles::Insertion(roles) => check_insertion_contract(presentation, &roles).unwrap(),
        LaneRoles::Carpet(roles) => check_carpet_contract(presentation, &roles).unwrap(),
        LaneRoles::Triangle(roles) => check_triangle_contract(presentation, &roles).unwrap(),
        LaneRoles::Diamond(roles) => check_diamond_contract(presentation, &roles).unwrap(),
        // M5 A1 schemes: the scheme-adapted presentation is checked on the graph
        // by `Presentation::from_slots` and the template by its descent witness.
        LaneRoles::Scheme(_) => {}
    }
}

/// The rows conserve momentum on the node's graph and the eps units are the
/// graph's, line by line; the ISPs are unit rows (nonzero rows on a
/// scheme-adapted presentation).
fn assert_routes_the_graph(node: &NodeInfo, presentation: &Presentation) {
    let name = forcer_name(&node.key);
    let width = usize::from(node.loops) + 1;
    let mut balance = vec![vec![0; width]; usize::from(node.topo.n)];
    for (slot, &(u, v, eps_units)) in presentation.slots.iter().zip(&node.topo.lines) {
        let Slot::Line(line) = slot else {
            panic!("{name}: a line slot is an ISP");
        };
        assert_eq!(line.eps_units, eps_units, "{name}");
        for (column, &value) in line.momentum.iter().enumerate() {
            balance[usize::from(u)][column] -= value;
            balance[usize::from(v)][column] += value;
        }
    }
    balance[usize::from(node.topo.s)][width - 1] += 1;
    balance[usize::from(node.topo.t)][width - 1] -= 1;
    assert!(
        balance.iter().flatten().all(|&value| value == 0),
        "{name}: {balance:?}"
    );
    for slot in &presentation.slots[node.topo.lines.len()..] {
        let Slot::Isp(SlotIsp { left, right }) = slot else {
            panic!("{name}: an ISP slot is a line");
        };
        for row in [left, right] {
            // Forcer's dot-product ISPs of a scheme-adapted presentation (M5
            // combined) are products of Forcer's own momenta, e.g. (p1+p2).p3:
            // nonzero rows, not necessarily unit rows.
            if presentation.kind == PresentationKind::SchemeAdapted {
                assert!(row.iter().any(|&c| c != 0), "{name}");
            } else {
                // P_PRESENTATION_VERSION 3 (M5 batch 4, route bases): an ISP
                // factor is a unit row or, for an edge-pair ISP p_a.p_b, the
                // momentum of a line of the node (up to sign).
                let unit = row.iter().map(|c| c.abs()).sum::<i64>() == 1;
                let line =
                    presentation.slots[..node.topo.lines.len()]
                        .iter()
                        .any(|slot| match slot {
                            Slot::Line(line) => {
                                line.momentum == *row
                                    || line.momentum.iter().zip(row.iter()).all(|(a, b)| *a == -*b)
                            }
                            Slot::Isp(_) => false,
                        });
                assert!(unit || line, "{name}: ISP factor {row:?}");
            }
        }
    }
}

#[test]
fn every_presentation_meets_its_contract() {
    for loops in 1..=4 {
        let built = lanes(loops);
        for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
            let name = forcer_name(&node.key);
            let Some(presentation) = built.presentation(node.index) else {
                assert_eq!(decision.lane, OwnerLane::Zero, "{name}");
                assert_eq!(node.action, Action::Trivial, "{name}");
                continue;
            };
            check_contract(presentation, decision);
            let reference = presentation.node.as_ref().unwrap();
            assert_eq!(reference.index, NodeIndex(node.index), "{name}");
            assert_eq!(reference.graph_key, node.key.0, "{name}");
            assert_eq!(presentation.loops, node.loops, "{name}");
            let expected = match (decision.lane, node.eps_lines) {
                (OwnerLane::SeriesParallel | OwnerLane::Insertion, _) => {
                    Some(PresentationKind::InsertionAdapted)
                }
                (OwnerLane::Carpet, _) => Some(PresentationKind::CarpetAdapted),
                (OwnerLane::Triangle, _) => Some(PresentationKind::TriangleAdapted),
                (OwnerLane::Diamond, _) => Some(PresentationKind::DiamondAdapted),
                (OwnerLane::Scheme, _) => Some(PresentationKind::SchemeAdapted),
                (OwnerLane::Custom, 0) => None,
                (OwnerLane::Custom, _) => Some(PresentationKind::EpsNode),
                (lane, _) => panic!("{name}: lane {lane:?}"),
            };
            if let Some(expected) = expected {
                assert_eq!(presentation.kind, expected, "{name}");
                assert_routes_the_graph(node, presentation);
            }
        }
    }
}

#[test]
fn insertion_roles_name_the_bubble() {
    for loops in 1..=4 {
        let built = lanes(loops);
        for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
            let LaneRoles::Insertion(BubbleRoles { lines, loop_index }) = decision.roles else {
                continue;
            };
            assert_eq!(loop_index, ADAPTED_LOOP);
            let Action::Insertion(Ins { keep, drop }) = node.action else {
                panic!("{}: no insertion", forcer_name(&node.key));
            };
            assert_eq!(lines, [u16::from(keep), u16::from(drop)]);
            let (a, b) = (
                node.topo.lines[usize::from(keep)],
                node.topo.lines[usize::from(drop)],
            );
            assert_eq!((a.0, a.1), (b.0, b.1));
        }
    }
}

/// d121 and d60 by hand: `q` only in `r`, `third = p`, the ISPs free of `p.q`.
#[test]
fn the_two_3l_carpets_pass_the_corrected_contract() {
    let built = lanes(3);
    let width = 4;
    let (mut with_third, mut without_third) = (0, 0);
    for name in ["d121", "d60"] {
        let (node, decision, presentation) = built.by_name(name);
        let LaneRoles::Carpet(CarpetRoles {
            r,
            third,
            outer_loop,
        }) = decision.roles
        else {
            panic!("{name}: {decision:?}");
        };
        assert_eq!(outer_loop, ADAPTED_LOOP);
        let Action::Carpet(ref carpet) = node.action else {
            panic!("{name}: {:?}", node.action);
        };
        assert_eq!(usize::from(r), usize::from(carpet.r));
        assert_eq!(third, carpet.third.map(u16::from));
        match third {
            Some(_) => with_third += 1,
            None => without_third += 1,
        }

        let q_lines: Vec<usize> = presentation
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                Slot::Line(line) if line.momentum[width - 1] != 0 => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(q_lines, [usize::from(r)], "{name}");
        let Slot::Line(line_r) = &presentation.slots[usize::from(r)] else {
            panic!("{name}: r is an ISP");
        };
        assert_eq!(line_r.momentum[0].abs(), 1, "{name}");
        assert_eq!(line_r.momentum[0], -line_r.momentum[width - 1], "{name}");
        if let Some(third) = third {
            let Slot::Line(line_third) = &presentation.slots[usize::from(third)] else {
                panic!("{name}: third is an ISP");
            };
            assert_eq!(line_third.momentum[0].abs(), 1, "{name}");
            assert_eq!(line_third.momentum[1..], [0, 0, 0], "{name}");
        }
        for slot in &presentation.slots {
            if let Slot::Isp(SlotIsp { left, right }) = slot {
                let carries_q = left[width - 1] != 0 || right[width - 1] != 0;
                let carries_p = left[0] != 0 || right[0] != 0;
                assert!(!(carries_q && carries_p), "{name}: p.q in {slot:?}");
            }
        }
        check_contract(presentation, decision);
    }
    assert_eq!((with_third, without_third), (1, 1));
}

/// Every carpet of the 4L DAG; third-absent carpets exist there.
#[test]
fn third_absent_carpets_pass_the_contract() {
    let built = lanes(4);
    let carpets = built
        .dag
        .nodes
        .iter()
        .zip(&built.decisions)
        .filter_map(|(node, decision)| match decision.roles {
            LaneRoles::Carpet(roles) => Some((node, decision, roles)),
            _ => None,
        });
    let (mut with_third, mut without_third) = (0, 0);
    for (node, decision, roles) in carpets {
        check_contract(built.presentation(node.index).unwrap(), decision);
        match roles.third {
            Some(_) => with_third += 1,
            None => without_third += 1,
        }
    }
    assert_eq!((with_third, without_third), (15, 9));
}

#[test]
fn triangle_roles_are_the_matchers_with_k_in_three_lines() {
    let built = lanes(4);
    let mut triangles = 0;
    for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
        let LaneRoles::Triangle(TriangleRoles { b, a, c, k_loop }) = decision.roles else {
            continue;
        };
        let Action::Triangle(m) = node.action else {
            panic!("{}: no triangle", forcer_name(&node.key));
        };
        assert_eq!(k_loop, ADAPTED_LOOP);
        assert_eq!(
            (b, a, c),
            (
                u16::from(m.b),
                [u16::from(m.a1), u16::from(m.a2)],
                [u16::from(m.c1), u16::from(m.c2)]
            )
        );
        let presentation = built.presentation(node.index).unwrap();
        let carrying: BTreeSet<u16> = (presentation.slots.iter().enumerate())
            .filter_map(|(index, slot)| match slot {
                Slot::Line(line) if line.momentum[usize::from(k_loop)] != 0 => {
                    Some(u16::try_from(index).unwrap())
                }
                _ => None,
            })
            .collect();
        assert_eq!(carrying, BTreeSet::from([b, a[0], a[1]]));
        triangles += 1;
    }
    assert_eq!(triangles, 53);
    // The eps apex of d30 sits on the triangle lines, not on b or c.
    let (node, decision, _) = lanes(3).by_name("d30");
    let LaneRoles::Triangle(roles) = decision.roles else {
        panic!("d30: {decision:?}");
    };
    let eps = |line: u16| node.topo.lines[usize::from(line)].2;
    assert_eq!(eps(roles.a[0]) + eps(roles.a[1]), 1);
}

/// Custom nodes: integer = a vakint top sector with the top's family, eps = the
/// default routing with its shifts; every line of the sector is a mask bit.
#[test]
fn custom_presentations_are_induced_or_eps_nodes() {
    let (file, top_presentations) = tops();
    let top_named = |id: PresentationId| &file.families[id.0 as usize];

    let (node, _, nostar) = lanes(3).by_name("nostar0");
    let PresentationKind::Induced { top, mask } = nostar.kind else {
        panic!("nostar0: {:?}", nostar.kind);
    };
    assert_eq!(top_named(top).name, "P3L_NO");
    assert_eq!(mask, 0xff);
    assert_eq!(node.eps_lines, 0);
    assert!(Arc::ptr_eq(
        &nostar.family,
        &top_presentations[top.0 as usize].family
    ));
    assert_eq!(nostar.slots, top_presentations[top.0 as usize].slots);

    let (node, decision, kite) = lanes(3).by_name("t1star05");
    assert_eq!(decision.roles, LaneRoles::None);
    assert_eq!(kite.kind, PresentationKind::EpsNode);
    assert_eq!(kite.forced_axes().count_ones(), 1);
    assert_eq!(node.t, 5);
    assert_eq!(kite.slots.len(), 5);

    let built = lanes(4);
    let mut integer = 0;
    let mut eps = 0;
    for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
        if decision.lane != OwnerLane::Custom {
            continue;
        }
        let presentation = built.presentation(node.index).unwrap();
        match presentation.kind {
            PresentationKind::Induced { top, mask } => {
                integer += 1;
                assert_eq!(top_named(top).loops, node.loops);
                assert_eq!(mask.count_ones(), u32::from(node.t));
            }
            PresentationKind::EpsNode => {
                eps += 1;
                assert_ne!(node.eps_lines, 0);
                assert_eq!(
                    presentation.forced_axes().count_ones(),
                    u32::from(node.eps_lines)
                );
            }
            ref kind => panic!("{}: {kind:?}", forcer_name(&node.key)),
        }
    }
    // The 4 diamond nodes (d384, d394, d399, no3) are Diamond owners (M3-T5b);
    // no1, haha and bubu are Scheme owners (M5 combined), no2 and no6 too (M5 batch 2).
    assert_eq!((integer, eps), (5, 11));
}

#[test]
fn an_integer_custom_node_outside_the_tops_is_refused() {
    let (node, decision, _) = lanes(3).by_name("nostar0");
    let tops_without_no: Vec<_> = tops()
        .1
        .iter()
        .filter(|top| !matches!(&top.kind, PresentationKind::Top { name } if name == "P3L_NO"))
        .cloned()
        .collect();
    let refused = present(
        node,
        decision,
        &tops_without_no,
        PresentationId(99),
        context(),
    );
    assert!(matches!(refused, Err(PError::Contract(_))), "{refused:?}");
}

#[test]
fn every_rule_edge_has_a_verified_map() {
    // loops -> (rule edges with a map, rule edges into the trivial node)
    let mut counts = BTreeMap::new();
    for loops in 1..=4 {
        let built = lanes(loops);
        let (mut maps, mut trivial) = (0, 0);
        for edge in built.rule_edges() {
            let Some(map) = built.map_of(edge) else {
                assert!(built.decisions[edge.child as usize].trivial);
                assert_eq!(built.dag.nodes[edge.parent as usize].loops, 1);
                trivial += 1;
                continue;
            };
            let parent = built.presentation(edge.parent).unwrap();
            let child = built.presentation(edge.child).unwrap();
            verify_edge_map(parent, child, &map).unwrap();
            assert_edge_agrees_with_the_fates(parent, child, edge, &map);
            assert_edge_agrees_with_the_roles(&built.decisions[edge.parent as usize], &map);
            maps += 1;
        }
        counts.insert(loops, (maps, trivial));
    }
    assert_eq!(
        counts,
        BTreeMap::from([(1, (0, 1)), (2, (3, 2)), (3, (30, 4)), (4, (353, 6))])
    );
}

/// The integrated lines leave `line_slots`; every other line goes where its
/// fate says, and so does the bubble through `merged_slot`. Fates count canonical
/// lines, which an Induced child's slots are not: there a line lands or goes.
fn assert_edge_agrees_with_the_fates(
    parent: &Presentation,
    child: &Presentation,
    edge: &DagEdge,
    map: &EdgeMap,
) {
    assert_eq!(map.integrated_loop, ADAPTED_LOOP);
    assert_eq!(map.line_slots.len(), parent.slots.len());
    assert!(
        map.line_slots[edge.fates.len()..]
            .iter()
            .all(Option::is_none)
    );
    let canonical_child = !matches!(child.kind, PresentationKind::Induced { .. });
    for (index, fate) in edge.fates.iter().enumerate() {
        assert_ne!(*fate, Fate::Contracted, "a contracted line on a rule edge");
        let Slot::Line(line) = &parent.slots[index] else {
            panic!("a fate on an ISP");
        };
        let integrated =
            map.kind == EdgeKind::Insertion && line.momentum[usize::from(ADAPTED_LOOP)] != 0;
        if integrated {
            assert_eq!(map.line_slots[index], None);
        }
        let target = if integrated {
            map.merged_slot
        } else {
            map.line_slots[index]
        };
        let expected = fate.line().map(u16::from);
        if canonical_child {
            assert_eq!(target, expected);
        } else {
            assert_eq!(target.is_some(), expected.is_some());
        }
    }
}

/// No gate reads roles and map together: the step constructors of M2-T4 rely on
/// this.
fn assert_edge_agrees_with_the_roles(decision: &LaneDecision, map: &EdgeMap) {
    let removed = |slot: u16| assert_eq!(map.line_slots[usize::from(slot)], None);
    match decision.roles {
        LaneRoles::Insertion(roles) => {
            assert_eq!(map.kind, EdgeKind::Insertion);
            assert_eq!(map.integrated_loop, roles.loop_index);
            roles.lines.into_iter().for_each(removed);
        }
        LaneRoles::Carpet(roles) => {
            assert_eq!(map.kind, EdgeKind::Carpet);
            assert_eq!(map.integrated_loop, roles.outer_loop);
            removed(roles.r);
            roles.third.into_iter().for_each(removed);
        }
        other => panic!("a rule edge leaves {other:?}"),
    }
}

/// Rule edges into a top family sector: a carpet with a NO blob and a bubble in
/// series with NO, absorbed into `q`.
#[test]
fn rule_edges_reach_the_induced_no() {
    let built = lanes(4);
    let mut kinds = BTreeSet::new();
    for edge in built
        .rule_edges()
        .filter(|edge| forcer_name(&built.dag.nodes[edge.child as usize].key) == "nostar0")
    {
        let child = built.presentation(edge.child).unwrap();
        assert!(matches!(child.kind, PresentationKind::Induced { .. }));
        let map = built.map_of(edge).unwrap();
        assert_eq!(map.merged_slot, None);
        kinds.insert(format!("{:?}", map.kind));
    }
    assert_eq!(
        kinds,
        BTreeSet::from(["Carpet".to_owned(), "Insertion".to_owned()])
    );
}

#[test]
fn edge_maps_are_refused_on_contractions_and_foreign_nodes() {
    let built = lanes(2);
    let contraction = built
        .dag
        .edges
        .iter()
        .find(|edge| {
            matches!(edge.kind, DagEdgeKind::Contract(_))
                && built.presentation(edge.child).is_some()
        })
        .unwrap();
    let parent = built.presentation(contraction.parent).unwrap();
    let child = built.presentation(contraction.child).unwrap();
    assert!(matches!(
        edge_map(parent, child, contraction),
        Err(PError::Contract(_))
    ));

    let rule = built
        .rule_edges()
        .find(|edge| built.presentation(edge.child).is_some())
        .unwrap();
    let parent = built.presentation(rule.parent).unwrap();
    let child = built.presentation(rule.child).unwrap();
    assert!(edge_map(parent, child, rule).is_ok());
    let mut foreign = rule.clone();
    foreign.child += 1;
    assert!(matches!(
        edge_map(parent, child, &foreign),
        Err(PError::Contract(_))
    ));
    let mut short = rule.clone();
    short.fates.pop();
    assert!(matches!(
        edge_map(parent, child, &short),
        Err(PError::Contract(_))
    ));
    let mut malformed = (**parent).clone();
    let Slot::Line(line) = &mut malformed.slots[0] else {
        panic!("slot 0 is an ISP");
    };
    line.momentum.clear();
    assert!(matches!(
        edge_map(&malformed, child, rule),
        Err(PError::Contract(_))
    ));
}

/// d13 -> d4 ends in a 1L bubble with one eps line, whose two lines swap under
/// `k -> q - k`. Swapped fates therefore land every momentum, and only the eps
/// units give them away, which `verify_edge_map` alone reads.
#[test]
fn edge_map_verifies_its_result() {
    let built = lanes(3);
    let (node, _, parent) = built.by_name("d13");
    let edge = built
        .rule_edges()
        .find(|edge| edge.parent == node.index)
        .unwrap();
    let child = built.presentation(edge.child).unwrap();
    assert!(edge_map(parent, child, edge).is_ok());
    let mut swapped = edge.clone();
    swapped.fates.swap(2, 3);
    let reason = contract_reason(edge_map(parent, child, &swapped));
    assert!(reason.contains("eps units"), "{reason}");
}

#[test]
fn the_trivial_node_has_no_presentation() {
    let built = lanes(1);
    let node = built.dag.nodes.last().unwrap();
    assert_eq!(node.action, Action::Trivial);
    let decision = decide(node).unwrap();
    assert!(decision.trivial);
    assert_eq!(decision.roles, LaneRoles::None);
    let refused = present(node, &decision, &tops().1, PresentationId(99), context());
    assert!(matches!(refused, Err(PError::Contract(_))), "{refused:?}");
}

#[test]
fn a_scaleless_node_is_zero() {
    let mut node = lanes(1).dag.nodes[0].clone();
    node.topo.lines.truncate(1); // a line in series with q alone would be absorbed, not this
    node.topo.t = node.topo.s;
    let decision = decide(&node).unwrap();
    assert_eq!(decision.lane, OwnerLane::Zero);
    assert!(!decision.trivial);
}

/// The reason of a contract failure.
fn contract_reason<T: std::fmt::Debug>(outcome: Result<T, PError>) -> String {
    match outcome {
        Err(PError::Contract(reason)) => reason,
        other => panic!("no contract failure: {other:?}"),
    }
}

/// `node` with `action` holds the `lane`, `present` refuses it and `present_node`
/// makes it Custom with the same reason, which holds `expected`.
fn assert_falls_back(node: &NodeInfo, action: Action, lane: OwnerLane, expected: &str) {
    let mut node = node.clone();
    node.action = action;
    let decision = decide(&node).unwrap();
    assert_eq!(decision.lane, lane);
    let refused = present(&node, &decision, &tops().1, PresentationId(99), context());
    let reason = contract_reason(refused);
    assert!(reason.contains(expected), "{reason}");

    let built = present_node(&node, &tops().1, PresentationId(99), context()).unwrap();
    assert_eq!(built.decision.lane, OwnerLane::Custom);
    assert_eq!(built.decision.roles, LaneRoles::None);
    assert_eq!(built.fallback, Some(reason));
    let presentation = built.presentation.unwrap();
    assert!(matches!(
        presentation.kind,
        PresentationKind::Induced { .. }
    ));
}

/// A node whose presentation cannot meet its contract is Custom, with a reason.
#[test]
fn a_broken_contract_falls_back_to_custom() {
    let (kite, _, _) = lanes(2).by_name("d43");
    // The matcher names a pair that is no bubble.
    let pair = Action::Insertion(Ins { keep: 0, drop: 1 });
    assert_falls_back(kite, pair, OwnerLane::Insertion, "insertion contract");
    let pair = Action::Insertion(Ins { keep: 0, drop: 99 });
    assert_falls_back(kite, pair, OwnerLane::Insertion, "role line 99");

    // The apex lines are matched to the other side's lines.
    let Action::Triangle(mut triangle) = kite.action else {
        panic!("d43: {:?}", kite.action);
    };
    std::mem::swap(&mut triangle.c1, &mut triangle.c2);
    let swapped = Action::Triangle(triangle);
    assert_falls_back(kite, swapped, OwnerLane::Triangle, "triangle contract");

    // The bridge p of d121 is a line that the roles do not claim.
    let (d121, _, _) = lanes(3).by_name("d121");
    let Action::Carpet(mut carpet) = d121.action.clone() else {
        panic!("d121: {:?}", d121.action);
    };
    assert!(carpet.third.take().is_some());
    let unclaimed = Action::Carpet(carpet);
    assert_falls_back(d121, unclaimed, OwnerLane::Carpet, "carpet contract");
}

/// Only dependent denominators are the lane's fault: any other failure of the
/// ISP completion leaves the decision alone and stops the caller.
#[test]
fn only_dependent_denominators_leave_a_lane() {
    // Three denominators for the two scalar products of one loop.
    let mut overfull = lanes(1).dag.nodes[0].clone();
    overfull.topo = PTopo::new(3, 0, 1, [(0, 1, 0), (0, 2, 0), (2, 1, 0)]);
    overfull.action = Action::Insertion(Ins { keep: 0, drop: 1 });
    let decision = decide(&overfull).unwrap();
    let refused = present(
        &overfull,
        &decision,
        &tops().1,
        PresentationId(99),
        context(),
    );
    assert!(matches!(refused, Err(PError::Generation(_))), "{refused:?}");
    let refused = present_node(&overfull, &tops().1, PresentationId(99), context());
    assert!(matches!(refused, Err(PError::Generation(_))), "{refused:?}");

    // The series lines 1 and 2 carry one momentum.
    let (kite, _, _) = lanes(2).by_name("d43");
    let mut series = kite.clone();
    series.topo = PTopo::new(3, 0, 1, [(0, 1, 0), (0, 2, 0), (2, 1, 0), (0, 1, 0)]);
    let pair = Action::Insertion(Ins { keep: 0, drop: 3 });
    assert_falls_back(&series, pair, OwnerLane::Insertion, "generic rank");
}

/// An exhausted budget says nothing about the lane either.
#[test]
fn an_exhausted_budget_is_no_fallback() {
    let budget = ResourceBudget::new(ResourceLimits {
        max_process_rss_bytes: None,
        max_elapsed: Some(Duration::ZERO),
    });
    let (node, _, _) = lanes(2).by_name("d22");
    let refused = budget.run(|| {
        std::thread::sleep(Duration::from_millis(2));
        present_node(node, &tops().1, PresentationId(99), context())
    });
    assert!(matches!(refused, Err(PError::Engine(_))), "{refused:?}");
}

/// Role lines of every kind outside the node: a refusal, never an index panic.
#[test]
fn role_lines_outside_the_node_are_refused() {
    let (node, _, _) = lanes(2).by_name("d22");
    let outside = u16::try_from(node.topo.lines.len()).unwrap();
    let decision = |lane, roles| LaneDecision {
        lane,
        roles,
        trivial: false,
    };
    let insertion = |lines| {
        let loop_index = ADAPTED_LOOP;
        let roles = LaneRoles::Insertion(BubbleRoles { lines, loop_index });
        decision(OwnerLane::Insertion, roles)
    };
    let carpet = |r, third| {
        let outer_loop = ADAPTED_LOOP;
        let roles = CarpetRoles {
            r,
            third,
            outer_loop,
        };
        decision(OwnerLane::Carpet, LaneRoles::Carpet(roles))
    };
    let triangle = |b, a, c| {
        let k_loop = ADAPTED_LOOP;
        let roles = TriangleRoles { b, a, c, k_loop };
        decision(OwnerLane::Triangle, LaneRoles::Triangle(roles))
    };
    let cases = [
        insertion([outside, 1]),
        insertion([0, outside]),
        carpet(outside, None),
        carpet(0, Some(outside)),
        triangle(outside, [1, 2], [3, 0]),
        triangle(0, [outside, 2], [3, 1]),
        triangle(0, [1, outside], [3, 2]),
        triangle(0, [1, 2], [outside, 3]),
        triangle(0, [1, 2], [3, outside]),
    ];
    for wrong in cases {
        let refused = present(node, &wrong, &tops().1, PresentationId(99), context());
        let reason = contract_reason(refused);
        assert!(reason.contains(&format!("role line {outside}")), "{reason}");
    }
}

#[test]
fn a_decision_must_agree_with_itself() {
    let (node, decision, _) = lanes(2).by_name("d43");
    let cases = [
        LaneDecision {
            lane: OwnerLane::Carpet,
            ..*decision
        },
        LaneDecision {
            lane: OwnerLane::Custom,
            ..*decision
        },
        LaneDecision {
            trivial: true,
            ..*decision
        },
        LaneDecision {
            lane: OwnerLane::Custom,
            roles: LaneRoles::None,
            trivial: true,
        },
    ];
    for wrong in cases {
        let refused = present(node, &wrong, &tops().1, PresentationId(99), context());
        contract_reason(refused);
    }
}

#[test]
fn a_top_that_differs_from_the_file_is_refused() {
    let (node, decision, _) = lanes(3).by_name("nostar0");
    let top = (tops().1.iter())
        .find(|top| matches!(&top.kind, PresentationKind::Top { name } if name == "P3L_NO"))
        .unwrap();
    let mut slots = top.slots.clone();
    slots.swap(0, 1);
    let tampered = Presentation::from_slots(
        top.id,
        top.kind.clone(),
        "P3L_NO",
        top.loops,
        slots,
        context(),
        None,
    )
    .unwrap();
    let refused = present(
        node,
        decision,
        &[Arc::new(tampered)],
        PresentationId(99),
        context(),
    );
    let reason = contract_reason(refused);
    assert!(reason.contains("differs from the file"), "{reason}");
}

/// The Induced path takes its family from the top, the others build one: both
/// refuse a context other than `{d}`.
#[test]
fn a_foreign_context_is_refused() {
    let foreign = CoefficientContext::try_new(["d", "e"]).unwrap();
    let (induced, decision, _) = lanes(3).by_name("nostar0");
    let refused = present(induced, decision, &tops().1, PresentationId(99), &foreign);
    assert!(
        matches!(refused, Err(PError::ContextMismatch)),
        "{refused:?}"
    );
    let (adapted, decision, _) = lanes(2).by_name("d22");
    let refused = present(adapted, decision, &tops().1, PresentationId(99), &foreign);
    assert!(
        matches!(refused, Err(PError::ContextMismatch)),
        "{refused:?}"
    );
}

/// The production input: vakint's families give Forcer's DAG by name.
#[test]
fn the_production_dag_is_forcers() {
    let names = |loops: u8| -> Vec<String> {
        let dag = production_dag(loops);
        let names: Vec<&str> = dag.nodes.iter().map(|n| forcer_name(&n.key)).collect();
        sorted_names(&names)
    };
    assert_eq!(names(2), sorted_names(&DAG_2L));
    assert_eq!(names(3), sorted_names(&DAG_3L));
    let dag4 = production_dag(4);
    assert_eq!(dag4.nodes.len(), 438);
    let keys: BTreeSet<&GraphKey> = dag4.nodes.iter().map(|node| &node.key).collect();
    assert_eq!(keys, forcer_names().keys().collect());
    let forcer_tops: Vec<PTopo> = forcer::forcer_tops(4).into_iter().map(|t| t.1).collect();
    let golden = reduction_graph(&forcer_tops, ChildMode::ForcerTree);
    assert_eq!(dag4, golden, "vakint's and Forcer's tops give one DAG");
}

#[test]
fn every_vakint_top_is_exactly_one_forcer_top() {
    let key = |topo: &PTopo| match normalise(topo) {
        Normalised::Topo(normal, _) => canonical(&normal).0,
        other => panic!("{topo:?}: {other:?}"),
    };
    let (file, _) = tops();
    for loops in 2..=4 {
        let forcer_tops: Vec<(String, GraphKey)> = forcer::forcer_tops(loops)
            .into_iter()
            .map(|(name, topo)| {
                let top_key = key(&topo);
                (name, top_key)
            })
            .collect();
        let vakint_tops: Vec<_> = file.families.iter().filter(|f| f.loops == loops).collect();
        assert_eq!(vakint_tops.len(), forcer_tops.len(), "{loops}L");
        let mut matched = BTreeSet::new();
        for entry in vakint_tops {
            let top_key = key(&top_topology(entry));
            let hits: Vec<&str> = forcer_tops
                .iter()
                .filter(|(_, candidate)| *candidate == top_key)
                .map(|(name, _)| name.as_str())
                .collect();
            assert_eq!(hits.len(), 1, "{}: {hits:?}", entry.name);
            assert!(matched.insert(hits[0]), "{}: {hits:?}", entry.name);
        }
    }
}

fn row(momentum: &[i64]) -> String {
    let entries: Vec<String> = momentum.iter().map(|c| format!("{c:>2}")).collect();
    format!("[{}]", entries.join(" "))
}

fn slot_text(slot: Option<u16>) -> String {
    slot.map_or_else(|| "-".to_owned(), |slot| slot.to_string())
}

/// One line: a rule edge into the trivial node has no map.
fn dump_edge(built: &Lanes, edge: &DagEdge) -> String {
    let name = |index: u32| forcer_name(&built.dag.nodes[index as usize].key);
    let (parent, child) = (name(edge.parent), name(edge.child));
    let Some(map) = built.map_of(edge) else {
        return format!("edge {parent} -> {child} no map");
    };
    let outer = &map.outer_map;
    let slots: Vec<String> = map.line_slots.iter().copied().map(slot_text).collect();
    format!(
        "edge {parent} -> {child} kind={:?} integrated_loop={} \
         outer_map=linear{:?} external{:?} sign{:+} line_slots=[{}] merged_slot={}",
        map.kind,
        map.integrated_loop,
        outer.loop_linear,
        outer.loop_external,
        outer.external_sign,
        slots.join(" "),
        slot_text(map.merged_slot)
    )
}

fn dump(built: &Lanes, loops: u8) -> String {
    let mut text = format!(
        "# Node presentations and rule edge maps of the {loops}L DAG, \
         P_PRESENTATION_VERSION {P_PRESENTATION_VERSION}.\n\
         # Momenta over (k_1, .., k_L, q).\n"
    );
    for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
        let name = forcer_name(&node.key);
        let Some(presentation) = built.presentation(node.index) else {
            writeln!(text, "node {name} {} trivial", node.key.to_hex()).unwrap();
            continue;
        };
        writeln!(
            text,
            "node {name} {} L={} t={} lane={:?} kind={} roles={:?}",
            node.key.to_hex(),
            node.loops,
            node.t,
            decision.lane,
            dump_kind(&presentation.kind),
            decision.roles
        )
        .unwrap();
        for (index, slot) in presentation.slots.iter().enumerate() {
            match slot {
                Slot::Line(line) => {
                    let row = row(&line.momentum);
                    writeln!(text, "  {index:>2} line {row} eps {}", line.eps_units).unwrap();
                }
                Slot::Isp(isp) => {
                    let (left, right) = (row(&isp.left), row(&isp.right));
                    writeln!(text, "  {index:>2} isp  {left} . {right}").unwrap();
                }
            }
        }
    }
    for edge in built.rule_edges() {
        writeln!(text, "{}", dump_edge(built, edge)).unwrap();
    }
    text
}

/// Tops by name: ids are library-local.
fn dump_kind(kind: &PresentationKind) -> String {
    match kind {
        PresentationKind::Induced { top, mask } => format!(
            "Induced({}, {mask:#x})",
            tops().0.families[top.0 as usize].name
        ),
        other => format!("{other:?}"),
    }
}

/// A dump fails when its content changes without `P_PRESENTATION_VERSION`.
#[test]
fn presentation_dumps_are_pinned_to_their_version() {
    let (version, pins) = DUMP_PINS;
    let bless = std::env::var_os("RUSTRED_P_BLESS").is_some();
    for (loops, pin) in pins {
        let text = dump(lanes(loops), loops);
        let path = Path::new(GOLDEN_DIR).join(format!("presentations_L{loops}.txt"));
        if bless {
            std::fs::write(&path, &text).unwrap();
            println!("{loops}L pin {}", blake3::hash(text.as_bytes()).to_hex());
            continue;
        }
        let golden = std::fs::read_to_string(&path).unwrap();
        assert!(
            golden == text,
            "{loops}L presentations differ from {}: bump P_PRESENTATION_VERSION, re-bless, new pin",
            path.display()
        );
        assert_eq!(
            (
                P_PRESENTATION_VERSION,
                blake3::hash(text.as_bytes()).to_hex().as_str()
            ),
            (version, pin),
            "{loops}L dump: bump P_PRESENTATION_VERSION with every new pin"
        );
    }
}

/// Two builds give the same bytes, and a node does not depend on its DAG: its
/// presentation is a function of the key, the lane and the tops.
#[test]
fn presentations_are_deterministic_and_independent_of_the_dag() {
    assert_eq!(dump(&Lanes::build(3), 3), dump(lanes(3), 3));
    let (three, four) = (lanes(3), lanes(4));
    for node in &three.dag.nodes {
        let other = four.dag.find(&node.key).unwrap();
        assert_eq!(node.topo, other.topo);
        assert_eq!(node.action, other.action);
        let (index, other_index) = (node.index as usize, other.index as usize);
        assert_eq!(three.decisions[index], four.decisions[other_index]);
        match (
            three.presentation(node.index),
            four.presentation(other.index),
        ) {
            (Some(a), Some(b)) => {
                assert_eq!(a.slots, b.slots, "{}", forcer_name(&node.key));
                assert_eq!(a.kind, b.kind);
                assert_eq!(a.family.name(), b.family.name());
            }
            (None, None) => {}
            _ => panic!("{}", forcer_name(&node.key)),
        }
    }
}

/// `decide` and `present` for all 438 nodes, plus every contract check. An
/// unoptimised build spends some 100 ms per family in Symbolica: optimised only.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "slow: a timing budget of the optimised build"
)]
fn the_4l_presentations_build_fast() {
    let dag = production_dag(4);
    let (_, top_presentations) = tops();
    let start = Instant::now();
    for node in &dag.nodes {
        let decision = decide(node).unwrap();
        if !decision.trivial {
            let id = PresentationId(node.index);
            let presentation = present(node, &decision, top_presentations, id, context()).unwrap();
            check_contract(&presentation, &decision);
        }
    }
    let elapsed = start.elapsed();
    println!("4L decide + present + contract: {elapsed:?}");
    assert!(elapsed.as_secs_f64() < 5.0, "{elapsed:?}");
}

/// Non-unit ISP slots (p5l-maps-rs): every eps Custom node of 3L and 4L with
/// ISPs, given Forcer-style edge-pair products `p_a.p_b` (lines and `q`)
/// first, gets a valid presentation whose ISPs are exactly the first
/// independent pairs (unit rows only where the pairs do not span), keeps its
/// line rows, and expresses every pair product; no preference is `present`.
#[test]
fn edge_pair_isps_present_eps_nodes() {
    let mut checked = 0;
    for loops in [3, 4] {
        let built = lanes(loops);
        for (node, decision) in built.dag.nodes.iter().zip(&built.decisions) {
            let Some(default) = built.presentation(node.index) else {
                continue;
            };
            if default.kind != PresentationKind::EpsNode {
                continue;
            }
            let lines = default
                .slots
                .iter()
                .filter(|s| matches!(s, Slot::Line(_)))
                .count();
            if lines == default.slots.len() {
                continue;
            }
            let mut pairs = Vec::new();
            for a in 0..lines as i64 {
                for b in (a + 1)..lines as i64 {
                    pairs.push([a, b]);
                }
                pairs.push([a, -1]);
            }
            let preference = IspPreference::Pairs {
                pairs: pairs.clone(),
                units: Vec::new(),
            };
            let given = present_with_isp_preference(
                node,
                decision,
                &tops().1,
                default.id,
                context(),
                Some(&preference),
            )
            .unwrap();
            given.validate().unwrap();
            assert_eq!(given.slots[..lines], default.slots[..lines]);
            assert_eq!(given.slots.len(), default.slots.len());
            let row = |i: i64| -> Vec<i64> {
                if i < 0 {
                    let mut q = vec![0; usize::from(node.loops) + 1];
                    q[usize::from(node.loops)] = 1;
                    q
                } else {
                    let Slot::Line(line) = &given.slots[i as usize] else {
                        unreachable!()
                    };
                    line.momentum.clone()
                }
            };
            // every ISP is one of the pairs, in the order offered
            let mut last = None;
            for slot in &given.slots[lines..] {
                let Slot::Isp(SlotIsp { left, right }) = slot else {
                    panic!("ISP expected")
                };
                let position = pairs
                    .iter()
                    .position(|&[a, b]| row(a) == *left && row(b) == *right)
                    .expect("a pair product");
                assert!(last.is_none_or(|l| l < position));
                last = Some(position);
            }
            for &[a, b] in &pairs {
                given.express_scalar_product(&row(a), &row(b)).unwrap();
            }
            let again = present_with_isp_preference(
                node,
                decision,
                &tops().1,
                default.id,
                context(),
                Some(&IspPreference::Default),
            )
            .unwrap();
            assert_eq!(again.slots, default.slots);
            checked += 1;
        }
    }
    assert!(checked > 0);
}
