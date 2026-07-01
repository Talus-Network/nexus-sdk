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
        nexus::{
            scheduler::CreateTaskResult,
            tap::{
                AccomplishExecutionPaymentResult,
                BindAgentSkillResult,
                DepositAgentVaultResult,
                RefillExecutionPaymentResult,
                WaitForPaymentResult,
            },
            workflow::{AbortExecutionResult, CommittedToolResultSettlementResult, ExecuteResult},
        },
        types::{
            interface::{agent::AgentPaymentVault, payment::ExecutionPayment},
            AgentId,
            AgentRegistrySnapshot,
            DefaultDagExecutorRecord,
            SkillConfig,
        },
    },
};

// ============================================================================
// Local-only commands: scaffold, validate-skill, dry-run
// ============================================================================

pub(crate) fn scaffold_result_json(root: &std::path::Path) -> serde_json::Value {
    json!({ "path": root })
}

pub(crate) fn validate_skill_result_json(config: &SkillConfig) -> serde_json::Value {
    json!({
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision,
    })
}

pub(crate) fn dry_run_result_json(config: &SkillConfig) -> serde_json::Value {
    json!({
        "dry_run": true,
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision,
        "next_step": "publish TAP plus DAG, then create-agent and register-skill",
    })
}

pub(crate) fn create_skill_artifact_result_json(
    artifact: &TapPublishArtifact,
) -> serde_json::Value {
    json!(artifact)
}

// ============================================================================
// Publish + bind lifecycle
// ============================================================================

pub(crate) fn create_agent_result_json(result: &CreateAgentResult) -> serde_json::Value {
    json!({
        "function": nexus_sdk::idents::registry::AgentRegistry::CREATE_AGENT.name.to_string(),
        "agent_id": result.agent_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
    })
}

pub(crate) fn publish_skill_result_json(result: &PublishSkillResult) -> serde_json::Value {
    json!({
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
        "function": nexus_sdk::idents::registry::AgentRegistry::REGISTER_SKILL.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
    })
}

pub(crate) fn bind_result_json(
    artifact: &TapPublishArtifact,
    result: &BindAgentSkillResult,
) -> serde_json::Value {
    json!({
        "function": "bind_agent_skill",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "agent_object_id": result.agent_object.object_id(),
        "agent_object_version": result.agent_object.version(),
        "skill_id": result.skill_id,
        "dag_id": artifact.dag_id,
    })
}

pub(crate) fn update_skill_result_json(
    artifact: &TapPublishArtifact,
    result: &UpdateSkillResult,
) -> serde_json::Value {
    json!({
        "function": "update_skill",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "current_interface_revision": result.current_interface_revision,
        "dag_id": artifact.dag_id,
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
        }))
    })
}

pub(crate) fn requirements_result_json(result: &GetSkillRequirementResult) -> serde_json::Value {
    json!({
        "function": nexus_sdk::idents::registry::AgentRegistry::GET_SKILL_REQUIREMENTS.name.to_string(),
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "active_skill_revision_key": result.active_skill_revision_key,
        "requirements": result.requirements,
    })
}

pub(crate) fn schedule_task_result_json(
    result: &CreateTaskResult,
    agent_id: sui::types::Address,
    skill_id: SkillId,
    dag_id: sui::types::Address,
) -> serde_json::Value {
    json!({
        "function": "schedule_task",
        "digest": result.tx_digest,
        "scheduled_task_id": result.task_id,
        "agent_id": agent_id,
        "skill_id": skill_id,
        "dag_id": dag_id,
        "tap_payment": result.tap_payment.as_ref().map(|payment| json!({
            "agent_id": payment.agent_id,
            "skill_id": payment.skill_id,
            "prepay_amount": payment.prepay_amount,
            "occurrence_budget": payment.occurrence_budget,
        })),
        "initial_schedule": result.initial_schedule.as_ref().map(|schedule| json!({
            "digest": schedule.tx_digest,
            "event": schedule.event,
        })),
    })
}

pub(crate) fn execution_settle_result_json(
    result: &CommittedToolResultSettlementResult,
) -> serde_json::Value {
    json!({
        "function": "settle_committed_tool_result_for_walk",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
        "walk_index": result.walk_index,
    })
}

