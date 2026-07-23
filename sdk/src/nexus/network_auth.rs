//! Tool focused helpers for the `nexus_registry::network_auth` package.
//!
//! This module is designed for tool operators and other off chain clients that need to:
//! - register or rotate a ToolId message signing key on chain, and
//! - export a tool side allowlist of permitted leaders for the signed HTTP runtime.
//!
//! # Background: what is registered on chain?
//! `nexus_registry::network_auth` binds an off chain identity, leader address or stable Tool ID, to an
//! Ed25519 public key used for signed HTTP.
//!
//! Registration requires a proof of possession signature:
//! `POP_DOMAIN || bcs(IdentityKey) || bcs(key_id) || public_key`
//!
//! Where `key_id` is the binding current `next_key_id`, which makes each signature single use.
//!
//! # Tool runtime (no RPC)
//! Tools must not perform RPC calls at runtime. With the `signed_http` feature, a tool operator
//! can export the typed allowlist data consumed by nexus toolkit.

#[cfg(feature = "signed_http")]
use crate::signed_http::v2::wire::{
    AllowedLeaderFileV1,
    AllowedLeaderKeyFileV1,
    AllowedLeadersFileV1,
};
use {
    crate::{
        move_bindings::registry::network_auth::{IdentityKey, KeyBinding, KeyRecord, NetworkAuth},
        nexus::{
            client::NexusClient,
            crawler::{Crawler, Response},
            error::NexusError,
        },
        sui,
        transactions::{self, tool::OffChainToolRegistration},
        types::{Tool, ToolMeta},
        ToolFqn,
    },
    ed25519_dalek::{Signature, Signer as _, SigningKey},
    std::sync::Arc,
    tokio::sync::Mutex,
};

const POP_DOMAIN_V1: &[u8] = b"nexus_registry.network_auth.pop_v1";
const KEY_SCHEME_ED25519: u8 = 0;

/// Result returned after registering a ToolId message-signing key.
#[derive(Clone, Debug)]
pub struct RegisteredToolKey {
    /// Transaction digest that performed the registration.
    pub tx_digest: sui::types::Digest,
    /// Stable on-chain Tool object ID.
    pub tool_id: sui::types::Address,
    /// Registered key ID carried in signed HTTP transport headers.
    pub tool_kid: u64,
    /// Registered Ed25519 public key bytes.
    pub public_key: [u8; 32],
    /// Deterministic binding object ID under the on-chain `NetworkAuth` registry.
    pub binding_object_id: sui::types::Address,
}

/// An individual key entry returned by [`NetworkAuthActions::list_tool_keys`].
#[derive(Clone, Debug)]
pub struct ToolKeyEntry {
    /// Key identifier used to select this key in signed HTTP transport headers.
    pub kid: u64,
    /// Hex-encoded Ed25519 public key.
    pub public_key_hex: String,
    /// Millisecond timestamp when the key was added.
    pub added_at_ms: u64,
    /// Whether the key has been revoked.
    pub revoked: bool,
}

/// All registered keys for a specific tool, returned by [`NetworkAuthActions::list_tool_keys`].
#[derive(Clone, Debug)]
pub struct ToolKeyList {
    /// On-chain object ID of the `KeyBinding` for this tool.
    pub binding_object_id: sui::types::Address,
    /// The currently active key ID, if any.
    pub active_key_id: Option<u64>,
    /// The next key ID that will be assigned on the next registration.
    pub next_key_id: u64,
    /// All key entries, sorted by kid ascending.
    pub keys: Vec<ToolKeyEntry>,
}

/// Active Ed25519 key material resolved from a `KeyBinding`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEd25519Key {
    pub kid: u64,
    pub public_key: [u8; 32],
}

/// A `KeyBinding` plus its validated active Ed25519 key, if one exists.
pub struct ResolvedKeyBinding {
    pub binding: Response<KeyBinding>,
    pub active_key: Option<ActiveEd25519Key>,
}

pub struct NetworkAuthActions {
    pub(super) client: NexusClient,
}

impl NetworkAuthActions {
    /// Derive the deterministic [`KeyBinding`] object ID for a network auth identity.
    pub fn binding_object_id(
        &self,
        identity: &IdentityKey,
    ) -> Result<sui::types::Address, NexusError> {
        let objects = &self.client.nexus_objects;
        NetworkAuthCodec::new(objects.registry_pkg_id, *objects.network_auth.object_id())
            .binding_object_id(identity)
    }

