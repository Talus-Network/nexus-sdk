//! Ed25519 key management for browser/WASM.
//!
//! Mirrors the CLI's `nexus tool auth keygen` / key-status flow using
//! localStorage instead of the file system.  The Sui private key is stored
//! as base64 in localStorage and used by the JS-side Sui SDK to sign
//! transactions.
//!
//! This replaces the old X3DH + Double Ratchet session crypto that was
//! removed from the SDK.

use {
    base64::{engine::general_purpose::STANDARD as B64, Engine as _},
    ed25519_dalek::{SigningKey, VerifyingKey},
    rand::RngCore,
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    wasm_bindgen::prelude::*,
};

const LS_SUI_PK: &str = "nexus-sui-pk";
const LS_TOOL_SK: &str = "nexus-tool-signing-key";

// ---------------------------------------------------------------------------
// localStorage helpers
// ---------------------------------------------------------------------------

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn storage_get(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok()?
}

fn storage_set(key: &str, value: &str) -> Result<(), String> {
    local_storage()
        .ok_or("localStorage not available")?
        .set_item(key, value)
        .map_err(|_| "failed to write to localStorage".to_string())
}

fn storage_remove(key: &str) {
    if let Some(s) = local_storage() {
        let _ = s.remove_item(key);
    }
}

// ---------------------------------------------------------------------------
// JSON response helpers
// ---------------------------------------------------------------------------

fn ok_json(value: serde_json::Value) -> String {
    let mut v = value;
    v["success"] = serde_json::Value::Bool(true);
    v.to_string()
}

fn err_json(msg: &str) -> String {
    serde_json::json!({ "success": false, "error": msg }).to_string()
}

// ---------------------------------------------------------------------------
// Sui private key management  (replaces old master-key / passphrase)
// ---------------------------------------------------------------------------

/// Store a Sui Ed25519 private key (base64, hex, or Sui keytool format).
///
/// This is the web equivalent of `nexus conf set --sui.pk <BASE64>`.
#[wasm_bindgen]
pub fn set_sui_private_key(raw_key: &str) -> String {
    let raw = raw_key.trim();
    if raw.is_empty() {
        return err_json("private key must not be empty");
    }

    match parse_ed25519_private_key(raw) {
        Ok(sk) => {
            let b64 = B64.encode(sk.to_bytes());
            match storage_set(LS_SUI_PK, &b64) {
                Ok(()) => ok_json(serde_json::json!({
                    "message": "Sui private key saved to localStorage",
                    "public_key_hex": hex::encode(VerifyingKey::from(&sk).to_bytes()),
                })),
                Err(e) => err_json(&e),
            }
        }
        Err(e) => err_json(&format!("invalid key: {e}")),
    }
}

/// Get the stored Sui private key (base64).
/// JS-side Sui SDK can use this to sign transactions.
#[wasm_bindgen]
pub fn get_sui_private_key_b64() -> Option<String> {
    storage_get(LS_SUI_PK)
}

/// Return status of stored Sui key.
#[wasm_bindgen]
pub fn sui_key_status() -> String {
    match storage_get(LS_SUI_PK) {
        Some(b64) => {
            let pk_hex = B64
                .decode(&b64)
                .ok()
                .and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
                .map(|sk| {
                    let signing = SigningKey::from_bytes(&sk);
                    hex::encode(signing.verifying_key().to_bytes())
                })
                .unwrap_or_else(|| "corrupt".to_string());
            ok_json(serde_json::json!({
                "exists": true,
                "public_key_hex": pk_hex,
            }))
        }
        None => ok_json(serde_json::json!({ "exists": false })),
    }
}

/// Remove stored Sui private key.
#[wasm_bindgen]
pub fn remove_sui_private_key() -> String {
    storage_remove(LS_SUI_PK);
    ok_json(serde_json::json!({ "message": "Sui private key removed" }))
}

// ---------------------------------------------------------------------------
// Tool signing key management  (mirrors `nexus tool auth keygen`)
// ---------------------------------------------------------------------------

