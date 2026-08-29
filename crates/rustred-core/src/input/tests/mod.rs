use symbolica::prelude::*;

use super::limits::Stats;
use super::parse::{RawSourceKind, parse_expression_accumulating};
use super::*;

mod frontends;
mod grammar;
mod lowering;
mod resources;

pub(super) fn compiler() -> Compiler {
    Compiler::new(Limits::default()).expect("plain RustRed input grammar must initialize")
}

pub(super) fn compiler_with(update: impl FnOnce(&mut Limits)) -> Compiler {
    let mut limits = Limits::default();
    update(&mut limits);
    Compiler::new(limits).expect("bounded RustRed input grammar must initialize")
}

pub(super) fn one_loop_source(target: i64, numerator: &str) -> String {
    format!("I(loops(k),externals(),dimension(d),prop(D1,k^2-m2,{target}),numerator({numerator}))")
}

pub(super) fn parse_base(compiler: &Compiler, source: &str) -> Result<Atom, Error> {
    let mut stats = Stats::default();
    parse_expression_accumulating(
        source,
        RawSourceKind::BaseCoefficientExpression,
        &mut stats,
        compiler.limits,
    )
}
