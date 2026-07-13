# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### `nexus-sdk`

#### Added

- Move binding regeneration now accepts an optional matching Move source root for restoring
  function parameter names. Network package metadata remains authoritative, and regeneration
  without source keeps deterministic `argN` names.

#### Changed

- Move binding regeneration now preserves the reduced Move standard library and Sui framework IR,
  limiting deployment refreshes to Nexus packages.
- Move binding regeneration now commits canonical SDK package identities, preventing package ID
  churn when the same Move ABI is rebound from another deployment.

## [`2.0.0-rc.4`] - 2026-07-09

### `nexus-cli`

#### Added

- `dag abort-expired-execution` command that takes a DAGExecution ID, derives the selected DAG and expiry clock from on-chain state, discovers eligible ToolGas candidates for an expired TAP DAG execution, submits the ToolGas-assisted abort PTB, and emits the selected ToolGas plus matching walk metadata as JSON.
- `tap publish-skill` now publishes the TAP Move package, publishes the DAG, derives the current skill artifact, and writes the agent/skill binding data needed for follow-up execution.
- `tap create-skill-artifact` command that builds the current skill `TapPublishArtifact` JSON from explicit skill inputs and a read-only published-DAG fetch for `requirements.input_schema_commitment`.
- `tap update-skill` command that updates an existing agent skill from a publish artifact and reports the resulting current interface revision, DAG binding, and requirements.
- `tap scheduled-task pause|resume|cancel` commands for explicit-agent scheduled task state changes with required `--agent-id`.
- `tap execution settle` and `tap execution abort` commands for permissionless committed-result settlement and expired execution abort flows, with abort automatically cleaning finalized tracked on-chain results whose required stamps are insufficient after double expiry before submitting the abort.
- `tap execution resolve-expired-walk` command that classifies one double-timeout walk and submits committed-result settlement, plain abort, or ToolGas-assisted abort through the shared SDK helper.
- `tap payments refill` command for coin top-ups and agent-vault-funded execution payment refills.
- `DagExecution` decoding now exposes execution walk summary counters: `active_walks`, `pending_abort_walks`, `successful_walks`, `failed_walks`, `aborted_walks`, `consumed_walks`, and `cancelled_walks`.

#### Changed

- SDK idents, PTB builders, CLI calls, event fixtures, schema helpers, and execution discovery now target the split Move module owners (`interface::Dag`/`interface::Graph`, `registry::ToolRegistry`/`registry::VerifierRegistry`, and `workflow::Execution*`) introduced by the workflow module split.
- TAP CLI terminology now uses default agent or `DefaultDAGExecutor` for Agent Registry surfaces, including the `tap default-agent show` command replacing `tap default-target show`.
- `tap create-agent` and `tap bind` now derive authorization from the shared agent object flow and no longer accept an explicit `--operator` argument.
- `tap publish-skill`, `tap bind`, `tap register-skill`, `tap dry-run`, registry inspection, default-agent inspection, requirements output, execution output, payment output, and scheduling output now reflect the simplified current-skill model rather than endpoint-revision/config-digest records.
- TAP and scheduler command outputs now use current-skill, fixed-tool, scheduled task, direct `UserFunded` and `AgentFunded`, and DAG-owned worksheet terminology, matching the current Move interface split and removing stale endpoint-revision, shared-object, refund-mode, and config-digest fields from active flows.
- direct TAP execution and scheduled TAP execution commands now construct and report the same skill, grant, payment, worksheet, and settlement model instead of exposing attach-style scheduled compatibility state.
- `--agent-id` command paths now validate whether the resolved agent object can be used mutably or immutably before submission, returning a local CLI error for immutable objects on mutable operations.
- `scheduler task pause`, `scheduler task resume`, and `scheduler task cancel` now use the registry-backed default-agent task-state path without requiring a caller-supplied agent object.

#### Removed

- `tap announce` endpoint-revision command; use `tap update-skill` to move an existing skill to a new current revision.
- TAP execute and schedule refund-mode flags; payment and schedule policy now come from the simplified skill requirements.
- Replace `tap schedule` with `tap schedule-task` for explicit-agent scheduled TAP task creation and scheduler occurrence/periodic commands for timing.

### `nexus-sdk`

#### Added

- `WorkflowActions`/crawler support for fetching on-chain tool result state by execution and walk, returning finalized state through the generated `nexus_primitives::onchain_tool_result::OnchainToolResult` type plus the shared object reference.
- `DagExecution` walk decoding plus `WorkflowActions::abort_expired_execution_tool_gas_candidates` and `abort_expired_execution_with_tool_gas`, which derive the DAG from execution state, compare active walks against the on-chain Clock, find matching TAP vertex locks, and submit the ToolGas-assisted Move abort wrapper.
- `DagExecution` now decodes the on-chain `dag` field so execution recovery paths can use the DAG selected when the execution was created.
- Low-level and high-level helpers for current committed-result settlement, record-only leader gas-charge submission, expired execution abort, and execution-payment refill flows.
- `WorkflowActions::resolve_expired_walk` and `inspect_expired_walk_resolution` classify timeout-expired walks into settlement, plain abort, ToolGas abort, or skipped outcomes without adding new Move entrypoints.
- `inspect_expired_walk_resolution` now classifies finalized raw `OnchainToolResult` evidence as a leader-authenticated consume-and-settle branch before no-result abort planning.
- `WorkflowActions::resolve_expired_walk` and the raw on-chain result settlement PTB builder now submit finalized `OnchainToolResult` settlement permissionlessly without a leader cap or leader gas-charge arguments.
- `WorkflowActions::abort_expired_execution` can prepend the `cleanup_broken_onchain_tool_result` PTB call for double-expired finalized on-chain results with insufficient required stamps before submitting the abort.
- Receipt lifecycle event decoding for `ExecutionPaymentReceiptCreatedEvent`, `ExecutionPaymentReceiptResolvedEvent`, and `ScheduledPaymentReserveReceiptCreatedEvent`.
- `SchedulerActions::create_task` (via `CreateTaskParams::agent_id` and `CreateTaskParams::skill_id`) now routes through `transactions::scheduler::new_agent_execution_policy` (`BeginAgentExecutionWitness`) when both ids are supplied, so callers can register an agent-bound scheduled task without dropping to a raw PTB. Half-supplied bindings (one id without the other) fail locally with `NexusError::Configuration`.
- Scheduler PTB builders for settling finished scheduled TAP execution payments from task-owned address-funded or agent-vault reserves.
- `TapActions::update_skill_from_artifact` and `transactions::tap` helpers for updating an existing skill's DAG binding, payment policy, schedule policy, fixed-tool requirements, and current interface revision from a `TapPublishArtifact`.
- `transactions::agent_input::AgentInput`, a reusable transaction input type for owned, shared, and immutable agent objects with explicit mutable and immutable argument export.
- `TapFixedTool` requirements and DAG input commitment derivation for the current skill artifact shape.
- `LeaderClaimedEvent { registry, leader_cap_id, claim_token }` mirroring the on-chain `nexus_registry::leader::activate_and_claim` event, with its `NexusEventKind::LeaderClaimed` variant and BCS decoder generated by the `events!` macro. The leader consumes it to learn which activation claim currently owns a leader record.
- typed authorization and payment models mirroring the split Move modules, including recipient-bound grant/cap values, direct payment-source identity, scheduled reserve state, and verifier worksheet proof shapes.
- Generated Move bindings now come from committed normalized Move package IR under `sdk/src/move_bindings/ir/*.json`, rendered at build time by `build.rs`. The `regenerate_bindings` binary refreshes that IR from a published objects TOML and a running gRPC endpoint (`cargo run --features binding_codegen --bin regenerate_bindings -- <objects_toml> [grpc_url]`, wrapped by `just sdk rebind`).

