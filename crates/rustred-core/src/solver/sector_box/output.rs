//! Materialised reductions of the finite-field lane.

use symbolica::domains::rational::Rational;

use crate::algebra::reconstruct::RationalFunction;

use super::ff::FfReport;

/// Provenance of a materialisation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxProvenance {
    /// blake3 of the selected rows' provenance (the modular selection).
    pub selected_hash: String,
    pub master_hash: String,
    /// Kira sector ids of masters whose sector has no seeded box (a
    /// routed representative that was never seeded; such a master is an
    /// unreduced integral, correct but not minimal). Empty when every
    /// master sector was seeded.
    pub unseeded_master_sectors: Vec<u32>,
}

/// Every requested target reduced onto the masters, with coefficients
/// exact in the free parameter at fixed values of the others.
#[derive(Clone, Debug)]
pub struct BoxSolution<const N: usize> {
    /// Family parameters, in coefficient-variable order.
    pub parameters: Vec<String>,
    /// Fixed parameters and their exact values.
    pub fixed: Vec<(String, Rational)>,
    /// The free parameter.
    pub free: String,
    pub targets: Vec<[i16; N]>,
    /// Masters in column order (hardest first).
    pub masters: Vec<[i16; N]>,
    /// `coefficients[t][m]`: coefficient of `masters[m]` in `targets[t]`.
    pub coefficients: Vec<Vec<RationalFunction>>,
    /// Statistics; `report.reconstruction` lists every reconstruction and
    /// verification prime.
    pub report: FfReport,
    pub provenance: BoxProvenance,
    /// The family arity (native slots used in exchange formats).
    pub arity: usize,
}

impl<const N: usize> BoxSolution<N> {
    /// Nonzero `(master, coefficient)` terms of target `t`.
    pub fn terms(&self, target: usize) -> impl Iterator<Item = (&[i16; N], &RationalFunction)> {
        self.masters
            .iter()
            .zip(&self.coefficients[target])
            .filter(|(_, coefficient)| !coefficient.is_zero())
    }

    /// Primes the reconstruction lifted over, in order.
    pub fn reconstruction_primes(&self) -> Vec<u64> {
        self.report
            .reconstruction
            .primes
            .iter()
            .map(|prime| prime.prime)
            .collect()
    }

    /// Fresh primes the reconstruction was verified at.
    pub fn verification_primes(&self) -> Vec<u64> {
        self.report
            .reconstruction
            .verifications
            .iter()
            .map(|verification| verification.prime)
            .collect()
    }
}
