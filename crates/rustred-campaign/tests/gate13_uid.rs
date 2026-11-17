//! gate13 audit (WS/notes/implementer.gate13.md), item 4: the walk's uid rule (c') of ac/procfix7 1be053b6
//! (resources/procfs.rs `children_all_threads_proven`: a children list needs its proof only where it can hold a marked
//! process: real uid ours, real uid 0, an unreadable real uid, or an ancestor of this process; any other process is read
//! once, best effort). Fake proc trees only (no process is spawned or signalled).
//!
//! ONE test function on purpose: `is_own_ancestor` keeps the ancestor chain of the process in a process-wide OnceLock,
//! filled by the FIRST call from whatever `ProcRoot` makes it; the ancestor case below must be that first call.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::resources::our_uid;
use rustred_campaign::resources::procfs::{ProcError, ProcRoot};
use std::path::Path;

fn stat_line(pid: u32, ppid: u32, start: u64) -> String {
    format!("{pid} (x) S {ppid} {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 3 0 {start} 0 0\n")
}

/// A process whose task list can never be proven (Threads: 3, two tasks listed), with real uid `ruid` (None: an
/// unparsable Uid line), children `kids` on its leader thread.
fn mk(d: &Path, pid: u32, ppid: u32, ruid: Option<u32>, kids: &str) {
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
    let uid = match ruid {
        Some(u) => format!("Uid:\t{u}\t{u}\t{u}\t{u}\n"),
        None => "Uid:\tgarbage\n".to_string(),
    };
    std::fs::write(p.join("status"), format!("Name:\tx\n{uid}Threads:\t3\n")).unwrap();
}

#[test]
fn g13_uid_rule_cases() {
    let me = std::process::id();
    let d = std::env::temp_dir().join(format!(
        "rr-g13-uid-{me}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    let ours = our_uid();
    let foreign = if ours == 4242 { 4343 } else { 4242 };
    // this process (in the fake tree) whose parent 900 is a FOREIGN-uid process: an ancestor (re-parent target)
    std::fs::create_dir_all(d.join(me.to_string())).unwrap();
    std::fs::write(d.join(format!("{me}/stat")), stat_line(me, 900, 50)).unwrap();
    mk(&d, 900, 0, Some(foreign), "901 ");
    // root, unparsable uid, foreign, ours
    mk(&d, 500, 1, Some(0), "501 ");
    mk(&d, 600, 1, None, "601 ");
    mk(&d, 700, 1, Some(foreign), "701 702 ");
    mk(&d, 800, 1, Some(ours), "801 ");
    let r = ProcRoot::at(&d);
    let mut out = Vec::new();
    for (pid, what, must_prove) in [
        (900u32, "foreign-uid ANCESTOR of this process", true),
        (500, "real uid 0", true),
        (600, "unparsable real uid", true),
        (700, "foreign real uid", false),
        (800, "our real uid", true),
    ] {
        let a = r.children_all_threads_proven(pid);
        let proven_path = matches!(&a, Err(ProcError::Unreadable(w)) if w.contains("cannot be proven"));
        out.push(format!("{pid} {what}: {a:?}"));
        if must_prove {
            assert!(proven_path, "{what}: must keep its proof (Unreadable): {a:?}");
        } else {
            assert!(
                matches!(&a, Ok(v) if !v.is_empty()),
                "{what}: read once, best effort: {a:?}"
            );
        }
    }
    eprintln!("G13 uid rule: {out:#?}");
    let _ = std::fs::remove_dir_all(&d);
}
