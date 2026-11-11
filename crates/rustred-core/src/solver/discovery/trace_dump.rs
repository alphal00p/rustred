//! Opt-in dump of one exact-lift trace in a replayable text form (p5l-lift-sampled).
//!
//! Diagnostics only: enabled by the environment variable `RUSTRED_TRACE_DUMP=<path>`; a frame is written when it has
//! at least `RUSTRED_TRACE_DUMP_MIN_ROWS` source rows (default 1500). With `RUSTRED_TRACE_DUMP_EXIT=1` the process
//! exits (status 42) after the dump, so a hanging lift can be reproduced and captured in seconds.
//!
//! Format (one record per line, integers in decimal):
//! `TRACE v1` / `vars K name_1 .. name_K` / `columns C` / `c i <integral debug>` (C lines, harder first) /
//! `target t` / `rows R` / per row `r n` followed by n lines `t col NUM | DEN`, a polynomial being
//! `coef:e_1,..,e_K;coef:...` over the original coefficient variables.

use std::io::Write;

use crate::algebra::Coefficient;

use super::{ExactRow, Integral, IntegralOrder};

fn poly(
    out: &mut String,
    p: &symbolica::poly::polynomial::MultivariatePolynomial<symbolica::prelude::IntegerRing, u16>,
) {
    let n = p.nvars();
    for (i, c) in p.coefficients.iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        out.push_str(&c.to_string());
        out.push(':');
        for (k, e) in p.exponents[i * n..(i + 1) * n].iter().enumerate() {
            if k > 0 {
                out.push(',');
            }
            out.push_str(&e.to_string());
        }
    }
}

fn coefficient(out: &mut String, c: &Coefficient) {
    poly(out, &c.numerator);
    out.push_str(" | ");
    poly(out, &c.denominator);
}

pub(super) fn maybe_dump<const N: usize>(
    rows: &[ExactRow<N>],
    columns: &[Integral<N>],
    order: &IntegralOrder<N>,
    target_column: usize,
) {
    let Ok(path) = std::env::var("RUSTRED_TRACE_DUMP") else {
        return;
    };
    let min_rows: usize = std::env::var("RUSTRED_TRACE_DUMP_MIN_ROWS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1500);
    if rows.len() < min_rows {
        return;
    }
    let Some(first) = rows.iter().flatten().next() else {
        return;
    };
    let vars = first.coefficient.get_variables();
    let mut s = String::new();
    s.push_str("TRACE v1\n");
    s.push_str(&format!("vars {}", vars.len()));
    for v in vars.iter() {
        s.push(' ');
        s.push_str(&v.to_string().replace(' ', ""));
    }
    s.push('\n');
    s.push_str(&format!("columns {}\n", columns.len()));
    for (i, c) in columns.iter().enumerate() {
        s.push_str(&format!("c {i} {c:?}\n"));
    }
    s.push_str(&format!("target {target_column}\nrows {}\n", rows.len()));
    for row in rows {
        s.push_str(&format!("r {}\n", row.len()));
        for term in row {
            let col = columns
                .binary_search_by(|candidate| order.compare(candidate, &term.integral))
                .expect("column union contains every row integral");
            s.push_str(&format!("t {col} "));
            coefficient(&mut s, &term.coefficient);
            s.push('\n');
        }
    }
    let tmp = format!("{path}.tmp");
    let mut f = std::fs::File::create(&tmp).expect("trace dump file");
    f.write_all(s.as_bytes()).expect("trace dump write");
    drop(f);
    std::fs::rename(&tmp, &path).expect("trace dump rename");
    eprintln!(
        "TRACE_DUMP {path} rows={} columns={} target={target_column}",
        rows.len(),
        columns.len()
    );
    if std::env::var("RUSTRED_TRACE_DUMP_EXIT").as_deref() == Ok("1") {
        std::process::exit(42);
    }
}
