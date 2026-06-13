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

/// TAP skill interface revision used for fresh lookup and in-flight pinning.
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

/// Key for a pinned skill interface revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TapSkillRevisionKey {
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
    pub interface_revision: InterfaceRevision,
}

impl fmt::Display for TapSkillRevisionKey {
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
    #[serde(deserialize_with = "deserialize_tap_u64_value")]
    pub skill_id: SkillId,
}

impl fmt::Display for TapWorksheetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.agent_id, self.skill_id)
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum TapPaymentPolicy {
    UserFunded,
    AgentFunded { max_budget: u64 },
}

impl<'de> Deserialize<'de> for TapPaymentPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum RawTapPaymentPolicy {
                UserFunded,
                AgentFunded { max_budget: u64 },
            }

            return RawTapPaymentPolicy::deserialize(deserializer).map(|raw| match raw {
                RawTapPaymentPolicy::UserFunded => TapPaymentPolicy::UserFunded,
                RawTapPaymentPolicy::AgentFunded { max_budget } => {
                    TapPaymentPolicy::AgentFunded { max_budget }
                }
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_payment_policy_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP payment policy value"))
    }
}

fn deserialize_tap_payment_policy_value(value: &serde_json::Value) -> Option<TapPaymentPolicy> {
    fn variant_text(object: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
        ["@variant", "variant", "type"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
    }

    fn parse_agent_funded(fields: Option<&serde_json::Value>) -> Option<TapPaymentPolicy> {
        let max_budget = fields
            .and_then(|fields| match fields {
                serde_json::Value::Object(object) => object.get("max_budget"),
                other => Some(other),
            })
            .and_then(|value| super::parse_u64_value(value).ok().flatten())?;
        Some(TapPaymentPolicy::AgentFunded { max_budget })
    }

    match value {
        serde_json::Value::String(text) if text == "UserFunded" || text == "user_funded" => {
            Some(TapPaymentPolicy::UserFunded)
        }
        serde_json::Value::Object(object) => {
            if let Some(text) = variant_text(object) {
                return match text {
                    "UserFunded" | "user_funded" => Some(TapPaymentPolicy::UserFunded),
                    "AgentFunded" | "agent_funded" => {
                        parse_agent_funded(object.get("fields").or(Some(value)))
                    }
                    _ => None,
                };
            }

            if object.contains_key("UserFunded") {
                return Some(TapPaymentPolicy::UserFunded);
            }
            if let Some(fields) = object.get("AgentFunded") {
                return parse_agent_funded(Some(fields));
            }
            if object.contains_key("max_budget") {
                return parse_agent_funded(Some(value));
            }
            None
        }
        _ => None,
    }
}

impl TapPaymentPolicy {
    pub fn user_funded() -> Self {
        Self::UserFunded
    }

    pub fn agent_funded(max_budget: u64) -> Self {
        Self::AgentFunded { max_budget }
    }

    pub fn mode(&self) -> TapPaymentMode {
        match self {
            Self::UserFunded => TapPaymentMode::UserFunded,
            Self::AgentFunded { .. } => TapPaymentMode::AgentFunded,
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
        Self::UserFunded
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

/// TAP-facing recurrence mode for a scheduled skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum TapRecurrenceKind {
    Once,
    Recursive {
        min_interval_ms: u64,
        max_occurrences: Option<u64>,
    },
}

impl<'de> Deserialize<'de> for TapRecurrenceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum RawTapRecurrenceKind {
                Once,
                Recursive {
                    min_interval_ms: u64,
                    max_occurrences: Option<u64>,
                },
            }

            return RawTapRecurrenceKind::deserialize(deserializer).map(|raw| match raw {
                RawTapRecurrenceKind::Once => TapRecurrenceKind::Once,
                RawTapRecurrenceKind::Recursive {
                    min_interval_ms,
                    max_occurrences,
                } => TapRecurrenceKind::Recursive {
                    min_interval_ms,
                    max_occurrences,
                },
            });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        deserialize_tap_recurrence_kind_value(&value)
            .ok_or_else(|| D::Error::custom("missing TAP recurrence kind value"))
    }
}

fn deserialize_tap_recurrence_kind_value(value: &serde_json::Value) -> Option<TapRecurrenceKind> {
    fn variant_text(object: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
        ["@variant", "variant", "type"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(serde_json::Value::as_str))
    }

    fn parse_recursive(fields: Option<&serde_json::Value>) -> Option<TapRecurrenceKind> {
        let serde_json::Value::Object(object) = fields? else {
            return None;
        };
        let min_interval_ms = object
            .get("min_interval_ms")
            .and_then(|value| super::parse_u64_value(value).ok().flatten())?;
        let max_occurrences = object
            .get("max_occurrences")
            .and_then(|value| {
                crate::types::deserialize_move_option_sui_u64(value.clone())
                    .ok()
                    .map(|value| value.0)
            })
            .flatten();
        Some(TapRecurrenceKind::Recursive {
            min_interval_ms,
            max_occurrences,
        })
    }

    match value {
        serde_json::Value::String(text) if text == "Once" || text == "once" => {
            Some(TapRecurrenceKind::Once)
        }
        serde_json::Value::Object(object) => {
            if let Some(text) = variant_text(object) {
                return match text {
                    "Once" | "once" => Some(TapRecurrenceKind::Once),
                    "Recursive" | "recursive" => {
                        parse_recursive(object.get("fields").or(Some(value)))
                    }
                    _ => None,
                };
            }

            if object.contains_key("Once") {
                return Some(TapRecurrenceKind::Once);
            }
            if let Some(fields) = object.get("Recursive") {
                return parse_recursive(Some(fields));
            }
            if object.contains_key("min_interval_ms") {
                return parse_recursive(Some(value));
            }
            None
        }
        _ => None,
    }
}

/// TAP-facing schedule policy summary used by dry-run checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSchedulePolicy {
    pub recurrence: TapRecurrenceKind,
    pub allow_recursive: bool,
}

