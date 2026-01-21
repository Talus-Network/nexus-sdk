# Tool Communication (HTTPS + Signed HTTP)

This guide explains how Nexus communicates with off-chain Tools (via its Leader nodes), why Nexus requires both **HTTPS** and **signed HTTP messages**, and what Tool developers/operators must do to deploy Tools correctly.

It is written for:

- **Tool developers** (build the Tool binary and its schemas).
- **Tool operators** (deploy the Tool and manage keys/certs).
- **Agent developers** (understand what guarantees Tool calls do and do not provide).

---

## TL;DR (what Nexus expects)

1. **Tools are HTTP servers** that expose (at minimum) `GET /health`, `GET /meta`, and `POST /invoke`.
1. **Nexus Leader nodes call Tools over HTTPS** and validate the Tool’s TLS certificate using the **system root store** (the same trust model as `curl`).
   - Your Tool must present a certificate chain that validates against standard roots.
1. **Tool invocations are signed** at the application layer (a.k.a. “signed HTTP”):
   - Leader node → Tool requests carry `X-Nexus-Sig-*` headers that authenticate the calling Leader node, bind the request metadata, and bind the request body bytes.
   - Tool → Leader node responses also carry `X-Nexus-Sig-*` headers so the Leader node can verify provenance and bind the response to the exact request.
1. **Tools should not do on-chain reads at runtime** to validate Leader nodes. Instead, the Tool operator exports a local allowlist file of permitted Leader nodes and deploys it next to the Tool.

{% hint style="info" %}
Terminology: a **Leader node** is the Nexus node calling the Tool. In signed HTTP claims, `leader_id` identifies the Leader node (a Sui like `address`) that signed the request.
{% endhint %}

---

## Terminology: “TLS termination”

Tools are plain HTTP servers. Run them behind a reverse proxy / gateway / load balancer that serves **HTTPS** on the ingress and forwards requests to the Tool over HTTP.

This setup is commonly called **TLS termination** (sometimes people informally say “HTTPS terminal”).

---

## Architecture overview

### Actors and their responsibilities

- **Leader node**
  - Discovers Tool URL from the on-chain Tool Registry.
  - Calls the Tool over HTTPS (certificate validated via system roots).
  - Signs `/invoke` requests with the Leader node message-signing key.
  - Verifies signed `/invoke` responses using the Tool message-signing public key registered on-chain.

- **Tool**
  - Runs an HTTP server that implements the Nexus Tool interface.
  - Verifies signed `/invoke` requests using a local allowlist of Leader node public keys.
  - Signs `/invoke` responses with its Tool message-signing key.

---

## Why both HTTPS and signed HTTP?

Nexus uses two layers because they solve different problems.

### HTTPS (TLS) solves transport security

HTTPS provides:

- **Confidentiality**: protects Tool inputs/outputs from passive observers.
- **Integrity**: prevents tampering in transit.
- **Server authentication**: a Leader node can verify it is talking to the expected server for `https://your-tool-domain/...`.

TLS authenticates the Tool endpoint to the Leader node. Nexus Leader nodes do not currently present client certificates when calling Tools, so Tools cannot authenticate callers at the TLS layer (no mTLS client authentication today).

{% hint style="info" %}
Nexus currently relies on system-root certificate validation and signed HTTP for Tool authentication.
In a future update, Nexus will support self-signed certificates and TLS client authentication (mTLS) for Tool communication.
{% endhint %}

### Certificate verification policy

Leader nodes validate Tool certificates using the **system root CA store** (similar to `curl`). Tool operators must:

- use a publicly trusted certificate (e.g., Let’s Encrypt, cloud managed cert)

### Signed HTTP solves identity, auditability, and replay resistance

Signed HTTP provides:

- **Strong identity binding**: “this invocation was signed by Leader node `0x...` using key id `kid`”.
- **Request/response binding**: “this response corresponds to this specific request”.
- **Body integrity at the application layer**: binds the exact body bytes via SHA-256.
- **Replay resistance**: prevents an attacker from replaying old requests, while still allowing safe retries.
- **Auditability / dispute support**: the signed claims are small and can be logged and verified later.

Signed HTTP is Nexus’ authentication mechanism for `POST /invoke`:

- Tools authenticate the calling Leader node by verifying the request signature.
- Leader nodes authenticate Tool responses by verifying the response signature (and binding it to the request).

{% hint style="info" %}
Signed HTTP does **not** guarantee that a Tool’s output is correct or safe. It only proves who signed the message and what bytes they signed. Nexus uses collateral + slashing as an after-the-fact enforcement mechanism for malicious behavior.
{% endhint %}

