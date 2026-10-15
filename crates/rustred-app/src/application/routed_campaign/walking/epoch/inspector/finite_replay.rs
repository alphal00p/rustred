//! Exact initial-query eligibility and result adaptation, using the existing
//! native trace. V2 retains its original ID0/fresh-only contract.
use super::super::super::{finite_replay as finite, inspection, queue::Phase};
use super::super::job::{JobResult, NativeKind};
use super::{Context, Job};
use std::ops::ControlFlow;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn attempt<const N: usize>(
    context: &Context<'_, N>,
    job: &Job<N>,
) -> Option<finite::Outcome> {
    let limits = context.request.finite_replay?;
    if job.flags != 0 {
        return None;
    }
    let domain = job.image.expand();
    let account = context.finite_account?;
    let v3 = context.request.finite_replay_initial_queries;
    let _permit = if v3 {
        if !account
            .bindings()
            .is_some_and(|bindings| bindings.matches(job.parent, &domain))
        {
            return None;
        }
        match account.acquire(job.parent, context.cancellation) {
            Ok(Some(permit)) => Some(permit),
            Ok(None) => return None,
            Err(()) => {
                return Some(finite::Outcome::Failed {
                    error: "finite initial query cancelled before admission".into(),
                    cancelled: true,
                    work: serde_json::json!({"status":"cancelled","attempted":false}),
                });
            }
        }
    } else {
        if job.parent != 0 || job.attempts != 0 {
            return None;
        }
        None
    };
    // G2 never applies to a true initial ID; reject a malformed context rather
    // than reinterpreting a partial obligation as a whole-domain summary.
    if !v3
        && (context
            .g2
            .is_some_and(|g2| g2.eligible(job.parent as usize, &domain))
            || (context.request.reuse_initial_d_bands
                && context
                    .overlap
                    .plan(&domain, context.cancellation)
                    .is_some()))
    {
        return None;
    }
    if !v3 && !account.reserve() {
        return None;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if v3 {
            return finite::run(
                context.reducer,
                &domain,
                finite::Recipe {
                    version: finite::INITIAL_QUERIES_VERSION,
                    limits,
                },
                context.cancellation,
                Some(account),
            );
        }
        match finite::original_initial(context.reducer, context.request) {
            Ok(Some(original)) if original == domain => finite::run(
                context.reducer,
                &domain,
                finite::Recipe {
                    version: finite::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION,
                    limits,
                },
                context.cancellation,
                Some(account),
            ),
            Ok(_) => finite::Outcome::Declined {
                work: serde_json::json!({"status":"not_original_whole_initial","attempted":false}),
            },
            Err(error) => finite::Outcome::Failed {
                error,
                cancelled: false,
                work: serde_json::json!({"status":"original_query_error"}),
            },
        }
    }))
    .unwrap_or_else(|_| finite::Outcome::Failed {
        error: "finite replay native attempt panicked".into(),
        cancelled: false,
        work: account.report(),
    });
    account.record(outcome.work().clone());
    account.completed_attempt(&outcome);
    Some(outcome)
}

pub(super) fn finished<const N: usize>(
    job: &Job<N>,
    outcome: &finite::Outcome,
    emit: &mut impl FnMut(inspection::Event<N>) -> ControlFlow<()>,
) -> Option<inspection::Finished> {
    let (events, error, error_kind) = match outcome {
        finite::Outcome::Declined { .. } => return None,
        finite::Outcome::Failed {
            error, cancelled, ..
        } => (
            0,
            Some(error.clone()),
            if *cancelled {
                "cancelled"
            } else {
                "native_failure"
            },
        ),
        finite::Outcome::Closed { .. } => {
            if emit(inspection::Event::one(inspection::Effect::Count)).is_break() {
                (
                    1,
                    Some("finite replay completion publication stopped".into()),
                    "consumer_stop",
                )
            } else {
                (1, None, "none")
            }
        }
    };
    let stats = match job.image.phase() {
        Phase::Apply => inspection::NativeStats::Apply(rustred::solver::OwnerAppliedStats {
            events,
            ..Default::default()
        }),
        Phase::Route => {
            inspection::NativeStats::Route(rustred::solver::CandidateDomainRouteStats {
                events,
                ..Default::default()
            })
        }
    };
    Some(inspection::Finished {
        stats,
        error,
        error_kind,
        seconds: 0.0,
    })
}

pub(super) fn annotate<const N: usize>(result: &mut JobResult<N>, outcome: &finite::Outcome) {
    if let finite::Outcome::Closed { recipe, .. } = outcome {
        // A publication stop must not become a stored success/recipe.
        if result.error.is_none() && result.break_reason == super::super::job::BreakReason::None {
            result.kind = NativeKind::FiniteReplay;
            result.finite_replay = Some(*recipe);
        }
    }
    let mut stats: serde_json::Value =
        serde_json::from_slice(&result.stats_json).expect("native stats JSON");
    stats["finite_replay"] = outcome.work().clone();
    result.stats_json = serde_json::to_vec(&stats).expect("finite replay diagnostics JSON");
}
