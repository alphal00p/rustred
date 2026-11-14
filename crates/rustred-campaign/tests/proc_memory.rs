//! The memory cap by sampling kills a child that allocates beyond it (the child allocates at most 250 MB).
//! The child is this test executable re-run as a helper (`RR_PROC_HELPER=alloc`).

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{
    ChildHandle, ChildLauncher, KillReason, MEMORY_CAP_EXCEEDED, MemoryMechanism,
};
use std::time::Duration;

/// Helper entry: does nothing unless run as a helper.
#[test]
fn helper_entry() {
    if std::env::var("RR_PROC_HELPER").as_deref() != Ok("alloc") {
        return;
    }
    // Allocate and touch 10 MB every 50 ms up to 250 MB, then sleep (the supervisor must kill us first).
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    for _ in 0..25 {
        let mut b = vec![0u8; 10_000_000];
        for i in (0..b.len()).step_by(4096) {
            b[i] = 1;
        }
        blocks.push(b);
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_secs(600));
    drop(blocks);
}

#[test]
fn memory_cap_by_sampling_kills_the_session() {
    let _g = serial();
    let dir = TempDir::new("memcap");
    let mut l = launcher();
    let mut spec = helper_spec(dir.path(), "memcap", "helper_entry", "alloc");
    spec.memory_cap_bytes = Some(80_000_000);
    spec.cap_mechanism = rustred_campaign::launch::CapRequest::Sampling;
    let mut h = l.launch(&spec).unwrap();
    let sid = h.identity().pid;
    let r = drive(&mut h);
    assert_eq!(r.memory_mechanism, MemoryMechanism::Sampling);
    match &r.kill_reason {
        Some(KillReason::MemoryCap {
            rss_bytes,
            cap_bytes,
        }) => {
            assert_eq!(*cap_bytes, 80_000_000);
            assert!(*rss_bytes > 80_000_000);
        }
        other => panic!("expected a memory-cap kill, got {other:?} ({r:?})"),
    }
    assert_eq!(r.supervisor_outcome(), Some(MEMORY_CAP_EXCEEDED));
    assert_eq!(r.exit.and_then(|e| e.signal()), Some(libc::SIGKILL));
    assert!(
        r.peak_rss_bytes > 80_000_000 && r.peak_rss_bytes < 400_000_000,
        "peak {}",
        r.peak_rss_bytes
    );
    assert!(r.session_verified_empty);
    assert_session_gone(sid);
}
