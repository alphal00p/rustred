//! procfix7 round 14 (host step 3, reports/HOST-STEP-3-ANALYSIS.md): the USER MANAGER as a host condition, separate
//! from the job condition.
//!
//! M3, THE MANAGER GATE ([`ManagerGate`]): while a scope launch is PENDING (the launcher still waiting for the manager,
//! decided from the kernel: `launch::LaunchState`) longer than `resources.scope_pending_warn_s`, or while the manager
//! probe does not answer, the supervisor starts NO new job (running jobs go on: they need no manager). The probe is ONE
//! `systemctl --user show -p Version --value` on its own worker thread: at most one in flight (a stalled manager holds
//! at most one probe of ours), never per job; every 30 s while the gate is open, backing off from 5 s to 60 s while it is
//! closed. The gate opens again when no launch is pending too long and a probe that STARTED after the closure answered
//! within [`PROBE_BOUND`]. Closed longer than `resources.manager_wait_limit_s`: the run drains and stops typed (exit 69,
//! "manager unavailable").
//!
//! M4, THE LEFTOVER UNITS ([`LEFTOVER_FILE`] in the campaign directory, JSON, rewritten atomically): the exact unit names
//! of OUR scopes that could not be confirmed cleared (a launcher ended while its request may still be queued in the
//! manager; a unit not read, still active, or not reset at the exit). Cleared by [`clear_leftovers`] at the next start,
//! at the exit when the manager answers, and by `rr_campaign units clear <dir>`: each name must be this campaign's
//! (`rr-<campaign id>-...-<16 hex>`, the BLAKE3 of the campaign id with its nonce and the label) and recorded by this
//! campaign's ledger; its state is READ first; not loaded: done; failed: reset-failed; active / activating /
//! deactivating with the kernel showing its cgroup absent or `populated 0`: stopped (an empty scope: no process can
//! receive anything); populated: never stopped (its members go through the marker-based backstop); unreadable: kept.
//! Never a pattern.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A probe that has not answered within this is "no answer" (the gate closes; the probe itself stays in flight).
pub(crate) const PROBE_BOUND: Duration = Duration::from_secs(5);
/// The probe period while the gate is open.
const PROBE_PERIOD_OPEN: Duration = Duration::from_secs(30);
/// The back-off of the probe while the gate is closed: from 5 s doubling to 60 s.
const PROBE_BACKOFF_MIN: Duration = Duration::from_secs(5);
const PROBE_BACKOFF_MAX: Duration = Duration::from_secs(60);

/// What changed in one [`ManagerGate::tick`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum GateChange {
    None,
    /// The gate closed (why).
    Closed(String),
    /// The gate opened again after this long.
    Opened(Duration),
}

pub(crate) struct ManagerGate {
    /// Probe at all (job scopes in use, or `manager_probe = "on"`, or the test emulation).
    pub(crate) probing: bool,
    allow_in_namespace: bool,
    /// The probe in flight: when it started, and its answer channel.
    in_flight: Option<(Instant, mpsc::Receiver<Result<Duration, String>>)>,
    /// The latest completed probe: when it STARTED, and its answer.
    last: Option<(Instant, Result<Duration, String>)>,
    next_probe: Instant,
    backoff: Duration,
    /// Closed since, and why (the first reason).
    closed: Option<(Instant, String)>,
}

impl ManagerGate {
    pub(crate) fn new(probing: bool, allow_in_namespace: bool) -> Self {
        Self {
            probing,
            allow_in_namespace,
            in_flight: None,
            last: None,
            next_probe: Instant::now(),
            backoff: PROBE_BACKOFF_MIN,
            closed: None,
        }
    }

    /// Closed: since when, and why.
    pub(crate) fn closed(&self) -> Option<&(Instant, String)> {
        self.closed.as_ref()
    }

