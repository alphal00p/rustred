//! The 1L-3L basis layer on the assembled library (owner: M2-T9 rr-basis).
//!
//! `rustred::propagator::basis` on the libraries of `assemble_library`: the
//! scope 1-3 layer maps the residuals of NO and t1star05 onto Forcer's masters
//! `no` and `t105`, the Forcer anchors (kites, NO, the t1star05 parents incl.
//! the n = 0 chart) hold exactly in the (Gamma(1 + m eps) monomial x master)
//! basis, random inputs reduce onto masters without a residual, and
//! `rustred p-basis` writes the layer. The anchors are the core tests' copy of
//! vakint's fixture (`rustred-core/tests/p_library/basis_anchors/`).
//!
//! This module was registered in `main.rs` by M2-T9 (departure D-T9-1: the
//! anchors need the Custom owners, which only the app can generate).
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::family::IntegralKey;
use rustred::propagator::PresentationId;
use rustred::propagator::basis::anchors::{ForcerCase, parse_fixture};
use rustred::propagator::basis::{
    FORCER_MASTER_NAMES, P_BASIS_MASTERS_1_TO_3, PBasisDerivation, PBasisLayer, PBasisOptions,
    PBasisProvenance, derive_basis_layer, with_basis_layer, with_derived_basis_layer,
};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::library::{
    PBasis, PLibrary, PReductionRequest, PTerm, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot};
use rustred::propagator::tag::GammaTag;
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits,
};
use rustred_app::propagator::assemble::{
    AssembleOptions, AssembledLibrary, SplitMix, ValidateOptions, random_key, validation_keys,
};
use rustred_app::propagator::{CustomOwnerOptions, CustomPayloads, assemble_library};

const ANCHORS_JSON: &str =
    include_str!("../../../rustred-core/tests/p_library/basis_anchors/p_anchors.json");

fn assemble(max_loops: u8) -> AssembledLibrary {
    let families: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    assemble_library(
        &families,
        max_loops,
        CustomPayloads::Generate(CustomOwnerOptions::default()),
        &AssembleOptions::default(),
    )
    .unwrap()
}

fn scope_two() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| assemble(2))
}

fn scope_three() -> &'static AssembledLibrary {
    static LIBRARY: OnceLock<AssembledLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| assemble(3))
}

fn derivation_three() -> &'static PBasisDerivation {
    static DERIVATION: OnceLock<PBasisDerivation> = OnceLock::new();
    DERIVATION.get_or_init(|| {
        derive_basis_layer(&scope_three().library, &PBasisOptions::default()).unwrap()
    })
}

/// The scope 1-3 library with its basis layer.
fn mapped_three() -> &'static PLibrary {
    static LIBRARY: OnceLock<PLibrary> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        with_basis_layer(
            &scope_three().library,
            Arc::new(derivation_three().layer.clone()),
        )
        .unwrap()
    })
}

fn coefficient(c: &CoefficientContext, text: &str) -> Coefficient {
    rustred::propagator::basis::parse_coefficient(c, text, Default::default()).unwrap()
}

/// `(eps G(1,1))^loops` without `(d - 3)^-loops`.
fn scheme(loops: i16) -> GammaTag {
    GammaTag::from_pairs([(1, loops), (-1, 2 * loops), (-2, -loops)]).unwrap()
}

fn modular() -> OrderedWeightArithmetic {
    OrderedWeightArithmetic::ModularPolynomial(
        ModularCommon {
            threads: 1,
            ..Default::default()
        },
        PolynomialOptions::default(),
    )
}

fn reduce(
    library: &PLibrary,
    inputs: Vec<(PresentationId, IntegralKey, Coefficient)>,
    arithmetic: OrderedWeightArithmetic,
    onto_masters: bool,
) -> Result<Vec<PTerm>, PError> {
    Ok(library
        .reduce(PReductionRequest {
            inputs,
            arithmetic,
            scheduler: OrderedSchedulerOptions::default(),
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters,
            limits: ReductionLimits::default(),
            inspector: None,
        })?
        .terms
        .into_iter()
        .filter(|term| !term.coefficient.is_zero())
        .collect())
}

