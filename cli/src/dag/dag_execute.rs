use {
    crate::{
        command_title,
        dag::dag_inspect_execution::inspect_dag_execution,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
        workflow,
    },
    anyhow::anyhow,
    nexus_sdk::{
        nexus::{error::NexusError, workflow::AgentDagExecuteOptions},
        types::tap_payment_source_for_address,
    },
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_dag(
    dag_id: sui::types::Address,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    inspect: bool,
    priority_fee_per_gas_unit: u64,
    payment_coin: Option<sui::types::Address>,
    payment_budget: Option<u64>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let pk = get_signing_key(&conf).await?;
    let owner = pk.public_key().derive_address();
    if payment_coin.is_none() {
        return Err(NexusCliError::Any(anyhow!(
            "nexus dag execute requires --payment-coin for default agent DAG execution"
        )));
    }
    if let (Some(gas_coin), Some(payment_coin)) = (sui_gas_coin, payment_coin) {
        if gas_coin == payment_coin {
            return Err(NexusCliError::Any(anyhow!(
                "--sui-gas-coin and --payment-coin must be different objects"
            )));
        }
    }
    let payment_coin_id = payment_coin.expect("checked above");
    let client = build_sui_grpc_client(&conf).await?;
    let (gas_coin, _) = fetch_coin_with_balance_excluding(
        client.clone(),
        owner,
        sui_gas_coin,
        0,
        &[payment_coin_id],
    )
    .await?;
    if *gas_coin.object_id() == payment_coin_id {
        return Err(NexusCliError::Any(anyhow!(
            "auto-selected gas coin matches --payment-coin; pass --sui-gas-coin with a different coin"
        )));
    }
    let (payment_coin, balance) =
        fetch_coin_with_balance(client.clone(), owner, Some(payment_coin_id), 0).await?;
    let budget = payment_budget.unwrap_or(balance);
    if budget > balance {
        return Err(NexusCliError::Any(anyhow!(
            "payment budget {budget} exceeds payment coin balance {balance}"
        )));
    }

    let nexus_client = get_nexus_client(Some(*gas_coin.object_id()), sui_gas_budget).await?;

    // Build the remote storage conf.
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf = conf.data_storage.clone().into();

    // Store ports remote if they need to be stored remotely.
    let input_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;
    let agent_dag_options = AgentDagExecuteOptions {
        payment_source: tap_payment_source_for_address(owner).map_err(NexusCliError::Any)?,
        payment_coin: Some(payment_coin),
        payment_coin_balance: Some(balance),
        payment_max_budget: budget,
        payment_refund_mode: 0,
        authorization_plan_commitment: None,
        authorization_plan: Vec::new(),
    };

    let tx_handle = loading!("Crafting and executing transaction...");

    let workflow = nexus_client.workflow();
    let result = match workflow
        .execute_default_agent_dag(
            dag_id,
            input_data,
            priority_fee_per_gas_unit,
            Some(&entry_group),
            &storage_conf,
            agent_dag_options,
        )
        .await
    {
        Ok(r) => r,
        Err(NexusError::Storage(e)) => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "{e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                e = e,
                command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
            )));
        }
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    notify_success!(
        "DAGExecution checkpoint: {id}",
        id = result.tx_checkpoint.to_string().truecolor(100, 100, 100)
    );

    if inspect {
        inspect_dag_execution(result.execution_object_id, result.tx_checkpoint).await?;
    } else {
        json_output(&json!({
            "execution_id": result.execution_object_id,
            "digest": result.tx_digest,
            "tx_checkpoint": result.tx_checkpoint
        }))?;
    }

    Ok(())
}
