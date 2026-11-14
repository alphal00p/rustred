//! The DRIVER: pid 1 of a private user+pid namespace. It logs every signal it receives (SA_SIGINFO handler), spawns
//! and reaps every decoy (so it knows the exact wait status of each), serves decoy requests of the supervisor process,
//! and at the end checks every decoy. The supervisor under test runs as a GRANDCHILD in its own session (so pid 1 is
//! neither its parent nor its session / group leader: only the layer's own rules protect pid 1).
use crate::{CAMP, alive, hex, rb_cmd, stat, which, write_atomic};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::time::{Duration, Instant};

static SIGCOUNT: [AtomicU32; 65] = [const { AtomicU32::new(0) }; 65];
static SIGFROM: [AtomicI32; 65] = [const { AtomicI32::new(0) }; 65];

extern "C" fn on_sig(sig: libc::c_int, info: *mut libc::siginfo_t, _c: *mut libc::c_void) {
    if (0..65).contains(&sig) {
        SIGCOUNT[sig as usize].fetch_add(1, Ordering::SeqCst);
        // SAFETY: the kernel passes a valid siginfo with SA_SIGINFO.
        let p = unsafe { (*info).si_pid() };
        SIGFROM[sig as usize].store(p, Ordering::SeqCst);
    }
}

fn install_handlers() {
    for s in 1..=64 {
        // not KILL/STOP (cannot), not CHLD (we reap), not glibc-internal 32/33, not real faults (would loop)
        if [
            libc::SIGKILL,
            libc::SIGSTOP,
            libc::SIGCHLD,
            32,
            33,
            4,
            7,
            8,
            11,
        ]
        .contains(&s)
        {
            continue;
        }
        // SAFETY: a zeroed sigaction with our handler.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = on_sig as *const () as usize;
            sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(s, &sa, std::ptr::null_mut());
        }
    }
}

struct Decoy {
    name: String,
    class: String,
    pid: u32,
    start: u64,
    status: Option<i32>,
}

enum Kid {
    Decoy,
    Mid,
    Churn,
    Junk,
}

#[derive(Default)]
struct Churn {
    on: bool,
    live: usize,
    spawned: u64,
    normal: u64,
    other_exit: u64,
    signaled: Vec<(u32, i32)>,
    forced_ok: u64,
    forced_fail: u64,
    freed_pos: usize,
    n: u64,
}

struct Drv {
    run: PathBuf,
    decoys: Vec<Decoy>,
    kids: HashMap<u32, Kid>,
    mid_status: HashMap<u32, i32>,
    orphan_signaled: Vec<(u32, i32)>,
    handled: BTreeSet<String>,
    churn: Churn,
    sleep: PathBuf,
}

fn decode_status(st: i32) -> String {
    if libc::WIFEXITED(st) {
        format!("exit({})", libc::WEXITSTATUS(st))
    } else if libc::WIFSIGNALED(st) {
        format!("SIGNAL({})", libc::WTERMSIG(st))
    } else {
        format!("raw({st})")
    }
}

impl Drv {
    fn reap(&mut self) {
        loop {
            let mut st = 0;
            // SAFETY: waitpid with a valid out-pointer; we are pid 1 of our own namespace.
            let p = unsafe { libc::waitpid(-1, &mut st, libc::WNOHANG) };
            if p <= 0 {
                break;
            }
            let p = p as u32;
            match self.kids.remove(&p) {
                Some(Kid::Mid) => {
                    self.mid_status.insert(p, st);
                }
                Some(Kid::Churn) => {
                    self.churn.live = self.churn.live.saturating_sub(1);
                    if libc::WIFSIGNALED(st) {
                        self.churn.signaled.push((p, libc::WTERMSIG(st)));
                    } else if libc::WIFEXITED(st) && libc::WEXITSTATUS(st) == 0 {
                        self.churn.normal += 1;
                    } else {
                        self.churn.other_exit += 1;
                    }
                }
                Some(Kid::Junk) => {}
                Some(Kid::Decoy) | None => {
                    if let Some(d) = self
                        .decoys
                        .iter_mut()
                        .rev()
                        .find(|d| d.pid == p && d.status.is_none())
                    {
                        d.status = Some(st);
                    } else if libc::WIFSIGNALED(st) {
                        self.orphan_signaled.push((p, libc::WTERMSIG(st)));
                    }
                }
            }
        }
    }