#### Changed

- On-chain tool transaction builders now follow the split result flow without an SDK finalize helper: create the workflow result object for the tool phase, and consume finalized shared results without leader-supplied output or failure-evidence arguments.
- `TapPublishArtifact` no longer carries `tap_package_id`, shared objects, or config-digest fields; reusable skill artifacts now contain only `skill_name`, `dag_id`, `interface_revision`, and simplified requirements consumed by register, bind, and update flows.
- `TapSkillRequirements` now carries `input_schema_commitment`, `payment_policy`, `schedule_policy`, and `fixed_tools`, replacing workflow/metadata commitments and `TapVertexAuthorizationSchema`.
- `TapPaymentPolicy` and `TapSchedulePolicy` now mirror the current on-chain enum shapes, including user-funded vs agent-funded payment policy and once vs recursive schedule recurrence.
- `SchedulerActions::create_task` now keeps user-funded scheduled tasks sender-owned while agent-vault-funded tasks route through the mutable-agent, agent-owned scheduler constructor.
- `transactions::tap::register_skill_with_vertex_authorization_schema` is now `transactions::tap::register_skill_with_fixed_tools`, with fixed-tool argument helpers so skills can register registry-verified `TapFixedTool` requirements without carrying a vertex authorization schema.
- TAP registry, default-agent, active execution, and requirement resolution models now use current skill records and derived skill revisions instead of endpoint revision records.
- TAP package publishing options now pass the selected Sui Move build environment by name.
- SDK idents, event decoding, object models, action helpers, PTB builders, and crawler lookups now follow the split `agent`, `authorization`, `payment`, `scheduled_request`, `verifier`, registry, scheduler, and workflow Move module layout.
- DAG and workflow builders now use DAG-owned worksheet submission and generic leader worksheet stamping, removing SDK references to registry worksheet confirmation and the dry-run-only `leader_stamp_worksheet_for_dry_run` surface.
- scheduled execution builders now pass scheduled task identity and task-attached authorization/payment reserve state through the same execution configuration shape used by direct TAP execution, unifying occurrence preparation, settlement, and event decoding.
- Explicit-agent scheduled task payment constructors are exposed through `transactions::tap`, while default-agent scheduled task constructors remain under `transactions::scheduler`, matching the CLI split between explicit agent TAP flows and default scheduler flows.
- Scheduled task state decoding now mirrors the on-chain five-state lifecycle `Active`, `Paused`, `Canceled`, `Completed`, and `Failed`, while legacy `Exhausted` scheduled-state wire values decode as `Completed`.
- Agent-object transaction builders and high-level actions now route user-supplied agent ids through `AgentInput`, so mutable calls reject immutable objects locally and immutable calls can borrow shared objects immutably.
- Abort-expired workflow actions now read DAG metadata from `DagExecution.dag` instead of re-resolving the current active skill binding, allowing runtime-selected active bindings to recover already-created executions.
- ToolGas abort candidate discovery now ignores timeout-expired pending-settlement walks, leaving committed-result timeouts to the settlement branch.
- DAG and scheduled occurrence execution builders now use the current begin-lock-start Move flow without a request-walk hot potato or lock ticket.
- Scheduled task settlement builders now keep automatic occurrence settlement task+execution-only and expose whole-reserve idle agent-funded scheduled reserve collection as a task+agent operation.
- Scheduler task-state builders and `SchedulerActions::set_task_state` now pass the agent registry so default-agent task-state calls can validate against the registry-owned default agent and guarded scheduled-count cleanup path.
- `TapActions::set_agent_task_state` and explicit-agent scheduler task-state PTB builders now operate TAP scheduled tasks with a required agent id.
- Committed-result leader gas-charge and settlement helpers now take a settlement gas charge in addition to the prior commit gas charge.
- Bump Sui SDK and images to `mainnet-v1.73.2`

#### Removed

- Endpoint-revision announcement and inspection SDK surfaces, endpoint config-digest helpers, shared-object requirement plumbing, and stale `TapV1`/default-target terminology from active TAP paths.
- stale SDK identifiers, structures, and PTB helpers for endpoint revisions, vertex authorization schemas, payment-source hashes, registry worksheet confirmation, TAP-specific leader stamps, dry-run-only workflow stamps, and attach-style scheduled occurrence entrypoints.
- Stale attach-style and registry scheduled payment APIs, including `ScheduleSkillExecutionResult`, `ScheduleReserveFund`, `ScheduleSkillExecutionFromAgentVaultParams`, `TapActions::schedule_skill_execution`, `TapActions::schedule_skill_execution_from_agent_vault`, `TapActions::schedule_skill_execution_address_funded`, `TapActions::schedule_default_dag_executor_skill_execution_address_funded`, `transactions::tap::schedule_skill_execution`, registry `SCHEDULE_SKILL_EXECUTION*` idents, and raw registry/scheduler attachment PTB builders; use scheduled task creation with `CreateTaskTapPayment` plus occurrence and periodic APIs.
- Per-occurrence scheduled settlement-to-agent-vault SDK builders and idents; use task+execution settlement for occurrence completion and whole-reserve collection or cancellation for agent-vault reserve exits.
- `LEADER_STAMP_WORKSHEET` and its method, as this should be internal during submission result.
- use `transactions::dag::prepare_tool_result_submission_worksheet` to build the public worksheet preparation call.

