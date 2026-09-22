use super::*;

const FIXED: RoutedFeedbackNomination = RoutedFeedbackNomination::FixedTargets;

#[test]
fn fixed_feedback_complete_finite_entry_envelope_with_external_terminal() {
    use crate::{EntryPowerBudget, FiniteEntryDomain};
    let domain = FiniteEntryDomain::new(
        vec![true],
        EntryPowerBudget::renormalizable_marginal_feynman(1).unwrap(),
    )
    .unwrap();
    let targets = domain.targets().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(targets, vec![key([2]), key([3]), key([4])]);
    let mut observations = Vec::new();
    for workers in [1, 2, 6] {
        let mut run = session(family(K1), vec![([true], vec![])], targets.clone());
        run.workers = workers;
        run.options = fixed_options();
        run.options.fixed_residual_policy =
            RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
        let first = run.run_round(&AtomicBool::new(false), |_| {});
        assert!(first.feedback_round_complete, "{}", first.document);
        assert!(!first.completed_finite_trace);
        assert!(first.document["jobs"][0]["rules"].as_u64().unwrap() >= 3);
        let second = run.run_round(&AtomicBool::new(false), |_| {});
        assert!(second.completed_finite_trace, "{}", second.document);
        let trace = run.reducer.trace_targets(targets.clone()).unwrap();
        assert!(trace.frontier().is_empty());
        assert_eq!(
            trace.declared_terminals(),
            &std::collections::BTreeSet::from([key([1])])
        );
        // The terminal lies outside the entry A-R>=2 band and must survive.
        assert!(!domain.contains(&key([1])));
        observations.push((
            trace.rule_applications(),
            trace.declared_terminals().clone(),
        ));
    }
    assert!(observations.windows(2).all(|pair| pair[0] == pair[1]));
}

fn fixed_options() -> RoutedFeedbackOptions {
    let mut out = options();
    out.nomination = FIXED;
    out.prospective_policy.numerical_depth = 1;
    out
}

#[test]
fn fixed_feedback_policy_is_opt_in_search_only_and_terminals_require_positive_depth() {
    let legacy = options();
    assert_eq!(legacy.nomination, RoutedFeedbackNomination::PositiveRays);
    assert_eq!(
        legacy.fixed_residual_policy,
        RoutedFeedbackFixedResidualPolicy::KeepUnresolved
    );
    assert!(legacy.validate::<3>().is_ok());
    let mut opts = fixed_options();
    assert!(opts.validate::<3>().is_ok());
    opts.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert!(opts.validate::<3>().is_err());
    opts.finite_case_policy = FiniteCasePolicy::SearchFinite;
    opts.prospective_policy.numerical_depth = 0;
    assert!(opts.validate::<3>().is_ok()); // no residual may be declared.
    opts.fixed_residual_policy = RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    assert!(opts.validate::<3>().is_err());
    opts.prospective_policy.numerical_depth = 1;
    assert!(opts.validate::<3>().is_ok());
}

#[test]
fn fixed_feedback_preserves_all_powers_actual_rank_and_compact_boundaries() {
    let case = SourceCase::from_target([true, true, false], &key([7, 3, -11]), FIXED).unwrap();
    assert_eq!(case.fixed, [Some(7), Some(3), Some(-11)]);
    assert_eq!(case.rank, 11);
    assert!(case.case().is_numerical());
    assert_eq!(
        case.case().integral(),
        Integral::numeric([7, 3, -11]).unwrap()
    );
    assert!(SourceCase::from_target([true, false], &key([63, -64]), FIXED).is_ok());
    for powers in [[64, -1], [i64::MAX, -1], [1, -65], [1, i64::MIN], [0, -1]] {
        assert!(SourceCase::from_target([true, false], &key(powers), FIXED).is_err());
    }
    // The old ray policy deliberately keeps arbitrarily large positive powers symbolic.
    assert!(
        SourceCase::from_target(
            [true, false],
            &key([i64::MAX, -1]),
            RoutedFeedbackNomination::PositiveRays
        )
        .is_ok()
    );
}

