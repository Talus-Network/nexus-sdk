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
        move_bindings::interface::{
            agent::{SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
            payment::{
                ExecutionPayment, ExecutionPaymentFinalState, PaymentSourceKind, SkillPaymentPolicy,
            },
        },
        nexus::{
            tap::{
                BindAgentSkillResult, DepositAgentVaultResult, RefillExecutionPaymentResult,
                WaitForPaymentResult,
            },
            workflow::{
                AbortExecutionResult, CommittedToolResultSettlementResult,
                ExpiredWalkResolutionResult,
            },
        },
        types::{AgentId, AgentRegistrySnapshot, DefaultDagExecutorRecord, SkillConfig},
    },
    std::fmt::Write as _,
};

const FIELD_WIDTH: usize = 20;

fn write_field(output: &mut String, label: &str, value: impl std::fmt::Display) {
    writeln!(output, "{label:<FIELD_WIDTH$}{value}").expect("writing to a String cannot fail");
}

fn payment_policy_label(policy: &SkillPaymentPolicy) -> String {
    match policy {
        SkillPaymentPolicy::UserFunded => "user funded".to_owned(),
        SkillPaymentPolicy::AgentFunded { max_budget_mist } => {
            format!("agent funded, maximum {max_budget_mist} MIST")
        }
    }
}

fn schedule_policy_label(policy: &SkillSchedulePolicy) -> String {
    match policy {
        SkillSchedulePolicy::Once => "once".to_owned(),
        SkillSchedulePolicy::Recurring {
            min_interval_ms,
            max_occurrences,
        } => format!(
            "recurring every {min_interval_ms} ms, maximum {}",
            max_occurrences
                .as_option()
                .map_or_else(|| "unlimited".to_owned(), |value| value.to_string())
        ),
    }
}

fn dag_binding_label(binding: &SkillDagBinding) -> String {
    match binding {
        SkillDagBinding::Pinned { dag_id } => format!("pinned to {dag_id}"),
        SkillDagBinding::RuntimeSelected => "selected at runtime".to_owned(),
    }
}

pub(crate) fn render_requirements(result: &GetSkillRequirementResult) -> String {
    let mut output = String::new();
    write_field(&mut output, "Agent", result.agent_id);
    write_field(&mut output, "Skill", result.skill_id);
    write_field(
        &mut output,
        "Interface revision",
        result.active_skill_revision_key.interface_revision,
    );
    write_field(
        &mut output,
        "Input commitment",
        hex::encode(&result.requirements.input_commitment),
    );
    write_field(
        &mut output,
        "Payment",
        payment_policy_label(&result.requirements.payment_policy),
    );
    write_field(
        &mut output,
        "Schedule",
        schedule_policy_label(&result.requirements.schedule_policy),
    );
    write_field(
        &mut output,
        "Fixed tools",
        result.requirements.fixed_tools.len(),
    );
    for tool in &result.requirements.fixed_tools {
        writeln!(
            output,
            "  {} in registry {}",
            tool.tool_fqn_string(),
            tool.tool_registry_address(),
        )
        .expect("writing to a String cannot fail");
    }
    output
}

pub(crate) fn render_payment(payment: &ExecutionPayment) -> String {
    let mut output = String::new();
    let source = match &payment.source_kind {
        PaymentSourceKind::UserFunded { user } => format!("user {user}"),
        PaymentSourceKind::AgentFunded { agent_id } => format!("agent {}", agent_id.bytes),
    };
    let final_state = match payment.final_state {
        ExecutionPaymentFinalState::Pending => "pending",
        ExecutionPaymentFinalState::Accomplished => "accomplished",
        ExecutionPaymentFinalState::Refunded => "refunded",
    };

    write_field(&mut output, "Payment", payment.payment_id());
    write_field(&mut output, "Execution", payment.execution_id);
    write_field(&mut output, "Agent", payment.agent_id.bytes);
    write_field(&mut output, "Skill", payment.skill_id);
    write_field(
        &mut output,
        "Interface revision",
        payment.interface_revision,
    );
    write_field(&mut output, "State", final_state);
    write_field(&mut output, "Source", source);
    write_field(
        &mut output,
        "Policy",
        payment_policy_label(&payment.payment_policy),
    );
    write_field(
        &mut output,
        "Available funds",
        format_args!("{} MIST", payment.funds.value),
    );
    write_field(
        &mut output,
        "Locked budget",
        format_args!("{} MIST", payment.locked_budget_mist),
    );
    write_field(
        &mut output,
        "Consumed",
        format_args!("{} MIST", payment.consumed),
    );
    write_field(
        &mut output,
        "Locked vertices",
        payment.locked_vertices.len(),
    );
    output
}