#### Fixed

- Rebased the Move-binding migration on the current split workflow/on-chain result surface, keeping generated binding types and PTB builders as the SDK boundary for local and on-chain tool result flows.
- On-chain tool result inspection now filters same-shape dynamic-field keys by value type before BCS decoding, avoiding false settlement-marker reads from unrelated execution fields.
- Verification verdict event parsing now accepts string-valued Move-JSON option payloads for checked leader/tool key ids, matching transaction event JSON emitted by current workflow contracts.
- `TerminalErrEvalRecordedEvent.outcome` is now optional so SDK, CLI, and parser callers can represent primary retry `_err_eval` records before a post-failure action is resolved.
- Verification verdict event parsing now accepts nested inspection JSON that omits `dag`, matching the existing default used for submission-failure evidence helpers.
- Committed-result dynamic-field reads now decode wake metadata without parsing raw `variant_ports_to_data` payload bytes as JSON, so listener freshness checks can handle non-JSON tool outputs.
- Event parsing now accepts foreign-emitter `RequestScheduledOccurrenceEvent` wrappers so extension packages that call public scheduler occurrence entrypoints still produce distributable scheduled occurrence events.
- `DagExecution` walk decoding now mirrors the current workflow `DAGWalk::PendingSettlement` variant so executions waiting on committed-result settlement can be inspected.
- Signed HTTP invoke authentication now rejects replayed signatures paired with a different request body by comparing the signed `body_sha256` claim to the inbound body hash.
- `DagExecution` JSON decoding now accepts the on-chain `dag` field for abort-expired execution recovery paths.
- `transactions::tap::register_skill_with_fixed_tools` now passes the required immutable `ToolRegistry` shared object to the registry entry so fixed-tool validation matches the current Move signature.
- SDK and CLI PTB helpers now use the current `sui-transaction-builder` object, pure input, function type-argument, and opaque argument APIs consistently, restoring `just sdk test` and `just cli test` under the `0.3.1` builder stack.
- Scheduler task state decoding now accepts Sui Move JSON enum objects with `@variant`, allowing leader scheduled occurrence execution to fetch current task objects while preserving string JSON output.

### `docs`

#### Changed

- Moved docs to <https://github.com/Talus-Network/nexus-docs>.

## [`2.0.0-rc.2`] - 2026-06-10

### `nexus-cli`

#### Added

- `tap registry show` command that prints the full standard TAP agent registry contents (id, default executor, agents, skills, endpoint revisions) as stable JSON.
- `tap default-target show` command that flattens the configured standard TAP default DAG executor — agent id, skill id, dag id, interface revision, config-digest hex, shared objects, and skill requirements — into one JSON document.
- `tap payments show` command that reads a `TapExecutionPayment` object and emits a flat JSON of all payment fields plus a computed `terminal` flag.
- `tap payments wait` command that polls a `TapExecutionPayment` until accomplished/refunded or until a configurable timeout, emitting `elapsed_ms`/`timed_out` alongside the payment state.
- `tap payments resolve --execution-id <OBJECT_ID> [--alias <NAME> | --agent-id <OBJECT_ID>]` command that wraps the on-chain `nexus_workflow::dag::accomplish_tap_execution_payment` (invoker-funded) and `accomplish_tap_execution_payment_from_agent_vault` (vault-funded) PTBs depending on whether an agent is supplied. Backed by `TapActions::accomplish_execution_payment` and `AccomplishExecutionPaymentParams`/`AccomplishExecutionPaymentResult` (now carrying `agent_id: Option<sui::types::Address>`) on the SDK side.
- `tap bind` command that composes `tap::create_agent` and `tap::register_skill` into a single PTB and returns the new agent id, skill id, agent object ref, config-digest evidence, and transaction metadata.
- `tool inspect` command that derives `Tool` and `ToolGas` object IDs from an FQN, probes both on-chain, and emits the full decoded `Tool` record (HTTP- or Sui-variant) under a stable `tool` JSON field, so callers do not have to BCS-decode it themselves. `tool register on-chain` and `tool register off-chain` JSON outputs now emit the same `tool` field after re-fetching the freshly-registered object — scripted consumers only need to learn one Tool contract.
- `tool register on-chain --workflow-authorization-cap-first` flag to route registration through the cap-gated `register_on_chain_tool_with_workflow_authorization_cap` entrypoint.
- `tool register on-chain` JSON output now includes the locally-derived `tool_id` and `tool_gas_id`, the `workflow_authorization_cap_first` flag, and the transaction checkpoint, so callers do not need to derive these values themselves.
- `tap vault deposit` command that funds an agent's payment vault by splitting MIST from the signer's gas coin, taking the agent via `--agent-id` (or local `--alias`) and `--amount`.
- `tap schedule-address-funded` command that creates an address-funded scheduled TAP task tied to an existing scheduler task, attaches the `TapScheduledTaskLink`, and shares the resulting `ScheduledSkillTask` in one transaction.
- `tap schedule-from-vault` command that creates an agent-vault-funded scheduled TAP task tied to an existing scheduler task and attaches the `TapScheduledTaskLink` in one transaction.
- `tap schedule-default-address-funded` command that creates an address-funded scheduled TAP task for the registry-owned default DAG executor and attaches the `TapScheduledTaskLink` in one transaction.
- `scheduler task create --agent-id <OBJECT_ID> --skill-id <U64>` flag pair that scopes the created task to a registered TAP agent skill so the workflow dispatches walks under `BeginAgentExecutionWitness` (agent-bound) instead of the default `BeginExecutionWitness` policy. The flags must be supplied together; one without the other is rejected before any RPC is made.

#### Changed

