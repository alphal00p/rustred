//! Vacuum pins: identities and bytes that every p-integral change must leave
//! untouched (plan M2 section 2 rule 5, gate G6). Computed on rustred
//! `6e3a6f7e` before any p-library edit.
//!
//! - `binding.stamp()` of the three Ordered step-cache adapters (fixed
//!   fingerprints, context `{d}`), plus the version constants they digest;
//! - rustred-order canonical bytes of one v1 and one v2 descriptor;
//! - for small vacuum families (1L tadpole, 2L sunset, 2L with an ISP, and
//!   two programmed orders) a canonical text dump of the generated candidate
//!   rules, produced exactly as `family_candidates` does with default request
//!   settings (zero census over every mask, root downset, one worker). The
//!   dump lists sectors, order, rules in generation order (case, target,
//!   right-hand side, seed sources, exclusions) and finite residuals, with
//!   coefficients printed by Symbolica's `Display`. Its BLAKE3 is a test
//!   guard, never a cache identity, and is stable across processes.
//!
//! A deliberate change bumps the matching version constant and re-pins here;
//! the failure message prints every new value (and the dump text).
mod support;

use std::convert::Infallible;
use std::fmt::Write as _;

use rustred::algebra::CoefficientContext;
use rustred::order::{
    CompiledOrder, CoordinateGroups, DegreeRow, Direction, Limits as OrderLimits, OrderDescriptor,
};
use rustred::reduction::{
    ORDERED_STEP_CACHE_SCHEMA, ORDERED_STEP_SEMANTICS, ORDERED_TAIL_SCHEMA, ORDERED_TAIL_SEMANTICS,
    OrderedStepCacheAdapter, OrderedStepCacheBinding,
};
use rustred::sector::{Mask, zero};
use rustred::solver::{
    Case, SectorCompleted, SectorConfig, SectorExecutor, SectorSolveOptions, SourceSystem,
    UnresolvedPolicy,
};

