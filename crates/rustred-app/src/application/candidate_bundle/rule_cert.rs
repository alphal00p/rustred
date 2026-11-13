//! Exact per-rule IBP certificates of a SAVED candidate program (bebe-cert).
//!
//! No search and no regeneration: the stored rules (with their seed records
//! `RuleRecord::sources`) are decoded, the generation's source system, zero
//! census, permutation and integral order are rebuilt from the bundle exactly
//! as `p-generate` prepared them (`preparation::prepare`, the same routine as
//! `certify_candidates`), and per rule the engine's own
//! `SectorSolver::rule_frame` re-instantiates the seeds, eliminates exactly and
//! REQUIRES the stored rule to be reproduced; its weights are then composed
//! with the checked basis derivation onto the ORIGINAL ordinary IBP rows
//! (`SectorSolver::original_frame`) and multiplied out exactly once more.
//!
//! Output: JSON lines (schema [`RULE_CERTIFICATE_SCHEMA`])
//!   cert_header, source_row* (the original rows, for audit), basis_row* per
//!   chart, then per rule `cert_rule` (frame + original weights, all weight
//!   pole branches classified exception / covered / hidden, positive powers
//!   of the declared dot-product axes on weighted seeds) or `cert_error`, and
//!   a final `cert_end` summary.
//! A rule that is not reproduced is reported, never silently skipped.
//!
//! Batch 6 (helper review y33, fixes 1-6): the rule and its exception branches
//! are those DECODED FROM THE SHIPPED PAYLOAD (never a certificate copy); the
//! weight-pole layers are RECOMPUTED from the weights on the ORIGINAL rows and
//! classified against the payload's stored exception branches (the frame-level
//! classification is kept as `weight_poles_frame`); the positive-dot boundary
//! layers are tested on the original-row weights; exactly one record per rule
//! is required (`complete` in the summary, the CLI fails otherwise). Meaning of
//! a `cert_rule`: for symbolic d, the rule holds at every integer point of its
//! CHART where no stored exception branch vanishes (no zero-sector projection
//! beyond the proved zero census of the header).

use std::io::Write;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientPolynomial};
use rustred::identity::ParametricIbpGenerator;
use rustred::solver::{
    Integral, RuleCandidate, SectorConfig, SectorSolver, Term, extract_exceptions,
};
use serde_json::{Value, json};
use symbolica::prelude::Factorize;

use crate::application::AppError;

use super::{codec, model::*, preparation};

/// Schema of the certificate JSON lines (v2, batch 6: original-row weight
/// poles and dot layers, per-rule completeness).
pub const RULE_CERTIFICATE_SCHEMA: &str = "rustred.p-rule-certificate.v2";

/// The statement a `cert_rule` record certifies (fix 6 of the y33 review).
pub const RULE_CERTIFICATE_STATEMENT: &str = "for symbolic d, the rule holds at every integer point of its CHART where no stored exception branch vanishes; weight-pole layers are recomputed from the ORIGINAL-row weights and must each be a stored exception branch or implied by one (status 'hidden' otherwise); the rule and its exceptions are decoded from the shipped payload";

/// What to certify and what to flag.
#[derive(Clone, Debug, Default)]
pub struct RuleCertificateOptions {
    /// Axes whose slot is a non-square (dot-product) form: a weighted seed
    /// with a positive power there is flagged (reviewer condition (a)).
    pub dot_axes: Vec<usize>,
    /// Certify only these table indices (all when empty).
    pub only: Vec<usize>,
    /// Free text recorded in the header (payload path, digest, ...).
    pub provenance: Value,
}