#[test]
fn fixed_feedback_actual_descendants_keep_above_entry_rank_and_increased_dots() {
    let family = family(K3);
    let context = rustred::identity::ParametricIbpGenerator::try_new(&family)
        .unwrap()
        .context()
        .clone();
    // Synthetic descending edges isolate nomination, not source provenance.
    let rules = [([1, 3, -10], [1, 1, -11]), ([1, 1, -3], [3, 1, 0])]
        .into_iter()
        .map(|(parent, child)| SectorRule {
            candidate: RuleCandidate {
                case: CoordinateCase::new(parent.map(Some)).unwrap().into(),
                target: Integral::numeric(parent).unwrap(),
                rhs: vec![Term {
                    integral: Integral::numeric(child).unwrap(),
                    coefficient: context.one().raw().clone(),
                }],
                sources: vec![],
                stats: SearchStats::default(),
            },
            exceptions: ExceptionalConditions::default(),
        })
        .collect();
    let run = session(family, vec![([true, true, false], rules)], vec![]);
    let report = run
        .reducer
        .trace_targets([key([1, 3, -10]), key([1, 1, -3])])
        .unwrap();
    let jobs = nominate(&report, &[], FIXED, 8, 4096, &AtomicBool::new(false));
    assert_eq!(jobs.jobs.len(), 2);
    assert!(
        jobs.jobs
            .iter()
            .any(|case| case.fixed == [Some(1), Some(1), Some(-11)] && case.rank == 11)
    );
    assert!(
        jobs.jobs
            .iter()
            .any(|case| case.fixed == [Some(3), Some(1), Some(0)] && case.rank == 0)
    );
    let groups: Vec<_> = batches(&jobs.jobs, FIXED, 8).collect();
    assert_eq!(groups.len(), 1);
    let receipt = job_receipt(0, groups[0], FIXED);
    assert_eq!(receipt["actual_numerator_rank"], 11);
    assert_eq!(receipt["fixed_targets"], json!([[1, 1, -11], [3, 1, 0]]));
}

#[test]
fn fixed_feedback_dedup_and_owner_batches_are_deterministic_and_capped() {
    let run = session(
        family(K3),
        vec![([true, true, false], vec![]), ([true, false, true], vec![])],
        vec![],
    );
    let targets = [
        key([3, 1, 0]),
        key([2, 0, 2]),
        key([2, 1, 0]),
        key([3, 1, 0]),
    ];
    let report = run.reducer.trace_targets(targets.clone()).unwrap();
    let reversed = run
        .reducer
        .trace_targets(targets.into_iter().rev())
        .unwrap();
    let jobs = nominate(&report, &[], FIXED, 8, 4096, &AtomicBool::new(false));
    let again = nominate(&reversed, &[], FIXED, 8, 4096, &AtomicBool::new(false));
    assert_eq!(jobs.jobs, again.jobs);
    assert_eq!(jobs.jobs.len(), 3); // native frontier already deduplicated the repeated point.
    assert_ne!(jobs.jobs[1].fixed, jobs.jobs[2].fixed); // distinct dots do not collapse.
    let groups: Vec<_> = batches(&jobs.jobs, FIXED, 8).collect();
    assert_eq!(
        groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(
        groups
            .iter()
            .all(|group| group.iter().all(|case| case.owner == group[0].owner))
    );
    assert_eq!(batches(&jobs.jobs, FIXED, 1).count(), 3);
    assert_eq!(
        batches(&jobs.jobs, RoutedFeedbackNomination::PositiveRays, 8).count(),
        3
    );
    let installed = nominate(&report, &jobs.jobs, FIXED, 8, 4096, &AtomicBool::new(false));
    assert!(installed.jobs.is_empty());
    assert_eq!(installed.already_installed_entries, 3);
    let capped = nominate(&report, &[], FIXED, 2, 4096, &AtomicBool::new(false));
    assert_eq!(capped.jobs.len(), 2);
    assert_eq!(
        capped.incomplete.as_ref().unwrap().0,
        "nomination resource limit"
    );
    let cancelled = nominate(&report, &[], FIXED, 8, 4096, &AtomicBool::new(true));
    assert!(cancelled.jobs.is_empty());
    assert!(cancelled.incomplete.is_some());
}

#[test]
fn fixed_feedback_batched_real_repair_installs_fixed_rules_without_positive_widening() {
    let mut run = tiny();
    run.options = fixed_options();
    let old = run.programs().clone();
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(result.feedback_round_complete, "{:?}", result.document);
    assert!(!result.completed_finite_trace); // I(1) is not declared by fixing only I(2), I(3).
    assert_eq!(result.document["source_jobs_started"], 1);
    assert_eq!(result.document["nomination"]["cases"], 2);
    assert_eq!(
        result.document["jobs"][0]["fixed_targets"],
        json!([[2], [3]])
    );
    assert_eq!(
        result.document["jobs"][0]["active_coordinates_symbolic"],
        false
    );
    assert_eq!(result.document["jobs"][0]["search_residuals"], 0);
    assert_eq!(result.document["jobs"][0]["productive_rule_repair"], true);
    assert_eq!(result.document["jobs"][0]["numerical_cases_searched"], 2);
    assert_eq!(result.document["jobs"][0]["symbolic_cases_searched"], 0);
    assert!(result.document["jobs"][0]["rules"].as_u64().unwrap() >= 2);
    assert_eq!(run.installed_jobs(), 2);
    assert_eq!(run.programs().overlay_usage().batches, 1);
    assert!(
        serde_json::to_vec(&result.document).unwrap().len()
            <= receipt_bound(run.options.max_jobs_per_round, run.options.max_error_bytes).unwrap()
    );
    assert_eq!(old.overlay_usage().batches, 0);
    let trace = run.reducer.trace_targets([key([2]), key([3])]).unwrap();
    assert!(trace.rule_applications() >= 2);
    assert_eq!(
        trace
            .frontier()
            .iter()
            .map(|item| item.target.clone())
            .collect::<Vec<_>>(),
        [key([1])]
    );
    // Unrequested higher dots still miss; no implicit parametric patch was installed.
    let beyond = run.reducer.trace_targets([key([7])]).unwrap();
    assert_eq!(beyond.rule_applications(), 0);
    assert_eq!(beyond.frontier().iter().next().unwrap().target, key([7]));
}

#[test]
fn fixed_feedback_residual_default_is_incomplete_without_terminal_publication() {
    let mut run = session(family(K1), vec![([true], vec![])], vec![key([1])]);
    run.options = fixed_options();
    let old = run.programs().clone();
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(!result.completed_finite_trace && !result.feedback_round_complete);
    let job = &result.document["jobs"][0];
    assert_eq!(job["status"], "fixed_search_residuals_uninstalled");
    assert_eq!(job["searched_residual_keys"], json!([[1]]));
    assert_eq!(job["search_residuals"], 1);
    assert_eq!(job["declared_terminals"], 0);
    assert_eq!(job["productive_rule_repair"], false);
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&old, run.programs()));
    assert!(
        !run.reducer
            .trace_targets([key([1])])
            .unwrap()
            .frontier()
            .is_empty()
    );
}

