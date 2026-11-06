//! The Triangle template step (owner: M2-T5 rr-triangle): the row read from
//! RustRed's own IBP generator, its shape on every triangle node of the production
//! DAGs, the step closed onto series-parallel leaves against P0 SpIRed reductions,
//! and the eps apex against the integer row and every IBP row of its family.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits, IndexedAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::identity::{ParametricIbpGenerator, ParametricRelation, RowId};
use rustred::propagator::P_TRIANGLE_TEMPLATE_VERSION;
use rustred::propagator::census::{PCensus, PClassRecord};
use rustred::propagator::class::{ClassLimits, classify};
use rustred::propagator::decide::decide;
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile, load_p_families};
use rustred::propagator::lane::{LaneRoles, OwnerLane, TriangleRoles};
use rustred::propagator::library::{PLibrary, POwnerSpec};
use rustred::propagator::present::{present_node, top_topology};
use rustred::propagator::presentation::{
    Presentation, PresentationKind, Slot, SlotIsp, SlotLine, eps_power_shift,
};
use rustred::propagator::step::{PKeyView, PLaneStep, PStage, PStepContext};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::{
    ChildMode, Dag, GraphKey, NodeInfo, Normalised, PTopo, is_series_parallel, normalise,
    reduction_graph,
};
use rustred::propagator::triangle::{
    RowForm, RowForms, TemplateError, TemplateMismatch, TriangleStep, TriangleTemplate,
};
use rustred::propagator::{OwnerClassId, PresentationId};
use rustred::reduction::ReductionLimits;
use rustred::solver::litered::{DynamicLiteRedSession, LiteRedOptions};
use symbolica::domains::rational::{Q, Rational};

use crate::support::p_integrals::{GValue, Power, collect, g, rational};

#[path = "../../../rustred-topology/tests/support/forcer.rs"]
mod forcer;

// ---------------------------------------------------------------------------
// The production DAGs
// ---------------------------------------------------------------------------

fn tops() -> &'static (PFamiliesFile, Vec<Arc<Presentation>>) {
    static TOPS: OnceLock<(PFamiliesFile, Vec<Arc<Presentation>>)> = OnceLock::new();
    TOPS.get_or_init(|| load_p_families(P_FAMILIES_V1_JSON).unwrap())
}

fn context() -> &'static CoefficientContext {
    tops().1[0].family.coefficient_context()
}

fn limits() -> ReductionLimits {
    ReductionLimits::default()
}

fn exact_limits() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}

fn forcer_names() -> &'static BTreeMap<GraphKey, String> {
    static NAMES: OnceLock<BTreeMap<GraphKey, String>> = OnceLock::new();
    NAMES.get_or_init(|| forcer::forcer_names_by_key(&forcer::forcer_dag()))
}

/// The DAG of vakint's `loops`-loop tops: the production input.
fn production_dag(loops: u8) -> Dag {
    let roots: Vec<PTopo> = tops()
        .0
        .families
        .iter()
        .filter(|entry| entry.loops == loops)
        .map(top_topology)
        .collect();
    reduction_graph(&roots, ChildMode::ForcerTree)
}

/// A node of the Triangle lane with its presentation.
struct TriangleNode {
    name: String,
    node: NodeInfo,
    presentation: Arc<Presentation>,
    roles: TriangleRoles,
}

fn triangle_nodes_of(dag: &Dag, name: impl Fn(&GraphKey) -> String) -> Vec<TriangleNode> {
    let (_, top_presentations) = tops();
    let first_id = u32::try_from(top_presentations.len()).unwrap();
    let triangles = dag
        .nodes
        .iter()
        .filter(|node| decide(node).unwrap().lane == OwnerLane::Triangle);
    triangles
        .map(|node| {
            let id = PresentationId(first_id + node.index);
            let built = present_node(node, top_presentations, id, context()).unwrap();
            assert_eq!(built.fallback, None, "{}", name(&node.key));
            let LaneRoles::Triangle(roles) = built.decision.roles else {
                panic!("{}: {:?}", name(&node.key), built.decision);
            };
            TriangleNode {
                name: name(&node.key),
                node: node.clone(),
                presentation: Arc::new(built.presentation.unwrap()),
                roles,
            }
        })
        .collect()
}

/// The Triangle nodes of the DAG of the `loops`-loop tops, Forcer's names.
fn triangle_nodes(loops: u8) -> &'static [TriangleNode] {
    static NODES: [OnceLock<Vec<TriangleNode>>; 4] = [const { OnceLock::new() }; 4];
    NODES[usize::from(loops) - 1].get_or_init(|| {
        triangle_nodes_of(&production_dag(loops), |key| {
            forcer_names()
                .get(key)
                .unwrap_or_else(|| panic!("{key:?} is not a Forcer topology"))
                .clone()
        })
    })
}

/// Every named node lives in the 3L DAG.
fn named(name: &str) -> &'static TriangleNode {
    triangle_nodes(3)
        .iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("no Triangle node {name}"))
}

