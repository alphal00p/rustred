//! The Diamond lane against the SpIRed owners (owner: M3-T5b rr-diamond; gate G3).
//!
//! Two scope 1-4 library programs from the same Provided Custom payloads: the
//! reference presents Forcer's four diamond nodes as Custom owners (their SpIRed
//! payloads, M3-T15), the other runs them on the Diamond lane. Everything below
//! the four nodes is the same program, so equal reductions of random keys of a
//! diamond sector (dots and numerators, exact in `Q(d)`, then compared at two
//! distinct `d`) are the template equal to the SpIRed owner on that node.
//!
//! Forcer does not apply its diamond rule to no3: it overrides it with
//! `SchemeZno3` (forcer-rules.h:7). The no3 battery of M3-V7 checks the lane
//! against Forcer's values there.
//!
//! The payloads come from `RUSTRED_P_REUSE_1_4`: a scope 1-4 package (M3-T8c's
//! `artefacts/m3/p-library-1-4-s1`) or anything `read_custom_payloads` reads;
//! M3-V7's 4L Forcer fixtures from `RUSTRED_P4L_FORCER`. Every test here is
//! `#[ignore]` and runs optimised; unset, they panic with this message, never
//! pass silently.
//!
//! The reference side of the G3 comparison reduces under `Exact` by default;
//! `RUSTRED_G3_REFERENCE_ARITHMETIC=modular_polynomial` reduces it under
//! `OrderedWeightArithmetic::ModularPolynomial` instead (the SpIRed owner of
//! no3 is very slow under `Exact`): its terms are lifted over `Q(d)` by the
//! shared modular driver (CRT, rational reconstruction, a confirming prime,
//! a fresh verification prime), and the Diamond side stays `Exact`, so the
//! comparison is still exact in `Q(d)`.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use rustred::algebra::{Coefficient, ExactAlgebraLimits};
use rustred::family::IntegralKey;
use rustred::propagator::basis::anchors::{ForcerCase, PInput, parse_fixture};
use rustred::propagator::diamond::{DiamondError, DiamondStep};
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::lane::OwnerLane;
use rustred::propagator::library::{
    PBasis, PLibrary, PReduction, PReductionRequest, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot};
use rustred::propagator::step::{PKeyView, PStage};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::triangle::TriangleTemplate;
use rustred::propagator::{NodeIndex, PresentationId};
use rustred::reduction::{
    ModularCommon, OrderedSchedulerOptions, OrderedWeightArithmetic, PolynomialOptions,
    ReductionLimits,
};
use rustred_app::propagator::assemble::{SplitMix, assemble_library_checked};
use rustred_app::propagator::loader::read_custom_payloads;
use rustred_app::propagator::{
    AssembleOptions, AssembledLibrary, CustomPayloads, PLibrarySource, assemble_library,
};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::{Q, Rational};

/// Forcer's diamond nodes and their class labels (M3-T15 owners_1-4.tsv).
const DIAMONDS: [(&str, &str); 4] = [
    ("d384", "L4T09-fcbb654a0b4296f8"),
    ("d394", "L4T10-d1f7ffcd2b2f3614"),
    ("d399", "L4T10-d07d0fffbe48f1f2"),
    ("no3", "L4T11-b08ea0232a8b1503"),
];

/// Random keys per node (G3: at least 30).
const KEYS_PER_NODE: usize = 30;

fn reuse_directory() -> PathBuf {
    std::env::var_os("RUSTRED_P_REUSE_1_4")
        .map(PathBuf::from)
        .expect("RUSTRED_P_REUSE_1_4 must name the scope 1-4 Custom payloads (a package directory)")
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

/// The reference (diamonds as Custom owners) and the Diamond-lane library.
struct Pair {
    reference: AssembledLibrary,
    diamond: AssembledLibrary,
}

fn pair() -> &'static Pair {
    static PAIR: OnceLock<Pair> = OnceLock::new();
    PAIR.get_or_init(|| {
        let families: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
        let payloads = read_custom_payloads(&PLibrarySource::Directory(reuse_directory())).unwrap();
        for (name, label) in DIAMONDS {
            assert!(
                payloads.contains_key(label),
                "no SpIRed payload for {name} {label}"
            );
        }
        let triangle =
            |p: &Presentation, roles: &_| TriangleTemplate::extract(p, roles).map(|_| ());
        let as_custom = |p: &Presentation, _: &_| {
            Err(DiamondError::Mismatch {
                presentation: p.id,
                reason: "the G3 reference presents the diamonds as Custom owners".to_owned(),
            })
        };
        let start = Instant::now();
        let reference = assemble_library_checked(
            &families,
            4,
            CustomPayloads::Provided(payloads.clone()),
            &AssembleOptions {
                threads: threads(),
                verify_descent: false,
            },
            &triangle,
            &as_custom,
        )
        .unwrap();
        println!(
            "reference assembled in {:.1} s",
            start.elapsed().as_secs_f64()
        );
        let start = Instant::now();
        let diamond = assemble_library(
            &families,
            4,
            CustomPayloads::Provided(payloads),
            &AssembleOptions {
                threads: threads(),
                verify_descent: true,
            },
        )
        .unwrap();
        println!(
            "Diamond lane assembled and certified in {:.1} s: {:?}",
            start.elapsed().as_secs_f64(),
            diamond.timings
        );
        Pair { reference, diamond }
    })
}

