//! Runtime-arity facade over [`LiteRedSession`] for hosts that hold families
//! of dynamic arity (for example RustFlow's reduction backends). Candidate
//! formulas are generated in process; the lane stays UNCERTIFIED.

use std::sync::Arc;

use symbolica::domains::rational::Rational;

use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::Mask;
use crate::sector::symmetry::{
    SectorSymmetryCensus, SectorSymmetryCensusControl, SectorSymmetryCensusOptions,
};
use crate::sector::zero;
use crate::solver::CandidateFamilyAdmission;

use super::LiteRedError;
use super::dag::DagStatistics;
use super::generate::{CandidateGenerationStatistics, generate_candidate_program};
use super::session::{
    CompressionReport, ExactPointReduction, LiteRedOptions, LiteRedSession, UnivariateReduction,
};

/// Construction report of a dynamic lane.
#[derive(Clone, Debug)]
pub struct LiteRedLaneReport {
    pub generation: CandidateGenerationStatistics,
    pub census_classes: Option<usize>,
    pub census_seconds: f64,
}

trait Lane: Send + Sync {
    fn add_targets(&mut self, targets: &[IntegralKey]) -> Result<Vec<u32>, LiteRedError>;
    fn compress(&mut self, preferred: &[IntegralKey]) -> Result<CompressionReport, LiteRedError>;
    fn masters(&self) -> Vec<IntegralKey>;
    fn terminals(&self) -> usize;
    fn parameter_names(&self) -> Vec<String>;
    fn statistics(&self, targets: &[u32]) -> DagStatistics;
    fn nodes(&self) -> usize;
    fn reduce_univariate(
        &self,
        fixed: &[Option<Rational>],
        targets: &[u32],
    ) -> Result<UnivariateReduction, LiteRedError>;
    fn reduce_exact_at_point(
        &self,
        point: &[Rational],
        targets: &[u32],
    ) -> Result<ExactPointReduction, LiteRedError>;
}

impl<const N: usize> Lane for LiteRedSession<N> {
    fn add_targets(&mut self, targets: &[IntegralKey]) -> Result<Vec<u32>, LiteRedError> {
        LiteRedSession::add_targets(self, targets)
    }
    fn compress(&mut self, preferred: &[IntegralKey]) -> Result<CompressionReport, LiteRedError> {
        LiteRedSession::compress(self, preferred)
    }
    fn masters(&self) -> Vec<IntegralKey> {
        LiteRedSession::masters(self)
    }
    fn terminals(&self) -> usize {
        LiteRedSession::terminals(self).len()
    }
    fn parameter_names(&self) -> Vec<String> {
        LiteRedSession::parameter_names(self).to_vec()
    }
    fn statistics(&self, targets: &[u32]) -> DagStatistics {
        LiteRedSession::statistics(self, targets)
    }
    fn nodes(&self) -> usize {
        self.dag_len()
    }
    fn reduce_univariate(
        &self,
        fixed: &[Option<Rational>],
        targets: &[u32],
    ) -> Result<UnivariateReduction, LiteRedError> {
        LiteRedSession::reduce_univariate(self, fixed, targets)
    }
    fn reduce_exact_at_point(
        &self,
        point: &[Rational],
        targets: &[u32],
    ) -> Result<ExactPointReduction, LiteRedError> {
        LiteRedSession::reduce_exact_at_point(self, point, targets)
    }
}

/// A LiteRed lane of runtime arity.
pub struct DynamicLiteRedSession {
    lane: Box<dyn Lane>,
    arity: usize,
}

impl std::fmt::Debug for DynamicLiteRedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicLiteRedSession")
            .field("arity", &self.arity)
            .field("nodes", &self.lane.nodes())
            .finish()
    }
}

