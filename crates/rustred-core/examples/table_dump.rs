//! Knock-out (rules study) table probe; built from rustred p_integral (rr-ko-tooling).
//!
//!   table_dump FAMILY MASK ORDER OUT.jsonl [--cert OUT.cert.jsonl] [--depth K] [--zero all|downset]
//!              [--workers W] [--charts M1,M2,..] [--cert-max-rows K] [--defer-unsupported]
//!              [--case-gsplit]
//!
//! Solves every chart of the sector MASK of FAMILY (one chart without two-sided axes, 2^j with j axes)
//! and writes the complete machine table as JSONL. rules_study/s5-tables/FORMAT.md is the format.
//! MASK = 0/1 string, or `top` = the prop(..,1) lines of the family file.
//! ORDER = auto | legacy | two_sided | eps_first | eps_third | plain_two_sided | one_sided
//!   auto = legacy without eps axes, two_sided with them.
//!
//! `--defer-unsupported` (opt-in S4, off by default; without it the output is unchanged): an
//! exceptional branch that the case intersection cannot resolve no longer fails the chart. It is
//! written as a `deferred_region` record (after the chart's rules and residuals) and counted in
//! `chart_end`/`end` (`deferred_regions`); the header schema is then `table_dump.v1-defer` and
//! carries `"defer_unsupported":true`. Contract: an integer point of the chart (its guards) at which
//! every polynomial of a region's `predicates` vanishes is NOT reduced by any rule of the chart;
//! it must come from a separate exact table, else fail closed. Certificates are unchanged.
//!
//! `--case-gsplit` (opt-in, requires `--defer-unsupported`; off by default): the exceptional-case
//! admission of every found rule uses the exact factor-first + saturation + G-split refinement of
//! the case intersection (`CaseRefinement::all()`). The header schema is then
//! `table_dump.v1-defer-gsplit` with `"case_gsplit":true`, and every `deferred_region` record also
//! carries `exclusions` (the region holds only points where EVERY exclusion is nonzero; points of
//! the predicates where an exclusion vanishes belong to ordinary cases/rules or to other regions)
//! and `dependents` (informational: `n_axis = numerator/denominator` on the region, triangular).
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;
use std::time::Instant;

use rustred::family::IntegralFamily;
use rustred::input::{Compiler, Limits, LoweringLimits};
use rustred::order::{CompiledOrder, DegreeRow, OrderDescriptor};
use rustred::sector::{Mask, zero};
use rustred::solver::{
    Case, CaseRefinement, Integral, SectorConfig, SectorDeferredRegion, SectorEvent,
    SectorExecutionError, SectorExecutor, SectorRule, SectorSolveOptions, SectorSolver,
    SourceSystem,
};

/// Bump on any change of the record layout (FORMAT.md).
const SCHEMA: &str = "table_dump.v1";
/// Table schema with `--defer-unsupported` (v1 plus `deferred_region` records and the
/// `deferred_regions` counters). A v1-only reader refuses it instead of treating the deferred
/// points as covered. The certificate file keeps [`SCHEMA`]: its layout is unchanged.
const SCHEMA_DEFER: &str = "table_dump.v1-defer";
/// Table schema with `--defer-unsupported --case-gsplit`: v1-defer plus `exclusions` and
/// `dependents` in every `deferred_region`. A v1-defer reader refuses it (it would ignore the
/// exclusions).
const SCHEMA_DEFER_GSPLIT: &str = "table_dump.v1-defer-gsplit";
/// The source line of the build. Was "94ec3300" for the rules-s5-tables study
/// builds (B3, B5: rustred 94ec3300 plus the example and solver/table_export.rs).
const SOURCE_BASE: &str = "p_integral";

fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => write!(out, "\\u{:04x}", c as u32).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn strings<T: std::fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let parts: Vec<String> = items.into_iter().map(|x| q(&x.to_string())).collect();
    format!("[{}]", parts.join(","))
}

