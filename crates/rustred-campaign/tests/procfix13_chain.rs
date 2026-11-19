//! Lane procfix7 round 9 (gate13 m13-chain): the walk's "ancestor of this process" rule (resources/procfs.rs
//! `is_own_ancestor`, the uid rule (c') of round 8b) keeps the ancestor chain PER PROC ROOT. Before, one process-wide
//! chain was filled by the FIRST caller's root: a fake root of a test decided the answer for every other root.
//! Two fake proc trees that disagree about this process's parent; the root that does NOT name 900 as an ancestor is asked
//! first. Fake proc trees only: nothing is spawned or signalled.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::resources::our_uid;
use rustred_campaign::resources::procfs::{ProcError, ProcRoot};
use std::path::{Path, PathBuf};

fn stat_line(pid: u32, ppid: u32, start: u64) -> String {
    format!("{pid} (x) S {ppid} {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 3 0 {start} 0 0\n")
}

/// A process whose task list can never be proven (Threads: 3, two tasks listed), real uid `ruid`, children `kids`.
fn mk(d: &Path, pid: u32, ppid: u32, ruid: u32, kids: &str) {
    let p = d.join(pid.to_string());
    for t in [pid, pid + 1] {
        std::fs::create_dir_all(p.join(format!("task/{t}"))).unwrap();
        std::fs::write(p.join(format!("task/{t}/stat")), stat_line(t, ppid, 77)).unwrap();
        std::fs::write(
            p.join(format!("task/{t}/children")),
            if t == pid { kids } else { "" },
        )
        .unwrap();
    }
    std::fs::write(p.join("stat"), stat_line(pid, ppid, 77)).unwrap();
    std::fs::write(
        p.join("status"),
        format!("Name:\tx\nUid:\t{ruid}\t{ruid}\t{ruid}\t{ruid}\nThreads:\t3\n"),
    )
    .unwrap();
}

/// A fake root where this process's parent is `parent`, and the foreign-uid process 900 exists.
fn root(tag: &str, parent: u32) -> PathBuf {
    let me = std::process::id();
    let d = std::env::temp_dir().join(format!(
        "rr-pf13-chain-{tag}-{me}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(d.join(me.to_string())).unwrap();
    std::fs::write(d.join(format!("{me}/stat")), stat_line(me, parent, 50)).unwrap();
    let foreign = if our_uid() == 4242 { 4343 } else { 4242 };
    mk(&d, 900, 0, foreign, "901 ");
    if parent != 900 {
        mk(&d, parent, 0, foreign, "");
    }
    d
}

#[test]
fn the_ancestor_chain_is_kept_per_proc_root() {
    let not_ancestor = root("b", 950);
    let ancestor = root("a", 900);
    let rb = ProcRoot::at(&not_ancestor);
    let ra = ProcRoot::at(&ancestor);
    // asked FIRST: in this root 900 is a foreign-uid process that is not an ancestor: read once, best effort
    let b = rb.children_all_threads_proven(900);
    assert!(matches!(&b, Ok(v) if v == &vec![901]), "{b:?}");
    // in this root 900 IS this process's parent (a re-parent target): its children list keeps its proof
    let a = ra.children_all_threads_proven(900);
    assert!(
        matches!(&a, Err(ProcError::Unreadable(w)) if w.contains("cannot be proven")),
        "the ancestor's list must keep its proof: {a:?}"
    );
    // and the first root's answer is unchanged
    let b2 = rb.children_all_threads_proven(900);
    assert!(matches!(&b2, Ok(v) if v == &vec![901]), "{b2:?}");
    let _ = std::fs::remove_dir_all(&not_ancestor);
    let _ = std::fs::remove_dir_all(&ancestor);
}