fn owners_by_lane(library: &PLibrary) -> BTreeMap<OwnerLane, Vec<String>> {
    let census = library.census();
    let mut lanes: BTreeMap<OwnerLane, Vec<String>> = BTreeMap::new();
    for owner in library.owners() {
        let label = census.classes[&owner.spec.class].key.label();
        lanes.entry(owner.spec.lane).or_default().push(label);
    }
    lanes
}

fn diamond_labels() -> Vec<String> {
    let mut labels: Vec<String> = DIAMONDS
        .iter()
        .map(|(_, label)| (*label).to_owned())
        .collect();
    labels.sort();
    labels
}

/// G1's Diamond item: the four nodes own Diamond programs, certified, with no
/// fallback, and nothing else moved (21 Custom owners left); the reference keeps
/// them as Custom owners, the M3-T8c STAGE 1 state.
#[test]
#[ignore = "slow: two scope 1-4 assemblies (RUSTRED_P_REUSE_1_4)"]
fn the_diamond_lane_owns_forcers_four_diamonds() {
    let Pair { reference, diamond } = pair();
    let lanes = owners_by_lane(&diamond.library);
    let mut owned = lanes[&OwnerLane::Diamond].clone();
    owned.sort();
    assert_eq!(owned, diamond_labels());
    assert_eq!(lanes[&OwnerLane::Custom].len(), 21);
    let report = diamond.descent.as_ref().unwrap();
    assert_eq!(report.owners[&OwnerLane::Diamond], 4);
    assert!(diamond.nodes.iter().all(|node| node.fallback.is_none()));
    let reference_lanes = owners_by_lane(&reference.library);
    assert!(!reference_lanes.contains_key(&OwnerLane::Diamond));
    assert_eq!(reference_lanes[&OwnerLane::Custom].len(), 25);
    for lane in [
        OwnerLane::SeriesParallel,
        OwnerLane::Insertion,
        OwnerLane::Carpet,
        OwnerLane::Triangle,
    ] {
        assert_eq!(lanes[&lane], reference_lanes[&lane], "{lane:?}");
    }
    assert_eq!(
        diamond.library.terminals(),
        reference.library.terminals(),
        "the diamonds carry no residual"
    );
    println!(
        "owners by lane: {:?}",
        lanes
            .iter()
            .map(|(lane, owners)| (lane, owners.len()))
            .collect::<Vec<_>>()
    );
}

/// The top and sector of the reference's Custom presentation of a diamond node.
fn diamond_sector(reference: &AssembledLibrary, label: &str) -> (PresentationId, u64, NodeIndex) {
    let census = reference.library.census();
    let record = (census.classes.values())
        .find(|record| record.key.label() == label)
        .unwrap_or_else(|| panic!("no class {label}"));
    let node = record.node.unwrap();
    let presentation = reference.library.presentation(record.owner.0);
    let PresentationKind::Induced { top, mask } = presentation.kind else {
        panic!("{label}: {:?}", presentation.kind);
    };
    (top, mask, node)
}

/// A random key of the sector `mask` of `top`: its lines at `1 + dots`, the other
/// lines and the ISPs at powers `<= 0`, `rank` in total over them.
fn sector_key(top: &Presentation, mask: u64, dots: u32, rank: u32, rng: &mut SplitMix) -> Vec<i64> {
    let lines: Vec<usize> = (0..top.slots.len())
        .filter(|&s| mask >> s & 1 == 1)
        .collect();
    let others: Vec<usize> = (0..top.slots.len())
        .filter(|&s| mask >> s & 1 == 0)
        .collect();
    let mut powers: Vec<i64> = (0..top.slots.len())
        .map(|s| i64::from(mask >> s & 1 == 1))
        .collect();
    for _ in 0..dots {
        powers[lines[rng.below(lines.len() as u64) as usize]] += 1;
    }
    for _ in 0..rank {
        powers[others[rng.below(others.len() as u64) as usize]] -= 1;
    }
    assert!(
        top.slots
            .iter()
            .zip(&powers)
            .all(|(slot, &power)| matches!(slot, Slot::Line(_)) || power <= 0)
    );
    powers
}

