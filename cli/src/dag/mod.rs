mod dag_execute;
mod dag_publish;
mod dag_validate;
mod parser;
mod validator;

use {crate::prelude::*, dag_execute::*, dag_publish::*, dag_validate::*};

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
        #[arg(long = "dag-id", short = 'd', help = "The object ID of the Nexus DAG")]
        dag_id: sui::ObjectID,
        /// The initial input data for the DAG.
        #[arg(
            long = "input-data",
            short = 'i',
            help = "The initial input data for the DAG"
        )]
        input_data: serde_json::Value,
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

        // == `$ nexus dag execute` ==
        DagCommand::Execute {
            dag_id,
            input_data,
            gas,
        } => execute_dag(dag_id, input_data, gas.sui_gas_coin, gas.sui_gas_budget).await,
    }
}