fn step_of(node: &TriangleNode) -> TriangleStep {
    TriangleStep::new(node.presentation.clone(), node.roles).unwrap()
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

/// `count` distinct rational `d`.
fn dimensions(rng: &mut StdRng, count: usize) -> Vec<Rational> {
    let mut points = Vec::with_capacity(count);
    while points.len() < count {
        // Over the prime 101, never a multiple of it: no integer or half-integer.
        let d = rational(
            101 * rng.random_range(1..20_i64) + rng.random_range(1..101),
            101,
        );
        if !points.contains(&d) {
            points.push(d);
        }
    }
    points
}

fn line_count(p: &Presentation) -> usize {
    p.slots
        .iter()
        .take_while(|slot| matches!(slot, Slot::Line(_)))
        .count()
}

/// The eps units of a line slot, zero on an ISP.
fn eps_units(p: &Presentation, slot: usize) -> u8 {
    match &p.slots[slot] {
        Slot::Line(line) => line.eps_units,
        Slot::Isp(_) => 0,
    }
}

/// A random Apply key: integer lines at `1 + dots`, `dots` distributed over the
/// lines, `rank` over the ISPs (with no ISP, no rank), an eps line anywhere in
/// `eps_range`.
fn random_key(
    p: &Presentation,
    rng: &mut StdRng,
    dots: i64,
    rank: i64,
    eps_range: (i64, i64),
) -> Vec<i64> {
    let lines = line_count(p);
    let mut powers: Vec<i64> = (0..p.slots.len())
        .map(|slot| {
            if slot >= lines {
                0
            } else if eps_units(p, slot) != 0 {
                rng.random_range(eps_range.0..=eps_range.1)
            } else {
                1
            }
        })
        .collect();
    for _ in 0..dots {
        let slot = rng.random_range(0..lines);
        if eps_units(p, slot) == 0 {
            powers[slot] += 1;
        }
    }
    if p.slots.len() > lines {
        for _ in 0..rank {
            powers[rng.random_range(lines..p.slots.len())] -= 1;
        }
    }
    powers
}

fn tag_one() -> GammaTag {
    GammaTag::one()
}

fn key_view<'a>(p: &Presentation, powers: &'a [i64], tag: &'a GammaTag) -> PKeyView<'a> {
    PKeyView {
        presentation: p.id,
        powers,
        tag,
    }
}

/// `n_b + n_c1 + n_c2`.
fn measure(roles: &TriangleRoles, powers: &[i64]) -> i64 {
    powers[usize::from(roles.b)] + powers[usize::from(roles.c[0])] + powers[usize::from(roles.c[1])]
}

/// One step on `powers`, every property of the rule asserted: four children on the
/// presentation with the parent's tag, no terminal, no condition, strict descent,
/// and Route exactly on the children with `b`, `c1` or `c2` at 0.
fn checked_step(
    node: &TriangleNode,
    step: &TriangleStep,
    powers: &[i64],
) -> Vec<(Vec<i64>, Coefficient, PStage)> {
    let tag = GammaTag::from_pairs([(1, 3), (-2, -1)]).unwrap();
    let key = key_view(&node.presentation, powers, &tag);
    let output = step.apply(&key, &limits()).unwrap();
    assert!(output.terminals.is_empty() && output.conditions.is_empty());
    assert_eq!(output.children.len(), 4, "{} {powers:?}", node.name);
    let roles = &node.roles;
    output
        .children
        .into_iter()
        .map(|child| {
            assert_eq!(child.presentation, node.presentation.id);
            assert_eq!(child.tag, tag);
            assert!(!child.coefficient.is_zero(), "{} {powers:?}", node.name);
            assert_eq!(
                measure(roles, &child.powers),
                measure(roles, powers) - 1,
                "{} {powers:?} -> {:?}",
                node.name,
                child.powers
            );
            let pinched = [roles.b, roles.c[0], roles.c[1]]
                .into_iter()
                .any(|slot| child.powers[usize::from(slot)] == 0);
            let expected = if pinched {
                PStage::Route
            } else {
                PStage::Apply
            };
            assert_eq!(child.stage, expected, "{} {:?}", node.name, child.powers);
            (child.powers, child.coefficient, child.stage)
        })
        .collect()
}

/// `I(powers) = Sum_leaves weight I(leaf)`: every Apply child stepped, ordered by
/// the descent measure; the leaves are the Route keys.
fn close(
    node: &TriangleNode,
    step: &TriangleStep,
    powers: &[i64],
) -> BTreeMap<Vec<i64>, Coefficient> {
    let context = context();
    let mut leaves: BTreeMap<Vec<i64>, Coefficient> = BTreeMap::new();
    let mut frontier = BTreeMap::from([(
        (measure(&node.roles, powers), powers.to_vec()),
        context.one(),
    )]);
    let add = |slot: &mut Coefficient, value: &Coefficient| {
        *slot = context.try_add(slot, value, exact_limits()).unwrap();
    };
    while let Some(((_, parent), weight)) = frontier.pop_last() {
        for (powers, coefficient, stage) in checked_step(node, step, &parent) {
            let term = context
                .try_mul(&weight, &coefficient, exact_limits())
                .unwrap();
            match stage {
                PStage::Route => add(
                    leaves.entry(powers).or_insert_with(|| context.zero()),
                    &term,
                ),
                PStage::Apply => {
                    let order = (measure(&node.roles, &powers), powers);
                    add(
                        frontier.entry(order).or_insert_with(|| context.zero()),
                        &term,
                    );
                }
            }
        }
    }
    leaves.retain(|_, weight| !weight.is_zero());
    leaves
}

// ---------------------------------------------------------------------------
// Closed forms of the series-parallel leaves
// ---------------------------------------------------------------------------

/// `I_M` of a key without numerators on integer lines (convention C1), from the
/// node's graph, where line slot `i` is canonical line `i`: the Euclidean value by
/// series and parallel moves, with the sign `(-1)^{Sum n}` of the integer parts. A
/// series pair adds its powers, a parallel pair is a bubble `G(a, b)` of power
/// `a + b - d/2`, and a line left between `s` and `t` is 1 at `Q^2 = 1`. An eps line
/// `n + j eps` has the power `n + 2 j - j d/2`. `None` when the moves stop before
/// that: a numerator, or no series-parallel graph.
fn chain_value(node: &NodeInfo, powers: &[i64], d: &Rational) -> Option<GValue> {
    let topo = &node.topo;
    let (lines, isps) = powers.split_at(topo.lines.len());
    if isps.iter().any(|&power| power != 0) {
        return None;
    }
    let total: i64 = powers.iter().sum();
    let mut value = GValue::rational(rational(if total % 2 == 0 { 1 } else { -1 }, 1));
    let mut vertex: Vec<u8> = (0..topo.n).collect();
    let mut graph: Vec<(u8, u8, Power)> = Vec::new();
    for (&(u, v, eps), &power) in topo.lines.iter().zip(lines) {
        let j = i64::from(eps);
        if j == 0 && power < 0 {
            return None;
        }
        graph.push((u, v, (power + 2 * j, -j)));
    }
    // Contract the lines at power 0, then identify their vertices everywhere.
    while let Some(zero) = graph.iter().position(|line| line.2 == (0, 0)) {
        let (u, v, _) = graph.swap_remove(zero);
        let (keep, drop) = (u.min(v), u.max(v));
        for label in &mut vertex {
            if *label == drop {
                *label = keep;
            }
        }
        for line in &mut graph {
            if line.0 == drop {
                line.0 = keep;
            }
            if line.1 == drop {
                line.1 = keep;
            }
        }
    }
    let (s, t) = (vertex[usize::from(topo.s)], vertex[usize::from(topo.t)]);
    if s == t {
        return Some(GValue::zero());
    }
    loop {
        if graph.iter().any(|line| line.0 == line.1) {
            return Some(GValue::zero());
        }
        if let [(u, v, _)] = graph.as_slice() {
            return ((*u, *v) == (s, t) || (*v, *u) == (s, t)).then_some(value);
        }
        // A parallel pair is a bubble.
        let parallel = (0..graph.len()).find_map(|i| {
            ((i + 1)..graph.len())
                .find(|&j| {
                    let (a, b) = (graph[i], graph[j]);
                    (a.0, a.1) == (b.0, b.1) || (a.0, a.1) == (b.1, b.0)
                })
                .map(|j| (i, j))
        });
        if let Some((i, j)) = parallel {
            let (u, v, a) = graph[i];
            let (_, _, b) = graph.swap_remove(j);
            value = value.mul(&g(a, b, d));
            if value.is_zero() {
                return Some(value);
            }
            graph[i] = (u, v, (a.0 + b.0, a.1 + b.1 - 1));
            continue;
        }
        // A vertex inside a chain, or a dangling line, which has no scale.
        let inner = graph.iter().flat_map(|line| [line.0, line.1]).find(|&x| {
            x != s && x != t && graph.iter().filter(|l| l.0 == x || l.1 == x).count() <= 2
        })?;
        let at_inner: Vec<usize> = (0..graph.len())
            .filter(|&l| graph[l].0 == inner || graph[l].1 == inner)
            .collect();
        let [i, j] = at_inner.as_slice() else {
            return Some(GValue::zero());
        };
        let (a, b) = (graph[*i], graph[*j]);
        let other = |line: (u8, u8, Power)| if line.0 == inner { line.1 } else { line.0 };
        graph[*i] = (other(a), other(b), (a.2.0 + b.2.0, a.2.1 + b.2.1));
        graph.swap_remove(*j);
    }
}

/// The sector of `powers` in the node's graph: lines at power 0 without eps units
/// are contracted. Line slot `i` of a node presentation is canonical line `i`.
fn sector_topology(node: &NodeInfo, powers: &[i64]) -> PTopo {
    let topo = &node.topo;
    let mut parent: Vec<u8> = (0..topo.n).collect();
    fn find(parent: &mut [u8], mut x: u8) -> u8 {
        while parent[usize::from(x)] != x {
            x = parent[usize::from(x)];
        }
        x
    }
    let kept = |line: usize, eps: u8| powers[line] > 0 || eps != 0;
    for (line, &(u, v, eps)) in topo.lines.iter().enumerate() {
        if !kept(line, eps) {
            let (a, b) = (find(&mut parent, u), find(&mut parent, v));
            parent[usize::from(a.max(b))] = a.min(b);
        }
    }
    let lines: Vec<(u8, u8, u8)> = topo
        .lines
        .iter()
        .enumerate()
        .filter(|&(line, &(_, _, eps))| kept(line, eps))
        .map(|(_, &(u, v, eps))| (find(&mut parent, u), find(&mut parent, v), eps))
        .collect();
    let (s, t) = (find(&mut parent, topo.s), find(&mut parent, topo.t));
    PTopo::new(topo.n, s, t, lines)
}

/// A leaf is a subsector of the node whose graph is series-parallel, or has no
/// scale; either way a chain of bubbles gives its value.
fn assert_series_parallel_leaf(node: &TriangleNode, leaf: &[i64], d: &Rational) {
    let lines = line_count(&node.presentation);
    let full = (0..lines).all(|slot| leaf[slot] > 0 || eps_units(&node.presentation, slot) != 0);
    assert!(
        !full,
        "{}: the leaf {leaf:?} is the sector itself",
        node.name
    );
    let sector = sector_topology(&node.node, leaf);
    match normalise(&sector) {
        Normalised::Zero | Normalised::Trivial => {}
        Normalised::Topo(normal, _) => {
            assert!(
                is_series_parallel(&normal),
                "{}: the leaf {leaf:?} has the graph {normal:?}",
                node.name
            );
        }
    }
    assert!(
        chain_value(&node.node, leaf, d).is_some(),
        "{}: the leaf {leaf:?} is no chain of bubbles",
        node.name
    );
}

/// `Sum_leaves weight I_M(leaf)` at `d`, each term one Gamma monomial: comparing
/// them collected is the comparison in the tag basis.
fn leaves_value(
    node: &TriangleNode,
    leaves: &BTreeMap<Vec<i64>, Coefficient>,
    d: &Rational,
) -> Vec<GValue> {
    leaves
        .iter()
        .map(|(leaf, weight)| {
            chain_value(&node.node, leaf, d)
                .unwrap_or_else(|| panic!("{}: the leaf {leaf:?} is no chain", node.name))
                .scale(&at(weight, d))
        })
        .collect()
}

fn assert_values_equal(left: &[GValue], right: &[GValue], label: &str) {
    let minus = rational(-1, 1);
    let difference = left
        .iter()
        .cloned()
        .chain(right.iter().map(|value| value.scale(&minus)));
    assert!(
        collect(difference).is_empty(),
        "{label}: {:?} differs from {:?}",
        collect(left.iter().cloned()),
        collect(right.iter().cloned())
    );
}

// ---------------------------------------------------------------------------
// The P0 lane
// ---------------------------------------------------------------------------

fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

/// A presentation's family under the P0 lane: all-rank SpIRed rules, the family as
/// it stands, so its reductions hold for the `I_M` of the presentation.
struct ReductionLane {
    lane: DynamicLiteRedSession,
    masters: Vec<IntegralKey>,
}

impl ReductionLane {
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

    /// The coefficients of `keys` on the masters at `d`.
    fn rows(&mut self, keys: &[Vec<i64>], d: &Rational) -> Vec<Vec<Rational>> {
        let keys: Vec<IntegralKey> = keys
            .iter()
            .map(|powers| IntegralKey::try_new(powers.clone()).unwrap())
            .collect();
        let ids = self.lane.add_targets(&keys).unwrap();
        assert_eq!(self.lane.statistics(&ids).uncovered, 0);
        self.lane
            .reduce_exact_at_point(std::slice::from_ref(d), &ids)
            .unwrap()
            .coefficients
    }

    /// `I_M` of a coefficient row, the masters being chains of bubbles.
    fn value(&self, node: &NodeInfo, row: &[Rational], d: &Rational) -> Vec<GValue> {
        self.masters
            .iter()
            .zip(row)
            .filter(|(_, coefficient)| !coefficient.is_zero())
            .map(|(master, coefficient)| {
                chain_value(node, master.powers(), d)
                    .unwrap_or_else(|| panic!("the master {:?} is no chain", master.powers()))
                    .scale(coefficient)
            })
            .collect()
    }
}

fn add_scaled(sum: &mut [Rational], row: &[Rational], factor: &Rational) {
    for (entry, value) in sum.iter_mut().zip(row) {
        *entry = &*entry + &(value * factor);
    }
}

// ---------------------------------------------------------------------------
// The shape of the row
// ---------------------------------------------------------------------------

/// `b, a, c, j (eps units of a), slots of the k-linear ISPs`, read off the golden
/// presentation dump `golden/presentations_L3.txt`.
struct Shape {
    name: &'static str,
    b: u16,
    a: [u16; 2],
    c: [u16; 2],
    j: [u8; 2],
    linear_isps: &'static [u16],
}

const EXACT_SHAPES: [Shape; 6] = [
    Shape {
        name: "d43",
        b: 0,
        a: [1, 3],
        c: [2, 4],
        j: [0, 0],
        linear_isps: &[],
    },
    Shape {
        name: "d30",
        b: 0,
        a: [2, 4],
        c: [1, 3],
        j: [0, 1],
        linear_isps: &[],
    },
    Shape {
        name: "d118",
        b: 0,
        a: [2, 6],
        c: [1, 3],
        j: [0, 0],
        linear_isps: &[7],
    },
    Shape {
        name: "d137",
        b: 0,
        a: [1, 3],
        c: [2, 4],
        j: [0, 0],
        linear_isps: &[7],
    },
    Shape {
        name: "d192",
        b: 0,
        a: [2, 4],
        c: [1, 3],
        j: [0, 0],
        linear_isps: &[8],
    },
    Shape {
        name: "d197",
        b: 3,
        a: [4, 6],
        c: [5, 7],
        j: [0, 0],
        linear_isps: &[8],
    },
];

/// RustRed's rows of `p`, by id.
fn relations(p: &Presentation) -> Vec<ParametricRelation> {
    let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..batch.len()).map(|row| batch.generate(row)).collect();
    batch.complete(rows).unwrap().into_relations()
}

