//! `rr_fake_child` emulating kind `symmetry-composite` (lane symjob; interfaces/symmetry-composite-job.md 6): the
//! tool `rr_rf_symflow run --request` WITHOUT Symbolica. It reads job.json, the request and its pinned inputs, loads
//! the import and derives the verified group with the campaign's own code (`adapters::symcomp::load_import`), and per
//! key composes over Q with the CORE (`QField(Independent)`) and a catalogue that serves nothing: it ships PLAIN
//! relations only (a catalogue replacement needs Q(d)); `over_cap` arises naturally from the policy's
//! `route0.max_terms`. It writes the tool's sealed layout (five files, SHA256SUMS, result.json last) and exits 0
//! WITHOUT OUTCOME (a legacy child: the supervisor-side wrapper checks, seals and writes OUTCOME).
//! Script ops: `prepare {ms}`; `fail {message}` (an ERROR line on stdout, exit 2); `proof_answer {status, key, ms}`
//! with status `slow` (`ms` per key), `forge_coefficient` (`key`'s shipped relation: first coefficient + 1, sealed
//! VALIDLY), `unsealed` (no result.json), `tamper_after_seal` (the table changed after SHA256SUMS / result.json),
//! `pin_mismatch` (result.json names another request digest). A signal ends the child like the tool (killed by it).

use super::SIGNALS;
use rustred_campaign::adapters::symcomp::load_import;
use rustred_campaign_contract::classify::symcomp as sc;
use rustred_campaign_contract::fake::FakeOp;
use rustred_campaign_contract::spec::JobSpec;
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::repairflow::composite::{
    Catalogue, ComposeOptions, KeyChoices, QField, Service, compose_all, import_lower,
    rows_by_element, shipped_record, verification_record,
};
use rustred_campaign_core::replay_base::algebra::CheckAlgebra;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

fn die(msg: &str) -> ! {
    println!("{}", json!({"verdict": "ERROR", "error": msg}));
    std::process::exit(2)
}

fn check_signal() {
    if SIGNALS.load(SeqCst) > 0 {
        let s = super::LAST_SIG.load(SeqCst);
        super::die_by_signal(if s > 0 { s } else { libc::SIGTERM });
    }
}

fn nap(ms: u64) {
    let mut left = ms;
    while left > 0 {
        check_signal();
        let d = left.min(20);
        std::thread::sleep(Duration::from_millis(d));
        left -= d;
    }
    check_signal();
}

