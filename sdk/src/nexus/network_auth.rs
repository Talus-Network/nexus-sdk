//! Tool-focused helpers for `nexus_workflow::network_auth`.
//!
//! This module is designed for tool operators and other off-chain clients that need to:
//! - register/rotate a ToolId message-signing key on-chain, and
//! - export a tool-side allowlist of permitted Leaders (public keys) for the signed HTTP runtime.
//!
//! # Background: what is registered on-chain?
//! `nexus_workflow::network_auth` binds an off-chain identity (Leader address or Tool FQN) to an
//! Ed25519 public key used for signed HTTP.
//!
//! Registration requires a proof-of-possession (PoP) signature:
//! `POP_DOMAIN || bcs(IdentityKey) || bcs(key_id) || public_key`
//!
//! Where `key_id` is the binding's current `next_key_id` (making PoP signatures one-time-use).
//!
//! # Tool runtime (no RPC)
//! Tools must not perform RPC calls at runtime. Instead, a tool operator can call
//! [`NetworkAuthActions::export_allowed_leaders_file_v1`] (or the `write_*` helper) to produce an
//! `allowed_leaders.json` file consumed by `nexus-toolkit`.

use {
    crate::{
        idents::workflow,
        nexus::{client::NexusClient, crawler::Crawler, error::NexusError},
        signed_http::v1::wire::{
            AllowedLeaderFileV1,
            AllowedLeaderKeyFileV1,
            AllowedLeadersFileV1,
        },
        sui,
        transactions,
        types::{IdentityKey, KeyBinding, NetworkAuth, Tool},
        ToolFqn,
    },
    ed25519_dalek::{Signature, Signer as _, SigningKey},
    std::{
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    },
    tokio::sync::Mutex,
};

const POP_DOMAIN_V1: &[u8] = b"nexus_workflow.network_auth.pop_v1";
const KEY_SCHEME_ED25519: u8 = 0;

/// Result returned after registering a ToolId message-signing key.
#[derive(Clone, Debug)]
pub struct RegisteredToolKey {
    /// Transaction digest that performed the registration.
    pub tx_digest: sui::types::Digest,
    /// Tool identifier (FQN string form).
    pub tool_id: ToolFqn,
    /// Registered key id (kid) that must be used in signed HTTP claims.
    pub tool_kid: u64,
    /// Registered Ed25519 public key bytes.
    pub public_key: [u8; 32],
    /// Deterministic binding object ID under the on-chain `NetworkAuth` registry.
    pub binding_object_id: sui::types::Address,
}

pub struct NetworkAuthActions {
    pub(super) client: NexusClient,
}

impl NetworkAuthActions {
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
            NetworkAuthCodec::new(objects.workflow_pkg_id, *objects.network_auth.object_id());

        let identity = IdentityKey::tool_fqn(&tool_fqn.to_string());
        let binding_object_id = codec.binding_object_id(&identity)?;

        let binding = self.try_get_key_binding(binding_object_id).await?;
        let (binding_ref, next_key_id) = match binding {
            None => (None, 0),
            Some(b) => (Some(b.object_ref()), b.data.next_key_id),
        };

        let public_key = tool_signing_key.verifying_key().to_bytes();
        let pop_msg = codec.pop_message_v1(&identity, next_key_id, public_key)?;
        let pop_sig = sign_bytes(&tool_signing_key, &pop_msg);

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
        let tool_id =
            Tool::derive_id(*objects.tool_registry.object_id(), &tool_fqn).map_err(|e| {
                NexusError::Parsing(anyhow::anyhow!(
                    "failed to derive ToolId for FQN '{tool_fqn}': {e}"
                ))
            })?;

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

        // Craft and submit tx.
        let mut tx = sui::tx::TransactionBuilder::new();

        match binding_ref {
            None => transactions::network_auth::create_tool_binding_and_register_key(
                &mut tx,
                objects,
                address,
                &tool,
                &owner_cap_ref,
                public_key,
                pop_sig,
                description,
            )
            .map_err(NexusError::TransactionBuilding)?,
            Some(binding_ref) => transactions::network_auth::register_tool_key_on_existing_binding(
                &mut tx,
                objects,
                &binding_ref,
                &tool,
                &owner_cap_ref,
                public_key,
                pop_sig,
            )
            .map_err(NexusError::TransactionBuilding)?,
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(RegisteredToolKey {
            tx_digest: response.digest,
            tool_id: tool_fqn,
            tool_kid: next_key_id,
            public_key,
            binding_object_id,
        })
    }

