# Verify an offchain tool result

This guide is for tool developers and workflow authors who need to understand when an offchain tool result is accepted, when it needs verifier evidence, and how to inspect the result path. It explains the current verifier model from the verification concept page, [Tool communication](tool-communication.md), `sui/workflow/sources/execution_verification.move`, `sui/workflow/sources/execution_submission.move`, and the demo verifier contract.

## How verifier evidence is structured

```mermaid
%% Declares the evidence path for offchain tool verification; see `sui/workflow/sources/execution_verification.move`.
sequenceDiagram
  %% A leader invocation produces request transcript evidence.
  participant Leader as Leader HTTP invocation
  %% The tool response contributes body, headers, and optional signed HTTP evidence.
  participant Tool as Tool response
  %% Request and response evidence is the proof input to workflow.
  participant Evidence as Request and response evidence
  %% NetworkAuth stores registered key bindings for proof checks.
  participant NetworkAuth as network_auth key binding
  %% VerifierRegistry and verifier packages implement external verifier paths.
  participant Verifier as VerifierRegistry and verifier package
  %% Workflow verification normalizes the selected proof path.
  participant Workflow as execution_verification
  %% The verdict accepts output or classifies invalid proof.
  participant Verdict as Accept result or classify invalid proof
  %% A leader invocation produces request transcript evidence.
  Leader->>Evidence: Record request transcript
  %% The tool response contributes body, headers, and optional signed HTTP evidence.
  Tool->>Evidence: Add response body, headers, and signature evidence
  %% Workflow chooses the proof path configured on the DAG or tool verifier method.
  alt registered-key verifier path
    %% Registered-key verification uses leader or tool key bindings from network_auth.
    Evidence->>NetworkAuth: Resolve registered key binding
    %% Registered-key proof is normalized by workflow verification.
    NetworkAuth-->>Workflow: Return key-backed proof result
  %% This branch calls an external verifier package.
  else tool verifier contract path
    %% External verifier verification calls a registered verifier package.
    Evidence->>Verifier: Call registered verifier package
    %% External verifier output is normalized by workflow verification.
    Verifier-->>Workflow: Return verifier outcome
  %% This line closes the configured verifier-path branch.
  end
  %% Workflow either accepts the result or classifies the invalid proof path.
  Workflow->>Verdict: Accept result or classify invalid proof
```

## How verification runs

```mermaid
%% Declares the runtime verification sequence for an offchain result.
sequenceDiagram
  %% `L` is the leader process invoking the offchain tool.
  participant L as Leader
  %% `T` is the HTTP tool server.
  participant T as Tool
  %% `W` is workflow submission and verification onchain code.
  participant W as Workflow
  %% `V` is an optional external verifier Move package.
  participant V as Verifier package
  %% The leader sends the tool request, optionally signed with signed HTTP headers.
  L->>T: POST /invoke with optional signed HTTP request
  %% The tool returns JSON output, optionally signed with response headers.
  T-->>L: JSON response with optional signed HTTP response
  %% The leader submits output and proof evidence to workflow.
  L->>W: submit output plus verifier proof envelope
  %% This branch runs when the DAG/tool config requires an external verifier.
  alt external verifier configured
    %% Workflow calls the verifier package with the offchain evidence.
    W->>V: verify_offchain_result evidence
    %% The verifier returns a worksheet-stamped contract result.
    V-->>W: VerifierContractResult stamped on worksheet
  %% This line closes the external-verifier branch.
  end
  %% Workflow accepts, rejects, or classifies the committed result based on the proof.
  W-->>L: accepted committed result or invalid proof rejection
```

## Configure transport evidence

For HTTP tools, start with the transport contract in [Tool communication](tool-communication.md). The leader supports signed HTTP modes through environment variables such as:

```sh
# Require signed HTTP for leader/tool communication; valid modes are documented in `docs/guides/tool-communication.md`.
EXECUTOR_SIGNED_HTTP_MODE=required
# Provide the leader's Ed25519 private key as base64; generate and register the matching public key through `network_auth`.
EXECUTOR_SIGNED_HTTP_SIGNING_KEY=base64_ed25519_private_key_for_leader
# Select the active leader signing key ID registered onchain for this leader identity.
EXECUTOR_SIGNED_HTTP_LEADER_KID=1
```

This is not enough by itself to prove a workflow result onchain. Signed HTTP gives the leader request/response material and key identity; the workflow still accepts or rejects according to the verifier configuration attached to the DAG/tool path.

## Inspect tool and execution state

Use these commands for operational visibility:

```sh
# Inspect the tool registration and verifier configuration by FQN.
nexus tool inspect --tool-fqn "$tool_fqn" --json
# Inspect the workflow execution state that should contain committed, failed, or pending verification output.
nexus dag inspect-execution --json --dag-execution-id "$execution_id"
```

For offchain verifier debugging, use the same inspection pattern to confirm the tool registration, the execution state, and whether the result reached a committed or failed branch.

## Understand the verifier contract shape

The demo verifier source implements a minimal external verifier:

```move
// `public fun` exposes the verifier entry point that the registered verifier method calls.
public fun verify_offchain_result(
    // `self` is the verifier package state object registered in `VerifierRegistry`.
    self: &mut DemoVerifier,
    // `worksheet` is the proof object workflow expects the verifier to stamp before returning.
    mut worksheet: ProofOfUID,
    // `evidence` carries the offchain request/response transcript submitted by the leader.
    evidence: OffchainVerifierEvidence,
// The function returns the stamped worksheet plus the normalized verifier contract result.
): (ProofOfUID, VerifierContractResult)
```

It reads the offchain request/response evidence, accepts 2xx responses except a demo rejection vertex, creates a `VerifierContractResult`, stamps the worksheet, and returns both. The workflow then consumes the stamped worksheet and normalized result.

## Failure classes

`execution_verification.move` classifies failed verification into invalid leader proof or invalid tool proof. Leader registered-key proof checks leader-side identity and transcript evidence. Tool verifier-contract proof checks the external verifier result and credential binding. If neither side requires proof, the workflow can proceed without a verifier proof envelope, but result shape and worksheet facts still have to match.
