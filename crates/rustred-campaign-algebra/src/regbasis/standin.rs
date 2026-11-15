//! An EXACT stand-in of the composition (constructed tests, table-driven runs): the composed coefficients of a key on the
//! terminals are given exactly (rational functions of d, texts) by a table or a formula of the key's indices; the series
//! are their exact expansions at d0 (Symbolica). Keys listed as undecided answer `Undecided` (the band verdict then drops).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use symbolica::domains::rational::Rational;
use symbolica::poly::PolyVariable;

use rustred_campaign_core::key::Key;
use rustred_campaign_core::regbasis::{Compose, ComposeError, ComposedVec};

use super::series::{expand_rf, parse_rf, t_vars};

/// The coefficient source: key -> exact coefficient texts on the terminals (`None` = the key is not in the reduction).
pub type Formula = Box<dyn Fn(&[i32]) -> Option<Vec<String>>>;

pub struct StandIn {
    n: usize,
    d0: Rational,
    tv: Arc<Vec<PolyVariable>>,
    f: Formula,
    pub undecided: HashSet<Key>,
    /// Keys composed (series) so far, for cost checks in tests.
    pub series_calls: usize,
}

impl StandIn {
    pub fn new(n: usize, d0: Rational, f: Formula) -> StandIn {
        StandIn {
            n,
            d0,
            tv: t_vars(),
            f,
            undecided: HashSet::new(),
            series_calls: 0,
        }
    }

    /// From an explicit table.
    pub fn from_table(n: usize, d0: Rational, table: HashMap<Key, Vec<String>>) -> StandIn {
        StandIn::new(n, d0, Box::new(move |k| table.get(k).cloned()))
    }
}

impl Compose for StandIn {
    fn n_terminals(&self) -> usize {
        self.n
    }

    fn stream(
        &mut self,
        keys: &[Key],
        abs_prec: i64,
        visit: &mut dyn FnMut(usize, Result<ComposedVec, ComposeError>),
    ) -> Result<(), String> {
        for (i, k) in keys.iter().enumerate() {
            self.series_calls += 1;
            if self.undecided.contains(k) {
                visit(
                    i,
                    Err(ComposeError::Undecided("stand-in: listed undecided".into())),
                );
                continue;
            }
            let Some(texts) = (self.f)(k) else {
                visit(
                    i,
                    Err(ComposeError::Undecided(
                        "stand-in: key not in the table".into(),
                    )),
                );
                continue;
            };
            if texts.len() != self.n {
                return Err(format!(
                    "stand-in: {} coefficients for {k:?}, expected {}",
                    texts.len(),
                    self.n
                ));
            }
            let mut v = Vec::with_capacity(self.n);
            for t in &texts {
                let rf = parse_rf(t)?;
                v.push(expand_rf(&rf, &self.d0, abs_prec, &self.tv).to_text());
            }
            visit(i, Ok(v));
        }
        Ok(())
    }

    fn exact(&mut self, key: &[i32]) -> Result<Vec<String>, ComposeError> {
        (self.f)(key)
            .ok_or_else(|| ComposeError::Unavailable("stand-in: key not in the table".into()))
    }
}
