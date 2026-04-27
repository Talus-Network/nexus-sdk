//! Standard TAP models shared by SDK, CLI, leader, and future Move surfaces.

use {
    crate::sui,
    serde::{de::Error as _, Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{fmt, path::PathBuf},
};

fn deserialize_tap_address_value<'de, D>(deserializer: D) -> Result<sui::types::Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return sui::types::Address::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    super::parse_address_value(&value)
        .map_err(D::Error::custom)?
        .ok_or_else(|| D::Error::custom("missing TAP address value"))
}

fn deserialize_tap_u64_value<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return u64::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    super::parse_u64_value(&value)
        .map_err(D::Error::custom)?
        .ok_or_else(|| D::Error::custom("missing TAP u64 value"))
}

/// On-chain generated agent identity handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct AgentId(pub sui::types::Address);

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_tap_address_value(deserializer).map(Self)
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// On-chain generated skill identity handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SkillId(pub sui::types::Address);

impl<'de> Deserialize<'de> for SkillId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_tap_address_value(deserializer).map(Self)
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TAP endpoint revision used for active lookup and in-flight pinning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct InterfaceRevision(pub u64);

impl<'de> Deserialize<'de> for InterfaceRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserialize_tap_u64_value(deserializer).map(Self)
    }
}

impl fmt::Display for InterfaceRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Key for an in-flight endpoint revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapEndpointKey {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
}

impl fmt::Display for TapEndpointKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.agent_id, self.skill_id, self.interface_revision
        )
    }
}

/// Key for fresh worksheet and active-revision lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapWorksheetKey {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
}

impl fmt::Display for TapWorksheetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.agent_id, self.skill_id)
    }
}

/// Shared object metadata required by a standard TAP endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapSharedObjectRef {
    pub id: sui::types::Address,
    pub initial_shared_version: u64,
    pub mutable: bool,
}

impl TapSharedObjectRef {
    pub fn immutable(id: sui::types::Address, initial_shared_version: u64) -> Self {
        Self {
            id,
            initial_shared_version,
            mutable: false,
        }
    }

    pub fn mutable(id: sui::types::Address, initial_shared_version: u64) -> Self {
        Self {
            id,
            initial_shared_version,
            mutable: true,
        }
    }
}

/// Payment source policy for an agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapPaymentMode {
    UserFunded,
    AgentFunded,
    Hybrid,
    Sponsored,
}

/// TAP-facing payment policy summary used by config digest and dry-run checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapPaymentPolicy {
    pub mode: TapPaymentMode,
    pub max_budget: u64,
    pub token_type_hash: Vec<u8>,
    pub auth_mode: u8,
    pub refund_mode: u8,
}

impl Default for TapPaymentPolicy {
    fn default() -> Self {
        Self {
            mode: TapPaymentMode::UserFunded,
            max_budget: 0,
            token_type_hash: Vec::new(),
            auth_mode: 0,
            refund_mode: 0,
        }
    }
}

/// TAP-facing schedule policy summary used by config digest and dry-run checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSchedulePolicy {
    #[serde(
        deserialize_with = "crate::types::deserialize_move_ascii_string",
        serialize_with = "crate::types::serialize_move_ascii_string"
    )]
    pub recurrence_kind: String,
    pub min_interval_ms: u64,
    pub max_occurrences: u64,
    pub allow_recursive: bool,
}

impl Default for TapSchedulePolicy {
    fn default() -> Self {
        Self {
            recurrence_kind: "once".to_string(),
            min_interval_ms: 0,
            max_occurrences: 1,
            allow_recursive: false,
        }
    }
}

/// Fixed on-chain tool entry that can receive vertex authorization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapAuthorizedTool {
    pub package_id: sui::types::Address,
    #[serde(
        alias = "module_name",
        deserialize_with = "crate::types::deserialize_move_ascii_string",
        serialize_with = "crate::types::serialize_move_ascii_string"
    )]
    pub module: String,
    #[serde(
        alias = "function_name",
        deserialize_with = "crate::types::deserialize_move_ascii_string",
        serialize_with = "crate::types::serialize_move_ascii_string"
    )]
    pub function: String,
    pub operation_hash: Vec<u8>,
}

