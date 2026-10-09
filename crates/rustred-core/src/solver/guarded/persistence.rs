//! Native generated-data transport bound to an expected guarded source context.

use std::sync::Arc;

use bincode::{Decode, Encode};

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId,
    CoefficientTableBuilder, DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use crate::solver::{
    CoordinateCase, Integral, IntegralOrder, Power, RuleCandidate, SearchStats, Seed, SeedSource,
    Term,
};

use super::{
    GuardedProgram, GuardedRule, GuardedSourceSystem, IndexBounds, IndexDomain, IndexRole,
};

const SCHEMA: &str = "rustred.guarded-source-program.v1";
const STRUCTURE_LIMIT: usize = 64 * 1024 * 1024;

type DomainRecord = Vec<(Option<i64>, Option<i64>)>;
type IntegralRecord = Vec<(bool, i16)>;

#[derive(Encode, Decode)]
struct TermRecord {
    integral: IntegralRecord,
    coefficient: usize,
}

#[derive(Encode, Decode)]
struct SourceRecord {
    id: String,
    domain: DomainRecord,
    conditions: Vec<usize>,
    terms: Vec<TermRecord>,
}

#[derive(Encode, Decode)]
struct SeedRecord {
    row: usize,
    integral: IntegralRecord,
    shifts: Vec<i16>,
}

#[derive(Encode, Decode)]
struct RuleRecord {
    fixed: Vec<Option<i16>>,
    target: IntegralRecord,
    rhs: Vec<TermRecord>,
    sources: Vec<SeedRecord>,
    domain: DomainRecord,
    discovery_domain: DomainRecord,
    conditions: Vec<usize>,
    sector: Vec<bool>,
    permutation: Option<Vec<usize>>,
}

#[derive(Encode, Decode)]
struct Record {
    schema: String,
    measure: String,
    roles: Vec<u8>,
    indices: Vec<usize>,
    sources: Vec<SourceRecord>,
    zero_domains: Vec<DomainRecord>,
    rules: Vec<RuleRecord>,
    terminals: Vec<Vec<i64>>,
}

impl<const N: usize> GuardedProgram<N> {
    /// Save rules and their complete source binding using native Symbolica
    /// coefficients. This records conditional consequences, never family closure.
    pub fn encode_native(&self, limits: BinaryIoLimits) -> Result<Vec<u8>, BinaryIoError> {
        let mut table = CoefficientTableBuilder::new(limits);
        let mut sources = Vec::new();
        for (info, row) in self.sources.sources.iter().zip(self.sources.system.rows()) {
            sources.push(SourceRecord {
                id: info.id.clone(),
                domain: domain_record(&info.domain),
                conditions: encode_conditions(&info.nonzero_conditions, &mut table)?,
                terms: row
                    .iter()
                    .map(|term| {
                        Ok(TermRecord {
                            integral: integral_record(&term.integral),
                            coefficient: table.intern(&term.coefficient.clone().into())?.index(),
                        })
                    })
                    .collect::<Result<_, BinaryIoError>>()?,
            });
        }
        let mut rules = Vec::new();
        for rule in &self.rules {
            if rule.candidate.case.coordinate().is_none()
                || rule.order.program().is_some()
                || rule.order.physical_arity() != N
                || rule.order.roles() != Some(&self.sources.roles)
            {
                return Err(BinaryIoError::Invalid(
                    "unsupported guarded case or order transport",
                ));
            }
            rules.push(RuleRecord {
                fixed: rule.candidate.case.fixed().to_vec(),
                target: integral_record(&rule.candidate.target),
                rhs: rule
                    .candidate
                    .rhs
                    .iter()
                    .map(|term| {
                        Ok(TermRecord {
                            integral: integral_record(&term.integral),
                            coefficient: table.intern(&term.coefficient)?.index(),
                        })
                    })
                    .collect::<Result<_, BinaryIoError>>()?,
                sources: rule
                    .candidate
                    .sources
                    .iter()
                    .map(|source| SeedRecord {
                        row: source.basis_row,
                        integral: integral_record(&source.seed.integral),
                        shifts: source.seed.shifts.to_vec(),
                    })
                    .collect(),
                domain: domain_record(&rule.domain),
                discovery_domain: domain_record(&rule.discovery_domain),
                conditions: encode_conditions(&rule.nonzero_conditions, &mut table)?,
                sector: rule.order.sector().to_vec(),
                permutation: rule
                    .order
                    .permutation()
                    .map(|permutation| permutation.to_vec()),
            });
        }
        let record = Record {
            schema: SCHEMA.into(),
            measure: self.sources.measure_id.clone(),
            roles: self
                .sources
                .roles
                .iter()
                .map(|role| role_code(*role))
                .collect(),
            indices: self.sources.system.index_variables().to_vec(),
            sources,
            zero_domains: self
                .sources
                .zero_domains
                .iter()
                .map(domain_record)
                .collect(),
            rules,
            terminals: self
                .terminals
                .iter()
                .map(|terminal| terminal.to_vec())
                .collect(),
        };
        validate_record_limits(&record, limits)?;
        let structure =
            bincode::encode_to_vec(&record, bincode::config::standard()).map_err(native)?;
        check_limit(
            "guarded program structure bytes",
            structure.len(),
            STRUCTURE_LIMIT.min(limits.max_program_bytes),
        )?;
        let encoded = table.finish()?;
        encode_program(
            BinaryProgramKind::DomainRules,
            &[
                BinarySection {
                    tag: SectionTag::SYMBOLICA_STATE,
                    bytes: &encoded.state,
                },
                BinarySection {
                    tag: SectionTag::COEFFICIENTS,
                    bytes: &encoded.atoms,
                },
                BinarySection {
                    tag: SectionTag::PROGRAM,
                    bytes: &structure,
                },
            ],
            limits,
        )
    }

