//! Mandatory native replay of typed CP6 recipes. Never symbolic fallback.
use super::super::finite_replay::{self as finite, Outcome, Recipe};
#[cfg(test)]
mod tests;
use super::*;

fn binding<const N: usize>(
    ctx: &Ctx<'_, N>,
    id: usize,
    recipe: Recipe,
) -> Result<Domain<N>, String> {
    recipe.validate().map_err(str::to_owned)?;
    if ctx.loaded.epoch.is_none()
        || ctx.request.finite_replay != Some(recipe.limits)
        || ctx.scope_request.finite_replay_version() != Some(recipe.version)
        || id >= ctx.loaded.nodes.len()
        || id >= ctx.loaded.domains.len()
        || !ctx.graph.out(id).is_empty()
        || ctx.loaded.residuals.contains_key(&id)
        || ctx.loaded.g2.contains_key(&id)
    {
        return Err("finite replay is not a bound, whole completed input query".into());
    }
    let domain = ctx.loaded.domains[id].expand();
    match recipe.version {
        finite::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION => {
            if id != 0
                || ctx.loaded.raw.counters[10] == 0
                || ctx.loaded.raw.inputs.first().is_none_or(|row| {
                    row["domain"].as_u64() != Some(0) || row["source_validity_unresolved"] == true
                })
            {
                return Err("legacy finite replay is not original initial ID0".into());
            }
            let original = finite::original_initial(ctx.reducer, ctx.request)?
                .ok_or("finite replay original source frontier")?;
            if domain != original {
                return Err("finite replay original geometry/caps changed".into());
            }
        }
        finite::INITIAL_QUERIES_VERSION => {
            let bindings = finite::InitialBindings::build(
                ctx.reducer,
                ctx.scope_request,
                &ctx.loaded.raw.inputs,
                |index| {
                    ctx.loaded
                        .domains
                        .get(index as usize)
                        .map(|domain| domain.expand())
                },
            )?;
            let index = u32::try_from(id).map_err(|_| "finite initial domain ID range")?;
            if !bindings.matches(index, &domain) {
                return Err(
                    "finite replay does not equal an authenticated whole input query".into(),
                );
            }
        }
        _ => return Err("unsupported finite recipe version".into()),
    }
    let node = ctx.loaded.nodes[id];
    if node.kind != Kind::Native
        || node.error
        || node.frontiers != 0
        || !node.finished
        || node.abandoned
        || node.events != Some(1)
        || node.accepted != Some(1)
    {
        return Err("finite replay is not a whole successful typed completion".into());
    }
    Ok(domain)
}

pub(super) fn reinspect<const N: usize>(
    ctx: &Ctx<'_, N>,
    id: usize,
    recipe: Recipe,
    tally: &mut Tally,
    violations: &mut Violations,
    inventory: Option<&inventory::Collector>,
) {
    let started = Instant::now();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // This is read-only replay: serialization bounds arena memory and does
        // not change publication order. Cancellation remains native and exact.
        let _serial = ctx
            .finite_replay_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let domain = binding(ctx, id, recipe)?;
        // Mutation fixtures may tighten a replay allowance. As with a real
        // changed request, this must fail; never reclassify as ordinary native.
        if ctx.failing.is_some_and(|(failed_id, _)| failed_id == id) {
            return Err("finite replay forced reference failure".into());
        }
        // This branch always executes the exact finite kernel; no symbolic
        // AsRun reuse or matching-only census can substitute for this replay.
        Ok::<_, String>(if let Some(collector) = inventory {
            finite::run_with_census(
                ctx.reducer,
                &domain,
                recipe,
                ctx.cancellation,
                collector.finite_limits(),
            )
        } else {
            finite::run(ctx.reducer, &domain, recipe, ctx.cancellation, None)
        })
    }));
    let mut local = Tally {
        inspected: 1,
        events: 1,
        native_seconds: started.elapsed().as_secs_f64(),
        ..Default::default()
    };
    match outcome {
        Ok(Ok(Outcome::Closed { work, census, .. })) => {
            if let Some(collector) = inventory {
                let result = census
                    .ok_or_else(|| {
                        AppError::internal_invariant(
                            "finite replay completed without requested census",
                        )
                    })
                    .and_then(|census| collector.record_finite(census, ctx.cancellation));
                if let Err(error) = result {
                    local.errors = 1;
                    violations.add("finite_inventory", || error.to_string());
                }
            }
            if recipe.version == finite::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION {
                local.finite_replay_work = Some(work.clone());
            }
            local.finite_replay_cases.insert(id, work);
        }
        Ok(Ok(other)) => {
            local.errors = 1;
            if recipe.version == finite::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION {
                local.finite_replay_work = Some(other.work().clone());
            }
            local.finite_replay_cases.insert(id, other.work().clone());
            violations.add("finite_replay", || {
                "recorded finite summary did not close under cold native replay".into()
            });
        }
        Ok(Err(error)) => {
            local.errors = 1;
            violations.add("finite_replay", || error);
        }
        Err(_) => {
            local.errors = 1;
            violations.add("finite_replay", || "cold finite replay panicked".into());
        }
    }
    tally.add(&local);
}