impl Default for TapSchedulePolicy {
    fn default() -> Self {
        Self {
            recurrence: TapRecurrenceKind::Once,
            allow_recursive: false,
        }
    }
}

/// Fixed tool that must be preserved by a skill DAG.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapFixedTool {
    pub tool_registry_id: sui::types::Address,
    #[serde(
        alias = "fqn",
        deserialize_with = "crate::types::deserialize_move_ascii_string",
        serialize_with = "crate::types::serialize_move_ascii_string"
    )]
    pub tool_fqn: String,
}

/// User-facing skill requirements fetched before dry-run or execution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillRequirements {
    #[serde(deserialize_with = "deserialize_tap_byte_vector")]
    pub input_schema_commitment: Vec<u8>,
    pub payment_policy: TapPaymentPolicy,
    pub schedule_policy: TapSchedulePolicy,
    #[serde(default)]
    pub fixed_tools: Vec<TapFixedTool>,
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
}

/// Expanded `nexus_registry::agent_registry::AgentRegistry` contents with table entries fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapRegistry {
    pub id: sui::types::Address,
    pub agents: Vec<TapAgentRecord>,
    pub skills: Vec<TapSkillRecord>,
    #[serde(default)]
    pub default_executor: Option<DefaultDagExecutor>,
}

impl TapRegistry {
    /// Convert current skill revisions into leader-facing skill revision records.
    pub fn skill_revision_records(&self) -> anyhow::Result<Vec<TapSkillRevisionRecord>> {
        self.skills
            .iter()
            .filter_map(TapSkillRevisionRecord::from_skill_record)
            .map(|record| {
                record.validate()?;
                Ok(record)
            })
            .collect()
    }

