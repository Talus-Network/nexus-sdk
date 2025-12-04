//! [`NexusObjects`] struct is holding the Nexus object IDs and refs that are
//! generated during Nexus package deployment.
#[cfg(feature = "sui_idents")]
use super::scheduler::{MoveTypeName, PolicySymbol};
#[cfg(all(test, feature = "sui_idents"))]
use crate::idents::primitives;
#[cfg(feature = "sui_idents")]
use crate::idents::{workflow, ModuleAndNameIdent};
use {
    crate::sui,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusObjects {
    pub workflow_pkg_id: sui::types::Address,
    pub primitives_pkg_id: sui::types::Address,
    pub interface_pkg_id: sui::types::Address,
    pub network_id: sui::types::Address,
    pub tool_registry: sui::types::ObjectReference,
    pub default_tap: sui::types::ObjectReference,
    pub gas_service: sui::types::ObjectReference,
    pub pre_key_vault: sui::types::ObjectReference,
}

#[cfg(feature = "sui_idents")]
impl NexusObjects {
    /// Returns true when the event payload originates from the configured workflow or interface package.
    pub fn is_event_from_nexus(&self, event: &sui::types::Event) -> bool {
        let Some(sui::types::TypeTag::Struct(inner_tag)) = event.type_.type_params().first() else {
            return false;
        };

        if *inner_tag.address() == self.workflow_pkg_id {
            return true;
        }

        if *inner_tag.address() == self.interface_pkg_id
            && inner_tag.module().as_str() == "v1"
            && inner_tag.name().as_str() == "AnnounceInterfacePackageEvent"
        {
            let Some(sui::types::TypeTag::Struct(witness)) = inner_tag.type_params().first() else {
                return false;
            };

            return *witness.address() == self.workflow_pkg_id;
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
        PolicySymbol::Witness(MoveTypeName {
            name: ident.qualified_name(self.workflow_pkg_id),
        })
    }

    fn generator_matches(&self, symbol: &PolicySymbol, ident: &ModuleAndNameIdent) -> bool {
        let expected = ident.qualified_name(self.workflow_pkg_id);
        symbol.matches_qualified_name(&expected)
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
            pre_key_vault: sui::types::ObjectReference::new(
                sui::types::Address::generate(&mut rng),
                1,
                sui::types::Digest::generate(&mut rng),
            ),
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
