//! The campaign spec subset used by the supervisor now (`rustred.campaign.spec.v1`, plan 3.3.1; interfaces/supervisor.md
//! section 13). Unknown fields are refused: a misspelt limit must not silently disable it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const CAMPAIGN_SPEC_SCHEMA: &str = "rustred.campaign.spec.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignSpec {
    pub schema: String,
    pub campaign: String,
    #[serde(default = "d_kind")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dirs: Dirs,
    #[serde(default)]
    pub programs: BTreeMap<String, Program>,
    #[serde(default)]
    pub licence: Licence,
    #[serde(default)]
    pub resources: Resources,
    #[serde(default)]
    pub progress: ProgressCfg,
    #[serde(default)]
    pub synthetic: Option<Synthetic>,
    /// Existing material the campaign starts from (tables, hand rules, evidence), by path + digest (R41 P-1): the
    /// pre-flight and every start verify each one and refuse otherwise; they are registered as ledger inputs.
    #[serde(default)]
    pub inputs: Vec<SpecInput>,
    /// `campaign` (default) or `optimise` (user ruling U-13: boot from an existing campaign result and write the new
    /// result beside it; the work source decides the work, the supervisor passes the mode through).
    #[serde(default = "d_mode")]
    pub mode: String,
    /// The campaign directory an `optimise` run boots from (read-only for this run: never written).
    #[serde(default)]
    pub source_campaign_dir: Option<PathBuf>,
    /// Default terminal bound per node (record 1 `campaign_created.terminal_bound`; a node's own registration may
    /// state another). Spec data, never a constant of the code.
    #[serde(default = "d_terminal_bound")]
    pub terminal_bound: u32,
}

fn d_terminal_bound() -> u32 {
    100
}

fn d_mode() -> String {
    "campaign".into()
}

fn d_kind() -> String {
    "test".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dirs {
    #[serde(default)]
    pub campaign_dir: Option<PathBuf>,
    /// Non-snapshotted root for run directories (default: the campaign directory).
    #[serde(default)]
    pub work_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub path: PathBuf,
    /// Optional pinned digest (BLAKE3 hex); pre-flight refuses a mismatch.
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub optional: bool,
    /// Lane closeloop (closeloop.md 6.3): launch as `<path> <job_verb> --job RUN/job.json` (an engine verb's job
    /// mode) instead of `<path> job RUN/job.json`. A launch difference only.
    #[serde(default)]
    pub job_verb: Option<String>,
}

/// One input of the spec (generic: the work source gives the role its meaning).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecInput {
    /// Ledger input id (`[A-Za-z0-9._:-]`).
    pub id: String,
    /// e.g. `table`, `family`, `evidence`, `box`.
    pub role: String,
    pub path: PathBuf,
    /// BLAKE3 hex of the file.
    pub blake3: String,
}

