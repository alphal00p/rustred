//! Independent re-audit (lane audit-contract2, 2026-11-14): domain separation of the contract's digests.
//!
//! FINDING ac2-m7 (minor): every digest of the contract is the PLAIN BLAKE3 of some bytes (`blake3_hex`): the spec
//! digest (job.json bytes), the RESUME reference (RESUME.json bytes), file / prefix digests, the canonical stream
//! digest (canonical lines + "\n") and the per-record digest (one canonical line + "\n"). There is no domain
//! separation, so digests of DIFFERENT kinds coincide whenever their preimages do: the per-record digest of a record
//! equals the canonical digest of the one-record stream, and equals the raw file digest of that stream when its
//! line is already canonical. No slot of the contract compares a digest of one kind with a digest of another kind
//! [read: classify.rs, preflight.rs, seal.rs; every check recomputes the digest of the expected kind from the file
//! at its layout path], so no wrong acceptance follows today; the requirement "two record kinds can never collide"
//! is nevertheless not met by construction. Smallest fix (a contract version bump: goldens and engine change
//! together): one `blake3::Hasher::new_derive_key("rustred.child-job.v1/<kind>")` context per digest kind (spec,
//! resume, file, prefix, canonical-stream, record), or a kind prefix byte in the preimage.

use rustred_campaign_contract::canon::{CanonicalDigest, canonical_line, canonical_line_blake3};
use rustred_campaign_contract::{blake3_hex, spec::spec_blake3};

#[test]
fn ac2_digest_kinds_are_not_domain_separated() {
    let rec = br#"{"index":0,"rhs":[],"type":"rule"}"#;
    assert_eq!(
        canonical_line(rec).unwrap(),
        rec.to_vec(),
        "already canonical"
    );
    let per_record = canonical_line_blake3(rec).unwrap();
    let mut d = CanonicalDigest::new();
    d.push_line(rec).unwrap();
    let stream = d.hex();
    let file = blake3_hex(&[&rec[..], b"\n"].concat());
    let spec_like = spec_blake3(&[&rec[..], b"\n"].concat());
    eprintln!(
        "per-record {per_record}\nstream     {stream}\nraw file   {file}\nspec-style {spec_like}"
    );
    // the requirement: digests of different kinds never coincide
    assert!(
        per_record != stream && stream != file && file != spec_like,
        "record, canonical-stream, file and spec digests of one preimage are identical"
    );
}