    /// Load only generated native data from the matching RustRed/Symbolica
    /// stack. The supplied expected context must match every source, guard,
    /// coordinate role and measure identifier. Every loaded rule is replayed.
    pub fn decode_generated(
        bytes: &[u8],
        expected: Arc<GuardedSourceSystem<N>>,
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        let envelope = inspect_program(bytes, limits)?;
        if envelope.kind() != BinaryProgramKind::DomainRules || envelope.sections().len() != 3 {
            return Err(BinaryIoError::Invalid("not a guarded source program"));
        }
        let section = |tag| {
            envelope
                .section(tag)
                .ok_or(BinaryIoError::Invalid("missing guarded program section"))
        };
        let structure = section(SectionTag::PROGRAM)?;
        check_limit(
            "guarded program structure bytes",
            structure.len(),
            STRUCTURE_LIMIT.min(limits.max_program_bytes),
        )?;
        let (record, used): (Record, usize) = bincode::decode_from_slice(
            structure,
            bincode::config::standard().with_limit::<STRUCTURE_LIMIT>(),
        )
        .map_err(native)?;
        if used != structure.len() || record.schema != SCHEMA {
            return Err(BinaryIoError::Invalid(
                "guarded program schema or trailing bytes",
            ));
        }
        validate_record_limits(&record, limits)?;
        if record.measure != expected.measure_id
            || record.roles
                != expected
                    .roles
                    .iter()
                    .map(|role| role_code(*role))
                    .collect::<Vec<_>>()
            || record.indices.as_slice() != expected.system.index_variables().as_slice()
            || record.sources.len() != expected.sources.len()
            || record.zero_domains
                != expected
                    .zero_domains
                    .iter()
                    .map(domain_record)
                    .collect::<Vec<_>>()
        {
            return Err(BinaryIoError::Invalid(
                "guarded program source context mismatch",
            ));
        }
        let table = DecodedCoefficientTable::import_generated_normalized(
            section(SectionTag::SYMBOLICA_STATE)?,
            section(SectionTag::COEFFICIENTS)?,
            limits,
        )?;
        let coefficient = |id| -> Result<Coefficient, BinaryIoError> {
            let value = table.coefficient(CoefficientId::try_from_index(id)?)?;
            if value.numerator.variables().as_ref() != expected.system.coefficient_variables()
                || value.denominator.variables().as_ref() != expected.system.coefficient_variables()
            {
                return Err(BinaryIoError::Invalid(
                    "guarded coefficient variable map mismatch",
                ));
            }
            Ok(value.clone())
        };
        let polynomial = |id| -> Result<CoefficientPolynomial, BinaryIoError> {
            let value = coefficient(id)?;
            if !value.denominator.is_one() {
                return Err(BinaryIoError::Invalid(
                    "guarded polynomial has a denominator",
                ));
            }
            Ok(value.numerator)
        };
        for ((record, info), row) in record
            .sources
            .iter()
            .zip(&expected.sources)
            .zip(expected.system.rows())
        {
            if record.id != info.id
                || record.domain != domain_record(&info.domain)
                || record.conditions.len() != info.nonzero_conditions.len()
                || record.terms.len() != row.len()
            {
                return Err(BinaryIoError::Invalid(
                    "guarded original source metadata mismatch",
                ));
            }
            for (id, expected) in record.conditions.iter().zip(&info.nonzero_conditions) {
                if polynomial(*id)? != *expected {
                    return Err(BinaryIoError::Invalid(
                        "guarded original source condition mismatch",
                    ));
                }
            }
            for (term, expected) in record.terms.iter().zip(row) {
                if decode_integral::<N>(&term.integral)? != expected.integral
                    || polynomial(term.coefficient)? != expected.coefficient
                {
                    return Err(BinaryIoError::Invalid(
                        "guarded original source equation mismatch",
                    ));
                }
            }
        }
        let mut rules = Vec::new();
        for record in record.rules {
            let sector = array(record.sector)?;
            let mut order = IntegralOrder::new(
                sector,
                expected.roles.map(|role| role == IndexRole::RequiredCut),
            )
            .with_roles(expected.roles)
            .map_err(native)?;
            if let Some(permutation) = record.permutation {
                order = order
                    .with_permutation(array(permutation)?)
                    .map_err(native)?;
            }
            let candidate = RuleCandidate {
                case: CoordinateCase::new(array(record.fixed)?)
                    .map_err(native)?
                    .into(),
                target: decode_integral(&record.target)?,
                rhs: record
                    .rhs
                    .into_iter()
                    .map(|term| {
                        Ok(Term {
                            integral: decode_integral(&term.integral)?,
                            coefficient: coefficient(term.coefficient)?,
                        })
                    })
                    .collect::<Result<_, BinaryIoError>>()?,
                sources: record
                    .sources
                    .into_iter()
                    .map(|source| {
                        if source.row >= expected.sources.len() {
                            return Err(BinaryIoError::Invalid(
                                "guarded proof source ordinal out of range",
                            ));
                        }
                        Ok(SeedSource {
                            basis_row: source.row,
                            seed: Seed {
                                integral: decode_integral(&source.integral)?,
                                shifts: array(source.shifts)?,
                            },
                        })
                    })
                    .collect::<Result<_, BinaryIoError>>()?,
                stats: SearchStats::default(),
            };
            rules.push(GuardedRule {
                candidate,
                domain: decode_domain(record.domain)?,
                discovery_domain: decode_domain(record.discovery_domain)?,
                nonzero_conditions: record
                    .conditions
                    .into_iter()
                    .map(&polynomial)
                    .collect::<Result<_, _>>()?,
                order,
            });
        }
        let terminals = record
            .terminals
            .into_iter()
            .map(array)
            .collect::<Result<Vec<[i64; N]>, _>>()?;
        // The constructor independently replays, including guard propagation
        // and strict descent. Decoding alone grants no mathematical authority.
        Self::new(expected, rules, terminals).map_err(native)
    }
}

