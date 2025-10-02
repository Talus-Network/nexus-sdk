// Bring Base64 trait into scope for Engine::encode/decode methods
use {
    base64::{self, Engine as _},
    js_sys,
    nexus_sdk::crypto::{
        secret_bytes::SecretBytes,
        session::{Message, Session},
        x3dh::{IdentityKey, PreKeyBundle},
    },
    rand::{self, RngCore},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
    web_sys::console,
    x25519_dalek,
};

// Helper macro for console logging in WASM
macro_rules! console_log {
    ($($t:tt)*) => (console::log_1(&format!($($t)*).into()));
}

// Storage for sessions - using localStorage for persistence like CLI config
thread_local! {
    static SESSIONS: std::cell::RefCell<HashMap<[u8; 32], Session>> = std::cell::RefCell::new(HashMap::new()); // CLI-parity: Use [u8; 32] as key
    static IDENTITY_KEYS: std::cell::RefCell<HashMap<String, String>> = std::cell::RefCell::new(HashMap::new());
}

/// Key status/result structure as JSON for parity with CLI key-status
#[wasm_bindgen]
pub fn key_status() -> String {
    // JS side stores the encrypted master key in localStorage under 'nexus-master-key'
    // Here we only report presence; actual secure check is delegated to JS.
    let window = web_sys::window();
    if let Some(win) = window {
        if let Ok(Some(storage)) = win.local_storage() {
            if let Ok(Some(_val)) = storage.get_item("nexus-master-key") {
                return serde_json::json!({
                    "exists": true,
                    "storage": "localStorage+AES-GCM",
                })
                .to_string();
            }
        }
    }
    serde_json::json!({
        "exists": false,
        "storage": "localStorage+AES-GCM",
    })
    .to_string()
}

/// CLI-compatible key init behavior: matches CLI crypto_init_key exactly
#[wasm_bindgen]
pub fn key_init(force: bool) -> String {
    console_log!("🔍 WASM key_init: Starting with force={}", force);
    console_log!("Generating and storing a new 32-byte master key");

    // 1. Check for existing keys (like CLI)
    let status = key_status();
    let exists = serde_json::from_str::<serde_json::Value>(&status)
        .ok()
        .and_then(|v| v.get("exists").and_then(|e| e.as_bool()))
        .unwrap_or(false);

    console_log!("🔍 WASM key_init: Existing key check - exists={}", exists);

    if exists && !force {
        console_log!("🔍 WASM key_init: Key already exists and force=false, aborting (CLI-parity)");
        return serde_json::json!({
            "success": false,
            "error": "KeyAlreadyExists",
            "message": "A different persistent key already exists; re-run with --force if you really want to replace it",
            "requires_force": true,
            "cli_compatible": true
        })
        .to_string();
    }

    console_log!("🔍 WASM key_init: Key check passed, proceeding with generation (CLI-parity)");

    // 2. Generate new 32-byte key (like CLI)
    let master_key_hex = generate_random_master_key();

    console_log!(
        "🔍 WASM key_init: Generated {} hex chars key",
        master_key_hex.len()
    );
    console_log!("🔍 WASM key_init: Key hex: {}", master_key_hex);
    console_log!(
        "🔍 WASM key_init: Key preview: {}...",
        &master_key_hex[..16]
    );

    console_log!("🔍 WASM key_init: Successfully generated key (CLI-parity)");

    serde_json::json!({
        "success": true,
        "action": "store_key",
        "master_key": master_key_hex,
        "message": if force { "Overwriting existing key (CLI-parity)" } else { "Creating new key (CLI-parity)" },
        "cli_compatible": true,
        "key_length": 32,
        "key_length_hex": 64
    })
    .to_string()
}

/// Convert bytes to hex string using hex crate
fn bytes_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Convert hex string to bytes using hex crate
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    hex::decode(hex).map_err(|e| format!("Hex decode error: {}", e))
}

