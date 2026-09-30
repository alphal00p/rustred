//! Synthetic lifecycle controls, not a mathematical closure certificate.
use super::*;

#[test]
fn reserved_preparation_helpers_stop_before_publish_then_restore_and_drain() {
    for rolling in [false, true] {
        let mut fixture = Fixture::new();
        fixture.request.workers = 5;
        fixture.request.epoch_preparation_workers = Some(2);
        fixture.request.epoch_rolling = rolling;
        fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
            fixture.directory.0.clone(),
        ));
        fixture.save(3, 0);
        let identity = fixture.identity();
        let mut restored = fixture.open().unwrap();
        let cancelled = AtomicBool::new(false);
        let ticks = AtomicUsize::new(0);
        let inspect =
            |bytes: &[u8], _: &AtomicBool| result(&Job::<1>::decode(bytes).unwrap(), false);
        let config = MergeConfig {
            lockstep: !rolling,
            ..config()
        };
        // Five total reservations = two inspectors + two preparation helpers
        // + this controller. The executor receives only its own three slots.
        let outcome = controller::run_observed(
            &mut restored,
            &identity,
            16,
            3,
            config,
            &|| Ok(()),
            &inspect,
            || stop::requested(&cancelled, None),
            |_| false,
            |_, _, _| {},
            None,
            |_, _, phase, _| {
                if phase == "p2" && ticks.fetch_add(1, Ordering::Relaxed) >= 2 {
                    cancelled.store(true, Ordering::Release);
                }
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Stopped(merge::StopReason::Paused));
        assert_eq!(restored.state.k, 0, "cancelled preparation cannot publish");
        assert_eq!(restored.records.total(), 0);
        assert!(!restored.state.poisoned);
        drop(restored);
        // Preparation helpers are a session execution receipt. Resume the
        // helper-produced checkpoint with no helper and one inline worker.
        fixture.request.workers = 1;
        fixture.request.epoch_preparation_workers = Some(0);
        fixture.request.epoch_preparation_max_obligations = Some(17);
        assert!(
            fixture
                .open()
                .err()
                .expect("logical preparation allowances remain bound")
                .to_string()
                .contains("logical allowances")
        );
        fixture.request.epoch_preparation_max_obligations = None;
        let identity = fixture.identity();
        assert_eq!(identity.preparation().helpers, 0);
        let mut restored = fixture.open().unwrap();
        assert_eq!(restored.state.k, 0);
        assert_eq!(restored.replay.len(), if rolling { 3 } else { 0 });
        let outcome = controller::run_observed(
            &mut restored,
            &identity,
            16,
            1,
            config,
            &|| Ok(()),
            &inspect,
            || None,
            |_| false,
            |_, _, _| {},
            None,
            |_, _, _, _| {},
            |_| {},
        )
        .unwrap();
        assert_eq!(outcome, Outcome::Drained);
        assert_eq!(restored.records.total(), 3);
        assert!(restored.state.preparation.cuts > 0);
        drop(restored);
        let restored = fixture.open().unwrap();
        assert_eq!(
            restored.records.total(),
            3,
            "binary authority cold-restores"
        );
        assert_eq!(restored.state.pending_or_reserved(), 0);
    }
}