    /// Register (or rotate) a ToolId message-signing key under `network_auth`.
    ///
    /// This function:
    /// - derives the deterministic `KeyBinding` object ID for the tool identity,
    /// - reads the binding (if present) to discover `next_key_id`,
    /// - signs a PoP message using `tool_signing_key`,
    /// - submits a transaction that creates the binding if needed and registers the key.
    pub async fn register_tool_message_key(
        &self,
        tool_fqn: ToolFqn,
        owner_cap_over_tool: sui::types::Address,
        tool_signing_key: SigningKey,
        description: Option<Vec<u8>>,
    ) -> Result<RegisteredToolKey, NexusError> {
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;

        let codec =
            NetworkAuthCodec::new(objects.registry_pkg_id, *objects.network_auth.object_id());

        let tool_id =
            Tool::derive_id(*objects.tool_registry.object_id(), &tool_fqn).map_err(|e| {
                NexusError::Parsing(anyhow::anyhow!(
                    "failed to derive ToolId for FQN '{tool_fqn}': {e}"
                ))
            })?;
        let identity = IdentityKey::tool(tool_id);
        let binding_object_id = codec.binding_object_id(&identity)?;

        let binding = self.try_get_key_binding(binding_object_id).await?;
        let (binding_ref, next_key_id) = match binding {
            None => (None, 0),
            Some(b) => (Some(b.object_ref()), b.data.next_key_id),
        };

        let (public_key, pop_sig) = tool_key_material(&identity, next_key_id, &tool_signing_key)?;

        // Resolve owner cap object ref for PTB.
        let owner_cap_ref = self
            .client
            .crawler()
            .get_object_metadata(owner_cap_over_tool)
            .await
            .map(|r| r.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch OwnerCap metadata ({owner_cap_over_tool}): {e}"
                ))
            })?;

        // Resolve derived tool object ref for PTB.
        let tool = self
            .client
            .crawler()
            .get_object_metadata(tool_id)
            .await
            .map(|r| r.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch Tool object ref for FQN '{tool_fqn}': {e}"
                ))
            })?;

        let tx = match binding_ref {
            None => transactions::network_auth::create_tool_binding_and_register_key_ptb(
                objects,
                &tool,
                &owner_cap_ref,
                public_key,
                pop_sig,
                description,
            ),
            Some(binding_ref) => {
                transactions::network_auth::register_tool_key_on_existing_binding_ptb(
                    objects,
                    &binding_ref,
                    &tool,
                    &owner_cap_ref,
                    public_key,
                    pop_sig,
                )
            }
        }
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

        Ok(RegisteredToolKey {
            tx_digest: response.digest,
            tool_id,
            tool_kid: next_key_id,
            public_key,
            binding_object_id,
        })
    }

    /// Query all registered message-signing keys for a tool FQN.
    ///
    /// Returns `None` if the tool has no `KeyBinding` on-chain (no keys have ever been
    /// registered for it). Returns `Some(list)` with the full key history otherwise.
    pub async fn list_tool_keys(
        &self,
        tool_fqn: &ToolFqn,
    ) -> Result<Option<ToolKeyList>, NexusError> {
        let objects = &self.client.nexus_objects;
        let codec =
            NetworkAuthCodec::new(objects.registry_pkg_id, *objects.network_auth.object_id());

        let tool_id =
            Tool::derive_id(*objects.tool_registry.object_id(), tool_fqn).map_err(|e| {
                NexusError::Parsing(anyhow::anyhow!(
                    "failed to derive ToolId for FQN '{tool_fqn}': {e}"
                ))
            })?;
        let identity = IdentityKey::tool(tool_id);
        let binding_object_id = codec.binding_object_id(&identity)?;

        let binding = match self.try_get_key_binding(binding_object_id).await? {
            None => return Ok(None),
            Some(b) => b,
        };

        let key_records = self
            .client
            .crawler()
            .get_dynamic_fields::<u64, KeyRecord>(
                binding.data.key_table_id(),
                binding.data.key_table_size(),
            )
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch tool key records ({binding_object_id}): {e}"
                ))
            })?;

        let mut keys: Vec<ToolKeyEntry> = key_records
            .into_iter()
            .map(|(kid, record)| ToolKeyEntry {
                kid,
                public_key_hex: hex::encode(&record.public_key),
                added_at_ms: record.added_at_ms,
                revoked: record.revoked_at_ms().is_some(),
            })
            .collect();
        keys.sort_by_key(|k| k.kid);

        Ok(Some(ToolKeyList {
            binding_object_id,
            active_key_id: binding.data.active_key_id(),
            next_key_id: binding.data.next_key_id,
            keys,
        }))
    }

    /// Export tool side allowlist data containing the active key for each leader.
    ///
    /// The returned file model matches [`crate::signed_http::v2::wire::AllowedLeadersFileV1`].
    ///
    /// `leader_cap_ids` are leader capability ID values for
    /// [`crate::move_bindings::registry::leader_cap::OverNetwork`] objects.
    #[cfg(feature = "signed_http")]
    pub async fn export_allowed_leaders_file_v1(
        &self,
        leader_cap_ids: &[sui::types::Address],
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let objects = &self.client.nexus_objects;
        let codec =
            NetworkAuthCodec::new(objects.registry_pkg_id, *objects.network_auth.object_id());

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            let identity = IdentityKey::leader(*leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity)?;

            let binding = self
                .client
                .crawler()
                .get_object::<KeyBinding>(binding_object_id)
                .await
                .map_err(|e| {
                    NexusError::Rpc(anyhow::anyhow!(
                        "failed to fetch leader KeyBinding ({binding_object_id}): {e}"
                    ))
                })?;

            let active_kid = binding.data.active_key_id().ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} has no active key"
                ))
            })?;

            let keys = self
                .client
                .crawler()
                .get_dynamic_fields::<u64, KeyRecord>(
                    binding.data.key_table_id(),
                    binding.data.key_table_size(),
                )
                .await
                .map_err(|e| {
                    NexusError::Rpc(anyhow::anyhow!(
                        "failed to fetch leader key records ({binding_object_id}): {e}"
                    ))
                })?;

            let record = keys.get(&active_kid).ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} missing active key record kid={active_kid}"
                ))
            })?;
            let public_key: [u8; 32] = record.public_key.as_slice().try_into().map_err(|_| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} active key is not 32 bytes"
                ))
            })?;

            if record.scheme != KEY_SCHEME_ED25519 {
                return Err(NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} active key uses unsupported scheme {}",
                    record.scheme
                )));
            }

            out.push(AllowedLeaderFileV1 {
                leader_id: leader_cap_id.to_string(),
                keys: vec![AllowedLeaderKeyFileV1 {
                    kid: active_kid,
                    public_key: hex::encode(public_key),
                }],
            });
        }

        Ok(AllowedLeadersFileV1 {
            version: 1,
            leaders: out,
        })
    }

    /// List the leader capability IDs currently present in `network_auth.identities`.
    pub async fn list_leader_cap_ids_from_network_auth(
        &self,
    ) -> Result<Vec<sui::types::Address>, NexusError> {
        let objects = &self.client.nexus_objects;
        let network_auth_object_id = *objects.network_auth.object_id();

        let registry = self
            .client
            .crawler()
            .get_object::<NetworkAuth>(network_auth_object_id)
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch NetworkAuth object ({network_auth_object_id}): {e}"
                ))
            })?;

        let mut out = registry
            .data
            .identities
            .contents
            .iter()
            .filter_map(IdentityKey::leader_cap_id)
            .collect::<Vec<_>>();

        out.sort_unstable();
        out.dedup();

        Ok(out)
    }

    /// Export tool side allowlist data containing the active key for every leader identity
    /// found in `network_auth.identities`.
    ///
    /// Leaders that do not have an active key are skipped.
    #[cfg(feature = "signed_http")]
    pub async fn export_allowed_leaders_file_v1_for_all_leaders(
        &self,
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let leader_cap_ids = self.list_leader_cap_ids_from_network_auth().await?;
        if leader_cap_ids.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "network_auth contains no leader identities"
            )));
        }

        let objects = &self.client.nexus_objects;
        let codec =
            NetworkAuthCodec::new(objects.registry_pkg_id, *objects.network_auth.object_id());

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            if let Some(entry) = self
                .export_allowed_leader_entry_file_v1(&codec, leader_cap_id)
                .await?
            {
                out.push(entry);
            }
        }

        if out.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "no leaders with an active Ed25519 key were found in network_auth"
            )));
        }

        Ok(AllowedLeadersFileV1 {
            version: 1,
            leaders: out,
        })
    }

    async fn try_get_key_binding(
        &self,
        binding_object_id: sui::types::Address,
    ) -> Result<Option<crate::nexus::crawler::Response<KeyBinding>>, NexusError> {
        try_get_key_binding_by_object_id(self.client.crawler(), binding_object_id).await
    }

    #[cfg(feature = "signed_http")]
    async fn export_allowed_leader_entry_file_v1(
        &self,
        codec: &NetworkAuthCodec,
        leader_cap_id: sui::types::Address,
    ) -> Result<Option<AllowedLeaderFileV1>, NexusError> {
        let identity = IdentityKey::leader(leader_cap_id);
        let binding_object_id = codec.binding_object_id(&identity)?;

        let binding = self
            .client
            .crawler()
            .get_object::<KeyBinding>(binding_object_id)
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch leader KeyBinding ({binding_object_id}): {e}"
                ))
            })?;

        let Some(active_kid) = binding.data.active_key_id() else {
            return Ok(None);
        };

        let keys = self
            .client
            .crawler()
            .get_dynamic_fields::<u64, KeyRecord>(
                binding.data.key_table_id(),
                binding.data.key_table_size(),
            )
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch leader key records ({binding_object_id}): {e}"
                ))
            })?;

        let record = keys.get(&active_kid).ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "leader binding {binding_object_id} missing active key record kid={active_kid}"
            ))
        })?;
        let public_key: [u8; 32] = record.public_key.as_slice().try_into().map_err(|_| {
            NexusError::Parsing(anyhow::anyhow!(
                "leader binding {binding_object_id} active key is not 32 bytes"
            ))
        })?;

        if record.scheme != KEY_SCHEME_ED25519 {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "leader binding {binding_object_id} active key uses unsupported scheme {}",
                record.scheme
            )));
        }

        Ok(Some(AllowedLeaderFileV1 {
            leader_id: leader_cap_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: active_kid,
                public_key: hex::encode(public_key),
            }],
        }))
    }
}

