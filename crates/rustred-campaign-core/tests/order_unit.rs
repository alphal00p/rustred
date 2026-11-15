//! core-order: semantics, header selection, presentation, pattern comparison, properties, differential tests and the
//! automorphism-invariance test of `rustred_campaign_core::order` (default test run, in-tree data only).

use std::cmp::Ordering;

use rustred_campaign_core::order::{
    GroupElement, NodeComparator, NodeOrderRecord, OrderError, PatternSlot, Presentation,
    Selection, SourceOrder, automorphism_invariance, descriptor_from_order_json, parse_pattern,
};
use rustred_order::{CompiledOrder, CoordinateGroups, Direction, Limits, OrderDescriptor};

/// Deterministic xorshift64* generator (no dependency).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo + 1) as u64) as i32
    }
    fn perm(&mut self, n: usize) -> Vec<usize> {
        let mut p: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            p.swap(i, (self.next() % (i as u64 + 1)) as usize);
        }
        p
    }
    /// A key with `lines` line slots: mostly top sector, sometimes a removed line (<= 0) or a positive numerator.
    fn key(&mut self, n: usize, lines: usize) -> Vec<i32> {
        (0..n)
            .map(|s| {
                let wild = self.next().is_multiple_of(8);
                match (s < lines, wild) {
                    (true, false) => self.range(1, 4),
                    (true, true) => self.range(-2, 0),
                    (false, false) => self.range(-3, 0),
                    (false, true) => self.range(1, 2),
                }
            })
            .collect()
    }
    /// A key of the same support, total excess and numerator excess as `base` (one unit moved between two active
    /// or two inactive slots): such pairs are decided at the coordinate-tie level.
    fn tie_partner(&mut self, base: &[i32]) -> Vec<i32> {
        let mut k = base.to_vec();
        for _ in 0..50 {
            let i = (self.next() % k.len() as u64) as usize;
            let j = (self.next() % k.len() as u64) as usize;
            if i == j || (k[i] > 0) != (k[j] > 0) {
                continue;
            }
            if k[i] > 0 && k[j] > 1 {
                k[i] += 1;
                k[j] -= 1;
                return k;
            }
            if k[i] <= 0 && k[j] < 0 {
                k[i] -= 1;
                k[j] += 1;
                return k;
            }
        }
        k
    }
    /// A key near `base` (small changes), to make tie-level comparisons frequent.
    fn near(&mut self, base: &[i32]) -> Vec<i32> {
        let mut k = base.to_vec();
        for _ in 0..1 + self.next() % 3 {
            let s = (self.next() % k.len() as u64) as usize;
            k[s] += self.range(-1, 1);
        }
        k
    }
}

fn spired(n: usize, priority: Option<&[usize]>) -> NodeComparator {
    let mut d = OrderDescriptor::spired_uncut(n);
    if let Some(p) = priority {
        d.coordinate_priority = p.to_vec();
    }
    NodeComparator::from_descriptor(d).unwrap()
}

const ARITIES: [(usize, usize); 4] = [(5, 3), (9, 6), (14, 9), (20, 10)];

// ---------------------------------------------------------------- semantics ----

#[test]
fn order_direction_and_all_key_semantics() {
    for c in [NodeComparator::legacy(5, None).unwrap(), spired(5, None)] {
        // Less = a LOWER
        assert_eq!(c.cmp(&[1, 1, 1, 0, 0], &[2, 1, 1, 0, 0]), Ordering::Less);
        assert!(c.lower(&[1, 1, 1, 0, -1], &[1, 1, 1, 0, -2]));
        // a subsector key (removed line) is lower than any top-sector key, whatever its dots/rank
        assert!(c.lower(&[9, 9, 0, -7, -7], &[1, 1, 1, 0, 0]));
        assert!(c.lower(&[1, 1, -3, 0, 0], &[1, 1, 1, 0, 0]));
        // a positive numerator adds an active slot: HIGHER than the chart key with that numerator at 0
        assert!(c.lower(&[1, 1, 1, 0, 0], &[1, 1, 1, 0, 1]));
        assert!(c.lower(&[5, 5, 5, -4, -4], &[1, 1, 1, 1, 0]));
        // equal support count, different supports: first differing support slot (slot order) without the line is lower
        assert!(c.lower(&[0, 1, 1, 0, 0], &[1, 0, 1, 0, 0]));
        assert!(c.lower(&[1, 0, 1, 0, 0], &[1, 1, 0, 0, 0]));
        // same support: D + r first, then r
        assert!(c.lower(&[2, 1, 1, 0, 0], &[1, 1, 1, -1, -1]));
        assert!(c.lower(&[3, 1, 1, 0, 0], &[2, 1, 1, -1, 0]));
        // ties: larger line power on the first slot is lower; then larger |numerator| on the first numerator slot
        assert!(c.lower(&[2, 1, 1, 0, 0], &[1, 2, 1, 0, 0]));
        assert!(c.lower(&[1, 1, 1, -1, 0], &[1, 1, 1, 0, -1]));
        // irreflexive
        assert_eq!(c.cmp(&[1, 2, 3, -1, 0], &[1, 2, 3, -1, 0]), Ordering::Equal);
        // defined beyond the engine's compact range; check_engine_range reports it
        assert!(c.lower(&[1, 1, 1, 0, -1000], &[1, 1, 1, 0, -1001]));
        assert_eq!(
            c.check_engine_range(&[1, 1, 1, 0, -65]),
            Err(OrderError::OutsideEngineRange {
                slot: 4,
                value: -65
            })
        );
        assert!(c.check_engine_range(&[63, 1, 1, 0, -64]).is_ok());
        assert_eq!(
            c.try_cmp(&[1, 1], &[1, 1, 1, 0, 0]),
            Err(OrderError::Arity {
                expected: 5,
                actual: 2
            })
        );
    }
}

