//! Standard TAP models shared by SDK, CLI, leader, and future Move surfaces.

use {
    super::{
        serde_parsers::{
            deserialize_tap_address_value,
            deserialize_tap_byte_vector,
            deserialize_tap_u64_value,
        },
        MoveOption,
        MoveString,
        MoveTable,
    },
    crate::sui,
    serde::{de::Error as _, Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{fmt, path::PathBuf},
};

const TAP_PAYMENT_SOURCE_KIND_INVOKER: u8 = 0;
const TAP_PAYMENT_SOURCE_KIND_AGENT_VAULT: u8 = 1;

/// On-chain generated standard Talus agent ID.
pub type AgentId = sui::types::Address;

/// Agent-local standard TAP skill identity index.
pub type SkillId = u64;

pub const fn skill_id(value: u64) -> SkillId {
    value
}

/// TAP skill interface version used for fresh lookup and in-flight pinning.
pub type InterfaceVersion = crate::types::generated::interface_types::version::InterfaceVersion;
/// Generated fixed-tool requirement for a skill DAG.
pub type FixedTool = crate::types::generated::interface_types::agent::FixedTool;
/// Generated DAG binding mode for a registered TAP skill.
pub type SkillDagBinding = crate::types::generated::interface_types::agent::SkillDagBinding;
/// Generated recurrence mode for a scheduled skill.
pub type SkillRecurrenceKind = crate::types::generated::interface_types::agent::SkillRecurrenceKind;
/// Backwards-compatible public recurrence alias for the generated Move type.
pub type RecurrenceKind = SkillRecurrenceKind;
/// Generated TAP-facing skill requirements.
pub type SkillRequirement = crate::types::generated::interface_types::agent::SkillRequirement;
/// Backwards-compatible plural alias for the generated TAP skill requirement type.
pub type SkillRequirements = SkillRequirement;
/// Generated TAP-facing schedule policy.
pub type SkillSchedulePolicy = crate::types::generated::interface_types::agent::SkillSchedulePolicy;
/// Generated TAP-facing payment policy.
pub type SkillPaymentPolicy = crate::types::generated::interface_types::payment::SkillPaymentPolicy;

impl InterfaceVersion {
    pub const fn new(inner: u64) -> Self {
        Self { inner }
    }
}

impl Copy for InterfaceVersion {}

impl std::hash::Hash for InterfaceVersion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl PartialOrd for InterfaceVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InterfaceVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl fmt::Display for InterfaceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

/// Key for a pinned skill interface revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillRevisionKey {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub interface_revision: InterfaceVersion,
}

impl fmt::Display for SkillRevisionKey {
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
pub struct WorksheetKey {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
}

impl fmt::Display for WorksheetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.agent_id, self.skill_id)
    }
}

/// Payment source policy for an agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMode {
    UserFunded,
    AgentFunded,
}

impl<'de> Deserialize<'de> for PaymentMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawTapPaymentMode {
                UserFunded,
                AgentFunded,
            }

            return RawTapPaymentMode::deserialize(deserializer).map(|mode| match mode {
                RawTapPaymentMode::UserFunded => Self::UserFunded,
                RawTapPaymentMode::AgentFunded => Self::AgentFunded,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_payment_mode_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment mode value"))
    }
}

