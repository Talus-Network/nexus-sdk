use {
    super::ToolAuthCommand,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::{build_sui_grpc_client, get_nexus_client, get_nexus_objects},
    },
    nexus_sdk::{
        nexus::network_auth::{
            AllowedLeadersFileSyncerV1,
            AllowedLeadersSyncOutcome,
            NetworkAuthReader,
        },
        signed_http::keys::{parse_ed25519_signing_key, Ed25519Keypair},
        ToolFqn,
    },
    std::{path::PathBuf, time::Duration},
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
        ToolAuthCommand::ExportAllowedLeaders { all, leaders, out } => {
            export_allowed_leaders(all, leaders, out).await
        }
        ToolAuthCommand::SyncAllowedLeaders {
            out,
            interval,
            once,
        } => sync_allowed_leaders(out, interval, once).await,
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
    all: bool,
    leaders: Vec<sui::types::Address>,
    out: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!("Exporting allowed leaders file");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let handle = loading!("Resolving leader keys and writing allowlist...");
    if all {
        nexus_client
            .network_auth()
            .write_allowed_leaders_file_v1_for_all_leaders(&out)
            .await
            .map_err(NexusCliError::Nexus)?;
    } else {
        if leaders.is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "at least one --leader (leader capability ID) is required (or use --all)"
            )));
        }

        nexus_client
            .network_auth()
            .write_allowed_leaders_file_v1(&leaders, &out)
            .await
            .map_err(NexusCliError::Nexus)?;
    }
    handle.success();

    notify_success!(
        "Wrote allowlist JSON to {path}",
        path = out.display().to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({
        "out": out,
        "all": all,
        "leaders": leaders,
    }))?;

    Ok(())
}

async fn sync_allowed_leaders(
    out: PathBuf,
    interval: Duration,
    once: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Syncing allowed leaders file");

    if interval.is_zero() {
        return Err(NexusCliError::Any(anyhow!(
            "invalid duration (must be > 0)"
        )));
    }

    let mut conf = CliConf::load().await.unwrap_or_default();
    let client = build_sui_grpc_client(&conf).await?;
    let rpc_url = client.lock().await.uri().to_string();
    let objects = get_nexus_objects(&mut conf).await?;

    let reader = NetworkAuthReader::from_rpc_url(
        &rpc_url,
        objects.workflow_pkg_id,
        *objects.network_auth.object_id(),
    )
    .map_err(NexusCliError::Nexus)?;

    let syncer = AllowedLeadersFileSyncerV1::new(reader, out.clone());

    if once {
        let handle = loading!("Syncing allowlist...");
        let outcome = syncer.sync_once().await.map_err(NexusCliError::Nexus)?;
        handle.success();

        notify_success!(
            "Wrote allowlist JSON to {path} ({outcome})",
            path = out.display().to_string().truecolor(100, 100, 100),
            outcome = match outcome {
                AllowedLeadersSyncOutcome::Updated => "updated",
                AllowedLeadersSyncOutcome::Unchanged => "unchanged",
            }
        );

        json_output(&json!({
            "out": out,
            "once": true,
            "interval_ms": u64::try_from(interval.as_millis()).unwrap_or(u64::MAX),
            "outcome": match outcome {
                AllowedLeadersSyncOutcome::Updated => "updated",
                AllowedLeadersSyncOutcome::Unchanged => "unchanged",
            }
        }))?;

        return Ok(());
    }

    notify_success!(
        "Syncing allowlist to {path} every {interval:?} (Ctrl-C to stop)",
        path = out.display().to_string().truecolor(100, 100, 100),
        interval = interval
    );

    json_output(&json!({
        "out": out,
        "once": false,
        "interval_ms": u64::try_from(interval.as_millis()).unwrap_or(u64::MAX),
    }))?;

    tokio::select! {
        _ = syncer.run_best_effort(interval) => Ok(()),
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn clap_parses_sync_allowed_leaders_interval() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "auth",
            "sync-allowed-leaders",
            "--out",
            "/tmp/allowed-leaders.json",
            "--interval",
            "500ms",
            "--once",
        ])
        .unwrap();

        match cli.command {
            crate::Command::Tool(crate::tool::ToolCommand::Auth { cmd }) => match cmd {
                ToolAuthCommand::SyncAllowedLeaders {
                    out,
                    interval,
                    once,
                } => {
                    assert_eq!(out, PathBuf::from("/tmp/allowed-leaders.json"));
                    assert_eq!(interval, Duration::from_millis(500));
                    assert!(once);
                }
                _ => panic!("unexpected tool auth command"),
            },
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn clap_rejects_invalid_sync_allowed_leaders_interval() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "auth",
            "sync-allowed-leaders",
            "--out",
            "/tmp/allowed-leaders.json",
            "--interval",
            "not-a-duration",
            "--once",
        ])
        .is_err());
    }

    #[tokio::test]
    async fn sync_allowed_leaders_rejects_zero_interval() {
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("allowed-leaders.json");

        let err = sync_allowed_leaders(out_path, Duration::from_secs(0), true)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid duration"));
    }
}
