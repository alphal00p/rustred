//! One LiteRed-lane session: DAG over a target set, census compression, and
//! coefficients onto the compressed basis at modular or rational samples.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

use crate::family::IntegralKey;
use crate::sector::Mask;
use crate::sector::symmetry::{SectorSymmetryCensus, sector_id_cmp};
use crate::solver::CandidateReducer;
use crate::solver::candidate_reduction::CandidateProgramView;

use super::LiteRedError;
use super::compress::{
    CensusRow, CensusRowStatistics, ColumnOrder, ColumnSpace, Elimination, RowOrigin, SymmetryRows,
    census_rows,
};
use super::dag::{
    DagStatistics, DagStructure, DagValues, SampleProgram, SampleProgramStatistics, pad,
};
use super::modular::{self, SparseVector};
use super::reconstruct::{CrtAccumulator, RationalFunction, Thiele};
use super::zp::{DEFAULT_PRIME, PrimeField, ZpSample, reconstruction_primes};

/// Options of the uncertified LiteRed lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiteRedOptions {
    pub order: ColumnOrder,
    pub rows: SymmetryRows,
    /// Seed of the structure-building sample point (prime `2^61 - 1`).
    pub seed: u64,
    pub max_nodes: usize,
    pub max_orbit: usize,
    /// Primes available to rational reconstruction at one point.
    pub max_primes: usize,
    /// Primes evaluated concurrently.
    pub workers: usize,
}

impl Default for LiteRedOptions {
    fn default() -> Self {
        Self {
            order: ColumnOrder::default(),
            rows: SymmetryRows::default(),
            seed: 0x5eed_1173,
            max_nodes: 20_000_000,
            max_orbit: 100_000,
            max_primes: 256,
            workers: 1,
        }
    }
}

/// Census class summary of a compressed basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassCount {
    pub representative: Mask,
    pub representative_id: u128,
    pub members: usize,
    pub masters: usize,
}

/// Outcome of [`LiteRedSession::compress`] at the structure sample.
#[derive(Clone, Debug)]
pub struct CompressionReport {
    pub terminals: usize,
    pub columns: usize,
    pub preferred_columns: usize,
    pub rows: CensusRowStatistics,
    pub rank: usize,
    pub masters: Vec<IntegralKey>,
    pub classes: Vec<ClassCount>,
    /// Nodes of the whole DAG after adding every row integral.
    pub dag_nodes: usize,
    pub row_seconds: f64,
    pub elimination_seconds: f64,
}

/// Modular coefficients of targets onto the masters at one sample.
#[derive(Clone, Debug)]
pub struct SampleReduction {
    pub prime: u64,
    pub coefficients: Vec<Vec<u64>>,
}

/// Exact rational coefficients at one rational point (multi-prime CRT and
/// maximal-quotient rational reconstruction, verified at a fresh prime).
#[derive(Clone, Debug)]
pub struct ExactPointReduction {
    pub point: Vec<Rational>,
    pub primes_used: usize,
    pub skipped_primes: usize,
    pub coefficients: Vec<Vec<Rational>>,
    pub seconds: f64,
}

/// Exact coefficients in `Q(x)` for one free base parameter `x`, all other
/// base parameters fixed to rationals.
#[derive(Clone, Debug)]
pub struct UnivariateReduction {
    pub free_parameter: usize,
    pub primes_used: usize,
    pub skipped_primes: usize,
    /// Largest number of interpolation points used for one entry and prime.
    pub max_points: usize,
    pub coefficients: Vec<Vec<RationalFunction>>,
    pub seconds: f64,
}

/// Interpolation points tried per prime before giving up.
const MAX_POINTS_PER_PRIME: usize = 512;

struct Compressed {
    columns: ColumnSpace,
    rows: Vec<CensusRow>,
    pivots: Vec<usize>,
    masters: Vec<usize>,
    /// Column position of every terminal id.
    terminal_column: Vec<usize>,
}

