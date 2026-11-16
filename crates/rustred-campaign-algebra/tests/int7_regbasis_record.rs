//! int7 (integration round 2, wire-in after merge 6): lane epsdepth reads lane regbasis' record
//! (`rustred.campaign.regular-basis.v1`) only after verifying its BLAKE3 digest (core `regbasis::record::verify`):
//! a sealed record gives the map; an ALTERED record (one entry of `M` changed after sealing), an unsealed one and one of
//! another schema are refused. Needs the Symbolica licence (`source WS/ops/env.sh`); SKIPS without it.

use rustred_campaign_algebra::epsdepth::{BasisMap, SymbolicaExact, SymbolicaLaurent};
use rustred_campaign_algebra::{SymbolicaAlgebra, licensed};
use rustred_campaign_core::regbasis::record::{RECORD_SCHEMA, seal};
use serde_json::json;

#[test]
fn an_altered_regular_basis_record_is_refused_and_a_sealed_one_is_read() {
    if !licensed() {
        eprintln!("SKIP: no Symbolica licence (source WS/ops/env.sh)");
        return;
    }
    let alg = SymbolicaAlgebra::new().unwrap();
    let lau = SymbolicaLaurent::new("4").unwrap();
    let ex = SymbolicaExact::new().unwrap();
    // a toy 2 x 2 map T = M R (test data: keys and entries are arbitrary)
    let rec = json!({
        "schema": RECORD_SCHEMA,
        "binding": {"node": {"id": "toy", "identity_digest": "x"}, "box": {"id": "b", "digest": "y"},
                    "closure_statement_digest": "z", "d0": "4"},
        "terminals": [[1, 1, 0], [2, 1, 0]],
        "regular_set": [[1, 2, 0], [2, 2, 0]],
        "M": [["1", "(d-4)/(d-3)"], ["0", "1"]],
    });
    let (sealed, _) = seal(rec.clone()).unwrap();
    let (map, r, t) = BasisMap::from_regbasis_record(&alg, &lau, &ex, &sealed).unwrap();
    assert_eq!(map.m.len(), 2);
    assert!(map.m[1][0].is_none(), "an identically zero entry");
    assert_eq!(r, vec![vec![1, 2, 0], vec![2, 2, 0]]);
    assert_eq!(t, vec![vec![1, 1, 0], vec![2, 1, 0]]);

    // ALTERED after sealing: one entry of M
    let mut altered = sealed.clone();
    altered["M"][0][1] = json!("1/(d-3)");
    let e = BasisMap::from_regbasis_record(&alg, &lau, &ex, &altered)
        .err()
        .expect("an altered record is refused");
    assert!(e.contains("digest mismatch"), "{e}");
    // altered R
    let mut altered_r = sealed.clone();
    altered_r["regular_set"][0] = json!([1, 3, 0]);
    assert!(BasisMap::from_regbasis_record(&alg, &lau, &ex, &altered_r).is_err());
    // never sealed
    let e = BasisMap::from_regbasis_record(&alg, &lau, &ex, &rec)
        .err()
        .expect("an unsealed record is refused");
    assert!(e.contains("no digest"), "{e}");
    // another schema (sealed as a tower record)
    let mut other = rec.clone();
    other["schema"] = json!(rustred_campaign_core::regbasis::record::TOWER_SCHEMA);
    let (other, _) = seal(other).unwrap();
    assert!(BasisMap::from_regbasis_record(&alg, &lau, &ex, &other).is_err());
}
