//! Forcer's 16 masters on the scope 1-4 library (owner: M3-T17 rr-basis4).
//!
//! `rustred::propagator::basis` on the scope 1-4 program: its layer is square
//! (16 reference masters for 16 residual terminals) and passes the fresh-`d`
//! checks; each residual maps onto the master of its own topology; every Forcer
//! anchor (the 17 four-loop ones of M3-V7 and the 11 of 1L-3L) holds exactly in
//! the (Gamma(1 + m eps) monomial x master) basis under Exact,
//! ModularPolynomial and ModularSampled, which pins every sign and the
//! normalisations Forcer marks "XXX: normalization?"; the 64 rows of the
//! rules-study M_MAP name these masters; the Gamma tags stay within
//! `P_GAMMA_TAG_MAX_M`; and random four-loop inputs reduce onto `Unit` and
//! masters only.
//!
//! The program is the package named by `RUSTRED_P_LIBRARY_1_4` (M3-T8c's, not
//! in the repository; its 10- and 11-line Custom owners take hours to
//! generate). The ignored tests need it; the reduced-bound random test runs by
//! default and says that it skipped when the variable is unset.
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::propagator::basis::anchors::{ForcerCase, parse_fixture};
use rustred::propagator::basis::{
    FORCER_MASTER_NAMES, P_BASIS_MASTERS_1_TO_4, PBasisDerivation, PBasisOptions, PBasisProvenance,
    PMasterDefinition, RELATION_CHECKS, derive_basis_layer, largest_gamma_m, node_corner,
    with_basis_layer,
};
use rustred::propagator::library::{
    PBasis, PLibrary, PReductionRequest, PTerm, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{PresentationKind, Slot};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::topology::{Action, PTopo, canonical};
use rustred::propagator::{P_GAMMA_TAG_MAX_M, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits, SampledOptions,
};
use rustred_app::propagator::assemble::SplitMix;
use rustred_app::propagator::{PLibraryOptions, PLibrarySource, load_p_library};

const ANCHORS_JSON: &str =
    include_str!("../../../rustred-core/tests/p_library/basis_anchors/p_anchors.json");
const ANCHORS_4L_JSON: &str =
    include_str!("../../../rustred-core/tests/p_library/basis_anchors/p4l_anchors.json");
const M_MAP_TSV: &str =
    include_str!("../../../rustred-core/tests/p_library/basis_anchors/m_map.tsv");

/// The directory of the scope 1-4 package.
const PACKAGE: &str = "RUSTRED_P_LIBRARY_1_4";

fn package() -> Option<PathBuf> {
    std::env::var_os(PACKAGE).map(PathBuf::from)
}

/// The scope 1-4 program of the package, loaded once (the writer checked its
/// descent certificate).
fn scope_four() -> &'static Arc<PLibrary> {
    static LIBRARY: OnceLock<Arc<PLibrary>> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let directory =
            package().unwrap_or_else(|| panic!("{PACKAGE} names the scope 1-4 package"));
        let start = Instant::now();
        let library = load_p_library(
            PLibrarySource::Directory(directory),
            &PLibraryOptions {
                verify_descent: false,
            },
        )
        .unwrap();
        assert_eq!(library.scope(), 1..=4);
        println!(
            "scope 1-4 package loaded in {:.1} s",
            start.elapsed().as_secs_f64()
        );
        library
    })
}

fn derivation_four() -> &'static PBasisDerivation {
    static DERIVATION: OnceLock<PBasisDerivation> = OnceLock::new();
    DERIVATION.get_or_init(|| {
        let start = Instant::now();
        let options = PBasisOptions::for_library(scope_four()).unwrap();
        let derivation = derive_basis_layer(scope_four(), &options).unwrap();
        println!(
            "scope 1-4 layer derived and checked in {:.1} s",
            start.elapsed().as_secs_f64()
        );
        derivation
    })
}

/// The scope 1-4 program with its layer.
fn mapped_four() -> &'static PLibrary {
    static LIBRARY: OnceLock<PLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        with_basis_layer(scope_four(), Arc::new(derivation_four().layer.clone())).unwrap()
    })
}

