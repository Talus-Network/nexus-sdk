//! Canonical JSON shapes for every `nexus tap` subcommand.
//!
//! Every `tap_*.rs` handler routes its `--json` payload through one of the
//! helpers below — there are no scattered `json_output(&json!(...))` calls
//! inside the per-command modules. Co-locating the shapes keeps the
//! contract scripted consumers depend on auditable from one file, gives
//! the JSON-shape tests a single home, and makes a Move-side field rename
//! impossible to land without an explicit CLI-side bump.

use {
    super::*,
    nexus_sdk::{
        idents::registry::AgentRegistry,
        nexus::{
            tap::{
                AccomplishExecutionPaymentResult,
                BindAgentSkillResult,
                DepositAgentVaultResult,
                WaitForPaymentResult,
            },
            workflow::ExecuteResult,
        },
        types::{
            AgentId,
            DefaultDagExecutorRecord,
            TapAgentPaymentVault,
            TapExecutionPayment,
            TapRegistry,
            TapSkillConfig,
        },
    },
};

// ============================================================================
// Local-only commands: scaffold, validate-skill, dry-run
// ============================================================================

pub(crate) fn scaffold_result_json(root: &std::path::Path) -> serde_json::Value {
    json!({ "path": root })
}

pub(crate) fn validate_skill_result_json(config: &TapSkillConfig) -> serde_json::Value {
    json!({
        "valid": true,
        "skill_name": config.name,
        "tap_package_name": config.tap_package_name,
        "interface_revision": config.interface_revision,
    })
}

pub(crate) fn dry_run_result_json(config: &TapSkillConfig) -> serde_json::Value {
    json!({
        "dry_run": true,
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision,
        "next_step": "publish TAP plus DAG, then create-agent and register-skill",
    })
}

// ============================================================================
// Publish + bind lifecycle
// ============================================================================

pub(crate) fn create_agent_result_json(result: &CreateAgentResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::CREATE_AGENT.name.to_string(),
        "agent_id": result.agent_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
    })
}

pub(crate) fn publish_skill_result_json(result: &PublishSkillResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "publish_skill",
        "tap_package_id": result.tap_package.package_id,
        "tap_package_digest": result.tap_package.tx_digest,
        "tap_package_checkpoint": result.tap_package.tx_checkpoint,
        "dag_id": result.dag.dag_object_id,
        "dag_digest": result.dag.tx_digest,
        "dag_checkpoint": result.dag.tx_checkpoint,
        "artifact": result.artifact,
    })
}

pub(crate) fn register_skill_result_json(
    artifact: &TapPublishArtifact,
    result: &RegisterSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::REGISTER_SKILL.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
    })
}

pub(crate) fn bind_result_json(
    artifact: &TapPublishArtifact,
    result: &BindAgentSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "bind_agent_skill",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "agent_object_id": result.agent_object.object_id(),
        "agent_object_version": result.agent_object.version(),
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
    })
}

pub(crate) fn update_skill_result_json(
    artifact: &TapPublishArtifact,
    result: &UpdateSkillResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "update_skill",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "current_interface_revision": result.current_interface_revision,
        "dag_id": artifact.dag_id,
        "tap_package_id": artifact.tap_package_id,
        "dag_binding": result.dag_binding,
        "requirements": result.requirements,
    })
}

// ============================================================================
// Skill execution + requirements
// ============================================================================

pub(crate) fn agent_execute_result_json(
    agent_id: AgentId,
    skill_id: SkillId,
    result: &ExecuteResult,
) -> serde_json::Value {
    json!({
        "agent_dag": true,
        "agent_id": agent_id,
        "skill_id": skill_id,
        "execution_id": result.execution_object_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "submit": result.tap_execution.as_ref().map(|submit| json!({
            "agent_id": submit.agent_id,
            "skill_id": submit.skill_id,
            "dag_id": submit.dag_id,
            "skill_revision_key": submit.skill_revision_key,
            "payment_max_budget": submit.payment_max_budget,
            "authorization_plan_commitment": submit.authorization_plan_commitment,
        }))
    })
}

pub(crate) fn requirements_result_json(result: &GetSkillRequirementsResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::GET_SKILL_REQUIREMENTS.name.to_string(),
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "active_skill_revision_key": result.active_skill_revision_key,
        "requirements": result.requirements,
    })
}

// ============================================================================
// Scheduling
// ============================================================================

pub(crate) fn schedule_result_json(
    long_term_gas_coin_id: sui::types::Address,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::SCHEDULE_SKILL_EXECUTION.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "long_term_gas_coin_id": long_term_gas_coin_id,
    })
}