/// Generate a new Ed25519 tool signing key and store it.
///
/// Web equivalent of `nexus tool auth keygen`.
#[wasm_bindgen]
pub fn tool_auth_keygen(force: bool) -> String {
    if !force && storage_get(LS_TOOL_SK).is_some() {
        return err_json("tool signing key already exists; call with force=true to overwrite");
    }

    let mut sk_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut sk_bytes);
    let signing = SigningKey::from_bytes(&sk_bytes);
    let pk_hex = hex::encode(signing.verifying_key().to_bytes());
    let sk_hex = hex::encode(sk_bytes);

    match storage_set(LS_TOOL_SK, &sk_hex) {
        Ok(()) => ok_json(serde_json::json!({
            "message": "Ed25519 tool signing key generated",
            "private_key_hex": sk_hex,
            "public_key_hex": pk_hex,
        })),
        Err(e) => err_json(&e),
    }
}

/// Import an existing tool signing key (hex or base64).
#[wasm_bindgen]
pub fn tool_auth_import_key(raw_key: &str, force: bool) -> String {
    if !force && storage_get(LS_TOOL_SK).is_some() {
        return err_json("tool signing key already exists; call with force=true to overwrite");
    }

    match parse_ed25519_private_key(raw_key.trim()) {
        Ok(sk) => {
            let sk_hex = hex::encode(sk.to_bytes());
            let pk_hex = hex::encode(sk.verifying_key().to_bytes());
            match storage_set(LS_TOOL_SK, &sk_hex) {
                Ok(()) => ok_json(serde_json::json!({
                    "message": "tool signing key imported",
                    "public_key_hex": pk_hex,
                })),
                Err(e) => err_json(&e),
            }
        }
        Err(e) => err_json(&format!("invalid key: {e}")),
    }
}

/// Return status of stored tool signing key.
#[wasm_bindgen]
pub fn tool_key_status() -> String {
    match storage_get(LS_TOOL_SK) {
        Some(sk_hex) => {
            let pk_hex = hex::decode(&sk_hex)
                .ok()
                .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
                .map(|sk| hex::encode(SigningKey::from_bytes(&sk).verifying_key().to_bytes()))
                .unwrap_or_else(|| "corrupt".to_string());
            ok_json(serde_json::json!({
                "exists": true,
                "public_key_hex": pk_hex,
            }))
        }
        None => ok_json(serde_json::json!({ "exists": false })),
    }
}

/// Remove stored tool signing key.
#[wasm_bindgen]
pub fn remove_tool_signing_key() -> String {
    storage_remove(LS_TOOL_SK);
    ok_json(serde_json::json!({ "message": "tool signing key removed" }))
}

// ---------------------------------------------------------------------------
// Signed HTTP claims  (mirrors `sdk/src/signed_http/v1`)
// ---------------------------------------------------------------------------

/// Minimal signed-HTTP claims structure.
/// This is a simplified version of the SDK's `V1Claims` designed for
/// browser-side use.
#[derive(Serialize, Deserialize)]
struct SignedHttpClaims {
    version: u8,
    method: String,
    path: String,
    query: String,
    body_sha256: String,
    iat_ms: u64,
    exp_ms: u64,
    nonce: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leader_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_id: Option<String>,
}

