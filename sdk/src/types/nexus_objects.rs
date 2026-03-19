//! [`NexusObjects`] struct is holding the Nexus object IDs and refs that are
//! generated during Nexus package deployment.
#[cfg(feature = "sui_idents")]
use super::{scheduler::PolicySymbol, TypeName};
#[cfg(all(test, feature = "sui_idents"))]
use crate::idents::primitives;
#[cfg(feature = "sui_idents")]
use crate::idents::{workflow, ModuleAndNameIdent};
use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::sync::Arc,
    tokio::sync::Mutex,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusObjects {
    pub workflow_pkg_id: sui::types::Address,
    pub primitives_pkg_id: sui::types::Address,
    pub interface_pkg_id: sui::types::Address,
    pub network_id: sui::types::Address,
    pub tool_registry: sui::types::ObjectReference,
    pub network_auth: sui::types::ObjectReference,
    pub default_tap: sui::types::ObjectReference,
    pub gas_service: sui::types::ObjectReference,
    pub leader_registry: sui::types::ObjectReference,

    /// Original (defining) package address for the workflow package.
    ///
    /// After a Sui Move package upgrade, on-chain types still reference the
    /// original package address in their type tags. This field stores that
    /// address for use in derived object ID computations and type matching.
    ///
    /// When `None`, falls back to `workflow_pkg_id` (no upgrade has occurred).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_original_pkg_id: Option<sui::types::Address>,
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

    /// Returns true when the given address matches any known workflow
    /// package address (current or original).
    pub fn is_workflow_package(&self, address: sui::types::Address) -> bool {
        address == self.workflow_pkg_id
            || self
                .workflow_original_pkg_id
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
}

#[cfg(feature = "sui_idents")]
impl NexusObjects {
    /// Returns true when the event payload originates from the configured workflow or interface package.
    pub fn is_event_from_nexus(&self, event: &sui::types::Event) -> bool {
        let Some(sui::types::TypeTag::Struct(inner_tag)) = event.type_.type_params().first() else {
            return false;
        };

        if self.is_workflow_package(*inner_tag.address()) {
            return true;
        }

        if *inner_tag.address() == self.interface_pkg_id
            && inner_tag.module().as_str() == "v1"
            && inner_tag.name().as_str() == "AnnounceInterfacePackageEvent"
        {
            let Some(sui::types::TypeTag::Struct(witness)) = inner_tag.type_params().first() else {
                return false;
            };

            return self.is_workflow_package(*witness.address());
        }

        false
    }

    /// Fully-qualified Move type name for the queue generator witness.
    pub fn scheduler_queue_generator_symbol(&self) -> PolicySymbol {
        self.sched_generator_symbol(&workflow::Scheduler::QUEUE_GENERATOR_WITNESS)
    }

    /// Fully-qualified Move type name for the periodic generator witness.
    pub fn scheduler_periodic_generator_symbol(&self) -> PolicySymbol {
        self.sched_generator_symbol(&workflow::Scheduler::PERIODIC_GENERATOR_WITNESS)
    }

    /// Returns true when the provided policy symbol references the queue generator witness.
    pub fn scheduler_matches_queue_generator(&self, symbol: &PolicySymbol) -> bool {
        self.generator_matches(symbol, &workflow::Scheduler::QUEUE_GENERATOR_WITNESS)
    }

    /// Returns true when the provided policy symbol references the periodic generator witness.
    pub fn scheduler_matches_periodic_generator(&self, symbol: &PolicySymbol) -> bool {
        self.generator_matches(symbol, &workflow::Scheduler::PERIODIC_GENERATOR_WITNESS)
    }

    fn sched_generator_symbol(&self, ident: &ModuleAndNameIdent) -> PolicySymbol {
        PolicySymbol::Witness(TypeName::new(
            &ident.qualified_name(self.workflow_type_origin_pkg_id()),
        ))
    }

    fn generator_matches(&self, symbol: &PolicySymbol, ident: &ModuleAndNameIdent) -> bool {
        // Match against both original and current package addresses since
        // on-chain type names could reference either after an upgrade.
        let original = ident.qualified_name(self.workflow_type_origin_pkg_id());
        if symbol.matches_qualified_name(&original) {
            return true;
        }
        if self.workflow_original_pkg_id.is_some() {
            let current = ident.qualified_name(self.workflow_pkg_id);
            return symbol.matches_qualified_name(&current);
        }
        false
    }
}

#[cfg(all(test, feature = "sui_idents"))]
mod tests {
    use super::*;

    fn sample_objects() -> NexusObjects {
        let mut rng = rand::thread_rng();

        NexusObjects {
            workflow_pkg_id: sui::types::Address::generate(&mut rng),
            primitives_pkg_id: sui::types::Address::generate(&mut rng),
            interface_pkg_id: sui::types::Address::generate(&mut rng),
            network_id: sui::types::Address::generate(&mut rng),
            tool_registry: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            network_auth: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
            default_tap: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
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
            workflow_original_pkg_id: None,
        }
    }

    fn wrap_event(objects: &NexusObjects, inner: sui::types::StructTag) -> sui::types::Event {
        let rng = &mut rand::thread_rng();

        sui::types::Event {
            package_id: objects.primitives_pkg_id,
            module: primitives::Event::EVENT_WRAPPER.module,
            sender: sui::types::Address::generate(rng),
            type_: sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: vec![],
        }
    }

    #[test]
    fn matches_workflow_and_interface_events() {
        let objects = sample_objects();
        let rng = &mut rand::thread_rng();

        let workflow_event = wrap_event(
            &objects,
            sui::types::StructTag::new(
                objects.workflow_pkg_id,
                workflow::Scheduler::TASK.module,
                workflow::Scheduler::TASK.name,
                vec![],
            ),
        );

        assert!(objects.is_event_from_nexus(&workflow_event));

        let interface_event = wrap_event(
            &objects,
            sui::types::StructTag::new(
                objects.interface_pkg_id,
                sui::types::Identifier::from_static("v1"),
                sui::types::Identifier::from_static("AnnounceInterfacePackageEvent"),
                vec![sui::types::TypeTag::Struct(Box::new(
                    sui::types::StructTag::new(
                        objects.workflow_pkg_id,
                        workflow::Scheduler::TASK.module,
                        workflow::Scheduler::TASK.name,
                        vec![],
                    ),
                ))],
            ),
        );

        assert!(objects.is_event_from_nexus(&interface_event));

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
    fn generator_helpers_match_symbols() {
        let objects = sample_objects();

        let queue = objects.scheduler_queue_generator_symbol();
        assert!(objects.scheduler_matches_queue_generator(&queue));
        assert!(!objects.scheduler_matches_periodic_generator(&queue));

        let periodic = objects.scheduler_periodic_generator_symbol();
        assert!(objects.scheduler_matches_periodic_generator(&periodic));
        assert!(!objects.scheduler_matches_queue_generator(&periodic));
    }
}
