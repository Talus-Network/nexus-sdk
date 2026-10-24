mod cli_conf;
mod completion;
mod conf;
mod dag;
mod display;
mod error;
mod execution;
mod gas;
mod move_package;
mod network;
mod prelude;
mod sui;
mod tap;
mod task;
mod tool;
mod walrus;
mod workflow;

use {crate::prelude::*, std::io::IsTerminal as _};

#[derive(Parser)]
#[command(version, about = "Nexus CLI")]
struct Cli {
    /// Whether to output JSON.
    #[arg(
        global = true,
        long = "json",
        help = "Emit machine readable JSON",
        help_heading = "Output"
    )]
    json: bool,

    #[command(flatten)]
    verbosity: CliVerbosity,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(subcommand, about = "Manage wallet funded Walrus storage")]
    Walrus(walrus::WalrusCommand),
    #[command(subcommand, about = "Manage Nexus Tools")]
    Tool(tool::ToolCommand),
    #[command(subcommand, about = "Manage Nexus Configuration")]
    Conf(conf::ConfCommand),
    #[command(subcommand, about = "Validate and publish Nexus DAGs")]
    Dag(dag::DagCommand),
    #[command(subcommand, about = "Create and operate scheduled Tasks")]
    Task(Box<task::TaskCommand>),
    #[command(
        subcommand,
        about = "Inspect and authorize workflow executions",
        long_about = execution::EXECUTION_HELP
    )]
    Execution(execution::ExecutionCommand),
    #[command(subcommand, about = "Manage Sui transaction gas")]
    Gas(gas::GasCommand),
    #[command(subcommand, about = "Manage Nexus network fees")]
    Network(network::NetworkCommand),
    #[command(subcommand, about = "Prepare, test and operate TAP applications")]
    Tap(tap::TapCommand),
    #[command(about = "Provide shell completions")]
    Completion(completion::CompletionCommand),
}

#[derive(Args)]
struct CliVerbosity {
    #[arg(
        global = true,
        short = 'v',
        long,
        action = clap::ArgAction::Count,
        conflicts_with = "quiet",
        help = "Increase diagnostic detail",
        help_heading = "Output"
    )]
    verbose: u8,
    #[arg(
        global = true,
        short = 'q',
        long,
        action = clap::ArgAction::Count,
        conflicts_with = "verbose",
        help = "Show errors only",
        help_heading = "Output"
    )]
    quiet: u8,
}

