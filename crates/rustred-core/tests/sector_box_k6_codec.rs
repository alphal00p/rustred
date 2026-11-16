//! Session codec (checkpoint of the modular phase) on the three-loop K6
//! vacuum family with line 0 deformed by `eta`: a checkpoint written after
//! `extend` restores the selection in a fresh session (the original dropped,
//! as after a kill), encodes back to the same bytes and is materialise-only;
//! another family, router, option set or storage arity and every corruption
//! are refused with an error. Seconds in CI.
use std::sync::Arc;

use rustred::algebra::{CoefficientContext, ExactAlgebraLimits};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::sector::CutConstraint;
use rustred::solver::SolverError;
use rustred::solver::sector_box::{
    BoxLaportaSession, BoxOptions, CensusRouter, ColumnOrder, DEFAULT_CERTIFICATE_PRIME, FfOptions,
    IdentityRouter, SectorRouter, Seeding, SlotPriority,
};
use serde_json::Value;
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

const N: usize = 6;
const TOP: [i16; N] = [1, 1, 1, 1, 1, 1];

/// K6 with the line `deformed` shifted to mass^2 `1 + eta`.
fn k6(deformed: usize) -> Arc<IntegralFamily> {
    const ROWS: [[i64; 6]; 6] = [
        [1, 0, 0, 0, 0, 0],
        [0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1],
        [1, 0, -2, 0, 0, 1],
        [1, -2, 0, 1, 0, 0],
        [0, 0, 0, 1, -2, 1],
    ];
    let context = CoefficientContext::try_new(["d", "eta"]).unwrap();
    let denominators = ROWS
        .iter()
        .enumerate()
        .map(|(slot, row)| {
            let mut constant = context.integer(-1);
            if slot == deformed {
                constant = context
                    .try_sub(
                        &constant,
                        &context.parameter("eta").unwrap(),
                        ExactAlgebraLimits::default(),
                    )
                    .unwrap();
            }
            AffineDenominator::new(constant, row.map(|value| context.integer(value)).to_vec())
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "k6_eta",
            ["k1", "k2", "k3"].map(String::from).to_vec(),
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.zero(); N],
        )
        .unwrap(),
    )
}

fn census(family: &Arc<IntegralFamily>) -> Arc<dyn SectorRouter<N>> {
    Arc::new(CensusRouter::discover(family.clone(), &[true; N]).unwrap())
}

fn options() -> BoxOptions {
    BoxOptions {
        seeding: Seeding::Staircase {
            s_pad: 0,
            d_pad: 0,
            max_growth: 0,
        },
        order: ColumnOrder::LinesRankDots,
        slot_priority: SlotPriority::TopSectorFirst(63),
        workers: 2,
        lp_gate: false,
        ..Default::default()
    }
}

/// A session after `extend` on TOP and two of its derivative seeds.
fn extended() -> (BoxLaportaSession<N>, Vec<[i16; N]>) {
    let family = k6(0);
    let mut session = BoxLaportaSession::<N>::new(
        family.clone(),
        &CutConstraint::none(N).unwrap(),
        census(&family),
        options(),
    )
    .unwrap();
    let targets = vec![TOP, [2, 1, 1, 1, 1, 1], [2, 1, 1, 1, 1, 0]];
    session.extend(&targets, &[TOP]).unwrap();
    (session, targets)
}

#[test]
fn round_trip_restores_the_selection_after_the_session_is_dropped() {
    let (session, targets) = extended();
    let masters = session.masters().to_vec();
    assert!(!masters.is_empty());
    let path = std::env::temp_dir().join(format!(
        "rustred-sector-box-codec-{}.bin",
        std::process::id()
    ));
    let bytes = session.to_native_bytes().unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let family = session.family().clone();
    drop(session);
    let read = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut restored = BoxLaportaSession::<N>::from_native_bytes(
        &read,
        family.clone(),
        census(&family),
        options(),
    )
    .unwrap();
    assert_eq!(restored.masters(), masters.as_slice());
    assert_eq!(restored.to_native_bytes().unwrap(), bytes);
    // Materialise-only: the modular row space is not part of the checkpoint.
    assert!(matches!(
        restored.extend(&targets, &[]),
        Err(SolverError::InvalidInput(_))
    ));
}