#[test]
#[should_panic(expected = "NodeComparator::cmp")]
fn order_cmp_panics_on_arity_mismatch() {
    NodeComparator::legacy(5, None)
        .unwrap()
        .cmp(&[1, 1, 1, 0], &[1, 1, 1, 0, 0]);
}

// ------------------------------------------------------- properties (O3 iii) ----

fn check_properties(c: &NodeComparator, rng: &mut Rng, n: usize, lines: usize, count: usize) {
    for _ in 0..count {
        let a = rng.key(n, lines);
        let b = match rng.next() % 3 {
            0 => rng.near(&a),
            1 => rng.tie_partner(&a),
            _ => rng.key(n, lines),
        };
        let x = match rng.next() % 3 {
            0 => rng.near(&b),
            1 => rng.tie_partner(&b),
            _ => rng.key(n, lines),
        };
        // irreflexivity and totality: Equal exactly on identical keys
        assert_eq!(c.cmp(&a, &a), Ordering::Equal);
        assert_eq!(c.cmp(&a, &b) == Ordering::Equal, a == b, "{a:?} {b:?}");
        // antisymmetry
        assert_eq!(c.cmp(&a, &b), c.cmp(&b, &a).reverse(), "{a:?} {b:?}");
        // transitivity
        if c.lower(&a, &b) && c.lower(&b, &x) {
            assert!(c.lower(&a, &x), "{a:?} < {b:?} < {x:?}");
        }
        if c.lower(&b, &a) && c.lower(&x, &b) {
            assert!(c.lower(&x, &a), "{x:?} < {b:?} < {a:?}");
        }
        // rank is a sort key of cmp
        assert_eq!(c.rank(&a).cmp(&c.rank(&b)), c.cmp(&a, &b), "{a:?} {b:?}");
    }
}

#[test]
fn order_properties_on_random_keys_arities_5_9_14_20() {
    let mut rng = Rng(0x5eed_0001);
    for (n, lines) in ARITIES {
        let p = rng.perm(n);
        let comparators = vec![
            NodeComparator::legacy(n, None).unwrap(),
            NodeComparator::legacy(n, Some(p.clone())).unwrap(),
            spired(n, None),
            spired(n, Some(&p)),
            NodeComparator::from_descriptor({
                let mut d =
                    OrderDescriptor::spired_uncut_two_sided((0..n).map(|s| s == 0).collect());
                d.coordinate_priority = p.clone();
                d
            })
            .unwrap(),
            NodeComparator::from_descriptor({
                let mut d = OrderDescriptor::spired_uncut(n);
                d.coordinate_groups = CoordinateGroups::Interleaved;
                d.active_direction = Direction::Ascending;
                d.support_priority = rng.perm(n);
                d.support_weights = (0..n as u64).collect();
                d
            })
            .unwrap(),
        ];
        for c in &comparators {
            check_properties(c, &mut rng, n, lines, 20_000);
        }
    }
}

// ------------------------------------------------------- differential (O3 iv) ----