/// Read-only access to the on-chain `network_auth` registry.
///
/// Unlike [`NetworkAuthActions`], this type does not require a wallet private key
/// or gas coins. It is intended for tool operators that want to export and
/// periodically refresh signed-HTTP config files from chain.
#[derive(Clone)]
pub struct NetworkAuthReader {
    crawler: Crawler,
    registry_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthReader {
    pub fn new(
        crawler: Crawler,
        registry_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        Self {
            crawler,
            registry_pkg_id,
            network_auth_object_id,
        }
    }

    /// Construct a reader by creating a Sui gRPC client for `rpc_url`.
    pub fn from_rpc_url(
        rpc_url: &str,
        registry_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Result<Self, NexusError> {
        let client = sui::grpc::client(rpc_url).map_err(NexusError::Rpc)?;
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));
        Ok(Self::new(crawler, registry_pkg_id, network_auth_object_id))
    }

    /// Derive the deterministic `KeyBinding` object id for `identity`.
    pub fn binding_object_id(
        &self,
        identity: &IdentityKey,
    ) -> Result<sui::types::Address, NexusError> {
        NetworkAuthCodec::new(self.registry_pkg_id, self.network_auth_object_id)
            .binding_object_id(identity)
    }

    /// Fetch the `KeyBinding` for `identity` if it exists.
    pub async fn try_get_key_binding(
        &self,
        identity: &IdentityKey,
    ) -> Result<Option<Response<KeyBinding>>, NexusError> {
        let binding_object_id = self.binding_object_id(identity)?;
        try_get_key_binding_by_object_id(&self.crawler, binding_object_id).await
    }

    /// Fetch the `KeyBinding` for `identity` and resolve its active Ed25519 key, if present.
    pub async fn try_get_active_key_binding(
        &self,
        identity: &IdentityKey,
    ) -> Result<Option<ResolvedKeyBinding>, NexusError> {
        let Some(binding) = self.try_get_key_binding(identity).await? else {
            return Ok(None);
        };
        let active_key = try_get_active_ed25519_key(&self.crawler, &binding).await?;

        Ok(Some(ResolvedKeyBinding {
            binding,
            active_key,
        }))
    }

