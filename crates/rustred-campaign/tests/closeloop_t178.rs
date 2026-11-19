//! Lane closeloop2: measurements of the closing pass on REAL material (ignored: big data, release). Spawns nothing (the
//! pass is in-process): no pid namespace needed. Paths are test data (env overrides), never code.
#![allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code

mod inputs;
use rustred_campaign::closeloop::config::Budgets;
use rustred_campaign::closeloop::pass::{PassIn, run_pass};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn env_path(k: &str, d: &str) -> PathBuf {
    std::env::var_os(k)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(d))
}

fn vm_hwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

/// The FULL statement material of T178 (import2's spec) + bound proofs + verified symmetry relations, tier from
/// `RR_T178_TIER` (default C): one pass, no solve; prints the counts the exact tier needs.
#[test]
#[ignore = "big data: T178 full statement material; release"]
fn t178_full_material_exact_tier_pass() {
    let scratch = env_path(
        "RR_CL2_SCRATCH",
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/closeloop2",
    );
    let spec_in = env_path(
        "RR_T178_SPEC",
        "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/import/t178/spec.T178.json",
    );
    let proofs = env_path(
        "RR_T178_PROOFS",
        "/common/build-cache/vhirschi/p_integral_dev/t178-close/proofs-bound-20261114T091622Z/proofs.jsonl",
    );
    let fam = env_path(
        "RR_T178_FAMILY",
        "/shared/vhirschi/t3code/vakint_dev/p_integral_dev/SCRATCHPAD/t178-close/data/T178.family.sym",
    );
    if !spec_in.is_file() || !proofs.is_file() {
        inputs::missing(format_args!("SKIP: material missing"));
        return;
    }
    let tier = std::env::var("RR_T178_TIER").unwrap_or_else(|_| "C".into());
    let mut v: Value = serde_json::from_slice(&std::fs::read(&spec_in).unwrap()).unwrap();
    v["policies"]["tier"] = json!(tier);
    v["evidence"] = json!({"proofs": [{"path": proofs.display().to_string()}]});
    for s in v["sources"].as_array_mut().unwrap() {
        if s["kind"] == "supplied_relations" {
            s["verify"] = json!({"family": {"path": fam.display().to_string()}});
        }
    }
    let out = scratch.join(format!("t178-full-{tier}"));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let text = serde_json::to_string_pretty(&v).unwrap();
    std::fs::write(out.join("spec.json"), &text).unwrap();
    let inp = PassIn {
        node: "T178".into(),
        round: 1,
        spec_text: text,
        base: PathBuf::from("/"),
        answers: vec![],
        threads: 4,
        required: None,
        issued: BTreeSet::new(),
        keys_issued: 0,
        jobs_issued: 0,
        budgets: Budgets::default(),
        max_keys_per_job: 16,
        hand: None,
        final_dir: out.join("final"),
        attention_items: 0,
        relation_rule: "m1_first".into(),
        exact_tier: tier == "C",
        rows_path: out.join("rows.jsonl"),
        ..PassIn::first("T178", String::new(), PathBuf::new(), PathBuf::new())
    };
    let t0 = std::time::Instant::now();
    let o = run_pass(&inp, None).unwrap();
    let c = &o.check;
    println!(
        "T178 full tier {tier}: {:.1} s, VmHWM {} MB; keys {} certified {} defect {}; tier {} closure {}; blocking {}; requests {}; counts {}; by class {}",
        t0.elapsed().as_secs_f64(),
        vm_hwm_kb() / 1024,
        c["keys"],
        c["certified"],
        c["defect"],
        c["tier"],
        c["closure"],
        c["blocking"],
        c["requests"],
        c["counts"],
        c["evidence_by_class"]
    );
}
