//! Read-only structural census of a native terminal-relation session.
//!
//! Usage: SYMBOLICA_HIDE_BANNER=1 cargo run -p rustred
//! --example terminal_relation_census -- SESSION.bin
//! This accepts the native session payload, not its enclosing artifact metadata.
//! It uses conservative core normalization limits. For profile-bearing artifacts
//! (especially standard-v1), use the application's metadata-aware loader and
//! `master-inspect`; do not reinterpret a native payload under different limits.
//! All geometry checks use the public exact normalization service. No coefficient
//! expressions or Symbolica state are emitted, and no reduction steps are run.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::{self, Write};

use rustred::family::IntegralKey;
use rustred::persistence::BinaryIoLimits;
use rustred::reduction::terminal_normalization::TerminalAliasPlan;
use rustred::reduction::terminal_relations::TerminalRelationSession;
use rustred::sector::OrderingPolicy;

#[path = "terminal_relation_census/factors.rs"]
mod factors;

fn keys(out: &mut impl Write, keys: &BTreeSet<IntegralKey>) -> io::Result<()> {
    write!(out, "[")?;
    for (index, key) in keys.iter().enumerate() {
        if index != 0 {
            write!(out, ",")?;
        }
        // A slice of signed integers has identical Rust-debug and JSON syntax.
        write!(out, "{:?}", key.powers())?;
    }
    write!(out, "]")
}

fn histogram(out: &mut impl Write, keys: &BTreeSet<IntegralKey>) -> io::Result<()> {
    let mut counts = BTreeMap::<(i128, i128, Vec<usize>), usize>::new();
    for key in keys {
        let mut positive_power = 0i128;
        let mut rank = 0i128;
        let mut support = Vec::new();
        for (slot, &power) in key.powers().iter().enumerate() {
            if power > 0 {
                positive_power += i128::from(power);
                support.push(slot);
            } else {
                rank -= i128::from(power);
            }
        }
        *counts.entry((positive_power, rank, support)).or_default() += 1;
    }
    write!(out, "[")?;
    for (index, ((positive_power, rank, support), count)) in counts.into_iter().enumerate() {
        if index != 0 {
            write!(out, ",")?;
        }
        write!(
            out,
            "{{\"positive_power_sum\":{positive_power},\"numerator_rank\":{rank},\"power_difference\":{},\"positive_support_slots\":{support:?},\"count\":{count}}}",
            positive_power - rank
        )?;
    }
    write!(out, "]")
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: terminal_relation_census SESSION.bin")?;
    if path == "--help" || path == "-h" {
        println!(
            "usage: terminal_relation_census SESSION.bin\nRead a native terminal-session binary and print a structural JSON census."
        );
        return Ok(());
    }
    if args.next().is_some() {
        return Err("usage: terminal_relation_census SESSION.bin".into());
    }
    let io_limits = BinaryIoLimits::default();
    if std::fs::metadata(&path)?.len() > io_limits.max_program_bytes as u64 {
        return Err("native session exceeds the default binary size limit".into());
    }
    let session = TerminalRelationSession::from_native_bytes(
        &std::fs::read(path)?,
        Default::default(),
        io_limits,
    )?;
    let family = session.family_owner();
    let raw = session.raw_terminals();
    let remaining = session.remaining_terminals();
    let products = TerminalAliasPlan::independent_tadpole_products(
        family,
        &remaining,
        OrderingPolicy::SpiredUncutV1,
    )?;
    let stats = products.statistics();
    let mut out = io::BufWriter::new(io::stdout().lock());
    write!(
        out,
        "{{\"schema\":1,\"loop_count\":{},\"arity\":{},\"complete\":{},\"pending_rebuild_rows\":{},\"raw_count\":{},\"remaining_count\":{},\"raw_keys\":",
        family.loop_count(),
        family.denominator_count(),
        session.is_complete(),
        session.statistics().pending_rebuild_rows,
        raw.len(),
        remaining.len()
    )?;
    keys(&mut out, raw)?;
    write!(out, ",\"remaining_keys\":")?;
    keys(&mut out, &remaining)?;
    write!(out, ",\"raw_histogram\":")?;
    histogram(&mut out, raw)?;
    write!(out, ",\"remaining_histogram\":")?;
    histogram(&mut out, &remaining)?;
    write!(
        out,
        ",\"independent_tadpole_products\":{{\"raw_terminals\":{},\"eligible_products\":{},\"analyzed_denominators\":{},\"verified_aliases\":{},\"canonical_terminals\":{},\"skipped\":{{",
        stats.raw_terminals,
        stats.eligible_products,
        stats.analyzed_denominators,
        stats.verified_aliases,
        stats.canonical_terminals
    )?;
    for (index, (reason, count)) in stats.skipped.iter().enumerate() {
        if index != 0 {
            write!(out, ",")?;
        }
        // Public enum variant identifiers contain no JSON escape characters.
        write!(out, "\"{reason:?}\":{count}")?;
    }
    let normalization = session.normalization().statistics();
    write!(
        out,
        "}}}},\"normalization\":{{\"analyzed_supports\":{},\"verified_generators\":{},\"projected_numerators\":{},\"skipped\":{{",
        normalization.analyzed_supports,
        normalization.verified_generators,
        normalization.projected_numerators,
    )?;
    for (index, (reason, count)) in normalization.skipped.iter().enumerate() {
        if index != 0 {
            write!(out, ",")?;
        }
        write!(out, "\"{reason:?}\":{count}")?;
    }
    write!(out, "}}}},\"tadpole_factor_candidates\":")?;
    factors::write(&mut out, family, &remaining)?;
    writeln!(out, "}}")?;
    out.flush()?;
    Ok(())
}
