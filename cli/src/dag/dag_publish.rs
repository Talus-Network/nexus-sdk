use crate::{
    command_title,
    dag::{
        dag_validate::validate_dag,
        parser::{Data, DefaultValue, Edge, EntryVertex, Vertex, VertexKind},
    },
    loading,
    prelude::*,
    sui::*,
};

/// Sui `std::ascii::string`
// TODO: idents can be moved to a common module (nexus-types)
const SUI_ASCII_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("ascii");
const SUI_ASCII_FROM_STRING: &sui::MoveIdentStr = sui::move_ident_str!("string");

// Sui `sui::vec_set`
const SUI_VEC_SET_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("vec_set");
const SUI_VEC_SET_EMPTY: &sui::MoveIdentStr = sui::move_ident_str!("empty");
const SUI_VEC_SET_INSERT: &sui::MoveIdentStr = sui::move_ident_str!("insert");

// Sui `sui::transfer`
const SUI_TRANSFER_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("transfer");
const SUI_TRANSFER_PUBLIC_SHARE_OBJECT: &sui::MoveIdentStr =
    sui::move_ident_str!("public_share_object");

// Nexus `workflow::dag` module and its functions.
const WORKFLOW_DAG_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("dag");
const WORKFLOW_DAG_NEW: &sui::MoveIdentStr = sui::move_ident_str!("new");

const WORKFLOW_DAG_WITH_VERTEX: &sui::MoveIdentStr = sui::move_ident_str!("with_vertex");
const WORKFLOW_DAG_WITH_EDGE: &sui::MoveIdentStr = sui::move_ident_str!("with_edge");
const WORKFLOW_DAG_WITH_DEFAULT_VALUE: &sui::MoveIdentStr =
    sui::move_ident_str!("with_default_value");
const WORKFLOW_DAG_WITH_ENTRY_VERTEX: &sui::MoveIdentStr =
    sui::move_ident_str!("with_entry_vertex");

const WORKFLOW_DAG_VERTEX_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("vertex_from_string");
const WORKFLOW_DAG_INPUT_PORT_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("input_port_from_string");
const WORKFLOW_DAG_OUTPUT_VARIANT_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("output_variant_from_string");
const WORKFLOW_DAG_OUTPUT_PORT_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("output_port_from_string");

const WORKFLOW_DAG_VERTEX_OFF_CHAIN: &sui::MoveIdentStr = sui::move_ident_str!("vertex_off_chain");

const WORKFLOW_INPUT_PORT: &sui::MoveIdentStr = sui::move_ident_str!("InputPort");

// Nexus `primitives::data` module and its functions.
const PRIMITIVES_DATA_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("data");
const PRIMITIVES_DATA_INLINE_ONE: &sui::MoveIdentStr = sui::move_ident_str!("inline_one");

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        primitives_pkg_id,
        ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::Any(e));
        }
    };

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // Create an empty DAG.
    let dag_arg = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_NEW.into(),
        vec![],
        vec![],
    );

    // Create all entry vertices.
    for entry_vertex in dag.entry_vertices {
        match create_entry_vertex(&mut tx, workflow_pkg_id, dag_arg, entry_vertex) {
            Ok(()) => (),
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Create all default values if present.
    if let Some(default_values) = dag.default_values {
        for default_value in default_values {
            match create_default_value(
                &mut tx,
                workflow_pkg_id,
                primitives_pkg_id,
                dag_arg,
                &default_value,
            ) {
                Ok(()) => (),
                Err(e) => {
                    tx_handle.error();

                    return Err(NexusCliError::Any(e));
                }
            }
        }
    }

    // Create all vertices.
    for vertex in dag.vertices {
        match create_vertex(&mut tx, workflow_pkg_id, dag_arg, &vertex) {
            Ok(()) => (),
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Create all edges.
    for edge in dag.edges {
        match create_edge(&mut tx, workflow_pkg_id, dag_arg, &edge) {
            Ok(()) => (),
            Err(e) => {
                tx_handle.error();

                return Err(NexusCliError::Any(e));
            }
        }
    }

    // Public share the DAG, locking it.
    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        SUI_TRANSFER_MODULE.into(),
        SUI_TRANSFER_PUBLIC_SHARE_OBJECT.into(),
        vec![],
        vec![dag_arg],
    );

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    let effects = sign_transaction(&sui, &wallet, tx_data).await?;

    println!("{:#?}", effects);

    Ok(())
}

/// Craft transaction arguments to create a [crate::dag::parser::EntryVertex]
/// on-chain.
fn create_entry_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: EntryVertex,
) -> AnyResult<()> {
    // `name: Vertex`
    let name = into_sui_ascii_string(tx, vertex.name.as_str())?;

    let name = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![name],
    );

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            let tool_fqn = into_sui_ascii_string(tx, tool_fqn.to_string().as_str())?;

            tx.programmable_move_call(
                workflow_pkg_id,
                WORKFLOW_DAG_MODULE.into(),
                WORKFLOW_DAG_VERTEX_OFF_CHAIN.into(),
                vec![],
                vec![tool_fqn],
            )
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `input_ports: VecSet<InputPort>`
    let input_port_type = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *workflow_pkg_id,
        module: WORKFLOW_DAG_MODULE.into(),
        name: WORKFLOW_INPUT_PORT.into(),
        type_params: vec![],
    }));

    let input_ports = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        SUI_VEC_SET_MODULE.into(),
        SUI_VEC_SET_EMPTY.into(),
        vec![input_port_type.clone()],
        vec![],
    );

    for input_port in vertex.input_ports {
        // `input_port: InputPort`
        let input_port = into_sui_ascii_string(tx, input_port.as_str())?;

        let input_port = tx.programmable_move_call(
            workflow_pkg_id,
            WORKFLOW_DAG_MODULE.into(),
            WORKFLOW_DAG_INPUT_PORT_FROM_STRING.into(),
            vec![],
            vec![input_port],
        );

        // `input_ports.insert(input_port)`
        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            SUI_VEC_SET_MODULE.into(),
            SUI_VEC_SET_INSERT.into(),
            vec![input_port_type.clone()],
            vec![input_ports, input_port],
        );
    }

    // `dag.with_entry_vertex(name, kind, input_ports)`
    tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_WITH_ENTRY_VERTEX.into(),
        vec![],
        vec![dag, name, kind, input_ports],
    );

    Ok(())
}

