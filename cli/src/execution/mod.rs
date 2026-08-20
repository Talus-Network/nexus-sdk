mod authorize;
mod inspect;
mod output;

use crate::prelude::*;

pub(crate) const EXECUTION_HELP: &str = r#"An Execution is created after a Task occurrence is dispatched.
It is the workflow runtime containing walks, vertex results, failure evidence,
payment locks, and the final outcome. The occurrence remains the durable
scheduler record.

This command replays ordered history, then follows new object versions until
the Execution finishes or the timeout expires.

The human view highlights vertex progress, failure reasons, aborts, and the
final outcome. Use --json for the complete ordered event records.

Identify an Execution directly, or let the CLI resolve it from its occurrence:
  nexus execution inspect --execution-id 0x42
  nexus execution inspect --task-id 0x21 --occurrence-id 7

Authorize one exact active Tool Invocation through an accepted policy:
  nexus execution authorize --execution-id 0x42 --vertex worker fixed-price
  nexus execution authorize --execution-id 0x42 --vertex worker finite-credits --credits-id 0x99

After runtime inspection, return to the occurrence to settle or verify it:
  nexus task occurrence inspect --task-id 0x21 --occurrence-id 7

Walk recovery commands are listed by:
  nexus tap execution --help"#;

/// Commands for workflow execution history and Invocation admission.
#[derive(Subcommand)]
#[command(about = "Inspect workflow executions", long_about = EXECUTION_HELP)]
pub(crate) enum ExecutionCommand {
    #[command(
        about = "Replay and follow one execution",
        long_about = EXECUTION_HELP,
        override_usage = "nexus execution inspect [OPTIONS] (--execution-id <OBJECT_ID> | \
                          --task-id <OBJECT_ID> --occurrence-id <U64>)"
    )]
    Inspect {
        #[command(flatten)]
        identity: ExecutionIdentityArgs,
        #[arg(
            long,
            value_name = "U64",
            requires = "task_id",
            conflicts_with = "execution_id",
            help = "Occurrence that created the Execution"
        )]
        occurrence_id: Option<u64>,
        #[arg(
            long,
            value_name = "SECONDS",
            default_value_t = 3600,
            value_parser = clap::value_parser!(u64).range(1..),
            help = "Maximum observation time"
        )]
        timeout_secs: u64,
        #[arg(
            long,
            value_name = "MILLISECONDS",
            default_value_t = 1000,
            value_parser = clap::value_parser!(u64).range(1..),
            help = "Delay between object reads"
        )]
        poll_ms: u64,
    },
    #[command(about = "Authorize one exact active Tool Invocation")]
    Authorize {
        #[arg(
            long,
            short = 'e',
            value_name = "OBJECT_ID",
            help = "Execution object ID"
        )]
        execution_id: nexus_sdk::sui::types::Address,
        #[arg(long, value_name = "NAME", help = "DAG vertex name")]
        vertex: String,
        #[arg(
            long,
            value_name = "U64",
            requires = "out_of",
            help = "Zero based iterator position"
        )]
        iteration: Option<u64>,
        #[arg(
            long = "out-of",
            value_name = "U64",
            requires = "iteration",
            value_parser = clap::value_parser!(u64).range(1..),
            help = "Total iterator item count"
        )]
        out_of: Option<u64>,
        #[command(subcommand)]
        policy: InvocationPolicyCommand,
        #[command(flatten)]
        gas: GasArgs,
    },
}

#[derive(Subcommand)]
pub(crate) enum InvocationPolicyCommand {
    #[command(about = "Pay the invocation price snapshotted by the Execution")]
    FixedPrice,
    #[command(about = "Use owner enabled sponsored free access")]
    Free,
    #[command(about = "Consume one unit from a finite Credits object")]
    FiniteCredits {
        #[arg(long, value_name = "OBJECT_ID", help = "Shared Credits object ID")]
        credits_id: nexus_sdk::sui::types::Address,
    },
    #[command(about = "Use an immutable TimePass object")]
    TimePass {
        #[arg(long, value_name = "OBJECT_ID", help = "Immutable TimePass object ID")]
        pass_id: nexus_sdk::sui::types::Address,
    },
    #[command(
        about = "Call an arbitrary accepted policy",
        long_about = "Call an arbitrary accepted policy. Every policy exposes get_invocation. Arguments preserve command order and use object:<ID>, mutable:<ID>, id:<ID>, or pure:<BCS_HEX>."
    )]
    Custom {
        #[arg(
            long,
            value_name = "TYPE_NAME",
            help = "Policy witness as <PACKAGE>::<MODULE>::Policy"
        )]
        policy: String,
        #[arg(
            long = "argument",
            value_name = "KIND:VALUE",
            action = clap::ArgAction::Append,
            help = "Policy argument in exact Move parameter order"
        )]
        arguments: Vec<String>,
    },
}