{% hint style="info" %}
Signed HTTP verification happens after the TLS handshake. If you want to reduce unwanted TLS handshakes/traffic, apply policy at your TLS terminator / edge (rate limiting, firewall/WAF, mTLS, or private ingress such as Cloudflare Tunnel).
{% endhint %}

---

## Signed HTTP protocol (v1) – what is signed?

Signed HTTP uses three headers on every signed request/response:

- `X-Nexus-Sig-V`: protocol version (currently `"1"`).
- `X-Nexus-Sig-Input`: base64url (no padding) of the raw JSON “claims” bytes.
- `X-Nexus-Sig`: base64url (no padding) of the 64-byte Ed25519 signature.

The signature is computed over:

- a protocol-specific **domain separator** (request vs response), and
- the exact `sig_input` bytes (the JSON-encoded claims).

This avoids fragile “HTTP canonicalization” and keeps Tool schemas unchanged: the Tool input/output remains the normal HTTP body.

### Request claims (Leader node → Tool)

The signed request claims include:

- `leader_id`, `leader_kid`: identify the calling Leader node and its active signing key id.
- `tool_id`: the Tool’s on-chain identifier (typically the Tool FQN string form).
- `iat_ms`, `exp_ms`: a validity window (milliseconds since the Unix epoch, UTC).
- `nonce`: unique token per invocation to prevent replay (UUID/random is fine).
- `method`, `path`, `query`: bind the HTTP request target.
- `body_sha256`: hex-encoded SHA-256 of the raw request body bytes.

### Response claims (Tool → Leader node)

The signed response claims include:

- `tool_id`, `tool_kid`: identify the Tool and its signing key id.
- `iat_ms`, `exp_ms`: a validity window.
- `nonce`: echo of the request nonce.
- `req_sig_input_sha256`: hex-encoded SHA-256 of the **request** `sig_input` bytes (binds response to the exact request).
- `status`: HTTP status code the Tool claims it produced.
- `body_sha256`: hex-encoded SHA-256 of the raw response body bytes.

### Replay rules (how retries stay safe)

Tools MUST track nonce usage (typically keyed by `(leader_id, nonce)`):

- If a request with the same `(leader_id, nonce)` arrives and the signed request bytes match exactly, it is a safe **retry**.
- If the same `(leader_id, nonce)` is used with a different request hash, it is a **conflicting replay** and MUST be rejected.

The Rust toolkit runtime implements this in-memory for you.

---

## Key registration and rotation (Network Auth)

Signed HTTP requires public keys to be discoverable and verifiable.

Nexus uses an on-chain **Network Auth registry** to bind identities to message-signing public keys:

- **Leader node identity**: a Sui address (the Leader node’s on-chain identifier).
- **Tool identity**: the Tool FQN (the Tool’s on-chain identifier).

Each identity has:

- a `next_key_id` counter (key id / `kid`),
- a set of keys,
- an optional active key id,
- optional metadata (e.g., description).

Registration uses a **proof-of-possession** (PoP) signature so the chain can verify the registrant actually controls the private key corresponding to the public key being registered.

---

## How to deploy a Tool (end-to-end checklist)

### 1) Implement the Tool as an HTTP server

Use `nexus-toolkit` to implement the `NexusTool` trait and bootstrap an HTTP server.

- `/health` and `/meta` stay unsigned.
- `/invoke` is the authenticated endpoint and is signed when enabled.

### 2) Put the Tool behind HTTPS (TLS termination)

The toolkit runtime is an **HTTP server**. Run it behind a reverse proxy / load balancer that provides TLS and forwards requests to the Tool over HTTP.

Common options:

