//! Standard TAP models shared by SDK, CLI, leader, and future Move surfaces.

use {
    super::{MoveOption, MoveTable, RuntimeVertex},
    crate::sui,
    serde::{de::Error as _, Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{fmt, path::PathBuf},
};

const TAP_PAYMENT_SOURCE_KIND_INVOKER: u8 = 0;
const TAP_PAYMENT_SOURCE_KIND_AGENT_VAULT: u8 = 1;

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

fn deserialize_tap_u64_value_or_default<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return u64::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(super::parse_u64_value(&value)
        .map_err(D::Error::custom)?
        .unwrap_or_default())
}

fn deserialize_tap_address_value_or_default<'de, D>(
    deserializer: D,
) -> Result<sui::types::Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return sui::types::Address::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(super::parse_address_value(&value)
        .map_err(D::Error::custom)?
        .unwrap_or(sui::types::Address::ZERO))
}

fn default_tap_address() -> sui::types::Address {
    sui::types::Address::ZERO
}

fn deserialize_move_option_tap_address<'de, D>(
    deserializer: D,
) -> Result<Option<sui::types::Address>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    MoveOption::<sui::types::Address>::deserialize(deserializer).map(|value| value.0)
}

fn deserialize_move_option_interface_revision<'de, D>(
    deserializer: D,
) -> Result<Option<InterfaceRevision>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    MoveOption::<InterfaceRevision>::deserialize(deserializer).map(|value| value.0)
}

fn deserialize_move_option_payment_source_kind<'de, D>(
    deserializer: D,
) -> Result<Option<TapPaymentSourceKind>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    MoveOption::<TapPaymentSourceKind>::deserialize(deserializer).map(|value| value.0)
}

fn deserialize_tap_byte_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        return String::from_utf8(bytes).map_err(D::Error::custom);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(bytes) = super::parse_byte_vector_value(&value).map_err(D::Error::custom)? {
        return String::from_utf8(bytes).map_err(D::Error::custom);
    }

    let text = super::parse_string_value(&value)
        .map_err(D::Error::custom)?
        .ok_or_else(|| D::Error::custom("missing TAP byte-string value"))?;

    if let Some(hex) = text.strip_prefix("0x") {
        if hex.len() % 2 == 0 && hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            if let Ok(bytes) = hex::decode(hex) {
                if let Ok(decoded) = String::from_utf8(bytes) {
                    return Ok(decoded);
                }
            }
        }
    }

    Ok(text)
}

fn deserialize_tap_byte_vector<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return Vec::<u8>::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    super::parse_byte_vector_value(&value)
        .map_err(D::Error::custom)?
        .ok_or_else(|| D::Error::custom("missing TAP byte-vector value"))
}

fn deserialize_tap_runtime_vertex<'de, D>(deserializer: D) -> Result<RuntimeVertex, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return RuntimeVertex::deserialize(deserializer);
    }

    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(vertex) = super::parse_runtime_vertex_value(&value).map_err(D::Error::custom)? {
        return Ok(vertex);
    }

    if let Some(name) = super::parse_string_value(&value).map_err(D::Error::custom)? {
        return Ok(RuntimeVertex::plain(&name));
    }

    Err(D::Error::custom("missing TAP runtime vertex value"))
}

/// On-chain generated standard Talus agent ID.
pub type AgentId = sui::types::Address;

/// Agent-local standard TAP skill identity index.
pub type SkillId = u64;

pub const fn skill_id(value: u64) -> SkillId {
    value
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
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
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

/// Dynamic table key used by the registry-level endpoint revision table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapEndpointRevisionKey {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
}

impl TapEndpointRevisionKey {
    pub fn new(
        agent_id: AgentId,
        skill_id: SkillId,
        interface_revision: InterfaceRevision,
    ) -> Self {
        Self {
            agent_id,
            skill_id,
            interface_revision,
        }
    }
}

/// Key for fresh worksheet and active-revision lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapWorksheetKey {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
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
    pub mutable: bool,
}

impl TapSharedObjectRef {
    pub fn immutable(id: sui::types::Address) -> Self {
        Self { id, mutable: false }
    }

    pub fn mutable(id: sui::types::Address) -> Self {
        Self { id, mutable: true }
    }
}

/// Payment source policy for an agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapPaymentMode {
    UserFunded,
    AgentFunded,
}

impl<'de> Deserialize<'de> for TapPaymentMode {
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
        deserialize_tap_payment_mode_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment mode value"))
    }
}

fn deserialize_tap_payment_mode_value(value: &serde_json::Value) -> Option<TapPaymentMode> {
    fn from_text(text: &str) -> Option<TapPaymentMode> {
        match text {
            "user_funded" | "UserFunded" | "userFunded" => Some(TapPaymentMode::UserFunded),
            "agent_funded" | "AgentFunded" | "agentFunded" => Some(TapPaymentMode::AgentFunded),
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
                if let Some(mode) = deserialize_tap_payment_mode_value(fields) {
                    return Some(mode);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// TAP-facing payment policy summary used by config digest and dry-run checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapPaymentPolicy {
    pub mode: TapPaymentMode,
    pub max_budget: u64,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub token_type_commitment: Vec<u8>,
    pub refund_mode: u8,
}

/// Source kind for standard TAP execution payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapPaymentSourceKind {
    Invoker,
    AgentVault,
}

impl TapPaymentSourceKind {
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
pub enum TapScheduledPaymentSource {
    Address {
        #[serde(deserialize_with = "deserialize_tap_address_value")]
        refund_recipient: sui::types::Address,
    },
    AgentVault {
        agent_id: AgentId,
    },
}

impl<'de> Deserialize<'de> for TapScheduledPaymentSource {
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
) -> Option<TapScheduledPaymentSource> {
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
        return Some(TapScheduledPaymentSource::Address { refund_recipient });
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
        return Some(TapScheduledPaymentSource::AgentVault { agent_id });
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
            Some(TapScheduledPaymentSource::Address { refund_recipient })
        }
        Some("AgentVault") | Some("agent_vault") | Some("agentVault") => {
            let agent_id = object
                .get("agent_id")
                .and_then(|value| super::parse_address_value(value).ok().flatten())?;
            Some(TapScheduledPaymentSource::AgentVault { agent_id })
        }
        _ => None,
    }
}

impl TapScheduledPaymentSource {
    pub fn source_kind(&self) -> TapPaymentSourceKind {
        match self {
            Self::Address { .. } => TapPaymentSourceKind::Invoker,
            Self::AgentVault { .. } => TapPaymentSourceKind::AgentVault,
        }
    }

    pub fn source_identity(&self) -> sui::types::Address {
        match self {
            Self::Address { refund_recipient } => *refund_recipient,
            Self::AgentVault { agent_id } => *agent_id,
        }
    }
}

/// TAP scheduled-task link stored in `nexus_scheduler::scheduler::Task.data`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledTaskLink {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub source_kind: TapPaymentSourceKind,
}

impl<'de> Deserialize<'de> for TapPaymentSourceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return u8::deserialize(deserializer)
                .and_then(|value| Self::from_u8(value).map_err(D::Error::custom));
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_payment_source_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment source kind value"))
    }
}

