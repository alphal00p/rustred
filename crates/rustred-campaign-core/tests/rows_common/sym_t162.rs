//! T162 symmetry-group helpers shared by the rows_sym_* tests (lane I-ROWS rows2).
#![allow(dead_code)]

use std::sync::Arc;

use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, GroupCheck, SymGroup, VerifiedGroup, cross_check, derive_group,
    verify_cross_checked,
};
use serde_json::{Value, json};

use super::data;

pub fn t162_chart() -> Vec<bool> {
    (0..20).map(|i| i < 9).collect()
}

/// The checker's map file as a generic group: line_perm on the 9 lines, `forms` = D_j(Tk) on the D basis.
pub fn checker_group() -> SymGroup {
    let v: Value = serde_json::from_str(&data("sym/checker_symgroup_T162P.json")).unwrap();
    let mut els = Vec::new();
    for g in v.as_array().unwrap() {
        let mut action = Vec::new();
        for (j, f) in g["forms"].as_array().unwrap().iter().enumerate() {
            let terms: Vec<Value> = f[0]
                .as_object()
                .unwrap()
                .iter()
                .map(|(m, c)| json!([m.parse::<usize>().unwrap(), c]))
                .collect();
            if j < 9 {
                action.push(json!([g["line_perm"][j], false]));
            } else if terms.len() == 1
                && f[1] == "0"
                && terms[0][1] == "1"
                && terms[0][0].as_u64().unwrap() >= 9
            {
                action.push(json!([terms[0][0], false]));
            } else {
                action.push(json!({"terms": terms, "constant": f[1]}));
            }
        }
        els.push(json!({"matrix": null, "action": action}));
    }
    SymGroup::from_json(
        &json!({"arity": 20, "loops": 5, "chart": "11111111100000000000", "elements": els}),
    )
    .unwrap()
}

pub fn t162_group(fam: &Family) -> Arc<VerifiedGroup> {
    let (g, _) = derive_group(fam, &t162_chart(), &DeriveOptions::default()).unwrap();
    let ck = checker_group();
    assert!(matches!(cross_check(&g, &ck), GroupCheck::Equal { .. }));
    // the provenance label is computed by verify_cross_checked (it derives and compares itself)
    Arc::new(
        verify_cross_checked(
            fam,
            &ck,
            "checker h3/symgroup_T162P.json",
            &DeriveOptions::default(),
        )
        .unwrap(),
    )
}
