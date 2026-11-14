//! Materialisation entry points of a session that has a modular selection:
//! the request (exact selected system, routed targets, parameter split),
//! the explicit-sample probe and the finite-field lane.

use std::sync::Arc;

use symbolica::prelude::Rational;

use super::super::super::{Integral, SolverError};
use super::super::ff::{self, FfOptions};
use super::super::output::{BoxProvenance, BoxSolution};
use super::super::probe::{self, LaneRequest};
use super::super::router::integral_bits;
use super::super::system::ExactRowBuilder;
use super::BoxLaportaSession;

/// An explicit point (every family parameter fixed) and a prime, for
/// reproducible comparisons with an oracle evaluated at the same point
/// modulo the same prime (Kira `numerical_points` with `--prime_user`,
/// arXiv:2505.20197 §3.4).
#[derive(Clone, Debug, PartialEq)]
pub struct ExplicitSample {
    /// An odd prime below `2^63`.
    pub prime: u64,
    /// `(parameter name, value)` for every family parameter.
    pub point: Vec<(String, Rational)>,
}

/// `(target, master)` values of one [`ExplicitSample`].
#[derive(Clone, Debug, PartialEq)]
pub struct ExplicitTable<const N: usize> {
    pub sample: ExplicitSample,
    pub targets: Vec<[i16; N]>,
    /// Masters in column order (hardest first).
    pub masters: Vec<[i16; N]>,
    /// `values[t][m]`: coefficient of `masters[m]` in `targets[t]`,
    /// canonical residues modulo `sample.prime`.
    pub values: Vec<Vec<u64>>,
}

fn powers<const N: usize>(integral: &Integral<N>) -> [i16; N] {
    std::array::from_fn(|axis| integral[axis].value())
}

impl<const N: usize> BoxLaportaSession<N> {
    /// The `(target, master)` values at one explicit point modulo one prime.
    ///
    /// The selected rows are compiled at the point and eliminated along the
    /// modular pivots (the bunch kernel of the finite-field lane), and the
    /// result is cross-checked with an independent reference elimination
    /// (Symbolica's sparse row reducer with pivot search). Every target must
    /// have been demanded by [`Self::extend`].
    ///
    /// Errors: [`SolverError::InvalidInput`] for a target that was not
    /// demanded, a parameter that is unknown, fixed twice or not fixed, or a
    /// prime that is not an odd prime below `2^63`;
    /// [`SolverError::UnluckyBoxSample`] when the point is special modulo the
    /// prime (a pole of a fixed value or of a row coefficient, a vanishing
    /// modular pivot, a different pivot structure, or a nonzero coefficient
    /// outside the masters); [`SolverError::Certification`] when the two
    /// eliminations disagree.
    pub fn probe_explicit(
        &mut self,
        targets: &[[i16; N]],
        sample: &ExplicitSample,
    ) -> Result<ExplicitTable<N>, SolverError> {
        let (request, masters) = self.lane_request(targets, &sample.point)?;
        let values = probe::probe_explicit(&request, sample.prime)?;
        Ok(ExplicitTable {
            sample: sample.clone(),
            targets: targets.to_vec(),
            values: split_rows(values, targets.len(), masters.len()),
            masters,
        })
    }

    /// The finite-field lane: replay the selected rows at points of the
    /// single free parameter modulo word-size primes and reconstruct every
    /// `(target, master)` coefficient over Q, verified at a fresh prime
    /// with the independent reference elimination. `fixed` gives exact
    /// values of every other family parameter. Every target must have been
    /// demanded by [`Self::extend`].
    ///
    /// The result does not depend on the worker count. With
    /// `options.checkpoint_dir` the probe values are logged per prime and a
    /// resumed call replays them; the log is bound to the family, the
    /// routing, the column order, the selection, the request, the point
    /// stream and the fault injection.
    ///
    /// Errors: [`SolverError::InvalidInput`] for a bad request (as for
    /// [`Self::probe_explicit`], or not exactly one free parameter, a bunch
    /// size outside 1, 2, 4, 8, 16, no verification, an unusable checkpoint
    /// directory); [`SolverError::Certification`] when the prime or point
    /// budget is exhausted or the fresh-prime verification keeps failing;
    /// [`SolverError::UnluckyBoxSample`] when no fresh verification point is
    /// usable.
    pub fn materialize_ff(
        &mut self,
        targets: &[[i16; N]],
        fixed: &[(String, Rational)],
        options: &FfOptions,
    ) -> Result<BoxSolution<N>, SolverError> {
        let built = std::time::Instant::now();
        let (request, masters) = self.lane_request(targets, fixed)?;
        let build_seconds = built.elapsed().as_secs_f64();
        let names = self.family.coefficient_context().parameter_names().to_vec();
        let free = request.free();
        if free.len() != 1 {
            return Err(SolverError::InvalidInput(format!(
                "the univariate finite-field lane needs exactly one free parameter, got {}",
                free.len()
            )));
        }
        let key = self.session_key();
        let workers = self.options.workers.max(1);
        let (functions, mut report) = self
            .pool
            .install(|| ff::reconstruct(&request, options, workers, &key))?;
        report.build_seconds = build_seconds;
        let selection = self
            .selection
            .as_ref()
            .expect("lane_request checked the selection");
        let unseeded_master_sectors = masters
            .iter()
            .map(integral_bits)
            .filter(|bits| !self.boxes.contains_key(bits))
            .map(|bits| bits as u32)
            .collect();
        let fixed = request
            .fixed
            .iter()
            .zip(&names)
            .filter_map(|(value, name)| Some((name.clone(), value.clone()?)))
            .collect();
        Ok(BoxSolution {
            free: names[free[0]].clone(),
            parameters: names,
            fixed,
            targets: targets.to_vec(),
            coefficients: split_rows(functions, targets.len(), masters.len()),
            masters,
            report,
            provenance: BoxProvenance {
                selected_hash: selection.selected_hash.clone(),
                master_hash: selection.master_hash.clone(),
                unseeded_master_sectors,
            },
            arity: self.arity,
        })
    }