/// Real X3DH session initiation with peer bundle (CLI-compatible)
#[wasm_bindgen]
pub fn initiate_x3dh_session(master_key_hex: &str, peer_bundle_bytes: &[u8]) -> String {
    console_log!("Initiating X3DH session with peer bundle...");

    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        // Parse master key
        if master_key_hex.len() != 64 {
            return Err("Master key must be 64 hex characters".into());
        }

        let master_key_bytes = hex_to_bytes(master_key_hex)?;
        if master_key_bytes.len() != 32 {
            return Err("Master key must be exactly 32 bytes".into());
        }

        // Generate identity key randomly (CLI-compatible)
        let identity_key = IdentityKey::generate();

        console_log!("Generated identity key from master key");

        // Store identity key for session management
        let identity_key_hex = bytes_to_hex(&identity_key.dh_public.to_bytes());

        // Store identity key hex in WASM memory (equivalent to CLI config)
        IDENTITY_KEYS.with(|keys| {
            keys.borrow_mut().clear(); // Clear any previous keys
            keys.borrow_mut()
                .insert("current".to_string(), identity_key_hex.clone());
        });

        // If peer bundle is provided, try to deserialize and initiate X3DH
        if !peer_bundle_bytes.is_empty() {
            console_log!(
                "Attempting to deserialize peer bundle ({} bytes)...",
                peer_bundle_bytes.len()
            );

            // Deserialize pre-key bundle from on-chain bytes (like CLI)
            let peer_bundle: PreKeyBundle = bincode::deserialize(peer_bundle_bytes)
                .map_err(|e| format!("Failed to deserialize PreKeyBundle: {}", e))?;

            console_log!("Successfully deserialized peer bundle");

            // Run X3DH with "nexus auth" message (exactly like CLI)
            let first_message = b"nexus auth";
            let (initial_msg, session) =
                Session::initiate(&identity_key, &peer_bundle, first_message)
                    .map_err(|e| format!("X3DH initiate failed: {}", e))?;

            console_log!("X3DH session initiated successfully");

            // Extract InitialMessage from Message enum (like CLI)
            let initial_message = match initial_msg {
                Message::Initial(msg) => msg,
                _ => return Err("Expected Initial message from session initiation".into()),
            };

            // Serialize for storage/transport (but not for transaction)
            let initial_message_bytes = bincode::serialize(&initial_message)
                .map_err(|e| format!("InitialMessage serialize failed: {}", e))?;

            // Store session and get session ID (like CLI)
            let session_id = *session.id(); // CLI-parity: Use [u8; 32] directly
            let session_id_hex = bytes_to_hex(&session_id); // For display only

            console_log!("🔍 WASM: Session ID format: {:?}", session_id);
            console_log!("🔍 WASM: Session ID hex: {}", session_id_hex);
            console_log!("🔍 WASM: Session ID length: {} bytes", session_id.len());

            SESSIONS.with(|sessions| {
                sessions.borrow_mut().insert(session_id, session); // CLI-parity: Use [u8; 32] key
            });

            console_log!("Session stored with ID: {}", session_id_hex);

            let response = serde_json::json!({
                "success": true,
                "session_id": session_id_hex,
                "identity_key": identity_key_hex,
                "initial_message_bytes": initial_message_bytes,
                "initial_message_b64": base64::engine::general_purpose::STANDARD.encode(&initial_message_bytes),
                "message": "X3DH session created successfully"
            });

            Ok(response.to_string())
        } else {
            return Err("Peer bundle required for X3DH session initiation".into());
        }
    })();

    match result {
        Ok(response) => response,
        Err(e) => {
            console_log!("X3DH session initiation error: {}", e);
            serde_json::json!({
                "success": false,
                "error": e.to_string()
            })
            .to_string()
        }
    }
}

