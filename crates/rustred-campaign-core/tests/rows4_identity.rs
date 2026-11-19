//! Lane rows4 (WS/notes/implementer.rows4.md): the re-audit RA3 fixes of the identity test.
//! - RA3-M1: backend identity of `Family` / `VerifiedGroup` (sealed `AlgebraOrigin`), certificate runs refuse a
//!   diagnostic origin, `ReplayProver` parses the spec text itself, `verify_record` re-parses the family;
//! - RA3-m2: the certificate nonce is derived from the rule digest and a campaign beacon; no beacon = `Unbound`; a
//!   non-default master seed is an override;
//! - mutation survivors M38 (eps axes not forced active in the zero-sector test) and M49 (the stream seed does not bind
//!   the served-key set): killed here;
//! - the traced certificate entry points (the `Both` mode's record of record) record without changing the run.

mod inputs;
mod rows_common;

use rows_common::face::{face, served};
use rows_common::*;
use rustred_campaign_core::accept::prover::ReplayProver;
use rustred_campaign_core::ibp_rows::{
    Beacon, NonceSource, PointSpec, ReplayLevel, ReplayPolicy, RowsOptions, RowsRule, TableBinding,
    Verdict, derive_nonce, identity_test, identity_test_traced, key_set_digest, replay_evidence,
    replay_evidence_traced, replay_evidence_with, stream_seed, verify_record,
};
use rustred_campaign_core::independent_algebra::Independent;
use rustred_campaign_core::replay_base::algebra::{Recorder, Stage};
use rustred_campaign_core::replay_base::family::Family;
use rustred_campaign_core::replay_base::symrows::{
    DeriveOptions, verify_derived, verify_group, verify_group_with,
};

/// M38: the zero-sector test ignores the mask bits of the eps axes (a line with a power shift k eps never has power 0:
/// it is ALWAYS active). Every mask of the two-eps-line family T001c and a sample of the arity-14 eps family T070c.
#[test]
fn rows4_zero_mask_forces_eps_axes_active() {
    for (name, cap) in [("T001c.sym", usize::MAX), ("T070c.sym", 2_000)] {
        let fam = Family::parse(&data(name)).unwrap();
        let n = fam.n();
        let eps = fam.eps_axes().to_vec();
        assert!(!eps.is_empty(), "{name} has eps lines");
        let (mut checked, mut differ_without) = (0, 0);
        for m in (0u64..(1u64 << n)).step_by(((1u64 << n) as usize / cap.min(1 << n)).max(1)) {
            let mask: Vec<bool> = (0..n).map(|i| m >> i & 1 == 1).collect();
            let mut forced = mask.clone();
            let mut cleared = mask.clone();
            for &i in &eps {
                forced[i] = true;
                cleared[i] = false;
            }
            let z = fam.zero_mask(&mask);
            assert_eq!(z, fam.zero_mask(&forced), "{name} mask {m:b}");
            assert_eq!(z, fam.zero_mask(&cleared), "{name} mask {m:b}");
            checked += 1;
            // the eps lines matter: without forcing, some masks would be decided differently (computed through a
            // family copy WITHOUT power shifts: the same propagators, no eps axes)
            differ_without += usize::from(z != no_eps(name).zero_mask(&cleared));
        }
        println!(
            "{name}: {checked} masks, eps axes {eps:?}; {differ_without} masks decided differently without them"
        );
        assert!(
            differ_without > 0,
            "{name}: the eps axes never matter (test has no power)"
        );
    }
}

fn no_eps(name: &str) -> Family {
    let text: String = data(name)
        .lines()
        .filter(|l| !l.contains("power_shift"))
        .collect::<Vec<_>>()
        .join("\n");
    Family::parse(&text).unwrap()
}

/// M49: the stream seed binds the served-key set (and every other input): records of the same rule on two served
/// sets use different streams.
#[test]
fn rows4_stream_seed_binds_the_served_keys() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let keys = served(&f);
    let (a, b) = (key_set_digest(&keys[..10]), key_set_digest(&keys[..11]));
    let fd = f.fam.source_digest();
    let s = |m: u64, kd: &str, nonce: u64| stream_seed(m, &good, fd, kd, nonce);
    assert_ne!(s(1, &a, 7), s(1, &b, 7), "key set");
    assert_ne!(s(1, &a, 7), s(2, &a, 7), "master seed");
    assert_ne!(s(1, &a, 7), s(1, &a, 8), "nonce");
    assert_ne!(
        s(1, &a, 7),
        stream_seed(1, &good, &"0".repeat(64), &a, 7),
        "family"
    );
    let pol = ReplayPolicy::default();
    let r10 = replay_evidence(&f.fam, &good, &keys[..10], &pol, &bound(&f.fam));
    let r11 = replay_evidence(&f.fam, &good, &keys[..11], &pol, &bound(&f.fam));
    assert_eq!(
        r10.nonce, r11.nonce,
        "the derived nonce depends on rule and beacon only"
    );
    assert_ne!(r10.seed, r11.seed, "the served set changes the stream");
    assert_ne!(r10.primes, r11.primes);
}