/// An uncertified LiteRed lane over one candidate program. The session owns
/// its program and census (shared), so it can be retained across calls.
pub struct LiteRedSession<const N: usize> {
    reducer: Arc<CandidateReducer<N>>,
    census: Option<Arc<SectorSymmetryCensus>>,
    options: LiteRedOptions,
    build: SampleProgram<N>,
    structure: DagStructure<N>,
    values: DagValues,
    terminals: Vec<IntegralKey>,
    compressed: Option<Compressed>,
    build_seconds: f64,
}

impl<const N: usize> LiteRedSession<N> {
    pub fn new(
        reducer: Arc<CandidateReducer<N>>,
        census: Option<Arc<SectorSymmetryCensus>>,
        options: LiteRedOptions,
    ) -> Result<Self, LiteRedError> {
        let view = reducer.program_view();
        if let Some(census) = &census {
            if census.family().fingerprint() != view.family_fingerprint() {
                return Err(LiteRedError::Census(
                    "census family differs from the candidate program family".into(),
                ));
            }
        }
        let field = PrimeField::new(DEFAULT_PRIME)?;
        let mut rng = StdRng::seed_from_u64(options.seed);
        let base = (0..view.context().base().parameter_names().len())
            .map(|_| rng.random_range(1..DEFAULT_PRIME))
            .collect();
        let build = SampleProgram::new(view, ZpSample::new(field, base))?;
        let terminals: Vec<IntegralKey> = view.terminals().iter().cloned().collect();
        Ok(Self {
            reducer,
            census,
            options,
            build,
            structure: DagStructure::new(terminals.len()),
            values: DagValues::default(),
            terminals,
            compressed: None,
            build_seconds: 0.0,
        })
    }

