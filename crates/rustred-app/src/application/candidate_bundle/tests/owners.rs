//! Immutable selected records share one context; they are never a closure claim.
use std::collections::BTreeSet;
use std::sync::Arc;

use rustred::family::IntegralKey;
use rustred::persistence::{BinaryProgramKind, BinarySection, encode_program};
use rustred::sector::Mask;
use rustred::solver::{
    CandidateReductionError, CandidateRoutedError, CandidateRoutedFrontierReason,
    RoutedCandidateReducer,
};

use super::*;

fn generated() -> Bundle {
    let mut request = FamilyCandidatesRequest::new(K3);
    request.numerical_depth = 0;
    request.max_numerator_rank = Some(2);
    request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    request.permutation = Some(vec![2, 0, 1]);
    let output = family_candidates(request).unwrap();
    codec::read(output.bundle(), Default::default()).unwrap()
}

fn split(bundle: &Bundle) -> Vec<(Mask, Vec<u8>)> {
    bundle
        .sectors
        .iter()
        .map(|sector| {
            let mut shard = bundle.clone();
            shard.sectors = vec![sector.clone()];
            (
                Mask::try_new(sector.sector.clone()).unwrap(),
                codec::write(&shard, Default::default()).unwrap(),
            )
        })
        .collect()
}

fn inputs(saved: &[(Mask, Vec<u8>)]) -> Vec<CandidateOwnerBundle<'_>> {
    saved
        .iter()
        .map(|(mask, bytes)| CandidateOwnerBundle {
            bytes,
            owner_sector: mask,
        })
        .collect()
}

fn section(bytes: &[u8], tag: SectionTag, replacement: &[u8]) -> Vec<u8> {
    let limits = CandidateBundleLimits::default();
    let (envelope, _, _) = codec::read_structure(bytes, limits).unwrap();
    let sections = envelope
        .sections()
        .iter()
        .map(|part| BinarySection {
            tag: part.tag,
            bytes: if part.tag == tag {
                replacement
            } else {
                part.bytes
            },
        })
        .collect::<Vec<_>>();
    encode_program(
        BinaryProgramKind::Candidates,
        &sections,
        limits.binary_limits(),
    )
    .unwrap()
}

#[test]
fn selected_owner_loader_matches_monolithic_trace_and_shares_native_context() {
    let bundle = generated();
    let whole = codec::write(&bundle, Default::default()).unwrap();
    let saved = split(&bundle);
    let before = saved.clone();
    let (family, programs) = load_generated_candidate_owners::<3>(
        &inputs(&saved),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(&family, programs.context().family_owner()));
    assert_eq!(programs.owner_count(), bundle.sectors.len());
    assert_eq!(programs.context().scope().max_numerator_rank, Some(2));
    assert_eq!(
        programs.context().scope().finite_case_policy,
        FiniteCasePolicy::RetainRankFinite
    );
    assert_eq!(
        programs.owner_sectors().copied().collect::<BTreeSet<_>>(),
        saved
            .iter()
            .map(|(mask, _)| mask.active_bits().try_into().unwrap())
            .collect()
    );
    let (_, mut reference) =
        load_generated_candidate_bundle::<3>(&whole, Default::default(), Default::default())
            .unwrap();
    assert_eq!(programs.terminal_count(), reference.terminals().len());
    let routed =
        RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
    let targets = [[3, 2, 1], [0, 3, 2], [-2, 2, 1], [0, 0, 1]]
        .into_iter()
        .map(|p| IntegralKey::try_new(p).unwrap())
        .collect::<Vec<_>>();
    let expected = reference
        .trace_targets(targets.clone(), Default::default())
        .unwrap();
    let actual = routed.trace_targets(targets).unwrap();
    assert_eq!(
        actual
            .frontier()
            .iter()
            .map(|x| x.target.clone())
            .collect::<BTreeSet<_>>(),
        *expected.uncovered()
    );
    assert_eq!(actual.declared_terminals(), expected.declared_terminals());
    assert_eq!(actual.visited_zeros(), expected.visited_zeros());
    assert_eq!(actual.rule_applications(), expected.rule_applications());
    assert_eq!(actual.reachable_integrals(), expected.reachable_integrals());
    assert_eq!(saved, before);
}