/// `(4 - d) / 2`.
fn eps(j: i64) -> Coefficient {
    let units = u8::try_from(j).unwrap();
    eps_power_shift(context(), units).unwrap()
}

fn add(a: &Coefficient, b: &Coefficient) -> Coefficient {
    context().try_add(a, b, exact_limits()).unwrap()
}

fn sub(a: &Coefficient, b: &Coefficient) -> Coefficient {
    context().try_sub(a, b, exact_limits()).unwrap()
}

fn neg(a: &Coefficient) -> Coefficient {
    context().try_neg(a, exact_limits()).unwrap()
}

fn int(value: i64) -> Coefficient {
    context().integer(value)
}

/// Random integer powers on every slot, of either sign.
fn random_powers(p: &Presentation, rng: &mut StdRng) -> Vec<i64> {
    (0..p.slots.len())
        .map(|_| rng.random_range(-6..=6))
        .collect()
}

/// The six named nodes: the formula of the row, written out here with the pinned
/// roles and eps units, is the row of the generator entry by entry, in `d` and at
/// random powers, and what the template solves.
#[test]
fn the_named_nodes_have_the_exact_row() {
    let mut rng = StdRng::seed_from_u64(0x7a11);
    let d = context().parameter("d").unwrap();
    for shape in &EXACT_SHAPES {
        let node = named(shape.name);
        let p = &*node.presentation;
        assert_eq!(
            (node.roles.b, node.roles.a, node.roles.c, node.roles.k_loop),
            (shape.b, shape.a, shape.c, 0),
            "{}",
            shape.name
        );
        let j = shape.j.map(i64::from);
        assert_eq!(j[0], i64::from(eps_units(p, usize::from(shape.a[0]))));
        assert_eq!(j[1], i64::from(eps_units(p, usize::from(shape.a[1]))));
        let linear: Vec<u16> = (line_count(p)..p.slots.len())
            .filter(|&slot| match &p.slots[slot] {
                Slot::Isp(isp) => isp.left[0] != 0 || isp.right[0] != 0,
                Slot::Line(_) => false,
            })
            .map(|slot| u16::try_from(slot).unwrap())
            .collect();
        assert_eq!(linear, shape.linear_isps, "{}", shape.name);

        let template = TriangleTemplate::extract(p, &node.roles).unwrap();
        let rows = relations(p);
        let row = rows
            .iter()
            .find(|row| {
                *row.row_id()
                    == RowId::OrdinaryIbp {
                        contraction_momentum: 0,
                        differentiated_loop: 0,
                    }
            })
            .unwrap();
        assert_eq!(row.terms().len(), 5, "{}", shape.name);
        let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
        let indexed = generator.context();
        let entry = |shift: &[(u16, i64)], powers: &[i64]| {
            let mut wanted = vec![0_i64; p.slots.len()];
            for &(slot, step) in shift {
                wanted[usize::from(slot)] = step;
            }
            let (_, value) = row
                .terms()
                .iter()
                .find(|(key, _)| key.values() == wanted)
                .unwrap_or_else(|| panic!("{}: no shift {wanted:?}", shape.name));
            indexed
                .specialize(value, powers, IndexedAlgebraLimits::default())
                .unwrap()
                .0
        };

        for _ in 0..20 {
            let n = random_powers(p, &mut rng);
            let at_slot = |slot: u16| n[usize::from(slot)];
            let numerator_degree: i64 = shape.linear_isps.iter().map(|&s| -at_slot(s)).sum();
            let integer_part =
                numerator_degree - at_slot(shape.a[0]) - at_slot(shape.a[1]) - 2 * at_slot(shape.b);
            let pivot = sub(&add(&d, &int(integer_part)), &eps(j[0] + j[1]));
            let apex = [0, 1].map(|i| add(&int(at_slot(shape.a[i])), &eps(j[i])));

            assert_eq!(entry(&[], &n), pivot, "{} pivot at {n:?}", shape.name);
            for (i, x) in apex.iter().enumerate() {
                let lower_b = [(shape.a[i], 1), (shape.b, -1)];
                let lower_c = [(shape.a[i], 1), (shape.c[i], -1)];
                assert_eq!(entry(&lower_b, &n), neg(x), "{} at {n:?}", shape.name);
                assert_eq!(entry(&lower_c, &n), *x, "{} at {n:?}", shape.name);
            }

            let terms = template.solve(&n, exact_limits()).unwrap();
            let weight = |i: usize| context().try_div(&apex[i], &pivot, exact_limits()).unwrap();
            let shifted = |raised: u16, lowered: u16| {
                let mut child = n.clone();
                child[usize::from(raised)] += 1;
                child[usize::from(lowered)] -= 1;
                child
            };
            let expected = [
                (shifted(shape.a[0], shape.b), weight(0), shape.b),
                (shifted(shape.a[0], shape.c[0]), neg(&weight(0)), shape.c[0]),
                (shifted(shape.a[1], shape.b), weight(1), shape.b),
                (shifted(shape.a[1], shape.c[1]), neg(&weight(1)), shape.c[1]),
            ];
            for (term, (powers, weight, lowered)) in terms.iter().zip(expected) {
                assert_eq!(term.powers, powers, "{}", shape.name);
                assert_eq!(term.weight, weight, "{} at {n:?}", shape.name);
                assert_eq!(term.lowered, lowered, "{}", shape.name);
            }
        }
    }
}