fn master(name: &str) -> PBasis {
    PBasis::Master(name.to_owned())
}

#[test]
fn scope_one_to_three_maps_its_two_residuals_onto_no_and_t105() {
    let library = &scope_three().library;
    let c = library.coefficient_context();
    let derivation = derivation_three();
    assert!(derivation.absent.is_empty());
    // The residual terminals of the library: exactly NO's and t1star05's.
    let residuals: Vec<_> = (library.terminals().iter())
        .filter(|t| matches!(t, PTerminal::Residual { .. }))
        .collect();
    assert_eq!(residuals.len(), 2);
    assert_eq!(library.terminals().len(), 3);

    // Each master sits on its Custom node and reduces to one residual, weight 1.
    let sites: Vec<_> = derivation.sites.iter().map(|s| s.master.name).collect();
    assert_eq!(sites, ["no", "t105"]);
    for site in &derivation.sites {
        let presentation = library.presentation(site.presentation);
        let expected_kind = if site.master.name == "no" {
            matches!(presentation.kind, PresentationKind::Induced { .. })
        } else {
            presentation.kind == PresentationKind::EpsNode
        };
        assert!(
            expected_kind,
            "{}: {:?}",
            site.master.name, presentation.kind
        );
        let [reduced] = site.reduction.as_slice() else {
            panic!("{}: {:?}", site.master.name, site.reduction);
        };
        assert!(matches!(reduced.basis, PBasis::Residual(_)));
        assert!(reduced.tag.is_one());
        assert_eq!(reduced.coefficient, c.one());
        let PBasis::Residual(id) = reduced.basis else {
            unreachable!()
        };
        let PTerminal::Residual { key, .. } = &library.terminals()[id.0 as usize] else {
            panic!("terminal {} is not a residual", id.0);
        };
        assert_eq!(
            key.powers(),
            site.key,
            "the residual is the master integral"
        );
    }
    assert_eq!(
        derivation.sites[0].key,
        [1, 1, 1, 1, 1, 1, 1, 1, 0],
        "NO(1^8, 0)"
    );
    assert_eq!(
        derivation.sites[1].key,
        [1, 1, 1, 1, 1],
        "t1star05(1,1,1,1,1+eps)"
    );

    // Rows: Unit (identity), then one master term per residual with the pinned
    // signs: I_M(t1star05) = -(eps G)^2 t105, I_M(NO) = +(eps G)^3 no.
    let layer = &derivation.layer;
    assert_eq!(layer.rows().len(), 3);
    assert_eq!(layer.rows()[0].provenance, PBasisProvenance::Identity);
    for row in &layer.rows()[1..] {
        assert_eq!(row.provenance, PBasisProvenance::Rustred);
        let [term] = row.terms.as_slice() else {
            panic!("{row:?}");
        };
        let PBasis::Master(name) = &term.basis else {
            panic!("{row:?}");
        };
        assert!(FORCER_MASTER_NAMES.contains(&name.as_str()));
        let (loops, coefficient_text) = match name.as_str() {
            "t105" => (2, "-1/(d-3)^2"),
            "no" => (3, "1/(d-3)^3"),
            other => panic!("master {other}"),
        };
        assert_eq!(term.tag, scheme(loops));
        assert_eq!(term.coefficient, coefficient(c, coefficient_text));
    }
    assert_eq!(layer.masters(), ["no", "t105"]);

    // Fresh-d checks modulo 2^61 - 1.
    assert_eq!(derivation.checks.len(), 2);
    assert!(derivation.checks.iter().all(|check| check.components == 2));
    layer.check_library(library).unwrap();
}

