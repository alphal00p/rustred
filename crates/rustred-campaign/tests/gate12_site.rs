//! gate12 audit (WS/notes/implementer.gate12.md), item 2: the RESERVED CPUs of this host (168-175, 288-319, 192-215,
//! 258-260, 262, 271, 335) must never be granted by the site policy of RUNBOOK 1.7 (host-tests/host_step_fake1.sh) WHATEVER
//! the supervisor's affinity (on the host the supervisor runs in its scope without a taskset: affinity = every CPU) and
//! REGARDLESS of foreign-pin detection. PURE test: it spawns and signals nothing; it takes flocks on PRIVATE lock files and
//! writes a PRIVATE ledger (a temp directory); the held files are the host's (read only).
//!
//! - `g12_site_shipped_exclusions_hold_with_a_full_affinity`: the shipped site.json (lock_dir / ledger_file private, a lock
//!   file for every CPU 0-383; idle_min 0 and max_cores 400 so that EVERY grantable CPU is granted; respect_foreign_pinning
//!   true with an EMPTY pinned set): acquire one CPU at a time until nothing more: no reserved CPU, no CPU of the exclude
//!   lists, none of the held files.
//! - `g12_site_never_admits_reserved_only`: an affinity of only reserved CPUs is refused by `never_admits`.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

use rustred_campaign::cpu_policy::{Acquire, CpuPolicy, SiteConfig, SitePolicy, cpus_at_key};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const WS: &str = "/shared/vhirschi/t3code/vakint_dev/p_integral_dev";

fn cpus(s: &str) -> BTreeSet<usize> {
    let mut r = BTreeSet::new();
    for p in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match p.split_once('-') {
            Some((a, b)) => r.extend(a.parse::<usize>().unwrap()..=b.parse::<usize>().unwrap()),
            None => {
                r.insert(p.parse::<usize>().unwrap());
            }
        }
    }
    r
}

fn reserved() -> BTreeSet<usize> {
    cpus("168-175,288-319,192-215,258-260,262,271,335")
}

/// The site.json of host_step_fake1.sh (RUNBOOK 1.7) with a private lock directory and ledger.
fn shipped(dir: &Path, idle_min: f64, max_cores: usize) -> SiteConfig {
    let json = format!(
        r#"{{"schema":"rustred.campaign.site-policy.v1","tag":"rr-campaign",
 "lock_dir":"{d}/locks","lock_name":"cpu.{{n}}",
 "ledger_file":"{d}/CPU_LEDGER.md","ledger_scan_lines":1500,"candidates":null,
 "exclude":["168-175","288-319","128-162,164-166","192-215,258-260,262,271,335","342,362,363,378"],
 "held_files":[{{"path":"{WS}/ops/FOREIGN_5L_LIFT.held","key":"held"}},
               {{"path":"{WS}/ops/FOREIGN_5L_LIFT.json","key":"all_cpus_listed"}},
               {{"path":"{WS}/ops/OUR_PINS.json","key":"entries[].cpus"}}],
 "pause_files":["{d}/no-such-pause"],
 "idle_min":{idle_min},"max_cores":{max_cores},"respect_foreign_pinning":true,"ignore_own_tag_claims":true}}"#,
        d = dir.display()
    );
    serde_json::from_str(&json).unwrap()
}

fn private_site(tag: &str, cpus_with_lock: &BTreeSet<usize>) -> PathBuf {
    let d = std::env::temp_dir().join(format!("g12site-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("locks")).unwrap();
    for c in cpus_with_lock {
        std::fs::write(d.join("locks").join(format!("cpu.{c}")), "").unwrap();
    }
    std::fs::write(d.join("CPU_LEDGER.md"), "").unwrap();
    d
}

fn held_now() -> BTreeSet<usize> {
    let mut h = BTreeSet::new();
    for (f, k) in [
        ("ops/FOREIGN_5L_LIFT.held", "held"),
        ("ops/FOREIGN_5L_LIFT.json", "all_cpus_listed"),
        ("ops/OUR_PINS.json", "entries[].cpus"),
    ] {
        if let Ok(t) = std::fs::read_to_string(Path::new(WS).join(f))
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(&t)
        {
            h.extend(cpus_at_key(&v, k).unwrap_or_default());
        }
    }
    h
}

#[test]
fn g12_site_shipped_exclusions_hold_with_a_full_affinity() {
    if !Path::new(WS).join("ops/FOREIGN_5L_LIFT.json").exists() {
        eprintln!("SKIPPED: not on the host with WS/ops");
        return;
    }
    let all: BTreeSet<usize> = (0..384).collect();
    let d = private_site("full", &all);
    let mut p = SitePolicy::with_affinity(shipped(&d, 0.0, 400), &all).unwrap();
    assert!(p.never_admits().is_none());
    p.update_foreign_pinned(Some(BTreeSet::new()));
    let mut leases = Vec::new();
    let mut granted = BTreeSet::new();
    loop {
        match p.acquire(1, "g12-site").unwrap() {
            Acquire::Granted(l) => {
                granted.extend(l.cpus().iter().copied());
                leases.push(l);
            }
            Acquire::NotNow(why) => {
                eprintln!("G12SITE stop: {why}");
                break;
            }
        }
        assert!(granted.len() <= 384);
    }
    let excl: BTreeSet<usize> = [
        "168-175",
        "288-319",
        "128-162,164-166",
        "192-215,258-260,262,271,335",
        "342,362,363,378",
    ]
    .iter()
    .flat_map(|s| cpus(s))
    .collect();
    let held = held_now();
    eprintln!(
        "G12SITE granted {} CPUs (of 384; exclude lists {}, held files now {}): {:?}",
        granted.len(),
        excl.len(),
        held.len(),
        granted
    );
    assert!(!granted.is_empty());
    let resv = reserved();
    let r: Vec<_> = granted.intersection(&resv).collect();
    assert!(r.is_empty(), "RESERVED CPUs granted: {r:?}");
    let e: Vec<_> = granted.intersection(&excl).collect();
    assert!(e.is_empty(), "excluded CPUs granted: {e:?}");
    let h: Vec<_> = granted.intersection(&held).collect();
    assert!(h.is_empty(), "held CPUs granted: {h:?}");
    // every CPU not excluded / held was grantable (the exclusion is exact, nothing else silently lost)
    let expect: BTreeSet<usize> = all
        .difference(&excl)
        .copied()
        .filter(|c| !held.contains(c))
        .collect();
    assert_eq!(granted, expect);
    for l in leases {
        l.release().unwrap();
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn g12_site_never_admits_reserved_only() {
    if !Path::new(WS).join("ops/FOREIGN_5L_LIFT.json").exists() {
        eprintln!("SKIPPED: not on the host with WS/ops");
        return;
    }
    // an affinity of reserved CPUs only (e.g. a supervisor started under a taskset of the build slots)
    let d = private_site("resv", &(0..384).collect());
    let p = SitePolicy::with_affinity(shipped(&d, 0.85, 8), &reserved()).unwrap();
    let why = p.never_admits();
    eprintln!("G12SITE reserved-only affinity: never_admits = {why:?}");
    assert!(why.is_some());
    // lock files ONLY for reserved CPUs, full affinity
    let d2 = private_site("resvlocks", &reserved());
    let all: BTreeSet<usize> = (0..384).collect();
    let p2 = SitePolicy::with_affinity(shipped(&d2, 0.85, 8), &all).unwrap();
    let why2 = p2.never_admits();
    eprintln!("G12SITE lock files of reserved CPUs only: never_admits = {why2:?}");
    assert!(why2.is_some());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}
