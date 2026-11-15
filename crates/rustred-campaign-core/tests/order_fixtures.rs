//! core-order acceptance on the FULL reference data (O1, O2, O3 counts, speed). Large files are not in the
//! repository: set `RUSTRED_CAMPAIGN_FIXTURES` to the directory holding `order/` (converted by core-order's
//! `make_fixtures.py`; see WS/plan/auto-campaign/interfaces/campaign-core-order.md) and run
//! `cargo test -p rustred-campaign-core --release --test order_fixtures -- --ignored --nocapture`.
//!
//! Binary record layouts (written by the core-order regeneration scripts that replay the checker's own procedures):
//! * `o1_t162_bank_pairs.bin`, 42 bytes: target i8[20], rhs i8[20], flags u16 LE (bit0 checker rankC lower, bit1
//!   checker compare.rs-port lower, bit2 checker O2 lower, bit3 rhs top sector). z27 banktest.py procedure.
//! * `o2_t178B_pairs.bin`, 44 bytes: target i8[20], rhs i8[20], class u8 (0 top, 1 sub, 2 numpos), pad u8, flags
//!   u16 LE (bit m = d1.py reading m says lower; top keys only). h13b d1.py procedure.

mod order_common;

use std::path::PathBuf;
use std::time::Instant;

use order_common::*;
use rustred_campaign_core::order::NodeComparator;

fn fixture(name: &str) -> PathBuf {
    let dir = std::env::var("RUSTRED_CAMPAIGN_FIXTURES")
        .expect("set RUSTRED_CAMPAIGN_FIXTURES (see module docs)");
    PathBuf::from(dir).join("order").join(name)
}

struct Pair {
    target: [i32; 20],
    rhs: [i32; 20],
    class: u8,
    flags: u16,
}

fn read_pairs(name: &str, size: usize) -> Vec<Pair> {
    let bytes = std::fs::read(fixture(name)).unwrap();
    assert_eq!(bytes.len() % size, 0);
    bytes
        .chunks_exact(size)
        .map(|r| {
            let key = |o: usize| -> [i32; 20] { std::array::from_fn(|i| r[o + i] as i8 as i32) };
            let (class, flags) = if size == 42 {
                let f = u16::from_le_bytes([r[40], r[41]]);
                (if f & 8 != 0 { 0 } else { 1 }, f)
            } else {
                (r[40], u16::from_le_bytes([r[42], r[43]]))
            };
            Pair {
                target: key(0),
                rhs: key(20),
                class,
                flags,
            }
        })
        .collect()
}