    /// Export a tool-side allowlist file containing the active key for each leader.
    ///
    /// The returned JSON schema matches `nexus_sdk::signed_http::v1::AllowedLeadersFileV1`
    /// and can be written to disk and mounted into `nexus-toolkit`.
    ///
    /// `leader_cap_ids` are leader capability ID (`leader_cap::OverNetwork` object IDs)
    pub async fn export_allowed_leaders_file_v1(
        &self,
        leader_cap_ids: &[sui::types::Address],
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let objects = &self.client.nexus_objects;
        let codec =
            NetworkAuthCodec::new(objects.workflow_pkg_id, *objects.network_auth.object_id());

        let mut out = Vec::with_capacity(leader_cap_ids.len());
        for leader_cap_id in leader_cap_ids {
            let identity = IdentityKey::leader(*leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity)?;

            let binding = self
                .client
                .crawler()
                .get_object_contents_bcs::<KeyBinding>(binding_object_id)
                .await
                .map_err(|e| {
                    NexusError::Rpc(anyhow::anyhow!(
                        "failed to fetch leader KeyBinding ({binding_object_id}): {e}"
                    ))
                })?;

            let active_kid = binding.data.active_key_id.ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} has no active key"
                ))
            })?;

            let keys = self
                .client
                .crawler()
                .get_dynamic_fields_bcs::<u64, crate::types::KeyRecord>(
                    binding.data.keys.id,
                    binding.data.keys.size(),
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

    /// Convenience helper to write an allowlist file to disk as pretty JSON.
    pub async fn write_allowed_leaders_file_v1(
        &self,
        leader_cap_ids: &[sui::types::Address],
        path: impl AsRef<Path>,
    ) -> Result<(), NexusError> {
        let file = self.export_allowed_leaders_file_v1(leader_cap_ids).await?;
        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to serialize allowlist: {e}"))
        })?;
        std::fs::write(path, bytes).map_err(|e| NexusError::Parsing(e.into()))?;
        Ok(())
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
            .get_object_contents_bcs::<NetworkAuth>(network_auth_object_id)
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
            .into_iter()
            .filter_map(|id| match id {
                IdentityKey::Leader { leader_cap_id } => Some(leader_cap_id),
                _ => None,
            })
            .collect::<Vec<_>>();

        out.sort_unstable();
        out.dedup();

        Ok(out)
    }

    /// Export a tool-side allowlist file containing the active key for every Leader identity
    /// found in `network_auth.identities`.
    ///
    /// Leaders that do not have an active key are skipped.
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
            NetworkAuthCodec::new(objects.workflow_pkg_id, *objects.network_auth.object_id());

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

    /// Convenience helper to write an allowlist file for all leaders to disk as pretty JSON.
    pub async fn write_allowed_leaders_file_v1_for_all_leaders(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), NexusError> {
        let file = self
            .export_allowed_leaders_file_v1_for_all_leaders()
            .await?;
        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to serialize allowlist: {e}"))
        })?;
        std::fs::write(path, bytes).map_err(|e| NexusError::Parsing(e.into()))?;
        Ok(())
    }

    async fn try_get_key_binding(
        &self,
        binding_object_id: sui::types::Address,
    ) -> Result<Option<crate::nexus::crawler::Response<KeyBinding>>, NexusError> {
        match self
            .client
            .crawler()
            .get_object_contents_bcs::<KeyBinding>(binding_object_id)
            .await
        {
            Ok(obj) => Ok(Some(obj)),
            Err(e) if e.to_string().contains("not found") => Ok(None),
            Err(e) => Err(NexusError::Rpc(e)),
        }
    }

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
            .get_object_contents_bcs::<KeyBinding>(binding_object_id)
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch leader KeyBinding ({binding_object_id}): {e}"
                ))
            })?;

        let Some(active_kid) = binding.data.active_key_id else {
            return Ok(None);
        };

        let keys = self
            .client
            .crawler()
            .get_dynamic_fields_bcs::<u64, crate::types::KeyRecord>(
                binding.data.keys.id,
                binding.data.keys.size(),
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
    workflow_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthReader {
    pub fn new(
        crawler: Crawler,
        workflow_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        Self {
            crawler,
            workflow_pkg_id,
            network_auth_object_id,
        }
    }

    /// Construct a reader by creating a Sui gRPC client for `rpc_url`.
    pub fn from_rpc_url(
        rpc_url: &str,
        workflow_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Result<Self, NexusError> {
        let client = sui::grpc::Client::new(rpc_url).map_err(|e| NexusError::Rpc(e.into()))?;
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));
        Ok(Self::new(crawler, workflow_pkg_id, network_auth_object_id))
    }

    /// List the leader capability IDs currently present in `network_auth.identities`.
    pub async fn list_leader_cap_ids_from_network_auth(
        &self,
    ) -> Result<Vec<sui::types::Address>, NexusError> {
        let registry = self
            .crawler
            .get_object_contents_bcs::<NetworkAuth>(self.network_auth_object_id)
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
            .into_iter()
            .filter_map(|id| match id {
                IdentityKey::Leader { leader_cap_id } => Some(leader_cap_id),
                _ => None,
            })
            .collect::<Vec<_>>();

        out.sort_unstable();
        out.dedup();

        Ok(out)
    }

    /// Export a tool-side allowlist file containing the active key for every Leader identity
    /// found in `network_auth.identities`.
    ///
    /// Leaders that do not have an active key are skipped.
    pub async fn export_allowed_leaders_file_v1_for_all_leaders(
        &self,
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let leader_cap_ids = self.list_leader_cap_ids_from_network_auth().await?;
        if leader_cap_ids.is_empty() {
            return Err(NexusError::Parsing(anyhow::anyhow!(
                "network_auth contains no leader identities"
            )));
        }

        let codec = NetworkAuthCodec::new(self.workflow_pkg_id, self.network_auth_object_id);

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

    /// Convenience helper to write an allowlist file for all leaders to disk as pretty JSON.
    pub async fn write_allowed_leaders_file_v1_for_all_leaders(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), NexusError> {
        let file = self
            .export_allowed_leaders_file_v1_for_all_leaders()
            .await?;
        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to serialize allowlist: {e}"))
        })?;
        std::fs::write(path, bytes).map_err(|e| NexusError::Parsing(e.into()))?;
        Ok(())
    }

    async fn export_allowed_leader_entry_file_v1(
        &self,
        codec: &NetworkAuthCodec,
        leader_cap_id: sui::types::Address,
    ) -> Result<Option<AllowedLeaderFileV1>, NexusError> {
        let identity = IdentityKey::leader(leader_cap_id);
        let binding_object_id = codec.binding_object_id(&identity)?;

        let binding = self
            .crawler
            .get_object_contents_bcs::<KeyBinding>(binding_object_id)
            .await
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "failed to fetch leader KeyBinding ({binding_object_id}): {e}"
                ))
            })?;

        let Some(active_kid) = binding.data.active_key_id else {
            return Ok(None);
        };

        let keys = self
            .crawler
            .get_dynamic_fields_bcs::<u64, crate::types::KeyRecord>(
                binding.data.keys.id,
                binding.data.keys.size(),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllowedLeadersSyncOutcome {
    Unchanged,
    Updated,
}

/// Periodically refresh a tool-side `allowed-leaders.json` file from on-chain `network_auth`.
///
/// This is intended to run outside of the tool request path. Tools can then
/// hot-reload allowlist updates via `nexus-toolkit`'s config watcher.
#[derive(Clone)]
pub struct AllowedLeadersFileSyncerV1 {
    reader: NetworkAuthReader,
    out_path: PathBuf,
}

impl AllowedLeadersFileSyncerV1 {
    pub fn new(reader: NetworkAuthReader, out_path: impl Into<PathBuf>) -> Self {
        Self {
            reader,
            out_path: out_path.into(),
        }
    }

    pub fn out_path(&self) -> &Path {
        &self.out_path
    }

    /// Export and (atomically) write the latest allowlist if it differs from the current file.
    pub async fn sync_once(&self) -> Result<AllowedLeadersSyncOutcome, NexusError> {
        let file = self
            .reader
            .export_allowed_leaders_file_v1_for_all_leaders()
            .await?;

        let bytes = serde_json::to_vec_pretty(&file).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to serialize allowlist: {e}"))
        })?;

        match std::fs::read(&self.out_path) {
            Ok(existing) if existing == bytes => return Ok(AllowedLeadersSyncOutcome::Unchanged),
            Ok(_) | Err(_) => {}
        }

        atomic_write(&self.out_path, &bytes).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!(
                "failed to write {}: {e}",
                self.out_path.display()
            ))
        })?;

        Ok(AllowedLeadersSyncOutcome::Updated)
    }

    /// Run `sync_once()` in a loop, sleeping `poll_interval` between iterations.
    pub async fn run(&self, poll_interval: Duration) -> Result<(), NexusError> {
        loop {
            self.sync_once().await?;
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Run `sync_once()` in a loop, swallowing transient errors.
    ///
    /// This is the recommended mode for long-running processes: on any error
    /// (RPC, parsing, IO), the syncer waits `poll_interval` and tries again.
    pub async fn run_best_effort(&self, poll_interval: Duration) {
        loop {
            let _ = self.sync_once().await;
            tokio::time::sleep(poll_interval).await;
        }
    }
}

/// Internal helper that knows how to compute binding ids and PoP bytes.
struct NetworkAuthCodec {
    workflow_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
}

impl NetworkAuthCodec {
    fn new(
        workflow_pkg_id: sui::types::Address,
        network_auth_object_id: sui::types::Address,
    ) -> Self {
        Self {
            workflow_pkg_id,
            network_auth_object_id,
        }
    }

    fn binding_object_id(&self, identity: &IdentityKey) -> Result<sui::types::Address, NexusError> {
        let key_type =
            workflow::into_type_tag(self.workflow_pkg_id, workflow::NetworkAuth::IDENTITY_KEY);
        let key_bcs = bcs::to_bytes(identity).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to BCS-encode IdentityKey: {e}"))
        })?;
        Ok(self
            .network_auth_object_id
            .derive_object_id(&key_type, &key_bcs))
    }

    fn pop_message_v1(
        &self,
        identity: &IdentityKey,
        key_id: u64,
        public_key: [u8; 32],
    ) -> Result<Vec<u8>, NexusError> {
        let mut out = Vec::new();
        out.extend_from_slice(POP_DOMAIN_V1);
        out.extend_from_slice(&bcs::to_bytes(identity).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to BCS-encode IdentityKey: {e}"))
        })?);
        out.extend_from_slice(&bcs::to_bytes(&key_id).map_err(|e| {
            NexusError::Parsing(anyhow::anyhow!("failed to BCS-encode key_id: {e}"))
        })?);
        out.extend_from_slice(&public_key);
        Ok(out)
    }
}

