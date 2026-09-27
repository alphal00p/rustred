//! The typed record lines against the former `json!` records. Every commit
//! of the whole suite runs the same comparison (`legacy::check_native` and
//! `legacy::check_delegated`) on the records its walks publish; these tests
//! add every combination of the conditional keys and adversarial values.
use super::super::super::{mask, power_bounds_json, route_stats, stats_json};
use super::legacy::{self, seen};
use super::*;
use rustred::solver::OwnerDomainMatchStats;
use serde_json::json;

fn line(value: &impl Serialize) -> String {
    String::from_utf8(serde_json::to_vec(value).unwrap()).unwrap()
}

/// Every field distinct, so a key bound to the wrong field cannot pass.
fn applied(base: usize) -> OwnerAppliedStats {
    OwnerAppliedStats {
        matching: OwnerDomainMatchStats {
            rules: base + 1,
            terminal_checks: base + 2,
            predicates: base + 3,
            pieces: base + 4,
            cells: base + 5,
            split_operations: base + 6,
            coordinate_cells: base + 7,
            rank_empty_cells: base + 8,
            correlation_empty_cells: base + 9,
            refinement_cells: base + 10,
            refinement_steps: base + 11,
        },
        selected_pieces: base + 12,
        term_visits: base + 13,
        shift_groups: base + 14,
        boundary_cells: base + 15,
        application_refinement_steps: base + 16,
        application_refinement_cells: base + 17,
        sign_splits: base + 18,
        native_operations: base + 19,
        optional_coefficient_refusals: base + 20,
        optional_original_refusals: base + 21,
        optional_coalesced_refusals: base + 22,
        coalescing_additions: base + 23,
        events: base + 24,
        successors: base + 25,
        conditional_successors: base + 26,
        same_support_successors: base + 27,
        strict_subsupport_successors: base + 28,
        unsupported_support_successors: base + 29,
        conditional_unsupported_support_successors: base + 30,
        problems: base + 31,
        zero_terms: base + 32,
        cancelled_groups: base + 33,
        zero_sector_groups: base + 34,
        correlation_empty_cells: base + 35,
    }
}

fn route(base: usize) -> CandidateDomainRouteStats {
    CandidateDomainRouteStats {
        masks_examined: base + 1,
        masks_pruned: base + 2,
        joint_support_masks_pruned: base + 3,
        events: base + 4,
        apply_domains: base + 5,
        route_domains: base + 6,
        zero_sectors: base + 7,
        missing_routes: base + 8,
        coordinate_cells: base + 9,
    }
}

fn powers() -> [DomainPowerBounds; 3] {
    [
        DomainPowerBounds::default(),
        DomainPowerBounds {
            max_positive_power: Some(u64::MAX),
            min_power_difference: Some(i64::MIN),
            max_power_difference: Some(-3),
        },
        DomainPowerBounds {
            max_positive_power: Some(0),
            min_power_difference: Some(3),
            max_power_difference: None,
        },
    ]
}

fn domains() -> Vec<Domain<3>> {
    let [open, negative, positive] = powers();
    vec![
        Domain {
            phase: Phase::Apply,
            owner: [true, false, true],
            lower: vec![0, 7, u64::MAX],
            upper: vec![None, Some(9), Some(u64::MAX)],
            rank: Some(11),
            powers: negative,
        },
        Domain {
            phase: Phase::Route,
            owner: [false, false, false],
            lower: vec![],
            upper: vec![],
            rank: None,
            powers: open,
        },
        Domain {
            phase: Phase::Apply,
            owner: [true, true, true],
            lower: vec![1, 2, 3],
            upper: vec![Some(1), None, None],
            rank: Some(u32::MAX),
            powers: positive,
        },
    ]
}

#[test]
fn key_order_check_is_string_order() {
    assert!(sorted_keys(&["a"]));
    assert!(sorted_keys(&["a", "a_b", "ab", "b"]));
    assert!(sorted_keys(&["stats", "stats_scope"]));
    assert!(!sorted_keys(&["b", "a"]));
    assert!(!sorted_keys(&["a", "a"]));
    assert!(!sorted_keys(&["ab", "a_b"]));
    assert!(!sorted_keys(&["stats_scope", "stats"]));
}

#[test]
fn nested_objects_equal_the_json_helpers() {
    for base in [0, 1_000, usize::MAX - 40] {
        let stats = applied(base);
        assert_eq!(line(&AppliedStats(stats)), line(&stats_json(stats)));
        assert_eq!(
            serde_json::to_value(AppliedStats(stats)).unwrap(),
            stats_json(stats)
        );
        let stats = route(base);
        assert_eq!(line(&RouteStats(stats)), line(&route_stats(stats)));
        assert_eq!(
            serde_json::to_value(RouteStats(stats)).unwrap(),
            route_stats(stats)
        );
    }
    for p in powers() {
        assert_eq!(line(&PowerBounds(p)), line(&power_bounds_json(p)));
    }
    for domain in domains() {
        assert_eq!(line(&Mask(&domain.owner)), line(&mask(&domain.owner)));
        assert_eq!(
            line(&PhaseName(domain.phase)),
            line(&format!("{:?}", domain.phase))
        );
        assert_eq!(
            serde_json::to_value(Mask(&domain.owner)).unwrap(),
            json!(mask(&domain.owner))
        );
    }
}

fn refusal(n: usize) -> Value {
    json!({"phase":"original","selected_rule":format!("SelectedRule {{ batch: {n}, rule: 1 }}"),
        "source_lower":[0, n], "source_upper":[null, n], "rank":null, "shift":[-1, 2],
        "resource":"guard \"factor\"\nvariables\u{1F600}", "requested":n, "limit":1.5})
}