use support::p_integrals::compile;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// Compare every `(name, actual, expected)` and fail once, listing all
/// mismatches with their new values.
fn check(pins: &[(&str, String, &str)]) {
    let mismatches: Vec<_> = pins
        .iter()
        .filter(|(_, actual, expected)| actual != expected)
        .map(|(name, actual, expected)| {
            format!("{name}:\n  expected {expected}\n  actual   {actual}")
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "vacuum pins changed:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn ordered_step_cache_versions_and_binding_stamps_are_pinned() {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let stamp = |adapter, fingerprint: &str, policy| {
        hex(&OrderedStepCacheBinding::new(adapter, fingerprint, &context, policy).stamp())
    };
    check(&[
        (
            "ORDERED_STEP_CACHE_SCHEMA",
            ORDERED_STEP_CACHE_SCHEMA.to_owned(),
            "rustred.ordered-step-cache.v3",
        ),
        (
            "ORDERED_STEP_SEMANTICS",
            ORDERED_STEP_SEMANTICS.to_owned(),
            "rustred-ordered-steps-2026-10-31",
        ),
        (
            "ORDERED_TAIL_SCHEMA",
            ORDERED_TAIL_SCHEMA.to_owned(),
            "rustred.ordered-tail.v3",
        ),
        (
            "ORDERED_TAIL_SEMANTICS",
            ORDERED_TAIL_SEMANTICS.to_owned(),
            "rustred-ordered-tail-2026-10-31",
        ),
        (
            "closed-artifact stamp",
            stamp(
                OrderedStepCacheAdapter::ClosedArtifact,
                "vacuum-pin-closed-artifact",
                None,
            ),
            "58bca2732a625af1be4917b9ed22171164c9d05f73c433a2bb64da043da88499",
        ),
        (
            "candidate-program stamp (reject)",
            stamp(
                OrderedStepCacheAdapter::CandidateProgram,
                "vacuum-pin-candidate-program",
                Some(UnresolvedPolicy::Reject),
            ),
            "2bc4638c008309666cd42010ceebb5245fc8b18d40d86faa222b390c2740f69e",
        ),
        (
            "candidate-program stamp (preserve)",
            stamp(
                OrderedStepCacheAdapter::CandidateProgram,
                "vacuum-pin-candidate-program",
                Some(UnresolvedPolicy::Preserve),
            ),
            "fa34b5cbe75bf032275adf8cc6a5c008e9723ae4ab62ba3deb289af36d6af44f",
        ),
        (
            "saved-application stamp (reject)",
            stamp(
                OrderedStepCacheAdapter::SavedApplication,
                "vacuum-pin-saved-application",
                Some(UnresolvedPolicy::Reject),
            ),
            "3f64f0d930b7be06fe1e36b7a4979732eba278ed450fd375c9a454eda5493947",
        ),
    ]);
}

/// The support-primary v1 descriptor of the candidate-bundle order tests.
fn descriptor(arity: usize) -> OrderDescriptor {
    OrderDescriptor {
        pre_support_degree_rows: vec![],
        support_weights: vec![0; arity],
        support_priority: (0..arity).collect(),
        degree_rows: vec![
            DegreeRow {
                active: vec![1; arity],
                inactive: vec![1; arity],
            },
            DegreeRow {
                active: vec![0; arity],
                inactive: vec![1; arity],
            },
        ],
        coordinate_priority: (0..arity).collect(),
        coordinate_groups: CoordinateGroups::ActiveFirst,
        active_direction: Direction::Descending,
        inactive_direction: Direction::Descending,
    }
}

/// The same order with one absolute-degree prefix row (v2 encoding).
fn prefixed_descriptor(arity: usize) -> OrderDescriptor {
    OrderDescriptor {
        pre_support_degree_rows: vec![DegreeRow {
            active: vec![1; arity],
            inactive: vec![1; arity],
        }],
        ..descriptor(arity)
    }
}

fn compiled(descriptor: OrderDescriptor) -> CompiledOrder {
    CompiledOrder::compile(descriptor, OrderLimits::default()).unwrap()
}

#[test]
fn order_descriptor_v1_and_v2_bytes_are_pinned() {
    let v1 = compiled(descriptor(3));
    let v2 = compiled(prefixed_descriptor(3));
    assert_eq!(&v1.canonical_bytes()[..8], b"RRDORD01");
    assert_eq!(&v2.canonical_bytes()[..8], b"RRDORD02");
    for order in [&v1, &v2] {
        let decoded =
            CompiledOrder::from_canonical_bytes(order.canonical_bytes(), OrderLimits::default())
                .unwrap();
        assert_eq!(decoded.descriptor(), order.descriptor());
    }
    check(&[
        (
            "v1 descriptor bytes",
            hex(v1.canonical_bytes()),
            "5252444f5244303103000000000000000200000000000000000101000000000000000000000000000000000000000000000000000000000000000001000000000000000200000000000000000000000000000001000000000000000200000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000000000000000000000000000000000000000000000000000010000000000000001000000000000000100000000000000",
        ),
        (
            "v2 descriptor bytes",
            hex(v2.canonical_bytes()),
            "5252444f52443032030000000000000002000000000000000100000000000000000101000000000000000000000000000000000000000000000000000000000000000001000000000000000200000000000000000000000000000001000000000000000200000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000000000000000000000000000000000000000000000000000010000000000000001000000000000000100000000000000",
        ),
    ]);
}

/// Canonical text of the candidate rules generated for one vacuum family.
fn candidate_dump<const N: usize>(
    name: &str,
    source: &str,
    nonpositive: &[usize],
    order: Option<CompiledOrder>,
) -> String {
    let family = compile(source);
    assert_eq!(family.denominator_count(), N, "{name}: arity");
    let mut root = [true; N];
    for &axis in nonpositive {
        root[axis] = false;
    }
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let (mut zeros, mut sectors) = (Vec::new(), Vec::new());
    for bits in 0..1usize << N {
        let sector: [bool; N] = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        let mask = Mask::try_new(sector.iter().copied()).unwrap();
        match analyzer.analyze(&mask).unwrap() {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            zero::Decision::Inconclusive(_) => {
                if sector
                    .iter()
                    .zip(root)
                    .all(|(&active, allowed)| !active || allowed)
                {
                    sectors.push(sector);
                }
            }
            zero::Decision::Excluded(_) => panic!("{name}: unrestricted analysis excluded"),
        }
    }
    sectors.sort_unstable();
    let mask = |sector: &[bool; N]| -> String {
        sector.iter().map(|&a| if a { '1' } else { '0' }).collect()
    };
    let masks = |sectors: &[[bool; N]]| sectors.iter().map(mask).collect::<Vec<_>>().join(" ");
    let mut out = String::new();
    writeln!(out, "family {name}").unwrap();
    writeln!(out, "fingerprint {}", family.fingerprint()).unwrap();
    writeln!(out, "zeros {}", masks(&zeros)).unwrap();
    writeln!(out, "sectors {}", masks(&sectors)).unwrap();

    let sources = SourceSystem::from_family_with_capacity(&family, true).unwrap();
    let config = SectorConfig {
        zero_sectors: zeros.into(),
        integral_order: order,
        ..Default::default()
    };
    let render = |done: SectorCompleted<N>| -> Result<String, Infallible> {
        let solution = &done.solution;
        let mut text = String::new();
        let order = match solution.order.program() {
            Some(program) => format!("programmed {}", hex(program.canonical_bytes())),
            None => format!("{:?}", solution.order.persisted_policy().unwrap()),
        };
        writeln!(text, "sector {} order {order}", mask(&done.sector)).unwrap();
        writeln!(
            text,
            "  rank {:?} finite-policy {:?}",
            solution.max_numerator_rank, solution.finite_case_policy
        )
        .unwrap();
        for (index, rule) in solution.rules.iter().enumerate() {
            let candidate = &rule.candidate;
            let case = match &candidate.case {
                Case::Coordinate(case) => format!("coordinate {:?}", case.fixed()),
                Case::Affine(case) => format!(
                    "affine {:?} vars {:?} eqs [{}]",
                    case.face().fixed(),
                    case.index_variables(),
                    case.equations()
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            writeln!(text, "  rule {index} case {case}").unwrap();
            writeln!(
                text,
                "    target {} dispatch {:?}",
                candidate.target, rule.dispatch_policy
            )
            .unwrap();
            for term in &candidate.rhs {
                writeln!(text, "    rhs {} * ({})", term.integral, term.coefficient).unwrap();
            }
            for source in &candidate.sources {
                writeln!(
                    text,
                    "    source row {} seed {} shifts {:?}",
                    source.basis_row, source.seed.integral, source.seed.shifts
                )
                .unwrap();
            }
            for branch in &rule.exceptions.branches {
                let branch: Vec<_> = branch.iter().map(ToString::to_string).collect();
                writeln!(text, "    exclusion [{}]", branch.join(", ")).unwrap();
            }
        }
        for residual in &solution.finite_residuals {
            writeln!(text, "  residual {residual}").unwrap();
        }
        Ok(text)
    };
    let executor = SectorExecutor::new(1).unwrap();
    let solved = executor
        .map(
            &sources,
            &sectors,
            &config,
            SectorSolveOptions::default(),
            render,
        )
        .unwrap();
    for text in solved {
        out.push_str(&text);
    }
    out
}

const TADPOLE: &str = "I(loops(q),externals(),dimension(d),prop(D1,q^2-1,1))";
const SUNSET: &str = "I(loops(p,q),externals(),dimension(d),\
    prop(A,p^2-1,1),prop(B,q^2-1,1),prop(C,(p-q)^2-1,1))";
/// Two unit-mass lines and the massless `(p-q)^2` as an ISP (slot 2).
const SUNSET_ISP: &str = "I(loops(p,q),externals(),dimension(d),\
    prop(A,p^2-1,1),prop(B,q^2-1,1),prop(C,(p-q)^2,1))";

fn dumps() -> Vec<(&'static str, String)> {
    vec![
        (
            "tadpole",
            candidate_dump::<1>("tadpole", TADPOLE, &[], None),
        ),
        ("sunset", candidate_dump::<3>("sunset", SUNSET, &[], None)),
        (
            "sunset-isp",
            candidate_dump::<3>("sunset-isp", SUNSET_ISP, &[2], None),
        ),
        (
            "tadpole-programmed-v2",
            candidate_dump::<1>(
                "tadpole-programmed-v2",
                TADPOLE,
                &[],
                Some(compiled(prefixed_descriptor(1))),
            ),
        ),
        (
            "sunset-programmed-v1",
            candidate_dump::<3>(
                "sunset-programmed-v1",
                SUNSET,
                &[],
                Some(compiled(descriptor(3))),
            ),
        ),
    ]
}

#[test]
fn vacuum_candidate_rule_dumps_are_pinned() {
    let expected = [
        (
            "tadpole",
            "f9ae7a32e615376fe7e5bd243cf61e7fd962e4eb77bb7d5b22335955420b32fb",
        ),
        (
            "sunset",
            "5658d994d152c85f9e69a59ae350d3e89994a3b1c37e918bc61248c294f17844",
        ),
        (
            "sunset-isp",
            "57c882a3ab60068c1bd3f4329da83d2b9d72da35b70dfee40ff585d461d8c107",
        ),
        (
            "tadpole-programmed-v2",
            "b746ab14e9045d312601cab3936952d35d34bfe9649ee35ec76a20b6eb7321ad",
        ),
        (
            "sunset-programmed-v1",
            "97e45a61164140c4ffab1c16e68cbb07bc7ba3756da3904598b099467b37b7f1",
        ),
    ];
    let dumps = dumps();
    if let Ok(directory) = std::env::var("RUSTRED_VACUUM_PINS_DUMP_DIR") {
        for (name, text) in &dumps {
            std::fs::write(format!("{directory}/{name}.txt"), text).unwrap();
        }
    }
    let pins: Vec<_> = dumps
        .iter()
        .zip(expected)
        .map(|((name, text), (expected_name, digest))| {
            assert_eq!(*name, expected_name);
            (
                *name,
                blake3::hash(text.as_bytes()).to_hex().to_string(),
                digest,
            )
        })
        .collect();
    let changed: Vec<_> = pins
        .iter()
        .filter(|(_, actual, expected)| actual != expected)
        .map(|(name, _, _)| *name)
        .collect();
    for (name, text) in &dumps {
        if changed.contains(name) {
            eprintln!("---- dump {name} ----\n{text}");
        }
    }
    check(&pins);
}