fn deserialize_payment_mode_value(value: &serde_json::Value) -> Option<PaymentMode> {
    fn from_text(text: &str) -> Option<PaymentMode> {
        match text {
            "user_funded" | "UserFunded" | "userFunded" => Some(PaymentMode::UserFunded),
            "agent_funded" | "AgentFunded" | "agentFunded" => Some(PaymentMode::AgentFunded),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(mode) = from_text(text) {
                        return Some(mode);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(mode) = deserialize_payment_mode_value(fields) {
                    return Some(mode);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

impl Default for SkillPaymentPolicy {
    fn default() -> Self {
        Self::UserFunded
    }
}

impl SkillPaymentPolicy {
    pub fn user_funded() -> Self {
        Self::UserFunded
    }

    pub fn agent_funded(max_budget: u64) -> Self {
        Self::AgentFunded { max_budget }
    }

    pub fn mode(&self) -> PaymentMode {
        match self {
            Self::UserFunded => PaymentMode::UserFunded,
            Self::AgentFunded { .. } => PaymentMode::AgentFunded,
        }
    }

    pub fn max_budget(&self) -> u64 {
        match self {
            Self::UserFunded => 0,
            Self::AgentFunded { max_budget } => *max_budget,
        }
    }
}

/// Source kind for standard TAP execution payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentSourceKind {
    Invoker,
    AgentVault,
}

impl PaymentSourceKind {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Invoker => TAP_PAYMENT_SOURCE_KIND_INVOKER,
            Self::AgentVault => TAP_PAYMENT_SOURCE_KIND_AGENT_VAULT,
        }
    }

    pub fn from_u8(value: u8) -> anyhow::Result<Self> {
        match value {
            TAP_PAYMENT_SOURCE_KIND_INVOKER => Ok(Self::Invoker),
            TAP_PAYMENT_SOURCE_KIND_AGENT_VAULT => Ok(Self::AgentVault),
            _ => anyhow::bail!("unsupported TAP payment source kind {value}"),
        }
    }
}

/// Long-lived payment source recorded on a scheduled TAP task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduledPaymentSource {
    Address {
        #[serde(deserialize_with = "deserialize_tap_address_value")]
        refund_recipient: sui::types::Address,
    },
    AgentVault {
        agent_id: AgentId,
    },
}

impl<'de> Deserialize<'de> for ScheduledPaymentSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum RawSource {
                Address {
                    refund_recipient: sui::types::Address,
                },
                AgentVault {
                    agent_id: AgentId,
                },
            }

            return RawSource::deserialize(deserializer).map(|source| match source {
                RawSource::Address { refund_recipient } => Self::Address { refund_recipient },
                RawSource::AgentVault { agent_id } => Self::AgentVault { agent_id },
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_scheduled_payment_source_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP scheduled payment source value"))
    }
}

fn deserialize_tap_scheduled_payment_source_value(
    value: &serde_json::Value,
) -> Option<ScheduledPaymentSource> {
    let unwrapped = super::strip_fields_owned(value.clone());
    let value = if &unwrapped != value {
        &unwrapped
    } else {
        value
    };
    let serde_json::Value::Object(object) = value else {
        return None;
    };

    if let Some(address) = object.get("Address").or_else(|| object.get("address")) {
        let address = super::strip_fields_owned(address.clone());
        let refund_recipient = address
            .get("refund_recipient")
            .and_then(|value| super::parse_address_value(value).ok().flatten())?;
        return Some(ScheduledPaymentSource::Address { refund_recipient });
    }

    if let Some(vault) = object
        .get("AgentVault")
        .or_else(|| object.get("agent_vault"))
        .or_else(|| object.get("agentVault"))
    {
        let vault = super::strip_fields_owned(vault.clone());
        let agent_id = vault
            .get("agent_id")
            .and_then(|value| super::parse_address_value(value).ok().flatten())?;
        return Some(ScheduledPaymentSource::AgentVault { agent_id });
    }

    let variant = object
        .get("@variant")
        .or_else(|| object.get("variant"))
        .or_else(|| object.get("type"))
        .and_then(|value| value.as_str());
    match variant {
        Some("Address") | Some("address") => {
            let refund_recipient = object
                .get("refund_recipient")
                .and_then(|value| super::parse_address_value(value).ok().flatten())?;
            Some(ScheduledPaymentSource::Address { refund_recipient })
        }
        Some("AgentVault") | Some("agent_vault") | Some("agentVault") => {
            let agent_id = object
                .get("agent_id")
                .and_then(|value| super::parse_address_value(value).ok().flatten())?;
            Some(ScheduledPaymentSource::AgentVault { agent_id })
        }
        _ => None,
    }
}

impl ScheduledPaymentSource {
    pub fn source_kind(&self) -> PaymentSourceKind {
        match self {
            Self::Address { .. } => PaymentSourceKind::Invoker,
            Self::AgentVault { .. } => PaymentSourceKind::AgentVault,
        }
    }

    pub fn source_identity(&self) -> sui::types::Address {
        match self {
            Self::Address { refund_recipient } => *refund_recipient,
            Self::AgentVault { agent_id } => *agent_id,
        }
    }
}

impl<'de> Deserialize<'de> for PaymentSourceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return u8::deserialize(deserializer)
                .and_then(|value| Self::from_u8(value).map_err(D::Error::custom));
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_payment_source_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment source kind value"))
    }
}

