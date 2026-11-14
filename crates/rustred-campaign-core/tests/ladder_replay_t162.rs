//! REPLAY of the helper's T162 hard numerator-free faces (h2/STAGES.txt X4b, X4c, line 59: "48 cases, 0 point keys,
//! 0 extra terminals") through the ladder with a scripted engine answering from the recorded attempts
//! (tests/data/ladder/t162_h2_*.jsonl; unrecorded settings answer "no relation", which can only ADD cases).

mod ladder_common;

use std::collections::BTreeMap;

use ladder_common::*;
use rustred_campaign_core::worklist::items::{Face, NodeParams};

#[test]
fn t162_44_hard_faces_close_in_at_most_48_cases() {
    let items: Vec<RecItem> = read_jsonl("t162_h2_items.jsonl");
    let recs: Vec<Rec> = read_jsonl("t162_h2_attempts.jsonl");
    assert_eq!(items.len(), 44);
    let params = NodeParams::new("T162", 20, 9);
    let cmp = t162_comparator();
    let mut solver = Scripted::new(params.clone(), recs, BTreeMap::new());
    let mut facts = ScriptedFacts {
        cmp: &cmp,
        n_lines: 9,
        line_priority: (0..9).collect(),
    };
    let mut total = 0;
    let mut per_face: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
    for it in &items {
        let face = Face::new(it.sd.clone().unwrap(), vec![]);
        let (_rep, o) = run_item(&params, &mut solver, &cmp, &mut facts, face.clone(), it.census.clone());
        assert!(o.terminals.is_empty(), "face {:?}: terminals {:?} ({:?})", face, o.terminals, o.rules);
        assert_eq!(o.point_rules, 0, "face {:?}: point rules {:?}", face, o.rules);
        assert!(o.attention.is_empty(), "{:?}", o.attention);
        total += o.cases;
        per_face.insert(face.sd.clone(), o.cases);
        if face.sd == vec![1, 3, 4] {
            // two generic centres + the diagonal child (equations READ from the two rules' exception sets) closed by
            // the union of the three line centres (h2/STAGES.txt:53; mc.jsonl 134diag_mc_floor)
            assert_eq!(o.cases, 3, "{:?}", o.rules);
            assert!(
                o.rules.iter().any(|(case, set, _)| case.contains("eq=n1-n4,n3-n4") && set.starts_with("union_lines")),
                "{:?}",
                o.rules
            );
        }
    }
    let ones = per_face.values().filter(|&&c| c == 1).count();
    eprintln!("T162 replay: {total} cases, one-case faces {ones}, per face {per_face:?}");
    assert!(total <= 48, "{total} cases > 48");
    assert_eq!(ones, 41);
    assert_eq!(per_face[&vec![0, 2, 4]], 2);
    assert_eq!(per_face[&vec![2, 3, 6]], 2);
}
