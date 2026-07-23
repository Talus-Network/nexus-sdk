//! [`NexusObjects`] struct is holding the Nexus object IDs and refs that are
//! generated during Nexus package deployment.
#[cfg(test)]
use crate::move_bindings::{
    primitives::event as event_move,
    registry::agent_registry as agent_registry_move,
    scheduler::task as scheduler_task_move,
    workflow::execution as execution_move,
};
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
#[cfg(feature = "nexus")]
use {std::sync::Arc, tokio::sync::Mutex};

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

    /// Returns true when the given address matches any known workflow
    /// package address (current or original).
    pub fn is_workflow_package(&self, address: sui::types::Address) -> bool {
        address == self.workflow_pkg_id
            || self
                .workflow_original_pkg_id
                .is_some_and(|orig| address == orig)
    }

    /// Returns true when the given address matches any known scheduler package
    /// address (current or original).
    pub fn is_scheduler_package(&self, address: sui::types::Address) -> bool {
        address == self.scheduler_pkg_id
            || self
                .scheduler_original_pkg_id
                .is_some_and(|orig| address == orig)
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
        client: &Arc<Mutex<sui::grpc::Client>>,
    ) -> anyhow::Result<()> {
        use sui::traits::FieldMaskUtil;

        let field_mask = sui::grpc::FieldMask::from_paths(["package"]);

        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(self.workflow_pkg_id)
            .with_read_mask(field_mask);

        let response = client
            .lock()
            .await
            .ledger_client()
            .get_object(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| anyhow::anyhow!("Failed to fetch workflow package object: {e}"))?;

        let object = response
            .object
            .ok_or_else(|| anyhow::anyhow!("Workflow package object not found"))?;

        let package = object
            .package
            .ok_or_else(|| anyhow::anyhow!("Object is not a package"))?;

        // Find the first type origin entry that references a different package.
        // All workflow types should originate from the same package.
        let original = package.type_origins.iter().find_map(|origin| {
            let pkg_id_str = origin.package_id.as_deref()?;
            let addr = pkg_id_str.parse::<sui::types::Address>().ok()?;
            if addr != self.workflow_pkg_id {
                Some(addr)
            } else {
                None
            }
        });

        self.workflow_original_pkg_id = original;

        Ok(())
    }

    /// Resolve the original scheduler package address from the on-chain
    /// `type_origin_table` and set `scheduler_original_pkg_id`.
    ///
    /// If no upgrade has occurred, `scheduler_original_pkg_id` remains `None`.
    #[cfg(feature = "nexus")]
    pub async fn resolve_scheduler_original_pkg_id(
        &mut self,
        client: &Arc<Mutex<sui::grpc::Client>>,
    ) -> anyhow::Result<()> {
        use sui::traits::FieldMaskUtil;

        let field_mask = sui::grpc::FieldMask::from_paths(["package"]);

        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(self.scheduler_pkg_id)
            .with_read_mask(field_mask);

        let response = client
            .lock()
            .await
            .ledger_client()
            .get_object(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| anyhow::anyhow!("Failed to fetch scheduler package object: {e}"))?;

        let object = response
            .object
            .ok_or_else(|| anyhow::anyhow!("Scheduler package object not found"))?;

        let package = object
            .package
            .ok_or_else(|| anyhow::anyhow!("Object is not a package"))?;

        let original = package.type_origins.iter().find_map(|origin| {
            let pkg_id_str = origin.package_id.as_deref()?;
            let addr = pkg_id_str.parse::<sui::types::Address>().ok()?;
            if addr != self.scheduler_pkg_id {
                Some(addr)
            } else {
                None
            }
        });

        self.scheduler_original_pkg_id = original;

        Ok(())
    }
}

impl NexusObjects {
    /// Returns true when the event payload originates from a configured Nexus package.
    pub fn is_event_from_nexus(&self, event: &sui::types::Event) -> bool {
        let Some(sui::types::TypeTag::Struct(inner_tag)) = event.type_.type_params().first() else {
            return false;
        };

        if self.is_workflow_package(*inner_tag.address()) {
            return true;
        }

        if self.is_scheduler_package(*inner_tag.address()) {
            return true;
        }

        if *inner_tag.address() == self.registry_pkg_id {
            return true;
        }

        if *inner_tag.address() == self.interface_pkg_id
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