/// The apex and pivot forms of every Triangle node of the production DAGs: the
/// extraction is the shape check, so success is the claim; the count pins that no
/// node fell out of the lane. The `d` coefficient of the pivot never vanishes.
#[test]
fn every_triangle_node_of_the_4l_dag_has_the_shape() {
    assert_eq!(triangle_nodes(1).len(), 0);
    assert_eq!(triangle_nodes(2).len(), 1);
    assert_eq!(triangle_nodes(3).len(), 6);
    let nodes = triangle_nodes(4);
    assert_eq!(nodes.len(), 53);
    let mut with_eps_apex = 0;
    for node in nodes {
        let p = &*node.presentation;
        let template = TriangleTemplate::extract(p, &node.roles)
            .unwrap_or_else(|error| panic!("{}: {error}", node.name));
        let units: i64 = node
            .roles
            .a
            .iter()
            .map(|&slot| i64::from(eps_units(p, usize::from(slot))))
            .sum();
        with_eps_apex += i64::from(units != 0);
        // P at d = 0 and d = 2 fixes its slope in d: 1 + (j1 + j2) / 2.
        let constant = &template.pivot().constant;
        let slope =
            (&at(constant, &rational(2, 1)) - &at(constant, &rational(0, 1))) / rational(2, 1);
        assert_eq!(slope, rational(2 + units, 2), "{}", node.name);
    }
    assert!(with_eps_apex > 1, "{with_eps_apex} eps apex nodes");
}

/// `n_ai -> n_ai + j_i eps` in the integer row is the eps row, symbolically: on
/// d30, and on every node with an eps apex of the 4L DAG. The twin family has the
/// same momenta without eps units.
#[test]
fn the_eps_row_is_the_integer_row_at_shifted_apex_powers() {
    let mut seen = 0;
    for node in triangle_nodes(4) {
        let p = &*node.presentation;
        let forced = p.forced_axes();
        let on_apex = node.roles.a.iter().any(|&slot| forced >> slot & 1 == 1);
        if !on_apex {
            continue;
        }
        seen += 1;
        let eps_row = TriangleTemplate::extract(p, &node.roles).unwrap();
        let slots = p
            .slots
            .iter()
            .map(|slot| match slot {
                Slot::Line(line) => Slot::Line(SlotLine {
                    momentum: line.momentum.clone(),
                    eps_units: 0,
                }),
                Slot::Isp(isp) => Slot::Isp(isp.clone()),
            })
            .collect();
        let twin = Presentation::from_slots(
            p.id,
            PresentationKind::TriangleAdapted,
            &format!("{}_integer_twin", node.name),
            p.loops,
            slots,
            context(),
            None,
        )
        .unwrap();
        let integer_row = TriangleTemplate::extract(&twin, &node.roles).unwrap();

        // Substitute `n_slot -> n_slot + j_slot eps` on the apex lines of a form.
        let substituted = |form: &RowForm| {
            let mut constant = form.constant.clone();
            for &(slot, slope) in &form.slopes {
                if node.roles.a.contains(&slot) {
                    let shift = eps(i64::from(eps_units(p, usize::from(slot))));
                    let term = context()
                        .try_mul(&int(slope), &shift, exact_limits())
                        .unwrap();
                    constant = add(&constant, &term);
                }
            }
            RowForm {
                constant,
                slopes: form.slopes.clone(),
            }
        };
        assert_eq!(
            *eps_row.pivot(),
            substituted(integer_row.pivot()),
            "{}",
            node.name
        );
        for i in 0..2 {
            assert_eq!(
                eps_row.apex()[i],
                substituted(&integer_row.apex()[i]),
                "{}",
                node.name
            );
        }
    }
    assert!(seen > 1, "{seen} eps apex nodes, d30 among them");
    let d30 = named("d30");
    assert_eq!(d30.presentation.forced_axes(), 1 << d30.roles.a[1]);
}

// ---------------------------------------------------------------------------
// The kite against P0
// ---------------------------------------------------------------------------

/// Every key of the sector of `p` with at most `dots` dots in total over its integer
/// lines, each exactly once: integer lines `1 + dots`, eps lines at 1, ISPs at 0.
fn dotted_keys(p: &Presentation, dots: i64) -> Vec<Vec<i64>> {
    let lines = line_count(p);
    let corner: Vec<i64> = (0..p.slots.len())
        .map(|slot| i64::from(slot < lines))
        .collect();
    let mut keys = BTreeSet::from([corner]);
    for _ in 0..dots {
        let dotted: Vec<Vec<i64>> = keys
            .iter()
            .flat_map(|key| {
                (0..lines)
                    .filter(|&slot| eps_units(p, slot) == 0)
                    .map(move |slot| {
                        let mut key = key.clone();
                        key[slot] += 1;
                        key
                    })
            })
            .collect();
        keys.extend(dotted);
    }
    keys.into_iter().collect()
}

/// [`dotted_keys`] with the eps line `slot` at every power of `powers`.
fn with_eps_powers(
    keys: &[Vec<i64>],
    slot: usize,
    powers: std::ops::RangeInclusive<i64>,
) -> Vec<Vec<i64>> {
    keys.iter()
        .flat_map(|key| {
            powers.clone().map(move |power| {
                let mut key = key.clone();
                key[slot] = power;
                key
            })
        })
        .collect()
}

