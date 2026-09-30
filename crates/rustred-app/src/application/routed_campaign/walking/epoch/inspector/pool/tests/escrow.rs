use super::*;

#[test]
fn channel_buffers_are_charged_before_poll_and_recycling_is_not_memory_release() {
    let inspect = |_: &[u8], _: &AtomicBool| Vec::with_capacity(4096);
    with_polling_pool(1, &inspect, |pool| {
        pool.enable_result_escrow(8).unwrap();
        pool.submit_rolling(vec![work(1)]).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while pool.result_memory().count != 1 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(pool.result_memory().bytes, 4096);
        let bytes = loop {
            match pool.poll(Duration::from_secs(5)).unwrap() {
                Poll::Started(1) => {}
                Poll::Result { key: 1, bytes } => break bytes,
                _ => panic!("expected one complete result"),
            }
        };
        pool.recycle_result(1).unwrap();
        assert!(
            pool.submit_rolling(vec![work(1)]).is_err(),
            "escrow key stays logically reserved"
        );
        assert_eq!(pool.activity().unwrap().occupied, 0);
        assert_eq!(pool.result_memory().bytes, 4096);
        pool.submit_rolling(vec![work(2)]).unwrap();
        let second = loop {
            match pool.poll(Duration::from_secs(5)).unwrap() {
                Poll::Started(2) => {}
                Poll::Result { key: 2, bytes } => break bytes,
                _ => panic!("recycled slot did not progress"),
            }
        };
        assert_eq!(pool.result_memory().count, 2);
        drop(bytes);
        assert_eq!(pool.result_memory().count, 1);
        pool.cancel().unwrap();
        drop(second);
        assert_eq!(pool.result_memory().count, 0);
        assert!(pool.take_cancelled_status().unwrap().capacity() >= 8);
    })
    .unwrap();
}

#[test]
fn stale_started_and_result_messages_cannot_alias_reused_same_key_slots() {
    for stale_result in [false, true] {
        let queue = Mutex::new(Queue {
            jobs: VecDeque::new(),
            status: vec![Status {
                key: 7,
                started: true,
                returned: false,
            }],
            occupied: vec![true],
            generations: vec![2],
            next_generation: 2,
            shutdown: false,
        });
        let ready = Condvar::new();
        let stop = AtomicBool::new(false);
        let (sender, receiver) = mpsc::channel();
        let mut pool = Pool {
            queue: &queue,
            ready: &ready,
            stop: &stop,
            receiver: Some(receiver),
            receipts: vec![u8::from(stale_result)],
            remaining: 1,
            threads: 1,
            cancelled: false,
            result_memory: Arc::new(memory::Counter::default()),
            recycled: Vec::new(),
            logical_limit: MAX_BATCH,
        };
        sender
            .send(if stale_result {
                Message::Result(0, 7, 1, ReturnedBytes::uncharged(vec![]))
            } else {
                Message::Started(0, 7, 1)
            })
            .unwrap();
        assert!(
            pool.poll(Duration::ZERO)
                .err()
                .unwrap()
                .contains("stale result slot generation")
        );
        assert_eq!(pool.remaining, 1);
    }
}