#[derive(Args)]
#[group(
    id = "execution_identity",
    required = true,
    multiple = false,
    args = ["execution_id", "task_id"]
)]
pub(crate) struct ExecutionIdentityArgs {
    #[arg(
        long,
        short = 'e',
        value_name = "OBJECT_ID",
        conflicts_with_all = ["task_id", "occurrence_id"],
        help = "Execution object ID"
    )]
    execution_id: Option<nexus_sdk::sui::types::Address>,
    #[arg(
        long,
        value_name = "OBJECT_ID",
        requires = "occurrence_id",
        conflicts_with = "execution_id",
        help = "Task that owns the occurrence"
    )]
    task_id: Option<nexus_sdk::sui::types::Address>,
}

pub(crate) async fn handle(command: ExecutionCommand) -> AnyResult<(), NexusCliError> {
    match command {
        ExecutionCommand::Inspect {
            identity,
            occurrence_id,
            timeout_secs,
            poll_ms,
        } => {
            let ExecutionIdentityArgs {
                execution_id,
                task_id,
            } = identity;
            inspect::run(
                execution_id,
                task_id.zip(occurrence_id),
                timeout_secs,
                poll_ms,
            )
            .await
        }
        ExecutionCommand::Authorize {
            execution_id,
            vertex,
            iteration,
            out_of,
            policy,
            gas,
        } => authorize::run(execution_id, vertex, iteration.zip(out_of), policy, gas).await,
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{Cli, Command},
        clap::Parser,
    };

    #[test]
    fn execution_inspect_accepts_an_execution_id() {
        let cli = Cli::try_parse_from(["nexus", "execution", "inspect", "--execution-id", "0x42"])
            .unwrap();

        assert!(matches!(cli.command, Command::Execution(_)));
    }

    #[test]
    fn execution_authorize_selects_a_policy_and_exact_runtime_vertex() {
        let cli = Cli::try_parse_from([
            "nexus",
            "execution",
            "authorize",
            "--execution-id",
            "0x42",
            "--vertex",
            "worker",
            "--iteration",
            "2",
            "--out-of",
            "5",
            "finite-credits",
            "--credits-id",
            "0x99",
        ])
        .expect("Invocation authorization should parse");

        assert!(matches!(cli.command, Command::Execution(_)));
    }

    #[test]
    fn iterator_authorization_requires_complete_runtime_identity() {
        assert!(Cli::try_parse_from([
            "nexus",
            "execution",
            "authorize",
            "--execution-id",
            "0x42",
            "--vertex",
            "worker",
            "--iteration",
            "2",
            "fixed-price",
        ])
        .is_err());
    }

    #[test]
    fn execution_inspect_accepts_an_occurrence_identity() {
        let cli = Cli::try_parse_from([
            "nexus",
            "execution",
            "inspect",
            "--task-id",
            "0x42",
            "--occurrence-id",
            "7",
            "--timeout-secs",
            "30",
            "--poll-ms",
            "250",
        ])
        .unwrap();

        assert!(matches!(cli.command, Command::Execution(_)));
        assert!(
            Cli::try_parse_from(["nexus", "execution", "inspect", "--task-id", "0x42",]).is_err()
        );
    }

    #[test]
    fn execution_inspect_missing_identity_reports_alternatives() {
        let error = match Cli::try_parse_from(["nexus", "execution", "inspect"]) {
            Ok(_) => panic!("an execution identity should be required"),
            Err(error) => error,
        };
        let message = error.to_string();

        assert!(message.contains("--execution-id <OBJECT_ID>"));
        assert!(message.contains("--task-id <OBJECT_ID>"));
        assert!(message.contains("--occurrence-id <U64>"));
        assert!(message.contains("Usage:"));
        assert!(message.contains('|'));
    }

    #[test]
    fn execution_help_explains_its_occurrence_relationship_and_recovery() {
        for args in [
            ["nexus", "execution", "--help", ""],
            ["nexus", "execution", "inspect", "--help"],
        ] {
            let args = args.into_iter().filter(|arg| !arg.is_empty());
            let error = match Cli::try_parse_from(args) {
                Ok(_) => panic!("help exits through Clap"),
                Err(error) => error,
            };
            let help = error.to_string();

            for expected in [
                "created after a Task occurrence is dispatched",
                "--execution-id",
                "--task-id",
                "--occurrence-id",
                "nexus task occurrence inspect",
                "nexus tap execution --help",
            ] {
                assert!(
                    help.contains(expected),
                    "missing '{expected}' from help:\n{help}"
                );
            }
        }
    }
}
