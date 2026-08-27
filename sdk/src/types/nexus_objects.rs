//! Stable Nexus environment identities.

use {
    crate::{
        move_bindings::{sui_framework::coin::Coin as MoveCoin, talus::us::US},
        sui,
    },
    serde::{Deserialize, Serialize},
    talus_sui_move::{MoveStruct, MoveType},
};

/// Stable identity of one Sui object.
///
/// This type is used for capabilities whose current object version and digest
/// are not part of environment identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectIdentity {
    /// Stable Sui object ID.
    pub object_id: sui::types::Address,
}

impl ObjectIdentity {
    /// Creates an identity for `object_id`.
    pub const fn new(object_id: sui::types::Address) -> Self {
        Self { object_id }
    }

    /// Returns the stable Sui object ID.
    pub const fn object_id(&self) -> sui::types::Address {
        self.object_id
    }
}

/// Stable identity and initial shared version of one canonical root.
///
/// A [`sui::types::ObjectReference`] cannot represent this invariant because
/// it includes the current mutable version and digest. A shared transaction
/// input instead requires the stable object ID and the version at which the
/// object first became shared.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharedRoot {
    /// Stable Sui object ID.
    pub object_id: sui::types::Address,
    /// Version at which the object first became shared.
    pub initial_shared_version: u64,
}

impl SharedRoot {
    /// Creates a canonical root identity.
    pub const fn new(object_id: sui::types::Address, initial_shared_version: u64) -> Self {
        Self {
            object_id,
            initial_shared_version,
        }
    }

    /// Returns the stable Sui object ID.
    pub const fn object_id(&self) -> sui::types::Address {
        self.object_id
    }
}

/// Stable identities for the external US token.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsTokenConfig {
    /// Package that defines [`US`].
    pub package_id: sui::types::Address,
    /// Protected treasury object for [`US`].
    pub protected_treasury: sui::types::Address,
    /// Coin metadata object for [`US`].
    pub metadata: sui::types::Address,
}

impl UsTokenConfig {
    /// Creates the complete external US token identity.
    pub const fn new(
        package_id: sui::types::Address,
        protected_treasury: sui::types::Address,
        metadata: sui::types::Address,
    ) -> Self {
        Self {
            package_id,
            protected_treasury,
            metadata,
        }
    }

    /// Returns the configured [`US`] type tag.
    pub fn type_tag(&self) -> sui::types::TypeTag {
        crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::type_tag_static,
        )
    }

    /// Returns the configured `Coin<US>` struct tag.
    pub fn coin_type_tag(&self) -> sui::types::StructTag {
        crate::move_bindings::sui_framework::with_packages(
            sui::types::Address::from_static("0x2"),
            sui::types::Address::from_static("0x2"),
            || {
                crate::move_bindings::talus::with_packages(
                    self.package_id,
                    self.package_id,
                    MoveCoin::<US>::struct_tag_static,
                )
            },
        )
    }

    /// Returns the fully qualified configured [`US`] type name.
    pub fn qualified_type(&self) -> String {
        let tag = crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::struct_tag_static,
        );
        format!("{}::{}::{}", tag.address(), tag.module(), tag.name())
    }
}

/// Stable identities required to operate in one Nexus environment.
///
/// Package authority and live derived values are intentionally absent. An
/// operation resolves those values from object witnesses and immutable package
/// metadata when it needs them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NexusObjects {
    /// Complete genesis checkpoint digest expected from the Sui service.
    pub chain_id: String,
    /// Stable Nexus network identity.
    pub network_id: sui::types::Address,
    /// Canonical [`ToolRegistry`](crate::move_bindings::tool::tool_registry::ToolRegistry) root.
    pub tool_registry: SharedRoot,
    /// Canonical [`NetworkAuth`](crate::move_bindings::registry::network_auth::NetworkAuth) root.
    pub network_auth: SharedRoot,
    /// Canonical [`AgentRegistry`](crate::move_bindings::registry::agent_registry::AgentRegistry)
    /// root.
    pub agent_registry: SharedRoot,
    /// Canonical [`LeaderRegistry`](crate::move_bindings::registry::leader::LeaderRegistry) root.
    pub leader_registry: SharedRoot,
    /// Canonical priority fee vault root.
    pub priority_fee_vault: SharedRoot,
    /// Fixed authority selecting the Scheduler package allowed to produce protocol effects.
    pub runtime_authority: SharedRoot,
    /// Authority that manages leader capabilities.
    pub leader_admin_cap: ObjectIdentity,
    /// Authority that manages the canonical [`ToolRegistry`](crate::move_bindings::tool::tool_registry::ToolRegistry).
    pub tool_registry_admin_cap: ObjectIdentity,
    /// Authority that records slashing decisions.
    pub slashing_cap: ObjectIdentity,
    /// Authority that manages the canonical priority fee vault.
    pub priority_fee_vault_owner_cap: ObjectIdentity,
    /// Leader capability created during initial publication.
    pub initial_leader_cap: ObjectIdentity,
    /// Authority for emergency pause and WorkAdmission policy changes.
    pub runtime_authority_cap: ObjectIdentity,
    /// External [`US`] token identities.
    pub us_token: UsTokenConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENVIRONMENT: &str = r#"
