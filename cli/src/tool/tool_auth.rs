use {
    super::ToolAuthCommand,
    crate::{
        command_title,
        display::{human_output, json_output},
        loading,
        notify_success,
        prelude::*,
        sui::{get_nexus_client, get_read_only_nexus_client},
    },
    nexus_sdk::{
        nexus::{client::NexusClient, network_auth::ToolKeyList},
        signed_http::{
            keys::{parse_ed25519_signing_key, Ed25519Keypair},
            v2::wire::AllowedLeadersFileV1,
        },
        ToolFqn,
    },
    std::{
        fmt::Write as _,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
};

fn render_generated_key(private_key_hex: &str, public_key_hex: &str) -> String {
    format!(
        "{:<20}{}\n{:<20}{}\n",
        "Private key", private_key_hex, "Public key", public_key_hex,
    )
}

fn render_tool_keys(tool_fqn: &ToolFqn, list: Option<&ToolKeyList>) -> String {
    let mut output = String::new();
    writeln!(output, "{:<20}{tool_fqn}", "Tool").expect("writing to a String cannot fail");
    let Some(list) = list else {
        output.push_str("No message signing keys are registered.\n");
        return output;
    };

    writeln!(output, "{:<20}{}", "Binding", list.binding_object_id)
        .expect("writing to a String cannot fail");
    writeln!(
        output,
        "{:<20}{}",
        "Active key",
        list.active_key_id
            .map_or_else(|| "none".to_owned(), |key| key.to_string()),
    )
    .expect("writing to a String cannot fail");
    writeln!(output, "{:<20}{}", "Next key", list.next_key_id)
        .expect("writing to a String cannot fail");

    for key in &list.keys {
        let state = match (list.active_key_id == Some(key.kid), key.revoked) {
            (true, true) => "active, revoked",
            (true, false) => "active",
            (false, true) => "revoked",
            (false, false) => "available",
        };
        writeln!(output, "\nKey {} ({state})", key.kid).expect("writing to a String cannot fail");
        writeln!(output, "{:<20}{}", "Public key", key.public_key_hex)
            .expect("writing to a String cannot fail");
        writeln!(output, "{:<20}{}", "Added at", key.added_at_ms)
            .expect("writing to a String cannot fail");
    }
    output
}

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
    } else {
        human_output(&render_generated_key(
            &keypair.private_key_hex(),
            &keypair.public_key_hex(),
        ));
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

    notify_success!(
        "Registered key {kid} for Tool '{tool_fqn}' with public key {public_key}.",
        kid = result.tool_kid,
        public_key = hex::encode(result.public_key),
    );

    json_output(&json!({
        "digest": result.tx_digest,
        "tool_fqn": tool_fqn,
        "tool_id": result.tool_id,
        "binding_object_id": result.binding_object_id,
        "tool_kid": result.tool_kid,
        "public_key_hex": hex::encode(result.public_key),
    }))?;

    Ok(())
}

/// Query and display all registered message-signing keys for the given tool FQN.
async fn list_keys(tool_fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Listing keys for tool '{tool_fqn}'");

    let nexus_client = get_read_only_nexus_client().await?;

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

    human_output(&render_tool_keys(&tool_fqn, list.as_ref()));

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

    let nexus_client = get_read_only_nexus_client().await?;

    let handle = loading!("Resolving leader keys and writing allowlist...");
    let file = if all {
        nexus_client
            .network_auth()
            .export_allowed_leaders_file_v1_for_all_leaders()
            .await
            .map_err(NexusCliError::Nexus)?
    } else {
        if leaders.is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "at least one --leader (leader capability ID) is required (or use --all)"
            )));
        }

        nexus_client
            .network_auth()
            .export_allowed_leaders_file_v1(&leaders)
            .await
            .map_err(NexusCliError::Nexus)?
    };
    write_allowed_leaders_file_v1(&file, &out)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AllowedLeadersSyncOutcome {
    Unchanged,
    Updated,
}

#[derive(Clone)]
struct AllowedLeadersFileSyncerV1 {
    client: NexusClient,
    out_path: PathBuf,
}

impl AllowedLeadersFileSyncerV1 {
    fn new(client: NexusClient, out_path: impl Into<PathBuf>) -> Self {
        Self {
            client,
            out_path: out_path.into(),
        }
    }