fn deserialize_payment_source_kind_value(value: &serde_json::Value) -> Option<PaymentSourceKind> {
    fn from_text(text: &str) -> Option<PaymentSourceKind> {
        match text {
            "invoker" | "Invoker" => Some(PaymentSourceKind::Invoker),
            "agent_vault" | "AgentVault" | "agentVault" => Some(PaymentSourceKind::AgentVault),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Number(number) => number.as_u64().and_then(|value| {
            u8::try_from(value)
                .ok()
                .and_then(|value| PaymentSourceKind::from_u8(value).ok())
        }),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(kind) = from_text(text) {
                        return Some(kind);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(kind) = deserialize_payment_source_kind_value(fields) {
                    return Some(kind);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// Typed standard TAP payment source payload.
///
/// New standard TAP calls use this encoded pair to distinguish invoker-funded
/// settlement from agent-vault-funded settlement. Legacy address-only BCS
/// sources remain accepted by validation for backward compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaymentSource {
    pub kind: PaymentSourceKind,
    pub identity: sui::types::Address,
}

impl PaymentSource {
    pub fn invoker(invoker: sui::types::Address) -> Self {
        Self {
            kind: PaymentSourceKind::Invoker,
            identity: invoker,
        }
    }

    pub fn agent_vault(agent_id: AgentId) -> Self {
        Self {
            kind: PaymentSourceKind::AgentVault,
            identity: agent_id,
        }
    }

    pub fn to_bcs_bytes(self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&(self.kind.as_u8(), self.identity))?)
    }

    pub fn from_bcs_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let (kind, identity): (u8, sui::types::Address) = bcs::from_bytes(bytes)?;
        Ok(Self {
            kind: PaymentSourceKind::from_u8(kind)?,
            identity,
        })
    }
}

impl SkillDagBinding {
    pub fn pinned(dag_id: sui::types::Address) -> Self {
        Self::Pinned { dag_id }
    }

    pub fn runtime_selected() -> Self {
        Self::RuntimeSelected
    }

    pub fn pinned_dag_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::Pinned { dag_id } => Some(*dag_id),
            Self::RuntimeSelected => None,
        }
    }
}

impl Default for SkillSchedulePolicy {
    fn default() -> Self {
        Self {
            recurrence: RecurrenceKind::Once,
            allow_recursive: false,
        }
    }
}

impl FixedTool {
    pub fn new(tool_registry_id: sui::types::Address, tool_fqn: impl Into<String>) -> Self {
        Self {
            tool_registry_id: crate::types::sui_address_to_id(tool_registry_id),
            tool_fqn: MoveString::from(tool_fqn.into()),
        }
    }

    pub fn tool_registry_address(&self) -> sui::types::Address {
        self.tool_registry_id.clone().into()
    }

    pub fn tool_fqn_string(&self) -> String {
        self.tool_fqn.clone().into()
    }
}

impl Default for SkillRequirements {
    fn default() -> Self {
        Self {
            input_commitment: Vec::new(),
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        }
    }
}

/// Stored `nexus_registry::agent_registry::AgentRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRecord {
    pub active: bool,
    pub skills: MoveTable<SkillId, SkillRecord>,
}

/// Stored `nexus_interface::tap::SkillRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecord {
    /// SDK-expanded dynamic table context. This is not part of the on-chain `SkillRecord`.
    #[serde(skip)]
    pub agent_id: Option<AgentId>,
    /// SDK-expanded dynamic table context. This is not part of the on-chain `SkillRecord`.
    #[serde(skip)]
    pub skill_id: Option<SkillId>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub description: Vec<u8>,
    pub active: bool,
    pub dag_binding: SkillDagBinding,
    pub requirements: SkillRequirements,
    pub current_interface_revision: InterfaceVersion,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub scheduled_task_count: u64,
}

/// Standard network default DAG executor for arbitrary-DAG execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefaultDagExecutor {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
}

/// Dynamic field key for the registry-owned default DAG executor value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DefaultDagExecutorFieldKey {}

/// Stored `nexus_interface::tap::DefaultDagExecutor` value. The wrapper owns
/// the default agent on chain; SDK callers only expose its public target IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultDagExecutorValue {
    pub agent: Agent,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
}

impl DefaultDagExecutorValue {
    pub fn target(&self) -> DefaultDagExecutor {
        DefaultDagExecutor {
            agent_id: self.agent.id,
            skill_id: self.skill_id,
        }
    }
}

/// Stored `nexus_interface::tap::Agent` object shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub next_skill_id: u64,
    pub registry_id: MoveOption<sui::types::Address>,
}

/// Raw shared `nexus_registry::agent_registry::AgentRegistry` object contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRegistryObject {
    pub id: sui::types::Address,
    pub agents: MoveTable<sui::types::Address, AgentRecord>,
}

/// Expanded `nexus_registry::agent_registry::AgentRegistry` contents with table entries fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRegistry {
    pub id: sui::types::Address,
    pub agents: Vec<AgentRecord>,
    pub skills: Vec<SkillRecord>,
    #[serde(default)]
    pub default_executor: Option<DefaultDagExecutor>,
}

impl AgentRegistry {
    /// Convert current skill revisions into leader-facing skill revision records.
    pub fn skill_revision_records(&self) -> anyhow::Result<Vec<SkillRevisionRecord>> {
        self.skills
            .iter()
            .filter_map(SkillRevisionRecord::from_skill_record)
            .map(|record| {
                record.validate()?;
                Ok(record)
            })
            .collect()
    }

