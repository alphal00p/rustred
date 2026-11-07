//! Library assembly, descent and exactness (owner: M2-T8a rr-assemble).
//!
//! The lane-program factory, the descent certificate and the support-only trace
//! on hand-built one-loop libraries: the reference library of M2-T6 and a bubble
//! whose fake lane misbehaves on purpose. The assembled 1L-2L library and its
//! exactness are tested in `rustred-app` (`tests/p_library_app/assemble.rs`),
//! where `assemble_library` lives.
use std::collections::BTreeMap;
use std::sync::Arc;

use rustred::algebra::CoefficientContext;
use rustred::family::IntegralKey;
use rustred::propagator::census::{PCensus, PClassRecord, PSectorFate, SignedAffineLoopMap};
use rustred::propagator::class::OwnerClassKey;
use rustred::propagator::descent::{self, TraceFailureKind, TraceOptions};
use rustred::propagator::error::PError;
use rustred::propagator::families::{P_FAMILIES_V1_JSON, PFamiliesFile};
use rustred::propagator::lane::{BubbleRoles, LaneRoles, OwnerLane};
use rustred::propagator::library::{OwnerProgram, PLibrary, POwnerSpec};
use rustred::propagator::programs::lane_program;
use rustred::propagator::step::{PChild, PKeyView, PLaneStep, PStage, PStepContext, PStepOut};
use rustred::propagator::testing::{
    REFERENCE_FLIPPED, REFERENCE_SHIFTED, REFERENCE_SWAPPED, REFERENCE_TOP,
    one_loop_reference_library,
};
use rustred::propagator::{OwnerClassId, PTerminalId, PresentationId};
use rustred::reduction::ReductionLimits;

fn limits() -> ReductionLimits {
    ReductionLimits::default()
}

/// Every key of the bubble with powers in `-2..=3`.
fn bubble_keys(presentation: PresentationId) -> Vec<(PresentationId, Vec<i64>)> {
    (-2..=3)
        .flat_map(|a| (-2..=3).map(move |b| (presentation, vec![a, b])))
        .collect()
}

/// The reference library has a certificate: one SeriesParallel owner, three
/// routes onto it (swapped, flipped, shifted), the owner sector, and the three
/// scaleless sectors of each of the four presentations.
#[test]
fn the_reference_library_has_a_descent_certificate() {
    let library = one_loop_reference_library().unwrap();
    let report = descent::check(&library).unwrap();
    assert_eq!(
        report.owners,
        BTreeMap::from([(OwnerLane::SeriesParallel, 1)])
    );
    assert_eq!(
        (report.routes, report.owner_sectors, report.zero_sectors),
        (3, 1, 12)
    );
    assert_eq!(
        (report.edge_maps, report.guarded_owners, report.overlays),
        (0, 0, 0)
    );
}

/// Every key of every routing reaches the owner, or a zero sector, and descends:
/// 36 distinct keys per routing, the Apply sites of the owner are the keys with
/// both powers positive (9, shared by all routings).
#[test]
fn the_trace_of_the_reference_routings_is_clean() {
    let library = one_loop_reference_library().unwrap();
    let inputs: Vec<_> = [
        REFERENCE_TOP,
        REFERENCE_SWAPPED,
        REFERENCE_FLIPPED,
        REFERENCE_SHIFTED,
    ]
    .into_iter()
    .flat_map(bubble_keys)
    .collect();
    let report = descent::trace(&library, &inputs, limits()).unwrap();
    assert!(report.is_clean(), "{:?}", report.samples);
    assert_eq!(report.distinct_inputs, 4 * 36);
    assert_eq!(report.apply_sites, 9);
    assert_eq!(report.applied.values().sum::<usize>(), 9);
    // The fake lane adds the bubble tag: one tag per site, the inputs being at tag one.
    let tagged = descent::trace_with(
        &library,
        &inputs,
        TraceOptions {
            limits: limits(),
            tags: true,
        },
    )
    .unwrap();
    assert_eq!((tagged.apply_keys, tagged.apply_sites), (9, 9));
    // An input that is no key of the library is an error.
    assert!(matches!(
        descent::trace(&library, &[(PresentationId(9), vec![1, 1])], limits()),
        Err(PError::Unsupported(_))
    ));
    assert!(matches!(
        descent::trace(&library, &[(REFERENCE_TOP, vec![1, 1, 1])], limits()),
        Err(PError::Unsupported(_))
    ));
}

/// A bubble lane that misbehaves on the keys `(a, b)` with `a >= 2`.
#[derive(Debug)]
enum Misbehaving {
    /// An Apply child on its own key.
    Stays,
    /// An Apply child one dot higher.
    Climbs,
    /// `PError::Uncovered`.
    Uncovered,
}

impl PLaneStep for Misbehaving {
    fn lane(&self) -> OwnerLane {
        OwnerLane::SeriesParallel
    }

    fn step(&self, cx: &PStepContext<'_>, key: &PKeyView<'_>) -> Result<PStepOut, PError> {
        let context = cx.library.coefficient_context();
        let &[a, b] = key.powers else {
            unreachable!("a bubble key")
        };
        if a < 2 {
            return Ok(PStepOut {
                terminals: vec![(PTerminalId(0), key.tag.clone(), context.one())],
                ..PStepOut::default()
            });
        }
        let child = |powers: Vec<i64>| PChild {
            stage: PStage::Apply,
            presentation: key.presentation,
            powers,
            tag: key.tag.clone(),
            coefficient: context.one(),
        };
        match self {
            Self::Stays => Ok(PStepOut {
                children: vec![child(vec![a, b])],
                ..PStepOut::default()
            }),
            Self::Climbs => Ok(PStepOut {
                children: vec![child(vec![a + 1, b])],
                ..PStepOut::default()
            }),
            Self::Uncovered => Err(PError::Uncovered {
                class: cx.owner.spec.class,
                key: IntegralKey::try_new([a, b]).unwrap(),
            }),
        }
    }
}

