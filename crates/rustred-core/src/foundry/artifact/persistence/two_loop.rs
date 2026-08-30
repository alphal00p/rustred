//! Durable K=3 closure payload encoding.

use crate::foundry::cell::{
    ResidualTermDisposition, RuleCell, RuleCellDomainProof, SourceViewConstruction,
};
use crate::sector::Mask;

use super::super::error::ArtifactPersistenceError;
use super::super::model::ClosedArtifact;
use super::binary::Writer;
use super::coefficient;
use super::semantic::{
    encode_bool_slice, encode_i64_slice, encode_integral_key, encode_row_id, encode_rule_snapshot,
};
use super::{
    REGISTERED_RULE_CELL_PLAN, TWO_LOOP_CLOSURE_HEADER, encode_into_writer, encode_source_snapshot,
};

pub(super) fn encode(
    writer: &mut Writer,
    artifact: &ClosedArtifact,
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(artifact.rule_cells().len(), "artifact rule cells")?;
    let mut header = writer.child();
    let canonicalizer =
        artifact
            .canonicalizer()
            .ok_or(ArtifactPersistenceError::UnsupportedFeature {
                detail: "two-loop closure has no canonical symmetry owner",
            })?;
    header.string(
        canonicalizer.ordering().stable_id(),
        "canonical ordering identifier",
    )?;
    header.usize(canonicalizer.generator_count(), "canonical generators")?;
    header.usize(canonicalizer.group_order(), "canonical group elements")?;
    for mapping in canonicalizer.group_elements() {
        header.usize(mapping.len(), "canonical permutation arity")?;
        for &source in mapping {
            header.usize(source, "canonical permutation source")?;
        }
    }
    header.usize(artifact.dependencies().len(), "artifact dependencies")?;
    for dependency in artifact.dependencies() {
        // Nested documents own independent byte buffers but inherit this
        // root writer's aggregate coefficient and semantic-witness budgets.
        let mut dependency_writer = writer.child();
        encode_into_writer(dependency, &mut dependency_writer)?;
        let durable = dependency_writer.finish();
        header.bytes(&durable, "dependency artifact bytes")?;
    }
    header.usize(
        artifact.factorization_rules().len(),
        "artifact factorization rules",
    )?;
    for factorization in artifact.factorization_rules() {
        encode_domain(
            &mut header,
            factorization.application_domain().sector(),
            factorization.application_domain().bounds(),
        )?;
        coefficient::encode_base_coefficient(&mut header, factorization.normalization())?;
        header.usize(
            factorization.loop_basis().dimension(),
            "factorization loop-basis dimension",
        )?;
        encode_i64_slice(&mut header, factorization.loop_basis().row_major())?;
        header.usize(factorization.factors().len(), "factorization factors")?;
        for factor in factorization.factors() {
            header.usize(
                factor.dependency_ordinal(),
                "factorization dependency ordinal",
            )?;
            header.usize(
                factor.parent_positions().len(),
                "factorization parent positions",
            )?;
            for &position in factor.parent_positions() {
                header.usize(position, "factorization parent position")?;
            }
            header.usize(
                factor.transformed_loop_positions().len(),
                "factor transformed loops",
            )?;
            for &position in factor.transformed_loop_positions() {
                header.usize(position, "factor transformed loop position")?;
            }
        }
        header.usize(
            factorization.master_embeddings().len(),
            "factorization master embeddings",
        )?;
        for embedding in factorization.master_embeddings() {
            encode_integral_key(&mut header, embedding.raw_parent_master())?;
            encode_integral_key(&mut header, embedding.parent_terminal())?;
        }
    }
    writer.u16(TWO_LOOP_CLOSURE_HEADER)?;
    writer.bytes(&header.finish(), "two-loop closure header")?;

    for cell in artifact.rule_cells() {
        let mut plan = writer.child();
        encode_rule_cell_snapshot(&mut plan, cell)?;
        let bytes = plan.finish();
        writer.charge_witness_payload(bytes.len())?;
        writer.u16(REGISTERED_RULE_CELL_PLAN)?;
        writer.bytes(&bytes, "rule-cell plan bytes")?;
    }
    Ok(())
}

