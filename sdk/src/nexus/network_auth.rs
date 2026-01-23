//! Tool-focused helpers for `nexus_workflow::network_auth`.
//!
//! This module is designed for **tool operators** and other off-chain clients that need to:
//! - register/rotate a ToolId message-signing key on-chain, and
//! - export a tool-side allowlist of permitted Leaders (public keys) for the signed HTTP runtime.
//!
//! It intentionally does **not** implement "leader key registration" flows. Those are part of the
//! Leader service itself (`/be`) and should be managed there.
//!
//! # Background: what is registered on-chain?
//! `nexus_workflow::network_auth` binds an off-chain identity (Leader address or Tool FQN) to an
//! Ed25519 public key used for **signed HTTP**.
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
        nexus::{client::NexusClient, error::NexusError},
        signed_http::v1::wire::{
            AllowedLeaderFileV1,
            AllowedLeaderKeyFileV1,
            AllowedLeadersFileV1,
        },
        sui,
        transactions,
        types::{IdentityKey, KeyBinding},
        ToolFqn,
    },
    ed25519_dalek::{Signature, Signer as _, SigningKey},
    std::path::Path,
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

        // Craft and submit tx.
        let mut tx = sui::tx::TransactionBuilder::new();

        match binding_ref {
            None => transactions::network_auth::create_tool_binding_and_register_key(
                &mut tx,
                objects,
                address,
                &tool_fqn,
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
                &tool_fqn,
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

    /// Export a tool-side allowlist file containing the *active* key for each leader.
    ///
    /// The returned JSON schema matches `nexus_sdk::signed_http::v1::AllowedLeadersFileV1`
    /// and can be written to disk and mounted into `nexus-toolkit`.
    pub async fn export_allowed_leaders_file_v1(
        &self,
        leaders: &[sui::types::Address],
    ) -> Result<AllowedLeadersFileV1, NexusError> {
        let objects = &self.client.nexus_objects;
        let codec =
            NetworkAuthCodec::new(objects.workflow_pkg_id, *objects.network_auth.object_id());

        let mut out = Vec::with_capacity(leaders.len());
        for leader in leaders {
            let identity = IdentityKey::leader(*leader);
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

            let active_kid = binding.data.active_key_id.ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "leader binding {binding_object_id} has no active key"
                ))
            })?;

            let keys = self
                .client
                .crawler()
                .get_dynamic_fields(&binding.data.keys)
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
                leader_id: leader.to_string(),
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
        leaders: &[sui::types::Address],
        path: impl AsRef<Path>,
    ) -> Result<(), NexusError> {
        let file = self.export_allowed_leaders_file_v1(leaders).await?;
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
            .get_object::<KeyBinding>(binding_object_id)
            .await
        {
            Ok(obj) => Ok(Some(obj)),
            Err(e) if e.to_string().contains("not found") => Ok(None),
            Err(e) => Err(NexusError::Rpc(e.into())),
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
}
