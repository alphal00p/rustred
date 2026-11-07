//! Custom owners: NO and t1star05 (owner: M2-T7c rr-custom).
//!
//! What `rustred::propagator::custom` decides without a generated payload: the
//! raw charts of an owner support, the hard false-knock-out guard on an owner
//! record, and the fail-closed refusals of `build_custom_step` (an arity
//! outside the compiled registry, bytes that are no payload). The generated
//! owners themselves (NO, t1star05), their closure and their reductions are
//! tested where they can be generated: `rustred-app`'s
//! `tests/p_library_app/custom.rs`.
use rustred::algebra::CoefficientContext;
use rustred::compiled_runtime_arities;
use rustred::propagator::PresentationId;
use rustred::propagator::custom::{
    build_custom_step, check_custom_owner, custom_owner_charts, custom_owner_support,
    inspect_custom_payload,
};
use rustred::propagator::error::PError;
use rustred::propagator::library::CustomOwnerMeta;
use rustred::propagator::presentation::{Presentation, PresentationKind, Slot, SlotLine};

fn context() -> CoefficientContext {
    CoefficientContext::try_new(["d"]).unwrap()
}

fn line(momentum: &[i64], eps_units: u8) -> Slot {
    Slot::Line(SlotLine {
        momentum: momentum.to_vec(),
        eps_units,
    })
}

/// The 2L kite `k1, k2, k1 - q, k2 - q, k1 - k2`, `eps_units` on the central
/// line (slot 4): t1star05 at one unit.
fn kite(eps_units: u8) -> Presentation {
    Presentation::from_slots(
        PresentationId(0),
        PresentationKind::EpsNode,
        "custom_kite",
        2,
        vec![
            line(&[1, 0, 0], 0),
            line(&[0, 1, 0], 0),
            line(&[1, 0, -1], 0),
            line(&[0, 1, -1], 0),
            line(&[1, -1, 0], eps_units),
        ],
        &context(),
        None,
    )
    .unwrap()
}

/// `loops` loop momenta, their differences with `q` and with each other: a
/// line basis of every scalar product (20 slots at 5 loops).
fn all_lines(loops: usize) -> Presentation {
    let unit = |index: usize| -> Vec<i64> {
        let mut row = vec![0; loops + 1];
        row[index] = 1;
        row
    };
    let minus = |left: usize, right: usize| {
        let mut row = unit(left);
        row[right] -= 1;
        row
    };
    let mut slots: Vec<_> = (0..loops).map(|i| line(&unit(i), 0)).collect();
    slots.extend((0..loops).map(|i| line(&minus(i, loops), 0)));
    for i in 0..loops {
        slots.extend((i + 1..loops).map(|j| line(&minus(i, j), 0)));
    }
    Presentation::from_slots(
        PresentationId(0),
        PresentationKind::EpsNode,
        "custom_wide",
        loops as u8,
        slots,
        &context(),
        None,
    )
    .unwrap()
}

fn meta(lp_count: u32, residuals: u32, two_sided_axes: u64, charts: Vec<u64>) -> CustomOwnerMeta {
    CustomOwnerMeta {
        lp_count,
        residuals,
        two_sided_axes,
        charts,
    }
}

#[test]
fn the_charts_of_a_support_are_its_raw_charts_on_both_sides_of_every_eps_axis() {
    assert_eq!(custom_owner_charts(0b1_1111, 0), [0b1_1111]);
    assert_eq!(
        custom_owner_charts(0b1_1111, 0b1_0000),
        [0b0_1111, 0b1_1111]
    );
    let two = custom_owner_charts(0b1_1111, 0b1_0001);
    assert_eq!(two, [0b0_1110, 0b0_1111, 0b1_1110, 0b1_1111]);
    // The support of an owner presentation: every line of an EpsNode.
    assert_eq!(custom_owner_support(&kite(1)), 0b1_1111);
    assert_eq!(kite(1).forced_axes(), 0b1_0000);
}

