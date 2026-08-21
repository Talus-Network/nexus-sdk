//! Tool focused helpers for the `nexus_registry::network_auth` package.
//!
//! This module supports tool operators and other clients that need to register
//! or rotate a Tool message signing key on chain, or export a tool side
//! allowlist for the signed HTTP runtime.
//!
//! # Registered state
//!
//! `nexus_registry::network_auth` binds an off chain identity, leader address,
//! or stable Tool ID to an Ed25519 public key used for signed HTTP.
//!
//! Registration requires a proof of possession signature:
//! `POP_DOMAIN || bcs(IdentityKey) || bcs(key_id) || public_key`
//!
//! Where `key_id` is the binding current `next_key_id`, which makes each signature single use.
//!
//! # Tool runtime
//!
//! Tools must not perform RPC calls at runtime. With the `signed_http` feature, a tool operator
//! can export the typed allowlist data consumed by nexus toolkit.

#[cfg(feature = "signed_http")]
use crate::signed_http::v3::wire::{
    AllowedLeaderFileV1,
    AllowedLeaderKeyFileV1,
    AllowedLeadersFileV1,
};
use {
    crate::{
        move_bindings::{
            registry::{
                era::V1 as RegistryWitnessV1,
                network_auth::{
                    IdentityKey,
                    KeyBinding,
                    KeyBindingInnerV1,
                    KeyRecord,
                    NetworkAuth,
                    NetworkAuthInnerV1,
                },
            },
            tool::{
                era::V1 as ToolWitnessV1,
                tool_registry::{Tool as ToolAnchor, ToolInnerV1},
            },
        },
        nexus::{
            client::NexusClient,
            crawler::{Crawler, Response},
            error::NexusError,
            state::StateResolver,
        },
        sui,
        transactions::{self, tool::OffChainToolRegistration},
        types::{NexusContext, PackageRole, Tool, ToolMeta},
        ToolFqn,
    },
    ed25519_dalek::{Signature, Signer as _, SigningKey},
    std::sync::Arc,
};

const POP_DOMAIN_V1: &[u8] = b"nexus_registry.network_auth.pop_v1";
const KEY_SCHEME_ED25519: u8 = 0;

/// Result of registering a Tool message signing key.
#[derive(Clone, Debug)]
pub struct RegisteredToolKey {
    /// Transaction digest that performed the registration.
    pub tx_digest: sui::types::Digest,
    /// Stable on chain [`ToolAnchor`] object ID.
    pub tool_id: sui::types::Address,
    /// Registered key ID carried in signed HTTP transport headers.
    pub tool_kid: u64,
    /// Registered Ed25519 public key bytes.
    pub public_key: [u8; 32],
    /// Deterministic [`KeyBinding`] object ID below [`NetworkAuth`].
    pub binding_object_id: sui::types::Address,
}

/// An individual key entry returned by [`NetworkAuthActions::list_tool_keys`].
#[derive(Clone, Debug)]
pub struct ToolKeyEntry {
    /// Key identifier used to select this key in signed HTTP transport headers.
    pub kid: u64,
    /// Hex encoded Ed25519 public key.
    pub public_key_hex: String,
    /// Millisecond timestamp when the key was added.
    pub added_at_ms: u64,
    /// Whether the key has been revoked.
    pub revoked: bool,
}

/// All registered keys for a specific tool, returned by [`NetworkAuthActions::list_tool_keys`].
#[derive(Clone, Debug)]
pub struct ToolKeyList {
    /// On chain object ID of the [`KeyBinding`] for this tool.
    pub binding_object_id: sui::types::Address,
    /// The currently active key ID, if any.
    pub active_key_id: Option<u64>,
    /// The next key ID that will be assigned on the next registration.
    pub next_key_id: u64,
    /// All key entries, sorted by kid ascending.
    pub keys: Vec<ToolKeyEntry>,
}

/// Active Ed25519 key material resolved from a [`KeyBinding`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveEd25519Key {
    /// Key identifier used by signed HTTP messages.
    pub kid: u64,
    /// Exact Ed25519 public key bytes.
    pub public_key: [u8; 32],
}