pub(crate) fn execution_abort_result_json(result: &AbortExecutionResult) -> serde_json::Value {
    json!({
        "function": "abort_expired_execution",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
    })
}

// ============================================================================
// Payments: show, wait, list
// ============================================================================

pub(crate) fn payment_show_result_json(payment: &ExecutionPayment) -> serde_json::Value {
    json!({
        "payment_id": payment.payment_id(),
        "execution_id": payment.execution_id,
        "agent_id": payment.agent_id.bytes,
        "skill_id": payment.skill_id,
        "interface_revision": payment.interface_revision,
        "payment_policy": payment.payment_policy,
        "source_kind": payment.source_kind,
        "max_budget": payment.max_budget,
        "locked_budget": payment.locked_budget,
        "funds": payment.funds,
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
    wallet_receipts: &[ExecutionPaymentReceipt],
    vault_receipts: &[ExecutionPaymentReceipt],
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
        "function": function,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "execution_id": result.execution_id,
        "agent_id": result.agent_id,
    })
}

pub(crate) fn payment_refill_result_json(
    result: &RefillExecutionPaymentResult,
) -> serde_json::Value {
    json!({
        "function": if result.agent_id.is_some() {
            "refill_tap_execution_payment_from_agent_vault"
        } else {
            "refill_tap_execution_payment"
        },
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "execution_id": result.execution_id,
        "agent_id": result.agent_id,
        "amount": result.amount,
    })
}

// ============================================================================
// Registry + default-agent inspection
// ============================================================================

pub(crate) fn registry_show_result_json(registry: &AgentRegistrySnapshot) -> serde_json::Value {
    json!({
        "id": registry.id,
        "default_executor": registry.default_executor,
        "agents": registry.agents,
        "skills": registry.skills,
    })
}

pub(crate) fn default_agent_result_json(record: &DefaultDagExecutorRecord) -> serde_json::Value {
    json!({
        "agent_id": record.target.agent_id,
        "skill_id": record.target.skill_id,
        "dag_binding": record.skill.dag_binding(),
        "dag_id": record.skill.dag_binding().pinned_dag_id(),
        "interface_revision": record.skill_revision.key.interface_revision,
        "requirements": record.skill_revision.requirements,
    })
}

// ============================================================================
// Vault: balance, deposit
// ============================================================================

pub(crate) fn vault_balance_result_json(
    agent_id: AgentId,
    vault: &nexus_sdk::nexus::crawler::Response<AgentPaymentVault>,
) -> serde_json::Value {
    json!({
        "agent_id": agent_id,
        "vault_id": vault.object_id,
        "available_balance": vault.data.available_balance_value(),
        "locked_amount": vault.data.locked_amount,
        "unlocked_balance": vault.data.unlocked_balance_value(),
    })
}

