//! The Diamond template step (owner: M3-T5b rr-diamond): the summed row read from
//! RustRed's own IBP generator on every diamond node of the 4L DAG, the step's
//! children and order, and the refusals. The template against the SpIRed owners of
//! the four nodes, through two scope 1-4 libraries, is in `rustred-app`
//! (`tests/p_library_app/diamond.rs`), where `assemble_library` lives.
use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::propagator::decide::decide;
use rustred::propagator::diamond::{DiamondError, DiamondRowForms, DiamondStep, DiamondTemplate};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile, load_p_families};
use rustred::propagator::lane::{DiamondRoles, LaneRoles, OwnerLane};
use rustred::propagator::present::{present_node, top_topology};
use rustred::propagator::presentation::{Presentation, Slot};
use rustred::propagator::step::{PKeyView, PStage};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::{ChildMode, GraphKey, NodeInfo, PTopo, reduction_graph};
use rustred::propagator::triangle::RowForm;
use rustred::propagator::{P_DIAMOND_TEMPLATE_VERSION, PresentationId};
use rustred::reduction::ReductionLimits;
use symbolica::domains::rational::{Q, Rational};

use crate::support::p_integrals::rational;

#[path = "../../../rustred-topology/tests/support/forcer.rs"]
mod forcer;

fn tops() -> &'static (PFamiliesFile, Vec<Arc<Presentation>>) {
    static TOPS: OnceLock<(PFamiliesFile, Vec<Arc<Presentation>>)> = OnceLock::new();
    TOPS.get_or_init(|| load_p_families(P_FAMILIES_V1_JSON).unwrap())
}

fn context() -> &'static CoefficientContext {
    tops().1[0].family.coefficient_context()
}

fn forcer_names() -> &'static BTreeMap<GraphKey, String> {
    static NAMES: OnceLock<BTreeMap<GraphKey, String>> = OnceLock::new();
    NAMES.get_or_init(|| forcer::forcer_names_by_key(&forcer::forcer_dag()))
}

/// A node of the Diamond lane with its presentation and roles.
struct DiamondNode {
    name: String,
    node: NodeInfo,
    presentation: Arc<Presentation>,
    roles: DiamondRoles,
}

/// The Diamond nodes of the DAG of vakint's `loops`-loop tops, Forcer's names.
fn diamond_nodes(loops: u8) -> Vec<DiamondNode> {
    let (file, top_presentations) = tops();
    let roots: Vec<PTopo> = (file.families.iter())
        .filter(|entry| entry.loops == loops)
        .map(top_topology)
        .collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let first_id = u32::try_from(top_presentations.len()).unwrap();
    let diamonds =
        (dag.nodes.iter()).filter(|node| decide(node).unwrap().lane == OwnerLane::Diamond);
    diamonds
        .map(|node| {
            let name = forcer_names()[&node.key].clone();
            let id = PresentationId(first_id + node.index);
            let built = present_node(node, top_presentations, id, context()).unwrap();
            assert_eq!(built.fallback, None, "{name}");
            let LaneRoles::Diamond(roles) = built.decision.roles else {
                panic!("{name}: {:?}", built.decision);
            };
            DiamondNode {
                name,
                node: node.clone(),
                presentation: Arc::new(built.presentation.unwrap()),
                roles,
            }
        })
        .collect()
}

fn four_loop_diamonds() -> &'static [DiamondNode] {
    static NODES: OnceLock<Vec<DiamondNode>> = OnceLock::new();
    NODES.get_or_init(|| diamond_nodes(4))
}

fn named(name: &str) -> &'static DiamondNode {
    (four_loop_diamonds().iter())
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("no Diamond node {name}"))
}

fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

/// The slope in `d` of a coefficient affine in `d`, from its values at 0 and 2.
fn d_slope(value: &Coefficient) -> Rational {
    (&at(value, &rational(2, 1)) - &at(value, &rational(0, 1))) / rational(2, 1)
}

fn eps_units(p: &Presentation, slot: u16) -> u8 {
    match &p.slots[usize::from(slot)] {
        Slot::Line(line) => line.eps_units,
        Slot::Isp(_) => 0,
    }
}