/// Counts of one certification run.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct RuleCertificateSummary {
    pub rules: usize,
    pub certified: usize,
    pub not_reproduced: usize,
    pub failed: usize,
    pub weights: usize,
    pub original_weights: usize,
    pub rules_with_weight_poles: usize,
    pub weight_pole_branches: usize,
    pub branches_exception: usize,
    pub branches_covered: usize,
    pub branches_hidden: usize,
    pub rules_with_hidden_branches: usize,
    pub rules_with_positive_dot_seed: usize,
    /// Batch 6: frame-level hidden branches (the original-row ones are
    /// `branches_hidden`).
    pub branches_hidden_frame: usize,
    /// Batch 6: positive-dot boundary layers of weighted ORIGINAL-row seeds
    /// (`n_a` in `[1-o, 0]` for an offset `o >= 1` on a dot axis).
    pub dot_layers: usize,
    pub dot_layers_excluded: usize,
    pub dot_layers_weight_zero: usize,
    pub dot_layers_exposed: usize,
    /// Weighted original-row seeds at a FIXED positive power of a dot axis.
    pub dot_fixed_positive: usize,
    /// Rules of the payload (selected by `only`) without exactly one record.
    pub missing_records: usize,
    /// Exactly one `cert_rule` per selected payload rule, none failed, no
    /// hidden layer, no exposed dot layer.
    pub complete: bool,
    pub seconds: f64,
}

/// Status of one weight-pole branch against the stored exception branches.
fn classify(
    branch: &[CoefficientPolynomial],
    exceptions: &[Vec<CoefficientPolynomial>],
) -> &'static str {
    if exceptions.iter().any(|e| e.as_slice() == branch) {
        "exception"
    } else if exceptions
        .iter()
        .any(|e| !e.is_empty() && e.iter().all(|p| branch.contains(p)))
    {
        "covered"
    } else {
        "hidden"
    }
}

fn q(text: &str) -> Value {
    Value::String(text.to_owned())
}

fn integral_json<const N: usize>(integral: &Integral<N>, arity: usize) -> Value {
    Value::Array(
        integral.powers()[..arity]
            .iter()
            .map(|p| json!([u8::from(p.is_symbolic()), p.value()]))
            .collect(),
    )
}

fn polynomial_factors(polynomial: &CoefficientPolynomial) -> Value {
    if polynomial.is_constant() {
        return json!([[polynomial.to_string(), 1]]);
    }
    Value::Array(
        polynomial
            .factor()
            .into_iter()
            .map(|(factor, power)| json!([factor.to_string(), power]))
            .collect(),
    )
}

fn coefficient_json(value: &Coefficient) -> Value {
    json!({
        "num": value.numerator.to_string(),
        "den": value.denominator.to_string(),
        "den_factors": polynomial_factors(&value.denominator),
    })
}

fn branch_json(branch: &[CoefficientPolynomial]) -> Value {
    Value::Array(branch.iter().map(|p| q(&p.to_string())).collect())
}

fn line(out: &mut impl Write, value: &Value) -> Result<(), AppError> {
    writeln!(out, "{value}").map_err(|e| AppError::serialization(e.to_string()))
}

/// Certify every (selected) rule of the saved candidate program `bytes`,
/// writing the JSON lines to `out`.
pub fn rule_certificates(
    bytes: &[u8],
    limits: CandidateBundleLimits,
    options: &RuleCertificateOptions,
    out: &mut impl Write,
) -> Result<RuleCertificateSummary, AppError> {
    let bundle = codec::read(bytes, limits)?;
    let family = bundle
        .family
        .to_family(
            &bundle.coefficients,
            limits.family_limits(),
            limits.binary_limits(),
        )
        .map_err(codec::binary_error)?;
    if family.fingerprint() != bundle.family_fingerprint
        || family.denominator_count() != bundle.root_sector.len()
    {
        return Err(AppError::input(
            "candidate family binding differs from reconstructed family",
        ));
    }
    macro_rules! dispatch {
        ($($n:literal),*) => { match rustred::campaign_storage_arity(family.denominator_count()) {
            $($n => certify::<$n>(family, bundle, options, out),)*
            _ => Err(crate::AppError::input("campaign arity is not compiled")),
        } };
    }
    crate::ensure_runtime_arity(family.denominator_count())?;
    rustred::with_app_runtime_arities!(dispatch)
}