fn build<const N: usize>(
    family: Arc<IntegralFamily>,
    root: &[bool],
    workers: usize,
    census: bool,
    options: LiteRedOptions,
) -> Result<(Box<dyn Lane>, LiteRedLaneReport), LiteRedError> {
    let arity = family.denominator_count();
    let root_array: [bool; N] = std::array::from_fn(|i| i < arity && root[i]);
    let (reducer, generation) = generate_candidate_program::<N>(
        &family,
        root_array,
        workers,
        CandidateFamilyAdmission::ParametricVacuum,
    )?;
    let start = std::time::Instant::now();
    let census = if census {
        let analyzer = zero::Analyzer::try_unrestricted(&family)
            .map_err(|e| LiteRedError::Census(e.to_string()))?;
        let top =
            Mask::try_new(root.iter().copied()).map_err(|e| LiteRedError::Census(e.to_string()))?;
        Some(Arc::new(
            SectorSymmetryCensus::discover_subsectors(
                Arc::clone(&family),
                &top,
                &analyzer,
                SectorSymmetryCensusOptions::default(),
                &SectorSymmetryCensusControl::default(),
            )
            .map_err(|e| LiteRedError::Census(e.to_string()))?,
        ))
    } else {
        None
    };
    let census_classes = census.as_ref().map(|c| c.classes().len());
    let session = LiteRedSession::<N>::new(Arc::new(reducer), census, options)?;
    Ok((
        Box::new(session),
        LiteRedLaneReport {
            generation,
            census_classes,
            census_seconds: start.elapsed().as_secs_f64(),
        },
    ))
}

impl DynamicLiteRedSession {
    /// Generate the candidate program of `root`'s downset in process (on
    /// `workers` threads), optionally build the sector-symmetry census, and
    /// return an uncompressed lane.
    pub fn generate(
        family: Arc<IntegralFamily>,
        root: &[bool],
        workers: usize,
        census: bool,
        options: LiteRedOptions,
    ) -> Result<(Self, LiteRedLaneReport), LiteRedError> {
        let arity = family.denominator_count();
        if root.len() != arity {
            return Err(LiteRedError::InvalidInput(
                "root has the wrong arity".into(),
            ));
        }
        macro_rules! arms {
            ($($n:literal),* $(,)?) => {
                match crate::campaign_storage_arity(arity) {
                    $( $n => build::<$n>(family, root, workers, census, options)?, )*
                    _ => {
                        return Err(LiteRedError::InvalidInput(format!(
                            "arity {arity} is not compiled into this RustRed build"
                        )))
                    }
                }
            };
        }
        let (lane, report) = with_app_runtime_arities!(arms);
        Ok((Self { lane, arity }, report))
    }

    pub fn arity(&self) -> usize {
        self.arity
    }
    pub fn add_targets(&mut self, targets: &[IntegralKey]) -> Result<Vec<u32>, LiteRedError> {
        self.lane.add_targets(targets)
    }
    pub fn compress(
        &mut self,
        preferred: &[IntegralKey],
    ) -> Result<CompressionReport, LiteRedError> {
        self.lane.compress(preferred)
    }
    pub fn masters(&self) -> Vec<IntegralKey> {
        self.lane.masters()
    }
    pub fn terminals(&self) -> usize {
        self.lane.terminals()
    }
    pub fn parameter_names(&self) -> Vec<String> {
        self.lane.parameter_names()
    }
    pub fn statistics(&self, targets: &[u32]) -> DagStatistics {
        self.lane.statistics(targets)
    }
    pub fn nodes(&self) -> usize {
        self.lane.nodes()
    }
    pub fn reduce_univariate(
        &self,
        fixed: &[Option<Rational>],
        targets: &[u32],
    ) -> Result<UnivariateReduction, LiteRedError> {
        self.lane.reduce_univariate(fixed, targets)
    }
    pub fn reduce_exact_at_point(
        &self,
        point: &[Rational],
        targets: &[u32],
    ) -> Result<ExactPointReduction, LiteRedError> {
        self.lane.reduce_exact_at_point(point, targets)
    }
}