- `tool register on-chain` now extracts the `OwnerCap<OverGas>` minted by the registration PTB (disambiguated from `OwnerCap<OverTool>` by its generic type parameter), reports it as `owner_cap_over_gas_id`, and persists it so later gas-management commands (`tool set-invocation-cost`, `gas tickets …`) can resolve it.
- `tap scaffold` now writes a `tap/Move.toml` that declares all four published Nexus dependencies (`nexus_primitives`, `nexus_interface`, `nexus_registry`, `nexus_workflow`). The previous scaffold omitted `nexus_registry`, forcing authors to add it by hand before the package would compile against the TAP development guide's recommended template.
- `tap validate-skill` and `tap publish-skill` no longer accept `--tap-package`. The flag was a redundant override of `tap_package_path` from the skill config; relying on it from a parent directory produced confusing double-prefixed paths (`tutorial-transfer/tutorial-transfer/tap/Move.toml does not exist`). Both commands now resolve the TAP package strictly from the config's `tap_package_path` (resolved relative to the config file's directory).
- `dag inspect-execution` no longer accepts `--execution-checkpoint`. The SDK now derives the starting checkpoint from the `DAGExecution` object's creation transaction (via `Crawler::get_object_creation_checkpoint`).

#### Fixed

- `tap publish-skill --out`, `tap scaffold`, and `tool new` now write generated files with `tokio::fs::write` instead of `File::create` + `write_all`. A dropped `tokio::fs::File` does not flush its internal buffer, so under load a reader (or the next command in a pipeline) could observe a truncated or empty artifact/scaffold file — surfacing as an intermittent `EOF while parsing a value` failure (e.g. the flaky `publish_artifact_flow_writes_revision_metadata` test).
- `tool register onchain` now correctly disambiguates `OwnerCap<OverTool>` vs `OwnerCap<OverGas>` in the post-registration response.

### `nexus-sdk`

#### Added

- `Crawler::get_object_creation_checkpoint(object_id)` that resolves the checkpoint sequence number of the transaction that created a shared object by chaining three gRPC calls: current metadata (for `Owner::Shared(initial_shared_version)`) → version-pinned `GetObject` (for `previous_transaction`) → `BatchGetTransactions` (for `checkpoint`). Owned objects are rejected with a clear error.
- `TapActions::inspect_endpoint` reading an endpoint object's on-chain metadata and returning an `EndpointInspection` carrying its object ref. In the current TAP model endpoint revisions live on the agent registry keyed by `(agent_id, skill_id, interface_revision)`, so use `tap registry show` to inspect revisions and active endpoints.
- `TapActions::bind_agent_skill` composed PTB that runs `tap::create_agent` and `tap::register_skill` in a single transaction, with `BindAgentSkillParams` capturing operator and artifact, and `BindAgentSkillResult` capturing the transaction digest/checkpoint, agent/skill ids, agent object ref, and the derived config-digest plus its `TapConfigDigestInput`.
- `TapActions::wait_for_payment_settled` poll helper with `WaitForPaymentResult` (final payment state, `terminal`, `elapsed_ms`, `timed_out`) and a `payment_is_terminal` free function that recognizes `accomplished`/`refunded`/non-`Pending` `TapExecutionPaymentFinalState`. A zero `poll_interval` is rejected with `NexusError::Configuration` to avoid busy-looping the poller.
- `ToolActions::inspect_tool` that derives `Tool`/`ToolGas` ids from an FQN, probes both on-chain, and decodes the on-chain `Tool` into a `ToolInspection` carrying the full `Tool` record (HTTP- or Sui-variant). Mixed-existence states (one present, the other missing) surface as a clear `NexusError::Configuration`.
- `TapActions::deposit_agent_payment_vault` high-level helper that fetches the agent object reference, splits the deposit coin from gas, and submits the `tap::deposit_agent_payment_vault` call. `DepositAgentVaultParams` and `DepositAgentVaultResult` expose `agent_id` and `amount` for callers.
- `transactions::tap::register_skill_with_vertex_authorization_schema` PTB builder, plus `authorized_tool_arg` and `vertex_authorization_schema_arg` helpers, so cap-gated skills can register through the agent registry with a non-default `TapVertexAuthorizationSchema`. `TapActions::register_skill` and `TapActions::bind_agent_skill` auto-detect the schema and route through this builder when `TapVertexAuthorizationSchema::is_default()` returns `false`, falling back to the simpler `register_skill` call when both `fixed_tools` is empty and `requires_payment` is `false`.
- `TapVertexAuthorizationSchema::is_default()` predicate so callers can introspect whether a schema needs to be sent through the cap-gated registration entry point.
- `TapPublishArtifact::from_config` now substitutes the `0x0` sentinel in any `fixed_tools[].package_id` with the just-published `tap_package_id` before computing the artifact's config digest. Authors can declare a self-referential on-chain tool entry without knowing the package id ahead of time.
- `SchedulerActions::create_task` (via `CreateTaskParams::agent_id` and `CreateTaskParams::skill_id`) now routes through `transactions::scheduler::new_agent_execution_policy` (`BeginAgentExecutionWitness`) when both ids are supplied, so callers can register an agent-bound scheduler task without dropping to a raw PTB. Half-supplied bindings (one id without the other) fail locally with `NexusError::Configuration`.

#### Changed

- `WorkflowActions::inspect_execution` and `WorkflowActions::inspect_execution_until_completion` no longer take an `execution_checkpoint: u64` argument. The starting checkpoint is now derived internally by the SDK from the `DAGExecution` object's creation transaction via the new `Crawler::get_object_creation_checkpoint` helper (chain: current `Owner::Shared(initial_shared_version)` → version-pinned `previous_transaction` → transaction `checkpoint`). Callers should drop the second positional argument.
- Added `max_transaction_budget` to `LeaderRegistry` model

### `docs`

#### Added

- Six-page **TAP development** guide series under `docs/guides/` walking an entry-level standard TAP skill end-to-end.

## [`2.0.0-rc.1`] - 2026-06-01

### `nexus-cli`

#### Added

- Terminal `_err_eval` event handling in DAG execution inspection, including failure class, post-failure action, reason, duplicate-submission status, and `_err_eval` hash output.
- `tap publish-skill` now publishes the TAP Move package, publishes the DAG, computes endpoint revision metadata and config digest, and writes a complete endpoint-revision artifact for operator handoff.

#### Changed