fn reduce(
    library: &PLibrary,
    top: PresentationId,
    powers: &[i64],
    arithmetic: OrderedWeightArithmetic,
) -> PReduction {
    let context = library.coefficient_context();
    let key = IntegralKey::try_new(powers.iter().copied()).unwrap();
    reduce_with(library, vec![(top, key, context.one())], arithmetic)
}

fn reduce_inputs(library: &PLibrary, inputs: Vec<PInput>) -> PReduction {
    reduce_with(library, inputs, OrderedWeightArithmetic::Exact)
}

/// The arithmetic of the G3 reference side: `Exact`, or `ModularPolynomial`
/// (default options: lifted over `Q(d)` with a confirming prime and one fresh
/// verification prime) under `RUSTRED_G3_REFERENCE_ARITHMETIC=modular_polynomial`.
fn reference_arithmetic() -> OrderedWeightArithmetic {
    match std::env::var("RUSTRED_G3_REFERENCE_ARITHMETIC").as_deref() {
        Err(_) | Ok("exact") => OrderedWeightArithmetic::Exact,
        Ok("modular_polynomial") => OrderedWeightArithmetic::ModularPolynomial(
            ModularCommon {
                threads: threads(),
                ..ModularCommon::default()
            },
            PolynomialOptions::default(),
        ),
        Ok(other) => panic!("RUSTRED_G3_REFERENCE_ARITHMETIC={other}: exact or modular_polynomial"),
    }
}

fn reduce_with(
    library: &PLibrary,
    inputs: Vec<PInput>,
    arithmetic: OrderedWeightArithmetic,
) -> PReduction {
    library
        .reduce(PReductionRequest {
            inputs,
            arithmetic,
            scheduler: OrderedSchedulerOptions {
                threads: threads(),
                window_keys: 8,
                ..Default::default()
            },
            step_cache: None,
            uncovered: PUncoveredPolicy::Reject,
            onto_masters: false,
            limits: ReductionLimits::default(),
            inspector: None,
        })
        .unwrap()
}

/// Terms by terminal name (`Unit`, or a residual by owner class and key) and tag.
type Terms = BTreeMap<(String, GammaTag), Coefficient>;

/// A reduction onto the terminals, each named by value, so that two libraries
/// compare term by term.
fn named_terms(library: &PLibrary, reduction: &PReduction) -> Terms {
    let context = library.coefficient_context();
    let mut terms = Terms::new();
    for term in &reduction.terms {
        let name = match &term.basis {
            PBasis::Unit => "Unit".to_owned(),
            PBasis::Residual(id) => match &library.terminals()[id.0 as usize] {
                PTerminal::Unit => "Unit".to_owned(),
                PTerminal::Residual { owner, key } => {
                    format!("{:016x} {:?}", owner.0, key.powers())
                }
            },
            PBasis::Master(name) => panic!("a master {name} without a basis layer"),
        };
        let entry = terms
            .entry((name, term.tag.clone()))
            .or_insert_with(|| context.zero());
        *entry = context
            .try_add(entry, &term.coefficient, Default::default())
            .unwrap();
    }
    terms.retain(|_, value| !value.is_zero());
    terms
}

fn at(value: &Coefficient, d: &Rational) -> Rational {
    value.evaluate_with_coeff_map(|c| Rational::from(c.clone()), std::slice::from_ref(d), &Q)
}

