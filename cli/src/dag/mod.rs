mod dag_abort_expired_execution;
mod dag_execution_cost;
mod dag_inspect_execution;
mod dag_publish;
mod dag_validate;

use {
    crate::prelude::*,
    dag_abort_expired_execution::*,
    dag_execution_cost::*,
    dag_inspect_execution::*,
    dag_publish::*,
    dag_validate::*,
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
        about = "Publish a Nexus DAG spec file to the currently active Sui net. This command also performs validation on the file before publishing."
    )]
    Publish {
        /// The path to the Nexus DAG spec file to publish.
        #[arg(
            long = "path",
            short = 'p',
            help = "The path to the Nexus DAG spec file to publish",
            value_parser = ValueParser::from(expand_tilde)
        )]
        path: PathBuf,
        #[command(flatten)]
        gas: GasArgs,
    },

    #[command(about = "Inspect a Nexus DAG execution.")]
    InspectExecution {
        /// The object ID of the Nexus DAGExecution object.
        #[arg(
            long = "dag-execution-id",
            short = 'e',
            help = "The object ID of the Nexus DAGExecution object.",
            value_name = "OBJECT_ID"
        )]
        dag_execution_id: sui::types::Address,
    },

    #[command(about = "Show the standard TAP execution payment consumed by a DAG execution.")]
    ExecutionCost {
        /// The object ID of the Nexus DAGExecution object.
        #[arg(
            long = "dag-execution-id",
            short = 'e',
            help = "The object ID of the Nexus DAGExecution object.",
            value_name = "OBJECT_ID"
        )]
        dag_execution_id: sui::types::Address,
    },

    #[command(about = "Trigger the ToolGas-assisted abort flow for an expired DAG execution.")]
    AbortExpiredExecution {
        /// The object ID of the Nexus DAGExecution object.
        #[arg(
            long = "dag-execution-id",
            short = 'e',
            help = "The object ID of the Nexus DAGExecution object.",
            value_name = "OBJECT_ID"
        )]
        dag_execution_id: sui::types::Address,
        /// Optional ToolGas object ID to require. When omitted, the first eligible candidate is used.
        #[arg(
            long = "tool-gas-id",
            help = "ToolGas object ID to use when it is eligible for this abort.",
            value_name = "OBJECT_ID"
        )]
        tool_gas_id: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
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

        // == `$ nexus dag inspect-execution` ==
        DagCommand::InspectExecution { dag_execution_id } => {
            inspect_dag_execution(dag_execution_id).await
        }

        // == `$ nexus dag execution-cost` ==
        DagCommand::ExecutionCost { dag_execution_id } => execution_cost(dag_execution_id).await,

        // == `$ nexus dag abort-expired-execution` ==
        DagCommand::AbortExpiredExecution {
            dag_execution_id,
            tool_gas_id,
            gas,
        } => {
            abort_expired_execution(
                dag_execution_id,
                tool_gas_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}
