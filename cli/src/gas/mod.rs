mod balance;

use {crate::prelude::*, balance::*};

/// Sui transaction gas and address balance commands.
#[derive(Subcommand)]
pub(crate) enum GasCommand {
    #[command(
        about = "Show SUI address balance and owned coin balance",
        long_about = "Shows the configured signer's SUI address balance and the total held in \
                      owned Coin<SUI> objects. Scheduler reserves and default transaction gas use \
                      the address balance."
    )]
    Balance,

    #[command(
        about = "Deposit an owned SUI coin amount into the address balance",
        long_about = "Moves MIST from an owned Coin<SUI> object into the configured signer's SUI \
                      address balance. When --sui-gas-coin is omitted, the command selects a \
                      sufficient owned coin deterministically. The selected coin must cover the \
                      deposit amount and transaction gas budget."
    )]
    Deposit {
        #[arg(
            long,
            value_name = "MIST",
            help = "Positive MIST amount deposited into the address balance"
        )]
        amount: std::num::NonZeroU64,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn handle(command: GasCommand) -> AnyResult<(), NexusCliError> {
    match command {
        GasCommand::Balance => show().await,
        GasCommand::Deposit { amount, gas } => deposit(amount, gas).await,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn parses_address_balance_commands() {
        let balance = crate::Cli::try_parse_from(["nexus", "gas", "balance"])
            .expect("balance command should parse");
        assert!(matches!(
            balance.command,
            crate::Command::Gas(GasCommand::Balance)
        ));

        let deposit = crate::Cli::try_parse_from([
            "nexus",
            "gas",
            "deposit",
            "--amount",
            "50000000",
            "--sui-gas-budget",
            "12345",
        ])
        .expect("deposit command should parse");
        let crate::Command::Gas(GasCommand::Deposit { amount, gas }) = deposit.command else {
            panic!("expected address balance deposit command");
        };
        assert_eq!(amount.get(), 50_000_000);
        assert_eq!(gas.sui_gas_budget, 12_345);
    }
}