/// The legacy port (index.rs:386-491) agrees with rustred-order's own `compare` of `spired_uncut` with the same
/// coordinate priority, on random numeric keys including subsector and positive-numerator keys.
#[test]
fn order_differential_legacy_port_vs_rustred_order_compare() {
    let mut rng = Rng(0x5eed_0002);
    let mut decided_by_ties = 0usize;
    for (n, lines) in ARITIES {
        for round in 0..4 {
            let p = if round == 0 {
                (0..n).collect()
            } else {
                rng.perm(n)
            };
            let legacy = NodeComparator::legacy(n, Some(p.clone())).unwrap();
            let mut d = OrderDescriptor::spired_uncut(n);
            d.coordinate_priority = p.clone();
            let program = CompiledOrder::compile(d, Limits::default()).unwrap();
            for _ in 0..25_000 {
                let a = rng.key(n, lines);
                let b = match rng.next() % 3 {
                    0 => rng.near(&a),
                    1 => rng.tie_partner(&a),
                    _ => rng.key(n, lines),
                };
                let a64: Vec<i64> = a.iter().map(|&x| x.into()).collect();
                let b64: Vec<i64> = b.iter().map(|&x| x.into()).collect();
                let reference = program.compare(&a64, &b64).unwrap();
                if matches!(
                    reference.component,
                    Some(rustred_order::Component::Coordinate(_))
                ) {
                    decided_by_ties += 1;
                }
                assert_eq!(
                    legacy.cmp(&a, &b),
                    reference.ordering,
                    "n={n} p={p:?} {a:?} {b:?}"
                );
            }
        }
    }
    assert!(
        decided_by_ties > 50_000,
        "the sample must exercise the tie level: {decided_by_ties}"
    );
}

/// The program path of NodeComparator is rustred-order itself; its rank agrees with compare (checked above) and the
/// legacy pattern port on all-fixed patterns agrees with the concrete legacy path.
#[test]
fn order_differential_pattern_paths_on_fixed_patterns() {
    let mut rng = Rng(0x5eed_0003);
    for (n, lines) in ARITIES {
        let p = rng.perm(n);
        for c in [
            NodeComparator::legacy(n, Some(p.clone())).unwrap(),
            spired(n, Some(&p)),
        ] {
            let chart: Vec<bool> = (0..n).map(|s| s < lines).collect();
            for _ in 0..5_000 {
                let a = rng.key(n, lines);
                let b = rng.near(&a);
                let pa: Vec<PatternSlot> = a.iter().map(|&x| PatternSlot::Fixed(x)).collect();
                let pb: Vec<PatternSlot> = b.iter().map(|&x| PatternSlot::Fixed(x)).collect();
                assert_eq!(c.cmp_pattern(&chart, &pa, &pb).unwrap(), c.cmp(&a, &b));
            }
        }
    }
}

// ------------------------------------------------------------- presentation ----

const T162_PERM: [usize; 20] = [
    3, 1, 2, 0, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];

#[test]
fn order_presentation_t162_phys_swap_0_3() {
    let p = Presentation::from_new_to_old(&T162_PERM).unwrap();
    assert!(!p.is_identity());
    let original: Vec<i32> = (0..20).map(|i| i * 10 - 50).collect();
    let presented = p.present(&original);
    assert_eq!(presented[0], original[3]);
    assert_eq!(presented[3], original[0]);
    assert_eq!(&presented[4..], &original[4..]);
    assert_eq!(p.original(&presented), original);
    assert_eq!(p.old_to_new(), &T162_PERM); // an involution
    // a non-involutive permutation round-trips too
    let q = Presentation::from_new_to_old(&[1, 2, 0, 3]).unwrap();
    assert_eq!(q.present(&[10, 20, 30, 40]), vec![20, 30, 10, 40]);
    assert_eq!(q.original(&[20, 30, 10, 40]), vec![10, 20, 30, 40]);
    assert!(Presentation::from_new_to_old(&[0, 0, 1]).is_err());
}

