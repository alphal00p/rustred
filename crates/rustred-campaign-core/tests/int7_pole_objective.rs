//! int7 (integration round 2, wire-in B1): every choice among candidate relations goes through the ONE function
//! `poles::pole_objective` (default objective [eps_box_excess, max_pole_order, sum_pole_orders, singular_terms, size],
//! max_size_ratio "unbounded").
//! (a) a call site without any pole profile ranks by size (then id) and the choice records "no profile";
//! (b) repairflow's `prefer_regular` decides through `poles::select` in EXACT integer arithmetic (it compared floats):
//!     a regular candidate one term over ratio 1 x 2^53 is not kept (the float comparison rounded it to equal), a text
//!     rational "5/2" is read by the one parser, a JSON float by its exact binary value.

use rustred_campaign_core::poles::{Candidate, ChoiceReason, PoleObjective, pole_objective};
use rustred_campaign_core::repairflow::policy::{SizeRatio, prefer_regular};

fn cand(id: &str, size: u64, pole: Option<u64>) -> Candidate {
    Candidate {
        id: id.into(),
        admissible: true,
        excluded_by: None,
        evidence_level: 0,
        size,
        pole_order: pole,
    }
}

#[test]
fn without_any_pole_profile_the_size_decides_and_the_choice_says_no_profile() {
    let cs = [
        cand("b", 30, None),
        cand("a", 30, None),
        cand("c", 10, None),
    ];
    let c = pole_objective(&cs, &[], &PoleObjective::default(), None)
        .unwrap()
        .unwrap();
    assert_eq!(c.id, "c");
    assert_eq!(c.profile_used, "none");
    assert_eq!(c.reason, ChoiceReason::Economy);
    assert!(
        c.report_line().contains("no profile"),
        "{}",
        c.report_line()
    );
    assert_eq!(c.to_json()["profile_used"], "none");
    // equal sizes: the smaller id (order-independent)
    let c = pole_objective(&cs[..2], &[], &PoleObjective::default(), None)
        .unwrap()
        .unwrap();
    assert_eq!(c.id, "a");
    // one candidate (the termmin / closeloop accept call sites today)
    let c = pole_objective(&cs[2..], &[], &PoleObjective::default(), None)
        .unwrap()
        .unwrap();
    assert_eq!((c.reason, c.profile_used), (ChoiceReason::Only, "none"));
    // as soon as one candidate carries a pole order the profile is local (behaviour of lane epsdepth unchanged)
    let c = pole_objective(
        &[cand("big-regular", 300, Some(0)), cand("small", 10, None)],
        &[],
        &PoleObjective::default(),
        None,
    )
    .unwrap()
    .unwrap();
    assert_eq!((c.id.as_str(), c.profile_used), ("big-regular", "local"));
}

#[test]
fn prefer_regular_is_exact_and_reads_the_one_parser() {
    let r1 = SizeRatio::Ratio(1.0);
    let big = 1u64 << 53;
    // f64: (2^53 + 1) as f64 == 2^53, so the float comparison kept the larger regular candidate; exact: it is over 1x
    assert!(!prefer_regular(big + 1, big, &r1).unwrap().0);
    assert!(prefer_regular(big, big, &r1).unwrap().0);
    // a text rational, the word, a float by its exact value (1.5 = 3/2)
    let r52 = SizeRatio::Word("5/2".into());
    assert!(prefer_regular(25, 10, &r52).unwrap().0);
    assert!(!prefer_regular(26, 10, &r52).unwrap().0);
    let line = prefer_regular(26, 10, &r52).unwrap().1;
    assert!(
        line.contains("max_size_ratio 5/2") && line.contains("over max_size_ratio"),
        "{line}"
    );
    assert!(
        prefer_regular(u64::MAX, 1, &SizeRatio::Word("unbounded".into()))
            .unwrap()
            .0
    );
    assert!(prefer_regular(15, 10, &SizeRatio::Ratio(1.5)).unwrap().0);
    assert!(!prefer_regular(16, 10, &SizeRatio::Ratio(1.5)).unwrap().0);
    // refused: below 1, not a rational, not finite
    assert!(prefer_regular(1, 1, &SizeRatio::Ratio(0.5)).is_err());
    assert!(prefer_regular(1, 1, &SizeRatio::Word("big".into())).is_err());
    assert!(prefer_regular(1, 1, &SizeRatio::Ratio(f64::INFINITY)).is_err());
}