/// The P0 rows of `keys`, deduplicated, at `d`.
fn rows_by_key(
    lane: &mut ReductionLane,
    keys: impl IntoIterator<Item = Vec<i64>>,
    d: &Rational,
) -> BTreeMap<Vec<i64>, Vec<Rational>> {
    let keys: Vec<Vec<i64>> = keys
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let rows = lane.rows(&keys, d);
    keys.into_iter().zip(rows).collect()
}

/// The kite through the step and its series-parallel leaves equals the P0 SpIRed
/// reductions, in the Gamma monomials, for all 56 distinct keys with at most 3 dots in
/// total, each at the same 50 distinct random `d`. Numerators cannot appear here: the
/// kite's five slots are its five lines, so a key with rank has a line at a negative
/// power, which the census routes to an SP owner; see the next test for the rank.
#[test]
fn the_kite_d43_equals_p0_sector_reductions() {
    let node = named("d43");
    let p = &*node.presentation;
    let step = step_of(node);
    let mut lane = ReductionLane::new(p);
    assert_eq!(
        lane.masters.len(),
        2,
        "the kite's sunset and bubble squared"
    );
    let keys = dotted_keys(p, 3);
    assert_eq!(keys.len(), 56);
    let closed: Vec<_> = keys.iter().map(|key| close(node, &step, key)).collect();
    let mut rng = StdRng::seed_from_u64(0x0d43);
    let mut nonzero = 0;
    for d in dimensions(&mut rng, 50) {
        let rows = lane.rows(&keys, &d);
        for ((key, leaves), row) in keys.iter().zip(&closed).zip(&rows) {
            for leaf in leaves.keys() {
                assert_series_parallel_leaf(node, leaf, &d);
            }
            let from_step = leaves_value(node, leaves, &d);
            let from_p0 = lane.value(&node.node, row, &d);
            nonzero += usize::from(!collect(from_p0.iter().cloned()).is_empty());
            assert_values_equal(&from_step, &from_p0, &format!("d43 {key:?} at d = {d}"));
        }
    }
    assert_eq!(nonzero, 50 * keys.len());
}

/// Every kite key with `a1` or `a2` at a power `<= 0`, at most `rank` in total on them
/// and at most `dots` dots in total on its lines.
fn apex_numerator_keys(node: &TriangleNode, dots: i64, rank: i64) -> Vec<Vec<i64>> {
    let p = &*node.presentation;
    let [a1, a2] = node.roles.a.map(usize::from);
    let mut keys = BTreeSet::new();
    for n1 in -rank..=dots + 1 {
        for n2 in -rank..=dots + 1 {
            let numerator = -n1.min(0) - n2.min(0);
            let used = (n1 - 1).max(0) + (n2 - 1).max(0);
            if (n1 > 0 && n2 > 0) || numerator > rank || used > dots {
                continue;
            }
            for key in dotted_keys(p, dots - used) {
                if key[a1] == 1 && key[a2] == 1 {
                    let mut key = key;
                    key[a1] = n1;
                    key[a2] = n2;
                    keys.insert(key);
                }
            }
        }
    }
    keys.into_iter().collect()
}

/// The row with numerators on the kite, on its whole domain: all 650 distinct keys with
/// `a1` or `a2` at a power `<= 0`, rank up to 4 and dots up to 3 in total, at 5 distinct
/// random `d`. Such a key is an SP sector and never an Apply key, but the row holds at
/// every integer point and its weights are these: both sides reduced by P0.
#[test]
fn the_kite_row_holds_with_numerators_on_its_apex() {
    let node = named("d43");
    let p = &*node.presentation;
    let template = TriangleTemplate::extract(p, &node.roles).unwrap();
    let mut lane = ReductionLane::new(p);
    let keys = apex_numerator_keys(node, 3, 4);
    assert_eq!(keys.len(), 650);
    let terms: Vec<_> = keys
        .iter()
        .map(|key| template.solve(key, exact_limits()).unwrap())
        .collect();
    let needed = keys
        .iter()
        .cloned()
        .chain(terms.iter().flatten().map(|term| term.powers.clone()));
    let needed: Vec<Vec<i64>> = needed.collect();
    let mut rng = StdRng::seed_from_u64(0x4a2c);
    let mut nonzero = 0;
    for d in dimensions(&mut rng, 5) {
        let rows = rows_by_key(&mut lane, needed.iter().cloned(), &d);
        for (key, terms) in keys.iter().zip(&terms) {
            let mut right = vec![Rational::zero(); lane.masters.len()];
            for term in terms {
                add_scaled(&mut right, &rows[&term.powers], &at(&term.weight, &d));
            }
            let left = &rows[key];
            nonzero += usize::from(left.iter().any(|entry| !entry.is_zero()));
            assert_eq!(*left, right, "d43 {key:?} at d = {d}");
        }
    }
    assert!(
        nonzero > 5 * 100,
        "{nonzero} nonzero of {} checks",
        5 * keys.len()
    );
}

/// One step and the whole closure of the 3L triangle nodes against P0 reductions of
/// their own family, on the masters: 8 distinct random keys, dots up to 3 and rank up
/// to 4 in total, at 2 distinct random `d`.
fn three_loop_node_equals_p0(name: &str, seed: u64) {
    let node = named(name);
    let p = &*node.presentation;
    let step = step_of(node);
    let mut lane = ReductionLane::new(p);
    let mut rng = StdRng::seed_from_u64(seed);
    let mut keys = BTreeSet::new();
    while keys.len() < 8 {
        let dots = rng.random_range(0..=3);
        let rank = rng.random_range(0..=4);
        keys.insert(random_key(p, &mut rng, dots, rank, (0, 0)));
    }
    let mut nonzero = 0;
    for d in dimensions(&mut rng, 2) {
        for key in &keys {
            let children = checked_step(node, &step, key);
            let powers: Vec<Vec<i64>> = children.iter().map(|child| child.0.clone()).collect();
            let rows = lane.rows(&powers, &d);
            let mut right = vec![Rational::zero(); lane.masters.len()];
            for ((_, weight, _), row) in children.iter().zip(&rows) {
                add_scaled(&mut right, row, &at(weight, &d));
            }
            let left = lane.rows(std::slice::from_ref(key), &d).remove(0);
            nonzero += usize::from(left.iter().any(|entry| !entry.is_zero()));
            assert_eq!(left, right, "{name} {key:?} one step at d = {d}");

            let leaves = close(node, &step, key);
            let powers: Vec<Vec<i64>> = leaves.keys().cloned().collect();
            let rows = lane.rows(&powers, &d);
            let mut right = vec![Rational::zero(); lane.masters.len()];
            for (weight, row) in leaves.values().zip(&rows) {
                add_scaled(&mut right, row, &at(weight, &d));
            }
            assert_eq!(left, right, "{name} {key:?} closed at d = {d}");
        }
    }
    assert!(nonzero > 0, "{name}: every key vanished");
}

#[test]
#[ignore = "slow: P0 generation of an 8-line 3L family"]
fn the_3l_node_d192_equals_p0_reductions() {
    three_loop_node_equals_p0("d192", 0x0192);
}

#[test]
#[ignore = "slow: P0 generation of an 8-line 3L family"]
fn the_3l_node_d197_equals_p0_reductions() {
    three_loop_node_equals_p0("d197", 0x0197);
}

#[test]
#[ignore = "slow: P0 generation of a 7-line 3L family"]
fn the_3l_node_d118_equals_p0_reductions() {
    three_loop_node_equals_p0("d118", 0x0118);
}

#[test]
#[ignore = "slow: P0 generation of a 7-line 3L family"]
fn the_3l_node_d137_equals_p0_reductions() {
    three_loop_node_equals_p0("d137", 0x0137);
}

// ---------------------------------------------------------------------------
// The eps apex
// ---------------------------------------------------------------------------

