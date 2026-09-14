use super::*;
use rustred::algebra::CoefficientContext;
use rustred::solver::{CoordinateCase, ExceptionalConditions, SearchStats, SectorRule, Term};

fn fixture() -> (CoefficientContext, SourceSystem<1>, RuleCandidate<1>) {
    let context = CoefficientContext::try_new(["s", "t", "a"]).unwrap();
    let source = SourceSystem::new(
        vec![vec![Term {
            integral: Integral::symbolic([0]).unwrap(),
            coefficient: context.one().numerator,
        }]],
        [2],
    )
    .unwrap();
    let s = context.parameter("s").unwrap();
    let t = context.parameter("t").unwrap();
    let candidate = RuleCandidate {
        case: CoordinateCase::generic(),
        target: Integral::symbolic([0]).unwrap(),
        rhs: vec![Term {
            integral: Integral::symbolic([-1]).unwrap(),
            coefficient: &(&(&s * &s) + &t) / &(&s - &context.one()),
        }],
        sources: Vec::new(),
        stats: SearchStats::default(),
    };
    (context, source, candidate)
}

fn reference(coefficient: &str) -> String {
    format!("{{int[n1_?Positive]/;!(n1==1)->({coefficient})*int[-1+n1]}}")
}

const ALIAS: CoefficientAlias<'static> = CoefficientAlias {
    reference: "dot[p,p]",
    parameter: "s",
};

#[test]
fn alias_keeps_invariant_symbolic_and_requires_explicit_opt_in() {
    let (context, source, mut candidate) = fixture();
    let text = reference("(dot[p,p]^2+t)/(dot[p,p]-1)");
    assert!(compare(&text, &candidate, &source, None).is_err());
    compare_with_aliases(&text, &candidate, &source, None, &[ALIAS]).unwrap();
    // A numerical substitution for the invariant is not an exact match.
    candidate.rhs[0].coefficient = &context.integer(4) + &context.parameter("t").unwrap();
    assert!(compare_with_aliases(&text, &candidate, &source, None, &[ALIAS]).is_err());
}

#[test]
fn aliases_use_native_function_tokens_not_textual_substrings() {
    let (_, source, candidate) = fixture();
    for function in ["dot[p,p]", "dot(p,p)", "dot [ p , p ]"] {
        let text = reference(&format!("({function}^2+t)/({function}-1)"));
        compare_with_aliases(&text, &candidate, &source, None, &[ALIAS]).unwrap();
    }
    for function in ["otherdot[p,p]", "dot[p,q]", "dot[p,p,p]"] {
        let text = reference(&format!("({function}^2+t)/({function}-1)"));
        assert!(compare_with_aliases(&text, &candidate, &source, None, &[ALIAS]).is_err());
    }
}

#[test]
fn multiple_aliases_preserve_independent_parameters_and_native_normalization() {
    let (_, source, candidate) = fixture();
    let aliases = [
        ALIAS,
        CoefficientAlias {
            reference: "dot[q,q]",
            parameter: "t",
        },
    ];
    let text = reference("((dot[p,p]^2+dot[q,q])*(dot[p,p]+1))/(dot[p,p]^2-1)");
    compare_with_aliases(&text, &candidate, &source, None, &aliases).unwrap();
    let wrong = reference("(dot[p,p]^2+dot[p,p])/(dot[p,p]-1)");
    assert!(compare_with_aliases(&wrong, &candidate, &source, None, &aliases).is_err());
}

#[test]
fn alias_configuration_rejects_indices_numbers_missing_scalars_and_arithmetic() {
    let (_, source, candidate) = fixture();
    let text = reference("(dot[p,p]^2+t)/(dot[p,p]-1)");
    for parameter in ["n1", "a", "1", "s+1", "missing"] {
        let alias = CoefficientAlias {
            reference: "dot[p,p]",
            parameter,
        };
        assert!(compare_with_aliases(&text, &candidate, &source, None, &[alias]).is_err());
    }
    for expression in ["s", "dot[p,p]+1", "dot[p+q,p]", "dot[]"] {
        let alias = CoefficientAlias {
            reference: expression,
            parameter: "s",
        };
        assert!(compare_with_aliases(&text, &candidate, &source, None, &[alias]).is_err());
    }
    assert!(compare_with_aliases(&text, &candidate, &source, None, &[ALIAS, ALIAS]).is_err());
    let round_bracket_alias = CoefficientAlias {
        reference: "dot(p,p)",
        parameter: "t",
    };
    assert!(
        compare_with_aliases(
            &text,
            &candidate,
            &source,
            None,
            &[ALIAS, round_bracket_alias]
        )
        .is_err()
    );
}

#[test]
fn alias_aware_rule_comparison_does_not_rewrite_coordinate_guards() {
    let (context, source, candidate) = fixture();
    let a = context.parameter("a").unwrap();
    let rule = SectorRule {
        candidate,
        exceptions: ExceptionalConditions {
            branches: vec![vec![(&a - &context.one()).numerator]],
        },
    };
    let text = reference("(dot[p,p]^2+t)/(dot[p,p]-1)");
    assert!(compare_rule(&text, &rule, &source, &[true], None).is_err());
    compare_rule_with_aliases(&text, &rule, &source, &[true], None, &[ALIAS]).unwrap();
    for guard in ["n1==2", "dot[p,p]==1"] {
        let wrong = text.replace("n1==1", guard);
        assert!(
            compare_rule_with_aliases(&wrong, &rule, &source, &[true], None, &[ALIAS]).is_err()
        );
    }
}