/// G3 on one node: [`KEYS_PER_NODE`] distinct random keys of its sector, dots up
/// to 2 and numerator rank up to 3 in total (both present among the keys), reduce
/// to the same terminals with the same `Q(d)` coefficients through the Diamond
/// lane as through the SpIRed owner; the values agree at two distinct `d` and
/// some key is nonzero at both. One progress line per key (start and result).
fn template_equals_spired(name: &str, seed: u64) {
    let Pair { reference, diamond } = pair();
    let label = DIAMONDS.iter().find(|(n, _)| *n == name).unwrap().1;
    let (top, mask, node) = diamond_sector(reference, label);
    assert_eq!(
        diamond.nodes[node.0 as usize].decision.lane,
        OwnerLane::Diamond,
        "{name}"
    );
    let top_presentation = reference.library.presentation(top);
    let mut rng = SplitMix::new(seed);
    let mut keys: Vec<Vec<i64>> = Vec::new();
    while keys.len() < KEYS_PER_NODE {
        let dots = rng.below(3) as u32;
        let rank = rng.below(4) as u32;
        let key = sector_key(top_presentation, mask, dots, rank, &mut rng);
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    let dotted = keys
        .iter()
        .filter(|key| (0..key.len()).any(|s| mask >> s & 1 == 1 && key[s] > 1))
        .count();
    let numerators = keys
        .iter()
        .filter(|key| key.iter().any(|&power| power < 0))
        .count();
    assert!(
        dotted > 0 && numerators > 0,
        "{name}: {dotted} dotted, {numerators} with numerators"
    );
    let rational = |n: i64, d: i64| Rational::from((Integer::from(n), Integer::from(d)));
    let dimensions = [rational(1213, 101), rational(-307, 101)];
    let (mut nonzero, mut seconds) = (0, [0.0_f64; 2]);
    let arithmetic = reference_arithmetic();
    let begun = Instant::now();
    for (index, key) in keys.iter().enumerate() {
        println!(
            "G3 {name} key {}/{}: {key:?} start at {:.1} s (reference {})",
            index + 1,
            keys.len(),
            begun.elapsed().as_secs_f64(),
            arithmetic.name()
        );
        let start = Instant::now();
        let expected = named_terms(
            &reference.library,
            &reduce(&reference.library, top, key, arithmetic),
        );
        let reference_seconds = start.elapsed().as_secs_f64();
        seconds[0] += reference_seconds;
        let start = Instant::now();
        let found = named_terms(
            &diamond.library,
            &reduce(&diamond.library, top, key, OrderedWeightArithmetic::Exact),
        );
        let diamond_seconds = start.elapsed().as_secs_f64();
        seconds[1] += diamond_seconds;
        println!(
            "G3 {name} key {}/{}: SpIRed owner ({}) {reference_seconds:.1} s, Diamond lane (exact) \
             {diamond_seconds:.1} s, {} terms, equal {}",
            index + 1,
            keys.len(),
            arithmetic.name(),
            expected.len(),
            found == expected
        );
        assert_eq!(
            found, expected,
            "{name} {key:?}: the Diamond lane against the SpIRed owner"
        );
        for d in &dimensions {
            let values = |terms: &Terms| -> Vec<Rational> {
                terms.values().map(|value| at(value, d)).collect()
            };
            assert_eq!(
                values(&found),
                values(&expected),
                "{name} {key:?} at d = {d}"
            );
        }
        let nonzero_at = |d: &Rational| expected.values().any(|value| !at(value, d).is_zero());
        nonzero += usize::from(dimensions.iter().all(nonzero_at));
    }
    println!(
        "G3 {name} ({label}, top {}, sector {mask:#x}): {} keys ({dotted} dotted, {numerators} with numerators), \
         {nonzero} nonzero at both d, equal exactly; SpIRed owner ({}) {:.1} s, Diamond lane (exact) {:.1} s",
        top.0,
        keys.len(),
        arithmetic.name(),
        seconds[0],
        seconds[1]
    );
    assert!(nonzero > 0, "{name}: every key vanished");
}

#[test]
#[ignore = "slow: two scope 1-4 assemblies and 4L reductions (RUSTRED_P_REUSE_1_4)"]
fn the_diamond_lane_equals_spired_on_d384() {
    template_equals_spired("d384", 0x0384);
}

#[test]
#[ignore = "slow: two scope 1-4 assemblies and 4L reductions (RUSTRED_P_REUSE_1_4)"]
fn the_diamond_lane_equals_spired_on_d394() {
    template_equals_spired("d394", 0x0394);
}

#[test]
#[ignore = "slow: two scope 1-4 assemblies and 4L reductions (RUSTRED_P_REUSE_1_4)"]
fn the_diamond_lane_equals_spired_on_d399() {
    template_equals_spired("d399", 0x0399);
}

#[test]
#[ignore = "slow: two scope 1-4 assemblies and 4L reductions (RUSTRED_P_REUSE_1_4)"]
fn the_diamond_lane_equals_spired_on_no3() {
    template_equals_spired("no3", 0x0003);
}

/// G3 has teeth (review of M3-T5b): on d384's corner (every line at power 1, no
/// numerator) one Diamond step by hand, its `2 (L + 1)` children (Route keys, a
/// `b` or `c` at 0) reduced by the Diamond-lane library, whose owners below the
/// diamond are the reference's, rebuilds the SpIRed owner's value; with one
/// coefficient of the template mutated (`X_i + 1` on path `i`, or `E + 1`) it
/// does not, so G3 fails on a mutated template.
#[test]
#[ignore = "slow: two scope 1-4 assemblies (RUSTRED_P_REUSE_1_4)"]
fn a_mutated_diamond_coefficient_fails_g3() {
    let Pair { reference, diamond } = pair();
    let label = DIAMONDS[0].1;
    let (top, mask, _) = diamond_sector(reference, label);
    let arity = reference.library.presentation(top).slots.len();
    let corner: Vec<i64> = (0..arity).map(|s| i64::from(mask >> s & 1 == 1)).collect();
    let exact = OrderedWeightArithmetic::Exact;
    let expected = named_terms(
        &reference.library,
        &reduce(&reference.library, top, &corner, exact),
    );
    assert!(!expected.is_empty(), "d384's corner vanished");

    let library = &diamond.library;
    let census = library.census();
    let owner = (library.owners())
        .find(|owner| census.classes[&owner.spec.class].key.label() == label)
        .unwrap();
    let step = DiamondStep::from_owner(library, &owner.spec).unwrap();
    let p = step.presentation();
    let lines = (p.slots.iter())
        .filter(|slot| matches!(slot, Slot::Line(_)))
        .count();
    let powers: Vec<i64> = (0..p.slots.len()).map(|s| i64::from(s < lines)).collect();
    assert_eq!(
        named_terms(library, &reduce(library, p.id, &powers, exact)),
        expected,
        "the corner of the owner presentation against the corner of the top"
    );
    let tag = GammaTag::one();
    let key = PKeyView {
        presentation: p.id,
        powers: &powers,
        tag: &tag,
    };
    let output = step.apply(&key, &ReductionLimits::default()).unwrap();
    let paths = step.template().roles().paths();
    assert_eq!(output.children.len(), 2 * paths);
    assert!(output.children.iter().all(|c| c.stage == PStage::Route));
    let children: Vec<Terms> = (output.children.iter())
        .map(|child| named_terms(library, &reduce(library, p.id, &child.powers, exact)))
        .collect();
    let context = library.coefficient_context();
    let combine = |weights: &[Coefficient]| -> Terms {
        let mut terms = Terms::new();
        for (weight, child) in weights.iter().zip(&children) {
            for (name, value) in child {
                let value = context.try_mul(weight, value, Default::default()).unwrap();
                let entry = terms.entry(name.clone()).or_insert_with(|| context.zero());
                *entry = context.try_add(entry, &value, Default::default()).unwrap();
            }
        }
        terms.retain(|_, value| !value.is_zero());
        terms
    };
    let weights: Vec<Coefficient> = (output.children.iter())
        .map(|child| child.coefficient.clone())
        .collect();
    assert_eq!(
        combine(&weights),
        expected,
        "one Diamond step, true template"
    );

    let limits = ExactAlgebraLimits::default();
    let template = step.template();
    let pivot = template.pivot().at(context, &powers, limits).unwrap();
    let one = context.one();
    let plus_one = |value: &Coefficient| context.try_add(value, &one, limits).unwrap();
    let over =
        |value: &Coefficient, pivot: &Coefficient| context.try_div(value, pivot, limits).unwrap();
    let mut caught = Vec::new();
    for path in 0..paths {
        let apex = template.apex()[path].at(context, &powers, limits).unwrap();
        let weight = over(&plus_one(&apex), &pivot);
        let mut mutated = weights.clone();
        mutated[2 * path] = weight.clone();
        mutated[2 * path + 1] = context.try_neg(&weight, limits).unwrap();
        caught.push((format!("X_{path} + 1"), combine(&mutated) != expected));
    }
    let shifted = plus_one(&pivot);
    let rescaled: Vec<Coefficient> = (weights.iter())
        .map(|weight| over(&context.try_mul(weight, &pivot, limits).unwrap(), &shifted))
        .collect();
    caught.push(("E + 1".to_owned(), combine(&rescaled) != expected));
    println!(
        "G3 mutation on d384's corner ({} children, {} terms): caught {caught:?}",
        children.len(),
        expected.len()
    );
    assert!(caught.iter().all(|(_, caught)| *caught), "{caught:?}");
}

// ---------------------------------------------------------------------------
// What the lane buys on vakint's 4L benchmark cells
// ---------------------------------------------------------------------------

/// The 11 tops of vakint's 4L benchmark cells (benchmark_p_integrals.json).
const BENCHMARK_TOPS: [&str; 11] = [
    "P4L_LA4", "P4L_J1", "P4L_J2", "P4L_J4", "P4L_J5", "P4L_J6", "P4L_J11", "P4L_HAHA", "P4L_NO1",
    "P4L_NO2", "P4L_NO3",
];

/// The tops of the benchmark cells that reach a diamond node: P4L_NO3 is no3,
/// d384 and d394 are sectors of P4L_NO2, d399 of P4L_J11.
const BENCHMARK_DIAMOND_TOPS: [&str; 3] = ["P4L_NO2", "P4L_J11", "P4L_NO3"];

/// The line powers of a numerator-free 4L benchmark cell (as vakint lowers it:
/// the same powers on every top, the ISPs at 0).
fn benchmark_powers(cell: &str) -> Vec<i64> {
    let mut powers = vec![1; 11];
    match cell {
        "L4_d0_r0" => {}
        "L4_d1_r0" => powers[0] = 2,
        "L4_d2_r0" => (powers[0], powers[10]) = (2, 2),
        other => panic!("{other}: L4_d0_r0, L4_d1_r0 or L4_d2_r0"),
    }
    powers
}

fn benchmark_inputs(library: &PLibrary, tops: &[&str], powers: &[i64]) -> Vec<PInput> {
    let context = library.coefficient_context();
    (tops.iter())
        .map(|family| {
            let top = (library.top_presentation(family)).unwrap_or_else(|| panic!("top {family}"));
            let arity = library.presentation(top).slots.len();
            let key = (0..arity).map(|slot| powers.get(slot).copied().unwrap_or(0));
            (top, IntegralKey::try_new(key).unwrap(), context.one())
        })
        .collect()
}

fn arithmetic_named(name: &str) -> OrderedWeightArithmetic {
    match name {
        "exact" => OrderedWeightArithmetic::Exact,
        "modular_polynomial" => OrderedWeightArithmetic::ModularPolynomial(
            ModularCommon {
                threads: threads(),
                ..ModularCommon::default()
            },
            PolynomialOptions::default(),
        ),
        other => panic!("{other}: exact or modular_polynomial"),
    }
}

/// What the Diamond lane buys at 4L on vakint's benchmark cells, each of which
/// reaches the diamond nodes: every `cell:arithmetic` of
/// `RUSTRED_P4L_BENCH_CELLS` (default `L4_d0_r0:exact,L4_d0_r0:modular_polynomial,
/// L4_d2_r0:modular_polynomial`) reduced as one request through the reference
/// (the diamonds as SpIRed Custom owners) and through the Diamond lane, twice
/// each (the second run has the kernel memos of the first), then per top on the
/// three tops with a diamond. The terms are equal; the times are a
/// measurement (indicative, no bound).
#[test]
#[ignore = "measurement: two scope 1-4 assemblies and 4L benchmark cells (RUSTRED_P_REUSE_1_4)"]
fn benchmark_cells_through_the_diamond_lane_and_the_spired_owners() {
    let Pair { reference, diamond } = pair();
    let cells = std::env::var("RUSTRED_P4L_BENCH_CELLS").unwrap_or_else(|_| {
        "L4_d0_r0:exact,L4_d0_r0:modular_polynomial,L4_d2_r0:modular_polynomial".to_owned()
    });
    for item in cells.split(',') {
        let (cell, arithmetic) = item.split_once(':').unwrap();
        let powers = benchmark_powers(cell);
        let arithmetic = arithmetic_named(arithmetic);
        let mut runs = Vec::new();
        for pass in ["first", "second"] {
            for (side, library) in [
                ("SpIRed owners", &reference.library),
                ("Diamond lane", &diamond.library),
            ] {
                let inputs = benchmark_inputs(library, &BENCHMARK_TOPS, &powers);
                let start = Instant::now();
                let reduction = reduce_with(library, inputs, arithmetic);
                let seconds = start.elapsed().as_secs_f64();
                let terms = named_terms(library, &reduction);
                println!(
                    "BENCH {cell} {} t{} {pass} {side}: {seconds:.3} s, {} input keys, {} terminal keys, {} terms",
                    arithmetic.name(),
                    threads(),
                    reduction.statistics.progress.input_keys,
                    reduction.statistics.progress.terminal_keys,
                    terms.len()
                );
                runs.push(terms);
            }
        }
        assert!(
            runs.iter().all(|terms| *terms == runs[0]),
            "{cell}: the programs differ"
        );
        for family in BENCHMARK_DIAMOND_TOPS {
            let mut tops = Vec::new();
            for (side, library) in [
                ("SpIRed owners", &reference.library),
                ("Diamond lane", &diamond.library),
            ] {
                let inputs = benchmark_inputs(library, &[family], &powers);
                let start = Instant::now();
                let terms = named_terms(library, &reduce_with(library, inputs, arithmetic));
                println!(
                    "BENCH {cell} {} t{} top {family} {side}: {:.3} s, {} terms",
                    arithmetic.name(),
                    threads(),
                    start.elapsed().as_secs_f64(),
                    terms.len()
                );
                tops.push(terms);
            }
            assert_eq!(tops[0], tops[1], "{cell} {family}");
        }
    }
}

// ---------------------------------------------------------------------------
// no3 against Forcer (SchemeZno3)
// ---------------------------------------------------------------------------

/// vakint's 1L-3L Forcer anchors (the masters `no` and `t105`).
const P_ANCHORS_JSON: &str =
    include_str!("../../../rustred-core/tests/p_library/basis_anchors/p_anchors.json");

/// The cases of one of M3-V7's frozen 4L Forcer fixtures (`$WS/artefacts/m3/
/// vk-fixtures4`, gammaloop `crates/vakint/tests/fixtures/forcer/`).
fn four_loop_forcer_cases(file: &str) -> Vec<ForcerCase> {
    let directory = std::env::var_os("RUSTRED_P4L_FORCER")
        .map(PathBuf::from)
        .expect("RUSTRED_P4L_FORCER must name M3-V7's 4L Forcer fixtures (p4l_anchors.json, p4l_battery.json)");
    let text = std::fs::read_to_string(directory.join(file)).unwrap();
    parse_fixture(&text).unwrap().cases
}

fn case_loops(library: &PLibrary, case: &ForcerCase) -> (PresentationId, u8) {
    let top = (library.top_presentation(&case.family))
        .unwrap_or_else(|| panic!("{}: no family {}", case.name, case.family));
    (top, library.presentation(top).loops)
}

/// A Forcer case reduced by `library` onto its terminals, in Forcer's
/// normalisation: the reduction (convention C1 at `Q^2 = 1`) times
/// `(eps G(1,1))^-L`, as `ForcerCase::expected` (convention C3, basis.rs).
fn case_terms(library: &PLibrary, case: &ForcerCase) -> Terms {
    let (top, loops) = case_loops(library, case);
    let context = library.coefficient_context();
    let inputs = case.inputs(context, library.presentation(top)).unwrap();
    in_g_scheme(
        library,
        named_terms(library, &reduce_inputs(library, inputs)),
        loops,
    )
}

/// `(eps G(1,1))^-loops` times `terms`: per loop the tag
/// `{(+1,-1),(-1,-2),(-2,+1)}` and the factor `d - 3` (basis.rs `scheme_tag`).
fn in_g_scheme(library: &PLibrary, terms: Terms, loops: u8) -> Terms {
    let context = library.coefficient_context();
    let l = i16::from(loops);
    let shift = GammaTag::from_pairs([(1, -l), (-1, -2 * l), (-2, l)]).unwrap();
    let d = context.parameter("d").unwrap();
    let d_minus_3 = context
        .try_sub(&d, &context.integer(3), Default::default())
        .unwrap();
    let mut factor = context.one();
    for _ in 0..loops {
        factor = context
            .try_mul(&factor, &d_minus_3, Default::default())
            .unwrap();
    }
    (terms.into_iter())
        .map(|((name, tag), value)| {
            let value = context
                .try_mul(&value, &factor, Default::default())
                .unwrap();
            ((name, tag.add(&shift)), value)
        })
        .collect()
}

/// A Forcer master in a library's terminals: an anchor whose Forcer value is
/// `c T Master(name)` alone, with `c`, `T` and the anchor's reduction (in
/// Forcer's normalisation, so anchors of fewer loops than the case serve).
struct MasterImage {
    anchor: String,
    tag: GammaTag,
    coefficient: Coefficient,
    terms: Terms,
}

/// The images of the masters in Forcer's values of `cases`.
fn master_images(
    library: &PLibrary,
    anchors: &[ForcerCase],
    cases: &[&ForcerCase],
) -> BTreeMap<String, MasterImage> {
    let context = library.coefficient_context();
    let mut needed = BTreeSet::new();
    for case in cases {
        let (_, loops) = case_loops(library, case);
        for term in case.expected(context, loops).unwrap() {
            if let PBasis::Master(name) = term.basis {
                needed.insert(name);
            }
        }
    }
    let mut images = BTreeMap::new();
    for case in anchors {
        let (_, loops) = case_loops(library, case);
        let expected = case.expected(context, loops).unwrap();
        let [term] = expected.as_slice() else {
            continue;
        };
        let PBasis::Master(name) = &term.basis else {
            continue;
        };
        if needed.contains(name) && !images.contains_key(name) {
            let terms = case_terms(library, case);
            assert!(!terms.is_empty(), "the anchor {} vanished", case.name);
            images.insert(
                name.clone(),
                MasterImage {
                    anchor: case.name.clone(),
                    tag: term.tag.clone(),
                    coefficient: term.coefficient.clone(),
                    terms,
                },
            );
        }
    }
    images
}

/// Forcer's value of `case` with every `Master(name)` replaced by its image:
/// the terms that `library` gives if it agrees with Forcer.
fn forcer_terms(
    library: &PLibrary,
    case: &ForcerCase,
    images: &BTreeMap<String, MasterImage>,
) -> Terms {
    let context = library.coefficient_context();
    let (_, loops) = case_loops(library, case);
    let mut terms = Terms::new();
    let mut add = |key: (String, GammaTag), value: Coefficient| {
        let entry = terms.entry(key).or_insert_with(|| context.zero());
        *entry = context.try_add(entry, &value, Default::default()).unwrap();
    };
    for term in case.expected(context, loops).unwrap() {
        match &term.basis {
            PBasis::Unit => add(("Unit".to_owned(), term.tag), term.coefficient),
            PBasis::Master(name) => {
                let image = (images.get(name))
                    .unwrap_or_else(|| panic!("{}: no anchor is Master({name}) alone", case.name));
                let scale = context
                    .try_div(&term.coefficient, &image.coefficient, Default::default())
                    .unwrap();
                let shift = term.tag.add(&image.tag.neg());
                for ((terminal, tag), value) in &image.terms {
                    let value = context.try_mul(&scale, value, Default::default()).unwrap();
                    add((terminal.clone(), tag.add(&shift)), value);
                }
            }
            PBasis::Residual(id) => panic!("{}: a residual {id:?} in Forcer's value", case.name),
        }
    }
    terms.retain(|_, value| !value.is_zero());
    terms
}

/// G3's Forcer item. Forcer reduces no3 with `SchemeZno3`, the Diamond lane
/// with the diamond rule: on M3-V7's no3 battery (the top, a numerator of
/// rank 1 and of rank 2, a dot) the lane's reduction, in Forcer's
/// normalisation, equals Forcer's value exactly in `Q(d)`, and at two `d`,
/// once each Forcer master is replaced by the reduction of its Forcer anchor.
/// The bubu top (a Custom owner, masters bebe, cross, t145) checks the
/// replacement itself.
#[test]
#[ignore = "slow: two scope 1-4 assemblies and 4L reductions (RUSTRED_P_REUSE_1_4, RUSTRED_P4L_FORCER)"]
fn the_diamond_lane_agrees_with_forcer_on_no3() {
    let Pair { diamond, .. } = pair();
    let library = &diamond.library;
    let anchors: Vec<ForcerCase> = (four_loop_forcer_cases("p4l_anchors.json").into_iter())
        .chain(parse_fixture(P_ANCHORS_JSON).unwrap().cases)
        .collect();
    let battery = four_loop_forcer_cases("p4l_battery.json");
    let mut cases: Vec<&ForcerCase> = battery.iter().filter(|c| c.family == "P4L_NO3").collect();
    let names: Vec<&str> = cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["no3_top", "no3_isp", "no3_rank3", "no3_dot"]);
    cases.push(anchors.iter().find(|c| c.name == "bubu_top").unwrap());
    let start = Instant::now();
    let images = master_images(library, &anchors, &cases);
    println!(
        "master images ({:.1} s): {:?}",
        start.elapsed().as_secs_f64(),
        (images.iter())
            .map(|(name, image)| (name, &image.anchor, image.terms.len()))
            .collect::<Vec<_>>()
    );
    let rational = |n: i64, d: i64| Rational::from((Integer::from(n), Integer::from(d)));
    let dimensions = [rational(1213, 101), rational(-307, 101)];
    for case in cases {
        let start = Instant::now();
        let found = case_terms(library, case);
        let expected = forcer_terms(library, case, &images);
        assert!(!expected.is_empty(), "{}", case.name);
        assert_eq!(
            found, expected,
            "{}: the Diamond-lane library against Forcer",
            case.name
        );
        for d in &dimensions {
            let values = |terms: &Terms| -> Vec<Rational> {
                terms.values().map(|value| at(value, d)).collect()
            };
            assert_eq!(
                values(&found),
                values(&expected),
                "{} at d = {d}",
                case.name
            );
            assert!(
                values(&found).iter().any(|v| !v.is_zero()),
                "{} at d = {d}",
                case.name
            );
        }
        println!(
            "Forcer {} ({}, {} {:?}): {} terms, equal exactly and at both d ({:.1} s)",
            case.name,
            case.family,
            case.numerator,
            case.powers,
            found.len(),
            start.elapsed().as_secs_f64()
        );
    }
}