chain_id = "test-chain"
network_id = "0x01"

[tool_registry]
object_id = "0x02"
initial_shared_version = 2

[network_auth]
object_id = "0x03"
initial_shared_version = 3

[agent_registry]
object_id = "0x04"
initial_shared_version = 3

[leader_registry]
object_id = "0x05"
initial_shared_version = 3

[priority_fee_vault]
object_id = "0x06"
initial_shared_version = 3

[runtime_authority]
object_id = "0x0f"
initial_shared_version = 3

[leader_admin_cap]
object_id = "0x07"

[tool_registry_admin_cap]
object_id = "0x08"

[slashing_cap]
object_id = "0x09"

[priority_fee_vault_owner_cap]
object_id = "0x0a"

[initial_leader_cap]
object_id = "0x0b"

[runtime_authority_cap]
object_id = "0x10"

[us_token]
package_id = "0x0c"
protected_treasury = "0x0d"
metadata = "0x0e"
"#;

    #[test]
    fn stable_environment_round_trips_through_toml() {
        let objects: NexusObjects = toml::from_str(ENVIRONMENT).unwrap();
        let encoded = toml::to_string(&objects).unwrap();
        let decoded: NexusObjects = toml::from_str(&encoded).unwrap();

        assert_eq!(decoded, objects);
        assert_eq!(objects.tool_registry.initial_shared_version, 2);
        assert_eq!(objects.runtime_authority.object_id, address("0x0f"));
        assert_eq!(objects.us_token.metadata, address("0x0e"));
    }

    #[test]
    fn stable_environment_requires_every_root() {
        let without_root = ENVIRONMENT.replace(
            "[leader_registry]\nobject_id = \"0x05\"\ninitial_shared_version = 3\n\n",
            "",
        );
        let error = toml::from_str::<NexusObjects>(&without_root)
            .unwrap_err()
            .to_string();

        assert!(error.contains("missing field `leader_registry`"));
    }

    #[test]
    fn shared_root_rejects_current_object_truth() {
        let with_digest = ENVIRONMENT.replace(
            "[tool_registry]\nobject_id = \"0x02\"\ninitial_shared_version = 2",
            "[tool_registry]\nobject_id = \"0x02\"\ninitial_shared_version = 2\ndigest = \"mutable\"",
        );
        let error = toml::from_str::<NexusObjects>(&with_digest)
            .unwrap_err()
            .to_string();

        assert!(error.contains("unknown field `digest`"));
    }

    #[test]
    fn stable_environment_rejects_package_authority() {
        let with_packages = format!("{ENVIRONMENT}\n[packages]\nregistry = \"0x0f\"\n");
        let error = toml::from_str::<NexusObjects>(&with_packages)
            .unwrap_err()
            .to_string();

        assert!(error.contains("unknown field `packages`"));
    }

    #[test]
    fn token_type_helpers_use_the_configured_package() {
        let token = UsTokenConfig::new(address("0xc1"), address("0xc2"), address("0xc3"));
        let type_tag = token.type_tag();
        let sui::types::TypeTag::Struct(type_tag) = type_tag else {
            panic!("expected US struct tag");
        };
        assert_eq!(*type_tag.address(), address("0xc1"));
        assert_eq!(
            token.coin_type_tag().type_params().first(),
            Some(&sui::types::TypeTag::Struct(type_tag))
        );
        assert!(token
            .qualified_type()
            .starts_with(&address("0xc1").to_string()));
    }

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }
}