fn deserialize_tap_payment_source_kind_value(
    value: &serde_json::Value,
) -> Option<TapPaymentSourceKind> {
    fn from_text(text: &str) -> Option<TapPaymentSourceKind> {
        match text {
            "invoker" | "Invoker" => Some(TapPaymentSourceKind::Invoker),
            "agent_vault" | "AgentVault" | "agentVault" => Some(TapPaymentSourceKind::AgentVault),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Number(number) => number.as_u64().and_then(|value| {
            u8::try_from(value)
                .ok()
                .and_then(|value| TapPaymentSourceKind::from_u8(value).ok())
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
                if let Some(kind) = deserialize_tap_payment_source_kind_value(fields) {
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
pub struct TapPaymentSource {
    pub kind: TapPaymentSourceKind,
    pub identity: sui::types::Address,
}

impl TapPaymentSource {
    pub fn invoker(invoker: sui::types::Address) -> Self {
        Self {
            kind: TapPaymentSourceKind::Invoker,
            identity: invoker,
        }
    }

    pub fn agent_vault(agent_id: AgentId) -> Self {
        Self {
            kind: TapPaymentSourceKind::AgentVault,
            identity: agent_id,
        }
    }

    pub fn to_bcs_bytes(self) -> anyhow::Result<Vec<u8>> {
        Ok(bcs::to_bytes(&(self.kind.as_u8(), self.identity))?)
    }

    pub fn from_bcs_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let (kind, identity): (u8, sui::types::Address) = bcs::from_bytes(bytes)?;
        Ok(Self {
            kind: TapPaymentSourceKind::from_u8(kind)?,
            identity,
        })
    }
}

impl Default for TapPaymentPolicy {
    fn default() -> Self {
        Self {
            mode: TapPaymentMode::UserFunded,
            max_budget: 0,
            token_type_commitment: Vec::new(),
            refund_mode: 0,
        }
    }
}

/// DAG binding mode for a registered TAP skill.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TapDagBinding {
    Pinned { dag_id: sui::types::Address },
    RuntimeSelected,
}

impl TapDagBinding {
    pub fn pinned(dag_id: sui::types::Address) -> Self {
        Self::Pinned { dag_id }
    }

    pub fn runtime_selected() -> Self {
        Self::RuntimeSelected
    }

    pub fn pinned_dag_id(self) -> Option<sui::types::Address> {
        match self {
            Self::Pinned { dag_id } => Some(dag_id),
            Self::RuntimeSelected => None,
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
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub min_interval_ms: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
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
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub operation_commitment: Vec<u8>,
}

/// Vertex authorization schema committed into endpoint config.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapVertexAuthorizationSchema {
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub schema_commitment: Vec<u8>,
    pub fixed_tools: Vec<TapAuthorizedTool>,
    pub requires_payment: bool,
}

impl TapVertexAuthorizationSchema {
    /// True when the schema carries no fixed tools and does not require payment.
    /// In that case, the chain's `register_skill` reconstructs the requirements
    /// digest with this same default schema, so callers can use the simpler
    /// register entrypoint. When this returns false, callers must route through
    /// `register_skill_with_vertex_authorization_schema` so the chain sees the
    /// real schema during digest validation.
    pub fn is_default(&self) -> bool {
        self.fixed_tools.is_empty() && !self.requires_payment
    }
}

/// User-facing skill requirements fetched before dry-run or execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillRequirements {
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub input_schema_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub workflow_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub metadata_commitment: Vec<u8>,
    pub payment_policy: TapPaymentPolicy,
    pub schedule_policy: TapSchedulePolicy,
    pub vertex_authorization_schema: TapVertexAuthorizationSchema,
}

/// Stored `nexus_registry::agent_registry::AgentRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapAgentRecord {
    pub active: bool,
    pub skills: MoveTable<SkillId, TapSkillRecord>,
}

/// Stored `nexus_interface::tap::SkillRecord`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillRecord {
    /// SDK-expanded dynamic table context. This is not part of the on-chain `SkillRecord`.
    #[serde(skip)]
    pub agent_id: Option<AgentId>,
    /// SDK-expanded dynamic table context. This is not part of the on-chain `SkillRecord`.
    #[serde(skip)]
    pub skill_id: Option<SkillId>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub description: Vec<u8>,
    pub active: bool,
    pub dag_binding: TapDagBinding,
    pub requirements: TapSkillRequirements,
    pub current_interface_revision: InterfaceRevision,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub outstanding_scheduled_task_count: u64,
}

/// Stored `nexus_interface::tap::EndpointRevision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapEndpointRevision {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub config_digest: Vec<u8>,
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
        let record = TapEndpointRecord {
            key: self.key(),
            shared_objects: self.shared_objects.clone(),
            config_digest: self.config_digest.clone(),
            requirements: self.requirements.clone(),
        };

        record.validate()?;
        Ok(record)
    }
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
    pub agent: TapAgentObject,
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
pub struct TapAgentObject {
    pub id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub next_skill_id: u64,
    pub registry_id: MoveOption<sui::types::Address>,
}

/// Raw shared `nexus_registry::agent_registry::AgentRegistry` object contents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapRegistryObject {
    pub id: sui::types::Address,
    pub agents: MoveTable<sui::types::Address, TapAgentRecord>,
    pub endpoints: MoveTable<TapEndpointRevisionKey, TapEndpointRevision>,
}

/// Expanded `nexus_registry::agent_registry::AgentRegistry` contents with table entries fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapRegistry {
    pub id: sui::types::Address,
    pub agents: Vec<TapAgentRecord>,
    pub skills: Vec<TapSkillRecord>,
    pub endpoints: Vec<TapEndpointRevision>,
    #[serde(default)]
    pub default_executor: Option<DefaultDagExecutor>,
}

impl TapRegistry {
    /// Convert all endpoint revisions into leader-facing endpoint records.
    pub fn endpoint_records(&self) -> anyhow::Result<Vec<TapEndpointRecord>> {
        self.endpoints
            .iter()
            .map(TapEndpointRevision::to_endpoint_record)
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
            [endpoint] => endpoint.to_endpoint_record(),
            _ => anyhow::bail!("duplicate TAP endpoint revisions found for key {key}"),
        }
    }

    pub fn active_endpoint_record(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> anyhow::Result<TapEndpointRecord> {
        let skills = self
            .skills
            .iter()
            .filter(|skill| {
                skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
            })
            .collect::<Vec<_>>();

        let skill = match skills.as_slice() {
            [] => {
                return Err(TapEndpointResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            [skill] if skill.active => *skill,
            [_] => {
                return Err(TapEndpointResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            _ => {
                return Err(TapEndpointResolutionError::DuplicateActiveRevision {
                    agent_id,
                    skill_id,
                    count: skills.len(),
                }
                .into())
            }
        };

        let key = TapEndpointKey {
            agent_id,
            skill_id,
            interface_revision: skill.current_interface_revision,
        };
        self.endpoint_record(key)
    }

    pub fn default_dag_executor(&self) -> anyhow::Result<DefaultDagExecutor> {
        self.default_executor
            .ok_or_else(|| anyhow::anyhow!("AgentRegistry missing default TAP DAG executor"))
    }
}

/// Active or pinned endpoint record returned to leader and SDK callers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapEndpointRecord {
    pub key: TapEndpointKey,
    pub shared_objects: Vec<TapSharedObjectRef>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub config_digest: Vec<u8>,
    pub requirements: TapSkillRequirements,
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

/// Execution-linked payment object model used by leader recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapExecutionPayment {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payer: sui::types::Address,
    pub payment_mode: TapPaymentMode,
    #[serde(
        default,
        deserialize_with = "deserialize_move_option_payment_source_kind"
    )]
    pub source_kind: Option<TapPaymentSourceKind>,
    #[serde(default, deserialize_with = "deserialize_move_option_tap_address")]
    pub source_identity: Option<sui::types::Address>,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub max_budget: u64,
    #[serde(default, deserialize_with = "deserialize_tap_u64_value")]
    pub locked_budget: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub consumed: u64,
    pub refund_mode: u8,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub payment_source_hash: Vec<u8>,
    pub accomplished: bool,
    pub refunded: bool,
    #[serde(default)]
    pub final_state: Option<TapExecutionPaymentFinalState>,
    #[serde(default)]
    pub locked_vertices: Vec<TapExecutionPaymentVertexLock>,
}

impl TapExecutionPayment {
    pub fn payment_id(&self) -> sui::types::Address {
        self.id
    }

    pub fn endpoint_key(&self) -> TapEndpointKey {
        TapEndpointKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }

    pub fn outstanding_locks(&self) -> u64 {
        self.locked_vertices.len() as u64
    }
}

/// Wallet- or agent-owned receipt for a standard TAP execution payment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapExecutionPaymentReceipt {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payment_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payer: sui::types::Address,
    pub source_kind: TapPaymentSourceKind,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub source_identity: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub max_budget: u64,
    pub resolved: bool,
}

impl TapExecutionPaymentReceipt {
    pub fn receipt_id(&self) -> sui::types::Address {
        self.id
    }
}

/// Dynamic-field key for vault-owned execution payment receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapExecutionPaymentReceiptFieldKey {
    pub execution_id: sui::types::Address,
}

/// Dynamic-field key for an agent's standard payment vault.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapAgentVaultFieldKey {}

/// Dynamic-field key for vault payment-history lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapExecutionPaymentHistoryFieldKey {
    pub resolved: bool,
}

