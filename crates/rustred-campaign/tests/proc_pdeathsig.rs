//! PDEATHSIG: a helper supervisor (this executable re-run with `RR_PROC_HELPER=minisup`) launches a child through the
//! real launcher and records its identity; the test SIGKILLs the helper supervisor; its child must die by itself.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

#[path = "proc_common/mod.rs"]
mod common;

use common::*;
use rustred_campaign::launch::{ChildHandle, ChildLauncher, KillReason};
use rustred_campaign::resources::{Liveness, ProcessIdentity};
use std::time::Duration;

#[test]
fn helper_entry() {
    if std::env::var("RR_PROC_HELPER").as_deref() != Ok("minisup") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("RR_PROC_OUT").unwrap());
    let mut l = launcher();
    let spec = sh(&out, "grandchild", "exec sleep 600");
    let h = l.launch(&spec).unwrap();
    let id = serde_json::to_string(h.identity()).unwrap();
    std::fs::write(out.join("identity.json.tmp"), id).unwrap();
    std::fs::rename(out.join("identity.json.tmp"), out.join("identity.json")).unwrap();
    std::thread::sleep(Duration::from_secs(600));
    drop(h);
}

#[test]
fn child_dies_when_its_supervisor_is_sigkilled() {
    let _g = serial();
    let dir = TempDir::new("pdeath");
    let mut l = launcher();
    let mut spec = helper_spec(dir.path(), "minisup", "helper_entry", "minisup");
    spec.env.push(("RR_PROC_OUT".into(), dir.path().into()));
    let mut sup = l.launch(&spec).unwrap();
    let sup_sid = sup.identity().pid;
    let idfile = dir.path().join("identity.json");
    wait_until("helper supervisor wrote its child's identity", LONG, || {
        idfile.exists()
    });
    let child: ProcessIdentity =
        serde_json::from_str(&std::fs::read_to_string(&idfile).unwrap()).unwrap();
    assert_eq!(child.check().unwrap(), Liveness::Alive { zombie: false });
    // The grandchild lives in its own session: the helper's group kill cannot reach it; only PDEATHSIG can.
    sup.kill_now(KillReason::Other("test kills the helper supervisor".into()));
    let r = drive(&mut sup);
    assert_eq!(r.exit.and_then(|e| e.signal()), Some(libc::SIGKILL));
    assert_session_gone(sup_sid);
    wait_until(
        "grandchild killed by PDEATHSIG",
        Duration::from_secs(120),
        || !matches!(child.check(), Ok(Liveness::Alive { zombie: false })),
    );
    assert_session_gone(child.pid);
}
