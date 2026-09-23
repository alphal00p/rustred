use super::*;

#[test]
fn large_entry_description_is_charged_in_receipt_admission() {
    let queries: Vec<_> = (0..9000)
        .map(|i| {
            json!({
                "id":format!("q{i}"), "owner":"1", "lower":[0], "upper":[0],
                "max_numerator_rank":0
            })
        })
        .collect();
    let text =
        json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":queries}).to_string();
    assert!(text.len() < 1024 * 1024);
    let domain = super::super::entry::RequestedEntryDomain::<1>::parse(&text).unwrap();
    let bytes = domain.description_json_bytes();
    assert_eq!(
        bytes,
        serde_json::to_vec(&super::super::entry::description(Some(&domain)))
            .unwrap()
            .len()
    );
    assert!(bytes > receipt_bound(1, 1).unwrap());
    options().validate_receipt(bytes).unwrap();
    let mut near_limit = options();
    near_limit.max_jobs_per_round = 1;
    near_limit.max_error_bytes = (crate::application::MAX_OUTPUT_BYTES - 8192 - 512 * 1024) / 30;
    near_limit.validate::<1>().unwrap();
    assert!(near_limit.validate_receipt(bytes).is_err());
    assert!(near_limit.validate_receipt(usize::MAX).is_err());
}

#[test]
fn explicit_entry_policy_survives_repairs_rebinding_and_atomic_replacement() {
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"starting-powers","owner":"1","lower":[1],"upper":[3],
         "max_numerator_rank":0,"power_bounds":{"min_power_difference":2}}]})
    .to_string();
    for workers in [1, 2, 6] {
        let mut run = session(
            family(K1),
            vec![([true], vec![])],
            vec![key([2]), key([3]), key([4])],
        );
        run.entry_domain = Some(super::super::entry::RequestedEntryDomain::parse(&text).unwrap());
        run.workers = workers;
        run.options.nomination = RoutedFeedbackNomination::FixedTargets;
        run.options.prospective_policy.numerical_depth = 1;
        run.options.fixed_residual_policy =
            RoutedFeedbackFixedResidualPolicy::DeclareSearchedFiniteTerminals;
        let original_context = run.programs().context().clone();
        let first = run.run_round(&AtomicBool::new(false), |_| {});
        assert!(first.feedback_round_complete, "{}", first.document);
        assert!(!first.completed_finite_trace);
        let second = run.run_round(&AtomicBool::new(false), |_| {});
        assert!(second.completed_finite_trace, "{}", second.document);
        assert_eq!(
            second.document["entry_admission"]["mode"],
            "explicit_finite"
        );
        assert_eq!(second.document["saved_generation_max_numerator_rank"], 10);
        assert_eq!(second.document["family_closure_claim"], false);
        assert!(Arc::ptr_eq(&original_context, run.programs().context()));
        let trace = run
            .trace("verify", &AtomicBool::new(false), &|_| {})
            .unwrap();
        // The terminal is OUTSIDE the original A>=2 input domain, and is still
        // reached by exact tracing. No entry bound may clip that descendant.
        assert_eq!(
            trace.trace().declared_terminals(),
            &std::collections::BTreeSet::from([key([1])])
        );
        assert!(
            run.entry_domain
                .as_ref()
                .unwrap()
                .validate(&key([1]))
                .is_err()
        );
        let previous = run.targets.clone();
        let programs = run.programs().clone();
        let installed = run.installed_jobs();
        for bad in [vec![key([3]), key([1])], vec![key([5])]] {
            assert!(run.replace_targets(bad).is_err());
            assert_eq!(run.targets, previous);
            assert!(Arc::ptr_eq(&programs, run.programs()));
            assert_eq!(run.installed_jobs(), installed);
        }
        run.replace_targets([key([4])]).unwrap();
        let reused = run.run_round(&AtomicBool::new(false), |_| {});
        assert!(reused.completed_finite_trace);
        assert_eq!(reused.document["source_jobs_started"], 0);
        assert_eq!(
            reused.document["entry_admission"],
            second.document["entry_admission"]
        );
    }
}

#[test]
fn explicit_above_saved_rank_trace_is_not_an_implicit_rule_or_terminal() {
    let text = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"rank-eleven","owner":"110","lower":[0,0,11],"upper":[0,0,11],
         "max_numerator_rank":11}]})
    .to_string();
    let mut run = session(
        family(K3),
        vec![([true, true, false], vec![])],
        vec![key([1, 1, -11])],
    );
    assert!(
        run.trace("default", &AtomicBool::new(false), &|_| {})
            .is_err()
    );
    run.entry_domain = Some(super::super::entry::RequestedEntryDomain::parse(&text).unwrap());
    for workers in [1, 2, 6] {
        run.workers = workers;
        let report = run
            .trace("explicit", &AtomicBool::new(false), &|_| {})
            .unwrap();
        assert_eq!(report.trace().frontier().len(), 1);
        assert_eq!(
            report.trace().frontier().iter().next().unwrap().target,
            key([1, 1, -11])
        );
        assert!(report.trace().declared_terminals().is_empty());
        assert_eq!(
            run.programs().context().scope().max_numerator_rank,
            Some(10)
        );
    }
    assert_eq!(run.installed_jobs(), 0);
}