/// The hard guard of plan M2 sec.3.4 on constructed records.
#[test]
fn the_guard_refuses_a_false_knock_out_and_eps_owners_without_two_sided_data() {
    let integer = kite(0);
    let eps = kite(1);
    let both = vec![0b0_1111, 0b1_1111];
    check_custom_owner(&integer, &meta(1, 1, 0, vec![0b1_1111])).unwrap();
    check_custom_owner(&eps, &meta(1, 1, 0b1_0000, both.clone())).unwrap();
    // LP 0 with residuals is not a knock-out; LP 0 without is a closed owner.
    check_custom_owner(&eps, &meta(0, 1, 0b1_0000, both.clone())).unwrap();
    check_custom_owner(&eps, &meta(0, 0, 0b1_0000, both.clone())).unwrap();

    let knock_out = Err(PError::Generation(
        "false knock-out guard: LP=1, residuals=0".to_owned(),
    ));
    assert_eq!(
        check_custom_owner(&integer, &meta(1, 0, 0, vec![0b1_1111])),
        knock_out
    );
    assert_eq!(
        check_custom_owner(&eps, &meta(1, 0, 0b1_0000, both.clone())),
        knock_out
    );
    // The guard comes before the payload is read.
    assert_eq!(
        build_custom_step(&eps, b"", &meta(1, 0, 0b1_0000, both.clone())).err(),
        knock_out.err()
    );

    let unsupported = Err(PError::Unsupported(
        "eps custom owner requires two-sided axes".to_owned(),
    ));
    assert_eq!(
        check_custom_owner(&eps, &meta(1, 1, 0, both.clone())),
        unsupported
    );
    assert_eq!(
        build_custom_step(&eps, b"", &meta(1, 1, 0, vec![0b1_1111])).err(),
        unsupported.err()
    );
    for wrong in [
        // Two-sided data on an integer owner, or off the eps axis.
        (&integer, meta(1, 1, 0b1_0000, vec![0b1_1111])),
        (&eps, meta(1, 1, 0b0_0001, both.clone())),
        // A chart missing, a chart off the lines, no chart.
        (&eps, meta(1, 1, 0b1_0000, vec![0b1_1111])),
        (&eps, meta(1, 1, 0b1_0000, vec![0b10_1111, 0b11_1111])),
        (&integer, meta(1, 1, 0, Vec::new())),
        // Unsorted charts.
        (&eps, meta(1, 1, 0b1_0000, vec![0b1_1111, 0b0_1111])),
    ] {
        assert!(
            matches!(
                check_custom_owner(wrong.0, &wrong.1),
                Err(PError::Generation(_))
            ),
            "{:?}",
            wrong.1
        );
    }
}

/// Plan M2 sec.3.6: an arity outside `compiled_runtime_arities()` fails closed,
/// naming `RUSTRED_RUNTIME_ARITIES`.
#[test]
fn build_custom_step_fails_closed_for_an_uncompiled_arity() {
    let wide = all_lines(5);
    assert_eq!(wide.slots.len(), 20);
    let record = meta(1, 1, 0, vec![(1 << 20) - 1]);
    let built = build_custom_step(&wide, b"", &record);
    if compiled_runtime_arities().contains(&20) {
        // A build with 20 in the registry reads the (missing) payload instead.
        assert!(matches!(built, Err(PError::MalformedLibrary(_))));
        return;
    }
    match built {
        Err(PError::Unsupported(message)) => {
            assert!(message.contains("RUSTRED_RUNTIME_ARITIES"), "{message}");
            assert!(message.contains("arity 20"), "{message}");
        }
        other => panic!("arity 20 was not refused: {:?}", other.err()),
    }
}

#[test]
fn bytes_that_are_no_payload_are_refused() {
    let eps = kite(1);
    let record = meta(1, 1, 0b1_0000, vec![0b0_1111, 0b1_1111]);
    for bytes in [&b""[..], b"rustred", &[0_u8; 64]] {
        assert!(matches!(
            build_custom_step(&eps, bytes, &record),
            Err(PError::MalformedLibrary(_))
        ));
        assert!(matches!(
            inspect_custom_payload(bytes),
            Err(PError::MalformedLibrary(_))
        ));
    }
}