#[test]
fn selected_owner_loader_rejects_ambiguous_or_incompatible_records() {
    let bundle = generated();
    let saved = split(&bundle);
    assert!(saved.len() > 1);
    let before = saved.clone();
    let load = |records: &[(Mask, Vec<u8>)]| {
        load_generated_candidate_owners::<3>(
            &inputs(records),
            Default::default(),
            Default::default(),
        )
    };
    assert!(load(&[]).unwrap_err().message().contains("empty"));
    assert!(
        load(&[saved[0].clone(), saved[0].clone()])
            .unwrap_err()
            .message()
            .contains("duplicate")
    );
    let whole = codec::write(&bundle, Default::default()).unwrap();
    assert!(
        load(&[(saved[0].0.clone(), whole)])
            .unwrap_err()
            .message()
            .contains("exactly one sector")
    );
    assert!(
        load(&[(saved[1].0.clone(), saved[0].1.clone())])
            .unwrap_err()
            .message()
            .contains("mask differs")
    );
    assert!(
        load_generated_candidate_owners::<1>(
            &inputs(&saved),
            Default::default(),
            Default::default()
        )
        .unwrap_err()
        .message()
        .contains("arity mismatch")
    );
    let mut truncated = saved.clone();
    truncated[0].1.pop();
    assert!(load(&truncated).is_err());

    let original = codec::read(&saved[1].1, Default::default()).unwrap();
    for (label, changed_policy) in [
        ("rank", super::super::policy::encode_scoped(0, Some(3))),
        (
            "finite policy",
            super::super::policy::encode_scoped(0, Some(2)),
        ),
        ("depth", {
            let mut request = FamilyCandidatesRequest::new(K3);
            request.max_numerator_rank = Some(2);
            request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
            super::super::policy::encode_request(&request)
        }),
        ("finite limits", {
            let mut request = FamilyCandidatesRequest::new(K3);
            request.numerical_depth = 0;
            request.max_numerator_rank = Some(2);
            request.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
            request.finite_case_limits.max_visited_points += 1;
            super::super::policy::encode_request(&request)
        }),
    ] {
        let mut changed = original.clone();
        changed.solver_policy = changed_policy;
        let pair = [
            saved[0].clone(),
            (
                saved[1].0.clone(),
                codec::write(&changed, Default::default()).unwrap(),
            ),
        ];
        let error = load(&pair).unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Input, "{label}: {error}");
        assert!(
            error.message().contains("policies differ"),
            "{label}: {error}"
        );
    }
    let mut changed = original;
    changed
        .family_fingerprint
        .push_str("-not-the-native-family");
    let bad_family = (
        saved[1].0.clone(),
        codec::write(&changed, Default::default()).unwrap(),
    );
    assert!(
        load(&[saved[0].clone(), bad_family.clone()])
            .unwrap_err()
            .message()
            .contains("fingerprints differ")
    );
    // A single forged textual fingerprint must still fail native reconstruction.
    assert!(load(&[bad_family]).is_err());
    assert_eq!(saved, before);
}

#[test]
fn selected_owner_loader_checks_whole_list_budgets_before_native_import() {
    let mut bundle = generated();
    // Make the structural threshold larger than either admitted dictionary,
    // so the intended second-sector collection failure is unambiguous even
    // if native coefficient deduplication changes the tiny fixture's count.
    let repeated_terminals = 2 * bundle.coefficients.len() + 100;
    for sector in &mut bundle.sectors {
        sector.rules.clear();
        sector.finite_residuals = vec![
            IntegralRecord {
                symbolic: vec![false; 3],
                values: sector.sector.iter().map(|&x| i16::from(x)).collect(),
            };
            repeated_terminals
        ];
    }
    let mut saved = split(&bundle);
    assert!(saved.len() > 1);
    // No successful path may import this deliberately invalid native state.
    // Each failure below must arise during cumulative structural preflight.
    saved[0].1 = section(
        &saved[0].1,
        SectionTag::SYMBOLICA_STATE,
        b"invalid native state",
    );
    let before = saved.clone();
    let load = |limits| {
        load_generated_candidate_owners::<3>(&inputs(&saved), limits, Default::default())
            .unwrap_err()
    };
    let check = |limits, expected: &str| {
        let error = load(limits);
        assert_eq!(error.kind(), AppErrorKind::Limit);
        assert!(error.message().contains(expected), "{error}");
    };
    let base = CandidateOwnerLoadLimits::default();
    check(
        CandidateOwnerLoadLimits {
            max_total_input_bytes: 1,
            ..base
        },
        "aggregate input-byte",
    );
    check(
        CandidateOwnerLoadLimits {
            bundle: CandidateBundleLimits {
                max_collection_entries: 1,
                ..base.bundle
            },
            ..base
        },
        "collection-entry",
    );
    check(
        CandidateOwnerLoadLimits {
            bundle: CandidateBundleLimits {
                max_collection_entries: 2 * repeated_terminals,
                ..base.bundle
            },
            ..base
        },
        "aggregate collection-entry",
    );
    let section_max = |tag| {
        saved
            .iter()
            .map(|(_, bytes)| {
                codec::read_structure(bytes, base.bundle)
                    .unwrap()
                    .0
                    .section(tag)
                    .unwrap()
                    .len()
            })
            .max()
            .unwrap()
    };
    check(
        CandidateOwnerLoadLimits {
            max_total_symbolica_state_bytes: section_max(SectionTag::SYMBOLICA_STATE),
            ..base
        },
        "aggregate Symbolica-state",
    );
    check(
        CandidateOwnerLoadLimits {
            bundle: CandidateBundleLimits {
                max_total_coefficient_bytes: section_max(SectionTag::COEFFICIENTS),
                ..base.bundle
            },
            ..base
        },
        "aggregate coefficient-table",
    );
    check(
        CandidateOwnerLoadLimits {
            max_zero_sector_visits: 7,
            ..base
        },
        "zero-sector enumeration",
    );
    assert_eq!(saved, before);

    let mut inflated = saved.clone();
    for (_, bytes) in &mut inflated {
        let (envelope, _, _) = codec::read_structure(bytes, base.bundle).unwrap();
        let mut coefficients = envelope.section(SectionTag::COEFFICIENTS).unwrap().to_vec();
        coefficients[..8].copy_from_slice(&100_000_u64.to_le_bytes());
        *bytes = section(bytes, SectionTag::COEFFICIENTS, &coefficients);
    }
    let error = load_generated_candidate_owners::<3>(
        &inputs(&inflated),
        CandidateOwnerLoadLimits {
            bundle: CandidateBundleLimits {
                max_collection_entries: 150_000,
                ..base.bundle
            },
            ..base
        },
        Default::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Limit);
    assert!(error.message().contains("aggregate coefficient-entry"));
}

