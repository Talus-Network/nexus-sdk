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
    pub workflow_pkg_id: sui::ObjectID,
    pub primitives_pkg_id: sui::ObjectID,
    pub interface_pkg_id: sui::ObjectID,
    pub network_id: sui::ObjectID,
    pub tool_registry: sui::ObjectRef,
    pub default_tap: sui::ObjectRef,
    pub gas_service: sui::ObjectRef,
    pub pre_key_vault: sui::ObjectRef,
}

#[cfg(feature = "sui_idents")]
impl NexusObjects {
    /// Returns true when the event payload originates from the configured workflow or interface package.
    pub fn is_event_from_nexus(&self, event: &sui::Event) -> bool {
        use sui::MoveTypeTag;

        let Some(MoveTypeTag::Struct(inner_tag)) = event.type_.type_params.first() else {
            return false;
        };

        if inner_tag.address == *self.workflow_pkg_id {
            return true;
        }

        if inner_tag.address == *self.interface_pkg_id
            && inner_tag.module.as_str() == "v1"
            && inner_tag.name.as_str() == "AnnounceInterfacePackageEvent"
        {
            let Some(MoveTypeTag::Struct(witness)) = inner_tag.type_params.first() else {
                return false;
            };

            return witness.address == *self.workflow_pkg_id;
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
    use {super::*, serde_json::json};

    fn sample_objects() -> NexusObjects {
        NexusObjects {
            workflow_pkg_id: sui::ObjectID::random(),
            primitives_pkg_id: sui::ObjectID::random(),
            interface_pkg_id: sui::ObjectID::random(),
            network_id: sui::ObjectID::random(),
            tool_registry: (
                sui::ObjectID::random(),
                sui::SequenceNumber::from_u64(1),
                sui::ObjectDigest::random(),
            )
                .into(),
            default_tap: (
                sui::ObjectID::random(),
                sui::SequenceNumber::from_u64(1),
                sui::ObjectDigest::random(),
            )
                .into(),
            gas_service: (
                sui::ObjectID::random(),
                sui::SequenceNumber::from_u64(1),
                sui::ObjectDigest::random(),
            )
                .into(),
            pre_key_vault: (
                sui::ObjectID::random(),
                sui::SequenceNumber::from_u64(1),
                sui::ObjectDigest::random(),
            )
                .into(),
        }
    }

    fn wrap_event(objects: &NexusObjects, inner: sui::MoveStructTag) -> sui::Event {
        sui::Event {
            id: sui::EventID {
                tx_digest: sui::TransactionDigest::random(),
                event_seq: 0,
            },
            package_id: objects.primitives_pkg_id,
            transaction_module: primitives::Event::EVENT_WRAPPER.module.into(),
            sender: sui::Address::default(),
            type_: sui::MoveStructTag {
                address: *objects.primitives_pkg_id,
                module: primitives::Event::EVENT_WRAPPER.module.into(),
                name: primitives::Event::EVENT_WRAPPER.name.into(),
                type_params: vec![sui::MoveTypeTag::Struct(Box::new(inner))],
            },
            parsed_json: json!({}),
            bcs: sui::BcsEvent::Base64 { bcs: vec![] },
            timestamp_ms: None,
        }
    }

    #[test]
    fn matches_workflow_and_interface_events() {
        let objects = sample_objects();

        let workflow_event = wrap_event(
            &objects,
            sui::MoveStructTag {
                address: *objects.workflow_pkg_id,
                module: workflow::Scheduler::TASK.module.into(),
                name: workflow::Scheduler::TASK.name.into(),
                type_params: vec![],
            },
        );

        assert!(objects.is_event_from_nexus(&workflow_event));

        let interface_event = wrap_event(
            &objects,
            sui::MoveStructTag {
                address: *objects.interface_pkg_id,
                module: sui::move_ident_str!("v1").into(),
                name: sui::move_ident_str!("AnnounceInterfacePackageEvent").into(),
                type_params: vec![sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                    address: *objects.workflow_pkg_id,
                    module: workflow::Scheduler::TASK.module.into(),
                    name: workflow::Scheduler::TASK.name.into(),
                    type_params: vec![],
                }))],
            },
        );

        assert!(objects.is_event_from_nexus(&interface_event));

        let unrelated_event = wrap_event(
            &objects,
            sui::MoveStructTag {
                address: sui::ObjectID::random().into(),
                module: sui::move_ident_str!("foo").into(),
                name: sui::move_ident_str!("bar").into(),
                type_params: vec![],
            },
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