#[test]
fn fixed_feedback_strict_residual_batch_is_atomic_even_with_real_rules() {
    let mut run = session(family(K1), vec![([true], vec![])], vec![key([1]), key([2])]);
    run.options = fixed_options();
    let old = run.programs().clone();
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(!result.completed_finite_trace && !result.feedback_round_complete);
    let job = &result.document["jobs"][0];
    assert_eq!(job["status"], "fixed_search_residuals_uninstalled");
    assert_eq!(job["productive_rule_repair"], true);
    assert_eq!(job["searched_residual_keys"], json!([[1]]));
    assert_eq!(job["declared_terminals"], 0);
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&old, run.programs()));
}

#[test]
fn fixed_feedback_explicit_searched_terminal_policy_keeps_nonminimal_status_and_keys() {
    let mut run = session(family(K1), vec![([true], vec![])], vec![key([1]), key([2])]);
    run.options = fixed_options();
    run.options.fixed_residual_policy =
        RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    assert!(run.options.validate::<1>().is_ok());
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(
        result.completed_finite_trace && result.feedback_round_complete,
        "{:?}",
        result.document
    );
    let job = &result.document["jobs"][0];
    assert_eq!(job["status"], "installed");
    assert_eq!(job["searched_residual_keys"], json!([[1]]));
    assert_eq!(job["residual_search_depth"], 1);
    assert_eq!(job["numerical_cases_searched"], 2);
    assert_eq!(
        job["residual_origin"],
        "completed-bounded-fixed-source-search"
    );
    assert_eq!(job["declared_terminals"], 1);
    assert_eq!(job["productive_rule_repair"], true);
    assert_eq!(result.document["family_closure_claim"], false);
    assert_eq!(result.document["terminal_independence_claim"], false);
    assert_eq!(result.document["numerical_terminal_values_claim"], false);
    assert_eq!(
        run.reducer
            .trace_targets([key([2])])
            .unwrap()
            .declared_terminals(),
        &std::collections::BTreeSet::from([key([1])])
    );
    let repeat = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(repeat.completed_finite_trace);
    assert_eq!(repeat.document["source_jobs_started"], 0);
}

#[test]
fn fixed_feedback_errors_and_post_search_cancellation_never_declare_terminals() {
    let mut run = session(family(K1), vec![([true], vec![])], vec![key([1])]);
    run.options = fixed_options();
    run.options.fixed_residual_policy =
        RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    run.options.prospective_policy.symbolic.prime = 2; // mandatory native input failure.
    let old = run.programs().clone();
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert_eq!(result.document["jobs"][0]["status"], "search_failed");
    assert!(!result.completed_finite_trace && !result.feedback_round_complete);
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&old, run.programs()));
    run.options.prospective_policy.symbolic = Default::default();
    let cancellation = AtomicBool::new(false);
    let result = run.run_round(&cancellation, |event| {
        if event["event"] == "local_domain_complete" {
            cancellation.store(true, Ordering::Relaxed);
        }
    });
    assert_eq!(result.document["status"], "cancelled");
    assert!(!result.completed_finite_trace);
    assert_eq!(result.document["jobs"][0]["declared_terminals"], 0);
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&old, run.programs()));
}