/// Sign an HTTP request's claims blob with the stored tool signing key.
///
/// Returns JSON with `sig_input` (base64url) and `signature` (base64url)
/// that should be placed into `X-Nexus-Sig-Input` and `X-Nexus-Sig` headers.
#[wasm_bindgen]
pub fn sign_http_request(
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
    tool_id: &str,
    key_id: &str,
    ttl_ms: u64,
) -> String {
    let result = (|| -> Result<serde_json::Value, String> {
        let sk_hex =
            storage_get(LS_TOOL_SK).ok_or("no tool signing key; call tool_auth_keygen() first")?;
        let sk_bytes = hex::decode(&sk_hex).map_err(|e| format!("hex decode: {e}"))?;
        let sk_arr: [u8; 32] = sk_bytes
            .try_into()
            .map_err(|_| "corrupt tool signing key")?;
        let signing = SigningKey::from_bytes(&sk_arr);

        let now_ms = js_sys::Date::now() as u64;
        let mut nonce_bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

        let body_hash = Sha256::digest(body);

        let claims = SignedHttpClaims {
            version: 1,
            method: method.to_uppercase(),
            path: path.to_string(),
            query: query.to_string(),
            body_sha256: hex::encode(body_hash),
            iat_ms: now_ms,
            exp_ms: now_ms + ttl_ms,
            nonce: hex::encode(nonce_bytes),
            leader_id: None,
            tool_id: Some(tool_id.to_string()),
            key_id: Some(key_id.to_string()),
        };

        let claims_bytes =
            serde_json::to_vec(&claims).map_err(|e| format!("serialize claims: {e}"))?;

        use ed25519_dalek::Signer;
        let signature = signing.sign(&claims_bytes);

        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let sig_input_b64 = URL_SAFE_NO_PAD.encode(&claims_bytes);
        let sig_b64 = URL_SAFE_NO_PAD.encode(signature.to_bytes());

        Ok(serde_json::json!({
            "sig_input": sig_input_b64,
            "signature": sig_b64,
            "headers": {
                "X-Nexus-Sig-Version": "1",
                "X-Nexus-Sig-Input": sig_input_b64,
                "X-Nexus-Sig": sig_b64,
            }
        }))
    })();

    match result {
        Ok(v) => ok_json(v),
        Err(e) => err_json(&e),
    }
}

// ---------------------------------------------------------------------------
// Clear all stored crypto material
// ---------------------------------------------------------------------------

/// Wipe all Nexus crypto data from localStorage.
#[wasm_bindgen]
pub fn crypto_clear_all() -> String {
    storage_remove(LS_SUI_PK);
    storage_remove(LS_TOOL_SK);
    ok_json(serde_json::json!({ "message": "all crypto data cleared" }))
}

// ---------------------------------------------------------------------------
// Private key parsing (mirrors sdk/src/signed_http/keys.rs logic)
// ---------------------------------------------------------------------------

fn parse_ed25519_private_key(raw: &str) -> Result<SigningKey, String> {
    let raw = raw.trim();
    let raw_no_0x = raw.strip_prefix("0x").unwrap_or(raw);

    let looks_hex = raw.starts_with("0x")
        || ((raw_no_0x.len() == 64 || raw_no_0x.len() == 66)
            && raw_no_0x.chars().all(|c| c.is_ascii_hexdigit()));

    if looks_hex {
        let bytes = hex::decode(raw_no_0x).map_err(|e| format!("hex: {e}"))?;
        return signing_key_from_bytes(&bytes);
    }

    // base64 / base64url
    let try_b64 = |engine: &base64::engine::general_purpose::GeneralPurpose| -> Option<Vec<u8>> {
        engine.decode(raw.as_bytes()).ok()
    };

    use base64::engine::general_purpose::*;
    let bytes = try_b64(&STANDARD)
        .or_else(|| try_b64(&STANDARD_NO_PAD))
        .or_else(|| try_b64(&URL_SAFE))
        .or_else(|| try_b64(&URL_SAFE_NO_PAD))
        .ok_or("expected hex or base64 encoded key")?;

    signing_key_from_bytes(&bytes)
}

fn signing_key_from_bytes(bytes: &[u8]) -> Result<SigningKey, String> {
    match bytes.len() {
        32 => {
            let arr: [u8; 32] = bytes.try_into().unwrap();
            Ok(SigningKey::from_bytes(&arr))
        }
        33 => {
            if bytes[0] != 0x00 {
                return Err(format!(
                    "unsupported Sui key scheme flag 0x{:02x} (expected 0x00 for Ed25519)",
                    bytes[0]
                ));
            }
            let arr: [u8; 32] = bytes[1..].try_into().unwrap();
            Ok(SigningKey::from_bytes(&arr))
        }
        len => Err(format!("invalid key length {len}, expected 32 or 33 bytes")),
    }
}