    /// Binds probe logs to the family, the routing, the column order and
    /// the selection.
    fn session_key(&self) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"rustred-sector-box-session-v1\0");
        hasher.update(self.family.fingerprint().as_bytes());
        hasher.update(&self.router.fingerprint());
        hasher.update(self.order.fingerprint());
        if let Some(selection) = &self.selection {
            hasher.update(selection.selected_hash.as_bytes());
            hasher.update(selection.master_hash.as_bytes());
        }
        hasher.finalize().as_bytes().to_vec()
    }

    /// The exact selected system and the target routes, with `fixed`
    /// parameter values; returns the request and the masters (column order).
    pub(super) fn lane_request(
        &mut self,
        targets: &[[i16; N]],
        fixed: &[(String, Rational)],
    ) -> Result<(LaneRequest<N>, Vec<[i16; N]>), SolverError> {
        let selection = self.selection.as_ref().ok_or_else(|| {
            SolverError::InvalidInput(
                "materialisation needs a modular selection; extend first".into(),
            )
        })?;
        for values in targets {
            self.check_powers(values)?;
            for key in self.factory.route_keys(values)? {
                if !self.demand.contains(&key) {
                    return Err(SolverError::InvalidInput(format!(
                        "target {values:?} was not demanded (route key {key:?}); extend first"
                    )));
                }
            }
        }
        let context = self.family.coefficient_context().clone();
        let names = context.parameter_names();
        let mut fixed_values: Vec<Option<Rational>> = vec![None; names.len()];
        for (name, value) in fixed {
            let index = names
                .iter()
                .position(|candidate| candidate == name)
                .ok_or_else(|| {
                    SolverError::InvalidInput(format!("unknown family parameter {name}"))
                })?;
            if fixed_values[index].replace(value.clone()).is_some() {
                return Err(SolverError::InvalidInput(format!(
                    "parameter {name} fixed twice"
                )));
            }
        }
        let mut builder = ExactRowBuilder::new(
            &self.sources,
            &self.factory,
            &*self.router,
            &self.order,
            &context,
        )?;
        let mut target_terms = Vec::with_capacity(targets.len());
        for target in targets {
            target_terms.push(builder.target_route(target)?);
        }
        // The exact system depends only on the selection and the column
        // order, so it is reused across calls (e.g. one call per value of a
        // fixed parameter).
        let cache_key = format!(
            "{}:{}:{}",
            selection.selected_hash,
            selection.master_hash,
            blake3::hash(self.order.fingerprint()).to_hex()
        );
        let system = match &self.exact_system {
            Some((key, system)) if *key == cache_key => system.clone(),
            _ => {
                let system = Arc::new(builder.system(selection, self.order.clone())?);
                self.exact_system = Some((cache_key, system.clone()));
                system
            }
        };
        // Every route term is a demanded representative integral, hence a
        // column of the system.
        let mut routes = Vec::with_capacity(target_terms.len());
        for terms in target_terms {
            let mut route = Vec::with_capacity(terms.len());
            for (integral, coefficient) in terms {
                let column = system.column_index(&integral).ok_or_else(|| {
                    SolverError::InvalidInput(format!(
                        "route term {:?} is not a column of the selected system",
                        powers(&integral)
                    ))
                })?;
                route.push((column as u32, coefficient));
            }
            routes.push(route);
        }
        let masters = system
            .masters
            .iter()
            .map(|&column| powers(&system.columns[column as usize]))
            .collect();
        Ok((
            LaneRequest {
                system,
                targets: targets.to_vec(),
                routes,
                fixed: fixed_values,
            },
            masters,
        ))
    }
}

/// Row-major `targets x masters` values as one row per target.
fn split_rows<T: Clone>(values: Vec<T>, targets: usize, masters: usize) -> Vec<Vec<T>> {
    if masters == 0 {
        return vec![Vec::new(); targets];
    }
    debug_assert_eq!(values.len(), targets * masters);
    values.chunks(masters).map(<[T]>::to_vec).collect()
}

#[cfg(test)]
pub(crate) mod tests;