fn certify<const N: usize>(
    family: rustred::family::IntegralFamily,
    bundle: Bundle,
    options: &RuleCertificateOptions,
    out: &mut impl Write,
) -> Result<RuleCertificateSummary, AppError> {
    let started = Instant::now();
    let arity = family.denominator_count();
    let prepared =
        preparation::prepare::<N>(family, &bundle.root_sector, bundle.permutation.as_deref())?;
    let context = ParametricIbpGenerator::try_new(&prepared.family)
        .map_err(|e| AppError::from_native(e, crate::AppErrorKind::Execution))?
        .context()
        .clone();
    let solved = codec::solutions::<N>(
        &bundle,
        &context,
        prepared.sources.index_variables(),
        CandidateBundleLimits::default(),
    )?;
    let sources = &prepared.sources;
    let (loops, externals) = (
        prepared.family.loop_count(),
        prepared.family.external_count(),
    );
    let mask = |sector: &[bool; N]| -> String {
        sector[..arity]
            .iter()
            .map(|&b| if b { '1' } else { '0' })
            .collect()
    };
    line(
        out,
        &json!({
            "type": "cert_header",
            "schema": RULE_CERTIFICATE_SCHEMA,
            "family": prepared.family.name(),
            "family_fingerprint": bundle.family_fingerprint,
            "solver_policy": bundle.solver_policy,
            "status": bundle.status,
            "loops": loops,
            "externals": externals,
            "arity": arity,
            "root": mask(&prepared.root),
            "permutation": bundle.permutation,
            "source_rows": sources.rows().len(),
            "conditions": sources.conditions().iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            "zero_sectors": prepared.zeros.iter().map(|z| mask(z)).collect::<Vec<_>>(),
            "dot_axes": options.dot_axes,
            "statement": RULE_CERTIFICATE_STATEMENT,
            "row_convention": "m = differentiated_loop * (externals + loops) + c; contraction c = 0.. over reversed externals then loops k1..kL (momentum index j: j < loops is k_{j+1}, j >= loops the external)",
            "integral_convention": "integral = [[symbolic, value]] per axis: symbolic -> n_axis + value, else the fixed power value",
            "identity": "rule := I(target) - sum_t c_t I(t) == sum_frame w_j B_k[at_j] == sum_original W R_m[at] modulo proved-zero sectors; R_m[at]: each term c(n) I(offset) of row m becomes c(n_a -> n_a + at_a on symbolic axes, at_a on fixed axes) I(at + offset)",
            "provenance": options.provenance,
        }),
    )?;
    let contraction: Vec<usize> = (loops..loops + externals).rev().chain(0..loops).collect();
    for (m, row) in sources.rows().iter().enumerate() {
        let terms: Vec<Value> = row
            .iter()
            .map(|t| json!([integral_json(&t.integral, arity), t.coefficient.to_string()]))
            .collect();
        line(
            out,
            &json!({
                "type": "source_row",
                "m": m,
                "differentiated_loop": m / contraction.len(),
                "contraction_momentum": contraction[m % contraction.len()],
                "terms": terms,
            }),
        )?;
    }
    let mut summary = RuleCertificateSummary::default();
    let mut index = 0usize;
    for (sector, solution) in &solved {
        let label = mask(sector);
        let config = SectorConfig {
            zero_sectors: prepared.zeros.clone(),
            permutation: prepared.permutation,
            integral_order: solution.order.program().cloned(),
            ..SectorConfig::default()
        };
        let (solver, derivation) = match SectorSolver::with_basis_derivation(
            sources, *sector, config,
        ) {
            Ok(x) => x,
            Err(e) => {
                line(
                    out,
                    &json!({"type": "cert_chart_error", "chart": label, "error": format!("{e:?}")}),
                )?;
                summary.rules += solution.rules.len();
                summary.failed += solution.rules.len();
                index += solution.rules.len();
                continue;
            }
        };
        for (k, row) in derivation.rows.iter().enumerate() {
            let entries: Vec<Value> = row
                .iter()
                .map(|(m, p)| json!([m, coefficient_json(p)]))
                .collect();
            line(
                out,
                &json!({"type": "basis_row", "chart": label, "k": k, "sources": entries}),
            )?;
        }
        for (ordinal, rule) in solution.rules.iter().enumerate() {
            let table_index = index;
            index += 1;
            if !options.only.is_empty() && !options.only.contains(&table_index) {
                continue;
            }
            summary.rules += 1;
            let t0 = Instant::now();
            let candidate = &rule.candidate;
            let fixed: Vec<Value> = (0..arity)
                .filter_map(|a| candidate.case.fixed()[a].map(|v| json!([a, v])))
                .collect();
            let base = json!({
                "chart": label,
                "rule": ordinal,
                "index": table_index,
                "fixed": fixed,
                "affine": candidate.case.affine().is_some(),
                "trace_rows": candidate.sources.len(),
                "rhs_terms": candidate.rhs.len(),
            });
            let frame = match solver.rule_frame(rule) {
                Ok(frame) => frame,
                Err(e) => {
                    let text = format!("{e:?}");
                    let reproduced = !text.contains("no pivot reproducing the rule");
                    if reproduced {
                        summary.failed += 1;
                    } else {
                        summary.not_reproduced += 1;
                    }
                    let mut record = base.clone();
                    record["type"] = q("cert_error");
                    record["stage"] = q("rule_frame");
                    record["not_reproduced"] = json!(!reproduced);
                    record["error"] = q(&text.chars().take(2000).collect::<String>());
                    line(out, &record)?;
                    continue;
                }
            };
            let original = match solver.original_frame(rule, &frame, &derivation) {
                Ok(entries) => entries,
                Err(e) => {
                    summary.failed += 1;
                    let mut record = base.clone();
                    record["type"] = q("cert_error");
                    record["stage"] = q("original_frame");
                    record["error"] = q(&format!("{e:?}").chars().take(2000).collect::<String>());
                    line(out, &record)?;
                    continue;
                }
            };
            // Reviewer condition (b), batch 6: the weight-pole layers are
            // recomputed from the ORIGINAL-row weights (fix 2) and classified
            // against the payload's stored exception branches; the frame-level
            // classification is kept for comparison.
            let exceptions = &rule.exceptions.branches;
            let indices = sources.index_variables();
            let original_candidate = RuleCandidate {
                case: candidate.case.clone(),
                target: candidate.target,
                rhs: original
                    .iter()
                    .filter(|entry| !entry.weight.is_zero())
                    .map(|entry| Term {
                        integral: candidate.target,
                        coefficient: entry.weight.clone(),
                    })
                    .collect(),
                sources: Vec::new(),
                stats: Default::default(),
                hidden_poles: Vec::new(),
            };
            let original_poles = match extract_exceptions(&original_candidate, indices, solver.ordering().sector()) {
                Ok(conditions) => conditions.branches,
                Err(e) => {
                    summary.failed += 1;
                    let mut record = base.clone();
                    record["type"] = q("cert_error");
                    record["stage"] = q("original_weight_poles");
                    record["error"] = q(&format!("{e:?}").chars().take(2000).collect::<String>());
                    line(out, &record)?;
                    continue;
                }
            };
            let mut classified = Vec::new();
            let (mut hidden, mut covered, mut exact) = (0, 0, 0);
            for branch in &original_poles {
                let status = classify(branch, exceptions);
                match status {
                    "exception" => exact += 1,
                    "covered" => covered += 1,
                    _ => hidden += 1,
                }
                classified.push(json!({"branch": branch_json(branch), "status": status}));
            }
            let mut classified_frame = Vec::new();
            let mut hidden_frame = 0;
            for branch in &frame.all_weight_poles {
                let status = classify(branch, exceptions);
                hidden_frame += usize::from(status == "hidden");
                classified_frame.push(json!({"branch": branch_json(branch), "status": status}));
            }
            // Reviewer condition (a), batch 6 (fix 3): positive powers of the
            // dot-product axes on ORIGINAL-row seeds with a nonzero weight.
            // Fixed power >= 1: a violation. Symbolic offset o >= 1: the
            // layers n_a = 1-o..0 (free ISP domain n_a <= 0) must each be
            // excluded by a stored exception branch or carry a weight that
            // vanishes there; otherwise the layer is exposed.
            let mut positive_dot = Vec::new();
            let mut exposed_layers = 0;
            for entry in &original {
                for &axis in &options.dot_axes {
                    if axis >= arity {
                        continue;
                    }
                    let p = entry.at[axis];
                    if p.value() < 1 {
                        continue;
                    }
                    if !p.is_symbolic() {
                        summary.dot_fixed_positive += 1;
                        exposed_layers += 1;
                        positive_dot.push(json!({
                            "row": entry.row, "axis": axis, "symbolic": false,
                            "value": p.value(), "status": "fixed_positive"}));
                        continue;
                    }
                    let variable = indices[axis];
                    for v in (1 - i64::from(p.value()))..=0 {
                        let at_layer = |polynomial: &CoefficientPolynomial| {
                            polynomial
                                .replace(variable, &symbolica::prelude::Integer::from(v))
                                .is_zero()
                        };
                        summary.dot_layers += 1;
                        let status = if exceptions
                            .iter()
                            .any(|branch| !branch.is_empty() && branch.iter().all(at_layer))
                        {
                            summary.dot_layers_excluded += 1;
                            "excluded"
                        } else if at_layer(&entry.weight.numerator) {
                            summary.dot_layers_weight_zero += 1;
                            "weight_zero"
                        } else {
                            summary.dot_layers_exposed += 1;
                            exposed_layers += 1;
                            "exposed"
                        };
                        positive_dot.push(json!({
                            "row": entry.row, "axis": axis, "symbolic": true,
                            "offset": p.value(), "layer": v, "status": status}));
                    }
                }
            }
            summary.certified += 1;
            summary.weights += frame.entries.len();
            summary.original_weights += original.len();
            summary.weight_pole_branches += original_poles.len();
            summary.branches_hidden_frame += hidden_frame;
            summary.branches_exception += exact;
            summary.branches_covered += covered;
            summary.branches_hidden += hidden;
            summary.rules_with_weight_poles += usize::from(!original_poles.is_empty());
            summary.rules_with_hidden_branches += usize::from(hidden > 0);
            summary.rules_with_positive_dot_seed += usize::from(!positive_dot.is_empty());
            let _ = exposed_layers;
            let mut record = base;
            record["type"] = q("cert_rule");
            record["via"] = q(frame.via);
            record["recenter"] = json!(&frame.recenter[..arity]);
            record["zero_weights"] = json!(frame.zero_weights);
            record["target"] = integral_json(&candidate.target, arity);
            record["rhs"] = Value::Array(
                candidate
                    .rhs
                    .iter()
                    .map(|t| {
                        json!([
                            integral_json(&t.integral, arity),
                            t.coefficient.to_string(),
                            coefficient_json(&t.coefficient)
                        ])
                    })
                    .collect(),
            );
            record["exceptions"] =
                Value::Array(exceptions.iter().map(|b| branch_json(b)).collect());
            record["weight_poles"] = Value::Array(classified);
            record["weight_poles_frame"] = Value::Array(classified_frame);
            record["hidden_branches"] = json!(hidden);
            record["hidden_branches_frame"] = json!(hidden_frame);
            record["positive_dot_seeds"] = Value::Array(positive_dot);
            record["frame"] = Value::Array(
                frame
                    .entries
                    .iter()
                    .map(|e| {
                        json!({
                            "source": e.source,
                            "k": e.basis_row,
                            "at": integral_json(&e.at, arity),
                            "w": coefficient_json(&e.weight),
                        })
                    })
                    .collect(),
            );
            record["original"] = Value::Array(
                original
                    .iter()
                    .map(|e| {
                        json!({
                            "m": e.row,
                            "seed": integral_json(&e.seed.integral, arity),
                            "shifts": &e.seed.shifts[..arity],
                            "at": integral_json(&e.at, arity),
                            "w": coefficient_json(&e.weight),
                        })
                    })
                    .collect(),
            );
            record["seconds"] = json!(t0.elapsed().as_secs_f64());
            line(out, &record)?;
            out.flush()
                .map_err(|e| AppError::serialization(e.to_string()))?;
        }
    }
    // Fix 5: one record per selected payload rule is REQUIRED.
    let selected = if options.only.is_empty() {
        index
    } else {
        options.only.iter().filter(|&&i| i < index).count()
    };
    summary.missing_records = selected.saturating_sub(summary.rules);
    summary.complete = summary.missing_records == 0
        && summary.rules == summary.certified
        && summary.branches_hidden == 0
        && summary.dot_layers_exposed == 0
        && summary.dot_fixed_positive == 0;
    summary.seconds = started.elapsed().as_secs_f64();
    let mut end =
        serde_json::to_value(&summary).map_err(|e| AppError::serialization(e.to_string()))?;
    end["type"] = q("cert_end");
    line(out, &end)?;
    out.flush()
        .map_err(|e| AppError::serialization(e.to_string()))?;
    Ok(summary)
}