/// RA3-m2: the certificate nonce is derived (rule digest, beacon): reproducible, recorded, rule- and beacon-specific;
/// without a beacon the record is `Unbound`; a caller nonce or a non-default master seed is an override.
#[test]
fn rows4_nonce_is_derived_from_the_beacon() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let bad = RowsRule::from_value(f.bad.clone(), n).unwrap();
    let keys = served(&f);
    let pol = ReplayPolicy::default();
    assert!(Beacon::from_hex("abc").is_err());
    assert!(Beacon::from_hex(&"g".repeat(32)).is_err());
    let b0 = test_beacon(0);
    assert_ne!(derive_nonce(&good, &b0), derive_nonce(&bad, &b0));
    assert_ne!(
        derive_nonce(&good, &b0),
        derive_nonce(&good, &test_beacon(1))
    );
    let r = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    assert_eq!(r.level, ReplayLevel::Established);
    assert_eq!(r.nonce_source, NonceSource::Derived);
    assert_eq!(r.nonce, derive_nonce(&good, &b0));
    assert_eq!(r.nonce_beacon.as_deref(), Some(b0.as_str()));
    assert!(r.certificate);
    let again = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    assert_eq!(
        (again.seed, again.primes),
        (r.seed, r.primes),
        "reproducible"
    );
    assert!(verify_record(&r, &good, &f.fam, &keys).is_ok());
    // a record whose beacon is replaced does not verify
    let mut other = r.clone();
    other.nonce_beacon = Some(test_beacon(9).as_str().to_string());
    assert!(verify_record(&other, &good, &f.fam, &keys).is_err());
    // no beacon: an OS nonce, Unbound
    let nb = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            beacon: None,
            ..bound(&f.fam)
        },
    );
    assert_eq!(nb.nonce_source, NonceSource::Os);
    assert!(
        matches!(&nb.level, ReplayLevel::Unbound { reason } if reason.contains("beacon")),
        "{:?}",
        nb.level
    );
    // a non-default master seed is a caller choice
    let ms = replay_evidence(
        &f.fam,
        &good,
        &keys,
        &pol,
        &RowsOptions {
            seed: 7,
            ..bound(&f.fam)
        },
    );
    assert!(
        matches!(&ms.level, ReplayLevel::Overridden { overrides, .. } if overrides.iter().any(|o| o.contains("master seed"))),
        "{:?}",
        ms.level
    );
    println!(
        "derived nonce {:#x}; no beacon -> {:?}; master seed 7 -> Overridden",
        r.nonce,
        level_name(&nb.level)
    );
}

fn level_name(l: &ReplayLevel) -> &'static str {
    match l {
        ReplayLevel::Established => "Established",
        ReplayLevel::ReplaySampled { .. } => "ReplaySampled",
        ReplayLevel::NotEstablished { .. } => "NotEstablished",
        ReplayLevel::Diagnostic { .. } => "Diagnostic",
        ReplayLevel::Overridden { .. } => "Overridden",
        ReplayLevel::Unbound { .. } => "Unbound",
    }
}

/// RA3-M1: origins. `Family::parse` / `parse_traced` are certificate; `parse_with` (any backend, even `Independent`)
/// is diagnostic and refused by a certificate run; `verify_group_with` is diagnostic; a certificate group needs a
/// certificate family; the generic `replay_evidence_with` is never a certificate.
#[test]
fn rows4_origins_are_sealed() {
    let text = data("LA3_handwritten.sym");
    let cert = Family::parse(&text).unwrap();
    let diag = Family::parse_with(&Independent, &text).unwrap();
    assert!(cert.origin().is_certificate());
    assert!(!diag.origin().is_certificate());
    assert_eq!(
        cert.table_digest(),
        diag.table_digest(),
        "same backend, same table"
    );
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let keys = served(&f);
    let pol = ReplayPolicy::default();
    let on_diag = replay_evidence(&diag, &good, &keys, &pol, &bound(&diag));
    assert!(matches!(on_diag.identity.verdict, Verdict::Unsupported(_)));
    assert!(matches!(on_diag.level, ReplayLevel::NotEstablished { .. }));
    // verify_record accepts the diagnostic object whose table equals the certificate parse (it re-parses the text)
    let r = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    assert!(verify_record(&r, &good, &diag, &keys).is_ok());
    let w = replay_evidence_with(&Independent, &f.fam, &good, &keys, &pol, &bound(&f.fam));
    assert!(!w.certificate);
    assert!(matches!(w.level, ReplayLevel::Diagnostic { .. }));
    // groups
    let t = t162_family();
    let chart: Vec<bool> = (0..t.n()).map(|i| i < 9).collect();
    let g = verify_derived(&t, &chart, &DeriveOptions::default()).unwrap();
    assert!(g.origin().is_certificate());
    let gd = verify_group_with(&Independent, &t, g.group().clone(), "test").unwrap();
    assert!(!gd.origin().is_certificate());
    let t_diag = Family::parse_with(&Independent, t.source()).unwrap();
    let gs = verify_group(&t_diag, g.group().clone(), "test").unwrap();
    assert!(
        !gs.origin().is_certificate(),
        "a group verified on a diagnostic family is diagnostic"
    );
}