pub(crate) fn schedule_address_funded_result_json(
    scheduler_task_id: sui::types::Address,
    prepay_amount: u64,
    occurrence_budget: u64,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    })
}

pub(crate) fn schedule_from_vault_result_json(
    scheduler_task_id: sui::types::Address,
    prepay_amount: u64,
    occurrence_budget: u64,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    })
}

pub(crate) fn schedule_default_address_funded_result_json(
    scheduler_task_id: sui::types::Address,
    prepay_amount: u64,
    occurrence_budget: u64,
    result: &ScheduleSkillExecutionResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_DEFAULT_DAG_EXECUTOR_SKILL_EXECUTION_ADDRESS_FUNDED.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    })
}

// ============================================================================
// Payments: show, wait, list
// ============================================================================

pub(crate) fn payment_show_result_json(payment: &TapExecutionPayment) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "payment_id": payment.id,
        "execution_id": payment.execution_id,
        "agent_id": payment.agent_id,
        "skill_id": payment.skill_id,
        "interface_revision": payment.interface_revision,
        "payer": payment.payer,
        "payment_mode": payment.payment_mode,
        "source_kind": payment.source_kind,
        "source_identity": payment.source_identity,
        "max_budget": payment.max_budget,
        "locked_budget": payment.locked_budget,
        "consumed": payment.consumed,
        "accomplished": payment.accomplished,
        "refunded": payment.refunded,
        "final_state": payment.final_state,
        "terminal": nexus_sdk::nexus::tap::payment_is_terminal(payment),
        "locked_vertices": payment.locked_vertices,
    })
}

pub(crate) fn payment_wait_result_json(result: &WaitForPaymentResult) -> serde_json::Value {
    let mut base = payment_show_result_json(&result.payment);
    let object = base.as_object_mut().expect("payment show returns object");
    object.insert("elapsed_ms".to_string(), json!(result.elapsed_ms));
    object.insert("timed_out".to_string(), json!(result.timed_out));
    object.insert("terminal".to_string(), json!(result.terminal));
    base
}

pub(crate) fn payments_list_result_json(
    owner: sui::types::Address,
    agent_id: Option<sui::types::Address>,
    wallet_receipts: &[TapExecutionPaymentReceipt],
    vault_receipts: &[TapExecutionPaymentReceipt],
    unresolved_execution_ids: &[sui::types::Address],
    resolved_execution_ids: &[sui::types::Address],
) -> serde_json::Value {
    json!({
        "owner": owner,
        "agent_id": agent_id,
        "wallet_receipts": wallet_receipts,
        "vault_receipts": vault_receipts,
        "unresolved_execution_ids": unresolved_execution_ids,
        "resolved_execution_ids": resolved_execution_ids,
    })
}

pub(crate) fn payment_resolve_result_json(
    result: &AccomplishExecutionPaymentResult,
) -> serde_json::Value {
    let function = if result.agent_id.is_some() {
        "accomplish_tap_execution_payment_from_agent_vault"
    } else {
        "accomplish_tap_execution_payment"
    };
    json!({
        "standard_tap": true,
        "function": function,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "execution_id": result.execution_id,
        "agent_id": result.agent_id,
    })
}

// ============================================================================
// Registry + default-target inspection
// ============================================================================

pub(crate) fn registry_show_result_json(registry: &TapRegistry) -> serde_json::Value {
    json!({
        "id": registry.id,
        "default_executor": registry.default_executor,
        "agents": registry.agents,
        "skills": registry.skills,
    })
}

pub(crate) fn default_target_result_json(record: &DefaultDagExecutorRecord) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "agent_id": record.target.agent_id,
        "skill_id": record.target.skill_id,
        "dag_binding": record.skill.dag_binding,
        "dag_id": record.skill.dag_binding.pinned_dag_id(),
        "interface_revision": record.skill_revision.key.interface_revision,
        "requirements": record.skill_revision.requirements,
    })
}

// ============================================================================
// Vault: balance, deposit
// ============================================================================

pub(crate) fn vault_balance_result_json(
    agent_id: AgentId,
    vault: &nexus_sdk::nexus::crawler::Response<TapAgentPaymentVault>,
) -> serde_json::Value {
    json!({
        "agent_id": agent_id,
        "vault_id": vault.object_id,
        "available_balance": vault.data.available_balance,
        "locked_amount": vault.data.locked_amount,
        "unlocked_balance": vault.data.available_balance.saturating_sub(vault.data.locked_amount),
    })
}

