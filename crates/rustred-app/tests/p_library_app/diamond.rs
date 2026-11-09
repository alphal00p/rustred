//! The Diamond lane against the SpIRed owners (owner: M3-T5b rr-diamond; gate G3).
//!
//! Two scope 1-4 library programs from the same Provided Custom payloads: the
//! reference presents Forcer's four diamond nodes as Custom owners (their SpIRed
//! payloads, M3-T15), the other runs them on the Diamond lane. Everything below
//! the four nodes is the same program, so equal reductions of random keys of a
//! diamond sector (dots and numerators, exact in `Q(d)`, then compared at two
//! distinct `d`) are the template equal to the SpIRed owner on that node.
//!
//! The payloads come from `RUSTRED_P_REUSE_1_4`: a scope 1-4 package (M3-T8c's
//! `artefacts/m3/p-library-1-4-s1`) or anything `read_custom_payloads` reads.
//! Every test here is `#[ignore]` and runs optimised; unset, they panic with
//! this message, never pass silently.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

use rustred::algebra::Coefficient;
use rustred::family::IntegralKey;
use rustred::propagator::diamond::DiamondError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::lane::OwnerLane;
use rustred::propagator::library::{
    PBasis, PLibrary, PReduction, PReductionRequest, PTerminal, PUncoveredPolicy,
};
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot};
use rustred::propagator::tag::GammaTag;
use rustred::propagator::triangle::TriangleTemplate;
use rustred::propagator::{NodeIndex, PresentationId};
use rustred::reduction::{OrderedSchedulerOptions, OrderedWeightArithmetic, ReductionLimits};
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

fn reduce(library: &PLibrary, top: PresentationId, powers: &[i64]) -> PReduction {
    let context = library.coefficient_context();
    let key = IntegralKey::try_new(powers.iter().copied()).unwrap();
    library
        .reduce(PReductionRequest {
            inputs: vec![(top, key, context.one())],
            arithmetic: OrderedWeightArithmetic::Exact,
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

/// A reduction onto the terminals, each named by value (`Unit`, or a residual by
/// owner class and key), so that two libraries compare term by term.
fn named_terms(
    library: &PLibrary,
    reduction: &PReduction,
) -> BTreeMap<(String, GammaTag), Coefficient> {
    let context = library.coefficient_context();
    let mut terms: BTreeMap<(String, GammaTag), Coefficient> = BTreeMap::new();
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
/// some key is nonzero at both.
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
    for key in &keys {
        let start = Instant::now();
        let expected = named_terms(&reference.library, &reduce(&reference.library, top, key));
        seconds[0] += start.elapsed().as_secs_f64();
        let start = Instant::now();
        let found = named_terms(&diamond.library, &reduce(&diamond.library, top, key));
        seconds[1] += start.elapsed().as_secs_f64();
        assert_eq!(
            found, expected,
            "{name} {key:?}: the Diamond lane against the SpIRed owner"
        );
        for d in &dimensions {
            let values = |terms: &BTreeMap<(String, GammaTag), Coefficient>| -> Vec<Rational> {
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
         {nonzero} nonzero at both d, equal exactly; SpIRed owner {:.1} s, Diamond lane {:.1} s",
        top.0,
        keys.len(),
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
