//! Typed PTB builders for activating published Nexus V1 objects through V2 state migrations.

use {
    crate::{
        move_bindings::{
            interface::{
                agent::{self as agent_binding, ExecutionInputMigrationWitness},
                dag::{self as dag_binding, DAGDefaultMigrationWitness},
                graph::{self as graph_binding, InputPort, Vertex, VertexInputPort},
            },
            primitives::meta_schema::MetaSchema,
            scheduler::task as task_binding,
            sui_framework::vec_map::{self as vec_map_binding, VecMap},
            tool::tool_registry as tool_registry_binding,
        },
        move_boundary::{self, NexusPtbBuilder},
        sui,
        types::{NexusData, NexusObjects},
    },
    anyhow::ensure,
    std::collections::BTreeMap,
    sui::types::{Argument, ObjectReference, ProgrammableTransaction},
};

/// Stable identity and initial shared version needed to pass a shared migration object to a PTB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedMigrationObject {
    pub object_id: sui::types::Address,
    pub initial_shared_version: sui::types::Version,
}

/// Vertex-name to immutable Tool schema replacements required by DAG migration.
pub type DagMigrationSchemas = BTreeMap<String, MetaSchema>;

/// `(vertex, input-port)` to exact typed replacement for a legacy DAG default value.
pub type DagDefaultMigrationWitnesses = BTreeMap<(String, String), NexusData>;

/// Vertex and input-port names to exact typed replacements for legacy Task inputs.
pub type TaskInputMigrationWitnesses = BTreeMap<String, BTreeMap<String, NexusData>>;

type MoveTaskPortWitnesses = VecMap<InputPort, ExecutionInputMigrationWitness>;

/// Builds a PTB that migrates one V1 DAG through the owner-authorized production endpoint.
pub fn migrate_dag_v1_to_v2_ptb(
    objects: &NexusObjects,
    dag: SharedMigrationObject,
    owner_cap: &ObjectReference,
    schemas: &DagMigrationSchemas,
    default_witnesses: &DagDefaultMigrationWitnesses,
) -> anyhow::Result<ProgrammableTransaction> {
    validate_witnesses(default_witnesses.values())?;
    move_boundary::ptb(objects, |tx| {
        let dag = shared_object(tx, dag, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let schemas = dag_schemas_arg(tx, schemas)?;
        let default_witnesses = dag_default_witnesses_arg(tx, default_witnesses)?;
        tx.call_target(
            dag_binding::migrate_to_v2_target,
            vec![dag, owner_cap, schemas, default_witnesses],
        )?;
        Ok(())
    })
}

/// Builds a PTB that migrates one address-controlled V1 Task.
pub fn migrate_address_task_v1_to_v2_ptb(
    objects: &NexusObjects,
    task: SharedMigrationObject,
    input_witnesses: &TaskInputMigrationWitnesses,
) -> anyhow::Result<ProgrammableTransaction> {
    validate_witnesses(input_witnesses.values().flat_map(|ports| ports.values()))?;
    move_boundary::ptb(objects, |tx| {
        let task = shared_object(tx, task, true)?;
        let input_witnesses = task_input_witnesses_arg(tx, input_witnesses)?;
        tx.call_target(
            task_binding::migrate_address_task_to_v2_target,
            vec![task, input_witnesses],
        )?;
        Ok(())
    })
}

/// Builds a PTB that migrates one Agent-controlled V1 Task after borrowing its controlling Agent.
pub fn migrate_agent_task_v1_to_v2_ptb(
    objects: &NexusObjects,
    task: SharedMigrationObject,
    controller_agent: SharedMigrationObject,
    input_witnesses: &TaskInputMigrationWitnesses,
) -> anyhow::Result<ProgrammableTransaction> {
    validate_witnesses(input_witnesses.values().flat_map(|ports| ports.values()))?;
    move_boundary::ptb(objects, |tx| {
        let task = shared_object(tx, task, true)?;
        let controller_agent = shared_object(tx, controller_agent, false)?;
        let input_witnesses = task_input_witnesses_arg(tx, input_witnesses)?;
        tx.call_target(
            task_binding::migrate_agent_task_to_v2_target,
            vec![task, controller_agent, input_witnesses],
        )?;
        Ok(())
    })
}

/// Builds a PTB that migrates one V1 Tool Registry through its bound protocol administrator.
pub fn migrate_tool_registry_v1_to_v2_ptb(
    objects: &NexusObjects,
    registry: SharedMigrationObject,
    protocol_admin_cap: &ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry = shared_object(tx, registry, true)?;
        let protocol_admin_cap = tx.owned_object(protocol_admin_cap)?;
        tx.call_target(
            tool_registry_binding::migrate_tool_registry_to_v2_target,
            vec![registry, protocol_admin_cap],
        )?;
        Ok(())
    })
}

