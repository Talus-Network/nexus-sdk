# Agent development

This guide is for builders who are starting with Nexus agent development and need the current onchain model before choosing a client or package workflow. It explains how registered agents, skills, DAGs, fixed tools, payment vaults, and scheduled execution fit together, with behavior grounded in `sui/interface/sources/agent.move`, `sui/registry/sources/agent_registry.move`, and the user-flow guides under `docs/guides`.

If the terms are unfamiliar, read the [glossary](../../nexus-next/glossary.md), [Talus agent](../../nexus-next/concepts/01-talus-agent.md), [Workflow DAG execution](../../nexus-next/concepts/05-workflow-dag-execution.md), and [Payment vaults, reserves, and settlement](../../nexus-next/concepts/09-payment-vaults-reserves-and-settlement.md).

## Current agent model

Current agent development uses `nexus_interface::agent`, `nexus_interface::payment`, `nexus_interface::authorization`, `nexus_interface::scheduled_request`, and `nexus_registry::agent_registry`. There is no separate live `tap` module; the registry owns agent records, skill records, default runtime DAG executor metadata, and the active interface revision for each skill.

## Development paths that the code supports

- **Registered agent skill**: Create an `Agent`, publish or select a DAG, register a skill with `register_skill` or `register_skill_with_fixed_tools`, and update the skill's DAG binding or payment/schedule policy requirements through the registry when the skill contract changes.
- **Default runtime DAG executor**: Use the deployment-provided default DAG executor for runtime-selected DAG execution when no app-specific business logic is needed.
- **Custom package + standard registry**: Publish a package with custom state or business logic, but still register the agent, skill requirements, authorization templates, and payments through the standard registry and interface. Use [Build a TAP Move package](./build-tap-move-package.md) when the package embeds an agent, owns assets, and exposes cap-gated onchain tools.

## Procedure for a registered skill

1. Publish the workflow DAG or choose runtime-selected default DAG execution.
1. Define the skill requirements: input commitment, DAG binding, `SkillPaymentPolicy`, `SkillSchedulePolicy`, fixed tools, and any `AgentVertexAuthorizationTemplate` values needed for protected onchain vertices. Use `register_skill_with_fixed_tools` when the skill must commit to a fixed-tool list; the plain `register_skill` path records an empty fixed-tool list.
1. Create the agent through the registry. Agent creation also creates the agent's `AgentPaymentVault`.
1. Register the skill with the owning `Agent`. `register_skill` allocates the next agent-local `SkillId` from the `Agent` object and stores the registry-side `SkillRecord` with the current DAG binding, requirements, and interface revision.
1. Update the skill contract through `update_dag` when the live DAG binding changes or through `update_skill_policies` when the payment or schedule policy changes. Current update APIs preserve the existing fixed-tool list, so choose fixed tools at registration time or register a replacement skill when that set must change.
1. Publish discovery metadata by relying on standard registry events and records. Leaders and SDK clients resolve active runtime state by `(agent_id, skill_id, interface_revision)`.
1. Execute through the registered agent path or the default runtime DAG executor path. A worksheet pins the `Agent`, `SkillId`, interface revision, selected DAG, and execution ID so later payment and authorization checks use the same revision.
1. Create payment using the skill policy. Invoker-funded execution supplies a payment coin; agent-funded execution uses the `AgentPaymentVault`. Both paths create an execution-bound `ExecutionPayment` child under the `DAGExecution`.
1. Finalize payment after execution. Successful executions accomplish the payment; failed or rejected executions refund it. For scheduled execution, workflow settlement records the occurrence as accomplished or refunded against the scheduled reserve, and `finish` marks the task `Completed` only when no scheduled work remains.

## Default runtime DAG execution

The default path is a registry-owned default agent target. Deployment bootstraps a runtime-selected default skill and stores a `DefaultDagExecutor` in the registry. Default DAG execution and scheduler-triggered default execution resolve that target, create the same worksheet and payment context, and pin the runtime-selected DAG in execution evidence.

Use this path when the agent does not need package-specific skill logic. Use a registered skill when the agent needs a stable skill with its own DAG binding, payment policy, schedule policy, fixed tool set, or authorization templates.

## Payment vaults

Every standard Talus agent receives an `AgentPaymentVault` when the agent is created. The vault is the canonical onchain balance holder for agent-funded execution.

Anyone can deposit SUI into an agent vault. Withdrawals require a transaction that can present the mutable `Agent` object and a registered agent record; the current registry helper checks that the agent exists and the interface vault code enforces the vault and balance operation. This is not a separate registry owner/operator ACL in the current Move implementation. When a skill policy selects agent-funded execution, payment creation locks the requested budget from the agent vault into an `ExecutionPayment`. Consumption is tracked on the payment, and finalization either charges the consumed amount on accomplish or releases the lock on refund.

## Scheduling and authorization

Scheduled agent execution creates a scheduler `Task` tied to the agent, skill, interface revision, payment source, reserve, and occurrence policy. Each triggered occurrence converts prepaid reserve into a normal execution-bound payment. Settlement records whether that occurrence accomplished or refunded, pauses the task on refunded scheduled payment, and `finish` marks the task `Completed` only when no scheduled work remains.

Fixed onchain tool authorization uses `SkillRequirement.fixed_tools`, `AgentVertexAuthorizationTemplate`, and per-vertex authorization grants. Skill revisions commit to the allowed fixed tools and payment requirement. Leaders verify the grant against the pinned worksheet before a `ProvenValue<AgentVertexAuthorization>` can authorize the fixed tool call.

## What to read next

Continue learning about agent development with:

- the [Nexus interface package](../../nexus-next/packages/nexus-interface.md), which summarizes the active interface modules used by standard flows
- the [Agent registry reference](../../nexus-next/packages/reference/nexus_registry/agent_registry.md), which summarizes the registry-side agent, skill, payment, scheduling, and default DAG executor surfaces
- the [TAP Move package guide](./build-tap-move-package.md), which gives the Move package recipe for embedded-agent assets, fixed tools, grants, scheduled follow-up execution, and tests