#[test]
fn order_t162_header_selects_legacy_in_presented_frame_rotation_irrelevant() {
    let headers: serde_json::Value =
        serde_json::from_str(include_str!("data/order/headers.json")).unwrap();
    let h = &headers["t162_bank"];
    let c = NodeComparator::from_header(h).unwrap();
    assert_eq!(c.selection(), &Selection::NodeOrder);
    assert_eq!(c.name(), "phys_swap_0_3_rot7");
    assert!(matches!(
        c.engine(),
        rustred_campaign_core::order::Engine::Legacy { ties: None }
    ));
    assert_eq!(c.presentation().new_to_old(), &T162_PERM);
    assert_eq!(c.source_order(), Some(&SourceOrder::Rotate(7)));
    // the same header with source order Reverse (46 of the 60 bank origins) gives the same comparator and frame
    let mut h2 = h.clone();
    h2["node_order"]["source_order"] = serde_json::json!("Reverse");
    let c2 = NodeComparator::from_header(&h2).unwrap();
    assert_eq!(c2.presentation(), c.presentation());
    let mut rng = Rng(7);
    for _ in 0..2000 {
        let a = rng.key(20, 9);
        let b = rng.near(&a);
        assert_eq!(c.cmp(&a, &b), c2.cmp(&a, &b));
        // cmp_original = cmp on presented keys
        let p = c.presentation();
        assert_eq!(
            c.cmp_original(&p.original(&a), &p.original(&b)),
            c.cmp(&a, &b)
        );
    }
    // the presentation matters: the swap of slots 0 and 3 changes the tie verdict
    let a = [2, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let b = [1, 1, 1, 2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert!(c.lower(&a, &b));
    // read as ORIGINAL-frame keys, a has its dot on presented slot 3 and b on presented slot 0
    assert_eq!(c.cmp_original(&a, &b), Ordering::Greater);
}

// --------------------------------------------------------- header selection ----

#[test]
fn order_header_selection_reproduces_descent_rs() {
    let headers: serde_json::Value =
        serde_json::from_str(include_str!("data/order/headers.json")).unwrap();
    let prio = [
        6usize, 4, 9, 8, 5, 2, 1, 0, 7, 3, 11, 12, 10, 13, 15, 14, 16, 17, 18, 19,
    ];
    // T178 order B table: node_order with coordinate_priority -> spired_uncut program with that priority
    let b = NodeComparator::from_header(&headers["t178B_table"]).unwrap();
    assert_eq!(b.selection(), &Selection::NodeOrder);
    assert_eq!(b.equivalent_descriptor().coordinate_priority, prio.to_vec());
    assert!(matches!(
        b.engine(),
        rustred_campaign_core::order::Engine::Program(_)
    ));
    // the helper's wrapped keys header has no order_is_program: the ENGINE reads it as the legacy identity order
    // (it also lacks `denominators`, which the engine requires)
    assert_eq!(
        NodeComparator::from_header(&headers["helper_keys_t178B"]).unwrap_err(),
        OrderError::Header("header denominators".into())
    );
    let mut wrapped = headers["helper_keys_t178B"].clone();
    wrapped["header"]["denominators"] = 20.into();
    let w = NodeComparator::from_header(&wrapped).unwrap();
    assert_eq!(w.selection(), &Selection::LegacyNoProgram);
    // the same with order_is_program = true and no node_order: coordinate_priority ONLY (support_priority ignored)
    let mut h = headers["helper_keys_t178B"]["header"].clone();
    h["denominators"] = 20.into();
    h["order_is_program"] = true.into();
    h["order"]["support_priority"] = serde_json::json!((0..20).rev().collect::<Vec<usize>>());
    let p = NodeComparator::from_header(&h).unwrap();
    assert_eq!(p.selection(), &Selection::ProgramCoordinatePriority);
    assert_eq!(p.name(), "program");
    let d = p.equivalent_descriptor();
    assert_eq!(d.coordinate_priority, prio.to_vec());
    assert_eq!(
        d.support_priority,
        (0..20).collect::<Vec<_>>(),
        "support_priority is not read (descent.rs:155-175)"
    );
    // a missing coordinate_priority is an error
    let mut h3 = h.clone();
    h3["order"]
        .as_object_mut()
        .unwrap()
        .remove("coordinate_priority");
    assert!(matches!(
        NodeComparator::from_header(&h3),
        Err(OrderError::Header(_))
    ));
    // node_order: null is present and fails to deserialise, as in the engine
    let mut h4 = headers["t162_bank"].clone();
    h4["node_order"] = serde_json::Value::Null;
    assert!(matches!(
        NodeComparator::from_header(&h4),
        Err(OrderError::Header(_))
    ));
    // denominators is required
    let mut h5 = headers["t162_bank"].clone();
    h5.as_object_mut().unwrap().remove("denominators");
    assert!(NodeComparator::from_header(&h5).is_err());
    // node_order with a bad permutation is rejected (NodeOrder::validate)
    let mut h6 = headers["t162_bank"].clone();
    h6["node_order"]["slot_permutation"] = serde_json::json!([0, 0, 1]);
    assert!(matches!(
        NodeComparator::from_header(&h6),
        Err(OrderError::Permutation(_))
    ));
}

/// descent.rs test `two_sided_eps_axes_are_checked_with_the_full_descriptor`, concretely: with a FULL header
/// descriptor (degree_rows present, no node_order, order_is_program) the eps axis is two-sided.
#[test]
fn order_full_header_descriptor_two_sided() {
    let mut d = OrderDescriptor::spired_uncut_two_sided(vec![true, false, false, false, false]);
    d.coordinate_priority = (0..5).collect();
    let rows = |rows: &[rustred_order::DegreeRow]| {
        rows.iter()
            .map(|r| serde_json::json!({"active": r.active, "inactive": r.inactive}))
            .collect::<Vec<_>>()
    };
    let header = serde_json::json!({
        "type": "header", "denominators": 5, "order_is_program": true, "order_name": "eps",
        "order": {
            "pre_support_degree_rows": rows(&d.pre_support_degree_rows),
            "support_weights": d.support_weights, "support_priority": d.support_priority,
            "degree_rows": rows(&d.degree_rows), "coordinate_priority": d.coordinate_priority,
            "coordinate_groups": "ActiveFirst", "active_direction": "Descending", "inactive_direction": "Descending",
            "two_sided": [1, 0, 0, 0, 0]
        }
    });
    let c = NodeComparator::from_header(&header).unwrap();
    assert_eq!(c.selection(), &Selection::FullHeaderDescriptor);
    assert_eq!(c.name(), "eps (full header descriptor)");
    assert_eq!(c.equivalent_descriptor(), d);
    // I(1, ..) is lower than I(0, ..) on the eps axis (n = 1 is the unique minimum), I(-1, ..) higher than I(2, ..)
    assert!(c.lower(&[1, 1, 1, 0, 0], &[0, 1, 1, 0, 0]));
    assert!(c.lower(&[2, 1, 1, 0, 0], &[-1, 1, 1, 0, 0]));
    // the one-sided legacy order disagrees on the first pair
    assert!(
        !NodeComparator::legacy(5, None)
            .unwrap()
            .lower(&[1, 1, 1, 0, 0], &[0, 1, 1, 0, 0])
    );
    // the compact form (no degree_rows, no support_weights) and the slot map of permuted_order
    let compact = serde_json::json!({"support_priority": [0, 1, 2], "coordinate_priority": [2, 0, 1], "two_sided": []});
    let dc = descriptor_from_order_json(&compact, &[1, 2, 0]).unwrap();
    assert_eq!(dc.coordinate_priority, vec![0, 1, 2]);
    assert_eq!(dc.support_priority, vec![1, 2, 0]);
    assert!(descriptor_from_order_json(&serde_json::json!({"degree_rows": []}), &[0, 1]).is_err());
}

// ----------------------------------------------------------------- patterns ----

fn t212_rules(set: &serde_json::Value) -> Vec<(Vec<PatternSlot>, Vec<Vec<PatternSlot>>)> {
    set.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                parse_pattern(r["target"].as_str().unwrap()).unwrap(),
                r["rhs_keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| parse_pattern(k.as_str().unwrap()).unwrap())
                    .collect(),
            )
        })
        .collect()
}

