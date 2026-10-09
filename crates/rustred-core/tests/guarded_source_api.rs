//! The complete guarded lifecycle through public Rust interfaces only.
use std::sync::Arc;

use rustred::algebra::CoefficientContext;
use rustred::solver::guarded::{
    GuardedApplicationStatus, GuardedProgram, GuardedSource, GuardedSourceSystem, IndexBounds,
    IndexDomain, IndexRole,
};
use rustred::solver::{Integral, SearchOptions, Term};

#[test]
fn public_guarded_sources_discover_replay_save_and_apply_a_bulk_surface_rule() {
    let context = CoefficientContext::try_new(["a", "b", "x"]).unwrap();
    let x = context.parameter("x").unwrap();
    let domain = IndexDomain::new([
        IndexBounds::new(Some(2), None).unwrap(),
        IndexBounds::fixed(0),
    ])
    .unwrap();
    let source = GuardedSource::new(
        "supplied-boundary",
        vec![
            Term {
                integral: Integral::symbolic([0, 0]).unwrap(),
                coefficient: x.numerator.clone(),
            },
            Term {
                integral: Integral::symbolic([-1, 1]).unwrap(),
                coefficient: context.try_neg(&x, Default::default()).unwrap().numerator,
            },
        ],
        domain.clone(),
    );
    let sources = Arc::new(
        GuardedSourceSystem::new(
            "public-weighted-source-example-v1",
            [IndexRole::Ordinary, IndexRole::Occupation],
            [0, 1],
            vec![source],
        )
        .unwrap(),
    );
    let found = sources
        .solve_domain(
            domain,
            SearchOptions {
                max_depth: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(found.unresolved.is_empty());
    let program = GuardedProgram::new(sources.clone(), found.rules, [[1, 1]]).unwrap();
    let bytes = program.encode_native(Default::default()).unwrap();
    let loaded = GuardedProgram::decode_generated(&bytes, sources, Default::default()).unwrap();
    let applied = loaded.apply(&[2, 0]).unwrap();
    assert!(matches!(
        applied.status,
        GuardedApplicationStatus::Applied { .. }
    ));
    assert_eq!(applied.terms.get(&[1, 1]), Some(&context.one()));
    assert!(applied.nonzero_conditions.contains(&x.numerator));
    let reduced = loaded.reduce([2, 0], Default::default()).unwrap();
    assert!(reduced.unresolved.is_empty());
    assert_eq!(reduced.terms.get(&[1, 1]), Some(&context.one()));
}