/// A [`KeyBinding`] plus its validated active Ed25519 key, when present.
pub struct ResolvedKeyBinding {
    /// Current supported binding state.
    pub binding: Response<KeyBindingInnerV1>,
    /// Active supported Ed25519 key, when configured.
    pub active_key: Option<ActiveEd25519Key>,
}

/// Wallet backed network authorization operations.
pub struct NetworkAuthActions {
    pub(super) client: NexusClient,
}

impl NetworkAuthActions {
    /// Derive the deterministic [`KeyBinding`] object ID for a network auth identity.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when the [`NetworkAuth`] package graph cannot be
    /// resolved or the binding ID cannot be derived.
    pub async fn binding_object_id(
        &self,
        identity: &IdentityKey,
    ) -> Result<sui::types::Address, NexusError> {
        let context = self.network_auth_context().await?;
        network_auth_codec(&context)?.binding_object_id(identity)
    }

    /// Registers or rotates a Tool message signing key below [`NetworkAuth`].
    ///
    /// The binding witness is the authority source for rotation. A new binding
    /// instead uses the canonical [`NetworkAuth`] root. In both cases the
    /// [`ToolAnchor`] state pair must match the selected package graph before a
    /// transaction is constructed.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when state or package compatibility cannot be
    /// established, proof creation fails, or transaction submission fails.
    pub async fn register_tool_message_key(
        &self,
        tool_fqn: ToolFqn,
        owner_cap_over_tool: sui::types::Address,
        tool_signing_key: SigningKey,
        description: Option<Vec<u8>>,
    ) -> Result<RegisteredToolKey, NexusError> {
        let address = self.client.owner()?;
        let objects = self.client.get_nexus_objects();
        let tool_id =
            Tool::derive_id(objects.tool_registry.object_id(), &tool_fqn).map_err(|e| {
                NexusError::Parsing(anyhow::anyhow!(
                    "failed to derive ToolId for FQN '{tool_fqn}': {e}"
                ))
            })?;
        let identity = IdentityKey::tool(tool_id);
        let root_context = self.network_auth_context().await?;
        let binding_object_id = network_auth_codec(&root_context)?.binding_object_id(&identity)?;

        let existing = Self::try_get_key_binding(&self.client, binding_object_id).await?;
        let (context, binding_ref, next_key_id) = match existing {
            None => (root_context, None, 0),
            Some((context, binding)) => {
                validate_binding_identity(binding_object_id, &binding.data, &identity)?;
                let next_key_id = binding.data.next_key_id;
                (context, Some(binding.object_ref()), next_key_id)
            }
        };

        let registry_storage = context
            .require_package(PackageRole::Registry)
            .map_err(|error| NexusError::Configuration(error.to_string()))?
            .storage_id;
        context
            .require_package(PackageRole::Tool)
            .map_err(|error| NexusError::IncompatiblePackage {
                package: registry_storage,
                reason: error.to_string(),
            })?;
        self.client
            .state_resolver()
            .validate_state_pair::<ToolAnchor, ToolWitnessV1, ToolInnerV1>(tool_id, &context)
            .await?;

        let (public_key, pop_sig) = tool_key_material(&identity, next_key_id, &tool_signing_key)?;
        let (owner_cap_ref, tool) = tokio::try_join!(
            self.client.object_reference(owner_cap_over_tool),
            self.client.object_reference(tool_id),
        )?;

        let tx = match binding_ref {
            None => transactions::network_auth::create_tool_binding_and_register_key_ptb(
                &context,
                &tool,
                &owner_cap_ref,
                public_key,
                pop_sig,
                description,
            ),
            Some(binding_ref) => {
                transactions::network_auth::register_tool_key_on_existing_binding_ptb(
                    &context,
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

    /// Queries every registered message signing key for a Tool FQN.
    ///
    /// Returns [`None`] when the Tool has no [`KeyBinding`].
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when root authority, binding state, or key records
    /// cannot be resolved.
    pub async fn list_tool_keys(
        &self,
        tool_fqn: &ToolFqn,
    ) -> Result<Option<ToolKeyList>, NexusError> {
        let objects = self.client.get_nexus_objects();
        let context = self.network_auth_context().await?;
        let codec = network_auth_codec(&context)?;
        let tool_id =
            Tool::derive_id(objects.tool_registry.object_id(), tool_fqn).map_err(|e| {
                NexusError::Parsing(anyhow::anyhow!(
                    "failed to derive ToolId for FQN '{tool_fqn}': {e}"
                ))
            })?;
        let identity = IdentityKey::tool(tool_id);
        let binding_object_id = codec.binding_object_id(&identity)?;

        let (_, binding) = match Self::try_get_key_binding(&self.client, binding_object_id).await? {
            None => return Ok(None),
            Some(binding) => binding,
        };
        validate_binding_identity(binding_object_id, &binding.data, &identity)?;

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

    /// Exports the active key for each requested leader.
    ///
    /// The returned file model matches [`crate::signed_http::v3::wire::AllowedLeadersFileV1`].
    ///
    /// `leader_cap_ids` are leader capability ID values for
    /// [`crate::move_bindings::registry::leader_cap::OverNetwork`] objects.
    #[cfg(feature = "signed_http")]
    pub async fn export_allowed_leaders_file_v1(
        &self,
        leader_cap_ids: &[sui::types::Address],
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let context = self.network_auth_context().await?;
        let codec = network_auth_codec(&context)?;

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            let identity = IdentityKey::leader(*leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity)?;
            let (_, binding) = Self::try_get_key_binding(&self.client, binding_object_id)
                .await?
                .ok_or_else(|| {
                    NexusError::Rpc(anyhow::anyhow!(
                        "leader KeyBinding '{binding_object_id}' was not found"
                    ))
                })?;
            validate_binding_identity(binding_object_id, &binding.data, &identity)?;

            let active_kid = binding.data.active_key_id().ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} has no active key"
                ))
            })?;

            let record = fetch_key_record(self.client.crawler(), &binding, active_kid).await?;
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

    /// Lists leader capability IDs currently present in [`NetworkAuth`].
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when the root package graph or stored state cannot
    /// be resolved.
    pub async fn list_leader_cap_ids_from_network_auth(
        &self,
    ) -> Result<Vec<sui::types::Address>, NexusError> {
        let (_, state) = self.network_auth_state().await?;
        Ok(sorted_leader_cap_ids(&state.data))
    }

    /// Exports the active key for every leader identity in [`NetworkAuth`].
    ///
    /// Leaders that do not have an active key are skipped.
    #[cfg(feature = "signed_http")]
    pub async fn export_allowed_leaders_file_v1_for_all_leaders(
        &self,
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let (context, state) = self.network_auth_state().await?;
        let leader_cap_ids = sorted_leader_cap_ids(&state.data);
        if leader_cap_ids.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "network_auth contains no leader identities"
            )));
        }

        let codec = network_auth_codec(&context)?;

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            if let Some(entry) =
                Self::export_allowed_leader_entry_file_v1(&self.client, &codec, leader_cap_id)
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

    async fn network_auth_context(&self) -> Result<Arc<NexusContext>, NexusError> {
        let objects = self.client.get_nexus_objects();
        self.client.context_for_root(&objects.network_auth).await
    }

    async fn network_auth_state(
        &self,
    ) -> Result<(Arc<NexusContext>, Response<NetworkAuthInnerV1>), NexusError> {
        let context = self.network_auth_context().await?;
        let object_id = context.network_auth.object_id();
        let state = self
            .client
            .state_resolver()
            .load_inner::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(object_id, &context)
            .await?;
        Ok((context, state))
    }

    async fn try_get_key_binding(
        client: &NexusClient,
        binding_object_id: sui::types::Address,
    ) -> Result<Option<(Arc<NexusContext>, Response<KeyBindingInnerV1>)>, NexusError> {
        let Some(_) = client
            .crawler()
            .get_optional_object::<KeyBinding>(binding_object_id)
            .await
            .map_err(NexusError::Rpc)?
        else {
            return Ok(None);
        };
        let context = client.context_for_object(binding_object_id).await?;
        let binding = client
            .state_resolver()
            .load_inner::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
                binding_object_id,
                &context,
            )
            .await?;
        Ok(Some((context, binding)))
    }

    #[cfg(feature = "signed_http")]
    async fn export_allowed_leader_entry_file_v1(
        client: &NexusClient,
        codec: &NetworkAuthCodec,
        leader_cap_id: sui::types::Address,
    ) -> Result<Option<AllowedLeaderFileV1>, NexusError> {
        let identity = IdentityKey::leader(leader_cap_id);
        let binding_object_id = codec.binding_object_id(&identity)?;
        let (_, binding) = Self::try_get_key_binding(client, binding_object_id)
            .await?
            .ok_or_else(|| {
                NexusError::Rpc(anyhow::anyhow!(
                    "leader KeyBinding '{binding_object_id}' was not found"
                ))
            })?;
        validate_binding_identity(binding_object_id, &binding.data, &identity)?;

        let Some(active_kid) = binding.data.active_key_id() else {
            return Ok(None);
        };

        let record = fetch_key_record(client.crawler(), &binding, active_kid).await?;
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

/// Read only access to the on chain [`NetworkAuth`] registry.
///
/// Unlike [`NetworkAuthActions`], this type does not require a wallet private
/// key or gas coins. It supports tool operators that export and periodically
/// refresh signed HTTP configuration from chain.
#[derive(Clone)]
pub struct NetworkAuthReader {
    crawler: Crawler,
    state_resolver: StateResolver,
    registry_type_origin_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthReader {
    /// Creates a reader for one [`NetworkAuth`] root.
    ///
    /// `registry_type_origin_pkg_id` is the package that first defined
    /// [`IdentityKey`]. It is used only for the stable derived binding ID. Each
    /// object witness independently selects the package graph used to decode
    /// its state.
    pub fn new(
        crawler: Crawler,
        registry_type_origin_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        let state_resolver = StateResolver::new(Arc::new(crawler.clone()));
        Self {
            crawler,
            state_resolver,
            registry_type_origin_pkg_id,
            network_auth_object_id,
        }
    }

    /// Creates a reader backed by the Sui gRPC endpoint at `rpc_url`.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Rpc`] when `rpc_url` is invalid.
    pub fn from_rpc_url(
        rpc_url: &str,
        registry_type_origin_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Result<Self, NexusError> {
        let client = sui::grpc::client(rpc_url).map_err(NexusError::Rpc)?;
        let crawler = Crawler::new(Arc::new(client));
        Ok(Self::new(
            crawler,
            registry_type_origin_pkg_id,
            network_auth_object_id,
        ))
    }

    /// Derives the deterministic [`KeyBinding`] object ID for `identity`.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Parsing`] when the identity cannot be encoded.
    pub fn binding_object_id(
        &self,
        identity: &IdentityKey,
    ) -> Result<sui::types::Address, NexusError> {
        NetworkAuthCodec::new(
            self.registry_type_origin_pkg_id,
            self.network_auth_object_id,
        )
        .binding_object_id(identity)
    }

    /// Fetches the [`KeyBinding`] for `identity`, when it exists.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when object authority or stored state cannot be
    /// resolved.
    pub async fn try_get_key_binding(
        &self,
        identity: &IdentityKey,
    ) -> Result<Option<Response<KeyBindingInnerV1>>, NexusError> {
        let binding_object_id = self.binding_object_id(identity)?;
        try_get_key_binding_by_object_id(&self.crawler, &self.state_resolver, binding_object_id)
            .await
    }

    /// Fetches a [`KeyBinding`] and resolves its active Ed25519 key.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when binding state or its active key record is
    /// invalid or unavailable.
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

    /// Lists leader capability IDs currently present in [`NetworkAuth`].
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when root authority or stored state cannot be
    /// resolved.
    pub async fn list_leader_cap_ids_from_network_auth(
        &self,
    ) -> Result<Vec<sui::types::Address>, NexusError> {
        let state = self
            .state_resolver
            .resolve_and_load_inner::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(
                self.network_auth_object_id,
            )
            .await?;
        Ok(sorted_leader_cap_ids(&state.data))
    }

    /// Exports the active key for every leader identity in [`NetworkAuth`].
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

        let codec = NetworkAuthCodec::new(
            self.registry_type_origin_pkg_id,
            self.network_auth_object_id,
        );

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
    state_resolver: &StateResolver,
    binding_object_id: sui::types::Address,
) -> Result<Option<Response<KeyBindingInnerV1>>, NexusError> {
    let Some(_) = crawler
        .get_optional_object::<KeyBinding>(binding_object_id)
        .await
        .map_err(NexusError::Rpc)?
    else {
        return Ok(None);
    };

    state_resolver
        .resolve_and_load_inner::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
            binding_object_id,
        )
        .await
        .map(Some)
}

fn sorted_leader_cap_ids(state: &NetworkAuthInnerV1) -> Vec<sui::types::Address> {
    let mut leaders = state.leader_cap_ids().collect::<Vec<_>>();
    leaders.sort_unstable();
    leaders.dedup();
    leaders
}

async fn try_get_active_ed25519_key(
    crawler: &Crawler,
    binding: &Response<KeyBindingInnerV1>,
) -> Result<Option<ActiveEd25519Key>, NexusError> {
    let Some(active_kid) = binding.data.active_key_id() else {
        return Ok(None);
    };

    let record = fetch_key_record(crawler, binding, active_kid).await?;
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

async fn fetch_key_record(
    crawler: &Crawler,
    binding: &Response<KeyBindingInnerV1>,
    key_id: u64,
) -> Result<KeyRecord, NexusError> {
    crawler
        .get_dynamic_field_by_key::<u64, KeyRecord>(
            binding.data.key_table_id(),
            key_id,
            &sui::types::TypeTag::U64,
        )
        .await
        .map_err(|error| {
            NexusError::Rpc(anyhow::anyhow!(
                "failed to fetch key record ({}, kid={key_id}): {error}",
                binding.object_id
            ))
        })?
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "key binding {} is missing key record kid={key_id}",
                binding.object_id
            ))
        })
}