/// Validate master key format
#[wasm_bindgen]
pub fn validate_master_key(master_key_hex: &str) -> String {
    let result = if master_key_hex.len() != 64 {
        serde_json::json!({
            "valid": false,
            "length": master_key_hex.len(),
            "message": format!("Invalid length: expected 64 hex characters, got {}", master_key_hex.len())
        })
    } else if master_key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        serde_json::json!({
            "valid": true,
            "length": 64,
            "message": "Valid 32-byte master key"
        })
    } else {
        serde_json::json!({
            "valid": false,
            "message": "Invalid hex format: contains non-hex characters"
        })
    };

    result.to_string()
}

/// Generate a random master key using CLI-compatible logic
#[wasm_bindgen]
pub fn generate_random_master_key() -> String {
    // CLI-parity: Use OsRng like CLI does
    let mut key = [0u8; 32]; // KEY_LEN = 32 bytes like CLI
    rand::rngs::OsRng.fill_bytes(&mut key);

    // Convert to hex like CLI does
    let hex_key = bytes_to_hex(&key);
    console_log!(
        "🔍 WASM generate_random_master_key: Generated master key (hex): {}",
        hex_key
    );

    hex_key
}

/// Check if master key exists in localStorage (placeholder - will be called from JS)
#[wasm_bindgen]
pub fn check_master_key_status() -> String {
    serde_json::json!({
        "status": "check_required_from_js",
        "message": "Master key status should be checked from JavaScript localStorage"
    })
    .to_string()
}

/// Get current session count
#[wasm_bindgen]
pub fn get_session_count() -> usize {
    SESSIONS.with(|sessions| sessions.borrow().len())
}

/// Export sessions for secure localStorage persistence (CLI-compatible)
#[wasm_bindgen]
pub fn export_sessions_for_storage() -> Option<String> {
    SESSIONS.with(|sessions| {
        let sessions_ref = sessions.borrow();
        if sessions_ref.is_empty() {
            console_log!("⚠️ No sessions to export");
            return None;
        }

        console_log!(
            "📤 Exporting {} sessions for localStorage",
            sessions_ref.len()
        );

        // Export sessions with serialized session data (bincode + base64)
        let sessions_data: std::collections::HashMap<String, serde_json::Value> = sessions_ref
            .iter()
            .map(|(session_id, session)| {
                let session_id_hex = bytes_to_hex(session_id); // CLI-parity: Convert [u8; 32] to hex
                console_log!("📤 Exporting session: {}", session_id_hex);

                let session_bytes = match bincode::serialize(session) {
                    Ok(bytes) => {
                        console_log!(
                            "✅ Session {} serialized: {} bytes",
                            session_id_hex,
                            bytes.len()
                        );
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    }
                    Err(e) => {
                        console_log!("❌ Failed to serialize session {}: {}", session_id_hex, e);
                        String::new()
                    }
                };

                (
                    session_id_hex.clone(), // Use hex string as key for JS compatibility
                    serde_json::json!({
                        "session_id": session_id_hex,
                        "session_id_bytes": session_id.to_vec(), // Store original bytes too
                        "session_data": session_bytes,
                        "created_timestamp": js_sys::Date::now() as u64,
                        "session_type": "x3dh_session",
                        "requires_encryption": true
                    }),
                )
            })
            .collect();

        let json_string = serde_json::to_string(&sessions_data);
        match json_string {
            Ok(json) => {
                console_log!(
                    "✅ Successfully exported {} sessions to JSON ({} chars)",
                    sessions_data.len(),
                    json.len()
                );
                Some(json)
            }
            Err(e) => {
                console_log!("❌ Failed to serialize sessions to JSON: {}", e);
                None
            }
        }
    })
}

