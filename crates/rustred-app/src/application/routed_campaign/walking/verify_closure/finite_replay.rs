//! Mandatory native replay of typed CP6 recipes. Never symbolic fallback.
use super::super::finite_replay::{self as finite, Outcome, Recipe};
#[cfg(test)]
mod tests;
use super::*;

fn binding<const N: usize>(ctx: &Ctx<'_, N>, recipe: Recipe) -> Result<Domain<N>, String> {
    recipe.validate().map_err(str::to_owned)?;
    if ctx.loaded.epoch.is_none()
        || ctx.request.finite_replay != Some(recipe.limits)
        || ctx.loaded.raw.counters[10] == 0
        || ctx.loaded.nodes.is_empty()
        || ctx.loaded.raw.inputs.first().is_none_or(|row| {
            row["domain"].as_u64() != Some(0) || row["source_validity_unresolved"] == true
        })
        || !ctx.graph.out(0).is_empty()
        || ctx.loaded.residuals.contains_key(&0)
        || ctx.loaded.g2.contains_key(&0)
    {
        return Err("finite replay is not the bound, whole original initial ID0".into());
    }
    let original = finite::original_initial(ctx.reducer, ctx.request)?
        .ok_or("finite replay original source frontier")?;
    if ctx.loaded.domains[0].expand() != original {
        return Err("finite replay original geometry/caps changed".into());
    }
    let node = ctx.loaded.nodes[0];
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
    Ok(original)
}

pub(super) fn reinspect<const N: usize>(
    ctx: &Ctx<'_, N>,
    recipe: Recipe,
    tally: &mut Tally,
    violations: &mut Violations,
) {
    let started = Instant::now();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let domain = binding(ctx, recipe)?;
        // Mutation fixtures may tighten a replay allowance. As with a real
        // changed request, this must fail; never reclassify as ordinary native.
        if ctx.failing.is_some_and(|(id, _)| id == 0) {
            return Err("finite replay forced reference failure".into());
        }
        Ok::<_, String>(finite::run(
            ctx.reducer,
            &domain,
            recipe,
            ctx.cancellation,
            None,
        ))
    }));
    let mut local = Tally {
        inspected: 1,
        events: 1,
        native_seconds: started.elapsed().as_secs_f64(),
        ..Default::default()
    };
    match outcome {
        Ok(Ok(Outcome::Closed { work, .. })) => {
            local.finite_replay_work = Some(work);
        }
        Ok(Ok(other)) => {
            local.errors = 1;
            local.finite_replay_work = Some(other.work().clone());
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