pub(crate) fn vault_deposit_result_json(result: &DepositAgentVaultResult) -> serde_json::Value {
    json!({
        "function": nexus_sdk::idents::interface::Agent::DEPOSIT_AGENT_PAYMENT_VAULT.name.to_string(),
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
                interface::{
                    agent::{SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
                    payment::{ExecutionPaymentFinalState, SkillPaymentPolicy},
                    version::InterfaceVersion,
                },
                registry::agent_registry::SkillRecord,
                DefaultDagExecutorTarget,
                SkillRecordContext,
                SkillRevisionContext,
                SkillRevisionLookupKey,
            },
        },
    };

    // ---- shared fixtures ----

    fn fixture_artifact() -> TapPublishArtifact {
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirement {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
        };
        TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
            .expect("artifact builds")
    }

    fn fixture_payment(accomplished: bool, refunded: bool) -> ExecutionPayment {
        let final_state = if accomplished {
            ExecutionPaymentFinalState::Accomplished
        } else if refunded {
            ExecutionPaymentFinalState::Refunded
        } else {
            ExecutionPaymentFinalState::Pending
        };

        ExecutionPayment {
            id: nexus_sdk::types::sui_address_to_uid(sui::types::Address::from_static("0xaa")),
            execution_id: sui::types::Address::from_static("0xbb"),
            agent_id: nexus_sdk::types::sui_address_to_id(sui::types::Address::from_static("0xcc")),
            skill_id: 11,
            interface_revision: InterfaceVersion::new(2),
            payment_policy: nexus_sdk::types::interface::payment::SkillPaymentPolicy::UserFunded,
            source_kind: nexus_sdk::types::interface::payment::PaymentSourceKind::user_funded(
                sui::types::Address::from_static("0xee"),
            ),
            max_budget: 1_000,
            locked_budget: 0,
            funds: nexus_sdk::types::sui_framework::balance::Balance {
                value: 1_000,
                phantom_t0: std::marker::PhantomData,
            },
            consumed: 0,
            tool_cost_snapshot: nexus_sdk::types::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            accomplished,
            refunded,
            final_state,
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
            current_interface_revision: InterfaceVersion::new(2),
            dag_binding: nexus_sdk::types::interface::agent::SkillDagBinding::pinned(
                artifact.dag_id,
            ),
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
                skill_revision_key: SkillRevisionLookupKey {
                    agent_id: sui::types::Address::from_static("0xa"),
                    skill_id: 11,
                    interface_revision: InterfaceVersion::new(3),
                },
                payment_max_budget: 99,
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
    }

    #[test]
    fn tap_requirements_result_json_exposes_live_state() {
        let requirements = SkillRequirement {
            input_commitment: vec![1],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };

        let requirements_output = requirements_result_json(&GetSkillRequirementResult {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            active_skill_revision_key: SkillRevisionLookupKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(3),
            },
            requirements,
        });
        assert_eq!(
            requirements_output["active_skill_revision_key"]["interface_revision"],
            serde_json::json!(3)
        );
        assert_eq!(
            requirements_output["requirements"]["input_commitment"],
            serde_json::json!([1])
        );
    }

    #[test]
    fn schedule_task_result_json_is_schedule_output_replacement() {
        let result = CreateTaskResult {
            tx_digest: sui::types::Digest::from([8; 32]),
            task_id: sui::types::Address::from_static("0x77"),
            initial_schedule: None,
            tap_payment: Some(nexus_sdk::nexus::scheduler::CreateTaskTapPaymentResult {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                prepay_amount: 500,
                occurrence_budget: 50,
            }),
        };

        let output = schedule_task_result_json(
            &result,
            sui::types::Address::from_static("0xa"),
            11,
            sui::types::Address::from_static("0xd"),
        );

        assert_eq!(output["function"], serde_json::json!("schedule_task"));
        assert_eq!(
            output["scheduled_task_id"],
            serde_json::json!(sui::types::Address::from_static("0x77").to_string())
        );
        assert_eq!(
            output["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert_eq!(
            output["tap_payment"]["prepay_amount"],
            serde_json::json!(500)
        );
        assert_eq!(
            output["tap_payment"]["occurrence_budget"],
            serde_json::json!(50)
        );
        assert!(output["initial_schedule"].is_null());
    }

    // ---- payments ----

    #[test]
    fn payment_show_result_json_includes_terminal_flag() {
        let json = payment_show_result_json(&fixture_payment(true, false));
        assert!(json.get("standard_tap").is_none());
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
        assert!(json.get("standard_tap").is_none());
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

    #[test]
    fn payment_refill_result_json_marks_coin_refill_function() {
        let result = RefillExecutionPaymentResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 5,
            execution_id: sui::types::Address::from_static("0xe"),
            agent_id: None,
            amount: 123,
        };

        let json = payment_refill_result_json(&result);

        assert_eq!(json["function"], "refill_tap_execution_payment");
        assert_eq!(json["tx_checkpoint"], 5);
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
        assert_eq!(json["agent_id"], serde_json::Value::Null);
        assert_eq!(json["amount"], 123);
    }

    #[test]
    fn payment_refill_result_json_marks_agent_vault_refill_function() {
        let result = RefillExecutionPaymentResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 8,
            execution_id: sui::types::Address::from_static("0xe"),
            agent_id: Some(sui::types::Address::from_static("0xa")),
            amount: 456,
        };

        let json = payment_refill_result_json(&result);

        assert_eq!(
            json["function"],
            "refill_tap_execution_payment_from_agent_vault"
        );
        assert_eq!(
            json["agent_id"],
            sui::types::Address::from_static("0xa").to_string()
        );
        assert_eq!(json["amount"], 456);
    }

    #[test]
    fn execution_settle_result_json_includes_stable_fields() {
        let result = CommittedToolResultSettlementResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 7,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
            walk_index: 3,
        };

        let json = execution_settle_result_json(&result);

        assert_eq!(json["function"], "settle_committed_tool_result_for_walk");
        assert_eq!(json["tx_checkpoint"], 7);
        assert_eq!(
            json["dag_id"],
            sui::types::Address::from_static("0xda6").to_string()
        );
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
        assert_eq!(json["walk_index"], 3);
    }

    #[test]
    fn execution_abort_result_json_includes_stable_fields() {
        let result = AbortExecutionResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 9,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
        };

        let json = execution_abort_result_json(&result);

        assert_eq!(json["function"], "abort_expired_execution");
        assert_eq!(json["tx_checkpoint"], 9);
        assert_eq!(
            json["dag_id"],
            sui::types::Address::from_static("0xda6").to_string()
        );
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
    }

    // ---- registry + default-agent inspection ----

    #[test]
    fn default_agent_result_json_keeps_flat_agent_schema() {
        let agent_id = sui::types::Address::from_static("0xad");
        let dag_id = sui::types::Address::from_static("0xd");
        let requirements = SkillRequirement {
            input_commitment: vec![1, 2, 3],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };
        let record = DefaultDagExecutorRecord {
            target: DefaultDagExecutorTarget {
                agent_id,
                skill_id: 7,
            },
            skill: SkillRecordContext {
                agent_id,
                skill_id: 7,
                record: SkillRecord {
                    description: b"default agent".to_vec(),
                    active: true,
                    dag_binding: SkillDagBinding::pinned(dag_id),
                    requirements: requirements.clone(),
                    current_interface_revision: InterfaceVersion::new(3),
                    scheduled_task_count: 0,
                },
            },
            skill_revision: SkillRevisionContext {
                key: SkillRevisionLookupKey {
                    agent_id,
                    skill_id: 7,
                    interface_revision: InterfaceVersion::new(3),
                },
                requirements,
            },
        };

        let json = default_agent_result_json(&record);

        assert!(json.get("standard_tap").is_none());
        assert_eq!(json["agent_id"], serde_json::json!(agent_id.to_string()));
        assert_eq!(json["skill_id"], serde_json::json!(7));
        assert_eq!(json["dag_id"], serde_json::json!(dag_id.to_string()));
        assert_eq!(json["interface_revision"], serde_json::json!(3));
        assert!(json.get("requirements").is_some());
        assert!(json.get("target").is_none());
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
        assert!(json.get("standard_tap").is_none());
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

        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: PathBuf::from("dag.json"),
            requirements: SkillRequirement {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(7),
        };

        let validate = validate_skill_result_json(&config);
        assert_eq!(validate["valid"], serde_json::Value::Bool(true));
        assert_eq!(validate["interface_revision"], serde_json::json!(7));

        let dry_run = dry_run_result_json(&config);
        assert_eq!(dry_run["dry_run"], serde_json::Value::Bool(true));
        assert!(dry_run.get("config_digest_hex_with_zero_package").is_none());
    }

    #[test]
    fn create_skill_artifact_result_json_is_raw_artifact_shape() {
        let artifact = fixture_artifact();
        let output = create_skill_artifact_result_json(&artifact);

        assert_eq!(output["skill_name"], serde_json::json!("weather skill"));
        assert_eq!(output["interface_revision"], serde_json::json!(1));
        assert_eq!(
            output["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert!(output.get("requirements").is_some());
        assert!(output.get("standard_tap").is_none());
        assert!(output.get("function").is_none());
        assert!(output.get("out").is_none());
        assert!(output.get("tap_package_id").is_none());
    }
}
