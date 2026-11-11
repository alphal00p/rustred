//! Masters no and t105 as the basis layer (owner: M2-T9 rr-basis).
//!
//! The layer's rows, its `p-layer.json` format and its fail-closed checks, the
//! reference master definitions (plan M2 section 3.3) and Forcer's G-scheme
//! translation, and the one-loop normalisation anchors on the reference
//! library. The 2L and 3L anchors (kites, NO, the t1star05 parents) need the
//! assembled library with its Custom owners: `rustred-app`
//! `tests/p_library_app/basis.rs`.
//!
//! Anchors: `basis_anchors/p_anchors.json`, a copy of vakint's Forcer fixture
//! (provenance in `basis_anchors/README.md`).
use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::basis::anchors::{ForcerCase, parse_fixture};
use rustred::propagator::basis::{
    FORCER_MASTER_NAMES, P_BASIS_MASTERS_1_TO_3, PBasisLayer, PBasisOptions, PBasisProvenance,
    PBasisRow, PBasisTerminal, coefficient_text, derive_basis_layer, forcer_g_scheme_terms,
    forcer_topology, g_scheme_constant, parse_coefficient, with_basis_layer,
};
use rustred::propagator::error::PError;
use rustred::propagator::library::{
    PBasis, PBasisMap, PLibrary, PReductionRequest, PTerm, PUncoveredPolicy,
};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::testing::one_loop_reference_library;
use rustred::propagator::topology::GraphKey;
use rustred::propagator::{
    NodeIndex, P_BASIS_LAYER_SCHEMA, P_OWNER_LIBRARY_SEMANTICS, PTerminalId, PresentationId,
};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits, SampledOptions,
};
use rustred::solver::litered::{DEFAULT_PRIME, PrimeField, ZpSample};
use serde_json::Value;

const ANCHORS_JSON: &str = include_str!("basis_anchors/p_anchors.json");
pub(crate) const GRAPH_KEYS_TSV: &str =
    include_str!("../../../rustred-topology/tests/fixtures/forcer/graph_keys_p_topology_v1.tsv");

pub(crate) fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

pub(crate) fn exact() -> ExactAlgebraLimits {
    ExactAlgebraLimits::default()
}

pub(crate) fn coefficient(c: &CoefficientContext, text: &str) -> Coefficient {
    parse_coefficient(c, text, exact()).unwrap()
}

pub(crate) fn tag(pairs: &[(i8, i16)]) -> GammaTag {
    GammaTag::from_pairs(pairs.iter().copied()).unwrap()
}

/// `(eps G(1,1))^loops` without `(d - 3)^-loops`.
pub(crate) fn scheme(loops: i16) -> GammaTag {
    tag(&[(1, loops), (-1, 2 * loops), (-2, -loops)])
}

pub(crate) fn term(basis: PBasis, tag: GammaTag, coefficient: Coefficient) -> PTerm {
    PTerm {
        basis,
        tag,
        coefficient,
    }
}

pub(crate) fn master(name: &str) -> PBasis {
    PBasis::Master(name.to_owned())
}

// ---------------------------------------------------------------------------
// The anchors
// ---------------------------------------------------------------------------

fn anchors() -> Vec<ForcerCase> {
    parse_fixture(ANCHORS_JSON).unwrap().cases
}

/// Exact, ModularPolynomial and ModularSampled (one thread each): every
/// reduction onto masters must agree in all three (M2 exit criterion 2).
fn arithmetics() -> [OrderedWeightArithmetic; 3] {
    let common = ModularCommon {
        threads: 1,
        ..Default::default()
    };
    [
        OrderedWeightArithmetic::Exact,
        OrderedWeightArithmetic::ModularPolynomial(common, PolynomialOptions::default()),
        OrderedWeightArithmetic::ModularSampled(common, SampledOptions::default()),
    ]
}

