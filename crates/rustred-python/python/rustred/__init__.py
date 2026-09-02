"""Python frontend for RustRed's shared application API."""

from enum import StrEnum

from ._rustred import (
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
    derive,
    generate_closing_artifact,
    inspect_closing_artifact,
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
    "derive",
    "generate_closing_artifact",
    "inspect_closing_artifact",
    "reduce_with_closing_artifact",
    "run_foundry_campaign",
    "run_foundry_wave_campaign",
]
