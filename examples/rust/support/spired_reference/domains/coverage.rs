//! Conservative exact domain coverage for reference-only redundant rules.
//!
//! This runs only after every retained candidate has matched its own reference
//! RHS and guards. It never parses a reference RHS into a solver input.

use super::*;
use rustred::solver::CaseIntersectionLimits;

pub(super) fn covered_by_matched_rules<const N: usize>(
    reference: &ReferenceEntry<'_, N>,
    matched: &[SectorRule<N>],
    system: &SourceSystem<N>,
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
) -> Result<bool> {
    let reference_excluded = reference_exceptions(reference, system, sector)?;
    let matched_domains = matched
        .iter()
        .map(|rule| {
            Ok((
                &rule.candidate.case,
                rule.exceptional_cases(system.index_variables(), sector)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let template = template(system)?;
    let mut pending = vec![reference.case.clone()];
    let mut work = 0;
    while let Some(case) = pending.pop() {
        if work >= limits.max_work_items {
            return Err(invalid(
                "reference-only coverage exceeded its geometry budget",
            ));
        }
        work += 1;
        if reference_excluded
            .iter()
            .try_fold(false, |covered, excluded| {
                Ok::<_, Box<dyn Error>>(covered || excluded.contains(&case)?)
            })?
        {
            continue;
        }
        let mut remaining = None;
        for (required, excluded) in &matched_domains {
            if !required.contains(&case)? {
                // Subtracting a proper affine subcase would need inequalities.
                // Do not pretend that testing its intersection covers this case.
                continue;
            }
            if excluded.iter().try_fold(false, |blocked, excluded| {
                Ok::<_, Box<dyn Error>>(blocked || excluded.contains(&case)?)
            })? {
                // This rule is excluded throughout the remaining case and
                // cannot make progress, even if its required domain contains it.
                continue;
            }
            let mut holes = Vec::new();
            for exclusion in excluded {
                let mut equations = exclusion
                    .affine()
                    .map_or_else(Vec::new, |affine| affine.equations().to_vec());
                for (axis, fixed) in exclusion.fixed().iter().enumerate() {
                    if let Some(value) = fixed {
                        let variable = template
                            .variable(&template.variables()[system.index_variables()[axis]])
                            .expect("validated source coordinate");
                        equations.push(&variable - &template.constant(Integer::from(*value)));
                    }
                }
                let intersection = case.intersect_many(
                    &equations,
                    system.index_variables(),
                    sector,
                    CaseIntersectionLimits {
                        max_work_items: limits.max_work_items - work,
                        ..limits
                    },
                )?;
                work += intersection.stats.work_items;
                holes.extend(intersection.cases);
            }
            holes.sort_by(Case::queue_cmp);
            holes.dedup();
            // For C contained in the rule's required case R, removing R\E
            // leaves exactly C intersect E. Keep EVERY exceptional component.
            remaining = Some(holes);
            break;
        }
        let Some(holes) = remaining else {
            return Ok(false);
        };
        pending.extend(holes.into_iter().rev());
        if pending.len() > limits.max_work_items - work {
            return Err(invalid(
                "reference-only coverage exceeded its geometry budget",
            ));
        }
    }
    Ok(true)
}
