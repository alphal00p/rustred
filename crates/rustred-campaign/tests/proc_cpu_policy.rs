//! SitePolicy against a TEMPORARY tree of fake lock / ledger / held / pause files (never the real WS/ops), including two
//! policies in two processes competing for the same CPUs (the second process is this executable re-run as a helper).

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::cpu_policy::{
    Acquire, CpuPolicy, HeldFile, SITE_SCHEMA, SiteConfig, SitePolicy, ledger_claims,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Sixteen CPU numbers for the fake site (no process is pinned in these tests; the numbers only name lock files).
fn cpus16() -> Vec<usize> {
    let allowed = rustred_campaign::resources::allowed_cpus().unwrap();
    allowed
        .into_iter()
        .filter(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .take(16)
        .collect()
}

fn site_raw(root: &Path, cpus: &[usize]) -> SiteConfig {
    let list = cpus
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");
    SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rr-campaign-test".into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(list),
        exclude: vec![cpus[7].to_string()],
        held_files: vec![
            HeldFile {
                path: root.join("FOREIGN.held"),
                key: "held".into(),
                required: false,
            },
            HeldFile {
                path: root.join("OUR_PINS.json"),
                key: "entries[].cpus".into(),
                required: false,
            },
            HeldFile {
                path: root.join("absent.json"),
                key: "held".into(),
                required: false,
            },
        ],
        pause_files: vec![root.join("PAUSE")],
        idle_min: 0.0,
        max_cores: 16,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: true,
    }
}

fn write_tree(root: &Path, c: &[usize]) {
    fs::create_dir_all(root.join("locks")).unwrap();
    let ledger = format!(
        "# CPU ledger (fake)\n\
         2026-11-13T23:00:37Z (t162-cert): + CPU {a} (flock) -> worker. Released at its end.\n\
         2026-11-13T23:00:38Z (other-lane): + CPU {b} (flock) -> long job. Released at its end.\n\
         2026-11-13T23:00:39Z (t162-cert): RELEASED {a} (worker ended)\n",
        a = c[2],
        b = c[3]
    );
    fs::write(root.join("CPU_LEDGER.md"), ledger).unwrap();
    fs::write(
        root.join("FOREIGN.held"),
        format!("{{\"utc\":\"x\",\"held\":[{}],\"waiting\":[]}}", c[4]),
    )
    .unwrap();
    fs::write(
        root.join("OUR_PINS.json"),
        format!(
            "{{\"entries\":[{{\"owner\":\"x\",\"cpus\":\"{}-{}\"}}]}}",
            c[5], c[6]
        ),
    )
    .unwrap();
}

#[test]
fn helper_entry() {
    if std::env::var("RR_PROC_HELPER").as_deref() != Ok("compete") {
        return;
    }
    let root = std::path::PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
    let cfg: SiteConfig =
        serde_json::from_str(&fs::read_to_string(root.join("site.json")).unwrap()).unwrap();
    let mut p = SitePolicy::new(cfg).unwrap();
    let mut leases = Vec::new();
    // Grab CPUs one at a time until none is left, while the other process does the same.
    while let Acquire::Granted(l) = p.acquire(1, "helper").unwrap() {
        leases.push(l);
        std::thread::sleep(Duration::from_millis(20));
    }
    let got: Vec<String> = leases.iter().map(|l| l.cpus()[0].to_string()).collect();
    fs::write(root.join("helper.tmp"), got.join(",")).unwrap();
    fs::rename(root.join("helper.tmp"), root.join("helper.done")).unwrap();
    // Hold until the test is done (bounded).
    let t0 = std::time::Instant::now();
    while !root.join("release").exists() && t0.elapsed() < Duration::from_secs(300) {
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(leases);
}

#[test]
fn site_policy_rules_flocks_ledger_and_pause() {
    let _g = serial();
    let dir = TempDir::new("site");
    let c = cpus16();
    write_tree(dir.path(), &c);
    let mut p = SitePolicy::new(site(dir.path(), &c)).unwrap();
    // Eligible: all 16 minus exclude c7, held c4, pinned pool c5-c6, live foreign claim c3 (c2 was released).
    let expected: BTreeSet<usize> = c
        .iter()
        .copied()
        .filter(|x| ![c[3], c[4], c[5], c[6], c[7]].contains(x))
        .collect();
    let a = p.acquire(3, "job-a a1").unwrap().granted().expect("3 CPUs");
    for x in a.cpus() {
        assert!(expected.contains(x), "CPU {x} must be excluded");
    }
    // The flock is really held: another open file description cannot take it.
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join(format!("locks/cpu.{}", a.cpus()[0])))
        .unwrap();
    assert!(f.try_lock().is_err());
    // The claim is in the ledger in the existing format, parsable by the pick_free rules.
    let text = fs::read_to_string(dir.path().join("CPU_LEDGER.md")).unwrap();
    let claims = ledger_claims(text.lines());
    assert_eq!(
        claims["rr-campaign-test"],
        a.cpus().iter().copied().collect::<BTreeSet<_>>()
    );
    assert!(
        text.lines()
            .last()
            .unwrap()
            .contains("(rr-campaign-test): + CPUs "),
        "{text}"
    );
    // Never the same CPU twice: drain the rest one by one.
    let mut all: Vec<usize> = a.cpus().to_vec();
    let mut leases = vec![a];
    while let Acquire::Granted(l) = p.acquire(1, "drain").unwrap() {
        all.extend_from_slice(l.cpus());
        leases.push(l);
    }
    let set: BTreeSet<usize> = all.iter().copied().collect();
    assert_eq!(set.len(), all.len(), "a CPU was granted twice");
    assert_eq!(set, expected);
    // Release writes RELEASED and frees the flock.
    let first = leases.remove(0);
    let cpu0 = first.cpus()[0];
    first.release().unwrap();
    assert!(f.try_lock().is_ok());
    f.unlock().unwrap();
    let text = fs::read_to_string(dir.path().join("CPU_LEDGER.md")).unwrap();
    assert!(!ledger_claims(text.lines())["rr-campaign-test"].contains(&cpu0));
    drop(leases);
    assert_eq!(p.in_use(), 0);
    // Pause flag: nothing launches.
    fs::write(dir.path().join("PAUSE"), "").unwrap();
    assert!(!p.launch_blockers().is_empty());
    assert!(matches!(p.acquire(1, "x").unwrap(), Acquire::NotNow(r) if r.contains("PAUSE")));
    fs::remove_file(dir.path().join("PAUSE")).unwrap();
    // Optional second flag.
    p.add_pause_file(dir.path().join("CAMPAIGN_PAUSE"));
    fs::write(dir.path().join("CAMPAIGN_PAUSE"), "").unwrap();
    assert!(matches!(p.acquire(1, "x").unwrap(), Acquire::NotNow(_)));
    fs::remove_file(dir.path().join("CAMPAIGN_PAUSE")).unwrap();
    // A held file that exists but does not parse: fail closed.
    fs::write(dir.path().join("FOREIGN.held"), "{not json").unwrap();
    assert!(matches!(p.acquire(1, "x").unwrap(), Acquire::NotNow(r) if r.contains("FOREIGN.held")));
    write_tree(dir.path(), &c);
    // Budget.
    let mut cfg = site(dir.path(), &c);
    cfg.max_cores = 2;
    let mut q = SitePolicy::new(cfg).unwrap();
    let _l = q.acquire(2, "b").unwrap().granted().unwrap();
    assert!(matches!(q.acquire(1, "b").unwrap(), Acquire::NotNow(r) if r.contains("budget")));
    // Foreign pinning respected once fed, refused before.
    let mut cfg = site(dir.path(), &c);
    cfg.respect_foreign_pinning = true;
    let mut r = SitePolicy::new(cfg).unwrap();
    assert!(matches!(r.acquire(1, "c").unwrap(), Acquire::NotNow(_)));
    r.set_foreign_pinned(expected.clone());
    assert!(matches!(r.acquire(1, "c").unwrap(), Acquire::NotNow(_)));
}

#[test]
fn two_processes_compete_for_the_same_cpus() {
    let _g = serial();
    let dir = TempDir::new("compete");
    let c = cpus16();
    write_tree(dir.path(), &c);
    let cfg = site(dir.path(), &c);
    fs::write(
        dir.path().join("site.json"),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .unwrap();
    let mut l = launcher();
    let mut spec = helper_spec(dir.path(), "compete", "helper_entry", "compete");
    spec.env.push(("RR_PROC_OUT".into(), dir.path().into()));
    use rustred_campaign::launch::{ChildHandle, ChildLauncher};
    let mut h = l.launch(&spec).unwrap();
    let sid = h.identity().pid;
    // This process competes at the same time.
    let mut p = SitePolicy::new(cfg).unwrap();
    let mut mine = Vec::new();
    while let Acquire::Granted(x) = p.acquire(1, "main").unwrap() {
        mine.push(x);
        std::thread::sleep(Duration::from_millis(20));
    }
    wait_until("helper finished acquiring", LONG, || {
        dir.path().join("helper.done").exists()
    });
    // The helper may still have been acquiring when this loop ended: try again now that it has finished.
    while let Acquire::Granted(x) = p.acquire(1, "main").unwrap() {
        mine.push(x);
    }
    let theirs: BTreeSet<usize> = fs::read_to_string(dir.path().join("helper.done"))
        .unwrap()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect();
    let ours: BTreeSet<usize> = mine.iter().map(|x| x.cpus()[0]).collect();
    assert!(
        ours.is_disjoint(&theirs),
        "both processes hold {:?}",
        ours.intersection(&theirs).collect::<Vec<_>>()
    );
    let expected: BTreeSet<usize> = c
        .iter()
        .copied()
        .filter(|x| ![c[3], c[4], c[5], c[6], c[7]].contains(x))
        .collect();
    assert_eq!(
        ours.union(&theirs).copied().collect::<BTreeSet<_>>(),
        expected
    );
    // Both sets of claims are in the ledger under the same tag; the flock decided.
    let text = fs::read_to_string(dir.path().join("CPU_LEDGER.md")).unwrap();
    assert_eq!(ledger_claims(text.lines())["rr-campaign-test"], expected);
    fs::write(dir.path().join("release"), "").unwrap();
    let r = drive(&mut h);
    assert_eq!(r.exit.and_then(|e| e.code()), Some(0));
    drop(mine);
    let text = fs::read_to_string(dir.path().join("CPU_LEDGER.md")).unwrap();
    assert!(
        ledger_claims(text.lines())["rr-campaign-test"].is_empty(),
        "every claim released"
    );
    assert_session_gone(sid);
}

/// gate3 m3 (lane int5): the campaign never creates the site's files: a test site has its ledger and one lock file per
/// CPU (0..=1023 would be too many: the candidate CPUs and the excluded ones) before `SitePolicy::new`.
fn make_site_files(cfg: &SiteConfig, cpus: &[usize]) {
    std::fs::create_dir_all(&cfg.lock_dir).unwrap();
    if !cfg.ledger_file.exists() {
        std::fs::write(&cfg.ledger_file, b"").unwrap();
    }
    for c in cpus {
        let p = cfg.lock_dir.join(cfg.lock_name.replace("{n}", &c.to_string()));
        if !p.exists() {
            std::fs::write(&p, b"").unwrap();
        }
    }
}

fn site(root: &Path, cpus: &[usize]) -> SiteConfig {
    let c = site_raw(root, cpus);
    make_site_files(&c, cpus);
    c
}