    pub fn skill_revision_record(
        &self,
        key: SkillRevisionKey,
    ) -> anyhow::Result<SkillRevisionRecord> {
        let matches = self
            .skill_revision_records()?
            .into_iter()
            .filter(|skill_revision| skill_revision.key == key)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [] => anyhow::bail!("TAP skill revision not found for key {key}"),
            [skill_revision] => Ok(skill_revision.clone()),
            _ => anyhow::bail!("duplicate TAP skill revisions found for key {key}"),
        }
    }

    pub fn active_skill_revision_record(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> anyhow::Result<SkillRevisionRecord> {
        let skills = self
            .skills
            .iter()
            .filter(|skill| {
                skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
            })
            .collect::<Vec<_>>();

        let skill = match skills.as_slice() {
            [] => {
                return Err(SkillRevisionResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            [skill] if skill.active => *skill,
            [_] => {
                return Err(SkillRevisionResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            _ => {
                return Err(SkillRevisionResolutionError::DuplicateActiveRevision {
                    agent_id,
                    skill_id,
                    count: skills.len(),
                }
                .into())
            }
        };

        let skill_revision = SkillRevisionRecord {
            key: SkillRevisionKey {
                agent_id,
                skill_id,
                interface_revision: skill.current_interface_revision,
            },
            requirements: skill.requirements.clone(),
        };
        skill_revision.validate()?;
        Ok(skill_revision)
    }

    pub fn default_dag_executor(&self) -> anyhow::Result<DefaultDagExecutor> {
        self.default_executor
            .ok_or_else(|| anyhow::anyhow!("AgentRegistry missing default agent"))
    }
}

/// Active or pinned skill revision record returned to leader and SDK callers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRevisionRecord {
    pub key: SkillRevisionKey,
    pub requirements: SkillRequirements,
}

impl SkillRevisionRecord {
    fn from_skill_record(skill: &SkillRecord) -> Option<Self> {
        let agent_id = skill.agent_id?;
        let skill_id = skill.skill_id?;
        Some(Self {
            key: SkillRevisionKey {
                agent_id,
                skill_id,
                interface_revision: skill.current_interface_revision,
            },
            requirements: skill.requirements.clone(),
        })
    }

    pub fn worksheet_key(&self) -> WorksheetKey {
        WorksheetKey {
            agent_id: self.key.agent_id,
            skill_id: self.key.skill_id,
        }
    }

    pub fn validate(&self) -> Result<(), TapValidationError> {
        validate_requirements(&self.requirements)?;

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DagExecutionPaymentFieldKey {}

/// Dynamic-field key for vault-owned execution payment receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionPaymentReceiptFieldKey {
    pub execution_id: sui::types::Address,
}

/// Dynamic-field key for an agent's standard payment vault.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentVaultFieldKey {}

/// Dynamic-field key for vault payment-history lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionPaymentHistoryFieldKey {
    pub resolved: bool,
}

/// Shared standard Talus agent payment vault object.
///
/// Each agent created by the standard TAP interface has one vault. The vault's
/// `available_balance` includes locked funds; `locked_amount` records the
/// portion reserved by in-flight execution payments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentPaymentVault {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub available_balance: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub locked_amount: u64,
}

/// Registered skill plus the currently active skill revision used for fresh standard execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSkillExecutionTarget {
    pub skill: SkillRecord,
    pub skill_revision: SkillRevisionRecord,
}

/// Default execution target plus active skill revision recovered for fresh default DAG execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultDagExecutorRecord {
    pub target: DefaultDagExecutor,
    pub skill: SkillRecord,
    pub skill_revision: SkillRevisionRecord,
}

/// DAG-backed TAP skill config used by SDK/CLI authoring helpers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillConfig {
    pub name: String,
    pub dag_path: PathBuf,
    pub requirements: SkillRequirements,
    pub interface_revision: InterfaceVersion,
}

impl SkillConfig {
    pub fn validate(&self) -> Result<(), TapValidationError> {
        if self.name.trim().is_empty() {
            return Err(TapValidationError::MissingSkillName);
        }
        if self.dag_path.as_os_str().is_empty() {
            return Err(TapValidationError::MissingDagPath);
        }

        validate_requirements(&self.requirements)
    }
}

/// Published TAP plus DAG artifact used when binding an agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapPublishArtifact {
    pub skill_name: String,
    pub dag_id: sui::types::Address,
    pub interface_revision: InterfaceVersion,
    pub requirements: SkillRequirements,
}

impl TapPublishArtifact {
    pub fn from_config(config: &SkillConfig, dag_id: sui::types::Address) -> anyhow::Result<Self> {
        config.validate()?;

        Ok(Self {
            skill_name: config.name.clone(),
            dag_id,
            interface_revision: config.interface_revision,
            requirements: config.requirements.clone(),
        })
    }
}