pub(crate) fn render_payment_wait(result: &WaitForPaymentResult) -> String {
    let mut output = render_payment(&result.payment);
    write_field(
        &mut output,
        "Elapsed",
        format_args!("{} ms", result.elapsed_ms),
    );
    write_field(&mut output, "Timed out", result.timed_out);
    output
}

pub(crate) fn render_registry(registry: &AgentRegistrySnapshot) -> String {
    let mut output = String::new();
    write_field(&mut output, "Registry", registry.id);
    write_field(&mut output, "Agents", registry.agents.len());
    write_field(&mut output, "Skills", registry.skills.len());
    if let Some(default_executor) = &registry.default_executor {
        let target = default_executor.target();
        write_field(&mut output, "Default agent", target.agent_id);
        write_field(&mut output, "Default skill", target.skill_id);
    } else {
        write_field(&mut output, "Default executor", "none");
    }

    for agent in &registry.agents {
        output.push('\n');
        write_field(&mut output, "Agent", agent.agent_id);
        write_field(&mut output, "Active", agent.active());
        write_field(&mut output, "Skill count", agent.record.skills.size());
    }
    for skill in &registry.skills {
        output.push('\n');
        write_field(
            &mut output,
            "Skill",
            format_args!("{}:{}", skill.agent_id, skill.skill_id),
        );
        write_field(
            &mut output,
            "Description",
            String::from_utf8_lossy(&skill.record.description),
        );
        write_field(&mut output, "Active", skill.active());
        write_field(
            &mut output,
            "Interface revision",
            skill.current_interface_revision(),
        );
        write_field(&mut output, "DAG", dag_binding_label(skill.dag_binding()));
    }
    output
}

pub(crate) fn render_default_agent(record: &DefaultDagExecutorRecord) -> String {
    let mut output = String::new();
    write_field(&mut output, "Agent", record.target.agent_id);
    write_field(&mut output, "Skill", record.target.skill_id);
    write_field(
        &mut output,
        "Interface revision",
        record.skill_revision.key.interface_revision,
    );
    write_field(
        &mut output,
        "DAG",
        dag_binding_label(record.skill.dag_binding()),
    );
    write_field(
        &mut output,
        "Payment",
        payment_policy_label(&record.skill_revision.requirements.payment_policy),
    );
    write_field(
        &mut output,
        "Schedule",
        schedule_policy_label(&record.skill_revision.requirements.schedule_policy),
    );
    output
}

pub(crate) fn render_agent_list(agents: &[(String, AgentId)]) -> String {
    if agents.is_empty() {
        return "No saved Talus agent aliases.\n".to_owned();
    }

    let mut output = String::new();
    for (name, agent_id) in agents {
        write_field(&mut output, name, agent_id);
    }
    output
}

pub(crate) fn render_agent_remove(name: &str, removed: Option<AgentId>) -> String {
    match removed {
        Some(agent_id) => format!("Removed Talus agent alias {name} ({agent_id}).\n"),
        None => format!("No Talus agent alias named {name}.\n"),
    }
}

pub(crate) fn render_vault_balance(
    agent_id: AgentId,
    vault: &nexus_sdk::nexus::crawler::Response<
        nexus_sdk::move_bindings::interface::agent::AgentPaymentVaultStateV1,
    >,
) -> String {
    let mut output = String::new();
    write_field(&mut output, "Agent", agent_id);
    write_field(&mut output, "Vault", vault.object_id);
    write_field(
        &mut output,
        "Available balance",
        format_args!("{} MIST", vault.data.available_balance_value()),
    );
    write_field(
        &mut output,
        "Locked amount",
        format_args!("{} MIST", vault.data.locked_amount),
    );
    write_field(
        &mut output,
        "Unlocked balance",
        format_args!("{} MIST", vault.data.unlocked_balance_value()),
    );
    output
}

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
        "interface_revision": config.interface_revision.inner,
    })
}

pub(crate) fn dry_run_result_json(config: &SkillConfig) -> serde_json::Value {
    json!({
        "dry_run": true,
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision.inner,
        "next_step": "publish TAP plus DAG, then create-agent and register-skill",
    })
}

pub(crate) fn create_skill_artifact_result_json(
    artifact: &TapPublishArtifact,
) -> serde_json::Value {
    artifact_result_json(artifact)
}

