mod gas_add_budget;

use {crate::prelude::*, gas_add_budget::*};

#[derive(Subcommand)]
pub(crate) enum GasCommand {
    #[command(about = "Add a SUI coin as gas budget")]
    AddBudget {
        #[arg(
            long = "coin",
            short = 'c',
            help = "Owned SUI coin object ID to use as budget",
            value_name = "OBJECT_ID"
        )]
        coin: sui::ObjectID,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided gas command. The [GasCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: GasCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus gas add-budget` ==
        GasCommand::AddBudget { coin, gas } => {
            add_gas_budget(coin, gas.sui_gas_coin, gas.sui_gas_budget).await
        } /* TODO:
           * Tool::SetCost
           * Gas::EnableX
           * Gas::DisableX
           * Gas::BuyTicket */
    }
}
