//! Forcer's 16 masters as the four-loop basis (owner: M3-T17 rr-basis4).
//!
//! The definitions of `P_BASIS_MASTERS_1_TO_4` (topology, sign, source) and
//! their choice by scope, t155's two eps units, the copy of M3-V7's four-loop
//! Forcer anchors read through the shared anchor code, and the bound of the
//! Gamma tags. The 4L layer, its fresh-`d` checks and the exactness of the
//! anchors need the assembled scope 1-4 library of M3-T8c.
//!
//! Anchors: `basis_anchors/p4l_anchors.json`, a copy of vakint's four-loop
//! Forcer fixture (provenance in `basis_anchors/README.md`).
use std::collections::BTreeMap;

use rustred::family::IntegralKey;
use rustred::propagator::P_GAMMA_TAG_MAX_M;
use rustred::propagator::basis::anchors::{ForcerCase, parse_fixture};
use rustred::propagator::basis::{
    FORCER_MASTER_NAMES, P_BASIS_MASTERS_1_TO_3, P_BASIS_MASTERS_1_TO_4, PBasisOptions,
    PMasterDefinition, largest_gamma_m, p_basis_masters,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, load_p_families};
use rustred::propagator::kernel::bubble::relative_tag;
use rustred::propagator::lane::{BubbleRoles, LaneRoles};
use rustred::propagator::library::{PBasis, PLibrary};
use rustred::propagator::presentation::PresentationKind;
use rustred::propagator::tag::GammaTag;
use rustred::propagator::testing::one_loop_reference_library;
use rustred::propagator::topology::GraphKey;
use serde_json::Value;

use super::basis::{GRAPH_KEYS_TSV, coefficient, context, exact, master, scheme, term};

const ANCHORS_4L_JSON: &str = include_str!("basis_anchors/p4l_anchors.json");

/// blake3 of `basis_anchors/p4l_anchors.json` (sha256 607013f2... in its README).
const ANCHORS_4L_BLAKE3: &str = "f1ea381e05a69d18764ce979b2c70f609a8f07a3f6bf92d32c716fdab1621fc3";

/// `(name, Forcer topology, loops, lines, eps units of its eps lines)`.
const MASTERS: [(&str, &str, u8, u8, &[u8]); 16] = [
    ("bebe", "bebe", 4, 8, &[]),
    ("cross", "cross", 4, 8, &[]),
    ("fastar2", "fastar2", 3, 7, &[1]),
    ("haha", "haha", 4, 11, &[]),
    ("lala", "lala", 4, 9, &[]),
    ("no", "nostar0", 3, 8, &[]),
    ("no1", "no1", 4, 11, &[]),
    ("no2", "no2", 4, 11, &[]),
    ("no6", "no6", 4, 10, &[]),
    ("nono", "nono", 4, 9, &[]),
    ("nostar5", "nostar5", 3, 8, &[1]),
    ("nostar6", "nostar6", 3, 8, &[1]),
    ("t105", "t1star05", 2, 5, &[1]),
    ("t124", "t1star24", 2, 5, &[1, 1]),
    ("t145", "t1star45", 2, 5, &[1, 1]),
    ("t155", "t1star55", 2, 5, &[2]),
];

fn definition(name: &str) -> &'static PMasterDefinition {
    P_BASIS_MASTERS_1_TO_4
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no definition of {name}"))
}

/// The eps units of the master's eps lines, sorted.
fn eps_units(master: &PMasterDefinition) -> Vec<u8> {
    let mut units: Vec<u8> = (master.graph().unwrap().lines.iter())
        .map(|line| line.2)
        .filter(|&units| units > 0)
        .collect();
    units.sort_unstable();
    units
}

#[test]
fn the_sixteen_masters_are_forcer_topologies_with_pinned_signs() {
    let golden: BTreeMap<&str, &str> = (GRAPH_KEYS_TSV.lines())
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split_once('\t').unwrap())
        .collect();
    let names: Vec<&str> = P_BASIS_MASTERS_1_TO_4.iter().map(|m| m.name).collect();
    assert_eq!(names, FORCER_MASTER_NAMES);
    for old in &P_BASIS_MASTERS_1_TO_3 {
        assert_eq!(definition(old.name), old, "the 1L-3L entries are unchanged");
    }
    let c = context();
    for (m, (name, topology, loops, lines, units)) in P_BASIS_MASTERS_1_TO_4.iter().zip(MASTERS) {
        assert_eq!((m.name, m.topology), (name, topology));
        let graph = m.graph().unwrap();
        assert_eq!(
            (graph.loops(), graph.line_count()),
            (loops, lines),
            "{name}"
        );
        assert_eq!(eps_units(m), units, "{name}");
        // Every master is a four-loop integral (three-loop: no, t105) once
        // each eps unit is a one-loop insertion again.
        let insertions: u8 = units.iter().sum();
        let expected = if ["no", "t105"].contains(&name) { 3 } else { 4 };
        assert_eq!(loops + insertions, expected, "{name}");
        // Convention C1: every line at n = 1, so I_M = (-1)^lines I_E.
        assert_eq!(
            i64::from(m.sign),
            if lines % 2 == 0 { 1 } else { -1 },
            "{name}"
        );
        assert_eq!(
            m.graph_key().unwrap(),
            GraphKey::from_hex(golden[topology]).unwrap(),
            "{name}: the golden key of {topology}"
        );
        assert_eq!(
            m.value(&c, exact()).unwrap(),
            term(
                master(name),
                scheme(i16::from(loops)),
                coefficient(&c, &format!("{}/(d-3)^{loops}", m.sign))
            )
        );
        assert!(m.source.contains(".h:"), "{name}: a source path:line");
    }
}