fn not_lower(
    c: &NodeComparator,
    rules: &[(Vec<PatternSlot>, Vec<Vec<PatternSlot>>)],
) -> (usize, usize) {
    let chart: Vec<bool> = "11111111111100000000".chars().map(|x| x == '1').collect();
    let mut terms = 0;
    let mut bad = 0;
    for (target, rhs) in rules {
        for k in rhs {
            terms += 1;
            if c.cmp_pattern(&chart, k, target).unwrap() != Ordering::Less {
                bad += 1;
            }
        }
    }
    (terms, bad)
}

/// The engine's own regression (descent.rs tests `each_t212_set_descends_under_its_own_order`,
/// `t212_reciprocal_two_cycle_mixing_orders_is_rejected_under_every_one_order`) on its fixture, symbolic patterns.
#[test]
fn order_t212_engine_regression_on_patterns() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("data/order/t212_mixed_orders.json")).unwrap();
    let v1 = t212_rules(&f["sets"]["v1_identity"]);
    let v3 = t212_rules(&f["sets"]["v3_tie_isp_rev"]);
    let id = NodeComparator::from_node_order(&NodeOrderRecord::identity(), 20).unwrap();
    let rev = NodeComparator::from_node_order(
        &NodeOrderRecord {
            name: "tie_isp_rev".into(),
            coordinate_priority: Some((0..12).chain((12..20).rev()).collect()),
            ..NodeOrderRecord::identity()
        },
        20,
    )
    .unwrap();
    assert_eq!(not_lower(&id, &v1), (234, 0));
    assert_eq!(not_lower(&rev, &v3), (147 + 255 + 172, 0));
    assert_eq!(not_lower(&id, &v3).1, 11);
    assert_eq!(not_lower(&rev, &v1).1, 11);
    // the legacy path with the reversed tie permutation gives the same verdicts as the program
    let rev_legacy =
        NodeComparator::legacy(20, Some((0..12).chain((12..20).rev()).collect())).unwrap();
    assert_eq!(not_lower(&rev_legacy, &v3), (574, 0));
    assert_eq!(not_lower(&rev_legacy, &v1).1, 11);
    // different symbolic patterns: explicit error where the engine asserts
    let chart = vec![true; 20];
    let a = parse_pattern("I(n0,1,1,1,1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0)").unwrap();
    let b = parse_pattern("I(2,1,1,1,1,1,1,1,1,1,1,1,0,0,0,0,0,0,0,0)").unwrap();
    assert_eq!(
        id.cmp_pattern(&chart, &a, &b),
        Err(OrderError::PatternMismatch { slot: 0 })
    );
    assert_eq!(
        rev.cmp_pattern(&chart, &a, &b),
        Err(OrderError::PatternMismatch { slot: 0 })
    );
}