fn mask_string(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

fn degree_row(row: &DegreeRow) -> String {
    format!(
        "{{\"active\":{:?},\"inactive\":{:?}}}",
        row.active, row.inactive
    )
}

fn descriptor_json(d: &OrderDescriptor) -> String {
    let rows = |r: &[DegreeRow]| r.iter().map(degree_row).collect::<Vec<_>>().join(",");
    format!(
        "{{\"pre_support_degree_rows\":[{}],\"support_weights\":{:?},\"support_priority\":{:?},\"degree_rows\":[{}],\"coordinate_priority\":{:?},\"coordinate_groups\":{},\"active_direction\":{},\"inactive_direction\":{},\"two_sided\":{:?}}}",
        rows(&d.pre_support_degree_rows),
        d.support_weights,
        d.support_priority,
        rows(&d.degree_rows),
        d.coordinate_priority,
        q(&format!("{:?}", d.coordinate_groups)),
        q(&format!("{:?}", d.active_direction)),
        q(&format!("{:?}", d.inactive_direction)),
        d.two_sided
    )
}

fn vmhwm_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmHWM"))
                .and_then(|l| l.split_whitespace().nth(1).map(|v| v.parse().unwrap_or(0)))
        })
        .unwrap_or(0)
}

struct Cli {
    family_path: String,
    family_text: String,
    mask: Vec<bool>,
    order: String,
    out: String,
    cert: Option<String>,
    depth: u32,
    zero_scope: String,
    workers: usize,
    charts: Option<Vec<Vec<bool>>>,
    cert_max_rows: usize,
    defer_unsupported: bool,
    case_gsplit: bool,
}

fn cli() -> (Cli, IntegralFamily) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 5 {
        eprintln!(
            "usage: table_dump FAMILY MASK ORDER OUT.jsonl [--cert F] [--depth K] [--zero all|downset] [--workers W] [--charts M1,M2] [--cert-max-rows K] [--defer-unsupported] [--case-gsplit]"
        );
        std::process::exit(2);
    }
    let family_text = std::fs::read_to_string(&args[1]).expect("read family");
    let family = Compiler::new(Limits::default())
        .expect("compiler")
        .compile_compact(&family_text, None)
        .expect("compile compact family")
        .into_lowered(LoweringLimits::default())
        .expect("lower family")
        .into_family();
    let n = family.denominator_count();
    let mask: Vec<bool> = if args[2] == "top" {
        let lines: Vec<bool> = family_text
            .lines()
            .filter(|l| l.trim_start().starts_with("prop("))
            .map(|l| l.trim_end().trim_end_matches(',').ends_with(",1)"))
            .collect();
        assert_eq!(lines.len(), n, "prop lines of the family file");
        lines
    } else {
        args[2].chars().map(|c| c == '1').collect()
    };
    assert_eq!(mask.len(), n, "mask length");
    let mut c = Cli {
        family_path: args[1].clone(),
        family_text,
        mask,
        order: args[3].clone(),
        out: args[4].clone(),
        cert: None,
        depth: 2,
        zero_scope: if n <= 16 {
            "all".into()
        } else {
            "downset".into()
        },
        workers: 1,
        charts: None,
        cert_max_rows: usize::MAX,
        defer_unsupported: false,
        case_gsplit: false,
    };
    let mut i = 5;
    while i < args.len() {
        if args[i] == "--defer-unsupported" {
            c.defer_unsupported = true;
            i += 1;
            continue;
        }
        if args[i] == "--case-gsplit" {
            c.case_gsplit = true;
            i += 1;
            continue;
        }
        let value = args.get(i + 1).expect("flag value").clone();
        match args[i].as_str() {
            "--cert" => c.cert = Some(value),
            "--depth" => c.depth = value.parse().unwrap(),
            "--zero" => c.zero_scope = value,
            "--workers" => c.workers = value.parse().unwrap(),
            "--charts" => {
                c.charts = Some(
                    value
                        .split(',')
                        .map(|m| m.chars().map(|x| x == '1').collect())
                        .collect(),
                )
            }
            "--cert-max-rows" => c.cert_max_rows = value.parse().unwrap(),
            other => panic!("unknown flag {other}"),
        }
        i += 2;
    }
    assert!(
        !c.case_gsplit || c.defer_unsupported,
        "--case-gsplit requires --defer-unsupported"
    );
    (c, family)
}