- DAG execution inspection now includes terminal `_err_eval` trace entries in JSON output and highlights duplicate terminal submissions in human-readable output.
- On-chain tool registration config persistence is now covered by a serialized test to avoid cross-test config interference.
- Generated standard fixed-tool templates now include the hidden `VertexAuthorizationCheckCap` plus workflow worksheet arguments expected by endpoint-declared authorization-aware fixed tools.

### `nexus-sdk`

#### Added

- `PaymentLockUpdateEvent` parsing is now allowed from public workflow/TAP calls emitted through non-Nexus caller packages, with regression coverage for the wrapped event shape used by cap-gated standard TAP executions.
- SDK models now expose scheduled TAP task and occurrence context from `DAGExecution`, so execution inspection and payment recovery can identify the scheduled task that funded a walk.
- `ExecutionCostResult` now reports the standard TAP payment object, locked budget, consumed amount, outstanding vertex locks, and terminal accomplishment/refund status from execution-owned payment state.
- PTB helpers now cover vertex authorization grant creation and active-walk authorization-check-cap minting for cap-gated on-chain tools.
- `TerminalErrEvalRecordedEvent` and `SubmissionFailureEvidenceRecordedEvent` SDK event types.
- Nested Move-JSON parsers for terminal `_err_eval` records and submission failure evidence, including wrapper, option, enum, address, runtime-vertex, byte-vector, and normalized string forms.
- Workflow failure model types, including `FailureEvidenceKind`, `WorkflowFailureClass`, `PostFailureAction`, and `ExecutionTerminalRecord`.
- DAG JSON `post_failure_action`, `leader_verifier`, and `tool_verifier` support at both DAG and vertex scope.
- Workflow Move identifiers and transaction helpers for post-failure actions, verifier configs, failure-evidence kinds, terminal `_err_eval` submission, success-path tool evaluation submission, failure-evidence submission, typed on-chain tool result submission, off-chain verifier proof submission, and expired execution aborts.
- `settle_gas_state_for_vertex` gas transaction helper.
- `WorkflowActions::inspect_execution_until_completion`, returning terminal state, terminal `_err_eval` records, end-state outputs, and the underlying execution event stream.
- Focused coverage for branch-specific SDK transaction builder behavior, including terminal `_err_eval` output shape, verifier config wiring, no-verifier auxiliary routing, verifier proof routing, and explicit on-chain witness passthrough.
- Typed external-verifier PTB helper that constructs `OffchainVerifierEvidence`, calls the registered verifier package, wraps the returned `VerifierContractResult` as typed verifier proof, and submits through the verifier-aware workflow entrypoint.
- Additional signed HTTP tests for response signing verification and multi-variant output handling.
- SDK-owned standard TAP authorization-plan models and current-vertex grant resolution helpers for fixed-tool execution.
- High-level standard TAP package publishing, DAG publishing orchestration, standard endpoint revision metadata construction, and complete publish-artifact construction.
- Standard TAP transaction helpers for endpoint revision announcement, active skill-revision updates, and SDK-owned authorization-cap fixed-tool submit and dry-run PTB sequencing.
- Compatibility-focused parser fixtures for current standard TAP event BCS layouts, including Move `Option<T>` event fields used by request, payment, authorization, and scheduled-execution events.
- Standard Talus agent payment vault models, fetch helper, deposit/withdraw PTB builders, and typed payment source helpers for invoker-funded and agent-vault-funded settlement.
- Agent-scoped workflow gas helpers for standard TAP funding.
- Durable scheduled TAP models, events, fetch helpers, and transaction builders for address-funded and agent-vault-funded scheduled prepayment, scheduled occurrence payment conversion, scheduled occurrence completion, and scheduler-task link attachment.
- SDK-level `fetch_task_tap_scheduled_task_link` and `fetch_tap_scheduled_skill_task` helpers so leaders can recover on-chain scheduled task state without local-only BCS parsing.
- Default-DAG-executor address-funded scheduling action and PTB builder that omit `agent_id`/`skill_id` arguments and resolve the registry-owned default executor through `TapRegistry`.

#### Changed

- Workflow object decoding now treats the standard TAP execution context as complete only when agent, skill, endpoint revision, payment, selected DAG, authorization plan, and scheduled occurrence fields are consistent.
- `VertexAuthorizationGrantCreatedEvent` and `WorkflowVertexAuthorizationGrant` now use `execution_id`, matching the current Move object/event layout.
- Default DAG execution helpers prefer the configured `default_tap_target` when it resolves to a runtime-selected active skill, then fall back to registry recovery.
- `full` and `nexus` no longer enable `move_publish`; package publishing APIs now require the explicit `move_publish` feature, and the CLI opts into it for deployment commands.
- Optimized nested event value parsing with improved performance and reduced redundant checks
- Made `parse_nested_event_value` public for use in nested event parsing
- Fixed `FailureEvidenceKind` enum serialization with explicit serde rename attributes and backward compatibility aliases
- Improved signed HTTP verification with proper body hash validation and detection of tampered requests
- Enhanced canonical output body SHA256 calculation with validation for single-variant JSON structure
- Fixed signature message generation to handle references correctly in signed HTTP operations
- Removed unused sender parameters from `create_tool_binding_and_register_key` and `create_leader_binding_and_register_key`; new binding objects are now shared with `public_share_object` instead of transferred to a sender.
- Event parsing now accepts terminal `_err_eval` and submission-failure records from BCS-wrapped events and common nested Move-JSON trace shapes.
- Move-JSON deserialization is more tolerant of Sui wrapper shapes for fields, options, published Move enums, runtime vertices, execution terminal records, strings, booleans, integers, byte vectors, and addresses.
- `NexusEventKind` execution matching and workflow inspection now account for terminal `_err_eval` events.
- Walrus file downloads now flush the destination file before returning.
- On-chain DAG transaction helpers now use the single typed `submit_on_chain_tool_result_for_walk_v1` surface and no longer expose the stale BCS-envelope or split success/failure helper API.
- Signed HTTP response signing now steers low-level callers to `sign_invoke_response_with_body_v1`; the deprecated status-only helper rejects 2xx responses because `_err_eval` outcome derivation depends on the response body.

#### Fixed

- Workflow-owned TAP authorization grant and authorization-plan decoding now accepts current Move-JSON runtime-vertex payloads, and standard runtime-stamp contract tests track explicit vertex construction calls.

### `nexus-toolkit`

