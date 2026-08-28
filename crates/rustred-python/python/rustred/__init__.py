"""Python frontend for RustRed's shared application API."""

from enum import StrEnum

from ._rustred import (
    CampaignPlanResult,
    CampaignPreflightResult,
    DeriveResult,
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


__all__ = [
    "CampaignPlanResult",
    "CampaignPreflightResult",
    "DeriveResult",
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
]