    fn spawn_raw(
        &self,
        argv: &[String],
        env: &[String],
        setsid: bool,
        setpgid: bool,
    ) -> std::io::Result<u32> {
        let mut args: Vec<String> = vec!["helper".into(), "rawexec".into()];
        args.extend(env.iter().cloned());
        args.push("--".into());
        args.extend(argv.iter().cloned());
        let mut c = rb_cmd(&args);
        c.env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: async-signal-safe calls between fork and exec.
        unsafe {
            c.pre_exec(move || {
                if setsid {
                    libc::setsid();
                } else if setpgid {
                    libc::setpgid(0, 0);
                }
                Ok(())
            });
        }
        let child = c.spawn()?;
        let pid = child.id();
        std::mem::forget(child); // reaped by waitpid(-1)
        Ok(pid)
    }

    /// Spawn with a forced pid (ns_last_pid; namespace only). Wrong-pid attempts are our own unreaped children: killed.
    fn spawn_forced(
        &mut self,
        want: u32,
        argv: &[String],
        env: &[String],
        setsid: bool,
        setpgid: bool,
        tries: usize,
    ) -> Option<u32> {
        for _ in 0..tries {
            if std::fs::write("/proc/sys/kernel/ns_last_pid", (want - 1).to_string()).is_err() {
                return None;
            }
            let Ok(p) = self.spawn_raw(argv, env, setsid, setpgid) else {
                continue;
            };
            if p == want {
                return Some(p);
            }
            // SAFETY: our own just-spawned, unreaped child inside our namespace.
            unsafe { libc::kill(p as i32, libc::SIGKILL) };
            self.kids.insert(p, Kid::Junk);
            self.reap();
        }
        None
    }