fn sign_bytes(signing_key: &SigningKey, msg: &[u8]) -> [u8; 64] {
    let sig: Signature = signing_key.sign(msg);
    sig.to_bytes()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;

    let base = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "allowed-leaders.json".to_string());

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_nanos(0))
        .as_nanos();
    let pid = std::process::id();
    let tmp = parent.join(format!(".{base}.{pid}.{nanos}.tmp"));

    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_object_id_is_deterministic_and_distinct() {
        let mut rng = rand::thread_rng();
        let workflow_pkg_id = sui::types::Address::generate(&mut rng);
        let network_auth_object_id = sui::types::Address::generate(&mut rng);
        let codec = NetworkAuthCodec::new(workflow_pkg_id, network_auth_object_id);

        let leader = IdentityKey::leader(sui::types::Address::generate(&mut rng));
        let tool = IdentityKey::tool_fqn("xyz.demo.tool@1");

        let leader_id_first = codec.binding_object_id(&leader).unwrap();
        let leader_id_second = codec.binding_object_id(&leader).unwrap();
        let tool_id = codec.binding_object_id(&tool).unwrap();

        assert_eq!(leader_id_first, leader_id_second);
        assert_ne!(leader_id_first, tool_id);
    }

    #[test]
    fn pop_message_v1_matches_expected_layout() {
        let mut rng = rand::thread_rng();
        let workflow_pkg_id = sui::types::Address::generate(&mut rng);
        let network_auth_object_id = sui::types::Address::generate(&mut rng);
        let codec = NetworkAuthCodec::new(workflow_pkg_id, network_auth_object_id);

        let identity = IdentityKey::tool_fqn("xyz.demo.tool@1");
        let key_id = 7u64;
        let public_key = [9u8; 32];

        let msg = codec.pop_message_v1(&identity, key_id, public_key).unwrap();

        let mut expected = Vec::new();
        expected.extend_from_slice(POP_DOMAIN_V1);
        expected.extend_from_slice(&bcs::to_bytes(&identity).unwrap());
        expected.extend_from_slice(&bcs::to_bytes(&key_id).unwrap());
        expected.extend_from_slice(&public_key);

        assert_eq!(msg, expected);
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
                test_utils::sui_mocks,
                types::{KeyRecord, MoveTable, MoveVecSet},
            },
            serde::Serialize,
            tonic::{Response, Status},
        };

        #[derive(Clone, Debug, Serialize)]
        struct NetworkAuthBcs {
            id: sui::types::Address,
            identities: MoveVecSet<IdentityKey>,
        }

        #[derive(Clone, Debug, Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
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

        async fn build_reader_and_syncer(
            out_path: PathBuf,
            workflow_pkg_id: sui::types::Address,
            network_auth_object_id: sui::types::Address,
            leader_cap_id: sui::types::Address,
            active_kid: u64,
            record: KeyRecord,
        ) -> AllowedLeadersFileSyncerV1 {
            let codec = NetworkAuthCodec::new(workflow_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let key_table_id = sui::types::Address::from_static("0x111");
            let binding = KeyBinding {
                id: sui::types::Address::from_static("0x222"),
                identity: identity.clone(),
                description: None,
                next_key_id: active_kid + 1,
                active_key_id: Some(active_kid),
                keys: MoveTable::new(key_table_id, 1),
            };

            let network_auth = NetworkAuthBcs {
                id: network_auth_object_id,
                identities: MoveVecSet {
                    contents: vec![identity.clone(), IdentityKey::tool_fqn("xyz.demo.tool@1")],
                },
            };

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

            let reader =
                NetworkAuthReader::from_rpc_url(&rpc_url, workflow_pkg_id, network_auth_object_id)
                    .unwrap();
            AllowedLeadersFileSyncerV1::new(reader, out_path)
        }

        #[tokio::test]
        async fn actions_export_and_write_allowlists() {
            let mut rng = rand::thread_rng();
            let workflow_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);

            let codec = NetworkAuthCodec::new(workflow_pkg_id, network_auth_object_id);
            let identity = IdentityKey::leader(leader_cap_id);
            let binding_object_id = codec.binding_object_id(&identity).unwrap();

            let active_kid = 3u64;
            let public_key = [7u8; 32];
            let record = KeyRecord {
                scheme: 0,
                public_key: public_key.to_vec(),
                added_at_ms: 0,
                revoked_at_ms: None,
            };

            let key_table_id = sui::types::Address::from_static("0x111");
            let binding = KeyBinding {
                id: sui::types::Address::from_static("0x222"),
                identity: identity.clone(),
                description: None,
                next_key_id: active_kid + 1,
                active_key_id: Some(active_kid),
                keys: MoveTable::new(key_table_id, 1),
            };

            let network_auth = NetworkAuthBcs {
                id: network_auth_object_id,
                identities: MoveVecSet {
                    contents: vec![identity.clone(), IdentityKey::tool_fqn("xyz.demo.tool@1")],
                },
            };

            let network_auth_bytes = bcs::to_bytes(&network_auth).unwrap();
            let binding_bytes = bcs::to_bytes(&binding).unwrap();

            let field_object_id = sui::types::Address::from_static("0x333");
            let field_value = DynamicFieldValueBcs {
                id: sui::types::Address::from_static("0x444"),
                name: active_kid,
                value: record.clone(),
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
                .times(7)
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
                .times(4)
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
                .times(4)
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

            let nexus_objects = crate::types::NexusObjects {
                workflow_pkg_id,
                primitives_pkg_id: sui::types::Address::generate(&mut rng),
                interface_pkg_id: sui::types::Address::generate(&mut rng),
                network_id: sui::types::Address::generate(&mut rng),
                tool_registry: sui_mocks::mock_sui_object_ref(),
                network_auth: sui::types::ObjectReference::new(
                    network_auth_object_id,
                    1,
                    sui::types::Digest::generate(&mut rng),
                ),
                default_tap: sui_mocks::mock_sui_object_ref(),
                gas_service: sui_mocks::mock_sui_object_ref(),
                leader_registry: sui_mocks::mock_sui_object_ref(),
            };

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

            let out_dir = tempfile::tempdir().unwrap();
            let out_one = out_dir.path().join("one.json");
            client
                .network_auth()
                .write_allowed_leaders_file_v1(&[leader_cap_id], &out_one)
                .await
                .unwrap();

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

            let out_all = out_dir.path().join("all.json");
            client
                .network_auth()
                .write_allowed_leaders_file_v1_for_all_leaders(&out_all)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn syncer_writes_allowlist_when_missing() {
            let mut rng = rand::thread_rng();
            let workflow_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);

            let out_dir = tempfile::tempdir().unwrap();
            let out_path = out_dir.path().join("allowed-leaders.json");

            let active_kid = 7u64;
            let record = KeyRecord {
                scheme: 0,
                public_key: vec![9u8; 32],
                added_at_ms: 0,
                revoked_at_ms: None,
            };

            let syncer = build_reader_and_syncer(
                out_path.clone(),
                workflow_pkg_id,
                network_auth_object_id,
                leader_cap_id,
                active_kid,
                record,
            )
            .await;

            assert_eq!(syncer.out_path(), out_path.as_path());

            let outcome = syncer.sync_once().await.unwrap();
            assert_eq!(outcome, AllowedLeadersSyncOutcome::Updated);

            let bytes = std::fs::read(&out_path).unwrap();
            let allowlist: AllowedLeadersFileV1 = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(allowlist.version, 1);
            assert_eq!(allowlist.leaders.len(), 1);
            assert_eq!(allowlist.leaders[0].leader_id, leader_cap_id.to_string());
            assert_eq!(allowlist.leaders[0].keys.len(), 1);
            assert_eq!(allowlist.leaders[0].keys[0].kid, active_kid);
            assert_eq!(
                allowlist.leaders[0].keys[0].public_key,
                hex::encode([9u8; 32])
            );
        }

        #[tokio::test]
        async fn syncer_returns_unchanged_when_file_matches() {
            let mut rng = rand::thread_rng();
            let workflow_pkg_id = sui::types::Address::generate(&mut rng);
            let network_auth_object_id = sui::types::Address::generate(&mut rng);
            let leader_cap_id = sui::types::Address::generate(&mut rng);

            let out_dir = tempfile::tempdir().unwrap();
            let out_path = out_dir.path().join("allowed-leaders.json");

            let active_kid = 1u64;
            let public_key = [4u8; 32];
            let record = KeyRecord {
                scheme: 0,
                public_key: public_key.to_vec(),
                added_at_ms: 0,
                revoked_at_ms: None,
            };

            let expected = AllowedLeadersFileV1 {
                version: 1,
                leaders: vec![AllowedLeaderFileV1 {
                    leader_id: leader_cap_id.to_string(),
                    keys: vec![AllowedLeaderKeyFileV1 {
                        kid: active_kid,
                        public_key: hex::encode(public_key),
                    }],
                }],
            };
            let expected_bytes = serde_json::to_vec_pretty(&expected).unwrap();
            std::fs::write(&out_path, &expected_bytes).unwrap();

            let syncer = build_reader_and_syncer(
                out_path.clone(),
                workflow_pkg_id,
                network_auth_object_id,
                leader_cap_id,
                active_kid,
                record,
            )
            .await;

            let outcome = syncer.sync_once().await.unwrap();
            assert_eq!(outcome, AllowedLeadersSyncOutcome::Unchanged);

            let bytes = std::fs::read(&out_path).unwrap();
            assert_eq!(bytes, expected_bytes);
        }
    }
}