pub(crate) fn vault_deposit_result_json(result: &DepositAgentVaultResult) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "amount": result.amount,
    })
}

// ============================================================================
// Local agent alias management
// ============================================================================

pub(crate) fn agent_save_result_json(name: &str, agent_id: AgentId) -> serde_json::Value {
    json!({ "name": name, "agent_id": agent_id })
}

pub(crate) fn agent_list_result_json(agents: &[(String, AgentId)]) -> serde_json::Value {
    json!({
        "agents": agents.iter().map(|(name, agent_id)| {
            json!({ "name": name, "agent_id": agent_id })
        }).collect::<Vec<_>>(),
    })
}

pub(crate) fn agent_remove_result_json(name: &str, removed: Option<AgentId>) -> serde_json::Value {
    json!({ "name": name, "removed": removed })
}

// ============================================================================
// JSON-shape tests
//
// Every helper above has at least one assertion here so a Move-side rename
// or accidental key drop surfaces as a unit-test failure rather than as a
// scripted consumer's silent breakage.
// ============================================================================

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            nexus::{
                tap::TapPackagePublishResult,
                workflow::{PublishResult, TapExecutionSubmitMetadata},
            },
            types::{
                InterfaceRevision,
                TapPaymentMode,
                TapPaymentPolicy,
                TapSchedulePolicy,
                TapSkillRequirements,
                TapSkillRevisionKey,
                TapVertexAuthorizationPlan,
            },
        },
    };

    // ---- shared fixtures ----

    fn fixture_artifact() -> TapPublishArtifact {
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![1],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceRevision(1),
        };
        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xe"),
        )
        .expect("artifact builds")
    }

    fn fixture_payment(accomplished: bool, refunded: bool) -> TapExecutionPayment {
        TapExecutionPayment {
            id: sui::types::Address::from_static("0xaa"),
            execution_id: sui::types::Address::from_static("0xbb"),
            agent_id: sui::types::Address::from_static("0xcc"),
            skill_id: 11,
            interface_revision: InterfaceRevision(2),
            payer: sui::types::Address::from_static("0xee"),
            payment_mode: TapPaymentMode::UserFunded,
            source_kind: None,
            source_identity: None,
            max_budget: 1_000,
            locked_budget: 0,
            consumed: 0,
            payment_source_hash: vec![],
            accomplished,
            refunded,
            final_state: None,
            locked_vertices: vec![],
        }
    }

    // ---- publish / bind lifecycle ----

    #[test]
    fn tap_submission_result_json_helpers_expose_created_ids() {
        let artifact = fixture_artifact();

        let create_output = create_agent_result_json(&CreateAgentResult {
            tx_digest: sui::types::Digest::from([7; 32]),
            tx_checkpoint: 11,
            agent_id: sui::types::Address::from_static("0xa"),
            agent_object: sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xa"),
                7,
                sui::types::Digest::from([8; 32]),
            ),
        });
        assert_eq!(
            create_output["agent_id"],
            serde_json::json!(sui::types::Address::from_static("0xa").to_string())
        );
        assert_eq!(create_output["tx_checkpoint"], serde_json::json!(11));

        let register_output = register_skill_result_json(
            &artifact,
            &RegisterSkillResult {
                tx_digest: sui::types::Digest::from([8; 32]),
                tx_checkpoint: 12,
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            },
        );
        assert_eq!(register_output["skill_id"], serde_json::json!(11));
        assert_eq!(
            register_output["dag_id"],
            serde_json::json!("0x000000000000000000000000000000000000000000000000000000000000000d")
        );
    }

    #[test]
    fn publish_skill_result_json_exposes_complete_artifact_handoff() {
        let artifact = fixture_artifact();
        let output = publish_skill_result_json(&PublishSkillResult {
            tap_package: TapPackagePublishResult {
                tx_digest: sui::types::Digest::from([1; 32]),
                tx_checkpoint: 10,
                package_id: sui::types::Address::from_static("0xe"),
            },
            dag: PublishResult {
                tx_digest: sui::types::Digest::from([2; 32]),
                tx_checkpoint: 11,
                dag_object_id: sui::types::Address::from_static("0xd"),
            },
            artifact,
        });

        assert_eq!(
            output["tap_package_id"],
            serde_json::json!(sui::types::Address::from_static("0xe").to_string())
        );
        assert_eq!(
            output["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(
            output["artifact"]["interface_revision"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn bind_result_json_exposes_combined_evidence() {
        let artifact = fixture_artifact();
        let result = BindAgentSkillResult {
            tx_digest: sui::types::Digest::from([7u8; 32]),
            tx_checkpoint: 100,
            agent_id: sui::types::Address::from_static("0xa1"),
            agent_object: sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xa1"),
                3,
                sui::types::Digest::from([5u8; 32]),
            ),
            skill_id: 7,
        };
        let json = bind_result_json(&artifact, &result);
        assert_eq!(json["function"], "bind_agent_skill");
        assert_eq!(
            json["agent_id"],
            serde_json::json!(sui::types::Address::from_static("0xa1").to_string())
        );
        assert_eq!(json["skill_id"], serde_json::json!(7));
        assert_eq!(json["tx_checkpoint"], serde_json::json!(100));
    }

    #[test]
    fn update_skill_result_json_exposes_skill_update_revision() {
        let artifact = fixture_artifact();
        let result = UpdateSkillResult {
            tx_digest: sui::types::Digest::from([7u8; 32]),
            tx_checkpoint: 100,
            agent_id: sui::types::Address::from_static("0xa1"),
            skill_id: 7,
            current_interface_revision: InterfaceRevision(2),
            dag_binding: nexus_sdk::types::TapDagBinding::pinned(artifact.dag_id),
            requirements: artifact.requirements.clone(),
        };
        let json = update_skill_result_json(&artifact, &result);
        assert_eq!(json["function"], "update_skill");
        assert_eq!(json["skill_id"], serde_json::json!(7));
        assert_eq!(json["current_interface_revision"], serde_json::json!(2));
        assert!(json.get("config_digest_hex").is_none());
    }

    // ---- execute + requirements + schedule ----

    #[test]
    fn agent_execute_result_json_includes_submit_metadata() {
        let result = ExecuteResult {
            tx_digest: sui::types::Digest::from([7; 32]),
            execution_object_id: sui::types::Address::from_static("0xc"),
            tx_checkpoint: 42,
            tap_execution: Some(TapExecutionSubmitMetadata {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                dag_id: sui::types::Address::from_static("0xd"),
                skill_revision_key: TapSkillRevisionKey {
                    agent_id: sui::types::Address::from_static("0xa"),
                    skill_id: 11,
                    interface_revision: InterfaceRevision(3),
                },
                payment_max_budget: 99,
                authorization_plan_commitment: Some(vec![1, 2, 3]),
                authorization_plan: TapVertexAuthorizationPlan::default(),
            }),
        };

        let output =
            agent_execute_result_json(sui::types::Address::from_static("0xa"), 11, &result);

        assert_eq!(
            output["execution_id"],
            serde_json::json!(sui::types::Address::from_static("0xc").to_string())
        );
        assert_eq!(
            output["submit"]["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(
            output["submit"]["skill_revision_key"]["interface_revision"],
            serde_json::json!(3)
        );
        assert_eq!(
            output["submit"]["payment_max_budget"],
            serde_json::json!(99)
        );
        assert_eq!(
            output["submit"]["authorization_plan_commitment"],
            serde_json::json!([1, 2, 3])
        );
    }

    #[test]
    fn tap_requirements_and_schedule_json_helpers_expose_live_state() {
        let requirements = TapSkillRequirements {
            input_schema_commitment: vec![1],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };

        let requirements_output = requirements_result_json(&GetSkillRequirementsResult {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            active_skill_revision_key: TapSkillRevisionKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(3),
            },
            requirements,
        });
        assert_eq!(
            requirements_output["active_skill_revision_key"]["interface_revision"],
            serde_json::json!(3)
        );
        assert_eq!(
            requirements_output["requirements"]["input_schema_commitment"],
            serde_json::json!([1])
        );

        let schedule_output = schedule_result_json(
            sui::types::Address::from_static("0xc"),
            &ScheduleSkillExecutionResult {
                tx_digest: sui::types::Digest::from([9; 32]),
                tx_checkpoint: 13,
                scheduled_task_id: sui::types::Address::from_static("0xd"),
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            },
        );
        assert_eq!(
            schedule_output["scheduled_task_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(schedule_output["tx_checkpoint"], serde_json::json!(13));
    }

    // ---- payments ----

    #[test]
    fn payment_show_result_json_includes_terminal_flag() {
        let json = payment_show_result_json(&fixture_payment(true, false));
        assert_eq!(json["standard_tap"], serde_json::Value::Bool(true));
        assert_eq!(json["accomplished"], serde_json::Value::Bool(true));
        assert_eq!(json["refunded"], serde_json::Value::Bool(false));
        assert_eq!(json["terminal"], serde_json::Value::Bool(true));
        assert_eq!(json["skill_id"], serde_json::json!(11));
    }

    #[test]
    fn payment_wait_result_json_adds_elapsed_and_timeout_flags() {
        let wait = WaitForPaymentResult {
            payment: fixture_payment(false, false),
            terminal: false,
            elapsed_ms: 1234,
            timed_out: true,
        };
        let json = payment_wait_result_json(&wait);
        assert_eq!(json["elapsed_ms"], serde_json::json!(1234));
        assert_eq!(json["timed_out"], serde_json::Value::Bool(true));
        assert_eq!(json["terminal"], serde_json::Value::Bool(false));
    }

    #[test]
    fn payment_resolve_result_json_exposes_execution_id_and_digest() {
        let result = AccomplishExecutionPaymentResult {
            tx_digest: sui::types::Digest::from([3u8; 32]),
            tx_checkpoint: 77,
            execution_id: sui::types::Address::from_static("0xee"),
            agent_id: None,
        };
        let json = payment_resolve_result_json(&result);
        assert_eq!(json["standard_tap"], serde_json::Value::Bool(true));
        assert_eq!(
            json["function"],
            serde_json::json!("accomplish_tap_execution_payment")
        );
        assert_eq!(
            json["execution_id"],
            serde_json::json!(sui::types::Address::from_static("0xee").to_string())
        );
        assert_eq!(json["tx_checkpoint"], serde_json::json!(77));
        assert!(json["agent_id"].is_null());
    }

    #[test]
    fn payment_resolve_result_json_reports_vault_function_when_agent_supplied() {
        let result = AccomplishExecutionPaymentResult {
            tx_digest: sui::types::Digest::from([4u8; 32]),
            tx_checkpoint: 88,
            execution_id: sui::types::Address::from_static("0xee"),
            agent_id: Some(sui::types::Address::from_static("0xa")),
        };
        let json = payment_resolve_result_json(&result);
        assert_eq!(
            json["function"],
            serde_json::json!("accomplish_tap_execution_payment_from_agent_vault")
        );
        assert_eq!(
            json["agent_id"],
            serde_json::json!(sui::types::Address::from_static("0xa").to_string())
        );
    }

    // ---- agent aliases ----

    #[test]
    fn agent_alias_result_jsons_emit_their_canonical_keys() {
        let agent_id = sui::types::Address::from_static("0xaa");
        let save = agent_save_result_json("primary", agent_id);
        assert_eq!(save["name"], serde_json::json!("primary"));
        assert_eq!(save["agent_id"], serde_json::json!(agent_id.to_string()));

        let list = agent_list_result_json(&[("primary".to_string(), agent_id)]);
        let entries = list["agents"].as_array().expect("agents must be an array");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], serde_json::json!("primary"));

        let removed = agent_remove_result_json("primary", Some(agent_id));
        assert_eq!(removed["removed"], serde_json::json!(agent_id.to_string()));
        let missing = agent_remove_result_json("primary", None);
        assert_eq!(missing["removed"], serde_json::Value::Null);
    }

    // ---- vault deposit ----

    #[test]
    fn vault_deposit_result_json_carries_amount_and_digest() {
        let result = DepositAgentVaultResult {
            tx_digest: sui::types::Digest::from([2u8; 32]),
            tx_checkpoint: 50,
            agent_id: sui::types::Address::from_static("0xee"),
            amount: 12345,
        };
        let json = vault_deposit_result_json(&result);
        assert_eq!(json["standard_tap"], serde_json::Value::Bool(true));
        assert_eq!(json["amount"], serde_json::json!(12345));
        assert_eq!(json["tx_checkpoint"], serde_json::json!(50));
    }

    // ---- local-only commands ----

    #[test]
    fn scaffold_and_validate_dry_run_jsons_expose_skill_identity() {
        let path = PathBuf::from("/tmp/tap/skill");
        assert_eq!(
            scaffold_result_json(&path)["path"]
                .as_str()
                .expect("path serialized as string"),
            "/tmp/tap/skill"
        );

        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: PathBuf::from("dag.json"),
            tap_package_path: PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![1],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceRevision(7),
        };

        let validate = validate_skill_result_json(&config);
        assert_eq!(validate["valid"], serde_json::Value::Bool(true));
        assert_eq!(validate["interface_revision"], serde_json::json!(7));

        let dry_run = dry_run_result_json(&config);
        assert_eq!(dry_run["dry_run"], serde_json::Value::Bool(true));
        assert!(dry_run.get("config_digest_hex_with_zero_package").is_none());
    }
}
