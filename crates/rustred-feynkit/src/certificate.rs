//! Python view of a checked Laporta solution.

use std::collections::BTreeMap;

use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};
use rustred::{
    sector::{Mask, MasterCount, MasterCountOptions, MasterCounter, NoVerdictReason},
    solver::bridge::{ReductionCertificate, certify_laporta},
};

use crate::LaportaRecord;

/// One sector of the residuals, compared with its master count. A count that
/// could not be computed is kept as its error message.
struct SectorCheck {
    sector: Vec<bool>,
    residuals: usize,
    count: Result<MasterCount, String>,
}

impl SectorCheck {
    fn exceeds(&self) -> bool {
        match self.count {
            Ok(MasterCount::Zero) => true,
            Ok(MasterCount::Counted(masters)) => self.residuals > masters,
            Ok(MasterCount::NoVerdict { .. }) | Err(_) => false,
        }
    }

    fn no_verdict(&self) -> Option<String> {
        let reason = match &self.count {
            Ok(MasterCount::NoVerdict { reason, .. }) => match reason {
                NoVerdictReason::GramSingular => "singular external Gram matrix",
                NoVerdictReason::MorseNonIsolated => "non-isolated critical points",
                NoVerdictReason::MorseEulerMismatch => "critical-point counts disagree",
                NoVerdictReason::NegativeEuler => "negative Euler characteristic",
                NoVerdictReason::SampleDisagreement => "samples disagree",
            },
            Err(error) => return Some(format!("counting failed: {error}")),
            Ok(_) => return None,
        };
        Some(reason.to_owned())
    }
}

/// Checks of a ``reduce_laporta`` solution, from ``IBPSolution.certify()``.
///
/// ``reduction`` is ``"verified"`` when every rule was derived again from the
/// family's original identities and every returned rule follows from them, or
/// ``"unchecked"``; a failed check raises instead. Nonzero conditions are not
/// checked against the coefficients' poles. ``masters`` compares each residual sector with its number of master
/// integrals, counted without symmetries from critical points of ``U + F`` at
/// random finite-field kinematics:
///
/// - ``"incomplete"``: some sector holds more residuals than masters, so the
///   search missed relations; search deeper, for example with ``until_stable``.
/// - ``"count-consistent"``: no sector exceeds its count. This does not prove
///   the residuals independent: a valid but unusual master such as a dotted
///   integral is consistent too.
/// - ``"no-verdict"``: some sector's count is unreliable, for instance with a
///   singular external Gram matrix, where the parametric count can be lower
///   than what IBP relations reach; see ``no_verdict``.
/// - ``"unchecked"``: counting was not requested.
///
/// The counts are probabilistic but checked on two independent samples. They
/// compare sector by sector, so a relation the search misses between
/// integrals of different sectors, such as two equal-mass tadpoles related
/// by a light-like shift, is not detected. ``stable_depth`` repeats the
/// solution's ``until_stable`` result.
#[pyclass(name = "IBPCertificate", module = "symbolica.community.hepkit", frozen)]
pub struct PyIbpCertificate {
    replay: Option<ReductionCertificate>,
    sectors: Option<Vec<SectorCheck>>,
    stable_depth: Option<u32>,
    seed: u64,
}

impl PyIbpCertificate {
    pub(crate) fn check(
        record: &LaportaRecord,
        stable_depth: Option<u32>,
        count_masters: bool,
        replay: bool,
        seed: u64,
    ) -> Result<Self, String> {
        let replay = replay
            .then(|| {
                certify_laporta(
                    &record.family,
                    &record.cuts,
                    &record.solution,
                    record.include_lorentz,
                )
            })
            .transpose()
            .map_err(|error| error.to_string())?;
        let sectors = count_masters
            .then(|| Self::count(record, seed))
            .transpose()?;
        Ok(Self {
            replay,
            sectors,
            stable_depth,
            seed,
        })
    }