/// Import sessions from localStorage with full restoration (CLI-compatible)
#[wasm_bindgen]
pub fn import_sessions_from_storage(sessions_json: &str) -> String {
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let sessions_data: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(sessions_json)?;

        console_log!("📥 Importing {} sessions from storage", sessions_data.len());

        let mut imported_count = 0usize;
        let mut failed_count = 0usize;

        SESSIONS.with(|sessions| {
            // Clear existing sessions first
            sessions.borrow_mut().clear();

            for (session_id_hex, session_info) in sessions_data.iter() {
                // CLI-parity: Convert hex string back to [u8; 32]
                let session_id_bytes = match hex_to_bytes(session_id_hex) {
                    Ok(bytes) => {
                        if bytes.len() != 32 {
                            console_log!(
                                "⚠️ Invalid session ID length for {}: {} bytes",
                                session_id_hex,
                                bytes.len()
                            );
                            failed_count += 1;
                            continue;
                        }
                        let mut session_id = [0u8; 32];
                        session_id.copy_from_slice(&bytes);
                        session_id
                    }
                    Err(e) => {
                        console_log!(
                            "⚠️ Failed to parse session ID hex {}: {}",
                            session_id_hex,
                            e
                        );
                        failed_count += 1;
                        continue;
                    }
                };

                // Check if this session has serialized data
                if let Some(session_data_b64) =
                    session_info.get("session_data").and_then(|v| v.as_str())
                {
                    if !session_data_b64.is_empty() {
                        match base64::engine::general_purpose::STANDARD
                            .decode(session_data_b64)
                            .ok()
                            .and_then(|bytes| bincode::deserialize::<Session>(&bytes).ok())
                        {
                            Some(session) => {
                                sessions.borrow_mut().insert(session_id_bytes, session);
                                imported_count += 1;
                                console_log!("✅ Imported session: {}", session_id_hex);
                            }
                            None => {
                                console_log!("⚠️ Failed to restore session {}", session_id_hex);
                                failed_count += 1;
                            }
                        }
                    } else {
                        console_log!("⚠️ Empty session data for {}", session_id_hex);
                        failed_count += 1;
                    }
                } else {
                    console_log!(
                        "⚠️ Session {} has no serialized data, skipping",
                        session_id_hex
                    );
                    failed_count += 1;
                }
            }
        });

        Ok(serde_json::json!({
            "success": true,
            "imported_sessions": imported_count,
            "failed_sessions": failed_count,
            "total_sessions": sessions_data.len(),
            "message": format!("Successfully imported {} out of {} sessions", imported_count, sessions_data.len())
        }).to_string())
    })();

    match result {
        Ok(response) => response,
        Err(e) => {
            console_log!("Session import error: {}", e);
            serde_json::json!({
                "success": false,
                "error": e.to_string()
            })
            .to_string()
        }
    }
}

/// Clear all sessions
#[wasm_bindgen]
pub fn clear_all_sessions() -> String {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let count = sessions.len();
        sessions.clear();

        serde_json::json!({
            "status": "cleared",
            "sessions_cleared": count
        })
        .to_string()
    })
}

/// Get active session for DAG execution (CLI-compatible)
#[wasm_bindgen]
pub fn get_active_session_for_execution(_master_key_hex: &str) -> String {
    console_log!("🔍 Looking for active session for DAG execution...");

    SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();

        console_log!("🔍 Total sessions in WASM memory: {}", sessions.len());

        // Try to find any session (like CLI's approach)
        if sessions.is_empty() {
            console_log!("❌ No sessions found in WASM memory");
            return serde_json::json!({
                "success": false,
                "error": "Authentication required — crypto auth must be completed first",
                "requires_auth": true,
                "sessions_count": 0
            })
            .to_string();
        }

        // Get the first available session (CLI takes first available)
        if let Some((session_id, _session)) = sessions.iter().next() {
            let session_id_hex = bytes_to_hex(session_id); // CLI-parity: Convert to hex for display
            console_log!("✅ Found active session: {}", session_id_hex);

            return serde_json::json!({
                "success": true,
                "session_id": session_id_hex,
                "session_id_bytes": session_id.to_vec(), // Store original bytes too
                "message": "Active session found for execution",
                "ready_for_encryption": true,
                "sessions_count": sessions.len()
            })
            .to_string();
        }

        console_log!("❌ No sessions available (unexpected)");
        serde_json::json!({
            "success": false,
            "error": "No active sessions available",
            "requires_auth": true,
            "sessions_count": sessions.len()
        })
        .to_string()
    })
}

