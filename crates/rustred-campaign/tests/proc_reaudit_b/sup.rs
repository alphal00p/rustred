//! SUPERVISOR side of each scenario: drives the process layer through its PUBLIC API only.
use crate::{CAMP, alive, exe, hex, my_zombies, rb_args, read_u32_file, stat, which, write_atomic};
use rustred_campaign::cpu_policy::{
    CpuPolicy, CpuSpec, HeldFile, SITE_SCHEMA, SiteConfig, SitePolicy,
};
use rustred_campaign::launch::{
    CapPreference, CapRequest, ChildHandle, ChildLauncher, ChildProgress, FinalReport, KillReason,
    LaunchSpec, PidfdProbe, RealConfig, RealLauncher, ReattachChoice, Reattached, Refusal,
    RetireEvent, Verdict, Verification, kill_orphan, pidfd_probe, pidfd_supported,
    signal_verified_process,
};
use rustred_campaign::resources::{
    CampaignMarker, InvalidPid, MarkedProcess, ProcessIdentity, Sampler, SamplerConfig, ValidPid,
    boot_id, parse_cpu_list, pid_max, scan_campaign,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Ctx {
    scen: String,
    run: PathBuf,
    n: u32,
    pass: u32,
    fail: u32,
    note: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum V {
    Pass,
    Fail,
    Note,
}

impl Ctx {
    fn res(&mut self, case: &str, expected: &str, observed: impl AsRef<str>, v: V) {
        let tag = match v {
            V::Pass => {
                self.pass += 1;
                "PASS"
            }
            V::Fail => {
                self.fail += 1;
                "FAIL"
            }
            V::Note => {
                self.note += 1;
                "NOTE"
            }
        };
        println!(
            "RESULT {} | {case} | expected: {expected} | observed: {} | {tag}",
            self.scen,
            observed.as_ref()
        );
    }
    fn chk(&mut self, case: &str, expected: &str, observed: impl AsRef<str>, ok: bool) {
        self.res(case, expected, observed, if ok { V::Pass } else { V::Fail });
    }
    fn req(&mut self, mut v: Value) -> Value {
        self.n += 1;
        let name = format!("{:05}-{}.json", self.n, std::process::id());
        v["kind"] = v.get("kind").cloned().unwrap_or(json!("decoy"));
        write_atomic(&self.run.join("req").join(&name), &v.to_string());
        let rep = self.run.join("rep").join(&name);
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(120) {
            if let Ok(t) = std::fs::read_to_string(&rep)
                && let Ok(v) = serde_json::from_str::<Value>(&t)
            {
                return v;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("driver did not answer {name}");
    }
    /// Ask the driver (pid 1) for a decoy; returns (pid, start).
    fn decoy(&mut self, spec: Value) -> (u32, u64) {
        let r = self.req(spec);
        assert!(
            r["ok"].as_bool().unwrap_or(false),
            "decoy spawn failed: {r}"
        );
        (
            r["pid"].as_u64().unwrap() as u32,
            r["start"].as_u64().unwrap(),
        )
    }
    fn register(&mut self, name: &str, class: &str, pid: u32, start: u64) {
        self.req(
            json!({"kind": "register", "name": name, "class": class, "pid": pid, "start": start}),
        );
    }
}

fn ds(name: &str, class: &str, env: &[&[u8]], setsid: bool) -> Value {
    json!({"name": name, "class": class, "env": env.iter().map(|e| hex(e)).collect::<Vec<_>>(), "setsid": setsid})
}

fn sampler(ms: u64) -> Arc<Sampler> {
    Arc::new(
        Sampler::spawn(SamplerConfig {
            interval: Duration::from_millis(ms),
            collect_foreign_pinning: false,
        })
        .unwrap(),
    )
}

fn launcher(camp: &str) -> RealLauncher {
    launcher_ms(camp, 200)
}

fn launcher_ms(camp: &str, ms: u64) -> RealLauncher {
    let c = RealConfig {
        campaign_id: camp.to_string(),
        cap_preference: CapPreference::NoSystemd,
        verify_timeout: Duration::from_secs(30),
        ..RealConfig::default()
    };
    RealLauncher::with_config(sampler(ms), c)
}

fn sh(run: &Path, label: &str, script: &str) -> LaunchSpec {
    let mut s = LaunchSpec::new(label, "/bin/sh", run).arg("-c").arg(script);
    s.env
        .push(("PATH".into(), std::env::var_os("PATH").unwrap()));
    s.stdout = run.join(format!("job-{}.out", sanitize(label)));
    s.stderr = run.join(format!("job-{}.err", sanitize(label)));
    s
}

fn sanitize(l: &str) -> String {
    l.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn helper_spec(run: &Path, label: &str, args: &[&str]) -> LaunchSpec {
    let mut s = LaunchSpec::new(label, exe(), run);
    let mut v = vec!["helper".to_string()];
    v.extend(args.iter().map(|a| a.to_string()));
    for a in rb_args(&v) {
        s = s.arg(a);
    }
    s.env
        .push(("PATH".into(), std::env::var_os("PATH").unwrap()));
    s.stdout = run.join(format!("job-{}.out", sanitize(label)));
    s.stderr = run.join(format!("job-{}.err", sanitize(label)));
    s
}

fn drive<H: ChildHandle>(h: &mut H, t: Duration) -> Option<FinalReport> {
    let t0 = Instant::now();
    while t0.elapsed() < t {
        match h.poll() {
            Ok(ChildProgress::Finished(r)) => return Some(r),
            Ok(_) => {}
            Err(e) => println!("  poll error: {e}"),
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn rs(r: &FinalReport) -> String {
    let ver = match &r.verification {
        Verification::Verified { by } => format!("Verified({by:?})"),
        Verification::Unverified { leftovers, reason } => {
            format!("Unverified({reason}; {} left)", leftovers.len())
        }
    };
    format!(
        "exit={:?} kill_reason={:?} term={} kill={} {ver} leftovers_killed={:?} escaped={:?} mech={:?}",
        r.exit,
        r.kill_reason,
        r.sigterm_sent,
        r.sigkill_sent,
        r.leftovers_killed.iter().map(|l| l.pid).collect::<Vec<_>>(),
        r.escaped
            .iter()
            .map(|e| (e.pid, e.alive))
            .collect::<Vec<_>>(),
        r.memory_mechanism
    )
}

fn mp(pid: u32, start: u64, attempt: &str) -> MarkedProcess {
    MarkedProcess {
        pid,
        start_ticks: start,
        attempt_id: attempt.into(),
        ppid: 0,
        pgrp: pid,
        session: pid,
        comm: "x".into(),
        state: 'S',
        rss_bytes: 0,
        cpu_ticks: 0,
    }
}

fn invalid_pid_err(e: &std::io::Error) -> bool {
    e.get_ref()
        .is_some_and(|x| x.downcast_ref::<InvalidPid>().is_some())
}

fn is_invalid(v: &Verdict) -> bool {
    matches!(v, Verdict::NotOurs(Refusal::InvalidPid(_)))
}

fn reat<H>(r: &std::io::Result<Reattached<H>>) -> String {
    match r {
        Err(e) => format!("Err({e})"),
        Ok(Reattached::Lost(l)) => format!("Lost({l:?})"),
        Ok(Reattached::Live(_)) => "LIVE".into(),
    }
}

fn wait_scan(l: &RealLauncher) {
    let _ = l
        .sampler()
        .wait_for_scan_after(Instant::now(), Duration::from_secs(10));
    let _ = l
        .sampler()
        .wait_for_scan_after(Instant::now(), Duration::from_secs(10));
}

fn nofile() -> (u64, u64) {
    // SAFETY: getrlimit with a valid out-pointer.
    let mut r: libc::rlimit = unsafe { std::mem::zeroed() };
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) };
    (r.rlim_cur, r.rlim_max)
}

fn set_nofile(cur: u64, max: u64) {
    let r = libc::rlimit {
        rlim_cur: cur,
        rlim_max: max,
    };
    // SAFETY: setrlimit with a valid pointer (soft <= hard).
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &r) };
}

/// Exhaust the descriptor table: soft RLIMIT_NOFILE = fds in use + 8, then open /dev/null until EMFILE.
/// The caller restores with `set_nofile(cur, max)` after dropping the returned files.
fn exhaust_fds() -> Vec<std::fs::File> {
    let (_, max) = nofile();
    let n = std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count())
        .unwrap_or(64) as u64;
    set_nofile(n + 8, max);
    let mut held = vec![];
    while let Ok(f) = std::fs::File::open("/dev/null") {
        held.push(f);
        if held.len() > 10_000 {
            break;
        }
    }
    held
}

// ------------------------------------------------------------------------------------------------ section 1

fn s_pid1(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let s1 = stat(1).unwrap().start_ticks;
    let env1 = std::fs::read("/proc/1/environ").unwrap_or_default();
    let has = env1
        .split(|b| *b == 0)
        .any(|kv| kv == b"RR_CAMPAIGN_JOB=rbcamp:L1");
    c.res(
        "pid 1 (driver) carries THIS campaign's marker rbcamp:L1",
        "true (adversarial set-up)",
        format!("{has}"),
        if has { V::Pass } else { V::Note },
    );
    let me = std::process::id();
    let st = stat(me).unwrap();
    c.res(
        "pid 1 is not my parent / session / group",
        "true",
        format!("ppid={} sid={} pgrp={}", st.ppid, st.session, st.pgrp),
        if st.ppid != 1 && st.session != 1 && st.pgrp != 1 {
            V::Pass
        } else {
            V::Fail
        },
    );
    let j = format!(r#"{{"pid":1,"start_ticks":{s1},"boot_id":"{boot}"}}"#);
    let r = serde_json::from_str::<ProcessIdentity>(&j);
    c.chk(
        "serde ProcessIdentity pid 1",
        "rejected",
        format!("{:?}", r.as_ref().err().map(|e| e.to_string())),
        r.is_err(),
    );
    let r = ProcessIdentity::capture(1);
    c.chk(
        "ProcessIdentity::capture(1)",
        "error",
        format!("{:?}", r.as_ref().err()),
        r.is_err(),
    );
    let id = ProcessIdentity {
        pid: 1,
        start_ticks: s1,
        boot_id: boot.clone(),
    };
    let r = id.check();
    c.chk(
        "literal identity pid 1 (real start time) .check()",
        "error",
        format!("{r:?}"),
        r.is_err(),
    );
    let m = CampaignMarker::new(CAMP, "L1").unwrap();
    for sig in [0, libc::SIGUSR1, libc::SIGTERM, libc::SIGKILL] {
        let v = signal_verified_process(&id, &m, sig);
        c.chk(
            &format!("signal_verified_process(pid 1, correct marker, sig {sig})"),
            "NotOurs(InvalidPid)",
            format!("{v:?}"),
            is_invalid(&v),
        );
        let v = kill_orphan(CAMP, &boot, &mp(1, s1, "L1"), sig);
        c.chk(
            &format!("kill_orphan(pid 1, sig {sig})"),
            "NotOurs(InvalidPid)",
            format!("{v:?}"),
            is_invalid(&v),
        );
    }
    let mut l = launcher(CAMP);
    for ch in [
        ReattachChoice::Kill {
            grace: Duration::ZERO,
        },
        ReattachChoice::Adopt,
    ] {
        let r = l.reattach(&id, "L1", ch);
        let ok = matches!(&r, Err(e) if invalid_pid_err(e));
        c.chk(
            &format!("reattach(pid 1, {ch:?})"),
            "Err(InvalidPid)",
            reat(&r),
            ok,
        );
    }
    let sc = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    c.chk(
        "scan_campaign never lists pid 1",
        "no pid 1",
        format!(
            "{:?}",
            sc.processes.iter().map(|p| p.pid).collect::<Vec<_>>()
        ),
        !sc.processes.iter().any(|p| p.pid == 1),
    );
    wait_scan(&l);
    let snap = l.sampler().latest().unwrap();
    c.chk(
        "sampler Snapshot::marked never lists pid 1",
        "no pid 1",
        format!(
            "{:?}",
            snap.marked.iter().map(|p| p.pid).collect::<Vec<_>>()
        ),
        !snap.marked.iter().any(|p| p.pid == 1),
    );
    c.chk(
        "ValidPid::new(1) / for_signal(1)",
        "both Err",
        format!(
            "{:?} {:?}",
            ValidPid::new(1).is_err(),
            ValidPid::for_signal(1).is_err()
        ),
        ValidPid::new(1).is_err() && ValidPid::for_signal(1).is_err(),
    );
    // own pid, parent, own session leader, own group leader
    for (what, p) in [
        ("self", me),
        ("parent", st.ppid),
        ("own session leader", st.session),
        ("own group leader", st.pgrp),
    ] {
        let v = match ProcessIdentity::capture(p) {
            Ok(idp) => signal_verified_process(&idp, &m, libc::SIGTERM),
            Err(e) => Verdict::Unverified(format!("capture: {e}")),
        };
        c.chk(
            &format!("signal_verified_process({what} = pid {p})"),
            "refused",
            format!("{v:?}"),
            !v.delivered(),
        );
    }
    // a job in the same launcher to make sure the launcher itself is functional here
    let mut h = l
        .launch(&sh(&c.run, "pid1-job", "exec sleep 3000"))
        .unwrap();
    h.terminate(Duration::from_millis(500), KillReason::Stop);
    let r = drive(&mut h, Duration::from_secs(30));
    c.chk(
        "control: own job terminated & verified",
        "Verified",
        r.as_ref().map(rs).unwrap_or("timeout".into()),
        r.is_some_and(|r| r.verification.is_verified()),
    );
    std::thread::sleep(Duration::from_millis(500));
}

fn s_pid0(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    {
        let j = r#"{"pid":0,"start_ticks":0,"boot_id":"B"}"#.replace('B', &boot);
        let r = serde_json::from_str::<ProcessIdentity>(&j);
        c.chk(
            "serde pid 0",
            "rejected",
            format!("{:?}", r.as_ref().err().map(|e| e.to_string())),
            r.is_err(),
        );
    }
    let r = ProcessIdentity::capture(0);
    c.chk(
        "capture(0)",
        "error",
        format!("{:?}", r.as_ref().err()),
        r.is_err(),
    );
    let mut l = launcher(CAMP);
    for start in [0u64, 1, stat(std::process::id()).unwrap().start_ticks] {
        let id = ProcessIdentity {
            pid: 0,
            start_ticks: start,
            boot_id: boot.clone(),
        };
        c.chk(
            &format!("literal pid 0 start {start} .check()"),
            "error",
            format!("{:?}", id.check()),
            id.check().is_err(),
        );
        let m = CampaignMarker::new(CAMP, "L0").unwrap();
        for sig in [0, libc::SIGTERM, libc::SIGKILL] {
            let v = signal_verified_process(&id, &m, sig);
            c.chk(
                &format!("signal_verified_process(pid 0, start {start}, sig {sig})"),
                "NotOurs(InvalidPid)",
                format!("{v:?}"),
                is_invalid(&v),
            );
            let v = kill_orphan(CAMP, &boot, &mp(0, start, "L0"), sig);
            c.chk(
                &format!("kill_orphan(pid 0, sig {sig})"),
                "NotOurs(InvalidPid)",
                format!("{v:?}"),
                is_invalid(&v),
            );
        }
        for ch in [
            ReattachChoice::Kill {
                grace: Duration::ZERO,
            },
            ReattachChoice::Adopt,
        ] {
            let r = l.reattach(&id, "L0", ch);
            let ok = matches!(&r, Err(e) if invalid_pid_err(e));
            c.chk(
                &format!("reattach(pid 0, start {start}, {ch:?})"),
                "Err(InvalidPid)",
                reat(&r),
                ok,
            );
        }
    }
    wait_scan(&l);
    std::thread::sleep(Duration::from_millis(500));
}

fn s_bigpid(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let pm = pid_max();
    println!("  pid_max here = {pm}");
    for raw in [
        "-1".to_string(),
        "-2147483648".into(),
        "0".into(),
        "1".into(),
        "2147483647".into(),
        "2147483648".into(),
        "4294967295".into(),
        "4294967296".into(),
        "4294967298".into(),
        format!("{}", pm + 1),
        "9223372036854775807".into(),
        "-9223372036854775808".into(),
        "1.5".into(),
        "\"7\"".into(),
        "null".into(),
    ] {
        let j = format!(r#"{{"pid":{raw},"start_ticks":5,"boot_id":"{boot}"}}"#);
        let r = serde_json::from_str::<ProcessIdentity>(&j);
        c.chk(
            &format!("serde pid {raw}"),
            "rejected",
            format!("{:?}", r.as_ref().map(|i| i.pid).map_err(|e| e.to_string())),
            r.is_err(),
        );
    }
    for v in [
        i64::MIN,
        -(1i64 << 31),
        -1,
        0,
        1,
        i64::from(pm) + 1,
        1i64 << 31,
        (1i64 << 32) + 5,
        i64::MAX,
    ] {
        c.chk(
            &format!("ValidPid::new({v})"),
            "Err",
            format!("{:?}", ValidPid::new(v)),
            ValidPid::new(v).is_err(),
        );
    }
    let mut l = launcher(CAMP);
    let m = CampaignMarker::new(CAMP, "big").unwrap();
    for p in [
        0u32,
        1,
        pm + 1,
        (1u32 << 31),
        (1u32 << 31) + 5,
        u32::MAX,
        u32::MAX - 1,
    ] {
        let id = ProcessIdentity {
            pid: p,
            start_ticks: 5,
            boot_id: boot.clone(),
        };
        let v = signal_verified_process(&id, &m, libc::SIGKILL);
        c.chk(
            &format!("signal_verified_process(pid {p})"),
            "NotOurs(InvalidPid)",
            format!("{v:?}"),
            is_invalid(&v),
        );
        let v = kill_orphan(CAMP, &boot, &mp(p, 5, "big"), libc::SIGKILL);
        c.chk(
            &format!("kill_orphan(pid {p})"),
            "NotOurs(InvalidPid)",
            format!("{v:?}"),
            is_invalid(&v),
        );
        let r = l.reattach(
            &id,
            "big",
            ReattachChoice::Kill {
                grace: Duration::ZERO,
            },
        );
        let ok = matches!(&r, Err(e) if invalid_pid_err(e));
        c.chk(
            &format!("reattach(pid {p}, Kill)"),
            "Err(InvalidPid)",
            reat(&r),
            ok,
        );
        c.chk(
            &format!("check(pid {p})"),
            "error",
            format!("{:?}", id.check()),
            id.check().is_err(),
        );
    }
    std::thread::sleep(Duration::from_millis(500));
}

/// Launch a job, end it verified, return its identity (its pid is then free for a forced decoy).
fn record_and_end(c: &mut Ctx, l: &mut RealLauncher, label: &str) -> ProcessIdentity {
    let mut h = l.launch(&sh(&c.run, label, "exec sleep 3000")).unwrap();
    let id = h.identity().clone();
    h.kill_now(KillReason::Other("test".into()));
    let r = drive(&mut h, Duration::from_secs(30)).expect("job end");
    assert!(r.verification.is_verified());
    assert!(
        crate::wait_for(Duration::from_secs(20), || stat(id.pid).is_none()),
        "pid not freed"
    );
    id
}

fn try_reattach_all(c: &mut Ctx, id: &ProcessIdentity, label: &str, case: &str, expect_lost: bool) {
    let mut l2 = launcher(CAMP);
    for ch in [
        ReattachChoice::Kill {
            grace: Duration::ZERO,
        },
        ReattachChoice::Adopt,
    ] {
        let r = l2.reattach(id, label, ch);
        let s = reat(&r);
        let lost = matches!(r, Ok(Reattached::Lost(_)));
        if let Ok(Reattached::Live(mut h)) = r {
            if ch == ReattachChoice::Adopt {
                let _ = h.signal_group(libc::SIGTERM);
                h.terminate(Duration::ZERO, KillReason::Stop);
                h.kill_now(KillReason::Reattach);
            }
            let rr = drive(&mut h, Duration::from_secs(20));
            println!(
                "  adopted handle ended: {}",
                rr.as_ref().map(rs).unwrap_or("timeout".into())
            );
        }
        c.res(
            &format!("{case}: reattach {ch:?}"),
            if expect_lost {
                "Lost"
            } else {
                "Live (by design: carries marker)"
            },
            s,
            if lost == expect_lost {
                V::Pass
            } else if expect_lost {
                V::Fail
            } else {
                V::Note
            },
        );
    }
    let m = CampaignMarker::new(CAMP, label).unwrap();
    let v = signal_verified_process(id, &m, libc::SIGTERM);
    c.chk(
        &format!("{case}: signal_verified_process(recorded id)"),
        "refused (start mismatch)",
        format!("{v:?}"),
        !v.delivered(),
    );
}

fn s_reuse(c: &mut Ctx) {
    let sleep = which("sleep").display().to_string();
    let shp = which("sh").display().to_string();
    let mut l = launcher(CAMP);
    let rounds: Vec<(&str, Vec<Vec<u8>>, &str, bool)> = vec![
        ("unmarked", vec![], "decoy", true),
        (
            "other_campaign",
            vec![b"RR_CAMPAIGN_JOB=othercamp:LBL".to_vec()],
            "decoy",
            true,
        ),
        (
            "ours_other_attempt",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:other".to_vec()],
            "decoy",
            true,
        ),
        (
            "prefix_id",
            vec![b"RR_CAMPAIGN_JOB=rbcampX:LBL".to_vec()],
            "decoy",
            true,
        ),
        (
            "ours_same_attempt",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:LBL".to_vec()],
            "bydesign",
            false,
        ),
        ("leaderless_unmarked", vec![], "decoy", true),
        (
            "leaderless_other",
            vec![b"RR_CAMPAIGN_JOB=othercamp:LBL".to_vec()],
            "decoy",
            true,
        ),
    ];
    for (i, (name, env, class, expect_lost)) in rounds.into_iter().enumerate() {
        let label = format!("ru{i}");
        let env: Vec<Vec<u8>> = env
            .into_iter()
            .map(|e| {
                String::from_utf8(e)
                    .unwrap()
                    .replace("LBL", &label)
                    .into_bytes()
            })
            .collect();
        let id = record_and_end(c, &mut l, &label);
        let mf = c.run.join(format!("ru{i}.mpid"));
        let script = if name.starts_with("leaderless") {
            format!("{sleep} 3000 & echo $! > {}; exit 0", mf.display())
        } else {
            format!(
                "{sleep} 3000 & echo $! > {}; exec {sleep} 3000",
                mf.display()
            )
        };
        let r = c.req(json!({"name": format!("reuse:{name}"), "class": class, "argv": [shp, "-c", script],
            "env": env.iter().map(|e| hex(e)).collect::<Vec<_>>(), "setsid": true, "force_pid": id.pid,
            "member_pidfile": mf.display().to_string(), "leader_class": if name.starts_with("leaderless") { "exits" } else { class }}));
        if !r["ok"].as_bool().unwrap_or(false) {
            c.res(
                &format!("reuse {name}"),
                "decoy on recorded pid",
                "could not force the pid",
                V::Note,
            );
            continue;
        }
        std::thread::sleep(Duration::from_millis(400));
        let st = stat(id.pid);
        println!(
            "  round {name}: recorded pid {} start {} now held by start {:?} sid {:?}",
            id.pid,
            id.start_ticks,
            st.as_ref().map(|s| s.start_ticks),
            st.as_ref().map(|s| s.session)
        );
        try_reattach_all(c, &id, &label, &format!("reuse {name}"), expect_lost);
    }
    std::thread::sleep(Duration::from_millis(800));
}

fn s_leaderless(c: &mut Ctx) {
    let sleep = which("sleep").display().to_string();
    let shp = which("sh").display().to_string();
    let boot = boot_id().unwrap();
    let my_start = stat(std::process::id()).unwrap().start_ticks;
    for (i, (name, env, class, expect_lost)) in [
        ("unmarked", vec![], "decoy", true),
        (
            "other",
            vec![b"RR_CAMPAIGN_JOB=othercamp:ll".to_vec()],
            "decoy",
            true,
        ),
        (
            "ours_same_label",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:ll".to_vec()],
            "bydesign",
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mf = c.run.join(format!("ll{i}.mpid"));
        let script = format!("{sleep} 3000 & echo $! > {}; exit 0", mf.display());
        let r = c.req(json!({"name": format!("leaderless:{name}"), "class": class, "argv": [shp, "-c", script],
            "env": env.iter().map(|e: &Vec<u8>| hex(e)).collect::<Vec<_>>(), "setsid": true,
            "member_pidfile": mf.display().to_string(), "leader_class": "exits"}));
        let sid = r["pid"].as_u64().unwrap() as u32;
        std::thread::sleep(Duration::from_millis(500));
        // record: the session number with an OLDER start time (mine)
        let id = ProcessIdentity {
            pid: sid,
            start_ticks: my_start,
            boot_id: boot.clone(),
        };
        try_reattach_all(
            c,
            &id,
            "ll",
            &format!("leaderless {name} (older record)"),
            expect_lost,
        );
    }
    std::thread::sleep(Duration::from_millis(800));
}

fn s_nonleader(c: &mut Ctx) {
    for (name, env, class, expect_lost) in [
        ("unmarked", vec![], "decoy", true),
        (
            "other",
            vec![b"RR_CAMPAIGN_JOB=othercamp:nl".to_vec()],
            "decoy",
            true,
        ),
        (
            "ours_same_label",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:nl".to_vec()],
            "bydesign",
            false,
        ),
    ] {
        let mut spec = ds(
            &format!("nonleader:{name}"),
            class,
            &env.iter()
                .map(|e: &Vec<u8>| e.as_slice())
                .collect::<Vec<_>>(),
            false,
        );
        spec["setpgid"] = json!(true);
        let (pid, _) = c.decoy(spec);
        std::thread::sleep(Duration::from_millis(200));
        let id = ProcessIdentity::capture(pid).unwrap();
        let st = stat(pid).unwrap();
        println!(
            "  nonleader {name}: pid {pid} pgrp {} sid {}",
            st.pgrp, st.session
        );
        let m = CampaignMarker::new(CAMP, "nl").unwrap();
        let v = signal_verified_process(&id, &m, 0);
        c.res(
            &format!("nonleader {name}: signal_verified_process(CORRECT identity, sig 0 probe)"),
            if expect_lost {
                "refused"
            } else {
                "Delivered (by design: marker)"
            },
            format!("{v:?}"),
            if v.delivered() != expect_lost {
                V::Pass
            } else if expect_lost {
                V::Fail
            } else {
                V::Note
            },
        );
        try_reattach_all(c, &id, "nl", &format!("nonleader {name}"), expect_lost);
    }
    std::thread::sleep(Duration::from_millis(800));
}

fn s_stalegroup(c: &mut Ctx) {
    let sleep = which("sleep").display().to_string();
    let shp = which("sh").display().to_string();
    let boot = boot_id().unwrap();
    let mut l = launcher(CAMP);
    let gf = c.run.join("g.pid");
    let script = format!(
        "set -m; {sleep} 3000 & echo $! > {}; while :; do {sleep} 1; done",
        gf.display()
    );
    let mut h = l.launch(&sh(&c.run, "sg", &script)).unwrap();
    let g = read_u32_file(&gf, Duration::from_secs(10)).unwrap();
    let gs = stat(g).unwrap();
    println!(
        "  job sid {} ; member {g} pgrp {} sid {}",
        h.identity().pid,
        gs.pgrp,
        gs.session
    );
    wait_scan(&l);
    let mut l2 = launcher(CAMP);
    let mut h2 = match l2.reattach(h.identity(), "sg", ReattachChoice::Adopt) {
        Ok(Reattached::Live(x)) => x,
        r => {
            c.res("stalegroup adopt", "Live", reat(&r), V::Fail);
            return;
        }
    };
    let _ = h2.poll();
    // end the group-G member (verified, it carries the job's marker), wait until its pid is free
    let v = kill_orphan(CAMP, &boot, &mp(g, gs.start_ticks, "sg"), libc::SIGKILL);
    println!("  killed member {g}: {v:?}");
    crate::wait_for(Duration::from_secs(20), || stat(g).is_none());
    let mf = c.run.join("sg.mpid");
    let r = c.req(json!({"name": "stalegroup:decoy-group", "class": "decoy", "argv": [shp, "-c", format!("{sleep} 3000 & echo $! > {}; exec {sleep} 3000", mf.display())],
        "env": [], "setsid": true, "force_pid": g, "member_pidfile": mf.display().to_string()}));
    if !r["ok"].as_bool().unwrap_or(false) {
        c.res(
            "stalegroup",
            "decoy group on the stale group number",
            "could not force pid",
            V::Note,
        );
        return;
    }
    std::thread::sleep(Duration::from_millis(400));
    for _ in 0..5 {
        let _ = h2.poll();
        std::thread::sleep(Duration::from_millis(100));
    }
    let o = h2.signal_group(libc::SIGTERM);
    println!("  adopted.signal_group(SIGTERM) -> {o:?}");
    h2.terminate(Duration::ZERO, KillReason::Stop);
    h2.kill_now(KillReason::Reattach);
    let r2 = drive(&mut h2, Duration::from_secs(30));
    println!(
        "  adopted handle: {}",
        r2.as_ref().map(rs).unwrap_or("timeout".into())
    );
    h.terminate(Duration::ZERO, KillReason::Stop);
    let r1 = drive(&mut h, Duration::from_secs(30));
    c.chk(
        "stalegroup: original job ended",
        "Verified",
        r1.as_ref().map(rs).unwrap_or("timeout".into()),
        r1.is_some_and(|r| r.verification.is_verified()),
    );
    let alive_d = alive(
        r["pid"].as_u64().unwrap() as u32,
        r["start"].as_u64().unwrap(),
    );
    c.chk(
        "stalegroup: decoy group on the stale number survives",
        "alive",
        format!("{alive_d}"),
        alive_d,
    );
}

fn s_daemon(c: &mut Ctx) {
    let sleep = which("sleep").display().to_string();
    let shp = which("sh").display().to_string();
    let setsid = which("setsid").display().to_string();
    let envp = which("env").display().to_string();
    let r = c.run.display().to_string();
    let mut l = launcher(CAMP);
    let script = format!(
        "( {setsid} {shp} -c 'echo $$ > {r}/d1; exec {sleep} 3000' & ); \
         ( {shp} -c 'echo $$ > {r}/d2; exec {sleep} 3000' & ); \
         ( {envp} -i {setsid} {shp} -c 'echo $$ > {r}/d3; exec {sleep} 3000' & ); \
         ( {envp} -i {setsid} {shp} -c 'echo $$ > {r}/d4; exec {sleep} 4' & ); \
         ( {envp} -i {shp} -c 'echo $$ > {r}/d5; exec {sleep} 3000' & ); \
         {sleep} 1; exit 0"
    );
    let mut h = l.launch(&sh(&c.run, "dm", &script)).unwrap();
    let me = std::process::id();
    let mut d = vec![];
    for k in 1..=5 {
        let p = read_u32_file(&c.run.join(format!("d{k}")), Duration::from_secs(10)).unwrap();
        let s = stat(p).unwrap();
        println!(
            "  d{k}: pid {p} ppid {} (me {me}) sid {} start {}",
            s.ppid, s.session, s.start_ticks
        );
        d.push((p, s.start_ticks));
    }
    let rep = drive(&mut h, Duration::from_secs(40));
    println!(
        "  report: {}",
        rep.as_ref().map(rs).unwrap_or("timeout".into())
    );
    let rep = rep.unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let a: Vec<bool> = d.iter().map(|(p, s)| alive(*p, *s)).collect();
    let in_left = |p: u32| rep.leftovers_killed.iter().any(|x| x.pid == p);
    let in_esc = |p: u32| rep.escaped.iter().any(|x| x.pid == p);
    c.chk(
        "daemon d1 (double fork + setsid, marked): found, killed, reported",
        "dead & in leftovers_killed",
        format!("alive={} leftover={}", a[0], in_left(d[0].0)),
        !a[0] && in_left(d[0].0),
    );
    c.chk(
        "daemon d2 (double fork, same session, marked)",
        "dead & reported",
        format!("alive={} leftover={}", a[1], in_left(d[1].0)),
        !a[1] && in_left(d[1].0),
    );
    c.chk(
        "daemon d5 (env -i, same session)",
        "dead (session proof)",
        format!("alive={} leftover={}", a[4], in_left(d[4].0)),
        !a[4],
    );
    c.res(
        "daemon d3 (env -i + setsid: no marker, own session) NOT killed",
        "alive (cannot be proven ours; documented)",
        format!("alive={}", a[2]),
        if a[2] { V::Pass } else { V::Fail },
    );
    let in_un = |p: u32| rep.unattributed_children.iter().any(|x| x.pid == p);
    let reported = in_left(d[2].0) || in_esc(d[2].0) || in_un(d[2].0);
    c.res(
        "daemon d3 reported anywhere in the FinalReport",
        "reported (it is a live descendant re-parented to us)",
        format!(
            "leftovers={} escaped={} unattributed={} verification={:?}",
            in_left(d[2].0),
            in_esc(d[2].0),
            in_un(d[2].0),
            rep.verification
        ),
        if reported { V::Pass } else { V::Fail },
    );
    let ppid3 = stat(d[2].0).map(|s| s.ppid);
    c.res(
        "daemon d3 is now a child of the supervisor (subreaper)",
        "ppid == supervisor",
        format!("ppid={ppid3:?} me={me}"),
        V::Note,
    );
    std::thread::sleep(Duration::from_secs(5));
    let z = my_zombies();
    let d4z = z.iter().any(|(p, _)| *p == d[3].0);
    c.res(
        "unmarked re-parented daemon d4 exited (sleep 4): zombie left unreaped by the supervisor?",
        "reaped (no zombie)",
        format!("zombies of supervisor: {z:?}"),
        if d4z { V::Fail } else { V::Pass },
    );
    println!(
        "  reaper: {:?}",
        rustred_campaign::resources::reaper_stats()
    );
    let _ = l;
}

fn s_escapee(c: &mut Ctx) {
    let r = c.run.display().to_string();
    let mut l = launcher(CAMP);
    let script = format!(
        "setsid sh -c 'echo $$ > {r}/e1; exec sleep 3000' & sh -c 'echo $$ > {r}/m1; exec sleep 3000' & exec sleep 3000"
    );
    let mut h = l.launch(&sh(&c.run, "esc", &script)).unwrap();
    let e1 = read_u32_file(&c.run.join("e1"), Duration::from_secs(10)).unwrap();
    let m1 = read_u32_file(&c.run.join("m1"), Duration::from_secs(10)).unwrap();
    let (se, sm) = (stat(e1).unwrap().start_ticks, stat(m1).unwrap().start_ticks);
    wait_scan(&l);
    for _ in 0..3 {
        let _ = h.poll();
    }
    h.terminate(Duration::from_secs(1), KillReason::Stop);
    let rep = drive(&mut h, Duration::from_secs(40));
    println!(
        "  report: {}",
        rep.as_ref().map(rs).unwrap_or("timeout".into())
    );
    std::thread::sleep(Duration::from_millis(300));
    c.chk(
        "setsid escapee + member killed on terminate, Verified",
        "both dead, Verified",
        format!(
            "e1 alive={} m1 alive={} ver={:?}",
            alive(e1, se),
            alive(m1, sm),
            rep.as_ref().map(|r| r.verification.clone())
        ),
        !alive(e1, se) && !alive(m1, sm) && rep.is_some_and(|r| r.verification.is_verified()),
    );
}

/// Supervisor 1 of the crash/recovery choreography: launches jobs, writes records, then waits to be SIGKILLed.
fn s_recover1(c: &mut Ctx) {
    let me = std::process::id();
    let mut l = launcher(CAMP);
    let mut recs = vec![];
    let jobs: Vec<(&str, LaunchSpec)> = vec![
        (
            "J1",
            sh(&c.run, "J1", "sleep 3000 & sleep 3000 & exec sleep 3000"),
        ),
        ("J2", helper_spec(&c.run, "J2", &["nopdeath"])),
        (
            "J3",
            sh(&c.run, "J3", "setsid sleep 3000 & exec sleep 3000"),
        ),
        (
            "J4",
            helper_spec(
                &c.run,
                "J4",
                &["thread-leader", &c.run.join("j4.pid").display().to_string()],
            ),
        ),
        ("J5", sh(&c.run, "J5", "exec sleep 3000")),
    ];
    let mut handles = vec![];
    for (lab, spec) in jobs {
        let h = l.launch(&spec).unwrap();
        recs.push(json!({"label": lab, "identity": serde_json::to_value(h.identity()).unwrap()}));
        handles.push(h);
    }
    // decoys owned by the driver
    for s in [
        ds("rec:unmarked", "decoy", &[], true),
        ds(
            "rec:other",
            "decoy",
            &[b"RR_CAMPAIGN_JOB=othercamp:J1"],
            true,
        ),
        ds(
            "rec:prefix",
            "decoy",
            &[b"RR_CAMPAIGN_JOB=rbcampd:J1"],
            true,
        ),
        ds("rec:shorter", "decoy", &[b"RR_CAMPAIGN_JOB=rbcam:J1"], true),
        ds(
            "rec:value",
            "decoy",
            &[b"X=RR_CAMPAIGN_JOB=rbcamp:J1"],
            true,
        ),
        ds("rec:nogroup", "decoy", &[], false),
    ] {
        c.decoy(s);
    }
    let _ = read_u32_file(&c.run.join("j4.pid"), Duration::from_secs(10));
    wait_scan(&l);
    for h in handles.iter_mut() {
        let _ = h.poll();
    }
    write_atomic(&c.run.join("records.json"), &Value::Array(recs).to_string());
    write_atomic(
        &c.run.join("sup1.pid"),
        &format!("{me} {}", stat(me).unwrap().start_ticks),
    );
    write_atomic(&c.run.join("ready"), "1");
    println!("  supervisor-1 ready; waiting for SIGKILL");
    std::thread::sleep(Duration::from_secs(200));
}

fn s_recover2(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let recs: Value =
        serde_json::from_str(&std::fs::read_to_string(c.run.join("records.json")).unwrap())
            .unwrap();
    let mut l = launcher(CAMP);
    // B-M1: adopt the thread-leader job (main thread exited, a thread lives): must stay Running
    let j4 = recs
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["label"] == "J4")
        .unwrap();
    let id4: ProcessIdentity = serde_json::from_value(j4["identity"].clone()).unwrap();
    // gate8 m8-reattach: under load a marker read can time out: reattach is then UNDECIDED (an error), never Lost; try
    // again (bounded) as a caller must
    let mut r4 = l.reattach(&id4, "J4", ReattachChoice::Adopt);
    for _ in 0..50 {
        match &r4 {
            Err(e)
                if e.to_string()
                    .starts_with(rustred_campaign::launch::REATTACH_UNDECIDED) =>
            {
                std::thread::sleep(Duration::from_millis(200));
                r4 = l.reattach(&id4, "J4", ReattachChoice::Adopt);
            }
            _ => break,
        }
    }
    match r4 {
        Ok(Reattached::Live(mut h)) => {
            let mut states = vec![];
            for _ in 0..8 {
                states.push(match h.poll() {
                    Ok(ChildProgress::Running) => "R",
                    Ok(ChildProgress::Stopping) => "S",
                    Ok(ChildProgress::Finished(_)) => "F",
                    Err(_) => "E",
                });
                std::thread::sleep(Duration::from_millis(250));
            }
            c.chk(
                "adopted job whose main thread exited (live thread): stays Running",
                "all R",
                states.join(""),
                states.iter().all(|s| *s == "R"),
            );
        }
        r => c.res("adopt thread-leader J4", "Live", reat(&r), V::Fail),
    }
    let sc = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    println!(
        "  orphan scan: {} marked, {} unreadable",
        sc.processes.len(),
        sc.unreadable
    );
    for p in &sc.processes {
        let v = kill_orphan(CAMP, &boot, p, libc::SIGTERM);
        println!(
            "  kill_orphan(TERM) pid {} attempt {} comm {}: {v:?}",
            p.pid, p.attempt_id, p.comm
        );
    }
    std::thread::sleep(Duration::from_secs(1));
    let sc2 = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    for p in &sc2.processes {
        let v = kill_orphan(CAMP, &boot, p, libc::SIGKILL);
        println!(
            "  kill_orphan(KILL) pid {} attempt {}: {v:?}",
            p.pid, p.attempt_id
        );
    }
    std::thread::sleep(Duration::from_secs(1));
    for r in recs.as_array().unwrap() {
        let id: ProcessIdentity = serde_json::from_value(r["identity"].clone()).unwrap();
        let lab = r["label"].as_str().unwrap();
        let x = l.reattach(
            &id,
            lab,
            ReattachChoice::Kill {
                grace: Duration::from_millis(500),
            },
        );
        let s = reat(&x);
        if let Ok(Reattached::Live(mut h)) = x {
            let rr = drive(&mut h, Duration::from_secs(20));
            println!(
                "  reattach-kill {lab}: {}",
                rr.as_ref().map(rs).unwrap_or("timeout".into())
            );
        }
        c.res(
            &format!("after the orphan kill: reattach(Kill) of record {lab}"),
            "Lost(Gone)",
            s,
            V::Note,
        );
    }
    let sc3 = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    c.chk(
        "rescan after recovery",
        "no marked process left",
        format!(
            "{:?}",
            sc3.processes
                .iter()
                .map(|p| (p.pid, p.attempt_id.clone()))
                .collect::<Vec<_>>()
        ),
        sc3.processes.is_empty(),
    );
    std::thread::sleep(Duration::from_millis(500));
}

fn s_cpurange(c: &mut Ctx) {
    let lim = libc::rlimit {
        rlim_cur: 300 << 20,
        rlim_max: 300 << 20,
    };
    // SAFETY: setrlimit with a valid pointer.
    unsafe { libc::setrlimit(libc::RLIMIT_AS, &lim) };
    for s in [
        "0-60000000",
        "0-4000000000",
        "0-18446744073709551615",
        "99999999999999999999999",
        "1023",
        "1024",
        "0-1023",
        "5-3",
    ] {
        let t0 = Instant::now();
        let r = parse_cpu_list(s);
        c.chk(
            &format!("parse_cpu_list({s:?})"),
            if s == "1023" || s == "0-1023" {
                "Ok"
            } else {
                "Err, cheaply"
            },
            format!("{:?} in {:?}", r.as_ref().map(|x| x.len()), t0.elapsed()),
            (r.is_ok() == (s == "1023" || s == "0-1023")) && t0.elapsed() < Duration::from_secs(1),
        );
        let r = CpuSpec::parse(s);
        println!(
            "  CpuSpec::parse({s:?}) -> {:?}",
            r.as_ref().map(|x| match x {
                CpuSpec::List(l) => format!("List({})", l.len()),
                CpuSpec::Count(n) => format!("Count({n})"),
            })
        );
    }
    let root = c.run.join("site");
    std::fs::create_dir_all(root.join("locks")).unwrap();
    let cpu = rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .rev()
        .find(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .unwrap();
    // lane int5 (gate3 m3): the site's files exist beforehand (never created by the campaign), so that a refusal below
    // is the held file's, not a missing ledger's
    std::fs::write(root.join("CPU_LEDGER.md"), b"").unwrap();
    std::fs::write(root.join("locks").join(format!("cpu.{cpu}")), b"").unwrap();
    let base = SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rb-reaudit".into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(cpu.to_string()),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 1,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: true,
    };
    let mut ex = base.clone();
    ex.exclude = vec!["0-60000000".into()];
    c.chk(
        "site exclude \"0-60000000\" validate",
        "Err",
        format!("{:?}", ex.validate().err().map(|e| e.to_string())),
        ex.validate().is_err(),
    );
    let mut cand = base.clone();
    cand.candidates = Some("0-60000000".into());
    c.chk(
        "site candidates \"0-60000000\" validate",
        "Err",
        format!("{:?}", cand.validate().err().map(|e| e.to_string())),
        cand.validate().is_err(),
    );
    for (k, content) in [
        ("held", r#"{"held":"0-60000000"}"#),
        ("held", r#"{"held":["0-4000000000"]}"#),
        ("held", r#"{"held":60000000}"#),
    ] {
        let hf = root.join("held.json");
        std::fs::write(&hf, content).unwrap();
        let mut h = base.clone();
        h.held_files = vec![HeldFile {
            path: hf.clone(),
            key: k.into(),
            required: false,
        }];
        let t0 = Instant::now();
        let r = SitePolicy::new(h).and_then(|mut p| p.acquire(1, "x"));
        let s = match &r {
            Ok(a) => format!("{a:?}"),
            Err(e) => format!("Err({e})"),
        };
        let refused = !matches!(r, Ok(rustred_campaign::cpu_policy::Acquire::Granted(_)));
        c.res(
            &format!("held file {content} then acquire"),
            "refused / NotNow, no abort",
            format!("{s} in {:?}", t0.elapsed()),
            if refused || content.contains("60000000}") {
                V::Pass
            } else {
                V::Fail
            },
        );
    }
    c.res(
        "process survived every case under a 300 MB address-space limit",
        "no abort",
        "alive",
        V::Pass,
    );
}

fn s_cow(c: &mut Ctx) {
    let mut l = launcher(CAMP);
    let mut spec = helper_spec(&c.run, "cow", &["cow"]);
    spec.memory_cap_bytes = Some(120_000_000);
    spec.cap_mechanism = CapRequest::Sampling;
    let mut h = l.launch(&spec).unwrap();
    let t0 = Instant::now();
    let mut killed = None;
    let mut last = String::new();
    while t0.elapsed() < Duration::from_secs(12) {
        match h.poll().unwrap() {
            ChildProgress::Finished(r) => {
                killed = Some(rs(&r));
                break;
            }
            _ => {
                if let Some(u) = h.usage() {
                    last = format!(
                        "rss={} pss={:?} mem={:?} members={}",
                        u.rss_bytes,
                        u.pss_bytes,
                        u.mem_bytes(),
                        u.members.len()
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    c.chk(
        "CoW 80 MB + fork (unique ~85 MB) under a 120 MB cap",
        "not killed (PSS)",
        format!("killed={killed:?} last usage {last}"),
        killed.is_none(),
    );
    h.kill_now(KillReason::Other("end".into()));
    let _ = drive(&mut h, Duration::from_secs(30));
    let mut spec = helper_spec(&c.run, "alloc", &["alloc", "250"]);
    spec.memory_cap_bytes = Some(120_000_000);
    spec.cap_mechanism = CapRequest::Sampling;
    let mut h = l.launch(&spec).unwrap();
    let r = drive(&mut h, Duration::from_secs(30));
    c.chk(
        "control: 250 MB allocation under a 120 MB cap",
        "killed memory_cap_exceeded",
        r.as_ref().map(rs).unwrap_or("timeout".into()),
        r.is_some_and(|r| matches!(r.kill_reason, Some(KillReason::MemoryCap { .. }))),
    );
}

fn s_exitterm(c: &mut Ctx) {
    let mut l = launcher(CAMP);
    let mut h = l.launch(&sh(&c.run, "et", "exit 0")).unwrap();
    crate::wait_for(Duration::from_secs(10), || {
        matches!(h.poll_exit(), Ok(Some(_)))
    });
    h.terminate(Duration::from_secs(1), KillReason::Stop);
    h.kill_now(KillReason::ForceStop);
    let r = drive(&mut h, Duration::from_secs(30)).unwrap();
    c.chk(
        "terminate()/kill_now() after a normal exit",
        "kill_reason None, no TERM/KILL, outcome None",
        rs(&r),
        r.kill_reason.is_none()
            && !r.sigterm_sent
            && !r.sigkill_sent
            && r.supervisor_outcome().is_none(),
    );
}

fn s_leak(c: &mut Ctx) {
    const SECRET: &str = "DUMMY-SECRET-4f1a9";
    let mut l = launcher(CAMP);
    for (what, f) in [
        ("passthrough NAME=value", 0),
        ("passthrough with spaces", 1),
        ("env key containing '='", 2),
    ] {
        let mut s = sh(&c.run, "lk", "exit 0");
        match f {
            0 => s
                .env_passthrough
                .push(format!("SYMBOLICA_LICENSE={SECRET}")),
            1 => s.env_passthrough.push(format!("X {SECRET}")),
            _ => s.env.push((format!("A={SECRET}").into(), "v".into())),
        }
        let e = s
            .validate()
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default();
        let dbg = format!("{s:?}");
        let le = l
            .launch(&s)
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "LAUNCHED".into());
        let leaked = e.contains(SECRET) || dbg.contains(SECRET) || le.contains(SECRET);
        c.chk(
            &format!("{what}: refused and never echoed"),
            "Err without the value (validate, Debug, launch)",
            format!(
                "validate={e:?} launch={le:?} debug_leak={}",
                dbg.contains(SECRET)
            ),
            !leaked && !e.is_empty() && le != "LAUNCHED",
        );
    }
}

// ------------------------------------------------------------------------------------------------ section 2

fn s_marker(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let rbx = exe().display().to_string();
    let hz = |m: &str| -> Vec<String> {
        let mut v = vec![rbx.clone()];
        v.extend(rb_args(&["helper".to_string(), m.to_string()]));
        v
    };
    let mut long_att = b"RR_CAMPAIGN_JOB=rbcamp:".to_vec();
    long_att.extend(std::iter::repeat_n(b'a', 300));
    type Case<'a> = (
        &'a str,
        &'a str,
        Vec<Vec<u8>>,
        Option<Vec<String>>,
        bool,
        bool,
    );
    let cases: Vec<Case> = vec![
        ("m_none_empty_environ", "decoy", vec![], None, true, false),
        (
            "m_none",
            "decoy",
            vec![b"PATH=/x".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_other",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=othercamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_value",
            "decoy",
            vec![b"FOO=RR_CAMPAIGN_JOB=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_suffix_name",
            "decoy",
            vec![b"XRR_CAMPAIGN_JOB=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_name_prefix",
            "decoy",
            vec![b"RR_CAMPAIGN_JOBX=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_lower_name",
            "decoy",
            vec![b"rr_campaign_job=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_space_name1",
            "decoy",
            vec![b" RR_CAMPAIGN_JOB=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_space_name2",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB =rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_id_longer(rbcampd)",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcampd:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_id_shorter(rbcam)",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcam:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_id_case",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=RBCAMP:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_empty_value",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_empty_id",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_no_colon",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_empty_attempt",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_newline_in_id1",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp\n:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_newline_in_id2",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=\nrbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_eq_in_id1",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp=:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_eq_in_id2",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB==rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_space_in_id1",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB= rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_space_in_id2",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp :a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_dup_other_first",
            "decoy",
            vec![
                b"RR_CAMPAIGN_JOB=othercamp:a".to_vec(),
                b"RR_CAMPAIGN_JOB=rbcamp:a".to_vec(),
            ],
            None,
            true,
            false,
        ),
        (
            "m_badutf8_in_id",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=rbcamp\xff:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_unicode_lookalike_id",
            "decoy",
            vec!["RR_CAMPAIGN_JOB=rbc\u{0430}mp:a".as_bytes().to_vec()],
            None,
            true,
            false,
        ),
        (
            "m_attempt_too_long(300)",
            "decoy",
            vec![long_att],
            None,
            true,
            false,
        ),
        (
            "m_other_zmain",
            "decoy",
            vec![b"RR_CAMPAIGN_JOB=othercamp:z".to_vec()],
            Some(hz("zmain")),
            true,
            false,
        ),
        (
            "m_unmarked_nonleader",
            "decoy",
            vec![b"PATH=/x".to_vec()],
            None,
            false,
            true,
        ),
        (
            "o_plain",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:a".to_vec()],
            None,
            true,
            false,
        ),
        (
            "o_dup_ours_first",
            "ours",
            vec![
                b"RR_CAMPAIGN_JOB=rbcamp:a".to_vec(),
                b"RR_CAMPAIGN_JOB=othercamp:a".to_vec(),
            ],
            None,
            true,
            false,
        ),
        (
            "o_badutf8_attempt",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:a\xff".to_vec()],
            None,
            true,
            false,
        ),
        (
            "o_colons_in_attempt",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:a:b:c".to_vec()],
            None,
            true,
            false,
        ),
        (
            "o_zmain(thread alive)",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:z".to_vec()],
            Some(hz("zmain")),
            true,
            false,
        ),
        (
            "o_stopped",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:s".to_vec()],
            Some(hz("stopself")),
            true,
            false,
        ),
        (
            "o_nonleader",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:n".to_vec()],
            None,
            false,
            true,
        ),
        (
            "o_in_pid1_group",
            "ours",
            vec![b"RR_CAMPAIGN_JOB=rbcamp:g".to_vec()],
            None,
            false,
            false,
        ),
    ];
    let mut spawned: Vec<(String, String, u32, u64)> = vec![];
    for (name, class, env, argv, setsid, setpgid) in &cases {
        let mut spec = json!({"name": format!("marker:{name}"), "class": class,
            "env": env.iter().map(|e| hex(e)).collect::<Vec<_>>(), "setsid": setsid, "setpgid": setpgid});
        if let Some(a) = argv {
            spec["argv"] = json!(a);
        }
        let (p, s) = c.decoy(spec);
        spawned.push((name.to_string(), class.to_string(), p, s));
    }
    std::thread::sleep(Duration::from_millis(500));
    let l = launcher(CAMP);
    wait_scan(&l);
    let sc = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    let found: BTreeSet<u32> = sc.processes.iter().map(|p| p.pid).collect();
    let expect: BTreeSet<u32> = spawned
        .iter()
        .filter(|x| x.1 == "ours")
        .map(|x| x.2)
        .collect();
    let name_of = |p: &u32| {
        spawned
            .iter()
            .find(|x| x.2 == *p)
            .map(|x| x.0.clone())
            .unwrap_or(format!("?{p}"))
    };
    c.chk(
        "scan_campaign(rbcamp) lists EXACTLY the marked non-children",
        "the 8 o_* processes",
        format!(
            "found {:?}; unreadable={}",
            found.iter().map(name_of).collect::<Vec<_>>(),
            sc.unreadable
        ),
        found == expect,
    );
    let snap = l.sampler().latest().unwrap();
    let sm: BTreeSet<u32> = snap.marked.iter().map(|p| p.pid).collect();
    c.chk(
        "sampler Snapshot::marked == scan",
        "same set",
        format!("{:?}", sm.iter().map(name_of).collect::<Vec<_>>()),
        sm == expect,
    );
    for (camp, want) in [
        ("rbcam", "m_id_shorter(rbcam)"),
        ("rbcampd", "m_id_longer(rbcampd)"),
    ] {
        let s = scan_campaign(camp, 0, &BTreeSet::new()).unwrap();
        let names: Vec<String> = s.processes.iter().map(|p| name_of(&p.pid)).collect();
        c.chk(
            &format!("scan_campaign({camp}) (prefix relation with rbcamp)"),
            want,
            format!("{names:?}"),
            names == vec![want.to_string()],
        );
    }
    for bad in [
        "",
        "rb:camp",
        "rb=camp",
        "rb\ncamp",
        "rb camp",
        &"x".repeat(65),
    ] {
        let r = scan_campaign(bad, 0, &BTreeSet::new());
        c.chk(
            &format!("scan_campaign({bad:?})"),
            "Err (invalid id)",
            format!(
                "{:?}",
                r.as_ref()
                    .map(|s| s.processes.len())
                    .map_err(|e| e.to_string())
            ),
            r.is_err(),
        );
        let cfg = RealConfig {
            campaign_id: bad.to_string(),
            cap_preference: CapPreference::NoSystemd,
            ..RealConfig::default()
        };
        let l2 = RealLauncher::with_config(sampler(500), cfg);
        let got = l2.config().campaign_id.clone();
        c.chk(
            &format!("RealConfig.campaign_id {bad:?}"),
            "replaced by an ad-hoc id",
            got.clone(),
            got.starts_with("adhoc-"),
        );
        c.chk(
            &format!("CampaignMarker::new({bad:?}, a)"),
            "Err",
            format!("{:?}", CampaignMarker::new(bad, "a").is_err()),
            CampaignMarker::new(bad, "a").is_err(),
        );
    }
    // probes with signal 0 (nothing delivered for real): which processes WOULD the verified paths signal?
    let mut wrong = vec![];
    for (name, class, p, s) in &spawned {
        let v1 = kill_orphan(CAMP, &boot, &mp(*p, *s, "a"), 0);
        let v2 = match ProcessIdentity::capture(*p) {
            Ok(id) => signal_verified_process(&id, &CampaignMarker::new(CAMP, "a").unwrap(), 0),
            Err(e) => Verdict::Unverified(e.to_string()),
        };
        println!(
            "  probe {name:28} class={class:6} kill_orphan={v1:?} signal_verified_process(rbcamp:a)={v2:?}"
        );
        if class == "decoy" && (v1.delivered() || v2.delivered()) {
            wrong.push(name.clone());
        }
        if class == "ours" && !v1.delivered() {
            wrong.push(format!("{name}(ours not accepted)"));
        }
    }
    c.chk(
        "signal-0 probes: kill_orphan / signal_verified_process accept only this campaign's marker",
        "decoys refused, o_* accepted by kill_orphan",
        format!("wrong={wrong:?}"),
        wrong.is_empty(),
    );
    // the REAL kills: first through the scan, then adversarially at EVERY process directly
    for p in &sc.processes {
        let _ = kill_orphan(CAMP, &boot, p, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut delivered_to_decoy = vec![];
    for (name, class, p, s) in &spawned {
        let v = kill_orphan(CAMP, &boot, &mp(*p, *s, "a"), libc::SIGKILL);
        if class == "decoy" && v.delivered() {
            delivered_to_decoy.push(name.clone());
        }
    }
    std::thread::sleep(Duration::from_millis(500));
    c.chk(
        "adversarial kill_orphan(SIGKILL) at every decoy",
        "none delivered",
        format!("{delivered_to_decoy:?}"),
        delivered_to_decoy.is_empty(),
    );
}

/// Two supervisors with the SAME campaign id (a copied campaign directory, or a fixed id from the spec). Since the second
/// fix package (RB-M2) the second one is REFUSED at construction (host-wide id lock) and signals nothing.
fn s_two_a(c: &mut Ctx) {
    let mut l = launcher(CAMP);
    let mut h1 = l
        .launch(&sh(&c.run, "J1", "sleep 3000 & exec sleep 3000"))
        .unwrap();
    let mut h2 = l.launch(&sh(&c.run, "J2", "exec sleep 3000")).unwrap();
    println!(
        "  A: J1 leader {} J2 leader {}",
        h1.identity().pid,
        h2.identity().pid
    );
    wait_scan(&l);
    write_atomic(&c.run.join("A.ready"), "1");
    crate::wait_for(Duration::from_secs(60), || c.run.join("B.done").exists());
    let t0 = Instant::now();
    let (mut r1, mut r2) = (None, None);
    while t0.elapsed() < Duration::from_secs(5) && (r1.is_none() || r2.is_none()) {
        if r1.is_none()
            && let Ok(ChildProgress::Finished(r)) = h1.poll()
        {
            r1 = Some(r);
        }
        if r2.is_none()
            && let Ok(ChildProgress::Finished(r)) = h2.poll()
        {
            r2 = Some(r);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    c.res(
        "supervisor A's live job J1 after B's attempt",
        "keeps running (it is not B's)",
        r1.as_ref()
            .map(|r| format!("KILLED BY THE OTHER SUPERVISOR: {}", rs(r)))
            .unwrap_or("running".into()),
        if r1.is_some() { V::Fail } else { V::Pass },
    );
    c.res(
        "supervisor A's live job J2 after B's attempt",
        "keeps running",
        r2.as_ref()
            .map(|r| format!("KILLED BY THE OTHER SUPERVISOR: {}", rs(r)))
            .unwrap_or("running".into()),
        if r2.is_some() { V::Fail } else { V::Pass },
    );
    h1.kill_now(KillReason::Other("end".into()));
    h2.kill_now(KillReason::Other("end".into()));
    let _ = drive(&mut h1, Duration::from_secs(30));
    let _ = drive(&mut h2, Duration::from_secs(30));
}

fn s_two_b(c: &mut Ctx) {
    crate::wait_for(Duration::from_secs(60), || c.run.join("A.ready").exists());
    let cfg = RealConfig {
        campaign_id: CAMP.to_string(),
        cap_preference: CapPreference::NoSystemd,
        ..RealConfig::default()
    };
    let r = RealLauncher::try_with_config(sampler(200), cfg);
    let refused = r.is_err();
    c.res(
        "second supervisor with the same campaign id",
        "refused at construction (host-wide id lock), nothing signalled",
        match &r {
            Ok(_) => "STARTED".to_string(),
            Err(e) => format!("refused: {e}"),
        },
        if refused { V::Pass } else { V::Fail },
    );
    drop(r);
    write_atomic(&c.run.join("B.done"), "1");
}

fn site_cfg(root: &Path, cpu: usize) -> SiteConfig {
    std::fs::create_dir_all(root.join("locks")).unwrap();
    // lane int5 (gate3 m3): the campaign never creates the site's ledger or lock files: the test site has them
    std::fs::write(root.join("CPU_LEDGER.md"), b"").unwrap();
    std::fs::write(root.join("locks").join(format!("cpu.{cpu}")), b"").unwrap();
    SiteConfig {
        schema: SITE_SCHEMA.into(),
        tag: "rb-reaudit".into(),
        lock_dir: root.join("locks"),
        lock_name: "cpu.{n}".into(),
        ledger_file: root.join("CPU_LEDGER.md"),
        ledger_scan_lines: 1500,
        candidates: Some(cpu.to_string()),
        exclude: vec![],
        held_files: vec![],
        pause_files: vec![],
        idle_min: 0.0,
        max_cores: 1,
        respect_foreign_pinning: false,
        ignore_own_tag_claims: true,
    }
}

fn s_lease(c: &mut Ctx) {
    let sleep = which("sleep").display().to_string();
    let shp = which("sh").display().to_string();
    let setsid = which("setsid").display().to_string();
    let envp = which("env").display().to_string();
    let r = c.run.display().to_string();
    let cpu = rustred_campaign::resources::allowed_cpus()
        .unwrap()
        .into_iter()
        .rev()
        .find(|c| !(168..=175).contains(c) && !(288..=319).contains(c))
        .unwrap();
    let cfg = site_cfg(&c.run.join("site"), cpu);
    let mut p = SitePolicy::new(cfg.clone()).unwrap();
    let lease = p.acquire(1, "lease job").unwrap().granted().unwrap();
    println!("  leased cpu {cpu}, lock fds {:?}", lease.lock_fds());
    let mut l = launcher(CAMP);
    let script = format!(
        "( {envp} -i {setsid} {shp} -c 'echo $$ > {r}/u; exec {sleep} 3000' & ); ( {setsid} {shp} -c 'echo $$ > {r}/m; exec {sleep} 3000' & ); {sleep} 1; exit 0"
    );
    let mut h = l
        .launch(&sh(&c.run, "ls", &script).with_lease(&lease))
        .unwrap();
    let u = read_u32_file(&c.run.join("u"), Duration::from_secs(10)).unwrap();
    let rep = drive(&mut h, Duration::from_secs(40));
    println!(
        "  job report: {}",
        rep.as_ref().map(rs).unwrap_or("timeout".into())
    );
    let fds: Vec<String> = std::fs::read_dir(format!("/proc/{u}/fd"))
        .map(|d| {
            d.flatten()
                .filter_map(|e| std::fs::read_link(e.path()).ok())
                .map(|p| p.display().to_string())
                .filter(|p| p.contains("locks"))
                .collect()
        })
        .unwrap_or_default();
    c.res(
        "unmarked escapee (env -i + setsid) survives holding the inherited CPU lock fd",
        "documented: invisible to the layer",
        format!("alive={} lock fds held={fds:?}", stat(u).is_some()),
        V::Note,
    );
    let (rr, held) = lease.release_checked();
    println!("  lease.release_checked() -> {rr:?}, still held: {held:?}");
    c.res(
        "CPU still held after the release is reported with its holder",
        "held by the unmarked survivor's pid",
        format!("{held:?}"),
        if held.iter().any(|h| h.holders.contains(&u)) {
            V::Pass
        } else {
            V::Fail
        },
    );
    let t0 = Instant::now();
    let a = p.acquire(1, "second");
    let el = t0.elapsed();
    let s = match &a {
        Ok(x) => format!("{x:?}"),
        Err(e) => format!("Err({e})"),
    };
    let clean = matches!(a, Ok(rustred_campaign::cpu_policy::Acquire::NotNow(_)))
        && el < Duration::from_secs(12);
    c.chk(
        "next acquire of that CPU while the escapee holds its flock",
        "NotNow quickly (fails cleanly, no grant)",
        format!("{s} after {el:?}"),
        clean,
    );
    let mut q = SitePolicy::new(cfg).unwrap();
    let rec = q.reconcile_own_claims();
    c.res(
        "reconcile_own_claims at restart",
        "the CPU listed as held (reported)",
        format!("{rec:?}"),
        V::Note,
    );
    let reported = rep.as_ref().is_some_and(|r| {
        r.escaped.iter().any(|e| e.pid == u)
            || r.leftovers_killed.iter().any(|e| e.pid == u)
            || r.unattributed_children.iter().any(|e| e.pid == u)
    });
    c.res(
        "is the lock-holding survivor reported in the job's FinalReport?",
        "reported",
        format!("{reported}"),
        if reported { V::Pass } else { V::Fail },
    );
    c.res(
        "reconcile_own_claims lists the holder",
        "held_by contains the survivor",
        format!("{:?}", rec.as_ref().map(|r| r.held_by.clone())),
        if rec
            .as_ref()
            .is_ok_and(|r| r.held_by.iter().any(|(_, h)| h.contains(&u)))
        {
            V::Pass
        } else {
            V::Fail
        },
    );
}

/// (e1) RLIMIT_NOFILE exhausted at the moment of the FIRST pidfd probe; later the limit is restored.
fn s_fd1(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let (cur, max) = nofile();
    let held = exhaust_fds();
    let probe = pidfd_probe();
    drop(held);
    set_nofile(cur, max);
    let later = pidfd_supported();
    c.res(
        "pidfd probe first called while fds are exhausted (EMFILE)",
        "never cached as unsupported; supported once fds are back",
        format!("probe under EMFILE={probe:?}; afterwards supported={later}"),
        if probe != PidfdProbe::Unsupported && later {
            V::Pass
        } else {
            V::Fail
        },
    );
    let mut l = launcher(CAMP);
    println!("  launcher setup: {:?}", l.setup());
    let (po, ps) = c.decoy(ds(
        "fd1:ours-orphan",
        "ours",
        &[b"RR_CAMPAIGN_JOB=rbcamp:fd"],
        true,
    ));
    let (pd, sd) = c.decoy(ds("fd1:unmarked", "decoy", &[b"PATH=/x"], true));
    let v1 = kill_orphan(CAMP, &boot, &mp(pd, sd, "fd"), libc::SIGKILL);
    c.chk(
        "unmarked decoy via kill_orphan (fallback path)",
        "refused",
        format!("{v1:?}"),
        !v1.delivered(),
    );
    let v2 = kill_orphan(CAMP, &boot, &mp(po, ps, "fd"), libc::SIGKILL);
    c.res(
        "marked orphan via kill_orphan (which syscall? see strace)",
        "Delivered",
        format!("{v2:?}"),
        V::Note,
    );
    let mut h = l
        .launch(&sh(&c.run, "fd1job", "sleep 3000 & exec sleep 3000"))
        .unwrap();
    wait_scan(&l);
    let _ = h.poll();
    h.terminate(Duration::from_millis(300), KillReason::Stop);
    let r = drive(&mut h, Duration::from_secs(30));
    c.chk(
        "own job terminated without pidfds",
        "Verified",
        r.as_ref().map(rs).unwrap_or("timeout".into()),
        r.is_some_and(|r| r.verification.is_verified()),
    );
    std::thread::sleep(Duration::from_millis(500));
}

/// (e2) fds exhausted while signalling: nothing unchecked may be sent.
fn s_fd2(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    assert!(pidfd_supported());
    let mut l = launcher(CAMP);
    let (po, ps) = c.decoy(ds(
        "fd2:ours-orphan",
        "bydesign",
        &[b"RR_CAMPAIGN_JOB=rbcamp:fdo"],
        true,
    ));
    let (pd, sd) = c.decoy(ds("fd2:unmarked", "decoy", &[b"PATH=/x"], true));
    let idd = ProcessIdentity::capture(pd).unwrap();
    let mut h = l
        .launch(&sh(&c.run, "fd2job", "sleep 3000 & exec sleep 3000"))
        .unwrap();
    wait_scan(&l);
    let _ = h.poll();
    let (cur, max) = nofile();
    let held = exhaust_fds();
    let v1 = kill_orphan(CAMP, &boot, &mp(po, ps, "fdo"), libc::SIGKILL);
    let v2 = signal_verified_process(
        &idd,
        &CampaignMarker::new(CAMP, "fdo").unwrap(),
        libc::SIGKILL,
    );
    let v3 = kill_orphan(CAMP, &boot, &mp(pd, sd, "fdo"), libc::SIGKILL);
    h.terminate(Duration::ZERO, KillReason::Stop);
    let t0 = Instant::now();
    let mut st = vec![];
    while t0.elapsed() < Duration::from_secs(3) {
        st.push(match h.poll() {
            Ok(ChildProgress::Finished(r)) => format!("F({:?})", r.verification),
            Ok(ChildProgress::Stopping) => "S".into(),
            Ok(ChildProgress::Running) => "R".into(),
            Err(e) => format!("E({e})"),
        });
        std::thread::sleep(Duration::from_millis(300));
    }
    drop(held);
    set_nofile(cur, max);
    c.chk(
        "kill_orphan of a MARKED process while fds are exhausted",
        "Unverified (no unchecked fallback)",
        format!("{v1:?}"),
        matches!(v1, Verdict::Unverified(_)),
    );
    c.chk(
        "signal_verified_process / kill_orphan of an UNMARKED decoy while fds are exhausted",
        "not delivered",
        format!("{v2:?} / {v3:?}"),
        !v2.delivered() && !v3.delivered(),
    );
    c.res(
        "own job terminate() while fds are exhausted (poll states)",
        "pinned group signal still works; no false Verified",
        st.join(" "),
        V::Note,
    );
    let r = drive(&mut h, Duration::from_secs(40));
    c.chk(
        "own job after the limit is restored",
        "Verified",
        r.as_ref().map(rs).unwrap_or("timeout".into()),
        r.is_some_and(|r| r.verification.is_verified()),
    );
    c.chk(
        "decoy alive after the fd episode",
        "alive",
        format!("{}", alive(pd, sd)),
        alive(pd, sd),
    );
}

/// (e3) the leader exits while fds are exhausted: can the job be reported Verified while members still live?
fn s_fd3(c: &mut Ctx) {
    let r = c.run.display().to_string();
    let mut l = launcher(CAMP);
    let script = format!(
        "sleep 3000 & echo $! > {r}/m; setsid sh -c 'echo $$ > {r}/e; exec sleep 3000' & sleep 3; exit 0"
    );
    let mut h = l.launch(&sh(&c.run, "fd3", &script)).unwrap();
    let m = read_u32_file(&c.run.join("m"), Duration::from_secs(10)).unwrap();
    let e = read_u32_file(&c.run.join("e"), Duration::from_secs(10)).unwrap();
    let (sm, se) = (stat(m).unwrap().start_ticks, stat(e).unwrap().start_ticks);
    c.register("fd3:job-member", "ours", m, sm);
    c.register("fd3:job-marked-escapee", "ours", e, se);
    wait_scan(&l);
    let _ = h.poll();
    let (cur, max) = nofile();
    let held = exhaust_fds();
    let t0 = Instant::now();
    let mut fin = None;
    while t0.elapsed() < Duration::from_secs(12) {
        if let Ok(ChildProgress::Finished(rep)) = h.poll() {
            fin = Some(rep);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(held);
    set_nofile(cur, max);
    let (am, ae) = (alive(m, sm), alive(e, se));
    let ver = fin
        .as_ref()
        .map(|r| r.verification.is_verified())
        .unwrap_or(false);
    c.res(
        "leader exits while the supervisor cannot open files (EMFILE): report vs reality",
        "never Verified while a member / marked escapee lives",
        format!(
            "finished={} member alive={am} escapee alive={ae} report={}",
            fin.is_some(),
            fin.as_ref().map(rs).unwrap_or("-".into())
        ),
        if ver && (am || ae) { V::Fail } else { V::Pass },
    );
    if fin.is_none() {
        let r2 = drive(&mut h, Duration::from_secs(40));
        println!(
            "  after restore: {}",
            r2.as_ref().map(rs).unwrap_or("timeout".into())
        );
    }
    std::thread::sleep(Duration::from_secs(1));
    let sc = scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
    println!(
        "  later orphan scan sees: {:?}",
        sc.processes
            .iter()
            .map(|p| (p.pid, p.attempt_id.clone()))
            .collect::<Vec<_>>()
    );
    let ses: Vec<u32> = [m, e].into_iter().filter(|p| stat(*p).is_some()).collect();
    println!("  still alive after the scenario: {ses:?} (left for the driver's final check)");
}

// ------------------------------------------------------------------------------------------------ churn (2f) and relay

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn s_churn(c: &mut Ctx, cycles: u64, seed: u64) {
    let boot = boot_id().unwrap();
    let sleep = which("sleep").display().to_string();
    let mut rng = Rng(seed | 1);
    let mut l = launcher_ms(CAMP, 100);
    let scripts = [
        format!("exec {sleep} 30"),
        format!("{sleep} 30 & exec {sleep} 30"),
        format!("setsid {sleep} 30 & exec {sleep} 30"),
        format!("( setsid {sleep} 30 & ); exec {sleep} 30"),
        format!("{sleep} 30 & {sleep} 0.05; exit 0"),
        format!("( {sleep} 30 & ); exit 0"),
        format!("( setsid {sleep} 30 & ); exit 0"),
    ];
    let mut kind_of: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut verify_ctx: std::collections::HashMap<String, (u64, usize, u64, usize)> =
        std::collections::HashMap::new();
    struct J {
        h: rustred_campaign::launch::RealChild,
        stop_at: Instant,
        kill: bool,
        grace: Duration,
        stopped: bool,
    }
    let mut active: Vec<J> = vec![];
    let (
        mut launched,
        mut finished,
        mut verified,
        mut unverified,
        mut leftovers,
        mut errors,
        mut orphans,
    ) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    // procfix2: attempts reported Verified (only these may never have a marked survivor) and the quarantine of the others.
    let mut verified_labels: BTreeSet<String> = BTreeSet::new();
    let mut cleared = 0u64;
    let fdstorm = std::env::var("RB_FDSTORM").is_ok();
    let mut storms = 0u64;
    let mut last_storm = Instant::now();
    let mut pending_freed: Vec<(Instant, u32)> = vec![];
    let t0 = Instant::now();
    let freed = c.run.join("freed");
    while finished < cycles && t0.elapsed() < Duration::from_secs(1300) {
        while active.len() < 8 && launched < cycles {
            let k = rng.below(scripts.len() as u64) as usize;
            kind_of.insert(format!("c{launched}"), k);
            match l.launch(&sh(&c.run, &format!("c{launched}"), &scripts[k])) {
                Ok(h) => active.push(J {
                    h,
                    stop_at: Instant::now() + Duration::from_millis(rng.below(300)),
                    kill: rng.below(2) == 0,
                    grace: Duration::from_millis(rng.below(200)),
                    stopped: false,
                }),
                Err(e) => {
                    errors += 1;
                    println!("  launch error: {e}");
                }
            }
            launched += 1;
        }
        let now = Instant::now();
        let mut i = 0;
        while i < active.len() {
            let j = &mut active[i];
            if !j.stopped && now >= j.stop_at {
                j.stopped = true;
                if j.kill {
                    j.h.kill_now(KillReason::Other("churn".into()));
                } else {
                    j.h.terminate(j.grace, KillReason::Stop);
                }
            }
            match j.h.poll() {
                Ok(ChildProgress::Finished(r)) => {
                    finished += 1;
                    let verified_now = r.verification.is_verified();
                    if verified_now {
                        verified += 1;
                        verified_labels.insert(r.label.clone());
                    } else {
                        unverified += 1;
                        if unverified <= 20 {
                            println!("  UNVERIFIED {}: {}", r.label, rs(&r));
                        }
                    }
                    leftovers += r.leftovers_killed.len() as u64;
                    if let Some(sn) = l.sampler().latest() {
                        verify_ctx.insert(
                            r.label.clone(),
                            (
                                sn.seq,
                                sn.marked_unreadable,
                                sn.read_errors,
                                sn.processes_seen,
                            ),
                        );
                    }
                    pending_freed.push((Instant::now(), r.identity.pid));
                    for x in &r.leftovers_killed {
                        pending_freed.push((Instant::now(), x.pid));
                    }
                    let j = active.swap_remove(i);
                    if !verified_now {
                        let _ = l.retire(j.h, None);
                    }
                    continue;
                }
                Ok(_) => {}
                Err(e) => println!("  poll error {e}"),
            }
            i += 1;
        }
        for ev in l.tick() {
            if let RetireEvent::Cleared { .. } = ev {
                cleared += 1;
            }
        }
        // RB_FDSTORM: every ~2 s, exhaust the descriptor table for ~300 ms while jobs end (RB-C1 trigger).
        if fdstorm && last_storm.elapsed() > Duration::from_secs(2) {
            last_storm = Instant::now();
            storms += 1;
            let (cur, max) = nofile();
            let held = exhaust_fds();
            let t1 = Instant::now();
            while t1.elapsed() < Duration::from_millis(300) {
                for j in active.iter_mut() {
                    if let Ok(ChildProgress::Finished(r)) = j.h.poll()
                        && r.verification.is_verified()
                    {
                        // Kept in `active`: the next poll returns the same report and the loop above books it.
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            drop(held);
            set_nofile(cur, max);
        }
        let ready: Vec<u32> = pending_freed
            .iter()
            .filter(|(t, _)| t.elapsed() > Duration::from_millis(300))
            .map(|(_, p)| *p)
            .collect();
        if !ready.is_empty() {
            pending_freed.retain(|(t, _)| t.elapsed() <= Duration::from_millis(300));
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&freed)
                .unwrap();
            for p in ready {
                let _ = writeln!(f, "{p}");
            }
        }
        if finished > 0 && finished % 250 == 0 {
            let live: BTreeSet<String> = active.iter().map(|j| j.h.label().to_string()).collect();
            if let Ok(sc) = scan_campaign(CAMP, 0, &live) {
                // Only attempts REPORTED VERIFIED count (unverified ones are in the quarantine, hunted there).
                for p in sc
                    .processes
                    .iter()
                    .filter(|p| verified_labels.contains(&p.attempt_id))
                {
                    orphans += 1;
                    let v = kill_orphan(CAMP, &boot, p, libc::SIGKILL);
                    println!(
                        "  ORPHAN of finished (Verified) attempt {} (script {:?}) pid {} sid {} ppid {} start {} comm {}: {v:?}; snapshot at report (seq, marked_unreadable, read_errors, seen)={:?}",
                        p.attempt_id,
                        kind_of.get(&p.attempt_id),
                        p.pid,
                        p.session,
                        p.ppid,
                        p.start_ticks,
                        p.comm,
                        verify_ctx.get(&p.attempt_id)
                    );
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Drain the quarantine (bounded), then the final scan.
    for ev in l.shutdown(Duration::from_secs(120)) {
        if let RetireEvent::Cleared { .. } = ev {
            cleared += 1;
        }
    }
    let still_q = l.quarantined().len();
    let final_scan = scan_campaign(CAMP, 0, &BTreeSet::new())
        .map(|s| s.processes)
        .unwrap_or_default();
    let verified_left = final_scan
        .iter()
        .filter(|p| verified_labels.contains(&p.attempt_id))
        .count();
    let sc = final_scan.len();
    println!(
        "CHURNSUP cycles_launched={launched} finished={finished} verified={verified} unverified={unverified} cleared_by_quarantine={cleared} still_quarantined={still_q} leftovers_killed_total={leftovers} launch_errors={errors} verified_with_marked_survivor={} marked_left_at_end={sc} fd_storms={storms} elapsed={:?} seed={seed}",
        orphans + verified_left as u64,
        t0.elapsed()
    );
    c.chk(
        "churn: ZERO jobs Verified with a marked survivor (acceptance)",
        "0",
        format!(
            "{} (periodic scans {orphans}, final scan {verified_left})",
            orphans + verified_left as u64
        ),
        orphans == 0 && verified_left == 0,
    );
    c.chk(
        "churn: every unverified end cleared by the quarantine, nothing marked left",
        "still_quarantined=0, left=0",
        format!("unverified={unverified} cleared={cleared} still={still_q} left={sc}"),
        still_q == 0 && sc == 0,
    );
    std::thread::sleep(Duration::from_secs(2));
}

/// A job that becomes a "relay": each generation forks a child in a NEW session and exits.
fn s_relay(c: &mut Ctx) {
    let boot = boot_id().unwrap();
    let mut l = launcher_ms(CAMP, 100);
    let count = |f: &Path| {
        std::fs::read_to_string(f)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    for (i, pause_us) in [20_000u64, 5_000, 2_000, 500, 20_000, 5_000, 2_000, 500]
        .into_iter()
        .enumerate()
    {
        let label = format!("relay{i}");
        let cf = c.run.join(format!("relay{i}.count"));
        let h = match l.launch(&helper_spec(
            &c.run,
            &label,
            &[
                "relay",
                "18446744073709551615",
                &pause_us.to_string(),
                &cf.display().to_string(),
            ],
        )) {
            Ok(h) => h,
            Err(e) => {
                c.res(
                    &format!("relay job #{i}: launch"),
                    "launched",
                    format!(
                        "LAUNCH FAILED: {e}; zombies of the supervisor now: {}",
                        my_zombies().len()
                    ),
                    V::Fail,
                );
                continue;
            }
        };
        let mut h = h;
        let r = drive(&mut h, Duration::from_secs(40));
        let c1 = count(&cf);
        std::thread::sleep(Duration::from_millis(1000));
        let c2 = count(&cf);
        let running = c2 > c1;
        let ver = r
            .as_ref()
            .map(|r| r.verification.is_verified())
            .unwrap_or(false);
        c.res(
            &format!("relay job #{i} (hop every {pause_us} us, new session each hop; leader exits at once)"),
            "never Verified while the marked relay still runs",
            format!("report={} ; relay hop counter {c1} -> {c2} one second AFTER the report (still running={running})", r.as_ref().map(rs).unwrap_or("timeout".into())),
            if ver && running { V::Fail } else { V::Pass },
        );
        if r.as_ref().is_some_and(|r| !r.verification.is_verified()) {
            let tq = Instant::now();
            let _ = l.retire(h, None);
            let mut ok = false;
            while tq.elapsed() < Duration::from_secs(60) && !ok {
                ok = l
                    .tick()
                    .iter()
                    .any(|e| matches!(e, RetireEvent::Cleared { label: x, .. } if *x == label));
                std::thread::sleep(Duration::from_millis(50));
            }
            c.res(
                &format!("relay job #{i}: unverified end cleared by the quarantine"),
                "within 60 s",
                format!("cleared={ok} after {:?}", tq.elapsed()),
                if ok { V::Pass } else { V::Fail },
            );
        }
        // clean up: repeated verified orphan kills of this attempt until the counter stops
        let t0 = Instant::now();
        let mut kills = 0;
        let mut last = count(&cf);
        let mut still = 0;
        while t0.elapsed() < Duration::from_secs(60) {
            if let Ok(sc) = scan_campaign(CAMP, 0, &BTreeSet::new()) {
                for p in sc.processes.iter().filter(|p| p.attempt_id == label) {
                    if kill_orphan(CAMP, &boot, p, libc::SIGKILL).delivered() {
                        kills += 1;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(30));
            let now = count(&cf);
            if now == last {
                still += 1;
                if still > 30 {
                    break;
                }
            } else {
                still = 0;
                last = now;
            }
        }
        println!(
            "  relay{i}: cleanup needed {kills} verified kills over {:?}; counter stopped at {last}; zombies of the supervisor now: {}",
            t0.elapsed(),
            my_zombies().len()
        );
    }
}

/// VERIFY race: jobs end BY THEMSELVES leaving descendants; after each Verified report, is any marked process of that
/// attempt still alive? (Each verification is one non-atomic /proc pass.)
fn s_vrace(c: &mut Ctx, cycles: u64) {
    let boot = boot_id().unwrap();
    let sleep = which("sleep").display().to_string();
    let scripts = [
        format!("( setsid {sleep} 30 & ); exit 0"),
        format!("( {sleep} 30 & ); exit 0"),
        format!("{sleep} 30 & exit 0"),
        format!("setsid {sleep} 30 & exit 0"),
    ];
    let mut l = launcher_ms(CAMP, 100);
    let mut active: Vec<(rustred_campaign::launch::RealChild, usize)> = vec![];
    let mut checks: Vec<(Instant, String, usize)> = vec![];
    let (mut launched, mut fin, mut unver) = (0u64, 0u64, 0u64);
    let mut missed = [0u64; 4];
    let mut per = [0u64; 4];
    let t0 = Instant::now();
    while (fin < cycles || !checks.is_empty()) && t0.elapsed() < Duration::from_secs(1200) {
        while active.len() < 8 && launched < cycles {
            let k = (launched % 4) as usize;
            if let Ok(h) = l.launch(&sh(&c.run, &format!("v{launched}"), &scripts[k])) {
                active.push((h, k));
            }
            launched += 1;
        }
        let mut i = 0;
        while i < active.len() {
            if let Ok(ChildProgress::Finished(r)) = active[i].0.poll() {
                fin += 1;
                let k = active[i].1;
                per[k] += 1;
                let (h, _) = active.swap_remove(i);
                if r.verification.is_verified() {
                    checks.push((Instant::now(), r.label.clone(), k));
                } else {
                    unver += 1;
                    let _ = l.retire(h, None);
                }
                continue;
            }
            i += 1;
        }
        let due: Vec<(String, usize)> = checks
            .iter()
            .filter(|x| x.0.elapsed() > Duration::from_millis(300))
            .map(|x| (x.1.clone(), x.2))
            .collect();
        if !due.is_empty() {
            checks.retain(|x| x.0.elapsed() <= Duration::from_millis(300));
            if let Ok(sc) = scan_campaign(CAMP, 0, &BTreeSet::new()) {
                for (lab, k) in &due {
                    let left: Vec<&MarkedProcess> = sc
                        .processes
                        .iter()
                        .filter(|p| &p.attempt_id == lab)
                        .collect();
                    if !left.is_empty() {
                        missed[*k] += 1;
                        if missed.iter().sum::<u64>() <= 8 {
                            println!(
                                "  MISSED: attempt {lab} (script {k}) reported Verified, still alive: {:?}",
                                left.iter()
                                    .map(|p| (p.pid, p.session, p.ppid, p.comm.clone()))
                                    .collect::<Vec<_>>()
                            );
                        }
                        for p in left {
                            let _ = kill_orphan(CAMP, &boot, p, libc::SIGKILL);
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = l.shutdown(Duration::from_secs(60));
    println!(
        "VRACE pid_max={} jobs={fin} unverified={unver} per_script={per:?} verified_but_marked_orphan_alive={missed:?} elapsed={:?}",
        pid_max(),
        t0.elapsed()
    );
    for (k, s) in scripts.iter().enumerate() {
        println!("VRACE   script {k}: {s}");
    }
    let total: u64 = missed.iter().sum();
    c.chk(
        "job reported Verified while a marked process of the attempt was still alive",
        "0",
        format!("{total} of {fin} (per script {missed:?})"),
        total == 0,
    );
}

/// Short-lived double-forked descendants that leave the session: are their zombies reaped by the subreaper?
fn s_zombies(c: &mut Ctx) {
    let envp = which("env").display().to_string();
    let shp = which("sh").display().to_string();
    let setsid = which("setsid").display().to_string();
    let mut l = launcher(CAMP);
    let before = my_zombies().len();
    let script = format!(
        "for i in $(seq 300); do ( setsid sh -c 'exit 0' & ); done; for i in $(seq 100); do ( {envp} -i {setsid} {shp} -c 'exit 0' & ); done; for i in $(seq 100); do ( sh -c 'exit 0' & ); done; sleep 1; exit 0"
    );
    let mut h = l.launch(&sh(&c.run, "zb", &script)).unwrap();
    let r = drive(&mut h, Duration::from_secs(60));
    println!(
        "  report: {}",
        r.as_ref().map(rs).unwrap_or("timeout".into())
    );
    std::thread::sleep(Duration::from_secs(4));
    let z = my_zombies();
    let _ = &l;
    c.res(
        "500 short-lived double forks (300 marked+setsid, 100 env -i+setsid, 100 same session): zombies left in the supervisor",
        "0 (every re-parented child reaped)",
        format!("zombies before={before} after={} ; reaper {:?}", z.len(), rustred_campaign::resources::reaper_stats()),
        if z.is_empty() { V::Pass } else { V::Fail },
    );
}

pub fn main(a: &[String]) {
    let scen = a[0].clone();
    let run = PathBuf::from(a.last().unwrap());
    let mut c = Ctx {
        scen: scen.clone(),
        run,
        n: 0,
        pass: 0,
        fail: 0,
        note: 0,
    };
    let me = std::process::id();
    println!("SUP {scen} pid {me} start {}", crate::utc());
    let seed: u64 = std::env::var("RB_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0x5eed_0000_0000_b001);
    let cycles: u64 = std::env::var("RB_CYCLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    match scen.as_str() {
        "pid1" => s_pid1(&mut c),
        "pid0" => s_pid0(&mut c),
        "bigpid" => s_bigpid(&mut c),
        "reuse" => s_reuse(&mut c),
        "leaderless" => s_leaderless(&mut c),
        "nonleader" => s_nonleader(&mut c),
        "stalegroup" => s_stalegroup(&mut c),
        "daemon" => s_daemon(&mut c),
        "escapee" => s_escapee(&mut c),
        "recover1" => s_recover1(&mut c),
        "recover2" => s_recover2(&mut c),
        "cpurange" => s_cpurange(&mut c),
        "cow" => s_cow(&mut c),
        "exitterm" => s_exitterm(&mut c),
        "leak" => s_leak(&mut c),
        "marker" => s_marker(&mut c),
        "twoA" => s_two_a(&mut c),
        "twoB" => s_two_b(&mut c),
        "lease" => s_lease(&mut c),
        "fd1" => s_fd1(&mut c),
        "fd2" => s_fd2(&mut c),
        "fd3" => s_fd3(&mut c),
        "churn" => s_churn(&mut c, cycles, seed),
        "churn_relay" => s_relay(&mut c),
        "zombies" => s_zombies(&mut c),
        "churn_vrace" | "vrace_nowrap" => s_vrace(&mut c, cycles),
        s => {
            println!("unknown scenario {s}");
            std::process::exit(2);
        }
    }
    println!(
        "SUPSUMMARY {scen} pass={} fail={} note={} end {}",
        c.pass,
        c.fail,
        c.note,
        crate::utc()
    );
}
