use {
    super::ToolAuthCommand,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::{
        signed_http::keys::{parse_ed25519_signing_key, Ed25519Keypair},
        ToolFqn,
    },
    std::path::PathBuf,
};

pub(crate) async fn handle_tool_auth(cmd: ToolAuthCommand) -> AnyResult<(), NexusCliError> {
    match cmd {
        ToolAuthCommand::Keygen { out } => keygen(out).await,
        ToolAuthCommand::RegisterKey {
            tool_fqn,
            owner_cap,
            signing_key,
            description,
            gas,
        } => register_key(tool_fqn, owner_cap, signing_key, description, gas).await,
        ToolAuthCommand::ExportAllowedLeaders { leaders, out } => {
            export_allowed_leaders(leaders, out).await
        }
    }
}

async fn keygen(out: Option<PathBuf>) -> AnyResult<(), NexusCliError> {
    command_title!("Generating tool message-signing key");

    let keypair = Ed25519Keypair::generate();
    let payload = json!({
        "private_key_hex": keypair.private_key_hex(),
        "public_key_hex": keypair.public_key_hex(),
    });

    if let Some(path) = out {
        std::fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap())
            .map_err(|e| NexusCliError::Any(anyhow!("failed to write {}: {e}", path.display())))?;

        notify_success!(
            "Wrote keypair JSON to {path}",
            path = path.display().to_string().truecolor(100, 100, 100)
        );
    }

    json_output(&payload)?;
    Ok(())
}

async fn register_key(
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    signing_key: String,
    description: Option<String>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Registering signed HTTP key for tool '{tool_fqn}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let Some(owner_cap) = owner_cap.or(conf.tools.get(&tool_fqn).map(|t| t.over_tool)) else {
        return Err(NexusCliError::Any(anyhow!(
            "No OwnerCap<OverTool> object ID found for tool '{tool_fqn}'."
        )));
    };

    let key_handle = loading!("Parsing signing key...");
    let signing_key_raw = signing_key.trim();
    let path = PathBuf::from(signing_key_raw);
    let signing_key_raw = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| {
            NexusCliError::Any(anyhow!(
                "failed to read signing key file {}: {e}",
                path.display()
            ))
        })?
    } else {
        signing_key_raw.to_string()
    };

    let signing_key = parse_ed25519_signing_key(&signing_key_raw).map_err(|e| {
        NexusCliError::Any(anyhow!(
            "invalid signing key (expected hex/base64/base64url or a file containing it): {e}"
        ))
    })?;
    key_handle.success();

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let description_bytes = description.map(|s| s.into_bytes());

    let handle = loading!("Submitting network_auth transaction...");
    let result = nexus_client
        .network_auth()
        .register_tool_message_key(tool_fqn.clone(), owner_cap, signing_key, description_bytes)
        .await
        .map_err(NexusCliError::Nexus)?;
    handle.success();

    json_output(&json!({
        "digest": result.tx_digest,
        "tool_fqn": tool_fqn,
        "binding_object_id": result.binding_object_id,
        "tool_kid": result.tool_kid,
        "public_key_hex": hex::encode(result.public_key),
    }))?;

    Ok(())
}

async fn export_allowed_leaders(
    leaders: Vec<sui::types::Address>,
    out: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Exporting allowed leaders file");
    if leaders.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "at least one --leader is required"
        )));
    }

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let handle = loading!("Resolving leader keys and writing allowlist...");
    nexus_client
        .network_auth()
        .write_allowed_leaders_file_v1(&leaders, &out)
        .await
        .map_err(NexusCliError::Nexus)?;
    handle.success();

    notify_success!(
        "Wrote allowlist JSON to {path}",
        path = out.display().to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({
        "out": out,
        "leaders": leaders,
    }))?;

    Ok(())
}