/// O1: all 2,448 T162 bank rules at the checker's 7,341 sample points: every RHS key lower than its target under
/// the comparator the bank header selects (legacy, presented frame); agreement with the checker's rankC and its
/// compare.rs port on every key; negative controls.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES/order/o1_t162_bank_pairs.bin (65 MB)"]
fn order_o1_t162_bank_all_rules() {
    let pairs = read_pairs("o1_t162_bank_pairs.bin", 42);
    let c = NodeComparator::from_header(&headers()["t162_bank"]).unwrap();
    let program =
        NodeComparator::from_descriptor(rustred_order::OrderDescriptor::spired_uncut(20)).unwrap();
    let (mut top, mut top_lower, mut sub, mut sub_lower) = (0usize, 0usize, 0usize, 0usize);
    let (mut rank_c_disagree, mut port_disagree, mut program_disagree, mut swapped_lower) =
        (0, 0, 0, 0);
    let (mut o2_top, mut o2_top_disagree, mut o2_not_lower_all) = (0usize, 0usize, 0usize);
    let o2 = t162_o2();
    let reversed = NodeComparator::legacy(20, Some((0..20).rev().collect())).unwrap();
    let ascending = {
        let mut d = rustred_order::OrderDescriptor::spired_uncut(20);
        d.active_direction = rustred_order::Direction::Ascending;
        d.inactive_direction = rustred_order::Direction::Ascending;
        NodeComparator::from_descriptor(d).unwrap()
    };
    let lines_rev = NodeComparator::legacy(20, Some((0..9).rev().chain(9..20).collect())).unwrap();
    let (mut rev_fail, mut asc_fail, mut lrev_fail) = (0usize, 0usize, 0usize);
    for p in &pairs {
        let lower = c.lower(&p.rhs, &p.target);
        assert_eq!(p.class == 0, is_top(&p.rhs, 9));
        if p.class == 0 {
            top += 1;
            top_lower += lower as usize;
        } else {
            sub += 1;
            sub_lower += lower as usize;
        }
        rank_c_disagree += (lower != (p.flags & 1 != 0)) as usize;
        port_disagree += (lower != (p.flags & 2 != 0)) as usize;
        program_disagree += (lower != program.lower(&p.rhs, &p.target)) as usize;
        swapped_lower += c.lower(&p.target, &p.rhs) as usize;
        let o2_lower = o2.lower(&p.rhs, &p.target);
        o2_not_lower_all += !o2_lower as usize;
        if p.class == 0 && is_top(&p.target, 9) {
            o2_top += 1;
            o2_top_disagree += (o2_lower != (p.flags & 4 != 0)) as usize;
        }
        rev_fail += !reversed.lower(&p.rhs, &p.target) as usize;
        asc_fail += !ascending.lower(&p.rhs, &p.target) as usize;
        lrev_fail += !lines_rev.lower(&p.rhs, &p.target) as usize;
    }
    let checker_o2_not_lower = pairs.iter().filter(|p| p.flags & 4 == 0).count();
    println!(
        "O1 T162: pairs {} top {top}/{top_lower} lower, sub {sub}/{sub_lower} lower; disagree rankC {rank_c_disagree} \
         port {port_disagree} legacy-vs-program {program_disagree}; swapped lower {swapped_lower}",
        pairs.len()
    );
    println!(
        "O1 negative controls: reversed tie priority {rev_fail} not lower; Ascending directions {asc_fail}; \
         reversed line priority {lrev_fail}; O2 descriptor {o2_not_lower_all} (checker O2: {checker_o2_not_lower}), \
         O2 top-sector agreement {}/{o2_top}",
        o2_top - o2_top_disagree
    );
    assert_eq!(pairs.len(), 1_543_361);
    assert_eq!((top, top_lower), (380_314, 380_314));
    assert_eq!((sub, sub_lower), (1_163_047, 1_163_047));
    assert_eq!(
        (
            rank_c_disagree,
            port_disagree,
            program_disagree,
            swapped_lower
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(o2_top_disagree, 0);
    assert!(rev_fail > 10_000 && asc_fail > 10_000 && lrev_fail > 1_000);
}

/// O2: T178 order B, 15,399 domain points: 769,226 top-sector RHS keys, 0 not lower under the comparator the order-B
/// table header selects; each of the checker's eleven readings reproduced bit for bit (O3 i: the ten wrong readings
/// fail on exactly the checker's counts); the helper's keys header read by the engine rule (identity) fails on 78,354.
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES/order/o2_t178B_pairs.bin (204 MB)"]
fn order_o2_t178b_all_rules() {
    let pairs = read_pairs("o2_t178B_pairs.bin", 44);
    let h = headers();
    let c = NodeComparator::from_header(&h["t178B_table"]).unwrap();
    let identity = NodeComparator::from_header(&helper_header_with_arity()).unwrap();
    let readings = t178_readings();
    let mut counts = [0usize; 3];
    let mut class_lower = [0usize; 3];
    let mut reading_fail = vec![0usize; readings.len()];
    let mut reading_disagree = vec![0usize; readings.len()];
    let (mut identity_fail, mut swapped_lower, mut self_keys) = (0usize, 0usize, 0usize);
    let mut sub_not_lower_with_positive_numerator = 0usize;
    for p in &pairs {
        let cl = p.class as usize;
        counts[cl] += 1;
        let lower = c.lower(&p.rhs, &p.target);
        class_lower[cl] += lower as usize;
        if cl == 1 && !lower {
            sub_not_lower_with_positive_numerator += p.rhs[10..].iter().any(|&x| x > 0) as usize;
        }
        if cl != 0 {
            continue;
        }
        self_keys += (p.rhs == p.target) as usize;
        swapped_lower += c.lower(&p.target, &p.rhs) as usize;
        identity_fail += !identity.lower(&p.rhs, &p.target) as usize;
        for (m, (_, r)) in readings.iter().enumerate() {
            let l = r.lower(&p.rhs, &p.target);
            reading_fail[m] += !l as usize;
            reading_disagree[m] += (l != (p.flags >> m & 1 != 0)) as usize;
        }
    }
    println!(
        "O2 T178B: pairs {} top {}/{} lower, sub {}/{} lower, numpos {}/{} lower; self {self_keys}; swapped lower {swapped_lower}",
        pairs.len(),
        counts[0],
        class_lower[0],
        counts[1],
        class_lower[1],
        counts[2],
        class_lower[2]
    );
    for (m, (name, _)) in readings.iter().enumerate() {
        println!(
            "  {name}: not lower {} (bit disagreements with the checker {})",
            reading_fail[m], reading_disagree[m]
        );
    }
    println!(
        "  helper keys header read by the engine rule (legacy identity): not lower {identity_fail}"
    );
    println!(
        "  subsector keys not lower: {} (with a positive numerator: {sub_not_lower_with_positive_numerator})",
        counts[1] - class_lower[1]
    );
    assert_eq!(pairs.len(), 4_626_543);
    assert_eq!(counts, [769_226, 3_755_978, 101_339]);
    assert_eq!(class_lower[0], 769_226, "O2: 0 not lower");
    // engine semantics outside the checker's scope (it skips these classes): a positive numerator adds an active
    // slot, so such keys are NOT lower; they descend only through a vanishing coefficient (requirement R9 (d))
    assert_eq!(class_lower[2], 0);
    assert_eq!(counts[1] - class_lower[1], 499);
    assert_eq!(sub_not_lower_with_positive_numerator, 499);
    assert_eq!(swapped_lower, 0);
    assert_eq!(
        reading_fail,
        vec![
            0, 225_327, 133_930, 91_397, 78_354, 146_973, 118_462, 106_865, 62_274, 163_053, 50_321
        ]
    );
    assert!(
        reading_disagree.iter().all(|&d| d == 0),
        "{reading_disagree:?}"
    );
    assert_eq!(identity_fail, 78_354);
}

/// The k3 frozen comparator pair fixtures in full (F3 T162: 1,430 lines; F5 T178B: 1,470 lines).
#[test]
#[ignore = "needs RUSTRED_CAMPAIGN_FIXTURES/order/F3_T162_pairs.txt and F5_T178B_pairs.txt"]
fn order_k3_comparator_fixtures_in_full() {
    let h = headers();
    let t162 = NodeComparator::from_header(&h["t162_bank"]).unwrap();
    let t178 = NodeComparator::from_header(&h["t178B_table"]).unwrap();
    let identity = NodeComparator::from_header(&helper_header_with_arity()).unwrap();
    let f3 = parse_pairs(&std::fs::read_to_string(fixture("F3_T162_pairs.txt")).unwrap());
    let f5 = parse_pairs(&std::fs::read_to_string(fixture("F5_T178B_pairs.txt")).unwrap());
    assert_eq!((f3.len(), f5.len()), (1430, 1470));
    for p in &f3 {
        assert_eq!(
            t162.lower(&p.rhs, &p.target),
            p.expect,
            "{} {:?} {:?}",
            p.kind,
            p.target,
            p.rhs
        );
    }
    let mut discriminators = 0;
    for p in &f5 {
        assert_eq!(
            t178.lower(&p.rhs, &p.target),
            p.expect,
            "{} {:?} {:?}",
            p.kind,
            p.target,
            p.rhs
        );
        assert_eq!(identity.lower(&p.rhs, &p.target), p.alt.unwrap());
        if p.kind == "priority_discriminator" {
            discriminators += 1;
            assert!(p.expect && !identity.lower(&p.rhs, &p.target));
        }
    }
    assert_eq!(discriminators, 200);
    println!(
        "k3 F3 1430/1430 and F5 1470/1470 verdicts reproduced; 200 discriminators fail under identity"
    );
}

/// Speed: comparisons per second on the O1 pairs (legacy path, program path, rank).
#[test]
#[ignore = "speed measurement; run with --release"]
fn order_speed() {
    let pairs = read_pairs("o1_t162_bank_pairs.bin", 42);
    let legacy = NodeComparator::legacy(20, None).unwrap();
    let mut d = rustred_order::OrderDescriptor::spired_uncut(20);
    d.coordinate_priority = T178_PRIO.to_vec();
    let program = NodeComparator::from_descriptor(d).unwrap();
    for (name, c) in [("legacy", &legacy), ("program", &program)] {
        let reps = 5;
        let t = Instant::now();
        let mut n = 0usize;
        for _ in 0..reps {
            for p in &pairs {
                n += c.lower(&p.rhs, &p.target) as usize;
            }
        }
        let s = t.elapsed().as_secs_f64();
        println!(
            "{name}: {:.1} M comparisons/s ({} comparisons, {n} lower)",
            (reps * pairs.len()) as f64 / s / 1e6,
            reps * pairs.len()
        );
        let t = Instant::now();
        let mut m = 0usize;
        for p in &pairs {
            m += c.rank(&p.rhs).0.len();
        }
        println!(
            "{name} rank: {:.2} M keys/s ({m})",
            pairs.len() as f64 / t.elapsed().as_secs_f64() / 1e6
        );
    }
}