/// RA3-M1: `ReplayProver::new` takes the family TEXT (parses it itself) and the beacon.
#[test]
fn rows4_replay_prover_parses_the_text() {
    let text = data("LA3_handwritten.sym");
    let fam = Family::parse(&text).unwrap();
    let p = ReplayProver::new(
        &text,
        TableBinding::from_family_digest(fam.squashed_digest()),
        test_beacon(3),
        RowsOptions::default(),
    )
    .unwrap();
    assert!(p.family().origin().is_certificate());
    assert_eq!(p.family().table_digest(), fam.table_digest());
    assert_eq!(p.opts().beacon, Some(test_beacon(3)));
    assert!(
        ReplayProver::new(
            "I(nonsense)",
            TableBinding::from_family_digest(fam.squashed_digest()),
            test_beacon(3),
            RowsOptions::default()
        )
        .is_err()
    );
}

/// The traced certificate entry points record the stages (Family, Primes, Points, Coefficient, Rows, Columns,
/// Elimination, Membership, Verdict) and give the same verdict / record as the plain ones.
#[test]
fn rows4_traced_certificate_runs_record_without_changing_the_run() {
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let keys = served(&f);
    let pts = vec![
        PointSpec::Generic,
        PointSpec::Generic,
        PointSpec::Key(keys[0].clone()),
    ];
    let rec = Recorder::new();
    let fam = Family::parse_traced(f.fam.source(), &rec).unwrap();
    assert!(fam.origin().is_certificate());
    let v = identity_test_traced(&fam, &good, &pts, &RowsOptions::default(), &rec);
    let plain = identity_test(&f.fam, &good, &pts, &RowsOptions::default());
    assert!(v.certificate && plain.certificate);
    assert_eq!(v.verdict, plain.verdict);
    let ev = rec.take();
    let stages: std::collections::BTreeSet<Stage> = ev.iter().map(|e| e.stage).collect();
    println!("traced identity: {} events, stages {stages:?}", ev.len());
    for s in [
        Stage::Family,
        Stage::Primes,
        Stage::Points,
        Stage::Coefficient,
        Stage::Rows,
        Stage::Columns,
        Stage::Elimination,
        Stage::Membership,
        Stage::Verdict,
    ] {
        assert!(stages.contains(&s), "{s:?}");
    }
    let pol = ReplayPolicy::default();
    let r1 = replay_evidence_traced(&fam, &good, &keys, &pol, &bound(&fam), &rec);
    let r2 = replay_evidence(&f.fam, &good, &keys, &pol, &bound(&f.fam));
    assert_eq!(
        (r1.seed, r1.primes, &r1.level),
        (r2.seed, r2.primes, &r2.level)
    );
    assert_eq!(r1.level, ReplayLevel::Established);
    assert!(!rec.take().is_empty());
}

/// RA3-M1 for GROUPS (mutation N02 survived the first harness run: nothing checked the refusal): a certificate run
/// REFUSES a symmetry group of diagnostic origin (verified through a generic backend), even an honest one; the same group
/// verified on the certificate path is accepted; a diagnostic run accepts both.
#[test]
fn rows4_certificate_run_refuses_a_diagnostic_group() {
    use rustred_campaign_core::ibp_rows::{SymUse, Unsupported, identity_test_with};
    use std::sync::Arc;
    let f = face();
    let n = f.fam.n();
    let good = RowsRule::from_value(f.good.clone(), n).unwrap();
    let chart: Vec<bool> = (0..n).map(|i| i < 7).collect();
    let cert = verify_derived(&f.fam, &chart, &DeriveOptions::default()).unwrap();
    let diag = verify_group_with(&Independent, &f.fam, cert.group().clone(), "test").unwrap();
    assert!(cert.origin().is_certificate() && !diag.origin().is_certificate());
    let opts = |g: Arc<_>| RowsOptions {
        symmetry: Some(g),
        sym_use: SymUse::Always,
        ..RowsOptions::default()
    };
    let (cert, diag) = (Arc::new(cert), Arc::new(diag));
    let pts = [PointSpec::Generic];
    let vd = identity_test(&f.fam, &good, &pts, &opts(diag.clone()));
    let vc = identity_test(&f.fam, &good, &pts, &opts(cert.clone()));
    let vw = identity_test_with(&Independent, &f.fam, &good, &pts, &opts(diag));
    println!(
        "group origin: certificate run with a diagnostic group {:?}; with the certificate group {}; diagnostic run with the diagnostic group {} ({} elements)",
        vd.verdict,
        vc.verdict.kind(),
        vw.verdict.kind(),
        cert.group().elements.len()
    );
    assert!(
        matches!(&vd.verdict, Verdict::Unsupported(Unsupported::SymGroupMismatch(w)) if w.contains("diagnostic origin")),
        "{:?}",
        vd.verdict
    );
    for v in [&vc.verdict, &vw.verdict] {
        assert!(
            !matches!(v, Verdict::Unsupported(Unsupported::SymGroupMismatch(_))),
            "{v:?}"
        );
    }
}