fn network_auth_codec(context: &NexusContext) -> Result<NetworkAuthCodec, NexusError> {
    let registry = context
        .require_package(PackageRole::Registry)
        .map_err(|error| NexusError::Configuration(error.to_string()))?;
    let identity_origin = registry
        .type_origin("network_auth", "IdentityKey")
        .map_err(|error| NexusError::IncompatiblePackage {
            package: registry.storage_id,
            reason: error.to_string(),
        })?;

    Ok(NetworkAuthCodec::new(
        identity_origin,
        context.network_auth.object_id(),
    ))
}

fn validate_binding_identity(
    binding_object_id: sui::types::Address,
    binding: &KeyBindingInnerV1,
    expected: &IdentityKey,
) -> Result<(), NexusError> {
    if &binding.identity == expected {
        return Ok(());
    }

    Err(NexusError::InvalidObjectState {
        object: binding_object_id,
        reason: "stored binding identity does not match its derived object ID".to_owned(),
    })
}

/// Internal helper that knows how to compute binding IDs.
struct NetworkAuthCodec {
    registry_type_origin_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthCodec {
    fn new(
        registry_type_origin_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        Self {
            registry_type_origin_pkg_id,
            network_auth_object_id,
        }
    }

    fn binding_object_id(&self, identity: &IdentityKey) -> Result<sui::types::Address, NexusError> {
        crate::move_bindings::derive_network_auth_binding_id(
            self.registry_type_origin_pkg_id,
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
                    sui_framework::{object::UID, table::Table as MoveTable, vec_set::VecSet},
                },
                test_utils::{nexus_mocks, sui_mocks},
            },
            serde::Serialize,
            tonic::Response,
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
        ) -> (NetworkAuth, NetworkAuthInnerV1) {
            (
                NetworkAuth::new(UID::new(id)),
                NetworkAuthInnerV1::new(
                    UID::new(sui::types::Address::from_static("0x123")),
                    VecSet {
                        contents: identities,
                    },
                ),
            )
        }

