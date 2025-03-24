mod network_create;

use {crate::prelude::*, network_create::*, std::default};

#[derive(Subcommand)]
pub(crate) enum NetworkCommand {
    #[command(
        about = "Create a new Nexus network and assign leader caps to the provided addresses"
    )]
    Create {
        /// Space separated list of addresses to assign leader caps to
        #[arg(
            long = "addresses",
            short = 'a',
            help = "Space separated list of addresses to assign leader caps to",
            num_args = 0..,
            value_name = "ADDRESSES"
        )]
        addresses: Vec<sui::ObjectID>,
        /// How many leader caps to assign to each address
        #[arg(
            long = "count-leader-caps",
            short = 'c',
            help = "How many leader caps to assign to each address",
            default_value = "5",
            value_name = "COUNT"
        )]
        count_leader_caps: u32,
        #[command(flatten)]
        gas: GasArgs,
    },
}

/// Handle the provided dag command. The [NetworkCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: NetworkCommand) -> AnyResult<(), NexusCliError> {
    match command {
        // == `$ nexus network create` ==
        NetworkCommand::Create {
            addresses,
            count_leader_caps,
            gas,
        } => {
            create_network(
                addresses,
                count_leader_caps,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
    }
}