/// d30 closes onto series-parallel leaves, on its whole domain: all 245 distinct keys
/// with at most 3 dots in total on the four integer lines and the eps line at every
/// power in `-3..=3`, each at the same 20 distinct random `d`. The descent is strict
/// all the way ([`checked_step`]).
#[test]
fn the_eps_apex_d30_closes_onto_series_parallel_leaves() {
    let node = named("d30");
    let p = &*node.presentation;
    let step = step_of(node);
    let eps_slot = usize::from(node.roles.a[1]);
    assert_eq!(eps_units(p, eps_slot), 1);
    let keys = with_eps_powers(&dotted_keys(p, 3), eps_slot, -3..=3);
    assert_eq!(keys.len(), 245);
    assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 245);
    let closed: Vec<_> = keys.iter().map(|key| close(node, &step, key)).collect();
    let mut rng = StdRng::seed_from_u64(0x0d30);
    let points = dimensions(&mut rng, 20);
    for (key, leaves) in keys.iter().zip(&closed) {
        assert!(!leaves.is_empty(), "d30 {key:?}");
        for leaf in leaves.keys() {
            assert_series_parallel_leaf(node, leaf, &points[0]);
        }
    }
    for d in &points {
        for (key, leaves) in keys.iter().zip(&closed) {
            for (leaf, weight) in leaves {
                assert_ne!(at(weight, d), Rational::zero(), "d30 {key:?} -> {leaf:?}");
            }
            let value = collect(leaves_value(node, leaves, d));
            assert!(!value.is_empty(), "d30 {key:?} at d = {d}");
        }
    }
}

/// The value of a key: the closure onto chains of bubbles when the key is in the
/// sector, the chain itself otherwise. Memoised symbolically by `closures`.
fn eps_family_value(
    node: &TriangleNode,
    step: &TriangleStep,
    closures: &mut HashMap<Vec<i64>, BTreeMap<Vec<i64>, Coefficient>>,
    key: &[i64],
    d: &Rational,
) -> Vec<GValue> {
    let p = &*node.presentation;
    let in_sector = (0..line_count(p)).all(|slot| key[slot] > 0 || eps_units(p, slot) != 0);
    if !in_sector {
        let value = chain_value(&node.node, key, d)
            .unwrap_or_else(|| panic!("{}: {key:?} is no chain", node.name));
        return vec![value];
    }
    let leaves = closures
        .entry(key.to_vec())
        .or_insert_with(|| close(node, step, key));
    leaves_value(node, leaves, d)
}

/// The values of d30 and of the integer kite, from the step and the chains of
/// bubbles, satisfy EVERY ordinary IBP row of their family, none of which is the
/// row of the step but one: signs, weights and leaves are all in this sum. Seeds: all
/// distinct keys with at most 2 dots in total, the eps line of d30 at every power in
/// `-3..=3` (105 seeds, 21 on the kite), each at the same 5 distinct random `d`. It does
/// not replace the end-to-end values of M2-T8a and M2-V5.
#[test]
fn step_values_satisfy_every_ibp_row_of_the_family() {
    for (name, seed, seeds) in [("d30", 0x130_u64, 105), ("d43", 0x143, 21)] {
        let node = named(name);
        let p = &*node.presentation;
        let step = step_of(node);
        let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
        let indexed = generator.context();
        let rows = relations(p);
        assert_eq!(rows.len(), 6, "(d/dk_l).(k_1, k_2, q)");
        let dotted = dotted_keys(p, 2);
        let seed_keys = match node
            .roles
            .a
            .iter()
            .find(|&&a| eps_units(p, usize::from(a)) != 0)
        {
            Some(&eps_slot) => with_eps_powers(&dotted, usize::from(eps_slot), -3..=3),
            None => dotted,
        };
        assert_eq!(seed_keys.len(), seeds, "{name}");
        let mut closures = HashMap::new();
        let mut rng = StdRng::seed_from_u64(seed);
        let mut nonzero = 0;
        for d in dimensions(&mut rng, 5) {
            for seed_key in &seed_keys {
                for row in &rows {
                    let mut sum = Vec::new();
                    for (shift, coefficient) in row.terms() {
                        let (value, _) = indexed
                            .specialize(coefficient, seed_key, IndexedAlgebraLimits::default())
                            .unwrap();
                        let key: Vec<i64> = seed_key
                            .iter()
                            .zip(shift.values())
                            .map(|(a, b)| a + b)
                            .collect();
                        let weight = at(&value, &d);
                        let values = eps_family_value(node, &step, &mut closures, &key, &d);
                        nonzero += usize::from(!collect(values.iter().cloned()).is_empty());
                        sum.extend(values.iter().map(|term| term.scale(&weight)));
                    }
                    assert!(
                        collect(sum).is_empty(),
                        "{name} row {} at {seed_key:?}, d = {d}",
                        row.row_id().stable_string()
                    );
                }
            }
        }
        assert!(nonzero > 5 * seeds, "{name}: {nonzero}");
    }
}

// ---------------------------------------------------------------------------
// Fail closed
// ---------------------------------------------------------------------------

/// The forms a template read from its row.
fn forms_of(template: &TriangleTemplate) -> RowForms {
    let negated = |form: &RowForm| RowForm {
        constant: neg(&form.constant),
        slopes: form
            .slopes
            .iter()
            .map(|&(slot, slope)| (slot, -slope))
            .collect(),
    };
    let [x1, x2] = template.apex();
    RowForms {
        pivot: template.pivot().clone(),
        lower_b: [negated(x1), negated(x2)],
        lower_c: [x1.clone(), x2.clone()],
    }
}

fn assert_mismatch(result: Result<TriangleTemplate, TemplateError>, reason: &str) {
    match result {
        Err(TemplateError::Mismatch(TemplateMismatch { reason: found, .. })) => {
            assert!(found.contains(reason), "{found}");
        }
        Err(TemplateError::Failure(error)) => panic!("a failure, not a mismatch: {error}"),
        Ok(_) => panic!("no mismatch, wanted: {reason}"),
    }
}

/// Every entry and every kind of change: the shape check refuses, and the original
/// forms pass.
#[test]
fn the_shape_check_refuses_every_changed_entry() {
    for name in ["d30", "d118"] {
        let node = named(name);
        let p = &*node.presentation;
        let roles = &node.roles;
        let valid = forms_of(&TriangleTemplate::extract(p, roles).unwrap());
        let extracted = TriangleTemplate::from_forms(p, roles, valid.clone()).unwrap();
        assert_eq!(*extracted.pivot(), valid.pivot);

        // The rational part, the eps part, a slope, an extra slope.
        let changes: [fn(&mut RowForm); 4] = [
            |form| form.constant = add(&form.constant, &int(1)),
            |form| form.constant = add(&form.constant, &eps(1)),
            |form| form.slopes[0].1 += 1,
            |form| {
                let free = (0..64).find(|&slot| form.slopes.iter().all(|s| s.0 != slot));
                form.slopes.push((free.unwrap(), 1));
                form.slopes.sort_unstable();
            },
        ];
        for change in changes {
            let mut forms = valid.clone();
            change(&mut forms.pivot);
            let result = TriangleTemplate::from_forms(p, roles, forms);
            assert_mismatch(result, "the zero-shift entry");
            for i in 0..2 {
                let mut forms = valid.clone();
                change(&mut forms.lower_b[i]);
                let result = TriangleTemplate::from_forms(p, roles, forms);
                assert_mismatch(result, "A+ B-");
                let mut forms = valid.clone();
                change(&mut forms.lower_c[i]);
                let result = TriangleTemplate::from_forms(p, roles, forms);
                assert_mismatch(result, "A+ C-");
            }
        }
        // The entries of the two sides swapped, an overall sign.
        let mut forms = valid.clone();
        std::mem::swap(&mut forms.lower_b, &mut forms.lower_c);
        assert_mismatch(TriangleTemplate::from_forms(p, roles, forms), "A+ B-");
    }
}