/// Vertex authorization schema committed into endpoint config.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapVertexAuthorizationSchema {
    pub schema_hash: Vec<u8>,
    pub fixed_tools: Vec<TapAuthorizedTool>,
    pub requires_payment: bool,
}

/// User-facing skill requirements fetched before dry-run or execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillRequirements {
    pub input_schema_hash: Vec<u8>,
    pub workflow_hash: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub payment_policy: TapPaymentPolicy,
    pub schedule_policy: TapSchedulePolicy,
    pub vertex_authorization_schema: TapVertexAuthorizationSchema,
}

/// Stored `nexus_interface::tap::AgentRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapAgentRecord {
    pub agent_id: AgentId,
    pub owner: sui::types::Address,
    pub operator: sui::types::Address,
    pub metadata_hash: Vec<u8>,
    pub auth_mode: u8,
    pub active: bool,
}

/// Stored `nexus_interface::tap::SkillRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillRecord {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub dag_id: sui::types::Address,
    pub tap_package_id: sui::types::Address,
    pub workflow_hash: Vec<u8>,
    pub requirements_hash: Vec<u8>,
    pub metadata_hash: Vec<u8>,
    pub payment_policy: TapPaymentPolicy,
    pub schedule_policy: TapSchedulePolicy,
    pub capability_schema_hash: Vec<u8>,
    pub active: bool,
}

/// Stored `nexus_interface::tap::EndpointRevision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapEndpointRevision {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub package_id: sui::types::Address,
    pub endpoint_object_id: sui::types::Address,
    pub endpoint_object_version: u64,
    pub endpoint_object_digest: Vec<u8>,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
    pub config_digest: Vec<u8>,
    pub active_for_new_executions: bool,
}

impl TapEndpointRevision {
    pub fn key(&self) -> TapEndpointKey {
        TapEndpointKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }

    pub fn to_endpoint_record(&self) -> anyhow::Result<TapEndpointRecord> {
        let endpoint_object_digest = sui::types::Digest::from_bytes(
            self.endpoint_object_digest.as_slice(),
        )
        .map_err(|error| {
            anyhow::anyhow!(
                "invalid TAP endpoint object digest for endpoint {}: {error}",
                self.key()
            )
        })?;

        let record = TapEndpointRecord {
            key: self.key(),
            package_id: self.package_id,
            endpoint_object: sui::types::ObjectReference::new(
                self.endpoint_object_id,
                self.endpoint_object_version,
                endpoint_object_digest,
            ),
            shared_objects: self.shared_objects.clone(),
            config_digest: self.config_digest.clone(),
            requirements: self.requirements.clone(),
            active_for_new_executions: self.active_for_new_executions,
        };

        record.validate()?;
        Ok(record)
    }
}

/// Stored `nexus_interface::tap::EndpointActivation`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapEndpointActivation {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
}

/// Shared `nexus_interface::tap::TapRegistry` object contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapRegistry {
    pub id: sui::types::Address,
    pub agents: Vec<TapAgentRecord>,
    pub skills: Vec<TapSkillRecord>,
    pub endpoints: Vec<TapEndpointRevision>,
    pub active_endpoints: Vec<TapEndpointActivation>,
}

impl TapRegistry {
    /// Convert all endpoint revisions into leader-facing endpoint records,
    /// normalizing active state from the registry activation vector.
    pub fn endpoint_records(&self) -> anyhow::Result<Vec<TapEndpointRecord>> {
        self.endpoints
            .iter()
            .map(|endpoint| {
                let mut record = endpoint.to_endpoint_record()?;
                record.active_for_new_executions = self.is_active_endpoint(
                    endpoint.agent_id,
                    endpoint.skill_id,
                    endpoint.interface_revision,
                );
                Ok(record)
            })
            .collect()
    }