/// Forcer's `Z(0,0,0,0,2,1,1,1,1,1)` (forcer-rules.h:281) is t1star55 at unit
/// powers: its leading arguments are eps units (forcer-rules.h:279), not a dot.
#[test]
fn t155_is_the_t1_topology_with_two_eps_units_on_one_line() {
    let t155 = definition("t155");
    assert!(t155.source.contains("forcer/forcer-rules.h:281"));
    let graph = t155.graph().unwrap();
    let units: Vec<u8> = graph.lines.iter().map(|line| line.2).collect();
    assert_eq!(units.iter().filter(|&&u| u == 2).count(), 1);
    assert_eq!(units.iter().filter(|&&u| u != 0).count(), 1);
    // One eps unit less on that line is t1star05, t105's topology.
    let mut t105 = definition("t105").graph().unwrap();
    for line in t105.lines.iter_mut().filter(|line| line.2 == 1) {
        line.2 = 2;
    }
    assert_eq!(
        rustred::propagator::topology::canonical(&t105).0,
        t155.graph_key().unwrap()
    );
}

#[test]
fn the_masters_follow_the_scope() {
    for loops in 1..=3 {
        assert_eq!(p_basis_masters(loops).unwrap(), P_BASIS_MASTERS_1_TO_3);
    }
    assert_eq!(p_basis_masters(4).unwrap(), P_BASIS_MASTERS_1_TO_4);
    for loops in [0, 5] {
        assert!(matches!(
            p_basis_masters(loops),
            Err(PError::Unsupported(_))
        ));
    }
    let four = PBasisOptions::for_max_loops(4).unwrap();
    let default = PBasisOptions::default();
    assert_eq!(four.masters, P_BASIS_MASTERS_1_TO_4);
    assert_eq!(
        (four.check_points, four.seed),
        (default.check_points, default.seed)
    );
    assert_eq!(default.masters, P_BASIS_MASTERS_1_TO_3, "the 1-3 default");
    let reference = one_loop_reference_library().unwrap();
    assert_eq!(
        PBasisOptions::for_library(&reference).unwrap().masters,
        P_BASIS_MASTERS_1_TO_3
    );
}

// ---------------------------------------------------------------------------
// The four-loop anchors
// ---------------------------------------------------------------------------

fn anchors_4l() -> Vec<ForcerCase> {
    parse_fixture(ANCHORS_4L_JSON).unwrap().cases
}

#[test]
fn the_4l_anchor_copy_is_the_vakint_fixture() {
    let file = parse_fixture(ANCHORS_4L_JSON).unwrap();
    assert_eq!(
        file.forcer_commit,
        "68a30bfb2aec3672a2f674ca8666bfc970105e24"
    );
    assert_eq!(file.form_version, "5.0.0");
    assert!(
        file.provenance
            .unwrap()
            .contains("reduced = (-1)^(sum a + r) C_G")
    );
    assert_eq!(
        blake3::hash(ANCHORS_4L_JSON.as_bytes()).to_hex().as_str(),
        ANCHORS_4L_BLAKE3,
        "basis_anchors/p4l_anchors.json changed: refresh its README provenance and this pin"
    );
    let names: Vec<String> = anchors_4l().into_iter().map(|a| a.name).collect();
    assert_eq!(
        names,
        [
            "bebe", "cross", "haha", "lala", "no1", "no2", "no6", "nono", "fastar2", "nostar5",
            "nostar6", "t124", "t145", "t155", "la4_top", "no3_top", "bubu_top",
        ]
    );
    // Each master case names its definition as ours does: topology, notation
    // and the first Forcer source line.
    let raw: Value = serde_json::from_str(ANCHORS_4L_JSON).unwrap();
    let mut anchored = vec!["no", "t105"]; // basis_anchors/p_anchors.json
    for case in raw["cases"].as_array().unwrap() {
        let Some(name) = case["master"].as_str() else {
            continue;
        };
        assert_eq!(case["name"], name);
        let m = definition(name);
        let text = case["definition"].as_str().unwrap();
        assert!(
            text.contains(&format!("topology {} {}", m.topology, m.notation)),
            "{name}: {text}"
        );
        let first = text.strip_prefix("form_src/forcer/").unwrap();
        let end = first.find(".h:").unwrap() + 3;
        let digits = first[end..].find(|c: char| !c.is_ascii_digit()).unwrap();
        assert!(
            m.source.contains(&first[..end + digits]),
            "{name}: {} not in {}",
            &first[..end + digits],
            m.source
        );
        anchored.push(m.name);
    }
    anchored.sort_unstable();
    assert_eq!(anchored, FORCER_MASTER_NAMES, "every master has an anchor");
}