fn identity() -> SignedAffineLoopMap {
    SignedAffineLoopMap {
        loops_in: 1,
        loops_out: 1,
        loop_linear: Box::new([1]),
        loop_external: Box::new([0]),
        external_sign: 1,
    }
}

/// The 1L top alone, one class, its owner running `program`.
fn bubble_library(
    spec: impl FnOnce(OwnerClassId) -> POwnerSpec,
    program: OwnerProgram,
) -> PLibrary {
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let file: PFamiliesFile = serde_json::from_str(P_FAMILIES_V1_JSON).unwrap();
    let top = file.families[0]
        .presentation(PresentationId(0), &context)
        .unwrap();
    let mut builder = PLibrary::builder(context, 1..=1);
    builder.add_presentation(top).unwrap();
    let mut census = PCensus::default();
    let class = census
        .insert_class(PClassRecord {
            key: OwnerClassKey {
                loops: 1,
                lines: 2,
                eps_lines: 0,
                canonical: Arc::from(&b"assemble test bubble"[..]),
            },
            owner: (PresentationId(0), 0b11),
            node: None,
            nodes: Vec::new(),
            t: 2,
            eps_lines: 0,
        })
        .unwrap();
    census.sectors.insert(
        (PresentationId(0), 0b11),
        PSectorFate::Owner {
            class,
            map: identity(),
        },
    );
    for support in 0b00..0b11 {
        census
            .sectors
            .insert((PresentationId(0), support), PSectorFate::Zero);
    }
    builder.set_census(census).unwrap();
    builder
        .add_owner_with_program(spec(class), program)
        .unwrap();
    builder.build().unwrap()
}

fn sp_spec(class: OwnerClassId) -> POwnerSpec {
    POwnerSpec {
        class,
        lane: OwnerLane::SeriesParallel,
        presentation: PresentationId(0),
        node: None,
        roles: LaneRoles::Insertion(BubbleRoles {
            lines: [0, 1],
            loop_index: 0,
        }),
        edge_maps: Vec::new(),
        payload: None,
        custom: None,
        terminals: vec![PTerminalId(0)],
    }
}

/// The trace counts each refusal once, for the first input that reaches it, and
/// does not expand the refused key.
#[test]
fn the_trace_counts_uncovered_keys_and_steps_that_do_not_descend() {
    let inputs = bubble_keys(PresentationId(0));
    for (lane, kind) in [
        (Misbehaving::Stays, TraceFailureKind::Descent),
        (Misbehaving::Climbs, TraceFailureKind::Descent),
        (Misbehaving::Uncovered, TraceFailureKind::Uncovered),
    ] {
        let library = bubble_library(sp_spec, OwnerProgram::Lane(Arc::new(lane)));
        let report = descent::trace(&library, &inputs, limits()).unwrap();
        // The keys (2, b) and (3, b) with b in 1..=3.
        assert_eq!(
            report.failures_of(kind),
            6,
            "{kind:?}: {:?}",
            report.samples
        );
        assert_eq!(report.failures.values().sum::<usize>(), 6);
        assert!(!report.is_clean());
    }
    let library = bubble_library(sp_spec, OwnerProgram::Missing { ledger: 3 });
    let report = descent::trace(&library, &inputs, limits()).unwrap();
    assert_eq!(report.failures_of(TraceFailureKind::MissingOwner), 9);
    assert!(matches!(
        descent::check(&library),
        Err(PError::MissingOwner { ledger: 3, .. })
    ));
}

/// The factory builds the real SeriesParallel step from roles alone, and the
/// certificate accepts it; an owner of a lane without a program, or with roles
/// that break the contract, fails closed.
#[test]
fn the_factory_builds_the_bubble_from_its_roles() {
    let library = bubble_library(sp_spec, OwnerProgram::Missing { ledger: 0 });
    let class = library.owners().next().unwrap().spec.class;
    let spec = sp_spec(class);
    let OwnerProgram::Lane(step) = lane_program(&library, &spec).unwrap() else {
        panic!("no lane")
    };
    assert_eq!(step.lane(), OwnerLane::SeriesParallel);
    let mut insertion = spec.clone();
    insertion.lane = OwnerLane::Insertion;
    assert_eq!(
        lane_program(&library, &insertion)
            .unwrap()
            .clone_lane()
            .lane(),
        OwnerLane::Insertion
    );
    for lane in [OwnerLane::Zero, OwnerLane::Diamond, OwnerLane::Product] {
        let mut wrong = spec.clone();
        wrong.lane = lane;
        assert!(matches!(
            lane_program(&library, &wrong),
            Err(PError::MalformedLibrary(_))
        ));
    }
    let mut swapped_roles = spec.clone();
    swapped_roles.roles = LaneRoles::Insertion(BubbleRoles {
        lines: [0, 0],
        loop_index: 0,
    });
    assert!(lane_program(&library, &swapped_roles).is_err());
    let mut no_roles = spec;
    no_roles.roles = LaneRoles::None;
    assert!(matches!(
        lane_program(&library, &no_roles),
        Err(PError::Contract(_))
    ));
}

trait CloneLane {
    fn clone_lane(&self) -> Arc<dyn PLaneStep>;
}

impl CloneLane for OwnerProgram {
    fn clone_lane(&self) -> Arc<dyn PLaneStep> {
        match self {
            OwnerProgram::Lane(step) => Arc::clone(step),
            OwnerProgram::Missing { .. } => panic!("a missing program"),
        }
    }
}
