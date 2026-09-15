//! Complete canonical-fixture regression for supplied-domain descent.
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::sector::{InteriorBounds, SectorMonotoneDomain, zero};
use crate::solver::{SectorConfig, SectorSolveOptions, SectorSolver, SourceSystem};

use super::super::SourcePortAudit;
use super::*;

fn runtime_piece(piece: &LatticeBox, sector: &[bool; 6], shift: &[i64; 6]) -> SectorMonotoneDomain {
    let mask = Mask::try_new(*sector).unwrap();
    let representable =
        SectorMonotoneDomain::try_maximal_for_rule(mask.clone(), &[0; 6], &[shift]).unwrap();
    let bounds = (0..6).map(|axis| {
        let lo = i128::from(piece.lower()[axis]);
        let hi = piece.upper()[axis].map(i128::from);
        let (lo, hi) = if sector[axis] {
            (lo + 1, hi.map(|value| value + 1))
        } else {
            (
                hi.map(|value| -value).unwrap_or(i128::from(i64::MIN)),
                Some(-lo),
            )
        };
        let carrier = representable.bounds()[axis];
        InteriorBounds::new(
            i64::try_from(lo.max(i128::from(carrier.lower()))).unwrap(),
            i64::try_from(
                hi.unwrap_or(i128::from(i64::MAX))
                    .min(i128::from(carrier.upper())),
            )
            .unwrap(),
        )
    });
    SectorMonotoneDomain::try_new_for_rule(mask, bounds, &[0; 6], &[shift]).unwrap()
}

#[test]
#[ignore = "full canonical K6 supplied-domain descent grounding"]
fn canonical_k6_supplied_domain_descent_grounding() {
    let family = crate::foundry::artifact::three_loop::canonical_family().unwrap();
    let analyzer = zero::Analyzer::try_unrestricted(&family).unwrap();
    let mut zeros = Vec::new();
    let mut nonzeros = Vec::new();
    for bits in 0u64..64 {
        let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        match analyzer.analyze(&Mask::try_new(sector).unwrap()).unwrap() {
            zero::Decision::ProvedZero(_) => zeros.push(sector),
            _ => nonzeros.push(sector),
        }
    }
    assert_eq!((zeros.len(), nonzeros.len()), (26, 38));
    let zeros: Arc<[[bool; 6]]> = zeros.into();
    let sources = SourceSystem::from_family(&family).unwrap();
    let audit = SourcePortAudit::try_new(&family, zeros.clone()).unwrap();
    let mut counts: BTreeMap<&str, usize> = [
        "surviving_activation_or_swap",
        "existing_descent_failures",
        "coefficient_dead_sign_cells",
        "zero_sector_sign_cells",
    ]
    .into_iter()
    .map(|key| (key, 0))
    .collect();
    let mut activation_terms = BTreeSet::new();
    let mut failed_terms = BTreeSet::new();
    let mut examples = 0;
    for sector in nonzeros {
        let solution = SectorSolver::new(
            &sources,
            sector,
            SectorConfig {
                zero_sectors: zeros.clone(),
                ..Default::default()
            },
        )
        .unwrap()
        .solve_sector(SectorSolveOptions::default())
        .unwrap();
        let checked = audit.audit_sector(sector, None, &solution).unwrap();
        assert!(
            checked.issues.is_empty(),
            "{sector:?}: {:?}",
            checked.issues
        );
        assert_eq!(checked.exact_replayed_rules, solution.rules.len());
        assert_eq!(checked.uniformly_descending_rules, solution.rules.len());
        assert_eq!(checked.checked_rule_uncovered_boxes, 0);
        assert_eq!(
            checked.additional_replay_guard_branches, 0,
            "diagnostic must use the complete authenticated guard domain"
        );
        *counts.entry("sectors").or_default() += 1;
        *counts.entry("rules").or_default() += solution.rules.len();
        for (ordinal, rule) in solution.rules.iter().enumerate() {
            let boxes = application_boxes(rule, sources.index_variables(), &sector, &[]).unwrap();
            *counts.entry("rule_application_boxes").or_default() += boxes.len();
            for (term_ordinal, term) in rule.candidate.rhs.iter().enumerate() {
                let shift: [i64; 6] = std::array::from_fn(|axis| {
                    i64::from(term.integral[axis].value())
                        - i64::from(rule.candidate.target[axis].value())
                });
                *counts.entry("rhs_terms").or_default() += 1;
                for cell in &boxes {
                    for piece in sign_partition(cell, &sector, &shift).unwrap() {
                        *counts.entry("term_sign_cells").or_default() += 1;
                        let actual: [bool; 6] = std::array::from_fn(|axis| {
                            let local = i128::from(piece.lower()[axis]);
                            let parent = if sector[axis] { local + 1 } else { -local };
                            parent + i128::from(shift[axis]) > 0
                        });
                        if coefficient_vanishes(
                            &term.coefficient,
                            &piece,
                            &sector,
                            sources.index_variables(),
                            CompletionGeometryLimits::default(),
                        )
                        .unwrap()
                        {
                            *counts.entry("coefficient_dead_sign_cells").or_default() += 1;
                            continue;
                        }
                        if zeros.contains(&actual) {
                            *counts.entry("zero_sector_sign_cells").or_default() += 1;
                            continue;
                        }
                        *counts.entry("surviving_sign_cells").or_default() += 1;
                        let activates = (0..6).any(|axis| !sector[axis] && actual[axis]);
                        if activates {
                            activation_terms.insert((sector, ordinal, term_ordinal));
                        }
                        let kind = if activates {
                            "surviving_activation_or_swap"
                        } else if actual != sector {
                            "surviving_proper_pinches"
                        } else {
                            "surviving_same_sector"
                        };
                        *counts.entry(kind).or_default() += 1;
                        // This probe clips only the runtime witness carrier.
                        // Above classification and the source audit still use
                        // genuinely unbounded mathematical application boxes.
                        let runtime = runtime_piece(&piece, &sector, &shift);
                        let witness = OrderingPolicy::SpiredUncutV1
                            .prove_sector_monotone_shift_descent(&runtime, &[0; 6], &shift);
                        if let Err(error) = witness {
                            failed_terms.insert((sector, ordinal, term_ordinal));
                            *counts.entry("existing_descent_failures").or_default() += 1;
                            if examples < 16 {
                                eprintln!(
                                    "descent gap: sector={sector:?} rule={ordinal} term={term_ordinal} shift={shift:?} actual={actual:?} local={:?}..{:?} error={error}",
                                    piece.lower(),
                                    piece.upper()
                                );
                                examples += 1;
                            }
                        } else {
                            *counts.entry("existing_descent_successes").or_default() += 1;
                        }
                    }
                }
            }
        }
    }
    counts.insert("unique_surviving_activation_terms", activation_terms.len());
    counts.insert("unique_existing_descent_failure_terms", failed_terms.len());
    assert_eq!(counts["surviving_activation_or_swap"], 0);
    assert_eq!(counts["existing_descent_failures"], 0);
    assert!(counts["surviving_same_sector"] > 0);
    assert!(counts["surviving_proper_pinches"] > 0);
    for (name, count) in counts {
        eprintln!("grounding {name}={count}");
    }
}