#### Changed

- Runtime config hot reload now moves filesystem metadata checks and config parsing onto Tokio's blocking pool and reduces fallback polling frequency, while keeping notify-driven reloads as the fast path.
- Standard fixed-tool schema generation hides TAP authorization-cap and workflow worksheet internal arguments from user-facing input schemas.
- CLI and SDK object metadata now resolve default DAG execution through `agent_registry` and `default_tap_target` rather than a `default_tap` object.
- Legacy `TapV1` and `AnnounceInterfacePackageEvent` support is retained only as compatibility for historical data; active standard builders use standard TAP idents.
- SDK parser sample coverage now uses generated current-layout standard TAP event fixtures instead of stale hard-coded witness-era bytes.
- scheduler and peer TAP PTB tests now assert standard TAP calls by package/module/function and current BCS argument layout rather than brittle absolute command indexes.
- Standard TAP payment validation now accepts typed invoker and agent-vault payment sources while preserving legacy address-BCS compatibility.
- `TapExecutionPayment` decoding now tolerates source kind, source identity, locked budget, and final-state fields emitted by the updated TAP interface.
- `TapScheduledSkillTask` decoding now mirrors the durable on-chain `ScheduledSkillTask` shape, including payment source, remaining reserve, in-flight occurrence, occurrence records, and final state.
- Scheduled occurrence transaction helpers now chain scheduler occurrence checks with standard TAP scheduled-payment preparation instead of requiring leader-built local payment arguments.
- `DagExecution` decoding accepts Sui Move JSON `Option<u64>` scheduled occurrence indexes, including string-valued `vec` forms emitted by object JSON.
- Standard TAP SDK models and PTB builders now match the Move package split: general TAP identities, payments, endpoint, worksheet, authorization, and schedule types resolve through `nexus_interface::tap`, while registry storage records remain under `nexus_registry::tap`.
- Active execution APIs now expose explicit agent DAG execution and default-agent DAG execution names (`execute_agent_dag`, `execute_default_agent_dag`, `AgentDagExecuteInput`, and `AgentDagExecuteOptions`), while "standard TAP" remains reserved for protocol/model surfaces.
- TAP payment settlement builders now target execution-owned `DAGExecution` payment helpers, with separate agent-vault variants only when vault accounting needs the explicit non-default `Agent`.
- Default DAG execution and scheduling builders now use mutable `agent_registry` inputs and the configured `default_tap_target` identity without fetching or passing a separate default `Agent` object.

#### Fixed

- Registered-key and external-verifier submissions now build typed verifier proof values and route through the single verifier-aware workflow entrypoint; no-verifier submissions route through the dedicated no-verifier entrypoint, and auxiliary bytes carry only optional `_err_eval` failure-evidence classification.
- SDK test fixtures now track the current standard TAP event and PTB layouts, fixing parsing and PTB-order regressions exposed after the standard TAP cutover.
- Raw `TapRegistryObject` BCS decoding now models Move `Option<T>` layout for `default_executor`, fixing default TAP DAG executor recovery from shared registry objects used by leader bootstrap and scheduler flows.
- Default executor address-funded scheduling no longer fetches the configured default agent as a top-level object, fixing leader scheduled default TAP setup after default agent custody moved under `TapRegistry`.

#### Removed

- Stale `OnChainToolResultSubmissionV1` SDK type/export and obsolete onchain BCS-envelope/split-submit Move identifiers.
- Active SDK `DefaultTap`/`TapV1` default execution builders and `NexusObjects.default_tap` deployment metadata requirements.
- active payment-auth parameters, wrapped-style `SkillId(...)` construction, and public `InvokerGas`/`ExecutionGas` caller paths from SDK and CLI TAP flows.

### `docs`

#### Added

- Comprehensive DAG construction guide sections for `post_failure_action`, `leader_verifier`, and `tool_verifier` configuration at both DAG and vertex levels
- Updated basic DAG JSON structure documentation to include all optional configuration fields
- Refactoring-003 PR notes now map the standard TAP SDK/CLI builders, parsers, object models, and CI coverage workflow into the commit-scoped TAP lifecycle coverage matrix.

## [`1.0.2`] - 2026-07-02

### `nexus-sdk`

#### Fixed

- event poller transaction fetching now handles resource exhausted responses by splitting batches and quarantining oversized single digests

## [`1.0.1`] - 2026-06-09

### `nexus-cli`

#### Fixed

- `nexus tool list` breaking if any tool is registered with invalid FQN

## [`1.0.0`] - 2026-04-23

### `nexus-cli`

#### Added

- tool list now shows tool timeout and prints a table for better readability

### `nexus-sdk`

#### Added

- documentation for the standardized `err` prefix for output variants that represent errors

#### Fixed

- incorrect output variant name in LLM DAG construction intro

## [`1.0.0-testnet.1`] - 2026-04-20

### `nexus-sdk`

#### Added

- Move identifiers for changing the leader status

## [`0.8.4`] - 2026-04-08

### `nexus-cli`

#### Added

- `nexus tool register offchain --from-meta <FILE|->` to register tools from a JSON metadata file or stdin, bypassing the live HTTP endpoint
- `nexus tool auth list-keys --tool-fqn <FQN>` to query registered message-signing keys for a tool
- `nexus tool auth register-key --skip-if-active` for idempotent key registration in CI pipelines

### `nexus-sdk`

#### Added

- `ToolKeyEntry` and `ToolKeyList` types in `nexus::network_auth`
- `list_tool_keys()` method on `NetworkAuthActions`

### `nexus-toolkit`

#### Added

- Built-in `--meta` flag in the `bootstrap!` macro: prints a JSON array of tool metadata to stdout and exits without starting the HTTP server

## [`0.8.3`] - 2026-04-08

### `nexus-cli`

#### Added

- `nexus gas balance` command to check the balance of an invoker's gas funds
- `nexus dag execution-cost` command to check the cost of a DAG execution

### `nexus-sdk`

#### Added

- support for gas cost and balance related commands to the `NexusClient`

## [`0.8.2`] - 2026-03-30

### `nexus-cli`

#### Added

- Sui testnet presets and automatic Nexus objects fetching

#### Fixed

- `ToolRegistry` struct to correctly deserialize timeout

## [`0.8.0`] - 2026-03-27

### `nexus-cli`

#### Added