fn role_code(role: IndexRole) -> u8 {
    match role {
        IndexRole::Ordinary => 0,
        IndexRole::RequiredCut => 1,
        IndexRole::Occupation => 2,
    }
}

fn domain_record<const N: usize>(domain: &IndexDomain<N>) -> DomainRecord {
    domain
        .bounds()
        .iter()
        .map(|bound| (bound.lower(), bound.upper()))
        .collect()
}

fn decode_domain<const N: usize>(record: DomainRecord) -> Result<IndexDomain<N>, BinaryIoError> {
    let bounds = record
        .into_iter()
        .map(|(lower, upper)| IndexBounds::new(lower, upper).map_err(native))
        .collect::<Result<Vec<_>, _>>()?;
    IndexDomain::new(array(bounds)?).map_err(native)
}

fn integral_record<const N: usize>(integral: &Integral<N>) -> IntegralRecord {
    integral
        .powers()
        .iter()
        .map(|power| (power.is_symbolic(), power.value()))
        .collect()
}

fn decode_integral<const N: usize>(record: &IntegralRecord) -> Result<Integral<N>, BinaryIoError> {
    let powers = record
        .iter()
        .map(|(symbolic, value)| Power::new(*symbolic, *value).map_err(native))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Integral::new(array(powers)?))
}

fn array<T, const N: usize>(values: Vec<T>) -> Result<[T; N], BinaryIoError> {
    values
        .try_into()
        .map_err(|_| BinaryIoError::Invalid("guarded coordinate arity mismatch"))
}

