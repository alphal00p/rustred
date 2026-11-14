//! Reducer speed. The measured number (release build) is recorded in the interface document:
//! `cargo test -p rustred-campaign --release --test ledger_speed -- --ignored --nocapture`.
mod ledger_support;

use std::time::Instant;

use ledger_support::*;
use rustred_campaign::ledger::{self, Event, LEDGER_SCHEMA, Record};
use rustred_campaign::state::{CampaignState, EvidenceLevel};

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
    // reduction alone over pre-parsed records (chunks of 100,000 records: bounded memory)
    let mb = bytes.len() >> 20;
    let mut st2 = CampaignState::default();
    let mut pure = std::time::Duration::ZERO;
    let mut chunk: Vec<Record> = Vec::with_capacity(100_000);
    let mut lines = bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .peekable();
    while lines.peek().is_some() {
        chunk.clear();
        for l in lines.by_ref().take(100_000) {
            chunk.push(ledger::decode_record(l).unwrap().0);
        }
        let t = Instant::now();
        for r in &chunk {
            st2.apply_record(r).unwrap();
        }
        pure += t.elapsed();
    }
    drop(chunk);
    drop(bytes);
    assert_eq!(st.digest(), st2.digest());
    drop(st2);
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

/// Scale build (audit B experiment 7): `RR_SCALE_DIR=<dir> RR_SCALE_ITEMS=200000` writes a real ledger of about
/// 10 records per item through the writer (batches of 100 item lives), then its progress files.
#[test]
#[ignore = "scale: builds a multi-million-record ledger in RR_SCALE_DIR"]
fn scale_build() {
    use rustred_campaign::ledger::LedgerConfig;
    use rustred_campaign::progress::{ProgressWriter, Resources};
    let Some(dir) = std::env::var_os("RR_SCALE_DIR") else {
        return;
    };
    let d = std::path::PathBuf::from(dir);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let n: usize = std::env::var("RR_SCALE_ITEMS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);
    let t = Instant::now();
    let clk = clock();
    let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
    l.append_all(setup_events("T162")).unwrap();
    let mut batch = Vec::new();
    for k in 0..n {
        batch.extend(item_life(
            "T162",
            k,
            (k % 5) as u8,
            EvidenceLevel::IdentityTested,
            (k % 1000) as u64,
        ));
        if batch.len() >= 1000 {
            l.append_all(std::mem::take(&mut batch)).unwrap();
        }
    }
    l.append_all(batch).unwrap();
    ProgressWriter::new()
        .write(&d, l.state(), l.head(), &Resources::default(), clk.as_ref())
        .unwrap();
    eprintln!(
        "SCALE build: {} records, {} sealed segments, {:.1} s, VmHWM {}",
        l.seq(),
        l.manifest().sealed.len(),
        t.elapsed().as_secs_f64(),
        vm_hwm()
    );
}

/// `RR_SCALE_DIR=<dir>`: open (write handle), open_read_only and progress::verify timed, peak memory per phase is
/// read from a fresh process each (run the test three times with RR_SCALE_PHASE=open|read|verify).
#[test]
#[ignore = "scale: measures open / read-only open / streaming verify on RR_SCALE_DIR"]
fn scale_measure() {
    use rustred_campaign::ledger::LedgerConfig;
    let Some(dir) = std::env::var_os("RR_SCALE_DIR") else {
        return;
    };
    let d = std::path::PathBuf::from(dir);
    let phase = std::env::var("RR_SCALE_PHASE").unwrap_or_else(|_| "verify".into());
    let t = Instant::now();
    match phase.as_str() {
        "open" => {
            let clk = clock();
            let (l, _) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
            eprintln!(
                "SCALE open: seq {} in {:.2} s, VmHWM {}",
                l.seq(),
                t.elapsed().as_secs_f64(),
                vm_hwm()
            );
        }
        "read" => {
            let v = ledger::open_read_only(&d).unwrap();
            eprintln!(
                "SCALE open_read_only: seq {} in {:.2} s, VmHWM {}",
                v.seq,
                t.elapsed().as_secs_f64(),
                vm_hwm()
            );
        }
        _ => {
            let r = rustred_campaign::progress::verify(&d).unwrap();
            eprintln!(
                "SCALE verify: {} records, clean {}, {:.1} s, VmHWM {}",
                r.records,
                r.clean(),
                t.elapsed().as_secs_f64(),
                vm_hwm()
            );
            assert!(
                r.clean(),
                "{:?} {:?} {:?}",
                r.differences.first(),
                r.missing,
                r.stale
            );
        }
    }
}

fn vm_hwm() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .map(|l| l.split_whitespace().skip(1).collect::<Vec<_>>().join(" "))
        })
        .unwrap_or_default()
}
