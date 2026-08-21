use {
    super::InvocationPolicyCommand,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::{
        move_bindings::{interface::graph::RuntimeVertex, move_std::type_name::TypeName},
        nexus::workflow::InvocationPolicy,
        transactions::{dag::OnchainToolArgument, invocation::InvocationPolicyCall},
    },
};

enum CustomArgument {
    Object {
        object_id: sui::types::Address,
        mutable: bool,
    },
    ObjectId(sui::types::Address),
    Pure(Vec<u8>),
}

fn parse_custom_argument(value: &str) -> AnyResult<CustomArgument> {
    let (kind, value) = value.split_once(':').ok_or_else(|| {
        anyhow!(
            "Policy argument '{value}' must use object:<ID>, mutable:<ID>, id:<ID>, or pure:<BCS_HEX>"
        )
    })?;
    match kind {
        "object" | "mutable" => Ok(CustomArgument::Object {
            object_id: value
                .parse()
                .map_err(|error| anyhow!("Invalid policy object ID '{value}': {error}"))?,
            mutable: kind == "mutable",
        }),
        "id" => Ok(CustomArgument::ObjectId(value.parse().map_err(
            |error| anyhow!("Invalid Move object ID '{value}': {error}"),
        )?)),
        "pure" => Ok(CustomArgument::Pure(hex::decode(value).map_err(
            |error| anyhow!("Invalid pure BCS hex '{value}': {error}"),
        )?)),
        _ => Err(anyhow!(
            "Unknown policy argument kind '{kind}'. Use object, mutable, id, or pure"
        )),
    }
}

async fn resolve_custom_argument(
    client: &nexus_sdk::nexus::client::NexusClient,
    argument: CustomArgument,
) -> AnyResult<OnchainToolArgument> {
    match argument {
        CustomArgument::Object { object_id, mutable } => {
            let object = client.crawler().get_object_metadata(object_id).await?;
            match object.owner {
                sui::types::Owner::Shared(initial_shared_version) => {
                    Ok(OnchainToolArgument::SharedObject {
                        object_id,
                        initial_shared_version,
                        mutable,
                    })
                }
                sui::types::Owner::ConsensusAddress { start_version, .. } => {
                    Ok(OnchainToolArgument::SharedObject {
                        object_id,
                        initial_shared_version: start_version,
                        mutable,
                    })
                }
                _ => Ok(OnchainToolArgument::Object(object.object_ref())),
            }
        }
        CustomArgument::ObjectId(object_id) => Ok(OnchainToolArgument::ObjectId(object_id)),
        CustomArgument::Pure(bytes) => Ok(OnchainToolArgument::Pure(bytes)),
    }
}

pub(super) async fn run(
    execution_id: sui::types::Address,
    vertex_name: String,
    iterator: Option<(u64, u64)>,
    policy: InvocationPolicyCommand,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    let vertex = match iterator {
        Some((iteration, out_of)) if iteration < out_of => {
            RuntimeVertex::with_iterator(&vertex_name, iteration, out_of)
        }
        Some((iteration, out_of)) => {
            return Err(NexusCliError::Any(anyhow!(
                "Iterator position '{iteration}' must be smaller than item count '{out_of}'"
            )));
        }
        None => RuntimeVertex::plain(&vertex_name),
    };
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let policy = match policy {
        InvocationPolicyCommand::FixedPrice => InvocationPolicy::FixedPrice,
        InvocationPolicyCommand::Free => InvocationPolicy::Free,
        InvocationPolicyCommand::FiniteCredits { credits_id } => {
            InvocationPolicy::FiniteCredits { credits_id }
        }
        InvocationPolicyCommand::TimePass { pass_id } => InvocationPolicy::TimePass { pass_id },
        InvocationPolicyCommand::Custom { policy, arguments } => {
            let mut resolved = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let argument = parse_custom_argument(&argument).map_err(NexusCliError::Any)?;
                resolved.push(
                    resolve_custom_argument(&client, argument)
                        .await
                        .map_err(NexusCliError::Any)?,
                );
            }
            InvocationPolicy::Custom(InvocationPolicyCall::new(TypeName::new(&policy), resolved))
        }
    };

    command_title!("Authorizing Invocation '{execution_id}::{vertex}'");
    let progress = loading!("Submitting Invocation policy transaction...");
    let result = match client
        .workflow()
        .authorize_invocation(execution_id, vertex, policy)
        .await
    {
        Ok(result) => {
            progress.success();
            result
        }
        Err(error) => {
            progress.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    notify_success!(
        "Invocation locked: {id}",
        id = result
            .lock
            .invocation
            .bytes
            .to_string()
            .truecolor(100, 100, 100)
    );
    json_output(&json!({
        "action": "authorize_invocation",
        "digest": result.tx_digest,
        "checkpoint": result.tx_checkpoint,
        "execution_id": result.lock.execution.bytes,
        "vertex": result.lock.vertex.to_string(),
        "tool_id": result.lock.tool.bytes,
        "tool_fqn": result.lock.tool_fqn.as_str(),
        "cashier_id": result.lock.cashier.bytes,
        "invocation_id": result.lock.invocation.bytes,
        "beneficiary": result.lock.beneficiary,
        "policy": result.lock.policy.as_str(),
        "sources": result
            .lock
            .sources
            .iter()
            .map(|source| source.bytes)
            .collect::<Vec<_>>(),
        "amount_mist": result.lock.amount,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_arguments_preserve_explicit_kinds() {
        assert!(matches!(
            parse_custom_argument("mutable:0x7").unwrap(),
            CustomArgument::Object { mutable: true, .. }
        ));
        assert!(matches!(
            parse_custom_argument("id:0x8").unwrap(),
            CustomArgument::ObjectId(_)
        ));
        assert!(matches!(
            parse_custom_argument("pure:0102").unwrap(),
            CustomArgument::Pure(bytes) if bytes == [1, 2]
        ));
    }
}