// ------------------------------------------------- automorphism invariance (O4) ----

#[test]
fn order_automorphism_invariance_synthetic_group() {
    // 3 lines + 2 numerators; group generated by the line swap 0 <-> 1 (numerators fixed)
    let c = NodeComparator::legacy(5, None).unwrap();
    let id = GroupElement {
        lines: vec![0, 1, 2],
        numerators: Some(vec![0, 1]),
    };
    let swap = GroupElement {
        lines: vec![1, 0, 2],
        numerators: Some(vec![0, 1]),
    };
    let line_only = GroupElement {
        lines: vec![1, 0, 2],
        numerators: None,
    };
    let mut rng = Rng(11);
    let pairs: Vec<(Vec<i32>, Vec<i32>)> = (0..2000)
        .map(|_| {
            let a = rng.key(5, 3);
            let b = rng.near(&a);
            (a, b)
        })
        .collect();
    let r = automorphism_invariance(&c, std::slice::from_ref(&id), &pairs).unwrap();
    assert!(r.invariant_on_sample());
    assert!(r.elements[0].descriptor_invariant);
    let r = automorphism_invariance(&c, &[id.clone(), swap.clone()], &pairs).unwrap();
    assert!(!r.invariant_on_sample());
    assert!(!r.elements[1].descriptor_invariant);
    assert_eq!(r.elements[1].pairs_tested, 2000);
    assert!(r.elements[1].pairs_disagreeing > 0);
    // the canonical counterexample: dot on line 0 vs dot on line 1
    let ab = vec![(vec![2, 1, 1, 0, 0], vec![1, 2, 1, 0, 0])];
    let r = automorphism_invariance(&c, std::slice::from_ref(&swap), &ab).unwrap();
    assert_eq!(r.elements[0].pairs_disagreeing, 1);
    // a swap of lines that only ever differ at the aggregate level never disagrees: pairs at different (D+r)
    let agg = vec![
        (vec![2, 1, 1, 0, 0], vec![2, 2, 1, 0, 0]),
        (vec![1, 1, 0, 0, 0], vec![1, 1, 1, 0, -1]),
    ];
    assert!(
        automorphism_invariance(&c, std::slice::from_ref(&swap), &agg)
            .unwrap()
            .invariant_on_sample()
    );
    // without a numerator map, pairs with a non-zero numerator are skipped
    let r = automorphism_invariance(&c, &[line_only], &pairs).unwrap();
    assert_eq!(
        r.elements[0].pairs_tested + r.elements[0].pairs_skipped,
        2000
    );
    assert!(r.elements[0].pairs_skipped > 0);
    // a priority with line 2 first is invariant under the swap on pairs that differ only on line 2 / aggregates,
    // but the swap still exchanges the tie order of lines 0 and 1: both priorities disagree on the swap pair
    let c2 = NodeComparator::legacy(5, Some(vec![2, 0, 1, 3, 4])).unwrap();
    assert_eq!(
        automorphism_invariance(&c2, &[swap], &ab)
            .unwrap()
            .total_disagreeing(),
        1
    );
}