#[test]
fn checkpoints_are_bound_to_family_router_options_and_arity() {
    let (session, _) = extended();
    let bytes = session.to_native_bytes().unwrap();
    let family = session.family().clone();
    let message = |result: Result<BoxLaportaSession<N>, SolverError>| {
        result.err().expect("must be refused").to_string()
    };
    // Another family: the deformation on another line.
    let other = k6(1);
    assert!(
        message(BoxLaportaSession::<N>::from_native_bytes(
            &bytes,
            other.clone(),
            census(&other),
            options()
        ))
        .contains("another family")
    );
    // Another router.
    let identity: Arc<dyn SectorRouter<N>> = IdentityRouter::shared(&family).unwrap();
    assert!(
        message(BoxLaportaSession::<N>::from_native_bytes(
            &bytes,
            family.clone(),
            identity,
            options()
        ))
        .contains("router")
    );
    // Options that change the column order or the sample.
    let mut seed = options();
    seed.sample.seed ^= 1;
    for other in [
        BoxOptions {
            order: ColumnOrder::Numeric,
            ..options()
        },
        BoxOptions {
            slot_priority: SlotPriority::Natural,
            ..options()
        },
        seed,
    ] {
        assert!(
            BoxLaportaSession::<N>::from_native_bytes(
                &bytes,
                family.clone(),
                census(&family),
                other
            )
            .is_err()
        );
    }
    // Options outside the binding may differ.
    assert!(
        BoxLaportaSession::<N>::from_native_bytes(
            &bytes,
            family.clone(),
            census(&family),
            BoxOptions {
                workers: 1,
                ..options()
            }
        )
        .is_ok()
    );
    // Another storage arity (the 6-slot family in 7 slots).
    let wide: Arc<dyn SectorRouter<7>> = IdentityRouter::shared(&family).unwrap();
    let refused =
        BoxLaportaSession::<7>::from_native_bytes(&bytes, family.clone(), wide, options());
    assert!(refused.err().unwrap().to_string().contains("storage arity"));
}

#[test]
fn corrupted_or_tampered_checkpoints_are_refused() {
    let (session, _) = extended();
    let bytes = session.to_native_bytes().unwrap();
    let family = session.family().clone();
    let decode = |bytes: &[u8]| {
        BoxLaportaSession::<N>::from_native_bytes(bytes, family.clone(), census(&family), options())
    };
    let message = |bytes: &[u8]| decode(bytes).err().expect("must be refused").to_string();
    assert!(decode(&bytes).is_ok());
    assert!(decode(&[]).is_err());
    for cut in [1, 31, 32, bytes.len() / 2, bytes.len() - 1] {
        assert!(decode(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    for at in [
        0,
        16,
        20,
        bytes.len() / 3,
        bytes.len() / 2,
        bytes.len() - 40,
        bytes.len() - 1,
    ] {
        let mut flipped = bytes.clone();
        flipped[at] ^= 0x10;
        assert!(decode(&flipped).is_err(), "bit flip at {at}");
    }
    // Tampering with a valid checksum.
    let resealed = |mut body: Vec<u8>| {
        let checksum = blake3::hash(&body);
        body.extend_from_slice(checksum.as_bytes());
        body
    };
    let body = bytes[..bytes.len() - 32].to_vec();
    assert!(decode(&resealed(body.clone())).is_ok());
    let mut magic = body.clone();
    magic[0] ^= 1;
    assert!(message(&resealed(magic)).contains("magic"));
    let mut version = body.clone();
    version[16] ^= 1;
    assert!(message(&resealed(version)).contains("version"));
    // The body ends with [len][64 hex selected_hash][len][64 hex master_hash];
    // both hashes are recomputed from the decoded rows and masters.
    for (at, what) in [
        (body.len() - 1, "master hash"),
        (body.len() - 64 - 8 - 1, "selected-row hash"),
    ] {
        let mut tampered = body.clone();
        tampered[at] = if tampered[at] == b'0' { b'1' } else { b'0' };
        assert!(message(&resealed(tampered)).contains(what), "{what}");
    }
}

#[test]
fn encoding_needs_a_selection() {
    let family = k6(0);
    let session = BoxLaportaSession::<N>::new(
        family.clone(),
        &CutConstraint::none(N).unwrap(),
        census(&family),
        options(),
    )
    .unwrap();
    assert!(session.to_native_bytes().is_err());
}

/// The checkpoint carries everything the lanes need: a session decoded from
/// it (the original dropped) materialises the same finite-field solution
/// (masters, coefficients, provenance and the exchange file up to its
/// timing statistics), and that solution passes the probabilistic
/// certificate in the restored session.
#[test]
fn restored_session_materialises_the_same_finite_field_solution() {
    let (mut session, targets) = extended();
    let d0 = Rational::from((Integer::from(37), Integer::from(11)));
    let fixed = vec![("d".to_string(), d0)];
    let reference = session
        .materialize_ff(&targets, &fixed, &FfOptions::default())
        .unwrap();
    let family = session.harness_family();
    let without_stats = |text: &str| {
        let mut value: Value = serde_json::from_str(text).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("stats");
        object.remove("content_blake3");
        value
    };
    let reference_file = reference
        .to_harness_json(&family, &Value::Null, &[])
        .unwrap();
    let bytes = session.to_native_bytes().unwrap();
    let source = session.family().clone();
    drop(session);
    let mut restored = BoxLaportaSession::<N>::from_native_bytes(
        &bytes,
        source.clone(),
        census(&source),
        options(),
    )
    .unwrap();
    let again = restored
        .materialize_ff(&targets, &fixed, &FfOptions::default())
        .unwrap();
    assert_eq!(again.masters, reference.masters);
    assert_eq!(again.coefficients, reference.coefficients);
    assert_eq!(again.provenance, reference.provenance);
    assert_eq!(
        without_stats(&again.to_harness_json(&family, &Value::Null, &[]).unwrap()),
        without_stats(&reference_file)
    );
    restored
        .certify(&again, Some(DEFAULT_CERTIFICATE_PRIME), 5)
        .unwrap();
}