fn coefficient(c: &CoefficientContext, text: &str) -> Coefficient {
    rustred::propagator::basis::parse_coefficient(c, text, Default::default()).unwrap()
}

fn master(name: &str) -> PBasis {
    PBasis::Master(name.to_owned())
}

fn definition(name: &str) -> &'static PMasterDefinition {
    (P_BASIS_MASTERS_1_TO_4.iter())
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no master {name}"))
}

/// `(eps G(1,1))^loops` without `(d - 3)^-loops`.
fn scheme(loops: i16) -> GammaTag {
    GammaTag::from_pairs([(1, loops), (-1, 2 * loops), (-2, -loops)]).unwrap()
}

fn modular_polynomial() -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularPolynomial(
        ModularCommon {
            threads: 1,
            ..Default::default()
        },
        PolynomialOptions::default(),
    )
}

fn modular_sampled() -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularSampled(
        ModularCommon {
            threads: 1,
            ..Default::default()
        },
        SampledOptions::default(),
    )
}

type Input = (PresentationId, IntegralKey, Coefficient);

/// The nonzero terms of `inputs` reduced onto masters under `arithmetic`.
fn onto_masters(
    library: &PLibrary,
    inputs: &[Input],
    arithmetic: OrderedWeightArithmetic,
) -> Vec<PTerm> {
    let mut reduction = library
        .reduce(PReductionRequest {
            inputs: inputs.to_vec(),
            arithmetic,
            scheduler: OrderedSchedulerOptions::default(),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: true,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap();
    reduction.terms.retain(|term| !term.coefficient.is_zero());
    reduction.terms
}

/// Exact's terms, asserted equal under each of `others`.
fn onto_masters_in(
    library: &PLibrary,
    inputs: &[Input],
    others: &[(&str, OrderedWeightArithmetic)],
    what: &str,
) -> Vec<PTerm> {
    let exact = onto_masters(library, inputs, OrderedWeightArithmetic::Exact);
    for (name, arithmetic) in others {
        assert_eq!(
            onto_masters(library, inputs, *arithmetic),
            exact,
            "{what}: {name} differs from Exact"
        );
    }
    exact
}

/// The residual terminals of `library` on DAG node `node`.
fn residuals_on(library: &PLibrary, node: u32) -> Vec<usize> {
    (library.terminals().iter().enumerate())
        .filter(|(_, terminal)| match terminal {
            PTerminal::Residual { owner, .. } => library
                .census()
                .classes
                .get(owner)
                .is_some_and(|record| record.node.is_some_and(|n| n.0 == node)),
            PTerminal::Unit => false,
        })
        .map(|(index, _)| index)
        .collect()
}

/// Steps (2), (3) and (6) of M3-T17: the 16 masters and the relations between
/// residuals square the 17 residual terminals, each master integral is its own
/// residual, each row is the master of the residual's topology with its pinned
/// sign, the rows pass the fresh-`d` checks, and the Gamma tags of the program
/// fit the tag range. M5 B2: the rank-gated no1 owner leaves a second residual
/// `[2, 1^10, 0, 0, 0]` on the no1 node; one relation from RustRed's IBP rows
/// of its family closes the system, two more hold as checks, and its row is
/// a combination of the masters (with Unit), not a single master term.
/// M5 combined: no1 is on the Scheme lane by default (Forcer's scheme, one
/// residual), so the layer is M3-T17's square system of 17 rows without
/// relations; a package assembled with `RUSTRED_P_SCHEMES=none` keeps the
/// rank-gated no1 and B2's extra residual.
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn the_scope_four_layer_is_square_and_maps_each_residual_onto_its_master() {
    let library = scope_four();
    let c = library.coefficient_context();
    let derivation = derivation_four();
    assert!(derivation.absent.is_empty(), "{:?}", derivation.absent);
    let residuals: Vec<usize> = (library.terminals().iter().enumerate())
        .filter(|(_, t)| matches!(t, PTerminal::Residual { .. }))
        .map(|(index, _)| index)
        .collect();
    // B2's extra no1 residual only without the Scheme lane (M5 combined).
    let scheme_no1 = library
        .owners()
        .any(|owner| owner.spec.lane == rustred::propagator::lane::OwnerLane::Scheme);
    let extras = usize::from(!scheme_no1);
    assert_eq!(residuals.len(), 16 + extras);
    assert_eq!(library.terminals().len(), 17 + extras);

    let sites: Vec<&str> = derivation.sites.iter().map(|s| s.master.name).collect();
    assert_eq!(sites, FORCER_MASTER_NAMES);
    for site in &derivation.sites {
        let name = site.master.name;
        let [reduced] = site.reduction.as_slice() else {
            panic!("{name}: {:?}", site.reduction);
        };
        let PBasis::Residual(id) = reduced.basis else {
            panic!("{name}: {reduced:?}");
        };
        assert!(
            reduced.tag.is_one() && reduced.coefficient == c.one(),
            "{name}"
        );
        let PTerminal::Residual { key, .. } = &library.terminals()[id.0 as usize] else {
            panic!("{name}: terminal {}", id.0);
        };
        assert_eq!(
            key.powers(),
            site.key,
            "{name}: the residual is the master integral"
        );
        let on_node = residuals_on(library, site.node.0);
        assert_eq!(on_node[0], id.0 as usize, "{name}");
        assert_eq!(
            on_node.len(),
            if name == "no1" { 1 + extras } else { 1 },
            "{name}"
        );
        let presentation = library.presentation(site.presentation);
        let eps_lines = (presentation.slots.iter())
            .filter(|slot| matches!(slot, Slot::Line(line) if line.eps_units > 0))
            .count();
        assert_eq!(
            presentation.kind == PresentationKind::EpsNode,
            eps_lines > 0,
            "{name}: {:?}",
            presentation.kind
        );
        println!(
            "master {name} ({}, sign {:+}): node {}, presentation {} ({:?}), key {:?}",
            site.master.topology,
            site.master.sign,
            site.node.0,
            site.presentation.0,
            presentation.kind,
            site.key
        );
    }

    // Rows: Unit, then I_M(master) = sign (eps G)^L Master(name) solved for
    // the residual of the master's topology.
    let layer = &derivation.layer;
    assert_eq!(layer.rows().len(), 17 + extras);
    assert_eq!(layer.rows()[0].provenance, PBasisProvenance::Identity);
    for site in &derivation.sites {
        let PBasis::Residual(id) = site.reduction[0].basis else {
            unreachable!()
        };
        let row = &layer.rows()[id.0 as usize];
        assert_eq!(row.provenance, PBasisProvenance::Rustred);
        let loops = site.master.loops().unwrap();
        assert_eq!(
            row.terms,
            [PTerm {
                basis: master(site.master.name),
                tag: scheme(i16::from(loops)),
                coefficient: coefficient(c, &format!("{}/(d-3)^{loops}", site.master.sign)),
            }],
            "{}",
            site.master.name
        );
    }
    assert_eq!(layer.masters(), FORCER_MASTER_NAMES);
    layer.check_library(library).unwrap();

    // The extra residual of no1 and its relations.
    let site_terminals: BTreeSet<usize> = (derivation.sites.iter())
        .map(|site| match site.reduction[0].basis {
            PBasis::Residual(id) => id.0 as usize,
            _ => unreachable!(),
        })
        .collect();
    let extra: Vec<usize> = (residuals.iter().copied())
        .filter(|index| !site_terminals.contains(index))
        .collect();
    if extras == 0 {
        assert!(extra.is_empty(), "extra residuals {extra:?}");
        assert!(derivation.relations.is_empty());
    }
    for extra in &extra {
        let PTerminal::Residual { key, .. } = &library.terminals()[*extra] else {
            unreachable!()
        };
        assert_eq!(key.powers(), [2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0]);
        let row = &layer.rows()[*extra];
        assert_eq!(row.provenance, PBasisProvenance::Rustred);
        assert!(
            row.terms.len() > 1
                && (row.terms.iter())
                    .any(|term| term.basis == master("no1") && !term.coefficient.is_zero()),
            "{:?}",
            row.terms
        );
        let selected = derivation.relations.iter().filter(|r| r.selected).count();
        assert_eq!(
            (selected, derivation.relations.len()),
            (1, 1 + RELATION_CHECKS)
        );
        for relation in &derivation.relations {
            println!(
                "relation: IBP row {} at {:?} (presentation {}), {}, {} terms",
                relation.ibp_row,
                relation.seed,
                relation.presentation.0,
                if relation.selected {
                    "in the system"
                } else {
                    "check"
                },
                relation.reduction.len()
            );
        }
        println!(
            "extra residual {extra} {:?}: {} terms",
            key.powers(),
            row.terms.len()
        );
    }

    // Fresh d modulo 2^61 - 1, one component per master.
    assert_eq!(derivation.checks.len(), 2);
    for check in &derivation.checks {
        assert_eq!(check.components, 16 + extras * (1 + RELATION_CHECKS));
        println!(
            "fresh check: d = {} modulo {}: {} components",
            check.d, check.prime, check.components
        );
    }
    assert_ne!(derivation.checks[0].d, derivation.checks[1].d);

    // Step (6): the largest Gamma(1 + m eps) of a step of the program.
    let largest = largest_gamma_m(library).unwrap();
    println!("largest |m| of a Gamma(1 + m eps) at scope 1-4: {largest}");
    assert_eq!(largest, 5, "T_ins(j1, j2) with j1 + j2 = 3 at four loops");
    assert!(largest <= P_GAMMA_TAG_MAX_M);
}

/// Step (4): every Forcer anchor, the 17 four-loop ones (each master of the
/// four-loop basis at Forcer's definition, the five "XXX: normalization?"
/// lines and t155 among them, and the tops la4, no3, bubu) and the 11 of
/// 1L-3L, equals Forcer exactly on the scope 1-4 program, under Exact,
/// ModularPolynomial and ModularSampled.
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn every_forcer_anchor_holds_exactly_on_the_scope_four_library() {
    let library = mapped_four();
    let c = library.coefficient_context();
    let others = [
        ("ModularPolynomial", modular_polynomial()),
        ("ModularSampled", modular_sampled()),
    ];
    let mut checked = Vec::new();
    for json in [ANCHORS_JSON, ANCHORS_4L_JSON] {
        for case in parse_fixture(json).unwrap().cases {
            let start = Instant::now();
            let top = (library.top_presentation(&case.family))
                .unwrap_or_else(|| panic!("{}: family {}", case.name, case.family));
            let presentation = library.presentation(top);
            let inputs = case.inputs(c, presentation).unwrap();
            let expected = case.expected(c, presentation.loops).unwrap();
            assert!(!expected.is_empty(), "{}", case.name);
            let what = format!("anchor {}", case.name);
            assert_eq!(
                onto_masters_in(library, &inputs, &others, &what),
                expected,
                "{what}"
            );
            println!(
                "{what} (L{}): {} terms, {:.2} s for three arithmetics",
                presentation.loops,
                expected.len(),
                start.elapsed().as_secs_f64()
            );
            checked.push(case.name);
        }
    }
    assert_eq!(checked.len(), 11 + 17);
    for name in [
        "no2", "no6", "nono", "nostar5", "nostar6", "t155", "t124", "t145",
    ] {
        assert!(checked.iter().any(|c| c == name), "{name}");
    }
}

/// One row of the rules-study M_MAP copy.
struct MMapRow<'a> {
    id: &'a str,
    crossed: bool,
    forcer: &'a str,
    graph: PTopo,
}

/// The rows of `basis_anchors/m_map.tsv`, each with its integer graph (the
/// crossed line's eps unit dropped) on the external vertices `S`, `T`.
fn m_map() -> Vec<MMapRow<'static>> {
    let mut lines = M_MAP_TSV.lines().filter(|line| !line.starts_with('#'));
    assert_eq!(
        lines.next(),
        Some("id\tcrossed\tparent\tparent_forcer_or_action\tmasters\tggppss\tS\tT\tedges")
    );
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let [id, crossed, _, forcer, _, _, s, t, edges] = fields[..] else {
                panic!("m_map.tsv: {line}");
            };
            let mut vertices: Vec<u32> = vec![s.parse().unwrap(), t.parse().unwrap()];
            let mut graph = Vec::new();
            for edge in edges.split_whitespace() {
                let (a, b) = edge.trim_end_matches('*').split_once('-').unwrap();
                let mut id = |v: &str| {
                    let v: u32 = v.parse().unwrap();
                    let index = vertices.iter().position(|&w| w == v).unwrap_or_else(|| {
                        vertices.push(v);
                        vertices.len() - 1
                    });
                    u8::try_from(index).unwrap()
                };
                graph.push((id(a), id(b), 0));
            }
            let n = u8::try_from(vertices.len()).unwrap();
            MMapRow {
                id,
                crossed: !crossed.is_empty(),
                forcer,
                graph: PTopo::new(n, 0, 1, graph),
            }
        })
        .collect()
}