    pub fn endpoint_record(&self, key: TapEndpointKey) -> anyhow::Result<TapEndpointRecord> {
        let matches = self
            .endpoints
            .iter()
            .filter(|endpoint| endpoint.key() == key)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => anyhow::bail!("TAP endpoint revision not found for key {key}"),
            [endpoint] => {
                let mut record = endpoint.to_endpoint_record()?;
                record.active_for_new_executions =
                    self.is_active_endpoint(key.agent_id, key.skill_id, key.interface_revision);
                Ok(record)
            }
            _ => anyhow::bail!("duplicate TAP endpoint revisions found for key {key}"),
        }
    }

    pub fn active_endpoint_record(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> anyhow::Result<TapEndpointRecord> {
        let activations = self
            .active_endpoints
            .iter()
            .filter(|active| active.agent_id == agent_id && active.skill_id == skill_id)
            .collect::<Vec<_>>();

        let activation = match activations.as_slice() {
            [] => {
                return Err(TapEndpointResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            [activation] => *activation,
            _ => {
                return Err(TapEndpointResolutionError::DuplicateActiveRevision {
                    agent_id,
                    skill_id,
                    count: activations.len(),
                }
                .into())
            }
        };

        let key = TapEndpointKey {
            agent_id,
            skill_id,
            interface_revision: activation.interface_revision,
        };
        let record = self.endpoint_record(key)?;

        if !record.active_for_new_executions {
            anyhow::bail!("active TAP endpoint activation resolved inactive endpoint {key}");
        }

        Ok(record)
    }

    fn is_active_endpoint(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        interface_revision: InterfaceRevision,
    ) -> bool {
        self.active_endpoints.iter().any(|active| {
            active.agent_id == agent_id
                && active.skill_id == skill_id
                && active.interface_revision == interface_revision
        })
    }
}

/// Active or pinned endpoint record returned to leader and SDK callers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapEndpointRecord {
    pub key: TapEndpointKey,
    pub package_id: sui::types::Address,
    pub endpoint_object: sui::types::ObjectReference,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub config_digest: Vec<u8>,
    pub requirements: TapSkillRequirements,
    pub active_for_new_executions: bool,
}

impl TapEndpointRecord {
    pub fn worksheet_key(&self) -> TapWorksheetKey {
        TapWorksheetKey {
            agent_id: self.key.agent_id,
            skill_id: self.key.skill_id,
        }
    }

    pub fn validate(&self) -> Result<(), TapValidationError> {
        if self.config_digest.is_empty() {
            return Err(TapValidationError::MissingConfigDigest);
        }

        validate_requirements(&self.requirements)?;

        Ok(())
    }
}

/// Legacy standalone active endpoint pointer model. New standard TAP recovery
/// should read `TapRegistry.active_endpoints` instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapActiveEndpoint {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub active_revision: InterfaceRevision,
    pub endpoint_object_id: sui::types::Address,
}

/// Execution-linked payment object model used by leader recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapExecutionPayment {
    pub payment_id: sui::types::Address,
    pub execution_id: sui::types::Address,
    pub key: TapEndpointKey,
    pub payer: sui::types::Address,
    pub max_budget: u64,
    pub consumed: u64,
    pub auth_mode: u8,
}

/// Execution-bound authorization grant model used by fetch/event surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapVertexAuthorizationGrant {
    pub grant_id: sui::types::Address,
    pub grantor: sui::types::Address,
    pub target_object_id: sui::types::Address,
    pub key: TapEndpointKey,
    pub walk_execution_id: sui::types::Address,
    pub vertex_execution_id: sui::types::Address,
    pub leader_assignment_id: sui::types::Address,
    pub allowed_tool: TapAuthorizedTool,
    pub constraints_hash: Vec<u8>,
    pub expires_at_ms: u64,
    pub max_uses: u64,
    pub used: u64,
    pub revoked: bool,
    pub payment_required: bool,
}

/// Scheduled peer of immediate skill execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledSkillTask {
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub pinned_revision: Option<InterfaceRevision>,
    pub input_commitment: Vec<u8>,
    pub long_term_gas_coin_id: sui::types::Address,
    pub refill_policy_hash: Vec<u8>,
    pub authorization_plan_hash: Option<Vec<u8>>,
    pub schedule_policy: TapSchedulePolicy,
    pub schedule_entries_hash: Vec<u8>,
    pub next_after_ms: u64,
    pub occurrences_spawned: u64,
    pub active: bool,
}

/// Registered skill plus the currently active endpoint used for fresh standard execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapActiveSkillExecutionTarget {
    pub skill: TapSkillRecord,
    pub endpoint: TapEndpointRecord,
}

/// Digest input committed by endpoint announcements and publish artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapConfigDigestInput {
    pub package_id: sui::types::Address,
    pub endpoint_object_id: Option<sui::types::Address>,
    pub interface_revision: InterfaceRevision,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
}