    /// Does the manager answer now: the latest completed probe answered within the bound and none is in flight longer
    /// than the bound? (None: never probed.)
    pub(crate) fn answers(&self) -> Option<bool> {
        if self
            .in_flight
            .as_ref()
            .is_some_and(|(t, _)| t.elapsed() > PROBE_BOUND)
        {
            return Some(false);
        }
        self.last
            .as_ref()
            .map(|(_, r)| matches!(r, Ok(d) if *d <= PROBE_BOUND))
    }

    /// One tick: collect a finished probe, start one when due (never a second while one is in flight), decide the gate.
    /// `pending_too_long`: the oldest pending scope launch is older than the warn time (its unit, for the reason).
    pub(crate) fn tick(&mut self, pending_too_long: Option<&str>) -> GateChange {
        let now = Instant::now();
        if let Some((t0, rx)) = &self.in_flight {
            match rx.try_recv() {
                Ok(r) => {
                    self.last = Some((*t0, r));
                    self.in_flight = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.last = Some((
                        *t0,
                        Err("the probe's worker ended without an answer".into()),
                    ));
                    self.in_flight = None;
                }
            }
        }
        if self.probing && self.in_flight.is_none() && now >= self.next_probe {
            let (tx, rx) = mpsc::channel();
            let allow = self.allow_in_namespace;
            let spawned = std::thread::Builder::new()
                .name("rr-manager-probe".into())
                .spawn(move || {
                    #[allow(clippy::let_underscore_must_use)]
                    // ignored-ok (procfix7): round 14: the receiver is gone only when the supervisor ends
                    let _ = tx.send(crate::launch::manager_probe(allow));
                });
            match spawned {
                Ok(_) => self.in_flight = Some((now, rx)),
                Err(e) => self.last = Some((now, Err(format!("probe thread: {e}")))),
            }
            self.next_probe = now
                + if self.closed.is_some() {
                    let b = self.backoff;
                    self.backoff = (self.backoff * 2).min(PROBE_BACKOFF_MAX);
                    b
                } else {
                    PROBE_PERIOD_OPEN
                };
        }
        let probe_bad = self.probing && self.answers() == Some(false);
        match (&self.closed, pending_too_long, probe_bad) {
            (None, Some(u), _) => {
                let why = format!("a scope launch is pending longer than the warn time ({u})");
                self.close(now, why.clone());
                GateChange::Closed(why)
            }
            (None, None, true) => {
                let why = format!(
                    "the user manager does not answer the probe ({})",
                    match &self.last {
                        Some((_, Err(e))) => e.clone(),
                        Some((_, Ok(d))) => format!("{:.1} s", d.as_secs_f64()),
                        None => format!("no answer within {} s", PROBE_BOUND.as_secs()),
                    }
                );
                self.close(now, why.clone());
                GateChange::Closed(why)
            }
            (Some((since, _)), None, false) => {
                // open again only on a probe that STARTED after the closure and answered (when probing at all)
                let fresh = !self.probing
                    || self.last.as_ref().is_some_and(|(t, r)| {
                        *t >= *since && matches!(r, Ok(d) if *d <= PROBE_BOUND)
                    });
                if fresh {
                    let after = since.elapsed();
                    self.closed = None;
                    self.backoff = PROBE_BACKOFF_MIN;
                    self.next_probe = now + PROBE_PERIOD_OPEN;
                    GateChange::Opened(after)
                } else {
                    GateChange::None
                }
            }
            _ => GateChange::None,
        }
    }

    fn close(&mut self, now: Instant, why: String) {
        self.closed = Some((now, why));
        self.backoff = PROBE_BACKOFF_MIN;
        // a fresh probe soon (the one in flight, if any, keeps its slot)
        self.next_probe = now;
    }
}

/// The leftover-unit list's file in the campaign directory.
pub(crate) const LEFTOVER_FILE: &str = "units.leftover.json";

/// One leftover unit of ours.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Leftover {
    /// The exact unit name WITHOUT `.scope`.
    pub unit: String,
    /// The attempt (`<job>:a<k>`) that launched it.
    pub attempt: String,
    pub why: String,
    pub utc: String,
}

