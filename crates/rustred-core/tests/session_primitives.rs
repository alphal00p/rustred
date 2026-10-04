//! Public native primitives used by the in-process generation session.
use std::convert::Infallible;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use rustred::algebra::{Coefficient, CoefficientContext};
use rustred::persistence::{
    BinaryIoLimits, CoefficientId, CoefficientTableBuilder, LazyDecodedCoefficientTable,
};
use rustred::solver::{
    Integral, SectorCompleted, SectorConfig, SectorExecutionError, SectorExecutor,
    SectorSolveOptions, SourceSystem, Term,
};
use symbolica::atom::AtomCore;
use symbolica::prelude::{Q, Z};

fn coefficient(source: &str) -> Coefficient {
    symbolica::parse!(source).to_rational_polynomial::<_, _, u16>(&Q, &Z, None)
}

#[test]
fn lazy_table_keeps_polynomials_encoded_until_selected_access() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<LazyDecodedCoefficientTable>();
    let limits = BinaryIoLimits::default();
    let values = [
        coefficient("lazy_atom_x/(lazy_atom_x+1)"),
        coefficient("2/(lazy_atom_x+2)"),
    ];
    let mut builder = CoefficientTableBuilder::new(limits);
    for value in &values {
        builder.intern(value).unwrap();
    }
    let encoded = builder.finish().unwrap();
    let lazy = LazyDecodedCoefficientTable::new(encoded.state.into(), encoded.atoms.into(), limits)
        .unwrap();
    assert_eq!(lazy.len(), 2);
    assert_eq!(lazy.decoded_count().unwrap(), 0);
    let id = CoefficientId::try_from_index(1).unwrap();
    let selected = lazy.coefficient(id).unwrap();
    assert_eq!(selected.as_ref(), &values[1]);
    assert_eq!(lazy.decoded_count().unwrap(), 1);
    assert!(Arc::ptr_eq(&selected, &lazy.coefficient(id).unwrap()));
    assert_eq!(lazy.decoded_count().unwrap(), 1);
    assert!(
        lazy.coefficient(CoefficientId::try_from_index(2).unwrap())
            .is_err()
    );
}

fn trivial_sources() -> SourceSystem<3> {
    let context = CoefficientContext::try_new(["n0", "n1", "n2"]).unwrap();
    SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0; 3]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [0, 1, 2],
    )
    .unwrap()
}

fn completed_ordinal(completed: SectorCompleted<3>) -> Result<usize, Infallible> {
    Ok(completed.ordinal)
}

#[test]
fn cancellation_before_dispatch_skips_all_sector_work() {
    let source = trivial_sources();
    let executor = SectorExecutor::new(1)
        .unwrap()
        .with_cancellation(Arc::new(AtomicBool::new(true)));
    let outcome = executor.map_configured_with_observer(
        &source,
        &[[true; 3]],
        |_, _| panic!("cancelled sector must not configure"),
        SectorSolveOptions::default(),
        |_, _, _| {},
        completed_ordinal,
    );
    assert!(matches!(
        outcome,
        Err(SectorExecutionError::Cancelled { ordinal: 0, .. })
    ));
}

#[test]
fn cancellation_during_a_sector_drains_it_and_skips_the_next() {
    let source = trivial_sources();
    let token = Arc::new(AtomicBool::new(false));
    let completed = AtomicUsize::new(0);
    let executor = SectorExecutor::new(1)
        .unwrap()
        .with_cancellation(token.clone());
    let outcome = executor.map_configured_with_observer(
        &source,
        &[[true; 3], [true; 3]],
        |_, _| SectorConfig::default(),
        SectorSolveOptions::default(),
        |_, _, _| {
            token.store(true, Ordering::Release);
        },
        |done| {
            completed.fetch_add(1, Ordering::AcqRel);
            completed_ordinal(done)
        },
    );
    assert!(matches!(
        outcome,
        Err(SectorExecutionError::Cancelled { ordinal: 1, .. })
    ));
    assert_eq!(completed.load(Ordering::Acquire), 1);
}