/// Roles that do not fit the presentation are a mismatch, not a failure of the run;
/// `b`, `c1` and `c2` must be integer lines.
#[test]
fn roles_that_break_the_contract_are_a_mismatch() {
    let node = named("d43");
    let p = &*node.presentation;
    let swapped = TriangleRoles {
        c: [node.roles.c[1], node.roles.c[0]],
        ..node.roles
    };
    assert_mismatch(TriangleTemplate::extract(p, &swapped), "triangle contract");
    let isp_role = TriangleRoles {
        b: 99,
        ..node.roles
    };
    assert_mismatch(TriangleTemplate::extract(p, &isp_role), "triangle contract");

    let with_units = |slot: u16| {
        let slots = p
            .slots
            .iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Slot::Line(line) => Slot::Line(SlotLine {
                    momentum: line.momentum.clone(),
                    eps_units: u8::from(index == usize::from(slot)),
                }),
                Slot::Isp(isp) => Slot::Isp(isp.clone()),
            })
            .collect();
        Presentation::from_slots(
            p.id,
            PresentationKind::TriangleAdapted,
            "with_units",
            p.loops,
            slots,
            context(),
            None,
        )
        .unwrap()
    };
    let roles = &node.roles;
    let on_b = with_units(roles.b);
    assert_mismatch(TriangleTemplate::extract(&on_b, roles), "b carries eps");
    let on_c = with_units(roles.c[0]);
    assert_mismatch(TriangleTemplate::extract(&on_c, roles), "carries eps");
    for apex in roles.a {
        let eps_apex = with_units(apex);
        let template = TriangleTemplate::extract(&eps_apex, roles).unwrap();
        assert_eq!(template.presentation(), p.id);
    }

    // A mismatch becomes the error the lane is at fault for, a failure stays itself.
    let mismatch = TemplateError::Mismatch(TemplateMismatch {
        presentation: p.id,
        reason: "because".to_owned(),
    });
    assert!(
        matches!(PError::from(mismatch), PError::Contract(reason) if reason.contains("because"))
    );
    let failure = TemplateError::Failure(PError::ContextMismatch);
    assert_eq!(PError::from(failure), PError::ContextMismatch);
}

/// A hand-built triangle with `b = k^2`, `a = (k + l)^2, (k + r)^2`, `c = l^2, r^2`,
/// two more lines and the two ISPs left: `N` counts the `k`-linear ISP alone, of either
/// sign and on any `Y` free of `k`, and an ISP quadratic in `k` breaks the contract.
#[test]
fn only_k_linear_isps_enter_the_pivot() {
    let line = |momentum: [i64; 4]| {
        Slot::Line(SlotLine {
            momentum: momentum.to_vec(),
            eps_units: 0,
        })
    };
    let isp = |left: [i64; 4], right: [i64; 4]| {
        Slot::Isp(SlotIsp {
            left: left.to_vec(),
            right: right.to_vec(),
        })
    };
    let lines = [
        line([1, 0, 0, 0]),
        line([1, 1, 0, 0]),
        line([1, 0, 1, 0]),
        line([0, 1, 0, 0]),
        line([0, 0, 1, 0]),
        line([0, 1, 0, -1]),
        line([0, 0, 1, -1]),
    ];
    let roles = TriangleRoles {
        b: 0,
        a: [1, 2],
        c: [3, 4],
        k_loop: 0,
    };
    let with_isp = |linear: Slot| {
        let free = isp([0, 1, 0, 0], [0, 0, 1, 0]);
        let slots = lines.iter().cloned().chain([linear, free]).collect();
        Presentation::from_slots(
            PresentationId(0),
            PresentationKind::TriangleAdapted,
            "hand_triangle",
            3,
            slots,
            context(),
            None,
        )
        .unwrap()
    };
    for linear in [
        isp([1, 0, 0, 0], [0, 0, 0, 1]),
        isp([-1, 0, 0, 0], [0, 0, 0, 1]),
        isp([0, 0, 0, 1], [1, 0, 0, 0]),
        isp([1, 0, 0, 0], [0, 1, 0, 1]),
    ] {
        let p = with_isp(linear);
        let template = TriangleTemplate::extract(&p, &roles).unwrap();
        assert_eq!(
            template.pivot().slopes,
            vec![(0, -2), (1, -1), (2, -1), (7, -1)]
        );
    }
    let quadratic = with_isp(isp([1, 0, 0, 1], [1, 0, 0, 0]));
    assert_mismatch(
        TriangleTemplate::extract(&quadratic, &roles),
        "is not linear in loop 0",
    );
}

/// Every other IBP row of the family is refused: only `(d/dk).k` has the shape.
#[test]
fn only_the_row_of_k_on_k_has_the_shape() {
    for name in ["d43", "d30", "d118"] {
        let node = named(name);
        let p = &*node.presentation;
        let generator = ParametricIbpGenerator::try_new(&p.family).unwrap();
        let wanted = RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 0,
        };
        let rows = relations(p);
        assert!(rows.len() >= 6);
        for row in &rows {
            let result = TriangleTemplate::from_relation(p, &node.roles, &generator, row);
            if *row.row_id() == wanted {
                assert!(result.is_ok(), "{name}");
            } else {
                assert_mismatch(result, "the row has");
            }
        }
    }
    // A row of another family is no mismatch: the run is broken, not the lane.
    let (d43, d30) = (named("d43"), named("d30"));
    let generator = ParametricIbpGenerator::try_new(&d43.presentation.family).unwrap();
    let foreign = &relations(&d30.presentation)[0];
    let result =
        TriangleTemplate::from_relation(&d43.presentation, &d43.roles, &generator, foreign);
    assert!(
        matches!(result, Err(TemplateError::Failure(_))),
        "{result:?}"
    );
}

/// Apply keys only: the step refuses what the adapter must have routed, and a line
/// with eps units is fine at any power.
#[test]
fn the_step_refuses_what_is_no_apply_key() {
    let kite = named("d43");
    let step = step_of(kite);
    let tag = tag_one();
    let p = &*kite.presentation;
    let refused = |powers: &[i64]| step.apply(&key_view(p, powers, &tag), &limits());
    for slot in 0..5 {
        for power in [0, -2] {
            let mut powers = vec![1; 5];
            powers[slot] = power;
            assert!(
                matches!(refused(&powers), Err(PError::Contract(_))),
                "{powers:?}"
            );
        }
    }
    assert!(matches!(refused(&[1, 1, 1, 1]), Err(PError::Contract(_))));
    let foreign = PKeyView {
        presentation: PresentationId(p.id.0 + 1),
        powers: &[1, 1, 1, 1, 1],
        tag: &tag,
    };
    assert!(matches!(
        step.apply(&foreign, &limits()),
        Err(PError::Contract(_))
    ));

    // The row itself, on keys no step lets through.
    let template = step.template();
    let short = template.solve(&[1, 1, 1, 1], exact_limits());
    assert!(matches!(short, Err(PError::Contract(_))));
    let huge = template.solve(&[i64::MAX; 5], exact_limits());
    assert!(matches!(huge, Err(PError::Unsupported(_))));

    let with_isp = named("d118");
    let step = step_of(with_isp);
    let p = &*with_isp.presentation;
    let mut powers = vec![1, 1, 1, 1, 1, 1, 1, 1, 0];
    let view = key_view(p, &powers, &tag);
    assert!(matches!(
        step.apply(&view, &limits()),
        Err(PError::Unsupported(_))
    ));
    powers[7] = 0;
    powers[8] = -2;
    assert_eq!(
        step.apply(&key_view(p, &powers, &tag), &limits())
            .unwrap()
            .children
            .len(),
        4
    );

    let eps = named("d30");
    let step = step_of(eps);
    let p = &*eps.presentation;
    for power in [-3, 0, 1, 2] {
        let mut powers = vec![1; 5];
        powers[usize::from(eps.roles.a[1])] = power;
        let children = checked_step(eps, &step, &powers);
        assert!(children.iter().all(|(_, weight, _)| !weight.is_zero()));
        let other = step.apply(&key_view(p, &powers, &tag), &limits()).unwrap();
        assert_eq!(other.children.len(), 4);
    }
}