        fn raw_key_binding_for_test(
            id: sui::types::Address,
            identity: IdentityKey,
            next_key_id: u64,
            active_key_id: Option<u64>,
            keys: MoveTable<u64, KeyRecord>,
        ) -> (KeyBinding, KeyBindingInnerV1) {
            (
                KeyBinding::new(UID::new(id)),
                KeyBindingInnerV1::new_for_test(identity, None, next_key_id, active_key_id, keys),
            )
        }

        fn standalone_context(
            registry_package: sui::types::Address,
            network_auth: sui::types::Address,
        ) -> NexusContext {
            let mut objects = sui_mocks::mock_nexus_objects();
            objects.network_auth = crate::types::SharedRoot::new(network_auth, 1);
            let mut packages = sui_mocks::mock_nexus_packages();
            let registry = packages.registry.as_mut().unwrap();
            let previous_origin = registry.initial_id;
            registry.initial_id = registry_package;
            registry.storage_id = registry_package;
            for datatypes in registry.type_origins.values_mut() {
                for origin in datatypes.values_mut() {
                    if *origin == previous_origin {
                        *origin = registry_package;
                    }
                }
            }
            NexusContext::new(Arc::new(objects), packages)
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

        fn mock_key_record_field(
            ledger_service: &mut sui_mocks::grpc::MockLedgerService,
            key_table_id: sui::types::Address,
            key_id: u64,
            record: KeyRecord,
            times: usize,
        ) {
            let field_id = key_table_id.derive_dynamic_child_id(
                &sui::types::TypeTag::U64,
                &bcs::to_bytes(&key_id).expect("key ID serializes"),
            );
            let field = DynamicFieldValueBcs {
                id: field_id,
                name: key_id,
                value: record,
            };
            let field_bytes = bcs::to_bytes(&field).expect("key record field serializes");

            for _ in 0..times {
                sui_mocks::grpc::mock_get_object_bcs(
                    ledger_service,
                    sui_mocks::object_ref_for_id(field_id),
                    sui::types::Owner::Object(key_table_id),
                    field_bytes.clone(),
                );
            }
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
            let (binding, binding_state) = raw_key_binding_for_test(
                binding_object_id,
                identity,
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );

            let context = standalone_context(registry_pkg_id, network_auth_object_id);
            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();
            sui_mocks::grpc::mock_object_state::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(binding_object_id),
                sui::types::Owner::Shared(1),
                binding,
                binding_state,
            );
            sui_mocks::grpc::mock_nexus_package_graph(
                &mut ledger_service,
                &mut package_service,
                context.packages(),
            );

            mock_key_record_field(&mut ledger_service, key_table_id, active_kid, record, 1);

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                package_service_mock: Some(package_service),
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
            let (binding, binding_state) = raw_key_binding_for_test(
                binding_object_id,
                identity.clone(),
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );

            let (network_auth, network_auth_state) = raw_network_auth_for_test(
                network_auth_object_id,
                vec![
                    identity.clone(),
                    IdentityKey::tool(sui::types::Address::from_static("0x42")),
                ],
            );

            let context = standalone_context(registry_pkg_id, network_auth_object_id);
            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();
            sui_mocks::grpc::mock_object_state::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(network_auth_object_id),
                sui::types::Owner::Shared(1),
                network_auth,
                network_auth_state,
            );
            sui_mocks::grpc::mock_object_state::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(binding_object_id),
                sui::types::Owner::Shared(1),
                binding,
                binding_state,
            );
            sui_mocks::grpc::mock_nexus_package_graph(
                &mut ledger_service,
                &mut package_service,
                context.packages(),
            );

            mock_key_record_field(&mut ledger_service, key_table_id, active_kid, record, 1);

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                package_service_mock: Some(package_service),
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
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);
            let mut nexus_objects = sui_mocks::mock_nexus_objects();
            nexus_objects.network_auth = crate::types::SharedRoot::new(network_auth_object_id, 1);
            let context = sui_mocks::mock_nexus_context_for(&nexus_objects);
            let registry_pkg_id = context
                .require_package(PackageRole::Registry)
                .unwrap()
                .initial_id;

            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let active_kid = 3u64;
            let public_key = [7u8; 32];
            let record = KeyRecord::new_for_test(0, public_key.to_vec(), 0, None);

            let key_table_id = sui::types::Address::from_static("0x111");
            let (binding, binding_state) = raw_key_binding_for_test(
                binding_object_id,
                identity.clone(),
                active_kid + 1,
                Some(active_kid),
                MoveTable::new(key_table_id, 1),
            );

            let (network_auth, network_auth_state) = raw_network_auth_for_test(
                network_auth_object_id,
                vec![
                    identity.clone(),
                    IdentityKey::tool(sui::types::Address::from_static("0x42")),
                ],
            );

            let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
            let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();
            sui_mocks::grpc::mock_object_state::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(network_auth_object_id),
                sui::types::Owner::Shared(1),
                network_auth,
                network_auth_state,
            );
            sui_mocks::grpc::mock_object_state::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(binding_object_id),
                sui::types::Owner::Shared(1),
                binding,
                binding_state,
            );
            sui_mocks::grpc::mock_nexus_package_graph(
                &mut ledger_service,
                &mut package_service,
                context.packages(),
            );

            mock_key_record_field(&mut ledger_service, key_table_id, active_kid, record, 2);

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service),
                package_service_mock: Some(package_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            let client =
                nexus_mocks::mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await;

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
            let network_auth_object_id = sui::types::Address::generate(&mut rng);

            let tool_fqn_str = "xyz.demo.tool@1";
            let tool_fqn: crate::ToolFqn = tool_fqn_str.parse().unwrap();
            let tool_registry_id = sui::types::Address::generate(&mut rng);
            let mut nexus_objects = sui_mocks::mock_nexus_objects();
            nexus_objects.network_auth = crate::types::SharedRoot::new(network_auth_object_id, 1);
            nexus_objects.tool_registry = crate::types::SharedRoot::new(tool_registry_id, 1);
            let context = sui_mocks::mock_nexus_context_for(&nexus_objects);
            let registry_pkg_id = context
                .require_package(PackageRole::Registry)
                .unwrap()
                .initial_id;

            let codec = NetworkAuthCodec::new(registry_pkg_id, network_auth_object_id);
            let identity = IdentityKey::tool(
                Tool::derive_id(tool_registry_id, &tool_fqn).expect("Tool ID derives"),
            );
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let key_table_id = sui::types::Address::from_static("0x111");

            // Two keys: kid=0 (revoked), kid=1 (active).
            let record_0 = KeyRecord::new_for_test(0, vec![0xaau8; 32], 1000, Some(2000));
            let record_1 = KeyRecord::new_for_test(0, vec![0xbbu8; 32], 3000, None);

            let (binding, binding_state) = raw_key_binding_for_test(
                binding_object_id,
                identity.clone(),
                2,
                Some(1),
                MoveTable::new(key_table_id, 2),
            );

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
            let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
            let mut state_service = sui_mocks::grpc::MockStateService::new();
            let (network_auth, network_auth_state) =
                raw_network_auth_for_test(network_auth_object_id, vec![]);
            sui_mocks::grpc::mock_object_state::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(network_auth_object_id),
                sui::types::Owner::Shared(1),
                network_auth,
                network_auth_state,
            );

            sui_mocks::grpc::mock_object_state::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
                &mut ledger_service,
                &mut state_service,
                &context,
                sui_mocks::object_ref_for_id(binding_object_id),
                sui::types::Owner::Shared(1),
                binding,
                binding_state,
            );
            sui_mocks::grpc::mock_nexus_package_graph(
                &mut ledger_service,
                &mut package_service,
                context.packages(),
            );

            // list_dynamic_fields: returns two field entries (kid=0 and kid=1).
            // Return kid=1 first to verify the sort.
            let expected_key_table = key_table_id.to_string();
            state_service
                .expect_list_dynamic_fields()
                .withf(move |request| {
                    request.get_ref().parent.as_deref() == Some(expected_key_table.as_str())
                })
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
                package_service_mock: Some(package_service),
                state_service_mock: Some(state_service),
                ..Default::default()
            });

            let client =
                nexus_mocks::mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await;

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