/// The diamond nodes of the 4L DAG are Forcer's four, each with the template:
/// the extraction is the shape check, so success is the claim. The `d` slope of
/// the pivot is `L + S + Sum j_a / 2 + j_s > 0` (architecture section 3.5).
#[test]
fn every_diamond_node_of_the_4l_dag_has_the_shape() {
    for loops in 1..=3 {
        assert!(
            diamond_nodes(loops).is_empty(),
            "a diamond at {loops} loops"
        );
    }
    let mut names: Vec<&str> = (four_loop_diamonds().iter())
        .map(|node| node.name.as_str())
        .collect();
    names.sort_unstable();
    assert_eq!(names, ["d384", "d394", "d399", "no3"]);
    for node in four_loop_diamonds() {
        let p = &*node.presentation;
        let template = DiamondTemplate::extract(p, &node.roles)
            .unwrap_or_else(|error| panic!("{}: {error}", node.name));
        let roles = &node.roles;
        let apex_units: i64 = (roles.a().iter())
            .map(|&slot| i64::from(eps_units(p, slot)))
            .sum();
        let spectator_units = roles
            .spectator
            .map_or(0, |slot| i64::from(eps_units(p, slot)));
        let loops = i64::try_from(roles.loops()).unwrap();
        let slope = d_slope(&template.pivot().constant);
        assert_eq!(
            slope,
            rational(2 * loops + apex_units + 2 * spectator_units, 2),
            "{}",
            node.name
        );
        assert!(slope > rational(0, 1), "{}", node.name);
        assert_eq!(template.apex().len(), roles.paths(), "{}", node.name);
        println!(
            "{}: node {}, {} lines, {} paths, spectator {:?}, L + S = {}, pivot {:?}",
            node.name,
            node.node.index,
            node.node.t,
            roles.paths(),
            roles.spectator,
            roles.loops(),
            template.pivot().slopes
        );
    }
}

/// A random Apply key: integer lines at `1 + dots`, `rank` over the ISPs.
fn random_key(p: &Presentation, rng: &mut StdRng, dots: i64, rank: i64) -> Vec<i64> {
    let lines = p
        .slots
        .iter()
        .filter(|s| matches!(s, Slot::Line(_)))
        .count();
    let mut powers: Vec<i64> = (0..p.slots.len())
        .map(|slot| i64::from(slot < lines))
        .collect();
    for _ in 0..dots {
        powers[rng.random_range(0..lines)] += 1;
    }
    if p.slots.len() > lines {
        for _ in 0..rank {
            powers[rng.random_range(lines..p.slots.len())] -= 1;
        }
    }
    powers
}

/// One step on random Apply keys with dots and numerators: `2 (L + 1)` children on
/// the presentation with the parent's tag, no terminal, no condition, nonzero
/// weights; each lowers `Sum (n_b + n_c)` by one and is a Route key exactly when
/// it lowers a line to 0. An Apply child is strictly below its parent in the
/// step's order.
#[test]
fn a_diamond_step_lowers_one_lower_or_outer_line() {
    let mut rng = StdRng::seed_from_u64(0xd1a);
    let limits = ReductionLimits::default();
    let tag = GammaTag::from_pairs([(1, 3), (-2, -1)]).unwrap();
    for node in four_loop_diamonds() {
        let p = &node.presentation;
        let step = DiamondStep::new(Arc::clone(p), node.roles).unwrap();
        let (mut routes, mut applies) = (0, 0);
        for _ in 0..12 {
            let (dots, rank) = (rng.random_range(0..=3), rng.random_range(0..=3));
            let powers = random_key(p, &mut rng, dots, rank);
            let key = PKeyView {
                presentation: p.id,
                powers: &powers,
                tag: &tag,
            };
            let output = step.apply(&key, &limits).unwrap();
            assert!(output.terminals.is_empty() && output.conditions.is_empty());
            assert_eq!(
                output.children.len(),
                2 * node.roles.paths(),
                "{}",
                node.name
            );
            let measure = step.measure(&powers).unwrap();
            for child in &output.children {
                assert_eq!((child.presentation, &child.tag), (p.id, &tag));
                assert!(!child.coefficient.is_zero(), "{} {powers:?}", node.name);
                assert_eq!(step.measure(&child.powers).unwrap(), measure - 1);
                let pinched = (node.roles.b().iter().chain(node.roles.c()))
                    .any(|&slot| child.powers[usize::from(slot)] == 0);
                let expected = if pinched {
                    PStage::Route
                } else {
                    PStage::Apply
                };
                assert_eq!(child.stage, expected, "{} {:?}", node.name, child.powers);
                routes += usize::from(pinched);
                applies += usize::from(!pinched);
            }
        }
        assert!(
            routes > 0 && applies > 0,
            "{}: {routes} Route, {applies} Apply",
            node.name
        );
    }
}