/// Step (5): M_MAP (rules study, 64/64) against the scope 1-4 program. Each
/// row's integer graph is a node of the program. Where M_MAP names a Forcer
/// master, that node is the master's topology and carries exactly one
/// residual, whose row is that master alone; on the 8 integer rows the corner
/// reduces onto `(-1)^lines Master(name)` exactly (`idx = (1/eps)
/// Master(name)` in the G scheme: the corner's `C_G` is `Master(name)`).
/// Where it names bubu (a knock-out), a triangle or a diamond, the node
/// carries no residual (and has that Forcer action). The crossed rows' values
/// (`(1/eps) I(corner, 0+eps)`, an eps line on a four-loop graph) are of scope
/// 1-5.
#[test]
#[ignore = "slow: needs the scope 1-4 package (RUSTRED_P_LIBRARY_1_4); optimised"]
fn m_map_names_the_masters_of_the_scope_four_library() {
    let library = mapped_four();
    let c = library.coefficient_context();
    let dag = library.topology().unwrap();
    let rows = m_map();
    assert_eq!(rows.len(), 64);
    let mut integer = BTreeSet::new();
    for row in &rows {
        let key = canonical(&row.graph).0;
        let node = dag
            .find(&key)
            .unwrap_or_else(|| panic!("{}: no node", row.id));
        assert_eq!(node.loops, 4, "{}", row.id);
        let residuals = residuals_on(library, node.index);
        if FORCER_MASTER_NAMES.contains(&row.forcer) {
            let m = definition(row.forcer);
            assert_eq!(key, m.graph_key().unwrap(), "{}: {}", row.id, row.forcer);
            let [id] = residuals[..] else {
                panic!("{}: residuals {residuals:?}", row.id);
            };
            let terms = &derivation_four().layer.rows()[id].terms;
            assert_eq!(terms.len(), 1, "{}", row.id);
            assert_eq!(terms[0].basis, master(row.forcer), "{}", row.id);
            if !row.crossed {
                let presentation = (library.presentations().iter())
                    .find(|p| p.node.as_ref().is_some_and(|r| r.index.0 == node.index))
                    .unwrap();
                let corner = IntegralKey::try_new(node_corner(presentation)).unwrap();
                let input = [(presentation.id, corner, c.one())];
                let lines = row.graph.line_count();
                assert_eq!(
                    onto_masters(library, &input, OrderedWeightArithmetic::Exact),
                    [PTerm {
                        basis: master(row.forcer),
                        tag: GammaTag::one(),
                        coefficient: c.integer(if lines % 2 == 0 { 1 } else { -1 }),
                    }],
                    "{}",
                    row.id
                );
                integer.insert(row.forcer);
            }
        } else {
            assert!(
                residuals.is_empty(),
                "{}: {} {residuals:?}",
                row.id,
                row.forcer
            );
            let action = match &node.action {
                Action::Triangle(_) => "triangle",
                Action::Diamond(_) => "diamond",
                Action::Custom => "bubu",
                other => panic!("{}: {other:?}", row.id),
            };
            assert_eq!(action, row.forcer, "{}", row.id);
        }
    }
    // The 8 four-loop masters without eps lines are M_MAP's integer rows.
    let four_loop: BTreeSet<&str> = (P_BASIS_MASTERS_1_TO_4.iter())
        .filter(|m| m.loops().unwrap() == 4)
        .map(|m| m.name)
        .collect();
    assert_eq!(integer, four_loop);
    println!(
        "M_MAP: 64/64 rows consistent, {} integer rows exact",
        integer.len()
    );
}