fn encode_conditions(
    conditions: &[CoefficientPolynomial],
    table: &mut CoefficientTableBuilder,
) -> Result<Vec<usize>, BinaryIoError> {
    conditions
        .iter()
        .map(|condition| table.intern(&condition.clone().into()).map(|id| id.index()))
        .collect()
}

fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), BinaryIoError> {
    if requested > limit {
        Err(BinaryIoError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn validate_record_limits(record: &Record, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    let mut entries = 0_usize;
    let mut add = |count: usize| -> Result<(), BinaryIoError> {
        entries = entries
            .checked_add(count)
            .ok_or(BinaryIoError::Invalid("guarded entry count overflow"))?;
        check_limit(
            "guarded program collection entries",
            entries,
            limits.max_collection_entries,
        )
    };
    add(record.sources.len())?;
    add(record.rules.len())?;
    add(record.terminals.len())?;
    add(record.zero_domains.len())?;
    for source in &record.sources {
        add(source.terms.len())?;
        add(source.conditions.len())?;
    }
    for rule in &record.rules {
        add(rule.rhs.len())?;
        add(rule.sources.len())?;
        add(rule.conditions.len())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::lifecycle::tests::sample;
    use super::*;

    #[test]
    fn native_roundtrip_replays_and_preserves_conditional_reduction() {
        let program = sample("native-roundtrip");
        let bytes = program.encode_native(Default::default()).unwrap();
        let restored =
            GuardedProgram::decode_generated(&bytes, program.sources.clone(), Default::default())
                .unwrap();
        let before = program.reduce([3], Default::default()).unwrap();
        let after = restored.reduce([3], Default::default()).unwrap();
        assert_eq!(before.terms, after.terms);
        assert_eq!(before.nonzero_conditions, after.nonzero_conditions);
        assert!(after.unresolved.is_empty());
    }

    #[test]
    fn saved_program_rejects_a_different_measure_and_source_guard() {
        let program = sample("measure-a");
        let bytes = program.encode_native(Default::default()).unwrap();
        let other = sample("measure-b");
        assert!(
            GuardedProgram::decode_generated(&bytes, other.sources, Default::default()).is_err()
        );
        let mut context = Arc::try_unwrap(sample("measure-a").sources).unwrap();
        context.sources[0].domain =
            IndexDomain::new([IndexBounds::new(Some(2), None).unwrap()]).unwrap();
        assert!(
            GuardedProgram::decode_generated(&bytes, Arc::new(context), Default::default())
                .is_err()
        );
    }

    #[test]
    fn saved_program_rejects_corrupt_rule_domain_via_replay() {
        let program = sample("replay-domain");
        let bytes = program.encode_native(Default::default()).unwrap();
        let envelope = inspect_program(&bytes, Default::default()).unwrap();
        let (mut record, _): (Record, _) = bincode::decode_from_slice(
            envelope.section(SectionTag::PROGRAM).unwrap(),
            bincode::config::standard(),
        )
        .unwrap();
        record.rules[0].domain = vec![(Some(0), None)];
        let structure = bincode::encode_to_vec(record, bincode::config::standard()).unwrap();
        let sections: Vec<_> = envelope
            .sections()
            .iter()
            .map(|section| BinarySection {
                tag: section.tag,
                bytes: if section.tag == SectionTag::PROGRAM {
                    &structure
                } else {
                    section.bytes
                },
            })
            .collect();
        let corrupt = encode_program(envelope.kind(), &sections, Default::default()).unwrap();
        assert!(
            GuardedProgram::decode_generated(&corrupt, program.sources.clone(), Default::default())
                .is_err()
        );
        let (mut record, _): (Record, _) = bincode::decode_from_slice(
            envelope.section(SectionTag::PROGRAM).unwrap(),
            bincode::config::standard(),
        )
        .unwrap();
        record.rules[0].rhs[0].coefficient = record.sources[0].terms[0].coefficient;
        let structure = bincode::encode_to_vec(record, bincode::config::standard()).unwrap();
        let sections: Vec<_> = envelope
            .sections()
            .iter()
            .map(|section| BinarySection {
                tag: section.tag,
                bytes: if section.tag == SectionTag::PROGRAM {
                    &structure
                } else {
                    section.bytes
                },
            })
            .collect();
        let corrupt = encode_program(envelope.kind(), &sections, Default::default()).unwrap();
        assert!(
            GuardedProgram::decode_generated(&corrupt, program.sources, Default::default())
                .is_err()
        );
    }
}