fn sha(b: &[u8]) -> String {
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// The fake serves no catalogue rule (Q only): an image term that is not lower refuses the element.
struct NoCatalogue;

impl<'a> Catalogue<QField<'a, Independent>> for NoCatalogue {
    fn serve(
        &self,
        _: &QField<'a, Independent>,
        _: &[i32],
    ) -> Result<Service<rustred_campaign_core::replay_base::algebra::Q>, String> {
        Ok(Service::Unserved(
            "fake child: no catalogue (Q only)".into(),
        ))
    }
}

pub fn run(spec_path: &Path) -> ! {
    let spec: JobSpec = std::fs::read(spec_path)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
        .unwrap_or_else(|e| die(&format!("job.json: {e}")));
    let ops: Vec<FakeOp> = spec
        .fake
        .as_ref()
        .map(|f| f.script.clone())
        .unwrap_or_default();
    let mut slow_ms = 0;
    let mut forge: Option<Vec<i64>> = None;
    // wireflow (b): keys whose (plain) relation is shipped as a TWO-ROW form (one row of the element at the key, kind
    // two_row, not plain): the import then admits it only through a kernel certificate
    let mut two_row: Vec<Vec<i64>> = vec![];
    let (mut unsealed, mut tamper, mut pin_mismatch) = (false, false, false);
    for op in &ops {
        match op {
            FakeOp::Prepare { ms } => nap(*ms),
            FakeOp::Fail { message, .. } => die(message),
            FakeOp::ProofAnswer {
                status, key, ms, ..
            } => match status.as_deref() {
                Some("slow") => slow_ms = *ms,
                Some("forge_coefficient") => forge = key.clone(),
                Some("two_row") => two_row.extend(key.clone()),
                Some("unsealed") => unsealed = true,
                Some("tamper_after_seal") => tamper = true,
                Some("pin_mismatch") => pin_mismatch = true,
                s => die(&format!(
                    "fake symmetry-composite: unknown proof_answer status {s:?}"
                )),
            },
            _ => {}
        }
    }
    let input = |role: &str| -> Option<(String, Vec<u8>)> {
        spec.inputs.iter().find(|i| i.role == role).map(|i| {
            let p = spec.input_path(i);
            let b = std::fs::read(&p).unwrap_or_else(|e| die(&format!("{role}: {e}")));
            (p.display().to_string(), b)
        })
    };
    let (_, req_b) = input("request").unwrap_or_else(|| die("no request input"));
    let req: Value =
        serde_json::from_slice(&req_b).unwrap_or_else(|e| die(&format!("request: {e}")));
    if req["schema"] != sc::REQUEST_SCHEMA {
        die("request schema");
    }
    // the tool verifies every stated sha256 before use
    for role in sc::PINNED_ROLES {
        if req[role].is_null() {
            continue;
        }
        let p = req[role]["path"].as_str().unwrap_or_default();
        let b = std::fs::read(p).unwrap_or_else(|e| die(&format!("{role} {p}: {e}")));
        if req[role]["sha256"]
            .as_str()
            .map(|s| s.trim_start_matches("sha256:"))
            != Some(sha(&b).as_str())
        {
            die(&format!("{role} {p}: sha256 differs from the request's"));
        }
    }
    let out = PathBuf::from(&spec.run_dir).join("out").join(sc::CHILD_DIR);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| die(&e.to_string()));
    let _ = std::fs::remove_file(out.join(sc::RESULT_FILE));
    let ctx = load_import(&spec).unwrap_or_else(|e| die(&e));
    let keys: Vec<Vec<i32>> = match input("keys") {
        Some((_, b)) => String::from_utf8_lossy(&b)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| die(&format!("keys: {e}"))))
            .collect(),
        None => die("the fake child needs a keys input"),
    };
    let keys: Vec<Vec<i32>> = match req["limit"].as_u64() {
        Some(n) => keys.into_iter().take(n as usize).collect(),
        None => keys,
    };
    let qf = QField(&Independent);
    let cmp = |a: &[i32], b: &[i32]| ctx.set.cmp.cmp(a, b);
    let lower = |k: &[i32], t: &[i32]| import_lower(&ctx.lines, k, t, &cmp);
    let opts = ComposeOptions {
        max_depth: ctx.policy.composite.max_depth,
        max_terms: ctx.policy.composite.max_terms,
        max_replacements: ctx.policy.composite.max_replacements,
    };
    let chart = ctx.set.frame.mask_string();
    // the frame header's family (as the real tool writes it: the import refuses a supplied table of another family)
    let family = rustred_campaign_core::repairflow::symflow::frame_table_path(&ctx.set)
        .and_then(|p| {
            let p = if Path::new(&p).is_absolute() {
                PathBuf::from(&p)
            } else {
                PathBuf::from(req["import_base"].as_str().unwrap_or("/")).join(&p)
            };
            rustred_campaign_core::import::lines::read_header("frame", &p).ok()
        })
        .map(|h| h.raw["family"].clone())
        .unwrap_or(json!("fake"));
    let mut table = vec![
        json!({"type": "header", "schema": "table_dump.v1", "family": family, "producer": "rr_fake_child",
        "kind": "symmetry-composite"})
        .to_string(),
    ];
    let mut ver = vec![];
    let mut kl = String::new();
    let mut cl = String::new();
    for k in &keys {
        nap(slow_ms);
        let line = match rows_by_element(&Independent, &ctx.group, k, ctx.policy.route0.max_terms) {
            Err(e) => {
                let ch = KeyChoices::failed(k, &ctx.lines, &e);
                cl.push_str(&serde_json::to_string(&ch).expect("json"));
                cl.push('\n');
                json!({"key": k, "face": ch.face, "status": ch.status})
            }
            Ok(by) => {
                let kc = compose_all(&Independent, &qf, &NoCatalogue, &by, k, &lower, &cmp, &opts);
                let ch = KeyChoices::of_composites(k, &kc, &ctx.lines);
                cl.push_str(&serde_json::to_string(&ch).expect("json"));
                cl.push('\n');
                if let Some(b) = kc.best() {
                    let idx = table.len() - 1;
                    let mut rec = shipped_record(&qf, k, b, &chart, idx);
                    if forge
                        .as_ref()
                        .is_some_and(|f| f.iter().map(|&x| x as i32).eq(k.iter().copied()))
                    {
                        // one coefficient changed (q -> q + 1), then sealed VALIDLY below
                        let t = rec["rhs"][0][1].as_str().unwrap_or("0").to_string();
                        let q = Independent.q_parse(&t).unwrap_or_else(|e| die(&e));
                        let q1 = Independent
                            .q_add(q, rustred_campaign_core::replay_base::algebra::Q::int(1))
                            .unwrap_or_else(|e| die(&e));
                        rec["rhs"][0][1] = json!(if q1.d == 1 {
                            q1.n.to_string()
                        } else {
                            format!("{}/{}", q1.n, q1.d)
                        });
                    }
                    let tr = two_row
                        .iter()
                        .any(|f| f.iter().map(|&x| x as i32).eq(k.iter().copied()));
                    let mut vline =
                        verification_record(&qf, k, b, &ctx.group_digest, "rr_fake_child/core-q");
                    if tr {
                        rec["kind"] = json!("symmetry-two_row");
                        vline["plain"] = json!(false);
                        vline["kind"] = json!("two_row");
                        vline["combination"] = json!({"rows": [{"element": b.element, "seed": k}]});
                    }
                    table.push(rec.to_string());
                    ver.push(vline.to_string());
                }
                let tr = two_row
                    .iter()
                    .any(|f| f.iter().map(|&x| x as i32).eq(k.iter().copied()));
                json!({"key": k, "face": ch.face, "status": "ok", "best": kc.best().map(|b| json!({
                    "element": b.element, "terms": b.rhs.len(), "plain": b.is_plain() && !tr,
                    "replacements": b.replacements.len()})), "alternatives": kc.accepted().count()})
            }
        };
        kl.push_str(&line.to_string());
        kl.push('\n');
    }
    let join = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("choices.jsonl", cl.into_bytes()),
        ("keys.jsonl", kl.into_bytes()),
        ("composite.table.jsonl", join(&table).into_bytes()),
        ("composite.verification.jsonl", join(&ver).into_bytes()),
        ("report.json", (json!({"schema": "rustred.campaign.repairflow-symflow.v1", "producer": "rr_fake_child",
            "keys": keys.len()}).to_string() + "\n").into_bytes()),
    ];
    let mut sums = String::new();
    let mut outs = vec![];
    for (n, b) in &files {
        let part = out.join(format!("{n}.part"));
        std::fs::write(&part, b).unwrap_or_else(|e| die(&e.to_string()));
        std::fs::rename(&part, out.join(n)).unwrap_or_else(|e| die(&e.to_string()));
        sums.push_str(&format!("{}  {n}\n", sha(b)));
        outs.push(json!({"file": n, "sha256": sha(b), "lines": b.iter().filter(|&&c| c == b'\n').count()}));
    }
    std::fs::write(out.join(sc::SUMS_FILE), sums).unwrap_or_else(|e| die(&e.to_string()));
    check_signal();
    if unsealed {
        std::process::exit(0);
    }
    let exe = std::fs::read("/proc/self/exe")
        .map(|b| sha(&b))
        .unwrap_or_default();
    let inputs: Vec<Value> = sc::PINNED_ROLES
        .iter()
        .filter(|r| !req[**r].is_null())
        .map(|r| json!({"role": r, "path": req[*r]["path"], "sha256": req[*r]["sha256"]}))
        .collect();
    let request_sha = if pin_mismatch {
        "00".repeat(32)
    } else {
        sha(&req_b)
    };
    let shipped = ver.len();
    let result = json!({"schema": sc::RESULT_SCHEMA, "verdict": "DONE", "request_sha256": request_sha,
        "tool": {"name": "rr_fake_child", "sha256": exe}, "inputs": inputs,
        "group": {"digest": ctx.group_digest, "elements": 0}, "policy_digest": ctx.policy.digest(),
        "spec_digest": null, "keys": keys.len(), "outputs": outs, "totals": {}, "gate": "pass",
        "shipped_relations": shipped, "independently_verified": shipped});
    let part = out.join("result.json.part");
    std::fs::write(
        &part,
        serde_json::to_string_pretty(&result).expect("json") + "\n",
    )
    .unwrap_or_else(|e| die(&e.to_string()));
    std::fs::rename(&part, out.join(sc::RESULT_FILE)).unwrap_or_else(|e| die(&e.to_string()));
    if tamper {
        let p = out.join(sc::TABLE_FILE);
        let mut b = std::fs::read(&p).unwrap_or_default();
        b.extend_from_slice(b"{\"type\":\"rule\",\"index\":999}\n");
        std::fs::write(&p, b).unwrap_or_else(|e| die(&e.to_string()));
    }
    println!("{}", json!({"keys": keys.len(), "shipped": shipped}));
    std::process::exit(0)
}
