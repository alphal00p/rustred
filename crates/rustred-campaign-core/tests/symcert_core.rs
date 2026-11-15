//! Lane symcert (interfaces/proof-symrows-job.md 7): the readers of the three producers' files on the real records
//! (counts only; comparison data, skipped when absent). The import-side check of a sealed certificate is lane
//! symfix's (import::supplied_verify route (b)). Test data: the lanes' scratch records (read-only).

use rustred_campaign_core::symcert::producers;
use std::path::PathBuf;

fn scratch() -> PathBuf {
    PathBuf::from("/common/build-cache/vhirschi/p_integral_dev/implementer-scratch")
}

#[test]
fn readers_on_the_producers_records() {
    let s = scratch();
    let f8 = s.join("tworow/t178f8/f8b.jsonl");
    if let Ok(t) = std::fs::read_to_string(&f8) {
        let e = producers::tworow(&t).unwrap();
        assert_eq!(e.len(), 79, "tworow F8 served keys");
        let terms: usize = e.iter().map(|x| x.rhs.len()).sum();
        println!(
            "tworow F8: {} keys, {terms} rhs terms (best per key)",
            e.len()
        );
        assert_eq!(terms, 37_228, "the lane's total of the best relations");
    } else {
        println!("SKIP tworow: {} missing", f8.display());
    }
    let r1 = s.join("repairflow/runs/t162v3-r1b");
    if let (Ok(t), Ok(v)) = (
        std::fs::read_to_string(r1.join("composite.table.jsonl")),
        std::fs::read_to_string(r1.join("composite.verification.jsonl")),
    ) {
        let c = producers::composites(&t, &v).unwrap();
        assert_eq!(c.len(), 6_297);
        assert_eq!(c.iter().filter(|x| !x.plain).count(), 117);
    } else {
        println!("SKIP composites T162");
    }
    let acc = s.join("symjob/real/accepted");
    if let (Ok(t), Ok(v)) = (
        std::fs::read_to_string(acc.join("symcomp.f424a5b69c733d41.table.jsonl")),
        std::fs::read_to_string(acc.join("symcomp.b0d774069205d52a.verification.jsonl")),
    ) {
        let c = producers::composites(&t, &v).unwrap();
        assert_eq!(c.iter().filter(|x| !x.plain).count(), 44);
    } else {
        println!("SKIP composites T178");
    }
}