impl CliVerbosity {
    fn log_level_filter(&self) -> log::LevelFilter {
        if self.quiet > 0 {
            return log::LevelFilter::Off;
        }
        match self.verbose {
            0 => log::LevelFilter::Error,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    }

    fn is_quiet(&self) -> bool {
        self.quiet > 0
    }

    fn is_verbose(&self) -> bool {
        self.verbose > 0
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    env_logger::builder()
        .filter(None, cli.verbosity.log_level_filter())
        .init();

    JSON_MODE.store(cli.json, Ordering::Relaxed);
    let error_report_mode = if cli.json {
        error::ErrorReportMode::Json
    } else if cli.verbosity.is_quiet() {
        error::ErrorReportMode::Quiet
    } else {
        let verbose = cli.verbosity.is_verbose();
        let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        error::ErrorReportMode::Human { verbose, color }
    };

    // Send each sub-command to the respective handler.
    let result = match cli.command {
        Command::Walrus(command) => walrus::handle(command).await,
        Command::Tool(tool) => tool::handle(tool).await,
        Command::Conf(conf) => conf::handle(conf).await,
        Command::Dag(dag) => dag::handle(dag).await,
        Command::Gas(gas) => gas::handle(gas).await,
        Command::Network(network) => network::handle(network).await,
        Command::Tap(tap) => tap::handle(tap).await,
        Command::Task(task) => task::handle(*task).await,
        Command::Execution(execution) => execution::handle(execution).await,
        Command::Completion(completion) => completion::handle(completion, cli.json),
    };

    // Handle any errors that occurred during command execution.
    if let Err(e) = result {
        let report = error::render_error(&e, error_report_mode);
        if matches!(error_report_mode, error::ErrorReportMode::Human { .. }) {
            eprintln!();
        }
        eprint!("{report}");
        if !report.ends_with('\n') {
            eprintln!();
        }

        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{sui, test_utils::sui_mocks},
        std::ffi::OsString,
    };

    struct CliEnv(Vec<(&'static str, Option<OsString>)>);

    impl CliEnv {
        fn new(conf_path: &std::path::Path) -> Self {
            let guard = Self(
                [CLI_CONF_PATH_ENV, "SUI_RPC_URL", "SUI_PK"]
                    .into_iter()
                    .map(|name| (name, std::env::var_os(name)))
                    .collect(),
            );
            std::env::set_var(CLI_CONF_PATH_ENV, conf_path);
            std::env::remove_var("SUI_RPC_URL");
            std::env::remove_var("SUI_PK");
            guard
        }
    }

    impl Drop for CliEnv {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn public_reads_reach_rpc_without_a_wallet() {
        // Match the CLI main thread stack for the large Tool command future.
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
                        ledger.expect_get_object().returning(|_| {
                            Err(tonic::Status::permission_denied("public read reached RPC"))
                        });
                        let mut state = sui_mocks::grpc::MockStateService::new();
                        state.expect_list_dynamic_fields().returning(|_| {
                            Err(tonic::Status::permission_denied("public read reached RPC"))
                        });
                        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                            ledger_service_mock: Some(ledger),
                            state_service_mock: Some(state),
                            ..Default::default()
                        });
                        let directory = tempfile::tempdir().unwrap();
                        let path = directory.path().join("conf.toml");
                        let mut conf = CliConf {
                            sui: SuiConf {
                                rpc_url: Some(rpc_url.parse().unwrap()),
                                pk: None,
                            },
                            nexus: Some(sui_mocks::mock_nexus_objects()),
                            ..Default::default()
                        };
                        conf.agents.insert(
                            "someone_else".into(),
                            sui::types::Address::from_static("0xa"),
                        );
                        conf.save_to_path(&path).await.unwrap();
                        let _env = CliEnv::new(&path);
                        let commands: &[&[&str]] = &[
                            &[
                                "nexus",
                                "tap",
                                "requirements",
                                "--agent-id",
                                "0xa",
                                "--skill-id",
                                "0",
                            ],
                            &["nexus", "tap", "vault", "balance", "--agent-id", "0xa"],
                            &[
                                "nexus",
                                "tap",
                                "vault",
                                "balance",
                                "--alias",
                                "someone_else",
                            ],
                            &[
                                "nexus",
                                "tool",
                                "cashier",
                                "access",
                                "--tool-fqn",
                                "com.example.tool@1",
                                "--beneficiary-user",
                                "0xa",
                            ],
                            &[
                                "nexus",
                                "tool",
                                "cashier",
                                "access",
                                "--tool-fqn",
                                "com.example.tool@1",
                                "--beneficiary-agent",
                                "0xa",
                            ],
                        ];
                        for key in [None, Some("invalid key must be ignored by public reads")] {
                            if let Some(key) = key {
                                std::env::set_var("SUI_PK", key);
                            }
                            for args in commands {
                                let command = Cli::try_parse_from(*args).unwrap().command;
                                let result = match command {
                                    Command::Tap(command) => tap::handle(command).await,
                                    Command::Tool(command) => tool::handle(command).await,
                                    _ => unreachable!(),
                                };
                                let error = anyhow::Error::new(
                                    result.expect_err("mock RPC rejects the public read"),
                                );
                                assert!(
                                    format!("{error:#}").contains("public read reached RPC"),
                                    "{args:?}: {error:#}"
                                );
                            }
                        }
                        use base64::Engine as _;
                        let valid_key = base64::prelude::BASE64_STANDARD.encode([23; 32]);
                        for (key, expected_error) in [
                            (None, "Sui private key is not configured"),
                            (Some("invalid key"), "Failed to decode Sui private key"),
                            (Some(valid_key.as_str()), "public read reached RPC"),
                        ] {
                            match key {
                                Some(key) => std::env::set_var("SUI_PK", key),
                                None => std::env::remove_var("SUI_PK"),
                            }
                            let command = Cli::try_parse_from([
                                "nexus",
                                "tool",
                                "cashier",
                                "access",
                                "--tool-fqn",
                                "com.example.tool@1",
                            ])
                            .unwrap()
                            .command;
                            let Command::Tool(command) = command else {
                                unreachable!()
                            };
                            let error =
                                anyhow::Error::new(tool::handle(command).await.unwrap_err());
                            assert!(format!("{error:#}").contains(expected_error), "{error:#}");
                        }
                    });
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn walrus_estimate_needs_no_wallet() {
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        ledger.expect_get_service_info().returning(|_| {
            let mut response = sui::grpc::GetServiceInfoResponse::default();
            response.chain_id = Some(sui::types::Digest::ZERO.to_string());
            response.chain = Some("testnet".into());
            Ok(response.into())
        });
        ledger.expect_batch_get_objects().returning(|request| {
            let requested: Vec<_> = request
                .get_ref()
                .requests
                .iter()
                .map(|object| object.object_id.as_deref().unwrap())
                .collect();
            let (system, staking) = nexus_sdk::walrus::WalrusNetwork::Testnet.contracts();
            assert_eq!(requested, vec![system, staking]);
            Err(tonic::Status::permission_denied(
                "estimate reached public RPC",
            ))
        });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("conf.toml");
        let conf = CliConf {
            sui: SuiConf {
                rpc_url: Some(rpc_url.parse().unwrap()),
                pk: None,
            },
            ..Default::default()
        };
        conf.save_to_path(&path).await.unwrap();
        let _env = CliEnv::new(&path);
        let input = directory.path().join("input.json");
        tokio::fs::write(&input, br#"["first", "second"]"#)
            .await
            .unwrap();
        for key in [None, Some("invalid key must be ignored by estimates")] {
            if let Some(key) = key {
                std::env::set_var("SUI_PK", key);
            }
            let command = Cli::try_parse_from([
                "nexus",
                "walrus",
                "upload",
                input.to_str().unwrap(),
                "--many",
                "--estimate",
            ])
            .unwrap()
            .command;
            let Command::Walrus(command) = command else {
                unreachable!()
            };
            let error = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                Box::pin(walrus::handle(command)),
            )
            .await
            .expect("estimate should finish")
            .unwrap_err();
            let error = anyhow::Error::new(error);
            assert!(
                format!("{error:#}").contains("estimate reached public RPC"),
                "{error:#}"
            );
            assert!(!directory.path().join("input.json.walrus.json").exists());
            assert!(!directory.path().join("input.json.walrus.uploads").exists());

            let command = Cli::try_parse_from([
                "nexus",
                "walrus",
                "upload",
                input.to_str().unwrap(),
                "--many",
            ])
            .unwrap()
            .command;
            let Command::Walrus(command) = command else {
                unreachable!()
            };
            let error = walrus::handle(command).await.unwrap_err();
            let expected_error = if key.is_some() {
                "Failed to decode Sui private key"
            } else {
                "Sui private key is not configured"
            };
            assert!(error.to_string().contains(expected_error), "{error}");
        }
    }

    #[test]
    fn root_help_describes_diagnostic_output_without_domain_ambiguity() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();

        assert!(help.contains("Increase diagnostic detail"));
        assert!(help.contains("Show errors only"));
        assert!(!help.contains("per occurrence"));
    }

    #[test]
    fn verbose_and_quiet_are_mutually_exclusive() {
        let error = match Cli::try_parse_from([
            "nexus",
            "-v",
            "-q",
            "task",
            "list",
            "--scheduler-package",
            "0xa5",
        ]) {
            Ok(_) => panic!("verbose and quiet should conflict"),
            Err(error) => error,
        };

        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn completion_rejects_json_mode() {
        let cli = Cli::try_parse_from(["nexus", "--json", "completion", "bash"]).unwrap();
        assert!(cli.json);
    }
}