/// Each master case is its master at Forcer's definition, with one bubble in
/// series per eps unit: Forcer gives `GschemeConstants(0,0)^k Master(name)`
/// with `GschemeConstants(0,0) = 1/eps`, and the Wick sign of the realisation
/// is the master's (two more lines per bubble). So vakint's `reduced` is
/// `sign eps^-k Master(name)`, what RustRed's `onto_masters` must give on the
/// same input. The three tops parse onto the 16 masters too.
#[test]
fn the_4l_anchors_read_as_their_masters() {
    let c = context();
    let (_, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    let raw: Value = serde_json::from_str(ANCHORS_4L_JSON).unwrap();
    for (case, raw) in anchors_4l().iter().zip(raw["cases"].as_array().unwrap()) {
        let top = (tops.iter())
            .find(|p| matches!(&p.kind, PresentationKind::Top { name } if *name == case.family))
            .unwrap_or_else(|| panic!("{}: family {}", case.name, case.family));
        assert_eq!(top.loops, 4, "{}", case.name);
        let mut key = case.powers.clone();
        key.resize(top.slots.len(), 0);
        let inputs = case.inputs(&c, top).unwrap();
        assert_eq!(
            inputs,
            [(top.id, IntegralKey::try_new(key).unwrap(), c.one())],
            "{}: numerator 1, ISPs at 0",
            case.name
        );
        let expected = case.expected(&c, 4).unwrap();
        let Some(name) = raw["master"].as_str() else {
            assert!(!expected.is_empty(), "{}", case.name);
            assert!(
                expected
                    .iter()
                    .any(|t| matches!(t.basis, PBasis::Master(_))),
                "{}",
                case.name
            );
            continue;
        };
        let m = definition(name);
        let k: u8 = eps_units(m).iter().sum();
        assert_eq!(
            expected,
            [term(
                master(name),
                GammaTag::one(),
                coefficient(&c, &format!("{}*(2/(4-d))^{k}", m.sign))
            )],
            "{name}"
        );
    }
}

// ---------------------------------------------------------------------------
// The Gamma tag bound
// ---------------------------------------------------------------------------

/// The reference library with Insertion roles on its bubble owner (its own
/// fake lane has none), the same program.
fn reference_with_bubble_roles() -> PLibrary {
    let reference = one_loop_reference_library().unwrap();
    let mut builder = PLibrary::builder(reference.coefficient_context().clone(), reference.scope());
    for presentation in reference.presentations() {
        builder
            .add_presentation(presentation.as_ref().clone())
            .unwrap();
    }
    builder.set_census(reference.census().clone()).unwrap();
    for owner in reference.owners() {
        let mut spec = owner.spec.clone();
        spec.roles = LaneRoles::Insertion(BubbleRoles {
            lines: [0, 1],
            loop_index: 0,
        });
        let program = owner.program(&reference).unwrap().clone();
        builder.add_owner_with_program(spec, program).unwrap();
    }
    builder.build().unwrap()
}

/// At four loops a presentation of `L` loops carries at most `4 - L` eps units,
/// so a bubble has `j1 + j2 <= 3` and a carpet `j_inner + j_r <= 3`;
/// `T_ins(j1, j2)` reaches `|m| = 2 + j1 + j2 <= 5`, within
/// `P_GAMMA_TAG_MAX_M`. `largest_gamma_m` counts it on a library (a one-loop
/// bubble: 2; the scope 1-4 count is M3-T17's STAGE 2).
#[test]
fn the_gamma_tag_bound_covers_four_loops() {
    let largest = |tag: &GammaTag| tag.exponents().iter().map(|&(m, _)| m.abs()).max();
    let mut bound = 0;
    for j1 in 0..=3_i64 {
        for j2 in 0..=3 - j1 {
            let m = largest(&relative_tag(j1, j2).unwrap()).unwrap();
            assert_eq!(i64::from(m), 2 + j1 + j2, "T_ins({j1}, {j2})");
            bound = bound.max(m);
        }
    }
    assert_eq!(bound, 5);
    assert!(bound <= P_GAMMA_TAG_MAX_M);
    let reference = one_loop_reference_library().unwrap();
    assert_eq!(
        largest_gamma_m(&reference).unwrap(),
        0,
        "a lane without roles"
    );
    assert_eq!(largest_gamma_m(&reference_with_bubble_roles()).unwrap(), 2);
}