/// The list (empty when the file does not exist). Err: it exists and cannot be read or parsed.
pub(crate) fn load_leftovers(cdir: &Path) -> Result<Vec<Leftover>, String> {
    let p = cdir.join(LEFTOVER_FILE);
    match crate::fsx::read("leftover", &p) {
        crate::fsx::Read::Content(b) => {
            serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))
        }
        crate::fsx::Read::Absent => Ok(Vec::new()),
        crate::fsx::Read::Failed(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Write the list atomically (the file is removed when the list is empty).
pub(crate) fn store_leftovers(cdir: &Path, list: &[Leftover]) -> Result<(), String> {
    let p = cdir.join(LEFTOVER_FILE);
    if list.is_empty() {
        return match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{}: {e}", p.display())),
        };
    }
    let b = serde_json::to_vec_pretty(list).map_err(|e| e.to_string())?;
    crate::atomic::write_atomic(&p, &b).map_err(|e| format!("{}: {e}", p.display()))
}

/// Add entries (a name already listed is kept once).
pub(crate) fn add_leftovers(cdir: &Path, new: Vec<Leftover>) -> Result<usize, String> {
    let mut list = load_leftovers(cdir)?;
    let mut n = 0;
    for l in new {
        if !list.iter().any(|x| x.unit == l.unit) {
            list.push(l);
            n += 1;
        }
    }
    store_leftovers(cdir, &list)?;
    Ok(n)
}

/// Is `unit` a scope name THIS campaign creates (`launch::scope_unit_name` of this campaign id and some label: the
/// cleaned campaign id as prefix and 16 hex as suffix)?
pub(crate) fn ours(unit: &str, campaign_id: &str) -> bool {
    let probe = crate::launch::scope_unit_name(campaign_id, "x");
    let Some(prefix) = probe
        .rsplit_once('-')
        .and_then(|(stem, _)| stem.strip_suffix("-x"))
    else {
        return false;
    };
    let Some((stem, hex)) = unit.rsplit_once('-') else {
        return false;
    };
    stem.starts_with(&format!("{prefix}-"))
        && hex.len() == 16
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The scope units this campaign's ledger recorded at its launches (None: the ledger cannot be read).
pub(crate) fn recorded_scope_units(cdir: &Path) -> Option<BTreeSet<String>> {
    use crate::ledger::{Event, JobEvent};
    let recs = crate::ledger::read_records(cdir).ok()?;
    Some(
        recs.into_iter()
            .filter_map(|r| match r.event {
                Event::Job {
                    ev:
                        JobEvent::Launch {
                            scope_unit: Some(u),
                            ..
                        },
                    ..
                } => Some(u),
                _ => None,
            })
            .collect(),
    )
}

/// What [`clear_leftovers`] did with one unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cleared {
    pub unit: String,
    /// "not loaded" | "reset-failed" | "stopped (empty)" | "kept: <why>" | "refused: <why>".
    pub action: String,
    /// The entry leaves the list.
    pub done: bool,
}

/// The kernel's view of a scope's cgroup: Some(true) populated, Some(false) not populated or absent, None unknown.
fn scope_populated(unit: &str) -> Option<bool> {
    let uid = crate::resources::our_uid();
    let root = PathBuf::from(format!(
        "/sys/fs/cgroup/user.slice/user-{uid}.slice/user@{uid}.service"
    ));
    let mut any_unknown = false;
    for d in [
        root.join("app.slice").join(format!("{unit}.scope")),
        root.join(format!("{unit}.scope")),
    ] {
        match crate::fsx::read_to_string("cgroup", &d.join("cgroup.events")) {
            crate::fsx::Read::Content(t) => {
                return Some(t.lines().any(|l| l.trim() == "populated 1"));
            }
            crate::fsx::Read::Absent => {}
            crate::fsx::Read::Failed(_) => any_unknown = true,
        }
    }
    (!any_unknown).then_some(false)
}