pub fn tap_input_commitment_from_dag_inputs<I, V, P>(inputs: I) -> Vec<u8>
where
    I: IntoIterator<Item = (V, P)>,
    V: AsRef<str>,
    P: AsRef<str>,
{
    let mut canonical_inputs = inputs
        .into_iter()
        .map(|(vertex, port)| (vertex.as_ref().to_string(), port.as_ref().to_string()))
        .collect::<Vec<_>>();
    canonical_inputs.sort();

    let encoded =
        bcs::to_bytes(&canonical_inputs).expect("canonical TAP DAG input pairs should encode");
    Sha256::digest(encoded).to_vec()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapValidationError {
    MissingSkillName,
    MissingDagPath,
    MissingInputCommitment,
    EmptyAuthorizedToolModule,
    EmptyAuthorizedToolFunction,
}

impl fmt::Display for TapValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapValidationError::MissingSkillName => write!(f, "skill name is required"),
            TapValidationError::MissingDagPath => write!(f, "DAG path is required"),
            TapValidationError::MissingInputCommitment => write!(f, "input commitment is required"),
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
pub enum SkillRevisionResolutionError {
    MissingActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
    },
    DuplicateActiveRevision {
        agent_id: AgentId,
        skill_id: SkillId,
        count: usize,
    },
    InvalidSkillRevision(TapValidationError),
}

impl fmt::Display for SkillRevisionResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id } => {
                write!(f, "no active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}")
            }
            SkillRevisionResolutionError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count,
            } => write!(
                f,
                "expected one active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}, found {count}"
            ),
            SkillRevisionResolutionError::InvalidSkillRevision(error) => {
                write!(f, "invalid TAP skill revision: {error}")
            }
        }
    }
}

impl std::error::Error for SkillRevisionResolutionError {}

pub fn validate_requirements(requirements: &SkillRequirements) -> Result<(), TapValidationError> {
    if requirements.input_commitment.is_empty() {
        return Err(TapValidationError::MissingInputCommitment);
    }
    for tool in &requirements.fixed_tools {
        if tool.tool_fqn_string().trim().is_empty() {
            return Err(TapValidationError::EmptyAuthorizedToolModule);
        }
    }

    Ok(())
}

pub fn validate_execution_payment_options(
    agent_id: AgentId,
    policy: &SkillPaymentPolicy,
    payment_source: &[u8],
    payment_max_budget: u64,
    payer: sui::types::Address,
) -> anyhow::Result<()> {
    match policy {
        SkillPaymentPolicy::UserFunded => {
            let expected = bcs::to_bytes(&payer)?;
            let source_is_valid =
                payment_source.is_empty() || payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard TAP user-funded payment source must be empty or payer address BCS"
                );
            }
        }
        SkillPaymentPolicy::AgentFunded { max_budget } => {
            if payment_max_budget == 0 || payment_max_budget > *max_budget {
                anyhow::bail!(
                    "standard TAP agent-funded payment budget {} must be positive and no greater than skill policy max {}",
                    payment_max_budget,
                    max_budget
                );
            }
            let expected = bcs::to_bytes(&agent_id)?;
            let source_is_valid = payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard Talus agent-funded payment source must be agent_id address BCS"
                );
            }
        }
    }

    Ok(())
}

pub fn payment_source_from_address(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    Ok(bcs::to_bytes(&address)?)
}

pub fn tap_payment_source_for_invoker(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    PaymentSource::invoker(address).to_bcs_bytes()
}

pub fn tap_payment_source_for_agent_vault(agent_id: AgentId) -> anyhow::Result<Vec<u8>> {
    PaymentSource::agent_vault(agent_id).to_bcs_bytes()
}

/// Resolve exactly one active skill revision for fresh execution.
pub fn resolve_active_tap_skill_revision<'a>(
    records: &'a [SkillRevisionRecord],
    skills: &[SkillRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a SkillRevisionRecord, SkillRevisionResolutionError> {
    let skill_matches = skills
        .iter()
        .filter(|skill| {
            skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
        })
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => {
            return Err(SkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id })
        }
        [skill] if skill.active => *skill,
        [_] => {
            return Err(SkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id })
        }
        _ => {
            return Err(SkillRevisionResolutionError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count: skill_matches.len(),
            })
        }
    };

    let active = records
        .iter()
        .filter(|record| {
            record.key.agent_id == agent_id
                && record.key.skill_id == skill_id
                && record.key.interface_revision == skill.current_interface_revision
        })
        .collect::<Vec<_>>();

    match active.as_slice() {
        [] => Err(SkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id }),
        [record] => {
            record
                .validate()
                .map_err(SkillRevisionResolutionError::InvalidSkillRevision)?;
            Ok(record)
        }
        _ => Err(SkillRevisionResolutionError::DuplicateActiveRevision {
            agent_id,
            skill_id,
            count: active.len(),
        }),
    }
}