#[test]
fn selected_owner_loader_keeps_above_rank_children_and_missing_owner_frontiers() {
    // Deliberately uncertified synthetic formulas test the transport/entry
    // boundary, not an assertion that the written rule is an IBP identity.
    let mut bundle = generated();
    bundle.solver_policy = super::super::policy::encode_scoped(0, Some(0));
    let context =
        ParametricIbpGenerator::try_new(&preparation::family(K3, InputFormat::Toml).unwrap())
            .unwrap()
            .context()
            .clone();
    let one = (0..bundle.coefficients.len())
        .find(|&i| {
            let coefficient = bundle
                .coefficients
                .coefficient(CoefficientId::try_from_index(i).unwrap())
                .unwrap();
            coefficient == context.one().raw()
                && coefficient.numerator.variables() == context.one().raw().numerator.variables()
        })
        .unwrap() as u32;
    let child = IntegralRecord {
        symbolic: vec![false; 3],
        values: vec![-3, 1, 1],
    };
    for sector in &mut bundle.sectors {
        sector.rules.clear();
        sector.finite_residuals.clear();
        if sector.sector == [true; 3] {
            sector.rules.push(RuleRecord {
                case: CaseRecord {
                    kind: "coordinate".into(),
                    fixed_axes: vec![0, 1, 2],
                    fixed_values: vec![1, 1, 1],
                    equations: vec![],
                },
                target: IntegralRecord {
                    symbolic: vec![false; 3],
                    values: vec![1, 1, 1],
                },
                rhs: vec![TermRecord {
                    integral: child.clone(),
                    coefficient: one,
                }],
                sources: vec![],
                exclusions: vec![],
            });
        } else if sector.sector == [false, true, true] {
            sector.finite_residuals.push(child.clone());
        }
    }
    let root = IntegralKey::try_new([1, 1, 1]).unwrap();
    let child = IntegralKey::try_new([-3, 1, 1]).unwrap();
    for omit_child_owner in [false, true] {
        let mut saved = split(&bundle);
        if omit_child_owner {
            saved.retain(|(mask, _)| mask.active_bits() != [false, true, true]);
        }
        let (_, programs) = load_generated_candidate_owners::<3>(
            &inputs(&saved),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let reducer =
            RoutedCandidateReducer::try_new(Arc::new(programs), [], Default::default()).unwrap();
        let trace = reducer.trace_targets([root.clone()]).unwrap();
        assert_eq!(trace.max_negative_index_degree(), 3);
        if omit_child_owner {
            assert_eq!(trace.frontier().len(), 1);
            let missing = trace.frontier().first().unwrap();
            assert_eq!(missing.target, child);
            assert_eq!(missing.reason, CandidateRoutedFrontierReason::MissingOwner);
        } else {
            assert!(trace.frontier().is_empty());
            assert_eq!(trace.declared_terminals(), &BTreeSet::from([child.clone()]));
        }
        assert!(matches!(
            reducer.trace_targets([child.clone()]),
            Err(CandidateRoutedError::Candidate(
                CandidateReductionError::OutsideNumeratorRank { .. }
            ))
        ));
    }
}