    /// List the leader capability IDs currently present in `network_auth.identities`.
    pub async fn list_leader_cap_ids_from_network_auth(
        &self,
    ) -> Result<Vec<sui::types::Address>, NexusError> {
        let registry = self
            .crawler
            .get_object::<NetworkAuth>(self.network_auth_object_id)
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch NetworkAuth object ({}): {e}",
                    self.network_auth_object_id
                ))
            })?;

        let mut out = registry
            .data
            .identities
            .contents
            .iter()
            .filter_map(IdentityKey::leader_cap_id)
            .collect::<Vec<_>>();

        out.sort_unstable();
        out.dedup();

        Ok(out)
    }

    /// Export tool side allowlist data containing the active key for every leader identity
    /// found in `network_auth.identities`.
    ///
    /// Leaders that do not have an active key are skipped.
    #[cfg(feature = "signed_http")]
    pub async fn export_allowed_leaders_file_v1_for_all_leaders(
        &self,
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let leader_cap_ids = self.list_leader_cap_ids_from_network_auth().await?;
        if leader_cap_ids.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "network_auth contains no leader identities"
            )));
        }

        let codec = NetworkAuthCodec::new(self.registry_pkg_id, self.network_auth_object_id);

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            if let Some(entry) = self
                .export_allowed_leader_entry_file_v1(&codec, leader_cap_id)
                .await?
            {
                out.push(entry);
            }
        }

        if out.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "no leaders with an active Ed25519 key were found in network_auth"
            )));
        }

        Ok(AllowedLeadersFileV1 {
            version: 1,
            leaders: out,
        })
    }

    #[cfg(feature = "signed_http")]
    async fn export_allowed_leader_entry_file_v1(
        &self,
        codec: &NetworkAuthCodec,
        leader_cap_id: sui::types::Address,
    ) -> Result<Option<AllowedLeaderFileV1>, NexusError> {
        let identity = IdentityKey::leader(leader_cap_id);
        let Some(binding) = self.try_get_active_key_binding(&identity).await? else {
            let binding_object_id = codec.binding_object_id(&identity)?;
            return Err(NexusError::Rpc(anyhow::anyhow!(
                "failed to fetch leader KeyBinding ({binding_object_id}): not found"
            )));
        };

        let Some(active_key) = binding.active_key else {
            return Ok(None);
        };

        Ok(Some(AllowedLeaderFileV1 {
            leader_id: leader_cap_id.to_string(),
            keys: vec![AllowedLeaderKeyFileV1 {
                kid: active_key.kid,
                public_key: hex::encode(active_key.public_key),
            }],
        }))
    }
}

async fn try_get_key_binding_by_object_id(
    crawler: &Crawler,
    binding_object_id: sui::types::Address,
) -> Result<Option<Response<KeyBinding>>, NexusError> {
    match crawler.get_object::<KeyBinding>(binding_object_id).await {
        Ok(binding) => Ok(Some(binding)),
        Err(e) if e.to_string().contains("not found") => Ok(None),
        Err(e) => Err(NexusError::Rpc(e)),
    }
}

async fn try_get_active_ed25519_key(
    crawler: &Crawler,
    binding: &Response<KeyBinding>,
) -> Result<Option<ActiveEd25519Key>, NexusError> {
    let Some(active_kid) = binding.data.active_key_id() else {
        return Ok(None);
    };

    let keys = crawler
        .get_dynamic_fields::<u64, KeyRecord>(
            binding.data.key_table_id(),
            binding.data.key_table_size(),
        )
        .await
        .map_err(|e| {
            NexusError::Rpc(anyhow::anyhow!(
                "failed to fetch key records ({}): {e}",
                binding.object_id
            ))
        })?;

    let record = keys.get(&active_kid).ok_or_else(|| {
        NexusError::Parsing(anyhow::anyhow!(
            "key binding {} is missing active key record kid={active_kid}",
            binding.object_id
        ))
    })?;
    let public_key: [u8; 32] = record.public_key.as_slice().try_into().map_err(|_| {
        NexusError::Parsing(anyhow::anyhow!(
            "key binding {} active key kid={active_kid} is not 32 bytes",
            binding.object_id
        ))
    })?;

    if record.scheme != KEY_SCHEME_ED25519 {
        return Err(NexusError::Parsing(anyhow::anyhow!(
            "key binding {} active key kid={active_kid} uses unsupported scheme {}",
            binding.object_id,
            record.scheme
        )));
    }

    if record.revoked_at_ms().is_some() {
        return Err(NexusError::Parsing(anyhow::anyhow!(
            "key binding {} active key kid={active_kid} is revoked",
            binding.object_id
        )));
    }

    Ok(Some(ActiveEd25519Key {
        kid: active_kid,
        public_key,
    }))
}

/// Internal helper that knows how to compute binding IDs.
struct NetworkAuthCodec {
    registry_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthCodec {
    fn new(
        registry_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        Self {
            registry_pkg_id,
            network_auth_object_id,
        }
    }

    fn binding_object_id(&self, identity: &IdentityKey) -> Result<sui::types::Address, NexusError> {
        crate::move_bindings::derive_network_auth_binding_id(
            self.registry_pkg_id,
            self.network_auth_object_id,
            identity,
        )
        .map_err(NexusError::Parsing)
    }
}

/// Creates an [`OffChainToolRegistration`] whose initial key uses key id zero.
///
/// # Errors
///
/// Returns [`NexusError::Parsing`] if the stable Tool ID cannot be derived or
/// the proof cannot be encoded.
pub fn initial_tool_registration(
    tool_registry_id: sui::types::Address,
    meta: ToolMeta,
    signing_key: &SigningKey,
    invocation_cost_mist: u64,
) -> Result<OffChainToolRegistration, NexusError> {
    let tool_id = Tool::derive_id(tool_registry_id, &meta.fqn).map_err(|error| {
        NexusError::Parsing(anyhow::anyhow!(
            "failed to derive Tool ID for FQN '{}': {error}",
            meta.fqn
        ))
    })?;
    let identity = IdentityKey::tool(tool_id);
    let (public_key, pop_signature) = tool_key_material(&identity, 0, signing_key)?;

    Ok(OffChainToolRegistration {
        meta,
        public_key,
        pop_signature,
        invocation_cost_mist,
    })
}

fn tool_key_material(
    identity: &IdentityKey,
    key_id: u64,
    signing_key: &SigningKey,
) -> Result<([u8; 32], [u8; 64]), NexusError> {
    let public_key = signing_key.verifying_key().to_bytes();
    let message = pop_message_v1(identity, key_id, public_key)?;
    Ok((public_key, sign_bytes(signing_key, &message)))
}

fn pop_message_v1(
    identity: &IdentityKey,
    key_id: u64,
    public_key: [u8; 32],
) -> Result<Vec<u8>, NexusError> {
    let mut out = Vec::new();
    out.extend_from_slice(POP_DOMAIN_V1);
    out.extend_from_slice(&identity_bcs(identity)?);
    out.extend_from_slice(&bcs::to_bytes(&key_id).map_err(|error| {
        NexusError::Parsing(anyhow::anyhow!("failed to BCS encode key_id: {error}"))
    })?);
    out.extend_from_slice(&public_key);
    Ok(out)
}

fn identity_bcs(identity: &IdentityKey) -> Result<Vec<u8>, NexusError> {
    bcs::to_bytes(identity)
        .map_err(|e| NexusError::Parsing(anyhow::anyhow!("failed to BCS-encode IdentityKey: {e}")))
}

fn sign_bytes(signing_key: &SigningKey, msg: &[u8]) -> [u8; 64] {
    let sig: Signature = signing_key.sign(msg);
    sig.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_object_id_is_deterministic_and_distinct() {
        let mut rng = rand::thread_rng();
        let registry_pkg_id = sui::types::Address::generate(&mut rng);
        let network_auth_object_id = sui::types::Address::generate(&mut rng);
        let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);

        let leader = IdentityKey::leader(sui::types::Address::generate(&mut rng));
        let tool = IdentityKey::tool(sui::types::Address::generate(&mut rng));

        let leader_id_first = codec.binding_object_id(&leader).unwrap();
        let leader_id_second = codec.binding_object_id(&leader).unwrap();
        let tool_id = codec.binding_object_id(&tool).unwrap();

        assert_eq!(leader_id_first, leader_id_second);
        assert_ne!(leader_id_first, tool_id);
    }

