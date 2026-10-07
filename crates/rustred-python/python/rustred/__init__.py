"""Python frontend for RustRed's shared application API."""

from enum import StrEnum
from .discovery import discovery_strategy, rule_portfolio
from .ordering import integral_order

from ._rustred import (
    CandidateBundleResult,
    CandidateGenerationRequest,
    CandidateGenerationSession,
    CandidateArtifact,
    TerminalNormalization,
    CampaignPlanResult,
    CampaignPreflightResult,
    ClosingArtifactGenerationResult,
    ClosingArtifactInspectionResult,
    ClosingArtifactReductionResult,
    DeriveResult,
    ExactMasterCoefficient,
    FoundryCampaignRunResult,
    FoundryWaveCampaignRunResult,
    RustRedCoordinatorPoisonedError,
    RustRedDerivationError,
    RustRedError,
    RustRedExecutionError,
    RustRedInputError,
    RustRedInternalError,
    RustRedLicenseError,
    RustRedLimitError,
    RustRedLoweringError,
    RustRedOutputLimitError,
    RustRedSchemaError,
    RustRedSerializationError,
    __version__,
    campaign_plan,
    campaign_preflight,
    certify_candidates,
    derive,
    entry_domain_plan,
    execution_capabilities,
    family_close,
    family_candidates,
    candidate_generation_request,
    start_family_candidates,
    generate_closing_artifact,
    inspect_closing_artifact,
    inspect_candidate_program,
    reduce_with_closing_artifact,
    run_foundry_campaign,
    run_foundry_wave_campaign,
)


class InputFormat(StrEnum):
    """Accepted family and campaign source encodings."""

    AUTO = "auto"
    TOML = "toml"
    SYMBOLICA = "symbolica"


class RelationSelection(StrEnum):
    """Parametric relation families emitted by :func:`derive`."""

    ALL = "all"
    ORDINARY = "ordinary"
    LORENTZ_INVARIANCE = "li"


class ClosingFamily(StrEnum):
    """Semantic family presets available to closing-artifact generation."""

    UNIT_MASS_VACUUM_K1 = "unit-mass-vacuum-k1"
    UNIT_MASS_VACUUM_K3 = "unit-mass-vacuum-k3"


__all__ = [
    "CandidateGenerationRequest",
    "CandidateGenerationSession",
    "CandidateArtifact",
    "TerminalNormalization",
    "candidate_generation_request",
    "start_family_candidates",
    "discovery_strategy",
    "rule_portfolio",
    "integral_order",
    "CandidateBundleResult",
    "CampaignPlanResult",
    "CampaignPreflightResult",
    "ClosingArtifactGenerationResult",
    "ClosingArtifactInspectionResult",
    "ClosingArtifactReductionResult",
    "ClosingFamily",
    "DeriveResult",
    "ExactMasterCoefficient",
    "FoundryCampaignRunResult",
    "FoundryWaveCampaignRunResult",
    "InputFormat",
    "RelationSelection",
    "RustRedCoordinatorPoisonedError",
    "RustRedDerivationError",
    "RustRedError",
    "RustRedExecutionError",
    "RustRedInputError",
    "RustRedInternalError",
    "RustRedLicenseError",
    "RustRedLimitError",
    "RustRedLoweringError",
    "RustRedOutputLimitError",
    "RustRedSchemaError",
    "RustRedSerializationError",
    "__version__",
    "campaign_plan",
    "campaign_preflight",
    "certify_candidates",
    "derive",
    "entry_domain_plan",
    "execution_capabilities",
    "family_close",
    "family_candidates",
    "generate_closing_artifact",
    "inspect_closing_artifact",
    "inspect_candidate_program",
    "reduce_with_closing_artifact",
    "run_foundry_campaign",
    "run_foundry_wave_campaign",
]