- **Caddy (recommended for simplicity)** – automatic HTTPS (Let’s Encrypt): [Automatic HTTPS](https://caddyserver.com/docs/automatic-https)
- **Nginx + Certbot (Let’s Encrypt)**: [Certbot instructions](https://certbot.eff.org/instructions?ws=nginx)
- **Traefik (ACME)**: [ACME](https://doc.traefik.io/traefik/https/acme/)
- **Cloudflare (Proxy / Tunnel)**: [SSL](https://developers.cloudflare.com/ssl/) and [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)
- **Cloud load balancers** (AWS ALB/ACM, GCP HTTPS LB, etc.)

Local certificate option (useful for local testing):

- **mkcert** (local trusted dev certs): [mkcert](https://github.com/FiloSottile/mkcert)

{% hint style="warning" %}
If you use a self-signed certificate, Leader nodes will reject it unless the Nexus deployment is explicitly configured to trust your CA. Use a publicly trusted cert whenever possible.
{% endhint %}

#### TLS termination example (Caddy)

If your Tool listens on `127.0.0.1:8080` and you want to serve it at `https://tool.example.com`, a minimal `Caddyfile` looks like:

```caddyfile
tool.example.com {
  reverse_proxy 127.0.0.1:8080
}
```

Then run:

```bash
sudo caddy run --config ./Caddyfile
```

Operational notes:

- Ensure your proxy forwards `X-Nexus-Sig-*` headers (most do by default; but some “API gateways” may drop unknown headers).
- Avoid middleware that rewrites request/response bodies. The signature binds the raw body bytes.

### 3) Generate a Tool message-signing keypair

You need an Ed25519 keypair dedicated to signing Tool responses.

Using the CLI:

```bash
nexus tool tool-auth keygen --out ./tool_keypair.json
```

Store the private key securely. Treat it like an API credential.

### 4) Register the Tool in the Tool Registry (off-chain Tool)

Register the Tool URL and schema on-chain. The CLI persists the resulting OwnerCaps locally (unless `--no-save`), so you can reuse them for future operations:

```bash
nexus tool register --off-chain https://tool.example.com/ \
  --collateral-coin 0x... \
  --invocation-cost 0
```

The Tool URL should be `https://...`.

### 5) Register the Tool message-signing public key on-chain (Network Auth)

Using the CLI (requires the tool’s OwnerCap object id and a gas coin):

```bash
nexus tool tool-auth register-key \
  --tool-fqn com.example.my-tool@1 \
  --owner-cap 0x... \
  --signing-key ./tool_signing_key.hex \
  --sui-gas-coin 0x... \
  --sui-gas-budget 50000000
```

Notes:

- `--signing-key` can be a hex/base64/base64url private key string **or** a path to a file containing it.
- If `--owner-cap` is omitted, the CLI will try to use the OwnerCap saved in the CLI config for that Tool.

This creates (or updates) the Tool’s key binding in `network_auth` and returns the `tool_kid` you must configure in the Tool runtime.

### 6) Export `allowed_leaders.json` for Tool-side verification

Tools should not RPC to Sui on every request. Instead, generate a local allowlist file of permitted Leader nodes:

```bash
nexus tool tool-auth export-allowed-leaders \
  --leader 0x... \
  --leader 0x... \
  --out ./allowed_leaders.json
```

Deploy `allowed_leaders.json` next to your Tool.

### 7) Configure the toolkit runtime to require signed HTTP

Set `NEXUS_TOOLKIT_CONFIG_PATH` to point at a JSON config file that includes:

- the allowlist file (`allowed_leaders_path`), and
- the Tool signing key + `tool_kid`.

Example `toolkit.json`:

```json
{
  "version": 1,
  "invoke_max_body_bytes": 10485760,
  "signed_http": {
    "mode": "required",
    "allowed_leaders_path": "./allowed_leaders.json",
    "tools": {
      "com.example.my-tool@1": {
        "tool_kid": 0,
        "tool_signing_key": "0000000000000000000000000000000000000000000000000000000000000000"
      }
    }
  }
}
```

Then run your Tool with:

```bash
export NEXUS_TOOLKIT_CONFIG_PATH=./toolkit.json
./my-tool-binary
```

{% hint style="info" %}
If you run multiple tools on the same server, add each tool id under `signed_http.tools` with its own signing key and `tool_kid`.
{% endhint %}

---

## Optional: additional Tool-side authorization

Signed HTTP tells the Tool “this request was signed by Leader node `X`”. Tool authors may still want additional policy:

- allow only a subset of Leader nodes (beyond the allowlist file),
- add rate limiting, allow only certain task types, etc.

`nexus-toolkit` exposes an `authorize(ctx)` hook (and an `AuthContext`) for this.

---

## Troubleshooting

### Tool rejects all requests (401 auth_failed)

Common causes:

- Reverse proxy is stripping `X-Nexus-Sig-*` headers.
- Tool has the wrong `allowed_leaders.json` file (missing Leader node key).
- Clock skew is too high (Tool host time is wrong).
- `tool_id` mismatch (Tool is configured for a different Tool FQN / id).

### Leader node rejects Tool responses

Common causes:

- Tool is using the wrong signing key or `tool_kid`.
- Tool key is not registered (or not the active key) on-chain.
- Tool response body is being modified by middleware.

### TLS / certificate errors from Leader nodes

Common causes:

- Certificate is self-signed or missing intermediate chain.
- Certificate hostname does not match the Tool URL.
- Leader node environment does not include the required root CA (custom CA deployments).
