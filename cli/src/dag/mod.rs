mod dag_execute;
mod dag_inspect_execution;
mod dag_publish;
mod dag_validate;
mod validator;

use {
    crate::prelude::*,
    dag_execute::*,
    dag_inspect_execution::*,
    dag_publish::*,
    dag_validate::*,
    nexus_sdk::types::DEFAULT_ENTRY_GROUP,
};

#[derive(Subcommand)]
pub(crate) enum DagCommand {
    #[command(about = "Validate if a JSON file at the provided location is a valid Nexus DAG.")]
    Validate {
        /// The path to the JSON file to validate.
        #[arg(
            long = "path",
            short = 'p',
            help = "The path to the JSON file to validate",
            value_parser = ValueParser::from(expand_tilde)
        )]
        path: PathBuf,
    },

    #[command(
        about = "Publish a Nexus DAG JSON file to the currently active Sui net. This commands also performs validation on the file before publishing."
    )]
    Publish {
        /// The path to the Nexus DAG JSON file to publish.
        #[arg(
            long = "path",
            short = 'p',
            help = "The path to the Nexus DAG JSON file to publish",
            value_parser = ValueParser::from(expand_tilde)
        )]
        path: PathBuf,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(
        about = "Execute a Nexus DAG based on the provided object ID and initial input data."
    )]
    Execute {
        /// The object ID of the Nexus DAG.
        #[arg(
            long = "dag-id",
            short = 'd',
            help = "The object ID of the Nexus DAG",
            value_name = "OBJECT_ID"
        )]
        dag_id: sui::ObjectID,
        /// The entry group to invoke.
        #[arg(
            long = "entry-group",
            short = 'e',
            help = "The entry group to invoke",
            value_name = "NAME",
            default_value = DEFAULT_ENTRY_GROUP,
        )]
        entry_group: String,
        /// The initial input data for the DAG.
        #[arg(
            long = "input-json",
            short = 'i',
            help = "The initial input data for the DAG as a JSON object. Keys are names of entry vertices and values are the input data.",
            value_parser = ValueParser::from(parse_json_string),
            value_name = "DATA"
        )]
        input_json: serde_json::Value,
        /// Whether to inspect the DAG execution process.
        #[arg(
            long = "inspect",
            short = 'n',
            help = "Whether to inspect the DAG execution process. If not provided, command returns after submitting the transaction."
        )]
        inspect: bool,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(
        about = "Inspect a Nexus DAG execution process based on the provided object ID and execution digest."
    )]
    InspectExecution {
        /// The object ID of the Nexus DAGExecution object.
        #[arg(
            long = "dag-execution-id",
            short = 'e',
            help = "The object ID of the Nexus DAGExecution object.",
            value_name = "OBJECT_ID"
        )]
        dag_execution_id: sui::ObjectID,
        /// The entry group to invoke.
        #[arg(
            long = "execution-digest",
            short = 'd',
            help = "The transaction digest of the execution.",
            value_name = "DIGEST"
        )]
        execution_digest: sui::TransactionDigest,
    },
}

/// Handle the provided dag command. The [DagCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: DagCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus dag validate` ==
        DagCommand::Validate { path } => validate_dag(path).await.map(|_| ()),

        // == `$ nexus dag publish` ==
        DagCommand::Publish { path, gas } => {
            publish_dag(path, gas.sui_gas_coin, gas.sui_gas_budget).await
        }

        // == `$ nexus dag execute` ==
        DagCommand::Execute {
            dag_id,
            entry_group,
            input_json,
            inspect,
            gas,
        } => {
            // Optional: Check auth at CLI level instead of inside execute_dag
            // validate_cli_authentication().await?;

            execute_dag(
                dag_id,
                entry_group,
                input_json,
                inspect,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }

        // == `$ nexus dag inspect-execution` ==
        DagCommand::InspectExecution {
            dag_execution_id,
            execution_digest,
        } => inspect_dag_execution(dag_execution_id, execution_digest).await,
    }
}
