//! Lane pieces: KeyChoices records (lane repairflow2's per-key record of all acceptable choices) as planner input.

use std::collections::BTreeSet;

use rustred_campaign_core::order::NodeComparator;
use rustred_campaign_core::pieces::keychoices::{
    KeyChoicesOptions, KeyChoicesRecord, plan_input_from_key_choices,
};
use rustred_campaign_core::pieces::plan::HandDownTarget;
use rustred_campaign_core::pieces::{ServiceKind, plan};

fn rec(line: &str) -> KeyChoicesRecord {
    serde_json::from_str(line).unwrap()
}

#[test]
fn key_choices_become_candidates_and_one_term_images_onto_open_keys_are_hand_downs() {
    let c = NodeComparator::legacy(4, None).unwrap();
    let recs = vec![
        // a plain image onto the OPEN key [2,1,0,0] (another face): a hand-down; a composite alternative
        rec(
            r#"{"schema":"rustred.campaign.repairflow-keychoices.v1","key":[1,3,0,0],"face":"B","face_slots":[1],
            "choices":[{"id":"sym:e1","kind":"plain","element":1,"form":"e1","rhs":[[2,1,0,0]],"records":[],
                        "key_solved":false,"regular_at_4":null},
                       {"id":"sym:e2","kind":"composite","element":2,"form":"e2+T#3","rhs":[[1,1,0,0],[0,1,0,0]],
                        "records":["ab"],"key_solved":false,"regular_at_4":false}],
            "refused":[[0,"not_lower"]],"preferred":0,"extra_field":1}"#,
        ),
        // the open key [2,1,0,0] with a core recipe choice
        rec(
            r#"{"key":[2,1,0,0],"face":"A","choices":[{"id":"core:r","kind":"core","rhs":[[1,1,0,0]]}]}"#,
        ),
    ];
    let open: BTreeSet<Vec<i32>> = [vec![1, 3, 0, 0], vec![2, 1, 0, 0], vec![3, 1, 0, 0]]
        .into_iter()
        .collect();
    let inp = plan_input_from_key_choices(
        4,
        &recs,
        &open,
        &|_k: &[i32]| "Z".to_string(),
        &KeyChoicesOptions::default(),
    )
    .unwrap();
    assert_eq!(inp.keys.len(), 3);
    let k = inp.keys.iter().find(|k| k.key == vec![1, 3, 0, 0]).unwrap();
    assert_eq!(k.candidates[0].kind, ServiceKind::HandDown);
    assert_eq!(k.candidates[1].kind, ServiceKind::Composite);
    assert_eq!(k.candidates[1].provenance, "sym:e2 [e2+T#3]");
    assert_eq!(k.candidates[1].regular_at_4, Some(false));
    let z = inp.keys.iter().find(|k| k.key == vec![3, 1, 0, 0]).unwrap();
    assert_eq!((z.face.as_str(), z.candidates.len()), ("Z", 0));
    let p = plan(&c, &inp).unwrap();
    // the composite is singular at d = 4: the regular hand-down wins although composites rank first by kind
    let ch = p.chosen.iter().find(|x| x.key == vec![1, 3, 0, 0]).unwrap();
    assert_eq!(ch.kind, Some(ServiceKind::HandDown));
    assert_eq!(ch.hand_down_target, Some(HandDownTarget::OtherFace));
    assert_eq!(p.core_open, vec![vec![3, 1, 0, 0]]);
    // without the regularity preference the composite is chosen
    let mut inp2 = inp.clone();
    inp2.ranking.singular_last = false;
    let p2 = plan(&c, &inp2).unwrap();
    let ch = p2
        .chosen
        .iter()
        .find(|x| x.key == vec![1, 3, 0, 0])
        .unwrap();
    assert_eq!(ch.kind, Some(ServiceKind::Composite));
}

#[test]
fn unknown_kind_words_and_schemas_are_errors() {
    let r = rec(r#"{"key":[2,1,0,0],"face":"A","choices":[{"id":"x","kind":"mystery","rhs":[]}]}"#);
    let e = plan_input_from_key_choices(
        4,
        &[r],
        &BTreeSet::new(),
        &|_k: &[i32]| String::new(),
        &KeyChoicesOptions::default(),
    );
    assert!(e.unwrap_err().contains("unknown kind"));
    let dir = std::env::var_os("TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let f = dir.join(format!("pieces-kc-{}.jsonl", std::process::id()));
    std::fs::write(&f, "{\"schema\":\"other.v9\",\"key\":[1],\"face\":\"A\"}\n").unwrap();
    let e = rustred_campaign_core::pieces::keychoices::read_key_choices(&f);
    std::fs::remove_file(&f).ok();
    assert!(e.unwrap_err().contains("schema"));
}
