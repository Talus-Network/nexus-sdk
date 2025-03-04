use crate::{command_title, loading, prelude::*, sui::*};

/// Sui `sui::object`
const SUI_OBJECT_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("object");
const SUI_OBJECT_ID_FROM_ADDRESS: &sui::MoveIdentStr = sui::move_ident_str!("id_from_address");

/// Sui `sui::address`
const SUI_ADDRESS_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("address");
const SUI_ADDRESS_FROM_ASCII_BYTES: &sui::MoveIdentStr = sui::move_ident_str!("from_ascii_bytes");

/// Sui `sui::vec_map`
const SUI_VEC_MAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("vec_map");
const SUI_VEC_MAP_EMPTY: &sui::MoveIdentStr = sui::move_ident_str!("empty");
const SUI_VEC_MAP_INSERT: &sui::MoveIdentStr = sui::move_ident_str!("insert");

/// Sui `std::ascii::string`
const SUI_ASCII_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("ascii");
const SUI_ASCII_FROM_STRING: &sui::MoveIdentStr = sui::move_ident_str!("string");

// Nexus `workflow::default_sap` module and its functions.
const WORKFLOW_DEFAULT_SAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("default_sap");
const WORKFLOW_DEFAULT_SAP_BEGIN_DAG_EXECUTION: &sui::MoveIdentStr =
    sui::move_ident_str!("begin_dag_execution");

// Nexus `workflow::dag` module and its functions.
const WORKFLOW_DAG_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("dag");

const WORKFLOW_DAG_VERTEX_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("vertex_from_string");
const WORKFLOW_DAG_INPUT_PORT_FROM_STRING: &sui::MoveIdentStr =
    sui::move_ident_str!("input_port_from_string");

const WORKFLOW_INPUT_PORT: &sui::MoveIdentStr = sui::move_ident_str!("InputPort");

// Nexus `primitives::data` module and its functions.
const PRIMITIVES_DATA_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("data");
const PRIMITIVES_DATA_INLINE_ONE: &sui::MoveIdentStr = sui::move_ident_str!("inline_one");

const PRIMITIVES_DATA_NEXUS_DATA: &sui::MoveIdentStr = sui::move_ident_str!("NexusData");

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_vertex: String,
    input_json: serde_json::Value,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        primitives_pkg_id,
        default_sap_object_id,
        network_id,
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

    // Fetch DAG object for its ObjectRef.
    let dag = fetch_object_by_id(&sui, dag_id).await?;

    // Fetch DefaultSAP object for its ObjectRef.
    let default_sap = fetch_object_by_id(&sui, default_sap_object_id).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match prepare_transaction(
        default_sap,
        dag,
        entry_vertex,
        input_json,
        workflow_pkg_id,
        primitives_pkg_id,
        network_id,
    ) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let _response = sign_transaction(&sui, &wallet, tx_data).await?;

    Ok(())
}

/// Build a programmable transaction to execute a DAG.
fn prepare_transaction(
    default_sap: sui::ObjectRef,
    dag: sui::ObjectRef,
    entry_vertex: String,
    input_json: serde_json::Value,
    workflow_pkg_id: sui::ObjectID,
    primitives_pkg_id: sui::ObjectID,
    network_id: sui::ObjectID,
) -> AnyResult<sui::ProgrammableTransactionBuilder> {
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // `self: &mut DefaultSAP`
    let default_sap = tx.obj(sui::ObjectArg::SharedObject {
        id: default_sap.object_id,
        initial_shared_version: default_sap.version,
        mutable: true,
    })?;

    // `dag: &DAG`
    let dag = tx.obj(sui::ObjectArg::SharedObject {
        id: dag.object_id,
        initial_shared_version: dag.version,
        mutable: false,
    })?;

    // `network: ID`
    let network = tx.pure(network_id.to_canonical_string(false).as_bytes())?;

    let network = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        SUI_ADDRESS_MODULE.into(),
        SUI_ADDRESS_FROM_ASCII_BYTES.into(),
        vec![],
        vec![network],
    );

    let network = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        SUI_OBJECT_MODULE.into(),
        SUI_OBJECT_ID_FROM_ADDRESS.into(),
        vec![],
        vec![network],
    );

    // `entry_vertex: Vertex`
    let entry_vertex = into_sui_ascii_string(&mut tx, entry_vertex.as_str())?;

    let entry_vertex = tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DAG_MODULE.into(),
        WORKFLOW_DAG_VERTEX_FROM_STRING.into(),
        vec![],
        vec![entry_vertex],
    );

    // `with_vertex_input: VecMap<InputPort, NexusData>`
    let vec_map_type = vec![
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *workflow_pkg_id,
            module: WORKFLOW_DAG_MODULE.into(),
            name: WORKFLOW_INPUT_PORT.into(),
            type_params: vec![],
        })),
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *primitives_pkg_id,
            module: PRIMITIVES_DATA_MODULE.into(),
            name: PRIMITIVES_DATA_NEXUS_DATA.into(),
            type_params: vec![],
        })),
    ];

    let with_vertex_input = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        SUI_VEC_MAP_MODULE.into(),
        SUI_VEC_MAP_EMPTY.into(),
        vec_map_type.clone(),
        vec![],
    );

    let Some(data) = input_json.as_object() else {
        bail!(
            "Input JSON must be an object containing the input ports and their respective values."
        );
    };

    for (port, value) in data {
        let port = into_sui_ascii_string(&mut tx, port.as_str())?;

        let port = tx.programmable_move_call(
            workflow_pkg_id,
            WORKFLOW_DAG_MODULE.into(),
            WORKFLOW_DAG_INPUT_PORT_FROM_STRING.into(),
            vec![],
            vec![port],
        );

        let value = tx.pure(serde_json::to_string(value)?.into_bytes())?;

        let value = tx.programmable_move_call(
            primitives_pkg_id,
            PRIMITIVES_DATA_MODULE.into(),
            PRIMITIVES_DATA_INLINE_ONE.into(),
            vec![],
            vec![value],
        );

        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            SUI_VEC_MAP_MODULE.into(),
            SUI_VEC_MAP_INSERT.into(),
            vec_map_type.clone(),
            vec![with_vertex_input, port, value],
        );
    }

    // `workflow::default_sap::begin_dag_execution()`
    tx.programmable_move_call(
        workflow_pkg_id,
        WORKFLOW_DEFAULT_SAP_MODULE.into(),
        WORKFLOW_DEFAULT_SAP_BEGIN_DAG_EXECUTION.into(),
        vec![],
        vec![default_sap, dag, network, entry_vertex, with_vertex_input],
    );

    Ok(tx)
}

/// Transform a string to Sui `std::ascii::string`.
// TODO: extract this.
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