/// Builds a PTB that atomically migrates one V1 Tool and activates its schema in the V2 registry.
pub fn migrate_tool_v1_to_v2_ptb(
    objects: &NexusObjects,
    registry: SharedMigrationObject,
    tool: SharedMigrationObject,
    owner_cap: &ObjectReference,
    schema: &MetaSchema,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool = shared_object(tx, tool, true)?;
        let registry = shared_object(tx, registry, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let schema = tx.meta_schema(schema)?;
        tx.call_target(
            tool_registry_binding::migrate_tool_to_v2_target,
            vec![tool, registry, owner_cap, schema],
        )?;
        Ok(())
    })
}

fn shared_object(
    tx: &mut NexusPtbBuilder,
    object: SharedMigrationObject,
    mutable: bool,
) -> anyhow::Result<Argument> {
    Ok(tx.shared_object_by_id(object.object_id, object.initial_shared_version, mutable)?)
}

fn dag_schemas_arg(
    tx: &mut NexusPtbBuilder,
    schemas: &DagMigrationSchemas,
) -> anyhow::Result<Argument> {
    let map = tx.call_target(vec_map_binding::empty_target::<Vertex, MetaSchema>, vec![])?;
    for (vertex, schema) in schemas {
        let vertex = tx.graph_vertex(vertex)?;
        let schema = tx.meta_schema(schema)?;
        tx.call_target(
            vec_map_binding::insert_target::<Vertex, MetaSchema>,
            vec![map, vertex, schema],
        )?;
    }
    Ok(map)
}

fn dag_default_witnesses_arg(
    tx: &mut NexusPtbBuilder,
    witnesses: &DagDefaultMigrationWitnesses,
) -> anyhow::Result<Argument> {
    let map = tx.call_target(
        vec_map_binding::empty_target::<VertexInputPort, DAGDefaultMigrationWitness>,
        vec![],
    )?;
    for ((vertex, port), value) in witnesses {
        let vertex = tx.graph_vertex(vertex)?;
        let port = tx.graph_input_port(port)?;
        let target = tx.call_target(graph_binding::vertex_input_port_target, vec![vertex, port])?;
        let is_many = tx.arg(&value.is_many())?;
        let values = tx.nexus_value_witnesses(value)?;
        let witness = tx.call_target(
            dag_binding::default_value_migration_witness_target,
            vec![is_many, values],
        )?;
        tx.call_target(
            vec_map_binding::insert_target::<VertexInputPort, DAGDefaultMigrationWitness>,
            vec![map, target, witness],
        )?;
    }
    Ok(map)
}

fn task_input_witnesses_arg(
    tx: &mut NexusPtbBuilder,
    witnesses: &TaskInputMigrationWitnesses,
) -> anyhow::Result<Argument> {
    let map = tx.call_target(
        vec_map_binding::empty_target::<Vertex, MoveTaskPortWitnesses>,
        vec![],
    )?;
    for (vertex, ports) in witnesses {
        let vertex = tx.graph_vertex(vertex)?;
        let port_map = tx.call_target(
            vec_map_binding::empty_target::<InputPort, ExecutionInputMigrationWitness>,
            vec![],
        )?;
        for (port, value) in ports {
            let port = tx.graph_input_port(port)?;
            let is_many = tx.arg(&value.is_many())?;
            let values = tx.nexus_value_witnesses(value)?;
            let witness = tx.call_target(
                agent_binding::execution_input_migration_witness_target,
                vec![is_many, values],
            )?;
            tx.call_target(
                vec_map_binding::insert_target::<InputPort, ExecutionInputMigrationWitness>,
                vec![port_map, port, witness],
            )?;
        }
        tx.call_target(
            vec_map_binding::insert_target::<Vertex, MoveTaskPortWitnesses>,
            vec![map, vertex, port_map],
        )?;
    }
    Ok(map)
}