    #[test]
    fn binding_object_id_matches_move_derived_object_snapshot() {
        let registry_pkg_id = "0x1b7beaf7c749f48e8746b2ee2803eaad6303bd353ad967c3e23db50317919beb"
            .parse()
            .unwrap();
        let network_auth_object_id =
            "0x47fc1741e0f9d0c3a8f573f82fc5c632bc3f3068c325bff24ecb76e4d685b696"
                .parse()
                .unwrap();
        let leader_cap_id = "0x1b7b4eeb8a11033f52b9394b6e284abd6dc33a2a22ff18f678b65d7a909b6eb7"
            .parse()
            .unwrap();
        let expected_binding_id =
            "0xcd2e634ec159ea299824d23a437992dba70c2a2239cfb7cd16a8ee767b17c040"
                .parse()
                .unwrap();

        let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
        let actual = codec
            .binding_object_id(&IdentityKey::leader(leader_cap_id))
            .unwrap();

        assert_eq!(actual, expected_binding_id);
    }

    #[test]
    fn pop_message_v1_matches_expected_layout() {
        let mut rng = rand::thread_rng();
        let identity = IdentityKey::tool(sui::types::Address::generate(&mut rng));
        let key_id = 7u64;
        let public_key = [9u8; 32];

        let msg = pop_message_v1(&identity, key_id, public_key).unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(POP_DOMAIN_V1);
        expected.extend_from_slice(&identity_bcs(&identity).unwrap());
        expected.extend_from_slice(&bcs::to_bytes(&key_id).unwrap());
        expected.extend_from_slice(&public_key);

        assert_eq!(msg, expected);
    }

    #[test]
    fn initial_tool_registration_signs_key_zero_for_tool_identity() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let tool_registry_id = sui::types::Address::from_static("0x42");
        let meta = ToolMeta {
            fqn: "xyz.taluslabs.atomic@1".parse().unwrap(),
            url: "https://example.com/atomic".to_string(),
            description: "atomic".to_string(),
            timeout: std::time::Duration::from_secs(1),
            input_schema: b"{}".to_vec(),
            output_schema: b"{}".to_vec(),
        };

        let registration =
            initial_tool_registration(tool_registry_id, meta.clone(), &signing_key, 9).unwrap();

        assert_eq!(registration.meta, meta);
        assert_eq!(registration.invocation_cost_mist, 9);
        assert_eq!(
            registration.public_key,
            signing_key.verifying_key().to_bytes()
        );

