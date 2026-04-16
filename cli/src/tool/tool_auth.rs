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
            skip_if_active,
            gas,
        } => {
            register_key(
                tool_fqn,
                owner_cap,
                signing_key,
                description,
                skip_if_active,
                gas,
            )
            .await
        }
        ToolAuthCommand::ListKeys { tool_fqn } => list_keys(tool_fqn).await,
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
    skip_if_active: bool,
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

    // When --skip-if-active is requested, check whether the same public key is
    // already the active key. If it is, skip the on-chain transaction entirely
    // so the command is safe to run repeatedly in CI without accumulating
    // duplicate key registrations.
    //
    // Note: this check is best-effort. There is a TOCTOU (Time-Of-Check to
    // Time-Of-Use) gap between this read and the subsequent registration call.
    // Concurrent invocations may both see "key not active" and both proceed,
    // resulting in the same public key registered twice with different key IDs.
    // This is non-fatal (no on-chain corruption, just an extra key entry) and
    // acceptable for a CI convenience flag targeting single-pipeline usage.
    if skip_if_active {
        let new_pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let check_handle = loading!("Checking current active key...");

        match nexus_client
            .network_auth()
            .list_tool_keys(&tool_fqn)
            .await
            .map_err(NexusCliError::Nexus)?
        {
            Some(list) => {
                if let Some(active_kid) = list.active_key_id {
                    if let Some(active) = list.keys.iter().find(|k| k.kid == active_kid) {
                        if active.public_key_hex == new_pubkey_hex {
                            check_handle.success();

                            notify_success!(
                                "Key is already the active key (kid={kid}), skipping registration.",
                                kid = active_kid
                            );

                            return json_output(&json!({
                                "tool_fqn": tool_fqn,
                                "skipped": true,
                                "reason": "key already active",
                                "active_kid": active_kid,
                                "public_key_hex": new_pubkey_hex,
                            }));
                        }
                    }
                }

                check_handle.success();
            }
            None => {
                // No binding yet — proceed with first-time registration.
                check_handle.success();
            }
        }
    }
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

/// Query and display all registered message-signing keys for the given tool FQN.
async fn list_keys(tool_fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Listing keys for tool '{tool_fqn}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let handle = loading!("Fetching key binding...");
    let list = match nexus_client.network_auth().list_tool_keys(&tool_fqn).await {
        Ok(list) => {
            handle.success();
            list
        }
        Err(e) => {
            handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };

    match list {
        None => {
            json_output(&json!({
                "tool_fqn": tool_fqn,
                "binding_object_id": null,
                "active_key_id": null,
                "next_key_id": null,
                "keys": [],
            }))?;
        }
        Some(list) => {
            json_output(&json!({
                "tool_fqn": tool_fqn,
                "binding_object_id": list.binding_object_id,
                "active_key_id": list.active_key_id,
                "next_key_id": list.next_key_id,
                "keys": list.keys.iter().map(|k| json!({
                    "kid": k.kid,
                    "public_key_hex": k.public_key_hex,
                    "added_at_ms": k.added_at_ms,
                    "revoked": k.revoked,
                })).collect::<Vec<_>>(),
            }))?;
        }
    }

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

    /// Verifies that clap correctly parses the humantime duration format for
    /// `sync-allowed-leaders --interval`. Guards against regressions where the
    /// custom value_parser is accidentally removed or replaced.
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

    /// Verifies that clap rejects non-duration strings for `--interval`.
    /// Guards against the custom value_parser being silently bypassed.
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

    /// Verifies that a zero-length interval is rejected at runtime (not just at parse time).
    /// Guards against the `interval.is_zero()` guard being removed.
    #[tokio::test]
    async fn sync_allowed_leaders_rejects_zero_interval() {
        let out_dir = tempfile::tempdir().unwrap();
        let out_path = out_dir.path().join("allowed-leaders.json");

        let err = sync_allowed_leaders(out_path, Duration::from_secs(0), true)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid duration"));
    }

    /// Verifies that `register-key --skip-if-active` is accepted by clap.
    /// Guards against the flag being accidentally dropped from the command definition.
    #[test]
    fn clap_parses_register_key_skip_if_active_flag() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "auth",
            "register-key",
            "--tool-fqn",
            "xyz.demo.tool@1",
            "--signing-key",
            "deadbeef",
            "--skip-if-active",
        ])
        .is_ok());
    }

    /// Verifies that `list-keys` is accepted by clap with a valid FQN.
    /// Guards against the subcommand being accidentally removed.
    #[test]
    fn clap_parses_list_keys() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "auth",
            "list-keys",
            "--tool-fqn",
            "xyz.demo.tool@1",
        ])
        .unwrap();

        match cli.command {
            crate::Command::Tool(crate::tool::ToolCommand::Auth { cmd }) => {
                assert!(matches!(cmd, ToolAuthCommand::ListKeys { .. }));
            }
            _ => panic!("unexpected command"),
        }
    }
}