/// A random input of `family` (`lines` lines): the sector of all lines but 2
/// or 3 at power 1 with at most one dot, times `rank` scalar products, the
/// first `k_i.p` (so the sign of `q` matters), the others `k_i.k_j` or `k_i.p`.
fn random_case(
    family: &str,
    lines: usize,
    rank: u64,
    external: &str,
    rng: &mut SplitMix,
) -> ForcerCase {
    let mut powers = vec![1_i64; lines];
    for _ in 0..2 + rng.below(2) {
        let kept: Vec<usize> = (0..lines).filter(|&l| powers[l] == 1).collect();
        powers[kept[rng.below(kept.len() as u64) as usize]] = 0;
    }
    if rng.below(2) == 1 {
        let kept: Vec<usize> = (0..lines).filter(|&l| powers[l] == 1).collect();
        powers[kept[rng.below(kept.len() as u64) as usize]] = 2;
    }
    let factors: Vec<String> = (0..rank)
        .map(|factor| {
            let (i, j) = (rng.below(4) + 1, rng.below(5) + 1);
            if factor == 0 || j == 5 {
                format!("dot(k({i}),p(1))")
            } else {
                format!("dot(k({i}),k({j}))")
            }
        })
        .collect();
    ForcerCase {
        name: format!("{family} {powers:?} {factors:?} {external}"),
        family: family.to_owned(),
        powers,
        numerator: factors.join("*"),
        external: external.to_owned(),
        outputs: Vec::new(),
    }
}

