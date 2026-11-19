//! Lane gencheck round 6 J4 (emission contract section 17): the CONE-PRICING acceptance. PERMANENT NEGATIVE TEST: the review's T269
//! glued triangle (lane t162opt's A/B, implementer-scratch/t162opt/feas/ab2: the first dot key p85:2,1^13 alone vs its seven glued
//! inputs, diag4186 owner statistics, 1 thread) wins 327 -> 12 keys at the twelve-line Route entrance (phase 4), is equal within
//! 4 % two levels down (phase 7, the eleven-line Apply: 905,807 vs 922,379 children) and is REFUSED. Env RUSTRED_T269_AB2 = that
//! directory (read only; SKIPS without it).
use rustred_campaign_core::emission::cone::{Rules, parse_arm, price};
use serde_json::json;

#[test]
fn t269_glued_triangle_entrance_only_win_is_refused() {
    let Ok(d) = std::env::var("RUSTRED_T269_AB2") else {
        eprintln!("SKIP: RUSTRED_T269_AB2 not set");
        return;
    };
    let rd = |f: &str| std::fs::read_to_string(format!("{d}/{f}")).unwrap();
    let old = parse_arm(&rd("old.owner_stats.json"), None).unwrap();
    let glued = parse_arm(&rd("glued.owner_stats.json"), Some(old.request_dr)).unwrap();
    let v = price(&old, &glued, 4, &Rules::default(), json!({"case": "T269 glued triangle"}), None, vec![]).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&v).unwrap());
    assert_eq!(v.levels[0]["steps"], json!([327, 12]), "the entrance: twelve-line Route keys");
    assert_eq!(v.levels[1]["children"], json!([23780, 23886]), "the twelve-line Apply");
    assert_eq!(v.levels[3]["children"], json!([905807, 922379]), "the second Apply (eleven lines)");
    assert!(!v.accepted, "an entrance-only win is refused");
    // the shell histogram at the second Apply: its keys are the phase's keys stepped, per arm
    let rows = v.second_apply_shells["rows"].as_array().unwrap();
    let sum = |i: usize| rows.iter().map(|r| r[if i == 0 { "old" } else { "new" }].as_u64().unwrap()).sum::<u64>();
    assert_eq!((sum(0), sum(1)), (27819, 28859), "{:#}", v.second_apply_shells);
    assert!(!v.second_apply_owners.is_empty());
    let r = v.reason.unwrap();
    assert!(r.starts_with("cone_worse"), "{r}");
    // equal within 4 % at the second Apply after a 27x entrance win: absorbed as well
    let (e, s) = (v.entrance_ratio_old_over_new.unwrap(), v.second_apply_ratio_old_over_new.unwrap());
    assert!(e > 27.0 && (s - 1.0).abs() < 0.04, "{e} {s}");
    // the same through the CLI (pure mode), exit status refused
    let out = std::env::temp_dir().join(format!("cone-t269-{}.json", std::process::id()));
    let args: Vec<String> = ["--old-stats", &format!("{d}/old.owner_stats.json"), "--new-stats", &format!("{d}/glued.owner_stats.json"),
                             "--entrance", "4", "--out", &out.display().to_string()].iter().map(|s| s.to_string()).collect();
    let (text, ok) = rustred_campaign::emission::coneprice::cmd(&args).unwrap();
    assert!(!ok, "{text}");
    let _ = std::fs::remove_file(out);
}