impl TapConfigDigestInput {
    pub fn digest(&self) -> anyhow::Result<Vec<u8>> {
        let bytes = bcs::to_bytes(self)?;
        Ok(Sha256::digest(bytes).to_vec())
    }

    pub fn digest_hex(&self) -> anyhow::Result<String> {
        Ok(hex::encode(self.digest()?))
    }
}

/// DAG-backed TAP skill config used by SDK/CLI authoring helpers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillConfig {
    pub name: String,
    pub tap_package_name: String,
    pub dag_path: PathBuf,
    pub tap_package_path: PathBuf,
    pub requirements: TapSkillRequirements,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub interface_revision: InterfaceRevision,
    pub active_for_new_executions: bool,
}

impl TapSkillConfig {
    pub fn digest_input(&self, package_id: sui::types::Address) -> TapConfigDigestInput {
        TapConfigDigestInput {
            package_id,
            endpoint_object_id: None,
            interface_revision: self.interface_revision,
            shared_objects: self.shared_objects.clone(),
            requirements: self.requirements.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), TapValidationError> {
        if self.name.trim().is_empty() {
            return Err(TapValidationError::MissingSkillName);
        }
        if self.tap_package_name.trim().is_empty() {
            return Err(TapValidationError::MissingTapPackageName);
        }
        if self.dag_path.as_os_str().is_empty() {
            return Err(TapValidationError::MissingDagPath);
        }
        if self.tap_package_path.as_os_str().is_empty() {
            return Err(TapValidationError::MissingTapPackagePath);
        }

        validate_requirements(&self.requirements)
    }
}

/// Author-to-operator artifact produced after TAP plus DAG publishing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapPublishArtifact {
    pub skill_name: String,
    pub dag_id: sui::types::Address,
    pub tap_package_id: sui::types::Address,
    pub interface_revision: InterfaceRevision,
    pub config_digest: Vec<u8>,
    pub config_digest_hex: String,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
}

impl TapPublishArtifact {
    pub fn from_config(
        config: &TapSkillConfig,
        dag_id: sui::types::Address,
        tap_package_id: sui::types::Address,
    ) -> anyhow::Result<Self> {
        config.validate()?;
        let digest_input = config.digest_input(tap_package_id);
        let config_digest = digest_input.digest()?;
        let config_digest_hex = hex::encode(&config_digest);

        Ok(Self {
            skill_name: config.name.clone(),
            dag_id,
            tap_package_id,
            interface_revision: config.interface_revision,
            config_digest,
            config_digest_hex,
            shared_objects: config.shared_objects.clone(),
            requirements: config.requirements.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapValidationError {
    MissingSkillName,
    MissingTapPackageName,
    MissingDagPath,
    MissingTapPackagePath,
    MissingWorkflowHash,
    MissingRequirementsHash,
    MissingConfigDigest,
    EmptyAuthorizedToolModule,
    EmptyAuthorizedToolFunction,
}

impl fmt::Display for TapValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapValidationError::MissingSkillName => write!(f, "skill name is required"),
            TapValidationError::MissingTapPackageName => write!(f, "TAP package name is required"),
            TapValidationError::MissingDagPath => write!(f, "DAG path is required"),
            TapValidationError::MissingTapPackagePath => write!(f, "TAP package path is required"),
            TapValidationError::MissingWorkflowHash => write!(f, "workflow hash is required"),
            TapValidationError::MissingRequirementsHash => {
                write!(f, "requirements hash is required")
            }
            TapValidationError::MissingConfigDigest => {
                write!(f, "endpoint config digest is required")
            }
            TapValidationError::EmptyAuthorizedToolModule => {
                write!(f, "authorized tool module is required")
            }
            TapValidationError::EmptyAuthorizedToolFunction => {
                write!(f, "authorized tool function is required")
            }
        }
    }
}

impl std::error::Error for TapValidationError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapEndpointResolutionError {
    MissingActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
    },
    DuplicateActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
        count: usize,
    },
    InvalidEndpoint(TapValidationError),
}