        let tool_id = Tool::derive_id(tool_registry_id, &registration.meta.fqn).unwrap();
        let identity = IdentityKey::tool(tool_id);
        let message = pop_message_v1(&identity, 0, registration.public_key).unwrap();
        let signature = Signature::from_bytes(&registration.pop_signature);
        signing_key
            .verifying_key()
            .verify_strict(&message, &signature)
            .unwrap();
    }

    #[test]
    fn sign_bytes_produces_valid_signature() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let msg = b"nexus";
        let sig = sign_bytes(&key, msg);
        let verify_key = key.verifying_key();
        let signature = Signature::from_bytes(&sig);
        verify_key.verify_strict(msg, &signature).unwrap();
    }

    #[cfg(feature = "test_utils")]
    mod grpc_tests {
        use {
            super::*,
            crate::{
                move_bindings::{
                    registry::network_auth::KeyRecord,
                    sui_framework::table::Table as MoveTable,
                },
                test_utils::sui_mocks,
            },
            serde::Serialize,
            tonic::{Response, Status},
        };

        #[derive(Clone, Debug, Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        fn raw_network_auth_for_test(
            id: sui::types::Address,
            identities: Vec<IdentityKey>,
        ) -> NetworkAuth {
            NetworkAuth::new_for_test(id, identities)
        }

        fn owner_immutable() -> sui::grpc::Owner {
            let mut owner = sui::grpc::Owner::default();
            owner.kind = Some(sui::grpc::owner::OwnerKind::Immutable as i32);
            owner
        }

        fn object_with_contents(
            object_id: Option<sui::types::Address>,
            contents: Vec<u8>,
        ) -> sui::grpc::Object {
            let mut rng = rand::thread_rng();
            let digest = sui::types::Digest::generate(&mut rng);
            let mut object = sui::grpc::Object::default();
            object.object_id = object_id.map(|id| id.to_string());
            object.owner = Some(owner_immutable());
            object.digest = Some(digest.to_string());
            object.version = Some(1);
            let mut bcs = sui::grpc::Bcs::default();
            bcs.value = Some(contents.into());
            object.contents = Some(bcs);
            object
        }

        async fn build_reader(
            registry_pkg_id: sui::types::Address,
            network_auth_object_id: sui::types::Address,
            leader_cap_id: sui::types::Address,
            active_kid: u64,
            record: KeyRecord,
        ) -> NetworkAuthReader {
            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let key_table_id = sui::types::Address::from_static("0x111");
            let binding = KeyBinding::new_for_test(
                sui::types::Address::from_static("0x222"),
                identity,
                None,
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );
            let binding_bytes = bcs::to_bytes(&binding).unwrap();

            let field_object_id = sui::types::Address::from_static("0x333");
            let field_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x444"),
                name: active_kid,
                value: record,
            };
            let field_bytes = bcs::to_bytes(&field_value).unwrap();

            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();

            let binding_object_id_str = binding_object_id.to_string();
            ledger_service
                .expect_get_object()
                .times(1)
                .returning(move |request| {
                    let requested_id = request.get_ref().object_id.as_deref().unwrap_or_default();
                    if requested_id != binding_object_id_str {
                        return Err(Status::not_found(format!(
                            "unexpected object id {requested_id}"
                        )));
                    }

                    let object = object_with_contents(None, binding_bytes.clone());
                    let mut response = sui::grpc::GetObjectResponse::default();
                    response.object = Some(object);
                    Ok(Response::new(response))
                });

            sui_mocks::grpc::mock_list_dynamic_fields(
                &mut state_service,
                vec![(active_kid, field_object_id)],
            );

            ledger_service
                .expect_batch_get_objects()
                .times(1)
                .returning(move |_request| {
                    let object = object_with_contents(Some(field_object_id), field_bytes.clone());
                    let mut result = sui::grpc::GetObjectResult::default();
                    result.result = Some(sui::grpc::get_object_result::Result::Object(object));

                    let mut response = sui::grpc::BatchGetObjectsResponse::default();
                    response.objects = vec![result];
                    Ok(Response::new(response))
                });

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            NetworkAuthReader::from_rpc_url(&rpc_url, registry_pkg_id, network_auth_object_id)
                .unwrap()
        }

        #[cfg(feature = "signed_http")]
        async fn build_reader_with_network_auth(
            registry_pkg_id: sui::types::Address,
            network_auth_object_id: sui::types::Address,
            leader_cap_id: sui::types::Address,
            active_kid: u64,
            record: KeyRecord,
        ) -> NetworkAuthReader {
            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let key_table_id = sui::types::Address::from_static("0x111");
            let binding = KeyBinding::new_for_test(
                sui::types::Address::from_static("0x222"),
                identity.clone(),
                None,
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );

            let network_auth = raw_network_auth_for_test(
                network_auth_object_id,
                vec![
                    identity.clone(),
                    IdentityKey::tool(sui::types::Address::from_static("0x42")),
                ],
            );

            let network_auth_bytes = bcs::to_bytes(&network_auth).unwrap();
            let binding_bytes = bcs::to_bytes(&binding).unwrap();

            let field_object_id = sui::types::Address::from_static("0x333");
            let field_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x444"),
                name: active_kid,
                value: record,
            };
            let field_bytes = bcs::to_bytes(&field_value).unwrap();

            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();

            let network_auth_object_id_str = network_auth_object_id.to_string();
            let binding_object_id_str = binding_object_id.to_string();
            ledger_service
                .expect_get_object()
                .times(2)
                .returning(move |request| {
                    let requested_id = request.get_ref().object_id.as_deref().unwrap_or_default();
                    let object = if requested_id == network_auth_object_id_str {
                        object_with_contents(None, network_auth_bytes.clone())
                    } else if requested_id == binding_object_id_str {
                        object_with_contents(None, binding_bytes.clone())
                    } else {
                        return Err(Status::not_found(format!(
                            "unexpected object id {requested_id}"
                        )));
                    };

                    let mut response = sui::grpc::GetObjectResponse::default();
                    response.object = Some(object);
                    Ok(Response::new(response))
                });

            sui_mocks::grpc::mock_list_dynamic_fields(
                &mut state_service,
                vec![(active_kid, field_object_id)],
            );

            ledger_service
                .expect_batch_get_objects()
                .times(1)
                .returning(move |_request| {
                    let object = object_with_contents(Some(field_object_id), field_bytes.clone());
                    let mut result = sui::grpc::GetObjectResult::default();
                    result.result = Some(sui::grpc::get_object_result::Result::Object(object));

                    let mut response = sui::grpc::BatchGetObjectsResponse::default();
                    response.objects = vec![result];
                    Ok(Response::new(response))
                });

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            NetworkAuthReader::from_rpc_url(&rpc_url, registry_pkg_id, network_auth_object_id)
                .unwrap()
        }

        #[tokio::test]
        async fn reader_try_get_active_key_binding_returns_validated_active_key() {
            let mut rng = rand::thread_rng();
            let registry_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);
            let active_kid = 5u64;
            let public_key = [7u8; 32];
            let reader = build_reader(
                registry_pkg_id,
                network_auth_object_id,
                leader_cap_id,
                active_kid,
                KeyRecord::new_for_test(0, public_key.to_vec(), 0, None),
            )
            .await;

            let identity = IdentityKey::leader(leader_cap_id);
            let resolved = reader
                .try_get_active_key_binding(&identity)
                .await
                .unwrap()
                .expect("binding should exist");

            assert_eq!(
                reader.binding_object_id(&identity).unwrap(),
                resolved.binding.object_id
            );
            assert_eq!(
                resolved.active_key,
                Some(ActiveEd25519Key {
                    kid: active_kid,
                    public_key,
                })
            );
        }

        #[cfg(feature = "signed_http")]
        #[tokio::test]
        async fn actions_export_allowlists() {
            let mut rng = rand::thread_rng();
            let registry_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);

            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let active_kid = 3u64;
            let public_key = [7u8; 32];
            let record = KeyRecord::new_for_test(0, public_key.to_vec(), 0, None);

            let key_table_id = sui::types::Address::from_static("0x111");
            let binding = KeyBinding::new_for_test(
                sui::types::Address::from_static("0x222"),
                identity.clone(),
                None,
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );

            let network_auth = raw_network_auth_for_test(
                network_auth_object_id,
                vec![
                    identity.clone(),
                    IdentityKey::tool(sui::types::Address::from_static("0x42")),
                ],
            );

            let network_auth_bytes = bcs::to_bytes(&network_auth).unwrap();
            let binding_bytes = bcs::to_bytes(&binding).unwrap();

            let field_object_id = sui::types::Address::from_static("0x333");
            let field_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x444"),
                name: active_kid,
                value: record,
            };
            let field_bytes = bcs::to_bytes(&field_value).unwrap();

            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();

            // Called once by NexusClientBuilder during initialization.
            sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 42);

            let network_auth_object_id_str = network_auth_object_id.to_string();
            let binding_object_id_str = binding_object_id.to_string();
            ledger_service
                .expect_get_object()
                .times(4)
                .returning(move |request| {
                    let requested_id = request.get_ref().object_id.as_deref().unwrap_or_default();
                    let object = if requested_id == network_auth_object_id_str {
                        object_with_contents(None, network_auth_bytes.clone())
                    } else if requested_id == binding_object_id_str {
                        object_with_contents(None, binding_bytes.clone())
                    } else {
                        return Err(Status::not_found(format!(
                            "unexpected object id {requested_id}"
                        )));
                    };

                    let mut response = sui::grpc::GetObjectResponse::default();
                    response.object = Some(object);
                    Ok(Response::new(response))
                });

            state_service
                .expect_list_dynamic_fields()
                .times(2)
                .returning(move |_request| {
                    let mut dynamic_field = sui::grpc::DynamicField::default();
                    dynamic_field.set_child_id(field_object_id);
                    dynamic_field.set_field_id(field_object_id);
                    let mut name = sui::grpc::Bcs::default();
                    name.value = Some(bcs::to_bytes(&active_kid).unwrap().into());
                    dynamic_field.set_name(name);

                    let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                    response.dynamic_fields = vec![dynamic_field];
                    Ok(Response::new(response))
                });

            ledger_service
                .expect_batch_get_objects()
                .times(2)
                .returning(move |_request| {
                    let object = object_with_contents(Some(field_object_id), field_bytes.clone());
                    let mut result = sui::grpc::GetObjectResult::default();
                    result.result = Some(sui::grpc::get_object_result::Result::Object(object));

                    let mut response = sui::grpc::BatchGetObjectsResponse::default();
                    response.objects = vec![result];
                    Ok(Response::new(response))
                });

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            let mut rng = rand::thread_rng();
            let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);

            let mut nexus_objects = crate::test_utils::sui_mocks::mock_nexus_objects();
            nexus_objects.network_auth = sui::types::ObjectReference::new(
                network_auth_object_id,
                1,
                sui::types::Digest::generate(&mut rng),
            );
            nexus_objects.registry_pkg_id = registry_pkg_id;

            let gas_coin = sui_mocks::mock_sui_object_ref();
            let client = NexusClient::builder()
                .with_private_key(pk)
                .with_rpc_url(&rpc_url)
                .with_nexus_objects(nexus_objects)
                .with_gas(vec![gas_coin], 1_000_000)
                .build()
                .await
                .unwrap();

            let expected_entry = AllowedLeaderFileV1 {
                leader_id: leader_cap_id.to_string(),
                keys: vec![AllowedLeaderKeyFileV1 {
                    kid: active_kid,
                    public_key: hex::encode(public_key),
                }],
            };

            let file = client
                .network_auth()
                .export_allowed_leaders_file_v1(&[leader_cap_id])
                .await
                .unwrap();
            assert_eq!(file.leaders.len(), 1);
            assert_eq!(file.leaders[0].leader_id, expected_entry.leader_id);
            assert_eq!(file.leaders[0].keys.len(), 1);
            assert_eq!(file.leaders[0].keys[0].kid, active_kid);
            assert_eq!(file.leaders[0].keys[0].public_key, hex::encode(public_key));

            let leaders = client
                .network_auth()
                .list_leader_cap_ids_from_network_auth()
                .await
                .unwrap();
            assert_eq!(leaders, vec![leader_cap_id]);

            let file = client
                .network_auth()
                .export_allowed_leaders_file_v1_for_all_leaders()
                .await
                .unwrap();
            assert_eq!(file.leaders.len(), 1);
            assert_eq!(file.leaders[0].leader_id, expected_entry.leader_id);
            assert_eq!(file.leaders[0].keys.len(), 1);
            assert_eq!(file.leaders[0].keys[0].kid, active_kid);
            assert_eq!(file.leaders[0].keys[0].public_key, hex::encode(public_key));
        }

        #[cfg(feature = "signed_http")]
        #[tokio::test]
        async fn reader_export_allowed_leaders_file_v1_for_all_leaders() {
            let mut rng = rand::thread_rng();
            let registry_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);

            let active_kid = 7u64;
            let public_key = [9u8; 32];

            let reader = build_reader_with_network_auth(
                registry_pkg_id,
                network_auth_object_id,
                leader_cap_id,
                active_kid,
                KeyRecord::new_for_test(0, public_key.to_vec(), 0, None),
            )
            .await;

            let allowlist = reader
                .export_allowed_leaders_file_v1_for_all_leaders()
                .await
                .unwrap();
            assert_eq!(allowlist.version, 1);
            assert_eq!(allowlist.leaders.len(), 1);
            assert_eq!(allowlist.leaders[0].leader_id, leader_cap_id.to_string());
            assert_eq!(allowlist.leaders[0].keys.len(), 1);
            assert_eq!(allowlist.leaders[0].keys[0].kid, active_kid);
            assert_eq!(
                allowlist.leaders[0].keys[0].public_key,
                hex::encode(public_key)
            );
        }

        /// Verifies that `list_tool_keys` returns the correct key list for a tool
        /// with an active key and a revoked key, sorted by kid ascending.
        /// Guards against regressions in the binding lookup, dynamic field
        /// deserialization, key sorting, and revocation flag mapping.
        #[tokio::test]
        async fn list_tool_keys_returns_sorted_entries() {
            let mut rng = rand::thread_rng();
            let registry_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);

            let tool_fqn_str = "xyz.demo.tool@1";
            let tool_fqn: crate::ToolFqn = tool_fqn_str.parse().unwrap();
            let tool_registry_id = sui::types::Address::generate(&mut rng);

            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::tool(
                Tool::derive_id(tool_registry_id, &tool_fqn).expect("Tool ID derives"),
            );
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let key_table_id = sui::types::Address::from_static("0x111");

            // Two keys: kid=0 (revoked), kid=1 (active).
            let record_0 = KeyRecord::new_for_test(0, vec![0xaau8; 32], 1000, Some(2000));
            let record_1 = KeyRecord::new_for_test(0, vec![0xbbu8; 32], 3000, None);

            let binding = KeyBinding::new_for_test(
                sui::types::Address::from_static("0x222"),
                identity.clone(),
                None,
                2,
                Some(1),
                MoveTable::new(key_table_id, 2),
            );

            let binding_bytes = bcs::to_bytes(&binding).unwrap();

            let field_0_id = sui::types::Address::from_static("0x333");
            let field_1_id = sui::types::Address::from_static("0x444");

            let field_0_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x555"),
                name: 0u64,
                value: record_0,
            };
            let field_1_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x666"),
                name: 1u64,
                value: record_1,
            };
            let field_0_bytes = bcs::to_bytes(&field_0_value).unwrap();
            let field_1_bytes = bcs::to_bytes(&field_1_value).unwrap();

            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();

            // Called once by NexusClientBuilder during initialization.
            sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 42);

            // get_object: returns the binding (called by try_get_key_binding).
            let binding_object_id_str = binding_object_id.to_string();
            ledger_service
                .expect_get_object()
                .times(1)
                .returning(move |request| {
                    let requested_id = request.get_ref().object_id.as_deref().unwrap_or_default();
                    if requested_id == binding_object_id_str {
                        let object = object_with_contents(None, binding_bytes.clone());
                        let mut response = sui::grpc::GetObjectResponse::default();
                        response.object = Some(object);
                        Ok(Response::new(response))
                    } else {
                        Err(Status::not_found(format!(
                            "unexpected object id {requested_id}"
                        )))
                    }
                });

            // list_dynamic_fields: returns two field entries (kid=0 and kid=1).
            // Return kid=1 first to verify the sort.
            state_service
                .expect_list_dynamic_fields()
                .times(1)
                .returning(move |_request| {
                    let mut df1 = sui::grpc::DynamicField::default();
                    df1.set_child_id(field_1_id);
                    df1.set_field_id(field_1_id);
                    let mut name1 = sui::grpc::Bcs::default();
                    name1.value = Some(bcs::to_bytes(&1u64).unwrap().into());
                    df1.set_name(name1);

                    let mut df0 = sui::grpc::DynamicField::default();
                    df0.set_child_id(field_0_id);
                    df0.set_field_id(field_0_id);
                    let mut name0 = sui::grpc::Bcs::default();
                    name0.value = Some(bcs::to_bytes(&0u64).unwrap().into());
                    df0.set_name(name0);

                    let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                    response.dynamic_fields = vec![df1, df0];
                    Ok(Response::new(response))
                });

            // batch_get_objects: returns both field values.
            ledger_service
                .expect_batch_get_objects()
                .times(1)
                .returning(move |_request| {
                    let obj1 = object_with_contents(Some(field_1_id), field_1_bytes.clone());
                    let mut r1 = sui::grpc::GetObjectResult::default();
                    r1.result = Some(sui::grpc::get_object_result::Result::Object(obj1));

                    let obj0 = object_with_contents(Some(field_0_id), field_0_bytes.clone());
                    let mut r0 = sui::grpc::GetObjectResult::default();
                    r0.result = Some(sui::grpc::get_object_result::Result::Object(obj0));

                    let mut response = sui::grpc::BatchGetObjectsResponse::default();
                    response.objects = vec![r1, r0];
                    Ok(Response::new(response))
                });

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
            let mut nexus_objects = crate::test_utils::sui_mocks::mock_nexus_objects();

            nexus_objects.network_auth = sui::types::ObjectReference::new(
                network_auth_object_id,
                1,
                sui::types::Digest::generate(&mut rng),
            );
            nexus_objects.registry_pkg_id = registry_pkg_id;
            nexus_objects.tool_registry = sui::types::ObjectReference::new(
                tool_registry_id,
                1,
                sui::types::Digest::generate(&mut rng),
            );

            let gas_coin = sui_mocks::mock_sui_object_ref();
            let client = NexusClient::builder()
                .with_private_key(pk)
                .with_rpc_url(&rpc_url)
                .with_nexus_objects(nexus_objects)
                .with_gas(vec![gas_coin], 1_000_000)
                .build()
                .await
                .unwrap();

            let list = client
                .network_auth()
                .list_tool_keys(&tool_fqn)
                .await
                .unwrap()
                .expect("binding exists, should return Some");

            assert_eq!(list.binding_object_id, binding_object_id);
            assert_eq!(list.active_key_id, Some(1));
            assert_eq!(list.next_key_id, 2);
            assert_eq!(list.keys.len(), 2);

            // Sorted by kid ascending.
            assert_eq!(list.keys[0].kid, 0);
            assert_eq!(list.keys[0].public_key_hex, hex::encode([0xaau8; 32]));
            assert_eq!(list.keys[0].added_at_ms, 1000);
            assert!(list.keys[0].revoked);

            assert_eq!(list.keys[1].kid, 1);
            assert_eq!(list.keys[1].public_key_hex, hex::encode([0xbbu8; 32]));
            assert_eq!(list.keys[1].added_at_ms, 3000);
            assert!(!list.keys[1].revoked);
        }
    }
}
