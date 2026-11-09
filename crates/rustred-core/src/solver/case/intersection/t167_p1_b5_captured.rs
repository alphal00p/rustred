//! Exact captured T167 p1 refusal of the B5 case-lift binary (S1/S2 on), not a solver hint or
//! a closure claim. Mechanically converted (no rewriting of any polynomial) by
//! SCRATCHPAD/rr-defer/scripts/gen_p1_fixture.py from the untruncated DISPLAY section of
//! SCRATCHPAD/rr-case-lift/runs/p1/T167p1.table.jsonl.chart_error.11111111110000000000.txt
//! (sha256 c7ab392cc0cfeccb1142c84670b92a6d2bdaf10a042da4f8eabae69dab889dcc): table_dump.caselift-4925278b,
//! rules_study/ko4/T167/ko4-hard-T167/families/T167p1.sym (legacy order), chart 11111111110000000000,
//! refused at 526 s with UnsupportedGeometry.
//! Coefficient coordinates are [d, n0, ..., n19]; index variables 1..=20.

use super::t167_captured_inputs::CapturedT167;

/// Expected under S4 Defer: exactly this unresolved branch as the deferred region.
pub(super) const T167_P1_B5: CapturedT167 = CapturedT167 {
    order: "p1-b5",
    sector: "11111111110000000000",
    original_fixed: [
        None,
        Some(1),
        None,
        None,
        None,
        Some(1),
        None,
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
    ],
    original_equalities: &[],
    original_conjunction: &[
        "5-5*n9+n9^2-n9^3+n6-n6*n9+n6*n9^2-n6*n9^3-5*n6^2+5*n6^2*n9-n6^3+n6^3*n9-5*n4+5*n4*n9-n4*n9^2+n4*n9^3+4*n4*n6-4*n4*n6*n9+n4*n6^2-n4*n6^2*n9-n4^2+n4^2*n9-n4^2*n6+n4^2*n6*n9+n4^3-n4^3*n9+n3-n3*n9-n3*n6^2+n3*n6^2*n9-n3*n4+n3*n4*n9+n3*n4*n6-n3*n4*n6*n9+n2-n2*n9^2+n2*n6*n9-n2*n6*n9^2-n2*n6^2+n2*n6^2*n9-2*n2*n4+n2*n4*n9+n2*n4*n9^2+n2*n4^2-n2*n4^2*n9+7*n0-3*n0*n9-n0*n9^2-3*n0*n6+4*n0*n6*n9-n0*n6*n9^2-4*n0*n6^2+n0*n6^2*n9-5*n0*n4+n0*n4*n9+n0*n4*n9^2+3*n0*n4*n6+n0*n4^2-n0*n4^2*n9+2*n0*n3-n0*n3*n9-n0*n3*n6+n0*n3*n6*n9-n0*n3*n6^2-n0*n3*n4+n0*n3*n4*n6+2*n0*n2-n0*n2*n9-n0*n2*n6+n0*n2*n6*n9-n0*n2*n6^2-n0*n2*n4+n0*n2*n4*n6+2*n0^2-n0^2*n9-n0^2*n6+n0^2*n6*n9-n0^2*n6^2-n0^2*n4+n0^2*n4*n6",
        "2-n9-n9^2+n6*n9-n6*n9^2-2*n6^2+2*n6^2*n9-3*n4+2*n4*n9+n4*n9^2+n4*n6-n4*n6*n9+n4^2-n4^2*n9+2*n0-n0*n9-n0*n6+n0*n6*n9-n0*n6^2-n0*n4+n0*n4*n6",
    ],
    unresolved_fixed: [
        None,
        Some(1),
        None,
        None,
        None,
        Some(1),
        None,
        Some(1),
        Some(1),
        None,
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
        Some(0),
    ],
    unresolved_equalities: &[],
    unresolved_conjunction: &[
        "-1-3*n9+n9^2+2*n6-n6^2+3*n4-n4^2-n3-n3*n9+n3*n6+n3*n4-n2+n2*n6-n0+n0*n6",
        "n9-3*n9^2+3*n9^3-n9^4+3*n6+7*n6*n9-16*n6*n9^2+3*n6*n9^3-5*n6^2+8*n6^2*n9+2*n6^2*n9^2+n6^3-4*n6^3*n9+n6^4-3*n4-n4*n9+4*n4*n9^2-7*n4*n6+10*n4*n6*n9-2*n4*n6^2-n4*n6^2*n9+6*n4^2-5*n4^2*n9-n4^2*n9^2+2*n4^2*n6-3*n4^2*n6*n9+n4^2*n6^2-2*n4^3+2*n4^3*n9-2*n3-5*n3*n9+8*n3*n9^2-n3*n9^3+6*n3*n6-7*n3*n6*n9-4*n3*n6*n9^2-2*n3*n6^2+7*n3*n6^2*n9-2*n3*n6^3+5*n3*n4-5*n3*n4*n9+n3*n4*n6+3*n3*n4*n6*n9-n3*n4*n6^2-n3*n4^2+n3*n4^2*n9-n3*n4^2*n6-2*n3^2+2*n3^2*n9^2+n3^2*n6-3*n3^2*n6*n9+n3^2*n6^2+2*n3^2*n4-2*n3^2*n4*n9+n3^2*n4*n6-2*n2+2*n2*n9^2+3*n2*n6-4*n2*n6*n9+2*n2*n6^2*n9-n2*n6^3+2*n2*n4-n2*n4*n9-n2*n4*n9^2-n2*n4^2+n2*n4^2*n9-2*n2*n3+2*n2*n3*n9+n2*n3*n6-2*n2*n3*n6*n9+n2*n3*n6^2+7*n0*n9-4*n0*n9^2+n0*n9^3-7*n0*n4-n0*n4*n9+5*n0*n4^2-n0*n4^3+3*n0*n3*n9-n0*n3*n9^2-3*n0*n3*n4+n0*n3*n4^2",
        "6*n9-4*n9^2+n9^3-3*n6+4*n6*n9+2*n6^2-3*n6^2*n9+n6^3-4*n4-2*n4*n9-2*n4*n6+n4*n6*n9-n4*n6^2+4*n4^2+n4^2*n6-n4^3+2*n3+n3*n9-n3*n9^2-n3*n6+2*n3*n6*n9-n3*n6^2-3*n3*n4+n3*n4^2+2*n2-n2*n9-n2*n6+n2*n6*n9-n2*n6^2-n2*n4+n2*n4*n6",
    ],
};
