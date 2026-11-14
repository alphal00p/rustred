//! Reducer speed. The measured number (release build) is recorded in the interface document:
//! `cargo test -p rustred-campaign --release --test ledger_speed -- --ignored --nocapture`.
mod ledger_support;

use std::time::Instant;

use ledger_support::*;
use rustred_campaign::ledger::{self, Event, LEDGER_SCHEMA, Record};
use rustred_campaign::state::{CampaignState, EvidenceLevel, reduce};

/// Builds `n_items` item lives (10 records each) as serialised, hash-chained lines.
fn build_lines(n_items: usize) -> Vec<u8> {
    let mut events: Vec<Event> = vec![genesis_event()];
    events.extend(setup_events("T162"));
    let mut out = Vec::new();
    let mut prev = ledger::GENESIS.to_string();
    let mut seq = 0u64;
    let mut push = |e: Event, out: &mut Vec<u8>| {
        seq += 1;
        let r = Record {
            schema: LEDGER_SCHEMA.into(),
            seq,
            utc: "2026-11-13T23:00:00.000Z".into(),
            prev: prev.clone(),
            event: e,
        };
        let (line, h) = ledger::encode_record(&r);
        prev = h;
        out.extend_from_slice(&line);
        out.push(b'\n');
    };
    for e in events {
        push(e, &mut out);
    }
    for k in 0..n_items {
        for e in item_life(
            "T162",
            k,
            (k % 5) as u8,
            EvidenceLevel::IdentityTested,
            (k % 1000) as u64,
        ) {
            push(e, &mut out);
        }
    }
    out
}

/// Framing + digest + canonical check + chain check + reduce, exactly what `Ledger::open` does per line.
fn parse_and_reduce(bytes: &[u8]) -> (CampaignState, u64) {
    let mut st = CampaignState::default();
    let mut prev = ledger::GENESIS.to_string();
    let mut n = 0;
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let (r, h) = ledger::decode_record(line).unwrap();
        assert_eq!(r.prev, prev);
        st.apply_record(&r).unwrap();
        prev = h;
        n += 1;
    }
    (st, n)
}

#[test]
fn reducer_speed_small() {
    let bytes = build_lines(5_000);
    let t = Instant::now();
    let (st, n) = parse_and_reduce(&bytes);
    let dt = t.elapsed();
    assert_eq!(st.rules.accepted, 5_000);
    eprintln!(
        "{n} records ({} bytes) parsed + chain-checked + reduced in {dt:?} (debug build)",
        bytes.len()
    );
    assert!(dt.as_secs() < 60);
}

#[test]
#[ignore = "speed measurement: run in release with --ignored --nocapture"]
fn reducer_speed_one_million() {
    let bytes = build_lines(100_000); // 100,000 lives x 10 records + 9 setup records
    let t = Instant::now();
    let (st, n) = parse_and_reduce(&bytes);
    let full = t.elapsed();
    assert!(n >= 1_000_000);
    // reduction alone over pre-parsed records
    let mb = bytes.len() >> 20;
    let recs: Vec<Record> = bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect();
    drop(bytes);
    let t = Instant::now();
    let st2 = reduce(&recs).unwrap();
    let pure = t.elapsed();
    assert_eq!(st, st2);
    // snapshot size and (de)serialisation cost of that state
    let t = Instant::now();
    let snap = serde_json::to_vec(&st).unwrap();
    let ser = t.elapsed();
    let t = Instant::now();
    let back: CampaignState = serde_json::from_slice(&snap).unwrap();
    let de = t.elapsed();
    assert_eq!(back, st);
    eprintln!(
        "SPEED: {n} records, {} MB: parse+chain+reduce {full:?}; reduce only {pure:?}; snapshot {} MB, write {ser:?}, read {de:?}",
        mb,
        snap.len() >> 20
    );
}