fn validate_witnesses<'a>(
    witnesses: impl IntoIterator<Item = &'a NexusData>,
) -> anyhow::Result<()> {
    ensure!(
        witnesses.into_iter().all(NexusData::is_well_formed),
        "migration witness contains malformed NexusData",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::primitives::meta_schema::{OutputVariantSchema, PortSchema, ValueKind},
            test_utils::sui_mocks,
        },
    };

    fn addr(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str, version: u64, digest: u8) -> ObjectReference {
        ObjectReference::new(addr(value), version, sui::types::Digest::from([digest; 32]))
    }

    fn shared(value: &'static str, version: u64) -> SharedMigrationObject {
        SharedMigrationObject {
            object_id: addr(value),
            initial_shared_version: version,
        }
    }

    fn schema() -> MetaSchema {
        MetaSchema::new(
            vec![PortSchema::new(b"input".to_vec(), false, ValueKind::Data)],
            vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
        )
    }

    fn move_functions(transaction: &ProgrammableTransaction) -> Vec<String> {
        transaction
            .commands
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call.function.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn migration_builders_target_every_production_migration() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner_cap = object_ref("0xc1", 1, 1);
        let protocol_admin_cap = object_ref("0xc2", 1, 2);
        let mut dag_schemas = DagMigrationSchemas::new();
        dag_schemas.insert("worker".to_owned(), schema());
        let mut defaults = DagDefaultMigrationWitnesses::new();
        defaults.insert(
            ("worker".to_owned(), "input".to_owned()),
            NexusData::inline_data(b"default").unwrap(),
        );
        let dag = migrate_dag_v1_to_v2_ptb(
            &objects,
            shared("0xd1", 1),
            &owner_cap,
            &dag_schemas,
            &defaults,
        )
        .unwrap();
        let dag_functions = move_functions(&dag);
        assert!(dag_functions
            .iter()
            .any(|name| name == "default_value_migration_witness"));
        assert!(dag_functions.iter().any(|name| name == "migrate_to_v2"));

        let mut task_inputs = TaskInputMigrationWitnesses::new();
        task_inputs.insert(
            "worker".to_owned(),
            BTreeMap::from([(
                "input".to_owned(),
                NexusData::inline_data(b"input").unwrap(),
            )]),
        );
        let address_task =
            migrate_address_task_v1_to_v2_ptb(&objects, shared("0xd2", 2), &task_inputs).unwrap();
        let address_functions = move_functions(&address_task);
        assert!(address_functions
            .iter()
            .any(|name| name == "execution_input_migration_witness"));
        assert!(address_functions
            .iter()
            .any(|name| name == "migrate_address_task_to_v2"));

        let agent_task = migrate_agent_task_v1_to_v2_ptb(
            &objects,
            shared("0xd3", 3),
            shared("0xa1", 4),
            &task_inputs,
        )
        .unwrap();
        assert!(move_functions(&agent_task)
            .iter()
            .any(|name| name == "migrate_agent_task_to_v2"));

        let registry =
            migrate_tool_registry_v1_to_v2_ptb(&objects, shared("0xd4", 5), &protocol_admin_cap)
                .unwrap();
        assert!(move_functions(&registry)
            .iter()
            .any(|name| name == "migrate_tool_registry_to_v2"));

        let tool = migrate_tool_v1_to_v2_ptb(
            &objects,
            shared("0xd4", 5),
            shared("0xd5", 6),
            &owner_cap,
            &schema(),
        )
        .unwrap();
        assert!(move_functions(&tool)
            .iter()
            .any(|name| name == "migrate_tool_to_v2"));
    }

    #[test]
    fn migration_builders_reject_empty_many_before_ptb_construction() {
        let objects = sui_mocks::mock_nexus_objects();
        let malformed = NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new());
        let defaults = DagDefaultMigrationWitnesses::from([(
            ("worker".to_owned(), "input".to_owned()),
            malformed.clone(),
        )]);
        assert!(migrate_dag_v1_to_v2_ptb(
            &objects,
            shared("0xd1", 1),
            &object_ref("0xc1", 1, 1),
            &DagMigrationSchemas::new(),
            &defaults,
        )
        .unwrap_err()
        .to_string()
        .contains("migration witness contains malformed NexusData"));

        let inputs = TaskInputMigrationWitnesses::from([(
            "worker".to_owned(),
            BTreeMap::from([("input".to_owned(), malformed)]),
        )]);
        assert!(
            migrate_address_task_v1_to_v2_ptb(&objects, shared("0xd2", 2), &inputs,)
                .unwrap_err()
                .to_string()
                .contains("migration witness contains malformed NexusData")
        );
    }
}