fn onto_masters(
    library: &PLibrary,
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
    arithmetic: OrderedWeightArithmetic,
) -> Vec<PTerm> {
    library
        .reduce(PReductionRequest {
            inputs,
            arithmetic,
            scheduler: OrderedSchedulerOptions::default(),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap()
        .terms
        .into_iter()
        .filter(|t| !t.coefficient.is_zero())
        .collect()
}

#[test]
fn the_anchor_copy_is_the_vakint_fixture() {
    let file = parse_fixture(ANCHORS_JSON).unwrap();
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
        blake3::hash(ANCHORS_JSON.as_bytes()).to_hex().as_str(),
        ANCHORS_BLAKE3,
        "basis_anchors/p_anchors.json changed: refresh its README provenance and this pin"
    );
    let names: Vec<String> = anchors().into_iter().map(|a| a.name).collect();
    assert_eq!(
        names,
        [
            "p1l_11",
            "p1l_kq_plus",
            "p1l_kq_minus",
            "kite",
            "kite_rank3_kp",
            "kite_rank3_mixed",
            "no",
            "t105_parent_la",
            "t105_parent_no",
            "t105_parent_la_dotted",
            "t105_parent_la_rank2",
        ]
    );
}

/// blake3 of `basis_anchors/p_anchors.json` (sha256 50ad1d17... in its README).
const ANCHORS_BLAKE3: &str = "b01f541bb4f320412bf8ad6311770d9b4302200be6a5ea9d9a06a4b00659da63";

// ---------------------------------------------------------------------------
// Reference masters
// ---------------------------------------------------------------------------

#[test]
fn the_reference_masters_are_forcer_topologies_with_pinned_signs() {
    let golden: BTreeMap<&str, &str> = GRAPH_KEYS_TSV
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split_once('\t').unwrap())
        .collect();
    let c = context();
    let names: Vec<&str> = P_BASIS_MASTERS_1_TO_3.iter().map(|m| m.name).collect();
    assert_eq!(names, ["no", "t105"]);
    // (name, Forcer topology, loops, lines, eps lines, sign): plan M2 section 3.3.
    for (definition, (name, topology, loops, lines, eps_lines, sign)) in
        P_BASIS_MASTERS_1_TO_3.iter().zip([
            ("no", "nostar0", 3, 8, 0, 1),
            ("t105", "t1star05", 2, 5, 1, -1),
        ])
    {
        assert_eq!((definition.name, definition.topology), (name, topology));
        assert!(FORCER_MASTER_NAMES.contains(&name));
        let graph = definition.graph().unwrap();
        assert_eq!(
            (graph.loops(), graph.line_count(), graph.eps_lines()),
            (loops, lines, eps_lines),
            "{name}"
        );
        // Convention C1: every line at n = 1, so I_M = (-1)^lines I_E.
        assert_eq!(definition.sign, sign);
        assert_eq!(i64::from(sign), if lines % 2 == 0 { 1 } else { -1 });
        assert_eq!(
            definition.graph_key().unwrap(),
            GraphKey::from_hex(golden[topology]).unwrap(),
            "{name}: the golden key of {topology}"
        );
        assert_eq!(definition.loops().unwrap(), loops);
        assert_eq!(definition.scheme_tag().unwrap(), scheme(i16::from(loops)));
        let value = definition.value(&c, exact()).unwrap();
        let expected = coefficient(&c, &format!("{sign}/(d-3)^{loops}"));
        assert_eq!(
            value,
            term(master(name), scheme(i16::from(loops)), expected)
        );
    }
    assert_eq!(FORCER_MASTER_NAMES.len(), 16);
    assert!(FORCER_MASTER_NAMES.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn forcer_notation_is_read_fail_closed() {
    let kite = forcer_topology("0-1* 2-3 1-2 1-4 3-4 2-4* 3-9").unwrap();
    assert_eq!((kite.n, kite.s, kite.t, kite.line_count()), (4, 0, 1, 5));
    assert_eq!(kite.eps_lines(), 1);
    for bad in [
        "",
        "0-1 1-2",                 // no internal line between the legs
        "0-1 1-2 2-3 x-4 2-9",     // not a vertex
        "0-1 1-2 2-3 1-3",         // one leg only
        "0-1 0-2 1-2 2-3 1-3 3-9", // two legs at vertex 0
        "0-1 1-2 1-9",             // both legs on one vertex
        "0-1 1-2 2",               // not a-b
    ] {
        assert!(
            matches!(forcer_topology(bad), Err(PError::Unsupported(_))),
            "{bad:?}"
        );
    }
    let mut not_normal = P_BASIS_MASTERS_1_TO_3[1];
    not_normal.notation = "0-1 1-2 2-3 1-3 3-9"; // lines in series: not a normal form
    assert!(matches!(not_normal.graph(), Err(PError::Unsupported(_))));
    let mut unknown = P_BASIS_MASTERS_1_TO_3[1];
    unknown.name = "t1";
    assert!(matches!(
        unknown.value(&context(), exact()),
        Err(PError::Unsupported(_))
    ));
}

// ---------------------------------------------------------------------------
// Forcer's G-scheme
// ---------------------------------------------------------------------------

/// `Prod GC^e` of `(x1, x2, e)` factors, times `rational`, as (tag, coefficient).
fn gc_product(
    c: &CoefficientContext,
    factors: &[(i8, i8, i16)],
    rational: &str,
) -> (GammaTag, Coefficient) {
    let mut total = (GammaTag::one(), coefficient(c, rational));
    for &(x1, x2, e) in factors {
        let (g, value) = g_scheme_constant(c, x1, x2, exact()).unwrap();
        for _ in 0..e.unsigned_abs() {
            if e > 0 {
                total.0 = total.0.add(&g);
                total.1 = c.try_mul(&total.1, &value, exact()).unwrap();
            } else {
                total.0 = total.0.add(&g.neg());
                total.1 = c.try_div(&total.1, &value, exact()).unwrap();
            }
        }
    }
    total
}

#[test]
fn g_scheme_constants_obey_the_identities_of_masters_prc() {
    let c = context();
    let ep = "((4-d)/2)";
    // GschemeConstants(0,0) = rat(1,ep).
    assert_eq!(
        gc_product(&c, &[(0, 0, 1)], "1"),
        (GammaTag::one(), coefficient(&c, &format!("1/{ep}")))
    );
    // Masters.prc:472-473: GC(2,2) = GC(3,1) GC(2,0) / GC(1,0) (1-4ep)/(1-3ep).
    assert_eq!(
        gc_product(&c, &[(2, 2, 1)], "1"),
        gc_product(
            &c,
            &[(3, 1, 1), (2, 0, 1), (1, 0, -1)],
            &format!("(1-4*{ep})/(1-3*{ep})")
        )
    );
    // Masters.prc:474-475: GC(3,1) GC(0,0) = GC(4,0) GC(3,0) (1-5ep)/(1-2ep).
    assert_eq!(
        gc_product(&c, &[(3, 1, 1), (0, 0, 1)], "1"),
        gc_product(
            &c,
            &[(4, 0, 1), (3, 0, 1)],
            &format!("(1-5*{ep})/(1-2*{ep})")
        )
    );
    // Without the rational factor the identity fails.
    assert_ne!(
        gc_product(&c, &[(2, 2, 1)], "1"),
        gc_product(&c, &[(3, 1, 1), (2, 0, 1), (1, 0, -1)], "1")
    );
    // GC(1,0) = Gamma(1+2eps) Gamma(1-2eps)^2 / (Gamma(1-eps) Gamma(1+eps)^2 Gamma(1-3eps))
    // times (1-2eps)/(eps (1-3eps)).
    let (g10, value) = g_scheme_constant(&c, 1, 0, exact()).unwrap();
    assert_eq!(g10, tag(&[(-3, -1), (-2, 2), (-1, -1), (1, -2), (2, 1)]));
    assert_eq!(
        value,
        coefficient(&c, &format!("(1-2*{ep})/({ep}*(1-3*{ep}))"))
    );
}

#[test]
fn forcer_outputs_become_tag_basis_terms() {
    let c = context();
    let ep = "((4-d)/2)";
    // The kite anchor: GC(0,0)^2 (1-2ep)/ep + GC(0,0) GC(1,0) (3ep-1)/ep.
    let kite = forcer_g_scheme_terms(
        &c,
        "GschemeConstants(0,0)^2*rat( - 2*ep + 1,ep) + \
         GschemeConstants(0,0)*GschemeConstants(1,0)*rat(3*ep - 1,ep)\n",
        exact(),
    )
    .unwrap();
    let (g10, v10) = g_scheme_constant(&c, 1, 0, exact()).unwrap();
    let first = coefficient(&c, &format!("(1-2*{ep})/{ep}^3"));
    let second = c
        .try_mul(
            &v10,
            &coefficient(&c, &format!("(3*{ep}-1)/{ep}^2")),
            exact(),
        )
        .unwrap();
    let mut expected = vec![
        term(PBasis::Unit, GammaTag::one(), first),
        term(PBasis::Unit, g10, second),
    ];
    expected.sort_by(|a, b| a.tag.cmp(&b.tag));
    assert_eq!(kite, expected);
    // A master term, a negative power, and terms that cancel.
    let terms = forcer_g_scheme_terms(
        &c,
        "GschemeConstants(0,0)*Master(t105)*rat( - 2*ep,3*ep - 1) + rat(1,2)*GschemeConstants(1,0)^-1 \
         - rat(1,2)*GschemeConstants(1,0)^-1",
        exact(),
    )
    .unwrap();
    assert_eq!(
        terms,
        [term(
            master("t105"),
            GammaTag::one(),
            coefficient(&c, &format!("-2*{ep}/((3*{ep}-1)*{ep})"))
        )]
    );
    for bad in [
        "Master(t1)",
        "Master(no)^2",
        "Master(no)*Master(t105)",
        "GschemeConstants(0)",
        "Gamma(1+ep)",
        "rat(1,2",
        "rat(1,2)+",
        "rat(1,x)",
    ] {
        assert!(forcer_g_scheme_terms(&c, bad, exact()).is_err(), "{bad}");
    }
}

// ---------------------------------------------------------------------------
// The layer
// ---------------------------------------------------------------------------

fn unit_row(c: &CoefficientContext, node: Option<NodeIndex>) -> PBasisRow {
    PBasisRow {
        terminal: PBasisTerminal {
            id: PTerminalId(0),
            class: None,
            node,
            key: Vec::new(),
        },
        terms: vec![term(PBasis::Unit, GammaTag::one(), c.one())],
        provenance: PBasisProvenance::Identity,
    }
}

fn residual_row(id: u32, class: &str, node: u32, key: &[i64], terms: Vec<PTerm>) -> PBasisRow {
    PBasisRow {
        terminal: PBasisTerminal {
            id: PTerminalId(id),
            class: Some(class.to_owned()),
            node: Some(NodeIndex(node)),
            key: key.to_vec(),
        },
        terms,
        provenance: PBasisProvenance::Rustred,
    }
}

/// A layer in the shape of the 1L-3L library's (and a mixed third row).
fn sample_rows(c: &CoefficientContext) -> Vec<PBasisRow> {
    vec![
        unit_row(c, Some(NodeIndex(42))),
        residual_row(
            1,
            "L2T05-810c5f0ff11c4c40",
            28,
            &[1, 1, 1, 1, 1],
            vec![term(
                master("t105"),
                scheme(2),
                coefficient(c, "-1/(d-3)^2"),
            )],
        ),
        residual_row(
            2,
            "L3T08-61cd69124cf81eb3",
            1,
            &[1, 1, 1, 1, 1, 1, 1, 1, 0],
            vec![
                term(
                    PBasis::Unit,
                    tag(&[(1, 1)]),
                    coefficient(c, "(d-4)/(2*d-7)"),
                ),
                term(master("no"), scheme(3), coefficient(c, "1/(d-3)^3")),
                term(master("t105"), GammaTag::one(), c.zero()),
            ],
        ),
    ]
}

#[test]
fn rows_are_checked_when_a_layer_is_made() {
    let c = context();
    assert!(PBasisLayer::new(&c, sample_rows(&c)).is_ok());
    let refused = |rows: Vec<PBasisRow>| {
        matches!(PBasisLayer::new(&c, rows), Err(PError::MalformedLibrary(_)))
    };
    assert!(refused(Vec::new()));
    let mut rows = sample_rows(&c);
    rows.swap(1, 2);
    assert!(refused(rows), "rows out of id order");
    let mut rows = sample_rows(&c);
    rows[0].terms[0].coefficient = c.integer(2);
    assert!(refused(rows), "Unit = 2 Unit");
    let mut rows = sample_rows(&c);
    rows[0].provenance = PBasisProvenance::Rustred;
    assert!(refused(rows), "a derived Unit row");
    let mut rows = sample_rows(&c);
    rows[1].provenance = PBasisProvenance::Identity;
    assert!(refused(rows), "an identity residual row");
    let mut rows = sample_rows(&c);
    rows[1].terminal.class = None;
    assert!(refused(rows), "a residual without its class");
    let mut rows = sample_rows(&c);
    rows[1].terms.clear();
    assert!(refused(rows), "an empty row is a silent zero");
    let mut rows = sample_rows(&c);
    rows[1].terms[0].basis = PBasis::Residual(PTerminalId(2));
    assert!(refused(rows), "a residual term");
    let mut rows = sample_rows(&c);
    rows[1].terms[0].basis = master("t1");
    assert!(refused(rows), "a master outside Forcer's 16");
    let mut rows = sample_rows(&c);
    rows[2].terms.swap(0, 1);
    assert!(refused(rows), "terms out of order");
    let mut rows = sample_rows(&c);
    let repeated = rows[2].terms[1].clone();
    rows[2].terms.insert(1, repeated);
    assert!(refused(rows), "a repeated term");
    let mut rows = sample_rows(&c);
    rows[1].terms[0].tag = tag(&[(1, 127)]).add(&tag(&[(1, 1)]));
    assert!(matches!(
        PBasisLayer::new(&c, rows),
        Err(PError::TagOutOfRange)
    ));
    let other = CoefficientContext::try_new(["d", "x"]).unwrap();
    let mut rows = sample_rows(&c);
    rows[1].terms[0].coefficient = other.parameter("x").unwrap();
    assert!(matches!(
        PBasisLayer::new(&c, rows),
        Err(PError::ContextMismatch)
    ));
}

#[test]
fn the_layer_json_round_trips_and_fails_closed() {
    let c = context();
    let layer = PBasisLayer::new(&c, sample_rows(&c)).unwrap();
    let json = layer.to_json();
    assert!(json.ends_with('\n'));
    let file: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(file["schema"], P_BASIS_LAYER_SCHEMA);
    assert_eq!(
        file["base"]["P_OWNER_LIBRARY_SEMANTICS"],
        P_OWNER_LIBRARY_SEMANTICS
    );
    assert_eq!(file["base"]["terminals"], 3);
    assert_eq!(file["masters"], serde_json::json!(["no", "t105"]));
    assert_eq!(file["rows"][0]["terms"][0]["master"], "1");
    assert_eq!(file["rows"][0]["provenance"], "identity");
    assert_eq!(file["rows"][1]["provenance"], "rustred");
    assert_eq!(
        file["rows"][1]["terms"][0]["tag"],
        serde_json::json!([[-2, -2], [-1, 4], [1, 2]])
    );
    let back = PBasisLayer::from_json(&json, &c).unwrap();
    assert_eq!(back, layer);
    assert_eq!(back.to_json(), json, "bytes are deterministic");
    for row in layer.rows() {
        for t in &row.terms {
            assert_eq!(
                parse_coefficient(&c, &coefficient_text(&t.coefficient), exact()).unwrap(),
                t.coefficient
            );
        }
    }

    let edited = |from: &str, to: &str| {
        assert!(json.contains(from), "{from}");
        PBasisLayer::from_json(&json.replacen(from, to, 1), &c)
    };
    match edited(
        P_OWNER_LIBRARY_SEMANTICS,
        "rustred-p-owner-library-1999-01-01",
    ) {
        Err(PError::VersionMismatch {
            name,
            found,
            expected,
        }) => {
            assert_eq!(name, "P_OWNER_LIBRARY_SEMANTICS");
            assert_eq!(found, "rustred-p-owner-library-1999-01-01");
            assert_eq!(expected, P_OWNER_LIBRARY_SEMANTICS);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        edited("\"P_CENSUS_VERSION\": \"2\"", "\"P_CENSUS_VERSION\": \"3\""),
        Err(PError::VersionMismatch { .. })
    ));
    assert!(matches!(
        edited(P_BASIS_LAYER_SCHEMA, "rustred.p-basis-layer.v0"),
        Err(PError::VersionMismatch { .. })
    ));
    assert!(matches!(
        edited("\"schema\"", "\"extra\": 1, \"schema\""),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        edited("\"terminals\": 3", "\"terminals\": 2"),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        edited(
            "\"masters\": [\n    \"no\",",
            "\"masters\": [\n    \"nono\","
        ),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        edited(
            "\"provenance\": \"rustred\"",
            "\"provenance\": \"kira-stopgap\""
        ),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        edited("\"master\": \"t105\"", "\"master\": \"t1\""),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        edited(
            "\"parameters\": [\n    \"d\"",
            "\"parameters\": [\n    \"D\""
        ),
        Err(PError::ContextMismatch)
    ));
    let coefficient_of_row_1 = file["rows"][1]["terms"][0]["coefficient"]
        .as_str()
        .unwrap()
        .to_owned();
    for bad in ["x/(d-3)", "Gamma(d)", "1;2", "", "d^(1/2)"] {
        assert!(
            matches!(
                edited(
                    &format!("\"coefficient\": \"{coefficient_of_row_1}\""),
                    &format!("\"coefficient\": \"{bad}\"")
                ),
                Err(PError::Unsupported(_))
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn the_map_adds_its_tags_to_the_absolute_tag() {
    let c = context();
    let layer = PBasisLayer::new(&c, sample_rows(&c)).unwrap();
    let absolute = tag(&[(1, -3), (2, 1)]);
    assert_eq!(
        layer.map(PTerminalId(0), &absolute).unwrap(),
        [term(PBasis::Unit, absolute.clone(), c.one())]
    );
    assert_eq!(
        layer.map(PTerminalId(1), &absolute).unwrap(),
        [term(
            master("t105"),
            absolute.add(&scheme(2)),
            coefficient(&c, "-1/(d-3)^2")
        )]
    );
    let row = layer.map(PTerminalId(2), &GammaTag::one()).unwrap();
    assert_eq!(row.len(), 3, "a zero term stays: it is not a silent drop");
    assert!(matches!(
        layer.map(PTerminalId(3), &absolute),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        layer.map(PTerminalId(1), &tag(&[(1, 127)])),
        Err(PError::TagOutOfRange)
    ));
}

#[test]
fn rows_evaluate_modulo_a_prime() {
    let c = context();
    let layer = PBasisLayer::new(&c, sample_rows(&c)).unwrap();
    let field = PrimeField::new(DEFAULT_PRIME).unwrap();
    let images = layer
        .images(&ZpSample::new(field.clone(), vec![5]))
        .unwrap();
    // d = 5: -1/(d-3)^2 = -1/4, (d-4)/(2d-7) = 1/3, 1/(d-3)^3 = 1/8.
    let value = |numerator: i64, denominator: u64| {
        field.mul(field.from_i64(numerator), field.inv(denominator).unwrap())
    };
    assert_eq!(images[0], [(PBasis::Unit, GammaTag::one(), 1)]);
    assert_eq!(images[1], [(master("t105"), scheme(2), value(-1, 4))]);
    assert_eq!(
        images[2],
        [
            (PBasis::Unit, tag(&[(1, 1)]), value(1, 3)),
            (master("no"), scheme(3), value(1, 8)),
            (master("t105"), GammaTag::one(), 0),
        ]
    );
    // d = 3 is a pole of the masters' rows: refused, never a value.
    assert!(matches!(
        layer.images(&ZpSample::new(field, vec![3])),
        Err(PError::Unsupported(_))
    ));
}

// ---------------------------------------------------------------------------
// The reference library: the layer of a library without residuals, 1L anchors
// ---------------------------------------------------------------------------

#[test]
fn a_library_without_residuals_has_the_unit_row_only() {
    let library = one_loop_reference_library().unwrap();
    let derivation = derive_basis_layer(&library, &PBasisOptions::default()).unwrap();
    assert_eq!(
        derivation.absent,
        ["no", "t105"],
        "no topology, no master node"
    );
    assert!(derivation.sites.is_empty());
    assert_eq!(derivation.checks.len(), 2);
    assert!(
        derivation
            .checks
            .iter()
            .all(|check| check.prime == DEFAULT_PRIME)
    );
    assert_ne!(derivation.checks[0].d, derivation.checks[1].d);
    let c = library.coefficient_context();
    assert_eq!(derivation.layer.rows(), [unit_row(c, None)]);
    derivation.layer.check_library(&library).unwrap();

    // The inventory binds: a layer with residual rows does not fit this library.
    let foreign = Arc::new(PBasisLayer::new(c, sample_rows(c)).unwrap());
    assert!(matches!(
        foreign.check_library(&library),
        Err(PError::MalformedLibrary(_))
    ));
    assert!(matches!(
        with_basis_layer(&library, foreign),
        Err(PError::MalformedLibrary(_))
    ));
    let moved = Arc::new(PBasisLayer::new(c, vec![unit_row(c, Some(NodeIndex(0)))]).unwrap());
    assert!(matches!(
        with_basis_layer(&library, moved),
        Err(PError::MalformedLibrary(_))
    ));

    // No master needed, none given: still the Unit row.
    let mut options = PBasisOptions::default();
    options.masters.clear();
    assert_eq!(
        derive_basis_layer(&library, &options)
            .unwrap()
            .layer
            .rows()
            .len(),
        1
    );
}

#[test]
fn the_one_loop_anchors_hold_on_the_reference_library() {
    let reference = one_loop_reference_library().unwrap();
    let layer = derive_basis_layer(&reference, &PBasisOptions::default())
        .unwrap()
        .layer;
    let library = with_basis_layer(&reference, Arc::new(layer.clone())).unwrap();
    assert_eq!(library.identity(), reference.identity());
    assert_eq!(PBasisLayer::from_library(&reference).unwrap(), None);
    assert_eq!(PBasisLayer::from_library(&library).unwrap(), Some(layer));
    let c = library.coefficient_context();
    let top = library.top_presentation("P1L").unwrap();
    let mut checked = 0;
    for anchor in anchors().iter().filter(|a| a.family == "P1L") {
        let inputs = anchor.inputs(c, library.presentation(top)).unwrap();
        let expected = anchor.expected(c, 1).unwrap();
        for arithmetic in arithmetics() {
            assert_eq!(
                onto_masters(&library, inputs.clone(), arithmetic),
                expected,
                "{}, {}",
                anchor.name,
                arithmetic.name()
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 3);
    // P1L(1,1) = 1/eps exactly (convention C3), whatever the fixture says.
    let one_over_eps = coefficient(c, "2/(4-d)");
    assert_eq!(
        onto_masters(
            &library,
            vec![(top, IntegralKey::try_new([1, 1]).unwrap(), c.one())],
            OrderedWeightArithmetic::Exact
        ),
        [term(PBasis::Unit, GammaTag::one(), one_over_eps)]
    );
}

#[test]
fn fixture_numerators_are_lowered_onto_the_slots() {
    use rustred::propagator::families::{P_FAMILIES_V1_JSON, load_p_families};
    let c = context();
    let (_, tops) = load_p_families(P_FAMILIES_V1_JSON).unwrap();
    let p1l = tops.iter().find(|p| p.loops == 1).unwrap();
    let case = |numerator: &str, external: &str| ForcerCase {
        name: "case".to_owned(),
        family: "P1L".to_owned(),
        powers: vec![1, 1],
        numerator: numerator.to_owned(),
        external: external.to_owned(),
        outputs: Vec::new(),
    };
    // k.q = (k^2 + q.q - (k - q)^2) / 2 with q.q = -1 (convention C1).
    let key = |powers: [i64; 2]| IntegralKey::try_new(powers).unwrap();
    let lowered = |numerator: &str, external: &str| -> BTreeMap<IntegralKey, Coefficient> {
        (case(numerator, external)
            .inputs(&c, p1l)
            .unwrap()
            .into_iter())
        .map(|(id, key, weight)| {
            assert_eq!(id, p1l.id);
            (key, weight)
        })
        .collect()
    };
    assert_eq!(
        lowered("dot(k(1),p(1))", "p(1)"),
        BTreeMap::from([
            (key([0, 1]), coefficient(&c, "1/2")),
            (key([1, 0]), coefficient(&c, "-1/2")),
            (key([1, 1]), coefficient(&c, "-1/2")),
        ])
    );
    // p(1) = -q: every weight flips.
    assert_eq!(
        lowered("dot(k(1),p(1))", "-p(1)"),
        BTreeMap::from([
            (key([0, 1]), coefficient(&c, "-1/2")),
            (key([1, 0]), coefficient(&c, "1/2")),
            (key([1, 1]), coefficient(&c, "1/2")),
        ])
    );
    for (numerator, external) in [
        ("dot(k(1),p(1))", "q"),
        ("dot(p(1),p(1))", "p(1)"),
        ("dot(k(2),p(1))", "p(1)"),
        ("k(1)", "p(1)"),
    ] {
        assert!(
            matches!(
                case(numerator, external).inputs(&c, p1l),
                Err(PError::Unsupported(_))
            ),
            "{numerator} {external}"
        );
    }
    let mut short = case("1", "p(1)");
    short.powers = vec![1];
    assert!(short.inputs(&c, p1l).is_err());
}