/// Resolve the unique active skill and skill revision for fresh standard execution.
pub fn resolve_active_tap_skill_execution_target(
    registry: &AgentRegistry,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<ActiveSkillExecutionTarget> {
    let skill_matches = registry
        .skills
        .iter()
        .filter(|skill| {
            skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
        })
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

    let skill_revision = registry.active_skill_revision_record(agent_id, skill_id)?;

    Ok(ActiveSkillExecutionTarget {
        skill,
        skill_revision,
    })
}

/// Resolve the configured default agent from registry state.
pub fn resolve_default_tap_dag_executor(
    registry: &AgentRegistry,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    let target = registry.default_dag_executor()?;
    let execution_target =
        resolve_active_tap_skill_execution_target(registry, target.agent_id, target.skill_id)?;

    if execution_target.skill.dag_binding != SkillDagBinding::RuntimeSelected {
        anyhow::bail!(
            "default agent skill {} for agent {} is not runtime-DAG selected",
            target.skill_id,
            target.agent_id
        );
    }

    Ok(DefaultDagExecutorRecord {
        target,
        skill: execution_target.skill,
        skill_revision: execution_target.skill_revision,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{
            ExecutionPaymentFinalState,
            ScheduledOccurrenceFinalState,
            VertexExecutionPaymentSettlementKind,
        },
        std::str::FromStr,
    };

    fn addr(value: &str) -> sui::types::Address {
        sui::types::Address::from_str(value).expect("valid address")
    }

    fn requirements() -> SkillRequirements {
        SkillRequirements {
            input_commitment: vec![1],
            payment_policy: SkillPaymentPolicy::AgentFunded { max_budget: 100 },
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        }
    }

    fn skill_revision(revision: u64) -> SkillRevisionRecord {
        SkillRevisionRecord {
            key: SkillRevisionKey {
                agent_id: addr("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(revision),
            },
            requirements: requirements(),
        }
    }

    fn skill(active: bool, current_interface_revision: u64) -> SkillRecord {
        SkillRecord {
            agent_id: Some(addr("0xa")),
            skill_id: Some(11),
            description: vec![3],
            active,
            dag_binding: SkillDagBinding::pinned(addr("0x44")),
            requirements: requirements(),
            current_interface_revision: InterfaceVersion::new(current_interface_revision),
            scheduled_task_count: 0,
        }
    }

    fn registry_with_active_skill() -> AgentRegistry {
        AgentRegistry {
            id: addr("0xf"),
            agents: Vec::new(),
            skills: vec![skill(true, 2)],
            default_executor: None,
        }
    }

    #[test]
    fn skill_revision_records_are_derived_from_current_skills() {
        let registry = registry_with_active_skill();
        let records = registry.skill_revision_records().expect("derived records");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].key.agent_id, addr("0xa"));
        assert_eq!(records[0].key.skill_id, 11);
        assert_eq!(records[0].key.interface_revision, InterfaceVersion::new(2));
    }

    #[test]
    fn validate_rejects_missing_input_commitment() {
        let mut requirements = requirements();
        requirements.input_commitment.clear();

        assert_eq!(
            validate_requirements(&requirements),
            Err(TapValidationError::MissingInputCommitment)
        );
    }

    #[test]
    fn active_resolution_requires_exactly_one_active_revision() {
        let active = skill_revision(1);
        let inactive = skill_revision(2);
        let records = vec![active.clone(), inactive];
        let skills = vec![skill(true, 1)];

        let resolved = resolve_active_tap_skill_revision(
            &records,
            &skills,
            active.key.agent_id,
            active.key.skill_id,
        )
        .expect("one active skill revision");

        assert_eq!(resolved.key.interface_revision, InterfaceVersion::new(1));

        let duplicate = vec![skill_revision(1), skill_revision(1)];
        assert!(matches!(
            resolve_active_tap_skill_revision(&duplicate, &skills, addr("0xa"), 11),
            Err(SkillRevisionResolutionError::DuplicateActiveRevision { count: 2, .. })
        ));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn agent_registry_object_bcs_decodes_without_inline_default_executor() {
        #[derive(Serialize)]
        struct RawTapRegistryObjectBcs {
            id: sui::types::Address,
            agents: MoveTable<sui::types::Address, AgentRecord>,
        }

        let raw = RawTapRegistryObjectBcs {
            id: addr("0xf"),
            agents: MoveTable::new(addr("0x90"), 0),
        };
        let bytes = bcs::to_bytes(&raw).expect("raw Move registry BCS should encode");
        let decoded: AgentRegistryObject =
            bcs::from_bytes(&bytes).expect("raw Move registry BCS should decode");

        assert_eq!(decoded.id, addr("0xf"));
        assert_eq!(decoded.agents.id(), addr("0x90"));
    }

    #[test]
    fn publish_artifact_contains_skill_artifact_fields() {
        let config = SkillConfig {
            name: "weather".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            requirements: requirements(),
            interface_revision: InterfaceVersion::new(1),
        };

        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.dag_id, addr("0x8"));
        assert_eq!(artifact.skill_name, "weather");
    }

    #[test]
    fn publish_artifact_preserves_current_skill_inputs() {
        let config = SkillConfig {
            name: "weather".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            requirements: requirements(),
            interface_revision: InterfaceVersion::new(1),
        };
        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.interface_revision, InterfaceVersion::new(1));
        assert_eq!(artifact.requirements, config.requirements);
    }

    #[test]
    fn dag_input_commitment_is_order_independent() {
        let first = tap_input_commitment_from_dag_inputs([
            ("weather::V2", "city"),
            ("weather::V1", "country"),
        ]);
        let second = tap_input_commitment_from_dag_inputs([
            ("weather::V1", "country"),
            ("weather::V2", "city"),
        ]);

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn validate_execution_payment_options_enforces_user_funded_policy() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let explicit_source = payment_source_from_address(payer).expect("payer source");
        let typed_source = tap_payment_source_for_invoker(payer).expect("typed payer source");
        let other_source = payment_source_from_address(addr("0x2")).expect("other source");
        let policy = SkillPaymentPolicy::UserFunded;

        validate_execution_payment_options(agent, &policy, &[], 100, payer)
            .expect("implicit payer source");
        validate_execution_payment_options(agent, &policy, &explicit_source, 100, payer)
            .expect("explicit payer source");
        assert!(
            validate_execution_payment_options(agent, &policy, &typed_source, 100, payer,).is_err(),
            "typed invoker sources are not accepted by Move direct user-funded policy"
        );
        assert!(
            validate_execution_payment_options(agent, &policy, &other_source, 100, payer,).is_err()
        );
    }

    #[test]
    fn validate_execution_payment_options_enforces_source_modes() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let legacy_agent_source = payment_source_from_address(agent).expect("agent source");
        let agent_source = tap_payment_source_for_agent_vault(agent).expect("agent vault source");

        let agent_funded = SkillPaymentPolicy::AgentFunded { max_budget: 100 };
        validate_execution_payment_options(agent, &agent_funded, &legacy_agent_source, 100, payer)
            .expect("agent-funded source at policy cap");
        assert!(
            validate_execution_payment_options(agent, &agent_funded, &[], 100, payer,).is_err()
        );
        assert!(validate_execution_payment_options(
            agent,
            &agent_funded,
            &agent_source,
            100,
            payer,
        )
        .is_err());
        assert!(validate_execution_payment_options(
            agent,
            &agent_funded,
            &legacy_agent_source,
            101,
            payer,
        )
        .is_err());
    }

    #[test]
    fn removed_payment_modes_do_not_deserialize() {
        for mode in ["hybrid", "Hybrid", "sponsored", "Sponsored"] {
            let value = serde_json::json!(mode);
            assert!(serde_json::from_value::<PaymentMode>(value).is_err());
        }
    }

    #[test]
    fn tap_enum_deserializers_accept_move_json_forms() {
        assert_eq!(
            serde_json::from_value::<PaymentMode>(serde_json::json!({
                "fields": { "variant": "agentFunded" }
            }))
            .expect("nested payment mode"),
            PaymentMode::AgentFunded
        );
        assert_eq!(
            serde_json::from_value::<PaymentMode>(serde_json::json!({
                "UserFunded": {}
            }))
            .expect("keyed payment mode"),
            PaymentMode::UserFunded
        );
        assert_eq!(
            serde_json::from_value::<PaymentSourceKind>(serde_json::json!(1))
                .expect("numeric payment source kind"),
            PaymentSourceKind::AgentVault
        );
        assert_eq!(
            serde_json::from_value::<PaymentSourceKind>(serde_json::json!({
                "fields": { "@variant": "invoker" }
            }))
            .expect("nested payment source kind"),
            PaymentSourceKind::Invoker
        );
        assert!(serde_json::from_value::<PaymentSourceKind>(serde_json::json!(7)).is_err());

        assert_eq!(
            serde_json::from_value::<VertexExecutionPaymentSettlementKind>(serde_json::json!({
                "Paid": {}
            }))
            .expect("keyed settlement kind"),
            VertexExecutionPaymentSettlementKind::Paid
        );
        assert_eq!(
            serde_json::from_value::<VertexExecutionPaymentSettlementKind>(serde_json::json!({
                "fields": { "type": "Ticket" }
            }))
            .expect("nested settlement kind"),
            VertexExecutionPaymentSettlementKind::Ticket
        );
        assert_eq!(
            bcs::from_bytes::<VertexExecutionPaymentSettlementKind>(
                &bcs::to_bytes(&9_u8).expect("raw settlement kind")
            )
            .expect("unknown raw settlement kind falls back"),
            VertexExecutionPaymentSettlementKind::Paid
        );

        assert_eq!(
            serde_json::from_value::<ExecutionPaymentFinalState>(serde_json::json!({
                "fields": { "variant": "Accomplished" }
            }))
            .expect("nested payment final state"),
            ExecutionPaymentFinalState::Accomplished
        );
        assert_eq!(
            serde_json::from_value::<ScheduledOccurrenceFinalState>(serde_json::json!({
                "fields": { "@variant": "inFlight" }
            }))
            .expect("nested scheduled occurrence state"),
            ScheduledOccurrenceFinalState::InFlight
        );
    }

    #[test]
    fn scheduled_payment_source_deserializes_supported_shapes() {
        let address_source: ScheduledPaymentSource = serde_json::from_value(serde_json::json!({
            "fields": {
                "@variant": "address",
                "refund_recipient": "0xee"
            }
        }))
        .expect("variant address source");
        assert_eq!(address_source.source_kind(), PaymentSourceKind::Invoker);
        assert_eq!(address_source.source_identity(), addr("0xee"));

        let vault_source: ScheduledPaymentSource = serde_json::from_value(serde_json::json!({
            "agentVault": {
                "fields": {
                    "agent_id": "0xaa"
                }
            }
        }))
        .expect("nested vault source");
        assert_eq!(vault_source.source_kind(), PaymentSourceKind::AgentVault);
        assert_eq!(vault_source.source_identity(), addr("0xaa"));

        assert!(
            serde_json::from_value::<ScheduledPaymentSource>(serde_json::json!({
                "fields": { "@variant": "agentVault" }
            }))
            .is_err()
        );
    }

    #[test]
    fn tap_byte_string_deserializes_hex_utf8_and_plain_text() {
        let template: crate::types::AgentVertexAuthorizationTemplate =
            serde_json::from_value(serde_json::json!({
                "skill_id": "7",
                "vertex": "0x656e747279",
                "recipient_id": "0xda6"
            }))
            .expect("hex byte strings decode as UTF-8");

        assert_eq!(template.vertex.as_str(), "entry");

        let template: crate::types::AgentVertexAuthorizationTemplate =
            serde_json::from_value(serde_json::json!({
                "skill_id": "7",
                "vertex": "0xnothex",
                "recipient_id": "0xda6"
            }))
            .expect("plain byte string remains text");

        assert_eq!(template.vertex.as_str(), "0xnothex");
    }

    #[test]
    fn tap_payment_source_bcs_roundtrips_and_rejects_unknown_kind() {
        let invoker = addr("0x21");
        let typed = PaymentSource::from_bcs_bytes(
            &tap_payment_source_for_invoker(invoker).expect("typed invoker source"),
        )
        .expect("typed invoker source decodes");
        assert_eq!(typed.kind, PaymentSourceKind::Invoker);
        assert_eq!(typed.identity, invoker);

        let agent = addr("0x22");
        let typed = PaymentSource::from_bcs_bytes(
            &tap_payment_source_for_agent_vault(agent).expect("typed vault source"),
        )
        .expect("typed vault source decodes");
        assert_eq!(typed.kind, PaymentSourceKind::AgentVault);
        assert_eq!(typed.identity, agent);

        let invalid = bcs::to_bytes(&(9_u8, invoker)).expect("invalid source kind bytes");
        assert!(PaymentSource::from_bcs_bytes(&invalid).is_err());
    }

    #[test]
    fn active_skill_execution_target_requires_one_active_skill_and_endpoint() {
        let registry = registry_with_active_skill();
        let resolved = resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11)
            .expect("active skill target");

        assert_eq!(
            resolved.skill.dag_binding,
            SkillDagBinding::pinned(addr("0x44"))
        );
        assert_eq!(
            resolved.skill_revision.key.interface_revision,
            InterfaceVersion::new(2)
        );
    }

    #[test]
    fn default_dag_executor_requires_runtime_selected_skill() {
        let mut registry = registry_with_active_skill();
        registry.default_executor = Some(DefaultDagExecutor {
            agent_id: addr("0xa"),
            skill_id: 11,
        });

        let error = resolve_default_tap_dag_executor(&registry)
            .expect_err("pinned skill cannot be default runtime target");
        assert!(error.to_string().contains("is not runtime-DAG selected"));

        registry.skills[0].dag_binding = SkillDagBinding::runtime_selected();
        let target = resolve_default_tap_dag_executor(&registry)
            .expect("runtime-selected default skill resolves");

        assert_eq!(
            target.target,
            DefaultDagExecutor {
                agent_id: addr("0xa"),
                skill_id: 11,
            }
        );
        assert_eq!(target.skill.dag_binding, SkillDagBinding::RuntimeSelected);
    }

    #[test]
    fn active_skill_execution_target_rejects_missing_skill() {
        let mut registry = registry_with_active_skill();
        registry.skills.clear();

        assert!(resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11).is_err());
    }
}