#[test]
fn scope_one_to_two_has_only_the_unit_row_and_its_anchors_hold() {
    let library = &scope_two().library;
    let (mapped, derivation) =
        with_derived_basis_layer(library, &PBasisOptions::default()).unwrap();
    assert_eq!(derivation.absent, ["no", "t105"]);
    assert!(derivation.sites.is_empty());
    assert_eq!(derivation.layer.rows().len(), 1);
    assert!(
        derivation.layer.rows()[0].terminal.node.is_some(),
        "the trivial node"
    );
    derivation.layer.check_library(library).unwrap();
    // The 1L and 2L anchors hold on the scope 1-2 program as well.
    let c = mapped.coefficient_context();
    let mut checked = 0;
    for case in anchor_cases() {
        let Some(top) = mapped.top_presentation(&case.family) else {
            continue;
        };
        let presentation = mapped.presentation(top);
        let inputs = case.inputs(c, presentation).unwrap();
        let expected = case.expected(c, presentation.loops).unwrap();
        let reduced = reduce(&mapped, inputs, OrderedWeightArithmetic::Exact, true).unwrap();
        assert_eq!(reduced, expected, "anchor {} at scope 1-2", case.name);
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn p_basis_fails_closed_without_a_square_system() {
    let library = &scope_three().library;
    for masters in [Vec::new(), vec![P_BASIS_MASTERS_1_TO_3[1]]] {
        let options = PBasisOptions {
            masters,
            ..PBasisOptions::default()
        };
        assert!(
            matches!(
                derive_basis_layer(library, &options),
                Err(PError::Unsupported(_))
            ),
            "{:?}",
            options.masters
        );
    }
    // A layer of another library does not attach.
    let two = derive_basis_layer(&scope_two().library, &PBasisOptions::default()).unwrap();
    assert!(matches!(
        with_basis_layer(library, Arc::new(two.layer)),
        Err(PError::MalformedLibrary(_))
    ));
}

#[test]
fn the_master_integrals_reduce_onto_their_signed_masters() {
    let library = mapped_three();
    let c = library.coefficient_context();
    assert_eq!(library.identity(), scope_three().library.identity());
    for site in &derivation_three().sites {
        let input = vec![(
            site.presentation,
            IntegralKey::try_new(site.key.iter().copied()).unwrap(),
            c.one(),
        )];
        // (eps G)^-L I_M(master) = sign Master(name) at Q^2 = 1.
        for arithmetic in [OrderedWeightArithmetic::Exact, modular()] {
            assert_eq!(
                reduce(library, input.clone(), arithmetic, true).unwrap(),
                [PTerm {
                    basis: master(site.master.name),
                    tag: GammaTag::one(),
                    coefficient: c.integer(i64::from(site.master.sign)),
                }],
                "{}",
                site.master.name
            );
        }
        // Onto terminals the mapped library is the library.
        assert_eq!(
            reduce(
                library,
                input.clone(),
                OrderedWeightArithmetic::Exact,
                false
            )
            .unwrap(),
            site.reduction
        );
    }
}

fn anchor_cases() -> Vec<ForcerCase> {
    parse_fixture(ANCHORS_JSON).unwrap().cases
}

#[test]
fn the_forcer_anchors_hold_exactly_on_the_assembled_library() {
    let library = mapped_three();
    let c = library.coefficient_context();
    let mut checked = Vec::new();
    for case in anchor_cases() {
        let top = library
            .top_presentation(&case.family)
            .unwrap_or_else(|| panic!("family {}", case.family));
        let presentation = library.presentation(top);
        let inputs = case.inputs(c, presentation).unwrap();
        let expected = case.expected(c, presentation.loops).unwrap();
        assert!(!expected.is_empty(), "{}", case.name);
        for arithmetic in [OrderedWeightArithmetic::Exact, modular()] {
            let reduced = reduce(library, inputs.clone(), arithmetic, true).unwrap();
            assert_eq!(reduced, expected, "anchor {}", case.name);
        }
        checked.push(case.name);
    }
    assert_eq!(checked.len(), 11);
}

#[test]
fn the_layer_of_scope_three_round_trips_through_json() {
    let library = &scope_three().library;
    let c = library.coefficient_context();
    let layer = &derivation_three().layer;
    let json = layer.to_json();
    let back = PBasisLayer::from_json(&json, c).unwrap();
    assert_eq!(&back, layer);
    back.check_library(library).unwrap();
    assert_eq!(back.to_json(), json);
    // Derivation is deterministic.
    let again = derive_basis_layer(library, &PBasisOptions::default()).unwrap();
    assert_eq!(again.layer.to_json(), json);
    // A writer reads the layer back from the mapped library's basis map.
    assert_eq!(PBasisLayer::from_library(library).unwrap(), None);
    assert_eq!(
        PBasisLayer::from_library(mapped_three()).unwrap().as_ref(),
        Some(layer)
    );
}

/// A random numerator input on top `top`: a random key's line powers (dots
/// <= 2 in total, random subsector) times `rank <= max_rank` random scalar
/// products `k_i.k_j`, `k_i.p` with `p = +-q`, as a fixture case.
fn numerator_case(p: &Presentation, max_rank: u32, rng: &mut SplitMix) -> ForcerCase {
    let dots = rng.below(3) as u32;
    let powers = random_key(p, dots, 0, rng);
    let lines: Vec<i64> = (p.slots.iter().zip(&powers))
        .filter(|(slot, _)| matches!(slot, Slot::Line(_)))
        .map(|(_, &power)| power)
        .collect();
    let loops = u64::from(p.loops);
    let rank = rng.below(u64::from(max_rank) + 1);
    let factors: Vec<String> = (0..rank)
        .map(|_| {
            let i = rng.below(loops) + 1;
            let j = rng.below(loops + 1) + 1;
            if j > loops {
                format!("dot(k({i}),p(1))")
            } else {
                format!("dot(k({i}),k({j}))")
            }
        })
        .collect();
    ForcerCase {
        name: format!("random {lines:?} {factors:?}"),
        family: String::new(),
        powers: lines,
        numerator: if factors.is_empty() {
            "1".to_owned()
        } else {
            factors.join("*")
        },
        external: if rng.below(2) == 0 { "p(1)" } else { "-p(1)" }.to_owned(),
        outputs: Vec::new(),
    }
}

/// Per loop count, random inputs reduce onto `Unit` and masters only: `count`
/// distinct keys of `rustred p-validate`'s generator (dots <= 3 and rank <=
/// `max_rank` in total, random subsectors), every top's corner (lines at 1,
/// ISPs at 0: the full NO sector is rare among random subsectors), and
/// `count / 4` numerator inputs lowered onto the slots. Returns
/// `(loops, inputs, seconds)` per loop count.
fn random_inputs_reduce_onto_masters_only(
    count: usize,
    max_rank: u32,
    batch: usize,
) -> Vec<(u8, usize, f64)> {
    let assembled = scope_three();
    let library = mapped_three();
    let c = library.coefficient_context();
    let mut report = Vec::new();
    for loops in 1..=3 {
        let start = Instant::now();
        let options = ValidateOptions {
            points: count,
            seed: 0x000b_4515 + u64::from(loops),
            max_dots: 3,
            max_rank,
            ..ValidateOptions::default()
        };
        let (mut keys, _) = validation_keys(assembled, loops, &options);
        assert!(
            keys.len() >= count.min(100),
            "L{loops}: {} keys",
            keys.len()
        );
        let tops = assembled.tops_of(loops);
        for &top in &tops {
            let corner = (library.presentation(top).slots.iter())
                .map(|slot| i64::from(matches!(slot, Slot::Line(_))))
                .collect();
            keys.push((top, corner));
        }
        let mut requests: Vec<Vec<(PresentationId, IntegralKey, Coefficient)>> = keys
            .chunks(batch)
            .map(|chunk| {
                (chunk.iter().enumerate())
                    .map(|(i, (top, powers))| {
                        (
                            *top,
                            IntegralKey::try_new(powers.iter().copied()).unwrap(),
                            c.integer(i as i64 + 1),
                        )
                    })
                    .collect()
            })
            .collect();
        let mut rng = SplitMix::new(0x000b_4515_7105 ^ u64::from(loops));
        let numerators = count / 4;
        for _ in 0..numerators {
            let top = tops[rng.below(tops.len() as u64) as usize];
            let case = numerator_case(library.presentation(top), max_rank, &mut rng);
            let inputs = case.inputs(c, library.presentation(top)).unwrap();
            if !inputs.is_empty() {
                requests.push(inputs);
            }
        }
        let mut masters = std::collections::BTreeSet::new();
        for inputs in requests {
            for term in reduce(library, inputs, OrderedWeightArithmetic::Exact, true).unwrap() {
                match term.basis {
                    PBasis::Unit => {}
                    PBasis::Master(name) => {
                        masters.insert(name);
                    }
                    PBasis::Residual(id) => panic!("L{loops}: residual terminal {}", id.0),
                }
            }
        }
        if loops == 3 {
            assert_eq!(
                masters.into_iter().collect::<Vec<_>>(),
                ["no", "t105"],
                "both masters are reached at 3L"
            );
        }
        let inputs = keys.len() + numerators;
        let seconds = start.elapsed().as_secs_f64();
        println!(
            "L{loops}: {inputs} inputs ({} keys incl. corners, {numerators} numerator inputs) onto masters, 0 residual terms, {seconds:.1} s",
            keys.len()
        );
        report.push((loops, inputs, seconds));
    }
    report
}

#[test]
fn random_inputs_reduce_onto_unit_and_masters_only() {
    let report = random_inputs_reduce_onto_masters_only(40, 4, 8);
    assert_eq!(report.len(), 3);
}

#[test]
#[ignore = "slow: 400 keys and 100 numerator inputs per loop count up to rank 8"]
fn random_inputs_reduce_onto_unit_and_masters_only_at_full_bounds() {
    let report = random_inputs_reduce_onto_masters_only(400, 8, 20);
    assert!(report.iter().all(|&(_, inputs, _)| inputs >= 300));
}

fn rustred() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rustred"))
}

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("rustred-p-basis-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn the_p_basis_command_derives_and_writes_the_layer() {
    let directory = scratch("cli");
    let out = directory.join("p-layer.json");
    let output = rustred()
        .args(["p-basis", "--max-loops", "3", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with(&scope_three().library.identity()));
    assert!(stdout.contains("master no (nostar0, sign +1)"), "{stdout}");
    assert!(
        stdout.contains("master t105 (t1star05, sign -1)"),
        "{stdout}"
    );
    assert_eq!(stdout.matches("fresh check: d = ").count(), 2, "{stdout}");
    assert!(
        stdout.contains("p basis: 3 rows, masters [\"no\", \"t105\"]"),
        "{stdout}"
    );
    // The layer of a fresh process is the in-process layer, byte for byte.
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        derivation_three().layer.to_json()
    );
    // An existing output needs --force.
    let again = rustred()
        .args(["p-basis", "--max-loops", "1", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!again.status.success());

    // Scope 1-2 has no residual: the Unit row only. Usage errors exit 2.
    let two = rustred()
        .args(["p-basis", "--max-loops", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&two.stdout);
    assert!(two.status.success(), "{stdout}");
    assert!(stdout.contains("masters without a node in this library: no, t105"));
    assert!(stdout.contains("p basis: 1 rows, masters []"), "{stdout}");
    let usage = rustred().args(["p-basis"]).output().unwrap();
    assert_eq!(usage.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&directory);
}