    fn spawn_decoy(&mut self, spec: &Value) -> Value {
        let name = spec["name"].as_str().unwrap_or("d").to_string();
        let class = spec["class"].as_str().unwrap_or("decoy").to_string();
        let argv: Vec<String> = spec["argv"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_else(|| vec![self.sleep.display().to_string(), "3000".into()]);
        let env: Vec<String> = spec["env"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_default();
        let setsid = spec["setsid"].as_bool().unwrap_or(true);
        let setpgid = spec["setpgid"].as_bool().unwrap_or(false);
        let pid = match spec["force_pid"].as_u64() {
            Some(w) => self.spawn_forced(w as u32, &argv, &env, setsid, setpgid, 400),
            None => self.spawn_raw(&argv, &env, setsid, setpgid).ok(),
        };
        let Some(pid) = pid else {
            println!(
                "DRIVER decoy {name}: SPAWN FAILED (forced={:?})",
                spec["force_pid"]
            );
            return json!({"ok": false});
        };
        self.kids.insert(pid, Kid::Decoy);
        std::thread::sleep(Duration::from_millis(30));
        let start = stat(pid).map(|s| s.start_ticks).unwrap_or(0);
        let leader_class = spec["leader_class"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| class.clone());
        self.decoys.push(Decoy {
            name: name.clone(),
            class: leader_class,
            pid,
            start,
            status: None,
        });
        let mut rep = json!({"ok": true, "pid": pid, "start": start});
        if let Some(f) = spec["member_pidfile"].as_str()
            && let Some(m) = crate::read_u32_file(Path::new(f), Duration::from_secs(10))
        {
            let ms = stat(m).map(|s| s.start_ticks).unwrap_or(0);
            self.decoys.push(Decoy {
                name: format!("{name}.member"),
                class,
                pid: m,
                start: ms,
                status: None,
            });
            rep["member_pid"] = json!(m);
            rep["member_start"] = json!(ms);
        }
        rep
    }

    fn handle_requests(&mut self) {
        let dir = self.run.join("req");
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut names: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        for n in names {
            if self.handled.contains(&n) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(dir.join(&n)) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            self.handled.insert(n.clone());
            let rep = match v["kind"].as_str() {
                Some("decoy") => self.spawn_decoy(&v),
                Some("register") => {
                    self.decoys.push(Decoy {
                        name: v["name"].as_str().unwrap_or("r").into(),
                        class: v["class"].as_str().unwrap_or("any").into(),
                        pid: v["pid"].as_u64().unwrap() as u32,
                        start: v["start"].as_u64().unwrap(),
                        status: None,
                    });
                    json!({"ok": true})
                }
                _ => json!({"ok": false}),
            };
            write_atomic(&self.run.join("rep").join(&n), &rep.to_string());
        }
    }

    fn churn_tick(&mut self) {
        if !self.churn.on {
            return;
        }
        let sleep = self.sleep.display().to_string();
        let other = hex(b"RR_CAMPAIGN_JOB=othercamp:churn");
        if self.churn.live < 40 {
            self.churn.n += 1;
            let k = self.churn.n % 4;
            let (argv, env, ss, pg): (Vec<String>, Vec<String>, bool, bool) = match k {
                0 => (vec![sleep.clone(), "0.5".into()], vec![], false, false),
                1 => (
                    vec![sleep.clone(), "0.5".into()],
                    vec![other.clone()],
                    false,
                    false,
                ),
                2 => (vec![sleep.clone(), "0.8".into()], vec![], true, false),
                _ => (
                    vec![sleep.clone(), "0.3".into()],
                    vec![other.clone()],
                    false,
                    true,
                ),
            };
            if let Ok(p) = self.spawn_raw(&argv, &env, ss, pg) {
                self.kids.insert(p, Kid::Churn);
                self.churn.live += 1;
                self.churn.spawned += 1;
            }
        }
        // Pids just freed by the supervisor's jobs: put a decoy on them (forced reuse).
        if std::env::var("RB_NOFORCE").is_ok() {
            return;
        }
        let f = self.run.join("freed");
        if let Ok(text) = std::fs::read_to_string(&f)
            && text.len() > self.churn.freed_pos
        {
            let new = text[self.churn.freed_pos..].to_string();
            let complete = new.rfind('\n').map(|i| i + 1).unwrap_or(0);
            self.churn.freed_pos += complete;
            for line in new[..complete].lines().take(4) {
                let Ok(p) = line.trim().parse::<u32>() else {
                    continue;
                };
                if p < 3 || stat(p).is_some() {
                    continue;
                }
                let env = if p % 2 == 0 {
                    vec![]
                } else {
                    vec![other.clone()]
                };
                match self.spawn_forced(p, &[sleep.clone(), "1.5".into()], &env, true, false, 20) {
                    Some(q) => {
                        self.kids.insert(q, Kid::Churn);
                        self.churn.live += 1;
                        self.churn.spawned += 1;
                        self.churn.forced_ok += 1;
                    }
                    None => self.churn.forced_fail += 1,
                }
            }
        }
    }

    fn run_sup(&mut self, args: &[&str], strace: Option<&str>) -> u32 {
        let _ = strace; // strace is not used in the ported harness
        let mut v: Vec<String> = vec!["mid".into(), "-".into(), "--".into(), "sup".into()];
        v.extend(args.iter().map(|a| a.to_string()));
        v.push(self.run.display().to_string());
        let mut c = rb_cmd(&v);
        c.env_remove("RR_CAMPAIGN_JOB").stdin(Stdio::null());
        // SAFETY: setsid between fork and exec.
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let ch = c.spawn().unwrap();
        let p = ch.id();
        std::mem::forget(ch);
        self.kids.insert(p, Kid::Mid);
        p
    }

    fn pump_until(&mut self, deadline: Instant, mut done: impl FnMut(&Drv) -> bool) -> bool {
        loop {
            self.handle_requests();
            self.reap();
            self.churn_tick();
            if done(self) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_mids(&mut self, mids: &[u32], deadline: Instant) {
        let m: Vec<u32> = mids.to_vec();
        let ok = self.pump_until(deadline, |d| m.iter().all(|p| d.mid_status.contains_key(p)));
        if !ok {
            println!("DRIVER TIMEOUT waiting for supervisor(s) {m:?}");
        }
        for p in mids {
            if let Some(st) = self.mid_status.get(p) {
                println!("DRIVER supervisor-mid {p} ended {}", decode_status(*st));
            }
        }
    }

    fn final_report(&mut self, scenario: &str) {
        std::thread::sleep(Duration::from_millis(500));
        self.reap();
        let (
            mut bad,
            mut ours_missed,
            mut ours_ok,
            mut decoys_ok,
            mut decoys_n,
            mut by_killed,
            mut by_n,
        ) = (0, 0, 0, 0, 0, 0, 0);
        for d in &self.decoys {
            let a = alive(d.pid, d.start);
            let st = d.status.map(decode_status).unwrap_or_else(|| "-".into());
            let verdict = match d.class.as_str() {
                "decoy" => {
                    decoys_n += 1;
                    if a {
                        decoys_ok += 1;
                        "ok"
                    } else {
                        bad += 1;
                        "CRITICAL:DECOY-DEAD"
                    }
                }
                "ours" => {
                    if a {
                        ours_missed += 1;
                        "FAIL:MARKED-ORPHAN-ALIVE"
                    } else {
                        ours_ok += 1;
                        "ok(killed)"
                    }
                }
                "bydesign" => {
                    by_n += 1;
                    if !a {
                        by_killed += 1;
                    }
                    if a {
                        "note:alive"
                    } else {
                        "note:killed(by design: carries this campaign's marker)"
                    }
                }
                _ => "-",
            };
            println!(
                "DECOY {scenario} name={} class={} pid={} start={} alive={a} status={st} verdict={verdict}",
                d.name, d.class, d.pid, d.start
            );
        }
        let mut sigs = vec![];
        for (s, c) in SIGCOUNT.iter().enumerate() {
            let n = c.load(Ordering::SeqCst);
            if n > 0 {
                sigs.push(format!(
                    "sig{s}x{n}(from pid {})",
                    SIGFROM[s].load(Ordering::SeqCst)
                ));
            }
        }
        if self.churn.on {
            println!(
                "CHURN {scenario} decoys_spawned={} exited_normally={} other_exit={} killed_by_signal={} forced_reuse_ok={} forced_reuse_fail={} live_at_end={}",
                self.churn.spawned,
                self.churn.normal,
                self.churn.other_exit,
                self.churn.signaled.len(),
                self.churn.forced_ok,
                self.churn.forced_fail,
                self.churn.live
            );
            for (p, s) in &self.churn.signaled {
                println!("CHURN-KILLED pid={p} signal={s}  <-- CRITICAL");
            }
        }
        let mut sig_orph: HashMap<i32, usize> = HashMap::new();
        for (_, s) in &self.orphan_signaled {
            *sig_orph.entry(*s).or_default() += 1;
        }
        println!(
            "SUMMARY {scenario} decoys_alive={decoys_ok}/{decoys_n} decoys_dead={bad} marked_orphans_killed={ours_ok} marked_orphans_missed={ours_missed} bydesign_killed={by_killed}/{by_n} driver_signals={} reaped_orphans_by_signal={:?}",
            if sigs.is_empty() {
                "none".into()
            } else {
                sigs.join(",")
            },
            sig_orph
        );
    }
}

/// `rb mid <strace-file|-> -- argv...`: run argv (own session already set by the driver) and exit with its status.
pub fn mid(a: &[String]) {
    let sep = a.iter().position(|x| x == "--").unwrap();
    let argv = &a[sep + 1..];
    let mut ch = rb_cmd(argv).spawn().expect("spawn supervisor");
    let s = ch.wait().unwrap();
    use std::os::unix::process::ExitStatusExt;
    std::process::exit(s.code().unwrap_or(128 + s.signal().unwrap_or(0)));
}

fn d(name: &str, class: &str, env: &[&[u8]], setsid: bool) -> Value {
    json!({"name": name, "class": class, "env": env.iter().map(|e| hex(e)).collect::<Vec<_>>(), "setsid": setsid})
}

pub fn main(a: &[String]) {
    // SAFETY: getpid cannot fail.
    if unsafe { libc::getpid() } != 1 {
        eprintln!("REFUSED: the driver runs only as pid 1 of a private pid namespace");
        std::process::exit(3);
    }
    install_handlers();
    let scenario = a[0].clone();
    let run = PathBuf::from(&a[1]).join(&scenario);
    let strace = a.get(2).map(|s| s == "strace").unwrap_or(false);
    let _ = std::fs::remove_dir_all(&run);
    std::fs::create_dir_all(run.join("req")).unwrap();
    std::fs::create_dir_all(run.join("rep")).unwrap();
    let mut drv = Drv {
        run: run.clone(),
        decoys: vec![],
        kids: HashMap::new(),
        mid_status: HashMap::new(),
        orphan_signaled: vec![],
        handled: BTreeSet::new(),
        churn: Churn::default(),
        sleep: which("sleep"),
    };
    println!(
        "DRIVER {scenario} start {} (pid 1, run dir {})",
        crate::utc(),
        run.display()
    );
    let stfile = |n: &str| run.join(format!("strace.{n}.txt")).display().to_string();
    // Decoys in SESSION 0 (outside view) must exist before we setsid.
    if scenario == "pid0" {
        for s in [
            d("sid0_unmarked", "decoy", &[], false),
            d(
                "sid0_other",
                "decoy",
                &[b"RR_CAMPAIGN_JOB=othercamp:L0"],
                false,
            ),
        ] {
            drv.spawn_decoy(&s);
        }
    }
    // SAFETY: setsid of pid 1 (it is not a group leader: its group is the unshare parent's).
    unsafe { libc::setsid() };
    if scenario.starts_with("churn") && std::env::var("RB_PIDMAX").as_deref() != Ok("none") {
        match std::fs::write("/proc/sys/kernel/pid_max", "4096") {
            Ok(()) => println!("DRIVER pid_max in this namespace set to 4096 (namespaced sysctl)"),
            Err(e) => println!("DRIVER pid_max NOT set ({e}); ns_last_pid only"),
        }
    }
    if scenario.starts_with("churn") {
        drv.churn.on = true;
    }
    match scenario.as_str() {
        "pid1" | "pid0" | "bigpid" => {
            // In pid 1's own group and session (a killpg(1) or kill(-1) would hit them), and in their own sessions.
            for s in [
                d("g1_unmarked", "decoy", &[], false),
                d(
                    "g1_other",
                    "decoy",
                    &[b"RR_CAMPAIGN_JOB=othercamp:L1"],
                    false,
                ),
                d("own_unmarked", "decoy", &[], true),
                d(
                    "own_other",
                    "decoy",
                    &[b"RR_CAMPAIGN_JOB=othercamp:L1"],
                    true,
                ),
            ] {
                drv.spawn_decoy(&s);
            }
        }
        _ => {}
    }
    let deadline = Instant::now()
        + Duration::from_secs(if scenario.starts_with("churn") {
            1500
        } else {
            280
        });
    match scenario.as_str() {
        "recover" => {
            let m1 = drv.run_sup(&["recover1"], None);
            let rd = run.join("ready");
            drv.pump_until(deadline, |dv| {
                rd.exists() || dv.mid_status.contains_key(&m1)
            });
            let text = std::fs::read_to_string(run.join("sup1.pid")).unwrap_or_default();
            let f: Vec<u64> = text
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if f.len() == 2 && alive(f[0] as u32, f[1]) {
                // SAFETY: our own descendant (start time verified just now) inside our namespace.
                unsafe { libc::kill(f[0] as i32, libc::SIGKILL) };
                println!(
                    "DRIVER SIGKILLed supervisor-1 pid {} (simulated crash)",
                    f[0]
                );
            } else {
                println!("DRIVER could not find supervisor-1 ({text:?})");
            }
            drv.wait_mids(&[m1], deadline);
            let t = Instant::now() + Duration::from_secs(2);
            drv.pump_until(t, |_| false);
            let scan =
                rustred_campaign::resources::scan_campaign(CAMP, 0, &BTreeSet::new()).unwrap();
            println!(
                "DRIVER expected marked orphans before recovery: {}",
                scan.processes.len()
            );
            for p in &scan.processes {
                println!(
                    "DRIVER   orphan pid={} start={} attempt={} ppid={} sid={} pgrp={} comm={} state={}",
                    p.pid, p.start_ticks, p.attempt_id, p.ppid, p.session, p.pgrp, p.comm, p.state
                );
                drv.decoys.push(Decoy {
                    name: format!("orphan:{}:{}", p.attempt_id, p.pid),
                    class: "ours".into(),
                    pid: p.pid,
                    start: p.start_ticks,
                    status: None,
                });
            }
            let st = stfile("recover2");
            let m2 = drv.run_sup(&["recover2"], strace.then_some(st.as_str()));
            drv.wait_mids(&[m2], deadline);
        }
        "two" => {
            let ma = drv.run_sup(&["twoA"], None);
            let mb = drv.run_sup(&["twoB"], None);
            drv.wait_mids(&[ma, mb], deadline);
        }
        s => {
            let st = stfile(s);
            let m = drv.run_sup(&[s], strace.then_some(st.as_str()));
            drv.wait_mids(&[m], deadline);
        }
    }
    drv.final_report(&scenario);
    if strace {
        for e in std::fs::read_dir(&run).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("strace.") {
                summarize_strace(&e.path(), &drv);
            }
        }
    }
    println!("DRIVER {scenario} end {}", crate::utc());
}

/// Every signal syscall in the strace log, classified by target: decoys, pid 1, <= 0 (broadcast / group), others.
fn summarize_strace(path: &Path, drv: &Drv) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let decoy_pids: BTreeSet<i64> = drv
        .decoys
        .iter()
        .filter(|d| d.class == "decoy")
        .map(|d| i64::from(d.pid))
        .collect();
    let (mut kill_calls, mut pidfd_sends, mut pidfd_opens, mut bad) = (0, 0, 0, vec![]);
    let mut opened_decoy = 0;
    for line in text.lines() {
        let l = line.split_once(' ').map(|x| x.1).unwrap_or(line).trim();
        if let Some(r) = l
            .strip_prefix("kill(")
            .or_else(|| l.strip_prefix("tkill("))
            .or_else(|| l.strip_prefix("tgkill("))
        {
            kill_calls += 1;
            let target: i64 = r
                .split([',', ')'])
                .next()
                .unwrap_or("")
                .trim()
                .parse()
                .unwrap_or(0);
            if (-1..=1).contains(&target) || decoy_pids.contains(&target.abs()) {
                bad.push(line.to_string());
            }
        } else if l.starts_with("pidfd_send_signal(") {
            pidfd_sends += 1;
        } else if let Some(r) = l.strip_prefix("pidfd_open(") {
            pidfd_opens += 1;
            let target: i64 = r
                .split([',', ')'])
                .next()
                .unwrap_or("")
                .trim()
                .parse()
                .unwrap_or(0);
            if decoy_pids.contains(&target) {
                opened_decoy += 1;
            }
        }
    }
    println!(
        "STRACE {} kill/tkill/tgkill={kill_calls} pidfd_open={pidfd_opens} (of decoys: {opened_decoy}) pidfd_send_signal={pidfd_sends} kill-to-{{-1,0,1}}-or-decoy(pid/group)={}",
        path.display(),
        bad.len()
    );
    for b in bad.iter().take(20) {
        println!("STRACE-SUSPECT {b}");
    }
}