/// Encrypt input data using active session (CLI-compatible)
#[wasm_bindgen]
pub fn encrypt_entry_ports_with_session(
    _master_key_hex: &str,
    input_json: &str,
    encrypted_ports_json: &str,
) -> String {
    console_log!("Encrypting entry ports with active session...");

    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        // Parse inputs
        let mut input_data: serde_json::Value = serde_json::from_str(input_json)?;
        let encrypted_ports: std::collections::HashMap<String, Vec<String>> =
            serde_json::from_str(encrypted_ports_json)?;

        if encrypted_ports.is_empty() {
            // No encryption needed
            return Ok(serde_json::json!({
                "success": true,
                "input_data": input_data,
                "encrypted_count": 0,
                "message": "No encrypted ports, input data unchanged"
            })
            .to_string());
        }

        // Find active session
        let session_result = SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();

            if sessions.is_empty() {
                return Err("No active sessions available".to_string());
            }

            // Get first available session (CLI-parity: mutable reference)
            let (session_id, session) = sessions
                .iter_mut()
                .next()
                .ok_or("No sessions available for encryption")?;
            let session_id_hex = bytes_to_hex(session_id); // CLI-parity: Convert to hex for display
            console_log!("Using session {} for encryption", session_id_hex);

            let mut encrypted_count = 0;

            // Encrypt each target port (like CLI encrypt_entry_ports_once)
            for (vertex, ports) in &encrypted_ports {
                for port in ports {
                    if let Some(slot) = input_data.get_mut(vertex).and_then(|v| v.get_mut(port)) {
                        let plaintext = slot.take();
                        let bytes = serde_json::to_vec(&plaintext)
                            .map_err(|e| format!("JSON serialization failed: {}", e))?;

                        // Encrypt using session (CLI-parity: mutable session)
                        let msg = session
                            .encrypt(&bytes)
                            .map_err(|e| format!("Encryption failed: {}", e))?;

                        // Extract StandardMessage like CLI
                        let Message::Standard(pkt) = msg else {
                            return Err("Session returned non-standard packet".to_string());
                        };

                        // Serialize with bincode exactly like the CLI implementation
                        let serialized = bincode::serialize(&pkt)
                            .map_err(|e| format!("Bincode serialization failed: {}", e))?;

                        *slot = serde_json::to_value(&serialized)
                            .map_err(|e| format!("Value serialization failed: {}", e))?;
                        encrypted_count += 1;

                        console_log!("Encrypted {}.{}", vertex, port);
                    }
                }
            }

            // CLI-parity: Commit session state (exactly like CLI)
            session.commit_sender(None);
            console_log!("Session state committed (CLI-parity)");

            Ok(serde_json::json!({
                "success": true,
                "input_data": input_data,
                "encrypted_count": encrypted_count,
                "message": format!("Successfully encrypted {} ports", encrypted_count)
            }))
        });

        match session_result {
            Ok(result) => Ok(result.to_string()),
            Err(e) => Err(e.into()),
        }
    })();

    match result {
        Ok(response) => response,
        Err(e) => {
            console_log!("Encryption error: {}", e);
            serde_json::json!({
                "success": false,
                "error": e.to_string()
            })
            .to_string()
        }
    }
}