    fn view(&self) -> CandidateProgramView<'_, N> {
        self.reducer.program_view()
    }
    pub fn reducer(&self) -> &Arc<CandidateReducer<N>> {
        &self.reducer
    }
    pub fn census(&self) -> Option<&Arc<SectorSymmetryCensus>> {
        self.census.as_ref()
    }
    pub fn parameter_names(&self) -> &[String] {
        self.reducer.coefficient_context().base().parameter_names()
    }
    pub fn terminals(&self) -> &[IntegralKey] {
        &self.terminals
    }
    pub fn options(&self) -> LiteRedOptions {
        self.options
    }
    pub fn dag_len(&self) -> usize {
        self.structure.len()
    }
    pub fn build_seconds(&self) -> f64 {
        self.build_seconds
    }
    pub fn sample_statistics(&self) -> SampleProgramStatistics {
        self.build.statistics()
    }
    /// The structure-building sample (prime `2^61 - 1`, seeded point).
    pub fn structure_sample(&self) -> &ZpSample {
        self.build.sample()
    }
    pub fn structure(&self) -> &DagStructure<N> {
        &self.structure
    }

    /// Expand targets into the DAG (exact structure, structure-sample values).
    pub fn add_targets(&mut self, targets: &[IntegralKey]) -> Result<Vec<u32>, LiteRedError> {
        let arity = self.reducer.coefficient_context().index_count();
        let padded: Vec<[i64; N]> = targets
            .iter()
            .map(|key| {
                if key.powers().len() != arity {
                    Err(LiteRedError::InvalidInput(format!(
                        "target {key:?} has arity {}, family arity {arity}",
                        key.powers().len()
                    )))
                } else {
                    Ok(pad::<N>(key.powers()))
                }
            })
            .collect::<Result<_, _>>()?;
        let start = Instant::now();
        let view = self.reducer.program_view();
        let ids = self.structure.extend(
            view,
            &mut self.build,
            &mut self.values,
            &padded,
            self.options.max_nodes,
        )?;
        self.build_seconds += start.elapsed().as_secs_f64();
        Ok(ids)
    }

    pub fn statistics(&self, targets: &[u32]) -> DagStatistics {
        self.structure.statistics(targets)
    }

    /// Forward decompositions onto terminals at the structure sample.
    pub fn forward(&self) -> Result<Vec<Option<SparseVector>>, LiteRedError> {
        modular::forward(&self.structure, &self.values, &self.build.sample().field)
    }

    /// Adjoint decompositions onto terminals at the structure sample.
    pub fn adjoint(&self, targets: &[u32]) -> Result<Vec<Vec<u64>>, LiteRedError> {
        modular::adjoint(
            &self.structure,
            &self.values,
            &self.build.sample().field,
            targets,
        )
    }

    fn zero_test(&self) -> impl Fn(&IntegralKey) -> bool + '_ {
        move |key: &IntegralKey| {
            let sector: [bool; N] =
                std::array::from_fn(|i| key.powers().get(i).is_some_and(|&n| n > 0));
            self.reducer.program_view().zero_sectors().contains(&sector)
        }
    }

    /// Compress the candidate terminals with census rows (or with no rows if
    /// the session has no census) at the structure sample.
    pub fn compress(
        &mut self,
        preferred: &[IntegralKey],
    ) -> Result<CompressionReport, LiteRedError> {
        let start = Instant::now();
        let (mut rows, row_statistics) = match &self.census {
            Some(census) => {
                let zero = self.zero_test();
                census_rows(
                    census,
                    &self.terminals,
                    self.options.rows,
                    &zero,
                    self.options.max_orbit,
                )?
            }
            None => (Vec::new(), CensusRowStatistics::default()),
        };
        let terminal_set: std::collections::HashSet<&IntegralKey> = self.terminals.iter().collect();
        for key in preferred {
            if !terminal_set.contains(key) {
                rows.push(CensusRow {
                    source: key.clone(),
                    origin: RowOrigin::Preferred,
                    terms: vec![(key.clone(), self.reducer.coefficient_context().base().one())],
                });
            }
        }
        let mut keys: Vec<IntegralKey> = rows
            .iter()
            .flat_map(|row| row.terms.iter().map(|(key, _)| key.clone()))
            .collect();
        keys.sort();
        keys.dedup();
        self.add_targets(&keys)?;
        let row_seconds = start.elapsed().as_secs_f64();
        let columns = ColumnSpace::new(
            self.terminals.clone(),
            preferred.to_vec(),
            self.options.order,
            &self.reducer.ordering(),
        )?;
        let terminal_column = self
            .terminals
            .iter()
            .map(|key| columns.position[key])
            .collect();
        let mut compressed = Compressed {
            columns,
            rows,
            pivots: Vec::new(),
            masters: Vec::new(),
            terminal_column,
        };
        let start = Instant::now();
        let elimination = self.eliminate(&compressed, &self.values, self.build.sample())?;
        compressed.pivots = elimination.pivot_columns();
        let (masters, _) = elimination.solve(&self.build.sample().field);
        compressed.masters = masters;
        let elimination_seconds = start.elapsed().as_secs_f64();
        let master_keys: Vec<IntegralKey> = compressed
            .masters
            .iter()
            .map(|&column| compressed.columns.columns[column].clone())
            .collect();
        let classes = self.class_counts(&master_keys)?;
        let report = CompressionReport {
            terminals: self.terminals.len(),
            columns: compressed.columns.len(),
            preferred_columns: compressed.columns.preferred,
            rows: row_statistics,
            rank: elimination.rank,
            masters: master_keys,
            classes,
            dag_nodes: self.structure.len(),
            row_seconds,
            elimination_seconds,
        };
        self.compressed = Some(compressed);
        Ok(report)
    }

    pub fn masters(&self) -> Vec<IntegralKey> {
        self.compressed.as_ref().map_or_else(Vec::new, |c| {
            c.masters
                .iter()
                .map(|&column| c.columns.columns[column].clone())
                .collect()
        })
    }

    fn class_counts(&self, masters: &[IntegralKey]) -> Result<Vec<ClassCount>, LiteRedError> {
        let Some(census) = &self.census else {
            return Ok(Vec::new());
        };
        let mut counts: HashMap<Mask, usize> = HashMap::new();
        for master in masters {
            let sector = Mask::try_from_indices(master.powers())
                .map_err(|e| LiteRedError::Census(e.to_string()))?;
            if let Some(representative) = census.representative(&sector) {
                *counts.entry(representative.clone()).or_default() += 1;
            }
        }
        let mut classes: Vec<ClassCount> = census
            .classes()
            .iter()
            .map(|class| ClassCount {
                representative: class.representative().clone(),
                representative_id: class
                    .representative()
                    .active_bits()
                    .iter()
                    .enumerate()
                    .filter(|(_, active)| **active)
                    .map(|(slot, _)| 1u128 << slot)
                    .sum(),
                members: class.members().len(),
                masters: counts.get(class.representative()).copied().unwrap_or(0),
            })
            .collect();
        classes.sort_by(|a, b| sector_id_cmp(&a.representative, &b.representative));
        Ok(classes)
    }

    fn eliminate(
        &self,
        compressed: &Compressed,
        values: &DagValues,
        sample: &ZpSample,
    ) -> Result<Elimination, LiteRedError> {
        let field = &sample.field;
        let forward = modular::forward(&self.structure, values, field)?;
        let width = compressed.columns.len();
        let mut elimination = Elimination::new(width);
        for row in &compressed.rows {
            let mut dense = vec![0u64; width];
            if row.origin == RowOrigin::Preferred {
                let column = compressed.columns.position[&row.source];
                dense[column] = 1;
                let node = self.node(&row.source)?;
                for &(terminal, value) in modular::require(&forward, node)? {
                    let position = compressed.terminal_column[terminal as usize];
                    dense[position] = field.sub(dense[position], value);
                }
            } else {
                for (key, coefficient) in &row.terms {
                    let weight =
                        sample
                            .coefficient(coefficient)?
                            .ok_or(LiteRedError::UnluckySample {
                                phase: "census row coefficient",
                                detail: format!("{key:?}"),
                            })?;
                    if weight == 0 {
                        continue;
                    }
                    let node = self.node(key)?;
                    for &(terminal, value) in modular::require(&forward, node)? {
                        let position = compressed.terminal_column[terminal as usize];
                        dense[position] = field.mul_add(dense[position], weight, value);
                    }
                }
            }
            elimination.add_row(dense, field);
        }
        Ok(elimination)
    }

    fn node(&self, key: &IntegralKey) -> Result<u32, LiteRedError> {
        self.structure
            .node_of(&pad::<N>(key.powers()))
            .ok_or_else(|| LiteRedError::InvalidInput(format!("{key:?} is not in the DAG")))
    }

    /// Node ids of keys already in the DAG.
    pub fn nodes(&self, keys: &[IntegralKey]) -> Result<Vec<u32>, LiteRedError> {
        keys.iter().map(|key| self.node(key)).collect()
    }

    /// Coefficients of `targets` (DAG node ids) onto the masters at a sample.
    /// The pivot set must equal the structure sample's (else `UnluckySample`).
    pub fn reduce_at(
        &self,
        sample: &ZpSample,
        targets: &[u32],
    ) -> Result<SampleReduction, LiteRedError> {
        let compressed = self.compressed.as_ref().ok_or_else(|| {
            LiteRedError::InvalidInput("compress() must run before reduce_at()".into())
        })?;
        let owned;
        let values = if sample == self.build.sample() {
            &self.values
        } else {
            let view = self.view();
            let mut program = SampleProgram::new(view, sample.clone())?;
            owned = self.structure.evaluate(view, &mut program)?;
            &owned
        };
        let field = &sample.field;
        let elimination = self.eliminate(compressed, values, sample)?;
        if elimination.pivot_columns() != compressed.pivots {
            return Err(LiteRedError::UnluckySample {
                phase: "compression pivots",
                detail: format!("rank {} at prime {}", elimination.rank, field.prime()),
            });
        }
        let (masters, map) = elimination.solve(field);
        debug_assert_eq!(masters, compressed.masters);
        let forward = modular::forward(&self.structure, values, field)?;
        let mut coefficients = Vec::with_capacity(targets.len());
        for &target in targets {
            let mut dense = vec![0u64; masters.len()];
            for &(terminal, value) in modular::require(&forward, target)? {
                for &(master, coefficient) in &map[compressed.terminal_column[terminal as usize]] {
                    let slot = &mut dense[master as usize];
                    *slot = field.mul_add(*slot, value, coefficient);
                }
            }
            coefficients.push(dense);
        }
        Ok(SampleReduction {
            prime: field.prime(),
            coefficients,
        })
    }

    /// Exact coefficients in `Q(x)`, `x` the single `None` entry of `fixed`
    /// (in [`Self::parameter_names`] order), the others fixed rationals.
    /// Per prime, every entry is interpolated by an early-terminating Thiele
    /// continued fraction over random Zp points of `x`; normalized polynomial
    /// coefficients are combined by CRT and MQRR, and a reconstruction is
    /// accepted only once a further, unused prime reproduces every entry.
    pub fn reduce_univariate(
        &self,
        fixed: &[Option<Rational>],
        targets: &[u32],
    ) -> Result<UnivariateReduction, LiteRedError>
    where
        Self: Sync,
    {
        let start = Instant::now();
        let names = self.parameter_names().len();
        let free: Vec<usize> = (0..fixed.len()).filter(|&i| fixed[i].is_none()).collect();
        if fixed.len() != names || free.len() != 1 {
            return Err(LiteRedError::InvalidInput(format!(
                "univariate reduction needs {names} parameters with exactly one free"
            )));
        }
        let free = free[0];
        let width = self.compressed.as_ref().map_or(0, |c| c.masters.len());
        let entries = targets.len() * width;
        let mut accumulators: Vec<CrtAccumulator> = vec![CrtAccumulator::default(); entries];
        let mut modulus = Integer::one();
        let mut candidate: Option<Vec<RationalFunction>> = None;
        let (mut used, mut skipped, mut max_points) = (0usize, 0usize, 0usize);
        let workers = self.options.workers.max(1);
        for prime in reconstruction_primes(self.options.max_primes) {
            let field = PrimeField::new(prime)?;
            let base: Option<Vec<u64>> = fixed
                .iter()
                .map(|value| match value {
                    Some(value) => field.from_rational(value),
                    None => Some(0),
                })
                .collect();
            let Some(base) = base else {
                skipped += 1;
                continue;
            };
            let mut rng = StdRng::seed_from_u64(prime ^ self.options.seed);
            let mut thieles = vec![Thiele::default(); entries];
            let mut tried = 0usize;
            let mut unlucky = false;
            while thieles.iter().any(|t| !t.done()) {
                if tried >= MAX_POINTS_PER_PRIME {
                    unlucky = true;
                    break;
                }
                let xs: Vec<u64> = (0..workers).map(|_| rng.random_range(1..prime)).collect();
                tried += xs.len();
                let results: Vec<Result<Option<SampleReduction>, LiteRedError>> = xs
                    .par_iter()
                    .map(|&x| {
                        let mut point = base.clone();
                        point[free] = x;
                        match self.reduce_at(&ZpSample::new(field.clone(), point), targets) {
                            Ok(reduction) => Ok(Some(reduction)),
                            Err(LiteRedError::UnluckySample { .. }) => Ok(None),
                            Err(error) => Err(error),
                        }
                    })
                    .collect();
                for (&x, result) in xs.iter().zip(results) {
                    let Some(reduction) = result? else {
                        continue;
                    };
                    for (entry, &y) in reduction.coefficients.concat().iter().enumerate() {
                        thieles[entry].feed(x, y, &field);
                    }
                }
            }
            if unlucky || thieles.iter().any(Thiele::failed) {
                skipped += 1;
                continue;
            }
            let images: Option<Vec<(Vec<u64>, Vec<u64>)>> =
                thieles.iter().map(|t| t.polynomials(&field)).collect();
            let Some(images) = images else {
                skipped += 1;
                continue;
            };
            max_points = max_points.max(thieles.iter().map(Thiele::points).max().unwrap_or(0));
            if let Some(guess) = &candidate {
                let verified = guess
                    .iter()
                    .zip(&images)
                    .all(|(g, (n, d))| CrtAccumulator::matches(g, n, d, &field));
                if verified {
                    let coefficients = if width == 0 {
                        vec![Vec::new(); targets.len()]
                    } else {
                        guess
                            .chunks(width)
                            .map(<[RationalFunction]>::to_vec)
                            .collect()
                    };
                    return Ok(UnivariateReduction {
                        free_parameter: free,
                        primes_used: used + 1,
                        skipped_primes: skipped,
                        max_points,
                        coefficients,
                        seconds: start.elapsed().as_secs_f64(),
                    });
                }
            }
            let consistent = accumulators
                .iter()
                .zip(&images)
                .all(|(acc, (n, d))| acc.shape().is_none_or(|shape| shape == (n.len(), d.len())));
            if !consistent {
                skipped += 1;
                continue;
            }
            for (acc, (n, d)) in accumulators.iter_mut().zip(&images) {
                acc.add(n, d, prime, &modulus);
            }
            modulus = &modulus * &Integer::from(prime);
            used += 1;
            candidate = accumulators
                .iter()
                .map(|acc| acc.reconstruct(&modulus))
                .collect();
        }
        Err(LiteRedError::Unstable(format!(
            "univariate reconstruction did not stabilise ({used} primes used, {skipped} skipped)"
        )))
    }

    /// Exact coefficients at a rational point of the base parameters (in
    /// [`Self::parameter_names`] order), by CRT over successive primes and
    /// MQRR; a reconstruction is accepted only once a further, unused prime
    /// reproduces every entry.
    pub fn reduce_exact_at_point(
        &self,
        point: &[Rational],
        targets: &[u32],
    ) -> Result<ExactPointReduction, LiteRedError>
    where
        Self: Sync,
    {
        let start = Instant::now();
        let primes = reconstruction_primes(self.options.max_primes);
        let width = self.compressed.as_ref().map_or(0, |c| c.masters.len());
        let entries = targets.len() * width;
        let mut residues: Vec<Integer> = Vec::new();
        let mut modulus = Integer::one();
        let mut candidate: Option<Vec<Rational>> = None;
        let mut used = 0usize;
        let mut skipped = 0usize;
        for batch in primes.chunks(self.options.workers.max(1)) {
            let results: Vec<Result<Option<SampleReduction>, LiteRedError>> = batch
                .par_iter()
                .map(|&prime| {
                    let field = PrimeField::new(prime)?;
                    let Some(sample) = ZpSample::from_rationals(field, point) else {
                        return Ok(None);
                    };
                    match self.reduce_at(&sample, targets) {
                        Ok(reduction) => Ok(Some(reduction)),
                        Err(LiteRedError::UnluckySample { .. }) => Ok(None),
                        Err(error) => Err(error),
                    }
                })
                .collect();
            for result in results {
                let Some(reduction) = result? else {
                    skipped += 1;
                    continue;
                };
                let prime = Integer::from(reduction.prime);
                let flat: Vec<u64> = reduction.coefficients.concat();
                debug_assert_eq!(flat.len(), entries);
                if let Some(guess) = &candidate {
                    let field = PrimeField::new(reduction.prime)?;
                    let verified = guess
                        .iter()
                        .zip(&flat)
                        .all(|(value, &image)| field.from_rational(value) == Some(image));
                    if verified {
                        let coefficients = guess
                            .chunks(width.max(1))
                            .map(<[Rational]>::to_vec)
                            .collect();
                        return Ok(ExactPointReduction {
                            point: point.to_vec(),
                            primes_used: used + 1,
                            skipped_primes: skipped,
                            coefficients: if width == 0 {
                                vec![Vec::new(); targets.len()]
                            } else {
                                coefficients
                            },
                            seconds: start.elapsed().as_secs_f64(),
                        });
                    }
                }
                if used == 0 {
                    residues = flat.iter().map(|&v| Integer::from(v)).collect();
                    modulus = prime;
                } else {
                    residues = residues
                        .into_iter()
                        .zip(&flat)
                        .map(|(old, &new)| {
                            Integer::chinese_remainder(
                                old,
                                Integer::from(new),
                                modulus.clone(),
                                prime.clone(),
                            )
                        })
                        .collect();
                    modulus = &modulus * &prime;
                }
                used += 1;
                candidate = residues
                    .iter()
                    .map(|value| {
                        Rational::maximal_quotient_reconstruction(value, &modulus, None).ok()
                    })
                    .collect();
            }
        }
        Err(LiteRedError::Unstable(format!(
            "rational reconstruction did not stabilise within {} primes ({skipped} skipped)",
            primes.len()
        )))
    }
}