/// procfix7 round 14 (M4): clear the leftover list of `cdir` by exact names (see the module doc). `recorded`: the
/// scope units this campaign's ledger recorded at its launches (None: the ledger could not be read: nothing is done).
/// The list is rewritten without the entries cleared; the outcome of every entry is returned (the caller records it).
pub(crate) fn clear_leftovers(
    cdir: &Path,
    campaign_id: &str,
    recorded: Option<&BTreeSet<String>>,
) -> Result<Vec<Cleared>, String> {
    let list = load_leftovers(cdir)?;
    if list.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut keep = Vec::new();
    for l in list {
        let c = clear_one(&l.unit, campaign_id, recorded);
        if !c.done {
            keep.push(l);
        }
        out.push(c);
    }
    store_leftovers(cdir, &keep)?;
    Ok(out)
}

fn clear_one(unit: &str, campaign_id: &str, recorded: Option<&BTreeSet<String>>) -> Cleared {
    let c = |action: String, done: bool| Cleared {
        unit: unit.to_string(),
        action,
        done,
    };
    if !ours(unit, campaign_id) {
        return c("refused: not a scope name of this campaign".into(), false);
    }
    match recorded {
        None => {
            return c(
                "kept: the ledger's launch records could not be read".into(),
                false,
            );
        }
        Some(r) if !r.contains(unit) => {
            return c(
                "refused: not recorded by this campaign's ledger".into(),
                false,
            );
        }
        Some(_) => {}
    }
    let st = match crate::launch::scope_unit_state(unit) {
        Ok(st) => st,
        Err(e) => return c(format!("kept: state unreadable ({e})"), false),
    };
    if !st.loaded() {
        return c("not loaded".into(), true);
    }
    match st.active.as_str() {
        "failed" => match crate::launch::scope_reset_failed(unit) {
            Ok(()) => c(format!("reset-failed (result {})", st.result), true),
            Err(e) => c(format!("kept: reset-failed: {e}"), false),
        },
        "inactive" => c("inactive (unloaded by the manager)".into(), true),
        "active" | "activating" | "deactivating" | "reloading" => match scope_populated(unit) {
            Some(false) => match crate::launch::scope_stop(unit) {
                Ok(()) => c(
                    format!("stopped (was {}, its cgroup empty)", st.active),
                    true,
                ),
                Err(e) => c(format!("kept: stop: {e}"), false),
            },
            Some(true) => c(
                format!(
                    "kept: {} and POPULATED (never stopped; the marker backstop)",
                    st.active
                ),
                false,
            ),
            None => c(
                format!("kept: {} and its cgroup unreadable", st.active),
                false,
            ),
        },
        other => c(format!("kept: state {other}"), false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// procfix7 round 14 (M3): without the probe, the gate closes on a launch pending too long and opens again when no
    /// launch is pending too long; never twice in a row.
    #[test]
    fn the_gate_closes_on_a_pending_launch_and_opens_when_none_is_pending() {
        let mut g = ManagerGate::new(false, false);
        assert_eq!(g.tick(None), GateChange::None);
        assert!(g.closed().is_none());
        assert!(
            matches!(g.tick(Some("j a1 pending 6 s")), GateChange::Closed(w) if w.contains("j a1 pending 6 s"))
        );
        assert!(g.closed().is_some());
        assert_eq!(g.tick(Some("j a1 pending 7 s")), GateChange::None);
        assert!(matches!(g.tick(None), GateChange::Opened(_)));
        assert!(g.closed().is_none());
        assert_eq!(g.tick(None), GateChange::None);
    }

    /// procfix7 round 14 (M3): with the probe on, a probe that answered BEFORE the closure never reopens the gate; the
    /// gate waits for a fresh answer (here: the probe's worker reports an error inside a namespace without a manager).
    #[test]
    fn the_gate_reopens_only_on_a_fresh_probe_answer() {
        let mut g = ManagerGate::new(true, false);
        g.last = Some((Instant::now(), Ok(Duration::from_millis(5))));
        g.next_probe = Instant::now() + Duration::from_secs(3600);
        assert!(matches!(
            g.tick(Some("j a1 pending 6 s")),
            GateChange::Closed(_)
        ));
        // the old answer predates the closure: still closed although nothing is pending
        g.next_probe = Instant::now() + Duration::from_secs(3600);
        assert_eq!(g.tick(None), GateChange::None);
        // a fresh answer after the closure opens it
        g.last = Some((Instant::now(), Ok(Duration::from_millis(5))));
        assert!(matches!(g.tick(None), GateChange::Opened(_)));
        // a fresh FAILED probe closes it again
        g.last = Some((Instant::now(), Err("no answer".into())));
        g.next_probe = Instant::now() + Duration::from_secs(3600);
        assert!(matches!(g.tick(None), GateChange::Closed(w) if w.contains("does not answer")));
        assert_eq!(g.answers(), Some(false));
    }

    #[test]
    fn only_this_campaigns_scope_names_are_ours() {
        let cid = "host-fake-1.e8090c684f269eb5";
        let u = crate::launch::scope_unit_name(cid, "N1.i0058.L0-t0:a1");
        assert!(ours(&u, cid), "{u}");
        assert!(!ours(&u, "host-fake-1.0000000000000000"));
        for bad in [
            "rr-host-fake-1.e8090c684f269eb5-N1.i0058.L0-t0_a1",
            "rr-host-fake-1.e8090c684f269eb5-N1-e284ca118b12afdg",
            "hoststep-fake1-045548",
            "rr-other-N1-e284ca118b12afdd",
            "",
        ] {
            assert!(!ours(bad, cid), "{bad}");
        }
    }

    #[test]
    fn the_list_round_trips_and_keeps_a_name_once() {
        let d = std::env::temp_dir().join(format!("rr-leftover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        assert!(load_leftovers(&d).unwrap().is_empty());
        let l = |u: &str| Leftover {
            unit: u.into(),
            attempt: "j:a1".into(),
            why: "w".into(),
            utc: "t".into(),
        };
        assert_eq!(add_leftovers(&d, vec![l("a"), l("b"), l("a")]).unwrap(), 2);
        assert_eq!(add_leftovers(&d, vec![l("b")]).unwrap(), 0);
        assert_eq!(load_leftovers(&d).unwrap().len(), 2);
        store_leftovers(&d, &[]).unwrap();
        assert!(matches!(
            crate::fsx::presence("leftover", &d.join(LEFTOVER_FILE)),
            crate::fsx::Presence::Absent
        ));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Inside a private namespace no unit can be read: every entry of ours is kept; a foreign or unrecorded name is
    /// refused; nothing is ever signalled or stopped.
    #[test]
    fn clearing_refuses_foreign_and_unrecorded_names_and_keeps_unreadable_ones() {
        if !crate::launch::systemd_units_unavailable() {
            return;
        }
        let cid = "c.0123456789abcdef";
        let mine = crate::launch::scope_unit_name(cid, "j:a1");
        let unrec = crate::launch::scope_unit_name(cid, "j:a2");
        let d = std::env::temp_dir().join(format!("rr-leftover-c-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let e = |u: &str| Leftover {
            unit: u.into(),
            attempt: "j:a1".into(),
            why: "w".into(),
            utc: "t".into(),
        };
        add_leftovers(
            &d,
            vec![e(&mine), e(&unrec), e("rr-other-x-0123456789abcdef")],
        )
        .unwrap();
        let rec: BTreeSet<String> = [mine.clone()].into();
        let r = clear_leftovers(&d, cid, Some(&rec)).unwrap();
        assert_eq!(r.len(), 3);
        assert!(r[0].action.starts_with("kept: state unreadable"), "{r:?}");
        assert!(r[1].action.starts_with("refused: not recorded"), "{r:?}");
        assert!(
            r[2].action.starts_with("refused: not a scope name"),
            "{r:?}"
        );
        assert!(r.iter().all(|c| !c.done));
        assert_eq!(load_leftovers(&d).unwrap().len(), 3);
        let r = clear_leftovers(&d, cid, None).unwrap();
        assert!(r[0].action.starts_with("kept: the ledger"), "{r:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