/// Comprehensive peer bundle validation and analysis
#[wasm_bindgen]
pub fn validate_peer_bundle_comprehensive(peer_bundle_bytes: &[u8]) -> String {
    console_log!("=== Peer Bundle Validation ===");

    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        if peer_bundle_bytes.is_empty() {
            return Err("Peer bundle is empty".into());
        }

        console_log!("Peer bundle size: {} bytes", peer_bundle_bytes.len());

        // Try to deserialize the peer bundle
        let peer_bundle: PreKeyBundle = match bincode::deserialize(peer_bundle_bytes) {
            Ok(bundle) => {
                console_log!("✅ Successfully deserialized peer bundle");
                bundle
            }
            Err(e) => {
                console_log!("❌ Failed to deserialize peer bundle: {}", e);
                return Err(format!("Deserialization failed: {}", e).into());
            }
        };

        // Analyze bundle structure
        let bundle_analysis = serde_json::json!({
            "spk_id": peer_bundle.spk_id,
            "has_otpk": peer_bundle.otpk_id.is_some(),
            "otpk_id": peer_bundle.otpk_id,
            "identity_pk_hex": bytes_to_hex(&peer_bundle.identity_pk.to_bytes()),
            "spk_pub_hex": bytes_to_hex(&peer_bundle.spk_pub.to_bytes()),
            "identity_verify_bytes_hex": bytes_to_hex(&peer_bundle.identity_verify_bytes),
            "spk_sig_hex": bytes_to_hex(&peer_bundle.spk_sig)
        });

        console_log!("Bundle analysis: {}", bundle_analysis.to_string());

        // Validate SPK signature
        let spk_valid = peer_bundle.verify_spk();
        console_log!("SPK signature valid: {}", spk_valid);

        if !spk_valid {
            return Err("SPK signature verification failed".into());
        }

        // Test X3DH with a dummy identity key
        console_log!("Testing X3DH with dummy identity key...");
        let dummy_identity = IdentityKey::generate();
        let test_message = b"test";

        match Session::initiate(&dummy_identity, &peer_bundle, test_message) {
            Ok((initial_msg, session)) => {
                console_log!("✅ X3DH test successful");

                let initial_message_bytes = match initial_msg {
                    Message::Initial(msg) => bincode::serialize(&msg)
                        .map_err(|e| format!("InitialMessage serialize failed: {}", e))?,
                    _ => return Err("Expected Initial message from session initiation".into()),
                };

                let session_id_hex = bytes_to_hex(session.id());

                Ok(serde_json::json!({
                    "success": true,
                    "bundle_valid": true,
                    "spk_signature_valid": true,
                    "x3dh_test_successful": true,
                    "bundle_analysis": bundle_analysis,
                    "test_session_id": session_id_hex,
                    "test_initial_message_size": initial_message_bytes.len(),
                    "message": "Peer bundle is valid and ready for authentication"
                })
                .to_string())
            }
            Err(e) => {
                console_log!("❌ X3DH test failed: {}", e);
                Err(format!("X3DH test failed: {}", e).into())
            }
        }
    })();

    match result {
        Ok(response) => response,
        Err(e) => {
            console_log!("❌ Peer bundle validation error: {}", e);
            serde_json::json!({
                "success": false,
                "bundle_valid": false,
                "error": e.to_string(),
                "message": "Peer bundle validation failed"
            })
            .to_string()
        }
    }
}