/// Agent-owned unresolved or resolved execution-payment history list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapExecutionPaymentHistoryList {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    pub execution_ids: Vec<sui::types::Address>,
}

/// Execution payment vertex lock decoded from TAP payment state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapExecutionPaymentVertexLock {
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub vertex_key: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub tool_fqn: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub amount: u64,
    pub settlement_kind: TapVertexExecutionPaymentSettlementKind,
}

/// Tool-payment settlement class for an execution payment vertex lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapVertexExecutionPaymentSettlementKind {
    Free,
    Ticket,
    Paid,
}

impl<'de> Deserialize<'de> for TapVertexExecutionPaymentSettlementKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            return u8::deserialize(deserializer).map(|value| match value {
                0 => Self::Free,
                1 => Self::Ticket,
                2 => Self::Paid,
                _ => Self::Paid,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_payment_settlement_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment settlement kind value"))
    }
}

fn deserialize_tap_payment_settlement_kind_value(
    value: &serde_json::Value,
) -> Option<TapVertexExecutionPaymentSettlementKind> {
    fn from_text(text: &str) -> Option<TapVertexExecutionPaymentSettlementKind> {
        match text {
            "free" | "Free" => Some(TapVertexExecutionPaymentSettlementKind::Free),
            "ticket" | "Ticket" => Some(TapVertexExecutionPaymentSettlementKind::Ticket),
            "paid" | "Paid" => Some(TapVertexExecutionPaymentSettlementKind::Paid),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(kind) = from_text(text) {
                        return Some(kind);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(kind) = deserialize_tap_payment_settlement_kind_value(fields) {
                    return Some(kind);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// Final state for a standard TAP execution payment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapExecutionPaymentFinalState {
    Pending,
    Accomplished,
    Refunded,
}

impl<'de> Deserialize<'de> for TapExecutionPaymentFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawState {
                Pending,
                Accomplished,
                Refunded,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::Pending => Self::Pending,
                RawState::Accomplished => Self::Accomplished,
                RawState::Refunded => Self::Refunded,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_execution_payment_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP execution payment final state value"))
    }
}

/// Lifecycle state for a durable scheduled TAP task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapScheduledTaskState {
    Active,
    Canceled,
    Completed,
    Exhausted,
}

impl<'de> Deserialize<'de> for TapScheduledTaskState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawState {
                Active,
                Canceled,
                Completed,
                Exhausted,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::Active => Self::Active,
                RawState::Canceled => Self::Canceled,
                RawState::Completed => Self::Completed,
                RawState::Exhausted => Self::Exhausted,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_scheduled_task_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP scheduled task state value"))
    }
}

fn deserialize_tap_scheduled_task_state_value(
    value: &serde_json::Value,
) -> Option<TapScheduledTaskState> {
    fn from_text(text: &str) -> Option<TapScheduledTaskState> {
        match text {
            "active" | "Active" => Some(TapScheduledTaskState::Active),
            "canceled" | "Canceled" => Some(TapScheduledTaskState::Canceled),
            "completed" | "Completed" => Some(TapScheduledTaskState::Completed),
            "exhausted" | "Exhausted" => Some(TapScheduledTaskState::Exhausted),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(state) = from_text(text) {
                        return Some(state);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(state) = deserialize_tap_scheduled_task_state_value(fields) {
                    return Some(state);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// Final state for one scheduled occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TapScheduledOccurrenceFinalState {
    InFlight,
    Accomplished,
    Refunded,
}

impl<'de> Deserialize<'de> for TapScheduledOccurrenceFinalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            #[serde(rename_all = "snake_case")]
            enum RawState {
                InFlight,
                Accomplished,
                Refunded,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::InFlight => Self::InFlight,
                RawState::Accomplished => Self::Accomplished,
                RawState::Refunded => Self::Refunded,
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_scheduled_occurrence_final_state_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP scheduled occurrence final state value"))
    }
}

fn deserialize_tap_scheduled_occurrence_final_state_value(
    value: &serde_json::Value,
) -> Option<TapScheduledOccurrenceFinalState> {
    fn from_text(text: &str) -> Option<TapScheduledOccurrenceFinalState> {
        match text {
            "in_flight" | "inFlight" | "InFlight" => {
                Some(TapScheduledOccurrenceFinalState::InFlight)
            }
            "accomplished" | "Accomplished" => Some(TapScheduledOccurrenceFinalState::Accomplished),
            "refunded" | "Refunded" => Some(TapScheduledOccurrenceFinalState::Refunded),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(state) = from_text(text) {
                        return Some(state);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(state) = deserialize_tap_scheduled_occurrence_final_state_value(fields)
                {
                    return Some(state);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

fn deserialize_tap_execution_payment_final_state_value(
    value: &serde_json::Value,
) -> Option<TapExecutionPaymentFinalState> {
    fn from_text(text: &str) -> Option<TapExecutionPaymentFinalState> {
        match text {
            "pending" | "Pending" => Some(TapExecutionPaymentFinalState::Pending),
            "accomplished" | "Accomplished" => Some(TapExecutionPaymentFinalState::Accomplished),
            "refunded" | "Refunded" => Some(TapExecutionPaymentFinalState::Refunded),
            _ => None,
        }
    }

    match value {
        serde_json::Value::String(text) => from_text(text),
        serde_json::Value::Object(object) => {
            for key in ["@variant", "variant", "type"] {
                if let Some(serde_json::Value::String(text)) = object.get(key) {
                    if let Some(state) = from_text(text) {
                        return Some(state);
                    }
                }
            }

            if let Some(fields) = object.get("fields") {
                if let Some(state) = deserialize_tap_execution_payment_final_state_value(fields) {
                    return Some(state);
                }
            }

            object.keys().find_map(|key| from_text(key))
        }
        _ => None,
    }
}

/// Shared standard Talus agent payment vault object.
///
/// Each agent created by the standard TAP interface has one vault. The vault's
/// `available_balance` includes locked funds; `locked_amount` records the
/// portion reserved by in-flight execution payments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapAgentPaymentVault {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub available_balance: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub locked_amount: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowVertexAuthorizationGrant {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_runtime_vertex")]
    pub vertex: RuntimeVertex,
    #[serde(default, deserialize_with = "deserialize_move_option_tap_address")]
    pub scheduled_grant_id: Option<sui::types::Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowVertexAuthorizationGrantFieldKey {
    #[serde(deserialize_with = "deserialize_tap_runtime_vertex")]
    pub vertex: RuntimeVertex,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowVertexAuthorizationGrantAccess {
    pub grant: WorkflowVertexAuthorizationGrant,
    pub object_ref: sui::types::ObjectReference,
}

impl WorkflowVertexAuthorizationGrantAccess {
    pub fn grant_id(&self) -> sui::types::Address {
        self.grant.id
    }

    pub fn object_id(&self) -> sui::types::Address {
        *self.object_ref.object_id()
    }
}

/// Execution-scoped authorization-plan entry that maps one runtime vertex to a concrete grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapVertexAuthorizationPlanEntry {
    #[serde(deserialize_with = "deserialize_tap_runtime_vertex")]
    pub vertex: RuntimeVertex,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub grant_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub tool_package: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_module: String,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_function: String,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub operation_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub constraints_commitment: Vec<u8>,
    #[serde(
        default,
        deserialize_with = "deserialize_move_option_interface_revision"
    )]
    pub endpoint_revision: Option<InterfaceRevision>,
    #[serde(default, deserialize_with = "deserialize_move_option_tap_address")]
    pub payment_id: Option<sui::types::Address>,
}

impl TapVertexAuthorizationPlanEntry {
    pub fn allowed_tool(&self) -> TapAuthorizedTool {
        TapAuthorizedTool {
            package_id: self.tool_package,
            module: self.tool_module.clone(),
            function: self.tool_function.clone(),
            operation_commitment: self.operation_commitment.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TapVertexAuthorizationPlan(pub Vec<TapVertexAuthorizationPlanEntry>);

impl TapVertexAuthorizationPlan {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn hash(&self) -> anyhow::Result<Vec<u8>> {
        Ok(Sha256::digest(bcs::to_bytes(&self.0)?).to_vec())
    }

    pub fn find_for_vertex(
        &self,
        vertex: &RuntimeVertex,
    ) -> anyhow::Result<Option<&TapVertexAuthorizationPlanEntry>> {
        let mut matches = self.0.iter().filter(|entry| &entry.vertex == vertex);
        let first = matches.next();
        if matches.next().is_some() {
            anyhow::bail!("duplicate TAP authorization-plan entries for vertex {vertex:?}");
        }
        Ok(first)
    }
}

/// Creation-time template for a scheduled TAP authorization grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledAuthorizationGrantTemplate {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub dag_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub vertex: String,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub tool_package: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_module: String,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_function: String,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub operation_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub constraints_commitment: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledAuthorizationGrantRef {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub scheduled_grant_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub dag_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub vertex: String,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub tool_package: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_module: String,
    #[serde(deserialize_with = "deserialize_tap_byte_string")]
    pub tool_function: String,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub operation_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub constraints_commitment: Vec<u8>,
    pub consumed: bool,
}

/// Scheduled peer of immediate skill execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledSkillTask {
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub id: sui::types::Address,
    #[serde(
        default = "default_tap_address",
        deserialize_with = "deserialize_tap_address_value_or_default"
    )]
    pub scheduler_task_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    #[serde(
        default,
        deserialize_with = "deserialize_move_option_interface_revision"
    )]
    pub pinned_revision: Option<InterfaceRevision>,
    /// Payment-source identity anchor from the Move scheduled-task record.
    /// This is not spendable custody by itself; durable occurrence refill
    /// needs an explicit custody/linking contract.
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub long_term_gas_coin_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub refill_policy_commitment: Vec<u8>,
    pub payment_source: TapScheduledPaymentSource,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub payment_source_bytes: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub payment_source_hash: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrence_budget: u64,
    #[serde(default, deserialize_with = "deserialize_tap_u64_value_or_default")]
    pub remaining_funds: u64,
    pub refund_mode: u8,
    pub schedule_policy: TapSchedulePolicy,
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub schedule_entries_commitment: Vec<u8>,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub next_after_ms: u64,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrences_spawned: u64,
    #[serde(default, deserialize_with = "deserialize_tap_u64_value_or_default")]
    pub occurrences_finalized: u64,
    #[serde(default)]
    pub in_flight: Vec<TapScheduledOccurrenceRecord>,
    #[serde(default)]
    pub scheduled_authorization_grants: Vec<TapScheduledAuthorizationGrantRef>,
    pub state: TapScheduledTaskState,
    pub active: bool,
}

impl TapScheduledSkillTask {
    pub fn scheduled_task_id(&self) -> sui::types::Address {
        self.id
    }

    pub fn source_kind(&self) -> TapPaymentSourceKind {
        self.payment_source.source_kind()
    }

    pub fn source_identity(&self) -> sui::types::Address {
        self.payment_source.source_identity()
    }

    pub fn can_spawn_occurrence(&self) -> bool {
        self.active
            && self.state == TapScheduledTaskState::Active
            && self.occurrence_budget > 0
            && self.remaining_funds >= self.occurrence_budget
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapScheduledOccurrenceRecord {
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub occurrence_index: u64,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub execution_id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_tap_address_value")]
    pub payment_id: sui::types::Address,
    pub interface_revision: InterfaceRevision,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub budget: u64,
    pub final_state: TapScheduledOccurrenceFinalState,
}

/// Registered skill plus the currently active endpoint used for fresh standard execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapActiveSkillExecutionTarget {
    pub skill: TapSkillRecord,
    pub endpoint: TapEndpointRecord,
}

/// Default execution target plus active endpoint recovered for fresh default DAG execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultDagExecutorRecord {
    pub target: DefaultDagExecutor,
    pub skill: TapSkillRecord,
    pub endpoint: TapEndpointRecord,
}

/// Digest input committed by endpoint announcements and publish artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapConfigDigestInput {
    pub interface_revision: InterfaceRevision,
    pub shared_objects: Vec<TapSharedObjectRef>,
    pub requirements: TapSkillRequirements,
}

#[derive(Serialize)]
struct TapConfigDigestInputBcs {
    interface_revision: InterfaceRevision,
    shared_objects: Vec<TapSharedObjectRef>,
    requirements: TapSkillRequirements,
}

impl TapConfigDigestInput {
    pub fn digest(&self) -> anyhow::Result<Vec<u8>> {
        let input = TapConfigDigestInputBcs {
            interface_revision: self.interface_revision,
            shared_objects: self.shared_objects.clone(),
            requirements: self.requirements.clone(),
        };
        let bytes = bcs::to_bytes(&input)?;
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
}

impl TapSkillConfig {
    pub fn digest_input(&self) -> TapConfigDigestInput {
        TapConfigDigestInput {
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

/// Published TAP plus DAG artifact used when binding an agent skill.
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
        // Substitute the `0x0` sentinel in `fixed_tools.package_id` with the
        // just-published `tap_package_id` so authors can declare self-referential
        // on-chain tools without a chicken-and-egg with the package address.
        let mut requirements = config.requirements.clone();
        for tool in &mut requirements.vertex_authorization_schema.fixed_tools {
            if tool.package_id == sui::types::Address::ZERO {
                tool.package_id = tap_package_id;
            }
        }
        let digest_input = TapConfigDigestInput {
            interface_revision: config.interface_revision,
            shared_objects: config.shared_objects.clone(),
            requirements: requirements.clone(),
        };
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
            requirements,
        })
    }

    pub fn endpoint_config_digest_input(&self) -> TapConfigDigestInput {
        TapConfigDigestInput {
            interface_revision: self.interface_revision,
            shared_objects: self.shared_objects.clone(),
            requirements: self.requirements.clone(),
        }
    }

    pub fn endpoint_config_digest(&self) -> anyhow::Result<Vec<u8>> {
        self.endpoint_config_digest_input().digest()
    }

    pub fn endpoint_config_digest_hex(&self) -> anyhow::Result<String> {
        self.endpoint_config_digest_input().digest_hex()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapValidationError {
    MissingSkillName,
    MissingTapPackageName,
    MissingDagPath,
    MissingTapPackagePath,
    MissingWorkflowCommitment,
    MissingRequirementsCommitment,
    MissingConfigDigest,
    EmptyAuthorizedToolModule,
    EmptyAuthorizedToolFunction,
    DuplicateAuthorizationPlanVertex,
    AuthorizationPlanCommitmentMismatch,
    AuthorizationPlanGrantMismatch,
    AuthorizationPlanToolNotAuthorized,
    AuthorizationPlanEndpointMismatch,
    AuthorizationPlanPaymentMismatch,
    AuthorizationGrantNotShared,
}

impl fmt::Display for TapValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapValidationError::MissingSkillName => write!(f, "skill name is required"),
            TapValidationError::MissingTapPackageName => write!(f, "TAP package name is required"),
            TapValidationError::MissingDagPath => write!(f, "DAG path is required"),
            TapValidationError::MissingTapPackagePath => write!(f, "TAP package path is required"),
            TapValidationError::MissingWorkflowCommitment => write!(f, "workflow hash is required"),
            TapValidationError::MissingRequirementsCommitment => {
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
            TapValidationError::DuplicateAuthorizationPlanVertex => {
                write!(
                    f,
                    "authorization plan contains duplicate runtime-vertex entries"
                )
            }
            TapValidationError::AuthorizationPlanCommitmentMismatch => {
                write!(f, "authorization plan hash does not match plan entries")
            }
            TapValidationError::AuthorizationPlanGrantMismatch => {
                write!(f, "authorization plan entry does not match fetched grant")
            }
            TapValidationError::AuthorizationPlanToolNotAuthorized => {
                write!(
                    f,
                    "authorization plan tool is not allowed by endpoint requirements"
                )
            }
            TapValidationError::AuthorizationPlanEndpointMismatch => {
                write!(
                    f,
                    "authorization plan endpoint revision does not match request context"
                )
            }
            TapValidationError::AuthorizationPlanPaymentMismatch => {
                write!(
                    f,
                    "authorization plan payment binding does not match request context"
                )
            }
            TapValidationError::AuthorizationGrantNotShared => {
                write!(f, "authorization grant object is not shared")
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
    if requirements.workflow_commitment.is_empty() {
        return Err(TapValidationError::MissingWorkflowCommitment);
    }
    if requirements.input_schema_commitment.is_empty() {
        return Err(TapValidationError::MissingRequirementsCommitment);
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

pub fn validate_authorization_plan(
    requirements: &TapSkillRequirements,
    plan: &TapVertexAuthorizationPlan,
    expected_hash: Option<&[u8]>,
) -> Result<(), TapValidationError> {
    if plan.is_empty() {
        return Ok(());
    }

    for (index, entry) in plan.0.iter().enumerate() {
        if plan.0[..index]
            .iter()
            .any(|prior| prior.vertex == entry.vertex)
        {
            return Err(TapValidationError::DuplicateAuthorizationPlanVertex);
        }
        let allowed = entry.allowed_tool();
        if !requirements
            .vertex_authorization_schema
            .fixed_tools
            .iter()
            .any(|tool| tool == &allowed)
        {
            return Err(TapValidationError::AuthorizationPlanToolNotAuthorized);
        }
    }

    if let Some(expected_hash) = expected_hash {
        let actual = plan
            .hash()
            .map_err(|_| TapValidationError::AuthorizationPlanCommitmentMismatch)?;
        if actual.as_slice() != expected_hash {
            return Err(TapValidationError::AuthorizationPlanCommitmentMismatch);
        }
    }

    Ok(())
}

pub fn validate_standard_tap_payment_options(
    agent_id: AgentId,
    policy: &TapPaymentPolicy,
    payment_source: &[u8],
    payment_max_budget: u64,
    payment_refund_mode: u8,
    payer: sui::types::Address,
) -> anyhow::Result<()> {
    if policy.max_budget != 0 && payment_max_budget > policy.max_budget {
        anyhow::bail!(
            "standard TAP payment budget {} exceeds endpoint policy max {}",
            payment_max_budget,
            policy.max_budget
        );
    }
    if payment_refund_mode != policy.refund_mode {
        anyhow::bail!(
            "standard TAP payment refund mode {} does not match endpoint policy {}",
            payment_refund_mode,
            policy.refund_mode
        );
    }
    match policy.mode {
        TapPaymentMode::UserFunded => {
            let expected = bcs::to_bytes(&payer)?;
            let source_is_valid =
                payment_source.is_empty() || payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard TAP user-funded payment source must be empty or payer address BCS"
                );
            }
        }
        TapPaymentMode::AgentFunded => {
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

pub fn tap_payment_source_for_address(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    Ok(bcs::to_bytes(&address)?)
}

pub fn tap_payment_source_for_invoker(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    TapPaymentSource::invoker(address).to_bcs_bytes()
}

pub fn tap_payment_source_for_agent_vault(agent_id: AgentId) -> anyhow::Result<Vec<u8>> {
    TapPaymentSource::agent_vault(agent_id).to_bcs_bytes()
}

/// Resolve exactly one active endpoint for fresh execution.
pub fn resolve_active_tap_endpoint<'a>(
    records: &'a [TapEndpointRecord],
    skills: &[TapSkillRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a TapEndpointRecord, TapEndpointResolutionError> {
    let skill_matches = skills
        .iter()
        .filter(|skill| {
            skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
        })
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => return Err(TapEndpointResolutionError::MissingActiveRevision { agent_id, skill_id }),
        [skill] if skill.active => *skill,
        [_] => {
            return Err(TapEndpointResolutionError::MissingActiveRevision { agent_id, skill_id })
        }
        _ => {
            return Err(TapEndpointResolutionError::DuplicateActiveRevision {
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

    let endpoint = registry.active_endpoint_record(agent_id, skill_id)?;

    Ok(TapActiveSkillExecutionTarget { skill, endpoint })
}

/// Resolve the configured default TAP DAG executor from registry state.
pub fn resolve_default_tap_dag_executor(
    registry: &TapRegistry,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    let target = registry.default_dag_executor()?;
    let execution_target =
        resolve_active_tap_skill_execution_target(registry, target.agent_id, target.skill_id)?;

    if execution_target.skill.dag_binding != TapDagBinding::RuntimeSelected {
        anyhow::bail!(
            "default TAP skill {} for agent {} is not runtime-DAG selected",
            target.skill_id,
            target.agent_id
        );
    }

    Ok(DefaultDagExecutorRecord {
        target,
        skill: execution_target.skill,
        endpoint: execution_target.endpoint,
    })
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    fn addr(value: &str) -> sui::types::Address {
        sui::types::Address::from_str(value).expect("valid address")
    }

    fn requirements() -> TapSkillRequirements {
        TapSkillRequirements {
            input_schema_commitment: vec![1],
            workflow_commitment: vec![2],
            metadata_commitment: vec![3],
            payment_policy: TapPaymentPolicy {
                max_budget: 100,
                ..TapPaymentPolicy::default()
            },
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        }
    }

    fn endpoint(revision: u64) -> TapEndpointRecord {
        TapEndpointRecord {
            key: TapEndpointKey {
                agent_id: addr("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(revision),
            },
            shared_objects: vec![TapSharedObjectRef::immutable(addr("0xd"))],
            config_digest: vec![9],
            requirements: requirements(),
        }
    }

    fn skill(active: bool, current_interface_revision: u64) -> TapSkillRecord {
        TapSkillRecord {
            agent_id: Some(addr("0xa")),
            skill_id: Some(11),
            description: vec![3],
            active,
            dag_binding: TapDagBinding::pinned(addr("0x44")),
            requirements: requirements(),
            current_interface_revision: InterfaceRevision(current_interface_revision),
            outstanding_scheduled_task_count: 0,
        }
    }

    fn registry_with_active_skill() -> TapRegistry {
        TapRegistry {
            id: addr("0xf"),
            agents: Vec::new(),
            skills: vec![skill(true, 2)],
            endpoints: vec![TapEndpointRevision {
                agent_id: addr("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(2),
                shared_objects: vec![TapSharedObjectRef::immutable(addr("0xd"))],
                requirements: requirements(),
                config_digest: vec![9],
            }],
            default_executor: None,
        }
    }

    #[test]
    fn config_digest_is_deterministic() {
        let input = TapConfigDigestInput {
            interface_revision: InterfaceRevision(3),
            shared_objects: vec![TapSharedObjectRef::mutable(addr("0x4"))],
            requirements: requirements(),
        };

        assert_eq!(input.digest().unwrap(), input.digest().unwrap());
        assert_eq!(input.digest_hex().unwrap().len(), 64);
    }

    #[test]
    fn validate_rejects_missing_requirements_commitment() {
        let mut requirements = requirements();
        requirements.input_schema_commitment.clear();

        assert_eq!(
            validate_requirements(&requirements),
            Err(TapValidationError::MissingRequirementsCommitment)
        );
    }

    #[test]
    fn active_resolution_requires_exactly_one_active_revision() {
        let active = endpoint(1);
        let inactive = endpoint(2);
        let records = vec![active.clone(), inactive];
        let skills = vec![skill(true, 1)];

        let resolved = resolve_active_tap_endpoint(
            &records,
            &skills,
            active.key.agent_id,
            active.key.skill_id,
        )
        .expect("one active endpoint");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

        let duplicate = vec![endpoint(1), endpoint(1)];
        assert!(matches!(
            resolve_active_tap_endpoint(&duplicate, &skills, addr("0xa"), 11),
            Err(TapEndpointResolutionError::DuplicateActiveRevision { count: 2, .. })
        ));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn agent_registry_object_bcs_decodes_without_inline_default_executor() {
        #[derive(Serialize)]
        struct RawTapRegistryObjectBcs {
            id: sui::types::Address,
            agents: MoveTable<sui::types::Address, TapAgentRecord>,
            endpoints: MoveTable<TapEndpointRevisionKey, TapEndpointRevision>,
        }

        let raw = RawTapRegistryObjectBcs {
            id: addr("0xf"),
            agents: MoveTable::new(addr("0x90"), 0),
            endpoints: MoveTable::new(addr("0x91"), 0),
        };
        let bytes = bcs::to_bytes(&raw).expect("raw Move registry BCS should encode");
        let decoded: TapRegistryObject =
            bcs::from_bytes(&bytes).expect("raw Move registry BCS should decode");

        assert_eq!(decoded.id, addr("0xf"));
        assert_eq!(decoded.agents.id, addr("0x90"));
        assert_eq!(decoded.endpoints.id, addr("0x91"));
    }

    #[test]
    fn publish_artifact_contains_digest_and_onchain_package_ids() {
        let config = TapSkillConfig {
            name: "weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: requirements(),
            shared_objects: vec![TapSharedObjectRef::immutable(addr("0x9"))],
            interface_revision: InterfaceRevision(1),
        };

        let artifact = TapPublishArtifact::from_config(&config, addr("0x8"), addr("0x7"))
            .expect("valid artifact");

        assert_eq!(artifact.dag_id, addr("0x8"));
        assert_eq!(artifact.tap_package_id, addr("0x7"));
        assert_eq!(artifact.config_digest_hex.len(), 64);
    }

    #[test]
    fn vertex_authorization_schema_is_default_predicate() {
        // is_default toggles which register entrypoint the SDK takes. Both
        // populated fixed_tools and a true requires_payment flag must flip it
        // false so the cap-gated path runs; the empty/!requires_payment shape
        // is what the chain reconstructs on the simple register entrypoint.
        let mut schema = TapVertexAuthorizationSchema::default();
        assert!(schema.is_default());

        schema.requires_payment = true;
        assert!(!schema.is_default());

        schema.requires_payment = false;
        schema.fixed_tools = vec![TapAuthorizedTool {
            package_id: addr("0x42"),
            module: "tap".to_string(),
            function: "execute".to_string(),
            operation_commitment: vec![1, 2, 3],
        }];
        assert!(!schema.is_default());
    }

    #[test]
    fn publish_artifact_substitutes_zero_sentinel_fixed_tool_package_id() {
        // `from_config` rewrites any fixed_tools[].package_id == 0x0 into the
        // just-published tap_package_id so authors can declare self-referential
        // cap-gated tools without knowing the package address ahead of time.
        // Crucially, the rewrite must propagate into the config digest input so
        // the on-chain digest validation sees the same shape; otherwise
        // register_skill rejects the artifact with a digest mismatch.
        let mut reqs = requirements();
        reqs.vertex_authorization_schema = TapVertexAuthorizationSchema {
            schema_commitment: vec![],
            fixed_tools: vec![
                TapAuthorizedTool {
                    package_id: sui::types::Address::ZERO,
                    module: "tap".to_string(),
                    function: "execute_self".to_string(),
                    operation_commitment: vec![],
                },
                TapAuthorizedTool {
                    package_id: addr("0x99"),
                    module: "external".to_string(),
                    function: "call".to_string(),
                    operation_commitment: vec![],
                },
            ],
            requires_payment: false,
        };
        let config = TapSkillConfig {
            name: "self".to_string(),
            tap_package_name: "self_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: reqs,
            shared_objects: vec![],
            interface_revision: InterfaceRevision(1),
        };

        let dag_id = addr("0x8");
        let tap_pkg = addr("0x7");
        let artifact =
            TapPublishArtifact::from_config(&config, dag_id, tap_pkg).expect("valid artifact");

        let tools = &artifact
            .requirements
            .vertex_authorization_schema
            .fixed_tools;
        assert_eq!(tools[0].package_id, tap_pkg, "0x0 sentinel must rewrite");
        assert_eq!(
            tools[1].package_id,
            addr("0x99"),
            "non-sentinel entries must be left alone"
        );

        // The digest must be computed after substitution so on-chain validation
        // agrees with what the artifact carries.
        let mut substituted_reqs = config.requirements.clone();
        substituted_reqs.vertex_authorization_schema.fixed_tools[0].package_id = tap_pkg;
        let expected_digest = TapConfigDigestInput {
            interface_revision: InterfaceRevision(1),
            shared_objects: vec![],
            requirements: substituted_reqs,
        }
        .digest()
        .expect("expected digest");
        assert_eq!(artifact.config_digest, expected_digest);
    }

    #[test]
    fn publish_artifact_builds_revision_digest_input() {
        let config = TapSkillConfig {
            name: "weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: requirements(),
            shared_objects: vec![TapSharedObjectRef::immutable(addr("0x9"))],
            interface_revision: InterfaceRevision(1),
        };
        let artifact = TapPublishArtifact::from_config(&config, addr("0x8"), addr("0x7"))
            .expect("valid artifact");

        let input = artifact.endpoint_config_digest_input();
        let endpoint_digest = artifact.endpoint_config_digest().expect("endpoint digest");

        assert_eq!(input.interface_revision, InterfaceRevision(1));
        assert_eq!(
            artifact
                .endpoint_config_digest_hex()
                .expect("endpoint digest hex")
                .len(),
            64
        );
        assert_eq!(endpoint_digest, artifact.config_digest);
    }

    #[test]
    fn validate_standard_tap_payment_options_enforces_user_funded_policy() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let explicit_source = tap_payment_source_for_address(payer).expect("payer source");
        let typed_source = tap_payment_source_for_invoker(payer).expect("typed payer source");
        let other_source = tap_payment_source_for_address(addr("0x2")).expect("other source");
        let policy = TapPaymentPolicy {
            max_budget: 100,
            ..TapPaymentPolicy::default()
        };

        validate_standard_tap_payment_options(agent, &policy, &[], 100, 0, payer)
            .expect("implicit payer source");
        validate_standard_tap_payment_options(agent, &policy, &explicit_source, 100, 0, payer)
            .expect("explicit payer source");
        assert!(
            validate_standard_tap_payment_options(agent, &policy, &typed_source, 100, 0, payer,)
                .is_err(),
            "typed invoker sources are not accepted by Move direct user-funded policy"
        );
        assert!(validate_standard_tap_payment_options(
            agent,
            &policy,
            &other_source,
            100,
            0,
            payer,
        )
        .is_err());
        assert!(validate_standard_tap_payment_options(agent, &policy, &[], 101, 0, payer).is_err());
        assert!(validate_standard_tap_payment_options(agent, &policy, &[], 100, 9, payer).is_err());
    }

    #[test]
    fn validate_standard_tap_payment_options_enforces_source_modes() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let legacy_agent_source = tap_payment_source_for_address(agent).expect("agent source");
        let agent_source = tap_payment_source_for_agent_vault(agent).expect("agent vault source");

        let agent_funded = TapPaymentPolicy {
            mode: TapPaymentMode::AgentFunded,
            ..TapPaymentPolicy::default()
        };
        assert!(
            validate_standard_tap_payment_options(
                agent,
                &agent_funded,
                &agent_source,
                0,
                0,
                payer,
            )
            .is_err(),
            "typed agent-vault sources are not accepted by Move direct agent-funded policy"
        );
        validate_standard_tap_payment_options(
            agent,
            &agent_funded,
            &legacy_agent_source,
            0,
            0,
            payer,
        )
        .expect("legacy agent-funded source");
        assert!(
            validate_standard_tap_payment_options(agent, &agent_funded, &[], 0, 0, payer,).is_err()
        );
        assert!(validate_standard_tap_payment_options(
            agent,
            &agent_funded,
            &agent_source,
            0,
            0,
            payer,
        )
        .is_err());
    }

    #[test]
    fn removed_tap_payment_modes_do_not_deserialize() {
        for mode in ["hybrid", "Hybrid", "sponsored", "Sponsored"] {
            let value = serde_json::json!(mode);
            assert!(serde_json::from_value::<TapPaymentMode>(value).is_err());
        }
    }

    #[test]
    fn tap_enum_deserializers_accept_move_json_forms() {
        assert_eq!(
            serde_json::from_value::<TapPaymentMode>(serde_json::json!({
                "fields": { "variant": "agentFunded" }
            }))
            .expect("nested payment mode"),
            TapPaymentMode::AgentFunded
        );
        assert_eq!(
            serde_json::from_value::<TapPaymentMode>(serde_json::json!({
                "UserFunded": {}
            }))
            .expect("keyed payment mode"),
            TapPaymentMode::UserFunded
        );
        assert_eq!(
            serde_json::from_value::<TapPaymentSourceKind>(serde_json::json!(1))
                .expect("numeric payment source kind"),
            TapPaymentSourceKind::AgentVault
        );
        assert_eq!(
            serde_json::from_value::<TapPaymentSourceKind>(serde_json::json!({
                "fields": { "@variant": "invoker" }
            }))
            .expect("nested payment source kind"),
            TapPaymentSourceKind::Invoker
        );
        assert!(serde_json::from_value::<TapPaymentSourceKind>(serde_json::json!(7)).is_err());

        assert_eq!(
            serde_json::from_value::<TapVertexExecutionPaymentSettlementKind>(serde_json::json!({
                "Paid": {}
            }))
            .expect("keyed settlement kind"),
            TapVertexExecutionPaymentSettlementKind::Paid
        );
        assert_eq!(
            serde_json::from_value::<TapVertexExecutionPaymentSettlementKind>(serde_json::json!({
                "fields": { "type": "Ticket" }
            }))
            .expect("nested settlement kind"),
            TapVertexExecutionPaymentSettlementKind::Ticket
        );
        assert_eq!(
            bcs::from_bytes::<TapVertexExecutionPaymentSettlementKind>(
                &bcs::to_bytes(&9_u8).expect("raw settlement kind")
            )
            .expect("unknown raw settlement kind falls back"),
            TapVertexExecutionPaymentSettlementKind::Paid
        );

        assert_eq!(
            serde_json::from_value::<TapExecutionPaymentFinalState>(serde_json::json!({
                "fields": { "variant": "Accomplished" }
            }))
            .expect("nested payment final state"),
            TapExecutionPaymentFinalState::Accomplished
        );
        assert_eq!(
            serde_json::from_value::<TapScheduledTaskState>(serde_json::json!({
                "Exhausted": {}
            }))
            .expect("keyed scheduled task state"),
            TapScheduledTaskState::Exhausted
        );
        assert_eq!(
            serde_json::from_value::<TapScheduledOccurrenceFinalState>(serde_json::json!({
                "fields": { "@variant": "inFlight" }
            }))
            .expect("nested scheduled occurrence state"),
            TapScheduledOccurrenceFinalState::InFlight
        );
    }

    #[test]
    fn scheduled_payment_source_deserializes_supported_shapes() {
        let address_source: TapScheduledPaymentSource = serde_json::from_value(serde_json::json!({
            "fields": {
                "@variant": "address",
                "refund_recipient": "0xee"
            }
        }))
        .expect("variant address source");
        assert_eq!(address_source.source_kind(), TapPaymentSourceKind::Invoker);
        assert_eq!(address_source.source_identity(), addr("0xee"));

        let vault_source: TapScheduledPaymentSource = serde_json::from_value(serde_json::json!({
            "agentVault": {
                "fields": {
                    "agent_id": "0xaa"
                }
            }
        }))
        .expect("nested vault source");
        assert_eq!(vault_source.source_kind(), TapPaymentSourceKind::AgentVault);
        assert_eq!(vault_source.source_identity(), addr("0xaa"));

        assert!(
            serde_json::from_value::<TapScheduledPaymentSource>(serde_json::json!({
                "fields": { "@variant": "agentVault" }
            }))
            .is_err()
        );
    }

    #[test]
    fn tap_byte_string_deserializes_hex_utf8_and_plain_text() {
        let entry: TapVertexAuthorizationPlanEntry = serde_json::from_value(serde_json::json!({
            "vertex": [101, 110, 116, 114, 121],
            "grant_id": "0xaa",
            "tool_package": "0x14",
            "tool_module": "0x746f6f6c",
            "tool_function": "0x72756e",
            "operation_commitment": [1, 2],
            "constraints_commitment": [3, 4]
        }))
        .expect("hex byte strings decode as UTF-8");

        assert_eq!(entry.tool_module, "tool");
        assert_eq!(entry.tool_function, "run");

        let entry: TapVertexAuthorizationPlanEntry = serde_json::from_value(serde_json::json!({
            "vertex": [101, 110, 116, 114, 121],
            "grant_id": "0xaa",
            "tool_package": "0x14",
            "tool_module": "0xnothex",
            "tool_function": "run",
            "operation_commitment": [1, 2],
            "constraints_commitment": [3, 4]
        }))
        .expect("plain byte string remains text");

        assert_eq!(entry.tool_module, "0xnothex");
    }

    #[test]
    fn tap_execution_payment_deserializes_move_json_byte_vectors() {
        use base64::Engine as _;

        let source_hash = Sha256::digest(b"payer").to_vec();
        let payment: TapExecutionPayment = serde_json::from_value(serde_json::json!({
            "id": "0x10",
            "execution_id": "0x11",
            "agent_id": "0x12",
            "skill_id": "19",
            "interface_revision": "1",
            "payer": "0x15",
            "payment_mode": "user_funded",
            "source_kind": "agent_vault",
            "source_identity": "0x12",
            "max_budget": "100",
            "locked_budget": "80",
            "consumed": "0",
            "refund_mode": 0,
            "payment_source_hash": base64::engine::general_purpose::STANDARD.encode(&source_hash),
            "accomplished": false,
            "refunded": false,
            "final_state": "pending",
            "locked_vertices": [
                {
                    "vertex_key": [1, 2],
                    "tool_fqn": base64::engine::general_purpose::STANDARD.encode(b"tool"),
                    "amount": "7",
                    "settlement_kind": "paid"
                },
                {
                    "vertex_key": [3],
                    "tool_fqn": [116, 105, 99, 107, 101, 116],
                    "amount": "0",
                    "settlement_kind": "ticket"
                }
            ]
        }))
        .expect("payment parses base64 and hex byte-vector forms");

        assert_eq!(payment.payment_source_hash, source_hash);
        assert_eq!(payment.source_kind, Some(TapPaymentSourceKind::AgentVault));
        assert_eq!(payment.source_identity, Some(addr("0x12")));
        assert_eq!(payment.locked_budget, 80);
        assert_eq!(
            payment.final_state,
            Some(TapExecutionPaymentFinalState::Pending)
        );
        assert_eq!(payment.outstanding_locks(), 2);
        assert_eq!(
            payment.locked_vertices[0].settlement_kind,
            TapVertexExecutionPaymentSettlementKind::Paid
        );
        assert_eq!(
            payment.locked_vertices[1].settlement_kind,
            TapVertexExecutionPaymentSettlementKind::Ticket
        );
    }

    #[test]
    fn tap_execution_payment_deserializes_move_json_payment_mode() {
        let payment: TapExecutionPayment = serde_json::from_value(serde_json::json!({
            "id": "0x10",
            "execution_id": "0x11",
            "agent_id": "0x12",
            "skill_id": "19",
            "interface_revision": "1",
            "payer": "0x15",
            "payment_mode": {"@variant": "user_funded"},
            "max_budget": "100",
            "consumed": "0",
            "refund_mode": 0,
            "payment_source_hash": [],
            "accomplished": false,
            "refunded": false
        }))
        .expect("payment parses Move enum JSON form");

        assert_eq!(payment.payment_mode, TapPaymentMode::UserFunded);
    }

    #[test]
    fn tap_payment_source_bcs_roundtrips_and_rejects_unknown_kind() {
        let invoker = addr("0x21");
        let typed = TapPaymentSource::from_bcs_bytes(
            &tap_payment_source_for_invoker(invoker).expect("typed invoker source"),
        )
        .expect("typed invoker source decodes");
        assert_eq!(typed.kind, TapPaymentSourceKind::Invoker);
        assert_eq!(typed.identity, invoker);

        let agent = addr("0x22");
        let typed = TapPaymentSource::from_bcs_bytes(
            &tap_payment_source_for_agent_vault(agent).expect("typed vault source"),
        )
        .expect("typed vault source decodes");
        assert_eq!(typed.kind, TapPaymentSourceKind::AgentVault);
        assert_eq!(typed.identity, agent);

        let invalid = bcs::to_bytes(&(9_u8, invoker)).expect("invalid source kind bytes");
        assert!(TapPaymentSource::from_bcs_bytes(&invalid).is_err());
    }

    fn authorization_plan_entry(vertex: RuntimeVertex) -> TapVertexAuthorizationPlanEntry {
        TapVertexAuthorizationPlanEntry {
            vertex,
            grant_id: addr("0x30"),
            tool_package: addr("0x40"),
            tool_module: "tool".to_string(),
            tool_function: "run".to_string(),
            operation_commitment: vec![7],
            constraints_commitment: vec![8],
            endpoint_revision: Some(InterfaceRevision(2)),
            payment_id: Some(addr("0x60")),
        }
    }

    #[test]
    fn authorization_plan_commitment_and_lookup_are_stable() {
        let vertex = RuntimeVertex::plain("entry");
        let entry = authorization_plan_entry(vertex.clone());
        let plan = TapVertexAuthorizationPlan(vec![entry.clone()]);

        assert_eq!(plan.find_for_vertex(&vertex).unwrap(), Some(&entry));
        assert!(plan
            .find_for_vertex(&RuntimeVertex::plain("other"))
            .unwrap()
            .is_none());
        assert_eq!(plan.hash().unwrap(), plan.hash().unwrap());
    }

    #[test]
    fn authorization_plan_validation_rejects_duplicate_and_unlisted_tools() {
        let vertex = RuntimeVertex::plain("entry");
        let entry = authorization_plan_entry(vertex.clone());
        let mut requirements = requirements();
        requirements.vertex_authorization_schema.fixed_tools = vec![entry.allowed_tool()];
        let plan = TapVertexAuthorizationPlan(vec![entry.clone()]);
        let hash = plan.hash().unwrap();

        validate_authorization_plan(&requirements, &plan, Some(&hash)).expect("valid plan");
        assert_eq!(
            validate_authorization_plan(&requirements, &plan, Some(&[0])).unwrap_err(),
            TapValidationError::AuthorizationPlanCommitmentMismatch
        );
        validate_authorization_plan(
            &requirements,
            &TapVertexAuthorizationPlan::default(),
            Some(&[9, 8]),
        )
        .expect("hash-only contexts without concrete plan entries remain tolerated");
        assert_eq!(
            validate_authorization_plan(
                &requirements,
                &TapVertexAuthorizationPlan(vec![entry.clone(), entry.clone()]),
                None,
            )
            .unwrap_err(),
            TapValidationError::DuplicateAuthorizationPlanVertex
        );

        requirements.vertex_authorization_schema.fixed_tools.clear();
        assert_eq!(
            validate_authorization_plan(&requirements, &plan, None).unwrap_err(),
            TapValidationError::AuthorizationPlanToolNotAuthorized
        );
    }

    #[test]
    fn active_skill_execution_target_requires_one_active_skill_and_endpoint() {
        let registry = registry_with_active_skill();
        let resolved = resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11)
            .expect("active skill target");

        assert_eq!(
            resolved.skill.dag_binding,
            TapDagBinding::pinned(addr("0x44"))
        );
        assert_eq!(
            resolved.endpoint.key.interface_revision,
            InterfaceRevision(2)
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

        registry.skills[0].dag_binding = TapDagBinding::runtime_selected();
        let target = resolve_default_tap_dag_executor(&registry)
            .expect("runtime-selected default skill resolves");

        assert_eq!(
            target.target,
            DefaultDagExecutor {
                agent_id: addr("0xa"),
                skill_id: 11,
            }
        );
        assert_eq!(target.skill.dag_binding, TapDagBinding::RuntimeSelected);
    }

    #[test]
    fn active_skill_execution_target_rejects_missing_skill() {
        let mut registry = registry_with_active_skill();
        registry.skills.clear();

        assert!(resolve_active_tap_skill_execution_target(&registry, addr("0xa"), 11).is_err());
    }

    #[test]
    fn workflow_vertex_authorization_grant_model_matches_live_object_shape() {
        let grant: WorkflowVertexAuthorizationGrant = serde_json::from_value(serde_json::json!({
            "id": "0xaa",
            "execution_id": "0xff",
            "vertex": [101, 110, 116, 114, 121],
            "scheduled_grant_id": { "fields": { "vec": ["0x77"] } }
        }))
        .expect("grant should deserialize");

        assert_eq!(grant.id, addr("0xaa"));
        assert_eq!(grant.execution_id, addr("0xff"));
        assert_eq!(grant.vertex, RuntimeVertex::plain("entry"));
        assert_eq!(grant.scheduled_grant_id, Some(addr("0x77")));
    }

    #[test]
    fn scheduled_skill_task_model_matches_live_object_shape() {
        let source_hash = Sha256::digest(bcs::to_bytes(&addr("0xee")).unwrap()).to_vec();
        let task: TapScheduledSkillTask = serde_json::from_value(serde_json::json!({
            "id": "0xaa",
            "scheduler_task_id": "0xab",
            "agent_id": "0xbb",
            "skill_id": "204",
            "pinned_revision": { "fields": { "vec": [{ "fields": { "value": "9" } }] } },
            "long_term_gas_coin_id": "0xee",
            "refill_policy_commitment": [3, 4],
            "payment_source": { "Address": { "refund_recipient": "0xee" } },
            "payment_source_bytes": bcs::to_bytes(&addr("0xee")).unwrap(),
            "payment_source_hash": source_hash,
            "occurrence_budget": "25",
            "remaining_funds": { "value": "50" },
            "refund_mode": 0,
            "schedule_policy": {
                "recurrence_kind": "once",
                "min_interval_ms": "0",
                "max_occurrences": "3",
                "allow_recursive": false
            },
            "schedule_entries_commitment": [7, 8],
            "next_after_ms": "11",
            "occurrences_spawned": "2",
            "occurrences_finalized": "1",
            "in_flight": [{
                "occurrence_index": "1",
                "execution_id": "0xf1",
                "payment_id": "0xf2",
                "interface_revision": "9",
                "budget": "25",
                "final_state": "refunded"
            }],
            "scheduled_authorization_grants": [{
                "scheduled_grant_id": "0x91",
                "scheduled_task_id": "0xaa",
                "agent_id": "0xbb",
                "skill_id": "204",
                "dag_id": "0xda6",
                "vertex": [101, 110, 116, 114, 121],
                "tool_package": "0x1234",
                "tool_module": [116, 111, 111, 108],
                "tool_function": [101, 120, 101, 99, 117, 116, 101],
                "operation_commitment": [1, 2],
                "constraints_commitment": [],
                "consumed": false
            }],
            "state": "active",
            "active": true
        }))
        .expect("scheduled task should deserialize");

        assert_eq!(task.scheduled_task_id(), addr("0xaa"));
        assert_eq!(task.scheduler_task_id, addr("0xab"));
        assert_eq!(task.agent_id, addr("0xbb"));
        assert_eq!(task.skill_id, 204);
        assert_eq!(task.pinned_revision, Some(InterfaceRevision(9)));
        assert_eq!(task.source_kind(), TapPaymentSourceKind::Invoker);
        assert_eq!(task.source_identity(), addr("0xee"));
        assert_eq!(task.occurrence_budget, 25);
        assert_eq!(task.remaining_funds, 50);
        assert_eq!(task.occurrences_finalized, 1);
        assert_eq!(task.in_flight.len(), 1);
        assert_eq!(
            task.in_flight[0].final_state,
            TapScheduledOccurrenceFinalState::Refunded
        );
        assert_eq!(task.scheduled_authorization_grants.len(), 1);
        assert_eq!(
            task.scheduled_authorization_grants[0].scheduled_grant_id,
            addr("0x91")
        );
        assert_eq!(task.scheduled_authorization_grants[0].vertex, "entry");
        assert_eq!(task.scheduled_authorization_grants[0].tool_module, "tool");
        assert!(task.can_spawn_occurrence());
        assert_eq!(task.next_after_ms, 11);
        assert_eq!(task.occurrences_spawned, 2);
        assert!(task.active);
    }

    #[test]
    fn scheduled_skill_task_defaults_and_spawn_checks_are_conservative() {
        let task: TapScheduledSkillTask = serde_json::from_value(serde_json::json!({
            "id": "0xaa",
            "agent_id": "0xbb",
            "skill_id": "204",
            "long_term_gas_coin_id": "0xee",
            "refill_policy_commitment": [3, 4],
            "payment_source": { "agent_vault": { "agent_id": "0xbb" } },
            "payment_source_bytes": [9],
            "payment_source_hash": [8],
            "occurrence_budget": "25",
            "refund_mode": 0,
            "schedule_policy": {
                "recurrence_kind": "once",
                "min_interval_ms": "0",
                "max_occurrences": "3",
                "allow_recursive": false
            },
            "schedule_entries_commitment": [7, 8],
            "next_after_ms": "11",
            "occurrences_spawned": "2",
            "state": { "Completed": {} },
            "active": true
        }))
        .expect("scheduled task with defaults should deserialize");

        assert_eq!(task.scheduler_task_id, sui::types::Address::ZERO);
        assert_eq!(task.remaining_funds, 0);
        assert_eq!(task.occurrences_finalized, 0);
        assert_eq!(task.source_kind(), TapPaymentSourceKind::AgentVault);
        assert_eq!(task.source_identity(), addr("0xbb"));
        assert!(!task.can_spawn_occurrence());
    }
}
