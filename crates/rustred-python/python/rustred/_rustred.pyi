import os
from typing import final

class RustRedError(Exception): ...
class RustRedInputError(RustRedError): ...
class RustRedSchemaError(RustRedInputError): ...
class RustRedLimitError(RustRedInputError): ...
class RustRedLoweringError(RustRedInputError): ...
class RustRedDerivationError(RustRedError): ...
class RustRedExecutionError(RustRedError): ...
class RustRedLicenseError(RustRedExecutionError): ...
class RustRedSerializationError(RustRedError): ...
class RustRedOutputLimitError(RustRedSerializationError): ...
class RustRedInternalError(RustRedError): ...
class RustRedCoordinatorPoisonedError(RustRedInternalError): ...

@final
class CandidateBundleResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def bundle(self) -> bytes: ...
    def to_toml(self) -> str: ...

@final
class DeriveResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    def to_toml(self) -> str: ...

@final
class CampaignPlanResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    def to_toml(self) -> str: ...

@final
class CampaignPreflightResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    def to_toml(self) -> str: ...

@final
class FoundryCampaignRunResult:
    @property
    def schema(self) -> str: ...
    @property
    def measurements_schema(self) -> str: ...
    def to_toml(self) -> str: ...
    def measurements_to_toml(self) -> str: ...

@final
class FoundryWaveCampaignRunResult:
    @property
    def schema(self) -> str: ...
    @property
    def measurements_schema(self) -> str: ...
    @property
    def artifact_bytes(self) -> bytes | None: ...
    def to_toml(self) -> str: ...
    def measurements_to_toml(self) -> str: ...

@final
class ClosingArtifactGenerationResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def artifact(self) -> bytes: ...
    def to_toml(self) -> str: ...

@final
class ClosingArtifactInspectionResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    def to_toml(self) -> str: ...

@final
class ExactMasterCoefficient:
    @property
    def master_powers(self) -> list[int]: ...
    @property
    def unit_mass_coefficient(self) -> str: ...
    @property
    def common_mass_squared_power(self) -> int: ...

@final
class ClosingArtifactReductionResult:
    @property
    def schema(self) -> str: ...
    @property
    def status(self) -> str: ...
    @property
    def family_fingerprint(self) -> str: ...
    @property
    def target_powers(self) -> list[int]: ...
    @property
    def terms(self) -> list[ExactMasterCoefficient]: ...
    def to_toml(self) -> str: ...

__version__: str

def derive(
    source: str,
    *,
    input_format: str = "auto",
    relations: str = "all",
    n_cores: int = 1,
) -> DeriveResult: ...

def campaign_plan(
    source: str,
    *,
    input_format: str = "auto",
    root_id: str | None = None,
) -> CampaignPlanResult: ...

def campaign_preflight(
    profile: str,
    *,
    n_cores: int = 1,
    max_memory_bytes: int,
) -> CampaignPreflightResult: ...

def run_foundry_campaign(
    config: str,
) -> FoundryCampaignRunResult: ...

def run_foundry_wave_campaign(
    config: str,
    *,
    n_cores: int = 1,
) -> FoundryWaveCampaignRunResult: ...

def generate_closing_artifact(
    *,
    family: str = "unit-mass-vacuum-k1",
) -> ClosingArtifactGenerationResult: ...

def family_close(
    source: str,
    *,
    input_format: str = "auto",
    n_cores: int = 1,
    permutation: list[int] | None = None,
    nonpositive_indices: list[int] | None = None,
    max_domain_bound_endpoint_cells: int | None = None,
    max_predicate_consistency_work: int | None = None,
    max_predicate_atoms: int | None = None,
) -> ClosingArtifactGenerationResult: ...

def family_candidates(
    source: str,
    *,
    input_format: str = "auto",
    n_cores: int = 1,
    permutation: list[int] | None = None,
    nonpositive_indices: list[int] | None = None,
    exact_backend: str = "sparse",  # sparse, sparse-factorized, sparse-target-factorized, semi-numerical
    numerical_depth: int = 2,
    max_numerator_rank: int | None = None,
    finite_case_policy: str = "search",
    finite_max_visited_points: int | None = None,
    finite_max_retained_terminals: int | None = None,
    bundle_max_bytes: int | None = None,
    bundle_max_entries: int | None = None,
    bundle_max_coefficient_bytes: int | None = None,
    bundle_max_total_coefficient_bytes: int | None = None,
    checkpoint_dir: str | os.PathLike[str] | None = None,
    resume: bool = False,
    checkpoint_max_bytes: int | None = None,
) -> CandidateBundleResult: ...

def certify_candidates(
    bundle: bytes,
    *,
    max_domain_bound_endpoint_cells: int | None = None,
    max_predicate_consistency_work: int | None = None,
    max_predicate_atoms: int | None = None,
    max_negative_index_degree: int | None = None,
    max_total_excess_degree: int | None = None,
) -> ClosingArtifactGenerationResult: ...

def inspect_closing_artifact(
    artifact: bytes,
    *,
    max_domain_bound_endpoint_cells: int | None = None,
    max_predicate_consistency_work: int | None = None,
    max_predicate_atoms: int | None = None,
) -> ClosingArtifactInspectionResult: ...

def reduce_with_closing_artifact(
    artifact: bytes,
    target_powers: list[int],
    *,
    max_rule_applications: int = 1_000_000,
    max_domain_bound_endpoint_cells: int | None = None,
    max_predicate_consistency_work: int | None = None,
    max_predicate_atoms: int | None = None,
) -> ClosingArtifactReductionResult: ...
