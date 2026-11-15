//! gate7 audit (WS/notes/implementer.gate7.md): reproductions of findings on ac/procfix7 452d0207. Spawns processes and
//! sends signals: ONLY inside a private pid namespace (`LIGHT_PIDNS=1 light_build.sh ...`, rule L10); every test refuses
//! to run outside one. The tests marked `#[ignore = "FAILS on 452d0207 ..."]` assert the CORRECT behaviour and fail on
//! the audited code (run them with `--ignored`); the others are regression tests that pass.
//! B1 (exit 0 with a marked survivor when one ledger read fails at recovery) needs a fault injector (LD_PRELOAD): its
//! reproduction is the script audit/gate7/scripts/g7_partial_inner.sh with audit/gate7/src/g7fault.c.

#[path = "sup_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::ledger::{Event, JobEvent};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn in_ns() -> bool {
    if rustred_campaign::launch::in_private_namespace() {
        return true;
    }
    eprintln!(
        "SKIPPED: spawns and signals processes: only inside a private pid namespace (rule L10)"
    );
    false
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Mount a private tmpfs at `dir` (the test's own mount namespace).
fn mount_tmpfs(dir: &Path, opts: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let c = std::ffi::CString::new(dir.to_string_lossy().as_bytes()).unwrap();
    let o = std::ffi::CString::new(opts).unwrap();
    // SAFETY: NUL-terminated strings; tmpfs may be mounted by the root of a user namespace.
    let r = unsafe {
        libc::mount(
            c"tmpfs".as_ptr(),
            c.as_ptr(),
            c"tmpfs".as_ptr(),
            0,
            o.as_ptr().cast(),
        )
    };
    assert_eq!(r, 0, "tmpfs mount: {}", std::io::Error::last_os_error());
}

fn umount(dir: &Path) {
    let c = std::ffi::CString::new(dir.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: unmount our own test mount.
    unsafe { libc::umount2(c.as_ptr(), libc::MNT_DETACH) };
}

fn dev_ino(p: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(p).unwrap();
    format!("{}-{}", m.dev(), m.ino())
}

fn started_count(d: &Path) -> usize {
    if !d.join("ledger").join("MANIFEST.json").exists() {
        return 0;
    }
    count(d, |e| {
        matches!(
            e,
            Event::Job {
                ev: JobEvent::Started { .. },
                ..
            }
        )
    })
}

/// gate7 finding m-fb (guard.rs intent_fallback_dir / supervisor reconcile_intents): the tmpfs fallback intent logs
/// are keyed only by the campaign directory's <dev>-<ino>, and the restart merges every `*.jsonl` there WITHOUT
/// checking that the intents belong to this campaign. A new campaign directory that gets the inode number of a removed
/// one (ZFS / ext4 recycle object numbers) records the OTHER campaign's intents as its own `signal_sent` ("POSSIBLY
/// SENT ... of an earlier session") at its FIRST start. Simulated here by renaming A's key directory to B's key.
#[test]
#[ignore = "FAILS on 452d0207: gate7 m-fb (a stale fallback intent log of another campaign is merged into this ledger)"]
fn g7_fallback_intent_log_of_another_campaign_is_never_recorded() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g7-fb");
    let xdg = t.path.join("xdg");
    mount_tmpfs(&xdg, "size=4m,mode=0700");
    let x = xdg.to_str().unwrap().to_string();
    // campaign A: its campaign-directory log is full, the first send goes through the fallback, then it aborts
    let va = spec(
        &t,
        serde_json::json!({"items": 1, "nodes": 1, "rungs": 1, "deps": 0.0, "case_ms": [600000, 600000],
            "seed": 9, "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0}),
        serde_json::json!({"grace_seconds": 2}),
    );
    let sa = write_spec(&t, "a.json", &va);
    let a = t.cdir("a");
    let mut c = start(
        &sa,
        &a,
        &[],
        &[
            ("XDG_RUNTIME_DIR", &x),
            (
                "RR_CAMPAIGN_FAILPOINT",
                "intent_primary_enospc:1,signal_after_send:1",
            ),
        ],
    );
    wait_until("a started job of A", Duration::from_secs(120), || {
        started_count(&a) >= 1
    });
    std::thread::sleep(Duration::from_secs(1));
    signal(&c, libc::SIGINT);
    assert_eq!(
        wait(&mut c, Duration::from_secs(120)),
        134,
        "{}",
        log_tail(&a)
    );
    let base = std::fs::read_dir(&xdg)
        .unwrap()
        .flatten()
        .next()
        .expect("fallback base directory")
        .path();
    let ka = base.join(dev_ino(&a));
    assert!(ka.is_dir(), "A's fallback key directory {}", ka.display());
    // campaign B: a NEW directory whose <dev>-<ino> key A's fallback now carries (inode number recycled)
    let mut vb = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 10, "case_ms": [200, 400],
            "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0}),
        serde_json::json!({}),
    );
    vb["campaign"] = serde_json::json!(format!("{}-b", t.campaign));
    let sb = write_spec(&t, "b.json", &vb);
    let b = t.cdir("b");
    std::fs::create_dir_all(&b).unwrap();
    std::fs::rename(&ka, base.join(dev_ino(&b))).unwrap();
    let mut c2 = start(&sb, &b, &[], &[("XDG_RUNTIME_DIR", &x)]);
    let code = wait(&mut c2, Duration::from_secs(240));
    assert!(finished(code), "{code}: {}", log_tail(&b));
    let foreign: Vec<String> = records(&b)
        .into_iter()
        .filter_map(|r| match r.event {
            Event::SignalSent { pid, by, .. } if by.contains("recovered at restart") => {
                Some(format!("pid {pid}: {by}"))
            }
            _ => None,
        })
        .collect();
    assert!(
        foreign.is_empty(),
        "B's FIRST start recorded signals of another campaign as its own: {foreign:?}"
    );
    assert_eq!(verify(&b), 0);
    umount(&xdg);
}

/// gate7 finding m-br (supervisor barrier_refused / launch::barrier_shim): the "spawn barrier refused" evidence is a
/// file in the job's OWN run directory (RUN/sup/BARRIER_REFUSED), which the program that RAN can write. A program that
/// writes it and exits 125 is recorded as "the program never ran" (row 15s, a typed retry, not a crash), so the crash
/// rules are evaded and the ledger states something false. Here every first attempt of a job leaves a mark that it
/// ran and then fakes the refusal.
#[test]
#[ignore = "FAILS on 452d0207: gate7 m-br (the job program can fake 'the spawn barrier refused: the program never ran')"]
fn g7_a_program_that_ran_is_never_recorded_as_never_ran() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    let t = Tmp::new("g7-br");
    let ran = t.path.join("ran.log");
    let wrap = script(
        &t.path,
        "wrap.sh",
        &format!(
            "#!/bin/sh\nif [ \"$1\" = job ]; then\n  run=$(dirname \"$2\")\n  case \"$run\" in *.a1)\n    echo \"$run\" >> {ran}\n    \
             printf 'eof: faked by the program\\n' > \"$run/sup/BARRIER_REFUSED\"\n    exit 125;;\n  esac\nfi\nexec {fake} \"$@\"\n",
            ran = ran.display(),
            fake = FAKE
        ),
    );
    let mut v = spec(
        &t,
        serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": 4, "case_ms": [300, 600],
            "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0}),
        serde_json::json!({"max_jobs": 2}),
    );
    v["programs"]["rr_fake_child"]["path"] = serde_json::json!(wrap);
    let sp = write_spec(&t, "spec.json", &v);
    let d = t.cdir("c");
    let mut c = start(&sp, &d, &["--fake-child"], &[]);
    let code = wait(&mut c, Duration::from_secs(240));
    let runs = std::fs::read_to_string(&ran).unwrap_or_default();
    assert!(!runs.is_empty(), "the programs ran: {}", log_tail(&d));
    let never_ran = count(
        &d,
        |e| matches!(e, Event::Job { ev: JobEvent::Requeue { reason }, .. } if reason.contains("row 15s")),
    );
    assert_eq!(
        never_ran,
        0,
        "attempts whose program RAN ({} runs) recorded as 'never ran' (row 15s); exit {code}: {}",
        runs.lines().count(),
        log_tail(&d)
    );
}

/// gate7 MJ-B regression (passes on 452d0207): two DIFFERENT campaign directories with the same external work_dir,
/// started at the same moment, 5 rounds: exactly one claims the work directory and runs; the other is refused (exit 1)
/// naming the owner; no run directory of the refused one exists.
#[test]
fn g7_two_campaigns_racing_for_one_work_dir_exactly_one_wins() {
    if !in_ns() {
        return;
    }
    let _g = serial();
    for round in 0..5 {
        let t = Tmp::new(&format!("g7-race{round}"));
        let w = t.path.join("work");
        let mk = |name: &str, seed: u64| {
            let mut v = spec(
                &t,
                serde_json::json!({"items": 2, "nodes": 1, "rungs": 1, "seed": seed, "case_ms": [200, 400],
                    "p_mem": 0.0, "p_crash": 0.0, "p_norule": 0.0}),
                serde_json::json!({"max_jobs": 2}),
            );
            v["campaign"] = serde_json::json!(format!("{}-{name}", t.campaign));
            v["dirs"] = serde_json::json!({"work_dir": w});
            write_spec(&t, &format!("{name}.json"), &v)
        };
        let (sa, sb) = (mk("a", 3), mk("b", 4));
        let (a, b) = (t.cdir("a"), t.cdir("b"));
        let mut ca = start(&sa, &a, &[], &[]);
        let mut cb = start(&sb, &b, &[], &[]);
        let (ra, rb) = (
            wait(&mut ca, Duration::from_secs(240)),
            wait(&mut cb, Duration::from_secs(240)),
        );
        let owner = std::fs::read_to_string(w.join("RR_CAMPAIGN_WORK_OWNER")).unwrap_or_default();
        let refused = |d: &Path| {
            std::fs::read_to_string(d.with_extension("log"))
                .unwrap_or_default()
                .contains("belongs to campaign")
        };
        assert!(
            (finished(ra) && rb == 1 && refused(&b)) || (finished(rb) && ra == 1 && refused(&a)),
            "round {round}: rc A {ra}, rc B {rb}, owner {owner:?}"
        );
        let winner = if ra == 1 { &b } else { &a };
        let id: String = std::fs::read_dir(winner.join("ledger"))
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect();
        assert!(
            !owner.trim().is_empty() && id.contains(owner.trim()),
            "round {round}: the owner file names the winner ({owner:?})"
        );
    }
}
