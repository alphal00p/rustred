use super::*;
use crate::application::routed_campaign::walking::epoch::snapshot::{Publication, StoreOwner};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn last_result_and_panicking_callback_release_lookup_leases_before_drained() {
    for panic in [false, true] {
        let mut owner = StoreOwner::<1>::new();
        let slot = Publication::new();
        slot.publish(owner.snapshot(0).unwrap()).unwrap();
        let inspect = |_: &[u8], _: &AtomicBool| {
            let _lease = slot.acquire().unwrap();
            assert!(_lease.domains.is_empty());
            if panic {
                panic!("owned snapshot callback failure");
            }
            vec![7]
        };
        with_polling_pool(1, &inspect, |pool| {
            pool.submit(vec![
                Work {
                    key: 1,
                    bytes: vec![],
                },
                Work {
                    key: 2,
                    bytes: vec![],
                },
            ])
            .unwrap();
            let mut returned = 0;
            loop {
                match pool.poll(Duration::from_secs(5)).unwrap() {
                    Poll::Result { bytes, .. } => {
                        assert_eq!(bytes, if panic { vec![] } else { vec![7] });
                        returned += 1;
                    }
                    Poll::Drained => break,
                    Poll::Started(_) => {}
                    Poll::Waiting => panic!("owned tiny callback did not finish"),
                }
            }
            assert_eq!(returned, 2);
            slot.clear().unwrap();
            owner.unique_mut().unwrap();
        })
        .unwrap();
    }
}

#[test]
fn cancel_keeps_immutable_lookup_readable_until_held_callback_joins() {
    let mut owner = StoreOwner::<1>::new();
    let slot = Publication::new();
    slot.publish(owner.snapshot(0).unwrap()).unwrap();
    let (started, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    let calls = AtomicUsize::new(0);
    let inspect = |_: &[u8], stop: &AtomicBool| {
        let lease = slot.acquire().unwrap();
        calls.fetch_add(1, Ordering::SeqCst);
        started.send(()).unwrap();
        released
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(stop.load(Ordering::Acquire));
        assert!(lease.domains.is_empty());
        vec![7]
    };
    with_polling_pool(1, &inspect, |pool| {
        pool.submit(vec![
            Work {
                key: 1,
                bytes: vec![],
            },
            Work {
                key: 2,
                bytes: vec![],
            },
        ])
        .unwrap();
        received.recv_timeout(Duration::from_secs(5)).unwrap();
        pool.cancel().unwrap();
        assert!(owner.ensure_unique().is_ok());
        // Like S3's save-before-join, immutable state is still readable. The
        // queued second callback is never executed. Canonical mutation no
        // longer depends on a reader of an independent lookup buffer.
        assert_eq!(owner.len(), 0);
        release.send(()).unwrap();
    })
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    slot.clear().unwrap();
    owner.unique_mut().unwrap();
}