fn encode_rule_cell_snapshot(
    writer: &mut Writer,
    cell: &RuleCell,
) -> Result<(), ArtifactPersistenceError> {
    let rule = encode_rule_snapshot(cell.rule(), writer)?;
    writer.bytes(&rule, "rule-cell exact rule snapshot")?;
    encode_domain(
        writer,
        cell.proof_domain().sector(),
        cell.proof_domain().bounds(),
    )?;
    encode_domain(
        writer,
        cell.application_domain().sector(),
        cell.application_domain().bounds(),
    )?;
    writer.u8(match cell.domain_proof() {
        RuleCellDomainProof::TightenedOriginalInterior => 0,
        RuleCellDomainProof::ReprovedSectorMonotone => 1,
    })?;
    encode_fixed_restrictions(writer, cell.fixed_restrictions())?;
    writer.usize(cell.pruned_rhs_ordinals().len(), "pruned RHS ordinals")?;
    for &ordinal in cell.pruned_rhs_ordinals() {
        writer.usize(ordinal, "pruned RHS ordinal")?;
    }
    writer.usize(cell.terms().len(), "retained rule-cell terms")?;
    for term in cell.terms() {
        writer.usize(term.source_rhs_ordinal(), "retained RHS ordinal")?;
    }
    writer.usize(cell.guards().len(), "rule-cell guards")?;
    for guard in cell.guards() {
        writer.usize(guard.source_guard_ordinal(), "source guard ordinal")?;
        coefficient::encode_indexed_polynomial(writer, guard.polynomial())?;
    }

    let sources = cell.sources();
    writer.string(
        sources.family_fingerprint(),
        "source-view family fingerprint",
    )?;
    writer.string(
        sources.context_fingerprint(),
        "source-view context fingerprint",
    )?;
    encode_source_snapshot(writer, sources.relations())?;
    writer.usize(sources.provenance().len(), "source-view provenance")?;
    for provenance in sources.provenance() {
        let translated = provenance.translated();
        writer.usize(translated.source_ordinal(), "translated source ordinal")?;
        encode_row_id(writer, translated.source_row())?;
        encode_i64_slice(writer, translated.offset().values())?;
        match provenance.symmetry() {
            None => writer.u8(0)?,
            Some(symmetry) => {
                writer.u8(1)?;
                writer.usize(symmetry.group_element(), "source symmetry group element")?;
            }
        }
    }
    match sources.construction() {
        SourceViewConstruction::Direct => writer.u8(0)?,
        SourceViewConstruction::ResidualProjection(evidence) => {
            writer.u8(1)?;
            encode_domain(
                writer,
                evidence.domain().sector(),
                evidence.domain().bounds(),
            )?;
            encode_fixed_restrictions(writer, evidence.fixed_restrictions())?;
            encode_source_snapshot(writer, evidence.original_relations())?;
            writer.usize(
                evidence.stabilizer_group_elements().len(),
                "projection stabilizer elements",
            )?;
            for &group_element in evidence.stabilizer_group_elements() {
                writer.usize(group_element, "projection stabilizer element")?;
            }
            writer.usize(
                evidence.term_projections().len(),
                "projected relation term rows",
            )?;
            for row in evidence.term_projections() {
                writer.usize(row.len(), "projected relation terms")?;
                for term in row.iter() {
                    encode_i64_slice(writer, term.source_shift())?;
                    match term.disposition() {
                        ResidualTermDisposition::CoefficientZero => writer.u8(0)?,
                        ResidualTermDisposition::ProvedZero { zero_sector } => {
                            writer.u8(1)?;
                            encode_bool_slice(writer, zero_sector.active_bits())?;
                        }
                        ResidualTermDisposition::Routed {
                            group_element,
                            projected_shift,
                        } => {
                            writer.u8(2)?;
                            writer.usize(*group_element, "projection route group element")?;
                            encode_i64_slice(writer, projected_shift)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn encode_domain(
    writer: &mut Writer,
    sector: &Mask,
    bounds: &[crate::sector::InteriorBounds],
) -> Result<(), ArtifactPersistenceError> {
    encode_bool_slice(writer, sector.active_bits())?;
    writer.usize(bounds.len(), "domain bounds")?;
    for bounds in bounds {
        writer.i64(bounds.lower())?;
        writer.i64(bounds.upper())?;
    }
    Ok(())
}

fn encode_fixed_restrictions(
    writer: &mut Writer,
    fixed: &[crate::foundry::cell::FixedIndexRestriction],
) -> Result<(), ArtifactPersistenceError> {
    writer.usize(fixed.len(), "fixed-index restrictions")?;
    for restriction in fixed {
        writer.usize(restriction.position(), "fixed-index position")?;
        writer.i64(restriction.value())?;
    }
    Ok(())
}