/// Returns (descriptor to record, program for the solver, name).
fn order_for<const N: usize>(
    name: &str,
    axes: &[bool; N],
    n: usize,
) -> (OrderDescriptor, Option<CompiledOrder>, String) {
    let any = axes.iter().any(|&a| a);
    let resolved = match name {
        "auto" if any => "two_sided",
        "auto" => "legacy",
        other => other,
    };
    let eps_row = DegreeRow {
        active: axes[..n].iter().map(|&a| u64::from(a)).collect(),
        inactive: vec![0; n],
    };
    let descriptor = match resolved {
        "legacy" | "one_sided" => OrderDescriptor::spired_uncut(n),
        "two_sided" => OrderDescriptor::spired_uncut_two_sided(axes[..n].to_vec()),
        "eps_first" | "eps_third" => {
            let mut d = OrderDescriptor::spired_uncut(n);
            d.two_sided = axes[..n].to_vec();
            d.degree_rows
                .insert(if resolved == "eps_first" { 0 } else { 2 }, eps_row);
            d
        }
        "plain_two_sided" => {
            let mut d = OrderDescriptor::spired_uncut(n);
            d.two_sided = axes[..n].to_vec();
            d
        }
        other => panic!("unknown order {other}"),
    };
    let program = if resolved == "legacy" || resolved == "one_sided" {
        None
    } else {
        Some(CompiledOrder::compile(descriptor.clone(), Default::default()).expect("order"))
    };
    (descriptor, program, resolved.to_string())
}

fn write_rule<const N: usize>(
    f: &mut impl std::io::Write,
    chart: &str,
    index: usize,
    rule: &SectorRule<N>,
) -> usize {
    let c = &rule.candidate;
    let rhs: Vec<String> = c
        .rhs
        .iter()
        .map(|t| {
            format!(
                "[{},{}]",
                q(&t.integral.to_string()),
                q(&t.coefficient.to_string())
            )
        })
        .collect();
    let exceptions: Vec<String> = rule
        .exceptions
        .branches
        .iter()
        .map(|b| strings(b.iter()))
        .collect();
    let (fixed, equations) = case_fields(&c.case);
    let mut seeds: Vec<String> = c
        .sources
        .iter()
        .map(|s| s.seed.integral.to_string())
        .collect();
    seeds.sort();
    seeds.dedup();
    writeln!(
        f,
        "{{\"type\":\"rule\",\"chart\":{},\"index\":{},\"case\":{},\"fixed\":[{}],\"equations\":{},\"target\":{},\"rhs\":[{}],\"exceptions\":[{}],\"dispatch\":{},\"nsources\":{},\"seeds\":{},\"direct_hit\":{},\"seconds_search\":{:.6}}}",
        q(chart),
        index,
        q(&format!("{:?}", c.case)),
        fixed,
        equations,
        q(&c.target.to_string()),
        rhs.join(","),
        exceptions.join(","),
        q(&format!("{:?}", rule.dispatch_policy)),
        c.sources.len(),
        strings(seeds.iter()),
        c.stats.direct_hit,
        c.stats.elapsed.as_secs_f64()
    )
    .unwrap();
    c.rhs.len()
}

/// `fixed` (without brackets) and `equations` of a case, as in `rule` records.
fn case_fields<const N: usize>(case: &Case<N>) -> (String, String) {
    let fixed: Vec<String> = case
        .fixed()
        .iter()
        .map(|v| v.map_or("null".to_string(), |x| x.to_string()))
        .collect();
    let equations = match case {
        Case::Coordinate(_) => "[]".to_string(),
        Case::Affine(a) => strings(a.equations().iter()),
    };
    (fixed.join(","), equations)
}

