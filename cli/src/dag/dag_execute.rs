//! Executes DAGs with client-owned gas and payment-coin selection.

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
        types::payment_source_from_address,
    },
};

fn agent_dag_execute_options_from_cli_budget(
    owner: sui::types::Address,
    payment_coin: sui::types::ObjectReference,
    payment_coin_balance: u64,
    payment_max_budget_mist: u64,
) -> AnyResult<AgentDagExecuteOptions, NexusCliError> {
    Ok(AgentDagExecuteOptions {
        payment_source: payment_source_from_address(owner).map_err(NexusCliError::Any)?,
        payment_coin: Some(payment_coin),
        payment_coin_balance: Some(payment_coin_balance),
        payment_max_budget_mist,
    })
}

fn required_payment_coin(
    payment_coin: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
) -> AnyResult<sui::types::Address, NexusCliError> {
    let payment_coin = payment_coin.ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "nexus dag execute requires --payment-coin for default agent DAG execution"
        ))
    })?;

    if sui_gas_coin == Some(payment_coin) {
        return Err(NexusCliError::Any(anyhow!(
            "--sui-gas-coin and --payment-coin must be different objects"
        )));
    }

    Ok(payment_coin)
}

/// Execute a Nexus DAG based on the provided object ID and initial input data.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_dag(
    dag_id: sui::types::Address,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    inspect: bool,
    priority_fee_percentage: u64,
    payment_coin: Option<sui::types::Address>,
    payment_max_budget_mist: Option<u64>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    let payment_coin_id = required_payment_coin(payment_coin, sui_gas_coin)?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let owner = nexus_client.owner().map_err(NexusCliError::Nexus)?;
    let (payment_coin, balance) = nexus_client
        .fetch_coin_with_balance(payment_coin_id)
        .await
        .map_err(NexusCliError::Nexus)?;
    let payment_max_budget_mist = payment_max_budget_mist.unwrap_or(balance);
    if payment_max_budget_mist > balance {
        return Err(NexusCliError::Any(anyhow!(
            "payment maximum budget {payment_max_budget_mist} MIST exceeds payment coin balance {balance} MIST"
        )));
    }

    // Build the remote storage conf.
    let conf = CliConf::load().await.unwrap_or_default();
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf = conf.data_storage.clone().into();

    // Store ports remote if they need to be stored remotely.
    let input_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;
    let agent_dag_options = agent_dag_execute_options_from_cli_budget(
        owner,
        payment_coin,
        balance,
        payment_max_budget_mist,
    )?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let workflow = nexus_client.workflow();
    let result = match workflow
        .execute_default_agent_dag(
            dag_id,
            input_data,
            Some(priority_fee_percentage),
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
        inspect_dag_execution(result.execution_object_id).await?;
    } else {
        json_output(&json!({
            "execution_id": result.execution_object_id,
            "digest": result.tx_digest,
            "tx_checkpoint": result.tx_checkpoint
        }))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::test_utils::sui_mocks};

    #[test]
    fn dag_execute_payment_budget_is_total_escrow() {
        let owner = sui::types::Address::from_static("0xa11ce");
        let payment_coin = sui_mocks::mock_sui_object_ref();

        let options =
            agent_dag_execute_options_from_cli_budget(owner, payment_coin.clone(), 1_000, 120)
                .expect("CLI options should build");

        assert_eq!(options.payment_coin, Some(payment_coin));
        assert_eq!(options.payment_coin_balance, Some(1_000));
        assert_eq!(options.payment_max_budget_mist, 120);
    }

    #[test]
    fn dag_execute_accepts_address_balance_gas() {
        let payment_coin = sui::types::Address::from_static("0x1");

        assert_eq!(
            required_payment_coin(Some(payment_coin), None).expect("gas coin should be optional"),
            payment_coin
        );
    }

    #[test]
    fn dag_execute_rejects_same_explicit_payment_and_gas_coin() {
        let coin = sui::types::Address::from_static("0x1");

        let error = required_payment_coin(Some(coin), Some(coin))
            .expect_err("payment and explicit gas coin must differ");

        assert!(error
            .to_string()
            .contains("--sui-gas-coin and --payment-coin must be different"));
    }
}