    /// Residual sectors lie inside any cut, where the uncut count applies.
    fn count(record: &LaportaRecord, seed: u64) -> Result<Vec<SectorCheck>, String> {
        let mut residuals = BTreeMap::<Vec<bool>, usize>::new();
        for residual in &record.solution.residuals {
            let sector = residual.iter().map(|&power| power > 0).collect();
            *residuals.entry(sector).or_default() += 1;
        }
        let options = MasterCountOptions {
            seed,
            ..Default::default()
        };
        let mut counter =
            MasterCounter::try_new(&record.family, options).map_err(|error| error.to_string())?;
        residuals
            .into_iter()
            .map(|(sector, residuals)| {
                let mask =
                    Mask::try_new(sector.iter().copied()).map_err(|error| error.to_string())?;
                let count = counter.count(&mask).map_err(|error| error.to_string());
                Ok(SectorCheck {
                    sector,
                    residuals,
                    count,
                })
            })
            .collect()
    }

    fn sector_dict<'py, T: IntoPyObject<'py>>(
        &self,
        py: Python<'py>,
        value: impl Fn(&SectorCheck) -> Option<T>,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(sectors) = &self.sectors else {
            return Ok(None);
        };
        let dict = PyDict::new(py);
        for check in sectors {
            if let Some(value) = value(check) {
                dict.set_item(PyTuple::new(py, &check.sector)?, value)?;
            }
        }
        Ok(Some(dict))
    }
}

#[pymethods]
impl PyIbpCertificate {
    #[getter]
    fn reduction(&self) -> &'static str {
        if self.replay.is_some() {
            "verified"
        } else {
            "unchecked"
        }
    }

    #[getter]
    fn masters(&self) -> &'static str {
        let Some(sectors) = &self.sectors else {
            return "unchecked";
        };
        if sectors.iter().any(SectorCheck::exceeds) {
            "incomplete"
        } else if sectors.iter().any(|check| check.no_verdict().is_some()) {
            "no-verdict"
        } else {
            "count-consistent"
        }
    }

    /// Master count per residual sector, ``None`` without a verdict.
    #[getter]
    fn master_counts<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sector_dict(py, |check| {
            Some(match check.count {
                Ok(MasterCount::Zero) => Some(0),
                Ok(MasterCount::Counted(masters)) => Some(masters),
                Ok(MasterCount::NoVerdict { .. }) | Err(_) => None,
            })
        })
    }

    /// Number of residuals per sector.
    #[getter]
    fn residual_counts<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sector_dict(py, |check| Some(check.residuals))
    }

    /// Sectors holding more residuals than masters.
    #[getter]
    fn excess_sectors(&self) -> Vec<Vec<bool>> {
        self.sectors
            .iter()
            .flatten()
            .filter(|check| check.exceeds())
            .map(|check| check.sector.clone())
            .collect()
    }

    /// Why a sector has no count verdict.
    #[getter]
    fn no_verdict<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        self.sector_dict(py, SectorCheck::no_verdict)
    }

    #[getter]
    fn stable_depth(&self) -> Option<u32> {
        self.stable_depth
    }

    /// Rules derived again from the original identities.
    #[getter]
    fn replayed_rules(&self) -> Option<usize> {
        self.replay.map(|replay| replay.replayed_rules)
    }

    /// Instantiated original identities used by the replay.
    #[getter]
    fn identities(&self) -> Option<usize> {
        self.replay.map(|replay| replay.identities)
    }

    #[getter]
    fn seed(&self) -> Option<u64> {
        self.sectors.as_ref().map(|_| self.seed)
    }

    fn __repr__(&self) -> String {
        let stable_depth = self
            .stable_depth
            .map_or_else(|| "None".to_owned(), |depth| depth.to_string());
        format!(
            "IBPCertificate(reduction='{}', masters='{}', excess_sectors={}, stable_depth={stable_depth})",
            self.reduction(),
            self.masters(),
            self.excess_sectors().len(),
        )
    }
}
