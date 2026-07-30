//! [`NexusObjects`] struct is holding the Nexus object IDs and refs that are
//! generated during Nexus package deployment.
#[cfg(test)]
use crate::move_bindings::{
    online_payment::gas as online_payment_gas_move,
    primitives::event as event_move,
    registry::agent_registry as agent_registry_move,
    scheduler::task as scheduler_task_move,
    workflow::execution as execution_move,
};
#[cfg(feature = "nexus")]
use std::sync::Arc;
use {
    crate::{
        move_bindings::{
            interface::{
                agent as agent_move,
                authorization as authorization_move,
                dag as dag_move,
                payment as payment_move,
                version as version_move,
            },
            sui_framework::coin::Coin as MoveCoin,
            talus::us::US,
        },
        sui,
        types::DefaultDagExecutorTarget,
    },
    serde::{Deserialize, Serialize},
    sui_move::{MoveStruct, MoveType},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsTokenConfig {
    pub package_id: sui::types::Address,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_treasury: Option<sui::types::Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<sui::types::Address>,
}

impl Default for UsTokenConfig {
    fn default() -> Self {
        Self::new(sui::types::Address::ZERO)
    }
}

impl UsTokenConfig {
    pub fn new(package_id: sui::types::Address) -> Self {
        Self {
            package_id,
            protected_treasury: None,
            metadata: None,
        }
    }

    pub fn type_tag(&self) -> sui::types::TypeTag {
        crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::type_tag_static,
        )
    }

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

    pub fn qualified_type(&self) -> String {
        let tag = crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::struct_tag_static,
        );
        format!("{}::{}::{}", tag.address(), tag.module(), tag.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusObjects {
    pub online_payment_pkg_id: sui::types::Address,
    pub workflow_pkg_id: sui::types::Address,
    pub scheduler_pkg_id: sui::types::Address,
    pub primitives_pkg_id: sui::types::Address,
    pub interface_pkg_id: sui::types::Address,
    pub network_id: sui::types::Address,
    pub registry_pkg_id: sui::types::Address,
    pub tool_registry: sui::types::ObjectReference,
    pub verifier_registry: sui::types::ObjectReference,
    pub network_auth: sui::types::ObjectReference,
    pub agent_registry: sui::types::ObjectReference,
    pub default_dag_executor: DefaultDagExecutorTarget,
    pub gas_service: sui::types::ObjectReference,
    pub leader_registry: sui::types::ObjectReference,
    pub priority_fee_vault: sui::types::ObjectReference,
    #[serde(default = "default_object_reference")]
    pub priority_fee_vault_owner_cap: sui::types::ObjectReference,
    #[serde(default)]
    pub us_token: UsTokenConfig,

    /// Original package address that defines primitive types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitives_original_pkg_id: Option<sui::types::Address>,
    /// Original package address that defines interface types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_original_pkg_id: Option<sui::types::Address>,
    /// Original package address that defines registry types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_original_pkg_id: Option<sui::types::Address>,
    /// Original package address that defines online payment types.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub online_payment_original_pkg_id: Option<sui::types::Address>,
    /// Original (defining) package address for the workflow package.
    ///
    /// After a Sui Move package upgrade, on-chain types still reference the
    /// original package address in their type tags. This field stores that
    /// address for use in derived object ID computations and type matching.
    ///
    /// When `None`, falls back to `workflow_pkg_id` (no upgrade has occurred).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_original_pkg_id: Option<sui::types::Address>,
    /// Original (defining) package address for the scheduler package.
    ///
    /// After a Sui Move package upgrade, scheduler object/event types still
    /// reference the original package address in their type tags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler_original_pkg_id: Option<sui::types::Address>,
}

fn default_object_reference() -> sui::types::ObjectReference {
    sui::types::ObjectReference::new(sui::types::Address::ZERO, 1, sui::types::Digest::ZERO)
}

impl NexusObjects {
    /// Resolve every package origin needed for stable type identity after upgrades.
    #[cfg(feature = "nexus")]
    pub async fn resolve_original_pkg_ids(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        if self.primitives_original_pkg_id.is_none() {
            self.primitives_original_pkg_id =
                resolve_original_package_id(client, self.primitives_pkg_id, "primitives").await?;
        }
        if self.interface_original_pkg_id.is_none() {
            self.interface_original_pkg_id =
                resolve_original_package_id(client, self.interface_pkg_id, "interface").await?;
        }
        if self.registry_original_pkg_id.is_none() {
            self.registry_original_pkg_id =
                resolve_original_package_id(client, self.registry_pkg_id, "registry").await?;
        }
        self.resolve_online_payment_original_pkg_id(client).await?;
        self.resolve_workflow_original_pkg_id(client).await?;
        self.resolve_scheduler_original_pkg_id(client).await
    }

    /// Returns the package address that defines primitive types.
    pub fn primitives_type_origin_pkg_id(&self) -> sui::types::Address {
        self.primitives_original_pkg_id
            .unwrap_or(self.primitives_pkg_id)
    }

    /// Returns the package address that defines interface types.
    pub fn interface_type_origin_pkg_id(&self) -> sui::types::Address {
        self.interface_original_pkg_id
            .unwrap_or(self.interface_pkg_id)
    }

    /// Returns the package address that defines registry types.
    pub fn registry_type_origin_pkg_id(&self) -> sui::types::Address {
        self.registry_original_pkg_id
            .unwrap_or(self.registry_pkg_id)
    }

    /// Returns the package address that defines online payment types.
    pub fn online_payment_type_origin_pkg_id(&self) -> sui::types::Address {
        self.online_payment_original_pkg_id
            .unwrap_or(self.online_payment_pkg_id)
    }

    /// Returns the original (defining) workflow package address.
    ///
    /// After a Sui package upgrade, on-chain types reference the original
    /// package address. Use this for derived object ID computations and
    /// type tag matching. Falls back to `workflow_pkg_id` when no upgrade
    /// has occurred.
    pub fn workflow_type_origin_pkg_id(&self) -> sui::types::Address {
        self.workflow_original_pkg_id
            .unwrap_or(self.workflow_pkg_id)
    }

    /// Returns the original (defining) scheduler package address.
    ///
    /// After a Sui package upgrade, scheduler types reference the original
    /// package address. Falls back to `scheduler_pkg_id` when no upgrade has
    /// occurred.
    pub fn scheduler_type_origin_pkg_id(&self) -> sui::types::Address {
        self.scheduler_original_pkg_id
            .unwrap_or(self.scheduler_pkg_id)
    }

    /// Returns true when the address matches the current or original primitives package.
    pub fn is_primitives_package(&self, address: sui::types::Address) -> bool {
        package_matches(
            address,
            self.primitives_pkg_id,
            self.primitives_original_pkg_id,
        )
    }

    /// Returns true when the address matches the current or original interface package.
    pub fn is_interface_package(&self, address: sui::types::Address) -> bool {
        package_matches(
            address,
            self.interface_pkg_id,
            self.interface_original_pkg_id,
        )
    }

    /// Returns true when the address matches the current or original registry package.
    pub fn is_registry_package(&self, address: sui::types::Address) -> bool {
        package_matches(address, self.registry_pkg_id, self.registry_original_pkg_id)
    }

    /// Returns true when the given address matches a known online payment package.
    pub fn is_online_payment_package(&self, address: sui::types::Address) -> bool {
        package_matches(
            address,
            self.online_payment_pkg_id,
            self.online_payment_original_pkg_id,
        )
    }

    /// Returns true when the given address matches any known workflow
    /// package address (current or original).
    pub fn is_workflow_package(&self, address: sui::types::Address) -> bool {
        package_matches(address, self.workflow_pkg_id, self.workflow_original_pkg_id)
    }

    /// Returns true when the given address matches any known scheduler package
    /// address (current or original).
    pub fn is_scheduler_package(&self, address: sui::types::Address) -> bool {
        package_matches(
            address,
            self.scheduler_pkg_id,
            self.scheduler_original_pkg_id,
        )
    }

    /// Returns true when the address matches any configured Nexus package.
    pub fn is_nexus_package(&self, address: sui::types::Address) -> bool {
        self.is_primitives_package(address)
            || self.is_interface_package(address)
            || self.is_registry_package(address)
            || self.is_online_payment_package(address)
            || self.is_workflow_package(address)
            || self.is_scheduler_package(address)
    }

    /// Resolve and store the package address that defines online payment types.
    #[cfg(feature = "nexus")]
    pub async fn resolve_online_payment_original_pkg_id(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        if self.online_payment_original_pkg_id.is_some() {
            return Ok(());
        }
        self.online_payment_original_pkg_id =
            resolve_original_package_id(client, self.online_payment_pkg_id, "online payment")
                .await?;
        Ok(())
    }

    /// Resolve the original workflow package address from the on-chain
    /// `type_origin_table` and set `workflow_original_pkg_id`.
    ///
    /// After a Sui package upgrade, the `type_origin_table` on the upgraded
    /// package records which package originally defined each type. This
    /// method fetches that table and extracts the original address.
    ///
    /// If no upgrade has occurred (i.e. the type origins point to the same
    /// address as `workflow_pkg_id`), `workflow_original_pkg_id` remains `None`.
    #[cfg(feature = "nexus")]
    pub async fn resolve_workflow_original_pkg_id(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        if self.workflow_original_pkg_id.is_some() {
            return Ok(());
        }
        self.workflow_original_pkg_id =
            resolve_original_package_id(client, self.workflow_pkg_id, "workflow").await?;
        Ok(())
    }

    /// Resolve the original scheduler package address from the on-chain
    /// `type_origin_table` and set `scheduler_original_pkg_id`.
    ///
    /// If no upgrade has occurred, `scheduler_original_pkg_id` remains `None`.
    #[cfg(feature = "nexus")]
    pub async fn resolve_scheduler_original_pkg_id(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        if self.scheduler_original_pkg_id.is_some() {
            return Ok(());
        }
        self.scheduler_original_pkg_id =
            resolve_original_package_id(client, self.scheduler_pkg_id, "scheduler").await?;
        Ok(())
    }
}

fn package_matches(
    address: sui::types::Address,
    current: sui::types::Address,
    original: Option<sui::types::Address>,
) -> bool {
    address == current || original.is_some_and(|original| address == original)
}

#[cfg(feature = "nexus")]
async fn resolve_original_package_id(
    client: &Arc<sui::grpc::Client>,
    current: sui::types::Address,
    package_name: &str,
) -> anyhow::Result<Option<sui::types::Address>> {
    use sui::traits::FieldMaskUtil;

    let request = sui::grpc::GetObjectRequest::default()
        .with_object_id(current)
        .with_read_mask(sui::grpc::FieldMask::from_paths(["package"]));
    let response = client
        .as_ref()
        .clone()
        .ledger_client()
        .get_object(request)
        .await
        .map(|response| response.into_inner())
        .map_err(|error| anyhow::anyhow!("Failed to fetch {package_name} package: {error}"))?;
    let package = response
        .object
        .ok_or_else(|| anyhow::anyhow!("{package_name} package not found"))?
        .package
        .ok_or_else(|| anyhow::anyhow!("{package_name} object is not a package"))?;

    let mut original = None;
    for origin in &package.type_origins {
        let Some(package_id) = origin.package_id.as_deref() else {
            continue;
        };
        let package_id = package_id
            .parse::<sui::types::Address>()
            .map_err(|error| anyhow::anyhow!("Invalid {package_name} type origin: {error}"))?;
        if package_id == current {
            continue;
        }
        if original.is_some_and(|original| original != package_id) {
            anyhow::bail!(
                "{package_name} has multiple type origins; configure its original package ID"
            );
        }
        original = Some(package_id);
    }
    Ok(original)
}

impl NexusObjects {
    /// Returns true when the event payload originates from a configured Nexus package.
    pub fn is_event_from_nexus(&self, event: &sui::types::Event) -> bool {
        let Some(sui::types::TypeTag::Struct(inner_tag)) = event.type_.type_params().first() else {
            return false;
        };

        if self.is_online_payment_package(*inner_tag.address())
            || self.is_workflow_package(*inner_tag.address())
        {
            return true;
        }

        if self.is_scheduler_package(*inner_tag.address()) {
            return true;
        }

        if self.is_registry_package(*inner_tag.address()) {
            return true;
        }

        if self.is_interface_package(*inner_tag.address())
            && (self.interface_module_matches::<agent_move::Agent>(inner_tag.module())
                || self.interface_module_matches::<authorization_move::AgentVertexAuthorization>(
                    inner_tag.module(),
                )
                || self
                    .interface_module_matches::<payment_move::ExecutionPayment>(inner_tag.module())
                || self
                    .interface_module_matches::<version_move::InterfaceVersion>(inner_tag.module())
                || self.interface_module_matches::<dag_move::DAG>(inner_tag.module()))
        {
            return true;
        }

        false
    }

    fn interface_module_matches<T>(&self, module: &sui::types::Identifier) -> bool
    where
        T: MoveStruct,
    {
        let tag = crate::move_bindings::struct_tag::<T>(self);
        module == tag.module()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_objects() -> NexusObjects {
        let mut rng = rand::thread_rng();

        NexusObjects {
            online_payment_pkg_id: sui::types::Address::generate(&mut rng),
            workflow_pkg_id: sui::types::Address::generate(&mut rng),
            scheduler_pkg_id: sui::types::Address::generate(&mut rng),
            primitives_pkg_id: sui::types::Address::generate(&mut rng),
            interface_pkg_id: sui::types::Address::generate(&mut rng),
            network_id: sui::types::Address::generate(&mut rng),
            registry_pkg_id: sui::types::Address::generate(&mut rng),
            tool_registry: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            verifier_registry: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            network_auth: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            agent_registry: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: sui::types::Address::generate(&mut rng),
                skill_id: 1,
            },
            gas_service: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            leader_registry: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            priority_fee_vault: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            priority_fee_vault_owner_cap: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            us_token: UsTokenConfig::new(sui::types::Address::generate(&mut rng)),
            primitives_original_pkg_id: None,
            interface_original_pkg_id: None,
            registry_original_pkg_id: None,
            online_payment_original_pkg_id: None,
            workflow_original_pkg_id: None,
            scheduler_original_pkg_id: None,
        }
    }

    fn struct_tag_with_package<T>(
        objects: &NexusObjects,
        package: sui::types::Address,
    ) -> sui::types::StructTag
    where
        T: MoveStruct,
    {
        crate::move_bindings::struct_tag_with_package::<T>(objects, package)
    }

    fn wrap_event(objects: &NexusObjects, inner: sui::types::StructTag) -> sui::types::Event {
        let rng = &mut rand::thread_rng();
        let wrapper = crate::move_bindings::struct_tag::<
            event_move::EventWrapper<agent_move::AgentCreatedEvent>,
        >(objects);

        sui::types::Event {
            package_id: *wrapper.address(),
            module: wrapper.module().clone(),
            sender: sui::types::Address::generate(rng),
            type_: sui::types::StructTag::new(
                *wrapper.address(),
                wrapper.module().clone(),
                wrapper.name().clone(),
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: vec![],
        }
    }

    #[test]
    fn us_token_config_scopes_generated_token_and_coin_tags() {
        let package = sui::types::Address::from_static("0x42");
        let config = UsTokenConfig::new(package);

        let sui::types::TypeTag::Struct(us_tag) = config.type_tag() else {
            panic!("US must be a generated struct type");
        };
        assert_eq!(*us_tag.address(), package);
        assert_eq!(us_tag.module().as_str(), "us");
        assert_eq!(us_tag.name().as_str(), "US");

        let coin_tag = config.coin_type_tag();
        assert_eq!(*coin_tag.address(), sui::types::Address::from_static("0x2"));
        assert_eq!(coin_tag.module().as_str(), "coin");
        assert_eq!(coin_tag.name().as_str(), "Coin");
        assert_eq!(
            coin_tag.type_params(),
            &[sui::types::TypeTag::Struct(us_tag)]
        );
        assert_eq!(config.qualified_type(), format!("{package}::us::US"));
    }

    #[test]
    fn matches_workflow_interface_and_agent_registry_events() {
        let objects = sample_objects();
        let rng = &mut rand::thread_rng();

        let workflow_event = wrap_event(
            &objects,
            crate::move_bindings::struct_tag::<execution_move::DAGExecution>(&objects),
        );

        assert!(objects.is_event_from_nexus(&workflow_event));

        let interface_dag_event = wrap_event(
            &objects,
            crate::move_bindings::struct_tag::<dag_move::DAG>(&objects),
        );

        assert!(objects.is_event_from_nexus(&interface_dag_event));

        let interface_tap_event = wrap_event(
            &objects,
            crate::move_bindings::struct_tag::<agent_move::AgentCreatedEvent>(&objects),
        );

        assert!(objects.is_event_from_nexus(&interface_tap_event));

        let registry_tap_event = wrap_event(
            &objects,
            crate::move_bindings::struct_tag::<agent_registry_move::SkillRegisteredEvent>(&objects),
        );

        assert!(objects.is_event_from_nexus(&registry_tap_event));

        let unrelated_interface_event = wrap_event(
            &objects,
            sui::types::StructTag::new(
                objects.interface_pkg_id,
                sui::types::Identifier::from_static("unrelated"),
                sui::types::Identifier::from_static("SkillContractRevisionedEvent"),
                vec![],
            ),
        );

        assert!(!objects.is_event_from_nexus(&unrelated_interface_event));

        let unrelated_event = wrap_event(
            &objects,
            sui::types::StructTag::new(
                sui::types::Address::generate(rng),
                sui::types::Identifier::from_static("foo"),
                sui::types::Identifier::from_static("bar"),
                vec![],
            ),
        );

        assert!(!objects.is_event_from_nexus(&unrelated_event));
    }

    #[test]
    fn matches_registry_events() {
        let mut objects = sample_objects();
        let mut rng = rand::thread_rng();
        let registry_pkg_id = sui::types::Address::generate(&mut rng);
        objects.registry_pkg_id = registry_pkg_id;

        let registry_event = wrap_event(
            &objects,
            sui::types::StructTag::new(
                registry_pkg_id,
                sui::types::Identifier::from_static("tool_registry"),
                sui::types::Identifier::from_static("ToolRegisteredEvent"),
                vec![],
            ),
        );

        assert!(objects.is_event_from_nexus(&registry_event));
    }

    #[test]
    fn matches_scheduler_events() {
        let objects = sample_objects();
        let task_tag = crate::move_bindings::struct_tag::<scheduler_task_move::Task>(&objects);

        let scheduler_event = wrap_event(&objects, task_tag);

        assert!(objects.is_event_from_nexus(&scheduler_event));
    }

    fn sample_objects_with_upgrade() -> NexusObjects {
        let mut objects = sample_objects();
        let mut rng = rand::thread_rng();
        objects.workflow_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects
    }

    fn sample_objects_with_foundation_upgrades() -> NexusObjects {
        let mut objects = sample_objects();
        let mut rng = rand::thread_rng();
        objects.primitives_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects.interface_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects.registry_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects
    }

    fn sample_objects_with_online_payment_upgrade() -> NexusObjects {
        let mut objects = sample_objects();
        let mut rng = rand::thread_rng();
        objects.online_payment_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects
    }

    fn sample_objects_with_scheduler_upgrade() -> NexusObjects {
        let mut objects = sample_objects();
        let mut rng = rand::thread_rng();
        objects.scheduler_original_pkg_id = Some(sui::types::Address::generate(&mut rng));
        objects
    }

    #[test]
    fn workflow_type_origin_pkg_id_without_upgrade() {
        let objects = sample_objects();
        assert_eq!(
            objects.workflow_type_origin_pkg_id(),
            objects.workflow_pkg_id
        );
    }

    #[test]
    fn foundation_type_origins_and_events_remain_stable_after_upgrades() {
        let objects = sample_objects_with_foundation_upgrades();

        assert_eq!(
            objects.primitives_type_origin_pkg_id(),
            objects.primitives_original_pkg_id.unwrap()
        );
        assert_eq!(
            objects.interface_type_origin_pkg_id(),
            objects.interface_original_pkg_id.unwrap()
        );
        assert_eq!(
            objects.registry_type_origin_pkg_id(),
            objects.registry_original_pkg_id.unwrap()
        );

        for package in [
            objects.primitives_original_pkg_id.unwrap(),
            objects.interface_original_pkg_id.unwrap(),
            objects.registry_original_pkg_id.unwrap(),
        ] {
            assert!(objects.is_nexus_package(package));
        }

        let event = wrap_event(
            &objects,
            struct_tag_with_package::<agent_move::AgentCreatedEvent>(
                &objects,
                objects.interface_original_pkg_id.unwrap(),
            ),
        );
        assert!(objects.is_event_from_nexus(&event));
    }

    #[test]
    fn online_payment_type_origin_uses_current_package_without_upgrade() {
        let objects = sample_objects();
        assert_eq!(
            objects.online_payment_type_origin_pkg_id(),
            objects.online_payment_pkg_id
        );
    }

    #[test]
    fn online_payment_type_origin_uses_original_package_after_upgrade() {
        let objects = sample_objects_with_online_payment_upgrade();
        assert_eq!(
            objects.online_payment_type_origin_pkg_id(),
            objects.online_payment_original_pkg_id.unwrap()
        );
    }

    #[test]
    fn online_payment_events_match_current_and_original_packages() {
        let objects = sample_objects_with_online_payment_upgrade();
        let current = wrap_event(
            &objects,
            struct_tag_with_package::<online_payment_gas_move::PaymentLockUpdateEvent>(
                &objects,
                objects.online_payment_pkg_id,
            ),
        );
        let original = wrap_event(
            &objects,
            struct_tag_with_package::<online_payment_gas_move::PaymentLockUpdateEvent>(
                &objects,
                objects.online_payment_original_pkg_id.unwrap(),
            ),
        );

        assert!(objects.is_event_from_nexus(&current));
        assert!(objects.is_event_from_nexus(&original));
    }

    #[test]
    fn workflow_type_origin_pkg_id_with_upgrade() {
        let objects = sample_objects_with_upgrade();
        assert_eq!(
            objects.workflow_type_origin_pkg_id(),
            objects.workflow_original_pkg_id.unwrap()
        );
        assert_ne!(
            objects.workflow_type_origin_pkg_id(),
            objects.workflow_pkg_id
        );
    }

    #[test]
    fn scheduler_type_origin_pkg_id_without_upgrade() {
        let objects = sample_objects();
        assert_eq!(
            objects.scheduler_type_origin_pkg_id(),
            objects.scheduler_pkg_id
        );
    }

    #[test]
    fn scheduler_type_origin_pkg_id_with_upgrade() {
        let objects = sample_objects_with_scheduler_upgrade();
        assert_eq!(
            objects.scheduler_type_origin_pkg_id(),
            objects.scheduler_original_pkg_id.unwrap()
        );
        assert_ne!(
            objects.scheduler_type_origin_pkg_id(),
            objects.scheduler_pkg_id
        );
    }

    #[test]
    fn is_workflow_package_matches_current() {
        let objects = sample_objects();
        assert!(objects.is_workflow_package(objects.workflow_pkg_id));
    }

    #[test]
    fn is_workflow_package_matches_original_after_upgrade() {
        let objects = sample_objects_with_upgrade();
        let original = objects.workflow_original_pkg_id.unwrap();
        assert!(objects.is_workflow_package(objects.workflow_pkg_id));
        assert!(objects.is_workflow_package(original));
    }

    #[test]
    fn is_workflow_package_rejects_unrelated() {
        let mut rng = rand::thread_rng();
        let objects = sample_objects_with_upgrade();
        assert!(!objects.is_workflow_package(sui::types::Address::generate(&mut rng)));
    }

    #[test]
    fn is_scheduler_package_matches_current_and_original_after_upgrade() {
        let objects = sample_objects_with_scheduler_upgrade();
        let original = objects.scheduler_original_pkg_id.unwrap();
        assert!(objects.is_scheduler_package(objects.scheduler_pkg_id));
        assert!(objects.is_scheduler_package(original));
    }

    #[test]
    fn event_from_original_pkg_matches_after_upgrade() {
        let objects = sample_objects_with_upgrade();
        let original = objects.workflow_original_pkg_id.unwrap();

        // Event referencing the original package address should match.
        let event = wrap_event(
            &objects,
            struct_tag_with_package::<execution_move::DAGExecution>(&objects, original),
        );
        assert!(objects.is_event_from_nexus(&event));

        // Event referencing the current (upgraded) package should also match.
        let event = wrap_event(
            &objects,
            struct_tag_with_package::<execution_move::DAGExecution>(
                &objects,
                objects.workflow_pkg_id,
            ),
        );
        assert!(objects.is_event_from_nexus(&event));
    }

    #[test]
    fn toml_round_trip_without_upgrade() {
        let objects = sample_objects();
        let encoded = toml::to_string(&objects).unwrap();
        assert!(!encoded.contains("workflow_original_pkg_id"));
        let deserialized: NexusObjects = toml::from_str(&encoded).unwrap();
        assert_eq!(deserialized, objects);
    }

    #[test]
    fn toml_round_trip_with_upgrade() {
        let objects = sample_objects_with_scheduler_upgrade();
        let encoded = toml::to_string(&objects).unwrap();
        assert!(encoded.contains("scheduler_original_pkg_id"));
        let deserialized: NexusObjects = toml::from_str(&encoded).unwrap();
        assert_eq!(deserialized, objects);
    }
}