/// Fails closed: the roles of a diamond on a presentation that is not that
/// diamond (a 4L triangle node, and d384 with `b` and `c` of a path swapped)
/// are a mismatch, never a template; a summed row of another pivot is
/// one too; the step refuses a key that is not an Apply key of its owner.
#[test]
fn the_template_refuses_what_is_not_a_diamond() {
    let mismatch = |result: Result<DiamondTemplate, DiamondError>, what: &str| match result {
        Err(DiamondError::Mismatch { reason, .. }) => println!("{what}: {reason}"),
        other => panic!("{what}: {other:?}"),
    };
    let d384 = named("d384");
    let roles = &d384.roles;
    let swapped = {
        let (mut b, mut c) = (roles.b().to_vec(), roles.c().to_vec());
        std::mem::swap(&mut b[0], &mut c[0]);
        DiamondRoles::new(roles.a(), &b, &c, roles.spectator).unwrap()
    };
    mismatch(
        DiamondTemplate::extract(&d384.presentation, &swapped),
        "d384 with b0 and c0 swapped",
    );
    // A Triangle node of the 4L DAG with as many lines as d384, read with d384's roles.
    let (file, top_presentations) = tops();
    let first_id = u32::try_from(top_presentations.len()).unwrap();
    let roots: Vec<PTopo> = (file.families.iter())
        .filter(|entry| entry.loops == 4)
        .map(top_topology)
        .collect();
    let dag = reduction_graph(&roots, ChildMode::ForcerTree);
    let triangle = (dag.nodes.iter())
        .find(|node| node.t == d384.node.t && decide(node).unwrap().lane == OwnerLane::Triangle)
        .unwrap();
    let id = PresentationId(first_id + triangle.index);
    let built = present_node(triangle, top_presentations, id, context()).unwrap();
    let other = built.presentation.unwrap();
    mismatch(
        DiamondTemplate::extract(&other, roles),
        "a 4L triangle node with the roles of d384",
    );

    // The true row of d384 with the pivot's d slope off by one.
    let template = DiamondTemplate::extract(&d384.presentation, roles).unwrap();
    let context = d384.presentation.family.coefficient_context();
    let d = context.parameter("d").unwrap();
    let shifted = context
        .try_add(&template.pivot().constant, &d, Default::default())
        .unwrap();
    let negate = |form: &RowForm| RowForm {
        constant: context.try_neg(&form.constant, Default::default()).unwrap(),
        slopes: form
            .slopes
            .iter()
            .map(|&(slot, slope)| (slot, -slope))
            .collect(),
    };
    let forms = DiamondRowForms {
        pivot: RowForm {
            constant: shifted,
            slopes: template.pivot().slopes.clone(),
        },
        lower_b: template.apex().iter().map(negate).collect(),
        lower_c: template.apex().to_vec(),
    };
    mismatch(
        DiamondTemplate::from_forms(&d384.presentation, roles, forms),
        "a pivot of slope L + S + 1",
    );

    // A line of the diamond at power 0 is a Route key, never stepped.
    let step = DiamondStep::new(Arc::clone(&d384.presentation), *roles).unwrap();
    let mut powers = random_key(&d384.presentation, &mut StdRng::seed_from_u64(1), 0, 0);
    powers[usize::from(roles.b()[0])] = 0;
    let tag = GammaTag::one();
    let key = PKeyView {
        presentation: d384.presentation.id,
        powers: &powers,
        tag: &tag,
    };
    assert!(matches!(
        step.apply(&key, &ReductionLimits::default()),
        Err(PError::Contract(_))
    ));
    assert!(matches!(
        PError::from(DiamondError::Mismatch {
            presentation: d384.presentation.id,
            reason: "x".to_owned()
        }),
        PError::Contract(_)
    ));
}

/// The content of the lane on the four nodes: roles, the pivot and apex forms at
/// one `d`, and the children of one step on a dotted key with a numerator.
fn template_digest() -> String {
    let d = rational(37, 10);
    let form = |form: &RowForm| format!("{} {:?}", at(&form.constant, &d), form.slopes);
    let mut text = String::new();
    let mut nodes: Vec<&DiamondNode> = four_loop_diamonds().iter().collect();
    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    for node in nodes {
        let p = &node.presentation;
        let step = DiamondStep::new(Arc::clone(p), node.roles).unwrap();
        let template = step.template();
        text += &format!(
            "{} {:?} {} {:?}\n",
            node.name,
            template.roles(),
            form(template.pivot()),
            template.apex().iter().map(form).collect::<Vec<_>>()
        );
        let lines = (p.slots.iter())
            .filter(|s| matches!(s, Slot::Line(_)))
            .count();
        let mut powers: Vec<i64> = (0..p.slots.len())
            .map(|slot| i64::from(slot < lines) * (1 + i64::try_from(slot % 2).unwrap()))
            .collect();
        if let Some(isp) = powers.get_mut(lines) {
            *isp = -1;
        }
        let tag = GammaTag::one();
        let key = PKeyView {
            presentation: p.id,
            powers: &powers,
            tag: &tag,
        };
        let output = step.apply(&key, &ReductionLimits::default()).unwrap();
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
/// `P_DIAMOND_TEMPLATE_VERSION` with a new digest here.
const TEMPLATE_PIN: (u32, &str) = (
    1,
    "2da6645c22eb72f3439b71ed151a759321fba07cfc9b0446a94f86f4e568d69e",
);

#[test]
fn the_template_content_is_pinned_to_its_version() {
    assert_eq!(
        (P_DIAMOND_TEMPLATE_VERSION, template_digest().as_str()),
        TEMPLATE_PIN,
        "bump P_DIAMOND_TEMPLATE_VERSION with every new digest"
    );
}
