mod dag_inspect;
mod dag_publish;
mod dag_validate;

use {crate::prelude::*, dag_inspect::*, dag_publish::*, dag_validate::*};

#[derive(Subcommand)]
pub(crate) enum DagCommand {
    #[command(about = "Inspect published DAG entry groups and required inputs")]
    Inspect {
        #[arg(
            long,
            short = 'd',
            value_name = "OBJECT_ID",
            help = "Published DAG object ID"
        )]
        dag_id: sui::types::Address,
    },

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
}

/// Handle the provided dag command. The [DagCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: DagCommand) -> AnyResult<(), NexusCliError> {
    match command {
        DagCommand::Inspect { dag_id } => inspect_dag(dag_id).await,

        // == `$ nexus dag validate` ==
        DagCommand::Validate { path } => validate_dag(path).await.map(|_| ()),

        // == `$ nexus dag publish` ==
        DagCommand::Publish { path, gas } => {
            publish_dag(path, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn inspect_accepts_a_published_dag_id() {
        let cli = crate::Cli::try_parse_from(["nexus", "dag", "inspect", "--dag-id", "0x42"])
            .expect("DAG inspection should parse");

        assert!(matches!(
            cli.command,
            crate::Command::Dag(DagCommand::Inspect { .. })
        ));
    }
}