impl fmt::Display for TapEndpointResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapEndpointResolutionError::MissingActiveRevision { agent_id, skill_id } => {
                write!(f, "no active TAP endpoint for agent_id={agent_id}, skill_id={skill_id}")
            }
            TapEndpointResolutionError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count,
            } => write!(
                f,
                "expected one active TAP endpoint for agent_id={agent_id}, skill_id={skill_id}, found {count}"
            ),
            TapEndpointResolutionError::InvalidEndpoint(error) => {
                write!(f, "invalid TAP endpoint: {error}")
            }
        }
    }
}

impl std::error::Error for TapEndpointResolutionError {}

pub fn validate_requirements(
    requirements: &TapSkillRequirements,
) -> Result<(), TapValidationError> {
    if requirements.workflow_hash.is_empty() {
        return Err(TapValidationError::MissingWorkflowHash);
    }
    if requirements.input_schema_hash.is_empty() {
        return Err(TapValidationError::MissingRequirementsHash);
    }
    for tool in &requirements.vertex_authorization_schema.fixed_tools {
        if tool.module.trim().is_empty() {
            return Err(TapValidationError::EmptyAuthorizedToolModule);
        }
        if tool.function.trim().is_empty() {
            return Err(TapValidationError::EmptyAuthorizedToolFunction);
        }
    }

    Ok(())
}