/// One `deferred_region` record (S4, `--defer-unsupported`).
fn write_deferred<const N: usize>(
    f: &mut impl std::io::Write,
    chart: &str,
    index: usize,
    deferred: &SectorDeferredRegion<N>,
    rules: &[SectorRule<N>],
    indices: &[usize; N],
    case_gsplit: bool,
) {
    let region = &deferred.region;
    let (fixed, equations) = case_fields(&region.chart);
    let (parent_fixed, parent_equations) = case_fields(&region.parent);
    let rule = &rules[deferred.rule];
    let branch = &rule.exceptions.branches[deferred.branch];
    let predicates = region
        .predicates(indices)
        .expect("an engine-deferred region has a nonempty conjunction");
    let gsplit_fields = if case_gsplit {
        let dependents = region
            .dependents
            .iter()
            .map(|dependent| {
                format!(
                    "{{\"axis\":{},\"numerator\":{},\"denominator\":{}}}",
                    dependent.axis,
                    q(&dependent.numerator.to_string()),
                    q(&dependent.denominator.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            ",\"exclusions\":{},\"dependents\":[{dependents}]",
            strings(region.exclusions.iter())
        )
    } else {
        String::new()
    };
    writeln!(
        f,
        "{{\"type\":\"deferred_region\",\"chart\":{},\"index\":{index},\"rule\":{},\"branch\":{},\"reason\":{},\"parent_is_rule_case\":{},\"parent_case\":{},\"parent_fixed\":[{}],\"parent_equations\":{},\"branch_conjunction\":{},\"case\":{},\"fixed\":[{}],\"equations\":{},\"conjunction\":{},\"predicates\":{}{gsplit_fields}}}",
        q(chart),
        deferred.rule,
        deferred.branch,
        q(&format!("{:?}", region.reason)),
        region.parent == rule.candidate.case,
        q(&format!("{:?}", region.parent)),
        parent_fixed,
        parent_equations,
        strings(branch.iter()),
        q(&format!("{:?}", region.chart)),
        fixed,
        equations,
        strings(region.conjunction.iter()),
        strings(predicates.iter())
    )
    .unwrap();
}

fn write_cert_header<const N: usize>(
    f: &mut impl std::io::Write,
    fam: &IntegralFamily,
    sources: &SourceSystem<N>,
) {
    let (loops, externals) = (fam.loop_count(), fam.external_count());
    writeln!(
        f,
        "{{\"type\":\"cert_header\",\"schema\":{},\"family\":{},\"loops\":{loops},\"externals\":{externals},\"source_rows\":{},\"conditions\":{}}}",
        q(SCHEMA),
        q(fam.name()),
        sources.rows().len(),
        strings(sources.conditions().iter())
    )
    .unwrap();
    let contraction: Vec<usize> = (loops..loops + externals).rev().chain(0..loops).collect();
    for (m, row) in sources.rows().iter().enumerate() {
        // SourceSystem order: differentiated-loop major, reversed externals then loops. Momentum j: k_{j+1}, j >= loops: the external.
        let (loop_id, momentum) = (m / contraction.len(), contraction[m % contraction.len()]);
        let terms: Vec<String> = row
            .iter()
            .map(|t| {
                format!(
                    "[{},{}]",
                    q(&t.integral.to_string()),
                    q(&t.coefficient.to_string())
                )
            })
            .collect();
        writeln!(
            f,
            "{{\"type\":\"source_row\",\"m\":{m},\"differentiated_loop\":{loop_id},\"contraction_momentum\":{momentum},\"terms\":[{}]}}",
            terms.join(",")
        )
        .unwrap();
    }
}

#[derive(Default)]
struct CertTotals {
    ok: usize,
    failed: usize,
    skipped: usize,
    seconds: f64,
    weights: usize,
    rules_with_weight_poles: usize,
}

fn write_chart_cert<const N: usize>(
    f: &mut impl std::io::Write,
    sources: &SourceSystem<N>,
    chart: [bool; N],
    label: &str,
    config: &SectorConfig<N>,
    rules: &[SectorRule<N>],
    max_rows: usize,
    totals: &mut CertTotals,
) {
    let t0 = Instant::now();
    let (solver, derivation) =
        match SectorSolver::with_basis_derivation(sources, chart, config.clone()) {
            Ok(x) => x,
            Err(e) => {
                writeln!(
                    f,
                    "{{\"type\":\"cert_chart_error\",\"chart\":{},\"error\":{}}}",
                    q(label),
                    q(&format!("{e:?}"))
                )
                .unwrap();
                totals.failed += rules.len();
                return;
            }
        };
    for (k, row) in derivation.rows.iter().enumerate() {
        let entries: Vec<String> = row
            .iter()
            .map(|(m, p)| format!("[{m},{}]", q(&p.to_string())))
            .collect();
        writeln!(
            f,
            "{{\"type\":\"basis_row\",\"chart\":{},\"k\":{k},\"sources\":[{}]}}",
            q(label),
            entries.join(",")
        )
        .unwrap();
    }
    f.flush().unwrap();
    for (index, rule) in rules.iter().enumerate() {
        let rows = rule.candidate.sources.len();
        if rows > max_rows {
            writeln!(
                f,
                "{{\"type\":\"cert_skip\",\"chart\":{},\"rule\":{index},\"trace_rows\":{rows}}}",
                q(label)
            )
            .unwrap();
            totals.skipped += 1;
            continue;
        }
        let t1 = Instant::now();
        match solver.rule_frame(rule) {
            Ok(frame) => {
                let entries: Vec<String> = frame
                    .entries
                    .iter()
                    .map(|e| {
                        format!(
                            "[{},{},{}]",
                            e.basis_row,
                            q(&e.at.to_string()),
                            q(&e.weight.to_string())
                        )
                    })
                    .collect();
                let poles: Vec<String> = frame
                    .weight_poles
                    .iter()
                    .map(|b| strings(b.iter()))
                    .collect();
                writeln!(
                    f,
                    "{{\"type\":\"cert_rule\",\"chart\":{},\"rule\":{index},\"via\":{},\"recenter\":{:?},\"trace_rows\":{rows},\"zero_weights\":{},\"seconds\":{:.6},\"weight_poles\":[{}],\"frame\":[{}]}}",
                    q(label),
                    q(frame.via),
                    frame.recenter,
                    frame.zero_weights,
                    t1.elapsed().as_secs_f64(),
                    poles.join(","),
                    entries.join(",")
                )
                .unwrap();
                totals.ok += 1;
                totals.weights += frame.entries.len();
                totals.rules_with_weight_poles += usize::from(!frame.weight_poles.is_empty());
            }
            Err(e) => {
                writeln!(
                    f,
                    "{{\"type\":\"cert_error\",\"chart\":{},\"rule\":{index},\"error\":{}}}",
                    q(label),
                    q(&format!("{e:?}").chars().take(800).collect::<String>())
                )
                .unwrap();
                totals.failed += 1;
            }
        }
        f.flush().unwrap();
    }
    totals.seconds += t0.elapsed().as_secs_f64();
}

fn run<const N: usize>(cli: &Cli, fam: &IntegralFamily) -> i32 {
    let n = fam.denominator_count();
    let t_total = Instant::now();
    let mut sources = SourceSystem::<N>::from_family_with_capacity(fam, true).expect("sources");
    let requested = cli.order.as_str();
    let one_sided = matches!(requested, "legacy" | "one_sided");
    let axes: [bool; N] = if one_sided {
        [false; N]
    } else {
        sources.set_two_sided_eps_axes(fam).expect("eps axes")
    };
    let (descriptor, program, order_name) = order_for::<N>(requested, &axes, n);

    let eps: Vec<usize> = (0..n).filter(|&i| axes[i]).collect();
    let base: [bool; N] = std::array::from_fn(|i| cli.mask[i] || axes[i]);
    let charts: Vec<[bool; N]> = match &cli.charts {
        Some(list) => list.iter().map(|m| std::array::from_fn(|i| m[i])).collect(),
        None => (0..(1u64 << eps.len()))
            .map(|bits| {
                let mut c = base;
                for (j, &i) in eps.iter().enumerate() {
                    c[i] = bits >> j & 1 == 0;
                }
                c
            })
            .collect(),
    };

    let analyzer = zero::Analyzer::try_unrestricted(fam).expect("analyzer");
    let scope: Vec<usize> = match cli.zero_scope.as_str() {
        "all" => (0..n).collect(),
        "downset" => (0..n).filter(|&i| base[i]).collect(),
        other => panic!("unknown zero scope {other}"),
    };
    let t0 = Instant::now();
    let mut zeros: Vec<[bool; N]> = Vec::new();
    for bits in 0u64..(1u64 << scope.len()) {
        let mut active = [false; N];
        for (j, &i) in scope.iter().enumerate() {
            active[i] = bits >> j & 1 == 1;
        }
        let m = Mask::try_new(active.iter().copied()).unwrap();
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&m).expect("analyze") {
            zeros.push(active);
        }
    }
    let zero_seconds = t0.elapsed().as_secs_f64();

    let mut out = std::io::BufWriter::new(std::fs::File::create(&cli.out).expect("output file"));
    let bytes = program.as_ref().map_or(String::new(), |p| {
        p.canonical_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    });
    let defer_field = if cli.case_gsplit {
        ",\"defer_unsupported\":true,\"case_gsplit\":true"
    } else if cli.defer_unsupported {
        ",\"defer_unsupported\":true"
    } else {
        ""
    };
    writeln!(
        out,
        "{{\"type\":\"header\",\"schema\":{},\"source_base\":{},\"family\":{},\"family_file\":{},\"family_source\":{},\"loops\":{},\"externals\":{},\"denominators\":{n},\"parameters\":{},\"power_shifts\":{},\"mask\":{},\"order_name\":{},\"order_is_program\":{},\"order\":{},\"order_bytes\":{},\"two_sided_axes\":{:?},\"charts\":{},\"numerical_depth\":{},\"max_numerator_rank\":null,\"source_conditions\":{},\"zero_scope\":{},\"zero_seconds\":{zero_seconds:.3}{defer_field}}}",
        q(if cli.case_gsplit {
            SCHEMA_DEFER_GSPLIT
        } else if cli.defer_unsupported {
            SCHEMA_DEFER
        } else {
            SCHEMA
        }),
        q(SOURCE_BASE),
        q(fam.name()),
        q(&cli.family_path),
        q(&cli.family_text),
        fam.loop_count(),
        fam.external_count(),
        strings(fam.coefficient_context().parameter_names().iter()),
        strings(fam.power_shifts().iter()),
        q(&mask_string(&cli.mask)),
        q(&order_name),
        program.is_some(),
        descriptor_json(&descriptor),
        q(&bytes),
        &axes[..n],
        strings(charts.iter().map(|c| mask_string(&c[..n]))),
        cli.depth,
        strings(sources.conditions().iter()),
        q(&cli.zero_scope)
    )
    .unwrap();
    writeln!(
        out,
        "{{\"type\":\"zero_sectors\",\"scope\":{},\"count\":{},\"masks\":{}}}",
        q(&cli.zero_scope),
        zeros.len(),
        strings(zeros.iter().map(|z| mask_string(&z[..n])))
    )
    .unwrap();
    out.flush().unwrap();

    let mut cert = cli.cert.as_ref().map(|p| {
        let mut w = std::io::BufWriter::new(std::fs::File::create(p).expect("cert file"));
        write_cert_header(&mut w, fam, &sources);
        w.flush().unwrap();
        w
    });

    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        integral_order: program.clone(),
        ..Default::default()
    };
    let executor = SectorExecutor::new(cli.workers).expect("executor");
    let options = SectorSolveOptions {
        numerical_depth: cli.depth,
        ..Default::default()
    };
    let (mut total_rules, mut total_terms, mut total_residuals, mut errors) =
        (0usize, 0usize, 0usize, 0usize);
    let mut total_deferred = 0usize;
    let mut totals = CertTotals::default();
    for chart in &charts {
        let label = mask_string(&chart[..n]);
        let guards: Vec<String> = (0..n)
            .map(|i| match (axes[i], chart[i]) {
                (true, true) => "two_sided:n>=1".to_string(),
                (true, false) => "two_sided:n<=0".to_string(),
                (false, true) => "n>=1".to_string(),
                (false, false) => "n<=0".to_string(),
            })
            .collect();
        if zeros.contains(chart) {
            writeln!(
                out,
                "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":true}}",
                q(&label),
                strings(guards.iter())
            )
            .unwrap();
            println!("{{\"chart\":\"{label}\",\"zero\":true}}");
            continue;
        }
        writeln!(
            out,
            "{{\"type\":\"chart\",\"chart\":{},\"guards\":{},\"zero\":false}}",
            q(&label),
            strings(guards.iter())
        )
        .unwrap();
        out.flush().unwrap();
        let t1 = Instant::now();
        let result = if cli.defer_unsupported {
            // The executor's single-chart job (SectorSolver::new + solve on its pool), with the
            // opt-in S4 solve instead of solve_sector; errors keep the executor's shape.
            let sector = *chart;
            executor.install(|| {
                let solver =
                    SectorSolver::new(&sources, sector, config.clone()).map_err(|source| {
                        SectorExecutionError::<N, String>::Prepare {
                            ordinal: 0,
                            sector,
                            source,
                        }
                    })?;
                let solved = if cli.case_gsplit {
                    // Progress on stderr only (the table bytes are unaffected): one line per
                    // started case, exceptional-geometry phase and found rule.
                    let started = Instant::now();
                    let mut found = 0usize;
                    solver.solve_sector_deferring_refined_with_observer(
                        options,
                        CaseRefinement::all(),
                        |event| match event {
                            SectorEvent::CaseStarted { case, pending } => eprintln!(
                                "[gsplit-progress] {:.1}s case_started pending={pending} fixed={:?} affine={}",
                                started.elapsed().as_secs_f64(),
                                case.fixed(),
                                case.affine().is_some()
                            ),
                            SectorEvent::PhaseStarted { phase, .. } => eprintln!(
                                "[gsplit-progress] {:.1}s phase {phase:?}",
                                started.elapsed().as_secs_f64()
                            ),
                            SectorEvent::RuleFound { rule, pending } => {
                                found += 1;
                                eprintln!(
                                    "[gsplit-progress] {:.1}s rule_found {found} pending={pending} exception_branches={}",
                                    started.elapsed().as_secs_f64(),
                                    rule.exceptions.branches.len()
                                );
                            }
                            _ => {}
                        },
                    )
                } else {
                    solver.solve_sector_deferring(options)
                };
                solved
                    .map(|done| vec![(done.solution, done.deferred)])
                    .map_err(|source| SectorExecutionError::Solve {
                        ordinal: 0,
                        sector,
                        source,
                    })
            })
        } else {
            executor
                .map(&sources, &[*chart], &config, options, |done| {
                    Ok::<_, String>(done.solution)
                })
                .map(|v| v.into_iter().map(|s| (s, Vec::new())).collect::<Vec<_>>())
        };
        let seconds = t1.elapsed().as_secs_f64();
        let (solution, deferred) = match result {
            Ok(mut v) => v.pop().unwrap(),
            Err(e) => {
                let full = format!("{e:?}");
                let text = full.chars().take(2000).collect::<String>();
                // The complete exact payload (unresolved branch, original conjunction,
                // stats) in a side file: Display first, then the whole Debug.
                let full_path = format!("{}.chart_error.{label}.txt", cli.out);
                let full_bytes = full.len();
                std::fs::write(
                    &full_path,
                    format!("DISPLAY_ERROR {e}\nFULL_ERROR {full}\n"),
                )
                .expect("full chart error file");
                writeln!(out, "{{\"type\":\"chart_error\",\"chart\":{},\"seconds\":{seconds:.3},\"error\":{},\"full_error_file\":{},\"full_error_bytes\":{full_bytes}}}", q(&label), q(&text), q(&full_path)).unwrap();
                out.flush().unwrap();
                println!(
                    "{{\"chart\":\"{label}\",\"seconds\":{seconds:.3},\"error\":{}}}",
                    q(&text)
                );
                errors += 1;
                continue;
            }
        };
        let mut terms = 0;
        let mut branches = 0;
        for (index, rule) in solution.rules.iter().enumerate() {
            terms += write_rule(&mut out, &label, index, rule);
            branches += rule.exceptions.branches.len();
        }
        for key in &solution.finite_residuals {
            let key: &Integral<N> = key;
            writeln!(
                out,
                "{{\"type\":\"residual\",\"chart\":{},\"key\":{}}}",
                q(&label),
                q(&key.to_string())
            )
            .unwrap();
        }
        for (index, region) in deferred.iter().enumerate() {
            write_deferred(
                &mut out,
                &label,
                index,
                region,
                &solution.rules,
                sources.index_variables(),
                cli.case_gsplit,
            );
        }
        let deferred_field = if cli.defer_unsupported {
            format!(",\"deferred_regions\":{}", deferred.len())
        } else {
            String::new()
        };
        let s = &solution.stats;
        writeln!(
            out,
            "{{\"type\":\"chart_end\",\"chart\":{},\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches},\"residuals\":{},\"symbolic_cases\":{},\"numerical_cases\":{},\"discarded_cases\":{},\"case_lifts\":{},\"case_modular_empty\":{},\"finite_points_visited\":{},\"max_numerator_rank\":{},\"finite_case_policy\":{}{deferred_field}}}",
            q(&label),
            solution.rules.len(),
            solution.finite_residuals.len(),
            s.symbolic_cases,
            s.numerical_cases,
            s.discarded_cases,
            s.case_lifts,
            s.case_modular_empty,
            s.finite_points_visited,
            solution.max_numerator_rank.map_or("null".to_string(), |r| r.to_string()),
            q(&format!("{:?}", solution.finite_case_policy))
        )
        .unwrap();
        out.flush().unwrap();
        println!(
            "{{\"chart\":\"{label}\",\"seconds\":{seconds:.3},\"rules\":{},\"rhs_terms\":{terms},\"branches\":{branches},\"residuals\":{}{deferred_field}}}",
            solution.rules.len(),
            solution.finite_residuals.len()
        );
        total_deferred += deferred.len();
        total_rules += solution.rules.len();
        total_terms += terms;
        total_residuals += solution.finite_residuals.len();
        if let Some(c) = cert.as_mut() {
            write_chart_cert(
                c,
                &sources,
                *chart,
                &label,
                &config,
                &solution.rules,
                cli.cert_max_rows,
                &mut totals,
            );
            c.flush().unwrap();
        }
    }
    if let Some(c) = cert.as_mut() {
        writeln!(
            c,
            "{{\"type\":\"cert_end\",\"ok\":{},\"failed\":{},\"skipped\":{},\"weights\":{},\"rules_with_weight_poles\":{},\"seconds\":{:.3},\"vmhwm_kb\":{}}}",
            totals.ok,
            totals.failed,
            totals.skipped,
            totals.weights,
            totals.rules_with_weight_poles,
            totals.seconds,
            vmhwm_kb()
        )
        .unwrap();
        c.flush().unwrap();
        println!(
            "{{\"cert_ok\":{},\"cert_failed\":{},\"cert_skipped\":{},\"cert_rules_with_weight_poles\":{},\"cert_seconds\":{:.3}}}",
            totals.ok,
            totals.failed,
            totals.skipped,
            totals.rules_with_weight_poles,
            totals.seconds
        );
    }
    let deferred_total_field = if cli.defer_unsupported {
        format!(",\"deferred_regions\":{total_deferred}")
    } else {
        String::new()
    };
    writeln!(
        out,
        "{{\"type\":\"end\",\"charts\":{},\"rules\":{total_rules},\"rhs_terms\":{total_terms},\"residuals\":{total_residuals},\"chart_errors\":{errors},\"seconds\":{:.3},\"vmhwm_kb\":{}{deferred_total_field}}}",
        charts.len(),
        t_total.elapsed().as_secs_f64(),
        vmhwm_kb()
    )
    .unwrap();
    out.flush().unwrap();
    println!(
        "{{\"rules\":{total_rules},\"rhs_terms\":{total_terms},\"residuals\":{total_residuals},\"chart_errors\":{errors},\"seconds\":{:.3},\"vmhwm_kb\":{}{deferred_total_field}}}",
        t_total.elapsed().as_secs_f64(),
        vmhwm_kb()
    );
    i32::from(errors != 0)
}

fn main() {
    let (cli, fam) = cli();
    eprintln!(
        "family {} loops={} externals={} denominators={}",
        fam.name(),
        fam.loop_count(),
        fam.external_count(),
        fam.denominator_count()
    );
    let code = match fam.denominator_count() {
        2 => run::<2>(&cli, &fam),
        5 => run::<5>(&cli, &fam),
        9 => run::<9>(&cli, &fam),
        14 => run::<14>(&cli, &fam),
        20 => run::<20>(&cli, &fam),
        other => panic!("arity {other} is not compiled into table_dump (2, 5, 9, 14, 20)"),
    };
    std::process::exit(code);
}
