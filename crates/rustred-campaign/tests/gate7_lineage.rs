//! gate7 audit (WS/notes/implementer.gate7.md): what the parent-chain rule of procfix7 (resources::lineage) RELIES ON.
//! Pure tests on a fake proc tree (no process is spawned or signalled; no namespace needed). They pass on 452d0207: they
//! document the inputs whose completeness / integrity the rule needs, so that a fix of B1 (recover() ignores an error of
//! the ledger read that collects the sessions, supervisor/mod.rs:1372) can be checked against them.

use rustred_campaign::resources::lineage::{
    Hidden, LineageRules, RecordedSession, SessionLineage, classify_hidden_at,
};
use std::path::PathBuf;

struct Fake(PathBuf);

impl Fake {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "rr-g7-lineage-{tag}-{}-{}",
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
            format!("Name:\t{comm}\nPPid:\t{ppid}\nNSpid:\t{pid}\n"),
        )
        .unwrap();
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn lineage(pid: u32, start: u64, anc: &[(u32, u64)]) -> SessionLineage {
    SessionLineage {
        pid,
        start_ticks: start,
        boot_id: "b".into(),
        pid_ns: 7,
        ancestors: anc.to_vec(),
        time_ns: None, // procfix7 (gate7 MJ-2): field added
    }
}

fn session(pid: u32, start: u64) -> RecordedSession {
    RecordedSession {
        pid,
        start_ticks: start,
        boot_id: "b".into(),
    }
}

/// A crashed session S1 (pid 600, start 8000) left a daemonised, non-dumpable descendant D re-parented to S1's recorded
/// ancestor `systemd --user` (pid 700, a child subreaper started long before). With S1 in the session list D is
/// POSSIBLY OURS (inconclusive, named); when the list passed to `LineageRules::build` lacks S1 (what recover() builds
/// after a failed ledger read, B1) D is classified NOT OURS: the rule is only as conservative as the session list.
#[test]
fn g7_an_incomplete_session_list_turns_a_crashed_sessions_orphan_into_not_ours() {
    let f = Fake::new("incomplete");
    f.put(1, "systemd", 0, 1);
    f.put(700, "systemd", 1, 100); // systemd --user: S1's recorded ancestor (a subreaper)
    f.put(900, "rr_campaign", 950, 10_000); // live session S2
    f.put(950, "bash", 1, 9_000);
    f.put(2001, "daemon", 700, 8_500); // S1's orphan, re-parented to systemd --user
    let s1 = lineage(600, 8_000, &[(700, 100), (1, 1)]);
    let s2 = lineage(900, 10_000, &[(950, 9_000), (1, 1)]);
    let full = LineageRules::build(
        &[session(600, 8_000), session(900, 10_000)],
        &[s1.clone(), s2.clone()],
        &s2,
        true,
    )
    .unwrap();
    assert!(matches!(
        classify_hidden_at(
            &rustred_campaign::resources::ProcRoot::at(&f.0),
            2001,
            8_500,
            &full
        ),
        Hidden::Possible(_)
    ));
    // the session list of a failed read: only the live session (earliest = its start, no targets)
    let partial = LineageRules::build(
        &[session(900, 10_000)],
        std::slice::from_ref(&s2),
        &s2,
        true,
    )
    .unwrap();
    assert!(partial.targets.is_empty() && partial.earliest == 10_000);
    // D starts before the bound of the partial rule: it is not even a candidate of a strict scan any more; classified
    // anyway, it is NOT OURS (systemd --user is old and no target)
    assert!(matches!(
        classify_hidden_at(
            &rustred_campaign::resources::ProcRoot::at(&f.0),
            2001,
            8_500,
            &partial
        ),
        Hidden::NotOurs(_)
    ));
}

/// Notes are matched to sessions by (pid, start, boot) and the FIRST matching note wins: a note for a recorded session
/// that precedes the real one (only possible by rewriting the ledger's history: no API writes a note starting with
/// `session_lineage `) replaces its recorded ancestors. A note of another pid namespace for this boot turns the rule OFF
/// (more conservative); a duplicate note AFTER the real one changes nothing.
#[test]
fn g7_the_first_note_of_a_session_wins_and_other_namespaces_turn_the_rule_off() {
    let s1 = lineage(600, 8_000, &[(700, 100), (1, 1)]);
    let s2 = lineage(900, 10_000, &[(1, 1)]);
    let forged = lineage(600, 8_000, &[]);
    let sessions = [session(600, 8_000), session(900, 10_000)];
    let real_first = LineageRules::build(
        &sessions,
        &[s1.clone(), forged.clone(), s2.clone()],
        &s2,
        true,
    )
    .unwrap();
    assert!(real_first.targets.contains(&(700, 100)));
    let forged_first =
        LineageRules::build(&sessions, &[forged, s1.clone(), s2.clone()], &s2, true).unwrap();
    assert!(!forged_first.targets.contains(&(700, 100)));
    let mut other_ns = s1.clone();
    other_ns.pid_ns = 8;
    assert!(LineageRules::build(&sessions, &[other_ns, s1, s2.clone()], &s2, true).is_err());
}
