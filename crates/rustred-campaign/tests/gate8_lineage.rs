//! gate8 audit (WS/notes/implementer.gate8.md), BLOCKER G8-B1: a FAILED READ during the parent-chain classification of
//! an unreadable candidate (resources::lineage) is taken for "the process is gone", and the strict scans drop it
//! (resources/marked.rs `Hidden::Gone => {}`). End to end (scripts/gate8/g8_inj_inner.sh, host-like namespace): the
//! crashed session's marked non-dumpable daemon survives a resumed run that ends with exit 0.
//! Pure tests on a fake proc tree (no process is spawned or signalled; no namespace needed). The first two tests state
//! the conservative behaviour and FAILED on ac/procfix7 a19d5d0b (they were `#[ignore]`d there); procfix7 round 4
//! (resources::procfs: Gone only for ENOENT / ESRCH) fixes them: un-ignored. The third shows the contrast (a failed read
//! of the PARENT is inconclusive, as intended).

use rustred_campaign::resources::lineage::{Hidden, LineageRules, classify_hidden_at};
use rustred_campaign::resources::{ProcError, ProcRoot, hidden_descendants_at};
use std::collections::BTreeSet;
use std::path::PathBuf;

struct Fake(PathBuf);

impl Fake {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "rr-g8-lineage-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn put(&self, pid: u32, comm: &str, ppid: u32, start: u64) {
        let d = self.0.join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("stat"),
            format!(
                "{pid} ({comm}) S {ppid} {pid} {pid} 0 -1 0 0 0 0 0 1 1 0 0 20 0 1 0 {start} 0 0\n"
            ),
        )
        .unwrap();
        std::fs::write(
            d.join("status"),
            format!("Name:\t{comm}\nPPid:\t{ppid}\nNSpid:\t{pid}\nUid:\t7\t7\t7\t7\n"),
        )
        .unwrap();
    }
    /// A live process whose `stat` / `status` cannot be READ (a directory: EISDIR, not ENOENT / ESRCH).
    fn unreadable(&self, pid: u32, file: &str) {
        let d = self.0.join(pid.to_string());
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join(file);
        let _ = std::fs::remove_file(&f);
        std::fs::create_dir_all(&f).unwrap();
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The crashed session (pid 50, start 100) below init; the live supervisor (pid 70, start 500).
fn rules() -> LineageRules {
    LineageRules {
        live: (70, 500),
        targets: BTreeSet::from([(50, 100), (1, 1)]),
        earliest: 100,
    }
}

/// G8-B1 (resources/lineage.rs stat_at / read_link): the candidate's own stat cannot be read (an error, not ENOENT):
/// the walk answers Gone and the strict scan drops the candidate. Conservative: Possible (inconclusive).
#[test]
fn g8_unreadable_stat_of_the_candidate_is_never_gone() {
    let f = Fake::new("cand");
    f.put(1, "init", 0, 1);
    f.put(50, "rr_campaign", 1, 100);
    f.unreadable(60, "stat"); // the crashed session's marked daemon (re-parented to init), stat read fails
    let h = classify_hidden_at(&ProcRoot::at(&f.0), 60, 200, &rules());
    assert!(
        matches!(h, Hidden::Possible(_)),
        "a candidate whose stat could not be read was classified {h:?} (the strict scan drops Gone)"
    );
}

/// G8-B1, second site (resources/marked.rs hidden_descendants_at): a descendant of the live supervisor whose parent's
/// status cannot be read is not listed (the chain walk breaks as 'not a descendant'). Conservative: listed.
#[test]
fn g8_unreadable_ancestor_status_keeps_the_descendant_listed() {
    let f = Fake::new("hd");
    f.put(100, "rr_campaign", 1, 500); // the live supervisor (the ancestor of the check)
    f.unreadable(101, "status"); // a job process whose status cannot be read
    f.put(102, "agent", 101, 600); // non-dumpable grandchild (uninspectable), our real uid
    let v = hidden_descendants_at(&ProcRoot::at(&f.0), 100, 7, &|p| {
        if p == 102 {
            Err(ProcError::Unreadable("refused".into()))
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert!(
        v.iter().any(|(p, _)| *p == 102),
        "an uninspectable descendant behind an unreadable ancestor was not listed: {v:?}"
    );
}

/// Contrast (passes): when the PARENT's stat cannot be read the walk is retried and ends Possible (intended).
#[test]
fn g8_unreadable_parent_stat_stays_inconclusive() {
    let f = Fake::new("par");
    f.put(1, "init", 0, 1);
    f.unreadable(55, "stat");
    f.put(60, "daemon", 55, 200);
    let h = classify_hidden_at(&ProcRoot::at(&f.0), 60, 200, &rules());
    assert!(matches!(h, Hidden::Possible(_)), "{h:?}");
}