    pub fn skill_revision_record(
        &self,
        key: TapSkillRevisionKey,
    ) -> anyhow::Result<TapSkillRevisionRecord> {
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
    ) -> anyhow::Result<TapSkillRevisionRecord> {
        let skills = self
            .skills
            .iter()
            .filter(|skill| {
                skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
            })
            .collect::<Vec<_>>();

        let skill = match skills.as_slice() {
            [] => {
                return Err(TapSkillRevisionResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            [skill] if skill.active => *skill,
            [_] => {
                return Err(TapSkillRevisionResolutionError::MissingActiveRevision {
                    agent_id,
                    skill_id,
                }
                .into())
            }
            _ => {
                return Err(TapSkillRevisionResolutionError::DuplicateActiveRevision {
                    agent_id,
                    skill_id,
                    count: skills.len(),
                }
                .into())
            }
        };

        let skill_revision = TapSkillRevisionRecord {
            key: TapSkillRevisionKey {
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
pub struct TapSkillRevisionRecord {
    pub key: TapSkillRevisionKey,
    pub requirements: TapSkillRequirements,
}

impl TapSkillRevisionRecord {
    fn from_skill_record(skill: &TapSkillRecord) -> Option<Self> {
        let agent_id = skill.agent_id?;
        let skill_id = skill.skill_id?;
        Some(Self {
            key: TapSkillRevisionKey {
                agent_id,
                skill_id,
                interface_revision: skill.current_interface_revision,
            },
            requirements: skill.requirements.clone(),
        })
    }

    pub fn worksheet_key(&self) -> TapWorksheetKey {
        TapWorksheetKey {
            agent_id: self.key.agent_id,
            skill_id: self.key.skill_id,
        }
    }

    pub fn validate(&self) -> Result<(), TapValidationError> {
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

    pub fn skill_revision_key(&self) -> TapSkillRevisionKey {
        TapSkillRevisionKey {
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
    Failed,
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
                Failed,
            }

            return RawState::deserialize(deserializer).map(|state| match state {
                RawState::Active => Self::Active,
                RawState::Canceled => Self::Canceled,
                RawState::Completed => Self::Completed,
                RawState::Exhausted => Self::Exhausted,
                RawState::Failed => Self::Failed,
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
            "failed" | "Failed" => Some(TapScheduledTaskState::Failed),
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
    pub interface_revision: Option<InterfaceRevision>,
    #[serde(default, deserialize_with = "deserialize_move_option_tap_address")]
    pub payment_id: Option<sui::types::Address>,
}

impl TapVertexAuthorizationPlanEntry {
    pub fn matches_fixed_tool(&self, fixed_tool: &TapFixedTool) -> bool {
        let module_and_function = format!("{}::{}", self.tool_module, self.tool_function);
        fixed_tool.tool_fqn == module_and_function
            || fixed_tool
                .tool_fqn
                .ends_with(&format!("::{module_and_function}"))
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
    pub payment_policy: TapPaymentPolicy,
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
    #[serde(default, deserialize_with = "deserialize_tap_byte_vector")]
    pub failure_reason: Vec<u8>,
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
        self.state == TapScheduledTaskState::Active
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

/// Registered skill plus the currently active skill revision used for fresh standard execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapActiveSkillExecutionTarget {
    pub skill: TapSkillRecord,
    pub skill_revision: TapSkillRevisionRecord,
}

/// Default execution target plus active skill revision recovered for fresh default DAG execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultDagExecutorRecord {
    pub target: DefaultDagExecutor,
    pub skill: TapSkillRecord,
    pub skill_revision: TapSkillRevisionRecord,
}

/// DAG-backed TAP skill config used by SDK/CLI authoring helpers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapSkillConfig {
    pub name: String,
    pub tap_package_name: String,
    pub dag_path: PathBuf,
    pub tap_package_path: PathBuf,
    pub requirements: TapSkillRequirements,
    pub interface_revision: InterfaceRevision,
}

impl TapSkillConfig {
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
    pub interface_revision: InterfaceRevision,
    pub requirements: TapSkillRequirements,
}

impl TapPublishArtifact {
    pub fn from_config(
        config: &TapSkillConfig,
        dag_id: sui::types::Address,
    ) -> anyhow::Result<Self> {
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
    MissingTapPackageName,
    MissingDagPath,
    MissingTapPackagePath,
    MissingInputCommitment,
    EmptyAuthorizedToolModule,
    EmptyAuthorizedToolFunction,
    DuplicateAuthorizationPlanVertex,
    AuthorizationPlanCommitmentMismatch,
    AuthorizationPlanGrantMismatch,
    AuthorizationPlanToolNotAuthorized,
    AuthorizationPlanSkillRevisionMismatch,
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
            TapValidationError::MissingInputCommitment => write!(f, "input commitment is required"),
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
                    "authorization plan tool is not allowed by skill revision requirements"
                )
            }
            TapValidationError::AuthorizationPlanSkillRevisionMismatch => {
                write!(
                    f,
                    "authorization plan skill revision does not match request context"
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
pub enum TapSkillRevisionResolutionError {
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

impl fmt::Display for TapSkillRevisionResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TapSkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id } => {
                write!(f, "no active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}")
            }
            TapSkillRevisionResolutionError::DuplicateActiveRevision {
                agent_id,
                skill_id,
                count,
            } => write!(
                f,
                "expected one active TAP skill revision for agent_id={agent_id}, skill_id={skill_id}, found {count}"
            ),
            TapSkillRevisionResolutionError::InvalidSkillRevision(error) => {
                write!(f, "invalid TAP skill revision: {error}")
            }
        }
    }
}

impl std::error::Error for TapSkillRevisionResolutionError {}

pub fn validate_requirements(
    requirements: &TapSkillRequirements,
) -> Result<(), TapValidationError> {
    if requirements.input_schema_commitment.is_empty() {
        return Err(TapValidationError::MissingInputCommitment);
    }
    for tool in &requirements.fixed_tools {
        if tool.tool_fqn.trim().is_empty() {
            return Err(TapValidationError::EmptyAuthorizedToolModule);
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
        if !requirements
            .fixed_tools
            .iter()
            .any(|tool| entry.matches_fixed_tool(tool))
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
    payer: sui::types::Address,
) -> anyhow::Result<()> {
    match policy {
        TapPaymentPolicy::UserFunded => {
            let expected = bcs::to_bytes(&payer)?;
            let source_is_valid =
                payment_source.is_empty() || payment_source == expected.as_slice();
            if !source_is_valid {
                anyhow::bail!(
                    "standard TAP user-funded payment source must be empty or payer address BCS"
                );
            }
        }
        TapPaymentPolicy::AgentFunded { max_budget } => {
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

pub fn tap_payment_source_for_address(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    Ok(bcs::to_bytes(&address)?)
}

pub fn tap_payment_source_for_invoker(address: sui::types::Address) -> anyhow::Result<Vec<u8>> {
    TapPaymentSource::invoker(address).to_bcs_bytes()
}

pub fn tap_payment_source_for_agent_vault(agent_id: AgentId) -> anyhow::Result<Vec<u8>> {
    TapPaymentSource::agent_vault(agent_id).to_bcs_bytes()
}

/// Resolve exactly one active skill revision for fresh execution.
pub fn resolve_active_tap_skill_revision<'a>(
    records: &'a [TapSkillRevisionRecord],
    skills: &[TapSkillRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a TapSkillRevisionRecord, TapSkillRevisionResolutionError> {
    let skill_matches = skills
        .iter()
        .filter(|skill| {
            skill.agent_id == Some(agent_id) && skill.skill_id == Some(skill_id) && skill.active
        })
        .collect::<Vec<_>>();

    let skill = match skill_matches.as_slice() {
        [] => {
            return Err(TapSkillRevisionResolutionError::MissingActiveRevision {
                agent_id,
                skill_id,
            })
        }
        [skill] if skill.active => *skill,
        [_] => {
            return Err(TapSkillRevisionResolutionError::MissingActiveRevision {
                agent_id,
                skill_id,
            })
        }
        _ => {
            return Err(TapSkillRevisionResolutionError::DuplicateActiveRevision {
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
        [] => Err(TapSkillRevisionResolutionError::MissingActiveRevision { agent_id, skill_id }),
        [record] => {
            record
                .validate()
                .map_err(TapSkillRevisionResolutionError::InvalidSkillRevision)?;
            Ok(record)
        }
        _ => Err(TapSkillRevisionResolutionError::DuplicateActiveRevision {
            agent_id,
            skill_id,
            count: active.len(),
        }),
    }
}

/// Resolve the unique active skill and skill revision for fresh standard execution.
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

    let skill_revision = registry.active_skill_revision_record(agent_id, skill_id)?;

    Ok(TapActiveSkillExecutionTarget {
        skill,
        skill_revision,
    })
}

/// Resolve the configured default agent from registry state.
pub fn resolve_default_tap_dag_executor(
    registry: &TapRegistry,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    let target = registry.default_dag_executor()?;
    let execution_target =
        resolve_active_tap_skill_execution_target(registry, target.agent_id, target.skill_id)?;

    if execution_target.skill.dag_binding != TapDagBinding::RuntimeSelected {
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
    use {super::*, std::str::FromStr};

    fn addr(value: &str) -> sui::types::Address {
        sui::types::Address::from_str(value).expect("valid address")
    }

    fn requirements() -> TapSkillRequirements {
        TapSkillRequirements {
            input_schema_commitment: vec![1],
            payment_policy: TapPaymentPolicy::AgentFunded { max_budget: 100 },
            schedule_policy: TapSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        }
    }

    fn skill_revision(revision: u64) -> TapSkillRevisionRecord {
        TapSkillRevisionRecord {
            key: TapSkillRevisionKey {
                agent_id: addr("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(revision),
            },
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
            scheduled_task_count: 0,
        }
    }

    fn registry_with_active_skill() -> TapRegistry {
        TapRegistry {
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
        assert_eq!(records[0].key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn validate_rejects_missing_input_commitment() {
        let mut requirements = requirements();
        requirements.input_schema_commitment.clear();

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

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

        let duplicate = vec![skill_revision(1), skill_revision(1)];
        assert!(matches!(
            resolve_active_tap_skill_revision(&duplicate, &skills, addr("0xa"), 11),
            Err(TapSkillRevisionResolutionError::DuplicateActiveRevision { count: 2, .. })
        ));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn agent_registry_object_bcs_decodes_without_inline_default_executor() {
        #[derive(Serialize)]
        struct RawTapRegistryObjectBcs {
            id: sui::types::Address,
            agents: MoveTable<sui::types::Address, TapAgentRecord>,
        }

        let raw = RawTapRegistryObjectBcs {
            id: addr("0xf"),
            agents: MoveTable::new(addr("0x90"), 0),
        };
        let bytes = bcs::to_bytes(&raw).expect("raw Move registry BCS should encode");
        let decoded: TapRegistryObject =
            bcs::from_bytes(&bytes).expect("raw Move registry BCS should decode");

        assert_eq!(decoded.id, addr("0xf"));
        assert_eq!(decoded.agents.id, addr("0x90"));
    }

    #[test]
    fn publish_artifact_contains_skill_artifact_fields() {
        let config = TapSkillConfig {
            name: "weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: requirements(),
            interface_revision: InterfaceRevision(1),
        };

        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.dag_id, addr("0x8"));
        assert_eq!(artifact.skill_name, "weather");
    }

    #[test]
    fn publish_artifact_preserves_current_skill_inputs() {
        let config = TapSkillConfig {
            name: "weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("skill.dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: requirements(),
            interface_revision: InterfaceRevision(1),
        };
        let artifact =
            TapPublishArtifact::from_config(&config, addr("0x8")).expect("valid artifact");

        assert_eq!(artifact.interface_revision, InterfaceRevision(1));
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
    fn validate_standard_tap_payment_options_enforces_user_funded_policy() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let explicit_source = tap_payment_source_for_address(payer).expect("payer source");
        let typed_source = tap_payment_source_for_invoker(payer).expect("typed payer source");
        let other_source = tap_payment_source_for_address(addr("0x2")).expect("other source");
        let policy = TapPaymentPolicy::UserFunded;

        validate_standard_tap_payment_options(agent, &policy, &[], 100, payer)
            .expect("implicit payer source");
        validate_standard_tap_payment_options(agent, &policy, &explicit_source, 100, payer)
            .expect("explicit payer source");
        assert!(
            validate_standard_tap_payment_options(agent, &policy, &typed_source, 100, payer,)
                .is_err(),
            "typed invoker sources are not accepted by Move direct user-funded policy"
        );
        assert!(
            validate_standard_tap_payment_options(agent, &policy, &other_source, 100, payer,)
                .is_err()
        );
    }

    #[test]
    fn validate_standard_tap_payment_options_enforces_source_modes() {
        let agent = addr("0xa");
        let payer = addr("0x1");
        let legacy_agent_source = tap_payment_source_for_address(agent).expect("agent source");
        let agent_source = tap_payment_source_for_agent_vault(agent).expect("agent vault source");

        let agent_funded = TapPaymentPolicy::AgentFunded { max_budget: 100 };
        validate_standard_tap_payment_options(
            agent,
            &agent_funded,
            &legacy_agent_source,
            100,
            payer,
        )
        .expect("agent-funded source at policy cap");
        assert!(
            validate_standard_tap_payment_options(agent, &agent_funded, &[], 100, payer,).is_err()
        );
        assert!(validate_standard_tap_payment_options(
            agent,
            &agent_funded,
            &agent_source,
            100,
            payer,
        )
        .is_err());
        assert!(validate_standard_tap_payment_options(
            agent,
            &agent_funded,
            &legacy_agent_source,
            101,
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
            interface_revision: Some(InterfaceRevision(2)),
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
        let mut fixed_tool_requirements = requirements();
        fixed_tool_requirements.fixed_tools = vec![TapFixedTool {
            tool_registry_id: addr("0x6"),
            tool_fqn: "demo::tool::run".to_string(),
        }];
        let plan = TapVertexAuthorizationPlan(vec![entry.clone()]);
        let hash = plan.hash().unwrap();

        validate_authorization_plan(&fixed_tool_requirements, &plan, Some(&hash))
            .expect("valid plan");
        assert_eq!(
            validate_authorization_plan(&fixed_tool_requirements, &plan, Some(&[0])).unwrap_err(),
            TapValidationError::AuthorizationPlanCommitmentMismatch
        );
        validate_authorization_plan(
            &fixed_tool_requirements,
            &TapVertexAuthorizationPlan::default(),
            Some(&[9, 8]),
        )
        .expect("hash-only contexts without concrete plan entries remain tolerated");
        assert_eq!(
            validate_authorization_plan(
                &fixed_tool_requirements,
                &TapVertexAuthorizationPlan(vec![entry.clone(), entry.clone()]),
                None,
            )
            .unwrap_err(),
            TapValidationError::DuplicateAuthorizationPlanVertex
        );
        assert_eq!(
            validate_authorization_plan(&requirements(), &plan, None).unwrap_err(),
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
            resolved.skill_revision.key.interface_revision,
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
            "payment_policy": "UserFunded",
            "schedule_policy": {
                "recurrence": "Once",
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
        assert_eq!(task.state, TapScheduledTaskState::Active);
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
            "payment_policy": { "AgentFunded": { "max_budget": "100" } },
            "schedule_policy": {
                "recurrence": "Once",
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
