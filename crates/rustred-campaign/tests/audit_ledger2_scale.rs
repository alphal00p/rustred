//! Re-audit of the ledger (lane audit-ledger2): behaviour and memory at ~2 million records. One phase per
//! process (fresh VmHWM), release build, under the light-build scope:
//! `RR_A2_SCALE_DIR=<dir> RR_A2_SCALE_PHASE=build|open|read|verify|final|ops cargo test --release --test
//! audit_ledger2_scale -- --ignored --nocapture`. A counting global allocator gives the live heap of the reduced
//! state and of each of its components (what a leaner state could save).
mod ledger_support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ledger_support::*;
use rustred_campaign::ledger::{self, Event, ItemEvent, LedgerConfig};
use rustred_campaign::state::EvidenceLevel;

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            LIVE.fetch_sub(l.size(), Ordering::Relaxed);
            let n = LIVE.fetch_add(new, Ordering::Relaxed) + new;
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

fn mb(b: usize) -> String {
    format!("{:.1} MB", b as f64 / 1e6)
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

const NODES: usize = 300;

fn node_setup(n: &str) -> Vec<Event> {
    vec![
        input(&format!("{n}.family"), "family", "family"),
        input(&format!("{n}.box"), "box", "box"),
        Event::NodeRegistered {
            node: n.into(),
            inputs: vec![format!("{n}.family")],
            terminal_bound: None,
            spec_box: Some(format!("{n}.box")),
            spec: ledger::NodeSpecValues::default(),
            reference: None,
        },
        Event::Node {
            node: n.into(),
            ev: rustred_campaign::state::NodeEvent::Start,
        },
    ]
}

fn node_name(k: usize) -> String {
    format!("X{:03}", k % NODES)
}

#[test]
#[ignore = "scale: RR_A2_SCALE_DIR / RR_A2_SCALE_PHASE"]
fn a2_scale() {
    let Some(dir) = std::env::var_os("RR_A2_SCALE_DIR") else {
        return;
    };
    let d = std::path::PathBuf::from(dir);
    let phase = std::env::var("RR_A2_SCALE_PHASE").unwrap_or_default();
    let target: u64 = std::env::var("RR_A2_SCALE_RECORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2_000_000);
    let t = Instant::now();
    match phase.as_str() {
        "build" => {
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            let clk = clock();
            let mut l = new_ledger(&d, as_dyn(&clk), LedgerConfig::default());
            let mut setup = vec![
                input("spec", "spec", "spec"),
                input("rr_node", "binary", "rr_node v1"),
            ];
            setup.push(Event::Campaign {
                ev: rustred_campaign::state::CampaignEvent::Start,
                reason: "scale".into(),
            });
            setup.push(Event::Campaign {
                ev: rustred_campaign::state::CampaignEvent::Run,
                reason: "scale".into(),
            });
            for k in 0..NODES {
                setup.extend(node_setup(&node_name(k)));
            }
            l.append_all(setup).unwrap();
            let mut batch = Vec::new();
            let mut k = 0usize;
            let mut worst = std::time::Duration::ZERO;
            while l.seq() < target {
                let n = node_name(k);
                let mut life = item_life(
                    &n,
                    k,
                    (k % 5) as u8,
                    EvidenceLevel::IdentityTested,
                    (k % 1000) as u64,
                );
                if k.is_multiple_of(10) {
                    // a rejected first result and a second job (more jobs per item, a reopen)
                    life.push(Event::Item {
                        item: item_id(&n, k),
                        ev: ItemEvent::Reject {
                            reason: "late".into(),
                            disproof: None,
                        },
                    });
                    life.push(Event::Item {
                        item: item_id(&n, k),
                        ev: ItemEvent::Requeue {
                            reason: "again".into(),
                        },
                    });
                    life.push(Event::Note {
                        text: format!("retry {k}"),
                    });
                }
                batch.extend(life);
                k += 1;
                if batch.len() >= 1000 {
                    let t1 = Instant::now();
                    l.append_all(std::mem::take(&mut batch)).unwrap();
                    worst = worst.max(t1.elapsed());
                }
            }
            l.append_all(batch).unwrap();
            println!(
                "A2SCALE build: {} records, {} items, {} sealed segments, {:.1} s, worst append_all of 1000 records {:.2} s (a rotation writes the full snapshot), VmHWM {}, warnings {}",
                l.seq(),
                k,
                l.manifest().sealed.len(),
                t.elapsed().as_secs_f64(),
                worst.as_secs_f64(),
                vm_hwm(),
                l.warnings().len()
            );
        }
        "open" => {
            let clk = clock();
            let base = LIVE.load(Ordering::SeqCst);
            let (l, rep) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
            let dt = t.elapsed();
            let live = LIVE.load(Ordering::SeqCst) - base;
            let st = l.state();
            println!(
                "A2SCALE open: seq {} in {:.2} s (from snapshot {:?}, replayed {}), VmHWM {}, live heap {} ({} B/record), peak heap {}",
                l.seq(),
                dt.as_secs_f64(),
                rep.from_snapshot,
                rep.replayed,
                vm_hwm(),
                mb(live),
                live as u64 / l.seq().max(1),
                mb(PEAK.load(Ordering::SeqCst))
            );
            let _ = st;
        }
        "breakdown" => {
            let mut v = ledger::open_read_only(&d).unwrap();
            let seq = v.seq;
            let (ni, nj, nd) = (
                v.state.items.len(),
                v.state.jobs.len(),
                v.state.done_ops.len(),
            );
            let settled = v
                .state
                .jobs
                .values()
                .filter(|j| j.state.is_settled())
                .count();
            let l0 = LIVE.load(Ordering::SeqCst);
            let mut parts: Vec<(&str, usize)> = Vec::new();
            macro_rules! part {
                ($name:expr, $field:ident) => {{
                    let a = LIVE.load(Ordering::SeqCst);
                    drop(std::mem::take(&mut v.state.$field));
                    parts.push(($name, a - LIVE.load(Ordering::SeqCst)));
                }};
            }
            part!("items", items);
            part!("jobs", jobs);
            part!("done_ops", done_ops);
            part!("pending_ops", pending_ops);
            part!("costs_ms", costs_ms);
            part!("nodes", nodes);
            part!("inputs", inputs);
            part!("provenance", provenance);
            part!("controls", controls);
            part!("recent", recent);
            let rest = LIVE.load(Ordering::SeqCst);
            drop(v);
            let total: usize =
                parts.iter().map(|p| p.1).sum::<usize>() + (rest - LIVE.load(Ordering::SeqCst));
            println!(
                "A2SCALE state heap: {} = {} B/record; {ni} items, {nj} jobs ({settled} settled), {nd} done ops; (live before drops {})",
                mb(total),
                total as u64 / seq.max(1),
                mb(l0)
            );
            for (k, b) in &parts {
                println!(
                    "A2SCALE   {k:<12} {:>10}  {:>5.1} %",
                    mb(*b),
                    100.0 * *b as f64 / total.max(1) as f64
                );
            }
            println!(
                "A2SCALE   per item {} B, per job {} B",
                parts[0].1 / ni.max(1),
                parts[1].1 / nj.max(1)
            );
        }
        "read" => {
            let v = ledger::open_read_only(&d).unwrap();
            println!(
                "A2SCALE open_read_only: seq {} in {:.2} s, VmHWM {}, peak heap {}",
                v.seq,
                t.elapsed().as_secs_f64(),
                vm_hwm(),
                mb(PEAK.load(Ordering::SeqCst))
            );
        }
        "verify" => {
            let r = ledger::verify_chain(&d).unwrap();
            println!(
                "A2SCALE verify_chain: {} records in {:.1} s, snapshot matches {:?}, VmHWM {}, peak heap {}",
                r.records,
                t.elapsed().as_secs_f64(),
                r.snapshot_matches,
                vm_hwm(),
                mb(PEAK.load(Ordering::SeqCst))
            );
        }
        "final" => {
            // supervisor::result::read_final_result = read_records (all records in memory) + reduce
            let r = rustred_campaign::supervisor::result::read_final_result(&d);
            println!(
                "A2SCALE read_final_result: ok {} in {:.1} s, VmHWM {}, peak heap {}",
                r.is_ok(),
                t.elapsed().as_secs_f64(),
                vm_hwm(),
                mb(PEAK.load(Ordering::SeqCst))
            );
        }
        "ops" => {
            let clk = clock();
            let (mut l, _) = open_ledger(&d, as_dyn(&clk), LedgerConfig::default()).unwrap();
            let items: Vec<String> = l
                .state()
                .items
                .iter()
                .filter(|(_, i)| matches!(i.phase, rustred_campaign::state::ItemPhase::Accepted(_)))
                .map(|(k, _)| k.clone())
                .take(1000)
                .collect();
            // 1000 mark_stale + reopen pairs: an item reopen scans every item for a live child (O(items))
            let t1 = Instant::now();
            for id in &items {
                l.append_all(vec![
                    Event::Item {
                        item: id.clone(),
                        ev: ItemEvent::MarkStale { input: "x".into() },
                    },
                    Event::Item {
                        item: id.clone(),
                        ev: ItemEvent::Reopen { reason: "x".into() },
                    },
                ])
                .unwrap();
            }
            let per_pair = t1.elapsed().as_secs_f64() / items.len().max(1) as f64;
            let t2 = Instant::now();
            l.append(input("X000.family", "family", "family v2"))
                .unwrap();
            let input_change = t2.elapsed();
            let t3 = Instant::now();
            l.rotate().unwrap();
            let rot = t3.elapsed();
            let snap = l.manifest().snapshot.clone().unwrap();
            let sz = std::fs::metadata(ledger::ledger_dir(&d).join(&snap.file))
                .unwrap()
                .len();
            println!(
                "A2SCALE ops: mark_stale+reopen pair {:.2} ms (one group commit each); one input change (O(items+jobs) scan) {:.1} ms; one rotation {:.2} s writing a {} snapshot; VmHWM {}",
                per_pair * 1e3,
                input_change.as_secs_f64() * 1e3,
                rot.as_secs_f64(),
                mb(sz as usize),
                vm_hwm()
            );
        }
        _ => panic!("RR_A2_SCALE_PHASE?"),
    }
}

/// Reducer cost of ONE input change as a function of the number of items (in memory, no I/O): a node-input
/// change stales the node's items; a binary change stales every item with a result (stale_items looks up the
/// children of each staled item by scanning every item).
#[test]
#[ignore = "scale: RR_A2_ITEMS=5000,10000,20000,40000"]
fn a2_input_change_cost() {
    let sizes: Vec<usize> = std::env::var("RR_A2_ITEMS")
        .unwrap_or_else(|_| "2000,4000,8000".into())
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    for n in sizes {
        let mut st = rustred_campaign::state::CampaignState::default();
        let mut seq = 0u64;
        let mut apply = |st: &mut rustred_campaign::state::CampaignState, e: Event| {
            seq += 1;
            let r = ledger::Record {
                schema: ledger::LEDGER_SCHEMA.into(),
                seq,
                utc: T0.into(),
                prev: ledger::GENESIS.into(),
                event: e,
            };
            st.apply_record(&r).unwrap();
        };
        apply(&mut st, genesis_event());
        apply(&mut st, input("rr_node", "binary", "rr_node v1"));
        for k in 0..NODES {
            for e in node_setup(&node_name(k)) {
                apply(&mut st, e);
            }
        }
        for k in 0..n {
            for e in item_life(&node_name(k), k, 0, EvidenceLevel::IdentityTested, 1) {
                apply(&mut st, e);
            }
        }
        let t = Instant::now();
        apply(&mut st, input("X000.family", "family", "family v2"));
        let node_change = t.elapsed();
        let t = Instant::now();
        apply(&mut st, input("rr_node", "binary", "rr_node v2"));
        let bin_change = t.elapsed();
        let stale = st
            .items
            .values()
            .filter(|i| i.phase == rustred_campaign::state::ItemPhase::Stale)
            .count();
        println!(
            "A2INPUT items {n}: node-input change {:.3} s; binary change {:.3} s ({stale} items stale)",
            node_change.as_secs_f64(),
            bin_change.as_secs_f64()
        );
    }
}