/// Test the complete crypto auth flow with a generated peer bundle
#[wasm_bindgen]
pub fn test_crypto_auth_flow(master_key_hex: &str) -> String {
    console_log!("=== Testing Complete Crypto Auth Flow ===");

    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        // Validate master key
        if master_key_hex.len() != 64 {
            return Err("Master key must be 64 hex characters".into());
        }

        if !master_key_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Master key contains invalid hex characters".into());
        }

        console_log!("✅ Master key validation passed");

        // Generate identity key from master key
        let master_key_bytes = hex_to_bytes(master_key_hex)?;
        let secret_bytes = SecretBytes(master_key_bytes.clone().try_into().unwrap());
        let static_secret = secret_bytes.into();
        let identity_key = IdentityKey::from_secret(static_secret);

        console_log!("✅ Identity key generated");

        // Generate a test peer bundle (like a network would)
        console_log!("Generating test peer bundle...");
        let test_receiver_id = IdentityKey::generate();
        let spk_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let otpk_secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let bundle = PreKeyBundle::new(
            &test_receiver_id,
            1,
            &spk_secret,
            Some(1),
            Some(&otpk_secret),
        );

        console_log!("✅ Test peer bundle generated");

        // Serialize the bundle
        let bundle_bytes = bincode::serialize(&bundle)
            .map_err(|e| format!("Failed to serialize test bundle: {}", e))?;

        console_log!(
            "✅ Test peer bundle serialized ({} bytes)",
            bundle_bytes.len()
        );

        // Test the X3DH flow directly (without network calls)
        console_log!("Testing X3DH flow directly...");

        // Deserialize the test bundle
        let peer_bundle: PreKeyBundle = bincode::deserialize(&bundle_bytes)
            .map_err(|e| format!("Failed to deserialize test bundle: {}", e))?;

        // Validate the peer bundle
        if !peer_bundle.verify_spk() {
            return Err("Invalid test bundle: SPK signature verification failed".into());
        }

        // Run X3DH session initiation
        let first_message = b"nexus auth";
        let (_initial_msg, session) = Session::initiate(&identity_key, &peer_bundle, first_message)
            .map_err(|e| format!("X3DH initiate failed: {}", e))?;

        console_log!("✅ X3DH session initiated");

        // Create test format InitialMessage (like CLI test)
        use x25519_dalek::PublicKey;
        let initial_message = nexus_sdk::crypto::x3dh::InitialMessage {
            ika_pub: PublicKey::from([0; 32]), // 32 zero bytes like CLI test
            ek_pub: PublicKey::from([0; 32]),  // 32 zero bytes like CLI test
            spk_id: 1,
            otpk_id: Some(1),
            nonce: [0; 24],          // 24 zero bytes like CLI test
            ciphertext: vec![0; 32], // 32 zero bytes like CLI test
        };

        let initial_message_bytes = bincode::serialize(&initial_message)
            .map_err(|e| format!("InitialMessage serialize failed: {}", e))?;

        // Store session
        let session_id = *session.id(); // CLI-parity: Use [u8; 32] directly
        let session_id_hex = bytes_to_hex(&session_id); // For display only

        SESSIONS.with(|sessions| {
            sessions.borrow_mut().insert(session_id, session); // CLI-parity: Use [u8; 32] key
        });

        // Auto-save session to localStorage
        if let Some(sessions_json) = export_sessions_for_storage() {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(()) = storage.set_item("nexus-sessions", &sessions_json) {
                        console_log!("✅ Test session auto-saved to localStorage");
                    }
                }
            }
        }

        console_log!("✅ Test flow completed");

        // Create test result
        let result_json = serde_json::json!({
            "success": true,
            "session_id": session_id_hex,
            "identity_key": bytes_to_hex(&identity_key.dh_public.to_bytes()),
            "initial_message": {
                "bytes": initial_message_bytes,
                "base64": base64::engine::general_purpose::STANDARD.encode(&initial_message_bytes),
                "length": initial_message_bytes.len()
            },
            "initial_message_struct": {
                "ika_pub": bytes_to_hex(&initial_message.ika_pub.to_bytes()),
                "ek_pub": bytes_to_hex(&initial_message.ek_pub.to_bytes()),
                "spk_id": initial_message.spk_id,
                "otpk_id": initial_message.otpk_id,
                "nonce": bytes_to_hex(&initial_message.nonce),
                "ciphertext": bytes_to_hex(&initial_message.ciphertext)
            },
            "test_mode": true,
            "test_bundle_size": bundle_bytes.len(),
            "test_receiver_id": bytes_to_hex(&test_receiver_id.dh_public.to_bytes()),
            "flow_status": "ready_for_execution",
            "message": "Crypto auth flow test completed successfully - ready for transaction execution"
        });

        Ok(result_json.to_string())
    })();

    match result {
        Ok(response) => {
            console_log!("=== Test Completed Successfully ===");
            response
        }
        Err(e) => {
            console_log!("❌ Test failed: {}", e);
            serde_json::json!({
                "success": false,
                "test_mode": true,
                "error": e.to_string(),
                "message": "Crypto auth flow test failed"
            })
            .to_string()
        }
    }
}