/// The owner path: the program of a Triangle owner in a real library, and owners it
/// must refuse.
#[test]
fn the_step_runs_as_the_program_of_an_owner() {
    let dag = Arc::new(production_dag(2));
    let (_, top_presentations) = tops();
    let node = dag
        .nodes
        .iter()
        .find(|node| decide(node).unwrap().lane == OwnerLane::Triangle)
        .unwrap();
    let built = present_node(node, top_presentations, PresentationId(0), context()).unwrap();
    let LaneRoles::Triangle(roles) = built.decision.roles else {
        panic!("{:?}", built.decision);
    };
    let p = built.presentation.unwrap();
    let sector = classify(&p, 0b1_1111, &ClassLimits::default())
        .unwrap()
        .unwrap();
    let class: OwnerClassId = sector.key.id();
    let record = PClassRecord {
        key: sector.key.clone(),
        owner: (p.id, sector.support),
        node: None,
        nodes: Vec::new(),
        t: 5,
        eps_lines: 0,
    };
    let census = PCensus {
        sectors: BTreeMap::new(),
        classes: BTreeMap::from([(class, record)]),
    };
    let spec = POwnerSpec {
        class,
        lane: OwnerLane::Triangle,
        presentation: p.id,
        node: None,
        roles: LaneRoles::Triangle(roles),
        edge_maps: Vec::new(),
        payload: None,
        custom: None,
        terminals: Vec::new(),
    };
    let mut builder = PLibrary::builder(context().clone(), 2..=2);
    builder.set_topology(dag.clone()).unwrap();
    let id = builder.add_presentation(p).unwrap();
    builder.set_census(census).unwrap();
    builder.add_owner(spec.clone()).unwrap();
    let library = builder.build().unwrap();

    let step = TriangleStep::from_owner(&library, &spec).unwrap();
    assert_eq!(step.lane(), OwnerLane::Triangle);
    let owner = library.owner(class).unwrap();
    let cx_limits = limits();
    let cx = PStepContext {
        library: &library,
        owner,
        limits: &cx_limits,
    };
    let tag = tag_one();
    let powers = [2, 1, 1, 1, 2];
    let key = PKeyView {
        presentation: id,
        powers: &powers,
        tag: &tag,
    };
    assert_eq!(
        step.step(&cx, &key).unwrap(),
        step.apply(&key, &cx_limits).unwrap()
    );

    let insertion = POwnerSpec {
        lane: OwnerLane::Insertion,
        ..spec.clone()
    };
    assert!(matches!(
        TriangleStep::from_owner(&library, &insertion),
        Err(PError::Contract(_))
    ));
    let no_roles = POwnerSpec {
        roles: LaneRoles::None,
        ..spec.clone()
    };
    assert!(matches!(
        TriangleStep::from_owner(&library, &no_roles),
        Err(PError::Contract(_))
    ));
    let edges = POwnerSpec {
        edge_maps: vec![0],
        ..spec.clone()
    };
    assert!(matches!(
        TriangleStep::from_owner(&library, &edges),
        Err(PError::Contract(_))
    ));
    let wrong_roles = POwnerSpec {
        roles: LaneRoles::Triangle(TriangleRoles {
            c: [roles.c[1], roles.c[0]],
            ..roles
        }),
        ..spec
    };
    let error = TriangleStep::from_owner(&library, &wrong_roles).unwrap_err();
    assert!(matches!(error, PError::Contract(reason) if reason.contains("triangle template")));
}

// ---------------------------------------------------------------------------
// Versions and cost
// ---------------------------------------------------------------------------

/// The forms and the step of the named nodes at fixed keys and one rational `d`.
fn template_digest() -> String {
    let d = rational(37, 10);
    let mut text = String::new();
    for shape in &EXACT_SHAPES {
        let node = named(shape.name);
        let step = step_of(node);
        let template = step.template();
        let p = &*node.presentation;
        let form = |form: &RowForm| format!("{} {:?}", at(&form.constant, &d), form.slopes);
        text += &format!(
            "{} {:?} {} {} {}\n",
            shape.name,
            template.roles(),
            form(template.pivot()),
            form(&template.apex()[0]),
            form(&template.apex()[1])
        );
        let mut powers = vec![0; p.slots.len()];
        for (slot, power) in powers.iter_mut().enumerate().take(line_count(p)) {
            *power = 1 + i64::try_from(slot % 2).unwrap();
        }
        if let Some(last) = powers.get_mut(line_count(p)) {
            *last = -1;
        }
        let tag = tag_one();
        let output = step.apply(&key_view(p, &powers, &tag), &limits()).unwrap();
        for child in &output.children {
            text += &format!(
                "{:?} {:?} {}\n",
                child.stage,
                child.powers,
                at(&child.coefficient, &d)
            );
        }
    }
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// A change of the meaning of a recorded step needs a bump of
/// `P_TRIANGLE_TEMPLATE_VERSION` with a new digest here.
const TEMPLATE_PIN: (u32, &str) = (
    1,
    "f7192928ace7a38cd5235d810df1be07340833d834752348a6443d9b560413c8",
);

#[test]
fn the_template_content_is_pinned_to_its_version() {
    assert_eq!(
        (P_TRIANGLE_TEMPLATE_VERSION, template_digest().as_str()),
        TEMPLATE_PIN,
        "bump P_TRIANGLE_TEMPLATE_VERSION with every new digest"
    );
}

/// [M] numbers for the notes, optimised builds only: extraction per node of the 4L
/// DAG, and one step on the largest key.
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "timing: only meaningful in an optimised build"
)]
fn the_step_is_cheap() {
    let nodes = triangle_nodes(4);
    let start = Instant::now();
    let steps: Vec<TriangleStep> = nodes.iter().map(step_of).collect();
    let extraction = start.elapsed() / u32::try_from(steps.len()).unwrap();
    let mut rng = StdRng::seed_from_u64(0x71);
    let tag = tag_one();
    let keys: Vec<Vec<i64>> = nodes
        .iter()
        .map(|node| random_key(&node.presentation, &mut rng, 3, 4, (-2, 2)))
        .collect();
    let rounds = 200_u32;
    let start = Instant::now();
    for _ in 0..rounds {
        for ((node, step), key) in nodes.iter().zip(&steps).zip(&keys) {
            let view = key_view(&node.presentation, key, &tag);
            step.apply(&view, &limits()).unwrap();
        }
    }
    let per_step = start.elapsed() / (rounds * u32::try_from(steps.len()).unwrap());
    println!("extraction per node {extraction:?}, one step {per_step:?}");
    assert!(per_step.as_millis() < 5, "{per_step:?}");
}

/// Shape on all 5L triangle nodes of Forcer's top diagrams. Not an M2 gate: it keeps
/// the template honest about what comes next. Arity 20, eps apex up to `j1 + j2 > 1`.
#[test]
#[ignore = "slow: the 5L DAG and its Triangle presentations"]
fn every_triangle_node_of_the_5l_dag_has_the_shape() {
    let roots: Vec<PTopo> = (1..=5)
        .flat_map(forcer::forcer_tops)
        .map(|(_, topo)| topo)
        .collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let nodes = triangle_nodes_of(&dag, |key| key.to_hex());
    let mut units = BTreeMap::<i64, usize>::new();
    for node in &nodes {
        let p = &*node.presentation;
        TriangleTemplate::extract(p, &node.roles)
            .unwrap_or_else(|error| panic!("{}: {error}", node.name));
        let total: i64 = node
            .roles
            .a
            .iter()
            .map(|&slot| i64::from(eps_units(p, usize::from(slot))))
            .sum();
        *units.entry(total).or_default() += 1;
    }
    println!(
        "{} nodes of the 5L DAG, by eps units on the apex: {units:?}",
        nodes.len()
    );
    assert!(nodes.len() > 53);
}