impl SpecInput {
    /// Ok((bytes)) when the file exists with the stated digest.
    pub fn check(&self) -> Result<u64, String> {
        match crate::atomic::blake3_file(&self.path) {
            Ok((bytes, d)) if d == self.blake3 => Ok(bytes),
            Ok((_, d)) => Err(format!(
                "input {} ({}): blake3 {d} differs from the spec's {}",
                self.id,
                self.path.display(),
                self.blake3
            )),
            Err(e) => Err(format!(
                "input {} missing: {} ({e})",
                self.id,
                self.path.display()
            )),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Licence {
    /// NAME of the licence variable passed through to children (its value is never read into logs).
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    /// "plain" (CPU list or count) or "site" (site-policy JSON file, process-layer.md section 7).
    #[serde(default = "d_policy")]
    pub policy: String,
    /// lane closeloop4 (closeloop.md 20): the DEFAULT CPU list (or `count:N`) of a session under policy plain. A
    /// LAUNCH-TIME value, NOT part of the campaign's identity ([`identity_digest`]): `rr_campaign run --cpus LIST`
    /// overrides it per session, and without both the session takes the process's affinity mask (the launcher's
    /// pick, e.g. `taskset`), capped by `max_cores`; the list used is recorded in the ledger per session.
    #[serde(default)]
    pub cpus: Option<String>,
    /// lane closeloop4 (closeloop.md 20): CPUs a session never uses (part of the campaign's CPU BUDGET = policy +
    /// `max_cores` + `exclude_cpus`, which IS identity).
    #[serde(default)]
    pub exclude_cpus: Option<String>,
    #[serde(default)]
    pub site_policy: Option<PathBuf>,
    /// Cores the campaign may hold (default: what the policy allows).
    #[serde(default)]
    pub max_cores: Option<usize>,
    #[serde(default = "d_max_jobs")]
    pub max_jobs: usize,
    /// Sum of the reserved caps of running jobs (bytes or "30G").
    #[serde(default = "d_mem_total")]
    pub mem_total: ByteSize,
    #[serde(default = "d_floor")]
    pub host_floor: ByteSize,
    #[serde(default = "d_disk_floor")]
    pub disk_floor: ByteSize,
    /// Grace given to running jobs on the first stop request (plus 5 s before the group kill).
    #[serde(default = "d_grace")]
    pub grace_seconds: u64,
    /// Existence of any = no launch (agents' test campaigns: WS/ops/PAUSE and the implementer's; the user's run: none).
    #[serde(default)]
    pub pause_files: Vec<PathBuf>,
    #[serde(default = "d_sampler_ms")]
    pub sampler_ms: u64,
    /// Heartbeat timeout (default 3 x heartbeat_s + 5 s, contract 6.2).
    #[serde(default)]
    pub heartbeat_timeout_s: Option<u64>,
    /// No `job_started` event this long after spawn = unresponsive (contract 6.2: 60 s).
    #[serde(default = "d_start_timeout")]
    pub start_timeout_s: u64,
    /// Allow running on a kernel without pidfd (signals then use the documented weaker path). Default false: refused.
    #[serde(default)]
    pub allow_no_pidfd: bool,
    /// procfix5 (gate4 letter (a)): how descendants that DROP the marker are contained. `"scope"` (default): REAL
    /// children run only when the systemd scope mechanism is available (each job in its recorded scope, stopped by name
    /// and leader after a crash); without it the run is refused. `"marker_only"`: explicitly accept the marker as the
    /// only containment (a marker-dropping escapee survives a crash). Fake children (`rr_fake_child`, synthetic specs,
    /// `--fake-child`) are never refused.
    #[serde(default)]
    pub containment: Option<String>,
    /// lane t266run (rule L17): the PER-JOB memory cap of this session (bytes or "8G"), a LAUNCH-TIME resource like the
    /// CPU list: when set, every job of the session is launched and reserved with this cap instead of the work
    /// source's (`rr_campaign run --mem-cap`); recorded per session (ledger note `session_resources`) and per attempt
    /// (`job_queued.reserved_bytes`). A child killed at its cap is a resource end (retry, then attention
    /// `memory_cap_exceeded`), never evidence; a session with a higher cap reopens those items.
    #[serde(default)]
    pub job_mem_cap: Option<ByteSize>,
    /// lane t266run: this session's closure-check threads (`rr_campaign run --check-threads N`) and evidence-store
    /// build threads (`--store-threads N`) of every closeloop node: launch-time resources (results never depend on
    /// them), recorded per session (note `session_resources`); absent = the closeloop config's.
    #[serde(default)]
    pub check_threads: Option<usize>,
    #[serde(default)]
    pub store_threads: Option<usize>,
}

fn d_policy() -> String {
    "plain".into()
}
fn d_max_jobs() -> usize {
    8
}
fn d_mem_total() -> ByteSize {
    ByteSize(30_000_000_000)
}
fn d_floor() -> ByteSize {
    ByteSize(crate::admission::DEFAULT_FLOOR_BYTES)
}
fn d_disk_floor() -> ByteSize {
    ByteSize(20_000_000_000)
}
fn d_grace() -> u64 {
    120
}
fn d_sampler_ms() -> u64 {
    1000
}
fn d_start_timeout() -> u64 {
    60
}

impl Default for Resources {
    fn default() -> Self {
        serde_json::from_str("{}").expect("defaults")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressCfg {
    #[serde(default = "d_cadence")]
    pub cadence_seconds: u64,
}

fn d_cadence() -> u64 {
    2
}

impl Default for ProgressCfg {
    fn default() -> Self {
        ProgressCfg {
            cadence_seconds: d_cadence(),
        }
    }
}

/// The synthetic campaign of the `ScriptedWorkSource` (interfaces/supervisor.md section 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Synthetic {
    pub items: u32,
    #[serde(default = "d_nodes")]
    pub nodes: u32,
    #[serde(default = "d_rungs")]
    pub rungs: u8,
    #[serde(default)]
    pub seed: u64,
    /// Range of the case duration (ms) of a fake job.
    #[serde(default = "d_case_ms")]
    pub case_ms: [u64; 2],
    #[serde(default = "d_p_norule")]
    pub p_norule: f64,
    #[serde(default = "d_p_mem")]
    pub p_mem: f64,
    #[serde(default = "d_p_crash")]
    pub p_crash: f64,
    /// Probability that an item depends on one or two earlier items.
    #[serde(default = "d_deps")]
    pub deps: f64,
    /// Memory cap of a fake job (MiB); a mem_excess job allocates twice this much.
    #[serde(default = "d_mem_cap_mb")]
    pub mem_cap_mb: u64,
    #[serde(default = "d_hb")]
    pub heartbeat_s: u32,
    #[serde(default = "d_child_grace")]
    pub stop_grace_s: u32,
    /// Reaction of every fake child to SIGTERM (tests: `ignore` = only the group kill ends it).
    #[serde(default)]
    pub on_sigterm: rustred_campaign_contract::fake::OnSigterm,
    /// Every rule job forks a helper that ESCAPES its group (setsid, no PDEATHSIG): a contract violation that the
    /// marker sweep after the job end must clean up.
    #[serde(default)]
    pub escape_helpers: bool,
    /// Items whose rung-0 job stops its heartbeat and hangs (the heartbeat timeout must kill it).
    #[serde(default)]
    pub hang_items: Vec<u32>,
}

fn d_nodes() -> u32 {
    3
}
fn d_rungs() -> u8 {
    3
}
fn d_case_ms() -> [u64; 2] {
    [100, 400]
}
fn d_p_norule() -> f64 {
    0.25
}
fn d_p_mem() -> f64 {
    0.1
}
fn d_p_crash() -> f64 {
    0.1
}
fn d_deps() -> f64 {
    0.3
}
fn d_mem_cap_mb() -> u64 {
    48
}
fn d_hb() -> u32 {
    1
}
fn d_child_grace() -> u32 {
    5
}

/// Bytes, from a number or a string "30G" / "512M" / "1.5T" (decimal units, as the plan writes them).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(pub u64);

impl ByteSize {
    pub fn parse(s: &str) -> Result<ByteSize, String> {
        let s = s.trim();
        let (num, mult) = match s.chars().last() {
            Some('K' | 'k') => (&s[..s.len() - 1], 1e3),
            Some('M' | 'm') => (&s[..s.len() - 1], 1e6),
            Some('G' | 'g') => (&s[..s.len() - 1], 1e9),
            Some('T' | 't') => (&s[..s.len() - 1], 1e12),
            _ => (s, 1.0),
        };
        let v: f64 = num
            .trim()
            .parse()
            .map_err(|_| format!("bad byte size {s:?}"))?;
        if !(v.is_finite() && v >= 0.0) {
            return Err(format!("bad byte size {s:?}"));
        }
        Ok(ByteSize((v * mult).round() as u64))
    }
}

impl Serialize for ByteSize {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::Number(n) => n.as_u64().map(ByteSize).ok_or_else(|| {
                serde::de::Error::custom("byte size must be a non-negative integer")
            }),
            serde_json::Value::String(s) => ByteSize::parse(&s).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom("byte size: number or string")),
        }
    }
}

/// lane closeloop4 (closeloop.md 20): the spec fields that are LAUNCH-TIME resources, outside the campaign identity.
pub const LAUNCH_TIME_FIELDS: [(&str, &str); 1] = [("resources", "cpus")];
const IDENTITY_DOMAIN: &[u8] = b"rustred.campaign.spec-identity.v1\n";

/// lane closeloop4 (closeloop.md 20): the campaign IDENTITY digest of a spec text: BLAKE3 over a domain tag and the
/// canonical JSON (sorted keys, no insignificant whitespace) of the spec WITHOUT its launch-time fields
/// ([`LAUNCH_TIME_FIELDS`]: the concrete CPU list). Two specs that differ only there are the same campaign: a stopped
/// run resumes on other CPUs. Everything else (programs, inputs, budgets incl. the CPU budget) stays identity.
pub fn identity_digest(bytes: &[u8]) -> Result<String, String> {
    let mut v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| format!("spec: {e}"))?;
    for (obj, field) in LAUNCH_TIME_FIELDS {
        if let Some(o) = v.get_mut(obj).and_then(serde_json::Value::as_object_mut) {
            o.remove(field);
        }
    }
    let raw = serde_json::to_vec(&v).map_err(|e| e.to_string())?;
    let canon =
        rustred_campaign_contract::canon::canonical_line(&raw).map_err(|e| e.to_string())?;
    let mut h = blake3::Hasher::new();
    h.update(IDENTITY_DOMAIN);
    h.update(&canon);
    Ok(h.finalize().to_hex().to_string())
}

