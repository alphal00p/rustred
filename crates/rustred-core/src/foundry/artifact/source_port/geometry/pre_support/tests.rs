use super::*;
use crate::order::{CompiledOrder, CoordinateGroups, Direction, OrderDescriptor};

fn programmed(arity: usize, rows: Vec<DegreeRow>) -> OrderingPolicy {
    OrderingPolicy::try_programmed(
        CompiledOrder::compile(
            OrderDescriptor {
                pre_support_degree_rows: rows,
                support_weights: vec![0; arity],
                support_priority: (0..arity).collect(),
                degree_rows: vec![DegreeRow {
                    active: vec![1; arity],
                    inactive: vec![1; arity],
                }],
                coordinate_priority: (0..arity).collect(),
                coordinate_groups: CoordinateGroups::ActiveFirst,
                active_direction: Direction::Descending,
                inactive_direction: Direction::Descending,
            },
            Default::default(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn global_f(arity: usize) -> OrderingPolicy {
    programmed(
        arity,
        vec![DegreeRow {
            active: vec![1; arity],
            inactive: vec![1; arity],
        }],
    )
}

fn point(n: &[i64]) -> (Vec<bool>, LatticeBox) {
    let sector: Vec<_> = n.iter().map(|&p| p > 0).collect();
    let local: Vec<_> = n
        .iter()
        .map(|&p| {
            if p > 0 {
                (p - 1) as u64
            } else {
                p.unsigned_abs()
            }
        })
        .collect();
    let cell = LatticeBox::try_new(local.clone(), local.into_iter().map(Some)).unwrap();
    (sector, cell)
}

fn prove(
    ordering: OrderingPolicy,
    source: &[bool],
    cell: &LatticeBox,
    shift: &[i64],
    vanishes: bool,
) -> Result<(), SourcePortAuditError> {
    super::super::prove_wide_descent_with_limits(
        [(shift, &())],
        std::slice::from_ref(cell),
        source,
        ordering,
        Default::default(),
        |_, _| Ok(vanishes),
    )
}

#[test]
fn saved_owner481_pinch_cannot_bypass_global_f() {
    // Actual saved rule151 has a uniform 1/6 term with this shift: F7 -> F9.
    let parent = [0, 1, 1, 2, 2, 0, 0, 0, 0, 1];
    let child = [-2, 3, 0, 2, 1, 0, 0, 0, 0, 1];
    let shift: Vec<_> = child.iter().zip(parent).map(|(c, p)| c - p).collect();
    let (sector, cell) = point(&parent);
    prove(OrderingPolicy::SpiredUncutV1, &sector, &cell, &shift, false).unwrap();
    prove(programmed(10, vec![]), &sector, &cell, &shift, false).unwrap();
    assert!(prove(global_f(10), &sector, &cell, &shift, false).is_err());
    // The existing authenticated zero/excluded-domain callback remains the
    // only escape for a term whose descent is not established.
    prove(global_f(10), &sector, &cell, &shift, true).unwrap();
}

#[test]
fn physical_absolute_degree_precedes_support_but_support_breaks_equal_grade() {
    for (parent, shift, accepted) in [
        ([-3, 1], [4, 0], true),  // F4 -> F2 despite more active denominators.
        ([2, 1], [-2, 2], true),  // F3 tie, strict pinch is lower.
        ([0, 3], [1, -1], false), // F3 tie, activation is higher.
        ([0, 2], [2, -2], false), // Equal F and support count, higher support lex.
        ([2, 0], [-2, 2], true),  // Equal F and support count, lower support lex.
        ([2, -1], [-1, 0], true), // Same-support degree descent.
        ([2, -1], [1, 0], false), // Same-support degree growth.
    ] {
        let (sector, cell) = point(&parent);
        assert_eq!(
            prove(global_f(2), &sector, &cell, &shift, false).is_ok(),
            accepted
        );
    }
}

#[test]
fn exact_points_agree_with_concrete_program_for_weighted_prefixes() {
    let policies = [
        global_f(2),
        programmed(
            2,
            vec![
                DegreeRow {
                    active: vec![2, 1],
                    inactive: vec![1, 3],
                },
                DegreeRow {
                    active: vec![0, 3],
                    inactive: vec![2, 0],
                },
            ],
        ),
    ];
    for policy in policies {
        for p0 in -2..=2 {
            for p1 in -2..=2 {
                let parent = [p0, p1];
                let (sector, cell) = point(&parent);
                for c0 in -2..=2 {
                    for c1 in -2..=2 {
                        let child = [c0, c1];
                        let shift = [c0 - p0, c1 - p1];
                        let expected = policy.compare(&child, &parent).unwrap() == Ordering::Less;
                        assert_eq!(
                            prove(policy.clone(), &sector, &cell, &shift, false).is_ok(),
                            expected,
                            "parent={parent:?}, child={child:?}",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn crossing_interval_is_checked_whole_and_never_truncated_to_a_good_point() {
    // n=1..3 -> n-4=-3..-1. F grows at n=1, ties at n=2, drops at n=3.
    let full = LatticeBox::try_new([0], [Some(2)]).unwrap();
    assert!(prove(global_f(1), &[true], &full, &[-4], false).is_err());
    let safe = LatticeBox::try_new([1], [Some(2)]).unwrap();
    prove(global_f(1), &[true], &safe, &[-4], false).unwrap();
}

#[test]
fn weighted_multiple_prefix_rows_and_unbounded_unchanged_axes_are_respected() {
    let ordering = programmed(
        2,
        vec![
            DegreeRow {
                active: vec![1, 0],
                inactive: vec![1, 0],
            },
            DegreeRow {
                active: vec![0, 1],
                inactive: vec![0, 2],
            },
        ],
    );
    let (sector, cell) = point(&[1, 1]);
    // First grade ties under the first pinch; second grade grows and must
    // defeat the otherwise lower support count.
    assert!(prove(ordering, &sector, &cell, &[-2, -3], false).is_err());
    let ray = LatticeBox::try_new([1, 0], [Some(1), None]).unwrap();
    prove(global_f(2), &[true, true], &ray, &[-3, 0], false).unwrap();
}