fn frontier(n: usize) -> Value {
    json!({"kind":"missing_route_cover", "domain":{"lower":[n], "upper":[null]},
        "reached_missing_rule_claim":false, "note":"tab\there \u{7}", "weight":-0.0,
        "nested":[[], {}, [null, true, 1e300]]})
}

fn part(part: u8, refusals: usize, truncated: bool) -> Value {
    json!({"part":part, "seconds":0.25, "error":null, "error_kind":"none",
        "optional_refusals":(0..refusals).map(refusal).collect::<Vec<_>>(),
        "optional_refusal_provenance_truncated":truncated,
        "frontier_count":1, "frontiers_published_on_parent":true})
}

fn stats_variants() -> Vec<NativeStats> {
    let scope = InitialOverlapScope {
        anchor_id: 5,
        cut: -3,
        residual_powers: powers()[1],
    };
    vec![
        NativeStats::Apply(applied(0)),
        NativeStats::Apply(OwnerAppliedStats::default()),
        NativeStats::ApplyPartial(applied(7), scope),
        NativeStats::ApplyPartial(
            applied(1),
            InitialOverlapScope {
                anchor_id: 0,
                cut: i64::MAX,
                residual_powers: DomainPowerBounds::default(),
            },
        ),
        NativeStats::Route(route(3)),
    ]
}

fn refusal_variants(stats: NativeStats) -> Vec<Option<OptionalRefusals>> {
    if matches!(stats, NativeStats::Route(_)) {
        return vec![None];
    }
    let mut two = OptionalRefusals::default();
    two.records = vec![refusal(1), refusal(2)];
    let mut one = OptionalRefusals::default();
    one.records = vec![refusal(3)];
    vec![Some(OptionalRefusals::default()), Some(one), Some(two)]
}

fn parts_variants(stats: NativeStats) -> Vec<Option<Vec<Value>>> {
    if !matches!(stats, NativeStats::Apply(_)) {
        return vec![None];
    }
    vec![
        None,
        Some(vec![part(0, 0, false)]),
        Some(vec![part(0, 2, false), part(1, 1, true)]),
        Some(vec![part(1, 0, false)]),
        Some(Vec::new()),
    ]
}

/// Every combination of the conditional keys, through both paths.
#[test]
fn native_records_equal_the_former_json_records() {
    seen::take();
    let mut checked = 0;
    for domain in domains() {
        for stats in stats_variants() {
            for refusals in refusal_variants(stats) {
                for physical_parts in parts_variants(stats) {
                    for (error, error_free) in [
                        (None, true),
                        (None, false),
                        (Some("cancelled".to_owned()), false),
                        (
                            Some("quote \" backslash \\ newline \n nul \0 é".to_owned()),
                            false,
                        ),
                    ] {
                        for delegation in [false, true] {
                            for accepted_events in [None, Some(0), Some(usize::MAX)] {
                                for frontiers in [Vec::new(), vec![frontier(1), frontier(2)]] {
                                    for seconds in
                                        [0.0, -0.0, 0.125, 1e-7, 3.0e20, f64::NAN, f64::INFINITY]
                                    {
                                        legacy::check_native(&NativeInputs {
                                            id: checked,
                                            domain: domain.clone(),
                                            error: error.clone(),
                                            error_free,
                                            stats,
                                            seconds,
                                            accepted_events,
                                            frontiers: frontiers.clone(),
                                            delegation,
                                            refusals: refusals.clone(),
                                            physical_parts: physical_parts.clone(),
                                        });
                                        checked += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 30_000, "{checked}");
    let all = seen::NATIVE
        | seen::ROUTE
        | seen::PARTIAL
        | seen::SUBDIVIDED
        | seen::FRONTIERS
        | seen::REFUSALS
        | seen::ACCEPTED_EVENTS
        | seen::ERROR
        | seen::DELEGATION_LEDGER;
    assert_eq!(seen::take(), all);
}

#[test]
fn delegated_records_equal_the_former_json_records() {
    for (id, domain) in domains().into_iter().enumerate() {
        for representative in [0, id + 1, usize::MAX] {
            legacy::check_delegated(&DelegatedRecord {
                id,
                domain: domain.clone(),
                representative,
            });
        }
    }
}

/// The Memory sink's `Value` moves the retained arrays instead of copying
/// them, and still equals the parsed line.
#[test]
fn memory_values_equal_parsed_lines() {
    let mut refusals = OptionalRefusals::default();
    refusals.records = vec![refusal(1)];
    let record = native(NativeInputs {
        id: 3,
        domain: domains().remove(0),
        error: None,
        error_free: true,
        stats: NativeStats::Apply(applied(4)),
        seconds: 0.5,
        accepted_events: Some(2),
        frontiers: vec![frontier(1)],
        delegation: true,
        refusals: Some(refusals),
        physical_parts: Some(vec![part(0, 1, false), part(1, 0, false)]),
    });
    let parsed: Value = serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
    let value = record.into_value().unwrap();
    assert_eq!(value, parsed);
    assert_eq!(value["frontiers"], json!([frontier(1)]));
    assert_eq!(value["optional_refusals"], json!([refusal(1)]));
    assert_eq!(value["physical_parts"][0]["part"], 0);
    assert_eq!(value["seconds"], Value::Null);
    assert_eq!(value["physical_seconds_sum"], 0.5);
}