/// Resolve exactly one active endpoint for fresh execution.
pub fn resolve_active_tap_endpoint(
    records: &[TapEndpointRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&TapEndpointRecord, TapEndpointResolutionError> {
    let active = records
        .iter()
        .filter(|record| {
            record.key.agent_id == agent_id
                && record.key.skill_id == skill_id
                && record.active_for_new_executions
        })
        .collect::<Vec<_>>();

    match active.as_slice() {
        [] => Err(TapEndpointResolutionError::MissingActiveRevision { agent_id, skill_id }),
        [record] => {
            record
                .validate()
                .map_err(TapEndpointResolutionError::InvalidEndpoint)?;
            Ok(record)
        }
        _ => Err(TapEndpointResolutionError::DuplicateActiveRevision {
            agent_id,
            skill_id,
            count: active.len(),
        }),
    }
}

/// Resolve the unique active skill and endpoint for fresh standard execution.
pub fn resolve_active_tap_skill_execution_target(
    registry: &TapRegistry,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<TapActiveSkillExecutionTarget> {
    let skill_matches = registry
        .skills
        .iter()
        .filter(|skill| skill.agent_id == agent_id && skill.skill_id == skill_id && skill.active)
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => anyhow::bail!(
            "active TAP skill not found for agent {} skill {}",
            agent_id,
            skill_id
        ),
        [skill] => (*skill).clone(),
        _ => anyhow::bail!(
            "duplicate active TAP skills found for agent {} skill {}",
            agent_id,
            skill_id
        ),
    };

    let endpoint = registry.active_endpoint_record(agent_id, skill_id)?;

    Ok(TapActiveSkillExecutionTarget { skill, endpoint })
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    fn addr(value: &str) -> sui::types::Address {
        sui::types::Address::from_str(value).expect("valid address")
    }

    fn requirements() -> TapSkillRequirements {
        TapSkillRequirements {
            input_schema_hash: vec![1],
            workflow_hash: vec![2],
            metadata_hash: vec![3],
            payment_policy: TapPaymentPolicy {
                max_budget: 100,
                ..TapPaymentPolicy::default()
            },
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        }
    }

    fn endpoint(revision: u64, active: bool) -> TapEndpointRecord {
        let object_ref =
            sui::types::ObjectReference::new(addr("0x123"), 7, sui::types::Digest::from([1; 32]));
        TapEndpointRecord {
            key: TapEndpointKey {
                agent_id: AgentId(addr("0xa")),
                skill_id: SkillId(addr("0xb")),
                interface_revision: InterfaceRevision(revision),
            },
            package_id: addr("0xc"),
            endpoint_object: object_ref,
            shared_objects: vec![TapSharedObjectRef::immutable(addr("0xd"), 9)],
            config_digest: vec![9],
            requirements: requirements(),
            active_for_new_executions: active,
        }
    }

    fn skill(active: bool) -> TapSkillRecord {
        TapSkillRecord {
            agent_id: AgentId(addr("0xa")),
            skill_id: SkillId(addr("0xb")),
            dag_id: addr("0x44"),
            tap_package_id: addr("0xc"),
            workflow_hash: vec![2],
            requirements_hash: vec![1],
            metadata_hash: vec![3],
            payment_policy: TapPaymentPolicy {
                max_budget: 100,
                ..TapPaymentPolicy::default()
            },
            schedule_policy: TapSchedulePolicy::default(),
            capability_schema_hash: vec![7],
            active,
        }
    }

    fn registry_with_active_skill() -> TapRegistry {
        TapRegistry {
            id: addr("0xf"),
            agents: Vec::new(),
            skills: vec![skill(true)],
            endpoints: vec![TapEndpointRevision {
                agent_id: AgentId(addr("0xa")),
                skill_id: SkillId(addr("0xb")),
                interface_revision: InterfaceRevision(2),
                package_id: addr("0xc"),
                endpoint_object_id: addr("0x123"),
                endpoint_object_version: 7,
                endpoint_object_digest: vec![1; 32],
                shared_objects: vec![TapSharedObjectRef::immutable(addr("0xd"), 9)],
                requirements: requirements(),
                config_digest: vec![9],
                active_for_new_executions: true,
            }],
            active_endpoints: vec![TapEndpointActivation {
                agent_id: AgentId(addr("0xa")),
                skill_id: SkillId(addr("0xb")),
                interface_revision: InterfaceRevision(2),
            }],
        }
    }

    #[test]
    fn config_digest_is_deterministic() {
        let input = TapConfigDigestInput {
            package_id: addr("0x1"),
            endpoint_object_id: Some(addr("0x2")),
            interface_revision: InterfaceRevision(3),
            shared_objects: vec![TapSharedObjectRef::mutable(addr("0x4"), 5)],
            requirements: requirements(),
        };

        assert_eq!(input.digest().unwrap(), input.digest().unwrap());
        assert_eq!(input.digest_hex().unwrap().len(), 64);
    }

    #[test]
    fn validate_rejects_missing_requirements_hash() {
        let mut requirements = requirements();
        requirements.input_schema_hash.clear();

        assert_eq!(
            validate_requirements(&requirements),
            Err(TapValidationError::MissingRequirementsHash)
        );
    }

    #[test]
    fn active_resolution_requires_exactly_one_active_revision() {
        let active = endpoint(1, true);
        let inactive = endpoint(2, false);
        let records = vec![active.clone(), inactive];

        let resolved =
            resolve_active_tap_endpoint(&records, active.key.agent_id, active.key.skill_id)
                .expect("one active endpoint");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

        let duplicate = vec![endpoint(1, true), endpoint(2, true)];
        assert!(matches!(
            resolve_active_tap_endpoint(&duplicate, AgentId(addr("0xa")), SkillId(addr("0xb"))),
            Err(TapEndpointResolutionError::DuplicateActiveRevision { count: 2, .. })
        ));
    }

    #[test]
    fn publish_artifact_contains_digest_and_onchain_package_ids() {
        let config = TapSkillConfig {
            name: "weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: requirements(),
            shared_objects: vec![TapSharedObjectRef::immutable(addr("0x9"), 1)],
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };

        let artifact = TapPublishArtifact::from_config(&config, addr("0x8"), addr("0x7"))
            .expect("valid artifact");

        assert_eq!(artifact.dag_id, addr("0x8"));
        assert_eq!(artifact.tap_package_id, addr("0x7"));
        assert_eq!(artifact.config_digest_hex.len(), 64);
    }

    #[test]
    fn active_skill_execution_target_requires_one_active_skill_and_endpoint() {
        let registry = registry_with_active_skill();
        let resolved = resolve_active_tap_skill_execution_target(
            &registry,
            AgentId(addr("0xa")),
            SkillId(addr("0xb")),
        )
        .expect("active skill target");

        assert_eq!(resolved.skill.dag_id, addr("0x44"));
        assert_eq!(
            resolved.endpoint.key.interface_revision,
            InterfaceRevision(2)
        );
    }

    #[test]
    fn active_skill_execution_target_rejects_missing_skill() {
        let mut registry = registry_with_active_skill();
        registry.skills.clear();

        assert!(resolve_active_tap_skill_execution_target(
            &registry,
            AgentId(addr("0xa")),
            SkillId(addr("0xb"))
        )
        .is_err());
    }
}