/// Craft transaction arguments to set the default value for
/// a [crate::dag::parser::Vertex] input port on-chain.
fn create_default_value(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    default_value: &DefaultValue,
) -> AnyResult<()> {
    // `vertex: Vertex`
    let vertex = into_sui_ascii_string(tx, default_value.vertex.as_str())?;

    let vertex = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![vertex],
    );

    // `port: InputPort`
    let port = into_sui_ascii_string(tx, default_value.input_port.as_str())?;

    let port = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_INPUT_PORT_FROM_STRING.into(),
        vec![],
        vec![port],
    );

    // `value: NexusData`
    let value = match &default_value.value {
        Data::Inline { data } => {
            let value = tx.pure(serde_json::to_string(data)?.into_bytes())?;

            tx.programmable_move_call(
                primitives_pkg_id,
                PRIMITIVES_DATA_MODULE.into(),
                PRIMITIVES_DATA_INLINE_ONE.into(),
                vec![],
                vec![value],
            )
        }
        // Allowing to remind us that any other data storages can be added here.
        #[allow(unreachable_patterns)]
        _ => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>")
        }
    };

    // `dag.with_default_value(vertex, port, value)`
    tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_WITH_DEFAULT_VALUE.into(),
        vec![],
        vec![dag, vertex, port, value],
    );

    Ok(())
}

/// Craft transaction arguments to create a [crate::dag::parser::Vertex] on-chain.
fn create_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    vertex: &Vertex,
) -> AnyResult<()> {
    // `name: Vertex`
    let name = into_sui_ascii_string(tx, vertex.name.as_str())?;

    let name = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![name],
    );

    // `kind: VertexKind`
    let kind = match &vertex.kind {
        VertexKind::OffChain { tool_fqn } => {
            // `tool_fqn: AsciiString`
            let tool_fqn = into_sui_ascii_string(tx, tool_fqn.to_string().as_str())?;

            tx.programmable_move_call(
                workflow_pkg_id,
                WORKFLOW_DAG_MODULE.into(),
                WORKFLOW_DAG_VERTEX_OFF_CHAIN.into(),
                vec![],
                vec![tool_fqn],
            )
        }
        VertexKind::OnChain { .. } => {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
        }
    };

    // `dag.with_vertex(name, kind)`
    tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_WITH_VERTEX.into(),
        vec![],
        vec![dag, name, kind],
    );

    Ok(())
}

/// Craft transaction arguments to create a [crate::dag::parser::Edge] on-chain.
fn create_edge(
    tx: &mut sui::ProgrammableTransactionBuilder,
    workflow_pkg_id: sui::ObjectID,
    dag: sui::Argument,
    edge: &Edge,
) -> AnyResult<()> {
    // `from_vertex: Vertex`
    let from_vertex = into_sui_ascii_string(tx, edge.from.vertex.as_str())?;

    let from_vertex = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![from_vertex],
    );

    // `from_variant: OutputVariant`
    let from_variant = into_sui_ascii_string(tx, edge.from.output_variant.as_str())?;

    let from_variant = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_OUTPUT_VARIANT_FROM_STRING.into(),
        vec![],
        vec![from_variant],
    );

    // `from_port: OutputPort`
    let from_port = into_sui_ascii_string(tx, edge.from.output_port.as_str())?;

    let from_port = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_OUTPUT_PORT_FROM_STRING.into(),
        vec![],
        vec![from_port],
    );

    // `to_vertex: Vertex`
    let to_vertex = into_sui_ascii_string(tx, edge.to.vertex.as_str())?;

    let to_vertex = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![to_vertex],
    );

    // `to_port: InputPort`
    let to_port = into_sui_ascii_string(tx, edge.to.input_port.as_str())?;

    let to_port = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_INPUT_PORT_FROM_STRING.into(),
        vec![],
        vec![to_port],
    );

    // `dag.with_edge(frpm_vertex, from_variant, from_port, to_vertex, to_port)`
    tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_WITH_EDGE.into(),
        vec![],
        vec![
            dag,
            from_vertex,
            from_variant,
            from_port,
            to_vertex,
            to_port,
        ],
    );

    Ok(())
}

/// Transform a string to Sui `std::ascii::string`.
fn into_sui_ascii_string(
    tx: &mut sui::ProgrammableTransactionBuilder,
    string: &str,
) -> AnyResult<sui::Argument> {
    let ascii = tx.pure(string.as_bytes())?;

    Ok(tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        SUI_ASCII_MODULE.into(),
        SUI_ASCII_FROM_STRING.into(),
        vec![],
        vec![ascii],
    ))
}