- `nexus tool update-timeout` command to update a tool's timeout duration

### `nexus-sdk`

#### Added

- support and tests for `Static` edge kinds
- support for configurable tool timeouts
- leader stamp identifiers

#### Fixed

- issue with event poller where rpc failures would cause it to exceed the max batch size and fail to make progress
- another poller issue where the checkpoint stream would start from the first checkpoint, restreaming all events

## [`0.7.0`] - 2026-03-13

### `nexus-cli`

#### Removed

- `sui_gql_url` config field

### `nexus-sdk`

#### Changed

- transaction templates adjusted to allow for locking and finalizing gas payments
- `GasSettlementUpdate` event replaced with `GasLockUpdate`
- removed graphql client and changed event fetching to poll GRPC
- bump Sui version to `mainnet-v1.67.3`

## [`0.6.0`] - 2026-02-24

### `nexus-sdk`

#### Added

- `signed_http` feature and module for application layer HTTP request/response signatures.
- `network_auth` helpers, types, and PTB templates for tool key registration and leader allowlists.
- derived gas service identifiers and PTB templates
- `secret_store` module providing a minimal at-rest secret wrapper with optional encryption.
- `NexusClient` code for gas tickets
- support for distribution by fetching `DistributedEventWrapper` events
- `leader_registry` to `NexusObjects` and as argument to PTB templates that require it
- `InterfacePackageConfig` and `InterfaceVersionKey` type mirrors
- Support for `LeaderCapIssuedEvent`
- `bcs` support in `Crawler`

#### Removed

- X3DH+DR encryption

#### Changed

- adjusted transaction templates and events to support tools as derived objects
- removed all `Tool*` types for a unified `Tool` type that supports both offchain and onchain tools and `ToolRef` type to differentiate
- `NexusClient::workflow::execute` to work with derived gas service
- replaced `secret_core` with `secret_store` for at-rest secrets.
- bump sui version to 1.65.2
- `signed_http` is agnostic to the leader that makes the request and can be used with any leader.

#### Fixed

- bug where foreign `AnnounceInterfacePackageEvent` events could not be parsed because they did not originate from Nexus packages, which is however expected

### `nexus-cli`

#### Added

- `nexus tool auth` subcommands for key generation, tool key registration, and leader allowlist export.
- `SharedObjectRef` type to represent shared object references with mutability information
- `AnnounceInterfacePackageEvent` now has `shared_objects` field of type `Vec<SharedObjectRef>` that carries the reference type information
- added support for tagged_output in sdk
- `nexus secrets` command group for local at-rest secrets:
  - `nexus secrets status` / `enable` / `disable` / `rotate` / `wipe`
- `nexus tool auth sync-allowed-leaders` to keep tool config in sync with onchain

#### Changed

- `nexus tool list` now works with derived objects
- at-rest secret storage now auto-creates a master key in the OS keyring on first secret write (when possible); if the keyring is unavailable, it warns and writes plaintext.

#### Removed

- `nexus crypto set-passphrase` command (passphrase-based encryption).
- `nexus crypto init-key` and `nexus crypto key-status` (moved to `nexus secrets`).

### `nexus-toolkit-rust`

#### Added

- Signed HTTP runtime support with tool signing keys and leader allowlists.
- `SharedObjectRef` type to represent shared object references with mutability information
- `AnnounceInterfacePackageEvent` now has `shared_objects` field of type `Vec<SharedObjectRef>` that carries the reference type information
- added support for tagged_output in sdk

## [`0.5.0`] - 2026-01-16

### `nexus-cli`

#### Added

- `nexus scheduler` command group for on-chain task management:
  - `nexus scheduler task create` / `inspect` / `metadata` / `pause` / `resume` / `cancel`
  - `nexus scheduler occurrence add`
  - `nexus scheduler periodic set` / `disable`
- `ToolRef` to combine offchain url and onchain move module id
- add `--verbose` flag for debug log output

### `nexus-sdk`

#### Changed

- leader and crypto caps in PTB templates are now party objects
- added `ToolRegistryCreated` as tracked event
- combined some functions in tool_registry.move
- set Rust toolchain back to `stable`

## [`0.4.0`] - 2026-01-07

### `nexus-cli`

#### Added

- `--priority-fee-per-gas-unit` flag on `nexus dag execute` to forward a priority fee with DAG executions
- `nexus tool register onchain` command to register onchain tools
- onchain tool development guide
- `nexus tool new` onchain tool move template

#### Changed

- CLI now uses GRPC behind the scenes to communicate with the Sui blockchain
- CLI now uses the `EventFetcher` to fetche evens where necessary from Sui GraphQL

### `nexus-sdk`

#### Added

- support for `scheduler` transactions and events
- onchain schema generation
- `EventFetcher` under `nexus` module to fetch events from Sui GraphQL

#### Changed

- `crypto auth` now uses the new handshake algorithm
- `nexus tool register` now has two subcommands for both types of tools
- wrap large numbers as JSON strings to preserve precision for u128/u256 in nexus parser
- all identifiers and transaction templates now use new `sui-rust-sdk` types
- `NexusClient` uses GRPC client under the hood
- `ObjectCrawler` moved under `nexus` module and uses GRPC
- `onchain_schema_gen` module now uses GRPC
- all types in the SDK changed to use `sui-rust-sdk` types instead of `sui-sdk`

#### Removed

- dependency on `sui-sdk` crate in favour of `sui-rust-sdk`

## [`0.3.0`] - 2025-11-10

### `nexus-cli`

#### Fixed

- `nexus dag inspect-execution` now uses new `NexusData` implementation that supports remote storage
- `nexus dag execute` now uses new `NexusData` implementation that supports remote storage
- `nexus crypto init-key --force` wipes the old `crypto` state from config before rotating the key to avoid parsing errors

### `nexus-sdk`

#### Added

- `.nightly-version` file that specifies the Rust nightly version to use
- `nexus_sdk::nexus` module that holds `NexusClient` functionality to interact with the Nexus network
- `NexusEventKind::name` method that returns a string representation of the event kind

#### Changed

- standardized array and single value serialization of `NexusData`
- `NexusData` can now represent data stored remotely in Walrus

#### Fixed

- made faucet requests compatible with old and latest versions of the `sui-faucet`
- allow skipping the first encrypted message in a new `dh` chain