#[test]
fn fixed_feedback_batches_respect_native_case_and_cumulative_ledger_admission() {
    let mut run = tiny();
    run.options = fixed_options();
    run.options.attempt_limits.max_requested_cases = 1;
    let result = run.run_round(&AtomicBool::new(false), |_| {});
    assert_eq!(result.document["source_jobs_started"], 2);
    assert_eq!(result.document["jobs"][0]["fixed_targets"], json!([[2]]));
    assert_eq!(result.document["jobs"][1]["fixed_targets"], json!([[3]]));
    assert_eq!(run.installed_jobs(), 2);
    assert_eq!(run.programs().overlay_usage().batches, 2);
    let mut run = tiny();
    run.options = fixed_options();
    run.options.max_installed_jobs = 1;
    let old = run.programs().clone();
    let denied = run.run_round(&AtomicBool::new(false), |_| {});
    assert_eq!(denied.document["source_jobs_started"], 0);
    assert_eq!(
        denied.document["jobs"][0]["status"],
        "installed_ledger_limit"
    );
    assert_eq!(run.installed_jobs(), 0);
    assert!(Arc::ptr_eq(&old, run.programs()));
}

#[test]
fn fixed_feedback_replace_targets_reuses_programs_ledger_and_fixed_source_work() {
    let mut run = session(family(K1), vec![([true], vec![])], vec![key([1]), key([2])]);
    run.options = fixed_options();
    run.options.fixed_residual_policy =
        RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    let first = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(first.completed_finite_trace, "{:?}", first.document);
    assert_eq!(run.installed_jobs(), 2);
    let retained = run.programs().clone();
    run.replace_targets([key([2])]).unwrap();
    assert!(Arc::ptr_eq(&retained, run.programs()));
    assert_eq!(run.installed_jobs(), 2);
    let reused = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(reused.completed_finite_trace);
    assert_eq!(reused.document["round"], 2);
    assert_eq!(reused.document["input_targets"], 1);
    assert_eq!(reused.document["source_jobs_started"], 0);
    assert!(Arc::ptr_eq(&retained, run.programs()));
    run.replace_targets([key([3])]).unwrap();
    assert!(Arc::ptr_eq(&retained, run.programs()));
    let repaired = run.run_round(&AtomicBool::new(false), |_| {});
    assert!(repaired.completed_finite_trace, "{:?}", repaired.document);
    assert_eq!(repaired.document["round"], 3);
    assert_eq!(repaired.document["source_jobs_started"], 1);
    assert_eq!(repaired.document["jobs"][0]["fixed_targets"], json!([[3]]));
    assert_eq!(repaired.document["jobs"][0]["search_residuals"], 0);
    assert_eq!(run.installed_jobs(), 3);
    assert_eq!(retained.overlay_usage().batches, 1);
    assert_eq!(run.programs().overlay_usage().batches, 2);
}

#[test]
fn fixed_feedback_replace_targets_rejects_invalid_batches_atomically() {
    let mut run = tiny();
    run.options = fixed_options();
    let programs = run.programs().clone();
    run.reducer = RoutedCandidateReducer::try_new(
        programs.clone(),
        [],
        rustred::solver::RoutedCandidateLimits {
            max_input_targets: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let before = run.targets.clone();
    for bad in [
        vec![],
        vec![key([4]), key([4, 1])],
        vec![key([4]), key([4]), key([4])],
    ] {
        assert!(run.replace_targets(bad).is_err());
        assert_eq!(run.targets, before);
        assert!(Arc::ptr_eq(&programs, run.programs()));
        assert_eq!(run.installed_jobs(), 0);
        assert_eq!(run.rounds, 0);
    }
    // Equality at the original cap is admitted; duplicates are retained and charged.
    run.replace_targets([key([4]), key([4])]).unwrap();
    assert_eq!(run.targets, [key([4]), key([4])]);
    assert!(Arc::ptr_eq(&programs, run.programs()));
}

#[test]
fn fixed_feedback_replace_targets_cannot_bypass_search_or_depth_policy() {
    let mut run = tiny();
    run.options = fixed_options();
    let before = run.targets.clone();
    // Private fault injection: public session options are immutable after prepare.
    run.options.finite_case_policy = FiniteCasePolicy::RetainRankFinite;
    assert!(run.replace_targets([key([4])]).is_err());
    assert_eq!(run.targets, before);
    run.options.finite_case_policy = FiniteCasePolicy::SearchFinite;
    run.options.fixed_residual_policy =
        RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
    run.options.prospective_policy.numerical_depth = 0;
    assert!(run.replace_targets([key([4])]).is_err());
    assert_eq!(run.targets, before);
    run.options.prospective_policy.numerical_depth = 1;
    run.replace_targets([key([4])]).unwrap();
    assert_eq!(run.targets, [key([4])]);
}