fn artifact_result_json(artifact: &TapPublishArtifact) -> serde_json::Value {
    json!({
        "skill_name": artifact.skill_name,
        "dag_id": artifact.dag_id,
        "interface_revision": artifact.interface_revision.inner,
        "requirements": skill_requirements_json(&artifact.requirements),
    })
}

fn skill_requirements_json(requirements: &SkillRequirement) -> serde_json::Value {
    let schedule_policy = match &requirements.schedule_policy {
        SkillSchedulePolicy::Once => json!({ "kind": "once" }),
        SkillSchedulePolicy::Recurring {
            min_interval_ms,
            max_occurrences,
        } => json!({
            "kind": "recurring",
            "min_interval_ms": min_interval_ms,
            "max_occurrences": max_occurrences.as_option(),
        }),
    };
    let fixed_tools = requirements
        .fixed_tools
        .iter()
        .map(|tool| {
            json!({
                "tool_registry_id": tool.tool_registry_address(),
                "tool_fqn": tool.tool_fqn_string(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "input_commitment_hex": hex::encode(&requirements.input_commitment),
        "payment_policy": skill_payment_policy_json(&requirements.payment_policy),
        "schedule_policy": schedule_policy,
        "fixed_tools": fixed_tools,
    })
}

fn skill_payment_policy_json(policy: &SkillPaymentPolicy) -> serde_json::Value {
    match policy {
        SkillPaymentPolicy::UserFunded => json!({ "kind": "user_funded" }),
        SkillPaymentPolicy::AgentFunded { max_budget_mist } => json!({
            "kind": "agent_funded",
            "max_budget_mist": max_budget_mist,
        }),
    }
}

fn skill_dag_binding_json(binding: &SkillDagBinding) -> serde_json::Value {
    match binding {
        SkillDagBinding::Pinned { dag_id } => json!({
            "kind": "pinned",
            "dag_id": dag_id,
        }),
        SkillDagBinding::RuntimeSelected => json!({ "kind": "runtime_selected" }),
    }
}

// ============================================================================
// Publish + bind lifecycle
// ============================================================================

pub(crate) fn create_agent_result_json(result: &CreateAgentResult) -> serde_json::Value {
    json!({
        "function": nexus_sdk::move_bindings::registry::agent_registry::create_agent_target()
            .expect("generated agent_registry::create_agent target")
            .function
            .to_string(),
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
        "artifact": artifact_result_json(&result.artifact),
    })
}

pub(crate) fn register_skill_result_json(
    artifact: &TapPublishArtifact,
    result: &RegisterSkillResult,
) -> serde_json::Value {
    json!({
        "function": nexus_sdk::move_bindings::registry::agent_registry::register_skill_target()
            .expect("generated agent_registry::register_skill target")
            .function
            .to_string(),
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
        "current_interface_revision": result.current_interface_revision.inner,
        "dag_id": artifact.dag_id,
        "dag_binding": skill_dag_binding_json(&result.dag_binding),
        "requirements": skill_requirements_json(&result.requirements),
    })
}

// ============================================================================
// Skill execution + requirements
// ============================================================================

pub(crate) fn requirements_result_json(result: &GetSkillRequirementResult) -> serde_json::Value {
    json!({
        "function": nexus_sdk::move_bindings::registry::agent_registry::get_skill_requirements_target()
            .expect("generated agent_registry::get_skill_requirements target")
            .function
            .to_string(),
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "active_skill_revision_key": {
            "agent_id": result.active_skill_revision_key.agent_id,
            "skill_id": result.active_skill_revision_key.skill_id,
            "interface_revision": result.active_skill_revision_key.interface_revision.inner,
        },
        "requirements": skill_requirements_json(&result.requirements),
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

pub(crate) fn execution_resolve_expired_walk_result_json(
    result: &ExpiredWalkResolutionResult,
) -> serde_json::Value {
    json!({
        "function": "resolve_expired_walk",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
        "walk_index": result.walk_index,
        "resolution_kind": result.resolution_kind.as_str(),
        "resolution": result.resolution_kind,
        "skip_reason": result.resolution_kind.skip_reason(),
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
    let source = match &payment.source_kind {
        PaymentSourceKind::UserFunded { user } => json!({
            "kind": "user_funded",
            "user": user,
        }),
        PaymentSourceKind::AgentFunded { agent_id } => json!({
            "kind": "agent_funded",
            "agent_id": agent_id.bytes,
        }),
    };
    let final_state = match payment.final_state {
        ExecutionPaymentFinalState::Pending => "pending",
        ExecutionPaymentFinalState::Accomplished => "accomplished",
        ExecutionPaymentFinalState::Refunded => "refunded",
    };
    let tool_costs = payment
        .tool_cost_snapshot
        .contents
        .iter()
        .map(|entry| {
            json!({
                "tool_fqn": String::from_utf8_lossy(&entry.key),
                "cost_mist": entry.value,
            })
        })
        .collect::<Vec<_>>();
    let locked_vertices = payment
        .locked_vertices
        .iter()
        .map(|lock| {
            json!({
                "vertex_key": String::from_utf8_lossy(&lock.vertex_key),
                "invocation_id": lock.invocation_id.bytes,
                "amount_mist": lock.amount,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "payment_id": payment.payment_id(),
        "protocol_version": payment.protocol_version,
        "execution_id": payment.execution_id,
        "agent_id": payment.agent_id.bytes,
        "skill_id": payment.skill_id,
        "interface_revision": payment.interface_revision.inner,
        "payment_policy": skill_payment_policy_json(&payment.payment_policy),
        "source": source,
        "max_budget_mist": payment.max_budget_mist,
        "gas_budget_mist": payment.gas_budget_mist,
        "priority_fee_reserve_mist": payment.priority_fee_reserve_mist,
        "locked_budget_mist": payment.locked_budget_mist,
        "available_funds_mist": payment.funds.value,
        "consumed_mist": payment.consumed,
        "tool_fee_charged_mist": payment.tool_fee_charged,
        "priority_fee_charged_mist": payment.priority_fee_charged,
        "priority_fee_percentage": payment.priority_fee_percentage,
        "accomplished": payment.accomplished,
        "refunded": payment.refunded,
        "final_state": final_state,
        "terminal": nexus_sdk::nexus::tap::payment_is_terminal(payment),
        "tool_costs": tool_costs,
        "locked_vertices": locked_vertices,
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
    let agents = registry
        .agents
        .iter()
        .map(|agent| {
            json!({
                "agent_id": agent.agent_id,
                "active": agent.active(),
                "skill_count": agent.record.skills.size(),
            })
        })
        .collect::<Vec<_>>();
    let skills = registry
        .skills
        .iter()
        .map(|skill| {
            json!({
                "agent_id": skill.agent_id,
                "skill_id": skill.skill_id,
                "description": String::from_utf8_lossy(&skill.record.description),
                "active": skill.active(),
                "dag_binding": skill_dag_binding_json(skill.dag_binding()),
                "interface_revision": skill.current_interface_revision().inner,
                "scheduled_task_count": skill.record.scheduled_task_count,
                "requirements": skill_requirements_json(skill.requirements()),
            })
        })
        .collect::<Vec<_>>();
    let default_executor = registry.default_executor.as_ref().map(|executor| {
        let target = executor.target();
        json!({
            "agent_id": target.agent_id,
            "skill_id": target.skill_id,
        })
    });

    json!({
        "id": registry.id,
        "agent_count": agents.len(),
        "skill_count": skills.len(),
        "default_executor": default_executor,
        "agents": agents,
        "skills": skills,
    })
}

pub(crate) fn default_agent_result_json(record: &DefaultDagExecutorRecord) -> serde_json::Value {
    json!({
        "agent_id": record.target.agent_id,
        "skill_id": record.target.skill_id,
        "dag_binding": skill_dag_binding_json(record.skill.dag_binding()),
        "dag_id": record.skill.dag_binding().pinned_dag_id(),
        "interface_revision": record.skill_revision.key.interface_revision.inner,
        "requirements": skill_requirements_json(&record.skill_revision.requirements),
    })
}

// ============================================================================
// Vault: balance, deposit
// ============================================================================

pub(crate) fn vault_balance_result_json(
    agent_id: AgentId,
    vault: &nexus_sdk::nexus::crawler::Response<
        nexus_sdk::move_bindings::interface::agent::AgentPaymentVaultStateV1,
    >,
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
        "function": nexus_sdk::move_bindings::interface::agent::deposit_agent_payment_vault_target()
            .expect("generated agent::deposit_agent_payment_vault target")
            .function
            .to_string(),
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
            move_bindings::{
                interface::{
                    agent::{Agent, SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
                    payment::{ExecutionPaymentFinalState, SkillPaymentPolicy},
                    version::InterfaceVersion,
                },
                registry::agent_registry::{AgentRecord, DefaultDagExecutor, SkillRecord},
                sui_framework::table::Table as MoveTable,
            },
            nexus::{
                tap::TapPackagePublishResult,
                workflow::{ExpiredWalkResolutionKind, PublishResult},
            },
            types::{
                AgentRecordContext, DefaultDagExecutorTarget, SkillRecordContext,
                SkillRevisionContext, SkillRevisionLookupKey,
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
            id: nexus_sdk::move_bindings::sui_framework::object::UID::new(
                sui::types::Address::from_static("0xaa"),
            ),
            protocol_version: 1,
            execution_id: sui::types::Address::from_static("0xbb"),
            agent_id: nexus_sdk::move_bindings::sui_framework::object::ID::new(
                sui::types::Address::from_static("0xcc"),
            ),
            skill_id: 11,
            interface_revision: InterfaceVersion::new(2),
            payment_policy:
                nexus_sdk::move_bindings::interface::payment::SkillPaymentPolicy::UserFunded,
            source_kind:
                nexus_sdk::move_bindings::interface::payment::PaymentSourceKind::user_funded(
                    sui::types::Address::from_static("0xee"),
                ),
            max_budget_mist: 1_000,
            gas_budget_mist: 333,
            priority_fee_reserve_mist: 666,
            locked_budget_mist: 0,
            funds: nexus_sdk::move_bindings::sui_framework::balance::Balance {
                value: 1_000,
                phantom_t0: std::marker::PhantomData,
            },
            consumed: 0,
            tool_cost_snapshot: nexus_sdk::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            accomplished,
            refunded,
            final_state,
            locked_vertices: vec![],
            tool_fee_charged: 0,
            priority_fee_charged: 0,
            priority_fee_percentage: 200,
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
        assert!(!output.to_string().contains("\"bytes\""));
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
            dag_binding: nexus_sdk::move_bindings::interface::agent::SkillDagBinding::pinned(
                artifact.dag_id,
            ),
            requirements: artifact.requirements.clone(),
        };
        let json = update_skill_result_json(&artifact, &result);
        assert_eq!(json["function"], "update_skill");
        assert_eq!(json["skill_id"], serde_json::json!(7));
        assert_eq!(json["current_interface_revision"], serde_json::json!(2));
        assert_eq!(json["dag_binding"]["kind"], "pinned");
        assert_eq!(json["requirements"]["input_commitment_hex"], "01");
        assert!(!json.to_string().contains("\"inner\""));
        assert!(!json.to_string().contains("\"bytes\""));
        assert!(json.get("config_digest_hex").is_none());
    }

    // ---- execute + requirements + schedule ----

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
            requirements_output["requirements"]["input_commitment_hex"],
            serde_json::json!("01")
        );
        assert!(!requirements_output.to_string().contains("\"inner\""));
        assert!(!requirements_output.to_string().contains("\"bytes\""));

        let report = render_requirements(&GetSkillRequirementResult {
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            active_skill_revision_key: SkillRevisionLookupKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(3),
            },
            requirements: SkillRequirement {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
        });
        assert!(report.contains("Interface revision  3"));
        assert!(report.contains("Payment             user funded"));
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
        assert_eq!(json["interface_revision"], serde_json::json!(2));
        assert_eq!(json["payment_policy"]["kind"], "user_funded");
        assert_eq!(json["source"]["kind"], "user_funded");
        assert_eq!(
            json["source"]["user"],
            sui::types::Address::from_static("0xee").to_string()
        );
        assert_eq!(json["available_funds_mist"], serde_json::json!(1_000));
        assert_eq!(json["final_state"], "accomplished");
        assert_eq!(json["tool_costs"], serde_json::json!([]));
        assert_eq!(json["locked_vertices"], serde_json::json!([]));
        assert!(!json.to_string().contains("\"inner\""));
        assert!(!json.to_string().contains("\"bytes\""));
        assert!(!json.to_string().contains("\"phantom_t0\""));

        let report = render_payment(&fixture_payment(true, false));
        assert!(report.contains("State               accomplished"));
        assert!(report.contains("Available funds     1000 MIST"));
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
    fn execution_resolve_expired_walk_result_json_includes_resolution_fields() {
        let result = ExpiredWalkResolutionResult {
            tx_digest: None,
            tx_checkpoint: None,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
            walk_index: 4,
            resolution_kind: ExpiredWalkResolutionKind::Skipped {
                reason: "not expired".to_string(),
            },
        };

        let json = execution_resolve_expired_walk_result_json(&result);

        assert_eq!(json["function"], "resolve_expired_walk");
        assert_eq!(json["resolution_kind"], "skipped");
        assert_eq!(json["resolution"]["kind"], "skipped");
        assert_eq!(json["skip_reason"], "not expired");
        assert_eq!(json["walk_index"], 4);
    }

    #[test]
    fn execution_abort_result_json_includes_stable_fields() {
        let result = AbortExecutionResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 9,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
            cleaned_broken_onchain_results: Vec::new(),
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
    fn registry_show_result_json_exposes_domain_records() {
        let agent_id = sui::types::Address::from_static("0xad");
        let dag_id = sui::types::Address::from_static("0xd");
        let registry = AgentRegistrySnapshot {
            id: sui::types::Address::from_static("0xa0"),
            agents: vec![AgentRecordContext {
                agent_id,
                record: AgentRecord {
                    active: true,
                    skills: MoveTable::new(sui::types::Address::from_static("0xa2"), 1),
                },
            }],
            skills: vec![SkillRecordContext {
                agent_id,
                skill_id: 7,
                record: SkillRecord {
                    description: b"default agent".to_vec(),
                    active: true,
                    dag_binding: SkillDagBinding::pinned(dag_id),
                    requirements: SkillRequirement {
                        input_commitment: vec![1],
                        payment_policy: SkillPaymentPolicy::default(),
                        schedule_policy: SkillSchedulePolicy::default(),
                        fixed_tools: Vec::new(),
                    },
                    current_interface_revision: InterfaceVersion::new(3),
                    scheduled_task_count: 2,
                },
            }],
            default_executor: Some(DefaultDagExecutor {
                agent: Agent::from_anchor(agent_id, sui::types::Address::from_static("0xa3"), 1),
                skill_id: 7,
            }),
        };

        let json = registry_show_result_json(&registry);

        assert_eq!(json["agent_count"], 1);
        assert_eq!(json["skill_count"], 1);
        assert_eq!(json["agents"][0]["agent_id"], agent_id.to_string());
        assert_eq!(json["agents"][0]["skill_count"], 1);
        assert_eq!(json["skills"][0]["description"], "default agent");
        assert_eq!(json["skills"][0]["interface_revision"], 3);
        assert_eq!(json["skills"][0]["dag_binding"]["kind"], "pinned");
        assert_eq!(json["default_executor"]["agent_id"], agent_id.to_string());
        assert!(!json.to_string().contains("\"inner\""));
        assert!(!json.to_string().contains("\"bytes\""));

        let report = render_registry(&registry);
        assert!(report.contains("default agent"));
        assert!(report.contains(&agent_id.to_string()));
    }

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
        assert_eq!(json["dag_binding"]["kind"], "pinned");
        assert_eq!(json["requirements"]["input_commitment_hex"], "010203");
        assert!(!json.to_string().contains("\"inner\""));
        assert!(!json.to_string().contains("\"bytes\""));
        assert!(json.get("target").is_none());

        let report = render_default_agent(&record);
        assert!(report.contains("Interface revision  3"));
        assert!(report.contains(&dag_id.to_string()));
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
        assert!(render_agent_list(&[("primary".to_string(), agent_id)]).contains("primary"));

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
        assert_eq!(dry_run["interface_revision"], serde_json::json!(7));
        assert!(dry_run.get("config_digest_hex_with_zero_package").is_none());
    }

    #[test]
    fn create_skill_artifact_result_json_is_semantic_artifact_shape() {
        let artifact = fixture_artifact();
        let output = create_skill_artifact_result_json(&artifact);

        assert_eq!(output["skill_name"], serde_json::json!("weather skill"));
        assert_eq!(output["interface_revision"], serde_json::json!(1));
        assert_eq!(
            output["dag_id"],
            serde_json::json!(sui::types::Address::from_static("0xd").to_string())
        );
        assert!(output.get("requirements").is_some());
        assert_eq!(output["requirements"]["input_commitment_hex"], "01");
        assert_eq!(
            output["requirements"]["payment_policy"]["kind"],
            "user_funded"
        );
        assert_eq!(output["requirements"]["schedule_policy"]["kind"], "once");
        assert!(!output.to_string().contains("\"inner\""));
        assert!(!output.to_string().contains("\"bytes\""));
        assert!(output.get("standard_tap").is_none());
        assert!(output.get("function").is_none());
        assert!(output.get("out").is_none());
        assert!(output.get("tap_package_id").is_none());
    }
}