/// Step (7) at a reduced bound: on each of the 11 four-loop families, one
/// nonzero input of rank 1 with `p(1) = q` and one of rank 3 with `p(1) = -q`
/// ([`random_case`]; zero draws are redrawn) reduce onto `Unit` and masters
/// only, equal under Exact and ModularSampled. Skipped (said so) without the
/// package.
#[test]
fn random_four_loop_inputs_reduce_onto_unit_and_masters_only() {
    if package().is_none() {
        eprintln!("SKIPPED: {PACKAGE} is unset (the scope 1-4 package of M3-T8c)");
        return;
    }
    let library = mapped_four();
    let c = library.coefficient_context();
    let others = [("ModularSampled", modular_sampled())];
    let mut rng = SplitMix::new(0x0004_b0a5_7017);
    let (mut families, mut masters, mut zero_draws) = (BTreeSet::new(), BTreeSet::new(), 0);
    for top in (0..library.presentations().len() as u32).map(PresentationId) {
        let presentation = library.presentation(top);
        let PresentationKind::Top { name } = &presentation.kind else {
            continue;
        };
        if presentation.loops != 4 {
            continue;
        }
        let lines = presentation
            .slots
            .iter()
            .filter(|s| matches!(s, Slot::Line(_)))
            .count();
        for (rank, external) in [(1, "p(1)"), (3, "-p(1)")] {
            let (case, terms) = (0..16)
                .find_map(|_| {
                    let start = Instant::now();
                    let case = random_case(name, lines, rank, external, &mut rng);
                    let inputs = case.inputs(c, presentation).unwrap();
                    let terms = onto_masters_in(library, &inputs, &others, &case.name);
                    let seconds = start.elapsed().as_secs_f64();
                    println!("{}: {} terms, {seconds:.2} s", case.name, terms.len());
                    zero_draws += usize::from(terms.is_empty());
                    (!terms.is_empty()).then_some((case.name, terms))
                })
                .unwrap_or_else(|| panic!("{name}: 16 zero draws"));
            for term in terms {
                match term.basis {
                    PBasis::Unit => {}
                    PBasis::Master(name) => {
                        masters.insert(name);
                    }
                    PBasis::Residual(id) => panic!("{case}: residual terminal {}", id.0),
                }
            }
        }
        families.insert(name.clone());
    }
    assert_eq!(families.len(), 11, "{families:?}");
    println!(
        "22 nonzero four-loop inputs ({zero_draws} zero draws) onto masters {masters:?}, no residual"
    );
}