    async fn sync_once(&self) -> AnyResult<AllowedLeadersSyncOutcome, NexusCliError> {
        let file = self
            .client
            .network_auth()
            .export_allowed_leaders_file_v1_for_all_leaders()
            .await
            .map_err(NexusCliError::Nexus)?;
        let bytes = allowed_leaders_file_bytes(&file)?;

        match std::fs::read(&self.out_path) {
            Ok(existing) if existing == bytes => return Ok(AllowedLeadersSyncOutcome::Unchanged),
            Ok(_) | Err(_) => {}
        }

        atomic_write(&self.out_path, &bytes).map_err(|e| {
            NexusCliError::Any(anyhow!("failed to write {}: {e}", self.out_path.display()))
        })?;

        Ok(AllowedLeadersSyncOutcome::Updated)
    }

    async fn run_best_effort(&self, poll_interval: Duration) {
        loop {
            let _ = self.sync_once().await;
            tokio::time::sleep(poll_interval).await;
        }
    }
}

fn write_allowed_leaders_file_v1(
    file: &AllowedLeadersFileV1,
    path: &Path,
) -> AnyResult<(), NexusCliError> {
    let bytes = allowed_leaders_file_bytes(file)?;
    atomic_write(path, &bytes)
        .map_err(|e| NexusCliError::Any(anyhow!("failed to write {}: {e}", path.display())))
}

fn allowed_leaders_file_bytes(file: &AllowedLeadersFileV1) -> AnyResult<Vec<u8>, NexusCliError> {
    serde_json::to_vec_pretty(file)
        .map_err(|e| NexusCliError::Any(anyhow!("failed to serialize allowlist: {e}")))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;

    let base = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "allowed-leaders.json".to_string());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_nanos(0))
        .as_nanos();
    let pid = std::process::id();
    let tmp = parent.join(format!(".{base}.{pid}.{nanos}.tmp"));

    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)?;
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

    let client = get_read_only_nexus_client().await?;
    let syncer = AllowedLeadersFileSyncerV1::new(client, out.clone());

    if once {
        let handle = loading!("Syncing allowlist...");
        let outcome = syncer.sync_once().await?;
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
    use {
        super::*,
        clap::Parser,
        nexus_sdk::{
            nexus::network_auth::{ToolKeyEntry, ToolKeyList},
            test_utils::nexus_mocks,
        },
    };

    #[test]
    fn key_generation_report_contains_both_keys() {
        let report = render_generated_key("private", "public");

        assert!(report.contains("Private key         private"));
        assert!(report.contains("Public key          public"));
    }

    #[test]
    fn key_list_report_marks_the_active_key() {
        let tool_fqn = "xyz.demo.tool@1".parse().unwrap();
        let list = ToolKeyList {
            binding_object_id: sui::types::Address::from_static("0xb"),
            active_key_id: Some(1),
            next_key_id: 2,
            keys: vec![ToolKeyEntry {
                kid: 1,
                public_key_hex: "abcd".to_owned(),
                added_at_ms: 123,
                revoked: false,
            }],
        };

        let report = render_tool_keys(&tool_fqn, Some(&list));

        assert!(report.contains("xyz.demo.tool@1"));
        assert!(report.contains("Key 1 (active)"));
        assert!(report.contains("abcd"));
    }

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

    #[tokio::test]
    async fn sync_once_writes_allowlist_without_wallet_or_owned_coins() {
        let out_dir = tempfile::tempdir().expect("temporary output directory");
        let out_path = out_dir.path().join("allowed-leaders.json");
        let client = nexus_mocks::mock_network_auth_client_without_wallet().await;
        let syncer = AllowedLeadersFileSyncerV1::new(client, out_path.clone());

        let outcome = syncer
            .sync_once()
            .await
            .expect("read only client sync should not require wallet configuration");
        let file: AllowedLeadersFileV1 = serde_json::from_slice(
            &std::fs::read(out_path).expect("allowlist output should be written"),
        )
        .expect("allowlist output should decode");

        assert_eq!(outcome, AllowedLeadersSyncOutcome::Updated);
        assert_eq!(file.version, 1);
        assert_eq!(file.leaders.len(), 1);
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
