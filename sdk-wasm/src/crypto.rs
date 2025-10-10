// Bring Base64 trait into scope for Engine::encode/decode methods
use {
    base64::{self, Engine as _},
    js_sys,
    nexus_sdk::crypto::{
        session::{Message, Session},
        x3dh::{IdentityKey, PreKeyBundle},
    },
    rand::{self, RngCore},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
};

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
    // 1. Check for existing keys (like CLI)
    let status = key_status();
    let exists = serde_json::from_str::<serde_json::Value>(&status)
        .ok()
        .and_then(|v| v.get("exists").and_then(|e| e.as_bool()))
        .unwrap_or(false);

    if exists && !force {
        return serde_json::json!({
            "success": false,
            "error": "KeyAlreadyExists",
            "message": "A different persistent key already exists; re-run with --force if you really want to replace it",
            "requires_force": true,
            "cli_compatible": true
        })
        .to_string();
    }

    // 2. Generate new 32-byte key (like CLI)
    let master_key_hex = generate_random_master_key();

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
            // Deserialize pre-key bundle from on-chain bytes (like CLI)
            let peer_bundle: PreKeyBundle = bincode::deserialize(peer_bundle_bytes)
                .map_err(|e| format!("Failed to deserialize PreKeyBundle: {}", e))?;

            // Run X3DH with "nexus auth" message (exactly like CLI)
            let first_message = b"nexus auth";
            let (initial_msg, session) =
                Session::initiate(&identity_key, &peer_bundle, first_message)
                    .map_err(|e| format!("X3DH initiate failed: {}", e))?;

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

            SESSIONS.with(|sessions| {
                sessions.borrow_mut().insert(session_id, session); // CLI-parity: Use [u8; 32] key
            });

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
        Err(e) => serde_json::json!({
            "success": false,
            "error": e.to_string()
        })
        .to_string(),
    }
}

/// Generate a random master key using CLI-compatible logic
#[wasm_bindgen]
pub fn generate_random_master_key() -> String {
    // CLI-parity: Use OsRng like CLI does
    let mut key = [0u8; 32]; // KEY_LEN = 32 bytes like CLI
    rand::rngs::OsRng.fill_bytes(&mut key);

    // Convert to hex like CLI does
    let hex_key = bytes_to_hex(&key);

    hex_key
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
            return None;
        }

        // Export sessions with serialized session data (bincode + base64)
        let sessions_data: std::collections::HashMap<String, serde_json::Value> = sessions_ref
            .iter()
            .map(|(session_id, session)| {
                let session_id_hex = bytes_to_hex(session_id); // CLI-parity: Convert [u8; 32] to hex

                let session_bytes = match bincode::serialize(session) {
                    Ok(bytes) => base64::engine::general_purpose::STANDARD.encode(bytes),
                    Err(_e) => String::new(),
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
            Ok(json) => Some(json),
            Err(_e) => None,
        }
    })
}

/// Import sessions from localStorage with full restoration (CLI-compatible)
#[wasm_bindgen]
pub fn import_sessions_from_storage(sessions_json: &str) -> String {
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let sessions_data: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(sessions_json)?;

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
                            failed_count += 1;
                            continue;
                        }
                        let mut session_id = [0u8; 32];
                        session_id.copy_from_slice(&bytes);
                        session_id
                    }
                    Err(_e) => {
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
                            }
                            None => {
                                failed_count += 1;
                            }
                        }
                    } else {
                        failed_count += 1;
                    }
                } else {
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
        Err(e) => serde_json::json!({
            "success": false,
            "error": e.to_string()
        })
        .to_string(),
    }
}

/// Get active session for DAG execution (CLI-compatible)
#[wasm_bindgen]
pub fn get_active_session_for_execution(_master_key_hex: &str) -> String {
    SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();

        // Try to find any session (like CLI's approach)
        if sessions.is_empty() {
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
            let (_session_id, session) = sessions
                .iter_mut()
                .next()
                .ok_or("No sessions available for encryption")?;

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
                    }
                }
            }

            // CLI-parity: Commit session state (exactly like CLI)
            session.commit_sender(None);

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
        Err(e) => serde_json::json!({
            "success": false,
            "error": e.to_string()
        })
        .to_string(),
    }
}