impl CampaignSpec {
    /// Parses and validates; returns the spec and its campaign IDENTITY digest ([`identity_digest`]; lane closeloop4:
    /// no longer the BLAKE3 of the bytes, which [`CampaignSpec::read_digests`] also returns for older ledgers).
    pub fn read(path: &Path) -> Result<(CampaignSpec, String), String> {
        let (s, id, _) = Self::read_digests(path)?;
        Ok((s, id))
    }

    /// (spec, identity digest, BLAKE3 of the bytes = the identity a ledger created before lane closeloop4 recorded).
    pub fn read_digests(path: &Path) -> Result<(CampaignSpec, String, String), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let spec: CampaignSpec =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        spec.validate()?;
        Ok((
            spec,
            identity_digest(&bytes)?,
            blake3::hash(&bytes).to_hex().to_string(),
        ))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CAMPAIGN_SPEC_SCHEMA {
            return Err(format!(
                "schema {:?}, expected {CAMPAIGN_SPEC_SCHEMA}",
                self.schema
            ));
        }
        if self.campaign.is_empty()
            || self.campaign.len() > 96
            || !self
                .campaign
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(format!(
                "campaign id {:?}: 1-96 of [A-Za-z0-9._-]",
                self.campaign
            ));
        }
        match self.resources.policy.as_str() {
            // lane closeloop4: the CPU list is launch-time (`--cpus`, else `resources.cpus`, else the affinity mask)
            "plain" => {
                if let Some(c) = &self.resources.cpus {
                    crate::cpu_policy::CpuSpec::parse(c)
                        .map_err(|e| format!("resources.cpus: {e}"))?;
                }
                if let Some(x) = &self.resources.exclude_cpus {
                    crate::resources::parse_cpu_list(x)
                        .map_err(|e| format!("resources.exclude_cpus: {e}"))?;
                }
            }
            "site" => {
                if self.resources.site_policy.is_none() {
                    return Err("resources.site_policy is required with policy site".into());
                }
            }
            p => return Err(format!("resources.policy {p:?}: plain or site")),
        }
        if self.resources.max_jobs == 0 {
            return Err("resources.max_jobs is 0".into());
        }
        if let Some(s) = &self.synthetic {
            if s.items == 0 || s.nodes == 0 || s.rungs == 0 || s.rungs > 9 {
                return Err("synthetic: items, nodes > 0 and 1 <= rungs <= 9".into());
            }
            if s.case_ms[0] > s.case_ms[1] {
                return Err("synthetic.case_ms: [min, max]".into());
            }
            for p in [s.p_norule, s.p_mem, s.p_crash, s.deps] {
                if !(0.0..=1.0).contains(&p) {
                    return Err("synthetic probabilities must lie in [0, 1]".into());
                }
            }
            if s.p_norule + s.p_mem + s.p_crash > 1.0 {
                return Err("synthetic: p_norule + p_mem + p_crash > 1".into());
            }
            if !self.programs.contains_key("rr_fake_child") {
                return Err("a synthetic campaign needs programs.rr_fake_child".into());
            }
        } else if !self
            .inputs
            .iter()
            .any(|i| i.role == "closeloop" || i.role == "shipecon")
        {
            // lane closeloop (closeloop.md 6.2): a close-from-material campaign names its config as an input; lane
            // shipecon (shipecon.md 13): a ship-economy campaign names its node file (REQUEST TO I-INT: keep)
            return Err(
                "only synthetic, close-from-material (an input of role closeloop) and ship-economy (an input of role \
                 shipecon) campaigns are supported by this version"
                    .into(),
            );
        }
        for i in &self.inputs {
            if i.id.is_empty()
                || i.id.starts_with("bin:")
                || !i
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
            {
                return Err(format!("input id {:?}: [A-Za-z0-9._:-], not bin:*", i.id));
            }
            if !rustred_campaign_contract::is_blake3_hex(&i.blake3) {
                return Err(format!("input {}: blake3 must be 64 lower-case hex", i.id));
            }
        }
        match self.mode.as_str() {
            "campaign" => {}
            "optimise" => {
                if self.source_campaign_dir.is_none() {
                    return Err("mode optimise needs source_campaign_dir".into());
                }
            }
            m => return Err(format!("mode {m:?}: campaign or optimise")),
        }
        if let Some(e) = &self.licence.env
            && (e.is_empty() || e.contains('='))
        {
            return Err("licence.env must be a variable name".into());
        }
        Ok(())
    }

    /// Refuses a campaign directory that is (inside) the source campaign of an optimise run (U-13).
    pub fn check_dirs(&self, cdir: &Path) -> Result<(), String> {
        if let Some(src) = &self.source_campaign_dir {
            let c = std::fs::canonicalize(cdir).unwrap_or_else(|_| cdir.to_path_buf());
            let s = std::fs::canonicalize(src).unwrap_or_else(|_| src.clone());
            if c.starts_with(&s) || s.starts_with(&c) {
                return Err(format!(
                    "campaign directory {} overlaps the source campaign {} of this optimise run: the source is never written",
                    c.display(),
                    s.display()
                ));
            }
        }
        Ok(())
    }

    /// The heartbeat timeout for a job with heartbeat `hb` seconds.
    pub fn heartbeat_timeout(&self, hb: u32) -> std::time::Duration {
        std::time::Duration::from_secs(
            self.resources
                .heartbeat_timeout_s
                .unwrap_or(3 * hb as u64 + 5),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_sizes_and_defaults() {
        assert_eq!(ByteSize::parse("30G").unwrap().0, 30_000_000_000);
        assert_eq!(ByteSize::parse("512M").unwrap().0, 512_000_000);
        assert_eq!(ByteSize::parse("1.5T").unwrap().0, 1_500_000_000_000);
        assert_eq!(ByteSize::parse("17").unwrap().0, 17);
        assert!(ByteSize::parse("x").is_err());
        let r = Resources::default();
        assert_eq!(r.host_floor.0, 50_000_000_000);
        assert_eq!(r.grace_seconds, 120);
        let s: CampaignSpec = serde_json::from_str(
            r#"{"schema":"rustred.campaign.spec.v1","campaign":"t","programs":{"rr_fake_child":{"path":"/x"}},
                "resources":{"cpus":"0-3","mem_total":"4G"},"synthetic":{"items":3}}"#,
        )
        .unwrap();
        s.validate().unwrap();
        assert_eq!(s.resources.mem_total.0, 4_000_000_000);
        assert!(
            serde_json::from_str::<CampaignSpec>(r#"{"schema":"x","campaign":"t","bogus":1}"#)
                .is_err()
        );
    }

    #[test]
    fn identity_digest_ignores_the_cpu_list_only() {
        let a = br#"{"schema":"rustred.campaign.spec.v1","campaign":"t","resources":{"cpus":"0-3","max_cores":4}}"#;
        let b = br#"{"campaign":"t","resources":{"max_cores":4,"cpus":"8,9,10,11"},"schema":"rustred.campaign.spec.v1"}"#;
        let c = br#"{"schema":"rustred.campaign.spec.v1","campaign":"t","resources":{"cpus":"0-3","max_cores":5}}"#;
        let d =
            br#"{"schema":"rustred.campaign.spec.v1","campaign":"t","resources":{"max_cores":4}}"#;
        let (ia, ib, ic, id) = (
            identity_digest(a).unwrap(),
            identity_digest(b).unwrap(),
            identity_digest(c).unwrap(),
            identity_digest(d).unwrap(),
        );
        assert_eq!(ia, ib, "another CPU list and key order: the same campaign");
        assert_eq!(ia, id, "no CPU list at all: the same campaign");
        assert_ne!(ia, ic, "the CPU budget is identity");
        assert_ne!(ia, blake3::hash(a).to_hex().to_string());
    }
}