## [`0.2.0`] - 2025-08-12

### Repository

#### Added

- CONTRIBUTING.md
- CODE_OF_CONDUCT.md
- `pre-commit` hook (also in CI)

### `nexus-cli`

#### Added

- `nexus gas add-budget` command to be able to pay for evaluations
- `nexus gas expiry enable` to enable the expiry gas extension for a tool
- `nexus gas expiry disable` to disable the expiry gas extension for a tool
- `nexus gas expiry buy-ticket` to buy an expiry gas ticket for a tool
- `nexus tool set-invocation-cost` to set the invocation cost for a tool
- `indicatif` crate to handle progress spinners
- `--batch` flag to `nexus tool register` command to allow registering multiple tools at once
- upon tool registration, the CLI will save the owner caps to the CLI conf; these are then used to fall back to if no `--owner-cap` arg is provided for some commands
- `secrets` module that provides a wrapper to encrypt and decrypt its inner values
- `crypto` section to the CLI configuration to save the current state of the `crypto`
- `nexus conf set --sui.rpc-url` to set a custom Sui RPC URL for the CLI to use
- `nexus crypto auth` establishes a secure session with the network
- `nexus crypto generate-identity-key` generates and stores a fresh identity key
- `nexus crypto init-key` generates and stores a random 32-byte master key
- `nexus crypto set-passphrase` prompts for and stores a passphrase securely in the keyring
- `nexus crypto key-status` shows where the key was loaded from
- automatically fetching devnet objects for user ergonomics
- configured `cargo-deny` rules
- not failing if a tool is already registered when registering a tool
- not failing a whole tool registration batch if one of the tools fails to register
- `nexus gas limited-invocations enable` to enable the limited invocations gas extension for a tool
- `nexus gas limited-invocations disable` to disable the limited invocations gas extension for a tool
- `nexus gas limited-invocations buy-ticket` to buy a limited invocations gas ticket for a tool
- `--no-save` flag to `nexus tool register` to not save the owner caps to the CLI config

#### Changed

- JSON DAG definition no longer specifies entry input ports
- renamed JSON DAG `vertices.input_ports` to `vertices.entry_ports`
- `nexus tool list` supports the new `description` and `registered_at_ms` attributes
- tool registration now takes `invocation_cost` parameter and returns 2 owner caps `OverTool` and `OverGas`
- `nexus conf --nexus.objects` is now the only way to populate the `nexus.objects` field in the config
- `nexus conf` changed to have `set` and `get` subcommands
- `nexus dag execute` now takes `--encrypt` argument that accepts `vertex.port` pairs to encrypt before sending data on-chain
- JSON DAG now accepts `encrypted` field on `edges.[].from`
- `nexus dag execute` now encrypts any `vertex.port` mentioned in the arguments
- removed `--encrypt` flag in favour of storing the information in the JSON DAG definition
- replaced all occurrences of `sap` with `tap`

#### Removed

- automated faucet calls for gas and collateral coins
- basic auth from the CLI configuration
- DAG validation (moved to `nexus-sdk`)

#### Fixed

- `create_wallet_context` takes `SUI_RPC_URL` into consideration when checking active env
- when `nexus conf get` fails to parse the config it shows the error instead of defaulting
- `master-key` uses keyring platform specific dependencies
- `nexus crypto auth` fetches a new gas coin now

### `nexus-sdk`

#### Added

- Walrus Client module to interact with Walrus decentralized storage
- `transactions::gas` module containing PTB templates for gas-related transactions
- support for generating shell completions
- `crypto` module
- `x3dh` in `crypto` that implements X3DH key-exchange protocol according to the Signal Specs.
- `double_ratchet` in `crypto` that implements Double Ratchet with header encryption.
- `session` in `crypto` that glues together X3DH + Double Ratchet for a complete e2d Secure Session Layer.
- generic `secret` type interface for encrypting and decrypting wrapped values
- `transactions::crypto` module containing PTB templates for crypto-related transactions
- `idents::workflow::PreKeyVault` struct that contains pre key vault identifiers
- `pre_key_vault` key to `NexusObjects`
- pre key vault related Nexus events and their definitions
- DAG validation (moved from `nexus-cli`)
- `LinkedTable` support for object crawler
- added identifiers to `tool_registry`'s allow list functions

#### Changed

- `transactions::tool` register PTB template now accepts invocation cost
- all transaction templates now accept an `objects` argument instead of accepting objects one by one
- replaced all occurrences of `sap` with `tap`

#### Fixed

- `test_utils::contracts` now creates a `Move.lock` if it doesn't exist yet
- Fixed a bug that erases the current basic auth credentials from the config when any value is updated

### `nexus-toolkit-rust`

#### Added

- `/tools` endpoint to the `bootstrap!` macro that returns a list of all tools registered on the webserver

### `docs`

#### Added

- added markdown linter configuration
- added Github workflow for markdown linting and spellcheck actions
- added markdown style guide

## [`0.1.0`] - 2025-04-14

### `nexus-cli`

#### Added

- commands to validate, register, unregister and claim collateral for Nexus Tools
- commands to scaffold a new Nexus Tool
- commands to validate, publish, execute and inspect DAGs
- commands to load and save configuration
- commands to create a new Nexus network
- release workflow
- added dev guides that showcase how to use CLI to publish and register tools, and publish and execute DAGs

#### Changed

- changing the notion of entry vertices to entry input ports and adjusting parsing, validation and PTB templates in accordance

#### Fixed

- fixing tool registration, unregistration and collateral claiming based on changes in tool registry

### `nexus-toolkit-rust`

#### Added

- added basic structure for Nexus Tools written in Rust in the form of a trait
- added a macro that starts a webserver for one or multiple tools, providing all necessary endpoints
- added a first, dumb version of secret manager
- added a dev guide that goes through the steps to use CLI to scaffold a boilerplate tool and implement NexusTool trait

### `nexus-sdk`

#### Added

- added Nexus Sui identifiers module
- added `object_crawler` that parses Sui objects to structs
- added `test_utils` that handle spinning up Redis or Sui containers for testing, along with some helper functions
- added `types` module and `tool_fqn` that holds some reusable types
- added `events` module that holds definitions of Nexus events fired from Sui
- added `sui` module that holds and categorizes all `sui_sdk` types

#### Fixed

- added implicit dependencies to `test_utils`
